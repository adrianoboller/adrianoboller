//! O lado da REPLICA: puxar os eventos do source e aplicar aqui.
//!
//! # A direcao da conexao
//!
//! Quem procura e a replica; o source nao empurra nada. E o mesmo desenho do
//! MySQL(R), e ele existe por causa do firewall: o source abre UMA porta de
//! entrada para o IP da replica, e nao precisa alcancar a replica de volta.
//!
//! ```text
//!    REPLICA 192.168.50.20  ──── TCP 5000 ────►  SOURCE 10.1.1.102
//!         (quem procura)                            (quem responde)
//! ```
//!
//! # O laco
//!
//! 1. `posicao` no source: quantos eventos cada tabela tem, e o esquema dela;
//! 2. compara com a posicao local -- o evento N e a posicao N, entao a replica
//!    guarda UM numero por tabela e nao ha GTID a inventar;
//! 3. `replicar` a partir dali, em lotes;
//! 4. aplica com `Table::aplicar_evento`, que confere o rowid;
//! 5. dorme e repete.
//!
//! # A senha nao viaja
//!
//! A replica se autentica pelo mesmo desafio-resposta do resto do protocolo:
//! pede um nonce, calcula o HMAC com a chave derivada e manda a PROVA. No
//! `config.json` da replica mora o `senha_hash` -- o mesmo texto que ja mora
//! no cadastro de usuarios --, e dele sai a chave derivada sem nunca haver
//! senha em claro em lugar nenhum.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;
use phxsql_store::log::Operacao;

use crate::config::Origem;
use crate::valores::hex_para_bytes;

/// Quantos eventos puxar por vez.
///
/// Nao e so cortesia com a memoria: cada lote e uma resposta JSON inteira, com
/// a imagem de cada linha em hexadecimal. Um lote de dez mil linhas com anexo
/// seria uma resposta de dezenas de megabytes montada de uma vez dos dois
/// lados.
const LOTE: u64 = 500;

/// Uma conexao com o source, falando o JSON por linha da porta de dados.
pub struct Cliente {
    fluxo: TcpStream,
    leitor: BufReader<TcpStream>,
    token: String,
}

impl Cliente {
    pub fn conectar(host: &str, porta: u16, token: &str, espera: Duration) -> Result<Cliente> {
        let alvo = format!("{host}:{porta}");
        let fluxo = TcpStream::connect(&alvo)
            .map_err(|e| PhxError::Io(std::io::Error::other(format!("{alvo}: {e}"))))?;
        // Nagle segura a resposta em ate 40 ms, e aqui toda troca e um pedido
        // pequeno esperando resposta -- exatamente o caso em que ele so atrasa.
        let _ = fluxo.set_nodelay(true);
        fluxo.set_read_timeout(Some(espera))?;
        fluxo.set_write_timeout(Some(espera))?;
        let leitor = BufReader::new(fluxo.try_clone()?);
        Ok(Cliente {
            fluxo,
            leitor,
            token: token.to_string(),
        })
    }

    /// Manda um pedido e devolve o `resultado`, ou o erro que o source disse.
    pub fn pedir(&mut self, mut campos: Vec<(&str, Json)>) -> Result<Json> {
        if !self.token.is_empty() {
            campos.push(("token", Json::texto_de(self.token.clone())));
        }
        let linha = Json::objeto(campos).escrever();
        self.fluxo.write_all(linha.as_bytes())?;
        self.fluxo.write_all(b"\n")?;
        self.fluxo.flush()?;

        let mut resposta = String::new();
        if self.leitor.read_line(&mut resposta)? == 0 {
            return Err(PhxError::Io(std::io::Error::other(
                "o source fechou a conexao",
            )));
        }
        let j = Json::analisar(&resposta)?;
        if !j.booleano_ou("ok", false) {
            // O erro do outro lado ja vem classificado -- `nome` e `classe`
            // fazem parte da resposta. Reembalar tudo como "acesso negado"
            // fazia o log da replica dizer autorizacao para um database que
            // ainda nao existe, que e o pior tipo de mensagem: a que manda
            // procurar no lugar errado.
            let texto = format!(
                "{}: {}",
                j.texto_ou("op", "?"),
                j.texto_ou("erro", "o source recusou sem dizer o motivo")
            );
            return Err(match j.texto_ou("nome", "") {
                "NAO_ENCONTRADO" => PhxError::NaoEncontrado(texto),
                "ACESSO_NEGADO" => PhxError::Autorizacao(texto),
                "DUPLICADO" => PhxError::Duplicado(texto),
                "CORROMPIDO" => PhxError::Corrompido(texto),
                "TIPO_INVALIDO" => PhxError::Tipo(texto),
                "LIMITE_EXCEDIDO" => PhxError::LimiteExcedido(texto),
                "CONFLITO" => PhxError::Conflito(texto),
                _ => PhxError::Esquema(texto),
            });
        }
        Ok(j.campo("resultado").cloned().unwrap_or(Json::Nulo))
    }

    /// Desafio-resposta. `senha_hash` e o preferido; `senha` e o caminho de
    /// quem ainda nao trocou o `config.json`.
    pub fn autenticar(&mut self, usuario: &str, senha_hash: &str, senha: &str) -> Result<()> {
        let d = self.pedir(vec![
            ("op", Json::texto_de("desafio")),
            ("usuario", Json::texto_de(usuario)),
        ])?;
        let nonce = d.texto_ou("nonce", "").to_string();
        let sal = d.texto_ou("sal", "").to_string();
        let iteracoes = d.inteiro_ou("iteracoes", 0).max(0) as u32;
        let nonce_cliente = phxsql_core::desafio::nonce();

        let prova = if !senha_hash.is_empty() {
            // Do hash guardado sai a MESMA chave derivada que o source usa --
            // sem senha em claro em lado nenhum.
            let dk = phxsql_core::senha::derivado_do_hash(senha_hash)?;
            phxsql_core::desafio::calcular_prova(&dk, &nonce, &nonce_cliente, usuario)
        } else {
            phxsql_core::desafio::prova_de_senha(
                senha,
                &sal,
                iteracoes,
                &nonce,
                &nonce_cliente,
                usuario,
            )?
        };

        self.pedir(vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(usuario)),
            ("prova", Json::texto_de(prova)),
            ("nonce_cliente", Json::texto_de(nonce_cliente)),
        ])?;
        Ok(())
    }

    /// Os databases do source, para a origem que nao lista nenhum.
    pub fn databases(&mut self) -> Result<Vec<String>> {
        let r = self.pedir(vec![("op", Json::texto_de("bancos"))])?;
        Ok(r.campo("bancos")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .map(|b| match b {
                        Json::Texto(t) => t.clone(),
                        outro => outro.texto_ou("nome", "").to_string(),
                    })
                    .filter(|n| !n.is_empty())
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// O que o source diz sobre uma tabela.
pub struct NoSource {
    pub nome: String,
    pub eventos: u64,
    pub esquema: Option<Schema>,
}

/// Le a resposta de `posicao`.
pub fn posicao(cliente: &mut Cliente, database: &str) -> Result<(bool, Vec<NoSource>)> {
    let r = cliente.pedir(vec![
        ("op", Json::texto_de("posicao")),
        ("database", Json::texto_de(database)),
        ("com_esquema", Json::Bool(true)),
    ])?;
    let com_imagem = r.booleano_ou("imagem_da_linha", false);
    let mut saida = Vec::new();
    if let Some(Json::Objeto(pares)) = r.campo("tabelas") {
        for (nome, v) in pares {
            let hex = v.texto_ou("esquema", "");
            let esquema = if hex.is_empty() {
                None
            } else {
                Some(Schema::desserializar(&hex_para_bytes(hex)?)?)
            };
            saida.push(NoSource {
                nome: nome.clone(),
                eventos: v.inteiro_ou("eventos", 0).max(0) as u64,
                esquema,
            });
        }
    }
    Ok((com_imagem, saida))
}

/// Um evento vindo do source, pronto para aplicar.
pub struct EventoRecebido {
    pub operacao: Operacao,
    pub rowid: u64,
    pub imagem: Vec<u8>,
}

/// Puxa ate `LOTE` eventos a partir de `desde`.
pub fn puxar(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
) -> Result<Vec<EventoRecebido>> {
    let r = cliente.pedir(vec![
        ("op", Json::texto_de("replicar")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("desde", Json::de_u64(desde)),
        ("max", Json::de_u64(LOTE)),
    ])?;
    let mut saida = Vec::new();
    for e in r.campo("eventos").and_then(Json::lista).unwrap_or(&[]) {
        saida.push(EventoRecebido {
            operacao: match e.texto_ou("operacao", "") {
                "inclusao" => Operacao::Inclusao,
                "alteracao" => Operacao::Alteracao,
                "exclusao" => Operacao::Exclusao,
                outro => {
                    return Err(PhxError::Corrompido(format!(
                        "o source mandou operacao desconhecida: {outro:?}"
                    )))
                }
            },
            rowid: e.inteiro_ou("rowid", 0).max(0) as u64,
            imagem: hex_para_bytes(e.texto_ou("imagem", ""))?,
        });
    }
    Ok(saida)
}

/// Abre a conexao e entra autenticado. Usado pelo laco e pelos testes.
pub fn ligar(origem: &Origem) -> Result<Cliente> {
    let mut c = Cliente::conectar(
        &origem.host,
        origem.porta,
        &origem.token,
        Duration::from_secs(30),
    )?;
    if !origem.usuario.is_empty() {
        c.autenticar(&origem.usuario, &origem.senha_hash, &origem.senha)?;
    }
    Ok(c)
}

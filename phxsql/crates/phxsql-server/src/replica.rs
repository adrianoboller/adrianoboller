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

use std::io::{BufRead, BufReader, Read, Write};
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

/// O teto de UMA resposta do source, em bytes.
///
/// # Por que ele passou a existir
///
/// Enquanto o lote era lido COM a trava de dados na mao, o tamanho dele era o
/// menor dos problemas. Agora que ele e lido antes -- e mora inteiro na
/// memoria da replica ate a trava chegar --, «quanto isso pode crescer» virou
/// uma pergunta com resposta obrigatoria, e a resposta nao pode ser «o que o
/// outro lado mandar»: `read_line` sem teto aceita uma linha do tamanho da
/// memoria da maquina, e quem escolhe o tamanho e o outro lado.
///
/// # A conta
///
/// [`LOTE`] eventos por resposta, e a imagem de cada linha viaja em
/// hexadecimal -- dois caracteres por byte. O source corta o lote em
/// [`crate::servidor::TETO_DO_LOTE_SERVIDO`] bytes de imagem, o que da
/// ~32 MiB de texto mais o enfeite do JSON. Este teto e o DOBRO disso, para
/// que um par sadio nunca encoste nele e ele sirva so para o que ele existe:
/// impedir que a replica aloque sem limite por ordem de quem esta do outro
/// lado do fio.
///
/// Uma linha unica maior que isto nao passa, e a recusa diz o numero -- o que
/// e melhor que o `Killed` do nucleo, que nao diz nada.
const TETO_DA_RESPOSTA: u64 = 128 * 1024 * 1024;

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
        Cliente::montar(fluxo, token, espera)
    }

    /// Conecta com PRAZO. O `connect` sem prazo pode ficar minutos pendurado
    /// num host que caiu -- para a replicacao isso e tolerável, para o pulso
    /// do cluster nao: um no morto seguraria a conferencia dos vivos.
    pub fn conectar_com_prazo(
        host: &str,
        porta: u16,
        token: &str,
        espera: Duration,
        prazo_conexao: Duration,
    ) -> Result<Cliente> {
        use std::net::ToSocketAddrs;
        let alvo = format!("{host}:{porta}");
        let endereco = alvo
            .to_socket_addrs()
            .map_err(|e| PhxError::Io(std::io::Error::other(format!("{alvo}: {e}"))))?
            .next()
            .ok_or_else(|| PhxError::Io(std::io::Error::other(format!("{alvo}: sem endereco"))))?;
        let fluxo = TcpStream::connect_timeout(&endereco, prazo_conexao)
            .map_err(|e| PhxError::Io(std::io::Error::other(format!("{alvo}: {e}"))))?;
        Cliente::montar(fluxo, token, espera)
    }

    fn montar(fluxo: TcpStream, token: &str, espera: Duration) -> Result<Cliente> {
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

        // Le no MAXIMO `TETO_DA_RESPOSTA` + 1 byte: o `+1` e o que separa
        // "coube" de "estourou" sem ter de contar o que ainda vem. Passou do
        // teto, a conexao fica no meio de uma linha e nao serve mais -- por
        // isso a recusa e um erro, que faz a rodada inteira voltar e a
        // proxima abrir uma conexao nova.
        let mut resposta = String::new();
        let lidos = {
            let mut limitado = (&mut self.leitor).take(TETO_DA_RESPOSTA + 1);
            limitado.read_line(&mut resposta)?
        };
        if lidos == 0 {
            return Err(PhxError::Io(std::io::Error::other(
                "o source fechou a conexao",
            )));
        }
        if lidos as u64 > TETO_DA_RESPOSTA {
            return Err(PhxError::LimiteExcedido(format!(
                "o source mandou mais de {} MiB numa resposta so, e a replica \
                 nao guarda um lote desse tamanho na memoria; baixe o tamanho \
                 do lote do lado do source ou parta a tabela",
                TETO_DA_RESPOSTA / (1024 * 1024)
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
                // Os dois nomes caem no MESMO erro: "escrita na replica" e
                // "redireciona" sempre foram o mesmo evento com nomes
                // diferentes, e um servidor de versao anterior ainda manda o
                // nome antigo.
                "REDIRECIONA" | "ESCRITA_NA_REPLICA" => PhxError::Redireciona(texto),
                "SPARE_EM_ESPERA" => PhxError::SpareEmEspera(texto),
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
    ///
    /// O `bancos` responde uma LISTA direta -- e este leitor procurava um
    /// campo `"bancos"` que nunca existiu. Consequencia: origem com
    /// `databases: []` (= todos) nao replicava NADA, em silencio, e ninguem
    /// viu porque a bancada de replicacao sempre fixou a lista. Foi o laco do
    /// cluster, que depende de descobrir os databases sozinho, que pisou aqui
    /// primeiro. Os dois formatos ficam aceitos, para um source antigo ou
    /// novo responderem igual.
    pub fn databases(&mut self) -> Result<Vec<String>> {
        let r = self.pedir(vec![("op", Json::texto_de("bancos"))])?;
        let lista = r
            .lista()
            .or_else(|| r.campo("bancos").and_then(Json::lista));
        Ok(lista
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

/// O que o source diz sobre um database inteiro.
pub struct PosicaoDoSource {
    pub com_imagem: bool,
    /// O `id_servidor` do source -- e com ele que o bidirecional confere a
    /// colisao de hash antes de confiar na supressao de origem.
    pub id_servidor: String,
    pub tabelas: Vec<NoSource>,
}

/// Le a resposta de `posicao`.
pub fn posicao(cliente: &mut Cliente, database: &str) -> Result<PosicaoDoSource> {
    let r = cliente.pedir(vec![
        ("op", Json::texto_de("posicao")),
        ("database", Json::texto_de(database)),
        ("com_esquema", Json::Bool(true)),
    ])?;
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
    Ok(PosicaoDoSource {
        com_imagem: r.booleano_ou("imagem_da_linha", false),
        id_servidor: r.texto_ou("id_servidor", "").to_string(),
        tabelas: saida,
    })
}

/// Um evento vindo do source, pronto para aplicar.
pub struct EventoRecebido {
    pub operacao: Operacao,
    pub rowid: u64,
    pub imagem: Vec<u8>,
    /// O instante em que a escrita NASCEU, no relogio de quem a fez.
    pub carimbo_ms: i64,
    /// [`crate::bidirecional::hash_id`] do servidor onde a escrita nasceu.
    pub origem: u16,
}

/// Um lote do `replicar`, com a posicao ATE ONDE o source andou.
///
/// `ate` pode passar da conta dos eventos: no bidirecional o source suprime
/// os eventos cuja origem e quem pediu, e a posicao anda por cima deles.
pub struct LoteRecebido {
    pub eventos: Vec<EventoRecebido>,
    pub ate: u64,
    pub fim: bool,
}

/// Puxa ate `LOTE` eventos a partir de `desde`.
pub fn puxar(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
) -> Result<Vec<EventoRecebido>> {
    Ok(puxar_lote(cliente, database, tabela, desde, None)?.eventos)
}

/// O mesmo que [`puxar`], dizendo QUEM pede -- e devolvendo a posicao.
///
/// `para` e o `id_servidor` de quem puxa: o source nao devolve os eventos que
/// nasceram nele, que e o que mata o laco do bidirecional.
pub fn puxar_lote(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
    para: Option<&str>,
) -> Result<LoteRecebido> {
    let mut campos = vec![
        ("op", Json::texto_de("replicar")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("desde", Json::de_u64(desde)),
        ("max", Json::de_u64(LOTE)),
    ];
    if let Some(quem) = para {
        campos.push(("para", Json::texto_de(quem)));
    }
    let r = cliente.pedir(campos)?;
    let mut eventos = Vec::new();
    for e in r.campo("eventos").and_then(Json::lista).unwrap_or(&[]) {
        eventos.push(EventoRecebido {
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
            carimbo_ms: e.inteiro_ou("carimbo_ms", 0),
            origem: e.inteiro_ou("origem", 0).clamp(0, u16::MAX as i64) as u16,
        });
    }
    Ok(LoteRecebido {
        eventos,
        ate: r.inteiro_ou("ate", desde as i64).max(0) as u64,
        fim: r.booleano_ou("fim", true),
    })
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

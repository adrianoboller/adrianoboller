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

use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;
use phxsql_store::log::Operacao;

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido};

use crate::config::Origem;
use crate::prazo::{self, ComPrazo, Prazo};
use crate::valores::hex_para_bytes;

/// Quantos eventos puxar por vez.
///
/// Nao e so cortesia com a memoria: cada lote e uma resposta JSON inteira, com
/// a imagem de cada linha em hexadecimal. Um lote de dez mil linhas com anexo
/// seria uma resposta de dezenas de megabytes montada de uma vez dos dois
/// lados.
const LOTE: u64 = 500;

// O teto de um registro lido do fio NAO mora mais aqui: ele desceu para o
// `Canal` do `phxsql-core` (`TETO_DO_REGISTRO`), porque a leitura passou a
// ser dele. Um teto nesta camada voltaria a deixar o caminho cifrado sem
// nenhum -- que foi exatamente o risco desta integracao.

/// O soquete de quem conversa: claro ou TLS, pelo motor do core (pedido 572,
/// T6b-2). O `Canal` por cima nao sabe qual dos dois e -- dentro do TLS ele
/// fica `Claro`, porque a linha ja viaja protegida.
use phxsql_core::tls::FioDeCliente as Fio;

/// Uma conexao com o source, falando o JSON por linha da porta de dados.
pub struct Cliente {
    fluxo: Fio,
    leitor: BufReader<Fio>,
    token: String,
    /// Em claro (como sempre foi) ou dentro do tunel. Ver
    /// `docs/CIFRA-DO-FIO.md`.
    canal: Canal,
    /// O `id_servidor` de quem puxa, quando ele quer SEGURAR o diario da
    /// origem -- pedido 706. Viaja como `consumidor` em todo `replicar`; a
    /// origem so o conta se ele estiver em `diario.consumidores` de la.
    consumidor: Option<String>,
    /// Ate onde o diario DESTA replica ja foi ao disco na tabela do proximo
    /// `replicar` -- pedido 706. O `desde` conta a cauda aplicada nesta
    /// rodada e ainda sem `fsync`; o `duravel` nao. Vale para UM pedido.
    duravel: Option<u64>,
}

/// O `close_notify` sai no `Drop`, como no `fio_dados` do servidor: o
/// cliente vive em lacos com dezenas de saidas, e a que alguem esquecesse
/// fecharia sem ele -- o source veria um corte em vez de um adeus.
impl Drop for Cliente {
    fn drop(&mut self) {
        self.fluxo.despedir();
    }
}

impl Cliente {
    /// Conecta com o prazo padrao de conexao, [`PRAZO_DE_CONEXAO`], e o
    /// prazo da conversa de [`prazo_da_conversa`]: `espera` e o silencio, e o
    /// total por pedido sai dele.
    ///
    /// Ate 17/09/2026 isto era um `TcpStream::connect` cru, que fica
    /// pendurado ate o sistema desistir do SYN (no Linux de fabrica, seis
    /// retransmissoes, perto de 127 s). O prazo tinha entrado so para o pulso
    /// do cluster, em [`Cliente::conectar_com_prazo`], e os IRMAOS ficaram sem
    /// ele: o laco da replica e a sonda `replicacao_testar` (via `ligar`), o
    /// dblink para outro PhxSql e o console de linha de comando -- todos
    /// chamavam esta funcao (o dblink saiu para [`Cliente::conectar_com_total`]
    /// no pedido 578). Revisao SEC, A5. Ela continua existindo em vez de
    /// ser apagada porque os tres chamadores nao tem prazo proprio a dizer; o
    /// que nao existe mais e um caminho SEM prazo -- nem de conexao, nem, desde
    /// o pedido 580, de conversa.
    pub fn conectar(host: &str, porta: u16, token: &str, espera: Duration) -> Result<Cliente> {
        Cliente::conectar_com_prazo(
            host,
            porta,
            token,
            prazo_da_conversa(espera),
            PRAZO_DE_CONEXAO,
        )
    }

    /// Conecta com os prazos ditos por quem chama: o da conversa (silencio e
    /// total por pedido) e o do `connect`. O pulso do cluster usa 1-2 s de
    /// conexao: um no morto nao pode segurar a conferencia dos vivos alem do
    /// proprio pulso.
    pub fn conectar_com_prazo(
        host: &str,
        porta: u16,
        token: &str,
        prazo: Prazo,
        prazo_conexao: Duration,
    ) -> Result<Cliente> {
        Cliente::abrir(host, porta, token, prazo, prazo_conexao)
    }

    /// Conecta com prazo TOTAL por pedido e o `connect` de fabrica -- pedido
    /// 578, o caminho do DbLink para outro PhxSql.
    pub fn conectar_com_total(
        host: &str,
        porta: u16,
        token: &str,
        prazo: Prazo,
    ) -> Result<Cliente> {
        Cliente::abrir(host, porta, token, prazo, PRAZO_DE_CONEXAO)
    }

    fn abrir(
        host: &str,
        porta: u16,
        token: &str,
        prazo: Prazo,
        prazo_conexao: Duration,
    ) -> Result<Cliente> {
        use std::net::ToSocketAddrs;
        let alvo = format!("{host}:{porta}");
        // O `ErrorKind` do sistema sobrevive ao embrulho: e por ele que a sonda
        // `replicacao_testar` classifica a falha (recusada, prazo, sem rota)
        // sem devolver o texto cru do sistema operacional -- revisao SEC de
        // 17/09/2026, A5. `Error::other` apagava o tipo e obrigava a
        // classificar pela frase, que se compara por chave e nunca por texto.
        let endereco = alvo
            .to_socket_addrs()
            .map_err(|e| PhxError::Io(std::io::Error::new(e.kind(), format!("{alvo}: {e}"))))?
            .next()
            .ok_or_else(|| {
                PhxError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{alvo}: sem endereco"),
                ))
            })?;
        let fluxo = TcpStream::connect_timeout(&endereco, prazo_conexao)
            .map_err(|e| PhxError::Io(std::io::Error::new(e.kind(), format!("{alvo}: {e}"))))?;
        Cliente::montar(fluxo, token, prazo)
    }

    fn montar(fluxo: TcpStream, token: &str, prazo: Prazo) -> Result<Cliente> {
        // Nagle segura a resposta em ate 40 ms, e aqui toda troca e um pedido
        // pequeno esperando resposta -- exatamente o caso em que ele so atrasa.
        let _ = fluxo.set_nodelay(true);
        // O relogio do total comeca DEPOIS do `connect`, que tem prazo
        // proprio por endereco.
        let (leitura, fluxo) = ComPrazo::armar(fluxo, prazo.rearmado())?;
        let leitor = BufReader::new(Fio::Claro(leitura));
        Ok(Cliente {
            fluxo: Fio::Claro(fluxo),
            leitor,
            token: token.to_string(),
            canal: Canal::Claro,
            consumidor: None,
            duravel: None,
        })
    }

    /// Diz a origem quem puxa, para ela segurar o diario ate este consumidor
    /// confirmar -- pedido 706. Sem isto o `replicar` sai como sempre saiu.
    pub fn dizer_quem_puxa(&mut self, id_servidor: &str) {
        let id = id_servidor.trim();
        self.consumidor = (!id.is_empty()).then(|| id.to_string());
    }

    /// Diz a origem, so no PROXIMO `replicar`, ate onde o diario daqui ja
    /// esta no disco (pedido 706). Sem consumidor declarado nao viaja.
    pub fn dizer_o_duravel(&mut self, duravel: u64) {
        self.duravel = Some(duravel);
    }

    /// Pede o aperto de mao e passa a falar por dentro do tunel.
    ///
    /// `pino` e a chave publica que se ESPERA do source. Com pino, um source
    /// que apresente outra chave derruba a conexao -- e e assim que a replica
    /// se protege de quem esta no meio. Sem pino, o tunel protege da escuta
    /// PASSIVA e nada mais, porque o atacante apresenta a chave dele e nao ha
    /// com o que comparar.
    ///
    /// Devolve a chave que o source apresentou, para quem quiser anota-la.
    pub fn cifrar(&mut self, pino: Option<[u8; 32]>) -> Result<[u8; 32]> {
        self.so_uma_cifra()?;
        self.rearmar();
        let (iniciador, m1) = Iniciador::comecar(pino);
        let pedido = Json::objeto(vec![
            ("op", Json::texto_de("cifrar")),
            ("e", Json::texto_de(base64::codificar(&m1))),
        ])
        .escrever();
        self.fluxo
            .write_all(pedido.as_bytes())
            .and_then(|_| self.fluxo.write_all(b"\n"))
            .and_then(|_| self.fluxo.flush())
            .map_err(|e| prazo::classificar(e, PhxError::Io))?;

        // O TETO vale aqui tambem -- pedido 312. Esta leitura acontece antes
        // de existir tunel e antes de qualquer autenticacao, e durante muito
        // tempo ela foi um `read_line` cru: medido, um source falso empurrou
        // 192 MiB numa linha so, em 294 ms, e esta replica guardou tudo. O
        // canal ainda e `Claro` neste ponto, entao quem le e o mesmo `Canal`
        // de sempre -- so com o teto do APERTO, que e curto de proposito.
        let resposta = match self
            .canal
            .ler_ate(&mut self.leitor, phxsql_core::fio::TETO_DO_APERTO)
            .map_err(prazo::reclassificar)?
        {
            Recebido::Linha(l) => l,
            Recebido::Fim => {
                return Err(PhxError::Io(std::io::Error::other(
                    "o source fechou a conexao no aperto de mao",
                )))
            }
        };
        let j = Json::analisar(&resposta)?;
        if !j.booleano_ou("ok", false) {
            return Err(PhxError::Autorizacao(format!(
                "o source recusou o aperto de mao: {}",
                j.texto_ou("erro", "sem motivo")
            )));
        }
        let m2 = base64::decodificar(
            j.campo("resultado")
                .map(|r| r.texto_ou("m2", ""))
                .unwrap_or(""),
        )?;
        let (transporte, apresentada) = iniciador.terminar(&m2)?;
        self.canal = Canal::Cifrado(Box::new(transporte));
        Ok(apresentada)
    }

    /// Passa a falar TLS 1.3 com o source, autenticado pelo PINO do
    /// certificado dele (`sha256//...` do SPKI, a forma do curl) -- pedido
    /// 572, T6b-2. O lugar do [`Cliente::cifrar`] (Noise) para quem escreveu
    /// `pino_tls` na configuracao: o servidor decide pelo primeiro byte
    /// (`0x16`) e atende os dois na mesma porta.
    ///
    /// So por pino, e de proposito: entre dois PhxSql nao ha autoridade
    /// certificadora a consultar, e TLS sem conferir quem responde protegeria
    /// da escuta passiva e de nada mais -- que e o que o Noise sem pino ja da.
    /// A cadeia e o nome sao a T6c, para falar com servidores de fora.
    pub fn cifrar_tls(&mut self, pino: [u8; 32]) -> Result<()> {
        self.so_uma_cifra()?;
        Fio::passar_a_tls(&mut self.leitor, &mut self.fluxo, pino).map_err(prazo::reclassificar)?;
        Ok(())
    }

    /// A decisao UNICA de como proteger a conexao, para todo iniciador que
    /// usa este cliente (replica, pulso e propagacao do cluster, sonda):
    /// com `pino_tls` e TLS 1.3 por aquele pino, e a `cifra` nao decide mais;
    /// sem ele, o Noise de sempre quando `cifra` pede. Espalhar o `if` por
    /// cada chamador deixaria o que alguem esquecesse falando Noise com o
    /// pino TLS escrito na configuracao -- e o operador achando que nao.
    pub fn proteger(
        &mut self,
        cifra: bool,
        pino_do_fio: Option<[u8; 32]>,
        pino_tls: Option<[u8; 32]>,
    ) -> Result<()> {
        match pino_tls {
            Some(p) => self.cifrar_tls(p),
            None if cifra => self.cifrar(pino_do_fio).map(|_| ()),
            None => Ok(()),
        }
    }

    /// Uma cifra por conexao: Noise dentro de TLS (ou o contrario) seria a
    /// mesma protecao paga duas vezes, e um segundo aperto no meio de uma
    /// conexao cifrada confundiria o servidor sobre qual transcricao amarrar.
    fn so_uma_cifra(&self) -> Result<()> {
        if self.canal.cifrado() || self.fluxo.tls() {
            return Err(PhxError::Esquema(
                "esta conexao ja esta cifrada: uma cifra por conexao".into(),
            ));
        }
        Ok(())
    }

    /// `true` quando a conversa passa por TLS.
    pub fn tls(&self) -> bool {
        self.fluxo.tls()
    }

    /// Cada pedido tem o prazo total inteiro, quando ha total -- pedido 578.
    /// Sem total e um `Copy` de tres campos, e o soquete nem e tocado.
    fn rearmar(&mut self) {
        Fio::rearmar(self.leitor.get_mut(), &mut self.fluxo);
    }

    /// A transcricao do aperto, quando esta conexao passou pelo tunel -- ou,
    /// no TLS, o `tls-exporter` da RFC 9266, que faz o mesmo papel.
    ///
    /// Quem a usa e a prova de identidade do pulso (pedido 278), pelo mesmo
    /// motivo que o desafio-resposta ja a usa: uma prova gravada numa conexao
    /// nao pode valer em outra.
    pub fn transcricao(&self) -> Option<[u8; 32]> {
        self.fluxo
            .vinculo_do_canal()
            .or_else(|| self.canal.transcricao())
    }

    /// Manda um pedido e devolve o `resultado`, ou o erro que o source disse.
    pub fn pedir(&mut self, campos: Vec<(&str, Json)>) -> Result<Json> {
        let resposta = self.trocar(campos, phxsql_core::fio::TETO_DO_REGISTRO, &mut false)?;
        Cliente::desembrulhar(&resposta)
    }

    /// Manda um pedido e devolve a linha CRUA da resposta, lida com o teto de
    /// quem chama -- pedido 610. `None` quer dizer que a linha passou do teto,
    /// e a conexao ficou no meio dela: nao se reaproveita.
    ///
    /// Existe porque o teto de bytes do DbLink (546) pesava a COPIA, e a copia
    /// so nasce depois de `Json::analisar` montar a arvore inteira -- que custa
    /// de 16 a 32 vezes a linha. Quem tem orcamento proprio precisa pesar a
    /// linha ANTES de analisa-la, e para isso precisa dela crua. O `None`, e
    /// nao um erro, e o que deixa quem chama dizer a recusa com o teto DELE
    /// (`max_mib`) sem comparar frase: o `LimiteExcedido` do outro lado so
    /// aparece depois da analise, entao aqui ele nunca se confunde com este.
    pub fn pedir_cru(&mut self, campos: Vec<(&str, Json)>, teto: u64) -> Result<Option<String>> {
        let mut estourou = false;
        match self.trocar(campos, teto, &mut estourou) {
            Ok(l) => Ok(Some(l)),
            Err(_) if estourou => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Escreve o pedido e le a linha da resposta, com `teto`. `estourou` vira
    /// `true` quando a linha passou dele -- pela pergunta do proprio `Canal`,
    /// que so a faz nesse caso.
    fn trocar(
        &mut self,
        mut campos: Vec<(&str, Json)>,
        teto: u64,
        estourou: &mut bool,
    ) -> Result<String> {
        if !self.token.is_empty() {
            campos.push(("token", Json::texto_de(self.token.clone())));
        }
        let linha = Json::objeto(campos).escrever();
        self.rearmar();
        self.canal
            .escrever(&mut self.fluxo, &linha)
            .map_err(prazo::reclassificar)?;

        match self
            .canal
            .ler_decidindo(&mut self.leitor, teto, &mut || {
                *estourou = true;
                teto
            })
            .map_err(prazo::reclassificar)?
        {
            Recebido::Linha(l) => Ok(l),
            Recebido::Fim => Err(PhxError::Io(std::io::Error::other(
                "o source fechou a conexao",
            ))),
        }
    }

    /// A resposta analisada: o `resultado`, ou o erro que o source disse.
    pub fn desembrulhar(resposta: &str) -> Result<Json> {
        let j = Json::analisar(resposta)?;
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

        // Amarracao ao canal: quando a replica fala por dentro do tunel, a
        // prova nasce presa a transcricao do aperto, e o source a confere
        // contra a dele -- e o que derruba um homem-no-meio que tenha
        // terminado o tunel. Sem tunel (`None`), e a prova de sempre. Ver
        // `docs/CIFRA-DO-FIO.md` §10.
        let transcricao = self.transcricao();
        let canal_ref = transcricao.as_ref().map(|t| &t[..]);

        let prova = if !senha_hash.is_empty() {
            // Do hash guardado sai a MESMA chave derivada que o source usa --
            // sem senha em claro em lado nenhum.
            let dk = phxsql_core::senha::derivado_do_hash(senha_hash)?;
            phxsql_core::desafio::calcular_prova(&dk, &nonce, &nonce_cliente, usuario, canal_ref)
        } else {
            phxsql_core::desafio::prova_de_senha(
                senha,
                &sal,
                iteracoes,
                &nonce,
                &nonce_cliente,
                usuario,
                canal_ref,
            )?
        };

        let mut campos = vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(usuario)),
            ("prova", Json::texto_de(prova)),
            ("nonce_cliente", Json::texto_de(nonce_cliente)),
        ];
        if canal_ref.is_some() {
            campos.push(("amarrar_canal", Json::Bool(true)));
        }
        self.pedir(campos)?;
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
    /// O PROXIMO numero que a `Sequence` do source vai entregar (pedido 229,
    /// c-pleno). Zero = o source nao disse, ou a tabela nao tem `Sequence`.
    /// Viaja no `posicao`, que a replica pergunta ANTES de puxar eventos: o
    /// contador chega mesmo quando os eventos ainda nao chegaram, e e esse
    /// descompasso que a promocao de uma replica atrasada pagava.
    pub proxima_sequencia: u64,
}

/// O contador da sequencia como vai ao fio: numero ate 2^53, TEXTO acima.
/// O `Json` da casa so tem `f64`, e um contador arredondado seria o numero de
/// um vizinho -- o mesmo crivo do `valor_para_json` para o id gravado.
pub fn proxima_sequencia_para_o_fio(n: u64) -> Json {
    if n >= phxsql_core::json::INTEIRO_EXATO_MAX {
        Json::texto_de(n.to_string())
    } else {
        Json::de_u64(n)
    }
}

/// O inverso de [`proxima_sequencia_para_o_fio`]. Um UNICO leitor para o
/// `posicao` e para o lote do quorum: dois leitores do mesmo campo divergiriam
/// no primeiro formato novo. Torto ou ausente vira zero (`nao disse`), que a
/// adocao ignora -- nunca um numero inventado.
pub fn proxima_sequencia_do_fio(v: Option<&Json>) -> u64 {
    match v {
        Some(Json::Texto(t)) => t.parse().unwrap_or(0),
        Some(j) => match j.inteiro() {
            Some(n) if n > 0 => n as u64,
            _ => 0,
        },
        None => 0,
    }
}

/// O id de transacao como vai ao fio: SEMPRE texto. Ele passa de 2^53 desde
/// o primeiro numero (o piso e o relogio em ms deslocado 16 bits), e o `Json`
/// da casa so tem `f64` -- um id arredondado juntaria transacoes vizinhas.
pub fn tx_para_o_fio(tx: u64) -> Json {
    Json::texto_de(tx.to_string())
}

/// O inverso de [`tx_para_o_fio`]. Ausente, torto ou zero = «sem id».
pub fn tx_do_fio(v: Option<&Json>) -> u64 {
    match v {
        Some(Json::Texto(t)) => t.parse().unwrap_or(0),
        Some(j) => j.inteiro().filter(|n| *n > 0).unwrap_or(0) as u64,
        None => 0,
    }
}

/// O que o source diz sobre um database inteiro.
pub struct PosicaoDoSource {
    /// O source grava o id de transacao no diario e o manda no fio (pedido
    /// 676). Falso = source anterior: a replica aplica evento a evento, e a
    /// venda de varias tabelas pode aparecer pela metade enquanto chega.
    pub com_tx: bool,
    pub com_imagem: bool,
    /// O `id_servidor` do source -- e com ele que o bidirecional confere a
    /// colisao de hash antes de confiar na supressao de origem.
    pub id_servidor: String,
    /// O numero de origem EFETIVO do source (pedido 329). Zero quando o source
    /// e de antes do campo -- e ai quem le usa o hash do id, que e o numero
    /// que ele de fato grava (`bidirecional::numero_do_servidor`).
    pub numero_servidor: u16,
    pub tabelas: Vec<NoSource>,
    /// O primeiro evento que a origem ainda guarda de cada tabela -- pedido
    /// 706. Zero, ou ausente numa origem anterior, e «tudo desde o comeco».
    pub bases: Vec<(String, u64)>,
    /// Quantas tabelas do database a origem deixou de fora porque o usuario
    /// da replicacao nao as alcanca -- pedido 299, R5. Zero numa origem
    /// anterior ao campo.
    pub fora_do_escopo: u64,
}

/// O nome do campo que diz quantas tabelas ficaram fora do escopo -- pedido
/// 299, R5. UM nome para quem escreve (o `posicao` da origem), quem le (a
/// replica) e quem mostra (`replicacao_estado`): dois literais divergiriam no
/// dia em que alguem renomeasse um.
pub const CAMPO_FORA_DO_ESCOPO: &str = "tabelas_fora_do_escopo";

/// Le a resposta de `posicao`.
pub fn posicao(cliente: &mut Cliente, database: &str) -> Result<PosicaoDoSource> {
    let r = cliente.pedir(vec![
        ("op", Json::texto_de("posicao")),
        ("database", Json::texto_de(database)),
        ("com_esquema", Json::Bool(true)),
    ])?;
    let mut saida = Vec::new();
    let mut bases = Vec::new();
    if let Some(Json::Objeto(pares)) = r.campo("tabelas") {
        for (nome, v) in pares {
            bases.push((nome.clone(), v.inteiro_ou("base", 0).max(0) as u64));
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
                proxima_sequencia: proxima_sequencia_do_fio(v.campo("proxima_sequencia")),
            });
        }
    }
    Ok(PosicaoDoSource {
        com_tx: r.booleano_ou("tx_no_diario", false),
        com_imagem: r.booleano_ou("imagem_da_linha", false),
        id_servidor: r.texto_ou("id_servidor", "").to_string(),
        // Fora da faixa vira zero, que e «nao disse»: um numero inventado por
        // corte pertenceria a outro servidor.
        numero_servidor: u16::try_from(r.inteiro_ou("numero_servidor", 0)).unwrap_or(0),
        tabelas: saida,
        bases,
        fora_do_escopo: r.inteiro_ou(CAMPO_FORA_DO_ESCOPO, 0).max(0) as u64,
    })
}

/// Um evento vindo do source, pronto para aplicar.
pub struct EventoRecebido {
    pub operacao: Operacao,
    pub rowid: u64,
    /// Versao do registro depois da operacao -- um dos quatro campos que a
    /// conferencia de continuidade compara com o diario daqui.
    pub versao: u64,
    pub imagem: Vec<u8>,
    /// O instante em que a escrita NASCEU, no relogio de quem a fez.
    pub carimbo_ms: i64,
    /// [`crate::bidirecional::hash_id`] do servidor onde a escrita nasceu.
    pub origem: u16,
    /// A posicao deste evento no diario da ORIGEM -- o nosso equivalente do
    /// LSN que o `ALTER SUBSCRIPTION ... SKIP` do PostgreSQL recebe.
    ///
    /// # Por que ela vem do source, e nao se conta aqui
    ///
    /// Porque `desde + indice_na_lista` esta ERRADO no bidirecional: o source
    /// suprime os eventos cuja origem e quem pediu, e a posicao anda por cima
    /// deles (ver [`LoteRecebido`]). Contar aqui daria uma posicao menor que a
    /// verdadeira, e o `replicacao_pular` andaria para o lugar errado --
    /// pulando um evento inocente e deixando o culpado no caminho.
    ///
    /// `u64::MAX` = o source e velho demais para dizer onde o evento mora. A
    /// operacao de pular RECUSA nesse caso, nomeando: adivinhar a posicao e
    /// pular dado alheio em silencio.
    pub posicao: u64,
    /// O id da transacao que gravou o evento NA ORIGEM -- pedido 676. Zero =
    /// sem id (volume velho do `.log`, ou source anterior ao campo): o evento
    /// e aplicado sozinho, como sempre foi.
    pub tx: u64,
}

/// O valor de [`EventoRecebido::posicao`] quando o source nao a informa.
pub const POSICAO_DESCONHECIDA: u64 = u64::MAX;

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
/// `para` e o `id_servidor` de quem puxa e o numero de origem dele: o source
/// nao devolve os eventos que nasceram nele, que e o que mata o laco do
/// bidirecional. O numero viaja junto (pedido 329) porque, ATRIBUIDO, ele nao
/// se deduz do id do outro lado.
pub fn puxar_lote(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
    para: Option<(&str, u16)>,
) -> Result<LoteRecebido> {
    puxar_ate(cliente, database, tabela, desde, para, LOTE)
}

/// UM evento do source -- o da posicao `qual` -- ou nenhum, quando o diario
/// de la nao chega ate ele.
///
/// E a conferencia de continuidade no caso em que a replica nao tem nada a
/// aplicar: o evento que ela ja tem, pedido de novo ao source, para saber se
/// o diario de la ainda e o daqui.
pub fn puxar_um(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    qual: u64,
) -> Result<Option<EventoRecebido>> {
    Ok(puxar_ate(cliente, database, tabela, qual, None, 1)?
        .eventos
        .into_iter()
        .next())
}

fn puxar_ate(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
    para: Option<(&str, u16)>,
    max: u64,
) -> Result<LoteRecebido> {
    let mut campos = vec![
        ("op", Json::texto_de("replicar")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("desde", Json::de_u64(desde)),
        ("max", Json::de_u64(max)),
    ];
    if let Some((quem, numero)) = para {
        campos.push(("para", Json::texto_de(quem)));
        campos.push(("para_numero", Json::de_u64(numero as u64)));
    }
    if let Some(quem) = &cliente.consumidor {
        campos.push(("consumidor", Json::texto_de(quem)));
    }
    if let (Some(_), Some(d)) = (&cliente.consumidor, cliente.duravel.take()) {
        campos.push(("duravel", Json::de_u64(d)));
    }
    let r = cliente.pedir(campos)?;
    let eventos = eventos_do_fio(r.campo("eventos"))?;
    Ok(LoteRecebido {
        eventos,
        ate: r.inteiro_ou("ate", desde as i64).max(0) as u64,
        fim: r.booleano_ou("fim", true),
    })
}

/// A lista `eventos` do fio, lida -- UM leitor para o `replicar` de sempre e
/// para o lote do quorum (pedido 207), que viaja no `replicar_aguardar` com
/// a mesma forma: dois leitores da mesma lista divergiriam no primeiro campo
/// novo.
pub fn eventos_do_fio(lista: Option<&Json>) -> Result<Vec<EventoRecebido>> {
    let mut eventos = Vec::new();
    for e in lista.and_then(Json::lista).unwrap_or(&[]) {
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
            versao: e.inteiro_ou("versao", 0).max(0) as u64,
            imagem: hex_para_bytes(e.texto_ou("imagem", ""))?,
            carimbo_ms: e.inteiro_ou("carimbo_ms", 0),
            origem: e.inteiro_ou("origem", 0).clamp(0, u16::MAX as i64) as u16,
            posicao: match e.campo("posicao").and_then(Json::inteiro) {
                Some(n) if n >= 0 => n as u64,
                _ => POSICAO_DESCONHECIDA,
            },
            tx: tx_do_fio(e.campo("tx")),
        });
    }
    Ok(eventos)
}

// ------------------------------------------- a transacao inteira (676)

// O teto e o custo de um evento moram no `phxsql_store::log` desde o pedido
// 685: a ORIGEM recusa no COMMIT a transacao que passaria dele, e a constante
// tem de ser UMA so dos dois lados. Reexportados para os chamadores de sempre.
pub use phxsql_store::log::{CUSTO_DO_EVENTO, TETO_DA_TRANSACAO};

/// O maior id de transacao de um grupo do [`Juntador`] -- a ultima
/// transacao da origem que ele leva. Zero quando so ha evento sem id.
///
/// UMA conta para quem diz «a ultima transacao aplicada inteira» (pedido 299,
/// R6) nos tres caminhos que aplicam grupo (a replica fiel, o quorum e o
/// bidirecional).
pub fn maior_tx(grupo: &[(usize, Vec<EventoRecebido>)]) -> u64 {
    grupo
        .iter()
        .flat_map(|(_, v)| v.iter().map(|e| e.tx))
        .max()
        .unwrap_or(0)
}

/// Quantos eventos uma tomada da trava aplica de uma vez, somando as
/// transacoes inteiras que cabem: o mesmo [`LOTE`] de antes, para que o
/// alcance de um diario de autocommits nao pague uma tomada por evento.
const EVENTOS_POR_TOMADA: usize = LOTE as usize;

/// Uma tabela no [`Juntador`]: o que ja chegou e ainda nao se aplicou, e ate
/// onde puxar.
struct FilaDoJuntador {
    eventos: std::collections::VecDeque<EventoRecebido>,
    /// A posicao, no diario do source, do proximo evento a puxar.
    proximo: u64,
    /// Ate onde puxar: a contagem que o `posicao` deu. O `posicao` e lido
    /// com a trava de escrita do source na mao, entao esse vetor e uma
    /// FRONTEIRA DE COMMIT -- toda transacao antes dele esta inteira antes
    /// dele, em todas as tabelas. Puxar alem dele traria o comeco de uma
    /// transacao cujo resto, noutra tabela, nao entrou na conta.
    alvo: u64,
    /// O source devolveu menos do que a contagem prometia (encolheu entre o
    /// `posicao` e o `replicar`), ou a tabela rompeu: nada mais dela.
    esgotada: bool,
}

impl FilaDoJuntador {
    fn pode_puxar(&self) -> bool {
        !self.esgotada && self.proximo < self.alvo
    }

    /// A transacao `tx` pode continuar alem do que ja chegou desta tabela?
    fn tx_continua(&self, tx: u64) -> bool {
        self.pode_puxar() && self.eventos.back().is_some_and(|e| e.tx == tx)
    }
}

/// O proximo passo do alcance de um database. Ver [`Juntador`].
pub enum Passo {
    /// Puxe a tabela `fila` a partir de `desde` (fora da trava) e devolva o
    /// lote em [`Juntador::receber`].
    Puxar { fila: usize, desde: u64 },
    /// Aplique estes eventos, de varias tabelas, sob UMA tomada da trava.
    /// `inteiro` falso = pedaco de uma transacao acima do
    /// [`TETO_DA_TRANSACAO`].
    Aplicar {
        grupo: Vec<(usize, Vec<EventoRecebido>)>,
        inteiro: bool,
    },
    /// Nada mais a fazer nesta rodada.
    Fim,
}

/// Junta os eventos de VARIAS tabelas de um database pelo id de transacao, e
/// diz o que aplicar de cada vez -- pedido 676.
///
/// # O desenho
///
/// O diario e por tabela, e um commit entre tabelas tem eventos em diarios
/// diferentes. O id de transacao de cada evento (o mesmo para a tomada
/// inteira da trava na origem) e ESTRITAMENTE crescente no processo da
/// origem, entao cada diario esta em ordem de id, e a transacao `T` esta
/// inteira na mao quando, em toda tabela, ou o ultimo evento que chegou ja e
/// de outra transacao, ou a tabela chegou a fronteira do `posicao`. E a fusao
/// de listas ordenadas, com a menor cabeca na frente.
///
/// Sem rede e sem disco, de proposito: a decisao se prova aqui com listas na
/// mao, e o servidor so executa os passos.
///
/// # O que ele NAO resolve
///
/// Ordem entre transacoes que nao dividem tabela nenhuma: elas comutam na
/// replica (cada `.reg` so recebe o proprio diario), e a fusao as aplica na
/// ordem dos ids. E o evento sem id (zero) vai sozinho, como sempre foi.
pub struct Juntador {
    filas: Vec<FilaDoJuntador>,
    teto: usize,
    /// A transacao que esta indo em pedacos, para conta-la uma vez so.
    partida: Option<u64>,
    /// Quantas transacoes passaram do teto nesta rodada.
    pub em_pedacos: u64,
    /// Daqui em diante nada se aplica: a transacao com este id (ou maior)
    /// toca uma tabela que parou -- pedido 299, F2. Ver [`Juntador::barrar`].
    barreira: Option<u64>,
    /// A rodada chegou a barreira com transacao na mao: alguma ficou
    /// segurada.
    pub segurou: bool,
}

impl Juntador {
    /// `alvos` e, por tabela, `(posicao local, contagem do source)`.
    pub fn novo(alvos: &[(u64, u64)], teto: usize) -> Juntador {
        Juntador {
            filas: alvos
                .iter()
                .map(|&(proximo, alvo)| FilaDoJuntador {
                    eventos: Default::default(),
                    proximo,
                    alvo,
                    esgotada: false,
                })
                .collect(),
            teto,
            partida: None,
            em_pedacos: 0,
            barreira: None,
            segurou: false,
        }
    }

    /// A transacao `tx` toca uma tabela que PAROU (continuidade rompida, ou
    /// o ensaio que recusou): ela e toda transacao depois dela ficam fora
    /// desta rodada, em TODAS as tabelas -- pedido 299, F2.
    ///
    /// # Por que a parada e por transacao, e nao por tabela
    ///
    /// Largar so a tabela rompida deixava as irmas da mesma transacao
    /// entrarem: a venda chegava sem o pagamento, legivel e indistinguivel de
    /// uma venda sem pagamento. E as transacoes DEPOIS dela tambem esperam,
    /// porque podem depender da que parou -- os tres maduros param o fluxo, e
    /// nao a tabela (o `worker.c` do PostgreSQL, a thread SQL do MySQL e do
    /// MariaDB). As de ANTES que nao a tocam entram: os ids sao estritamente
    /// crescentes, entao nenhuma delas depende da parada.
    ///
    /// Varias barreiras na mesma rodada: vale a menor.
    pub fn barrar(&mut self, tx: u64) {
        self.barreira = Some(self.barreira.map_or(tx, |b| b.min(tx)));
    }

    /// As tabelas cuja mao ja tem algum evento da transacao `tx` -- o que a
    /// parada diz de onde a transacao segurada mexe.
    pub fn tabelas_de(&self, tx: u64) -> Vec<usize> {
        self.filas
            .iter()
            .enumerate()
            .filter(|(_, f)| f.eventos.iter().any(|e| e.tx == tx))
            .map(|(i, _)| i)
            .collect()
    }

    /// O lote que chegou para a tabela `fila`, a partir do `desde` que o
    /// [`Passo::Puxar`] pediu. Corta na fronteira do `posicao`; vazio quer
    /// dizer que o source encolheu, e a tabela sai da rodada.
    pub fn receber(&mut self, fila: usize, eventos: Vec<EventoRecebido>) {
        let f = &mut self.filas[fila];
        if eventos.is_empty() {
            f.esgotada = true;
            return;
        }
        let cabem = f.alvo.saturating_sub(f.proximo) as usize;
        for e in eventos.into_iter().take(cabem) {
            f.eventos.push_back(e);
            f.proximo += 1;
        }
    }

    /// O lote do bidirecional (pedido 681): o source SUPRIME os eventos que
    /// nasceram em quem pede, e a posicao anda por cima deles ate `ate`. A
    /// conta por quantidade do [`Juntador::receber`] deixaria a fila atras do
    /// source para sempre; aqui o que manda e a posicao de cada evento.
    ///
    /// Nada andou (`ate` nao passa do que se pediu e nada veio): a tabela sai
    /// da rodada, como o lote vazio do `receber`.
    pub fn receber_ate(&mut self, fila: usize, eventos: Vec<EventoRecebido>, ate: u64) {
        let f = &mut self.filas[fila];
        if eventos.is_empty() && ate <= f.proximo {
            f.esgotada = true;
            return;
        }
        let alvo = f.alvo;
        for e in eventos {
            // O evento sem posicao (source velho) conta pela ordem, como no
            // `receber`.
            let onde = if e.posicao == POSICAO_DESCONHECIDA {
                f.proximo
            } else {
                e.posicao
            };
            if onde >= alvo {
                break;
            }
            f.eventos.push_back(e);
            f.proximo = onde + 1;
        }
        f.proximo = f.proximo.max(ate.min(alvo));
    }

    /// Ate onde a fila `fila` esta CONSUMIDA: a posicao do primeiro evento na
    /// mao que ainda nao se aplicou, ou, com a mao vazia, a proxima a puxar.
    /// E a posicao que o bidirecional grava (pedido 681) -- nunca a frente
    /// de uma transacao que esperou na mao.
    pub fn consumido(&self, fila: usize) -> u64 {
        let f = &self.filas[fila];
        match f.eventos.front() {
            Some(e) if e.posicao != POSICAO_DESCONHECIDA => e.posicao,
            Some(_) => f.proximo - f.eventos.len() as u64,
            None => f.proximo,
        }
    }

    /// A tabela `fila` nao se puxa mais nesta rodada, e o que JA esta na mao
    /// dela fica -- pedido 299, F3. E a irma do [`Juntador::largar`] para
    /// quem parou num evento conhecido: os eventos de antes dele sao de
    /// transacoes MENORES que a barreira (cada diario esta em ordem de id), e
    /// descarta-los deixaria essas transacoes entrarem sem esta tabela -- a
    /// meia venda pela porta do proprio conserto. Quem chama poe a barreira
    /// no id do evento que parou ([`Juntador::barrar`]).
    pub fn encerrar(&mut self, fila: usize) {
        self.filas[fila].esgotada = true;
    }

    /// A tabela rompeu (a continuidade nao confere): o que esta na mao dela
    /// se descarta e ela nao e mais puxada nesta rodada.
    pub fn largar(&mut self, fila: usize) {
        let f = &mut self.filas[fila];
        f.eventos.clear();
        f.esgotada = true;
    }

    /// Bytes de `tx` ja na mao, somando todas as tabelas.
    fn bytes_de(&self, tx: u64) -> usize {
        self.filas
            .iter()
            .flat_map(|f| f.eventos.iter().take_while(|e| e.tx == tx))
            .map(|e| phxsql_store::log::custo_na_transacao(e.imagem.len()))
            .sum()
    }

    /// Tira das cabecas os eventos de `tx` (de todas as tabelas) e os poe
    /// no grupo, tabela por tabela, na ordem de cada diario.
    fn tirar(&mut self, tx: u64, grupo: &mut Vec<(usize, Vec<EventoRecebido>)>) -> usize {
        let mut n = 0;
        for (i, f) in self.filas.iter_mut().enumerate() {
            while f.eventos.front().is_some_and(|e| e.tx == tx) {
                let e = f.eventos.pop_front().expect("ha cabeca");
                match grupo.iter_mut().find(|(j, _)| *j == i) {
                    Some((_, v)) => v.push(e),
                    None => grupo.push((i, vec![e])),
                }
                n += 1;
                // O evento sem id e uma transacao de UM evento: zero nao
                // junta nada com nada.
                if tx == 0 {
                    break;
                }
            }
        }
        n
    }

    /// O proximo passo.
    pub fn passo(&mut self) -> Passo {
        let mut grupo: Vec<(usize, Vec<EventoRecebido>)> = Vec::new();
        let mut eventos = 0usize;
        let mut bytes = 0usize;
        loop {
            // Uma tabela vazia que ainda tem o que puxar pode trazer uma
            // transacao MENOR que todas as cabecas: nada se decide sem ela.
            if let Some(i) = self
                .filas
                .iter()
                .position(|f| f.eventos.is_empty() && f.pode_puxar())
            {
                if grupo.is_empty() {
                    return Passo::Puxar {
                        fila: i,
                        desde: self.filas[i].proximo,
                    };
                }
                break;
            }
            // A menor cabeca. Zero (sem id) e a menor de todas, e o evento
            // sem id so existe ANTES dos com id no mesmo diario -- o volume
            // velho vem primeiro.
            let Some(tx) = self
                .filas
                .iter()
                .filter_map(|f| f.eventos.front().map(|e| e.tx))
                .min()
            else {
                break;
            };
            // A barreira vem ANTES de tudo, inclusive do «puxe o resto»: a
            // transacao segurada nao precisa estar inteira na mao para nao
            // entrar.
            if self.barreira.is_some_and(|b| tx >= b) {
                self.segurou = true;
                break;
            }
            let incompleta = if tx == 0 {
                None
            } else {
                self.filas.iter().position(|f| f.tx_continua(tx))
            };
            let custo = self.bytes_de(tx);
            if let Some(i) = incompleta {
                if !grupo.is_empty() {
                    break;
                }
                if custo <= self.teto {
                    return Passo::Puxar {
                        fila: i,
                        desde: self.filas[i].proximo,
                    };
                }
                // Acima do teto: vai o que esta na mao, e a transacao fica
                // contada como partida.
                if self.partida != Some(tx) {
                    self.partida = Some(tx);
                    self.em_pedacos += 1;
                }
                self.tirar(tx, &mut grupo);
                return Passo::Aplicar {
                    grupo,
                    inteiro: false,
                };
            }
            // Inteira. Entra no grupo se couber -- a primeira entra sempre.
            if !grupo.is_empty()
                && (eventos >= EVENTOS_POR_TOMADA || bytes.saturating_add(custo) > self.teto)
            {
                break;
            }
            let pedaco_final = self.partida == Some(tx);
            eventos += self.tirar(tx, &mut grupo);
            bytes = bytes.saturating_add(custo);
            if pedaco_final {
                // O ultimo pedaco de uma transacao partida vai SOZINHO e
                // marcado: juntar transacoes inteiras a ele faria a conta
                // de quem esta inteiro mentir.
                self.partida = None;
                return Passo::Aplicar {
                    grupo,
                    inteiro: false,
                };
            }
        }
        if grupo.is_empty() {
            Passo::Fim
        } else {
            Passo::Aplicar {
                grupo,
                inteiro: true,
            }
        }
    }
}

/// Quanto tempo [`ligar`] espera o `connect` antes de desistir.
///
/// Dez segundos: acima de qualquer ida e volta sadia, inclusive fora da rede
/// local, e igual ao `reconectar_em` de fabrica -- esperar mais para conectar
/// do que se espera para tentar de novo nao faz sentido. O pulso do cluster
/// usa 1-2 s porque e rede local e porque um no morto nao pode segurar a
/// conferencia dos vivos; aqui o custo de esperar e so o atraso da replica.
pub const PRAZO_DE_CONEXAO: Duration = Duration::from_secs(10);

/// O silencio da conversa da replica com a origem: quanto um pedido pode
/// ficar sem um byte sequer. Abaixo dos 60 s do `replica_net_timeout`
/// (MySQL/MariaDB) e do `wal_receiver_timeout` (PG) porque o nosso pedido e
/// curto e a origem responde de uma vez; era o `30` cravado em `ligar`.
pub const SILENCIO_DA_REPLICA: Duration = Duration::from_secs(30);

// O multiplo do total e o `prazo_da_conversa` desceram para o motor
// (`phxsql_core::prazo`) no pedido 585, porque o driver ODBC conversa com o
// mesmo servidor pelo mesmo fio e nao depende deste crate. Reexportados aqui
// para os chamadores de sempre (laco, sonda, console) nao mudarem de porta.
pub use crate::prazo::{prazo_da_conversa, MULTIPLO_DO_TOTAL_DA_CONVERSA};

static ROTULO_DO_PULSO: prazo::Rotulo = prazo::Rotulo {
    quem: "o pulso do cluster",
    regra: "o pedido inteiro do pulso tem de caber num silencio \
            (2 x pulso_s, no minimo 5 s) -- pedido 580",
};

/// O prazo do pulso do cluster e da propagacao das ordens dele -- pedido 580.
///
/// O total e IGUAL ao silencio: a resposta de um pulso e pequena e sai de
/// uma vez, entao um pedido que nao coube num silencio ja nao e pulso, e o
/// no que goteja tem de sair da conta no mesmo prazo do no que calou. E a
/// ideia do `MASTER_HEARTBEAT_PERIOD` (MySQL/MariaDB), que por padrao e a
/// metade do `replica_net_timeout`: o sinal de vida cabe folgado dentro do
/// prazo. Um lugar so para o pulso e para a propagacao, que chamam as mesmas
/// funcoes na mesma ordem.
pub fn prazo_do_pulso(pulso_s: u64) -> Prazo {
    let espera = Duration::from_secs(pulso_s.saturating_mul(2).max(5));
    Prazo::com_total(espera, espera, &ROTULO_DO_PULSO)
}

/// Abre a conexao e entra autenticado. Usado pelo laco e pelos testes.
pub fn ligar(origem: &Origem) -> Result<Cliente> {
    ligar_com_prazo(origem, SILENCIO_DA_REPLICA, PRAZO_DE_CONEXAO)
}

/// [`ligar`] com o silencio e o prazo de conexao ditos por quem chama.
///
/// Existe para as provas contra o sistema operacional: um host que engole o
/// SYN nao pode prender quem liga, e um que goteja nao pode prender quem
/// conversa -- e os testes medem isso com prazos curtos em vez de esperar os
/// de fabrica. O total sai do silencio pelo MESMO [`prazo_da_conversa`].
pub fn ligar_com_prazo(
    origem: &Origem,
    silencio: Duration,
    prazo_conexao: Duration,
) -> Result<Cliente> {
    let mut c = Cliente::conectar_com_prazo(
        &origem.host,
        origem.porta,
        &origem.token,
        prazo_da_conversa(silencio),
        prazo_conexao,
    )?;
    // O tunel ANTES do login, de proposito: e a prova do desafio-resposta e o
    // token que ele existe para esconder, e depois do login ja seria tarde.
    c.proteger(origem.cifra, origem.pino_do_fio()?, origem.pino_tls()?)?;
    if !origem.usuario.is_empty() {
        c.autenticar(&origem.usuario, &origem.senha_hash, &origem.senha)?;
    }
    Ok(c)
}

// ---------------------------------------------------------------------------
// O que o laco faz quando a rodada falha -- e por que sao TRES respostas.
//
// Pedido 203, filmado em 07/09/2026: uma replica com a credencial errada
// tentava entrar a cada `reconectar_em`, o master contava cada recusa como
// tentativa leve e bloqueava o IP em 4 s -- levando junto o operador que
// saia do mesmo 127.0.0.1. Medido pelo soquete
// (`bancada/replicacao/credencial-recusada.py`): 75 tentativas por minuto,
// bloqueio na quinta, por 60 minutos.
//
// O bloqueio do master esta certo e nao mudou. O que estava errado era tratar
// «a origem me recusou» como se fosse «a origem caiu»: uma e deterministica
// -- a mesma prova contra o mesmo hash da a mesma resposta amanha -- e a
// outra e transitoria. Insistir na primeira so gasta a tolerancia do bloqueio
// do outro lado; insistir na segunda e o proprio trabalho da replica.
// ---------------------------------------------------------------------------

/// Por que uma rodada falhou. Decide o que o laco faz em seguida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falha {
    /// A origem RECUSOU o token, a credencial ou a propria conexao (IP
    /// barrado). Nao e transitorio: tentar de novo com a mesma configuracao
    /// da o mesmo resultado, e cada tentativa conta contra o IP no master.
    CredencialRecusada,
    /// Nao deu para chegar na origem, ou a conexao caiu no meio. Passa
    /// sozinho: a origem volta, a rede volta.
    Rede,
    /// A conversa passou de um LIMITE -- pedido 585. O caso que motivou e o
    /// par que goteja: o prazo total (`prazo.rs`) corta o pedido e devolve
    /// `LimiteExcedido`; antes isso caia em `Outra` e o laco voltava ao
    /// mesmo par no intervalo fixo, prendendo a thread um total inteiro de
    /// cada vez. Entram junto a linha acima do teto do `Canal` e o
    /// `LIMITE_EXCEDIDO` que a propria origem responde (lotada, por exemplo):
    /// os tres dizem «esta conversa, agora, nao cabe», e insistir no mesmo
    /// ritmo so repete a conta. Recua como a [`Falha::Rede`], no mesmo
    /// contador: os maduros tratam o estouro do prazo da conversa como
    /// conexao caida (ver [`Ritmo`]).
    Limite,
    /// Qualquer outra coisa -- esquema, tabela recusada, dado corrompido.
    /// Continua no intervalo fixo de sempre, porque ninguem mediu que o
    /// recuo ajudaria aqui e guarda nova entra pedida.
    Outra,
}

impl Falha {
    /// A classificacao de um erro que veio de [`ligar`] -- a fase em que a
    /// origem diz sim ou nao para QUEM chega.
    ///
    /// So aqui `Autorizacao` vira credencial recusada. Uma `Autorizacao`
    /// depois de entrar (um `replicar` sem direito, por exemplo) e outra
    /// coisa: nao conta como tentativa leve no master, e um administrador de
    /// la pode corrigir sem que ninguem religue aqui.
    pub fn ao_ligar(e: &PhxError) -> Falha {
        match e {
            PhxError::Autorizacao(_) => Falha::CredencialRecusada,
            PhxError::Io(_) => Falha::Rede,
            // O par que goteja no desafio ou no aperto estoura o total AQUI,
            // antes de entrar -- e o irmao do mesmo estouro na rodada.
            PhxError::LimiteExcedido(_) => Falha::Limite,
            _ => Falha::Outra,
        }
    }

    /// A classificacao de um erro DEPOIS de entrar: a rede e o limite se
    /// distinguem; a credencial nao (ver [`Falha::ao_ligar`]).
    pub fn na_rodada(e: &PhxError) -> Falha {
        match e {
            PhxError::Io(_) => Falha::Rede,
            PhxError::LimiteExcedido(_) => Falha::Limite,
            _ => Falha::Outra,
        }
    }
}

/// [`ligar`] com a falha ja classificada, para quem chama decidir sem
/// adivinhar pelo texto do erro -- texto se compara por chave, nunca por
/// frase.
pub fn ligar_classificado(origem: &Origem) -> std::result::Result<Cliente, (Falha, PhxError)> {
    ligar(origem).map_err(|e| (Falha::ao_ligar(&e), e))
}

/// O que o laco faz depois de uma rodada que falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisao {
    /// Espera tanto e tenta de novo.
    Dormir(Duration),
    /// Para de tentar ate alguem religar (`replicacao_ligar`) ou a
    /// configuracao mudar (reinicio).
    Estacionar,
}

/// O ritmo do laco: intervalo fixo entre rodadas em vao, recuo exponencial
/// para a REDE e o LIMITE, estacionamento para a credencial.
///
/// **O limite recua como a rede -- pedido 585, decisao do J pela regua.** Os
/// tres maduros convergem em tratar o estouro do prazo da conversa como
/// conexao caida, pela MESMA via de nova tentativa: o MySQL e o MariaDB, ao
/// estourar o `replica_net_timeout`/`slave_net_timeout` (60 s), dao a conexao
/// por quebrada e religam a cada `MASTER_CONNECT_RETRY` (60 s); o PostgreSQL,
/// ao estourar o `wal_receiver_timeout` (60 s), encerra o walreceiver e tenta
/// de novo depois do `wal_retrieve_retry_interval` (5 s). Convergencia 9
/// (PG 4 + MariaDB 3 + MySQL 2) contra 0; o SQLite nao replica. Entra sem
/// pergunta. O que eles NAO tem e o recuo exponencial -- os tres esperam o
/// intervalo fixo; o nosso recuo e o do pedido 203, e o teto dele (60 s) e o
/// `MASTER_CONNECT_RETRY` de fabrica. A primeira falha espera so a base: uma
/// replica que gotejou UMA vez volta no intervalo de sempre.
///
/// O recuo da rede dobra a partir do `reconectar_em` e para em
/// [`Ritmo::TETO`] -- um minuto. O teto e baixo de proposito: uma conexao por
/// minuto a um host morto nao custa nada, e uma origem que volta depois de
/// uma noite fora e encontrada em ate um minuto, que e o que importa para o
/// atraso da replica. Um `reconectar_em` acima do teto vale como esta: o teto
/// nunca encurta o que o administrador pediu.
#[derive(Debug, Clone)]
pub struct Ritmo {
    base: Duration,
    /// Falhas de rede SEGUIDAS -- o expoente do recuo. Zera no sucesso.
    pub seguidas: u32,
    /// Desde quando a origem nao se alcanca: a primeira falha de rede (ou de
    /// limite) DESTE episodio. `None` enquanto ela responde.
    ///
    /// Um relogio proprio, e nao o `ultima_rodada_ms` do estado: aquele sobe
    /// no erro tambem (lacuna L6 do `aquario-707.md`), e «inalcancavel ha
    /// quanto tempo» nao sai dele.
    inalcancavel_desde: Option<Instant>,
    /// O alarme deste episodio ja saiu: uma pedra por queda, e nao uma por
    /// tentativa -- a tentativa e a cada minuto, e a queda dura a noite.
    avisado: bool,
}

impl Ritmo {
    pub const TETO: Duration = Duration::from_secs(60);

    /// Quanto a origem pode ficar inalcancavel antes do alarme
    /// `OrigemInalcancavel` (pedido 769): tres vezes o [`Ritmo::TETO`] --
    /// tres tentativas no recuo maximo, todas em vao. *Raciocinado*, do
    /// `aquario-707.md` §2.4 e §9: a falha que goteja uma vez volta na
    /// tentativa seguinte e nao chega aqui.
    pub const PRAZO_DE_INALCANCAVEL: Duration = Duration::from_secs(3 * 60);

    pub fn novo(base: Duration) -> Ritmo {
        Ritmo {
            base,
            seguidas: 0,
            inalcancavel_desde: None,
            avisado: false,
        }
    }

    /// O intervalo entre rodadas que nao acharam nada -- o `reconectar_em`.
    pub fn base(&self) -> Duration {
        self.base
    }

    /// A rodada deu certo (achou algo ou nao): o recuo volta ao comeco.
    pub fn sucesso(&mut self) {
        self.seguidas = 0;
        self.alcancou();
    }

    /// A origem respondeu -- com sucesso, ou recusando (a credencial, o
    /// esquema): ela esta la. O episodio de inalcancavel acaba.
    fn alcancou(&mut self) {
        self.inalcancavel_desde = None;
        self.avisado = false;
    }

    /// A origem esta inalcancavel ha mais que o
    /// [`Ritmo::PRAZO_DE_INALCANCAVEL`], e o alarme deste episodio ainda nao
    /// saiu? Devolve `true` UMA vez por episodio.
    pub fn cruzou_o_prazo(&mut self) -> bool {
        self.cruzou_o_prazo_em(Instant::now())
    }

    fn cruzou_o_prazo_em(&mut self, agora: Instant) -> bool {
        let Some(desde) = self.inalcancavel_desde else {
            return false;
        };
        if self.avisado || agora.saturating_duration_since(desde) < Self::PRAZO_DE_INALCANCAVEL {
            return false;
        }
        self.avisado = true;
        true
    }

    /// PROVA: o episodio comecou `ha` atras, sem esperar o relogio.
    #[cfg(test)]
    pub(crate) fn inalcancavel_ha(&mut self, ha: Duration) {
        self.inalcancavel_desde = Instant::now().checked_sub(ha);
    }

    /// A rodada falhou assim: o que fazer.
    pub fn apos(&mut self, falha: Falha) -> Decisao {
        match falha {
            Falha::CredencialRecusada => {
                self.seguidas = 0;
                self.alcancou();
                Decisao::Estacionar
            }
            // O limite recua no MESMO contador da rede, e nao num proprio:
            // um par que ora cai e ora goteja e o mesmo par doente, e dois
            // contadores o deixariam voltar na base a cada troca de sintoma.
            Falha::Rede | Falha::Limite => {
                let espera = self.espera_de_rede();
                self.seguidas = self.seguidas.saturating_add(1);
                self.inalcancavel_desde.get_or_insert_with(Instant::now);
                Decisao::Dormir(espera)
            }
            Falha::Outra => {
                self.alcancou();
                Decisao::Dormir(self.base)
            }
        }
    }

    /// `base * 2^seguidas`, sem passar do teto e sem nunca ficar abaixo da
    /// base. O deslocamento e limitado antes de multiplicar, senao um laco
    /// que passou a noite recuando estouraria o `u32` ao chegar em 2^32.
    fn espera_de_rede(&self) -> Duration {
        let fator = 1u32 << self.seguidas.min(20);
        let crescida = self.base.saturating_mul(fator);
        crescida.min(Self::TETO).max(self.base)
    }
}

#[cfg(test)]
mod testes_do_ritmo {
    use super::*;

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// A tres respostas do `ao_ligar`: e ESTA classificacao que separa
    /// «a origem me recusou» de «a origem caiu».
    #[test]
    fn ao_ligar_separa_credencial_de_rede_e_do_resto() {
        let recusa = PhxError::Autorizacao("login: credencial invalida".into());
        let queda = PhxError::Io(std::io::Error::other("connection refused"));
        let esquema = PhxError::Esquema("imagem_da_linha desligada".into());
        assert_eq!(Falha::ao_ligar(&recusa), Falha::CredencialRecusada);
        assert_eq!(Falha::ao_ligar(&queda), Falha::Rede);
        assert_eq!(Falha::ao_ligar(&esquema), Falha::Outra);
        // Depois de entrar, `Autorizacao` NAO e credencial recusada: um
        // `replicar` sem direito se corrige no master, sem religar aqui.
        assert_eq!(Falha::na_rodada(&recusa), Falha::Outra);
        assert_eq!(Falha::na_rodada(&queda), Falha::Rede);
        // O limite (pedido 585) se distingue nas duas fases.
        let limite = PhxError::LimiteExcedido("passou do prazo total".into());
        assert_eq!(Falha::ao_ligar(&limite), Falha::Limite);
        assert_eq!(Falha::na_rodada(&limite), Falha::Limite);
    }

    /// O limite recua no MESMO contador da rede: alternar os sintomas nao
    /// devolve o par doente a base. E a primeira falha, de qualquer um, e a
    /// base -- a volta depois de uma falha unica nao atrasa.
    #[test]
    fn limite_e_rede_recuam_no_mesmo_contador() {
        let mut r = Ritmo::novo(s(1));
        assert_eq!(r.apos(Falha::Limite), Decisao::Dormir(s(1)));
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(2)));
        assert_eq!(r.apos(Falha::Limite), Decisao::Dormir(s(4)));
        r.sucesso();
        assert_eq!(r.apos(Falha::Limite), Decisao::Dormir(s(1)));
    }

    /// O defeito do pedido 203 reposto seria este teste caindo: a credencial
    /// recusada tem de ESTACIONAR na primeira, e nao dormir e voltar.
    #[test]
    fn credencial_recusada_estaciona_na_primeira() {
        let mut r = Ritmo::novo(s(1));
        assert_eq!(r.apos(Falha::CredencialRecusada), Decisao::Estacionar);
        // E de novo: religou, recusou de novo, estaciona de novo -- nunca
        // uma segunda tentativa por conta propria.
        assert_eq!(r.apos(Falha::CredencialRecusada), Decisao::Estacionar);
        assert_eq!(r.seguidas, 0);
    }

    /// A rede recua dobrando: 1, 2, 4, 8, 16, 32, e para no teto de 60 s.
    #[test]
    fn rede_recua_dobrando_ate_o_teto() {
        let mut r = Ritmo::novo(s(1));
        let esperas: Vec<Duration> = (0..8)
            .map(|_| match r.apos(Falha::Rede) {
                Decisao::Dormir(d) => d,
                Decisao::Estacionar => panic!("rede nunca estaciona"),
            })
            .collect();
        assert_eq!(
            esperas,
            vec![s(1), s(2), s(4), s(8), s(16), s(32), s(60), s(60)]
        );
        assert_eq!(r.seguidas, 8);
        // O sucesso zera: a proxima queda volta a esperar a base.
        r.sucesso();
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(1)));
    }

    /// Um `reconectar_em` acima do teto vale como esta: o teto nunca encurta
    /// o que o administrador pediu.
    #[test]
    fn o_teto_nunca_encurta_a_base() {
        let mut r = Ritmo::novo(s(300));
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(300)));
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(300)));
    }

    /// Uma noite inteira de recuo nao estoura o expoente.
    #[test]
    fn muitas_falhas_seguidas_nao_estouram() {
        let mut r = Ritmo::novo(s(10));
        for _ in 0..100_000 {
            let _ = r.apos(Falha::Rede);
        }
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(60)));
    }

    /// As outras falhas ficam no intervalo fixo de sempre -- guarda nova
    /// entra pedida, e ninguem mediu que o recuo ajudaria aqui.
    #[test]
    fn outra_falha_mantem_o_intervalo_fixo() {
        let mut r = Ritmo::novo(s(10));
        assert_eq!(r.apos(Falha::Outra), Decisao::Dormir(s(10)));
        assert_eq!(r.apos(Falha::Outra), Decisao::Dormir(s(10)));
        assert_eq!(r.seguidas, 0);
    }
}

/// O prazo de conexao de [`ligar`], provado contra o sistema operacional.
///
/// Revisao SEC de 17/09/2026, A5: `ligar` caia num `connect` sem prazo, e o
/// irmao com prazo (`conectar_com_prazo`) so servia o pulso do cluster.
#[cfg(test)]
mod testes_do_prazo_de_conexao {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;

    /// Um buraco negro LOCAL: um ouvinte que nunca aceita, com a fila de
    /// `accept` cheia.
    ///
    /// Com a fila cheia o nucleo DESCARTA o SYN em vez de recusar, e quem
    /// chega fica pendurado retransmitindo -- e o mesmo que um host que
    /// engole pacotes, sem depender de rota nenhuma. Medido antes de escrever
    /// (17/09/2026): neste ambiente `192.0.2.1` responde «Connection refused»
    /// na hora, entao NAO serve de buraco negro; a fila cheia no loopback
    /// pendurou um `connect` de 1 s ate o prazo. As conexoes que encheram a
    /// fila voltam junto, porque fechar qualquer uma abriria uma vaga.
    ///
    /// `None` quando o sistema nao encheu a fila em 4.096 conexoes: ai nao ha
    /// como provar, e o teste diz isso em vez de passar por engano.
    fn buraco_negro() -> Option<(TcpListener, Vec<TcpStream>, u16)> {
        let ouvinte = TcpListener::bind("127.0.0.1:0").ok()?;
        let endereco = ouvinte.local_addr().ok()?;
        let porta = endereco.port();
        let mut presos = Vec::new();
        for _ in 0..4_096 {
            match TcpStream::connect_timeout(&endereco, Duration::from_millis(200)) {
                Ok(c) => presos.push(c),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    return Some((ouvinte, presos, porta));
                }
                Err(_) => return None,
            }
        }
        None
    }

    fn origem_para(porta: u16) -> Origem {
        Origem {
            nome: "buraco-negro".into(),
            host: "127.0.0.1".into(),
            porta,
            token: String::new(),
            databases: Vec::new(),
            reconectar_em: 1,
            usuario: String::new(),
            senha_hash: String::new(),
            senha: String::new(),
            cada_minutos: 0,
            hora: String::new(),
            cifra: false,
            chave_do_fio: String::new(),
            pino_tls: String::new(),
            espelho: false,
        }
    }

    /// **Prova real do A5.** Com o `connect` sem prazo reposto em `ligar`,
    /// este teste nao falha: PENDURA -- e por isso ele mede numa thread e
    /// reprova pelo `recv_timeout`, em vez de esperar o SO desistir do SYN.
    ///
    /// E a falha tem de sair classificada como REDE: e assim que o laco
    /// recua dobrando em vez de estacionar.
    #[test]
    fn ligar_nao_fica_pendurado_num_host_que_engole_o_syn() {
        let Some((_ouvinte, presos, porta)) = buraco_negro() else {
            eprintln!(
                "este sistema nao encheu a fila de accept em 4.096 conexoes: \
                 sem buraco negro nao ha como provar o prazo aqui"
            );
            return;
        };
        let prazo = Duration::from_millis(500);
        let (envio, volta) = mpsc::channel();
        std::thread::spawn(move || {
            let inicio = Instant::now();
            let r = ligar_com_prazo(&origem_para(porta), SILENCIO_DA_REPLICA, prazo);
            let _ = envio.send((
                r.map(|_| ()).map_err(|e| Falha::ao_ligar(&e)),
                inicio.elapsed(),
            ));
        });
        match volta.recv_timeout(Duration::from_secs(5)) {
            Ok((resultado, levou)) => {
                assert_eq!(
                    resultado,
                    Err(Falha::Rede),
                    "um host que engole o SYN e falha de REDE, e nada mais"
                );
                assert!(
                    levou < Duration::from_secs(3),
                    "`ligar` levou {levou:?} para desistir com prazo de {prazo:?}"
                );
            }
            Err(_) => panic!(
                "`ligar` ficou pendurado alem de 5 s num host que engole o SYN \
                 (prazo pedido: {prazo:?}): o prazo de conexao nao esta valendo"
            ),
        }
        // As conexoes presas so sao soltas AQUI, depois da medida.
        assert!(
            !presos.is_empty(),
            "a fila de accept encheu sem nenhuma conexao?"
        );
    }

    /// O padrao que o laco usa tem de ser o de dez segundos -- e a origem que
    /// existe (um ouvinte que aceita) continua entrando pelo mesmo caminho.
    #[test]
    fn o_prazo_padrao_e_de_dez_segundos_e_a_origem_viva_conecta() {
        assert_eq!(PRAZO_DE_CONEXAO, Duration::from_secs(10));
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        // Sem token e sem usuario, `ligar` so conecta -- e o que se quer medir.
        let c = ligar(&origem_para(porta));
        assert!(c.is_ok(), "{:?}", c.err());
        let (aceita, _) = ouvinte.accept().unwrap();
        drop(aceita);
    }
}

// ---------------------------------------------------------------------------
// Pedido 312: o aperto de mao nao pode ler sem teto
// ---------------------------------------------------------------------------

#[cfg(test)]
mod testes_do_teto_do_aperto {
    use super::*;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// Quanto o source falso TENTA empurrar numa linha so, sem nunca mandar o
    /// `\n`. Passa do [`phxsql_core::fio::TETO_DO_REGISTRO`] de proposito: e o
    /// que mostra que o teto do registro nao alcancava esta leitura.
    const DESPEJO: u64 = 192 * 1024 * 1024;

    /// Um "source" que aceita a conexao e nunca termina de falar.
    ///
    /// Devolve a porta e o contador do que ele conseguiu EMPURRAR -- que nao
    /// e o que o outro lado guardou: quando a replica para de ler, ainda cabem
    /// alguns MiB nos buffers do nucleo antes de o `write_all` travar. Por
    /// isso a medida que decide e o ERRO, e o contador so separa "parou" de
    /// "engoliu tudo".
    fn source_que_nunca_cala() -> (u16, Arc<AtomicU64>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let empurrados = Arc::new(AtomicU64::new(0));
        let conta = Arc::clone(&empurrados);
        std::thread::spawn(move || {
            let Ok((mut fluxo, _)) = ouvinte.accept() else {
                return;
            };
            let _ = fluxo.set_write_timeout(Some(Duration::from_secs(3)));
            let lixo = vec![b'x'; 64 * 1024];
            while conta.load(Ordering::SeqCst) < DESPEJO {
                if fluxo.write_all(&lixo).is_err() {
                    break;
                }
                conta.fetch_add(lixo.len() as u64, Ordering::SeqCst);
            }
        });
        (porta, empurrados)
    }

    /// **Prova real do 312, nos dois sentidos.** Com o `read_line` cru reposto
    /// no lugar do `ler_ate`, este teste passa a NAO ver erro nenhum e o
    /// contador chega aos 192 MiB -- medido em 294 ms, antes do conserto.
    ///
    /// Quem decide quanta memoria esta replica reserva no aperto de mao tem de
    /// ser este lado, e nao o outro -- que ali ainda nao provou ser ninguem.
    #[test]
    fn o_aperto_de_mao_recusa_a_linha_sem_fim() {
        let (porta, empurrados) = source_que_nunca_cala();
        let mut c = Cliente::conectar("127.0.0.1", porta, "t", Duration::from_secs(2)).unwrap();
        let erro = c
            .cifrar(None)
            .expect_err("o aperto tinha de RECUSAR a linha sem fim");
        assert!(
            matches!(erro, PhxError::LimiteExcedido(_)),
            "a recusa saiu como {erro:?}, e nao como limite excedido"
        );
        let engolido = empurrados.load(Ordering::SeqCst);
        assert!(
            engolido < DESPEJO,
            "o source empurrou os {DESPEJO} bytes inteiros: nao ha teto nenhum"
        );
    }

    /// O COMPORTAMENTO VELHO, que e o que mais importa numa guarda nova: um
    /// source que responde o aperto como sempre respondeu continua passando
    /// pelo mesmo caminho -- o teto so morde quem passa dele.
    ///
    /// Aqui o source responde uma linha legitima de RECUSA (nao ha estatica
    /// para fechar o aperto num teste de unidade), e o que se prova e que a
    /// linha foi LIDA e ANALISADA: o erro que volta e o do source, com o
    /// motivo dele dentro, e nao um limite.
    #[test]
    fn resposta_curta_do_source_continua_passando() {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut fluxo, _)) = ouvinte.accept() else {
                return;
            };
            let _ = fluxo
                .write_all(br#"{"ok":false,"op":"cifrar","erro":"a cifra do fio esta desligada"}"#);
            let _ = fluxo.write_all(b"\n");
            let _ = fluxo.flush();
            std::thread::sleep(Duration::from_millis(200));
        });
        let mut c = Cliente::conectar("127.0.0.1", porta, "t", Duration::from_secs(2)).unwrap();
        let erro = c
            .cifrar(None)
            .expect_err("o source recusou: tinha de vir erro");
        assert!(
            matches!(erro, PhxError::Autorizacao(_)),
            "a resposta curta nao foi lida como sempre foi: {erro:?}"
        );
        assert!(
            erro.to_string().contains("desligada"),
            "o motivo do source se perdeu: {erro}"
        );
    }
}

// ---------------------------------------------------------------------------
// Pedido 580: o laco da replica e o pulso do cluster com prazo TOTAL
// ---------------------------------------------------------------------------

#[cfg(test)]
mod testes_do_prazo_total_da_conversa {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;

    /// O silencio curto dos testes. O total da replica sai dele pelo MESMO
    /// `prazo_da_conversa` da producao: 200 ms x 20 = 4 s.
    const SILENCIO: Duration = Duration::from_millis(200);
    /// Um byte a cada passo, dez vezes abaixo do silencio: so uma parada de
    /// 180 ms da thread que goteja estouraria o silencio antes do total.
    const PASSO: Duration = Duration::from_millis(20);
    /// Quanto se espera antes de declarar a thread presa. O maior total aqui
    /// e o do pulso, 5 s; sem total a thread ficaria presa para sempre.
    const PACIENCIA: Duration = Duration::from_secs(10);

    fn origem_para(porta: u16) -> Origem {
        Origem {
            nome: "prova580".into(),
            host: "127.0.0.1".into(),
            porta,
            token: String::new(),
            databases: Vec::new(),
            reconectar_em: 1,
            usuario: String::new(),
            senha_hash: String::new(),
            senha: String::new(),
            cada_minutos: 0,
            hora: String::new(),
            cifra: false,
            chave_do_fio: String::new(),
            pino_tls: String::new(),
            espelho: false,
        }
    }

    /// Aceita, e goteja um byte a cada `PASSO` sem nunca fechar a linha. Para
    /// quando o cliente fecha -- e o que o conserto faz ao cortar.
    fn par_que_goteja() -> u16 {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            loop {
                std::thread::sleep(PASSO);
                if s.write_all(b"x").is_err() {
                    return;
                }
            }
        });
        porta
    }

    type Volta = (&'static str, Result<()>, Duration, Duration);

    /// **580: o laco da replica e o pulso param no prazo total.** Cada um abre
    /// pela porta que a producao usa (`ligar_com_prazo`, e o `prazo_do_pulso`
    /// que o pulso e a propagacao chamam) contra um par que goteja, e tem de
    /// voltar com `LimiteExcedido` perto do total -- nem antes (o silencio nao
    /// estourou), nem nunca.
    ///
    /// # Prova real
    ///
    /// Com o defeito reposto (so o silencio, o de antes), nenhum dos dois
    /// volta em `PACIENCIA`: o vermelho lista quais ficaram presos.
    #[test]
    fn o_par_que_goteja_para_no_prazo_total() {
        let (avisar, avisos) = mpsc::channel::<Volta>();
        {
            let avisar = avisar.clone();
            let porta = par_que_goteja();
            std::thread::spawn(move || {
                let inicio = Instant::now();
                let r = ligar_com_prazo(&origem_para(porta), SILENCIO, Duration::from_secs(2))
                    .and_then(|mut c| puxar(&mut c, "db", "t", 0).map(|_| ()));
                let _ = avisar.send(("replica", r, inicio.elapsed(), Duration::from_secs(4)));
            });
        }
        {
            let porta = par_que_goteja();
            std::thread::spawn(move || {
                let inicio = Instant::now();
                let r = Cliente::conectar_com_prazo(
                    "127.0.0.1",
                    porta,
                    "",
                    prazo_do_pulso(1),
                    Duration::from_secs(2),
                )
                .and_then(|mut c| c.pedir(vec![("op", Json::texto_de("ping"))]).map(|_| ()));
                let _ = avisar.send(("pulso", r, inicio.elapsed(), Duration::from_secs(5)));
            });
        }
        let prazo_final = Instant::now() + PACIENCIA;
        let mut voltaram = Vec::new();
        while voltaram.len() < 2 {
            let resta = prazo_final.saturating_duration_since(Instant::now());
            match avisos.recv_timeout(resta) {
                Ok(v) => voltaram.push(v),
                Err(_) => break,
            }
        }
        let presos: Vec<&str> = ["replica", "pulso"]
            .into_iter()
            .filter(|q| !voltaram.iter().any(|v| v.0 == *q))
            .collect();
        assert!(
            presos.is_empty(),
            "ficaram presos alem de {PACIENCIA:?} ao par que goteja: {presos:?}"
        );
        for (quem, r, durou, total) in voltaram {
            match r {
                Err(PhxError::LimiteExcedido(m)) => {
                    assert!(m.contains("prazo total"), "{quem}: {m}");
                }
                outro => panic!("{quem}: esperava o prazo total, veio {outro:?}"),
            }
            // Perto do total, e nao do silencio: foi o total que cortou.
            assert!(
                durou + Duration::from_millis(300) >= total,
                "{quem} voltou em {durou:?}, antes do total de {total:?}"
            );
        }
    }

    /// O ritmo depois de `falhas` rodadas perdidas para o mesmo erro, a
    /// partir do `reconectar_em` de 1 s das origens destes testes.
    fn esperas_apos(falha: Falha, falhas: usize) -> Vec<Duration> {
        let mut r = Ritmo::novo(Duration::from_secs(1));
        (0..falhas)
            .map(|_| match r.apos(falha) {
                Decisao::Dormir(d) => d,
                Decisao::Estacionar => panic!("o par que goteja estacionou o laco"),
            })
            .collect()
    }

    /// Roda `f` numa thread e espera ate `PACIENCIA`: com o total fora do
    /// lugar o par que goteja prende, e o teste tem de reprovar em vez de
    /// pendurar a suite.
    fn sem_pendurar<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (avisar, aviso) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = avisar.send(f());
        });
        aviso
            .recv_timeout(PACIENCIA)
            .expect("preso ao par que goteja alem da paciencia")
    }

    /// **585: o par que goteja NA RODADA recua, e nao volta no intervalo
    /// fixo.** O erro e o de verdade, tirado do soquete pela porta que o laco
    /// usa (`ligar_com_prazo` + `puxar`), e a classificacao e a mesma que o
    /// `servidor::rodada_classificada` aplica (`Falha::na_rodada`). A primeira
    /// espera e a base -- a replica que gotejou uma vez volta no intervalo de
    /// sempre --, e dali em diante dobra.
    ///
    /// # Prova real
    ///
    /// Com o `LimiteExcedido` de volta a `Outra` em `na_rodada`, as esperas
    /// saem `[1, 1, 1, 1]`: o laco voltaria ao par a cada segundo.
    #[test]
    fn o_par_que_goteja_na_rodada_recua() {
        let porta = par_que_goteja();
        let erro = sem_pendurar(move || {
            ligar_com_prazo(&origem_para(porta), SILENCIO, Duration::from_secs(2))
                .and_then(|mut c| puxar(&mut c, "db", "t", 0).map(|_| ()))
                .expect_err("o par que goteja nao pode completar a rodada")
        });
        assert!(
            matches!(erro, PhxError::LimiteExcedido(_)),
            "o estouro do total saiu como {erro:?}"
        );
        let s = Duration::from_secs;
        assert_eq!(
            esperas_apos(Falha::na_rodada(&erro), 4),
            vec![s(1), s(2), s(4), s(8)],
            "o estouro do total na rodada tem de recuar como a rede"
        );
    }

    /// **585, o irmao: o par que goteja no DESAFIO, antes de entrar.** Mesma
    /// conversa, outra fase -- e a fase tem classificacao propria
    /// (`Falha::ao_ligar`), que e por onde o estouro escaparia para `Outra`
    /// se so a rodada fosse consertada.
    #[test]
    fn o_par_que_goteja_no_desafio_recua() {
        let porta = par_que_goteja();
        let erro = sem_pendurar(move || {
            let mut o = origem_para(porta);
            o.usuario = "replicador".into();
            o.senha = "irrelevante".into();
            ligar_com_prazo(&o, SILENCIO, Duration::from_secs(2))
                .map(|_| ())
                .expect_err("o par que goteja nao pode completar o desafio")
        });
        assert!(
            matches!(erro, PhxError::LimiteExcedido(_)),
            "o estouro do total no desafio saiu como {erro:?}"
        );
        let s = Duration::from_secs;
        assert_eq!(
            esperas_apos(Falha::ao_ligar(&erro), 3),
            vec![s(1), s(2), s(4)],
            "o estouro do total ao ligar tem de recuar como a rede"
        );
    }

    /// **O COMPORTAMENTO VELHO: a replica saudavel com lote longo continua.**
    /// Um source que manda cada resposta devagar -- um byte a cada 25 ms,
    /// ~2,5 s por lote, mais de doze silencios -- e legitimo: nenhum pedido
    /// passa do total de 4 s, mas a SOMA dos dois passa. O total e por
    /// pedido; se fosse pela vida da conexao, o segundo lote cairia, e e isso
    /// que derrubaria uma replica saudavel.
    #[test]
    fn a_replica_saudavel_com_lote_longo_continua() {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let resposta: &'static [u8] = br#"{"ok":true,"resultado":{"eventos":[],"ate":7,"fim":true,"folga":"................................"}}"#;
        std::thread::spawn(move || {
            let Ok((s, _)) = ouvinte.accept() else {
                return;
            };
            let Ok(mut escrita) = s.try_clone() else {
                return;
            };
            // O pedido se consome byte a byte ate a quebra: aqui so importa
            // saber que ele chegou, e o `Canal` e o lado de quem e testado.
            let mut leitor = s;
            let mut byte = [0u8; 1];
            loop {
                loop {
                    match leitor.read(&mut byte) {
                        Ok(1) if byte[0] == b'\n' => break,
                        Ok(1) => {}
                        _ => return,
                    }
                }
                for b in resposta.iter().chain(b"\n") {
                    std::thread::sleep(Duration::from_millis(25));
                    if escrita.write_all(&[*b]).is_err() {
                        return;
                    }
                }
            }
        });
        let mut c = ligar_com_prazo(&origem_para(porta), SILENCIO, Duration::from_secs(2)).unwrap();
        let inicio = Instant::now();
        for lote in 0..2 {
            let r = puxar_lote(&mut c, "db", "t", 0, None)
                .unwrap_or_else(|e| panic!("o lote {lote} de uma replica saudavel caiu: {e}"));
            assert_eq!(r.ate, 7);
        }
        let total = SILENCIO.saturating_mul(MULTIPLO_DO_TOTAL_DA_CONVERSA);
        assert!(
            inicio.elapsed() > total,
            "a soma dos lotes ({:?}) nao passou do total ({total:?}): a prova nao prova nada",
            inicio.elapsed()
        );
    }
}

#[cfg(test)]
mod testes_da_sequencia_no_fio {
    use super::*;

    #[test]
    fn o_contador_vai_e_volta_inteiro_inclusive_acima_de_2_elevado_a_53() {
        for n in [1u64, 42, (1 << 53) - 1, 1 << 53, (1 << 53) + 1, u64::MAX] {
            let fio = proxima_sequencia_para_o_fio(n).escrever();
            let lido = Json::analisar(&fio).unwrap();
            assert_eq!(proxima_sequencia_do_fio(Some(&lido)), n, "{n}: {fio}");
        }
    }

    #[test]
    fn campo_ausente_ou_torto_e_nao_disse() {
        assert_eq!(proxima_sequencia_do_fio(None), 0);
        assert_eq!(proxima_sequencia_do_fio(Some(&Json::Nulo)), 0);
        assert_eq!(proxima_sequencia_do_fio(Some(&Json::texto_de("abc"))), 0);
        assert_eq!(proxima_sequencia_do_fio(Some(&Json::Numero(-5.0))), 0);
    }
}

/// Pedido 676: o [`Juntador`] provado com listas na mao -- sem rede e sem
/// disco, que e onde a decisao mora.
#[cfg(test)]
mod testes_do_juntador {
    use super::*;

    fn ev(tx: u64, tamanho: usize) -> EventoRecebido {
        EventoRecebido {
            operacao: Operacao::Inclusao,
            rowid: 1,
            versao: 1,
            imagem: vec![7u8; tamanho],
            carimbo_ms: 0,
            origem: 0,
            posicao: POSICAO_DESCONHECIDA,
            tx,
        }
    }

    /// O grupo como `(fila, [tx...])`, para comparar.
    fn forma(grupo: &[(usize, Vec<EventoRecebido>)]) -> Vec<(usize, Vec<u64>)> {
        let mut v: Vec<(usize, Vec<u64>)> = grupo
            .iter()
            .map(|(i, es)| (*i, es.iter().map(|e| e.tx).collect()))
            .collect();
        v.sort();
        v
    }

    fn aplicar(j: &mut Juntador) -> (Vec<(usize, Vec<u64>)>, bool) {
        match j.passo() {
            Passo::Aplicar { grupo, inteiro } => (forma(&grupo), inteiro),
            Passo::Puxar { fila, desde } => panic!("pediu puxar {fila} desde {desde}"),
            Passo::Fim => panic!("acabou cedo"),
        }
    }

    fn puxar(j: &mut Juntador) -> (usize, u64) {
        match j.passo() {
            Passo::Puxar { fila, desde } => (fila, desde),
            Passo::Aplicar { grupo, .. } => panic!("aplicou {:?}", forma(&grupo)),
            Passo::Fim => panic!("acabou cedo"),
        }
    }

    /// **A venda que chega por partes nao se aplica por partes.** Os itens
    /// (fila 0) vem em dois lotes; a venda (fila 1) num so. Com o primeiro
    /// lote dos itens na mao a transacao 5 NAO esta inteira -- o ultimo item
    /// que chegou ainda e dela, e a tabela tem mais para dar --, entao o
    /// passo e puxar, e nunca aplicar.
    #[test]
    fn a_transacao_so_se_aplica_inteira_mesmo_vindo_em_dois_lotes() {
        let mut j = Juntador::novo(&[(0, 4), (0, 1)], TETO_DA_TRANSACAO);
        assert_eq!(puxar(&mut j), (0, 0));
        j.receber(0, vec![ev(5, 10), ev(5, 10)]);
        assert_eq!(puxar(&mut j), (1, 0));
        j.receber(1, vec![ev(5, 10)]);
        // O resto dos itens ainda nao chegou: aplicar aqui seria a venda com
        // metade dos itens.
        assert_eq!(puxar(&mut j), (0, 2));
        j.receber(0, vec![ev(5, 10), ev(9, 10)]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![5, 5, 5, 9]), (1, vec![5])]);
        assert!(matches!(j.passo(), Passo::Fim));
    }

    /// **Pedido 299, F2: a parada e por TRANSACAO.** A transacao 3 toca uma
    /// tabela que parou (a barreira): ela nao entra em NENHUMA tabela, nem a 4
    /// que vem depois; a 1 e a 2, de antes, entram. Com a parada por tabela
    /// de antes (so a fila rompida largada), o grupo levava a 3 e a 4 nas
    /// filas 0 e 1 -- a venda sem o pagamento.
    #[test]
    fn a_barreira_segura_a_transacao_inteira_e_as_seguintes() {
        let mut j = Juntador::novo(&[(0, 2), (0, 2), (0, 1)], TETO_DA_TRANSACAO);
        j.barrar(3);
        assert_eq!(puxar(&mut j), (0, 0));
        j.receber(0, vec![ev(1, 1), ev(3, 1)]);
        assert_eq!(puxar(&mut j), (1, 0));
        j.receber(1, vec![ev(3, 1), ev(4, 1)]);
        assert_eq!(puxar(&mut j), (2, 0));
        j.receber(2, vec![ev(2, 1)]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![1]), (2, vec![2])]);
        assert!(matches!(j.passo(), Passo::Fim));
        assert!(j.segurou);
        assert_eq!(j.tabelas_de(3), vec![0, 1]);
        // A menor barreira vale.
        j.barrar(9);
        assert!(matches!(j.passo(), Passo::Fim));
    }

    /// O comportamento VELHO: sem barreira, as mesmas filas entram inteiras
    /// e nada fica segurado.
    #[test]
    fn sem_barreira_nada_fica_segurado() {
        let mut j = Juntador::novo(&[(0, 2), (0, 2), (0, 1)], TETO_DA_TRANSACAO);
        assert_eq!(puxar(&mut j), (0, 0));
        j.receber(0, vec![ev(1, 1), ev(3, 1)]);
        assert_eq!(puxar(&mut j), (1, 0));
        j.receber(1, vec![ev(3, 1), ev(4, 1)]);
        assert_eq!(puxar(&mut j), (2, 0));
        j.receber(2, vec![ev(2, 1)]);
        let (g, _) = aplicar(&mut j);
        assert_eq!(g, vec![(0, vec![1, 3]), (1, vec![3, 4]), (2, vec![2])]);
        assert!(!j.segurou);
    }

    /// Transacoes inteiras seguidas dividem a tomada: o alcance de um diario
    /// de autocommits nao paga uma tomada por evento.
    #[test]
    fn transacoes_inteiras_seguidas_vao_na_mesma_tomada() {
        let mut j = Juntador::novo(&[(10, 13), (20, 22)], TETO_DA_TRANSACAO);
        assert_eq!(puxar(&mut j), (0, 10));
        j.receber(0, vec![ev(1, 1), ev(3, 1), ev(4, 1)]);
        assert_eq!(puxar(&mut j), (1, 20));
        j.receber(1, vec![ev(2, 1), ev(4, 1)]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![1, 3, 4]), (1, vec![2, 4])]);
    }

    /// A fronteira do `posicao` e de commit: o que vem depois dela fica para
    /// a proxima rodada, mesmo que o source o mande.
    #[test]
    fn o_que_passa_da_fronteira_do_posicao_nao_entra() {
        let mut j = Juntador::novo(&[(0, 2)], TETO_DA_TRANSACAO);
        puxar(&mut j);
        j.receber(0, vec![ev(1, 1), ev(1, 1), ev(2, 1)]);
        let (g, _) = aplicar(&mut j);
        assert_eq!(g, vec![(0, vec![1, 1])]);
        assert!(matches!(j.passo(), Passo::Fim));
    }

    /// O evento sem id (volume velho, source anterior ao 676) vai sozinho,
    /// como sempre foi -- e nao segura nada esperando.
    #[test]
    fn o_evento_sem_id_nao_espera_ninguem() {
        let mut j = Juntador::novo(&[(0, 3), (0, 1)], TETO_DA_TRANSACAO);
        puxar(&mut j);
        j.receber(0, vec![ev(0, 1), ev(0, 1)]);
        puxar(&mut j);
        j.receber(1, vec![ev(0, 1)]);
        // A fila 0 ainda tem um evento a puxar e esta com dois na mao: os
        // dois sem id se aplicam sem esperar o terceiro.
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![0, 0]), (1, vec![0])]);
    }

    /// **Acima do teto, em pedacos -- contados.** A transacao 7 nao cabe:
    /// vai o que esta na mao, `inteiro` falso, e a conta sobe UMA vez. O
    /// ultimo pedaco tambem sai marcado, e a transacao seguinte volta a ser
    /// inteira.
    #[test]
    fn a_transacao_acima_do_teto_vai_em_pedacos_e_e_contada_uma_vez() {
        let mut j = Juntador::novo(&[(0, 6), (0, 1)], 1_000);
        puxar(&mut j);
        j.receber(0, vec![ev(7, 400), ev(7, 400)]);
        puxar(&mut j);
        j.receber(1, vec![ev(7, 400)]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(!inteiro);
        assert_eq!(g, vec![(0, vec![7, 7]), (1, vec![7])]);
        assert_eq!(j.em_pedacos, 1);
        assert_eq!(puxar(&mut j), (0, 2));
        j.receber(0, vec![ev(7, 400), ev(7, 400)]);
        let (_, inteiro) = aplicar(&mut j);
        assert!(!inteiro);
        assert_eq!(j.em_pedacos, 1, "a mesma transacao contou duas vezes");
        puxar(&mut j);
        j.receber(0, vec![ev(7, 1), ev(8, 1)]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(!inteiro, "o ultimo pedaco da partida saiu como inteiro");
        assert_eq!(g, vec![(0, vec![7])]);
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![8])]);
        assert_eq!(j.em_pedacos, 1);
    }

    /// O source que encolheu entre o `posicao` e o `replicar` (lote vazio)
    /// e a tabela que rompeu saem da rodada sem pendurar o laco.
    #[test]
    fn a_tabela_vazia_ou_rompida_nao_pendura_a_rodada() {
        let mut j = Juntador::novo(&[(0, 5), (0, 2)], TETO_DA_TRANSACAO);
        puxar(&mut j);
        j.receber(0, vec![]);
        puxar(&mut j);
        j.receber(1, vec![ev(1, 1)]);
        j.largar(1);
        assert!(matches!(j.passo(), Passo::Fim));
    }

    fn ev_em(tx: u64, posicao: u64) -> EventoRecebido {
        EventoRecebido {
            posicao,
            ..ev(tx, 1)
        }
    }

    /// **O lote do bidirecional anda pela posicao, nao pela contagem**
    /// (pedido 681). O source suprimiu os eventos 1 e 2 (nasceram aqui): a
    /// fila vai a 4, e a transacao 5 -- que tem o evento 3 da fila 0 e o
    /// unico da fila 1 -- so se aplica com as duas na mao. O consumido de
    /// cada fila e onde a gravacao da posicao pode chegar.
    #[test]
    fn o_lote_com_eventos_suprimidos_anda_pela_posicao() {
        let mut j = Juntador::novo(&[(0, 4), (0, 1)], TETO_DA_TRANSACAO);
        assert_eq!(puxar(&mut j), (0, 0));
        j.receber_ate(0, vec![ev_em(4, 0), ev_em(5, 3)], 4);
        assert_eq!(j.consumido(0), 0);
        // A 4 esta inteira (a fila 0 ja passou dela); a 5 espera a fila 1.
        assert_eq!(puxar(&mut j), (1, 0));
        j.receber_ate(1, vec![ev_em(5, 0)], 1);
        let (g, inteiro) = aplicar(&mut j);
        assert!(inteiro);
        assert_eq!(g, vec![(0, vec![4, 5]), (1, vec![5])]);
        assert_eq!((j.consumido(0), j.consumido(1)), (4, 1));
        assert!(matches!(j.passo(), Passo::Fim));
    }

    /// Com a transacao esperando na mao, o consumido NAO passa dela: gravar
    /// a posicao alem faria o evento nunca mais ser pedido.
    #[test]
    fn o_consumido_nao_passa_da_transacao_que_espera() {
        let mut j = Juntador::novo(&[(10, 14), (0, 2)], TETO_DA_TRANSACAO);
        puxar(&mut j);
        j.receber_ate(0, vec![ev_em(2, 10), ev_em(7, 13)], 14);
        assert_eq!(puxar(&mut j), (1, 0));
        j.receber_ate(1, vec![ev_em(7, 0)], 1);
        // A 2 entra; a 7 ainda pode continuar na fila 1, e espera na mao --
        // o consumido da fila 0 e o 13 dela, nao o 14.
        let (g, _) = aplicar(&mut j);
        assert_eq!(g, vec![(0, vec![2])]);
        assert_eq!(j.consumido(0), 13);
        assert_eq!(puxar(&mut j), (1, 1));
        j.receber_ate(1, vec![], 1);
        let (g, _) = aplicar(&mut j);
        assert_eq!(g, vec![(0, vec![7]), (1, vec![7])]);
        assert_eq!(j.consumido(0), 14);
    }

    #[test]
    fn o_id_vai_e_volta_pelo_fio_inteiro() {
        for tx in [1u64, (1 << 53) + 1, u64::MAX - 1] {
            let fio = tx_para_o_fio(tx).escrever();
            assert_eq!(tx_do_fio(Some(&Json::analisar(&fio).unwrap())), tx);
        }
        assert_eq!(tx_do_fio(None), 0);
        assert_eq!(tx_do_fio(Some(&Json::texto_de("x"))), 0);
    }
}

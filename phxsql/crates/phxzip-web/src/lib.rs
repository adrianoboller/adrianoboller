//! PhxZipWeb -- a porta web do PhxZip (pedido 454, fatias Z5 a Z8).
//!
//! O contrato e o `docs/PHXZIP-WEB.md`; onde este arquivo e o contrato
//! discordarem, o contrato manda. A Z5 entregou o servidor e os estaticos; as
//! rotas `/api/*` (Z6 a Z8) moram em [`ops`] e os textos em [`textos`].
//!
//! # As decisoes que seguram a porta
//!
//! * **So `127.0.0.1`.** A porta extrai arquivos e aceita senha. Nao ha
//!   opcao de endereco, nem na linha de comando nem na [`Config`]: o endereco
//!   sai de [`endereco_de_escuta`], que so recebe a porta. Opcao que nao
//!   existe nao se liga por engano.
//! * **`Host` conferido em todo pedido.** Sem isso, um site de fora religa o
//!   proprio nome para `127.0.0.1` (DNS rebinding) e o navegador da pessoa
//!   conversa com esta porta achando que fala com o site.
//! * **Estaticos por lista fechada, embutidos.** Nao existe «servir a pasta
//!   `ui/`», entao nao existe `../` para pedir: o caminho e comparado por
//!   igualdade com quatro nomes, e o resto e 404.
//! * **A recusa vem ANTES do corpo.** A cabeca do pedido e lida primeiro
//!   ([`http::ler_cabeca`]); `Host`, `Origin`, rota, tipo, tamanho e a vaga
//!   do `simultaneas` se decidem ali, e so o pedido que passou recebe o
//!   corpo. Quem recusa ali responde e DRENA (contrato §4): sem o dreno, o
//!   navegador que ainda esta mandando ve erro de rede no lugar da recusa.
//!
//! # O HTTP vem do core
//!
//! A leitura e o [`phxsql_core::http`] e a montagem da cabeca e o
//! [`phxsql_core::http::montar_cabeca`] -- o mesmo motor do servidor do
//! PhxSql (petrea «funcao e comando vem do mesmo motor»). Daqui sai so o que
//! e desta porta: os tetos, a politica de seguranca e as rotas.

pub mod ops;
pub mod textos;

use std::borrow::Cow;
use std::io::{BufReader, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phxsql_core::http::{self, CabecaLida, Excesso, Pedido, Tetos};
use phxsql_core::json::Json;
use phxsql_core::semaforo::{Permissao, Semaforo};

/// A porta de fabrica. Livre das quatro do PhxSql (5000, 5001, 6000, 7000);
/// a tela nao a conhece, porque so usa caminho relativo.
pub const PORTA_PADRAO: u16 = 7700;

/// O teto do corpo de um pedido -- o `limites.envio` do contrato (§4),
/// conferido no `Content-Length` ANTES de ler um byte do corpo.
pub const ENVIO_MAX: usize = 256 * 1024 * 1024;

/// O teto de cada linha e do cabecalho somado. O mesmo da porta do PhxSql:
/// navegador nenhum manda 16 KiB de cabecalho para uma pagina local.
pub const CABECALHO_MAX: usize = 16 * 1024;

/// Quantas conexoes se atendem ao mesmo tempo. Nao e o `simultaneas` do
/// contrato -- aquele conta os `POST` que descomprimem ([`ops::SIMULTANEAS`]).
/// Este e o teto de threads da porta: sem ele, cada conexao aberta e uma
/// thread, e quem abre mil sem mandar nada prende mil threads por
/// [`PRAZO_DE_LEITURA`].
pub const CONEXOES_MAX: usize = 32;

/// Quanto se espera por um pedido que comecou e nao terminou.
const PRAZO_DE_LEITURA: Duration = Duration::from_secs(10);

/// Quanto uma escrita da resposta pode ficar parada. A vaga do `simultaneas`
/// e segurada ate a resposta sair (ela e um dos tres pedacos da memoria de
/// pico), e um cliente que para de ler nao pode prende-la para sempre.
const PRAZO_DE_ESCRITA: Duration = Duration::from_secs(30);

/// O prazo do dreno, por leitura (ver [`http::drenar`]). Na mesma maquina o
/// navegador que ainda esta mandando nao fica um segundo calado; o que ficou,
/// parou.
const PRAZO_DO_DRENO: Duration = Duration::from_secs(1);

/// A politica de toda resposta (contrato §7). Sem `'unsafe-inline'`: um nome
/// de entrada que escapasse do `textContent` ainda nao rodaria. A tela foi
/// exercitada sob esta politica e nao viola nenhuma diretiva.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; \
form-action 'none'; frame-ancestors 'none'";

/// Um estatico: a rota, o tipo e os bytes, embutidos no binario.
struct Estatico {
    rota: &'static str,
    tipo: &'static str,
    bytes: &'static [u8],
}

/// A lista fechada do contrato (§2). O `textos.json` NAO esta aqui de
/// proposito: ele se serve resolvido por idioma (§5), nunca cru.
const ESTATICOS: [Estatico; 4] = [
    Estatico {
        rota: "/",
        tipo: "text/html; charset=utf-8",
        bytes: include_bytes!("../ui/index.html"),
    },
    Estatico {
        rota: "/phxzip.css",
        tipo: "text/css; charset=utf-8",
        bytes: include_bytes!("../ui/phxzip.css"),
    },
    Estatico {
        rota: "/phxzip.js",
        tipo: "text/javascript; charset=utf-8",
        bytes: include_bytes!("../ui/phxzip.js"),
    },
    Estatico {
        rota: "/fonte/exo2-latin.woff2",
        tipo: "font/woff2",
        bytes: include_bytes!("../ui/fonte/exo2-latin.woff2"),
    },
];

/// O que se pode ajustar. Nao ha campo de endereco, e isso e a decisao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Zero pede ao sistema uma porta livre (os testes usam isso).
    pub porta: u16,
    /// O teto do corpo, ate [`ENVIO_MAX`]. Os testes e a prova no navegador
    /// o baixam para provar o 413 sem mandar 256 MiB.
    pub envio: usize,
    /// A pasta onde `/api/extrair` com `"na_pasta": true` grava. Quem a
    /// escolhe e o OPERADOR, na linha de comando; o navegador nunca manda
    /// caminho (contrato §1). Sem ela, a extracao e so download.
    pub pasta: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            porta: PORTA_PADRAO,
            envio: ENVIO_MAX,
            pasta: None,
        }
    }
}

/// O endereco em que a porta escuta. So recebe a porta: o IP e o
/// `127.0.0.1`, sempre.
pub fn endereco_de_escuta(porta: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, porta))
}

/// O que a linha de comando pediu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acao {
    Servir(Config),
    Versao,
    Ajuda,
}

/// O texto do `--ajuda`.
pub const AJUDA: &str = "phxzipweb -- o PhxZip no navegador, so nesta maquina\n\
\n\
uso: phxzipweb [--porta N] [--pasta DIR] [--envio BYTES]\n\
\n\
  --porta N      a porta em 127.0.0.1 (padrao 7700)\n\
  --pasta DIR    onde a tela pode extrair (\"extrair na pasta\"); sem ela, so\n\
                 download. A pasta e de quem roda o phxzipweb, sem escrita\n\
                 para outros usuarios -- senao a porta nao sobe\n\
  --envio BYTES  baixa o teto do envio (padrao e maximo: 256 MiB)\n\
  -V, --version  a versao\n\
  -h, --ajuda    este texto\n\
\n\
Nao ha opcao de endereco: a porta extrai arquivos e aceita senha, e por\n\
isso escuta so em 127.0.0.1.\n";

/// Le os argumentos. Qualquer coisa que nao seja porta, versao ou ajuda e
/// recusada com o motivo -- inclusive o pedido de outro endereco, que recebe
/// a explicacao em vez de um «opcao desconhecida» que convida a procurar a
/// grafia certa.
pub fn ler_argumentos(args: &[String]) -> Result<Acao, String> {
    let mut cfg = Config::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let (nome, valor_junto) = match a.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (a, None),
        };
        let mut valor = |pede: &str| -> Result<String, String> {
            match valor_junto.clone() {
                Some(v) => Ok(v),
                None => {
                    i += 1;
                    args.get(i).cloned().ok_or_else(|| pede.to_string())
                }
            }
        };
        match nome {
            "-V" | "--version" | "--versao" => return Ok(Acao::Versao),
            "-h" | "--help" | "--ajuda" => return Ok(Acao::Ajuda),
            "--porta" | "--port" => {
                let v = valor("--porta pede um numero de 1 a 65535")?;
                cfg.porta = match v.parse::<u16>() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        return Err(format!(
                            "porta invalida: {v:?} -- e um numero de 1 a 65535; \
                             o endereco e sempre 127.0.0.1"
                        ))
                    }
                };
            }
            "--pasta" => {
                let v = valor("--pasta pede uma pasta")?;
                if v.is_empty() {
                    return Err("--pasta pede uma pasta".to_string());
                }
                cfg.pasta = Some(PathBuf::from(v));
            }
            "--envio" => {
                let v = valor("--envio pede um numero de bytes")?;
                cfg.envio = match v.parse::<usize>() {
                    Ok(n) if n > 0 && n <= ENVIO_MAX => n,
                    _ => return Err(format!("envio invalido: {v:?} -- de 1 a {ENVIO_MAX} bytes")),
                };
            }
            "--endereco" | "--host" | "--bind" | "--escutar" | "--listen" => {
                return Err("nao ha opcao de endereco: o PhxZipWeb escuta so em \
                            127.0.0.1, porque a porta extrai arquivos e aceita senha"
                    .to_string())
            }
            _ => return Err(format!("argumento desconhecido: {a} (veja --ajuda)")),
        }
        i += 1;
    }
    Ok(Acao::Servir(cfg))
}

/// Uma resposta pronta para escrever.
#[derive(Debug, Clone)]
pub struct Resposta {
    pub codigo: u16,
    pub tipo: &'static str,
    /// Cabecalhos desta resposta alem dos comuns, cada um terminado em
    /// `\r\n` (o `Allow` do 405).
    pub extras: String,
    pub corpo: Cow<'static, [u8]>,
}

impl Resposta {
    /// O erro do contrato (§6): `{"ok":false,"erro":NOME}`, com o `detalhe`
    /// quando houver. A tela decide pelo nome, nunca pela frase -- e por isso
    /// nao ha frase.
    pub fn erro(codigo: u16, nome: &str, detalhe: Option<Json>) -> Resposta {
        let mut pares = vec![("ok", Json::Bool(false)), ("erro", Json::texto_de(nome))];
        if let Some(d) = detalhe {
            pares.push(("detalhe", d));
        }
        Resposta {
            codigo,
            tipo: "application/json; charset=utf-8",
            extras: String::new(),
            corpo: Cow::Owned(Json::objeto(pares).escrever().into_bytes()),
        }
    }

    /// Os bytes da cabeca, com a politica de toda resposta (contrato §7).
    pub fn cabeca(&self) -> String {
        let politica = format!(
            "Content-Security-Policy: {CSP}\r\n\
             X-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\n\
             Cache-Control: no-store\r\n\
             {}",
            self.extras
        );
        http::montar_cabeca(self.codigo, self.tipo, self.corpo.len(), &politica)
    }

    /// Escreve cabeca e corpo. Dois `write_all`, e nao uma copia do corpo
    /// atras da cabeca: o corpo e a fonte embutida hoje e um pacote amanha.
    pub fn escrever<W: Write + ?Sized>(&self, fluxo: &mut W) -> std::io::Result<()> {
        fluxo.write_all(self.cabeca().as_bytes())?;
        fluxo.write_all(&self.corpo)?;
        fluxo.flush()
    }
}

/// O `Host` e desta porta? So `127.0.0.1:<porta>` e `localhost:<porta>`
/// (contrato §6). Sem `Host` tambem e recusa: HTTP/1.1 o exige (RFC 9112
/// §3.2), e o navegador sempre manda.
fn host_valido(pedido: &Pedido, porta: u16) -> bool {
    let Some(h) = pedido.cabecalho("host") else {
        return false;
    };
    let h = h.to_ascii_lowercase();
    h == format!("127.0.0.1:{porta}") || h == format!("localhost:{porta}")
}

/// A origem e desta porta? Vale para o que nao e `GET`: o `POST` de outra
/// origem com `application/octet-stream` ja cai no preflight, e esta
/// conferencia e a segunda tranca (contrato §7).
fn origem_valida(pedido: &Pedido, porta: u16) -> bool {
    if let Some(o) = pedido.cabecalho("origin") {
        let o = o.to_ascii_lowercase();
        if o != format!("http://127.0.0.1:{porta}") && o != format!("http://localhost:{porta}") {
            return false;
        }
    }
    match pedido.cabecalho("sec-fetch-site") {
        Some(s) => s.eq_ignore_ascii_case("same-origin"),
        None => true,
    }
}

/// Uma operacao `POST` sobre o motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Compactar,
    Listar,
    Testar,
    Extrair,
}

/// O que um caminho e.
#[derive(Clone, Copy)]
enum Rota {
    Estatico(&'static Estatico),
    Estado,
    Idiomas,
    Op(Op),
}

impl Rota {
    fn de(caminho: &str) -> Option<Rota> {
        // Igualdade com a lista fechada, e nada mais: nao se normaliza, nao
        // se decodifica `%2e`, nao se junta a caminho nenhum. `/../Cargo.toml`
        // nao e igual a nenhuma rota, e e 404 pelo mesmo motivo que `/x` e.
        if let Some(e) = ESTATICOS.iter().find(|e| e.rota == caminho) {
            return Some(Rota::Estatico(e));
        }
        Some(match caminho {
            "/api/estado" => Rota::Estado,
            "/api/idiomas" => Rota::Idiomas,
            "/api/compactar" => Rota::Op(Op::Compactar),
            "/api/listar" => Rota::Op(Op::Listar),
            "/api/testar" => Rota::Op(Op::Testar),
            "/api/extrair" => Rota::Op(Op::Extrair),
            _ => return None,
        })
    }

    fn metodo(self) -> &'static str {
        match self {
            Rota::Op(_) => "POST",
            _ => "GET",
        }
    }
}

/// O que cada atendimento precisa saber da porta.
#[derive(Debug, Clone)]
pub struct Contexto {
    pub porta: u16,
    pub envio: usize,
    pub pasta: Option<PathBuf>,
}

/// O tipo de midia do pedido, sem parametros e sem caixa.
fn tipo_do_pedido(pedido: &Pedido) -> Option<String> {
    pedido.cabecalho("content-type").map(|t| {
        t.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
    })
}

/// A triagem pela CABECA: tudo o que se decide sem o corpo. `Err` e a
/// recusa, ja pronta.
fn triar(pedido: &Pedido, porta: u16) -> Result<Rota, Resposta> {
    if !host_valido(pedido, porta) {
        return Err(Resposta::erro(403, "HOST_RECUSADO", None));
    }
    if pedido.metodo != "GET" && !origem_valida(pedido, porta) {
        return Err(Resposta::erro(403, "ORIGEM_RECUSADA", None));
    }
    let Some(rota) = Rota::de(&pedido.caminho) else {
        return Err(Resposta::erro(404, "ROTA_INEXISTENTE", None));
    };
    if pedido.metodo != rota.metodo() {
        let mut r = Resposta::erro(405, "METODO_HTTP", None);
        r.extras = format!("Allow: {}\r\n", rota.metodo());
        return Err(r);
    }
    if let Rota::Op(_) = rota {
        // `application/octet-stream` nao e tipo «simples»: o `POST` de outra
        // origem cai no preflight, que esta porta nao responde (contrato §7).
        if tipo_do_pedido(pedido).as_deref() != Some("application/octet-stream") {
            return Err(Resposta::erro(415, "TIPO_DE_CONTEUDO", None));
        }
        if pedido.cabecalho("content-length").is_none() {
            return Err(Resposta::erro(411, "TAMANHO_AUSENTE", None));
        }
    }
    Ok(rota)
}

/// A resposta de uma rota que ja passou pela triagem, com o corpo lido.
fn executar(rota: Rota, pedido: &Pedido, ctx: &Contexto) -> Resposta {
    match rota {
        Rota::Estatico(e) => Resposta {
            codigo: 200,
            tipo: e.tipo,
            extras: String::new(),
            corpo: Cow::Borrowed(e.bytes),
        },
        Rota::Estado => ops::estado(ctx.envio, ctx.pasta.is_some()),
        Rota::Idiomas => ops::idiomas(&pedido.consulta),
        Rota::Op(op) => {
            let env = match ops::abrir_envelope(&pedido.corpo) {
                Ok(e) => e,
                Err(r) => return r,
            };
            let r = match op {
                Op::Compactar => ops::compactar(&env),
                Op::Listar => ops::listar(&env),
                Op::Testar => ops::testar(&env),
                Op::Extrair => ops::extrair(&env, ctx.pasta.as_deref()),
            };
            r.unwrap_or_else(|e| e)
        }
    }
}

/// A resposta para um pedido ja lido INTEIRO: a triagem e a rota. Nao toca
/// no soquete nem na vaga do `simultaneas`, e por isso se testa sem rede; a
/// prova pelo soquete esta em `tests/soquete.rs`.
pub fn decidir(pedido: &Pedido, ctx: &Contexto) -> Resposta {
    match triar(pedido, ctx.porta) {
        Ok(rota) => executar(rota, pedido, ctx),
        Err(r) => r,
    }
}

/// O 413/431 do contrato (§4). O `oque` diz qual teto: `envio` e o do corpo,
/// o mesmo nome que `/api/estado` vai declarar; `cabecalho_http` e o do
/// cabecalho do pedido, que a tela nunca alcanca.
fn resposta_do_excesso(excesso: &Excesso) -> Resposta {
    let detalhe = match excesso {
        Excesso::Corpo { declarado, teto } => Json::objeto(vec![
            ("oque", Json::texto_de("envio")),
            ("declarado", Json::de_u64(*declarado)),
            ("teto", Json::de_u64(*teto as u64)),
        ]),
        Excesso::Linha { teto }
        | Excesso::LinhaDeCabecalho { teto }
        | Excesso::Cabecalho { teto } => Json::objeto(vec![
            ("oque", Json::texto_de("cabecalho_http")),
            ("teto", Json::de_u64(*teto as u64)),
        ]),
    };
    Resposta::erro(excesso.codigo_http(), "GRANDE_DEMAIS", Some(detalhe))
}

/// O que fazer com a conexao, decidido enquanto o leitor ainda a empresta.
enum Passo {
    /// Responder; a vaga do `simultaneas`, quando houver, vai junto ate a
    /// resposta sair.
    Responder(Resposta, Option<Permissao>),
    /// Responder sem ter lido o corpo -- e drenar se ele estiver no fio.
    Recusar(Resposta, bool),
}

fn http_torto() -> Resposta {
    let detalhe = Json::objeto(vec![("campo", Json::texto_de("http"))]);
    Resposta::erro(400, "PEDIDO_MALFORMADO", Some(detalhe))
}

/// Atende UMA conexao: le a cabeca pelo motor do core, faz a triagem, toma a
/// vaga se a rota descomprime, le o corpo, responde e fecha.
pub fn atender(mut fluxo: TcpStream, ctx: &Contexto, ops_vagas: &Semaforo) {
    let _ = fluxo.set_read_timeout(Some(PRAZO_DE_LEITURA));
    let _ = fluxo.set_write_timeout(Some(PRAZO_DE_ESCRITA));
    let tetos = Tetos {
        cabecalho: CABECALHO_MAX,
        corpo: ctx.envio,
    };
    let passo = {
        let mut leitor = BufReader::new(&fluxo);
        match http::ler_cabeca(&mut leitor, tetos) {
            CabecaLida::Cabeca(mut p, tamanho) => {
                let no_fio = tamanho > 0 || p.cabecalho("transfer-encoding").is_some();
                match triar(&p, ctx.porta) {
                    Err(r) => Passo::Recusar(r, no_fio),
                    Ok(rota) => {
                        let vaga = match rota {
                            Rota::Op(_) => ops_vagas.tentar().map(Some),
                            _ => Some(None),
                        };
                        match vaga {
                            // Sem vaga: recusa ANTES de receber o pacote --
                            // e o que faz a memoria de pico ser a declarada.
                            None => Passo::Recusar(Resposta::erro(503, "OCUPADO", None), no_fio),
                            Some(vaga) => {
                                if http::ler_corpo(&mut leitor, &mut p, tamanho) {
                                    // Panico vira 500 sem o texto dele (contrato
                                    // §6): a mensagem de um panico e do programa,
                                    // e pode carregar dado do pedido.
                                    let r =
                                        catch_unwind(AssertUnwindSafe(|| executar(rota, &p, ctx)))
                                            .unwrap_or_else(|_| {
                                                Resposta::erro(500, "INTERNO", None)
                                            });
                                    Passo::Responder(r, vaga)
                                } else {
                                    Passo::Responder(http_torto(), None)
                                }
                            }
                        }
                    }
                }
            }
            // Conexao vazia ou pedido torto. Se o outro lado ainda ouve, ouve
            // o motivo; se ja foi, o erro de escrita nao interessa a ninguem.
            CabecaLida::Nada => Passo::Responder(http_torto(), None),
            // Recusou ANTES de ler o corpo: o resto ainda esta chegando.
            CabecaLida::GrandeDemais(excesso) => {
                Passo::Recusar(resposta_do_excesso(&excesso), true)
            }
        }
    };
    match passo {
        Passo::Responder(r, vaga) => {
            let _ = r.escrever(&mut fluxo);
            drop(vaga);
        }
        Passo::Recusar(r, drenar) => {
            let _ = r.escrever(&mut fluxo);
            if drenar {
                // Sem drenar, o fecho vira RST e o navegador ve «erro de rede»
                // no lugar da recusa (contrato §4). O descarte e o dobro do
                // teto; acima dele fecha, e o cliente ve a conexao cair, que e
                // verdade.
                let _ = fluxo.shutdown(Shutdown::Write);
                http::drenar(&mut fluxo, 2 * ctx.envio as u64, PRAZO_DO_DRENO);
            }
        }
    }
}

/// Confere a pasta de extracao do operador pelo `disco::Destino` do motor --
/// a mesma regra que cada extracao confere de novo. Existe para a porta NAO
/// subir com uma pasta que toda extracao recusaria: o erro sai no terminal de
/// quem a escolheu, e nao num cartao do navegador horas depois.
pub fn conferir_pasta(pasta: &Path) -> Result<(), String> {
    phxzip::disco::Destino::novo(pasta)
        .map(|_| ())
        .map_err(|e| format!("a pasta de extracao {pasta:?} foi recusada: {e}"))
}

/// A porta aberta, antes de comecar a atender.
pub struct Servidor {
    ouvinte: TcpListener,
    ctx: Arc<Contexto>,
    vagas: Semaforo,
    ops: Semaforo,
}

impl Servidor {
    /// Abre a porta em [`endereco_de_escuta`]. Separado de [`Servidor::servir`]
    /// para quem chama saber o endereco de verdade (porta zero) antes de a
    /// thread ficar presa no laco.
    ///
    /// Recusa subir com o dicionario de textos torto ou com uma pasta de
    /// extracao que o motor recusa.
    pub fn escutar(cfg: Config) -> std::io::Result<Servidor> {
        textos::tabela().map_err(std::io::Error::other)?;
        if let Some(p) = &cfg.pasta {
            conferir_pasta(p).map_err(std::io::Error::other)?;
        }
        let ouvinte = TcpListener::bind(endereco_de_escuta(cfg.porta))?;
        let porta = ouvinte.local_addr()?.port();
        Ok(Servidor {
            ouvinte,
            ctx: Arc::new(Contexto {
                porta,
                envio: cfg.envio.min(ENVIO_MAX),
                pasta: cfg.pasta,
            }),
            vagas: Semaforo::novo(CONEXOES_MAX),
            ops: Semaforo::novo(ops::SIMULTANEAS),
        })
    }

    /// O endereco em que o sistema operacional diz que a porta esta.
    pub fn endereco(&self) -> std::io::Result<SocketAddr> {
        self.ouvinte.local_addr()
    }

    /// Atende para sempre, uma thread por conexao, ate [`CONEXOES_MAX`].
    pub fn servir(self) -> std::io::Result<()> {
        for conexao in self.ouvinte.incoming() {
            let Ok(mut fluxo) = conexao else { continue };
            let Some(vaga) = self.vagas.tentar() else {
                // Sem vaga: responde aqui, no aceitador, e com dreno curto --
                // o pedido quase sempre ja veio junto do `connect`, e fechar
                // com ele no buffer seria RST no lugar do 503.
                let _ = Resposta::erro(503, "OCUPADO", None).escrever(&mut fluxo);
                http::drenar(&mut fluxo, CABECALHO_MAX as u64, Duration::from_millis(20));
                continue;
            };
            let ctx = Arc::clone(&self.ctx);
            let ops = self.ops.clone();
            let lancou = std::thread::Builder::new()
                .name("phxzipweb".into())
                .spawn(move || {
                    let _vaga = vaga;
                    atender(fluxo, &ctx, &ops);
                });
            if lancou.is_err() {
                // Sem thread a conexao cai sozinha (o fluxo foi junto e
                // morreu); nao ha a quem responder daqui.
                continue;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    const PORTA: u16 = 7700;

    fn ctx() -> Contexto {
        Contexto {
            porta: PORTA,
            envio: ENVIO_MAX,
            pasta: None,
        }
    }

    fn pedido(metodo: &str, caminho: &str, cabecalhos: &[(&str, &str)]) -> Pedido {
        let mut c: HashMap<String, String> = cabecalhos
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            .collect();
        c.entry("host".into())
            .or_insert_with(|| format!("127.0.0.1:{PORTA}"));
        Pedido {
            metodo: metodo.into(),
            caminho: caminho.into(),
            consulta: String::new(),
            cabecalhos: c,
            corpo: Vec::new(),
        }
    }

    fn erro_de(r: &Resposta) -> String {
        let j = Json::analisar(std::str::from_utf8(&r.corpo).unwrap()).unwrap();
        j.campo("erro").and_then(Json::texto).unwrap().to_string()
    }

    #[test]
    fn os_quatro_estaticos_saem_com_o_tipo_certo() {
        for (rota, tipo) in [
            ("/", "text/html; charset=utf-8"),
            ("/phxzip.css", "text/css; charset=utf-8"),
            ("/phxzip.js", "text/javascript; charset=utf-8"),
            ("/fonte/exo2-latin.woff2", "font/woff2"),
        ] {
            let r = decidir(&pedido("GET", rota, &[]), &ctx());
            assert_eq!((r.codigo, r.tipo), (200, tipo), "{rota}");
            assert!(!r.corpo.is_empty(), "{rota}");
        }
        // A fonte e binaria e chega como esta no disco: 40.896 bytes,
        // assinatura `wOF2` (contrato §2).
        let f = decidir(&pedido("GET", "/fonte/exo2-latin.woff2", &[]), &ctx());
        assert_eq!(f.corpo.len(), 40_896);
        assert_eq!(&f.corpo[..4], b"wOF2");
    }

    /// O que esta em `ui/` e nao esta na lista nao se serve -- o `textos.json`
    /// e a licenca inclusive (o primeiro sai resolvido, §5).
    #[test]
    fn fora_da_lista_e_404() {
        for rota in [
            "/textos.json",
            "/fonte/OFL.txt",
            "/index.html",
            "/../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/fonte/../phxzip.js",
            "//phxzip.js",
            "/api/",
            "/api/estado/",
            "/api/../api/estado",
            "",
        ] {
            let r = decidir(&pedido("GET", rota, &[]), &ctx());
            assert_eq!(r.codigo, 404, "{rota:?}");
            assert_eq!(erro_de(&r), "ROTA_INEXISTENTE");
        }
    }

    #[test]
    fn host_alheio_e_recusado_e_os_dois_nomes_da_casa_passam() {
        for host in [
            "evil.com",
            "evil.com:7700",
            "127.0.0.1",
            "127.0.0.1:7701",
            "localhost",
            "0.0.0.0:7700",
            "127.0.0.1:7700.evil.com",
        ] {
            let r = decidir(&pedido("GET", "/", &[("Host", host)]), &ctx());
            assert_eq!(r.codigo, 403, "{host}");
            assert_eq!(erro_de(&r), "HOST_RECUSADO");
        }
        let mut sem_host = pedido("GET", "/", &[]);
        sem_host.cabecalhos.remove("host");
        assert_eq!(decidir(&sem_host, &ctx()).codigo, 403);
        for host in ["127.0.0.1:7700", "localhost:7700", "LocalHost:7700"] {
            assert_eq!(
                decidir(&pedido("GET", "/", &[("Host", host)]), &ctx()).codigo,
                200
            );
        }
    }

    #[test]
    fn origem_alheia_recusa_o_que_nao_e_get() {
        for (nome, valor) in [
            ("Origin", "http://evil.com"),
            ("Origin", "http://127.0.0.1:7701"),
            ("Origin", "null"),
            ("Sec-Fetch-Site", "cross-site"),
            ("Sec-Fetch-Site", "same-site"),
        ] {
            let r = decidir(&pedido("POST", "/", &[(nome, valor)]), &ctx());
            assert_eq!(r.codigo, 403, "{nome}: {valor}");
            assert_eq!(erro_de(&r), "ORIGEM_RECUSADA");
        }
        // A mesma origem passa a tranca e cai no 405 da rota de leitura.
        let r = decidir(
            &pedido(
                "POST",
                "/",
                &[
                    ("Origin", "http://127.0.0.1:7700"),
                    ("Sec-Fetch-Site", "same-origin"),
                ],
            ),
            &ctx(),
        );
        assert_eq!(r.codigo, 405);
        assert_eq!(r.extras, "Allow: GET\r\n");
        assert_eq!(erro_de(&r), "METODO_HTTP");
        // E nas rotas que gravam: a origem alheia e recusada ANTES da rota.
        for rota in [
            "/api/compactar",
            "/api/extrair",
            "/api/listar",
            "/api/testar",
        ] {
            let r = decidir(
                &pedido("POST", rota, &[("Origin", "http://evil.com")]),
                &ctx(),
            );
            assert_eq!((r.codigo, erro_de(&r)), (403, "ORIGEM_RECUSADA".into()));
        }
    }

    /// Os `POST` exigem o tipo e o tamanho (contrato §6): 415 e 411. O GET
    /// numa rota de `POST` e 405 com `Allow: POST`.
    #[test]
    fn post_sem_tipo_ou_sem_tamanho_e_get_em_rota_de_post() {
        let r = decidir(
            &pedido("POST", "/api/listar", &[("Content-Length", "0")]),
            &ctx(),
        );
        assert_eq!((r.codigo, erro_de(&r)), (415, "TIPO_DE_CONTEUDO".into()));
        let r = decidir(
            &pedido(
                "POST",
                "/api/listar",
                &[("Content-Type", "text/plain"), ("Content-Length", "0")],
            ),
            &ctx(),
        );
        assert_eq!(r.codigo, 415);
        let r = decidir(
            &pedido(
                "POST",
                "/api/listar",
                &[("Content-Type", "application/octet-stream")],
            ),
            &ctx(),
        );
        assert_eq!((r.codigo, erro_de(&r)), (411, "TAMANHO_AUSENTE".into()));
        // Com tipo e tamanho, o envelope vazio e pedido torto -- passou da
        // triagem.
        let r = decidir(
            &pedido(
                "POST",
                "/api/listar",
                &[
                    ("Content-Type", "Application/Octet-Stream; x=1"),
                    ("Content-Length", "0"),
                ],
            ),
            &ctx(),
        );
        assert_eq!((r.codigo, erro_de(&r)), (400, "PEDIDO_MALFORMADO".into()));
        let r = decidir(&pedido("GET", "/api/extrair", &[]), &ctx());
        assert_eq!((r.codigo, r.extras.as_str()), (405, "Allow: POST\r\n"));
        let r = decidir(&pedido("POST", "/api/estado", &[]), &ctx());
        assert_eq!((r.codigo, r.extras.as_str()), (405, "Allow: GET\r\n"));
    }

    /// O estado declara o que o motor confere -- nenhum numero digitado.
    #[test]
    fn o_estado_sai_das_constantes() {
        let r = decidir(&pedido("GET", "/api/estado", &[]), &ctx());
        assert_eq!(r.codigo, 200);
        let j = Json::analisar(std::str::from_utf8(&r.corpo).unwrap()).unwrap();
        let l = j.campo("limites").unwrap();
        let m = phxzip::Limites::default();
        assert_eq!(
            l.campo("entrada").and_then(Json::inteiro),
            Some(m.entrada as i64)
        );
        assert_eq!(
            l.campo("cabecalho").and_then(Json::inteiro),
            Some(m.cabecalho as i64)
        );
        assert_eq!(
            l.campo("envio").and_then(Json::inteiro),
            Some(ENVIO_MAX as i64)
        );
        assert_eq!(
            l.campo("simultaneas").and_then(Json::inteiro),
            Some(ops::SIMULTANEAS as i64)
        );
        assert_eq!(
            j.campo("idiomas").and_then(Json::lista).map(|v| v.len()),
            Some(phxsql_core::idiomas::QUANTOS)
        );
        assert_eq!(j.campo("extrair_na_pasta"), Some(&Json::Bool(false)));
        let mut p = pedido("GET", "/api/idiomas", &[]);
        p.consulta = "idioma=Ingles".into();
        let r = decidir(&p, &ctx());
        let j = Json::analisar(std::str::from_utf8(&r.corpo).unwrap()).unwrap();
        assert_eq!(j.campo("idioma").and_then(Json::texto), Some("Ingles"));
    }

    /// A politica do contrato (§7) em TODA resposta, inclusive nas de erro --
    /// cabecalho de seguranca e o tipo de coisa que some numa refatoracao sem
    /// ninguem notar.
    #[test]
    fn toda_resposta_leva_a_politica() {
        let respostas = [
            decidir(&pedido("GET", "/", &[]), &ctx()),
            decidir(&pedido("GET", "/x", &[]), &ctx()),
            decidir(&pedido("GET", "/", &[("Host", "evil.com")]), &ctx()),
            resposta_do_excesso(&Excesso::Corpo {
                declarado: 9,
                teto: 1,
            }),
        ];
        for r in respostas {
            let c = r.cabeca();
            for linha in [
                format!("Content-Security-Policy: {CSP}\r\n"),
                "X-Content-Type-Options: nosniff\r\n".into(),
                "Referrer-Policy: no-referrer\r\n".into(),
                "Cache-Control: no-store\r\n".into(),
                format!("Content-Length: {}\r\n", r.corpo.len()),
                "Connection: close\r\n\r\n".into(),
            ] {
                assert!(c.contains(&linha), "{} sem {linha:?}", r.codigo);
            }
            assert!(!c.contains("unsafe-inline"));
        }
    }

    #[test]
    fn o_413_diz_o_teto_e_o_declarado() {
        let r = resposta_do_excesso(&Excesso::Corpo {
            declarado: 300,
            teto: 200,
        });
        assert_eq!(r.codigo, 413);
        assert_eq!(
            std::str::from_utf8(&r.corpo).unwrap(),
            r#"{"ok":false,"erro":"GRANDE_DEMAIS","detalhe":{"oque":"envio","declarado":300,"teto":200}}"#
        );
        assert!(r.cabeca().starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
    }

    #[test]
    fn o_endereco_e_sempre_127_0_0_1() {
        for porta in [0, 1, PORTA_PADRAO, u16::MAX] {
            let e = endereco_de_escuta(porta);
            assert_eq!(e.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
            assert_eq!(e.port(), porta);
        }
    }

    #[test]
    fn argumentos() {
        let a = |v: &[&str]| ler_argumentos(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(a(&[]), Ok(Acao::Servir(Config::default())));
        match a(&["--pasta", "/tmp/x", "--envio=1000"]) {
            Ok(Acao::Servir(c)) => {
                assert_eq!(c.pasta.as_deref(), Some(Path::new("/tmp/x")));
                assert_eq!(c.envio, 1000);
            }
            outro => panic!("{outro:?}"),
        }
        let acima = format!("--envio={}", ENVIO_MAX + 1);
        for v in [
            &["--envio", "0"][..],
            &[acima.as_str()],
            &["--pasta"],
            &["--pasta="],
        ] {
            assert!(a(v).is_err(), "{v:?} devia ser recusado");
        }
        assert_eq!(Config::default().porta, 7700);
        for v in [&["--porta", "8123"][..], &["--porta=8123"]] {
            match a(v) {
                Ok(Acao::Servir(c)) => assert_eq!(c.porta, 8123),
                outro => panic!("{v:?}: {outro:?}"),
            }
        }
        assert_eq!(a(&["-V"]), Ok(Acao::Versao));
        assert_eq!(a(&["--ajuda"]), Ok(Acao::Ajuda));
        // Endereco nao se escolhe, nem disfarcado de porta.
        for v in [
            &["--porta", "0.0.0.0:7700"][..],
            &["--porta", "0"],
            &["--porta", "70000"],
            &["--porta"],
            &["--endereco", "0.0.0.0"],
            &["--host=0.0.0.0"],
            &["--bind", "::"],
            &["0.0.0.0"],
        ] {
            assert!(a(v).is_err(), "{v:?} devia ser recusado");
        }
    }
}

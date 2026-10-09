//! PhxZipWeb -- a porta web do PhxZip (pedido 454, fatia Z5).
//!
//! O contrato e o `docs/PHXZIP-WEB.md`; onde este arquivo e o contrato
//! discordarem, o contrato manda. Esta fatia entrega o servidor e os
//! estaticos; as rotas `/api/*` sao das fatias Z6 a Z8 e, ate la, respondem
//! `404 ROTA_INEXISTENTE` -- que e verdade, e nao uma lista vazia fingindo
//! que a rota existe.
//!
//! # As tres decisoes que seguram a porta
//!
//! * **So `127.0.0.1`.** A porta vai extrair arquivos e aceitar senha. Nao ha
//!   opcao de endereco, nem na linha de comando nem na [`Config`]: o endereco
//!   sai de [`endereco_de_escuta`], que so recebe a porta. Opcao que nao
//!   existe nao se liga por engano.
//! * **`Host` conferido em todo pedido.** Sem isso, um site de fora religa o
//!   proprio nome para `127.0.0.1` (DNS rebinding) e o navegador da pessoa
//!   conversa com esta porta achando que fala com o site.
//! * **Estaticos por lista fechada, embutidos.** Nao existe «servir a pasta
//!   `ui/`», entao nao existe `../` para pedir: o caminho e comparado por
//!   igualdade com quatro nomes, e o resto e 404.
//!
//! # O HTTP vem do core
//!
//! A leitura e o [`phxsql_core::http::ler_pedido`] e a montagem da cabeca e
//! o [`phxsql_core::http::montar_cabeca`] -- o mesmo motor do servidor do
//! PhxSql (petrea «funcao e comando vem do mesmo motor»). Daqui sai so o que
//! e desta porta: os tetos, a politica de seguranca e as rotas.

use std::borrow::Cow;
use std::io::Write;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

use phxsql_core::http::{self, Excesso, Pedido, PedidoLido, Tetos};
use phxsql_core::json::Json;
use phxsql_core::semaforo::Semaforo;

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
/// contrato -- aquele conta os `POST` que descomprimem, e entra com eles (Z6).
/// Este e o teto de threads da porta: sem ele, cada conexao aberta e uma
/// thread, e quem abre mil sem mandar nada prende mil threads por
/// [`PRAZO_DE_LEITURA`].
pub const CONEXOES_MAX: usize = 32;

/// Quanto se espera por um pedido que comecou e nao terminou.
const PRAZO_DE_LEITURA: Duration = Duration::from_secs(10);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Zero pede ao sistema uma porta livre (os testes usam isso).
    pub porta: u16,
    /// O teto do corpo. Os testes o baixam para provar o 413 sem mandar
    /// 256 MiB; o binario usa sempre o [`ENVIO_MAX`].
    pub envio: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            porta: PORTA_PADRAO,
            envio: ENVIO_MAX,
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
uso: phxzipweb [--porta N]\n\
\n\
  --porta N     a porta em 127.0.0.1 (padrao 7700)\n\
  -V, --version a versao\n\
  -h, --ajuda   este texto\n\
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
        match nome {
            "-V" | "--version" | "--versao" => return Ok(Acao::Versao),
            "-h" | "--help" | "--ajuda" => return Ok(Acao::Ajuda),
            "--porta" | "--port" => {
                let valor = match valor_junto {
                    Some(v) => v,
                    None => {
                        i += 1;
                        args.get(i)
                            .cloned()
                            .ok_or("--porta pede um numero de 1 a 65535")?
                    }
                };
                cfg.porta = match valor.parse::<u16>() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        return Err(format!(
                            "porta invalida: {valor:?} -- e um numero de 1 a 65535; \
                             o endereco e sempre 127.0.0.1"
                        ))
                    }
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

/// A resposta para um pedido ja lido. Nao toca no soquete, e por isso se
/// testa sem rede; a prova pelo soquete esta em `tests/soquete.rs`.
pub fn decidir(pedido: &Pedido, porta: u16) -> Resposta {
    if !host_valido(pedido, porta) {
        return Resposta::erro(403, "HOST_RECUSADO", None);
    }
    if pedido.metodo != "GET" && !origem_valida(pedido, porta) {
        return Resposta::erro(403, "ORIGEM_RECUSADA", None);
    }
    // Igualdade com a lista fechada, e nada mais: nao se normaliza, nao se
    // decodifica `%2e`, nao se junta a caminho nenhum. `/../Cargo.toml` nao
    // e igual a nenhuma rota, e e 404 pelo mesmo motivo que `/x` e.
    let Some(e) = ESTATICOS.iter().find(|e| e.rota == pedido.caminho) else {
        return Resposta::erro(404, "ROTA_INEXISTENTE", None);
    };
    if pedido.metodo != "GET" {
        let mut r = Resposta::erro(405, "METODO_HTTP", None);
        r.extras = "Allow: GET\r\n".to_string();
        return r;
    }
    Resposta {
        codigo: 200,
        tipo: e.tipo,
        extras: String::new(),
        corpo: Cow::Borrowed(e.bytes),
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

/// Atende UMA conexao: le o pedido pelo motor do core, responde e fecha.
pub fn atender(mut fluxo: TcpStream, porta: u16, envio: usize) {
    let _ = fluxo.set_read_timeout(Some(PRAZO_DE_LEITURA));
    let tetos = Tetos {
        cabecalho: CABECALHO_MAX,
        corpo: envio,
    };
    match http::ler_pedido(&mut fluxo, tetos) {
        PedidoLido::Pedido(p) => {
            // Panico vira 500 sem o texto dele (contrato §6): a mensagem de
            // um panico e do programa, e pode carregar dado do pedido.
            let r = catch_unwind(AssertUnwindSafe(|| decidir(&p, porta)))
                .unwrap_or_else(|_| Resposta::erro(500, "INTERNO", None));
            let _ = r.escrever(&mut fluxo);
        }
        PedidoLido::GrandeDemais(excesso) => {
            // Recusou ANTES de ler o corpo: o resto ainda esta chegando. Sem
            // drenar, o fecho vira RST e o navegador ve «erro de rede» no
            // lugar do 413 (contrato §4). O descarte e o dobro do teto; acima
            // dele fecha, e o cliente ve a conexao cair, que e verdade.
            let _ = resposta_do_excesso(&excesso).escrever(&mut fluxo);
            let _ = fluxo.shutdown(Shutdown::Write);
            http::drenar(&mut fluxo, 2 * envio as u64, PRAZO_DO_DRENO);
        }
        PedidoLido::Nada => {
            // Conexao vazia ou pedido torto. Se o outro lado ainda ouve, ouve
            // o motivo; se ja foi, o erro de escrita nao interessa a ninguem.
            let detalhe = Json::objeto(vec![("campo", Json::texto_de("http"))]);
            let _ = Resposta::erro(400, "PEDIDO_MALFORMADO", Some(detalhe)).escrever(&mut fluxo);
        }
    }
}

/// A porta aberta, antes de comecar a atender.
pub struct Servidor {
    ouvinte: TcpListener,
    porta: u16,
    envio: usize,
    vagas: Semaforo,
}

impl Servidor {
    /// Abre a porta em [`endereco_de_escuta`]. Separado de [`Servidor::servir`]
    /// para quem chama saber o endereco de verdade (porta zero) antes de a
    /// thread ficar presa no laco.
    pub fn escutar(cfg: Config) -> std::io::Result<Servidor> {
        let ouvinte = TcpListener::bind(endereco_de_escuta(cfg.porta))?;
        let porta = ouvinte.local_addr()?.port();
        Ok(Servidor {
            ouvinte,
            porta,
            envio: cfg.envio,
            vagas: Semaforo::novo(CONEXOES_MAX),
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
            let (porta, envio) = (self.porta, self.envio);
            let lancou = std::thread::Builder::new()
                .name("phxzipweb".into())
                .spawn(move || {
                    let _vaga = vaga;
                    atender(fluxo, porta, envio);
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
            let r = decidir(&pedido("GET", rota, &[]), PORTA);
            assert_eq!((r.codigo, r.tipo), (200, tipo), "{rota}");
            assert!(!r.corpo.is_empty(), "{rota}");
        }
        // A fonte e binaria e chega como esta no disco: 40.896 bytes,
        // assinatura `wOF2` (contrato §2).
        let f = decidir(&pedido("GET", "/fonte/exo2-latin.woff2", &[]), PORTA);
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
            "/api/estado",
            "",
        ] {
            let r = decidir(&pedido("GET", rota, &[]), PORTA);
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
            let r = decidir(&pedido("GET", "/", &[("Host", host)]), PORTA);
            assert_eq!(r.codigo, 403, "{host}");
            assert_eq!(erro_de(&r), "HOST_RECUSADO");
        }
        let mut sem_host = pedido("GET", "/", &[]);
        sem_host.cabecalhos.remove("host");
        assert_eq!(decidir(&sem_host, PORTA).codigo, 403);
        for host in ["127.0.0.1:7700", "localhost:7700", "LocalHost:7700"] {
            assert_eq!(
                decidir(&pedido("GET", "/", &[("Host", host)]), PORTA).codigo,
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
            let r = decidir(&pedido("POST", "/", &[(nome, valor)]), PORTA);
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
            PORTA,
        );
        assert_eq!(r.codigo, 405);
        assert_eq!(r.extras, "Allow: GET\r\n");
        assert_eq!(erro_de(&r), "METODO_HTTP");
    }

    /// A politica do contrato (§7) em TODA resposta, inclusive nas de erro --
    /// cabecalho de seguranca e o tipo de coisa que some numa refatoracao sem
    /// ninguem notar.
    #[test]
    fn toda_resposta_leva_a_politica() {
        let respostas = [
            decidir(&pedido("GET", "/", &[]), PORTA),
            decidir(&pedido("GET", "/x", &[]), PORTA),
            decidir(&pedido("GET", "/", &[("Host", "evil.com")]), PORTA),
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

//! `replicacao.replicas_autorizadas` pela porta HTTP com proxy declarado --
//! provado PELO SOQUETE (pedido 284).
//!
//! # O defeito
//!
//! O portao 2a-bis compara o IP da sessao com a lista. Pela porta HTTP esse
//! IP e o par do soquete, e atras de um proxy reverso o par e o PROXY: com
//! `replicas_autorizadas: ["127.0.0.1"]` e `web.atras_de_proxy: true`, todo
//! cliente que chega pelo proxy e `127.0.0.1` para o servidor, e leva o
//! diario inteiro com `{"op":"replicar"}` -- a lista preenchida autorizava
//! qualquer um. O servidor SABE que ha proxy (`atras_de_proxy` e a declaracao
//! de quem implanta), e o portao nao perguntava.
//!
//! # Por que pelo soquete
//!
//! O que esta em jogo e de onde o IP vem, e isso so existe com uma conexao de
//! verdade: um teste de unidade montaria a `Sessao` a mao e escreveria o campo
//! que o conserto le, provando o campo e nao o caminho.
//!
//! # Os dois sentidos
//!
//! - com proxy declarado e lista preenchida, `replicar` pelo `/api` RECUSA, e
//!   a recusa nomeia o proxy (o dano medido e o diario: a resposta nao pode
//!   trazer `eventos`);
//! - sem proxy declarado, a mesma lista com o mesmo IP continua passando -- o
//!   IP ali e o do cliente, e a lista vale;
//! - com proxy e lista VAZIA, nada muda: guarda nova entra pedida.

mod comum;
use comum::DirTemp;

use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";

fn subir(base: &std::path::Path, atras_de_proxy: bool, lista: &str) -> (Arc<Servidor>, u16) {
    let proxy = if atras_de_proxy {
        r#", "atras_de_proxy": true"#
    } else {
        ""
    };
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "cifra_fio": {{ "exigir": false }},
              "replicacao": {{ "papel": "source", "id_servidor": "proxy-01",
                               "replicas_autorizadas": [{lista}] }},
              "web":  {{ "ligado": true, "bind": "127.0.0.1:0"{proxy} }}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_web());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_err() {
        assert!(Instant::now() < ate, "a porta web nao abriu em 5 s");
        std::thread::sleep(Duration::from_millis(20));
    }
    (s, porta)
}

/// `POST /api` cru, e a resposta como (codigo, corpo).
fn api(porta: u16, corpo: &str) -> (u16, String) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let _ = write!(
        escrita,
        "POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let _ = escrita.flush();
    let mut resposta = String::new();
    let _ = BufReader::new(fluxo).read_to_string(&mut resposta);
    let codigo = resposta
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem codigo: {resposta:?}"));
    let corpo = resposta
        .split_once("\r\n\r\n")
        .map(|(_, c)| c.to_string())
        .unwrap_or_default();
    (codigo, corpo)
}

/// O valor que so o diario leva: e por ele que o dano se mede, e nao pelo
/// codigo da resposta.
const MARCA: &str = "linha-que-so-o-diario-leva";

/// A imagem da linha viaja em hexadecimal: e assim que a MARCA aparece no
/// diario entregue.
fn marca_no_diario() -> String {
    MARCA.bytes().map(|b| format!("{b:02x}")).collect()
}

/// Uma tabela com uma linha, criada pela propria porta web -- o diario do
/// source passa a ter o que entregar.
fn encher(porta: u16) {
    for corpo in [
        format!(r#"{{"token":"{TOKEN}","op":"criar_database","database":"loja"}}"#),
        format!(
            r#"{{"token":"{TOKEN}","op":"criar_tabela","database":"loja","tabela":"c","colunas":[{{"nome":"nome","tipo":"Str(40)"}}]}}"#
        ),
        format!(
            r#"{{"token":"{TOKEN}","op":"inserir","database":"loja","tabela":"c","valores":{{"nome":"{MARCA}"}}}}"#
        ),
    ] {
        let (_, r) = api(porta, &corpo);
        assert!(r.contains("\"ok\":true"), "{corpo} -> {r}");
    }
}

fn pedir(porta: u16, op: &str) -> String {
    api(
        porta,
        &format!(r#"{{"op":"{op}","token":"{TOKEN}","database":"loja","tabela":"c","desde":0}}"#),
    )
    .1
}

/// **Prova real do 284.** Tirando a pergunta pelo proxy do portao 2a-bis, a
/// primeira asercao cai: o `replicar` volta com a linha.
#[test]
fn replicar_pela_porta_web_atras_de_proxy_nao_passa_pela_lista() {
    let d = DirTemp::novo("rep-proxy");
    let (_s, porta) = subir(&d, true, r#""127.0.0.1""#);
    encher(porta);
    for op in ["replicar", "posicao"] {
        let corpo = pedir(porta, op);
        assert!(
            !corpo.contains("\"ok\":true") && !corpo.contains(&marca_no_diario()),
            "{op} atras de proxy com a lista preenchida tinha de recusar: {corpo}"
        );
        assert!(
            corpo.contains("atras_de_proxy"),
            "{op}: a recusa tem de nomear o proxy, e nao mandar procurar o IP: {corpo}"
        );
    }
}

/// O irmao do comportamento velho: sem proxy declarado, o IP da conexao e o
/// do cliente, e a lista continua autorizando quem esta nela.
#[test]
fn sem_proxy_a_lista_continua_autorizando_pelo_ip() {
    let d = DirTemp::novo("rep-sem-proxy");
    let (_s, porta) = subir(&d, false, r#""127.0.0.1""#);
    encher(porta);
    let corpo = pedir(porta, "replicar");
    assert!(corpo.contains(&marca_no_diario()), "{corpo}");
}

/// Guarda nova entra pedida: com proxy e lista VAZIA, nada muda.
#[test]
fn atras_de_proxy_sem_lista_nada_muda() {
    let d = DirTemp::novo("rep-proxy-vazia");
    let (_s, porta) = subir(&d, true, "");
    encher(porta);
    let corpo = pedir(porta, "replicar");
    assert!(corpo.contains(&marca_no_diario()), "{corpo}");
}

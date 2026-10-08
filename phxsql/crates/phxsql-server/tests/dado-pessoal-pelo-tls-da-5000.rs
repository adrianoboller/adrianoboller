//! O dado pessoal pelo TLS REAL da porta de dados (pedido 674, resto do
//! 667) -- pelo soquete, com o `openssl s_client` conferindo o certificado.
//!
//! O `fio_cifrado` so passou a enxergar o TLS da porta de dados (o campo
//! `fio_tls` da `Sessao`) no 667, e nenhum teste entregava dado pessoal por
//! ele. O que esta prova segura, na MESMA porta e no MESMO servidor:
//!
//! 1. `replicar` de tabela com coluna marcada, pedido por TLS 1.3, devolve
//!    os eventos COM o valor -- o TLS conta como fio cifrado;
//! 2. o mesmo pedido em claro (sem TLS e sem tunel) e recusado com
//!    `erro.replicar_marcada_exige_cifra`, e o valor nao cruza o fio.
//!
//! A cifra exigida fica DESLIGADA (`cifra_fio.exigir: false`) de proposito:
//! com ela ligada o claro nem chegaria ao `replicar`, e a prova (2) mediria
//! o portao errado.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use phxsql_core::{Column, ColumnType, DadoPessoal, IndexColumn, IndexDef, Schema, Value};
use phxsql_server::{Config, Servidor};
use phxsql_store::Table;

const TOKEN: &str = "o token de servico deste teste";
/// O valor marcado: e ele que tem de chegar pelo TLS e NAO pelo claro.
const VALOR: &str = "fulano-674-cpf";

fn encher(base: &Path) {
    let dir = base.join("loja");
    std::fs::create_dir_all(&dir).unwrap();
    let nome = Column::new("nome", ColumnType::Str(40))
        .obrigatoria()
        .com_dado_pessoal(DadoPessoal::Pessoal);
    let esquema = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int8).obrigatoria(), nome],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = Table::criar(dir, esquema)
        .unwrap()
        .com_imagem_no_diario(true);
    for i in 1..=3 {
        t.inserir(&[Value::Int(i), Value::Str(format!("{VALOR}-{i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
}

fn subir(base: &Path) -> (Arc<Servidor>, u16) {
    let dados = base.join("base");
    encher(&dados);
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "{TOKEN}",
              "log_acessos": "{}", "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}", "jobs": "{}",
              "tls": true,
              "cifra_fio": {{ "exigir": false }},
              "replicacao": {{ "papel": "source", "id_servidor": "src-674",
                               "imagem_da_linha": true }}
            }}"#,
            bar(dados),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert!(!c.cifra_fio.exigir);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

/// Uma linha de pedido pelo `openssl s_client` (TLS 1.3, certificado
/// conferido), e a primeira linha JSON que voltar. O mesmo ajudante do
/// `tls-da-porta-de-dados.rs`.
fn pedido_tls(porta: u16, ca: &Path, linha: &str) -> String {
    let mut filho = Command::new("openssl")
        .args(["s_client", "-connect", &format!("127.0.0.1:{porta}")])
        .args(["-tls1_3", "-quiet", "-verify_return_error", "-CAfile"])
        .arg(ca)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut entrada = filho.stdin.take().unwrap();
    entrada.write_all(format!("{linha}\n").as_bytes()).unwrap();
    let saida = filho.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines().map_while(Result::ok) {
            if l.starts_with('{') {
                let _ = tx.send(l);
                return;
            }
        }
    });
    let resposta = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_default();
    let _ = filho.kill();
    let _ = filho.wait();
    drop(entrada);
    resposta
}

fn pedido_em_claro(porta: u16, linha: &str) -> String {
    let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    writeln!(c, "{linha}").unwrap();
    let mut r = String::new();
    BufReader::new(&c).read_line(&mut r).unwrap();
    r
}

fn replicar() -> String {
    format!(
        r#"{{"token":"{TOKEN}","op":"replicar","database":"loja","tabela":"clientes","desde":0}}"#
    )
}

/// O valor aparece no fio? Procura o texto e as duas grafias em que a
/// imagem pode viajar (hexadecimal e base64), para a ausencia no claro nao
/// passar por engano so porque o valor mudou de roupa.
fn leva_o_valor(resposta: &str) -> bool {
    let cru = format!("{VALOR}-1");
    let hex = phxsql_core::hash::para_hex(cru.as_bytes());
    let b64 = phxsql_core::base64::codificar(cru.as_bytes());
    // O base64 de um trecho depende do alinhamento: confere os tres.
    let b64_trechos: Vec<String> = (0..3)
        .map(|k| {
            let deslocado = format!("{}{cru}", "x".repeat(k));
            let t = phxsql_core::base64::codificar(deslocado.as_bytes());
            t[4..t.len().saturating_sub(4)].to_string()
        })
        .collect();
    resposta.contains(&cru)
        || resposta.contains(&hex)
        || resposta.contains(&hex.to_uppercase())
        || resposta.contains(&b64)
        || b64_trechos
            .iter()
            .any(|t| t.len() > 8 && resposta.contains(t.as_str()))
}

#[test]
fn dado_pessoal_passa_pelo_tls_real_e_se_recusa_em_claro_na_mesma_porta() {
    let d = DirTemp::novo("dado-pessoal-tls-5000");
    let (_s, porta) = subir(&d.0);
    let ca = d.0.join("tls-dados-certificado.pem");
    assert!(ca.exists(), "o autoassinado da porta de dados nao nasceu");

    // (1) pelo TLS: os tres eventos, com o valor marcado dentro.
    let r = pedido_tls(porta, &ca, &replicar());
    assert!(r.contains(r#""ok":true"#), "o TLS foi recusado: {r:?}");
    let j = phxsql_core::json::Json::analisar(&r).unwrap();
    let eventos = j
        .campo("resultado")
        .and_then(|x| x.campo("eventos"))
        .and_then(phxsql_core::json::Json::lista)
        .map_or(0, |l| l.len());
    assert_eq!(eventos, 3, "{r}");
    assert!(leva_o_valor(&r), "o evento chegou sem o valor: {r}");

    // (2) na MESMA porta, em claro: recusado nomeando a cifra, sem o valor.
    let r = pedido_em_claro(porta, &replicar());
    assert!(r.contains(r#""ok":false"#), "o claro entregou: {r}");
    assert!(r.contains("clientes"), "a recusa nao nomeia a tabela: {r}");
    assert!(!leva_o_valor(&r), "o valor cruzou o fio em claro: {r}");
}

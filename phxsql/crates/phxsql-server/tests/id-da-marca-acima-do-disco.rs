//! Pedido 714 (F4): o id da marca que o servidor grava nunca sai abaixo de
//! uma marca que ja esta no disco -- provado contra o processo de verdade.
//!
//! # O defeito
//!
//! O contador do servidor (`Transacoes`) nascia do relogio e so. O embutido ja
//! usava `max(relogio, maior marca + 1)`. Com o relogio recuado entre dois
//! arranques e uma marca RETIDA da vida anterior (cifrada sem a chave, linha
//! perdida), o `COMMIT` novo nascia com id MENOR -- e o arranque seguinte o
//! completaria ANTES da antiga, contra a ordem de criacao -- ou IGUAL, e o
//! `create_new` recusava o `COMMIT`.
//!
//! # Como o relogio «recua»
//!
//! Sem mexer no relogio da maquina: a marca retida nasce com um id do FUTURO,
//! que e exatamente o que a vida anterior deixa quando o relogio volta. Ela e
//! cifrada e este servidor nao tem a chave, entao a recuperacao a deixa no
//! disco (`Leitura::SemChave`) e o servidor sobe.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use phxsql_core::json::Json;

const TOKEN: &str = "id-da-marca";

fn subir(dir: &Path, vez: u32) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let r = Json::analisar(&r).unwrap();
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
        r
    }
}

/// Uma marca v4 CIFRADA, sem operacao nenhuma, que este servidor nao abre: o
/// cabecalho declara o material com a flag de cifra, e o cofre do processo
/// esta vazio. E o que a recuperacao RETEM (`SemChave`).
fn marca_retida(dir: &Path, id: u64) {
    let mut b = Vec::new();
    b.extend_from_slice(b"PHXTX\0\0\0");
    b.extend_from_slice(&4u32.to_le_bytes());
    b.extend_from_slice(&id.to_le_bytes());
    b.extend_from_slice(&0i64.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    let mut material = [0u8; 40];
    material[0] = 1;
    material[4..8].copy_from_slice(&10_000u32.to_le_bytes());
    b.extend_from_slice(&material);
    let crc = phxsql_core::crc::crc32(&b);
    b.extend_from_slice(&crc.to_le_bytes());
    std::fs::write(dir.join(format!("transacao_{id}.tx")), b).unwrap();
}

fn commit(porta: u16) -> u64 {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"t","linha":{"id":1}"#);
    let r = b.exigir(r#""op":"commit""#);
    r.campo("resultado")
        .and_then(|x| x.campo("transaction_id"))
        .and_then(Json::inteiro)
        .expect("o COMMIT nao disse o id") as u64
}

/// **A prova do 714.** Vermelho medido antes do conserto: o `COMMIT` da
/// segunda vida sai com id MENOR que o da marca retida.
#[test]
fn o_commit_nasce_acima_da_marca_retida_no_disco() {
    let base = DirTemp::novo("id-da-marca");
    let (filho, porta) = subir(&base.0, 1);
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    b.exigir(
        r#""op":"criar_tabela","database":"loja","tabela":"t",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    );
    let primeiro = commit(porta);
    drop(b);
    drop(filho);

    // A vida anterior deixou uma marca de ~3 horas no futuro deste relogio.
    let retida = primeiro + 10_000_000;
    marca_retida(&base.0.join("dados").join("loja"), retida);

    let (filho, porta) = subir(&base.0, 2);
    let erro = std::fs::read_to_string(base.0.join("stderr-2.txt")).unwrap_or_default();
    assert!(
        base.0
            .join("dados")
            .join("loja")
            .join(format!("transacao_{retida}.tx"))
            .exists(),
        "premissa: a recuperacao tinha de RETER a marca sem chave ({erro})"
    );
    let novo = commit(porta);
    drop(filho);
    assert!(
        novo > retida,
        "o COMMIT da vida nova saiu com o id {novo}, abaixo da marca {retida} que esta \
         no disco: o arranque seguinte o completaria antes dela"
    );
}

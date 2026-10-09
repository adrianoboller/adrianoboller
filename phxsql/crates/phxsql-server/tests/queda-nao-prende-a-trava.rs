//! Pedido 758: o `phxsqld` morto por `SIGKILL` nao pode deixar a
//! `.phxsql.trava` presa num filho dele -- provado contra o sistema
//! operacional, com o `phxsqld` de verdade.
//!
//! # O defeito
//!
//! A trava e `flock`, e o `flock` e da DESCRICAO aberta: todo `spawn` copia o
//! descritor para o filho, que o segura ate o `exec`. O servidor que morre
//! nesse instante deixa o filho orfao com a trava na mao, e a abertura
//! gravavel seguinte ouve `InstanciaOcupada` apontando o pid do morto. No
//! servidor quem cria filho sozinho e a thread `vigia-disco`, que chama o `df`
//! logo no arranque: medido pelo SO, 37 de 300 quedas deixaram a trava presa,
//! e o detentor era um processo `vigia-disco` com `ppid` 1.
//!
//! # Como se prova
//!
//! O teste mata o servidor logo depois de uma gravacao (a trava tomada e
//! fixada), espera o processo, e abre a tabela para gravar pelo motor, como
//! o `commit-inteiro-na-queda` fazia quando caiu. Repetido [`VEZES`] vezes.
#![cfg(unix)]

mod comum;
use comum::{porta_no_texto, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "queda-nao-prende-a-trava";
/// Quedas por corrida. Com o `df` de volta ao `Command` direto, 6 corridas
/// de 100 quedas deram 6, 4, 6, 11, 7 e 13 recusas (vermelho nas seis); com
/// 40 quedas, uma corrida em tres passava sem ver o defeito.
const VEZES: u64 = 100;

/// O `phxsqld` com o erro padrao num cano: cada linha chega pelo canal assim
/// que ele a escreve.
fn subir(dir: &Path) -> (Filho, Receiver<String>, u16) {
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
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let erro = filho.0.stderr.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for linha in BufReader::new(erro).lines() {
            let Ok(linha) = linha else { break };
            if tx.send(linha).is_err() {
                break;
            }
        }
    });
    let porta = loop {
        let linha = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("o phxsqld nao abriu a porta em 20 s");
        if let Some(p) = porta_no_texto(&format!("{linha}\n")) {
            break p.unwrap_or_else(|e| panic!("{e}"));
        }
    };
    (filho, rx, porta)
}

/// Manda as linhas numa conexao so; `None` quando ela caiu sem resposta. A
/// resposta que nao e `ok` so e aceita na `inserir`: depois de um `SIGKILL`
/// o indice pode pedir reparo, e o que se mede aqui e a trava, que a
/// abertura gravavel ja tomou.
fn pedir(porta: u16, linhas: &[String]) -> Option<()> {
    let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    for l in linhas {
        writeln!(escrita, "{{\"token\":\"{TOKEN}\",{l}}}").ok()?;
        let mut r = String::new();
        match leitor.read_line(&mut r) {
            Ok(n) if n > 0 => {
                assert!(
                    r.contains("\"ok\":true") || l.contains("\"inserir\""),
                    "{l} -> {r}"
                );
            }
            _ => return None,
        }
    }
    Some(())
}

fn preparar(base: &Path) {
    let (filho, _rx, porta) = subir(base);
    pedir(
        porta,
        &[
            r#""op":"criar_database","database":"loja""#.to_string(),
            r#""op":"criar_tabela","database":"loja","tabela":"vendas",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        ],
    )
    .expect("a criacao nao respondeu");
    drop(filho);
}

/// Mata, espera, e abre `vendas` para gravar pelo motor. `Some` com a recusa.
fn matar_e_abrir(mut filho: Filho, dados: &Path) -> Option<String> {
    let _ = filho.0.kill();
    let _ = filho.0.wait();
    drop(filho);
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    db.abrir_qualificada("vendas").err().map(|e| e.to_string())
}

fn exigir_nenhuma(recusas: Vec<String>) {
    assert!(
        recusas.is_empty(),
        "{} de {VEZES} quedas deixaram a trava presa num processo que herdou o \
         descritor do morto:\n{}",
        recusas.len(),
        recusas.join("\n")
    );
}

/// O servidor comum, sem gancho de teste nenhum, morto logo depois de gravar
/// -- quando a `vigia-disco` pode estar chamando o `df`.
#[test]
fn o_sigkill_no_arranque_nao_deixa_a_trava_no_filho_do_df() {
    let base = DirTemp::novo("queda-vigia");
    preparar(&base.0);
    let dados = base.0.join("dados");
    let mut recusas = Vec::new();
    for vez in 1..=VEZES {
        let (filho, _rx, porta) = subir(&base.0);
        pedir(
            porta,
            &[format!(
                r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{vez}}}"#
            )],
        )
        .expect("a insercao nao respondeu");
        if let Some(e) = matar_e_abrir(filho, &dados) {
            recusas.push(format!("queda {vez}: {e}"));
        }
    }
    exigir_nenhuma(recusas);
}

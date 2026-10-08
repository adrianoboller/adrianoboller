//! Pedido 687 -- a parada pedida (`SIGTERM`, `SIGINT`) leva a janela de
//! durabilidade ao disco antes de sair.
//!
//! # Por que um processo de verdade
//!
//! O que se prova e o que o NUCLEO faz com o sinal: sem tratador, ele mata o
//! processo pelo padrao, e nenhum teste unitario chega la. E o `phxsqld`
//! compilado, gravando, recebendo `kill -TERM`, e o `.ndx` que sobra na
//! pasta lido por outro processo -- este --, que nao tem o atestado do
//! servidor e por isso le o byte 52 como a CLI le.
//!
//! Medido em 08/10/2026 com o tratamento tirado: `sistema` e `por_lote`
//! morrem pelo sinal 15 e o `verificar` recusa com «ficou para tras numa
//! queda»; `por_operacao`, que sincroniza a cada escrita, sai INTEGRA e e o
//! CONTROLE de que o instrumento nao esta cego para o caso bom.

#![cfg(unix)]

mod comum;
use comum::{DirTemp, Filho};

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use phxsql_store::table::{SemEscrever, Table};

/// Sobe o `phxsqld` no regime pedido, grava 30 linhas numa tabela com
/// indice, manda o sinal e devolve como o processo saiu.
fn gravar_e_sinalizar(dir: &Path, regime: &str, sinal: &str) -> ExitStatus {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "t",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": false }},
                "recursos": {{
                    "durabilidade": "{regime}",
                    "lote_milissegundos": 3600000,
                    "lote_operacoes": 1000000
                }}
            }}"#,
            base = dir.join("dados").display().to_string()
        ),
    )
    .unwrap();
    let erro_padrao = dir.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = comum::porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    let mut linhas = vec![
        r#"{"token":"t","op":"criar_database","database":"loja"}"#.to_string(),
        r#"{"token":"t","op":"criar_tabela","database":"loja","tabela":"c","colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},{"nome":"v","tipo":"Int8"}],"indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]}"#.to_string(),
    ];
    for i in 1..=30 {
        linhas.push(format!(
            r#"{{"token":"t","op":"inserir","database":"loja","tabela":"c","linha":{{"id":{i},"v":{i}}}}}"#
        ));
    }
    for l in &linhas {
        let r = comum::pedir(porta, l);
        assert!(r.contains("\"ok\":true"), "{l} recusou: {r}");
    }
    let pid = filho.0.id().to_string();
    let st = Command::new("kill")
        .args([sinal, &pid])
        .status()
        .expect("kill");
    assert!(st.success(), "o kill {sinal} falhou");
    let ate = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(s) = filho.0.try_wait().unwrap() {
            return s;
        }
        assert!(
            Instant::now() < ate,
            "o phxsqld nao saiu em 20 s com {sinal}: {}",
            std::fs::read_to_string(&erro_padrao).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// O que a CLI faz no `phxsql verificar`, por dentro: abre para ler, num
/// processo que nao escreveu, e confere tudo.
fn verificar(dir: &Path) -> Result<u64, String> {
    let pasta = dir.join("dados").join("loja");
    let mut t = match Table::abrir_para_ler(&pasta, "c").map_err(|e| e.to_string())? {
        SemEscrever::Aberta(t) => t,
        SemEscrever::PrecisaEscrever(_) => Table::abrir(&pasta, "c").map_err(|e| e.to_string())?,
    };
    t.verificar()
        .map(|r| r.registros)
        .map_err(|e| e.to_string())
}

fn parada_limpa(regime: &str, sinal: &str) {
    let dir = DirTemp::novo(&format!("parada-{regime}"));
    let st = gravar_e_sinalizar(&dir, regime, sinal);
    let texto = std::fs::read_to_string(dir.join("stderr.txt")).unwrap_or_default();
    assert_eq!(
        st.code(),
        Some(0),
        "{regime}/{sinal}: o phxsqld nao saiu pela parada em ordem ({st}): {texto}"
    );
    assert_eq!(
        verificar(&dir),
        Ok(30),
        "{regime}/{sinal}: o indice ficou marcado depois da parada pedida"
    );
}

/// **A prova do 687.** `sistema` e o regime que nunca sincroniza sozinho: a
/// janela inteira so vai ao disco se a parada a levar.
#[test]
fn sigterm_em_sistema_sai_integra_sem_reindex() {
    parada_limpa("sistema", "-TERM");
}

/// `por_lote` com a janela de uma hora: o relogio nao fecharia a tempo.
#[test]
fn sigterm_em_por_lote_sai_integra_sem_reindex() {
    parada_limpa("por_lote", "-TERM");
}

/// O Ctrl-C do terminal e o mesmo caminho.
#[test]
fn sigint_sai_integra_sem_reindex() {
    parada_limpa("sistema", "-INT");
}

/// O CONTROLE: o `SIGKILL` nao se trata, e o mesmo instrumento ACUSA o
/// indice marcado. Sem ele, um `verificar` que nunca recusasse passaria nos
/// tres de cima por engano.
#[test]
fn sigkill_continua_deixando_o_indice_marcado() {
    let dir = DirTemp::novo("parada-kill");
    let st = gravar_e_sinalizar(&dir, "sistema", "-KILL");
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(st.signal(), Some(9), "nao caiu pelo SIGKILL: {st:?}");
    let r = verificar(&dir);
    assert!(
        r.as_ref()
            .is_err_and(|e| e.contains("ficou para tras numa queda")),
        "o instrumento nao acusou a queda: {r:?}"
    );
}

/// O IRMAO: a ponte MCP escreve pelo mesmo `Servidor`, sem o relogio de
/// gravacao (ele sobe so no `escutar`). O fim da entrada -- o jeito normal de
/// um cliente MCP terminar -- tem de passar pelo mesmo fecho da janela.
#[test]
fn a_ponte_mcp_fecha_a_janela_no_fim_da_entrada() {
    use std::io::{Read, Write};
    let dir = DirTemp::novo("parada-mcp");
    // A tabela nasce pela porta de dados, e a parada pelo sinal ja a deixa
    // integra -- e o que o teste de cima prova.
    let st = gravar_e_sinalizar(&dir, "sistema", "-TERM");
    assert_eq!(st.code(), Some(0), "a primeira parada nao foi limpa: {st}");
    let mut filho = Command::new(env!("CARGO_BIN_EXE_phxsqld"))
        .arg("--config")
        .arg(dir.join("config.json"))
        .args(["--mcp", "--escrita"])
        .current_dir(&*dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("nao consegui subir o phxsqld --mcp");
    let mut entrada = filho.stdin.take().unwrap();
    for i in 31..=35 {
        writeln!(
            entrada,
            r#"{{"jsonrpc":"2.0","id":{i},"method":"tools/call","params":{{"name":"phx_inserir","arguments":{{"database":"loja","tabela":"c","valores":{{"id":{i},"v":{i}}}}}}}}}"#
        )
        .unwrap();
    }
    drop(entrada);
    let mut saida = String::new();
    filho
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut saida)
        .unwrap();
    let st = filho.wait().unwrap();
    assert!(
        !saida.contains("\"error\""),
        "a ponte recusou uma insercao: {saida}"
    );
    assert_eq!(st.code(), Some(0), "a ponte nao saiu limpa: {st}");
    assert_eq!(
        verificar(&dir),
        Ok(35),
        "o indice ficou marcado depois de a ponte fechar"
    );
}

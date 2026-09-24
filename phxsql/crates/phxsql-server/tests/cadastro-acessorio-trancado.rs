//! Pedido 466 -- cadastro do DbLink ilegivel TRANCA o DbLink, e o motor sobe.
//! E o irmao: o `jobs.json`, aberto logo depois pelo mesmo `?`.
//!
//! # Por que o `phxsqld` de verdade
//!
//! O defeito era o PROCESSO nao subir: o `?` do `Servidor::novo` levava o
//! erro do `dblink.json` ate o `main`, que saia com falha antes de abrir a
//! porta. Isso so se ve de fora -- a porta que nunca abre --, entao a prova
//! sobe o binario compilado, fala com ele pelo soquete e confere o arquivo no
//! disco depois.
//!
//! Os dois sentidos, em cada um dos cinco jeitos de o arquivo nao abrir:
//! o servidor ABRE a porta e as tabelas respondem; e o DbLink RECUSA, dizendo
//! o motivo e nomeando o arquivo -- sem regravar o arquivo por cima.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Sobe o `phxsqld` e devolve a porta de dados -- ou o motivo de o processo
/// ter SAIDO sem abrir, que e o vermelho deste pedido.
fn subir(dir: &Path) -> Result<(Filho, u16), String> {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "t",
                "base": {base:?},
                "dblink": {dbl:?},
                "jobs": {jobs:?},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": false }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            dbl = dir.join("dblink.json").display().to_string(),
            jobs = dir.join("jobs.json").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join("stderr.txt");
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
    const LINHA: &str = "porta de dados escutando em ";
    let ate = Instant::now() + Duration::from_secs(20);
    loop {
        let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
        if let Some(resto) = texto.lines().find_map(|l| l.strip_prefix(LINHA)) {
            let alvo: SocketAddr = resto.trim().parse().unwrap();
            return Ok((filho, alvo.port()));
        }
        if let Some(st) = filho.0.try_wait().unwrap() {
            // Relido DEPOIS da saida: o texto de cima pode ser de antes do
            // ultimo `eprintln!` do processo.
            let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
            return Err(format!(
                "o phxsqld SAIU ({st}) sem abrir a porta -- o motor inteiro fora do \
                 ar por causa de um cadastro acessorio: {}",
                texto.trim()
            ));
        }
        if Instant::now() > ate {
            return Err(format!("o phxsqld nao abriu a porta em 20 s: {texto}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{linha}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

/// Os cinco cadastros que nao abrem, e o que cada um e.
fn ilegiveis() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("json torto", Some(r#"{"ligacoes": [ {"nome": "loja", "#)),
        (
            "formato maior que o deste binario",
            Some(r#"{"formato": 99, "ligacoes": []}"#),
        ),
        (
            "ligacao repetida",
            Some(
                r#"[{"nome":"loja","host":"127.0.0.1","porta":1},
                    {"nome":"LOJA","host":"127.0.0.1","porta":2}]"#,
            ),
        ),
        (
            "material de cifra torto",
            Some(r#"{"formato": 2, "cifra_do_cadastro": {"sal": "nao-e-hex"}, "ligacoes": []}"#),
        ),
        // `None`: um DIRETORIO no lugar do arquivo -- existe e nao se le. Antes
        // ele virava cadastro VAZIO, e o primeiro `dblink_salvar` o regravaria.
        ("arquivo que existe e nao se le", None),
    ]
}

/// **A prova do pedido 466.**
///
/// Com o defeito (medido antes do conserto): o `phxsqld` SAI com falha nos
/// quatro primeiros casos, sem abrir a porta; no quinto, sobe com o cadastro
/// VAZIO e o `dblink_salvar` responde `ok`. Com o conserto: sobe nos cinco, o
/// `inserir` numa tabela responde, e o DbLink recusa nomeando o arquivo.
#[test]
fn cadastro_do_dblink_ilegivel_tranca_o_dblink_e_o_motor_sobe() {
    // Um vermelho por caso, e todos juntos no fim: com o defeito reposto, o
    // relatorio diz o que cada um dos cinco fez, e nao so o primeiro.
    let mut falhas: Vec<String> = Vec::new();
    for (caso, conteudo) in ilegiveis() {
        if let Err(e) = um_caso(conteudo) {
            falhas.push(format!("[{caso}] {e}"));
        }
    }
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
}

fn um_caso(conteudo: Option<&str>) -> Result<(), String> {
    let dir = DirTemp::novo("dblink-trancado");
    let arquivo = dir.join("dblink.json");
    match conteudo {
        Some(texto) => std::fs::write(&arquivo, texto).unwrap(),
        None => std::fs::create_dir(&arquivo).unwrap(),
    }
    let antes = std::fs::read(&arquivo).ok();

    let (_filho, porta) = subir(&dir)?;

    // O motor de pe: uma tabela nasce e grava.
    for linha in [
        r#"{"token":"t","op":"criar_database","database":"loja"}"#,
        r#"{"token":"t","op":"criar_tabela","database":"loja","tabela":"c","colunas":[{"nome":"n","tipo":"Int8"}]}"#,
        r#"{"token":"t","op":"inserir","database":"loja","tabela":"c","valores":{"n":1}}"#,
    ] {
        let r = pedir(porta, linha);
        if !r.contains("\"ok\":true") {
            return Err(format!("o motor recusou {linha}: {r}"));
        }
    }

    // O DbLink recusa -- a lista, o uso e a gravacao --, dizendo por que.
    let nome = arquivo.display().to_string();
    for linha in [
        r#"{"token":"t","op":"dblink"}"#,
        r#"{"token":"t","op":"dblink_testar","dblink":"loja"}"#,
        r#"{"token":"t","op":"dblink_salvar","nome":"nova","host":"127.0.0.1","porta":1}"#,
        r#"{"token":"t","op":"dblink_excluir","nome":"loja"}"#,
    ] {
        let r = pedir(porta, linha);
        if !r.contains("\"ok\":false") {
            return Err(format!("o DbLink trancado ATENDEU {linha}: {r}"));
        }
        if !(r.contains("TRANCADO") && r.contains(&nome)) {
            return Err(format!(
                "a recusa nao diz o motivo nem nomeia o arquivo: {r}"
            ));
        }
    }

    // E o arquivo continua como estava: trancado nao regrava.
    if std::fs::read(&arquivo).ok() != antes || (conteudo.is_none() && !arquivo.is_dir()) {
        return Err("o cadastro trancado foi REGRAVADO".into());
    }

    // O arranque avisou, pela lista de avisos de sempre.
    let erro_padrao = std::fs::read_to_string(dir.join("stderr.txt")).unwrap();
    if !erro_padrao.contains("AVISO: o DbLink esta TRANCADO") {
        return Err(format!("o arranque nao avisou: {erro_padrao}"));
    }
    Ok(())
}

/// O comportamento VELHO que nao pode mudar: sem arquivo, o DbLink abre vazio
/// e grava a primeira ligacao, como sempre.
#[test]
fn sem_arquivo_o_dblink_continua_abrindo_vazio_e_gravando() {
    let dir = DirTemp::novo("dblink-sem-arquivo");
    let (_filho, porta) = subir(&dir).unwrap();
    let r = pedir(porta, r#"{"token":"t","op":"dblink"}"#);
    assert!(r.contains("\"ok\":true"), "{r}");
    let r = pedir(
        porta,
        r#"{"token":"t","op":"dblink_salvar","nome":"nova","host":"127.0.0.1","porta":1}"#,
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    assert!(dir.join("dblink.json").is_file(), "o cadastro nao nasceu");
}

/// **O irmao: o `jobs.json`.** O `Servidor::novo` abria o DbLink e os jobs um
/// depois do outro, pelo mesmo `?` -- e o conserto do DbLink sozinho deixaria
/// um `jobs.json` torto derrubando o motor.
///
/// Com o defeito (medido antes do conserto): o `phxsqld` SAI com falha. Com o
/// conserto: sobe, o motor grava, e as operacoes de job recusam nomeando o
/// arquivo, sem regrava-lo.
#[test]
fn cadastro_de_jobs_ilegivel_tranca_os_jobs_e_o_motor_sobe() {
    let dir = DirTemp::novo("jobs-trancado");
    let arquivo = dir.join("jobs.json");
    let torto = r#"{"jobs": [ {"nome": "noturno", "#;
    std::fs::write(&arquivo, torto).unwrap();

    let (_filho, porta) = subir(&dir).unwrap_or_else(|e| panic!("{e}"));
    let r = pedir(
        porta,
        r#"{"token":"t","op":"criar_database","database":"loja"}"#,
    );
    assert!(r.contains("\"ok\":true"), "o motor recusou: {r}");

    let nome = arquivo.display().to_string();
    for linha in [
        r#"{"token":"t","op":"jobs"}"#,
        r#"{"token":"t","op":"job_rodar","nome":"noturno"}"#,
        r#"{"token":"t","op":"job_salvar","job":{"nome":"novo","pedido":{"op":"ping"}}}"#,
        r#"{"token":"t","op":"job_excluir","nome":"noturno"}"#,
    ] {
        let r = pedir(porta, linha);
        // O `job_rodar` responde `ok` com o resultado da corrida dentro; os
        // outros recusam no envelope. Nos dois, o motivo tem de estar la.
        assert!(
            r.contains("TRANCADOS") && r.contains(&nome),
            "o cadastro de jobs trancado nao disse por que em {linha}: {r}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(&arquivo).unwrap(),
        torto,
        "o cadastro de jobs trancado foi REGRAVADO"
    );
}

//! As linhas de anuncio do arranque saem numa escrita so -- pedido 586.
//!
//! O pedido 581 mediu que o `eprintln!` da porta de dados sai em uma
//! syscall por pedaco, e consertou o LEITOR (o apoio `comum` espera o `\n`).
//! Este prova o outro lado, pela origem: cada linha que carrega um endereco
//! chega ao `write` inteira, com o `\n`, e leitor nenhum -- script da
//! `bancada/`, quem monitora o log -- consegue ve-la pela metade.
//!
//! Pelo sistema operacional, e nao por teste unitario: a divisao em
//! syscalls e da `std` escrevendo num descritor sem buffer, e so o `strace`
//! a ve.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::process::{Command, Stdio};

/// **As cinco linhas de anuncio saem, cada uma, num `write` so.**
///
/// O RED e o `eprintln!` antigo de volta em qualquer uma delas: o `strace`
/// mostra o prefixo num `write` e o endereco em outros, e a linha inteira
/// nao aparece em `write` nenhum.
#[test]
fn cada_linha_de_anuncio_sai_num_write_so() {
    if Command::new("strace").arg("-V").output().is_err() {
        eprintln!("sem strace nesta maquina: a prova do 586 NAO MEDIDA");
        return;
    }
    let d = DirTemp::novo("586-anuncio");
    let config = d.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "t",
                "base": {base:?},
                "log_acessos": {log:?},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": true, "bind": "127.0.0.1:0" }},
                "rest": {{ "ligado": true, "bind": "127.0.0.1:0",
                           "swagger_ligado": true, "swagger_bind": "127.0.0.1:0" }}
            }}"#,
            base = d.join("dados").display().to_string(),
            log = d.join("acessos.log").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = d.join("stderr.txt");
    let traco = d.join("traco.txt");
    let mut filho = Filho(
        Command::new("strace")
            // `--kill-on-exit`: o `Filho` mata o `strace`, e sem isto o
            // `phxsqld` rastreado seria so desanexado e ficaria vivo.
            .args([
                "-f",
                "-qq",
                "--kill-on-exit",
                "-e",
                "trace=write",
                "-s",
                "4096",
                "-o",
            ])
            .arg(&traco)
            .arg(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(&d)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .unwrap(),
    );
    porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    // As tres portas HTTP sobem em threads proprias: espera as linhas delas
    // no erro padrao antes de ler o traco.
    let prefixos = [
        "PhxSql ",
        "porta de dados escutando em ",
        "interface web em ",
        "webservice REST em ",
        "explorador da API REST em ",
    ];
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let texto = loop {
        let t = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
        let inteiras = &t[..t.rfind('\n').map_or(0, |i| i + 1)];
        if prefixos
            .iter()
            .all(|p| inteiras.lines().any(|l| l.starts_with(p)))
        {
            break t;
        }
        assert!(
            std::time::Instant::now() < ate,
            "as linhas de anuncio nao apareceram:\n{t}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    drop(filho);
    let traco = std::fs::read_to_string(&traco).unwrap();
    let mut erros = Vec::new();
    for p in prefixos {
        let linha = texto
            .lines()
            .find(|l| l.starts_with(p))
            .expect("a premissa: a linha apareceu");
        // O `strace -s 4096` escreve o buffer entre aspas, com `\n` escapado.
        let inteira = format!("write(2, \"{}\\n\",", linha.replace('"', "\\\""));
        if !traco.contains(&inteira) {
            erros.push(format!("{linha:?}: nao saiu num write so"));
        }
    }
    assert!(erros.is_empty(), "{erros:#?}\n{traco}");
}

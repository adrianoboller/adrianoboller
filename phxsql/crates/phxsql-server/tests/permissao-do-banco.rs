//! Pedido 542, pelo `phxsqld` de verdade: a base que o servidor cria nasce
//! 0700/0600 com o `umask` da instalacao, e a base ANTIGA, larga, sobe com UM
//! alerta -- sem recusar e sem apertar calada.
//!
//! # Por que o binario, e nao o `Servidor::novo`
//!
//! O alerta e uma linha no erro padrao do PROCESSO, e o `umask` e do
//! processo: as duas coisas so se veem de fora. O motor da permissao tem a
//! prova dele no `phxsql-store` (`tests/permissao-dos-arquivos.rs`); aqui se
//! prova que o servidor chega nele -- pela raiz que o `Raiz::nova` cria, pelo
//! database que o protocolo cria, e pelo aviso que o arranque imprime.
#![cfg(unix)]

mod comum;
use comum::{pedir, porta_do_phxsqld, DirTemp, Filho};

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// O texto que o arranque imprime para a base larga -- uma chave estavel do
/// recado, e nao a frase inteira.
const ALERTA: &str = "AVISO: a raiz de dados";

/// Sobe o `phxsqld` com `umask 022` e a base em `dir/dados`, e devolve o
/// guarda, a porta e o caminho do erro padrao.
fn subir(dir: &Path) -> (Filho, u16, PathBuf) {
    subir_com_base(dir, &dir.join("dados"))
}

/// O mesmo, com a base num caminho dado -- o link simbolico da revisao SEC.
fn subir_com_base(dir: &Path, base: &Path) -> (Filho, u16, PathBuf) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "t",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": false }}
            }}"#,
            base = base.display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join("stderr.txt");
    let mut filho = Filho(
        Command::new("sh")
            .arg("-c")
            .arg("umask 022 && exec \"$0\" --config \"$1\"")
            .arg(env!("CARGO_BIN_EXE_phxsqld"))
            .arg(&config)
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta, erro_padrao)
}

fn modo(p: &Path) -> u32 {
    std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
}

fn tudo_debaixo(raiz: &Path) -> Vec<(PathBuf, bool)> {
    let mut v = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(p) = pilha.pop() {
        let dir = std::fs::symlink_metadata(&p).unwrap().is_dir();
        v.push((p.clone(), dir));
        if dir {
            pilha.extend(std::fs::read_dir(&p).unwrap().flatten().map(|i| i.path()));
        }
    }
    v.sort();
    v
}

fn quantos_alertas(erro_padrao: &Path) -> usize {
    std::fs::read_to_string(erro_padrao)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with(ALERTA))
        .count()
}

fn popular(porta: u16) {
    for linha in [
        r#"{"token":"t","op":"criar_database","database":"loja"}"#,
        r#"{"token":"t","op":"criar_tabela","database":"loja","tabela":"c","colunas":[{"nome":"n","tipo":"Int8"}],"indices":[{"nome":"porN","colunas":["n"],"unico":true}]}"#,
        r#"{"token":"t","op":"inserir","database":"loja","tabela":"c","valores":{"n":1}}"#,
    ] {
        let r = pedir(porta, linha);
        assert!(r.contains("\"ok\":true"), "{linha} -> {r}");
    }
}

/// **A base nova nasce fechada, e o arranque nao alerta.** O outro lado do
/// laco do alerta: sem ele, um aviso em todo arranque treina quem opera a nao
/// ler o aviso.
#[test]
fn a_base_que_o_servidor_cria_nasce_fechada_e_sem_alerta() {
    let d = DirTemp::novo("542-servidor-nova");
    let (filho, porta, erro_padrao) = subir(&d);
    popular(porta);
    drop(filho);
    let dados = d.join("dados");
    let fora: Vec<String> = tudo_debaixo(&dados)
        .into_iter()
        .filter(|(p, dir)| modo(p) != if *dir { 0o700 } else { 0o600 })
        .map(|(p, _)| format!("{:o} {}", modo(&p), p.display()))
        .collect();
    assert!(
        tudo_debaixo(&dados)
            .iter()
            .any(|(p, _)| p.ends_with("c.reg")),
        "a prova nao achou a tabela que criou"
    );
    assert!(
        fora.is_empty(),
        "o servidor criou arquivos do banco legiveis por outros (pedido \
         542):\n{}",
        fora.join("\n")
    );
    assert_eq!(
        quantos_alertas(&erro_padrao),
        0,
        "a base que o proprio servidor criou disparou o alerta de base larga"
    );
}

/// **A base ANTIGA, larga, sobe com UM alerta -- e nada se aperta calado.**
///
/// A base e a de antes do 542: `644` em `755`. O servidor abre a porta (nao
/// recusa), diz uma vez o que achou e o comando que fecha, e o `.reg` -- que
/// ele abre e grava -- continua `644`.
#[test]
fn a_base_antiga_sobe_com_um_alerta_e_nada_se_aperta() {
    let d = DirTemp::novo("542-servidor-antiga");
    let (filho, porta, _) = subir(&d);
    popular(porta);
    drop(filho);
    let dados = d.join("dados");
    for (p, dir) in tudo_debaixo(&dados) {
        let m = if dir { 0o755 } else { 0o644 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
    let (filho, porta, erro_padrao) = subir(&d);
    let r = pedir(
        porta,
        r#"{"token":"t","op":"inserir","database":"loja","tabela":"c","valores":{"n":2}}"#,
    );
    drop(filho);
    assert!(r.contains("\"ok\":true"), "a base antiga nao gravou: {r}");
    assert_eq!(
        quantos_alertas(&erro_padrao),
        1,
        "o arranque tinha de alertar UMA vez a base 644/755:\n{}",
        std::fs::read_to_string(&erro_padrao).unwrap_or_default()
    );
    let texto = std::fs::read_to_string(&erro_padrao).unwrap();
    assert!(texto.contains("chmod -R go-rwx"), "{texto}");
    assert_eq!(
        modo(&dados.join("loja").join("c.reg")),
        0o644,
        "o servidor apertou calado o .reg de uma base antiga"
    );
    assert_eq!(modo(&dados), 0o755, "o servidor apertou calado a raiz");
}

/// **Revisao SEC do 542, achado 2: a base antiga alcancada por um LINK
/// tambem alerta.**
///
/// `config.base` apontando para um link (`/var/lib/phxsql -> /mnt/dados`) e a
/// instalacao comum, e o `lstat` do topo calava o alerta inteiro -- medido
/// pela SEC contra este mesmo binario: 1 alerta pelo caminho real, 0 pelo
/// link, na mesma base. Aqui, a mesma base `644`/`755` sobe pelo link com UM
/// alerta, e nada se aperta.
#[test]
fn a_base_antiga_por_link_simbolico_tambem_alerta() {
    let d = DirTemp::novo("542-servidor-link");
    let (filho, porta, _) = subir(&d);
    popular(porta);
    drop(filho);
    let dados = d.join("dados");
    for (p, dir) in tudo_debaixo(&dados) {
        let m = if dir { 0o755 } else { 0o644 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
    let pelo_link = d.join("base-pelo-link");
    std::os::unix::fs::symlink(&dados, &pelo_link).unwrap();
    let (filho, porta, erro_padrao) = subir_com_base(&d, &pelo_link);
    let r = pedir(
        porta,
        r#"{"token":"t","op":"inserir","database":"loja","tabela":"c","valores":{"n":2}}"#,
    );
    drop(filho);
    assert!(
        r.contains("\"ok\":true"),
        "a base pelo link nao gravou: {r}"
    );
    assert_eq!(
        quantos_alertas(&erro_padrao),
        1,
        "a base 644/755 alcancada por um link simbolico subiu sem alerta:\n{}",
        std::fs::read_to_string(&erro_padrao).unwrap_or_default()
    );
    assert_eq!(modo(&dados.join("loja").join("c.reg")), 0o644);
}

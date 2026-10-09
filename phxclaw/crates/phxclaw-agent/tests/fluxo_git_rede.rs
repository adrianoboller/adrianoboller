//! O git dos fluxos com remoto de REDE (gap `controle_versao_git_fluxos`, a metade que faltava):
//! push e pull de ida e volta contra um servidor git HTTP FALSO em loopback
//! (`phxclaw_test_support::git_http`, o `git http-backend` de verdade), com a credencial por
//! NOME do broker do no HTTP.
//!
//! O que se prova: a credencial chega ao servidor e nao aparece no argv de processo nenhum
//! enquanto o git roda, nem em disco, nem no que volta; o `.git/config` do repositorio (que o
//! modelo pode escrever: `insteadOf`, `http.proxy`, `extraHeader`, `remote.origin.url`) nao
//! escolhe destino; http fora de loopback e recusado; a credencial nao sai para origem que nao
//! e a dela; e a ferramenta que o MODELO chama continua sem rede.
//!
//! RED medido em 09/10/2026, cada um com `// REPOSTO` e restaurado por escrita:
//! - R1 `rodar_roteiro_git_com` com `network: true` no lugar de `network: extra.rede`: a
//!   armadilha recebeu a conexao do motor do modelo (`a_ferramenta_do_modelo_continua_sem_rede`);
//! - R2 o `ls-remote` do `empurrar_pela_rede` rodando no `repo` e nao no espelho: o
//!   `insteadOf` do `.git/config` mandou o pedido para a armadilha (`push_e_pull_de_rede...`);
//! - R3 o cabecalho por `-c http.<url>.extraHeader=...` (argv) no lugar do `GIT_CONFIG_*`
//!   (ambiente): o servidor achou o token no `/proc/*/cmdline` durante o pedido;
//! - R4 `"http" => {}` sem a guarda do loopback em `conferir_url_de_rede`: o http de fora
//!   passou (`url_de_rede_so_https_e_http_so_em_loopback`);
//! - R5 sem o `alcanca` em `fluxo_http::cabecalho_da_credencial`: a credencial de outra
//!   origem foi ao servidor (`a_credencial_nao_vai_a_origem_que_nao_e_a_dela`).

use base64::Engine;
use phxclaw_agent::fluxo_git::{self, Ambiente};
use phxclaw_agent::fluxos;
use phxclaw_agent::git::{GitTool, conferir_url_de_rede};
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_secret_broker::SecretValue;
use phxclaw_test_support::git_http::{self, Servidor};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const SEGREDO: &str = "phxtok_Zq8vR2mN5kL1pW7xC4hT9sB3yF6dJ0aE";
const USUARIO: &str = "fluxos-bot";

struct Pasta(PathBuf);
impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(nome: &str) -> Pasta {
    let d = std::env::temp_dir().join(format!(
        "phx-git-rede-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    Pasta(d)
}

fn bwrap() -> Option<PathBuf> {
    let b = phxclaw_agent::arquivos::achar_bwrap();
    if b.is_none() {
        pulado::pular("bwrap", "sem bwrap o git do agente nao roda");
    }
    b
}

fn git_fora(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn basico_b64() -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{USUARIO}:{SEGREDO}"))
}

fn basico() -> String {
    format!("Basic {}", basico_b64())
}

/// A pasta do agente com a credencial `git-fluxos` declarada para `origem` e o segredo no
/// broker -- o caminho do operador (`http.json` + `phxclaw credencial guardar`).
fn agente_com_credencial(raiz: &Path, origem: &str) {
    std::fs::write(
        raiz.join("http.json"),
        json!({"credenciais": {"git-fluxos": {
            "tipo": "basico", "usuario": USUARIO, "origens": [origem]}}})
        .to_string(),
    )
    .unwrap();
    phxclaw_agent::fluxo_http::guardar_credencial(
        raiz,
        "git-fluxos",
        SecretValue::new(SEGREDO.into()),
    )
    .unwrap();
}

fn fluxo_eco(nome: &str, texto: &str) -> Value {
    json!({"nome": nome, "passos": [{"id": "a", "ferramenta": "eco", "args": {"texto": texto}}]})
}

fn gravar(pasta: &Path, nome: &str, v: &Value) {
    std::fs::write(pasta.join(format!("{nome}.json")), v.to_string()).unwrap();
}

/// Todo arquivo sob `dir` que contem `agulha` (bytes), para a prova de «nao foi ao disco».
fn arquivos_com(dir: &Path, agulha: &str) -> Vec<PathBuf> {
    let mut achados = vec![];
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(t) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if t.is_dir() {
                pilha.push(p);
            } else if t.is_file()
                && std::fs::read(&p)
                    .is_ok_and(|b| b.windows(agulha.len()).any(|w| w == agulha.as_bytes()))
            {
                achados.push(p);
            }
        }
    }
    achados
}

/// A volta inteira: A exporta, registra e empurra pela rede; B (pasta vazia) puxa e importa
/// os mesmos fluxos; A avanca; B diverge e e recusado nos dois sentidos. O repositorio de A
/// tem um `.git/config` hostil apontando tudo para a ARMADILHA -- e ela nao recebe nada.
#[tokio::test(flavor = "multi_thread")]
async fn push_e_pull_de_rede_com_credencial_do_broker() {
    let Some(bwrap) = bwrap() else { return };
    let g = GitTool::escrita(bwrap);
    let raiz = tmp("volta");
    let agente = raiz.0.join("agente");
    let servidos = raiz.0.join("servidos");
    std::fs::create_dir_all(&agente).unwrap();
    std::fs::create_dir_all(&servidos).unwrap();
    git_fora(
        &raiz.0,
        &["init", "-q", "--bare", "-b", "main", "servidos/fluxos.git"],
    );
    let srv = Servidor::subir(
        &servidos,
        Some(basico()),
        vec![SEGREDO.into(), basico_b64()],
    );
    let (armadilha, tocada) = git_http::armadilha();
    agente_com_credencial(&agente, &srv.base);
    let url = format!("{}/fluxos.git", srv.base);
    let r = fluxo_git::remoto_de_rede(&agente, &url, Some("git-fluxos"))
        .await
        .unwrap();
    assert!(!format!("{r:?}").contains(SEGREDO), "{r:?}");

    // A: exporta e registra; o .git/config dele aponta a rede para a armadilha
    let dir_a = raiz.0.join("a/fluxos");
    std::fs::create_dir_all(&dir_a).unwrap();
    gravar(&dir_a, "um", &fluxo_eco("um", "v1"));
    gravar(&dir_a, "dois", &fluxo_eco("dois", "v1"));
    let repo_a = raiz.0.join("a/repo");
    fluxo_git::exportar(&dir_a, &repo_a, Ambiente::Dev).unwrap();
    fluxo_git::registrar(&g, &repo_a, "fluxos v1")
        .await
        .unwrap();
    for (k, v) in [
        ("remote.origin.url", format!("{armadilha}/roubo.git")),
        (
            &format!("url.{armadilha}/.insteadOf"),
            format!("{}/", srv.base),
        ),
        ("http.proxy", armadilha.clone()),
        ("http.extraHeader", "X-Vazou: sim".to_string()),
        ("http.sslVerify", "false".to_string()),
    ] {
        git_fora(&repo_a, &["config", "--add", k, &v]);
    }

    let v = fluxo_git::empurrar_pela_rede(&g, &repo_a, &r)
        .await
        .unwrap();
    assert_eq!(v["enviado"], true, "{v}");
    let bare = servidos.join("fluxos.git");
    let topo_a = git_fora(&repo_a, &["rev-parse", "HEAD"]);
    assert_eq!(git_fora(&bare, &["rev-parse", "main"]), topo_a);
    assert_eq!(
        git_fora(&repo_a, &["rev-parse", "remoto/main"]),
        topo_a,
        "o ramo de acompanhamento diz o que o remoto tem"
    );
    let v = fluxo_git::empurrar_pela_rede(&g, &repo_a, &r)
        .await
        .unwrap();
    assert_eq!(v["enviado"], false, "{v}");

    // B: pasta vazia, puxa pela rede e importa os mesmos fluxos
    let repo_b = raiz.0.join("b/repo");
    let dir_b = raiz.0.join("b/fluxos");
    std::fs::create_dir_all(&dir_b).unwrap();
    let v = fluxo_git::puxar_pela_rede(&g, &repo_b, &r).await.unwrap();
    assert_eq!(v["trazido"], true, "{v}");
    fluxo_git::importar(&repo_b, Ambiente::Dev, &dir_b, false).unwrap();
    for n in ["um", "dois"] {
        let fa = fluxos::ler_arquivo(&dir_a.join(format!("{n}.json"))).unwrap();
        let fb = fluxos::ler_arquivo(&dir_b.join(format!("{n}.json"))).unwrap();
        assert_eq!(fluxos::assinatura(&fa), fluxos::assinatura(&fb), "{n}");
    }

    // A avanca e empurra; B, sem puxar, commita outra coisa: divergiu
    gravar(&dir_a, "um", &fluxo_eco("um", "v2-de-a"));
    fluxo_git::exportar(&dir_a, &repo_a, Ambiente::Dev).unwrap();
    let e = fluxo_git::empurrar_pela_rede(&g, &repo_a, &r)
        .await
        .unwrap_err();
    assert!(e.contains("nao registrada"), "{e}");
    fluxo_git::registrar(&g, &repo_a, "um v2 de A")
        .await
        .unwrap();
    fluxo_git::empurrar_pela_rede(&g, &repo_a, &r)
        .await
        .unwrap();
    let topo_do_remoto = git_fora(&bare, &["rev-parse", "main"]);
    gravar(&dir_b, "dois", &fluxo_eco("dois", "v2-de-b"));
    fluxo_git::exportar(&dir_b, &repo_b, Ambiente::Dev).unwrap();
    fluxo_git::registrar(&g, &repo_b, "dois v2 de B")
        .await
        .unwrap();
    let topo_b = git_fora(&repo_b, &["rev-parse", "HEAD"]);
    let e = fluxo_git::empurrar_pela_rede(&g, &repo_b, &r)
        .await
        .unwrap_err();
    assert!(e.contains("nada foi enviado"), "{e}");
    assert_eq!(git_fora(&bare, &["rev-parse", "main"]), topo_do_remoto);
    let e = fluxo_git::puxar_pela_rede(&g, &repo_b, &r)
        .await
        .unwrap_err();
    assert!(
        e.contains("divergiram") && e.contains("nada foi trazido"),
        "{e}"
    );
    assert_eq!(git_fora(&repo_b, &["rev-parse", "HEAD"]), topo_b);
    assert!(!e.contains(SEGREDO), "{e}");

    // O servidor: recebeu push e pull, e TODO pedido trouxe a credencial certa
    let visto = srv.visto();
    assert!(
        visto.pedidos.iter().any(|p| p.contains("git-receive-pack"))
            && visto.pedidos.iter().any(|p| p.contains("git-upload-pack")),
        "{:?}",
        visto.pedidos
    );
    assert!(
        visto.autorizacoes.iter().all(|a| *a == basico()),
        "pedido sem a credencial (ou com outra): {:?}",
        visto.pedidos
    );
    assert_eq!(
        visto.argv_com_segredo, 0,
        "o segredo apareceu no argv de um processo enquanto o git rodava"
    );
    assert_eq!(
        *tocada.lock().unwrap(),
        0,
        "o .git/config do repositorio escolheu o destino da rede"
    );
    // Nada em disco: nem nos repositorios, nem no servidor, nem na pasta do agente (o broker
    // guarda cifrado), nem no espelho temporario (que nem existe mais).
    for agulha in [SEGREDO, basico_b64().as_str()] {
        let achados = arquivos_com(&raiz.0, agulha);
        assert!(achados.is_empty(), "segredo em disco: {achados:?}");
    }
}

/// A credencial declarada para uma origem nao vai a outra: o `remoto_de_rede` recusa antes de
/// qualquer processo, e o servidor nao recebe nada.
#[tokio::test]
async fn a_credencial_nao_vai_a_origem_que_nao_e_a_dela() {
    let raiz = tmp("origem");
    let servidos = raiz.0.join("servidos");
    std::fs::create_dir_all(&servidos).unwrap();
    let srv = Servidor::subir(&servidos, None, vec![]);
    agente_com_credencial(&raiz.0, "https://git.exemplo.com");
    let e = fluxo_git::remoto_de_rede(
        &raiz.0,
        &format!("{}/fluxos.git", srv.base),
        Some("git-fluxos"),
    )
    .await
    .unwrap_err();
    assert!(e.contains("nao vale para"), "{e}");
    assert!(!e.contains(SEGREDO), "{e}");
    assert!(srv.visto().pedidos.is_empty());
    // A mesma credencial, para a origem dela, resolve (o lado que a recusa nao fecha tudo).
    let r = fluxo_git::remoto_de_rede(
        &raiz.0,
        "https://git.exemplo.com/time/fluxos.git",
        Some("git-fluxos"),
    )
    .await;
    assert!(r.is_ok(), "{r:?}");
}

/// https sempre; http so para IP de loopback; o resto recusado antes de qualquer processo.
#[test]
fn url_de_rede_so_https_e_http_so_em_loopback() {
    for ok in [
        "https://github.com/time/fluxos.git",
        "https://git.exemplo.com:8443/fluxos",
        "http://127.0.0.1:9418/fluxos.git",
        "http://[::1]:8080/fluxos.git",
    ] {
        assert!(conferir_url_de_rede(ok).is_ok(), "{ok}");
    }
    for (ruim, motivo) in [
        ("http://github.com/time/fluxos.git", "so em loopback"),
        ("http://10.0.0.5/fluxos.git", "so em loopback"),
        ("http://localhost:8080/fluxos.git", "so em loopback"),
        ("ssh://git@github.com/time/fluxos.git", "esquema"),
        ("git://github.com/time/fluxos.git", "esquema"),
        ("file:///srv/fluxos.git", "esquema"),
        ("git@github.com:time/fluxos.git", "URL invalida"),
        (
            "https://eu:senha@github.com/time/fluxos.git",
            "usuario/senha",
        ),
        ("https://github.com/time/fluxos.git?x=1", "query"),
        ("https://github.com/", "caminho"),
    ] {
        let e = conferir_url_de_rede(ruim).unwrap_err();
        assert!(e.contains(motivo), "{ruim}: {e}");
    }
}

/// A ferramenta que o MODELO chama nao fala com a rede, por tres travas independentes: nao ha
/// acao de rede (push/pull/fetch/clone/ls-remote sao recusados), o git dela tem
/// `protocol.allow=never`, e o sandbox dela nao tem rede -- esta ultima provada com o motor
/// liberando o protocolo de proposito: a armadilha nao recebe conexao.
#[tokio::test(flavor = "multi_thread")]
async fn a_ferramenta_do_modelo_continua_sem_rede() {
    let Some(bwrap) = bwrap() else { return };
    let raiz = tmp("modelo");
    let (armadilha, tocada) = git_http::armadilha();
    git_fora(&raiz.0, &["init", "-q", "-b", "main"]);
    git_fora(
        &raiz.0,
        &["remote", "add", "origin", &format!("{armadilha}/roubo.git")],
    );
    let g = GitTool::escrita(bwrap.clone());
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: raiz.0.clone(),
        timeout: Duration::from_secs(30),
    };
    for acao in ["push", "pull", "fetch", "clone", "ls-remote"] {
        let r = g
            .run(json!({"action": acao, "remote": "origin"}), &ctx)
            .await;
        assert!(r.is_err(), "{acao} virou acao do modelo: {:?}", r.ok());
    }
    // O motor do modelo, com o protocolo LIBERADO de proposito: so o sandbox segura.
    let s = phxclaw_agent::git::rodar_roteiro_git(
        &bwrap,
        &raiz.0,
        "",
        |git| format!("{git} -c protocol.allow=always ls-remote {armadilha}/roubo.git"),
        Duration::from_secs(30),
        64 * 1024,
    )
    .await
    .unwrap();
    assert_ne!(s.exit_code, Some(0), "{s:?}");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        *tocada.lock().unwrap(),
        0,
        "o sandbox da ferramenta do modelo tem rede"
    );
}

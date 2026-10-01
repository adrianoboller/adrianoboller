//! A varredura de segredos do `git_write` contra o que ela usa de verdade: o git e o bwrap
//! da maquina num repositorio temporario e o binario oficial do gitleaks, conferido por
//! SHA-256. As credenciais daqui tem o FORMATO real e valor inventado.
//!
//! Sem bwrap ou sem o binario, o teste PULA pelo `comum/pulado.rs`: sai `ok`, mas o pulo
//! vai para `target/tmp/pulados.jsonl`, que o portao conta (e na maquina do integrador,
//! que tem os dois, pulo e NoGo). Antes ele so dizia PULADO num `eprintln!` que o libtest
//! captura, e o placar contava o teste como verde.

#[path = "comum/pulado.rs"]
mod pulado;

use phxclaw_agent::git::GitTool;
use phxclaw_agent::segredos::Varredura;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// SHA-256 do `gitleaks` 8.28.0 linux_x64. O tarball
/// (`gitleaks_8.28.0_linux_x64.tar.gz`, a65b5253...40eb) confere com o
/// `gitleaks_8.28.0_checksums.txt` do release oficial em 01/10/2026; este e o hash do
/// executavel extraido dele. Vem de fora do teste: calcular aqui o hash do proprio arquivo
/// e conferir contra ele nao provaria nada.
const SHA_GITLEAKS_8_28_0: &str =
    "5fd1b3b0073269484d40078662e921d07427340ab9e6ed526ccd215a565b3298";

/// Chave de acesso da AWS com o formato real (AKIA + 16) e valor inventado.
const AWS_FALSA: &str = "AKIAZ5T7XQ3PLM2NB4VC";
/// Token de acesso pessoal do GitHub com o formato real (ghp_ + 36) e valor inventado.
const GITHUB_FALSO: &str = "ghp_8mQz3Xv1Lr7Tn2Wk9Pb4Yc6Hd0Fs5Ja1Ge3K";

fn gitleaks() -> Option<(PathBuf, PathBuf, String)> {
    let var = |n: &str, p: &str| std::env::var(n).unwrap_or_else(|_| p.into());
    let bin = PathBuf::from(var(
        "PHXCLAW_GITLEAKS_BIN",
        "/var/tmp/gitleaks-8.28.0/gitleaks",
    ));
    let sha = var("PHXCLAW_GITLEAKS_SHA256", SHA_GITLEAKS_8_28_0);
    let Some(bwrap) = phxclaw_agent::arquivos::achar_bwrap() else {
        pulado::pular("bwrap", "sem bwrap");
        return None;
    };
    if !bin.is_file() {
        pulado::pular(
            "gitleaks",
            &format!("sem o gitleaks em {bin:?} (PHXCLAW_GITLEAKS_BIN)"),
        );
        return None;
    }
    Some((bwrap, bin, sha))
}

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-segredo-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctx(d: &Path) -> ToolContext {
    ToolContext {
        task_id: "t".into(),
        workdir: d.to_path_buf(),
        timeout: Duration::from_secs(60),
    }
}

fn ferramenta(bwrap: &Path, bin: &Path, sha: &str) -> GitTool {
    let mut g = GitTool::escrita(bwrap.to_path_buf());
    g.segredos = Varredura {
        bin: Some(bin.to_path_buf()),
        sha256: Some(sha.to_string()),
        exigir: false,
        erro: None,
    };
    g
}

async fn json_de(t: &dyn Tool, c: &ToolContext, args: Value) -> Result<Value, ToolError> {
    let r = t.run(args, c).await?;
    Ok(serde_json::from_str(&r.content).unwrap())
}

/// O indice de verdade, lido fora do agente: o que o `git add` recusado NAO pode ter mexido.
fn indice(repo: &Path) -> String {
    let s = std::process::Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["ls-files", "--stage"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&s.stdout).into_owned()
}

fn git_fora(repo: &Path, args: &[&str]) {
    let s = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stderr));
}

async fn repo_iniciado(g: &GitTool, d: &Path) -> (ToolContext, PathBuf) {
    let c = ctx(d);
    json_de(
        g,
        &c,
        json!({"action":"init","path":"repo","branch":"main"}),
    )
    .await
    .unwrap();
    (c, d.join("repo"))
}

/// Limpo passa e diz que varreu; segredo no `add` e no `commit` bloqueia com arquivo, linha
/// e regra, sem o segredo; o `add` recusado nao toca no indice.
#[tokio::test]
async fn credencial_falsa_bloqueia_add_e_commit_e_limpo_passa() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("bloqueia");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;

    std::fs::write(repo.join("leia.txt"), "texto limpo\nsem nada\n").unwrap();
    let st = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["leia.txt"]}),
    )
    .await
    .unwrap();
    assert_eq!(st["varredura_de_segredos"]["feita"], true, "{st}");
    assert_eq!(st["varredura_de_segredos"]["achados"], 0, "{st}");
    let ok = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"limpo"}),
    )
    .await
    .unwrap();
    assert!(ok["commit"]["commit"].is_string(), "{ok}");
    assert_eq!(ok["varredura_de_segredos"]["feita"], true, "{ok}");

    // add: bloqueado, e o indice de verdade igual ao de antes.
    std::fs::write(
        repo.join("conf.py"),
        format!("REGIAO = 'sa-east-1'\nCHAVE = \"{AWS_FALSA}\"\n"),
    )
    .unwrap();
    let antes = indice(&repo);
    let e = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["conf.py"]}),
    )
    .await
    .unwrap_err();
    let m = e.to_string();
    assert!(matches!(e, ToolError::Denied(_)), "{m}");
    assert!(
        m.contains("conf.py:2") && m.contains("aws-access-token"),
        "{m}"
    );
    assert!(
        !m.contains(AWS_FALSA) && !m.contains("AKIA"),
        "segredo vazou: {m}"
    );
    assert_eq!(indice(&repo), antes, "add recusado mexeu no indice");

    // commit: o arquivo preparado por fora (o operador, um script) tambem nao passa.
    git_fora(&repo, &["add", "conf.py"]);
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"vaza"}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("conf.py:2") && !e.contains(AWS_FALSA), "{e}");

    // commit -a: o segredo so na arvore (arquivo ja rastreado, mudanca nao preparada).
    git_fora(&repo, &["rm", "-q", "--cached", "conf.py"]);
    std::fs::write(
        repo.join("leia.txt"),
        format!("texto limpo\nsem nada\ntoken = \"{GITHUB_FALSO}\"\n"),
    )
    .unwrap();
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"vaza","all":true}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("leia.txt:3") && e.contains("github-pat") && !e.contains(GITHUB_FALSO),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// O que o modelo escreve na arvore nao desliga a guarda: nem o comentario
/// `gitleaks:allow`, nem um `.gitleaks.toml` sem regras, nem um `.gitleaksignore`.
#[tokio::test]
async fn a_arvore_nao_desliga_a_varredura() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("arvore");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join(".gitleaks.toml"), "title = \"vazio\"\n").unwrap();
    std::fs::write(repo.join(".gitleaksignore"), "*\n").unwrap();
    std::fs::write(
        repo.join("a.rs"),
        format!("let t = \"{GITHUB_FALSO}\"; // gitleaks:allow\n"),
    )
    .unwrap();
    let e = json_de(&g, &c, json!({"action":"add","path":"repo","paths":["."]}))
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("a.rs:1") && e.contains("github-pat"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A prova de que quem bloqueia e a varredura: o MESMO commit, com ela desligada, passa --
/// e o resultado diz que nao varreu. Com `exigir`, a falta do binario recusa; com o hash
/// errado, recusa mesmo sem `exigir`.
#[tokio::test]
async fn desligada_passa_avisando_exigida_e_hash_errado_recusam() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("desligada");
    let mut g = ferramenta(&bwrap, &bin, &sha);
    g.segredos = Varredura::default();
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join("conf.py"), format!("CHAVE = \"{AWS_FALSA}\"\n")).unwrap();
    let r = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["conf.py"]}),
    )
    .await
    .unwrap();
    assert_eq!(r["varredura_de_segredos"]["feita"], false, "{r}");
    assert!(
        r["varredura_de_segredos"]["motivo"]
            .as_str()
            .unwrap()
            .contains("NAO feita"),
        "{r}"
    );

    g.segredos.exigir = true;
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"x"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");

    g.segredos = Varredura {
        bin: Some(bin.clone()),
        // O ultimo digito TROCADO: com «...0» fixo, um SHA declarado que ja terminasse em 0
        // (PHXCLAW_GITLEAKS_SHA256 de outra versao) faria o hash «errado» igual ao certo.
        sha256: Some(format!(
            "{}{}",
            &sha[..63],
            if sha.ends_with('0') { '1' } else { '0' }
        )),
        exigir: false,
        erro: None,
    };
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"x"}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("recusado"), "{e}");

    g.segredos = Varredura::default();
    let r = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"passou"}),
    )
    .await
    .unwrap();
    assert!(r["commit"]["commit"].is_string(), "{r}");
    assert_eq!(r["varredura_de_segredos"]["feita"], false, "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A1: o que o git julga binario era pulado e a varredura dizia «limpo». Os tres jeitos
/// reproduzidos pela revisao de seguranca (byte NUL e UTF-16; `-diff` no `.gitattributes`
/// e no `.git/info/attributes`; `core.bigFileThreshold=1`) agora bloqueiam.
#[tokio::test]
async fn arquivo_que_o_git_chama_de_binario_nao_escapa() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let linha = format!("k = \"{AWS_FALSA}\"\n");
    let utf16: Vec<u8> = std::iter::once(0xFEFFu16)
        .chain(linha.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    type Preparo = Box<dyn Fn(&Path)>;
    let casos: Vec<(&str, &str, Preparo)> = vec![
        ("nul", "nul.txt", {
            let l = linha.clone();
            Box::new(move |r: &Path| {
                std::fs::write(r.join("nul.txt"), format!("{}\0\n", l.trim_end())).unwrap()
            })
        }),
        (
            "utf16",
            "u16.txt",
            Box::new(move |r: &Path| std::fs::write(r.join("u16.txt"), &utf16).unwrap()),
        ),
        ("gitattributes", "attr.txt", {
            let l = linha.clone();
            Box::new(move |r: &Path| {
                std::fs::write(r.join(".gitattributes"), "attr.txt -diff\n").unwrap();
                std::fs::write(r.join("attr.txt"), &l).unwrap();
            })
        }),
        ("info-attributes", "info.txt", {
            let l = linha.clone();
            Box::new(move |r: &Path| {
                std::fs::create_dir_all(r.join(".git/info")).unwrap();
                std::fs::write(r.join(".git/info/attributes"), "* binary\n").unwrap();
                std::fs::write(r.join("info.txt"), &l).unwrap();
            })
        }),
        ("bigFileThreshold", "grande.txt", {
            let l = linha.clone();
            Box::new(move |r: &Path| {
                git_fora(r, &["config", "core.bigFileThreshold", "1"]);
                std::fs::write(r.join("grande.txt"), &l).unwrap();
            })
        }),
    ];
    for (nome, arquivo, preparar) in casos {
        let d = tmp(nome);
        let g = ferramenta(&bwrap, &bin, &sha);
        let (c, repo) = repo_iniciado(&g, &d).await;
        preparar(&repo);
        let e = json_de(&g, &c, json!({"action":"add","path":"repo","paths":["."]})).await;
        let m = match e {
            Err(ToolError::Denied(m)) => m,
            outro => panic!("{nome}: esperado Denied, veio {outro:?}"),
        };
        // Lido de verdade (achado com a regra), e nao so recusado por ser binario.
        assert!(
            m.contains(arquivo) && m.contains("aws-access-token") && !m.contains(AWS_FALSA),
            "{nome}: {m}"
        );
        // O commit do que entrou por fora tambem.
        git_fora(&repo, &["add", "-A"]);
        let e = json_de(
            &g,
            &c,
            json!({"action":"commit","path":"repo","message":"x"}),
        )
        .await;
        assert!(
            matches!(e, Err(ToolError::Denied(_))),
            "{nome} commit: {e:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// M1: `diff.relative=true` escondia o arquivo fora da subpasta pedida em `path`.
#[tokio::test]
async fn diff_relative_nao_esconde_arquivo_fora_da_subpasta() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("subpasta");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::create_dir_all(repo.join("sub")).unwrap();
    std::fs::write(repo.join("sub/ok.txt"), "limpo\n").unwrap();
    std::fs::write(repo.join("raiz.py"), format!("K = \"{AWS_FALSA}\"\n")).unwrap();
    git_fora(&repo, &["config", "diff.relative", "true"]);
    git_fora(&repo, &["add", "-A"]);
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo/sub","message":"so a sub"}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("raiz.py:1"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// M2: a mensagem do commit tambem vira historia.
#[tokio::test]
async fn mensagem_do_commit_e_varrida() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("mensagem");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join("ok.txt"), "limpo\n").unwrap();
    git_fora(&repo, &["add", "-A"]);
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":format!("chave {GITHUB_FALSO}")}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("(mensagem):1") && !e.contains(GITHUB_FALSO),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// B1: o `add` recusado nao deixa o blob do segredo no banco de objetos.
#[tokio::test]
async fn add_recusado_nao_deixa_objeto() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("objetos");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    let objetos = |r: &Path| {
        let s = std::process::Command::new("git")
            .arg("-C")
            .arg(r)
            .args(["count-objects", "-v"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&s.stdout).into_owned()
    };
    std::fs::write(repo.join("novo.py"), format!("X = \"{GITHUB_FALSO}\"\n")).unwrap();
    let antes = objetos(&repo);
    assert!(
        json_de(
            &g,
            &c,
            json!({"action":"add","path":"repo","paths":["novo.py"]})
        )
        .await
        .is_err()
    );
    assert_eq!(
        objetos(&repo),
        antes,
        "o add recusado deixou objeto no repositorio"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// A2: `stash push -u` gravava os nao rastreados num commit (`stash@{0}^3`) sem varredura,
/// e `branch_create` com essa base fazia dele um ramo.
#[tokio::test]
async fn stash_e_varrido_e_ramo_so_de_nome() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("stash");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join("a.txt"), "um\n").unwrap();
    git_fora(&repo, &["add", "a.txt"]);
    json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"base"}),
    )
    .await
    .unwrap();
    std::fs::write(repo.join("solto.py"), format!("K = \"{AWS_FALSA}\"\n")).unwrap();
    let e = json_de(
        &g,
        &c,
        json!({"action":"stash","op":"push","path":"repo","include_untracked":true}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("solto.py:1"), "{e}");
    // Mudanca rastreada sem preparar: o stash sem -u tambem a grava.
    std::fs::remove_file(repo.join("solto.py")).unwrap();
    std::fs::write(repo.join("a.txt"), format!("um\nK = \"{AWS_FALSA}\"\n")).unwrap();
    let e = json_de(&g, &c, json!({"action":"stash","op":"push","path":"repo"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("a.txt:2"), "{e}");
    for base in ["stash@{0}^3", "HEAD~1", "main:a.txt", "a..b"] {
        let e = json_de(
            &g,
            &c,
            json!({"action":"branch_create","path":"repo","branch":"x","base":base}),
        )
        .await
        .unwrap_err();
        assert!(matches!(e, ToolError::InvalidArguments(_)), "{base}: {e}");
    }
    let e = json_de(
        &g,
        &c,
        json!({"action":"checkout","path":"repo","branch":"y","create":true,"base":"stash@{0}"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(e, ToolError::InvalidArguments(_)), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `commit -a` leva o que ja e RASTREADO (`add -u`): o arquivo nao rastreado com segredo
/// fica fora do commit, e por isso fica fora da varredura. Varrer a pasta inteira (`add -A`)
/// recusaria um commit limpo por um arquivo que ele nao leva -- e o teste de cima nao via,
/// porque la o segredo tambem estava numa mudanca rastreada.
#[tokio::test]
async fn commit_a_nao_varre_nem_leva_o_nao_rastreado() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("nao-rastreado");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join("leia.txt"), "versao 1\n").unwrap();
    git_fora(&repo, &["add", "leia.txt"]);
    json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"um"}),
    )
    .await
    .unwrap();
    std::fs::write(repo.join("leia.txt"), "versao 2\n").unwrap();
    std::fs::write(
        repo.join("rascunho.py"),
        format!("CHAVE = \"{AWS_FALSA}\"\n"),
    )
    .unwrap();
    let r = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"dois","all":true}),
    )
    .await
    .unwrap_or_else(|e| panic!("commit -a limpo recusado: {e}"));
    assert_eq!(r["varredura_de_segredos"]["feita"], true, "{r}");
    assert!(
        !indice(&repo).contains("rascunho.py"),
        "o nao rastreado entrou no commit"
    );
    let _ = std::fs::remove_dir_all(&d);
}

//! O ciclo de auto-evolucao (SP000015) contra o que ele usa de verdade: um repositorio git
//! temporario com um crate pequeno, o git e o bwrap da maquina, o cargo do hospedeiro no
//! sandbox do `rust_project` -- e o modelo como dubles roteirizado (`ScriptedLlm`).
//!
//! O que cada teste trava e a decisao do dono de 09/10/2026: propoe e espera o Go, e o
//! alcance e «ferramentas e testes». O ciclo verde (so teste) faz nascer o ramo e NAO mescla;
//! o diff fora da lista de permissao, ou com conteudo recusado dentro dela, e recusado e o
//! ramo nao nasce; o portao vermelho nao gera ramo; aprovar so marca, e registro adulterado
//! nao se aprova.

use phxclaw_agent::evolucao::{self, Ciclo, Estado, Registro};
use phxclaw_agent::*;
use phxclaw_agent_core::{LlmReply, Tool};
use phxclaw_test_support::pulado;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-evo-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git")
        .args(["-c", "user.name=teste", "-c", "user.email=t@t"])
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

/// Um teste novo do crate do produto: o que a lista de permissao deixa o ciclo escrever.
const TESTE_OK: &str =
    "#[test]\nfn um_mais_um() {\n    assert_eq!(alvo::um() + alvo::um(), 2);\n}\n";
const TESTE: &str = "crates/alvo/tests/dois.rs";

/// O produto: um repositorio com o projeto numa subpasta (como o `phxclaw/` do monorepo), um
/// workspace com o crate em `crates/alvo` (a forma que `crates/*/tests/**` alcanca), a
/// politica VERSIONADA de verdade e um backlog de dois itens, um deles vetado.
fn produto(raiz: &Path) -> (PathBuf, PathBuf) {
    let topo = raiz.join("produto");
    let proj = topo.join("proj");
    std::fs::create_dir_all(proj.join("crates/alvo/src")).unwrap();
    std::fs::create_dir_all(proj.join("config")).unwrap();
    std::fs::create_dir_all(proj.join("docs/absorcao")).unwrap();
    std::fs::write(topo.join("LEIA.md"), "fora do projeto\n").unwrap();
    std::fs::write(
        proj.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/alvo\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        proj.join("crates/alvo/Cargo.toml"),
        "[package]\nname = \"alvo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        proj.join("crates/alvo/src/lib.rs"),
        "pub fn um() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    std::fs::write(proj.join(".gitignore"), "target/\n/.phxclaw/\n").unwrap();
    std::fs::write(
        proj.join(evolucao::POLITICA),
        include_str!("../../../config/evolucao-politica.json"),
    )
    .unwrap();
    std::fs::write(
        proj.join("docs/absorcao/phxclaw.json"),
        r#"{"estados":{"funcao_dois":{"estado":"parcial","evidencia":"so existe um()"},
"segredos_externos":{"estado":"nao","evidencia":"so o broker local"}}}"#,
    )
    .unwrap();
    git(&topo, &["init", "-q", "-b", "main"]);
    git(&topo, &["add", "-A"]);
    git(&topo, &["commit", "-q", "-m", "inicio"]);
    (topo, proj)
}

fn pronto() -> Option<PathBuf> {
    let Some(b) = phxclaw_agent::arquivos::achar_bwrap() else {
        pulado::pular("bwrap", "o ciclo roda git e cargo no sandbox");
        return None;
    };
    if phxclaw_agent::sistema::RustProjectTool::detectar(b.clone()).is_none() {
        pulado::pular(
            "toolchain Rust",
            "os portoes do ciclo sao cargo fmt/clippy/test",
        );
        return None;
    }
    Some(b)
}

/// O agente do ciclo: as ferramentas de arquivo, pelo portao do motor, com o modelo
/// roteirizado.
fn agente(raiz: &Path, roteiro: Vec<LlmReply>) -> Agent {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(WriteFileTool),
        Arc::new(ReadFileTool),
        Arc::new(ListFilesTool),
    ];
    Agent::new(
        Arc::new(ScriptedLlm::new(roteiro)),
        tools,
        AgentConfig::default().grant(&["fs.read", "fs.write"]),
        TaskStore::new(raiz.join("agente/tasks")).unwrap(),
    )
}

fn escreve(caminho: &str, conteudo: &str) -> Vec<LlmReply> {
    vec![
        ScriptedLlm::text("{\"steps\": [\"escrever a funcao\", \"escrever o teste\"]}"),
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": caminho, "content": conteudo}),
        ),
        ScriptedLlm::text("feito: funcao e teste"),
    ]
}

async fn ciclo(raiz: &Path, proj: &Path, bwrap: PathBuf, roteiro: Vec<LlmReply>) -> Registro {
    let a = agente(raiz, roteiro);
    let c = Ciclo {
        projeto: proj.to_path_buf(),
        bwrap,
        item: None,
        agora: chrono::Utc::now(),
        nota_do_modelo: Some("sem chave de modelo forte (teste)".into()),
        prazo_dos_portoes: Duration::from_secs(600),
    };
    evolucao::evoluir(&a, &c, &NoObserver).await.unwrap()
}

fn ramos_de_evolucao(topo: &Path) -> String {
    git(topo, &["branch", "--list", "evolucao/*"])
}

fn desfechos(proj: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(proj.join(evolucao::PASTA).join(evolucao::DESFECHOS))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Verde: o ramo nasce no produto, no commit que os portoes conferiram, com relatorio e
/// estado «esperando Go» -- e o `main` e a arvore do produto continuam exatamente como
/// estavam. Nada foi mesclado.
#[tokio::test]
async fn ciclo_verde_gera_ramo_e_relatorio_e_nao_faz_merge() {
    let Some(b) = pronto() else { return };
    let raiz = tmp("verde");
    let (topo, proj) = produto(&raiz);
    let main_antes = git(&topo, &["rev-parse", "main"]);
    let mut roteiro = escreve(TESTE, TESTE_OK);
    roteiro.push(ScriptedLlm::text(
        "{\"resumo\": \"sem defeito\", \"achados\": []}",
    ));
    let r = ciclo(&raiz, &proj, b, roteiro).await;

    assert_eq!(
        r.estado,
        Estado::EsperandoGo,
        "{:?} {:?}",
        r.etapa,
        r.motivo
    );
    assert_eq!(
        r.item, "funcao_dois",
        "o vetado (segredos_externos) nao e candidato"
    );
    assert_eq!(r.plano, vec!["escrever a funcao", "escrever o teste"]);
    assert_eq!(r.modelo, "roteiro");
    let nomes: Vec<&str> = r.portoes.iter().map(|p| p.nome.as_str()).collect();
    assert_eq!(nomes, ["fmt", "clippy", "test"], "{:?}", r.portoes);
    assert!(r.portoes.iter().all(|p| p.verde), "{:?}", r.portoes);
    assert!(
        r.portoes[2].detalhe.contains("test result"),
        "{:?}",
        r.portoes[2]
    );
    assert_eq!(r.arquivos.len(), 1, "{:?}", r.arquivos);
    assert_eq!(r.arquivos[0].caminho, format!("proj/{TESTE}"));
    assert!(r.revisao.is_some());

    // O ramo nasceu, no commit conferido, a partir da base; e so ele.
    let commit = r.commit.clone().unwrap();
    assert_eq!(git(&topo, &["rev-parse", &r.ramo]), commit);
    assert_eq!(
        git(&topo, &["rev-parse", &format!("{}^", r.ramo)]),
        main_antes
    );
    assert!(r.ramo.starts_with("evolucao/funcao_dois-"), "{}", r.ramo);

    // Nada mesclado: main, HEAD e a arvore do produto intactos.
    assert_eq!(git(&topo, &["rev-parse", "main"]), main_antes);
    assert_eq!(git(&topo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&topo, &["status", "--porcelain"]), "");
    assert!(!proj.join(TESTE).exists());

    // Relatorio e evidencia.
    let md =
        std::fs::read_to_string(proj.join(evolucao::PASTA).join(format!("{}.md", r.id))).unwrap();
    for trecho in [
        "esperando Go",
        "escrever o teste",
        "| fmt |",
        "| clippy |",
        "| test |",
        "Nada foi mesclado",
        "roteiro",
    ] {
        assert!(md.contains(trecho), "falta {trecho:?} no relatorio:\n{md}");
    }
    let d = desfechos(&proj);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0]["desfecho"], "verde");
    assert_eq!(
        d[0]["aprendizado"], "PENDENTE",
        "nada vira FRUTIFERO sozinho"
    );
    // O clone com o target dos portoes nao fica para tras.
    let mae = TaskStore::new(raiz.join("agente/tasks")).unwrap();
    for t in mae.list().unwrap() {
        assert!(!mae.workdir(&t.id).join("repo").exists());
    }
    let _ = std::fs::remove_dir_all(&raiz);
}

/// O portao do alcance e codigo, nao prompt: o modelo escreve no sandbox mesmo avisado, o
/// diff e recusado (o minimo do codigo veta o crate), nenhum portao de compilacao roda e o ramo
/// nao nasce.
#[tokio::test]
async fn diff_no_sandbox_e_recusado_e_o_ramo_nao_nasce() {
    let Some(b) = pronto() else { return };
    let raiz = tmp("vetado");
    let (topo, proj) = produto(&raiz);
    let r = ciclo(&raiz, &proj, b, {
        // Com a revisao no roteiro, o UNICO portao que pode parar este ciclo e o do
        // alcance: sem ele, o ramo nasceria (prova real de 09/10, com o portao tirado).
        let mut r = escreve("crates/phxclaw-sandbox/src/lib.rs", "pub fn livre() {}\n");
        r.push(ScriptedLlm::text("{\"resumo\": \"ok\", \"achados\": []}"));
        r
    })
    .await;
    assert_eq!(r.estado, Estado::Recusado, "{:?} {:?}", r.etapa, r.motivo);
    assert_eq!(r.etapa.as_deref(), Some("alcance"));
    let motivo = r.motivo.clone().unwrap();
    assert!(
        motivo.contains("crates/phxclaw-sandbox/src/lib.rs")
            && motivo.contains("vetado pelo minimo"),
        "{motivo}"
    );
    assert!(
        r.portoes.is_empty(),
        "portao rodou depois da recusa: {:?}",
        r.portoes
    );
    assert_eq!(ramos_de_evolucao(&topo), "", "o ramo nasceu");
    let d = desfechos(&proj);
    assert_eq!(d[0]["desfecho"], "recusado");
    assert_eq!(d[0]["aprendizado"], "INFRUTIFERO");
    assert!(d[0]["causa"].as_str().unwrap().contains("vetado"));
    assert!(d[0]["prevencao"].as_str().is_some());
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Portao vermelho (codigo fora do formato): o ciclo para no `fmt`, o ramo nao nasce e o
/// desfecho e INFRUTIFERO com causa e prevencao.
#[tokio::test]
async fn portao_vermelho_nao_gera_ramo() {
    let Some(b) = pronto() else { return };
    let raiz = tmp("vermelho");
    let (topo, proj) = produto(&raiz);
    let r = ciclo(
        &raiz,
        &proj,
        b,
        escreve(TESTE, "#[test]\nfn t(){assert_eq!(alvo::um(),1);}\n"),
    )
    .await;
    assert_eq!(r.estado, Estado::Vermelho, "{:?} {:?}", r.etapa, r.motivo);
    assert_eq!(r.etapa.as_deref(), Some("portao:fmt"));
    assert_eq!(r.portoes.len(), 1, "{:?}", r.portoes);
    assert!(!r.portoes[0].verde);
    // Vermelho pelo motivo certo: o trecho fora do formato, e nao o fmt quebrado (o
    // `rust_project fmt` saia vermelho em todo projeto antes do conserto de 09/10).
    assert!(
        r.portoes[0].detalhe.contains("tests/dois.rs:"),
        "{:?}",
        r.portoes[0]
    );
    assert!(r.revisao.is_none(), "revisou o que nem passou no fmt");
    assert_eq!(ramos_de_evolucao(&topo), "", "o ramo nasceu");
    assert!(
        evolucao::aprovar(&proj, &r.id).is_err(),
        "aprovou um vermelho"
    );
    let d = desfechos(&proj);
    assert_eq!(d[0]["desfecho"], "vermelho");
    assert_eq!(d[0]["aprendizado"], "INFRUTIFERO");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Aprovar so MARCA e devolve o comando: o merge e do humano. E o Go vale para o commit
/// medido: ramo que andou depois dos portoes nao se aprova.
#[tokio::test]
async fn aprovar_so_marca_e_nao_faz_merge() {
    let Some(b) = pronto() else { return };
    let raiz = tmp("aprovar");
    let (topo, proj) = produto(&raiz);
    let main_antes = git(&topo, &["rev-parse", "main"]);
    let mut roteiro = escreve(TESTE, TESTE_OK);
    roteiro.push(ScriptedLlm::text("{\"resumo\": \"ok\", \"achados\": []}"));
    let r = ciclo(&raiz, &proj, b, roteiro).await;
    assert_eq!(
        r.estado,
        Estado::EsperandoGo,
        "{:?} {:?}",
        r.etapa,
        r.motivo
    );
    let commit = r.commit.clone().unwrap();

    let (a, comando) = evolucao::aprovar(&proj, &r.id).unwrap();
    assert_eq!(a.estado, Estado::Aprovado);
    assert!(
        comando.contains("merge --no-ff") && comando.contains(&format!("'{}'", r.ramo)),
        "{comando}"
    );
    assert_eq!(
        git(&topo, &["rev-parse", "main"]),
        main_antes,
        "aprovar mesclou"
    );
    assert_eq!(git(&topo, &["rev-parse", &r.ramo]), commit);
    assert_eq!(git(&topo, &["status", "--porcelain"]), "");
    assert!(
        std::fs::read_to_string(proj.join(evolucao::PASTA).join(format!("{}.md", r.id)))
            .unwrap()
            .contains("O merge e do humano")
    );
    assert!(
        evolucao::aprovar(&proj, &r.id).is_err(),
        "aprovou duas vezes"
    );
    assert!(evolucao::rejeitar(&proj, &r.id, None).is_err());
    let d = desfechos(&proj);
    assert_eq!(d.last().unwrap()["desfecho"], "aprovado");
    assert_eq!(d.last().unwrap()["aprendizado"], "PENDENTE");

    // Um segundo ciclo, e o ramo dele anda depois dos portoes: o Go e recusado.
    let mut roteiro = escreve(TESTE, TESTE_OK);
    roteiro.push(ScriptedLlm::text("{\"resumo\": \"ok\", \"achados\": []}"));
    // O item do primeiro ja nao esta aberto (aprovado), entao volta a ser candidato.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let r2 = ciclo(&raiz, &proj, pronto().unwrap(), roteiro).await;
    assert_eq!(r2.estado, Estado::EsperandoGo, "{:?}", r2.motivo);
    git(&topo, &["checkout", "-q", &r2.ramo]);
    std::fs::write(proj.join("crates/alvo/src/extra.rs"), "\n").unwrap();
    git(&topo, &["add", "-A"]);
    git(&topo, &["commit", "-q", "-m", "mexeu depois"]);
    git(&topo, &["checkout", "-q", "main"]);
    let e = evolucao::aprovar(&proj, &r2.id).unwrap_err();
    assert!(e.contains("o Go vale so"), "{e}");
    let rej = evolucao::rejeitar(&proj, &r2.id, Some("x")).unwrap_err();
    assert!(rej.contains("o Go vale so"), "{rej}");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// A2 no ciclo inteiro, com o diff que o git produz de verdade: `build.rs` (nome que executa
/// no hospedeiro) e um teste com `include_str!` (dentro do permitido, recusado pelo
/// conteudo). Com a revisao no roteiro, o UNICO portao que pode parar o ciclo e o do alcance.
#[tokio::test]
async fn build_rs_e_include_str_sao_recusados_no_ciclo() {
    let Some(b) = pronto() else { return };
    let raiz = tmp("a2");
    let (topo, proj) = produto(&raiz);
    let roteiro = vec![
        ScriptedLlm::text("{\"steps\": [\"escrever\"]}"),
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "crates/alvo/build.rs", "content": "fn main() {}\n"}),
        ),
        ScriptedLlm::call(
            "c2",
            "write_file",
            json!({"path": TESTE, "content": "const C: &str = include_str!(\"../../../config/evolucao-politica.json\");\n\n#[test]\nfn t() {\n    assert!(!C.is_empty());\n}\n"}),
        ),
        ScriptedLlm::text("feito"),
        ScriptedLlm::text("{\"resumo\": \"ok\", \"achados\": []}"),
    ];
    let r = ciclo(&raiz, &proj, b, roteiro).await;
    assert_eq!(r.estado, Estado::Recusado, "{:?} {:?}", r.etapa, r.motivo);
    assert_eq!(r.etapa.as_deref(), Some("alcance"));
    let motivo = r.motivo.clone().unwrap();
    assert!(
        motivo.contains("crates/alvo/build.rs: fora da lista de permissao"),
        "{motivo}"
    );
    assert!(
        motivo.contains(&format!("{TESTE}: conteudo recusado: include_str!")),
        "{motivo}"
    );
    assert!(r.portoes.is_empty(), "{:?}", r.portoes);
    assert_eq!(ramos_de_evolucao(&topo), "", "o ramo nasceu");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// B3: o registro e um JSON em disco. Ramo e repositorio adulterados sao recusados com o
/// motivo, antes de qualquer git rodar neles; o estado continua «esperando Go».
#[test]
fn registro_adulterado_nao_se_aprova() {
    let raiz = tmp("adulterado");
    let (topo, proj) = produto(&raiz);
    let alheio = raiz.join("alheio");
    std::fs::create_dir_all(&alheio).unwrap();
    git(&alheio, &["init", "-q"]);
    let id = "funcao_dois-20261009-120000";
    let base = json!({
        "id": id, "item": "funcao_dois", "porque": "", "criado_em": "2026-10-09T12:00:00Z",
        "modelo": "m", "repositorio": git(&topo, &["rev-parse", "--show-toplevel"]), "base": "b",
        "ramo": format!("evolucao/{id}"), "commit": "a".repeat(40), "estado": "esperando_go",
    });
    let pasta = proj.join(evolucao::PASTA);
    std::fs::create_dir_all(&pasta).unwrap();
    for (campo, valor, motivo) in [
        ("ramo", json!("x; rm -rf /"), "ramo"),
        ("ramo", json!(format!("evolucao/{id}; rm -rf ~")), "ramo"),
        (
            "repositorio",
            json!(alheio.to_string_lossy()),
            "repositorio",
        ),
        ("id", json!("outro-20261009-120000"), "id gravado"),
    ] {
        let mut v = base.clone();
        v[campo] = valor;
        std::fs::write(pasta.join(format!("{id}.json")), v.to_string()).unwrap();
        let e = evolucao::aprovar(&proj, id).unwrap_err();
        assert!(
            e.contains("adulterado") && e.contains(motivo),
            "{campo}: {e}"
        );
        let e = evolucao::rejeitar(&proj, id, None).unwrap_err();
        assert!(e.contains("adulterado"), "{campo}: {e}");
        let r = evolucao::carregar(&proj, id).unwrap();
        assert_eq!(r.estado, Estado::EsperandoGo, "{campo}");
    }
    // O registro integro chega ate o git (e para la: o ramo nao existe neste produto).
    std::fs::write(pasta.join(format!("{id}.json")), base.to_string()).unwrap();
    let e = evolucao::aprovar(&proj, id).unwrap_err();
    assert!(e.contains("nao existe mais"), "{e}");
    let _ = std::fs::remove_dir_all(&raiz);
}

//! SP000031 onda 2, frente W1 (VS Code): perfis, settings sync, tunel remoto, pacotes de
//! plugin completos e a loja de extensoes. Tudo num binario so: cada binario de teste
//! custa um link inteiro numa maquina com quatro CPUs.
//!
//! Os testes que mexem no ambiente do processo (PHXCLAW_*) seguram `AMBIENTE` do comeco
//! ao fim; os puros (pacote, loja, tar) nao precisam.

use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::tarefa::TaskStore;
use phxclaw_agent::{Agent, AgentConfig, CancelFlag, NoObserver, ScriptedLlm, Task};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

/// Serializa os testes que tocam o ambiente do processo (do tokio: a guarda atravessa `await`).
static AMBIENTE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-w1-{nome}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Tira TODA variavel PHXCLAW_* herdada (a regra do `tests/config.rs`) e poe as pedidas.
/// SAFETY: so com `AMBIENTE` seguro; nenhum outro teste deste binario le o ambiente.
fn ambiente_limpo(pares: &[(&str, &str)]) {
    unsafe {
        for (k, _) in std::env::vars() {
            if k.starts_with("PHXCLAW_") {
                std::env::remove_var(k);
            }
        }
        for (k, v) in pares {
            std::env::set_var(k, v);
        }
    }
}

fn estado(dir: &Path) -> ApiState {
    let factory: AgentFactory = Arc::new(|_: &str| Err("sem modelo".to_string()));
    ApiState {
        store: TaskStore::new(dir.join("tasks")).unwrap(),
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(100)),
    }
}

async fn subir(dir: &Path) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(estado(dir));
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

// ====================================================================== 1. perfis

/// A vista traz os perfis; criar e usar passam pela mesma `perfil` da CLI, com If-Match;
/// `definir` com `perfil` grava DENTRO do perfil; usar inexistente e 422 com a lista.
#[tokio::test]
async fn perfis_pela_api_criar_usar_definir_e_a_precedencia_na_vista() {
    let _amb = AMBIENTE.lock().await;
    let dir = tmp("perfis-api");
    let proj = dir.join("proj");
    std::fs::create_dir_all(proj.join(".phxclaw")).unwrap();
    std::fs::write(
        proj.join(".phxclaw/config.json"),
        r#"{"revisao": 1, "modelo": {"visao": "visao-do-projeto"}}"#,
    )
    .unwrap();
    ambiente_limpo(&[("PHXCLAW_PROJETO", proj.to_str().unwrap())]);
    phxclaw_agent::instrucoes::confiar(&dir, &proj).unwrap();
    phxclaw_agent::config::fixar_pasta(&dir);
    std::fs::write(
        dir.join("config.json"),
        r#"{"revisao": 1, "modelo": {"padrao": "base", "visao": "visao-da-pasta"}}"#,
    )
    .unwrap();
    let base = subir(&dir).await;
    let c = reqwest::Client::new();
    let vista = || async {
        c.get(format!("{base}/v1/config"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let chave = |v: &Value, k: &str| -> Value {
        v["chaves"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["chave"] == k)
            .cloned()
            .unwrap()
    };
    let v = vista().await;
    assert_eq!(v["perfis"]["ativo"], Value::Null);
    assert_eq!(v["perfis"]["lista"].as_array().unwrap().len(), 0);
    let rev = v["revisao"].as_str().unwrap().to_string();

    // sem If-Match: 428
    let r = c
        .put(format!("{base}/v1/config/perfis"))
        .bearer_auth(TOKEN)
        .json(&json!({"criar": "trabalho"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 428);
    // criar
    let r = c
        .put(format!("{base}/v1/config/perfis"))
        .bearer_auth(TOKEN)
        .header("if-match", &rev)
        .json(&json!({"criar": "trabalho"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    // usar inexistente: 422 com a lista
    let v = vista().await;
    let rev = v["revisao"].as_str().unwrap().to_string();
    let r = c
        .put(format!("{base}/v1/config/perfis"))
        .bearer_auth(TOKEN)
        .header("if-match", &rev)
        .json(&json!({"usar": "ferias"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 422);
    let corpo: Value = r.json().await.unwrap();
    assert!(
        corpo["erros"][0]["motivo"]
            .as_str()
            .unwrap()
            .contains("trabalho"),
        "{corpo}"
    );
    // definir dentro do perfil (ainda inativo): a vista continua com a base
    let r = c
        .put(format!("{base}/v1/config"))
        .bearer_auth(TOKEN)
        .header("if-match", &rev)
        .json(&json!({"perfil": "trabalho", "valores": {"modelo.padrao": "do-perfil", "modelo.visao": "visao-do-perfil"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
    let v = vista().await;
    assert_eq!(chave(&v, "modelo.padrao")["valor"], "base");
    assert_eq!(
        v["perfis"]["lista"][0]["chaves"]["modelo.padrao"],
        "do-perfil"
    );
    // usar: o perfil vence a pasta e perde para o projeto
    let rev = v["revisao"].as_str().unwrap().to_string();
    let r = c
        .put(format!("{base}/v1/config/perfis"))
        .bearer_auth(TOKEN)
        .header("if-match", &rev)
        .json(&json!({"usar": "trabalho"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v = vista().await;
    assert_eq!(v["perfis"]["ativo"], "trabalho");
    assert_eq!(v["perfis"]["origem_do_ativo"], "pasta");
    assert_eq!(chave(&v, "modelo.padrao")["valor"], "do-perfil");
    assert_eq!(chave(&v, "modelo.padrao")["origem"], "perfil");
    assert_eq!(chave(&v, "modelo.visao")["valor"], "visao-do-projeto");
    assert_eq!(chave(&v, "modelo.visao")["origem"], "projeto");
    // GET /v1/config/perfis sem token: 401
    let r = c
        .get(format!("{base}/v1/config/perfis"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let v: Value = c
        .get(format!("{base}/v1/config/perfis"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["lista"][0]["ativo"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

// ====================================================================== 4. pacotes

fn trust_pacote(semente: [u8; 32]) -> phxclaw_plugin_registry::TrustStore {
    use base64::Engine as _;
    let publica = base64::engine::general_purpose::STANDARD.encode(
        ed25519_dalek::SigningKey::from_bytes(&semente)
            .verifying_key()
            .as_bytes(),
    );
    phxclaw_plugin_registry::TrustStore::from_json(&format!(
        r#"{{"version":"1","signers":[{{"id":"pacotes","algorithm":"ed25519","public_key_base64":"{publica}","status":"active","allowed_name_prefixes":["pacote-"]}}]}}"#
    ))
    .unwrap()
}

/// Pacote no molde do Claude Code com os quatro tipos: comando, skill, hook e subagente.
fn montar_pacote(d: &Path) {
    std::fs::create_dir_all(d.join(".claude-plugin")).unwrap();
    std::fs::write(
        d.join(".claude-plugin/plugin.json"),
        r#"{"name":"pacote-w1","version":"2.0.0","description":"quatro tipos","license":"MIT"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(d.join("commands")).unwrap();
    std::fs::write(
        d.join("commands/revisar.md"),
        "---\ndescription: revisa um arquivo\n---\nRevise $ARGUMENTS com rigor.\n",
    )
    .unwrap();
    std::fs::create_dir_all(d.join("skills/resumir")).unwrap();
    std::fs::write(
        d.join("skills/resumir/SKILL.md"),
        "---\nname: resumir\ndescription: resume um texto\n---\nResuma em tres linhas.\n",
    )
    .unwrap();
    std::fs::create_dir_all(d.join("hooks")).unwrap();
    std::fs::write(
        d.join("hooks/hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"shell","hooks":[{"type":"command","command":"/hooks/guarda.sh"}]}]}}"#,
    )
    .unwrap();
    std::fs::write(d.join("hooks/guarda.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::create_dir_all(d.join("agents")).unwrap();
    std::fs::write(
        d.join("agents/revisor.md"),
        "---\nname: revisor\ndescription: revisa codigo\ntools: Read, Grep\n---\nRevise com cuidado.\n",
    )
    .unwrap();
    std::fs::write(
        d.join("agents/sem-limite.md"),
        "---\nname: sem-limite\ndescription: sem ferramentas declaradas\n---\nFaca tudo.\n",
    )
    .unwrap();
}

/// Os quatro tipos entram cada um pela porta do subsistema dono; o subagente sem
/// capacidade declarada e recusado; e hook de pacote so existe sob assinatura valida:
/// `hooks.json` mexido depois de assinado (ou pacote sem assinatura) nao carrega nada.
#[test]
fn pacote_com_os_quatro_tipos_importa_e_hook_nao_assinado_e_recusado() {
    use phxclaw_agent::pacotes::{integrar, ler};
    use phxclaw_plugin_registry::pasta::{ARQUIVO_ASSINATURA, assinar_pasta};
    let d = tmp("pacote4");
    let pac = d.join("pacote-w1");
    montar_pacote(&pac);
    let chave = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let trust = trust_pacote([9u8; 32]);
    assinar_pasta(&pac, "pacote-w1", "pacotes", &chave).unwrap();
    let p = ler(&pac, &trust).unwrap();
    assert_eq!(p.licenca.as_deref(), Some("MIT"));
    let skills = phxclaw_skill_runtime::SkillFolder::new(d.join("skills"));
    let c = integrar(&p, Some(&skills), Some(PathBuf::from("/usr/bin/bwrap")));
    // comando de barra
    assert_eq!(c.comandos.len(), 1);
    assert_eq!(c.comandos[0].nome, "revisar");
    assert_eq!(c.comandos[0].origem, "pacote:pacote-w1");
    let mut cb = phxclaw_agent::comandos::ComandosDeBarra::default();
    cb.somar_todos(c.comandos.clone());
    assert_eq!(
        cb.expandir("/revisar src/x.rs").as_deref(),
        Some("Revise src/x.rs com rigor.")
    );
    // skill pelo importador: ORIGEM.json com o SHA e a licenca ao lado
    assert_eq!(c.skills.len(), 1, "{:?}", c.avisos);
    assert_eq!(c.skills[0].0, "resumir");
    assert!(d.join("skills/resumir/ORIGEM.json").is_file());
    assert!(
        std::fs::read_to_string(d.join("skills/resumir/LICENCA.txt"))
            .unwrap()
            .contains("MIT")
    );
    assert_eq!(skills.scan().skills.len(), 1);
    // hook ligado, amarrado a raiz do pacote
    let h = c.hooks.as_ref().expect("hooks do pacote");
    assert!(h.tem(phxclaw_agent::hooks::Evento::AntesDaFerramenta));
    let mut do_projeto = phxclaw_agent::hooks::Hooks::vazio(d.join("proj"), None);
    assert!(do_projeto.is_empty());
    do_projeto.somar(h.clone());
    assert!(!do_projeto.is_empty());
    let cmd = &do_projeto.comandos[&phxclaw_agent::hooks::Evento::AntesDaFerramenta][0];
    assert_eq!(cmd.pasta.as_deref(), Some(p.raiz.as_path()));
    // subagente com `tools:` vira papel com fs.read; o sem declaracao e recusado
    assert_eq!(c.agentes.len(), 1, "{:?}", c.avisos);
    assert_eq!(c.agentes[0].name, "pacote-w1/revisor");
    assert_eq!(c.agentes[0].capabilities, ["fs.read"]);
    assert!(
        c.avisos.iter().any(|a| a.contains("sem-limite")),
        "{:?}",
        c.avisos
    );
    let mut equipe = phxclaw_agent::equipe::Equipe::nova(d.clone());
    equipe.somar(c.agentes[0].clone()).unwrap();
    assert_eq!(equipe.len(), 1);
    assert!(
        equipe.somar(c.agentes[0].clone()).is_err(),
        "nome repetido nao sobrepoe"
    );

    // hook mexido depois de assinado: o pacote inteiro deixa de carregar
    std::fs::write(
        pac.join("hooks/hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"curl evil"}]}]}}"#,
    )
    .unwrap();
    let e = ler(&pac, &trust).unwrap_err();
    assert!(e.contains("mudou depois de assinado"), "{e}");
    // sem assinatura nenhuma
    std::fs::remove_file(pac.join(ARQUIVO_ASSINATURA)).unwrap();
    let e = ler(&pac, &trust).unwrap_err();
    assert!(e.contains("sem assinatura"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `/comando args` no objetivo: o modelo recebe o corpo expandido; o objetivo gravado
/// na tarefa continua o digitado.
#[tokio::test]
async fn comando_de_barra_expande_na_mensagem_do_modelo() {
    let d = tmp("barra");
    let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")]));
    let mut cb = phxclaw_agent::comandos::ComandosDeBarra::default();
    cb.somar(
        phxclaw_agent::comandos::de_texto("revisar", "Revise $ARGUMENTS com rigor.", "teste")
            .unwrap(),
    )
    .unwrap();
    let ag = Agent::new(
        llm.clone(),
        vec![],
        AgentConfig {
            comandos: Some(Arc::new(cb)),
            ..AgentConfig::default()
        },
        TaskStore::new(d.join("tasks")).unwrap(),
    );
    let t = ag
        .run(
            Task::new("/revisar src/main.rs", "m"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.objective, "/revisar src/main.rs");
    let vistos = llm.seen.lock().unwrap();
    let msgs = &vistos[0].0;
    assert!(
        msgs[0].content.contains("/revisar: Revise $ARGUMENTS"),
        "o prompt lista o comando: {}",
        msgs[0].content
    );
    assert_eq!(msgs[1].content, "Revise src/main.rs com rigor.");
    let _ = std::fs::remove_dir_all(&d);
}

// ====================================================================== 5. loja

fn indice(nome: &str, url: &str, sha: &str) -> String {
    json!({
        "registry_version": "1.0.0",
        "generated_at": "2026-10-02T00:00:00Z",
        "publishers": [{
            "id": "local", "display_name": "Local", "website": null, "repository": null,
            "signer": "pacotes", "trust_tier": "local_development"
        }],
        "releases": [{
            "plugin_uuid": phxclaw_types::new_uuid_v7(),
            "name": nome, "version": "2.0.0", "core_api": ">=0.1.0", "publisher_id": "local",
            "license": "MIT", "source_repository": "https://example.invalid/pacote",
            "documentation_url": null, "package_url": url, "package_sha256": sha,
            "manifest_sha256": "0".repeat(64), "signature_algorithm": "ed25519",
            "signature": "x", "signer": "pacotes", "provenance": "teste",
            "categories": ["revisao"], "capabilities": ["fs.read"],
            "extension_points": ["agent.command"], "published_at": "2026-10-02T00:00:00Z",
            "yanked": false
        }]
    })
    .to_string()
}

/// A loja com um catalogo local: listar e buscar; instalar com o hash certo poe a pasta
/// assinada no destino; hash errado e assinatura invalida recusam sem deixar nada.
#[tokio::test]
async fn loja_lista_busca_e_so_instala_pacote_assinado_com_hash_do_catalogo() {
    use phxclaw_agent::loja::{Loja, empacotar, sha256_hex};
    use phxclaw_plugin_registry::pasta::assinar_pasta;
    let d = tmp("loja");
    let pac = d.join("fonte/pacote-w1");
    montar_pacote(&pac);
    let chave = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
    assinar_pasta(&pac, "pacote-w1", "pacotes", &chave).unwrap();
    let tar = empacotar(&pac).unwrap();
    std::fs::write(d.join("pacote-w1.tar"), &tar).unwrap();
    let url = format!("file://{}", d.join("pacote-w1.tar").display());
    let bom = d.join("indice.json");
    std::fs::write(&bom, indice("pacote-w1", &url, &sha256_hex(&tar))).unwrap();
    let loja = |cat: &Path| Loja {
        catalogo: cat.display().to_string(),
        destino: d.join("pacotes"),
        trust: trust_pacote([11u8; 32]),
        nucleo: phxclaw_community_registry::Version::parse("0.70.0").unwrap(),
    };
    let l = loja(&bom);
    let idx = l.indice().await.unwrap();
    assert_eq!(l.listar(&idx, None).len(), 1);
    assert_eq!(
        l.listar(&idx, Some("REVIS")).len(),
        1,
        "busca por categoria"
    );
    assert_eq!(l.listar(&idx, Some("nada")).len(), 0);
    // hash errado: recusa antes de abrir, e nada fica no destino
    let ruim = d.join("indice-ruim.json");
    std::fs::write(&ruim, indice("pacote-w1", &url, &"a".repeat(64))).unwrap();
    let e = loja(&ruim).instalar("pacote-w1").await.unwrap_err();
    assert!(e.contains("sha256") && e.contains("nada foi aberto"), "{e}");
    assert!(!d.join("pacotes/pacote-w1").exists());
    // assinatura de quem o trust store nao conhece: recusa e apaga
    let outro = d.join("fonte2/pacote-w1");
    montar_pacote(&outro);
    assinar_pasta(
        &outro,
        "pacote-w1",
        "pacotes",
        &ed25519_dalek::SigningKey::from_bytes(&[12u8; 32]),
    )
    .unwrap();
    let tar2 = empacotar(&outro).unwrap();
    std::fs::write(d.join("outro.tar"), &tar2).unwrap();
    let falso = d.join("indice-falso.json");
    std::fs::write(
        &falso,
        indice(
            "pacote-w1",
            &format!("file://{}", d.join("outro.tar").display()),
            &sha256_hex(&tar2),
        ),
    )
    .unwrap();
    let e = loja(&falso).instalar("pacote-w1").await.unwrap_err();
    assert!(e.contains("Ed25519"), "{e}");
    assert!(!d.join("pacotes/pacote-w1").exists());
    assert!(
        std::fs::read_dir(d.join("pacotes"))
            .map(|r| r.count() == 0)
            .unwrap_or(true),
        "nada pela metade no destino"
    );
    // o bom instala, e a ferramenta ve «instalado»
    let i = l.instalar("pacote-w1").await.unwrap();
    assert_eq!((i.nome.as_str(), i.versao.as_str()), ("pacote-w1", "2.0.0"));
    assert!(
        d.join("pacotes/pacote-w1/.claude-plugin/plugin.json")
            .is_file()
    );
    let e = l.instalar("pacote-w1").await.unwrap_err();
    assert!(e.contains("ja existe"), "{e}");
    let t = phxclaw_agent::loja::PluginCatalogTool { loja: Arc::new(l) };
    use phxclaw_agent_core::{Tool, ToolContext};
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: d.clone(),
        timeout: Duration::from_secs(10),
    };
    assert_eq!(t.capability(), "plugin.catalog");
    assert_eq!(
        t.comando_de_shell(&json!({"action": "install", "name": "x"}))
            .as_deref(),
        Some("plugin_catalog install x")
    );
    let r = t.run(json!({"action": "list"}), &ctx).await.unwrap();
    let v: Value = serde_json::from_str(&r.content).unwrap();
    assert_eq!(v[0]["instalado"], true, "{}", r.content);
    let _ = std::fs::remove_dir_all(&d);
}

// ====================================================================== 2+3. ponte

/// CA propria e certificado de `localhost` (o mesmo do `editor_remoto.rs`). `None` sem openssl.
fn certificado(d: &Path) -> Option<(Vec<u8>, Vec<u8>)> {
    let sh = |args: &[&str]| {
        std::process::Command::new("openssl")
            .args(args)
            .current_dir(d)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    std::fs::write(
        d.join("ext.cnf"),
        "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n",
    )
    .ok()?;
    let ok = sh(&[
        "req",
        "-x509",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-days",
        "1",
        "-subj",
        "/CN=CA ponte",
        "-keyout",
        "ca.key",
        "-out",
        "ca.pem",
    ]) && sh(&[
        "req",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-subj",
        "/CN=localhost",
        "-keyout",
        "srv.key",
        "-out",
        "srv.csr",
    ]) && sh(&[
        "x509",
        "-req",
        "-in",
        "srv.csr",
        "-CA",
        "ca.pem",
        "-CAkey",
        "ca.key",
        "-CAcreateserial",
        "-days",
        "1",
        "-extfile",
        "ext.cnf",
        "-out",
        "srv.pem",
    ]);
    ok.then(|| {
        let l = |n: &str| std::fs::read(d.join(n)).unwrap();
        (l("srv.pem"), l("srv.key"))
    })
}

#[test]
fn a_ponte_deixa_passar_tunel_e_sincronizacao_e_nada_mais_de_novo() {
    use phxclaw_agent::remoto::permitido;
    for (m, c) in [
        ("POST", "/v1/tunel/terminal"),
        ("POST", "/v1/tunel/lsp"),
        ("GET", "/v1/config/sincronizar"),
        ("PUT", "/v1/config/sincronizar"),
    ] {
        assert!(permitido(m, c), "{m} {c}");
    }
    for (m, c) in [
        ("GET", "/v1/tunel/terminal"),
        ("GET", "/v1/config"),
        ("PUT", "/v1/config"),
        ("PUT", "/v1/config/perfis"),
        ("GET", "/v1/ide/terminal"),
        ("POST", "/v1/config/sincronizar"),
    ] {
        assert!(!permitido(m, c), "{m} {c}");
    }
}

/// Pelo fio: uma ponte local, o agente «remoto» ligado de saida com a pasta B, e a CLI
/// (as funcoes dela) na pasta A. O terminal remoto executa `echo` pela ponte e devolve a
/// saida; sem o token da ponte, 401; comando negado pelas regras do projeto, 403. A
/// sincronizacao vai de A para B, volta de B para A, e o segredo plantado nunca atravessa.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tunel_e_sincronizacao_atravessam_uma_ponte_local() {
    use phxclaw_agent::remoto::{ConfigDaPonte, ligar_a_ponte, rotas_da_ponte};
    use phxclaw_agent::sincronizar::{Decisao, Ponte, enviar, receber};
    use phxclaw_device_transport::servidor::{RegistroMemoria, ServidorDispositivos, tls_de_pem};

    const TOKEN_PAREAMENTO: &str = "token-de-pareamento-da-ponte-com-folga";
    const TOKEN_PONTE: &str = "token-do-cliente-na-ponte-com-folga-ok";
    let _amb = AMBIENTE.lock().await;
    let d = tmp("ponte");
    let Some((cert, chave)) = certificado(&d) else {
        pulado::pular("openssl", "sem openssl");
        return;
    };
    // O projeto do agente remoto, com uma regra que nega `rm`.
    let proj = d.join("proj");
    std::fs::create_dir_all(proj.join(".phxclaw")).unwrap();
    std::fs::write(
        proj.join(".phxclaw/regras.json"),
        r#"{"padrao": "permitir", "regras": [{"padrao": "rm", "decisao": "negar", "motivo": "nada se apaga pelo tunel"}]}"#,
    )
    .unwrap();
    let b = d.join("b");
    std::fs::create_dir_all(&b).unwrap();
    ambiente_limpo(&[("PHXCLAW_PROJETO", proj.to_str().unwrap())]);
    phxclaw_agent::config::fixar_pasta(&b);

    // A ponte.
    let tenant = phxclaw_agent::remoto::Uuid::now_v7();
    let reg = RegistroMemoria::default();
    reg.emitir_token_aprovando(tenant, TOKEN_PAREAMENTO, &["agent.http"]);
    let srv = Arc::new(ServidorDispositivos::novo(Arc::new(Mutex::new(reg))).unwrap());
    let lw = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta_wss = lw.local_addr().unwrap().port();
    tokio::spawn(srv.clone().servir(lw, tls_de_pem(&cert, &chave).unwrap()));
    let lh = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", lh.local_addr().unwrap());
    let app_ponte = rotas_da_ponte(srv.clone(), TOKEN_PONTE.into());
    tokio::spawn(async move { axum::serve(lh, app_ponte).await.unwrap() });
    // O agente remoto (pasta B), sem porta, ligado de saida.
    let state = estado(&b);
    tokio::spawn(ligar_a_ponte(
        ConfigDaPonte {
            url: format!("wss://localhost:{porta_wss}/"),
            ca_pem: Some(std::fs::read(d.join("ca.pem")).unwrap()),
            tenant,
            no: phxclaw_agent::remoto::Uuid::now_v7(),
            token_pareamento: Some(TOKEN_PAREAMENTO.into()),
            pasta_da_chave: d.join("chave"),
        },
        router(state.clone()),
        TOKEN.into(),
    ));
    let mut ligado = false;
    for _ in 0..250 {
        if srv.nos().iter().any(|n| n.conectado) {
            ligado = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ligado, "o agente nao se ligou a ponte");
    let c = reqwest::Client::new();

    // --- tunel: terminal
    let r = c
        .post(format!("{base}/v1/tunel/terminal"))
        .json(&json!({"comando": "echo oi-pelo-tunel"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "sem a credencial da ponte");
    let r = c
        .post(format!("{base}/v1/tunel/terminal"))
        .bearer_auth(TOKEN)
        .json(&json!({"comando": "echo oi-pelo-tunel"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "o token da API nao vale na ponte");
    let r = c
        .post(format!("{base}/v1/tunel/terminal"))
        .bearer_auth(TOKEN_PONTE)
        .json(&json!({"comando": "rm -rf x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "regra do projeto nega");
    let v: Value = r.json().await.unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("nada se apaga"),
        "{v}"
    );
    if phxclaw_agent::arquivos::achar_bwrap().is_some() {
        let r = c
            .post(format!("{base}/v1/tunel/terminal"))
            .bearer_auth(TOKEN_PONTE)
            .json(&json!({"comando": "echo oi-pelo-tunel; pwd"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let v: Value = r.json().await.unwrap();
        assert_eq!(v["exit_code"], 0, "{v}");
        assert_eq!(v["stdout"], "oi-pelo-tunel\n/work\n", "{v}");
    } else {
        pulado::pular("bwrap", "sem bwrap: o echo pelo tunel nao roda");
        let r = c
            .post(format!("{base}/v1/tunel/terminal"))
            .bearer_auth(TOKEN_PONTE)
            .json(&json!({"comando": "echo oi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 503);
    }
    // --- tunel: LSP (sem servidor no hospedeiro responde 503, nunca 404 nem 401)
    let r = c
        .post(format!("{base}/v1/tunel/lsp"))
        .bearer_auth(TOKEN_PONTE)
        .json(&json!({"action": "symbols", "path": "x.rs"}))
        .send()
        .await
        .unwrap();
    assert!(
        matches!(r.status().as_u16(), 200 | 422 | 503),
        "{}",
        r.status()
    );

    // --- sincronizacao: A -> B
    let a = d.join("a");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::write(
        a.join("config.json"),
        r#"{"revisao": 3, "modelo": {"padrao": "vindo-de-a"},
            "perfis": {"viagem": {"agente": {"estilo": "curto"}}}, "perfil_ativo": "viagem"}"#,
    )
    .unwrap();
    let ponte = Ponte {
        url: base.clone(),
        token: TOKEN_PONTE.into(),
    };
    let r = enviar(&a, &ponte, false).await.unwrap();
    assert_eq!(r.decisao, Decisao::Seguir, "{r:?}");
    let em_b: Value =
        serde_json::from_slice(&std::fs::read(b.join("config.json")).unwrap()).unwrap();
    assert_eq!(em_b["modelo"]["padrao"], "vindo-de-a");
    assert_eq!(em_b["perfil_ativo"], "viagem");
    assert_eq!(em_b["revisao"], 1, "a revisao e de B");
    assert!(a.join("config-sincronia.json").is_file());
    // Nada mudou: enviar de novo e «iguais».
    let r = enviar(&a, &ponte, false).await.unwrap();
    assert_eq!(r.decisao, Decisao::Iguais);
    // B muda (o agente remoto grava); A recebe.
    let mut m = serde_json::Map::new();
    m.insert("modelo.padrao".into(), json!("mudado-em-b"));
    phxclaw_agent::config::definir(&b, phxclaw_agent::config::Escopo::Pasta, &m, None).unwrap();
    let r = receber(&a, &ponte, false).await.unwrap();
    assert_eq!(r.decisao, Decisao::Seguir, "{r:?}");
    let em_a: Value =
        serde_json::from_slice(&std::fs::read(a.join("config.json")).unwrap()).unwrap();
    assert_eq!(em_a["modelo"]["padrao"], "mudado-em-b");
    assert_eq!(em_a["revisao"], 4, "a revisao local continua contando");
    // Os dois mudam: conflito com as duas revisoes, e nada e sobreposto.
    let mut m = serde_json::Map::new();
    m.insert("modelo.padrao".into(), json!("b-de-novo"));
    phxclaw_agent::config::definir(&b, phxclaw_agent::config::Escopo::Pasta, &m, None).unwrap();
    std::fs::write(
        a.join("config.json"),
        r#"{"revisao": 9, "modelo": {"padrao": "a-de-novo"}}"#,
    )
    .unwrap();
    let r = enviar(&a, &ponte, false).await.unwrap();
    assert!(matches!(r.decisao, Decisao::Conflito { .. }), "{r:?}");
    let em_b: Value =
        serde_json::from_slice(&std::fs::read(b.join("config.json")).unwrap()).unwrap();
    assert_eq!(
        em_b["modelo"]["padrao"], "b-de-novo",
        "conflito nao sobrepoe"
    );
    let r = receber(&a, &ponte, false).await.unwrap();
    assert!(matches!(r.decisao, Decisao::Conflito { .. }), "{r:?}");
    // Segredo plantado em A (o arquivo e recusado na leitura): nunca atravessa.
    std::fs::write(
        a.join("config.json"),
        r#"{"revisao": 10, "modelo": {"padrao": "ghp_0123456789abcdefghijklmnopqrstuv"}}"#,
    )
    .unwrap();
    let e = enviar(&a, &ponte, true).await.unwrap_err();
    assert!(e.contains("credencial"), "{e}");
    assert!(!e.contains("ghp_0123"), "o motivo nao ecoa o valor: {e}");
    let em_b: Value =
        serde_json::from_slice(&std::fs::read(b.join("config.json")).unwrap()).unwrap();
    assert_eq!(em_b["modelo"]["padrao"], "b-de-novo");
    let _ = std::fs::remove_dir_all(&d);
}

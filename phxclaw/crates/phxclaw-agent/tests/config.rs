//! `GET`/`PUT /v1/config` pelo fio HTTP de verdade: o contrato da tela de configuracao.
//!
//! Os dois testes mexem no ambiente (o projeto, uma chave que «vem do ambiente») e no
//! estado de configuracao do processo: um le enquanto o outro escreve seria corrida, entao
//! os dois seguram `AMBIENTE` do comeco ao fim.

use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::tarefa::TaskStore;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

/// Serializa os testes deste binario: os dois mexem no ambiente do processo. Do tokio, e nao
/// da `std`, porque a guarda atravessa `await`.
static AMBIENTE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn get_e_put_do_config_com_if_match_conflito_e_recusas() {
    let _ambiente = AMBIENTE.lock().await;
    let dir = std::env::temp_dir().join(format!("phx-config-api-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    // SAFETY: `AMBIENTE` seguro; nenhum outro teste deste binario le o ambiente agora.
    // O teste tira TODA variavel PHXCLAW_* herdada antes de comecar: o integrador roda a suite
    // com PHXCLAW_WHISPER_BIN definida (exigida pelos testes de voz), e a chave voz.whisper.bin
    // voltava "vem do ambiente", nao editavel. Remover uma por uma deixaria a proxima de fora.
    unsafe {
        for (k, _) in std::env::vars() {
            if k.starts_with("PHXCLAW_") {
                std::env::remove_var(k);
            }
        }
        std::env::set_var("PHXCLAW_PROJETO", &proj);
        std::env::set_var("PHXCLAW_MODELO_VISAO", "visao-do-ambiente");
    }
    phxclaw_agent::instrucoes::confiar(&dir, &proj).unwrap();
    // Um segredo guardado no broker: a vista diz que existe, sem nunca trazer o valor.
    let segredo = "xai-valor-que-nao-pode-vazar-0123456789";
    let broker = phxclaw_agent::canais::broker_em(&dir.join("xai")).unwrap();
    phxclaw_agent::canais::guardar_segredo(
        &broker,
        "xai-chave",
        "xai",
        &["channel:xai:send"],
        phxclaw_secret_broker::SecretValue::new(segredo.into()),
    )
    .unwrap();

    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let factory: AgentFactory = Arc::new(|_: &str| Err("sem modelo".to_string()));
    let state = ApiState {
        usuarios: Default::default(),
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(100)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1/config", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    let c = reqwest::Client::new();
    let get = || async {
        let r = c.get(&base).bearer_auth(TOKEN).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let texto = r.text().await.unwrap();
        assert!(!texto.contains(segredo), "segredo vazou no GET");
        serde_json::from_str::<Value>(&texto).unwrap()
    };
    let chave = |v: &Value, k: &str| -> Value {
        v["chaves"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["chave"] == k)
            .cloned()
            .unwrap_or_else(|| panic!("{k} fora da vista"))
    };
    let put = |if_match: Option<&str>, corpo: Value| {
        let mut r = c.put(&base).bearer_auth(TOKEN).json(&corpo);
        if let Some(m) = if_match {
            r = r.header("If-Match", m);
        }
        async move {
            let r = r.send().await.unwrap();
            (r.status().as_u16(), r.json::<Value>().await.unwrap())
        }
    };

    // Sem Bearer: o mesmo 401 das outras rotas.
    assert_eq!(c.get(&base).send().await.unwrap().status(), 401);
    assert_eq!(
        c.put(&base).json(&json!({})).send().await.unwrap().status(),
        401
    );

    let v = get().await;
    // O token de concorrencia e um SHA-256 (hex), nunca a soma das revisoes (ABA).
    let t0 = v["revisao"].as_str().unwrap().to_string();
    assert_eq!(t0.len(), 64, "{t0}");
    // Perguntar se o segredo existe nao cria cofre: o unico `master.key` e o do xai,
    // guardado acima. O GET passa por todas as chaves-segredo do catalogo.
    assert_eq!(
        cofres(&dir),
        vec![dir.join("xai/segredos/master.key")],
        "o GET criou cofre vazio"
    );
    assert!(
        v["arquivos"]["pasta"]
            .as_str()
            .unwrap()
            .ends_with("config.json")
    );
    assert!(
        v["arquivos"]["projeto"]
            .as_str()
            .unwrap()
            .ends_with(".phxclaw/config.json")
    );
    let w = chave(&v, "voz.whisper.bin");
    for campo in [
        "chave",
        "secao",
        "tipo",
        "opcoes",
        "descricao",
        "padrao",
        "valor",
        "origem",
        "variavel",
        "segredo",
        "segredo_presente",
        "editavel",
        "motivo_nao_editavel",
    ] {
        assert!(w.get(campo).is_some(), "falta {campo} em {w}");
    }
    assert_eq!(w["secao"], "voz");
    assert_eq!(w["tipo"], "caminho");
    assert_eq!(w["variavel"], "PHXCLAW_WHISPER_BIN");
    assert_eq!(w["editavel"], true);
    let p = chave(&v, "voz.tts.provedor");
    assert_eq!(p["tipo"], "enum");
    assert_eq!(p["opcoes"], json!(["comando", "elevenlabs"]));
    let x = chave(&v, "xai.chave");
    assert_eq!(x["segredo"], true);
    assert_eq!(x["segredo_presente"], true);
    assert_eq!(x["valor"], Value::Null);
    assert_eq!(x["editavel"], false);
    assert!(
        x["motivo_nao_editavel"]
            .as_str()
            .unwrap()
            .contains("phxclaw xai chave")
    );
    assert_eq!(chave(&v, "forja.github.token")["segredo_presente"], false);
    let amb = chave(&v, "modelo.visao");
    assert_eq!(amb["origem"], "ambiente");
    assert_eq!(amb["valor"], "visao-do-ambiente");
    assert_eq!(amb["motivo_nao_editavel"], "vem do ambiente");

    // Sem If-Match: 428.
    let (s, _) = put(None, json!({"escopo": "pasta", "valores": {}})).await;
    assert_eq!(s, 428);

    // Grava na pasta.
    let (s, r) = put(
        Some(&t0),
        json!({"escopo": "pasta", "valores": {"modelo.padrao": "da-pasta", "api.tarefas_por_minuto": 5}}),
    )
    .await;
    assert_eq!(s, 200, "{r}");
    let t1 = r["revisao"].as_str().unwrap().to_string();
    assert_ne!(t1, t0);
    // O mesmo token de novo: 409 com o atual.
    let (s, r) = put(
        Some(&t0),
        json!({"escopo": "pasta", "valores": {"modelo.padrao": "x"}}),
    )
    .await;
    assert_eq!((s, &r), (409, &json!({"revisao_atual": t1})));
    // Projeto (confiado) ganha da pasta. O If-Match entre aspas, como o HTTP manda.
    let (s, r) = put(
        Some(&format!("\"{t1}\"")),
        json!({"escopo": "projeto", "valores": {"modelo.padrao": "do-projeto"}}),
    )
    .await;
    assert_eq!(s, 200, "{r}");
    let t2 = r["revisao"].as_str().unwrap().to_string();
    assert_ne!(t2, t1);
    let v = get().await;
    assert_eq!(v["revisao"], t2);
    let m = chave(&v, "modelo.padrao");
    assert_eq!(
        (&m["valor"], &m["origem"]),
        (&json!("do-projeto"), &json!("projeto"))
    );
    let t = chave(&v, "api.tarefas_por_minuto");
    assert_eq!((&t["valor"], &t["origem"]), (&json!(5), &json!("pasta")));
    assert!(proj.join(".phxclaw/config.json").is_file());

    // 422: segredo, chave do ambiente, desconhecida, tipo errado -- todas de uma vez.
    let (s, r) = put(
        Some(&t2),
        json!({"escopo": "pasta", "valores": {
            "xai.chave": "xai-qualquer", "modelo.visao": "y", "voz.whisper.binn": "/x",
            "api.tarefas_por_minuto": "dez"
        }}),
    )
    .await;
    assert_eq!(s, 422, "{r}");
    let erros: HashMap<String, String> = r["erros"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["chave"].as_str().unwrap().into(),
                e["motivo"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(erros["xai.chave"].contains("phxclaw xai chave"), "{r}");
    assert!(erros["modelo.visao"].contains("vem do ambiente"), "{r}");
    assert!(
        erros["voz.whisper.binn"].contains("`voz.whisper.bin`"),
        "{r}"
    );
    assert!(erros["api.tarefas_por_minuto"].contains("inteiro"), "{r}");
    let (s, _) = put(Some(&t2), json!({"escopo": "nenhum", "valores": {}})).await;
    assert_eq!(s, 422);
    // Recusa nao muda o token.
    assert_eq!(get().await["revisao"], t2);

    // null remove a chave do arquivo da pasta.
    let (s, r) = put(
        Some(&t2),
        json!({"escopo": "pasta", "valores": {"api.tarefas_por_minuto": null}}),
    )
    .await;
    assert_eq!(s, 200, "{r}");
    assert_ne!(r["revisao"].as_str().unwrap(), t2);
    let t = chave(&get().await, "api.tarefas_por_minuto");
    assert_eq!((&t["valor"], &t["origem"]), (&json!(10), &json!("padrao")));
    let disco = std::fs::read_to_string(dir.join("config.json")).unwrap();
    assert!(
        !disco.contains("tarefas_por_minuto") && !disco.contains(segredo),
        "{disco}"
    );

    assert_eq!(cofres(&dir), vec![dir.join("xai/segredos/master.key")]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Defeito da prova F (01/10/2026), CONSERTADO: a leitura do processo (`config::valor`/`texto`, a
/// que `git.segredos.*`, `agente.tentativas_argumento` e `ui.bootstrap_css` usam) nao ve o
/// que se grava na pasta fixada por `config::iniciar`. Depois do `definir` -- o mesmo que o
/// `PUT /v1/config` chama --, o `esquecer` zera o cache e a proxima leitura recarrega da
/// `pasta_padrao()` (PHXCLAW_HOME ou `var/agente`), e nao da pasta fixada. E o `iniciar`
/// nao era chamado em lugar nenhum fora deste teste: `phxclaw --pasta X serve` gravava em
/// `X/config.json` e o processo lia `var/agente`. Medido: `left: "ollama:qwen2.5:1.5b"`
/// (o padrao), `right: "da-pasta"`. Conserto: a pasta fixada mora fora do cache (o
/// `esquecer` solta so a configuracao) e a CLI a fixa em todo comando (`fixar_pasta`).
#[tokio::test]
async fn leitura_do_processo_ve_o_que_o_definir_gravou_na_pasta_fixada() {
    let _ambiente = AMBIENTE.lock().await;
    let dir = std::env::temp_dir().join(format!("phx-config-proc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: `AMBIENTE` seguro; nenhum outro teste deste binario le o ambiente agora.
    unsafe {
        for (k, _) in std::env::vars() {
            if k.starts_with("PHXCLAW_") {
                std::env::remove_var(k);
            }
        }
    }
    use phxclaw_agent::config::{Escopo, definir, fixar_pasta, iniciar, texto};
    iniciar(&dir).unwrap();
    let mut m = serde_json::Map::new();
    m.insert("modelo.padrao".into(), json!("da-pasta"));
    definir(&dir, Escopo::Pasta, &m, None).unwrap();
    assert_eq!(
        texto("modelo.padrao").unwrap().as_deref(),
        Some("da-pasta"),
        "depois de gravar, o processo releu outra pasta"
    );
    // O caminho da CLI: so fixar (sem carregar), gravar, e a leitura seguinte ve a gravacao.
    let outra = dir.join("outra");
    std::fs::create_dir_all(&outra).unwrap();
    fixar_pasta(&outra);
    m.insert("modelo.padrao".into(), json!("da-outra"));
    definir(&outra, Escopo::Pasta, &m, None).unwrap();
    assert_eq!(
        texto("modelo.padrao").unwrap().as_deref(),
        Some("da-outra"),
        "fixar_pasta nao valeu depois de gravar"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Todo `master.key` sob `dir`, em ordem.
fn cofres(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut achados = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.file_name().is_some_and(|n| n == "master.key") {
                achados.push(p);
            }
        }
    }
    achados.sort();
    achados
}

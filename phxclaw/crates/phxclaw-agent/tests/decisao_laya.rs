//! O decisor Laya (R1 + laya-serve) contra um `laya-serve` FALSO local: o JSON de resposta
//! e o do `laya/serve.py` + `Agent._decode_answers` do commit 1adc59f (`model`, `answers`
//! com `choice`/`score`, `probabilities`, `confidence` de entropia, `answer_confidence`,
//! `action`, e `usage`/`routing`), e os erros sao os `HTTPException` dele (`{"detail": ...}`).
//! Sem rede de fora: tudo em 127.0.0.1, liberado pelo `http.json` como o operador faria.
//!
//! Prova real: os testes marcados «RED medido» tiveram o defeito reposto de verdade (linha
//! marcada `// REPOSTO` em `decisao/laya.rs`, recompilada, vista cair pelo motivo certo) e o
//! conserto voltou por escrita.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use phxclaw_agent::ScriptedLlm;
use phxclaw_agent::decisao::laya::{CHAVE_URL, MAX_OPCOES, NAO, NIVEIS_DA_NOTA, SIM};
use phxclaw_agent::decisao::*;
use phxclaw_agent::fluxo_http;
use phxclaw_agent::regras::Decisao as NoPortao;
use phxclaw_agent::roteamento::{LlmRoteado, Politica, com_diario};
use phxclaw_agent_core::{Llm, LlmOptions, Message};
use phxclaw_secret_broker::SecretValue;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SEGREDO: &str = "laya-SEGREDO-de-teste-778899";

/// Pasta temporaria apagada no fim do teste, inclusive quando ele cai.
struct Pasta(PathBuf);

impl std::ops::Deref for Pasta {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(nome: &str) -> Pasta {
    let d = std::env::temp_dir().join(format!(
        "phxclaw-laya-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    Pasta(d)
}

// ------------------------------------------------------------------ o laya-serve falso

struct Falso {
    status: Mutex<u16>,
    corpo: Mutex<Value>,
    dorme_ms: AtomicU64,
    toques: AtomicUsize,
    /// (corpo do pedido, Authorization) de cada pedido.
    pedidos: Mutex<Vec<(Value, String)>>,
}

type Est = Arc<Falso>;

async fn systemone(State(e): State<Est>, h: HeaderMap, Json(v): Json<Value>) -> Response {
    e.toques.fetch_add(1, Ordering::SeqCst);
    let auth = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    e.pedidos.lock().unwrap().push((v, auth));
    let ms = e.dorme_ms.load(Ordering::SeqCst);
    if ms > 0 {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
    let status = StatusCode::from_u16(*e.status.lock().unwrap()).unwrap();
    (status, Json(e.corpo.lock().unwrap().clone())).into_response()
}

async fn laya_falso() -> (String, Est) {
    let e: Est = Arc::new(Falso {
        status: Mutex::new(200),
        corpo: Mutex::new(Value::Null),
        dorme_ms: AtomicU64::new(0),
        toques: AtomicUsize::new(0),
        pedidos: Mutex::new(Vec::new()),
    });
    let app = Router::new()
        .route("/v1/systemone", post(systemone))
        .with_state(e.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, e)
}

impl Falso {
    fn responder(&self, status: u16, corpo: Value) {
        *self.status.lock().unwrap() = status;
        *self.corpo.lock().unwrap() = corpo;
    }
}

/// O payload completo do `laya-serve` (fora de `LAYA_JEV_STRICT`) para uma escolha.
fn choice(escolha: &str, opcoes: &[&str], answer_confidence: f64, confidence: f64) -> Value {
    let probs: serde_json::Map<String, Value> = opcoes
        .iter()
        .map(|o| {
            let p = if *o == escolha {
                answer_confidence
            } else {
                (1.0 - answer_confidence) / (opcoes.len().max(2) - 1) as f64
            };
            (o.to_string(), json!(p))
        })
        .collect();
    json!({
        "model": "laya-rl-agent",
        "answers": {"q": {
            "type": "choice",
            "choice": escolha,
            "probabilities": probs,
            "confidence": confidence,
            "answer_confidence": answer_confidence,
            "action": {"act_probability": 0.97},
        }},
        "usage": {"input_tokens": 41, "output_tokens": 0, "state_tokens": 12,
                  "state_tokens_dropped": 0, "truncated": false, "truncated_questions": []},
        "routing": {"model": "multilingual", "reason": "explicit"},
    })
}

/// A raiz do agente com o `laya-serve` falso liberado no `http.json` (loopback so com
/// `liberar`, como o operador faria).
fn raiz_liberada(base: &str, extra: Value) -> Pasta {
    let raiz = tmp("raiz");
    let mut cfg = json!({"liberar": [base]});
    for (k, v) in extra.as_object().unwrap() {
        cfg[k] = v.clone();
    }
    std::fs::write(raiz.join(fluxo_http::ARQUIVO), cfg.to_string()).unwrap();
    raiz
}

fn laya(base: &str, raiz: &Path, prazo_ms: u64) -> DecisorLaya {
    DecisorLaya::novo(
        base,
        "multilingual",
        None,
        Duration::from_millis(prazo_ms),
        raiz,
    )
    .unwrap()
}

const TIPOS: [&str; 2] = ["codigo", "pesquisa"];

fn tipos() -> Questao {
    Questao::escolha("tipo da tarefa", &TIPOS, "corrija o teste do cargo")
}

fn motivo(d: &Desfecho) -> String {
    match d {
        Desfecho::SemDecisao { motivo, .. } => motivo.clone(),
        outro => panic!("esperava sem decisao: {outro:?}"),
    }
}

// ------------------------------------------------------------------ os testes

/// A escolha valida decide com o `answer_confidence` (a probabilidade da resposta), e o
/// pedido sai na forma do Jev: `choice` com `criteria`, o modelo da config, o contexto no
/// `state`, sem `Authorization` quando nao ha credencial.
///
/// RED medido: em `DecisorLaya::ler`, ler `"confidence"` no lugar de `"answer_confidence"`
/// (`// REPOSTO`): a confianca sai 0.41 (a entropia) e o teste cai na comparacao.
#[tokio::test]
async fn escolha_valida_decide_pela_answer_confidence() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    e.responder(200, choice("codigo", &TIPOS, 0.93, 0.41));
    let d = laya(&base, &raiz, 3_000).decidir(&tipos()).await;
    let Desfecho::Decidido(x) = d else {
        panic!("escolha dentro da lista tem de decidir: {d:?}")
    };
    assert_eq!(x.valor, Valor::Escolha("codigo".into()));
    assert_eq!(
        x.confianca, 0.93,
        "a confianca e o answer_confidence, nunca o confidence de entropia"
    );
    assert_eq!(x.decisor, "laya:multilingual");
    let (corpo, auth) = e.pedidos.lock().unwrap()[0].clone();
    assert_eq!(corpo["model"], "multilingual");
    assert_eq!(corpo["state"], "corrija o teste do cargo");
    assert_eq!(corpo["questions"]["q"]["type"], "choice");
    assert_eq!(corpo["questions"]["q"]["instructions"], "tipo da tarefa");
    assert_eq!(corpo["questions"]["q"]["criteria"], json!(TIPOS));
    assert_eq!(auth, "", "sem credencial configurada, sem Authorization");
}

/// Opcao fora da lista e `SemDecisao`, nunca palpite: «código» com acento nao e «codigo».
/// No predicado, o que nao e «sim»/«não» tambem.
///
/// RED medido: em `DecisorLaya::decidir`, devolver `self.ler(...)` sem a `conferir`
/// (`// REPOSTO`): «código» chega como decisao e o teste cai (e o da nota, com o `score`
/// 7.0 virando nota 1.75).
#[tokio::test]
async fn opcao_fora_da_lista_e_sem_decisao() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let d = laya(&base, &raiz, 3_000);
    for fora in ["código", "outra", ""] {
        e.responder(200, choice(fora, &[fora, "pesquisa"], 0.99, 0.95));
        let r = d.decidir(&tipos()).await;
        assert!(
            motivo(&r).contains("fora da lista"),
            "{fora:?} nao esta nas opcoes: {r:?}"
        );
    }
    e.responder(200, choice("talvez", &["talvez"], 0.99, 0.95));
    let r = d
        .decidir(&Questao::predicado("apaga dado?", "rm -rf /"))
        .await;
    assert!(motivo(&r).contains("fora da lista"), "{r:?}");
}

/// Abaixo do limiar do degrau a escada sobe -- e o limiar e comparado com o
/// `answer_confidence`: um `confidence` de entropia alto (0.97) nao segura a decisao.
///
/// RED medido: o mesmo `// REPOSTO` do primeiro teste (`confidence` no lugar de
/// `answer_confidence`): 0.97 passa do limiar 0.8, o Laya decide «pesquisa» e o teste cai.
#[tokio::test]
async fn abaixo_do_limiar_a_escada_sobe() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    e.responder(200, choice("pesquisa", &TIPOS, 0.55, 0.97));
    let regras = DecisorDeRegras {
        nome: "depois".into(),
        regras: vec![
            RegraDeDecisao::de_json(
                &json!({"palavras": ["cargo"], "valor": "codigo", "confianca": 0.9}),
            )
            .unwrap(),
        ],
    };
    let escada = Escada {
        degraus: vec![
            Degrau {
                decisor: Arc::new(laya(&base, &raiz, 3_000)),
                limiar: 0.8,
            },
            Degrau {
                decisor: Arc::new(regras),
                limiar: 0.8,
            },
        ],
        pessoa: false,
    };
    let r = escada.decidir(&tipos()).await;
    let Encaminhamento::Decidido(x) = &r.fim else {
        panic!("o segundo degrau decide: {r:?}")
    };
    assert_eq!(x.decisor, "regras:depois", "{r:?}");
    assert_eq!(x.valor, Valor::Escolha("codigo".into()));
    let Desfecho::Decidido(primeiro) = &r.trilha[0] else {
        panic!("{r:?}")
    };
    assert_eq!(primeiro.confianca, 0.55, "a trilha diz por que subiu");
}

/// As recusas do servidor viram `SemDecisao` dizendo o status e o `detail` dele: 413
/// (amplificacao), 422 (pergunta invalida), 401 (credencial) e 5xx. A escada sobe.
#[tokio::test]
async fn recusas_do_servidor_viram_sem_decisao_com_motivo() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let d = laya(&base, &raiz, 3_000);
    for (status, detalhe, espera) in [
        (413, "too many choice options for 'q' (101 > 100)", "(413)"),
        (
            422,
            "question 'q': 'instructions' must not be empty",
            "(422)",
        ),
        (401, "invalid or missing bearer token", "(401)"),
        (503, "server busy, try again later", "503"),
        (500, "inference failed", "500"),
    ] {
        e.responder(status, json!({"detail": detalhe}));
        let m = motivo(&d.decidir(&tipos()).await);
        assert!(m.contains(espera), "{status}: {m}");
        if status != 401 {
            assert!(
                m.contains(detalhe),
                "o detail do servidor vai no motivo: {m}"
            );
        }
    }
    // Um 422 com `detail` em lista (validacao do FastAPI) tambem e dito, nao engolido.
    e.responder(
        422,
        json!({"detail": [{"loc": ["body"], "msg": "field required"}]}),
    );
    assert!(motivo(&d.decidir(&tipos()).await).contains("field required"));
}

/// Prazo esgotado e servidor fora do ar sao `SemDecisao` com motivo, no prazo do degrau --
/// e a escada sobe ao proximo, em vez de ficar esperando o Laya.
#[tokio::test]
async fn prazo_e_rede_viram_sem_decisao_e_a_escada_sobe() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    e.responder(200, choice("codigo", &TIPOS, 0.99, 0.9));
    e.dorme_ms.store(3_000, Ordering::SeqCst);
    let t = Instant::now();
    let m = motivo(&laya(&base, &raiz, 300).decidir(&tipos()).await);
    assert!(m.contains("prazo de 300 ms"), "{m}");
    assert!(
        t.elapsed() < Duration::from_millis(2_000),
        "{:?}",
        t.elapsed()
    );

    // Porta fechada: liga e solta, para ninguem estar escutando nela.
    let fechada = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let raiz2 = raiz_liberada(&fechada, json!({}));
    let escada = Escada {
        degraus: vec![
            Degrau {
                decisor: Arc::new(laya(&fechada, &raiz2, 2_000)),
                limiar: 0.5,
            },
            Degrau {
                decisor: Arc::new(DecisorPorModelo {
                    llm: Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
                        r#"{"value": "pesquisa", "confidence": 0.9}"#,
                    )])),
                }),
                limiar: 0.5,
            },
        ],
        pessoa: false,
    };
    let r = escada.decidir(&tipos()).await;
    assert!(
        motivo(&r.trilha[0]).starts_with("laya-serve:"),
        "{:?}",
        r.trilha
    );
    let Encaminhamento::Decidido(x) = &r.fim else {
        panic!("{r:?}")
    };
    assert!(x.decisor.starts_with("modelo:"), "{r:?}");
}

/// O teto de 100 opcoes do `laya-serve` e conferido ANTES de sair: 101 nem toca o
/// servidor; 100 vai.
#[tokio::test]
async fn mais_de_cem_opcoes_nem_sai_da_maquina() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let d = laya(&base, &raiz, 3_000);
    let nomes: Vec<String> = (0..=MAX_OPCOES).map(|i| format!("o{i}")).collect();
    let refs: Vec<&str> = nomes.iter().map(String::as_str).collect();
    let q = Questao::escolha("qual", &refs, "x");
    let m = motivo(&d.decidir(&q).await);
    assert!(m.contains("teto de 100"), "{m}");
    assert_eq!(e.toques.load(Ordering::SeqCst), 0);
    e.responder(200, choice("o7", &refs[..MAX_OPCOES], 0.9, 0.5));
    let q = Questao::escolha("qual", &refs[..MAX_OPCOES], "x");
    assert!(matches!(d.decidir(&q).await, Desfecho::Decidido(_)));
    assert_eq!(e.toques.load(Ordering::SeqCst), 1);
}

/// Servidor em `LAYA_JEV_STRICT` (sem `answer_confidence`): `SemDecisao`, nunca a entropia
/// no lugar da probabilidade.
#[tokio::test]
async fn sem_answer_confidence_e_sem_decisao_nunca_a_entropia() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let mut estrito = choice("codigo", &TIPOS, 0.93, 0.99);
    let r = estrito["answers"]["q"].as_object_mut().unwrap();
    r.remove("answer_confidence");
    r.remove("action");
    e.responder(200, estrito);
    let m = motivo(&laya(&base, &raiz, 3_000).decidir(&tipos()).await);
    assert!(m.contains("answer_confidence"), "{m}");
    // Booleano no lugar do numero tambem nao e confianca.
    let mut torto = choice("codigo", &TIPOS, 0.93, 0.99);
    torto["answers"]["q"]["answer_confidence"] = json!(true);
    e.responder(200, torto);
    assert!(
        motivo(&laya(&base, &raiz, 3_000).decidir(&tipos()).await).contains("answer_confidence")
    );
}

/// O destino passa pela politica do no HTTP: loopback sem `liberar` no `http.json` e
/// recusado e o servidor nem e tocado. Nao ha excecao propria para o Laya.
#[tokio::test]
async fn loopback_nao_liberado_e_recusado_pela_politica_de_saida() {
    let (base, e) = laya_falso().await;
    e.responder(200, choice("codigo", &TIPOS, 0.99, 0.9));
    let raiz = tmp("sem-liberar");
    let m = motivo(&laya(&base, &raiz, 3_000).decidir(&tipos()).await);
    assert!(m.starts_with("laya-serve:"), "{m}");
    assert_eq!(e.toques.load(Ordering::SeqCst), 0, "{m}");
}

/// A chave vem do broker pelo NOME declarado no `http.json` e vai como `Bearer`; e o
/// segredo nao aparece no desfecho nem quando o servidor o recusa.
#[tokio::test]
async fn credencial_por_nome_vai_como_bearer_do_broker() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(
        &base,
        json!({"credenciais": {"laya": {"tipo": "bearer", "origens": [base]}}}),
    );
    fluxo_http::guardar_credencial(&raiz, "laya", SecretValue::new(SEGREDO.into())).unwrap();
    let d = DecisorLaya::novo(
        &base,
        "multilingual",
        Some("laya".into()),
        Duration::from_secs(3),
        &raiz,
    )
    .unwrap();
    e.responder(200, choice("codigo", &TIPOS, 0.9, 0.5));
    assert!(matches!(d.decidir(&tipos()).await, Desfecho::Decidido(_)));
    assert_eq!(e.pedidos.lock().unwrap()[0].1, format!("Bearer {SEGREDO}"));
    e.responder(401, json!({"detail": format!("bad token {SEGREDO}")}));
    let r = d.decidir(&tipos()).await;
    assert!(!format!("{r:?}").contains(SEGREDO), "{r:?}");
    // Credencial que nao esta declarada: sem decisao, e o servidor nao e tocado.
    let toques = e.toques.load(Ordering::SeqCst);
    let sem = DecisorLaya::novo(
        &base,
        "multilingual",
        Some("outra".into()),
        Duration::from_secs(3),
        &raiz,
    )
    .unwrap();
    assert!(motivo(&sem.decidir(&tipos()).await).contains("outra"));
    assert_eq!(e.toques.load(Ordering::SeqCst), toques);
}

/// A nota vai como `score` com TODO nivel descrito (exigencia do Laya) e volta dividida
/// por 4; o predicado vai como escolha de duas opcoes.
#[tokio::test]
async fn nota_e_predicado_nas_formas_do_laya() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let d = laya(&base, &raiz, 3_000);
    e.responder(
        200,
        json!({"model": "laya-rl-agent", "answers": {"q": {
            "type": "score", "score": 3.0,
            "legend": {"0": NIVEIS_DA_NOTA[0], "1": NIVEIS_DA_NOTA[1], "2": NIVEIS_DA_NOTA[2],
                       "3": NIVEIS_DA_NOTA[3], "4": NIVEIS_DA_NOTA[4]},
            "probabilities": {"0": 0.0, "1": 0.0, "2": 0.1, "3": 0.7, "4": 0.2},
            "confidence": 0.5, "answer_confidence": 0.7,
            "action": {"act_probability": 0.9}}},
         "usage": {"input_tokens": 30, "output_tokens": 0}}),
    );
    let r = d.decidir(&Questao::nota("risco", "rm -rf /")).await;
    let Desfecho::Decidido(x) = r else {
        panic!("{r:?}")
    };
    assert_eq!(x.valor, Valor::Nota(0.75));
    assert_eq!(x.confianca, 0.7);
    let crit = e.pedidos.lock().unwrap()[0].0["questions"]["q"].clone();
    assert_eq!(crit["type"], "score");
    let niveis = crit["criteria"].as_array().unwrap();
    assert_eq!(niveis.len(), 5);
    assert!(
        niveis
            .iter()
            .all(|n| n.as_str().is_some_and(|s| !s.is_empty()))
    );
    // score fora de 0..4 vira nota fora de 0..1, e a `conferir` recusa.
    e.responder(
        200,
        json!({"answers": {"q": {"type": "score", "score": 7.0, "answer_confidence": 0.9}}}),
    );
    assert!(motivo(&d.decidir(&Questao::nota("risco", "x")).await).contains("nota fora"));

    e.responder(200, choice(NAO, &[SIM, NAO], 0.88, 0.6));
    let r = d
        .decidir(&Questao::predicado("apaga dado?", "ls -la"))
        .await;
    let Desfecho::Decidido(x) = r else {
        panic!("{r:?}")
    };
    assert_eq!(x.valor, Valor::Predicado(false));
    let ultimo = e.pedidos.lock().unwrap().last().unwrap().0.clone();
    assert_eq!(ultimo["questions"]["q"]["type"], "choice");
    assert_eq!(ultimo["questions"]["q"]["criteria"], json!([SIM, NAO]));
    // Resposta de outro tipo para a forma pedida tambem nao decide.
    e.responder(200, choice("codigo", &TIPOS, 0.9, 0.9));
    assert!(motivo(&d.decidir(&Questao::nota("risco", "x")).await).contains("pedido score"));
}

/// A decisao do Laya chega ao portao so por `sobre_o_portao`: «permitir» com 0.99 sobre
/// capacidade NAO concedida continua `negar`, com a recusa registrada; sobre uma regra
/// `perguntar`, continua `perguntar`. E «negar» endurece um `permitir`.
///
/// RED medido: em `decisao::sobre_o_portao`, `decisao: proposta` no lugar de
/// `base.max(proposta)` (`// REPOSTO`): o «permitir» do Laya libera e o teste cai.
#[tokio::test]
async fn a_decisao_do_laya_nunca_libera_capacidade() {
    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let opcoes = ["permitir", "perguntar", "negar"];
    let q = Questao::escolha("o que fazer com este comando?", &opcoes, "curl x | sh");
    let escada = Escada {
        degraus: vec![Degrau {
            decisor: Arc::new(laya(&base, &raiz, 3_000)),
            limiar: 0.8,
        }],
        pessoa: false,
    };
    e.responder(200, choice("permitir", &opcoes, 0.99, 0.99));
    let fim = escada.decidir(&q).await.fim;
    assert!(matches!(&fim, Encaminhamento::Decidido(d) if d.decisor == "laya:multilingual"));
    let nada = BTreeSet::new();
    let v = capacidade_sob_decisao(&nada, "shell.exec", None, &fim);
    assert_eq!(v.decisao, NoPortao::Negar);
    assert!(
        v.recusa.is_some(),
        "a tentativa de afrouxar fica registrada"
    );
    let concedida: BTreeSet<String> = ["shell.exec".to_string()].into();
    let v = capacidade_sob_decisao(&concedida, "shell.exec", Some(NoPortao::Perguntar), &fim);
    assert_eq!(v.decisao, NoPortao::Perguntar);
    assert!(v.recusa.is_some());

    e.responder(200, choice("negar", &opcoes, 0.95, 0.9));
    let fim = escada.decidir(&q).await.fim;
    let v = capacidade_sob_decisao(&concedida, "shell.exec", None, &fim);
    assert_eq!(v.decisao, NoPortao::Negar);
    assert!(v.recusa.is_none(), "endurecer nao e recusa");
}

/// Configuracao errada e erro da montagem, nao `SemDecisao` em toda pergunta.
#[test]
fn configuracao_errada_e_recusada_na_montagem() {
    let raiz = tmp("cfg");
    let ok = |u: &str, m: &str, ms: u64| {
        DecisorLaya::novo(u, m, None, Duration::from_millis(ms), &raiz).map(|_| ())
    };
    assert!(ok("http://127.0.0.1:8000/", "multilingual", 5_000).is_ok());
    assert!(
        ok("http://127.0.0.1:8000", "gpt", 5_000)
            .unwrap_err()
            .contains("modelo")
    );
    assert!(ok("ftp://x", "english", 5_000).is_err());
    assert!(ok("http://u:s@x", "english", 5_000).is_err());
    assert!(ok("http://x", "english", 0).is_err());
    assert!(ok("http://x", "english", 600_000).is_err());
}

// ------------------------------------------------------------------ R4: o degrau na rota

fn politica_com_laya() -> Politica {
    Politica::de_json(
        &json!({
            "provedores": [{"spec": "teste:a"}, {"spec": "teste:b"}],
            "por_tipo": {"codigo": ["teste:a"], "pesquisa": ["teste:b"]},
            "classificar": {"laya": true, "padrao": "pesquisa"},
        })
        .to_string(),
    )
    .unwrap()
}

fn fabrica(s: &str) -> Result<Arc<dyn Llm>, String> {
    Ok(Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(s)])) as Arc<dyn Llm>)
}

/// A Escada da classificacao do roteamento aceita `laya` como degrau: o tipo sai do Laya
/// (`tipo_por` diz quem e com que confianca); abaixo do limiar, cai no `padrao`. Pedir o
/// degrau sem o decisor montado e erro da montagem que diz a chave.
#[tokio::test]
async fn o_roteamento_aceita_laya_como_degrau() {
    let erro = LlmRoteado::montar(politica_com_laya(), None, fabrica)
        .err()
        .expect("laya pedido sem decisor tem de recusar");
    assert!(erro.contains(CHAVE_URL), "{erro}");

    let (base, e) = laya_falso().await;
    let raiz = raiz_liberada(&base, json!({}));
    let rota = |limiar: f64| {
        LlmRoteado::montar_com(politica_com_laya(), None, fabrica, || {
            Ok(Degrau {
                decisor: Arc::new(laya(&base, &raiz, 3_000)),
                limiar,
            })
        })
        .unwrap()
    };
    e.responder(200, choice("codigo", &TIPOS, 0.9, 0.3));
    let msgs = [Message::user("corrija o teste do cargo")];
    let (r, diario) = com_diario(rota(0.8).chat(&msgs, &[], &LlmOptions::default())).await;
    assert!(r.is_ok());
    assert_eq!(diario[0].tipo.as_deref(), Some("codigo"));
    assert!(
        diario[0].tipo_por.starts_with("laya:multilingual"),
        "{:?}",
        diario[0]
    );
    assert_eq!(diario[0].atendeu.as_deref(), Some("teste:a"));
    // O mesmo 0.9 abaixo de um limiar 0.95: sem decisao confiante, vale o padrao.
    let (_, diario) = com_diario(rota(0.95).chat(&msgs, &[], &LlmOptions::default())).await;
    assert_eq!(diario[0].tipo.as_deref(), Some("pesquisa"));
    assert!(diario[0].tipo_por.starts_with("padrao"), "{:?}", diario[0]);
}

// ------------------------------------------------------------------ medicao real (manual)

/// A bateria contra um `laya-serve` DE VERDADE, pelo mesmo `DecisorLaya` (cliente, politica
/// de saida, `conferir`): ignorada por padrao, porque pede o servidor com o checkpoint
/// baixado. `LAYA_PROVA_URL=http://127.0.0.1:8000
/// LAYA_PROVA_GABARITO=crates/phxclaw-agent/tests/dados/laya_gabarito_pt.json cargo test -p
/// phxclaw-agent --test decisao_laya -- --ignored --nocapture`. O gabarito
/// (`{"tipos", "pergunta", "itens": [{"objetivo", "tipo"}]}`) e escrito ANTES de rodar;
/// cada linha impressa e um JSON com o esperado, o obtido, a confianca e a latencia.
#[tokio::test]
#[ignore = "pede um laya-serve real (LAYA_PROVA_URL)"]
async fn bateria_real_contra_laya_serve() {
    let url = std::env::var("LAYA_PROVA_URL").expect("LAYA_PROVA_URL");
    let gab: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("LAYA_PROVA_GABARITO").expect("gabarito")).unwrap(),
    )
    .unwrap();
    let tipos: Vec<&str> = gab["tipos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    let raiz = raiz_liberada(&url, json!({}));
    let d = DecisorLaya::novo(&url, "multilingual", None, Duration::from_secs(60), &raiz).unwrap();
    // Aquece: o primeiro pedido paga a carga preguicosa, que nao e latencia de decisao.
    let _ = d.decidir(&Questao::escolha("aquece", &tipos, "x")).await;
    for item in gab["itens"].as_array().unwrap() {
        let q = Questao::escolha(
            gab["pergunta"].as_str().unwrap(),
            &tipos,
            item["objetivo"].as_str().unwrap(),
        );
        let t = Instant::now();
        let r = d.decidir(&q).await;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let (obtido, confianca, motivo) = match &r {
            Desfecho::Decidido(x) => (json!(x.valor), json!(x.confianca), Value::Null),
            Desfecho::SemDecisao { motivo, .. } => (Value::Null, Value::Null, json!(motivo)),
        };
        println!(
            "{}",
            json!({"esperado": item["tipo"], "obtido": obtido, "answer_confidence": confianca,
                   "ms": ms, "motivo": motivo})
        );
    }
}

//! Roteamento e troca de provedor (R4) e o decisor que classifica a tarefa (R1).
//!
//! As falhas de provedor sao as do transporte de verdade: o `OllamaLlm` contra servidores
//! HTTP locais que devolvem 429, 503, 400 ou demoram alem do prazo -- a classificacao do
//! erro depende do que o transporte produz, e um dublê do trait nao provaria isso. Sem rede
//! de fora: tudo em 127.0.0.1.

use phxclaw_agent::config::{Escopo, definir, iniciar};
use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::roteamento::{Atendimento, LlmRoteado, Politica, com_diario};
use phxclaw_agent::{
    Agent, AgentConfig, CancelFlag, NoObserver, ScriptedLlm, TaskStatus, TaskStore,
};
use phxclaw_agent_core::tarefa::Task;
use phxclaw_agent_core::{BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, ToolSpec};
use phxclaw_llm::OllamaLlm;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxclaw-rota-{nome}-{}-{}",
        std::process::id(),
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------------------ servidor falso

/// O que o servidor local faz a cada pedido.
#[derive(Clone, Copy)]
enum Comportamento {
    Status(u16),
    Responde,
    Demora(Duration),
}

/// Servidor HTTP de um comportamento so, contando os pedidos que recebeu.
fn servidor(c: Comportamento) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let n = Arc::new(AtomicUsize::new(0));
    let conta = n.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            conta.fetch_add(1, Ordering::SeqCst);
            // Le o pedido inteiro (cabecalho + corpo) antes de responder.
            let mut buf = Vec::new();
            let mut pedaco = [0u8; 4096];
            while let Ok(k) = s.read(&mut pedaco) {
                if k == 0 {
                    break;
                }
                buf.extend_from_slice(&pedaco[..k]);
                let t = String::from_utf8_lossy(&buf);
                if let Some(i) = t.find("\r\n\r\n") {
                    let tam = t[..i]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if buf.len() >= i + 4 + tam {
                        break;
                    }
                }
            }
            let (status, corpo) = match c {
                Comportamento::Status(st) => (st, r#"{"error":"falso"}"#.to_string()),
                Comportamento::Responde => (
                    200,
                    json!({"message": {"role": "assistant", "content": "pronto"},
                           "prompt_eval_count": 3, "eval_count": 2})
                    .to_string(),
                ),
                Comportamento::Demora(d) => {
                    std::thread::sleep(d);
                    (200, "{}".to_string())
                }
            };
            let _ = write!(
                s,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
                corpo.len()
            );
        }
    });
    (url, n)
}

/// Politica com os provedores `ollama:<nome>` e a fabrica que os aponta para os servidores.
fn rota(
    servidores: &[(&str, Comportamento)],
    extra: serde_json::Value,
) -> (LlmRoteado, BTreeMap<String, Arc<AtomicUsize>>) {
    let mut urls = BTreeMap::new();
    let mut contas = BTreeMap::new();
    for (nome, c) in servidores {
        let (u, n) = servidor(*c);
        urls.insert(format!("ollama:{nome}"), u);
        contas.insert(format!("ollama:{nome}"), n);
    }
    let mut p = json!({
        "provedores": servidores.iter().map(|(n, _)| json!({"spec": format!("ollama:{n}")})).collect::<Vec<_>>(),
    });
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    let politica = Politica::de_json(&p.to_string()).unwrap();
    let r = LlmRoteado::montar(politica, None, |s| {
        let modelo = s.split_once(':').unwrap().1;
        Ok(Arc::new(
            OllamaLlm::with_timeout(&urls[s], modelo, Duration::from_millis(700))
                .map_err(|e| e.to_string())?,
        ) as Arc<dyn Llm>)
    })
    .unwrap();
    (r, contas)
}

async fn chamar(r: &LlmRoteado) -> (Result<LlmReply, LlmError>, Vec<Atendimento>) {
    let msgs = [Message::user("oi")];
    com_diario(r.chat(&msgs, &[], &LlmOptions::default())).await
}

fn desfechos(a: &Atendimento) -> Vec<&str> {
    a.tentativas.iter().map(|t| t.desfecho.as_str()).collect()
}

// ------------------------------------------------------------------ troca e registro

#[tokio::test]
async fn em_429_e_5xx_troca_e_registra_quem_atendeu() {
    let (r, contas) = rota(
        &[
            ("a", Comportamento::Status(429)),
            ("b", Comportamento::Status(503)),
            ("c", Comportamento::Responde),
        ],
        json!({}),
    );
    let (res, diario) = chamar(&r).await;
    assert_eq!(res.unwrap().content, "pronto");
    assert_eq!(diario.len(), 1, "uma chamada, um registro: {diario:?}");
    let a = &diario[0];
    assert_eq!(desfechos(a), ["429", "5xx", "ok"], "{a:?}");
    assert_eq!(a.atendeu.as_deref(), Some("ollama:c"));
    assert!(a.resumo().contains("ollama:a -> 429"), "{}", a.resumo());
    for s in ["ollama:a", "ollama:b", "ollama:c"] {
        assert_eq!(contas[s].load(Ordering::SeqCst), 1, "{s}");
    }
}

#[tokio::test]
async fn prazo_estourado_do_provedor_troca() {
    let (r, _) = rota(
        &[
            ("lento", Comportamento::Demora(Duration::from_secs(3))),
            ("b", Comportamento::Responde),
        ],
        json!({}),
    );
    let (res, diario) = chamar(&r).await;
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(desfechos(&diario[0]), ["timeout", "ok"], "{diario:?}");
}

#[tokio::test]
async fn erro_de_argumento_nao_troca() {
    let (r, contas) = rota(
        &[
            ("a", Comportamento::Status(400)),
            ("b", Comportamento::Responde),
        ],
        json!({}),
    );
    let (res, diario) = chamar(&r).await;
    assert!(
        matches!(res, Err(LlmError::Api { status: 400, .. })),
        "o 400 tem de chegar a quem chamou: {res:?}"
    );
    assert_eq!(
        contas["ollama:b"].load(Ordering::SeqCst),
        0,
        "a reserva nao pode ser chamada"
    );
    let a = &diario[0];
    assert_eq!(desfechos(a), ["nao_troca"]);
    assert!(a.parada.as_deref().unwrap().contains("4xx"), "{a:?}");
    assert!(a.atendeu.is_none());
}

#[tokio::test]
async fn teto_de_trocas_para_a_cadeia() {
    let (r, contas) = rota(
        &[
            ("a", Comportamento::Status(500)),
            ("b", Comportamento::Status(502)),
            ("c", Comportamento::Status(503)),
            ("d", Comportamento::Responde),
        ],
        json!({"trocas_max": 2}),
    );
    let (res, diario) = chamar(&r).await;
    assert!(
        matches!(res, Err(LlmError::Api { status: 503, .. })),
        "{res:?}"
    );
    let a = &diario[0];
    assert_eq!(desfechos(a), ["5xx", "5xx", "5xx"], "{a:?}");
    assert_eq!(
        contas["ollama:d"].load(Ordering::SeqCst),
        0,
        "passou do teto"
    );
    assert!(a.parada.as_deref().unwrap().contains("teto de 2"), "{a:?}");
}

#[tokio::test]
async fn falha_fora_de_trocar_em_nao_troca() {
    let (r, contas) = rota(
        &[
            ("a", Comportamento::Status(429)),
            ("b", Comportamento::Responde),
        ],
        json!({"trocar_em": ["5xx"]}),
    );
    let (res, diario) = chamar(&r).await;
    assert!(
        matches!(res, Err(LlmError::Api { status: 429, .. })),
        "{res:?}"
    );
    assert_eq!(contas["ollama:b"].load(Ordering::SeqCst), 0);
    assert!(diario[0].parada.as_deref().unwrap().contains("trocar_em"));
}

// ------------------------------------------------------------------ dublê do trait

type Passo = Box<dyn FnOnce() -> Result<LlmReply, LlmError> + Send>;

/// Provedor de roteiro: cada chamada consome um item; erro por fabrica (o `LlmError` nao
/// e `Clone`).
struct Dubla {
    id: String,
    roteiro: std::sync::Mutex<Vec<Passo>>,
    chamadas: AtomicUsize,
}

impl Dubla {
    fn nova(id: &str, mut r: Vec<Passo>) -> Arc<Self> {
        r.reverse();
        Arc::new(Self {
            id: id.into(),
            roteiro: std::sync::Mutex::new(r),
            chamadas: AtomicUsize::new(0),
        })
    }
}

impl Llm for Dubla {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn chat<'a>(
        &'a self,
        _m: &'a [Message],
        _t: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            let f = self.roteiro.lock().unwrap().pop();
            f.map_or_else(|| Err(LlmError::Parse("roteiro acabou".into())), |f| f())
        })
    }
}

fn texto(t: &'static str) -> Passo {
    Box::new(move || Ok(ScriptedLlm::text(t)))
}

fn erro_500() -> Passo {
    Box::new(|| {
        Err(LlmError::Api {
            status: 500,
            body: "falso".into(),
        })
    })
}

fn montar_com(
    politica: serde_json::Value,
    cabeca: Option<&str>,
    dublas: &[Arc<Dubla>],
) -> LlmRoteado {
    let mapa: BTreeMap<String, Arc<Dubla>> =
        dublas.iter().map(|d| (d.id.clone(), d.clone())).collect();
    LlmRoteado::montar(
        Politica::de_json(&politica.to_string()).unwrap(),
        cabeca,
        |s| {
            mapa.get(s)
                .map(|d| d.clone() as Arc<dyn Llm>)
                .ok_or_else(|| format!("sem dubla para {s}"))
        },
    )
    .unwrap()
}

#[tokio::test]
async fn politica_negada_nao_troca() {
    let a = Dubla::nova(
        "x:a",
        vec![Box::new(|| {
            Err(LlmError::Denied("resposta acima do teto".into()))
        })],
    );
    let b = Dubla::nova("x:b", vec![texto("nao devia")]);
    let r = montar_com(
        json!({"provedores": [{"spec": "x:a"}, {"spec": "x:b"}]}),
        None,
        &[a, b.clone()],
    );
    let (res, d) = chamar(&r).await;
    assert!(matches!(res, Err(LlmError::Denied(_))), "{res:?}");
    assert_eq!(b.chamadas.load(Ordering::SeqCst), 0);
    assert_eq!(desfechos(&d[0]), ["nao_troca"]);
}

#[tokio::test]
async fn o_motor_grava_passo_e_evidencia_de_quem_atendeu() {
    let a = Dubla::nova("x:a", vec![erro_500()]);
    let b = Dubla::nova("x:b", vec![texto("feito")]);
    let r = Arc::new(montar_com(
        json!({"provedores": [{"spec": "x:a"}, {"spec": "x:b"}]}),
        Some("x:a"),
        &[a, b],
    ));
    assert_eq!(r.id(), "rota:x:a", "o id volta a montagem pela retomada");
    let dir = tmp("motor");
    let store = TaskStore::new(&dir).unwrap();
    let agente = Agent::new(r.clone(), vec![], AgentConfig::default(), store.clone());
    let t = agente
        .run(
            Task::new("diga feito", r.id()),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let passo = t
        .steps
        .iter()
        .find(|p| p.kind == "modelo")
        .expect("o passo `modelo` tem de existir");
    assert_eq!(passo.outcome, "trocou");
    assert!(passo.summary.contains("atendeu x:b"), "{}", passo.summary);
    assert!(passo.summary.contains("x:a -> 5xx"), "{}", passo.summary);
    let ev = std::fs::read_to_string(store.evidence_path(&t.id)).unwrap();
    assert!(
        ev.lines()
            .any(|l| l.contains("modelo.roteamento") && l.contains("x:b")),
        "{ev}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------ tipo da tarefa (R1)

fn politica_por_tipo(classificar: serde_json::Value) -> serde_json::Value {
    json!({
        "provedores": [{"spec": "x:barato", "custo": 0}, {"spec": "x:forte", "custo": 5}],
        "por_tipo": {"codigo": ["x:forte", "x:barato"], "geral": ["x:barato", "x:forte"]},
        "classificar": classificar,
    })
}

#[tokio::test]
async fn o_tipo_da_tarefa_escolhe_a_cadeia_e_o_incerto_cai_no_padrao() {
    let regras = json!({
        "regras": [{"palavras": ["cargo"], "valor": "codigo", "confianca": 0.9}],
        "padrao": "geral",
    });
    let barato = Dubla::nova("x:barato", vec![texto("b")]);
    let forte = Dubla::nova("x:forte", vec![texto("f")]);
    let r = montar_com(politica_por_tipo(regras.clone()), None, &[barato, forte]);
    let msgs = [Message::user("rode o Cargo test")];
    let (res, d) = com_diario(r.chat(&msgs, &[], &LlmOptions::default())).await;
    assert_eq!(res.unwrap().content, "f");
    assert_eq!(d[0].tipo.as_deref(), Some("codigo"));
    assert!(d[0].tipo_por.starts_with("regras:"), "{:?}", d[0]);
    assert_eq!(d[0].cadeia, ["x:forte", "x:barato"]);

    let barato = Dubla::nova("x:barato", vec![texto("b")]);
    let forte = Dubla::nova("x:forte", vec![texto("f")]);
    let r = montar_com(politica_por_tipo(regras), None, &[barato, forte]);
    let msgs = [Message::user("bom dia")];
    let (_, d) = com_diario(r.chat(&msgs, &[], &LlmOptions::default())).await;
    assert_eq!(d[0].tipo.as_deref(), Some("geral"));
    assert!(d[0].tipo_por.starts_with("padrao"), "{:?}", d[0]);
}

#[tokio::test]
async fn classificador_de_modelo_fora_das_opcoes_nao_escolhe_o_tipo() {
    let classificar = json!({"modelo": "x:juiz", "padrao": "geral"});
    // O juiz responde uma opcao que nao existe com confianca alta: sem decisao, padrao.
    let juiz = Dubla::nova(
        "x:juiz",
        vec![texto(r#"{"value": "programacao", "confidence": 0.99}"#)],
    );
    let barato = Dubla::nova("x:barato", vec![texto("b")]);
    let forte = Dubla::nova("x:forte", vec![texto("f")]);
    let r = montar_com(
        politica_por_tipo(classificar),
        None,
        &[juiz.clone(), barato, forte],
    );
    let msgs = [Message::user("escreva uma funcao")];
    let (res, d) = com_diario(r.chat(&msgs, &[], &LlmOptions::default())).await;
    assert_eq!(res.unwrap().content, "b");
    assert_eq!(d[0].tipo.as_deref(), Some("geral"), "{:?}", d[0]);
    // Lembrado por objetivo: a segunda volta do laco nao pergunta ao juiz de novo.
    let _ = com_diario(r.chat(&msgs, &[], &LlmOptions::default())).await;
    assert_eq!(juiz.chamadas.load(Ordering::SeqCst), 1);
}

#[test]
fn politica_confere_o_que_le() {
    for (ruim, onde) in [
        (json!({"provedores": []}), "vazia"),
        (
            json!({"provedores": [{"spec": "x:a"}], "trocas": 3}),
            "unknown field",
        ),
        (
            json!({"provedores": [{"spec": "x:a"}], "trocas_max": 99}),
            "teto",
        ),
        (
            json!({"provedores": [{"spec": "x:a"}], "trocar_em": ["404"]}),
            "trocar_em",
        ),
        (
            json!({"provedores": [{"spec": "x:a"}], "por_tipo": {"c": ["x:z"]}}),
            "nao esta em 'provedores'",
        ),
        (
            json!({"provedores": [{"spec": "x:a"}], "por_tipo": {"c": ["x:a"]},
                   "classificar": {"regras": [{"palavras": ["k"], "valor": "d", "confianca": 1}]}}),
            "chave de 'por_tipo'",
        ),
        (json!({"provedores": [{"spec": "rota:x"}]}), "spec invalido"),
    ] {
        let e = Politica::de_json(&ruim.to_string()).unwrap_err();
        assert!(e.contains(onde), "{ruim}: {e}");
    }
    let p = Politica::de_json(
        &json!({"provedores": [{"spec": "x:caro", "custo": 3}, {"spec": "x:a", "custo": 1}, {"spec": "x:b", "custo": 1}],
                "preferir_barato": true})
        .to_string(),
    )
    .unwrap();
    assert_eq!(p.cadeia(None, None), ["x:a", "x:b", "x:caro"]);
    assert_eq!(p.cadeia(Some("x:b"), None), ["x:b", "x:a", "x:caro"]);
}

// ------------------------------------------------------------------ montagem

#[test]
fn a_montagem_so_roteia_quem_pede_e_diz_o_que_falta() {
    let dir = tmp("montagem");
    iniciar(&dir).unwrap();
    let m = Montagem::new(TaskStore::new(dir.join("tasks")).unwrap());
    let e = m
        .agent("rota")
        .err()
        .expect("rota sem politica tem de falhar");
    assert!(e.contains("modelo.roteamento"), "{e}");

    let arq = dir.join("rota.json");
    std::fs::write(
        &arq,
        json!({"provedores": [{"spec": "ollama:qwen2.5:3b"}]}).to_string(),
    )
    .unwrap();
    let mut v = serde_json::Map::new();
    v.insert("modelo.roteamento".into(), json!(arq.to_string_lossy()));
    definir(&dir, Escopo::Pasta, &v, None).unwrap();
    let a = m.agent("rota:ollama:qwen2.5:1.5b").unwrap();
    assert_eq!(a.llm.id(), "rota:ollama:qwen2.5:1.5b");
    // Quem nao pede rota continua indo direto ao provedor.
    assert_eq!(
        m.agent("ollama:qwen2.5:1.5b").unwrap().llm.id(),
        "ollama:qwen2.5:1.5b"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------ o classificador cobra (M3)

/// M3: o classificador de modelo gasta DENTRO da chamada do laco, e o uso dele nao esta na
/// resposta que o motor cobra. Ele cobra a conta da tarefa por si: com teto de 20 tokens,
/// juiz (15) + resposta (15) = 30 para a tarefa em `budget_exceeded`, e o gasto e o uso
/// dizem 30.
///
/// RED medido: em `LlmRoteado::montar_com`, a linha
/// `let llm = crate::orcamento::LlmDaTarefa::por_dentro(llm);` apagada (`// REPOSTO`) -- a
/// tarefa terminou `Completed` com gasto de 15 tokens, e a asserção do estado caiu.
#[tokio::test]
async fn o_classificador_de_modelo_cobra_a_conta_da_tarefa() {
    let juiz = Dubla::nova(
        "x:juiz",
        vec![texto(r#"{"value": "geral", "confidence": 0.99}"#)],
    );
    let barato = Dubla::nova("x:barato", vec![texto("feito")]);
    let forte = Dubla::nova("x:forte", vec![]);
    let r = Arc::new(montar_com(
        politica_por_tipo(json!({"modelo": "x:juiz", "padrao": "geral"})),
        None,
        &[juiz.clone(), barato, forte],
    ));
    let dir = tmp("classificador");
    let agente = Agent::new(
        r.clone(),
        vec![],
        AgentConfig::default(),
        TaskStore::new(&dir).unwrap(),
    );
    let mut t = Task::new("diga feito", r.id());
    t.orcamento = Some(phxclaw_agent_core::tarefa::Orcamento {
        tokens: Some(20),
        custo: None,
    });
    let t = agente.run(t, &CancelFlag::default(), &NoObserver).await;
    assert_eq!(juiz.chamadas.load(Ordering::SeqCst), 1);
    assert_eq!(t.status, TaskStatus::BudgetExceeded, "{:?}", t.error);
    assert!(
        t.error
            .as_deref()
            .unwrap()
            .contains("gastou 30 tokens, teto de 20 tokens"),
        "{:?}",
        t.error
    );
    assert_eq!(t.gasto.as_ref().unwrap().tokens, 30);
    assert_eq!(
        (t.usage.input_tokens, t.usage.output_tokens),
        (20, 10),
        "o uso da tarefa inclui o do juiz"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// M3: quem pode GASTAR numa chamada inclui o modelo do classificador; sem preco para ele,
/// orcamento em dinheiro nao se confere e a criacao recusa dizendo qual.
///
/// RED medido: `provedores()` do `LlmRoteado` de volta a `self.modelos.keys()` (sem o
/// classificador, `// REPOSTO`) -- a lista saiu `["x:barato", "x:forte"]`, sem o juiz, e a
/// asserção caiu.
#[test]
fn o_classificador_entra_nos_provedores_e_o_dinheiro_exige_preco_dele() {
    let r = montar_com(
        politica_por_tipo(json!({"modelo": "x:juiz", "padrao": "geral"})),
        None,
        &[
            Dubla::nova("x:juiz", vec![]),
            Dubla::nova("x:barato", vec![]),
            Dubla::nova("x:forte", vec![]),
        ],
    );
    let p = r.provedores();
    assert!(p.contains(&"x:juiz".to_string()), "{p:?}");
    let tab = phxclaw_agent::custo::TabelaDePrecos::de_texto(
        r#"{"moeda":"USD","modelos":{
        "x:barato":{"entrada":1,"saida":1,"data":"2026-10-09","fonte":"prova"},
        "x:forte":{"entrada":5,"saida":5,"data":"2026-10-09","fonte":"prova"}}}"#,
    )
    .unwrap();
    let din = phxclaw_agent_core::tarefa::Orcamento {
        tokens: None,
        custo: Some(1.0),
    };
    let e = phxclaw_agent::orcamento::conferir_preco(Some(&din), Some(&tab), &p).unwrap_err();
    assert!(e.contains("x:juiz não tem preço"), "{e}");
}

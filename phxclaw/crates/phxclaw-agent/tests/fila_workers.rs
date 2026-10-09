//! O modo fila (`fila.rs`) contra um PostgreSQL DE VERDADE (`phxclaw_test_support::pg`):
//! dois workers nunca levam a mesma execucao, worker morto no meio e retomado por outro
//! depois do prazo (o que terminou nao roda de novo), a posse vencida nao fecha o run de
//! outro, e o resultado pela fila e o mesmo do modo normal.
//!
//! Sem `initdb` na maquina, cada teste registra o pulo pelo `pulado` (nunca verde calado).

use phxclaw_agent::api::{AgentFactory, ApiState, Limite};
use phxclaw_agent::fila::{self, Fila, Worker};
use phxclaw_agent::*;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_task_graph::{PostgresTaskJournal, QueueClaim, RunOutcome, TaskStatus as Linha};
use phxclaw_test_support::pg::PgEfemero;
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// O banco efemero com a migracao 0004 aplicada, ou o motivo de nao haver (quem chama
/// registra o pulo).
fn banco() -> Result<PgEfemero, String> {
    let pg = PgEfemero::subir()?;
    let m = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/0004_task_graph.sql");
    pg.aplicar(&m).expect("migracao 0004");
    Ok(pg)
}

fn cliente(pg: &PgEfemero) -> postgres::Client {
    postgres::Client::connect(&pg.url(), postgres::NoTls).unwrap()
}

/// O cliente bloqueante do `postgres` sobe o proprio runtime e entra em panico dentro de uma
/// tarefa do tokio: o teste conversa com o banco numa thread propria.
fn fora<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| s.spawn(f).join().unwrap())
}

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-fila-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `marca`: registra o `texto` de cada chamada (na ordem) e dorme `dorme_ms`. O registro e
/// do teste inteiro: os agentes que a fabrica monta (um por execucao) escrevem no mesmo.
struct Marca(Arc<Mutex<Vec<String>>>);

impl Tool for Marca {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "marca".into(),
            description: "marca".into(),
            parameters: json!({"type": "object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _c: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let t = args
                .get("texto")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            self.0.lock().unwrap().push(t.clone());
            if let Some(ms) = args.get("dorme_ms").and_then(Value::as_u64) {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            Ok(ToolOutput::text(t))
        })
    }
}

fn estado(raiz: &Path, marcas: Arc<Mutex<Vec<String>>>) -> ApiState {
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(Marca(marcas.clone()))];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")])),
            tools,
            AgentConfig::default().grant(&["fs.read"]),
            st.clone(),
        ))
    });
    ApiState {
        usuarios: Default::default(),
        store,
        factory,
        default_model: "roteiro".into(),
        token: "token-de-teste-com-tamanho-suficiente".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(10_000)),
    }
}

fn gravar_fluxo(raiz: &Path, v: Value) -> String {
    let p = raiz.join("fluxo.json");
    std::fs::write(&p, v.to_string()).unwrap();
    p.to_string_lossy().into_owned()
}

fn fila_de(pg: &PgEfemero, prazo: Duration) -> Arc<Fila> {
    let mut f = Fila::nova(&pg.url_sem_senha(), Some(&pg.senha)).unwrap();
    f.prazo = prazo;
    f.conferir_esquema().unwrap();
    Arc::new(f)
}

async fn ate(s: &ApiState, id: &str, pronto: impl Fn(&Task) -> bool, segundos: u64) -> Task {
    for _ in 0..segundos * 20 {
        if let Ok(t) = s.store.load(id)
            && pronto(&t)
        {
            return t;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("tarefa {id} nao chegou: {:#?}", s.store.load(id));
}

fn relatorio(t: &Task) -> phxclaw_agent::fluxos::Relatorio {
    serde_json::from_str(t.answer.as_deref().unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dois_workers_nao_pegam_a_mesma_execucao() {
    let pg = match banco() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    let raiz = tmp("dois");
    let marcas = Arc::new(Mutex::new(vec![]));
    let s = estado(&raiz, marcas.clone());
    let f = fila_de(&pg, Duration::from_secs(30));
    fila::ligar(s.store.root(), f.clone());

    // (1) A tomada que segura a linha (transacao aberta) e a segunda tomada, ao mesmo tempo:
    // a segunda PULA a linha travada (e a fila tem uma so) em vez de leva-la.
    let arq = gravar_fluxo(
        &raiz,
        json!({"nome": "um", "passos": [{"id": "a", "ferramenta": "marca", "args": {"texto": "x"}}]}),
    );
    phxclaw_agent::api::criar_fluxo_com(&s, &arq, vec![], |_| Ok(())).unwrap();
    fora(|| {
        let caps: Vec<String> = fila::CAPACIDADES.iter().map(|c| c.to_string()).collect();
        let mut a = cliente(&pg);
        let mut tx = a.transaction().unwrap();
        let primeira = PostgresTaskJournal::claim_in(&mut tx, "A", &caps, 30_000).unwrap();
        assert!(matches!(primeira, Some(QueueClaim::Run(_))), "{primeira:?}");
        let (url, caps2) = (pg.url(), caps.clone());
        let (envia, recebe) = std::sync::mpsc::channel();
        let b = std::thread::spawn(move || {
            let mut c = postgres::Client::connect(&url, postgres::NoTls).unwrap();
            let r = PostgresTaskJournal::claim(&mut c, "B", &caps2, 30_000);
            let _ = envia.send(r.map(|c| c.is_some()).map_err(|e| e.to_string()));
        });
        let segunda = recebe.recv_timeout(Duration::from_secs(2));
        tx.commit().unwrap();
        let segunda = segunda.or_else(|_| recebe.recv_timeout(Duration::from_secs(10)));
        b.join().unwrap();
        assert_eq!(
            segunda,
            Ok(Ok(false)),
            "a segunda tomada esperou a linha travada ou levou a mesma execucao (sem SKIP LOCKED)"
        );
        // A primeira posse continua dela: uma terceira tomada agora tambem nao a leva.
        assert!(
            PostgresTaskJournal::claim(&mut a, "C", &caps, 30_000)
                .unwrap()
                .is_none()
        );
        // Fecha a execucao tomada a mao, para a fila voltar vazia.
        if let Some(QueueClaim::Run(l)) = primeira {
            let ok = RunOutcome {
                status: Linha::Succeeded,
                result: json!({}),
                error: None,
            };
            assert!(PostgresTaskJournal::finish(&mut a, &l, &ok).unwrap());
        }
    });

    // (2) Doze execucoes, dois workers de tres vagas cada: cada uma tomada UMA vez.
    let mut ids = vec![];
    for i in 0..12 {
        let arq = gravar_fluxo(
            &raiz,
            json!({"nome": format!("n{i}"), "passos": [{"id": "a", "ferramenta": "marca", "args": {"texto": format!("e{i}"), "dorme_ms": 150}}]}),
        );
        ids.push(
            phxclaw_agent::api::criar_fluxo_com(&s, &arq, vec![], |_| Ok(()))
                .unwrap()
                .id,
        );
    }
    let (parar, rx) = tokio::sync::watch::channel(false);
    let mut ws = vec![];
    for nome in ["w1", "w2"] {
        let (st, f, rx) = (s.clone(), f.clone(), rx.clone());
        ws.push(tokio::spawn(fila::trabalhar(
            st,
            f,
            Worker {
                nome: nome.into(),
                concorrencia: 3,
                ocioso: Duration::from_millis(50),
            },
            rx,
            |_l: &str| {},
        )));
    }
    for id in &ids {
        let t = ate(&s, id, |t| t.status == TaskStatus::Completed, 30).await;
        assert!(relatorio(&t).sucesso, "{t:#?}");
    }
    parar.send(true).unwrap();
    for w in ws {
        w.await.unwrap();
    }
    fila::desligar(s.store.root());
    let tomadas: Vec<(String, i64)> = fora(|| {
        cliente(&pg)
        .query(
            "SELECT t.payload->>'tarefa', count(*) FROM phoenix_task_events e JOIN phoenix_tasks t ON t.uuid = e.task_uuid \
             WHERE e.event_type = 'claimed' GROUP BY 1",
            &[],
        )
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect()
    });
    for id in &ids {
        assert_eq!(
            tomadas.iter().find(|(t, _)| t == id).map(|x| x.1),
            Some(1),
            "a execucao {id} foi tomada mais de uma vez (ou nenhuma): {tomadas:?}"
        );
    }
    let marcas = marcas.lock().unwrap().clone();
    for i in 0..12 {
        assert_eq!(
            marcas.iter().filter(|m| **m == format!("e{i}")).count(),
            1,
            "a ferramenta da execucao {i} rodou mais de uma vez: {marcas:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&raiz);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worker_morto_no_meio_outro_retoma_depois_do_prazo() {
    let pg = match banco() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    let raiz = tmp("morto");
    let marcas = Arc::new(Mutex::new(vec![]));
    let s = estado(&raiz, marcas.clone());
    let f = fila_de(&pg, Duration::from_millis(1500));
    fila::ligar(s.store.root(), f.clone());
    let arq = gravar_fluxo(
        &raiz,
        json!({"nome": "lento", "passos": [
            {"id": "a", "ferramenta": "marca", "args": {"texto": "A"}},
            {"id": "b", "depende": ["a"], "ferramenta": "marca", "args": {"texto": "B", "dorme_ms": 4000}}
        ]}),
    );
    let id = phxclaw_agent::api::criar_fluxo_com(&s, &arq, vec![], |_| Ok(()))
        .unwrap()
        .id;
    // O worker 1 toma e roda; quando o passo `a` esta gravado e o `b` dorme, ele morre (a
    // execucao e o batimento caem juntos, como no SIGKILL: nada fecha o run).
    let (st, ff) = (s.clone(), f.clone());
    let w1 = tokio::spawn(async move { fila::trabalhar_uma(&st, &ff, "w1").await });
    ate(
        &s,
        &id,
        |t| {
            t.answer
                .as_deref()
                .and_then(|a| serde_json::from_str::<phxclaw_agent::fluxos::Relatorio>(a).ok())
                .is_some_and(|r| r.passos.iter().any(|p| p.id == "a" && p.estado == "ok"))
        },
        10,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    w1.abort();
    let _ = w1.await;
    // Antes do prazo, a posse ainda e do morto: o worker 2 nao leva nada.
    let cedo = fila::trabalhar_uma(&s, &f, "w2").await.unwrap();
    assert!(cedo.is_none(), "tomou antes do prazo vencer: {cedo:?}");
    tokio::time::sleep(Duration::from_millis(1700)).await;
    let feito = fila::trabalhar_uma(&s, &f, "w2")
        .await
        .unwrap()
        .expect("depois do prazo, o worker 2 retoma");
    assert!(feito.retomada && feito.aceito, "{feito:?}");
    assert_eq!(feito.estado, TaskStatus::Completed, "{feito:?}");
    let t = s.store.load(&id).unwrap();
    let r = relatorio(&t);
    assert!(r.sucesso, "{r:#?}");
    let m = marcas.lock().unwrap().clone();
    assert_eq!(
        m.iter().filter(|x| *x == "A").count(),
        1,
        "o passo que terminou antes da queda rodou de novo: {m:?}"
    );
    assert!(
        r.passos.iter().any(|p| p.id == "a" && p.reaproveitado),
        "{r:#?}"
    );
    // Na fila: dois runs, o do morto `failed` pela posse vencida e o da retomada `succeeded`.
    let runs: Vec<(i32, String, Option<String>)> = fora(|| {
        cliente(&pg)
        .query(
            "SELECT r.attempt, r.status, r.error FROM phoenix_task_runs r JOIN phoenix_tasks t ON t.uuid = r.task_uuid \
             WHERE t.payload->>'tarefa' = $1 ORDER BY r.attempt",
            &[&id],
        )
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect()
    });
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!((runs[0].0, runs[0].1.as_str()), (1, "failed"), "{runs:?}");
    assert!(
        runs[0].2.as_deref().unwrap_or("").contains("posse vencida"),
        "{runs:?}"
    );
    assert_eq!(
        (runs[1].0, runs[1].1.as_str()),
        (2, "succeeded"),
        "{runs:?}"
    );
    fila::desligar(s.store.root());
    let _ = std::fs::remove_dir_all(&raiz);
}

/// A cerca: a posse vencida nao bate nem fecha o run que outro tomou; e a execucao que
/// venceu vezes demais vai para `dead_letter` com a tarefa FALHA dizendo por que.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_posse_vencida_nao_fecha_o_run_de_outro_e_o_teto_vira_dead_letter() {
    let pg = match banco() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    let raiz = tmp("cerca");
    let s = estado(&raiz, Arc::new(Mutex::new(vec![])));
    let mut fl = Fila::nova(&pg.url_sem_senha(), Some(&pg.senha)).unwrap();
    fl.tentativas = 2;
    let f = Arc::new(fl);
    fila::ligar(s.store.root(), f.clone());
    let arq = gravar_fluxo(
        &raiz,
        json!({"nome": "c", "passos": [{"id": "a", "ferramenta": "marca", "args": {"texto": "x"}}]}),
    );
    let id = phxclaw_agent::api::criar_fluxo_com(&s, &arq, vec![], |_| Ok(()))
        .unwrap()
        .id;
    let caps: Vec<String> = fila::CAPACIDADES.iter().map(|c| c.to_string()).collect();
    let vencer = || {
        fora(|| {
            cliente(&pg)
                .execute(
                    "UPDATE phoenix_tasks SET next_eligible_at = now() - interval '1 second' WHERE status = 'running'",
                    &[],
                )
                .unwrap()
        })
    };
    let ok = RunOutcome {
        status: Linha::Succeeded,
        result: json!({}),
        error: None,
    };
    let l2 = fora(|| {
        let mut c = cliente(&pg);
        let Some(QueueClaim::Run(l1)) =
            PostgresTaskJournal::claim(&mut c, "A", &caps, 60_000).unwrap()
        else {
            panic!("sem execucao")
        };
        c.execute(
            "UPDATE phoenix_tasks SET next_eligible_at = now() - interval '1 second' WHERE status = 'running'",
            &[],
        )
        .unwrap();
        let Some(QueueClaim::Run(l2)) =
            PostgresTaskJournal::claim(&mut c, "B", &caps, 60_000).unwrap()
        else {
            panic!("a posse vencida nao voltou a fila")
        };
        assert!(l2.reclaimed && l2.attempt == 2, "{l2:?}");
        assert_eq!(
            PostgresTaskJournal::renew(&mut c, &l1, 60_000).unwrap(),
            phxclaw_task_graph::Heartbeat::Lost
        );
        assert!(
            !PostgresTaskJournal::finish(&mut c, &l1, &ok).unwrap(),
            "o run vencido fechou o de outro"
        );
        l2
    });
    // O teto: a segunda posse tambem vence, e a terceira tomada manda para dead_letter.
    vencer();
    let feito = fila::trabalhar_uma(&s, &f, "C")
        .await
        .unwrap()
        .expect("dead letter");
    assert_eq!(feito.estado, TaskStatus::Failed);
    let t = s.store.load(&id).unwrap();
    assert_eq!(t.status, TaskStatus::Failed);
    assert!(
        t.error.as_deref().unwrap_or("").contains("posse venceu"),
        "{t:#?}"
    );
    fora(|| {
        let mut c = cliente(&pg);
        let st: String = c
            .query_one(
                "SELECT status FROM phoenix_tasks WHERE payload->>'tarefa' = $1",
                &[&id],
            )
            .unwrap()
            .get(0);
        assert_eq!(st, "dead_letter");
        assert!(!PostgresTaskJournal::finish(&mut c, &l2, &ok).unwrap());
    });
    fila::desligar(s.store.root());
    let _ = std::fs::remove_dir_all(&raiz);
}

/// O resultado pela fila e o do modo normal: a mesma definicao, a mesma entrada, o mesmo
/// relatorio (passo a passo, itens e portas) e o mesmo estado final.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn o_resultado_pela_fila_e_o_mesmo_do_modo_normal() {
    let pg = match banco() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    let def = json!({"nome": "igual", "passos": [
        {"id": "a", "ferramenta": "marca", "args": {"texto": "{\"n\": 7}"}},
        {"id": "b", "depende": ["a"], "ferramenta": "marca", "args": {"texto": "{{entrada}} + {{a.n}}"}},
        {"id": "se", "depende": ["a"], "se": {"caminho": "n", "operador": "maior", "valor": 5}},
        {"id": "sim", "depende": ["se:verdadeiro"], "ferramenta": "marca", "args": {"texto": "grande"}},
        {"id": "nao", "depende": ["se:falso"], "ferramenta": "marca", "args": {"texto": "pequeno"}}
    ]});
    let entrada = vec![json!({"k": "v"})];
    // Modo normal.
    let r1 = tmp("igual-normal");
    let s1 = estado(&r1, Arc::new(Mutex::new(vec![])));
    let a1 = gravar_fluxo(&r1, def.clone());
    let c1 = phxclaw_agent::api::criar_fluxo_com(&s1, &a1, entrada.clone(), |_| Ok(())).unwrap();
    let t1 = c1.fim.await.unwrap();
    // Modo fila.
    let r2 = tmp("igual-fila");
    let s2 = estado(&r2, Arc::new(Mutex::new(vec![])));
    let f = fila_de(&pg, Duration::from_secs(30));
    fila::ligar(s2.store.root(), f.clone());
    let a2 = gravar_fluxo(&r2, def);
    let c2 = phxclaw_agent::api::criar_fluxo_com(&s2, &a2, entrada, |_| Ok(())).unwrap();
    let feito = fila::trabalhar_uma(&s2, &f, "w")
        .await
        .unwrap()
        .expect("execucao na fila");
    assert!(feito.aceito && !feito.retomada, "{feito:?}");
    let t2 = c2.fim.await.unwrap();
    fila::desligar(s2.store.root());
    assert_eq!(t1.status, t2.status);
    let (x, y) = (relatorio(&t1), relatorio(&t2));
    assert_eq!(x.fluxo_sha256, y.fluxo_sha256);
    assert_eq!(x.sucesso, y.sucesso);
    let resumo = |r: &phxclaw_agent::fluxos::Relatorio| {
        r.passos
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    p.estado.clone(),
                    p.saida.clone(),
                    p.itens.clone(),
                    p.portas.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(resumo(&x), resumo(&y));
    assert!(
        resumo(&y)
            .iter()
            .any(|p| p.0 == "b" && p.2.contains("\"k\"")),
        "{y:#?}"
    );
    let _ = std::fs::remove_dir_all(&r1);
    let _ = std::fs::remove_dir_all(&r2);
}

/// Cancelar a execucao que ainda nao saiu da fila: ela sai `cancelled` e ninguem a toma.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelar_antes_de_comecar_tira_da_fila() {
    let pg = match banco() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    let raiz = tmp("cancela");
    let s = estado(&raiz, Arc::new(Mutex::new(vec![])));
    let f = fila_de(&pg, Duration::from_secs(30));
    fila::ligar(s.store.root(), f.clone());
    let arq = gravar_fluxo(
        &raiz,
        json!({"nome": "c", "passos": [{"id": "a", "ferramenta": "marca", "args": {"texto": "x"}}]}),
    );
    let id = phxclaw_agent::api::criar_fluxo_com(&s, &arq, vec![], |_| Ok(()))
        .unwrap()
        .id;
    let t = s.store.load(&id).unwrap();
    assert_eq!(
        fila::cancelar(&f, &t).unwrap(),
        fila::Cancelamento::AntesDeComecar
    );
    assert!(fila::trabalhar_uma(&s, &f, "w").await.unwrap().is_none());
    fila::desligar(s.store.root());
    let _ = std::fs::remove_dir_all(&raiz);
}

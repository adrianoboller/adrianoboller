//! E2E contra PostgreSQL real, como papel sujeito a RLS (NOSUPERUSER, NOBYPASSRLS).
//!
//! Roda so quando pedido: `PHXCLAW_E2E_RLS_URL=... cargo test -p phxclaw-hypothesis
//! --test pg_e2e -- --ignored`. O `tests/postgres/run_e2e.sh` prepara o banco e o papel.
//! Ignorado por padrao em vez de "passar vazio": sem banco, o placar mostra que nao rodou.

use chrono::Utc;
use phxclaw_hypothesis::{ExperimentRunResult, HypothesisCore, HypothesisPersistenceContext};
use phxclaw_types::{EvidenceRef, HypothesisStatus, new_uuid_v7};
use postgres::{Client, NoTls};
use std::sync::{Arc, Barrier};
use uuid::Uuid;

const TENANT_A: &str = "0190e2e0-0000-7000-8000-00000000000a";
const TENANT_B: &str = "0190e2e0-0000-7000-8000-00000000000b";

fn url() -> String {
    std::env::var("PHXCLAW_E2E_RLS_URL").expect("PHXCLAW_E2E_RLS_URL nao definido")
}

fn conectar() -> Client {
    let mut c = Client::connect(&url(), NoTls).expect("conexao PostgreSQL");
    let papel = c
        .query_one(
            "SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .unwrap();
    let (su, bypass): (bool, bool) = (papel.get(0), papel.get(1));
    assert!(
        !su && !bypass,
        "prova de RLS exige papel sem superuser/bypassrls"
    );
    c
}

fn ctx(tenant: Uuid) -> HypothesisPersistenceContext {
    HypothesisPersistenceContext {
        tenant_uuid: tenant,
        research_uuid: None,
        source_state_sha256: "ab".repeat(32),
    }
}

fn status_no_banco(c: &mut Client, tenant: Uuid, h: Uuid) -> Option<String> {
    c.query_one(
        "SELECT set_config('phxclaw.tenant_uuid',$1,false)",
        &[&tenant.to_string()],
    )
    .unwrap();
    c.query_opt(
        "SELECT current_status FROM phxclaw.hypothesis_records WHERE hypothesis_uuid=$1",
        &[&h],
    )
    .unwrap()
    .map(|r| r.get(0))
}

#[test]
#[ignore = "requer PostgreSQL: PHXCLAW_E2E_RLS_URL"]
fn ciclo_completo_persistido_e_isolado_por_tenant() {
    let a: Uuid = TENANT_A.parse().unwrap();
    let b: Uuid = TENANT_B.parse().unwrap();
    let core = HypothesisCore;
    let mut c = conectar();

    let h = core.propose(
        "indice parcial reduz p95",
        "medido em bancada",
        vec!["rodar bancada".into()],
        vec!["p95 cai".into()],
        vec!["p95 sobe".into()],
    );
    core.persist(&mut c, &ctx(a), &h).unwrap();
    assert_eq!(
        status_no_banco(&mut c, a, h.uuid).as_deref(),
        Some("proposed")
    );

    core.record_decision(
        &mut c,
        a,
        h.uuid,
        HypothesisStatus::Proposed,
        HypothesisStatus::Testing,
        "abrir experimento",
        "e2e",
    )
    .unwrap();
    let run = core.start_experiment(&mut c, a, h.uuid).unwrap();
    core.finish_experiment(
        &mut c,
        a,
        &ExperimentRunResult {
            run_uuid: run.run_uuid,
            success: Some(true),
            result_summary: serde_json::json!({"p95_ms": 12}),
            evidence: vec![EvidenceRef {
                uuid: new_uuid_v7(),
                uri: "bench://e2e".into(),
                source_type: "benchmark".into(),
                retrieved_at: Utc::now(),
                sha256: "cd".repeat(32),
                notes: None,
            }],
            completed_at: Utc::now(),
        },
    )
    .unwrap();
    core.record_decision(
        &mut c,
        a,
        h.uuid,
        HypothesisStatus::Testing,
        HypothesisStatus::Supported,
        "p95 caiu",
        "e2e",
    )
    .unwrap();
    assert_eq!(
        status_no_banco(&mut c, a, h.uuid).as_deref(),
        Some("supported")
    );

    let estado: String = c
        .query_one(
            "SELECT state FROM phxclaw.experiment_runs WHERE run_uuid=$1",
            &[&run.run_uuid],
        )
        .unwrap()
        .get(0);
    assert_eq!(estado, "completed");

    // Tenant B nao enxerga nada do A, pela GUC que o motor usa.
    assert_eq!(status_no_banco(&mut c, b, h.uuid), None, "vazamento de RLS");
}

#[test]
#[ignore = "requer PostgreSQL: PHXCLAW_E2E_RLS_URL"]
fn decisoes_concorrentes_so_uma_vence() {
    let a: Uuid = TENANT_A.parse().unwrap();
    let core = HypothesisCore;
    let mut c = conectar();
    let h = core.propose("x", "y", vec![], vec![], vec![]);
    core.persist(&mut c, &ctx(a), &h).unwrap();
    core.record_decision(
        &mut c,
        a,
        h.uuid,
        HypothesisStatus::Proposed,
        HypothesisStatus::Testing,
        "abrir",
        "e2e",
    )
    .unwrap();

    // Dois revisores decidem ao mesmo tempo a partir de Testing, em sentidos opostos.
    let largada = Arc::new(Barrier::new(2));
    let alvos = [HypothesisStatus::Supported, HypothesisStatus::Rejected];
    let fios: Vec<_> = alvos
        .into_iter()
        .map(|alvo| {
            let largada = Arc::clone(&largada);
            let id = h.uuid;
            std::thread::spawn(move || {
                let mut c = conectar();
                largada.wait();
                HypothesisCore
                    .record_decision(
                        &mut c,
                        a,
                        id,
                        HypothesisStatus::Testing,
                        alvo,
                        "concorrente",
                        "e2e",
                    )
                    .is_ok()
            })
        })
        .collect();
    let vencedores = fios
        .into_iter()
        .map(|f| f.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(
        vencedores, 1,
        "as duas decisoes partiram de Testing; so uma pode ter acontecido"
    );

    c.query_one(
        "SELECT set_config('phxclaw.tenant_uuid',$1,false)",
        &[&a.to_string()],
    )
    .unwrap();
    let registradas: i64 = c
        .query_one(
            "SELECT count(*) FROM phxclaw.hypothesis_decisions WHERE hypothesis_uuid=$1 AND from_status='testing'",
            &[&h.uuid],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        registradas, 1,
        "o diario nao pode registrar a decisao perdedora"
    );
}

#[test]
#[ignore = "requer PostgreSQL: PHXCLAW_E2E_RLS_URL"]
fn experimento_nao_termina_duas_vezes() {
    let a: Uuid = TENANT_A.parse().unwrap();
    let core = HypothesisCore;
    let mut c = conectar();
    let h = core.propose("x", "y", vec![], vec![], vec![]);
    core.persist(&mut c, &ctx(a), &h).unwrap();
    let run = core.start_experiment(&mut c, a, h.uuid).unwrap();
    let res = |ok| ExperimentRunResult {
        run_uuid: run.run_uuid,
        success: Some(ok),
        result_summary: serde_json::json!({}),
        evidence: vec![],
        completed_at: Utc::now(),
    };
    core.finish_experiment(&mut c, a, &res(true)).unwrap();
    assert!(
        core.finish_experiment(&mut c, a, &res(false)).is_err(),
        "segundo fecho de um experimento ja concluido tem de ser recusado, nao ignorado"
    );
}

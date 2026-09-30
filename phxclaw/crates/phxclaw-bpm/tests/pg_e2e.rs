//! E2E do BpmStore contra PostgreSQL real, como papel sujeito a RLS.
//! `PHXCLAW_E2E_RLS_URL=... cargo test -p phxclaw-bpm --test pg_e2e -- --ignored`

use phxclaw_bpm::{BpmStore, PersistentBpmError};
use phxclaw_types::new_uuid_v7;
use postgres::{Client, NoTls};
use uuid::Uuid;

const TENANT: &str = "0190e2e0-0000-7000-8000-0000000000b1";

#[test]
#[ignore = "requer PostgreSQL: PHXCLAW_E2E_RLS_URL"]
fn token_nasce_e_reivindicado_avanca_e_cerca_lease_velho() {
    let url = std::env::var("PHXCLAW_E2E_RLS_URL").expect("PHXCLAW_E2E_RLS_URL");
    let mut c = Client::connect(&url, NoTls).unwrap();
    let tenant: Uuid = TENANT.parse().unwrap();
    let processo = new_uuid_v7();
    c.batch_execute(&format!(
        "SELECT set_config('phxclaw.tenant_uuid','{tenant}',false)"
    ))
    .unwrap();
    c.execute(
        "INSERT INTO phxclaw.bpm_process_definitions(process_uuid,tenant_uuid,version,bpmn_xml,compiled_graph,content_sha256) VALUES($1,$2,1,'<definitions/>','{}'::jsonb,repeat('a',64))",
        &[&processo, &tenant],
    )
    .unwrap();

    let mut store = BpmStore::new(&mut c);
    let (_instancia, token) = store
        .start_instance(tenant, processo, "inicio", serde_json::json!({}))
        .unwrap();
    // claim_token: com $7 sem tipo, o PostgreSQL recusava o PREPARE e isto falhava sempre.
    let lease = store.claim_ready_token(tenant, "worker-e2e", 30).unwrap();
    assert_eq!(lease.token_uuid, token);
    assert_eq!(lease.fencing_token, 1);
    store
        .advance_token(tenant, &lease, "fim", serde_json::json!({"passo": 1}))
        .unwrap();

    // Novo claim sobe o fencing; o lease anterior deixa de valer, o novo completa.
    let novo = store.claim_ready_token(tenant, "worker-e2e-2", 30).unwrap();
    assert_eq!(novo.fencing_token, 2);
    assert!(matches!(
        store.complete_token(tenant, &lease, serde_json::json!({})),
        Err(PersistentBpmError::StaleLease)
    ));
    store
        .complete_token(tenant, &novo, serde_json::json!({}))
        .unwrap();
}

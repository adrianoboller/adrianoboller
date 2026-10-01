//! O broker tem de reabrir a pasta que ele mesmo gravou. Achado ao ligar o canal Telegram:
//! o segundo processo caia em "JSON error: expected value at line 1 column 1", porque o
//! `reload` lia o envelope sem tirar o prefixo magico.

use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_live_bus::LiveEventHub;
use phxclaw_secret_broker::{FileMasterKeyProvider, SecretBroker, SecretValue};
use std::sync::Arc;

fn abrir(dir: &std::path::Path) -> SecretBroker {
    let key = Arc::new(FileMasterKeyProvider::new(dir.join("master.key")));
    key.ensure().unwrap();
    SecretBroker::new(
        dir.join("cofre"),
        key,
        LiveEventHub::new(16, 16),
        EvidenceLedger::open(dir.join("evidence.jsonl")).unwrap(),
    )
    .unwrap()
}

#[test]
fn broker_reabre_a_pasta_e_resolve_o_segredo_guardado_antes() {
    let dir = std::env::temp_dir().join(format!("phx-broker-{}", uuid::Uuid::now_v7()));
    let id = abrir(&dir)
        .store(
            "s",
            "ns",
            vec!["*".into()],
            SecretValue::new("valor-guardado".into()),
        )
        .unwrap()
        .uuid;
    let b = abrir(&dir);
    let lease = b.issue_lease(id, "teste", "x", 30).unwrap();
    assert_eq!(
        b.resolve(lease.uuid, "x").unwrap().expose(),
        "valor-guardado"
    );
    let _ = std::fs::remove_dir_all(dir);
}

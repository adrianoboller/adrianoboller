//! Portao `channel_provider_credentialed_e2e`: o Telegram de verdade, com o bot do dono.
//! O token vem do ambiente (`PHXCLAW_TELEGRAM_BOT_TOKEN`), entra no cofre e o provedor so
//! o ve por concessao curta -- o mesmo caminho da producao. O chat de destino vem de
//! `PHXCLAW_TELEGRAM_CHAT_ID`.
//!
//! Prova nos dois sentidos: o token certo conecta, identifica o bot e entrega a mensagem
//! (o Telegram devolve o message_id); um token adulterado NAO conecta, e o erro nao traz
//! o token.

use chrono::Utc;
use phxclaw_channel_gateway::{ChannelProvider, ChannelProviderV2, OutboundMessage};
use phxclaw_channel_providers::TelegramProvider;
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_live_bus::LiveEventHub;
use phxclaw_secret_broker::{FileMasterKeyProvider, SecretBroker, SecretValue};
use std::sync::Arc;
use uuid::Uuid;

fn provedor(dir: &std::path::Path, token: &str) -> TelegramProvider {
    let key = Arc::new(FileMasterKeyProvider::new(dir.join("master.key")));
    key.ensure().unwrap();
    let broker = Arc::new(
        SecretBroker::new(
            dir.join("secrets"),
            key,
            LiveEventHub::new(16, 16),
            EvidenceLedger::open(dir.join("evidence.jsonl")).unwrap(),
        )
        .unwrap(),
    );
    let d = broker
        .store(
            "telegram",
            "channels",
            vec!["*".into()],
            SecretValue::new(token.into()),
        )
        .unwrap();
    TelegramProvider::new(broker, d.uuid, "e2e").unwrap()
}

#[test]
#[ignore = "exige PHXCLAW_TELEGRAM_BOT_TOKEN e PHXCLAW_TELEGRAM_CHAT_ID reais"]
fn telegram_real_conecta_entrega_e_recusa_token_adulterado() {
    let token = std::env::var("PHXCLAW_TELEGRAM_BOT_TOKEN").expect("PHXCLAW_TELEGRAM_BOT_TOKEN");
    let chat = std::env::var("PHXCLAW_TELEGRAM_CHAT_ID").expect("PHXCLAW_TELEGRAM_CHAT_ID");
    let dir = std::env::temp_dir().join(format!("phx-tg-e2e-{}", Uuid::now_v7()));

    let p = provedor(&dir.join("certo"), &token);
    let sonda = ChannelProviderV2::probe(&p).expect("getMe");
    assert!(sonda.connected, "bot nao conectou: {:?}", sonda.error);
    println!("bot: {:?}", sonda.display_name);

    let msg = OutboundMessage {
        uuid: Uuid::now_v7(),
        session_uuid: Uuid::now_v7(),
        principal_uuid: Uuid::now_v7(),
        channel: "telegram".into(),
        account_id: "e2e".into(),
        conversation_id: chat,
        text: format!("PhxClaw E2E {}", Utc::now().to_rfc3339()),
        created_at: Utc::now(),
    };
    let recibo = p.send(&msg).expect("sendMessage");
    assert!(
        recibo
            .provider_message_id
            .parse::<i64>()
            .is_ok_and(|n| n > 0),
        "message_id invalido: {}",
        recibo.provider_message_id
    );
    println!("entregue: message_id={}", recibo.provider_message_id);

    // RED: o mesmo bot com o segredo trocado tem de ser recusado pelo Telegram.
    let (id, _) = token.split_once(':').expect("token no formato id:segredo");
    let falso = format!("{id}:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let q = provedor(&dir.join("falso"), &falso);
    let sonda = ChannelProviderV2::probe(&q).expect("getMe responde mesmo recusando");
    assert!(!sonda.connected, "token adulterado conectou");
    let erro = q.send(&msg).expect_err("token adulterado entregou");
    assert!(
        !erro.contains(&falso) && !erro.contains(&token),
        "token vazou: {erro}"
    );
    println!("adulterado recusado: {erro}");
    let _ = std::fs::remove_dir_all(dir);
}

#![forbid(unsafe_code)]
//! No de dispositivo: pareia (uma vez, com token), abre sessao e bate o coracao.
//!
//! Ambiente:
//! - PHXCLAW_DEVICE_WSS_URL   wss://servidor:porta/
//! - PHXCLAW_TENANT_UUID, PHXCLAW_NODE_UUID
//! - PHXCLAW_ENROLLMENT_TOKEN so no primeiro uso: gera a chave Ed25519 no chaveiro do
//!   sistema e pareia; depois disso, o no entra so com a chave guardada
//! - PHXCLAW_DEVICE_CA_PEM    caminho da CA do servidor; sem ela valem as raizes publicas
//! - PHXCLAW_DEVICE_KEYSTORE  "arquivo:/pasta" guarda a chave em arquivo 0600 (maquina
//!   sem chaveiro do sistema); sem isso vale o chaveiro, e chaveiro que nao persiste recusa
use phxclaw_device_transport::servidor::Boasvindas;
use phxclaw_device_transport::{
    DevicePlatform, EnrollmentRequest, NodeHello, NodeIdentity, WssDeviceClient, parear,
    platform_default_capabilities,
};
use phxclaw_key_provider::{ArquivoKeyProvider, KeyProvider, OsKeyringProvider};
use std::env;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = env::var("PHXCLAW_DEVICE_WSS_URL")?;
    let tenant_uuid: Uuid = env::var("PHXCLAW_TENANT_UUID")?.parse()?;
    let node_uuid: Uuid = env::var("PHXCLAW_NODE_UUID")?.parse()?;
    let platform = current_platform()?;
    let keyring: Box<dyn KeyProvider> = match env::var("PHXCLAW_DEVICE_KEYSTORE") {
        Ok(v) if v.starts_with("arquivo:") => {
            Box::new(ArquivoKeyProvider::new(&v["arquivo:".len()..])?)
        }
        Ok(v) => return Err(format!("PHXCLAW_DEVICE_KEYSTORE desconhecido: {v}").into()),
        Err(_) => {
            // identidade que some ao reiniciar faria o no parear de novo a cada boot,
            // gastando token -- melhor recusar dizendo o que fazer
            if !OsKeyringProvider::persistente() {
                return Err(
                    "o chaveiro desta plataforma nao guarda chave entre execucoes; \
use PHXCLAW_DEVICE_KEYSTORE=arquivo:/pasta"
                        .into(),
                );
            }
            Box::new(OsKeyringProvider::new("PhxClaw", "device-node"))
        }
    };
    let mut client = match env::var("PHXCLAW_DEVICE_CA_PEM") {
        Ok(p) => WssDeviceClient::connect_with_ca(&endpoint, &std::fs::read(p)?).await?,
        Err(_) => WssDeviceClient::connect(&endpoint).await?,
    };
    let mut seq = 0u64;
    let identity = match env::var("PHXCLAW_ENROLLMENT_TOKEN") {
        Ok(token) => {
            let pedido = EnrollmentRequest {
                tenant_uuid,
                node_uuid,
                display_name: env::var("HOSTNAME")
                    .or_else(|_| env::var("COMPUTERNAME"))
                    .unwrap_or_else(|_| "no".into()),
                platform,
                agent_version: env!("CARGO_PKG_VERSION").into(),
                public_key_ed25519_b64: String::new(),
                enrollment_token: token,
                capabilities: platform_default_capabilities(platform),
            };
            let id = parear(&mut client, keyring.as_ref(), pedido).await?;
            seq += 1;
            println!("pareado");
            id
        }
        Err(_) => NodeIdentity::load(keyring.as_ref(), node_uuid)?,
    };
    let hello = NodeHello {
        tenant_uuid,
        platform,
        agent_version: env!("CARGO_PKG_VERSION").into(),
        capabilities: platform_default_capabilities(platform),
    };
    let body = serde_json::to_vec(&hello)?;
    client
        .send(&identity.sign_envelope(Uuid::nil(), seq, "device.hello", &body)?)
        .await?;
    let b: Boasvindas = serde_json::from_slice(&esperar(&mut client, "device.welcome").await?)?;
    println!("sessao {} cerca {}", b.session_uuid, b.fencing_token);
    let mut n = 0u64;
    loop {
        n += 1;
        client
            .send(&identity.sign_envelope(b.session_uuid, n, "device.heartbeat", b"{}")?)
            .await?;
        esperar(&mut client, "device.ack").await?;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// Le a resposta; recusa do servidor vira erro com o motivo.
async fn esperar(
    client: &mut WssDeviceClient,
    tipo: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let r = client.receive().await?;
    let corpo = r.decode_and_verify_body()?;
    if r.kind != tipo {
        return Err(format!("{}: {}", r.kind, String::from_utf8_lossy(&corpo)).into());
    }
    Ok(corpo)
}

fn current_platform() -> Result<DevicePlatform, Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        return Ok(DevicePlatform::Windows);
    }
    #[cfg(target_os = "linux")]
    {
        return Ok(DevicePlatform::Linux);
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(DevicePlatform::Macos);
    }
    #[cfg(target_os = "android")]
    {
        return Ok(DevicePlatform::Android);
    }
    #[cfg(target_os = "ios")]
    {
        return Ok(DevicePlatform::Ios);
    }
    #[allow(unreachable_code)]
    Err("unsupported platform".into())
}

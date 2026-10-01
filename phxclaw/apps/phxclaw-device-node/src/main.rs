#![forbid(unsafe_code)]
//! No de dispositivo: pareia (uma vez, com token), abre sessao, bate o coracao e atende
//! `device.command` (hoje: `device.system.info`), respondendo `device.result` assinado.
//!
//! Ambiente:
//! - PHXCLAW_DEVICE_WSS_URL   wss://servidor:porta/
//! - PHXCLAW_TENANT_UUID, PHXCLAW_NODE_UUID
//! - PHXCLAW_ENROLLMENT_TOKEN so no primeiro uso: gera a chave Ed25519 no chaveiro do
//!   sistema e pareia; depois disso, o no entra so com a chave guardada
//! - PHXCLAW_DEVICE_CA_PEM    caminho da CA do servidor; sem ela valem as raizes publicas
//! - PHXCLAW_DEVICE_KEYSTORE  "arquivo:/pasta" guarda a chave em arquivo 0600 (maquina
//!   sem chaveiro do sistema); sem isso vale o chaveiro, e chaveiro que nao persiste recusa
use phxclaw_device_nodes::DeviceCommand;
use phxclaw_device_transport::servidor::ResultadoDeComando;
use phxclaw_device_transport::{
    DevicePlatform, DeviceTransportError, EnrollmentRequest, NodeHello, NodeIdentity,
    WssDeviceClient, parear, platform_default_capabilities,
};
use phxclaw_key_provider::{ArquivoKeyProvider, KeyProvider, OsKeyringProvider};
use std::env;
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
            let id = parear(&mut client, keyring.as_ref(), pedido)
                .await
                .map_err(|e| match e {
                    // o erro cru ("Secret Service: no result found") manda procurar o banco
                    // de chaves, e o que falta e a sessao do chaveiro; o token nao se gastou
                    DeviceTransportError::KeyProvider(_) => format!(
                        "{e}. O chaveiro do sistema nao gravou (sem sessao DBus/Secret \
Service?). O token NAO foi gasto: rode de novo numa sessao com chaveiro, ou use \
PHXCLAW_DEVICE_KEYSTORE=arquivo:/pasta"
                    ),
                    e => e.to_string(),
                })?;
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
    // O laco da sessao (hello, coracao, cerca e capacidade declarada) e o MESMO do agente
    // ligado a uma ponte; aqui so muda o que o no sabe executar.
    phxclaw_device_transport::no::atender(
        client,
        &identity,
        &hello,
        seq,
        |b| println!("sessao {} cerca {}", b.session_uuid, b.fencing_token),
        |cmd| async move { executar(&cmd) },
    )
    .await?;
    Ok(())
}

/// O que este no sabe fazer. Cerca e capacidade declarada ja vieram conferidas pelo laco
/// de `no::atender`.
fn executar(cmd: &DeviceCommand) -> ResultadoDeComando {
    let falha = |m: String| ResultadoDeComando {
        command_uuid: cmd.command_uuid,
        ok: false,
        saida: serde_json::Value::Null,
        erro: Some(m),
    };
    match cmd.capability.as_str() {
        "device.system.info" => ResultadoDeComando {
            command_uuid: cmd.command_uuid,
            ok: true,
            saida: serde_json::json!({
                "hostname": env::var("HOSTNAME").or_else(|_| env::var("COMPUTERNAME")).ok()
                    .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string())),
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "versao": env!("CARGO_PKG_VERSION"),
            }),
            erro: None,
        },
        outra => falha(format!("{outra} declarada mas nao implementada neste no")),
    }
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

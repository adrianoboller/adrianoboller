#![forbid(unsafe_code)]
use phxclaw_device_transport::{platform_default_capabilities, DevicePlatform, NodeHello, NodeIdentity, WssDeviceClient};
use phxclaw_key_provider::OsKeyringProvider;
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint=env::var("PHXCLAW_DEVICE_WSS_URL")?;
    let tenant_uuid:Uuid=env::var("PHXCLAW_TENANT_UUID")?.parse()?;
    let node_uuid:Uuid=env::var("PHXCLAW_NODE_UUID")?.parse()?;
    let platform=current_platform()?;
    let keyring=OsKeyringProvider::new("PhxClaw","device-node");
    let identity=NodeIdentity::load(&keyring,node_uuid)?;
    let hello=NodeHello{tenant_uuid,platform,agent_version:env!("CARGO_PKG_VERSION").into(),capabilities:platform_default_capabilities(platform)};
    let body=serde_json::to_vec(&hello)?;
    let envelope=identity.sign_envelope(Uuid::nil(),0,"device.hello",&body)?;
    let mut client=WssDeviceClient::connect(&endpoint).await?;
    client.send(&envelope).await?;
    loop { let incoming=client.receive().await?; println!("{}",serde_json::to_string(&incoming)?); }
}
fn current_platform()->Result<DevicePlatform,Box<dyn std::error::Error>>{
    #[cfg(target_os="windows")] { return Ok(DevicePlatform::Windows); }
    #[cfg(target_os="linux")] { return Ok(DevicePlatform::Linux); }
    #[cfg(target_os="macos")] { return Ok(DevicePlatform::Macos); }
    #[cfg(target_os="android")] { return Ok(DevicePlatform::Android); }
    #[cfg(target_os="ios")] { return Ok(DevicePlatform::Ios); }
    #[allow(unreachable_code)] Err("unsupported platform".into())
}

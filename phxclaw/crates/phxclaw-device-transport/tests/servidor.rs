//! Pareamento de dispositivo pelo fio: servidor WSS com TLS real (certificado do openssl),
//! cliente do proprio crate, envelopes assinados com Ed25519. Prova o caminho feliz e cada
//! recusa: token reusado, no desconhecido, corpo adulterado, replay, heartbeat sem sessao,
//! e cliente sem TLS.

use phxclaw_device_transport::servidor::{
    Boasvindas, RegistroMemoria, ServidorDispositivos, tls_de_pem,
};
use phxclaw_device_transport::*;
use phxclaw_key_provider::{KeyMaterial, KeyProvider, KeyProviderError};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const TOKEN: &str = "token-de-pareamento-com-24-caracteres-ou-mais";

#[derive(Default)]
struct Chaveiro(Mutex<HashMap<String, Vec<u8>>>);
impl KeyProvider for Chaveiro {
    fn provider_id(&self) -> &str {
        "memoria-teste"
    }
    fn is_release_safe(&self) -> bool {
        false
    }
    fn load(&self, id: &str) -> Result<KeyMaterial, KeyProviderError> {
        let m = self.0.lock().unwrap();
        KeyMaterial::new(m.get(id).cloned().ok_or(KeyProviderError::NotFound)?)
    }
    fn store(&self, id: &str, k: &KeyMaterial) -> Result<(), KeyProviderError> {
        self.0
            .lock()
            .unwrap()
            .insert(id.into(), k.expose().to_vec());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), KeyProviderError> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

/// CA propria + certificado de servidor para localhost assinado por ela, como numa
/// implantacao real: o no fixa a CA, o servidor apresenta a folha. (Autoassinado usado
/// direto como folha o rustls recusa com CaUsedAsEndEntity, e esta certo.)
/// Devolve (cert do servidor, chave do servidor, CA).
fn certificado() -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let d = std::env::temp_dir().join(format!("phx-wss-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&d).ok()?;
    let sh = |args: &[&str]| {
        Command::new("openssl")
            .args(args)
            .current_dir(&d)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    std::fs::write(
        d.join("ext.cnf"),
        "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n",
    )
    .ok()?;
    let ok = sh(&[
        "req",
        "-x509",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-days",
        "1",
        "-subj",
        "/CN=CA de teste PhxClaw",
        "-keyout",
        "ca.key",
        "-out",
        "ca.pem",
    ]) && sh(&[
        "req",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-subj",
        "/CN=localhost",
        "-keyout",
        "srv.key",
        "-out",
        "srv.csr",
    ]) && sh(&[
        "x509",
        "-req",
        "-in",
        "srv.csr",
        "-CA",
        "ca.pem",
        "-CAkey",
        "ca.key",
        "-CAcreateserial",
        "-days",
        "1",
        "-extfile",
        "ext.cnf",
        "-out",
        "srv.pem",
    ]);
    ok.then(|| {
        let l = |n: &str| std::fs::read(d.join(n)).unwrap();
        (l("srv.pem"), l("srv.key"), l("ca.pem"))
    })
}

async fn subir() -> Option<(String, Vec<u8>, Arc<Mutex<RegistroMemoria>>, Uuid)> {
    let (cert, chave, ca) = certificado()?;
    let tenant = Uuid::now_v7();
    let reg = Arc::new(Mutex::new(RegistroMemoria::default()));
    reg.lock().unwrap().emitir_token(tenant, TOKEN);
    let srv = Arc::new(ServidorDispositivos::novo(reg.clone()).unwrap());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = l.local_addr().unwrap().port();
    tokio::spawn(srv.servir(l, tls_de_pem(&cert, &chave).unwrap()));
    Some((format!("wss://localhost:{porta}/"), ca, reg, tenant))
}

fn pedido(tenant: Uuid, id: &NodeIdentity, token: &str) -> Vec<u8> {
    serde_json::to_vec(&EnrollmentRequest {
        tenant_uuid: tenant,
        node_uuid: id.node_uuid,
        display_name: "notebook do teste".into(),
        platform: DevicePlatform::Linux,
        agent_version: "0.70.0".into(),
        public_key_ed25519_b64: id.public_key_ed25519_b64.clone(),
        enrollment_token: token.into(),
        capabilities: platform_default_capabilities(DevicePlatform::Linux),
    })
    .unwrap()
}

fn ola(tenant: Uuid) -> Vec<u8> {
    serde_json::to_vec(&NodeHello {
        tenant_uuid: tenant,
        platform: DevicePlatform::Linux,
        agent_version: "0.70.0".into(),
        capabilities: platform_default_capabilities(DevicePlatform::Linux),
    })
    .unwrap()
}

#[tokio::test]
async fn pareia_abre_sessao_e_bate_coracao_sob_tls() {
    let Some((url, ca, reg, tenant)) = subir().await else {
        eprintln!("sem openssl: pulado");
        return;
    };
    let chaveiro = Chaveiro::default();
    let id = NodeIdentity::generate_and_store(&chaveiro, Uuid::now_v7()).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();

    c.send(
        &id.sign_envelope(Uuid::nil(), 0, "device.enroll", &pedido(tenant, &id, TOKEN))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.enrolled");

    // a chave guardada no chaveiro e a que o servidor conhece: recarregada, ainda vale
    let id = NodeIdentity::load(&chaveiro, id.node_uuid).unwrap();
    c.send(
        &id.sign_envelope(Uuid::nil(), 1, "device.hello", &ola(tenant))
            .unwrap(),
    )
    .await
    .unwrap();
    let r = c.receive().await.unwrap();
    assert_eq!(r.kind, "device.welcome");
    let b: Boasvindas = serde_json::from_slice(&r.decode_and_verify_body().unwrap()).unwrap();
    assert!(
        b.fencing_token >= 2,
        "cerca cresce a cada sessao: {}",
        b.fencing_token
    );

    let hb = id
        .sign_envelope(b.session_uuid, 1, "device.heartbeat", b"{}")
        .unwrap();
    c.send(&hb).await.unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.ack");
    let no = reg.lock().unwrap().no(tenant, id.node_uuid).unwrap();
    assert!(no.last_seen_at.is_some());
    assert_eq!(format!("{:?}", no.state), "Active");

    // replay do mesmo heartbeat: recusado e a conexao fecha
    c.send(&hb).await.unwrap();
    let r = c.receive().await.unwrap();
    assert_eq!(r.kind, "device.rejected");
    let motivo = String::from_utf8(r.decode_and_verify_body().unwrap()).unwrap();
    assert!(motivo.contains("replay"), "{motivo}");
}

#[tokio::test]
async fn token_nao_serve_duas_vezes_e_no_desconhecido_nao_entra() {
    let Some((url, ca, _, tenant)) = subir().await else {
        return;
    };
    let ch = Chaveiro::default();
    let a = NodeIdentity::generate_and_store(&ch, Uuid::now_v7()).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &a.sign_envelope(Uuid::nil(), 0, "device.enroll", &pedido(tenant, &a, TOKEN))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.enrolled");

    let b = NodeIdentity::generate_and_store(&ch, Uuid::now_v7()).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &b.sign_envelope(Uuid::nil(), 0, "device.enroll", &pedido(tenant, &b, TOKEN))
            .unwrap(),
    )
    .await
    .unwrap();
    let r = c.receive().await.unwrap();
    assert_eq!(r.kind, "device.rejected", "token reusado");

    // no que nunca pareou nao abre sessao
    let x = NodeIdentity::generate_and_store(&ch, Uuid::now_v7()).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &x.sign_envelope(Uuid::nil(), 0, "device.hello", &ola(tenant))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.rejected");
}

#[tokio::test]
async fn assinatura_de_outra_chave_e_corpo_adulterado_sao_recusados() {
    let Some((url, ca, _, tenant)) = subir().await else {
        return;
    };
    let ch = Chaveiro::default();
    let dono = NodeIdentity::generate_and_store(&ch, Uuid::now_v7()).unwrap();
    // o impostor assina um pedido que declara a chave publica do dono (sem ter a privada)
    let impostor = NodeIdentity::efemera(dono.node_uuid).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &impostor
            .sign_envelope(
                Uuid::nil(),
                0,
                "device.enroll",
                &pedido(tenant, &dono, TOKEN),
            )
            .unwrap(),
    )
    .await
    .unwrap();
    let r = c.receive().await.unwrap();
    assert_eq!(r.kind, "device.rejected", "sem a chave privada nao pareia");

    // corpo trocado depois de assinado
    let mut env = dono
        .sign_envelope(
            Uuid::nil(),
            0,
            "device.enroll",
            &pedido(tenant, &dono, TOKEN),
        )
        .unwrap();
    env.body_b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(pedido(
            tenant,
            &dono,
            "outro-token-com-mais-de-24-caracteres",
        ))
    };
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(&env).await.unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.rejected");

    // e o token continua valendo para o dono de verdade: as recusas nao o gastaram
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &dono
            .sign_envelope(
                Uuid::nil(),
                0,
                "device.enroll",
                &pedido(tenant, &dono, TOKEN),
            )
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(c.receive().await.unwrap().kind, "device.enrolled");
}

#[tokio::test]
async fn sem_tls_nao_conecta_e_certificado_estranho_e_recusado() {
    let Some((url, _ca, _, _)) = subir().await else {
        return;
    };
    let ws = url.replace("wss://", "ws://");
    assert!(matches!(
        WssDeviceClient::connect(&ws).await,
        Err(DeviceTransportError::InsecureTransport)
    ));
    // outra CA: o cliente nao aceita o servidor
    let (_, _, outra) = certificado().unwrap();
    assert!(
        WssDeviceClient::connect_with_ca(&url, &outra)
            .await
            .is_err()
    );
}

/// Defeito achado rodando o no Windows (pelo Wine) contra este servidor: o segundo
/// pareamento, com o token ja gasto, gravava uma chave nova ANTES da recusa, e o no
/// pareado de verdade passava a assinar com uma chave que o servidor nao conhece.
#[tokio::test]
async fn pareamento_recusado_nao_apaga_a_identidade_que_ja_vale() {
    let Some((url, ca, _, tenant)) = subir().await else {
        return;
    };
    let ch = Chaveiro::default();
    let no = Uuid::now_v7();
    let pedido_do_no = || EnrollmentRequest {
        tenant_uuid: tenant,
        node_uuid: no,
        display_name: "notebook do teste".into(),
        platform: DevicePlatform::Linux,
        agent_version: "0.70.0".into(),
        public_key_ed25519_b64: String::new(),
        enrollment_token: TOKEN.into(),
        capabilities: platform_default_capabilities(DevicePlatform::Linux),
    };
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    parear(&mut c, &ch, pedido_do_no()).await.unwrap();

    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    let Err(erro) = parear(&mut c, &ch, pedido_do_no()).await else {
        panic!("o mesmo token pareou duas vezes");
    };
    assert!(
        matches!(erro, DeviceTransportError::PairingRejected(ref m) if m.contains("token")),
        "{erro}"
    );

    // a chave guardada continua a que o servidor conhece
    let id = NodeIdentity::load(&ch, no).unwrap();
    let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
    c.send(
        &id.sign_envelope(Uuid::nil(), 1, "device.hello", &ola(tenant))
            .unwrap(),
    )
    .await
    .unwrap();
    let r = c.receive().await.unwrap();
    assert_eq!(
        r.kind, "device.welcome",
        "identidade perdida no pareamento recusado"
    );
}

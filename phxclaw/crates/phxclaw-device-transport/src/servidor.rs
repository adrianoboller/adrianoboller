//! O servidor WSS de dispositivos: a ponta que faltava. O cliente, a identidade Ed25519
//! do no e a verificacao do envelope ja existiam; faltava quem os recebesse.
//!
//! Protocolo (cada mensagem e um `DeviceEnvelope` assinado pelo no):
//! - `device.enroll` (corpo `EnrollmentRequest`): a assinatura e conferida com a chave que
//!   vem no proprio pedido (prova de posse), e o token de pareamento e consumido UMA vez;
//! - `device.hello` (corpo `NodeHello`): so de no pareado; abre a sessao e devolve o
//!   token de cerca (fencing) -- o que impede um no antigo de agir depois de outro assumir;
//! - `device.heartbeat`: so dentro da sessao aberta, com sequencia crescente.
//!
//! Toda verificacao passa por `phxclaw_device_nodes::verify_envelope`, a mesma do dominio:
//! uma segunda conferencia de assinatura escrita aqui seria a que alguem esqueceria de
//! apertar. Qualquer recusa responde `device.rejected` com o motivo e fecha a conexao.

use crate::{
    DeviceTransportError, EnrollmentAcceptance, EnrollmentRecord, EnrollmentRepository,
    EnrollmentRequest, NodeHello, NodeIdentity, consume_enrollment,
};
use chrono::{DateTime, Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use phxclaw_device_nodes::{
    DeviceError, DeviceNode, DeviceRepository, DeviceState, ReplayGuard, verify_envelope,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

/// TLS do servidor a partir de certificado e chave em PEM (o operador fornece).
pub fn tls_de_pem(cert_pem: &[u8], chave_pem: &[u8]) -> Result<TlsAcceptor, DeviceTransportError> {
    use rustls_pki_types::pem::PemObject;
    let erro = |e: String| DeviceTransportError::WebSocket(format!("TLS: {e}"));
    let certs = rustls_pki_types::CertificateDer::pem_slice_iter(cert_pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| erro(e.to_string()))?;
    let chave = rustls_pki_types::PrivateKeyDer::from_pem_slice(chave_pem)
        .map_err(|e| erro(e.to_string()))?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| erro(e.to_string()))?
    .with_no_client_auth()
    .with_single_cert(certs, chave)
    .map_err(|e| erro(e.to_string()))?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Replay em memoria: nonce nunca repete; dentro de uma sessao aberta a sequencia so
/// cresce. Na sessao nula (antes do hello) vale so o nonce, porque todo reconectar
/// recomeca nela com sequencia zero.
#[derive(Default)]
pub struct ReplayMemoria {
    nonces: HashSet<(Uuid, String)>,
    ultima: HashMap<(Uuid, Uuid), u64>,
}

impl ReplayGuard for ReplayMemoria {
    fn reserve(
        &mut self,
        node_uuid: Uuid,
        session_uuid: Uuid,
        sequence: u64,
        nonce_hex: &str,
    ) -> Result<(), DeviceError> {
        if self.nonces.contains(&(node_uuid, nonce_hex.to_string())) {
            return Err(DeviceError::Replay);
        }
        if !session_uuid.is_nil() {
            let chave = (node_uuid, session_uuid);
            if let Some(&u) = self.ultima.get(&chave)
                && sequence <= u
            {
                return Err(DeviceError::Replay);
            }
            self.ultima.insert(chave, sequence);
        }
        self.nonces.insert((node_uuid, nonce_hex.to_string()));
        Ok(())
    }
}

/// Registro em memoria de tokens de pareamento e nos. A verdade duravel e o PostgreSQL
/// (ver o dominio); este serve ao servidor local e aos testes, com as mesmas regras.
#[derive(Default)]
pub struct RegistroMemoria {
    /// sha256(token) -> tenant; sai daqui ao ser consumido (uso unico).
    tokens: Mutex<HashMap<String, Uuid>>,
    nos: Mutex<HashMap<(Uuid, Uuid), DeviceNode>>,
    cerca: Mutex<HashMap<(Uuid, Uuid), i64>>,
}

impl RegistroMemoria {
    /// Emite um token de pareamento; guarda so o hash.
    pub fn emitir_token(&self, tenant_uuid: Uuid, token: &str) {
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(crate::enrollment_token_sha256_hex(token), tenant_uuid);
    }

    pub fn no(&self, tenant_uuid: Uuid, node_uuid: Uuid) -> Option<DeviceNode> {
        self.nos
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(tenant_uuid, node_uuid))
            .cloned()
    }

    fn proxima_cerca(&self, tenant_uuid: Uuid, node_uuid: Uuid) -> i64 {
        let mut c = self.cerca.lock().unwrap_or_else(|p| p.into_inner());
        let v = c.entry((tenant_uuid, node_uuid)).or_insert(0);
        *v += 1;
        *v
    }
}

impl EnrollmentRepository for RegistroMemoria {
    fn consume(&self, r: EnrollmentRecord) -> Result<EnrollmentAcceptance, DeviceTransportError> {
        let dono = self
            .tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&r.token_sha256_hex);
        match dono {
            Some(t) if t == r.tenant_uuid => {}
            _ => {
                return Err(DeviceTransportError::PairingRejected(
                    "token de pareamento invalido ou ja usado".into(),
                ));
            }
        }
        let mut nos = self.nos.lock().unwrap_or_else(|p| p.into_inner());
        if nos.contains_key(&(r.tenant_uuid, r.node_uuid)) {
            return Err(DeviceTransportError::PairingRejected(
                "no ja pareado".into(),
            ));
        }
        nos.insert(
            (r.tenant_uuid, r.node_uuid),
            DeviceNode {
                node_uuid: r.node_uuid,
                tenant_uuid: r.tenant_uuid,
                display_name: r.display_name,
                state: DeviceState::Enrolled,
                public_key_ed25519_b64: r.public_key_ed25519_b64,
                capabilities: r.capabilities,
                last_seen_at: None,
            },
        );
        drop(nos);
        Ok(EnrollmentAcceptance {
            session_uuid: Uuid::now_v7(),
            fencing_token: self.proxima_cerca(r.tenant_uuid, r.node_uuid),
            expires_at: Utc::now() + Duration::hours(12),
        })
    }
}

impl DeviceRepository for RegistroMemoria {
    fn get_node(&self, tenant_uuid: Uuid, node_uuid: Uuid) -> Result<DeviceNode, DeviceError> {
        self.no(tenant_uuid, node_uuid)
            .ok_or_else(|| DeviceError::Repository("no nao pareado".into()))
    }
    fn persist_heartbeat(
        &mut self,
        tenant_uuid: Uuid,
        node_uuid: Uuid,
        at: DateTime<Utc>,
    ) -> Result<(), DeviceError> {
        let mut nos = self.nos.lock().unwrap_or_else(|p| p.into_inner());
        let n = nos
            .get_mut(&(tenant_uuid, node_uuid))
            .ok_or_else(|| DeviceError::Repository("no nao pareado".into()))?;
        n.last_seen_at = Some(at);
        if n.state == DeviceState::Enrolled {
            n.state = DeviceState::Active;
        }
        Ok(())
    }
    fn next_fencing_token(
        &mut self,
        tenant_uuid: Uuid,
        node_uuid: Uuid,
    ) -> Result<i64, DeviceError> {
        Ok(self.proxima_cerca(tenant_uuid, node_uuid))
    }
}

/// Resposta do servidor ao hello.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Boasvindas {
    pub session_uuid: Uuid,
    pub fencing_token: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recusa {
    pub motivo: String,
}

pub struct ServidorDispositivos {
    pub registro: Arc<Mutex<RegistroMemoria>>,
    pub replay: Arc<Mutex<ReplayMemoria>>,
    pub identidade: NodeIdentity,
    pub max_desvio_relogio: Duration,
}

impl ServidorDispositivos {
    pub fn novo(registro: Arc<Mutex<RegistroMemoria>>) -> Result<Self, DeviceTransportError> {
        Ok(Self {
            registro,
            replay: Arc::default(),
            identidade: NodeIdentity::efemera(Uuid::nil())?,
            max_desvio_relogio: Duration::seconds(60),
        })
    }

    /// Aceita conexoes para sempre; cada uma numa tarefa propria.
    pub async fn servir(self: Arc<Self>, l: TcpListener, tls: TlsAcceptor) {
        loop {
            let Ok((tcp, _)) = l.accept().await else {
                continue;
            };
            let (s, tls) = (self.clone(), tls.clone());
            tokio::spawn(async move {
                // handshake TLS falho (cliente sem TLS, certificado recusado) so encerra
                if let Ok(t) = tls.accept(tcp).await
                    && let Ok(ws) = tokio_tungstenite::accept_async(t).await
                {
                    s.conexao(ws).await;
                }
            });
        }
    }

    async fn conexao<S>(&self, mut ws: tokio_tungstenite::WebSocketStream<S>)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // (tenant, no, sessao) depois do hello
        let mut sessao: Option<(Uuid, Uuid, Uuid)> = None;
        let mut seq_saida = 0u64;
        while let Some(Ok(m)) = ws.next().await {
            let texto = match m {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                Message::Close(_) => break,
                _ => continue,
            };
            let resposta = match serde_json::from_str(&texto) {
                Ok(env) => self.tratar(&env, &mut sessao),
                Err(e) => Err(format!("envelope invalido: {e}")),
            };
            let (tipo, corpo, fim) = match resposta {
                Ok((t, c)) => (t, c, false),
                Err(motivo) => (
                    "device.rejected",
                    serde_json::to_vec(&Recusa { motivo }).unwrap_or_default(),
                    true,
                ),
            };
            let sessao_saida = sessao.map(|s| s.2).unwrap_or_default();
            seq_saida += 1;
            let Ok(env) = self
                .identidade
                .sign_envelope(sessao_saida, seq_saida, tipo, &corpo)
            else {
                break;
            };
            let Ok(txt) = serde_json::to_string(&env) else {
                break;
            };
            if ws.send(Message::Text(txt.into())).await.is_err() || fim {
                break;
            }
        }
        let _ = ws.close(None).await;
    }

    fn tratar(
        &self,
        env: &phxclaw_device_nodes::DeviceEnvelope,
        sessao: &mut Option<(Uuid, Uuid, Uuid)>,
    ) -> Result<(&'static str, Vec<u8>), String> {
        let agora = Utc::now();
        let mut replay = self.replay.lock().unwrap_or_else(|p| p.into_inner());
        match env.kind.as_str() {
            "device.enroll" => {
                // o corpo e lido ANTES da assinatura so para achar a chave que a confere;
                // nada dele vale ate verify_envelope passar
                let corpo = env.decode_and_verify_body().map_err(|e| e.to_string())?;
                let pedido: EnrollmentRequest =
                    serde_json::from_slice(&corpo).map_err(|e| format!("pedido: {e}"))?;
                if pedido.node_uuid != env.node_uuid {
                    return Err("no do pedido difere do envelope".into());
                }
                let candidato = DeviceNode {
                    node_uuid: pedido.node_uuid,
                    tenant_uuid: pedido.tenant_uuid,
                    display_name: pedido.display_name.clone(),
                    state: DeviceState::Pending,
                    public_key_ed25519_b64: pedido.public_key_ed25519_b64.clone(),
                    capabilities: pedido.capabilities.clone(),
                    last_seen_at: None,
                };
                verify_envelope(
                    &candidato,
                    env,
                    &mut *replay,
                    agora,
                    self.max_desvio_relogio,
                )
                .map_err(|e| e.to_string())?;
                let reg = self.registro.lock().unwrap_or_else(|p| p.into_inner());
                let aceite = consume_enrollment(&*reg, pedido).map_err(|e| e.to_string())?;
                Ok((
                    "device.enrolled",
                    serde_json::to_vec(&aceite).map_err(|e| e.to_string())?,
                ))
            }
            "device.hello" => {
                let corpo = env.decode_and_verify_body().map_err(|e| e.to_string())?;
                let ola: NodeHello =
                    serde_json::from_slice(&corpo).map_err(|e| format!("hello: {e}"))?;
                let mut reg = self.registro.lock().unwrap_or_else(|p| p.into_inner());
                let no = reg
                    .get_node(ola.tenant_uuid, env.node_uuid)
                    .map_err(|e| e.to_string())?;
                verify_envelope(&no, env, &mut *replay, agora, self.max_desvio_relogio)
                    .map_err(|e| e.to_string())?;
                let cerca = reg
                    .next_fencing_token(ola.tenant_uuid, env.node_uuid)
                    .map_err(|e| e.to_string())?;
                let s = Uuid::now_v7();
                *sessao = Some((ola.tenant_uuid, env.node_uuid, s));
                Ok((
                    "device.welcome",
                    serde_json::to_vec(&Boasvindas {
                        session_uuid: s,
                        fencing_token: cerca,
                    })
                    .map_err(|e| e.to_string())?,
                ))
            }
            "device.heartbeat" => {
                let (tenant, no_id, s) = sessao.ok_or("heartbeat antes do hello")?;
                if env.node_uuid != no_id || env.session_uuid != s {
                    return Err("heartbeat fora da sessao aberta".into());
                }
                let mut reg = self.registro.lock().unwrap_or_else(|p| p.into_inner());
                let no = reg.get_node(tenant, no_id).map_err(|e| e.to_string())?;
                verify_envelope(&no, env, &mut *replay, agora, self.max_desvio_relogio)
                    .map_err(|e| e.to_string())?;
                reg.persist_heartbeat(tenant, no_id, agora)
                    .map_err(|e| e.to_string())?;
                Ok(("device.ack", b"{}".to_vec()))
            }
            outro => Err(format!("tipo de mensagem desconhecido: {outro}")),
        }
    }
}

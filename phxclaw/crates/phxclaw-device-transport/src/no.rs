//! O lado do no de uma sessao: `device.hello`, `device.welcome`, coracao e o atendimento
//! de `device.command`. Um laco so para todo no -- o binario `phxclaw-device-node` e o
//! agente ligado a uma ponte (`phxclaw servir --ponte`) atendem pela MESMA funcao, e so o
//! que cada um sabe fazer (`executar`) muda.
//!
//! O que o laco decide, e por isso nao pode morar em dois lugares:
//! - uma sequencia SO de saida na sessao: coracao e resultado dividem o contador, e o
//!   servidor recusa sequencia que nao cresce;
//! - o no recusa por conta propria comando com cerca de outra sessao e capacidade que
//!   nao declarou: o servidor ja confere, mas o no nao executa so porque alguem com o
//!   TLS do servidor mandou;
//! - comandos correm em paralelo (cada um numa tarefa) e os resultados voltam por uma
//!   fila: um pedido lento nao pode atrasar o coracao nem o pedido seguinte.

use crate::servidor::{Boasvindas, ResultadoDeComando};
use crate::{DeviceTransportError, NodeHello, NodeIdentity, WssDeviceClient};
use phxclaw_device_nodes::DeviceCommand;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Intervalo do coracao.
pub const CORACAO: Duration = Duration::from_secs(30);

fn falha(cmd: &DeviceCommand, m: String) -> ResultadoDeComando {
    ResultadoDeComando {
        command_uuid: cmd.command_uuid,
        ok: false,
        saida: serde_json::Value::Null,
        erro: Some(m),
    }
}

/// Abre a sessao (`seq` e o ultimo numero ja usado na sessao nula, 1 depois de parear)
/// e atende ate a conexao cair. `ao_abrir` recebe as boas-vindas (para o binario dizer
/// a sessao no terminal). Volta com erro quando o fio cai: quem chama decide se religa.
pub async fn atender<F, Fut>(
    mut client: WssDeviceClient,
    identidade: &NodeIdentity,
    hello: &NodeHello,
    seq: u64,
    ao_abrir: impl FnOnce(&Boasvindas),
    executar: F,
) -> Result<(), DeviceTransportError>
where
    F: Fn(DeviceCommand) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ResultadoDeComando> + Send + 'static,
{
    let corpo = serde_json::to_vec(hello)
        .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
    client
        .send(&identidade.sign_envelope(Uuid::nil(), seq + 1, "device.hello", &corpo)?)
        .await?;
    let r = client.receive().await?;
    let corpo = r
        .decode_and_verify_body()
        .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
    if r.kind != "device.welcome" {
        return Err(DeviceTransportError::PairingRejected(format!(
            "{}: {}",
            r.kind,
            String::from_utf8_lossy(&corpo)
        )));
    }
    let b: Boasvindas = serde_json::from_slice(&corpo)
        .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
    ao_abrir(&b);
    let executar = Arc::new(executar);
    let (tx, mut prontos) = tokio::sync::mpsc::unbounded_channel::<ResultadoDeComando>();
    let mut n = 0u64;
    let mut proximo = tokio::time::Instant::now();
    loop {
        let (tipo, corpo) = tokio::select! {
            _ = tokio::time::sleep_until(proximo) => {
                proximo = tokio::time::Instant::now() + CORACAO;
                ("device.heartbeat", b"{}".to_vec())
            }
            Some(r) = prontos.recv() => (
                "device.result",
                serde_json::to_vec(&r)
                    .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?,
            ),
            r = client.receive() => {
                let env = r?;
                let corpo = env
                    .decode_and_verify_body()
                    .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
                match env.kind.as_str() {
                    "device.ack" => continue,
                    "device.command" => {
                        let cmd: DeviceCommand = serde_json::from_slice(&corpo)
                            .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
                        if cmd.fencing_token != b.fencing_token {
                            let _ = tx.send(falha(&cmd, "cerca de outra sessao".into()));
                        } else if !hello.capabilities.iter().any(|c| c.name == cmd.capability) {
                            let m = format!("{} nao declarada por este no", cmd.capability);
                            let _ = tx.send(falha(&cmd, m));
                        } else {
                            let (ex, tx) = (executar.clone(), tx.clone());
                            tokio::spawn(async move {
                                let _ = tx.send(ex(cmd).await);
                            });
                        }
                        continue;
                    }
                    outro => {
                        return Err(DeviceTransportError::PairingRejected(format!(
                            "{outro}: {}",
                            String::from_utf8_lossy(&corpo)
                        )));
                    }
                }
            }
        };
        n += 1;
        client
            .send(&identidade.sign_envelope(b.session_uuid, n, tipo, &corpo)?)
            .await?;
    }
}

//! Nos de dispositivo como ferramenta: `node_list` (device.read) lista os nos pareados;
//! `node_invoke` (device.command) manda um comando a um no.
//!
//! Um comando so sai se a capacidade estiver nas TRES listas:
//! 1. declarada pelo NO no pareamento (`authorize_command`, no servidor);
//! 2. aprovada pelo OPERADOR no pareamento (o token que pareou o no, no servidor);
//! 3. concedida ao AGENTE: a capacidade do dispositivo tem de estar na politica do
//!    agente (`PHXCLAW_CAPACIDADES=...,device.command,device.system.info`) -- e a lista
//!    que esta ferramenta confere, ANTES de qualquer coisa sair pelo fio.
//!
//! As duas primeiras moram no `ServidorDispositivos::comandar`, que tambem recusa
//! capacidade protegida sem aprovacao humana, janela vencida e segredo cru nos
//! argumentos, e assina o envelope; o no responde assinado, dentro da sessao. Uma lista
//! de permitidos copiada aqui seria a que divergiria da do servidor.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_device_transport::servidor::{FalhaDeComando, ServidorDispositivos};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// A mesma ferramenta registrada duas vezes, uma por capacidade, como `git`/`git_write`.
pub struct DeviceTool {
    pub servidor: Arc<ServidorDispositivos>,
    pub invocar: bool,
    /// A terceira lista: capacidades de dispositivo concedidas ao agente.
    pub permitidas: BTreeSet<String>,
}

impl DeviceTool {
    /// `permitidas`: a politica do agente inteira; so as `device.*` contam.
    pub fn par(
        servidor: Arc<ServidorDispositivos>,
        politica: &BTreeSet<String>,
    ) -> [Arc<dyn Tool>; 2] {
        let permitidas: BTreeSet<String> = politica
            .iter()
            .filter(|c| c.starts_with("device.") && *c != "device.read" && *c != "device.command")
            .cloned()
            .collect();
        [
            Arc::new(Self {
                servidor: servidor.clone(),
                invocar: false,
                permitidas: permitidas.clone(),
            }),
            Arc::new(Self {
                servidor,
                invocar: true,
                permitidas,
            }),
        ]
    }
}

fn uuid(args: &Value, campo: &str) -> Result<Uuid, ToolError> {
    args.get(campo)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta '{campo}'")))?
        .parse()
        .map_err(|_| ToolError::InvalidArguments(format!("'{campo}' nao e UUID")))
}

impl Tool for DeviceTool {
    fn spec(&self) -> ToolSpec {
        if self.invocar {
            ToolSpec {
                name: "node_invoke".into(),
                description: "Invoke a command on a paired device node (see node_list). \
'capability' must be in the node's 'invocaveis' list (declared by the node, approved at \
pairing and granted to this agent). Protected ones (exec, power, file write, network change, \
secret rotation) need human approval and are refused. Never put secrets in arguments."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "node":{"type":"string","description":"node_uuid"},
                    "capability":{"type":"string"},
                    "arguments":{"type":"object"},
                    "timeout_s":{"type":"integer","minimum":1,"maximum":600}
                },"required":["node","capability"]}),
            }
        } else {
            ToolSpec {
                name: "node_list".into(),
                description: "List paired device nodes: uuid, name, state, whether connected \
now, and 'invocaveis' (capabilities node_invoke may use on it)."
                    .into(),
                parameters: json!({"type":"object","properties":{}}),
            }
        }
    }
    fn capability(&self) -> &'static str {
        if self.invocar {
            "device.command"
        } else {
            "device.read"
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let nos = self.servidor.nos();
            if !self.invocar {
                let v: Vec<Value> = nos
                    .into_iter()
                    .map(|n| {
                        let declaradas: Vec<&String> =
                            n.no.capabilities.iter().map(|c| &c.name).collect();
                        let invocaveis: Vec<&String> = declaradas
                            .iter()
                            .copied()
                            .filter(|c| n.aprovadas.contains(*c) && self.permitidas.contains(*c))
                            .collect();
                        json!({"node": n.no.node_uuid, "nome": n.no.display_name,
                               "estado": n.no.state, "conectado": n.conectado,
                               "ultimo_sinal": n.no.last_seen_at,
                               "declaradas": declaradas, "invocaveis": invocaveis})
                    })
                    .collect();
                return Ok(ToolOutput::text(json!({ "nos": v }).to_string()));
            }
            let no = uuid(&args, "node")?;
            let cap = args
                .get("capability")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'capability'".into()))?;
            if !self.permitidas.contains(cap) {
                return Err(ToolError::Denied(format!(
                    "capacidade de dispositivo {cap} nao concedida ao agente"
                )));
            }
            // o tenant sai do registro, nunca do modelo: ele nao escolhe de quem e o no
            let tenant = nos
                .iter()
                .find(|n| n.no.node_uuid == no)
                .map(|n| n.no.tenant_uuid)
                .ok_or_else(|| ToolError::InvalidArguments(format!("no {no} nao pareado")))?;
            let prazo = Duration::from_secs(
                args.get("timeout_s")
                    .and_then(Value::as_u64)
                    .unwrap_or(30)
                    .clamp(1, 600),
            )
            .min(ctx.timeout);
            let r = self
                .servidor
                .comandar(
                    tenant,
                    no,
                    cap,
                    args.get("arguments").cloned().unwrap_or(json!({})),
                    prazo,
                )
                .await
                .map_err(|e| match e {
                    FalhaDeComando::Negado(m) => ToolError::Denied(m),
                    FalhaDeComando::Falhou(m) => ToolError::Failed(m),
                })?;
            if !r.ok {
                return Err(ToolError::Failed(format!(
                    "o no recusou ou falhou: {}",
                    r.erro.unwrap_or_default()
                )));
            }
            Ok(ToolOutput::text(
                json!({"command": r.command_uuid, "saida": r.saida}).to_string(),
            ))
        })
    }
}

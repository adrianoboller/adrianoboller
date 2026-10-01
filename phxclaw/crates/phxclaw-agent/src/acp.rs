//! `phxclaw acp`: o agente falando Agent Client Protocol (o protocolo dos editores, v1)
//! pela entrada padrao, JSON-RPC 2.0 em linhas.
//!
//! Escrito a mao sobre o JSON-RPC do `phxclaw-mcp-lsp-runtime` (`encode_mcp_line`,
//! `jsonrpc_result`, `jsonrpc_error`) e sobre o leitor com teto do `mcp-serve`: a crate
//! `agent-client-protocol` traria 20 dependencias que nao estao no lock, e o protocolo
//! e pequeno. E o MESMO `Agent` da `Montagem` -- o editor nao ganha um motor a parte.
//!
//! O que cada coisa vira, e por que:
//!
//! - **`session/new`** guarda o `cwd` do editor. Cada `session/prompt` e uma tarefa do
//!   `TaskStore`, com a pasta de trabalho ligada ao `cwd` pela mesma `ligar_pasta` das
//!   tarefas em worktree: o agente edita o projeto aberto, com checkpoint e evidencia.
//! - **Passos viram `session/update`**: o pensamento em `agent_thought_chunk`, cada
//!   ferramenta em `tool_call` com o `kind` tirado da CAPACIDADE da ferramenta (a moeda do
//!   portao), nunca do nome -- ferramenta nova cai no tipo certo sem lista a atualizar.
//! - **Pergunta da tarefa** (o `ask_user` e o «perguntar» das regras) encerra o turno com
//!   a pergunta como mensagem, e o PROXIMO `session/prompt` da sessao e a resposta,
//!   entregue por `perguntas::responder` -- a mesma porta da API e da CLI. O
//!   `session/request_permission` do ACP so tem botoes, e a pergunta do `ask_user` e texto
//!   livre; decidir qual pergunta vira botao comparando a frase seria resolver por texto
//!   o que tem de ser resolvido por chave.
//! - **Conversa**: o turno novo leva as ultimas trocas da sessao no objetivo. A tarefa do
//!   motor nao continua de uma para outra; isso e o que o editor espera de um chat.

use crate::mcp::{Linha, ler_linha};
use crate::motor::{Agent, CancelFlag, Observer};
use crate::tarefa::{Task, TaskStatus};
use phxclaw_mcp_lsp_runtime::{
    JSONRPC_INVALID_PARAMS, JSONRPC_METHOD_NOT_FOUND, JSONRPC_PARSE_ERROR, encode_mcp_line,
    jsonrpc_error, jsonrpc_result,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

/// Versao do protocolo que este agente fala (o ACP numera por inteiro).
pub const VERSAO_ACP: u64 = 1;
/// Quantas trocas anteriores da sessao entram no objetivo do turno novo.
const TROCAS_NO_CONTEXTO: usize = 4;
/// Teto de cada lado de uma troca no contexto: resposta longa nao pode engolir o turno.
const TETO_DA_TROCA: usize = 2000;

enum Evento {
    Atualizar(Value),
    Pergunta(String),
    Fim(Box<Task>),
}

#[derive(Default)]
struct Turnos {
    historico: Vec<(String, String)>,
    /// Tarefa parada esperando resposta: o proximo prompt e a resposta dela.
    pendente: Option<String>,
    /// A tarefa do turno que corre ou espera.
    tarefa: Option<String>,
    eventos: Option<mpsc::UnboundedReceiver<Evento>>,
}

struct Sessao {
    cwd: PathBuf,
    /// Segurado pelo turno inteiro; `try_lock` falhando e «turno em andamento».
    turnos: tokio::sync::Mutex<Turnos>,
    /// Fora do `turnos`: o `session/cancel` chega enquanto o turno segura aquele.
    cancelar: Mutex<Option<CancelFlag>>,
}

struct Estado {
    agente: Agent,
    sessoes: Mutex<HashMap<String, Arc<Sessao>>>,
    saida: mpsc::UnboundedSender<Value>,
}

/// O tipo de ferramenta do ACP pela capacidade que o portao confere.
fn tipo_da_capacidade(cap: &str) -> &'static str {
    match cap {
        "fs.read" | "session.read" | "memory.read" | "skill.read" | "git.read" => "read",
        "fs.write" | "doc.write" | "memory.write" | "git.write" => "edit",
        "shell.exec" => "execute",
        "web.search" => "search",
        "web.browse" => "fetch",
        _ => "other",
    }
}

fn texto_do_prompt(prompt: &Value) -> String {
    let Some(blocos) = prompt.as_array() else {
        return String::new();
    };
    blocos
        .iter()
        .filter_map(|b| match b["type"].as_str()? {
            "text" => b["text"].as_str().map(str::to_string),
            // Recurso embutido: o editor mandou o conteudo do arquivo junto.
            "resource" => {
                let r = &b["resource"];
                Some(format!(
                    "[{}]\n{}",
                    r["uri"].as_str().unwrap_or(""),
                    r["text"].as_str().unwrap_or("")
                ))
            }
            "resource_link" => Some(format!("[arquivo: {}]", b["uri"].as_str().unwrap_or(""))),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn cortar(t: &str) -> String {
    phxclaw_agent_core::truncate_for_model(t, TETO_DA_TROCA)
}

fn objetivo_com_conversa(historico: &[(String, String)], pedido: &str) -> String {
    if historico.is_empty() {
        return pedido.to_string();
    }
    let ini = historico.len().saturating_sub(TROCAS_NO_CONTEXTO);
    let mut s = String::from("Conversa anterior nesta sessao do editor:\n");
    for (p, r) in &historico[ini..] {
        s.push_str(&format!(
            "- usuario: {}\n  agente: {}\n",
            cortar(p),
            cortar(r)
        ));
    }
    s.push_str(&format!("\nPedido atual:\n{pedido}"));
    s
}

/// Traduz cada passo novo do motor em `session/update`, e a pergunta em evento proprio.
struct Ponte {
    sessao: String,
    tipos: HashMap<String, &'static str>,
    tx: mpsc::UnboundedSender<Evento>,
    visto: Mutex<(usize, Option<String>, usize)>,
}

impl Ponte {
    fn atualizar(&self, u: Value) {
        let _ = self.tx.send(Evento::Atualizar(
            json!({"jsonrpc": "2.0", "method": "session/update",
                   "params": {"sessionId": self.sessao, "update": u}}),
        ));
    }
}

impl Observer for Ponte {
    fn on_update(&self, t: &Task) {
        let mut v = self.visto.lock().unwrap_or_else(|p| p.into_inner());
        if v.2 == 0 && !t.plan.is_empty() {
            v.2 = t.plan.len();
            self.atualizar(json!({"sessionUpdate": "plan", "entries": t.plan.iter()
                .map(|p| json!({"content": p, "priority": "medium", "status": "pending"}))
                .collect::<Vec<_>>()}));
        }
        // O motor grava a resposta final como passo de pensamento ANTES de preencher
        // `answer`: o ultimo pensamento espera o passo seguinte ou o fim, para nao sair
        // duas vezes (como pensamento e como mensagem).
        let fim = t.status.is_final();
        let mut ate = t.steps.len();
        if !fim && t.steps.last().is_some_and(|p| p.tool.is_none()) {
            ate -= 1;
        }
        let resposta = t.answer.as_deref().map(str::trim);
        for (i, p) in t.steps.iter().enumerate().take(ate).skip(v.0) {
            let e_a_resposta = fim && i + 1 == t.steps.len() && resposta == Some(p.summary.trim());
            match &p.tool {
                Some(nome) => {
                    let id = format!("{}-{}", t.id, p.n);
                    let estado = if p.outcome == "ok" {
                        "completed"
                    } else {
                        "failed"
                    };
                    self.atualizar(json!({"sessionUpdate": "tool_call", "toolCallId": id,
                        "title": nome, "kind": self.tipos.get(nome).copied().unwrap_or("other"),
                        "status": "in_progress", "rawInput": p.arguments}));
                    self.atualizar(
                        json!({"sessionUpdate": "tool_call_update", "toolCallId": id,
                        "status": estado, "content": [{"type": "content",
                        "content": {"type": "text", "text": p.summary}}]}),
                    );
                }
                None if !p.summary.trim().is_empty() && !e_a_resposta => {
                    self.atualizar(json!({"sessionUpdate": "agent_thought_chunk",
                        "content": {"type": "text", "text": p.summary}}));
                }
                None => {}
            }
        }
        v.0 = v.0.max(ate);
        match (&t.status, &t.question) {
            (TaskStatus::AwaitingInput, Some(q)) if v.1.as_ref() != Some(q) => {
                v.1 = Some(q.clone());
                let _ = self.tx.send(Evento::Pergunta(q.clone()));
            }
            (TaskStatus::AwaitingInput, _) => {}
            _ => v.1 = None,
        }
    }
}

/// Serve o ACP ate o fim da entrada. As respostas saem por uma fila so, escrita por uma
/// tarefa so: o turno que corre e o laco que le escrevem no mesmo fio, e duas escritas
/// concorrentes intercalariam bytes de duas linhas.
pub async fn servir<R, W>(agente: Agent, mut entrada: R, mut saida: W) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let escritor = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if saida.write_all(&encode_mcp_line(&v)).await.is_err() {
                break;
            }
            let _ = saida.flush().await;
        }
    });
    let estado = Arc::new(Estado {
        agente,
        sessoes: Mutex::new(HashMap::new()),
        saida: tx.clone(),
    });
    let mut r = Ok(());
    loop {
        let linha = match ler_linha(&mut entrada).await? {
            Linha::Fim => break,
            Linha::Grande(e) => {
                let _ = tx.send(e);
                r = Err(std::io::Error::other("mensagem ACP acima do teto"));
                break;
            }
            Linha::Bytes(b) => b,
        };
        if linha.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(&linha) {
            Err(e) => {
                let _ = tx.send(jsonrpc_error(
                    Value::Null,
                    JSONRPC_PARSE_ERROR,
                    &e.to_string(),
                ));
            }
            Ok(v) => despachar(&estado, v),
        }
    }
    // Fim da entrada: o editor fechou. Cancela o que corre e espera a fila esvaziar.
    let sessoes: Vec<_> = estado
        .sessoes
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .values()
        .cloned()
        .collect();
    for s in sessoes {
        if let Some(c) = s
            .cancelar
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            c.cancel();
        }
    }
    drop(tx);
    drop(estado);
    let _ = escritor.await;
    r
}

fn despachar(estado: &Arc<Estado>, v: Value) {
    let Some(metodo) = v.get("method").and_then(Value::as_str).map(str::to_string) else {
        // Resposta a algo que nao pedimos: nada a fazer.
        return;
    };
    let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
    let id = v.get("id").cloned();
    if metodo == "session/cancel" {
        if let Some(s) = sessao(estado, &params)
            && let Some(c) = s
                .cancelar
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
        {
            c.cancel();
        }
        return;
    }
    // Notificacao desconhecida nao tem resposta, nem a de erro.
    let Some(id) = id else { return };
    let responder = |r: Result<Value, (i64, String)>| {
        let _ = estado.saida.send(match r {
            Ok(v) => jsonrpc_result(id.clone(), v),
            Err((c, m)) => jsonrpc_error(id.clone(), c, &m),
        });
    };
    match metodo.as_str() {
        "initialize" => responder(Ok(json!({
            "protocolVersion": VERSAO_ACP,
            "agentCapabilities": {
                "loadSession": false,
                "promptCapabilities": {"image": false, "audio": false, "embeddedContext": true},
                "mcpCapabilities": {"http": false, "sse": false}
            },
            "authMethods": [],
            "agentInfo": {"name": "phxclaw", "version": env!("CARGO_PKG_VERSION")}
        }))),
        "authenticate" => responder(Ok(json!({}))),
        "session/new" => responder(nova_sessao(estado, &params)),
        "session/prompt" => {
            let estado = estado.clone();
            tokio::spawn(async move {
                let r = turno(&estado, &params).await;
                let _ = estado.saida.send(match r {
                    Ok(v) => jsonrpc_result(id, v),
                    Err((c, m)) => jsonrpc_error(id, c, &m),
                });
            });
        }
        outro => responder(Err((
            JSONRPC_METHOD_NOT_FOUND,
            format!("metodo nao suportado: {outro}"),
        ))),
    }
}

fn sessao(estado: &Estado, params: &Value) -> Option<Arc<Sessao>> {
    let id = params.get("sessionId")?.as_str()?;
    estado
        .sessoes
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .cloned()
}

fn nova_sessao(estado: &Estado, params: &Value) -> Result<Value, (i64, String)> {
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or((JSONRPC_INVALID_PARAMS, "falta 'cwd' absoluto".to_string()))?;
    let cwd = std::fs::canonicalize(&cwd)
        .ok()
        .filter(|p| p.is_dir())
        .ok_or((
            JSONRPC_INVALID_PARAMS,
            format!("cwd nao e uma pasta: {}", cwd.display()),
        ))?;
    let id = phxclaw_types::new_uuid_v7().to_string();
    estado
        .sessoes
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(
            id.clone(),
            Arc::new(Sessao {
                cwd,
                turnos: tokio::sync::Mutex::new(Turnos::default()),
                cancelar: Mutex::new(None),
            }),
        );
    Ok(json!({"sessionId": id}))
}

async fn turno(estado: &Arc<Estado>, params: &Value) -> Result<Value, (i64, String)> {
    let s = sessao(estado, params).ok_or((JSONRPC_INVALID_PARAMS, "sessao desconhecida".into()))?;
    let sid = params["sessionId"].as_str().unwrap_or_default().to_string();
    let mut turnos = s.turnos.try_lock().map_err(|_| {
        (
            JSONRPC_INVALID_PARAMS,
            "ja ha um turno em andamento nesta sessao".to_string(),
        )
    })?;
    let pedido = texto_do_prompt(&params["prompt"]);
    if pedido.trim().is_empty() {
        return Err((JSONRPC_INVALID_PARAMS, "prompt sem texto".into()));
    }
    // A tarefa parada esperando: este prompt e a resposta dela. Se ninguem espera mais
    // (cancelada entre um turno e outro), o prompt vira tarefa nova.
    let respondeu = turnos
        .pendente
        .take()
        .is_some_and(|tid| crate::perguntas::responder(&tid, &pedido).is_ok());
    if !respondeu {
        let (tx, rx) = mpsc::unbounded_channel();
        turnos.eventos = Some(rx);
        let agente = estado.agente.clone();
        let objetivo = objetivo_com_conversa(&turnos.historico, &pedido);
        let task = Task::new(objetivo, agente.llm.id());
        turnos.tarefa = Some(task.id.clone());
        crate::nuvem::ligar_pasta(&agente.store, &task.id, &s.cwd)
            .map_err(|e| (JSONRPC_INVALID_PARAMS, e.to_string()))?;
        let cancel = CancelFlag::default();
        *s.cancelar.lock().unwrap_or_else(|p| p.into_inner()) = Some(cancel.clone());
        let ponte = Ponte {
            sessao: sid.clone(),
            tipos: agente
                .tools
                .iter()
                .map(|t| (t.spec().name, tipo_da_capacidade(t.capability())))
                .collect(),
            tx: tx.clone(),
            visto: Mutex::new((0, None, 0)),
        };
        turnos.historico.push((pedido.clone(), String::new()));
        tokio::spawn(async move {
            let fim = agente.run(task, &cancel, &ponte).await;
            let _ = tx.send(Evento::Fim(Box::new(fim)));
        });
    }
    let mut rx = turnos
        .eventos
        .take()
        .ok_or((JSONRPC_INVALID_PARAMS, "sessao sem tarefa".to_string()))?;
    let mensagem = |texto: &str| {
        json!({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": texto}}}})
    };
    while let Some(ev) = rx.recv().await {
        match ev {
            Evento::Atualizar(v) => {
                let _ = estado.saida.send(v);
            }
            Evento::Pergunta(q) => {
                let _ = estado.saida.send(mensagem(&q));
                turnos.pendente = turnos.tarefa.clone();
                turnos.eventos = Some(rx);
                return Ok(json!({"stopReason": "end_turn"}));
            }
            Evento::Fim(t) => {
                *s.cancelar.lock().unwrap_or_else(|p| p.into_inner()) = None;
                let texto = match (&t.answer, &t.error) {
                    (Some(a), _) => a.clone(),
                    (None, Some(e)) => format!("erro: {e}"),
                    (None, None) => String::new(),
                };
                if !texto.is_empty() {
                    let _ = estado.saida.send(mensagem(&texto));
                }
                if let Some(ultima) = turnos.historico.last_mut() {
                    ultima.1 = texto;
                }
                let motivo = if t.status == TaskStatus::Cancelled {
                    "cancelled"
                } else {
                    "end_turn"
                };
                return Ok(json!({"stopReason": motivo}));
            }
        }
    }
    Err((JSONRPC_INVALID_PARAMS, "a tarefa terminou sem fim".into()))
}

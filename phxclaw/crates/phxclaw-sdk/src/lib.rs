#![forbid(unsafe_code)]
//! SDK Rust do PhxClaw: cliente da API HTTP de tarefas (`phxclaw servir`).
//!
//! Os tipos sao os que o servidor serializa (`phxclaw_agent_core::tarefa`), reexportados
//! aqui: o cliente le exatamente o `Task` que o servidor grava, e um campo novo de um lado
//! aparece do outro no mesmo build. O SDK depende so do contrato comum, nunca do agente
//! inteiro -- quem chama a API nao precisa compilar navegador, banco e servidor HTTP.
//!
//! ```no_run
//! # async fn f() -> Result<(), phxclaw_sdk::Erro> {
//! use phxclaw_sdk::{Cliente, NovaTarefa};
//! use std::time::Duration;
//! let c = Cliente::do_ambiente("http://127.0.0.1:8787")?;
//! let id = c.criar(&NovaTarefa { objective: "resuma o README".into(), ..Default::default() }).await?;
//! let t = c.aguardar(&id, Duration::from_secs(120)).await?;
//! println!("{:?}: {:?}", t.status, t.answer);
//! # Ok(()) }
//! ```

pub use phxclaw_agent_core::tarefa::{
    ErroDaApi, NovaTarefa, StepRecord, TarefaCriada, Task, TaskStatus, TaskSummary,
};
pub use phxclaw_agent_core::{Artifact, Usage};

use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum Erro {
    /// Recusa da API, com o codigo HTTP e a mensagem dela. `retry_after` vem no 429 do
    /// limite de criacao: e o servidor dizendo quando tentar.
    #[error("HTTP {status}: {erro}")]
    Api {
        status: u16,
        erro: String,
        retry_after: Option<u64>,
    },
    #[error("transporte: {0}")]
    Transporte(String),
    #[error("resposta ilegivel: {0}")]
    Formato(String),
    #[error(
        "sem token: passe-o, defina PHXCLAW_API_TOKEN ou aponte PHXCLAW_HOME para onde o \
`phxclaw servir` gravou o api.token"
    )]
    SemToken,
    #[error("tarefa {id} ainda em {estado:?} depois de {segundos}s")]
    Prazo {
        id: String,
        estado: TaskStatus,
        segundos: u64,
    },
}

/// Estados em que a tarefa nao anda mais sozinha. `AwaitingApproval` entra porque esperar
/// por ele e esperar por uma pessoa: o `aguardar` devolve e quem chama decide.
pub fn parada(s: TaskStatus) -> bool {
    s.is_final() || s == TaskStatus::AwaitingApproval
}

#[derive(Clone)]
pub struct Cliente {
    base: Url,
    token: String,
    http: reqwest::Client,
}

/// O token nunca aparece em `{:?}`, log ou mensagem de panico.
impl std::fmt::Debug for Cliente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cliente")
            .field("base", &self.base.as_str())
            .finish_non_exhaustive()
    }
}

/// O mesmo caminho do servidor: `PHXCLAW_API_TOKEN`, senao o `api.token` da pasta
/// (`PHXCLAW_HOME`, ou `var/agente`) que o `servir` grava na primeira vez.
pub fn token_do_ambiente(pasta: Option<&Path>) -> Option<String> {
    if let Ok(t) = std::env::var("PHXCLAW_API_TOKEN")
        && !t.is_empty()
    {
        return Some(t);
    }
    let raiz = pasta.map(Path::to_path_buf).unwrap_or_else(|| {
        std::env::var_os("PHXCLAW_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("var/agente"))
    });
    std::fs::read_to_string(raiz.join("api.token"))
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

impl Cliente {
    pub fn new(base: &str, token: impl Into<String>) -> Result<Self, Erro> {
        let token = token.into();
        if token.is_empty() {
            return Err(Erro::SemToken);
        }
        let mut base = Url::parse(base).map_err(|e| Erro::Transporte(format!("{base}: {e}")))?;
        // `path_segments_mut` recusa URL sem base de caminho (ex.: `data:`).
        if base.cannot_be_a_base() {
            return Err(Erro::Transporte(format!("URL sem caminho: {base}")));
        }
        if !base.path().ends_with('/') {
            let p = format!("{}/", base.path());
            base.set_path(&p);
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| Erro::Transporte(e.to_string()))?;
        Ok(Self { base, token, http })
    }

    /// Token pelo ambiente (ver `token_do_ambiente`).
    pub fn do_ambiente(base: &str) -> Result<Self, Erro> {
        Self::new(base, token_do_ambiente(None).ok_or(Erro::SemToken)?)
    }

    /// Cada pedaco vira um segmento de caminho com escape: um id com `/` ou `..` nao
    /// muda a ROTA pedida.
    fn url(&self, segmentos: &[&str]) -> Url {
        let mut u = self.base.clone();
        if let Ok(mut p) = u.path_segments_mut() {
            p.pop_if_empty().extend(segmentos);
        }
        u
    }

    async fn pedir(
        &self,
        metodo: Method,
        segmentos: &[&str],
        corpo: Option<&Value>,
    ) -> Result<reqwest::Response, Erro> {
        let mut r = self
            .http
            .request(metodo, self.url(segmentos))
            .bearer_auth(&self.token);
        if let Some(c) = corpo {
            r = r.json(c);
        }
        let resp = r
            .send()
            .await
            .map_err(|e| Erro::Transporte(e.without_url().to_string()))?;
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status().as_u16();
        let texto = resp.text().await.unwrap_or_default();
        Err(match serde_json::from_str::<ErroDaApi>(&texto) {
            Ok(e) => Erro::Api {
                status,
                erro: e.error,
                retry_after: e.retry_after,
            },
            Err(_) => Erro::Api {
                status,
                erro: texto,
                retry_after: None,
            },
        })
    }

    async fn json<T: DeserializeOwned>(
        &self,
        metodo: Method,
        segmentos: &[&str],
        corpo: Option<&Value>,
    ) -> Result<T, Erro> {
        let bytes = self
            .pedir(metodo, segmentos, corpo)
            .await?
            .bytes()
            .await
            .map_err(|e| Erro::Transporte(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| Erro::Formato(e.to_string()))
    }

    pub async fn saude(&self) -> Result<bool, Erro> {
        let v: Value = self.json(Method::GET, &["health"], None).await?;
        Ok(v["ok"] == true)
    }

    /// Cria e devolve o id; a tarefa roda em segundo plano no servidor.
    pub async fn criar(&self, pedido: &NovaTarefa) -> Result<String, Erro> {
        let corpo = json!(pedido);
        let c: TarefaCriada = self
            .json(Method::POST, &["v1", "tasks"], Some(&corpo))
            .await?;
        Ok(c.id)
    }

    pub async fn tarefa(&self, id: &str) -> Result<Task, Erro> {
        self.json(Method::GET, &["v1", "tasks", id], None).await
    }

    pub async fn listar(&self) -> Result<Vec<TaskSummary>, Erro> {
        self.json(Method::GET, &["v1", "tasks"], None).await
    }

    /// Sonda ate a tarefa parar (ver `parada`). Consultar nao gasta ficha do servidor.
    pub async fn aguardar(&self, id: &str, prazo: Duration) -> Result<Task, Erro> {
        let inicio = Instant::now();
        loop {
            let t = self.tarefa(id).await?;
            if parada(t.status) {
                return Ok(t);
            }
            if inicio.elapsed() >= prazo {
                return Err(Erro::Prazo {
                    id: id.to_string(),
                    estado: t.status,
                    segundos: prazo.as_secs(),
                });
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn editar_plano(&self, id: &str, passos: &[String]) -> Result<Task, Erro> {
        let corpo = json!({"steps": passos});
        self.json(Method::POST, &["v1", "tasks", id, "plan"], Some(&corpo))
            .await
    }

    pub async fn aprovar(&self, id: &str) -> Result<(), Erro> {
        self.pedir(
            Method::POST,
            &["v1", "tasks", id, "approve"],
            Some(&json!({})),
        )
        .await
        .map(|_| ())
    }

    /// Devolve o estado que o servidor relatou (`cancelling` ou `cancelled`).
    pub async fn cancelar(&self, id: &str) -> Result<String, Erro> {
        let v: Value = self
            .json(
                Method::POST,
                &["v1", "tasks", id, "cancel"],
                Some(&json!({})),
            )
            .await?;
        Ok(v["status"].as_str().unwrap_or_default().to_string())
    }

    /// Bytes de um artefato. O caminho e o `path` do `Artifact`, relativo a pasta da tarefa.
    pub async fn artefato(&self, id: &str, caminho: &str) -> Result<Vec<u8>, Erro> {
        let mut seg = vec!["v1", "tasks", id, "artifacts"];
        seg.extend(caminho.split('/').filter(|s| !s.is_empty()));
        let r = self.pedir(Method::GET, &seg, None).await?;
        r.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| Erro::Transporte(e.to_string()))
    }
}

//! A montagem do agente completo: um lugar so, usado pela CLI, pela API e pelos testes.
//! Duas montagens paralelas acabariam com politicas diferentes para o mesmo produto.

use crate::adaptadores::{BrowserSessions, WebSearchTool, browser_tools, office_tools};
use crate::ferramentas::{
    ListFilesTool, ParallelAgentsTool, ReadFileTool, ShellTool, WriteFileTool,
};
use crate::motor::{Agent, AgentConfig};
use crate::tarefa::TaskStore;
use phxclaw_agent_core::{Llm, Tool};
use phxclaw_browser::BrowserPolicy;
use phxclaw_egress_broker::{EgressBroker, EgressPolicy};
use std::sync::Arc;
use std::time::Duration;

/// Capacidades concedidas por padrao. Rede no shell fica de fora: o sandbox sem rede e a
/// garantia de que um comando gerado pelo modelo nao exfiltra nada.
pub const CAPACIDADES_PADRAO: &[&str] = &[
    "web.search",
    "web.browse",
    "fs.read",
    "fs.write",
    "doc.write",
    "shell.exec",
    "agent.spawn",
    "site.publish",
];

#[derive(Clone)]
pub struct Montagem {
    pub store: TaskStore,
    pub capabilities: Vec<String>,
    pub max_steps: u32,
    pub shell_network: bool,
    /// Sessoes de navegador compartilhadas por todas as tarefas do processo.
    pub browser: Arc<BrowserSessions>,
    pub search: Option<Arc<dyn phxclaw_web_search::SearchBackend>>,
}

impl Montagem {
    pub fn new(store: TaskStore) -> Self {
        let capabilities = std::env::var("PHXCLAW_CAPACIDADES")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| CAPACIDADES_PADRAO.iter().map(|s| s.to_string()).collect());
        let policy = BrowserPolicy {
            allowed_origins: vec![],
            allow_any_public: true,
            block_private_networks: true,
        };
        Self {
            store,
            capabilities,
            max_steps: 16,
            shell_network: false,
            browser: BrowserSessions::new(policy),
            search: search_backend().ok().map(Arc::from),
        }
    }

    /// Agente para um modelo ("ollama:qwen2.5:1.5b", "openai:...", ...).
    pub fn agent(&self, model_spec: &str) -> Result<Agent, String> {
        let llm = phxclaw_llm::from_env(model_spec).map_err(|e| e.to_string())?;
        Ok(self.agent_with(llm))
    }

    pub fn agent_with(&self, llm: Arc<dyn Llm>) -> Agent {
        let mut tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(WriteFileTool),
            Arc::new(ReadFileTool),
            Arc::new(ListFilesTool),
        ];
        if let Some(bwrap) = ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
        {
            tools.push(Arc::new(ShellTool {
                bwrap: bwrap.into(),
                network: self.shell_network,
                timeout: Duration::from_secs(120),
            }));
        }
        if let Some(b) = &self.search {
            tools.push(Arc::new(WebSearchTool { backend: b.clone() }));
        }
        if phxclaw_browser::find_chromium().is_some() {
            tools.extend(browser_tools(self.browser.clone()));
        }
        tools.extend(office_tools());
        tools.push(Arc::new(crate::site::PublishSiteTool {
            base_url: std::env::var("PHXCLAW_PUBLIC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8787".into()),
        }));
        // E-mail so existe se o operador configurou SMTP, e so roda se `mail.send` for
        // concedida explicitamente: nao esta no padrao.
        if let Some(c) = crate::email::SmtpConfig::from_env() {
            tools.push(Arc::new(crate::email::EmailTool { config: c }));
        }
        let caps: Vec<&str> = self.capabilities.iter().map(String::as_str).collect();
        let config = AgentConfig {
            max_steps: self.max_steps,
            require_final_tool: true,
            ..AgentConfig::default()
        }
        .grant(&caps);
        let paralelo: Arc<dyn Tool> = Arc::new(ParallelAgentsTool {
            llm: llm.clone(),
            tools: tools.clone(),
            config: config.clone(),
            store: self.store.clone(),
            max_parallel: 6,
        });
        tools.push(paralelo);
        Agent::new(llm, tools, config, self.store.clone())
    }
}

/// O broker da busca libera so as origens do buscador escolhido: pergunta-as ao backend
/// (montado com um broker fechado) e so entao monta o de verdade.
fn search_backend() -> Result<Box<dyn phxclaw_web_search::SearchBackend>, String> {
    let fechado = Arc::new(EgressBroker::new(EgressPolicy::default()));
    let origens = phxclaw_web_search::from_env(fechado)
        .map_err(|e| e.to_string())?
        .origins();
    let mut p = EgressPolicy {
        enabled: true,
        ..EgressPolicy::default()
    };
    p.allowed_origins.extend(origens);
    phxclaw_web_search::from_env(Arc::new(EgressBroker::new(p))).map_err(|e| e.to_string())
}

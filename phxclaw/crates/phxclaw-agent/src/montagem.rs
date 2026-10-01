//! A montagem do agente completo: um lugar so, usado pela CLI, pela API e pelos testes.
//! Duas montagens paralelas acabariam com politicas diferentes para o mesmo produto.

use crate::adaptadores::{BrowserSessions, WebSearchTool, browser_tools, office_tools};
use crate::ferramentas::{
    EditFileTool, ListFilesTool, ParallelAgentsTool, ReadFileTool, ShellTool, WriteFileTool,
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
    // Interacao: perguntar ao usuario so pausa a propria tarefa; ler o historico e o
    // resumo do dia so tocam o `TaskStore` do agente; a calculadora nao tem efeito nenhum.
    "user.ask",
    "session.read",
    "calc",
    // Memoria e skills so tocam arquivos do proprio agente, fora da pasta da tarefa e sem
    // rede; a gravacao tarja o que tem forma de segredo antes do disco.
    "memory.read",
    "memory.write",
    "skill.read",
    // Equipe: ler o catalogo e delegar a um papel. Delegar nao amplia poder -- o
    // subagente fica na interseccao do papel com o pai, sem rede nova nem recursao.
    "team.read",
    "team.delegate",
    // Git local e revisao: presos na pasta, sem rede, e o `shell.exec` ja concedido pode
    // mais que eles. GitHub/GitLab ficam fora: rede em nome do operador.
    "git.read",
    "git.write",
    "code.review",
    // Tarefas paralelas em worktree: nada alem de `agent.spawn` + `git.write`, ja no
    // padrao -- cada filha fica presa na propria worktree, sem rede.
    "agent.parallel",
    // Listar nos de dispositivo so le o registro; MANDAR comando (`device.command`) fica
    // fora do padrao, e a capacidade protegida ainda exige aprovacao humana no servidor.
    "device.read",
];

#[derive(Clone)]
pub struct Montagem {
    pub store: TaskStore,
    pub capabilities: Vec<String>,
    pub max_steps: u32,
    pub shell_network: bool,
    /// Estilo de saida (`PHXCLAW_ESTILO` ou `--estilo`): embutido ou `.phxclaw/estilos/`.
    pub estilo: Option<String>,
    /// Sessoes de navegador compartilhadas por todas as tarefas do processo.
    pub browser: Arc<BrowserSessions>,
    pub search: Option<Arc<dyn phxclaw_web_search::SearchBackend>>,
    /// Canal de mensagens ligado pelo `phxclaw canal telegram`; sem ele, `channel_send`
    /// nem existe.
    pub canal: Option<crate::canal::CanalTelegram>,
    /// Ferramentas dos servidores MCP declarados em `PHXCLAW_MCP_CONFIG`, descobertas uma
    /// vez na montagem. Capacidade `mcp.<servidor>`, fora do padrao: o operador concede.
    pub mcp: Vec<Arc<dyn Tool>>,
    /// Os papeis de `config/agents` (`PHXCLAW_AGENTES_DIR`). Catalogo invalido nao
    /// derruba o agente: vira aviso e `None`, e `team_list`/`team_delegate` nem existem.
    pub equipe: Option<Arc<crate::equipe::Equipe>>,
    /// GitHub/GitLab com token guardado em `<pasta>/forja` (`phxclaw forja token`); sem
    /// token, vazio.
    pub forjas: Vec<Arc<dyn Tool>>,
    /// Servidor de nos de dispositivo no mesmo processo (`phxclaw servir --dispositivos`);
    /// sem ele, `node_list`/`node_invoke` nem existem.
    pub dispositivos: Option<Arc<phxclaw_device_transport::servidor::ServidorDispositivos>>,
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
        let forjas =
            crate::forja::ferramentas_da_pasta(store.root().parent().unwrap_or(store.root()));
        Self {
            store,
            capabilities,
            max_steps: 16,
            shell_network: false,
            estilo: std::env::var("PHXCLAW_ESTILO")
                .ok()
                .filter(|e| !e.trim().is_empty()),
            browser: BrowserSessions::new(policy),
            search: search_backend().ok().map(Arc::from),
            canal: None,
            // Os servidores MCP dos pacotes assinados (Claude/Codex) entram na MESMA lista:
            // passam pela mesma regra de nome livre da montagem.
            mcp: {
                let mut v = crate::mcp::carregar_do_ambiente();
                v.extend(crate::pacotes::do_ambiente());
                v
            },
            equipe: carregar_equipe(crate::equipe::Equipe::do_ambiente()),
            forjas,
            dispositivos: None,
        }
    }

    /// Modelo local dos papeis que a planilha roteia para o Ollama
    /// (`PHXCLAW_MODELO_LOCAL`); spec invalida vira aviso, e o papel cai no modelo do pai.
    pub fn modelo_local(&self) -> Option<Arc<dyn Llm>> {
        let spec = std::env::var(crate::equipe::VAR_MODELO_LOCAL).ok()?;
        phxclaw_llm::from_env(&spec)
            .map_err(|e| eprintln!("aviso: {}={spec}: {e}", crate::equipe::VAR_MODELO_LOCAL))
            .ok()
    }

    /// Agente para um modelo ("ollama:qwen2.5:1.5b", "openai:...", ...).
    pub fn agent(&self, model_spec: &str) -> Result<Agent, String> {
        let llm = phxclaw_llm::from_env(model_spec).map_err(|e| e.to_string())?;
        self.montar(llm)
    }

    /// Como `agent`, com o modelo na mao. Nome de ferramenta repetido entre as nativas e
    /// defeito de montagem, nao de configuracao: para aqui, alto, em todo teste.
    pub fn agent_with(&self, llm: Arc<dyn Llm>) -> Agent {
        self.montar(llm)
            .unwrap_or_else(|e| panic!("montagem do agente: {e}"))
    }

    fn montar(&self, llm: Arc<dyn Llm>) -> Result<Agent, String> {
        let mut tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(WriteFileTool),
            Arc::new(ReadFileTool),
            Arc::new(ListFilesTool),
            Arc::new(EditFileTool),
            Arc::new(crate::ui::DesignErpUiTool),
            Arc::new(crate::ui::ScreenshotToErpUiTool),
        ];
        // O mesmo achador dos conversores de `arquivos`: shell e conversor no MESMO bwrap.
        if let Some(bwrap) = crate::arquivos::achar_bwrap() {
            tools.push(Arc::new(ShellTool {
                bwrap: bwrap.clone(),
                network: self.shell_network,
                timeout: Duration::from_secs(120),
            }));
            tools.push(Arc::new(crate::ferramentas::BackgroundShellTool::new(
                bwrap,
                self.shell_network,
            )));
        }
        if let Some(b) = &self.search {
            tools.push(Arc::new(WebSearchTool { backend: b.clone() }));
        }
        if phxclaw_browser::find_chromium().is_some() {
            tools.extend(browser_tools(self.browser.clone()));
            tools.push(Arc::new(crate::visao::ImageRenderTool));
        }
        // Visao e voz: o ocr e o image so leem a pasta. O transcribe pede `media.stt` e o
        // desktop `desktop.control`, nenhuma das duas no padrao; o desktop so existe com a
        // feature `desktop`, que liga as bibliotecas de sessao grafica no binario.
        tools.push(Arc::new(crate::visao::OcrTool));
        tools.push(Arc::new(crate::visao::ImageInfoTool));
        tools.push(Arc::new(crate::visao::TranscribeTool::from_env()));
        // Fala, gatilho e imagem: capacidades fora do padrao (`media.tts`, `media.wake`,
        // `media.generate`); sem configuracao, cada uma recusa dizendo a variavel que falta.
        tools.push(Arc::new(crate::voz::SpeakTool::from_env()));
        tools.push(Arc::new(crate::voz::WakeWordTool::from_env()));
        tools.push(Arc::new(
            crate::midia::ImageGenerateTool::from_env().unwrap_or_else(|e| {
                eprintln!("aviso: geracao de imagem so local (svg): {e}");
                crate::midia::ImageGenerateTool { provedor: None }
            }),
        ));
        #[cfg(feature = "desktop")]
        tools.push(Arc::new(crate::visao::DesktopTool));
        tools.extend(office_tools());
        tools.extend(crate::arquivos::arquivos_tools());
        let base_url =
            std::env::var("PHXCLAW_PUBLIC_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
        // O canvas e o site publicado sao o mesmo risco (conteudo do agente servido em
        // origem opaca), com a mesma base e a mesma capacidade.
        tools.push(Arc::new(crate::canvas::CanvasTool {
            base_url: base_url.clone(),
        }));
        tools.push(Arc::new(crate::site::PublishSiteTool { base_url }));
        // Fala com chat so se o canal foi ligado, e so roda com `channel.send` concedida.
        if let Some(c) = &self.canal {
            tools.push(Arc::new(crate::canal::ChannelSendTool { canal: c.clone() }));
        }
        // E-mail so existe se o operador configurou SMTP, e so roda se `mail.send` for
        // concedida explicitamente: nao esta no padrao.
        if let Some(c) = crate::email::SmtpConfig::from_env() {
            tools.push(Arc::new(crate::email::EmailTool { config: c }));
        }
        // Maquina Linux: cada par leitura/mudanca e a mesma ferramenta registrada duas
        // vezes, uma por capacidade, para o portao do motor continuar sendo o unico.
        // Nenhuma destas capacidades esta no padrao.
        tools.push(Arc::new(crate::sistema::LinuxSystemTool::leitura()));
        tools.push(Arc::new(crate::sistema::LinuxSystemTool::admin()));
        tools.push(Arc::new(crate::sistema::NetworkTool::do_ambiente(false)));
        tools.push(Arc::new(crate::sistema::NetworkTool::do_ambiente(true)));
        // Banco so existe se o operador deu a URL; o modelo nunca a escolhe.
        for write in [false, true] {
            if let Some(pg) = crate::sistema::PostgresTool::do_ambiente(write) {
                tools.push(Arc::new(pg));
            }
        }
        if let Some(rust) =
            crate::arquivos::achar_bwrap().and_then(crate::sistema::RustProjectTool::detectar)
        {
            tools.push(Arc::new(rust));
        }
        // Python no mesmo bwrap; sem interpretador a ferramenta nao se registra.
        if let Some(py) = crate::arquivos::achar_bwrap()
            .and_then(|b| crate::python::PythonProjectTool::detectar(b).ok())
        {
            // O REPL usa as MESMAS extras do `python_project`: um interpretador so.
            let py = Arc::new(py);
            tools.push(Arc::new(crate::repl::PythonReplTool::new(py.clone())));
            tools.push(py);
        }
        tools.push(Arc::new(crate::calculadora::CalculatorTool));
        tools.push(Arc::new(crate::sessoes::SessionSearchTool {
            store: self.store.clone(),
        }));
        tools.push(Arc::new(crate::sessoes::DailySummaryTool {
            store: self.store.clone(),
        }));
        // Memoria e skills: o mesmo `Memoria` e a mesma pasta vao para as ferramentas e para
        // o motor, que injeta no comeco da tarefa o que a ferramenta acharia.
        let memoria = crate::memoria::Memoria::do_ambiente(self.store.root());
        let skills = crate::skills::pasta_do_ambiente(self.store.root());
        tools.push(Arc::new(crate::memoria::MemorySaveTool {
            memoria: memoria.clone(),
        }));
        tools.push(Arc::new(crate::memoria::MemorySearchTool {
            memoria: memoria.clone(),
        }));
        tools.push(Arc::new(crate::skills::SkillLoadTool {
            pasta: skills.clone(),
        }));
        tools.extend(crate::git::ferramentas_de_codigo(&self.store, llm.clone()));
        tools.extend(self.forjas.iter().cloned());
        // `x_search` so com chave da xAI guardada (`phxclaw xai chave`); `x.search` fora do
        // padrao: e conta paga do operador.
        if let Some(x) = crate::xai::XSearchTool::da_pasta(
            self.store.root().parent().unwrap_or(self.store.root()),
        ) {
            tools.push(Arc::new(x));
        }
        // Plugins assinados com o ponto `agent.tool` (`PHXCLAW_PLUGINS_RAIZ`); a capacidade
        // primaria de cada um passa pelo mesmo portao do motor.
        tools.extend(crate::plugins::do_ambiente(
            &self.capabilities.iter().cloned().collect(),
        ));
        if let Some(d) = &self.dispositivos {
            tools.extend(crate::dispositivos::DeviceTool::par(
                d.clone(),
                &self.capabilities.iter().cloned().collect(),
            ));
        }
        // Servidor de linguagem: `lsp` e o diagnostico anexado a `write_file`/`edit_file`.
        // Antes do MCP, para um `lsp` de fora nao sombrear o nativo.
        crate::lsp::ligar(&mut tools);
        // MCP entra por ultimo e so com nome livre: servidor de fora que declare `shell`
        // nao pode sombrear o nativo (o portao acha a ferramenta pelo nome). E antes das
        // copias para os subagentes, para todos verem a mesma lista.
        for m in &self.mcp {
            let nome = m.spec().name;
            if tools.iter().any(|t| t.spec().name == nome) {
                eprintln!("aviso: ferramenta MCP '{nome}' ignorada: o nome ja existe no agente");
            } else {
                tools.push(m.clone());
            }
        }
        let caps: Vec<&str> = self.capabilities.iter().map(String::as_str).collect();
        let config = AgentConfig {
            max_steps: self.max_steps,
            require_final_tool: true,
            memoria: Some(memoria),
            skills: Some(skills),
            hooks: self.hooks(),
            regras: self.regras(),
            estilo: self.estilo_de_saida(),
            prazo_de_resposta: Some(PRAZO_DE_RESPOSTA),
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
        // As tarefas em worktree recebem a mesma base (sem ferramenta de subagente).
        let em_worktree: Option<Arc<dyn Tool>> = crate::arquivos::achar_bwrap().map(|bwrap| {
            Arc::new(crate::nuvem::ParallelTasksTool {
                llm: llm.clone(),
                tools: tools.clone(),
                config: config.clone(),
                store: self.store.clone(),
                bwrap,
                max_parallel: 6,
            }) as Arc<dyn Tool>
        });
        // A equipe recebe a MESMA base do `parallel_research` (antes de ele entrar na
        // lista): o subagente de papel nao herda nenhuma ferramenta de subagente.
        if let Some(equipe) = &self.equipe {
            let base = Agent::new(
                llm.clone(),
                tools.clone(),
                config.clone(),
                self.store.clone(),
            );
            tools.push(Arc::new(crate::equipe::TeamListTool {
                equipe: equipe.clone(),
            }));
            tools.push(Arc::new(crate::equipe::TeamDelegateTool {
                equipe: equipe.clone(),
                base,
                local: self.modelo_local(),
            }));
        }
        tools.extend(em_worktree);
        tools.push(paralelo);
        let repetidos = nomes_repetidos(&tools);
        if !repetidos.is_empty() {
            return Err(format!(
                "nome de ferramenta repetido: {}",
                repetidos.join(", ")
            ));
        }
        Ok(Agent::new(llm, tools, config, self.store.clone()))
    }
}

/// Nomes que aparecem mais de uma vez, em ordem.
pub fn nomes_repetidos(tools: &[Arc<dyn Tool>]) -> Vec<String> {
    let mut vistos = std::collections::BTreeSet::new();
    let mut rep = std::collections::BTreeSet::new();
    for t in tools {
        let n = t.spec().name;
        if !vistos.insert(n.clone()) {
            rep.insert(n);
        }
    }
    rep.into_iter().collect()
}

/// Quanto uma pergunta ao usuario espera antes de o agente seguir pelo proprio juizo.
/// Longo de proposito: quem responde e gente, pela API ou pelo terminal.
pub const PRAZO_DE_RESPOSTA: Duration = Duration::from_secs(6 * 3600);

/// A pasta de configuracao do projeto: `$PHXCLAW_PROJETO/.phxclaw`, ou `.phxclaw` na
/// pasta corrente. Fica FORA do `work/` das tarefas: o que esta aqui manda no agente
/// (hooks, regras), e o shell do modelo nao pode reescreve-lo.
pub fn pasta_do_projeto() -> Option<std::path::PathBuf> {
    let raiz = std::env::var_os("PHXCLAW_PROJETO")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(raiz.join(".phxclaw")).filter(|p| p.is_dir())
}

impl Montagem {
    fn hooks(&self) -> Option<Arc<crate::hooks::Hooks>> {
        let p = pasta_do_projeto()?;
        let h = crate::hooks::Hooks::carregar(&p, crate::arquivos::achar_bwrap())?;
        if let Some(e) = &h.erro {
            eprintln!("aviso: {e}: PreToolUse fechado ate o arquivo se ler");
        }
        Some(Arc::new(h))
    }

    /// Arquivo de regras ilegivel nega todo comando: o operador escreveu regra para
    /// restringir, e ignora-la calado deixaria passar o que ele quis barrar.
    fn regras(&self) -> Option<Arc<crate::regras::RegrasDeComando>> {
        let arq = pasta_do_projeto()?.join("regras.json");
        match crate::regras::RegrasDeComando::carregar(&arq) {
            Ok(r) => r.map(Arc::new),
            Err(e) => {
                eprintln!("aviso: {e}: todo comando de shell negado ate o arquivo se ler");
                Some(Arc::new(crate::regras::RegrasDeComando::negar_tudo(&e)))
            }
        }
    }

    /// Estilo pedido e inexistente vira aviso e nenhum estilo: a tarefa nao deixa de
    /// rodar por causa da forma da resposta.
    fn estilo_de_saida(&self) -> Option<crate::estilos::EstiloDeSaida> {
        let nome = self.estilo.as_deref()?;
        crate::estilos::carregar(nome, pasta_do_projeto().as_deref())
            .map_err(|e| eprintln!("aviso: {e}"))
            .ok()
    }
}

/// Catalogo carregado vira equipe; falha vira aviso no stderr (o stdout do `mcp-serve` e
/// fio de protocolo) e agente sem equipe, nunca agente que nao sobe.
pub fn carregar_equipe(
    r: Result<crate::equipe::Equipe, String>,
) -> Option<Arc<crate::equipe::Equipe>> {
    r.map_err(|e| eprintln!("aviso: agente sem equipe: {e}"))
        .ok()
        .map(Arc::new)
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

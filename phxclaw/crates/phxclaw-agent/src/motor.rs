//! O motor: planeja, executa com ferramentas, registra cada passo e termina.
//!
//! O laco e o de todo agente de ferramenta: o modelo responde; se pediu ferramentas, cada
//! uma passa pela politica (capacidade concedida ou negada), roda com prazo, vira evidencia
//! com hash encadeado e volta ao modelo como resultado; se nao pediu, a resposta e final.
//! Teto de passos e de tokens sempre presente, e cancelamento conferido a cada volta.

use crate::tarefa::{StepRecord, Task, TaskStatus, TaskStore};
use chrono::Utc;
use phxclaw_agent_core::{
    Artifact, Llm, LlmOptions, Message, Tool, ToolCall, ToolContext, ToolError, ToolOutput,
    ToolSpec, truncate_for_model,
};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_steps: u32,
    /// Caracteres de resultado de ferramenta devolvidos ao modelo por chamada.
    pub max_tool_output_chars: usize,
    pub tool_timeout: Duration,
    pub options: LlmOptions,
    /// Politica: so as ferramentas cuja capacidade esta aqui rodam. Nega por padrao.
    pub capabilities: BTreeSet<String>,
    /// Instrucao extra do operador, somada ao prompt de sistema.
    pub extra_instructions: Option<String>,
    /// O fim e uma ferramenta explicita (`final_answer`). Medido com modelo pequeno: ele
    /// terminava NARRANDO a acao ("I will create rust.xlsx") sem chamar a ferramenta.
    /// Resposta so em texto recebe ate 2 lembretes antes de ser aceita como final.
    pub require_final_tool: bool,
    /// Memoria entre tarefas. Com ela e com `memory.read` concedida, o motor injeta no
    /// comeco as memorias mais relevantes para o objetivo. Sem ela, nada se le do disco.
    pub memoria: Option<crate::memoria::Memoria>,
    /// Pasta de skills. Com ela e com `skill.read` concedida, o prompt lista nome e
    /// descricao de cada skill.
    pub skills: Option<phxclaw_skill_runtime::SkillFolder>,
    /// Hooks do projeto (`.phxclaw/hooks.json`), disparados pelos eventos do laco.
    pub hooks: Option<Arc<crate::hooks::Hooks>>,
    /// Regras por comando de shell, conferidas no portao antes de a ferramenta rodar.
    pub regras: Option<Arc<crate::regras::RegrasDeComando>>,
    /// Estilo de saida somado ao prompt de sistema.
    pub estilo: Option<crate::estilos::EstiloDeSaida>,
    /// Os `AGENTS.md` do projeto confiado, ja varridos e cercados como dado do projeto
    /// (`instrucoes::do_projeto`). Lidos uma vez na montagem, nao a cada tarefa.
    pub instrucoes_projeto: Option<String>,
    /// Comandos de barra (`/nome args`) do projeto e dos pacotes: o objetivo que comeca
    /// por um deles vira o corpo do comando na mensagem do usuario.
    pub comandos: Option<Arc<crate::comandos::ComandosDeBarra>>,
    /// Com prazo, o modelo ganha o `ask_user` e a regra `perguntar` tem a quem perguntar.
    /// Sem prazo (subagente, `mcp-serve`), ninguem acompanha a tarefa: `perguntar` nega.
    pub prazo_de_resposta: Option<Duration>,
    /// Novas tentativas por ferramenta depois de um argumento invalido, contadas SEGUIDAS
    /// (a chamada que passa zera, como no PydanticAI). Esgotou, a tarefa termina FALHA.
    /// 2 e o medido: com 1, o 3B morria na primeira correcao que errava outro campo.
    pub tentativas_de_argumento: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 16,
            max_tool_output_chars: 6_000,
            tool_timeout: Duration::from_secs(120),
            options: LlmOptions::default(),
            capabilities: BTreeSet::new(),
            extra_instructions: None,
            require_final_tool: false,
            memoria: None,
            skills: None,
            hooks: None,
            regras: None,
            estilo: None,
            instrucoes_projeto: None,
            comandos: None,
            prazo_de_resposta: None,
            tentativas_de_argumento: 2,
        }
    }
}

impl AgentConfig {
    pub fn grant(mut self, caps: &[&str]) -> Self {
        self.capabilities.extend(caps.iter().map(|c| c.to_string()));
        self
    }
}

/// Sinal de cancelamento compartilhado entre a API e a execucao.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Observador de progresso (a API e a tela assinam; o motor nao sabe quem e).
pub trait Observer: Send + Sync {
    fn on_update(&self, task: &Task);
}

pub struct NoObserver;
impl Observer for NoObserver {
    fn on_update(&self, _task: &Task) {}
}

#[derive(Clone)]
pub struct Agent {
    pub llm: Arc<dyn Llm>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub config: AgentConfig,
    pub store: TaskStore,
}

const PROMPT_BASE: &str = "You are PhxClaw, an autonomous agent that completes the user's objective \
by calling tools. Work step by step. Use tools to get real information and to produce files; \
never invent facts, URLs or file contents you did not obtain from a tool. Files you create live \
in the task working directory; mention their relative paths in the final answer. When the \
objective is complete, reply with the final answer in the user's language WITHOUT calling any \
tool. If a tool is denied by policy, do not retry it; work with what is allowed or explain.";

/// O que o Plan Mode deixa o modelo usar: so ler e pesquisar. A lista e de CAPACIDADES, a
/// mesma moeda do portao, e nao de nomes de ferramenta -- ferramenta nova de leitura entra
/// no plano sem ninguem lembrar de atualizar uma segunda lista.
pub const CAPACIDADES_DE_LEITURA: &[&str] = &[
    "fs.read",
    "web.search",
    "memory.read",
    "skill.read",
    // Indice de documentos da pasta do agente: so le.
    "doc.read",
    "team.read",
    "session.read",
    "db.read",
    "calc",
    "user.ask",
    "git.read",
    "code.review",
    "system.read",
    "net.diagnose",
    "github.read",
    "gitlab.read",
    "media.stt",
    "media.wake",
    "device.read",
    // Busca no X pela xAI: so le (conta do operador, fora do padrao).
    "x.search",
    // Previsao do tempo do MET Norway: servico publico sem conta, so le (fora do padrao).
    "weather.read",
    // Lista de vozes da ElevenLabs: so le a conta do operador (fora do padrao).
    "media.voices",
];

/// O resto: toda capacidade que muda algo (disco, rede, processo, conta de terceiro). Cada
/// capacidade conhecida esta numa das duas listas, e a catraca dos testes reprova a que
/// nascer sem classificacao -- uma capacidade nova cairia calada fora do Plan Mode ou,
/// pior, dentro dele. As de servidor MCP (`mcp.<servidor>`) contam como escrita: o que a
/// ferramenta de fora faz, o agente nao sabe.
pub const CAPACIDADES_QUE_ESCREVEM: &[&str] = &[
    "fs.write",
    "doc.write",
    "shell.exec",
    "agent.spawn",
    "site.publish",
    "memory.write",
    "team.delegate",
    "web.browse",
    // Pesquisa profunda abre paginas como o `web.browse`: GET com dado na URL tambem sai.
    "web.research",
    "git.write",
    "system.admin",
    "net.lan",
    "db.write",
    "mail.send",
    "channel.send",
    "desktop.control",
    "github.write",
    "gitlab.write",
    "agent.parallel",
    "device.command",
    "media.tts",
    "media.generate",
    // Parecer Go/NoGo do conselho de integradores: grava na pasta do agente.
    "gonogo.write",
    // Fluxos do n8n do operador: `run` dispara trabalho em outro sistema (fora do padrao).
    "automacao.n8n",
];

/// Capacidades cujas ferramentas criam processo. Para elas a regra de comando vale MESMO
/// quando a ferramenta nao declara a linha (`comando_de_shell` devolve `None`): o portao
/// confere a linha sintetica `<ferramenta> <action>`, e o `padrao` do arquivo de regras
/// alcanca tudo. Sem isto o padrao aberto do trait seria a porta lateral do `negar` --
/// medido: `python_repl`, `git_write` e `rust_project` rodavam sob `padrao: negar`.
pub const CAPACIDADES_QUE_EXECUTAM: &[&str] = &[
    "shell.exec",
    "git.read",
    "git.write",
    "code.review",
    "system.read",
    "system.admin",
    "net.diagnose",
    "net.lan",
    // Motores de midia: lancam programa de terceiro (whisper, voz, detector de gatilho).
    "media.stt",
    "media.tts",
    "media.wake",
];

const PROMPT_PLANO: &str = "PLAN MODE: you can only read and research (the tools you see are read-only); you must not create, change or run anything. Investigate what you need, then finish with the plan as JSON: {\"steps\": [\"short step\", ...]} with 2 to 12 concrete steps, in the user's language. The plan is shown to the user, who approves it before anything executes.";

const ASK_USER: &str = "ask_user";

/// Desfecho do passo cujo argumento nao passou: pelo esquema no portao, ou pela propria
/// ferramenta (`ToolError::InvalidArguments`). E ele que o orcamento e o fim conferem.
pub const DESFECHO_INVALIDO: &str = "invalido";

/// Por que o fim nao foi aceito: o texto que volta ao modelo (ingles, como o resto do
/// prompt) e o erro da tarefa quando as recusas acabam.
struct FimRecusado {
    ao_modelo: String,
    erro: String,
}

/// Como a espera de uma pergunta terminou.
enum Resposta {
    Texto(String),
    PrazoEsgotado,
    Cancelada,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn Llm>,
        tools: Vec<Arc<dyn Tool>>,
        config: AgentConfig,
        store: TaskStore,
    ) -> Self {
        Self {
            llm,
            tools,
            config,
            store,
        }
    }

    /// Ferramentas que a politica deixa o modelo VER. As negadas nem aparecem: modelo que
    /// ve uma ferramenta proibida tenta usa-la e gasta passos.
    pub fn visible_specs(&self) -> Vec<ToolSpec> {
        self.specs_de(&self.config.capabilities)
    }

    fn specs_de(&self, caps: &BTreeSet<String>) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .filter(|t| caps.contains(t.capability()))
            .map(|t| t.spec())
            .collect()
    }

    /// As capacidades efetivas: no Plan Mode, a interseccao com as de leitura. Conferida na
    /// lista E no portao, porque o modelo pode pedir pelo nome o que nao lhe foi mostrado.
    fn capacidades(&self, plano: bool) -> BTreeSet<String> {
        if plano {
            self.config
                .capabilities
                .iter()
                .filter(|c| CAPACIDADES_DE_LEITURA.contains(&c.as_str()))
                .cloned()
                .collect()
        } else {
            self.config.capabilities.clone()
        }
    }

    fn pode_perguntar(&self, caps: &BTreeSet<String>) -> bool {
        self.config.prazo_de_resposta.is_some() && caps.contains("user.ask")
    }

    /// Plan Mode: o MESMO laco da execucao, so com as ferramentas de leitura, terminando
    /// num plano em vez de numa resposta. A tarefa fica em `AwaitingApproval` e so executa
    /// pelo fluxo de aprovacao de sempre (`/approve` na API, a confirmacao na CLI). Se o
    /// modelo nao devolver JSON, o plano sai das linhas da resposta, ou vira o objetivo.
    pub async fn plan(&self, task: &mut Task) -> Result<(), String> {
        let fim = self
            .executar_modo(task.clone(), &CancelFlag::default(), &NoObserver, true)
            .await;
        *task = fim;
        match task.status {
            TaskStatus::AwaitingApproval => Ok(()),
            s => Err(task
                .error
                .clone()
                .unwrap_or_else(|| format!("plano terminou em {s:?}"))),
        }
    }

    /// Executa a tarefa ate a resposta final, o teto de passos, erro ou cancelamento.
    /// Grava o estado a cada passo e devolve a tarefa final.
    pub async fn run(&self, task: Task, cancel: &CancelFlag, obs: &dyn Observer) -> Task {
        self.executar_modo(task, cancel, obs, false).await
    }

    async fn executar_modo(
        &self,
        task: Task,
        cancel: &CancelFlag,
        obs: &dyn Observer,
        plano: bool,
    ) -> Task {
        let id = task.id.clone();
        let fim = self.run_inner(task, cancel, obs, plano).await;
        // Fim de tarefa nao bloqueia: a tarefa ja terminou, o hook so e avisado.
        self.disparar_hook(
            crate::hooks::Evento::FimDaTarefa,
            None,
            &self.store.workdir(&id),
            json!({"hook_event_name": "TaskEnd", "task_id": id, "status": fim.status,
                   "answer": fim.answer, "error": fim.error}),
        )
        .await;
        // Solta o que as ferramentas seguraram para esta tarefa (navegador aberto etc.).
        for t in &self.tools {
            t.finish(&id).await;
        }
        fim
    }

    async fn run_inner(
        &self,
        mut task: Task,
        cancel: &CancelFlag,
        obs: &dyn Observer,
        plano: bool,
    ) -> Task {
        task.status = TaskStatus::Running;
        task.updated_at = Utc::now();
        self.persist(&task, obs);
        let ledger = match EvidenceLedger::open(self.store.evidence_path(&task.id)) {
            Ok(l) => l,
            Err(e) => {
                return self.finish(
                    task,
                    TaskStatus::Failed,
                    Some(format!("evidencia: {e}")),
                    obs,
                );
            }
        };
        let ctx = ToolContext {
            task_id: task.id.clone(),
            workdir: self.store.workdir(&task.id),
            timeout: self.config.tool_timeout,
        };
        let _ = std::fs::create_dir_all(&ctx.workdir);
        let caps = self.capacidades(plano);
        let pode_perguntar = self.pode_perguntar(&caps);
        let mut specs = self.specs_de(&caps);
        // Verificacao pedida que nao pode rodar fecha AGORA, e nao depois do trabalho todo:
        // e a regra dos hooks (guarda configurada que nao roda, fecha), e o `shell.exec`
        // negado nao pode voltar pela porta do `verificar`.
        if !plano && let Some(cmd) = task.verificar.clone() {
            let motivo = if !caps.contains("shell.exec") {
                Some("a capacidade shell.exec nao foi concedida")
            } else if crate::arquivos::achar_bwrap().is_none() {
                Some("nao ha bwrap para roda-lo no sandbox")
            } else {
                None
            };
            if let Some(m) = motivo {
                return self.finish(
                    task,
                    TaskStatus::Failed,
                    Some(format!("verificacao `{cmd}` pedida e {m}")),
                    obs,
                );
            }
        }
        if self.config.require_final_tool {
            let (descricao, parametros) = match &task.saida_esquema {
                // Saida tipada: o `answer` e o objeto do esquema, e o mesmo validador do
                // portao o confere antes de o fim ser aceito.
                Some(e) => (
                    "Call this ONLY when everything the user asked is done. `answer` MUST be JSON \
matching the given schema (an object, not prose)."
                        .to_string(),
                    json!({"type":"object","properties":{"answer": e},"required":["answer"]}),
                ),
                None => (
                    "Call this ONLY when everything the user asked is done (all requested files created). \
The answer is shown to the user."
                        .to_string(),
                    json!({"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"]}),
                ),
            };
            specs.push(ToolSpec {
                name: "final_answer".into(),
                description: descricao,
                parameters: parametros,
            });
        }
        if pode_perguntar {
            specs.push(ToolSpec {
                name: ASK_USER.into(),
                description: "Ask the user a question and WAIT for the answer. Use only when you cannot \
proceed without a decision or information that only the user has; do not ask what a tool can find."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "question":{"type":"string"},
                    "options":{"type":"array","items":{"type":"string"},"description":"optional choices"}
                },"required":["question"]}),
            });
        }
        // Inicio da tarefa: sair com 2 impede a tarefa; o stdout vira contexto do prompt.
        let inicio = self
            .disparar_hook(
                crate::hooks::Evento::InicioDaTarefa,
                None,
                &ctx.workdir,
                json!({"hook_event_name": "TaskStart", "task_id": task.id,
                       "objective": task.objective, "plan_mode": plano}),
            )
            .await;
        if let Some(m) = inicio.bloqueio {
            return self.finish(
                task,
                TaskStatus::Failed,
                Some(format!("hook TaskStart bloqueou: {m}")),
                obs,
            );
        }
        let mut lembretes = 0u32;
        let mut bloqueios_stop = 0u32;
        let mut sistema = PROMPT_BASE.to_string();
        if plano {
            sistema.push_str("\n\n");
            sistema.push_str(PROMPT_PLANO);
        } else if !task.plan.is_empty() {
            sistema.push_str("\n\nApproved plan:\n");
            for (i, p) in task.plan.iter().enumerate() {
                sistema.push_str(&format!("{}. {p}\n", i + 1));
            }
        }
        if let Some(x) = &self.config.extra_instructions {
            sistema.push_str("\n\n");
            sistema.push_str(x);
        }
        if let Some(e) = &self.config.estilo {
            sistema.push_str("\n\n");
            sistema.push_str(&crate::estilos::bloco_para_o_prompt(e));
        }
        if let Some(i) = &self.config.instrucoes_projeto {
            sistema.push_str("\n\n");
            sistema.push_str(i);
        }
        if let Some(b) = self
            .config
            .comandos
            .as_ref()
            .and_then(|c| c.bloco_para_o_prompt())
        {
            sistema.push_str("\n\n");
            sistema.push_str(&b);
        }
        for c in &inicio.contexto {
            sistema.push_str("\n\nContext from the project's TaskStart hook:\n");
            sistema.push_str(c);
        }
        // Memoria e skills entram so com a capacidade de LER concedida: injetar o que a
        // politica nega a ferramenta seria a porta dos fundos da propria politica. E o
        // portao vem antes do disco -- negado, nem se abre o arquivo.
        for bloco in [
            self.config
                .memoria
                .as_ref()
                .filter(|_| caps.contains("memory.read"))
                .and_then(|m| m.bloco_para_o_prompt(&task.objective)),
            self.config
                .skills
                .as_ref()
                .filter(|_| caps.contains("skill.read"))
                .and_then(crate::skills::bloco_para_o_prompt),
        ]
        .into_iter()
        .flatten()
        {
            sistema.push_str("\n\n");
            sistema.push_str(&bloco);
        }
        // Imagem anexada que nao se le derruba a tarefa: seguir so com o texto faria o
        // modelo responder sobre uma imagem que nunca viu.
        let imagens = match crate::imagens::carregar(&ctx.workdir, &task.images) {
            Ok(v) => v,
            Err(e) => {
                return self.finish(
                    task,
                    TaskStatus::Failed,
                    Some(format!("imagem anexada: {e}")),
                    obs,
                );
            }
        };
        // `/nome args` vira o corpo do comando; o objetivo gravado continua o digitado.
        let objetivo = self
            .config
            .comandos
            .as_ref()
            .and_then(|c| c.expandir(&task.objective))
            .unwrap_or_else(|| task.objective.clone());
        let mut msgs = vec![
            Message::system(sistema),
            Message::user_com_imagens(objetivo, imagens),
        ];
        let mut chamadas: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for passo in 0..self.config.max_steps {
            if cancel.is_cancelled() {
                return self.finish(task, TaskStatus::Cancelled, None, obs);
            }
            let reply = match self.llm.chat(&msgs, &specs, &self.config.options).await {
                Ok(r) => r,
                Err(e) => {
                    return self.finish(
                        task,
                        TaskStatus::Failed,
                        Some(format!("modelo: {e}")),
                        obs,
                    );
                }
            };
            task.usage.input_tokens += reply.usage.input_tokens;
            task.usage.output_tokens += reply.usage.output_tokens;
            if let Some(fim) = reply.tool_calls.iter().find(|c| c.name == "final_answer") {
                let mut r = fim
                    .arguments
                    .get("answer")
                    .and_then(Value::as_str)
                    .unwrap_or(&reply.content)
                    .trim()
                    .to_string();
                // Conclusao se VERIFICA, nao se aceita: medido com modelo pequeno, ele chamou
                // final_answer dizendo "rust.xlsx has been created" sem ter usado ferramenta.
                // No Plan Mode nada se cria ainda: a conferencia e a do plano, nao a dos arquivos.
                let faltando = if plano {
                    vec![]
                } else {
                    arquivos_pedidos(&task.objective)
                        .into_iter()
                        .filter(|f| !ctx.workdir.join(f).exists())
                        .collect::<Vec<_>>()
                };
                let nada_rodou = !task
                    .steps
                    .iter()
                    .any(|p| p.tool.is_some() && p.outcome == "ok");
                let recusa = if !faltando.is_empty() {
                    Some(format!(
                        "final_answer REJECTED: these requested files do not exist yet in the task directory: {}. \
Create them with the appropriate tool first.",
                        faltando.join(", ")
                    ))
                } else if !plano && nada_rodou && !specs.is_empty() && lembretes < 2 {
                    Some("final_answer REJECTED: you have not used any tool yet. Use the tools to actually do the work \
(search, open pages, create files) before finishing.".to_string())
                } else {
                    None
                };
                let recusa = match recusa.filter(|_| lembretes < 3) {
                    Some(m) => Some(m),
                    None => {
                        let resposta = fim
                            .arguments
                            .get("answer")
                            .cloned()
                            .unwrap_or_else(|| Value::String(r.clone()));
                        match self.conferir_fim(&task, &ctx, &resposta, plano).await {
                            Err(f) if lembretes < 3 => Some(f.ao_modelo),
                            Err(f) => {
                                self.record(
                                    &mut task,
                                    passo,
                                    "ferramenta",
                                    Some("final_answer".into()),
                                    fim.arguments.clone(),
                                    "recusado",
                                    &f.ao_modelo,
                                );
                                return self.finish(task, TaskStatus::Failed, Some(f.erro), obs);
                            }
                            Ok(texto) => {
                                r = texto;
                                self.hook_antes_de_responder(&task, &ctx, &r, &mut bloqueios_stop)
                                    .await
                                    .map(|m| {
                                        format!(
                                            "final_answer REJECTED by the project's Stop hook: {m}"
                                        )
                                    })
                            }
                        }
                    }
                };
                if let Some(motivo) = recusa {
                    lembretes += 1;
                    self.record(
                        &mut task,
                        passo,
                        "ferramenta",
                        Some("final_answer".into()),
                        fim.arguments.clone(),
                        "recusado",
                        &motivo,
                    );
                    msgs.push(Message::assistant(reply.content.clone(), vec![fim.clone()]));
                    msgs.push(Message::tool_result(fim, motivo));
                    continue;
                }
                return self.encerrar(task, passo, r, plano, &ctx.workdir, obs);
            }
            if reply.tool_calls.is_empty() && self.config.require_final_tool && lembretes < 2 {
                lembretes += 1;
                self.record(
                    &mut task,
                    passo,
                    "pensamento",
                    None,
                    Value::Null,
                    "ok",
                    &reply.content,
                );
                msgs.push(Message::assistant(reply.content.clone(), vec![]));
                msgs.push(Message::user(
                    "Continue: call the next tool needed to complete the objective (for example create the requested file). \
If everything requested is already done, call final_answer.",
                ));
                continue;
            }
            if reply.tool_calls.is_empty() {
                let mut r = reply.content.trim().to_string();
                // O fim em texto passa pela MESMA conferencia do `final_answer`: e o caminho
                // irmao, e o fim conferido so num dos dois seria a porta do outro.
                match self
                    .conferir_fim(&task, &ctx, &Value::String(r.clone()), plano)
                    .await
                {
                    Err(f) if lembretes < 3 => {
                        lembretes += 1;
                        self.record(
                            &mut task,
                            passo,
                            "pensamento",
                            None,
                            Value::Null,
                            "recusado",
                            &r,
                        );
                        msgs.push(Message::assistant(reply.content.clone(), vec![]));
                        msgs.push(Message::user(format!(
                            "{}\nContinue the work.",
                            f.ao_modelo
                        )));
                        continue;
                    }
                    Err(f) => {
                        self.record(
                            &mut task,
                            passo,
                            "pensamento",
                            None,
                            Value::Null,
                            "recusado",
                            &r,
                        );
                        return self.finish(task, TaskStatus::Failed, Some(f.erro), obs);
                    }
                    Ok(texto) => r = texto,
                }
                if let Some(m) = self
                    .hook_antes_de_responder(&task, &ctx, &r, &mut bloqueios_stop)
                    .await
                {
                    self.record(
                        &mut task,
                        passo,
                        "pensamento",
                        None,
                        Value::Null,
                        "recusado",
                        &r,
                    );
                    msgs.push(Message::assistant(reply.content.clone(), vec![]));
                    msgs.push(Message::user(format!(
                        "Your answer was not accepted by the project's Stop hook: {m}\nContinue the work."
                    )));
                    continue;
                }
                return self.encerrar(task, passo, r, plano, &ctx.workdir, obs);
            }
            if !reply.content.trim().is_empty() {
                self.record(
                    &mut task,
                    passo,
                    "pensamento",
                    None,
                    Value::Null,
                    "ok",
                    &reply.content,
                );
            }
            msgs.push(Message::assistant(
                reply.content.clone(),
                reply.tool_calls.clone(),
            ));
            for call in &reply.tool_calls {
                if cancel.is_cancelled() {
                    return self.finish(task, TaskStatus::Cancelled, None, obs);
                }
                // Mesma ferramenta com os mesmos argumentos pela 3a vez: nao roda. Medido com
                // modelo pequeno: 8 buscas identicas seguidas, ate o buscador bloquear por robo.
                let assinatura = format!("{}{}", call.name, call.arguments);
                let repeticoes = chamadas.entry(assinatura).or_insert(0u32);
                *repeticoes += 1;
                let (texto, outcome, artefatos) = if *repeticoes >= 3 {
                    (
                        "REPEATED CALL: you already made this exact call twice and have its result above. \
Do not repeat it; use that result, try a different tool or arguments, or give the final answer."
                            .to_string(),
                        "repetida",
                        vec![],
                    )
                } else if call.name == ASK_USER && pode_perguntar {
                    let mut q = call
                        .arguments
                        .get("question")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if let Some(op) = call.arguments.get("options").and_then(Value::as_array) {
                        let op: Vec<&str> = op.iter().filter_map(Value::as_str).collect();
                        if !op.is_empty() {
                            q.push_str(&format!(" [{}]", op.join(" / ")));
                        }
                    }
                    if q.is_empty() {
                        ("ERRO: falta 'question'".to_string(), "erro", vec![])
                    } else {
                        match self.perguntar(&mut task, &q, cancel, obs).await {
                            Resposta::Cancelada => {
                                return self.finish(task, TaskStatus::Cancelled, None, obs);
                            }
                            Resposta::Texto(t) => (format!("The user answered: {t}"), "ok", vec![]),
                            Resposta::PrazoEsgotado => (
                                "No answer from the user in time. Proceed with your best judgement and \
state the assumption you made in the final answer."
                                    .to_string(),
                                "sem_resposta",
                                vec![],
                            ),
                        }
                    }
                } else {
                    // Regra `perguntar`: a pergunta vem ANTES do portao, que so deixa passar
                    // com o sim na mao; sem quem responda, o proprio portao nega.
                    // A pergunta da regra e da POLITICA, nao do modelo: basta haver quem
                    // responda (prazo), sem a capacidade `user.ask` do `ask_user`.
                    let mut aprovado = false;
                    let mut recusado_pelo_usuario = None;
                    if self.config.prazo_de_resposta.is_some()
                        && let Some(v) = self.veredito_de_comando(call, &caps)
                        && v.decisao == crate::regras::Decisao::Perguntar
                    {
                        let cmd = self
                            .ferramenta(&call.name)
                            .and_then(|t| linha_para_as_regras(t.as_ref(), &call.arguments))
                            .unwrap_or_default();
                        let q = format!(
                            "Permitir o comando `{cmd}`? ({}) Responda sim ou nao.",
                            v.explicacao
                        );
                        match self.perguntar(&mut task, &q, cancel, obs).await {
                            Resposta::Cancelada => {
                                return self.finish(task, TaskStatus::Cancelled, None, obs);
                            }
                            Resposta::Texto(t) if crate::perguntas::afirmativa(&t) => {
                                aprovado = true
                            }
                            Resposta::Texto(t) => recusado_pelo_usuario = Some(t),
                            Resposta::PrazoEsgotado => {
                                recusado_pelo_usuario = Some("(sem resposta no prazo)".into())
                            }
                        }
                    }
                    match recusado_pelo_usuario {
                        Some(t) => (
                            format!(
                                "NEGADO pelo usuario: o comando exigia aprovacao e a resposta foi: {t}"
                            ),
                            "negado",
                            vec![],
                        ),
                        None => {
                            self.call_tool_com(call, &ctx, &ledger, &task.id, &caps, aprovado)
                                .await
                        }
                    }
                };
                // Orcamento de argumento: conta as invalidas SEGUIDAS desta ferramenta,
                // contando esta. Passou do teto, a tarefa termina FALHA dizendo qual
                // ferramenta e o ultimo erro -- repetir o mesmo erro ate o teto de passos
                // so gastaria o modelo.
                let mut texto = texto;
                if outcome == DESFECHO_INVALIDO {
                    let seguidas = invalidas_seguidas(&task, &call.name) + 1;
                    let teto = self.config.tentativas_de_argumento;
                    if seguidas > teto {
                        self.record(
                            &mut task,
                            passo,
                            "ferramenta",
                            Some(call.name.clone()),
                            call.arguments.clone(),
                            outcome,
                            &texto,
                        );
                        let e = format!(
                            "argumentos invalidos em {} {seguidas} vez(es) seguida(s); orcamento de {teto} \
nova(s) tentativa(s) esgotado. Ultimo erro: {}",
                            call.name,
                            truncate_for_model(texto.trim(), 600)
                        );
                        return self.finish(task, TaskStatus::Failed, Some(e), obs);
                    }
                    texto.push_str(&format!(
                        "\nRetries left for {}: {}.",
                        call.name,
                        teto + 1 - seguidas
                    ));
                }
                self.record(
                    &mut task,
                    passo,
                    "ferramenta",
                    Some(call.name.clone()),
                    call.arguments.clone(),
                    outcome,
                    &texto,
                );
                for a in artefatos {
                    if !task.artifacts.iter().any(|x| x.path == a.path) {
                        task.artifacts.push(a);
                    } else if let Some(x) = task.artifacts.iter_mut().find(|x| x.path == a.path) {
                        *x = a;
                    }
                }
                msgs.push(Message::tool_result(
                    call,
                    truncate_for_model(&texto, self.config.max_tool_output_chars),
                ));
            }
            self.persist(&task, obs);
        }
        self.finish(
            task,
            TaskStatus::Failed,
            Some(format!(
                "limite de {} passos sem resposta final",
                self.config.max_steps
            )),
            obs,
        )
    }

    /// A resposta aceita: na execucao, conclui (com a conferencia dos arquivos pedidos); no
    /// Plan Mode, vira o plano e a tarefa espera aprovacao. O plano nao vira passo: ele
    /// esta em `task.plan`, e «nenhum passo» e o que prova que nada executou antes do sim.
    fn encerrar(
        &self,
        mut task: Task,
        passo: u32,
        r: String,
        plano: bool,
        workdir: &std::path::Path,
        obs: &dyn Observer,
    ) -> Task {
        if plano {
            task.plan = plano_da_resposta(&r).unwrap_or_else(|| vec![task.objective.clone()]);
            return self.finish(task, TaskStatus::AwaitingApproval, None, obs);
        }
        self.record(&mut task, passo, "pensamento", None, Value::Null, "ok", &r);
        task.answer = Some(r);
        self.concluir(task, workdir, obs)
    }

    /// Para a tarefa em `AwaitingInput` ate a resposta, o cancelamento ou o prazo. A espera
    /// e registrada ANTES de a pergunta aparecer no disco: resposta que chegue no instante
    /// seguinte ja tem para onde ir.
    async fn perguntar(
        &self,
        task: &mut Task,
        pergunta: &str,
        cancel: &CancelFlag,
        obs: &dyn Observer,
    ) -> Resposta {
        let prazo = self
            .config
            .prazo_de_resposta
            .unwrap_or(Duration::from_secs(0));
        let mut rx = crate::perguntas::registrar(&task.id);
        task.status = TaskStatus::AwaitingInput;

        task.question = Some(pergunta.to_string());
        task.updated_at = Utc::now();
        self.persist(task, obs);
        let limite = tokio::time::Instant::now() + prazo;
        let r = loop {
            if cancel.is_cancelled() {
                break Resposta::Cancelada;
            }
            let agora = tokio::time::Instant::now();
            if agora >= limite {
                break Resposta::PrazoEsgotado;
            }
            let fatia = (limite - agora).min(Duration::from_millis(200));
            match tokio::time::timeout(fatia, &mut rx).await {
                Ok(Ok(t)) => break Resposta::Texto(t),
                Ok(Err(_)) => break Resposta::PrazoEsgotado,
                Err(_) => {}
            }
        };
        crate::perguntas::retirar(&task.id);
        task.status = TaskStatus::Running;
        task.question = None;
        task.updated_at = Utc::now();
        self.persist(task, obs);
        r
    }

    /// A ferramenta pelo nome, so se o nome for UNICO: com dois iguais, o `find` conferiria
    /// a capacidade de um e rodaria o que o modelo achou que era o outro. A montagem recusa
    /// nome repetido; este portao fecha para quem montar o `Agent` a mao.
    fn ferramenta(&self, nome: &str) -> Option<&Arc<dyn Tool>> {
        let mut achadas = self.tools.iter().filter(|t| t.spec().name == nome);
        let primeira = achadas.next()?;
        achadas.next().is_none().then_some(primeira)
    }

    /// O veredito das regras para a chamada, quando ela e de shell, concedida e ha regras.
    fn veredito_de_comando(
        &self,
        call: &ToolCall,
        caps: &BTreeSet<String>,
    ) -> Option<crate::regras::Veredito> {
        let regras = self.config.regras.as_ref()?;
        let t = self.ferramenta(&call.name)?;
        if !caps.contains(t.capability()) {
            return None;
        }
        linha_para_as_regras(t.as_ref(), &call.arguments).map(|c| regras.avaliar(&c))
    }

    async fn disparar_hook(
        &self,
        ev: crate::hooks::Evento,
        ferramenta: Option<&str>,
        workdir: &std::path::Path,
        entrada: Value,
    ) -> crate::hooks::Saida {
        let Some(h) = self.config.hooks.clone().filter(|h| h.tem(ev)) else {
            return crate::hooks::Saida::default();
        };
        let (f, w) = (ferramenta.map(str::to_string), workdir.to_path_buf());
        tokio::task::spawn_blocking(move || h.disparar(ev, f.as_deref(), &w, &entrada))
            .await
            .unwrap_or_default()
    }

    /// O `Stop` do Claude Code: sair com 2 recusa a resposta e devolve o motivo ao modelo.
    /// Ate duas vezes por tarefa -- um hook que recusa sempre nao pode prender o laco ate o
    /// teto de passos.
    async fn hook_antes_de_responder(
        &self,
        task: &Task,
        ctx: &ToolContext,
        resposta: &str,
        bloqueios: &mut u32,
    ) -> Option<String> {
        if *bloqueios >= 2 {
            return None;
        }
        let s = self
            .disparar_hook(
                crate::hooks::Evento::AntesDeResponder,
                None,
                &ctx.workdir,
                json!({"hook_event_name": "Stop", "task_id": task.id,
                       "objective": task.objective, "answer": resposta,
                       "stop_hook_active": *bloqueios > 0}),
            )
            .await;
        if s.bloqueio.is_some() {
            *bloqueios += 1;
        }
        s.bloqueio
    }

    /// O portao unico: capacidade, prazo e evidencia. Publico porque o `mcp-serve` atende
    /// por aqui -- um servidor MCP com portao proprio seria a segunda copia da politica.
    /// As regras de comando e os hooks de ferramenta tambem moram aqui, pelo mesmo motivo:
    /// quem chama pelo `mcp-serve` nao contorna o `negar` nem o `PreToolUse`.
    pub async fn call_tool(
        &self,
        call: &ToolCall,
        ctx: &ToolContext,
        ledger: &EvidenceLedger,
        task_id: &str,
    ) -> (String, &'static str, Vec<Artifact>) {
        self.call_tool_com(call, ctx, ledger, task_id, &self.config.capabilities, false)
            .await
    }

    async fn call_tool_com(
        &self,
        call: &ToolCall,
        ctx: &ToolContext,
        ledger: &EvidenceLedger,
        task_id: &str,
        caps: &BTreeSet<String>,
        aprovado: bool,
    ) -> (String, &'static str, Vec<Artifact>) {
        let tool = self.ferramenta(&call.name);
        let mut rodou = false;
        let mut pelo_esquema = false;
        let repetida = tool.is_none() && self.tools.iter().any(|t| t.spec().name == call.name);
        let (resultado, capability): (Result<ToolOutput, ToolError>, String) = match tool {
            None if repetida => (
                Err(ToolError::Denied(format!(
                    "nome de ferramenta repetido na montagem: {}",
                    call.name
                ))),
                "ambigua".into(),
            ),
            None => (
                Err(ToolError::InvalidArguments(format!(
                    "ferramenta inexistente: {}",
                    call.name
                ))),
                "desconhecida".into(),
            ),
            // Politica conferida de novo aqui, e nao so na lista: o modelo pode pedir pelo
            // nome uma ferramenta que nao lhe foi mostrada.
            Some(t) if !caps.contains(t.capability()) => (
                Err(ToolError::Denied(format!(
                    "capacidade {} nao concedida",
                    t.capability()
                ))),
                t.capability().to_string(),
            ),
            Some(t) => {
                // O esquema se confere ANTES de tudo que depende do argumento (regra, hook,
                // ponto de restauracao, a ferramenta): argumento invalido nao chega a nada
                // disso, e volta ao modelo com TODOS os erros e o caminho de cada campo.
                // Medido (qwen2.5:3b, 30/09): o erro cru do serde, sem caminho, fez o modelo
                // repetir o mesmo erro duas vezes.
                let validacao = crate::esquema::validar(&t.spec().parameters, &call.arguments);
                if !validacao.ok() {
                    pelo_esquema = true;
                }
                let regra = self.veredito_de_comando(call, caps).and_then(|v| {
                    use crate::regras::Decisao;
                    match v.decisao {
                        Decisao::Negar => Some(format!("regra de comando: {}", v.explicacao)),
                        Decisao::Perguntar if !aprovado => Some(format!(
                            "regra de comando manda perguntar ({}) e nao ha quem aprove nesta execucao",
                            v.explicacao
                        )),
                        _ => None,
                    }
                });
                // Com a varredura de segredos exigida, o shell nao grava no git por fora do
                // `git_write` (segredos.rs diz o alcance e o limite).
                let regra = regra.or_else(|| {
                    crate::segredos::recusa_no_shell(t.comando_de_shell(&call.arguments).as_deref())
                });
                let pre = match regra {
                    Some(m) => Some(m),
                    None if pelo_esquema => None,
                    None => self
                        .disparar_hook(
                            crate::hooks::Evento::AntesDaFerramenta,
                            Some(&call.name),
                            &ctx.workdir,
                            json!({"hook_event_name": "PreToolUse", "task_id": task_id,
                                   "tool_name": call.name, "tool_input": validacao.valor}),
                        )
                        .await
                        .bloqueio
                        .map(|m| format!("hook PreToolUse bloqueou: {m}")),
                };
                let r = match pre {
                    Some(m) => Err(ToolError::Denied(m)),
                    None if pelo_esquema => Err(ToolError::InvalidArguments(
                        crate::esquema::mensagem_ao_modelo(&call.name, &validacao),
                    )),
                    None => {
                        rodou = true;
                        // Ponto de restauracao no portao, depois das regras e do PreToolUse
                        // (escrita negada nao precisa de volta) e antes da escrita.
                        if crate::checkpoint::escreve(t.capability()) {
                            crate::checkpoint::no_portao(&self.store, ctx, &call.name);
                        }
                        match tokio::time::timeout(
                            self.config.tool_timeout,
                            // O valor COAGIDO ("5" -> 5 onde o esquema pede inteiro): o
                            // que o validador aceitou e o que a ferramenta recebe.
                            t.run(validacao.valor.clone(), ctx),
                        )
                        .await
                        {
                            Ok(r) => r,
                            Err(_) => Err(ToolError::Timeout(
                                self.config.tool_timeout.as_millis() as u64
                            )),
                        }
                    }
                };
                (r, t.capability().to_string())
            }
        };
        let (mut texto, outcome, ev, artefatos) = match resultado {
            Ok(o) => (o.content, "ok", EvidenceOutcome::Succeeded, o.artifacts),
            Err(ToolError::Denied(m)) => (
                format!("NEGADO pela politica: {m}"),
                "negado",
                EvidenceOutcome::Denied,
                vec![],
            ),
            // O texto do validador ja e a mensagem inteira; o da ferramenta ganha o prefixo
            // de sempre. Os dois sao argumento invalido para o orcamento e para o fim.
            Err(ToolError::InvalidArguments(m)) if pelo_esquema => {
                (m, DESFECHO_INVALIDO, EvidenceOutcome::Failed, vec![])
            }
            Err(e @ ToolError::InvalidArguments(_)) if capability != "desconhecida" => (
                format!("ERRO: {e}"),
                DESFECHO_INVALIDO,
                EvidenceOutcome::Failed,
                vec![],
            ),
            Err(e) => (
                format!("ERRO: {e}"),
                "erro",
                EvidenceOutcome::Failed,
                vec![],
            ),
        };
        if rodou {
            // Depois da ferramenta: sair com 2 nao desfaz nada (ja rodou), mas o motivo
            // chega ao modelo junto do resultado, como no Claude Code.
            let pos = self
                .disparar_hook(
                    crate::hooks::Evento::DepoisDaFerramenta,
                    Some(&call.name),
                    &ctx.workdir,
                    json!({"hook_event_name": "PostToolUse", "task_id": task_id,
                           "tool_name": call.name, "tool_input": call.arguments,
                           "tool_response": texto, "outcome": outcome}),
                )
                .await;
            if let Some(m) = pos.bloqueio {
                texto.push_str(&format!("\n\n[PostToolUse hook]: {m}"));
            }
        }
        let _ = ledger.append(EvidenceDraft {
            action_uuid: phxclaw_types::new_uuid_v7(),
            correlation_uuid: task_id.parse().ok(),
            actor: "phxclaw-agent".into(),
            capability,
            action: call.name.clone(),
            outcome: ev,
            request_summary: call.arguments.clone(),
            result_summary: json!({
                "chars": texto.chars().count(),
                "sha256": sha256_hex(texto.as_bytes()),
                "artifacts": artefatos.iter().map(|a| json!({"path": a.path, "sha256": a.sha256})).collect::<Vec<_>>(),
            }),
            artifact_uris: artefatos.iter().map(|a| a.path.clone()).collect(),
        });
        (texto, outcome, artefatos)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        task: &mut Task,
        passo: u32,
        kind: &str,
        tool: Option<String>,
        arguments: Value,
        outcome: &str,
        texto: &str,
    ) {
        task.steps.push(StepRecord {
            n: passo + 1,
            at: Utc::now(),
            kind: kind.into(),
            tool,
            arguments,
            outcome: outcome.into(),
            summary: truncate_for_model(texto.trim(), 400),
        });
    }

    /// O fim conferido, antes de a resposta ser aceita (o `final_answer` e o fim em texto
    /// passam aqui): nenhum argumento invalido sem conserto, a resposta no esquema de saida
    /// quando a tarefa traz um, e o comando de verificacao saindo com 0. Devolve a resposta
    /// final (o JSON compacto, na saida tipada) ou o motivo da recusa.
    ///
    /// Decisao: RECUSAR e devolver ao modelo, como as outras conferencias do fim, ate o
    /// limite de recusas; esgotado, a tarefa termina FALHA com o motivo. So falhar direto
    /// jogaria fora a tarefa que o modelo ainda consertaria com uma volta; so recusar
    /// deixaria o fim sem garantia quando o modelo insiste.
    async fn conferir_fim(
        &self,
        task: &Task,
        ctx: &ToolContext,
        resposta: &Value,
        plano: bool,
    ) -> Result<String, FimRecusado> {
        let texto = || match resposta {
            Value::String(s) => s.trim().to_string(),
            outro => outro.to_string(),
        };
        if plano {
            return Ok(texto());
        }
        if let Some(e) = pendencias_de_argumento(task) {
            return Err(FimRecusado {
                ao_modelo: format!(
                    "final_answer REJECTED: these tool calls failed with invalid arguments and were never fixed: \
{}. Call them again with valid arguments (the errors are in their results above) before finishing.",
                    e.ao_modelo
                ),
                erro: e.erro,
            });
        }
        let mut final_ = texto();
        if let Some(esquema) = &task.saida_esquema {
            let v = crate::esquema::resposta_tipada(esquema, resposta);
            if !v.ok() {
                return Err(FimRecusado {
                    ao_modelo: format!(
                        "final_answer REJECTED: the answer does not match the output schema.\n{}",
                        crate::esquema::mensagem_ao_modelo("final_answer", &v)
                    ),
                    erro: format!(
                        "resposta fora do esquema de saida: {}",
                        serde_json::to_string(&v.erros).unwrap_or_default()
                    ),
                });
            }
            final_ = v.valor.to_string();
        }
        if let Some(cmd) = &task.verificar
            && let Err((codigo, saida)) =
                rodar_verificacao(cmd, &ctx.workdir, self.config.tool_timeout).await
        {
            return Err(FimRecusado {
                ao_modelo: format!(
                    "final_answer REJECTED: the verification command `{cmd}` exited with {codigo}. Fix the \
work so that it passes, then finish.\n{saida}"
                ),
                erro: format!("verificacao `{cmd}` saiu com {codigo}: {saida}"),
            });
        }
        Ok(final_)
    }

    /// Toda conclusao passa por aqui: se o objetivo pediu arquivos que nao existem, a tarefa
    /// termina FALHA, dizendo quais. Medido: depois de 3 recusas, uma resposta em texto
    /// ("Your website is live at [insert URL here]") saia como completed sem o arquivo.
    /// O argumento invalido sem conserto tambem se confere AQUI, alem do `conferir_fim`:
    /// e a ultima porta antes do `Completed`, e caminho novo que esquecer a primeira cai
    /// nesta.
    fn concluir(&self, task: Task, workdir: &std::path::Path, obs: &dyn Observer) -> Task {
        let faltando: Vec<String> = arquivos_pedidos(&task.objective)
            .into_iter()
            .filter(|f| !workdir.join(f).exists())
            .collect();
        if let Some(e) = pendencias_de_argumento(&task) {
            return self.finish(task, TaskStatus::Failed, Some(e.erro), obs);
        }
        if faltando.is_empty() {
            self.finish(task, TaskStatus::Completed, None, obs)
        } else {
            let e = format!(
                "conclusao nao verificada: arquivos pedidos ausentes: {}",
                faltando.join(", ")
            );
            self.finish(task, TaskStatus::Failed, Some(e), obs)
        }
    }

    fn persist(&self, task: &Task, obs: &dyn Observer) {
        let _ = self.store.save(task);
        obs.on_update(task);
    }

    fn finish(
        &self,
        mut task: Task,
        status: TaskStatus,
        error: Option<String>,
        obs: &dyn Observer,
    ) -> Task {
        task.status = status;
        task.error = error;
        task.updated_at = Utc::now();
        self.persist(&task, obs);
        task
    }
}

/// Quantas chamadas SEGUIDAS de `ferramenta` terminaram em argumento invalido, contadas
/// do fim para o comeco ate a ultima que passou. Sai dos passos gravados, e nao de um
/// contador ao lado: o `task.json` e a unica fonte, e a tarefa retomada conta igual.
fn invalidas_seguidas(task: &Task, ferramenta: &str) -> u32 {
    let mut n = 0;
    for p in task.steps.iter().rev() {
        if p.kind != "ferramenta" || p.tool.as_deref() != Some(ferramenta) {
            continue;
        }
        match p.outcome.as_str() {
            "ok" => break,
            DESFECHO_INVALIDO => n += 1,
            _ => {}
        }
    }
    n
}

/// As ferramentas cuja ULTIMA chamada (entre as que passaram e as invalidas) foi argumento
/// invalido: o erro que o modelo deixou para tras. `None` se nao ha nenhuma.
fn pendencias_de_argumento(task: &Task) -> Option<FimRecusado> {
    let mut ultima: std::collections::BTreeMap<&str, bool> = std::collections::BTreeMap::new();
    for p in &task.steps {
        if let (Some(t), "ferramenta") = (p.tool.as_deref(), p.kind.as_str()) {
            match p.outcome.as_str() {
                "ok" => {
                    ultima.insert(t, false);
                }
                DESFECHO_INVALIDO => {
                    ultima.insert(t, true);
                }
                _ => {}
            }
        }
    }
    let nomes: Vec<String> = ultima
        .into_iter()
        .filter(|(_, invalida)| *invalida)
        .map(|(t, _)| format!("{t} ({}x)", invalidas_seguidas(task, t)))
        .collect();
    (!nomes.is_empty()).then(|| FimRecusado {
        ao_modelo: nomes.join(", "),
        erro: format!(
            "conclusao nao verificada: chamada com argumento invalido sem conserto: {}",
            nomes.join(", ")
        ),
    })
}

/// O comando de verificacao no MESMO executor do hook `Stop` (`run_in_workdir_com`, o
/// bwrap do `shell`, sem rede, a pasta da tarefa em `/work`). `Err((codigo, saida))` quando
/// nao sai com 0; sem bwrap e recusa, nunca execucao fora do sandbox.
async fn rodar_verificacao(
    cmd: &str,
    workdir: &std::path::Path,
    prazo: Duration,
) -> Result<(), (String, String)> {
    let Some(bwrap) = crate::arquivos::achar_bwrap() else {
        return Err(("-".into(), "no bwrap to run it in the sandbox".into()));
    };
    let c = phxclaw_sandbox::WorkdirCommand {
        workdir: workdir.to_path_buf(),
        script: cmd.to_string(),
        timeout: prazo,
        network: false,
        max_output_bytes: 16 * 1024,
    };
    let r = tokio::task::spawn_blocking(move || {
        phxclaw_sandbox::run_in_workdir_com(&bwrap, &c, &phxclaw_sandbox::SandboxExtras::default())
    })
    .await;
    match r {
        Ok(Ok(o)) if o.exit_code == Some(0) => Ok(()),
        Ok(Ok(o)) => {
            let saida = format!("{}\n{}", o.stdout.trim(), o.stderr.trim());
            Err((
                o.exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "sinal/prazo".into()),
                truncate_for_model(saida.trim(), 2_000),
            ))
        }
        Ok(Err(e)) => Err(("-".into(), e.to_string())),
        Err(e) => Err(("-".into(), e.to_string())),
    }
}

/// A linha que as regras de comando conferem: a declarada pela ferramenta, ou, para
/// ferramenta que cria processo e nao declara, `<nome> <action>`. As demais (ler arquivo,
/// buscar na web) nao passam pelas regras de COMANDO; a politica delas e a capacidade.
pub fn linha_para_as_regras(t: &dyn Tool, args: &Value) -> Option<String> {
    if let Some(c) = t.comando_de_shell(args) {
        return Some(c);
    }
    if !CAPACIDADES_QUE_EXECUTAM.contains(&t.capability()) {
        return None;
    }
    Some(linha_sintetica(&t.spec().name, args, &["action"]))
}

/// `<nome> <valor>`: a linha sintetica que as regras de comando conferem para ferramenta
/// que cria processo sem ter uma linha de shell de verdade. O valor e o primeiro dos
/// `campos` presente nos argumentos, entre aspas de shell. Um lugar so: a ferramenta que
/// declara `comando_de_shell` e o portao montam a linha do mesmo jeito.
pub fn linha_sintetica(nome: &str, args: &Value, campos: &[&str]) -> String {
    let mut l = nome.to_string();
    if let Some(v) = campos
        .iter()
        .find_map(|c| args.get(c).and_then(Value::as_str))
    {
        l.push(' ');
        l.push_str(&crate::python::aspas(v));
    }
    l
}

/// Nomes de arquivo que o objetivo pede explicitamente ("crie rust.xlsx"): a conclusao
/// so e aceita quando eles existem na pasta da tarefa.
pub fn arquivos_pedidos(objetivo: &str) -> Vec<String> {
    const EXT: &[&str] = &[
        "xlsx", "docx", "pptx", "md", "csv", "png", "html", "txt", "json", "pdf",
    ];
    let mut v: Vec<String> = objetivo
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '"' | '\'' | '`'))
        .map(|p| p.trim_end_matches(['.', ':', '!', '?']))
        .filter(|p| {
            p.rsplit_once('.').is_some_and(|(nome, ext)| {
                !nome.is_empty()
                    && !nome.contains("//")
                    && !p.contains("://")
                    && EXT.contains(&ext.to_ascii_lowercase().as_str())
                    && nome
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/'))
            })
        })
        .map(str::to_string)
        .collect();
    v.dedup();
    v
}

/// Aceita o JSON cercado de texto ou de ``` que modelos pequenos costumam devolver.
pub fn parse_plan(texto: &str) -> Option<Vec<String>> {
    let ini = texto.find('{')?;
    let fim = texto.rfind('}')?;
    let v: Value = serde_json::from_str(&texto[ini..=fim]).ok()?;
    let passos: Vec<String> = v
        .get("steps")
        .or_else(|| v.get("passos"))?
        .as_array()?
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .take(12)
        .collect();
    (!passos.is_empty()).then_some(passos)
}

/// O plano de uma resposta do Plan Mode: o JSON pedido, ou, se o modelo respondeu em
/// texto, as linhas de lista (`1.`, `-`, `*`). Plano que nao se acha e `None`.
pub fn plano_da_resposta(texto: &str) -> Option<Vec<String>> {
    if let Some(p) = parse_plan(texto) {
        return Some(p);
    }
    let itens: Vec<String> = texto
        .lines()
        .map(str::trim)
        .filter_map(|l| {
            let sem_num = l.trim_start_matches(|c: char| c.is_ascii_digit());
            if sem_num.len() < l.len() {
                sem_num
                    .strip_prefix('.')
                    .or_else(|| sem_num.strip_prefix(')'))
            } else {
                l.strip_prefix("- ").or_else(|| l.strip_prefix("* "))
            }
        })
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .take(12)
        .collect();
    (!itens.is_empty()).then_some(itens)
}

pub fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// Artefato a partir de um arquivo ja gravado na pasta da tarefa.
pub fn artifact_for(workdir: &std::path::Path, relative: &str) -> std::io::Result<Artifact> {
    let bytes = std::fs::read(workdir.join(relative))?;
    Ok(Artifact {
        path: relative.to_string(),
        media_type: media_type(relative).into(),
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    })
}

pub fn media_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" => "text/markdown",
        "txt" | "log" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arquivos_pedidos_saem_do_objetivo() {
        assert_eq!(
            arquivos_pedidos(
                "create a spreadsheet rust.xlsx, then write relatorio.md. See https://x.com/a.html"
            ),
            vec!["rust.xlsx", "relatorio.md"]
        );
        assert!(arquivos_pedidos("versao 1.98.1 do Rust").is_empty());
    }

    #[test]
    fn plano_aceita_json_cercado() {
        let p = parse_plan("Claro!\n```json\n{\"steps\": [\"buscar\", \" \", \"escrever\"]}\n```")
            .unwrap();
        assert_eq!(p, vec!["buscar", "escrever"]);
        assert_eq!(parse_plan("sem json"), None);
        assert_eq!(parse_plan("{\"passos\": [\"a\"]}").unwrap(), vec!["a"]);
    }
}

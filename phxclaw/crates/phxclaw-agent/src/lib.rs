#![forbid(unsafe_code)]
//! Motor do agente autonomo do PhxClaw: planeja, executa com ferramentas sob politica,
//! registra cada passo em evidencia com hash encadeado, roda subagentes em paralelo e
//! dispara tarefas agendadas. Os provedores de modelo e as ferramentas de navegador,
//! busca e documentos entram pelos traits de `phxclaw-agent-core`.

pub mod acp;
pub mod adaptadores;
pub mod agenda;
pub mod api;
pub mod arquivos;
pub mod busca;
pub mod calculadora;
pub mod canais;
pub mod canal;
pub mod canvas;
pub mod checkpoint;
pub mod dispositivos;
pub mod email;
pub mod equipe;
pub mod estilos;
pub mod ferramentas;
pub mod fluxos;
pub mod forja;
pub mod gatilhos;
pub mod git;
pub mod hooks;
pub mod lsp;
pub mod mcp;
pub mod memoria;
pub mod midia;
pub mod montagem;
pub mod motor;
pub mod notebook;
pub mod nuvem;
pub mod pacotes;
pub mod perguntas;
pub mod plugins;
pub mod pwa;
pub mod python;
pub mod regras;
pub mod remoto;
pub mod repl;
pub mod revisao;
pub mod sessoes;
pub mod sistema;
pub mod site;
pub mod skills;
pub mod tarefa;
pub mod ui;
pub mod visao;
pub mod voz;
pub mod xai;

pub use agenda::{Agenda, Schedule};
pub use ferramentas::{
    BackgroundShellTool, EditFileTool, ListFilesTool, ParallelAgentsTool, ReadFileTool,
    ScriptedLlm, ShellTool, WriteFileTool,
};
pub use motor::{Agent, AgentConfig, CancelFlag, NoObserver, Observer};
pub use tarefa::{StepRecord, Task, TaskStatus, TaskStore};

#![forbid(unsafe_code)]
//! Motor do agente autonomo do PhxClaw: planeja, executa com ferramentas sob politica,
//! registra cada passo em evidencia com hash encadeado, roda subagentes em paralelo e
//! dispara tarefas agendadas. Os provedores de modelo e as ferramentas de navegador,
//! busca e documentos entram pelos traits de `phxclaw-agent-core`.

pub mod adaptadores;
pub mod agenda;
pub mod api;
pub mod email;
pub mod ferramentas;
pub mod montagem;
pub mod motor;
pub mod site;
pub mod tarefa;

pub use agenda::{Agenda, Schedule};
pub use ferramentas::{
    ListFilesTool, ParallelAgentsTool, ReadFileTool, ScriptedLlm, ShellTool, WriteFileTool,
};
pub use motor::{Agent, AgentConfig, CancelFlag, NoObserver, Observer};
pub use tarefa::{StepRecord, Task, TaskStatus, TaskStore};

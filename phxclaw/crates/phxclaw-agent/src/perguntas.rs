//! Perguntas ao usuario no meio da tarefa: o `ask_user` do modelo e o `perguntar` das
//! regras de comando passam pela MESMA espera.
//!
//! A tarefa vai para `AwaitingInput` com a pergunta gravada no `task.json` (e e assim que a
//! API e a tela a mostram), e a execucao fica parada aqui ate a resposta chegar por
//! `responder` -- chamada pela rota `POST /v1/tasks/{id}/answer` e pela CLI, as duas pela
//! mesma funcao. A conversa com o modelo fica viva na memoria do processo enquanto
//! espera; reconstrui-la do disco perderia o que as ferramentas devolveram por inteiro,
//! porque o `task.json` guarda so o resumo de cada passo.
//!
//! O preco, declarado: processo que cai com a tarefa esperando deixa a tarefa em
//! `AwaitingInput` sem ninguem do outro lado, e `responder` diz isso em vez de fingir que
//! entregou.

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Uma espera por tarefa. Global ao processo porque quem responde (a rota HTTP, a CLI) nao
/// tem o agente na mao -- so o id da tarefa.
static ESPERAS: Mutex<Option<HashMap<String, oneshot::Sender<String>>>> = Mutex::new(None);

fn com<R>(f: impl FnOnce(&mut HashMap<String, oneshot::Sender<String>>) -> R) -> R {
    let mut g = ESPERAS.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

/// Abre a espera ANTES de a tarefa aparecer como `AwaitingInput`: uma resposta que chegue
/// logo depois de a pergunta ser gravada ja tem para onde ir.
pub fn registrar(task_id: &str) -> oneshot::Receiver<String> {
    let (tx, rx) = oneshot::channel();
    com(|m| m.insert(task_id.to_string(), tx));
    rx
}

pub fn retirar(task_id: &str) {
    com(|m| m.remove(task_id));
}

pub fn esperando(task_id: &str) -> bool {
    com(|m| m.contains_key(task_id))
}

/// Entrega a resposta a execucao parada. Erro quando ninguem espera: a tarefa nao
/// perguntou, ja recebeu, ou o processo que perguntou nao existe mais.
pub fn responder(task_id: &str, texto: &str) -> Result<(), String> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Err("resposta vazia".into());
    }
    let tx = com(|m| m.remove(task_id))
        .ok_or("nenhuma execucao esperando resposta desta tarefa neste processo")?;
    tx.send(texto.to_string())
        .map_err(|_| "a execucao que perguntou ja terminou".to_string())
}

/// Resposta que aprova: `sim`, `s`, `yes`, `y`, `ok`, `aprovo`, `permitir`... O que nao
/// for claramente sim e nao -- comando que a regra manda perguntar so roda com um sim.
pub fn afirmativa(texto: &str) -> bool {
    let t = texto.trim().to_lowercase();
    let primeira = t
        .split(|c: char| !c.is_alphanumeric())
        .find(|p| !p.is_empty())
        .unwrap_or("");
    matches!(
        primeira,
        "sim"
            | "s"
            | "yes"
            | "y"
            | "ok"
            | "aprovo"
            | "aprovado"
            | "aprovar"
            | "permitir"
            | "permito"
            | "pode"
            | "approve"
            | "approved"
            | "allow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_sim_claro_aprova() {
        for s in ["sim", "Sim, pode", "y", "OK.", "aprovo"] {
            assert!(afirmativa(s), "{s}");
        }
        for n in ["nao", "não", "talvez", "", "no", "simplesmente nao"] {
            assert!(!afirmativa(n), "{n}");
        }
    }

    #[test]
    fn resposta_sem_espera_e_erro_e_com_espera_chega() {
        assert!(responder("id-sem-espera", "x").is_err());
        let mut rx = registrar("id-com-espera");
        assert!(esperando("id-com-espera"));
        responder("id-com-espera", " 42 ").unwrap();
        assert_eq!(rx.try_recv().unwrap(), "42");
        assert!(!esperando("id-com-espera"));
        assert!(responder("id-com-espera", "de novo").is_err());
    }
}

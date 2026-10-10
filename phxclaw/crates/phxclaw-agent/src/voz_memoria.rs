//! Captura de cada turno da conversa de voz na memoria de cognicao local, para o assistente
//! melhorar o contexto com o tempo (Ollama raciocinando local). CAPTURA-SO: nada aqui se
//! aplica nem volta ao prompt. O consumo -- injetar o historico, aprender estrategia --
//! vem noutra rodada, so com o Go do integrador e do dono.
//!
//! Assinatura que a frente da voz (`voz.rs`, ao fim de cada turno de `conversar`) chama:
//!
//! ```ignore
//! pub fn registrar_turno(raiz_do_agente: &Path, turno: &crate::voz::TurnoDeVoz)
//!     -> Result<(), String>
//! ```
//!
//! - `raiz_do_agente`: a raiz do `TaskStore` (a mesma que `Memoria::do_ambiente` recebe).
//! - `turno`: o `crate::voz::TurnoDeVoz` que `conversar` ja monta -- este modulo NAO o edita.
//!
//! Grao: UM turno (o que se ouviu + o que o agente respondeu) por `MemoryRecord`, no
//! namespace `voz.turnos`, com a data (`created_at`) e a proveniencia (um `EvidenceRef` que
//! aponta a tarefa do turno).
//!
//! PII e segredo saem ANTES do disco pelo MESMO motor da tarja da fila
//! (`fluxo_politica::achar_pii` + `phxclaw_secret_broker::scrub_secret_like`), sem segundo
//! detector: dado pessoal/cliente vira a classe (`[pii: email]`), nunca o texto cru. A
//! redacao vem antes do teto de tamanho -- o que se mede e o que se grava.
//!
//! O arquivo e proprio (`<raiz>/_memoria/voz_turnos.json`), separado da memoria entre
//! tarefas (`memoria.rs`): captura-so nao pode vazar para a injecao de contexto de hoje, que
//! le o arquivo do escopo. E a classe e `Confidential` -- acima do teto padrao do compilador
//! de contexto (`Internal`) -- porque fala livre carrega contexto pessoal mesmo depois da
//! redacao, e capturar nao e liberar.

use chrono::Utc;
use phxclaw_memory_context::{
    DataClassification, FileMemoryStore, MemoryLimits, MemoryRecord, MemoryScope,
};
use phxclaw_types::EvidenceRef;
use serde_json::json;
use std::path::Path;
use std::sync::Mutex;

/// Namespace do grao: um turno por registro.
const NAMESPACE: &str = "voz.turnos";

/// Teto por campo de texto, em caracteres. Uma resposta falada e curta; sem o teto um turno
/// vira audiolivro na memoria e come o contexto que os outros turnos deviam dividir. Corta
/// por caractere (nunca no meio de um byte multibyte).
const TETO_CHARS: usize = 600;

/// Tetos do arquivo: folga para o turno redigido (dois campos de ate `TETO_CHARS` mais o
/// envelope JSON) e muitas entradas, com a mais antiga saindo pelo teto do `FileMemoryStore`.
const LIMITES: MemoryLimits = MemoryLimits {
    max_entry_bytes: 4_000,
    max_entries: 1_000,
};

/// Uma trava por processo: a conversa por voz e sequencial, mas a API e os subagentes podem
/// gravar o mesmo arquivo; ler-mudar-gravar sem trava perderia um turno.
static TRAVA: Mutex<()> = Mutex::new(());

/// Redige um campo pelo motor unico da casa: se ha PII, o campo inteiro vira a classe
/// (`[pii: email, cpf]`), como na tarja da fila -- `achar_pii` da a classe, nao o trecho,
/// entao nao da para recortar so o pedaco sem reimplementar o detector. Sem PII, ainda passa
/// pela tarja de segredo (`scrub_secret_like`). Depois corta no teto de caracteres.
fn redigir(texto: &str) -> String {
    let pii = crate::fluxo_politica::achar_pii(texto);
    let limpo = if pii.is_empty() {
        phxclaw_secret_broker::scrub_secret_like(texto)
    } else {
        format!("[pii: {}]", pii.join(", "))
    };
    limpo.chars().take(TETO_CHARS).collect()
}

/// Grava um turno da conversa de voz na memoria de cognicao local. Idempotente por turno: a
/// `key` e o id da tarefa do turno, entao regravar o mesmo turno o substitui em vez de
/// duplicar.
pub fn registrar_turno(
    raiz_do_agente: &Path,
    turno: &crate::voz::TurnoDeVoz,
) -> Result<(), String> {
    let arquivo = raiz_do_agente.join("_memoria").join("voz_turnos.json");
    let agora = Utc::now();

    let ouvido = redigir(&turno.ouvido);
    let resposta = redigir(&turno.resposta);
    // O estado vem do desfecho do turno (teste verde/vermelho, recusa, aprovacao): e o rotulo
    // que um aprendizado futuro le, nunca uma escolha do modelo que portao nenhum conferiu.
    let estado = serde_json::to_value(turno.estado).map_err(|e| e.to_string())?;
    let erro = turno.erro.as_deref().map(redigir);

    let valor = json!({
        "ouvido": ouvido,
        "resposta": resposta,
        "estado": estado,
        "erro": erro,
    });

    // Proveniencia: de onde veio este turno. A `uri` aponta a tarefa do turno; a nota diz se
    // houve WAV (so o nome do arquivo, nunca o conteudo).
    let proveniencia = EvidenceRef {
        uuid: phxclaw_types::new_uuid_v7(),
        uri: format!("phxclaw-voz-turno:{}", turno.tarefa),
        source_type: "voice_turn".into(),
        retrieved_at: agora,
        sha256: crate::motor::sha256_hex(valor.to_string().as_bytes()),
        notes: turno
            .wav
            .as_ref()
            .and_then(|w| w.file_name())
            .map(|n| format!("wav: {}", n.to_string_lossy())),
    };

    let chave = if turno.tarefa.trim().is_empty() {
        phxclaw_types::new_uuid_v7().simple().to_string()
    } else {
        turno.tarefa.clone()
    };

    let registro = MemoryRecord::new(
        NAMESPACE,
        chave,
        valor,
        MemoryScope::Project(escopo(raiz_do_agente)),
        DataClassification::Confidential,
        vec![proveniencia],
    )
    .map_err(|e| e.to_string())?;

    let _g = TRAVA.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = FileMemoryStore::open(&arquivo, LIMITES).map_err(|e| e.to_string())?;
    store.append(registro).map_err(|e| e.to_string())?;
    Ok(())
}

/// O escopo da memoria de voz segue o mesmo da memoria entre tarefas (projeto/usuario), para
/// um dia o consumo casar os dois acervos; na ausencia, "padrao".
fn escopo(_raiz: &Path) -> String {
    let bruto = crate::config::texto_de("agente.memoria_escopo").unwrap_or_default();
    let s: String = bruto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect();
    if s.is_empty() { "padrao".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskStatus;
    use crate::voz::TurnoDeVoz;
    use phxclaw_memory_context::MemoryStore;
    use std::path::PathBuf;

    fn raiz() -> PathBuf {
        std::env::temp_dir().join(format!("phx-voz-mem-{}", phxclaw_types::new_uuid_v7()))
    }

    fn turno(tarefa: &str, ouvido: &str, resposta: &str) -> TurnoDeVoz {
        TurnoDeVoz {
            tarefa: tarefa.into(),
            ouvido: ouvido.into(),
            resposta: resposta.into(),
            estado: TaskStatus::Completed,
            wav: Some(PathBuf::from("/x/pergunta.wav")),
            erro: None,
        }
    }

    /// PII redigida antes do disco: e-mail e CPF na transcricao viram a classe, nunca o dado
    /// cru. RED medido: gravar `turno.ouvido` cru (sem `redigir`) deixa o e-mail e o CPF no
    /// arquivo e o teste cai.
    #[test]
    fn registrar_turno_redige_pii() {
        let raiz = raiz();
        let t = turno(
            "t-pii",
            "meu e-mail e ana.souza@empresa.com.br e o cpf 529.982.247-25, quando chega?",
            "ok, vou verificar",
        );
        registrar_turno(&raiz, &t).unwrap();
        let arquivo = raiz.join("_memoria").join("voz_turnos.json");
        let disco = std::fs::read_to_string(&arquivo).unwrap();
        assert!(
            !disco.contains("ana.souza@empresa.com.br"),
            "e-mail cru no disco: {disco}"
        );
        assert!(
            !disco.contains("529.982.247-25"),
            "cpf cru no disco: {disco}"
        );
        assert!(disco.contains("[pii:"), "faltou a classe no disco: {disco}");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// O que se grava tem data e proveniencia. RED medido: `MemoryRecord::new` com `vec![]`
    /// no lugar da proveniencia deixa o registro sem origem e o teste cai.
    #[test]
    fn turno_vira_registro_com_data_e_proveniencia() {
        let raiz = raiz();
        let antes = Utc::now();
        let t = turno("t-prov", "qual a capital da franca?", "paris");
        registrar_turno(&raiz, &t).unwrap();
        let arquivo = raiz.join("_memoria").join("voz_turnos.json");
        let store = FileMemoryStore::open(&arquivo, LIMITES).unwrap();
        let registros = store.all();
        assert_eq!(registros.len(), 1);
        let r = registros[0];
        assert_eq!(r.namespace, NAMESPACE);
        // Data: dentro da janela da gravacao.
        assert!(r.created_at >= antes && r.created_at <= Utc::now());
        // Proveniencia: um EvidenceRef que aponta a tarefa do turno.
        assert_eq!(r.evidence.len(), 1, "faltou a proveniencia");
        assert_eq!(r.evidence[0].uri, "phxclaw-voz-turno:t-prov");
        assert_eq!(r.evidence[0].source_type, "voice_turn");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

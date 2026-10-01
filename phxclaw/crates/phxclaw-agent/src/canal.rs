//! Telegram como canal do agente: a mensagem de um chat permitido vira tarefa, e a resposta
//! final volta ao mesmo chat.
//!
//! O laco, a lista de permitidos, o cursor depois da tarefa e a saida partida moram em
//! `canais` e servem todos os provedores; aqui fica so o que e do Telegram: o long polling
//! do `getUpdates` (o cursor e o proximo `update_id`), o teto de 4096 unidades UTF-16 e a
//! conferencia do token com `getMe` antes de ouvir.

pub use crate::canais::{
    Canal, ChannelSendTool, Registro, broker_em, guardar_segredo, partir, resposta_da_tarefa,
};
use crate::canais::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::{TELEGRAM_MAX_POLL_SECS, TELEGRAM_MAX_TEXT, TelegramProvider};
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use std::collections::BTreeSet;
use std::path::Path;
use uuid::Uuid;

/// O Telegram e um `Canal` como os outros; o nome fica porque e o que o resto do agente
/// e os testes do canal chamam.
pub type CanalTelegram = Canal;

const CANAL: &str = "telegram";
const NOME_SEGREDO: &str = "telegram-bot";
const ESCOPOS: &[&str] = &[
    "channel:telegram:send",
    "channel:telegram:probe",
    "channel:telegram:receive",
];

/// Guarda o token do bot no broker e devolve o id do segredo. Reiniciar com o mesmo token
/// reaproveita o envelope; token novo rotaciona o mesmo segredo em vez de empilhar outro.
pub fn guardar_token(broker: &SecretBroker, token: SecretValue) -> Result<Uuid, String> {
    guardar_segredo(broker, NOME_SEGREDO, "canais", ESCOPOS, token)
}

/// Liga o canal de producao: guarda o token no broker da pasta, monta o provedor da origem
/// oficial e confere o token com `getMe` antes de ouvir -- token errado para aqui, dizendo
/// o HTTP, e nao vira um laco de erro a cada 5 s.
pub async fn ligar_telegram(
    pasta: &Path,
    token: SecretValue,
    permitidos: BTreeSet<i64>,
    log: Registro,
) -> Result<CanalTelegram, String> {
    use phxclaw_channel_gateway::ChannelProviderV2;
    if permitidos.is_empty() {
        return Err("lista de chats vazia: ninguem poderia falar com o agente".into());
    }
    let broker = broker_em(pasta)?;
    let id = guardar_token(&broker, token)?;
    let (provider, sonda) = tokio::task::spawn_blocking(move || {
        let p = TelegramProvider::new(broker, id, CANAL).map_err(|e| e.to_string())?;
        let sonda = p.probe()?;
        Ok::<_, String>((p, sonda))
    })
    .await
    .map_err(|e| e.to_string())??;
    if !sonda.connected {
        return Err(format!(
            "telegram: getMe recusou o token ({})",
            sonda.error.unwrap_or_else(|| "sem detalhe".into())
        ));
    }
    let conta = sonda.account_id.unwrap_or_else(|| CANAL.into());
    (log)(&format!(
        "telegram: conectado como {} ({conta})",
        sonda.display_name.as_deref().unwrap_or("bot")
    ));
    CanalTelegram::novo(provider, conta, permitidos, pasta, log)
}

/// Lista de chats de `PHXCLAW_TELEGRAM_CHATS` ("123,-100456"). Id que nao e numero e erro,
/// e nao omissao: um chat digitado errado sumir calado da lista parece bloqueio sem motivo.
pub fn chats_da_lista(texto: &str) -> Result<BTreeSet<i64>, String> {
    texto
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>()
                .map_err(|_| format!("id de chat invalido em PHXCLAW_TELEGRAM_CHATS: {s}"))
        })
        .collect()
}

/// O `getUpdates` como lote do laco: cada atualizacao leva o cursor `update_id + 1`, e a
/// que nao e mensagem (edicao, enquete, botao) so faz o cursor andar.
impl Provedor for TelegramProvider {
    fn nome(&self) -> &str {
        CANAL
    }

    fn limite(&self) -> (usize, Unidade) {
        (TELEGRAM_MAX_TEXT, Unidade::Utf16)
    }

    fn espera_maxima(&self) -> u64 {
        TELEGRAM_MAX_POLL_SECS
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let offset = cursor.and_then(|c| c.parse::<i64>().ok());
        let updates = self
            .get_updates(offset, espera_seg)
            .map_err(|e| format!("getUpdates: {e}"))?;
        Ok(updates
            .into_iter()
            .map(|u| Entrada {
                cursor: Some((u.update_id + 1).to_string()),
                mensagem: u.message.map(|m| Mensagem {
                    conversa: m.chat.id.to_string(),
                    autor: m
                        .from
                        .map_or_else(|| m.chat.id.to_string(), |f| f.id.to_string()),
                    id: m.message_id.to_string(),
                    texto: m.text,
                }),
            })
            .collect())
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        crate::canais::enviar_pelo(self, conversa, texto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partir_respeita_o_teto_e_nao_perde_nada() {
        let t = "a".repeat(10_000);
        let p = partir(&t, 4096);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|x| x.encode_utf16().count() <= 4096));
        assert_eq!(p.concat(), t);
        assert_eq!(partir("", 4096), vec![String::new()]);
        assert_eq!(partir("curto", 4096), vec!["curto".to_string()]);
    }

    #[test]
    fn partir_conta_utf16_e_prefere_quebra_de_linha() {
        // Emoji vale 2 unidades: 3000 deles sao 6000 unidades, dois pedacos.
        let t = "\u{1F600}".repeat(3000);
        let p = partir(&t, 4096);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|x| x.encode_utf16().count() <= 4096));
        assert_eq!(p.concat(), t);
        let t = format!("{}\n{}", "x".repeat(3000), "y".repeat(3000));
        let p = partir(&t, 4096);
        assert_eq!(p[0], format!("{}\n", "x".repeat(3000)));
        assert_eq!(p.concat(), t);
    }

    #[test]
    fn lista_de_chats_recusa_id_que_nao_e_numero() {
        assert_eq!(
            chats_da_lista(" 1, -100200 ,,").unwrap(),
            [1, -100200].into_iter().collect()
        );
        assert!(chats_da_lista("1,abc").is_err());
    }
}

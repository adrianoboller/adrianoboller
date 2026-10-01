//! Chave de servico pago (ElevenLabs, Gemini): onde mora, como se guarda e como vira
//! `Credencial`. Uma regra so para todos, para nenhum servico ganhar um segundo jeito de
//! guardar chave -- o valor sai do ambiente direto para o SecretBroker da pasta do servico
//! (`canais::guardar_segredo`) e so volta por concessao curta (`Credencial::com`), que tira a
//! chave de todo erro.
//!
//! A pasta so e aberta se o broker ja existe: `canais::broker_em` CRIA a chave-mestra, e
//! perguntar «tem chave?» nao pode deixar um cofre vazio para tras a cada arranque.
//! O `speak`, o `transcribe` e o `voice_list` pedem a mesma chave, e por isso o broker sai
//! do `canais::broker_em`, que devolve o MESMO broker por pasta (ver o motivo la).

use crate::canais::broker_em as broker;
use crate::canais::http::Credencial;
use phxclaw_secret_broker::SecretValue;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Um servico com chave no broker.
pub struct Servico {
    /// Pasta (sob a raiz do agente) e espaco do segredo.
    pub espaco: &'static str,
    pub nome_do_segredo: &'static str,
    /// Variaveis lidas por `guardar_do_ambiente`, na ordem.
    pub variaveis: &'static [&'static str],
    /// Nome para a mensagem («a chave da ElevenLabs»).
    pub rotulo: &'static str,
    /// O comando que guarda a chave, citado quando ela falta.
    pub comando: &'static str,
}

impl Servico {
    pub fn pasta(&self, raiz_do_agente: &Path) -> PathBuf {
        raiz_do_agente.join(self.espaco)
    }

    /// Guarda `valor` (mesmo valor reaproveita o envelope; outro rotaciona).
    pub fn guardar(&self, raiz_do_agente: &Path, valor: &str) -> Result<Uuid, String> {
        let valor = valor.trim();
        if valor.is_empty() {
            return Err(format!("{} vazia", self.rotulo));
        }
        let broker = broker(&self.pasta(raiz_do_agente))?;
        let escopos = crate::canais::escopos(self.espaco);
        let escopos: Vec<&str> = escopos.iter().map(String::as_str).collect();
        crate::canais::guardar_segredo(
            &broker,
            self.nome_do_segredo,
            self.espaco,
            &escopos,
            SecretValue::new(valor.to_string()),
        )
    }

    /// `phxclaw <servico> chave`: a primeira variavel definida vai para o broker.
    pub fn guardar_do_ambiente(&self, raiz_do_agente: &Path) -> Result<Uuid, String> {
        let valor = self
            .variaveis
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|t| !t.trim().is_empty()))
            .ok_or_else(|| format!("defina {} com {}", self.variaveis.join(" ou "), self.rotulo))?;
        self.guardar(raiz_do_agente, &valor)
    }

    /// A credencial guardada. Sem ela, o erro diz o comando que a guarda: e isso que o
    /// operador tem de fazer, e e isso que a ferramenta repete ao modelo.
    pub fn credencial(&self, raiz_do_agente: &Path) -> Result<Credencial, String> {
        let falta = || {
            format!(
                "{} nao esta guardada: rode `phxclaw {}` (o valor vai para o SecretBroker)",
                self.rotulo, self.comando
            )
        };
        let pasta = self.pasta(raiz_do_agente);
        if !pasta.join("segredos/master.key").exists() {
            return Err(falta());
        }
        let broker = broker(&pasta)?;
        let d = crate::canais::segredo_guardado(&broker, self.nome_do_segredo, self.espaco)?
            .ok_or_else(falta)?;
        Ok(Credencial::nova(broker, d.uuid, self.espaco))
    }
}

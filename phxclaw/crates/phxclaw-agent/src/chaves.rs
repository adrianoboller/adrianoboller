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
    /// A chave do catalogo do config.json: a variavel `PHXCLAW_*` lida por
    /// `guardar_do_ambiente` sai DE LA (`catalogo::por_chave`), nao de uma segunda lista
    /// aqui -- o nome da variavel mora num lugar so.
    pub chave: &'static str,
    /// Nomes de fora do catalogo aceitos depois da variavel do catalogo (`GEMINI_API_KEY`).
    pub aliases: &'static [&'static str],
    /// Nome para a mensagem («a chave da ElevenLabs»).
    pub rotulo: &'static str,
    /// O comando que guarda a chave, citado quando ela falta.
    pub comando: &'static str,
}

impl Servico {
    /// As variaveis lidas, na ordem: a do catalogo e depois os aliases.
    pub fn variaveis(&self) -> Vec<String> {
        let do_catalogo = crate::config::catalogo_do_config::por_chave(self.chave)
            .map(|c| c.variavel.clone())
            .unwrap_or_else(|| panic!("{} nao esta no catalogo do config.json", self.chave));
        std::iter::once(do_catalogo)
            .chain(self.aliases.iter().map(|a| a.to_string()))
            .collect()
    }

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
        let variaveis = self.variaveis();
        let valor = variaveis
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|t| !t.trim().is_empty()))
            .ok_or_else(|| format!("defina {} com {}", variaveis.join(" ou "), self.rotulo))?;
        self.guardar(raiz_do_agente, &valor)
    }

    /// O valor na ordem do catalogo: o ambiente do processo primeiro, senao o broker.
    /// `None` quando nao esta em nenhum dos dois. O texto sai da concessao curta para
    /// quem precisa dele como texto (o Bearer da API se compara, nao se manda): quem puder
    /// usar a `Credencial` deve preferir `credencial`, que nunca expoe fora do fechamento.
    pub fn do_ambiente_ou_broker(&self, raiz_do_agente: &Path) -> Result<Option<String>, String> {
        if let Some(v) = self
            .variaveis()
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|t| !t.trim().is_empty()))
        {
            return Ok(Some(v.trim().to_string()));
        }
        if !self
            .pasta(raiz_do_agente)
            .join("segredos/master.key")
            .exists()
        {
            return Ok(None);
        }
        let broker = broker(&self.pasta(raiz_do_agente))?;
        match crate::canais::segredo_guardado(&broker, self.nome_do_segredo, self.espaco)? {
            None => Ok(None),
            Some(d) => Credencial::nova(broker, d.uuid, self.espaco)
                .com("send", |t| Ok(Some(t.to_string()))),
        }
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

// Os segredos do catalogo que so tinham a variavel de ambiente (revisao de 01/10/2026):
// cada um ganha o comando `phxclaw <servico> chave` por ESTE mesmo caminho, e o leitor
// passa a olhar ambiente e depois broker (`do_ambiente_ou_broker`). A senha do SMTP nao
// esta aqui: ela ja mora no broker do canal de e-mail (`email::guardar_senha_smtp`).

/// `api.token`: o Bearer da API de tarefas (`phxclaw servir`).
pub const API: Servico = Servico {
    espaco: "api",
    nome_do_segredo: "api-token",
    chave: "api.token",
    aliases: &[],
    rotulo: "o token da API",
    comando: "api chave",
};

/// `ponte.token`: o Bearer do cliente da ponte (`phxclaw ponte`).
pub const PONTE: Servico = Servico {
    espaco: "ponte",
    nome_do_segredo: "ponte-token",
    chave: "ponte.token",
    aliases: &[],
    rotulo: "o token da ponte",
    comando: "ponte chave",
};

/// `dispositivos.token_pareamento`: o token de pareamento do no na ponte (`servir --ponte`).
pub const PAREAMENTO: Servico = Servico {
    espaco: "dispositivos",
    nome_do_segredo: "token-pareamento",
    chave: "dispositivos.token_pareamento",
    aliases: &[],
    rotulo: "o token de pareamento",
    comando: "dispositivos chave",
};

/// `imagem.chave`: a chave do gerador de imagem `openai` do `image_generate`.
pub const IMAGEM: Servico = Servico {
    espaco: "imagem",
    nome_do_segredo: "imagem-chave",
    chave: "imagem.chave",
    aliases: &[],
    rotulo: "a chave do gerador de imagem",
    comando: "imagem chave",
};

/// `plugins.chave_assinatura`: a semente Ed25519 que reassina os manifestos de plugin.
pub const ASSINATURA_DE_PLUGIN: Servico = Servico {
    espaco: "plugins",
    nome_do_segredo: "plugins-chave-assinatura",
    chave: "plugins.chave_assinatura",
    aliases: &[],
    rotulo: "a semente de assinatura de plugin",
    comando: "plugins chave",
};

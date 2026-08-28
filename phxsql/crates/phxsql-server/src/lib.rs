//! # phxsql-server
//!
//! Servidor TCP do PhxSql. Escuta na porta 5000 (configuravel em
//! `config.json`), fala JSON Lines e registra todo acesso com IP, data e hora.

pub mod acesso;
pub mod blacklist;
pub mod config;
pub mod dblink;
pub mod email;
pub mod exportar;
pub mod http;
pub mod juncao;
pub mod ligacoes;
pub mod pivot;
pub mod replica;
pub mod servidor;
pub mod sistema;
pub mod usuarios;
pub mod valores;

pub use acesso::{Acesso, LogAcessos, ResumoIp};
pub use blacklist::{Blacklist, Bloqueio, Firewall, Politica};
pub use config::{Config, Origem, Papel, Replicacao, PORTA_PADRAO};
pub use servidor::Servidor;
pub use usuarios::{Atividade, Cadastro, Permissoes, Usuario};

use std::time::{SystemTime, UNIX_EPOCH};

/// Modelos de `config.json` embutidos no binario, para que
/// `phxsqld --exemplo 2 > config.json` funcione numa maquina sem o repositorio.
///
/// Sao os mesmos arquivos de `exemplos/`, incluidos em tempo de compilacao --
/// entao nao ha como o binario e o repositorio discordarem.
pub const CONFIG_EXEMPLO_01: &str = include_str!("../../../exemplos/Config_exemplo_01.json");
pub const CONFIG_EXEMPLO_02: &str = include_str!("../../../exemplos/Config_exemplo_02.json");
pub const CONFIG_EXEMPLO_03: &str = include_str!("../../../exemplos/Config_exemplo_03.json");

/// Devolve o exemplo pedido: "1" isolado, "2" source, "3" replica.
pub fn config_exemplo(qual: &str) -> Option<&'static str> {
    match qual.trim() {
        "1" | "01" | "isolado" => Some(CONFIG_EXEMPLO_01),
        "2" | "02" | "source" | "master" => Some(CONFIG_EXEMPLO_02),
        "3" | "03" | "replica" | "slave" => Some(CONFIG_EXEMPLO_03),
        _ => None,
    }
}

/// Instante atual em milissegundos desde a epoca Unix.
pub fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

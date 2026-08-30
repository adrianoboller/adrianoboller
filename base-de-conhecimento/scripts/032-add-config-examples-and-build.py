# Add config examples and build
# 27/08 18:44

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
s=s.replace('''use std::time::{SystemTime, UNIX_EPOCH};''','''use std::time::{SystemTime, UNIX_EPOCH};

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
}''')
open(p,'w').write(s)

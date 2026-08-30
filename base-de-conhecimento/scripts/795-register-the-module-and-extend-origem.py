# Register the module and extend Origem
# 28/08 20:16

import pathlib
p = pathlib.Path("crates/phxsql-server/src/lib.rs")
s = p.read_text()
s = s.replace("pub mod pivot;", "pub mod pivot;\npub mod replica;")
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/config.rs")
s = p.read_text()
antigo = """    /// Databases a replicar. Vazio = todos.
    pub databases: Vec<String>,
    /// Segundos entre tentativas quando a conexao cai.
    pub reconectar_em: u64,
}"""
novo = """    /// Databases a replicar. Vazio = todos.
    pub databases: Vec<String>,
    /// Segundos entre tentativas quando a conexao cai.
    pub reconectar_em: u64,
    /// Login com que a replica entra no source.
    pub usuario: String,
    /// Hash da senha desse login -- o MESMO texto do cadastro de usuarios.
    ///
    /// Dele sai a chave derivada do desafio-resposta, entao a replica se
    /// autentica sem que exista senha em claro em lugar nenhum.
    pub senha_hash: String,
    /// Senha em claro. Existe so para quem ainda nao trocou o `config.json`,
    /// e o arranque avisa em voz alta.
    pub senha: String,
}"""
assert antigo in s
s = s.replace(antigo, novo)
antigo = """                                databases: o.textos("databases"),
                                reconectar_em: o.inteiro_ou("reconectar_em", 10).max(1) as u64,"""
novo = """                                databases: o.textos("databases"),
                                reconectar_em: o.inteiro_ou("reconectar_em", 10).max(1) as u64,
                                usuario: o.texto_ou("usuario", "").trim().to_string(),
                                senha_hash: o.texto_ou("senha_hash", "").trim().to_string(),
                                senha: o.texto_ou("senha", "").to_string(),"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

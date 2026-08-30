# Add public key field to Usuario
# 27/08 20:43

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()

s=s.replace('''    pub supervisor: bool,
    pub ativo: bool,''','''    pub supervisor: bool,
    pub ativo: bool,
    /// Chave publica Ed25519, se este usuario tambem prova posse de chave.
    ///
    /// A senha prova que ele SABE alguma coisa; a chave prova que ele TEM
    /// alguma coisa. Quem copiar o config.json leva so a publica, que nao
    /// assina nada -- e a diferenca em relacao ao hash da senha, que e
    /// exatamente o que o desafio-resposta usa para autenticar.
    pub chave_publica: Option<[u8; phxsql_core::ed25519::CHAVE_LEN]>,''')

s=s.replace('''        let bases = match j.campo("bases") {''','''        let chave_publica = match j.campo("chave_publica").and_then(Json::texto) {
            None => None,
            Some(hex) if hex.trim().is_empty() => None,
            Some(hex) => Some(phxsql_core::ed25519::chave_de_hex(hex).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "chave_publica de {login} nao e uma chave Ed25519 (precisa de 64 hexadecimais)"
                ))
            })?),
        };

        let bases = match j.campo("bases") {''')
open(p,'w').write(s)

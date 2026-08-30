# Wire signature verification into login
# 27/08 20:44

p='crates/phxsql-core/src/desafio.rs'
s=open(p).read()
s=s.replace('''/// Calcula a prova, em hexadecimal. E o que o cliente manda.''','''/// A mensagem que a CHAVE assina, quando ha segundo fator.
///
/// E a mesma do HMAC da senha, e isso e de proposito: os dois fatores provam
/// posse sobre exatamente o mesmo desafio, entao a assinatura tambem vale uma
/// vez so e tambem morre com o nonce.
pub fn mensagem_assinada(nonce_servidor: &str, nonce_cliente: &str, usuario: &str) -> Vec<u8> {
    mensagem(nonce_servidor, nonce_cliente, usuario)
}

/// Calcula a prova, em hexadecimal. E o que o cliente manda.''')
open(p,'w').write(s)

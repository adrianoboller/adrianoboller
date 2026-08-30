# Carry the code through the TCP path too
# 28/08 16:26

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# 1) o helper passa a carregar o codigo, e ganha um irmao para os erros de conexao
a='''fn resposta_erro(op: &str, mensagem: &str, ms: u64) -> Json {
    Json::objeto(vec![
        ("ok", Json::Bool(false)),
        ("op", Json::texto_de(op)),
        ("erro", Json::texto_de(mensagem)),
        ("ms", Json::de_u64(ms)),
    ])
}'''
b='''/// A resposta de erro do protocolo, com codigo.
///
/// O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e para quem
/// le, o codigo e para quem programa. Sem ele, integrar com o PhxSql obriga a
/// comparar TEXTO -- e melhorar a redacao de uma mensagem quebraria o cliente
/// sem ninguem perceber.
fn resposta_erro(op: &str, e: &PhxError, ms: u64) -> Json {
    Json::objeto(vec![
        ("ok", Json::Bool(false)),
        ("op", Json::texto_de(op)),
        ("erro", Json::texto_de(e.to_string())),
        ("codigo", Json::de_u64(e.codigo() as u64)),
        ("nome", Json::texto_de(e.nome())),
        ("classe", Json::texto_de(e.classe())),
        ("repetir", Json::Bool(e.adianta_repetir())),
        ("ms", Json::de_u64(ms)),
    ])
}'''
assert a in s; s=s.replace(a,b,1)

# 2) os dois sitios de recusa de conexao passam a construir um erro de verdade
s=s.replace('let _ = writeln!(saida, "{}", resposta_erro("conexao", &motivo, 0).escrever());',
            'let _ = writeln!(\n                    saida,\n                    "{}",\n                    resposta_erro("conexao", &PhxError::Autorizacao(motivo.clone()), 0).escrever()\n                );',1)
s=s.replace('''                resposta_erro("conexao", "ip nao autorizado", 0).escrever()''',
            '''                resposta_erro(\n                    "conexao",\n                    &PhxError::Autorizacao("ip nao autorizado".into()),\n                    0\n                )\n                .escrever()''',1)
s=s.replace('Err(e) => resposta_erro(&op, &e.to_string(), duracao),',
            'Err(e) => resposta_erro(&op, e, duracao),',1)

# 3) o sitio TCP tambem grava o objeto
a='''            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,'''
assert a in s
open(p,'w').write(s)
print('ok')

# Fix the double count and add the scope guard
# 28/08 16:31

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            if let Ok(mut l) = self.ligacoes.lock() {
                l.comecou(
                    id_ligacao,
                    &op,
                    sessao.login(),
                    "",
                    "",
                    quando_ms,
                );
                l.terminou(id_ligacao);
            }'''
b='''            // O login so se sabe DEPOIS: o pedido que autentica e o proprio
            // `login`, e antes dele a sessao ainda esta anonima.
            if let Ok(mut l) = self.ligacoes.lock() {
                l.terminou(id_ligacao, sessao.login());
            }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/src/ligacoes.rs'
s=open(p).read()
a='''    /// Marca o fim da operacao: a conexao volta a esperar.
    pub fn terminou(&mut self, id: u64) {
        if let Some(l) = self.dentro.get_mut(&id) {
            l.op.clear();
            l.op_desde_ms = 0;
            l.database.clear();
            l.tabela.clear();
        }
    }'''
b='''    /// Marca o fim da operacao: a conexao volta a esperar.
    ///
    /// Leva o login junto porque o pedido que autentica e o proprio `login`:
    /// quando ele COMECOU a sessao ainda era anonima, e so aqui se sabe quem
    /// entrou.
    pub fn terminou(&mut self, id: u64, usuario: &str) {
        if let Some(l) = self.dentro.get_mut(&id) {
            l.op.clear();
            l.op_desde_ms = 0;
            l.database.clear();
            l.tabela.clear();
            if !usuario.is_empty() {
                l.usuario = usuario.to_string();
            }
        }
    }'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('        l.terminou(id);\n', '        l.terminou(id, "adriano");\n',1)
open(p,'w').write(s)

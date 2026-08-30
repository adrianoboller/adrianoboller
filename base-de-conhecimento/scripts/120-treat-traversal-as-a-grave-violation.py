# Treat traversal as a grave violation
# 27/08 20:18

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho = '''        if self.config.politica.base_proibida(&base) {'''
novo = '''        // Nome com ".." ou barra nao e engano de digitacao: e sondagem de
        // travessia de diretorio. O motor ja recusava -- mas recusava calado, e
        // quem sonda podia tentar a noite inteira sem nunca ser barrado. Agora
        // e violacao grave, igual a comando proibido: bloqueia na primeira.
        for (rotulo, valor) in [
            ("database", &base),
            ("tabela", &pedido.texto_ou("tabela", "").to_string()),
            ("schema", &pedido.texto_ou("schema", "").to_string()),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                self.violacao_grave(ip, &op, "tentativa de travessia de diretorio");
                return (
                    op,
                    false,
                    Err(PhxError::Autorizacao(format!(
                        "{rotulo} {valor:?} nao e um nome; o IP foi bloqueado"
                    ))),
                );
            }
        }

        if self.config.politica.base_proibida(&base) {'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
print('servidor ok')

# Fix borrow error
# 27/08 20:35

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return Err((
                op,
                PhxError::Autorizacao(format!("operacao {op} esta proibida neste servidor")),
            ));
        }''','''        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            let erro = PhxError::Autorizacao(format!("operacao {op} esta proibida neste servidor"));
            return Err((op, erro));
        }''')
open(p,'w').write(s)

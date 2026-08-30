# Fix challenge restore, UI health probe, run tests
# 27/08 19:45

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s = s.replace('''        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() {''','''        // Um desafio em aberto so e consumido por um login. Qualquer outra
        // operacao no meio do caminho devolve o nonce para a sessao, senao um
        // "ping" entre o desafio e o login derrubaria a prova.
        if op != "login" && op != "desafio" {
            if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                vivas.guardar_desafio(&id_sessao, d);
            }
        }

        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() {''')
open(p,'w').write(s)

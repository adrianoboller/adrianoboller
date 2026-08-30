# Route API requests through the remote connection
# 27/08 20:34

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# no api_http: se a sessao tem remoto, encaminha em vez de executar aqui
velho = '''        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();
        let (op, autenticado, resultado) = self.despachar(&pedido.corpo, &mut sessao, ip);
        let ms = inicio.elapsed().as_millis() as u64;'''
novo = '''        // Abrir conexao para outro PhxSql, se o login pediu um destino.
        let destino = Json::analisar(&pedido.corpo)
            .ok()
            .map(|j| j.texto_ou("destino", "").trim().to_string())
            .unwrap_or_default();

        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();

        let ja_remota = self
            .remotos
            .lock()
            .ok()
            .and_then(|r| r.get(&id_sessao).cloned());

        let (op, autenticado, resultado) = match (&ja_remota, destino.is_empty()) {
            // Sessao ja amarrada a um servidor remoto: tudo vai para la.
            (Some(conexao), _) => self.encaminhar(conexao, &pedido.corpo, ip),
            // Login novo pedindo destino: abre, encaminha, e guarda se entrou.
            (None, false) => {
                let r = self.abrir_remoto(&destino, &pedido.corpo, ip);
                match r {
                    Ok((op, valor, conexao)) => {
                        if id_sessao.is_empty() {
                            if let Ok(mut vivas) = self.sessoes.lock() {
                                id_sessao = vivas.nova("", duracao, agora);
                            }
                        }
                        if let Ok(mut r) = self.remotos.lock() {
                            r.insert(id_sessao.clone(), conexao);
                        }
                        (op, true, Ok(valor))
                    }
                    Err((op, e)) => (op, false, Err(e)),
                }
            }
            (None, true) => self.despachar(&pedido.corpo, &mut sessao, ip),
        };
        let remota = ja_remota.is_some() || !destino.is_empty();
        let ms = inicio.elapsed().as_millis() as u64;'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# nao mexer nas sessoes locais quando a conversa e remota
s=s.replace('''        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() {''','''        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() && !remota {''')

# sair tambem derruba a conexao remota
s=s.replace('''                "sair" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        vivas.encerrar(&id_sessao);
                    }
                    id_sessao.clear();
                }''','''                "sair" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        vivas.encerrar(&id_sessao);
                    }
                    if let Ok(mut r) = self.remotos.lock() {
                        r.remove(&id_sessao);
                    }
                    id_sessao.clear();
                }''')

# e quando a sessao e remota, o sair tem de fechar tambem
s=s.replace('''        let codigo = match &resultado {
            Ok(_) => 200,''','''        if remota && op == "sair" {
            if let Ok(mut r) = self.remotos.lock() {
                r.remove(&id_sessao);
            }
            if let Ok(mut vivas) = self.sessoes.lock() {
                vivas.encerrar(&id_sessao);
            }
            id_sessao.clear();
        }

        let codigo = match &resultado {
            Ok(_) => 200,''')
open(p,'w').write(s)
print('api_http ok')

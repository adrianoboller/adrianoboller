# Wire signature verification into login
# 27/08 20:44

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho='''        match autenticado {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }'''
novo='''        // Segundo fator: quem tem chave publica no config.json tambem assina.
        //
        // A mensagem assinada e a MESMA do desafio-resposta -- os dois nonces
        // e o login --, entao a assinatura tambem vale uma vez so. Nao ha
        // atalho: sem desafio aberto nao ha o que assinar.
        if let Some(u) = &autenticado {
            if let Some(publica) = &u.chave_publica {
                let (nonce, nonce_cliente) = match &nonces {
                    Some(par) => par.clone(),
                    None => {
                        return Err(PhxError::Autorizacao(
                            "este usuario exige chave: peca um desafio e mande a prova assinada"
                                .into(),
                        ))
                    }
                };
                let hex = p.texto_ou("assinatura", "");
                let assinatura = phxsql_core::ed25519::assinatura_de_hex(hex).ok_or_else(|| {
                    PhxError::Autorizacao(
                        "este usuario exige \\"assinatura\\" com 128 hexadecimais".into(),
                    )
                })?;
                let mensagem = phxsql_core::desafio::mensagem_assinada(
                    &nonce,
                    &nonce_cliente,
                    &login,
                );
                if !phxsql_core::ed25519::conferir(publica, &mensagem, &assinatura) {
                    return Err(recusa());
                }
            }
        }

        match autenticado {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# guarda os nonces usados, para a assinatura poder conferir a mesma mensagem
s=s.replace('''        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {''','''        let mut nonces: Option<(String, String)> = None;
        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {''')
s=s.replace('''            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            match self.config.cadastro.por_login(&login) {''','''            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            nonces = Some((nonce.clone(), nonce_cliente.to_string()));
            match self.config.cadastro.por_login(&login) {''')
open(p,'w').write(s)

# Register connections in the TCP loop
# 28/08 16:30

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# 1) campo no Servidor
a='''    /// Quando o servidor subiu, para o `ping` poder dizer ha quanto tempo.'''
b='''    /// As conexoes vivas, para o operador ver quem esta falando e poder
    /// derrubar quem travou.
    ligacoes: Mutex<crate::ligacoes::Ligacoes>,
    /// Quando o servidor subiu, para o `ping` poder dizer ha quanto tempo.'''
assert a in s; s=s.replace(a,b,1)
a='''            desde_ms: crate::agora_ms(),'''
b='''            ligacoes: Mutex::new(crate::ligacoes::Ligacoes::default()),
            desde_ms: crate::agora_ms(),'''
assert a in s; s=s.replace(a,b,1)

# 2) registro no laco de conexao
a='''        let permitido = self.config.ip_permitido(&ip);
        let escrita = fluxo.try_clone();
        let mut leitor = BufReader::new(fluxo);
        let mut saida = match escrita {'''
b='''        let permitido = self.config.ip_permitido(&ip);
        let escrita = fluxo.try_clone();
        // O soquete vai para o registro para que `encerrar_sessao` consiga
        // fecha-lo de fora: a thread desta conexao passa a vida parada dentro
        // de um `read_line`, e so um `shutdown` a acorda.
        let para_fechar = fluxo.try_clone().ok().map(Arc::new);
        let mut leitor = BufReader::new(fluxo);
        let mut saida = match escrita {'''
assert a in s; s=s.replace(a,b,1)

a='''        let mut sessao = Sessao::default();
        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => return, // conexao fechada
                Ok(_) => {}
                Err(_) => return,
            }
            if linha.trim().is_empty() {
                continue;
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;'''
b='''        let mut sessao = Sessao::default();
        let (id_ligacao, morrer) = match self.ligacoes.lock() {
            Ok(mut l) => l.entrar(&ip, porta, crate::agora_ms(), para_fechar),
            Err(_) => (0, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        };
        // Sai do registro por qualquer caminho -- inclusive os `return` do
        // meio do laco. Sem isto, uma conexao caida ficaria na lista para
        // sempre, e a lista que existe para dizer a verdade passaria a mentir.
        let _saida_do_registro = AoSair(|| {
            if let Ok(mut l) = self.ligacoes.lock() {
                l.sair(id_ligacao);
            }
        });

        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => return, // conexao fechada
                Ok(_) => {}
                Err(_) => return,
            }
            // Conferido AQUI, e nao so no `shutdown`: se o pedido chegou junto
            // com o encerramento, quem mandou encerrar ganha.
            if morrer.load(Ordering::SeqCst) {
                return;
            }
            if linha.trim().is_empty() {
                continue;
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                if let Ok(mut l) = self.ligacoes.lock() {
                    l.comecou(
                        id_ligacao,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        quando_ms,
                    );
                }
            }
            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;
            if let Ok(mut l) = self.ligacoes.lock() {
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
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

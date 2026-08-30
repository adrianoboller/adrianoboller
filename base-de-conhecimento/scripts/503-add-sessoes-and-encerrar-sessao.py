# Add sessoes and encerrar_sessao
# 28/08 16:31

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''    // ------------------------------------------------------- estatisticas'''
b='''    /// Quem esta falando com o servidor agora.
    ///
    /// E o `SHOW PROCESSLIST`: sem ele, quando uma consulta prende a trava de
    /// dados nao havia como saber QUEM esta segurando -- so que estava lento.
    fn op_sessoes(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let l = self.ligacoes.lock().map_err(|_| trava_envenenada())?;
        let todas = l.todas();
        // A mais demorada primeiro: quando algo trava, e ela que interessa.
        let mais_longa = todas
            .iter()
            .filter(|x| x.op_desde_ms > 0)
            .map(|x| agora - x.op_desde_ms)
            .max()
            .unwrap_or(0);
        Ok(Json::objeto(vec![
            ("quantas", Json::de_u64(todas.len() as u64)),
            (
                "executando",
                Json::de_u64(todas.iter().filter(|x| !x.op.is_empty()).count() as u64),
            ),
            ("mais_longa_ms", Json::de_u64(mais_longa.max(0) as u64)),
            (
                "sessoes",
                Json::Lista(todas.iter().map(|x| x.para_json(agora)).collect()),
            ),
        ]))
    }

    /// Derruba uma conexao pelo numero.
    ///
    /// E o `KILL` -- e o que ele alcanca esta dito na resposta, em vez de
    /// prometer mais do que faz: fecha o soquete, o que e imediato para a
    /// conexao parada esperando pedido. Uma operacao que ja entrou na trava de
    /// dados termina assim mesmo; o que muda e que o resultado nao vai para
    /// lugar nenhum e a conexao nao volta.
    fn op_encerrar_sessao(&self, p: &Json) -> Result<Json> {
        let id = p.inteiro_ou("id", 0);
        if id <= 0 {
            return Err(PhxError::Esquema(
                "encerrar_sessao sem \\"id\\": o numero vem da operacao `sessoes`".into(),
            ));
        }
        let id = id as u64;
        let agora = crate::agora_ms();
        let mut l = self.ligacoes.lock().map_err(|_| trava_envenenada())?;
        let antes = l.todas().into_iter().find(|x| x.id == id);
        if !l.encerrar(id) {
            return Err(PhxError::NaoEncontrado(format!(
                "nao ha conexao {id}; a lista esta em `sessoes`"
            )));
        }
        let executando = antes.as_ref().map(|x| !x.op.is_empty()).unwrap_or(false);
        Ok(Json::objeto(vec![
            ("encerrada", Json::de_u64(id)),
            (
                "estava",
                Json::texto_de(if executando { "executando" } else { "esperando" }),
            ),
            (
                "op",
                match antes.as_ref().map(|x| x.op.clone()).unwrap_or_default() {
                    o if o.is_empty() => Json::Nulo,
                    o => Json::texto_de(o),
                },
            ),
            // Dito na resposta, e nao so na documentacao: quem manda encerrar
            // precisa saber se ja acabou ou se ainda vai acabar.
            (
                "aviso",
                Json::texto_de(if executando {
                    "a operacao em curso termina antes de a conexao fechar: nao ha como \\
                     abandonar uma varredura no meio sem arriscar deixar a tabela aberta \\
                     pela metade. O resultado nao vai para lugar nenhum"
                } else {
                    "a conexao estava esperando pedido e foi fechada na hora"
                }),
            ),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
        ]))
    }

    // ------------------------------------------------------- estatisticas'''
assert a in s; s=s.replace(a,b,1)
a='''            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),'''
b='''            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),
            "sessoes" | "processlist" => self.op_sessoes(),
            "encerrar_sessao" | "kill" => self.op_encerrar_sessao(p),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

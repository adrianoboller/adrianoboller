# Include web sessions in the list and in kill
# 28/08 16:37

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''        Ok(Json::objeto(vec![
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
        ]))'''
b='''        // As sessoes do navegador entram na MESMA lista. Quem pergunta "quem
        // esta conectado?" quer os dois -- e uma lista que so mostra a porta de
        // dados nao mostra quem esta olhando a propria tela.
        let web: Vec<Json> = self
            .sessoes
            .lock()
            .map(|s| {
                s.listar(agora)
                    .into_iter()
                    .map(|(id, login, desde, expira)| {
                        Json::objeto(vec![
                            ("id", Json::texto_de(&id)),
                            ("origem", Json::texto_de("web")),
                            (
                                "usuario",
                                match login.is_empty() {
                                    true => Json::Nulo,
                                    false => Json::texto_de(login),
                                },
                            ),
                            (
                                "desde",
                                Json::texto_de(phxsql_core::datahora::instante_iso(desde)),
                            ),
                            ("aberta_s", Json::de_u64(((agora - desde) / 1_000).max(0) as u64)),
                            (
                                "expira_em_s",
                                Json::de_u64(((expira - agora) / 1_000).max(0) as u64),
                            ),
                        ])
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Json::objeto(vec![
            ("quantas", Json::de_u64(todas.len() as u64)),
            (
                "executando",
                Json::de_u64(todas.iter().filter(|x| !x.op.is_empty()).count() as u64),
            ),
            ("mais_longa_ms", Json::de_u64(mais_longa.max(0) as u64)),
            (
                "sessoes",
                Json::Lista(
                    todas
                        .iter()
                        .map(|x| {
                            let mut j = x.para_json(agora);
                            if let Json::Objeto(campos) = &mut j {
                                campos.push(("origem".into(), Json::texto_de("dados")));
                            }
                            j
                        })
                        .collect(),
                ),
            ),
            ("web", Json::Lista(web.clone())),
            ("sessoes_web", Json::de_u64(web.len() as u64)),
        ]))'''
assert a in s; s=s.replace(a,b,1)

# encerrar tambem aceita sessao web, pelo prefixo do id
a='''    fn op_encerrar_sessao(&self, p: &Json) -> Result<Json> {
        let id = p.inteiro_ou("id", 0);'''
b='''    fn op_encerrar_sessao(&self, p: &Json) -> Result<Json> {
        // Sessao do navegador vem por texto ("a1b2c3d4"); conexao da porta de
        // dados, por numero. Aceitar os dois no mesmo campo evita duas
        // operacoes para a mesma pergunta.
        if let Some(texto) = p.campo("id").and_then(Json::texto) {
            if texto.chars().any(|c| !c.is_ascii_digit()) {
                let mut s = self.sessoes.lock().map_err(|_| trava_envenenada())?;
                if !s.encerrar_por_prefixo(texto) {
                    return Err(PhxError::NaoEncontrado(format!(
                        "nao ha sessao web {texto:?}; a lista esta em `sessoes`"
                    )));
                }
                return Ok(Json::objeto(vec![
                    ("encerrada", Json::texto_de(texto)),
                    ("origem", Json::texto_de("web")),
                    ("estava", Json::texto_de("aberta")),
                    (
                        "aviso",
                        Json::texto_de(
                            "a sessao do navegador foi invalidada: o proximo clique cai no login",
                        ),
                    ),
                ]));
            }
        }
        let id = p.inteiro_ou("id", 0);'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

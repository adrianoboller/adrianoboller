# Add relay open and forward functions
# 27/08 20:35

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
ancora = '    /// O `/api`: o mesmo protocolo da porta 5000, um pedido por vez.'
assert s.count(ancora)==1
bloco = '''    /// Abre uma conexao para outro PhxSql e manda o login por ela.
    ///
    /// A politica DESTE servidor vale antes de qualquer coisa sair daqui:
    /// comando proibido aqui nao vira pedido la. A interface nao e uma porta
    /// dos fundos para o que a porta da frente recusa.
    #[allow(clippy::type_complexity)]
    fn abrir_remoto(
        &self,
        destino: &str,
        linha: &str,
        ip: &str,
    ) -> std::result::Result<(String, Json, Arc<Mutex<Remoto>>), (String, PhxError)> {
        let op = Json::analisar(linha)
            .map(|j| j.texto_ou("op", "login").to_string())
            .unwrap_or_else(|_| "login".into());

        if !self.config.web.destinos_permitidos_algum() {
            return Err((
                op,
                PhxError::Autorizacao(
                    "esta interface nao fala com outro servidor: preencha web.destinos no config.json".into(),
                ),
            ));
        }
        if !self.config.web.destino_permitido(destino) {
            // Endereco fora da lista e sondagem de rede, nao engano: alguem
            // esta procurando o que mais existe do outro lado.
            self.violacao_grave(ip, &op, "destino fora de web.destinos");
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino} nao esta em web.destinos; o IP foi bloqueado"
                )),
            ));
        }
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return Err((
                op,
                PhxError::Autorizacao(format!("operacao {op} esta proibida neste servidor")),
            ));
        }

        let mut remoto = Remoto::abrir(destino, self.config.timeout_s).map_err(|e| (op.clone(), e))?;
        let resposta = remoto.conversar(linha).map_err(|e| (op.clone(), e))?;
        if !resposta.booleano_ou("ok", false) {
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino}: {}",
                    resposta.texto_ou("erro", "recusou o login")
                )),
            ));
        }
        let valor = resposta.campo("resultado").cloned().unwrap_or(Json::Nulo);
        Ok((op, valor, Arc::new(Mutex::new(remoto))))
    }

    /// Manda o pedido para o servidor remoto desta sessao.
    fn encaminhar(
        &self,
        conexao: &Arc<Mutex<Remoto>>,
        linha: &str,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let op = match Json::analisar(linha) {
            Ok(j) => {
                let o = j.texto_ou("op", "ping").trim().to_string();
                if o.is_empty() {
                    "ping".to_string()
                } else {
                    o
                }
            }
            Err(e) => return ("?".into(), false, Err(e)),
        };
        // A politica local vale para o que passa por aqui, mesmo indo embora.
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(format!(
                    "operacao {op} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }
        let mut r = match conexao.lock() {
            Ok(r) => r,
            Err(_) => return (op, false, Err(trava_envenenada())),
        };
        match r.conversar(linha) {
            Ok(resposta) => {
                if resposta.booleano_ou("ok", false) {
                    (
                        op,
                        true,
                        Ok(resposta.campo("resultado").cloned().unwrap_or(Json::Nulo)),
                    )
                } else {
                    let erro = resposta.texto_ou("erro", "o servidor remoto recusou").to_string();
                    (op, true, Err(PhxError::Autorizacao(erro)))
                }
            }
            Err(e) => (op, true, Err(e)),
        }
    }

''' + ancora
s=s.replace(ancora, bloco)
open(p,'w').write(s)

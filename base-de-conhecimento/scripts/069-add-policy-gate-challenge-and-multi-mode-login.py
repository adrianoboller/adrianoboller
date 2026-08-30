# Add policy gate, challenge and multi-mode login
# 27/08 19:26

p='crates/phxsql-server/src/servidor.rs'
linhas=open(p).read().split('\n')
ini=next(i for i,l in enumerate(linhas) if l.strip().startswith('/// Le o pedido e o leva por tres portoes'))
fim=next(i for i,l in enumerate(linhas) if i>ini and l.strip()=='}' and 'Err(PhxError::Autorizacao("usuario ou senha invalidos".into()))' in linhas[i-3])
novo = r'''    /// Le o pedido e o leva pelos portoes, nesta ordem: politica (o que ninguem
    /// pode), token (a rede), login (a identidade) e permissao (o poder).
    fn despachar(
        &self,
        linha: &str,
        sessao: &mut Sessao,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() { "ping".to_string() } else { op };
        let base = pedido.texto_ou("database", "").to_string();

        // Portao 0 -- a politica. Vale para todo mundo, root inclusive: e o
        // que o config.json diz que ninguem pede por esta porta. Pedir vira
        // bloqueio na hora, sem contar tentativa.
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
        if self.config.politica.base_proibida(&base) {
            self.violacao_grave(ip, &op, "base proibida pela politica");
            return (
                op,
                false,
                Err(PhxError::Autorizacao(format!(
                    "a base {base} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            self.violacao_leve(ip, &op, "token invalido");
            return (op, false, Err(PhxError::Autorizacao("token invalido".into())));
        }

        // Portao 2 -- o login.
        if op == "desafio" {
            let r = self.op_desafio(&pedido, sessao);
            return (op, true, r);
        }
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            if r.is_err() {
                self.violacao_leve(ip, "login", "credencial invalida");
            }
            return (op, r.is_ok(), r);
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()
            && Atividade::da_operacao(&op).is_some()
        {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}".into(),
                )),
            );
        }

        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                )),
            );
        }

        // Portao 3 -- o poder deste usuario sobre a base deste pedido.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            if !usuario.pode(&base, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        if base.is_empty() { "(sem base)" } else { &base }
                    ))),
                );
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Abre um desafio: devolve sal, iteracoes e um nonce de uso unico.
    ///
    /// Usuario que nao existe recebe um desafio de aparencia normal, com sal
    /// derivado do proprio login -- assim quem sonda nao descobre quem existe
    /// pela resposta.
    fn op_desafio(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\"".into()));
        }
        let (sal_hex, iteracoes) = match self.config.cadastro.por_login(&login) {
            Some(u) => {
                let (sal, it) = phxsql_core::senha::sal_e_iteracoes(&u.senha_hash)?;
                (phxsql_core::hash::para_hex(&sal), it)
            }
            None => {
                // Sal falso, estavel por login e indistinguivel de um real.
                let falso = phxsql_core::hash::hmac_sha256(
                    self.config.token.as_bytes(),
                    login.as_bytes(),
                );
                (
                    phxsql_core::hash::para_hex(&falso[..16]),
                    phxsql_core::senha::ITERACOES_PADRAO,
                )
            }
        };

        let nonce = phxsql_core::desafio::nonce();
        sessao.desafio = Some((
            login,
            nonce.clone(),
            crate::agora_ms() + phxsql_core::desafio::VALIDADE_MS,
        ));
        Ok(Json::objeto(vec![
            ("sal", Json::texto_de(sal_hex)),
            ("iteracoes", Json::de_u64(iteracoes as u64)),
            ("nonce", Json::texto_de(nonce)),
            ("validade_ms", Json::de_i64(phxsql_core::desafio::VALIDADE_MS)),
        ]))
    }

    /// Confere a credencial e guarda a identidade na conexao.
    ///
    /// Aceita tres formas, da mais segura para a menos:
    ///
    /// 1. `prova` + `nonce_cliente` -- desafio-resposta. A senha nao sai da
    ///    maquina do cliente.
    /// 2. `senha_b64` -- Base64. Some do grep e do olho, mas quem captura o
    ///    pacote decodifica: NAO e cifra.
    /// 3. `senha` -- texto puro.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = match p.campo("usuario_b64").and_then(Json::texto) {
            Some(b) => phxsql_core::base64::decodificar_texto(b)?,
            None => p
                .texto_ou("usuario", p.texto_ou("login", ""))
                .to_string(),
        };
        let login = login.trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }

        // Todo caminho de erro devolve a MESMA mensagem, para nao dizer se o
        // que falhou foi o login, a senha ou o desafio.
        let recusa = || PhxError::Autorizacao("usuario ou senha invalidos".into());

        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {
                PhxError::Autorizacao("peca um desafio antes de mandar a prova".into())
            })?;
            if crate::agora_ms() > expira {
                return Err(PhxError::Autorizacao("o desafio expirou; peca outro".into()));
            }
            if usuario_desafio != login {
                return Err(recusa());
            }
            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            match self.config.cadastro.por_login(&login) {
                Some(u) if u.ativo => {
                    let dk = phxsql_core::senha::derivado_do_hash(&u.senha_hash)?;
                    phxsql_core::desafio::conferir_prova(
                        &dk,
                        &nonce,
                        nonce_cliente,
                        &login,
                        prova,
                    )
                    .then_some(u)
                }
                _ => None,
            }
        } else {
            // (2) Base64 ou (3) texto puro
            let clara = match p.campo("senha_b64").and_then(Json::texto) {
                Some(b) => phxsql_core::base64::decodificar_texto(b)?,
                None => p.texto_ou("senha", "").to_string(),
            };
            self.config.cadastro.autenticar(&login, &clara)
        };

        match autenticado {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                Err(recusa())
            }
        }
    }'''.split('\n')
linhas[ini:fim+1]=novo
open(p,'w').write('\n'.join(linhas))
print("despachar, desafio e login substituidos")

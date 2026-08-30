# Replace despachar with the three-gate version
# 27/08 19:05

p='crates/phxsql-server/src/servidor.rs'
linhas=open(p).read().split('\n')
# Acha o inicio do comentario de despachar e o fim da funcao.
ini=next(i for i,l in enumerate(linhas) if l.strip().startswith('/// Le o pedido, confere o token'))
fim=next(i for i,l in enumerate(linhas) if i>ini and l.strip()=='}' and linhas[i-1].strip()=='(op, true, r)')
novo = r'''    /// Le o pedido e o leva por tres portoes, nesta ordem: o token (a rede),
    /// o login (a identidade) e a permissao (o poder). Devolve (operacao,
    /// autenticado, resultado) para que o log registre mesmo o que falhou.
    fn despachar(&self, linha: &str, sessao: &mut Sessao) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() { "ping".to_string() } else { op };

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            return (op, false, Err(PhxError::Esquema("token invalido".into())));
        }

        // Portao 2 -- o login. Havendo cadastro, o token sozinho nao basta.
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            return (op, r.is_ok(), r);
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()
            && Atividade::da_operacao(&op).is_some()
        {
            return (
                op,
                true,
                Err(PhxError::Esquema(
                    "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}".into(),
                )),
            );
        }

        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Esquema("servidor em modo somente leitura".into())),
            );
        }

        // Portao 3 -- o poder deste usuario sobre a base deste pedido.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            let base = pedido.texto_ou("database", "");
            if !usuario.pode(base, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Esquema(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        if base.is_empty() { "(sem base)" } else { base }
                    ))),
                );
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Confere login e senha e guarda a identidade na conexao.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        let clara = p.texto_ou("senha", "");
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }
        match self.config.cadastro.autenticar(&login, clara) {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                // Mensagem unica de proposito: nao dizer se o que errou foi o
                // login ou a senha.
                Err(PhxError::Esquema("usuario ou senha invalidos".into()))
            }
        }
    }'''.split('\n')
linhas[ini:fim+1]=novo
open(p,'w').write('\n'.join(linhas))
print("despachar substituido")

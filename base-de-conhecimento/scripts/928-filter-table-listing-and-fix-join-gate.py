# Filter table listing and fix join gate
# 29/08 00:26

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# ---------------------------------------------- op_tabelas passa a filtrar
alvo = '''    fn op_tabelas(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            (
                "tabelas",
                Json::Lista(
                    db.todas_as_tabelas()?
                        .into_iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
        ]))
    }'''
novo = '''    /// `tabelas`: as tabelas da base, **so as que quem pediu pode ler**.
    ///
    /// Filtrar aqui nao e enfeite. Sem isto, quem perdeu o direito a `folha`
    /// continuaria vendo o nome dela na arvore e so descobriria a recusa ao
    /// clicar -- e o nome de uma tabela ja conta parte da historia. A arvore
    /// mostra o que da para abrir.
    fn op_tabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(nome)?;
        let todas = db.todas_as_tabelas()?;
        let visiveis: Vec<Json> = todas
            .into_iter()
            .filter(|t| self.pode_ver_tabela(sessao, nome, t))
            .map(Json::texto_de)
            .collect();
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            ("tabelas", Json::Lista(visiveis)),
        ]))
    }

    /// Quem esta na sessao pode LER esta tabela desta base?
    ///
    /// Sem sessao -- servidor sem cadastro -- e sim: o portao de usuario nao
    /// existe naquele modo, e inventar um aqui negaria tudo.
    fn pode_ver_tabela(&self, sessao: &Sessao, database: &str, tabela: &str) -> bool {
        match &sessao.usuario {
            None => true,
            Some(u) => u.pode_em(database, tabela, Atividade::Ler),
        }
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace('''            "tabelas" => self.op_tabelas(p),''',
              '''            "tabelas" => self.op_tabelas(p, sessao),''', 1)

# ---------------------------------------------- juncao: as DUAS tabelas
alvo = '''        // O portao geral ja conferiu `ler`; isto e o cinto, e vale para as
        // DUAS tabelas -- uma junção que le B tem de pedir permissao de B.
        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            if !u.pode(base, Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {base}",
                    u.login
                )));
            }
        }'''
novo = '''        // O portao geral confere o campo `tabela` do pedido -- e uma junção
        // NAO TEM esse campo: as duas tabelas moram em `a.tabela` e
        // `b.tabela`. Sem esta conferencia, juntar seria a porta dos fundos
        // para ler uma tabela negada, bastando pedi-la como o lado B.
        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            for alvo in [na, nb] {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

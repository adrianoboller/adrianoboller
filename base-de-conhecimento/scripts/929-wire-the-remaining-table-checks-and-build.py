# Wire the remaining table checks and build
# 29/08 00:26

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
alvo = '''        if let Some(u) = &sessao.usuario {
            if !u.pode(base, Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {base}",
                    u.login
                )));
            }
        }

        let nomes: Vec<String> = p
            .campo("tabelas")
            .and_then(Json::lista)'''
novo = '''        let nomes: Vec<String> = p
            .campo("tabelas")
            .and_then(Json::lista)'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = '''        if nomes.len() < 2 {
            return Err(PhxError::Esquema(
                "a união precisa de ao menos duas tabelas em \\"tabelas\\"".into(),
            ));
        }

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(base)?;'''
novo = '''        if nomes.len() < 2 {
            return Err(PhxError::Esquema(
                "a união precisa de ao menos duas tabelas em \\"tabelas\\"".into(),
            ));
        }

        // A conferencia vem DEPOIS de ler a lista, e nao antes, porque e a
        // lista que diz o que precisa ser conferido: o campo `tabela` que o
        // portao geral olha nao existe numa união. Cada tabela do pedido
        // precisa da sua propria permissao -- senao unir vira a porta dos
        // fundos para ler uma tabela negada.
        if let Some(u) = &sessao.usuario {
            for alvo in &nomes {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(base)?;'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ------------------------------------------------ copiar_tabela: o destino
s = s.replace('''            if !u.pode(destino_db, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {destino_db}"
                )));
            }''','''            if !u.pode_em(destino_db, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {destino_db}.{destino}"
                )));
            }''',1)

# ------------------------------------------------ painel: conta so o visivel
alvo = '''                let db = dados.abrir_database(&nome)?;
                let lista = db.todas_as_tabelas()?;'''
novo = '''                let db = dados.abrir_database(&nome)?;
                // E so o que quem olha poderia abrir, tabela a tabela: o
                // total do painel nao pode contar linha de tabela negada.
                let lista: Vec<String> = db
                    .todas_as_tabelas()?
                    .into_iter()
                    .filter(|t| self.pode_ver_tabela(sessao, &nome, t))
                    .collect();'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ------------------------------------------------ memoria: cinto por tabela
alvo = '''        if let Some(u) = &sessao.usuario {
            if !u.pode(p.texto_ou("database", ""), Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {}",
                    u.login,
                    p.texto_ou("database", "")
                )));
            }
        }'''
novo = '''        if let Some(u) = &sessao.usuario {
            let (base, tabela) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
            if !u.pode_em(base, tabela, Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {base}.{tabela}",
                    u.login
                )));
            }
        }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

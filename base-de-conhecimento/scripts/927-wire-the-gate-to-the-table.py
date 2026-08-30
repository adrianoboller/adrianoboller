# Wire the gate to the table
# 29/08 00:25

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace('''            bases: vec![("*".into(), permissoes)],
        });''','''            bases: vec![("*".into(), permissoes)],
            tabelas: Vec::new(),
        });''',1)

# ---- o portao passa a olhar a tabela
alvo = '''        // Portao 3 -- o poder deste usuario sobre a base deste pedido.
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
        }'''
novo = '''        // Portao 3 -- o poder deste usuario sobre a base E A TABELA do pedido.
        //
        // A tabela entra aqui, e nao la dentro de cada operacao, porque o
        // portao tem de ser UM: espalhado por quarenta operacoes, a que
        // alguem esquecer de conferir vira a porta dos fundos, e ninguem
        // descobre por leitura.
        //
        // Pedido sem tabela -- `bancos`, `criar_database`, `sistema` -- cai na
        // regra da base, que e como sempre foi.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            let tabela = pedido.texto_ou("tabela", "").trim().to_string();
            if !usuario.pode_em(&base, &tabela, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        match (base.is_empty(), tabela.is_empty()) {
                            (true, _) => "(sem base)".to_string(),
                            (false, true) => base.clone(),
                            (false, false) => format!("{base}.{tabela}"),
                        }
                    ))),
                );
            }
        }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

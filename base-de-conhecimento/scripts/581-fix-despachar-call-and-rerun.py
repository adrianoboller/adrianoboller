# Fix despachar call and rerun
# 28/08 17:42

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let sessao = Sessao {
            usuario: Some(usuario),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar("lixeira", &pedido(r#"{"database":"b","tabela":"c"}"#), &sessao);
        let e = r.unwrap_err();
        assert!(
            format!("{e}").contains("administrar"),
            "o operador entrou na lixeira: {e}"
        );

        // Mas excluir ele pode.
        let (_, _, r) = s.despachar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
            &sessao,
        );
        assert!(r.is_ok(), "o operador nao conseguiu excluir: {r:?}");
    }'''
novo='''        let mut sessao = Sessao {
            usuario: Some(usuario),
            ..Sessao::default()
        };
        // Pelo `despachar`, que e por onde o pedido entra de verdade: e ali
        // que mora o portao de permissao, e nao no `executar`.
        let (_, _, r) = s.despachar(
            r#"{"op":"lixeira","database":"b","tabela":"c"}"#,
            &mut sessao,
            "1.2.3.4",
        );
        let e = r.unwrap_err();
        assert!(
            format!("{e}").contains("administrar"),
            "o operador entrou na lixeira: {e}"
        );

        let (_, _, r) = s.despachar(
            r#"{"op":"motivos","database":"b","tabela":"c"}"#,
            &mut sessao,
            "1.2.3.4",
        );
        assert!(r.is_err(), "o operador leu os motivos");

        // Mas excluir ele pode.
        let (_, _, r) = s.despachar(
            r#"{"op":"excluir","database":"b","tabela":"c","rowid":1}"#,
            &mut sessao,
            "1.2.3.4",
        );
        assert!(r.is_ok(), "o operador nao conseguiu excluir: {r:?}");
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

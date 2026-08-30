# Add server-level cursor tests
# 28/08 18:34

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// E o portao de verdade, com um usuario que tem tudo menos administrar.'''
novo='''    /// A prova da paginacao por cursor: pedir pagina a pagina reconstroi
    /// exatamente a tabela, sem repetir nem pular -- inclusive por cima dos
    /// buracos que a exclusao deixa.
    #[test]
    fn o_cursor_reconstroi_a_tabela_inteira() {
        let dir = dir_temp("cursor");
        let s = servidor(&dir, Cadastro::default());
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        for id in 1..=25 {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","linha":{{"id":{id}}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
        // Dois buracos: um marcado, um apagado de vez.
        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":7}"#),
            &sessao,
        )
        .unwrap();
        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":13,"fisico":true}"#),
            &sessao,
        )
        .unwrap();

        let mut vistos: Vec<i64> = Vec::new();
        let mut cursor = 0i64;
        let mut paginas = 0;
        loop {
            let r = s
                .executar(
                    "varrer",
                    &pedido(&format!(
                        r#"{{"database":"b","tabela":"c","max":7,"depois":{cursor}}}"#
                    )),
                    &sessao,
                )
                .unwrap();
            assert_eq!(r.texto_ou("modo", ""), "cursor");
            let linhas = r.campo("linhas").and_then(Json::lista).unwrap().clone();
            if linhas.is_empty() {
                assert!(
                    !matches!(r.campo("ha_mais"), Some(Json::Bool(true))),
                    "disse que ha mais e devolveu vazio"
                );
                break;
            }
            paginas += 1;
            assert!(paginas < 20, "nao terminou -- o cursor nao anda");
            for l in &linhas {
                vistos.push(l.inteiro_ou("id", -1));
            }
            cursor = r.inteiro_ou("cursor_fim", 0);
        }

        let esperado: Vec<i64> = (1..=25).filter(|i| *i != 7 && *i != 13).collect();
        assert_eq!(vistos, esperado, "o cursor pulou ou repetiu linha");
        assert_eq!(paginas, 4, "23 linhas em paginas de 7 dao 4 paginas");
    }

    /// `registros` sai do cabecalho e nao de varredura: e o numero que a tela
    /// mostra sem pagar por ele.
    #[test]
    fn varrer_nao_conta_a_tabela_para_responder() {
        let dir = dir_temp("sem-contar");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();
        let r = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c","max":2}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("devolvidas", -1), 2);
        assert_eq!(r.inteiro_ou("registros", -1), 3);
        assert!(matches!(r.campo("ha_mais"), Some(Json::Bool(true))));
        assert!(matches!(r.campo("ha_antes"), Some(Json::Bool(false))));

        // E a pagina de tras devolve o que veio antes, em ordem crescente.
        let fim = r.inteiro_ou("cursor_fim", 0);
        let atras = s
            .executar(
                "varrer",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","max":5,"antes":{fim}}}"#
                )),
                &sessao,
            )
            .unwrap();
        let ids: Vec<i64> = atras
            .campo("linhas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|l| l.inteiro_ou("id", -1))
            .collect();
        assert_eq!(ids, vec![1]);
    }

    /// O `rownum` chega na resposta e cresce com a ordem de digitacao.
    #[test]
    fn a_resposta_traz_o_numero_de_ordem() {
        let dir = dir_temp("rownum");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();
        let r = s
            .executar("varrer", &pedido(r#"{"database":"b","tabela":"c"}"#), &sessao)
            .unwrap();
        let nums: Vec<i64> = r
            .campo("linhas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|l| l.inteiro_ou("rownum", -1))
            .collect();
        assert_eq!(nums, vec![1, 2, 3]);
    }

    /// E o portao de verdade, com um usuario que tem tudo menos administrar.'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

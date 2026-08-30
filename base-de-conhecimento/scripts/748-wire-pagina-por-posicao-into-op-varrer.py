# Wire pagina_por_posicao into op_varrer
# 28/08 19:45

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

antigo = """        // Tres modos, e o padrao mudou de lado.
        //
        // `depois` / `antes` sao o CURSOR: a pagina custa o tamanho dela, e
        // nao o tamanho da tabela. `pular` continua existindo para tela
        // pequena e para quem ja escreveu cliente com ele, mas e o modo de
        // compatibilidade -- ele anda ate a posicao, e andar ate a posicao um
        // milhao custa um milhao de passos.
        let depois = p.inteiro_ou("depois", -1);
        let antes = p.inteiro_ou("antes", -1);
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
"""
novo = """        // Quatro modos.
        //
        // `depois` / `antes` sao o CURSOR: a pagina custa o tamanho dela, e
        // nao o tamanho da tabela. `desde_rownum` e o cursor de quem guardou o
        // numero de ordem em vez do rowid. E `pular` e a POSICAO -- o `OFFSET`
        // do SQL, que deixou de andar ate la sempre: quando a posicao e o
        // rownum, ele bisseta. A resposta diz qual dos dois pagou.
        let depois = p.inteiro_ou("depois", -1);
        let antes = p.inteiro_ou("antes", -1);
        let desde_rownum = p.inteiro_ou("desde_rownum", -1);
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let mut salto = None;
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        } else if antes >= 0 {
            (t.pagina_antes_de(antes as u64, max, visao)?, "cursor")
        } else if depois >= 0 {
            (t.pagina_depois_de(depois as u64, max, visao)?, "cursor")
        } else {
            (t.pagina(pular, max, visao)?, "posicao")
        };
"""
novo = """        } else if antes >= 0 {
            (t.pagina_antes_de(antes as u64, max, visao)?, "cursor")
        } else if depois >= 0 {
            (t.pagina_depois_de(depois as u64, max, visao)?, "cursor")
        } else if desde_rownum >= 0 {
            (
                t.pagina_desde_rownum(desde_rownum as u64, max, visao)?,
                "rownum",
            )
        } else {
            let (rowids, como) = t.pagina_por_posicao(pular, max, visao)?;
            salto = Some(como);
            (rowids, "posicao")
        };
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """            ("registros", Json::de_u64(t.registros())),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            ("modo", Json::texto_de(modo)),
            ("cursor_inicio", Json::de_u64(primeiro)),
            ("cursor_fim", Json::de_u64(ultimo)),
            ("ha_mais", Json::Bool(ha_mais)),
            ("ha_antes", Json::Bool(ha_antes)),"""
novo = """            ("registros", Json::de_u64(t.registros())),
            // Quantas linhas ESTA visao enxerga -- e a conta de «pagina 3 de
            // 40». Sai de dois contadores do cabecalho, sem varrer nada; era
            // por nao existir que a contagem tinha sido tirada da resposta.
            ("visiveis", Json::de_u64(t.contar(visao))),
            ("marcadas", Json::de_u64(t.marcadas())),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            ("modo", Json::texto_de(modo)),
            // Como o inicio da pagina foi achado, quando o modo e por posicao.
            // «bisseccao» sao ~20 leituras; «passo» sao `pular` leituras.
            (
                "salto",
                match salto {
                    Some(s) => Json::texto_de(s.nome()),
                    None => Json::Nulo,
                },
            ),
            ("cursor_inicio", Json::de_u64(primeiro)),
            ("cursor_fim", Json::de_u64(ultimo)),
            // O numero de ordem da primeira e da ultima linha da pagina: e o
            // cursor de quem pagina por `desde_rownum`, e o que a caixa «ir
            // para a linha N» devolve para a tela se localizar.
            ("rownum_inicio", Json::de_u64(rownum_inicio)),
            ("rownum_fim", Json::de_u64(rownum_fim)),
            ("ha_mais", Json::Bool(ha_mais)),
            ("ha_antes", Json::Bool(ha_antes)),"""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("ok")

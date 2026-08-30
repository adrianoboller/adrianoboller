# Rewrite op_varrer with cursor pagination
# 28/08 18:32

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho='''        let rowids: Vec<u64> = if indice.is_empty() {
            t.varrer_com(visao)?.into_iter().map(|(r, _)| r).collect()
        } else {
            // O indice devolve rowid, e a marca esta no registro: pela ordem
            // do indice a filtragem custa uma leitura por linha. E o preco de
            // pedir ordenado -- e por isso `Todas` nao paga nada.
            let todos = t.varrer_indice(&indice)?;
            t.filtrar(&todos, visao)?
        };
        let total = rowids.len();
        let mut linhas = Vec::new();
        for rowid in rowids.into_iter().take(max as usize) {
            if let Some(l) = t.ler(rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            (
                "ordem",
                Json::texto_de(if indice.is_empty() {
                    "digitacao".to_string()
                } else {
                    format!("indice {indice}")
                }),
            ),
            ("linhas", Json::Lista(linhas)),
        ]))
    }'''

novo='''        // Tres modos, e o padrao mudou de lado.
        //
        // `depois` / `antes` sao o CURSOR: a pagina custa o tamanho dela, e
        // nao o tamanho da tabela. `pular` continua existindo para tela
        // pequena e para quem ja escreveu cliente com ele, mas e o modo de
        // compatibilidade -- ele anda ate a posicao, e andar ate a posicao um
        // milhao custa um milhao de passos.
        let depois = p.inteiro_ou("depois", -1);
        let antes = p.inteiro_ou("antes", -1);
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;

        let por_indice = !indice.is_empty();
        let (rowids, modo) = if por_indice {
            // O indice devolve rowid na ordem da CHAVE, e nao na do arquivo:
            // continuar "depois do rowid X" nao quer dizer nada aqui, porque
            // o proximo da chave pode ter rowid menor. Entao por indice vale a
            // posicao, e a resposta diz isso em vez de fingir que paginou.
            let todos = t.varrer_indice(&indice)?;
            let vivos = t.filtrar(&todos, visao)?;
            let corte: Vec<u64> = vivos
                .into_iter()
                .skip(pular as usize)
                .take(max as usize)
                .collect();
            (corte, "posicao")
        } else if antes >= 0 {
            (t.pagina_antes_de(antes as u64, max, visao)?, "cursor")
        } else if depois >= 0 {
            (t.pagina_depois_de(depois as u64, max, visao)?, "cursor")
        } else {
            (t.pagina(pular, max, visao)?, "posicao")
        };

        let mut linhas = Vec::with_capacity(rowids.len());
        for &rowid in &rowids {
            if let Some(l) = t.ler(rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }

        // O cursor para pedir a proxima pagina e a anterior. Vai pronto na
        // resposta para o cliente nao ter de saber que ele e um rowid -- e
        // para poder deixar de ser um, se um dia a ordem mudar.
        let primeiro = rowids.first().copied().unwrap_or(0);
        let ultimo = rowids.last().copied().unwrap_or(0);
        // "Tem mais" sem contar a tabela: pede UM alem do teto. Uma leitura a
        // mais por pagina, contra uma varredura inteira so para mostrar
        // "pagina 3 de 40" -- que numa tabela grande e o item mais caro da
        // tela e o que ninguem le.
        let ha_mais = ultimo > 0 && !t.pagina_depois_de(ultimo, 1, visao)?.is_empty();
        let ha_antes = primeiro > 1 && !t.pagina_antes_de(primeiro, 1, visao)?.is_empty();

        Ok(Json::objeto(vec![
            // `registros` e o que a tabela tem, e sai do cabecalho: nao custa
            // varredura. `total` era a contagem da varredura inteira, e por
            // isso deixou de existir aqui.
            ("registros", Json::de_u64(t.registros())),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            ("modo", Json::texto_de(modo)),
            ("cursor_inicio", Json::de_u64(primeiro)),
            ("cursor_fim", Json::de_u64(ultimo)),
            ("ha_mais", Json::Bool(ha_mais)),
            ("ha_antes", Json::Bool(ha_antes)),
            (
                "ordem",
                Json::texto_de(if por_indice {
                    format!("indice {indice}")
                } else {
                    "digitacao".to_string()
                }),
            ),
            ("linhas", Json::Lista(linhas)),
        ]))
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

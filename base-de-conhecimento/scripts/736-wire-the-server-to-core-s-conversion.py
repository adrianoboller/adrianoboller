# Wire the server to core's conversion
# 28/08 19:31

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
# o lote passa a converter direto da Carga, sem passar por JSON
velho='''        let (itens, formato, de_texto) = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => phxsql_core::carga::adivinhar(texto),
                    outro => phxsql_core::carga::Formato::de_texto(outro)?,
                };
                let carga = phxsql_core::carga::ler(texto, f)?;
                // Carga COLADA e sempre convertida pelo esquema, nos cinco
                // formatos. Uma regra so, e nao duas: o leitor entrega texto,
                // e quem sabe que aquilo e um inteiro e a coluna. Vale para o
                // JSON tambem -- `{"id":1}` chega aqui como `"1"` e volta a
                // ser inteiro pelo tipo declarado.
                (carga.para_json(), f.nome().to_string(), true)
            }
            None => (
                p.campo("linhas")
                    .or_else(|| p.campo("valores"))
                    .cloned()
                    .ok_or_else(|| {
                        PhxError::Esquema(
                            "informe \\"linhas\\" com a lista, ou \\"texto\\" com a carga colada"
                                .into(),
                        )
                    })?,
                "lista".to_string(),
                false,
            ),
        };
        let itens = itens.lista().map(|l| l.to_vec()).ok_or_else(|| {
            PhxError::Esquema("\\"linhas\\" precisa ser uma lista de objetos".into())
        })?;
        if itens.is_empty() {
            return Err(PhxError::Esquema("a carga nao tem nenhuma linha".into()));
        }'''
novo='''        // Duas origens: uma carga COLADA (texto num dos cinco formatos) ou uma
        // lista de objetos JSON ja tipada. A colada e lida antes de a trava
        // ser tomada -- analisar um CSV malformado com a trava de dados na mao
        // seguraria todo mundo.
        let colada = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => phxsql_core::carga::adivinhar(texto),
                    outro => phxsql_core::carga::Formato::de_texto(outro)?,
                };
                Some((phxsql_core::carga::ler(texto, f)?, f.nome().to_string()))
            }
            None => None,
        };
        let itens: Vec<Json> = match &colada {
            Some(_) => Vec::new(),
            None => p
                .campo("linhas")
                .or_else(|| p.campo("valores"))
                .and_then(Json::lista)
                .map(|l| l.to_vec())
                .ok_or_else(|| {
                    PhxError::Esquema(
                        "informe \\"linhas\\" com a lista, ou \\"texto\\" com a carga colada".into(),
                    )
                })?,
        };
        let formato = match &colada {
            Some((_, f)) => f.clone(),
            None => "lista".to_string(),
        };
        let recebidas = match &colada {
            Some((c, _)) => c.linhas.len(),
            None => itens.len(),
        };
        if recebidas == 0 {
            return Err(PhxError::Esquema("a carga nao tem nenhuma linha".into()));
        }'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        let mut linhas = Vec::with_capacity(itens.len());
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        for (i, item) in itens.iter().enumerate() {
            let convertida = if de_texto {
                crate::valores::json_para_linha_de_texto(item, t.esquema())
            } else {
                json_para_linha(item, t.esquema())
            };
            match convertida {
                Ok(l) => linhas.push(l),
                Err(e) => {
                    recusadas.push((i, e.to_string()));
                    if parar {
                        return Ok(Self::resposta_do_lote(
                            p,
                            &formato,
                            itens.len(),
                            &[],
                            &recusadas,
                            inicio,
                        ));
                    }
                }
            }
        }'''
novo2='''        // A conversao acontece aqui, com o esquema na mao. Uma linha que nao
        // converte entra na lista de recusadas em vez de derrubar a carga
        // inteira -- a menos que `parar_no_erro` mande parar.
        let mut linhas: Vec<Vec<phxsql_core::value::Value>> = Vec::with_capacity(recebidas);
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        for i in 0..recebidas {
            let convertida = match &colada {
                Some((c, _)) => phxsql_core::carga::linha_de_texto(c, i, t.esquema()),
                None => json_para_linha(&itens[i], t.esquema()),
            };
            match convertida {
                Ok(l) => linhas.push(l),
                Err(e) => {
                    recusadas.push((i, e.to_string()));
                    if parar {
                        return Ok(Self::resposta_do_lote(
                            p, &formato, recebidas, &[], &recusadas, inicio,
                        ));
                    }
                }
            }
        }'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

s=s.replace('''        Ok(Self::resposta_do_lote(
            p,
            &formato,
            itens.len(),
            &lote.rowids,
            &recusadas,
            inicio,
        ))''','''        Ok(Self::resposta_do_lote(
            p,
            &formato,
            recebidas,
            &lote.rowids,
            &recusadas,
            inicio,
        ))''',1)
io.open(p,'w',encoding='utf-8').write(s)

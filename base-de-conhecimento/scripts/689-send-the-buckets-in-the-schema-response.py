# Send the buckets in the schema response
# 28/08 19:00

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''                        (
                            "modo",
                            Json::texto_de(match pag.modo.periodo() {
                                None => "quantidade".to_string(),
                                Some(per) => per.nome().to_string(),
                            }),
                        ),'''
novo='''                        (
                            "modo",
                            Json::texto_de(match pag.modo.periodo() {
                                Some(per) => per.nome().to_string(),
                                None => pag.modo.nome().to_string(),
                            }),
                        ),
                        // Os baldes da particao alfanumerica, ja com o nome do
                        // arquivo e quantas linhas tem. So a tabela sabe: a
                        // contagem mora no cabecalho de cada volume, e a tela
                        // nao tem como deduzir dela quantos slots foram usados
                        // no `_S` -- o `slots` daqui e a marca d'agua, e nao
                        // uma contagem.
                        (
                            "baldes",
                            if pag.modo.por_letra() {
                                let baldes = t.baldes();
                                let existentes = t.volumes_por_arquivo().0;
                                Json::Lista(
                                    phxsql_core::paginacao::BALDES
                                        .iter()
                                        .enumerate()
                                        .map(|(i, letra)| {
                                            let n = i as u32 + 1;
                                            Json::objeto(vec![
                                                ("volume", Json::de_u64(n as u64)),
                                                ("letra", Json::texto_de(*letra)),
                                                (
                                                    "arquivo",
                                                    Json::texto_de(format!(
                                                        "{}_{letra}.reg",
                                                        e.nome()
                                                    )),
                                                ),
                                                (
                                                    "registros",
                                                    Json::de_u64(
                                                        baldes.get(i).copied().unwrap_or(0),
                                                    ),
                                                ),
                                                (
                                                    "existe",
                                                    Json::Bool(existentes.contains(&n)),
                                                ),
                                                (
                                                    "primeiro_rowid",
                                                    Json::de_u64(
                                                        (n as u64 - 1)
                                                            * pag.registros_por_arquivo
                                                            + 1,
                                                    ),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                )
                            } else {
                                Json::Nulo
                            },
                        ),'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

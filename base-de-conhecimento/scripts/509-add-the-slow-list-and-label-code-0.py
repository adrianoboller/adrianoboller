# Add the slow list and label code 0
# 28/08 16:34

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''        duracoes.sort_unstable();'''
b='''        duracoes.sort_unstable();

        // As mais demoradas, com nome e objeto. E o registro de consulta lenta
        // do MySQL(R), so que sem precisar ligar nada: o log ja tinha o dado, e
        // faltava a pergunta.
        let mut mais_lentas: Vec<&Acesso> = considerar.clone();
        mais_lentas.sort_by(|a, b| b.duracao_ms.cmp(&a.duracao_ms));
        mais_lentas.truncate(15);'''
assert a in s; s=s.replace(a,b,1)
a='''            (
                "por_erro",'''
b='''            (
                "mais_lentas",
                Json::Lista(
                    mais_lentas
                        .iter()
                        .map(|a| {
                            Json::objeto(vec![
                                ("quando", Json::texto_de(a.quando())),
                                ("op", Json::texto_de(&a.op)),
                                ("ms", Json::de_u64(a.duracao_ms)),
                                ("usuario", Json::texto_de(&a.usuario)),
                                (
                                    "objeto",
                                    match (a.database.is_empty(), a.tabela.is_empty()) {
                                        (true, true) => Json::Nulo,
                                        (false, true) => Json::texto_de(&a.database),
                                        (true, false) => Json::texto_de(&a.tabela),
                                        _ => Json::texto_de(format!("{}.{}", a.database, a.tabela)),
                                    },
                                ),
                                ("ok", Json::Bool(a.ok)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_erro",'''
assert a in s; s=s.replace(a,b,1)
a='''                                ("codigo", Json::de_u64(*codigo as u64)),
                                ("quantas", Json::de_u64(*n)),
                                ("exemplo", Json::texto_de(exemplo)),'''
b='''                                ("codigo", Json::de_u64(*codigo as u64)),
                                // Zero nao e um erro: e uma linha gravada
                                // antes de o codigo existir. Chama-lo de
                                // "codigo 0" faria parecer um erro novo.
                                (
                                    "nome",
                                    Json::texto_de(match codigo {
                                        0 => "(log anterior ao codigo)",
                                        1001 => "CORROMPIDO",
                                        1002 => "ASSINATURA_INVALIDA",
                                        1003 => "VERSAO_NAO_SUPORTADA",
                                        2001 => "ESQUEMA_INVALIDO",
                                        2002 => "TIPO_INVALIDO",
                                        3001 => "NAO_ENCONTRADO",
                                        3002 => "DUPLICADO",
                                        3003 => "LIMITE_EXCEDIDO",
                                        4001 => "ACESSO_NEGADO",
                                        5001 => "ERRO_DE_ES",
                                        _ => "?",
                                    }),
                                ),
                                ("quantas", Json::de_u64(*n)),
                                ("exemplo", Json::texto_de(exemplo)),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

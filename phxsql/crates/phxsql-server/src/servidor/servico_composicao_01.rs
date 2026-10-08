//! As quatro operacoes de composicao: `op_pivotar`, `op_juntar`,
//! `op_diferencas` e `op_unir`, com os iteradores delas.
//!
//! **A conferencia propria de cada uma NAO e duplicacao do portao.** O
//! `CLAUDE.md` ("Portao de permissao e UM so -- e o campo que ele le e o furo")
//! explica: as quatro escondem tabela do campo `"tabela"` que o portao le --
//! `juntar` em `a.tabela`/`b.tabela`, `unir` numa lista, `pivotar` nas de
//! consulta dentro de um `juntar` aninhado. Cada `pode_em(..., Atividade::Ler)`
//! daqui responde a uma pergunta que o portao nao faz. Limpa-la reabre a porta
//! dos fundos, e NENHUM teste do portao acusa: so as guardas `*-sem-portao`.
//! Elas moram juntas, neste arquivo, para que ninguem as confunda com copia; e a
//! catraca `TETO`/`PISO` de `pode_em(` com `Atividade::Ler` conta as daqui.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    /// Monta a tabulacao cruzada de uma tabela, com junção opcional.
    ///
    /// ```json
    /// { "database": "loja", "tabela": "vendas",
    ///   "juntar": [ {"tabela":"clientes", "coluna":"cliente_id", "prefixo":"cliente"} ],
    ///   "linhas": [ {"campo":"cliente.cidade"} ],
    ///   "colunas": [ {"campo":"emissao", "granularidade":"mes"} ],
    ///   "valor": "total", "agregador": "soma", "max": 200000 }
    /// ```
    pub(super) fn op_pivotar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let agregador = Agregador::de_texto(p.texto_ou("agregador", "soma"))?;
        let max = self.limite_pivot(p);

        // O portao geral confere o campo `tabela` do pedido, e a tabela de
        // FATOS do pivot esta la -- mas as tabelas de CONSULTA nao: cada uma
        // mora num `tabela` DENTRO de um item de `juntar`, e o portao nao
        // desce ate ali. Sem esta conferencia, o pivot era a porta dos fundos
        // do `juntar` e do `unir` por um terceiro caminho: bastava juntar a
        // tabela negada e pedir um campo dela em `linhas` -- os rotulos das
        // linhas do cruzamento SAO os valores dela, e a resposta ainda diz o
        // nome da tabela e quantas linhas ela tem.
        //
        // A licao que isto repete: quando o portao passar a olhar um campo
        // novo, procure quem NAO tem esse campo -- inclusive quem o tem
        // aninhado, que e o disfarce mais facil de nao ver.
        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            for j in p.campo("juntar").and_then(Json::lista).unwrap_or(&[]) {
                let alvo = j.texto_ou("tabela", "");
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;
        let mut t = db.abrir_qualificada(p.texto_ou("tabela", ""))?;
        let esquema = t.esquema().clone();

        // As tabelas de consulta entram inteiras na memoria, uma vez. E o hash
        // join: para a forma de dado de um pivot -- muitos fatos, poucas
        // dimensoes -- ele custa uma varredura em vez de uma descida na arvore
        // por linha de fato.
        let mut juncoes: Vec<Juncao> = Vec::new();
        for (i, j) in p
            .campo("juntar")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let nome = j.texto_ou("tabela", "");
            let local = j.texto_ou("coluna", "");
            let prefixo = match j.texto_ou("prefixo", "").trim() {
                "" => nome.rsplit('.').next().unwrap_or(nome).to_string(),
                outro => outro.to_string(),
            };
            let coluna_local = posicao_da_coluna(&esquema, local).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a junção {i} usa a coluna {local:?}, que nao existe em {}",
                    esquema.nome()
                ))
            })?;
            // A chave da junção e a PRIMEIRA coluna da chave primaria da tabela
            // de consulta, ou a coluna nomeada em "chave". Sem chave primaria
            // nao ha por onde ligar, e dizer isso e melhor do que juntar pela
            // primeira coluna e devolver numero errado.
            let mut alvo = db.abrir_qualificada(nome)?;
            let esq_alvo = alvo.esquema().clone();
            let chave = match j.texto_ou("chave", "").trim() {
                "" => esq_alvo
                    .chave_primaria()
                    .and_then(|k| k.colunas.first())
                    .map(|ic| ic.coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a tabela {nome} nao tem chave primaria; diga por qual \
                             coluna juntar no campo \"chave\""
                        ))
                    })?,
                c => posicao_da_coluna(&esq_alvo, c).ok_or_else(|| {
                    PhxError::Esquema(format!("a coluna {c:?} nao existe em {nome}"))
                })?,
            };

            let mut mapa = HashMap::new();
            let rowids = alvo.varrer()?;
            let lidas = rowids.len();
            for (rowid, _) in rowids.into_iter().take(TETO_JUNCAO) {
                if let Some(linha) = alvo.ler(rowid)? {
                    // A chave e a canonica do `juncao`, com o tipo da coluna
                    // de LA -- e `pivot::valor_bruto` procura com o tipo da
                    // coluna de CA. Ver o cabecalho dela para os dois
                    // casamentos errados que as duas formas de antes davam.
                    mapa.insert(
                        crate::juncao::pedaco_de_chave(
                            &linha[chave],
                            &esq_alvo.colunas()[chave].ty,
                        ),
                        linha,
                    );
                }
            }
            if lidas > TETO_JUNCAO {
                return Err(PhxError::LimiteExcedido(format!(
                    "a tabela de consulta {nome} tem {lidas} linhas, acima do teto \
                     de {TETO_JUNCAO} para junção. Ela e lida inteira para a memoria; \
                     junte por uma tabela menor"
                )));
            }
            juncoes.push(Juncao {
                prefixo,
                esquema: esq_alvo,
                coluna_local,
                mapa,
                lidas,
            });
        }

        let campos = |chave: &str| -> Result<Vec<Campo>> {
            let mut out = Vec::new();
            for c in p.campo(chave).and_then(Json::lista).unwrap_or(&[]) {
                // Aceita tanto "cidade" quanto {"campo":"cidade","granularidade":"mes"}.
                let (nome, gran) = match c {
                    Json::Texto(t) => (t.as_str(), "exato"),
                    outro => (
                        outro.texto_ou("campo", ""),
                        outro.texto_ou("granularidade", "exato"),
                    ),
                };
                out.push(resolver_campo(nome, &esquema, &juncoes, gran)?);
            }
            Ok(out)
        };
        let linhas = campos("linhas")?;
        let colunas = campos("colunas")?;
        if linhas.is_empty() {
            return Err(PhxError::Esquema(
                "informe ao menos um campo em \"linhas\"".into(),
            ));
        }

        let nome_valor = p.texto_ou("valor", "").trim().to_string();
        let valor = if nome_valor.is_empty() {
            if agregador.precisa_de_valor() {
                return Err(PhxError::Esquema(format!(
                    "o agregador {} precisa de um campo em \"valor\"",
                    agregador.nome()
                )));
            }
            None
        } else {
            Some(resolver_campo(&nome_valor, &esquema, &juncoes, "exato")?)
        };

        let mut it = LinhasDaTabela {
            rowids: t
                .varrer()?
                .into_iter()
                .map(|(r, _)| r)
                .collect::<Vec<_>>()
                .into_iter(),
            tabela: &mut t,
        };
        let r = crate::pivot::cruzar(
            &mut it,
            &esquema,
            &juncoes,
            &linhas,
            &colunas,
            valor.as_ref(),
            agregador,
            max,
        )?;

        let txt = |o: &Option<String>| match o {
            None => Json::Nulo,
            Some(s) => Json::texto_de(s),
        };
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("agregador", Json::texto_de(agregador.nome())),
            (
                "campos_linha",
                Json::Lista(
                    linhas
                        .iter()
                        .map(|c| Json::texto_de(&c.qualificado))
                        .collect(),
                ),
            ),
            (
                "campos_coluna",
                Json::Lista(
                    colunas
                        .iter()
                        .map(|c| Json::texto_de(&c.qualificado))
                        .collect(),
                ),
            ),
            ("valor", Json::texto_de(&nome_valor)),
            (
                "rotulos_linha",
                Json::Lista(r.rotulos_linha.iter().map(Json::texto_de).collect()),
            ),
            (
                "rotulos_coluna",
                Json::Lista(r.rotulos_coluna.iter().map(Json::texto_de).collect()),
            ),
            (
                "celulas",
                Json::Lista(
                    r.celulas
                        .iter()
                        .map(|l| Json::Lista(l.iter().map(&txt).collect()))
                        .collect(),
                ),
            ),
            (
                "total_linha",
                Json::Lista(r.total_linha.iter().map(&txt).collect()),
            ),
            (
                "total_coluna",
                Json::Lista(r.total_coluna.iter().map(&txt).collect()),
            ),
            ("total", txt(&r.total)),
            ("lidas", Json::de_u64(r.lidas)),
            ("consideradas", Json::de_u64(r.consideradas)),
            (
                "juncoes",
                Json::Lista(
                    juncoes
                        .iter()
                        .map(|j| {
                            Json::objeto(vec![
                                ("prefixo", Json::texto_de(&j.prefixo)),
                                ("tabela", Json::texto_de(j.esquema.nome())),
                                ("linhas", Json::de_u64(j.lidas as u64)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// O teto de linhas que o pivot varre. Separado do `max_linhas` porque o
    /// pivot devolve um RESUMO: ler cem mil linhas para devolver uma grade de
    /// vinte por doze e barato, e o teto da resposta nao se aplica.
    fn limite_pivot(&self, p: &Json) -> u64 {
        let pedido = p.inteiro_ou("max", TETO_PIVOT as i64).max(1) as u64;
        pedido.min(TETO_PIVOT)
    }

    // -------------------------------------------------- junção e união

    /// Resolve as colunas da chave de um lado, aceitando nome ou lista.
    fn chave_do_lado(esquema: &Schema, j: &Json, campo: &str, tabela: &str) -> Result<Vec<usize>> {
        let nomes: Vec<String> = match j.campo(campo) {
            Some(Json::Lista(l)) => l
                .iter()
                .filter_map(|x| x.texto().map(str::to_string))
                .collect(),
            Some(outro) => outro
                .texto()
                .map(|t| vec![t.to_string()])
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if nomes.is_empty() {
            // Sem chave dita, a chave primaria e a escolha obvia -- e a unica
            // que nao e chute. Juntar pela primeira coluna daria numero errado
            // calado, que e pior do que recusar.
            let pk = esquema.chave_primaria().ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{tabela} nao tem chave primaria; diga por qual coluna juntar em {campo:?}"
                ))
            })?;
            return Ok(pk.colunas.iter().map(|ic| ic.coluna).collect());
        }
        nomes
            .iter()
            .map(|n| {
                posicao_da_coluna(esquema, n).ok_or_else(|| {
                    PhxError::Esquema(format!("a coluna {n:?} nao existe em {tabela}"))
                })
            })
            .collect()
    }

    /// Le uma tabela inteira para a memoria, com teto.
    ///
    /// O lado do mapa cabe na memoria ou a junção nao acontece. Recusar com o
    /// numero na mensagem e melhor do que engasgar a maquina.
    fn materializar(t: &mut Table, nome: &str) -> Result<Vec<Vec<Value>>> {
        let rowids = t.varrer()?;
        if rowids.len() > TETO_JUNCAO {
            return Err(PhxError::LimiteExcedido(format!(
                "{nome} tem {} linhas, acima do teto de {TETO_JUNCAO} para o lado que \
                 entra na memoria. Troque a ordem dos lados ou filtre antes",
                rowids.len()
            )));
        }
        let mut v = Vec::with_capacity(rowids.len());
        for (rowid, _) in rowids {
            if let Some(l) = t.ler(rowid)? {
                v.push(l);
            }
        }
        Ok(v)
    }

    /// As sete figuras do diagrama, entre duas tabelas.
    pub(super) fn op_juntar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let tipo = TipoJuncao::de_texto(p.texto_ou("tipo", "interna"))?;
        let max = self.limite_pivot(p);
        let comeco = Instant::now();

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;

        let (pa, pb) = (
            p.campo("a")
                .ok_or_else(|| PhxError::Esquema("junção sem o lado \"a\"".into()))?,
            p.campo("b")
                .ok_or_else(|| PhxError::Esquema("junção sem o lado \"b\"".into()))?,
        );
        let (na, nb) = (pa.texto_ou("tabela", ""), pb.texto_ou("tabela", ""));
        let mut ta = db.abrir_qualificada(na)?;
        let mut tb = db.abrir_qualificada(nb)?;
        let (ea, eb) = (ta.esquema().clone(), tb.esquema().clone());

        // O portao geral confere o campo `tabela` do pedido -- e uma junção
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
        }

        let lado = |esquema: Schema, j: &Json, nome: &str| -> Result<Lado> {
            let chave = Self::chave_do_lado(&esquema, j, "chave", nome)?;
            let prefixo = match j.texto_ou("prefixo", "").trim() {
                "" => nome.rsplit('.').next().unwrap_or(nome).to_string(),
                outro => outro.to_string(),
            };
            Ok(Lado {
                prefixo,
                esquema,
                chave,
            })
        };
        let la = lado(ea, pa, na)?;
        let lb = lado(eb, pb, nb)?;
        if la.prefixo == lb.prefixo {
            return Err(PhxError::Esquema(format!(
                "os dois lados usariam o prefixo {:?}; dê um \"prefixo\" a um deles \
                 para as colunas nao se sobreporem na saida",
                la.prefixo
            )));
        }
        crate::juncao::conferir_chaves(&la, &lb)?;

        // Quem entra na memoria e quem NAO precisa sair inteiro. Num RIGHT, o
        // lado que precisa inteiro e B, entao A vira mapa e B streama.
        let r = match tipo.trocando_os_lados() {
            Some(espelho) => {
                let memoria = Self::materializar(&mut ta, na)?;
                let mut fluxo = LinhasDaTabela {
                    rowids: tb
                        .varrer()?
                        .into_iter()
                        .map(|(r, _)| r)
                        .collect::<Vec<_>>()
                        .into_iter(),
                    tabela: &mut tb,
                };
                crate::juncao::juntar(&mut fluxo, &lb, &memoria, &la, espelho, true, max)?
            }
            None => {
                let memoria = Self::materializar(&mut tb, nb)?;
                let mut fluxo = LinhasDaTabela {
                    rowids: ta
                        .varrer()?
                        .into_iter()
                        .map(|(r, _)| r)
                        .collect::<Vec<_>>()
                        .into_iter(),
                    tabela: &mut ta,
                };
                crate::juncao::juntar(&mut fluxo, &la, &memoria, &lb, tipo, false, max)?
            }
        };

        Ok(Json::objeto(vec![
            ("tipo", Json::texto_de(tipo.nome())),
            ("sql", Json::texto_de(tipo.sql())),
            ("a", Json::texto_de(na)),
            ("b", Json::texto_de(nb)),
            ("colunas", colunas_da_juncao(&r.colunas)),
            (
                "linhas",
                Json::Lista(
                    r.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .zip(r.colunas.iter())
                                    .map(|(v, c)| crate::valores::valor_para_json(v, &c.ty))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(r.linhas.len() as u64)),
            ("lidas_a", Json::de_u64(r.lidas_esquerda)),
            ("lidas_b", Json::de_u64(r.lidas_direita)),
            // Contadas e devolvidas de proposito: um INNER que trouxe menos do
            // que se esperava costuma ter aqui a explicacao, e sem o numero ela
            // vira meia hora de investigacao.
            ("chave_nula_a", Json::de_u64(r.chave_nula_esquerda)),
            ("chave_nula_b", Json::de_u64(r.chave_nula_direita)),
            ("truncado", Json::Bool(r.truncado)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// `diferencas`: o que mudou entre duas tabelas, pela chave.
    ///
    /// # O TERCEIRO IRMAO da conferencia propria
    ///
    /// O portao geral confere o campo `"tabela"` do pedido, e esta operacao
    /// **nao tem esse campo**: as duas tabelas moram em `"a"` e `"b"`. E
    /// exatamente a forma do `juntar` (que guarda em `a.tabela` e `b.tabela`)
    /// e do `unir` (que guarda numa lista) -- e por isso ela paga conferencia
    /// propria, como os dois pagam.
    ///
    /// **Isto NAO e duplicacao do portao geral**, e a distincao importa numa
    /// futura divisao deste arquivo: limpar esta conferencia por parecer
    /// repetida reabre a porta dos fundos, e nenhum teste do portao geral
    /// acusa -- o que acusa e o teste que viaja com esta operacao.
    ///
    /// A pergunta que decide, e que vale para a proxima op que nascer: **esta
    /// operacao nomeia tabela onde o portao nao olha?**
    pub(super) fn op_diferencas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let comeco = Instant::now();
        let base = p.texto_ou("database", "").to_string();
        let (na, nb) = (
            p.texto_ou("a", "").trim().to_string(),
            p.texto_ou("b", "").trim().to_string(),
        );
        if na.is_empty() || nb.is_empty() {
            return Err(PhxError::Esquema(
                "informe as duas tabelas em \"a\" e \"b\"".into(),
            ));
        }
        // A CONFERENCIA PROPRIA -- ver a nota do cabecalho.
        if let Some(u) = &sessao.usuario {
            for alvo in [&na, &nb] {
                if !u.pode_em(&base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }
        // A sonda de travessia tambem: os dois nomes chegam por campos que o
        // `despachar` nao olha, entao a sonda dele nao os viu.
        for (rotulo, valor) in [("a", &na), ("b", &nb)] {
            if phxsql_store::catalogo::nome_hostil(valor) {
                return Err(PhxError::Autorizacao(format!(
                    "{rotulo} {valor:?} nao e um nome"
                )));
            }
        }

        let max = self.limite(p) as usize;
        let teto = self.max_linhas();
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(&base)?;
        let mut ta = db.abrir_qualificada(&na)?;
        let mut tb = db.abrir_qualificada(&nb)?;
        let (ea, eb) = (ta.esquema().clone(), tb.esquema().clone());

        // As COLUNAS tem de ser as mesmas, e a recusa nomeia a diferenca.
        //
        // Comparar so o que ha nos dois lados responderia «iguais» sobre
        // linhas que diferem numa coluna que um dos lados nao tem -- e
        // «iguais» errado e a pior resposta que esta operacao pode dar,
        // porque ela existe justamente para ser acreditada.
        let nomes =
            |e: &Schema| -> Vec<String> { e.colunas().iter().map(|c| c.nome.clone()).collect() };
        let (ca, cb) = (nomes(&ea), nomes(&eb));
        if ca != cb {
            let so_de_um = |x: &[String], y: &[String]| -> Vec<String> {
                x.iter().filter(|n| !y.contains(n)).cloned().collect()
            };
            return Err(PhxError::Esquema(format!(
                "{na} e {nb} nao tem as mesmas colunas, e comparar so as comuns \
                 responderia \"iguais\" sobre linhas que diferem. So em {na}: {:?}; \
                 so em {nb}: {:?}; ordem em {na}: {ca:?}",
                so_de_um(&ca, &cb),
                so_de_um(&cb, &ca)
            )));
        }

        // O INDICE, nos dois lados, com o mesmo nome e UNICO -- ver o
        // cabecalho de `crate::diferencas` para o porque dos tres requisitos.
        let indice = match p.texto_ou("indice", "").trim() {
            "" => ea.chave_primaria().map(|k| k.nome.clone()).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{na} nao tem chave primaria; diga em \"indice\" por qual \
                         indice unico as duas tabelas se comparam"
                ))
            })?,
            outro => outro.to_string(),
        };
        for (nome, e) in [(&na, &ea), (&nb, &eb)] {
            let def = e
                .indices()
                .iter()
                .find(|i| i.nome.eq_ignore_ascii_case(&indice))
                .ok_or_else(|| {
                    PhxError::NaoEncontrado(format!("o indice {indice:?} nao existe em {nome}"))
                })?;
            if !def.unico {
                return Err(PhxError::Esquema(format!(
                    "o indice {indice:?} de {nome} nao e unico, e sem chave unica \
                     nao ha par: duas linhas com a mesma chave de um lado nao tem \
                     par unico do outro"
                )));
            }
        }

        let ler = |t: &mut Table, nome: &str| -> Result<Vec<(Vec<Value>, Vec<Value>)>> {
            let rowids = t.varrer()?;
            if rowids.len() as u64 > teto {
                return Err(PhxError::LimiteExcedido(format!(
                    "{nome} tem {} linhas, acima do teto de {teto} de \
                     `max_linhas`: as duas tabelas entram inteiras na \
                     memoria para se comparar",
                    rowids.len()
                )));
            }
            let mut saida = Vec::with_capacity(rowids.len());
            for (rowid, _) in rowids {
                if let Some(l) = t.ler(rowid)? {
                    let chave = crate::upsert::valores_do_indice(t.esquema(), &indice, &l);
                    saida.push((chave, l));
                }
            }
            Ok(saida)
        };
        let la = ler(&mut ta, &na)?;
        let lb = ler(&mut tb, &nb)?;

        // O tipo de cada coluna da CHAVE, de cada lado. Os dois indices tem o
        // mesmo nome e as mesmas colunas, mas NAO obrigatoriamente o mesmo
        // tipo: a operação confere os nomes das colunas, nunca os tipos.
        let tipos_da_chave = |e: &Schema| -> Vec<phxsql_core::types::ColumnType> {
            e.indices()
                .iter()
                .find(|x| x.nome.eq_ignore_ascii_case(&indice))
                .map(|x| {
                    x.colunas
                        .iter()
                        .filter_map(|ic| e.colunas().get(ic.coluna).map(|c| c.ty))
                        .collect()
                })
                .unwrap_or_default()
        };
        let (tipos_a, tipos_b) = (tipos_da_chave(&ea), tipos_da_chave(&eb));
        let r = crate::diferencas::comparar(
            crate::diferencas::LadoDaComparacao {
                linhas: la,
                esquema: &ea,
                tipos_da_chave: tipos_a.clone(),
            },
            crate::diferencas::LadoDaComparacao {
                linhas: lb,
                esquema: &eb,
                tipos_da_chave: tipos_b.clone(),
            },
            max,
        );

        // A chave sai pelo tipo do SEU lado.
        //
        // Ela saia sempre pelo de `a`, e isso e o pedido 392 dentro desta
        // operação: a chave 10,5000 de uma `Decimal(12,4)` lida pela escala 2
        // de `a` foi publicada como **1050,00** -- cem vezes, sem erro e sem
        // aviso. Medido em 23/09/2026, `so_em_b: [["1050.00"]]`.
        let chave_json = |c: &[Value], tipos: &[phxsql_core::types::ColumnType]| -> Json {
            Json::Lista(
                c.iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let ty = tipos
                            .get(i)
                            .copied()
                            .unwrap_or(phxsql_core::types::ColumnType::Int8);
                        crate::valores::valor_para_json(v, &ty)
                    })
                    .collect(),
            )
        };
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("a", Json::texto_de(&na)),
            ("b", Json::texto_de(&nb)),
            ("indice", Json::texto_de(&indice)),
            ("iguais", Json::de_u64(r.iguais)),
            (
                "so_em_a",
                Json::Lista(r.so_em_a.iter().map(|c| chave_json(c, &tipos_a)).collect()),
            ),
            (
                "so_em_b",
                Json::Lista(r.so_em_b.iter().map(|c| chave_json(c, &tipos_b)).collect()),
            ),
            (
                "diferentes",
                Json::Lista(
                    r.diferentes
                        .iter()
                        .map(|d| {
                            Json::objeto(vec![
                                ("chave", chave_json(&d.chave, &tipos_a)),
                                (
                                    "colunas",
                                    Json::Lista(d.colunas.iter().map(Json::texto_de).collect()),
                                ),
                                ("a", linha_para_json(&d.a, &ea)),
                                ("b", linha_para_json(&d.b, &eb)),
                            ])
                        })
                        .collect(),
                ),
            ),
            // Cortou? A resposta DIZ. Uma lista truncada em silencio faria
            // quem confere acreditar que viu tudo.
            ("truncado", Json::Bool(r.truncado)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// `UNION` e `UNION ALL` entre duas ou mais partes.
    ///
    /// # Dois caminhos, e o velho NAO sai
    ///
    /// `"tabelas": ["a","b"]` -- nomes de tabela, cada uma entrando INTEIRA.
    /// E o que existe desde sempre e continua valendo igual: guarda nova entra
    /// PEDIDA, e ha clientes de `tabelas` no repositorio (a tela, o tradutor
    /// de SQL, o catalogo e duas bancadas).
    ///
    /// `"partes": [{pedido}, {pedido}]` -- cada braco e um PEDIDO que devolve
    /// linhas (`varrer`, `buscar`, `agrupar`, `group_by`, `consultar`), o
    /// MESMO contrato que o `consultar` ja compoe em cinco lugares. Nao e
    /// gramatica nova: e a sexta porta da que existe.
    ///
    /// Por que o braco deixou de ser um nome: `SELECT nome FROM a WHERE
    /// uf='SC' UNION SELECT nome FROM b` nao tinha substrato nenhum, e o que
    /// isso custa esta REMEDIDO contra o braco de verdade em 23/09/2026 pela
    /// `bancada/uniao/medir.py` (tres corridas de sete repeticoes, 2x20.000
    /// linhas, `uf='SC'` a 1%, 400 linhas na resposta, carga da maquina
    /// 0,98): unir inteiras e filtrar fora materializa 40.000 linhas e custa
    /// 171,4 ms (160,6-185,6); filtrar DENTRO de cada braco materializa 400 e
    /// custa 2,7 ms (2,5-3,4). **64,2x, com as faixas sem se cruzarem**
    /// (62,5x · 64,2x · 67,6x nas tres corridas). O ganho e do
    /// INDICE que o braco pode usar e que unir-inteiras estruturalmente nao
    /// pode; ele e funcao da seletividade, e a 100% seria ~1x -- o numero nao
    /// e «o unir e 64x lento», e «o unir obrigava a pagar a tabela inteira
    /// quando se quer 1% dela».
    ///
    /// **O 118,7x que o pedido 393 anunciava NAO se reproduziu, e o motivo e
    /// da casa:** naquela medicao o lado filtrado eram dois `buscar` SOLTOS,
    /// porque o braco ainda nao existia para ser medido. O braco de verdade
    /// paga a maquina da uniao por cima da busca -- empilhar, canonizar a
    /// chave do `distinta`, trazer a linha JSON de volta para `Value` por
    /// nome, o guarda de profundidade. «Bancada compara trabalho igual, nao
    /// so pergunta igual» cobrando o proprio pedido: o numero velho comparava
    /// uma SIMULACAO com a operacao.
    ///
    /// E o teto muda de alvo junto: o `TETO_JUNCAO` do caminho `tabelas` se
    /// aplica a TABELA, entao unir duas de 2 milhoes de linhas e impossivel
    /// mesmo para pegar dez delas; no caminho `partes` o teto que vale e o
    /// `max_linhas` de cada braco, sobre o RECORTE.
    ///
    /// # O choque que NAO fica calado: a trava unica morre no caminho novo
    ///
    /// O caminho `tabelas` toma a trava EXCLUSIVA e a segura por toda a
    /// materializacao. Remedido pela mesma bancada numa JANELA FIXA de 1,5 s
    /// de pressao continua -- senao «pior espera baixa» quer dizer so «a
    /// carga acabou antes» --, com um `varrer(max=1)` numa conexao vizinha:
    /// p95 de 0,39 ms sozinho (0,38-0,40), **115,33 ms sob uniao por
    /// `tabelas`** (114,54-117,99) e **0,85 ms sob uniao por `partes`**
    /// (0,82-0,86). Sao **135,7x**, com as faixas sem se cruzarem -- e o
    /// caminho novo aguentou 567 a 603 unioes na mesma janela em que o velho
    /// coube 11. O caminho `partes` NAO toma
    /// trava nenhuma, e isso nao e
    /// preferencia: `travar_dados` erra na reentrancia pela `COM_A_TRAVA`,
    /// entao uma uniao que segurasse a trava e chamasse `linhas_do_sub_pedido`
    /// receberia `trava_reentrante()` no primeiro braco. **E o codigo que
    /// obriga.** Cada braco toma e solta a sua, por dentro do
    /// `executar_derivado`, como o `consultar` ja faz.
    ///
    /// **O que isso CUSTA, escrito e nao implicito:** os bracos deixam de vir
    /// do mesmo instantaneo -- um escritor pode entrar entre o braco A e o
    /// braco B, e a uniao pode empilhar dois retratos de instantes diferentes.
    /// Tres coisas atenuam, e nenhuma decide sozinha: (a) o `consultar` ja e
    /// assim nas juncoes, desde sempre; (b) o isolamento entregue por padrao e
    /// READ COMMITTED, onde isso e legitimo; (c) quem pede
    /// `leitura_repetivel` segura a compartilhada em cada tabela ate o fim da
    /// transacao e fica coberto. Palavra do papel C (DBA), registrada tambem
    /// em `docs/JUNCOES.md` -- quem quiser os bracos do mesmo instantaneo
    /// abre transacao com leitura repetivel, e quem nao abrir sabe o que tem.
    pub(super) fn op_unir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let modo = Uniao::de_texto(p.texto_ou("modo", "distinta"))?;
        let max = self.limite_pivot(p);
        let comeco = Instant::now();
        let base = p.texto_ou("database", "");

        // Os dois campos dizem coisas diferentes sobre o mesmo braco. Escolher
        // um calado faria o outro virar enfeite -- e campo que parece pedido e
        // o motor ignora e a mesma familia da configuracao que ninguem le.
        if p.campo("partes").is_some() {
            if p.campo("tabelas").is_some() {
                return Err(PhxError::Esquema(
                    "a uniao recebeu \"partes\" e \"tabelas\" no mesmo pedido: \
                     \"partes\" sao pedidos e \"tabelas\" sao nomes de tabela, e o \
                     motor nao escolhe por voce. Mande um dos dois"
                        .into(),
                ));
            }
            return self.unir_por_pedidos(p, base, modo, max, comeco, sessao);
        }

        let nomes: Vec<String> = p
            .campo("tabelas")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .filter_map(|x| {
                        x.texto()
                            .map(str::to_string)
                            .or_else(|| x.campo("tabela").and_then(Json::texto).map(str::to_string))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if nomes.len() < 2 {
            return Err(PhxError::Esquema(
                "a união precisa de ao menos duas tabelas em \"tabelas\"".into(),
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

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(base)?;

        // As tabelas entram inteiras porque o `UNION` distinto precisa
        // comparar cada linha com todas as anteriores -- e porque abrir varias
        // tabelas e strearmar de todas ao mesmo tempo exigiria manter as
        // referencias vivas juntas, o que o emprestimo nao deixa aqui.
        let mut materias: Vec<(Vec<Vec<Value>>, Schema)> = Vec::with_capacity(nomes.len());
        for n in &nomes {
            let mut t = db.abrir_qualificada(n)?;
            let e = t.esquema().clone();
            materias.push((Self::materializar(&mut t, n)?, e));
        }

        let mut fontes: Vec<LinhasEmMemoria> = materias
            .iter()
            .map(|(l, _)| LinhasEmMemoria(l.clone().into_iter()))
            .collect();
        // O cabecalho sai do esquema AQUI, e a uniao ja nao fala de `Schema`:
        // desde que o braco pode ser um pedido, a parte tem cabecalho sem ter
        // arquivo -- ver `juncao::Cabecalho`. Quem confere que as partes
        // empilham e o proprio `unir`, num lugar so.
        let cabecalhos: Vec<Vec<(String, ColumnType)>> = materias
            .iter()
            .map(|(_, e)| crate::juncao::cabecalho_do_esquema(e))
            .collect();

        let mut partes: Vec<(&mut dyn crate::pivot::Iterador, &crate::juncao::Cabecalho)> = fontes
            .iter_mut()
            .zip(cabecalhos.iter())
            .map(|(f, c)| (f as &mut dyn crate::pivot::Iterador, c.as_slice()))
            .collect();
        let r = crate::juncao::unir(&mut partes, modo, max)?;

        Ok(Json::objeto(vec![
            ("modo", Json::texto_de(modo.nome())),
            ("sql", Json::texto_de(modo.sql())),
            (
                "tabelas",
                Json::Lista(nomes.iter().map(Json::texto_de).collect()),
            ),
            ("colunas", colunas_da_juncao(&r.colunas)),
            (
                "linhas",
                Json::Lista(
                    r.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .zip(r.colunas.iter())
                                    .map(|(v, c)| crate::valores::valor_para_json(v, &c.ty))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(r.linhas.len() as u64)),
            (
                "por_parte",
                Json::Lista(r.por_parte.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            ("repetidas", Json::de_u64(r.repetidas)),
            ("truncado", Json::Bool(r.truncado)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// A uniao cujos bracos sao PEDIDOS -- o caminho `"partes"`.
    ///
    /// Sem trava global, de proposito e por obrigacao: ver o cabecalho do
    /// `op_unir` para o choque e para o que ele custa.
    fn unir_por_pedidos(
        &self,
        p: &Json,
        base: &str,
        modo: Uniao,
        max: u64,
        comeco: Instant,
        sessao: &Sessao,
    ) -> Result<Json> {
        // O MESMO teto de aninhamento do `consultar`, e pelo mesmo motivo: os
        // bracos descem a pilha de verdade, e pilha estourada nao e recusa --
        // e a thread caindo, que numa conexao vira resposta nenhuma.
        let _fundo = Profundidade::descer()?;
        let lista = p.campo("partes").and_then(Json::lista).ok_or_else(|| {
            PhxError::Esquema(
                "\"partes\" precisa ser uma lista de pedidos, um por braco da uniao".into(),
            )
        })?;
        if lista.len() < 2 {
            return Err(PhxError::Esquema(format!(
                "a uniao precisa de ao menos dois bracos em \"partes\", e veio {}",
                lista.len()
            )));
        }

        let mut bracos = Vec::with_capacity(lista.len());
        for (i, sub) in lista.iter().enumerate() {
            bracos.push(self.braco_da_uniao(sub, base, i + 1, sessao)?);
        }
        // Lido ANTES de as linhas sairem dos bracos: o `truncado` da uniao
        // contava so o corte DELA, e um braco que parou no teto empilhava
        // meio braco anunciando uniao inteira.
        let cortou_braco = bracos.iter().any(|b| b.cortado);

        // As linhas SAEM do braco em vez de serem clonadas: elas ja estao em
        // memoria uma vez, e uma uniao de 500.000 linhas nao paga a segunda
        // copia so para atravessar duas linhas de codigo.
        let mut fontes: Vec<LinhasEmMemoria> = bracos
            .iter_mut()
            .map(|b| LinhasEmMemoria(std::mem::take(&mut b.linhas).into_iter()))
            .collect();
        let mut partes: Vec<(&mut dyn crate::pivot::Iterador, &crate::juncao::Cabecalho)> = fontes
            .iter_mut()
            .zip(bracos.iter())
            .map(|(f, b)| (f as &mut dyn crate::pivot::Iterador, b.cabecalho.as_slice()))
            .collect();
        let r = crate::juncao::unir(&mut partes, modo, max)?;

        // O campo `tabelas` continua na resposta e continua querendo dizer «o
        // que esta uniao leu» -- so que agora ele e DERIVADO dos bracos, e nao
        // o eco do que se pediu. Quem monta `partes` ja tem a lista dele; quem
        // le a resposta quer saber onde o dado foi buscar.
        let mut tabelas: Vec<String> = Vec::new();
        for b in &bracos {
            for t in &b.tabelas {
                if !tabelas.iter().any(|x| x == t) {
                    tabelas.push(t.clone());
                }
            }
        }

        Ok(Json::objeto(vec![
            ("modo", Json::texto_de(modo.nome())),
            ("sql", Json::texto_de(modo.sql())),
            (
                "tabelas",
                Json::Lista(tabelas.iter().map(Json::texto_de).collect()),
            ),
            ("colunas", colunas_da_juncao(&r.colunas)),
            (
                "linhas",
                Json::Lista(
                    r.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .zip(r.colunas.iter())
                                    .map(|(v, c)| crate::valores::valor_para_json(v, &c.ty))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(r.linhas.len() as u64)),
            (
                "por_parte",
                Json::Lista(r.por_parte.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            ("repetidas", Json::de_u64(r.repetidas)),
            ("truncado", Json::Bool(r.truncado || cortou_braco)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// Um braco da uniao que chegou como pedido, ja materializado.
    ///
    /// Passa pelo `linhas_do_sub_pedido`, que e o mesmo lugar por onde o
    /// `consultar` compoe o `de`, o `juntar[].de`, o `escalar[].de`, o
    /// `em[].de` e o `existe[].de`. A uniao e o SEXTO uso, e por isso o portao
    /// de permissao continua sendo UM: quem nao le a folha tambem nao a le
    /// como braco de uma uniao, e a recusa vem do `executar_derivado`, nao de
    /// uma conferencia propria que alguem tenha de lembrar de atualizar.
    ///
    /// # Por que as colunas do MOTOR saem aqui, antes de qualquer linha
    ///
    /// `varrer` e `buscar` poem o `rowid` na FRENTE de cada linha, e o
    /// esquema poe as de sistema no fim. Nenhuma das duas e dado que alguem
    /// pediu para empilhar -- e deixa-las passar nao seria ruido, seria
    /// defeito: o `rowid` e UNICO por linha, e a chave do `distinta` e a linha
    /// visivel inteira. Duas linhas iguais em tabelas diferentes nunca
    /// compartilham o `rowid`, entao o `UNION` devolveria o mesmo que o
    /// `UNION ALL` anunciando `repetidas: 0`. Foi exatamente isso que o
    /// `rownum` fez ate 22/09/2026 (cinco linhas e `repetidas: 0` onde o SQL
    /// manda quatro e uma), e o braco-pedido traria o defeito de volta pela
    /// porta nova. O caminho `tabelas` tambem nao as empilha -- os dois
    /// caminhos do `unir` respondem a mesma forma.
    ///
    /// # Por NOME, e nao por posicao
    ///
    /// A linha do sub-pedido e um objeto JSON. Ler por posicao seria mais
    /// barato e faria os valores DESLIZAREM de coluna, calados, na primeira
    /// linha que nao trouxesse um campo -- o estrago que o `docs/JUNCOES.md`
    /// ja nomeia para o empilhamento por nome, aqui ao contrario. O que falta
    /// na linha vira NULO, que e a mesma nocao do `agrupar`.
    fn braco_da_uniao(
        &self,
        sub: &Json,
        base: &str,
        ordinal: usize,
        sessao: &Sessao,
    ) -> Result<BracoDaUniao> {
        let rotulo = format!("braco {ordinal} da uniao");
        let (linhas, modelo, cortado) = self.linhas_do_sub_pedido(sub, base, &rotulo, sessao)?;

        let cabecalho: Vec<(String, ColumnType)> = modelo
            .into_iter()
            .filter(|(n, _)| !coluna_do_motor(n))
            .collect();
        if cabecalho.is_empty() {
            return Err(PhxError::Esquema(format!(
                "o {rotulo} nao trouxe coluna nenhuma para empilhar: uma uniao \
                 empilha posicao a posicao, e sem cabecalho nao ha posicao"
            )));
        }

        let valores: Vec<Vec<Value>> = linhas
            .iter()
            .map(|l| {
                cabecalho
                    .iter()
                    .map(|(nome, ty)| {
                        crate::consultar::campo(l, nome)
                            .map(|j| crate::consultar::valor_tipado(j, ty).0)
                            .unwrap_or(Value::Null)
                    })
                    .collect()
            })
            .collect();

        let op = sub.texto_ou("op", "varrer").trim().to_string();
        Ok(BracoDaUniao {
            linhas: valores,
            cabecalho,
            tabelas: crate::direito_coluna::tabelas_do_pedido(&op, sub),
            cortado,
        })
    }
}

/// Os valores de um indice, na ordem das colunas dele.
pub(super) fn valores_do_indice(esquema: &Schema, indice: &str, linha: &[Value]) -> Vec<Value> {
    // UM lugar so, e ele mora no `crate::upsert` porque o DbLink tambem
    // precisa dele e nao enxerga este modulo.
    crate::upsert::valores_do_indice(esquema, indice, linha)
}

/// Quantas linhas o pivot varre, no maximo.
const TETO_PIVOT: u64 = 5_000_000;
/// Quantas linhas uma tabela de consulta pode ter para caber na memoria.
const TETO_JUNCAO: usize = 500_000;

/// Le `cidade` ou `cliente.cidade` e diz de onde o campo vem.
fn resolver_campo(
    nome: &str,
    esquema: &Schema,
    juncoes: &[Juncao],
    granularidade: &str,
) -> Result<Campo> {
    let g = Granularidade::de_texto(granularidade)?;
    // Prefixo de junção primeiro: uma tabela de consulta chamada `cliente` com
    // uma coluna `cidade` tem de ganhar de uma coluna local chamada
    // `cliente.cidade`, que nao existe -- o ponto so aparece por junção.
    if let Some((pref, campo)) = nome.split_once('.') {
        if let Some(i) = juncoes.iter().position(|j| j.prefixo == pref) {
            let c = posicao_da_coluna(&juncoes[i].esquema, campo).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a coluna {campo:?} nao existe em {}",
                    juncoes[i].esquema.nome()
                ))
            })?;
            return Ok(Campo {
                qualificado: nome.to_string(),
                juncao: Some(i),
                coluna: c,
                granularidade: g,
            });
        }
    }
    let c = posicao_da_coluna(esquema, nome).ok_or_else(|| {
        PhxError::Esquema(format!(
            "a coluna {nome:?} nao existe em {}. Para usar uma coluna de tabela \
             juntada, escreva prefixo.coluna",
            esquema.nome()
        ))
    })?;
    Ok(Campo {
        qualificado: nome.to_string(),
        juncao: None,
        coluna: c,
        granularidade: g,
    })
}

/// Percorre linhas que ja estao na memoria.
/// Um braco da uniao que chegou como PEDIDO, ja materializado.
///
/// Mora fora do `impl` e nao numa tupla de tres listas porque tupla de tres
/// listas e o tipo que ninguem le duas vezes igual.
struct BracoDaUniao {
    /// As linhas na ordem do cabecalho, ja em `Value`.
    linhas: Vec<Vec<Value>>,
    /// Nome e tipo de cada coluna, na ordem da linha -- sem as do motor.
    cabecalho: Vec<(String, ColumnType)>,
    /// As tabelas que o braco nomeia, para a resposta dizer o que leu.
    tabelas: Vec<String>,
    /// O braco parou no teto e empilhou so um pedaco. Vive aqui e nao no
    /// resultado da uniao porque o corte e DO BRACO: a uniao tem o dela.
    cortado: bool,
}

/// Esta coluna e do MOTOR, e por isso nao entra numa uniao?
///
/// O `rowid` e a alca da linha e vem na frente de todo `varrer`/`buscar`; as
/// de sistema vem do formato e ficam no fim. A lista das de sistema sai do
/// `phxsql-core` em vez de ser copiada aqui: coluna de sistema nova entra la e
/// some daqui junto -- lista repetida e onde a quinta seria esquecida.
pub(super) fn coluna_do_motor(nome: &str) -> bool {
    let n = nome.to_ascii_lowercase();
    // `rowid` nao e coluna do esquema (nao tem constante no `schema.rs`): ele
    // e o numero do slot, que a resposta do `varrer` poe ao lado da linha --
    // entao ele NAO sai pela lista do `phxsql-core` e precisa do nome aqui.
    n == "rowid" || phxsql_core::schema::e_coluna_de_sistema(&n)
}

struct LinhasEmMemoria(std::vec::IntoIter<Vec<Value>>);

impl crate::pivot::Iterador for LinhasEmMemoria {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        Ok(self.0.next())
    }
}

/// O cabecalho de uma junção ou união, no formato que a grade da tela espera.
fn colunas_da_juncao(colunas: &[crate::juncao::ColunaSaida]) -> Json {
    Json::Lista(
        colunas
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("lado", Json::texto_de(c.lado)),
                    ("chave", Json::Bool(c.chave)),
                ])
            })
            .collect(),
    )
}

/// Percorre a tabela de fatos linha a linha, sem materializa-la.
struct LinhasDaTabela<'a> {
    rowids: std::vec::IntoIter<u64>,
    tabela: &'a mut Table,
}

impl crate::pivot::Iterador for LinhasDaTabela<'_> {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        for rowid in self.rowids.by_ref() {
            if let Some(l) = self.tabela.ler(rowid)? {
                return Ok(Some(l));
            }
        }
        Ok(None)
    }
}

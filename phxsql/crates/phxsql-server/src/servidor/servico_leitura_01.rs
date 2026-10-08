//! Leitura: catalogo, varrer, buscar, exportar, estatisticas e verificar.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

fn texto_ou_nulo(t: Option<&str>) -> Json {
    match t {
        Some(t) => Json::texto_de(t),
        None => Json::Nulo,
    }
}

impl Servidor {
    /// O catalogo das operacoes -- o `--help` do protocolo, servido por dados.
    ///
    /// Filtra pelo poder de quem perguntou, e a filtragem NAO e o portao: o
    /// portao continua sendo o do `despachar`, que confere de novo quando a
    /// operacao for chamada. Esconder aqui e cortesia, para nao oferecer
    /// oitenta operacoes a quem so pode chamar tres.
    ///
    /// Com o campo `"operacao"`, detalha uma so -- e e assim que o
    /// `/help <comando>` do `phxsqlcmd` funciona sem carregar o catalogo
    /// inteiro. O campo nao se chama `"op"` porque esse ja e o nome da
    /// operacao chamada: um pedido nao tem a mesma chave duas vezes.
    pub(super) fn op_catalogo(&self, p: &Json, sessao: &Sessao) -> Json {
        let base = p.texto_ou("database", "");
        let usuario = sessao.usuario.as_ref();
        let visiveis = crate::catalogo::visiveis(usuario, base);

        if let Some(pedida) = Some(p.texto_ou("operacao", "").trim()).filter(|s| !s.is_empty()) {
            // Operacao que existe mas este usuario nao pode chamar responde
            // "nao existe para voce", com a permissao que faltou -- e nao um
            // 404 seco, que mandaria procurar erro de digitacao onde nao ha.
            let achada = visiveis.iter().find(|o| o.nomes().any(|n| n == pedida));
            return match achada {
                Some(o) => Json::objeto(vec![
                    ("pedida", Json::texto_de(o.nome)),
                    ("operacao", o.para_json()),
                ]),
                None => {
                    let existe = crate::catalogo::por_nome(pedida);
                    Json::objeto(vec![
                        ("pedida", Json::texto_de(pedida)),
                        ("operacao", Json::Nulo),
                        (
                            "motivo",
                            Json::texto_de(match existe {
                                Some(o) => format!(
                                    "a operacao {pedida:?} existe, mas exige {}",
                                    o.atividade().map(|a| a.nome()).unwrap_or("login")
                                ),
                                None => format!("a operacao {pedida:?} nao existe"),
                            }),
                        ),
                    ])
                }
            };
        }

        Json::objeto(vec![
            ("total", Json::de_u64(visiveis.len() as u64)),
            // Quantas ficaram de fora por permissao. Sem este numero, quem ve
            // uma lista curta nao sabe se o servidor e pequeno ou se ele e que
            // pode pouco.
            (
                "ocultas",
                Json::de_u64((crate::catalogo::OPERACOES.len() - visiveis.len()) as u64),
            ),
            (
                "operacoes",
                Json::Lista(visiveis.iter().map(|o| o.para_json()).collect()),
            ),
        ])
    }

    pub(super) fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let t = self.abrir_travada(&_trava, p, sessao)?;
        let e = t.esquema();
        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let papel = e.papel_da_coluna(i);
                Json::objeto(vec![
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("dado_pessoal", Json::texto_de(c.dado_pessoal.nome())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    // As regras de escrita, em texto de expressao; nulo e
                    // «nao tem». Sao o que o `criar_tabela` recebeu.
                    (
                        "padrao",
                        texto_ou_nulo(c.padrao.as_ref().map(Expressao::texto)),
                    ),
                    (
                        "check",
                        texto_ou_nulo(c.check.as_ref().map(Expressao::texto)),
                    ),
                    (
                        "calculada",
                        texto_ou_nulo(c.calculada.as_ref().map(Expressao::texto)),
                    ),
                    // Coluna do MOTOR: a tela nao a oferece como campo de
                    // formulario. Quem manda nela e o botao de excluir.
                    (
                        "sistema",
                        Json::Bool(phxsql_core::schema::e_coluna_de_sistema(&c.nome)),
                    ),
                    // O papel nas chaves e DERIVADO dos indices e das FKs, e
                    // por isso nao pode discordar delas.
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nas_chaves_estrangeiras",
                        Json::Lista(
                            papel
                                .chaves_estrangeiras
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ])
            })
            .collect();
        let indices: Vec<Json> = e
            .indices()
            .iter()
            .map(|i| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),
                    ("primario", Json::Bool(i.primario)),
                    ("composto", Json::Bool(i.composta())),
                    ("onde", texto_ou_nulo(i.onde.as_ref().map(Expressao::texto))),
                    (
                        "colunas",
                        Json::Lista(
                            i.colunas
                                .iter()
                                .enumerate()
                                .map(|(k, ic)| {
                                    Json::objeto(vec![
                                        ("coluna", Json::texto_de(&e.colunas()[ic.coluna].nome)),
                                        ("desc", Json::Bool(ic.desc)),
                                        ("nocase", Json::Bool(ic.nocase)),
                                        (
                                            "expressao",
                                            texto_ou_nulo(
                                                i.expressoes
                                                    .get(k)
                                                    .and_then(Option::as_ref)
                                                    .map(Expressao::texto),
                                            ),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let fks: Vec<Json> = e
            .chaves_estrangeiras()
            .iter()
            .map(|fk| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&fk.nome)),
                    (
                        "colunas",
                        Json::Lista(
                            fk.colunas
                                .iter()
                                .map(|c| Json::texto_de(&e.colunas()[*c].nome))
                                .collect(),
                        ),
                    ),
                    ("tabela_ref", Json::texto_de(&fk.tabela_ref)),
                    (
                        "colunas_ref",
                        Json::Lista(fk.colunas_ref.iter().map(Json::texto_de).collect()),
                    ),
                    ("ao_excluir", Json::texto_de(format!("{:?}", fk.ao_excluir))),
                    ("ao_alterar", Json::texto_de(format!("{:?}", fk.ao_alterar))),
                    // Sem este campo, a ida-e-volta DESLIGA a conferencia em
                    // silencio: quem le o esquema e o manda de volta para
                    // `criar_tabela` perde a garantia sem nenhum aviso. O que
                    // a operacao devolve tem de poder voltar como entrada.
                    ("verificar", Json::Bool(fk.verificar)),
                ])
            })
            .collect();
        let pag = e.paginacao();
        // Os arquivos que a tabela TEM, medidos no disco -- e nao a lista
        // digitada que a tela carregava. Colhidos aqui porque a trava ja esta
        // na mao e o database ja esta aberto.
        let arquivos: Vec<Json> = _trava
            .abrir_database(p.texto_ou("database", ""))
            .ok()
            .and_then(|db| db.arquivos_da_tabela(p.texto_ou("tabela", "")).ok())
            .unwrap_or_default()
            .into_iter()
            .map(Json::texto_de)
            .collect();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(e.nome())),
            ("arquivos", Json::Lista(arquivos)),
            ("registros", Json::de_u64(t.registros())),
            ("slots", Json::de_u64(t.slots())),
            // O material do `.reg` desta tabela -- e nao o interruptor do
            // servidor. Ver `material_da_tabela`.
            ("material", Json::texto_de(material_da_tabela(&t))),
            // A UNICA diretiva que e da tabela e nao da geometria nem do
            // servidor. Ela era lida no `excluir` e nao aparecia em lugar
            // nenhum do esquema: a tela de Configuracoes da tabela nao tinha
            // como mostrar o que a tabela exige.
            ("motivo_obrigatorio", Json::Bool(e.motivo_obrigatorio())),
            ("colunas", Json::Lista(colunas)),
            ("indices", Json::Lista(indices)),
            ("chaves_estrangeiras", Json::Lista(fks)),
            // Os indices de TEXTO, e sem eles a tela nao tinha como saber que
            // a tabela sabe ser procurada por palavra -- ela mostraria a
            // varredura como unica saida numa tabela com `.fts` de pe. A
            // coluna sai por NOME, como no `criar_tabela`: posicao e detalhe
            // de implementacao, e a tela nao deve traduzir indice.
            (
                "indices_texto",
                Json::Lista(
                    e.indices_de_texto()
                        .iter()
                        .map(|it| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(&it.nome)),
                                (
                                    "coluna",
                                    Json::texto_de(
                                        e.colunas()
                                            .get(it.coluna)
                                            .map(|c| c.nome.as_str())
                                            .unwrap_or(""),
                                    ),
                                ),
                                ("dobrar", Json::Bool(it.dobrar)),
                            ])
                        })
                        .collect(),
                ),
            ),
            // Na particao por periodo o volume nao sai de conta: quem sabe
            // onde cada faixa comeca e a tabela de fronteiras, lida dos
            // cabecalhos. Sem isto a tela teria de adivinhar.
            (
                "volumes",
                Json::Lista(
                    t.fronteiras()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            Json::objeto(vec![
                                ("volume", Json::de_u64(i as u64 + 1)),
                                ("primeiro_rowid", Json::de_u64(f.primeiro_rowid)),
                                (
                                    "periodo",
                                    match pag.modo.periodo() {
                                        None => Json::Nulo,
                                        Some(per) => Json::texto_de(per.rotulo(f.chave_periodo)),
                                    },
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "paginacao",
                if pag.ligada() {
                    Json::objeto(vec![
                        (
                            "registros_por_arquivo",
                            Json::de_u64(pag.registros_por_arquivo),
                        ),
                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),
                        // A largura do sufixo vai junto porque sem ela nao da
                        // para escrever o nome do volume: `#1` e `#001` sao
                        // arquivos diferentes.
                        ("digitos", Json::de_u64(pag.digitos as u64)),
                        (
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
                                                    // Pelo motor que compoe o
                                                    // nome do volume (508).
                                                    Json::texto_de(format!(
                                                        "{}{}.reg",
                                                        e.nome(),
                                                        pag.sufixo(n)
                                                    )),
                                                ),
                                                (
                                                    "registros",
                                                    Json::de_u64(
                                                        baldes.get(i).copied().unwrap_or(0),
                                                    ),
                                                ),
                                                ("existe", Json::Bool(existentes.contains(&n))),
                                                (
                                                    "primeiro_rowid",
                                                    Json::de_u64(
                                                        (n as u64 - 1) * pag.registros_por_arquivo
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
                        ),
                        (
                            "coluna",
                            match pag.modo.coluna() {
                                None => Json::Nulo,
                                Some(i) => Json::texto_de(&e.colunas()[i].nome),
                            },
                        ),
                        // A coluna que o ROWID devolve a quem so viu o rowid
                        // (pedido 543). Sai separada da `coluna` pelo motivo
                        // que `coluna_que_o_rowid_revela` escreve: hoje as
                        // duas respondem igual, e o direito por coluna tem de
                        // ler a pergunta certa no dia em que divergirem.
                        (
                            "coluna_do_rowid",
                            match pag.modo.coluna_que_o_rowid_revela() {
                                None => Json::Nulo,
                                Some(i) => Json::texto_de(&e.colunas()[i].nome),
                            },
                        ),
                        ("bytes_por_arquivo", Json::de_u64(pag.bytes_por_arquivo)),
                    ])
                } else {
                    Json::Nulo
                },
            ),
        ]))
    }

    /// `ler`: uma linha pelo rowid.
    ///
    /// Com `"com_versao": true` a resposta deixa de ser a linha crua e passa
    /// a ser `{linha, rowid, versao}`. A forma muda porque a versao NAO pode
    /// entrar como mais uma chave dentro da linha: ali ela viraria uma coluna
    /// que nao existe no esquema, e todo cliente que percorre as chaves da
    /// resposta comecaria a mandar de volta um campo fantasma. Quem nao pede
    /// continua recebendo exatamente o que sempre recebeu.
    pub(super) fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let com_versao = p.booleano_ou("com_versao", false);
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let linha = match t.ler(rowid)? {
            None => return Ok(Json::Nulo),
            Some(l) => linha_para_json(&l, t.esquema()),
        };
        Self::trilhar_acesso(&mut t, rowid, 1, || format!("rowid={rowid}"))?;
        if !com_versao {
            return Ok(linha);
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("linha", linha),
            ("versao", Json::de_u64(t.versao(rowid)?.unwrap_or(0))),
        ]))
    }

    /// `varrer`: uma pagina da grade.
    ///
    /// # As duas pistas, e por que a de leitura e uma TENTATIVA
    ///
    /// Esta e a unica operacao que tenta a ficha COMPARTILHADA primeiro --
    /// leitor deixa de esperar leitor. Tres coisas fazem uma tabela voltar
    /// para a ficha exclusiva (ver `abrir_para_ler_travada`), e as tres sao
    /// escrita escondida num caminho de leitura. Quando alguma delas manda, a
    /// ficha compartilhada e SOLTA antes de a exclusiva ser pedida -- pedir as
    /// duas na mesma thread e o abraco mortal que a `COM_A_TRAVA` acusa.
    ///
    /// # Guarda nova entra pedida, nao imposta
    ///
    /// Nada muda para quem chama: mesma resposta, mesmos campos, mesma trilha.
    /// A pista de leitura e uma decisao do servidor sobre COMO atender, e nao
    /// um modo que o cliente liga -- e o teste que mais importa aqui e o do
    /// comportamento velho, `sem_a_ficha_compartilhada_nada_muda`.
    pub(super) fn op_varrer(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // O escopo existe para o guard MORRER aqui dentro. Sem ele, a ficha
        // compartilhada continuaria viva na linha em que a exclusiva e pedida.
        {
            let trava = self.travar_dados_para_ler()?;
            if let Some(mut t) = self.abrir_para_ler_travada(&trava, p, sessao)? {
                let (resposta, trilha) = self.varrer_a_pagina(&mut t, p)?;
                debug_assert!(
                    trilha.is_none(),
                    "a pista de leitura recusa tabela com dado pessoal: ninguem \
                     deveria ter trilha para gravar aqui"
                );
                return Ok(resposta);
            }
        }
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let (resposta, trilha) = self.varrer_a_pagina(&mut t, p)?;
        if let Some((criterio, linhas)) = trilha {
            t.registrar_acesso(0, &criterio, linhas)?;
        }
        Ok(resposta)
    }

    /// Junta os rowids de TODAS as linhas que casam o filtro -- o substrato do
    /// `UPDATE`/`DELETE` por faixa. Le so' os rowids (nenhum VALOR de coluna
    /// sai), entao e leitura pura e nao deixa trilha de acesso: quem grava, e
    /// por isso registra o acesso, e o `atualizar`/`excluir` que vem depois,
    /// por linha.
    pub(super) fn op_coletar_rowids(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        {
            let trava = self.travar_dados_para_ler()?;
            if let Some(mut t) = self.abrir_para_ler_travada(&trava, p, sessao)? {
                return self.coletar_os_rowids(&mut t, p);
            }
        }
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        self.coletar_os_rowids(&mut t, p)
    }

    /// O corpo do `coletar_rowids`, igual nas duas fichas -- `Legivel`, entao
    /// nao consegue escrever. Anda a tabela na ordem de digitacao ate o teto,
    /// aplica o MESMO `passa` do `varrer` e do `SelectMemory` (um so lugar
    /// decide «esta linha passa?»), e junta os rowids que casam. Recusa se a
    /// tabela passa de `TETO_COLETA_ROWIDS`, para nunca prometer «a tabela
    /// inteira» sobre uma faixa que so' viu o comeco.
    fn coletar_os_rowids<T: Legivel>(&self, t: &mut T, p: &Json) -> Result<Json> {
        let onde = filtros_do_pedido(p, t.esquema())?;
        let expressao = expressao_do_pedido(p, t.esquema())?;
        let visao = match p.texto_ou("visao", "ativas").trim() {
            "" | "ativas" | "ativos" => Visao::Ativas,
            "excluidas" | "excluidos" => Visao::Excluidas,
            "todas" | "todos" => Visao::Todas,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "visao {outro:?} nao existe; use ativas, excluidas ou todas"
                )))
            }
        };

        // O pedido pode BAIXAR o teto, nunca subir acima do duro -- o mesmo
        // padrao do `max` do `varrer`: um cliente cauteloso limita a propria
        // varredura, e o teste prova a recusa com uma tabela pequena. Zero ou
        // ausente vale o teto duro.
        let pedido_teto = p.inteiro_ou("teto", 0).max(0) as u64;
        let teto = if pedido_teto == 0 {
            TETO_COLETA_ROWIDS
        } else {
            pedido_teto.min(TETO_COLETA_ROWIDS)
        };

        // Um ALEM do teto: se vier `teto+1`, a tabela e maior do que o coletar
        // consegue prometer inteiro, e ele recusa nomeando o limite.
        let (rowids, _como) = t.pagina_por_posicao(0, teto + 1, visao)?;
        if rowids.len() as u64 > teto {
            return Err(PhxError::Esquema(format!(
                "coletar_rowids examina no maximo {teto} linhas para garantir que responde a \
                 tabela inteira, e esta tem mais que isso. Estreite o filtro por uma faixa que \
                 caiba, ou faca a operacao em lotes -- gravar sobre uma faixa com cara de ter \
                 gravado sobre tudo e pior que recusar"
            )));
        }

        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("coletando os rowids que casam"));
        let mut casaram: Vec<Json> = Vec::new();
        for &rowid in &rowids {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            if let Some(l) = t.ler(rowid)? {
                if phxsql_store::memoria::passa(&l, &onde, None, expressao.as_ref(), t.esquema())? {
                    casaram.push(Json::de_u64(rowid));
                }
            }
        }

        Ok(Json::objeto(vec![
            ("examinadas", Json::de_u64(rowids.len() as u64)),
            ("casaram", Json::de_u64(casaram.len() as u64)),
            ("rowids", Json::Lista(casaram)),
        ]))
    }

    /// O corpo da varredura, igual nas duas fichas.
    ///
    /// # Por que ele e generico, e o que isso garante
    ///
    /// Duas copias divergiriam, e a divergencia apareceria como a mesma
    /// consulta respondendo coisas diferentes conforme a tabela tivesse ou nao
    /// dado pessoal -- que e o pior jeito de um defeito aparecer.
    ///
    /// E o `Legivel` nao e so DRY: ele **nao tem metodo de escrita**, entao
    /// este corpo nao consegue escrever, em ficha nenhuma. O que ele deixa
    /// para a trilha volta como valor, e quem tem a ficha exclusiva e que
    /// grava.
    fn varrer_a_pagina<T: Legivel>(
        &self,
        t: &mut T,
        p: &Json,
    ) -> Result<(Json, Option<(String, u64)>)> {
        let max = self.limite(p);
        let indice = p.texto_ou("indice", "").to_string();

        // O PREDICADO. Ele nasce vazio, e vazio quer dizer «tudo passa»: quem
        // nao manda `"onde"` -- todo cliente escrito antes desta versao -- faz
        // exatamente o que fazia, byte por byte. Guarda nova entra pedida.
        //
        // O que ele muda e o TRANSPORTE, e nao a varredura: sem indice, o
        // motor le as mesmas `max` linhas para decidir quais passam. Medido em
        // `--example onde-doi-no-varrer`, numa tabela de 100.000 linhas com
        // `max=2500` e uma em cem casando: a leitura das linhas e 48,0% do
        // tempo e o transporte (montar o JSON, serializar, o fio e a analise
        // no cliente) e 52,0% -- 26.755 us viraram 12.902, e 532.777 bytes no
        // fio viraram 5.638.
        //
        // Por isso `max` continua querendo dizer LINHAS EXAMINADAS: mudar o
        // teto para «linhas devolvidas» faria o filtro que casa pouco varrer a
        // tabela inteira com a trava global na mao, e esse custo ninguem mediu.
        let onde = filtros_do_pedido(p, t.esquema())?;
        // A EXPRESSAO, irma do `onde` e no mesmo lugar dele: quem manda as
        // duas paga as duas, e quem nao manda nenhuma nao paga nada.
        let expressao = expressao_do_pedido(p, t.esquema())?;

        // `visao` decide o que a varredura enxerga. O padrao e "ativas": a
        // linha marcada como excluida some das listas, senao marcar nao teria
        // efeito nenhum.
        let visao = match p.texto_ou("visao", "ativas").trim() {
            "" | "ativas" | "ativos" => Visao::Ativas,
            "excluidas" | "excluidos" => Visao::Excluidas,
            "todas" | "todos" => Visao::Todas,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "visao {outro:?} nao existe; use ativas, excluidas ou todas"
                )))
            }
        };

        // Quatro modos.
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

        let por_indice = !indice.is_empty();
        let (rowids, modo) = if por_indice {
            // O indice devolve rowid na ordem da CHAVE, e nao na do arquivo:
            // continuar "depois do rowid X" nao quer dizer nada aqui, porque
            // o proximo da chave pode ter rowid menor. Entao por indice vale a
            // posicao, e a resposta diz isso em vez de fingir que paginou.
            //
            // E ela PARA no fim da pagina -- NOS DOIS ARQUIVOS, e esta linha
            // ja disse isso quando era verdade so em um. O conserto de 02/09
            // fez o `.reg` parar (192,5 ms com a trava global na mao, contra
            // 4 ms do caminho da ordem de digitacao) e deixou o `.ndx` sendo
            // percorrido inteiro antes do recorte: 50 linhas custavam o mesmo
            // que 1.000, e este comentario se declarava resolvido -- que e o
            // motivo de ninguem olhar de novo. O `.ndx` parou em 05/09
            // (pedido 188): numa tabela de 1.000.000 e pagina de 50, o pedido
            // pelo fio saiu de 54,81 ms e 8.335 paginas do indice para 0,56 ms
            // e 3, e a espera NA TELA saiu de 98 ms para 48 ms. Ver
            // `Table::pagina_por_indice`, `docs/CONCORRENCIA.md` e
            // `--example o-que-a-grade-ordenada-custa`.
            (t.pagina_por_indice(&indice, visao, pular, max)?, "posicao")
        } else if antes >= 0 {
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

        // PONTO DE CANCELAMENTO. A pagina cabe no teto de linhas, mas o teto
        // e de configuracao e pode ser grande: uma varredura de cem mil linhas
        // segura a trava por segundos como qualquer outra.
        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("lendo as linhas da pagina"));
        let mut linhas = Vec::with_capacity(rowids.len());
        for &rowid in &rowids {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            if let Some(l) = t.ler(rowid)? {
                // A PENEIRA VEM ANTES DE MONTAR O JSON, e essa ordem e a
                // funcionalidade inteira: montar para depois jogar fora seria
                // pagar o transporte que o filtro existe para nao pagar.
                //
                // `passa` e a MESMA funcao do `SelectMemory` (mora no
                // `phxsql-store`): duas copias divergiriam, e a divergencia
                // apareceria como o mesmo filtro dando respostas diferentes
                // conforme a tabela estivesse carregada em memoria ou nao.
                // Foi `casa` ate a expressao entrar; virou `passa` quando o
                // predicado passou a ter duas metades, para o LUGAR que decide
                // «esta linha passa?» continuar sendo um so.
                if !phxsql_store::memoria::passa(&l, &onde, None, expressao.as_ref(), t.esquema())?
                {
                    continue;
                }
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }

        // UM registro para a pagina inteira, e nao um por linha: e a decisao
        // que faz a varredura de 10.000 linhas custar um registro de trilha
        // em vez de 10.000 vezes o numero de colunas marcadas. O criterio
        // guarda como a pagina foi pedida, que e o que um auditor precisa
        // para refazer o caminho.
        //
        // O `if` VEM ANTES do `format!`, como antes: numa tabela sem coluna
        // marcada -- a maioria -- montar a frase seria alocar duas `String`
        // por leitura para jogar fora. O que mudou e quem grava: o criterio
        // volta como valor e a ficha exclusiva o leva ao disco, porque a
        // compartilhada nao sabe escrever (e por isso ela recusou a tabela).
        let trilha = if t.tem_dado_pessoal() && !linhas.is_empty() {
            let mut peneira = if onde.is_empty() {
                String::new()
            } else {
                format!(" onde={}", descrever_filtros(&onde, t.esquema()))
            };
            // A expressao entra na trilha pelo mesmo motivo do `onde`: «quem
            // procurou os clientes com preco acima de cem?» e exatamente a
            // pergunta que se faz a uma trilha de acesso, e uma trilha que
            // guarda metade do criterio nao responde a pergunta inteira.
            if let Some(e) = &expressao {
                peneira.push_str(&format!(" expressao={:?}", e.texto()));
            }
            let ordem_pedida = if por_indice {
                format!("indice={indice}")
            } else {
                "ordem=digitacao".to_string()
            };
            Some((
                format!(
                    "varrer {ordem_pedida} visao={} modo={modo} pular={pular}{peneira}",
                    match visao {
                        Visao::Ativas => "ativas",
                        Visao::Excluidas => "excluidas",
                        Visao::Todas => "todas",
                    }
                ),
                linhas.len() as u64,
            ))
        } else {
            None
        };

        // O cursor para pedir a proxima pagina e a anterior. Vai pronto na
        // resposta para o cliente nao ter de saber que ele e um rowid -- e
        // para poder deixar de ser um, se um dia a ordem mudar.
        let primeiro = rowids.first().copied().unwrap_or(0);
        let ultimo = rowids.last().copied().unwrap_or(0);
        let rownum_inicio = t.rownum_de(primeiro)?;
        let rownum_fim = t.rownum_de(ultimo)?;
        // "Tem mais" sem contar a tabela: pede UM alem do teto. Uma leitura a
        // mais por pagina, contra uma varredura inteira so para mostrar
        // "pagina 3 de 40" -- que numa tabela grande e o item mais caro da
        // tela e o que ninguem le.
        let ha_mais = ultimo > 0 && !t.pagina_depois_de(ultimo, 1, visao)?.is_empty();
        let ha_antes = primeiro > 1 && !t.pagina_antes_de(primeiro, 1, visao)?.is_empty();

        // Fora do `vec!` porque contar passou a poder falhar: dentro de uma
        // transacao ela le os slots que o conjunto de escrita tocou.
        let visiveis = t.contar(visao)?;
        Ok((
            Json::objeto(vec![
                // `registros` e o que a tabela tem, e sai do cabecalho: nao custa
                // varredura. `total` era a contagem da varredura inteira, e por
                // isso deixou de existir aqui.
                ("registros", Json::de_u64(t.registros())),
                // Quantas linhas ESTA visao enxerga -- e a conta de «pagina 3 de
                // 40». Sai de dois contadores do cabecalho, sem varrer nada; era
                // por nao existir que a contagem tinha sido tirada da resposta.
                ("visiveis", Json::de_u64(visiveis)),
                ("marcadas", Json::de_u64(t.marcadas())),
                ("devolvidas", Json::de_u64(linhas.len() as u64)),
                // Quantas linhas o motor OLHOU para responder. Sem filtro ela e
                // igual a `devolvidas` -- que e a verdade de sempre, agora dita em
                // voz alta. Com filtro as duas se separam, e e essa diferenca que
                // deixa a tela dizer «olhei 2.500 das 100.000, 25 casaram» em vez
                // de dizer que a tabela tem 25 linhas de Blumenau.
                ("examinadas", Json::de_u64(rowids.len() as u64)),
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
            ]),
            trilha,
        ))
    }

    pub(super) fn op_buscar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let indice = p.texto_ou("indice", "").to_string();
        if indice.is_empty() {
            return Err(PhxError::Esquema("informe \"indice\"".into()));
        }
        let chave_json = p
            .campo("chave")
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"chave\"".into()))?;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let pos = t
            .esquema()
            .indice_por_nome(&indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))?;
        let chave = json_para_chave(&chave_json, t.esquema(), pos)?;
        let rowids = t.buscar(&indice, &chave)?;

        let mut linhas = Vec::new();
        for rowid in rowids.iter().take(self.limite(p) as usize) {
            if let Some(l) = t.ler(*rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(*rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        // O criterio guarda o INDICE e a CHAVE pedidos: e o que responde
        // "quem procurou o CPF do fulano?", que e a pergunta que se faz a
        // uma trilha de acesso.
        Self::trilhar_acesso(&mut t, 0, linhas.len() as u64, || {
            format!("{indice}={}", chave_json.escrever())
        })?;
        Ok(Json::objeto(vec![
            ("encontrados", Json::de_u64(rowids.len() as u64)),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    /// Acha as linhas que contem uma palavra, por um indice de texto.
    ///
    /// **A ficha e a EXCLUSIVA, e nao a de leitura**, mesmo sendo uma busca:
    /// abrir uma tabela cujo `.fts` ainda nao nasceu ESCREVE, e a pista de
    /// leitura recusa por isso (`Table::abrir_para_ler`). Atender pela pista
    /// exigiria o vaivem do `varrer` -- tentar, receber o `None`, soltar a
    /// ficha e refazer --, e isso e otimizacao a se fazer com numero, depois
    /// de a bancada dizer se ela paga.
    pub(super) fn op_procurar_texto(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let indice = p.texto_ou("indice", "").to_string();
        if indice.is_empty() {
            return Err(PhxError::Esquema("informe \"indice\"".into()));
        }
        let palavra = p.texto_ou("palavra", "").to_string();
        if palavra.trim().is_empty() {
            return Err(PhxError::Esquema("informe \"palavra\"".into()));
        }
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let achado = t.procurar_texto(&indice, &palavra)?;

        let mut linhas = Vec::new();
        for rowid in achado.rowids.iter().take(self.limite(p) as usize) {
            if let Some(l) = t.ler(*rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(*rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        // Mesma trilha do `buscar`, e pelo mesmo motivo: e ela que responde
        // "quem procurou o nome do fulano?".
        Self::trilhar_acesso(&mut t, 0, linhas.len() as u64, || {
            format!("{indice}~{palavra}")
        })?;
        Ok(Json::objeto(vec![
            ("encontrados", Json::de_u64(achado.rowids.len() as u64)),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    /// Exporta uma tabela, ou o resultado de uma varredura, em sete formatos.
    ///
    /// Binario (XLSX, DOCX) volta em base64, porque o protocolo e JSON por
    /// linha e byte cru nao atravessa. Texto volta como texto, para caber num
    /// `curl` sem ninguem ter de decodificar nada.
    pub(super) fn op_exportar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let f = Formato::de_texto(p.texto_ou("formato", "csv"))?;
        let comeco = Instant::now();
        // Teto proprio, e maior que o da varredura: exportar e justamente o
        // caso em que se quer a tabela inteira, e nao a primeira pagina.
        let teto = p.inteiro_ou("max", 100_000).clamp(1, 1_000_000) as usize;

        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut linhas: Vec<Vec<Value>> = Vec::new();
        let mut truncado = false;
        // PONTO DE CANCELAMENTO. So a VARREDURA e cancelavel: dela para
        // frente o formato ja esta sendo montado em memoria, e abandonar no
        // meio da montagem so jogaria fora trabalho ja feito sem soltar a
        // trava mais cedo.
        let atividade = crate::telemetria::corrente();
        {
            let _fase = atividade
                .as_ref()
                .map(|a| a.fase_cancelavel("lendo a tabela para exportar"));
            for (rowid, _) in t.varrer()? {
                if let Some(a) = &atividade {
                    a.siga(1)?;
                }
                if linhas.len() >= teto {
                    truncado = true;
                    break;
                }
                if let Some(l) = t.ler(rowid)? {
                    linhas.push(l);
                }
            }
        }

        let nome = p.texto_ou("tabela", "tabela");
        let base = p.texto_ou("database", "");
        let planilha = crate::exportar::Planilha {
            titulo: nome.to_string(),
            subtitulo: format!(
                "{base} · {} linha(s) · exportado em {}",
                linhas.len(),
                phxsql_core::datahora::instante_iso(crate::agora_ms())
            ),
            colunas: crate::exportar::Planilha::do_esquema(&esquema, nome),
            linhas: &linhas,
        };
        let bytes = planilha.gerar(f)?;

        // O nome do arquivo sai daqui e nao da tela: quem chama por `curl` tem
        // o mesmo nome que quem clica, e o nome carrega a data.
        let arquivo = format!(
            "{}_{}.{}",
            nome.replace('.', "_"),
            phxsql_core::datahora::instante_iso(crate::agora_ms()).replace([' ', ':', ','], "-"),
            f.extensao()
        );

        let mut campos = vec![
            ("formato", Json::texto_de(p.texto_ou("formato", "csv"))),
            ("arquivo", Json::texto_de(&arquivo)),
            ("mime", Json::texto_de(f.mime())),
            ("bytes", Json::de_u64(bytes.len() as u64)),
            ("linhas", Json::de_u64(linhas.len() as u64)),
            ("truncado", Json::Bool(truncado)),
            ("binario", Json::Bool(f.binario())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ];
        if f.binario() {
            campos.push((
                "base64",
                Json::texto_de(phxsql_core::base64::codificar(&bytes)),
            ));
        } else {
            campos.push((
                "conteudo",
                Json::texto_de(String::from_utf8_lossy(&bytes).to_string()),
            ));
        }
        Ok(Json::objeto(campos))
    }

    // ------------------------------------------------------- estatisticas

    /// O que o log ja sabia e ninguem perguntava.
    ///
    /// # Por que histograma, e nao media
    ///
    /// O painel mostrava "ms medio". Media esconde exatamente o que interessa:
    /// mil respostas de 1 ms e uma de 30 s dao media de 30 ms, e o numero
    /// parece bom enquanto alguem espera meio minuto. O que responde "esta
    /// rapido?" e a cauda -- a mediana, o percentil 95 e o pior caso.
    ///
    /// As faixas dobram (1, 2, 4, 8, 16... ms) porque a diferenca entre 1 ms e
    /// 2 ms importa tanto quanto entre 1 s e 2 s, e faixa de largura fixa
    /// esmagaria a metade rapida num balde so.
    pub(super) fn op_estatisticas(&self, p: &Json) -> Result<Json> {
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let desde = match p.inteiro_ou("horas", 0) {
            0 => 0,
            h => crate::agora_ms() - h.max(1) * 3_600_000,
        };
        let considerar: Vec<&Acesso> = acessos.iter().filter(|a| a.quando_ms >= desde).collect();

        // ------------------------------------------------ por operacao
        let mut por_op: HashMap<&str, Contagem> = HashMap::new();
        let mut por_tabela: HashMap<String, Contagem> = HashMap::new();
        let mut por_usuario: HashMap<&str, Contagem> = HashMap::new();
        let mut por_codigo: HashMap<u16, (u64, String)> = HashMap::new();
        let mut geral = Contagem::default();
        let mut duracoes: Vec<u64> = Vec::with_capacity(considerar.len());

        for a in &considerar {
            geral.somar(a);
            por_op.entry(a.op.as_str()).or_default().somar(a);
            if !a.usuario.is_empty() {
                por_usuario.entry(a.usuario.as_str()).or_default().somar(a);
            }
            // A tabela so entra quando o pedido nomeou uma. Contar "sem tabela"
            // como se fosse uma tabela poluiria a lista com o `ping`.
            if !a.tabela.is_empty() {
                let chave = if a.database.is_empty() {
                    a.tabela.clone()
                } else {
                    format!("{}.{}", a.database, a.tabela)
                };
                por_tabela.entry(chave).or_default().somar(a);
            }
            if let (false, Some(e)) = (a.ok, a.erro.as_ref()) {
                let entrada = por_codigo.entry(a.codigo).or_insert((0, e.clone()));
                entrada.0 += 1;
            }
            duracoes.push(a.duracao_ms);
        }
        duracoes.sort_unstable();

        // As mais demoradas, com nome e objeto. E o registro de consulta lenta
        // do MySQL(R), so que sem precisar ligar nada: o log ja tinha o dado, e
        // faltava a pergunta.
        let mut mais_lentas: Vec<&Acesso> = considerar.clone();
        mais_lentas.sort_by(|a, b| b.duracao_ms.cmp(&a.duracao_ms));
        mais_lentas.truncate(15);

        let percentil = |q: f64| -> u64 {
            if duracoes.is_empty() {
                return 0;
            }
            // Percentil pelo metodo do vizinho mais proximo: com poucas
            // amostras, interpolar inventa um valor que ninguem mediu.
            let i = ((duracoes.len() as f64 - 1.0) * q).round() as usize;
            duracoes[i.min(duracoes.len() - 1)]
        };

        // -------------------------------------------------- histograma
        let mut faixas: Vec<(u64, u64, u64)> = Vec::new();
        let mut teto = 1u64;
        while teto <= 65_536 {
            let piso = if teto == 1 { 0 } else { teto / 2 };
            let quantas = duracoes
                .iter()
                .filter(|d| **d >= piso && **d < teto)
                .count() as u64;
            faixas.push((piso, teto, quantas));
            teto *= 2;
        }
        let acima = duracoes.iter().filter(|d| **d >= 65_536).count() as u64;

        let lista = |mut v: Vec<(String, Contagem)>| -> Json {
            v.sort_by(|a, b| b.1.quantas.cmp(&a.1.quantas));
            v.truncate(30);
            Json::Lista(v.iter().map(|(n, c)| c.para_json(n)).collect())
        };

        Ok(Json::objeto(vec![
            (
                "desde",
                match desde {
                    0 => Json::texto_de("sempre"),
                    d => Json::texto_de(phxsql_core::datahora::instante_iso(d)),
                },
            ),
            ("acessos", Json::de_u64(geral.quantas)),
            ("resumo", geral.para_json("tudo")),
            (
                "latencia",
                Json::objeto(vec![
                    ("p50", Json::de_u64(percentil(0.50))),
                    ("p90", Json::de_u64(percentil(0.90))),
                    ("p95", Json::de_u64(percentil(0.95))),
                    ("p99", Json::de_u64(percentil(0.99))),
                    ("pior", Json::de_u64(duracoes.last().copied().unwrap_or(0))),
                ]),
            ),
            (
                "histograma",
                Json::Lista(
                    faixas
                        .iter()
                        .map(|(piso, teto, n)| {
                            Json::objeto(vec![
                                ("de_ms", Json::de_u64(*piso)),
                                ("ate_ms", Json::de_u64(*teto)),
                                ("quantas", Json::de_u64(*n)),
                            ])
                        })
                        .chain(std::iter::once(Json::objeto(vec![
                            ("de_ms", Json::de_u64(65_536)),
                            ("ate_ms", Json::Nulo),
                            ("quantas", Json::de_u64(acima)),
                        ])))
                        .collect(),
                ),
            ),
            (
                "por_operacao",
                lista(
                    por_op
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
            ),
            ("por_tabela", lista(por_tabela.into_iter().collect())),
            (
                "por_usuario",
                lista(
                    por_usuario
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
            ),
            (
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
                "por_erro",
                Json::Lista({
                    let mut v: Vec<(u16, (u64, String))> = por_codigo.into_iter().collect();
                    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
                    v.truncate(15);
                    v.iter()
                        .map(|(codigo, (n, exemplo))| {
                            Json::objeto(vec![
                                ("codigo", Json::de_u64(*codigo as u64)),
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
                                ("exemplo", Json::texto_de(exemplo)),
                            ])
                        })
                        .collect()
                }),
            ),
        ]))
    }

    pub(super) fn op_verificar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let r = t.verificar()?;
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&r.tabela)),
            ("registros", Json::de_u64(r.registros)),
            ("slots", Json::de_u64(r.slots)),
            ("eventos", Json::de_u64(r.eventos)),
            (
                "indices",
                Json::Objeto(
                    r.indices
                        .iter()
                        .map(|(n, q)| (n.clone(), Json::de_u64(*q)))
                        .collect(),
                ),
            ),
            (
                "volumes",
                Json::objeto(vec![
                    ("reg", Json::de_u64(r.volumes.0 as u64)),
                    ("bin", Json::de_u64(r.volumes.1 as u64)),
                    ("memo", Json::de_u64(r.volumes.2 as u64)),
                    ("log", Json::de_u64(r.volumes.3 as u64)),
                ]),
            ),
        ]))
    }

    pub(super) fn op_reindexar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let indices = t.reindexar()?;
        self.gravar_de_verdade(&_trava, &mut t, p)?;
        Ok(Json::Objeto(
            indices
                .into_iter()
                .map(|(n, q)| (n, Json::de_u64(q)))
                .collect(),
        ))
    }
}

/// Os filtros `"onde"` de um pedido, contra o esquema da tabela.
///
/// UM parser para os dois caminhos -- o `SelectMemory`, que le da memoria, e
/// o `varrer`, que le do disco. Escrever a leitura do `"onde"` duas vezes
/// daria dois contratos com a mesma cara: bastava um deles aprender um
/// operador novo, ou tratar um nulo de outro jeito, para o mesmo pedido
/// responder coisas diferentes conforme a tabela estivesse carregada.
///
/// Pedido sem `"onde"` devolve lista vazia -- e e por isso que quem nunca
/// ouviu falar de filtro continua pagando exatamente o que pagava.
///
/// # O que ele NAO abre
///
/// Nenhum caminho novo para NOMEAR TABELA. O filtro fala de COLUNA da tabela
/// que ja esta em `"tabela"`, e o portao de permissao le esse campo. Se um dia
/// o `"onde"` ganhar uma subconsulta, ela nomeia tabela e passa a precisar de
/// conferencia propria, como `juntar`, `unir` e `pivotar` pagam hoje.
pub(super) fn filtros_do_pedido(
    p: &Json,
    esquema: &phxsql_core::schema::Schema,
) -> Result<Vec<Filtro>> {
    let Some(l) = p.campo("onde").and_then(Json::lista) else {
        return Ok(Vec::new());
    };
    let mut onde = Vec::with_capacity(l.len());
    for f in l {
        let coluna = coluna_de(
            f.campo("coluna")
                .ok_or_else(|| PhxError::Esquema("filtro sem \"coluna\"".into()))?,
            esquema,
        )?;
        let op = Operador::de_texto(f.texto_ou("op", "="))?;
        let valor = match f.campo("valor") {
            Some(v) => crate::valores::json_para_valor_da_coluna(v, &esquema.colunas()[coluna])?,
            None => phxsql_core::value::Value::Null,
        };
        onde.push(Filtro { coluna, op, valor });
    }
    Ok(onde)
}

/// A expressao `"expressao"` de um pedido, analisada UMA VEZ e conferida
/// contra o esquema.
///
/// # O portao vem antes do trabalho, e ele e um `is_none`
///
/// Pedido sem `"expressao"` -- todo cliente escrito antes desta versao --
/// devolve `None` depois de UM olhar no objeto, e dali em diante nada nesta
/// consulta muda: nem uma analise, nem uma `String`, nem uma avaliacao por
/// linha. E a licao que o Profiler cobrou, aplicada antes de doer.
///
/// # Analisada uma vez, avaliada muitas
///
/// A analise acontece AQUI, fora do laco das linhas. Analisar por linha numa
/// pagina de 2.500 seria pagar 2.500 vezes um trabalho que nao depende da
/// linha -- exatamente o que o ponto de captura do Profiler fazia com o JSON
/// do lote.
///
/// # Coluna inexistente recusa na ANALISE, e nao por linha
///
/// `Expressao::colunas()` da os nomes referidos, e eles sao conferidos contra
/// o esquema antes de a primeira linha ser lida. Deixar para o avaliador faria
/// `cidde = 'X'` (com o erro de digitacao) devolver zero linha em vez de
/// dizer que a coluna nao existe -- e zero linha e uma resposta que parece
/// certa.
pub(super) fn expressao_do_pedido(
    p: &Json,
    esquema: &phxsql_core::schema::Schema,
) -> Result<Option<phxsql_core::expressao::Expressao>> {
    let Some(texto) = p.campo("expressao").and_then(Json::texto) else {
        return Ok(None);
    };
    if texto.trim().is_empty() {
        return Ok(None);
    }
    let e = phxsql_core::expressao::Expressao::analisar(texto)?;
    for nome in e.colunas() {
        if !esquema
            .colunas()
            .iter()
            .any(|c| c.nome.eq_ignore_ascii_case(nome))
        {
            return Err(PhxError::Esquema(format!(
                "a expressao usa a coluna {nome:?}, que nao existe em {}",
                esquema.nome()
            )));
        }
    }
    Ok(Some(e))
}

/// Como o criterio da trilha de acesso descreve um filtro.
///
/// Vai para a trilha porque «quem procurou os clientes de Blumenau?» e
/// exatamente a pergunta que se faz a uma trilha de acesso -- a mesma razao
/// de o `buscar` guardar o indice e a chave pedidos.
pub(super) fn descrever_filtros(onde: &[Filtro], esquema: &phxsql_core::schema::Schema) -> String {
    onde.iter()
        .map(|f| {
            format!(
                "{} {} {}",
                esquema.colunas()[f.coluna].nome,
                f.op.nome(),
                f.valor.para_texto()
            )
        })
        .collect::<Vec<_>>()
        .join(" E ")
}
/// Quantas linhas o `coletar_rowids` EXAMINA, no maximo, para garantir que
/// responde INTEIRO -- e a razao de ele recusar em vez de truncar.
///
/// Ele e o substrato do `UPDATE`/`DELETE` por faixa: junta TODOS os rowids que
/// casam o filtro ANTES de aplicar (o coletar-antes-de-aplicar que fecha o
/// *Halloween problem*). Um teto sobre linhas EXAMINADAS, e nao devolvidas: se
/// a tabela passa disto, o motor nao consegue prometer «respondi sobre a tabela
/// inteira» -- entao ele recusa nomeando o limite, no lugar de gravar sobre a
/// primeira faixa com cara de ter gravado sobre tudo (a mesma lei que faz o
/// `SELECT` sem indice recusar em vez de responder a primeira pagina).
const TETO_COLETA_ROWIDS: u64 = 1_000_000;

/// O que se conta sobre um grupo de acessos.
///
/// Guarda a soma e o pior caso, e nao a lista: somar por tabela num log de
/// milhoes de linhas nao pode custar uma copia por linha.
#[derive(Debug, Default, Clone)]
pub(super) struct Contagem {
    quantas: u64,
    recusadas: u64,
    soma_ms: u64,
    pior_ms: u64,
    ultimo_ms: i64,
}

impl Contagem {
    fn somar(&mut self, a: &Acesso) {
        self.quantas += 1;
        if !a.ok {
            self.recusadas += 1;
        }
        self.soma_ms += a.duracao_ms;
        self.pior_ms = self.pior_ms.max(a.duracao_ms);
        self.ultimo_ms = self.ultimo_ms.max(a.quando_ms);
    }

    fn para_json(&self, nome: &str) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(nome)),
            ("quantas", Json::de_u64(self.quantas)),
            ("recusadas", Json::de_u64(self.recusadas)),
            (
                "ms_medio",
                Json::de_u64(if self.quantas > 0 {
                    self.soma_ms / self.quantas
                } else {
                    0
                }),
            ),
            ("ms_pior", Json::de_u64(self.pior_ms)),
            ("ms_total", Json::de_u64(self.soma_ms)),
            (
                "ultimo",
                match self.ultimo_ms {
                    0 => Json::Nulo,
                    q => Json::texto_de(phxsql_core::datahora::instante_iso(q)),
                },
            ),
        ])
    }
}

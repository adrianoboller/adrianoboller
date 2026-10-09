//! Visoes, consultar e agrupar, com os filtros e os iteradores da consulta.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    // ------------------------------------------------------------- visoes

    /// `criar_visao`: guarda o TEXTO de um `SELECT` por database.
    ///
    /// # Duas recusas na DECLARACAO, e nao na hora de usar
    ///
    /// 1. **SQL que a camada nao analisa.** Guardar o texto sem analisa-lo
    ///    faria o erro de sintaxe aparecer meses depois, na consulta de outra
    ///    pessoa, com o nome da visao no lugar do nome de quem a escreveu.
    /// 2. **Nome que colide com tabela.** Duas coisas com o mesmo nome no
    ///    `FROM` e uma delas invisivel para sempre -- e qual das duas ganha
    ///    viraria detalhe de implementacao.
    ///
    /// E a mesma decisao do `ao_excluir`: uma visao nasce uma vez e e usada um
    /// milhao de vezes. Recusar cedo custa um erro lido enquanto se cria;
    /// recusar tarde custa uma consulta quebrada no dia do primeiro uso.
    ///
    /// O que NAO se confere na declaracao e a existencia da TABELA de dentro:
    /// ela pode ser criada depois, e sobretudo pode ser APAGADA depois -- uma
    /// conferencia na criacao daria uma garantia que o tempo desfaz. Quem
    /// confere e o uso, que recusa nomeando a tabela que sumiu.
    pub(super) fn op_criar_visao(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let base = p.texto_ou("database", "").trim().to_string();
        let nome = p.texto_ou("nome", "").trim().to_string();
        let sql = p
            .texto_ou("sql", p.texto_ou("texto", ""))
            .trim()
            .to_string();
        if sql.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"sql\" com o SELECT da visao".into(),
            ));
        }
        // Analisada AQUI, e o resultado jogado fora de proposito: o que
        // interessa e o veredito. Guardar o plano congelaria a visao contra um
        // esquema que envelhece (ver `crate::visoes`).
        match phxsql_sql::analisar_comando(&sql)? {
            phxsql_sql::Comando::Selecao(_) => {}
            // Um SELECT COMPOSTO (JOIN, subconsulta, `IN (SELECT ...)`, WITH,
            // janela, UNION) analisa como `Comando::Consulta`, e o `verbo()`
            // dele tambem e "SELECT" -- por isso a recusa antiga se
            // contradizia: "uma visao guarda um SELECT, e este texto e um
            // SELECT". A visao guarda TEXTO e e reanalisada a cada uso por
            // `selecao_sobre_visao`, que so resolve o `Comando::Selecao`
            // simples; aceitar o composto aqui gravaria uma visao que so
            // falharia no dia do primeiro FROM. Ate haver esse substrato, a
            // recusa e HONESTA e na criacao -- e nomeia o que falta, nao um
            // verbo que se nega a si mesmo.
            phxsql_sql::Comando::Consulta(_) => {
                return Err(PhxError::Esquema(
                    "visao com JOIN, subconsulta, WITH, janela ou UNION nao tem substrato \
                     nesta rodada: guarde um SELECT simples, ou componha por fora \
                     (SELECT ... FROM (a visao) ...)"
                        .into(),
                ))
            }
            outro => {
                return Err(PhxError::Esquema(format!(
                    "uma visao guarda um SELECT, e este texto e um {}",
                    outro.verbo()
                )))
            }
        }
        {
            let dados = self.travar_dados()?;
            if let Ok(db) = dados.abrir_database(&base) {
                if db
                    .todas_as_tabelas()?
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(&nome))
                {
                    return Err(PhxError::Duplicado(format!(
                        "{base}.{nome} ja e uma TABELA: uma visao com esse nome \
                         deixaria uma das duas invisivel no FROM"
                    )));
                }
            }
        }
        let (criada, devido) = {
            let mut v = self.visoes.tomar("visoes")?;
            let feito = v.criar(
                &base,
                &nome,
                &sql,
                sessao.login(),
                p.booleano_ou("substituir", false),
            )?;
            self.ha_visoes.store(v.ha_visoes(), Ordering::Relaxed);
            feito
        };
        self.gravar_visoes(Some(devido))?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("visao", criada.para_json()),
        ]))
    }

    /// `visoes`: o que ha. O texto vai verbatim a quem administra o database
    /// ou escreveu a visao; aos outros, REDIGIDO pelo analisador (pedido 359).
    ///
    /// A op pede so `ler` (`usuarios.rs`), e a premissa que a sustentava --
    /// «o texto de um SELECT diz que tabelas existem, e nao o que ha nelas» --
    /// vale para a FORMA e nao para o literal: um `WHERE cpf='...'` guardado
    /// entregava o CPF a quem tinha a coluna negada. Sem usuario (token de
    /// servico) e o dono do servidor, como no `exigir_administrar_rotina`.
    pub(super) fn op_visoes(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let base = p.texto_ou("database", "").trim().to_string();
        let administra = sessao
            .usuario
            .as_ref()
            .is_none_or(|u| u.pode_em(&base, "", Atividade::Administrar));
        let v = self.visoes.tomar("visoes")?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            (
                "visoes",
                Json::Lista(
                    v.do_db(&base)
                        .iter()
                        .map(|x| {
                            let autor = !x.criado_por.is_empty() && x.criado_por == sessao.login();
                            if administra || autor {
                                x.para_json()
                            } else {
                                x.para_json_redigida()
                            }
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// `excluir_visao`. Visao que nao existia devolve `excluida: false` em vez
    /// de erro -- e o mesmo `DROP VIEW IF EXISTS` que todo mundo escreve, e
    /// erro aqui obrigaria a listar antes para poder apagar.
    pub(super) fn op_excluir_visao(&self, p: &Json) -> Result<Json> {
        let base = p.texto_ou("database", "").trim().to_string();
        let nome = p.texto_ou("nome", "").trim().to_string();
        let devido = {
            let mut v = self.visoes.tomar("visoes")?;
            let devido = v.excluir(&base, &nome);
            self.ha_visoes.store(v.ha_visoes(), Ordering::Relaxed);
            devido
        };
        let saiu = devido.is_some();
        self.gravar_visoes(devido)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("nome", Json::texto_de(&nome)),
            ("excluida", Json::Bool(saiu)),
        ]))
    }

    /// O `FROM` desta selecao aponta para uma VISAO? Entao o pedido vira um
    /// `consultar` sobre o plano dela.
    ///
    /// # O portao atomico vem primeiro
    ///
    /// A primeira linha e um `load` num `AtomicBool`. Num servidor sem visao
    /// nenhuma -- que e o de hoje -- toda consulta paga isso e mais nada: nem
    /// trava, nem String, nem busca. E a mesma decisao do `ha_gatilhos`,
    /// tomada antes de doer.
    ///
    /// # Por que o plano de dentro sai do MESMO caminho de sempre
    ///
    /// A visao guarda TEXTO. Analisar aqui e traduzir com
    /// `phxsql_sql::traduzir` -- os mesmos dois passos de um `SELECT` comum --
    /// e o que faz a visao envelhecer junto com a tabela: coluna nova aparece,
    /// tabela apagada RECUSA nomeando, no dia em que alguem consulta.
    ///
    /// # A integração que falta, e o que ela troca
    ///
    /// A frente F-SQL entrega `phxsql_sql::planejar_sobre(&selecao,
    /// plano_de_dentro)`, que monta o `consultar` de fora a partir da `Selecao`
    /// externa. Ela NAO existe nesta arvore ainda, entao o `consultar` e
    /// montado aqui, campo por campo -- e a integração troca este bloco pela
    /// chamada, sem mexer no resto: o que sai daqui e um pedido JSON, e o
    /// pedido e o contrato.
    pub(super) fn selecao_sobre_visao(
        &self,
        selecao: &phxsql_sql::Selecao,
        base: &str,
        sessao: &Sessao,
    ) -> Result<Option<phxsql_sql::Plano>> {
        if !self.ha_visoes.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let alvo = selecao.de.nome_no_protocolo();
        let sql_da_visao = {
            let v = self.visoes.tomar("visoes")?;
            match v.por_nome(base, &alvo) {
                Some(x) => x.sql.clone(),
                None => return Ok(None),
            }
        };

        // O SELECT de DENTRO, traduzido como qualquer outro -- inclusive
        // pedindo o esquema pelo protocolo, que ja exige `ler` naquela tabela.
        // E daqui que sai a recusa nomeando quando a tabela sumiu.
        let dentro = match phxsql_sql::analisar_comando(&sql_da_visao)? {
            phxsql_sql::Comando::Selecao(s) => s,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "a visao {alvo:?} guarda um {}, e nao um SELECT",
                    outro.verbo()
                )))
            }
        };
        let base_de_dentro = match dentro.de.database.trim() {
            "" => base.to_string(),
            outro => outro.to_string(),
        };
        let tabela_de_dentro = dentro.de.nome_no_protocolo();
        let ped_esquema = Json::objeto(vec![
            ("database", Json::texto_de(&base_de_dentro)),
            ("tabela", Json::texto_de(&tabela_de_dentro)),
        ]);
        let esquema = self
            .executar_derivado("esquema", &ped_esquema, sessao)
            .map_err(|e| erro_da_visao(&alvo, &tabela_de_dentro, e))?;
        let plano = phxsql_sql::traduzir(&dentro, &indices_do_esquema(&esquema), &base_de_dentro)?;

        // O `consultar` de FORA sai de `phxsql_sql::planejar_sobre`, que a
        // F-SQL entrega: a `Selecao` externa (o WHERE, a projecao, a ordem, o
        // LIMIT) montada em cima do plano de dentro. Este bloco ja foi feito a
        // mao aqui, com o comentario dizendo que a integração o trocaria pela
        // chamada -- e trocou. O contrato entre os dois lados e o PEDIDO JSON,
        // e por isso a troca nao mexeu em mais nada.
        let mut de = plano.pedido.clone();
        if let Json::Objeto(pares) = &mut de {
            if !pares.iter().any(|(k, _)| k == "op") {
                pares.push(("op".to_string(), Json::texto_de(&plano.op)));
            }
        }
        // A visao que PROJETA colunas guarda essa projecao no `plano.saida`, e
        // nao no `plano.pedido`: o pedido de dentro e um `varrer`/`buscar` que
        // devolve a linha INTEIRA, e a projecao e um passo de SAIDA do
        // tradutor. Passar so o `plano.pedido` como `de` descartava esse passo,
        // e por isso `SELECT * FROM v_so_nome` recebia todas as colunas -- a
        // visao criada para esconder colunas nao escondia nenhuma. Envolvemos o
        // `de` num `consultar` que aplica a projecao da visao ANTES do SELECT
        // de fora, pelo mesmo `colunas` que o `op_consultar` ja projeta: assim
        // o SELECT externo so enxerga o que a visao expoe, como toda
        // subconsulta.
        if let phxsql_sql::Saida::Colunas(cols) = &plano.saida {
            let colunas = Json::Lista(
                cols.iter()
                    .map(|(nome, apelido)| {
                        if nome == apelido {
                            Json::texto_de(nome)
                        } else {
                            Json::objeto(vec![
                                ("coluna", Json::texto_de(nome)),
                                ("apelido", Json::texto_de(apelido)),
                            ])
                        }
                    })
                    .collect(),
            );
            de = Json::objeto(vec![
                ("op", Json::texto_de("consultar")),
                ("database", Json::texto_de(&base_de_dentro)),
                ("de", de),
                ("colunas", colunas),
            ]);
        }
        Ok(Some(phxsql_sql::planejar_sobre(selecao, de)?))
    }

    /// Planeja um `SELECT` COMPOSTO -- o que a F-SQL devolve como
    /// `Comando::Consulta`.
    ///
    /// # O resolvedor, e por que ele e um fecho e nao uma tabela de esquemas
    ///
    /// `traduzir_consulta` sabe montar o `consultar`, mas nao sabe abrir
    /// tabela: para traduzir cada `SELECT` de dentro (o `de`, o de cada
    /// junção, o de cada `escalar`, o de cada `IN`) ela precisa dos INDICES
    /// daquela tabela, e quem os enxerga e o servidor. Entao ela recebe um
    /// fecho, e o fecho pede o esquema pelo `executar_derivado` -- que ja
    /// exige `ler` naquela tabela.
    ///
    /// **E aqui que a permissao entra duas vezes, e as duas contam.** Uma no
    /// planejamento (o `esquema` de cada tabela citada) e outra na execucao
    /// (cada sub-pedido, dentro do `op_consultar`). A primeira recusa antes de
    /// montar o plano; a segunda recusaria de qualquer jeito. Nenhuma das duas
    /// e enfeite: um plano que nao se pode montar nao chega a rodar, e um
    /// plano montado por outro caminho ainda para na segunda.
    ///
    /// E o `FROM` de dentro pode ser uma VISAO: o resolvedor pergunta antes de
    /// traduzir, pelo mesmo `selecao_sobre_visao` da consulta simples. Sem
    /// isso, `SELECT ... FROM v JOIN t` acharia que `v` e uma tabela.
    pub(super) fn planejar_consulta(
        &self,
        c: &phxsql_sql::Consulta,
        base: &str,
        sessao: &Sessao,
    ) -> Result<phxsql_sql::Plano> {
        let mut erro_de_dentro: Option<PhxError> = None;
        let mut resolver = |sel: &phxsql_sql::Selecao, db: &str| -> Result<phxsql_sql::Plano> {
            let base_do_lado = match sel.de.database.trim() {
                "" => db.to_string(),
                outro => outro.to_string(),
            };
            if let Some(plano) = self.selecao_sobre_visao(sel, &base_do_lado, sessao)? {
                return Ok(plano);
            }
            let ped_esquema = Json::objeto(vec![
                ("database", Json::texto_de(&base_do_lado)),
                ("tabela", Json::texto_de(sel.de.nome_no_protocolo())),
            ]);
            let esquema = match self.executar_derivado("esquema", &ped_esquema, sessao) {
                Ok(e) => e,
                Err(e) => {
                    // O erro viaja por fora porque o `Resolvedor` devolve o
                    // `Result` do crate de SQL, e reembalar aqui perderia a
                    // classe -- e a classe e o que diz se foi permissao ou
                    // tabela que nao existe.
                    erro_de_dentro = Some(e);
                    return Err(PhxError::Esquema("sub-pedido recusado".into()));
                }
            };
            phxsql_sql::traduzir(sel, &indices_do_esquema(&esquema), &base_do_lado)
        };
        let plano = phxsql_sql::traduzir_consulta(c, base, &mut resolver);
        match erro_de_dentro {
            Some(e) => Err(e),
            None => plano,
        }
    }

    /// As linhas de um sub-pedido, pelo MESMO portao de qualquer cliente --
    /// e o MODELO delas, que e o que a linha JSON perdeu ao atravessar o
    /// protocolo.
    ///
    /// # Esta e a funcao inteira do `consultar`
    ///
    /// O `consultar` nao le tabela nenhuma. Ele pede a outra operacao, e a
    /// outra operacao passa por `executar_derivado` -- politica, portao de
    /// permissao (inclusive o direito por coluna) e execucao, exatamente como
    /// um `{"op":"varrer","tabela":"folha"}` que chegasse pela rede. Por isso
    /// `SELECT ... FROM folha` de quem nao le a folha para AQUI, com o mesmo
    /// erro, e nao ha conferencia propria que alguem possa esquecer de
    /// atualizar.
    ///
    /// # De onde vem o modelo
    ///
    /// `varrer`/`buscar`: do `esquema` da tabela, pedido pelo MESMO
    /// `executar_derivado` (como `planejar_consulta` ja fazia para o SQL) --
    /// `rowid` na frente, como a linha traz, e SEM as colunas que o direito
    /// por coluna nega a este usuario, que o `esquema` lista em
    /// `colunas_sem_leitura`. `agrupar` e `consultar`: do cabecalho `colunas`
    /// que as duas respondem, montado no mesmo lugar que tipou os valores.
    ///
    /// E depois o modelo e ACERTADO pela primeira linha, quando ha uma: a
    /// ordem e os nomes passam a ser os da linha, e coluna que a linha tem e o
    /// modelo nao conhece entra com o tipo que o FORMATO do JSON sugere -- o
    /// que este modulo fazia antes de existir modelo. Assim o modelo nunca
    /// nomeia coluna que a linha nao tem, e nunca deixa de nomear uma que ela
    /// tem; sobre resultado vazio, ele e a unica forma que existe.
    ///
    /// # Quem chama, e por que a lista de ops nao muda por isso
    ///
    /// Eram cinco lugares, todos dentro do `consultar` (`de`, `juntar[].de`,
    /// `escalar[].de`, `em[].de` e `existe[].de`). Desde o pedido 393 ha um
    /// SEXTO, fora do `consultar`: cada braco de `unir` com `"partes"`. Nada
    /// aqui muda por causa dele -- e esse o ganho de reusar o contrato em vez
    /// de inventar um segundo. Por isso as recusas daqui falam da
    /// «composicao», e nao do `consultar`: quem le a mensagem pode nao ter
    /// escrito um `consultar`.
    ///
    /// # A lista de ops e curta de proposito
    ///
    /// So o que DEVOLVE `linhas`. Uma op de escrita aqui dentro faria um
    /// `SELECT` gravar; uma op que devolve outra forma faria a composicao
    /// receber `linhas: []` e responder «nenhuma linha» sobre um resultado que
    /// existe. Operacao nova nasce RECUSADA ate alguem decidir o contrario --
    /// o mesmo principio de `OPS_EMPILHAVEIS`.
    ///
    /// # O terceiro valor: o sub-pedido PAROU no teto?
    ///
    /// Um sub-pedido que parou em `max_linhas` entrega um PEDACO, e
    /// a composicao monta a resposta sobre esse pedaco: o `IN` responde «nao
    /// casa» a chave que casava, o `EXISTS` responde «nao existe» a linha que
    /// existia, e a conta sai com cara de resposta (pedido 419). O corte nao
    /// vira recusa -- quem ja compoe dentro do teto nao pode parar de compor
    /// --, vira o `truncado` da resposta. Quem traduz o dialeto de cada
    /// operacao e `parou_no_teto`, logo abaixo.
    pub(super) fn linhas_do_sub_pedido(
        &self,
        sub: &Json,
        base_de_fora: &str,
        rotulo: &str,
        sessao: &Sessao,
    ) -> Result<(Vec<crate::consultar::Linha>, crate::consultar::Modelo, bool)> {
        const OPS_QUE_DEVOLVEM_LINHAS: &[&str] =
            &["varrer", "buscar", "agrupar", "group_by", "consultar"];
        let op = sub.texto_ou("op", "varrer").trim().to_string();
        if !OPS_QUE_DEVOLVEM_LINHAS.contains(&op.as_str()) {
            return Err(PhxError::Esquema(format!(
                "o {rotulo} pede a operacao {op:?}, e a composicao so aceita o que \
                 devolve linhas: {}",
                OPS_QUE_DEVOLVEM_LINHAS.join(", ")
            )));
        }
        // O `database` de fora viaja para dentro quando o sub-pedido nao diz o
        // dele -- e quando diz, e o DELE que o portao confere. Herdar sem
        // deixar sobrescrever faria o campo do sub-pedido virar enfeite; nao
        // herdar obrigaria a repetir a base em cada nivel.
        let mut pedido = sub.clone();
        let sem_base = sub.texto_ou("database", "").trim().is_empty();
        if let (true, Json::Objeto(pares)) = (sem_base, &mut pedido) {
            pares.retain(|(k, _)| k != "database");
            pares.push(("database".to_string(), Json::texto_de(base_de_fora)));
        }
        let bruto = self.executar_derivado(&op, &pedido, sessao)?;
        let lista = bruto.campo("linhas").and_then(Json::lista).ok_or_else(|| {
            PhxError::Esquema(format!(
                "o {rotulo} chamou {op:?} e a resposta nao trouxe \"linhas\""
            ))
        })?;
        let teto = self.max_linhas();
        // ESTA RECUSA NAO E QUEM ACUSA O CORTE, e o `>` esta certo.
        //
        // Medido em 23/09/2026, funcao por funcao, nas quatro que atendem as
        // cinco operacoes da lista: `varrer_a_pagina`, `op_buscar`,
        // `op_agrupar` e `op_consultar` recortam todas por `self.limite(p)`,
        // que e `min(max pedido, teto)`. Nenhuma delas consegue devolver MAIS
        // que o teto, entao este `if` nao dispara -- ele e a rede da operacao
        // que entrar na lista sem recortar, nao o aviso de que houve corte.
        //
        // E por isso ele segue `>` e nao `>=`: `len() == teto` e exatamente o
        // resultado de quem FOI cortado pelo teto, o caso comum. Um `>=` aqui
        // viraria o aviso que faltava numa RECUSA nova, contra composicao que
        // hoje responde sem erro -- protecao que quebra quem ja funciona e
        // estrago, nao protecao.
        if lista.len() as u64 > teto {
            return Err(PhxError::LimiteExcedido(format!(
                "o {rotulo} trouxe {} linhas, acima do teto de {teto} de \
                 `max_linhas`: a composicao guarda cada sub-pedido \
                 INTEIRO em memoria. Filtre dentro do sub-pedido",
                lista.len()
            )));
        }
        let cortado = Self::parou_no_teto(&op, &pedido, &bruto, lista.len(), teto);
        let linhas: Vec<crate::consultar::Linha> = lista
            .iter()
            .map(|l| match l {
                Json::Objeto(pares) => pares.clone(),
                // Uma linha que nao e objeto nao tem coluna com nome, e o
                // resto do `consultar` fala por nome. Vira linha vazia em vez
                // de estourar: quem projetar uma coluna dela recebe a recusa
                // que nomeia a coluna, que ensina mais.
                _ => Vec::new(),
            })
            .collect();

        let declarado = match op.as_str() {
            "varrer" | "buscar" => self.modelo_da_tabela(&pedido, sessao)?,
            _ => crate::consultar::modelo_de_json(bruto.campo("colunas")),
        };
        let modelo = match linhas.first() {
            Some(primeira) => primeira
                .iter()
                .map(|(n, v)| {
                    let tipo = crate::consultar::tipo_no_modelo(&declarado, n)
                        .unwrap_or_else(|| crate::consultar::valor_de_json(v).1);
                    (n.clone(), tipo)
                })
                .collect(),
            None => declarado,
        };
        Ok((linhas, modelo, cortado))
    }

    /// O sub-pedido parou porque o TETO mandou parar?
    ///
    /// # Por que cada operacao avisa de um jeito, e por que a traducao fica
    /// AQUI
    ///
    /// Nenhuma das cinco precisou ganhar campo novo: as quatro ja dizem que
    /// pararam, cada uma na forma que faz sentido para ela. Dar a todas um
    /// `truncado` seria escrever a MESMA decisao com um segundo nome ao lado
    /// do primeiro -- o `ha_mais` do `varrer` ja e essa resposta.
    ///
    /// O preco e que alguem tem de saber os quatro dialetos, e este alguem e
    /// UM so: esta funcao, encostada na lista `OPS_QUE_DEVOLVEM_LINHAS` de
    /// proposito. Operacao nova entra na lista de cima e cai no `_` daqui,
    /// que conta como CORTADA ate alguem dizer como ela avisa -- pelo mesmo
    /// principio que faz a lista nascer recusando: ruido chama atencao,
    /// silencio nao.
    ///
    /// # O corte que o pedido PEDIU nao e truncamento
    ///
    /// `{"max":1,"ordem":[...]}` e o idioma documentado do `escalar`, e
    /// `{"max":10}` num sub-pedido e um `LIMIT`. Marcar truncado neles faria
    /// toda composicao correta acender a bandeira, e bandeira que acende
    /// sempre e bandeira que ninguem le -- que e como o corte calado
    /// sobreviveria ao proprio conserto. So conta o corte que o teto impos:
    /// sub-pedido sem `max`, ou com `max` acima do teto.
    fn parou_no_teto(op: &str, sub: &Json, bruto: &Json, trazidas: usize, teto: u64) -> bool {
        let do_teto = o_teto_e_quem_corta(sub, teto);
        match op {
            // `ha_mais` e a pagina que nao terminou a tabela -- a MESMA
            // pergunta que um `truncado` responderia.
            "varrer" => do_teto && bruto.booleano_ou("ha_mais", false),
            // O indice achou mais do que a pagina carregou. Aqui o par
            // `encontrados`/`linhas` e que diz, porque a busca por chave nao
            // tem cursor para ter `ha_mais`.
            "buscar" => do_teto && bruto.inteiro_ou("encontrados", 0).max(0) as usize > trazidas,
            // O `agrupar` varre a tabela INTEIRA e RECUSA quando os grupos
            // passam do teto (`o_teto_de_grupos_recusa_nomeando`), entao o
            // `truncado` dele so pode ser o `max` de quem pediu, e o crivo de
            // cima o apaga. Le-se o campo mesmo assim, e nao um `false`
            // cravado: o dia em que aquela recusa virar corte, este ramo ja
            // esta certo -- um `false` aqui seria a decisao de la escrita de
            // novo, em outro arquivo do raciocinio.
            "agrupar" | "group_by" => do_teto && bruto.booleano_ou("truncado", false),
            // O `consultar` JA aplicou esta regra nos sub-pedidos dele, e o
            // `truncado` dele viaja SEM o crivo: um `max` no nivel de fora
            // nao pode apagar o corte que aconteceu tres niveis abaixo.
            "consultar" => bruto.booleano_ou("truncado", false),
            _ => true,
        }
    }

    /// O modelo de uma tabela como o `varrer` e o `buscar` a devolvem:
    /// `rowid` e depois cada coluna do esquema, menos as negadas a este
    /// usuario.
    ///
    /// Passa pelo `esquema` via `executar_derivado`, e nao por uma leitura
    /// propria do arquivo: o portao continua sendo UM, e o `esquema` e quem
    /// ja sabe listar o que o direito por coluna esconde deste usuario --
    /// tirar as negadas aqui pela mesma lista e o que impede o modelo de
    /// nomear uma coluna que a peneira acabou de tirar da linha.
    fn modelo_da_tabela(&self, pedido: &Json, sessao: &Sessao) -> Result<crate::consultar::Modelo> {
        let esquema = self.executar_derivado(
            "esquema",
            &Json::objeto(vec![
                ("database", Json::texto_de(pedido.texto_ou("database", ""))),
                ("tabela", Json::texto_de(pedido.texto_ou("tabela", ""))),
            ]),
            sessao,
        )?;
        let negadas = esquema.textos("colunas_sem_leitura");
        let mut modelo = vec![("rowid".to_string(), phxsql_core::types::ColumnType::UInt8)];
        for c in esquema
            .campo("colunas")
            .and_then(Json::lista)
            .unwrap_or(&[])
        {
            let nome = c.texto_ou("nome", "").trim();
            if nome.is_empty() || negadas.iter().any(|n| n.eq_ignore_ascii_case(nome)) {
                continue;
            }
            modelo.push((
                nome.to_string(),
                crate::valores::tipo_de_texto(c.texto_ou("tipo", ""))?,
            ));
        }
        Ok(modelo)
    }

    /// `consultar`: a composicao -- o passo do SQL que nao e leitura de tabela.
    ///
    /// # A ordem dos passos, e por que ela e fixa
    ///
    /// `de` -> `juntar` (na ordem) -> `escalar` -> `existe` -> `em` ->
    /// `expressao` -> `por`/`agregados`/`tendo` -> `janela` -> `ordem` ->
    /// `pular`/`max` -> `colunas`. Cada
    /// um depende do anterior: a junção tem de acontecer antes de qualquer
    /// filtro que fale das duas tabelas, o escalar entra antes da expressao
    /// porque e ela quem o ve, o `existe` e o `em` sao filtros de conjunto e
    /// filtrar cedo evita avaliar a expressao sobre linha que ja saiu, a
    /// agregacao vem depois da `expressao` e antes da `janela` (a ordem dos
    /// quatro motores maduros: FROM+JOIN+WHERE, depois agrupar, depois
    /// janela), a
    /// janela numera o que sobrou do filtro (numerar antes daria buracos na
    /// numeracao), o recorte corta o que ja esta ordenado, e a PROJECAO VEM
    /// POR ULTIMO porque a expressao e a ordem podem falar de uma coluna que a
    /// resposta nao mostra -- `ORDER BY preco` num `SELECT nome` e SQL
    /// legitimo.
    ///
    /// # O modelo anda junto das linhas
    ///
    /// Cada passo que muda a forma da linha muda o modelo no mesmo lugar: a
    /// junção concatena os dois, o escalar e a janela acrescentam uma coluna,
    /// a agregacao SUBSTITUI o modelo pelas colunas de `por` mais os apelidos,
    /// a projecao escolhe e renomeia. A resposta traz o modelo final em
    /// `colunas`, e e por ele que um `consultar` aninhado e o cliente sabem a
    /// forma da linha sem esperar a primeira linha chegar.
    ///
    /// # A profundidade tem teto
    ///
    /// Um `consultar` cujo `de` e outro `consultar` desce a pilha de verdade.
    /// Sem teto, um pedido de duzentos niveis derruba a thread por estouro de
    /// pilha -- e derrubar a thread nao e recusar, e cair. O contador e por
    /// thread e o guarda o devolve na saida, inclusive quando o passo do meio
    /// falha.
    pub(super) fn op_consultar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::consultar::{self as cs, Modelo};
        let _fundo = Profundidade::descer()?;
        let comeco = Instant::now();
        let base = p.texto_ou("database", "").trim().to_string();

        let de = p
            .campo("de")
            .ok_or_else(|| PhxError::Esquema("o consultar precisa de \"de\"".into()))?;
        // `cortado` acumula o corte de TODOS os sub-pedidos, e nao so o do
        // `de`: o `IN` sobre meio conjunto e o `EXISTS` sobre meio lado dao
        // resposta errada com cara de certa, exatamente como o `de` cortado.
        let (mut linhas, mut modelo, mut cortado) =
            self.linhas_do_sub_pedido(de, &base, "\"de\"", sessao)?;

        // ------------------------------------------------------------ juntar
        //
        // Aplicadas NA ORDEM, uma por vez, sempre com o acumulado a esquerda.
        // E o mesmo que o SQL faz com `A JOIN B JOIN C`, e a ordem importa
        // porque um `esquerdo` no meio muda quantas linhas chegam ao proximo.
        let juncoes = p.campo("juntar").and_then(Json::lista).unwrap_or(&[]);
        // O lado de fora so ganha prefixo quando ha junção: sem ela nenhuma
        // coluna precisa dele, e prefixar mudaria a forma da resposta de todo
        // cliente que existe. Quem precisa saber se prefixou e o `existe`.
        let mut prefixado = false;
        if !juncoes.is_empty() {
            // Prefixar acontece ANTES da primeira junção e vale para os DOIS
            // lados: sem isso, `id` seria a coluna da esquerda antes do JOIN e
            // um nome ambiguo depois dele, e a mesma consulta mudaria de
            // sentido conforme alguem acrescentasse uma junção.
            let apelido = apelido_do_lado(p, de, "de")?;
            let mut usados = vec![apelido.to_lowercase()];
            cs::prefixar(&mut linhas, &mut modelo, &apelido);
            prefixado = true;

            for (i, j) in juncoes.iter().enumerate() {
                let rotulo = format!("\"juntar\"[{i}]");
                let tipo = cs::TipoJuncao::de_texto(j.texto_ou("tipo", ""))?;
                let sub = j
                    .campo("de")
                    .ok_or_else(|| PhxError::Esquema(format!("a junção {i} precisa de \"de\"")))?;
                let ap = apelido_do_lado(j, sub, &rotulo)?;
                if usados.iter().any(|u| *u == ap.to_lowercase()) {
                    return Err(PhxError::Esquema(format!(
                        "o apelido {ap:?} ja esta em uso nesta consulta; de outro \
                         a junção {i} para as colunas dos dois lados nao se \
                         sobreporem"
                    )));
                }
                usados.push(ap.to_lowercase());
                let (mut direita, mut modelo_dir, cortou) =
                    self.linhas_do_sub_pedido(sub, &base, &rotulo, sessao)?;
                cortado |= cortou;
                cs::prefixar(&mut direita, &mut modelo_dir, &ap);

                let itens = j.campo("em").and_then(Json::lista).unwrap_or(&[]);
                let teto = self.max_linhas();
                let mut pares: Vec<(String, String)> = Vec::with_capacity(itens.len());
                if tipo.casa_por_pares() {
                    // Os pares. Sem eles a junção seria um produto cartesiano
                    // disfarcado -- e um produto de duas tabelas de dez mil
                    // linhas e cem milhoes, que ninguem pediu por engano.
                    if itens.is_empty() {
                        return Err(PhxError::Esquema(format!(
                            "a junção {i} precisa de \"em\" com ao menos um par \
                             {{esquerda, direita}}: junção sem par e produto \
                             cartesiano, e ele se pede com tipo \"cruzado\""
                        )));
                    }
                    for (k, par) in itens.iter().enumerate() {
                        let e = par.texto_ou("esquerda", "").trim().to_string();
                        let d = par.texto_ou("direita", "").trim().to_string();
                        if e.is_empty() || d.is_empty() {
                            return Err(PhxError::Esquema(format!(
                                "o par {k} da junção {i} precisa de \"esquerda\" e \
                                 \"direita\""
                            )));
                        }
                        // Cada lado se resolve contra o SEU modelo: `p.id` do
                        // lado de ca, `c.id` do lado de la. O nome sem prefixo
                        // vale quando e unico naquele lado.
                        let re = resolver_ou_recusar(&modelo, &e, "o lado esquerdo")?;
                        let rd = resolver_ou_recusar(&modelo_dir, &d, "o lado direito")?;
                        pares.push((re, rd));
                    }
                } else {
                    // O `cruzado` e o produto, e produto nao casa por
                    // igualdade: um `em` aqui e um pedido que se contradiz, e
                    // escolher um dos dois sentidos calado seria responder
                    // outra pergunta.
                    if j.campo("em").is_some() {
                        return Err(PhxError::Esquema(format!(
                            "a junção cruzada {i} nao aceita \"em\": o produto nao \
                             casa por igualdade -- tire o \"em\", ou use \
                             \"interno\"/\"esquerdo\" para casar"
                        )));
                    }
                    // O teto e conferido ANTES de materializar. Materializar
                    // cem milhoes de linhas para depois recusar e o estrago, e
                    // nao a protecao.
                    let produto = (linhas.len() as u128) * (direita.len() as u128);
                    if produto > teto as u128 {
                        return Err(PhxError::LimiteExcedido(format!(
                            "a junção cruzada {i} produziria {} x {} = {produto} \
                             linhas, acima do teto de {teto} de \
                             `max_linhas` -- recusada ANTES de \
                             materializar. Filtre um dos lados dentro do \
                             sub-pedido dele",
                            linhas.len(),
                            direita.len()
                        )));
                    }
                }
                // O teto entra NA junção, que para na linha teto+1: conferir
                // sobre a lista pronta era materializar um milhao de linhas
                // (+561 MiB, medidos em 1000 x 1000 com a mesma chave) para
                // recusar contra um teto de mil. Ver `consultar::juntar`.
                let Some(juntadas) = cs::juntar(
                    linhas,
                    &modelo,
                    &direita,
                    &modelo_dir,
                    &pares,
                    tipo,
                    usize::try_from(teto).unwrap_or(usize::MAX),
                ) else {
                    return Err(PhxError::LimiteExcedido(format!(
                        "a junção {i} produziria mais de {teto} linhas, acima do teto \
                         de `max_linhas` -- recusada ao chegar na linha {}, \
                         sem materializar o resto. Filtre um dos lados dentro do \
                         sub-pedido dele",
                        teto + 1
                    )));
                };
                linhas = juntadas;
                modelo.extend(modelo_dir);
            }
        }

        // O apelido do lado de FORA, SEM junção -- e o que faz `c.id` (num
        // `EXISTS`, numa projecao `c.nome` ou num `ORDER BY c.id`) casar com a
        // coluna `id`/`nome` da linha de fora, que sem junção nao ganhou
        // prefixo. Com junção fica `None`: ali a linha JA vem prefixada e o
        // `c.` diz de qual lado a coluna vem -- descarta-lo perderia essa
        // informacao (pedido 240). O tradutor so poe `apelido` no pedido quando
        // alguem de fora o cita, entao a chave presente ja e sinal de citacao.
        let apelido_de_fora = if prefixado {
            None
        } else {
            apelido_do_lado(p, de, "de").ok()
        };

        // ----------------------------------------------------------- escalar
        //
        // Uma subconsulta NAO CORRELACIONADA que devolve uma linha so: ela roda
        // UMA vez, e o valor vira coluna de todas -- com o tipo que o
        // sub-pedido dela declarou, que e o que faz `preco > media` comparar
        // Decimal com Decimal. Correlacionada exigiria roda-la por linha, e
        // isso recusa nomeando (ver `docs/SQL.md`); a correlacao por igualdade
        // e o `existe`, logo abaixo.
        let mut escalares: Vec<String> = Vec::new();
        for (i, e) in p
            .campo("escalar")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let nome = e.texto_ou("nome", "").trim().to_string();
            if nome.is_empty() {
                return Err(PhxError::Esquema(format!(
                    "o escalar {i} precisa de \"nome\": e por ele que a expressao o ve"
                )));
            }
            let sub = e
                .campo("de")
                .ok_or_else(|| PhxError::Esquema(format!("o escalar {i} precisa de \"de\"")))?;
            let rotulo = format!("\"escalar\"[{i}]");
            let (dentro, modelo_dentro, cortou) =
                self.linhas_do_sub_pedido(sub, &base, &rotulo, sessao)?;
            cortado |= cortou;
            // Zero ou duas linhas RECUSA nomeando. Escolher a primeira faria a
            // resposta depender da ordem em que o motor devolveu as linhas, e
            // «depende da ordem» num numero e o defeito que nao se acha.
            if dentro.len() != 1 {
                return Err(PhxError::Esquema(format!(
                    "o escalar {nome:?} devolveu {} linhas, e um valor escalar \
                     precisa de exatamente uma. Ponha um agregado ou um \"max\":1 \
                     com \"ordem\" no sub-pedido",
                    dentro.len()
                )));
            }
            let campo_alvo = e.texto_ou("campo", "").trim().to_string();
            let unica = &dentro[0];
            let (real, valor) = if campo_alvo.is_empty() {
                // Sem `campo`, a linha tem de ter UMA coluna so -- senao nao ha
                // como saber qual delas e o valor.
                if unica.len() != 1 {
                    return Err(PhxError::Esquema(format!(
                        "o escalar {nome:?} nao diz \"campo\", e a linha devolvida \
                         tem {} colunas: diga qual delas e o valor",
                        unica.len()
                    )));
                }
                (unica[0].0.clone(), unica[0].1.clone())
            } else {
                let real = resolver_ou_recusar(
                    &modelo_dentro,
                    &campo_alvo,
                    &format!("o escalar {nome:?}"),
                )?;
                let valor = cs::campo(unica, &real).cloned().ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "o escalar {nome:?} pede o campo {campo_alvo:?}, que a \
                         linha devolvida nao tem. As que ha: {}",
                        unica
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?;
                (real, valor)
            };
            let tipo = cs::tipo_no_modelo(&modelo_dentro, &real)
                .unwrap_or_else(|| cs::valor_de_json(&valor).1);
            if modelo.iter().any(|(n, _)| n.eq_ignore_ascii_case(&nome)) {
                return Err(PhxError::Esquema(format!(
                    "o escalar {nome:?} usa o nome de uma coluna que a linha ja \
                     tem; de outro nome"
                )));
            }
            for l in linhas.iter_mut() {
                l.push((nome.clone(), valor.clone()));
            }
            modelo.push((nome.clone(), tipo));
            escalares.push(nome);
        }

        // ------------------------------------------------------------ existe
        //
        // O `[NOT] EXISTS (SELECT ... WHERE fora.c = dentro.c)`: semijunção
        // por espalhamento. Vem DEPOIS do escalar, que acrescenta coluna, e
        // ANTES da expressao, que e filtro por linha -- e nao acrescenta
        // coluna nenhuma, so decide quem fica. Cada `de` passa pelo mesmo
        // portao dos outros quatro caminhos; ver `crate::consultar::semijuntar`
        // para a divergencia (so igualdade) e o motivo dela.
        for (i, item) in p
            .campo("existe")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let rotulo = format!("\"existe\"[{i}]");
            let sub = item
                .campo("de")
                .ok_or_else(|| PhxError::Esquema(format!("o existe {i} precisa de \"de\"")))?;
            let ap = apelido_do_lado(item, sub, &rotulo)?;
            let (mut dentro, mut modelo_dentro, cortou) =
                self.linhas_do_sub_pedido(sub, &base, &rotulo, sessao)?;
            cortado |= cortou;
            cs::prefixar(&mut dentro, &mut modelo_dentro, &ap);
            let itens = item.campo("em").and_then(Json::lista).unwrap_or(&[]);
            if itens.is_empty() {
                return Err(PhxError::Esquema(format!(
                    "o existe {i} precisa de \"em\" com ao menos um par {{esquerda, \
                     direita}}: e por igualdade que a semijunção casa, e sem par \
                     ela seria «a tabela tem alguma linha?»"
                )));
            }
            // O lado de fora sem junção nao tem prefixo, mas quem escreve
            // `EXISTS` escreve `c.id` -- entao o prefixo do proprio lado de
            // fora e aceito e descartado (`apelido_de_fora`, calculado uma vez
            // la em cima). Outro prefixo continua recusando, porque nao nomeia
            // lado nenhum.
            let mut pares: Vec<(String, String)> = Vec::with_capacity(itens.len());
            for (k, par) in itens.iter().enumerate() {
                let e = par.texto_ou("esquerda", "").trim().to_string();
                let d = par.texto_ou("direita", "").trim().to_string();
                if e.is_empty() || d.is_empty() {
                    return Err(PhxError::Esquema(format!(
                        "o par {k} do existe {i} precisa de \"esquerda\" e \"direita\""
                    )));
                }
                let e = tirar_prefixo_de_fora(&e, apelido_de_fora.as_deref());
                let re =
                    resolver_ou_recusar(&modelo, &e, &format!("o lado de fora do existe {i}"))?;
                let rd = resolver_ou_recusar(
                    &modelo_dentro,
                    &d,
                    &format!("o lado de dentro do existe {i}"),
                )?;
                pares.push((re, rd));
            }
            let nao = item.booleano_ou("nao", false);
            let decimais = cs::pares_decimais(&pares, &modelo, &modelo_dentro);
            linhas = cs::semijuntar(linhas, &dentro, &pares, &decimais, nao);
        }

        // ---------------------------------------------------------------- em
        //
        // O `IN (SELECT campo FROM ...)`. Vem antes da expressao porque e um
        // filtro de conjunto, e filtrar cedo evita avaliar a expressao sobre
        // linha que ja saiu.
        for (i, item) in p
            .campo("em")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let coluna = item.texto_ou("coluna", "").trim().to_string();
            if coluna.is_empty() {
                return Err(PhxError::Esquema(format!(
                    "o item {i} de \"em\" precisa de \"coluna\""
                )));
            }
            let sub = item.campo("de").ok_or_else(|| {
                PhxError::Esquema(format!("o item {i} de \"em\" precisa de \"de\""))
            })?;
            let rotulo = format!("\"em\"[{i}]");
            let (dentro, modelo_dentro, cortou) =
                self.linhas_do_sub_pedido(sub, &base, &rotulo, sessao)?;
            cortado |= cortou;
            let campo_alvo = item.texto_ou("campo", &coluna).trim().to_string();
            // O `campo` se resolve contra o MODELO do sub-pedido, como o
            // `escalar` e o `existe` ja faziam -- e nao contra cada linha. O
            // `filter_map` sozinho engolia a ausencia: um `campo` que o
            // sub-pedido nao devolve dava conjunto vazio, zero linhas e
            // `ok: true`, que e resposta errada calada. E era um furo do
            // direito por coluna: `campo: "salario"` por quem nao le
            // `salario` chegava aqui sem a coluna (a peneira ja a tirou) e
            // saia como «nenhum cliente casa», em vez de recusar nomeando.
            let campo_real = resolver_ou_recusar(
                &modelo_dentro,
                &campo_alvo,
                &format!("o \"campo\" do \"em\"[{i}]"),
            )?;
            // O `IN` entre dois `Decimal` compara pelo VALOR, nao pela
            // escala: `"10.50"` e `"10.5000"` sao o mesmo dinheiro e eram
            // chaves diferentes -- medido em 23/09/2026, zero linhas onde os
            // quatro motores devolvem uma. Ver `consultar::pares_decimais`.
            let canonizar = {
                let dentro_decimal = matches!(
                    cs::tipo_no_modelo(&modelo_dentro, &campo_real),
                    Some(ColumnType::Decimal { .. })
                );
                move |k: String, fora_decimal: bool| -> String {
                    if dentro_decimal && fora_decimal {
                        crate::juncao::sem_zeros_a_direita(&k)
                    } else {
                        k
                    }
                }
            };
            // O lado de fora do `IN` tambem pode vir prefixado pelo apelido de
            // fora sem junção (`c.id IN (…)`); o prefixo se descarta, como no
            // `existe` (pedido 240).
            let coluna = tirar_prefixo_de_fora(&coluna, apelido_de_fora.as_deref());
            let real = resolver_ou_recusar(&modelo, &coluna, "o \"em\"")?;
            let fora_decimal = matches!(
                cs::tipo_no_modelo(&modelo, &real),
                Some(ColumnType::Decimal { .. })
            );
            let conjunto: std::collections::HashSet<String> = dentro
                .iter()
                .filter_map(|l| cs::campo(l, &campo_real).and_then(cs::chave_de_juncao))
                .map(|k| canonizar(k, fora_decimal))
                .collect();
            linhas.retain(|l| {
                cs::campo(l, &real)
                    .and_then(cs::chave_de_juncao)
                    .map(|k| canonizar(k, fora_decimal))
                    .is_some_and(|k| conjunto.contains(&k))
            });
        }

        // A expressao, sobre a linha composta, com cada coluna convertida
        // PELO TIPO do modelo. Ver `crate::consultar` para o que isso comprou.
        if let Some(txt) = p.campo("expressao").and_then(Json::texto) {
            if !txt.trim().is_empty() {
                // Pela porta do observador de injecao (495, F3).
                let e = crate::injecao::analisar_expressao(txt)?;
                // A ligacao dos nomes acontece UMA vez, contra o modelo, e e
                // ela que recusa o nome ambiguo -- por pedido, e nao por linha.
                let ligacoes = cs::ligar(&modelo, e.colunas())?;
                let mut mantidas = Vec::with_capacity(linhas.len());
                for l in linhas {
                    // `NULL` exclui, como em todo filtro deste motor.
                    if cs::avaliar_sobre(&l, &e, &ligacoes)? == Some(true) {
                        mantidas.push(l);
                    }
                }
                linhas = mantidas;
            }
        }

        // ----------------------------------------------------------- agregar
        //
        // O `GROUP BY` sobre a linha JA COMPOSTA. A vaga e entre a `expressao`
        // e a `janela`, e ela nao e escolha de estilo: e a ordem dos quatro
        // motores maduros -- no PostgreSQL, `query_planner` (FROM+JOIN+WHERE)
        // -> `create_grouping_paths` -> `create_window_paths`. Filtrar DEPOIS
        // de agrupar mudaria o numero de cada grupo (e e o que o `tendo` faz,
        // de proposito); numerar ANTES daria numero a linha que a agregacao ia
        // somar.
        //
        // Sem `por` e sem `agregados` nada disto roda, e a resposta e byte a
        // byte a de antes: guarda nova entra PEDIDA, nao imposta.
        if p.campo("por").is_some() || p.campo("agregados").is_some() {
            let (agrupadas, modelo_agrupado) =
                self.agregar_a_composicao(p, linhas, &modelo, apelido_de_fora.as_deref())?;
            linhas = agrupadas;
            modelo = modelo_agrupado;
        }

        for (i, j) in p
            .campo("janela")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let funcao = j
                .texto_ou("funcao", "row_number")
                .trim()
                .to_ascii_lowercase();
            if funcao != "row_number" {
                return Err(PhxError::Esquema(format!(
                    "a janela {i} pede {funcao:?}; nesta rodada so ha row_number"
                )));
            }
            let mut particao: Vec<String> = Vec::new();
            for x in j.campo("particao").and_then(Json::lista).unwrap_or(&[]) {
                if let Some(t) = x.texto() {
                    particao.push(resolver_ou_recusar(&modelo, t, "a particao")?);
                }
            }
            let mut ordem = cs::Criterio::da_lista(j.campo("ordem").and_then(Json::lista));
            for c in ordem.iter_mut() {
                c.coluna = resolver_ou_recusar(&modelo, &c.coluna, "a ordem da janela")?;
            }
            cs::tipar(&mut ordem, &modelo);
            let apelido = match j.texto_ou("apelido", "").trim() {
                "" => "row_number".to_string(),
                outro => outro.to_string(),
            };
            cs::numerar(&mut linhas, &mut modelo, &particao, &ordem, &apelido)?;
        }

        let mut ordem = cs::Criterio::da_lista(p.campo("ordem").and_then(Json::lista));
        if !ordem.is_empty() {
            for c in ordem.iter_mut() {
                // `ORDER BY c.id` sem junção: o `c.` do apelido de fora se
                // descarta antes de resolver `id` na linha (pedido 240).
                let coluna = tirar_prefixo_de_fora(&c.coluna, apelido_de_fora.as_deref());
                c.coluna = resolver_ou_recusar(&modelo, &coluna, "a ordem")?;
            }
            cs::tipar(&mut ordem, &modelo);
            linhas.sort_by(|a, b| cs::comparar_por(a, b, &ordem));
        }

        let achadas = linhas.len() as u64;
        let pular = p.inteiro_ou("pular", 0).max(0) as usize;
        let max = self.limite(p) as usize;
        let recorte: Vec<cs::Linha> = linhas.into_iter().skip(pular).take(max).collect();
        // O recorte DESTE nivel corta pelo mesmo teto, e cortar aqui e tao
        // calado quanto cortar la embaixo. So conta quando nao foi o `max` de
        // quem pediu -- `pular` entra na conta porque o que ele pula nao e
        // corte, e sim a janela que o pedido escolheu.
        let cortado = cortado
            || (achadas > (pular + recorte.len()) as u64
                && o_teto_e_quem_corta(p, self.max_linhas()));

        // A PROJECAO VEM POR ULTIMO, e e por isso que `ordem` pode falar de uma
        // coluna que a resposta nao mostra.
        let pedidas = p.campo("colunas").and_then(Json::lista);
        let (saida, modelo_saida): (Vec<Json>, Modelo) = match pedidas {
            // Sem projecao, sai a linha inteira MENOS as colunas de `escalar`:
            // elas existem para a expressao ver, e nao para engordar a
            // resposta com um numero repetido em toda linha. Quem as quer
            // pede-as pelo nome.
            None => {
                let e_escalar = |n: &str| escalares.iter().any(|e| e.eq_ignore_ascii_case(n));
                (
                    recorte
                        .into_iter()
                        .map(|l| {
                            Json::Objeto(l.into_iter().filter(|(n, _)| !e_escalar(n)).collect())
                        })
                        .collect(),
                    modelo.into_iter().filter(|(n, _)| !e_escalar(n)).collect(),
                )
            }
            Some(l) => {
                let mut escolhas: Vec<(String, String, phxsql_core::types::ColumnType)> =
                    Vec::new();
                for c in l {
                    let (nome, apelido) = match c {
                        Json::Texto(t) => (t.clone(), t.clone()),
                        outro => {
                            let nome = outro.texto_ou("coluna", "").to_string();
                            let apelido = match outro.texto_ou("apelido", "").trim() {
                                "" => nome.clone(),
                                a => a.to_string(),
                            };
                            (nome, apelido)
                        }
                    };
                    if nome.trim().is_empty() {
                        continue;
                    }
                    // O nome REAL para procurar, e o APELIDO para a chave da
                    // resposta: `{"coluna":"c.nome","apelido":"cliente"}` sai
                    // como `cliente`, e sem apelido sai como foi pedido. Sem
                    // junção o `c.` do apelido de fora se descarta ANTES de
                    // procurar (a linha nao foi prefixada); o rotulo da resposta
                    // segue como foi pedido, igual ao caminho com junção (240).
                    let procurar = tirar_prefixo_de_fora(&nome, apelido_de_fora.as_deref());
                    let real = resolver_ou_recusar(&modelo, &procurar, "a projecao")?;
                    let tipo = cs::tipo_no_modelo(&modelo, &real)
                        .unwrap_or(phxsql_core::types::ColumnType::Str(0));
                    escolhas.push((real, apelido, tipo));
                }
                (
                    recorte
                        .iter()
                        .map(|linha| {
                            Json::Objeto(
                                escolhas
                                    .iter()
                                    .map(|(nome, apelido, _)| {
                                        (
                                            apelido.clone(),
                                            cs::campo(linha, nome).cloned().unwrap_or(Json::Nulo),
                                        )
                                    })
                                    .collect(),
                            )
                        })
                        .collect(),
                    escolhas
                        .into_iter()
                        .map(|(_, apelido, tipo)| (apelido, tipo))
                        .collect(),
                )
            }
        };

        Ok(Json::objeto(vec![
            ("devolvidas", Json::de_u64(saida.len() as u64)),
            // `achadas` antes do recorte, como no `varrer`: sem ele quem
            // pagina nao sabe se a pagina e a ultima.
            ("achadas", Json::de_u64(achadas)),
            // Algum sub-pedido -- ou o recorte daqui -- parou no teto, e
            // entao esta resposta fala de um PEDACO. Campo novo: quem nao o
            // le continua recebendo o que recebia (pedido 419).
            ("truncado", Json::Bool(cortado)),
            // A forma da linha, ANTES das linhas: e por ela que um consultar
            // aninhado e o cliente sabem o que vem, mesmo quando nao vem nada.
            ("colunas", cs::modelo_para_json(&modelo_saida)),
            ("linhas", Json::Lista(saida)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// O `GROUP BY` do `consultar`: agrega a linha JA COMPOSTA.
    ///
    /// # Por que ele existe ao lado do `agrupar`, e nao no lugar dele
    ///
    /// O `agrupar` FLUI do disco: nao materializa linha nenhuma, ve a tabela
    /// INTEIRA e poe o teto sobre os GRUPOS. Esta porta compoe: junta, filtra
    /// e so entao agrega, sobre o que a composicao ja tem em memoria, sob o
    /// teto de `max_linhas`. Fazer o `agrupar` virar acucar desta aqui
    /// trocaria um `COUNT(*)` de 1.000.000 por 1.000 -- o teto da composicao --
    /// sem dizer que cortou, e numero errado calado e o que esta casa nao faz.
    /// Os quatro motores maduros tem UMA porta so porque tem planejador que
    /// escolhe o caminho fisico depois; nos nao temos, entao a escolha fisica
    /// fica visivel no contrato em vez de acontecer por engano.
    ///
    /// # O portao nao ganha pergunta nova, e isso e resultado
    ///
    /// `por`, `agregados` e `tendo` nomeiam COLUNA e texto -- nunca tabela. A
    /// tabela continua morando nos sub-pedidos, e cada um paga o portao
    /// inteiro no `executar_derivado`. E o direito por coluna fecha sozinho:
    /// `modelo_da_tabela` nao nomeia a coluna negada, entao agregar sobre ela
    /// cai em `resolver_ou_recusar` e RECUSA nomeando, sem peneira nova.
    fn agregar_a_composicao(
        &self,
        p: &Json,
        linhas: Vec<crate::consultar::Linha>,
        modelo: &crate::consultar::Modelo,
        apelido_de_fora: Option<&str>,
    ) -> Result<(Vec<crate::consultar::Linha>, crate::consultar::Modelo)> {
        // As colunas que entram em CONTA, e so elas. A posicao nesta lista e o
        // indice que `por` e `Agregado::coluna` usam.
        let mut usadas: Vec<(String, ColumnType)> = Vec::new();

        let mut por: Vec<usize> = Vec::new();
        let mut nomes_por: Vec<String> = Vec::new();
        let mut tipos_por: Vec<ColumnType> = Vec::new();
        for c in p.campo("por").and_then(Json::lista).unwrap_or(&[]) {
            let pedida = match c {
                Json::Texto(t) => t.clone(),
                outro => outro.texto_ou("coluna", "").to_string(),
            };
            if pedida.trim().is_empty() {
                return Err(PhxError::Esquema(
                    "cada item de \"por\" e o nome de uma coluna da linha \
                     composta, em texto ou em {\"coluna\": …}"
                        .into(),
                ));
            }
            // Sem junção a linha nao foi prefixada, e `GROUP BY c.cidade` cita
            // o apelido de fora: ele se descarta antes de resolver, como na
            // `ordem` e na projecao ja faziam (pedido 240).
            let procurar = tirar_prefixo_de_fora(&pedida, apelido_de_fora);
            let real = resolver_ou_recusar(modelo, &procurar, "o \"por\"")?;
            let tipo = crate::consultar::tipo_no_modelo(modelo, &real)
                .unwrap_or(phxsql_core::types::ColumnType::Str(0));
            por.push(posicao_na_lista(&mut usadas, &real, tipo));
            nomes_por.push(real);
            tipos_por.push(tipo);
        }

        let mut agregados: Vec<crate::agrupar::Agregado> = Vec::new();
        for a in p.campo("agregados").and_then(Json::lista).unwrap_or(&[]) {
            let funcao = Agregador::de_texto(a.texto_ou("funcao", "contagem"))?;
            let pedida = a.texto_ou("coluna", "").trim().to_string();
            let coluna = match pedida.as_str() {
                "" => {
                    if funcao.precisa_de_valor() {
                        return Err(PhxError::Esquema(format!(
                            "o agregado {:?} precisa de \"coluna\": so a contagem \
                             conta linhas em vez de valores",
                            funcao.nome()
                        )));
                    }
                    None
                }
                c => {
                    let procurar = tirar_prefixo_de_fora(c, apelido_de_fora);
                    let real = resolver_ou_recusar(
                        modelo,
                        &procurar,
                        &format!("o agregado {:?}", funcao.nome()),
                    )?;
                    let tipo = crate::consultar::tipo_no_modelo(modelo, &real)
                        .unwrap_or(phxsql_core::types::ColumnType::Str(0));
                    Some(posicao_na_lista(&mut usadas, &real, tipo))
                }
            };
            let apelido = match a.texto_ou("apelido", "").trim() {
                "" => crate::agrupar::Agregado::apelido_padrao(
                    funcao,
                    (!pedida.is_empty()).then(|| ultimo_segmento(&pedida)),
                ),
                outro => outro.to_string(),
            };
            agregados.push(crate::agrupar::Agregado {
                funcao,
                coluna,
                apelido,
            });
        }
        // Sem `agregados` a resposta seria a lista de valores distintos -- que
        // e um `SELECT DISTINCT`, e nao um `GROUP BY`. Contar e o padrao, como
        // no `agrupar`: dois caminhos que respondem coisas diferentes ao mesmo
        // pedido seriam duas semanticas com o mesmo nome.
        if agregados.is_empty() {
            agregados.push(crate::agrupar::Agregado {
                funcao: Agregador::Contagem,
                coluna: None,
                apelido: "contagem".to_string(),
            });
        }
        recusar_apelidos_repetidos(&nomes_por, &agregados)?;
        let tendo = tendo_do_pedido(p, &nomes_por, &agregados)?;

        let teto_grupos = self.max_linhas();
        let tipos: Vec<ColumnType> = usadas.iter().map(|(_, t)| *t).collect();
        let mut fonte = LinhasDaComposicao {
            linhas: linhas.into_iter(),
            usadas: &usadas,
        };
        let r = crate::agrupar::agrupar(&mut fonte, &tipos, &por, &agregados, teto_grupos)?;
        let campos = linhas_dos_grupos(&r, &nomes_por, &tipos_por, &agregados, tendo.as_ref())?;

        // O modelo e SUBSTITUIDO, e nao acrescentado: depois de agrupar so
        // existem as colunas de `por` e os apelidos. E o que faz a coluna nao
        // agregada RECUSAR na projecao, por `resolver_ou_recusar`, em vez de
        // sair com o valor de uma linha qualquer do grupo -- a decisao que a
        // media ponderada desta casa ja tomou (PG 4 + MySQL 2 = 6 contra
        // MariaDB 3 + SQLite 1 = 4, em `docs/propostas/semantica-4-motores.md`).
        let modelo_novo: crate::consultar::Modelo = nomes_por
            .iter()
            .cloned()
            .zip(tipos_por.iter().copied())
            .chain(agregados.iter().map(|a| {
                let (decimal, escala) =
                    crate::pivot::decimal_e_escala(a.coluna.map(|c| &usadas[c].1));
                (
                    a.apelido.clone(),
                    crate::pivot::tipo_do_agregado(a.funcao, decimal, escala),
                )
            }))
            .collect();
        let saida: Vec<crate::consultar::Linha> = campos
            .into_iter()
            .map(|campos| {
                campos
                    .into_iter()
                    .map(|(n, v, ty)| (n, crate::valores::valor_para_json(&v, &ty)))
                    .collect()
            })
            .collect();
        Ok((saida, modelo_novo))
    }

    /// `agrupar`: o `GROUP BY` generico -- agrupa por colunas e resume cada
    /// grupo.
    ///
    /// # Por que ela nao e um `pivotar` com outra roupa
    ///
    /// O `pivotar` cruza DUAS listas de campos numa grade e resume UMA coluna.
    /// Isto aqui resume VARIAS colunas de uma vez, com apelido para cada uma, e
    /// peneira o resultado ja agregado (`tendo`). Sao perguntas diferentes com
    /// o mesmo acumulador embaixo -- e e o acumulador que e compartilhado, e
    /// nao copiado: ver `crate::agrupar`.
    ///
    /// # O portao continua sendo UM
    ///
    /// Esta operacao nomeia UMA tabela, no campo `"tabela"` -- o mesmo que o
    /// `despachar` ja confere. Nao ha conferencia propria aqui porque nao ha
    /// segunda tabela escondida em lugar nenhum, ao contrario do `juntar`, do
    /// `unir` e do `pivotar`. **No dia em que o `agrupar` ganhar um `de`
    /// aninhado, ele passa a precisar de uma** -- e a pergunta que decide e
    /// «esta operacao nomeia tabela onde o portao nao olha?».
    pub(super) fn op_agrupar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p);
        let teto_grupos = self.max_linhas();
        let comeco = Instant::now();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut por = Vec::new();
        for c in p.campo("por").and_then(Json::lista).unwrap_or(&[]) {
            por.push(coluna_de(c, &esquema)?);
        }
        let nomes_por: Vec<String> = por
            .iter()
            .map(|i| esquema.colunas()[*i].nome.clone())
            .collect();

        // Sem `agregados` a resposta seria a lista de valores distintos --
        // que e um `SELECT DISTINCT`, e nao um `GROUP BY`. Contar e o padrao
        // porque e o que quem esquece o campo quase sempre queria.
        let pedidos = p.campo("agregados").and_then(Json::lista);
        let mut agregados: Vec<crate::agrupar::Agregado> = Vec::new();
        for a in pedidos.unwrap_or(&[]) {
            let funcao = Agregador::de_texto(a.texto_ou("funcao", "contagem"))?;
            let nome_coluna = a.texto_ou("coluna", "").trim().to_string();
            let coluna = match nome_coluna.as_str() {
                "" => {
                    if funcao.precisa_de_valor() {
                        return Err(PhxError::Esquema(format!(
                            "o agregado {:?} precisa de \"coluna\": so a contagem \
                             conta linhas em vez de valores",
                            funcao.nome()
                        )));
                    }
                    None
                }
                c => Some(posicao_da_coluna(&esquema, c).ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "o agregado {:?} usa a coluna {c:?}, que nao existe em {}",
                        funcao.nome(),
                        esquema.nome()
                    ))
                })?),
            };
            let apelido = match a.texto_ou("apelido", "").trim() {
                "" => crate::agrupar::Agregado::apelido_padrao(
                    funcao,
                    (!nome_coluna.is_empty()).then_some(nome_coluna.as_str()),
                ),
                outro => outro.to_string(),
            };
            agregados.push(crate::agrupar::Agregado {
                funcao,
                coluna,
                apelido,
            });
        }
        if agregados.is_empty() {
            agregados.push(crate::agrupar::Agregado {
                funcao: Agregador::Contagem,
                coluna: None,
                apelido: "contagem".to_string(),
            });
        }
        // Apelido repetido recusa AQUI, e nao vira a segunda chave de um
        // objeto JSON. A conferencia mora fora porque o `consultar` que agrega
        // recusa a MESMA coisa.
        recusar_apelidos_repetidos(&nomes_por, &agregados)?;

        // O MODELO da resposta, montado ANTES de existir grupo: e o que diz a
        // forma da linha a um `consultar` que receba zero grupos, e sai do
        // mesmo `tipo_do_agregado` que tipa cada valor -- um lugar, nao dois.
        let cabecalho: crate::consultar::Modelo = nomes_por
            .iter()
            .zip(por.iter())
            .map(|(n, c)| (n.clone(), esquema.colunas()[*c].ty))
            .chain(agregados.iter().map(|a| {
                let (decimal, escala) =
                    crate::pivot::decimal_e_escala(a.coluna.map(|c| &esquema.colunas()[c].ty));
                (
                    a.apelido.clone(),
                    crate::pivot::tipo_do_agregado(a.funcao, decimal, escala),
                )
            }))
            .collect();

        // A peneira da linha CRUA, no mesmo lugar unico do `varrer`.
        let onde = filtros_do_pedido(p, &esquema)?;
        let expressao = expressao_do_pedido(p, &esquema)?;

        // O `tendo` fala da linha AGREGADA: conferido contra `por` e os
        // apelidos, pela MESMA funcao que o `consultar` que agrega usa.
        let tendo = tendo_do_pedido(p, &nomes_por, &agregados)?;

        let rowids: Vec<u64> = t.varrer()?.into_iter().map(|(r, _)| r).collect();
        let mut fonte = LinhasPeneiradas {
            rowids: rowids.into_iter(),
            tabela: &mut t,
            onde,
            expressao,
            esquema: &esquema,
            examinadas: 0,
        };
        let tipos: Vec<ColumnType> = esquema.colunas().iter().map(|c| c.ty).collect();
        let r = crate::agrupar::agrupar(&mut fonte, &tipos, &por, &agregados, teto_grupos)?;
        let examinadas = fonte.examinadas;

        // A linha da resposta, montada uma vez e usada pelo `tendo`, pela
        // `ordem` e pela saida -- porque as tres falam dos MESMOS nomes. Pela
        // mesma funcao do `consultar` que agrega.
        let tipos_por: Vec<ColumnType> = por.iter().map(|c| esquema.colunas()[*c].ty).collect();
        let mut linhas = linhas_dos_grupos(&r, &nomes_por, &tipos_por, &agregados, tendo.as_ref())?;

        if let Some(l) = p.campo("ordem").and_then(Json::lista) {
            let mut chaves: Vec<(usize, bool)> = Vec::new();
            for o in l {
                let nome = match o {
                    Json::Texto(t) => t.as_str(),
                    outro => outro.texto_ou("coluna", ""),
                };
                let i = linhas
                    .first()
                    .and_then(|c| c.iter().position(|(n, _, _)| n.eq_ignore_ascii_case(nome)));
                // Sem grupo nenhum nao ha o que ordenar, e recusar ali seria
                // recusar por causa de uma tabela vazia.
                let Some(i) = i else {
                    if linhas.is_empty() {
                        break;
                    }
                    return Err(PhxError::Esquema(format!(
                        "a ordem pede {nome:?}, que nao e coluna de \"por\" nem \
                         apelido de agregado"
                    )));
                };
                chaves.push((i, o.booleano_ou("desc", false)));
            }
            linhas.sort_by(|a, b| {
                for (i, desc) in &chaves {
                    let ord = phxsql_store::memoria::comparar(&a[*i].1, &b[*i].1);
                    let ord = if *desc { ord.reverse() } else { ord };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        let grupos = linhas.len() as u64;
        let truncado = grupos > max;
        let saida: Vec<Json> = linhas
            .iter()
            .take(max as usize)
            .map(|campos| {
                Json::Objeto(
                    campos
                        .iter()
                        .map(|(n, v, ty)| (n.clone(), crate::valores::valor_para_json(v, ty)))
                        .collect(),
                )
            })
            .collect();

        // A TRILHA, pelo mesmo motivo do `varrer`: os rotulos dos grupos SAO
        // os valores da coluna agrupada. Agrupar por `cpf` e ler os CPFs, e
        // uma trilha que nao registrasse isso deixaria de responder «quem leu
        // o dado pessoal?» justamente por onde ele sai resumido.
        let devolvidas = saida.len() as u64;
        Self::trilhar_acesso(&mut t, 0, devolvidas, || {
            format!(
                "agrupar por={} agregados={}",
                nomes_por.join(","),
                agregados.len()
            )
        })?;

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            // `grupos` conta o que sobrou DEPOIS do `tendo`, e nao antes: e o
            // tamanho do resultado, que e o que quem pagina precisa saber.
            ("grupos", Json::de_u64(grupos)),
            ("devolvidas", Json::de_u64(devolvidas)),
            // As duas do `varrer`, e pela mesma razao: sem elas a tela nao
            // sabe se o numero que mostra e da tabela ou da pagina.
            ("examinadas", Json::de_u64(examinadas)),
            ("consideradas", Json::de_u64(r.lidas)),
            ("truncado", Json::Bool(truncado)),
            // A forma da linha, para quem compoe: nome e tipo de cada coluna
            // de `por` e de cada agregado, na ordem em que saem.
            ("colunas", crate::consultar::modelo_para_json(&cabecalho)),
            ("linhas", Json::Lista(saida)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }
}

/// Os indices da tabela, como o tradutor de SQL os espera.
///
/// Saem da resposta do `esquema` -- campo por campo, e nao de uma leitura
/// propria. E a mesma decisao do resto da op `sql`: quem abre tabela e o motor.
pub(super) fn indices_do_esquema(esquema: &Json) -> Vec<phxsql_sql::IndiceInfo> {
    esquema
        .campo("indices")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|i| phxsql_sql::IndiceInfo {
            nome: i.texto_ou("nome", "").to_string(),
            colunas: i
                .campo("colunas")
                .and_then(Json::lista)
                .unwrap_or(&[])
                .iter()
                .map(|c| phxsql_sql::ColunaDoIndice {
                    nome: c.texto_ou("coluna", "").to_string(),
                    desc: c.booleano_ou("desc", false),
                })
                .collect(),
            unico: i.booleano_ou("unico", false),
            primario: i.booleano_ou("primario", false),
        })
        .collect()
}

/// Fica com as colunas pedidas, nesta ordem, com estes rotulos.
///
/// Coluna que o `SELECT` pediu e a linha nao tem vira `null`, e nao some: uma
/// chave ausente faria a resposta ter forma diferente linha a linha, e quem le
/// por posicao quebraria na primeira.
pub(super) fn projetar(linha: &Json, colunas: &[(String, String)]) -> Json {
    Json::Objeto(
        colunas
            .iter()
            .map(|(nome, rotulo)| {
                (
                    rotulo.clone(),
                    linha.campo(nome).cloned().unwrap_or(Json::Nulo),
                )
            })
            .collect(),
    )
}

/// Quem cortou este pedido: o teto do servidor, ou o `max` de quem pediu?
///
/// Separa o truncamento (o teto decidiu, e ninguem foi avisado) do `LIMIT` (o
/// pedido escolheu, e recebeu o que escolheu). Sem `max`, ou com `max` acima
/// do teto, quem corta e o teto. Vive fora do `impl` porque `op_consultar` e
/// `parou_no_teto` fazem a MESMA pergunta sobre pedidos de niveis diferentes.
fn o_teto_e_quem_corta(p: &Json, teto: u64) -> bool {
    let pedido = p.inteiro_ou("max", 0).max(0) as u64;
    pedido == 0 || pedido > teto
}

/// O teto de aninhamento do `consultar`, contado POR THREAD.
///
/// Um `consultar` cujo `de` e outro `consultar` desce a pilha de verdade, e
/// pilha estourada nao e recusa: e a thread caindo, que numa conexao vira
/// resposta nenhuma e nenhum recado. O contador vive num `Cell` de thread
/// porque e exatamente isso que ele mede -- a profundidade DESTA chamada --,
/// e o guarda o devolve no `Drop`, inclusive quando o passo do meio falha.
///
/// Oito e fundo de sobra para consulta escrita por gente ou por tradutor de
/// SQL, e raso o bastante para nao chegar perto do limite da pilha.
const TETO_ANINHAMENTO: u32 = 8;

thread_local! {
    static FUNDO: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub(super) struct Profundidade;

impl Profundidade {
    pub(super) fn descer() -> Result<Profundidade> {
        let n = FUNDO.with(|f| {
            let n = f.get() + 1;
            f.set(n);
            n
        });
        if n > TETO_ANINHAMENTO {
            // O guarda ja subiu o contador, e o `Drop` nao roda para um valor
            // que nunca existiu -- entao a descida se desfaz aqui.
            FUNDO.with(|f| f.set(f.get() - 1));
            return Err(PhxError::LimiteExcedido(format!(
                "o consultar passou de {TETO_ANINHAMENTO} niveis de aninhamento; \
                 alem disso a pilha da thread nao aguenta, e cair nao e recusar"
            )));
        }
        Ok(Profundidade)
    }
}

impl Drop for Profundidade {
    fn drop(&mut self) {
        FUNDO.with(|f| f.set(f.get().saturating_sub(1)));
    }
}

/// A recusa de uma visao cuja tabela sumiu -- com o nome das DUAS.
///
/// Sem isto, quem consulta `v_clientes` recebe «clientes nao existe» e vai
/// procurar quem apagou uma tabela que ele nem citou. **Envolver nao e
/// substituir**: o erro de dentro continua inteiro no fim da frase, porque e
/// ele que diz se a tabela sumiu ou se foi a permissao que faltou.
fn erro_da_visao(visao: &str, tabela: &str, e: PhxError) -> PhxError {
    let recado = format!("a visao {visao:?} le a tabela {tabela:?}, e ela nao respondeu: {e}");
    match e {
        PhxError::Autorizacao(_) => PhxError::Autorizacao(recado),
        _ => PhxError::NaoEncontrado(recado),
    }
}

/// O apelido de um lado do `consultar`: o que veio escrito, ou o nome da
/// tabela do sub-pedido.
///
/// O padrao existe para o caso comum nao ter de escrever nada: `FROM pedidos p
/// JOIN clientes c` sem apelido nenhum vira `pedidos.id` e `clientes.id`, que
/// ja resolve a ambiguidade. Quando o sub-pedido nao nomeia tabela -- um
/// `consultar` aninhado, por exemplo --, nao ha padrao possivel e o apelido
/// passa a ser obrigatorio, com a recusa dizendo isso.
fn apelido_do_lado(dono: &Json, sub: &Json, rotulo: &str) -> Result<String> {
    let ap = dono.texto_ou("apelido", "").trim();
    if !ap.is_empty() {
        return Ok(ap.to_string());
    }
    // `schema.tabela` vira `tabela`, como o prefixo do `juntar` ja fazia:
    // um ponto no meio do apelido faria `matriz.estoque.id` ter dois.
    let tabela = sub.texto_ou("tabela", "").trim();
    let curto = tabela.rsplit('.').next().unwrap_or("").trim();
    if curto.is_empty() {
        return Err(PhxError::Esquema(format!(
            "{rotulo} nao nomeia tabela, entao ele precisa de \"apelido\": e o \
             prefixo com que as colunas dele aparecem depois da junção"
        )));
    }
    Ok(curto.to_string())
}

/// Tira o prefixo do apelido de FORA de um nome, quando ha um e ele casa --
/// `c.id` vira `id`. Um nome sem esse prefixo (ou sem ponto nenhum) passa
/// inteiro. `apelido_de_fora` e `None` quando ha junção: ali o prefixo diz de
/// qual lado a coluna vem, e descarta-lo seria perder essa informacao. Pedido
/// 240: sem junção a linha de fora nao ganha prefixo, entao o `c.` que quem
/// escreveu poe (num `EXISTS`, numa projecao, numa ordem, num `IN`) e ruido que
/// se descarta antes de resolver a coluna na linha.
fn tirar_prefixo_de_fora(nome: &str, apelido_de_fora: Option<&str>) -> String {
    match (apelido_de_fora, nome.split_once('.')) {
        (Some(ap), Some((prefixo, resto))) if prefixo.eq_ignore_ascii_case(ap) => resto.to_string(),
        _ => nome.to_string(),
    }
}

/// Resolve um nome contra o MODELO do lado, ou RECUSA nomeando.
///
/// Modelo vazio quer dizer «nao sei a forma» -- um sub-pedido que nao
/// devolveu linha nem cabecalho --, e ai o nome segue como veio: recusar
/// sobre o que nao se conhece seria recusar por causa de uma tabela vazia.
/// Com modelo, a resolucao vale tambem sobre resultado vazio, e a coluna que
/// nao existe recusa nomeando as que ha.
pub(super) fn resolver_ou_recusar(
    modelo: &crate::consultar::Modelo,
    nome: &str,
    onde: &str,
) -> Result<String> {
    if modelo.is_empty() {
        return Ok(nome.to_string());
    }
    match crate::consultar::resolver(modelo, nome)? {
        Some(real) => Ok(real),
        None => Err(PhxError::Esquema(format!(
            "{onde} pede {nome:?}, que nao e coluna do resultado. As que ha: {}",
            modelo
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Apelido de agregado que colide RECUSA, nos dois caminhos que agregam.
///
/// Dois apelidos iguais -- ou um apelido com o nome de uma coluna de `por` --
/// virariam duas chaves do MESMO objeto JSON, e quem le a resposta veria uma
/// das duas sem saber qual: numero errado calado, que e o que esta casa nao
/// faz. Mora aqui, e nao dentro de cada operacao, porque `agrupar` e
/// `consultar` tem de recusar a MESMA coisa -- uma checagem em cada lugar
/// divergiria na primeira vez que alguem mexesse numa so.
fn recusar_apelidos_repetidos(
    nomes_por: &[String],
    agregados: &[crate::agrupar::Agregado],
) -> Result<()> {
    for (i, a) in agregados.iter().enumerate() {
        if nomes_por.iter().any(|n| n.eq_ignore_ascii_case(&a.apelido)) {
            return Err(PhxError::Esquema(format!(
                "o apelido {:?} colide com a coluna de agrupamento de mesmo \
                 nome; de outro apelido ao agregado",
                a.apelido
            )));
        }
        if agregados[..i]
            .iter()
            .any(|b| b.apelido.eq_ignore_ascii_case(&a.apelido))
        {
            return Err(PhxError::Esquema(format!(
                "dois agregados usam o apelido {:?}; um deles ficaria \
                 invisivel na resposta",
                a.apelido
            )));
        }
    }
    Ok(())
}

/// O `tendo` (o `HAVING`) de um pedido que agrega.
///
/// Ele fala da linha AGREGADA, entao e conferido contra os nomes que existem
/// DEPOIS de agrupar -- as colunas de `por` e os apelidos --, e NUNCA contra o
/// esquema: conferir contra o esquema aceitaria `preco > 10`, que nao quer
/// dizer nada num grupo de mil linhas.
///
/// Os nomes sao os da RESPOSTA, letra por letra. Depois de uma junção a
/// coluna agrupada se chama `c.cidade`, e e assim que o `tendo` a escreve --
/// a recusa lista o que ha, entao quem escreveu `cidade` ve a diferenca na
/// primeira corrida em vez de receber um filtro que nao filtrou.
pub(super) fn tendo_do_pedido(
    p: &Json,
    nomes_por: &[String],
    agregados: &[crate::agrupar::Agregado],
) -> Result<Option<Expressao>> {
    let txt = match p.campo("tendo").and_then(Json::texto) {
        Some(t) if !t.trim().is_empty() => t,
        _ => return Ok(None),
    };
    // Pela porta do observador de injecao (495, F3).
    let e = crate::injecao::analisar_expressao(txt)?;
    for nome in e.colunas() {
        let conhecido = nomes_por.iter().any(|n| n.eq_ignore_ascii_case(nome))
            || agregados
                .iter()
                .any(|a| a.apelido.eq_ignore_ascii_case(nome));
        if !conhecido {
            return Err(PhxError::Esquema(format!(
                "o \"tendo\" usa {nome:?}, que nao e coluna de \"por\" \
                 nem apelido de agregado. O que ele pode ver e: {}",
                nomes_por
                    .iter()
                    .cloned()
                    .chain(agregados.iter().map(|a| a.apelido.clone()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    Ok(Some(e))
}

/// Os grupos virados LINHA, com o `tendo` ja aplicado.
///
/// A linha e montada UMA vez e serve ao `tendo`, a `ordem` e a saida, porque
/// as tres falam dos MESMOS nomes. Ela sai em `(nome, valor, tipo)` e nao em
/// JSON porque o `tendo` avalia uma expressao sobre o VALOR: comparar `n > 1`
/// com o texto de `n` responderia outra pergunta.
fn linhas_dos_grupos(
    r: &crate::agrupar::Resultado,
    nomes_por: &[String],
    tipos_por: &[ColumnType],
    agregados: &[crate::agrupar::Agregado],
    tendo: Option<&Expressao>,
) -> Result<Vec<Vec<(String, Value, ColumnType)>>> {
    let mut linhas = Vec::with_capacity(r.grupos.len());
    for g in &r.grupos {
        let mut campos = Vec::with_capacity(nomes_por.len() + agregados.len());
        for (i, nome) in nomes_por.iter().enumerate() {
            campos.push((
                nome.clone(),
                g.chave[i].clone(),
                tipos_por.get(i).copied().unwrap_or(ColumnType::Int8),
            ));
        }
        for (a, (v, ty)) in agregados.iter().zip(g.valores.iter()) {
            campos.push((a.apelido.clone(), v.clone(), *ty));
        }
        if let Some(e) = tendo {
            let passa = e.avaliar_bool(&|nome| {
                campos
                    .iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(nome))
                    .map(|(_, v, ty)| (v, ty))
            })?;
            // `NULL` exclui, como em todo filtro deste motor -- e como no
            // HAVING do SQL. No CHECK ele passa, e a diferenca esta escrita
            // no `phxsql_core::expressao`.
            if passa != Some(true) {
                continue;
            }
        }
        linhas.push(campos);
    }
    Ok(linhas)
}

/// A posicao de uma coluna na lista das que a agregacao usa, criando-a se
/// ainda nao estiver la.
///
/// E ela que vira o indice de `por` e de `Agregado::coluna`. Sair de uma lista
/// PROPRIA, e nao do modelo, e o que faz uma linha de quarenta colunas
/// agrupada por duas converter duas celulas por linha em vez de quarenta.
fn posicao_na_lista(usadas: &mut Vec<(String, ColumnType)>, real: &str, tipo: ColumnType) -> usize {
    match usadas
        .iter()
        .position(|(n, _)| n.eq_ignore_ascii_case(real))
    {
        Some(i) => i,
        None => {
            usadas.push((real.to_string(), tipo));
            usadas.len() - 1
        }
    }
}

/// O ultimo segmento de um nome qualificado: `p.total` -> `total`.
///
/// So serve ao apelido PADRAO de um agregado, e o motivo e o resolvedor de
/// nomes: `soma_p.total` tem um ponto, e `consultar::resolver` le ponto como
/// qualificacao -- entao pedir `total` acharia `p.total` E `soma_p.total` e
/// chamaria de ambiguo um nome que o usuario nem escreveu. Com o sufixo, dois
/// agregados sobre `p.total` e `c.total` colidem em `soma_total` e a colisao
/// RECUSA nomeando, que e a resposta certa em vez de um nome torto.
fn ultimo_segmento(nome: &str) -> &str {
    nome.rsplit_once('.').map_or(nome, |(_, s)| s)
}

/// As linhas da COMPOSICAO como o agregador as consome: so as colunas que
/// entram em conta, convertidas pelo TIPO do modelo.
///
/// Pelo MESMO `valor_tipado` da `expressao`, e nao por um segundo conversor:
/// um `Decimal` viaja como texto no JSON, e somar o texto daria um numero
/// diferente do que o filtro comparou -- os dois pareceriam certos.
struct LinhasDaComposicao<'a> {
    linhas: std::vec::IntoIter<crate::consultar::Linha>,
    usadas: &'a [(String, ColumnType)],
}

impl crate::pivot::Iterador for LinhasDaComposicao<'_> {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        Ok(self.linhas.next().map(|l| {
            self.usadas
                .iter()
                .map(|(nome, tipo)| {
                    // Coluna que a linha nao tem e NULA, e nao estouro: e o
                    // caso do lado esquerdo que nao casou na junção.
                    crate::consultar::campo(&l, nome)
                        .map_or(Value::Null, |j| crate::consultar::valor_tipado(j, tipo).0)
                })
                .collect()
        }))
    }
}

/// As linhas de uma tabela ja PENEIRADAS -- o `onde` e a `expressao` decididos
/// pelo mesmo `memoria::passa` do `varrer`.
///
/// Existe para o `agrupar` receber so o que passa, e para o filtro do
/// `agrupar` ser, letra por letra, o filtro do `varrer`. Escrever a peneira
/// aqui de novo faria `WHERE cidade = 'X'` significar uma coisa numa operacao
/// e outra na irma -- e a divergencia so apareceria quando alguem comparasse
/// a soma com a lista.
struct LinhasPeneiradas<'a> {
    rowids: std::vec::IntoIter<u64>,
    tabela: &'a mut Table,
    onde: Vec<Filtro>,
    expressao: Option<phxsql_core::expressao::Expressao>,
    esquema: &'a Schema,
    /// Quantas linhas foram OLHADAS -- o irmao do `examinadas` do `varrer`.
    examinadas: u64,
}

impl crate::pivot::Iterador for LinhasPeneiradas<'_> {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        for rowid in self.rowids.by_ref() {
            let Some(l) = self.tabela.ler(rowid)? else {
                continue;
            };
            self.examinadas += 1;
            if phxsql_store::memoria::passa(
                &l,
                &self.onde,
                None,
                self.expressao.as_ref(),
                self.esquema,
            )? {
                return Ok(Some(l));
            }
        }
        Ok(None)
    }
}

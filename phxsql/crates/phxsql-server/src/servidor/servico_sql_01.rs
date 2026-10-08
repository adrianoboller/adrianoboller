//! SQL, rotinas e gatilhos, com o `MotorDoServidor`.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Quanto tempo o corpo de um gatilho `BEFORE` pode segurar a trava GLOBAL de
/// dados.
///
/// # O teto que faltava, e o que o teto que existia nao segurava
///
/// O avaliador ja tinha `PASSOS_MAX` — um milhao de passos por corpo — e o
/// comentario dele nomeia esta razao exata. Medido, um milhao de passos de
/// aritmetica custa 22,5 ms, e esse numero virou a resposta de que «as cinco
/// secoes que rodam codigo do dono tem teto».
///
/// **Elas nao tinham.** Teto de PASSOS nao e teto de TRABALHO: `SET s =
/// CONCAT(s, s)` dobra o texto a cada volta, e o corpo nao morre no teto de
/// passos — morre no alocador, aos ~30 passos de um orcamento de um milhao.
/// Medido com o processo limitado a 2 GiB de espaco: **10,2 s** com a trava
/// global na mao, e entao `memory allocation failed` — que em Rust ABORTA o
/// processo. O servidor inteiro, derrubado por um gatilho.
///
/// # Por que 500 ms, e nao um numero de gosto
///
/// Sai de dentro desta casa: `transacao_lock_timeout_ms` ja vale 500, e ele e
/// a resposta que este servidor ja deu para «quanto uma conexao pode fazer
/// outra esperar». Usar o mesmo numero mantem uma resposta so.
///
/// E ele nao pode quebrar corpo nenhum que hoje funcione: o corpo honesto
/// medido custa **1 us**, e o pior corpo que ainda TERMINA e o que gasta o
/// `PASSOS_MAX` inteiro, em 22,5 ms — 22x de folga. Quem passa de meio segundo
/// aqui esta fazendo o que este teto existe para impedir.
const PRAZO_DO_GATILHO_ANTES: Duration = Duration::from_millis(500);

impl Servidor {
    /// Nome de quem tem este id no cadastro. Vazio quando ninguem tem.
    ///
    /// O `.log`, o `.trash` e o `.reason` guardam o id numerico, e nao o nome:
    /// o id nao muda quando alguem e renomeado, e uma exclusao de 2019 tem de
    /// continuar apontando para a mesma pessoa. Traduzir na hora de MOSTRAR e
    /// o que faz o registro ser legivel sem prender o arquivo ao cadastro.
    pub(super) fn nome_do_usuario(&self, id: u32) -> String {
        if id == 0 {
            return String::new();
        }
        let cadastro = self.cadastro();
        cadastro
            .root
            .iter()
            .chain(cadastro.usuarios.iter())
            .find(|u| u.id == id)
            .map(|u| u.nome.clone())
            .unwrap_or_default()
    }

    /// `sql`: um `SELECT` simples traduzido para as operacoes que ja existem.
    ///
    /// # O portao continua sendo UM
    ///
    /// Esta operacao nao le tabela nenhuma por conta propria. Ela faz duas
    /// coisas, e as duas pelo `executar_derivado`, que e o mesmo portao do
    /// pedido que chega pela rede:
    ///
    /// 1. pede o `esquema` da tabela do `FROM` -- que ja exige `ler` naquela
    ///    tabela --, e e de la que saem os indices que o tradutor precisa;
    /// 2. executa o `varrer` ou o `buscar` que a traducao produziu.
    ///
    /// O campo `tabela` do pedido TRADUZIDO e o que o portao confere. Por isso
    /// `SELECT * FROM folha` de quem nao pode ler a folha para no passo 1, com
    /// o mesmo erro de um `{"op":"varrer","tabela":"folha"}` -- e nao ha
    /// conferencia propria aqui que alguem possa esquecer de atualizar.
    ///
    /// # Por que o esquema vem pelo protocolo, e nao de um `abrir_travada`
    ///
    /// Porque abrir a tabela aqui seria o SEGUNDO caminho ate o dado, e o
    /// segundo caminho e sempre o que esquece uma conferencia. Custa um
    /// `esquema` a mais por consulta; a alternativa custa uma porta dos fundos.
    pub(super) fn op_sql(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let texto = p
            .texto_ou("texto", p.texto_ou("sql", ""))
            .trim()
            .to_string();
        if texto.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"texto\" com o comando SQL".into(),
            ));
        }
        // CREATE/ALTER/DROP USER, antes de tudo: `rotina::comando` reclama
        // todo CREATE e todo DROP para si, e `CREATE USER` chegando la vira
        // erro de sintaxe pedindo TRIGGER. Uma LINHA de despacho, de
        // proposito -- o corpo mora em `sql_de_cadastro`, e assim outra
        // frente mexendo nesta funcao nao esbarra nele.
        if let Some(r) = self.sql_de_cadastro(&texto, sessao) {
            return r;
        }
        // Os comandos de TRANSACAO vem PRIMEIRO, e a ordem foi corrigida por um
        // teste: o detector de rotina analisa o texto inteiro pelo lexico
        // comum, e o lexico recusa `500ms` -- numero colado em identificador.
        // Ele erra com `?` antes de o detector de transacao ser consultado, e
        // um `LOCK TIMEOUT 500ms` nunca chegaria aqui.
        //
        // Inverter e seguro, e nao por sorte: o unico `BEGIN` que NAO abre
        // transacao e o do corpo de um `CREATE PROCEDURE p() BEGIN … END`, e
        // esse texto comeca por `CREATE`. O detector olha a PRIMEIRA palavra,
        // entao ele nunca rouba um `CREATE` de ninguem -- e ha teste para os
        // dois lados, no proprio `phxsql_sql::transacao`.
        //
        // Eles nao passam pelo tradutor de SELECT porque nao sao consulta:
        // nao tem tabela, nao produzem linha e nao dependem de esquema
        // nenhum. Sao comandos de SESSAO, como o BULKINSERT.
        // As DIRETIVAS vem antes do detector de rotina, e a ordem importa: o
        // `rotina::comando` atende `SHOW` e recusa o que nao for TRIGGERS ou
        // PROCEDURES -- entao um `SHOW SERVER SETTINGS` chegando la voltaria
        // com «SHOW nesta camada lista TRIGGERS ou PROCEDURES», que manda quem
        // digitou procurar no lugar errado. O detector de diretiva so reclama
        // a frase quando a palavra depois do SHOW e SERVER, DATABASE, TABLE ou
        // CONNECTION, e nenhuma delas o outro atendia.
        if let Some(c) = phxsql_sql::diretiva::comando(&texto)? {
            let mut pedido = c.pedido();
            // O `database` do pedido de fora viaja junto: `SHOW TABLE clientes
            // SETTINGS` nao repete a base que a sessao ja disse.
            if let Json::Objeto(pares) = &mut pedido {
                if !pares.iter().any(|(k, _)| k == "database") {
                    pares.push((
                        "database".to_string(),
                        Json::texto_de(p.texto_ou("database", "")),
                    ));
                }
            }
            // A migracao da cifra (pedido 268) e OPERACAO sobre a tabela, nao
            // diretiva: a op `sql` so exige `ler`, entao ela entra pelos
            // MESMOS portoes que o `despachar` (permissao por tabela,
            // somente-leitura, politica) -- o `executar_derivado`, que e o
            // irmao feito para isto. As diretivas de sempre conferem por
            // dentro (`exigir_administrar_config`) e seguem como estavam.
            let bruto = if c.e_migracao_da_cifra() {
                self.executar_derivado(&c.op, &pedido, sessao)?
            } else {
                self.executar(&c.op, &pedido, sessao)?
            };
            return Ok(Json::objeto(vec![
                ("sql", sql_de_volta(&texto)),
                ("op", Json::texto_de(&c.op)),
                ("resultado", bruto),
            ]));
        }

        if let Some(c) = phxsql_sql::transacao::comando(&texto)? {
            let mut pedido = c.pedido();
            // O `database` do pedido de fora viaja junto: quem manda
            // `{"op":"sql","database":"loja","texto":"BEGIN"}` nao precisa
            // repetir a base na proxima operacao.
            if let Json::Objeto(pares) = &mut pedido {
                pares.push((
                    "database".to_string(),
                    Json::texto_de(p.texto_ou("database", "")),
                ));
            }
            let bruto = self.executar(&c.op, &pedido, sessao)?;
            return Ok(Json::objeto(vec![
                ("sql", sql_de_volta(&texto)),
                ("op", Json::texto_de(&c.op)),
                ("resultado", bruto),
            ]));
        }

        // CREATE TRIGGER/PROCEDURE, DROP, CALL e SHOW entram pela MESMA op:
        // sao SQL, e um driver os manda pelo mesmo campo.
        let parametros: Vec<Json> = p
            .campo("parametros")
            .and_then(Json::lista)
            .map(<[Json]>::to_vec)
            .unwrap_or_default();
        if let Some(comando) = phxsql_sql::rotina::comando_com(&texto, &parametros)? {
            return self.executar_rotina(comando, p, sessao);
        }

        // OS PARAMETROS, no LEXICO e nunca por substituicao de texto.
        //
        // `analisar_comando_com` troca cada `?` pelo literal da posicao DEPOIS
        // de o texto virar simbolos: um `'; DROP TABLE x --` chega como um
        // literal de texto e sai como um literal de texto. Substituir no texto
        // antes de analisar seria a definicao de injecao de SQL, e por isso
        // nao existe caminho nenhum por aqui que faca isso.
        // O erro de sintaxe ja vem com a coluna: «SQL, coluna 14: esperava
        // FROM». Reembalar aqui perderia a posicao, que e a unica parte da
        // mensagem que diz ONDE consertar.
        let comando = if parametros.is_empty() {
            phxsql_sql::analisar_comando(&texto)?
        } else {
            phxsql_sql::analisar_comando_com(&texto, &parametros)?
        };
        let selecao = match comando {
            phxsql_sql::Comando::Selecao(s) => s,
            // O SELECT COMPOSTO -- `WITH`, subconsulta, `IN (SELECT ...)`,
            // junção, janela -- vira a op `consultar`, e cada pedaco dele sai
            // pelo `executar_derivado`. E o que faz da composicao uma
            // composicao, e nao uma porta dos fundos.
            phxsql_sql::Comando::Consulta(c) => {
                let base = p.texto_ou("database", "").trim().to_string();
                let plano = self.planejar_consulta(&c, &base, sessao)?;
                let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;
                return Ok(resposta_do_sql(&texto, &plano, bruto));
            }
            // `SELECT ... UNION [ALL] SELECT ...` vira a op `unir`, e ela sai
            // pelo `executar_derivado` como qualquer outra. **Nao ha
            // conferencia de permissao AQUI de proposito**: o `op_unir` ja
            // confere tabela por tabela, porque o campo `"tabela"` que o
            // portao geral le nao existe num pedido de uniao. Repetir a
            // conferencia neste ponto criaria a duplicata que um dia alguem
            // limpa -- e o que ficaria aberto seria a porta dos fundos.
            phxsql_sql::Comando::Uniao(u) => {
                let base = p.texto_ou("database", "").trim().to_string();
                let plano = phxsql_sql::traduzir_uniao(&u, &base)?;
                let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;
                return Ok(resposta_do_sql(&texto, &plano, bruto));
            }
            // `CREATE VIEW` e `DROP VIEW`: o tradutor produz o pedido, e o
            // pedido volta pelo portao de sempre -- com `criar`/`excluir` na
            // base, que e o poder que as duas exigem.
            phxsql_sql::Comando::CriarVisao { nome, sql } => {
                let base = p.texto_ou("database", "").trim().to_string();
                let plano = phxsql_sql::traduzir_criar_visao(&nome, &sql, &base)?;
                let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;
                return Ok(resposta_do_sql(&texto, &plano, bruto));
            }
            phxsql_sql::Comando::ExcluirVisao { nome } => {
                let base = p.texto_ou("database", "").trim().to_string();
                let plano = phxsql_sql::traduzir_excluir_visao(&nome, &base)?;
                let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;
                return Ok(resposta_do_sql(&texto, &plano, bruto));
            }
            // INSERT, UPDATE e DELETE por chave: outro caminho, MESMO portao.
            escrita => return self.executar_dml(&escrita, &texto, p, sessao),
        };

        let base = match selecao.de.database.trim() {
            "" => p.texto_ou("database", "").trim().to_string(),
            outro => outro.to_string(),
        };
        // O `FROM` aponta para uma VISAO? Entao o pedido vira um `consultar`
        // sobre o plano dela. A pergunta comeca por um load atomico: num
        // servidor sem visao nenhuma, ela custa isso e mais nada.
        if let Some(plano) = self.selecao_sobre_visao(&selecao, &base, sessao)? {
            let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;
            // A resposta sai pela MESMA porta do SELECT de sempre: quem
            // consulta uma visao nao pode receber um envelope diferente do de
            // quem consulta uma tabela -- o cliente teria de saber, antes de
            // perguntar, se aquele nome e visao ou tabela.
            return Ok(resposta_do_sql(&texto, &plano, bruto));
        }
        let tabela = selecao.de.nome_no_protocolo();
        let ped_esquema = Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("tabela", Json::texto_de(&tabela)),
        ]);
        let esquema = self.executar_derivado("esquema", &ped_esquema, sessao)?;

        let plano = phxsql_sql::traduzir(&selecao, &indices_do_esquema(&esquema), &base)?;
        recusar_projecao_sobre_coluna_negada(&plano, &esquema, &base, &tabela)?;
        let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;

        Ok(resposta_do_sql(&texto, &plano, bruto))
    }

    /// `CREATE USER`, `ALTER USER` e `DROP USER` vindos pela op `sql`.
    ///
    /// `None` quando o texto nao e nenhum dos tres -- e ai a op `sql` segue o
    /// caminho de sempre, sem nada mudado.
    ///
    /// # O texto que volta e o texto REDIGIDO
    ///
    /// A op `sql` devolve o comando junto com o resultado, e a senha esta
    /// dentro do comando. Ela sai por ANALISE (`usuario::sem_a_senha`), nunca
    /// por recorte: o Profiler tapa o campo `senha` do JSON pelo NOME, e num
    /// texto SQL nao ha nome -- ha uma frase. Sem esta redacao, `CREATE USER`
    /// pela op `sql` poria a senha no `perfil.txt` e na resposta.
    fn sql_de_cadastro(&self, texto: &str, sessao: &Sessao) -> Option<Result<Json>> {
        let c = match phxsql_sql::usuario::comando(texto) {
            Ok(Some(c)) => c,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        let bruto = match self.executar(&c.op, &c.pedido(), sessao) {
            Ok(j) => j,
            Err(e) => return Some(Err(e)),
        };
        Some(Ok(Json::objeto(vec![
            (
                "sql",
                Json::texto_de(phxsql_sql::usuario::sem_a_senha(texto)),
            ),
            ("op", Json::texto_de(&c.op)),
            ("resultado", bruto),
        ])))
    }

    /// `INSERT`, `UPDATE` e `DELETE` por chave, vindos pela op `sql`.
    ///
    /// # Tres passos, pelo MESMO portao
    ///
    /// `INSERT` e um `inserir`. `UPDATE` e `DELETE` sao `buscar` -> `ler` ->
    /// `atualizar`/`excluir`, e o motivo esta escrito em `phxsql_sql::dml`: o
    /// `atualizar` grava a linha INTEIRA e preenche com NULL o que nao vem,
    /// entao a linha e lida e mesclada antes -- e a `versao` lida vai junto,
    /// para o motor recusar quem gravou entre os passos em vez de ser
    /// sobrescrito. Cada passo sai pelo `executar_derivado`, o portao que le
    /// o campo `tabela` do pedido traduzido: o mesmo do `SELECT`. Nao ha
    /// caminho de escrita proprio aqui, e e assim que a porta dos fundos nao
    /// nasce.
    ///
    /// # Zero linhas nao e erro
    ///
    /// Chave que nao existe devolve `afetadas: 0`, como todo SQL faz. Erro e
    /// para o que nao deveria acontecer: um indice unico que devolve duas
    /// linhas para a mesma chave.
    fn executar_dml(
        &self,
        comando: &phxsql_sql::Comando,
        texto: &str,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Json> {
        use phxsql_sql::{Comando, PlanoDml};
        let corrente = p.texto_ou("database", "").trim().to_string();
        let plano = match comando {
            Comando::Insercao(i) => {
                // Os indices so servem ao `ON CONFLICT`, que precisa achar o
                // indice unico da coluna; o INSERT de sempre nao paga a
                // leitura do esquema.
                let indices = if i.se_existir.is_some() {
                    self.indices_para_o_sql(&i.em, &corrente, sessao)?
                } else {
                    Vec::new()
                };
                phxsql_sql::traduzir_insercao(i, &indices, &corrente)?
            }
            Comando::Atualizacao(a) => {
                let indices = self.indices_para_o_sql(&a.em, &corrente, sessao)?;
                phxsql_sql::traduzir_atualizacao(a, &indices, &corrente)?
            }
            Comando::Exclusao(e) => {
                let indices = self.indices_para_o_sql(&e.de, &corrente, sessao)?;
                phxsql_sql::traduzir_exclusao(e, &indices, &corrente)?
            }
            Comando::Selecao(_) | Comando::Consulta(_) | Comando::Uniao(_) => {
                return Err(PhxError::Esquema(
                    "consulta nao entra pelo caminho de escrita".into(),
                ))
            }
            Comando::CriarVisao { .. } | Comando::ExcluirVisao { .. } => {
                return Err(PhxError::Esquema(
                    "CREATE/DROP VIEW nao entra pelo caminho de escrita de linha: \
                     eles mexem no catalogo, e nao no dado"
                        .into(),
                ))
            }
        };
        let op = plano.op();
        let mut notas: Vec<String> = plano.notas().to_vec();

        let (busca, atribuicoes) = match plano {
            PlanoDml::Inserir { pedido, .. } => {
                let r = self.executar_derivado("inserir", &pedido, sessao)?;
                return Ok(resposta_do_dml(
                    texto,
                    op,
                    &notas,
                    1,
                    &r,
                    &["rowid", "registros"],
                ));
            }
            PlanoDml::Atualizar {
                busca, atribuicoes, ..
            } => (busca, Some(atribuicoes)),
            PlanoDml::Excluir { busca, .. } => (busca, None),
            // POR FAIXA: coletar_rowids junta as linhas, e o laco aplica por
            // rowid. Sai por aqui porque o caminho por chave e' de UMA linha.
            PlanoDml::AtualizarPorFaixa {
                coletar,
                atribuicoes,
                ..
            } => {
                return self.executar_dml_por_faixa(
                    texto,
                    op,
                    notas,
                    &coletar,
                    Some(&atribuicoes),
                    sessao,
                );
            }
            PlanoDml::ExcluirPorFaixa { coletar, .. } => {
                return self.executar_dml_por_faixa(texto, op, notas, &coletar, None, sessao);
            }
        };

        // Passo 1: o rowid da chave.
        let achada = self.executar_derivado("buscar", &busca, sessao)?;
        let linhas = achada.campo("linhas").and_then(Json::lista).unwrap_or(&[]);
        let rowid = match linhas {
            [] => {
                notas.push("nenhuma linha tem essa chave: zero afetadas, e nao e erro".into());
                return Ok(resposta_do_dml(texto, op, &notas, 0, &Json::Nulo, &[]));
            }
            [uma] => uma.inteiro_ou("rowid", 0).max(0) as u64,
            varias => {
                return Err(PhxError::Esquema(format!(
                    "o indice unico do WHERE devolveu {} linhas para a mesma chave -- estado \
                     que nao deveria existir; nada foi gravado",
                    varias.len()
                )))
            }
        };
        let database = busca.texto_ou("database", "").to_string();
        let tabela = busca.texto_ou("tabela", "").to_string();

        // Passo 2: a linha inteira e a versao dela.
        let lida =
            self.executar_derivado("ler", &pedido_de_ler(&database, &tabela, rowid), sessao)?;
        if matches!(&lida, Json::Nulo) {
            notas.push("a linha sumiu entre o buscar e o ler: zero afetadas".into());
            return Ok(resposta_do_dml(texto, op, &notas, 0, &Json::Nulo, &[]));
        }
        let versao = lida.inteiro_ou("versao", 0).max(0) as u64;

        // Passo 3: gravar, com a versao lida.
        match atribuicoes {
            Some(set) => {
                let linha = lida.campo("linha").cloned().unwrap_or(Json::Nulo);
                let pedido = pedido_de_atualizar(&database, &tabela, rowid, &linha, &set, versao);
                let r = self.executar_derivado("atualizar", &pedido, sessao)?;
                Ok(resposta_do_dml(
                    texto,
                    op,
                    &notas,
                    1,
                    &r,
                    &["rowid", "versao"],
                ))
            }
            None => {
                let pedido = pedido_de_excluir(&database, &tabela, rowid, versao);
                let r = self.executar_derivado("excluir", &pedido, sessao)?;
                let afetadas = u64::from(r.booleano_ou("excluido", false));
                Ok(resposta_do_dml(
                    texto,
                    op,
                    &notas,
                    afetadas,
                    &r,
                    &["rowid", "modo", "reversivel", "na_lixeira"],
                ))
            }
        }
    }

    /// UPDATE/DELETE POR FAIXA: o `coletar_rowids` junta TODAS as linhas que
    /// casam a condicao ANTES de qualquer gravacao -- e a colheita-antes-da-
    /// aplicacao que fecha o Halloween: uma linha que o proprio UPDATE tira do
    /// filtro nao volta a ser vista, porque a lista de rowids ja esta fechada.
    /// Depois, por rowid, cada linha vira `ler` + `atualizar` (com a linha
    /// MESCLADA e a versao lida) ou `ler` + `excluir` SUAVE -- os MESMOS passos
    /// do caminho por chave, um por linha, e por isso cascata, restringir,
    /// direito por coluna e a janela de versao valem por linha sem um caminho de
    /// escrita proprio: cada gravacao sai pelo `executar_derivado`, o mesmo
    /// portao do SELECT.
    ///
    /// # Fora de transacao NAO e atomico -- de proposito
    ///
    /// Cada linha e um `atualizar`/`excluir` avulso; uma queda no meio deixa as
    /// ja gravadas gravadas. Numa transacao aberta cada uma empilha no
    /// super-journal, e o COMMIT/ROLLBACK as alcanca juntas: a atomicidade e da
    /// transacao, nao deste laco. O SQL nao abre transacao por conta -- quem
    /// quer tudo-ou-nada envolve o comando em BEGIN/COMMIT.
    ///
    /// # A versao lida vai em cada gravacao
    ///
    /// Quem gravar uma linha entre o `coletar` e o `ler`/`atualizar` faz o motor
    /// recusar AQUELA linha em vez de sobrescrever calado -- a mesma janela do
    /// caminho por chave, agora por linha. E a linha que sumiu entre o colher e
    /// o ler simplesmente nao conta: outra frente a excluiu, e o laco segue.
    ///
    /// # Zero linhas nao e erro
    ///
    /// Filtro que nao casa nada devolve `afetadas: 0`, como todo o resto do SQL.
    fn executar_dml_por_faixa(
        &self,
        texto: &str,
        op: &str,
        mut notas: Vec<String>,
        coletar: &Json,
        atribuicoes: Option<&Vec<(String, Json)>>,
        sessao: &Sessao,
    ) -> Result<Json> {
        // Passo 1: colher os rowids que casam. O `coletar_rowids` RECUSA se a
        // faixa for maior do que consegue prometer inteira, entao aqui ou vem a
        // faixa completa ou vem o erro -- nunca um pedaco com cara de tudo.
        let colhido = self.executar_derivado("coletar_rowids", coletar, sessao)?;
        let rowids: Vec<u64> = colhido
            .campo("rowids")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .map(|r| r.inteiro().unwrap_or(0).max(0) as u64)
            .collect();

        let database = coletar.texto_ou("database", "").to_string();
        let tabela = coletar.texto_ou("tabela", "").to_string();

        if rowids.is_empty() {
            notas.push("nenhuma linha casou o filtro: zero afetadas, e nao e erro".into());
            return Ok(resposta_do_dml(texto, op, &notas, 0, &Json::Nulo, &[]));
        }

        // Passo 2: por rowid, `ler` a linha e a versao, e gravar com a versao
        // LIDA. Os MESMOS `pedido_de_atualizar`/`pedido_de_excluir` do caminho
        // por chave -- a mescla, a marca de excluido preservada, o suave por
        // padrao. Nao ha caminho de escrita novo aqui.
        let mut afetadas: u64 = 0;
        for rowid in rowids {
            let lida =
                self.executar_derivado("ler", &pedido_de_ler(&database, &tabela, rowid), sessao)?;
            if matches!(&lida, Json::Nulo) {
                continue;
            }
            let versao = lida.inteiro_ou("versao", 0).max(0) as u64;
            match atribuicoes {
                Some(set) => {
                    let linha = lida.campo("linha").cloned().unwrap_or(Json::Nulo);
                    let pedido =
                        pedido_de_atualizar(&database, &tabela, rowid, &linha, set, versao);
                    self.executar_derivado("atualizar", &pedido, sessao)?;
                    afetadas += 1;
                }
                None => {
                    let pedido = pedido_de_excluir(&database, &tabela, rowid, versao);
                    let r = self.executar_derivado("excluir", &pedido, sessao)?;
                    afetadas += u64::from(r.booleano_ou("excluido", false));
                }
            }
        }

        Ok(resposta_do_dml(
            texto,
            op,
            &notas,
            afetadas,
            &Json::Nulo,
            &[],
        ))
    }

    /// Os indices da tabela, como o tradutor os espera -- a receita do caminho
    /// do SELECT: pede o `esquema` pelo portao e le campo por campo.
    fn indices_para_o_sql(
        &self,
        alvo: &phxsql_sql::Alvo,
        corrente: &str,
        sessao: &Sessao,
    ) -> Result<Vec<phxsql_sql::IndiceInfo>> {
        let base = if alvo.database.trim().is_empty() {
            corrente.to_string()
        } else {
            alvo.database.trim().to_string()
        };
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("tabela", Json::texto_de(alvo.nome_no_protocolo())),
        ]);
        let esquema = self.executar_derivado("esquema", &ped, sessao)?;
        Ok(indices_do_esquema(&esquema))
    }

    // ------------------------------------------------- gatilhos e rotinas

    /// Executa um comando de rotina vindo pela op `sql`.
    ///
    /// # Permissao: administrar, e por que nao `criar`
    ///
    /// Criar, excluir e LISTAR gatilhos e procedimentos exigem `administrar`
    /// na base — a mesma regra dos jobs, e pelo mesmo motivo: os tres sao
    /// codigo guardado que roda depois, sob o poder de OUTRA pessoa. Com
    /// `criar` bastando, quem cria tabela poderia pendurar um AFTER INSERT
    /// na tabela alheia e desviar cada linha gravada pelos outros para uma
    /// tabela sua — escalada por gatilho. `CALL`, ao contrario, nao pede nada
    /// proprio: cada pedido que o corpo produz passa pelo portao de sempre
    /// com o poder de quem chamou, entao chamar nunca da poder que a pessoa
    /// ja nao tinha.
    fn executar_rotina(
        &self,
        comando: phxsql_sql::rotina::Comando,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Json> {
        use phxsql_sql::rotina::Comando;
        let base_do_pedido = p.texto_ou("database", "").trim().to_string();
        let exigir_base = |base: &str| -> Result<()> {
            if base.is_empty() {
                return Err(PhxError::Esquema(
                    "informe \"database\" no pedido (rotina mora num database)".into(),
                ));
            }
            Ok(())
        };
        match comando {
            Comando::CriarGatilho(mut def) => {
                let base = if def.database.is_empty() {
                    base_do_pedido
                } else {
                    def.database.clone()
                };
                exigir_base(&base)?;
                self.exigir_administrar_rotina(sessao, &base, &def.tabela)?;
                self.recusar_somente_leitura("criar gatilho")?;
                // A tabela tem de existir — e abrir tambem valida os nomes
                // contra travessia, porque estes vieram de DENTRO do texto
                // SQL, que a sonda do despachar nao ve.
                {
                    let trava = self.travar_dados()?;
                    trava
                        .abrir_database(&base)?
                        .abrir_qualificada(&def.tabela)?;
                }
                // O database ja e o dono do arquivo; guardado dentro dele,
                // o campo seria redundancia que um dia discorda.
                def.database = String::new();
                let (g, devido) = {
                    let mut r = self.rotinas.tomar("rotinas")?;
                    let feito = r.criar_gatilho(&base, def, sessao.login())?;
                    self.ha_gatilhos.store(r.ha_gatilhos(), Ordering::Relaxed);
                    feito
                };
                self.gravar_rotinas(Some(devido))?;
                Ok(Json::objeto(vec![
                    ("gatilho", Json::texto_de(&g.nome)),
                    ("tabela", Json::texto_de(&g.tabela)),
                    ("quando", Json::texto_de(g.quando.nome())),
                    ("evento", Json::texto_de(g.evento.nome())),
                    ("criado", Json::Bool(true)),
                ]))
            }
            Comando::ExcluirGatilho { nome, se_existe } => {
                exigir_base(&base_do_pedido)?;
                self.exigir_administrar_rotina(sessao, &base_do_pedido, "")?;
                self.recusar_somente_leitura("excluir gatilho")?;
                let devido = {
                    let mut r = self.rotinas.tomar("rotinas")?;
                    let devido = r.excluir_gatilho(&base_do_pedido, &nome);
                    self.ha_gatilhos.store(r.ha_gatilhos(), Ordering::Relaxed);
                    devido
                };
                let saiu = devido.is_some();
                self.gravar_rotinas(devido)?;
                if !saiu && !se_existe {
                    return Err(PhxError::NaoEncontrado(format!(
                        "gatilho {nome:?} nao existe em {base_do_pedido}"
                    )));
                }
                Ok(Json::objeto(vec![
                    ("gatilho", Json::texto_de(nome)),
                    ("excluido", Json::Bool(saiu)),
                ]))
            }
            Comando::MostrarGatilhos => {
                exigir_base(&base_do_pedido)?;
                self.exigir_administrar_rotina(sessao, &base_do_pedido, "")?;
                let r = self.rotinas.tomar("rotinas")?;
                let lista = r.gatilhos_do_db(&base_do_pedido);
                Ok(Json::objeto(vec![
                    ("total", Json::de_u64(lista.len() as u64)),
                    (
                        "gatilhos",
                        Json::Lista(lista.iter().map(|g| g.para_json()).collect()),
                    ),
                ]))
            }
            Comando::CriarProcedimento(def) => {
                exigir_base(&base_do_pedido)?;
                self.exigir_administrar_rotina(sessao, &base_do_pedido, "")?;
                self.recusar_somente_leitura("criar procedimento")?;
                let quantos = def.parametros.len();
                let (criado, devido) = {
                    let mut r = self.rotinas.tomar("rotinas")?;
                    r.criar_procedimento(&base_do_pedido, def, sessao.login())?
                };
                self.gravar_rotinas(Some(devido))?;
                let nome = criado.nome.clone();
                Ok(Json::objeto(vec![
                    ("procedimento", Json::texto_de(nome)),
                    ("parametros", Json::de_u64(quantos as u64)),
                    ("criado", Json::Bool(true)),
                ]))
            }
            Comando::ExcluirProcedimento { nome, se_existe } => {
                exigir_base(&base_do_pedido)?;
                self.exigir_administrar_rotina(sessao, &base_do_pedido, "")?;
                self.recusar_somente_leitura("excluir procedimento")?;
                let devido = {
                    let mut r = self.rotinas.tomar("rotinas")?;
                    r.excluir_procedimento(&base_do_pedido, &nome)
                };
                let saiu = devido.is_some();
                self.gravar_rotinas(devido)?;
                if !saiu && !se_existe {
                    return Err(PhxError::NaoEncontrado(format!(
                        "procedimento {nome:?} nao existe em {base_do_pedido}"
                    )));
                }
                Ok(Json::objeto(vec![
                    ("procedimento", Json::texto_de(nome)),
                    ("excluido", Json::Bool(saiu)),
                ]))
            }
            Comando::MostrarProcedimentos => {
                exigir_base(&base_do_pedido)?;
                self.exigir_administrar_rotina(sessao, &base_do_pedido, "")?;
                let r = self.rotinas.tomar("rotinas")?;
                let lista = r.procedimentos_do_db(&base_do_pedido);
                Ok(Json::objeto(vec![
                    ("total", Json::de_u64(lista.len() as u64)),
                    (
                        "procedimentos",
                        Json::Lista(lista.iter().map(|q| q.para_json()).collect()),
                    ),
                ]))
            }
            Comando::Chamar { nome, argumentos } => {
                exigir_base(&base_do_pedido)?;
                self.chamar_procedimento(&base_do_pedido, &nome, argumentos, sessao)
            }
        }
    }

    /// `CALL nome(args)`: roda o corpo com o poder de quem chamou.
    fn chamar_procedimento(
        &self,
        base: &str,
        nome: &str,
        argumentos: Vec<phxsql_sql::rotina::Valor>,
        sessao: &Sessao,
    ) -> Result<Json> {
        use phxsql_sql::rotina::{executar, Contexto, Modo, Valor};
        // A trava do registro solta ANTES de o corpo rodar: o Arc viaja, e o
        // corpo pode demorar o quanto o teto de passos permitir.
        let procedimento = {
            let r = self.rotinas.tomar("rotinas")?;
            r.procedimento(base, nome)
        }
        .ok_or_else(|| {
            PhxError::NaoEncontrado(format!("procedimento {nome:?} nao existe em {base}"))
        })?;
        let programa = procedimento.programa.as_ref().map_err(|motivo| {
            PhxError::Esquema(format!(
                "o procedimento {nome:?} nao compila ({motivo}); DROP PROCEDURE e crie de novo"
            ))
        })?;

        // Os argumentos batem com os parametros? OUT no fim pode ser omitido
        // — `CALL somar(10)` — porque quem chama nao tem o que passar ali.
        let parametros = &procedimento.parametros;
        let saidas_no_fim = parametros
            .iter()
            .rev()
            .take_while(|q| q.modo == Modo::Saida)
            .count();
        if argumentos.len() != parametros.len()
            && argumentos.len() != parametros.len() - saidas_no_fim
        {
            return Err(PhxError::Esquema(format!(
                "{nome} espera {} argumento(s) — {} — e vieram {}. OUT no fim \
                 pode ser omitido; no meio, passe NULL",
                parametros.len(),
                parametros
                    .iter()
                    .map(|q| format!("{} {}", q.modo.nome(), q.nome))
                    .collect::<Vec<_>>()
                    .join(", "),
                argumentos.len()
            )));
        }
        let mut sementes = Vec::with_capacity(parametros.len());
        for (i, q) in parametros.iter().enumerate() {
            // OUT comeca NULL sempre, como no MySQL(R): o valor passado num
            // OUT e so um lugar, nunca uma entrada.
            let valor = if q.modo == Modo::Saida {
                Valor::Nulo
            } else {
                q.tipo
                    .coagir(argumentos.get(i).cloned().unwrap_or(Valor::Nulo))
                    .map_err(|e| {
                        PhxError::Tipo(format!("no parametro {:?} de {nome}: {e}", q.nome))
                    })?
            };
            sementes.push((q.nome.clone(), q.tipo, valor));
        }
        let mut ctx = Contexto::de_procedimento(sementes);
        let mut motor = MotorDoServidor {
            servidor: self,
            sessao,
            database: base.to_string(),
        };
        executar(programa, &mut ctx, &mut motor)?;
        let saida: Vec<(String, Json)> = parametros
            .iter()
            .filter(|q| q.modo != Modo::Entrada)
            .map(|q| {
                (
                    q.nome.clone(),
                    ctx.valor_de(&q.nome)
                        .cloned()
                        .unwrap_or(Valor::Nulo)
                        .para_json(),
                )
            })
            .collect();
        Ok(Json::objeto(vec![
            ("procedimento", Json::texto_de(&procedimento.nome)),
            ("saida", Json::Objeto(saida)),
        ]))
    }

    fn exigir_administrar_rotina(&self, sessao: &Sessao, base: &str, tabela: &str) -> Result<()> {
        if let Some(u) = &sessao.usuario {
            if !u.pode_em(base, tabela, Atividade::Administrar) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de administrar em {} — criar, excluir \
                     e listar rotinas exigem administrar",
                    u.login,
                    if tabela.is_empty() {
                        base.to_string()
                    } else {
                        format!("{base}.{tabela}")
                    }
                )));
            }
        }
        Ok(())
    }

    /// A op `sql` nao esta em `OPS_ESCRITA` porque um SELECT nao escreve —
    /// mas criar e excluir rotina grava arquivo, entao a conferencia mora
    /// aqui, no unico lugar por onde esses comandos passam.
    fn recusar_somente_leitura(&self, o_que: &str) -> Result<()> {
        if self.config.somente_leitura {
            return Err(PhxError::Autorizacao(format!(
                "servidor em modo somente leitura: {o_que} grava arquivo"
            )));
        }
        Ok(())
    }

    /// Leva ao disco o arquivo de rotinas que uma mudanca deixou devendo --
    /// pedido 595. O retrato sai sob a trava do registro (barato) e o
    /// `fsync` acontece FORA dela e fora da trava global: a escrita com
    /// gatilho, que le o registro a cada linha, nao espera o disco de um
    /// `CREATE TRIGGER`.
    ///
    /// `cadastro_no_disco` serializa tirar-e-gravar: sem ele, dois `CREATE`
    /// concorrentes podiam tirar o retrato numa ordem e gravar na outra, e o
    /// disco ficaria com o mais VELHO -- sem o gatilho de quem ja ouviu
    /// «criado».
    pub(super) fn gravar_rotinas(&self, devido: Option<crate::rotinas::PorGravar>) -> Result<()> {
        let Some(devido) = devido else {
            return Ok(());
        };
        let _vez = self.cadastro_no_disco.tomar("cadastro_no_disco")?;
        let retrato = self.rotinas.tomar("rotinas")?.retrato(&devido);
        retrato.gravar()
    }

    /// O [`Self::gravar_rotinas`] do `visoes.json`, pela mesma vez e pelo
    /// mesmo motor de disco.
    pub(super) fn gravar_visoes(&self, devido: Option<crate::visoes::PorGravar>) -> Result<()> {
        let Some(devido) = devido else {
            return Ok(());
        };
        let _vez = self.cadastro_no_disco.tomar("cadastro_no_disco")?;
        let retrato = self.visoes.tomar("visoes")?.retrato(&devido);
        retrato.gravar()
    }

    /// O portao dos gatilhos de uma escrita.
    ///
    /// Sem gatilho NENHUM no servidor, custa um load atomico e devolve vazio
    /// — sem trava, sem String, sem olhar o pedido. E a licao do Profiler
    /// aplicada antes de doer: o portao vem ANTES de qualquer trabalho.
    pub(super) fn gatilhos_para(
        &self,
        p: &Json,
        evento: phxsql_sql::rotina::Evento,
    ) -> Result<crate::rotinas::AntesEDepois> {
        if !self.ha_gatilhos.load(Ordering::Relaxed) {
            return Ok((Vec::new(), Vec::new()));
        }
        let r = self.rotinas.tomar("rotinas")?;
        Ok(r.gatilhos_de(
            p.texto_ou("database", "").trim(),
            p.texto_ou("tabela", "").trim(),
            evento,
        ))
    }

    /// Gatilho quebrado barra a escrita ANTES de ela comecar — inclusive o
    /// AFTER, que depois de gravado nao teria mais como barrar nada. Pular a
    /// regra quebrada seria fingir que ela nao existe, em silencio.
    pub(super) fn conferir_gatilhos_compilam(
        antes: &[Arc<crate::rotinas::Gatilho>],
        depois: &[Arc<crate::rotinas::Gatilho>],
    ) -> Result<()> {
        for g in antes.iter().chain(depois) {
            if let Err(motivo) = &g.programa {
                return Err(PhxError::Esquema(format!(
                    "o gatilho {:?} desta tabela nao compila ({motivo}); \
                     conserte-o ou exclua com DROP TRIGGER — a escrita nao \
                     prossegue com a regra quebrada",
                    g.nome
                )));
            }
        }
        Ok(())
    }

    /// Roda os BEFORE de uma escrita, na ordem de criacao, sobre a linha ja
    /// convertida. Roda com a trava de dados na mao — por isso o motor e o
    /// [`phxsql_sql::rotina::MotorNulo`]: o parser ja recusa DML nesses
    /// corpos, e o motor nulo e o cinto para a instrucao que um dia esquecer.
    ///
    /// `nova` e a linha que vai ser gravada (INSERT/UPDATE); `velha`, a que
    /// esta la (UPDATE/DELETE). O que o corpo tocar via `SET NEW.…` volta
    /// para a linha ja convertido no tipo da coluna — pelo MESMO
    /// `json_para_valor_da_coluna` de qualquer pedido.
    pub(super) fn rodar_gatilhos_antes(
        &self,
        gatilhos: &[Arc<crate::rotinas::Gatilho>],
        nova: Option<&mut Vec<Value>>,
        velha: Option<&[Value]>,
        esquema: &Schema,
    ) -> Result<()> {
        use phxsql_sql::rotina::{executar, Contexto, MotorNulo};
        let gravavel = nova.is_some();
        let mut nova_json = nova.as_ref().map(|l| linha_para_json(l, esquema));
        let velha_json = velha.map(|l| linha_para_json(l, esquema));
        let mut tocadas: Vec<String> = Vec::new();
        for g in gatilhos {
            let programa = g.programa.as_ref().map_err(|motivo| {
                PhxError::Esquema(format!("o gatilho {:?} nao compila: {motivo}", g.nome))
            })?;
            let mut ctx = Contexto::de_gatilho(nova_json.take(), gravavel, velha_json.clone())
                .com_prazo(PRAZO_DO_GATILHO_ANTES);
            let resultado = executar(programa, &mut ctx, &mut MotorNulo);
            // O NEW volta mesmo em erro: o proximo uso e de quem tratar.
            nova_json = ctx.nova.take();
            resultado.map_err(|e| erro_do_gatilho(&g.nome, e))?;
            for coluna in ctx.tocadas {
                if !tocadas.contains(&coluna) {
                    tocadas.push(coluna);
                }
            }
        }
        if let (Some(linha), Some(objeto)) = (nova, nova_json) {
            for coluna in &tocadas {
                if phxsql_core::schema::e_coluna_de_sistema(coluna) {
                    return Err(PhxError::Esquema(format!(
                        "gatilho tentou alterar a coluna de sistema {coluna:?} — \
                         rownum e softdeleted sao do motor"
                    )));
                }
                let Some(i) = esquema.coluna_por_nome(coluna) else {
                    continue;
                };
                let valor = objeto.campo(coluna).unwrap_or(&Json::Nulo);
                linha[i] =
                    json_para_valor_da_coluna(valor, &esquema.colunas()[i]).map_err(|e| {
                        PhxError::Tipo(format!("gatilho gravou NEW.{coluna} invalido: {e}"))
                    })?;
            }
        }
        Ok(())
    }

    /// Roda os AFTER, ja SEM a trava de dados: o corpo fala com o motor (o
    /// INSERT de auditoria) pelos MESMOS portoes de qualquer pedido, com o
    /// poder de quem disparou a escrita.
    ///
    /// # Falha de AFTER e aviso, nao erro — e isso esta escrito
    ///
    /// A escrita ja aconteceu e nao ha transacao que a desfaca. Devolver erro
    /// diria "nao gravou" a quem gravou — e o cliente repetiria, duplicando a
    /// linha. A resposta fica `ok` e carrega `gatilhos_avisos` com o nome do
    /// gatilho e o motivo: as duas verdades, na ordem certa.
    ///
    /// # A cadeia tem fundo, e o fundo nao e negociavel
    ///
    /// O corpo de um AFTER pode gravar, e o que ele grava dispara os AFTER
    /// daquela tabela. Um `AFTER INSERT ON t` que grava em `t` chama a si
    /// mesmo, e sem fundo isso nao e um laco infinito qualquer: e recursao de
    /// pilha, e o Rust ABORTA O PROCESSO com "stack overflow". Um gatilho que
    /// alguem escreveu derrubava o servidor inteiro para todo mundo — e como o
    /// corpo mora no `gatilhos.json`, ele derrubava de novo a cada tentativa.
    ///
    /// O teto e por thread e por cadeia, nao por gatilho: quem conta e a
    /// profundidade do aninhamento. Oito e folga larga para a cadeia real
    /// (venda -> auditoria -> resumo), e curta o laco em nove linhas.
    pub(super) fn rodar_gatilhos_depois(
        &self,
        gatilhos: &[Arc<crate::rotinas::Gatilho>],
        nova: Option<Json>,
        velha: Option<Json>,
        p: &Json,
        sessao: &Sessao,
    ) -> Vec<Json> {
        use phxsql_sql::rotina::{executar, Contexto};
        // Sem AFTER nenhum nesta tabela nao ha nem cadeia nem contador: o
        // caminho de escrita comum nao paga por uma guarda que nao usa.
        if gatilhos.is_empty() {
            return Vec::new();
        }
        let nivel = PROFUNDIDADE_DA_CADEIA.with(|c| c.get());
        if nivel >= CADEIA_MAXIMA {
            // O aviso NAO sai daqui. Aqui embaixo ele iria para a resposta de
            // um pedido derivado, que o corpo do gatilho descarta — e quem
            // gravou receberia um `ok` limpo sobre uma cadeia cortada. A marca
            // sobe ate o nivel zero, que e a resposta que alguem le.
            CADEIA_CORTADA.with(|c| c.set(true));
            return Vec::new();
        }
        if nivel == 0 {
            CADEIA_CORTADA.with(|c| c.set(false));
        }
        PROFUNDIDADE_DA_CADEIA.with(|c| c.set(nivel + 1));
        let _fim = AoSair(|| PROFUNDIDADE_DA_CADEIA.with(|c| c.set(nivel)));

        let mut avisos = Vec::new();
        let database = p.texto_ou("database", "").trim().to_string();
        for g in gatilhos {
            let programa = match g.programa.as_ref() {
                Ok(programa) => programa,
                // Barrado antes da escrita por `conferir_gatilhos_compilam`;
                // se chegou aqui quebrado, a unica coisa honesta e avisar.
                Err(motivo) => {
                    avisos.push(Json::texto_de(format!(
                        "gatilho {:?} nao compila: {motivo}",
                        g.nome
                    )));
                    continue;
                }
            };
            let mut ctx = Contexto::de_gatilho(nova.clone(), false, velha.clone());
            let mut motor = MotorDoServidor {
                servidor: self,
                sessao,
                database: database.clone(),
            };
            if let Err(e) = executar(programa, &mut ctx, &mut motor) {
                avisos.push(Json::texto_de(format!("gatilho {:?} falhou: {e}", g.nome)));
            }
        }
        // A cadeia cortada la embaixo vira aviso AQUI, no unico nivel cuja
        // resposta chega a alguem.
        if nivel == 0 && CADEIA_CORTADA.with(|c| c.get()) {
            avisos.push(Json::texto_de(format!(
                "a cadeia de gatilhos passou de {CADEIA_MAXIMA} niveis e foi cortada \
                 — um AFTER que grava na propria tabela dispara a si mesmo"
            )));
        }
        avisos
    }
}

/// O `ler` do passo 2 do UPDATE/DELETE por chave -- com a versao, que e o
/// que o `buscar` nao traz.
pub(super) fn pedido_de_ler(database: &str, tabela: &str, rowid: u64) -> Json {
    Json::objeto(vec![
        ("op", Json::texto_de("ler")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("rowid", Json::de_u64(rowid)),
        ("com_versao", Json::Bool(true)),
    ])
}

/// O `atualizar` do passo 3: a linha LIDA com o `SET` por cima, e a versao.
///
/// # A mescla e a parte que importa
///
/// O `atualizar` grava a linha inteira e preenche com NULL o que nao veio
/// (`json_para_linha`). Mandar so o `SET` zeraria as outras colunas -- e o
/// teste `update_por_chave_muda_so_a_coluna_do_set` e a prova real disso:
/// tire a mescla e ele falha na coluna que o SET nao citou.
///
/// A marca de excluido fica de FORA da mescla de proposito: o `atualizar` a
/// preserva quando ela nao vem, e manda-la de volta so repetiria o que o
/// motor ja faz -- com o risco de mandar `false` numa linha que outro excluiu
/// entre os passos. O numero de ordem VAI, como foi lido: e do motor, e
/// volta igual.
pub(super) fn pedido_de_atualizar(
    database: &str,
    tabela: &str,
    rowid: u64,
    linha_lida: &Json,
    atribuicoes: &[(String, Json)],
    versao: u64,
) -> Json {
    let mut valores: Vec<(String, Json)> = match linha_lida {
        Json::Objeto(pares) => pares
            .iter()
            .filter(|(k, _)| k != phxsql_core::schema::COLUNA_SOFTDELETED)
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    for (coluna, valor) in atribuicoes {
        match valores
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(coluna))
        {
            Some(par) => par.1 = valor.clone(),
            None => valores.push((coluna.clone(), valor.clone())),
        }
    }
    Json::objeto(vec![
        ("op", Json::texto_de("atualizar")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("rowid", Json::de_u64(rowid)),
        ("valores", Json::Objeto(valores)),
        ("versao", Json::de_u64(versao)),
    ])
}

/// O `excluir` do passo 3 -- suave, o padrao do motor, e com a versao.
pub(super) fn pedido_de_excluir(database: &str, tabela: &str, rowid: u64, versao: u64) -> Json {
    Json::objeto(vec![
        ("op", Json::texto_de("excluir")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("rowid", Json::de_u64(rowid)),
        ("versao", Json::de_u64(versao)),
    ])
}

/// O texto SQL como volta no campo `sql` da resposta da op `sql`.
///
/// Redigido quando menciona senha -- pedido 497, terceira volta. O roteiro
/// com a senha numa linha comentada RODA (o lexico descarta o comentario), e
/// a resposta ecoava o texto inteiro: senha em resposta do protocolo. A
/// decisao e a mesma do Profiler, e vem do mesmo motor.
fn sql_de_volta(texto: &str) -> Json {
    Json::texto_de(
        phxsql_sql::usuario::sem_a_senha_se_mencionada(texto).unwrap_or_else(|| texto.to_string()),
    )
}

/// A resposta de um comando de escrita pela op `sql`: o texto, a operacao que
/// gravou, as notas do tradutor e quantas linhas foram afetadas -- mais o
/// que a operacao devolveu e vale repetir (rowid, versao nova, modo), e os
/// avisos de gatilho quando houver.
fn resposta_do_dml(
    texto: &str,
    op: &str,
    notas: &[String],
    afetadas: u64,
    bruto: &Json,
    repetir: &[&str],
) -> Json {
    let mut pares = vec![
        ("sql".to_string(), sql_de_volta(texto)),
        ("op".to_string(), Json::texto_de(op)),
        (
            "notas".to_string(),
            Json::Lista(notas.iter().map(Json::texto_de).collect()),
        ),
        ("afetadas".to_string(), Json::de_u64(afetadas)),
    ];
    // `colunas_mantidas` viaja sempre, como os avisos de gatilho: e o direito
    // por coluna dizendo que o `SET salario = 1` de quem nao altera `salario`
    // deixou o gravado como estava. Perder isso no envelope faria o SQL
    // responder `afetadas: 1` a uma coluna que nao mudou -- a resposta
    // errada calada que o campo existe para impedir.
    for campo in repetir
        .iter()
        .chain(["gatilhos_avisos", "colunas_mantidas"].iter())
    {
        if let Some(v) = bruto.campo(campo) {
            pares.push((campo.to_string(), v.clone()));
        }
    }
    Json::Objeto(pares)
}

/// A resposta do `sql`: o que a operacao devolveu, mais o que a traducao
/// decidiu.
///
/// # Por que as notas viajam
///
/// Porque `ORDER BY nome` pode ter sido atendido pelo `.ndx` e `COUNT(*)` pode
/// ter saido do cabecalho sem varrer nada -- e quem escreveu o comando nao tem
/// como saber qual dos dois aconteceu. Sem as notas, quem pediu uma ordem e
/// recebeu a de digitacao culpa o motor.
pub(super) fn resposta_do_sql(texto: &str, plano: &phxsql_sql::Plano, bruto: Json) -> Json {
    let mut pares = vec![
        ("sql".to_string(), sql_de_volta(texto)),
        ("op".to_string(), Json::texto_de(&plano.op)),
        (
            "notas".to_string(),
            Json::Lista(plano.notas.iter().map(Json::texto_de).collect()),
        ),
    ];

    match &plano.saida {
        // A contagem sai do cabecalho da tabela (`registros`) ou do total da
        // busca (`encontrados`): nenhuma linha e varrida para contar.
        //
        // E a resposta para por aqui: o `COUNT(*)` traduzido pede `max: 1`
        // para ler o cabecalho, e a linha que vem junto e um efeito colateral
        // do caminho -- nao a resposta. Devolve-la faria um `SELECT COUNT(*)`
        // mostrar UMA linha de dado, que e a pior resposta possivel: quem
        // olha nao sabe se aquela linha quer dizer alguma coisa. Este defeito
        // so apareceu exercitando o console.
        phxsql_sql::Saida::Contagem => {
            let n = match bruto.campo("encontrados") {
                Some(e) => e.inteiro().unwrap_or(0),
                None => bruto.inteiro_ou("registros", 0),
            };
            pares.push(("contagem".to_string(), Json::de_i64(n)));
            if let Some(r) = bruto.campo("registros") {
                pares.push(("registros".to_string(), r.clone()));
            }
            return Json::Objeto(pares);
        }
        phxsql_sql::Saida::LinhaInteira => {}
        // A projecao e do cliente porque o protocolo sempre devolve a linha
        // inteira -- o `.reg` e de slot fixo, e ler meia linha custa a mesma
        // leitura. Aqui o "cliente" e o servidor, porque quem pediu escreveu
        // SQL e espera as colunas que pediu.
        phxsql_sql::Saida::Colunas(cols) => {
            pares.push((
                "colunas".to_string(),
                Json::Lista(
                    cols.iter()
                        .map(|(_, rotulo)| Json::texto_de(rotulo))
                        .collect(),
                ),
            ));
        }
        // O `unir` responde `linhas` em LISTA posicional e `colunas` em
        // `[{nome,tipo,lado,chave}]` -- as duas formas diferentes das de todo
        // outro SELECT. Quem escreveu SQL nao pode receber um envelope
        // diferente por causa da OPERACAO que atendeu: e a mesma lei que fez o
        // SELECT sobre visao sair pela porta do SELECT de sempre.
        //
        // Os rotulos saem da RESPOSTA, e nao do plano: os nomes de uma uniao
        // sao os da PRIMEIRA parte, e quem os conhece e o motor que abriu o
        // esquema dela. O tradutor nao ve esquema nenhum.
        phxsql_sql::Saida::Posicional => {
            pares.push((
                "colunas".to_string(),
                Json::Lista(
                    nomes_do_cabecalho(&bruto)
                        .into_iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ));
        }
    }

    // **A PROJECAO NAO ACONTECE DUAS VEZES**, e este `if` saiu do encontro das
    // duas frentes: o `consultar` recebe `colunas` no PROPRIO pedido e projeta
    // na fonte, com o APELIDO como chave da linha. Projetar de novo aqui
    // procuraria `nome` numa linha que ja se chama `quem`, e devolveria uma
    // coluna de NULOS -- com a cara de dado que nao existe. Nenhum teste de
    // tradução acusaria isso (la o pedido esta certo) e nenhum teste do
    // `consultar` acusaria (la a resposta esta certa): so aparece na costura,
    // e so exercitando o caminho inteiro.
    //
    // O cabecalho (`colunas`) continua saindo daqui, porque ele e o contrato
    // da RESPOSTA e nao depende de quem projetou.
    let ja_projetou = plano.op == "consultar";
    let posicional = matches!(plano.saida, phxsql_sql::Saida::Posicional);
    let rotulos = if posicional {
        nomes_do_cabecalho(&bruto)
    } else {
        Vec::new()
    };
    if let Json::Objeto(campos) = bruto {
        for (k, v) in campos {
            if posicional {
                // O cabecalho ja saiu acima, na forma do `sql`.
                if k == "colunas" {
                    continue;
                }
                // O `unir` responde um campo `"sql"` proprio -- o rotulo
                // "UNION"/"UNION ALL", que a tela dos cartoes de Venn mostra
                // ao lado do desenho. O envelope do `sql` ja tem um `"sql"`,
                // e ele e o TEXTO do comando: copiar o de dentro poria a mesma
                // chave duas vezes no objeto, com sentidos diferentes, e quem
                // le veria uma sem saber qual. E a mesma razao do `colunas`
                // logo acima. O `modo` continua vindo, e ele diz o mesmo.
                if k == "sql" {
                    continue;
                }
                if k == "linhas" {
                    if let Json::Lista(linhas) = &v {
                        pares.push((
                            k,
                            Json::Lista(
                                linhas
                                    .iter()
                                    .map(|l| nomear_posicional(l, &rotulos))
                                    .collect(),
                            ),
                        ));
                        continue;
                    }
                }
            }
            // O `consultar` responde o SEU `colunas` -- o modelo tipado,
            // `[{nome, tipo}]` -- e o `sql` ja tem o dele, a lista de rotulos.
            // Copiar o de dentro poria a mesma chave duas vezes no objeto,
            // com formas diferentes, e quem le veria uma sem saber qual. O
            // contrato do `sql` continua sendo a lista de rotulos.
            if k == "colunas" && ja_projetou {
                continue;
            }
            if k == "linhas" && !ja_projetou {
                if let (phxsql_sql::Saida::Colunas(cols), Json::Lista(linhas)) = (&plano.saida, &v)
                {
                    pares.push((
                        k,
                        Json::Lista(linhas.iter().map(|l| projetar(l, cols)).collect()),
                    ));
                    continue;
                }
            }
            pares.push((k, v));
        }
    }
    Json::Objeto(pares)
}

/// A projecao do plano Simples contra a coluna que este usuario NAO le.
///
/// O `varrer` que o plano pede sai PENEIRADO -- a coluna negada nao vem --,
/// e o `projetar` logo abaixo poe `null` na coluna que a linha nao tem. Os
/// dois estao certos sozinhos e errados juntos: `SELECT salario FROM folha`
/// por quem nao le `salario` devolvia `{"salario": null}` em toda linha, que
/// e mentira sobre o dado -- quem olha conclui que a coluna esta vazia no
/// banco. O `consultar` (visao, junção, subconsulta) ja recusava, porque
/// resolve a projecao contra o modelo; este e o irmao que projeta por conta.
///
/// A lista do que o usuario nao le vem do PROPRIO `esquema` que o tradutor
/// acabou de pedir: e o campo `colunas_sem_leitura`, que o direito por coluna
/// acrescenta so para quem tem regra. Sem regra a lista nao existe e nada
/// muda -- nem uma alocacao.
pub(super) fn recusar_projecao_sobre_coluna_negada(
    plano: &phxsql_sql::Plano,
    esquema: &Json,
    base: &str,
    tabela: &str,
) -> Result<()> {
    use crate::direito_coluna::mesmo_nome;
    let phxsql_sql::Saida::Colunas(cols) = &plano.saida else {
        return Ok(());
    };
    let negadas = esquema
        .campo("colunas_sem_leitura")
        .and_then(Json::lista)
        .unwrap_or(&[]);
    if negadas.is_empty() {
        return Ok(());
    }
    for (nome, _) in cols {
        // O nome pode vir qualificado (`f.salario`): quem a regra nomeia e a
        // COLUNA, e o prefixo e o apelido da tabela.
        let nu = nome.rsplit('.').next().unwrap_or(nome);
        if let Some(n) = negadas
            .iter()
            .filter_map(Json::texto)
            .find(|n| mesmo_nome(n, nu))
        {
            return Err(PhxError::Autorizacao(format!(
                "a coluna {n:?} de {base}.{tabela} nao pode ser lida por este usuario: \
                 tire-a da lista do SELECT (um `SELECT *` devolve a linha sem ela)"
            )));
        }
    }
    Ok(())
}

/// Os nomes do cabecalho de uma resposta posicional (`unir`), na ordem.
///
/// A coluna sem nome vira `coluna_N` em vez de sumir: uma chave ausente faria
/// a linha ter forma diferente das vizinhas, e quem le por posicao quebraria.
fn nomes_do_cabecalho(bruto: &Json) -> Vec<String> {
    bruto
        .campo("colunas")
        .and_then(Json::lista)
        .map(|cs| {
            cs.iter()
                .enumerate()
                .map(
                    |(i, c)| match c.texto().or_else(|| c.campo("nome").and_then(Json::texto)) {
                        Some(n) if !n.trim().is_empty() => n.to_string(),
                        _ => format!("coluna_{}", i + 1),
                    },
                )
                .collect()
        })
        .unwrap_or_default()
}

/// A linha POSICIONAL do `unir` vira o objeto que todo SELECT devolve.
///
/// Valor sem nome no cabecalho nao some: vira `coluna_N` pelo mesmo motivo do
/// `projetar` -- resposta de forma variavel e pior que resposta feia.
fn nomear_posicional(linha: &Json, rotulos: &[String]) -> Json {
    let Json::Lista(valores) = linha else {
        // Ja e objeto? Entao a resposta nao era posicional, e devolve-la
        // embrulhada de novo inventaria uma coluna. Passa como esta.
        return linha.clone();
    };
    Json::Objeto(
        valores
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let nome = rotulos
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("coluna_{}", i + 1));
                (nome, v.clone())
            })
            .collect(),
    )
}

fn erro_do_gatilho(nome: &str, e: PhxError) -> PhxError {
    match e {
        PhxError::Sinal { .. } => e,
        outro => PhxError::Esquema(format!("gatilho {nome:?}: {outro}")),
    }
}

/// O motor que os corpos de rotina enxergam: cada pedido que um corpo produz
/// passa pelo `executar_derivado` — o MESMO portao de politica e de permissao
/// dos pedidos da rede, com a sessao de quem disparou. E a licao do
/// `juntar`/`unir`: a rotina produz o pedido que o portao ja sabe conferir,
/// em vez de ganhar uma porta propria para os dados.
struct MotorDoServidor<'a> {
    servidor: &'a Servidor,
    sessao: &'a Sessao,
    /// O database corrente: completa o pedido que o corpo escreveu sem `db.`.
    database: String,
}

impl phxsql_sql::rotina::Motor for MotorDoServidor<'_> {
    fn operacao(&mut self, op: &str, pedido: &Json) -> Result<Json> {
        let pedido = match pedido.campo("database") {
            Some(_) => pedido.clone(),
            None => {
                let Json::Objeto(pares) = pedido else {
                    return Err(PhxError::Esquema("pedido de rotina nao e objeto".into()));
                };
                let mut pares = pares.clone();
                pares.insert(0, ("database".into(), Json::texto_de(&self.database)));
                Json::Objeto(pares)
            }
        };
        self.servidor.executar_derivado(op, &pedido, self.sessao)
    }

    fn consultar(&mut self, sql: &str) -> Result<Json> {
        // O caminho e o proprio op_sql: a traducao, as notas e — o que
        // importa — o portao sobre a operacao TRADUZIDA, tudo igual a um
        // SELECT que chegasse pela rede.
        let pedido = Json::objeto(vec![
            ("database", Json::texto_de(&self.database)),
            ("texto", Json::texto_de(sql)),
        ]);
        self.servidor.op_sql(&pedido, self.sessao)
    }
}

/// Quantos niveis de AFTER uma cadeia pode empilhar antes de o servidor
/// recusar o proximo.
///
/// A cadeia real e curta -- venda dispara auditoria, auditoria dispara
/// resumo -- e oito e folga larga para ela. O que este numero existe para
/// impedir e a cadeia que nao acaba: `AFTER INSERT ON t` gravando em `t`.
pub(super) const CADEIA_MAXIMA: u32 = 8;

thread_local! {
    /// A profundidade da cadeia de gatilhos DESTA thread.
    ///
    /// Por thread porque a cadeia e uma pilha de chamadas: cada pedido roda na
    /// sua linha de execucao, e o que interessa e quantos AFTER estao
    /// empilhados abaixo deste -- nao quantos o servidor inteiro esta rodando.
    static PROFUNDIDADE_DA_CADEIA: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };

    /// A cadeia desta thread bateu no teto e foi cortada?
    ///
    /// Marcada no fundo, lida no topo. O nivel que corta esta dentro de um
    /// pedido DERIVADO, e a resposta dele o corpo do gatilho descarta: um
    /// aviso emitido la nunca chegaria a quem gravou.
    static CADEIA_CORTADA: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

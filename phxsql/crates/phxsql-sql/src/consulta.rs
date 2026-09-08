//! `consultar` -- a consulta COMPOSTA: `WITH`, subconsulta no `FROM`, `IN
//! (SELECT …)`, junção e janela. Os itens 4, 5, 8 e 9 do roteiro moram
//! aqui, porque os quatro produzem a MESMA operacao do protocolo.
//!
//! # Por que um modulo a parte, e nao dentro de `sintaxe`/`traduzir`
//!
//! O `SELECT` simples (`sintaxe::selecao`) e o caminho RAPIDO -- ele so
//! sabe falar com UMA tabela, e e por isso que `buscar`/`varrer`/`agrupar`
//! nunca precisam de um resolvedor de indices por FORA de si mesmos. Uma
//! consulta composta toca VARIAS tabelas (a principal, cada junção, cada
//! subconsulta), e cada uma delas pode ter indices diferentes -- que so o
//! SERVIDOR conhece. Por isso `traduzir_consulta` recebe um RESOLVEDOR
//! (`&mut dyn FnMut(&Selecao, &str) -> Result<Plano>`) em vez de uma lista
//! de indices: para cada pedaco (`de`, cada `juntar`, cada `escalar`, cada
//! `em`), ela chama o resolvedor, que faz o MESMO que o servidor ja faz
//! hoje para o `SELECT` simples -- olhar o esquema daquela tabela e montar
//! os `IndiceInfo` dela -- so que uma vez por tabela em vez de uma vez por
//! consulta.
//!
//! # Como o servidor chama isto
//!
//! ```text
//! match phxsql_sql::analisar_comando(&texto)? {
//!     Comando::Consulta(c) => {
//!         let plano = traduzir_consulta(&c, &database_corrente, &mut |selecao, db| {
//!             // o MESMO que ja faz para o SELECT simples: executar_derivado
//!             // ("esquema", ...) da tabela de `selecao.de`, indices_do_esquema,
//!             // e phxsql_sql::traduzir(selecao, &indices, db).
//!             self.traduzir_com_indices(selecao, db)
//!         })?;
//!         self.executar_derivado(&plano.op, &plano.pedido, sessao)
//!     }
//!     ...
//! }
//! ```
//!
//! # O que esta rodada NAO cobre, e recusa nomeando
//!
//! Correlação (subconsulta que cita coluna de fora) e `EXISTS` recusam --
//! exigiriam rodar a subconsulta por linha. `RIGHT`/`FULL`/`CROSS JOIN`
//! recusam -- a direita se escreve trocando os lados. Uma CTE so, nao
//! recursiva. E o WHERE composto so reconhece subconsulta (IN ou escalar)
//! quando ela e o conjunto INTEIRO de um `AND` de nivel superior -- uma
//! subconsulta dentro de `OR` ou de uma expressao maior nao e detectada, e
//! cai na recusa de "forma nao suportada" em vez de virar texto errado.

use crate::lexico::{self, normalizar_tokens, Simbolo, Token};
use crate::sintaxe::{
    Analisador, FuncaoAgregada, ItemProjetado, Onde, Ordenacao, Projecao, Selecao,
};
use crate::traduzir::{apelido_padrao, igual_sem_caso, Plano, Saida};
use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// Uma coluna da projecao composta: nome (pode vir qualificado, `p.id`, a
/// partir do item 8) e apelido opcional.
#[derive(Debug, Clone, PartialEq)]
pub struct ColunaComposta {
    pub coluna: String,
    pub apelido: Option<String>,
}

/// `WHERE coluna IN (SELECT campo FROM …)`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmSubconsulta {
    pub coluna: String,
    pub de: Selecao,
    pub campo: String,
}

/// `ROW_NUMBER() OVER ([PARTITION BY …] [ORDER BY …]) [AS apelido]` -- so
/// `row_number` nesta rodada (item 5). O `apelido` fica AQUI e TAMBEM entra
/// como uma coluna comum em `colunas`, na posicao em que a chamada apareceu
/// na projecao -- e assim que o pedido de `consultar` mostra os dois.
#[derive(Debug, Clone, PartialEq)]
pub struct Janela {
    pub funcao: String,
    pub particao: Vec<String>,
    pub ordem: Vec<Ordenacao>,
    pub apelido: String,
}

/// A consulta composta pronta para traduzir.
#[derive(Debug, Clone, PartialEq)]
pub struct Consulta {
    /// A fonte principal -- tabela, subconsulta ou o corpo de uma CTE.
    pub de: Selecao,
    /// `IN (SELECT …)` do WHERE.
    pub em: Vec<EmSubconsulta>,
    /// O resto do WHERE, sempre como TEXTO -- aqui nao existe "onde" (lista
    /// de filtro) separado de "expressao": tudo vira texto, simples ou nao.
    pub onde: Option<Onde>,
    /// `None` = `SELECT *` = todas as colunas (o pedido nao leva "colunas").
    pub colunas: Option<Vec<ColunaComposta>>,
    pub janela: Vec<Janela>,
    pub ordem: Vec<Ordenacao>,
    pub pular: u64,
    pub max: Option<u64>,
}

/// O resolvedor que quem chama fornece: dado um SELECT (de `de`, de um
/// `juntar`, de um `escalar` ou de um `em`) e o database corrente, devolve
/// o `Plano` dele -- com os indices DAQUELA tabela, que so o servidor
/// enxerga.
pub type Resolvedor<'a> = dyn FnMut(&Selecao, &str) -> Result<Plano> + 'a;

/// Traduz uma consulta composta para o pedido `consultar`.
pub fn traduzir_consulta(
    c: &Consulta,
    database_corrente: &str,
    resolver: &mut Resolvedor<'_>,
) -> Result<Plano> {
    let database = if c.de.de.database.is_empty() {
        database_corrente.to_string()
    } else {
        c.de.de.database.clone()
    };
    let mut notas = vec![
        "SELECT composto (WITH/subconsulta/IN/junção/janela) vira `consultar`: cada pedaco \
         roda pelo MESMO portao de permissao que qualquer outro pedido -- e o que faz da \
         composicao uma composicao, e nao uma porta dos fundos"
            .into(),
    ];

    let de_plano = resolver(&c.de, &database)?;
    let mut pares = vec![
        ("database".to_string(), Json::texto_de(&database)),
        ("de".to_string(), de_plano.pedido),
    ];

    if !c.em.is_empty() {
        let mut em_json = Vec::with_capacity(c.em.len());
        for e in &c.em {
            let ep = resolver(&e.de, &database)?;
            em_json.push(Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de(&e.coluna)),
                ("de".to_string(), ep.pedido),
                ("campo".to_string(), Json::texto_de(&e.campo)),
            ]));
        }
        pares.push(("em".to_string(), Json::Lista(em_json)));
        notas.push(format!(
            "{} IN (SELECT …) rodam ANTES, cada um pelo portao, e viram conjunto",
            c.em.len()
        ));
    }

    pares.push((
        "expressao".to_string(),
        Json::texto_de(c.onde.as_ref().map(Onde::texto).unwrap_or_default()),
    ));

    if !c.janela.is_empty() {
        pares.push((
            "janela".to_string(),
            Json::Lista(
                c.janela
                    .iter()
                    .map(|j| {
                        Json::Objeto(vec![
                            ("funcao".to_string(), Json::texto_de(&j.funcao)),
                            (
                                "particao".to_string(),
                                Json::Lista(j.particao.iter().map(Json::texto_de).collect()),
                            ),
                            (
                                "ordem".to_string(),
                                Json::Lista(
                                    j.ordem
                                        .iter()
                                        .map(|o| {
                                            Json::Objeto(vec![
                                                ("coluna".to_string(), Json::texto_de(&o.coluna)),
                                                ("desc".to_string(), Json::Bool(o.desc)),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                            ("apelido".to_string(), Json::texto_de(&j.apelido)),
                        ])
                    })
                    .collect(),
            ),
        ));
        notas.push(format!(
            "{} janela(s) ROW_NUMBER() OVER (...) -- so essa funcao nesta rodada",
            c.janela.len()
        ));
    }

    match &c.colunas {
        None => notas.push("SELECT * -- todas as colunas da linha composta".into()),
        Some(cols) => {
            pares.push((
                "colunas".to_string(),
                Json::Lista(
                    cols.iter()
                        .map(|c| match &c.apelido {
                            None => Json::texto_de(&c.coluna),
                            Some(a) => Json::Objeto(vec![
                                ("coluna".to_string(), Json::texto_de(&c.coluna)),
                                ("apelido".to_string(), Json::texto_de(a)),
                            ]),
                        })
                        .collect(),
                ),
            ));
        }
    }

    if !c.ordem.is_empty() {
        pares.push((
            "ordem".to_string(),
            Json::Lista(
                c.ordem
                    .iter()
                    .map(|o| {
                        Json::Objeto(vec![
                            ("coluna".to_string(), Json::texto_de(&o.coluna)),
                            ("desc".to_string(), Json::Bool(o.desc)),
                        ])
                    })
                    .collect(),
            ),
        ));
    }
    if c.pular > 0 {
        pares.push(("pular".to_string(), Json::de_u64(c.pular)));
    }
    if let Some(m) = c.max {
        pares.push(("max".to_string(), Json::de_u64(m)));
    }

    let saida = match &c.colunas {
        None => Saida::LinhaInteira,
        Some(cols) => Saida::Colunas(
            cols.iter()
                .map(|c| {
                    (
                        c.coluna.clone(),
                        c.apelido.clone().unwrap_or(c.coluna.clone()),
                    )
                })
                .collect(),
        ),
    };

    Ok(Plano {
        op: "consultar".into(),
        pedido: crate::traduzir::pedido_com_op("consultar", pares),
        saida,
        notas,
    })
}

/// O nome da unica coluna que uma subconsulta (IN ou escalar) projeta -- ou
/// a recusa, quando ela projeta zero, mais de uma, ou `*`.
pub(crate) fn campo_escalar(p: &Projecao) -> Result<String> {
    match p {
        Projecao::Tudo => Err(PhxError::Esquema(
            "subconsulta (IN ou escalar) com SELECT * nao projeta uma coluna so -- nomeie a \
             coluna"
                .into(),
        )),
        Projecao::Contagem => Ok(apelido_padrao(FuncaoAgregada::Contagem, None)),
        Projecao::Colunas(cs) if cs.len() == 1 => Ok(cs[0].rotulo().to_string()),
        Projecao::Colunas(_) => Err(PhxError::Esquema(
            "subconsulta (IN ou escalar) projeta mais de uma coluna -- nomeie uma so".into(),
        )),
        Projecao::Agregada(itens) if itens.len() == 1 => match &itens[0] {
            ItemProjetado::Coluna(c) => Ok(c.rotulo().to_string()),
            ItemProjetado::Agregado {
                funcao,
                coluna,
                apelido,
            } => Ok(apelido
                .clone()
                .unwrap_or_else(|| apelido_padrao(*funcao, coluna.as_deref()))),
        },
        Projecao::Agregada(_) => Err(PhxError::Esquema(
            "subconsulta (IN ou escalar) projeta mais de uma coluna -- nomeie uma so".into(),
        )),
    }
}

/// A subconsulta cita alguma coluna qualificada que NAO e a tabela dela
/// mesma? Isso e correlação -- ela precisaria rodar por LINHA da consulta
/// de fora, e esta rodada nao faz isso.
///
/// # O alcance desta deteccao
///
/// So olha o WHERE (`Onde::Expressao`; `Onde::Simples` nunca correlaciona,
/// porque `condicao()` ja descarta o qualificador da forma simples). Uma
/// correlação escondida em `HAVING` ou numa coluna calculada da projecao
/// nao e pega aqui -- o caso comum (e o unico que o contrato pede recusar)
/// e no WHERE.
pub(crate) fn recusa_se_correlacionada(sel: &Selecao) -> Result<()> {
    let Some(Onde::Expressao(texto)) = &sel.onde else {
        return Ok(());
    };
    let proprio = sel.de.apelido.as_deref().unwrap_or(&sel.de.tabela);
    for pedaco in texto.split(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_')) {
        if let Some((qualificador, _resto)) = pedaco.split_once('.') {
            if !qualificador.is_empty() && !igual_sem_caso(qualificador, proprio) {
                return Err(PhxError::Esquema(format!(
                    "subconsulta correlacionada (cita {qualificador:?}, que nao e a tabela \
                     dela -- {proprio:?}) recusa nomeando: correlação exigiria rodar a \
                     subconsulta por linha da consulta de fora, e esta rodada nao faz isso"
                )));
            }
        }
    }
    Ok(())
}

// -------------------------------------------------------------- parser

impl Analisador {
    /// `WITH nome AS (SELECT …) SELECT …` -- so UMA CTE, nao recursiva.
    pub(crate) fn com_cte(&mut self) -> Result<Consulta> {
        if self.aceitar_palavra("RECURSIVE") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "WITH RECURSIVE nao existe nesta rodada -- so WITH de uma CTE nao recursiva",
            ));
        }
        let nome = self.identificador("nome da CTE")?;
        self.exigir_palavra("AS")?;
        if !self.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava ( depois do AS da CTE: WITH nome AS (SELECT ...)",
            ));
        }
        self.exigir_palavra("SELECT")?;
        let corpo = self.selecao()?;
        if !self.aceitar(&Token::FechaParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava ) fechando a CTE",
            ));
        }
        if self.aceitar(&Token::Virgula) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "uma CTE so nesta rodada -- duas CTEs (ou WITH RECURSIVE) recusam nomeando",
            ));
        }
        self.exigir_palavra("SELECT")?;
        self.consulta_apos_select(Some((nome, corpo)))
    }

    /// Sem consumir nada: este `SELECT` (ja com o `SELECT` consumido)
    /// precisa da gramatica composta? So dois sinais nesta rodada: `FROM (`
    /// (subconsulta) e `IN (SELECT` (IN de subconsulta) -- junção (item 8)
    /// e janela (item 5) entram nesta mesma sondagem quando chegarem.
    pub(crate) fn precisa_de_consulta_composta(&self) -> bool {
        let mut i = self.i;
        while let Some(s) = self.s.get(i) {
            if matches!(s.token, Token::PontoEVirgula) {
                break;
            }
            // `(SELECT` em QUALQUER profundidade, precedido de QUALQUER
            // coisa -- `FROM (`, `IN (SELECT`, `EXISTS (SELECT`, ou uma
            // comparacao escalar (`coluna > (SELECT ...)`, item 9). Nao
            // filtrar pelo que vem antes e deliberado: uma forma que esta
            // rodada nao entende (ainda) e melhor cair na gramatica
            // composta -- que recusa NOMEANDO o motivo -- do que escorregar
            // pelo caminho velho e virar texto de expressao que o
            // avaliador do motor nunca vai conseguir ler.
            if matches!(s.token, Token::AbreParen)
                && self
                    .s
                    .get(i + 1)
                    .and_then(|s| s.token.palavra_chave())
                    .as_deref()
                    == Some("SELECT")
            {
                return true;
            }
            // `) OVER` -- uma janela (item 5), `ROW_NUMBER()` ou outra.
            // Detectar TODA funcao com OVER (nao so ROW_NUMBER) e
            // deliberado, pelo mesmo motivo do `(SELECT` acima: RANK/
            // SUM() OVER ainda nao existem aqui, e a recusa nomeada vive na
            // gramatica composta, nao no caminho velho.
            if matches!(s.token, Token::FechaParen)
                && self
                    .s
                    .get(i + 1)
                    .and_then(|s| s.token.palavra_chave())
                    .as_deref()
                    == Some("OVER")
            {
                return true;
            }
            i += 1;
        }
        false
    }

    /// O corpo da consulta composta, com `SELECT` (e, se houver, o `WITH`)
    /// ja consumidos.
    pub(crate) fn consulta_apos_select(
        &mut self,
        cte: Option<(String, Selecao)>,
    ) -> Result<Consulta> {
        let (colunas, janela) = self.projecao_composta()?;
        self.exigir_palavra("FROM")?;
        let de = self.fonte_do_from(&cte)?;

        let (onde, em) = self.onde_composta()?;

        let ordem = if self.aceitar_palavra("ORDER") {
            self.exigir_palavra("BY")?;
            self.lista_de_ordenacoes()?
        } else {
            Vec::new()
        };

        let mut pular = 0u64;
        let mut max = None;
        if self.aceitar_palavra("LIMIT") {
            max = Some(self.inteiro("o limite do LIMIT")?);
            if self.aceitar_palavra("OFFSET") {
                pular = self.inteiro("o salto do OFFSET")?;
            }
        } else if self.aceitar_palavra("OFFSET") {
            pular = self.inteiro("o salto do OFFSET")?;
        }

        Ok(Consulta {
            de,
            em,
            onde,
            colunas,
            janela,
            ordem,
            pular,
            max,
        })
    }

    /// A projecao da consulta composta: `*`, ou uma lista de colunas (pode
    /// vir qualificada, `p.id`) e/ou chamadas de janela
    /// (`ROW_NUMBER() OVER (...)`). `*` nao se mistura com janela -- exige
    /// a lista explicita, porque `SELECT *, ROW_NUMBER() ...` teria de
    /// nomear a posicao da coluna nova, e `*` nao nomeia nada.
    fn projecao_composta(&mut self) -> Result<(Option<Vec<ColunaComposta>>, Vec<Janela>)> {
        if self.aceitar(&Token::Asterisco) {
            return Ok((None, Vec::new()));
        }
        let mut colunas = Vec::new();
        let mut janelas = Vec::new();
        loop {
            let (col, jan) = self.item_de_projecao_composta()?;
            colunas.push(col);
            if let Some(j) = jan {
                janelas.push(j);
            }
            if !self.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok((Some(colunas), janelas))
    }

    /// Um item da projecao composta: coluna simples, ou
    /// `ROW_NUMBER() OVER (...) [AS apelido]` -- que sempre devolve UMA
    /// coluna (o apelido da janela, para `colunas` mostrar na posicao em
    /// que a chamada apareceu) e, quando for janela, TAMBEM a `Janela`.
    fn item_de_projecao_composta(&mut self) -> Result<(ColunaComposta, Option<Janela>)> {
        if let Some(nome) = self.espiar().and_then(|s| s.token.palavra_chave()) {
            if self.s.get(self.i + 1).map(|s| &s.token) == Some(&Token::AbreParen) {
                if nome == "ROW_NUMBER" {
                    let (janela, coluna) = self.chamada_de_row_number()?;
                    return Ok((coluna, Some(janela)));
                }
                let pos_func = self.posicao_atual();
                let fechamento = self.posicao_do_fecha_parenteses_apos(self.i + 1)?;
                if self
                    .s
                    .get(fechamento + 1)
                    .and_then(|s| s.token.palavra_chave())
                    .as_deref()
                    == Some("OVER")
                {
                    return Err(lexico::erro(
                        pos_func,
                        &format!(
                            "{nome}() OVER (...) nao tem substrato nesta rodada -- so \
                             ROW_NUMBER() OVER (...)"
                        ),
                    ));
                }
            }
        }
        let nome = self.identificador("nome de coluna")?;
        let nome = if self.aceitar(&Token::Ponto) {
            format!(
                "{nome}.{}",
                self.identificador("nome de coluna depois do ponto")?
            )
        } else {
            nome
        };
        let apelido = if self.aceitar_palavra("AS") {
            Some(self.identificador("apelido depois de AS")?)
        } else {
            None
        };
        Ok((
            ColunaComposta {
                coluna: nome,
                apelido,
            },
            None,
        ))
    }

    /// Depois de espiar `ROW_NUMBER (` sem consumir.
    fn chamada_de_row_number(&mut self) -> Result<(Janela, ColunaComposta)> {
        self.i += 2; // ROW_NUMBER e o (
        if !self.aceitar(&Token::FechaParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "ROW_NUMBER nao aceita argumento: ROW_NUMBER() OVER (...)",
            ));
        }
        self.exigir_palavra("OVER")?;
        if !self.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava ( depois de OVER",
            ));
        }
        let particao = if self.aceitar_palavra("PARTITION") {
            self.exigir_palavra("BY")?;
            self.lista_de_colunas_do_group_by()?
        } else {
            Vec::new()
        };
        let ordem = if self.aceitar_palavra("ORDER") {
            self.exigir_palavra("BY")?;
            self.lista_de_ordenacoes()?
        } else {
            Vec::new()
        };
        if !self.aceitar(&Token::FechaParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava ) fechando o OVER (...)",
            ));
        }
        let apelido = if self.aceitar_palavra("AS") {
            self.identificador("apelido de ROW_NUMBER")?
        } else {
            "row_number".to_string()
        };
        let janela = Janela {
            funcao: "row_number".to_string(),
            particao,
            ordem,
            apelido: apelido.clone(),
        };
        let coluna = ColunaComposta {
            coluna: apelido,
            apelido: None,
        };
        Ok((janela, coluna))
    }

    /// O indice do `)` que fecha o `(` na posicao `abre_idx` -- sem
    /// consumir nada (pura sondagem, para decidir se uma funcao tem `OVER`
    /// depois dela).
    fn posicao_do_fecha_parenteses_apos(&self, abre_idx: usize) -> Result<usize> {
        let mut profundidade = 0i32;
        let mut i = abre_idx;
        while let Some(s) = self.s.get(i) {
            match &s.token {
                Token::AbreParen => profundidade += 1,
                Token::FechaParen => {
                    profundidade -= 1;
                    if profundidade == 0 {
                        return Ok(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Err(lexico::erro(
            self.posicao_atual(),
            "parenteses aberto e nao fechado",
        ))
    }

    /// O `FROM`: uma tabela de sempre, o nome de uma CTE (se uma foi
    /// declarada), ou `(SELECT …) AS apelido`.
    fn fonte_do_from(&mut self, cte: &Option<(String, Selecao)>) -> Result<Selecao> {
        if self.aceitar(&Token::AbreParen) {
            self.exigir_palavra("SELECT")?;
            let dentro = self.selecao()?;
            if !self.aceitar(&Token::FechaParen) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "esperava ) fechando a subconsulta do FROM",
                ));
            }
            self.aceitar_palavra("AS");
            // O apelido e obrigatorio -- uma subconsulta sem nome nao tem
            // como ser referenciada por ninguem (nem esta rodada precisa
            // dela para nada ainda, mas a exigencia evita `FROM (SELECT
            // ...) WHERE ...` calado, que e erro de digitacao mais vezes
            // do que intencao).
            self.identificador("apelido da subconsulta do FROM (obrigatorio)")?;
            return Ok(dentro);
        }
        let alvo = self.alvo()?;
        if let Some((nome_cte, corpo)) = cte {
            if alvo.database.is_empty()
                && alvo.schema.is_empty()
                && igual_sem_caso(nome_cte, &alvo.tabela)
            {
                return Ok(corpo.clone());
            }
        }
        Ok(Selecao {
            projecao: Projecao::Tudo,
            de: alvo,
            onde: None,
            agrupar_por: Vec::new(),
            tendo: None,
            ordem: None,
            ordem_lista: Vec::new(),
            limite: None,
            salto: 0,
        })
    }

    /// O `WHERE` composto: cada conjunto de nivel superior (dividido por
    /// `AND`) e classificado -- `coluna IN (SELECT …)` vira `em`; qualquer
    /// outra coisa vira texto (ANDado de volta na `expressao`).
    fn onde_composta(&mut self) -> Result<(Option<Onde>, Vec<EmSubconsulta>)> {
        if !self.aceitar_palavra("WHERE") {
            return Ok((None, Vec::new()));
        }
        let tokens = self.capturar_ate_clausula(&["ORDER", "LIMIT", "OFFSET"])?;
        if tokens.is_empty() {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava uma condicao depois de WHERE",
            ));
        }
        let conjuntos = dividir_por_and(tokens);
        let mut fragmentos = Vec::new();
        let mut em = Vec::new();
        for conj in conjuntos {
            if let Some(e) = tentar_in_subconsulta(&conj)? {
                em.push(e);
                continue;
            }
            recusar_forma_nao_suportada(&conj)?;
            fragmentos.push(normalizar_tokens(&conj));
        }
        let onde = if fragmentos.is_empty() {
            None
        } else {
            Some(Onde::Expressao(fragmentos.join(" AND ")))
        };
        Ok((onde, em))
    }
}

/// Divide os tokens de uma clausula pelos `AND` de NIVEL SUPERIOR (fora de
/// parenteses) -- serve para o `WHERE` composto (item 4/9) e para o `ON`
/// de junção (item 8), que sao as duas gramaticas desta camada que tratam
/// AND como lista em vez de arvore.
pub(crate) fn dividir_por_and(tokens: Vec<Simbolo>) -> Vec<Vec<Simbolo>> {
    let mut partes = Vec::new();
    let mut atual = Vec::new();
    let mut profundidade = 0i32;
    for s in tokens {
        match &s.token {
            Token::AbreParen => profundidade += 1,
            Token::FechaParen => profundidade -= 1,
            _ => {}
        }
        if profundidade == 0 && s.token.palavra_chave().as_deref() == Some("AND") {
            partes.push(std::mem::take(&mut atual));
            continue;
        }
        atual.push(s);
    }
    partes.push(atual);
    partes
}

/// `coluna[.coluna] IN ( SELECT ... )`, ocupando o conjunto INTEIRO -- ou
/// `None` quando o conjunto nao e essa forma (e ai quem chama tenta o
/// resto: escalar, ou texto).
fn tentar_in_subconsulta(conj: &[Simbolo]) -> Result<Option<EmSubconsulta>> {
    let mut i = 0usize;
    let Some(Token::Palavra {
        texto: primeiro,
        citado: false,
    }) = conj.first().map(|s| &s.token)
    else {
        return Ok(None);
    };
    let mut coluna = primeiro.clone();
    i += 1;
    if matches!(conj.get(i).map(|s| &s.token), Some(Token::Ponto)) {
        let Some(Token::Palavra {
            texto: segunda,
            citado: false,
        }) = conj.get(i + 1).map(|s| &s.token)
        else {
            return Ok(None);
        };
        coluna = format!("{coluna}.{segunda}");
        i += 2;
    }
    if conj.get(i).and_then(|s| s.token.palavra_chave()).as_deref() != Some("IN") {
        return Ok(None);
    }
    i += 1;
    if !matches!(conj.get(i).map(|s| &s.token), Some(Token::AbreParen)) {
        return Ok(None);
    }
    i += 1;
    if conj.get(i).and_then(|s| s.token.palavra_chave()).as_deref() != Some("SELECT") {
        return Ok(None);
    }
    i += 1;
    let Some(ultimo) = conj.last() else {
        return Ok(None);
    };
    if !matches!(ultimo.token, Token::FechaParen) || i >= conj.len() - 1 {
        return Ok(None);
    }
    let interior = conj[i..conj.len() - 1].to_vec();
    let mut sub = Analisador { s: interior, i: 0 };
    let dentro = sub.selecao()?;
    if sub.espiar().is_some() {
        return Err(lexico::erro(
            sub.posicao_atual(),
            "sobrou algo dentro do IN (SELECT ...)",
        ));
    }
    recusa_se_correlacionada(&dentro)?;
    let campo = campo_escalar(&dentro.projecao)?;
    Ok(Some(EmSubconsulta {
        coluna,
        de: dentro,
        campo,
    }))
}

/// `EXISTS (...)` e um `SELECT` solto (nao reconhecido como IN/escalar) nao
/// tem substrato nesta rodada -- e a recusa nomeia isso, em vez de deixar
/// o texto virar uma expressao que o avaliador do motor nao entende.
fn recusar_forma_nao_suportada(conj: &[Simbolo]) -> Result<()> {
    for s in conj {
        if let Some(p) = s.token.palavra_chave() {
            if p == "EXISTS" {
                return Err(lexico::erro(
                    s.posicao,
                    "EXISTS recusa nomeando nesta rodada: exigiria rodar a subconsulta por \
                     linha da consulta de fora",
                ));
            }
            if p == "SELECT" {
                return Err(lexico::erro(
                    s.posicao,
                    "esta forma de subconsulta no WHERE ainda nao existe nesta rodada -- so \
                     IN (SELECT ...) e comparacao com subconsulta escalar (coluna op \
                     (SELECT ...))",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sintaxe::{analisar_comando, Comando};
    use crate::traduzir::{ColunaDoIndice, IndiceInfo};

    fn consulta(sql: &str) -> Consulta {
        match analisar_comando(sql).unwrap() {
            Comando::Consulta(c) => c,
            outro => panic!("{sql} nao deu Consulta: {outro:?}"),
        }
    }

    fn recusa(sql: &str) -> String {
        analisar_comando(sql).unwrap_err().to_string()
    }

    /// O resolvedor de teste: qualquer tabela tem so o indice `porId`, uma
    /// coluna, primario e unico -- o bastante para o `SELECT *` de dentro
    /// de cada pedaco traduzir sem recusar por falta de indice.
    fn resolver_simples(sel: &Selecao, db: &str) -> Result<Plano> {
        let ix = vec![IndiceInfo {
            nome: "porId".into(),
            colunas: vec![ColunaDoIndice {
                nome: "id".into(),
                desc: false,
            }],
            unico: true,
            primario: true,
        }];
        crate::traduzir::traduzir(sel, &ix, db)
    }

    // --------------------------------------------------------------- WITH

    #[test]
    fn with_cte_e_select_de_fora_viram_consultar() {
        let c = consulta("WITH x AS (SELECT id, nome FROM clientes) SELECT id FROM x");
        // A fonte principal e o CORPO da CTE, nao uma tabela chamada "x".
        assert_eq!(c.de.de.tabela, "clientes");
        let Projecao::Colunas(cs) = &c.de.projecao else {
            panic!("esperava Colunas")
        };
        assert_eq!(cs.len(), 2);
        assert_eq!(
            c.colunas,
            Some(vec![ColunaComposta {
                coluna: "id".into(),
                apelido: None
            }])
        );
    }

    #[test]
    fn with_recursive_recusa_nomeando() {
        let e = recusa("WITH RECURSIVE x AS (SELECT id FROM t) SELECT id FROM x");
        assert!(e.contains("RECURSIVE"), "{e}");
    }

    #[test]
    fn duas_ctes_recusam_nomeando() {
        let e = recusa("WITH a AS (SELECT id FROM t), b AS (SELECT id FROM u) SELECT id FROM a");
        assert!(e.contains("uma CTE so"), "{e}");
    }

    #[test]
    fn with_traduz_para_consultar_com_de_pronto() {
        let c = consulta("WITH x AS (SELECT id FROM clientes) SELECT id FROM x WHERE id > 1");
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert_eq!(p.op, "consultar");
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
        let de = p.pedido.campo("de").unwrap();
        assert_eq!(de.texto_ou("op", ""), "varrer");
        assert_eq!(de.texto_ou("tabela", ""), "clientes");
        assert_eq!(p.pedido.texto_ou("expressao", ""), "id > 1");
        assert_eq!(
            p.pedido.campo("colunas").unwrap(),
            &Json::Lista(vec![Json::texto_de("id")])
        );
    }

    // -------------------------------------------------------- subconsulta FROM

    #[test]
    fn subconsulta_no_from_com_apelido() {
        let c = consulta("SELECT id FROM (SELECT id, nome FROM clientes) AS x");
        assert_eq!(c.de.de.tabela, "clientes");
    }

    #[test]
    fn subconsulta_no_from_sem_apelido_recusa() {
        let e = recusa("SELECT id FROM (SELECT id FROM clientes) WHERE id > 1");
        // sem AS nem identificador -- "WHERE" e clausula e nao serve de
        // apelido, entao a recusa nomeia o apelido que faltou.
        assert!(e.contains("apelido"), "{e}");
    }

    // ------------------------------------------------------------ IN subquery

    #[test]
    fn where_in_select_vira_em() {
        let c = consulta("SELECT * FROM pedidos WHERE cliente_id IN (SELECT id FROM clientes)");
        assert_eq!(c.em.len(), 1);
        assert_eq!(c.em[0].coluna, "cliente_id");
        assert_eq!(c.em[0].campo, "id");
        assert_eq!(c.em[0].de.de.tabela, "clientes");
        assert!(
            c.onde.is_none(),
            "o IN sozinho nao deixa sobra na expressao"
        );
    }

    #[test]
    fn in_subquery_combinado_com_outra_condicao_and() {
        let c = consulta(
            "SELECT * FROM pedidos WHERE ativo = TRUE AND cliente_id IN (SELECT id FROM \
             clientes) AND total > 10",
        );
        assert_eq!(c.em.len(), 1);
        let Some(Onde::Expressao(texto)) = &c.onde else {
            panic!("esperava Expressao")
        };
        assert_eq!(texto, "ativo = TRUE AND total > 10");
    }

    #[test]
    fn in_subquery_traduz_com_em_no_json() {
        let c = consulta("SELECT * FROM pedidos WHERE cliente_id IN (SELECT id FROM clientes)");
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        let em = p.pedido.campo("em").unwrap().lista().unwrap();
        assert_eq!(em.len(), 1);
        assert_eq!(em[0].texto_ou("coluna", ""), "cliente_id");
        assert_eq!(em[0].texto_ou("campo", ""), "id");
        assert_eq!(
            em[0].campo("de").unwrap().texto_ou("tabela", ""),
            "clientes"
        );
    }

    #[test]
    fn in_de_lista_de_literais_nao_e_subconsulta() {
        // `id IN (1,2)` sozinho e o item 2 (varrer.expressao) -- so um IN de
        // SUBCONSULTA justifica a gramatica composta.
        let s = crate::sintaxe::analisar("SELECT * FROM t WHERE id IN (1,2)").unwrap();
        assert!(matches!(s.onde, Some(Onde::Expressao(_))));
    }

    #[test]
    fn subconsulta_do_in_com_mais_de_uma_coluna_recusa() {
        let e = recusa("SELECT * FROM pedidos WHERE id IN (SELECT id, nome FROM clientes)");
        assert!(e.contains("mais de uma coluna"), "{e}");
    }

    #[test]
    fn exists_recusa_nomeando() {
        let e = recusa("SELECT * FROM pedidos WHERE EXISTS (SELECT 1 FROM clientes)");
        assert!(e.contains("EXISTS"), "{e}");
    }

    #[test]
    fn subconsulta_correlacionada_recusa_nomeando() {
        let e = recusa(
            "SELECT * FROM pedidos p WHERE p.cliente_id IN (SELECT id FROM clientes c WHERE \
             c.cidade = p.cidade)",
        );
        assert!(e.contains("correlacionada"), "{e}");
    }

    #[test]
    fn subconsulta_nao_correlacionada_com_qualificador_proprio_passa() {
        // c.id referencia a PROPRIA tabela da subconsulta -- nao e correlacao.
        let c = consulta(
            "SELECT * FROM pedidos WHERE cliente_id IN (SELECT c.id FROM clientes c WHERE \
             c.ativo = TRUE)",
        );
        assert_eq!(c.em[0].campo, "id");
    }

    // ------------------------------------------------------------ ordem/limit

    #[test]
    fn ordem_multipla_e_limit_offset() {
        let c = consulta(
            "SELECT id FROM (SELECT id FROM t) AS x ORDER BY id DESC, id ASC LIMIT 10 OFFSET 5",
        );
        assert_eq!(
            c.ordem,
            vec![
                Ordenacao {
                    coluna: "id".into(),
                    desc: true
                },
                Ordenacao {
                    coluna: "id".into(),
                    desc: false
                },
            ]
        );
        assert_eq!(c.max, Some(10));
        assert_eq!(c.pular, 5);
    }

    // ------------------------------------------------------------- projecao

    #[test]
    fn select_estrela_composto_nao_leva_colunas_no_pedido() {
        let c = consulta("SELECT * FROM (SELECT id FROM t) AS x");
        assert_eq!(c.colunas, None);
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert!(p.pedido.campo("colunas").is_none());
        assert_eq!(p.saida, Saida::LinhaInteira);
    }

    #[test]
    fn coluna_com_apelido_na_projecao_composta() {
        let c = consulta("SELECT nome AS n FROM (SELECT nome FROM t) AS x");
        assert_eq!(
            c.colunas,
            Some(vec![ColunaComposta {
                coluna: "nome".into(),
                apelido: Some("n".into())
            }])
        );
    }

    // --------------------------------------------------------- o portao

    /// A prova que o contrato pede: cada `de` roda pelo resolvedor -- se o
    /// resolvedor recusar UMA tabela (o portao de permissao, do lado do
    /// servidor), a consulta INTEIRA recusa, e nao finge que a tabela
    /// negada simplesmente nao devolveu linha nenhuma.
    #[test]
    fn tabela_negada_no_em_recusa_a_consulta_inteira() {
        let c = consulta("SELECT * FROM pedidos WHERE cliente_id IN (SELECT id FROM clientes)");
        let mut chamadas = 0;
        let mut resolver_com_negacao = |sel: &Selecao, db: &str| -> Result<Plano> {
            chamadas += 1;
            if sel.de.tabela == "clientes" {
                return Err(PhxError::Esquema(
                    "tabela clientes negada para este usuario".into(),
                ));
            }
            resolver_simples(sel, db)
        };
        let e = traduzir_consulta(&c, "loja", &mut resolver_com_negacao)
            .unwrap_err()
            .to_string();
        assert!(e.contains("negada"), "{e}");
        assert_eq!(
            chamadas, 2,
            "de + em -- as duas tabelas passam pelo resolvedor"
        );
    }

    // ------------------------------------------------------- item 5: janela

    #[test]
    fn row_number_com_particao_e_ordem() {
        let c = consulta(
            "SELECT id, nome, ROW_NUMBER() OVER (PARTITION BY cidade ORDER BY id DESC) AS n \
             FROM (SELECT id, nome, cidade FROM clientes) AS x",
        );
        assert_eq!(c.janela.len(), 1);
        let j = &c.janela[0];
        assert_eq!(j.funcao, "row_number");
        assert_eq!(j.particao, vec!["cidade".to_string()]);
        assert_eq!(
            j.ordem,
            vec![Ordenacao {
                coluna: "id".into(),
                desc: true
            }]
        );
        assert_eq!(j.apelido, "n");
        // A coluna da janela entra em `colunas`, na posicao em que apareceu.
        assert_eq!(
            c.colunas,
            Some(vec![
                ColunaComposta {
                    coluna: "id".into(),
                    apelido: None
                },
                ColunaComposta {
                    coluna: "nome".into(),
                    apelido: None
                },
                ColunaComposta {
                    coluna: "n".into(),
                    apelido: None
                },
            ])
        );
    }

    #[test]
    fn row_number_sem_apelido_usa_o_padrao() {
        let c = consulta("SELECT id, ROW_NUMBER() OVER (ORDER BY id) FROM (SELECT id FROM t) AS x");
        assert_eq!(c.janela[0].apelido, "row_number");
        assert_eq!(c.janela[0].particao, Vec::<String>::new());
        assert_eq!(
            c.colunas.unwrap()[1],
            ColunaComposta {
                coluna: "row_number".into(),
                apelido: None
            }
        );
    }

    #[test]
    fn row_number_sem_particao_nem_ordem() {
        let c = consulta("SELECT id, ROW_NUMBER() OVER () AS n FROM (SELECT id FROM t) AS x");
        assert_eq!(c.janela[0].particao, Vec::<String>::new());
        assert_eq!(c.janela[0].ordem, Vec::<Ordenacao>::new());
    }

    #[test]
    fn rank_e_sum_over_recusam_pelo_nome() {
        for sql in [
            "SELECT RANK() OVER (ORDER BY id) FROM (SELECT id FROM t) AS x",
            "SELECT DENSE_RANK() OVER (ORDER BY id) FROM (SELECT id FROM t) AS x",
            "SELECT SUM(preco) OVER (ORDER BY id) FROM (SELECT id, preco FROM t) AS x",
        ] {
            let e = recusa(sql);
            assert!(e.contains("so ROW_NUMBER"), "{sql} -> {e}");
        }
    }

    #[test]
    fn row_number_com_argumento_recusa() {
        let e = recusa("SELECT ROW_NUMBER(id) OVER (ORDER BY id) FROM (SELECT id FROM t) AS x");
        assert!(e.contains("nao aceita argumento"), "{e}");
    }

    #[test]
    fn row_number_traduz_com_o_json_do_contrato() {
        let c = consulta(
            "SELECT id, nome, ROW_NUMBER() OVER (PARTITION BY cidade ORDER BY id) AS n FROM \
             (SELECT id, nome, cidade FROM clientes) AS x",
        );
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        let janela = p.pedido.campo("janela").unwrap().lista().unwrap();
        assert_eq!(janela.len(), 1);
        assert_eq!(janela[0].texto_ou("funcao", ""), "row_number");
        assert_eq!(
            janela[0].campo("particao").unwrap(),
            &Json::Lista(vec![Json::texto_de("cidade")])
        );
        assert_eq!(janela[0].texto_ou("apelido", ""), "n");
        assert_eq!(
            p.pedido.campo("colunas").unwrap(),
            &Json::Lista(vec![
                Json::texto_de("id"),
                Json::texto_de("nome"),
                Json::texto_de("n"),
            ])
        );
    }

    #[test]
    fn selecao_simples_com_row_number_nao_muda() {
        // ROW_NUMBER fora desta gramatica (sem FROM composto) tambem
        // precisa desviar -- e a prova de que a sondagem de `) OVER` funciona
        // sozinha, sem FROM(SELECT nem IN(SELECT por perto.
        let c = consulta("SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS n FROM clientes");
        assert_eq!(c.de.de.tabela, "clientes");
        assert_eq!(c.janela[0].apelido, "n");
    }
}

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
//! `RIGHT`/`FULL`/`CROSS JOIN` (pedido 236) e `EXISTS`/`NOT EXISTS`
//! correlacionado POR IGUALDADE (idem) ganharam substrato nesta rodada --
//! quatro tipos de junção 1:1, e semijuncao por espalhamento no `existe`.
//! O que continua faltando, e ainda recusa nomeando:
//!
//! - **Correlação que nao e igualdade** -- `IN (SELECT …)` correlacionado,
//!   subconsulta ESCALAR correlacionada, e um termo de `EXISTS` que cita
//!   coluna de fora sem ser `fora.col = dentro.col` (op diferente de `=`,
//!   os dois lados de fora, ou o termo inteiro dentro de um `OR`) --
//!   exigiriam rodar a subconsulta por LINHA da consulta de fora.
//! - **`EXISTS` NAO correlacionado** -- "tem linha?" sem nenhum par
//!   `fora.col = dentro.col` ainda nao tem substrato.
//! - Uma CTE so, nao recursiva.
//!
//! E o WHERE composto so reconhece subconsulta (IN, escalar ou EXISTS)
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

/// `interno` (so quem casa), `esquerdo`/`direito` (a linha do lado que fica
/// entra com as colunas do outro lado nulas quando nao casa), `completo`
/// (a uniao dos dois) e `cruzado` (produto, sem `ON`). Os quatro sao 1:1 no
/// tradutor desde o pedido 236 -- quem decide se `direito`/`completo`/
/// `cruzado` tem substrato no `consultar` e o MOTOR (outra frente, mesmo
/// pedido); aqui so a traducao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoJuncao {
    Interno,
    Esquerdo,
    Direito,
    Completo,
    Cruzado,
}

impl TipoJuncao {
    pub fn nome_no_protocolo(&self) -> &'static str {
        match self {
            TipoJuncao::Interno => "interno",
            TipoJuncao::Esquerdo => "esquerdo",
            TipoJuncao::Direito => "direito",
            TipoJuncao::Completo => "completo",
            TipoJuncao::Cruzado => "cruzado",
        }
    }
}

/// Uma junção -- `[INNER|LEFT [OUTER]] JOIN fonte ON em {AND em}`. Item 8.
#[derive(Debug, Clone, PartialEq)]
pub struct Juncao {
    pub de: Selecao,
    /// O apelido dela -- default e o nome da tabela, como o `de` principal.
    pub apelido: Option<String>,
    pub tipo: TipoJuncao,
    /// Pares de coluna=coluna do `ON` -- igualdade, por espalhamento em
    /// memoria. Qualquer condicao do `ON` que NAO seja essa forma vira
    /// fragmento de texto, ANDado na `expressao` de fora junto com o
    /// WHERE (nao ha campo `expressao` proprio por junção no contrato).
    pub em: Vec<(String, String)>,
}

/// `[NOT] EXISTS (SELECT … FROM t [AS x] WHERE …)` do WHERE -- semijuncao
/// por espalhamento (pedido 236). So a forma correlacionada por IGUALDADE
/// tem substrato: `em` guarda os pares `fora.col = dentro.col` (esquerda e
/// sempre a coluna de FORA, direita a de DENTRO, na mesma convencao do `em`
/// de `Juncao`), e nunca vem vazio -- sem par nenhum a forma recusa
/// nomeando antes de existir um `Existe` para construir.
#[derive(Debug, Clone, PartialEq)]
pub struct Existe {
    /// O `FROM t [AS x] WHERE ...` de dentro -- roda por `executar_derivado`
    /// como qualquer outro pedaco (o `de` de uma junção, um `escalar`...):
    /// o portao de permissao e um so.
    pub de: Selecao,
    /// O apelido do lado de dentro -- sempre presente (o nome da tabela
    /// quando nao ha `AS`), porque e ele que decide se um `nome.coluna` do
    /// WHERE de dentro e de fora ou de dentro.
    pub apelido: Option<String>,
    pub em: Vec<(String, String)>,
    /// `true` para `NOT EXISTS`.
    pub nao: bool,
}

/// A consulta composta pronta para traduzir.
#[derive(Debug, Clone, PartialEq)]
pub struct Consulta {
    /// A fonte principal -- tabela, subconsulta ou o corpo de uma CTE.
    pub de: Selecao,
    /// O apelido do `de` -- so entra no pedido quando ha junção (sem
    /// junção nenhuma coluna precisa de prefixo).
    pub apelido_de: Option<String>,
    pub juntar: Vec<Juncao>,
    /// `IN (SELECT …)` do WHERE.
    pub em: Vec<EmSubconsulta>,
    /// `[NOT] EXISTS (SELECT …)` do WHERE (pedido 236) -- aplicado DEPOIS
    /// de `juntar`/`escalar` e ANTES da `expressao`, filtrando linha por
    /// semijuncao sem acrescentar coluna nenhuma.
    pub existe: Vec<Existe>,
    /// `coluna OP (SELECT …)` do WHERE -- subconsulta ESCALAR (item 9),
    /// numerada `sub_1`, `sub_2`... na ordem em que apareceu. A comparacao
    /// vira `coluna OP sub_N` dentro da `expressao`.
    pub escalar: Vec<Escalar>,
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

/// `coluna OP (SELECT …)` -- subconsulta escalar, NAO correlacionada, que
/// devolve exatamente uma linha e uma coluna. `nome` (`sub_1`, `sub_2`...)
/// e o que aparece na `expressao` de fora no lugar da subconsulta.
#[derive(Debug, Clone, PartialEq)]
pub struct Escalar {
    pub nome: String,
    pub de: Selecao,
    pub campo: String,
}

/// O resolvedor que quem chama fornece: dado um SELECT (de `de`, de um
/// `juntar`, de um `escalar` ou de um `em`) e o database corrente, devolve
/// o `Plano` dele -- com os indices DAQUELA tabela, que so o servidor
/// enxerga.
pub type Resolvedor<'a> = dyn FnMut(&Selecao, &str) -> Result<Plano> + 'a;

/// Os pares `em` de um `ON` (esquerda, direita) e os fragmentos de texto
/// que sobraram (nao eram igualdade de colunas) -- o que
/// `condicao_de_juncao` devolve.
type ParesEFragmentosDoOn = (Vec<(String, String)>, Vec<String>);

/// O que o `WHERE` composto separa: o texto que sobrou (`onde`), o `IN
/// (SELECT ...)`, a subconsulta ESCALAR e o `EXISTS`/`NOT EXISTS` -- o que
/// `onde_composta` devolve.
type PartesDoWhereComposto = (Option<Onde>, Vec<EmSubconsulta>, Vec<Escalar>, Vec<Existe>);

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

    if !c.juntar.is_empty() {
        // O apelido do `de` so importa quando ha junção -- e so entao que
        // uma coluna precisa de prefixo para dizer de qual lado ela vem.
        if let Some(a) = &c.apelido_de {
            pares.push(("apelido".to_string(), Json::texto_de(a)));
        }
        let mut juntar_json = Vec::with_capacity(c.juntar.len());
        for j in &c.juntar {
            let jp = resolver(&j.de, &database)?;
            let mut par = vec![("de".to_string(), jp.pedido)];
            if let Some(a) = &j.apelido {
                par.push(("apelido".to_string(), Json::texto_de(a)));
            }
            par.push((
                "tipo".to_string(),
                Json::texto_de(j.tipo.nome_no_protocolo()),
            ));
            // `cruzado` e o UNICO tipo sem `ON` -- `j.em` vem sempre vazio
            // (a gramatica ja recusa `CROSS JOIN ... ON`), e o contrato
            // pede a chave OMITIDA nesse caso, nao uma lista vazia: um
            // `[]` teria cara de junção comum que por acaso nao casou par
            // nenhum, em vez de dizer "isto e produto".
            if !j.em.is_empty() {
                par.push((
                    "em".to_string(),
                    Json::Lista(
                        j.em.iter()
                            .map(|(esq, dir)| {
                                Json::Objeto(vec![
                                    ("esquerda".to_string(), Json::texto_de(esq)),
                                    ("direita".to_string(), Json::texto_de(dir)),
                                ])
                            })
                            .collect(),
                    ),
                ));
            }
            juntar_json.push(Json::Objeto(par));
        }
        pares.push(("juntar".to_string(), Json::Lista(juntar_json)));
        notas.push(format!(
            "{} junção(s), aplicadas na ordem, cada uma pelo MESMO portao de permissao",
            c.juntar.len()
        ));
    }

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

    if !c.escalar.is_empty() {
        let mut escalar_json = Vec::with_capacity(c.escalar.len());
        for e in &c.escalar {
            let ep = resolver(&e.de, &database)?;
            escalar_json.push(Json::Objeto(vec![
                ("nome".to_string(), Json::texto_de(&e.nome)),
                ("de".to_string(), ep.pedido),
                ("campo".to_string(), Json::texto_de(&e.campo)),
            ]));
        }
        pares.push(("escalar".to_string(), Json::Lista(escalar_json)));
        notas.push(format!(
            "{} subconsulta(s) ESCALAR -- roda ANTES pelo portao, e a comparacao usa o \
             nome dela (sub_N) na expressao",
            c.escalar.len()
        ));
    }

    if !c.existe.is_empty() {
        // Depois de juntar/escalar e antes da expressao -- a ordem que o
        // pedido 236 fixou, porque `existe` filtra LINHA (semijuncao) sem
        // acrescentar coluna, e a `expressao` de fora pode citar coluna que
        // so existe depois de juntar/escalar terem rodado.
        let mut existe_json = Vec::with_capacity(c.existe.len());
        for e in &c.existe {
            let ep = resolver(&e.de, &database)?;
            let mut par = vec![("de".to_string(), ep.pedido)];
            if let Some(a) = &e.apelido {
                par.push(("apelido".to_string(), Json::texto_de(a)));
            }
            par.push((
                "em".to_string(),
                Json::Lista(
                    e.em.iter()
                        .map(|(esq, dir)| {
                            Json::Objeto(vec![
                                ("esquerda".to_string(), Json::texto_de(esq)),
                                ("direita".to_string(), Json::texto_de(dir)),
                            ])
                        })
                        .collect(),
                ),
            ));
            par.push(("nao".to_string(), Json::Bool(e.nao)));
            existe_json.push(Json::Objeto(par));
        }
        pares.push(("existe".to_string(), Json::Lista(existe_json)));
        notas.push(format!(
            "{} EXISTS/NOT EXISTS -- semijuncao por espalhamento (roda por INTEIRO pelo \
             portao, nao por linha da consulta de fora)",
            c.existe.len()
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

/// O gancho para o servidor resolver `FROM v_c` quando `v_c` e uma VISAO
/// (item 6). `plano_de_dentro` e o pedido JA TRADUZIDO do SQL guardado na
/// visao -- o servidor reanalisa esse texto e chama `traduzir` com os
/// indices da tabela QUE A VISAO USA (so ele conhece esse esquema); esta
/// funcao so aplica por cima o resto do `SELECT` de FORA: `WHERE`,
/// colunas, `ORDER BY`, `LIMIT`/`OFFSET` -- os MESMOS campos que
/// `traduzir_consulta` ja produz, porque uma visao usada num `FROM` e
/// exatamente uma subconsulta com nome.
///
/// # Por que nao reusa `traduzir_consulta`
///
/// `traduzir_consulta` pede um RESOLVEDOR porque ela mesma decide QUANDO
/// chamar (`de`, cada `em`...). Aqui o pedido de dentro ja chegou pronto --
/// pedir para resolver de novo seria abrir o esquema da visao duas vezes.
///
/// # O que esta rodada NAO cobre
///
/// `COUNT(*)`/`GROUP BY` sobre uma visao recusam nomeando -- o `consultar`
/// nao agrupa, so filtra e projeta. Quem precisa agregar sobre uma visao
/// compoe por fora: `SELECT COUNT(*) FROM (SELECT * FROM v_c) AS x`.
pub fn planejar_sobre(selecao: &Selecao, plano_de_dentro: Json) -> Result<Plano> {
    let database = if !selecao.de.database.is_empty() {
        selecao.de.database.clone()
    } else {
        plano_de_dentro.texto_ou("database", "").to_string()
    };
    if database.is_empty() {
        return Err(PhxError::Esquema(
            "nao sei em qual database: escreva FROM banco.visao ou escolha o banco antes".into(),
        ));
    }

    let colunas: Option<Vec<ColunaComposta>> = match &selecao.projecao {
        Projecao::Tudo => None,
        Projecao::Colunas(cs) => Some(
            cs.iter()
                .map(|c| ColunaComposta {
                    coluna: c.nome.clone(),
                    apelido: c.apelido.clone(),
                })
                .collect(),
        ),
        Projecao::Contagem | Projecao::Agregada(_) => {
            return Err(PhxError::Esquema(
                "COUNT(*)/GROUP BY sobre visao nao tem substrato nesta rodada -- componha \
                 por fora, com outro SELECT sobre o resultado da visao"
                    .into(),
            ));
        }
    };

    let mut pares = vec![
        ("database".to_string(), Json::texto_de(&database)),
        ("de".to_string(), plano_de_dentro),
        (
            "expressao".to_string(),
            Json::texto_de(selecao.onde.as_ref().map(Onde::texto).unwrap_or_default()),
        ),
    ];
    if let Some(cols) = &colunas {
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
    if let Some(o) = &selecao.ordem {
        pares.push((
            "ordem".to_string(),
            Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de(&o.coluna)),
                ("desc".to_string(), Json::Bool(o.desc)),
            ])]),
        ));
    }
    if selecao.salto > 0 {
        pares.push(("pular".to_string(), Json::de_u64(selecao.salto)));
    }
    if let Some(l) = selecao.limite {
        pares.push(("max".to_string(), Json::de_u64(l)));
    }

    let saida = match &colunas {
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
        notas: vec![
            "FROM sobre uma visao vira `consultar`: o pedido de dentro (a visao, ja \
             traduzida com os indices da tabela DELA) e o `de`, e o resto do SELECT de \
             fora aplica por cima -- WHERE vira expressao, ORDER BY/LIMIT/OFFSET valem \
             sobre o resultado da visao, nao sobre a tabela"
                .into(),
        ],
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
        let mut viu_from = false;
        while let Some(s) = self.s.get(i) {
            if matches!(s.token, Token::PontoEVirgula) {
                break;
            }
            if let Some(p) = s.token.palavra_chave() {
                if p == "FROM" {
                    viu_from = true;
                } else if viu_from
                    && matches!(
                        p.as_str(),
                        "JOIN" | "INNER" | "LEFT" | "RIGHT" | "FULL" | "CROSS"
                    )
                {
                    // Junção (item 8) -- qualquer uma das seis palavras,
                    // inclusive RIGHT/FULL/CROSS, que a gramatica composta
                    // recusa NOMEANDO em vez de deixar cair no "junção
                    // ainda nao passa por aqui" generico do caminho velho.
                    return true;
                }
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
        let (de, apelido_de) = self.fonte_do_from(&cte)?;

        let (juntar, extras_do_on) = self.juncoes()?;

        let (onde, em, escalar, existe) = self.onde_composta(extras_do_on)?;

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
            apelido_de,
            juntar,
            em,
            existe,
            escalar,
            onde,
            colunas,
            janela,
            ordem,
            pular,
            max,
        })
    }

    /// A cadeia de
    /// `[INNER|LEFT [OUTER]|RIGHT [OUTER]|FULL [OUTER]|CROSS] JOIN fonte
    /// [ON em {AND em}]` -- zero ou mais, aplicadas na ordem escrita.
    /// Devolve as junções e os fragmentos de `ON` que NAO eram igualdade de
    /// colunas (viram texto, ANDados na `expressao` de fora junto com o
    /// `WHERE`).
    ///
    /// `CROSS JOIN` e o unico sem `ON` -- e por isso o unico tratado a
    /// parte: os outros quatro tipos exigem `ON` com pelo menos um par
    /// (pedido 236 nao muda essa exigencia, so acrescenta tipo).
    fn juncoes(&mut self) -> Result<(Vec<Juncao>, Vec<String>)> {
        let mut juntar = Vec::new();
        let mut extras = Vec::new();
        loop {
            let tipo = if self.aceitar_palavra("INNER") {
                self.exigir_palavra("JOIN")?;
                TipoJuncao::Interno
            } else if self.aceitar_palavra("LEFT") {
                self.aceitar_palavra("OUTER");
                self.exigir_palavra("JOIN")?;
                TipoJuncao::Esquerdo
            } else if self.aceitar_palavra("RIGHT") {
                self.aceitar_palavra("OUTER");
                self.exigir_palavra("JOIN")?;
                TipoJuncao::Direito
            } else if self.aceitar_palavra("FULL") {
                self.aceitar_palavra("OUTER");
                self.exigir_palavra("JOIN")?;
                TipoJuncao::Completo
            } else if self.aceitar_palavra("CROSS") {
                self.exigir_palavra("JOIN")?;
                TipoJuncao::Cruzado
            } else if self.aceitar_palavra("JOIN") {
                TipoJuncao::Interno
            } else {
                break;
            };
            let (de_j, apelido_j) = self.fonte_do_from(&None)?;
            let em = if tipo == TipoJuncao::Cruzado {
                // Produto: SEM `ON`. Se vier um `ON` mesmo assim, recusa
                // nomeando -- aceitar calado ignoraria uma condicao que
                // quem escreveu achava que estava filtrando o produto.
                let pos_on = self.posicao_atual();
                if self.aceitar_palavra("ON") {
                    return Err(lexico::erro(
                        pos_on,
                        "CROSS JOIN ... ON recusa nomeando: CROSS JOIN e produto, sem \
                         filtro nenhum -- se ha condicao de igualdade, e um JOIN comum \
                         (INNER/LEFT/RIGHT/FULL) com ON",
                    ));
                }
                Vec::new()
            } else {
                self.exigir_palavra("ON")?;
                let (em, extras_da_on) = self.condicao_de_juncao()?;
                extras.extend(extras_da_on);
                em
            };
            juntar.push(Juncao {
                de: de_j,
                apelido: apelido_j,
                tipo,
                em,
            });
        }
        Ok((juntar, extras))
    }

    /// O `ON` de uma junção: divide pelos `AND` de nivel superior (a mesma
    /// regra do WHERE composto) e classifica cada conjunto -- `col = col`
    /// (com qualificador opcional dos dois lados) vira um par em `em`;
    /// qualquer outra coisa vira fragmento de texto.
    fn condicao_de_juncao(&mut self) -> Result<ParesEFragmentosDoOn> {
        let tokens = self.capturar_ate_clausula(&[
            "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "CROSS", "WHERE", "ORDER", "LIMIT", "OFFSET",
        ])?;
        if tokens.is_empty() {
            return Err(lexico::erro(
                self.posicao_atual(),
                "esperava uma condicao depois de ON",
            ));
        }
        let mut em = Vec::new();
        let mut extras = Vec::new();
        for conj in dividir_por_and(tokens) {
            if let Some(par) = tentar_igualdade_de_colunas(&conj) {
                em.push(par);
                continue;
            }
            recusar_forma_nao_suportada(&conj)?;
            extras.push(normalizar_tokens(&conj));
        }
        if em.is_empty() {
            return Err(lexico::erro(
                self.posicao_atual(),
                "ON sem nenhuma igualdade de colunas nao tem substrato: junção por \
                 espalhamento em memoria precisa de pelo menos um par coluna = coluna",
            ));
        }
        Ok((em, extras))
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
    /// A fonte de um `FROM` (ou de um `JOIN`): uma tabela de sempre, o nome
    /// de uma CTE (se uma foi declarada -- so o `FROM` principal recebe
    /// `cte`, nunca uma junção), ou `(SELECT …) AS apelido`. Devolve
    /// TAMBEM o apelido -- o nome da tabela por padrao, ou o `AS` -- que a
    /// junção (item 8) precisa para qualificar coluna.
    fn fonte_do_from(
        &mut self,
        cte: &Option<(String, Selecao)>,
    ) -> Result<(Selecao, Option<String>)> {
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
            // como ser referenciada por ninguem, e a partir do item 8 uma
            // junção PRECISA dele para qualificar coluna.
            let apelido = self.identificador("apelido da subconsulta do FROM (obrigatorio)")?;
            return Ok((dentro, Some(apelido)));
        }
        let alvo = self.alvo()?;
        if let Some((nome_cte, corpo)) = cte {
            if alvo.database.is_empty()
                && alvo.schema.is_empty()
                && igual_sem_caso(nome_cte, &alvo.tabela)
            {
                let apelido = alvo.apelido.clone().or_else(|| Some(nome_cte.clone()));
                return Ok((corpo.clone(), apelido));
            }
        }
        let apelido = alvo.apelido.clone().or_else(|| Some(alvo.tabela.clone()));
        let sel = Selecao {
            projecao: Projecao::Tudo,
            de: alvo,
            onde: None,
            agrupar_por: Vec::new(),
            tendo: None,
            ordem: None,
            ordem_lista: Vec::new(),
            limite: None,
            salto: 0,
        };
        Ok((sel, apelido))
    }

    /// O `WHERE` composto: cada conjunto de nivel superior (dividido por
    /// `AND`) e classificado -- `coluna IN (SELECT …)` vira `em`; qualquer
    /// outra coisa vira texto (ANDado de volta na `expressao`).
    /// `fragmentos` chega com o que sobrou de nao-igualdade do `ON` de cada
    /// junção (item 8) -- eles entram na MESMA `expressao`, porque o
    /// contrato nao tem um campo de expressao por junção.
    fn onde_composta(&mut self, mut fragmentos: Vec<String>) -> Result<PartesDoWhereComposto> {
        let mut em = Vec::new();
        let mut escalar = Vec::new();
        let mut existe = Vec::new();
        let mut contador_escalar = 1usize;
        if self.aceitar_palavra("WHERE") {
            let tokens = self.capturar_ate_clausula(&["ORDER", "LIMIT", "OFFSET"])?;
            if tokens.is_empty() {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "esperava uma condicao depois de WHERE",
                ));
            }
            for conj in dividir_por_and(tokens) {
                if let Some(e) = tentar_in_subconsulta(&conj)? {
                    em.push(e);
                    continue;
                }
                if let Some((fragmento, esc)) = tentar_escalar(&conj, &mut contador_escalar)? {
                    fragmentos.push(fragmento);
                    escalar.push(esc);
                    continue;
                }
                if let Some(ex) = tentar_existe(&conj)? {
                    existe.push(ex);
                    continue;
                }
                recusar_forma_nao_suportada(&conj)?;
                fragmentos.push(normalizar_tokens(&conj));
            }
        }
        let onde = if fragmentos.is_empty() {
            None
        } else {
            Some(Onde::Expressao(fragmentos.join(" AND ")))
        };
        Ok((onde, em, escalar, existe))
    }
}

/// Divide os tokens de uma clausula pelos `AND` de NIVEL SUPERIOR (fora de
/// parenteses) -- serve para o `WHERE` composto (item 4/9), para o `ON`
/// de junção (item 8) e para o `WHERE` de dentro de um `EXISTS` (pedido
/// 236), que sao as tres gramaticas desta camada que tratam AND como lista
/// em vez de arvore.
///
/// # A entrada vazia devolve UM conjunto vazio, nao zero
///
/// `dividir_por_and(Vec::new())` devolve `vec![vec![]]`, nao `vec![]` --
/// o laco que acumula em `atual` nunca ve um `AND` para separar, e o
/// `partes.push(atual)` final entra do mesmo jeito. Quem itera o resultado
/// sem WHERE nenhum (um `EXISTS (SELECT ... FROM t)` sem clausula) processa
/// UM termo vazio, e tem de pular explicitamente (`if termo.is_empty() {
/// continue; }`) em vez de contar com "zero termos" para decidir que nao
/// ha filtro.
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
/// Le `nome` ou `nome.nome` a partir de `*i`, avancando `*i` -- `None` sem
/// mexer em nada quando nao bate esse molde (nao e erro: quem chama decide
/// o que fazer com "nao bateu").
fn ler_nome_qualificado(conj: &[Simbolo], i: &mut usize) -> Option<String> {
    let Token::Palavra {
        texto,
        citado: false,
    } = &conj.get(*i)?.token
    else {
        return None;
    };
    let mut nome = texto.clone();
    let mut j = *i + 1;
    if matches!(conj.get(j).map(|s| &s.token), Some(Token::Ponto)) {
        let Token::Palavra {
            texto: segunda,
            citado: false,
        } = &conj.get(j + 1)?.token
        else {
            return None;
        };
        nome = format!("{nome}.{segunda}");
        j += 2;
    }
    *i = j;
    Some(nome)
}

/// `coluna[.coluna] = coluna[.coluna]`, ocupando o conjunto INTEIRO -- a
/// forma que `ON` de junção (item 8) reconhece como par de `em`. Qualquer
/// outra coisa (comparador diferente de `=`, funcao, literal de um dos
/// lados...) devolve `None`, e quem chama trata como fragmento de texto.
fn tentar_igualdade_de_colunas(conj: &[Simbolo]) -> Option<(String, String)> {
    let mut i = 0usize;
    let esquerda = ler_nome_qualificado(conj, &mut i)?;
    if !matches!(
        conj.get(i).map(|s| &s.token),
        Some(Token::Comparador(crate::lexico::Comparador::Igual))
    ) {
        return None;
    }
    i += 1;
    let direita = ler_nome_qualificado(conj, &mut i)?;
    if i != conj.len() {
        return None;
    }
    Some((esquerda, direita))
}

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

/// `coluna[.coluna] OP (SELECT ...)`, ocupando o conjunto INTEIRO -- a
/// subconsulta ESCALAR (item 9). Devolve o fragmento de texto pronto
/// (`coluna OP sub_N`) e o `Escalar` correspondente; `None` quando o
/// conjunto nao bate esse molde (quem chama tenta o resto).
fn tentar_escalar(conj: &[Simbolo], contador: &mut usize) -> Result<Option<(String, Escalar)>> {
    let mut i = 0usize;
    let Some(coluna) = ler_nome_qualificado(conj, &mut i) else {
        return Ok(None);
    };
    let Some(Token::Comparador(op)) = conj.get(i).map(|s| &s.token) else {
        return Ok(None);
    };
    let op = *op;
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
            "sobrou algo dentro da subconsulta escalar",
        ));
    }
    recusa_se_correlacionada(&dentro)?;
    let campo = campo_escalar(&dentro.projecao)?;
    let nome = format!("sub_{contador}");
    *contador += 1;
    let fragmento = format!("{coluna} {} {nome}", op.simbolo());
    Ok(Some((
        fragmento,
        Escalar {
            nome,
            de: dentro,
            campo,
        },
    )))
}

/// `[NOT] EXISTS ( SELECT ... FROM t [AS x] WHERE c1 AND c2 ... )`, ocupando
/// o conjunto INTEIRO -- a semijuncao por espalhamento (pedido 236). `None`
/// quando o conjunto nao COMECA com `[NOT] EXISTS` (nao e erro: quem chama
/// tenta o resto, que cai em `recusar_forma_nao_suportada`). A partir do
/// `EXISTS` reconhecido, qualquer forma que nao bate o molde vira erro
/// NOMEADO -- nunca "nao bateu" -- porque a essa altura ja se sabe que e um
/// EXISTS, e o texto cru dele nunca vira uma expressao que o avaliador do
/// motor consiga ler.
///
/// # A regra de correlacao (a divergencia desta casa)
///
/// So `fora.col = dentro.col` (em qualquer ordem dos lados) correlaciona --
/// e so por igualdade, porque a semijuncao roda por ESPALHAMENTO (uma
/// passada em cada lado), nao rodando a subconsulta por linha. Termo que so
/// cita coluna de dentro vira filtro do sub-pedido; termo que cita coluna
/// de fora sem ser essa igualdade (op diferente de `=`, os dois lados de
/// fora, ou o termo inteiro dentro de um `OR` -- que nunca bate
/// `tentar_igualdade_de_colunas` sozinho) recusa NOMEANDO o termo. Sem par
/// nenhum, o `EXISTS` e so "tem linha?" e ainda nao tem substrato.
fn tentar_existe(conj: &[Simbolo]) -> Result<Option<Existe>> {
    let mut i = 0usize;
    let nao = conj
        .first()
        .and_then(|s| s.token.palavra_chave())
        .as_deref()
        == Some("NOT")
        && conj.get(1).and_then(|s| s.token.palavra_chave()).as_deref() == Some("EXISTS");
    if nao {
        i = 1;
    }
    if conj.get(i).and_then(|s| s.token.palavra_chave()).as_deref() != Some("EXISTS") {
        return Ok(None);
    }
    let pos = conj[i].posicao;
    i += 1;
    if !matches!(conj.get(i).map(|s| &s.token), Some(Token::AbreParen)) {
        return Err(lexico::erro(pos, "esperava ( depois de EXISTS"));
    }
    i += 1;
    if conj.get(i).and_then(|s| s.token.palavra_chave()).as_deref() != Some("SELECT") {
        return Err(lexico::erro(pos, "esperava SELECT dentro do EXISTS (...)"));
    }
    i += 1;
    let Some(ultimo) = conj.last() else {
        return Err(lexico::erro(pos, "esperava ) fechando o EXISTS (...)"));
    };
    if !matches!(ultimo.token, Token::FechaParen) || i >= conj.len() - 1 {
        return Err(lexico::erro(pos, "esperava ) fechando o EXISTS (...)"));
    }
    let interior = &conj[i..conj.len() - 1];

    // Pula a projecao (EXISTS so pergunta "tem linha?"; o que vem entre
    // SELECT e FROM nunca importa) ate o FROM de nivel superior.
    let mut j = 0usize;
    let mut prof = 0i32;
    loop {
        let Some(s) = interior.get(j) else {
            return Err(lexico::erro(pos, "esperava FROM dentro do EXISTS (...)"));
        };
        match &s.token {
            Token::AbreParen => prof += 1,
            Token::FechaParen => prof -= 1,
            _ if prof == 0 && s.token.palavra_chave().as_deref() == Some("FROM") => break,
            _ => {}
        }
        j += 1;
    }
    let resto = interior[j + 1..].to_vec();
    let mut sub = Analisador { s: resto, i: 0 };
    let alvo = sub.alvo()?;
    let apelido_interno = alvo.apelido.clone().unwrap_or_else(|| alvo.tabela.clone());

    let onde_tokens: Vec<Simbolo> = if sub.aceitar_palavra("WHERE") {
        sub.s[sub.i..].to_vec()
    } else if sub.espiar().is_some() {
        // GROUP BY, ORDER BY, outro JOIN... nada disso tem substrato
        // dentro de um EXISTS nesta rodada -- so FROM e WHERE.
        return Err(lexico::erro(
            pos,
            "EXISTS so aceita FROM tabela [AS apelido] WHERE ... nesta rodada",
        ));
    } else {
        Vec::new()
    };

    let mut pares_em = Vec::new();
    let mut fragmentos_de_dentro = Vec::new();
    for termo in dividir_por_and(onde_tokens) {
        if termo.is_empty() {
            // `dividir_por_and(Vec::new())` (EXISTS sem WHERE) devolve UM
            // conjunto vazio, nao zero -- ver o comentario de `dividir_por_and`.
            continue;
        }
        if let Some((esq, dir)) = tentar_igualdade_de_colunas(&termo) {
            let esq_fora = eh_qualificador_de_fora(&esq, &apelido_interno);
            let dir_fora = eh_qualificador_de_fora(&dir, &apelido_interno);
            match (esq_fora, dir_fora) {
                (true, false) => {
                    pares_em.push((esq, dir));
                    continue;
                }
                (false, true) => {
                    pares_em.push((dir, esq));
                    continue;
                }
                (false, false) => {
                    fragmentos_de_dentro.push(normalizar_tokens(&termo));
                    continue;
                }
                (true, true) => {
                    return Err(lexico::erro(
                        pos,
                        &format!(
                            "EXISTS: {:?} compara duas colunas de FORA -- correlacao e \
                             sempre fora.col = dentro.col",
                            normalizar_tokens(&termo)
                        ),
                    ));
                }
            }
        }
        if let Some((qualificador, pos_termo)) =
            primeiro_qualificador_de_fora(&termo, &apelido_interno)
        {
            return Err(lexico::erro(
                pos_termo,
                &format!(
                    "EXISTS: termo {:?} cita coluna de fora ({qualificador:?}) sem ser \
                     igualdade -- so \"fora.col = dentro.col\" correlaciona (dentro de um \
                     OR recusa pelo mesmo motivo: o termo inteiro nao bate essa forma)",
                    normalizar_tokens(&termo)
                ),
            ));
        }
        fragmentos_de_dentro.push(normalizar_tokens(&termo));
    }

    if pares_em.is_empty() {
        return Err(lexico::erro(
            pos,
            "EXISTS sem correlacao (nenhum par fora.col = dentro.col) nao tem substrato \
             nesta rodada: um EXISTS nao correlacionado e so \"tem linha?\", e ainda nao ha \
             substrato para isso",
        ));
    }

    let onde_de_dentro = if fragmentos_de_dentro.is_empty() {
        None
    } else {
        Some(Onde::Expressao(fragmentos_de_dentro.join(" AND ")))
    };

    let dentro = Selecao {
        projecao: Projecao::Tudo,
        de: alvo,
        onde: onde_de_dentro,
        agrupar_por: Vec::new(),
        tendo: None,
        ordem: None,
        ordem_lista: Vec::new(),
        limite: None,
        salto: 0,
    };

    Ok(Some(Existe {
        de: dentro,
        apelido: Some(apelido_interno),
        em: pares_em,
        nao,
    }))
}

/// O qualificador (antes do `.`) de `nome` NAO e o apelido de dentro? Nome
/// SEM qualificador (sem `.`) e SEMPRE de dentro -- a mesma regra da
/// projecao e do `juntar`.
fn eh_qualificador_de_fora(nome: &str, apelido_interno: &str) -> bool {
    match nome.split_once('.') {
        Some((qualificador, _)) => !igual_sem_caso(qualificador, apelido_interno),
        None => false,
    }
}

/// Acha, em QUALQUER lugar do termo (funcao, parenteses, `OR`...), o
/// primeiro `qualificador.coluna` cujo qualificador NAO e o apelido de
/// dentro -- ou `None` se toda referencia qualificada do termo e de dentro
/// (ou nao ha qualificador nenhum). Devolve tambem a posicao, para a
/// mensagem apontar o lugar certo em vez do `EXISTS` la na frente.
fn primeiro_qualificador_de_fora(
    termo: &[Simbolo],
    apelido_interno: &str,
) -> Option<(String, usize)> {
    let mut i = 0usize;
    while i < termo.len() {
        if let Token::Palavra {
            texto,
            citado: false,
        } = &termo[i].token
        {
            if matches!(termo.get(i + 1).map(|s| &s.token), Some(Token::Ponto))
                && matches!(
                    termo.get(i + 2).map(|s| &s.token),
                    Some(Token::Palavra { citado: false, .. })
                )
            {
                if !igual_sem_caso(texto, apelido_interno) {
                    return Some((texto.clone(), termo[i].posicao));
                }
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Chega aqui o que `tentar_existe` ja recusou como "nao e a forma dele" --
/// ou seja, um `EXISTS` que nao ocupa o `AND` INTEIRO (esta dentro de um
/// `OR`, ou combinado com outra coisa no mesmo termo). So o `[NOT] EXISTS
/// (...)` sozinho tem substrato (pedido 236); um `SELECT` solto (nao
/// reconhecido como IN/escalar/EXISTS) tambem nao tem -- e a recusa nomeia
/// os dois casos, em vez de deixar o texto virar uma expressao que o
/// avaliador do motor nao entende.
fn recusar_forma_nao_suportada(conj: &[Simbolo]) -> Result<()> {
    for s in conj {
        if let Some(p) = s.token.palavra_chave() {
            if p == "EXISTS" {
                return Err(lexico::erro(
                    s.posicao,
                    "EXISTS recusa nomeando: so tem substrato quando ocupa o AND inteiro \
                     ([NOT] EXISTS (SELECT ...) sozinho) -- dentro de um OR, ou combinado \
                     com outra coisa no mesmo termo, exigiria rodar a subconsulta por linha \
                     da consulta de fora",
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
            Comando::Consulta(c) => *c,
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

    /// Sem WHERE nenhum dentro do EXISTS nao ha como correlacionar --
    /// continua recusando, so que agora pelo motivo certo (falta de par),
    /// nao mais "EXISTS nao tem substrato nenhum".
    #[test]
    fn exists_sem_correlacao_recusa_nomeando() {
        let e = recusa("SELECT * FROM pedidos WHERE EXISTS (SELECT 1 FROM clientes)");
        assert!(e.contains("EXISTS"), "{e}");
        assert!(e.contains("correlacao"), "{e}");
    }

    // ------------------------------------------------- EXISTS (pedido 236)

    /// A forma do contrato: falhava com o defeito reposto (recusava
    /// "EXISTS recusa nomeando..." antes de sequer olhar o WHERE de
    /// dentro); passa com o conserto, com o JSON exato do exemplo do
    /// pedido 236.
    #[test]
    fn exists_correlacionado_vira_existe_com_par_em() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id)",
        );
        assert_eq!(c.existe.len(), 1);
        let ex = &c.existe[0];
        assert_eq!(ex.de.de.tabela, "pedidos");
        assert_eq!(ex.apelido, Some("x".to_string()));
        assert_eq!(
            ex.em,
            vec![("c.id".to_string(), "x.cliente_id".to_string())]
        );
        assert!(!ex.nao);

        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        let existe = p.pedido.campo("existe").unwrap().lista().unwrap();
        assert_eq!(existe.len(), 1);
        assert_eq!(existe[0].texto_ou("apelido", ""), "x");
        assert_eq!(
            existe[0].campo("em").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("esquerda".to_string(), Json::texto_de("c.id")),
                ("direita".to_string(), Json::texto_de("x.cliente_id")),
            ])])
        );
        assert_eq!(existe[0].campo("nao").unwrap(), &Json::Bool(false));
        assert!(existe[0].campo("de").is_some());
    }

    /// A ordem dos lados da igualdade nao importa -- `x.col = c.col` casa
    /// igual a `c.col = x.col`, e o par sai SEMPRE com a de fora na
    /// esquerda.
    #[test]
    fn exists_aceita_igualdade_com_lado_de_dentro_primeiro() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id)",
        );
        let c2 = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             c.id = x.cliente_id)",
        );
        assert_eq!(c.existe[0].em, c2.existe[0].em);
    }

    #[test]
    fn not_exists_marca_nao_true() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE NOT EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id)",
        );
        assert!(c.existe[0].nao);
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        let existe = p.pedido.campo("existe").unwrap().lista().unwrap();
        assert_eq!(existe[0].campo("nao").unwrap(), &Json::Bool(true));
    }

    /// Termo que so cita coluna de DENTRO (sem qualificador de fora) vai
    /// para a `expressao` do sub-pedido, nao para `em`.
    #[test]
    fn exists_com_filtro_de_dentro_alem_da_correlacao() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id AND x.status = 'aberto')",
        );
        assert_eq!(
            c.existe[0].em,
            vec![("c.id".to_string(), "x.cliente_id".to_string())]
        );
        assert_eq!(
            c.existe[0].de.onde,
            Some(Onde::Expressao("x.status = 'aberto'".to_string()))
        );
    }

    /// Quando o apelido de dentro nao vem com AS, o padrao e o nome da
    /// tabela -- a mesma convencao do `juntar`.
    #[test]
    fn exists_sem_as_usa_o_nome_da_tabela_como_apelido() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos WHERE \
             pedidos.cliente_id = c.id)",
        );
        assert_eq!(c.existe[0].apelido, Some("pedidos".to_string()));
        assert_eq!(
            c.existe[0].em,
            vec![("c.id".to_string(), "pedidos.cliente_id".to_string())]
        );
    }

    /// Nome de coluna SEM qualificador dentro do EXISTS e sempre de
    /// DENTRO -- mesmo sem AS, `cliente_id = c.id` correlaciona igual a
    /// `pedidos.cliente_id = c.id`.
    #[test]
    fn exists_coluna_de_dentro_sem_qualificador_tambem_correlaciona() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos WHERE cliente_id \
             = c.id)",
        );
        assert_eq!(
            c.existe[0].em,
            vec![("c.id".to_string(), "cliente_id".to_string())]
        );
    }

    /// Termo que cita coluna de fora sem ser igualdade recusa nomeando.
    #[test]
    fn exists_correlacao_nao_igualdade_recusa_nomeando() {
        let e = recusa(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.preco > c.limite)",
        );
        assert!(e.contains("EXISTS"), "{e}");
        assert!(e.contains("limite") || e.contains("igualdade"), "{e}");
    }

    /// Termo de correlacao dentro de um OR recusa nomeando -- o `OR`
    /// impede o termo de bater `fora.col = dentro.col` sozinho, entao ele
    /// cai no crivo de "cita coluna de fora sem ser igualdade".
    #[test]
    fn exists_correlacao_dentro_de_or_recusa_nomeando() {
        let e = recusa(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id OR x.status = 'aberto')",
        );
        assert!(e.contains("EXISTS"), "{e}");
    }

    /// EXISTS com filtro so de DENTRO (sem nenhum par de correlacao)
    /// continua recusando -- e "tem linha?" sem substrato ainda.
    #[test]
    fn exists_so_com_filtro_de_dentro_sem_correlacao_recusa() {
        let e = recusa(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.status = 'aberto')",
        );
        assert!(e.contains("EXISTS"), "{e}");
        assert!(e.contains("correlacao"), "{e}");
    }

    /// `existe` roda pelo MESMO portao (`resolver`) que qualquer outro
    /// pedaco -- tabela negada dentro do EXISTS recusa a consulta INTEIRA,
    /// nao so o EXISTS. E o teste do comportamento velho ao lado: sem
    /// EXISTS, a mesma consulta nao muda.
    #[test]
    fn tabela_negada_dentro_do_exists_recusa_a_consulta_inteira() {
        let c = consulta(
            "SELECT * FROM clientes c WHERE EXISTS (SELECT 1 FROM pedidos AS x WHERE \
             x.cliente_id = c.id)",
        );
        let mut resolver_com_negacao = |sel: &Selecao, db: &str| -> Result<Plano> {
            if sel.de.tabela == "pedidos" {
                return Err(PhxError::Esquema(
                    "tabela pedidos negada para este usuario".into(),
                ));
            }
            resolver_simples(sel, db)
        };
        let e = traduzir_consulta(&c, "loja", &mut resolver_com_negacao)
            .unwrap_err()
            .to_string();
        assert!(e.contains("negada"), "{e}");
    }

    #[test]
    fn sem_exists_nada_muda() {
        // Uma consulta composta (aqui, com JOIN) sem EXISTS nenhum -- o
        // campo `existe` fica vazio e a chave nem aparece no JSON.
        let c = consulta("SELECT * FROM p JOIN clientes c ON p.cliente_id = c.id");
        assert!(c.existe.is_empty());
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert!(p.pedido.campo("existe").is_none());
    }

    /// IN correlacionado continua recusando -- o pedido 236 so abriu
    /// EXISTS por igualdade, nao correlacao em geral.
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

    /// Pedido 236: `ORDER BY p.id` (qualificado) -- o mais barato dos
    /// quatro, porque o `consultar` ja aceita `{"coluna":"p.id"}` na ordem;
    /// faltava so o tradutor SQL gerar isso. Antes do conserto, o `.` sem
    /// qualificador tratado sobrava para o `FROM`, e a recusa dizia
    /// "esperava FROM" -- confuso, porque o problema era no ORDER BY.
    #[test]
    fn order_by_qualificado_vira_coluna_com_ponto() {
        let c = consulta(
            "SELECT p.id, c.nome FROM pedidos p JOIN clientes c ON p.cliente_id = c.id \
             ORDER BY p.id DESC",
        );
        assert_eq!(
            c.ordem,
            vec![Ordenacao {
                coluna: "p.id".into(),
                desc: true,
            }]
        );
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert_eq!(
            p.pedido.campo("ordem").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de("p.id")),
                ("desc".to_string(), Json::Bool(true)),
            ])])
        );
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

    // ---------------------------------------- item 6: planejar_sobre (visoes)

    fn plano_de_dentro() -> Json {
        Json::Objeto(vec![
            ("op".to_string(), Json::texto_de("varrer")),
            ("database".to_string(), Json::texto_de("loja")),
            ("tabela".to_string(), Json::texto_de("clientes")),
        ])
    }

    #[test]
    fn planejar_sobre_select_estrela_da_visao() {
        let sel = crate::sintaxe::analisar("SELECT * FROM v_c").unwrap();
        let p = planejar_sobre(&sel, plano_de_dentro()).unwrap();
        assert_eq!(p.op, "consultar");
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
        assert_eq!(
            p.pedido.campo("de").unwrap().texto_ou("tabela", ""),
            "clientes"
        );
        assert!(p.pedido.campo("colunas").is_none());
        assert_eq!(p.saida, Saida::LinhaInteira);
    }

    #[test]
    fn planejar_sobre_aplica_where_colunas_ordem_e_limite() {
        let sel = crate::sintaxe::analisar(
            "SELECT nome AS n FROM v_c WHERE ativo = TRUE ORDER BY nome LIMIT 5 OFFSET 1",
        )
        .unwrap();
        let p = planejar_sobre(&sel, plano_de_dentro()).unwrap();
        assert_eq!(p.pedido.texto_ou("expressao", ""), "ativo = TRUE");
        assert_eq!(
            p.pedido.campo("colunas").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de("nome")),
                ("apelido".to_string(), Json::texto_de("n")),
            ])])
        );
        assert_eq!(
            p.pedido.campo("ordem").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de("nome")),
                ("desc".to_string(), Json::Bool(false)),
            ])])
        );
        assert_eq!(p.pedido.inteiro_ou("max", 0), 5);
        assert_eq!(p.pedido.inteiro_ou("pular", 0), 1);
        assert_eq!(p.saida, Saida::Colunas(vec![("nome".into(), "n".into())]));
    }

    #[test]
    fn planejar_sobre_count_ou_group_by_recusa() {
        let sel = crate::sintaxe::analisar("SELECT COUNT(*) FROM v_c").unwrap();
        let e = planejar_sobre(&sel, plano_de_dentro())
            .unwrap_err()
            .to_string();
        assert!(e.contains("GROUP BY"), "{e}");
    }

    #[test]
    fn planejar_sobre_usa_o_database_do_pedido_de_dentro_quando_from_nao_diz() {
        let sel = crate::sintaxe::analisar("SELECT * FROM v_c").unwrap();
        let p = planejar_sobre(&sel, plano_de_dentro()).unwrap();
        // "loja" veio do plano_de_dentro, nao de `sel.de.database` (vazio).
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
    }

    // -------------------------------------------------------- item 8: junção

    #[test]
    fn inner_join_com_apelido_e_igualdade() {
        let c = consulta(
            "SELECT p.id, c.nome AS cliente FROM pedidos p JOIN clientes c ON \
             p.cliente_id = c.id",
        );
        assert_eq!(c.de.de.tabela, "pedidos");
        assert_eq!(c.apelido_de, Some("p".to_string()));
        assert_eq!(c.juntar.len(), 1);
        let j = &c.juntar[0];
        assert_eq!(j.de.de.tabela, "clientes");
        assert_eq!(j.apelido, Some("c".to_string()));
        assert_eq!(j.tipo, TipoJuncao::Interno);
        assert_eq!(j.em, vec![("p.cliente_id".to_string(), "c.id".to_string())]);
        assert_eq!(
            c.colunas,
            Some(vec![
                ColunaComposta {
                    coluna: "p.id".into(),
                    apelido: None
                },
                ColunaComposta {
                    coluna: "c.nome".into(),
                    apelido: Some("cliente".into())
                },
            ])
        );
    }

    #[test]
    fn apelido_padrao_e_o_nome_da_tabela_sem_as() {
        let c = consulta("SELECT * FROM pedidos JOIN clientes ON pedidos.cliente_id = clientes.id");
        assert_eq!(c.apelido_de, Some("pedidos".to_string()));
        assert_eq!(c.juntar[0].apelido, Some("clientes".to_string()));
    }

    #[test]
    fn inner_explicito_e_igual_a_join_sozinho() {
        let a = consulta("SELECT * FROM p INNER JOIN c ON p.id = c.id");
        let b = consulta("SELECT * FROM p JOIN c ON p.id = c.id");
        assert_eq!(a.juntar[0].tipo, TipoJuncao::Interno);
        assert_eq!(a.juntar[0].tipo, b.juntar[0].tipo);
    }

    #[test]
    fn left_join_com_e_sem_outer() {
        let a = consulta("SELECT * FROM p LEFT JOIN c ON p.id = c.id");
        let b = consulta("SELECT * FROM p LEFT OUTER JOIN c ON p.id = c.id");
        assert_eq!(a.juntar[0].tipo, TipoJuncao::Esquerdo);
        assert_eq!(a.juntar[0].tipo, b.juntar[0].tipo);
    }

    /// Pedido 236: os quatro tipos que faltavam viram `tipo` 1:1 -- nao
    /// mais troca de lado nem recusa. Falhava antes do conserto (os quatro
    /// SQLs abaixo davam `Err` nomeando "RIGHT/FULL/CROSS JOIN recusa");
    /// passa depois, com o `tipo` certo e (so no cruzado) sem `em`.
    #[test]
    fn right_full_cross_viram_tipo_1_para_1() {
        let r = consulta("SELECT * FROM p RIGHT JOIN c ON p.id = c.id");
        assert_eq!(r.juntar[0].tipo, TipoJuncao::Direito);
        assert_eq!(
            r.juntar[0].em,
            vec![("p.id".to_string(), "c.id".to_string())]
        );

        let ro = consulta("SELECT * FROM p RIGHT OUTER JOIN c ON p.id = c.id");
        assert_eq!(ro.juntar[0].tipo, TipoJuncao::Direito);

        let f = consulta("SELECT * FROM p FULL JOIN c ON p.id = c.id");
        assert_eq!(f.juntar[0].tipo, TipoJuncao::Completo);

        let fo = consulta("SELECT * FROM p FULL OUTER JOIN c ON p.id = c.id");
        assert_eq!(fo.juntar[0].tipo, TipoJuncao::Completo);

        let cr = consulta("SELECT * FROM p CROSS JOIN c");
        assert_eq!(cr.juntar[0].tipo, TipoJuncao::Cruzado);
        assert!(
            cr.juntar[0].em.is_empty(),
            "CROSS JOIN nao tem ON -- em tem de vir vazio"
        );
    }

    /// O JSON que cada um produz -- o exemplo do relatorio. `direito` e
    /// `completo` levam `em` como qualquer junção comum; `cruzado` sai SEM
    /// a chave `em` (nao com `[]`), porque `[]` teria cara de junção que
    /// nao casou par nenhum, e cruzado nunca teve par para casar.
    #[test]
    fn json_de_direito_completo_e_cruzado() {
        let r = consulta("SELECT * FROM p RIGHT JOIN c ON p.id = c.id");
        let pr = traduzir_consulta(&r, "loja", &mut resolver_simples).unwrap();
        let jr = pr.pedido.campo("juntar").unwrap().lista().unwrap();
        assert_eq!(jr[0].texto_ou("tipo", ""), "direito");
        assert_eq!(
            jr[0].campo("em").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("esquerda".to_string(), Json::texto_de("p.id")),
                ("direita".to_string(), Json::texto_de("c.id")),
            ])])
        );

        let f = consulta("SELECT * FROM p FULL JOIN c ON p.id = c.id");
        let pf = traduzir_consulta(&f, "loja", &mut resolver_simples).unwrap();
        let jf = pf.pedido.campo("juntar").unwrap().lista().unwrap();
        assert_eq!(jf[0].texto_ou("tipo", ""), "completo");

        let cr = consulta("SELECT * FROM p CROSS JOIN c");
        let pcr = traduzir_consulta(&cr, "loja", &mut resolver_simples).unwrap();
        let jcr = pcr.pedido.campo("juntar").unwrap().lista().unwrap();
        assert_eq!(jcr[0].texto_ou("tipo", ""), "cruzado");
        assert!(
            jcr[0].campo("em").is_none(),
            "CROSS JOIN nao leva a chave em no JSON"
        );
    }

    /// `CROSS JOIN ... ON` recusa nomeando: e produto, sem filtro. Aceitar
    /// calado ignoraria uma condicao que quem escreveu achava que estava
    /// filtrando.
    #[test]
    fn cross_join_com_on_recusa_nomeando() {
        let e = recusa("SELECT * FROM p CROSS JOIN c ON p.id = c.id");
        assert!(e.contains("CROSS JOIN"), "{e}");
        assert!(e.contains("produto"), "{e}");
    }

    #[test]
    fn on_com_and_de_igualdades_vira_varios_pares() {
        let c = consulta("SELECT * FROM p JOIN c ON p.cliente_id = c.id AND p.filial = c.filial");
        assert_eq!(
            c.juntar[0].em,
            vec![
                ("p.cliente_id".to_string(), "c.id".to_string()),
                ("p.filial".to_string(), "c.filial".to_string()),
            ]
        );
    }

    #[test]
    fn on_com_condicao_nao_igualdade_vira_expressao_de_fora() {
        let c = consulta(
            "SELECT * FROM p JOIN c ON p.cliente_id = c.id AND p.total > 100 WHERE p.ativo = \
             TRUE",
        );
        assert_eq!(
            c.juntar[0].em,
            vec![("p.cliente_id".to_string(), "c.id".to_string())]
        );
        let Some(Onde::Expressao(texto)) = &c.onde else {
            panic!("esperava Expressao")
        };
        // O pedaco do ON entra ANTES do WHERE, na ordem em que apareceram.
        assert_eq!(texto, "p.total > 100 AND p.ativo = TRUE");
    }

    #[test]
    fn on_sem_nenhuma_igualdade_recusa() {
        let e = recusa("SELECT * FROM p JOIN c ON p.total > 100");
        assert!(e.contains("nenhuma igualdade"), "{e}");
    }

    #[test]
    fn join_com_subconsulta_e_apelido() {
        let c = consulta(
            "SELECT * FROM p JOIN (SELECT id, nome FROM clientes) AS c ON p.cliente_id = c.id",
        );
        assert_eq!(c.juntar[0].de.de.tabela, "clientes");
        assert_eq!(c.juntar[0].apelido, Some("c".to_string()));
    }

    #[test]
    fn duas_juncoes_encadeadas() {
        let c = consulta(
            "SELECT * FROM p JOIN c ON p.cliente_id = c.id LEFT JOIN e ON p.filial = e.id",
        );
        assert_eq!(c.juntar.len(), 2);
        assert_eq!(c.juntar[0].tipo, TipoJuncao::Interno);
        assert_eq!(c.juntar[1].tipo, TipoJuncao::Esquerdo);
        assert_eq!(c.juntar[1].de.de.tabela, "e");
    }

    #[test]
    fn join_traduz_com_o_json_do_contrato() {
        let c = consulta(
            "SELECT p.id, c.nome AS cliente FROM pedidos p JOIN clientes c ON \
             p.cliente_id = c.id WHERE c.cidade = 'Blumenau'",
        );
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert_eq!(p.pedido.texto_ou("apelido", ""), "p");
        let juntar = p.pedido.campo("juntar").unwrap().lista().unwrap();
        assert_eq!(juntar.len(), 1);
        assert_eq!(juntar[0].texto_ou("apelido", ""), "c");
        assert_eq!(juntar[0].texto_ou("tipo", ""), "interno");
        assert_eq!(
            juntar[0].campo("de").unwrap().texto_ou("tabela", ""),
            "clientes"
        );
        let em = juntar[0].campo("em").unwrap().lista().unwrap();
        assert_eq!(em[0].texto_ou("esquerda", ""), "p.cliente_id");
        assert_eq!(em[0].texto_ou("direita", ""), "c.id");
        assert_eq!(p.pedido.texto_ou("expressao", ""), "c.cidade = 'Blumenau'");
    }

    #[test]
    fn select_sem_join_nao_leva_apelido_nem_juntar_no_pedido() {
        // Sem junção nenhuma, "apelido" nao aparece no pedido -- o
        // contrato base (item 4) nao tem esse campo.
        let c = consulta("SELECT * FROM (SELECT id FROM t) AS x");
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        assert!(p.pedido.campo("apelido").is_none());
        assert!(p.pedido.campo("juntar").is_none());
    }

    /// A mesma prova do item 4, agora para `juntar`: uma tabela negada
    /// dentro da junção recusa a consulta INTEIRA.
    #[test]
    fn tabela_negada_na_juncao_recusa_a_consulta_inteira() {
        let c = consulta("SELECT * FROM p JOIN clientes ON p.cliente_id = clientes.id");
        let mut resolver_com_negacao = |sel: &Selecao, db: &str| -> Result<Plano> {
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
    }

    // -------------------------------------------- item 9: subconsulta escalar

    #[test]
    fn subconsulta_escalar_no_where() {
        let c = consulta("SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco) FROM pedidos)");
        assert_eq!(c.escalar.len(), 1);
        assert_eq!(c.escalar[0].nome, "sub_1");
        assert_eq!(c.escalar[0].campo, "media_preco");
        assert_eq!(c.escalar[0].de.de.tabela, "pedidos");
        let Some(Onde::Expressao(texto)) = &c.onde else {
            panic!("esperava Expressao")
        };
        assert_eq!(texto, "preco > sub_1");
    }

    #[test]
    fn duas_subconsultas_escalares_numeram_em_ordem() {
        let c = consulta(
            "SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco) FROM pedidos) AND preco \
             < (SELECT MAX(preco) FROM pedidos)",
        );
        assert_eq!(c.escalar.len(), 2);
        assert_eq!(c.escalar[0].nome, "sub_1");
        assert_eq!(c.escalar[1].nome, "sub_2");
        let Some(Onde::Expressao(texto)) = &c.onde else {
            panic!("esperava Expressao")
        };
        assert_eq!(texto, "preco > sub_1 AND preco < sub_2");
    }

    #[test]
    fn subconsulta_escalar_combinada_com_outra_condicao() {
        let c = consulta(
            "SELECT * FROM pedidos WHERE ativo = TRUE AND preco > (SELECT AVG(preco) FROM \
             pedidos)",
        );
        let Some(Onde::Expressao(texto)) = &c.onde else {
            panic!("esperava Expressao")
        };
        assert_eq!(texto, "ativo = TRUE AND preco > sub_1");
    }

    #[test]
    fn subconsulta_escalar_com_mais_de_uma_coluna_recusa() {
        let e = recusa(
            "SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco), MAX(preco) FROM pedidos)",
        );
        assert!(e.contains("mais de uma coluna"), "{e}");
    }

    #[test]
    fn subconsulta_escalar_correlacionada_recusa() {
        let e = recusa(
            "SELECT * FROM pedidos p WHERE p.preco > (SELECT AVG(preco) FROM pedidos x WHERE \
             x.cidade = p.cidade)",
        );
        assert!(e.contains("correlacionada"), "{e}");
    }

    #[test]
    fn subconsulta_escalar_traduz_com_o_json_do_contrato() {
        let c = consulta("SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco) FROM pedidos)");
        let p = traduzir_consulta(&c, "loja", &mut resolver_simples).unwrap();
        let escalar = p.pedido.campo("escalar").unwrap().lista().unwrap();
        assert_eq!(escalar.len(), 1);
        assert_eq!(escalar[0].texto_ou("nome", ""), "sub_1");
        assert_eq!(escalar[0].texto_ou("campo", ""), "media_preco");
        assert_eq!(
            escalar[0].campo("de").unwrap().texto_ou("tabela", ""),
            "pedidos"
        );
        assert_eq!(p.pedido.texto_ou("expressao", ""), "preco > sub_1");
    }

    #[test]
    fn subconsulta_escalar_com_apelido_de_agregado() {
        let c = consulta(
            "SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco) AS media FROM pedidos)",
        );
        assert_eq!(c.escalar[0].campo, "media");
    }

    /// Tabela negada dentro da subconsulta escalar recusa a consulta
    /// inteira -- a MESMA prova das outras tres formas de composicao.
    #[test]
    fn tabela_negada_no_escalar_recusa_a_consulta_inteira() {
        let c =
            consulta("SELECT * FROM pedidos WHERE preco > (SELECT AVG(preco) FROM outra_tabela)");
        let mut resolver_com_negacao = |sel: &Selecao, db: &str| -> Result<Plano> {
            if sel.de.tabela == "outra_tabela" {
                return Err(PhxError::Esquema(
                    "tabela outra_tabela negada para este usuario".into(),
                ));
            }
            resolver_simples(sel, db)
        };
        let e = traduzir_consulta(&c, "loja", &mut resolver_com_negacao)
            .unwrap_err()
            .to_string();
        assert!(e.contains("negada"), "{e}");
    }
}

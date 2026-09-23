//! Do `SELECT` lido para o pedido do protocolo.
//!
//! # O trabalho e de traducao, nao de motor
//!
//! Nada se inventa aqui. `docs/SQL.md` mediu que tudo que um `SELECT` precisa
//! ja e uma operacao com teste: `buscar` desce o indice, `varrer` pagina e
//! conta, e a ordem sai do `.ndx` sem ninguem ordenar nada.
//!
//! # E por isso a traducao recusa em vez de fingir
//!
//! O que nao tem operacao embaixo sai como recusa escrita, dizendo o que
//! faltou. Um `WHERE cidade = 'Blumenau'` sem indice em `cidade` nao vira uma
//! varredura filtrada em segredo.
//!
//! # O motivo mudou, e a recusa ficou
//!
//! Ate a 0.18 o motivo era «o `varrer` NAO filtra», e isso deixou de ser
//! verdade: o `varrer` ganhou `"onde"`. O que ele NAO ganhou foi varredura
//! ilimitada -- o `max` continua sendo quantas linhas ele EXAMINA, e a
//! resposta diz `examinadas` ao lado de `devolvidas` justamente para quem
//! chama saber sobre o que a conta foi feita.
//!
//! Um `SELECT` nao tem onde dizer isso. Traduzido para um `varrer` filtrado,
//! ele responderia sobre a PRIMEIRA PAGINA e teria a cara de ter respondido
//! sobre a tabela -- resposta errada com cara de certa, que e pior que erro.
//! Entao a recusa fica, sobre o motivo honesto: falta o indice que torna a
//! pergunta respondivel inteira.
//! DIVIDA: `SELECT` cuja pergunta nenhum indice responde inteira recusa em vez de varrer -- falta o indice, ou o planejador que escolha entre os que ha

use crate::sintaxe::{
    Alvo, Condicao, FuncaoAgregada, ItemProjetado, Onde, Ordenacao, Projecao, Selecao, Uniao,
};
use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// Uma coluna de um indice, como o esquema a declara.
#[derive(Debug, Clone, PartialEq)]
pub struct ColunaDoIndice {
    pub nome: String,
    pub desc: bool,
}

/// O que o tradutor precisa saber da tabela. Sai do `esquema`, campo por campo
/// -- e nao de uma leitura propria, porque quem abre tabela e o motor.
#[derive(Debug, Clone, PartialEq)]
pub struct IndiceInfo {
    pub nome: String,
    pub colunas: Vec<ColunaDoIndice>,
    pub unico: bool,
    pub primario: bool,
}

impl IndiceInfo {
    /// Um indice serve ao `WHERE col = ?` quando col e a PRIMEIRA coluna dele
    /// e ele nao tem mais nenhuma -- uma chave composta espera todas as
    /// partes, e mandar so a primeira nao e a mesma busca.
    pub(crate) fn atende_igualdade(&self, coluna: &str) -> bool {
        self.colunas.len() == 1 && igual_sem_caso(&self.colunas[0].nome, coluna)
    }

    /// Um indice serve ao `ORDER BY col [DESC]` quando a ordem dele e
    /// exatamente essa. A direcao esta gravada no `.ndx`: pedir ASC num indice
    /// DESC devolveria a lista ao contrario, e nao ha quem inverta depois.
    fn atende_ordem(&self, o: &Ordenacao) -> bool {
        self.colunas.len() == 1
            && igual_sem_caso(&self.colunas[0].nome, &o.coluna)
            && self.colunas[0].desc == o.desc
    }
}

pub(crate) fn igual_sem_caso(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.to_lowercase() == b.to_lowercase()
}

/// O que a resposta do protocolo deve virar antes de chegar a quem perguntou.
#[derive(Debug, Clone, PartialEq)]
pub enum Saida {
    /// Devolve a linha inteira, como o motor mandou.
    LinhaInteira,
    /// Fica so com estas colunas, nesta ordem, com estes rotulos.
    ///
    /// A projecao e do CLIENTE porque o protocolo sempre devolve a linha
    /// inteira -- e isso e uma escolha do motor, nao um esquecimento: o `.reg`
    /// e de slot fixo, e ler meia linha custa a mesma leitura.
    Colunas(Vec<(String, String)>),
    /// So o numero de registros, que sai do cabecalho da tabela.
    Contagem,
    /// A resposta vem POSICIONAL: `linhas` e uma lista de LISTAS e `colunas`
    /// e `[{nome, tipo, …}]`. E o formato da op `unir`, e ele nao e o de
    /// nenhum outro SELECT -- quem pergunta em SQL nao pode receber um
    /// envelope diferente por causa da operacao que atendeu, entao quem le
    /// esta saida vira cada lista em objeto pelo nome do cabecalho.
    ///
    /// Os nomes vem da PRIMEIRA parte, que e a regra do SQL nos quatro
    /// motores -- e por isso eles sao lidos da RESPOSTA, e nao montados aqui:
    /// o tradutor nao ve esquema nenhum.
    Posicional,
}

/// O pedido pronto, mais o que quem chamou ainda tem de fazer com a resposta.
#[derive(Debug, Clone, PartialEq)]
pub struct Plano {
    pub op: String,
    /// O objeto que vai pela porta de dados, sem o `token`.
    pub pedido: Json,
    pub saida: Saida,
    /// O que o tradutor decidiu, em portugues. Nao e enfeite: sem isso, quem
    /// escreveu `ORDER BY nome` e recebeu a ordem de digitacao nao descobre
    /// por que, e culpa o motor.
    pub notas: Vec<String>,
}

impl Plano {
    /// O pedido com o `database` preenchido e o `token` na frente, pronto para
    /// a linha do protocolo.
    pub fn linha(&self, token: &str) -> String {
        let mut pares = vec![("token".to_string(), Json::texto_de(token))];
        if let Json::Objeto(p) = &self.pedido {
            pares.extend(p.clone());
        }
        Json::Objeto(pares).escrever()
    }
}

/// Traduz. `database_corrente` entra quando o `FROM` nao disse o banco.
pub fn traduzir(s: &Selecao, indices: &[IndiceInfo], database_corrente: &str) -> Result<Plano> {
    let mut notas = Vec::new();
    let database = if s.de.database.is_empty() {
        database_corrente.to_string()
    } else {
        s.de.database.clone()
    };
    if database.is_empty() {
        return Err(PhxError::Esquema(
            "nao sei em qual database: escreva FROM banco.tabela ou escolha o banco antes".into(),
        ));
    }
    if let Some(a) = &s.de.apelido {
        notas.push(format!(
            "o apelido {a:?} da tabela foi lido e descartado: so ha uma tabela"
        ));
    }

    // GROUP BY (ou agregado fora dele) sempre vira `agrupar` -- e a excecao
    // documentada em `docs/propostas/comparativo-19.md`: um COUNT(*) que
    // teria ido pelo caminho rapido, mas o WHERE virou expressao, tambem
    // precisa de `agrupar`, porque contar com filtro exige varrer e testar
    // cada linha (o `registros` do cabecalho nao sabe nada do filtro).
    let count_com_expressao =
        matches!(s.projecao, Projecao::Contagem) && matches!(s.onde, Some(Onde::Expressao(_)));
    // `SELECT DISTINCT a, b` e `agrupar por [a, b]`: agrupar por N colunas E
    // eliminar o repetido delas -- a mesma chave, a mesma varredura, o mesmo
    // teto. A mensagem que recusava o DISTINCT aqui dizia uma verdade sobre a
    // VARREDURA («nenhuma varredura elimina repetido») e tirava dela uma
    // conclusao errada sobre o PROTOCOLO.
    if matches!(s.projecao, Projecao::Agregada(_)) || count_com_expressao || s.distinto {
        return plano_agrupar(s, &database, notas);
    }

    let saida = saida_de(&s.projecao);

    match &s.onde {
        // ------------------------------------------- filtro simples: buscar
        Some(Onde::Simples(c)) => plano_buscar(s, c, indices, &database, saida, notas),
        // ------------------- filtro em forma de expressao: varrer.expressao
        Some(Onde::Expressao(e)) => {
            plano_varrer(s, indices, &database, saida, notas, Some(e.as_str()))
        }
        // ------------------------------------------------- sem filtro: varrer
        None => plano_varrer(s, indices, &database, saida, notas, None),
    }
}

fn saida_de(p: &Projecao) -> Saida {
    match p {
        Projecao::Tudo => Saida::LinhaInteira,
        Projecao::Contagem => Saida::Contagem,
        Projecao::Colunas(cs) => Saida::Colunas(
            cs.iter()
                .map(|c| (c.nome.clone(), c.rotulo().to_string()))
                .collect(),
        ),
        Projecao::Agregada(_) => {
            unreachable!("Agregada sempre passa por plano_agrupar antes de chegar aqui")
        }
    }
}

/// O nome que o `agrupar` conhece, para uma coluna do `ORDER BY`.
///
/// Num `SELECT DISTINCT cidade AS praca ... ORDER BY praca`, o `agrupar`
/// agrupa por `cidade` e nomeia o grupo por `cidade` -- o apelido so existe na
/// projecao de SAIDA, que acontece depois. Mandar `praca` no `"ordem"` faria o
/// motor recusar falando de `"por"` e de apelido de agregado, um vocabulario
/// que quem escreveu SQL nao tem como reconhecer.
///
/// Fora do DISTINCT nada muda: ali os nomes do `ORDER BY` ja sao os que o
/// `agrupar` devolve (coluna de `por` ou apelido de agregado).
fn coluna_da_ordem<'a>(s: &'a Selecao, pedida: &'a str) -> &'a str {
    if !s.distinto {
        return pedida;
    }
    let Projecao::Colunas(cs) = &s.projecao else {
        return pedida;
    };
    cs.iter()
        .find(|c| {
            c.apelido
                .as_deref()
                .is_some_and(|a| igual_sem_caso(a, pedida))
        })
        .map(|c| c.nome.as_str())
        .unwrap_or(pedida)
}

/// O apelido do agregado que o `SELECT DISTINCT` carrega, escolhido para nao
/// colidir com nenhuma coluna projetada.
///
/// O `agrupar` recusa o apelido de agregado que tenha o mesmo nome de uma
/// coluna de agrupamento -- com razao, porque as duas viram chave do MESMO
/// objeto JSON e uma sumiria. Um apelido fixo faria `SELECT DISTINCT
/// contagem FROM t` -- uma consulta legitima sobre uma coluna chamada
/// `contagem` -- cair nessa recusa, falando de um agregado que quem escreveu
/// nunca pediu.
fn apelido_sem_colisao(colunas: &[String]) -> String {
    let mut nome = "contagem".to_string();
    while colunas.iter().any(|c| igual_sem_caso(c, &nome)) {
        nome.push('_');
    }
    nome
}

/// O apelido PADRAO de um agregado sem `AS`: `contagem` sozinho, os outros
/// com `_coluna` na cauda (`soma_preco`) -- os dois exemplos que o proprio
/// contrato mostra.
pub(crate) fn apelido_padrao(funcao: FuncaoAgregada, coluna: Option<&str>) -> String {
    match coluna {
        Some(c) => format!("{}_{}", funcao.nome_no_protocolo(), c.to_lowercase()),
        None => funcao.nome_no_protocolo().to_string(),
    }
}

/// `SELECT … UNION [ALL] SELECT …` vira a op `unir`.
///
/// # O portao de permissao, e por que nao se acrescenta um aqui
///
/// O `unir` **ja** confere tabela por tabela, e o comentario dele diz o
/// motivo: o campo `"tabela"` que o portao geral le nao existe num pedido de
/// uniao. E o terceiro caso da lei das sete operacoes que escondem tabela do
/// portao (`juntar`, `unir`, `pivotar`), e esta traducao nao cria um quarto --
/// ela desemboca exatamente naquele pedido. Conferir de novo aqui seria a
/// duplicata que um dia alguem limpa achando que e sobra.
pub fn traduzir_uniao(u: &Uniao, database_corrente: &str) -> Result<Plano> {
    let mut nomes = Vec::with_capacity(u.partes.len());
    let mut database = String::new();
    for p in &u.partes {
        let db = match p.de.database.trim() {
            "" => database_corrente.trim().to_string(),
            outro => outro.to_string(),
        };
        if db.is_empty() {
            return Err(PhxError::Esquema(
                "nao sei em qual database: escreva FROM banco.tabela ou escolha o banco \
                 antes do UNION"
                    .into(),
            ));
        }
        if database.is_empty() {
            database = db;
        } else if !igual_sem_caso(&database, &db) {
            // O pedido `unir` tem UM `database`. Traduzir mesmo assim poria o
            // banco da primeira parte no lugar do da segunda, e o `unir`
            // abriria a tabela ERRADA se houvesse uma de mesmo nome nos dois
            // -- ou recusaria falando de uma tabela que existe.
            return Err(PhxError::Esquema(format!(
                "UNION entre bancos diferentes ({database:?} e {db:?}) nao tem \
                 substrato: a op `unir` abre as tabelas de UM banco so"
            )));
        }
        nomes.push(Json::texto_de(p.de.nome_no_protocolo()));
    }
    if nomes.len() < 2 {
        return Err(PhxError::Esquema(
            "a união precisa de ao menos duas partes".into(),
        ));
    }

    // `distinta`/`tudo` sao os nomes canonicos de `juncao::Uniao::nome()`.
    let modo = if u.tudo { "tudo" } else { "distinta" };
    let pedido = pedido_com_op(
        "unir",
        vec![
            ("database".to_string(), Json::texto_de(&database)),
            ("tabelas".to_string(), Json::Lista(nomes)),
            ("modo".to_string(), Json::texto_de(modo)),
        ],
    );

    let notas = vec![
        format!(
            "{} vira `unir` com {} tabela(s): as colunas empilham por POSICAO, nunca por \
             nome -- duas tabelas com as mesmas colunas em ordem diferente trocariam os \
             valores calado, e por isso o motor confere quantidade e familia posicao a \
             posicao antes de empilhar",
            if u.tudo { "UNION ALL" } else { "UNION" },
            u.partes.len()
        ),
        "os NOMES das colunas saem da PRIMEIRA parte, como nos quatro motores; o TIPO \
         leva em conta todas -- duas escalas de decimal saem na maior"
            .into(),
        "cada tabela entra INTEIRA na memoria, e cada uma passa pelo portao de leitura \
         DELA: unir nao e a porta dos fundos para ler a tabela negada"
            .into(),
    ];

    Ok(Plano {
        op: "unir".into(),
        pedido,
        saida: Saida::Posicional,
        notas,
    })
}

/// `CREATE VIEW nome AS SELECT ...` (item 6) vira `criar_visao`. O `sql`
/// ja foi analisado uma vez em `sintaxe::criar_visao` (para recusar cedo);
/// aqui so falta embrulhar no pedido -- ele guarda TEXTO, e e reanalisado a
/// CADA uso, entao esta funcao nao abre mao de nada validando de novo.
pub fn traduzir_criar_visao(nome: &str, sql: &str, database_corrente: &str) -> Result<Plano> {
    if database_corrente.is_empty() {
        return Err(PhxError::Esquema(
            "nao sei em qual database: escolha o banco antes do CREATE VIEW".into(),
        ));
    }
    Ok(Plano {
        op: "criar_visao".into(),
        pedido: pedido_com_op(
            "criar_visao",
            vec![
                ("database".to_string(), Json::texto_de(database_corrente)),
                ("nome".to_string(), Json::texto_de(nome)),
                ("sql".to_string(), Json::texto_de(sql)),
            ],
        ),
        saida: Saida::LinhaInteira,
        notas: vec![
            "CREATE VIEW vira `criar_visao`: a visao guarda TEXTO e e reanalisada a cada \
             uso -- se a tabela dela sumir, quem descobre e quem tenta USAR a visao, \
             nomeando a tabela, nao quem a criou"
                .into(),
        ],
    })
}

/// `DROP VIEW nome` vira `excluir_visao`.
pub fn traduzir_excluir_visao(nome: &str, database_corrente: &str) -> Result<Plano> {
    if database_corrente.is_empty() {
        return Err(PhxError::Esquema(
            "nao sei em qual database: escolha o banco antes do DROP VIEW".into(),
        ));
    }
    Ok(Plano {
        op: "excluir_visao".into(),
        pedido: pedido_com_op(
            "excluir_visao",
            vec![
                ("database".to_string(), Json::texto_de(database_corrente)),
                ("nome".to_string(), Json::texto_de(nome)),
            ],
        ),
        saida: Saida::LinhaInteira,
        notas: vec!["DROP VIEW vira `excluir_visao`".into()],
    })
}

/// `GROUP BY` (e o agregado sem ele) vira `agrupar`. Ela nunca escolhe
/// indice -- agregar precisa varrer os grupos inteiros, entao `onde` (ou
/// `expressao`) e sempre um FILTRO da varredura, nunca uma descida de
/// arvore.
fn plano_agrupar(s: &Selecao, database: &str, mut notas: Vec<String>) -> Result<Plano> {
    let mut pares = base_do_pedido(&s.de, database);
    // O `DISTINCT` agrupa pelas colunas PROJETADAS; o `GROUP BY` pelas que
    // ele nomeia. A sintaxe ja garantiu que os dois nao chegam juntos.
    let colunas_distintas: Vec<String> = match (&s.distinto, &s.projecao) {
        (true, Projecao::Colunas(cs)) => cs.iter().map(|c| c.nome.clone()).collect(),
        _ => Vec::new(),
    };
    let por: &[String] = if s.distinto {
        &colunas_distintas
    } else {
        &s.agrupar_por
    };
    pares.push((
        "por".to_string(),
        Json::Lista(por.iter().map(Json::texto_de).collect()),
    ));

    // O DISTINCT sai ANTES do `match` da projecao porque a projecao dele e
    // `Colunas`, que naquele `match` nao existe -- e cair no `unreachable!`
    // seria trocar uma recusa nomeada por um estouro.
    let distinto = if s.distinto {
        // O `agrupar` EXIGE um agregado: sem `agregados` ele injeta um
        // `contagem` sozinho (`servidor.rs::op_agrupar`). Mandar o agregado
        // explicito e o que deixa escolher o APELIDO dele -- e o apelido
        // importa, porque o `agrupar` recusa o que colide com uma coluna de
        // agrupamento: `SELECT DISTINCT contagem FROM t` seria recusado pelo
        // motor, com uma mensagem sobre apelido que quem escreveu SQL nao tem
        // como entender. O apelido nao sai daqui na resposta de jeito nenhum:
        // a `Saida::Colunas` abaixo lista so as colunas pedidas, e a projecao
        // do `resposta_do_sql` joga fora o que nao esta nela.
        let apelido = apelido_sem_colisao(&colunas_distintas);
        let agregado = Json::Objeto(vec![
            ("funcao".to_string(), Json::texto_de("contagem")),
            ("apelido".to_string(), Json::texto_de(&apelido)),
        ]);
        let colunas_saida: Vec<(String, String)> = match &s.projecao {
            Projecao::Colunas(cs) => cs
                .iter()
                .map(|c| (c.nome.clone(), c.rotulo().to_string()))
                .collect(),
            _ => unreachable!("a sintaxe so deixa DISTINCT com projecao de colunas"),
        };
        notas.push(format!(
            "SELECT DISTINCT vira `agrupar` por {} coluna(s) -- agrupar por elas E \
             eliminar o repetido delas. A contagem de cada grupo e calculada e \
             descartada na projecao: o `agrupar` exige um agregado. NULO conta como \
             um valor, e dois NULOS sao a MESMA linha, como nos quatro motores",
            colunas_distintas.len()
        ));
        Some((vec![agregado], Saida::Colunas(colunas_saida)))
    } else {
        None
    };

    let (agregados, saida) = match (distinto, &s.projecao) {
        (Some(pronto), _) => pronto,
        (None, Projecao::Agregada(itens)) => {
            let mut agregados = Vec::with_capacity(itens.len());
            let mut colunas_saida = Vec::with_capacity(itens.len());
            for item in itens {
                match item {
                    ItemProjetado::Coluna(c) => {
                        colunas_saida.push((c.nome.clone(), c.rotulo().to_string()));
                    }
                    ItemProjetado::Agregado {
                        funcao,
                        coluna,
                        apelido,
                    } => {
                        let apelido = apelido
                            .clone()
                            .unwrap_or_else(|| apelido_padrao(*funcao, coluna.as_deref()));
                        let mut par = vec![
                            (
                                "funcao".to_string(),
                                Json::texto_de(funcao.nome_no_protocolo()),
                            ),
                            ("apelido".to_string(), Json::texto_de(&apelido)),
                        ];
                        if let Some(c) = coluna {
                            par.push(("coluna".to_string(), Json::texto_de(c)));
                        }
                        agregados.push(Json::Objeto(par));
                        colunas_saida.push((apelido.clone(), apelido));
                    }
                }
            }
            (agregados, Saida::Colunas(colunas_saida))
        }
        // O COUNT(*) que caiu aqui por causa do WHERE em forma de expressao
        // -- ver `count_com_expressao` em `traduzir`. Sai como um agregado
        // `contagem` solto, com o mesmo apelido padrao de sempre.
        (None, Projecao::Contagem) => {
            let apelido = apelido_padrao(FuncaoAgregada::Contagem, None);
            let agregado = Json::Objeto(vec![
                ("funcao".to_string(), Json::texto_de("contagem")),
                ("apelido".to_string(), Json::texto_de(&apelido)),
            ]);
            notas.push(
                "COUNT(*) com WHERE em forma de expressao vira `agrupar` com um agregado \
                 `contagem` solto -- a resposta tem `linhas` (uma so) em vez do campo \
                 `registros` do caminho rapido"
                    .into(),
            );
            (
                vec![agregado],
                Saida::Colunas(vec![(apelido.clone(), apelido)]),
            )
        }
        _ => unreachable!(
            "so DISTINCT, Agregada e Contagem (com WHERE-expressao) chegam em plano_agrupar"
        ),
    };
    pares.push(("agregados".to_string(), Json::Lista(agregados)));

    match &s.onde {
        None => {
            pares.push(("onde".to_string(), Json::Lista(Vec::new())));
            pares.push(("expressao".to_string(), Json::texto_de("")));
        }
        Some(Onde::Simples(c)) => {
            pares.push((
                "onde".to_string(),
                Json::Lista(vec![Json::Objeto(vec![
                    ("coluna".to_string(), Json::texto_de(&c.coluna)),
                    ("op".to_string(), Json::texto_de(c.op.simbolo())),
                    ("valor".to_string(), literal_para_json(&c.valor)),
                ])]),
            ));
            pares.push(("expressao".to_string(), Json::texto_de("")));
        }
        Some(Onde::Expressao(e)) => {
            pares.push(("onde".to_string(), Json::Lista(Vec::new())));
            pares.push(("expressao".to_string(), Json::texto_de(e)));
        }
    }

    pares.push((
        "tendo".to_string(),
        Json::texto_de(s.tendo.as_deref().unwrap_or("")),
    ));

    pares.push((
        "ordem".to_string(),
        Json::Lista(
            s.ordem_lista
                .iter()
                .map(|o| {
                    Json::Objeto(vec![
                        (
                            "coluna".to_string(),
                            Json::texto_de(coluna_da_ordem(s, &o.coluna)),
                        ),
                        ("desc".to_string(), Json::Bool(o.desc)),
                    ])
                })
                .collect(),
        ),
    ));

    if let Some(l) = s.limite {
        pares.push(("max".to_string(), Json::de_u64(l)));
    }
    if s.salto > 0 {
        // O DISTINCT herda a recusa porque herda a OPERACAO: e o mesmo
        // `agrupar`, e ele nao tem campo `pular`. A frase nomeia a clausula
        // que quem escreveu usou, e nao a que o tradutor escolheu.
        let clausula = if s.distinto { "DISTINCT" } else { "GROUP BY" };
        return Err(PhxError::Esquema(format!(
            "OFFSET com {clausula} nao tem substrato nesta rodada: o contrato de \
             `agrupar` nao tem campo `pular` -- pagine com um LIMIT maior, por enquanto"
        )));
    }

    if !s.distinto {
        notas.push(format!(
            "GROUP BY vira `agrupar`: {} coluna(s) de agrupamento -- sem indice nenhum, porque \
             agregar precisa varrer os grupos inteiros",
            s.agrupar_por.len()
        ));
    }
    // O teto e o do `agrupar`, e o DISTINCT o herda inteiro: os grupos ficam
    // TODOS em memoria ao mesmo tempo, e o motor recusa acima de
    // `recursos.max_linhas` nomeando o teto. Quem pede DISTINCT de uma coluna
    // de alta cardinalidade paga isso, e a recusa diz o que fazer.
    if s.distinto {
        notas.push(
            "o teto e o do `agrupar`: os grupos ficam TODOS em memoria ao mesmo tempo, e \
             acima de `recursos.max_linhas` o motor recusa nomeando o teto -- um DISTINCT \
             de coluna com muitos valores distintos para ali"
                .into(),
        );
    }

    Ok(Plano {
        op: "agrupar".into(),
        pedido: pedido_com_op("agrupar", pares),
        saida,
        notas,
    })
}

pub(crate) fn base_do_pedido(de: &Alvo, database: &str) -> Vec<(String, Json)> {
    vec![
        ("database".to_string(), Json::texto_de(database)),
        ("tabela".to_string(), Json::texto_de(de.nome_no_protocolo())),
    ]
}

fn plano_buscar(
    s: &Selecao,
    c: &Condicao,
    indices: &[IndiceInfo],
    database: &str,
    saida: Saida,
    mut notas: Vec<String>,
) -> Result<Plano> {
    use crate::lexico::Comparador;
    if c.op != Comparador::Igual {
        return Err(PhxError::Esquema(format!(
            "WHERE {} {} ... nao tem substrato: o indice desce ate uma chave IGUAL, e a \
             faixa ainda nao esta exposta no protocolo. So `=` passa por aqui",
            c.coluna,
            c.op.simbolo()
        )));
    }
    let Some(ix) = indices.iter().find(|i| i.atende_igualdade(&c.coluna)) else {
        let candidatos: Vec<&str> = indices
            .iter()
            .filter(|i| i.colunas.len() == 1)
            .map(|i| i.colunas[0].nome.as_str())
            .collect();
        return Err(PhxError::Esquema(format!(
            "WHERE {} = ... exige um indice de uma coluna sobre {}. Nao existe. O \
             `varrer` filtra, mas dentro da pagina que ele EXAMINA -- e um SELECT \
             que respondesse sobre a primeira pagina teria a cara de ter \
             respondido sobre a tabela. Ha indice de coluna unica sobre: {}",
            c.coluna,
            c.coluna,
            if candidatos.is_empty() {
                "nenhuma coluna".to_string()
            } else {
                candidatos.join(", ")
            }
        )));
    };
    if let Some(o) = &s.ordem {
        // A ordem de `buscar` e a das linhas daquela chave, e nao a de outra
        // coluna. Aceitar calado devolveria a lista em ordem nenhuma.
        if !igual_sem_caso(&o.coluna, &c.coluna) {
            return Err(PhxError::Esquema(format!(
                "WHERE {} = ... com ORDER BY {} nao tem substrato: `buscar` devolve na \
                 ordem do indice do filtro, e nao ha quem ordene depois",
                c.coluna, o.coluna
            )));
        }
        notas.push(format!(
            "ORDER BY {} ja e a ordem do indice {} usado no filtro",
            o.coluna, ix.nome
        ));
    }
    if s.salto > 0 {
        return Err(PhxError::Esquema(
            "OFFSET com WHERE nao tem substrato: `buscar` nao pula, e pular no cliente \
             seria trazer tudo para jogar fora"
                .into(),
        ));
    }
    if matches!(s.projecao, Projecao::Contagem) {
        notas.push(
            "COUNT(*) com WHERE conta o que a busca devolveu -- o campo `encontrados` da \
             resposta, e nao o `registros` da tabela"
                .into(),
        );
    }

    let mut pares = base_do_pedido(&s.de, database);
    pares.push(("indice".to_string(), Json::texto_de(&ix.nome)));
    pares.push((
        "chave".to_string(),
        Json::Lista(vec![literal_para_json(&c.valor)]),
    ));
    if let Some(l) = s.limite {
        pares.push(("max".to_string(), Json::de_u64(l)));
    }
    notas.push(format!(
        "indice {} escolhido pelo WHERE -- e nao ha planejador: se houvesse dois \
         candidatos, o primeiro declarado venceria",
        ix.nome
    ));
    Ok(Plano {
        op: "buscar".into(),
        pedido: pedido_com_op("buscar", pares),
        saida,
        notas,
    })
}

fn plano_varrer(
    s: &Selecao,
    indices: &[IndiceInfo],
    database: &str,
    saida: Saida,
    mut notas: Vec<String>,
    expressao: Option<&str>,
) -> Result<Plano> {
    let mut pares = base_do_pedido(&s.de, database);

    if let Some(e) = expressao {
        // O COUNT(*) rapido le `registros` do cabecalho, que nao sabe nada
        // de filtro -- por isso `traduzir` desvia esse caso para
        // `plano_agrupar` ANTES de chegar aqui, e este `debug_assert!` e o
        // cinto alem do suspensorio: se algum dia esse desvio esquecer um
        // caso, isto acusa em teste em vez de devolver o total da tabela com
        // cara de resposta filtrada.
        debug_assert!(
            !matches!(s.projecao, Projecao::Contagem),
            "COUNT(*) com WHERE em forma de expressao tinha de ter ido por plano_agrupar"
        );
        pares.push(("expressao".to_string(), Json::texto_de(e)));
        notas.push("varredura com expressao: nao ha indice para esta forma".into());
    }

    if let Some(o) = &s.ordem {
        let Some(ix) = indices.iter().find(|i| i.atende_ordem(o)) else {
            let mesma_coluna = indices
                .iter()
                .find(|i| i.colunas.len() == 1 && igual_sem_caso(&i.colunas[0].nome, &o.coluna));
            return Err(PhxError::Esquema(match mesma_coluna {
                // A SAIDA vai na mensagem, e ela diz ONDE se declara --
                // porque nao ha operacao de acrescentar indice a tabela que ja
                // existe, e um conselho que manda fazer o que nao da para
                // fazer e pior que nenhum (pedido 245, O6).
                Some(i) => format!(
                    "ORDER BY {} {} nao tem substrato: o indice {} guarda essa coluna em \
                     {}, e a direcao esta gravada no .ndx -- nao ha quem inverta a lista \
                     depois. Quem precisa das duas direcoes declara dois indices na \
                     criacao da tabela, um deles com a marca `desc` ({} desc)",
                    o.coluna,
                    if o.desc { "DESC" } else { "ASC" },
                    i.nome,
                    if i.colunas[0].desc { "DESC" } else { "ASC" },
                    o.coluna
                ),
                None => format!(
                    "ORDER BY {} exige um indice de uma coluna sobre {}. Nao existe, e \
                     nao ha ordenador: a ordem do PhxSql sai do .ndx, sem ordenar nada",
                    o.coluna, o.coluna
                ),
            }));
        };
        pares.push(("indice".to_string(), Json::texto_de(&ix.nome)));
        notas.push(format!(
            "ORDER BY atendido pelo indice {} -- a ordem sai do .ndx, sem ordenar nada",
            ix.nome
        ));
    } else {
        notas.push(
            "sem ORDER BY a ordem e a de DIGITACAO, que no PhxSql e estavel: o .reg nunca \
             reaproveita slot excluido"
                .into(),
        );
    }

    if s.salto > 0 {
        pares.push(("pular".to_string(), Json::de_u64(s.salto)));
    }
    match s.limite {
        Some(l) => pares.push(("max".to_string(), Json::de_u64(l))),
        None if matches!(s.projecao, Projecao::Contagem) => {
            // A contagem sai do cabecalho: `registros` vem na resposta mesmo
            // com uma linha pedida, e trazer a tabela inteira para conta-la
            // seria o erro que a bancada ja cometeu uma vez.
            pares.push(("max".to_string(), Json::de_u64(1)));
            notas.push(
                "COUNT(*) le o campo `registros` da resposta, que sai do cabecalho em O(1) \
                 -- nenhuma linha e varrida para contar"
                    .into(),
            );
        }
        None => notas.push(
            "sem LIMIT o servidor aplica o teto dele (`max_linhas` do config.json), e a \
             resposta diz se ha mais"
                .into(),
        ),
    }

    Ok(Plano {
        op: "varrer".into(),
        pedido: pedido_com_op("varrer", pares),
        saida,
        notas,
    })
}

pub(crate) fn pedido_com_op(op: &str, pares: Vec<(String, Json)>) -> Json {
    let mut todos = vec![("op".to_string(), Json::texto_de(op))];
    todos.extend(pares);
    Json::Objeto(todos)
}

/// O valor como o protocolo o espera.
///
/// Numero vira TEXTO, e nao `Json::Numero`. E deliberado: o motor le decimal
/// de texto justamente para nao passar por `f64`, e transformar aqui desfaria
/// a garantia dentro do tradutor que existe para preserva-la.
pub(crate) fn literal_para_json(l: &crate::sintaxe::Literal) -> Json {
    use crate::sintaxe::Literal;
    match l {
        Literal::Numero(n) => Json::texto_de(n),
        Literal::Texto(t) => Json::texto_de(t),
        Literal::Bool(b) => Json::Bool(*b),
        Literal::Nulo => Json::Nulo,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sintaxe::analisar;

    fn ix(nome: &str, coluna: &str, desc: bool, unico: bool) -> IndiceInfo {
        IndiceInfo {
            nome: nome.into(),
            colunas: vec![ColunaDoIndice {
                nome: coluna.into(),
                desc,
            }],
            unico,
            primario: unico,
        }
    }

    fn tabela() -> Vec<IndiceInfo> {
        vec![
            ix("porId", "id", false, true),
            ix("porNome", "nome", false, false),
            IndiceInfo {
                nome: "porCidadeLimite".into(),
                colunas: vec![
                    ColunaDoIndice {
                        nome: "cidade".into(),
                        desc: false,
                    },
                    ColunaDoIndice {
                        nome: "limite".into(),
                        desc: true,
                    },
                ],
                unico: false,
                primario: false,
            },
        ]
    }

    fn plano(sql: &str) -> Plano {
        traduzir(&analisar(sql).unwrap(), &tabela(), "Comercial").unwrap()
    }

    fn recusa(sql: &str) -> String {
        traduzir(&analisar(sql).unwrap(), &tabela(), "Comercial")
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn select_estrela_vira_varrer() {
        let p = plano("SELECT * FROM Clientes");
        assert_eq!(p.op, "varrer");
        assert_eq!(p.saida, Saida::LinhaInteira);
        let j = p.pedido;
        assert_eq!(j.texto_ou("op", ""), "varrer");
        assert_eq!(j.texto_ou("database", ""), "Comercial");
        assert_eq!(j.texto_ou("tabela", ""), "Clientes");
    }

    #[test]
    fn schema_chega_no_campo_tabela() {
        let p = plano("SELECT * FROM matriz.estoque");
        assert_eq!(p.pedido.texto_ou("tabela", ""), "matriz.estoque");
        assert_eq!(p.pedido.texto_ou("database", ""), "Comercial");
    }

    #[test]
    fn database_do_from_vence_o_corrente() {
        let p = plano("SELECT * FROM Outro.filial.estoque");
        assert_eq!(p.pedido.texto_ou("database", ""), "Outro");
        assert_eq!(p.pedido.texto_ou("tabela", ""), "filial.estoque");
    }

    #[test]
    fn where_de_igualdade_vira_buscar() {
        let p = plano("SELECT * FROM Clientes WHERE id = 7");
        assert_eq!(p.op, "buscar");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porId");
        let Some(Json::Lista(chave)) = p.pedido.campo("chave") else {
            panic!("esperava lista na chave")
        };
        // Numero vai como TEXTO -- e a garantia do decimal.
        assert_eq!(chave[0], Json::texto_de("7"));
    }

    #[test]
    fn decimal_chega_inteiro_na_chave() {
        let p = traduzir(
            &analisar("SELECT * FROM t WHERE limite = 1500.00").unwrap(),
            &[ix("porLimite", "limite", false, false)],
            "C",
        )
        .unwrap();
        let Some(Json::Lista(chave)) = p.pedido.campo("chave") else {
            panic!()
        };
        assert_eq!(chave[0], Json::texto_de("1500.00"));
    }

    #[test]
    fn order_by_usa_o_indice() {
        let p = plano("SELECT * FROM Clientes ORDER BY nome");
        assert_eq!(p.op, "varrer");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porNome");
    }

    #[test]
    fn limite_e_salto() {
        let p = plano("SELECT * FROM Clientes LIMIT 10 OFFSET 40");
        assert_eq!(p.pedido.inteiro_ou("max", 0), 10);
        assert_eq!(p.pedido.inteiro_ou("pular", 0), 40);
    }

    #[test]
    fn count_pede_uma_linha_so() {
        let p = plano("SELECT COUNT(*) FROM Clientes");
        assert_eq!(p.saida, Saida::Contagem);
        assert_eq!(
            p.pedido.inteiro_ou("max", 0),
            1,
            "contar nao pode varrer a tabela: o numero sai do cabecalho"
        );
    }

    #[test]
    fn projecao_e_do_cliente_e_o_plano_diz_isso() {
        let p = plano("SELECT nome, uf AS estado FROM Clientes");
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![
                ("nome".into(), "nome".into()),
                ("uf".into(), "estado".into())
            ])
        );
    }

    /// O ponto onde a traducao tem de recusar em vez de inventar. Cada um
    /// destes, aceito calado, devolveria resposta errada sem erro nenhum.
    #[test]
    fn recusa_o_que_nao_tem_substrato() {
        // Confere o que a recusa PROMETE -- a coluna pedida e a lista de
        // saidas --, e nao a redacao dela. A versao anterior casava a frase
        // «NAO filtra», que virou mentira no dia em que o `varrer` ganhou
        // `"onde"`: o teste continuou verde defendendo um motivo falso.
        let e = recusa("SELECT * FROM Clientes WHERE cidade = 'Blumenau'");
        assert!(e.contains("cidade"), "{e}");
        assert!(e.contains("indice"), "{e}");
        assert!(e.contains("porNome") || e.contains("nome"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id > 7");
        assert!(e.contains("faixa"), "{e}");

        let e = recusa("SELECT * FROM Clientes ORDER BY limite");
        assert!(e.contains("nao ha ordenador"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id = 1 ORDER BY nome");
        assert!(e.contains("ordem do indice do filtro"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id = 1 LIMIT 5 OFFSET 5");
        assert!(e.contains("OFFSET"), "{e}");
    }

    /// Um indice composto nao atende `WHERE primeira_coluna = ?`: a chave
    /// espera todas as partes.
    #[test]
    fn indice_composto_nao_serve_a_meia_chave() {
        let e = recusa("SELECT * FROM Clientes WHERE cidade = 'Blumenau'");
        assert!(e.contains("indice de uma coluna"), "{e}");
    }

    #[test]
    fn direcao_do_indice_e_a_do_ndx() {
        let e = traduzir(
            &analisar("SELECT * FROM t ORDER BY nome DESC").unwrap(),
            &[ix("porNome", "nome", false, false)],
            "C",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("gravada no .ndx"), "{e}");
        // E no sentido certo ele passa.
        let p = traduzir(
            &analisar("SELECT * FROM t ORDER BY nome DESC").unwrap(),
            &[ix("porNomeDesc", "nome", true, false)],
            "C",
        )
        .unwrap();
        assert_eq!(p.pedido.texto_ou("indice", ""), "porNomeDesc");
    }

    /// **A recusa diz a SAIDA, e diz onde ela se declara** -- pedido 245, O6.
    ///
    /// Ela explicava bem por que nao da, e nao dizia o que fazer. E o «o que
    /// fazer» aqui tem uma armadilha propria: nao ha operacao de acrescentar
    /// indice a tabela que ja existe -- o indice se declara no `criar_tabela`
    /// --, entao um conselho generico («crie um indice DESC») mandaria fazer o
    /// que nao da para fazer.
    ///
    /// Reponha o defeito tirando a segunda frase do `format!`: as duas
    /// asserções de baixo caem e a primeira continua verde, que e o que
    /// mostra que ela sozinha nao bastava.
    #[test]
    fn a_recusa_da_direcao_diz_o_que_fazer_e_onde() {
        let e = traduzir(
            &analisar("SELECT * FROM t ORDER BY nome DESC").unwrap(),
            &[ix("porNome", "nome", false, false)],
            "C",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("gravada no .ndx"), "{e}");
        assert!(e.contains("criacao da tabela"), "{e}");
        assert!(e.contains("nome desc"), "{e}");
    }

    #[test]
    fn sem_banco_nenhum_recusa() {
        let e = traduzir(&analisar("SELECT * FROM t").unwrap(), &tabela(), "")
            .unwrap_err()
            .to_string();
        assert!(e.contains("database"), "{e}");
    }

    #[test]
    fn a_linha_do_protocolo_leva_o_token_na_frente() {
        let l = plano("SELECT * FROM Clientes").linha("demo");
        assert!(l.starts_with("{\"token\":\"demo\""), "{l}");
        assert!(l.contains("\"op\":\"varrer\""), "{l}");
    }

    #[test]
    fn caso_da_coluna_nao_atrapalha() {
        // O motor aceita `Nome` e `nome` como a mesma coluna na tela; o
        // tradutor nao pode ser mais exigente que ele.
        let p = plano("SELECT * FROM Clientes WHERE ID = 3");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porId");
    }

    // --------------------------------------------- item 2: WHERE-expressao

    #[test]
    fn where_em_forma_de_expressao_vira_varrer_com_expressao() {
        let p = plano("SELECT * FROM Clientes WHERE limite > 100 AND cidade = 'Blumenau'");
        assert_eq!(p.op, "varrer");
        assert_eq!(
            p.pedido.texto_ou("expressao", ""),
            "limite > 100 AND cidade = 'Blumenau'"
        );
        assert!(
            p.notas.iter().any(|n| n.contains("nao ha indice")),
            "{:?}",
            p.notas
        );
        // Sem indice nenhum sobre `limite` ou `cidade` sozinha -- e mesmo
        // assim NAO recusa, porque a varredura com expressao nao depende de
        // indice. E o ponto inteiro do item 2.
    }

    #[test]
    fn where_em_forma_de_expressao_continua_paginando() {
        let p =
            plano("SELECT * FROM Clientes WHERE limite > 100 AND ativo = TRUE LIMIT 10 OFFSET 5");
        assert_eq!(p.pedido.inteiro_ou("max", 0), 10);
        assert_eq!(p.pedido.inteiro_ou("pular", 0), 5);
        assert_eq!(
            p.pedido.texto_ou("expressao", ""),
            "limite > 100 AND ativo = TRUE"
        );
    }

    /// Uma comparacao SO, com qualquer comparador -- inclusive `>` -- e a
    /// forma antiga (`coluna op literal`) e continua no caminho de sempre:
    /// so `=` tem indice, e `>` recusa exatamente como recusava antes do
    /// item 2. O item 2 nao muda ISSO -- ele so abre uma porta nova para o
    /// que tem AND/OR/funcao/IN/etc, que nunca tiveram caminho nenhum.
    #[test]
    fn uma_comparacao_so_continua_pelo_caminho_antigo_mesmo_sem_ser_igualdade() {
        let e = recusa("SELECT * FROM Clientes WHERE limite > 100");
        assert!(e.contains("faixa"), "{e}");
    }

    /// A excecao que o proprio item 2 documenta: `COUNT(*)` com WHERE em
    /// forma de expressao NAO tem substrato AINDA -- ela so ganha um em
    /// `agrupar` (o proximo item). Ate la a recusa e honesta.
    #[test]
    fn count_com_expressao_recusa_ate_o_agrupar_existir() {
        // O item 3 fechou a excecao que o item 2 deixou documentada: agora
        // COUNT(*) com WHERE em forma de expressao vira `agrupar`, nao
        // recusa mais.
        let p = plano("SELECT COUNT(*) FROM Clientes WHERE limite > 100 AND ativo = TRUE");
        assert_eq!(p.op, "agrupar");
        assert_eq!(p.pedido.campo("por").unwrap(), &Json::Lista(vec![]));
        assert_eq!(
            p.pedido.texto_ou("expressao", ""),
            "limite > 100 AND ativo = TRUE"
        );
        let ags = p.pedido.campo("agregados").unwrap().lista().unwrap();
        assert_eq!(ags.len(), 1);
        assert_eq!(ags[0].texto_ou("funcao", ""), "contagem");
        assert_eq!(ags[0].texto_ou("apelido", ""), "contagem");

        // Mas COUNT(*) com filtro SIMPLES continua no caminho de sempre.
        let p = plano("SELECT COUNT(*) FROM Clientes WHERE id = 1");
        assert_eq!(p.op, "buscar");
    }

    // ------------------------------------- item 3: agrupar (GROUP BY)

    #[test]
    fn group_by_vira_agrupar_com_o_json_do_contrato() {
        let p = plano(
            "SELECT cidade, COUNT(*), SUM(preco) AS total FROM Clientes WHERE ativo = TRUE \
             GROUP BY cidade HAVING total > 1 ORDER BY total DESC LIMIT 10",
        );
        assert_eq!(p.op, "agrupar");
        assert_eq!(p.pedido.texto_ou("database", ""), "Comercial");
        assert_eq!(p.pedido.texto_ou("tabela", ""), "Clientes");
        assert_eq!(
            p.pedido.campo("por").unwrap(),
            &Json::Lista(vec![Json::texto_de("cidade")])
        );
        let ags = p.pedido.campo("agregados").unwrap().lista().unwrap();
        assert_eq!(ags.len(), 2);
        assert_eq!(ags[0].texto_ou("funcao", ""), "contagem");
        assert_eq!(ags[0].texto_ou("apelido", ""), "contagem");
        assert!(ags[0].campo("coluna").is_none(), "COUNT(*) nao leva coluna");
        assert_eq!(ags[1].texto_ou("funcao", ""), "soma");
        assert_eq!(ags[1].texto_ou("coluna", ""), "preco");
        assert_eq!(ags[1].texto_ou("apelido", ""), "total");

        assert_eq!(
            p.pedido.campo("onde").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de("ativo")),
                ("op".to_string(), Json::texto_de("=")),
                ("valor".to_string(), Json::Bool(true)),
            ])])
        );
        assert_eq!(p.pedido.texto_ou("expressao", ""), "");
        assert_eq!(p.pedido.texto_ou("tendo", ""), "total > 1");
        assert_eq!(
            p.pedido.campo("ordem").unwrap(),
            &Json::Lista(vec![Json::Objeto(vec![
                ("coluna".to_string(), Json::texto_de("total")),
                ("desc".to_string(), Json::Bool(true)),
            ])])
        );
        assert_eq!(p.pedido.inteiro_ou("max", 0), 10);

        assert_eq!(
            p.saida,
            Saida::Colunas(vec![
                ("cidade".into(), "cidade".into()),
                ("contagem".into(), "contagem".into()),
                ("total".into(), "total".into()),
            ])
        );
    }

    #[test]
    fn agregado_sem_group_by_manda_por_vazio() {
        let p = plano("SELECT SUM(preco) FROM Clientes");
        assert_eq!(p.op, "agrupar");
        assert_eq!(p.pedido.campo("por").unwrap(), &Json::Lista(vec![]));
        let ags = p.pedido.campo("agregados").unwrap().lista().unwrap();
        // Apelido padrao: "soma_preco" -- o proprio exemplo do contrato.
        assert_eq!(ags[0].texto_ou("apelido", ""), "soma_preco");
    }

    /// Pedido 236: `COUNT(coluna)` vira o mesmo agregado de sempre, so que
    /// com `"coluna"` no JSON (conta nao-nulo, nao linha). Falhava antes do
    /// conserto (a traducao inteira recusava com "COUNT(coluna) nao tem
    /// substrato", entao nem chegava a montar `Plano`); passa depois.
    #[test]
    fn count_de_coluna_manda_funcao_contagem_com_coluna() {
        let p = plano("SELECT COUNT(preco) AS n FROM Clientes");
        assert_eq!(p.op, "agrupar");
        let ags = p.pedido.campo("agregados").unwrap().lista().unwrap();
        assert_eq!(ags.len(), 1);
        assert_eq!(ags[0].texto_ou("funcao", ""), "contagem");
        assert_eq!(ags[0].texto_ou("coluna", ""), "preco");
        assert_eq!(ags[0].texto_ou("apelido", ""), "n");
    }

    /// `COUNT(DISTINCT c)` ja tinha substrato antes desta rodada -- este
    /// teste e o controle: continua "distintos", nao "contagem".
    #[test]
    fn count_distinct_continua_distintos() {
        let p = plano("SELECT COUNT(DISTINCT preco) FROM Clientes");
        let ags = p.pedido.campo("agregados").unwrap().lista().unwrap();
        assert_eq!(ags[0].texto_ou("funcao", ""), "distintos");
        assert_eq!(ags[0].texto_ou("coluna", ""), "preco");
    }

    #[test]
    fn where_expressao_no_agrupar_usa_o_campo_expressao() {
        let p =
            plano("SELECT cidade, COUNT(*) FROM Clientes WHERE a > 1 AND b < 2 GROUP BY cidade");
        assert_eq!(p.pedido.campo("onde").unwrap(), &Json::Lista(vec![]));
        assert_eq!(p.pedido.texto_ou("expressao", ""), "a > 1 AND b < 2");
    }

    #[test]
    fn offset_com_group_by_recusa() {
        let e = recusa("SELECT cidade, COUNT(*) FROM Clientes GROUP BY cidade LIMIT 5 OFFSET 1");
        assert!(e.contains("OFFSET"), "{e}");
    }

    // ---------------------------------------------------- item 6: visoes

    #[test]
    fn traduzir_criar_visao_monta_o_pedido() {
        let p = traduzir_criar_visao("v_c", "SELECT * FROM c", "loja").unwrap();
        assert_eq!(p.op, "criar_visao");
        assert_eq!(p.pedido.texto_ou("op", ""), "criar_visao");
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
        assert_eq!(p.pedido.texto_ou("nome", ""), "v_c");
        assert_eq!(p.pedido.texto_ou("sql", ""), "SELECT * FROM c");
    }

    #[test]
    fn traduzir_criar_visao_sem_banco_recusa() {
        let e = traduzir_criar_visao("v_c", "SELECT * FROM c", "")
            .unwrap_err()
            .to_string();
        assert!(e.contains("database"), "{e}");
    }

    #[test]
    fn traduzir_excluir_visao_monta_o_pedido() {
        let p = traduzir_excluir_visao("v_c", "loja").unwrap();
        assert_eq!(p.op, "excluir_visao");
        assert_eq!(p.pedido.texto_ou("nome", ""), "v_c");
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
    }

    // ------------------------------------------------------- SELECT DISTINCT
    //
    // O que estes testes travam, e o defeito de cada um:
    //
    // 1. a traducao existe -- a recusa anterior dizia uma verdade sobre a
    //    VARREDURA e tirava dela uma conclusao errada sobre o PROTOCOLO;
    // 2. a lista de `por` tem TODAS as colunas projetadas, na ordem -- com
    //    uma so, `DISTINCT a, b` devolveria a primeira linha de cada `a`;
    // 3. a coluna de contagem que o `agrupar` exige NAO sai na resposta;
    // 4. o apelido dela nao colide com uma coluna chamada `contagem`;
    // 5. `*`, agregado e `GROUP BY` recusam NOMEANDO o que existe.

    fn selecao_distinta(sql: &str) -> crate::sintaxe::Selecao {
        analisar(sql).unwrap()
    }

    #[test]
    fn distinct_de_uma_coluna_vira_agrupar_por_ela() {
        let p = plano("SELECT DISTINCT cidade FROM Clientes");
        assert_eq!(p.op, "agrupar");
        assert_eq!(
            p.pedido.textos("por"),
            vec!["cidade".to_string()],
            "{}",
            p.pedido.escrever()
        );
        // A saida tem a coluna pedida e SO ela: a contagem que o `agrupar`
        // devolve junto e descartada aqui, na projecao do cliente.
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![("cidade".into(), "cidade".into())])
        );
    }

    /// `nomes_por` e plural no motor, e e por isso que `DISTINCT a, b`
    /// funciona: a chave do grupo e a TUPLA. Com uma coluna so, a resposta
    /// traria a primeira `b` de cada `a` -- um valor que nao e nem chave nem
    /// agregado, que e exatamente o que o `GROUP BY` desta casa recusa.
    #[test]
    fn distinct_de_varias_colunas_agrupa_pela_tupla_na_ordem_escrita() {
        let p = plano("SELECT DISTINCT cidade, nome FROM Clientes");
        assert_eq!(
            p.pedido.textos("por"),
            vec!["cidade".to_string(), "nome".to_string()]
        );
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![
                ("cidade".into(), "cidade".into()),
                ("nome".into(), "nome".into())
            ])
        );
    }

    /// O `AS` vale: o agrupamento e pela COLUNA, o rotulo e o apelido.
    #[test]
    fn distinct_com_apelido_agrupa_pela_coluna_e_rotula_pelo_apelido() {
        let p = plano("SELECT DISTINCT cidade AS praca FROM Clientes");
        assert_eq!(p.pedido.textos("por"), vec!["cidade".to_string()]);
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![("cidade".into(), "praca".into())])
        );
    }

    /// O `agrupar` EXIGE um agregado -- sem ele o motor injeta um `contagem`.
    /// O tradutor manda o dele para poder escolher o apelido, e o apelido
    /// desvia de uma coluna de mesmo nome: sem isso, `SELECT DISTINCT
    /// contagem FROM t` -- consulta legitima -- morreria no motor com uma
    /// mensagem sobre um agregado que ninguem pediu.
    #[test]
    fn o_agregado_que_o_distinct_carrega_nao_colide_com_uma_coluna_contagem() {
        let p = plano("SELECT DISTINCT contagem FROM Clientes");
        let ags = p.pedido.campo("agregados").and_then(Json::lista).unwrap();
        assert_eq!(ags.len(), 1);
        assert_eq!(ags[0].texto_ou("funcao", ""), "contagem");
        assert_eq!(ags[0].texto_ou("apelido", ""), "contagem_");
        // E a saida continua sendo so a coluna pedida.
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![("contagem".into(), "contagem".into())])
        );
    }

    #[test]
    fn distinct_com_where_e_limit_viaja_com_os_dois() {
        let p = plano("SELECT DISTINCT cidade FROM Clientes WHERE id = 7 LIMIT 5");
        assert_eq!(p.op, "agrupar");
        assert_eq!(p.pedido.inteiro_ou("max", -1), 5);
        let onde = p.pedido.campo("onde").and_then(Json::lista).unwrap();
        assert_eq!(onde.len(), 1);
        assert_eq!(onde[0].texto_ou("coluna", ""), "id");
    }

    /// O `ORDER BY` do DISTINCT usa a lista (`ordem_lista`), e nao a forma de
    /// uma coluna so do caminho de indice: o `agrupar` ordena um resultado ja
    /// em memoria, entao a segunda coluna nao pede indice composto nenhum.
    #[test]
    fn distinct_ordena_por_varias_colunas_sem_pedir_indice() {
        let s = selecao_distinta(
            "SELECT DISTINCT cidade, nome FROM Clientes ORDER BY nome, cidade DESC",
        );
        assert!(s.ordem.is_none());
        assert_eq!(s.ordem_lista.len(), 2);
        let p = plano("SELECT DISTINCT cidade, nome FROM Clientes ORDER BY nome, cidade DESC");
        let ordem = p.pedido.campo("ordem").and_then(Json::lista).unwrap();
        assert_eq!(ordem.len(), 2);
        assert_eq!(ordem[0].texto_ou("coluna", ""), "nome");
        assert!(ordem[1].booleano_ou("desc", false));
    }

    /// A regra dos quatro motores: o `ORDER BY` de um DISTINCT so alcanca
    /// coluna da lista projetada. A recusa nomeia a coluna E a lista.
    #[test]
    fn distinct_que_ordena_por_coluna_de_fora_da_lista_recusa_nomeando() {
        let e = analisar("SELECT DISTINCT cidade FROM Clientes ORDER BY nome")
            .unwrap_err()
            .to_string();
        assert!(e.contains("\"nome\""), "{e}");
        assert!(e.contains("cidade"), "{e}");
    }

    /// O DISTINCT herda a recusa do OFFSET porque herda a OPERACAO -- e a
    /// frase nomeia a clausula que quem escreveu usou.
    #[test]
    fn distinct_com_offset_recusa_nomeando_o_distinct() {
        let e = recusa("SELECT DISTINCT cidade FROM Clientes LIMIT 5 OFFSET 10");
        assert!(e.contains("OFFSET com DISTINCT"), "{e}");
        assert!(e.contains("pular"), "{e}");
    }

    /// `SELECT DISTINCT *` recusa NOMEANDO, e o motivo e medido: o `*` desta
    /// casa carrega `rowid`/`softdeleted`/`rownum`, e o `rownum` e unico por
    /// linha -- agrupar por tudo o que o `*` mostra nao tiraria repetida
    /// nenhuma e ainda pagaria a tabela de grupos.
    #[test]
    fn distinct_estrela_recusa_nomeando_as_colunas_de_sistema() {
        let e = analisar("SELECT DISTINCT * FROM Clientes")
            .unwrap_err()
            .to_string();
        assert!(e.contains("rownum"), "{e}");
        assert!(e.contains("Nomeie as colunas"), "{e}");
    }

    #[test]
    fn distinct_com_agregado_e_com_group_by_recusam_nomeando() {
        let e = analisar("SELECT DISTINCT COUNT(*) FROM Clientes")
            .unwrap_err()
            .to_string();
        assert!(e.contains("funcao agregada"), "{e}");
        assert!(e.contains("COUNT(DISTINCT coluna)"), "{e}");

        let e = analisar("SELECT DISTINCT cidade FROM Clientes GROUP BY cidade")
            .unwrap_err()
            .to_string();
        assert!(e.contains("GROUP BY/HAVING"), "{e}");
    }

    /// `COUNT(DISTINCT coluna)` e OUTRA pergunta e OUTRO caminho -- ele ja
    /// existia, e nada nesta rodada o tocou.
    #[test]
    fn count_distinct_de_coluna_continua_pelo_caminho_de_sempre() {
        let p = plano("SELECT COUNT(DISTINCT cidade) FROM Clientes");
        assert_eq!(p.op, "agrupar");
        let ags = p.pedido.campo("agregados").and_then(Json::lista).unwrap();
        assert_eq!(ags[0].texto_ou("funcao", ""), "distintos");
        assert_eq!(ags[0].texto_ou("coluna", ""), "cidade");
        // E o SELECT sem DISTINCT continua indo para a varredura.
        assert_eq!(plano("SELECT cidade FROM Clientes").op, "varrer");
    }

    /// O `distinto` se liga num lugar SO -- `Analisador::comando`. Toda
    /// `Selecao` de dentro de uma consulta composta passa por
    /// `Analisador::selecao`, que recusa a palavra nomeando: sem isso, a
    /// coluna de contagem do `agrupar` vazaria para o resultado de fora,
    /// porque `traduzir_consulta` usa o PEDIDO de cada pedaco e descarta a
    /// `saida` dele.
    #[test]
    fn distinct_dentro_de_consulta_composta_recusa_nomeando() {
        for sql in [
            "SELECT x.cidade FROM (SELECT DISTINCT cidade FROM Clientes) x",
            "WITH c AS (SELECT DISTINCT cidade FROM Clientes) SELECT cidade FROM c",
            "SELECT DISTINCT c.nome FROM Clientes c JOIN Pedidos p ON c.id = p.cid",
        ] {
            let e = crate::analisar_comando(sql).unwrap_err().to_string();
            assert!(
                e.contains("DISTINCT") && e.contains("substrato"),
                "{sql} -> {e}"
            );
        }
    }

    // --------------------------------------------------- UNION e UNION ALL
    //
    // A op `unir` ja existia -- ela e a porta dos sete cartoes de Venn da
    // tela. O que faltava era alguem escrever `UNION` e chegar la.
    //
    // O que estes testes travam:
    //
    // 1. as duas formas viram o `modo` certo (`distinta` e `tudo`), com os
    //    nomes EXATOS que `juncao::Uniao::de_texto` aceita;
    // 2. a lista de tabelas sai na ORDEM escrita -- a uniao empilha por
    //    posicao e o cabecalho sai da primeira, entao trocar a ordem troca a
    //    resposta;
    // 3. o que NAO tem substrato recusa nomeando o que existe, e nao cai num
    //    "sintaxe invalida".

    fn uniao(sql: &str) -> Plano {
        match crate::analisar_comando(sql).unwrap() {
            crate::Comando::Uniao(u) => traduzir_uniao(&u, "Comercial").unwrap(),
            outro => panic!("{sql} nao virou uniao: {outro:?}"),
        }
    }

    fn recusa_uniao(sql: &str) -> String {
        match crate::analisar_comando(sql) {
            Err(e) => e.to_string(),
            Ok(crate::Comando::Uniao(u)) => traduzir_uniao(&u, "Comercial")
                .expect_err("tinha de recusar")
                .to_string(),
            Ok(outro) => panic!("{sql} nao recusou e nao virou uniao: {outro:?}"),
        }
    }

    #[test]
    fn union_vira_unir_distinta_e_union_all_vira_tudo() {
        let p = uniao("SELECT * FROM a UNION SELECT * FROM b");
        assert_eq!(p.op, "unir");
        assert_eq!(p.pedido.texto_ou("modo", ""), "distinta");
        assert_eq!(p.pedido.texto_ou("database", ""), "Comercial");
        assert_eq!(
            p.pedido.textos("tabelas"),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(p.saida, Saida::Posicional);

        let p = uniao("SELECT * FROM a UNION ALL SELECT * FROM b");
        assert_eq!(p.pedido.texto_ou("modo", ""), "tudo");
    }

    /// Tres partes, e a ORDEM e a escrita: o cabecalho sai da primeira, e a
    /// uniao empilha por posicao.
    #[test]
    fn a_uniao_de_tres_mantem_a_ordem_escrita() {
        let p = uniao("SELECT * FROM c UNION ALL SELECT * FROM a UNION ALL SELECT * FROM b");
        assert_eq!(
            p.pedido.textos("tabelas"),
            vec!["c".to_string(), "a".to_string(), "b".to_string()]
        );
    }

    /// O `FROM banco.tabela` manda, e o esquema viaja junto -- `unir` abre
    /// `matriz.estoque` do mesmo jeito que `varrer` abre.
    #[test]
    fn a_uniao_le_o_banco_do_from_e_o_esquema_da_tabela() {
        let p = uniao("SELECT * FROM loja.matriz.estoque UNION SELECT * FROM loja.filial.estoque");
        assert_eq!(p.pedido.texto_ou("database", ""), "loja");
        assert_eq!(
            p.pedido.textos("tabelas"),
            vec!["matriz.estoque".to_string(), "filial.estoque".to_string()]
        );
    }

    /// O pedido `unir` tem UM `database`. Duas bases recusam NOMEANDO as
    /// duas, em vez de a segunda ser aberta na base da primeira -- que
    /// abriria a tabela errada se houvesse uma de mesmo nome nas duas.
    ///
    /// O nome de TRES partes e que diz o banco (`banco.esquema.tabela`); o de
    /// duas e `esquema.tabela` dentro do banco corrente, e por isso
    /// `um.a UNION dois.b` passa -- sao dois ESQUEMAS do mesmo banco.
    #[test]
    fn uniao_entre_bancos_diferentes_recusa_nomeando_os_dois() {
        let e = recusa_uniao("SELECT * FROM um.x.a UNION SELECT * FROM dois.y.b");
        assert!(e.contains("\"um\"") && e.contains("\"dois\""), "{e}");
        // E dois ESQUEMAS do mesmo banco continuam passando.
        let p = uniao("SELECT * FROM um.a UNION SELECT * FROM dois.b");
        assert_eq!(p.pedido.texto_ou("database", ""), "Comercial");
        assert_eq!(
            p.pedido.textos("tabelas"),
            vec!["um.a".to_string(), "dois.b".to_string()]
        );
    }

    /// **A recusa central desta rodada.** A op `unir` recebe TABELAS
    /// inteiras: nao ha `onde`, nao ha projecao, nao ha ordem. Tudo o que
    /// passar disso recusa dizendo o que existe -- e apontando onde a conta
    /// de mudar isso esta medida.
    #[test]
    fn uniao_com_filtro_ou_projecao_recusa_nomeando_o_que_existe() {
        for sql in [
            "SELECT nome FROM a UNION SELECT nome FROM b",
            "SELECT * FROM a WHERE id = 1 UNION SELECT * FROM b",
            "SELECT * FROM a UNION SELECT * FROM b WHERE id = 1",
            "SELECT * FROM a UNION SELECT cidade FROM b",
        ] {
            let e = recusa_uniao(sql);
            assert!(e.contains("TABELAS inteiras"), "{sql} -> {e}");
            assert!(e.contains("SELECT * FROM a UNION"), "{sql} -> {e}");
        }
    }

    /// O `unir` nao tem campo de ordem, e o `max` dele e o teto de memoria do
    /// servidor -- usa-lo como LIMIT faria a resposta dizer `truncado` por um
    /// corte que quem perguntou pediu. A recusa diz as duas coisas.
    #[test]
    fn uniao_com_ordem_ou_limite_recusa_nomeando_o_teto() {
        for sql in [
            "SELECT * FROM a UNION SELECT * FROM b ORDER BY nome",
            "SELECT * FROM a UNION SELECT * FROM b LIMIT 10",
            "SELECT * FROM a UNION SELECT * FROM b OFFSET 3",
        ] {
            let e = recusa_uniao(sql);
            assert!(e.contains("teto de memoria"), "{sql} -> {e}");
        }
    }

    /// O `modo` e UM para a lista toda. Misturar as duas formas escolheria
    /// uma calado, e o que se perderia e justamente a linha repetida.
    #[test]
    fn misturar_union_e_union_all_recusa_nomeando() {
        let e = recusa_uniao("SELECT * FROM a UNION SELECT * FROM b UNION ALL SELECT * FROM c");
        assert!(e.contains("UM modo"), "{e}");
    }

    /// O `WHERE` para no `UNION`, e nao o engole. Sem isto, a recusa falaria
    /// de uma expressao ilegivel em vez de falar do UNION que alguem escreveu.
    #[test]
    fn o_where_para_no_union_em_vez_de_engolir_o_segundo_select() {
        let e = recusa_uniao("SELECT * FROM a WHERE id = 1 UNION SELECT * FROM b");
        assert!(e.contains("TABELAS inteiras"), "{e}");
        // E o SELECT sem UNION continua lendo o WHERE inteiro.
        let s = analisar("SELECT * FROM t WHERE a = 1 AND b = 2").unwrap();
        assert!(matches!(&s.onde, Some(Onde::Expressao(e)) if e.contains("AND")));
    }

    /// `UNION` com junção, subconsulta, CTE ou janela recusa NOMEANDO, e nao
    /// morre com "sobrou UNION depois do fim do comando" -- verdade que nao
    /// ensina nada.
    #[test]
    fn uniao_com_forma_composta_recusa_nomeando() {
        for sql in [
            "SELECT * FROM a JOIN b ON a.id = b.id UNION SELECT * FROM c",
            "SELECT * FROM (SELECT * FROM a) x UNION SELECT * FROM c",
        ] {
            let e = crate::analisar_comando(sql).unwrap_err().to_string();
            assert!(
                e.contains("UNION") && e.contains("substrato"),
                "{sql} -> {e}"
            );
        }
    }
}

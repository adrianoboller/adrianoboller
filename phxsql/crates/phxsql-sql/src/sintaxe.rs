//! Analisador sintatico do `SELECT` simples.
//!
//! A gramatica inteira, e ela cabe aqui:
//!
//! ```text
//! SELECT  ( * | COUNT(*) | coluna [AS apelido] {, coluna [AS apelido]} )
//! FROM    [database.] [schema.] tabela [[AS] apelido]
//! [WHERE  coluna comparador literal | expressao]
//! [ORDER BY coluna [ASC|DESC]]
//! [LIMIT  n [OFFSET m]]
//! ```
//!
//! Nao ha `JOIN`, nao ha subconsulta, nao ha `GROUP BY` geral. Isso nao e
//! economia de esforco: e o que `docs/SQL.md` mediu, e cada coisa que falta
//! sai daqui como recusa escrita, com o nome da clausula -- nunca como
//! sintaxe aceita que quebra depois.
//!
//! # O WHERE tem DUAS formas, e uma so escolhe indice
//!
//! `coluna op literal` sozinho (`Onde::Simples`) e a forma que desce um
//! indice: `traduzir` vira `buscar`, e recusa se o indice nao existir --
//! nunca varre calado. Qualquer outra coisa no WHERE -- `AND`/`OR`,
//! aritmetica, funcao, `IN`, `BETWEEN`, `LIKE`, `IS [NOT] NULL`, parenteses,
//! coluna contra coluna -- vira `Onde::Expressao`: o TEXTO normalizado dos
//! tokens, que `traduzir` poe no campo `"expressao"` de um `varrer` para o
//! motor avaliar linha por linha. Nenhuma das duas formas e "melhor" -- a
//! primeira e mais rapida quando ha indice, e so isso.

use crate::dml::{Atualizacao, Exclusao, Insercao};
use crate::lexico::{self, normalizar_tokens, Comparador, Simbolo, Token};
use phxsql_core::{PhxError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Guardado como texto pelo mesmo motivo do lexico: decimal nao passa por
    /// `f64` sem perder digito.
    Numero(String),
    Texto(String),
    Bool(bool),
    Nulo,
}

impl Literal {
    pub fn escrever(&self) -> String {
        match self {
            Literal::Numero(n) => n.clone(),
            Literal::Texto(t) => format!("'{}'", t.replace('\'', "''")),
            Literal::Bool(true) => "TRUE".into(),
            Literal::Bool(false) => "FALSE".into(),
            Literal::Nulo => "NULL".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColunaPedida {
    pub nome: String,
    pub apelido: Option<String>,
}

impl ColunaPedida {
    /// O nome com que a coluna sai na resposta.
    pub fn rotulo(&self) -> &str {
        self.apelido.as_deref().unwrap_or(&self.nome)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Projecao {
    /// `SELECT *`
    Tudo,
    /// `SELECT COUNT(*)` -- o unico agregado do caminho RAPIDO, e so porque
    /// o `varrer` ja responde a contagem em O(1), lendo dois campos do
    /// cabecalho. Um `COUNT(*)` que precisa de `agrupar` (por causa de um
    /// `WHERE` em forma de expressao) continua sendo ESTE variante -- quem
    /// decide qual dos dois caminhos usar e `traduzir`, nao a sintaxe.
    Contagem,
    Colunas(Vec<ColunaPedida>),
    /// `GROUP BY`, ou agregado fora dele (`SELECT SUM(x) FROM t`, sem
    /// `GROUP BY` nenhum -- vira `agrupar` com `por: []`). Cada item e uma
    /// coluna simples (que so entra aqui se estiver no `GROUP BY`) ou um
    /// agregado.
    Agregada(Vec<ItemProjetado>),
}

/// As funcoes de `agrupar.agregados.funcao` -- as mesmas seis do `pivotar`
/// (`crates/phxsql-server/src/pivot.rs::Agregador`), e os MESMOS nomes que
/// vao no JSON: essa camada nao inventa vocabulario novo, so fala a lingua
/// que o motor ja fala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncaoAgregada {
    Contagem,
    Soma,
    Media,
    Minimo,
    Maximo,
    /// `COUNT(DISTINCT coluna)`.
    Distintos,
}

impl FuncaoAgregada {
    pub(crate) fn de_nome_de_funcao(nome: &str) -> Option<FuncaoAgregada> {
        match nome {
            "COUNT" => Some(FuncaoAgregada::Contagem),
            "SUM" => Some(FuncaoAgregada::Soma),
            "AVG" => Some(FuncaoAgregada::Media),
            "MIN" => Some(FuncaoAgregada::Minimo),
            "MAX" => Some(FuncaoAgregada::Maximo),
            _ => None,
        }
    }

    /// O texto que vai no campo `"funcao"` do pedido.
    pub fn nome_no_protocolo(&self) -> &'static str {
        match self {
            FuncaoAgregada::Contagem => "contagem",
            FuncaoAgregada::Soma => "soma",
            FuncaoAgregada::Media => "media",
            FuncaoAgregada::Minimo => "minimo",
            FuncaoAgregada::Maximo => "maximo",
            FuncaoAgregada::Distintos => "distintos",
        }
    }
}

/// Um item da projecao quando ela pode ter agregado: coluna simples (tem de
/// estar no `GROUP BY`) ou uma chamada de agregado.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemProjetado {
    Coluna(ColunaPedida),
    Agregado {
        funcao: FuncaoAgregada,
        /// `None` so em `COUNT(*)` -- os outros cinco exigem coluna.
        coluna: Option<String>,
        apelido: Option<String>,
    },
}

/// O que a projecao leu ANTES de saber se ha `GROUP BY`/`HAVING` -- so dai
/// da para decidir a forma final (`finalizar_projecao`), porque `GROUP BY`
/// vem DEPOIS na frase.
enum ProjecaoBruta {
    Tudo,
    Itens(Vec<ItemProjetado>),
}

/// A recusa de `SELECT *` com `GROUP BY`, nas DUAS gramaticas.
///
/// Um texto so porque e a mesma falta: a de uma tabela (`finalizar_projecao`)
/// e a COMPOSTA (`consulta.rs`, pedido 394) recusam pelo mesmo motivo, e duas
/// redacoes fariam a mesma consulta explicar-se de dois jeitos conforme
/// houvesse junção.
pub(crate) const SEM_ESTRELA_COM_GROUP_BY: &str =
    "SELECT * nao combina com GROUP BY: cada coluna do resultado tem de ser uma das \
     colunas do agrupamento ou um agregado, e `*` nao diz qual -- nomeie as colunas";

/// Decide a forma final da projecao, agora que se sabe se ha `GROUP
/// BY`/`HAVING`. Preserva os DOIS caminhos rapidos de sempre -- `Tudo` e
/// `Contagem` -- quando nada os empurra para `Agregada`, para nao mudar o
/// plano de quem nunca usou agregado nenhum.
fn finalizar_projecao(
    bruta: ProjecaoBruta,
    agrupar_por: &[String],
    tem_having: bool,
) -> Result<Projecao> {
    match bruta {
        ProjecaoBruta::Tudo => {
            if !agrupar_por.is_empty() || tem_having {
                return Err(PhxError::Esquema(SEM_ESTRELA_COM_GROUP_BY.into()));
            }
            Ok(Projecao::Tudo)
        }
        ProjecaoBruta::Itens(itens) => {
            let tem_agregado = itens
                .iter()
                .any(|i| matches!(i, ItemProjetado::Agregado { .. }));

            // O caminho de sempre: so colunas, sem GROUP BY nem HAVING.
            if !tem_agregado && agrupar_por.is_empty() && !tem_having {
                return Ok(Projecao::Colunas(
                    itens
                        .into_iter()
                        .map(|i| match i {
                            ItemProjetado::Coluna(c) => c,
                            ItemProjetado::Agregado { .. } => unreachable!(),
                        })
                        .collect(),
                ));
            }

            // O caminho rapido de sempre: SO `COUNT(*)`, sem GROUP BY/HAVING
            // e sem mais nada na lista.
            if agrupar_por.is_empty() && !tem_having && itens.len() == 1 {
                if let ItemProjetado::Agregado {
                    funcao: FuncaoAgregada::Contagem,
                    coluna: None,
                    ..
                } = &itens[0]
                {
                    return Ok(Projecao::Contagem);
                }
            }

            // Dali para baixo e sempre `agrupar`: cada coluna simples tem de
            // estar no GROUP BY, ou a resposta teria um valor que nao e nem
            // a chave do grupo nem um agregado -- SQL nenhum garante QUAL
            // linha do grupo aquele valor viria.
            for item in &itens {
                if let ItemProjetado::Coluna(c) = item {
                    if !agrupar_por
                        .iter()
                        .any(|g| crate::traduzir::igual_sem_caso(g, &c.nome))
                    {
                        return Err(PhxError::Esquema(format!(
                            "a coluna {:?} aparece no SELECT mas nao esta no GROUP BY e nao \
                             e agregado -- cada coluna do resultado tem de ser uma das \
                             colunas do agrupamento ou uma chamada de agregado",
                            c.nome
                        )));
                    }
                }
            }
            Ok(Projecao::Agregada(itens))
        }
    }
}

/// O que uma parte de `UNION` pode ser, e a recusa NOMEADA do resto.
///
/// `Err` traz a mensagem crua; quem chama poe a posicao do texto.
///
/// # O que existe hoje, e por que a lista e tao curta
///
/// A op `unir` recebe uma lista de NOMES de tabela: ela abre cada tabela
/// INTEIRA (`Servidor::materializar`), confere que os esquemas empilham
/// (`juncao::conferir_uniao`) e empilha. Nao ha `onde`, nao ha projecao, nao
/// ha ordem -- entao `SELECT * FROM a UNION SELECT * FROM b` traduz 1:1, e
/// qualquer coisa alem disso nao tem para onde ir.
///
/// **Isto nao vai ficar assim, e por isso a recusa aponta onde a conta esta.**
/// O braco do `UNION` e um SELECT nos quatro motores -- nenhum deles tem
/// sequer sintaxe para unir dois nomes de tabela --, e o pedido 393 mede o que
/// custa: filtrar dentro do braco contra unir inteiras e filtrar fora. Quem
/// muda a forma de uma op do protocolo e o dono, nao esta camada.
fn conferir_braco_da_uniao(s: &Selecao, i: usize) -> Result<()> {
    let ordinal = i + 1;
    if s.ordem.is_some() || !s.ordem_lista.is_empty() || s.limite.is_some() || s.salto > 0 {
        return Err(PhxError::Esquema(format!(
            "ORDER BY/LIMIT/OFFSET num UNION nao tem substrato (parte {ordinal}): a op \
             `unir` nao tem campo de ordem, e o `max` dela e o teto de memoria do \
             servidor, nao um LIMIT -- usa-lo como limite faria a resposta dizer \
             `truncado` por um corte que quem perguntou pediu"
        )));
    }
    if !matches!(s.projecao, Projecao::Tudo) || s.onde.is_some() || !s.agrupar_por.is_empty() {
        return Err(PhxError::Esquema(format!(
            "UNION entre SELECTs com filtro ou projecao nao tem substrato (parte \
             {ordinal}): a op `unir` recebe TABELAS inteiras, e o pedido 393 mede o \
             que custa mudar isso. O que passa hoje e `SELECT * FROM a UNION [ALL] \
             SELECT * FROM b`"
        )));
    }
    Ok(())
}

/// Os nomes pelos quais o `ORDER BY` de um `DISTINCT` pode chamar uma coluna
/// projetada: o nome DELA e o apelido do `AS`, quando ha um.
///
/// Os dois valem porque os dois querem dizer a mesma coluna, e recusar um
/// deles seria inventar uma regra que nenhum dos quatro motores tem. Quem
/// traduz o apelido de volta para a coluna e `traduzir::plano_agrupar` -- o
/// `agrupar` conhece a COLUNA, nao o apelido.
fn nomes_aceitos_na_ordem(p: &Projecao) -> Vec<String> {
    match p {
        Projecao::Colunas(cs) => cs
            .iter()
            .flat_map(|c| {
                let mut v = vec![c.nome.clone()];
                if let Some(a) = &c.apelido {
                    v.push(a.clone());
                }
                v
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// O que um `SELECT DISTINCT` pode ser, e a recusa NOMEADA do que nao pode.
///
/// `Err` traz a mensagem crua; quem chama poe a posicao do texto.
///
/// # Por que `SELECT DISTINCT *` recusa, com o numero
///
/// O tradutor nao ve esquema nenhum -- ele recebe os INDICES da tabela, nao
/// as colunas --, entao nao tem como montar a lista de agrupamento que o `*`
/// significa. E mesmo que visse: medido, `SELECT * FROM clientes` devolve
/// `rowid, id, nome, cidade, softdeleted, rownum`, e `rowid`/`rownum` sao
/// unicos por linha. Agrupar por tudo o que o `*` mostra nunca tiraria uma
/// linha -- medido, 3 linhas viram 3 grupos --, e ainda pagaria a tabela de
/// grupos inteira em memoria. Entregar isso seria lento e calado; recusar
/// custa uma frase e o nome das colunas.
fn conferir_distinto(p: &Projecao, agrupar_por: &[String], tem_having: bool) -> Result<()> {
    if !agrupar_por.is_empty() || tem_having {
        return Err(PhxError::Esquema(
            "DISTINCT com GROUP BY/HAVING nao tem substrato: os dois viram a MESMA \
             operacao `agrupar`, e empilhar um no outro pediria um segundo \
             agrupamento sobre o resultado do primeiro. Um GROUP BY ja devolve \
             uma linha por grupo -- tire o DISTINCT"
                .into(),
        ));
    }
    match p {
        Projecao::Colunas(_) => Ok(()),
        Projecao::Tudo => Err(PhxError::Esquema(
            "SELECT DISTINCT * nao tem substrato: o `*` desta casa carrega as colunas \
             de sistema (`rowid`, `softdeleted`, `rownum`), e o `rownum` e unico por \
             linha -- agrupar por tudo o que o `*` mostra nao tiraria repetida \
             nenhuma, so pagaria a tabela de grupos. Nomeie as colunas"
                .into(),
        )),
        Projecao::Contagem | Projecao::Agregada(_) => Err(PhxError::Esquema(
            "DISTINCT com funcao agregada nao tem substrato: o agregado ja vira \
             `agrupar`, e o DISTINCT por cima pediria um segundo agrupamento. \
             COUNT(DISTINCT coluna) -- que e outra pergunta -- existe e passa"
                .into(),
        )),
    }
}

/// Para onde o `FROM` aponta, ja separado nas tres partes que o motor usa.
///
/// O enderecamento com `schema` ja funciona hoje em toda operacao do
/// protocolo: `tabela: "matriz.estoque"` abre a pasta certa. O que faltava era
/// alguem escrever `FROM matriz.estoque` e chegar la -- e e isto.
#[derive(Debug, Clone, PartialEq)]
pub struct Alvo {
    /// Vazio quando a consulta nao disse -- quem executa poe o banco corrente.
    pub database: String,
    /// Vazio quando a tabela mora na raiz do banco.
    pub schema: String,
    pub tabela: String,
    pub apelido: Option<String>,
}

impl Alvo {
    /// O que vai no campo `"tabela"` do pedido: `estoque` ou `matriz.estoque`.
    pub fn nome_no_protocolo(&self) -> String {
        if self.schema.is_empty() {
            self.tabela.clone()
        } else {
            format!("{}.{}", self.schema, self.tabela)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condicao {
    pub coluna: String,
    pub op: Comparador,
    pub valor: Literal,
}

/// O que o `WHERE` de um `SELECT` virou.
///
/// A forma antiga (`coluna op literal`) continua preferida porque ela desce
/// um indice -- `buscar` custa uma leitura de arvore, e uma varredura com
/// expressao examina pagina por pagina. Qualquer coisa que NAO seja essa
/// forma unica (E/OU, aritmetica, funcao, `IN`, `BETWEEN`, `LIKE`, `IS [NOT]
/// NULL`, parenteses, coluna contra coluna) vira texto: quem avalia essa
/// expressao e o motor, nao esta camada.
#[derive(Debug, Clone, PartialEq)]
pub enum Onde {
    Simples(Condicao),
    /// O texto normalizado dos tokens do `WHERE`, um espaco entre cada um.
    Expressao(String),
}

impl Onde {
    /// O texto -- para quem so quer UM jeito de ver o `WHERE`, como o
    /// `consultar` composto (item 4): la nao existe "onde" separado de
    /// "expressao", tudo vira texto, simples ou nao.
    pub fn texto(&self) -> String {
        match self {
            Onde::Simples(c) => format!("{} {} {}", c.coluna, c.op.simbolo(), c.valor.escrever()),
            Onde::Expressao(t) => t.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ordenacao {
    pub coluna: String,
    pub desc: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selecao {
    /// `SELECT DISTINCT` -- so o SELECT SIMPLES de nivel superior o liga
    /// (`Analisador::comando`). Ele vira `agrupar` com as colunas projetadas
    /// em `por`, porque agrupar por N colunas E eliminar o repetido delas.
    ///
    /// Quem constroi uma `Selecao` a mao (subconsulta do `FROM`, corpo de
    /// CTE, lado de um `EXISTS`) a deixa em `false`, e `Analisador::selecao`
    /// -- a porta por onde TODAS essas passam -- recusa a palavra nomeando.
    /// A capacidade se liga num lugar so, e por isso nao ha um segundo lugar
    /// onde esquece-la vire resposta errada calada.
    pub distinto: bool,
    pub projecao: Projecao,
    pub de: Alvo,
    pub onde: Option<Onde>,
    /// `GROUP BY` -- vazio quando nao ha, e vazio TAMBEM quando ha agregado
    /// sem `GROUP BY` (`por: []` no pedido).
    pub agrupar_por: Vec<String>,
    /// `HAVING`, ja normalizado a texto -- a mesma forma de `Onde::Expressao`,
    /// porque o `HAVING` sempre vira texto: nao ha "forma simples" dele.
    pub tendo: Option<String>,
    /// `ORDER BY` de UMA coluna, do caminho antigo -- exige indice, e por
    /// isso continua limitado a uma coluna so.
    pub ordem: Option<Ordenacao>,
    /// `ORDER BY` de `agrupar`/`consultar`: sobre um resultado JA computado
    /// em memoria, entao varias colunas nao pedem indice nenhum.
    pub ordem_lista: Vec<Ordenacao>,
    pub limite: Option<u64>,
    pub salto: u64,
}

/// `SELECT … UNION [ALL] SELECT …` -- duas ou mais partes empilhadas.
///
/// # Por que as partes sao `Selecao` e nao `Alvo`
///
/// Porque e a `Selecao` que sabe dizer o que a parte pediu ALEM da tabela --
/// e e disso que sai a recusa NOMEADA. A op `unir` de hoje recebe uma lista
/// de NOMES de tabela (`servidor.rs::op_unir`): ela abre cada tabela inteira,
/// confere os esquemas e empilha. Nao ha `onde`, nao ha projecao, nao ha
/// ordem. Guardar so o `Alvo` jogaria fora o `WHERE` que alguem escreveu, e a
/// recusa nao teria como dizer o que foi ignorado.
#[derive(Debug, Clone, PartialEq)]
pub struct Uniao {
    pub partes: Vec<Selecao>,
    /// `true` e `UNION ALL`. E UM para a lista inteira porque o `modo` do
    /// pedido `unir` e um so -- misturar as duas formas recusa nomeando.
    pub tudo: bool,
}

/// O que a op `sql` recebe como instrucao de DADO: uma consulta, ou um dos
/// tres verbos de escrita por chave (`dml.rs`). Transacao, diretiva, rotina
/// e usuario nao passam por aqui -- sao comandos de sessao ou de catalogo, e
/// cada um tem o proprio detector, consultado antes.
#[derive(Debug, Clone, PartialEq)]
pub enum Comando {
    Selecao(Selecao),
    /// `SELECT … UNION [ALL] SELECT …`. Vira a op `unir`, que o protocolo ja
    /// tem -- ela e a porta dos sete cartoes de Venn da tela.
    Uniao(Uniao),
    /// `WITH`, subconsulta no `FROM`, `IN (SELECT …)`, junção ou janela --
    /// qualquer `SELECT` que precisa de COMPOSICAO. Vira a op `consultar`
    /// (`crate::consulta`), nunca `buscar`/`varrer`/`agrupar` sozinhos.
    Consulta(Box<crate::consulta::Consulta>),
    Insercao(Insercao),
    Atualizacao(Atualizacao),
    Exclusao(Exclusao),
    /// `CREATE VIEW nome AS SELECT ...` (item 6). `sql` e o TEXTO do
    /// SELECT, verbatim -- ja analisado uma vez (para recusar cedo), mas
    /// guardado como texto porque a visao e reanalisada a CADA uso: se a
    /// tabela dela sumir, quem descobre e quem tenta usar a visao, nao
    /// quem a criou.
    CriarVisao {
        nome: String,
        sql: String,
    },
    /// `DROP VIEW nome`.
    ExcluirVisao {
        nome: String,
    },
}

impl Comando {
    /// O verbo, para as mensagens.
    pub fn verbo(&self) -> &'static str {
        match self {
            Comando::Selecao(_) | Comando::Consulta(_) | Comando::Uniao(_) => "SELECT",
            Comando::Insercao(_) => "INSERT",
            Comando::Atualizacao(_) => "UPDATE",
            Comando::Exclusao(_) => "DELETE",
            Comando::CriarVisao { .. } => "CREATE VIEW",
            Comando::ExcluirVisao { .. } => "DROP VIEW",
        }
    }

    /// A tabela alvo, para quem precisa do esquema antes de traduzir.
    ///
    /// Numa consulta COMPOSTA isto e so a tabela PRINCIPAL (`de`) -- ela
    /// toca outras (`juntar`, `escalar`, `em`), e quem precisa dos indices
    /// de cada uma chama `crate::consulta::traduzir_consulta` com o
    /// resolvedor, nao este metodo.
    ///
    /// # Por que nao existe para visao
    ///
    /// `criar_visao`/`excluir_visao` nao leem esquema de tabela nenhuma
    /// antes de traduzir -- a visao guarda TEXTO, e so e analisada de novo
    /// no uso. Quem despacha por `Comando` trata esses dois casos ANTES de
    /// chegar aqui, exatamente como ja trata `Insercao`/`Atualizacao`/
    /// `Exclusao` num caminho proprio.
    pub fn alvo(&self) -> &Alvo {
        match self {
            Comando::Selecao(s) => &s.de,
            Comando::Consulta(c) => &c.de.de,
            // A PRIMEIRA parte, como os nomes de coluna do resultado. Quem
            // precisa das outras e `traduzir_uniao`, que le a lista inteira
            // -- e e por isso que a permissao da uniao nao pode sair daqui:
            // ver `op_unir`, que confere tabela por tabela.
            Comando::Uniao(u) => &u.partes[0].de,
            Comando::Insercao(i) => &i.em,
            Comando::Atualizacao(a) => &a.em,
            Comando::Exclusao(e) => &e.de,
            Comando::CriarVisao { .. } | Comando::ExcluirVisao { .. } => {
                unreachable!(
                    "CriarVisao/ExcluirVisao nao tem tabela alvo -- quem despacha por \
                     Comando trata os dois ANTES de chamar alvo()"
                )
            }
        }
    }
}

/// As palavras que ja tem significado no motor e nao podem virar identificador.
///
/// `docs/SQL.md` explica por que a lista mora AQUI e nao no `validar_nome` do
/// catalogo: reservar palavra no motor quebraria banco de quem ja tem a
/// tabela. O parser e novo, entao a reserva so custa a quem escrever SQL.
pub const RESERVADAS_DO_MOTOR: [&str; 1] = ["BULKINSERT"];

/// Palavras da gramatica que nao podem ser lidas como nome de tabela ou de
/// coluna sem aspas.
const CLAUSULAS: [&str; 26] = [
    "SELECT", "FROM", "WHERE", "ORDER", "GROUP", "BY", "LIMIT", "OFFSET", "AS", "HAVING", "JOIN",
    "UNION",
    // Os verbos de escrita e as clausulas deles. SET e VALUES precisam estar
    // aqui por um motivo concreto: o `alvo` aceita apelido SEM `AS`, e um
    // `UPDATE t SET ...` leria SET como apelido da tabela.
    "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
    // A cadeia de junção (item 8): `alvo()` aceita apelido SEM `AS`, e sem
    // estas aqui `FROM p LEFT JOIN c` leria "LEFT" como apelido de `p`, e
    // `FROM p JOIN c ON ...` leria "ON" como apelido de `c` -- os dois
    // calados, sem erro nenhum, so a junção quebrando silenciosamente.
    "ON", "INNER", "LEFT", "RIGHT", "FULL", "CROSS", "OUTER", "WITH",
];

/// O texto traz MAIS DE UM comando empilhado -- o `; DROP TABLE ...` classico?
///
/// # Por que ela ANALISA em vez de recortar
///
/// Procurar `";"` no texto acusaria `WHERE nome = '; DROP TABLE clientes'`,
/// que e um DADO legitimo e ja foi gravado assim nesta casa (a bateria
/// `bancada/seguranca/injecao.py` grava exatamente esse valor e o le de
/// volta). Quem decide aqui e o LEXICO do motor -- o mesmo que a consulta
/// usa --, entao o que esta dentro de aspas e um simbolo `Texto` e nunca um
/// separador de comando. E a mesma lei do Profiler: *o que mostra texto cru
/// analisa, nunca recorta*.
///
/// # O que ela responde `false`, e nao e engano
///
/// * texto que o lexico nao consegue ler (aspa aberta, byte estranho): sem
///   simbolos nao ha o que classificar, e chutar aqui seria acusar quem
///   digitou errado. Erro de sintaxe **nao** e injecao;
/// * `SELECT ... ;` com o ponto-e-virgula no fim, que e como todo cliente
///   ODBC manda;
/// * `-- comentario` depois do comando, que o lexico ja descarta.
///
/// Ou seja: so responde `true` quando ha simbolo DEPOIS de um
/// ponto-e-virgula -- que e um segundo comando, e nada mais e.
pub fn comando_empilhado(entrada: &str) -> bool {
    let Ok(simbolos) = lexico::analisar(entrada) else {
        return false;
    };
    if simbolos.is_empty() {
        return false;
    }
    let mut p = Analisador { s: simbolos, i: 0 };
    // Nem chegou a ser um comando desta gramatica -- `CREATE PROCEDURE`,
    // `CALL`, `BEGIN`, ou sintaxe torta. Sai `false`, e a saida e a que mais
    // importa: o CORPO de um procedimento e de um gatilho e cheio de
    // ponto-e-virgula legitimo, e classificar por simbolo solto acusaria todo
    // `CREATE PROCEDURE ... BEGIN a; b; END` que falhasse por qualquer outro
    // motivo.
    if p.comando(entrada).is_err() {
        return false;
    }
    while p.aceitar(&Token::PontoEVirgula) {}
    p.espiar().is_some()
}

/// Le um comando inteiro. Um por vez -- lote de comandos e outra rodada.
/// Le uma CONSULTA. Escrita por aqui recusa dizendo que e escrita: a porta
/// dos tres verbos e [`analisar_comando`], que a op `sql` usa.
pub fn analisar(entrada: &str) -> Result<Selecao> {
    match analisar_comando(entrada)? {
        Comando::Selecao(s) => Ok(s),
        Comando::Consulta(_) => Err(PhxError::Esquema(
            "esta consulta usa composicao (WITH, subconsulta, IN (SELECT ...), junção ou \
             janela) e traduz para `consultar`, nao para o Selecao simples desta porta -- \
             use `analisar_comando` e trate `Comando::Consulta`"
                .into(),
        )),
        outro => Err(PhxError::Esquema(format!(
            "{} e comando de ESCRITA, e esta porta le consultas -- a op `sql` aceita os \
             dois, por `analisar_comando`",
            outro.verbo()
        ))),
    }
}

/// Le qualquer instrucao de dado: `SELECT`, ou `INSERT`/`UPDATE`/`DELETE`
/// por chave. Sem parametro nenhum -- `analisar_comando_com(entrada, &[])`.
pub fn analisar_comando(entrada: &str) -> Result<Comando> {
    analisar_comando_com(entrada, &[])
}

/// A mesma leitura, com `?` resolvido contra `parametros` ANTES da sintaxe
/// rodar -- ver `lexico::resolver_parametros` para o porque disso ser troca
/// de TOKEN e nunca de texto. E a porta que a op `sql` usa quando o pedido
/// traz `"parametros"`.
pub fn analisar_comando_com(
    entrada: &str,
    parametros: &[phxsql_core::json::Json],
) -> Result<Comando> {
    let simbolos = lexico::analisar(entrada)?;
    if simbolos.is_empty() {
        return Err(PhxError::Esquema("comando SQL vazio".into()));
    }
    let simbolos = lexico::resolver_parametros(simbolos, parametros)?;
    let mut p = Analisador { s: simbolos, i: 0 };
    let cmd = p.comando(entrada)?;
    p.aceitar(&Token::PontoEVirgula);
    if let Some(sobra) = p.espiar() {
        return Err(lexico::erro(
            sobra.posicao,
            &format!(
                "sobrou {:?} depois do fim do comando; um comando por vez",
                sobra.token.descrever()
            ),
        ));
    }
    Ok(cmd)
}

/// A saida real de quem quis `ORDER BY SUM(v)`, medida: `ORDER BY s` com
/// `SUM(v) AS s` na projecao TRADUZ hoje. Fica numa constante porque as duas
/// portas do `ORDER BY` (a restrita, de uma coluna so, e a lista de
/// `agrupar`/`consultar`) tem de dar a MESMA saida -- duas redacoes da mesma
/// recusa e o comeco de uma divergir da outra.
const ORDENE_PELO_APELIDO: &str =
    "De um apelido ao agregado na projecao (SUM(x) AS a) e ordene por ele.";

/// O cursor de simbolos e compartilhado com `rotina`, que analisa os corpos
/// de gatilho e de procedimento: um leitor so, para as mensagens de erro (com
/// a coluna) sairem iguais nas duas gramaticas.
pub(crate) struct Analisador {
    pub(crate) s: Vec<Simbolo>,
    pub(crate) i: usize,
}

impl Analisador {
    pub(crate) fn espiar(&self) -> Option<&Simbolo> {
        self.s.get(self.i)
    }

    pub(crate) fn posicao_atual(&self) -> usize {
        match self.s.get(self.i) {
            Some(s) => s.posicao,
            // Fim do texto: aponta logo depois do ultimo simbolo lido.
            None => self.s.last().map(|s| s.posicao + 1).unwrap_or(0),
        }
    }

    /// Consome o proximo simbolo se ele for exatamente este.
    pub(crate) fn aceitar(&mut self, t: &Token) -> bool {
        if self.espiar().map(|s| &s.token) == Some(t) {
            self.i += 1;
            return true;
        }
        false
    }

    /// Consome a proxima palavra se ela for esta clausula.
    pub(crate) fn aceitar_palavra(&mut self, palavra: &str) -> bool {
        let bate = self
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .is_some_and(|p| p == palavra);
        if bate {
            self.i += 1;
        }
        bate
    }

    /// A proxima palavra e esta, SEM consumir.
    pub(crate) fn espiar_palavra(&self, palavra: &str) -> bool {
        self.espiar()
            .and_then(|s| s.token.palavra_chave())
            .is_some_and(|p| p == palavra)
    }

    pub(crate) fn exigir_palavra(&mut self, palavra: &str) -> Result<()> {
        if self.aceitar_palavra(palavra) {
            return Ok(());
        }
        Err(lexico::erro(
            self.posicao_atual(),
            &format!("esperava {palavra}{}", self.mas_veio()),
        ))
    }

    pub(crate) fn mas_veio(&self) -> String {
        match self.espiar() {
            Some(s) => format!(", e veio {:?}", s.token.descrever()),
            None => ", e o comando acabou".into(),
        }
    }

    /// Le um identificador: palavra que nao seja clausula, ou citada.
    pub(crate) fn identificador(&mut self, papel: &str) -> Result<String> {
        let Some(s) = self.espiar() else {
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava {papel}, e o comando acabou"),
            ));
        };
        let pos = s.posicao;
        match &s.token {
            Token::Palavra { texto, citado } => {
                let nome = texto.clone();
                if !citado {
                    let alto = nome.to_uppercase();
                    if CLAUSULAS.contains(&alto.as_str()) {
                        return Err(lexico::erro(
                            pos,
                            &format!(
                                "{alto} e palavra da linguagem e nao serve de {papel}; \
                                 entre aspas duplas ela vira nome"
                            ),
                        ));
                    }
                    if RESERVADAS_DO_MOTOR.contains(&alto.as_str()) {
                        return Err(lexico::erro(
                            pos,
                            &format!(
                                "{alto} e comando do PhxSql, e nao pode ser {papel}. \
                                 Ele reserva a tabela para carga e vive na CONEXAO, \
                                 nao na instrucao"
                            ),
                        ));
                    }
                }
                self.i += 1;
                Ok(nome)
            }
            outro => Err(lexico::erro(
                pos,
                &format!("esperava {papel}, e veio {:?}", outro.descrever()),
            )),
        }
    }

    fn comando(&mut self, texto: &str) -> Result<Comando> {
        let Some(primeiro) = self.espiar() else {
            return Err(PhxError::Esquema("comando SQL vazio".into()));
        };
        let pos = primeiro.posicao;
        let verbo = primeiro.token.palavra_chave().unwrap_or_default();
        match verbo.as_str() {
            "SELECT" => {
                self.i += 1;
                if self.precisa_de_consulta_composta() {
                    // A sondagem olha os tokens sem consumir, entao um
                    // DISTINCT na frente nao a atrapalha -- mas se ela disse
                    // que a consulta e COMPOSTA, o DISTINCT nao tem onde
                    // acontecer, e a recusa nomeia isso aqui em vez de virar
                    // um "esperava coluna" no meio da projecao composta.
                    if self.espiar_palavra("DISTINCT") {
                        return Err(lexico::erro(
                            self.posicao_atual(),
                            "DISTINCT com junção, subconsulta, CTE ou janela nao tem \
                             substrato: essas formas viram a op `consultar`, que compoe \
                             mas nao elimina repetido. Num SELECT de UMA tabela o \
                             DISTINCT passa",
                        ));
                    }
                    // Idem para o UNION: se o comando vai pela gramatica
                    // composta, a uniao nao tem onde acontecer. Sem esta
                    // recusa, o comando seguiria e morreria la na frente com
                    // "sobrou UNION depois do fim do comando" -- verdade que
                    // nao ensina nada.
                    if self.ha_union_no_nivel_externo() {
                        return Err(lexico::erro(
                            self.posicao_atual(),
                            "UNION com junção, subconsulta, CTE ou janela nao tem \
                             substrato: a op `unir` empilha TABELAS inteiras, e essas \
                             formas viram a op `consultar`, que nao empilha. A uniao \
                             que passa hoje e `SELECT * FROM a UNION [ALL] SELECT * \
                             FROM b`",
                        ));
                    }
                    Ok(Comando::Consulta(Box::new(
                        self.consulta_apos_select(None)?,
                    )))
                } else {
                    // O UNICO lugar que liga o `distinto`. Ver `Selecao`.
                    let primeira = if self.aceitar_palavra("DISTINCT") {
                        self.selecao_com(true)?
                    } else {
                        self.selecao()?
                    };
                    // O `UNION` sobra do `selecao` porque nao e clausula dela
                    // -- e a junta entre dois SELECT inteiros.
                    if self.espiar_palavra("UNION") {
                        self.uniao_apos(primeira)
                    } else {
                        Ok(Comando::Selecao(primeira))
                    }
                }
            }
            // WITH x AS (SELECT ...) SELECT ... -- so uma CTE, nao
            // recursiva (item 4). Sempre vira `consultar`.
            "WITH" => {
                self.i += 1;
                Ok(Comando::Consulta(Box::new(self.com_cte()?)))
            }
            // CREATE VIEW/DROP VIEW (item 6). So VIEW chega aqui -- as
            // outras formas de CREATE/DROP (TRIGGER, PROCEDURE, TABLE, ...)
            // sao interceptadas antes, por `rotina::comando`.
            "CREATE" => {
                self.i += 1;
                self.exigir_palavra("VIEW")?;
                self.criar_visao(texto)
            }
            "DROP" => {
                self.i += 1;
                self.exigir_palavra("VIEW")?;
                self.excluir_visao()
            }
            // O passo 2 do roteiro de `docs/SQL.md`: escrita por chave. O
            // parser mora em `dml.rs`, sobre este mesmo cursor.
            "INSERT" => {
                self.i += 1;
                Ok(Comando::Insercao(self.insercao(pos)?))
            }
            "UPDATE" => {
                self.i += 1;
                Ok(Comando::Atualizacao(self.atualizacao(pos)?))
            }
            "DELETE" => {
                self.i += 1;
                Ok(Comando::Exclusao(self.exclusao(pos)?))
            }
            "BULKINSERT" => Err(lexico::erro(
                pos,
                "BULKINSERT e comando de SESSAO, e nao de instrucao: ele reserva a \
                 tabela para carga e a reserva morre com a conexao. Hoje se pede pela \
                 porta de dados, com a operacao bulkinsert",
            )),
            // Chegar AQUI quer dizer que o `transacao::comando` nao
            // reconheceu a forma -- ele e consultado antes, no `op_sql`. Entao
            // a recusa nao e mais "nao ha transacao": e "esta forma nao".
            "BEGIN" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" | "START" => {
                Err(lexico::erro(
                    pos,
                    &format!(
                        "{verbo} e comando de SESSAO e nao de consulta, e esta \
                         forma dele nao foi reconhecida. As aceitas: BEGIN, \
                         BEGIN TRANSACTION, START TRANSACTION, COMMIT, ROLLBACK, \
                         SAVEPOINT <nome>, ROLLBACK TO SAVEPOINT <nome> e \
                         RELEASE SAVEPOINT <nome>"
                    ),
                ))
            }
            "" => Err(lexico::erro(pos, "o comando nao comeca por um verbo")),
            outro => Err(lexico::erro(
                pos,
                &format!("{outro} nao e um comando desta camada"),
            )),
        }
    }

    /// Depois de `CREATE VIEW` ja consumidos.
    ///
    /// O `sql` guardado e o TEXTO original (por posicao de CARACTERE, como
    /// `rotina::texto_a_partir` ja faz para corpo de gatilho/procedimento)
    /// -- nunca reconstruido dos tokens, que perderia espaco, caixa e
    /// comentario exatamente como o autor escreveu.
    fn criar_visao(&mut self, texto: &str) -> Result<Comando> {
        let nome = self.identificador("nome da visao")?;
        self.exigir_palavra("AS")?;
        if self
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .as_deref()
            != Some("SELECT")
        {
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava SELECT depois de AS na visao{}", self.mas_veio()),
            ));
        }
        let Some(s) = self.espiar() else {
            unreachable!("acabou de conferir que ha um SELECT aqui");
        };
        let sql: String = texto.chars().skip(s.posicao).collect();

        // Analisa AGORA, para recusar cedo -- e o mesmo texto que sera
        // reanalisado a CADA uso da visao, entao um SELECT que esta camada
        // nao entende hoje tambem nao vai entender no primeiro uso; melhor
        // a mensagem chegar na hora do CREATE, com a visao no topo da
        // cabeca de quem escreveu, do que num SELECT * FROM v_x qualquer
        // dali a um mes.
        match analisar_comando(&sql)? {
            Comando::Selecao(_) | Comando::Consulta(_) => {}
            outro => {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!(
                        "o corpo de uma visao tem de ser um SELECT, e {} nao e",
                        outro.verbo()
                    ),
                ))
            }
        }

        // O resto do texto ja virou o `sql` da visao -- nao ha mais nada
        // para o cursor principal ler. Sem isto, `analisar_comando_com`
        // acusaria "sobrou SELECT ... depois do fim do comando", porque os
        // tokens do SELECT continuam na lista, so que nao foram consumidos
        // um por um.
        self.i = self.s.len();

        Ok(Comando::CriarVisao { nome, sql })
    }

    /// Depois de `DROP VIEW` ja consumidos.
    fn excluir_visao(&mut self) -> Result<Comando> {
        let nome = self.identificador("nome da visao")?;
        Ok(Comando::ExcluirVisao { nome })
    }

    /// Ha um `UNION` fora de parenteses daqui ate o fim do comando?
    ///
    /// Sem consumir nada, como `precisa_de_consulta_composta` -- e pela mesma
    /// razao: a decisao de qual gramatica usar e tomada antes de ler.
    pub(crate) fn ha_union_no_nivel_externo(&self) -> bool {
        let mut profundidade = 0i32;
        for s in &self.s[self.i..] {
            match &s.token {
                Token::AbreParen => profundidade += 1,
                Token::FechaParen => profundidade -= 1,
                Token::PontoEVirgula if profundidade == 0 => break,
                _ if profundidade == 0 => {
                    if s.token.palavra_chave().as_deref() == Some("UNION") {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Depois da primeira parte, com o cursor no `UNION`.
    fn uniao_apos(&mut self, primeira: Selecao) -> Result<Comando> {
        let mut partes = vec![primeira];
        let mut tudo: Option<bool> = None;
        while self.aceitar_palavra("UNION") {
            let pos = self.posicao_atual();
            let este = self.aceitar_palavra("ALL");
            match tudo {
                None => tudo = Some(este),
                Some(anterior) if anterior != este => {
                    // O pedido `unir` tem UM `modo` para a lista inteira.
                    // Aceitar a mistura obrigaria a escolher um dos dois
                    // calado, e o que se perderia e justamente a linha
                    // repetida -- o resultado errado com cara de certo.
                    return Err(lexico::erro(
                        pos,
                        "misturar UNION e UNION ALL no mesmo comando nao tem substrato: \
                         a op `unir` tem UM modo para a lista toda. Escreva os dois \
                         comandos separados, ou use o mesmo modo nas duas juntas",
                    ));
                }
                _ => {}
            }
            self.exigir_palavra("SELECT")?;
            if self.espiar_palavra("DISTINCT") {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "DISTINCT dentro de um UNION nao tem substrato: o `unir` empilha \
                     tabelas inteiras, e o proprio UNION (sem ALL) ja elimina repetido",
                ));
            }
            partes.push(self.selecao()?);
        }
        // Sem posicao no texto, como `finalizar_projecao` e `conferir_distinto`:
        // o defeito nao esta num TOKEN, esta na forma da parte inteira.
        for (i, p) in partes.iter().enumerate() {
            conferir_braco_da_uniao(p, i)?;
        }
        Ok(Comando::Uniao(Uniao {
            partes,
            tudo: tudo.unwrap_or(false),
        }))
    }

    /// Um `SELECT` simples que NAO pode ser `DISTINCT`.
    ///
    /// Esta e a porta por onde passam TODAS as selecoes de dentro de uma
    /// consulta composta -- corpo de CTE, subconsulta do `FROM`, lado de um
    /// `IN`/escalar/`EXISTS`. Nenhuma delas tem por onde eliminar repetido: o
    /// `traduzir_consulta` usa o PEDIDO de cada pedaco e descarta a `saida`
    /// dele, entao a coluna de contagem que o `agrupar` injeta vazaria para o
    /// resultado de fora. O nivel superior chama `selecao_com(true)` direto.
    pub(crate) fn selecao(&mut self) -> Result<Selecao> {
        if self.espiar_palavra("DISTINCT") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "DISTINCT so existe no SELECT SIMPLES de nivel superior, onde vira \
                 `agrupar` pelas colunas projetadas. Aqui dentro (CTE, subconsulta do \
                 FROM, IN/EXISTS/escalar, junção ou janela) ele ainda nao tem \
                 substrato: o `consultar` compoe, mas nao elimina repetido",
            ));
        }
        self.selecao_com(false)
    }

    fn selecao_com(&mut self, distinto: bool) -> Result<Selecao> {
        let bruta = self.projecao()?;
        self.exigir_palavra("FROM")?;
        let de = self.alvo()?;

        if self.aceitar_palavra("JOIN")
            || self.aceitar_palavra("INNER")
            || self.aceitar_palavra("LEFT")
            || self.aceitar_palavra("RIGHT")
        {
            return Err(lexico::erro(
                self.posicao_atual(),
                "junção ainda nao passa por aqui. O motor ja junta -- e a operacao \
                 juntar, com sete formas -- mas a traducao do JOIN e outra rodada",
            ));
        }

        let onde = if self.aceitar_palavra("WHERE") {
            Some(self.onde_da_selecao()?)
        } else {
            None
        };

        let agrupar_por = if self.aceitar_palavra("GROUP") {
            self.exigir_palavra("BY")?;
            // `false`: na gramatica de uma tabela o qualificador se descarta.
            self.lista_de_colunas_do_group_by("GROUP BY", false)?
        } else {
            Vec::new()
        };

        let tendo = if self.aceitar_palavra("HAVING") {
            let tokens = self.capturar_ate_clausula(&["ORDER", "LIMIT", "OFFSET", "UNION"])?;
            if tokens.is_empty() {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "esperava uma condicao depois de HAVING",
                ));
            }
            Some(normalizar_tokens(&tokens))
        } else {
            None
        };

        let projecao = finalizar_projecao(bruta, &agrupar_por, tendo.is_some())?;
        if distinto {
            // Sem posicao no texto, como `finalizar_projecao`: o defeito nao
            // esta num TOKEN, esta na forma inteira do SELECT.
            conferir_distinto(&projecao, &agrupar_por, tendo.is_some())?;
        }
        let e_agrupada = matches!(projecao, Projecao::Agregada(_));

        let mut ordem = None;
        let mut ordem_lista = Vec::new();
        if self.aceitar_palavra("ORDER") {
            self.exigir_palavra("BY")?;
            // O `DISTINCT` cai no MESMO `agrupar` da projecao agregada, e o
            // resultado dele ja esta em memoria: ordenar por varias colunas
            // ali nao pede indice nenhum, ao contrario da varredura.
            if e_agrupada || distinto {
                ordem_lista = self.lista_de_ordenacoes()?;
            } else {
                ordem = Some(self.uma_ordenacao_restrita()?);
            }
        }
        if distinto {
            // A regra dos quatro motores: num `SELECT DISTINCT`, o `ORDER BY`
            // so alcanca coluna que esta na lista projetada. Ordenar por uma
            // coluna de FORA dela nao tem resposta unica -- o grupo tem varios
            // valores dela --, e os quatro recusam. Recusar aqui nomeia a
            // coluna e a lista; deixar passar cairia na recusa generica do
            // `agrupar`, que fala de `"por"` e de apelido de agregado, um
            // vocabulario que quem escreveu SQL nao tem como reconhecer.
            let rotulos = nomes_aceitos_na_ordem(&projecao);
            for o in &ordem_lista {
                if !rotulos
                    .iter()
                    .any(|r| crate::traduzir::igual_sem_caso(r, &o.coluna))
                {
                    return Err(lexico::erro(
                        self.posicao_atual(),
                        &format!(
                            "o ORDER BY de um SELECT DISTINCT so alcanca coluna da \
                             lista projetada, e {:?} nao esta la. A lista e: {}",
                            o.coluna,
                            rotulos.join(", ")
                        ),
                    ));
                }
            }
        }

        let mut limite = None;
        let mut salto = 0u64;
        if self.aceitar_palavra("LIMIT") {
            limite = Some(self.inteiro("o limite do LIMIT")?);
            if self.aceitar_palavra("OFFSET") {
                salto = self.inteiro("o salto do OFFSET")?;
            }
        } else if self.aceitar_palavra("OFFSET") {
            // OFFSET sem LIMIT e legal em SQL, e o `varrer` sabe pular.
            salto = self.inteiro("o salto do OFFSET")?;
        }

        Ok(Selecao {
            distinto,
            projecao,
            de,
            onde,
            agrupar_por,
            tendo,
            ordem,
            ordem_lista,
            limite,
            salto,
        })
    }

    /// Recusa `FUNCAO(` onde a gramatica espera NOME DE COLUNA.
    ///
    /// Existe porque `identificador()` NAO distingue nome de funcao de nome
    /// de coluna -- os dois chegam como `Token::Palavra`. Sem este crivo a
    /// palavra `SUM` de `ORDER BY SUM(v)` vira uma coluna FANTASMA chamada
    /// "SUM": a lista fecha ali, o `(` sobra, e o erro sai longe do problema
    /// falando de "sobrou" em vez de falar de agregado (pedido 394).
    ///
    /// Os tres lugares que leem lista de nome de coluna chamam este crivo --
    /// `uma_ordenacao_restrita`, `lista_de_ordenacoes` e
    /// `lista_de_colunas_do_group_by` (que serve ao `GROUP BY` e ao
    /// `PARTITION BY`) --, porque os tres chamam o MESMO `identificador()` na
    /// mesma posicao e herdariam a mesma coluna fantasma.
    pub(crate) fn recusar_funcao_no_lugar_de_coluna(
        &self,
        clausula: &str,
        saida: &str,
    ) -> Result<()> {
        let Some(f) = self.espiar().and_then(|s| s.token.palavra_chave()) else {
            return Ok(());
        };
        if self.s.get(self.i + 1).map(|s| &s.token) != Some(&Token::AbreParen) {
            return Ok(());
        }
        Err(lexico::erro(
            self.posicao_atual(),
            &format!(
                "{clausula} nao aceita a chamada {f}(...): aqui a gramatica le NOME DE \
                 COLUNA. {saida} Pedido 394"
            ),
        ))
    }

    /// Uma ordenacao SO, do caminho antigo -- ela exige indice, e por isso
    /// uma segunda coluna recusa (nao ha indice composto escolhido aqui).
    fn uma_ordenacao_restrita(&mut self) -> Result<Ordenacao> {
        self.recusar_funcao_no_lugar_de_coluna("ORDER BY", ORDENE_PELO_APELIDO)?;
        let coluna = self.identificador("nome de coluna")?;
        let desc = if self.aceitar_palavra("DESC") {
            true
        } else {
            self.aceitar_palavra("ASC");
            false
        };
        if self.aceitar(&Token::Virgula) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "ORDER BY de mais de uma coluna precisa de um indice composto com \
                 essas colunas nessa ordem -- e quem escolhe o indice ainda e quem \
                 chama, porque nao ha planejador",
            ));
        }
        Ok(Ordenacao { coluna, desc })
    }

    /// A lista de `ORDER BY` de `agrupar`/`consultar`: sobre um resultado JA
    /// computado em memoria, entao varias colunas nao pedem indice nenhum --
    /// e por isso NAO tem a restricao de uma coluna so da forma antiga.
    ///
    /// Aceita coluna QUALIFICADA (`p.id`, pedido 236) -- antes so lia o
    /// primeiro pedaco e deixava o ponto sobrando (o comando inteiro
    /// estourava mais na frente, com "esperava FROM" em vez de dizer que o
    /// problema era no `ORDER BY`). Quem resolve se o qualificador existe e
    /// se e ambiguo e o `consultar`, do mesmo jeito que ja resolve a
    /// projecao -- aqui e so leitura.
    pub(crate) fn lista_de_ordenacoes(&mut self) -> Result<Vec<Ordenacao>> {
        let mut ordens = Vec::new();
        loop {
            self.recusar_funcao_no_lugar_de_coluna("ORDER BY", ORDENE_PELO_APELIDO)?;
            let coluna = self.identificador("nome de coluna no ORDER BY")?;
            let coluna = if self.aceitar(&Token::Ponto) {
                format!(
                    "{coluna}.{}",
                    self.identificador("nome de coluna no ORDER BY depois do ponto")?
                )
            } else {
                coluna
            };
            let desc = if self.aceitar_palavra("DESC") {
                true
            } else {
                self.aceitar_palavra("ASC");
                false
            };
            ordens.push(Ordenacao { coluna, desc });
            if !self.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok(ordens)
    }

    pub(crate) fn lista_de_colunas_do_group_by(
        &mut self,
        clausula: &str,
        qualificada: bool,
    ) -> Result<Vec<String>> {
        let mut colunas = Vec::new();
        loop {
            self.recusar_funcao_no_lugar_de_coluna(
                clausula,
                "Agrupe pelas COLUNAS e deixe a chamada para a projecao.",
            )?;
            colunas
                .push(self.nome_de_coluna(qualificada, &format!("nome de coluna no {clausula}"))?);
            if !self.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok(colunas)
    }

    /// Le a projecao CRUA: `*`, ou uma lista de colunas e/ou chamadas de
    /// agregado. A decisao de qual `Projecao` isso vira fica para
    /// `finalizar_projecao`, chamada DEPOIS do `GROUP BY`/`HAVING` -- so
    /// entao da para saber se `COUNT(*)` sozinho e o caminho rapido ou se
    /// uma coluna simples precisa estar no `GROUP BY`.
    fn projecao(&mut self) -> Result<ProjecaoBruta> {
        if self.aceitar(&Token::Asterisco) {
            return Ok(ProjecaoBruta::Tudo);
        }
        let mut itens = Vec::new();
        loop {
            itens.push(self.item_de_projecao()?);
            if !self.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok(ProjecaoBruta::Itens(itens))
    }

    /// Um item da projecao: coluna simples, ou chamada de agregado
    /// (`COUNT/SUM/AVG/MIN/MAX(...)`, e `COUNT(DISTINCT coluna)`).
    fn item_de_projecao(&mut self) -> Result<ItemProjetado> {
        if let Some(nome) = self.espiar().and_then(|s| s.token.palavra_chave()) {
            if let Some(funcao) = FuncaoAgregada::de_nome_de_funcao(&nome) {
                if self.s.get(self.i + 1).map(|s| &s.token) == Some(&Token::AbreParen) {
                    // `false`: uma tabela so, o qualificador se descarta.
                    let (funcao, coluna, apelido) = self.chamada_de_agregado(funcao, false)?;
                    return Ok(ItemProjetado::Agregado {
                        funcao,
                        coluna,
                        apelido,
                    });
                }
            }
        }
        let nome = self.identificador("nome de coluna")?;
        // `t.coluna` -- o qualificador e aceito e descartado, porque so ha
        // uma tabela. Recusa-lo obrigaria a reescrever consulta de cliente
        // que sempre qualifica.
        let nome = if self.aceitar(&Token::Ponto) {
            self.identificador("nome de coluna depois do ponto")?
        } else {
            nome
        };
        let apelido = if self.aceitar_palavra("AS") {
            Some(self.identificador("apelido depois de AS")?)
        } else {
            None
        };
        Ok(ItemProjetado::Coluna(ColunaPedida { nome, apelido }))
    }

    /// Depois de espiar `FUNCAO (` sem consumir -- consome os dois e le o
    /// resto da chamada.
    ///
    /// UM leitor para a forma da chamada, e nao dois: a gramatica de uma
    /// tabela e a COMPOSTA (`consulta.rs`, pedido 394) leem o mesmo `*`, o
    /// mesmo `DISTINCT` e o mesmo `COUNT(coluna)`. Uma segunda copia
    /// divergiria no dia em que a forma mudasse -- e divergiria calada,
    /// porque as duas continuariam compilando. O que muda entre as duas e so
    /// o nome da COLUNA (ver `nome_de_coluna`), e por isso ele e um
    /// parametro.
    pub(crate) fn chamada_de_agregado(
        &mut self,
        funcao: FuncaoAgregada,
        qualificada: bool,
    ) -> Result<(FuncaoAgregada, Option<String>, Option<String>)> {
        self.i += 2; // a palavra da funcao, e o `(`
        let (funcao, coluna) =
            if funcao == FuncaoAgregada::Contagem && self.aceitar(&Token::Asterisco) {
                (FuncaoAgregada::Contagem, None)
            } else if funcao == FuncaoAgregada::Contagem && self.aceitar_palavra("DISTINCT") {
                let c = self.nome_de_coluna(qualificada, "coluna de COUNT(DISTINCT ...)")?;
                (FuncaoAgregada::Distintos, Some(c))
            } else if funcao == FuncaoAgregada::Contagem {
                // `COUNT(coluna)` -- conta so os NAO-NULOS da coluna, e nao
                // LINHA como `COUNT(*)` (pedido 236). O JSON e o mesmo
                // agregado de sempre (`"funcao":"contagem","coluna":c`); a
                // distincao entre contar linha e contar nao-nulo mora no
                // acumulador do lado do motor, que nao e desta crate --
                // aqui so faltava a leitura SQL.
                let c = self.nome_de_coluna(qualificada, "coluna de COUNT(...)")?;
                (FuncaoAgregada::Contagem, Some(c))
            } else {
                let c = self.nome_de_coluna(qualificada, "coluna do agregado")?;
                (funcao, Some(c))
            };
        if !self.aceitar(&Token::FechaParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava ) do agregado{}", self.mas_veio()),
            ));
        }
        let apelido = if self.aceitar_palavra("AS") {
            Some(self.identificador("apelido do agregado")?)
        } else {
            None
        };
        Ok((funcao, coluna, apelido))
    }

    /// Um nome de coluna, com ou sem qualificador.
    ///
    /// Na gramatica de UMA tabela o qualificador se le e se DESCARTA -- e o
    /// que o `item_de_projecao` ja fazia com a coluna simples, porque so ha
    /// uma tabela e recusar `t.preco` obrigaria a reescrever consulta de
    /// cliente que sempre qualifica. Na COMPOSTA ele FICA: depois de uma
    /// junção `p.total` e `c.total` sao colunas diferentes, e descartar o
    /// prefixo escolheria uma das duas calado.
    pub(crate) fn nome_de_coluna(&mut self, qualificada: bool, rotulo: &str) -> Result<String> {
        let nome = self.identificador(rotulo)?;
        if !self.aceitar(&Token::Ponto) {
            return Ok(nome);
        }
        let sufixo = self.identificador("nome de coluna depois do ponto")?;
        Ok(if qualificada {
            format!("{nome}.{sufixo}")
        } else {
            sufixo
        })
    }

    /// `tabela`, `schema.tabela` ou `database.schema.tabela`.
    pub(crate) fn alvo(&mut self) -> Result<Alvo> {
        let mut partes = vec![self.identificador("nome de tabela")?];
        while self.aceitar(&Token::Ponto) {
            partes.push(self.identificador("nome depois do ponto")?);
            if partes.len() > 3 {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "o endereco vai ate tres partes: database.schema.tabela",
                ));
            }
        }
        let (database, schema, tabela) = match partes.len() {
            1 => (String::new(), String::new(), partes.remove(0)),
            2 => {
                let t = partes.remove(1);
                (String::new(), partes.remove(0), t)
            }
            _ => {
                let t = partes.remove(2);
                let s = partes.remove(1);
                (partes.remove(0), s, t)
            }
        };
        // O apelido da tabela: com AS ou sem, mas nunca uma clausula solta.
        let apelido = if self.aceitar_palavra("AS") {
            Some(self.identificador("apelido da tabela depois de AS")?)
        } else {
            match self.espiar().map(|s| &s.token) {
                Some(Token::Palavra { texto, citado }) => {
                    let alto = texto.to_uppercase();
                    if *citado || !CLAUSULAS.contains(&alto.as_str()) {
                        let a = texto.clone();
                        self.i += 1;
                        Some(a)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        Ok(Alvo {
            database,
            schema,
            tabela,
            apelido,
        })
    }

    pub(crate) fn condicao(&mut self) -> Result<Condicao> {
        let coluna = self.identificador("nome de coluna no WHERE")?;
        let coluna = if self.aceitar(&Token::Ponto) {
            self.identificador("nome de coluna depois do ponto")?
        } else {
            coluna
        };
        let pos = self.posicao_atual();
        let op = match self.espiar().map(|s| s.token.clone()) {
            Some(Token::Comparador(c)) => {
                self.i += 1;
                c
            }
            _ => {
                // `IS NULL`, `LIKE`, `IN`, `BETWEEN` -- cada um recusado pelo
                // proprio nome, porque cada um falta por um motivo diferente.
                let palavra = self
                    .espiar()
                    .and_then(|s| s.token.palavra_chave())
                    .unwrap_or_default();
                let motivo = match palavra.as_str() {
                    "LIKE" => {
                        "LIKE precisaria varrer comparando texto linha a linha; o \
                               varrer sabe fazer isso (`onde` com `contem`), mas \
                               so dentro da pagina que examina, e um SELECT nao \
                               tem onde dizer isso a quem le a resposta"
                    }
                    "IN" => {
                        "IN e uma lista de buscas; o motor faz cada uma, mas quem \
                             junta os resultados ainda nao existe"
                    }
                    "BETWEEN" => {
                        "BETWEEN e faixa de indice, e a faixa ainda nao esta \
                                  exposta no protocolo"
                    }
                    "IS" => "IS NULL nao tem filtro embaixo: nulo se ve lendo a linha",
                    _ => "esperava um comparador (=, <>, <, <=, >, >=)",
                };
                return Err(lexico::erro(pos, motivo));
            }
        };
        let valor = self.literal()?;
        if self.aceitar_palavra("AND") || self.aceitar_palavra("OR") {
            return Err(lexico::erro(
                self.posicao_atual(),
                "o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, \
                 e nao ha planejador que decida por qual indice comecar",
            ));
        }
        Ok(Condicao { coluna, op, valor })
    }

    /// O `WHERE` de um `SELECT`. Tenta a forma antiga primeiro -- se os
    /// tokens ate a proxima clausula de nivel superior forem EXATAMENTE uma
    /// comparacao `coluna op literal`, sem sobra nenhuma, e `Onde::Simples`
    /// (a mesma `Condicao` de sempre, que ainda desce indice). Qualquer outra
    /// coisa vira o TEXTO normalizado dos tokens, para o avaliador do motor.
    ///
    /// # Por que capturar tudo ANTES de decidir
    ///
    /// `condicao()` ja recusa (com erro) no primeiro AND/OR, LIKE, IN etc. --
    /// e e exatamente esse erro que diz "nao e a forma simples". Mas ele
    /// olha o cursor PRINCIPAL, e um erro ali deixaria o cursor em posicao
    /// incerta para tentar de novo. Por isso a captura roda sobre uma COPIA
    /// dos tokens (um sub-cursor), e o cursor principal so anda depois de
    /// decidido -- ele nunca ve a tentativa que falhou.
    pub(crate) fn onde_da_selecao(&mut self) -> Result<Onde> {
        let pos = self.posicao_atual();
        // `UNION` entra nas paradas junto das clausulas: sem ele,
        // `WHERE x = 1 UNION SELECT ...` engoliria o segundo SELECT inteiro
        // para dentro da expressao, e a recusa falaria de uma expressao que o
        // motor nao le em vez de falar do UNION que alguem escreveu.
        let tokens =
            self.capturar_ate_clausula(&["GROUP", "HAVING", "ORDER", "LIMIT", "OFFSET", "UNION"])?;
        if tokens.is_empty() {
            return Err(lexico::erro(pos, "esperava uma condicao depois de WHERE"));
        }
        let mut sub = Analisador {
            s: tokens.clone(),
            i: 0,
        };
        if let Ok(c) = sub.condicao() {
            if sub.espiar().is_none() {
                return Ok(Onde::Simples(c));
            }
        }
        Ok(Onde::Expressao(normalizar_tokens(&tokens)))
    }

    /// Consome tokens ate achar, no NIVEL MAIS EXTERNO (fora de parenteses),
    /// uma das palavras de parada, um `;`, ou o fim do comando -- e devolve o
    /// que consumiu, sem a palavra de parada. Serve para `WHERE` e (mais
    /// adiante) `HAVING`: as duas clausulas nao tem gramatica fechada aqui, e
    /// e assim que a captura sabe onde parar sem entender o que ha por
    /// dentro.
    pub(crate) fn capturar_ate_clausula(&mut self, paradas: &[&str]) -> Result<Vec<Simbolo>> {
        let mut profundidade = 0i32;
        let mut tokens = Vec::new();
        while let Some(s) = self.espiar() {
            match &s.token {
                Token::AbreParen => profundidade += 1,
                Token::FechaParen => {
                    profundidade -= 1;
                    if profundidade < 0 {
                        return Err(lexico::erro(s.posicao, "fecha parenteses sem abrir"));
                    }
                }
                Token::PontoEVirgula if profundidade == 0 => break,
                _ if profundidade == 0 => {
                    if let Some(p) = s.token.palavra_chave() {
                        if paradas.contains(&p.as_str()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
            tokens.push(s.clone());
            self.i += 1;
        }
        if profundidade != 0 {
            return Err(lexico::erro(
                self.posicao_atual(),
                "parenteses aberto e nao fechado",
            ));
        }
        Ok(tokens)
    }

    pub(crate) fn literal(&mut self) -> Result<Literal> {
        let pos = self.posicao_atual();
        let Some(s) = self.espiar() else {
            return Err(lexico::erro(pos, "esperava um valor, e o comando acabou"));
        };
        // O NUMERO NEGATIVO. O lexico entrega `-5` como dois simbolos --
        // `Menos` e `Numero("5")` --, e este e o unico lugar que os junta:
        // um `SET a = -5` e um `VALUES (40, -3)` caiam com «esperava um valor
        // e veio "-"», enquanto o `WHERE a = -5` passava por outro caminho (a
        // expressao). O sinal so vale colado num numero: `- 'x'` continua
        // recusando pela frase de sempre, porque texto nao tem sinal.
        if matches!(s.token, Token::Menos) {
            if let Some(Token::Numero(n)) = self.s.get(self.i + 1).map(|x| &x.token) {
                let lit = Literal::Numero(format!("-{n}"));
                self.i += 2;
                return Ok(lit);
            }
        }
        let lit = match &s.token {
            Token::Numero(n) => Literal::Numero(n.clone()),
            Token::Texto(t) => Literal::Texto(t.clone()),
            Token::Palavra {
                texto,
                citado: false,
            } => match texto.to_uppercase().as_str() {
                "NULL" => Literal::Nulo,
                "TRUE" => Literal::Bool(true),
                "FALSE" => Literal::Bool(false),
                _ => {
                    return Err(lexico::erro(
                        pos,
                        &format!(
                            "esperava um valor e veio {texto:?}. Comparar coluna com \
                             coluna nao tem quem avalie"
                        ),
                    ))
                }
            },
            outro => {
                return Err(lexico::erro(
                    pos,
                    &format!("esperava um valor e veio {:?}", outro.descrever()),
                ))
            }
        };
        self.i += 1;
        Ok(lit)
    }

    pub(crate) fn inteiro(&mut self, papel: &str) -> Result<u64> {
        let pos = self.posicao_atual();
        match self.espiar().map(|s| s.token.clone()) {
            Some(Token::Numero(n)) if !n.contains('.') => {
                self.i += 1;
                n.parse::<u64>()
                    .map_err(|_| lexico::erro(pos, &format!("{papel} nao cabe: {n}")))
            }
            _ => Err(lexico::erro(
                pos,
                &format!("esperava {papel} como numero inteiro{}", self.mas_veio()),
            )),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A maioria dos testes desta secao ainda testa a forma SIMPLES do
    /// WHERE -- o dia em que ela recusa (e vira Expressao) e o item 2 quem
    /// prova, la embaixo.
    fn simples(o: Onde) -> Condicao {
        match o {
            Onde::Simples(c) => c,
            Onde::Expressao(e) => panic!("esperava Onde::Simples, veio expressao: {e}"),
        }
    }

    // ------------- pedido 394: chamada de funcao onde a gramatica le coluna

    /// As TRES portas que leem nome de coluna e caem no MESMO
    /// `identificador()`. Sao irmãs por chamada, nao por nome: sem o crivo,
    /// as tres fecham a lista numa coluna fantasma e o erro sai como
    /// `sobrou "(" depois do fim do comando` -- longe do problema.
    ///
    /// Com o defeito reposto (sem `recusar_funcao_no_lugar_de_coluna`) esta
    /// prova FALHA nas tres, porque nenhuma mensagem cita a chamada.
    #[test]
    fn chamada_de_funcao_no_lugar_de_coluna_recusa_nomeando() {
        let erro = |sql: &str| analisar_comando(sql).unwrap_err().to_string();
        // ORDER BY restrito (o SELECT que nao agrupa).
        let e = erro("SELECT * FROM t ORDER BY SUM(v)");
        assert!(e.contains("ORDER BY nao aceita a chamada SUM(...)"), "{e}");
        assert!(e.contains("Pedido 394"), "{e}");
        // ORDER BY em lista (o SELECT agrupado e o composto).
        let e = erro("SELECT cidade, SUM(v) AS s FROM t GROUP BY cidade ORDER BY SUM(v)");
        assert!(e.contains("ORDER BY nao aceita a chamada SUM(...)"), "{e}");
        // GROUP BY.
        let e = erro("SELECT cidade FROM t GROUP BY UPPER(cidade)");
        assert!(
            e.contains("GROUP BY nao aceita a chamada UPPER(...)"),
            "{e}"
        );
        // PARTITION BY -- a MESMA funcao do GROUP BY, e por isso a recusa
        // tem de dizer PARTITION BY e nao GROUP BY: mandar quem escreveu
        // `OVER (PARTITION BY ...)` procurar um GROUP BY que nao existe na
        // frase e a mesma mentira que o pedido 394 veio consertar.
        let e = erro(
            "SELECT ROW_NUMBER() OVER (PARTITION BY MAX(x)) AS n, a.i FROM a JOIN b ON a.i = b.i",
        );
        assert!(
            e.contains("PARTITION BY nao aceita a chamada MAX(...)"),
            "{e}"
        );
        assert!(!e.contains("GROUP BY"), "{e}");
    }

    /// COMPORTAMENTO VELHO, byte a byte: o `agrupar` que ja saia continua
    /// saindo igual -- inclusive a saida que a propria recusa RECOMENDA
    /// (`SUM(v) AS s` + `ORDER BY s`), medida e nao citada.
    #[test]
    fn o_agrupar_que_ja_traduzia_continua_traduzindo_byte_a_byte() {
        let ix = vec![crate::traduzir::IndiceInfo {
            nome: "porId".into(),
            colunas: vec![crate::traduzir::ColunaDoIndice {
                nome: "id".into(),
                desc: false,
            }],
            unico: true,
            primario: true,
        }];
        let pedido = |sql: &str| {
            crate::traduzir::traduzir(&analisar(sql).unwrap(), &ix, "loja")
                .unwrap()
                .pedido
                .escrever()
        };
        assert_eq!(
            pedido("SELECT cidade, SUM(v) AS s FROM t GROUP BY cidade ORDER BY s LIMIT 5"),
            r#"{"op":"agrupar","database":"loja","tabela":"t","por":["cidade"],"agregados":[{"funcao":"soma","apelido":"s","coluna":"v"}],"onde":[],"expressao":"","tendo":"","ordem":[{"coluna":"s","desc":false}],"max":5}"#
        );
        assert_eq!(
            pedido("SELECT cidade FROM t GROUP BY cidade HAVING COUNT(*) > 1"),
            r#"{"op":"agrupar","database":"loja","tabela":"t","por":["cidade"],"agregados":[],"onde":[],"expressao":"","tendo":"COUNT ( * ) > 1","ordem":[]}"#
        );
    }

    /// E o crivo olha o `(` SEGUINTE, nao a palavra: uma coluna entre aspas
    /// chamada `sum` continua ordenando.
    #[test]
    fn coluna_com_nome_de_funcao_continua_ordenando() {
        let s = analisar("SELECT * FROM t ORDER BY \"sum\"").unwrap();
        assert_eq!(s.ordem.unwrap().coluna, "sum");
    }

    #[test]
    fn select_estrela() {
        let s = analisar("SELECT * FROM Clientes").unwrap();
        assert_eq!(s.projecao, Projecao::Tudo);
        assert_eq!(s.de.tabela, "Clientes");
        assert!(s.de.schema.is_empty());
        assert!(s.onde.is_none());
    }

    #[test]
    fn colunas_com_apelido() {
        let s = analisar("select id, nome as cliente from Clientes").unwrap();
        let Projecao::Colunas(c) = s.projecao else {
            panic!("esperava lista de colunas")
        };
        assert_eq!(c[0].rotulo(), "id");
        assert_eq!(c[1].rotulo(), "cliente");
        assert_eq!(c[1].nome, "nome");
    }

    #[test]
    fn endereco_com_schema() {
        let s = analisar("SELECT * FROM matriz.estoque").unwrap();
        assert_eq!(s.de.schema, "matriz");
        assert_eq!(s.de.tabela, "estoque");
        assert_eq!(s.de.nome_no_protocolo(), "matriz.estoque");
        assert!(s.de.database.is_empty());
    }

    #[test]
    fn endereco_com_database_e_schema() {
        let s = analisar("SELECT * FROM Comercial.filial.estoque").unwrap();
        assert_eq!(s.de.database, "Comercial");
        assert_eq!(s.de.nome_no_protocolo(), "filial.estoque");
    }

    #[test]
    fn where_ordem_e_limite() {
        let s = analisar("SELECT * FROM t WHERE uf = 'SC' ORDER BY nome DESC LIMIT 10 OFFSET 20")
            .unwrap();
        let o = simples(s.onde.unwrap());
        assert_eq!(o.coluna, "uf");
        assert_eq!(o.op, Comparador::Igual);
        assert_eq!(o.valor, Literal::Texto("SC".into()));
        let ord = s.ordem.unwrap();
        assert_eq!(ord.coluna, "nome");
        assert!(ord.desc);
        assert_eq!(s.limite, Some(10));
        assert_eq!(s.salto, 20);
    }

    #[test]
    fn count_estrela() {
        let s = analisar("SELECT COUNT(*) AS quantos FROM Clientes").unwrap();
        assert_eq!(s.projecao, Projecao::Contagem);
    }

    #[test]
    fn decimal_nao_vira_f64() {
        let s = analisar("SELECT * FROM t WHERE limite = 1500.00").unwrap();
        assert_eq!(
            simples(s.onde.unwrap()).valor,
            Literal::Numero("1500.00".into()),
            "o decimal tem de chegar ao motor com os dois zeros"
        );
    }

    #[test]
    fn apelido_de_tabela_sem_as() {
        let s = analisar("SELECT c.nome FROM Clientes c WHERE c.uf = 'SC'").unwrap();
        assert_eq!(s.de.apelido.as_deref(), Some("c"));
        assert_eq!(simples(s.onde.unwrap()).coluna, "uf");
    }

    /// A armadilha do apelido sem AS: `FROM t WHERE ...` nao pode ler o WHERE
    /// como apelido da tabela e depois reclamar que falta o WHERE.
    #[test]
    fn clausula_nao_vira_apelido() {
        let s = analisar("SELECT * FROM Clientes WHERE id = 1").unwrap();
        assert_eq!(s.de.apelido, None);
        assert!(s.onde.is_some());
        let s = analisar("SELECT * FROM Clientes LIMIT 5").unwrap();
        assert_eq!(s.de.apelido, None);
        assert_eq!(s.limite, Some(5));
    }

    #[test]
    fn bulkinsert_nao_e_nome_de_tabela() {
        // A regra do docs/SQL.md: e palavra do motor, e o parser tem de dizer
        // isso em vez de tentar abrir uma tabela chamada BULKINSERT.
        let e = analisar("SELECT * FROM BULKINSERT")
            .unwrap_err()
            .to_string();
        assert!(e.contains("BULKINSERT"), "{e}");
        assert!(e.to_lowercase().contains("carga"), "{e}");
        // E entre aspas ela volta a ser um nome, para quem ja tem a tabela.
        assert!(analisar("SELECT * FROM \"BULKINSERT\"").is_ok());
    }

    #[test]
    fn bulkinsert_como_comando_diz_que_e_de_sessao() {
        let e = analisar("BULKINSERT(true)").unwrap_err().to_string();
        assert!(e.contains("SESSAO"), "{e}");
    }

    /// O `analisar` e o tradutor de CONSULTA, e transacao nao e consulta: ela
    /// e reconhecida antes, pelo `crate::transacao::comando`. Quem cai aqui
    /// escreveu uma forma que nenhum dos dois entende, e a recusa diz as
    /// formas que existem em vez de dizer que a transacao nao existe.
    #[test]
    fn transacao_nao_e_consulta_e_a_recusa_lista_as_formas() {
        for c in ["BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT p1"] {
            let e = analisar(c).unwrap_err().to_string();
            assert!(e.contains("SESSAO"), "{c}: {e}");
            assert!(e.contains("START TRANSACTION"), "{c}: {e}");
        }
        // E o caminho de verdade continua reconhecendo os quatro.
        for c in ["BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT p1"] {
            assert!(
                crate::transacao::comando(c).unwrap().is_some(),
                "{c} tem de ser reconhecido como comando de transacao"
            );
        }
    }

    // AND/OR, LIKE, IN (lista), BETWEEN e IS NULL sairam daqui no dia em que
    // o item 2 (WHERE em forma de expressao) entrou -- a prova deles esta em
    // `onde_em_forma_de_expressao`, la embaixo. SUM(x) e GROUP BY sairam no
    // dia do item 3 -- a prova deles esta em
    // `sintaxe::testes::group_by_e_agregados` e em `traduzir::testes`. JOIN
    // saiu no dia do item 8 -- a prova esta em `consulta::testes`
    // (`junção...`). `COUNT(coluna)` saiu no dia do pedido 236 -- a prova
    // (agora de PASSAR) esta em
    // `count_de_coluna_sem_distinct_vira_agregado_de_contagem`. INSERT,
    // UPDATE e DELETE sairam daqui no dia em que passaram a existir: as
    // recusas DELES moram em `dml.rs`, uma por falta.
    //
    // DISTINCT saiu no dia em que ganhou substrato, e o teste virou o PAR
    // dele: ele PASSA no SELECT simples e continua recusando onde nao ha por
    // onde. A lista ficou vazia -- e uma lista vazia se escreve como lista
    // vazia, porque uma que enumera menos casos do que existem e o defeito
    // que esta casa mais paga. A prova do que continua faltando mora onde a
    // falta mora: `consulta.rs` (WITH RECURSIVE, correlacao que nao e
    // igualdade, EXISTS nao correlacionado) e `traduzir.rs` (WHERE de faixa
    // sem indice, OFFSET com agrupamento).
    #[test]
    fn o_distinct_passa_no_select_simples_e_recusa_onde_nao_ha_por_onde() {
        // PASSA, e vira `agrupar` -- ver `traduzir::testes`.
        let s = analisar("SELECT DISTINCT a FROM t").unwrap();
        assert!(s.distinto);

        // E continua recusando, NOMEANDO, nas tres formas sem substrato.
        for sql in [
            "SELECT DISTINCT * FROM t",
            "SELECT DISTINCT COUNT(*) FROM t",
            "SELECT DISTINCT a FROM t GROUP BY a",
        ] {
            let e = analisar(sql).unwrap_err().to_string();
            assert!(e.contains("substrato"), "{sql} -> {e}");
        }
    }

    // ------------------------------------------- item 2: WHERE-expressao

    #[test]
    fn onde_simples_continua_simples() {
        // O caminho antigo nao muda: uma comparacao so continua Simples, e
        // ORDER BY/LIMIT/OFFSET continuam parando a captura do WHERE.
        let s = analisar("SELECT * FROM t WHERE id = 1 ORDER BY id LIMIT 5").unwrap();
        assert!(matches!(s.onde, Some(Onde::Simples(_))));
        assert_eq!(s.limite, Some(5));
    }

    #[test]
    fn onde_em_forma_de_expressao() {
        for (sql, esperado) in [
            ("SELECT * FROM t WHERE a = 1 AND b = 2", "a = 1 AND b = 2"),
            ("SELECT * FROM t WHERE a = 1 OR b = 2", "a = 1 OR b = 2"),
            ("SELECT * FROM t WHERE nome LIKE 'a%'", "nome LIKE 'a%'"),
            ("SELECT * FROM t WHERE id IN (1,2)", "id IN ( 1 , 2 )"),
            (
                "SELECT * FROM t WHERE id BETWEEN 1 AND 2",
                "id BETWEEN 1 AND 2",
            ),
            ("SELECT * FROM t WHERE id IS NULL", "id IS NULL"),
            ("SELECT * FROM t WHERE id IS NOT NULL", "id IS NOT NULL"),
            (
                "SELECT * FROM t WHERE preco * 1.1 > 100",
                "preco * 1.1 > 100",
            ),
            (
                "SELECT * FROM t WHERE UPPER(nome) = 'ANA'",
                "UPPER ( nome ) = 'ANA'",
            ),
            ("SELECT * FROM t WHERE (a = 1)", "( a = 1 )"),
            ("SELECT * FROM t WHERE a = b", "a = b"),
        ] {
            let s = analisar(sql).unwrap();
            match s.onde {
                Some(Onde::Expressao(texto)) => assert_eq!(texto, esperado, "{sql}"),
                outro => panic!("{sql} -> esperava Expressao, veio {outro:?}"),
            }
        }
    }

    /// A captura para na proxima clausula de nivel superior, respeitando
    /// parenteses -- um `ORDER BY` DENTRO de um `IN (...)` nao existe nesta
    /// gramatica, mas o teste garante que a captura nao para cedo demais num
    /// parentese aberto.
    #[test]
    fn expressao_do_where_para_na_proxima_clausula() {
        let s = analisar("SELECT * FROM t WHERE a > 1 AND b < 2 ORDER BY a LIMIT 10").unwrap();
        match s.onde {
            Some(Onde::Expressao(texto)) => assert_eq!(texto, "a > 1 AND b < 2"),
            outro => panic!("esperava Expressao, veio {outro:?}"),
        }
        assert_eq!(s.ordem.unwrap().coluna, "a");
        assert_eq!(s.limite, Some(10));
    }

    #[test]
    fn parenteses_desbalanceados_no_where_recusa() {
        let e = analisar("SELECT * FROM t WHERE (a = 1")
            .unwrap_err()
            .to_string();
        assert!(e.contains("parenteses"), "{e}");
        let e = analisar("SELECT * FROM t WHERE a = 1)")
            .unwrap_err()
            .to_string();
        assert!(e.contains("parenteses"), "{e}");
    }

    #[test]
    fn texto_com_aspa_normaliza_dobrando_a_aspa() {
        let s = analisar("SELECT * FROM t WHERE a = 1 AND nome LIKE 'O''Brien%'").unwrap();
        match s.onde {
            Some(Onde::Expressao(texto)) => {
                assert!(texto.contains("'O''Brien%'"), "{texto}")
            }
            outro => panic!("esperava Expressao, veio {outro:?}"),
        }
    }

    /// `phxsql_core::expressao` (o avaliador que ganhou a rodada de 08/09) le
    /// `c.uf` como UM token -- e so quando ele vem sem espaco em volta do
    /// ponto. Se a normalizacao juntasse os tokens com espaco tambem no
    /// ponto (`c . uf`), o avaliador do outro lado nao leria mais coluna
    /// nenhuma: essa e a prova de que os dois lados falam a mesma lingua,
    /// nao so um teste desta camada isolado.
    #[test]
    fn nome_qualificado_normaliza_sem_espaco_e_o_avaliador_do_core_le() {
        let s = analisar("SELECT * FROM Clientes c WHERE c.uf = 'SC' AND c.saldo > 0").unwrap();
        let texto = match s.onde {
            Some(Onde::Expressao(t)) => t,
            outro => panic!("esperava Expressao, veio {outro:?}"),
        };
        assert_eq!(texto, "c.uf = 'SC' AND c.saldo > 0");
        assert!(!texto.contains(" . "), "{texto}");

        // A prova real: o mesmo texto tem de ANALISAR no avaliador do core,
        // e citar as duas colunas qualificadas -- nao "c", "uf", "saldo"
        // separados.
        let e = phxsql_core::expressao::Expressao::analisar(&texto).unwrap();
        assert_eq!(e.colunas(), &["c.uf", "c.saldo"]);
    }

    #[test]
    fn erro_diz_onde() {
        let e = analisar("SELECT * FRON t").unwrap_err().to_string();
        assert!(e.contains("esperava FROM"), "{e}");
        assert!(e.contains("coluna 10"), "{e}");
    }

    #[test]
    fn sobra_depois_do_comando() {
        let e = analisar("SELECT * FROM t; SELECT * FROM t2")
            .unwrap_err()
            .to_string();
        assert!(e.contains("um comando por vez"), "{e}");
    }

    #[test]
    fn comando_empilhado_acha_o_segundo_comando() {
        for sql in [
            "SELECT * FROM clientes; DROP TABLE clientes; --",
            "SELECT * FROM clientes; DELETE FROM clientes",
            "SELECT * FROM clientes ;;; DROP TABLE clientes",
            "SELECT * FROM clientes WHERE nome = 'x'; EXEC xp_cmdshell('dir')",
        ] {
            assert!(comando_empilhado(sql), "devia acusar: {sql}");
        }
    }

    /// A metade que importa: nada disto e injecao, e acusar aqui bloquearia
    /// quem escreve SQL legitimo.
    #[test]
    fn comando_empilhado_nao_acusa_o_legitimo() {
        for sql in [
            "SELECT * FROM clientes",
            "SELECT * FROM clientes;",
            "SELECT * FROM clientes ; ",
            "SELECT * FROM clientes WHERE nome = 'Alves' -- e o resto",
            "SELECT /* comentario */ * FROM clientes",
            // O veneno como DADO: a aspa fecha, entao e um simbolo Texto so.
            "SELECT * FROM clientes WHERE nome = '; DROP TABLE clientes; --'",
            // Erro de sintaxe nao e injecao -- e o lexico nem le este.
            "SELECT * FROM clientes WHERE nome = '' OR '1'='1",
            // O corpo de um procedimento e cheio de `;` legitimo. Ele nao e
            // desta gramatica, e nao pode ser acusado nem quando falha.
            "CREATE PROCEDURE p(OUT n INT) BEGIN DECLARE i INT DEFAULT 0; \
             SET i = i + 1; SET n = i; END",
            "CREATE TRIGGER t BEFORE INSERT ON clientes FOR EACH ROW \
             BEGIN SET NEW.nome = 'x'; END",
            "CALL somar_ate(100)",
            "BEGIN",
            "COMMIT",
            "SHOW PROCEDURES",
            // Ponto-e-virgula sozinho no fim nao e um segundo comando.
            "SELECT * FROM clientes;;",
        ] {
            assert!(!comando_empilhado(sql), "nao devia acusar: {sql}");
        }
    }

    #[test]
    fn comando_vazio() {
        assert!(analisar("   ").is_err());
        assert!(analisar("-- so um comentario").is_err());
    }

    // -------------------------------------------------------- parametros

    #[test]
    fn parametro_resolve_no_where_antes_da_sintaxe() {
        use phxsql_core::json::Json;
        let c = analisar_comando_com("SELECT * FROM t WHERE id = ?", &[Json::Numero(7.0)]).unwrap();
        let Comando::Selecao(s) = c else {
            panic!("esperava SELECT")
        };
        assert_eq!(simples(s.onde.unwrap()).valor, Literal::Numero("7".into()));
    }

    #[test]
    fn parametro_de_texto_nao_reabre_o_comando() {
        // A prova real do item 1: o parametro carrega um `;DROP TABLE...`
        // como DADO. Se a substituicao fosse por texto (e nao por token), o
        // comando reanalisado quebraria em dois -- aqui ele continua um so,
        // e o valor chega inteiro ao literal.
        use phxsql_core::json::Json;
        let veneno = "; DROP TABLE clientes; --";
        let c = analisar_comando_com("SELECT * FROM t WHERE nome = ?", &[Json::texto_de(veneno)])
            .unwrap();
        let Comando::Selecao(s) = c else {
            panic!("esperava SELECT")
        };
        assert_eq!(
            simples(s.onde.unwrap()).valor,
            Literal::Texto(veneno.into())
        );
    }

    #[test]
    fn parametro_nulo_e_booleano() {
        use phxsql_core::json::Json;
        let c = analisar_comando_com(
            "UPDATE t SET a = ? WHERE id = ?",
            &[Json::Bool(false), Json::Numero(3.0)],
        )
        .unwrap();
        let Comando::Atualizacao(a) = c else {
            panic!("esperava UPDATE")
        };
        assert_eq!(a.atribuicoes[0].1, Literal::Bool(false));
        assert_eq!(a.onde.valor, Literal::Numero("3".into()));

        let c = analisar_comando_com("INSERT INTO t (a) VALUES (?)", &[Json::Nulo]).unwrap();
        let Comando::Insercao(i) = c else {
            panic!("esperava INSERT")
        };
        assert_eq!(i.valores[0], Literal::Nulo);
    }

    #[test]
    fn contagem_diferente_recusa_nomeando_os_dois_numeros() {
        use phxsql_core::json::Json;
        let e = analisar_comando_com("SELECT * FROM t WHERE id = ?", &[])
            .unwrap_err()
            .to_string();
        assert!(e.contains("vieram 0 parametros"), "{e}");
        assert!(e.contains("tem 1 `?`"), "{e}");

        let e = analisar_comando_com(
            "SELECT * FROM t WHERE id = ?",
            &[Json::Numero(1.0), Json::Numero(2.0)],
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("vieram 2 parametros"), "{e}");
        assert!(e.contains("tem 1 `?`"), "{e}");
    }

    #[test]
    fn analisar_comando_sem_parametro_continua_igual() {
        // `analisar_comando` e `analisar_comando_com(_, &[])` -- o mesmo
        // resultado, para quem nunca usou parametro nao precisar mudar nada.
        assert_eq!(
            analisar_comando("SELECT * FROM t WHERE id = 1").unwrap(),
            analisar_comando_com("SELECT * FROM t WHERE id = 1", &[]).unwrap()
        );
    }

    // ------------------------------------ item 3: GROUP BY e agregados

    #[test]
    fn count_estrela_sozinho_continua_o_caminho_rapido() {
        // Sem GROUP BY, sem mais nada na lista -- Projecao::Contagem, igual
        // a antes do item 3.
        let s = analisar("SELECT COUNT(*) FROM c").unwrap();
        assert_eq!(s.projecao, Projecao::Contagem);
        assert!(s.agrupar_por.is_empty());
    }

    #[test]
    fn agregado_sozinho_sem_group_by_vira_agregada_com_por_vazio() {
        let s = analisar("SELECT SUM(preco) AS total FROM c").unwrap();
        assert!(s.agrupar_por.is_empty());
        let Projecao::Agregada(itens) = s.projecao else {
            panic!("esperava Agregada")
        };
        assert_eq!(itens.len(), 1);
        assert_eq!(
            itens[0],
            ItemProjetado::Agregado {
                funcao: FuncaoAgregada::Soma,
                coluna: Some("preco".into()),
                apelido: Some("total".into()),
            }
        );
    }

    #[test]
    fn group_by_com_coluna_e_agregados_e_having() {
        let s = analisar(
            "SELECT cidade, COUNT(*), SUM(preco) AS total, AVG(preco), MIN(preco), \
             MAX(preco), COUNT(DISTINCT preco) FROM c WHERE ativo = TRUE GROUP BY cidade \
             HAVING total > 100 ORDER BY total DESC LIMIT 5",
        )
        .unwrap();
        assert_eq!(s.agrupar_por, vec!["cidade".to_string()]);
        assert_eq!(s.tendo.as_deref(), Some("total > 100"));
        let Projecao::Agregada(itens) = &s.projecao else {
            panic!("esperava Agregada")
        };
        assert_eq!(itens.len(), 7);
        assert_eq!(
            itens[0],
            ItemProjetado::Coluna(ColunaPedida {
                nome: "cidade".into(),
                apelido: None
            })
        );
        assert_eq!(
            itens[1],
            ItemProjetado::Agregado {
                funcao: FuncaoAgregada::Contagem,
                coluna: None,
                apelido: None
            }
        );
        assert_eq!(
            itens[6],
            ItemProjetado::Agregado {
                funcao: FuncaoAgregada::Distintos,
                coluna: Some("preco".into()),
                apelido: None
            }
        );
        assert_eq!(
            s.ordem_lista,
            vec![Ordenacao {
                coluna: "total".into(),
                desc: true
            }]
        );
        assert_eq!(s.limite, Some(5));
        // O onde SIMPLES continua Simples mesmo dentro do caminho agrupado.
        assert!(matches!(s.onde, Some(Onde::Simples(_))));
    }

    #[test]
    fn order_by_de_varias_colunas_so_vale_no_caminho_agrupado() {
        // Sem GROUP BY, duas colunas no ORDER BY continuam recusando (pediria
        // indice composto).
        let e = analisar("SELECT * FROM t ORDER BY a, b")
            .unwrap_err()
            .to_string();
        assert!(e.contains("indice composto"), "{e}");

        // Com GROUP BY, varias colunas no ORDER BY sao aceitas -- o
        // resultado ja esta em memoria.
        let s = analisar("SELECT a, COUNT(*) FROM t GROUP BY a ORDER BY a, a DESC").unwrap();
        assert_eq!(
            s.ordem_lista,
            vec![
                Ordenacao {
                    coluna: "a".into(),
                    desc: false
                },
                Ordenacao {
                    coluna: "a".into(),
                    desc: true
                },
            ]
        );
    }

    #[test]
    fn coluna_fora_do_group_by_recusa_nomeando() {
        let e = analisar("SELECT cidade, bairro, COUNT(*) FROM c GROUP BY cidade")
            .unwrap_err()
            .to_string();
        assert!(e.contains("bairro"), "{e}");
        assert!(e.contains("GROUP BY"), "{e}");
    }

    #[test]
    fn agregado_misturado_com_coluna_sem_group_by_recusa() {
        let e = analisar("SELECT nome, COUNT(*) FROM c")
            .unwrap_err()
            .to_string();
        assert!(e.contains("nome"), "{e}");
    }

    #[test]
    fn estrela_com_group_by_recusa() {
        let e = analisar("SELECT * FROM c GROUP BY cidade")
            .unwrap_err()
            .to_string();
        assert!(e.contains("GROUP BY"), "{e}");
    }

    /// Pedido 236: `COUNT(coluna)` vira agregado `contagem` com `coluna`
    /// (conta nao-nulo), diferente de `COUNT(*)` (conta linha, `coluna:
    /// None`). Ate esta rodada a forma recusava pelo nome
    /// ("COUNT(coluna) nao tem substrato") -- este teste falhava contra o
    /// defeito reposto (esperava `Agregada`, vinha `Err`) e passa com o
    /// conserto.
    #[test]
    fn count_de_coluna_sem_distinct_vira_agregado_de_contagem() {
        let s = analisar("SELECT COUNT(preco) FROM c").unwrap();
        let Projecao::Agregada(itens) = s.projecao else {
            panic!("esperava Agregada")
        };
        assert_eq!(
            itens,
            vec![ItemProjetado::Agregado {
                funcao: FuncaoAgregada::Contagem,
                coluna: Some("preco".into()),
                apelido: None,
            }]
        );
        // Continua distinto de COUNT(*) (coluna None) e de
        // COUNT(DISTINCT ...) (funcao Distintos) -- os dois caminhos de
        // sempre nao mudam de forma.
        let s2 = analisar("SELECT COUNT(*) FROM c").unwrap();
        assert_eq!(s2.projecao, Projecao::Contagem);
        let s3 = analisar("SELECT COUNT(DISTINCT preco) FROM c").unwrap();
        let Projecao::Agregada(itens3) = s3.projecao else {
            panic!("esperava Agregada")
        };
        assert_eq!(
            itens3,
            vec![ItemProjetado::Agregado {
                funcao: FuncaoAgregada::Distintos,
                coluna: Some("preco".into()),
                apelido: None,
            }]
        );
    }

    #[test]
    fn having_sem_condicao_recusa() {
        let e = analisar("SELECT cidade, COUNT(*) FROM c GROUP BY cidade HAVING")
            .unwrap_err()
            .to_string();
        assert!(e.contains("HAVING"), "{e}");
    }

    // ---------------------------------------------------- item 6: visoes

    #[test]
    fn create_view_guarda_o_sql_verbatim() {
        let c = analisar_comando("CREATE VIEW v_c AS SELECT * FROM c WHERE id = 1").unwrap();
        let Comando::CriarVisao { nome, sql } = c else {
            panic!("esperava CriarVisao: {c:?}")
        };
        assert_eq!(nome, "v_c");
        assert_eq!(sql, "SELECT * FROM c WHERE id = 1");
    }

    #[test]
    fn create_view_preserva_caixa_e_espaco_do_sql() {
        // O texto e VERBATIM -- nem reconstruido dos tokens, que perderia a
        // caixa original e o espaco duplo.
        let c = analisar_comando("CREATE VIEW v AS   SeLeCT  *  FROM  Clientes").unwrap();
        let Comando::CriarVisao { sql, .. } = c else {
            panic!("esperava CriarVisao")
        };
        assert_eq!(sql, "SeLeCT  *  FROM  Clientes");
    }

    #[test]
    fn create_view_com_sql_invalido_recusa_na_hora_de_criar() {
        let e = analisar_comando("CREATE VIEW v AS SELECT * FROM")
            .unwrap_err()
            .to_string();
        assert!(e.contains("esperava"), "{e}");
    }

    #[test]
    fn create_view_de_select_composto_tambem_e_analisavel() {
        // O corpo pode ser qualquer SELECT que esta camada ja traduza --
        // inclusive um WITH/subconsulta (item 4) ou agrupar (item 3).
        let c =
            analisar_comando("CREATE VIEW v AS SELECT id FROM (SELECT id FROM c) AS x").unwrap();
        assert!(matches!(c, Comando::CriarVisao { .. }));
        let c = analisar_comando("CREATE VIEW v AS SELECT cidade, COUNT(*) FROM c GROUP BY cidade")
            .unwrap();
        assert!(matches!(c, Comando::CriarVisao { .. }));
    }

    #[test]
    fn create_view_de_insert_recusa_nomeando() {
        // Nao da para escrever isto pela gramatica (CREATE VIEW exige AS
        // SELECT), mas o proprio corpo poderia, em teoria, comecar por
        // outra coisa se alguem inventasse -- a checagem existe mesmo
        // assim, e o teste prova que ela dispara.
        let e = analisar_comando("CREATE VIEW v AS UPDATE t SET a = 1 WHERE id = 1")
            .unwrap_err()
            .to_string();
        assert!(e.contains("esperava SELECT"), "{e}");
    }

    #[test]
    fn drop_view_le_o_nome() {
        let c = analisar_comando("DROP VIEW v_c").unwrap();
        assert_eq!(c, Comando::ExcluirVisao { nome: "v_c".into() });
    }

    #[test]
    fn create_view_nao_deixa_sobra_no_cursor_principal() {
        // O texto inteiro depois de AS virou `sql` -- analisar_comando_com
        // nao pode achar "sobra" nenhuma so porque os tokens do SELECT
        // continuam na lista.
        assert!(analisar_comando("CREATE VIEW v AS SELECT * FROM c;").is_ok());
    }
}

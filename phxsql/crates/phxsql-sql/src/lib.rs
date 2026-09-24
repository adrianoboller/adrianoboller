//! # phxsql-sql
//!
//! A camada SQL: analisador lexico, analisador sintatico e a traducao para
//! as operacoes que o protocolo do PhxSql ja tem. `docs/SQL.md` e o desenho;
//! `docs/propostas/comparativo-19.md` e o contrato da rodada de 08-09/2026
//! (itens 1-9) que fechou expressao, agregado, composicao, junção e visao.
//!
//! Ela existe porque **tres pendencias esperam a mesma coisa**: o driver
//! ODBC/OLE DB, o DBeaver e o protocolo de fio do PostgreSQL(R).
//!
//! ```text
//! SELECT [DISTINCT] ( * | coluna [AS apelido] {, ...}
//!          | FUNCAO(coluna) [AS apelido] {, ...}
//!          | ROW_NUMBER() OVER ([PARTITION BY ...] [ORDER BY ...]) [AS apelido] )
//! FROM   ( [database.][schema.]tabela [[AS] apelido]
//!          | nome_da_cte | (SELECT ...) AS apelido )
//!   {[INNER|LEFT [OUTER]] JOIN fonte ON coluna = coluna {AND ...}}*
//! [WHERE  coluna op literal | expressao | coluna IN (SELECT ...) | coluna op (SELECT ...)]
//! [GROUP BY coluna {, ...}] [HAVING expressao]
//! [ORDER BY coluna [DESC] {, ...}]
//! [LIMIT n [OFFSET m]]
//!
//! [WITH nome AS (SELECT ...)] SELECT ...   -- uma CTE, nao recursiva
//!
//! SELECT * FROM a UNION [ALL] SELECT * FROM b {UNION [ALL] ...}
//!
//! INSERT INTO tabela (coluna {, coluna}) VALUES (literal {, literal})
//!   [ON CONFLICT [(coluna)] DO NOTHING | DO UPDATE SET coluna = literal {, ...}]
//!   [ON DUPLICATE KEY UPDATE coluna = literal {, ...}]
//! UPDATE tabela SET coluna = literal {, coluna = literal} WHERE chave = literal
//! DELETE FROM tabela WHERE chave = literal
//!
//! CREATE VIEW nome AS SELECT ...
//! DROP VIEW nome
//! ```
//!
//! FUNCAO ∈ `COUNT(*) | COUNT(DISTINCT coluna) | SUM | AVG | MIN | MAX` --
//! e `COUNT(DISTINCT coluna)` e OUTRA pergunta que o `SELECT DISTINCT`, por
//! outro caminho: aquele conta valores distintos DENTRO de um grupo, este
//! devolve as linhas distintas. Os
//! tres verbos de escrita moram em [`dml`]: uma linha por `INSERT`, e
//! `UPDATE`/`DELETE` so por chave UNICA -- em tres passos, porque o
//! `atualizar` do protocolo grava a linha inteira. `WITH`/subconsulta no
//! `FROM`/`IN (SELECT ...)`/junção/janela moram em [`consulta`] e viram a op
//! `consultar`, traduzida por [`consulta::traduzir_consulta`] com um
//! RESOLVEDOR (ela toca varias tabelas, e cada uma pode ter indices
//! diferentes -- so o servidor conhece). `SELECT DISTINCT` vira a op
//! `agrupar` -- agrupar por N colunas E eliminar o repetido delas -- e
//! `UNION`/`UNION ALL` vira a op `unir`, por [`traduzir::traduzir_uniao`].
//!
//! # O que ele NAO faz, e por que isso esta escrito
//!
//! **Este paragrafo e um INVENTARIO, e por isso separa tres coisas que nao
//! sao a mesma**: o que nao existe, o que existe SO numa forma, e o que
//! recusa nomeando. A versao anterior dizia «nao ha `RIGHT`/`FULL`/`CROSS
//! JOIN`, correlação, `EXISTS`, `WITH RECURSIVE`, `UNION`» -- e tres dos
//! cinco ja existiam, um deles com catorze testes neste mesmo crate. Lista
//! que enumera menos casos do que existem nao protege menos hoje: protege
//! menos no dia em que alguem a usar como inventario, que foi o que aconteceu
//! com a lista das operacoes que escondem tabela do portao de permissao.
//!
//! **Existe, e e 1:1 com o protocolo:** `INNER`/`LEFT`/`RIGHT`/`FULL`/`CROSS
//! JOIN` (pedido 236), `EXISTS`/`NOT EXISTS` correlacionado POR IGUALDADE
//! (pedido 240), `SELECT DISTINCT` de colunas nomeadas, `UNION`/`UNION ALL`
//! entre tabelas INTEIRAS, uma CTE nao recursiva, subconsulta no `FROM`,
//! `IN (SELECT ...)`, subconsulta escalar e `ROW_NUMBER() OVER (...)`.
//!
//! **Existe SO nesta forma** -- e o resto da forma recusa nomeando:
//!
//! - **`UNION`**: os dois lados sao `SELECT * FROM tabela`, sem `WHERE`, sem
//!   projecao, sem ordem e sem limite. A op `unir` recebe uma lista de NOMES
//!   de tabela e abre cada uma inteira; um braco com filtro nao tem para onde
//!   ir. O pedido 393 mede o que custa mudar esse contrato.
//! - **`DISTINCT`**: so no `SELECT` SIMPLES de nivel superior, e so com as
//!   colunas nomeadas. `DISTINCT *` recusa porque o `*` desta casa carrega
//!   `rowid`/`softdeleted`/`rownum`, e o `rownum` e unico por linha --
//!   agrupar por tudo o que o `*` mostra nao tiraria repetida nenhuma.
//! - **correlação**: a de `EXISTS` POR IGUALDADE roda. A de `IN`/escalar nao,
//!   e a que nao e igualdade recusa nomeando
//!   (`exists_correlacao_nao_igualdade_recusa_nomeando`).
//! - **janela**: so `ROW_NUMBER`. Qualquer outra funcao com `OVER` recusa
//!   nomeando, e nao passa por engano.
//! - **agregado**: so no `SELECT` SIMPLES, que vira `agrupar`. Agregado
//!   `GROUP BY` e `HAVING` dentro da gramatica COMPOSTA (junção, subconsulta
//!   no `FROM`, `WITH`, `IN (SELECT …)`) recusam nomeando e citam o pedido
//!   394 -- a op `agrupar` recebe `tabela` e nunca um sub-pedido, e a op
//!   `consultar` compoe sem agregar, entao nao ha para onde traduzir.
//!   Ate 09/2026 isso nao recusava: lia `SUM` como NOME DE COLUNA e mandava
//!   procurar um `FROM` que estava escrito na frase.
//!
//! **Nao existe, e recusa nomeando:** `WITH RECURSIVE` e a segunda CTE
//! (`consulta.rs`), `EXISTS` NAO correlacionado, `WHERE` de FAIXA sem indice
//! (`traduzir.rs`), `OFFSET` com `GROUP BY`/`DISTINCT`, e `INTERSECT`/`EXCEPT`
//! -- que nao tem operacao embaixo.
//!
//! E o que NAO cabe onde a gramatica le nome de coluna: `ORDER BY SUM(v)`,
//! `GROUP BY UPPER(c)` e `PARTITION BY MAX(x)` recusam nomeando a chamada, em
//! vez de lerem a palavra da funcao como coluna.
//!
//! **Nao existe, e nao recusa -- escolhe calado:** o planejador de indice.
//! Dois candidatos de igualdade? O primeiro DECLARADO vence. E o unico item
//! desta secao que nao vira mensagem, e esta aqui para nao virar surpresa.
//!
//! Nada disso e economia de esforco: `docs/SQL.md` §3 mede o tamanho de cada
//! um. E **o que falta recusa dizendo o que falta**, com o nome da clausula:
//! aceitar a sintaxe e devolver a resposta errada calado seria o pior dos
//! dois mundos -- e e o que aconteceria se `WHERE cidade > 'X'` sem indice
//! virasse uma varredura com o filtro esquecido no caminho.
//!
//! # Como se usa
//!
//! ```
//! use phxsql_sql::{analisar, traduzir, ColunaDoIndice, IndiceInfo};
//!
//! let sel = analisar("SELECT nome FROM matriz.estoque WHERE id = 7").unwrap();
//! let indices = vec![IndiceInfo {
//!     nome: "porId".into(),
//!     colunas: vec![ColunaDoIndice { nome: "id".into(), desc: false }],
//!     unico: true,
//!     primario: true,
//! }];
//! let plano = traduzir(&sel, &indices, "Comercial").unwrap();
//! assert_eq!(plano.op, "buscar");
//! assert_eq!(plano.pedido.texto_ou("tabela", ""), "matriz.estoque");
//! ```
//!
//! `IndiceInfo` sai do `esquema` do proprio servidor, campo por campo. O crate
//! nao abre arquivo e nao fala com o disco: ele traduz texto em pedido.

pub mod consulta;
pub mod diretiva;
pub mod dml;
pub mod lexico;
pub mod rotina;
pub mod sintaxe;
pub mod traduzir;
pub mod transacao;
pub mod usuario;

pub use consulta::{
    planejar_sobre, traduzir_consulta, ColunaComposta, Consulta, EmSubconsulta, Escalar, Janela,
    Juncao, Resolvedor, TipoJuncao,
};
pub use dml::{
    traduzir_atualizacao, traduzir_exclusao, traduzir_insercao, Atualizacao, Exclusao, Insercao,
    PlanoDml, SeExistir,
};
pub use lexico::{Comparador, Simbolo, Token};
pub use sintaxe::{
    analisar, analisar_comando, analisar_comando_com, comando_empilhado, Alvo, ColunaPedida,
    Comando, Condicao, FuncaoAgregada, ItemProjetado, Literal, Onde, Ordenacao, Projecao, Selecao,
    Uniao, RESERVADAS_DO_MOTOR,
};
pub use traduzir::{
    traduzir, traduzir_criar_visao, traduzir_excluir_visao, traduzir_uniao, ColunaDoIndice,
    IndiceInfo, Plano, Saida,
};

/// Le e traduz de uma vez, para quem so quer o pedido.
pub fn compilar(
    sql: &str,
    indices: &[IndiceInfo],
    database_corrente: &str,
) -> phxsql_core::Result<Plano> {
    traduzir(&analisar(sql)?, indices, database_corrente)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn compilar_faz_os_dois_passos() {
        let ix = vec![IndiceInfo {
            nome: "porId".into(),
            colunas: vec![ColunaDoIndice {
                nome: "id".into(),
                desc: false,
            }],
            unico: true,
            primario: true,
        }];
        let p = compilar("SELECT * FROM Clientes WHERE id = 1", &ix, "Comercial").unwrap();
        assert_eq!(p.op, "buscar");
    }

    /// As tres palavras que `docs/SQL.md` manda reservar QUANDO houver parser.
    /// Agora ha -- e ROWNUM e SOFTDELETED continuam colunas legitimas, porque
    /// quem as reserva e o esquema, nao a linguagem.
    #[test]
    fn o_vocabulario_reservado_e_so_o_bulkinsert() {
        assert_eq!(RESERVADAS_DO_MOTOR, ["BULKINSERT"]);
        assert!(analisar("SELECT rownum, softdeleted FROM t").is_ok());
    }

    /// **Pedido 497: o literal do pedido nao volta no erro de sintaxe, por
    /// nenhuma das portas do tradutor.** O simbolo ofensor e citado pelo
    /// `Token::descrever`, e com o defeito ele citava o conteudo: `e veio
    /// "'SEGREDO123'"`. A mensagem vai ao `acessos.log` e ao Profiler, e la
    /// ja chega montada -- so aqui, na origem, se sabe o que e literal.
    ///
    /// E o comportamento que nao pode mudar junto: o erro continua dizendo
    /// ONDE (a coluna) e O QUE veio (um literal de texto).
    #[test]
    fn o_literal_do_pedido_nao_volta_no_erro_de_sintaxe() {
        const MARCA: &str = "SEGREDO123";
        let mut vazou = Vec::new();
        let mut sem_lugar = Vec::new();
        // O quarto campo e O QUE o erro tem de dizer que veio: o literal
        // redigido, o nome entre aspas duplas redigido -- ou nada, no
        // cadastro, onde todo simbolo pode ser a senha (B1 do parecer SEC).
        let literal = Some("'***'");
        // So o comeco: umas frases citam pelo `{:?}`, que escapa a aspa final.
        let citado = Some("\"***");
        let casos: [(&str, &str, phxsql_core::Result<()>, Option<&str>); 11] = [
            // Nao `WHERE n = 1 'x'`: esse o tradutor aceita e manda ao motor
            // como expressao, e quem recusa e o `phxsql-core` -- provado la e
            // pelo soquete.
            (
                "SELECT, sobrou",
                "SELECT n FROM t ORDER BY n 'SEGREDO123'",
                analisar_comando("SELECT n FROM t ORDER BY n 'SEGREDO123'").map(|_| ()),
                literal,
            ),
            (
                "INSERT, esperava",
                "INSERT INTO t (n) VALUES 'SEGREDO123'",
                analisar_comando("INSERT INTO t (n) VALUES 'SEGREDO123'").map(|_| ()),
                literal,
            ),
            (
                "P2, valor entre aspas duplas",
                "INSERT INTO t (n, nome) VALUES (2, \"SEGREDO123\")",
                analisar_comando("INSERT INTO t (n, nome) VALUES (2, \"SEGREDO123\")").map(|_| ()),
                citado,
            ),
            (
                "P2, aspas duplas onde vinha palavra",
                "INSERT INTO t (n) VALUES \"SEGREDO123\"",
                analisar_comando("INSERT INTO t (n) VALUES \"SEGREDO123\"").map(|_| ()),
                citado,
            ),
            (
                "CREATE USER, o login",
                "CREATE USER 'SEGREDO123' PASSWORD 'x'",
                usuario::comando("CREATE USER 'SEGREDO123' PASSWORD 'x'").map(|_| ()),
                None,
            ),
            (
                "DROP USER, sobrou",
                "DROP USER c 'SEGREDO123'",
                usuario::comando("DROP USER c 'SEGREDO123'").map(|_| ()),
                None,
            ),
            (
                "B1, pedaco de senha que sobra",
                "CREATE USER c PASSWORD 'ab'SEGREDO123'cd'",
                usuario::comando("CREATE USER c PASSWORD 'ab'SEGREDO123'cd'").map(|_| ()),
                None,
            ),
            (
                "SET TRANSACTION, o nivel",
                "SET TRANSACTION ISOLATION LEVEL 'SEGREDO123'",
                transacao::comando("SET TRANSACTION ISOLATION LEVEL 'SEGREDO123'").map(|_| ()),
                literal,
            ),
            (
                "COMMIT, sobrou",
                "COMMIT 'SEGREDO123'",
                transacao::comando("COMMIT 'SEGREDO123'").map(|_| ()),
                literal,
            ),
            (
                "SHOW, o escopo",
                "SHOW SERVER 'SEGREDO123'",
                diretiva::comando("SHOW SERVER 'SEGREDO123'").map(|_| ()),
                literal,
            ),
            (
                "SHOW, nome entre aspas duplas no escopo",
                "SHOW SERVER \"SEGREDO123\"",
                diretiva::comando("SHOW SERVER \"SEGREDO123\"").map(|_| ()),
                citado,
            ),
        ];
        for (caminho, sql, r, veio) in casos {
            let e = r
                .expect_err(&format!("{caminho}: {sql:?} devia recusar"))
                .to_string();
            if e.contains(MARCA) {
                vazou.push(format!("{caminho}: {e}"));
            }
            if !e.contains("SQL, coluna ") || veio.is_some_and(|v| !e.contains(v)) {
                sem_lugar.push(format!("{caminho}: {e}"));
            }
        }
        assert!(
            vazou.is_empty(),
            "o literal do pedido voltou no erro de sintaxe:\n  {}",
            vazou.join("\n  ")
        );
        assert!(
            sem_lugar.is_empty(),
            "o erro deixou de dizer onde e o que veio:\n  {}",
            sem_lugar.join("\n  ")
        );
    }
}

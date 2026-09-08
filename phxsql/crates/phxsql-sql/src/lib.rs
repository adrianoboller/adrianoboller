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
//! SELECT ( * | coluna [AS apelido] {, ...} | FUNCAO(coluna) [AS apelido] {, ...}
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
//! FUNCAO ∈ `COUNT(*) | COUNT(DISTINCT coluna) | SUM | AVG | MIN | MAX`. Os
//! tres verbos de escrita moram em [`dml`]: uma linha por `INSERT`, e
//! `UPDATE`/`DELETE` so por chave UNICA -- em tres passos, porque o
//! `atualizar` do protocolo grava a linha inteira. `WITH`/subconsulta no
//! `FROM`/`IN (SELECT ...)`/junção/janela moram em [`consulta`] e viram a op
//! `consultar`, traduzida por [`consulta::traduzir_consulta`] com um
//! RESOLVEDOR (ela toca varias tabelas, e cada uma pode ter indices
//! diferentes -- so o servidor conhece).
//!
//! # O que ele NAO faz, e por que isso esta escrito
//!
//! Nao ha planejador de indice (dois candidatos de igualdade? o primeiro
//! declarado vence). Nao ha `RIGHT`/`FULL`/`CROSS JOIN`, correlação,
//! `EXISTS`, `WITH RECURSIVE`, `UNION`, nem janela alem de `ROW_NUMBER`. Nada
//! disso e economia de esforco: `docs/SQL.md` §3 mede o tamanho de cada um.
//!
//! Por isso **o que falta recusa dizendo o que falta**, com o nome da
//! clausula. Aceitar a sintaxe e devolver a resposta errada calado seria o
//! pior dos dois mundos -- e e o que aconteceria se `WHERE cidade = 'X'` sem
//! indice virasse uma varredura com o filtro esquecido no caminho.
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
    RESERVADAS_DO_MOTOR,
};
pub use traduzir::{
    traduzir, traduzir_criar_visao, traduzir_excluir_visao, ColunaDoIndice, IndiceInfo, Plano,
    Saida,
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
}

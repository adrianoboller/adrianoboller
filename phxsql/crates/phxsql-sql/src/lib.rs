//! # phxsql-sql
//!
//! O comeco da camada SQL: analisador lexico, analisador sintatico de um
//! `SELECT` simples, e a traducao dele para as operacoes que o protocolo do
//! PhxSql ja tem.
//!
//! Ela existe porque **tres pendencias esperam a mesma coisa**: o driver
//! ODBC/OLE DB, o DBeaver e o protocolo de fio do PostgreSQL(R). O desenho
//! esta em `docs/SQL.md`, e este crate e o passo 1 dele.
//!
//! ```text
//! SELECT ( * | COUNT(*) | coluna [AS apelido] {, ...} )
//! FROM   [database.] [schema.] tabela [[AS] apelido]
//! [WHERE coluna ( = | <> | < | <= | > | >= ) literal]
//! [ORDER BY coluna [ASC|DESC]]
//! [LIMIT n [OFFSET m]]
//! ```
//!
//! # O que ele NAO faz, e por que isso esta escrito
//!
//! Nao ha planejador, nao ha junção, nao ha subconsulta, nao ha expressao e
//! nao ha transacao. Nada disso e economia de esforco: e o que o motor tem
//! embaixo. Um `WHERE preco * 1.1 > 100` nao tem quem avalie; um `GROUP BY`
//! geral nao existe (o `pivotar` faz a tabulacao cruzada, que e um caso); e
//! `BEGIN`/`COMMIT` nao teriam o que chamar.
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

pub mod lexico;
pub mod rotina;
pub mod sintaxe;
pub mod traduzir;
pub mod transacao;

pub use lexico::{Comparador, Simbolo, Token};
pub use sintaxe::{
    analisar, Alvo, ColunaPedida, Condicao, Literal, Ordenacao, Projecao, Selecao,
    RESERVADAS_DO_MOTOR,
};
pub use traduzir::{traduzir, ColunaDoIndice, IndiceInfo, Plano, Saida};

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

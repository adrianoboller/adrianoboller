//! O SQL que cada motor entende.
//!
//! # Por que este modulo existe
//!
//! O cliente do PostgreSQL(R) estava escrito, conferido contra o RFC 7677, e
//! **desligado de proposito**: `Motor::conecta()` devolvia `false` porque as
//! operacoes do DbLink montavam SQL de MySQL(R) -- crase em volta do nome,
//! `SHOW INDEX`, `current_user()`. Acender o botao sem o dialeto ligaria uma
//! tela que falha na primeira consulta.
//!
//! Aqui esta o dialeto. Nao e uma camada de abstracao de SQL: e a lista exata
//! das perguntas que o DbLink faz, escrita duas vezes, uma por motor.
//!
//! # Por que duas vezes, e nao um SQL "portatil"
//!
//! Porque as perguntas nao tem resposta portatil. O MySQL(R) responde "quais
//! sao os indices desta tabela" com `SHOW INDEX`; o PostgreSQL(R) responde com
//! um `JOIN` de `pg_index`, `pg_class` e `pg_attribute`. Escrever um SQL que
//! sirva aos dois significaria escrever o que **nenhum** dos dois faz bem, e a
//! diferenca reapareceria no formato do resultado de qualquer jeito.
//!
//! # As cinco diferencas que aparecem em todo lugar
//!
//! | | MySQL(R) | PostgreSQL(R) |
//! |---|---|---|
//! | identificador | crase, dobrada para escapar | aspas duplas, dobradas |
//! | catalogo | `information_schema` com `TABLE_SCHEMA` = a BASE | `information_schema` com `TABLE_SCHEMA` = o ESQUEMA |
//! | bases | `SHOW DATABASES` | `SELECT datname FROM pg_database` |
//! | booleano lido | `1` / `0` | `t` / `f` |
//! | data literal | `'2026-08-29'` | `DATE '2026-08-29'` |
//!
//! A segunda e a que mais engana: "database" quer dizer coisas diferentes nos
//! dois. No MySQL(R) uma conexao enxerga todas as bases do servidor e troca
//! entre elas; no PostgreSQL(R) uma conexao enxerga UMA base e os esquemas
//! dela, e trocar de base exige reconectar. Por isso `dblink_tabelas` com uma
//! base diferente da do login funciona num e nao no outro -- e o erro tem de
//! dizer isso, em vez de devolver lista vazia.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::types::ColumnType;

use super::{nome_seguro, Motor};

impl Motor {
    /// Um identificador entre as aspas deste motor, ja escapado.
    ///
    /// A aspa dobrada e como os dois escapam a aspa dentro do nome. Sem isto,
    /// um nome de tabela escolhido por quem usa a tela emendaria SQL -- e o
    /// `nome_seguro` ja recusa aspa, crase e contrabarra antes, entao isto e a
    /// segunda tranca e nao a primeira.
    pub fn citar(self, nome: &str) -> String {
        match self {
            Motor::MySql => format!("`{}`", nome.replace('`', "``")),
            Motor::Postgres => format!("\"{}\"", nome.replace('"', "\"\"")),
        }
    }

    /// `base.tabela`, ou so a tabela quando nao ha base escolhida.
    pub fn alvo(self, base: &str, tabela: &str) -> Result<String> {
        let t = self.citar(&nome_seguro(tabela)?);
        if base.trim().is_empty() {
            return Ok(t);
        }
        Ok(format!("{}.{t}", self.citar(&nome_seguro(base)?)))
    }

    /// O pedaco de paginacao. Os dois escrevem igual, e isso e sorte: o
    /// `LIMIT n OFFSET m` do PostgreSQL(R) e aceito pelo MySQL(R) desde o 4.0.
    ///
    /// O que NAO e igual e o `LIMIT m, n` do MySQL(R), que o PostgreSQL(R) nao
    /// entende -- e e a forma que a maioria dos exemplos de MySQL(R) usa.
    pub fn limite_offset(self, limite: i64, salto: i64) -> String {
        format!(" LIMIT {limite} OFFSET {salto}")
    }

    /// "Com quem o outro banco acha que esta falando, e em que base."
    ///
    /// Tres colunas, na mesma ordem nos dois: usuario, base, versao.
    pub fn sql_quem_sou(self) -> &'static str {
        match self {
            Motor::MySql => "SELECT current_user(), database(), version()",
            Motor::Postgres => "SELECT current_user, current_database(), version()",
        }
    }

    /// As bases do outro servidor, uma por linha na primeira coluna.
    ///
    /// No PostgreSQL(R) as bases-modelo (`datistemplate`) ficam de fora: elas
    /// aparecem na lista e nao se conectam, e uma tela que as mostra convida ao
    /// clique que falha.
    pub fn sql_bancos(self) -> &'static str {
        match self {
            Motor::MySql => "SHOW DATABASES",
            Motor::Postgres => {
                "SELECT datname FROM pg_database \
                 WHERE NOT datistemplate AND datallowconn ORDER BY datname"
            }
        }
    }

    /// As tabelas de uma base, com tamanho e comentario.
    ///
    /// Sete colunas, na mesma ordem nos dois: nome, tipo, motor, registros
    /// estimados, bytes, comentario, esquema.
    ///
    /// # A diferenca que morde
    ///
    /// No MySQL(R) `TABLE_SCHEMA` e a BASE. No PostgreSQL(R) e o ESQUEMA
    /// dentro da base, e a base e sempre a da conexao -- nao da para listar as
    /// tabelas de outra base sem reconectar nela. Por isso o filtro do
    /// PostgreSQL(R) tira os esquemas de sistema em vez de filtrar por base.
    pub fn sql_tabelas(self, base: &str) -> Result<String> {
        Ok(match self {
            Motor::MySql => {
                let onde = if base.trim().is_empty() {
                    // Do lado do MySQL(R) base vazia so acontece de um jeito:
                    // a ligacao foi salva SEM base padrao. E ai `DATABASE()`
                    // e NULO, `TABLE_SCHEMA = NULL` nao casa com nada, e o
                    // ramo devolvia lista VAZIA sem erro nenhum -- o mesmo
                    // sintoma mudo que a prova contra um PostgreSQL(R) de
                    // verdade achou do outro lado, e pelo mesmo motivo: uma
                    // consulta montada com um qualificador que nao existe.
                    //
                    // Vazio quer dizer «tudo o que este usuario enxerga», e e
                    // o que o ramo do PostgreSQL(R) ja fazia: tirar os
                    // esquemas de sistema em vez de nomear um. Cada linha traz
                    // o campo `schema`, entao quem for pedir a estrutura ou o
                    // dado sabe com que `database` pedir -- a resposta carrega
                    // o que a proxima pergunta precisa.
                    "TABLE_SCHEMA NOT IN \
                     ('mysql','information_schema','performance_schema','sys')"
                        .to_string()
                } else {
                    format!("TABLE_SCHEMA = {}", super::literal(base)?)
                };
                format!(
                    "SELECT TABLE_NAME, TABLE_TYPE, ENGINE, TABLE_ROWS, \
                     DATA_LENGTH + INDEX_LENGTH, TABLE_COMMENT, TABLE_SCHEMA \
                     FROM information_schema.TABLES WHERE {onde} ORDER BY TABLE_NAME"
                )
            }
            Motor::Postgres => {
                // `reltuples` e ESTIMATIVA (o que o ANALYZE deixou), igual ao
                // TABLE_ROWS do InnoDB -- e por isso a coluna se chama
                // "registros_estimados" na resposta, e nao "registros".
                //
                // `pg_total_relation_size` conta tabela + indices + TOAST, que
                // e o que o `DATA_LENGTH + INDEX_LENGTH` conta do outro lado.
                let onde = if base.trim().is_empty() {
                    "n.nspname NOT IN ('pg_catalog','information_schema') \
                     AND n.nspname NOT LIKE 'pg_toast%'"
                        .to_string()
                } else {
                    format!("n.nspname = {}", super::literal(base)?)
                };
                format!(
                    "SELECT c.relname, \
                     CASE c.relkind WHEN 'r' THEN 'BASE TABLE' WHEN 'v' THEN 'VIEW' \
                     WHEN 'm' THEN 'MATERIALIZED VIEW' WHEN 'p' THEN 'PARTITIONED TABLE' \
                     ELSE c.relkind::text END, \
                     'postgres', \
                     c.reltuples::bigint, \
                     pg_total_relation_size(c.oid), \
                     COALESCE(obj_description(c.oid, 'pg_class'), ''), \
                     n.nspname \
                     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE c.relkind IN ('r','v','m','p') AND {onde} \
                     ORDER BY c.relname"
                )
            }
        })
    }

    /// As colunas de uma tabela, pelos NOMES do `SHOW FULL COLUMNS`.
    ///
    /// # A forma nao casa por posicao, e nunca casou
    ///
    /// Esta doc dizia «seis colunas, na mesma ordem nos dois». Medido, e
    /// falso: o `SHOW FULL COLUMNS` do MySQL(R) devolve NOVE (`Field`, `Type`,
    /// `Collation`, `Null`, `Key`, `Default`, `Extra`, `Privileges`,
    /// `Comment`) e o lado do PostgreSQL(R) devolvia seis. As duas primeiras
    /// coincidiam por sorte; da terceira em diante nao.
    ///
    /// Quem pagou foi a tela, que lia por POSICAO: com PostgreSQL(R) ela
    /// mostrava a chave na coluna «nulo» -- e como `'PRI' != 'YES'`, toda
    /// coluna aparecia como **obrigatoria**, inclusive as que aceitam nulo.
    /// Isso e mentira sobre o dado, do mesmo naipe do «Blumenau» virando
    /// «BLUMENAU»: quem olha nao tem como saber que o banco diz outra coisa.
    /// As posicoes 6 e 8 caiam fora da linha e viravam a palavra `undefined`
    /// na tela.
    ///
    /// O conserto e o mesmo padrao que a casa ja escreveu para o texto de
    /// tela: **resolve-se por CHAVE, nunca por posicao.** O ramo do MySQL(R)
    /// nao muda uma letra -- os nomes sao os do proprio `SHOW FULL COLUMNS`;
    /// o do PostgreSQL(R) ganha `AS` com esses mesmos nomes, no lugar dos
    /// `case` e `coalesce` que o servidor inventava (e que vinham repetidos,
    /// entao nem por nome davam para ler). `Extra` e `Privileges` nao existem
    /// no PostgreSQL(R) e continuam ausentes: quem le por nome ve vazio, que
    /// e a verdade.
    pub fn sql_colunas(self, base: &str, tabela: &str) -> Result<String> {
        let t = nome_seguro(tabela)?;
        Ok(match self {
            Motor::MySql => format!("SHOW FULL COLUMNS FROM {}", self.alvo(base, &t)?),
            Motor::Postgres => {
                let esquema = if base.trim().is_empty() {
                    "current_schema()".to_string()
                } else {
                    super::literal(base)?
                };
                format!(
                    "SELECT a.attname AS \"Field\", \
                     format_type(a.atttypid, a.atttypmod) AS \"Type\", \
                     CASE WHEN a.attnotnull THEN 'NO' ELSE 'YES' END AS \"Null\", \
                     CASE WHEN EXISTS ( \
                       SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid \
                       AND i.indisprimary AND a.attnum = ANY(i.indkey)) \
                     THEN 'PRI' ELSE '' END AS \"Key\", \
                     COALESCE(pg_get_expr(d.adbin, d.adrelid), '') AS \"Default\", \
                     COALESCE(col_description(c.oid, a.attnum), '') AS \"Comment\" \
                     FROM pg_attribute a \
                     JOIN pg_class c ON c.oid = a.attrelid \
                     JOIN pg_namespace n ON n.oid = c.relnamespace \
                     LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum \
                     WHERE c.relname = {} AND n.nspname = {esquema} \
                     AND a.attnum > 0 AND NOT a.attisdropped \
                     ORDER BY a.attnum",
                    super::literal(&t)?
                )
            }
        })
    }

    /// Os indices de uma tabela, pelos NOMES do `SHOW INDEX`.
    ///
    /// # Duas mentiras nesta doc, e a segunda era pior
    ///
    /// Dizia «quatro colunas, na mesma ordem nos dois». O `SHOW INDEX` do
    /// MySQL(R) devolve QUINZE, comecando por `Table`, `Non_unique`,
    /// `Key_name`, `Seq_in_index`, `Column_name` -- entao nem a ordem nem a
    /// contagem casavam.
    ///
    /// A segunda e mais silenciosa: a POLARIDADE do «unico» era invertida. O
    /// MySQL(R) publica `Non_unique`, que vale **0 quando o indice E unico**;
    /// este ramo publicava 1 nesse caso. Quem lesse os dois pelo mesmo criterio
    /// leria o oposto num deles -- e ler indice unico como duplicado (ou o
    /// contrario) e afirmacao falsa sobre uma garantia do banco.
    ///
    /// Agora os dois falam `Non_unique`, com a polaridade do nome: 0 = unico.
    /// `Collation` (a direcao, `A`/`D`) so existe no MySQL(R) e continua so
    /// la; quem le por nome ve vazio, que e a verdade. O ramo do MySQL(R) nao
    /// muda uma letra.
    pub fn sql_indices(self, base: &str, tabela: &str) -> Result<String> {
        let t = nome_seguro(tabela)?;
        Ok(match self {
            Motor::MySql => format!("SHOW INDEX FROM {}", self.alvo(base, &t)?),
            Motor::Postgres => {
                let esquema = if base.trim().is_empty() {
                    "current_schema()".to_string()
                } else {
                    super::literal(base)?
                };
                format!(
                    "SELECT ic.relname AS \"Key_name\", \
                     a.attname AS \"Column_name\", \
                     CASE WHEN i.indisunique THEN 0 ELSE 1 END AS \"Non_unique\", \
                     k.n AS \"Seq_in_index\" \
                     FROM pg_index i \
                     JOIN pg_class c ON c.oid = i.indrelid \
                     JOIN pg_class ic ON ic.oid = i.indexrelid \
                     JOIN pg_namespace n ON n.oid = c.relnamespace \
                     JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, n) ON TRUE \
                     JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum \
                     WHERE c.relname = {} AND n.nspname = {esquema} \
                     ORDER BY ic.relname, k.n",
                    super::literal(&t)?
                )
            }
        })
    }

    /// O tipo deste motor que guarda uma coluna do PhxSql.
    ///
    /// # Onde a traducao PERDE alguma coisa, e por que ela avisa
    ///
    /// `Bin` e `Memo` viram `LONGBLOB`/`LONGTEXT` e `bytea`/`text`, que e uma
    /// traducao boa. Ja `Uuid256` nao tem tipo nativo em nenhum dos dois: vira
    /// binario de 32 bytes, e quem le do outro lado ve bytes e nao um
    /// identificador. `Sequence` vira o contador do motor -- `BIGINT AUTO_
    /// INCREMENT` e `bigserial` --, e ai o valor deixa de ser o nosso: o outro
    /// banco atribui o dele.
    ///
    /// Nada disso e escondido; esta escrito aqui porque exportar esquema para
    /// outro motor e uma conversa sobre o que se perde.
    pub fn tipo_no_create(self, tipo: &ColumnType) -> String {
        match self {
            Motor::MySql => match tipo {
                ColumnType::Bool => "TINYINT(1)".into(),
                ColumnType::Int1 => "TINYINT".into(),
                ColumnType::Int2 => "SMALLINT".into(),
                ColumnType::Int4 => "INT".into(),
                ColumnType::Int8 => "BIGINT".into(),
                ColumnType::UInt1 => "TINYINT UNSIGNED".into(),
                ColumnType::UInt2 => "SMALLINT UNSIGNED".into(),
                ColumnType::UInt4 => "INT UNSIGNED".into(),
                ColumnType::UInt8 => "BIGINT UNSIGNED".into(),
                ColumnType::Real4 => "FLOAT".into(),
                ColumnType::Real8 => "DOUBLE".into(),
                ColumnType::Decimal { precisao, escala } => {
                    format!("DECIMAL({precisao},{escala})")
                }
                ColumnType::Date => "DATE".into(),
                ColumnType::Time => "TIME".into(),
                ColumnType::DateTime => "DATETIME(3)".into(),
                ColumnType::Str(n) => format!("VARCHAR({n})"),
                ColumnType::Bin => "LONGBLOB".into(),
                ColumnType::Memo => "LONGTEXT".into(),
                ColumnType::Uuid => "BINARY(16)".into(),
                ColumnType::Uuid256 => "BINARY(32)".into(),
                ColumnType::Sequence => "BIGINT AUTO_INCREMENT".into(),
            },
            Motor::Postgres => match tipo {
                ColumnType::Bool => "boolean".into(),
                ColumnType::Int1 | ColumnType::Int2 => "smallint".into(),
                ColumnType::Int4 => "integer".into(),
                ColumnType::Int8 => "bigint".into(),
                // O PostgreSQL(R) NAO tem inteiro sem sinal. O tipo sobe um
                // tamanho para o valor caber -- e o `u64` nao cabe em `bigint`,
                // entao vira `numeric(20,0)`. Fingir que `bigint` serve
                // truncaria em silencio metade da faixa.
                ColumnType::UInt1 | ColumnType::UInt2 => "integer".into(),
                ColumnType::UInt4 => "bigint".into(),
                ColumnType::UInt8 => "numeric(20,0)".into(),
                ColumnType::Real4 => "real".into(),
                ColumnType::Real8 => "double precision".into(),
                ColumnType::Decimal { precisao, escala } => {
                    format!("numeric({precisao},{escala})")
                }
                ColumnType::Date => "date".into(),
                ColumnType::Time => "time".into(),
                ColumnType::DateTime => "timestamp(3)".into(),
                ColumnType::Str(n) => format!("varchar({n})"),
                ColumnType::Bin => "bytea".into(),
                ColumnType::Memo => "text".into(),
                ColumnType::Uuid => "uuid".into(),
                ColumnType::Uuid256 => "bytea".into(),
                ColumnType::Sequence => "bigserial".into(),
            },
        }
    }

    /// Um booleano como LITERAL no SQL deste motor.
    ///
    /// `TRUE`/`FALSE` valem nos dois, e e por isso que sao a escolha: `1`/`0`
    /// vale no MySQL(R) e o PostgreSQL(R) recusa comparar `boolean` com
    /// `integer`.
    pub fn booleano(self, v: bool) -> &'static str {
        if v {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    /// Uma data como literal. `aaaa-mm-dd` ja validado por quem chama.
    ///
    /// O PostgreSQL(R) leva o `DATE` na frente porque sem ele o literal e um
    /// texto, e texto nao se compara com coluna `date` em toda posicao. O
    /// MySQL(R) converte sozinho.
    pub fn data(self, iso: &str) -> Result<String> {
        let d = so_data_ou_hora(iso)?;
        Ok(match self {
            Motor::MySql => format!("'{d}'"),
            Motor::Postgres => format!("DATE '{d}'"),
        })
    }

    /// Uma data com hora como literal.
    pub fn data_hora(self, iso: &str) -> Result<String> {
        let d = so_data_ou_hora(iso)?;
        Ok(match self {
            Motor::MySql => format!("'{d}'"),
            Motor::Postgres => format!("TIMESTAMP '{d}'"),
        })
    }
}

/// Deixa passar so o que uma data pode ter.
///
/// A defesa e recusar, e nao escapar -- a mesma regra do `nome_seguro`. Uma
/// data nao precisa de aspa, contrabarra nem letra, entao nada disso entra, e
/// o literal fecha onde deve em qualquer modo do servidor.
fn so_data_ou_hora(iso: &str) -> Result<String> {
    let t = iso.trim();
    if t.is_empty() || t.len() > 30 {
        return Err(PhxError::Tipo(format!("data invalida: {iso:?}")));
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | ' ' | '.' | 'T'))
    {
        return Err(PhxError::Tipo(format!(
            "data com caractere que nao vale: {iso:?}"
        )));
    }
    Ok(t.to_string())
}

/// O booleano que veio do outro banco, como texto.
///
/// # Por que isto existe
///
/// O MySQL(R) devolve `1`/`0` e o PostgreSQL(R) devolve `t`/`f`. Uma grade que
/// mostre o texto cru mostra coisas diferentes para o mesmo dado, e uma
/// comparacao ingenua (`== "1"`) trata todo booleano do PostgreSQL(R) como
/// falso -- sem erro nenhum, que e o pior jeito de estar errado.
pub fn booleano_lido(texto: &str) -> Option<bool> {
    match texto.trim() {
        "1" | "t" | "true" | "TRUE" | "T" | "Y" | "y" => Some(true),
        "0" | "f" | "false" | "FALSE" | "F" | "N" | "n" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cada_motor_cita_com_a_aspa_dele() {
        assert_eq!(Motor::MySql.citar("clientes"), "`clientes`");
        assert_eq!(Motor::Postgres.citar("clientes"), "\"clientes\"");
        // O nome que emendaria SQL se entrasse cru.
        assert_eq!(
            Motor::MySql.citar("a`; DROP TABLE x; --"),
            "`a``; DROP TABLE x; --`"
        );
        assert_eq!(
            Motor::Postgres.citar("a\"; DROP TABLE x; --"),
            "\"a\"\"; DROP TABLE x; --\""
        );
    }

    #[test]
    fn o_alvo_qualifica_so_quando_ha_base() {
        assert_eq!(Motor::MySql.alvo("", "t").unwrap(), "`t`");
        assert_eq!(Motor::MySql.alvo("erp", "t").unwrap(), "`erp`.`t`");
        assert_eq!(Motor::Postgres.alvo("", "t").unwrap(), "\"t\"");
        assert_eq!(
            Motor::Postgres.alvo("public", "t").unwrap(),
            "\"public\".\"t\""
        );
        // E o nome torto e recusado antes de virar SQL, nos dois.
        assert!(Motor::Postgres.alvo("", "cli\"entes").is_err());
        assert!(Motor::MySql.alvo("cli`ente", "t").is_err());
    }

    /// A forma `LIMIT m, n` do MySQL(R) nao entra: o PostgreSQL(R) nao a
    /// entende, e ela e a que os exemplos de MySQL(R) usam.
    #[test]
    fn a_paginacao_sai_na_forma_que_os_dois_entendem() {
        for m in [Motor::MySql, Motor::Postgres] {
            let sql = m.limite_offset(50, 100);
            assert_eq!(sql, " LIMIT 50 OFFSET 100");
            assert!(!sql.contains(','), "saiu na forma so do MySQL: {sql}");
        }
    }

    #[test]
    fn as_consultas_de_catalogo_falam_o_dialeto_certo() {
        assert!(Motor::MySql.sql_bancos().contains("SHOW DATABASES"));
        assert!(Motor::Postgres.sql_bancos().contains("pg_database"));
        assert!(
            Motor::Postgres.sql_bancos().contains("datistemplate"),
            "as bases-modelo apareceriam na lista e nao conectam"
        );

        let t = Motor::Postgres.sql_tabelas("public").unwrap();
        assert!(t.contains("pg_class") && t.contains("'public'"));
        assert!(!t.contains('`'), "crase de MySQL no SQL do PostgreSQL: {t}");

        let c = Motor::Postgres.sql_colunas("public", "clientes").unwrap();
        assert!(c.contains("pg_attribute") && c.contains("'clientes'"));
        assert!(!c.contains("SHOW"), "SHOW nao existe no PostgreSQL: {c}");

        let i = Motor::Postgres.sql_indices("", "clientes").unwrap();
        assert!(i.contains("pg_index") && i.contains("current_schema()"));

        assert!(Motor::MySql.sql_quem_sou().contains("current_user()"));
        // Sem parenteses: no PostgreSQL(R) `current_user` e palavra reservada,
        // e `current_user()` e erro de sintaxe.
        assert!(Motor::Postgres.sql_quem_sou().contains("current_user,"));
        assert!(!Motor::Postgres.sql_quem_sou().contains("current_user()"));
    }

    /// O defeito que a prova contra um MySQL(R) DE VERDADE achou, e que o
    /// servidor de protocolo nao tinha como achar: uma ligacao salva sem base
    /// padrao listava ZERO tabelas, sem erro nenhum.
    ///
    /// A causa e de SQL, e nao de chamador: `TABLE_SCHEMA = DATABASE()` com
    /// `DATABASE()` nulo nao casa com nada -- em SQL `x = NULL` nunca e
    /// verdadeiro. E o ramo so e alcancado quando nao ha base padrao, entao
    /// ele estava SEMPRE vazio.
    ///
    /// Prova real: trocar a clausula de volta por `TABLE_SCHEMA = DATABASE()`
    /// derruba este teste.
    #[test]
    fn sem_base_padrao_o_mysql_nao_compara_com_o_database_nulo() {
        let sql = Motor::MySql.sql_tabelas("").unwrap();
        assert!(
            !sql.contains("DATABASE()"),
            "a comparacao com NULL voltou, e ela nunca casa: {sql}"
        );
        // Vazio quer dizer «tudo o que este usuario enxerga», como ja queria
        // dizer no PostgreSQL(R): fora os esquemas de sistema.
        for sistema in ["mysql", "information_schema", "performance_schema", "sys"] {
            assert!(
                sql.contains(&format!("'{sistema}'")),
                "o esquema de sistema {sistema} nao foi filtrado: {sql}"
            );
        }
        assert!(sql.contains("TABLE_SCHEMA NOT IN"), "{sql}");
    }

    /// O teste do comportamento VELHO, que e o que mais importa numa mudanca
    /// destas: com base escolhida nada muda, e o outro motor nao se mexeu.
    ///
    /// A regra que ele guarda: quem ja tinha uma ligacao com base padrao --
    /// que e o caso comum, e o que a tela cadastra -- ve exatamente a mesma
    /// consulta de antes. So o ramo que estava sempre vazio mudou.
    #[test]
    fn com_base_escolhida_nada_muda_e_o_postgres_nao_se_mexeu() {
        let sql = Motor::MySql.sql_tabelas("crm").unwrap();
        assert!(sql.contains("TABLE_SCHEMA = 'crm'"), "{sql}");
        assert!(
            !sql.contains("NOT IN"),
            "a base escolhida virou filtro de sistema: {sql}"
        );
        // E o filtro do PostgreSQL(R) continua o dele, pelo catalogo dele.
        let p = Motor::Postgres.sql_tabelas("").unwrap();
        assert!(p.contains("pg_catalog") && p.contains("pg_toast"), "{p}");
        assert!(
            !p.contains("TABLE_SCHEMA"),
            "SQL de MySQL no PostgreSQL: {p}"
        );
        assert!(
            Motor::Postgres
                .sql_tabelas("vendas")
                .unwrap()
                .contains("n.nspname = 'vendas'"),
            "o esquema pedido no PostgreSQL mudou"
        );
        // As outras duas perguntas de catalogo do MySQL nao foram tocadas.
        assert_eq!(
            Motor::MySql.sql_colunas("crm", "clientes").unwrap(),
            "SHOW FULL COLUMNS FROM `crm`.`clientes`"
        );
        assert_eq!(
            Motor::MySql.sql_indices("", "clientes").unwrap(),
            "SHOW INDEX FROM `clientes`"
        );
    }

    /// A forma da estrutura: os dois motores respondem pelos MESMOS NOMES.
    ///
    /// Medido antes de consertar, com os dois servidores no ar: o
    /// `SHOW FULL COLUMNS` traz nove colunas e o ramo do PostgreSQL(R) trazia
    /// seis, com nomes que o proprio servidor inventava -- `attname`,
    /// `format_type`, `case`, `case`, `coalesce`, `coalesce`. Repetidos, entao
    /// nem por nome davam para ler; e a tela lia por POSICAO.
    ///
    /// Prova real: tirar qualquer `AS` desta consulta derruba este teste.
    #[test]
    fn a_estrutura_responde_pelos_nomes_do_show_full_columns() {
        let p = Motor::Postgres.sql_colunas("publico", "clientes").unwrap();
        for nome in ["Field", "Type", "Null", "Key", "Default", "Comment"] {
            assert!(
                p.contains(&format!("AS \"{nome}\"")),
                "o PostgreSQL nao apelida {nome}: {p}"
            );
        }
        // O que o servidor inventava nao volta: nome repetido nao se le.
        assert!(!p.contains("END, "), "coluna sem apelido sobrou: {p}");
        // E o MySQL nao ganhou apelido nenhum: os nomes ja sao dele.
        assert_eq!(
            Motor::MySql.sql_colunas("crm", "clientes").unwrap(),
            "SHOW FULL COLUMNS FROM `crm`.`clientes`"
        );
    }

    /// A polaridade do «unico», que era invertida entre os dois e ninguem via.
    ///
    /// O MySQL(R) publica `Non_unique`: **0 quando o indice E unico**. Este
    /// ramo publicava 1 no mesmo caso. Ler indice unico como duplicado e
    /// afirmacao falsa sobre uma garantia do banco -- e o nome da coluna
    /// dizia uma coisa enquanto o valor dizia a outra.
    ///
    /// Prova real: voltar para `THEN 1 ELSE 0` derruba este teste.
    #[test]
    fn o_unico_do_indice_tem_a_polaridade_do_nome_nos_dois() {
        let p = Motor::Postgres.sql_indices("publico", "clientes").unwrap();
        assert!(
            p.contains("CASE WHEN i.indisunique THEN 0 ELSE 1 END AS \"Non_unique\""),
            "a polaridade nao e a de `Non_unique` (0 = unico): {p}"
        );
        for nome in ["Key_name", "Column_name", "Non_unique", "Seq_in_index"] {
            assert!(p.contains(&format!("AS \"{nome}\"")), "falta {nome}: {p}");
        }
        // `Collation` so existe no MySQL, e continua so la -- quem le por nome
        // ve vazio, que e a verdade, e nao um valor inventado para preencher.
        assert!(
            !p.contains("Collation"),
            "inventou a direcao no PostgreSQL: {p}"
        );
        assert_eq!(
            Motor::MySql.sql_indices("crm", "clientes").unwrap(),
            "SHOW INDEX FROM `crm`.`clientes`"
        );
    }

    /// O nome que emendaria SQL nao chega ao catalogo do outro lado.
    #[test]
    fn nome_torto_nao_entra_na_consulta_de_catalogo() {
        for ruim in ["cli'entes", "cli\"entes", "cli\\entes", "cli\nentes"] {
            assert!(
                Motor::Postgres.sql_colunas("public", ruim).is_err(),
                "aceitou {ruim:?}"
            );
            assert!(
                Motor::Postgres.sql_indices("public", ruim).is_err(),
                "aceitou {ruim:?}"
            );
            assert!(
                Motor::Postgres.sql_tabelas(ruim).is_err(),
                "aceitou {ruim:?}"
            );
        }
    }

    #[test]
    fn os_tipos_do_create_saem_no_dialeto_de_cada_um() {
        let m = Motor::MySql;
        let p = Motor::Postgres;
        assert_eq!(m.tipo_no_create(&ColumnType::Str(40)), "VARCHAR(40)");
        assert_eq!(p.tipo_no_create(&ColumnType::Str(40)), "varchar(40)");
        assert_eq!(m.tipo_no_create(&ColumnType::Bool), "TINYINT(1)");
        assert_eq!(p.tipo_no_create(&ColumnType::Bool), "boolean");
        assert_eq!(m.tipo_no_create(&ColumnType::Bin), "LONGBLOB");
        assert_eq!(p.tipo_no_create(&ColumnType::Bin), "bytea");
        assert_eq!(
            m.tipo_no_create(&ColumnType::Sequence),
            "BIGINT AUTO_INCREMENT"
        );
        assert_eq!(p.tipo_no_create(&ColumnType::Sequence), "bigserial");
        let d = ColumnType::Decimal {
            precisao: 15,
            escala: 2,
        };
        assert_eq!(m.tipo_no_create(&d), "DECIMAL(15,2)");
        assert_eq!(p.tipo_no_create(&d), "numeric(15,2)");
        // O PostgreSQL(R) nao tem inteiro sem sinal: o tipo SOBE para o valor
        // caber, e o u64 nao cabe em bigint.
        assert_eq!(p.tipo_no_create(&ColumnType::UInt4), "bigint");
        assert_eq!(p.tipo_no_create(&ColumnType::UInt8), "numeric(20,0)");
        assert_eq!(m.tipo_no_create(&ColumnType::UInt8), "BIGINT UNSIGNED");
    }

    #[test]
    fn booleano_e_data_saem_no_dialeto() {
        assert_eq!(Motor::MySql.booleano(true), "TRUE");
        assert_eq!(Motor::Postgres.booleano(false), "FALSE");
        assert_eq!(Motor::MySql.data("2026-08-29").unwrap(), "'2026-08-29'");
        assert_eq!(
            Motor::Postgres.data("2026-08-29").unwrap(),
            "DATE '2026-08-29'"
        );
        assert_eq!(
            Motor::Postgres.data_hora("2026-08-29 14:03:00").unwrap(),
            "TIMESTAMP '2026-08-29 14:03:00'"
        );
        // Data que emendaria SQL nao vira literal.
        for ruim in ["2026-08-29'; DROP TABLE x; --", "now()", ""] {
            assert!(Motor::Postgres.data(ruim).is_err(), "aceitou {ruim:?}");
        }
    }

    /// O defeito que este teste existe para pegar: `== "1"` trata todo
    /// booleano do PostgreSQL(R) como falso, sem erro nenhum.
    #[test]
    fn o_booleano_dos_dois_le_igual() {
        assert_eq!(booleano_lido("1"), Some(true));
        assert_eq!(booleano_lido("t"), Some(true));
        assert_eq!(booleano_lido("0"), Some(false));
        assert_eq!(booleano_lido("f"), Some(false));
        assert_eq!(booleano_lido("Blumenau"), None);
    }
}

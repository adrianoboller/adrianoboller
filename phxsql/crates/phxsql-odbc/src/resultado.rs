//! Da resposta JSON da op `sql` ao conjunto de resultados ODBC: colunas com
//! tipo honesto e linhas em texto canonico.
//!
//! O tipo honesto vem da op `esquema`, nao de adivinhar pelo valor: um "10.00"
//! pode ser texto ou decimal, e so o esquema sabe. Rotulo que o esquema nao
//! conhece (apelido de coluna, expressao) cai em SQL_VARCHAR -- e melhor
//! declarar texto e entregar o texto do que declarar um tipo inventado.

use crate::tipos::*;
use phxsql_core::json::Json;

/// Uma coluna do resultado, ja na lingua do SQLDescribeCol.
#[derive(Debug, Clone)]
pub struct Coluna {
    pub nome: String,
    pub tipo_sql: SqlSmallint,
    pub tamanho: SqlULen,
    pub decimais: SqlSmallint,
    pub nulavel: SqlSmallint,
}

impl Coluna {
    fn texto_livre(nome: &str) -> Coluna {
        Coluna {
            nome: nome.to_string(),
            tipo_sql: SQL_VARCHAR,
            tamanho: 65_535,
            decimais: 0,
            nulavel: SQL_NULLABLE_UNKNOWN,
        }
    }
}

/// O conjunto inteiro: o protocolo entrega todas as linhas de uma vez, entao
/// o cursor do ODBC anda sobre memoria, nao sobre a rede.
#[derive(Debug, Default)]
pub struct Resultado {
    pub colunas: Vec<Coluna>,
    pub linhas: Vec<Vec<Option<String>>>,
}

/// Traduz o `tipo` da ficha do esquema (o Debug do ColumnType, ex.: `Int4`,
/// `Str(40)`, `Decimal { precisao: 12, escala: 2 }`) para o trio do ODBC:
/// tipo SQL, tamanho de coluna e casas decimais.
pub fn tipo_odbc(texto: &str, tamanho: u64) -> (SqlSmallint, SqlULen, SqlSmallint) {
    let t = texto.trim();
    if let Some(resto) = t.strip_prefix("Decimal") {
        let numero = |chave: &str| -> u64 {
            resto
                .split(chave)
                .nth(1)
                .and_then(|s| {
                    let digitos: String = s
                        .trim_start_matches([':', ' '])
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    digitos.parse().ok()
                })
                .unwrap_or(0)
        };
        return (
            SQL_DECIMAL,
            numero("precisao") as SqlULen,
            numero("escala") as SqlSmallint,
        );
    }
    if t.starts_with("Str") {
        return (SQL_VARCHAR, tamanho.max(1) as SqlULen, 0);
    }
    match t {
        "Bool" => (SQL_BIT, 1, 0),
        "Int1" => (SQL_TINYINT, 3, 0),
        "Int2" => (SQL_SMALLINT, 5, 0),
        "Int4" => (SQL_INTEGER, 10, 0),
        // Sem sinal de 32 bits nao cabe em SQL_INTEGER, que e assinado:
        // declarar BIGINT e a saida que nao mente para valor acima de 2^31.
        "Int8" | "UInt4" | "UInt8" | "Sequence" => (SQL_BIGINT, 20, 0),
        "UInt1" => (SQL_TINYINT, 3, 0),
        "UInt2" => (SQL_SMALLINT, 5, 0),
        "Real4" => (SQL_REAL, 7, 0),
        "Real8" => (SQL_DOUBLE, 15, 0),
        "Date" => (SQL_TYPE_DATE, 10, 0),
        // A hora canonica do PhxSql e `HH:MM:SS,cc` (centesimos, virgula).
        "Time" => (SQL_TYPE_TIME, 11, 2),
        // `AAAA-MM-DD HH:MM:SS,mmm`.
        "DateTime" => (SQL_TYPE_TIMESTAMP, 23, 3),
        "Uuid" => (SQL_CHAR, 36, 0),
        "Uuid256" => (SQL_CHAR, 64, 0),
        // Memo e texto longo; Bin viaja como hexadecimal, que TAMBEM e texto
        // -- declarar VARBINARY prometeria bytes crus que nao vem.
        "Memo" | "Bin" => (SQL_LONGVARCHAR, 2_147_483_647, 0),
        _ => (SQL_VARCHAR, tamanho.max(1) as SqlULen, 0),
    }
}

/// A ficha de uma coluna, lida da resposta da op `esquema`.
#[derive(Debug, Clone)]
pub struct Ficha {
    pub coluna: Coluna,
    pub sistema: bool,
}

pub fn fichas_do_esquema(esquema: &Json) -> Vec<Ficha> {
    let Some(colunas) = esquema.campo("colunas").and_then(|c| c.lista()) else {
        return Vec::new();
    };
    colunas
        .iter()
        .map(|c| {
            let tipo = c.texto_ou("tipo", "");
            let tamanho = c.inteiro_ou("tamanho", 0).max(0) as u64;
            let (tipo_sql, tam, dec) = tipo_odbc(tipo, tamanho);
            Ficha {
                coluna: Coluna {
                    nome: c.texto_ou("nome", "").to_string(),
                    tipo_sql,
                    tamanho: tam,
                    decimais: dec,
                    nulavel: if c.booleano_ou("nullable", true) {
                        SQL_NULLABLE
                    } else {
                        SQL_NO_NULLS
                    },
                },
                sistema: c.booleano_ou("sistema", false),
            }
        })
        .collect()
}

/// Acha o alvo do `FROM` para pedir o esquema: `(database, tabela_no_protocolo)`.
///
/// E uma varredura, nao um parser: o SQL de verdade quem analisa e o servidor.
/// Aqui basta achar o mesmo nome que ele vai usar -- e a regra de partes e
/// copiada dele: `a.b` e schema.tabela (NAO database.tabela), `a.b.c` e
/// database.schema.tabela.
pub fn alvo_do_from(sql: &str) -> Option<(String, String)> {
    let minusculo = sql.to_ascii_lowercase();
    let bytes = minusculo.as_bytes();
    let e_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
    let mut i = 0;
    let alvo = loop {
        let resto = &minusculo[i..];
        let pos = resto.find("from")?;
        let inicio = i + pos;
        let antes_livre = inicio == 0 || !e_ident(bytes[inicio - 1]);
        let fim = inicio + 4;
        let depois_livre = fim >= bytes.len() || !e_ident(bytes[fim]);
        if antes_livre && depois_livre {
            break sql[fim..].trim_start();
        }
        i = fim;
    };
    let nome: String = alvo
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    if nome.is_empty() {
        return None;
    }
    let partes: Vec<&str> = nome.split('.').collect();
    match partes.len() {
        1 => Some((String::new(), nome)),
        2 => Some((String::new(), nome)),
        3 => Some((
            partes[0].to_string(),
            format!("{}.{}", partes[1], partes[2]),
        )),
        _ => None,
    }
}

/// O texto canonico de uma celula. `None` e o NULL do SQL.
pub fn celula_texto(v: &Json) -> Option<String> {
    match v {
        Json::Nulo => None,
        Json::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
        // O Json guarda numero em f64. Inteiro exato sai sem ".0", porque
        // "5.0" numa coluna INT faria o parse do cliente falhar.
        Json::Numero(n) => Some(if n.fract() == 0.0 && n.abs() < 9.0e15 {
            format!("{}", *n as i64)
        } else {
            format!("{n}")
        }),
        Json::Texto(t) => Some(t.clone()),
        // Lista ou objeto numa celula nao existe no protocolo de hoje; se um
        // dia vier, texto JSON e mais honesto que fingir que nao veio.
        outro => Some(outro.escrever()),
    }
}

/// Monta o conjunto de resultados a partir da resposta da op `sql` e das
/// fichas do esquema (que podem faltar -- ai todo rotulo vira texto).
pub fn montar(resposta: &Json, fichas: &[Ficha]) -> Resultado {
    let ficha_de = |nome: &str| {
        fichas
            .iter()
            .find(|f| f.coluna.nome == nome)
            .map(|f| f.coluna.clone())
    };

    // COUNT(*): a resposta e um numero, nao uma grade -- e vira uma grade de
    // uma celula, porque ODBC so tem grade.
    if let Some(n) = resposta.campo("contagem") {
        return Resultado {
            colunas: vec![Coluna {
                nome: "contagem".into(),
                tipo_sql: SQL_BIGINT,
                tamanho: 20,
                decimais: 0,
                nulavel: SQL_NO_NULLS,
            }],
            linhas: vec![vec![celula_texto(n)]],
        };
    }

    let linhas_cruas = resposta
        .campo("linhas")
        .and_then(|l| l.lista())
        .unwrap_or(&[]);

    // A lista de colunas, em ordem: a projecao do proprio SELECT quando houve
    // (`colunas`), senao o esquema sem as colunas de sistema (o `SELECT *`
    // nao mostra o que a tela tambem esconde; quem quiser uma coluna de
    // sistema pede por nome). Sem esquema, o ultimo recurso e a primeira
    // linha -- menos o `rowid`, que o protocolo poe na frente de toda linha.
    let colunas: Vec<Coluna> =
        if let Some(rotulos) = resposta.campo("colunas").and_then(Json::lista) {
            rotulos
                .iter()
                .filter_map(|r| r.texto())
                .map(|nome| ficha_de(nome).unwrap_or_else(|| Coluna::texto_livre(nome)))
                .collect()
        } else if !fichas.is_empty() {
            fichas
                .iter()
                .filter(|f| !f.sistema)
                .map(|f| f.coluna.clone())
                .collect()
        } else {
            linhas_cruas
                .first()
                .map(|l| {
                    l.chaves()
                        .into_iter()
                        .filter(|c| *c != "rowid")
                        .map(Coluna::texto_livre)
                        .collect()
                })
                .unwrap_or_default()
        };

    let linhas = linhas_cruas
        .iter()
        .map(|l| {
            colunas
                .iter()
                .map(|c| l.campo(&c.nome).and_then(celula_texto))
                .collect()
        })
        .collect();

    Resultado { colunas, linhas }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tipos_honestos() {
        assert_eq!(tipo_odbc("Int4", 4), (SQL_INTEGER, 10, 0));
        assert_eq!(tipo_odbc("Str(40)", 40), (SQL_VARCHAR, 40, 0));
        assert_eq!(
            tipo_odbc("Decimal { precisao: 12, escala: 2 }", 16),
            (SQL_DECIMAL, 12, 2)
        );
        assert_eq!(tipo_odbc("Date", 4), (SQL_TYPE_DATE, 10, 0));
        assert_eq!(tipo_odbc("Sequence", 8), (SQL_BIGINT, 20, 0));
        // UInt4 alcanca 4 bilhoes; SQL_INTEGER para em 2,1 -- BIGINT.
        assert_eq!(tipo_odbc("UInt4", 4).0, SQL_BIGINT);
    }

    #[test]
    fn from_com_uma_duas_e_tres_partes() {
        assert_eq!(
            alvo_do_from("SELECT * FROM clientes"),
            Some((String::new(), "clientes".into()))
        );
        // Duas partes e schema.tabela -- a regra do servidor, copiada.
        assert_eq!(
            alvo_do_from("select id from matriz.estoque where id = 1"),
            Some((String::new(), "matriz.estoque".into()))
        );
        assert_eq!(
            alvo_do_from("SELECT * FROM Comercial.filial.estoque"),
            Some(("Comercial".into(), "filial.estoque".into()))
        );
        // "from" dentro de identificador nao conta.
        assert_eq!(
            alvo_do_from("SELECT fromagem FROM queijos"),
            Some((String::new(), "queijos".into()))
        );
    }

    #[test]
    fn celula_inteira_sem_ponto_flutuante() {
        assert_eq!(celula_texto(&Json::de_i64(42)), Some("42".into()));
        assert_eq!(celula_texto(&Json::Numero(2.5)), Some("2.5".into()));
        assert_eq!(celula_texto(&Json::Nulo), None);
        assert_eq!(celula_texto(&Json::Bool(true)), Some("1".into()));
    }

    #[test]
    fn contagem_vira_grade_de_uma_celula() {
        let resposta = Json::analisar(r#"{"sql":"x","contagem":7}"#).unwrap();
        let r = montar(&resposta, &[]);
        assert_eq!(r.colunas.len(), 1);
        assert_eq!(r.colunas[0].tipo_sql, SQL_BIGINT);
        assert_eq!(r.linhas, vec![vec![Some("7".to_string())]]);
    }

    #[test]
    fn select_estrela_projeta_pelo_esquema_e_esconde_rowid() {
        let esquema = Json::analisar(
            r#"{"colunas":[
                {"nome":"id","tipo":"Int4","tamanho":4,"nullable":false,"sistema":false},
                {"nome":"nome","tipo":"Str(40)","tamanho":40,"nullable":true,"sistema":false},
                {"nome":"softdeleted","tipo":"Bool","tamanho":1,"nullable":false,"sistema":true}
            ]}"#,
        )
        .unwrap();
        let fichas = fichas_do_esquema(&esquema);
        let resposta =
            Json::analisar(r#"{"linhas":[{"rowid":3,"id":1,"nome":"Ana","softdeleted":false}]}"#)
                .unwrap();
        let r = montar(&resposta, &fichas);
        let nomes: Vec<&str> = r.colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["id", "nome"]);
        assert_eq!(r.linhas[0], vec![Some("1".into()), Some("Ana".into())]);
    }

    #[test]
    fn projecao_usa_os_rotulos_e_cai_em_texto_no_apelido() {
        let esquema = Json::analisar(
            r#"{"colunas":[{"nome":"limite","tipo":"Decimal { precisao: 12, escala: 2 }","tamanho":16,"nullable":true,"sistema":false}]}"#,
        )
        .unwrap();
        let fichas = fichas_do_esquema(&esquema);
        let resposta = Json::analisar(
            r#"{"colunas":["limite","apelido"],"linhas":[{"limite":"10.50","apelido":"x"}]}"#,
        )
        .unwrap();
        let r = montar(&resposta, &fichas);
        assert_eq!(r.colunas[0].tipo_sql, SQL_DECIMAL);
        assert_eq!(r.colunas[0].decimais, 2);
        assert_eq!(r.colunas[1].tipo_sql, SQL_VARCHAR);
    }
}

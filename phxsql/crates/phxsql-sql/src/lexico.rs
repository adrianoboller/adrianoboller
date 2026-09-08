//! Analisador lexico: o texto do comando vira uma lista de simbolos.
//!
//! # Por que o numero e guardado como TEXTO
//!
//! Porque `f64` nao representa `1500.00` exatamente, e o protocolo do PhxSql
//! ja trafega decimal como texto justamente por isso. Converter aqui para
//! numero de maquina e reconverter para texto la na frente perderia digito
//! num lugar onde ninguem procuraria depois. O lexico so confere o FORMATO;
//! quem sabe o tipo da coluna e o motor.

use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// Os comparadores que o `WHERE` aceita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparador {
    Igual,
    Diferente,
    Menor,
    MenorIgual,
    Maior,
    MaiorIgual,
}

impl Comparador {
    pub fn simbolo(&self) -> &'static str {
        match self {
            Comparador::Igual => "=",
            Comparador::Diferente => "<>",
            Comparador::Menor => "<",
            Comparador::MenorIgual => "<=",
            Comparador::Maior => ">",
            Comparador::MaiorIgual => ">=",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Identificador ou palavra do vocabulario, no caso em que foi escrito.
    ///
    /// O lexico NAO decide se `select` e palavra reservada ou nome de coluna:
    /// isso e trabalho da sintaxe, que sabe onde esta na frase.
    Palavra {
        texto: String,
        /// Veio entre aspas duplas -- entao e identificador, nunca palavra
        /// reservada, e o caso importa.
        citado: bool,
    },
    /// Literal numerico, preservado como foi escrito.
    Numero(String),
    /// Literal de texto, ja com o `''` desdobrado em `'`.
    Texto(String),
    Comparador(Comparador),
    Virgula,
    Ponto,
    AbreParen,
    FechaParen,
    Asterisco,
    PontoEVirgula,
    /// `+`, `-` e `/` existem por causa dos corpos de rotina (gatilhos e
    /// procedimentos), que fazem conta. O `SELECT` nao os usa -- e continua
    /// recusando expressao, agora com uma mensagem de sintaxe em vez de
    /// "caractere nao faz parte da linguagem".
    Mais,
    Menos,
    Barra,
    /// Um `?` -- parametro posicional. `n` e a ordem dele entre os `?` do
    /// MESMO comando, contada da esquerda e comecando em zero: o primeiro
    /// `?` e `Parametro(0)`, o segundo e `Parametro(1)`. E o indice direto
    /// em `parametros[n]` -- nao ha conversao de um para o outro depois.
    ///
    /// Ele nunca chega ao analisador SINTATICO: `resolver_parametros` troca
    /// cada um pelo literal da posicao ANTES da sintaxe rodar. Substituir por
    /// TEXTO em vez de por um token seria reabrir a porta que os parametros
    /// existem para fechar -- um valor viraria pedaco de comando.
    Parametro(usize),
}

impl Token {
    /// A palavra em maiusculas, quando ela puder ser do vocabulario.
    ///
    /// Identificador citado devolve `None` de proposito: `"from"` entre aspas
    /// e o nome de uma coluna chamada from, e nunca a clausula.
    pub fn palavra_chave(&self) -> Option<String> {
        match self {
            Token::Palavra { texto, citado } if !citado => Some(texto.to_uppercase()),
            _ => None,
        }
    }

    /// Como este simbolo apareceu no texto, para caber numa mensagem de erro.
    pub fn descrever(&self) -> String {
        match self {
            Token::Palavra { texto, .. } => texto.clone(),
            Token::Numero(n) => n.clone(),
            Token::Texto(t) => format!("'{t}'"),
            Token::Comparador(c) => c.simbolo().to_string(),
            Token::Virgula => ",".into(),
            Token::Ponto => ".".into(),
            Token::AbreParen => "(".into(),
            Token::FechaParen => ")".into(),
            Token::Asterisco => "*".into(),
            Token::PontoEVirgula => ";".into(),
            Token::Mais => "+".into(),
            Token::Menos => "-".into(),
            Token::Barra => "/".into(),
            Token::Parametro(_) => "?".into(),
        }
    }
}

/// Um simbolo e onde ele comeca no texto original.
///
/// A posicao existe para a mensagem de erro poder apontar o lugar. Sem ela,
/// "esperava FROM" numa consulta de tres linhas manda quem escreveu procurar.
#[derive(Debug, Clone, PartialEq)]
pub struct Simbolo {
    pub token: Token,
    pub posicao: usize,
}

/// Quebra o comando em simbolos.
///
/// Aceita comentario de linha (`-- ate o fim`) e de bloco (`/* ... */`), que
/// e o que qualquer cliente ODBC manda junto sem avisar.
pub fn analisar(entrada: &str) -> Result<Vec<Simbolo>> {
    let b: Vec<char> = entrada.chars().collect();
    let mut i = 0usize;
    let mut saida = Vec::new();
    let mut parametros = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // Comentarios. Vem antes dos operadores porque `--` comeca com `-`, e
        // `/*` com `/`. O `-` e o `/` sozinhos caem nos operadores adiante.
        if c == '-' && b.get(i + 1) == Some(&'-') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&'*') {
            let inicio = i;
            i += 2;
            loop {
                if i + 1 >= b.len() {
                    return Err(erro(inicio, "comentario /* aberto e nao fechado"));
                }
                if b[i] == '*' && b[i + 1] == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        let inicio = i;
        let token = match c {
            ',' => {
                i += 1;
                Token::Virgula
            }
            '.' => {
                i += 1;
                Token::Ponto
            }
            '(' => {
                i += 1;
                Token::AbreParen
            }
            ')' => {
                i += 1;
                Token::FechaParen
            }
            '*' => {
                i += 1;
                Token::Asterisco
            }
            ';' => {
                i += 1;
                Token::PontoEVirgula
            }
            '+' => {
                i += 1;
                Token::Mais
            }
            '-' => {
                i += 1;
                Token::Menos
            }
            '/' => {
                i += 1;
                Token::Barra
            }
            '?' => {
                let n = parametros;
                parametros += 1;
                i += 1;
                Token::Parametro(n)
            }
            '=' => {
                i += 1;
                Token::Comparador(Comparador::Igual)
            }
            '!' => {
                if b.get(i + 1) != Some(&'=') {
                    return Err(erro(inicio, "`!` sozinho nao e operador; use `!=` ou `<>`"));
                }
                i += 2;
                Token::Comparador(Comparador::Diferente)
            }
            '<' => match b.get(i + 1) {
                Some('>') => {
                    i += 2;
                    Token::Comparador(Comparador::Diferente)
                }
                Some('=') => {
                    i += 2;
                    Token::Comparador(Comparador::MenorIgual)
                }
                _ => {
                    i += 1;
                    Token::Comparador(Comparador::Menor)
                }
            },
            '>' => {
                if b.get(i + 1) == Some(&'=') {
                    i += 2;
                    Token::Comparador(Comparador::MaiorIgual)
                } else {
                    i += 1;
                    Token::Comparador(Comparador::Maior)
                }
            }
            '\'' => {
                let (texto, fim) = literal_de_texto(&b, i)?;
                i = fim;
                Token::Texto(texto)
            }
            '"' => {
                let (texto, fim) = identificador_citado(&b, i)?;
                i = fim;
                Token::Palavra {
                    texto,
                    citado: true,
                }
            }
            _ if c.is_ascii_digit() => {
                let (n, fim) = literal_numerico(&b, i)?;
                i = fim;
                Token::Numero(n)
            }
            _ if inicia_identificador(c) => {
                let mut fim = i;
                while fim < b.len() && continua_identificador(b[fim]) {
                    fim += 1;
                }
                let texto: String = b[i..fim].iter().collect();
                i = fim;
                Token::Palavra {
                    texto,
                    citado: false,
                }
            }
            outro => {
                return Err(erro(
                    inicio,
                    &format!("caractere {outro:?} nao faz parte da linguagem"),
                ))
            }
        };
        saida.push(Simbolo {
            token,
            posicao: inicio,
        });
    }
    Ok(saida)
}

/// Troca cada `Token::Parametro(n)` pelo literal de `parametros[n]`, ANTES de
/// a sintaxe rodar -- e por isso mora aqui e nao em `sintaxe.rs`: o cursor que
/// a sintaxe le nunca chega a ver um parametro nao resolvido.
///
/// # Por que troca de TOKEN, nunca de TEXTO
///
/// Substituir `?` pelo texto do parametro e reanalisar recriaria a injecao
/// que os `?` existem para fechar: um texto de usuario viraria pedaco de
/// comando, e `'; DROP TABLE clientes; --'` mandado como parametro voltaria a
/// ser SQL. Aqui o parametro vira exatamente UM token -- nunca uma sequencia
/// que o lexico teria que reanalisar --, entao o valor nunca e interpretado
/// como sintaxe.
///
/// # Contagem errada recusa nomeando os dois lados
///
/// `M` `?` no comando e `N` parametros: se `M != N` a chamada esta errada
/// (faltou ou sobrou parametro), e a mensagem diz os dois numeros -- nunca so
/// "contagem errada", que manda quem chamou contar de novo pelo texto.
pub fn resolver_parametros(simbolos: Vec<Simbolo>, parametros: &[Json]) -> Result<Vec<Simbolo>> {
    let total = simbolos
        .iter()
        .filter(|s| matches!(s.token, Token::Parametro(_)))
        .count();
    if total != parametros.len() {
        return Err(erro(
            0,
            &format!(
                "vieram {} parametros e o comando tem {total} `?`",
                parametros.len()
            ),
        ));
    }
    let mut saida = Vec::with_capacity(simbolos.len());
    for s in simbolos {
        let token = match s.token {
            Token::Parametro(n) => token_do_parametro(&parametros[n], s.posicao)?,
            outro => outro,
        };
        saida.push(Simbolo {
            token,
            posicao: s.posicao,
        });
    }
    Ok(saida)
}

/// O literal que representa este valor de parametro -- a mesma conversao que
/// `traduzir::literal_para_json` faz ao contrario: numero vira `Numero` com o
/// texto que `Json::escrever` produziria (o `Json` desta casa so tem um tipo
/// numerico, e essa e a mesma forma que ele grava em qualquer outro lugar do
/// protocolo); texto vira `Texto`; nulo e booleano viram a PALAVRA (`NULL`,
/// `TRUE`, `FALSE`), que e como a sintaxe ja le esses tres literais.
fn token_do_parametro(j: &Json, pos: usize) -> Result<Token> {
    Ok(match j {
        Json::Numero(_) => Token::Numero(j.escrever()),
        Json::Texto(t) => Token::Texto(t.clone()),
        Json::Nulo => Token::Palavra {
            texto: "NULL".into(),
            citado: false,
        },
        Json::Bool(true) => Token::Palavra {
            texto: "TRUE".into(),
            citado: false,
        },
        Json::Bool(false) => Token::Palavra {
            texto: "FALSE".into(),
            citado: false,
        },
        Json::Lista(_) | Json::Objeto(_) => {
            return Err(erro(
                pos,
                "parametro so aceita literal (numero, texto, booleano ou nulo); lista e \
                 objeto nao tem literal correspondente",
            ))
        }
    })
}

/// Acentuado vale como letra: `descrição` e nome de coluna legitimo aqui, e
/// recusa-lo obrigaria a citar entre aspas uma coluna que o motor aceita.
fn inicia_identificador(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn continua_identificador(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `'texto'`, com `''` valendo uma aspa dentro.
fn literal_de_texto(b: &[char], inicio: usize) -> Result<(String, usize)> {
    let mut i = inicio + 1;
    let mut texto = String::new();
    loop {
        if i >= b.len() {
            return Err(erro(inicio, "literal de texto aberto e nao fechado"));
        }
        if b[i] == '\'' {
            if b.get(i + 1) == Some(&'\'') {
                texto.push('\'');
                i += 2;
                continue;
            }
            return Ok((texto, i + 1));
        }
        texto.push(b[i]);
        i += 1;
    }
}

/// `"coluna"` -- identificador citado, com `""` valendo uma aspa dentro.
fn identificador_citado(b: &[char], inicio: usize) -> Result<(String, usize)> {
    let mut i = inicio + 1;
    let mut texto = String::new();
    loop {
        if i >= b.len() {
            return Err(erro(
                inicio,
                "identificador entre aspas aberto e nao fechado",
            ));
        }
        if b[i] == '"' {
            if b.get(i + 1) == Some(&'"') {
                texto.push('"');
                i += 2;
                continue;
            }
            if texto.is_empty() {
                return Err(erro(inicio, "identificador entre aspas vazio"));
            }
            return Ok((texto, i + 1));
        }
        texto.push(b[i]);
        i += 1;
    }
}

/// Digitos, com no maximo um ponto decimal. Sem expoente: nenhum tipo de
/// coluna do PhxSql se escreve `1e3`, e aceitar a notacao so criaria um
/// caminho por onde um `f64` entraria sem ninguem pedir.
fn literal_numerico(b: &[char], inicio: usize) -> Result<(String, usize)> {
    let mut i = inicio;
    let mut viu_ponto = false;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            i += 1;
        } else if b[i] == '.' && !viu_ponto && b.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            viu_ponto = true;
            i += 1;
        } else {
            break;
        }
    }
    if i < b.len() && continua_identificador(b[i]) {
        return Err(erro(
            inicio,
            "numero colado num identificador; separe com espaco",
        ));
    }
    Ok((b[inicio..i].iter().collect(), i))
}

pub(crate) fn erro(posicao: usize, texto: &str) -> PhxError {
    PhxError::Esquema(format!("SQL, coluna {}: {texto}", posicao + 1))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tokens(s: &str) -> Vec<Token> {
        analisar(s).unwrap().into_iter().map(|x| x.token).collect()
    }

    #[test]
    fn palavras_numeros_e_pontuacao() {
        assert_eq!(
            tokens("SELECT id, nome FROM Clientes;"),
            vec![
                Token::Palavra {
                    texto: "SELECT".into(),
                    citado: false
                },
                Token::Palavra {
                    texto: "id".into(),
                    citado: false
                },
                Token::Virgula,
                Token::Palavra {
                    texto: "nome".into(),
                    citado: false
                },
                Token::Palavra {
                    texto: "FROM".into(),
                    citado: false
                },
                Token::Palavra {
                    texto: "Clientes".into(),
                    citado: false
                },
                Token::PontoEVirgula,
            ]
        );
    }

    #[test]
    fn decimal_continua_texto() {
        // O dia em que isto virar f64, 1500.00 sai 1499.9999999999998.
        assert_eq!(tokens("1500.00"), vec![Token::Numero("1500.00".into())]);
    }

    #[test]
    fn aspas_dobradas_viram_uma() {
        assert_eq!(tokens("'O''Brien'"), vec![Token::Texto("O'Brien".into())]);
    }

    #[test]
    fn identificador_citado_nao_e_palavra_chave() {
        let t = &tokens("\"from\"")[0];
        assert_eq!(t.palavra_chave(), None);
        assert_eq!(
            t,
            &Token::Palavra {
                texto: "from".into(),
                citado: true
            }
        );
    }

    #[test]
    fn comparadores() {
        assert_eq!(
            tokens("= <> != < <= > >="),
            vec![
                Token::Comparador(Comparador::Igual),
                Token::Comparador(Comparador::Diferente),
                Token::Comparador(Comparador::Diferente),
                Token::Comparador(Comparador::Menor),
                Token::Comparador(Comparador::MenorIgual),
                Token::Comparador(Comparador::Maior),
                Token::Comparador(Comparador::MaiorIgual),
            ]
        );
    }

    #[test]
    fn comentarios_somem() {
        assert_eq!(
            tokens("SELECT -- isto some\n 1 /* e isto tambem */ , 2"),
            vec![
                Token::Palavra {
                    texto: "SELECT".into(),
                    citado: false
                },
                Token::Numero("1".into()),
                Token::Virgula,
                Token::Numero("2".into()),
            ]
        );
    }

    #[test]
    fn ponto_separa_o_endereco() {
        // `matriz.estoque` chega como tres simbolos; juntar e da sintaxe.
        assert_eq!(
            tokens("matriz.estoque"),
            vec![
                Token::Palavra {
                    texto: "matriz".into(),
                    citado: false
                },
                Token::Ponto,
                Token::Palavra {
                    texto: "estoque".into(),
                    citado: false
                },
            ]
        );
    }

    #[test]
    fn acentuado_e_identificador() {
        assert_eq!(
            tokens("descrição"),
            vec![Token::Palavra {
                texto: "descrição".into(),
                citado: false
            }]
        );
    }

    #[test]
    fn literal_aberto_e_erro_com_lugar() {
        let e = analisar("SELECT 'sem fim").unwrap_err().to_string();
        assert!(e.contains("coluna 8"), "{e}");
        assert!(e.contains("nao fechado"), "{e}");
    }

    #[test]
    fn numero_colado_em_letra_e_erro() {
        assert!(analisar("12abc").is_err());
    }

    #[test]
    fn expoente_nao_existe() {
        // `1e3` cai na regra do numero colado em letra, e e de proposito.
        assert!(analisar("1e3").is_err());
    }

    #[test]
    fn interrogacao_conta_da_esquerda_a_partir_de_zero() {
        assert_eq!(
            tokens("id = ? AND nome = ?"),
            vec![
                Token::Palavra {
                    texto: "id".into(),
                    citado: false
                },
                Token::Comparador(Comparador::Igual),
                Token::Parametro(0),
                Token::Palavra {
                    texto: "AND".into(),
                    citado: false
                },
                Token::Palavra {
                    texto: "nome".into(),
                    citado: false
                },
                Token::Comparador(Comparador::Igual),
                Token::Parametro(1),
            ]
        );
    }

    #[test]
    fn resolver_troca_cada_parametro_pelo_literal_da_posicao() {
        // a=0 ==1 ?=2 OR=3 b=4 ==5 ?=6 OR=7 c=8 ==9 ?=10 OR=11 d=12 ==13 ?=14
        let s = analisar("a = ? OR b = ? OR c = ? OR d = ?").unwrap();
        let r = resolver_parametros(
            s,
            &[
                Json::Numero(7.0),
                Json::texto_de("Ana"),
                Json::Nulo,
                Json::Bool(true),
            ],
        )
        .unwrap();
        let t: Vec<&Token> = r.iter().map(|s| &s.token).collect();
        assert_eq!(*t[2], Token::Numero("7".into()));
        assert_eq!(*t[6], Token::Texto("Ana".into()));
        assert_eq!(
            *t[10],
            Token::Palavra {
                texto: "NULL".into(),
                citado: false
            }
        );
        assert_eq!(
            *t[14],
            Token::Palavra {
                texto: "TRUE".into(),
                citado: false
            }
        );
    }

    #[test]
    fn resolver_com_contagem_diferente_recusa_nomeando_os_dois_numeros() {
        let s = analisar("a = ? AND b = ?").unwrap();
        let e = resolver_parametros(s, &[Json::Numero(1.0)])
            .unwrap_err()
            .to_string();
        assert!(e.contains("vieram 1 parametros"), "{e}");
        assert!(e.contains("tem 2 `?`"), "{e}");
    }

    #[test]
    fn resolver_sem_nenhum_parametro_no_texto_aceita_lista_vazia() {
        let s = analisar("a = 1").unwrap();
        assert!(resolver_parametros(s, &[]).is_ok());
    }

    /// Lista e objeto nao tem literal SQL correspondente -- a recusa nomeia o
    /// motivo em vez de tentar inventar uma sintaxe para eles.
    #[test]
    fn resolver_recusa_lista_e_objeto_pelo_nome() {
        let s = analisar("a = ?").unwrap();
        let e = resolver_parametros(s, &[Json::Lista(vec![])])
            .unwrap_err()
            .to_string();
        assert!(e.contains("literal"), "{e}");
    }
}

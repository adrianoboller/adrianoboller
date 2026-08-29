//! Analisador lexico: o texto do comando vira uma lista de simbolos.
//!
//! # Por que o numero e guardado como TEXTO
//!
//! Porque `f64` nao representa `1500.00` exatamente, e o protocolo do PhxSql
//! ja trafega decimal como texto justamente por isso. Converter aqui para
//! numero de maquina e reconverter para texto la na frente perderia digito
//! num lugar onde ninguem procuraria depois. O lexico so confere o FORMATO;
//! quem sabe o tipo da coluna e o motor.

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
}

//! Expressoes de esquema e de filtro.
//!
//! Uma gramatica so para o `CHECK`, o `DEFAULT`, a coluna calculada, o
//! indice parcial, o indice por expressao e o `expressao` do `varrer` -- e ela
//! mora AQUI, no core, por um motivo de dependencia e nao de gosto: quem
//! grava a linha e o `phxsql-store`, e o avaliador que ja existia (o da
//! linguagem dos gatilhos, em `phxsql-sql/src/rotina.rs`) mora numa crate
//! que o store nao enxerga. Restricao de esquema conferida no servidor e nao
//! no motor seria a mesma porta dos fundos que a chave estrangeira ja teve:
//! o FFI, a replica e o proprio `--example` gravariam sem passar por ela.
//!
//! # O que ela aceita
//!
//! Literais (`12`, `1.1`, `'texto'`, `TRUE`, `FALSE`, `NULL`), coluna pelo
//! nome (sem distinguir caixa), `+ - * /`, `= <> != < <= > >=`, `AND OR NOT`,
//! `IS [NOT] NULL`, `[NOT] IN (lit, ...)`, `[NOT] BETWEEN a AND b`,
//! `[NOT] LIKE` com `%` e `_`, parenteses, e as funcoes `UPPER LOWER TRIM
//! LENGTH ROUND ABS COALESCE CONCAT` -- as mesmas dos gatilhos, com a mesma
//! semantica (`LENGTH` conta caracteres, `CONCAT` com um `NULL` da `NULL`).
//!
//! # Numero e numero exato
//!
//! `1.1` e um decimal de escala 1, nao um `f64`: `preco * 1.1` sobre uma
//! coluna `Decimal(12,2)` da um decimal de escala 3, sem perder centavo. So a
//! divisao e o `Real` levam a conta para `f64`. Quem coage o resultado para
//! uma coluna `Decimal` com menos casas recebe recusa mandando usar `ROUND`,
//! em vez de um arredondamento calado.
//!
//! # `NULL`
//!
//! Propaga como no SQL: aritmetica e comparacao com `NULL` dao `NULL`; `AND`
//! e `OR` sao de tres valores; `IS NULL` e o unico que responde sobre ele.
//! Quem usa a expressao decide o que `NULL` significa -- o `CHECK` passa
//! (e o SQL), o filtro exclui -- e por isso [`Expressao::avaliar_bool`]
//! devolve `Option<bool>` e nao `bool`.
//!
//! # Data, hora e identificador
//!
//! Entram na conta como TEXTO no formato ISO, que compara bem por ordem de
//! bytes (`'2026-01-02' > '2026-01-01'`). Aritmetica de datas nao existe
//! nesta rodada, e recusa pelo nome.

use crate::carga::{decimal_para_texto, texto_para_decimal, valor_de_texto};
use crate::error::{PhxError, Result};
use crate::types::ColumnType;
use crate::value::Value;

// ------------------------------------------------------------------ valores

/// Um numero durante a conta. Tres dominios, porque a coluna decide qual vale:
/// inteiro e decimal sao exatos, e o real e o que sobra quando um dos lados
/// ja era `f64` ou quando se divide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numero {
    Inteiro(i128),
    /// Ja escalado: `1.10` e `Decimal { valor: 110, escala: 2 }`.
    Decimal {
        valor: i128,
        escala: u8,
    },
    Real(f64),
}

impl Numero {
    fn como_f64(self) -> f64 {
        match self {
            Numero::Inteiro(i) => i as f64,
            Numero::Decimal { valor, escala } => valor as f64 / 10f64.powi(escala as i32),
            Numero::Real(r) => r,
        }
    }

    /// Os dois na mesma escala decimal, quando nenhum e real.
    fn alinhar(a: Numero, b: Numero) -> Option<(i128, i128, u8)> {
        let (va, ea) = match a {
            Numero::Inteiro(i) => (i, 0),
            Numero::Decimal { valor, escala } => (valor, escala),
            Numero::Real(_) => return None,
        };
        let (vb, eb) = match b {
            Numero::Inteiro(i) => (i, 0),
            Numero::Decimal { valor, escala } => (valor, escala),
            Numero::Real(_) => return None,
        };
        let e = ea.max(eb);
        Some((
            va.checked_mul(pot10(e - ea)?)?,
            vb.checked_mul(pot10(e - eb)?)?,
            e,
        ))
    }

    fn texto(self) -> String {
        match self {
            Numero::Inteiro(i) => i.to_string(),
            Numero::Decimal { valor, escala } => decimal_para_texto(valor, escala),
            Numero::Real(r) => r.to_string(),
        }
    }
}

fn pot10(n: u8) -> Option<i128> {
    10i128.checked_pow(n as u32)
}

/// O resultado de uma avaliacao. Nao e [`Value`] porque um numero aqui ainda
/// nao tem coluna -- a escala de um `Decimal` do `Value` mora no esquema, e a
/// conta precisa carrega-la junto ate alguem coagir para um tipo.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Bool(bool),
    Num(Numero),
    Texto(String),
}

impl Valor {
    /// `Some(true)`/`Some(false)` para booleano; `None` para `NULL`.
    pub fn como_bool(&self) -> Result<Option<bool>> {
        match self {
            Valor::Nulo => Ok(None),
            Valor::Bool(b) => Ok(Some(*b)),
            Valor::Num(n) => Ok(Some(n.como_f64() != 0.0)),
            Valor::Texto(t) => Err(PhxError::Tipo(format!(
                "a expressao devolveu texto ({t:?}) onde se esperava verdadeiro ou falso"
            ))),
        }
    }

    fn como_texto(&self) -> String {
        match self {
            Valor::Nulo => String::new(),
            Valor::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
            Valor::Num(n) => n.texto(),
            Valor::Texto(t) => t.clone(),
        }
    }

    fn como_numero(&self, onde: &str) -> Result<Numero> {
        match self {
            Valor::Num(n) => Ok(*n),
            outro => Err(PhxError::Tipo(format!(
                "{onde} precisa de numero e recebeu {}",
                outro.descricao()
            ))),
        }
    }

    fn descricao(&self) -> String {
        match self {
            Valor::Nulo => "NULL".into(),
            Valor::Bool(b) => format!("o booleano {b}"),
            Valor::Num(n) => format!("o numero {}", n.texto()),
            Valor::Texto(t) => format!("o texto {t:?}"),
        }
    }

    /// De um valor gravado para um valor de conta, com o tipo da coluna, que
    /// e quem sabe a escala do `Decimal`.
    pub fn de_value(v: &Value, ty: &ColumnType) -> Valor {
        match v {
            Value::Null => Valor::Nulo,
            Value::Bool(b) => Valor::Bool(*b),
            Value::Int(i) => Valor::Num(Numero::Inteiro(*i as i128)),
            Value::UInt(u) => Valor::Num(Numero::Inteiro(*u as i128)),
            Value::Real(r) => Valor::Num(Numero::Real(*r)),
            Value::Decimal(d) => {
                let escala = match ty {
                    ColumnType::Decimal { escala, .. } => *escala,
                    _ => 0,
                };
                Valor::Num(Numero::Decimal { valor: *d, escala })
            }
            Value::Str(s) | Value::Memo(s) => Valor::Texto(s.clone()),
            // Data, hora, instante e identificadores entram como o texto ISO
            // que o proprio motor imprime -- comparam bem por bytes.
            outro => Valor::Texto(outro.para_texto()),
        }
    }
}

// --------------------------------------------------------------- gramatica

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    E,
    Ou,
    Igual,
    Diferente,
    Menor,
    MenorIgual,
    Maior,
    MaiorIgual,
    Mais,
    Menos,
    Vezes,
    Dividir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funcao {
    Maiusculas,
    Minusculas,
    Aparar,
    Comprimento,
    Arredondar,
    Absoluto,
    Coalescer,
    Concatenar,
}

impl Funcao {
    fn de_nome(nome: &str) -> Option<Funcao> {
        Some(match nome.to_ascii_uppercase().as_str() {
            "UPPER" | "UCASE" => Funcao::Maiusculas,
            "LOWER" | "LCASE" => Funcao::Minusculas,
            "TRIM" => Funcao::Aparar,
            "LENGTH" | "CHAR_LENGTH" => Funcao::Comprimento,
            "ROUND" => Funcao::Arredondar,
            "ABS" => Funcao::Absoluto,
            "COALESCE" | "IFNULL" => Funcao::Coalescer,
            "CONCAT" => Funcao::Concatenar,
            _ => return None,
        })
    }
}

const FUNCOES: &str =
    "UPPER, LOWER, TRIM, LENGTH/CHAR_LENGTH, ROUND, ABS, COALESCE/IFNULL e CONCAT";

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Valor),
    Coluna(String),
    Negativo(Box<Expr>),
    Nao(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    ENulo {
        e: Box<Expr>,
        negado: bool,
    },
    Em {
        e: Box<Expr>,
        lista: Vec<Expr>,
        negado: bool,
    },
    Entre {
        e: Box<Expr>,
        de: Box<Expr>,
        ate: Box<Expr>,
        negado: bool,
    },
    Parece {
        e: Box<Expr>,
        padrao: Box<Expr>,
        negado: bool,
    },
    Chamada(Funcao, Vec<Expr>),
}

/// Uma expressao analisada uma vez e avaliada muitas.
///
/// Duas expressoes sao iguais quando o TEXTO e igual: e o texto que o esquema
/// grava e compara, e a arvore carrega um `f64` (o real), que nao tem `Eq`.
#[derive(Debug, Clone)]
pub struct Expressao {
    texto: String,
    raiz: Expr,
    colunas: Vec<String>,
}

impl PartialEq for Expressao {
    fn eq(&self, outro: &Self) -> bool {
        self.texto == outro.texto
    }
}

impl Eq for Expressao {}

impl Expressao {
    pub fn analisar(texto: &str) -> Result<Expressao> {
        let tokens = lexer(texto)?;
        if tokens.is_empty() {
            return Err(PhxError::Esquema("expressao vazia".into()));
        }
        let mut p = Parser {
            tokens,
            i: 0,
            texto,
        };
        let raiz = p.ou()?;
        if p.i < p.tokens.len() {
            return Err(p.erro(&format!(
                "sobrou {} depois do fim da expressao",
                p.tokens[p.i].mostrar()
            )));
        }
        let mut colunas = Vec::new();
        colher_colunas(&raiz, &mut colunas);
        Ok(Expressao {
            texto: texto.trim().to_string(),
            raiz,
            colunas,
        })
    }

    /// O texto como veio, aparado -- e o que o esquema grava.
    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// As colunas referidas, sem repetir, na ordem em que aparecem. E com
    /// isto que a declaracao recusa coluna inexistente ANTES de gravar.
    pub fn colunas(&self) -> &[String] {
        &self.colunas
    }

    /// Avalia contra uma linha. O resolvedor recebe o nome da coluna como
    /// esta na expressao e devolve o valor e o tipo -- o tipo porque a escala
    /// de um `Decimal` mora nele.
    pub fn avaliar<'a>(
        &self,
        resolver: &dyn Fn(&str) -> Option<(&'a Value, &'a ColumnType)>,
    ) -> Result<Valor> {
        avaliar(&self.raiz, resolver)
    }

    /// `Some(true)`, `Some(false)` ou `None` quando a expressao da `NULL`.
    pub fn avaliar_bool<'a>(
        &self,
        resolver: &dyn Fn(&str) -> Option<(&'a Value, &'a ColumnType)>,
    ) -> Result<Option<bool>> {
        self.avaliar(resolver)?.como_bool()
    }

    /// Avalia sobre uma linha ja em `Value`s, na ordem do esquema.
    pub fn avaliar_linha(
        &self,
        nomes: &[&str],
        tipos: &[ColumnType],
        valores: &[Value],
    ) -> Result<Valor> {
        self.avaliar(&|nome| {
            let i = nomes.iter().position(|n| n.eq_ignore_ascii_case(nome))?;
            Some((valores.get(i)?, tipos.get(i)?))
        })
    }
}

fn colher_colunas(e: &Expr, saida: &mut Vec<String>) {
    match e {
        Expr::Lit(_) => {}
        Expr::Coluna(c) => {
            if !saida.iter().any(|s| s.eq_ignore_ascii_case(c)) {
                saida.push(c.clone());
            }
        }
        Expr::Negativo(a) | Expr::Nao(a) => colher_colunas(a, saida),
        Expr::Bin(_, a, b) => {
            colher_colunas(a, saida);
            colher_colunas(b, saida);
        }
        Expr::ENulo { e, .. } => colher_colunas(e, saida),
        Expr::Em { e, lista, .. } => {
            colher_colunas(e, saida);
            lista.iter().for_each(|x| colher_colunas(x, saida));
        }
        Expr::Entre { e, de, ate, .. } => {
            colher_colunas(e, saida);
            colher_colunas(de, saida);
            colher_colunas(ate, saida);
        }
        Expr::Parece { e, padrao, .. } => {
            colher_colunas(e, saida);
            colher_colunas(padrao, saida);
        }
        Expr::Chamada(_, args) => args.iter().for_each(|x| colher_colunas(x, saida)),
    }
}

// ------------------------------------------------------------------- lexico

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Numero(Numero),
    Texto(String),
    Palavra(String),
    Op(&'static str),
    Abre,
    Fecha,
    Virgula,
}

impl Token {
    fn mostrar(&self) -> String {
        match self {
            Token::Numero(n) => n.texto(),
            Token::Texto(t) => format!("'{t}'"),
            Token::Palavra(p) => p.clone(),
            Token::Op(o) => o.to_string(),
            Token::Abre => "(".into(),
            Token::Fecha => ")".into(),
            Token::Virgula => ",".into(),
        }
    }
}

fn lexer(texto: &str) -> Result<Vec<Token>> {
    let c: Vec<char> = texto.chars().collect();
    let mut i = 0;
    let mut saida = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let ini = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            let s: String = c[ini..i].iter().collect();
            saida.push(Token::Numero(numero_de_texto(&s)?));
            continue;
        }
        if ch == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                match c.get(i) {
                    None => {
                        return Err(PhxError::Esquema(format!(
                            "texto sem fechar na expressao: {texto:?}"
                        )))
                    }
                    Some('\'') if c.get(i + 1) == Some(&'\'') => {
                        s.push('\'');
                        i += 2;
                    }
                    Some('\'') => {
                        i += 1;
                        break;
                    }
                    Some(x) => {
                        s.push(*x);
                        i += 1;
                    }
                }
            }
            saida.push(Token::Texto(s));
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let ini = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            // Nome QUALIFICADO (`p.id`, `clientes.nome`): um token so, com o
            // ponto dentro. E a juncao que precisa dele -- duas tabelas na
            // mesma linha tem duas colunas `id` --, e quem resolve o nome
            // (o servidor) e quem sabe o que o prefixo quer dizer. Numa
            // expressao de esquema o resolvedor nao acha `p.id`, e a recusa
            // ja diz «coluna que a tabela nao tem».
            if c.get(i) == Some(&'.')
                && c.get(i + 1).is_some_and(|x| x.is_alphabetic() || *x == '_')
            {
                i += 1;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
            }
            saida.push(Token::Palavra(c[ini..i].iter().collect()));
            continue;
        }
        let dois: String = c[i..(i + 2).min(c.len())].iter().collect();
        let op = match dois.as_str() {
            "<>" | "!=" | "<=" | ">=" => Some(match dois.as_str() {
                "<>" | "!=" => "<>",
                "<=" => "<=",
                _ => ">=",
            }),
            _ => None,
        };
        if let Some(o) = op {
            saida.push(Token::Op(o));
            i += 2;
            continue;
        }
        let um = match ch {
            '=' => Some("="),
            '<' => Some("<"),
            '>' => Some(">"),
            '+' => Some("+"),
            '-' => Some("-"),
            '*' => Some("*"),
            '/' => Some("/"),
            _ => None,
        };
        if let Some(o) = um {
            saida.push(Token::Op(o));
            i += 1;
            continue;
        }
        match ch {
            '(' => saida.push(Token::Abre),
            ')' => saida.push(Token::Fecha),
            ',' => saida.push(Token::Virgula),
            outro => {
                return Err(PhxError::Esquema(format!(
                    "caractere {outro:?} nao tem lugar numa expressao ({texto:?})"
                )))
            }
        }
        i += 1;
    }
    Ok(saida)
}

fn numero_de_texto(s: &str) -> Result<Numero> {
    let invalido = || PhxError::Esquema(format!("numero invalido: {s:?}"));
    match s.split_once('.') {
        None => Ok(Numero::Inteiro(s.parse().map_err(|_| invalido())?)),
        Some((_, fracao)) => {
            if fracao.contains('.') || fracao.is_empty() {
                return Err(invalido());
            }
            let escala = u8::try_from(fracao.len()).map_err(|_| invalido())?;
            Ok(Numero::Decimal {
                valor: texto_para_decimal(s, escala).map_err(|_| invalido())?,
                escala,
            })
        }
    }
}

// ------------------------------------------------------------------ sintaxe

struct Parser<'t> {
    tokens: Vec<Token>,
    i: usize,
    texto: &'t str,
}

impl Parser<'_> {
    fn erro(&self, msg: &str) -> PhxError {
        PhxError::Esquema(format!("expressao {:?}: {msg}", self.texto.trim()))
    }

    fn espiar(&self) -> Option<&Token> {
        self.tokens.get(self.i)
    }

    fn palavra_e(&self, p: &str) -> bool {
        matches!(self.espiar(), Some(Token::Palavra(x)) if x.eq_ignore_ascii_case(p))
    }

    fn aceitar_palavra(&mut self, p: &str) -> bool {
        if self.palavra_e(p) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn aceitar_op(&mut self, o: &str) -> bool {
        if matches!(self.espiar(), Some(Token::Op(x)) if *x == o) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn exigir(&mut self, t: Token, o_que: &str) -> Result<()> {
        if self.espiar() == Some(&t) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.erro(&format!(
                "esperava {o_que}, e veio {}",
                self.espiar()
                    .map(Token::mostrar)
                    .unwrap_or_else(|| "o fim".into())
            )))
        }
    }

    fn ou(&mut self) -> Result<Expr> {
        let mut e = self.e()?;
        while self.aceitar_palavra("OR") {
            let d = self.e()?;
            e = Expr::Bin(Op::Ou, Box::new(e), Box::new(d));
        }
        Ok(e)
    }

    fn e(&mut self) -> Result<Expr> {
        let mut e = self.nao()?;
        while self.aceitar_palavra("AND") {
            let d = self.nao()?;
            e = Expr::Bin(Op::E, Box::new(e), Box::new(d));
        }
        Ok(e)
    }

    fn nao(&mut self) -> Result<Expr> {
        if self.aceitar_palavra("NOT") {
            return Ok(Expr::Nao(Box::new(self.nao()?)));
        }
        self.comparacao()
    }

    fn comparacao(&mut self) -> Result<Expr> {
        let e = self.soma()?;
        // Um so comparador por nivel: `a < b < c` e ambiguo e recusa.
        if self.aceitar_palavra("IS") {
            let negado = self.aceitar_palavra("NOT");
            if !self.aceitar_palavra("NULL") {
                return Err(self.erro("IS so se usa como IS NULL ou IS NOT NULL"));
            }
            return Ok(Expr::ENulo {
                e: Box::new(e),
                negado,
            });
        }
        let negado = self.aceitar_palavra("NOT");
        if self.aceitar_palavra("IN") {
            self.exigir(Token::Abre, "( depois de IN")?;
            let mut lista = Vec::new();
            loop {
                lista.push(self.soma()?);
                if self.aceitar_virgula() {
                    continue;
                }
                self.exigir(Token::Fecha, ") fechando o IN")?;
                break;
            }
            return Ok(Expr::Em {
                e: Box::new(e),
                lista,
                negado,
            });
        }
        if self.aceitar_palavra("BETWEEN") {
            let de = self.soma()?;
            if !self.aceitar_palavra("AND") {
                return Err(self.erro("BETWEEN pede `a AND b`"));
            }
            let ate = self.soma()?;
            return Ok(Expr::Entre {
                e: Box::new(e),
                de: Box::new(de),
                ate: Box::new(ate),
                negado,
            });
        }
        if self.aceitar_palavra("LIKE") {
            let padrao = self.soma()?;
            return Ok(Expr::Parece {
                e: Box::new(e),
                padrao: Box::new(padrao),
                negado,
            });
        }
        if negado {
            return Err(self.erro("NOT aqui so se usa antes de IN, BETWEEN ou LIKE"));
        }
        let op = match self.espiar() {
            Some(Token::Op("=")) => Op::Igual,
            Some(Token::Op("<>")) => Op::Diferente,
            Some(Token::Op("<")) => Op::Menor,
            Some(Token::Op("<=")) => Op::MenorIgual,
            Some(Token::Op(">")) => Op::Maior,
            Some(Token::Op(">=")) => Op::MaiorIgual,
            _ => return Ok(e),
        };
        self.i += 1;
        let d = self.soma()?;
        Ok(Expr::Bin(op, Box::new(e), Box::new(d)))
    }

    fn aceitar_virgula(&mut self) -> bool {
        if self.espiar() == Some(&Token::Virgula) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn soma(&mut self) -> Result<Expr> {
        let mut e = self.produto()?;
        loop {
            let op = if self.aceitar_op("+") {
                Op::Mais
            } else if self.aceitar_op("-") {
                Op::Menos
            } else {
                return Ok(e);
            };
            let d = self.produto()?;
            e = Expr::Bin(op, Box::new(e), Box::new(d));
        }
    }

    fn produto(&mut self) -> Result<Expr> {
        let mut e = self.unario()?;
        loop {
            let op = if self.aceitar_op("*") {
                Op::Vezes
            } else if self.aceitar_op("/") {
                Op::Dividir
            } else {
                return Ok(e);
            };
            let d = self.unario()?;
            e = Expr::Bin(op, Box::new(e), Box::new(d));
        }
    }

    fn unario(&mut self) -> Result<Expr> {
        if self.aceitar_op("-") {
            return Ok(Expr::Negativo(Box::new(self.unario()?)));
        }
        self.primario()
    }

    fn primario(&mut self) -> Result<Expr> {
        let t = match self.espiar() {
            Some(t) => t.clone(),
            None => return Err(self.erro("terminou onde faltava um valor")),
        };
        self.i += 1;
        Ok(match t {
            Token::Numero(n) => Expr::Lit(Valor::Num(n)),
            Token::Texto(s) => Expr::Lit(Valor::Texto(s)),
            Token::Abre => {
                let e = self.ou()?;
                self.exigir(Token::Fecha, ")")?;
                e
            }
            Token::Palavra(p) => {
                if p.eq_ignore_ascii_case("NULL") {
                    Expr::Lit(Valor::Nulo)
                } else if p.eq_ignore_ascii_case("TRUE") {
                    Expr::Lit(Valor::Bool(true))
                } else if p.eq_ignore_ascii_case("FALSE") {
                    Expr::Lit(Valor::Bool(false))
                } else if self.espiar() == Some(&Token::Abre) {
                    self.i += 1;
                    let f = Funcao::de_nome(&p).ok_or_else(|| {
                        self.erro(&format!(
                            "funcao {p} nao existe (as que existem: {FUNCOES})"
                        ))
                    })?;
                    let mut args = Vec::new();
                    if self.espiar() != Some(&Token::Fecha) {
                        loop {
                            args.push(self.ou()?);
                            if !self.aceitar_virgula() {
                                break;
                            }
                        }
                    }
                    self.exigir(Token::Fecha, ") fechando a funcao")?;
                    Expr::Chamada(f, args)
                } else if ["AND", "OR", "NOT", "IN", "IS", "BETWEEN", "LIKE"]
                    .iter()
                    .any(|k| p.eq_ignore_ascii_case(k))
                {
                    return Err(self.erro(&format!("{p} apareceu onde faltava um valor")));
                } else {
                    Expr::Coluna(p)
                }
            }
            outro => {
                return Err(self.erro(&format!(
                    "{} apareceu onde faltava um valor",
                    outro.mostrar()
                )))
            }
        })
    }
}

// ---------------------------------------------------------------- avaliacao

fn avaliar<'a>(e: &Expr, r: &dyn Fn(&str) -> Option<(&'a Value, &'a ColumnType)>) -> Result<Valor> {
    Ok(match e {
        Expr::Lit(v) => v.clone(),
        Expr::Coluna(nome) => match r(nome) {
            Some((v, ty)) => Valor::de_value(v, ty),
            None => {
                return Err(PhxError::Esquema(format!(
                    "a expressao usa a coluna {nome:?}, que a tabela nao tem"
                )))
            }
        },
        Expr::Negativo(a) => match avaliar(a, r)? {
            Valor::Nulo => Valor::Nulo,
            Valor::Num(n) => Valor::Num(match n {
                Numero::Inteiro(i) => Numero::Inteiro(
                    i.checked_neg()
                        .ok_or_else(|| PhxError::LimiteExcedido("numero grande demais".into()))?,
                ),
                Numero::Decimal { valor, escala } => Numero::Decimal {
                    valor: -valor,
                    escala,
                },
                Numero::Real(x) => Numero::Real(-x),
            }),
            outro => {
                return Err(PhxError::Tipo(format!(
                    "o sinal de menos precisa de numero e recebeu {}",
                    outro.descricao()
                )))
            }
        },
        Expr::Nao(a) => match avaliar(a, r)?.como_bool()? {
            None => Valor::Nulo,
            Some(b) => Valor::Bool(!b),
        },
        Expr::Bin(Op::E, a, b) => {
            // Tres valores, e curto-circuito so onde o SQL permite: FALSE AND
            // qualquer coisa e FALSE, mesmo com NULL do outro lado.
            let x = avaliar(a, r)?.como_bool()?;
            if x == Some(false) {
                return Ok(Valor::Bool(false));
            }
            let y = avaliar(b, r)?.como_bool()?;
            match (x, y) {
                (_, Some(false)) => Valor::Bool(false),
                (Some(true), Some(true)) => Valor::Bool(true),
                _ => Valor::Nulo,
            }
        }
        Expr::Bin(Op::Ou, a, b) => {
            let x = avaliar(a, r)?.como_bool()?;
            if x == Some(true) {
                return Ok(Valor::Bool(true));
            }
            let y = avaliar(b, r)?.como_bool()?;
            match (x, y) {
                (_, Some(true)) => Valor::Bool(true),
                (Some(false), Some(false)) => Valor::Bool(false),
                _ => Valor::Nulo,
            }
        }
        Expr::Bin(op, a, b) => {
            let x = avaliar(a, r)?;
            let y = avaliar(b, r)?;
            if x == Valor::Nulo || y == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            match op {
                Op::Mais | Op::Menos | Op::Vezes | Op::Dividir => Valor::Num(aritmetica(
                    *op,
                    x.como_numero("a conta")?,
                    y.como_numero("a conta")?,
                )?),
                _ => Valor::Bool(comparar(*op, &x, &y)?),
            }
        }
        Expr::ENulo { e, negado } => {
            let nulo = avaliar(e, r)? == Valor::Nulo;
            Valor::Bool(nulo != *negado)
        }
        Expr::Em { e, lista, negado } => {
            let x = avaliar(e, r)?;
            if x == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            let mut viu_nulo = false;
            for item in lista {
                let y = avaliar(item, r)?;
                if y == Valor::Nulo {
                    viu_nulo = true;
                    continue;
                }
                if comparar(Op::Igual, &x, &y)? {
                    return Ok(Valor::Bool(!*negado));
                }
            }
            if viu_nulo {
                Valor::Nulo
            } else {
                Valor::Bool(*negado)
            }
        }
        Expr::Entre { e, de, ate, negado } => {
            let x = avaliar(e, r)?;
            let a = avaliar(de, r)?;
            let b = avaliar(ate, r)?;
            if x == Valor::Nulo || a == Valor::Nulo || b == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            let dentro = comparar(Op::MaiorIgual, &x, &a)? && comparar(Op::MenorIgual, &x, &b)?;
            Valor::Bool(dentro != *negado)
        }
        Expr::Parece { e, padrao, negado } => {
            let x = avaliar(e, r)?;
            let p = avaliar(padrao, r)?;
            if x == Valor::Nulo || p == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            let (Valor::Texto(t), Valor::Texto(p)) = (&x, &p) else {
                return Err(PhxError::Tipo(format!(
                    "LIKE compara texto com texto; veio {} e {}",
                    x.descricao(),
                    p.descricao()
                )));
            };
            Valor::Bool(parece(t, p) != *negado)
        }
        Expr::Chamada(f, args) => {
            let mut vs = Vec::with_capacity(args.len());
            for a in args {
                vs.push(avaliar(a, r)?);
            }
            chamar(*f, vs)?
        }
    })
}

fn aritmetica(op: Op, a: Numero, b: Numero) -> Result<Numero> {
    let estouro = || PhxError::LimiteExcedido("a conta estourou o numero".into());
    if op == Op::Dividir {
        let d = b.como_f64();
        if d == 0.0 {
            return Err(PhxError::Tipo("divisao por zero".into()));
        }
        return Ok(Numero::Real(a.como_f64() / d));
    }
    match (a, b) {
        (Numero::Real(_), _) | (_, Numero::Real(_)) => {
            let (x, y) = (a.como_f64(), b.como_f64());
            Ok(Numero::Real(match op {
                Op::Mais => x + y,
                Op::Menos => x - y,
                _ => x * y,
            }))
        }
        (Numero::Inteiro(x), Numero::Inteiro(y)) => Ok(Numero::Inteiro(
            match op {
                Op::Mais => x.checked_add(y),
                Op::Menos => x.checked_sub(y),
                _ => x.checked_mul(y),
            }
            .ok_or_else(estouro)?,
        )),
        _ => {
            if op == Op::Vezes {
                // Escalas somam: 1.10 * 2.5 = 2.750.
                let (va, ea) = match a {
                    Numero::Inteiro(i) => (i, 0u8),
                    Numero::Decimal { valor, escala } => (valor, escala),
                    Numero::Real(_) => unreachable!("real ja saiu acima"),
                };
                let (vb, eb) = match b {
                    Numero::Inteiro(i) => (i, 0u8),
                    Numero::Decimal { valor, escala } => (valor, escala),
                    Numero::Real(_) => unreachable!("real ja saiu acima"),
                };
                let escala = ea.checked_add(eb).ok_or_else(estouro)?;
                if escala > 38 {
                    return Err(estouro());
                }
                return Ok(Numero::Decimal {
                    valor: va.checked_mul(vb).ok_or_else(estouro)?,
                    escala,
                });
            }
            let (x, y, escala) = Numero::alinhar(a, b).ok_or_else(estouro)?;
            let valor = match op {
                Op::Mais => x.checked_add(y),
                _ => x.checked_sub(y),
            }
            .ok_or_else(estouro)?;
            Ok(Numero::Decimal { valor, escala })
        }
    }
}

fn comparar(op: Op, a: &Valor, b: &Valor) -> Result<bool> {
    use std::cmp::Ordering;
    let ord = match (a, b) {
        (Valor::Num(x), Valor::Num(y)) => match Numero::alinhar(*x, *y) {
            Some((p, q, _)) => p.cmp(&q),
            None => x
                .como_f64()
                .partial_cmp(&y.como_f64())
                .ok_or_else(|| PhxError::Tipo("comparacao com numero invalido (NaN)".into()))?,
        },
        (Valor::Texto(x), Valor::Texto(y)) => x.as_str().cmp(y.as_str()),
        (Valor::Bool(x), Valor::Bool(y)) => x.cmp(y),
        _ => {
            return Err(PhxError::Tipo(format!(
                "nao da para comparar {} com {}",
                a.descricao(),
                b.descricao()
            )))
        }
    };
    Ok(match op {
        Op::Igual => ord == Ordering::Equal,
        Op::Diferente => ord != Ordering::Equal,
        Op::Menor => ord == Ordering::Less,
        Op::MenorIgual => ord != Ordering::Greater,
        Op::Maior => ord == Ordering::Greater,
        Op::MaiorIgual => ord != Ordering::Less,
        _ => unreachable!("comparar so recebe comparador"),
    })
}

/// `%` casa qualquer sequencia, `_` um caractere; o resto e literal e
/// distingue caixa -- quem quer sem caixa escreve `LOWER(x) LIKE 'a%'`.
fn parece(texto: &str, padrao: &str) -> bool {
    fn casa(t: &[char], p: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('%') => (0..=t.len()).any(|k| casa(&t[k..], &p[1..])),
            Some('_') => !t.is_empty() && casa(&t[1..], &p[1..]),
            Some(c) => t.first() == Some(c) && casa(&t[1..], &p[1..]),
        }
    }
    let t: Vec<char> = texto.chars().collect();
    let p: Vec<char> = padrao.chars().collect();
    casa(&t, &p)
}

fn chamar(f: Funcao, args: Vec<Valor>) -> Result<Valor> {
    let exigir = |n: usize| -> Result<()> {
        if args.len() != n {
            return Err(PhxError::Esquema(format!(
                "{f:?} recebe {n} argumento(s) e veio {}",
                args.len()
            )));
        }
        Ok(())
    };
    Ok(match f {
        Funcao::Concatenar => {
            if args.is_empty() {
                return Err(PhxError::Esquema("CONCAT sem argumentos".into()));
            }
            if args.contains(&Valor::Nulo) {
                return Ok(Valor::Nulo);
            }
            Valor::Texto(args.iter().map(Valor::como_texto).collect())
        }
        Funcao::Maiusculas | Funcao::Minusculas | Funcao::Aparar | Funcao::Comprimento => {
            exigir(1)?;
            if args[0] == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            let t = args[0].como_texto();
            match f {
                Funcao::Maiusculas => Valor::Texto(t.to_uppercase()),
                Funcao::Minusculas => Valor::Texto(t.to_lowercase()),
                Funcao::Aparar => Valor::Texto(t.trim().to_string()),
                _ => Valor::Num(Numero::Inteiro(t.chars().count() as i128)),
            }
        }
        Funcao::Absoluto => {
            exigir(1)?;
            if args[0] == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            Valor::Num(match args[0].como_numero("ABS")? {
                Numero::Inteiro(i) => Numero::Inteiro(i.abs()),
                Numero::Decimal { valor, escala } => Numero::Decimal {
                    valor: valor.abs(),
                    escala,
                },
                Numero::Real(x) => Numero::Real(x.abs()),
            })
        }
        Funcao::Arredondar => {
            if args.is_empty() || args.len() > 2 {
                return Err(PhxError::Esquema("ROUND(n) ou ROUND(n, casas)".into()));
            }
            if args[0] == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            let n = args[0].como_numero("ROUND")?;
            let casas = match args.get(1) {
                None => 0u8,
                Some(v) => match v.como_numero("as casas do ROUND")? {
                    Numero::Inteiro(c) if (0..=30).contains(&c) => c as u8,
                    _ => return Err(PhxError::Esquema("ROUND aceita de 0 a 30 casas".into())),
                },
            };
            Valor::Num(arredondar(n, casas)?)
        }
        Funcao::Coalescer => {
            if args.is_empty() {
                return Err(PhxError::Esquema("COALESCE sem argumentos".into()));
            }
            args.into_iter()
                .find(|a| *a != Valor::Nulo)
                .unwrap_or(Valor::Nulo)
        }
    })
}

/// Meio para longe do zero, como o ROUND dos outros motores.
fn arredondar(n: Numero, casas: u8) -> Result<Numero> {
    Ok(match n {
        Numero::Inteiro(_) => n,
        Numero::Real(x) => {
            let f = 10f64.powi(casas as i32);
            Numero::Real((x * f).round() / f)
        }
        Numero::Decimal { valor, escala } => {
            if casas >= escala {
                return Ok(n);
            }
            let div = pot10(escala - casas)
                .ok_or_else(|| PhxError::LimiteExcedido("ROUND estourou".into()))?;
            let (q, r) = (valor / div, valor % div);
            let meio = div / 2;
            let ajuste = if r.abs() >= meio { valor.signum() } else { 0 };
            Numero::Decimal {
                valor: q + ajuste,
                escala: casas,
            }
        }
    })
}

// ------------------------------------------------------------------ coercao

/// Do resultado da conta para o valor que a coluna grava. Nada aqui
/// arredonda nem trunca em silencio: decimal com casas demais recusa
/// mandando usar `ROUND`, e real numa coluna inteira recusa do mesmo jeito.
pub fn coagir(v: &Valor, ty: &ColumnType) -> Result<Value> {
    let recusa = |o_que: &str| {
        PhxError::Tipo(format!(
            "a expressao devolveu {} e a coluna e {ty:?}: {o_que}",
            v.descricao()
        ))
    };
    Ok(match (v, ty) {
        (Valor::Nulo, _) => Value::Null,
        (
            Valor::Num(n),
            ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8,
        )
        | (
            Valor::Num(n),
            ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8,
        )
        | (Valor::Num(n), ColumnType::Sequence) => {
            let i = inteiro_exato(*n).ok_or_else(|| recusa("nao e inteiro; use ROUND"))?;
            if matches!(
                ty,
                ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8
            ) {
                Value::Int(i64::try_from(i).map_err(|_| recusa("nao cabe"))?)
            } else {
                Value::UInt(u64::try_from(i).map_err(|_| recusa("e negativo ou nao cabe"))?)
            }
        }
        (Valor::Num(n), ColumnType::Real4 | ColumnType::Real8) => Value::Real(n.como_f64()),
        (Valor::Num(n), ColumnType::Decimal { escala, .. }) => Value::Decimal(match *n {
            Numero::Inteiro(i) => i
                .checked_mul(pot10(*escala).ok_or_else(|| recusa("nao cabe"))?)
                .ok_or_else(|| recusa("nao cabe"))?,
            Numero::Decimal { valor, escala: e } => {
                if e > *escala {
                    return Err(recusa(&format!(
                        "tem {e} casas e a coluna {escala}; use ROUND(..., {escala})"
                    )));
                }
                valor
                    .checked_mul(pot10(*escala - e).ok_or_else(|| recusa("nao cabe"))?)
                    .ok_or_else(|| recusa("nao cabe"))?
            }
            Numero::Real(_) => return Err(recusa("e real; use ROUND")),
        }),
        (Valor::Bool(b), ColumnType::Bool) => Value::Bool(*b),
        (Valor::Num(n), ColumnType::Bool) => Value::Bool(n.como_f64() != 0.0),
        (Valor::Texto(t), ColumnType::Str(_)) => Value::Str(t.clone()),
        (Valor::Num(n), ColumnType::Str(_)) => Value::Str(n.texto()),
        (Valor::Texto(t), ColumnType::Memo) => Value::Memo(t.clone()),
        // Data, hora, instante e identificador: o texto ISO, pelo mesmo
        // conversor da importacao -- um conversor, nao dois.
        (Valor::Texto(t), ColumnType::Date | ColumnType::Time | ColumnType::DateTime)
        | (Valor::Texto(t), ColumnType::Uuid | ColumnType::Uuid256) => valor_de_texto(t, ty)?,
        _ => return Err(recusa("tipos incompativeis")),
    })
}

fn inteiro_exato(n: Numero) -> Option<i128> {
    match n {
        Numero::Inteiro(i) => Some(i),
        Numero::Decimal { valor, escala } => {
            let div = pot10(escala)?;
            (valor % div == 0).then_some(valor / div)
        }
        Numero::Real(x) => (x.fract() == 0.0 && x.abs() < 1e30).then_some(x as i128),
    }
}

// ------------------------------------------------------------------- testes

#[cfg(test)]
mod testes {
    use super::*;

    fn linha<'a>(
        cols: &'a [(&'a str, ColumnType, Value)],
    ) -> impl Fn(&str) -> Option<(&'a Value, &'a ColumnType)> + 'a {
        move |nome| {
            cols.iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(nome))
                .map(|(_, ty, v)| (v, ty))
        }
    }

    fn dec(t: &str, escala: u8) -> Value {
        Value::Decimal(texto_para_decimal(t, escala).unwrap())
    }

    fn avalia(texto: &str, cols: &[(&str, ColumnType, Value)]) -> Valor {
        Expressao::analisar(texto)
            .unwrap()
            .avaliar(&linha(cols))
            .unwrap()
    }

    fn booleano(texto: &str, cols: &[(&str, ColumnType, Value)]) -> Option<bool> {
        Expressao::analisar(texto)
            .unwrap()
            .avaliar_bool(&linha(cols))
            .unwrap()
    }

    #[test]
    fn a_sonda_do_comparativo_e_exata_em_decimal() {
        // preco * 1.1 > 100, com preco Decimal(12,2): 91.00 * 1.1 = 100.100
        let preco = |t: &str| {
            vec![(
                "preco",
                ColumnType::Decimal {
                    precisao: 12,
                    escala: 2,
                },
                dec(t, 2),
            )]
        };
        assert_eq!(booleano("preco * 1.1 > 100", &preco("91.00")), Some(true));
        assert_eq!(booleano("preco * 1.1 > 100", &preco("90.90")), Some(false));
        // 90.91 * 1.1 = 100.001 -- e um f64 daria 100.00099999 e continuaria certo
        // por sorte; o decimal e certo por construcao.
        assert_eq!(booleano("preco * 1.1 > 100", &preco("90.91")), Some(true));
        assert_eq!(
            avalia("preco * 1.1", &preco("90.91")),
            Valor::Num(Numero::Decimal {
                valor: 100001,
                escala: 3
            })
        );
    }

    #[test]
    fn precedencia_e_a_do_sql() {
        let v = &[
            ("a", ColumnType::Int8, Value::Int(2)),
            ("b", ColumnType::Int8, Value::Int(3)),
        ];
        assert_eq!(avalia("a + b * 2", v), Valor::Num(Numero::Inteiro(8)));
        assert_eq!(avalia("(a + b) * 2", v), Valor::Num(Numero::Inteiro(10)));
        assert_eq!(avalia("-a + b", v), Valor::Num(Numero::Inteiro(1)));
        assert_eq!(booleano("a = 2 OR b = 9 AND a = 1", v), Some(true));
        assert_eq!(booleano("NOT a = 2 OR b = 3", v), Some(true));
        assert_eq!(booleano("NOT (a = 2 OR b = 3)", v), Some(false));
        assert_eq!(booleano("a / b < 1", v), Some(true));
    }

    #[test]
    fn nulo_propaga_e_o_check_decide_o_que_fazer_com_ele() {
        let v = &[
            ("v", ColumnType::Int8, Value::Null),
            ("w", ColumnType::Int8, Value::Int(5)),
        ];
        assert_eq!(booleano("v > 0", v), None);
        assert_eq!(booleano("v IS NULL", v), Some(true));
        assert_eq!(booleano("v IS NOT NULL", v), Some(false));
        assert_eq!(booleano("v > 0 AND w > 0", v), None);
        assert_eq!(booleano("v > 0 AND w < 0", v), Some(false));
        assert_eq!(booleano("v > 0 OR w > 0", v), Some(true));
        assert_eq!(booleano("NOT (v > 0)", v), None);
        assert_eq!(
            avalia("COALESCE(v, w, 1)", v),
            Valor::Num(Numero::Inteiro(5))
        );
        assert_eq!(avalia("v + 1", v), Valor::Nulo);
        assert_eq!(avalia("CONCAT('a', v)", v), Valor::Nulo);
    }

    #[test]
    fn in_between_like_com_e_sem_not() {
        let v = &[
            ("cidade", ColumnType::Str(40), Value::Str("Blumenau".into())),
            ("n", ColumnType::Int8, Value::Int(7)),
        ];
        assert_eq!(
            booleano("cidade IN ('Joinville', 'Blumenau')", v),
            Some(true)
        );
        assert_eq!(
            booleano("cidade NOT IN ('Joinville', 'Blumenau')", v),
            Some(false)
        );
        assert_eq!(booleano("n IN (1, 2, NULL)", v), None);
        assert_eq!(booleano("n BETWEEN 5 AND 7", v), Some(true));
        assert_eq!(booleano("n NOT BETWEEN 5 AND 7", v), Some(false));
        assert_eq!(booleano("cidade LIKE 'Blu%'", v), Some(true));
        assert_eq!(booleano("cidade LIKE 'blu%'", v), Some(false));
        assert_eq!(booleano("LOWER(cidade) LIKE 'blu%'", v), Some(true));
        assert_eq!(booleano("cidade LIKE 'B_umenau'", v), Some(true));
        assert_eq!(booleano("cidade NOT LIKE '%x%'", v), Some(true));
        assert_eq!(booleano("cidade LIKE '%'", v), Some(true));
    }

    #[test]
    fn funcoes_com_a_semantica_dos_gatilhos() {
        let v = &[("s", ColumnType::Str(40), Value::Str("  Ção  ".into()))];
        assert_eq!(avalia("UPPER(TRIM(s))", v), Valor::Texto("ÇÃO".into()));
        assert_eq!(avalia("LENGTH(TRIM(s))", v), Valor::Num(Numero::Inteiro(3)));
        assert_eq!(
            avalia("CONCAT(TRIM(s), '-', 1)", v),
            Valor::Texto("Ção-1".into())
        );
        assert_eq!(
            avalia("ABS(-2.5)", v),
            Valor::Num(Numero::Decimal {
                valor: 25,
                escala: 1
            })
        );
        assert_eq!(
            avalia("ROUND(2.567, 2)", v),
            Valor::Num(Numero::Decimal {
                valor: 257,
                escala: 2
            })
        );
        assert_eq!(
            avalia("ROUND(-2.565, 2)", v),
            Valor::Num(Numero::Decimal {
                valor: -257,
                escala: 2
            })
        );
        assert_eq!(
            avalia("ROUND(2.4)", v),
            Valor::Num(Numero::Decimal {
                valor: 2,
                escala: 0
            })
        );
        let e = Expressao::analisar("FOO(1)").unwrap_err().to_string();
        assert!(e.contains("funcao FOO nao existe"), "{e}");
    }

    #[test]
    fn as_colunas_referidas_saem_para_a_declaracao_recusar_cedo() {
        let e = Expressao::analisar("a * 2 + b > COALESCE(c, A)").unwrap();
        assert_eq!(e.colunas(), &["a", "b", "c"]);
        let erro = e
            .avaliar(&linha(&[("a", ColumnType::Int8, Value::Int(1))]))
            .unwrap_err()
            .to_string();
        assert!(erro.contains("coluna \"b\""), "{erro}");
    }

    #[test]
    fn coagir_nao_arredonda_calado() {
        let d2 = ColumnType::Decimal {
            precisao: 12,
            escala: 2,
        };
        assert_eq!(
            coagir(&Valor::Num(Numero::Inteiro(7)), &d2).unwrap(),
            Value::Decimal(700)
        );
        assert_eq!(
            coagir(
                &Valor::Num(Numero::Decimal {
                    valor: 15,
                    escala: 1
                }),
                &d2
            )
            .unwrap(),
            Value::Decimal(150)
        );
        let e = coagir(
            &Valor::Num(Numero::Decimal {
                valor: 100001,
                escala: 3,
            }),
            &d2,
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("use ROUND(..., 2)"), "{e}");
        assert_eq!(
            coagir(
                &Valor::Num(Numero::Decimal {
                    valor: 60,
                    escala: 1
                }),
                &ColumnType::Int8
            )
            .unwrap(),
            Value::Int(6)
        );
        assert!(coagir(
            &Valor::Num(Numero::Decimal {
                valor: 65,
                escala: 1
            }),
            &ColumnType::Int8
        )
        .is_err());
        assert!(coagir(&Valor::Num(Numero::Inteiro(-1)), &ColumnType::UInt4).is_err());
        assert_eq!(
            coagir(&Valor::Texto("x".into()), &ColumnType::Str(4)).unwrap(),
            Value::Str("x".into())
        );
        assert_eq!(
            coagir(&Valor::Texto("2026-09-08".into()), &ColumnType::Date).unwrap(),
            Value::Date(20704)
        );
        assert_eq!(
            coagir(&Valor::Nulo, &ColumnType::Int8).unwrap(),
            Value::Null
        );
        assert!(coagir(&Valor::Texto("x".into()), &ColumnType::Int8).is_err());
    }

    #[test]
    fn erros_de_sintaxe_dizem_onde() {
        for (t, trecho) in [
            ("", "expressao vazia"),
            ("a +", "terminou onde faltava um valor"),
            ("a < b < c", "sobrou <"),
            ("a IS 1", "IS NULL ou IS NOT NULL"),
            ("a IN 1", "( depois de IN"),
            ("a BETWEEN 1", "BETWEEN pede"),
            ("'aberto", "texto sem fechar"),
            ("a # 1", "caractere '#'"),
            ("1.2.3", "numero invalido"),
            ("(a", "esperava )"),
            ("AND a", "AND apareceu onde faltava um valor"),
        ] {
            let e = Expressao::analisar(t).unwrap_err().to_string();
            assert!(e.contains(trecho), "{t:?} -> {e}");
        }
    }

    #[test]
    fn texto_e_data_comparam_por_bytes_e_tipos_trocados_recusam() {
        let v = &[
            ("d", ColumnType::Date, Value::Date(20704)),
            ("n", ColumnType::Int8, Value::Int(1)),
        ];
        assert_eq!(booleano("d >= '2026-09-01'", v), Some(true));
        assert_eq!(booleano("d = '2026-09-08'", v), Some(true));
        let e = Expressao::analisar("n = 'x'")
            .unwrap()
            .avaliar(&linha(v))
            .unwrap_err()
            .to_string();
        assert!(e.contains("nao da para comparar"), "{e}");
        let e = Expressao::analisar("n / 0")
            .unwrap()
            .avaliar(&linha(v))
            .unwrap_err()
            .to_string();
        assert!(e.contains("divisao por zero"), "{e}");
    }

    /// A juncao poe duas tabelas na mesma linha, e `p.id` e `c.id` sao
    /// colunas diferentes: o nome qualificado e um token so.
    #[test]
    fn nome_qualificado_e_uma_coluna_so() {
        let e = Expressao::analisar("p.total > 100 AND c.cidade = 'Blumenau'").unwrap();
        assert_eq!(e.colunas(), &["p.total", "c.cidade"]);
        let v = &[
            ("p.total", ColumnType::Int8, Value::Int(150)),
            (
                "c.cidade",
                ColumnType::Str(40),
                Value::Str("Blumenau".into()),
            ),
        ];
        assert_eq!(e.avaliar_bool(&linha(v)).unwrap(), Some(true));
        // `1.5` continua numero, e `p.5` nao e nome.
        assert_eq!(
            Expressao::analisar("1.5").unwrap().colunas(),
            &[] as &[String]
        );
        assert!(Expressao::analisar("p.5").is_err());
    }

    #[test]
    fn o_texto_guardado_e_o_que_veio_aparado() {
        let e = Expressao::analisar("  v > 0  ").unwrap();
        assert_eq!(e.texto(), "v > 0");
        assert_eq!(Expressao::analisar("v>0").unwrap().raiz, e.raiz);
    }
}

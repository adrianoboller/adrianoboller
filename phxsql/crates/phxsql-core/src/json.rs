//! Leitor e escritor de JSON, sem dependencias externas.
//!
//! Existe para que o `config.json`, o protocolo do servidor e o log de acessos
//! nao obriguem o projeto a puxar uma crate de fora. E o que mantem o PhxSql
//! compilando offline -- e o que fez a compilacao cruzada para Windows
//! funcionar de primeira.
//!
//! Cobre JSON completo: objetos, listas, strings com escapes e `\uXXXX`
//! (inclusive pares substitutos), numeros com expoente, `true`, `false` e
//! `null`. Objetos preservam a ordem das chaves, entao gravar de volta um
//! arquivo lido nao embaralha a configuracao do usuario.

use crate::error::{PhxError, Result};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Nulo,
    Bool(bool),
    Numero(f64),
    Texto(String),
    Lista(Vec<Json>),
    /// Pares na ordem em que apareceram no arquivo.
    Objeto(Vec<(String, Json)>),
}

impl Json {
    pub fn analisar(entrada: &str) -> Result<Json> {
        let bytes = entrada.as_bytes();
        let mut p = Analisador { bytes, pos: 0 };
        p.pular_espaco();
        let v = p.valor()?;
        p.pular_espaco();
        if p.pos != bytes.len() {
            return Err(p.erro("sobrou conteudo depois do valor JSON"));
        }
        Ok(v)
    }

    // ------------------------------------------------------------ acesso

    pub fn campo(&self, nome: &str) -> Option<&Json> {
        match self {
            Json::Objeto(pares) => pares.iter().find(|(k, _)| k == nome).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn texto(&self) -> Option<&str> {
        match self {
            Json::Texto(s) => Some(s),
            _ => None,
        }
    }

    pub fn numero(&self) -> Option<f64> {
        match self {
            Json::Numero(n) => Some(*n),
            _ => None,
        }
    }

    pub fn inteiro(&self) -> Option<i64> {
        match self {
            Json::Numero(n) if n.fract() == 0.0 && n.is_finite() => Some(*n as i64),
            _ => None,
        }
    }

    pub fn booleano(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn lista(&self) -> Option<&[Json]> {
        match self {
            Json::Lista(v) => Some(v),
            _ => None,
        }
    }

    pub fn e_nulo(&self) -> bool {
        matches!(self, Json::Nulo)
    }

    /// Texto de um campo, ou o padrao quando ausente.
    pub fn texto_ou<'a>(&'a self, campo: &str, padrao: &'a str) -> &'a str {
        self.campo(campo).and_then(Json::texto).unwrap_or(padrao)
    }

    /// Inteiro de um campo, ou o padrao quando ausente.
    pub fn inteiro_ou(&self, campo: &str, padrao: i64) -> i64 {
        self.campo(campo).and_then(Json::inteiro).unwrap_or(padrao)
    }

    /// Booleano de um campo, ou o padrao quando ausente.
    pub fn booleano_ou(&self, campo: &str, padrao: bool) -> bool {
        self.campo(campo).and_then(Json::booleano).unwrap_or(padrao)
    }

    /// Lista de textos de um campo. Ausente devolve lista vazia.
    pub fn textos(&self, campo: &str) -> Vec<String> {
        self.campo(campo)
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .filter_map(Json::texto)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    // -------------------------------------------------------- construcao

    pub fn objeto(pares: Vec<(&str, Json)>) -> Json {
        Json::Objeto(pares.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    pub fn texto_de(s: impl Into<String>) -> Json {
        Json::Texto(s.into())
    }

    pub fn de_i64(n: i64) -> Json {
        Json::Numero(n as f64)
    }

    pub fn de_u64(n: u64) -> Json {
        Json::Numero(n as f64)
    }

    // ---------------------------------------------------------- escrita

    /// JSON compacto, numa linha so -- o formato do protocolo.
    pub fn escrever(&self) -> String {
        let mut s = String::new();
        self.render(&mut s, None, 0);
        s
    }

    /// JSON identado, para arquivos de configuracao legiveis.
    pub fn escrever_identado(&self) -> String {
        let mut s = String::new();
        self.render(&mut s, Some(2), 0);
        s
    }

    fn render(&self, saida: &mut String, identacao: Option<usize>, nivel: usize) {
        let (quebra, dentro, fecha) = match identacao {
            None => (String::new(), String::new(), String::new()),
            Some(n) => (
                "\n".to_string(),
                " ".repeat(n * (nivel + 1)),
                " ".repeat(n * nivel),
            ),
        };
        match self {
            Json::Nulo => saida.push_str("null"),
            Json::Bool(b) => saida.push_str(if *b { "true" } else { "false" }),
            Json::Numero(n) => {
                if n.is_finite() {
                    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                        let _ = write!(saida, "{}", *n as i64);
                    } else {
                        let _ = write!(saida, "{n}");
                    }
                } else {
                    saida.push_str("null");
                }
            }
            Json::Texto(s) => escrever_texto(saida, s),
            Json::Lista(itens) => {
                if itens.is_empty() {
                    saida.push_str("[]");
                    return;
                }
                saida.push('[');
                for (i, item) in itens.iter().enumerate() {
                    if i > 0 {
                        saida.push(',');
                    }
                    saida.push_str(&quebra);
                    saida.push_str(&dentro);
                    item.render(saida, identacao, nivel + 1);
                }
                saida.push_str(&quebra);
                saida.push_str(&fecha);
                saida.push(']');
            }
            Json::Objeto(pares) => {
                if pares.is_empty() {
                    saida.push_str("{}");
                    return;
                }
                saida.push('{');
                for (i, (chave, valor)) in pares.iter().enumerate() {
                    if i > 0 {
                        saida.push(',');
                    }
                    saida.push_str(&quebra);
                    saida.push_str(&dentro);
                    escrever_texto(saida, chave);
                    saida.push(':');
                    if identacao.is_some() {
                        saida.push(' ');
                    }
                    valor.render(saida, identacao, nivel + 1);
                }
                saida.push_str(&quebra);
                saida.push_str(&fecha);
                saida.push('}');
            }
        }
    }
}

fn escrever_texto(saida: &mut String, s: &str) {
    saida.push('"');
    for c in s.chars() {
        match c {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            '\u{08}' => saida.push_str("\\b"),
            '\u{0C}' => saida.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(saida, "\\u{:04x}", c as u32);
            }
            c => saida.push(c),
        }
    }
    saida.push('"');
}

struct Analisador<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Analisador<'_> {
    fn erro(&self, msg: &str) -> PhxError {
        PhxError::Esquema(format!("JSON invalido na posicao {}: {msg}", self.pos))
    }

    fn pular_espaco(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn atual(&self) -> Result<u8> {
        self.bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| self.erro("fim inesperado"))
    }

    fn consumir(&mut self, esperado: u8) -> Result<()> {
        if self.atual()? != esperado {
            return Err(self.erro(&format!("esperado {:?}", esperado as char)));
        }
        self.pos += 1;
        Ok(())
    }

    fn literal(&mut self, texto: &str) -> Result<()> {
        if self.bytes[self.pos..].starts_with(texto.as_bytes()) {
            self.pos += texto.len();
            Ok(())
        } else {
            Err(self.erro(&format!("esperado {texto}")))
        }
    }

    fn valor(&mut self) -> Result<Json> {
        match self.atual()? {
            b'{' => self.objeto(),
            b'[' => self.lista(),
            b'"' => Ok(Json::Texto(self.texto()?)),
            b't' => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            b'f' => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            b'n' => {
                self.literal("null")?;
                Ok(Json::Nulo)
            }
            b'-' | b'0'..=b'9' => self.numero(),
            outro => Err(self.erro(&format!("caractere inesperado {:?}", outro as char))),
        }
    }

    fn objeto(&mut self) -> Result<Json> {
        self.consumir(b'{')?;
        let mut pares = Vec::new();
        self.pular_espaco();
        if self.atual()? == b'}' {
            self.pos += 1;
            return Ok(Json::Objeto(pares));
        }
        loop {
            self.pular_espaco();
            let chave = self.texto()?;
            self.pular_espaco();
            self.consumir(b':')?;
            self.pular_espaco();
            let valor = self.valor()?;
            pares.push((chave, valor));
            self.pular_espaco();
            match self.atual()? {
                b',' => self.pos += 1,
                b'}' => {
                    self.pos += 1;
                    return Ok(Json::Objeto(pares));
                }
                _ => return Err(self.erro("esperado ',' ou '}'")),
            }
        }
    }

    fn lista(&mut self) -> Result<Json> {
        self.consumir(b'[')?;
        let mut itens = Vec::new();
        self.pular_espaco();
        if self.atual()? == b']' {
            self.pos += 1;
            return Ok(Json::Lista(itens));
        }
        loop {
            self.pular_espaco();
            itens.push(self.valor()?);
            self.pular_espaco();
            match self.atual()? {
                b',' => self.pos += 1,
                b']' => {
                    self.pos += 1;
                    return Ok(Json::Lista(itens));
                }
                _ => return Err(self.erro("esperado ',' ou ']'")),
            }
        }
    }

    fn texto(&mut self) -> Result<String> {
        self.consumir(b'"')?;
        let mut saida = String::new();
        loop {
            let b = self.atual()?;
            self.pos += 1;
            match b {
                b'"' => return Ok(saida),
                b'\\' => {
                    let e = self.atual()?;
                    self.pos += 1;
                    match e {
                        b'"' => saida.push('"'),
                        b'\\' => saida.push('\\'),
                        b'/' => saida.push('/'),
                        b'b' => saida.push('\u{08}'),
                        b'f' => saida.push('\u{0C}'),
                        b'n' => saida.push('\n'),
                        b'r' => saida.push('\r'),
                        b't' => saida.push('\t'),
                        b'u' => saida.push(self.unicode()?),
                        outro => {
                            return Err(self.erro(&format!("escape invalido \\{}", outro as char)))
                        }
                    }
                }
                b if b < 0x20 => return Err(self.erro("caractere de controle cru no texto")),
                b => {
                    // UTF-8: copia a sequencia inteira.
                    let extra = match b {
                        0x00..=0x7F => 0,
                        0xC0..=0xDF => 1,
                        0xE0..=0xEF => 2,
                        0xF0..=0xF7 => 3,
                        _ => return Err(self.erro("byte UTF-8 invalido")),
                    };
                    let inicio = self.pos - 1;
                    self.pos += extra;
                    if self.pos > self.bytes.len() {
                        return Err(self.erro("UTF-8 truncado"));
                    }
                    let trecho = std::str::from_utf8(&self.bytes[inicio..self.pos])
                        .map_err(|_| self.erro("UTF-8 invalido"))?;
                    saida.push_str(trecho);
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.erro("escape \\u truncado"));
        }
        let t = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
            .map_err(|_| self.erro("escape \\u invalido"))?;
        let v =
            u32::from_str_radix(t, 16).map_err(|_| self.erro("escape \\u nao e hexadecimal"))?;
        self.pos += 4;
        Ok(v)
    }

    fn unicode(&mut self) -> Result<char> {
        let alto = self.hex4()?;
        // Par substituto: 😀
        if (0xD800..0xDC00).contains(&alto) {
            self.literal("\\u")?;
            let baixo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&baixo) {
                return Err(self.erro("par substituto invalido"));
            }
            let cp = 0x10000 + ((alto - 0xD800) << 10) + (baixo - 0xDC00);
            return char::from_u32(cp).ok_or_else(|| self.erro("codepoint invalido"));
        }
        char::from_u32(alto).ok_or_else(|| self.erro("codepoint invalido"))
    }

    fn numero(&mut self) -> Result<Json> {
        let inicio = self.pos;
        if self.atual()? == b'-' {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'e' | b'E') {
            self.pos += 1;
            if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'+' | b'-') {
                self.pos += 1;
            }
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let t = std::str::from_utf8(&self.bytes[inicio..self.pos])
            .map_err(|_| self.erro("numero invalido"))?;
        t.parse::<f64>()
            .map(Json::Numero)
            .map_err(|_| self.erro("numero invalido"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos_basicos() {
        assert_eq!(Json::analisar("null").unwrap(), Json::Nulo);
        assert_eq!(Json::analisar("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::analisar("false").unwrap(), Json::Bool(false));
        assert_eq!(Json::analisar("42").unwrap(), Json::Numero(42.0));
        assert_eq!(Json::analisar("-3.5e2").unwrap(), Json::Numero(-350.0));
        assert_eq!(
            Json::analisar("\"ola\"").unwrap(),
            Json::Texto("ola".into())
        );
    }

    #[test]
    fn objeto_preserva_a_ordem_das_chaves() {
        let j = Json::analisar(r#"{"zeta":1,"alfa":2,"meio":3}"#).unwrap();
        match &j {
            Json::Objeto(pares) => {
                let chaves: Vec<&str> = pares.iter().map(|(k, _)| k.as_str()).collect();
                assert_eq!(chaves, vec!["zeta", "alfa", "meio"]);
            }
            outro => panic!("esperava objeto, veio {outro:?}"),
        }
        assert_eq!(j.escrever(), r#"{"zeta":1,"alfa":2,"meio":3}"#);
    }

    #[test]
    fn aninhamento_e_listas() {
        let txt =
            r#"{"bind":"0.0.0.0:5000","ips":["10.1.1.1","10.1.1.2"],"rep":{"papel":"source"}}"#;
        let j = Json::analisar(txt).unwrap();
        assert_eq!(j.texto_ou("bind", ""), "0.0.0.0:5000");
        assert_eq!(j.textos("ips"), vec!["10.1.1.1", "10.1.1.2"]);
        assert_eq!(j.campo("rep").unwrap().texto_ou("papel", ""), "source");
        assert_eq!(j.escrever(), txt);
    }

    #[test]
    fn escapes_e_acentos() {
        let j = Json::analisar(r#""linha\nnova \"aspas\" \\barra\\ ção""#).unwrap();
        assert_eq!(j.texto().unwrap(), "linha\nnova \"aspas\" \\barra\\ ção");
        // Ida e volta.
        let ida = j.escrever();
        assert_eq!(Json::analisar(&ida).unwrap(), j);
    }

    #[test]
    fn par_substituto_vira_emoji() {
        let j = Json::analisar(r#""😀""#).unwrap();
        assert_eq!(j.texto().unwrap(), "\u{1F600}");
    }

    #[test]
    fn utf8_cru_no_texto() {
        let j = Json::analisar("\"São Paulo — ação\"").unwrap();
        assert_eq!(j.texto().unwrap(), "São Paulo — ação");
    }

    #[test]
    fn identado_volta_igual() {
        let txt = r#"{"a":1,"b":[1,2,{"c":true}],"d":{}}"#;
        let j = Json::analisar(txt).unwrap();
        let bonito = j.escrever_identado();
        assert!(bonito.contains('\n'));
        assert_eq!(Json::analisar(&bonito).unwrap(), j);
    }

    #[test]
    fn entradas_invalidas_sao_recusadas() {
        for ruim in [
            "{",
            "[1,2",
            r#"{"a" 1}"#,
            r#"{"a":1,}"#,
            "tru",
            r#""sem fim"#,
            "{} sobra",
            r#""\q""#,
            r#""\ud83d""#,
        ] {
            assert!(Json::analisar(ruim).is_err(), "deveria recusar: {ruim}");
        }
    }

    #[test]
    fn acessores_com_padrao() {
        let j = Json::analisar(r#"{"porta":5000,"ligado":true}"#).unwrap();
        assert_eq!(j.inteiro_ou("porta", 0), 5000);
        assert_eq!(j.inteiro_ou("ausente", 77), 77);
        assert!(j.booleano_ou("ligado", false));
        assert!(!j.booleano_ou("ausente", false));
        assert_eq!(j.texto_ou("ausente", "padrao"), "padrao");
    }
}

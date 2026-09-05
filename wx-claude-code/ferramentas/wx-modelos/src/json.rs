//! JSON minimo, escrito aqui: o projeto nao aceita dependencia externa.
//!
//! Le o suficiente para catalogo e resposta de servico (objeto, lista, texto,
//! numero, booleano, nulo) e falha dizendo ONDE, em vez de devolver vazio --
//! catalogo truncado que "parece ler" e a pior falha possivel aqui.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Booleano(bool),
    Numero(f64),
    Texto(String),
    Lista(Vec<Valor>),
    Objeto(BTreeMap<String, Valor>),
}

impl Valor {
    pub fn campo(&self, nome: &str) -> Option<&Valor> {
        match self {
            Valor::Objeto(m) => m.get(nome),
            _ => None,
        }
    }
    pub fn texto(&self) -> Option<&str> {
        match self {
            Valor::Texto(s) => Some(s),
            _ => None,
        }
    }
    pub fn numero(&self) -> Option<f64> {
        match self {
            Valor::Numero(n) => Some(*n),
            _ => None,
        }
    }
    pub fn lista(&self) -> Option<&Vec<Valor>> {
        match self {
            Valor::Lista(v) => Some(v),
            _ => None,
        }
    }
    pub fn campo_texto(&self, nome: &str) -> Option<String> {
        self.campo(nome).and_then(|v| v.texto()).map(str::to_string)
    }
    pub fn campo_numero(&self, nome: &str) -> Option<f64> {
        self.campo(nome).and_then(|v| v.numero())
    }
}

#[derive(Debug)]
pub struct Erro {
    pub posicao: usize,
    pub motivo: String,
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON invalido na posicao {}: {}",
            self.posicao, self.motivo
        )
    }
}

pub fn analisar(entrada: &str) -> Result<Valor, Erro> {
    let bytes: Vec<char> = entrada.chars().collect();
    let mut p = Analisador { bytes, i: 0 };
    p.espaco();
    let v = p.valor()?;
    p.espaco();
    if p.i != p.bytes.len() {
        return Err(p.erro("sobrou texto depois do valor"));
    }
    Ok(v)
}

struct Analisador {
    bytes: Vec<char>,
    i: usize,
}

impl Analisador {
    fn erro(&self, motivo: &str) -> Erro {
        Erro {
            posicao: self.i,
            motivo: motivo.to_string(),
        }
    }
    fn atual(&self) -> Option<char> {
        self.bytes.get(self.i).copied()
    }
    fn espaco(&mut self) {
        while matches!(self.atual(), Some(c) if c.is_whitespace()) {
            self.i += 1;
        }
    }
    fn literal(&mut self, texto: &str) -> bool {
        let fim = self.i + texto.chars().count();
        if fim <= self.bytes.len() && self.bytes[self.i..fim].iter().collect::<String>() == texto {
            self.i = fim;
            true
        } else {
            false
        }
    }
    fn valor(&mut self) -> Result<Valor, Erro> {
        match self.atual() {
            Some('{') => self.objeto(),
            Some('[') => self.lista(),
            Some('"') => Ok(Valor::Texto(self.texto()?)),
            Some('t') if self.literal("true") => Ok(Valor::Booleano(true)),
            Some('f') if self.literal("false") => Ok(Valor::Booleano(false)),
            Some('n') if self.literal("null") => Ok(Valor::Nulo),
            Some(c) if c == '-' || c.is_ascii_digit() => self.numero(),
            _ => Err(self.erro("valor esperado")),
        }
    }
    fn objeto(&mut self) -> Result<Valor, Erro> {
        self.i += 1;
        let mut m = BTreeMap::new();
        self.espaco();
        if self.atual() == Some('}') {
            self.i += 1;
            return Ok(Valor::Objeto(m));
        }
        loop {
            self.espaco();
            let chave = self.texto()?;
            self.espaco();
            if self.atual() != Some(':') {
                return Err(self.erro("faltou ':' depois da chave"));
            }
            self.i += 1;
            self.espaco();
            m.insert(chave, self.valor()?);
            self.espaco();
            match self.atual() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(Valor::Objeto(m));
                }
                _ => return Err(self.erro("faltou ',' ou '}'")),
            }
        }
    }
    fn lista(&mut self) -> Result<Valor, Erro> {
        self.i += 1;
        let mut v = Vec::new();
        self.espaco();
        if self.atual() == Some(']') {
            self.i += 1;
            return Ok(Valor::Lista(v));
        }
        loop {
            self.espaco();
            v.push(self.valor()?);
            self.espaco();
            match self.atual() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Valor::Lista(v));
                }
                _ => return Err(self.erro("faltou ',' ou ']'")),
            }
        }
    }
    fn texto(&mut self) -> Result<String, Erro> {
        if self.atual() != Some('"') {
            return Err(self.erro("texto esperado"));
        }
        self.i += 1;
        let mut s = String::new();
        loop {
            match self.atual() {
                None => return Err(self.erro("texto sem fechamento")),
                Some('"') => {
                    self.i += 1;
                    return Ok(s);
                }
                Some('\\') => {
                    self.i += 1;
                    let c = self.atual().ok_or_else(|| self.erro("escape sem letra"))?;
                    self.i += 1;
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        'b' => '\u{8}',
                        'f' => '\u{c}',
                        'u' => {
                            let hex: String = self
                                .bytes
                                .get(self.i..self.i + 4)
                                .ok_or_else(|| self.erro("\\u incompleto"))?
                                .iter()
                                .collect();
                            self.i += 4;
                            let n = u32::from_str_radix(&hex, 16)
                                .map_err(|_| self.erro("\\u com hex invalido"))?;
                            char::from_u32(n).ok_or_else(|| self.erro("\\u fora do intervalo"))?
                        }
                        outro => outro,
                    });
                }
                Some(c) => {
                    self.i += 1;
                    s.push(c);
                }
            }
        }
    }
    fn numero(&mut self) -> Result<Valor, Erro> {
        let inicio = self.i;
        if self.atual() == Some('-') {
            self.i += 1;
        }
        while matches!(self.atual(), Some(c) if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
        {
            self.i += 1;
        }
        let bruto: String = self.bytes[inicio..self.i].iter().collect();
        bruto
            .parse::<f64>()
            .map(Valor::Numero)
            .map_err(|_| self.erro("numero invalido"))
    }
}

/// Escapa um texto para sair como valor JSON. Usado na saida --json.
pub fn escapar(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_objeto_com_lista_e_nulo() {
        let v = analisar(r#"{"a": [1, 2.5, -3e2], "b": null, "c": true, "d": "x\ny"}"#).unwrap();
        assert_eq!(v.campo("a").unwrap().lista().unwrap().len(), 3);
        assert_eq!(
            v.campo("a").unwrap().lista().unwrap()[2],
            Valor::Numero(-300.0)
        );
        assert_eq!(v.campo("b"), Some(&Valor::Nulo));
        assert_eq!(v.campo_texto("d").unwrap(), "x\ny");
    }

    #[test]
    fn recusa_json_truncado_dizendo_onde() {
        let e = analisar(r#"{"a": [1, 2"#).unwrap_err();
        assert!(e.motivo.contains("','"), "{}", e.motivo);
        assert!(e.to_string().contains("posicao"));
    }

    #[test]
    fn recusa_sobra_depois_do_valor() {
        assert!(analisar("{} lixo").is_err());
    }

    #[test]
    fn escapa_o_que_quebraria_a_saida() {
        assert_eq!(escapar("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }
}

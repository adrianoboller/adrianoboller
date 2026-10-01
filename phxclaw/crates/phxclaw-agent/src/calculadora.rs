//! Calculadora: aritmetica por analisador descendente proprio, sem `eval` e sem processo.
//!
//! Existe porque modelo pequeno erra conta -- e mandar a conta para o `python_repl` paga um
//! sandbox inteiro para somar dois numeros, alem de exigir `shell.exec`. A gramatica so
//! conhece numero, operador, parenteses, constante e uma lista fechada de funcoes: nao ha
//! nome que leve a outro lugar, entao nao ha o que escapar.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

/// Teto da expressao e da profundidade: `((((...))))` com um milhao de niveis estouraria a
/// pilha do analisador recursivo antes de qualquer conta.
const TAMANHO_MAX: usize = 2_000;
const FUNDO_MAX: u32 = 64;

struct Analisador<'a> {
    c: &'a [u8],
    i: usize,
    fundo: u32,
}

pub fn avaliar(expr: &str) -> Result<f64, String> {
    if expr.len() > TAMANHO_MAX {
        return Err(format!("expressao maior que {TAMANHO_MAX} caracteres"));
    }
    let mut a = Analisador {
        c: expr.as_bytes(),
        i: 0,
        fundo: 0,
    };
    let v = a.soma()?;
    a.espaco();
    if a.i < a.c.len() {
        return Err(format!(
            "sobrou texto na posicao {}: {:?}",
            a.i + 1,
            String::from_utf8_lossy(&a.c[a.i..])
        ));
    }
    if !v.is_finite() {
        return Err(format!("resultado nao finito ({v})"));
    }
    Ok(v)
}

/// Inteiro exato sem `.0`; o resto com ate 12 digitos significativos, sem ruido de ponto
/// flutuante (`0.1 + 0.2` sai `0.3`).
pub fn formatar(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 9.007_199_254_740_992e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.12e}", v);
    let n: f64 = s.parse().unwrap_or(v);
    let t = format!("{n}");
    if t.len() > 24 { format!("{v:e}") } else { t }
}

impl Analisador<'_> {
    fn espaco(&mut self) {
        while self.i < self.c.len() && self.c[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn ve(&mut self, s: &str) -> bool {
        self.espaco();
        if self.c[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            true
        } else {
            false
        }
    }

    fn soma(&mut self) -> Result<f64, String> {
        let mut v = self.produto()?;
        loop {
            if self.ve("+") {
                v += self.produto()?;
            } else if self.ve("-") {
                v -= self.produto()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn produto(&mut self) -> Result<f64, String> {
        let mut v = self.unario()?;
        loop {
            // `**` e potencia, nao dois `*`: confere antes do `*`.
            self.espaco();
            if self.c[self.i..].starts_with(b"**") {
                return Ok(v);
            }
            if self.ve("*") {
                v *= self.unario()?;
            } else if self.ve("/") {
                let d = self.unario()?;
                if d == 0.0 {
                    return Err("divisao por zero".into());
                }
                v /= d;
            } else if self.ve("%") {
                let d = self.unario()?;
                if d == 0.0 {
                    return Err("resto por zero".into());
                }
                v %= d;
            } else {
                return Ok(v);
            }
        }
    }

    fn unario(&mut self) -> Result<f64, String> {
        if self.ve("-") {
            return Ok(-self.unario()?);
        }
        if self.ve("+") {
            return self.unario();
        }
        self.potencia()
    }

    /// Associativa a direita, como na matematica: `2^3^2` = 2^9.
    fn potencia(&mut self) -> Result<f64, String> {
        let base = self.atomo()?;
        if self.ve("**") || self.ve("^") {
            let exp = self.unario()?;
            return Ok(base.powf(exp));
        }
        Ok(base)
    }

    fn atomo(&mut self) -> Result<f64, String> {
        self.espaco();
        self.fundo += 1;
        if self.fundo > FUNDO_MAX {
            return Err("parenteses demais".into());
        }
        let r = self.atomo_dentro();
        self.fundo -= 1;
        r
    }

    fn atomo_dentro(&mut self) -> Result<f64, String> {
        if self.ve("(") {
            let v = self.soma()?;
            if !self.ve(")") {
                return Err("falta ')'".into());
            }
            return Ok(v);
        }
        let ini = self.i;
        if self.i < self.c.len() && (self.c[self.i].is_ascii_digit() || self.c[self.i] == b'.') {
            while self.i < self.c.len()
                && (self.c[self.i].is_ascii_digit() || matches!(self.c[self.i], b'.' | b'_'))
            {
                self.i += 1;
            }
            // Expoente: 1e3, 2.5E-4.
            if self.i < self.c.len() && matches!(self.c[self.i], b'e' | b'E') {
                let j = self.i + 1;
                let k = if j < self.c.len() && matches!(self.c[j], b'+' | b'-') {
                    j + 1
                } else {
                    j
                };
                if k < self.c.len() && self.c[k].is_ascii_digit() {
                    self.i = k;
                    while self.i < self.c.len() && self.c[self.i].is_ascii_digit() {
                        self.i += 1;
                    }
                }
            }
            let t: String = String::from_utf8_lossy(&self.c[ini..self.i]).replace('_', "");
            return t.parse().map_err(|_| format!("numero invalido: {t}"));
        }
        while self.i < self.c.len() && (self.c[self.i].is_ascii_alphanumeric()) {
            self.i += 1;
        }
        let nome = String::from_utf8_lossy(&self.c[ini..self.i]).to_lowercase();
        if nome.is_empty() {
            return Err(match self.c.get(self.i) {
                Some(c) => format!(
                    "simbolo inesperado {:?} na posicao {}",
                    *c as char,
                    self.i + 1
                ),
                None => "expressao incompleta".into(),
            });
        }
        match nome.as_str() {
            "pi" => return Ok(std::f64::consts::PI),
            "e" => return Ok(std::f64::consts::E),
            "tau" => return Ok(std::f64::consts::TAU),
            _ => {}
        }
        if !self.ve("(") {
            return Err(format!("nome desconhecido: {nome}"));
        }
        let mut args = vec![self.soma()?];
        while self.ve(",") {
            args.push(self.soma()?);
        }
        if !self.ve(")") {
            return Err(format!("falta ')' em {nome}("));
        }
        aplicar(&nome, &args)
    }
}

fn aplicar(nome: &str, a: &[f64]) -> Result<f64, String> {
    let um = |f: fn(f64) -> f64| {
        if a.len() == 1 {
            Ok(f(a[0]))
        } else {
            Err(format!("{nome} recebe 1 argumento"))
        }
    };
    match nome {
        "sqrt" | "raiz" => {
            if a.len() == 1 && a[0] < 0.0 {
                return Err("raiz de negativo".into());
            }
            um(f64::sqrt)
        }
        "abs" => um(f64::abs),
        "ln" => um(f64::ln),
        "log" | "log10" => um(f64::log10),
        "log2" => um(f64::log2),
        "exp" => um(f64::exp),
        "sin" | "sen" => um(f64::sin),
        "cos" => um(f64::cos),
        "tan" | "tg" => um(f64::tan),
        "asin" => um(f64::asin),
        "acos" => um(f64::acos),
        "atan" => um(f64::atan),
        "floor" | "piso" => um(f64::floor),
        "ceil" | "teto" => um(f64::ceil),
        "round" => um(f64::round),
        "trunc" => um(f64::trunc),
        "min" | "max" if !a.is_empty() => Ok(a.iter().copied().fold(
            if nome == "min" {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            },
            if nome == "min" { f64::min } else { f64::max },
        )),
        "pow" if a.len() == 2 => Ok(a[0].powf(a[1])),
        _ => Err(format!("funcao desconhecida ou aridade errada: {nome}")),
    }
}

pub struct CalculatorTool;

impl Tool for CalculatorTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "calculator".into(),
            description:
                "Evaluate an arithmetic expression exactly as written: + - * / % ^ (or **), \
parentheses, pi, e, and sqrt abs ln log log2 exp sin cos tan asin acos atan floor ceil round \
trunc min max pow. Use it instead of doing arithmetic in your head."
                    .into(),
            parameters: json!({"type":"object","properties":{"expression":{"type":"string"}},"required":["expression"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "calc"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let e = args
                .get("expression")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'expression'".into()))?;
            let v = avaliar(e).map_err(ToolError::InvalidArguments)?;
            Ok(ToolOutput::text(format!("{} = {}", e.trim(), formatar(v))))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        formatar(avaliar(s).unwrap())
    }

    #[test]
    fn precedencia_associatividade_e_funcoes() {
        assert_eq!(c("1 + 2 * 3"), "7");
        assert_eq!(c("(1 + 2) * 3"), "9");
        assert_eq!(c("2 ^ 3 ^ 2"), "512");
        assert_eq!(c("2 ** 10"), "1024");
        assert_eq!(c("-2 ^ 2"), "-4");
        assert_eq!(c("10 % 4"), "2");
        assert_eq!(c("0.1 + 0.2"), "0.3");
        assert_eq!(c("sqrt(16) + max(1, 7, 3)"), "11");
        assert_eq!(c("1_000_000 * 1e3"), "1000000000");
        assert_eq!(c("floor(pi * 100) / 100"), "3.14");
        assert_eq!(c("3 * -2"), "-6");
    }

    #[test]
    fn recusa_o_que_nao_e_conta() {
        for e in [
            "1/0",
            "__import__('os')",
            "os.system(1)",
            "2 +",
            "(1",
            "1 2",
            "sqrt(-1)",
            "x + 1",
            "10 ** 400",
        ] {
            assert!(avaliar(e).is_err(), "{e}");
        }
        assert!(avaliar(&"(".repeat(100_000)).is_err());
        assert!(avaliar(&format!("{}1{}", "(".repeat(500), ")".repeat(500))).is_err());
    }
}

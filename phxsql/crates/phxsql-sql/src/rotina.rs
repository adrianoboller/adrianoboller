//! Rotinas: gatilhos (`CREATE TRIGGER`) e procedimentos (`CREATE PROCEDURE`),
//! na linguagem do MySQL(R)/MariaDB(R) — sintaxe similar, nao identica.
//!
//! # Por que UM interpretador
//!
//! Os dois pedidos (#49 e #50) esperavam a mesma coisa: alguem que leia um
//! corpo `BEGIN … END` e o execute. Escrever dois leitores seria como os dois
//! divergiriam no primeiro caso esquisito — entao o corpo de um gatilho e o
//! corpo de um procedimento passam pelo MESMO analisador e pelo MESMO
//! avaliador, e o que muda entre eles e so a tabela de regras ([`Regras`]):
//! o que cada contexto pode ver (`NEW`/`OLD`) e fazer (DML, `SIGNAL`).
//!
//! # O que cabe, e o que recusa dizendo o nome
//!
//! Cabem: `DECLARE`, `SET`, `IF/ELSEIF/ELSE`, `WHILE`, `SIGNAL SQLSTATE`,
//! `INSERT INTO t (…) VALUES (…)` e `SELECT … INTO var`. O resto — `CASE`,
//! `LOOP`, cursor, handler, `UPDATE`/`DELETE` no corpo, transacao — recusa
//! com o proprio nome e o motivo, na mesma regra do `SELECT` desta camada:
//! aceitar a sintaxe e responder errado calado e o pior dos dois mundos.
//!
//! # O numero e exato de proposito
//!
//! A conta aqui dentro nao passa por `f64`: [`Numero`] guarda mantissa `i128`
//! e escala, e `1.10 * 3` da `3.30` com os dois digitos. E a mesma decisao do
//! lexico e do protocolo — dinheiro nao perde centavo no caminho — levada ate
//! a aritmetica, porque `SET NEW.preco = NEW.preco * 1.1` e exatamente o tipo
//! de corpo que alguem vai escrever.
//!
//! # Quem fala com o motor e quem chama
//!
//! Este modulo nao abre arquivo e nao conhece tabela: o `INSERT` e o
//! `SELECT … INTO` de um corpo saem pelo trait [`Motor`], que o servidor
//! implementa por cima do MESMO portao de permissao dos pedidos da rede.
//! E a licao do `juntar`/`unir`: a rotina PRODUZ o pedido que o portao ja
//! sabe conferir, em vez de ganhar uma porta propria.

use crate::lexico::{self, Comparador, Token};
use crate::sintaxe::Analisador;
use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// Teto de passos do avaliador, por corpo executado.
///
/// Um `WHILE` sem fim nao pode segurar uma conexao para sempre — e, num
/// gatilho, roda com a trava de dados na mao. O teto e alto o bastante para
/// qualquer corpo honesto e baixo o bastante para devolver o erro em
/// milissegundos.
pub const PASSOS_MAX: u64 = 1_000_000;

// ------------------------------------------------------------------ numeros

/// Numero exato: mantissa inteira e escala decimal.
///
/// `12.34` e `{ mantissa: 1234, escala: 2 }`. Soma e comparacao alinham a
/// escala; multiplicacao soma as escalas; divisao arredonda para
/// `max(escalas) + 4` casas, como o `div_precision_increment` do MySQL(R).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numero {
    mantissa: i128,
    escala: u32,
}

/// 10^n em `i128`, recusando o estouro em vez de dar a volta.
fn pot10(n: u32) -> Result<i128> {
    if n > 38 {
        return Err(PhxError::LimiteExcedido(format!(
            "a conta pediu 10^{n}, alem da precisao de 38 digitos"
        )));
    }
    Ok((0..n).fold(1i128, |a, _| a * 10))
}

impl Numero {
    pub fn inteiro(n: i64) -> Numero {
        Numero {
            mantissa: n as i128,
            escala: 0,
        }
    }

    pub fn de_texto(t: &str) -> Option<Numero> {
        let t = t.trim();
        let (negativo, t) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let (inteiro, fracao) = match t.split_once('.') {
            Some((a, b)) => (a, b),
            None => (t, ""),
        };
        if inteiro.is_empty() && fracao.is_empty() {
            return None;
        }
        let digitos = |s: &str| s.chars().all(|c| c.is_ascii_digit());
        if !digitos(inteiro) || !digitos(fracao) {
            return None;
        }
        let mut m: i128 = 0;
        for c in inteiro.chars().chain(fracao.chars()) {
            m = m.checked_mul(10)?.checked_add((c as u8 - b'0') as i128)?;
        }
        Some(Numero {
            mantissa: if negativo { -m } else { m },
            escala: fracao.len() as u32,
        })
    }

    /// De um `f64` do JSON. Inteiro dentro da faixa exata do `f64` entra
    /// direto; fracionario passa pelo texto do proprio numero, que e a
    /// representacao mais curta que o reproduz.
    pub fn de_f64(f: f64) -> Result<Numero> {
        if !f.is_finite() {
            return Err(PhxError::Tipo(format!("{f} nao e um numero utilizavel")));
        }
        if f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 {
            return Ok(Numero {
                mantissa: f as i128,
                escala: 0,
            });
        }
        Numero::de_texto(&format!("{f}"))
            .ok_or_else(|| PhxError::Tipo(format!("{f} nao vira numero exato")))
    }

    pub fn escala(&self) -> u32 {
        self.escala
    }

    /// Escreve como o SQL escreveria: `-12.34`, `7`, `0.500`.
    pub fn escrever(&self) -> String {
        let negativo = self.mantissa < 0;
        let abs = self.mantissa.unsigned_abs().to_string();
        let corpo = if self.escala == 0 {
            abs
        } else {
            let e = self.escala as usize;
            let cheio = if abs.len() <= e {
                format!("{}{}", "0".repeat(e - abs.len() + 1), abs)
            } else {
                abs
            };
            let ponto = cheio.len() - e;
            format!("{}.{}", &cheio[..ponto], &cheio[ponto..])
        };
        if negativo {
            format!("-{corpo}")
        } else {
            corpo
        }
    }

    /// Leva para a escala pedida, arredondando meio para longe do zero —
    /// `0.005` com escala 2 vira `0.01`, como se espera de dinheiro.
    pub fn com_escala(&self, alvo: u32) -> Result<Numero> {
        if alvo >= self.escala {
            let fator = pot10(alvo - self.escala)?;
            let m = self.mantissa.checked_mul(fator).ok_or_else(estouro)?;
            return Ok(Numero {
                mantissa: m,
                escala: alvo,
            });
        }
        let fator = pot10(self.escala - alvo)?;
        let q = self.mantissa / fator;
        let resto = self.mantissa % fator;
        let ajuste = if resto.abs() * 2 >= fator {
            self.mantissa.signum()
        } else {
            0
        };
        Ok(Numero {
            mantissa: q + ajuste,
            escala: alvo,
        })
    }

    fn alinhar(a: Numero, b: Numero) -> Result<(i128, i128, u32)> {
        let escala = a.escala.max(b.escala);
        let ma = a.mantissa.checked_mul(pot10(escala - a.escala)?);
        let mb = b.mantissa.checked_mul(pot10(escala - b.escala)?);
        match (ma, mb) {
            (Some(x), Some(y)) => Ok((x, y, escala)),
            _ => Err(estouro()),
        }
    }

    pub fn somar(a: Numero, b: Numero) -> Result<Numero> {
        let (x, y, escala) = Numero::alinhar(a, b)?;
        Ok(Numero {
            mantissa: x.checked_add(y).ok_or_else(estouro)?,
            escala,
        })
    }

    pub fn subtrair(a: Numero, b: Numero) -> Result<Numero> {
        let (x, y, escala) = Numero::alinhar(a, b)?;
        Ok(Numero {
            mantissa: x.checked_sub(y).ok_or_else(estouro)?,
            escala,
        })
    }

    pub fn multiplicar(a: Numero, b: Numero) -> Result<Numero> {
        Ok(Numero {
            mantissa: a.mantissa.checked_mul(b.mantissa).ok_or_else(estouro)?,
            escala: a.escala + b.escala,
        })
    }

    /// Divide com 4 casas alem da maior escala dos operandos, arredondando —
    /// e o `div_precision_increment` padrao do MySQL(R), e e documentado.
    pub fn dividir(a: Numero, b: Numero) -> Result<Numero> {
        if b.mantissa == 0 {
            return Err(PhxError::Tipo("divisao por zero".into()));
        }
        let escala = a.escala.max(b.escala) + 4;
        // a/b na escala pedida: mantissa = a.m * 10^(escala + b.e - a.e) / b.m
        let fator = pot10(escala + b.escala - a.escala)?;
        let alto = a.mantissa.checked_mul(fator).ok_or_else(estouro)?;
        let q = alto / b.mantissa;
        let resto = alto % b.mantissa;
        let ajuste = if resto.abs() * 2 >= b.mantissa.abs() {
            if (alto < 0) == (b.mantissa < 0) {
                1
            } else {
                -1
            }
        } else {
            0
        };
        Ok(Numero {
            mantissa: q + ajuste,
            escala,
        })
    }

    pub fn comparar(a: Numero, b: Numero) -> Result<std::cmp::Ordering> {
        let (x, y, _) = Numero::alinhar(a, b)?;
        Ok(x.cmp(&y))
    }

    pub fn negativo(&self) -> Numero {
        Numero {
            mantissa: -self.mantissa,
            escala: self.escala,
        }
    }

    pub fn absoluto(&self) -> Numero {
        Numero {
            mantissa: self.mantissa.abs(),
            escala: self.escala,
        }
    }

    pub fn e_zero(&self) -> bool {
        self.mantissa == 0
    }

    pub fn como_i64(&self) -> Result<i64> {
        let n = self.com_escala(0)?;
        i64::try_from(n.mantissa).map_err(|_| estouro())
    }
}

fn estouro() -> PhxError {
    PhxError::LimiteExcedido("a conta estourou a precisao de 38 digitos".into())
}

// ------------------------------------------------------------------ valores

/// O valor que circula dentro de um corpo.
///
/// Numero e [`Numero`] exato; o que veio do JSON como `f64` inteiro entra
/// exato, e decimal ja viaja como TEXTO no protocolo — quem faz conta com ele
/// passa pela coercao numerica, sem tocar em `f64`.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Bool(bool),
    Numero(Numero),
    Texto(String),
}

impl Valor {
    pub fn de_json(j: &Json) -> Result<Valor> {
        Ok(match j {
            Json::Nulo => Valor::Nulo,
            Json::Bool(b) => Valor::Bool(*b),
            Json::Numero(f) => Valor::Numero(Numero::de_f64(*f)?),
            Json::Texto(t) => Valor::Texto(t.clone()),
            outro => {
                return Err(PhxError::Tipo(format!(
                    "valor composto ({}) nao circula dentro de rotina",
                    match outro {
                        Json::Lista(_) => "lista",
                        _ => "objeto",
                    }
                )))
            }
        })
    }

    /// Como este valor sai num pedido ao motor.
    ///
    /// Numero de escala zero que cabe exato em `f64` vai como numero JSON;
    /// o resto vai como TEXTO — que e como decimal ja viaja no protocolo, e
    /// que toda coluna aceita (o motor alarga texto para inteiro e real).
    pub fn para_json(&self) -> Json {
        match self {
            Valor::Nulo => Json::Nulo,
            Valor::Bool(b) => Json::Bool(*b),
            Valor::Numero(n) => {
                if n.escala() == 0 {
                    if let Ok(i) = n.como_i64() {
                        if i.abs() < 9_007_199_254_740_992 {
                            return Json::Numero(i as f64);
                        }
                    }
                }
                Json::texto_de(n.escrever())
            }
            Valor::Texto(t) => Json::texto_de(t),
        }
    }

    /// Este valor escrito como um literal SQL, para entrar num SELECT.
    pub fn escrever_literal_sql(&self) -> String {
        match self {
            Valor::Nulo => "NULL".into(),
            Valor::Bool(true) => "TRUE".into(),
            Valor::Bool(false) => "FALSE".into(),
            Valor::Numero(n) => n.escrever(),
            Valor::Texto(t) => format!("'{}'", t.replace('\'', "''")),
        }
    }

    /// A coercao numerica: numero fica, texto que LE como numero vira numero.
    ///
    /// Existe porque decimal chega do protocolo como texto de proposito —
    /// exigir cast explicito quebraria justamente a conta com dinheiro, que e
    /// a que este interpretador existe para fazer certa. Texto que nao e
    /// numero recusa com o proprio texto na mensagem, em vez do `0` calado
    /// que o MySQL(R) daria.
    fn como_numero(&self, papel: &str) -> Result<Numero> {
        match self {
            Valor::Numero(n) => Ok(*n),
            Valor::Texto(t) => Numero::de_texto(t)
                .ok_or_else(|| PhxError::Tipo(format!("{papel}: o texto {t:?} nao e um numero"))),
            Valor::Bool(_) => Err(PhxError::Tipo(format!(
                "{papel}: booleano nao entra em conta; compare com TRUE/FALSE"
            ))),
            Valor::Nulo => Err(PhxError::Tipo(format!("{papel}: o valor e NULL"))),
        }
    }

    fn como_texto(&self) -> String {
        match self {
            Valor::Nulo => String::new(),
            Valor::Bool(b) => if *b { "1" } else { "0" }.to_string(),
            Valor::Numero(n) => n.escrever(),
            Valor::Texto(t) => t.clone(),
        }
    }
}

// ------------------------------------------------------------------- tipos

/// Os tipos que `DECLARE` e os parametros aceitam.
///
/// Datas viajam como texto no protocolo e continuam texto aqui — declarar
/// `DATE` vale, e o valor e o `'AAAA-MM-DD'` de sempre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Inteiro,
    Decimal { escala: u32 },
    Texto,
    Booleano,
}

impl Tipo {
    pub fn nome(&self) -> String {
        match self {
            Tipo::Inteiro => "INT".into(),
            Tipo::Decimal { escala } => format!("DECIMAL(*,{escala})"),
            Tipo::Texto => "VARCHAR".into(),
            Tipo::Booleano => "BOOL".into(),
        }
    }

    /// Traz um valor para este tipo, como uma atribuicao do MySQL(R) faria —
    /// com arredondamento explicito, nunca com truncagem calada.
    pub fn coagir(&self, v: Valor) -> Result<Valor> {
        if v == Valor::Nulo {
            return Ok(Valor::Nulo);
        }
        Ok(match self {
            Tipo::Inteiro => {
                let n = v.como_numero("atribuicao a INT")?;
                Valor::Numero(n.com_escala(0)?)
            }
            Tipo::Decimal { escala } => {
                let n = v.como_numero("atribuicao a DECIMAL")?;
                Valor::Numero(n.com_escala(*escala)?)
            }
            Tipo::Texto => Valor::Texto(v.como_texto()),
            Tipo::Booleano => match v {
                Valor::Bool(b) => Valor::Bool(b),
                Valor::Numero(n) => Valor::Bool(!n.e_zero()),
                Valor::Texto(t) => match t.trim().to_lowercase().as_str() {
                    "true" | "1" => Valor::Bool(true),
                    "false" | "0" => Valor::Bool(false),
                    outro => {
                        return Err(PhxError::Tipo(format!(
                            "{outro:?} nao e booleano (use TRUE/FALSE)"
                        )))
                    }
                },
                Valor::Nulo => Valor::Nulo,
            },
        })
    }
}

// ------------------------------------------------------------------ a AST

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quando {
    Antes,
    Depois,
}

impl Quando {
    pub fn nome(&self) -> &'static str {
        // Na grafia da linguagem escolhida: e assim que o gatilho se escreve
        // e e assim que ele volta no SHOW TRIGGERS.
        match self {
            Quando::Antes => "BEFORE",
            Quando::Depois => "AFTER",
        }
    }

    pub fn de_texto(t: &str) -> Option<Quando> {
        match t.to_uppercase().as_str() {
            "BEFORE" => Some(Quando::Antes),
            "AFTER" => Some(Quando::Depois),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    Inserir,
    Atualizar,
    Excluir,
}

impl Evento {
    pub fn nome(&self) -> &'static str {
        match self {
            Evento::Inserir => "INSERT",
            Evento::Atualizar => "UPDATE",
            Evento::Excluir => "DELETE",
        }
    }

    pub fn de_texto(t: &str) -> Option<Evento> {
        match t.to_uppercase().as_str() {
            "INSERT" => Some(Evento::Inserir),
            "UPDATE" => Some(Evento::Atualizar),
            "DELETE" => Some(Evento::Excluir),
            _ => None,
        }
    }
}

/// As funcoes embutidas. Poucas e uteis; o resto recusa pelo nome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funcao {
    Concat,
    Maiusculas,
    Minusculas,
    Aparar,
    Comprimento,
    Arredondar,
    Absoluto,
    Coalescer,
}

impl Funcao {
    fn de_nome(nome: &str) -> Option<Funcao> {
        Some(match nome {
            "CONCAT" => Funcao::Concat,
            "UPPER" | "UCASE" => Funcao::Maiusculas,
            "LOWER" | "LCASE" => Funcao::Minusculas,
            "TRIM" => Funcao::Aparar,
            // LENGTH conta CARACTERES aqui (o do MySQL(R) conta bytes);
            // CHAR_LENGTH e o sinonimo honesto. Documentado em TRIGGERS.md.
            "LENGTH" | "CHAR_LENGTH" => Funcao::Comprimento,
            "ROUND" => Funcao::Arredondar,
            "ABS" => Funcao::Absoluto,
            "COALESCE" | "IFNULL" => Funcao::Coalescer,
            _ => return None,
        })
    }
}

const FUNCOES_EXISTENTES: &str =
    "CONCAT, UPPER, LOWER, TRIM, LENGTH/CHAR_LENGTH, ROUND, ABS e COALESCE/IFNULL";

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Valor),
    Variavel(String),
    /// `NEW.coluna` (`velha: false`) ou `OLD.coluna` (`velha: true`).
    Linha {
        velha: bool,
        coluna: String,
    },
    Nao(Box<Expr>),
    Negativo(Box<Expr>),
    ENulo {
        expr: Box<Expr>,
        negado: bool,
    },
    Bin {
        op: Op2,
        a: Box<Expr>,
        b: Box<Expr>,
    },
    Chamada {
        funcao: Funcao,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2 {
    E,
    Ou,
    Comp(Comparador),
    Mais,
    Menos,
    Vezes,
    Dividir,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlvoDoSet {
    Variavel(String),
    /// `SET NEW.coluna = …`, so onde ha NEW gravavel.
    ColunaNova(String),
}

/// Tabela alvo de um `INSERT` do corpo, ja separada como o protocolo espera.
#[derive(Debug, Clone, PartialEq)]
pub struct AlvoTabela {
    /// Vazio = o database do pedido que disparou a rotina.
    pub database: String,
    /// Qualificado (`schema.tabela`) quando ha schema.
    pub tabela: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instrucao {
    Declarar {
        nome: String,
        tipo: Tipo,
        padrao: Option<Expr>,
    },
    Atribuir(Vec<(AlvoDoSet, Expr)>),
    Se {
        ramos: Vec<(Expr, Vec<Instrucao>)>,
        senao: Vec<Instrucao>,
    },
    Enquanto {
        condicao: Expr,
        corpo: Vec<Instrucao>,
    },
    Sinalizar {
        estado: String,
        mensagem: String,
    },
    Inserir {
        alvo: AlvoTabela,
        colunas: Vec<String>,
        valores: Vec<Expr>,
    },
    /// `SELECT … INTO v1[, v2] FROM …` — o texto guardado ja esta SEM o
    /// `INTO`, pronto para a camada SELECT de sempre executar.
    SelecionarPara {
        sql: String,
        alvos: Vec<String>,
        /// Onde ha VARIAVEL no lugar do literal (`WHERE id = qual`): a faixa
        /// de caracteres DENTRO de `sql` e o nome, para a execucao trocar
        /// pelo valor como literal. So a posicao de valor do WHERE e
        /// substituida — coluna nunca vira variavel, e e de proposito: a
        /// sombra de coluna por variavel e um alcapao classico do MySQL(R).
        parametros: Vec<(usize, usize, String)>,
    },
}

// ------------------------------------------------------------------ regras

/// O que um contexto pode ver e fazer. E a UNICA diferenca entre o corpo de
/// um gatilho e o de um procedimento — o resto e o mesmo interpretador.
#[derive(Debug, Clone, Copy)]
pub struct Regras {
    pub dml: bool,
    pub sinal: bool,
    pub nova_leitura: bool,
    pub nova_escrita: bool,
    pub velha_leitura: bool,
    /// Para as mensagens: "gatilho BEFORE INSERT", "procedimento".
    pub contexto: &'static str,
}

/// As regras de um gatilho, por posicao e evento.
///
/// * `BEFORE` valida e ajusta: le e grava `NEW`, le `OLD`, pode `SIGNAL` —
///   e NAO fala com o motor, porque roda com a trava de dados na mao e um
///   DML ali dentro seria o abraco mortal com a propria trava.
/// * `AFTER` audita: le `NEW`/`OLD` e pode `INSERT` — e nao tem `SIGNAL`,
///   porque a escrita ja aconteceu e nao ha transacao que a desfaca.
///   Prometer o cancelamento seria mentira; a recusa vive no `BEFORE`.
pub fn regras_de_gatilho(quando: Quando, evento: Evento) -> Regras {
    let (nova, velha) = match evento {
        Evento::Inserir => (true, false),
        Evento::Atualizar => (true, true),
        Evento::Excluir => (false, true),
    };
    match quando {
        Quando::Antes => Regras {
            dml: false,
            sinal: true,
            nova_leitura: nova,
            nova_escrita: nova,
            velha_leitura: velha,
            contexto: match evento {
                Evento::Inserir => "gatilho BEFORE INSERT",
                Evento::Atualizar => "gatilho BEFORE UPDATE",
                Evento::Excluir => "gatilho BEFORE DELETE",
            },
        },
        Quando::Depois => Regras {
            dml: true,
            sinal: false,
            nova_leitura: nova,
            nova_escrita: false,
            velha_leitura: velha,
            contexto: match evento {
                Evento::Inserir => "gatilho AFTER INSERT",
                Evento::Atualizar => "gatilho AFTER UPDATE",
                Evento::Excluir => "gatilho AFTER DELETE",
            },
        },
    }
}

pub fn regras_de_procedimento() -> Regras {
    Regras {
        dml: true,
        sinal: true,
        nova_leitura: false,
        nova_escrita: false,
        velha_leitura: false,
        contexto: "procedimento",
    }
}

// --------------------------------------------------------- comandos de topo

#[derive(Debug, Clone, PartialEq)]
pub struct GatilhoDef {
    pub nome: String,
    pub quando: Quando,
    pub evento: Evento,
    /// Vazio = o database do pedido.
    pub database: String,
    /// Qualificado (`schema.tabela`) quando ha schema.
    pub tabela: String,
    pub corpo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Entrada,
    Saida,
    EntradaESaida,
}

impl Modo {
    pub fn nome(&self) -> &'static str {
        match self {
            Modo::Entrada => "IN",
            Modo::Saida => "OUT",
            Modo::EntradaESaida => "INOUT",
        }
    }

    pub fn de_texto(t: &str) -> Option<Modo> {
        match t.to_uppercase().as_str() {
            "IN" => Some(Modo::Entrada),
            "OUT" => Some(Modo::Saida),
            "INOUT" => Some(Modo::EntradaESaida),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parametro {
    pub modo: Modo,
    pub nome: String,
    pub tipo: Tipo,
    /// O tipo como foi escrito (`DECIMAL(15,2)`), para o SHOW devolver igual.
    pub tipo_escrito: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcedimentoDef {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub corpo: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Comando {
    CriarGatilho(GatilhoDef),
    ExcluirGatilho {
        nome: String,
        se_existe: bool,
    },
    MostrarGatilhos,
    CriarProcedimento(ProcedimentoDef),
    ExcluirProcedimento {
        nome: String,
        se_existe: bool,
    },
    MostrarProcedimentos,
    Chamar {
        nome: String,
        argumentos: Vec<Valor>,
    },
}

/// Reconhece um comando de rotina. `Ok(None)` = nao e daqui — o texto segue
/// para a camada SELECT de sempre. Os verbos vizinhos que NAO cabem recusam
/// com o caminho certo, em vez de "sintaxe invalida".
pub fn comando(texto: &str) -> Result<Option<Comando>> {
    let simbolos = lexico::analisar(texto)?;
    let Some(primeiro) = simbolos.first() else {
        return Ok(None);
    };
    let verbo = primeiro.token.palavra_chave().unwrap_or_default();
    let mut p = Analisador { s: simbolos, i: 0 };
    match verbo.as_str() {
        "CREATE" => {
            p.i += 1;
            criar(&mut p, texto).map(Some)
        }
        "DROP" => {
            p.i += 1;
            excluir(&mut p).map(Some)
        }
        "CALL" => {
            p.i += 1;
            chamar(&mut p).map(Some)
        }
        "SHOW" => {
            p.i += 1;
            mostrar(&mut p).map(Some)
        }
        _ => Ok(None),
    }
}

/// O texto original a partir de um simbolo — e o corpo, verbatim.
///
/// Vem por posicao de CARACTERE (que e o que o lexico guarda), e nao por
/// reconstrucao dos tokens: reconstruir perderia aspas, espacos e caixa, e o
/// corpo guardado tem de ser exatamente o que o autor escreveu.
fn texto_a_partir(texto: &str, p: &Analisador) -> Result<String> {
    let Some(s) = p.espiar() else {
        return Err(lexico::erro(
            p.posicao_atual(),
            "esperava o corpo, e o comando acabou",
        ));
    };
    let pos = s.posicao;
    Ok(texto.chars().skip(pos).collect())
}

fn criar(p: &mut Analisador, texto: &str) -> Result<Comando> {
    let pos = p.posicao_atual();
    if p.aceitar_palavra("DEFINER") {
        return Err(lexico::erro(
            pos,
            "DEFINER nao existe aqui: gatilho roda com o poder de quem dispara \
             a escrita, e CALL com o poder de quem chama",
        ));
    }
    if p.aceitar_palavra("OR") {
        return Err(lexico::erro(
            pos,
            "CREATE OR REPLACE nao existe: exclua com DROP e crie de novo",
        ));
    }
    if p.aceitar_palavra("TRIGGER") {
        return criar_gatilho(p, texto);
    }
    if p.aceitar_palavra("PROCEDURE") {
        return criar_procedimento(p, texto);
    }
    if p.aceitar_palavra("FUNCTION") {
        return Err(lexico::erro(
            pos,
            "CREATE FUNCTION nao existe nesta camada — so TRIGGER e PROCEDURE. \
             Funcao devolveria valor dentro de expressao SQL, e a camada SELECT \
             nao avalia expressao",
        ));
    }
    Err(lexico::erro(
        pos,
        "CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela se cria pela \
         operacao criar_tabela do protocolo",
    ))
}

fn criar_gatilho(p: &mut Analisador, texto: &str) -> Result<Comando> {
    let nome = p.identificador("nome do gatilho")?;
    let pos = p.posicao_atual();
    let quando = p
        .espiar()
        .and_then(|s| s.token.palavra_chave())
        .and_then(|w| Quando::de_texto(&w))
        .ok_or_else(|| lexico::erro(pos, "esperava BEFORE ou AFTER"))?;
    p.i += 1;
    let pos = p.posicao_atual();
    let evento = p
        .espiar()
        .and_then(|s| s.token.palavra_chave())
        .and_then(|w| Evento::de_texto(&w))
        .ok_or_else(|| lexico::erro(pos, "esperava INSERT, UPDATE ou DELETE"))?;
    p.i += 1;
    p.exigir_palavra("ON")?;
    let (database, tabela) = alvo_de_tabela(p)?;
    p.exigir_palavra("FOR")?;
    p.exigir_palavra("EACH")?;
    let pos = p.posicao_atual();
    if p.aceitar_palavra("STATEMENT") {
        return Err(lexico::erro(
            pos,
            "FOR EACH STATEMENT nao existe: o gatilho e por linha (FOR EACH ROW)",
        ));
    }
    p.exigir_palavra("ROW")?;
    let pos = p.posicao_atual();
    if p.aceitar_palavra("FOLLOWS") || p.aceitar_palavra("PRECEDES") {
        return Err(lexico::erro(
            pos,
            "FOLLOWS/PRECEDES nao existe: gatilhos disparam na ordem de criacao",
        ));
    }
    let corpo = texto_a_partir(texto, p)?;
    // O corpo e conferido AGORA, contra as regras deste evento: o erro sai no
    // CREATE, com coluna, e nao no primeiro INSERT de producao.
    analisar_corpo(&corpo, &regras_de_gatilho(quando, evento))?;
    Ok(Comando::CriarGatilho(GatilhoDef {
        nome,
        quando,
        evento,
        database,
        tabela,
        corpo,
    }))
}

fn criar_procedimento(p: &mut Analisador, texto: &str) -> Result<Comando> {
    let nome = p.identificador("nome do procedimento")?;
    if !p.aceitar(&Token::AbreParen) {
        return Err(lexico::erro(
            p.posicao_atual(),
            "esperava ( com os parametros — pode ser vazio: ()",
        ));
    }
    let mut parametros = Vec::new();
    if !p.aceitar(&Token::FechaParen) {
        loop {
            let modo = match p
                .espiar()
                .and_then(|s| s.token.palavra_chave())
                .and_then(|w| Modo::de_texto(&w))
            {
                Some(m) => {
                    p.i += 1;
                    m
                }
                // Sem IN/OUT/INOUT o padrao e IN, como no MySQL(R).
                None => Modo::Entrada,
            };
            let nome_p = p.identificador("nome do parametro")?.to_lowercase();
            if parametros.iter().any(|q: &Parametro| q.nome == nome_p) {
                return Err(lexico::erro(
                    p.posicao_atual(),
                    &format!("o parametro {nome_p:?} aparece duas vezes"),
                ));
            }
            let (tipo, tipo_escrito) = tipo_declarado(p)?;
            parametros.push(Parametro {
                modo,
                nome: nome_p,
                tipo,
                tipo_escrito,
            });
            if p.aceitar(&Token::Virgula) {
                continue;
            }
            if p.aceitar(&Token::FechaParen) {
                break;
            }
            return Err(lexico::erro(
                p.posicao_atual(),
                &format!("esperava , ou ) nos parametros{}", p.mas_veio()),
            ));
        }
    }
    let pos = p.posicao_atual();
    for caracteristica in [
        "DETERMINISTIC",
        "LANGUAGE",
        "COMMENT",
        "CONTAINS",
        "READS",
        "MODIFIES",
        "SQL",
        "NOT",
    ] {
        if p.aceitar_palavra(caracteristica) {
            return Err(lexico::erro(
                pos,
                "as caracteristicas (DETERMINISTIC, LANGUAGE SQL, COMMENT…) nao \
                 existem aqui: va do ) direto ao corpo",
            ));
        }
    }
    let corpo = texto_a_partir(texto, p)?;
    analisar_corpo(&corpo, &regras_de_procedimento())?;
    Ok(Comando::CriarProcedimento(ProcedimentoDef {
        nome,
        parametros,
        corpo,
    }))
}

/// Le um tipo escrito como texto (`DECIMAL(15,2)`) — e o caminho de volta do
/// disco: o registro guarda o tipo como o autor escreveu e recompoe por aqui.
pub fn tipo_de_texto(texto: &str) -> Result<Tipo> {
    let simbolos = lexico::analisar(texto)?;
    let mut p = Analisador { s: simbolos, i: 0 };
    let (tipo, _) = tipo_declarado(&mut p)?;
    Ok(tipo)
}

/// `INT`, `DECIMAL(15,2)`, `VARCHAR(60)`… — devolve o tipo e como foi escrito.
fn tipo_declarado(p: &mut Analisador) -> Result<(Tipo, String)> {
    let pos = p.posicao_atual();
    let nome = p.identificador("tipo do parametro")?.to_uppercase();
    let mut dims: Vec<u64> = Vec::new();
    if p.aceitar(&Token::AbreParen) {
        loop {
            dims.push(p.inteiro("a dimensao do tipo")?);
            if p.aceitar(&Token::Virgula) {
                continue;
            }
            if p.aceitar(&Token::FechaParen) {
                break;
            }
            return Err(lexico::erro(p.posicao_atual(), "esperava , ou ) no tipo"));
        }
    }
    let escrito = if dims.is_empty() {
        nome.clone()
    } else {
        format!(
            "{nome}({})",
            dims.iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let tipo = match nome.as_str() {
        "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT" | "INT4" | "INT8" => Tipo::Inteiro,
        "DECIMAL" | "NUMERIC" => Tipo::Decimal {
            escala: dims.get(1).copied().unwrap_or(0).min(30) as u32,
        },
        "VARCHAR" | "CHAR" | "TEXT" | "STR" | "STRING" => Tipo::Texto,
        "BOOL" | "BOOLEAN" => Tipo::Booleano,
        // Data e como o protocolo a escreve: texto. Declarar vale; o valor e
        // o 'AAAA-MM-DD' de sempre.
        "DATE" | "DATETIME" | "TIME" => Tipo::Texto,
        outro => {
            return Err(lexico::erro(
                pos,
                &format!(
                    "o tipo {outro} nao existe em rotina; use INT, DECIMAL(p,s), \
                     VARCHAR(n), BOOL ou DATE (que viaja como texto)"
                ),
            ))
        }
    };
    Ok((tipo, escrito))
}

fn excluir(p: &mut Analisador) -> Result<Comando> {
    let pos = p.posicao_atual();
    let gatilho = if p.aceitar_palavra("TRIGGER") {
        true
    } else if p.aceitar_palavra("PROCEDURE") {
        false
    } else if p.aceitar_palavra("TABLE") {
        return Err(lexico::erro(
            pos,
            "DROP TABLE e a operacao excluir_tabela do protocolo, que exige \
             repetir o nome no campo \"confirmar\"",
        ));
    } else {
        return Err(lexico::erro(
            pos,
            "DROP nesta camada e de TRIGGER ou PROCEDURE",
        ));
    };
    let se_existe = if p.aceitar_palavra("IF") {
        p.exigir_palavra("EXISTS")?;
        true
    } else {
        false
    };
    let nome = p.identificador(if gatilho {
        "nome do gatilho"
    } else {
        "nome do procedimento"
    })?;
    fim_do_comando(p)?;
    Ok(if gatilho {
        Comando::ExcluirGatilho { nome, se_existe }
    } else {
        Comando::ExcluirProcedimento { nome, se_existe }
    })
}

fn mostrar(p: &mut Analisador) -> Result<Comando> {
    let pos = p.posicao_atual();
    if p.aceitar_palavra("TRIGGERS") {
        fim_do_comando(p)?;
        return Ok(Comando::MostrarGatilhos);
    }
    if p.aceitar_palavra("PROCEDURES") {
        fim_do_comando(p)?;
        return Ok(Comando::MostrarProcedimentos);
    }
    // A grafia do MySQL(R): SHOW PROCEDURE STATUS.
    if p.aceitar_palavra("PROCEDURE") {
        p.exigir_palavra("STATUS")?;
        fim_do_comando(p)?;
        return Ok(Comando::MostrarProcedimentos);
    }
    Err(lexico::erro(
        pos,
        "SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas \
         saem por sistabelas/siscolunas",
    ))
}

fn chamar(p: &mut Analisador) -> Result<Comando> {
    let nome = p.identificador("nome do procedimento")?;
    let mut argumentos = Vec::new();
    if p.aceitar(&Token::AbreParen) && !p.aceitar(&Token::FechaParen) {
        loop {
            let negativo = p.aceitar(&Token::Menos);
            let lit = p.literal()?;
            let valor = valor_de_literal(&lit, negativo, p.posicao_atual())?;
            argumentos.push(valor);
            if p.aceitar(&Token::Virgula) {
                continue;
            }
            if p.aceitar(&Token::FechaParen) {
                break;
            }
            return Err(lexico::erro(
                p.posicao_atual(),
                &format!("esperava , ou ) nos argumentos{}", p.mas_veio()),
            ));
        }
    }
    fim_do_comando(p)?;
    Ok(Comando::Chamar { nome, argumentos })
}

fn valor_de_literal(lit: &crate::sintaxe::Literal, negativo: bool, pos: usize) -> Result<Valor> {
    use crate::sintaxe::Literal;
    let v = match lit {
        Literal::Numero(n) => {
            let numero = Numero::de_texto(n)
                .ok_or_else(|| lexico::erro(pos, &format!("{n:?} nao e um numero")))?;
            Valor::Numero(if negativo { numero.negativo() } else { numero })
        }
        Literal::Texto(t) => Valor::Texto(t.clone()),
        Literal::Bool(b) => Valor::Bool(*b),
        Literal::Nulo => Valor::Nulo,
    };
    if negativo && !matches!(v, Valor::Numero(_)) {
        return Err(lexico::erro(pos, "o sinal - so vale para numero"));
    }
    Ok(v)
}

fn fim_do_comando(p: &mut Analisador) -> Result<()> {
    p.aceitar(&Token::PontoEVirgula);
    if let Some(sobra) = p.espiar() {
        return Err(lexico::erro(
            sobra.posicao,
            &format!(
                "sobrou {:?} depois do fim do comando",
                sobra.token.descrever()
            ),
        ));
    }
    Ok(())
}

/// `tabela`, `schema.tabela` ou `database.schema.tabela` — a MESMA regra do
/// `FROM`: duas partes sao schema e tabela, e o database vem do pedido.
fn alvo_de_tabela(p: &mut Analisador) -> Result<(String, String)> {
    let mut partes = vec![p.identificador("nome de tabela")?];
    while p.aceitar(&Token::Ponto) {
        partes.push(p.identificador("nome depois do ponto")?);
        if partes.len() > 3 {
            return Err(lexico::erro(
                p.posicao_atual(),
                "o endereco vai ate tres partes: database.schema.tabela",
            ));
        }
    }
    Ok(match partes.len() {
        1 => (String::new(), partes.remove(0)),
        2 => (String::new(), format!("{}.{}", partes[0], partes[1])),
        _ => (partes[0].clone(), format!("{}.{}", partes[1], partes[2])),
    })
}

// ------------------------------------------------------------ corpo: parser

/// Palavras que estruturam o corpo e por isso nunca sao lidas como variavel
/// dentro de expressao. Sem esta lista, `IF x THEN` com um `x` esquecido
/// engoliria o THEN como nome e o erro sairia tres tokens depois do lugar.
const PALAVRAS_DO_CORPO: [&str; 22] = [
    "IF", "THEN", "ELSEIF", "ELSE", "END", "WHILE", "DO", "SET", "DECLARE", "SIGNAL", "INSERT",
    "INTO", "VALUES", "SELECT", "CALL", "BEGIN", "AND", "OR", "NOT", "IS", "DEFAULT", "WHEN",
];

/// Le e valida um corpo inteiro contra as regras do contexto.
///
/// E chamada tres vezes na vida de uma rotina — no CREATE, ao carregar do
/// disco e nunca no disparo (o servidor guarda o compilado) — e sempre com as
/// MESMAS regras, para o que o CREATE aceitou nunca falhar depois.
pub fn analisar_corpo(texto: &str, regras: &Regras) -> Result<Vec<Instrucao>> {
    let simbolos = lexico::analisar(texto)?;
    if simbolos.is_empty() {
        return Err(PhxError::Esquema(format!(
            "o corpo do {} esta vazio",
            regras.contexto
        )));
    }
    let mut corpo = CorpoParser {
        p: Analisador { s: simbolos, i: 0 },
        texto: texto.chars().collect(),
        regras,
    };
    let instrucoes = if corpo.p.aceitar_palavra("BEGIN") {
        let lista = corpo.lista(&["END"])?;
        corpo.p.exigir_palavra("END")?;
        lista
    } else {
        // Corpo de uma instrucao so, como o MySQL(R) aceita.
        vec![corpo.instrucao()?]
    };
    corpo.p.aceitar(&Token::PontoEVirgula);
    if let Some(sobra) = corpo.p.espiar() {
        return Err(lexico::erro(
            sobra.posicao,
            &format!(
                "sobrou {:?} depois do fim do corpo — mais de uma instrucao \
                 pede BEGIN … END",
                sobra.token.descrever()
            ),
        ));
    }
    Ok(instrucoes)
}

struct CorpoParser<'a> {
    p: Analisador,
    texto: Vec<char>,
    regras: &'a Regras,
}

impl CorpoParser<'_> {
    /// Instrucoes ate uma das palavras terminadoras (que fica por consumir).
    fn lista(&mut self, terminadores: &[&str]) -> Result<Vec<Instrucao>> {
        let mut saida = Vec::new();
        loop {
            while self.p.aceitar(&Token::PontoEVirgula) {}
            let palavra = self
                .p
                .espiar()
                .and_then(|s| s.token.palavra_chave())
                .unwrap_or_default();
            if terminadores.contains(&palavra.as_str()) {
                return Ok(saida);
            }
            if self.p.espiar().is_none() {
                return Err(lexico::erro(
                    self.p.posicao_atual(),
                    &format!("o corpo acabou sem {}", terminadores.join("/")),
                ));
            }
            saida.push(self.instrucao()?);
            // O ; e obrigatorio entre instrucoes, mas nao antes do END.
            let proxima = self
                .p
                .espiar()
                .and_then(|s| s.token.palavra_chave())
                .unwrap_or_default();
            if !self.p.aceitar(&Token::PontoEVirgula)
                && !terminadores.contains(&proxima.as_str())
                && self.p.espiar().is_some()
            {
                return Err(lexico::erro(
                    self.p.posicao_atual(),
                    &format!("esperava ; entre instrucoes{}", self.p.mas_veio()),
                ));
            }
        }
    }

    fn instrucao(&mut self) -> Result<Instrucao> {
        let pos = self.p.posicao_atual();
        let palavra = self
            .p
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .unwrap_or_default();
        match palavra.as_str() {
            "DECLARE" => {
                self.p.i += 1;
                self.declarar(pos)
            }
            "SET" => {
                self.p.i += 1;
                self.atribuir()
            }
            "IF" => {
                self.p.i += 1;
                self.se()
            }
            "WHILE" => {
                self.p.i += 1;
                self.enquanto()
            }
            "SIGNAL" => {
                self.p.i += 1;
                self.sinalizar(pos)
            }
            "INSERT" => {
                self.p.i += 1;
                self.inserir(pos)
            }
            "SELECT" => self.selecionar_para(pos),
            // As recusas com nome e motivo. Cada uma existe porque alguem vai
            // colar um corpo do MySQL(R) e precisa saber o que trocar.
            "UPDATE" | "DELETE" => Err(lexico::erro(
                pos,
                &format!(
                    "{palavra} dentro de rotina ainda nao existe: o motor atualiza \
                     e exclui por rowid, e traduzir o WHERE pede o planejador. \
                     O que ja da: INSERT e SELECT … INTO"
                ),
            )),
            "CASE" => Err(lexico::erro(
                pos,
                "CASE nao existe no corpo; escreva com IF/ELSEIF/ELSE",
            )),
            "LOOP" | "REPEAT" | "ITERATE" | "LEAVE" => Err(lexico::erro(
                pos,
                &format!("{palavra} nao existe no corpo; o laco daqui e o WHILE"),
            )),
            "RETURN" => Err(lexico::erro(
                pos,
                "RETURN e de FUNCTION, que nao existe; devolva por parametro OUT",
            )),
            "OPEN" | "FETCH" | "CLOSE" => Err(lexico::erro(
                pos,
                "cursor nao existe no corpo; leia com SELECT … INTO (uma linha)",
            )),
            "CALL" => Err(lexico::erro(
                pos,
                "CALL dentro de rotina (aninhado) nao e aceito nesta versao",
            )),
            "BEGIN" => Err(lexico::erro(
                pos,
                "bloco aninhado nao e aceito: um BEGIN … END so",
            )),
            "START" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" => Err(lexico::erro(
                pos,
                "transacao e comando de SESSAO e nao cabe num corpo de rotina: \
                 ela pertence a CONEXAO, e o corpo roda dentro de UM pedido. \
                 Abra a transacao pela conexao e chame a rotina de dentro dela",
            )),
            _ => Err(lexico::erro(
                pos,
                &format!(
                    "instrucao desconhecida{}. As aceitas: DECLARE, SET, IF, \
                     WHILE, SIGNAL, INSERT, SELECT … INTO",
                    self.p.mas_veio()
                ),
            )),
        }
    }

    fn declarar(&mut self, pos: usize) -> Result<Instrucao> {
        let nome = self.p.identificador("nome da variavel")?.to_lowercase();
        // DECLARE CONTINUE/EXIT HANDLER e DECLARE … CURSOR comecam igual;
        // o que os denuncia e a palavra seguinte ao nome.
        if ["CONTINUE", "EXIT", "UNDO"].contains(&nome.to_uppercase().as_str()) {
            return Err(lexico::erro(
                pos,
                "DECLARE … HANDLER nao existe: o erro sobe para quem chamou, \
                 com codigo e mensagem",
            ));
        }
        let (tipo, _) = tipo_declarado(&mut self.p)?;
        if self.p.aceitar_palavra("CURSOR") {
            return Err(lexico::erro(pos, "cursor nao existe no corpo"));
        }
        let padrao = if self.p.aceitar_palavra("DEFAULT") {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Instrucao::Declarar { nome, tipo, padrao })
    }

    fn atribuir(&mut self) -> Result<Instrucao> {
        let mut pares = Vec::new();
        loop {
            let pos = self.p.posicao_atual();
            let alvo = self.alvo_do_set(pos)?;
            if !self.p.aceitar(&Token::Comparador(Comparador::Igual)) {
                return Err(lexico::erro(
                    self.p.posicao_atual(),
                    &format!("esperava = na atribuicao{}", self.p.mas_veio()),
                ));
            }
            let expr = self.expr()?;
            pares.push((alvo, expr));
            if !self.p.aceitar(&Token::Virgula) {
                break;
            }
        }
        Ok(Instrucao::Atribuir(pares))
    }

    fn alvo_do_set(&mut self, pos: usize) -> Result<AlvoDoSet> {
        let palavra = self
            .p
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .unwrap_or_default();
        if palavra == "NEW" {
            self.p.i += 1;
            if !self.p.aceitar(&Token::Ponto) {
                return Err(lexico::erro(pos, "esperava NEW.coluna"));
            }
            let coluna = self.p.identificador("coluna depois de NEW.")?;
            if !self.regras.nova_escrita {
                let motivo = if self.regras.nova_leitura {
                    "em AFTER a linha ja foi gravada; NEW e so leitura, e o \
                     ajuste vive no BEFORE"
                } else if self.regras.velha_leitura {
                    "nao ha NEW em DELETE — a linha esta saindo, nao entrando"
                } else {
                    "nao ha NEW em procedimento; NEW e OLD sao dos gatilhos"
                };
                return Err(lexico::erro(pos, motivo));
            }
            return Ok(AlvoDoSet::ColunaNova(coluna));
        }
        if palavra == "OLD" {
            return Err(lexico::erro(
                pos,
                "OLD e so leitura: e a linha como ela ERA, e o passado nao se edita",
            ));
        }
        let nome = self.p.identificador("variavel do SET")?.to_lowercase();
        Ok(AlvoDoSet::Variavel(nome))
    }

    fn se(&mut self) -> Result<Instrucao> {
        let mut ramos = Vec::new();
        let condicao = self.expr()?;
        self.p.exigir_palavra("THEN")?;
        let corpo = self.lista(&["ELSEIF", "ELSE", "END"])?;
        ramos.push((condicao, corpo));
        let mut senao = Vec::new();
        loop {
            if self.p.aceitar_palavra("ELSEIF") {
                let condicao = self.expr()?;
                self.p.exigir_palavra("THEN")?;
                let corpo = self.lista(&["ELSEIF", "ELSE", "END"])?;
                ramos.push((condicao, corpo));
                continue;
            }
            if self.p.aceitar_palavra("ELSE") {
                senao = self.lista(&["END"])?;
            }
            self.p.exigir_palavra("END")?;
            self.p.exigir_palavra("IF")?;
            return Ok(Instrucao::Se { ramos, senao });
        }
    }

    fn enquanto(&mut self) -> Result<Instrucao> {
        let condicao = self.expr()?;
        self.p.exigir_palavra("DO")?;
        let corpo = self.lista(&["END"])?;
        self.p.exigir_palavra("END")?;
        self.p.exigir_palavra("WHILE")?;
        Ok(Instrucao::Enquanto { condicao, corpo })
    }

    fn sinalizar(&mut self, pos: usize) -> Result<Instrucao> {
        if !self.regras.sinal {
            return Err(lexico::erro(
                pos,
                &format!(
                    "SIGNAL em {} nao desfaz nada: a escrita ja aconteceu e nao \
                     ha transacao. A recusa vive no BEFORE",
                    self.regras.contexto
                ),
            ));
        }
        self.p.exigir_palavra("SQLSTATE")?;
        let pos_estado = self.p.posicao_atual();
        let estado = match self.p.espiar().map(|s| s.token.clone()) {
            Some(Token::Texto(t)) => {
                self.p.i += 1;
                t
            }
            _ => {
                return Err(lexico::erro(
                    pos_estado,
                    "esperava o SQLSTATE entre aspas simples, ex.: '45000'",
                ))
            }
        };
        if estado.len() != 5 || !estado.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(lexico::erro(
                pos_estado,
                &format!(
                    "SQLSTATE tem 5 caracteres, e {estado:?} tem {}",
                    estado.len()
                ),
            ));
        }
        let mensagem = if self.p.aceitar_palavra("SET") {
            self.p.exigir_palavra("MESSAGE_TEXT")?;
            if !self.p.aceitar(&Token::Comparador(Comparador::Igual)) {
                return Err(lexico::erro(
                    self.p.posicao_atual(),
                    "esperava = depois de MESSAGE_TEXT",
                ));
            }
            let pos_msg = self.p.posicao_atual();
            match self.p.espiar().map(|s| s.token.clone()) {
                Some(Token::Texto(t)) => {
                    self.p.i += 1;
                    t
                }
                _ => {
                    return Err(lexico::erro(
                        pos_msg,
                        "MESSAGE_TEXT quer um texto entre aspas simples",
                    ))
                }
            }
        } else {
            format!("SQLSTATE {estado}")
        };
        Ok(Instrucao::Sinalizar { estado, mensagem })
    }

    fn inserir(&mut self, pos: usize) -> Result<Instrucao> {
        if !self.regras.dml {
            return Err(lexico::erro(
                pos,
                &format!(
                    "em {} o corpo nao fala com o motor: ele roda com a trava \
                     de dados na mao. Valide e ajuste NEW aqui; a auditoria \
                     (INSERT) vive no AFTER",
                    self.regras.contexto
                ),
            ));
        }
        self.p.exigir_palavra("INTO")?;
        let (database, tabela) = alvo_de_tabela(&mut self.p)?;
        if !self.p.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.p.posicao_atual(),
                "escreva as colunas: INSERT INTO t (a, b) VALUES (…) — sem a \
                 lista, a ordem do esquema decidiria em silencio",
            ));
        }
        let mut colunas = Vec::new();
        loop {
            colunas.push(self.p.identificador("nome de coluna")?);
            if self.p.aceitar(&Token::Virgula) {
                continue;
            }
            if self.p.aceitar(&Token::FechaParen) {
                break;
            }
            return Err(lexico::erro(
                self.p.posicao_atual(),
                "esperava , ou ) nas colunas",
            ));
        }
        self.p.exigir_palavra("VALUES")?;
        if !self.p.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.p.posicao_atual(),
                "esperava ( depois de VALUES",
            ));
        }
        let mut valores = Vec::new();
        loop {
            valores.push(self.expr()?);
            if self.p.aceitar(&Token::Virgula) {
                continue;
            }
            if self.p.aceitar(&Token::FechaParen) {
                break;
            }
            return Err(lexico::erro(
                self.p.posicao_atual(),
                "esperava , ou ) nos valores",
            ));
        }
        if valores.len() != colunas.len() {
            return Err(lexico::erro(
                pos,
                &format!(
                    "{} colunas e {} valores — as listas andam juntas",
                    colunas.len(),
                    valores.len()
                ),
            ));
        }
        if self.p.aceitar(&Token::Virgula) {
            return Err(lexico::erro(
                self.p.posicao_atual(),
                "uma linha por INSERT dentro de rotina; para carga ha o inserir_lote",
            ));
        }
        Ok(Instrucao::Inserir {
            alvo: AlvoTabela { database, tabela },
            colunas,
            valores,
        })
    }

    /// `SELECT … INTO v1[, v2] FROM …` — separa os alvos e guarda o texto do
    /// SELECT ja SEM o INTO, verbatim, para a camada SELECT de sempre rodar.
    fn selecionar_para(&mut self, pos: usize) -> Result<Instrucao> {
        if !self.regras.dml {
            return Err(lexico::erro(
                pos,
                &format!(
                    "em {} o corpo nao fala com o motor (roda com a trava na \
                     mao); leia OLD e NEW, que ja estao aqui",
                    self.regras.contexto
                ),
            ));
        }
        // O fim da instrucao: o ; em profundidade zero de parenteses, ou o
        // fim do texto. E por posicao de simbolo, para o recorte ser exato.
        let inicio = self.p.i;
        if self
            .p
            .s
            .get(inicio + 1)
            .is_some_and(|s| s.token == Token::Asterisco)
        {
            return Err(lexico::erro(
                pos,
                "SELECT * … INTO nao diz a ordem das variaveis; escreva as colunas",
            ));
        }
        let mut fim = inicio;
        let mut profundidade = 0i32;
        let mut into: Option<usize> = None;
        let mut from: Option<usize> = None;
        // As variaveis no lugar de literal: a palavra logo DEPOIS de um
        // comparador (`WHERE id = qual`). E a unica posicao da gramatica do
        // SELECT onde um valor aparece, entao a troca nunca alcanca coluna.
        let mut parametros: Vec<(usize, usize, String)> = Vec::new();
        while let Some(s) = self.p.s.get(fim) {
            match &s.token {
                Token::AbreParen => profundidade += 1,
                Token::FechaParen => profundidade -= 1,
                Token::PontoEVirgula if profundidade == 0 => break,
                Token::Palavra { citado: false, .. } if profundidade == 0 => {
                    let alto = s.token.palavra_chave().unwrap_or_default();
                    if alto == "INTO" && into.is_none() {
                        into = Some(fim);
                    }
                    if alto == "FROM" && from.is_none() {
                        from = Some(fim);
                    }
                }
                _ => {}
            }
            if fim > inicio {
                if let (
                    Some(Token::Comparador(_)),
                    Token::Palavra {
                        texto,
                        citado: false,
                    },
                ) = (self.p.s.get(fim - 1).map(|x| &x.token), &s.token)
                {
                    let alto = texto.to_uppercase();
                    if !["NULL", "TRUE", "FALSE"].contains(&alto.as_str()) {
                        parametros.push((
                            s.posicao,
                            s.posicao + texto.chars().count(),
                            texto.to_lowercase(),
                        ));
                    }
                }
            }
            fim += 1;
        }
        let Some(i_into) = into else {
            return Err(lexico::erro(
                pos,
                "SELECT dentro de rotina precisa de INTO: o resultado vai para \
                 variaveis, nao para o cliente",
            ));
        };
        let Some(i_from) = from.filter(|f| *f > i_into) else {
            return Err(lexico::erro(
                self.p.s[i_into].posicao,
                "esperava FROM depois das variaveis do INTO",
            ));
        };
        // As variaveis entre INTO e FROM.
        let mut alvos = Vec::new();
        let mut i = i_into + 1;
        while i < i_from {
            match &self.p.s[i].token {
                Token::Palavra { texto, .. } => alvos.push(texto.to_lowercase()),
                Token::Virgula => {}
                outro => {
                    return Err(lexico::erro(
                        self.p.s[i].posicao,
                        &format!("esperava variavel no INTO, e veio {:?}", outro.descrever()),
                    ))
                }
            }
            i += 1;
        }
        if alvos.is_empty() {
            return Err(lexico::erro(
                self.p.s[i_into].posicao,
                "o INTO esta sem variaveis",
            ));
        }
        // O texto do SELECT sem o pedaco INTO…: [inicio, INTO) + [FROM, fim).
        let a = self.p.s[inicio].posicao;
        let b = self.p.s[i_into].posicao;
        let c = self.p.s[i_from].posicao;
        let d = match self.p.s.get(fim) {
            Some(s) => s.posicao,
            None => self.texto.len(),
        };
        let sql: String = self.texto[a..b]
            .iter()
            .chain(self.texto[c..d].iter())
            .collect();
        // As faixas das variaveis, reancoradas no texto JA recortado: o que
        // vinha depois do FROM anda para tras o tamanho do pedaco tirado.
        let recorte = c - b;
        let parametros = parametros
            .into_iter()
            .filter(|(ini, _, _)| *ini >= c)
            .map(|(ini, fim_v, nome)| (ini - a - recorte, fim_v - a - recorte, nome))
            .collect();
        self.p.i = fim;
        Ok(Instrucao::SelecionarPara {
            sql,
            alvos,
            parametros,
        })
    }

    // -------------------------------------------------------- expressoes

    fn expr(&mut self) -> Result<Expr> {
        self.ou()
    }

    fn ou(&mut self) -> Result<Expr> {
        let mut a = self.e()?;
        while self.p.aceitar_palavra("OR") {
            let b = self.e()?;
            a = Expr::Bin {
                op: Op2::Ou,
                a: Box::new(a),
                b: Box::new(b),
            };
        }
        Ok(a)
    }

    fn e(&mut self) -> Result<Expr> {
        let mut a = self.nao()?;
        while self.p.aceitar_palavra("AND") {
            let b = self.nao()?;
            a = Expr::Bin {
                op: Op2::E,
                a: Box::new(a),
                b: Box::new(b),
            };
        }
        Ok(a)
    }

    fn nao(&mut self) -> Result<Expr> {
        if self.p.aceitar_palavra("NOT") {
            return Ok(Expr::Nao(Box::new(self.nao()?)));
        }
        self.comparacao()
    }

    fn comparacao(&mut self) -> Result<Expr> {
        let a = self.soma()?;
        if let Some(Token::Comparador(c)) = self.p.espiar().map(|s| s.token.clone()) {
            self.p.i += 1;
            let b = self.soma()?;
            return Ok(Expr::Bin {
                op: Op2::Comp(c),
                a: Box::new(a),
                b: Box::new(b),
            });
        }
        if self.p.aceitar_palavra("IS") {
            let negado = self.p.aceitar_palavra("NOT");
            self.p.exigir_palavra("NULL")?;
            return Ok(Expr::ENulo {
                expr: Box::new(a),
                negado,
            });
        }
        Ok(a)
    }

    fn soma(&mut self) -> Result<Expr> {
        let mut a = self.produto()?;
        loop {
            let op = if self.p.aceitar(&Token::Mais) {
                Op2::Mais
            } else if self.p.aceitar(&Token::Menos) {
                Op2::Menos
            } else {
                return Ok(a);
            };
            let b = self.produto()?;
            a = Expr::Bin {
                op,
                a: Box::new(a),
                b: Box::new(b),
            };
        }
    }

    fn produto(&mut self) -> Result<Expr> {
        let mut a = self.unario()?;
        loop {
            let op = if self.p.aceitar(&Token::Asterisco) {
                Op2::Vezes
            } else if self.p.aceitar(&Token::Barra) {
                Op2::Dividir
            } else {
                return Ok(a);
            };
            let b = self.unario()?;
            a = Expr::Bin {
                op,
                a: Box::new(a),
                b: Box::new(b),
            };
        }
    }

    fn unario(&mut self) -> Result<Expr> {
        if self.p.aceitar(&Token::Menos) {
            return Ok(Expr::Negativo(Box::new(self.unario()?)));
        }
        self.primario()
    }

    fn primario(&mut self) -> Result<Expr> {
        let pos = self.p.posicao_atual();
        let Some(s) = self.p.espiar() else {
            return Err(lexico::erro(pos, "esperava um valor, e o corpo acabou"));
        };
        match s.token.clone() {
            Token::Numero(n) => {
                self.p.i += 1;
                let numero = Numero::de_texto(&n)
                    .ok_or_else(|| lexico::erro(pos, &format!("{n:?} nao e um numero")))?;
                Ok(Expr::Lit(Valor::Numero(numero)))
            }
            Token::Texto(t) => {
                self.p.i += 1;
                Ok(Expr::Lit(Valor::Texto(t)))
            }
            Token::AbreParen => {
                self.p.i += 1;
                let e = self.expr()?;
                if !self.p.aceitar(&Token::FechaParen) {
                    return Err(lexico::erro(self.p.posicao_atual(), "esperava )"));
                }
                Ok(e)
            }
            Token::Palavra { texto, citado } => {
                let alto = if citado {
                    String::new()
                } else {
                    texto.to_uppercase()
                };
                match alto.as_str() {
                    "NULL" => {
                        self.p.i += 1;
                        Ok(Expr::Lit(Valor::Nulo))
                    }
                    "TRUE" => {
                        self.p.i += 1;
                        Ok(Expr::Lit(Valor::Bool(true)))
                    }
                    "FALSE" => {
                        self.p.i += 1;
                        Ok(Expr::Lit(Valor::Bool(false)))
                    }
                    "NEW" | "OLD" => {
                        let velha = alto == "OLD";
                        self.p.i += 1;
                        if !self.p.aceitar(&Token::Ponto) {
                            return Err(lexico::erro(pos, &format!("esperava {alto}.coluna")));
                        }
                        let coluna = self.p.identificador("nome de coluna")?;
                        let pode = if velha {
                            self.regras.velha_leitura
                        } else {
                            self.regras.nova_leitura
                        };
                        if !pode {
                            return Err(lexico::erro(
                                pos,
                                &format!(
                                    "nao ha {alto} em {}{}",
                                    self.regras.contexto,
                                    if self.regras.contexto == "procedimento" {
                                        " — NEW e OLD sao dos gatilhos"
                                    } else {
                                        ""
                                    }
                                ),
                            ));
                        }
                        Ok(Expr::Linha { velha, coluna })
                    }
                    _ => {
                        // Chamada de funcao?
                        let e_chamada =
                            self.p.s.get(self.p.i + 1).map(|s| &s.token) == Some(&Token::AbreParen);
                        if e_chamada && !citado {
                            if let Some(funcao) = Funcao::de_nome(&alto) {
                                self.p.i += 2;
                                let mut args = Vec::new();
                                if !self.p.aceitar(&Token::FechaParen) {
                                    loop {
                                        args.push(self.expr()?);
                                        if self.p.aceitar(&Token::Virgula) {
                                            continue;
                                        }
                                        if self.p.aceitar(&Token::FechaParen) {
                                            break;
                                        }
                                        return Err(lexico::erro(
                                            self.p.posicao_atual(),
                                            "esperava , ou ) nos argumentos",
                                        ));
                                    }
                                }
                                return Ok(Expr::Chamada { funcao, args });
                            }
                            return Err(lexico::erro(
                                pos,
                                &format!(
                                    "a funcao {texto} nao existe nesta camada; \
                                     existem: {FUNCOES_EXISTENTES}"
                                ),
                            ));
                        }
                        if !citado && PALAVRAS_DO_CORPO.contains(&alto.as_str()) {
                            return Err(lexico::erro(
                                pos,
                                &format!("esperava um valor, e veio {texto}"),
                            ));
                        }
                        self.p.i += 1;
                        Ok(Expr::Variavel(texto.to_lowercase()))
                    }
                }
            }
            outro => Err(lexico::erro(
                pos,
                &format!("esperava um valor, e veio {:?}", outro.descrever()),
            )),
        }
    }
}

// ------------------------------------------------------------- interpretar

/// Quem executa os pedidos que um corpo produz.
///
/// O servidor implementa por cima do `executar_derivado` — o MESMO portao de
/// permissao e de politica dos pedidos da rede. A rotina nunca abre tabela.
pub trait Motor {
    fn operacao(&mut self, op: &str, pedido: &Json) -> Result<Json>;
    fn consultar(&mut self, sql: &str) -> Result<Json>;
}

/// O motor de quem nao pode falar com o motor: os corpos BEFORE.
///
/// O parser ja recusa DML nesses corpos; este e o cinto alem do suspensorio —
/// se um dia uma instrucao nova esquecer a regra, ela recebe este erro em vez
/// de tomar a trava de dados duas vezes e travar o servidor.
pub struct MotorNulo;

impl Motor for MotorNulo {
    fn operacao(&mut self, _op: &str, _pedido: &Json) -> Result<Json> {
        Err(PhxError::Esquema(
            "este corpo nao fala com o motor (roda com a trava de dados na mao)".into(),
        ))
    }

    fn consultar(&mut self, _sql: &str) -> Result<Json> {
        Err(PhxError::Esquema(
            "este corpo nao fala com o motor (roda com a trava de dados na mao)".into(),
        ))
    }
}

struct Variavel {
    nome: String,
    tipo: Tipo,
    valor: Valor,
}

/// O estado de UMA execucao de corpo.
pub struct Contexto {
    variaveis: Vec<Variavel>,
    /// A linha NOVA como objeto JSON, quando o evento tem uma.
    pub nova: Option<Json>,
    nova_gravavel: bool,
    /// A linha VELHA, so leitura.
    pub velha: Option<Json>,
    /// As colunas que o corpo alterou via `SET NEW.…`, com o nome REAL da
    /// coluna no esquema — e o que o servidor aplica de volta na linha.
    pub tocadas: Vec<String>,
    passos: u64,
}

impl Contexto {
    pub fn de_gatilho(nova: Option<Json>, nova_gravavel: bool, velha: Option<Json>) -> Contexto {
        Contexto {
            variaveis: Vec::new(),
            nova,
            nova_gravavel,
            velha,
            tocadas: Vec::new(),
            passos: 0,
        }
    }

    pub fn de_procedimento(parametros: Vec<(String, Tipo, Valor)>) -> Contexto {
        Contexto {
            variaveis: parametros
                .into_iter()
                .map(|(nome, tipo, valor)| Variavel {
                    nome: nome.to_lowercase(),
                    tipo,
                    valor,
                })
                .collect(),
            nova: None,
            nova_gravavel: false,
            velha: None,
            tocadas: Vec::new(),
            passos: 0,
        }
    }

    pub fn valor_de(&self, nome: &str) -> Option<&Valor> {
        let nome = nome.to_lowercase();
        self.variaveis
            .iter()
            .find(|v| v.nome == nome)
            .map(|v| &v.valor)
    }

    fn passo(&mut self) -> Result<()> {
        self.passos += 1;
        if self.passos > PASSOS_MAX {
            return Err(PhxError::LimiteExcedido(format!(
                "o corpo passou de {PASSOS_MAX} passos; ha um WHILE sem fim?"
            )));
        }
        Ok(())
    }

    fn atribuir_variavel(&mut self, nome: &str, valor: Valor) -> Result<()> {
        let v = self
            .variaveis
            .iter_mut()
            .find(|v| v.nome == nome)
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a variavel {nome:?} nao foi declarada (DECLARE {nome} TIPO;)"
                ))
            })?;
        v.valor = v.tipo.coagir(valor)?;
        Ok(())
    }

    /// Le `NEW.coluna`/`OLD.coluna` — o nome compara sem caixa, como o SQL.
    fn coluna_da_linha(&self, velha: bool, coluna: &str) -> Result<Valor> {
        let objeto = if velha { &self.velha } else { &self.nova };
        let rotulo = if velha { "OLD" } else { "NEW" };
        let Some(Json::Objeto(pares)) = objeto else {
            return Err(PhxError::Esquema(format!("nao ha {rotulo} aqui")));
        };
        let achado = pares
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(coluna))
            .map(|(_, v)| v);
        match achado {
            Some(v) => Valor::de_json(v),
            None => Err(PhxError::Esquema(format!(
                "a tabela nao tem coluna {coluna:?} (em {rotulo}.{coluna})"
            ))),
        }
    }

    fn gravar_coluna_nova(&mut self, coluna: &str, valor: &Valor) -> Result<()> {
        if !self.nova_gravavel {
            return Err(PhxError::Esquema("NEW e so leitura aqui".into()));
        }
        let Some(Json::Objeto(pares)) = &mut self.nova else {
            return Err(PhxError::Esquema("nao ha NEW aqui".into()));
        };
        let Some(par) = pares
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(coluna))
        else {
            return Err(PhxError::Esquema(format!(
                "a tabela nao tem coluna {coluna:?} (em SET NEW.{coluna})"
            )));
        };
        par.1 = valor.para_json();
        let nome_real = par.0.clone();
        if !self.tocadas.contains(&nome_real) {
            self.tocadas.push(nome_real);
        }
        Ok(())
    }
}

/// Executa um corpo. `SIGNAL` sai como [`PhxError::Sinal`], que o chamador
/// decide o que significa — no BEFORE, cancelar a escrita.
pub fn executar(corpo: &[Instrucao], ctx: &mut Contexto, motor: &mut dyn Motor) -> Result<()> {
    for instrucao in corpo {
        executar_uma(instrucao, ctx, motor)?;
    }
    Ok(())
}

fn executar_uma(i: &Instrucao, ctx: &mut Contexto, motor: &mut dyn Motor) -> Result<()> {
    ctx.passo()?;
    match i {
        Instrucao::Declarar { nome, tipo, padrao } => {
            if ctx.variaveis.iter().any(|v| v.nome == *nome) {
                return Err(PhxError::Esquema(format!(
                    "a variavel {nome:?} ja foi declarada"
                )));
            }
            let valor = match padrao {
                Some(e) => tipo.coagir(avaliar(e, ctx)?)?,
                None => Valor::Nulo,
            };
            ctx.variaveis.push(Variavel {
                nome: nome.clone(),
                tipo: *tipo,
                valor,
            });
        }
        Instrucao::Atribuir(pares) => {
            for (alvo, expr) in pares {
                let valor = avaliar(expr, ctx)?;
                match alvo {
                    AlvoDoSet::Variavel(nome) => ctx.atribuir_variavel(nome, valor)?,
                    AlvoDoSet::ColunaNova(coluna) => ctx.gravar_coluna_nova(coluna, &valor)?,
                }
            }
        }
        Instrucao::Se { ramos, senao } => {
            for (condicao, corpo) in ramos {
                if verdadeiro(&avaliar(condicao, ctx)?) {
                    return executar(corpo, ctx, motor);
                }
            }
            executar(senao, ctx, motor)?;
        }
        Instrucao::Enquanto { condicao, corpo } => {
            while verdadeiro(&avaliar(condicao, ctx)?) {
                ctx.passo()?;
                executar(corpo, ctx, motor)?;
            }
        }
        Instrucao::Sinalizar { estado, mensagem } => {
            return Err(PhxError::Sinal {
                estado: estado.clone(),
                mensagem: mensagem.clone(),
            });
        }
        Instrucao::Inserir {
            alvo,
            colunas,
            valores,
        } => {
            let mut pares = Vec::with_capacity(colunas.len() + 2);
            if !alvo.database.is_empty() {
                pares.push(("database".to_string(), Json::texto_de(&alvo.database)));
            }
            pares.push(("tabela".to_string(), Json::texto_de(&alvo.tabela)));
            let mut linha = Vec::with_capacity(colunas.len());
            for (coluna, expr) in colunas.iter().zip(valores) {
                linha.push((coluna.clone(), avaliar(expr, ctx)?.para_json()));
            }
            pares.push(("valores".to_string(), Json::Objeto(linha)));
            motor.operacao("inserir", &Json::Objeto(pares))?;
        }
        Instrucao::SelecionarPara {
            sql,
            alvos,
            parametros,
        } => {
            // As variaveis do WHERE viram literais no texto, de tras para
            // frente para as faixas nao andarem. Variavel que nao existe
            // recusa com a receita — deixa-la passar viraria o erro torto
            // "comparar coluna com coluna" da camada de baixo.
            let sql = if parametros.is_empty() {
                sql.clone()
            } else {
                let mut chars: Vec<char> = sql.chars().collect();
                for (ini, fim, nome) in parametros.iter().rev() {
                    let valor = ctx.valor_de(nome).cloned().ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a variavel {nome:?} do SELECT nao foi declarada \
                             (DECLARE {nome} TIPO;)"
                        ))
                    })?;
                    let literal: Vec<char> = valor.escrever_literal_sql().chars().collect();
                    chars.splice(ini..fim, literal);
                }
                chars.into_iter().collect()
            };
            let resposta = motor.consultar(&sql)?;
            if let Some(contagem) = resposta.campo("contagem") {
                if alvos.len() != 1 {
                    return Err(PhxError::Esquema(format!(
                        "COUNT(*) devolve UM valor, e o INTO tem {} variaveis",
                        alvos.len()
                    )));
                }
                let valor = Valor::de_json(contagem)?;
                return ctx.atribuir_variavel(&alvos[0], valor);
            }
            let colunas: Vec<String> = resposta
                .campo("colunas")
                .and_then(Json::lista)
                .map(|l| {
                    l.iter()
                        .map(|c| c.texto().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            if colunas.len() != alvos.len() {
                return Err(PhxError::Esquema(format!(
                    "o SELECT devolve {} colunas e o INTO tem {} variaveis",
                    colunas.len(),
                    alvos.len()
                )));
            }
            let linhas = resposta
                .campo("linhas")
                .and_then(Json::lista)
                .unwrap_or(&[]);
            if linhas.len() > 1 {
                return Err(PhxError::Esquema(format!(
                    "o SELECT … INTO devolveu {} linhas; limite a UMA (LIMIT 1)",
                    linhas.len()
                )));
            }
            for (alvo, coluna) in alvos.iter().zip(&colunas) {
                let valor = match linhas.first() {
                    // Sem linha, as variaveis ficam NULL — e quem quer saber
                    // se achou compara com IS NULL. Documentado.
                    None => Valor::Nulo,
                    Some(l) => Valor::de_json(l.campo(coluna).unwrap_or(&Json::Nulo))?,
                };
                ctx.atribuir_variavel(alvo, valor)?;
            }
        }
    }
    Ok(())
}

/// So `TRUE` executa; `NULL` e a incerteza, e incerteza nao dispara ramo.
fn verdadeiro(v: &Valor) -> bool {
    matches!(v, Valor::Bool(true))
}

fn avaliar(e: &Expr, ctx: &mut Contexto) -> Result<Valor> {
    ctx.passo()?;
    Ok(match e {
        Expr::Lit(v) => v.clone(),
        Expr::Variavel(nome) => ctx.valor_de(nome).cloned().ok_or_else(|| {
            PhxError::Esquema(format!(
                "a variavel {nome:?} nao foi declarada (DECLARE {nome} TIPO;)"
            ))
        })?,
        Expr::Linha { velha, coluna } => ctx.coluna_da_linha(*velha, coluna)?,
        Expr::Nao(a) => match avaliar(a, ctx)? {
            Valor::Bool(b) => Valor::Bool(!b),
            Valor::Nulo => Valor::Nulo,
            _ => return Err(PhxError::Tipo("NOT quer um booleano".into())),
        },
        Expr::Negativo(a) => {
            let n = avaliar(a, ctx)?.como_numero("o sinal -")?;
            Valor::Numero(n.negativo())
        }
        Expr::ENulo { expr, negado } => {
            let v = avaliar(expr, ctx)?;
            let nulo = v == Valor::Nulo;
            Valor::Bool(nulo != *negado)
        }
        Expr::Bin { op, a, b } => {
            let va = avaliar(a, ctx)?;
            match op {
                // AND e OR em tres valores, como o SQL: o dominante decide
                // sozinho, e o NULL so sobrevive quando nada domina.
                Op2::E => {
                    if va == Valor::Bool(false) {
                        return Ok(Valor::Bool(false));
                    }
                    let vb = avaliar(b, ctx)?;
                    match (bool_ou_nulo(&va)?, bool_ou_nulo(&vb)?) {
                        (_, Some(false)) => Valor::Bool(false),
                        (Some(true), Some(true)) => Valor::Bool(true),
                        _ => Valor::Nulo,
                    }
                }
                Op2::Ou => {
                    if va == Valor::Bool(true) {
                        return Ok(Valor::Bool(true));
                    }
                    let vb = avaliar(b, ctx)?;
                    match (bool_ou_nulo(&va)?, bool_ou_nulo(&vb)?) {
                        (_, Some(true)) => Valor::Bool(true),
                        (Some(false), Some(false)) => Valor::Bool(false),
                        _ => Valor::Nulo,
                    }
                }
                Op2::Comp(c) => {
                    let vb = avaliar(b, ctx)?;
                    comparar(&va, &vb, *c)?
                }
                Op2::Mais | Op2::Menos | Op2::Vezes | Op2::Dividir => {
                    let vb = avaliar(b, ctx)?;
                    if va == Valor::Nulo || vb == Valor::Nulo {
                        return Ok(Valor::Nulo);
                    }
                    let x = va.como_numero("a conta")?;
                    let y = vb.como_numero("a conta")?;
                    Valor::Numero(match op {
                        Op2::Mais => Numero::somar(x, y)?,
                        Op2::Menos => Numero::subtrair(x, y)?,
                        Op2::Vezes => Numero::multiplicar(x, y)?,
                        _ => Numero::dividir(x, y)?,
                    })
                }
            }
        }
        Expr::Chamada { funcao, args } => {
            let mut valores = Vec::with_capacity(args.len());
            for a in args {
                valores.push(avaliar(a, ctx)?);
            }
            chamar_funcao(*funcao, &valores)?
        }
    })
}

fn bool_ou_nulo(v: &Valor) -> Result<Option<bool>> {
    match v {
        Valor::Bool(b) => Ok(Some(*b)),
        Valor::Nulo => Ok(None),
        _ => Err(PhxError::Tipo(
            "AND/OR querem booleanos (compare antes)".into(),
        )),
    }
}

/// Compara como o SQL: numero com numero (texto numerico vale), texto com
/// texto (sensivel a caixa — diferente do MySQL(R), e documentado), booleano
/// com booleano. `NULL` de qualquer lado da `NULL`.
fn comparar(a: &Valor, b: &Valor, c: Comparador) -> Result<Valor> {
    use std::cmp::Ordering;
    if *a == Valor::Nulo || *b == Valor::Nulo {
        return Ok(Valor::Nulo);
    }
    let ordem: Ordering = match (a, b) {
        (Valor::Numero(_), _) | (_, Valor::Numero(_)) => Numero::comparar(
            a.como_numero("a comparacao")?,
            b.como_numero("a comparacao")?,
        )?,
        (Valor::Texto(x), Valor::Texto(y)) => x.cmp(y),
        (Valor::Bool(x), Valor::Bool(y)) => x.cmp(y),
        _ => {
            return Err(PhxError::Tipo(
                "comparacao entre tipos diferentes (texto com booleano?)".into(),
            ))
        }
    };
    Ok(Valor::Bool(match c {
        Comparador::Igual => ordem == Ordering::Equal,
        Comparador::Diferente => ordem != Ordering::Equal,
        Comparador::Menor => ordem == Ordering::Less,
        Comparador::MenorIgual => ordem != Ordering::Greater,
        Comparador::Maior => ordem == Ordering::Greater,
        Comparador::MaiorIgual => ordem != Ordering::Less,
    }))
}

fn chamar_funcao(f: Funcao, args: &[Valor]) -> Result<Valor> {
    let exigir = |n: usize| -> Result<()> {
        if args.len() != n {
            return Err(PhxError::Esquema(format!(
                "a funcao espera {n} argumento(s), e vieram {}",
                args.len()
            )));
        }
        Ok(())
    };
    Ok(match f {
        Funcao::Concat => {
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
                _ => Valor::Numero(Numero::inteiro(t.chars().count() as i64)),
            }
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
                Some(v) => {
                    let c = v.como_numero("as casas do ROUND")?.como_i64()?;
                    if !(0..=30).contains(&c) {
                        return Err(PhxError::Esquema("ROUND aceita de 0 a 30 casas".into()));
                    }
                    c as u32
                }
                None => 0,
            };
            Valor::Numero(n.com_escala(casas)?)
        }
        Funcao::Absoluto => {
            exigir(1)?;
            if args[0] == Valor::Nulo {
                return Ok(Valor::Nulo);
            }
            Valor::Numero(args[0].como_numero("ABS")?.absoluto())
        }
        Funcao::Coalescer => {
            if args.is_empty() {
                return Err(PhxError::Esquema("COALESCE sem argumentos".into()));
            }
            args.iter()
                .find(|a| **a != Valor::Nulo)
                .cloned()
                .unwrap_or(Valor::Nulo)
        }
    })
}

// -------------------------------------------------------------------- testes

#[cfg(test)]
mod testes {
    use super::*;

    fn corpo_de_procedimento(texto: &str) -> Vec<Instrucao> {
        analisar_corpo(texto, &regras_de_procedimento()).unwrap()
    }

    fn rodar(texto: &str, params: Vec<(String, Tipo, Valor)>) -> Contexto {
        let corpo = corpo_de_procedimento(texto);
        let mut ctx = Contexto::de_procedimento(params);
        executar(&corpo, &mut ctx, &mut MotorNulo).unwrap();
        ctx
    }

    // ------------------------------------------------------------- numeros

    #[test]
    fn a_conta_decimal_e_exata() {
        // O caso que denunciaria f64: 1.10 * 3 tem de dar 3.30, nao 3.3000000000000003.
        let a = Numero::de_texto("1.10").unwrap();
        let tres = Numero::inteiro(3);
        assert_eq!(Numero::multiplicar(a, tres).unwrap().escrever(), "3.30");
        // E 0.1 + 0.2 da 0.3 — a soma que o f64 erra.
        let x = Numero::de_texto("0.1").unwrap();
        let y = Numero::de_texto("0.2").unwrap();
        assert_eq!(Numero::somar(x, y).unwrap().escrever(), "0.3");
    }

    #[test]
    fn a_divisao_arredonda_com_quatro_casas_a_mais() {
        let a = Numero::inteiro(10);
        let b = Numero::inteiro(3);
        assert_eq!(Numero::dividir(a, b).unwrap().escrever(), "3.3333");
        let a = Numero::de_texto("1.00").unwrap();
        let b = Numero::inteiro(3);
        assert_eq!(Numero::dividir(a, b).unwrap().escrever(), "0.333333");
    }

    #[test]
    fn arredondar_meio_para_longe_do_zero() {
        let n = Numero::de_texto("0.005").unwrap();
        assert_eq!(n.com_escala(2).unwrap().escrever(), "0.01");
        let n = Numero::de_texto("-0.005").unwrap();
        assert_eq!(n.com_escala(2).unwrap().escrever(), "-0.01");
        let n = Numero::de_texto("1.004").unwrap();
        assert_eq!(n.com_escala(2).unwrap().escrever(), "1.00");
    }

    #[test]
    fn divisao_por_zero_recusa() {
        let e = Numero::dividir(Numero::inteiro(1), Numero::inteiro(0)).unwrap_err();
        assert!(e.to_string().contains("zero"), "{e}");
    }

    // -------------------------------------------------------------- parser

    #[test]
    fn create_trigger_inteiro() {
        let c = comando(
            "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.uf = UPPER(NEW.uf)",
        )
        .unwrap()
        .unwrap();
        let Comando::CriarGatilho(g) = c else {
            panic!("esperava gatilho")
        };
        assert_eq!(g.nome, "normaliza");
        assert_eq!(g.quando, Quando::Antes);
        assert_eq!(g.evento, Evento::Inserir);
        assert_eq!(g.tabela, "clientes");
        assert!(g.corpo.starts_with("SET NEW.uf"));
    }

    #[test]
    fn select_nao_e_comando_de_rotina() {
        assert_eq!(comando("SELECT * FROM t").unwrap(), None);
    }

    #[test]
    fn create_procedure_com_parametros() {
        let c = comando(
            "CREATE PROCEDURE somar(IN ate INT, OUT total DECIMAL(15,2)) BEGIN \
             SET total = 0; END",
        )
        .unwrap()
        .unwrap();
        let Comando::CriarProcedimento(p) = c else {
            panic!("esperava procedimento")
        };
        assert_eq!(p.nome, "somar");
        assert_eq!(p.parametros.len(), 2);
        assert_eq!(p.parametros[0].modo, Modo::Entrada);
        assert_eq!(p.parametros[1].modo, Modo::Saida);
        assert_eq!(p.parametros[1].tipo, Tipo::Decimal { escala: 2 });
        assert_eq!(p.parametros[1].tipo_escrito, "DECIMAL(15,2)");
    }

    #[test]
    fn call_com_argumentos() {
        let c = comando("CALL somar(10, -2.5, 'a', NULL)").unwrap().unwrap();
        let Comando::Chamar { nome, argumentos } = c else {
            panic!("esperava CALL")
        };
        assert_eq!(nome, "somar");
        assert_eq!(argumentos.len(), 4);
        assert_eq!(
            argumentos[1],
            Valor::Numero(Numero::de_texto("-2.5").unwrap())
        );
        assert_eq!(argumentos[3], Valor::Nulo);
    }

    #[test]
    fn drop_e_show() {
        assert_eq!(
            comando("DROP TRIGGER IF EXISTS x").unwrap().unwrap(),
            Comando::ExcluirGatilho {
                nome: "x".into(),
                se_existe: true
            }
        );
        assert_eq!(
            comando("SHOW TRIGGERS").unwrap().unwrap(),
            Comando::MostrarGatilhos
        );
        assert_eq!(
            comando("SHOW PROCEDURE STATUS").unwrap().unwrap(),
            Comando::MostrarProcedimentos
        );
    }

    /// **O que nao cabe recusa pelo nome** — a regra da camada SELECT vale
    /// aqui tambem, porque quem cola um corpo do MySQL(R) precisa saber o que
    /// trocar, e "sintaxe invalida" nao diz.
    #[test]
    fn o_que_nao_cabe_recusa_pelo_nome() {
        for (sql, pedaco) in [
            (
                "CREATE TRIGGER t BEFORE INSERT ON c FOR EACH ROW BEGIN \
                 INSERT INTO log (a) VALUES (1); END",
                "AFTER",
            ),
            (
                "CREATE TRIGGER t AFTER INSERT ON c FOR EACH ROW BEGIN \
                 SIGNAL SQLSTATE '45000'; END",
                "BEFORE",
            ),
            (
                "CREATE TRIGGER t AFTER INSERT ON c FOR EACH ROW SET NEW.a = 1",
                "so leitura",
            ),
            (
                "CREATE TRIGGER t BEFORE DELETE ON c FOR EACH ROW SET NEW.a = 1",
                "nao ha NEW em DELETE",
            ),
            ("CREATE PROCEDURE p() SET NEW.a = 1", "procedimento"),
            (
                "CREATE PROCEDURE p() BEGIN UPDATE t SET a = 1; END",
                "UPDATE",
            ),
            ("CREATE PROCEDURE p() BEGIN DELETE FROM t; END", "DELETE"),
            ("CREATE PROCEDURE p() BEGIN COMMIT; END", "transacao"),
            (
                "CREATE PROCEDURE p() BEGIN CASE WHEN 1 THEN SET a = 1; END CASE; END",
                "IF/ELSEIF/ELSE",
            ),
            (
                "CREATE DEFINER=root TRIGGER t BEFORE INSERT ON c FOR EACH ROW SET NEW.a=1",
                "DEFINER",
            ),
            ("CREATE FUNCTION f() RETURNS INT RETURN 1", "FUNCTION"),
            (
                "CREATE TRIGGER t BEFORE INSERT ON c FOR EACH STATEMENT SET NEW.a=1",
                "FOR EACH ROW",
            ),
            ("CREATE PROCEDURE p() BEGIN SELECT nome FROM t; END", "INTO"),
            (
                "CREATE PROCEDURE p() BEGIN SELECT * INTO v FROM t; END",
                "colunas",
            ),
            (
                "CREATE PROCEDURE p() BEGIN SET v = FLOOR(1.5); END",
                "FLOOR",
            ),
            (
                "CREATE PROCEDURE p() DETERMINISTIC BEGIN SET v = 1; END",
                "caracteristicas",
            ),
            ("DROP TABLE clientes", "excluir_tabela"),
        ] {
            let e = comando(sql).unwrap_err().to_string();
            assert!(e.contains(pedaco), "{sql}\n  esperava {pedaco:?} em: {e}");
        }
    }

    #[test]
    fn corpo_com_duas_instrucoes_sem_begin_recusa() {
        let e = comando("CREATE PROCEDURE p() SET a = 1; SET b = 2;")
            .unwrap_err()
            .to_string();
        assert!(e.contains("BEGIN"), "{e}");
    }

    // --------------------------------------------------------- interprete

    #[test]
    fn while_somando_de_um_ate_n() {
        let ctx = rodar(
            "BEGIN \
               DECLARE i INT DEFAULT 1; \
               WHILE i <= ate DO \
                 SET total = total + i; \
                 SET i = i + 1; \
               END WHILE; \
             END",
            vec![
                (
                    "ate".into(),
                    Tipo::Inteiro,
                    Valor::Numero(Numero::inteiro(100)),
                ),
                (
                    "total".into(),
                    Tipo::Inteiro,
                    Valor::Numero(Numero::inteiro(0)),
                ),
            ],
        );
        assert_eq!(
            ctx.valor_de("total"),
            Some(&Valor::Numero(Numero::inteiro(5050)))
        );
    }

    #[test]
    fn if_elseif_else() {
        for (nota, esperado) in [(95, "otimo"), (75, "bom"), (30, "ruim")] {
            let ctx = rodar(
                "IF nota >= 90 THEN SET faixa = 'otimo'; \
                 ELSEIF nota >= 70 THEN SET faixa = 'bom'; \
                 ELSE SET faixa = 'ruim'; END IF",
                vec![
                    (
                        "nota".into(),
                        Tipo::Inteiro,
                        Valor::Numero(Numero::inteiro(nota)),
                    ),
                    ("faixa".into(), Tipo::Texto, Valor::Nulo),
                ],
            );
            assert_eq!(ctx.valor_de("faixa"), Some(&Valor::Texto(esperado.into())));
        }
    }

    #[test]
    fn o_dinheiro_nao_passa_por_f64() {
        // O teste que da nome a decisao: 10% sobre 1500.00, arredondado a 2
        // casas, tem de dar 1650.00 exato.
        let ctx = rodar(
            "SET total = ROUND(preco * 1.1, 2)",
            vec![
                (
                    "preco".into(),
                    Tipo::Decimal { escala: 2 },
                    Valor::Texto("1500.00".into()),
                ),
                ("total".into(), Tipo::Decimal { escala: 2 }, Valor::Nulo),
            ],
        );
        let Some(Valor::Numero(n)) = ctx.valor_de("total") else {
            panic!("total nao e numero")
        };
        assert_eq!(n.escrever(), "1650.00");
    }

    #[test]
    fn o_parametro_texto_coage_para_o_tipo_declarado() {
        // Decimal chega do protocolo como texto; o parametro DECIMAL o
        // transforma em numero exato na entrada.
        let ctx = Contexto::de_procedimento(vec![(
            "p".into(),
            Tipo::Decimal { escala: 2 },
            Valor::Texto("12.34".into()),
        )]);
        // A coercao acontece via atribuicao; a semente entra crua. Confere-se
        // pelo uso em conta:
        let corpo = corpo_de_procedimento("SET p = p + 1");
        let mut ctx = ctx;
        executar(&corpo, &mut ctx, &mut MotorNulo).unwrap();
        let Some(Valor::Numero(n)) = ctx.valor_de("p") else {
            panic!()
        };
        assert_eq!(n.escrever(), "13.34");
    }

    #[test]
    fn sinal_sai_como_erro_proprio() {
        let corpo = corpo_de_procedimento(
            "IF x < 0 THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'negativo nao'; END IF",
        );
        let mut ctx = Contexto::de_procedimento(vec![(
            "x".into(),
            Tipo::Inteiro,
            Valor::Numero(Numero::inteiro(-1)),
        )]);
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        let PhxError::Sinal { estado, mensagem } = &e else {
            panic!("esperava Sinal, veio {e}")
        };
        assert_eq!(estado, "45000");
        assert_eq!(mensagem, "negativo nao");
    }

    #[test]
    fn while_sem_fim_para_no_teto_de_passos() {
        let corpo = corpo_de_procedimento("WHILE TRUE DO SET x = x + 1; END WHILE");
        let mut ctx = Contexto::de_procedimento(vec![(
            "x".into(),
            Tipo::Inteiro,
            Valor::Numero(Numero::inteiro(0)),
        )]);
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        assert!(e.to_string().contains("passos"), "{e}");
    }

    #[test]
    fn null_se_propaga_na_conta_e_nao_dispara_if() {
        let ctx = rodar(
            "BEGIN \
               SET y = x + 1; \
               IF y > 0 THEN SET rodou = 1; END IF; \
               IF y IS NULL THEN SET nulo = 1; END IF; \
             END",
            vec![
                ("x".into(), Tipo::Inteiro, Valor::Nulo),
                ("y".into(), Tipo::Inteiro, Valor::Nulo),
                (
                    "rodou".into(),
                    Tipo::Inteiro,
                    Valor::Numero(Numero::inteiro(0)),
                ),
                (
                    "nulo".into(),
                    Tipo::Inteiro,
                    Valor::Numero(Numero::inteiro(0)),
                ),
            ],
        );
        assert_eq!(ctx.valor_de("y"), Some(&Valor::Nulo));
        assert_eq!(
            ctx.valor_de("rodou"),
            Some(&Valor::Numero(Numero::inteiro(0))),
            "IF sobre NULL nao pode disparar"
        );
        assert_eq!(
            ctx.valor_de("nulo"),
            Some(&Valor::Numero(Numero::inteiro(1)))
        );
    }

    #[test]
    fn gatilho_before_altera_new_e_registra_a_coluna_tocada() {
        let regras = regras_de_gatilho(Quando::Antes, Evento::Inserir);
        let corpo = analisar_corpo(
            "BEGIN \
               SET NEW.uf = UPPER(TRIM(NEW.uf)); \
               IF NEW.limite IS NULL THEN SET NEW.limite = '100.00'; END IF; \
             END",
            &regras,
        )
        .unwrap();
        let nova = Json::Objeto(vec![
            ("uf".into(), Json::texto_de("  sc ")),
            ("limite".into(), Json::Nulo),
        ]);
        let mut ctx = Contexto::de_gatilho(Some(nova), true, None);
        executar(&corpo, &mut ctx, &mut MotorNulo).unwrap();
        let nova = ctx.nova.unwrap();
        assert_eq!(nova.texto_ou("uf", ""), "SC");
        assert_eq!(nova.texto_ou("limite", ""), "100.00");
        assert_eq!(ctx.tocadas, vec!["uf".to_string(), "limite".to_string()]);
    }

    #[test]
    fn new_respeita_a_caixa_do_esquema_mas_aceita_a_do_autor() {
        let regras = regras_de_gatilho(Quando::Antes, Evento::Inserir);
        let corpo = analisar_corpo("SET NEW.CIDADE = 'X'", &regras).unwrap();
        let nova = Json::Objeto(vec![("Cidade".into(), Json::texto_de("y"))]);
        let mut ctx = Contexto::de_gatilho(Some(nova), true, None);
        executar(&corpo, &mut ctx, &mut MotorNulo).unwrap();
        // A coluna tocada volta com o nome REAL do esquema.
        assert_eq!(ctx.tocadas, vec!["Cidade".to_string()]);
    }

    #[test]
    fn coluna_que_nao_existe_recusa_com_o_nome() {
        let regras = regras_de_gatilho(Quando::Antes, Evento::Inserir);
        let corpo = analisar_corpo("SET NEW.naoexiste = 1", &regras).unwrap();
        let nova = Json::Objeto(vec![("uf".into(), Json::texto_de("sc"))]);
        let mut ctx = Contexto::de_gatilho(Some(nova), true, None);
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        assert!(e.to_string().contains("naoexiste"), "{e}");
    }

    #[test]
    fn old_e_legivel_em_update() {
        let regras = regras_de_gatilho(Quando::Antes, Evento::Atualizar);
        let corpo = analisar_corpo(
            "IF NEW.salario > OLD.salario * 2 THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'aumento suspeito'; \
             END IF",
            &regras,
        )
        .unwrap();
        let nova = Json::Objeto(vec![("salario".into(), Json::texto_de("30.00"))]);
        let velha = Json::Objeto(vec![("salario".into(), Json::texto_de("10.00"))]);
        let mut ctx = Contexto::de_gatilho(Some(nova), true, Some(velha));
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        assert!(matches!(e, PhxError::Sinal { .. }), "{e}");
    }

    /// O cinto alem do suspensorio: se uma instrucao com DML escapasse para um
    /// corpo BEFORE, o MotorNulo recusa em vez de tomar a trava duas vezes.
    #[test]
    fn motor_nulo_recusa_em_vez_de_travar() {
        let corpo = vec![Instrucao::Inserir {
            alvo: AlvoTabela {
                database: String::new(),
                tabela: "log".into(),
            },
            colunas: vec!["a".into()],
            valores: vec![Expr::Lit(Valor::Numero(Numero::inteiro(1)))],
        }];
        let mut ctx = Contexto::de_gatilho(None, false, None);
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        assert!(e.to_string().contains("trava"), "{e}");
    }

    #[test]
    fn concat_com_null_da_null() {
        let ctx = rodar(
            "SET r = CONCAT('a', x, 'b')",
            vec![
                ("x".into(), Tipo::Texto, Valor::Nulo),
                ("r".into(), Tipo::Texto, Valor::Nulo),
            ],
        );
        assert_eq!(ctx.valor_de("r"), Some(&Valor::Nulo));
    }

    #[test]
    fn coalesce_pega_o_primeiro_vivo() {
        let ctx = rodar(
            "SET r = COALESCE(x, 'padrao')",
            vec![
                ("x".into(), Tipo::Texto, Valor::Nulo),
                ("r".into(), Tipo::Texto, Valor::Nulo),
            ],
        );
        assert_eq!(ctx.valor_de("r"), Some(&Valor::Texto("padrao".into())));
    }

    #[test]
    fn variavel_nao_declarada_recusa_com_a_receita() {
        let corpo = corpo_de_procedimento("SET total = 1");
        let mut ctx = Contexto::de_procedimento(vec![]);
        let e = executar(&corpo, &mut ctx, &mut MotorNulo).unwrap_err();
        assert!(e.to_string().contains("DECLARE"), "{e}");
    }

    #[test]
    fn o_corpo_guardado_e_verbatim() {
        // O corpo vai para o disco como o autor escreveu — com aspas, caixa e
        // espacos — porque ele volta no SHOW e sera recompilado ao carregar.
        let c =
            comando("CREATE TRIGGER t BEFORE INSERT ON c FOR EACH ROW SET NEW.nome = 'O''Brien'")
                .unwrap()
                .unwrap();
        let Comando::CriarGatilho(g) = c else {
            panic!()
        };
        assert_eq!(g.corpo, "SET NEW.nome = 'O''Brien'");
    }

    #[test]
    fn select_into_guarda_o_texto_sem_o_into() {
        let corpo = corpo_de_procedimento(
            "BEGIN SELECT nome, cidade INTO n, c FROM clientes WHERE id = 1; END",
        );
        let Instrucao::SelecionarPara { sql, alvos, .. } = &corpo[0] else {
            panic!("esperava SELECT INTO, veio {:?}", corpo[0])
        };
        assert_eq!(sql, "SELECT nome, cidade FROM clientes WHERE id = 1");
        assert_eq!(alvos, &vec!["n".to_string(), "c".to_string()]);
    }

    /// A variavel no WHERE vira parametro com a faixa certa DENTRO do texto
    /// ja recortado — e so a posicao de valor entra, nunca a coluna.
    #[test]
    fn select_into_acha_a_variavel_do_where() {
        let corpo = corpo_de_procedimento("SELECT nome INTO n FROM clientes WHERE id = qual");
        let Instrucao::SelecionarPara {
            sql, parametros, ..
        } = &corpo[0]
        else {
            panic!()
        };
        assert_eq!(sql, "SELECT nome FROM clientes WHERE id = qual");
        assert_eq!(parametros.len(), 1);
        let (ini, fim, nome) = &parametros[0];
        assert_eq!(nome, "qual");
        let recorte: String = sql.chars().skip(*ini).take(fim - ini).collect();
        assert_eq!(recorte, "qual", "a faixa nao aponta para a variavel");
        // Literal no WHERE nao vira parametro.
        let corpo = corpo_de_procedimento("SELECT nome INTO n FROM c WHERE id = 1");
        let Instrucao::SelecionarPara { parametros, .. } = &corpo[0] else {
            panic!()
        };
        assert!(parametros.is_empty());
    }

    #[test]
    fn endereco_de_tres_partes_no_insert() {
        let corpo = analisar_corpo(
            "INSERT INTO auditoria.matriz.eventos (o) VALUES (1)",
            &regras_de_procedimento(),
        )
        .unwrap();
        let Instrucao::Inserir { alvo, .. } = &corpo[0] else {
            panic!()
        };
        assert_eq!(alvo.database, "auditoria");
        assert_eq!(alvo.tabela, "matriz.eventos");
    }
}

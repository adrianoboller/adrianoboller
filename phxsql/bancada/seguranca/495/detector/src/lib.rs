//! Prototipo do pedido 495 (papel J) -- NAO e codigo de producao.
//! Detector estrutural sobre o LEXICO do motor (analisa, nunca recorta),
//! impressao digital (literal -> ?), e a versao "recortar" para comparar.

use phxsql_sql::lexico::{self, Simbolo, Token};

pub const C_EMPILHADO: u32 = 1 << 0;
pub const C_CONST_OU: u32 = 1 << 1;
pub const C_CONST_E: u32 = 1 << 2;
pub const C_UNIAO_SONDA: u32 = 1 << 3;
pub const C_COMENT_ASPA: u32 = 1 << 4;
pub const C_COMENT_VAZIO: u32 = 1 << 5;
pub const C_FUNCAO_FORA: u32 = 1 << 6;
pub const C_CATALOGO: u32 = 1 << 7;
pub const C_ASPA_ABERTA: u32 = 1 << 8;
/// O que `phxsql_sql::comando_empilhado` (pedido 215) responde HOJE.
pub const C_EMPILHADO_215: u32 = 1 << 9;

pub const FORTES: u32 = C_EMPILHADO | C_CONST_OU | C_UNIAO_SONDA | C_COMENT_ASPA;

pub const NOMES: [(u32, &str); 10] = [
    (C_EMPILHADO, "empilhado"),
    (C_CONST_OU, "constante_sob_or"),
    (C_CONST_E, "constante_sob_and"),
    (C_UNIAO_SONDA, "uniao_de_sondagem"),
    (C_COMENT_ASPA, "comentario_engole_aspa"),
    (C_COMENT_VAZIO, "comentario_vazio_como_espaco"),
    (C_FUNCAO_FORA, "funcao_fora_do_motor"),
    (C_CATALOGO, "catalogo_do_sistema"),
    (C_ASPA_ABERTA, "aspa_aberta"),
    (C_EMPILHADO_215, "comando_empilhado_do_215"),
];

/// Varredura que respeita aspas: so o que esta FORA de literal conta.
/// Devolve as classes de comentario e de aspa aberta.
pub fn varrer_comentarios(texto: &str) -> u32 {
    let b: Vec<char> = texto.chars().collect();
    let (mut i, n, mut r) = (0usize, b.len(), 0u32);
    while i < n {
        let c = b[i];
        if c == '\'' || c == '"' {
            let q = c;
            i += 1;
            loop {
                if i >= n {
                    if q == '\'' {
                        r |= C_ASPA_ABERTA;
                    }
                    return r;
                }
                if b[i] == q {
                    if b.get(i + 1) == Some(&q) {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == '-' && b.get(i + 1) == Some(&'-') {
            let ini = i + 2;
            while i < n && b[i] != '\n' {
                i += 1;
            }
            if b[ini..i].iter().filter(|c| **c == '\'').count() % 2 == 1 {
                r |= C_COMENT_ASPA;
            }
            continue;
        }
        if c == '#' {
            r |= C_COMENT_ASPA;
            i += 1;
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&'*') {
            let ini = i + 2;
            let mut j = ini;
            let mut fechou = false;
            while j + 1 < n {
                if b[j] == '*' && b[j + 1] == '/' {
                    fechou = true;
                    break;
                }
                j += 1;
            }
            if !fechou {
                r |= C_COMENT_ASPA;
                return r;
            }
            let dentro = &b[ini..j];
            if dentro.iter().filter(|c| **c == '\'').count() % 2 == 1 {
                r |= C_COMENT_ASPA;
            }
            let antes = i > 0 && !b[i - 1].is_whitespace();
            let depois = j + 2 < n && !b[j + 2].is_whitespace();
            if dentro.iter().all(|c| c.is_whitespace()) && antes && depois {
                r |= C_COMENT_VAZIO;
            }
            i = j + 2;
            continue;
        }
        i += 1;
    }
    r
}

fn pal(s: &Simbolo) -> Option<String> {
    s.token.palavra_chave()
}

fn e_literal(s: &Simbolo) -> bool {
    match &s.token {
        Token::Numero(_) | Token::Texto(_) => true,
        t => matches!(
            t.palavra_chave().as_deref(),
            Some("TRUE") | Some("FALSE") | Some("NULL")
        ),
    }
}

fn fronteira(s: Option<&Simbolo>) -> bool {
    match s {
        None => true,
        Some(s) => match &s.token {
            Token::FechaParen | Token::PontoEVirgula => true,
            t => matches!(
                t.palavra_chave().as_deref(),
                Some("AND" | "OR" | "ORDER" | "GROUP" | "LIMIT" | "HAVING" | "UNION" | "OFFSET")
            ),
        },
    }
}

/// `lit cmp lit` (ou `lit LIKE lit`) a partir de `i`, seguido de fronteira;
/// com `sozinho`, tambem `lit` isolado seguido de fronteira.
fn predicado_constante(s: &[Simbolo], mut i: usize, sozinho: bool) -> bool {
    while s.get(i).map(|x| x.token == Token::AbreParen).unwrap_or(false) {
        i += 1;
    }
    let Some(a) = s.get(i) else { return false };
    if !e_literal(a) {
        return false;
    }
    if sozinho && fronteira(s.get(i + 1)) {
        return true;
    }
    let Some(op) = s.get(i + 1) else { return false };
    let e_cmp = matches!(op.token, Token::Comparador(_)) || pal(op).as_deref() == Some("LIKE");
    if !e_cmp {
        return false;
    }
    let Some(b) = s.get(i + 2) else { return false };
    e_literal(b) && fronteira(s.get(i + 3))
}

const CONHECIDAS: [&str; 20] = [
    "UPPER", "UCASE", "LOWER", "LCASE", "TRIM", "LENGTH", "CHAR_LENGTH", "ROUND", "ABS",
    "COALESCE", "IFNULL", "CONCAT", "COUNT", "SUM", "AVG", "MIN", "MAX", "ROW_NUMBER", "IN",
    "EXISTS",
];
const NAO_CHAMADA_ANTES: [&str; 18] = [
    "INTO", "TABLE", "PROCEDURE", "CALL", "ON", "INDEX", "TRIGGER", "FUNCTION", "VIEW", "FROM",
    "JOIN", "UPDATE", "REFERENCES", "KEY", "AS", "EXISTS", "IN", "VALUES",
];
const CATALOGOS: [&str; 7] = [
    "information_schema", "pg_catalog", "mysql", "performance_schema", "sys", "phxsys",
    "sqlite_temp_master",
];
const TABELAS_DE_SISTEMA: [&str; 5] = ["sqlite_master", "sqlite_schema", "pg_shadow", "pg_authid", "pg_user"];

/// As classes que dependem de simbolo. `s` sai do MESMO lexico da consulta.
pub fn classes_dos_simbolos(s: &[Simbolo]) -> u32 {
    let mut r = 0u32;
    let verbo = s.first().and_then(pal).unwrap_or_default();
    let dml = matches!(verbo.as_str(), "SELECT" | "INSERT" | "UPDATE" | "DELETE" | "WITH");
    let mut entre_pendente = false;
    for i in 0..s.len() {
        let p = pal(&s[i]);
        match p.as_deref() {
            Some("BETWEEN") => entre_pendente = true,
            Some("OR") => {
                if predicado_constante(s, i + 1, true) {
                    r |= C_CONST_OU;
                }
            }
            Some("AND") => {
                if entre_pendente {
                    entre_pendente = false;
                } else if predicado_constante(s, i + 1, false) {
                    r |= C_CONST_E;
                }
            }
            Some("UNION") => {
                let mut j = i + 1;
                if matches!(s.get(j).and_then(pal).as_deref(), Some("ALL" | "DISTINCT")) {
                    j += 1;
                }
                if s.get(j).and_then(pal).as_deref() == Some("SELECT") {
                    j += 1;
                    let (mut so_lit, mut itens, mut tem_from, mut prof) = (true, 0usize, false, 0i32);
                    while let Some(x) = s.get(j) {
                        match &x.token {
                            Token::AbreParen => prof += 1,
                            Token::FechaParen => {
                                if prof == 0 {
                                    break;
                                }
                                prof -= 1
                            }
                            Token::PontoEVirgula if prof == 0 => break,
                            Token::Virgula if prof == 0 => {}
                            _ => {
                                let k = pal(x);
                                if prof == 0 && k.as_deref() == Some("FROM") {
                                    tem_from = true;
                                    break;
                                }
                                if prof == 0
                                    && matches!(k.as_deref(), Some("UNION" | "ORDER" | "LIMIT" | "WHERE"))
                                {
                                    break;
                                }
                                if prof == 0 {
                                    itens += 1;
                                    if !e_literal(x) {
                                        so_lit = false;
                                    }
                                }
                            }
                        }
                        j += 1;
                    }
                    if (itens > 0 && so_lit) || !tem_from {
                        r |= C_UNIAO_SONDA;
                    }
                }
            }
            _ => {}
        }
        if let Token::Palavra { texto, citado: false } = &s[i].token {
            let baixo = texto.to_ascii_lowercase();
            let esquema = baixo.split('.').next().unwrap_or("");
            let seguinte_e_ponto = s.get(i + 1).map(|x| x.token == Token::Ponto).unwrap_or(false);
            if (baixo.contains('.') || seguinte_e_ponto) && CATALOGOS.contains(&esquema) {
                r |= C_CATALOGO;
            }
            let ultimo = baixo.rsplit('.').next().unwrap_or("");
            if TABELAS_DE_SISTEMA.contains(&ultimo) {
                r |= C_CATALOGO;
            }
            if dml && s.get(i + 1).map(|x| x.token == Token::AbreParen).unwrap_or(false) {
                let up = texto.to_ascii_uppercase();
                let mut k = i;
                while k >= 2 && s[k - 1].token == Token::Ponto {
                    k -= 2;
                }
                let antes = if k > 0 { pal(&s[k - 1]) } else { None };
                let contexto_de_nome = antes
                    .as_deref()
                    .map(|a| NAO_CHAMADA_ANTES.contains(&a))
                    .unwrap_or(false);
                let palavra_da_gramatica = matches!(
                    up.as_str(),
                    "VALUES" | "OVER" | "AND" | "OR" | "NOT" | "WHERE" | "ON" | "SELECT" | "AS" | "FROM" | "USING" | "JOIN"
                );
                if !CONHECIDAS.contains(&up.as_str()) && !contexto_de_nome && !palavra_da_gramatica {
                    r |= C_FUNCAO_FORA;
                }
            }
        }
    }
    r
}

/// O detector inteiro, como seria chamado com o texto so (relexa).
pub fn detectar(texto: &str) -> u32 {
    let mut r = varrer_comentarios(texto);
    if let Ok(s) = lexico::analisar(texto) {
        r |= classes_dos_simbolos(&s);
    }
    if phxsql_sql::comando_empilhado(texto) {
        r |= C_EMPILHADO_215;
    }
    if empilhado_de_verdade(texto) {
        r |= C_EMPILHADO;
    }
    r
}

/// O que a doc do `comando_empilhado` promete: simbolo DEPOIS de um `;`.
pub fn empilhado_de_verdade(texto: &str) -> bool {
    let Ok(s) = lexico::analisar(texto) else { return false };
    match s.iter().position(|x| x.token == Token::PontoEVirgula) {
        None => false,
        Some(k) => {
            // corpo de rotina: o texto nao e um comando desta gramatica
            if matches!(s.first().and_then(pal).as_deref(), Some("CREATE" | "ALTER" | "BEGIN" | "DECLARE")) {
                return false;
            }
            s[k..].iter().any(|x| x.token != Token::PontoEVirgula)
        }
    }
}

/// O detector como seria no motor: os simbolos JA vieram da analise da
/// consulta (nao relexa), e o empilhado se decide pelo que sobrou dela.
pub fn detectar_com_simbolos(texto: &str, s: &[Simbolo], sobrou_depois_do_ponto_e_virgula: bool) -> u32 {
    let mut r = varrer_comentarios(texto) | classes_dos_simbolos(s);
    if sobrou_depois_do_ponto_e_virgula {
        r |= C_EMPILHADO;
    }
    r
}

pub fn nomes(r: u32) -> Vec<&'static str> {
    NOMES.iter().filter(|(b, _)| r & b != 0).map(|(_, n)| *n).collect()
}

/// Impressao digital: literal -> ?, lista de ? colapsada, palavra sem aspas
/// em maiusculas. E TAMBEM o texto redigido que poderia ir a uma IA.
pub fn normalizar(s: &[Simbolo]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(s.len());
    for x in s {
        let t = match &x.token {
            Token::Numero(_) | Token::Texto(_) | Token::Parametro(_) => "?".to_string(),
            Token::Palavra { texto, citado: true } => format!("\"{texto}\""),
            Token::Palavra { texto, .. } => {
                let up = texto.to_ascii_uppercase();
                if matches!(up.as_str(), "TRUE" | "FALSE" | "NULL") {
                    "?".into()
                } else {
                    up
                }
            }
            outro => outro.descrever(),
        };
        // colapsa "?, ?, ?" em "..." dentro de uma lista
        if t == "?" {
            let n = out.len();
            if n >= 2 && out[n - 1] == "," && (out[n - 2] == "?" || out[n - 2] == "...") {
                out.pop();
                out.pop();
                out.push("...".into());
                continue;
            }
        }
        out.push(t);
    }
    // colapsa tuplas repetidas de VALUES: "( ... ) , ( ... )" -> uma
    let mut junta = out.join(" ");
    loop {
        let antes = junta.len();
        junta = junta.replace("( ... ) , ( ... )", "( ... )").replace("( ? ) , ( ? )", "( ? )");
        if junta.len() == antes {
            break;
        }
    }
    junta
}

pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// O que "recortar" faria: as mesmas perguntas no TEXTO cru, sem saber o que
/// e literal. So existe para medir o custo de NAO analisar (H6).
pub fn recortar(texto: &str) -> u32 {
    let t = texto.to_ascii_uppercase();
    let mut r = 0;
    let tem = |p: &str| t.contains(p);
    if let Some(k) = t.find(';') {
        if !t[k + 1..].trim().trim_start_matches(';').trim().is_empty() {
            r |= C_EMPILHADO;
        }
    }
    let w: Vec<&str> = t
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|x| !x.is_empty())
        .collect();
    for k in 0..w.len() {
        let lit = |x: &str| {
            x.chars().all(|c| c.is_ascii_digit())
                || (x.starts_with('\'') && x.len() > 1)
                || matches!(x, "TRUE" | "FALSE" | "NULL")
        };
        if w[k] == "OR" {
            if let Some(a) = w.get(k + 1) {
                if lit(a) || a.contains('=') {
                    r |= C_CONST_OU;
                }
            }
        }
        if w[k] == "AND" {
            if let (Some(a), Some(b)) = (w.get(k + 1), w.get(k + 2)) {
                if lit(a) && (b.starts_with('=') || *b == "LIKE") {
                    r |= C_CONST_E;
                }
            }
        }
        if w[k] == "UNION" {
            r |= C_UNIAO_SONDA;
        }
    }
    if tem("--") || tem("#") || tem("/*") {
        r |= C_COMENT_ASPA;
    }
    if tem("INFORMATION_SCHEMA") || tem("PHXSYS.") || tem("SQLITE_MASTER") || tem("PG_CATALOG") {
        r |= C_CATALOGO;
    }
    r
}

//! Escape na escrita e um leitor de eventos pequeno para a leitura.
//!
//! O leitor nao e um parser XML completo (sem DTD, sem validacao de
//! namespace): basta para tirar texto de `w:t` e valores de `c`, e recusar
//! entidade externa por construcao evita a classe inteira de XXE.

/// XML 1.0, producao `Char`: fora disso o documento inteiro fica mal formado
/// e o Word recusa abrir. Nao ha escape possivel para esses caracteres
/// (nem `&#1;` e valido em 1.0), entao a unica saida honesta e remove-los.
pub fn is_xml_char(c: char) -> bool {
    matches!(c,
        '\u{9}' | '\u{A}' | '\u{D}'
        | '\u{20}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}')
}

/// Escapa para texto e para valor de atributo com o mesmo codigo: um so
/// caminho de escape e o que impede o atributo de esquecer as aspas.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if is_xml_char(c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// Remove so os caracteres proibidos, sem escapar. Usado para validar nomes
/// (aba de planilha) antes de decidir se sao aceitos.
pub fn strip_forbidden(s: &str) -> String {
    s.chars().filter(|&c| is_xml_char(c)).collect()
}

pub const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        empty: bool,
    },
    End(String),
    Text(String),
}

/// Nome sem prefixo: produtores diferentes usam prefixos diferentes para o
/// mesmo namespace, e aqui so importa o nome local.
pub fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name || local(k) == name)
        .map(|(_, v)| v.as_str())
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let ent = &rest[1..end];
        let rep = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match rep {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                // Entidade desconhecida fica literal: nao se resolve DTD aqui.
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_attrs(body: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ks = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = &body[ks..i];
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'=') {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'"' && b[i] != b'\'') {
            break;
        }
        let q = b[i];
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        if !key.is_empty() {
            attrs.push((key.to_string(), unescape(&body[vs..i.min(b.len())])));
        }
        i += 1;
    }
    attrs
}

/// Transforma o documento numa lista de eventos. Mal formado grave (tag sem
/// fechar) vira erro; o resto e tolerado, porque o objetivo e ler anexo.
pub fn events(src: &str) -> Result<Vec<Event>, String> {
    let mut ev = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            ev.push(Event::Text(unescape(rest)));
            break;
        };
        if lt > 0 {
            ev.push(Event::Text(unescape(&rest[..lt])));
        }
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>").ok_or("CDATA sem fechar")?;
            ev.push(Event::Text(r[..end].to_string()));
            rest = &r[end + 3..];
        } else if rest.starts_with("<!--") {
            let end = rest.find("-->").ok_or("comentario sem fechar")?;
            rest = &rest[end + 3..];
        } else if rest.starts_with("<?") {
            let end = rest.find("?>").ok_or("instrucao sem fechar")?;
            rest = &rest[end + 2..];
        } else if rest.starts_with("<!") {
            // DOCTYPE: ignorado de proposito (sem entidades externas).
            let end = rest.find('>').ok_or("declaracao sem fechar")?;
            rest = &rest[end + 1..];
        } else {
            // Procura o '>' fora de aspas: valor de atributo pode conter '>'.
            let b = rest.as_bytes();
            let mut i = 1;
            let mut q = 0u8;
            while i < b.len() {
                match b[i] {
                    b'"' | b'\'' if q == 0 => q = b[i],
                    c if c == q => q = 0,
                    b'>' if q == 0 => break,
                    _ => {}
                }
                i += 1;
            }
            if i >= b.len() {
                return Err("tag sem fechar".into());
            }
            let inner = &rest[1..i];
            rest = &rest[i + 1..];
            if let Some(name) = inner.strip_prefix('/') {
                ev.push(Event::End(name.trim().to_string()));
            } else {
                let (inner, empty) = match inner.strip_suffix('/') {
                    Some(x) => (x, true),
                    None => (inner, false),
                };
                let nend = inner
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(inner.len());
                ev.push(Event::Start {
                    name: inner[..nend].to_string(),
                    attrs: parse_attrs(&inner[nend..]),
                    empty,
                });
            }
        }
    }
    Ok(ev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_os_cinco_e_remove_controle() {
        assert_eq!(
            esc("a&b<c>d\"e'f\u{1}g\u{FFFE}h\tç"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;fgh\tç"
        );
    }

    #[test]
    fn eventos_e_entidades() {
        let e = events(
            "<?xml version=\"1.0\"?><a x=\"1&amp;2\" y='>'><b/>t&#233;&#xE7;<![CDATA[<z>]]></a>",
        )
        .unwrap();
        assert_eq!(
            e,
            vec![
                Event::Start {
                    name: "a".into(),
                    attrs: vec![("x".into(), "1&2".into()), ("y".into(), ">".into())],
                    empty: false
                },
                Event::Start {
                    name: "b".into(),
                    attrs: vec![],
                    empty: true
                },
                Event::Text("té\u{e7}".into()),
                Event::Text("<z>".into()),
                Event::End("a".into()),
            ]
        );
    }

    #[test]
    fn entidade_desconhecida_fica_literal() {
        assert_eq!(unescape("a &foo; b"), "a &foo; b");
    }
}

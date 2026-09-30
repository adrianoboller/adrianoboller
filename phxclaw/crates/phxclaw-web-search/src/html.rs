//! Leitor de HTML proprio, pequeno de proposito.
//!
//! Nao e um parser HTML5 conforme a norma: e um tokenizador tolerante que serve a duas
//! perguntas so — "quais sao os resultados desta pagina de busca" e "qual e o texto
//! legivel desta pagina". Uma crate de arvore DOM (scraper/html5ever) traria dezenas de
//! dependencias transitivas para responder o mesmo, e as duas perguntas nao precisam de
//! arvore: precisam de etiquetas em ordem, atributos e texto.
//!
//! Os indices de corte caem sempre em `<` e `>`, que sao ASCII; por isso fatiar a `&str`
//! nesses pontos nunca parte um caractere UTF-8 ao meio.

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token<'a> {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    End {
        name: String,
    },
    /// Texto cru, ainda com entidades; quem consome decide se decodifica.
    Text(&'a str),
}

impl Token<'_> {
    pub(crate) fn attr(&self, chave: &str) -> Option<&str> {
        match self {
            Token::Start { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k == chave)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }

    pub(crate) fn has_class(&self, classe: &str) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|x| x == classe))
    }
}

/// Elementos cujo conteudo nao e HTML: dentro deles `<` nao abre etiqueta. Sem isto, um
/// `if (a<b)` num script vira etiqueta e o resto da pagina se desalinha.
const TEXTO_CRU: &[&str] = &["script", "style", "textarea", "title"];

pub(crate) fn tokenize<'a>(html: &'a str) -> Vec<Token<'a>> {
    let b = html.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut texto_desde = 0;
    let flush = |out: &mut Vec<Token<'a>>, de: usize, ate: usize| {
        if ate > de {
            out.push(Token::Text(&html[de..ate]));
        }
    };
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let resto = &b[i..];
        if resto.starts_with(b"<!--") {
            flush(&mut out, texto_desde, i);
            i = find(b, i + 4, b"-->").map_or(b.len(), |p| p + 3);
            texto_desde = i;
            continue;
        }
        let prox = resto.get(1).copied().unwrap_or(0);
        if prox == b'!' || prox == b'?' {
            flush(&mut out, texto_desde, i);
            i = find(b, i + 2, b">").map_or(b.len(), |p| p + 1);
            texto_desde = i;
            continue;
        }
        let fecha = prox == b'/';
        let ini_nome = if fecha { i + 2 } else { i + 1 };
        if !b.get(ini_nome).is_some_and(|c| c.is_ascii_alphabetic()) {
            // `<` solto no texto ("a < b"): continua sendo texto.
            i += 1;
            continue;
        }
        flush(&mut out, texto_desde, i);
        let (tok, fim) = parse_tag(html, ini_nome, fecha);
        i = fim;
        texto_desde = i;
        let Some(tok) = tok else { continue };
        if let Token::Start {
            name,
            self_closing: false,
            ..
        } = &tok
            && TEXTO_CRU.contains(&name.as_str())
        {
            let nome = name.clone();
            out.push(tok);
            let fim_cru = find_ci(b, i, format!("</{nome}").as_bytes()).unwrap_or(b.len());
            flush(&mut out, i, fim_cru);
            i = find(b, fim_cru, b">").map_or(b.len(), |p| p + 1);
            texto_desde = i;
            out.push(Token::End { name: nome });
            continue;
        }
        out.push(tok);
    }
    flush(&mut out, texto_desde, b.len());
    out
}

/// Le uma etiqueta a partir do inicio do nome; devolve o token e o indice apos o `>`.
fn parse_tag(html: &str, mut i: usize, fecha: bool) -> (Option<Token<'_>>, usize) {
    let b = html.as_bytes();
    let ini = i;
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    let name = html[ini..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            // Etiqueta sem `>` ate o fim: descarta, nao inventa fechamento.
            return (None, b.len());
        }
        match b[i] {
            b'>' => {
                i += 1;
                break;
            }
            b'/' => {
                self_closing = b.get(i + 1) == Some(&b'>');
                i += 1;
                continue;
            }
            _ => {}
        }
        let ini_k = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let chave = html[ini_k..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut valor = String::new();
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            match b.get(i) {
                Some(&q @ (b'"' | b'\'')) => {
                    let fim = find(b, i + 1, &[q]).unwrap_or(b.len());
                    valor = decode_entities(&html[i + 1..fim]);
                    i = (fim + 1).min(b.len());
                }
                _ => {
                    let ini_v = i;
                    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                        i += 1;
                    }
                    valor = decode_entities(&html[ini_v..i]);
                }
            }
        }
        if !chave.is_empty() {
            attrs.push((chave, valor));
        }
    }
    let tok = if fecha {
        Token::End { name }
    } else {
        Token::Start {
            name,
            attrs,
            self_closing,
        }
    };
    (Some(tok), i)
}

fn find(b: &[u8], de: usize, agulha: &[u8]) -> Option<usize> {
    if de > b.len() {
        return None;
    }
    b[de..]
        .windows(agulha.len())
        .position(|w| w == agulha)
        .map(|p| p + de)
}

fn find_ci(b: &[u8], de: usize, agulha: &[u8]) -> Option<usize> {
    if de > b.len() {
        return None;
    }
    b[de..]
        .windows(agulha.len())
        .position(|w| w.eq_ignore_ascii_case(agulha))
        .map(|p| p + de)
}

/// Decodifica entidades numericas e as nomeadas que aparecem de fato em paginas de texto.
/// A tabela inteira do HTML5 tem mais de duas mil; entidade desconhecida fica como veio,
/// que e o que o navegador faz com `&foo;` que nao conhece.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(p) = resto.find('&') {
        out.push_str(&resto[..p]);
        let cauda = &resto[p..];
        let fim = cauda.bytes().take(12).position(|c| c == b';');
        let decodificado = fim.and_then(|f| entidade(&cauda[1..f]).map(|c| (c, f + 1)));
        match decodificado {
            Some((c, n)) => {
                out.push(c);
                resto = &cauda[n..];
            }
            None => {
                out.push('&');
                resto = &cauda[1..];
            }
        }
    }
    out.push_str(resto);
    out
}

fn entidade(nome: &str) -> Option<char> {
    if let Some(num) = nome.strip_prefix('#') {
        let cp = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse::<u32>().ok()?,
        };
        // Codigo nulo ou fora do Unicode vira o caractere de substituicao, como no
        // navegador, em vez de sumir calado com o texto em volta.
        return Some(
            char::from_u32(cp)
                .filter(|&c| c != '\0')
                .unwrap_or('\u{FFFD}'),
        );
    }
    Some(match nome {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{A0}',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "hellip" => '\u{2026}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        "laquo" => '\u{AB}',
        "raquo" => '\u{BB}',
        "copy" => '\u{A9}',
        "reg" => '\u{AE}',
        "trade" => '\u{2122}',
        "middot" => '\u{B7}',
        "bull" => '\u{2022}',
        "deg" => '\u{B0}',
        "euro" => '\u{20AC}',
        "times" => '\u{D7}',
        "ccedil" => 'ç',
        "atilde" => 'ã',
        "otilde" => 'õ',
        "aacute" => 'á',
        "eacute" => 'é',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "acirc" => 'â',
        "ecirc" => 'ê',
        "ocirc" => 'ô',
        "agrave" => 'à',
        _ => return None,
    })
}

/// Texto de um fragmento HTML numa linha so: etiquetas fora, entidades decodificadas,
/// espacos colapsados. Serve ao trecho de resultado (o Brave marca termos com `<strong>`).
pub fn inline_text(html: &str) -> String {
    let mut s = String::new();
    for t in tokenize(html) {
        if let Token::Text(t) = t {
            s.push_str(&decode_entities(t));
        }
    }
    collapse_ws(&s)
}

pub(crate) fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Elementos que nao sao conteudo: codigo, estilo e a moldura do site. Texto de menu e
/// de rodape repetido em toda pagina so gasta contexto do modelo sem dizer nada novo.
const PULAR: &[&str] = &[
    "script", "style", "nav", "footer", "noscript", "template", "svg", "iframe", "form", "button",
    "select",
];

const BLOCO: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "header",
    "aside",
    "ul",
    "ol",
    "table",
    "tr",
    "blockquote",
    "figure",
    "figcaption",
    "dl",
    "dt",
    "dd",
    "hr",
    "details",
    "summary",
    "address",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageLink {
    pub text: String,
    pub url: String,
}

#[derive(Debug, Default)]
pub struct Readable {
    pub title: String,
    pub text: String,
    pub links: Vec<PageLink>,
}

struct Saida {
    s: String,
    espaco_pendente: bool,
}

impl Saida {
    fn texto(&mut self, t: &str) {
        for c in t.chars() {
            if c.is_whitespace() {
                self.espaco_pendente = true;
                continue;
            }
            if self.espaco_pendente && !self.s.is_empty() && !self.s.ends_with(['\n', ' ']) {
                self.s.push(' ');
            }
            self.espaco_pendente = false;
            self.s.push(c);
        }
    }

    fn cru(&mut self, t: &str) {
        self.espaco_pendente = false;
        self.s.push_str(t);
    }

    /// Garante `n` quebras no fim, sem acumular mais que isso: dez `<div>` vazios
    /// seguidos nao podem virar dez linhas em branco.
    fn quebra(&mut self, n: usize) {
        self.espaco_pendente = false;
        while self.s.ends_with(' ') {
            self.s.pop();
        }
        if self.s.is_empty() {
            return;
        }
        let ja = self.s.chars().rev().take_while(|&c| c == '\n').count();
        for _ in ja..n {
            self.s.push('\n');
        }
    }
}

/// Converte HTML em texto legivel: titulos marcados com `#`, paragrafos separados por
/// linha em branco, itens de lista com `- `, `<pre>` preservado, e os links absolutos.
pub fn readable(html: &str, base: Option<&Url>) -> Readable {
    let mut r = Readable::default();
    let mut out = Saida {
        s: String::new(),
        espaco_pendente: false,
    };
    let mut pular = 0usize;
    let mut pre = 0usize;
    let mut no_titulo = false;
    let mut link: Option<(String, usize)> = None;
    let mut vistos = std::collections::BTreeSet::new();
    for t in tokenize(html) {
        match t {
            Token::Start {
                ref name,
                self_closing,
                ..
            } => {
                let n = name.as_str();
                if PULAR.contains(&n) {
                    if !self_closing {
                        pular += 1;
                    }
                    continue;
                }
                if pular > 0 {
                    continue;
                }
                match n {
                    "title" => no_titulo = true,
                    "br" => out.quebra(1),
                    "li" => {
                        out.quebra(1);
                        out.cru("- ");
                    }
                    "pre" => {
                        out.quebra(2);
                        pre += 1;
                    }
                    "td" | "th" => out.texto(" "),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        out.quebra(2);
                        let nivel = usize::from(n.as_bytes()[1] - b'0');
                        out.cru(&"#".repeat(nivel));
                        out.cru(" ");
                    }
                    "a" => {
                        link = t.attr("href").map(|h| (h.to_string(), out.s.len()));
                    }
                    _ if BLOCO.contains(&n) => out.quebra(if n == "p" { 2 } else { 1 }),
                    _ => {}
                }
            }
            Token::End { ref name } => {
                let n = name.as_str();
                if PULAR.contains(&n) {
                    pular = pular.saturating_sub(1);
                    continue;
                }
                if pular > 0 {
                    continue;
                }
                match n {
                    "title" => no_titulo = false,
                    "pre" => {
                        pre = pre.saturating_sub(1);
                        out.quebra(2);
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" => out.quebra(2),
                    "a" => {
                        if let Some((href, ini)) = link.take()
                            && let Some(abs) = absolute_link(&href, base)
                            && vistos.insert(abs.clone())
                        {
                            let texto = collapse_ws(out.s.get(ini..).unwrap_or(""));
                            r.links.push(PageLink {
                                text: texto,
                                url: abs,
                            });
                        }
                    }
                    _ if BLOCO.contains(&n) => out.quebra(1),
                    _ => {}
                }
            }
            Token::Text(t) => {
                if pular > 0 {
                    continue;
                }
                let d = decode_entities(t);
                if no_titulo {
                    r.title.push_str(&d);
                } else if pre > 0 {
                    out.cru(&d);
                } else {
                    out.texto(&d);
                }
            }
        }
    }
    r.title = collapse_ws(&r.title);
    r.text = out.s.trim().to_string();
    r
}

/// So links que o agente consegue seguir: http(s), absolutos, sem o fragmento (que aponta
/// para a mesma pagina e so duplicaria a lista).
fn absolute_link(href: &str, base: Option<&Url>) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let mut u = match base {
        Some(b) => b.join(href).ok()?,
        None => Url::parse(href).ok()?,
    };
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    u.set_fragment(None);
    Some(u.to_string())
}

/// Texto puro recebido como `text/plain`: so normaliza fins de linha e linhas em branco.
pub fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut brancas = 0;
    for linha in text.lines() {
        let l = linha.trim_end();
        if l.trim().is_empty() {
            brancas += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if brancas > 0 { "\n\n" } else { "\n" });
        }
        brancas = 0;
        out.push_str(l);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entidades_nomeadas_e_numericas() {
        assert_eq!(
            decode_entities("a &amp; b &lt;c&gt; &quot;d&quot; &#39;e&#x27; &eacute; &#0; &nada;"),
            "a & b <c> \"d\" 'e' é \u{FFFD} &nada;"
        );
        assert_eq!(decode_entities("sem ; &amp"), "sem ; &amp");
    }

    #[test]
    fn script_com_menor_que_nao_abre_etiqueta() {
        let r = readable(
            "<p>antes</p><script>if (a<b) { x = '</p><p>falso'; }</script><p>depois</p>",
            None,
        );
        assert_eq!(r.text, "antes\n\ndepois");
    }

    #[test]
    fn texto_legivel_da_fixture() {
        let base = Url::parse("https://exemplo.com.br/artigos/rust.html").unwrap();
        let r = readable(include_str!("../fixtures/pagina.html"), Some(&base));
        assert_eq!(r.title, "Rust & o PhxClaw — guia");
        assert!(r.text.starts_with("# Por que Rust?"), "{}", r.text);
        assert!(
            r.text
                .contains("Seguranca de memoria sem coletor & sem pausas.")
        );
        assert!(r.text.contains("## Instalacao"));
        assert!(r.text.contains("- rustup\n- cargo"));
        assert!(r.text.contains("fn main() {\n    println!(\"<ola>\");\n}"));
        for lixo in ["rastreador", "menu-topo", "Todos os direitos", "color:"] {
            assert!(!r.text.contains(lixo), "vazou {lixo:?}: {}", r.text);
        }
        assert!(!r.text.contains("\n\n\n"));
        let urls: Vec<_> = r.links.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://www.rust-lang.org/learn",
                "https://exemplo.com.br/instalar?so=linux&arq=x86",
            ]
        );
        assert_eq!(r.links[0].text, "documentacao oficial");
    }

    #[test]
    fn inline_tira_marcacao() {
        assert_eq!(
            inline_text("A <strong>linguagem</strong>  &amp;\n rapida"),
            "A linguagem & rapida"
        );
    }
}

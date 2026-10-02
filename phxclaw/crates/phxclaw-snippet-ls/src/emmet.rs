//! Expansao Emmet basica: `div>ul>li*3`, `.cls`, `#id`, `+`, `^`, `{texto}`, `p*2>a` para
//! HTML, e a tabela curta de propriedades para CSS (`m10`, `df`, `p10-20`).
//!
//! E o subconjunto que cabe numa abreviacao digitada: grupos entre parenteses e atributos
//! `[a=b]` ficam de fora de proposito -- o que nao se analisa nao se expande, e devolver
//! `None` e melhor que devolver HTML errado.

/// Um elemento da abreviacao, ja com os filhos.
#[derive(Debug, Default, Clone)]
struct No {
    nome: String,
    classes: Vec<String>,
    id: Option<String>,
    texto: Option<String>,
    vezes: usize,
    filhos: Vec<No>,
}

/// Elementos sem fechamento no HTML.
const VAZIOS: &[&str] = &["br", "hr", "img", "input", "meta", "link", "source", "wbr"];
/// Elementos que ficam na mesma linha do pai quando sao folha.
const INLINE: &[&str] = &[
    "a", "span", "b", "i", "em", "strong", "small", "code", "label",
];

/// `None` quando a abreviacao nao e Emmet (texto comum, caractere fora da gramatica).
pub fn expandir_html(abrev: &str) -> Option<String> {
    let abrev = abrev.trim();
    if abrev.is_empty() {
        return None;
    }
    let raiz = analisar(abrev)?;
    let mut saida = String::new();
    for n in &raiz {
        desenhar(n, 0, &mut saida);
    }
    Some(saida.trim_end().to_string())
}

fn analisar(s: &str) -> Option<Vec<No>> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut raiz: Vec<No> = Vec::new();
    // Caminho de indices ate o pai de agora: `>` desce, `^` sobe.
    let mut pais: Vec<usize> = Vec::new();
    // `+` so faz sentido depois de um elemento; a gramatica comeca por elemento.
    let mut espera_elemento = true;
    while i < c.len() {
        if espera_elemento {
            let (no, fim) = elemento(&c, i)?;
            i = fim;
            inserir(&mut raiz, &pais, no);
            espera_elemento = false;
            continue;
        }
        match c[i] {
            '>' => {
                // O pai novo e o ultimo irmao inserido no nivel de agora.
                let irmaos = nivel(&raiz, &pais);
                pais.push(irmaos.len().checked_sub(1)?);
            }
            '+' => {}
            '^' => {
                pais.pop();
            }
            _ => return None,
        }
        i += 1;
        espera_elemento = true;
    }
    (!espera_elemento).then_some(raiz)
}

fn nivel<'a>(raiz: &'a [No], pais: &[usize]) -> &'a [No] {
    let mut atual = raiz;
    for p in pais {
        atual = &atual[*p].filhos;
    }
    atual
}

fn inserir(raiz: &mut Vec<No>, pais: &[usize], no: No) {
    let mut atual = raiz;
    for p in pais {
        atual = &mut atual[*p].filhos;
    }
    atual.push(no);
}

fn e_nome(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

/// Um elemento: `nome.cls#id*N{texto}`, em qualquer ordem depois do nome. `.cls` e `#id`
/// sem nome viram `div`.
fn elemento(c: &[char], mut i: usize) -> Option<(No, usize)> {
    let mut no = No {
        vezes: 1,
        ..No::default()
    };
    let inicio = i;
    while i < c.len() && e_nome(c[i]) {
        no.nome.push(c[i]);
        i += 1;
    }
    loop {
        match c.get(i) {
            Some('.') => {
                i += 1;
                let (cls, fim) = palavra(c, i);
                if cls.is_empty() {
                    return None;
                }
                no.classes.push(cls);
                i = fim;
            }
            Some('#') => {
                i += 1;
                let (id, fim) = palavra(c, i);
                if id.is_empty() {
                    return None;
                }
                no.id = Some(id);
                i = fim;
            }
            Some('*') => {
                i += 1;
                let (n, fim) = palavra(c, i);
                no.vezes = n.parse().ok().filter(|v| (1..=100).contains(v))?;
                i = fim;
            }
            Some('{') => {
                let fim = c[i..].iter().position(|ch| *ch == '}')? + i;
                no.texto = Some(c[i + 1..fim].iter().collect());
                i = fim + 1;
            }
            _ => break,
        }
    }
    if i == inicio {
        return None;
    }
    if no.nome.is_empty() {
        if no.classes.is_empty() && no.id.is_none() && no.texto.is_none() {
            return None;
        }
        no.nome = "div".into();
    }
    Some((no, i))
}

/// Nome de classe ou id: alem do nome, o `$` da numeracao.
fn palavra(c: &[char], mut i: usize) -> (String, usize) {
    let mut s = String::new();
    while i < c.len() && (e_nome(c[i]) || c[i] == '$') {
        s.push(c[i]);
        i += 1;
    }
    (s, i)
}

/// `$` vira o numero da repeticao (1-based), como no Emmet.
fn numerar(s: &str, n: usize) -> String {
    s.replace('$', &n.to_string())
}

fn desenhar(no: &No, nivel: usize, saida: &mut String) {
    for n in 1..=no.vezes {
        let recuo = "    ".repeat(nivel);
        let mut abre = format!("<{}", no.nome);
        if let Some(id) = &no.id {
            abre.push_str(&format!(" id=\"{}\"", numerar(id, n)));
        }
        if !no.classes.is_empty() {
            let cls: Vec<String> = no.classes.iter().map(|c| numerar(c, n)).collect();
            abre.push_str(&format!(" class=\"{}\"", cls.join(" ")));
        }
        abre.push('>');
        if VAZIOS.contains(&no.nome.as_str()) {
            saida.push_str(&format!("{recuo}{abre}\n"));
            continue;
        }
        let texto = no
            .texto
            .as_deref()
            .map(|t| numerar(t, n))
            .unwrap_or_default();
        let folha = no.filhos.is_empty();
        let so_inline = no
            .filhos
            .iter()
            .all(|f| INLINE.contains(&f.nome.as_str()) && f.filhos.is_empty());
        if folha || so_inline {
            let mut dentro = texto;
            for f in &no.filhos {
                let mut s = String::new();
                desenhar(f, 0, &mut s);
                dentro.push_str(s.trim_end());
            }
            saida.push_str(&format!("{recuo}{abre}{dentro}</{}>\n", no.nome));
        } else {
            saida.push_str(&format!("{recuo}{abre}\n"));
            if !texto.is_empty() {
                saida.push_str(&format!("{recuo}    {texto}\n"));
            }
            for f in &no.filhos {
                desenhar(f, nivel + 1, saida);
            }
            saida.push_str(&format!("{recuo}</{}>\n", no.nome));
        }
    }
}

/// Propriedade CSS por abreviacao. (abreviacao, propriedade, valor fixo ou vazio.)
const CSS: &[(&str, &str, &str)] = &[
    ("m", "margin", ""),
    ("p", "padding", ""),
    ("w", "width", ""),
    ("h", "height", ""),
    ("t", "top", ""),
    ("r", "right", ""),
    ("b", "bottom", ""),
    ("l", "left", ""),
    ("z", "z-index", ""),
    ("c", "color", ""),
    ("bg", "background", ""),
    ("bgc", "background-color", ""),
    ("fz", "font-size", ""),
    ("fw", "font-weight", ""),
    ("ff", "font-family", ""),
    ("lh", "line-height", ""),
    ("bd", "border", ""),
    ("bdrs", "border-radius", ""),
    ("op", "opacity", ""),
    ("ta", "text-align", ""),
    ("tac", "text-align", "center"),
    ("tar", "text-align", "right"),
    ("d", "display", ""),
    ("db", "display", "block"),
    ("dn", "display", "none"),
    ("df", "display", "flex"),
    ("dib", "display", "inline-block"),
    ("dg", "display", "grid"),
    ("pos", "position", ""),
    ("poa", "position", "absolute"),
    ("por", "position", "relative"),
    ("pof", "position", "fixed"),
    ("ov", "overflow", ""),
    ("ovh", "overflow", "hidden"),
    ("cur", "cursor", ""),
    ("curp", "cursor", "pointer"),
    ("fxd", "flex-direction", ""),
    ("jc", "justify-content", ""),
    ("ai", "align-items", ""),
];

/// `m10` → `margin: 10px;`, `p10-20` → `padding: 10px 20px;`, `w50p` → `width: 50%;`,
/// `df` → `display: flex;`. `None` quando nao e abreviacao conhecida.
pub fn expandir_css(abrev: &str) -> Option<String> {
    let abrev = abrev.trim();
    // A abreviacao mais longa que casa o comeco: `bgc` antes de `bg`, `df` antes de `d`.
    let (chave, prop, fixo) = CSS
        .iter()
        .filter(|(k, _, _)| abrev.starts_with(k))
        .max_by_key(|(k, _, _)| k.len())?;
    let resto = &abrev[chave.len()..];
    if !fixo.is_empty() {
        return resto.is_empty().then(|| format!("{prop}: {fixo};"));
    }
    if resto.is_empty() {
        return Some(format!("{prop}: ;"));
    }
    if resto.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let valores: Option<Vec<String>> = resto.split('-').map(valor_css).collect();
    Some(format!("{prop}: {};", valores?.join(" ")))
}

fn valor_css(v: &str) -> Option<String> {
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    let (num, unidade) = match v.find(|c: char| c.is_ascii_alphabetic() || c == '%') {
        Some(p) => (&v[..p], &v[p..]),
        None => (v, ""),
    };
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let unidade = match unidade {
        "" => {
            if num == "0" || num.contains('.') {
                ""
            } else {
                "px"
            }
        }
        "p" | "%" => "%",
        "e" => "em",
        "r" => "rem",
        "x" => "ex",
        "v" => "vw",
        "vh" => "vh",
        "px" | "em" | "rem" | "vw" | "pt" => unidade,
        _ => return None,
    };
    Some(format!("{num}{unidade}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filho_e_multiplicacao() {
        assert_eq!(
            expandir_html("div>ul>li*3").unwrap(),
            "<div>\n    <ul>\n        <li></li>\n        <li></li>\n        <li></li>\n    </ul>\n</div>"
        );
    }

    #[test]
    fn classe_sem_nome_vira_div() {
        assert_eq!(expandir_html(".cls").unwrap(), "<div class=\"cls\"></div>");
        assert_eq!(
            expandir_html("span.a.b").unwrap(),
            "<span class=\"a b\"></span>"
        );
    }

    #[test]
    fn id_sem_nome_vira_div() {
        assert_eq!(expandir_html("#id").unwrap(), "<div id=\"id\"></div>");
    }

    #[test]
    fn irmao_com_mais() {
        assert_eq!(expandir_html("p+p").unwrap(), "<p></p>\n<p></p>");
    }

    #[test]
    fn circunflexo_sobe_um_nivel() {
        assert_eq!(
            expandir_html("div>p^span").unwrap(),
            "<div>\n    <p></p>\n</div>\n<span></span>"
        );
    }

    #[test]
    fn texto_entre_chaves_e_numeracao() {
        assert_eq!(expandir_html("a{Ir}").unwrap(), "<a>Ir</a>");
        assert_eq!(
            expandir_html("li.item$*2{Item $}").unwrap(),
            "<li class=\"item1\">Item 1</li>\n<li class=\"item2\">Item 2</li>"
        );
    }

    #[test]
    fn multiplicacao_com_filho_inline() {
        assert_eq!(
            expandir_html("p*2>a").unwrap(),
            "<p><a></a></p>\n<p><a></a></p>"
        );
    }

    #[test]
    fn css_e_o_que_nao_e_emmet() {
        assert_eq!(expandir_css("m10").unwrap(), "margin: 10px;");
        assert_eq!(expandir_css("p10-20").unwrap(), "padding: 10px 20px;");
        assert_eq!(expandir_css("w50p").unwrap(), "width: 50%;");
        assert_eq!(expandir_css("df").unwrap(), "display: flex;");
        assert_eq!(expandir_css("fz1.5e").unwrap(), "font-size: 1.5em;");
        assert_eq!(expandir_css("xyz"), None);
        assert_eq!(expandir_html("ola mundo"), None);
        assert_eq!(expandir_html("div>"), None);
        assert_eq!(expandir_html("br"), Some("<br>".into()));
    }
}

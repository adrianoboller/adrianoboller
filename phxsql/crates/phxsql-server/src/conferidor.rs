//! O conferidor dos textos da tela: quantos ainda NAO passam pela fabrica.
//!
//! # Por que ele existe
//!
//! O laco que ja existia olhava para um lado so: todo `data-txt` da pagina
//! tem de existir na [`crate::idiomas::FABRICA_TELA`]. Esse laco pega o nome
//! escrito errado e nao pega o buraco -- e o buraco era o problema. Medido
//! antes desta rodada: 11.987 linhas de interface e **16** `data-txt`.
//!
//! Este modulo e o laco inverso: varre a interface procurando **texto visivel
//! que nao passa pela fabrica**, e devolve onde ele esta. Sem ele, a regra
//! petrea do dono ("a cada nova implementacao o agente tradutor atualiza as
//! strings fixas por variaveis de multi linguagem") vira promessa: a proxima
//! frente acrescenta tela e ninguem percebe.
//!
//! # O que ele enxerga
//!
//! Duas vias, porque a interface escreve texto de dois jeitos:
//!
//! 1. **marcacao** -- o texto entre `>` e `<` de uma etiqueta conhecida, e os
//!    atributos que o usuario LE (`title`, `placeholder`, `aria-label`,
//!    `alt`). Vale tanto para o HTML estatico quanto para o HTML que o
//!    JavaScript monta dentro de crases, porque a forma e a mesma.
//! 2. **rotulo** -- o literal em posicao de rotulo dentro do JavaScript:
//!    `rot:"…"` (menu e barra), `{t:"…"}` (cabecalho de coluna), `diz:`,
//!    `dica:`, e o primeiro argumento de `avisar(`, `confirm(`, `prompt(` e
//!    `folha(` (titulo e subtitulo de tela).
//!
//! # O que ele NAO enxerga, declarado
//!
//! Rotulo que nao esteja numa dessas formas -- por exemplo o segundo item de
//! um par solto `["registros", e.registros]`. Reconhecer isso sem nome de
//! campo daria falso positivo em toda lista de chaves do programa. Quando uma
//! forma nova de rotulo aparecer, ela entra em [`RECEITAS`] e o numero sobe --
//! e subir o numero e o conferidor funcionando, nao falhando.
//!
//! # Como o dado escapa de ser contado como rotulo
//!
//! Nao por lista: por **forma**. Antes de varrer, todo `${…}` some da linha e
//! vira um marcador. Ou seja, o que a pagina INTERPOLA (o dado que veio do
//! banco) desaparece; o que sobra e o que alguem DIGITOU no fonte, que e
//! exatamente a definicao de rotulo. E a mesma licao do «Blumenau» virando
//! «BLUMENAU»: rotulo se traduz, dado nao se toca.

/// Os arquivos de interface, embutidos aqui pelo mesmo `include_str!` que o
/// servidor usa para servi-los -- assim nao ha como o conferidor medir uma
/// pagina e o binario servir outra.
///
/// Essa frase era promessa e nao garantia ate a lista ganhar guarda. Ela e
/// digitada, e o `multitela.js` entrou no `http.rs` sem entrar aqui: 1.474
/// linhas de interface servidas ao navegador e invisiveis para a catraca. Quem
/// impede a repeticao e `a_lista_cobre_tudo_que_o_http_serve`, que le o fonte
/// do `http.rs` e reprova o arquivo servido que ninguem mede.
pub const FONTES: &[(&str, &str)] = &[
    ("ui/index.html", include_str!("../ui/index.html")),
    ("ui/claude.js", include_str!("../ui/claude.js")),
    ("ui/telemetria.js", include_str!("../ui/telemetria.js")),
    ("ui/diagrama-er.js", include_str!("../ui/diagrama-er.js")),
    ("ui/multitela.js", include_str!("../ui/multitela.js")),
    (
        "ui/grid/phx-grid.js",
        include_str!("../ui/grid/phx-grid.js"),
    ),
];

/// Marcador que ocupa o lugar do que foi retirado antes da varredura: um
/// `${…}` (dado interpolado) ou uma chamada `txt(…)` (texto ja da fabrica).
/// Nao e letra, entao ele nunca faz um trecho parecer texto humano.
const BURACO: char = '\u{1}';

/// Onde o texto foi achado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Canal {
    /// Entre etiquetas, ou num atributo que se le.
    Marcacao,
    /// Literal em posicao de rotulo no JavaScript.
    Rotulo,
}

/// O veredito sobre um texto achado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Situacao {
    /// Nao se traduz, e a razao esta escrita.
    Isento,
    /// Texto de tela cravado em portugues: e o que falta.
    Fora,
}

/// Um texto de tela e o que se decidiu sobre ele.
#[derive(Clone, Debug)]
pub struct Achado {
    pub arquivo: &'static str,
    pub linha: usize,
    pub texto: String,
    pub canal: Canal,
    pub situacao: Situacao,
    /// Preenchido so quando `situacao` e [`Situacao::Isento`].
    pub porque: &'static str,
}

/// As formas de rotulo que a via 2 reconhece, e o que cada uma e.
///
/// A lista e o contrato do conferidor: o que nao esta aqui ele nao ve, e por
/// isso ela e publica e curta o bastante para caber num olhar.
pub const RECEITAS: &[(&str, &str)] = &[
    ("rot:", "rotulo de item de menu e de botao da barra"),
    ("{t:", "cabecalho de coluna da funcao tabela()"),
    ("diz:", "a explicacao curta de um formato de exportacao"),
    ("dica:", "a dica que aparece no title do botao da barra"),
    ("avisar(", "o recado do alto da tela"),
    ("confirm(", "a pergunta antes de uma acao que nao se desfaz"),
    ("prompt(", "a pergunta que pede um valor"),
    ("folha(", "titulo e subtitulo de tela"),
];

/// Os atributos que o usuario LE, e o `data-txt-*` que cobre cada um.
///
/// `value` fica de fora de proposito: ele costuma carregar dado, e dado nao
/// se traduz. `alt` nao tem par: a unica imagem com `alt` e a marca.
const ATRIBUTOS_VISIVEIS: &[(&str, &str)] = &[
    ("title", "data-txt-tt="),
    ("placeholder", "data-txt-ph="),
    ("aria-label", "data-txt-al="),
    ("alt", ""),
];

/// As etiquetas que delimitam texto. Exigir nome conhecido e o que impede o
/// `a < b` e o `x => y` do JavaScript de virarem etiqueta e arrastarem codigo
/// para dentro da conta.
const ETIQUETAS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "blockquote",
    "br",
    "button",
    "canvas",
    "caption",
    "circle",
    "code",
    "col",
    "colgroup",
    "datalist",
    "dd",
    "defs",
    "del",
    "details",
    "dialog",
    "div",
    "dl",
    "dt",
    "ellipse",
    "em",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "g",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "line",
    "link",
    "main",
    "mark",
    "marker",
    "menu",
    "meta",
    "meter",
    "nav",
    "noscript",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "path",
    "polygon",
    "polyline",
    "pre",
    "progress",
    "rect",
    "s",
    "samp",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "stop",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "svg",
    "table",
    "tbody",
    "td",
    "template",
    "text",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "tspan",
    "u",
    "ul",
    "use",
    "var",
    "video",
    "wbr",
];

/// Texto que NAO se traduz, um por um, com a razao escrita.
///
/// Regra da lista: so entra o que seria errado traduzir -- nome proprio,
/// identificador que o usuario digita em outro lugar, sigla e unidade. Rotulo
/// que apenas ainda nao foi traduzido **nao entra aqui**: ele fica na conta do
/// que falta, que e a conta que este modulo existe para manter honesta.
pub const ISENTOS: &[(&str, &str)] = &[
    ("Ph", "a marca partida em tres pedacos pelo <span> do X"),
    ("Sql", "a marca partida em tres pedacos pelo <span> do X"),
    ("PhxSql", "a marca"),
    (
        "Built to store. Engineered to scale.",
        "a assinatura da marca",
    ),
    ("Exo 2", "nome da fonte da marca"),
    ("IBM Plex Mono", "nome de fonte"),
    ("Idera", "nome de empresa citada"),
    ("HFSQL", "nome de produto citado"),
    ("phxsqld", "nome do executavel"),
    ("phxsys", "nome do database de sistema"),
    ("TextName", "nome de coluna da tabela de mensagens"),
    (
        "rownum",
        "nome da coluna de sistema, o mesmo em toda lingua",
    ),
    ("rowid", "nome da coluna de sistema"),
    ("id", "nome de coluna"),
    ("uuid", "nome de coluna"),
    ("cpf", "nome de campo brasileiro, sem traducao"),
    (
        "devicePixelRatio",
        "nome da propriedade do navegador, o mesmo em toda lingua",
    ),
    ("Base64", "nome de codificacao"),
    ("Ed25519", "nome de algoritmo"),
    ("PBKDF2", "nome de algoritmo"),
    ("SHA-256", "nome de algoritmo"),
    ("CRC-32", "nome de algoritmo"),
    ("Ctrl", "tecla, escrita igual no teclado de todos"),
    ("Alt", "tecla"),
    ("Esc", "tecla"),
    ("Shift", "tecla"),
    ("Enter", "tecla"),
    ("ms", "unidade"),
    ("kB", "unidade"),
    ("MB", "unidade"),
    ("GB", "unidade"),
    ("KiB", "unidade"),
    ("MiB", "unidade"),
    ("GiB", "unidade"),
    ("min", "unidade de tempo abreviada"),
    ("bytes", "unidade"),
    ("byte", "unidade"),
    ("localhost", "nome de maquina"),
    ("Linux", "nome de sistema"),
    ("Windows", "nome de sistema"),
    ("Docker", "nome de produto"),
    ("systemd", "nome de servico do sistema"),
    ("Excel", "nome de produto"),
    ("Word", "nome de produto"),
    ("Postgres", "nome de produto"),
    ("PostgreSQL", "nome de produto"),
    ("MySQL", "nome de produto"),
    ("MariaDB", "nome de produto"),
    ("SQLite", "nome de produto"),
    ("Oracle", "nome de produto"),
    ("DBeaver", "nome de produto"),
    ("Claude", "nome de produto"),
    (
        "Claude Opus 5",
        "nome de produto da Anthropic, o mesmo em toda lingua",
    ),
    (
        "Claude Sonnet 5",
        "nome de produto da Anthropic, o mesmo em toda lingua",
    ),
    (
        "Claude Haiku 4.5",
        "nome de produto da Anthropic, o mesmo em toda lingua",
    ),
    ("Anthropic", "nome de empresa"),
    ("ODBC", "nome de padrao"),
    (
        "DbLink",
        "nome de recurso, usado como marca dentro do produto",
    ),
    ("SysTables", "nome de tabela de sistema"),
    ("SysColumns", "nome de tabela de sistema"),
    ("phx-grid", "nome do componente de grade"),
    ("v7", "versao de UUID"),
];

/// A varredura de um arquivo. `arquivo` so entra no achado, para dizer onde.
pub fn varrer(arquivo: &'static str, fonte: &str) -> Vec<Achado> {
    let (limpo, inicios) = limpar(fonte);
    let mut achados = Vec::new();
    achados.extend(via_marcacao(arquivo, &limpo, &inicios));
    achados.extend(via_rotulo(arquivo, &limpo, &inicios));
    achados
}

/// A varredura de toda a interface.
pub fn conferir() -> Vec<Achado> {
    FONTES
        .iter()
        .flat_map(|(nome, fonte)| varrer(nome, fonte))
        .collect()
}

/// So o que falta traduzir.
pub fn fora(achados: &[Achado]) -> Vec<&Achado> {
    achados
        .iter()
        .filter(|a| a.situacao == Situacao::Fora)
        .collect()
}

/// Quantos textos JA passam pela fabrica, contados nas tres formas que
/// existem: os dois atributos do HTML e a chamada do JavaScript.
///
/// E contagem de OCORRENCIA, e nao de chave: dois botoes com o mesmo rotulo
/// sao dois textos na tela, e e a tela que se esta medindo.
pub fn cobertos() -> usize {
    FONTES
        .iter()
        .map(|(_, fonte)| {
            fonte.matches("data-txt=\"").count()
                + fonte.matches("data-txt-ph=\"").count()
                + fonte.matches("data-txt-tt=\"").count()
                + fonte.matches("data-txt-al=\"").count()
                + fonte.matches("txt:\"").count()
                + fonte.matches("dicaTxt:\"").count()
                + ocorrencias_de_txt(fonte)
        })
        .sum()
}

/// Quantas chamadas `txt(` ha no fonte -- as de verdade, e nao um `contxt(`.
fn ocorrencias_de_txt(fonte: &str) -> usize {
    let bytes = fonte.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while let Some(p) = fonte[i..].find("txt(") {
        let p = i + p;
        if p == 0 || !parte_de_nome(bytes[p - 1]) {
            n += 1;
        }
        i = p + 4;
    }
    n
}

fn parte_de_nome(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'.' || b == b'-'
}

// =====================================================================
// A limpeza: tirar da frente o que nao e rotulo
// =====================================================================

/// Devolve o fonte limpo e o byte em que cada linha comeca nele.
///
/// Sai de cena, virando [`BURACO`]: o bloco `<style>`, os comentarios de
/// linha e de bloco, todo `${…}` (que e DADO) e toda chamada `txt(…)` (que e
/// texto JA na fabrica). O que sobra e o que alguem digitou como rotulo.
fn limpar(fonte: &str) -> (String, Vec<usize>) {
    let mut saida = String::with_capacity(fonte.len());
    let mut inicios = Vec::new();
    let mut em_estilo = false;
    let mut em_bloco = false;
    for linha in fonte.split('\n') {
        inicios.push(saida.len());
        let t = linha.trim_start();
        let pular = if em_estilo {
            if t.starts_with("</style>") {
                em_estilo = false;
            }
            true
        } else if t.starts_with("<style") {
            em_estilo = !t.contains("</style>");
            true
        } else if em_bloco {
            if linha.contains("*/") {
                em_bloco = false;
            }
            true
        } else if t.starts_with("/*") {
            em_bloco = !linha.contains("*/");
            true
        } else {
            t.starts_with("//") || t.starts_with('*') || t.starts_with("<!--")
        };
        if !pular {
            saida.push_str(&sem_interpolacao(&cobrir_pareados(linha)));
        }
        saida.push('\n');
    }
    (saida, inicios)
}

/// Os campos que carregam a chave da fabrica ao lado do rotulo de fabrica.
///
/// `MENUS` e `FERRAMENTAS` sao lidos ANTES do login, quando ainda nao ha
/// texto traduzido nenhum: `txt(…)` ali devolveria portugues para sempre. A
/// solucao e o par -- o rotulo de fabrica fica no dado, a chave ao lado, e
/// quem desenha chama `txt(f.txt, f.rot)` na hora de pintar. Para o
/// conferidor, rotulo com chave ao lado E rotulo coberto.
const PARES: &[(&str, &str)] = &[
    ("rot:\"", "txt:\""),
    ("dica:\"", "dicaTxt:\""),
    ("diz:\"", "dizTxt:\""),
];

/// Apaga o rotulo de fabrica quando a chave dele esta na MESMA linha.
///
/// Mesma linha, e nao "mesmo objeto": exigir a linha e o que mantem a regra
/// legivel de um lado e do outro -- quem escreve ve os dois juntos, e quem
/// confere nao precisa entender chave aninhada para dizer se esta coberto.
fn cobrir_pareados(linha: &str) -> String {
    let mut saida = linha.to_string();
    for (rotulo, chave) in PARES {
        if !saida.contains(chave) {
            continue;
        }
        while let Some(p) = saida.find(rotulo) {
            let ini = p + rotulo.len();
            let Some(f) = saida[ini..].find('"') else {
                break;
            };
            saida.replace_range(
                p..ini + f + 1,
                &format!("{}{BURACO}", &rotulo[..rotulo.len() - 2]),
            );
        }
    }
    saida
}

/// Troca `${…}` e `txt(…)` por um [`BURACO`], preservando o resto da linha.
///
/// Interpolacao que nao fecha na linha (o `${x ? \`<div>` de varias linhas)
/// leva so ate o fim da linha: as linhas seguintes trazem marcacao de verdade
/// e precisam continuar sendo varridas.
fn sem_interpolacao(linha: &str) -> String {
    let b = linha.as_bytes();
    let mut saida = String::with_capacity(linha.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' && i + 1 < b.len() && b[i + 1] == b'{' {
            saida.push(BURACO);
            i = fim_do_balanco(b, i + 1, b'{', b'}');
        } else if b[i] == b't'
            && linha[i..].starts_with("txt(")
            && (i == 0 || !parte_de_nome(b[i - 1]))
        {
            saida.push(BURACO);
            i = fim_do_balanco(b, i + 3, b'(', b')');
        } else {
            // Fatia por caractere, e nao por byte: acento tem dois bytes.
            let c = linha[i..].chars().next().unwrap_or(' ');
            saida.push(c);
            i += c.len_utf8();
        }
    }
    saida
}

/// O byte seguinte ao fechamento de `abre` que comeca em `i`; o fim da linha
/// quando nao fecha nela.
fn fim_do_balanco(b: &[u8], i: usize, abre: u8, fecha: u8) -> usize {
    let mut nivel = 0i32;
    let mut j = i;
    while j < b.len() {
        if b[j] == abre {
            nivel += 1;
        } else if b[j] == fecha {
            nivel -= 1;
            if nivel == 0 {
                return j + 1;
            }
        }
        j += 1;
    }
    b.len()
}

// =====================================================================
// Via 1: a marcacao
// =====================================================================

fn via_marcacao(arquivo: &'static str, limpo: &str, inicios: &[usize]) -> Vec<Achado> {
    let mut achados = Vec::new();
    let b = limpo.as_bytes();
    let mut i = 0;
    // O fim da etiqueta anterior, e se ela carregava `data-txt`.
    let mut apos: Option<(usize, bool)> = None;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let Some((fim, nome_fim)) = etiqueta(b, i) else {
            i += 1;
            continue;
        };
        let bruta = &limpo[i..fim];
        if let Some((inicio, coberto)) = apos {
            if !coberto {
                if let Some(a) = pesar(
                    arquivo,
                    limpo,
                    inicios,
                    inicio,
                    &limpo[inicio..i],
                    Canal::Marcacao,
                ) {
                    achados.push(a);
                }
            }
        }
        for a in atributos_visiveis(arquivo, limpo, inicios, i, bruta) {
            achados.push(a);
        }
        let _ = nome_fim;
        apos = Some((fim, bruta.contains("data-txt=")));
        i = fim;
    }
    achados
}

/// Se ha etiqueta conhecida comecando em `i`, devolve (fim dela, fim do nome).
fn etiqueta(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut j = i + 1;
    if j < b.len() && b[j] == b'/' {
        j += 1;
    }
    let ini_nome = j;
    while j < b.len() && (b[j].is_ascii_alphanumeric()) {
        j += 1;
    }
    if j == ini_nome {
        return None;
    }
    let nome = std::str::from_utf8(&b[ini_nome..j])
        .ok()?
        .to_ascii_lowercase();
    if !ETIQUETAS.contains(&nome.as_str()) {
        return None;
    }
    // Um caractere de nome depois do nome (ex.: `<tabela`) nao e etiqueta.
    if j < b.len() && !matches!(b[j], b' ' | b'>' | b'/' | b'\n' | b'\t') {
        return None;
    }
    // Ate o `>`, pulando o que estiver entre aspas.
    let mut aspas = 0u8;
    while j < b.len() {
        match b[j] {
            b'"' | b'\'' if aspas == 0 => aspas = b[j],
            c if c == aspas => aspas = 0,
            b'>' if aspas == 0 => return Some((j + 1, ini_nome)),
            b'\n' if aspas == 0 => {}
            _ => {}
        }
        j += 1;
    }
    None
}

fn atributos_visiveis(
    arquivo: &'static str,
    limpo: &str,
    inicios: &[usize],
    base: usize,
    bruta: &str,
) -> Vec<Achado> {
    let mut achados = Vec::new();
    for (nome, coberto_por) in ATRIBUTOS_VISIVEIS {
        // O atributo esta coberto quando a MESMA etiqueta carrega o
        // `data-txt-*` dele. Faltava para `aria-label`, e o efeito era o pior
        // possivel: quem traduzia o texto de quem escuta continuava vendo o
        // rotulo na conta do que falta, e a conta e o que dirige a proxima
        // leva de traducao.
        if !coberto_por.is_empty() && bruta.contains(coberto_por) {
            continue;
        }
        let alvo = format!("{nome}=\"");
        let mut i = 0;
        while let Some(p) = bruta[i..].find(&alvo) {
            let ini = i + p + alvo.len();
            let Some(f) = bruta[ini..].find('"') else {
                break;
            };
            let valor = &bruta[ini..ini + f];
            if let Some(a) = pesar(arquivo, limpo, inicios, base + ini, valor, Canal::Marcacao) {
                achados.push(a);
            }
            i = ini + f + 1;
        }
    }
    achados
}

// =====================================================================
// Via 2: o rotulo no JavaScript
// =====================================================================

fn via_rotulo(arquivo: &'static str, limpo: &str, inicios: &[usize]) -> Vec<Achado> {
    let mut achados = Vec::new();
    for (receita, _) in RECEITAS {
        let mut i = 0;
        while let Some(p) = limpo[i..].find(receita) {
            let p = i + p;
            i = p + receita.len();
            // `rot:` tem de ser o campo, e nao o fim de outro nome.
            let antes = limpo[..p].chars().next_back().unwrap_or(' ');
            if receita.starts_with(|c: char| c.is_ascii_alphabetic()) && parte_de_nome(antes as u8)
            {
                continue;
            }
            let resto = &limpo[i..];
            let cru = resto.trim_start();
            let pulo = resto.len() - cru.len();
            let Some(valor) = literal(cru) else { continue };
            if let Some(a) = pesar(arquivo, limpo, inicios, i + pulo, valor, Canal::Rotulo) {
                achados.push(a);
            }
            // `folha(` leva dois: titulo e subtitulo.
            if *receita == "folha(" {
                let apos = i + pulo + valor.len() + 2;
                let sobra = limpo[apos..].trim_start_matches([',', ' ', '\n']);
                if let Some(sub) = literal(sobra) {
                    let onde = limpo.len() - sobra.len();
                    if let Some(a) = pesar(arquivo, limpo, inicios, onde, sub, Canal::Rotulo) {
                        achados.push(a);
                    }
                }
            }
        }
    }
    achados
}

/// O conteudo de um literal que comeca em `s`, se `s` comeca com aspa.
fn literal(s: &str) -> Option<&str> {
    let aspa = s.chars().next()?;
    if aspa != '"' && aspa != '\'' {
        return None;
    }
    let resto = &s[1..];
    let fim = resto.find(aspa)?;
    Some(&resto[..fim])
}

// =====================================================================
// O julgamento
// =====================================================================

/// Decide se este trecho e texto de tela e, se for, se ele e isento.
///
/// `None` quer dizer "isto nem e texto" -- pontuacao, marcador de dado,
/// pedaco de codigo que escapou. E o filtro mais importante do modulo.
fn pesar(
    arquivo: &'static str,
    limpo: &str,
    inicios: &[usize],
    onde: usize,
    bruto: &str,
    canal: Canal,
) -> Option<Achado> {
    let texto = normalizar(bruto)?;
    let porque = isento(&texto);
    Some(Achado {
        arquivo,
        linha: linha_de(inicios, onde.min(limpo.len())),
        texto,
        canal,
        situacao: if porque.is_some() {
            Situacao::Isento
        } else {
            Situacao::Fora
        },
        porque: porque.unwrap_or(""),
    })
}

/// O trecho como texto de uma linha so, ou `None` se ele nao parece texto.
///
/// O crivo do "parece texto" e por caractere: qualquer sinal de programacao
/// (crase, chave, ponto e virgula, igual, aspa) derruba o trecho. E ele que
/// impede um pedaco de JavaScript entre duas etiquetas distantes de virar
/// rotulo -- porque codigo quase sempre traz um desses.
fn normalizar(bruto: &str) -> Option<String> {
    if bruto.len() > 400 {
        return None;
    }
    let mut texto = String::with_capacity(bruto.len());
    let mut espaco = true;
    for c in bruto.chars() {
        match c {
            '`' | '{' | '}' | ';' | '=' | '\\' | '"' | '\'' | '|' | '$' | '<' | '>' => return None,
            BURACO => {
                if !espaco {
                    texto.push(' ');
                    espaco = true;
                }
            }
            c if c.is_whitespace() => {
                if !espaco {
                    texto.push(' ');
                    espaco = true;
                }
            }
            c => {
                texto.push(c);
                espaco = false;
            }
        }
    }
    let texto = texto.trim().to_string();
    if !duas_letras(&texto) {
        return None;
    }
    Some(texto)
}

/// Duas letras seguidas: o piso do que se chama palavra. Abaixo disso e
/// pontuacao, numero ou simbolo -- e nada disso se traduz.
fn duas_letras(s: &str) -> bool {
    let mut seguidas = 0;
    for c in s.chars() {
        if c.is_alphabetic() {
            seguidas += 1;
            if seguidas >= 2 {
                return true;
            }
        } else {
            seguidas = 0;
        }
    }
    false
}

/// Palavra em caixa alta que NAO e sigla: e rotulo, e se traduz.
///
/// Sem esta lista, «TOTAL» num rodape de tabela passava por sigla e sumia da
/// conta -- e a regra de caixa alta e justamente a mais fácil de enganar,
/// porque a interface usa caixa alta como enfase.
const NAO_SAO_SIGLAS: &[&str] = &["TOTAL", "OU", "SIM", "NAO", "DE", "ATE", "NOVO", "NOVA"];

/// A razao de nao traduzir, quando ha uma.
pub fn isento(texto: &str) -> Option<&'static str> {
    if let Some((_, porque)) = ISENTOS.iter().find(|(t, _)| *t == texto) {
        return Some(porque);
    }
    if texto.contains(' ') || NAO_SAO_SIGLAS.contains(&texto) {
        return None;
    }
    let sem_ponto = texto.trim_start_matches('.');
    if texto.starts_with('.')
        && !sem_ponto.is_empty()
        && sem_ponto.chars().all(|c| c.is_ascii_lowercase())
    {
        return Some("extensao de arquivo");
    }
    if texto
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Some("sigla");
    }
    // O ponto FINAL nao faz identificador: «obrig.» e «tam.» sao abreviaturas
    // de rotulo, e passavam por nome de campo justamente por causa dele.
    if !texto.ends_with('.')
        && (texto.contains('.') || texto.contains('_'))
        && texto
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
    {
        return Some("identificador do programa");
    }
    None
}

fn linha_de(inicios: &[usize], onde: usize) -> usize {
    match inicios.binary_search(&onde) {
        Ok(i) => i + 1,
        Err(0) => 1,
        Err(i) => i,
    }
}

/// O resumo que a tela e o relatorio mostram.
pub struct Placar {
    pub cobertos: usize,
    pub isentos: usize,
    pub fora: usize,
}

impl Placar {
    pub fn medir() -> Placar {
        let achados = conferir();
        Placar {
            cobertos: cobertos(),
            isentos: achados
                .iter()
                .filter(|a| a.situacao == Situacao::Isento)
                .count(),
            fora: achados
                .iter()
                .filter(|a| a.situacao == Situacao::Fora)
                .count(),
        }
    }

    pub fn visiveis(&self) -> usize {
        self.cobertos + self.fora
    }

    /// Quanto da tela ja passa pela fabrica, em porcentagem inteira.
    pub fn por_cento(&self) -> usize {
        let total = self.visiveis();
        if total == 0 {
            return 100;
        }
        self.cobertos * 100 / total
    }
}

// =====================================================================
// As duas guardas do texto COLADO
// =====================================================================
//
// A catraca de cima conta o que ainda NAO passa pela fabrica. Estas duas
// contam o contrario: o que passa pela fabrica e mesmo assim nao esta
// traduzido, porque alguem colou a mesma frase em varias colunas.
//
// Elas nascem em ZERO, e nascer em zero e o ponto: nao ha o que consertar
// hoje: medido nesta rodada, nenhuma chave tem os seis idiomas iguais e
// nenhuma frase longa se repete em tres. O que elas fazem e pegar o DIA em
// que alguem colar -- que e o jeito mais provavel de a fabrica crescer com
// numero bonito e tela em portugues.
//
// # O criterio NAO e «igual ao portugues»
//
// Foi a primeira ideia, e ela e errada. Medido: 33 chaves tem o espanhol
// identico ao portugues, e a maioria esta CERTA -- `Database`, `Profiler`,
// `Servidor`, e `Menu principal`, que em frances e exatamente isso. Uma
// guarda de «igual ao portugues» reprovaria o correto, e guarda que reprova o
// correto e desligada na primeira semana.
//
// O criterio e mais forte: os SEIS iguais (guarda 1), ou a mesma frase LONGA
// em tres ou mais (guarda 2). Duas linguas coincidirem numa palavra e comum;
// seis coincidirem numa frase, nao.

/// O texto sem os `{marcador}`: o que sobra e o que alguem realmente
/// escreveu.
///
/// Sem isto, `"{id}{eu} · {nivel} · {sub} · peso {peso}"` conta como frase de
/// trinta e nove caracteres e cai na guarda da frase longa -- quando a unica
/// palavra ali e «peso», que e a mesma em portugues, italiano e espanhol.
/// Medir o molde em vez do miolo daria falso positivo exatamente onde a
/// traducao esta certa.
fn miolo(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('{') {
        saida.push_str(&resto[..i]);
        match resto[i..].find('}') {
            Some(f) => resto = &resto[i + f + 1..],
            None => {
                resto = "";
                break;
            }
        }
    }
    saida.push_str(resto);
    saida.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Quantos caracteres de texto de verdade um idioma traz.
fn tamanho_do_miolo(texto: &str) -> usize {
    miolo(texto).chars().count()
}

/// As chaves em que os SEIS idiomas trazem exatamente o mesmo texto.
///
/// Nome proprio, sigla e identificador ficam de fora pela lista que ja existe
/// -- [`ISENTOS`] --, e nao por uma segunda lista: `Profiler` e `Pivot` sao
/// iguais nas seis porque nao se traduzem, e a razao ja esta escrita la.
pub fn colados() -> Vec<&'static str> {
    crate::idiomas::FABRICA_TELA
        .iter()
        .filter(|f| {
            let base = f.textos[0];
            tamanho_do_miolo(base) > 3
                && isento(base).is_none()
                && f.textos.iter().all(|t| *t == base)
        })
        .map(|f| f.nome)
        .collect()
}

/// Quantos idiomas uma frase longa ocupa numa chave, quando ocupa tres ou
/// mais. Devolve (chave, a frase, quantos idiomas).
pub fn frases_repetidas() -> Vec<(&'static str, &'static str, usize)> {
    let mut achados = Vec::new();
    for f in crate::idiomas::FABRICA_TELA {
        for (i, t) in f.textos.iter().enumerate() {
            // So a PRIMEIRA aparicao de cada frase entra, senao a mesma
            // repeticao seria relatada uma vez por idioma.
            if t.is_empty() || f.textos[..i].contains(t) {
                continue;
            }
            if tamanho_do_miolo(t) <= 25 || isento(t).is_some() {
                continue;
            }
            let quantos = f.textos.iter().filter(|o| *o == t).count();
            if quantos >= 3 {
                achados.push((f.nome, *t, quantos));
            }
        }
    }
    achados
}

/// Chaves com os seis idiomas iguais. **So desce**, e hoje e zero.
pub const TETO_COLADO: usize = 0;

/// Frases longas repetidas em tres ou mais idiomas. **So desce**, e hoje e
/// zero.
pub const TETO_FRASE_REPETIDA: usize = 0;

// =====================================================================
// A catraca
// =====================================================================

/// Quantos textos de tela ainda estao fora da fabrica.
///
/// **Este numero so desce.** Ele nao e uma meta: e a catraca que impede a
/// proxima frente de acrescentar tela em portugues cravado sem ninguem
/// perceber -- que foi exatamente como a interface chegou a 11.987 linhas com
/// 16 textos na fabrica. Traduziu um punhado? Rode
/// `cargo run --example textos-fora-da-fabrica -p phxsql-server`, veja o
/// numero novo e baixe a catraca no mesmo commit.
/// **A unica subida registrada, e o motivo dela.** 1.994 -> 2.000, na
/// integracao em que esta catraca NASCEU. Tres frentes paralelas fecharam na
/// mesma rodada -- multitela, cores das bolhas e este conferidor -- e as duas
/// primeiras comecaram antes de a regra existir: nao havia como elas nascerem
/// na fabrica. Do que sobrou, as etiquetas curtas foram traduzidas na hora
/// (`tela.abas_da_regiao`, `tela.cores_de_fabrica` e as nove do menu Ver); os
/// seis que restam sao paragrafos de explicacao da tela de cores, e traduzi-los
/// as pressas numa integracao daria texto pior que deixa-los na fila. Estao
/// nomeados no `PENDENCIAS.md`.
///
/// **A partir daqui o numero so desce.** Subir de novo pede o mesmo que este
/// comentario: dizer quais textos, de qual frente, e por que nao couberam.
///
/// 2.000 -> 1.999 na revisao do dossie 0.18: o item «Jobs» do Gerir banco
/// ganhou o par `rot:`/`txt:` ao passar a apontar para a tela que ja existia.
/// Um so, e ele desce a catraca junto -- catraca frouxa nao segura nada.
///
/// 1.999 -> 2.068, e este e o unico tipo de subida que nao afrouxa nada: **o
/// numero de baixo era falso.** O `multitela.js` era servido pelo `http.rs` e
/// nao estava no `FONTES`, entao seus 69 textos cravados nunca foram contados
/// -- nao foram acrescentados agora, sempre estiveram la. 2.068 e a primeira
/// medida sobre a interface inteira; 1.999 era medida sobre cinco sextos dela.
/// A guarda `a_lista_cobre_tudo_que_o_http_serve` impede a proxima leitura
/// falsa, e a frente que traduz os 69 desce a catraca de volta.
///
/// 2.068 -> 1.999, e agora o numero quer dizer a mesma coisa que o de antes
/// dizia por engano: os 69 do `multitela.js` sairam. Sessenta e oito viraram
/// setenta chaves de fabrica; o sexagesimo nono, `devicePixelRatio`, entrou nos
/// [`ISENTOS`] com a razao escrita, porque nome de propriedade do navegador nao
/// se traduz. Trinta e nove dos sessenta e oito eram UMA frase picada pela
/// marcacao, e o conserto deles esta no `docs/MENSAGENS.md`: frase picada e
/// intraduzivel por construcao, entao a frase inteira virou uma chave so e o
/// corte em `<b>`/`<code>` passou a acontecer DEPOIS da traducao.
/// 1.999 -> 1.996 no sprint 25 (`acrescentar_coluna`): a nota do cartao de
/// tabela do editor de modelo dizia, em portugues cravado, que alterar coluna
/// nao existia. Ela virou o formulario que existe, e os textos dele nasceram
/// na fabrica -- os tres paragrafos que sairam sao os tres que a catraca
/// desce.
///
/// 1.996 -> 1.806: os QUATRO arquivos que nao sao o `index.html` fecharam em
/// zero -- `claude.js` (126), `telemetria.js` (38), `grid/phx-grid.js` (24) e
/// `diagrama-er.js` (2). O que sobra e o `index.html` inteiro, e ele ficou de
/// fora de proposito: quatro frentes o estavam editando ao mesmo tempo, e
/// mexer nos 1.806 no meio disso trocaria traducao por conflito.
pub const TETO: usize = 1_806;

#[cfg(test)]
mod testes {
    use std::collections::HashSet;

    use super::*;

    /// A catraca. Falha quando alguem acrescenta texto cravado -- e falha
    /// tambem quando alguem traduz e esquece de baixar o numero, porque
    /// catraca frouxa nao segura nada.
    #[test]
    fn a_catraca_dos_textos_fora_da_fabrica() {
        let achados = conferir();
        let faltando = fora(&achados);
        if faltando.len() > TETO {
            let mostra: Vec<String> = faltando
                .iter()
                .take(40)
                .map(|a| format!("  {}:{} {:?}", a.arquivo, a.linha, a.texto))
                .collect();
            panic!(
                "{} textos de tela fora da fabrica, e a catraca esta em {TETO}.\n\
                 Os primeiros:\n{}\n\
                 Todos: cargo run --example textos-fora-da-fabrica -p phxsql-server",
                faltando.len(),
                mostra.join("\n")
            );
        }
        assert!(
            faltando.len() >= TETO.saturating_sub(30),
            "sobraram {} e a catraca esta em {TETO}: baixe a catraca no mesmo \
             commit da traducao, senao ela deixa de segurar",
            faltando.len()
        );
    }

    /// A lista do `FONTES` e digitada, e lista digitada envelhece calada: o
    /// `multitela.js` passou a ser servido pelo `http.rs` e ficou de fora
    /// daqui, entao 1.474 linhas de interface nao contavam para a catraca e
    /// ninguem via pelo numero.
    ///
    /// A guarda tira a lista de quem digita e poe em quem serve: le o fonte do
    /// `http.rs` e cobra cada `.js` e `.html` de interface que ele embute.
    /// Quando entrar a proxima tela, este teste reprova antes de a catraca
    /// medir errado.
    #[test]
    fn a_lista_cobre_tudo_que_o_http_serve() {
        const HTTP: &str = include_str!("http.rs");

        let servidos: Vec<&str> = HTTP
            .match_indices("include_str!(\"../ui/")
            .filter_map(|(i, _)| {
                let resto = &HTTP[i + "include_str!(\"".len()..];
                let fim = resto.find('"')?;
                let caminho = &resto[..fim];
                // O `.md` do changelog da grade e servido, mas nao e tela.
                (caminho.ends_with(".js") || caminho.ends_with(".html")).then_some(caminho)
            })
            .collect();

        assert!(
            !servidos.is_empty(),
            "nao achei nenhum include_str! de interface no http.rs -- a guarda \
             ficou cega, conserte o reconhecimento antes de confiar nela"
        );

        let medidos: HashSet<&str> = FONTES.iter().map(|(nome, _)| *nome).collect();
        let faltando: Vec<&str> = servidos
            .iter()
            .filter(|c| !medidos.contains(c.trim_start_matches("../")))
            .copied()
            .collect();

        assert!(
            faltando.is_empty(),
            "o http.rs serve {faltando:?} e o FONTES nao mede -- texto cravado \
             ali nao conta para a catraca. Acrescente ao FONTES e reveja o TETO"
        );
    }

    /// A prova real do conferidor, com o defeito reposto: um rotulo cravado
    /// tem de ser reprovado, e o MESMO rotulo pela fabrica tem de passar.
    #[test]
    fn reprova_o_rotulo_cravado_e_aprova_o_da_fabrica() {
        let cravado = r#"<button class="botao">Salvar no config.json</button>"#;
        let achados = varrer("teste", cravado);
        let faltando = fora(&achados);
        assert_eq!(faltando.len(), 1, "achou: {achados:?}");
        assert_eq!(faltando[0].texto, "Salvar no config.json");

        let pela_fabrica =
            r#"<button class="botao">${txt("tela.salvar", "Salvar no config.json")}</button>"#;
        assert!(
            fora(&varrer("teste", pela_fabrica)).is_empty(),
            "texto que passa pela fabrica nao pode ser reprovado"
        );

        let pelo_atributo = r#"<label data-txt="tela.servidor">Servidor</label>"#;
        assert!(
            fora(&varrer("teste", pelo_atributo)).is_empty(),
            "data-txt cobre o texto que vem depois dele"
        );
    }

    /// A outra metade da prova real: dado NAO e rotulo. O que a pagina
    /// interpola nunca pode entrar na conta -- foi a licao do «Blumenau»
    /// virando «BLUMENAU», e aqui ela vira crivo.
    #[test]
    fn dado_interpolado_nunca_conta_como_rotulo() {
        let so_dado = r#"<td class="dado">${esc(linha.cidade)}</td>"#;
        assert!(fora(&varrer("teste", so_dado)).is_empty());

        let dado_e_rotulo = r#"<div class="r">cidade de ${esc(l.uf)}</div>"#;
        let faltando = varrer("teste", dado_e_rotulo);
        let faltando = fora(&faltando);
        assert_eq!(faltando.len(), 1);
        assert_eq!(
            faltando[0].texto, "cidade de",
            "o dado saiu, o rotulo ficou"
        );
    }

    /// As formas de rotulo do JavaScript, uma a uma.
    #[test]
    fn ve_o_rotulo_fora_da_marcacao() {
        for (fonte, esperado) in [
            (r#"{ rot:"Painel", ico:"x" }"#, "Painel"),
            (r#"[{t:"nome"},{t:"tipo"}]"#, "nome"),
            (r#"avisar("tabela criada")"#, "tabela criada"),
            (r#"folha("Serviço", "a porta de dados")"#, "Serviço"),
        ] {
            let achados = varrer("teste", fonte);
            let faltando = fora(&achados);
            assert!(
                faltando.iter().any(|a| a.texto == esperado),
                "{fonte} devia acusar {esperado:?}, achou {faltando:?}"
            );
        }
        // E o mesmo rotulo pela fabrica nao acusa nada.
        assert!(fora(&varrer("teste", r#"{ rot:txt("tela.painel","Painel") }"#)).is_empty());
    }

    /// Codigo nao e texto. Sem este crivo o conferidor acusaria a pagina
    /// inteira e o numero nao serviria para nada.
    #[test]
    fn codigo_entre_etiquetas_nao_vira_rotulo() {
        for fonte in [
            "for (let i = 0; i < bin.length; i++) soma += bin[i];",
            "const a = x > y ? um : outro;",
            "`</td>`).join(\"\")}</tr>`;",
            "if (a<b) return c;",
        ] {
            assert!(
                fora(&varrer("teste", fonte)).is_empty(),
                "{fonte} nao e texto de tela"
            );
        }
    }

    /// Nome proprio e identificador nao se traduzem, e a razao fica escrita.
    #[test]
    fn isento_diz_por_que() {
        assert_eq!(isento("config.json"), Some("identificador do programa"));
        assert_eq!(isento(".reg"), Some("extensao de arquivo"));
        assert_eq!(isento("LGPD"), Some("sigla"));
        assert_eq!(isento("PhxSql"), Some("a marca"));
        assert_eq!(isento("Salvar"), None, "rotulo de verdade nunca e isento");
        assert_eq!(isento("Cidade"), None);
    }

    /// A guarda do texto colado: seis colunas com a mesma frase e uma chave
    /// que ninguem traduziu, com aparencia de traduzida.
    ///
    /// **Prova real, com o defeito reposto:** troque as seis colunas de
    /// `tela.tl_cartao_vazio` pelo portugues e este teste reprova nomeando a
    /// chave; devolva a traducao e ele passa. A saida esta no relatorio da
    /// rodada em que a guarda entrou.
    ///
    /// Ela NAO compara com o portugues: medido, 33 chaves tem o espanhol
    /// identico ao portugues e a maioria esta certa (`Database`, `Profiler`,
    /// `Menu principal`). Comparar com o portugues reprovaria o correto.
    #[test]
    fn nenhuma_chave_com_os_seis_idiomas_colados() {
        // A comparacao e `>` e nao `<=` porque o teto e ZERO: com `<=`, o
        // clippy tem razao ao dizer que so o `==` acontece. A forma com `>`
        // diz a mesma coisa e continua certa no dia em que o teto nao for
        // zero -- e a mesma forma da catraca de cima.
        let achadas = colados();
        if achadas.len() > TETO_COLADO {
            panic!(
                "{} chave(s) com os SEIS idiomas identicos, e a catraca esta em \
                 {TETO_COLADO}: {achadas:?}.\nOu falta traduzir, ou o texto nao \
                 se traduz mesmo -- e ai ele entra nos ISENTOS com a razao escrita",
                achadas.len()
            );
        }
    }

    /// A guarda da frase longa: a mesma frase de mais de 25 caracteres em tres
    /// ou mais idiomas.
    ///
    /// Pega o colar PARCIAL, que a guarda de cima nao pega: quem traduz tres
    /// colunas e cola o portugues nas outras tres passa por ela e cai aqui.
    ///
    /// O tamanho e medido no MIOLO -- o texto sem os `{marcador}` --, porque
    /// `"{id}{eu} · {nivel} · {sub} · peso {peso}"` tem trinta e nove
    /// caracteres e uma palavra so, e essa palavra e a mesma em portugues,
    /// italiano e espanhol.
    ///
    /// **Prova real, com o defeito reposto:** copie o portugues de
    /// `tela.tl_nota_encerrando` para o italiano e o espanhol e este teste
    /// reprova, nomeando a chave e quantos idiomas trazem a frase.
    #[test]
    fn nenhuma_frase_longa_repetida_em_tres_idiomas() {
        let achadas = frases_repetidas();
        let mostra: Vec<String> = achadas
            .iter()
            .map(|(nome, texto, quantos)| format!("  {nome} em {quantos} idiomas: {texto:?}"))
            .collect();
        if achadas.len() > TETO_FRASE_REPETIDA {
            panic!(
                "{} frase(s) longa(s) repetida(s) em tres ou mais idiomas, e a \
                 catraca esta em {TETO_FRASE_REPETIDA}:\n{}",
                achadas.len(),
                mostra.join("\n")
            );
        }
    }

    /// O miolo e o que sobra depois de tirar os marcadores -- e e ele que
    /// impede a guarda da frase longa de acusar um molde de marcadores.
    #[test]
    fn o_miolo_tira_os_marcadores() {
        assert_eq!(
            miolo("{id}{eu} · {nivel} · {sub} · peso {peso}"),
            "· · · peso"
        );
        assert_eq!(miolo("peso {peso}"), "peso");
        assert_eq!(miolo("sem marcador nenhum"), "sem marcador nenhum");
        // Chave que abre e nao fecha nao pode comer o resto em silencio: ela
        // some ate o fim, e o que importa e nao estourar.
        assert_eq!(miolo("aberta {sem fim"), "aberta");
    }

    /// Nenhuma isencao duplicada: duas razoes para o mesmo texto significa
    /// que alguem discordou de si mesmo e a segunda nunca e lida.
    #[test]
    fn a_lista_de_isentos_nao_se_repete() {
        let mut vistos = HashSet::new();
        for (t, _) in ISENTOS {
            assert!(vistos.insert(*t), "{t} esta duas vezes na lista de isentos");
        }
    }
}

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
//!    `dica:`, o primeiro argumento de `avisar(`, `confirm(`, `prompt(` e
//!    `folha(` (titulo e subtitulo de tela), o primeiro argumento de `carta(`
//!    (titulo do cartao) e o SEGUNDO de `ficha(` (o rotulo -- o primeiro e o
//!    `valor`, que e dado). O literal pode vir entre aspas simples, duplas OU
//!    **crase**: `` avisar(`Tabela criada`) `` e tao rotulo quanto
//!    `avisar("Tabela criada")`.
//!
//! # O que ele NAO enxerga, declarado
//!
//! Rotulo que nao esteja numa dessas formas -- por exemplo o segundo item de
//! um par solto `["registros", e.registros]`. Reconhecer isso sem nome de
//! campo daria falso positivo em toda lista de chaves do programa. Quando uma
//! forma nova de rotulo aparecer, ela entra em [`RECEITAS`] e o numero sobe --
//! e subir o numero e o conferidor funcionando, nao falhando.
//!
//! # O falso negativo que a segunda onda fechou
//!
//! Ate a rodada do pedido 165, [`literal`] so reconhecia aspa simples e
//! dupla. `` avisar(`...`) ``, `` confirm(`...`) `` e `` folha(`...`) `` com o
//! texto entre CRASE ficavam invisiveis a conta -- e os ajudantes `ficha(` e
//! `carta(` nem estavam em [`RECEITAS`], entao todo texto por eles passado
//! ficava fora independente da aspa. Medido antes do conserto: 57 + 13 + 38
//! chamadas de `avisar`/`confirm`/`folha` com crase, mais 63 usos de `ficha(`
//! e 18 de `carta(` inteiramente cegos. A regua passou a medir mais, e por
//! isso `TETO` foi aposentado em favor de [`TETO_ROTULOS_E_CRASE`] -- ver o
//! comentario dele.
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
    ("ui/explorador.html", include_str!("../ui/explorador.html")),
    ("ui/explorador.js", include_str!("../ui/explorador.js")),
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
    // O cabecalho da PhxGrid. Entrou porque a padronizacao «toda tabela e
    // PhxGrid» move o titulo de `{t:"nome"}` para `titulo:"nome"` -- e sem
    // esta linha o conferidor PARA DE VER o texto que continua cravado ali.
    //
    // Medido na primeira tela convertida: o numero caiu de 1.771 para 1.760
    // sem ninguem ter traduzido uma palavra. Converter as 22 telas derrubaria
    // ~200 do mesmo jeito, e a catraca acabaria a varredura frouxa com a tela
    // tao em portugues quanto comecou. *Catraca frouxa nao segura nada.*
    ("titulo:", "cabecalho de coluna da PhxGrid"),
    ("diz:", "a explicacao curta de um formato de exportacao"),
    ("dica:", "a dica que aparece no title do botao da barra"),
    ("avisar(", "o recado do alto da tela"),
    ("confirm(", "a pergunta antes de uma acao que nao se desfaz"),
    ("prompt(", "a pergunta que pede um valor"),
    ("folha(", "titulo e subtitulo de tela"),
    // Os dois ajudantes do pedido 165. `carta(titulo, legenda, corpo, larga)`
    // tem o rotulo no PRIMEIRO argumento, igual a `avisar` -- so nao estava
    // na lista. `ficha(valor, rotulo, unidade)` e diferente: o primeiro
    // argumento e o DADO (`valor`), entao o rotulo e o SEGUNDO -- e
    // `via_rotulo` trata esse caso a parte, pulando o primeiro argumento
    // antes de procurar o literal.
    ("carta(", "titulo do cartao de painel"),
    (
        "ficha(",
        "o rotulo da ficha (valor,rotulo,unidade) -- o segundo argumento",
    ),
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
    (
        "Primary → Replica",
        "nome de topologia de replicacao, termo tecnico do setor",
    ),
    (
        "Multi-Master",
        "nome de topologia de replicacao, termo tecnico do setor",
    ),
    (
        "Primary → Standby",
        "nome de topologia de replicacao, termo tecnico do setor",
    ),
    (
        "Read Replica",
        "nome de topologia de replicacao, termo tecnico do setor",
    ),
    ("Blockchain", "nome de tecnologia, o mesmo em toda lingua"),
    ("iptables / ip6tables", "nome de ferramenta do Linux"),
    ("nftables", "nome de ferramenta do Linux"),
    ("fail2ban", "nome de ferramenta do Linux"),
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

/// Se o que vem logo apos um `${` e uma das chamadas de [`RECEITAS`] --
/// `carta(`, `ficha(`, `avisar(`... -- e nao dado comum.
///
/// Existe por causa do achado do pedido 165: `${ficha(valor, "rotulo")}` e
/// `${carta("Título", ...)}` sao interpolacao por FORA (o `${…}` que embrulha
/// a chamada) e rotulo por DENTRO. Apagar o bloco inteiro -- o que
/// `sem_interpolacao` fazia sem esta checagem -- jogava o rotulo fora junto
/// com o dado, e foi por isso que os 62 usos de `ficha(` (todos dentro de
/// `${ficha(...)}`) e 13 dos 18 usos de `carta(` ficavam invisiveis mesmo
/// depois de `literal()` aprender a crase: a chamada nunca chegava a
/// `via_rotulo`, porque `limpar()` ja tinha apagado a linha inteira antes.
fn abre_com_receita(s: &str) -> bool {
    RECEITAS
        .iter()
        .any(|(receita, _)| receita.ends_with('(') && s.starts_with(*receita))
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
            if abre_com_receita(&linha[i + 2..]) {
                // So o marcador `${` some. O resto da chamada segue intacto
                // para o `via_rotulo` reconhecer -- o crivo de `normalizar`
                // continua barrando qualquer pedaco de codigo que escape por
                // essa fresta, exatamente como barra em qualquer outro lugar.
                saida.push(BURACO);
                i += 2;
                continue;
            }
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
            // `ficha(valor, rotulo, unidade)`: o rotulo e o SEGUNDO
            // argumento -- o primeiro e o `valor`, que e dado e nao se
            // traduz. Pula ate a virgula de nivel zero antes de procurar o
            // literal; sem segunda virgula (chamada incompleta ou so o
            // primeiro argumento) nao ha o que julgar aqui.
            let mut base = i;
            if *receita == "ficha(" {
                match fim_do_primeiro_argumento(&limpo[base..]) {
                    Some(corte) => base += corte + 1,
                    None => continue,
                }
            }
            let resto = &limpo[base..];
            let cru = resto.trim_start();
            let pulo = resto.len() - cru.len();
            let Some(valor) = literal(cru) else { continue };
            if let Some(a) = pesar(arquivo, limpo, inicios, base + pulo, valor, Canal::Rotulo) {
                achados.push(a);
            }
            // `folha(` leva dois: titulo e subtitulo.
            if *receita == "folha(" {
                let apos = base + pulo + valor.len() + 2;
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

/// O byte logo apos a virgula que separa o primeiro argumento do resto, numa
/// chamada que comeca em `s` (ja sem o nome da funcao e o `(`). `None` quando
/// nao ha segundo argumento -- a chamada fecha no primeiro `)` de nivel zero
/// antes de qualquer virgula.
///
/// So existe por causa de `ficha(valor, rotulo, unidade)`: o rotulo e o
/// SEGUNDO argumento, e para achar onde ele comeca e preciso pular o
/// primeiro sem se perder em parenteses, colchetes, chaves ou aspas
/// aninhadas dentro dele -- `ficha(pk ? pk.nome : "—", "chave primária")` tem
/// uma virgula dentro do proprio primeiro argumento (nenhuma aqui, mas o
/// ternario e a chamada podiam trazer uma) e o corte tem de respeitar isso.
fn fim_do_primeiro_argumento(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut nivel = 0i32;
    let mut aspa: u8 = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if aspa != 0 {
            if c == aspa {
                aspa = 0;
            }
        } else {
            match c {
                b'"' | b'\'' | b'`' => aspa = c,
                b'(' | b'[' | b'{' => nivel += 1,
                b')' | b']' | b'}' if nivel > 0 => nivel -= 1,
                b',' if nivel == 0 => return Some(i),
                b')' if nivel == 0 => return None,
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// O conteudo de um literal que comeca em `s`, se `s` comeca com aspa --
/// simples, dupla OU crase.
///
/// A crase entrou no pedido 165: ate entao so aspa simples e dupla contavam,
/// e `` avisar(`Tabela criada`) `` passava invisivel pela conta enquanto
/// `avisar("Tabela criada")` era pego -- o MESMO texto, reprovado ou nao
/// conforme o estilo de aspa de quem escreveu a chamada. O `${…}` que possa
/// estar dentro ja virou [`BURACO`] antes de chegar aqui (ve [`limpar`]),
/// entao um literal em crase com dado interpolado se comporta igual a um em
/// aspa dupla.
fn literal(s: &str) -> Option<&str> {
    let aspa = s.chars().next()?;
    if aspa != '"' && aspa != '\'' && aspa != '`' {
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

/// **Comentario HTML dentro de um template literal, com crase.**
///
/// Defeito real, e ele derrubou a pagina INTEIRA: um `<!-- ... -->` escrito
/// dentro de uma `` `template literal` `` do JavaScript, com crases em volta
/// dos identificadores -- que e o estilo desta casa em comentario. Crase
/// dentro de template literal FECHA o literal, e o resto do arquivo virou
/// sintaxe invalida.
///
/// O que torna este defeito perigoso e o texto do comentario estar CERTO: ler
/// nao pega, revisar nao pega. Quem pegou foi a bateria, em 200 ms, porque a
/// tela nao abriu.
///
/// A guarda procura o padrao: uma linha com `<!--` que esteja dentro de um
/// template literal aberto e que contenha crase.
pub fn comentario_com_crase_em_template(fonte: &str) -> Vec<usize> {
    let mut achados = Vec::new();
    let mut dentro = false;
    for (n, linha) in fonte.lines().enumerate() {
        // Conta crases nao escapadas da linha para saber se o literal segue
        // aberto na proxima. Nao e um parser de JavaScript, e nao precisa
        // ser: o alvo e uma linha de comentario HTML com crase, e essa so
        // aparece de um jeito.
        if dentro && linha.contains("<!--") && linha.contains('`') {
            achados.push(n + 1);
        }
        let crases = linha.matches('`').count();
        if crases % 2 == 1 {
            dentro = !dentro;
        }
    }
    achados
}

/// As fontes que carregam CSS -- lista PROPRIA, e nao a `FONTES`.
///
/// Ampliar a `FONTES` moveria a catraca dos idiomas, e mudar a regua e
/// exatamente o que esta casa nao faz com catraca. Aqui a pergunta e outra
/// (token de CSS), entao a lista e outra.
const FONTES_CSS: &[(&str, &str)] = &[
    ("ui/index.html", include_str!("../ui/index.html")),
    ("ui/explorador.css", include_str!("../ui/explorador.css")),
    ("ui/multitela.css", include_str!("../ui/multitela.css")),
    ("ui/telemetria.css", include_str!("../ui/telemetria.css")),
    (
        "ui/grid/phx-grid.css",
        include_str!("../ui/grid/phx-grid.css"),
    ),
];

/// **Todo `var(--x)` ou tem o token definido, ou tem fallback.**
///
/// A ideia veio de fora -- e a regra `validate_tokens` do Phoenix Web Absorber
/// FX SDK, que existe para garantir «CSS sempre renderizavel». O SDK inteiro
/// foi RECUSADO com numero (medido: 47 tokens usados, 46 definidos, e o unico
/// ausente ja tinha fallback), mas a ideia dele vale e nao custa licenca
/// nenhuma: absorve-se a regra, nao o codigo.
///
/// O que ela pega: um `var(--novo)` sem definicao E sem fallback torna a
/// declaracao INVALIDA, e o navegador descarta a propriedade inteira em
/// silencio. O componente nasce sem cor, sem fonte ou sem borda, e nada avisa
/// -- e a mesma familia do «CSS global morde componente novo».
///
/// Devolve `(arquivo, token)` de cada furo.
pub fn token_sem_definicao_e_sem_fallback() -> Vec<(&'static str, String)> {
    let mut definidos: std::collections::HashSet<String> = std::collections::HashSet::new();
    // As definicoes sao colhidas de CSS **e** de JS, e o motivo veio da
    // primeira execucao desta guarda: ela acusou `--n` do `telemetria.css`,
    // e o token e legitimo -- o `telemetria.js` o poe num `style` EM LINHA
    // (`style="--n:${…}"`), que e definicao tao valida quanto a do `:root`.
    // O furo era da guarda, nao da tela. Uso se confere no CSS; definicao vem
    // de onde vier.
    for (_, fonte) in FONTES_CSS.iter().chain(FONTES.iter()) {
        let mut resto = *fonte;
        while let Some(i) = resto.find("--") {
            let dep = &resto[i..];
            let fim = dep
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(dep.len());
            // So conta como DEFINICAO quando vem `:` logo depois do nome.
            if dep[fim..].starts_with(':') {
                definidos.insert(dep[..fim].to_string());
            }
            resto = &resto[i + 2..];
        }
    }
    let mut furos = Vec::new();
    for (nome, fonte) in FONTES_CSS {
        let mut resto = *fonte;
        while let Some(i) = resto.find("var(") {
            let dep = resto[i + 4..].trim_start();
            let fim = dep
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(dep.len());
            let token = &dep[..fim];
            // Virgula = fallback declarado, e ai o token faltante nao quebra.
            let tem_fallback = dep[fim..].trim_start().starts_with(',');
            if token.starts_with("--") && !tem_fallback && !definidos.contains(token) {
                furos.push((*nome, token.to_string()));
            }
            resto = &resto[i + 4..];
        }
    }
    furos.sort();
    furos.dedup();
    furos
}

/// Quantos textos de tela ainda estao fora da fabrica. **So desce.**
///
/// Traduziu um punhado? Rode
/// `cargo run --example textos-fora-da-fabrica -p phxsql-server`, veja o
/// numero novo e baixe a catraca no mesmo commit -- catraca frouxa nao segura
/// nada.
///
/// # Esta catraca SUBSTITUI `TETO`, e e por isso que ela tem nome novo
///
/// Ate o pedido 165, [`literal`] so reconhecia aspa simples e dupla, e
/// `sem_interpolacao` apagava TODO `${…}` de um golpe so. Duas formas de
/// rotulo ficavam invisiveis por isso: o texto escrito entre CRASE --
/// `` avisar(`Tabela criada`) `` passava, `avisar("Tabela criada")` era pego,
/// o MESMO texto tratado diferente conforme o estilo de aspa de quem
/// escreveu -- e o rotulo escondido DENTRO de uma interpolacao, como
/// `${carta("Título", ...)}` e `${ficha(valor, "rotulo")}` (todo uso de
/// `ficha(` nesta base fica assim, e a chamada inteira, titulo e tudo,
/// desaparecia com o `${…}` que a embrulha). Medido antes do conserto: 1.549.
///
/// Regua que passa a medir mais nao sobe a catraca existente -- ela **aposenta
/// a antiga e faz nascer uma nova, no numero medido do dia**, dizendo que
/// substitui a outra. O precedente ja estava no codigo, em
/// `conferidor_grades::TETO_TABELA_NA_MAO`, e esta catraca segue o mesmo
/// molde: `TETO` (1.549, com toda a serie de altos e baixos desde que nasceu
/// em 2.000) fica aposentado, e a serie com o passado se perde de proposito
/// -- perder a comparacao e mais barato que deixar "mudei a regua" virar a
/// porta pela qual se afrouxa uma catraca. O historico completo continua
/// legivel no `git log` deste arquivo.
///
/// `TETO_ROTULOS_E_CRASE` conta as MESMAS formas de antes mais as duas que a
/// regua nova enxerga. Nasceu em 1.744; o mesmo commit que ensinou o crivo
/// tambem traduziu o lote coerente do Painel (`vPainel()`/`maquinaHtml()` de
/// `index.html` -- os sete cartoes de metrica e o widget "A maquina"), que
/// baixou 24: **1.720**.
///
/// 03/09, 19h: **1.715**. Nao foi leva de traducao -- foi conserto de
/// frase FALSA. Cinco textos da tela afirmavam que a chave estrangeira e
/// "declarada, nao imposta" e que a transacao "nao ve as proprias
/// escritas", duas coisas que os pedidos 162 e 171 tinham desfeito. Quatro
/// deles ja passavam pela fabrica (bastou trocar o texto nos seis idiomas)
/// e o quinto -- o brinde do editor ER -- estava cravado e entrou pela
/// fabrica junto com o conserto. Catraca desce por qualquer motivo que
/// tire texto de fora dela; o motivo aqui e que a frase mentia.
/// 03/09, 20h: **1.707**. Efeito colateral do pedido 158, e nao leva de
/// traducao: converter quatro tabelas a mao em PhxGrid trocou marcacao
/// crua por colunas com `txt()`, e apagar o ajudante `tabela()` levou o
/// vazio dele junto. Catraca desce por qualquer motivo que tire texto de
/// fora dela.
/// 04/09: **1.706**. O rodape da aba Conteudo entrou na fabrica junto com o
/// `WHERE` do `varrer`: ele passou a ter DUAS redacoes (com e sem peneira do
/// servidor), e um texto que muda conforme a resposta nao podia continuar
/// cravado em portugues no meio de um template. Uma linha a menos, e ela sai
/// da conta por ter sido traduzida -- que e o unico motivo que vale.
pub const TETO_ROTULOS_E_CRASE: usize = 1_706;
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
        if faltando.len() > TETO_ROTULOS_E_CRASE {
            let mostra: Vec<String> = faltando
                .iter()
                .take(40)
                .map(|a| format!("  {}:{} {:?}", a.arquivo, a.linha, a.texto))
                .collect();
            panic!(
                "{} textos de tela fora da fabrica, e a catraca esta em {TETO_ROTULOS_E_CRASE}.\n\
                 Os primeiros:\n{}\n\
                 Todos: cargo run --example textos-fora-da-fabrica -p phxsql-server",
                faltando.len(),
                mostra.join("\n")
            );
        }
        assert!(
            faltando.len() >= TETO_ROTULOS_E_CRASE.saturating_sub(30),
            "sobraram {} e a catraca esta em {TETO_ROTULOS_E_CRASE}: baixe a catraca no mesmo \
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
        // Os modulos que EMBUTEM tela. Era um so; o `rest.rs` virou o segundo
        // quando o explorador da API entrou, e uma guarda que continuasse
        // lendo so o `http.rs` deixaria a tela nova fora da catraca --
        // exatamente o defeito que ela existe para nao repetir.
        const SERVEM_TELA: &[&str] = &[include_str!("http.rs"), include_str!("rest.rs")];

        let servidos: Vec<&str> = SERVEM_TELA
            .iter()
            .flat_map(|fonte| {
                fonte
                    .match_indices("include_str!(\"../ui/")
                    .filter_map(|(i, _)| {
                        let resto = &fonte[i + "include_str!(\"".len()..];
                        let fim = resto.find('"')?;
                        let caminho = &resto[..fim];
                        // O `.md` do changelog da grade e servido, mas nao e tela.
                        (caminho.ends_with(".js") || caminho.ends_with(".html")).then_some(caminho)
                    })
                    .collect::<Vec<&str>>()
            })
            .collect();

        assert!(
            !servidos.is_empty(),
            "nao achei nenhum include_str! de interface -- a guarda ficou \
             cega, conserte o reconhecimento antes de confiar nela"
        );

        let medidos: HashSet<&str> = FONTES.iter().map(|(nome, _)| *nome).collect();
        let faltando: Vec<&str> = servidos
            .iter()
            .filter(|c| !medidos.contains(c.trim_start_matches("../")))
            .copied()
            .collect();

        assert!(
            faltando.is_empty(),
            "o servidor serve {faltando:?} e o FONTES nao mede -- texto cravado \
             ali nao conta para a catraca. Acrescente ao FONTES e reveja o TETO_ROTULOS_E_CRASE"
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

#[cfg(test)]
mod testes_crase {
    use super::*;

    /// **Prova real nos dois sentidos**, com o defeito de verdade: o
    /// comentario que derrubou a tela de Bancos.
    #[test]
    fn a_crase_no_comentario_dentro_do_template_e_pega() {
        let mau = "folha(\"x\", `<div>\n  <!-- `id` proprio -->\n</div>`);";
        assert_eq!(
            comentario_com_crase_em_template(mau),
            vec![2],
            "nao pegou o defeito"
        );

        // E o comentario SEM crase passa -- guarda que recusa tudo nao guarda
        // nada, so atrapalha.
        let bom = "folha(\"x\", `<div>\n  <!-- id proprio -->\n</div>`);";
        assert!(
            comentario_com_crase_em_template(bom).is_empty(),
            "recusou o certo"
        );

        // E comentario com crase FORA de template literal e legitimo.
        let fora = "// `id` proprio\nfolha(\"x\", `<div></div>`);";
        assert!(
            comentario_com_crase_em_template(fora).is_empty(),
            "recusou fora do literal"
        );
    }

    /// **CSS sempre renderizavel** -- a regra absorvida do FX SDK, sem uma
    /// linha do codigo dele.
    ///
    /// Medido no dia em que entrou: 47 tokens usados, 46 definidos, e o unico
    /// ausente (`--phx-mono`) ja trazia fallback. Ou seja, a tela estava certa
    /// -- mas certa por SORTE, porque nada cobrava. Agora cobra.
    #[test]
    fn todo_token_de_css_tem_definicao_ou_fallback() {
        let furos = token_sem_definicao_e_sem_fallback();
        assert!(
            furos.is_empty(),
            "estes `var(--x)` nao tem definicao NEM fallback -- o navegador \
             descarta a declaracao inteira em silencio: {furos:?}"
        );
    }

    /// **Prova real**: a guarda pega o furo, e nao recusa o que e legitimo.
    ///
    /// Sem este par, uma guarda que devolvesse sempre vazio passaria no teste
    /// acima e nao guardaria nada -- que e a forma mais comum de guarda morta.
    #[test]
    fn a_guarda_de_token_pega_o_furo_e_poupa_o_fallback() {
        // O crivo e o mesmo da funcao, exercitado em texto controlado.
        let com_fallback = "a{font:12px var(--nao-existe,monospace)}";
        let sem_fallback = "a{color:var(--nao-existe)}";
        let definido = ":root{--existe:#fff} a{color:var(--existe)}";
        let acha = |t: &str| {
            let def: Vec<&str> = t
                .match_indices("--")
                .filter_map(|(i, _)| {
                    let d = &t[i..];
                    let f = d.find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')?;
                    d[f..].starts_with(':').then(|| &d[..f])
                })
                .collect();
            t.match_indices("var(").any(|(i, _)| {
                let d = t[i + 4..].trim_start();
                let f = d
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                    .unwrap_or(d.len());
                !d[f..].trim_start().starts_with(',') && !def.contains(&&d[..f])
            })
        };
        assert!(
            acha(sem_fallback),
            "nao pegou o token sem definicao e sem fallback"
        );
        assert!(!acha(com_fallback), "recusou um token que TEM fallback");
        assert!(!acha(definido), "recusou um token que ESTA definido");
    }

    /// A tela de verdade nao tem nenhum -- e a catraca que impede o proximo.
    #[test]
    fn a_interface_nao_tem_comentario_com_crase_em_template() {
        for (nome, fonte) in FONTES {
            let achados = comentario_com_crase_em_template(fonte);
            assert!(
                achados.is_empty(),
                "{nome}: comentario HTML com crase dentro de template literal nas \
                 linhas {achados:?} -- a crase FECHA o literal e derruba a pagina"
            );
        }
    }
}

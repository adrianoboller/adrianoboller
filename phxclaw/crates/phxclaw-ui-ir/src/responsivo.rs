//! Motor responsivo (Phx Responsive UI): compila a INTENCAO de layout do UI-IR v3 em CSS
//! nativo. E o mesmo para todos os adaptadores -- o `html` (adaptador «phoenix») e o
//! `bootstrap` desenham componentes; quantas colunas uma secao tem em cada largura sai
//! SO daqui. Se cada adaptador decidisse a propria grade, o mesmo IR daria telas diferentes
//! conforme o framework, e a prova de fidelidade mediria o adaptador em vez do IR.
//!
//! As escolhas:
//! - **Grid na estrutura, Flexbox nas barras.** A secao e bidimensional (filas e colunas de
//!   campos); botoes, totais e o par controle+botao sao uma linha que quebra.
//! - **Container query no componente, media query so na composicao.** A secao responde a
//!   largura do painel que a recebeu (`@container`): um painel de 360 px numa janela de
//!   1920 fica compacto. So menu+conteudo olha a janela (`@media`).
//! - **`min-inline-size:0` nos filhos** de grade e fila: o minimo automatico de um item
//!   de grade e o conteudo dele, e um `<select>` comprido empurraria a pagina para o lado.
//! - **Tabela com rolagem PROPRIA** (a excecao bidimensional do criterio 1.4.10 da WCAG),
//!   e em conteiner estreito vira cartoes com o rotulo em cada celula. Nunca
//!   `overflow:hidden` na pagina: esconder o que nao cabe e cortar dado.
//! - **Pontos de quebra num JSON so** (`breakpoints.json`, os do Bootstrap 5.3). O numero
//!   escrito em outro lugar divergiria na primeira mudanca; o teste
//!   `numeros_de_quebra_so_no_json` reprova.

use crate::ir::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

const BREAKPOINTS_JSON: &str = include_str!("../breakpoints.json");

/// Os pontos de quebra, em ordem crescente: (nome, px).
pub fn breakpoints() -> &'static [(String, u32)] {
    static B: OnceLock<Vec<(String, u32)>> = OnceLock::new();
    B.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(BREAKPOINTS_JSON).expect("breakpoints.json invalido");
        let mut b: Vec<(String, u32)> = v
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, _)| !k.starts_with('_'))
            .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n as u32)))
            .collect();
        b.sort_by_key(|(_, n)| *n);
        b
    })
}

/// Um ponto de quebra pelo nome (`sm`, `md`, `lg`, `xl`, `xxl`).
pub fn bp(nome: &str) -> u32 {
    breakpoints()
        .iter()
        .find(|(k, _)| k == nome)
        .map(|(_, n)| *n)
        .unwrap_or_else(|| panic!("ponto de quebra {nome} ausente de breakpoints.json"))
}

/// Tokens de espaco aceitos no `gap`. O valor de cada um e do tema (`--phx-gap-*`).
pub const GAPS: &[&str] = &["xs", "sm", "md", "lg", "xl"];

/// Conteiner da area de conteudo (o `<main>`, sem respiro: o respiro fica na tela, dentro
/// dele, para que a largura consultada seja a do painel inteiro). A regra de uma secao pode
/// consultar a largura dele.
pub const CONTEINER_TELA: &str = "tela";
/// Conteiner padrao de cada secao.
pub const CONTEINER_SECAO: &str = "secao";
/// Conteiner da grade de dados (a tabela que vira cartoes).
pub const CONTEINER_TABELA: &str = "tabela";

/// A intencao padrao de uma secao: uma coluna no celular, e mais colunas conforme o PAINEL
/// que recebeu a tela cresce (sm 2, md 3, lg 4) -- o alvo e o conteiner `tela`, a area de
/// conteudo, e nao a janela: a mesma tela num painel de 360 px de uma janela de 1920 fica
/// em uma coluna. Coluna minima de ~240 px: o rotulo mais longo do gabarito («Preço
/// unitário») e um campo de data cabem sem quebrar.
///
/// Por que `tela` e nao a propria secao: medido em 01/10, a 1280 px a secao tem a largura da
/// tela menos borda e respiro, e cai logo abaixo de `lg`; a mesma tela dava 3 colunas, o
/// OCR da prova de fidelidade leu pior (revocacao minima 0,67 contra 0,75 da linha de
/// base). A area de conteudo e a medida que quem desenha a tela tem na cabeca.
pub fn intencao_padrao() -> (LayoutIntencao, String) {
    let regra = |nome: &str, colunas: u32| RegraResponsiva {
        min_largura_px: bp(nome),
        colunas,
    };
    (
        LayoutIntencao {
            tipo: TipoLayout::Grade,
            gap: "md".into(),
            colunas: 1,
            responsivo: Some(Responsivo {
                base: BaseResponsiva::Conteiner,
                alvo: Some(CONTEINER_TELA.into()),
                regras: vec![regra("sm", 2), regra("md", 3), regra("lg", 4)],
            }),
        },
        CONTEINER_SECAO.into(),
    )
}

/// A intencao efetiva da secao: a declarada, ou a padrao.
pub fn intencao(s: &Section) -> (LayoutIntencao, String) {
    let (l, c) = intencao_padrao();
    (
        s.layout.clone().unwrap_or(l),
        s.conteiner.clone().unwrap_or(c),
    )
}

/// As secoes que uma tela desenha (a consulta nao tem secao).
pub fn secoes_da_tela(t: &Screen) -> Vec<&Section> {
    match t {
        Screen::Form { sections, .. } => sections.iter().collect(),
        Screen::MasterDetail { header, .. } => header.iter().collect(),
        Screen::List { .. } | Screen::Painel { .. } => vec![],
    }
}

/// Campo que ocupa a fila inteira: texto longo nao cabe numa coluna de 250 px.
pub fn campo_largo(f: &Field) -> bool {
    matches!(f.widget, Widget::TextArea) || f.max_len.unwrap_or(0) > 80
}

/// Colunas que a intencao da numa largura: `conteiner` e a do painel, `janela` a da janela.
/// E a conta que a prova no navegador confere contra o CSS compilado.
pub fn colunas_em(l: &LayoutIntencao, conteiner_px: f64, janela_px: f64) -> u32 {
    let Some(r) = &l.responsivo else {
        return l.colunas;
    };
    let w = match r.base {
        BaseResponsiva::Conteiner => conteiner_px,
        BaseResponsiva::Janela => janela_px,
    };
    // a ultima regra alcancada manda (regras em ordem crescente, conferida por `validar`)
    r.regras
        .iter()
        .rev()
        .find(|g| w >= g.min_largura_px as f64)
        .map_or(l.colunas, |g| g.colunas)
}

fn nome_valido(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some('a'..='z'))
        && n.len() <= 32
        && c.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Cada intencao que a tela desenha, com o conteiner que a abriga e onde ela esta (para a
/// mensagem): as secoes de cadastro e documento, e a colecao de um painel.
pub fn intencoes_da_tela(t: &Screen) -> Vec<(LayoutIntencao, String, String)> {
    match t {
        Screen::Painel {
            conteiner, colecao, ..
        } => vec![(
            colecao.layout.clone(),
            conteiner.clone(),
            format!("tela {}, colecao {}", t.id(), colecao.id),
        )],
        _ => secoes_da_tela(t)
            .into_iter()
            .map(|s| {
                let (l, c) = intencao(s);
                (l, c, format!("tela {}, secao «{}»", t.id(), s.title))
            })
            .collect(),
    }
}

/// Tokens que o app pode sobrepor, e a forma do valor de cada um.
pub const TOKENS: &[&str] = &[
    "gap-xs", "gap-sm", "gap-md", "gap-lg", "gap-xl", "acento", "raio",
];

fn token_valido(nome: &str, v: &str) -> bool {
    let px = |x: &str| {
        x.strip_suffix("px")
            .is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()))
    };
    match nome {
        "acento" => {
            v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
        }
        "raio" => px(v),
        _ => {
            let p: Vec<&str> = v.split(' ').collect();
            (1..=2).contains(&p.len()) && p.iter().all(|x| px(x))
        }
    }
}

/// Confere toda intencao do app. Nome de conteiner e token vao para DENTRO do CSS gerado:
/// aceitar texto livre ali seria deixar o IR escrever folha de estilo.
pub fn validar(app: &App) -> Result<(), String> {
    if let Some(q) = app.quebra_da_casca_px
        && !(320..=4000).contains(&q)
    {
        return Err(format!("quebra_da_casca_px {q} fora de 320..4000"));
    }
    for (k, v) in &app.tokens {
        if !TOKENS.contains(&k.as_str()) || !token_valido(k, v) {
            return Err(format!(
                "token «{k}» = «{v}» recusado (aceitos: {}; px de ate 3 digitos ou #rrggbb)",
                TOKENS.join(", ")
            ));
        }
    }
    for t in &app.screens {
        for (l, c, onde) in intencoes_da_tela(t) {
            if !nome_valido(&c) || c == CONTEINER_TELA || c == CONTEINER_TABELA {
                return Err(format!(
                    "{onde}: conteiner «{c}» invalido (a-z, 0-9, - e _, ate 32; tela e tabela sao reservados)"
                ));
            }
            if !GAPS.contains(&l.gap.as_str()) {
                return Err(format!(
                    "{onde}: gap «{}» nao e token ({})",
                    l.gap,
                    GAPS.join(", ")
                ));
            }
            if !(1..=12).contains(&l.colunas) {
                return Err(format!("{onde}: colunas {} fora de 1..12", l.colunas));
            }
            let Some(r) = &l.responsivo else { continue };
            if l.tipo != TipoLayout::Grade && !r.regras.is_empty() {
                return Err(format!(
                    "{onde}: regra de colunas so vale para tipo grade (fila e pilha nao tem coluna)"
                ));
            }
            if r.base == BaseResponsiva::Conteiner {
                match r.alvo.as_deref() {
                    Some(a) if a == c || a == CONTEINER_TELA => {}
                    outro => {
                        return Err(format!(
                            "{onde}: alvo {outro:?} nao e conteiner ancestral (use «{c}» ou «{CONTEINER_TELA}»)"
                        ));
                    }
                }
            }
            let mut ant = 0;
            for g in &r.regras {
                if g.min_largura_px <= ant || g.min_largura_px > 10_000 {
                    return Err(format!(
                        "{onde}: regras em ordem crescente de min_largura_px, de 1 a 10000"
                    ));
                }
                if !(1..=12).contains(&g.colunas) {
                    return Err(format!("{onde}: colunas {} fora de 1..12", g.colunas));
                }
                ant = g.min_largura_px;
            }
        }
    }
    Ok(())
}

/// Os tokens que o app sobrepoe, como propriedades `--phx-*`. Vem depois dos tokens da casa
/// nos dois adaptadores, e so com valor conferido por `validar`.
///
/// O `acento` vale so no tema escuro: o do exemplo do dono (#fa9866) tem contraste de
/// texto com o papel claro abaixo de 3:1, e o proprio exemplo troca o acento no tema claro.
/// No claro fica o da casa, que ja passa de 4,5:1.
pub fn tokens_do_app(app: &App) -> String {
    let conferidos = || {
        app.tokens
            .iter()
            .filter(|(k, v)| TOKENS.contains(&k.as_str()) && token_valido(k, v))
    };
    let v: Vec<String> = conferidos()
        .filter(|(k, _)| k.as_str() != "acento")
        .map(|(k, v)| format!("--phx-{k}:{v}"))
        .collect();
    let mut s = String::new();
    if !v.is_empty() {
        s.push_str(&format!(":root{{{}}}", v.join(";")));
    }
    if let Some((_, a)) = conferidos().find(|(k, _)| k.as_str() == "acento") {
        s.push_str(&format!(
            "@media (prefers-color-scheme: dark){{:root{{--phx-acento:{a}}}}}"
        ));
    }
    s
}

fn tipo_nome(t: TipoLayout) -> &'static str {
    match t {
        TipoLayout::Grade => "grade",
        TipoLayout::Fila => "fila",
        TipoLayout::Pilha => "pilha",
    }
}

/// Classe deterministica da intencao: a mesma intencao da a mesma classe em qualquer
/// adaptador, e a folha leva uma regra por intencao distinta, nao por secao.
pub fn classe(l: &LayoutIntencao) -> String {
    let mut s = format!("phx-{}-{}-{}", tipo_nome(l.tipo), l.gap, l.colunas);
    if let Some(r) = &l.responsivo {
        for g in &r.regras {
            match r.base {
                BaseResponsiva::Janela => {
                    s.push_str(&format!("-j{}x{}", g.min_largura_px, g.colunas))
                }
                BaseResponsiva::Conteiner => s.push_str(&format!(
                    "-c{}{}x{}",
                    r.alvo.as_deref().unwrap_or(""),
                    g.min_largura_px,
                    g.colunas
                )),
            }
        }
    }
    s
}

/// Classes da secao: (a do conteiner que ela abre, a do layout dos campos).
pub fn classes_da_secao(s: &Section) -> (String, String) {
    let (l, c) = intencao(s);
    (format!("phx-c-{c}"), classe(&l))
}

fn regra_css(l: &LayoutIntencao) -> String {
    let c = classe(l);
    let gap = format!("var(--phx-gap-{})", l.gap);
    let cols = |n: u32| format!("repeat({n},minmax(0,1fr))");
    match l.tipo {
        TipoLayout::Fila => format!(
            ".{c}{{display:flex;flex-wrap:wrap;align-items:flex-end;gap:{gap}}}.{c}>*{{min-inline-size:0;flex:1 1 12rem}}"
        ),
        TipoLayout::Pilha => format!(
            ".{c}{{display:flex;flex-direction:column;gap:{gap}}}.{c}>*{{min-inline-size:0}}"
        ),
        TipoLayout::Grade => {
            let mut s = format!(
                ".{c}{{display:grid;gap:{gap};grid-template-columns:{}}}.{c}>*{{min-inline-size:0}}",
                cols(l.colunas)
            );
            if let Some(r) = &l.responsivo {
                for g in &r.regras {
                    let corpo = format!(".{c}{{grid-template-columns:{}}}", cols(g.colunas));
                    match r.base {
                        BaseResponsiva::Janela => s.push_str(&format!(
                            "@media (min-width:{}px){{{corpo}}}",
                            g.min_largura_px
                        )),
                        BaseResponsiva::Conteiner => s.push_str(&format!(
                            "@container {} (min-width:{}px){{{corpo}}}",
                            r.alvo.as_deref().unwrap_or(CONTEINER_SECAO),
                            g.min_largura_px
                        )),
                    }
                }
            }
            s
        }
    }
}

/// A folha de layout do app: composicao, barras, grade de dados e uma regra por intencao
/// distinta. So estrutura -- cor, borda e tipografia sao do adaptador.
pub fn css(app: &App) -> String {
    // a unica regra de JANELA: menu e conteudo lado a lado a partir da quebra da casca
    let md = app.quebra_da_casca_px.unwrap_or_else(|| bp("md"));
    // `max-width` com 0,02 px a menos que o ponto, como o Bootstrap: o `min-width` do ponto
    // e o `max-width` logo abaixo nunca valem juntos nem deixam buraco em zoom fracionario
    let abaixo = |n: u32| format!("{}.98", n - 1);
    let mut s = format!(
        "\
.phx-app{{display:grid;min-block-size:100vh;grid-template-columns:minmax(0,1fr);grid-template-areas:\"menu\" \"principal\"}}\
.phx-app>nav{{grid-area:menu;min-inline-size:0}}.phx-app>main{{grid-area:principal;min-inline-size:0}}\
@media (min-width:{md}px){{.phx-app{{grid-template-columns:minmax(12rem,15rem) minmax(0,1fr);grid-template-areas:\"menu principal\"}}}}\
.phx-menu-lista{{display:flex;flex-wrap:wrap;gap:var(--phx-gap-xs) var(--phx-gap-sm);list-style:none;margin:0;padding:0}}\
@media (min-width:{md}px){{.phx-menu-lista{{flex-direction:column;flex-wrap:nowrap}}}}\
.phx-c-{CONTEINER_TELA}{{container:{CONTEINER_TELA}/inline-size;min-inline-size:0}}.phx-tela{{min-inline-size:0}}\
.phx-campo{{display:flex;flex-direction:column;gap:var(--phx-gap-xs);min-inline-size:0}}.phx-largo{{grid-column:1/-1}}\
.phx-fila{{display:flex;flex-wrap:wrap;align-items:center;gap:var(--phx-gap-sm)}}.phx-fila>*{{min-inline-size:0}}\
.phx-fila.phx-fim{{justify-content:flex-end}}\
.phx-junto{{display:flex;align-items:stretch;gap:var(--phx-gap-xs);min-inline-size:0}}\
.phx-junto>input,.phx-junto>select{{flex:1 1 auto;min-inline-size:0}}\
.phx-tabela{{container:{CONTEINER_TABELA}/inline-size;overflow-x:auto;min-inline-size:0}}\
@container {CONTEINER_TABELA} (max-width:{sm}px){{\
.phx-tabela table,.phx-tabela tbody,.phx-tabela tr{{display:block;inline-size:100%}}\
.phx-tabela thead{{position:absolute;inline-size:1px;block-size:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}}\
.phx-tabela td{{display:grid;grid-template-columns:minmax(0,2fr) minmax(0,3fr);gap:var(--phx-gap-sm);align-items:center}}\
.phx-tabela td:not([data-rotulo]){{display:block}}.phx-tabela td[data-rotulo]::before{{content:attr(data-rotulo)}}\
.phx-tabela td input,.phx-tabela td select{{min-inline-size:0}}}}\
",
        sm = abaixo(bp("sm")),
    );
    let mut conteineres = BTreeSet::new();
    let mut regras = BTreeSet::new();
    for t in &app.screens {
        for (l, c, _) in intencoes_da_tela(t) {
            conteineres.insert(c);
            regras.insert(regra_css(&l));
        }
    }
    for c in conteineres {
        s.push_str(&format!(
            ".phx-c-{c}{{container:{c}/inline-size;min-inline-size:0}}"
        ));
    }
    for r in regras {
        s.push_str(&r);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_sao_os_cinco_do_bootstrap_em_ordem() {
        let nomes: Vec<&str> = breakpoints().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(nomes, ["sm", "md", "lg", "xl", "xxl"]);
        assert!(breakpoints().windows(2).all(|w| w[0].1 < w[1].1));
    }

    #[test]
    fn colunas_seguem_a_regra_no_ponto_exato() {
        let (l, _) = intencao_padrao();
        let sm = bp("sm") as f64;
        assert_eq!(colunas_em(&l, sm - 1.0, 1920.0), 1);
        assert_eq!(colunas_em(&l, sm, 1920.0), 2);
        assert_eq!(colunas_em(&l, bp("lg") as f64, 320.0), 4);
    }
}

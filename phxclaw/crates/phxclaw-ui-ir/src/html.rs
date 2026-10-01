//! Renderizador HTML do UI-IR: um prototipo navegavel em arquivo unico (sem rede). E o
//! MOTOR de marcacao dos adaptadores web: o adaptador «phoenix» (este, nativo, com os
//! tokens da casa) e o `bootstrap` desenham a MESMA arvore -- mesma ordem, mesmos ids, mesma
//! tabulacao -- e so trocam as classes de componente. A grade (quantas colunas, quando
//! vira cartao) nao e de nenhum dos dois: sai do motor responsivo (`responsivo::css`), e e
//! por isso que o mesmo IR da a mesma estrutura de colunas nos dois.
//!
//! Convencao de acao da casa: verde inclui, amarelo altera, vermelho exclui, azul consulta --
//! sempre contorno, preenchimento so no hover (fundo cheio com texto escuro ficava ilegivel).

use crate::ir::*;
use crate::responsivo::{self, bp};

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// As classes de COMPONENTE de um adaptador. Layout nao entra aqui: um adaptador que
/// pudesse escrever `row`/`col-*` decidiria a grade, e o mesmo IR daria telas diferentes.
pub(crate) struct Adaptador {
    pub rotulo: &'static str,
    /// TextField (input, textarea).
    pub texto: &'static str,
    pub selecao: &'static str,
    pub marca: &'static str,
    /// Controle com prefixo ou botao colado (moeda, lookup).
    pub grupo: &'static str,
    pub prefixo: &'static str,
    /// Card: a secao.
    pub secao: &'static str,
    pub legenda: &'static str,
    /// DataTable.
    pub tabela: &'static str,
    pub obrig: &'static str,
    pub so_leitor: &'static str,
    pub vazio: &'static str,
    pub menu: &'static str,
    /// Card de uma colecao (o painel do PHX JSON).
    pub cartao: &'static str,
    /// Button pela variante da acao (include, alter, delete, query, custom) e se e mini.
    pub botao: fn(&str, bool) -> String,
}

fn botao_phoenix(variante: &str, mini: bool) -> String {
    format!("acao {}{}", esc(variante), if mini { " mini" } else { "" })
}

pub(crate) const PHOENIX: Adaptador = Adaptador {
    rotulo: "",
    texto: "",
    selecao: "",
    marca: "chk",
    grupo: "",
    prefixo: "",
    secao: "",
    legenda: "",
    tabela: "",
    obrig: "obrig",
    so_leitor: "sr",
    vazio: "vazio",
    menu: "",
    cartao: "",
    botao: botao_phoenix,
};

/// ` class="..."` com as partes nao vazias; nada quando todas sao vazias.
fn cl(partes: &[&str]) -> String {
    let v: Vec<&str> = partes.iter().copied().filter(|p| !p.is_empty()).collect();
    if v.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", v.join(" "))
    }
}

fn campo<'a>(app: &'a App, entidade: &str, nome: &str) -> Option<(&'a Entity, &'a Field)> {
    let e = app.entities.iter().find(|e| e.name == entidade)?;
    Some((e, e.fields.iter().find(|f| f.name == nome)?))
}

fn controle(app: &App, f: &Field, id: &str, a: &Adaptador) -> String {
    let req = if f.required {
        " required aria-required=\"true\""
    } else {
        ""
    };
    let ro = if f.readonly {
        " readonly tabindex=\"-1\""
    } else {
        ""
    };
    let ml = f
        .max_len
        .map(|n| format!(" maxlength=\"{n}\""))
        .unwrap_or_default();
    let base = format!("id=\"{id}\" name=\"{}\"{req}{ro}", esc(&f.name));
    let tx = cl(&[a.texto]);
    match &f.widget {
        Widget::TextArea => format!("<textarea {base}{tx} rows=\"3\"></textarea>"),
        Widget::Integer => format!("<input {base}{tx} type=\"number\" step=\"1\">"),
        Widget::Decimal { scale } => format!(
            "<input {base}{tx} type=\"number\" step=\"{}\">",
            1.0 / 10f64.powi(*scale as i32)
        ),
        Widget::Money => format!(
            "<span{}><span{} aria-hidden=\"true\">R$</span><input {base}{} inputmode=\"decimal\" data-money placeholder=\"0,00\"></span>",
            cl(&["phx-junto", a.grupo]),
            cl(&[a.prefixo]),
            cl(&[a.texto, "num"])
        ),
        // Data com mascara propria, como o dinheiro: o <input type=date> segue o idioma do
        // NAVEGADOR, e numa tela em portugues mostrava mm/dd/yyyy (visto no print).
        Widget::Date => format!(
            "<input {base}{tx} inputmode=\"numeric\" data-data placeholder=\"dd/mm/aaaa\" maxlength=\"10\" pattern=\"\\d{{2}}/\\d{{2}}/\\d{{4}}\">"
        ),
        Widget::DateTime => format!(
            "<input {base}{tx} inputmode=\"numeric\" data-datahora placeholder=\"dd/mm/aaaa hh:mm\" maxlength=\"16\" pattern=\"\\d{{2}}/\\d{{2}}/\\d{{4}} \\d{{2}}:\\d{{2}}\">"
        ),
        Widget::Checkbox => format!("<input {base}{} type=\"checkbox\">", cl(&[a.marca])),
        Widget::Email => format!("<input {base}{tx} type=\"email\"{ml}>"),
        Widget::Phone => format!("<input {base}{tx} type=\"tel\"{ml}>"),
        Widget::Select { options } => format!(
            "<select {base}{}><option value=\"\">Selecione</option>{}</select>",
            cl(&[a.selecao]),
            options
                .iter()
                .map(|o| format!("<option>{}</option>", esc(o)))
                .collect::<String>()
        ),
        Widget::Lookup { entity } => {
            let rot = app
                .entities
                .iter()
                .find(|e| &e.name == entity)
                .map(|e| e.label.clone())
                .unwrap_or_else(|| entity.clone());
            format!(
                "<span{}><select {base}{}><option value=\"\">Selecione {}</option></select><button type=\"button\"{} title=\"Pesquisar {2}\" aria-label=\"Pesquisar {2}\">&#128269;</button></span>",
                cl(&["phx-junto", a.grupo]),
                cl(&[a.selecao]),
                esc(&rot.to_lowercase()),
                cl(&[&(a.botao)("query", true)])
            )
        }
        Widget::Text => format!("<input {base}{tx} type=\"text\"{ml}>"),
    }
}

/// Um campo com o rotulo em cima. `largo` ocupa a fila inteira da grade (do motor).
fn campo_html(app: &App, f: &Field, id: &str, a: &Adaptador) -> String {
    format!(
        "<div{}><label for=\"{id}\"{}>{}{}</label>{}</div>",
        cl(&[
            "phx-campo",
            if responsivo::campo_largo(f) {
                "phx-largo"
            } else {
                ""
            }
        ]),
        cl(&[a.rotulo]),
        esc(&f.label),
        if f.required {
            format!(" <span{} aria-hidden=\"true\">*</span>", cl(&[a.obrig]))
        } else {
            String::new()
        },
        controle(app, f, id, a)
    )
}

fn secoes_html(
    app: &App,
    entidade: &str,
    secoes: &[Section],
    prefixo: &str,
    a: &Adaptador,
) -> String {
    let mut h = String::new();
    for s in secoes {
        let (conteiner, grade) = responsivo::classes_da_secao(s);
        h.push_str(&format!(
            "<fieldset{}><legend{}>{}</legend><div{}>",
            cl(&[&conteiner, a.secao]),
            cl(&[a.legenda]),
            esc(&s.title),
            cl(&[&grade])
        ));
        for nome in &s.fields {
            if let Some((_, f)) = campo(app, entidade, nome) {
                h.push_str(&campo_html(app, f, &format!("{prefixo}-{}", f.name), a));
            }
        }
        h.push_str("</div></fieldset>");
    }
    h
}

/// Toolbar: as acoes numa fila que quebra, na ordem do IR.
fn acoes_html(acoes: &[Action], a: &Adaptador) -> String {
    let mut h = String::from("<div class=\"acoes phx-fila\">");
    for x in acoes {
        h.push_str(&format!(
            "<button type=\"button\"{} data-acao=\"{}\">{}</button>",
            cl(&[&(a.botao)(&x.kind, false)]),
            esc(&x.id),
            esc(&x.label)
        ));
    }
    h.push_str("</div>");
    h
}

/// O painel com a colecao de cartoes. O painel abre o conteiner nomeado e a grade dos
/// cartoes (do motor) consulta a largura DELE. Cada no leva o UUID do PHX JSON em
/// `data-uuid`: redimensionar nao recria cartao, e a identidade e conferivel no DOM. O
/// detalhe usa `<details>` nativo -- sem JS de framework mexendo no DOM.
fn painel_html(conteiner: &str, c: &Colecao, a: &Adaptador) -> String {
    let grade = responsivo::classe(&c.layout);
    let mut h = format!(
        "<div class=\"phx-painel phx-c-{}\"><div{} data-uuid=\"{}\" data-template=\"{}\">",
        esc(conteiner),
        cl(&[&grade]),
        esc(&c.id),
        esc(&c.template)
    );
    for k in &c.itens {
        let caps: String = k
            .capacidades
            .iter()
            .map(|x| format!("<li>{}</li>", esc(x)))
            .collect();
        h.push_str(&format!(
            "<article{} data-uuid=\"{id}\" data-tom=\"{}\"><div class=\"phx-fila\"><span class=\"phx-sigla\" aria-hidden=\"true\">{}</span>\
             <div><h2>{}</h2><p class=\"phx-sub\">{}</p></div></div><p>{}</p><ul class=\"phx-fila phx-caps\">{caps}</ul>\
             <details><summary>Ver detalhes de {3}</summary><p>{}</p><p><code>{id}</code></p></details></article>",
            cl(&["phx-cartao", a.cartao]),
            esc(&k.tom),
            esc(&k.sigla),
            esc(&k.titulo),
            esc(&k.subtitulo),
            esc(&k.tarefa),
            esc(&k.detalhe),
            id = esc(&k.id),
        ));
    }
    h.push_str("</div></div>");
    h
}

/// Menu e telas, com o layout lido aplicado (o mesmo `App::com_layout` de todo desenho).
pub(crate) fn corpo(app: &App, a: &Adaptador) -> (String, String) {
    let app = &app.com_layout();
    let mut telas = String::new();
    for s in &app.screens {
        let corpo = match s {
            Screen::List {
                id,
                entity,
                columns,
                search_fields,
                opens,
                ..
            } => {
                let e = app.entities.iter().find(|e| &e.name == entity);
                let rot = |n: &str| {
                    e.and_then(|e| e.fields.iter().find(|f| f.name == n))
                        .map(|f| f.label.clone())
                        .unwrap_or_else(|| n.to_string())
                };
                // os filtros sao uma secao sem titulo: mesma intencao padrao do motor
                let (conteiner, grade) = responsivo::classes_da_secao(&Section {
                    title: String::new(),
                    fields: vec![],
                    layout: None,
                    conteiner: None,
                });
                let filtros: String = search_fields
                    .iter()
                    .map(|n| {
                        format!(
                            "<div class=\"phx-campo\"><label for=\"{id}-f-{n}\"{}>{}</label><input id=\"{id}-f-{n}\" type=\"search\"{}></div>",
                            cl(&[a.rotulo]),
                            esc(&rot(n)),
                            cl(&[a.texto])
                        )
                    })
                    .collect();
                let cab: String = columns
                    .iter()
                    .map(|c| format!("<th scope=\"col\">{}</th>", esc(&rot(c))))
                    .collect();
                format!(
                    "<form{} role=\"search\"><div{}>{filtros}</div>\
                     <div class=\"acoes phx-fila\"><button type=\"button\"{}>Pesquisar</button>\
                     <a{} href=\"#{opens}\">Incluir</a></div></form>\
                     <div class=\"phx-tabela\"><table{}><thead><tr>{cab}</tr></thead><tbody><tr><td colspan=\"{}\"{}>Nenhum registro. Use Pesquisar ou Incluir.</td></tr></tbody></table></div>",
                    cl(&["filtros", &conteiner]),
                    cl(&[&grade]),
                    cl(&[&(a.botao)("query", false)]),
                    cl(&[&(a.botao)("include", false)]),
                    cl(&[a.tabela]),
                    columns.len(),
                    cl(&[a.vazio])
                )
            }
            Screen::Form {
                id,
                entity,
                sections,
                actions,
                ..
            } => {
                format!(
                    "<form>{}{}</form>",
                    secoes_html(app, entity, sections, id, a),
                    acoes_html(actions, a)
                )
            }
            Screen::Painel {
                conteiner, colecao, ..
            } => painel_html(conteiner, colecao, a),
            Screen::MasterDetail {
                id,
                master,
                detail,
                detail_columns,
                totals,
                header,
                actions,
                ..
            } => {
                let de = app.entities.iter().find(|e| &e.name == detail);
                let rot_det = de
                    .map(|e| e.label_plural.clone())
                    .unwrap_or_else(|| detail.clone());
                let cab: String = detail_columns
                    .iter()
                    .filter_map(|c| campo(app, detail, c))
                    .map(|(_, f)| format!("<th scope=\"col\">{}</th>", esc(&f.label)))
                    .collect();
                // cada celula leva o rotulo da coluna: em conteiner estreito a linha vira
                // cartao, o cabecalho sai de vista e o rotulo aparece ao lado do controle
                let linha: String = detail_columns
                    .iter()
                    .filter_map(|c| campo(app, detail, c))
                    .map(|(_, f)| {
                        format!(
                            "<td data-rotulo=\"{}\">{}</td>",
                            esc(&f.label),
                            controle(app, f, &format!("{id}-det-{}-__n__", f.name), a)
                        )
                    })
                    .collect();
                let tot: String = totals
                    .iter()
                    .map(|t| format!("<div class=\"total\"><span>{}</span><output data-soma=\"{}\">R$ 0,00</output></div>", esc(&t.label), esc(&t.sum_of)))
                    .collect();
                format!(
                    "<form>{}<fieldset{}><legend{}>{}</legend><div class=\"phx-tabela\"><table{} data-modelo=\"{}\">\
                     <thead><tr>{cab}<th><span{}>Ações</span></th></tr></thead><tbody></tbody></table></div>\
                     <button type=\"button\"{} data-add-item>Adicionar item</button>\
                     <div class=\"totais phx-fila phx-fim\">{tot}</div></fieldset>{}</form>",
                    secoes_html(app, master, header, id, a),
                    cl(&[a.secao]),
                    cl(&[a.legenda]),
                    esc(&rot_det),
                    cl(&["itens", a.tabela]),
                    esc(&format!(
                        "{linha}<td data-rotulo=\"Ações\"><button type=\"button\"{} data-del-item aria-label=\"Remover item\">&#10005;</button></td>",
                        cl(&[&(a.botao)("delete", true)])
                    )),
                    cl(&[a.so_leitor]),
                    cl(&[&(a.botao)("include", false)]),
                    acoes_html(actions, a)
                )
            }
        };
        telas.push_str(&format!(
            "<section class=\"tela phx-tela\" id=\"{}\" aria-labelledby=\"{0}-t\" hidden><h1 id=\"{0}-t\">{}</h1>{corpo}</section>",
            esc(s.id()),
            esc(s.title())
        ));
    }
    let mut menu = String::new();
    for g in &app.menu {
        menu.push_str(&format!(
            "<h2>{}</h2><ul class=\"phx-menu-lista\">",
            esc(&g.title)
        ));
        for id in &g.screens {
            if let Some(s) = app.screens.iter().find(|s| s.id() == id) {
                menu.push_str(&format!(
                    "<li><a{} href=\"#{}\">{}</a></li>",
                    cl(&[a.menu]),
                    esc(id),
                    esc(s.title())
                ));
            }
        }
        menu.push_str("</ul>");
    }
    (menu, telas)
}

/// A pagina inteira: `cabeca` entra no `<head>` (a folha do adaptador), e o corpo e o mesmo
/// para todos -- nav e main numa grade de composicao do motor.
pub(crate) fn pagina(app: &App, a: &Adaptador, cabeca: &str, rodape: &str) -> String {
    let (menu, telas) = corpo(app, a);
    let primeira = app
        .screens
        .first()
        .map(|s| s.id().to_string())
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{nome}</title>{cabeca}</head><body class=\"phx-app\">\
         <nav aria-label=\"Menu do sistema\"><div class=\"marca\">{nome}</div>{menu}<p class=\"rodape\">{rodape} v{v} pelo PhxClaw</p></nav>\
         <main class=\"phx-c-tela\">{telas}</main><script>const PRIMEIRA=\"{primeira}\";{JS}</script></body></html>",
        nome = esc(&app.name),
        v = app.ir_version
    )
}

/// Tokens da casa: cor, fonte, espaco e o lado do alvo de toque. Mobile-first: o alvo nasce
/// com 44 px e so encolhe no desktop de ponteiro fino; com toque, volta a 44 em qualquer
/// largura. Os dois adaptadores leem estes nomes -- o «phoenix» direto, o Bootstrap pelas
/// variaveis `--bs-*` que o adaptador dele aponta para ca.
pub(crate) fn tokens_css(app: &App) -> String {
    format!(
        "\
:root{{--fundo:#07121d;--painel:#0b1b29;--borda:#1d3a50;--texto:#e6eef5;--fraco:#8aa4b8;--ouro:#f5c64d;\
--inclui:#3ecf8e;--altera:#f2c14e;--exclui:#ff5d5d;--consulta:#4fb3ff;--foco:#7fd1ff;\
--phx-gap-xs:4px;--phx-gap-sm:8px;--phx-gap-md:10px 14px;--phx-gap-lg:16px;--phx-gap-xl:24px;\
--alvo:44px;--alvo-mini:44px;--marca:24px;--fonte:16px;--fonte-rotulo:13px;--fonte-titulo:20px;--respiro:16px}}\
@media (prefers-color-scheme: light){{:root{{--fundo:#f4f6f9;--painel:#fff;--borda:#c9d3de;--texto:#15212c;--fraco:#51606e;\
--ouro:#8a5a00;--inclui:#137a4b;--altera:#8a6100;--exclui:#b3261e;--consulta:#0b5cad;--foco:#0b5cad}}}}\
@media (min-width:{lg}px){{:root{{--alvo:36px;--alvo-mini:0px;--marca:13px;--fonte:14px;--fonte-rotulo:12px;--fonte-titulo:22px;--respiro:24px 28px}}}}\
@media (pointer:coarse){{:root{{--alvo:44px;--alvo-mini:44px;--marca:24px}}}}\
:focus-visible{{outline:3px solid var(--foco);outline-offset:2px}}[data-money]{{text-align:end}}\
{app}",
        lg = bp("lg"),
        app = responsivo::tokens_do_app(app)
    )
}

/// A folha do adaptador «phoenix»: tokens, layout do motor e os componentes da casa.
pub(crate) fn css(app: &App) -> String {
    format!("{}{}{CSS}", tokens_css(app), responsivo::css(app))
}

pub fn render(app: &App) -> String {
    pagina(
        app,
        &PHOENIX,
        &format!("<style>{}</style>", css(app)),
        "Protótipo gerado do UI-IR",
    )
}

/// Os componentes do adaptador «phoenix». Sem numero de quebra: o que muda com a largura e
/// token (`--alvo`, `--fonte`) ou e do motor.
pub(crate) const CSS: &str = r#"
*{box-sizing:border-box}body{margin:0;background:var(--fundo);color:var(--texto);font:var(--fonte)/1.45 system-ui,-apple-system,"Segoe UI",sans-serif;overflow-wrap:break-word}
nav{background:var(--painel);padding:16px}nav .marca{font-weight:700;color:var(--ouro);font-size:18px;margin-bottom:8px}
nav h2{font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--fraco);margin:12px 0 6px}
nav a{display:flex;align-items:center;min-block-size:var(--alvo);min-inline-size:var(--alvo);padding:4px 10px;border-radius:6px;color:var(--texto);text-decoration:none}nav a:hover,nav a[aria-current]{background:var(--borda)}
.rodape{margin-top:16px;font-size:11px;color:var(--fraco)}main{padding:0}.tela{padding:var(--respiro)}h1{font-size:var(--fonte-titulo);margin:0 0 16px}
fieldset{border:1px solid var(--borda);border-radius:8px;padding:12px 14px;margin:0 0 14px;background:var(--painel);min-inline-size:0}legend{padding:0 6px;color:var(--fraco);font-size:var(--fonte-rotulo);letter-spacing:.06em}
label{font-size:var(--fonte-rotulo);color:var(--fraco)}.obrig{color:var(--exclui)}
input,select,textarea{background:var(--fundo);color:var(--texto);border:1px solid var(--borda);border-radius:6px;padding:7px 9px;font:inherit;min-block-size:var(--alvo);inline-size:100%;min-inline-size:0}
textarea{min-block-size:calc(var(--alvo) * 2)}
:user-invalid{border-color:var(--exclui);box-shadow:0 0 0 1px var(--exclui)}input.chk{inline-size:var(--marca);block-size:var(--marca);min-block-size:0;align-self:flex-start;accent-color:var(--consulta)}input[readonly]{opacity:.7}
.phx-junto>span{align-self:center}
.acao{background:transparent;border:1px solid;border-radius:6px;padding:7px 14px;font:inherit;cursor:pointer;text-decoration:none;display:inline-flex;align-items:center;justify-content:center;min-block-size:var(--alvo);min-inline-size:var(--alvo)}
.acao.mini{padding:4px 8px;min-block-size:var(--alvo-mini);min-inline-size:var(--alvo-mini)}.acao.include{color:var(--inclui)}.acao.alter{color:var(--altera)}.acao.delete{color:var(--exclui)}.acao.query,.acao.custom{color:var(--consulta)}
.acao:hover{color:var(--fundo)!important}.acao.include:hover{background:var(--inclui)}.acao.alter:hover{background:var(--altera)}.acao.delete:hover{background:var(--exclui)}.acao.query:hover,.acao.custom:hover{background:var(--consulta)}
.acoes{margin:10px 0}.phx-tabela{border:1px solid var(--borda);border-radius:8px;margin:8px 0}table{border-collapse:collapse;inline-size:100%}
th,td{padding:7px 9px;border-bottom:1px solid var(--borda);text-align:left;vertical-align:middle}th{font-size:12px;color:var(--fraco);font-weight:600;background:var(--painel);white-space:nowrap}
td input,td select{min-inline-size:110px}.phx-tabela td[data-rotulo]::before{font-size:var(--fonte-rotulo);color:var(--fraco)}.vazio{color:var(--fraco);text-align:center;padding:18px}
.totais{margin-top:10px;column-gap:24px}.total{display:flex;flex-direction:column;align-items:flex-end}.total span{font-size:12px;color:var(--fraco)}.total output{font-size:18px;font-weight:700}
.sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.phx-cartao{background:var(--painel);border:1px solid var(--borda);border-radius:var(--phx-raio,12px);padding:16px;display:flex;flex-direction:column;gap:8px}
.phx-cartao h2{font-size:15px;margin:0}.phx-cartao p{margin:0}.phx-sub{font-size:var(--fonte-rotulo);color:var(--fraco)}
.phx-sigla{display:grid;place-items:center;inline-size:40px;block-size:40px;border-radius:10px;background:var(--borda);font-weight:700}
.phx-caps{list-style:none;margin:0;padding:0}.phx-caps li{font-size:12px;border:1px solid var(--borda);border-radius:6px;padding:2px 6px}
.phx-cartao summary{min-block-size:var(--alvo);display:flex;align-items:center;cursor:pointer;color:var(--phx-acento,var(--consulta))}
"#;

const JS: &str = r#"
const telas=[...document.querySelectorAll('.tela')];
function mostra(){const id=(location.hash||'#'+PRIMEIRA).slice(1);let ok=false;
telas.forEach(t=>{const v=t.id===id;t.hidden=!v;ok=ok||v});if(!ok&&telas[0])telas[0].hidden=false;
document.querySelectorAll('nav a').forEach(a=>a.toggleAttribute('aria-current',a.getAttribute('href')==='#'+id))}
addEventListener('hashchange',mostra);mostra();
const num=v=>{v=String(v||'').replace(/\./g,'').replace(',','.');const n=parseFloat(v);return isNaN(n)?0:n};
const brl=n=>'R$ '+n.toLocaleString('pt-BR',{minimumFractionDigits:2,maximumFractionDigits:2});
function soma(form){form.querySelectorAll('output[data-soma]').forEach(o=>{const c=o.dataset.soma;let s=0;
form.querySelectorAll('table.itens tbody [name="'+c+'"]').forEach(i=>s+=num(i.value));o.value=brl(s);o.textContent=brl(s)})}
let seq=0;document.querySelectorAll('[data-add-item]').forEach(b=>b.addEventListener('click',()=>{const t=b.closest('fieldset').querySelector('table.itens');
const tr=document.createElement('tr');tr.innerHTML=t.dataset.modelo.replaceAll('__n__',String(++seq));t.tBodies[0].appendChild(tr);
const p=tr.querySelector('input,select');if(p)p.focus()}));
document.addEventListener('click',e=>{const d=e.target.closest('[data-del-item]');if(d){const f=d.closest('form');d.closest('tr').remove();soma(f)}});
document.addEventListener('input',e=>{if(e.target.closest('table.itens'))soma(e.target.closest('form'))});
const mascara=(v,dh)=>{const d=v.replace(/\D/g,'').slice(0,dh?12:8);let r=d.slice(0,2);
if(d.length>2)r+='/'+d.slice(2,4);if(d.length>4)r+='/'+d.slice(4,8);
if(dh&&d.length>8)r+=' '+d.slice(8,10)+(d.length>10?':'+d.slice(10,12):'');return r};
// mesma regra do DateValid do WLanguage (Help 3027003) e do Rust gerado
function dataValida(d,mo,a){if(a<1||a>9999||mo<1||mo>12)return false;const k=a*10000+mo*100+d;if(k>=15821005&&k<=15821014)return false;const bis=k<15821005?a%4===0:(a%4===0&&a%100!==0)||a%400===0;const dias=[31,bis?29:28,31,30,31,30,31,31,30,31,30,31];return d>=1&&d<=dias[mo-1]}
function dataOk(v,dh){const m=(dh?/^(\d{2})\/(\d{2})\/(\d{4}) (\d{2}):(\d{2})$/:/^(\d{2})\/(\d{2})\/(\d{4})$/).exec(v);
if(!m)return false;const d=+m[1],mo=+m[2],a=+m[3];return dataValida(d,mo,a)&&(!dh||(+m[4]<24&&+m[5]<60))}
document.addEventListener('input',e=>{const i=e.target;if(!i.matches('[data-data],[data-datahora]'))return;
const dh=i.hasAttribute('data-datahora');i.value=mascara(i.value,dh);
i.setCustomValidity(i.value.length===(dh?16:10)&&!dataOk(i.value,dh)?'Data inexistente':'')});
"#;

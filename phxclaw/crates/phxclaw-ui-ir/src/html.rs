//! Renderizador HTML do UI-IR: um prototipo navegavel em arquivo unico (sem framework, sem
//! rede). E UM renderizador; React, WinDev, WebDev entram como irmaos lendo o mesmo IR.
//!
//! Convencao de acao da casa: verde inclui, amarelo altera, vermelho exclui, azul consulta --
//! sempre contorno, preenchimento so no hover (fundo cheio com texto escuro ficava ilegivel).

use crate::ir::*;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn campo<'a>(app: &'a App, entidade: &str, nome: &str) -> Option<(&'a Entity, &'a Field)> {
    let e = app.entities.iter().find(|e| e.name == entidade)?;
    Some((e, e.fields.iter().find(|f| f.name == nome)?))
}

fn controle(app: &App, f: &Field, id: &str) -> String {
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
    match &f.widget {
        Widget::TextArea => format!("<textarea {base} rows=\"3\"></textarea>"),
        Widget::Integer => format!("<input {base} type=\"number\" step=\"1\">"),
        Widget::Decimal { scale } => format!(
            "<input {base} type=\"number\" step=\"{}\">",
            1.0 / 10f64.powi(*scale as i32)
        ),
        Widget::Money => format!(
            "<span class=\"moeda\"><span aria-hidden=\"true\">R$</span><input {base} inputmode=\"decimal\" class=\"num\" data-money placeholder=\"0,00\"></span>"
        ),
        // Data com mascara propria, como o dinheiro: o <input type=date> segue o idioma do
        // NAVEGADOR, e numa tela em portugues mostrava mm/dd/yyyy (visto no print).
        Widget::Date => format!(
            "<input {base} inputmode=\"numeric\" data-data placeholder=\"dd/mm/aaaa\" maxlength=\"10\" pattern=\"\\d{{2}}/\\d{{2}}/\\d{{4}}\">"
        ),
        Widget::DateTime => format!(
            "<input {base} inputmode=\"numeric\" data-datahora placeholder=\"dd/mm/aaaa hh:mm\" maxlength=\"16\" pattern=\"\\d{{2}}/\\d{{2}}/\\d{{4}} \\d{{2}}:\\d{{2}}\">"
        ),
        Widget::Checkbox => format!("<input {base} type=\"checkbox\" class=\"chk\">"),
        Widget::Email => format!("<input {base} type=\"email\"{ml}>"),
        Widget::Phone => format!("<input {base} type=\"tel\"{ml}>"),
        Widget::Select { options } => format!(
            "<select {base}><option value=\"\">Selecione</option>{}</select>",
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
                "<span class=\"lookup\"><select {base}><option value=\"\">Selecione {}</option></select><button type=\"button\" class=\"acao query mini\" title=\"Pesquisar {0}\" aria-label=\"Pesquisar {0}\">&#128269;</button></span>",
                esc(&rot.to_lowercase())
            )
        }
        Widget::Text => format!("<input {base} type=\"text\"{ml}>"),
    }
}

fn secoes_html(app: &App, entidade: &str, secoes: &[Section], prefixo: &str) -> String {
    let mut h = String::new();
    for s in secoes {
        h.push_str(&format!(
            "<fieldset><legend>{}</legend><div class=\"grade-form\">",
            esc(&s.title)
        ));
        for nome in &s.fields {
            if let Some((_, f)) = campo(app, entidade, nome) {
                let id = format!("{prefixo}-{}", f.name);
                let largo = matches!(f.widget, Widget::TextArea) || f.max_len.unwrap_or(0) > 80;
                h.push_str(&format!(
                    "<div class=\"campo{}\"><label for=\"{id}\">{}{}</label>{}</div>",
                    if largo { " largo" } else { "" },
                    esc(&f.label),
                    if f.required {
                        " <span class=\"obrig\" aria-hidden=\"true\">*</span>"
                    } else {
                        ""
                    },
                    controle(app, f, &id)
                ));
            }
        }
        h.push_str("</div></fieldset>");
    }
    h
}

fn acoes_html(acoes: &[Action]) -> String {
    let mut h = String::from("<div class=\"acoes\">");
    for a in acoes {
        h.push_str(&format!(
            "<button type=\"button\" class=\"acao {}\" data-acao=\"{}\">{}</button>",
            esc(&a.kind),
            esc(&a.id),
            esc(&a.label)
        ));
    }
    h.push_str("</div>");
    h
}

pub fn render(app: &App) -> String {
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
                let filtros: String = search_fields
                    .iter()
                    .map(|n| format!("<div class=\"campo\"><label for=\"{id}-f-{n}\">{}</label><input id=\"{id}-f-{n}\" type=\"search\"></div>", esc(&rot(n))))
                    .collect();
                let cab: String = columns
                    .iter()
                    .map(|c| format!("<th scope=\"col\">{}</th>", esc(&rot(c))))
                    .collect();
                format!(
                    "<form class=\"filtros\" role=\"search\"><div class=\"grade-form\">{filtros}</div>\
                     <div class=\"acoes\"><button type=\"button\" class=\"acao query\">Pesquisar</button>\
                     <a class=\"acao include\" href=\"#{opens}\">Incluir</a></div></form>\
                     <div class=\"tabela\"><table><thead><tr>{cab}</tr></thead><tbody><tr><td colspan=\"{}\" class=\"vazio\">Nenhum registro. Use Pesquisar ou Incluir.</td></tr></tbody></table></div>",
                    columns.len()
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
                    secoes_html(app, entity, sections, id),
                    acoes_html(actions)
                )
            }
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
                let linha: String = detail_columns
                    .iter()
                    .filter_map(|c| campo(app, detail, c))
                    .map(|(_, f)| {
                        format!(
                            "<td>{}</td>",
                            controle(app, f, &format!("{id}-det-{}-__n__", f.name))
                        )
                    })
                    .collect();
                let tot: String = totals
                    .iter()
                    .map(|t| format!("<div class=\"total\"><span>{}</span><output data-soma=\"{}\">R$ 0,00</output></div>", esc(&t.label), esc(&t.sum_of)))
                    .collect();
                format!(
                    "<form>{}<fieldset><legend>{}</legend><div class=\"tabela\"><table class=\"itens\" data-modelo=\"{}\">\
                     <thead><tr>{cab}<th><span class=\"sr\">Ações</span></th></tr></thead><tbody></tbody></table></div>\
                     <button type=\"button\" class=\"acao include\" data-add-item>Adicionar item</button>\
                     <div class=\"totais\">{tot}</div></fieldset>{}</form>",
                    secoes_html(app, master, header, id),
                    esc(&rot_det),
                    esc(&format!(
                        "{linha}<td><button type=\"button\" class=\"acao delete mini\" data-del-item aria-label=\"Remover item\">&#10005;</button></td>"
                    )),
                    acoes_html(actions)
                )
            }
        };
        telas.push_str(&format!(
            "<section class=\"tela\" id=\"{}\" aria-labelledby=\"{0}-t\" hidden><h1 id=\"{0}-t\">{}</h1>{corpo}</section>",
            esc(s.id()),
            esc(s.title())
        ));
    }
    let mut menu = String::new();
    for g in &app.menu {
        menu.push_str(&format!("<h2>{}</h2><ul>", esc(&g.title)));
        for id in &g.screens {
            if let Some(s) = app.screens.iter().find(|s| s.id() == id) {
                menu.push_str(&format!(
                    "<li><a href=\"#{}\">{}</a></li>",
                    esc(id),
                    esc(s.title())
                ));
            }
        }
        menu.push_str("</ul>");
    }
    let primeira = app
        .screens
        .first()
        .map(|s| s.id().to_string())
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{nome}</title><style>{CSS}</style></head><body>\
         <nav aria-label=\"Menu do sistema\"><div class=\"marca\">{nome}</div>{menu}<p class=\"rodape\">Protótipo gerado do UI-IR v{v} pelo PhxClaw</p></nav>\
         <main>{telas}</main><script>const PRIMEIRA=\"{primeira}\";{JS}</script></body></html>",
        nome = esc(&app.name),
        v = app.ir_version
    )
}

const CSS: &str = r#"
:root{--fundo:#07121d;--painel:#0b1b29;--borda:#1d3a50;--texto:#e6eef5;--fraco:#8aa4b8;--ouro:#f5c64d;
--inclui:#3ecf8e;--altera:#f2c14e;--exclui:#ff5d5d;--consulta:#4fb3ff;--foco:#7fd1ff}
@media (prefers-color-scheme: light){:root{--fundo:#f4f6f9;--painel:#fff;--borda:#c9d3de;--texto:#15212c;--fraco:#51606e;
--ouro:#8a5a00;--inclui:#137a4b;--altera:#8a6100;--exclui:#b3261e;--consulta:#0b5cad;--foco:#0b5cad}}
*{box-sizing:border-box}body{margin:0;display:grid;grid-template-columns:240px 1fr;min-height:100vh;background:var(--fundo);color:var(--texto);font:14px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif}
nav{background:var(--painel);border-right:1px solid var(--borda);padding:16px}nav .marca{font-weight:700;color:var(--ouro);font-size:18px;margin-bottom:12px}
nav h2{font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--fraco);margin:16px 0 6px}nav ul{list-style:none;margin:0;padding:0}
nav a{display:block;padding:6px 8px;border-radius:6px;color:var(--texto);text-decoration:none}nav a:hover,nav a[aria-current]{background:var(--borda)}
.rodape{margin-top:24px;font-size:11px;color:var(--fraco)}main{padding:24px 28px;min-width:0}h1{font-size:22px;margin:0 0 16px}
fieldset{border:1px solid var(--borda);border-radius:8px;padding:12px 14px;margin:0 0 14px;background:var(--painel)}legend{padding:0 6px;color:var(--fraco);font-size:12px;letter-spacing:.06em}
.grade-form{display:grid;grid-template-columns:repeat(auto-fill,minmax(210px,1fr));gap:10px 14px}.campo{display:flex;flex-direction:column;gap:4px}.campo.largo{grid-column:1/-1}
label{font-size:12px;color:var(--fraco)}.obrig{color:var(--exclui)}
input,select,textarea{background:var(--fundo);color:var(--texto);border:1px solid var(--borda);border-radius:6px;padding:7px 9px;font:inherit;min-width:0;width:100%}
input.chk{width:auto;align-self:flex-start}input[readonly]{opacity:.7}:focus-visible{outline:2px solid var(--foco);outline-offset:1px}
.moeda{display:flex;align-items:center;gap:6px}.num{text-align:right}.lookup{display:flex;gap:6px}
.acoes{display:flex;flex-wrap:wrap;gap:8px;margin:10px 0}.acao{background:transparent;border:1px solid;border-radius:6px;padding:7px 14px;font:inherit;cursor:pointer;text-decoration:none;display:inline-block}
.acao.mini{padding:4px 8px}.acao.include{color:var(--inclui)}.acao.alter{color:var(--altera)}.acao.delete{color:var(--exclui)}.acao.query,.acao.custom{color:var(--consulta)}
.acao:hover{color:var(--fundo)!important}.acao.include:hover{background:var(--inclui)}.acao.alter:hover{background:var(--altera)}.acao.delete:hover{background:var(--exclui)}.acao.query:hover,.acao.custom:hover{background:var(--consulta)}
.tabela{overflow-x:auto;border:1px solid var(--borda);border-radius:8px;margin:8px 0}table{border-collapse:collapse;width:100%}
th,td{padding:7px 9px;border-bottom:1px solid var(--borda);text-align:left;vertical-align:middle}th{font-size:12px;color:var(--fraco);font-weight:600;background:var(--painel);white-space:nowrap}
td input,td select{min-width:110px}.vazio{color:var(--fraco);text-align:center;padding:18px}
.totais{display:flex;justify-content:flex-end;gap:24px;margin-top:10px}.total{display:flex;flex-direction:column;align-items:flex-end}.total span{font-size:12px;color:var(--fraco)}.total output{font-size:18px;font-weight:700}
.sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
@media (max-width:760px){body{grid-template-columns:1fr}nav{border-right:0;border-bottom:1px solid var(--borda)}}
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
function dataOk(v,dh){const m=(dh?/^(\d{2})\/(\d{2})\/(\d{4}) (\d{2}):(\d{2})$/:/^(\d{2})\/(\d{2})\/(\d{4})$/).exec(v);
if(!m)return false;const d=+m[1],mo=+m[2],a=+m[3],t=new Date(a,mo-1,d);
return t.getFullYear()===a&&t.getMonth()===mo-1&&t.getDate()===d&&(!dh||(+m[4]<24&&+m[5]<60))}
document.addEventListener('input',e=>{const i=e.target;if(!i.matches('[data-data],[data-datahora]'))return;
const dh=i.hasAttribute('data-datahora');i.value=mascara(i.value,dh);
i.setCustomValidity(i.value.length===(dh?16:10)&&!dataOk(i.value,dh)?'Data inexistente':'')});
"#;

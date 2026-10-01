//! Renderizador React: o mesmo UI-IR vira um projeto React (esbuild, sem Vite).
//!
//! Divisao: `ui.jsx` e o tempo de execucao fixo (controles, mascaras, mestre/detalhe);
//! `telas.jsx` e o GERADO -- um componente por tela, com a descricao dela escrita no
//! proprio arquivo, para quem abrir o projeto editar tela por tela. O CSS e o mesmo do
//! renderizador HTML: duas copias de estilo divergiriam na primeira correcao de contraste.

use crate::ir::{App, Screen};
use serde_json::{Value, json};

/// Arquivos do projeto: (caminho relativo, conteudo).
pub fn render(app: &App) -> Vec<(String, String)> {
    // o JSON embutido ja sai com secoes e colunas na ordem lida (v2): o componente nao a refaz
    let app = &app.com_layout();
    vec![
        ("package.json".into(), package(app)),
        ("index.html".into(), index(app)),
        ("src/ui.jsx".into(), UI_JSX.trim_start().into()),
        ("src/main.jsx".into(), MAIN_JSX.trim_start().into()),
        ("src/telas.jsx".into(), telas(app)),
    ]
}

fn js(v: &impl serde::Serialize) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

/// "pedido_documento" -> "TelaPedidoDocumento": nome de componente valido em JSX.
fn componente(id: &str) -> String {
    let mut s = String::from("Tela");
    for p in id.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut c = p.chars();
        if let Some(f) = c.next() {
            s.extend(f.to_uppercase());
            s.push_str(c.as_str());
        }
    }
    s
}

fn package(app: &App) -> String {
    let nome: String = app
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let p = json!({
        "name": format!("{}-erp", nome.trim_matches('-')),
        "private": true,
        "version": "0.1.0",
        "scripts": {
            "build": "esbuild src/main.jsx --bundle --minify --jsx=automatic --outfile=dist/app.js"
        },
        "dependencies": {"react": "18.3.1", "react-dom": "18.3.1"},
        "devDependencies": {"esbuild": "0.24.0"}
    });
    serde_json::to_string_pretty(&p).unwrap_or_default() + "\n"
}

fn index(app: &App) -> String {
    let nome = app
        .name
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html>\n<html lang=\"pt-BR\">\n<head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>{nome}</title>\n<style>{}\n#raiz{{display:contents}}</style></head>\n\
         <body><div id=\"raiz\"></div><script src=\"dist/app.js\"></script></body>\n</html>\n",
        crate::html::CSS
    )
}

fn telas(app: &App) -> String {
    let rotulos: serde_json::Map<String, Value> = app
        .entities
        .iter()
        .map(|e| (e.name.clone(), Value::String(e.label.clone())))
        .collect();
    let campos: serde_json::Map<String, Value> = app
        .entities
        .iter()
        .map(|e| {
            let m: serde_json::Map<String, Value> = e
                .fields
                .iter()
                .map(|f| {
                    (
                        f.name.clone(),
                        serde_json::to_value(f).unwrap_or(Value::Null),
                    )
                })
                .collect();
            (e.name.clone(), Value::Object(m))
        })
        .collect();
    let mut s = format!(
        "// GERADO do UI-IR v{} pelo PhxClaw. Um componente por tela.\n\
         import {{ Consulta, Cadastro, MestreDetalhe }} from \"./ui.jsx\";\n\n\
         export const NOME = {};\nexport const VERSAO = {};\n\
         const ROTULOS = {};\nconst CAMPOS = {};\n\n",
        app.ir_version,
        js(&app.name),
        app.ir_version,
        js(&rotulos),
        js(&campos)
    );
    let mut lista = vec![];
    for t in &app.screens {
        let c = componente(t.id());
        let desc = js(t);
        let corpo = match t {
            Screen::List { entity, .. } => {
                format!(
                    "<Consulta tela={{TELA}} campos={{CAMPOS[{}]}} />",
                    js(entity)
                )
            }
            Screen::Form { entity, .. } => format!(
                "<Cadastro tela={{TELA}} campos={{CAMPOS[{}]}} rotulos={{ROTULOS}} />",
                js(entity)
            ),
            Screen::MasterDetail { master, detail, .. } => {
                let itens = app
                    .entities
                    .iter()
                    .find(|e| &e.name == detail)
                    .map(|e| e.label_plural.clone())
                    .unwrap_or_else(|| detail.clone());
                format!(
                    "<MestreDetalhe tela={{TELA}} campos={{CAMPOS[{}]}} camposDet={{CAMPOS[{}]}} rotulos={{ROTULOS}} rotuloItens={{{}}} />",
                    js(master),
                    js(detail),
                    js(&itens)
                )
            }
        };
        s.push_str(&format!(
            "export function {c}() {{\n  const TELA = {desc};\n  return {corpo};\n}}\n\n"
        ));
        lista.push(format!(
            "  {{ id: {}, title: {}, Comp: {c} }}",
            js(&t.id()),
            js(&t.title())
        ));
    }
    let menu: Vec<Value> = app
        .menu
        .iter()
        .map(|g| {
            json!({"title": g.title, "itens": g.screens.iter().filter_map(|id| {
                app.screens.iter().find(|s| s.id() == id).map(|s| json!({"id": id, "title": s.title()}))
            }).collect::<Vec<_>>()})
        })
        .collect();
    s.push_str(&format!(
        "export const TELAS = [\n{}\n];\nexport const MENU = {};\n",
        lista.join(",\n"),
        js(&menu)
    ));
    s
}

const MAIN_JSX: &str = r##"
import { createRoot } from "react-dom/client";
import { useEffect, useState } from "react";
import { MENU, NOME, TELAS, VERSAO } from "./telas.jsx";

const atual = () => location.hash.slice(1);

function App() {
  const [id, setId] = useState(atual);
  useEffect(() => {
    const f = () => setId(atual());
    addEventListener("hashchange", f);
    return () => removeEventListener("hashchange", f);
  }, []);
  const tela = TELAS.find((t) => t.id === id) || TELAS[0];
  return (
    <>
      <nav aria-label="Menu do sistema">
        <div className="marca">{NOME}</div>
        {MENU.map((g) => (
          <div key={g.title}>
            <h2>{g.title}</h2>
            <ul>
              {g.itens.map((i) => (
                <li key={i.id}>
                  <a href={"#" + i.id} aria-current={i.id === tela.id ? "page" : undefined}>{i.title}</a>
                </li>
              ))}
            </ul>
          </div>
        ))}
        <p className="rodape">Protótipo React gerado do UI-IR v{VERSAO} pelo PhxClaw</p>
      </nav>
      <main>
        {/* key: trocar de tela zera o estado do formulario, como abrir outra janela */}
        <section className="tela" id={tela.id} key={tela.id} aria-labelledby={tela.id + "-t"}>
          <h1 id={tela.id + "-t"}>{tela.title}</h1>
          <tela.Comp />
        </section>
      </main>
    </>
  );
}

createRoot(document.getElementById("raiz")).render(<App />);
"##;

const UI_JSX: &str = r##"
// Tempo de execucao das telas geradas. Mesmas regras do renderizador HTML:
// dinheiro e data com mascara propria em pt-BR, total recalculado a cada digito.
import { useEffect, useRef, useState } from "react";

export const num = (v) => {
  const n = parseFloat(String(v || "").replace(/\./g, "").replace(",", "."));
  return isNaN(n) ? 0 : n;
};
export const brl = (n) =>
  "R$ " + n.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

export function mascaraData(v, dh) {
  const d = v.replace(/\D/g, "").slice(0, dh ? 12 : 8);
  let r = d.slice(0, 2);
  if (d.length > 2) r += "/" + d.slice(2, 4);
  if (d.length > 4) r += "/" + d.slice(4, 8);
  if (dh && d.length > 8) r += " " + d.slice(8, 10) + (d.length > 10 ? ":" + d.slice(10, 12) : "");
  return r;
}

// mesma regra do DateValid do WLanguage (Help 3027003) e do Rust gerado
export function dataValida(d, mo, a) {
  if (a < 1 || a > 9999 || mo < 1 || mo > 12) return false;
  const k = a * 10000 + mo * 100 + d;
  if (k >= 15821005 && k <= 15821014) return false;
  const bis = k < 15821005 ? a % 4 === 0 : (a % 4 === 0 && a % 100 !== 0) || a % 400 === 0;
  const dias = [31, bis ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return d >= 1 && d <= dias[mo - 1];
}

export function dataOk(v, dh) {
  const m = (dh ? /^(\d{2})\/(\d{2})\/(\d{4}) (\d{2}):(\d{2})$/ : /^(\d{2})\/(\d{2})\/(\d{4})$/).exec(v);
  if (!m) return false;
  return dataValida(+m[1], +m[2], +m[3]) && (!dh || (+m[4] < 24 && +m[5] < 60));
}

function Data({ id, f, valor, muda, dh }) {
  const ref = useRef(null);
  useEffect(() => {
    ref.current.setCustomValidity(valor.length === (dh ? 16 : 10) && !dataOk(valor, dh) ? "Data inexistente" : "");
  }, [valor, dh]);
  return (
    <input ref={ref} id={id} name={f.name} required={f.required} readOnly={f.readonly} inputMode="numeric"
      placeholder={dh ? "dd/mm/aaaa hh:mm" : "dd/mm/aaaa"} maxLength={dh ? 16 : 10}
      value={valor} onChange={(e) => muda(mascaraData(e.target.value, dh))} />
  );
}

export function Controle({ id, f, rotulos, valor, muda }) {
  const w = f.widget;
  const comum = { id, name: f.name, required: f.required, readOnly: f.readonly };
  const txt = { ...comum, value: valor ?? "", onChange: (e) => muda(e.target.value) };
  switch (w.kind) {
    case "text_area": return <textarea rows={3} {...txt} />;
    case "integer": return <input type="number" step="1" {...txt} />;
    case "decimal": return <input type="number" step={Math.pow(10, -w.scale)} {...txt} />;
    case "money":
      return (
        <span className="moeda"><span aria-hidden="true">R$</span>
          <input inputMode="decimal" className="num" placeholder="0,00" {...txt} /></span>
      );
    case "date": return <Data id={id} f={f} valor={valor ?? ""} muda={muda} />;
    case "date_time": return <Data id={id} f={f} valor={valor ?? ""} muda={muda} dh />;
    case "checkbox":
      return <input type="checkbox" className="chk" {...comum} checked={valor === true} onChange={(e) => muda(e.target.checked)} />;
    case "email": return <input type="email" maxLength={f.max_len} {...txt} />;
    case "phone": return <input type="tel" maxLength={f.max_len} {...txt} />;
    case "select":
      return (
        <select {...txt}><option value="">Selecione</option>
          {w.options.map((o) => <option key={o}>{o}</option>)}</select>
      );
    case "lookup": {
      const r = (rotulos[w.entity] || w.entity).toLowerCase();
      return (
        <span className="lookup">
          <select {...txt}><option value="">Selecione {r}</option></select>
          <button type="button" className="acao query mini" title={"Pesquisar " + r} aria-label={"Pesquisar " + r}>&#128269;</button>
        </span>
      );
    }
    default: return <input type="text" maxLength={f.max_len} {...txt} />;
  }
}

function Secoes({ secoes, campos, rotulos, prefixo, dados, muda }) {
  return secoes.map((s) => (
    <fieldset key={s.title}>
      <legend>{s.title}</legend>
      <div className="grade-form">
        {s.fields.map((n) => campos[n]).filter(Boolean).map((f) => {
          const id = prefixo + "-" + f.name;
          const largo = f.widget.kind === "text_area" || (f.max_len || 0) > 80;
          return (
            <div key={f.name} className={"campo" + (largo ? " largo" : "")}>
              <label htmlFor={id}>{f.label}{f.required && <> <span className="obrig" aria-hidden="true">*</span></>}</label>
              <Controle id={id} f={f} rotulos={rotulos} valor={dados[f.name]} muda={(v) => muda(f.name, v)} />
            </div>
          );
        })}
      </div>
    </fieldset>
  ));
}

function Acoes({ acoes }) {
  return (
    <div className="acoes">
      {acoes.map((a) => <button key={a.id} type="button" className={"acao " + a.kind} data-acao={a.id}>{a.label}</button>)}
    </div>
  );
}

function useRegistro() {
  const [d, set] = useState({});
  return [d, (k, v) => set((x) => ({ ...x, [k]: v }))];
}

export function Consulta({ tela, campos }) {
  const rot = (n) => (campos[n] ? campos[n].label : n);
  return (
    <>
      <form className="filtros" role="search">
        <div className="grade-form">
          {tela.search_fields.map((n) => (
            <div key={n} className="campo">
              <label htmlFor={tela.id + "-f-" + n}>{rot(n)}</label>
              <input id={tela.id + "-f-" + n} type="search" />
            </div>
          ))}
        </div>
        <div className="acoes">
          <button type="button" className="acao query">Pesquisar</button>
          <a className="acao include" href={"#" + tela.opens}>Incluir</a>
        </div>
      </form>
      <div className="tabela"><table>
        <thead><tr>{tela.columns.map((c) => <th key={c} scope="col">{rot(c)}</th>)}</tr></thead>
        <tbody><tr><td colSpan={tela.columns.length} className="vazio">Nenhum registro. Use Pesquisar ou Incluir.</td></tr></tbody>
      </table></div>
    </>
  );
}

export function Cadastro({ tela, campos, rotulos }) {
  const [d, muda] = useRegistro();
  return (
    <form>
      <Secoes secoes={tela.sections} campos={campos} rotulos={rotulos} prefixo={tela.id} dados={d} muda={muda} />
      <Acoes acoes={tela.actions} />
    </form>
  );
}

export function MestreDetalhe({ tela, campos, camposDet, rotulos, rotuloItens }) {
  const [d, muda] = useRegistro();
  const [itens, setItens] = useState([]);
  const seq = useRef(0);
  const cols = tela.detail_columns.map((c) => camposDet[c]).filter(Boolean);
  const mudaItem = (k, n, v) => setItens((l) => l.map((i) => (i.k === k ? { ...i, [n]: v } : i)));
  return (
    <form>
      <Secoes secoes={tela.header} campos={campos} rotulos={rotulos} prefixo={tela.id} dados={d} muda={muda} />
      <fieldset>
        <legend>{rotuloItens}</legend>
        <div className="tabela"><table className="itens">
          <thead><tr>{cols.map((f) => <th key={f.name} scope="col">{f.label}</th>)}<th><span className="sr">Ações</span></th></tr></thead>
          <tbody>
            {itens.map((i) => (
              <tr key={i.k}>
                {cols.map((f) => (
                  <td key={f.name}>
                    <Controle id={tela.id + "-det-" + f.name + "-" + i.k} f={f} rotulos={rotulos}
                      valor={i[f.name]} muda={(v) => mudaItem(i.k, f.name, v)} />
                  </td>
                ))}
                <td><button type="button" className="acao delete mini" aria-label="Remover item"
                  onClick={() => setItens((l) => l.filter((x) => x.k !== i.k))}>&#10005;</button></td>
              </tr>
            ))}
          </tbody>
        </table></div>
        <button type="button" className="acao include" data-add-item
          onClick={() => setItens((l) => [...l, { k: ++seq.current }])}>Adicionar item</button>
        <div className="totais">
          {tela.totals.map((t) => (
            <div key={t.sum_of} className="total"><span>{t.label}</span>
              <output data-soma={t.sum_of}>{brl(itens.reduce((s, i) => s + num(i[t.sum_of]), 0))}</output></div>
          ))}
        </div>
      </fieldset>
      <Acoes acoes={tela.actions} />
    </form>
  );
}
"##;

//! Prova responsiva das telas geradas (Phx Responsive UI): as 20 telas do gabarito de
//! fidelidade desenhadas no Chromium e medidas pelo DOM. `phxclaw ui responsivo`.
//!
//! Larguras: 320 (reflow da WCAG 1.4.10), 390 (celular), o `md` (tablet) e 1280 (o desktop da
//! prova de fidelidade), mais cada ponto de quebra de `breakpoints.json` um pixel antes e
//! no ponto -- a regra que muda no ponto e a que se erra por um.
//!
//! O que se mede, e por que assim:
//! - **rolagem horizontal da pagina** (`scrollWidth - clientWidth`): deve ser 0. Rolagem
//!   DENTRO de uma caixa (`overflow-x:auto`) nao rola a pagina, entao sai em separado
//!   (`rolagem_interna`): tabela com rolagem propria e a excecao bidimensional da WCAG,
//!   mas nao e caber.
//! - **cortados**: peca da tela (rotulo, controle, botao, cabecalho) fora da janela, e
//!   texto que vaza da propria caixa (pelo retangulo do TEXTO, por `Range`).
//! - **sobrepostos**: pares de pecas, nenhuma dentro da outra, cujas caixas se cruzam.
//! - **alvos de toque**: controle com lado menor que 44 px. Caixa de marcar conta junto
//!   com o proprio `<label>`, porque clicar no rotulo marca -- e o que o HTML garante e o
//!   que a WCAG 2.5.5 aceita como alvo.
//! - **foco e ordem**: Tab de teclado de verdade (`Page::tecla_tab`), nao `focus()` por
//!   script. A ordem do Tab na tela tem de ser a do DOM (tau de Kendall = 1), e a VISUAL
//!   tem de seguir a do Tab (sem salto para cima, nem para a esquerda na mesma fila):
//!   reordenar com CSS quebraria a ordem lida do layout v2 sem teste de HTML perceber.
//! - **colunas contra a intencao**: cada secao, na largura do conteiner dela, tem de ter as
//!   colunas que `responsivo::colunas_em` calcula do IR. E o que prova que o CSS compilado
//!   diz o que o IR pediu -- e, no alvo Bootstrap, que o adaptador nao mudou a grade.
//! - **fronteira de conteiner**: o conteiner de cada secao forcado a um pixel antes e no
//!   `min_largura_px` de cada regra; **painel estreito**: a area de conteudo presa em 360 px
//!   numa janela de 1920 (tudo compacto); **redimensionar**: valor digitado e foco
//!   sobrevivem a quatro larguras, sem no novo nem perdido -- responsividade em CSS nao
//!   recria a tela, e o dia em que alguem a fizer em JS este numero acusa.

use crate::fidelidade_ui::{Pagina, faixa, r4};
use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions, Page};
use phxclaw_ui_ir::fidelidade::{self, GABARITO};
use phxclaw_ui_ir::responsivo::{self, bp};
use phxclaw_ui_ir::{App, Screen};
use serde_json::{Value, json};
use std::path::Path;

/// Lado minimo do alvo de toque (WCAG 2.5.5), cobrado na largura de celular.
pub const ALVO_MIN: f64 = 44.0;

/// Largura do celular onde o alvo de toque se cobra.
pub const CELULAR: u32 = 390;

/// Janela larga dos casos de painel e de fronteira de conteiner.
const JANELA_LARGA: (u32, u32) = (1920, 1080);

/// Largura do painel estreito.
const PAINEL: u32 = 360;

/// Uma largura medida. `toque` liga a emulacao de toque (`pointer: coarse` e meta viewport).
#[derive(Debug, Clone, Copy)]
pub struct Tamanho {
    pub largura: u32,
    pub altura: u32,
    pub toque: bool,
}

impl Tamanho {
    /// Abaixo do desktop o aparelho e de toque; do `lg` para cima, ponteiro fino.
    fn de(largura: u32, altura: u32) -> Self {
        Tamanho {
            largura,
            altura,
            toque: largura < bp("lg"),
        }
    }
}

/// As larguras medidas, com o motivo. A altura e a do aparelho. O desktop entra duas vezes:
/// com ponteiro fino (o alvo encolhe para 36 px, densidade de ERP) e com toque (o
/// `@media (pointer:coarse)` devolve os 44 px) -- e o que prova que o alvo de 44 px e
/// cobrado de quem toca, em qualquer largura, e nao de quem usa mouse.
pub fn larguras() -> Vec<(Tamanho, String)> {
    let desktop = crate::fidelidade_ui::JANELA;
    let mut v = vec![
        (Tamanho::de(320, 568), "reflow WCAG 1.4.10".to_string()),
        (Tamanho::de(CELULAR, 844), "celular".into()),
        (Tamanho::de(bp("md"), 1024), "tablet".into()),
        (
            Tamanho::de(desktop.0, desktop.1),
            "desktop da fidelidade".into(),
        ),
        (
            Tamanho {
                toque: true,
                ..Tamanho::de(desktop.0, desktop.1)
            },
            "desktop com toque".into(),
        ),
    ];
    for (nome, n) in responsivo::breakpoints() {
        for w in [n - 1, *n] {
            if !v.iter().any(|(x, _)| x.largura == w) {
                v.push((Tamanho::de(w, 900), format!("fronteira {nome}")));
            }
        }
    }
    v.sort_by_key(|(t, _)| (t.largura, t.toque));
    v
}

/// Qual desenho se mede: o adaptador «phoenix» (HTML nativo) ou o Bootstrap.
#[derive(Debug, Clone)]
pub enum Alvo {
    Html,
    /// Folha `bootstrap.min.css` local: servida pelo loopback na mesma origem da pagina.
    Bootstrap {
        css: Vec<u8>,
    },
}

impl Alvo {
    fn nome(&self) -> &'static str {
        match self {
            Alvo::Html => "html",
            Alvo::Bootstrap { .. } => "bootstrap",
        }
    }
    fn render(&self, app: &App) -> Result<String, String> {
        match self {
            Alvo::Html => Ok(phxclaw_ui_ir::html::render(app)),
            Alvo::Bootstrap { .. } => phxclaw_ui_ir::bootstrap::render(app, CSS_LOCAL),
        }
    }
}

/// Caminho da folha do Bootstrap na pagina da prova.
const CSS_LOCAL: &str = "bootstrap.min.css";

/// Ajudantes comuns: visibilidade (inclusive dentro de caixa de 1 px, o padrao do texto
/// so para leitor de tela), nome legivel da peca e a estrutura de colunas das secoes.
const JS_BASE: &str = r#"const t=document.getElementById(TELA);const de=document.documentElement;
const vis=e=>{const r=e.getBoundingClientRect();const cs=getComputedStyle(e);if(!((r.width>0||r.height>0)&&cs.visibility!=='hidden'&&cs.display!=='none'))return false;
for(let a=e.parentElement;a&&a!==t;a=a.parentElement){const q=a.getBoundingClientRect();if(q.width<=1&&q.height<=1)return false}return true};
const nome=e=>e.tagName.toLowerCase()+(e.id?'#'+e.id:'')+(e.getAttribute('name')?'[name='+e.getAttribute('name')+']':'')+(e.dataset.acao?'[acao='+e.dataset.acao+']':'')+(!e.id&&!e.getAttribute('name')&&e.textContent?'('+e.textContent.trim().slice(0,24)+')':'');
const cont=e=>{const cs=getComputedStyle(e);return e.clientWidth-parseFloat(cs.paddingLeft)-parseFloat(cs.paddingRight)};
const grade=f=>[...f.querySelectorAll('*')].find(x=>getComputedStyle(x).display==='grid'&&x.querySelector('label'));
const colunas=g=>g?getComputedStyle(g).gridTemplateColumns.split(' ').filter(Boolean).length:0;
const conts=g=>{const r={};for(let a=g&&g.parentElement;a;a=a.parentElement){const m=[...a.classList].find(c=>c.startsWith('phx-c-'));
if(m&&!(m.slice(6) in r))r[m.slice(6)]=cont(a)}
if(!('secao' in r))r.secao=g?cont(g.closest('fieldset')||g):0;if(!('tela' in r))r.tela=cont(t);return r};
const alvoDe=(g,nome)=>g&&g.parentElement.closest('.phx-c-'+nome);
const estrutura=()=>({secoes:[...t.querySelectorAll('fieldset')].filter(f=>!f.querySelector('table')).map(f=>{const g=grade(f);
return {colunas:colunas(g),conteineres:conts(g)}}),
tabelas:[...t.querySelectorAll('table')].map(x=>{const r=x.tBodies[0]&&x.tBodies[0].rows[0];return r?getComputedStyle(r).display:'vazia'}),janela:innerWidth});"#;

/// Marca cada peca focavel da tela com `data-k` (a ordem do DOM) e mede o que nao depende
/// do teclado.
const JS_MEDIR: &str = r#"(()=>{JS_BASE
const cw=de.clientWidth;
const pecas=[...t.querySelectorAll('h1,label,input,select,textarea,button,a[href],th,legend,output')].filter(vis);
const txt=e=>{let x0=1e9,y0=1e9,x1=-1e9,y1=-1e9;const w=document.createTreeWalker(e,NodeFilter.SHOW_TEXT);
for(let n=w.nextNode();n;n=w.nextNode()){if(!n.textContent.trim())continue;let ok=true;
for(let a=n.parentElement;a&&a!==e.parentElement;a=a.parentElement){const q=a.getBoundingClientRect();if(q.width<=1&&q.height<=1){ok=false;break}}
if(!ok)continue;const g=document.createRange();g.selectNodeContents(n);const q=g.getBoundingClientRect();
x0=Math.min(x0,q.left);y0=Math.min(y0,q.top);x1=Math.max(x1,q.right);y1=Math.max(y1,q.bottom)}
return x1<x0?{left:0,top:0,right:0,bottom:0,width:0,height:0}:{left:x0,top:y0,right:x1,bottom:y1,width:x1-x0,height:y1-y0}};
const fora=pecas.filter(e=>{const r=e.getBoundingClientRect();return r.right>cw+1||r.left<-1}).map(nome);
const interna=[];pecas.forEach(e=>{const r=e.getBoundingClientRect();let a=e.parentElement;
while(a&&a!==t){const cs=getComputedStyle(a);if(cs.overflowX!=='visible'){const q=a.getBoundingClientRect();if(r.right>q.right+1||r.left<q.left-1){interna.push(nome(e));break}}a=a.parentElement}});
const vaza=pecas.filter(e=>['H1','LABEL','BUTTON','A','TH','LEGEND'].includes(e.tagName)).filter(e=>{const r=e.getBoundingClientRect(),q=txt(e);return q.width>0&&(q.right>r.right+1||q.left<r.left-1)}).map(nome);
const caixa=e=>['LABEL','LEGEND','TH','H1'].includes(e.tagName)?txt(e):e.getBoundingClientRect();
const sob=[];for(let i=0;i<pecas.length;i++)for(let j=i+1;j<pecas.length;j++){const a=pecas[i],b=pecas[j];
if(a.contains(b)||b.contains(a))continue;const p=caixa(a),q=caixa(b);
const w=Math.min(p.right,q.right)-Math.max(p.left,q.left),h=Math.min(p.bottom,q.bottom)-Math.max(p.top,q.top);
if(w>1&&h>1)sob.push(nome(a)+' x '+nome(b))}
const alvo=e=>{const r=e.getBoundingClientRect();let x0=r.left,y0=r.top,x1=r.right,y1=r.bottom;
if(e.type==='checkbox'||e.type==='radio'){for(const l of (e.labels||[])){const q=l.getBoundingClientRect();x0=Math.min(x0,q.left);y0=Math.min(y0,q.top);x1=Math.max(x1,q.right);y1=Math.max(y1,q.bottom)}}
return Math.min(x1-x0,y1-y0)};
const inter='a[href],button,input:not([type=hidden]),select,textarea';
const pequenos=[...t.querySelectorAll(inter)].filter(vis).filter(e=>alvo(e)<ALVO-0.5).map(e=>nome(e)+'='+Math.round(alvo(e)));
const menu=[...document.querySelectorAll('nav a[href]')].filter(vis).filter(e=>alvo(e)<ALVO-0.5).length;
const foc=[...t.querySelectorAll(inter+',[tabindex]')].filter(e=>e.tabIndex>=0&&!e.disabled&&vis(e));
foc.forEach((e,i)=>e.dataset.k=String(i));
return {cw,rolagem:Math.max(0,de.scrollWidth-cw,document.body.scrollWidth-cw),fora,interna,vaza,sob,
alvos:t.querySelectorAll(inter).length,pequenos,menu,focaveis:foc.length,estrutura:estrutura()}})()"#;

/// O que o Tab alcancou agora.
const JS_FOCO: &str = r#"(()=>{const e=document.activeElement;if(!e||e===document.body)return null;
const t=document.getElementById(TELA);const r=e.getBoundingClientRect();const cs=getComputedStyle(e);
const anel=(cs.outlineStyle!=='none'&&parseFloat(cs.outlineWidth)>0)||(cs.boxShadow&&cs.boxShadow!=='none');
return {na_tela:t.contains(e),k:e.dataset.k===undefined?-1:+e.dataset.k,x:r.left+scrollX,y:r.top+scrollY,w:r.width,h:r.height,fv:e.matches(':focus-visible'),anel:!!anel}})()"#;

/// Forca o conteiner de cada secao a cada largura pedida e le as colunas. `PEDIDOS` e
/// `[[indice da secao, largura]...]`; o tamanho vai no content-box, que e o que a container
/// query compara.
const JS_FRONTEIRA: &str = r#"(()=>{JS_BASE
const fs=[...t.querySelectorAll('fieldset')].filter(f=>!f.querySelector('table'));
const r=[];for(const [i,w,nome] of PEDIDOS){const f=fs[i];const g=f&&grade(f);const c=g&&alvoDe(g,nome);
if(!c){r.push(null);continue}
const s=c.getAttribute('style');c.style.boxSizing='content-box';c.style.inlineSize=w+'px';c.style.maxInlineSize='none';
r.push([colunas(g),cont(c)]);if(s===null)c.removeAttribute('style');else c.setAttribute('style',s)}
const tb=[...t.querySelectorAll('.phx-tabela')].filter(x=>x.querySelector('tbody tr'));const tr=[];
for(const w of TABELA){const x=tb[0];if(!x){tr.push(null);continue}const s=x.getAttribute('style');x.style.boxSizing='content-box';x.style.inlineSize=w+'px';
tr.push(getComputedStyle(x.querySelector('tbody tr')).display);if(s===null)x.removeAttribute('style');else x.setAttribute('style',s)}
return {secoes:r,tabela:tr}})()"#;

/// Prende a area de conteudo em `PAINEL` px e le a estrutura.
const JS_PAINEL: &str = r#"(()=>{JS_BASE
const m=t.closest('main');m.style.maxInlineSize=PAINEL+'px';m.style.boxSizing='border-box';
const e=estrutura();e.rolagem=Math.max(0,de.scrollWidth-de.clientWidth);return e})()"#;

/// Marca os nos da tela, digita num campo e foca: a prova de que redimensionar nao recria.
const JS_MARCAR: &str = r#"(()=>{const t=document.getElementById(TELA);const n=[...t.querySelectorAll('*')];n.forEach(e=>e.__phx=1);
const i=[...t.querySelectorAll('input[type=text],input:not([type]),textarea')].find(e=>!e.readOnly);if(i){i.value='Blumenau 123';i.focus()}
return {nos:n.length,campo:i?i.id:null}})()"#;
const JS_CONFERIR: &str = r#"(()=>{const t=document.getElementById(TELA);const n=[...t.querySelectorAll('*')];
const i=CAMPO?document.getElementById(CAMPO):null;
return {nos:n.length,novos:n.filter(e=>!e.__phx).length,foco:!!i&&document.activeElement===i,valor:!!i&&i.value==='Blumenau 123'}})()"#;

fn js(s: &str, tela: &str) -> String {
    s.replace("JS_BASE", JS_BASE)
        .replace("TELA", &format!("{tela:?}"))
        .replace("ALVO", &ALVO_MIN.to_string())
}

/// Uma tela medida numa largura.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Medida {
    pub tela: String,
    pub largura: u32,
    pub toque: bool,
    pub rolagem_px: f64,
    pub fora_da_janela: Vec<String>,
    pub rolagem_interna: Vec<String>,
    pub texto_vazado: Vec<String>,
    pub sobrepostos: Vec<String>,
    pub alvos: usize,
    pub alvos_pequenos: Vec<String>,
    pub menu_alvos_pequenos: usize,
    pub focaveis: usize,
    /// Pecas da tela que o Tab alcancou.
    pub tab_alcancados: usize,
    pub tab_sem_anel: usize,
    /// Tau de Kendall entre a ordem do Tab e a do DOM (1 = igual).
    pub tab_tau: Option<f64>,
    /// Passos do Tab que vao para cima, ou para a esquerda na mesma fila.
    pub saltos_visuais: usize,
    /// Colunas medidas por secao, e o que o IR pede na largura medida do conteiner.
    pub colunas: Vec<u32>,
    pub colunas_esperadas: Vec<u32>,
    /// `table-row` (tabela) ou outro (cartoes), por tabela da tela.
    pub tabelas: Vec<String>,
}

fn lista(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

fn erro(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A tela do gabarito aberta numa largura, com as duas linhas de item da fidelidade.
async fn abrir(
    navegador: &Browser,
    pagina: &Pagina,
    app: &App,
    tela: &str,
    t: Tamanho,
) -> Result<Page, String> {
    let p = navegador.new_page().await.map_err(erro)?;
    p.emular_tela(t.largura, t.altura, t.toque)
        .await
        .map_err(erro)?;
    p.goto(&format!("{}/{tela}#{tela}", pagina.base))
        .await
        .map_err(erro)?;
    let detalhe = app
        .screens
        .iter()
        .any(|s| matches!(s, Screen::MasterDetail { id, .. } if id == tela));
    if detalhe {
        // grade vazia nao prova cartao: as mesmas duas linhas da prova de fidelidade
        for _ in 0..2 {
            p.eval(&format!(
                "document.querySelector('#{tela} [data-add-item]').click()"
            ))
            .await
            .map_err(erro)?;
        }
    }
    Ok(p)
}

/// Colunas esperadas pelo IR para as secoes, dadas as larguras medidas dos conteineres.
fn esperadas(app: &App, tela: &str, est: &Value) -> (Vec<u32>, Vec<u32>) {
    let janela = est["janela"].as_f64().unwrap_or(0.0);
    let secoes: Vec<_> = app
        .com_layout()
        .screens
        .iter()
        .find(|s| s.id() == tela)
        .map(|t| {
            responsivo::secoes_da_tela(t)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let medidas = est["secoes"].as_array().cloned().unwrap_or_default();
    let mut med = vec![];
    let mut esp = vec![];
    for (s, m) in secoes.iter().zip(medidas.iter()) {
        let (l, _) = responsivo::intencao(s);
        // a largura do conteiner que a regra CONSULTA (o alvo), nao a do mais proximo
        let alvo = l
            .responsivo
            .as_ref()
            .and_then(|r| r.alvo.clone())
            .unwrap_or_else(|| responsivo::CONTEINER_SECAO.into());
        let c = m["conteineres"][alvo.as_str()].as_f64().unwrap_or(0.0);
        med.push(m["colunas"].as_u64().unwrap_or(0) as u32);
        esp.push(responsivo::colunas_em(&l, c, janela));
    }
    (med, esp)
}

async fn medir_tela(
    navegador: &Browser,
    pagina: &Pagina,
    app: &App,
    tela: &str,
    tamanho: Tamanho,
    png: Option<&Path>,
) -> Result<Medida, String> {
    let p = abrir(navegador, pagina, app, tela, tamanho).await?;
    let v = p.eval(&js(JS_MEDIR, tela)).await.map_err(erro)?;
    let focaveis = v["focaveis"].as_u64().unwrap_or(0) as usize;
    let (colunas, colunas_esperadas) = esperadas(app, tela, &v["estrutura"]);
    let mut m = Medida {
        tela: tela.into(),
        largura: tamanho.largura,
        toque: tamanho.toque,
        rolagem_px: v["rolagem"].as_f64().unwrap_or(0.0),
        fora_da_janela: lista(&v["fora"]),
        rolagem_interna: lista(&v["interna"]),
        texto_vazado: lista(&v["vaza"]),
        sobrepostos: lista(&v["sob"]),
        alvos: v["alvos"].as_u64().unwrap_or(0) as usize,
        alvos_pequenos: lista(&v["pequenos"]),
        menu_alvos_pequenos: v["menu"].as_u64().unwrap_or(0) as usize,
        focaveis,
        colunas,
        colunas_esperadas,
        tabelas: lista(&v["estrutura"]["tabelas"]),
        ..Default::default()
    };
    // o Tab parte do comeco do documento: o "Adicionar item" deixou o foco na ultima linha,
    // e tabular dali mediria so o fim da tela. Focar a primeira peca focavel (o menu) poe
    // o ponto de partida da navegacao sequencial antes da tela.
    p.eval("(()=>{const a=document.querySelector('a[href],button,input,select,textarea');if(a)a.focus()})()")
        .await
        .map_err(erro)?;
    // Tab ate sair da tela: o menu vem antes; o limite cobre menu + tela com folga
    let mut seq: Vec<Value> = vec![];
    let mut entrou = false;
    for _ in 0..(focaveis + 80) {
        p.tecla_tab().await.map_err(erro)?;
        let f = p.eval(&js(JS_FOCO, tela)).await.map_err(erro)?;
        if f["na_tela"].as_bool() == Some(true) {
            entrou = true;
            seq.push(f);
        } else if entrou {
            break;
        }
    }
    m.tab_alcancados = seq.len();
    m.tab_sem_anel = seq
        .iter()
        .filter(|f| f["anel"].as_bool() != Some(true) || f["fv"].as_bool() != Some(true))
        .count();
    let ks: Vec<usize> = seq
        .iter()
        .filter_map(|f| f["k"].as_i64().filter(|k| *k >= 0).map(|k| k as usize))
        .collect();
    let mut ordenado = ks.clone();
    ordenado.sort_unstable();
    // posicao de cada alcancado na ordem do DOM, contra a ordem em que o Tab o alcancou
    let rank: Vec<usize> = ks
        .iter()
        .map(|k| ordenado.iter().position(|x| x == k).unwrap_or(0))
        .collect();
    let dom: Vec<usize> = (0..rank.len()).collect();
    m.tab_tau = fidelidade::kendall_tau(&dom, &rank).map(r4);
    let n = |f: &Value, k: &str| f[k].as_f64().unwrap_or(0.0);
    m.saltos_visuais = seq
        .windows(2)
        .filter(|w| {
            let (a, b) = (&w[0], &w[1]);
            let (ay, ah, by, bh) = (n(a, "y"), n(a, "h"), n(b, "y"), n(b, "h"));
            let acima = by + bh <= ay + 1.0;
            let mesma_fila = by < ay + ah - 1.0 && ay < by + bh - 1.0;
            acima || (mesma_fila && n(b, "x") + 1.0 < n(a, "x"))
        })
        .count();
    if let Some(png) = png {
        let bytes = p.screenshot_png().await.map_err(erro)?;
        std::fs::write(png, bytes).map_err(erro)?;
    }
    let _ = p.close().await;
    Ok(m)
}

/// Fronteiras de conteiner, painel estreito e redimensionar, numa tela.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Casos {
    pub tela: String,
    /// (secao, largura do conteiner, colunas medidas, colunas esperadas)
    pub fronteiras: Vec<(usize, u32, u32, u32)>,
    pub fronteiras_sem_conteiner: usize,
    /// (largura do conteiner da tabela, display da linha): cartao abaixo de sm, tabela no sm
    pub fronteira_tabela: Vec<(u32, String)>,
    pub painel_colunas: Vec<u32>,
    pub painel_tabelas: Vec<String>,
    pub painel_rolagem_px: f64,
    pub redimensionar_nos_novos: usize,
    pub redimensionar_nos_perdidos: usize,
    pub redimensionar_foco: bool,
    pub redimensionar_valor: bool,
}

async fn medir_casos(
    navegador: &Browser,
    pagina: &Pagina,
    app: &App,
    tela: &str,
) -> Result<Casos, String> {
    let mut c = Casos {
        tela: tela.into(),
        ..Default::default()
    };
    let secoes: Vec<_> = app
        .com_layout()
        .screens
        .iter()
        .find(|s| s.id() == tela)
        .map(|t| {
            responsivo::secoes_da_tela(t)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // fronteiras: cada regra de cada secao, um pixel antes e no ponto, na janela larga
    let mut pedidos: Vec<(usize, u32, String)> = vec![];
    for (i, s) in secoes.iter().enumerate() {
        let (l, _) = responsivo::intencao(s);
        // regra de janela ja e medida pelas larguras de fronteira; aqui, a de conteiner
        let Some(r) = l
            .responsivo
            .as_ref()
            .filter(|r| r.base == phxclaw_ui_ir::BaseResponsiva::Conteiner)
        else {
            continue;
        };
        let alvo = r.alvo.clone().unwrap_or_default();
        for g in &r.regras {
            pedidos.push((i, g.min_largura_px - 1, alvo.clone()));
            pedidos.push((i, g.min_largura_px, alvo.clone()));
        }
    }
    let sm = bp("sm");
    let p = abrir(
        navegador,
        pagina,
        app,
        tela,
        Tamanho::de(JANELA_LARGA.0, JANELA_LARGA.1),
    )
    .await?;
    let v = p
        .eval(
            &js(JS_FRONTEIRA, tela)
                .replace("PEDIDOS", &json!(pedidos).to_string())
                .replace("TABELA", &json!([sm - 1, sm]).to_string()),
        )
        .await
        .map_err(erro)?;
    for ((i, w, _), r) in pedidos
        .iter()
        .zip(v["secoes"].as_array().into_iter().flatten())
    {
        let Some(r) = r.as_array() else {
            c.fronteiras_sem_conteiner += 1;
            continue;
        };
        let (l, _) = responsivo::intencao(&secoes[*i]);
        let largura = r.get(1).and_then(Value::as_f64).unwrap_or(0.0);
        c.fronteiras.push((
            *i,
            *w,
            r.first().and_then(Value::as_u64).unwrap_or(0) as u32,
            responsivo::colunas_em(&l, largura, JANELA_LARGA.0 as f64),
        ));
    }
    for (w, r) in [sm - 1, sm]
        .iter()
        .zip(v["tabela"].as_array().into_iter().flatten())
    {
        if let Some(d) = r.as_str() {
            c.fronteira_tabela.push((*w, d.into()));
        }
    }
    // painel estreito na janela larga
    let e = p
        .eval(&js(JS_PAINEL, tela).replace("PAINEL", &PAINEL.to_string()))
        .await
        .map_err(erro)?;
    c.painel_colunas = e["secoes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| s["colunas"].as_u64().unwrap_or(0) as u32)
        .collect();
    c.painel_tabelas = lista(&e["tabelas"]);
    c.painel_rolagem_px = e["rolagem"].as_f64().unwrap_or(0.0);
    let _ = p.close().await;
    // redimensionar: celular -> desktop -> tablet -> reflow, na MESMA pagina
    let p = abrir(navegador, pagina, app, tela, Tamanho::de(CELULAR, 844)).await?;
    let m = p.eval(&js(JS_MARCAR, tela)).await.map_err(erro)?;
    let nos = m["nos"].as_u64().unwrap_or(0);
    let campo = m["campo"]
        .as_str()
        .map_or("null".to_string(), |s| format!("{s:?}"));
    let (mut foco, mut valor) = (true, true);
    let desktop = crate::fidelidade_ui::JANELA;
    for (w, h) in [desktop, (bp("md"), 1024), (320, 568)] {
        let t = Tamanho::de(w, h);
        p.emular_tela(t.largura, t.altura, t.toque)
            .await
            .map_err(erro)?;
        let r = p
            .eval(&js(JS_CONFERIR, tela).replace("CAMPO", &campo))
            .await
            .map_err(erro)?;
        c.redimensionar_nos_novos += r["novos"].as_u64().unwrap_or(0) as usize;
        c.redimensionar_nos_perdidos += nos.saturating_sub(r["nos"].as_u64().unwrap_or(0)) as usize;
        foco &= r["foco"].as_bool() == Some(true);
        valor &= r["valor"].as_bool() == Some(true);
    }
    c.redimensionar_foco = foco;
    c.redimensionar_valor = valor;
    let _ = p.close().await;
    Ok(c)
}

/// Mede `telas` telas do gabarito (so as nomeadas em `so`, quando nao vazio). No alvo
/// Bootstrap, a MESMA tela tambem se desenha pelo adaptador «phoenix», e as colunas das duas
/// se comparam largura a largura.
pub async fn medir(
    alvo: &Alvo,
    telas: usize,
    so: &[&str],
    capturas: Option<&Path>,
) -> Result<Value, String> {
    if phxclaw_browser::find_chromium().is_none() {
        return Err(format!(
            "Chromium nao encontrado; defina {}",
            phxclaw_browser::CHROMIUM_ENV
        ));
    }
    let pagina = Pagina::subir()?;
    if let Alvo::Bootstrap { css } = alvo {
        *pagina.extra.lock().unwrap_or_else(|e| e.into_inner()) = Some((
            format!("/{CSS_LOCAL}"),
            "text/css; charset=utf-8",
            css.clone(),
        ));
    }
    let navegador = Browser::launch(LaunchOptions {
        window_size: crate::fidelidade_ui::JANELA,
        ..LaunchOptions::with_policy(BrowserPolicy::only([pagina.base.clone()]))
    })
    .await
    .map_err(erro)?;
    if let Some(c) = capturas {
        std::fs::create_dir_all(c).map_err(erro)?;
    }
    let ls = larguras();
    let mut medidas: Vec<Medida> = vec![];
    let mut casos: Vec<Casos> = vec![];
    let mut estrutura_diferente: Vec<String> = vec![];
    let mut estrutura_conferida = 0usize;
    for (k, g) in GABARITO.iter().take(telas).enumerate() {
        if !so.is_empty() && !so.contains(&g.nome) {
            continue;
        }
        let (app, _) = phxclaw_ui_ir::from_sql(g.nome, g.sql);
        let tela = fidelidade::tela_de(&app, g.tabela)
            .ok_or_else(|| format!("{}: sem tela de edicao para {}", g.nome, g.tabela))?;
        *pagina.html.lock().unwrap_or_else(|e| e.into_inner()) = alvo.render(&app)?;
        let mut desta: Vec<Medida> = vec![];
        for (t, _) in &ls {
            let png = capturas.map(|c| {
                c.join(format!(
                    "{:02}-{}-{}{}.png",
                    k + 1,
                    g.nome,
                    t.largura,
                    if t.toque { "-toque" } else { "" }
                ))
            });
            desta.push(medir_tela(&navegador, &pagina, &app, &tela, *t, png.as_deref()).await?);
        }
        casos.push(medir_casos(&navegador, &pagina, &app, &tela).await?);
        if matches!(alvo, Alvo::Bootstrap { .. }) {
            *pagina.html.lock().unwrap_or_else(|e| e.into_inner()) = Alvo::Html.render(&app)?;
            for (m, (t, _)) in desta.iter().zip(&ls) {
                let w = t.largura;
                let p = abrir(&navegador, &pagina, &app, &tela, *t).await?;
                let v = p
                    .eval(&js("(()=>{JS_BASE return estrutura()})()", &tela))
                    .await
                    .map_err(erro)?;
                let _ = p.close().await;
                let (cols, _) = esperadas(&app, &tela, &v);
                let tabs = lista(&v["tabelas"]);
                estrutura_conferida += 1;
                if cols != m.colunas || tabs != m.tabelas {
                    estrutura_diferente.push(format!(
                        "{tela} a {w}px: phoenix {cols:?} {tabs:?}, bootstrap {:?} {:?}",
                        m.colunas, m.tabelas
                    ));
                }
            }
        }
        medidas.extend(desta);
    }
    let _ = navegador.close().await;
    let por_largura: Vec<Value> = ls
        .iter()
        .map(|(t, motivo)| agregado(t, motivo, &medidas))
        .collect();
    let comparacao = if matches!(alvo, Alvo::Bootstrap { .. }) {
        json!({"conferidas": estrutura_conferida, "diferentes": estrutura_diferente.len(),
               "exemplos": estrutura_diferente.iter().take(6).collect::<Vec<_>>()})
    } else {
        json!({"nao_medido": "so no alvo bootstrap (compara com o adaptador phoenix)"})
    };
    Ok(json!({
        "data": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "comando": format!("phxclaw ui responsivo --alvo {} --telas {telas}", alvo.nome()),
        "alvo": alvo.nome(),
        "breakpoints": responsivo::breakpoints().iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "regra": "rolagem da pagina, fora, vazado e sobrepostos = 0 em toda largura; alvos >= 44 px a 390; \
    Tab com anel visivel, na ordem do DOM (tau 1), sem salto visual; colunas = as do IR; painel de 360 px compacto; \
    redimensionar sem no novo, com foco e valor",
        "por_largura": por_largura,
        "casos": agregado_casos(&casos),
        "mesma_estrutura_que_phoenix": comparacao,
        "telas": medidas,
        "casos_por_tela": casos,
    }))
}

fn agregado_casos(cs: &[Casos]) -> Value {
    let front: Vec<_> = cs.iter().flat_map(|c| &c.fronteiras).collect();
    let tab: Vec<_> = cs.iter().flat_map(|c| &c.fronteira_tabela).collect();
    let sm = bp("sm");
    // abaixo de sm, cartao (linha que nao e `table-row`); no sm, tabela
    let tab_erradas = tab
        .iter()
        .filter(|(w, d)| (*w < sm) != (d != "table-row"))
        .count();
    let painel_cols: Vec<u32> = cs.iter().flat_map(|c| c.painel_colunas.clone()).collect();
    let painel_tabs: Vec<&String> = cs.iter().flat_map(|c| &c.painel_tabelas).collect();
    json!({
        "fronteiras_de_conteiner": {
            "conferidas": front.len(),
            "divergentes": front.iter().filter(|(_, _, m, e)| m != e).count(),
            "sem_conteiner": cs.iter().map(|c| c.fronteiras_sem_conteiner).sum::<usize>(),
            "exemplos": front.iter().filter(|(_, _, m, e)| m != e).take(6)
                .map(|(i, w, m, e)| format!("secao {i} a {w}px: {m} colunas, IR pede {e}")).collect::<Vec<_>>(),
        },
        "fronteira_da_tabela": {"conferidas": tab.len(), "erradas": tab_erradas},
        "painel_360_em_1920": {
            "secoes": painel_cols.len(),
            "com_uma_coluna": painel_cols.iter().filter(|c| **c == 1).count(),
            "tabelas": painel_tabs.len(),
            "em_cartoes": painel_tabs.iter().filter(|d| d.as_str() != "table-row" && d.as_str() != "vazia").count(),
            "rolagem_px_max": cs.iter().map(|c| c.painel_rolagem_px).fold(0.0, f64::max),
        },
        "redimensionar": {
            "telas": cs.len(),
            "nos_novos": cs.iter().map(|c| c.redimensionar_nos_novos).sum::<usize>(),
            "nos_perdidos": cs.iter().map(|c| c.redimensionar_nos_perdidos).sum::<usize>(),
            "foco_preservado": cs.iter().filter(|c| c.redimensionar_foco).count(),
            "valor_preservado": cs.iter().filter(|c| c.redimensionar_valor).count(),
        },
    })
}

fn agregado(t: &Tamanho, motivo: &str, ms: &[Medida]) -> Value {
    let largura = t.largura;
    let v: Vec<&Medida> = ms
        .iter()
        .filter(|m| m.largura == largura && m.toque == t.toque)
        .collect();
    let soma = |f: &dyn Fn(&Medida) -> usize| v.iter().map(|m| f(m)).sum::<usize>();
    let exemplos = |f: &dyn Fn(&Medida) -> &Vec<String>| {
        v.iter()
            .flat_map(|m| f(m).iter().map(move |x| format!("{}: {x}", m.tela)))
            .take(6)
            .collect::<Vec<_>>()
    };
    let secoes = soma(&|m| m.colunas.len());
    let divergentes = soma(&|m| {
        m.colunas
            .iter()
            .zip(&m.colunas_esperadas)
            .filter(|(a, b)| a != b)
            .count()
    });
    json!({
        "largura": largura,
        "toque": t.toque,
        "motivo": motivo,
        "telas_n": v.len(),
        "telas_com_rolagem": v.iter().filter(|m| m.rolagem_px > 0.0).count(),
        "rolagem_px": faixa(v.iter().map(|m| Some(m.rolagem_px))),
        "fora_da_janela": soma(&|m| m.fora_da_janela.len()),
        "rolagem_interna": soma(&|m| m.rolagem_interna.len()),
        "texto_vazado": soma(&|m| m.texto_vazado.len()),
        "sobrepostos": soma(&|m| m.sobrepostos.len()),
        "alvos": soma(&|m| m.alvos),
        "alvos_menores_que_44": soma(&|m| m.alvos_pequenos.len()),
        "menu_alvos_menores_que_44": soma(&|m| m.menu_alvos_pequenos),
        "focaveis": soma(&|m| m.focaveis),
        "tab_alcancados": soma(&|m| m.tab_alcancados),
        "tab_sem_anel": soma(&|m| m.tab_sem_anel),
        "tab_tau": faixa(v.iter().map(|m| m.tab_tau)),
        "saltos_visuais": soma(&|m| m.saltos_visuais),
        "secoes": secoes,
        "colunas_fora_do_ir": divergentes,
        "tabelas_em_cartoes": soma(&|m| m.tabelas.iter().filter(|d| d.as_str() != "table-row" && d.as_str() != "vazia").count()),
        "tabelas": soma(&|m| m.tabelas.iter().filter(|d| d.as_str() != "vazia").count()),
        "exemplos": {
            "fora_da_janela": exemplos(&|m| &m.fora_da_janela),
            "rolagem_interna": exemplos(&|m| &m.rolagem_interna),
            "texto_vazado": exemplos(&|m| &m.texto_vazado),
            "sobrepostos": exemplos(&|m| &m.sobrepostos),
            "alvos_menores_que_44": exemplos(&|m| &m.alvos_pequenos),
        },
    })
}

/// Tabela curta para o terminal: uma linha por largura e os casos.
pub fn tabela(v: &Value) -> String {
    let mut s = format!(
        "alvo {} ({}):\n  largura rol(telas/max) fora interna vazado sobrep alvo<44 menu<44 tab(alc/foc) sem-anel tau-min saltos col-fora-IR cartoes\n",
        v["alvo"].as_str().unwrap_or(""),
        v["data"].as_str().unwrap_or("")
    );
    for a in v["por_largura"].as_array().into_iter().flatten() {
        s.push_str(&format!(
            "  {:>5}{} {:>4}/{:<6} {:>4} {:>7} {:>6} {:>6} {:>7} {:>7} {:>5}/{:<5} {:>8} {:>7} {:>6} {:>5}/{:<5} {:>3}/{:<3}\n",
            a["largura"],
            if a["toque"].as_bool() == Some(true) { "t" } else { " " },
            a["telas_com_rolagem"],
            a["rolagem_px"]["max"],
            a["fora_da_janela"],
            a["rolagem_interna"],
            a["texto_vazado"],
            a["sobrepostos"],
            a["alvos_menores_que_44"],
            a["menu_alvos_menores_que_44"],
            a["tab_alcancados"],
            a["focaveis"],
            a["tab_sem_anel"],
            a["tab_tau"]["min"],
            a["saltos_visuais"],
            a["colunas_fora_do_ir"],
            a["secoes"],
            a["tabelas_em_cartoes"],
            a["tabelas"],
        ));
    }
    s.push_str(&format!("  casos {}\n", v["casos"]));
    s.push_str(&format!(
        "  mesma estrutura que phoenix {}\n",
        v["mesma_estrutura_que_phoenix"]
    ));
    s
}

// ------------------------------------------------------------------ PHX JSON do dono

/// As larguras de janela do teste do exemplo do dono (`tests/test_ui.py`). Quatro delas
/// sao os pontos `sm`, `md` e `lg` (e um antes deste): saem de `breakpoints.json`.
pub fn larguras_phx() -> Vec<u32> {
    vec![
        320,
        360,
        CELULAR,
        bp("sm"),
        bp("md"),
        bp("lg") - 1,
        bp("lg"),
        1280,
        1600,
        JANELA_LARGA.0,
    ]
}

/// Le a casa da tela do painel: rolagem, largura do conteiner, colunas, ids dos cartoes e
/// se menu e conteudo estao lado a lado. `ALVO` e o nome do conteiner do painel.
const JS_PHX: &str = r#"(()=>{const de=document.documentElement;const p=document.querySelector('.phx-c-ALVO');
const g=p&&[...p.querySelectorAll('*')].find(x=>getComputedStyle(x).display==='grid');
const cs=p&&getComputedStyle(p);const n=document.querySelector('nav'),m=document.querySelector('main');
const a=n.getBoundingClientRect(),b=m.getBoundingClientRect();
return {rolagem:Math.max(0,de.scrollWidth-de.clientWidth),janela:innerWidth,
conteiner:p?p.clientWidth-parseFloat(cs.paddingLeft)-parseFloat(cs.paddingRight):0,
colunas:g?getComputedStyle(g).gridTemplateColumns.split(' ').filter(Boolean).length:0,
ids:[...document.querySelectorAll('.phx-cartao')].map(x=>x.dataset.uuid),
mesmos:[...document.querySelectorAll('.phx-cartao')].every(x=>x.__phx===1),
lado_a_lado:a.right<=b.left+1&&a.top<b.bottom}})()"#;

/// Prende a largura do painel (como o `Testar largura do painel` do exemplo).
const JS_PHX_LARGURA: &str = r#"(()=>{const p=document.querySelector('.phx-c-ALVO');p.style.boxSizing='content-box';
p.style.inlineSize=LARGURA;p.style.maxInlineSize='none';return true})()"#;

/// Placar no formato do teste do dono: nome, passou, detalhe.
fn conferir(placar: &mut Vec<Value>, nome: String, ok: bool, detalhe: Value) {
    placar.push(json!({"nome": nome, "passou": ok, "detalhe": detalhe}));
}

/// Aceitacao do PHX JSON do dono nos dois adaptadores: a parte das 72 verificacoes do
/// `tests/test_ui.py` que se aplica a uma tela gerada (sem simulacao, sem configuracao,
/// sem dialogo de JSON -- isso e da aplicacao de demonstracao, nao do layout). Com
/// `exemplo`, o `index.html` do dono e medido nas mesmas larguras e as colunas comparadas.
pub async fn medir_phx(
    phx: &str,
    css_bootstrap: Option<Vec<u8>>,
    exemplo: Option<String>,
) -> Result<Value, String> {
    if phxclaw_browser::find_chromium().is_none() {
        return Err(format!(
            "Chromium nao encontrado; defina {}",
            phxclaw_browser::CHROMIUM_ENV
        ));
    }
    let (env, app) = phxclaw_ui_ir::phx_json::ler(phx)?;
    let ida_e_volta = phxclaw_ui_ir::phx_json::escrever(&env, &app)? == phx;
    let Some(Screen::Painel {
        id: tela,
        conteiner,
        colecao,
        ..
    }) = app.screens.first()
    else {
        return Err("PHX JSON sem painel".into());
    };
    let quebra = app.quebra_da_casca_px.unwrap_or_else(|| bp("md"));
    let ids: Vec<String> = colecao.itens.iter().map(|k| k.id.clone()).collect();
    let pagina = Pagina::subir()?;
    if let Some(css) = &css_bootstrap {
        *pagina.extra.lock().unwrap_or_else(|e| e.into_inner()) = Some((
            format!("/{CSS_LOCAL}"),
            "text/css; charset=utf-8",
            css.clone(),
        ));
    }
    let navegador = Browser::launch(LaunchOptions {
        window_size: crate::fidelidade_ui::JANELA,
        ..LaunchOptions::with_policy(BrowserPolicy::only([pagina.base.clone()]))
    })
    .await
    .map_err(erro)?;
    let mut adaptadores: Vec<(&str, String)> = vec![("phoenix", phxclaw_ui_ir::html::render(&app))];
    if css_bootstrap.is_some() {
        adaptadores.push((
            "bootstrap",
            phxclaw_ui_ir::bootstrap::render_com_versao(&app, CSS_LOCAL, &env.versao_do_adaptador)?,
        ));
    }
    let js_phx = JS_PHX.replace("ALVO", conteiner);
    let mut placar: Vec<Value> = vec![];
    conferir(
        &mut placar,
        "PHX JSON: ida e volta identica byte a byte".into(),
        ida_e_volta,
        json!(phx.len()),
    );
    let mut colunas_nossas: Vec<(String, u32, u32, f64)> = vec![];
    for (nome, html) in &adaptadores {
        *pagina.html.lock().unwrap_or_else(|e| e.into_inner()) = html.clone();
        let p = navegador.new_page().await.map_err(erro)?;
        p.emular_tela(1600, 1100, false).await.map_err(erro)?;
        p.goto(&format!("{}/{tela}#{tela}", pagina.base))
            .await
            .map_err(erro)?;
        let v0 = p.eval(&js_phx).await.map_err(erro)?;
        conferir(
            &mut placar,
            format!("{nome}: quatro cartoes do JSON, com o UUID de cada um"),
            v0["ids"] == json!(ids),
            v0["ids"].clone(),
        );
        // marca os cartoes: redimensionar tem de devolver os MESMOS nos
        p.eval("document.querySelectorAll('.phx-cartao').forEach(x=>x.__phx=1)")
            .await
            .map_err(erro)?;
        for w in larguras_phx() {
            p.emular_tela(w, 1000, w < bp("lg")).await.map_err(erro)?;
            let v = p.eval(&js_phx).await.map_err(erro)?;
            let c = v["conteiner"].as_f64().unwrap_or(0.0);
            let col = v["colunas"].as_u64().unwrap_or(0) as u32;
            let esperado = responsivo::colunas_em(&colecao.layout, c, w as f64);
            colunas_nossas.push((nome.to_string(), w, col, c));
            conferir(
                &mut placar,
                format!("{nome}: sem rolagem horizontal da pagina a {w}px"),
                v["rolagem"].as_f64() == Some(0.0),
                v["rolagem"].clone(),
            );
            conferir(
                &mut placar,
                format!("{nome}: colunas do conteiner a {w}px"),
                col == esperado,
                json!({"colunas": col, "esperado": esperado, "conteiner_px": c}),
            );
            conferir(
                &mut placar,
                format!("{nome}: identidades e nos preservados a {w}px"),
                v["ids"] == json!(ids) && v["mesmos"].as_bool() == Some(true),
                json!({"mesmos_nos": v["mesmos"]}),
            );
            conferir(
                &mut placar,
                format!("{nome}: casca a {w}px (lado a lado a partir de {quebra})"),
                v["lado_a_lado"].as_bool() == Some(w >= quebra),
                v["lado_a_lado"].clone(),
            );
        }
        p.emular_tela(1600, 1100, false).await.map_err(erro)?;
        let painel = |l: &str| {
            JS_PHX_LARGURA
                .replace("ALVO", conteiner)
                .replace("LARGURA", l)
        };
        for (l, esperado) in [("'1180px'", 4), ("'720px'", 2), ("'360px'", 1), ("''", 4)] {
            p.eval(&painel(l)).await.map_err(erro)?;
            let v = p.eval(&js_phx).await.map_err(erro)?;
            conferir(
                &mut placar,
                format!(
                    "{nome}: painel {} numa janela de 1600: {esperado} colunas",
                    l.trim_matches('\'')
                ),
                v["colunas"].as_u64() == Some(esperado) && v["janela"].as_u64() == Some(1600),
                v.clone(),
            );
        }
        for (w, esperado) in [
            (639, 1),
            (640, 2),
            (641, 2),
            (1119, 2),
            (1120, 4),
            (1121, 4),
        ] {
            p.eval(&painel(&format!("'{w}px'"))).await.map_err(erro)?;
            let v = p.eval(&js_phx).await.map_err(erro)?;
            conferir(
                &mut placar,
                format!("{nome}: fronteira exata do conteiner {w}px"),
                v["colunas"].as_u64() == Some(esperado),
                json!({"colunas": v["colunas"], "esperado": esperado}),
            );
        }
        // o painel de 360 px numa janela de 1920
        p.emular_tela(JANELA_LARGA.0, JANELA_LARGA.1, false)
            .await
            .map_err(erro)?;
        p.eval(&painel("'360px'")).await.map_err(erro)?;
        let v = p.eval(&js_phx).await.map_err(erro)?;
        conferir(
            &mut placar,
            format!("{nome}: painel de 360 px numa janela de 1920: 1 coluna"),
            v["colunas"].as_u64() == Some(1) && v["rolagem"].as_f64() == Some(0.0),
            v.clone(),
        );
        let _ = p.close().await;
    }
    // o exemplo do dono nas mesmas larguras: as colunas que ELE da
    let mut comparacao = json!({"nao_medido": "sem --exemplo"});
    if let Some(ex) = exemplo {
        *pagina.html.lock().unwrap_or_else(|e| e.into_inner()) = ex;
        let p = navegador.new_page().await.map_err(erro)?;
        p.emular_tela(1600, 1100, false).await.map_err(erro)?;
        p.goto(&format!("{}/exemplo", pagina.base))
            .await
            .map_err(erro)?;
        let mut linhas = vec![];
        let mut iguais = 0;
        for w in larguras_phx() {
            p.emular_tela(w, 1000, w < bp("lg")).await.map_err(erro)?;
            let s = p
                .eval("window.PhxDemo?PhxDemo.getSnapshot():null")
                .await
                .map_err(erro)?;
            let deles = s["columns"].as_u64().unwrap_or(0) as u32;
            let nossas: Vec<u32> = colunas_nossas
                .iter()
                .filter(|(_, x, _, _)| *x == w)
                .map(|(_, _, c, _)| *c)
                .collect();
            if nossas.iter().all(|c| *c == deles) {
                iguais += 1;
            }
            linhas.push(json!({"largura": w, "exemplo": deles, "exemplo_conteiner_px": s["containerWidth"],
                "nossas_por_adaptador": nossas,
                "nossos_conteineres_px": colunas_nossas.iter().filter(|(_, x, _, _)| *x == w).map(|(_, _, _, c)| *c).collect::<Vec<_>>()}));
        }
        let _ = p.close().await;
        comparacao = json!({"larguras": larguras_phx().len(), "iguais": iguais, "linhas": linhas});
    }
    let _ = navegador.close().await;
    let passou = placar
        .iter()
        .filter(|x| x["passou"].as_bool() == Some(true))
        .count();
    Ok(json!({
        "data": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "comando": "phxclaw ui responsivo --phx ARQ.phx.json [--bootstrap-css ARQ] [--exemplo index.html]",
        "adaptadores": adaptadores.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        "versao_bootstrap": env.versao_do_adaptador,
        "fora": "simulacao, configuracoes, dialogos de JSON e de agente, armazenamento local e eventos: sao da aplicacao de demonstracao, nao do layout gerado",
        "passou": passou,
        "total": placar.len(),
        "verificacoes": placar,
        "colunas_contra_o_exemplo": comparacao,
    }))
}

/// Placar curto para o terminal.
pub fn tabela_phx(v: &Value) -> String {
    let mut s = format!(
        "PHX JSON ({}): {}/{} verificacoes\n",
        v["data"].as_str().unwrap_or(""),
        v["passou"],
        v["total"]
    );
    for x in v["verificacoes"].as_array().into_iter().flatten() {
        if x["passou"].as_bool() != Some(true) {
            s.push_str(&format!("  FALHOU {} {}\n", x["nome"], x["detalhe"]));
        }
    }
    let c = &v["colunas_contra_o_exemplo"];
    if c["larguras"].is_u64() {
        s.push_str(&format!(
            "  colunas iguais as do exemplo em {}/{} larguras\n",
            c["iguais"], c["larguras"]
        ));
    }
    s
}

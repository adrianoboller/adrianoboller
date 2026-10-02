//! Prova de fidelidade da conversao de tela (SP000022), a borda que precisa de processo:
//! cada tela do gabarito sai do `design_erp_ui` (SQL -> UI-IR -> HTML), e desenhada no
//! Chromium headless, fotografada, e volta pelo MESMO `ler_tela` + `app_da_leitura` do
//! `screenshot_to_erp_ui`. A comparacao e pura e mora em `phxclaw_ui_ir::fidelidade`.
//!
//! Sem tela do dono: o gabarito e sintetico e determinístico, entao a mesma maquina da o
//! mesmo numero -- menos o tempo, que sai com faixa min-max como todo tempo desta casa.
//!
//! As caixas da origem vem do DOM (o retangulo do TEXTO do rotulo, por `Range`), nao do
//! `<label>` inteiro, que ocupa a coluna toda: o OCR so ve o texto.

use crate::avaliacao::Faixa;
use crate::ui::{app_da_leitura, ler_tela, modelo_de_visao};
use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions};
use phxclaw_ui_ir::fidelidade::{self, GABARITO, Medida};
use phxclaw_ui_ir::{Caixa, Screen};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Telas do gabarito (o padrao do `--telas`).
pub const TELAS: usize = GABARITO.len();

/// Tamanho da janela: cabe a tela mais alta do gabarito (conferido pelo `fora_da_captura`
/// de cada tela, que sai no resultado).
pub const JANELA: (u32, u32) = (1280, 1200);

/// Arquivo servido ao lado da pagina: (caminho da requisicao, tipo, corpo).
pub(crate) type Extra = Arc<Mutex<Option<(String, &'static str, Vec<u8>)>>>;

/// Servidor de UMA pagina, so no loopback: o Chromium nao ve arquivo nem rede. A prova
/// responsiva (`responsivo_ui`) usa o mesmo servidor; o alvo Bootstrap precisa de uma folha
/// de estilo ao lado, servida pelo `extra` -- e por isso a folha vem do loopback e nao de
/// uma CDN, que a politica do navegador recusaria e a prova mediria sem estilo.
pub(crate) struct Pagina {
    pub(crate) base: String,
    pub(crate) html: Arc<Mutex<String>>,
    /// (caminho da requisicao, tipo, corpo): o que nao casar recebe o `html`.
    pub(crate) extra: Extra,
    parar: Arc<AtomicBool>,
}

impl Pagina {
    pub(crate) fn subir() -> Result<Self, String> {
        let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let base = format!("http://{}", l.local_addr().map_err(|e| e.to_string())?);
        let html = Arc::new(Mutex::new(String::new()));
        let extra: Extra = Arc::default();
        let parar = Arc::new(AtomicBool::new(false));
        let (h, x, p) = (html.clone(), extra.clone(), parar.clone());
        std::thread::spawn(move || {
            for s in l.incoming() {
                if p.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut s) = s else { continue };
                let mut b = [0u8; 4096];
                let n = s.read(&mut b).unwrap_or(0);
                let pedido = String::from_utf8_lossy(&b[..n]);
                let caminho = pedido.split_whitespace().nth(1).unwrap_or("/");
                let (tipo, corpo) = match &*x.lock().unwrap_or_else(|e| e.into_inner()) {
                    Some((c, t, corpo)) if c == caminho => (*t, corpo.clone()),
                    _ => (
                        "text/html; charset=utf-8",
                        h.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .clone()
                            .into_bytes(),
                    ),
                };
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        corpo.len()
                    )
                    .as_bytes(),
                );
                let _ = s.write_all(&corpo);
            }
        });
        Ok(Pagina {
            base,
            html,
            extra,
            parar,
        })
    }
}

impl Drop for Pagina {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        // acorda o `incoming` para a thread ver o sinal e sair
        let _ = std::net::TcpStream::connect(self.base.trim_start_matches("http://"));
    }
}

/// Retangulo do texto de cada rotulo e de cada cabecalho da grade, em pixels da janela.
const JS_CAIXAS: &str = "(()=>{const t=document.getElementById(TELA);const r=[];\
const cx=(n)=>{const g=document.createRange();g.selectNodeContents(n);return g.getBoundingClientRect()};\
t.querySelectorAll('label[for]').forEach(l=>{const b=cx(l);r.push(['f',l.getAttribute('for'),b.x,b.y,b.width,b.height])});\
t.querySelectorAll('table.itens thead th').forEach((h,i)=>{const b=cx(h);r.push(['c',String(i),b.x,b.y,b.width,b.height])});\
return {w:innerWidth,h:innerHeight,r}})()";

/// Uma tela do gabarito medida.
struct Rodada {
    medida: Medida,
    segundos: f64,
    fora_da_captura: usize,
}

/// Desenha a tela, fotografa e mede as caixas da origem.
async fn fotografar(
    navegador: &Browser,
    pagina: &Pagina,
    app: &phxclaw_ui_ir::App,
    tela: &str,
    png: &Path,
) -> Result<(Vec<(String, String, Caixa)>, usize), String> {
    *pagina.html.lock().unwrap_or_else(|e| e.into_inner()) = phxclaw_ui_ir::html::render(app);
    let p = navegador.new_page().await.map_err(|e| e.to_string())?;
    p.goto(&format!("{}/{tela}#{tela}", pagina.base))
        .await
        .map_err(|e| e.to_string())?;
    let detalhe = app.screens.iter().find_map(|s| match s {
        Screen::MasterDetail {
            id,
            detail,
            detail_columns,
            ..
        } if id == tela => Some((detail.clone(), detail_columns.clone())),
        _ => None,
    });
    if detalhe.is_some() {
        // documento de verdade tem itens: duas linhas vazias, como o usuario as abre
        for _ in 0..2 {
            p.click(&format!("#{tela} [data-add-item]"))
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    // sem foco: o contorno do foco nao e parte da tela
    p.eval("document.activeElement&&document.activeElement.blur()")
        .await
        .map_err(|e| e.to_string())?;
    let v = p
        .eval(&JS_CAIXAS.replace("TELA", &format!("{tela:?}")))
        .await
        .map_err(|e| e.to_string())?;
    let bytes = p.screenshot_png().await.map_err(|e| e.to_string())?;
    let _ = p.close().await;
    std::fs::write(png, bytes).map_err(|e| e.to_string())?;
    let (w, h) = (
        v["w"].as_f64().unwrap_or(1.0).max(1.0),
        v["h"].as_f64().unwrap_or(1.0).max(1.0),
    );
    let mut caixas = vec![];
    let mut fora = 0;
    let ent = app
        .screens
        .iter()
        .find_map(|s| match s {
            Screen::Form { id, entity, .. } if id == tela => Some(entity.clone()),
            Screen::MasterDetail { id, master, .. } if id == tela => Some(master.clone()),
            _ => None,
        })
        .unwrap_or_default();
    for r in v["r"].as_array().into_iter().flatten() {
        let n = |i: usize| r[i].as_f64().unwrap_or(0.0);
        let chave = r[1].as_str().unwrap_or("");
        let (e, campo) = match r[0].as_str() {
            Some("f") => (
                ent.clone(),
                chave
                    .strip_prefix(&format!("{tela}-"))
                    .unwrap_or(chave)
                    .to_string(),
            ),
            _ => {
                let Some((d, cols)) = &detalhe else { continue };
                let Some(c) = chave.parse::<usize>().ok().and_then(|i| cols.get(i)) else {
                    continue;
                };
                (d.clone(), c.clone())
            }
        };
        if n(3) + n(5) > h {
            fora += 1;
        }
        let r4 = |x: f64| ((x * 10000.0).round() / 10000.0) as f32;
        caixas.push((
            e,
            campo,
            Caixa {
                x: r4(n(2) / w),
                y: r4(n(3) / h),
                w: r4(n(4) / w),
                h: r4(n(5) / h),
            },
        ));
    }
    Ok((caixas, fora))
}

pub(crate) fn faixa(v: impl Iterator<Item = Option<f64>>) -> Value {
    let v: Vec<f64> = v.flatten().collect();
    match Faixa::de(&v) {
        Some(f) => json!({"n": f.n, "min": r4(f.min), "mediana": r4(f.mediana), "max": r4(f.max)}),
        None => json!({"n": 0, "nao_medido": "nenhuma tela com o numero"}),
    }
}

pub(crate) fn r4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn agregado(rs: &[Rodada]) -> Value {
    let m = |f: &dyn Fn(&Rodada) -> Option<f64>| faixa(rs.iter().map(f));
    let soma = |f: &dyn Fn(&Medida) -> usize| rs.iter().map(|r| f(&r.medida)).sum::<usize>();
    json!({
        "revocacao": m(&|r| Some(r.medida.revocacao())),
        "precisao": m(&|r| Some(r.medida.precisao())),
        "rotulo_exato": m(&|r| r.medida.rotulo_exato_fracao()),
        "tipo_igual": m(&|r| r.medida.tipo_fracao()),
        "obrigatorio_igual": m(&|r| r.medida.obrigatorio_fracao()),
        "kendall_tau": m(&|r| r.medida.kendall_tau),
        "grupo_rand": m(&|r| r.medida.grupo_rand),
        "titulo_do_grupo_igual": m(&|r| r.medida.titulo_do_grupo_fracao()),
        "distancia_media": m(&|r| r.medida.distancia_media),
        "distancia_max": m(&|r| r.medida.distancia_max),
        "segundos_por_tela": m(&|r| Some(r.segundos)),
        "totais": {
            "campos_na_origem": soma(&|x| x.origem),
            "achados": soma(&|x| x.achados),
            "perdidos": soma(&|x| x.perdidos.len()),
            "inventados": soma(&|x| x.inventados.len()),
            "fora_da_captura": rs.iter().map(|r| r.fora_da_captura).sum::<usize>(),
        },
    })
}

fn por_tela(r: &Rodada) -> Value {
    let mut v = serde_json::to_value(&r.medida).unwrap_or(Value::Null);
    v["revocacao"] = json!(r4(r.medida.revocacao()));
    v["precisao"] = json!(r4(r.medida.precisao()));
    v["segundos"] = json!(r4(r.segundos));
    v["fora_da_captura"] = json!(r.fora_da_captura);
    v
}

/// Mede `telas` telas do gabarito so com OCR + layout e, nas `com_modelo` primeiras,
/// tambem com o modelo de visao. Devolve o JSON do resultado (quem chama grava).
///
/// `capturas`: pasta onde ficam os PNG (para refazer o OCR de uma tela que errou); sem ela,
/// vao para uma pasta temporaria apagada no fim.
pub async fn medir(
    telas: usize,
    com_modelo: usize,
    prazo: Duration,
    capturas: Option<&Path>,
) -> Result<Value, String> {
    if phxclaw_browser::find_chromium().is_none() {
        return Err(format!(
            "Chromium nao encontrado; defina {}",
            phxclaw_browser::variavel_do_chromium()
        ));
    }
    let pagina = Pagina::subir()?;
    let navegador = Browser::launch(LaunchOptions {
        window_size: JANELA,
        envoltorio: Some(crate::processo::envoltorio_do_navegador()?),
        ..LaunchOptions::with_policy(BrowserPolicy::only([pagina.base.clone()]))
    })
    .await
    .map_err(|e| e.to_string())?;
    let temporaria = PastaDaMedida::nova()?;
    let pasta = match capturas {
        Some(c) => {
            std::fs::create_dir_all(c).map_err(|e| e.to_string())?;
            c.to_path_buf()
        }
        None => temporaria.0.clone(),
    };
    let (base, modelo) = modelo_de_visao();
    let mut so_ocr = vec![];
    let mut com = vec![];
    let mut modelo_falhou: Option<String> = None;
    for (k, g) in GABARITO.iter().take(telas).enumerate() {
        let (app, _) = phxclaw_ui_ir::from_sql(g.nome, g.sql);
        let tela = fidelidade::tela_de(&app, g.tabela)
            .ok_or_else(|| format!("{}: sem tela de edicao para {}", g.nome, g.tabela))?;
        let png = pasta.join(format!("{:02}-{}.png", k + 1, g.nome));
        let (caixas, fora) = fotografar(&navegador, &pagina, &app, &tela, &png).await?;
        let mut modos: Vec<Option<(&str, &str)>> = vec![None];
        if k < com_modelo && modelo_falhou.is_none() {
            modos.push(Some((base.as_str(), modelo.as_str())));
        }
        for visao in modos {
            let t0 = Instant::now();
            let leitura = match ler_tela(&png, visao, prazo).await {
                Ok(l) => l,
                Err(e) if visao.is_some() => {
                    modelo_falhou = Some(e.to_string());
                    continue;
                }
                Err(e) => return Err(format!("{}: {e}", g.nome)),
            };
            let (conv, _, _) = app_da_leitura(g.nome, g.tabela, &leitura);
            let segundos = t0.elapsed().as_secs_f64();
            let medida = fidelidade::comparar(&app, &tela, g.padrao, &caixas, &conv, g.tabela);
            let r = Rodada {
                medida,
                segundos,
                fora_da_captura: fora,
            };
            if visao.is_some() {
                com.push(r);
            } else {
                so_ocr.push(r);
            }
        }
    }
    let _ = navegador.close().await;
    let modelo_v = if com_modelo == 0 {
        json!({"nao_medido": "nao pedido (--modelo 0)"})
    } else if let Some(e) = &modelo_falhou
        && com.is_empty()
    {
        json!({"modelo": modelo, "nao_medido": e})
    } else {
        json!({
            "modelo": modelo,
            "telas_n": com.len(),
            "falha": modelo_falhou,
            "agregado": agregado(&com),
            "telas": com.iter().map(por_tela).collect::<Vec<_>>(),
        })
    };
    Ok(json!({
        "data": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "comando": format!("phxclaw ui fidelidade --telas {telas} --modelo {com_modelo}"),
        "ida_e_volta": "SQL -> design_erp_ui -> HTML -> Chromium headless (PNG) -> tesseract TSV no bwrap -> layout -> SQL -> UI-IR",
        "janela": [JANELA.0, JANELA.1],
        "regra": "numero so medido; cada metrica com faixa min-max sobre as telas; a chave primaria fica fora da conta",
        "so_ocr": {
            "telas_n": so_ocr.len(),
            "agregado": agregado(&so_ocr),
            "telas": so_ocr.iter().map(por_tela).collect::<Vec<_>>(),
        },
        "com_modelo": modelo_v,
    }))
}

/// Pasta das capturas, apagada no fim: os PNG sao refeitos a cada medida, e o resultado
/// que importa e o JSON.
struct PastaDaMedida(PathBuf);

impl PastaDaMedida {
    fn nova() -> Result<Self, String> {
        let p = std::env::temp_dir().join(format!(
            "phx-fidelidade-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
        Ok(PastaDaMedida(p))
    }
}

impl Drop for PastaDaMedida {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Tabela curta do resultado, para o terminal.
pub fn tabela(v: &Value) -> String {
    let mut s = String::new();
    for (modo, chave) in [("so OCR + layout", "so_ocr"), ("com modelo", "com_modelo")] {
        let a = &v[chave]["agregado"];
        if a.is_null() {
            s.push_str(&format!(
                "{modo}: {}\n",
                v[chave]["nao_medido"].as_str().unwrap_or("nao medido")
            ));
            continue;
        }
        s.push_str(&format!(
            "{modo} ({} telas, {}):\n",
            v[chave]["telas_n"],
            v["data"].as_str().unwrap_or("")
        ));
        for m in [
            "revocacao",
            "precisao",
            "rotulo_exato",
            "tipo_igual",
            "obrigatorio_igual",
            "kendall_tau",
            "grupo_rand",
            "titulo_do_grupo_igual",
            "distancia_media",
            "segundos_por_tela",
        ] {
            let f = &a[m];
            s.push_str(&format!(
                "  {m:<22} mediana {:>7}  faixa {} .. {}  (n={})\n",
                f["mediana"], f["min"], f["max"], f["n"]
            ));
        }
        s.push_str(&format!("  totais {}\n", a["totais"]));
    }
    s
}

use crate::cdp::{Connection, Event};
use crate::error::{BrowserError, Result, timeout_err};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

/// Link da pagina com `href` ja absoluto (resolvido pelo proprio navegador).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub text: String,
    pub href: String,
}

/// Uma aba. Toda a rede dela passa pela politica do `Browser` que a criou.
pub struct Page {
    conn: Arc<Connection>,
    target_id: String,
    session: String,
    events: Mutex<mpsc::UnboundedReceiver<Event>>,
    navigation_timeout: Duration,
}

impl Page {
    pub(crate) async fn attach(
        conn: Arc<Connection>,
        target_id: String,
        session: String,
        navigation_timeout: Duration,
    ) -> Result<Self> {
        let events = conn.listen(&session);
        conn.call("Page.enable", json!({}), Some(&session)).await?;
        Ok(Self {
            conn,
            target_id,
            session,
            events: Mutex::new(events),
            navigation_timeout,
        })
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.conn.call(method, params, Some(&self.session)).await
    }

    /// Descarta eventos velhos para que a proxima espera de carga responda a
    /// acao que vem a seguir, e nao a uma carga anterior.
    async fn drain_events(&self) {
        let mut rx = self.events.lock().await;
        while rx.try_recv().is_ok() {}
    }

    /// Navega e espera o `load`. A URL passa pela politica aqui (recusa
    /// rapida, sem tocar a rede) e de novo na interceptacao, que e a que
    /// alcanca os redirecionamentos.
    pub async fn goto(&self, url: &str) -> Result<()> {
        self.conn.policy.check_url(url).await?;
        self.drain_events().await;
        let marca = self.conn.blocked_len();
        let r = self
            .conn
            .call_timeout(
                "Page.navigate",
                json!({ "url": url }),
                Some(&self.session),
                self.navigation_timeout,
            )
            .await?;
        if let Some(erro) = r
            .get("errorText")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(b) = self.conn.blocked_since(marca).into_iter().next() {
                return Err(BrowserError::NavigationBlocked {
                    url: b.url,
                    reason: b.reason,
                });
            }
            return Err(BrowserError::Navigation {
                url: url.to_string(),
                error: erro.to_string(),
            });
        }
        // Sem loaderId a navegacao foi dentro do mesmo documento (ancora) e
        // nao havera evento de carga para esperar.
        if r.get("loaderId").is_none() {
            return Ok(());
        }
        self.wait_for_load().await
    }

    /// Espera o proximo `Page.loadEventFired`, com o prazo de navegacao.
    pub async fn wait_for_load(&self) -> Result<()> {
        let prazo = self.navigation_timeout;
        let mut rx = self.events.lock().await;
        let espera = async {
            loop {
                match rx.recv().await {
                    Some(ev) if ev.method == "Page.loadEventFired" => return Ok(()),
                    Some(_) => continue,
                    None => return Err(BrowserError::ConnectionClosed),
                }
            }
        };
        tokio::time::timeout(prazo, espera)
            .await
            .map_err(|_| timeout_err("esperar carga da pagina", prazo))?
    }

    /// Avalia JS e devolve o valor por JSON; promessas sao aguardadas.
    pub async fn eval(&self, js: &str) -> Result<Value> {
        let r = self
            .call(
                "Runtime.evaluate",
                json!({ "expression": js, "returnByValue": true, "awaitPromise": true }),
            )
            .await?;
        if let Some(ex) = r.get("exceptionDetails") {
            let msg = ex
                .pointer("/exception/description")
                .and_then(Value::as_str)
                .or_else(|| ex.get("text").and_then(Value::as_str))
                .unwrap_or("excecao sem descricao");
            return Err(BrowserError::JavaScript(msg.to_string()));
        }
        Ok(r.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }

    async fn eval_string(&self, js: &str) -> Result<String> {
        match self.eval(js).await? {
            Value::String(s) => Ok(s),
            Value::Null => Ok(String::new()),
            outro => Err(BrowserError::Protocol(format!(
                "esperava texto, veio {outro}"
            ))),
        }
    }

    pub async fn title(&self) -> Result<String> {
        self.eval_string("document.title").await
    }

    pub async fn url(&self) -> Result<String> {
        self.eval_string("location.href").await
    }

    /// Texto legivel do corpo, como o usuario o veria (`innerText`).
    pub async fn text(&self) -> Result<String> {
        self.eval_string("document.body ? document.body.innerText : ''")
            .await
    }

    /// Markdown simples: titulos, paragrafos, itens de lista e links com
    /// `href` absoluto. Feito no proprio navegador porque la o DOM ja esta
    /// montado e o CSS ja disse o que esta escondido.
    pub async fn markdown_like(&self) -> Result<String> {
        self.eval_string(MARKDOWN_JS).await
    }

    pub async fn links(&self) -> Result<Vec<Link>> {
        let v = self
            .eval(
                "Array.from(document.querySelectorAll('a[href]')).map(a => \
                 ({ text: (a.innerText || '').trim(), href: a.href }))",
            )
            .await?;
        serde_json::from_value(v).map_err(|e| BrowserError::Protocol(e.to_string()))
    }

    /// Clica no centro do elemento com evento de mouse real (gesto de
    /// usuario), nao com `el.click()`: pagina que so reage a gesto confiavel
    /// ignoraria o clique sintetico.
    pub async fn click(&self, css: &str) -> Result<()> {
        let sel = serde_json::to_string(css).map_err(|e| BrowserError::Protocol(e.to_string()))?;
        let js = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return null; \
             el.scrollIntoView({{block: 'center', inline: 'center'}}); \
             const r = el.getBoundingClientRect(); \
             return {{x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height}}; }})()"
        );
        let v = self.eval(&js).await?;
        if v.is_null() {
            return Err(BrowserError::ElementNotFound(css.to_string()));
        }
        let num = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        if num("w") <= 0.0 || num("h") <= 0.0 {
            return Err(BrowserError::ElementNotFound(format!(
                "{css} (sem area visivel)"
            )));
        }
        let (x, y) = (num("x"), num("y"));
        self.drain_events().await;
        self.call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        for tipo in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                json!({ "type": tipo, "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )
            .await?;
        }
        Ok(())
    }

    /// Clica e espera a carga que o clique provoca.
    pub async fn click_and_wait(&self, css: &str) -> Result<()> {
        self.click(css).await?;
        self.wait_for_load().await
    }

    /// Foca o elemento e insere o texto como digitacao (dispara `input`).
    pub async fn type_text(&self, css: &str, text: &str) -> Result<()> {
        let sel = serde_json::to_string(css).map_err(|e| BrowserError::Protocol(e.to_string()))?;
        let js = format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return false; \
             el.focus(); return document.activeElement === el; }})()"
        );
        if self.eval(&js).await? != Value::Bool(true) {
            return Err(BrowserError::ElementNotFound(css.to_string()));
        }
        self.call("Input.insertText", json!({ "text": text }))
            .await
            .map(|_| ())
    }

    /// Enter no elemento focado; num campo de formulario isso o envia.
    pub async fn press_enter(&self) -> Result<()> {
        self.drain_events().await;
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown", "key": "Enter", "code": "Enter",
                "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13,
                "text": "\r", "unmodifiedText": "\r"
            }),
        )
        .await?;
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp", "key": "Enter", "code": "Enter",
                "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13
            }),
        )
        .await
        .map(|_| ())
    }

    /// Tab de teclado de verdade. `el.focus()` por script nao serve para provar a ordem de
    /// tabulacao (pula o `tabindex` do navegador) nem o anel de foco (`:focus-visible` so
    /// liga, garantido, quando o foco veio do teclado).
    pub async fn tecla_tab(&self) -> Result<()> {
        for tipo in ["keyDown", "keyUp"] {
            self.call(
                "Input.dispatchKeyEvent",
                json!({
                    "type": tipo, "key": "Tab", "code": "Tab",
                    "windowsVirtualKeyCode": 9, "nativeVirtualKeyCode": 9
                }),
            )
            .await?;
        }
        Ok(())
    }

    /// Emula a tela de um aparelho: largura e altura em CSS px e, se `celular`, o toque e a
    /// meta viewport valendo. Trocar a janela do processo nao basta para provar layout de
    /// celular: o `--window-size` do headless nao liga `pointer: coarse` nem le a meta
    /// viewport, e a pagina responderia como um desktop estreito.
    pub async fn emular_tela(&self, largura: u32, altura: u32, celular: bool) -> Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": largura, "height": altura,
                "deviceScaleFactor": 1, "mobile": celular
            }),
        )
        .await?;
        // o CDP recusa `maxTouchPoints` 0 mesmo desligando: com toque desligado, omitido
        let toque = if celular {
            json!({ "enabled": true, "maxTouchPoints": 5 })
        } else {
            json!({ "enabled": false })
        };
        self.call("Emulation.setTouchEmulationEnabled", toque)
            .await
            .map(|_| ())
    }

    pub async fn screenshot_png(&self) -> Result<Vec<u8>> {
        let r = self
            .call("Page.captureScreenshot", json!({ "format": "png" }))
            .await?;
        let dados = r
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Protocol("captura sem dados".into()))?;
        base64::engine::general_purpose::STANDARD
            .decode(dados)
            .map_err(|e| BrowserError::Protocol(format!("base64 da captura: {e}")))
    }

    pub async fn close(self) -> Result<()> {
        self.conn
            .call(
                "Target.closeTarget",
                json!({ "targetId": self.target_id }),
                None,
            )
            .await
            .map(|_| ())
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.conn.unlisten(&self.session);
        self.conn.send_nowait(
            "Target.closeTarget",
            json!({ "targetId": self.target_id }),
            None,
        );
    }
}

const MARKDOWN_JS: &str = r#"(() => {
  const out = [];
  const skip = new Set(['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','SVG','HEAD','IFRAME']);
  const block = new Set(['P','DIV','SECTION','ARTICLE','MAIN','HEADER','FOOTER','NAV','ASIDE',
    'FORM','TABLE','TBODY','THEAD','TR','BLOCKQUOTE','PRE','UL','OL','DL','DT','DD','HR','FIGURE']);
  const norm = s => s.replace(/\s+/g, ' ');
  const hidden = el => { const cs = getComputedStyle(el); return cs.display === 'none' || cs.visibility === 'hidden'; };
  function inline(node) {
    let s = '';
    for (const c of node.childNodes) {
      if (c.nodeType === 3) { s += c.textContent; continue; }
      if (c.nodeType !== 1) continue;
      const tag = c.tagName.toUpperCase();
      if (skip.has(tag) || hidden(c)) continue;
      if (tag === 'A' && c.href) {
        const t = norm(inline(c)).trim();
        if (t) s += '[' + t + '](' + c.href + ')';
      } else if (tag === 'IMG') {
        if (c.alt) s += c.alt;
      } else if (tag === 'BR') {
        s += ' ';
      } else {
        s += ' ' + inline(c) + ' ';
      }
    }
    return s;
  }
  function walk(el) {
    let buf = '';
    const flush = () => { const t = norm(buf).trim(); if (t) out.push(t); buf = ''; };
    for (const c of el.childNodes) {
      if (c.nodeType === 3) { buf += c.textContent; continue; }
      if (c.nodeType !== 1) continue;
      const tag = c.tagName.toUpperCase();
      if (skip.has(tag) || hidden(c)) continue;
      if (/^H[1-6]$/.test(tag)) {
        flush();
        const t = norm(inline(c)).trim();
        if (t) out.push('#'.repeat(Number(tag[1])) + ' ' + t);
      } else if (tag === 'LI') {
        flush();
        const t = norm(inline(c)).trim();
        if (t) out.push('- ' + t);
      } else if (block.has(tag) || ['block', 'flex', 'grid', 'table', 'list-item'].includes(getComputedStyle(c).display)) {
        flush();
        walk(c);
      } else if (tag === 'A' && c.href) {
        const t = norm(inline(c)).trim();
        if (t) buf += ' [' + t + '](' + c.href + ') ';
      } else {
        buf += ' ' + inline(c) + ' ';
      }
    }
    flush();
  }
  if (document.body) walk(document.body);
  return out.join('\n\n');
})()"#;

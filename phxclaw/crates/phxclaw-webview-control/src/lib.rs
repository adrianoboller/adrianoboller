use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum WebViewCommand {
    Navigate { url: String },
    LoadHtml { html: String, base_url: Option<String> },
    EvaluateJavascript { script: String },
    InjectCss { css: String },
    QuerySelector { selector: String },
    QuerySelectorAll { selector: String },
    GetOuterHtml { selector: Option<String> },
    SetInnerHtml { selector: String, html: String },
    SetAttribute { selector: String, name: String, value: String },
    RemoveAttribute { selector: String, name: String },
    Click { selector: String },
    Focus { selector: String },
    TypeText { selector: String, text: String },
    DispatchEvent { selector: String, event_type: String, detail: Value },
    ScrollIntoView { selector: String },
    GetComputedStyle { selector: String },
    DomToSvg { selector: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewRequest {
    pub uuid: Uuid,
    pub view_uuid: Uuid,
    pub command: WebViewCommand,
}

impl WebViewRequest {
    pub fn new(view_uuid: Uuid, command: WebViewCommand) -> Self {
        Self { uuid: new_uuid_v7(), view_uuid, command }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewResult {
    pub request_uuid: Uuid,
    pub value: Value,
}

#[derive(Debug, Error)]
pub enum WebViewError {
    #[error("view not found: {0}")]
    ViewNotFound(Uuid),
    #[error("operation denied: {0}")]
    Denied(String),
    #[error("webview backend error: {0}")]
    Backend(String),
}

/// Implemented by the desktop shell (WRY/Tauri is the v0.5 selected backend).
/// Keeping this as a trait prevents WebView code from entering the microkernel.
pub trait WebViewBackend {
    fn create_view(&mut self, initial_url: Option<&str>) -> Result<Uuid, WebViewError>;
    fn execute(&mut self, request: &WebViewRequest) -> Result<WebViewResult, WebViewError>;
    fn close_view(&mut self, view_uuid: Uuid) -> Result<(), WebViewError>;
}

/// JS injected into controlled views. It exposes only explicit bridge functions; arbitrary
/// JavaScript still requires the `webview.javascript.execute` permission.
pub const PHOENIX_DOM_BRIDGE: &str = r#"
(() => {
  if (window.__phoenixClaw) return;
  window.__phoenixClaw = {
    query(selector) {
      const el = document.querySelector(selector);
      return el ? { html: el.outerHTML, text: el.textContent, value: el.value ?? null } : null;
    },
    queryAll(selector) {
      return [...document.querySelectorAll(selector)].map(el => ({
        html: el.outerHTML, text: el.textContent, value: el.value ?? null
      }));
    },
    setHtml(selector, html) {
      const el = document.querySelector(selector);
      if (!el) return false;
      el.innerHTML = html;
      return true;
    },
    injectCss(css) {
      const style = document.createElement('style');
      style.dataset.phxclaw = 'injected';
      style.textContent = css;
      document.head.appendChild(style);
      return true;
    },
    domToSvg(selector) {
      const el = selector ? document.querySelector(selector) : document.documentElement;
      if (!el) return null;
      const xml = new XMLSerializer().serializeToString(el);
      const escaped = xml.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
      return `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800"><foreignObject width="100%" height="100%"><pre xmlns="http://www.w3.org/1999/xhtml">${escaped}</pre></foreignObject></svg>`;
    }
  };
})();
"#;

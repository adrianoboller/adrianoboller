#![forbid(unsafe_code)]
//! Navegador operado pelo agente: Chromium headless dirigido pelo Chrome
//! DevTools Protocol, sem Playwright nem Python. Toda requisicao de rede de
//! toda aba passa pela `BrowserPolicy`, que nega por padrao.

mod browser;
mod cdp;
mod error;
mod page;
mod policy;

pub use browser::{
    Browser, CHROMIUM_ENV, LaunchOptions, find_chromium, idioma_do_sistema, running_as_root,
};
pub use cdp::BlockedRequest;
pub use error::{BrowserError, Result};
pub use page::{Link, Page};
pub use policy::{BrowserPolicy, is_blocked_ip};

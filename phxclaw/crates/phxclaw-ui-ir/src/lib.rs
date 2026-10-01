#![forbid(unsafe_code)]
//! Phoenix ERP UI Designer, nucleo: SQL -> modelo ERP -> UI-IR -> renderizador.
//!
//! O UI-IR (`ir`) e neutro: o analisador (`schema`) nao conhece o renderizador, e o
//! renderizador (`html`) nao conhece SQL. Renderizadores novos (React, WinDev, WebDev,
//! Flutter) leem o mesmo IR sem tocar na inteligencia de ERP.

pub mod bootstrap;
pub mod fidelidade;
pub mod flutter;
pub mod html;
pub mod imagem;
pub mod ir;
pub mod layout;
pub mod phx_json;
pub mod react;
pub mod regras;
pub mod responsivo;
pub mod rust;
pub mod schema;
pub mod wlanguage;

pub use ir::*;

/// Atalho: DDL SQL -> (UI-IR, avisos do parser).
pub fn from_sql(app_name: &str, sql: &str) -> (App, Vec<String>) {
    let p = schema::parse(sql);
    (schema::analyze(app_name, &p), p.avisos)
}

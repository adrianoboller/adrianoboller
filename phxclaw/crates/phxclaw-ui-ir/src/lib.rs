#![forbid(unsafe_code)]
//! Phoenix ERP UI Designer, nucleo: SQL -> modelo ERP -> UI-IR -> renderizador.
//!
//! O UI-IR (`ir`) e neutro: o analisador (`schema`) nao conhece o renderizador, e o
//! renderizador (`html`) nao conhece SQL. Renderizadores novos (React, WinDev, WebDev,
//! Flutter) leem o mesmo IR sem tocar na inteligencia de ERP.

pub mod html;
pub mod ir;
pub mod react;
pub mod schema;

pub use ir::*;

/// Atalho: DDL SQL -> (UI-IR, avisos do parser).
pub fn from_sql(app_name: &str, sql: &str) -> (App, Vec<String>) {
    let p = schema::parse(sql);
    (schema::analyze(app_name, &p), p.avisos)
}

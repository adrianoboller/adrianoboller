#![forbid(unsafe_code)]
//! Ponte Python<->Rust do PhxClaw: o nucleo deterministico do UI-IR exposto ao Python.
//!
//! Por que este, e nao outro: e a parte do PhxClaw que um projeto Python quer chamar sem
//! subir servidor nenhum -- DDL SQL vira o modelo de telas de ERP (`ui_ir`), o HTML da
//! aplicacao (`html`) e a comparacao tolerante a OCR (`normalizar`). E e funcao pura: o
//! mesmo texto entra, o mesmo resultado sai, sem rede e sem disco.
//!
//! Nenhuma regra mora aqui. Cada funcao chama a do `phxclaw-ui-ir` e so traduz a borda:
//! uma segunda copia das regras divergiria da primeira no primeiro conserto.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Teto do SQL aceito: o analisador e linear, mas um texto de centenas de MB vindo de um
/// laco com defeito seguraria o interpretador inteiro (o GIL fica com a chamada).
const SQL_MAX: usize = 4 * 1024 * 1024;

fn analisar(nome: &str, sql: &str) -> PyResult<(phxclaw_ui_ir::App, Vec<String>)> {
    if nome.trim().is_empty() {
        return Err(PyValueError::new_err("nome da aplicacao vazio"));
    }
    if sql.len() > SQL_MAX {
        return Err(PyValueError::new_err(format!(
            "SQL maior que {SQL_MAX} bytes"
        )));
    }
    let (app, avisos) = phxclaw_ui_ir::from_sql(nome, sql);
    // App sem entidade nenhuma nao e resultado, e SQL que o analisador nao entendeu: a
    // recusa carrega os avisos dele, que dizem onde.
    if app.entities.is_empty() {
        let mut m = "nenhuma tabela reconhecida no SQL".to_string();
        if !avisos.is_empty() {
            m.push_str(": ");
            m.push_str(&avisos.join("; "));
        }
        return Err(PyValueError::new_err(m));
    }
    Ok((app, avisos))
}

/// DDL SQL -> (UI-IR como dict, avisos do analisador). O dict tem as chaves do IR
/// serializado (`name`, `ir_version`, `entities`, `screens`, `menu`).
#[pyfunction]
fn ui_ir<'py>(
    py: Python<'py>,
    nome: &str,
    sql: &str,
) -> PyResult<(Bound<'py, PyAny>, Vec<String>)> {
    let (app, avisos) = analisar(nome, sql)?;
    let texto = serde_json::to_string(&app).map_err(|e| PyValueError::new_err(e.to_string()))?;
    // O `json` do proprio interpretador monta o dict: tipos nativos do Python, sem um
    // segundo conversor de JSON escrito aqui.
    let v = py.import("json")?.call_method1("loads", (texto,))?;
    Ok((v, avisos))
}

/// DDL SQL -> HTML da aplicacao (cadastro, consulta e menu de cada tabela).
#[pyfunction]
fn html(nome: &str, sql: &str) -> PyResult<String> {
    let (app, _) = analisar(nome, sql)?;
    Ok(phxclaw_ui_ir::html::render(&app))
}

/// Comparacao tolerante a OCR: sem acento, sem caixa, so letras e digitos.
#[pyfunction]
fn normalizar(texto: &str) -> String {
    phxclaw_ui_ir::imagem::normalizar(texto)
}

#[pymodule]
fn phxclaw_nativo(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ui_ir, m)?)?;
    m.add_function(wrap_pyfunction!(html, m)?)?;
    m.add_function(wrap_pyfunction!(normalizar, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

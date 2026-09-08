//! Acesso ao MySQL -- o MESMO banco do legado, tabela `clientes` intocada.
//! BR-004 mora aqui: excluir e UPDATE ativo=0, nunca DELETE.
use mysql::prelude::*;
use mysql::{params, Pool, PooledConn};
use serde::Serialize;
use serde_json::{json, Value};

use crate::regras::{limite_como_texto, normalizar_email, normalizar_limite, normalizar_nome};

#[derive(Serialize)]
pub struct Cliente {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub limite_credito: String,
    pub ativo: i64,
}

pub fn conectar() -> Pool {
    // origem: config.php#3-6 -- as mesmas variaveis de ambiente do legado
    let host = std::env::var("LOJA_DB_HOST").unwrap_or_else(|_| "localhost".into());
    let nome = std::env::var("LOJA_DB_NAME").unwrap_or_else(|_| "loja".into());
    let user = std::env::var("LOJA_DB_USER").unwrap_or_else(|_| "loja".into());
    let pass = std::env::var("LOJA_DB_PASS").unwrap_or_default();
    Pool::new(format!("mysql://{user}:{pass}@{host}:3306/{nome}").as_str()).expect("Sem banco")
}

pub fn listar(cx: &mut PooledConn, so_ativos: bool) -> Value {
    let sql = format!(
        "SELECT id, nome, email, limite_credito, ativo FROM clientes{} ORDER BY nome, id",
        if so_ativos { " WHERE ativo = 1" } else { "" }
    );
    let linhas: Vec<(i64, String, String, String, i64)> = cx.query(sql).unwrap_or_default();
    Value::Array(
        linhas
            .into_iter()
            .map(|(id, nome, email, limite, ativo)| {
                // DECIMAL(12,2) chega como texto "1500.01"; number_format do PHP da o mesmo
                let cent: i64 = limite.replace('.', "").parse().unwrap_or(0);
                json!(Cliente { id, nome, email, limite_credito: limite_como_texto(cent), ativo })
            })
            .collect(),
    )
}

fn erro_de_banco(e: mysql::Error) -> Value {
    // origem: clientes.php#44-46 -- 1062 e a chave unica, segunda linha de defesa da BR-001
    if let mysql::Error::MySqlError(m) = &e {
        if m.code == 1062 {
            return json!({"erro": "e-mail ja cadastrado"});
        }
    }
    json!({"erro": e.to_string()})
}

pub fn incluir(cx: &mut PooledConn, nome: &str, email: &str, limite: &str) -> Value {
    let n = match normalizar_nome(nome) { Ok(v) => v, Err(e) => return json!({"erro": e}) };
    let e = normalizar_email(email);
    let l = match normalizar_limite(limite) { Ok(v) => v, Err(e) => return json!({"erro": e}) };
    match cx.exec_drop(
        "INSERT INTO clientes (nome, email, limite_credito) VALUES (:n, :e, :l)",
        params! { "n" => n, "e" => e, "l" => limite_como_texto(l) },
    ) {
        Ok(()) => json!({"id": cx.last_insert_id()}),
        Err(err) => erro_de_banco(err),
    }
}

pub fn alterar(cx: &mut PooledConn, id: i64, nome: &str, email: &str, limite: &str) -> Value {
    let n = match normalizar_nome(nome) { Ok(v) => v, Err(e) => return json!({"erro": e}) };
    let e = normalizar_email(email);
    let l = match normalizar_limite(limite) { Ok(v) => v, Err(e) => return json!({"erro": e}) };
    match cx.exec_iter(
        "UPDATE clientes SET nome = :n, email = :e, limite_credito = :l WHERE id = :id AND ativo = 1",
        params! { "n" => n, "e" => e, "l" => limite_como_texto(l), "id" => id },
    ) {
        Ok(r) => json!({"alterados": r.affected_rows()}),
        Err(err) => erro_de_banco(err),
    }
}

/// BR-004 — origem: clientes.php#62-68: desativa, nao apaga.
pub fn excluir(cx: &mut PooledConn, id: i64) -> Value {
    match cx.exec_iter("UPDATE clientes SET ativo = 0 WHERE id = :id AND ativo = 1", params! { "id" => id }) {
        Ok(r) => json!({"desativados": r.affected_rows()}),
        Err(err) => erro_de_banco(err),
    }
}

pub fn truncar(cx: &mut PooledConn) {
    cx.query_drop("TRUNCATE TABLE clientes").ok();
}

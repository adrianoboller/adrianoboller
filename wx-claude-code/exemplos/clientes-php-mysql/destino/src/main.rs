//! clientes-rs: o cadastro de clientes, servico em porta TCP (letra H:
//! interface = servico-tcp). Dois modos:
//!
//!   clientes-rs golden          le {id, regra, entrada:{passos}} do stdin,
//!                               roda a sequencia do zero e devolve as respostas
//!                               -- e o que o golden.py comparar chama
//!   clientes-rs servir [porta]  API JSON para o frontend React
mod regras;
mod repo;

use serde_json::{json, Value};
use std::io::Read;

fn executar(cx: &mut mysql::PooledConn, passo: &Value) -> Value {
    let p = passo.as_array().cloned().unwrap_or_default();
    let s = |i: usize| p.get(i).map(|v| match v { Value::String(s) => s.clone(), o => o.to_string() }).unwrap_or_default();
    let n = |i: usize| p.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
    match s(0).as_str() {
        "incluir" => repo::incluir(cx, &s(1), &s(2), &s(3)),
        "alterar" => repo::alterar(cx, n(1), &s(2), &s(3), &s(4)),
        "excluir" => repo::excluir(cx, n(1)),
        "listar" => repo::listar(cx, true),
        "listar_todos" => repo::listar(cx, false),
        outro => json!({"erro": format!("passo desconhecido: {outro}")}),
    }
}

fn golden() {
    let mut entrada = String::new();
    std::io::stdin().read_to_string(&mut entrada).expect("stdin");
    let caso: Value = serde_json::from_str(&entrada).expect("JSON do caso");
    let pool = repo::conectar();
    let mut cx = pool.get_conn().expect("conexao");
    repo::truncar(&mut cx);
    let passos = caso["entrada"]["passos"].as_array().cloned().unwrap_or_default();
    let respostas: Vec<Value> = passos.iter().map(|p| executar(&mut cx, p)).collect();
    println!("{}", serde_json::to_string(&respostas).unwrap());
}

fn servir(porta: u16) {
    let pool = repo::conectar();
    let servidor = tiny_http::Server::http(("0.0.0.0", porta)).expect("porta");
    eprintln!("clientes-rs servindo em http://0.0.0.0:{porta}/  (GET /clientes, POST /clientes, PUT /clientes/:id, DELETE /clientes/:id)");
    for mut req in servidor.incoming_requests() {
        let mut corpo = String::new();
        req.as_reader().read_to_string(&mut corpo).ok();
        let dados: Value = serde_json::from_str(&corpo).unwrap_or(Value::Null);
        let campo = |k: &str| dados.get(k).map(|v| match v { Value::String(s) => s.clone(), o => o.to_string() }).unwrap_or_default();
        let url = req.url().to_string();
        let id = url.rsplit('/').next().and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        let mut cx = pool.get_conn().expect("conexao");
        let resposta = match (req.method().as_str(), url.as_str()) {
            ("GET", "/clientes") => repo::listar(&mut cx, true),
            ("GET", "/clientes/todos") => repo::listar(&mut cx, false),
            ("POST", "/clientes") => repo::incluir(&mut cx, &campo("nome"), &campo("email"), &campo("limite_credito")),
            ("PUT", _) if url.starts_with("/clientes/") => repo::alterar(&mut cx, id, &campo("nome"), &campo("email"), &campo("limite_credito")),
            ("DELETE", _) if url.starts_with("/clientes/") => repo::excluir(&mut cx, id),
            ("OPTIONS", _) => json!({}),
            _ => json!({"erro": "rota desconhecida"}),
        };
        let status = if resposta.get("erro").is_some() { 422 } else { 200 };
        let cabecalhos = [
            ("Content-Type", "application/json; charset=utf-8"),
            ("Access-Control-Allow-Origin", "*"),
            ("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS"),
            ("Access-Control-Allow-Headers", "Content-Type"),
        ];
        let mut r = tiny_http::Response::from_string(resposta.to_string()).with_status_code(status);
        for (k, v) in cabecalhos {
            r = r.with_header(tiny_http::Header::from_bytes(k, v).unwrap());
        }
        req.respond(r).ok();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("golden") => golden(),
        Some("servir") => servir(args.get(2).and_then(|p| p.parse().ok()).unwrap_or(8080)),
        _ => eprintln!("uso: clientes-rs golden | clientes-rs servir [porta]"),
    }
}

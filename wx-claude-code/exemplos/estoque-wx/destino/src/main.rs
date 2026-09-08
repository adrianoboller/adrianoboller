//! estoque-rs: o ESTOQUE (WINDEV 2025) convertido para Rust + Axum + PostgreSQL 16.
//! `golden`  — le um caso do golden master em stdin e responde em stdout (golden.py comparar).
//! `servir`  — a API que a tela React (WIN_Venda) consome.
mod regras;
mod repo;

use axum::{extract::{Query, State}, http::StatusCode, routing::{get, post}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, io::Read, sync::Arc};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

type Db = Arc<Mutex<tokio_postgres::Client>>;

fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("")
}

/// Um caso por execucao: {"id","regra","entrada"} -> resultado no formato do resultados-esperados.json.
async fn golden() -> Result<(), String> {
    let mut txt = String::new();
    std::io::stdin().read_to_string(&mut txt).map_err(|e| e.to_string())?;
    let caso: Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    let regra = s(&caso, "regra");
    let e = caso.get("entrada").cloned().unwrap_or(Value::Null);
    let r: Value = if regra.starts_with("BR-001") {
        // o legado devolve -1 acima do teto (e mostra Error)
        match regras::calcula_desconto(f(&e, "subtotal"), f(&e, "percentual"), s(&e, "tipo")) {
            Ok(v) => json!(v),
            Err(_) => json!(-1),
        }
    } else if regra.starts_with("BR-003") {
        json!(regras::calcula_juros_atraso(f(&e, "valor"), s(&e, "vencimento"), s(&e, "pagamento"))?)
    } else if regra.starts_with("BR-004") {
        json!(regras::valores_das_parcelas(f(&e, "total"), f(&e, "parcelas") as u32)?)
    } else if regra.starts_with("BR-005") {
        json!(regras::valida_cpf(s(&e, "cpf")))
    } else if regra.starts_with("QRY-003") {
        let c = repo::conectar().await?;
        json!(repo::comissao_mensal(&c, f(&e, "mes") as i32, f(&e, "ano") as i32).await?)
    } else {
        return Err(format!("regra sem conversor: {regra}"));
    };
    println!("{}", json!({"resultado": r}));
    Ok(())
}

fn erro(msg: String) -> (StatusCode, Json<Value>) {
    let st = if msg.starts_with("CONFIRMAR:") { StatusCode::CONFLICT } else { StatusCode::UNPROCESSABLE_ENTITY };
    (st, Json(json!({"erro": msg})))
}
type Resp = Result<Json<Value>, (StatusCode, Json<Value>)>;

async fn get_clientes(State(db): State<Db>) -> Resp {
    Ok(Json(repo::clientes_ativos(&*db.lock().await).await.map_err(erro)?))
}
async fn get_produtos(State(db): State<Db>) -> Resp {
    Ok(Json(repo::produtos_ativos(&*db.lock().await).await.map_err(erro)?))
}
async fn get_depositos(State(db): State<Db>) -> Resp {
    Ok(Json(repo::depositos(&*db.lock().await).await.map_err(erro)?))
}
async fn get_comissao(State(db): State<Db>, Query(q): Query<HashMap<String, String>>) -> Resp {
    let mes = q.get("mes").and_then(|x| x.parse().ok()).unwrap_or(0);
    let ano = q.get("ano").and_then(|x| x.parse().ok()).unwrap_or(0);
    Ok(Json(json!(repo::comissao_mensal(&*db.lock().await, mes, ano).await.map_err(erro)?)))
}
async fn get_vendas(State(db): State<Db>, Query(q): Query<HashMap<String, String>>) -> Resp {
    let v = q.get("vendedor").cloned().unwrap_or_default();
    Ok(Json(repo::vendas_periodo(&*db.lock().await, q.get("ini").map(String::as_str).unwrap_or("2000-01-01"), q.get("fim").map(String::as_str).unwrap_or("2100-12-31"), &v).await.map_err(erro)?))
}
#[derive(Deserialize)]
struct ValidaQ { idproduto: i32, iddeposito: i32, quantidade: f64 }
/// BTN_AdicionarItem valida o estoque ANTES de incluir a linha (p.3): a tela chama isto a cada item.
async fn post_valida_estoque(State(db): State<Db>, Json(b): Json<ValidaQ>) -> Resp {
    let d = repo::valida_estoque(&*db.lock().await, b.idproduto, b.iddeposito, b.quantidade).await.map_err(erro)?;
    Ok(Json(json!({"disponivel": d})))
}
#[derive(Deserialize)]
struct ItemB { idproduto: i32, quantidade: f64 }
#[derive(Deserialize)]
struct VendaB { idcliente: i32, iddeposito: i32, vendedor: String, itens: Vec<ItemB>, perc_desconto: f64, parcelas: u32, #[serde(default)] confirmou_limite: bool }
async fn post_venda(State(db): State<Db>, Json(b): Json<VendaB>) -> Resp {
    let itens: Vec<repo::ItemNovo> = b.itens.iter().map(|i| repo::ItemNovo { idproduto: i.idproduto, quantidade: i.quantidade }).collect();
    let mut c = db.lock().await;
    Ok(Json(repo::fechar_venda(&mut c, b.idcliente, b.iddeposito, &b.vendedor, &itens, b.perc_desconto, b.parcelas, b.confirmou_limite).await.map_err(erro)?))
}
#[derive(Deserialize)]
struct DescQ { subtotal: f64, percentual: f64, tipo: String }
/// RecalculaTotais (p.3) pela API, para a tela mostrar o mesmo desconto que o servidor vai gravar.
async fn post_desconto(Json(b): Json<DescQ>) -> Resp {
    Ok(Json(json!({"desconto": regras::calcula_desconto(b.subtotal, b.percentual, &b.tipo).map_err(erro)?})))
}

async fn servir(porta: u16) -> Result<(), String> {
    let db: Db = Arc::new(Mutex::new(repo::conectar().await?));
    let app = Router::new()
        .route("/clientes", get(get_clientes))
        .route("/produtos", get(get_produtos))
        .route("/depositos", get(get_depositos))
        .route("/comissao", get(get_comissao))
        .route("/vendas", get(get_vendas).post(post_venda))
        .route("/estoque/valida", post(post_valida_estoque))
        .route("/desconto", post(post_desconto))
        .layer(CorsLayer::permissive())
        .with_state(db);
    let l = tokio::net::TcpListener::bind(("0.0.0.0", porta)).await.map_err(|e| e.to_string())?;
    println!("estoque-rs servindo em http://0.0.0.0:{porta}/  (GET /clientes /produtos /depositos /vendas /comissao · POST /estoque/valida /desconto /vendas)");
    axum::serve(l, app).await.map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r = match args.get(1).map(String::as_str) {
        Some("golden") => golden().await,
        Some("servir") => servir(args.get(2).and_then(|p| p.parse().ok()).unwrap_or(8080)).await,
        _ => Err("uso: estoque-rs golden | servir [porta]".into()),
    };
    if let Err(e) = r {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

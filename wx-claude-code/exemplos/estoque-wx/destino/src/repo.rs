//! Acesso ao PostgreSQL 16: o que no legado era HReadSeekFirst/HAdd/HModify sobre o HFSQL.
//! Cada funcao cita a procedure e a pagina do PDF de origem.
use serde::Serialize;
use serde_json::{json, Value};
use tokio_postgres::{Client, NoTls};

use crate::regras;

/// O Display do tokio_postgres diz so "db error"; a mensagem do servidor esta em as_db_error().
fn erro_db(e: tokio_postgres::Error) -> String {
    match e.as_db_error() {
        Some(d) => format!("{}: {}", e, d.message()),
        None => e.to_string(),
    }
}

pub async fn conectar() -> Result<Client, String> {
    // A senha vem do ambiente (letra K do questionario: credencial nunca no codigo).
    let senha = std::env::var("ESTOQUE_DB_PASS").map_err(|_| "ESTOQUE_DB_PASS nao definida".to_string())?;
    let host = std::env::var("ESTOQUE_DB_HOST").unwrap_or_else(|_| "localhost".into());
    let cfg = format!("host={host} user=estoque password={senha} dbname=estoque");
    let (client, conn) = tokio_postgres::connect(&cfg, NoTls).await.map_err(erro_db)?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("conexao: {e}");
        }
    });
    Ok(client)
}

fn num(v: &tokio_postgres::Row, i: usize) -> f64 {
    // NUMERIC chega como texto sem crate de decimal; o dado e dinheiro com 2 ou 3 casas, cabe em f64 para exibir.
    let s: String = v.get::<_, String>(i);
    s.parse().unwrap_or(0.0)
}

#[derive(Serialize)]
pub struct Comissao {
    pub vendedor: String,
    pub qtd_vendas: i64,
    pub total_vendido: f64,
    pub comissao: f64,
}

/// QRY-003 — QRY_ComissaoMensal, estoque-queries.pdf p.1: so STATUS = 'F'; 3 % calculado na query.
pub async fn comissao_mensal(c: &Client, mes: i32, ano: i32) -> Result<Vec<Comissao>, String> {
    let linhas = c
        .query(
            "SELECT vendedor, COUNT(idvenda), SUM(total)::text, ROUND(SUM(total) * 0.03, 2)::text \
             FROM venda WHERE EXTRACT(MONTH FROM data)::int = $1 AND EXTRACT(YEAR FROM data)::int = $2 AND status = 'F' \
             GROUP BY vendedor ORDER BY SUM(total) DESC",
            &[&mes, &ano],
        )
        .await
        .map_err(erro_db)?;
    Ok(linhas
        .iter()
        .map(|r| Comissao { vendedor: r.get(0), qtd_vendas: r.get(1), total_vendido: num(r, 2), comissao: num(r, 3) })
        .collect())
}

/// QRY_ClientesAtivos, estoque-queries.pdf p.2 — alimenta COMBO_Cliente de WIN_Venda.
pub async fn clientes_ativos(c: &Client) -> Result<Value, String> {
    let l = c
        .query("SELECT idcliente, nome, tipo, limite_credito::text FROM cliente WHERE ativo ORDER BY nome", &[])
        .await
        .map_err(erro_db)?;
    Ok(Value::Array(
        l.iter()
            .map(|r| json!({"idcliente": r.get::<_, i32>(0), "nome": r.get::<_, String>(1), "tipo": r.get::<_, String>(2).trim(), "limite_credito": num(r, 3)}))
            .collect(),
    ))
}

pub async fn produtos_ativos(c: &Client) -> Result<Value, String> {
    let l = c
        .query("SELECT idproduto, codigo, descricao, preco_venda::text FROM produto WHERE ativo ORDER BY descricao", &[])
        .await
        .map_err(erro_db)?;
    Ok(Value::Array(
        l.iter()
            .map(|r| json!({"idproduto": r.get::<_, i32>(0), "codigo": r.get::<_, String>(1), "descricao": r.get::<_, String>(2), "preco_venda": num(r, 3)}))
            .collect(),
    ))
}

pub async fn depositos(c: &Client) -> Result<Value, String> {
    let l = c.query("SELECT iddeposito, nome FROM deposito ORDER BY iddeposito", &[]).await.map_err(erro_db)?;
    Ok(Value::Array(l.iter().map(|r| json!({"iddeposito": r.get::<_, i32>(0), "nome": r.get::<_, String>(1)})).collect()))
}

/// BR-002 — ValidaEstoque, estoque-codigo.pdf p.1: nao vende o que nao tem no deposito escolhido.
/// As duas mensagens sao as do legado, porque a tela as mostra ao usuario.
pub async fn valida_estoque(c: &Client, idproduto: i32, iddeposito: i32, quantidade: f64) -> Result<f64, String> {
    let r = c
        .query_opt("SELECT quantidade::text FROM saldo WHERE idproduto = $1 AND iddeposito = $2", &[&idproduto, &iddeposito])
        .await
        .map_err(erro_db)?;
    let Some(r) = r else {
        return Err("Produto sem saldo cadastrado neste depósito.".into());
    };
    let disponivel = num(&r, 0);
    if disponivel < quantidade {
        // NumToString(SALDO.QUANTIDADE, "12.3f")
        return Err(format!("Estoque insuficiente. Disponível: {disponivel:.3}"));
    }
    Ok(disponivel)
}

pub struct ItemNovo {
    pub idproduto: i32,
    pub quantidade: f64,
}

/// Clique em BTN_Fechar (p.3-4) + BaixaEstoque (p.2) + GeraTitulos (p.4), numa transacao so:
/// o legado abria a transacao dentro de BaixaEstoque e gravava a VENDA antes, fora dela -- uma
/// venda 'A' orfa sobrava quando a baixa falhava. Aqui tudo ou nada, e a semantica visivel e a mesma:
/// a mensagem "Baixa cancelada: item N sem saldo." e o total, o desconto e as parcelas iguais.
pub async fn fechar_venda(
    c: &mut Client, idcliente: i32, iddeposito: i32, vendedor: &str, itens: &[ItemNovo], perc_desconto: f64, parcelas: u32, confirmou_limite: bool,
) -> Result<Value, String> {
    if itens.is_empty() {
        return Err("A venda não tem itens.".into());
    }
    let tx = c.transaction().await.map_err(erro_db)?;
    let cli = tx
        .query_opt("SELECT tipo, limite_credito::text FROM cliente WHERE idcliente = $1 AND ativo", &[&idcliente])
        .await
        .map_err(erro_db)?
        .ok_or("Escolha o cliente.")?;
    let tipo: String = cli.get::<_, String>(0).trim().to_string();
    let limite = num(&cli, 1);
    // TableAddLine: total do item = Round(qtd * preco, 2), preco lido do PRODUTO
    let mut linhas = Vec::new();
    let mut subtotal_c: i64 = 0;
    for it in itens {
        let p = tx
            .query_opt("SELECT preco_venda::text FROM produto WHERE idproduto = $1 AND ativo", &[&it.idproduto])
            .await
            .map_err(erro_db)?
            .ok_or("Escolha um produto.")?;
        if it.quantidade <= 0.0 {
            return Err("Quantidade deve ser maior que zero.".into());
        }
        let preco = num(&p, 0);
        let total_item_c = (it.quantidade * preco * 100.0).round() as i64;
        subtotal_c += total_item_c;
        linhas.push((it.idproduto, it.quantidade, preco, total_item_c));
    }
    // RecalculaTotais (p.3): acima do teto o legado zera o desconto e avisa
    let subtotal = regras::reais(subtotal_c);
    let desconto = regras::calcula_desconto(subtotal, perc_desconto, &tipo)?;
    let total = regras::reais(subtotal_c - (desconto * 100.0).round() as i64);
    // BR-007 (p.3): acima do limite pede confirmacao SO para cliente comum
    if total > limite && tipo == "C" && !confirmou_limite {
        return Err("CONFIRMAR: Total acima do limite de crédito do cliente. Fechar mesmo assim?".into());
    }
    let venda = tx
        .query_one(
            "INSERT INTO venda (idcliente, iddeposito, data, vendedor, subtotal, desconto, total, status) \
             VALUES ($1, $2, CURRENT_DATE, $3, $4::text::numeric, $5::text::numeric, $6::text::numeric, 'A') RETURNING idvenda",
            &[&idcliente, &iddeposito, &vendedor, &format!("{subtotal:.2}"), &format!("{desconto:.2}"), &format!("{total:.2}")],
        )
        .await
        .map_err(erro_db)?;
    let idvenda: i32 = venda.get(0);
    for (idproduto, qtd, preco, total_item_c) in &linhas {
        tx.execute(
            "INSERT INTO itemvenda (idvenda, idproduto, quantidade, preco_unitario, total_item) VALUES ($1, $2, $3::text::numeric, $4::text::numeric, $5::text::numeric)",
            &[&idvenda, idproduto, &format!("{qtd:.3}"), &format!("{preco:.2}"), &format!("{:.2}", regras::reais(*total_item_c))],
        )
        .await
        .map_err(erro_db)?;
        // BaixaEstoque (p.2): sem saldo, ou saldo menor, cancela tudo
        let n = tx
            .execute(
                "UPDATE saldo SET quantidade = quantidade - $3::text::numeric WHERE idproduto = $1 AND iddeposito = $2 AND quantidade >= $3::text::numeric",
                &[idproduto, &iddeposito, &format!("{qtd:.3}")],
            )
            .await
            .map_err(erro_db)?;
        if n == 0 {
            tx.rollback().await.map_err(erro_db)?;
            return Err(format!("Baixa cancelada: item {idproduto} sem saldo."));
        }
    }
    tx.execute("UPDATE venda SET status = 'F' WHERE idvenda = $1", &[&idvenda]).await.map_err(erro_db)?;
    // GeraTitulos (p.4): DateSys() + 30 * i
    let valores = regras::valores_das_parcelas(total, parcelas)?;
    for (i, v) in valores.iter().enumerate() {
        let dias = 30 * (i as i32 + 1);
        tx.execute(
            "INSERT INTO titulo (idvenda, vencimento, valor) VALUES ($1, CURRENT_DATE + $2::int, $3::text::numeric)",
            &[&idvenda, &dias, &format!("{v:.2}")],
        )
        .await
        .map_err(erro_db)?;
    }
    tx.commit().await.map_err(erro_db)?;
    Ok(json!({"idvenda": idvenda, "subtotal": subtotal, "desconto": desconto, "total": total, "parcelas": valores, "mensagem": format!("Venda {idvenda} fechada.")}))
}

/// QRY_VendasPeriodo, estoque-queries.pdf p.1 — WIN_ListaVendas.
pub async fn vendas_periodo(c: &Client, ini: &str, fim: &str, vendedor: &str) -> Result<Value, String> {
    let l = c
        .query(
            "SELECT v.idvenda, v.data::text, cl.nome, v.vendedor, v.total::text, v.status FROM venda v JOIN cliente cl ON cl.idcliente = v.idcliente \
             WHERE v.data BETWEEN $1::text::date AND $2::text::date AND ($3 = '' OR v.vendedor = $3) ORDER BY v.data DESC, v.idvenda DESC",
            &[&ini, &fim, &vendedor],
        )
        .await
        .map_err(erro_db)?;
    Ok(Value::Array(
        l.iter()
            .map(|r| json!({"idvenda": r.get::<_, i32>(0), "data": r.get::<_, String>(1), "cliente": r.get::<_, String>(2), "vendedor": r.get::<_, String>(3), "total": num(r, 4), "status": r.get::<_, String>(5).trim()}))
            .collect(),
    ))
}

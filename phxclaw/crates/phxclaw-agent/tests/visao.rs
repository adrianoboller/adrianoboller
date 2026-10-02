//! Print de tela -> telas ERP com as pecas reais: tesseract e o modelo de visao local.
//! Roda so com PHXCLAW_PROVA_VISAO=1 (cada print leva de segundos a minutos na CPU) e
//! com o tesseract e o modelo presentes; sem eles diz que pulou.

use phxclaw_agent::ui::ScreenshotToErpUiTool;
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_test_support::pulado;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn print_do_pedido_vira_mestre_detalhe() {
    if std::env::var("PHXCLAW_PROVA_VISAO").is_err() {
        pulado::pular("PHXCLAW_PROVA_VISAO", "PHXCLAW_PROVA_VISAO nao definida");
        return;
    }
    let print = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/ui-ir/pedido.png");
    if !std::path::Path::new(print).is_file() {
        pulado::pular(
            "target/ui-ir/pedido.png",
            "rode antes o teste navegador do phxclaw-ui-ir, que gera o print",
        );
        return;
    }
    let d = std::env::temp_dir().join(format!("phx-visao-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::copy(print, d.join("pedido.png")).unwrap();
    let c = ToolContext {
        task_id: "t".into(),
        workdir: d.clone(),
        timeout: Duration::from_secs(900),
    };
    let r = ScreenshotToErpUiTool
        .run(
            json!({"image":"pedido.png","table":"pedido","app_name":"Vendas","folder":"tela"}),
            &c,
        )
        .await
        .unwrap();
    eprintln!("{}", r.content);
    let sql = std::fs::read_to_string(d.join("tela/tela.sql")).unwrap();
    eprintln!("{sql}");
    for trecho in [
        "data_emissao date NOT NULL",
        "situacao varchar(40) NOT NULL",
        "valor_frete numeric(12,2)",
        "CREATE TABLE pedido_item",
        "valor_total numeric(12,2)",
    ] {
        assert!(sql.contains(trecho), "falta {trecho}");
    }
    // a mestre so tem os campos do formulario: colunas da grade e textos de exemplo fora
    let mestre = sql.split("CREATE TABLE pedido_item").next().unwrap();
    for intruso in [
        "produto",
        "quantidade",
        "preco_unitario",
        "selecione",
        "\n  r ",
    ] {
        assert!(
            !mestre.contains(intruso),
            "{intruso} na tabela mestre:\n{mestre}"
        );
    }
    assert!(r.content.contains("pedido_documento"), "{}", r.content);
}

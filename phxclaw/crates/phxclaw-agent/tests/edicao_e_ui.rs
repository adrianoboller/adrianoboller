//! edit_file, read_file com faixa e design_erp_ui chamados direto, sem modelo: o que se
//! prova e o contrato de cada ferramenta, inclusive as recusas.

use phxclaw_agent::ui::DesignErpUiTool;
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::json;
use std::time::Duration;

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-edicao-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(5),
    }
}

#[tokio::test]
async fn edit_file_troca_so_o_trecho_unico_e_recusa_ambiguo_e_ausente() {
    let c = ctx();
    std::fs::write(
        c.workdir.join("a.rs"),
        "fn a() {\n    1\n}\nfn b() {\n    1\n}\n",
    )
    .unwrap();
    let t = EditFileTool;
    // duas ocorrencias de "    1": recusa, e o arquivo fica intacto
    let e = t
        .run(
            json!({"path":"a.rs","old_text":"    1","new_text":"    2"}),
            &c,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&e, ToolError::InvalidArguments(m) if m.contains("2 vezes")),
        "{e}"
    );
    assert!(
        std::fs::read_to_string(c.workdir.join("a.rs"))
            .unwrap()
            .matches("    1")
            .count()
            == 2
    );
    // com a linha vizinha fica unico: so o b muda
    let r = t
        .run(
            json!({"path":"a.rs","old_text":"fn b() {\n    1","new_text":"fn b() {\n    2"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("linha 4"), "{}", r.content);
    assert_eq!(
        std::fs::read_to_string(c.workdir.join("a.rs")).unwrap(),
        "fn a() {\n    1\n}\nfn b() {\n    2\n}\n"
    );
    let e = t
        .run(
            json!({"path":"a.rs","old_text":"nao existe","new_text":"x"}),
            &c,
        )
        .await
        .unwrap_err();
    assert!(matches!(&e, ToolError::InvalidArguments(m) if m.contains("nao aparece")));
    // old_text vazio acrescenta no fim
    t.run(
        json!({"path":"a.rs","old_text":"","new_text":"// fim\n"}),
        &c,
    )
    .await
    .unwrap();
    assert!(
        std::fs::read_to_string(c.workdir.join("a.rs"))
            .unwrap()
            .ends_with("}\n// fim\n")
    );
    // fora da pasta: negado pela mesma confine das outras ferramentas
    let e = t
        .run(json!({"path":"../x","old_text":"","new_text":"x"}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)));
}

#[tokio::test]
async fn read_file_com_faixa_devolve_linhas_numeradas() {
    let c = ctx();
    std::fs::write(c.workdir.join("l.txt"), "um\ndois\ntres\nquatro\n").unwrap();
    let r = ReadFileTool
        .run(json!({"path":"l.txt","start_line":2,"end_line":3}), &c)
        .await
        .unwrap();
    assert_eq!(r.content, "    2\tdois\n    3\ttres");
    // sem faixa, continua o texto cru (comportamento velho)
    let r = ReadFileTool.run(json!({"path":"l.txt"}), &c).await.unwrap();
    assert_eq!(r.content, "um\ndois\ntres\nquatro\n");
    let e = ReadFileTool
        .run(json!({"path":"l.txt","start_line":9}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::InvalidArguments(_)));
}

#[tokio::test]
async fn design_erp_ui_grava_ir_e_html_do_sql_em_arquivo() {
    let c = ctx();
    std::fs::write(
        c.workdir.join("banco.sql"),
        "CREATE TABLE cliente (id serial PRIMARY KEY, nome varchar(80) NOT NULL);\n\
         CREATE TABLE pedido (id serial PRIMARY KEY, cliente_id int NOT NULL REFERENCES cliente(id), vl_total numeric(12,2));\n\
         CREATE TABLE pedido_item (id serial PRIMARY KEY, pedido_id int NOT NULL REFERENCES pedido(id), vl_total numeric(12,2));",
    )
    .unwrap();
    let r = DesignErpUiTool
        .run(
            json!({"sql_path":"banco.sql","app_name":"Vendas","folder":"telas"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("pedido_documento"), "{}", r.content);
    assert_eq!(r.artifacts.len(), 2);
    let ir: serde_json::Value =
        serde_json::from_slice(&std::fs::read(c.workdir.join("telas/ui-ir.json")).unwrap())
            .unwrap();
    assert_eq!(ir["name"], "Vendas");
    let html = std::fs::read_to_string(c.workdir.join("telas/index.html")).unwrap();
    assert!(html.contains("data-add-item"));
    // sem pedir React, nao grava React; pedindo, grava o projeto
    assert!(!c.workdir.join("telas/react").exists());
    let r = DesignErpUiTool
        .run(
            json!({"sql_path":"banco.sql","folder":"telas","react":true}),
            &c,
        )
        .await
        .unwrap();
    assert_eq!(r.artifacts.len(), 7, "html, ir e 5 arquivos do React");
    let telas = std::fs::read_to_string(c.workdir.join("telas/react/src/telas.jsx")).unwrap();
    assert!(telas.contains("export function TelaPedidoDocumento()"));
}

#[tokio::test]
async fn design_erp_ui_sem_tabela_recusa_em_vez_de_gravar_tela_vazia() {
    let c = ctx();
    let e = DesignErpUiTool
        .run(json!({"sql":"SELECT 1;"}), &c)
        .await
        .unwrap_err();
    assert!(
        matches!(&e, ToolError::InvalidArguments(m) if m.contains("nenhuma tabela")),
        "{e}"
    );
    assert!(!c.workdir.join("erp").exists());
}

//! Os dois backends de regras (Rust e WLanguage) saem do mesmo modelo. Prova:
//! 1. o crate Rust gerado COMPILA e o comportamento dele passa (cargo de verdade);
//! 2. o WLanguage carrega exatamente as mesmas mensagens que o Rust -- os dois nao
//!    podem recusar coisas diferentes com textos diferentes.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const PEDIDOS: &str = include_str!("fixtures/pedidos.sql");

const PROVA: &str = r#"
use vendas_regras::*;

fn pedido(cliente: i64) -> Pedido {
    Pedido {
        cliente_id: cliente,
        dt_emissao: Data::nova(15, 3, 2026),
        situacao: "aberto".into(),
        ..Default::default()
    }
}

#[test]
fn integridade_validacao_e_total() {
    let mut b = Banco::default();
    // so existe filho se o pai existir
    assert_eq!(
        b.incluir_pedido(pedido(1)),
        Err("Cliente: não existe cliente com esse código".into())
    );
    assert_eq!(
        b.incluir_cliente(Cliente::default()),
        Err("Preencha o campo Razão social".into())
    );
    let c = b
        .incluir_cliente(Cliente {
            razao_social: "ACME Ltda".into(),
            cnpj: "12.345.678/0001-90".into(),
            ativo: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(c, 1);
    let mut p = pedido(c);
    p.dt_emissao = Data::nova(31, 2, 2026);
    assert_eq!(b.incluir_pedido(p), Err("Data emissão não é uma data válida".into()));
    let mut p = pedido(c);
    p.situacao = "sumido".into();
    assert!(b.incluir_pedido(p).unwrap_err().starts_with("Situação fora das opções"));
    let ped = b.incluir_pedido(pedido(c)).unwrap();
    let prod = b
        .incluir_produto(Produto { descricao: "Parafuso".into(), preco: 105, ..Default::default() })
        .unwrap();
    for v in [1050, 123425] {
        b.incluir_pedido_item(PedidoItem {
            pedido_id: ped,
            produto_id: prod,
            quantidade: 1.0,
            preco_unitario: v,
            vl_total: v,
            ..Default::default()
        })
        .unwrap();
    }
    assert_eq!(b.total_pedido_vl_total(ped), 124475);
    // nunca se mata o pai que tem filhos
    assert_eq!(b.excluir_cliente(c), Err("Cliente tem pedidos ligados: exclua-os antes".into()));
    assert_eq!(b.excluir_pedido(ped), Err("Pedido tem itens ligados: exclua-os antes".into()));
    for i in [1, 2] {
        b.excluir_pedido_item(i).unwrap();
    }
    b.excluir_pedido(ped).unwrap();
    b.excluir_cliente(c).unwrap();
    assert_eq!(b.excluir_cliente(c), Err("Cliente não encontrado".into()));
    // codigo excluido nao volta: a sequencia so anda para frente
    let c2 = b
        .incluir_cliente(Cliente { razao_social: "Beta".into(), cnpj: "1".into(), ..Default::default() })
        .unwrap();
    assert_eq!(c2, 2);
    // tamanho da coluna
    let e = b
        .incluir_cliente(Cliente { razao_social: "x".repeat(121), cnpj: "1".into(), ..Default::default() })
        .unwrap_err();
    assert_eq!(e, "Razão social passa de 120 caracteres");
}

#[test]
fn calendario() {
    assert!(Data::nova(29, 2, 2024).valida());
    assert!(!Data::nova(29, 2, 2026).valida());
    assert!(!Data::nova(29, 2, 1900).valida());
    assert!(Data::nova(29, 2, 2000).valida());
    assert!(!Data::nova(31, 4, 2026).valida());
}
"#;

fn mensagens_rust(src: &str) -> BTreeSet<String> {
    src.split("Err(\"")
        .skip(1)
        .filter_map(|p| p.split("\".into())").next())
        .map(str::to_string)
        .collect()
}

fn mensagens_wl(src: &str) -> BTreeSet<String> {
    src.split("RESULT \"")
        .skip(1)
        .filter_map(|p| p.split('"').next())
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn rust_e_wlanguage_recusam_com_as_mesmas_mensagens() {
    let (app, _) = phxclaw_ui_ir::from_sql("Vendas", PEDIDOS);
    let rs = phxclaw_ui_ir::rust::render(&app);
    let wl = phxclaw_ui_ir::wlanguage::render(&app);
    let lib = &rs.iter().find(|(p, _)| p == "src/lib.rs").unwrap().1;
    let regras = &wl.iter().find(|(p, _)| p == "Regras.wl").unwrap().1;
    let (r, w) = (mensagens_rust(lib), mensagens_wl(regras));
    assert!(r.len() >= 8, "{r:?}");
    assert_eq!(r, w);
    // o WLanguage tem cada procedimento que o Rust tem, com o mesmo nome
    for p in [
        "PROCEDURE Validar_pedido()",
        "PROCEDURE Incluir_pedido_item()",
        "PROCEDURE Excluir_cliente(nCodigo)",
        "PROCEDURE Total_pedido_vl_total(nCodigo)",
    ] {
        assert!(regras.contains(p), "falta {p}");
    }
    assert!(lib.contains("pub fn total_pedido_vl_total(&self, codigo: i64)"));
}

#[test]
fn crate_rust_gerado_compila_e_o_comportamento_passa() {
    let (app, _) = phxclaw_ui_ir::from_sql("Vendas", PEDIDOS);
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/ui-ir/rust-regras"
    ));
    for (p, c) in phxclaw_ui_ir::rust::render(&app) {
        let alvo = dir.join(p);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(alvo, c).unwrap();
    }
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(dir.join("tests/prova.rs"), PROVA).unwrap();
    // alvo proprio: o cargo de fora segura a trava do target do workspace
    let out = Command::new(env!("CARGO"))
        .args(["test", "--offline", "-q"])
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .current_dir(&dir)
        .output()
        .unwrap();
    let txt =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "crate gerado falhou:\n{txt}");
    assert!(txt.contains("2 passed"), "{txt}");
}

//! O conselho de integradores pelo lado de fora da crate (prova F, 01/10/2026): o que os
//! unitarios do `gonogo.rs` nao fecham.

use phxclaw_agent::gonogo::{Conselho, Decisao, Identidade, Pedido, Veredito};
use std::path::PathBuf;

fn pasta(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-gonogo-f-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// NOGO cujos erros sao so espaco e o mesmo NOGO sem erros: recusado, e recusado ANTES de
/// gravar -- nem parecer na integracao, nem a identidade do integrador presa a esta tarefa
/// (o primeiro registro e o que grava a prova de quem ele e). O unitario so provava a lista
/// vazia; um `--erro " "` (ou `"errors": [""]` vindo do modelo) passava se o filtro dos
/// vazios sumisse, e o NOGO ficava sem dizer o que consertar.
#[test]
fn nogo_com_erros_em_branco_e_recusado_sem_gravar() {
    let d = pasta("branco");
    let c = Conselho::da_pasta_do_agente(&d);
    let quem = Identidade::Tarefa("tarefa-f".into());
    c.abrir("onda-7", &["A".into()], &quem).unwrap();
    let e = c
        .registrar(
            &Pedido {
                integracao: "onda-7".into(),
                integrador: "A".into(),
                parecer: Some(Veredito::NoGo),
                erros: vec!["   ".into(), String::new(), "\t".into()],
            },
            &quem,
        )
        .unwrap_err();
    assert!(e.contains("NOGO sem erros"), "{e}");
    let i = c.ler("onda-7").unwrap().expect("aberta");
    assert!(i.pareceres.is_empty(), "a recusa gravou parecer: {i:?}");
    assert!(
        !c.pasta.join("integradores.json").exists(),
        "a recusa prendeu a identidade de A a esta tarefa"
    );
    assert_eq!(
        c.decidir("onda-7").unwrap(),
        Decisao::Aguardar {
            faltam: vec!["A".into()]
        }
    );
    let _ = std::fs::remove_dir_all(&d);
}

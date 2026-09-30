//! Gera os backends de um SQL: `cargo run -p phxclaw-ui-ir --example gerar -- banco.sql Nome saida/`
//! Grava html/, react/, rust/ e wlanguage/ lado a lado, todos do mesmo UI-IR.

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [sql, nome, saida] = a.as_slice() else {
        eprintln!("uso: gerar <arquivo.sql> <Nome> <pasta-de-saida>");
        std::process::exit(2);
    };
    let sql = std::fs::read_to_string(sql).expect("ler o SQL");
    let (app, avisos) = phxclaw_ui_ir::from_sql(nome, &sql);
    for a in &avisos {
        eprintln!("aviso: {a}");
    }
    let base = std::path::Path::new(saida);
    let mut arquivos = vec![
        (
            "ui-ir.json".to_string(),
            serde_json::to_string_pretty(&app).unwrap(),
        ),
        ("html/index.html".into(), phxclaw_ui_ir::html::render(&app)),
    ];
    for (pasta, lista) in [
        ("react", phxclaw_ui_ir::react::render(&app)),
        ("flutter", phxclaw_ui_ir::flutter::render(&app)),
        ("rust", phxclaw_ui_ir::rust::render(&app)),
        ("wlanguage", phxclaw_ui_ir::wlanguage::render(&app)),
    ] {
        arquivos.extend(lista.into_iter().map(|(p, c)| (format!("{pasta}/{p}"), c)));
    }
    for (p, c) in &arquivos {
        let alvo = base.join(p);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(&alvo, c).unwrap();
    }
    println!("{} arquivos em {}", arquivos.len(), base.display());
}

//! `phxclaw fluxo pinar|exportar|importar` pelo processo de verdade: o teste
//! `cli_importar_exportar` do agente prova as funcoes do motor; este prova que a CLI e porta
//! fina para elas -- os argumentos chegam, a saida diz o que fez, e a recusa sai com codigo
//! diferente de zero e o motivo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn pasta(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cli-fluxo-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn phxclaw(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(args)
        .args(["--pasta", dir.join("agente").to_str().unwrap()])
        .env_remove("PHXCLAW_HOME")
        .current_dir(dir)
        .output()
        .unwrap()
}

fn texto(o: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn pinar_exportar_importar_pela_cli() {
    let d = pasta("pacote");
    let arq = d.join("origem.json");
    std::fs::write(
        &arq,
        r#"{"nome":"origem","etiquetas":["vendas"],"passos":[
            {"id":"a","ferramenta":"eco","args":{"texto":"x"}},
            {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}}!"}}]}"#,
    )
    .unwrap();
    let a = arq.to_str().unwrap();
    let o = phxclaw(&d, &["fluxo", "pinar", a, "a", "--json", r#"["PIN"]"#]);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("pinado a"), "{}", texto(&o));
    let pacote = d.join("pacote.json");
    let o = phxclaw(
        &d,
        &["fluxo", "exportar", a, "--saida", pacote.to_str().unwrap()],
    );
    assert!(o.status.success(), "{}", texto(&o));
    let destino = d.join("copia").join("importado.json");
    let imp = |p: &Path| {
        phxclaw(
            &d,
            &[
                "fluxo",
                "importar",
                p.to_str().unwrap(),
                destino.to_str().unwrap(),
            ],
        )
    };
    let o = imp(&pacote);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(destino.is_file());
    assert!(d.join("copia").join("importado.pins.json").is_file());
    // de novo: nao sobrescreve
    let o = imp(&pacote);
    assert!(!o.status.success());
    assert!(texto(&o).contains("ja existe"), "{}", texto(&o));
    // pacote editado no caminho: a conferencia (sha256) recusa
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pacote).unwrap()).unwrap();
    v["fluxo"]["passos"][0]["pin"] = serde_json::json!(["OUTRO"]);
    let torto = d.join("torto.json");
    std::fs::write(&torto, v.to_string()).unwrap();
    std::fs::remove_file(&destino).unwrap();
    std::fs::remove_file(d.join("copia").join("importado.pins.json")).unwrap();
    let o = imp(&torto);
    assert!(!o.status.success());
    assert!(texto(&o).contains("conferencia"), "{}", texto(&o));
    assert!(!destino.exists());
    let _ = std::fs::remove_dir_all(&d);
}

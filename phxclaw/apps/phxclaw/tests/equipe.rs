//! `phxclaw equipe` pelo processo de verdade, sem `PHXCLAW_AGENTES_DIR`: a pasta sai da
//! busca padrao (subindo a partir do executavel), que e o caminho de quem instalou.

use std::process::Command;

fn phxclaw(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(args)
        .env_remove("PHXCLAW_AGENTES_DIR")
        .env_remove("PHXCLAW_MODELO_LOCAL")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap()
}

#[test]
fn listar_mostra_todos() {
    let o = phxclaw(&["equipe", "listar"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let s = String::from_utf8(o.stdout).unwrap();
    let mut linhas = s.lines();
    // O total sai do indice dos papeis, nunca digitado aqui.
    let indice: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../config/agents/registry.index.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let n = indice["count"].as_u64().unwrap() as usize;
    assert_eq!(linhas.next(), Some(format!("{n} de {n} papeis").as_str()));
    assert_eq!(linhas.filter(|l| l.contains(" | ")).count(), n);
}

#[test]
fn mostrar_e_filtro() {
    let o = phxclaw(&["equipe", "mostrar", "8"]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("8 Johnson (agente)"));
    let o = phxclaw(&["equipe", "listar", "--texto", "johnson"]);
    let s = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(
        s.starts_with("1 de ") && s.lines().next().unwrap().ends_with(" papeis"),
        "{s}"
    );
}

/// Papel humano nao chama modelo nenhum: sai com 3 e o pedido de decisao, mesmo sem
/// Ollama nem chave de nuvem no ambiente.
#[test]
fn delegar_a_papel_humano_pede_decisao() {
    let pasta = std::env::temp_dir().join(format!("phx-cli-equipe-{}", std::process::id()));
    let o = phxclaw(&[
        "equipe",
        "delegar",
        "1",
        "aprovar a release",
        "--pasta",
        pasta.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("DECISAO HUMANA NECESSARIA"));
}

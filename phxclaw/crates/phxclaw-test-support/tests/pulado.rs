//! O registro do pulo: UMA linha JSON por pulo, com o crate e o nome do teste (a thread),
//! no `<target>/tmp/pulados.jsonl` resolvido pelo executavel -- vale no unitario, onde
//! `CARGO_TARGET_TMPDIR` nao existe.

use phxclaw_test_support::pulado;

#[test]
fn registra_crate_teste_recurso_e_motivo() {
    let reg = pulado::registro();
    assert!(reg.ends_with("tmp/pulados.jsonl"), "{}", reg.display());
    // O mesmo lugar que o cargo da aos testes de integracao.
    assert_eq!(
        reg,
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pulados.jsonl")
    );
    pulado::pular("recurso-de-prova", "motivo de prova");
    let texto = std::fs::read_to_string(&reg).unwrap();
    let linha = texto
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v["recurso"] == "recurso-de-prova")
        .expect("linha do pulo");
    assert_eq!(linha["crate"], "phxclaw-test-support");
    assert_eq!(linha["teste"], "registra_crate_teste_recurso_e_motivo");
    assert_eq!(linha["motivo"], "motivo de prova");
    // O registro e o mesmo da suite: a linha desta prova entraria no placar como pulo de
    // verdade (medido: 6 em vez de 5). Sai daqui, e so ela.
    let resto: String = texto
        .lines()
        .filter(|l| !l.contains("\"recurso-de-prova\""))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&reg, resto).unwrap();
    assert!(
        !std::fs::read_to_string(&reg)
            .unwrap()
            .contains("recurso-de-prova")
    );
}

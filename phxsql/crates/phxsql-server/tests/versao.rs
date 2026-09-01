//! `--version` responde sem config e sem servidor.
//!
//! O defeito que motivou: uma auditoria externa perguntou a versao dos tres
//! binarios e recebeu tres coisas diferentes, nenhuma delas a versao --
//! «comando desconhecido» do `phxsql`, erro de `config.json` ausente do
//! `phxsqld`, e erro de conexao recusada do `phxsqlcmd`, que tentava falar com
//! um servidor so para dizer quem era.
//!
//! Perguntar a versao e a PRIMEIRA linha de todo roteiro de operacao, e ela
//! nao pode exigir ambiente montado.

use std::process::Command;

/// Roda o binario num diretorio SEM `config.json`, que e o ponto: o defeito
/// antigo so aparecia fora do diretorio de trabalho arrumado.
fn versao(argumento: &str) -> (bool, String) {
    let temp = std::env::temp_dir();
    let saida = Command::new(env!("CARGO_BIN_EXE_phxsqld"))
        .arg(argumento)
        .current_dir(&temp)
        .output()
        .expect("nao consegui rodar o phxsqld");
    (
        saida.status.success(),
        String::from_utf8_lossy(&saida.stdout).trim().to_string(),
    )
}

#[test]
fn version_responde_sem_config_e_sem_servidor() {
    for argumento in ["--version", "-V"] {
        let (ok, linha) = versao(argumento);
        assert!(ok, "{argumento} devia sair com sucesso; saiu falhando");
        assert!(
            linha.starts_with("phxsqld "),
            "{argumento} devia se nomear: {linha:?}"
        );
        assert!(
            linha.contains(env!("CARGO_PKG_VERSION")),
            "{argumento} devia trazer a versao: {linha:?}"
        );
        // O commit e a metade que a auditoria pediu: versao sem commit nao
        // identifica build nenhum -- dois pacotes «0.18.0» podem ser arvores
        // diferentes.
        assert!(
            linha.contains('(') && linha.contains(')'),
            "{argumento} devia trazer a proveniencia entre parenteses: {linha:?}"
        );
    }
}

#[test]
fn as_duas_formas_dizem_a_mesma_coisa() {
    assert_eq!(versao("--version").1, versao("-V").1);
}

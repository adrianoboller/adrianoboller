//! O fecho de janela que sincroniza as K tabelas AO MESMO TEMPO nao pode gastar
//! menos `fsync` do que o que as sincronizava uma a uma.
//!
//! # O defeito que esta guarda existe para impedir
//!
//! O comboio do fecho (§12 do `docs/CONCORRENCIA.md`) e' 93-96% `fsync`, e o
//! conserto foi sincronizar as K tabelas sujas ao mesmo tempo em vez de em
//! laco. Um ganho de tempo num caminho de durabilidade tem exatamente uma
//! forma de ser falso: **ganhar tempo gastando menos `fsync`.** Um fio que
//! nunca foi lancado, um erro engolido no `join`, uma tabela que o arranjo
//! novo deixa de fora -- os tres aparecem como ganho e nenhum aparece como
//! defeito, porque o dado continua na tela.
//!
//! Entao a guarda nao mede tempo: conta os `fsync` dos DOIS arranjos, sobre as
//! mesmas K tabelas sujas, e exige que sejam **iguais**. Medido em 32 para
//! K=4 -- 8 por tabela, que e' a `TETO_FSYNC_POR_FECHO_V2`.
//!
//! # Por que por `strace`, e num processo filho
//!
//! Porque `fsync` que aconteceu ou nao e' fato do sistema operacional, e teste
//! unitario nao o observa -- a mesma lei que o `catraca-fsync-por-fecho` ja
//! aplica. E porque os contadores que a `Volumes` expoe medem a INTENCAO: o
//! `sincronizacoes()` e o `selo()` sobem ANTES do laco, e um deles subia com o
//! defeito do pedido 186 de pe. Quem conta o fato e' o traco do nucleo.
//!
//! Quem mede e' o exemplo `o-comboio-em-paralelo --contar`; este arquivo roda
//! o exemplo e cobra o veredito dele. Uma medicao so', num lugar so'.

/// O binario do exemplo, ao lado do binario deste teste.
fn caminho_do_exemplo() -> Option<std::path::PathBuf> {
    let eu = std::env::current_exe().ok()?;
    let deps = eu.parent()?; // target/<perfil>/deps
    for base in [deps.parent(), Some(deps)].into_iter().flatten() {
        let c = base.join("examples").join("o-comboio-em-paralelo");
        if c.exists() {
            return Some(c);
        }
    }
    None
}

#[test]
fn os_dois_arranjos_do_fecho_gastam_os_mesmos_fsync() {
    let Some(exemplo) = caminho_do_exemplo() else {
        panic!(
            "nao achei o binario do exemplo `o-comboio-em-paralelo` ao lado \
             deste teste. Rode `cargo test -p phxsql-store` (que compila os \
             exemplos) em vez de invocar o binario de teste na mao."
        );
    };
    let saida = std::process::Command::new(&exemplo)
        .arg("--contar")
        .output()
        .expect("rodar o exemplo o-comboio-em-paralelo");
    // 2 = esta maquina nao tem `strace`. Nao ha substituto.
    if saida.status.code() == Some(2) {
        eprintln!("strace nao esta instalado nesta maquina -- guarda pulada");
        return;
    }
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    assert!(
        saida.status.success(),
        "os dois arranjos do fecho gastaram numeros DIFERENTES de `fsync`. \
         Ganho de tempo num caminho de durabilidade que vem de `fsync` a menos \
         nao e' ganho -- e' durabilidade a menos.\n{texto}{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    // O numero em si tambem entra, para que a guarda reprove um arranjo que
    // gaste os mesmos `fsync` em arquivo nenhum que importe.
    let por_tabela: Vec<f64> = texto
        .lines()
        .filter_map(|l| l.split('(').nth(1))
        .filter_map(|r| r.split_whitespace().next())
        .filter_map(|n| n.parse().ok())
        .collect();
    assert_eq!(
        por_tabela.len(),
        2,
        "o exemplo nao imprimiu as duas linhas de contagem:\n{texto}"
    );
    for n in por_tabela {
        assert_eq!(
            n,
            phxsql_store::conferidor_fsync::TETO_FSYNC_POR_FECHO_V2 as f64,
            "o fecho gastou {n} fsync por tabela e a catraca esta em {}:\n{texto}",
            phxsql_store::conferidor_fsync::TETO_FSYNC_POR_FECHO_V2
        );
    }
}

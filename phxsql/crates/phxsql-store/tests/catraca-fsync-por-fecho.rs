//! A catraca do fecho de janela, cobrada: ele nao pode custar mais `fsync` do
//! que custa hoje.
//!
//! # Este arquivo nao mede mais nada -- e' de proposito
//!
//! Ele media, e por isso a catraca **nao existia para o inventario**. O
//! `docs/qa/medir.py` monta a tabela das catracas varrendo
//! `crates/*/examples/*.rs` atras de quem imprime `catraca:` e responde a
//! `--numeros`, e varre `crates/*/src/**/*.rs` atras de `pub const TETO*` para
//! achar teto sem medidor. Um teto medido dentro de um `tests/*.rs` escapa dos
//! dois crivos: nao entra na tabela e nem sequer aparece como buraco. Pelo
//! criterio escrito no proprio `docs/CATRACAS.md`, isso o torna **promessa, e
//! nao catraca**.
//!
//! O conserto foi por a catraca no formato que a casa ja usa, e nao mudar a
//! regua do `medir.py`: regua que passa a medir mais obrigaria a **aposentar**
//! as outras quatro catracas junto, e nao ha motivo para pagar isso aqui.
//! Entao:
//!
//! * o numero mora em `src/conferidor_fsync.rs`, onde os dois crivos o veem;
//! * quem mede e' o exemplo `fsync-por-fecho`, que se descreve;
//! * e este arquivo **roda o exemplo** e cobra o que ele reportou. Uma medicao
//!   so', num lugar so' -- catraca e inventario nao podem divergir porque nao
//!   ha duas contas.
//!
//! # O que substituiu o que
//!
//! A `TETO_FSYNC_POR_FECHO_V1` valia **7** e foi **aposentada**: o conserto
//! desta rodada acrescentou o `fsync` do `.reg` que faltava, e o numero real
//! subiu para 8 por CORRECAO. Catraca nao sobe nem quando a realidade sobe --
//! aposenta-se a antiga e nasce a nova, no numero medido do dia. Ver o motivo
//! inteiro em `src/conferidor_fsync.rs`.

use phxsql_store::conferidor_fsync::TETO_FSYNC_POR_FECHO_V2;

/// O binario do exemplo, ao lado do binario deste teste.
///
/// `cargo test` compila os exemplos junto, entao ele existe -- e procura-lo
/// pelo `current_exe()` e' o que faz o teste medir o MESMO perfil em que ele
/// proprio roda. Chamar `cargo run` daqui seria cargo dentro de cargo, com a
/// trava de build do cargo de fora ainda na mao.
fn caminho_do_exemplo() -> Option<std::path::PathBuf> {
    let eu = std::env::current_exe().ok()?;
    let deps = eu.parent()?; // target/<perfil>/deps
    for base in [deps.parent(), Some(deps)].into_iter().flatten() {
        let c = base.join("examples").join("fsync-por-fecho");
        if c.exists() {
            return Some(c);
        }
    }
    None
}

/// Recusa medir com binario VELHO.
///
/// `cargo test -p phxsql-store --test catraca-fsync-por-fecho` compila este
/// teste e **nao** compila os exemplos: o binario do medidor fica o da rodada
/// passada, e a catraca publica o numero de ontem. Foi assim que uma rodada
/// inteira de ganhos ficou invisivel na bancada desta casa -- `cargo build
/// --release` nao recompila example, e o medidor media o passado.
///
/// O crivo e' o mesmo que a lei descreve: fonte mais novo que o binario que o
/// mede. Comparar com o binario DESTE teste nao serviria -- mexer so' neste
/// arquivo o deixa mais novo que um exemplo que continua em dia.
fn mais_novo_que_o_exemplo(exemplo: &std::path::Path) -> Vec<String> {
    let Ok(bin) = exemplo.metadata().and_then(|m| m.modified()) else {
        return Vec::new();
    };
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut novos = Vec::new();
    let mut pilha = vec![raiz.join("src"), raiz.join("examples")];
    while let Some(dir) = pilha.pop() {
        let Ok(itens) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in itens.flatten() {
            let c = item.path();
            if c.is_dir() {
                pilha.push(c);
                continue;
            }
            if c.extension().is_some_and(|e| e == "rs")
                && item
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t > bin)
            {
                novos.push(c.display().to_string());
            }
        }
    }
    novos.sort();
    novos
}

/// Le a linha `catraca:` do exemplo e devolve os campos.
fn campos(saida: &str) -> Option<std::collections::HashMap<String, String>> {
    let linha = saida.lines().find(|l| l.starts_with("catraca:"))?;
    Some(
        linha["catraca:".len()..]
            .split(';')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect(),
    )
}

/// A catraca. **So desce.**
#[test]
fn fecho_de_janela_nao_pode_custar_mais_fsync_do_que_hoje() {
    let Some(exemplo) = caminho_do_exemplo() else {
        panic!(
            "nao achei o binario do exemplo `fsync-por-fecho` ao lado deste \
             teste. Ele e' quem mede esta catraca: rode \
             `cargo test -p phxsql-store` (que compila os exemplos) em vez de \
             invocar o binario de teste na mao."
        );
    };
    let velhos = mais_novo_que_o_exemplo(&exemplo);
    assert!(
        velhos.is_empty(),
        "o binario do medidor ({}) e' mais velho que {} arquivo(s) de fonte, e \
         medidor com binario velho mede o passado. Rode \
         `cargo test -p phxsql-store` (sem `--test`, que nao compila os \
         exemplos) ou `cargo build --examples -p phxsql-store` antes.\n{}",
        exemplo.display(),
        velhos.len(),
        velhos.join("\n")
    );
    let saida = std::process::Command::new(&exemplo)
        .arg("--numeros")
        .output()
        .expect("rodar o exemplo fsync-por-fecho");
    // 2 = esta maquina nao tem `strace`. Nao ha substituto de teste unitario:
    // `fsync` que aconteceu ou nao e' fato do sistema operacional.
    if saida.status.code() == Some(2) {
        eprintln!("strace nao esta instalado nesta maquina -- catraca pulada");
        return;
    }
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    assert!(
        saida.status.success(),
        "o exemplo fsync-por-fecho falhou ({:?}):\n{}\n{}",
        saida.status.code(),
        texto,
        String::from_utf8_lossy(&saida.stderr)
    );
    let c = campos(&texto).unwrap_or_else(|| {
        panic!("o exemplo nao imprimiu a linha `catraca:` com `--numeros`:\n{texto}")
    });
    let medido: usize = c["medido"].parse().expect("campo `medido` numerico");
    let valor: usize = c["valor"].parse().expect("campo `valor` numerico");

    // Se o exemplo e este teste discordarem do teto, ha duas contas -- e' o
    // defeito que este arquivo existe para nao ter.
    assert_eq!(
        valor, TETO_FSYNC_POR_FECHO_V2,
        "o exemplo reporta a catraca em {valor} e este teste cobra \
         {TETO_FSYNC_POR_FECHO_V2}: ha duas contas do mesmo numero"
    );
    assert!(
        medido <= TETO_FSYNC_POR_FECHO_V2,
        "o fecho da janela gastou {medido} fsync(s); o teto e' \
         {TETO_FSYNC_POR_FECHO_V2}. So desce -- se o `fsync` a mais vem de um \
         CONSERTO de verdade, aposente TETO_FSYNC_POR_FECHO_V2 e faca nascer \
         uma V3 no mesmo commit; nunca so suba este numero.\n\
         Relatorio inteiro:\n{texto}"
    );
    // O outro lado do laco: catraca frouxa nao segura nada. Quem baixar o
    // custo do fecho baixa o teto no mesmo commit.
    assert_eq!(
        medido, TETO_FSYNC_POR_FECHO_V2,
        "o fecho gasta {medido} e a catraca esta em {TETO_FSYNC_POR_FECHO_V2}: \
         baixe a catraca no mesmo commit, senao ela deixa de segurar"
    );
}

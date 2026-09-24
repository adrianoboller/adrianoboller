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

#[path = "apoio/exemplo.rs"]
mod exemplo;
use exemplo::{caminho_do_exemplo, campos, mais_novo_que_o_exemplo};

/// A catraca. **So desce.**
#[test]
fn fecho_de_janela_nao_pode_custar_mais_fsync_do_que_hoje() {
    let Some(exemplo) = caminho_do_exemplo("fsync-por-fecho") else {
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

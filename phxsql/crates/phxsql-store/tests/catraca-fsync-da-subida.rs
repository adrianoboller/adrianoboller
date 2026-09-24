//! A catraca da subida do byte 52, cobrada: a janela nao pode custar mais
//! `fsync` de subida do que custa hoje -- pedido 533.
//!
//! No molde da `catraca-fsync-por-fecho`: o numero mora em
//! `src/conferidor_fsync.rs` (onde o `docs/qa/medir.py` o ve), quem mede e' o
//! exemplo `fsync-da-subida` (que se descreve com `--numeros`), e este arquivo
//! roda o exemplo e cobra. Uma medicao so', num lugar so'.
//!
//! # Por que as duas catracas que ja existiam nao bastavam
//!
//! Medido pelo papel J com a subida sincronizada: a `TETO_FSYNC_POR_FECHO_V2`
//! ficou em 8 (conta so' o fecho) e a `alcancam-fsync-2` em 23 (conta secoes,
//! e as que passaram a alcancar `fsync` pela subida ja estavam la). As duas
//! sao cegas a um SEGUNDO `fsync` na subida. Esta nao.
//!
//! # E ela nao desce para zero sem desfazer o conserto
//!
//! Zero `fsync` na subida e o defeito do pedido 533 de volta, nao um ganho: o
//! recado da igualdade diz isso em vez de mandar baixar a catraca.

#[path = "apoio/exemplo.rs"]
mod exemplo;
use exemplo::{caminho_do_exemplo, campos, mais_novo_que_o_exemplo};

use phxsql_store::conferidor_fsync::TETO_FSYNC_DA_SUBIDA;

/// A catraca. **So desce** -- e nunca para zero.
#[test]
fn a_subida_do_byte_52_nao_pode_custar_mais_fsync_do_que_hoje() {
    let Some(exemplo) = caminho_do_exemplo("fsync-da-subida") else {
        panic!(
            "nao achei o binario do exemplo `fsync-da-subida` ao lado deste \
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
        .expect("rodar o exemplo fsync-da-subida");
    // 2 = esta maquina nao tem `strace`: `fsync` que aconteceu ou nao e' fato
    // do sistema operacional, e nao ha substituto de teste unitario.
    if saida.status.code() == Some(2) {
        eprintln!("strace nao esta instalado nesta maquina -- catraca pulada");
        return;
    }
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    assert!(
        saida.status.success(),
        "o exemplo fsync-da-subida falhou ({:?}):\n{}\n{}",
        saida.status.code(),
        texto,
        String::from_utf8_lossy(&saida.stderr)
    );
    let c = campos(&texto).unwrap_or_else(|| {
        panic!("o exemplo nao imprimiu a linha `catraca:` com `--numeros`:\n{texto}")
    });
    let medido: usize = c["medido"].parse().expect("campo `medido` numerico");
    let valor: usize = c["valor"].parse().expect("campo `valor` numerico");

    assert_eq!(
        valor, TETO_FSYNC_DA_SUBIDA,
        "o exemplo reporta a catraca em {valor} e este teste cobra \
         {TETO_FSYNC_DA_SUBIDA}: ha duas contas do mesmo numero"
    );
    assert!(
        medido <= TETO_FSYNC_DA_SUBIDA,
        "a subida do byte 52 gastou {medido} fsync(s) numa janela; o teto e' \
         {TETO_FSYNC_DA_SUBIDA}. Um segundo `fsync` na subida entra no laco \
         quente de toda escrita -- se ele vem de um CONSERTO de verdade, \
         aposente TETO_FSYNC_DA_SUBIDA e faca nascer uma V2 no mesmo commit; \
         nunca so suba este numero.\nRelatorio inteiro:\n{texto}"
    );
    assert!(
        medido > 0,
        "a janela abriu SEM `fsync` na subida do byte 52: e o defeito do pedido \
         533 de volta (o 1 fica so no cache do nucleo, e uma queda de energia \
         guarda paginas novas sob o 0). Nao baixe a catraca -- ponha o \
         `fdatasync` de volta em `NdxFile::levantar_marca`.\n{texto}"
    );
    assert_eq!(
        medido, TETO_FSYNC_DA_SUBIDA,
        "a subida gasta {medido} e a catraca esta em {TETO_FSYNC_DA_SUBIDA}: \
         baixe a catraca no mesmo commit, senao ela deixa de segurar"
    );
}

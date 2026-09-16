//! A catraca do fecho de janela: quantos `fsync` ele custa.
//!
//! O numero mora aqui, em `src/`, e nao no arquivo de teste que o cobra, por
//! dois motivos que sao o mesmo motivo:
//!
//! * o `docs/qa/medir.py` monta o inventario das catracas varrendo
//!   `crates/*/examples/*.rs` atras de quem imprime `catraca:` e responde a
//!   `--numeros` -- um teto que vive so' dentro de um `tests/*.rs` **nao
//!   aparece no inventario**, e pelo criterio do proprio `docs/CATRACAS.md`
//!   isso o torna promessa, e nao catraca;
//! * e o mesmo `medir.py` varre `crates/*/src/**/*.rs` atras de `pub const
//!   TETO*` para achar catraca que ninguem mede. Constante fora de `src/` nao
//!   e' vista por nenhum dos dois lados: nem como medida, nem como buraco.
//!
//! Quem mede e' o exemplo `fsync-por-fecho`, que se descreve; quem cobra e' o
//! teste `tests/catraca-fsync-por-fecho.rs`, que roda o exemplo.

/// Quantos `fsync` um fecho de janela pode custar. **So desce.**
///
/// # Ela SUBSTITUI a `TETO_FSYNC_POR_FECHO_V1`, que valia 7
///
/// A V1 media o fecho de uma tabela reaberta e achava **7** arquivos:
/// `.trash .bin .memo .log .reason .ndx .ndx`. Faltava o `.reg`, e faltava
/// por defeito -- `Volumes::sincronizar` so' alcancava o cache de descritores
/// desta instancia, e um `RegFile` recem-aberto nunca poe nada la. O conserto
/// desta rodada acrescentou o oitavo `fsync`, que e' NECESSARIO: o dado.
///
/// Catraca nao sobe, nem quando a realidade sobe por um conserto -- a lei
/// desta casa manda **aposentar** a antiga e fazer nascer uma nova, com nome
/// novo, no numero medido do dia, dizendo no proprio nome que substitui a
/// outra. E' o que o `TETO_TABELA_NA_MAO` ja registrou quando a regua das
/// grades mudou. A serie com o passado se perde de proposito: perder a
/// comparacao e' mais barato que deixar "mudei a regua" virar a porta pela
/// qual se afrouxa um teto.
///
/// # O que ela mede, e o que ela NAO mede
///
/// Mede o **custo** do fecho: quantos `fsync` ele gasta. Nao mede a
/// **correcao** do fecho -- essa e' da guarda
/// `tests/fecho-da-janela-sincroniza-o-reg.rs`, que cobra a IDENTIDADE do
/// arquivo (existe `fsync` sobre um `.reg`?) e nao a contagem. As duas
/// precisam existir separadas: um oitavo `fsync` solto por descuido, em
/// arquivo nenhum que importe, passaria pela guarda e e' exatamente o que
/// esta catraca reprova.
///
/// # Por que ele continua em 8 depois do pedido 258
///
/// Quatro dos oito arquivos -- `.trash`, `.reason`, `.bin`, `.memo` -- um
/// `inserir` comum nao muda, e desde 16/09/2026 `Volumes::sincronizar` os
/// PULA quando estao batizados (levados ao disco por este processo alguma
/// vez) e sem marca de escrita. Mas o que este medidor traca e' o PRIMEIRO
/// fecho de uma familia num processo novo (`fsync-por-fecho --sonda`, num
/// filho sob `strace`), e esse fecho nao pula ninguem, de proposito: a pagina
/// suja que um processo morto deixou no nucleo nao tem marca em RAM, e o
/// primeiro `fsync` e' o unico que a alcanca. Entao o numero daqui mede o
/// arranque, e o arranque continua custando 8. Quem desce e' o segundo fecho
/// em diante -- e quem o mede e' a guarda `tests/fsync-por-operacao.rs`, por
/// contador, e o `--example fsync-por-operacao`, pelo nucleo. Regua que
/// passasse a medir o segundo fecho seria OUTRA catraca, com outro nome; esta
/// nao se remenda. Ver `docs/DESEMPENHO.md` §24.
pub const TETO_FSYNC_POR_FECHO_V2: usize = 8;

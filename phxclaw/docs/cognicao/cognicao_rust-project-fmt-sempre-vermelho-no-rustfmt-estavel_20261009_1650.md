# `rust_project fmt` saía vermelho em todo projeto: o rustfmt estável recusa `--check` com JSON

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/tests/evolucao.rs` — com o comando antigo, o ciclo verde
parava em `portao:fmt` com a ajuda do `cargo fmt` como detalhe (medido em 09/10/2026); com o
conserto, os 4 testes passam e `portao_vermelho_nao_gera_ramo` exige `src/lib.rs:1` no detalhe.
Leitor novo provado em `sistema::tests::fmt_le_o_diff_em_texto_do_rustfmt_estavel`.

## O que aconteceu

O portão `fmt` da auto-evolução usa o `rust_project`, e o ciclo verde nunca ficava verde. A ação
`fmt` (sem `fix`) mandava `cargo fmt --check --message-format json`; o rustfmt 1.9.0 estável
responde «cannot include --check arg when --message-format is set to json», imprime a ajuda e sai 1.
Sem `--check`, o `json` é emissão instável («using an unstable value without --unstable-features»).
Nenhum teste chamava `rust_project fmt`, então o defeito vivia calado.

## O que eu concluí primeiro, e estava errado

Que o `portao_vermelho_nao_gera_ramo` provava o portão vermelho. Ele passou na primeira corrida —
porque o `fmt` estava SEMPRE vermelho. O teste conferia o veredito e não o motivo: é o «teste que
passa por engano» de novo, e só o ciclo verde falhando mostrou.

## O que a medição disse

`cargo fmt --check -- --color never` devolve texto estável (`Diff in ARQ:LINHA:` e as linhas `+`
esperadas) e sai 1 com diferença, 0 sem. Lido assim, o vermelho cita o arquivo e a linha.

## A regra

Teste de portão vermelho confere o MOTIVO do vermelho (o arquivo, a linha), não só o veredito:
portão quebrado também dá vermelho.

## Como está guardado hoje

`sistema.rs::diagnosticos_do_fmt` lê o texto, com o porquê no comentário; o teste do vermelho
da evolução exige o trecho.

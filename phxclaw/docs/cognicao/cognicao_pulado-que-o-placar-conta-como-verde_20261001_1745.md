# PULADO num `eprintln!` é verde no placar, e nem a linha aparece

**Estado:** FRUTÍFERO

**Evidência:** `segredos` com `PHXCLAW_GITLEAKS_BIN=/nao/existe` deu `test result: ok. 4 passed` e **0** linhas de registro antes; com `tests/comum/pulado.rs`, o mesmo `4 passed` e **4** linhas JSON em `target/tmp/pulados.jsonl` (corrida normal: 0 linhas). Medido em cópia isolada da crate, `CARGO_TARGET_DIR` próprio, 01/10/2026.

## O que aconteceu

O Integrador notou que `segredos.rs` imprime PULADO e sai `ok` sem gitleaks/bwrap. Medido: o placar (`gerar_dossie.py` lê `N passed; N failed; N ignored`) conta o teste pulado como **passou**, e o libtest **captura** o `eprintln!` de teste que passa — sem `--nocapture`, a linha PULADO nem sai na tela. O mesmo padrão existe em mais 21 pontos de 7 arquivos (`grep -rn PULADO crates/`), e três testes do `contexto_dados.rs` (`prova_real_de_visao_quando_apontada`, `gabarito_real_…`, `corpus_real_…`) voltam `return` sem dizer nada.

## O que eu concluí primeiro, e estava errado

1. Que a saída certa era uma variável `PHXCLAW_EXIGIR_PROVA` que transforma o pulo em falha. A catraca do `tools/config_catalogo.py` reprova todo nome `PHXCLAW_*` fora do catálogo — inclusive em `tests/` (as variáveis de prova vivem como `testes.*` no catálogo). Decidir «exigir» fica no **portão**, que lê o registro; o teste só registra.
2. Que `writeln!(arquivo, "{linha}")` com `O_APPEND` era uma escrita só. Não é: o `File` sem buffer faz uma chamada por pedaço do `format_args`. Na primeira medição, 4 testes em paralelo deixaram duas linhas coladas e uma vazia. Conserto: `write_all(format!("{linha}\n"))` — 4 linhas, todas JSON.

## A regra

Teste que depende de recurso da máquina **registra** o pulo onde o portão conta; o `eprintln!` não é registro. Quem exige a prova é o portão da máquina que tem o recurso (pulo de recurso exigido = NoGo), não uma variável nova.

## Como está guardado hoje

`crates/phxclaw-agent/tests/comum/pulado.rs` (`pulado::pular(recurso, motivo)`; o nome do teste sai da thread do libtest), usado no `segredos.rs`. Falta: o portão apagar `target/tmp/pulados.jsonl` antes da suíte e publicar «N passam, dos quais P pulados»; os outros 21 PULADO migrarem.

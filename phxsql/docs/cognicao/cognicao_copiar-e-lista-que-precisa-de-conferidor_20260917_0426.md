# Cognição — `COPIAR` é lista, e a terceira falta pediu um conferidor, não uma quarta linha

17/09/2026 04:26 UTC. Papel G (QA), onda 3 da rodada de replicação
(`docs/pmo/RODADA-2026-09-17-replicacao.md`).

## 1. O que aconteceu

O integrador (papel A) rodou os 13 `--so` da minha lista de ontem
(`docs/propostas/inventario-qa-replicacao-2026-09-17.md`) às 03:53–03:57 UTC.
Sete guardas ficaram **PROVADA**, uma **QUEBRADA** (`trava-atras-da-rede`,
tratada à parte nesta mesma rodada) e **cinco** voltaram sem veredito — todas
`phxsql-server --lib`: `colisao-de-sequence-calada`,
`replicacao-do-cluster-em-claro`, `posicao-sem-portao`,
`replica-insiste-na-credencial-recusada`, `cluster-devolve-a-credencial-na-tela`.

O motivo, nos cinco casos, era o mesmo: a árvore LIMPA da cópia do provador
reprovava em `segredos::testes::todo_parametro_com_cara_de_segredo_esta_na_lista`
— um teste que nem tinha relação com a família da replicação — porque esse
teste lê `bancada/guardas/debug-com-segredo.py` em tempo de execução, por
`Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bancada/guardas/debug-com-segredo.py")`,
e a lista `COPIAR` do `provar-guardas.py` (`crates`, `docs`, `testes-web`,
`exemplos`, `Cargo.toml`/`.lock`) nunca levava `bancada/`. A árvore limpa não
era limpa: estava faltando um arquivo, e a falta derrubava a base inteira
antes de qualquer defeito ser reposto.

## 2. O que eu concluí primeiro, e estava errado

A primeira leitura foi «falta uma entrada em `COPIAR`, acrescento
`bancada/guardas/debug-com-segredo.py` e resolvido» — exatamente o que o
`LEIA-ME.md` já tinha feito duas vezes antes (`docs/ROTEIRO-1.0.md` em
`error.rs`, depois `testes-web/botoes-exercitados.txt` em
`conferidor_botoes.rs`, cada vez com o mesmo texto: «quando um gerador
depende de uma lista, a lista tem de sair do código» — e cada vez a correção
foi só acrescentar a linha que faltava). Eu ia fazer a terceira cópia dessa
mesma correção.

Rodei um grep completo de `CARGO_MANIFEST_DIR` em `crates/**/*.rs` antes de
escrever a linha, só para ter certeza do texto exato do caminho — e apareceram
**15 ocorrências em 10 arquivos**, não uma. Duas delas liam fora do crate por
um caminho que `COPIAR` também não cobria
(`crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs`, que lê
`bancada/concorrencia/mapa-das-threads.py`) — um segundo buraco, do mesmo
tamanho, que **não tinha doído ainda** porque nenhuma entrada do catálogo usa
esse alvo `--test` hoje. Ele ia doer no dia em que alguém escrevesse uma
guarda para essa catraca.

## 3. O que a medição disse

- **15** usos de `CARGO_MANIFEST_DIR` em `crates/**/*.rs`, em **10** arquivos.
- Só **2** liam fora do crate um caminho que `COPIAR` não cobria, e os dois
  estão em `src/`/`tests/` (o que o provador de fato compila):
  `crates/phxsql-server/src/segredos.rs` → `bancada/guardas/debug-com-segredo.py`
  e `crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs` →
  `bancada/concorrencia/mapa-das-threads.py`.
- **2** liam fora do crate em `examples/` (`custo-da-colmeia.rs`,
  `custo-do-vizinho.rs`, ambos apontando para `bancada/`) — e não importam
  para o provador, porque `cargo test --lib`/`--test X` nunca compila
  `examples/`. Medido, não suposto: por isso ficaram fora do escopo do
  conferidor novo (`_fontes_em_escopo()` varre só `src/` e `tests/`).
- `bancada/` inteira mede **2,6 GiB** (medido agora, `du -sh`) contra os
  **5 MB** que o comentário de `COPIAR` promete — acrescentar a pasta inteira
  em vez dos dois arquivos teria multiplicado o custo da cópia por ~500×.

## 4. A regra

**Lista da qual um gerador depende não se conserta linha a linha na terceira
vez — se conserta com um conferidor que varre o código e prova que a lista
está completa.** A primeira falta de uma lista assim é acidente; a segunda é
padrão; a terceira é a hora de escrever a pergunta geral («todo caminho que o
código lê fora daqui está coberto?») em vez de responder de novo a pergunta
específica («este arquivo está na lista?»). É o mesmo `CLAUDE.md` («a receita
de um número também envelhece») visto de um ângulo que ele ainda não tinha
nomeado: não é só o NÚMERO que precisa de gerador — a LISTA que alimenta um
executor também precisa de um conferidor que a meça contra o código, ou ela
envelhece do mesmo jeito, uma vez por leitura nova que alguém escrever.

## 5. Como está guardado hoje

`bancada/guardas/provar-guardas.py` ganhou `verificar_copiar()` (mais
`_helpers_de_raiz`, `_leituras_fora_do_crate`, `_fontes_em_escopo`), chamável
por `--conferir-copiar`, e a prova real contra a árvore de verdade —
`--autoteste-copiar` — confere os dois sentidos: sem
`bancada/guardas/debug-com-segredo.py` em `COPIAR` ele reprova nomeando o
arquivo e quem o lê; com a lista de hoje, passa. Os dois arquivos que
faltavam entraram em `COPIAR`; a pasta `bancada/` inteira, não — o comentário
ao lado explica por quê (2,6 GiB medidos).

O buraco não fechado: o conferidor varre por convenção de nomes desta casa
(`fn raiz*() -> PathBuf`) e por dois idiomas de escape (`.parent()`/
`.ancestors()` e `.join("..")`) — é heurística, não um parser de Rust. Um
terceiro idioma para "sair do crate" que ninguém usou ainda passaria batido.
Isso está dito no comentário do código, não só aqui.

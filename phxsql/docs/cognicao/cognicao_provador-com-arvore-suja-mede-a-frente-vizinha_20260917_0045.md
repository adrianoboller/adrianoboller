# Provador com árvore suja mede a frente vizinha

**Descoberto em 17/09/2026, 00:45 UTC.** Frente F com o chapéu de G, ao rodar
a sonda do raio das sete guardas novas da pétrea da senha.

## 1. O que aconteceu

Às 00:25 o provador tinha dado árvore limpa **verde** (1.107 testes no
`phxsql-server --lib`). Às 00:45 a mesma cópia devolveu **1 vermelho**:
`idiomas::testes::todo_texto_da_fabrica_e_pedido_por_alguem` — «`tela.sv_sub_no_ar`
está na fábrica e nenhuma tela o pede». Não era meu: `git status` mostrava
`idiomas.rs` e `ui/index.html` sujos, trabalho em curso da frente dos
idiomas. O `provar-guardas.py` copia a árvore de **trabalho** por conteúdo
(`_sincronizar`), então o portão «a árvore limpa não está verde; nada aqui
prova nada» reprovou por motivo alheio.

## 2. O que eu concluí primeiro, e estava errado

Primeiro pensei que a minha inserção no catálogo tivesse quebrado algo — e
não podia: o catálogo não entra no `cargo test`. Depois pensei em esperar a
frente vizinha terminar, que é tempo sem dono. A regra de encontro do
orquestrador (nada de `stash`/`checkout`/`restore`/`reset` na árvore
compartilhada) fechava o atalho óbvio.

## 3. O que a medição disse

`git archive HEAD Cargo.toml Cargo.lock crates exemplos docs testes-web
bancada/guardas | tar -x -C <scratchpad>/wt` (34 MB, lê o *object store* e
não toca a árvore de trabalho), com o `catalogo.py` e o `trecho-vivo.py`
desta frente por cima, e o provador chamado **de lá**
(`python3 wt/bancada/guardas/provar-guardas.py`, `RAIZ = AQUI/../..`). A
cópia quente ressincronizou só os dois arquivos, recompilou o servidor, e a
árvore limpa voltou verde: 1.107 / 348 / 59. Nove guardas provadas em 8 min.

## 4. A regra

**Com frentes paralelas, o provador se aponta para um `git archive HEAD` no
scratchpad, nunca para a árvore de trabalho — e nunca se limpa a árvore de
trabalho para medir.**

## 5. Como está guardado hoje — e onde o buraco ficou

Na §15.7.7 do `docs/CATRACAS.md` (achado 4) e aqui. **O buraco:** o
`provar-guardas.py` tem `--arvore` para o **destino** da cópia, e não para a
**origem** — a receita acima é manual. Um `--raiz <dir>` (ou a origem por
variável de ambiente) no executor tornaria isto uma linha; é do papel G.

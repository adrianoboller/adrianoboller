# Dois worktrees com o mesmo `CARGO_TARGET_DIR` sobrescrevem um ao outro

**Estado:** INFRUTÍFERO (a tentativa falhou; causa e prevenção abaixo)

## O que aconteceu

A árvore compartilhada não compilava (duas frentes editando `config.rs`/`ide.rs` ao vivo), o disco
tinha 2,7 GB livres e o `target/` 15 GB. Abri um `git worktree` do commit base no scratchpad e
apontei `CARGO_TARGET_DIR` para o `target/` da árvore principal, para reaproveitar as
dependências. Funcionou para a primeira rodada; na segunda, o `phxclaw-agent` do worktree foi
compilado contra o `.rlib` de `phxclaw-config-runtime` da árvore PRINCIPAL (com o campo `perfil`
que só existe lá): `error[E0063]: missing field perfil` num fonte que não tem `perfil`.

## O que eu concluí primeiro, e estava errado

Que o caminho do pacote entrava no hash dos artefatos e dois worktrees conviveriam no mesmo
`target/`. Não entra: para membro do workspace o cargo calcula o `-C metadata` com o id do
pacote **relativo à raiz do workspace** (reprodutibilidade), então `arvore/phxclaw/crates/x` e
`phxclaw/crates/x` dão o MESMO hash, e o fingerprint decide «fresco» pelo mtime — o fonte do
worktree (checkout antigo) perde para o da árvore principal (editado agora).

## O que a medição disse

Rodada 1 no worktree: 9/9 guardas, 4/4 MCP, 3/3 adaptadores verdes. Rodada 2 (depois de a outra
frente compilar): erro E0063 em dois lotes seguidos; `cargo check -v` mostra `Dirty ... the file
has changed 1h 58m after last build` para 120 crates — cada lado invalida o outro inteiro.

## A regra

Worktree só com `target/` próprio. Sem disco para isso, a frente edita a árvore compartilhada e
roda os portões lá, aceitando esperar a compilação das outras frentes — compartilhar o `target/`
custa uma recompilação completa por troca de lado e pode entregar binário de outro fonte.

## Prevenção

Antes de apontar `CARGO_TARGET_DIR` para um `target/` em uso por outra árvore, conferir se é
outro workspace com a mesma estrutura: se for, não compartilhar.

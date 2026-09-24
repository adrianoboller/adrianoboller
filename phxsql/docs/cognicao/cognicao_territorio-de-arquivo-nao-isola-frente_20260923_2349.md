# Cognição — território de arquivo não isola frente, no mesmo worktree

**Descoberto em** 23/09/2026, 23:49 · papel A (integrador)

## 1. O que aconteceu

O integrador despachou três frentes em paralelo no **mesmo worktree**, e o
critério de colisão usado foi **por arquivo**: cada frente com território
próprio, nenhum arquivo em comum entre elas.

Em cerca de uma hora, três interferências medidas, nenhuma delas por arquivo
compartilhado:

1. `cargo fmt --all` rodado por uma frente formatou o arquivo de outra
   (relatado pela frente do pedido 435).
2. A suíte inteira da frente 435 deu **2.820 verdes e 0 vermelhos, mas
   `rc=1`**: o doctest do `phxsql-cmd` não compilou —
   `extern location for phxsql_server does not exist: …/libphxsql_server-23c6f607a03edb8c.rlib`
   — porque outra frente recompilou o servidor no mesmo `target` e trocou o
   `.rlib` no meio da corrida. Registrado no próprio commit `a272d8f`: «Um
   alvo nao rodou -- o doctest do phxsql-cmd, que nao compilou porque outra
   frente trocou o .rlib do servidor no mesmo target no meio da corrida. Isso
   nao e teste reprovado, e esta escrito como alvo nao rodado em vez de
   arredondado para verde.»
3. Logo depois, a árvore **inteira** deixou de compilar: `email.rs:97`, `no
   method named as_bytes found for Result` — a frente do pedido 372 mudou
   `cfg.senha()` para devolver `Result` (o conserto certo, para não deixar
   senha vazando por erro de tipo) e o chamador em `email.rs` ainda não tinha
   acompanhado a mudança. Confirmado ao vivo: no momento desta cognição,
   `email.rs:72` já chama `cfg.senha()?` (com `?`), e o arquivo segue com
   alteração não commitada — a frente 372 estava, de fato, no meio da
   correção.

A frente 435 também rodou `git checkout` no próprio arquivo de teste no meio
do trabalho; atingiu só o arquivo dela, mas em worktree compartilhado é
exatamente o tipo de comando que perde trabalho alheio sem conflito nenhum
aparecer na tela.

## 2. O que eu concluí primeiro, e estava errado

Concluí que separar as frentes **por arquivo** bastava para isolá-las — que
duas frentes sem interseção de arquivos não podiam se atrapalhar. Não bastou:
elas compartilham o `target` de compilação, o efeito de `cargo fmt --all`
sobre o workspace inteiro, e o **estado de compilação do crate**, que é
global e não por arquivo. Território de arquivo separa o que cada frente
*escreve*; não separa o que cada frente *lê pronto* (o `.rlib` compilado) nem
o que ela executa sobre a árvore inteira (`fmt --all`).

## 3. O que a medição disse

| medida | número/fato |
|---|---|
| interferências medidas em ~1 hora, nenhuma por arquivo compartilhado | 3 |
| resultado da suíte da frente 435 | 2.820 passaram, 0 falharam, `rc=1` (1 alvo não compilou) |
| causa do `rc=1` | `.rlib` do `phxsql_server` trocado por outra frente durante a corrida |
| ponto de quebra de compilação da árvore inteira | `email.rs:97`, `Result` sem `.as_bytes()` |
| frente que causou a mudança de tipo (correta) | pedido 372 (senha do DbLink) |
| comandos identificados como risco de perda silenciosa em worktree compartilhado | `cargo fmt --all`, `git checkout`/`stash`/`restore` |

## 4. A regra

**Território de arquivo não isola frentes que compartilham `target`, `cargo
fmt --all` e o estado de compilação do crate.** «Suíte verde» de qualquer
frente isolada passou a significar «verde numa árvore que nenhum commit
sozinho vai conter» — o commit de cada frente carrega só os arquivos dela, e
o estado de compilação que ela mediu pertence à árvore inteira no instante em
que mediu, não ao que ela vai entregar.

## 5. Como está guardado hoje

- O pedido/commit da frente 435 foi registrado com o alvo que não rodou
  **dito**, não arredondado para verde (é a própria disciplina que este
  arquivo de cognição descreve, já aplicada antes de esta cognição existir).
- Contrato das frentes seguintes, imposto pelo orquestrador: nunca
  `cargo fmt --all`, nunca `git checkout`/`stash`/`restore`, e erro de
  compilação num arquivo de outra frente não é seu para consertar nem para
  contar contra a sua.
- Número de frentes que **compilam** ao mesmo tempo passou a ser limitado; o
  resto do time trabalha em código que não compila até a vez dele.
- Por que não worktree isolado por frente: um `target` novo custa vários GiB
  cada, e o disco está em ~2 GiB livres — não cabe uma cópia por frente nesta
  rodada.
- **Não guardado ainda**: não há guarda automática que detecte troca de
  `.rlib` em voo nem que impeça `cargo fmt --all` de rodar num worktree com
  frentes concorrentes — a prevenção hoje é só o contrato verbal do
  orquestrador com cada frente. Papel G/D, se quiser fechar o buraco com
  script.

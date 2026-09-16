# O registro de escritas em RAM é cego ao processo que morreu — e por isso o `fsync` seletivo precisa de batismo, por volume

*Descoberto em 16/09/2026, 10:30, ao desenhar o conserto do pedido 258 (pular
o `fsync` do arquivo limpo) e confirmado pelo parecer do papel C às 10:35.*

## 1. O que aconteceu

O pedido 258 dizia: `Volumes::sincronizar` manda `fsync` a todo descritor
aberto sem perguntar quem mudou — 8 por inserir, 9 por excluir. O motor já
tinha o sinal de sujeira (`ESCRITAS_PENDENTES`, o registro do processo por
família de arquivos, alimentado por `marcar_escrito` em todo caminho de
escrita do `Volumes`), lido **só para somar** `fsync` desde a §16.2 do
`DESEMPENHO.md`, que recusou lê-lo para subtrair.

Medido antes, pelo núcleo (`--example fsync-por-operacao`, `strace -f -y`):
inserir 8 `fsync`, **4 em arquivo limpo**; atualizar 8, 5 limpos; excluir 9,
3 limpos. A premissa do pedido estava certa.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que bastava ler o registro no outro sentido**: `alvos = os volumes
com marca`, e o descritor aberto sem marca é limpo por definição, porque toda
escrita passa pelo `Volumes` e marca. A auditoria confirma que toda escrita
passa por lá (e os três caminhos fora dele — `ndx.rs`, os `.novo` do
`reg.rs`, o `restaurar.rs` — pagam o próprio `sync_all`). Parecia fechado.

**Estava errado num caso que nenhum teste unitário vê**: o registro vive em
RAM, e RAM morre com o processo. Processo A grava um `COMMIT` — marca `.tx`
com `sync_all`, slot no `.reg` ainda no cache do núcleo —, a janela não fecha,
A leva `SIGKILL`. A página suja sobrevive no núcleo (é o cego do pedido 186).
Processo B arranca, `transacao::recuperar` acha a marca, lê o slot (que está
lá, vindo do cache), conclui «já aplicada», chama `sincronizar` e apaga a
marca. Com «alvos = só os marcados», o registro de B **nasceu vazio**: zero
`fsync` no `.reg`, marca apagada, e uma queda de energia perde um commit
confirmado sem bilhete nenhum. Hoje isso não acontece porque a lista
`abertos` cobre: `RegFile::sincronizar` abre o volume 1 e o `fsync` acontece.
A lista `abertos` **não é cache** — é a única cobertura entre processos que o
motor tem.

E o segundo erro, menor: a marca nascia **depois** do `write_all`. Um
`write_all` que falha no meio deixa página suja sem marca. Inofensivo enquanto
`abertos` cobria tudo; furo no dia em que «limpo» passasse a ser pulado.

## 3. O que a medição disse

- O sinal correto tem **duas** metades, e a segunda é a que faltava:
  `escritos` (a marca, agora posta em `Volumes::arquivo(volume, true)`, antes
  do `write`) e `batizados` — os volumes que **este processo** já levou ao
  disco alguma vez. Pula-se só quem está batizado e sem marca.
- **Por volume, e não um bit por família**: um volume do meio que só entra
  no cache depois do primeiro fecho, aberto para ler, nunca foi batizado e
  paga o `fsync` dele na primeira vez — exatamente o que pagava antes. Um bit
  por família o pularia, calado.
- Depois, pelo núcleo: inserir 8 → 4, atualizar 8 → 4, excluir 9 → 6. Tempo
  em máquina parada, faixas que não se cruzam: inserir 1,21×–1,45×, atualizar
  1,31×–1,39×, excluir 1,15×–1,20× — o custo do `fsync` limpo (52–54 µs),
  quatro por operação, e não 2×.
- A versão ingênua **passa em todos os testes do caso comum** — é por isso que
  ela é tentadora. Só cai nos dois testes do processo morto
  (`o_primeiro_fecho_do_processo_nao_confia_no_registro` e o do volume do
  meio), que existem por causa deste arquivo. Guarda `fsync-so-dos-escritos`,
  provada 2/2.
- A catraca `TETO_FSYNC_POR_FECHO_V2` **não desce**: ela traça o primeiro
  fecho de um processo novo, que não pula ninguém de propósito. Quem desce é
  o segundo fecho, e ele tem guarda própria.

## 4. A regra

**Sinal em RAM só prova o que ESTE processo fez; o primeiro `fsync` de cada
descritor num processo é incondicional, porque é o único que alcança o que o
processo anterior deixou no núcleo.**

## 5. Como está guardado hoje

- `crates/phxsql-store/src/volume.rs`: `Registro { escritos, batizados }`,
  a marca em `arquivo(volume, true)`, o filtro em `sincronizar_listas`, e
  cinco testes (os dois do processo morto, o do batismo, o da marca antes do
  `write`, e `nenhum_caminho_de_escrita_novo_fora_do_volumes`).
- `crates/phxsql-store/tests/fsync-por-operacao.rs`: 2, 2 e 4 arquivos das
  sete famílias por inserir, atualizar e excluir depois do batismo, e a
  tabela reaberta herdando o batismo (o caminho do servidor).
- `bancada/guardas/catalogo.py`: `fsync-do-arquivo-limpo` e
  `fsync-so-dos-escritos`.
- `docs/FORMATO.md` §8 e `docs/DESEMPENHO.md` §24.
- **Onde o buraco ficou**: o `.ndx` continua pagando dois `fsync` por
  `sincronizar` mesmo limpo (o do atualizar sem mudança de chave). Ele não
  passa pelo `Volumes`, e o byte de sujo do cabeçalho dele responde outra
  pergunta. É item separado, não esquecimento.

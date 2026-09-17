# A regra do zelador dizia quando NÃO apagar; não dizia quando apagar vale a pena

**Descoberta:** 17/09/2026, 00:32 UTC, ao medir o par fria/quente do
`provar-guardas.py` depois de o zelador ter apagado o cache dele duas vezes na
mesma noite.

## 1. O que aconteceu

O `zelador.sh` apagou `/root/.cache/phx-guardas` às 23:00 (**892 MiB**) e às
00:09 (**1.432 MiB**), com a nota «o provador recria». Entre as duas corridas,
duas frentes o tinham reconstruído inteiro: é onde o `provar-guardas.py` guarda
o `target/` quente entre rodadas (`CARGO_TARGET_DIR` fixo em `alvo/`), e
reconstruir é compilar frio.

O zelador cumpriu a regra dele — provou por `cwd` em `/proc` que nenhum
processo vivo usava o diretório. A regra estava certa e estava **incompleta**:
ela diz quando não apagar e não diz quando apagar compra alguma coisa.

E, medindo, apareceu um buraco na prova que existia: o provador só tem `cwd`
no cache **dentro do `cargo` filho**. Entre duas guardas — enquanto repõe e
desfaz o defeito, julga e imprime — nenhum processo tem `cwd` ali, e a seção
antiga podia apagar o cache **no meio de uma rodada**. A tranca do próprio
provador (`flock` em `phx-guardas.tranca`, segurada do início ao fim) é a prova
que faltava, e o `flock -n` a lê sem tocar em nada.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** a corrida fria de `comando-invalido-vira-texto-cru` deu **48,7 s**
contra **5,2 s** quente, e eu atribuí a diferença à cópia «nova» da árvore —
34 MB de `docs/`, `crates/`, `exemplos/`, arquivo a arquivo. Medida isolada num
`--arvore` de rascunho, a cópia custa **0,07 s**. Os 34 s eram o
`flock /tmp/phx-cargo.lock` esperando o `cargo` de uma frente vizinha: o
diretório do cache nasceu às 00:32:12 (`mtime`) e a corrida tinha começado às
00:31:38. A fria de verdade é ~15 s.

**Segundo, e pior:** escrevi no comentário do zelador «51,6 s frio contra
3,5 s quente» para a árvore limpa de `phxsql-server --lib` **antes de a corrida
quente terminar**. Medida, deu **27,2 s** — porque o `garantir_frescor` do
provador põe data de agora em todo arquivo que o catálogo sabe mutar, e o
`cargo` recompila o crate em toda chamada, com ou sem cache. O cache poupa a
compilação das **dependências**, não a do crate mutado. Número digitado antes
da medição, no arquivo que existe para não digitar número.

**Terceiro, do briefing que recebi:** a opção «apagar só o subdiretório por
guarda não tocado há N horas» supõe um cache **por guarda**. Não há: é uma
árvore só, com um `alvo/` compartilhado por 170 guardas e 30 binários de teste
(90 delas usam o mesmo `phxsql-server --lib`). A unidade de idade é o cache
inteiro.

**Quarto, na própria prova, duas vezes seguidas:** a prova «quente abaixo do
piso» deu «EXISTE (errado)» e eu quase a li como defeito do script. Na primeira,
o `kill` no `flock … sleep 20 &` da prova anterior matou o `flock` e deixou o
`sleep` filho vivo **com o descritor da tranca herdado** — a tranca continuou
tomada, o zelador disse «rodada em curso, nao toco» (certo, para o lado
seguro), e o meu `grep` não casava essa linha. Na segunda, uma vizinha começou
a compilar, o portão de medição recusou com `exit 3`, e o mesmo `grep` engoliu
a recusa: saída vazia, fixture intacto. Só passou quando mostrei a saída
**inteira**. Prova que filtra a saída esconde justamente o motivo de ter
falhado — e `flock` segurado por um processo não se solta matando o pai.

## 3. O que a medição disse

Uma corrida de cada, com frentes vizinhas no ar (o `flock` do compilador
serializa; a CPU não):

| o que | frio | quente | o cache poupa |
|---|---|---|---|
| `phxsql-sql --lib`, árvore limpa (254 testes) | 13,6 s | 2,4 s | **11,2 s** |
| `phxsql-server --lib`, árvore limpa (1.107 testes) | 51,6 s | 27,2 s | **24,4 s** |
| uma guarda do servidor (mutação + recompilação + testes) | ~32 s | ~31 s | nada — é o desenho do provador |
| `montar` a cópia da árvore (34 MB, por conteúdo) | 0,07 s | 0,04 s | — |

Tamanho do cache: **177 MiB** só com o binário do `phxsql-sql`; **808 MiB** com
o do servidor (613 MiB é `alvo/debug/incremental`); **3,0 GiB** com todos os
30, medido em 05/09.

Frequência de uso: **16 commits** passaram por `bancada/guardas/` em 30 h
(16/09 06:36 → 17/09 00:12), a maior lacuna entre dois foi **3 h 45 min**
(17:45 → 21:30). Durante a medição, uma frente vizinha entrou na tranca no
segundo em que a soltei (00:38:28) e usou o binário que a minha corrida acabara
de compilar.

O disco, e onde ele está (`/dev/vda`, 258 GiB de tamanho, **4,1 GiB livres**
às 00:28 e **2,5 GiB** às 00:43, com uma vizinha compilando um exemplo
`--release`):

| item | tamanho | o zelador |
|---|---|---|
| `phxsql/target/` (deps 4,2 · examples 3,0 · incremental 2,7) | **10,2 GiB** | não toca: sempre há um shell com `cwd` na árvore |
| `~/.rustup` | 2,6 GiB | não toca |
| `bancada/phxsql/` (`precos.*`, 10 M linhas, `.gitignore` diz «refaz-se com `bancada/medir.py`») | **2,4 GiB**, sem escrita desde **29/08** | não enxerga |
| `/root/.claude/projects` (transcritos) | 0,97 GiB | não enxerga |
| `/tmp/android-ndk-r27c.zip`, desde 04/09 | 633 MiB | não enxerga |
| `target/` e `target-phx/` no scratchpad de uma frente | 366 MiB, escritos hoje | não enxerga (e não deve: estão em uso) |
| **cache do provador** | **0,8–1,4 GiB, refeito na hora seguinte** | apagava de hora em hora |

O espaço que o zelador liberava era o único da lista que voltava sozinho.

## 4. A regra

**Cache de trabalho só sai quando está frio — sem uso há mais que a maior
lacuna medida entre dois usos — ou quando o disco está abaixo do piso; e a
prova de uso vivo inclui a tranca de quem o usa, não só o `cwd`.** Espaço que
é reocupado pelo mesmo conteúdo na hora seguinte não é espaço ganho; é CPU e
E/S de quem está trabalhando.

## 5. Como está guardado hoje

- **`phxsql/zelador.sh`, seção «copias derivadas fora do repositorio»:** o
  cache fica se está em uso (`cwd` **ou** tranca tomada, `flock -n`); sai se
  está frio (`QUENTE_MIN=360`: 6 h, acima da lacuna de 3 h 45 min e dentro de
  uma noite); sai quente só se o disco está abaixo de `PISO_MIB=2048` — o
  mesmo piso do `comunicacao.sh`, cujo comentário «é o mesmo que o zelador usa»
  era falso até aqui — ou com `--mesmo-assim`. O último uso é o mais novo entre
  a tranca, `alvo/debug/deps` e o diretório. Em todo caso ele **imprime** a
  decisão com idade e tamanho, e a linha do `target` que ele não toca passou a
  dizer quanto é (10.431 MiB), porque quem lê o relatório procurando disco
  precisa ver onde ele está.
- **A prova nos dois sentidos** roda contra um cache de mentira apontado por
  `PHX_CACHE_GUARDAS` (quente fica; com `touch -d '7 hours ago'` sai; quente com
  `PHX_PISO_MIB` alto sai «abaixo do piso»; com a tranca segurada por um
  `flock` vizinho «rodada em curso, nao toco»). As duas variáveis existem para
  isso; sem elas o script é o de sempre.
- **O buraco que fica, nomeado:** `bancada/phxsql/` — 2,4 GiB parados há 19
  dias, regeneráveis por script — é a maior mesa que o zelador não enxerga, e
  é decisão da bancada (papel F), não do zelador, porque regenerar 10 milhões de
  linhas custa o que custa. O `PISO_MIB=2048` agora mora em **dois** scripts;
  número em dois lugares diverge, e juntá-los é um arquivo que os dois leiam. E
  o incentivo que a regra cria: quem quer disco amanhã roda `./zelador.sh` e lê
  o que ele decidiu não fazer, com o número — o cache quente não é onde o disco
  está.

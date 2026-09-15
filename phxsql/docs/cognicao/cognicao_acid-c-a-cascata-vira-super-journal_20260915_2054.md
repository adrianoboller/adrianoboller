# Cognição: a cascata do `ao_alterar` vira escrita da lista, não sombra da mãe

- **Assunto:** ACID-C — a cascata escrevia fora do conjunto de escrita da transação
- **Descoberto:** 2026-09-15, ~20:54 (papéis A+C+B+F, fechando o ACID-C sobre a
  fundação do P0)
- **Arquivos:** `crates/phxsql-store/src/table.rs`
  (`planejar_cascata_para_lista`, `atualizar_sem_cascata_com_maes`,
  `EscritaDaCascata`), `crates/phxsql-server/src/servidor.rs`
  (`empilhar_atualizar_com_cascata`, `empilhar_grupo`, `aplicar_conjunto`),
  `crates/phxsql-server/src/transacao.rs` (marca v3, `aplicar_uma`)

## 1. O que aconteceu

Dentro de uma transação, `BEGIN; UPDATE mãe.chave 1→2; COMMIT` deixava a filha
para trás na visão da transação: a mãe voltava `id:2` pelo read-your-own-writes,
mas a filha voltava `cliente_id:1`. O `COMMIT` respondia `gravadas:1` com **duas**
tabelas alteradas, e a cascata só acontecia no `aplicar_conjunto` — então o
`ROLLBACK` nunca a alcançava e a marca `.tx` não a descrevia. Era o **C** do
ACID parcial: a cascata escrevia numa tabela que a transação não declarara.

A pesquisa do DBA (`docs/propostas/dba-bases-2026-09.md` §1.1) já apontava o
conserto: o *super-journal* do SQLite enumera **tudo** que a transação vai
tocar antes do `fsync`, e o InnoDB nunca teve o buraco porque há **um objeto por
tabela**. O conserto: a cascata **inteira** entra no conjunto de escrita — mãe e
cada filha viram uma `Escrita` própria, na ordem pai-antes-de-filha, e cada elo
aplica SEM re-cascatear (a corrente já é a lista).

## 2. O que eu concluí primeiro, e estava errado

**Errei o DESENHO antes de errar o código, e a medição de um caso desfez o
medo.**

- Primeiro pensei em manter a mãe cascateando no commit **e** empilhar as filhas
  como escritas «de sombra», só para o read-your-own-writes e a contagem. Isso dá
  **duas representações da mesma filha** — a implícita (cascata da mãe) e a
  explícita (a sombra) —, e elas divergem dentro da própria transação: altere a
  chave da mãe (a sombra guarda a filha em X) e depois altere a filha
  diretamente; a sombra fica velha, e a mãe re-cascateando no commit sobrescreve
  a alteração direta. É exatamente o «segundo lugar para a mesma verdade
  divergir» que a pétrea condena. **Uma fonte de verdade: a lista.** A mãe aplica
  achatada, a filha é escrita da lista.

- Depois temi uma **regressão**: a cascata de hoje lê a filha FRESCA no commit e
  só troca as colunas da FK, preservando uma alteração concorrente de outra
  coluna; um retrato tirado no `empilhar` sobrescreveria essa alteração. Medindo
  o caminho de hoje, o medo caiu: a cascata do commit abre a filha num **segundo
  descritor** (`planejar_ao_alterar` chama `Table::abrir`), desconectado do
  handle que a passada está escrevendo — então uma filha inserida antes na mesma
  transação **já batia na guarda de visibilidade do `.ndx`** (o mesmo defeito do
  P0) ou não era vista. O canto não funcionava antes; não é regressão. E o que
  fecha o retrato do `empilhar` é **reservar a tabela filha**: travada, ela não
  muda até o commit.

- Terceiro, achei que dava para travar as filhas com a trava de dados na mão.
  Não dá: a espera por trava não pode segurar o servidor inteiro (lição do
  comboio). Daí as **três fases**: descobrir sob a trava de dados, travar as
  filhas fora dela, e **replanejar** sob a trava com as filhas já travadas — sem
  a terceira fase, uma escrita na fresta entre 1 e 2 deixaria a lista com um
  retrato velho.

## 3. O que a medição disse

- **Prova real nos dois sentidos, medida:** reposto o defeito (o `empilhar` não
  expande a cascata), os dois testes de comportamento **falham** — a filha volta
  `1` no read-your-own-writes e o `COMMIT` conta `1`; com o conserto, `2` e `2`.
- **A marca não precisou de byte por versão:** a v3 acrescenta um byte
  `cascata_na_lista` por operação, no fim do payload, depois da linha antiga — o
  leitor da v1/v2 nunca chega até ali, e o CRC continua cobrindo o payload de uma
  vez. A recuperação decide por ele: `true` aplica achatado (sem re-cascatear),
  `false` mantém o `recascatear` da v2. As duas convivem.
- **Custo desligado é zero:** só o `atualizar` que muda chave conferida planeja a
  cascata; o portão `alguma_coluna_indexada_mudou` do `planejar_ao_alterar` sai
  na primeira linha sem abrir filha nenhuma. As tabelas filhas só são travadas
  quando há cascata de verdade.
- **1037 testes do servidor + 177 do store + a bateria inteira do workspace**
  passam; `fmt` e `clippy --all-targets` sem aviso.

## 4. A regra

**Dentro da transação, a cascata do `ao_alterar` é a própria lista de escrita —
mãe e filhas achatadas, na ordem pai-antes-de-filha, cada elo aplicado sem
re-cascatear. Nunca duas representações da mesma filha; nunca um retrato de filha
não travada. Fora da transação, a cascata segue acontecendo dentro do
`atualizar`, e ali não é atômica por desenho.**

## 5. Como está guardado hoje

- `Table::planejar_cascata_para_lista` confere a árvore inteira (recusa antes de
  gravar) e a achata em `Vec<EscritaDaCascata>`; `atualizar_sem_cascata_com_maes`
  aplica o elo como um `atualizar` INTEIRO (índice, diário, trilha mantidos), só
  sem a recursão da cascata — **não** é o «atalho por baixo» que o
  `aplicar_ao_alterar` recusa.
- `empilhar_atualizar_com_cascata` faz as três fases (descobrir → travar →
  replanejar) e `empilhar_grupo` empilha mãe+filhas de uma vez, contando o teto
  pelo total e reservando cada tabela tocada; em `STRICT`, uma filha não
  declarada é recusada nomeando a tabela (guarda nova entra pedida).
- Marca **v3** (`VERSAO=3`, byte `cascata_na_lista`); `aplicar_uma` da
  recuperação e `aplicar_conjunto` do commit honram o byte.
- Provas: `acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao`,
  `acidc_o_rollback_desfaz_a_cascata`,
  `acidc_a_marca_v3_recupera_a_cascata_achatada` (todas em `servidor.rs`).
- **Onde o buraco fica:** uma filha inserida por OUTRA conexão sob a chave velha,
  entre o `empilhar` e o `COMMIT`, é um fantasma que a cascata não vê —
  consistente com o `READ COMMITTED` desta casa (a §4.1 do `ACID.md` já mede o
  fantasma como *acontece*). E a leitura repetível continua sem existir: é ela,
  não mais a cascata, que derruba *ACID compliant* seco.

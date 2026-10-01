# Existir não depende de tique de relógio; o conteúdo depende — o retrato da FASE A ganhou os ausentes e ficou com o furo da atualização no lugar

**Estado:** PENDENTE

## O que aconteceu

Pedido 427. O retrato da FASE A (`reg.rs`, `retratar`) fotografava só os
volumes que existiam; um volume nascido no meio dela não tinha `*.novo` e a
FASE B não o trocava. O pedido dizia que a consequência tinha sido **lida** e
não executada. Executada aqui por um gancho novo
(`panico_de_teste::armar_gancho`, no `Ponto::FaseADepoisDoRetrato`, passado
pelas duas FASES A): o vizinho abre a tabela por fora do congelamento e grava
a linha 91, que faz nascer `clientes#004.reg`.

## O que eu concluí primeiro, e estava errado

Duas hipóteses antes de medir:

- **H1 (morta no ext4):** «nascer um volume não toca os existentes, então o
  retrato passa». Morreu medida: o `mtime` do `clientes#001.reg` andou 5,9 ms,
  porque os contadores da tabela moram no cabeçalho do volume 1 e o `inserir`
  o regrava. No tique fino, o retrato velho **já pegava** o nascimento.
- **H2 (viva):** «o único sinal do nascimento nos volumes velhos é o `mtime`
  do volume 1 — o tamanho de nenhum muda». Medido: tamanhos 3.900 → 3.900 nos
  três, `mtime` só do volume 1. Com o tique grosso (FAT/exFAT 2 s, HFS+/NFS
  1 s, **simulado** devolvendo o `mtime` com `File::set_modified`), o retrato
  passa, a FASE B troca os três velhos e a tabela **não abre** (`Corrompido`).

O erro que eu quase cometi depois: achar que comparar o **conteúdo** do
cabeçalho do volume 1 (128 bytes, barato) fecharia todo o caso da
granularidade. A sonda mediu por operação os bytes do cabeçalho que mudam:
`inserir` 8, `excluir_suave` 5, `restaurar` 5, `excluir_de_vez` 5 —
`atualizar` **zero**.

## O que a medição disse

- Com o tique grosso simulado e o conserto **tirado**: os dois testes do cinto
  caem (`expect_err` no `conferir_retrato` e no `redeclarar_depois_de_conferir`);
  com o conserto, os dois passam e a tabela reabre com as 91 linhas.
- **O furo que fica:** com o tique grosso simulado, um `atualizar` da linha 45
  (volume 2) entre as fases passa no retrato e a linha volta a `c45` depois da
  troca — perda **calada**. Só acontece se um escritor escapar do
  congelamento, que é justamente o caso que a conferência existe para cobrir.

## A regra

Para saber se um conjunto de arquivos mudou, compare **existência** (que não
tem tique) antes de comparar `mtime` (que tem); e antes de trocar `mtime` por
«conteúdo do cabeçalho», meça quais operações mexem nele — a atualização no
lugar não mexe.

## Como está guardado hoje

- `crates/phxsql-store/tests/volume-que-nasce-na-fase-a.rs`: os dois cintos
  (acrescentar coluna e o irmão, a regravação de esquema), o defeito executado
  (`sem_o_cinto_...`, verde com e sem o conserto) e o comportamento velho do
  tique fino.
- Guarda `retrato-da-fase-a-nao-ve-volume-que-nasce-427`, PROVADA 2/2.
- `docs/FORMATO.md`, na seção da FASE A fora da trava.
- **O buraco:** a atualização no lugar com tique grosso **não** está guardada.
  Fechá-la pede um contador de escrita no cabeçalho do volume 1, que é mudança
  de formato (papel C). E o NTFS não foi medido — está fora desta máquina.
- **Atualização (pedido 634, 01/10/2026):** o buraco fechou **sem** mudar o
  formato — o papel C recusou o contador, e a FASE B passou a perguntar se o
  `mtime` ainda é a sentinela plantada no retrato. Ver
  `cognicao_selo-de-mtime-troca-andou-por-ainda-e-a-sentinela_20261001_2220.md`.

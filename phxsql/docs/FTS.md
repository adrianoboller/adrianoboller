# O `.fts`: achar uma palavra sem ler a linha

> **O que ele compra está medido, e o que ele NÃO compra também.** A busca de
> hoje custa **1,80 µs por linha** — 1.803 ms num milhão (`DESEMPENHO.md` §20).
> Um índice que apenas evitasse o `.memo` compraria **1,75×**. Os ~900× só
> vêm de um índice que **responde sem tocar na linha**. Quem defender este
> arquivo por «evitar o `.memo`» está defendendo o número errado.
>
> **E o que ele custa também está medido, e mudou o desenho:** escrever o
> índice a cada inserção custaria **9,05×** o `inserir` de hoje (§4.1). Por
> isso ele entra por **despejo em lote**, e não síncrono.

Este documento foi escrito **antes** do código, e é ele que o código obedece.
Onde os dois discordarem, um dos dois está errado — e a regra da casa é que o
número medido ganha do texto.

**Ele não implementa nada.** Existe para que a decisão seja tomada com o custo
na mesa. Onde não houve como medir, está escrito **«não medido»** com o que
decidiria o número — nomeado vale, estimado não.

Escolhido pelo dono em 06/09/2026 como a primeira das onze lacunas do
`HFSQL.md`, sob o critério **«vencer no que se prova»**.

---

## 0. O que ele é, em uma tela

Um segundo arquivo de índice por tabela, `Tabela.fts`, com **a mesma árvore
B+ do `.ndx`** e uma chave composta:

```
chave = (termo dobrado, rowid)
```

Procurar `fenix` é descer a árvore até o primeiro `("fenix", *)` e varrer para
a frente enquanto o termo não mudar. Cada chave encontrada **já é o rowid** —
e no PhxSql o rowid é o endereço por conta (`offset = data_offset + (rowid−1)
× slot_size`), então a linha sai do disco em O(1), e só as que casam saem.

| peça | o que é |
|---|---|
| onde mora | `Tabela.fts`, ao lado do `.ndx`, mesma paginação de volume |
| a estrutura | a árvore B+ que já existe, com CRC-32 por página no despejo |
| a chave | `(termo dobrado, rowid)`, e não um formato de *posting list* próprio |
| a dobra | a `sem_acento` do `paginacao.rs`, **reusada e não copiada** |
| o que promete | achar a palavra inteira; **não** promete prefixo, nem radical |

---

## 1. O que ele compra, com a prova do que falha hoje

Medido em `custo-da-busca-de-palavra`, e as duas metades importam:

| o que falha hoje | número |
|---|---|
| achar uma palavra num `Memo` | **1,80 µs/linha** → 1.803 ms num milhão |
| a fatia que é ler o `.reg` | **40,1%** |
| a fatia que é ler o `.memo` | **43,0%** |
| a fatia que é comparar texto | **16,9%** |
| achar `fenix` onde está escrito `fênix` | **0 de 200** |

O índice remove as três fatias de uma vez, porque não lê linha nenhuma para
decidir. É por isso que ele mira 900× e não 1,75×.

---

## 2. As divergências, e a restrição NOSSA que causou cada uma

A lei da casa manda a pergunta: *onde esta lógica diverge da de origem, e qual
restrição nossa causou a divergência?* Um índice invertido é receita de
domínio público desde os anos 60; o que segue é o que **muda** aqui, e por quê.

### 2.1 Sem tabela de mapeamento — a maior, e sai da ordem de digitação

Todo índice invertido clássico guarda um *doc id* interno e mantém uma
**tabela de mapeamento** doc id → endereço do documento. Ela existe porque o
armazenamento por baixo compacta, move e **reaproveita espaço**, então o
endereço de um documento muda.

Aqui não muda, **e isso é pétrea**: o `.reg` nunca reaproveita slot excluído,
e o endereço sai de uma conta sobre o rowid. Então a lista de ocorrências
guarda **o rowid direto**, e a tabela de mapeamento **não existe**. Uma peça
inteira a menos para manter, sincronizar e reconstruir.

### 2.2 Sem geração nem versão na ocorrência

Pelo mesmo motivo. Onde o slot se reaproveita, uma ocorrência antiga pode
apontar para uma linha **diferente** que nasceu no mesmo lugar, e por isso os
outros carregam um contador de geração junto do doc id. Aqui um rowid é
daquela linha para sempre; ocorrência velha aponta para linha excluída, que é
um caso que o `.ndx` já sabe tratar (o slot responde «inativo»).

### 2.3 Sem formato próprio de *posting list*

A receita padrão guarda, por termo, uma lista de doc ids ordenada, comprimida
por delta e varint. Aqui a chave `(termo, rowid)` entra na **árvore B+ que já
existe**, e a lista de um termo é a faixa contígua de chaves com aquele
prefixo.

**A restrição que causou:** zero dependências e uma casa que já pagou por
duplicar mecanismo. Um formato próprio compraria **tamanho**, e tamanho não é
o gargalo medido — o gargalo é tocar na linha. Reusar a árvore traz de graça o
CRC-32 por página, o cache de páginas de leitura que comprou 2,40×, o
`reindexar` do arranque e a bancada que já existe para tudo isso.

O que se perde está nomeado na §4.2: o arquivo fica **maior** que uma lista
comprimida. É recusa consciente, não descuido.

### 2.4 A dobra é REUSADA, não reescrita

A `sem_acento` do `crates/phxsql-core/src/paginacao.rs` já existe, escrita à
mão para a partição alfanumérica, e cobre português, espanhol e alemão.

**O `.fts` usa aquela função**, promovida a `pub`. Duas cópias divergiriam, e
a divergência apareceria do pior jeito possível: o balde do `.pag` e o termo do
`.fts` discordando sobre «Álvaro», na mesma tabela, sem conflito nenhum
aparecer. É a mesma lição do rodapé que publicou 780 KiB — **quando duas
coisas dependem de uma lista, a lista tem de ter um dono só.**

### 2.5 Sem radical (*stemming*), e é decisão

Os motores de busca reduzem «pedidos» e «pedido» ao mesmo radical. Isso pede
tabela morfológica por idioma — uma dependência com outro nome. Aqui o índice
acha **a palavra inteira, dobrada**, e o que ele não promete não aparece como
promessa. Quem precisar de radical usa o `contem` de hoje, que continua.

---

## 3. O formato em disco

**Muda formato, e por isso entra cedo** — enquanto não há dado em produção é
barato; depois vira migração.

- **Arquivo novo `Tabela.fts`**, com a mesma magia/versão/CRC do `.ndx`.
  Tabela sem índice de texto declarado **não ganha o arquivo** — não se paga
  por um recurso que não se pediu.
- **A declaração entra no `PSCH`**, como mais um tipo de índice: a coluna, e o
  interruptor `dobrar` (padrão **ligado**; ver §5.1).
- **A recusa acontece na DECLARAÇÃO, não na gravação.** Índice de texto sobre
  coluna que não é texto é erro de modelagem, e uma tabela nasce uma vez e
  grava um milhão de vezes. É a mesma decisão do `ao_excluir`.
- **É derivado.** O `.fts` se reconstrói do `.reg` inteiro, como o `.ndx` já
  faz no `reindexar` — perdê-lo custa tempo, nunca dado.

---

## 4. O que ele custa

### 4.1 O que está MEDIDO — e **matou o desenho síncrono**

O medidor da §4.2(a) rodou antes do índice, como este documento exigia, e o
resultado mudou o desenho:

`cargo run --release --example custo-da-chave-a-mais -- 50000`

| índices na tabela | µs por inserção | µs por chave a mais |
|---:|---:|---:|
| 1 (só `porId`) | 5,458 | — |
| 2 | 6,298 | 0,840 |
| 4 | 8,508 | 1,105 |
| 8 | 11,401 | 0,723 |
| **15** (= 14 do texto + `porId`) | **49,366** | **5,424** |
| 17 | 61,556 | 6,095 |

**Escrever o `.fts` a cada `inserir` custaria 9,05× a inserção de hoje**, e não
os 2,95× que uma reta previa. O custo por chave é plano até 8 índices e depois
**despenca num penhasco**: 0,72 µs vira 5,42.

*A causa está NÃO MEDIDA, e é hipótese:* a suspeita é que as páginas quentes de
15 árvores deixam de caber no cache, e cada chave passa a pagar leitura de
página de verdade. O que decidiria: os toques de página por linha, que o
medidor do `.ndx` já sabe contar. **Efeito medido, causa nomeada** — é a
diferença que esta casa cobra.

**Consequência para o desenho:** a escrita síncrona por inserção **não passa**.
O `.fts` entra por **despejo em lote** — o índice fica atrás por N inserções, e
a busca diz até onde enxerga. Isso é o §4.3, e não um ajuste.

### 4.2 O que fica NOMEADO e NÃO MEDIDO

**(b) O tamanho do arquivo.** ~14 chaves por linha, cada uma «termo + rowid».
Num milhão de linhas são ~14 milhões de chaves. *O que decidiria:* gerar e
medir com o `conferir-integridade`, que já sabe pesar arquivo.

**(c) A exclusão.** Apagar uma linha precisa tirar ~14 chaves do `.fts`, e
`excluir` já é o caminho que esta casa escolheu deixar caro. Provavelmente
cabe; **não medido**.

### 4.3 O despejo em lote, e a honestidade que ele obriga

Como a §4.1 mediu, o índice não pode andar junto de cada inserção. Ele anda
atrás, e **isso muda o que ele promete**:

- **A busca diz até onde enxerga.** A resposta traz o rowid mais alto que o
  índice já indexou. Quem precisa de «tudo, inclusive o que entrou agora» faz
  a busca e completa com o `contem` de hoje a partir dali — e é o cliente que
  decide, com o número na mão, em vez de o motor mentir por omissão.
- **Índice atrás não pode achar a MAIS.** Achar a menos é atraso, e ele é
  declarado; achar a mais é defeito. A prova da §7.1 confere o conjunto.
- **O que dispara o despejo** — número de linhas, tempo, ou o fecho de janela
  que já existe — está **em aberto**, e é a primeira decisão da implementação.

---

## 5. O que ele NÃO resolve

- **Prefixo e curinga.** `fen*` não sai de graça da chave `(termo, rowid)`.
- **Radical.** §2.5.
- **`OU` entre termos.** Duas descidas e a união das faixas — cabe, mas não
  está neste desenho.
- **Ordenar por relevância.** Não há contagem de ocorrências por linha na
  chave. Quem quiser relevância paga outra coluna na chave, e isso muda o
  formato — então é decisão de agora, não de depois.
- **A trava global.** Continua inteira; o `.fts` não é sobre concorrência.

### 5.1 O interruptor `dobrar`, e por que ele nasce LIGADO

Medido: a busca de hoje **não** dobra acento. Um índice sem dobra acharia
**menos que a varredura de hoje** em qualquer palavra acentuada, e índice que
acha menos que a varredura é pior que não ter índice.

Então aqui a decisão é o inverso da «guarda nova entra pedida»: **nasce
dobrando**, e quem quiser o contrário manda `"dobrar": false` — escolha
escrita, em vez de omissão. É a mesma forma da chave que nasce conferida.

E isto **não quebra tabela que já existe**, porque nenhuma tem `.fts`: o
recurso nasce hoje, e o que nasce hoje pode nascer certo.

---

## 6. As alternativas mais baratas, com o custo de cada uma

### (a) Não fazer nada — custo zero

O `contem` de hoje continua, e a lacuna vai para o `HFSQL.md` como recusa
medida: «1.803 ms por milhão, e é o que temos». Honesto, e perde a lacuna que
o dono escolheu como a primeira.

### (b) Só o mapa de igualdade em memória — RECUSADA, com o motivo

O `memoria.rs` já tem mapa de igualdade que evita varredura. Ele acha o
**valor inteiro**, não a palavra dentro do texto: procurar `fenix` num campo
que vale «pedido cliente fenix nota» não casa. Resolve outra pergunta.

### (c) Índice de texto só no `.memo`, deixando o `Str` de fora

Tentador porque o `.memo` «parece» o problema. **Medido, é 43%** — e o
recorte deixaria o `Str(200)` sem índice, que é onde mora a maioria dos campos
que a tela procura. Recusada pelo número.

---

## 7. Como se prova

A prova real é nos dois sentidos, e para um índice ela tem uma forma própria:

1. **O índice acha exatamente o que a varredura acha.** Mesma tabela, mesma
   palavra, os dois caminhos, e os conjuntos de rowids **idênticos** — não só
   as contagens. Índice que acha a mais é pior que índice que acha a menos.
2. **Com a dobra reposta como estava, a prova FALHA:** procurar `fenix` tem de
   achar as linhas de `fênix`, e sem a dobra ela acha zero.
3. **Depois de excluir**, o índice deixa de achar a linha excluída — e o teste
   confere o conjunto, não a contagem.
4. **Depois de `reindexar`**, o `.fts` reconstruído acha o mesmo que o
   incremental. Divergência aqui é o defeito clássico de índice.
5. **O medidor da §4.2(a) roda antes** e o número entra no `DESEMPENHO.md`,
   frutífero ou não.

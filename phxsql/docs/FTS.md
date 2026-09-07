# O `.fts`: achar uma palavra sem ler a linha

> **O que ele compra está medido, e o que ele NÃO compra também.** A busca de
> hoje custa **1,80 µs por linha** — 1.803 ms num milhão (`DESEMPENHO.md` §20).
> Um índice que apenas evitasse o `.memo` compraria **1,75×**. Os ~900× só
> vêm de um índice que **responde sem tocar na linha**. Quem defender este
> arquivo por «evitar o `.memo`» está defendendo o número errado.
>
> **E o que ele custa está medido duas vezes, porque a primeira mediu a forma
> errada:** o `.fts` custa **6,70×** o `inserir` de hoje, ~2,0 µs por chave — e
> o **despejo em lote não conserta isso**, compra 6–10% e não melhora com lotes
> maiores (§4.1). Não há conserto barato no caminho de escrita: a decisão é do
> dono, e está na §4.4.

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

### 4.1 O que está MEDIDO — e a primeira medição mediu a FORMA ERRADA

Duas medições, e a segunda desmente a primeira.

**A primeira** (`custo-da-chave-a-mais`) usou **15 índices separados** e achou
9,05×. Dela saiu a prescrição «entra por despejo em lote».

**Ela mediu outra coisa.** Quinze índices são quinze árvores B+, cada uma com
seu conjunto de páginas quentes. O `.fts` é **UMA árvore** recebendo ~14 chaves
por linha. *Bancada compara trabalho igual, e não só pergunta igual* — eu
comparei pergunta igual (14 chaves a mais) com trabalho diferente (14 árvores a
mais). E a causa que eu mesmo nomeei para o penhasco — «as páginas de 15
árvores deixam de caber no cache» — dizia justamente que a forma é o que
decide.

**A segunda** (`custo-do-fts-de-verdade`) usa o `FtsFile` real:

| medida | µs/linha | × sobre A |
|---|---:|---:|
| A — só a tabela (1 índice) | 8,989 | 1,00 |
| B — + `.fts` linha a linha | 60,192 | **6,70** |
| C — + `.fts` em lote de 200 | 56,603 | 6,30 |

E a segunda pergunta, que **também estava por medir**:

| tamanho do lote | C/B |
|---:|---:|
| 200 | 0,94× |
| 1.000 | 0,90× |
| 10.000 | 0,92× |

**O lote compra 6–10%, e não melhora com lotes maiores** — que é a assinatura
de «o lote não é o mecanismo». O custo é ~**2,0 µs por chave**, e ele é do
trabalho de pôr a chave na árvore, não de quando se põe.

**Consequência: a §4.3 desta página prescrevia uma cura para uma causa que ela
mesma marcava como NÃO MEDIDA, e a cura não cura.** Não há conserto barato no
caminho de escrita. A escolha vai para a §4.4, e é do dono.

### 4.2 O que fica NOMEADO e NÃO MEDIDO

**(b) O tamanho do arquivo.** ~14 chaves por linha, cada uma «termo + rowid».
Num milhão de linhas são ~14 milhões de chaves. *O que decidiria:* gerar e
medir com o `conferir-integridade`, que já sabe pesar arquivo.

**(c) A exclusão.** Apagar uma linha precisa tirar ~14 chaves do `.fts`, e
`excluir` já é o caminho que esta casa escolheu deixar caro. Provavelmente
cabe; **não medido**.

### 4.3 O índice atrasado, e a honestidade que ele obriga

Vale para qualquer saída da §4.4 em que o índice não ande junto da gravação:

- **A busca diz até onde enxerga.** A resposta traz o rowid mais alto que o
  índice já indexou. Quem precisa de «tudo, inclusive o que entrou agora» faz a
  busca e completa com o `contem` de hoje a partir dali — e é o cliente que
  decide, com o número na mão, em vez de o motor mentir por omissão.
- **Índice atrasado não pode achar a MAIS.** Achar a menos é atraso, e ele é
  declarado; achar a mais é defeito.

### 4.4 As três saídas, com o número de cada uma — **decisão do dono**

| saída | custo no `inserir` | o que o índice promete |
|---|---|---|
| **(a) manter na gravação** | **6,70×** nas tabelas que declaram | sempre atual; nada a explicar a ninguém |
| **(b) construir por `reindexar`** | **1,00×** — zero a mais | tão velho quanto a última reconstrução, e a busca diz quanto |
| **(c) indexar menos termos** | proporcional: são **2,0 µs por chave** | atual, mas acha menos — o que não entrar no índice não se acha |

A (c) tem uma conta direta: cortar os termos pela metade leva 6,70× para
~3,8×. O que se cortaria — números, palavras de uma ou duas letras, uma lista
de palavras vazias — **muda o que a busca acha**, e por isso é decisão e não
ajuste.

**O que NÃO é saída, e está recusado com número:** o despejo em lote (§4.1).

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

## 8. A queda, e o irmão que era um ARQUIVO

O `.fts` **é um `.ndx` por dentro**: `FtsFile` embrulha `NdxFile` para não
reescrever a árvore B+ nem a paginação. Ganhou tudo — e ganhou junto a **marca
de «ficou para trás numa queda»**, que o `.ndx` levanta no cabeçalho *antes* de
a primeira página suja ir ao disco.

Ganhou a marca. Não ganhava o conserto:

| pergunta | quem respondia antes | quem responde agora |
|---|---|---|
| «o índice ficou para trás?» | só o `.ndx` | os **dois** arquivos |
| «reconstrua» (`reindexar`) | só o `.ndx` | os **dois** arquivos |
| `.fts` ilegível na abertura | derrubava a tabela | **refaz**, varrendo |

E enquanto a marca estiver de pé, **toda** operação de índice recusa
(`ndx.rs:887`). Ou seja: uma queda deixava a tabela **sem gravar nunca mais**,
com o `reindexar` respondendo `Ok` — e a mensagem do próprio `.fts` mandando
*«reconstrua o índice de texto com `reindexar`»*, uma ordem que o código não
sabia cumprir.

**Refazer nunca custa dado**, e é isso que autoriza as três respostas acima: o
`.fts` é **derivado** do `.reg`. Recusar a tabela por causa de um arquivo
derivado ilegível seria pagar com o dado íntegro o preço de um índice quebrado.
O mesmo raciocínio já estava escrito para o caso de o arquivo **faltar** — o
que faltava era estendê-lo ao arquivo **ilegível**, que inclui o caso de quem
declara um índice de texto numa tabela que já tem dados.

**A recriação é o que tira a marca, e isso é de propósito**: `NdxFile::fechar`
recusa limpar a marca de um arquivo aberto já sujo, senão bastaria alguém abrir
e fechar para o defeito virar invisível. Por isso `reconstruir_fts` **recria**
o arquivo antes de varrer, em vez de escrever por cima — e de quebra vira
idempotente, que ele não era.

As três guardas estão no catálogo (`fts-reindexar-sem-o-irmao`,
`fts-reconstruir-sem-recriar`, `fts-abrir-recusa-a-tabela`), e o processo — com
os dois diagnósticos plausíveis que a medição matou — está em
`docs/cognicao/cognicao_o-irmao-pode-ser-um-arquivo_20260907_0130.md`.

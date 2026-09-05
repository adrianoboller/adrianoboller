# Arquivo derivado quer atomicidade, não durabilidade — e a diferença tem número

**Descoberto em 05/09/2026, 04:05**, medindo o `.pag` antes de decidir se ele
precisava de `fsync`.

## 1. O que aconteceu

O `.pag` é o descritor de partição da tabela, em JSON indentado. O cabeçalho do
`crates/phxsql-store/src/pag.rs` diz o que ele é, e diz certo: **derivado**, o
motor nunca o lê, apagá-lo não quebra a tabela. Ele é escrito por
`std::fs::write` — sem `fsync` **e** sem troca atômica —, e `Table::gravar_pag`
é chamado em **todo** `Table::sincronizar()`.

O pedido 14 do `PENDENCIAS.md` já nomeava metade disto («escrito a cada
`sincronizar` e nunca vai ao disco») e o classificava como não urgente, com o
argumento certo: arquivo derivado não precisa de durabilidade.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o assunto era **queda de energia** — «se o processo cair no meio do
`fs::write`, o `.pag` fica pela metade» — e que, sendo raro, o pedido 14 estava
certo em deixar para depois.

Errado no alvo. O `fs::write` abre com `O_TRUNC`: o arquivo fica **zerado e
visível assim** enquanto a escrita não termina. Isso não precisa de queda
nenhuma — acontece em **toda** regravação, e a regravação acontece a cada
`sincronizar()`. Quem sofre não é o motor no dia da queda: é o leitor de fora,
que é a **única plateia** que este arquivo tem, em qualquer dia.

Ou seja: eu estava medindo a probabilidade de uma queda quando o que importava
era a **largura da janela** e a frequência com que ela abre.

## 3. O que a medição disse

| medida | número |
|---|---|
| janela em que o `.pag` está truncado, por regravação | **33,2 µs** (36% dos 93 µs da gravação) |
| leitor de fora insistindo durante a regravação, com `fs::write` | **82,4% das leituras** pela metade (482.246 de 585.169) |
| o mesmo leitor, com temporário + `rename` | **0 de 606.086** |
| custo de gravar, `fs::write` — mediana de 6 corridas × 20.000 | **92,6 µs** |
| custo de gravar, temporário + `rename` | **76,0 µs** |
| queda no meio da escrita | arquivo com 100 bytes de 3.427, que não fecha em `}` |

A janela não depende do tamanho: 467 B (partição por quantidade) e 3.400 B
(alfanumérica com 37 baldes) deram os mesmos 33 µs — os dois cabem numa página.

**Quem depende do `.pag`, contado:** ninguém por dentro. O `pag.rs` **não tem
função de leitura nenhuma**; as únicas menções fora dele são movimentos de
arquivo — a lista de dez extensões do `catalogo.rs` (excluir e renomear) e o
`read_dir` do backup, que copia os bytes que encontrar. A plateia declarada no
cabeçalho — camada SQL, ETL, relatório, `ls` — é a plateia inteira.

## 4. A regra

**Para um arquivo derivado, atomicidade vale e durabilidade não: um perdido se
regrava, um pela metade mente.** E o corolário de método: *a largura da janela
vezes a frequência com que ela abre é o número que decide — não a
probabilidade de uma queda.*

## 5. Como está guardado hoje

* `pag::escrever` grava num `<nome>.pag.novo` e troca por `rename`, que é
  atômico no mesmo sistema de arquivos. **Não há `fsync` novo**, e a catraca
  `TETO_FSYNC_POR_FECHO_V2` continua valendo **8** — sem `fsync` de diretório,
  uma queda pode desfazer a troca e deixar o `.pag` **anterior**, que é velho e
  válido: é a degradação certa;
* a guarda é `crates/phxsql-store/tests/pag-se-troca-inteiro.rs`, que põe um
  leitor de fora insistindo enquanto o motor regrava 300 vezes e conta quantas
  leituras pegaram JSON que não fecha. Mede o **efeito**, e não «`escrever`
  chamou `rename`», que seria a intenção. **Prova real:** com o `fs::write` de
  volta, ela falha com **1.856 leituras partidas de 118.526**;
* **o resto que fica, dito em vez de escondido:** uma queda dentro da janela de
  33 µs deixa `<nome>.pag.novo` para trás. O próximo `sincronizar` da tabela o
  renomeia por cima, então ele não se acumula; o que ele não faz é sair no
  `excluir_tabela`, e isso é decisão — pôr o temporário nas dez extensões o
  faria aparecer no `arquivos_da_tabela`, que é a lista que a **tela** mostra;
* **o que ficou de fora, com o número:** o `fsync` do `.pag`. Recusado — ele
  compraria durabilidade para um arquivo que se regenera sozinho, ao preço de
  um nono `fsync` por fecho de janela (**+52 µs medidos** para arquivo que
  ninguém sujou) e da aposentadoria da catraca V2. A recusa do
  `PESQUISA-FSYNC-SELETIVO.md` §5 continua valendo, e agora com o motivo certo
  ao lado: o `.pag` não queria `fsync` — queria `rename`.

# Recorte antes do casamento cega o instrumento — e o «achado» era meu

**Descoberta:** 16/09/2026, 07:22 UTC, na passada rápida da bancada
`bancada/tomada/chutar-a-tomada.py` (papel F, «chutar a tomada»).

## 1. O que aconteceu

A bancada mata o `phxsqld` com `SIGKILL` no meio de um `reindexar` e, depois
de reabrir, pergunta ao índice se ele está sujo: `buscar` pela chave 1 e o
casador `indice_sujo(texto)`, que procurava «reparar indice» no texto do erro.
A passada rápida devolveu **4 de 6 quedas** classificadas como
`*** VAZIO_OU_PARCIAL_EM_SILENCIO ***` — índice limpo, 0 de 2.001 chaves, só
`verificar` acusando. Era exatamente a hipótese que eu tinha escrito antes de
rodar («queda entre o `criar()` e a primeira página deixa índice vazio e
LIMPO»), e ela parecia confirmada com número.

Não estava. O `buscar()` da bancada devolvia o erro **recortado em 160
caracteres**, e a mensagem do motor —
`[SP000010] arquivo corrompido: o indice de /home/…/t.ndx ficou para tras
numa queda e nao e confiavel: reconstrua com \`reparar indice\`` — cortava em
«nao e co». O casador não achava «reparar indice» porque a frase não tinha
chegado. O índice **estava** recusando, corretamente, desde o primeiro
pedido.

## 2. O que eu concluí primeiro, e estava errado

Que o motor tinha a janela que eu previa: `NdxFile::criar` grava um
cabeçalho limpo com árvore vazia, `Table::reindexar` varre o `.reg` inteiro
antes de `construir_em_lote`, logo uma queda durante a varredura deixaria
`.ndx` limpo e vazio, `buscar` calado e `inserir` aceitando chave repetida.
Li o código, achei a janela, escrevi a hipótese — e quando o número «bateu»,
não desconfiei justamente porque batia com o que eu esperava. A frase do
`verificar` no mesmo log já dizia «ficou para tras numa queda», que é a
mensagem de índice **sujo**; eu li e não vi.

## 3. O que a medição disse

Uma sonda direta (`scratchpad/sonda_reindex.py`): matar a 2, 4 e 7 ms do
`reindexar` e ler o **byte 52** do `.ndx` antes de qualquer pedido.

| atraso | byte 52 após a queda | tamanho do `.ndx` | `buscar(1)` | `verificar` | `inserir` chave repetida |
|---|---|---|---|---|---|
| 2 ms | **1** | 8.192 B | recusa (SP000010) | recusa | recusa |
| 4 ms | **1** | 8.192 B | recusa | recusa | recusa |
| 7 ms | **1** | 225.280 B | recusa | recusa | recusa |

Depois, na corrida completa: **111 quedas no meio do `reindexar`, 111 com
byte 52 = 1**, 0 silenciosas. A hipótese morreu medida: o `criar` grava a
página-raiz de cada índice por `gravar_pagina`, que levanta a marca de sujo
**antes** de a árvore existir. Não há janela limpa — e o motor estava certo
onde eu tinha previsto defeito.

## 4. A regra

**O texto que o casador lê é o texto inteiro; recorta-se só ao guardar.**
Instrumento que recorta antes de julgar responde «não vi» para o que não
coube — e «não vi» tem a mesma cara de «não aconteceu». E quando o número
bate com a hipótese, é hora de ler o campo vizinho, não de comemorar.

## 5. Como está guardado hoje

* `bancada/tomada/chutar-a-tomada.py`: `buscar()` devolve o erro inteiro (o
  comentário conta o caso); `indice_sujo()` casa também «ficou para tras numa
  queda»; `corrida()` lê o byte 52 do `.ndx` **antes de reabrir** e o grava
  em toda corrida (`ndx_byte52_apos_queda`), e o veredito da hipótese sai
  desse byte, não do texto de erro.
* `bancada/tomada/resultados.json`: `pontos.reindexar.hipotese_indice_vazio_e_limpo`
  com o veredito e a contagem de byte 52.
* O buraco que ficou: a bancada de durabilidade e a do ACID também casam
  textos de erro recortados (`[:160]`, `[:200]`) em alguns pontos — não medi
  se algum casador delas depende do fim da frase. Fica como pergunta para a
  próxima rodada do papel F, não como afirmação.

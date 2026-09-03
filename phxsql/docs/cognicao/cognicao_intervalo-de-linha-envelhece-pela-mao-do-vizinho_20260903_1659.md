# Intervalo de linha é receita que envelhece pela mão do VIZINHO — em minutos, e sem eu tocar em nada

**Descoberto:** 03/09/2026, 16:59.
**Onde:** `docs/fronteiras/mapa-do-servidor.py`, constante `REGIOES`;
alvo `crates/phxsql-server/src/servidor.rs`.

Não é cognição nova a pétrea «a receita de um número também envelhece» — ela já
existe. O que é novo é o **alcance** dela: até hoje a receita envelhecia porque
*nós* mudávamos o código medido (a lista de arquivos do `http.rs`). Numa árvore
com nove frentes ela envelhece **sem ninguém desta frente tocar em nada**.

## 1. O que aconteceu

O gerador do mapa divide o `impl Servidor` em quinze regiões contíguas, e eu as
escrevi como intervalos de linha: `("cluster", 1969, 2579)`. Junto delas escrevi
uma conferência de cobertura — as regiões têm de cobrir os 275 métodos, e o que
cair num vão sai listado como `FORA`. Ela deu 275 de 275, e commitei mentalmente
o assunto.

Meia hora depois, ao rodar o gerador pela segunda vez, a conferência acusou
**274 de 275 — FORA: `resposta_erro`**.

## 2. O que eu concluí primeiro, e estava errado

Que eu tinha errado um dos quinze intervalos ao digitá-los, e fui reconferir os
limites um por um.

Errado. Os intervalos estavam certos quando foram escritos. O que mudou foi o
arquivo: uma frente vizinha acrescentou linhas ao `servidor.rs` enquanto este
documento era escrito, tudo abaixo do ponto de inserção desceu, e o último
método escorregou para fora do último intervalo.

O sinal que eu quase li errado: o `sha256` do arquivo mudou entre duas corridas
do gerador separadas por **um segundo**, com a contagem de linhas **idêntica**.
Isso não é ruído — é o vizinho reescrevendo as mesmas linhas.

## 3. O que a medição disse

| | quando escrevi os intervalos | trinta minutos depois |
|---|---|---|
| `servidor.rs` | 23.163 linhas | **23.171** |
| `impl Servidor` termina em | 14.933 | **14.941** |
| cobertura das regiões | 275 de 275 | **274 de 275** |
| edições minhas no `servidor.rs` | **nenhuma** | **nenhuma** |

Oito linhas de outra pessoa, e a receita de quinze números virou pó. O
`servidor.rs` também já havia crescido **611** linhas desde as 22.560 que o
roteiro da SP000005 registra — o mesmo efeito, só que devagar.

O conserto: ancorar cada região na **primeira e na última função**, não em
linha. Nome de função sobrevive a uma inserção acima dele; número de linha, não.
Depois disso a cobertura voltou a 275 de 275 e ficou.

## 3-bis. A frente vizinha achou a outra metade, onze minutos antes

`cognicao_arvore-de-fonte-pega-no-meio-de-uma-edicao_20260903_1648.md` mede o
**mesmo ambiente** e um mecanismo **diferente**: lá a árvore compartilhada
mordeu porque o executor de guardas copiou o fonte no meio de uma edição alheia
— `mod.rs` declarando um módulo cujo arquivo ainda não existia — e a árvore
limpa reprovou sozinha.

São dois sintomas do mesmo ambiente, e vale ler os dois juntos porque a defesa é
diferente em cada um: lá a resposta é **esperar** (olhar o `mtime` da origem
antes de culpar a cópia); aqui é **não depender da posição** (ancorar em nome).
Nenhuma das duas defesas serve para o problema do outro.

## 4. A regra

**Numa árvore compartilhada, nunca ancore uma medida em número de linha —
ancore em nome.** E toda medida que fatia um arquivo carrega uma conferência de
cobertura, porque é ela que grita quando a âncora escorrega.

## 5. Como está guardado hoje

Três guardas no `docs/fronteiras/mapa-do-servidor.py`, e as três já dispararam
de verdade:

1. **`REGIOES` ancora em nomes de função**, e a resolução acontece contra o
   arquivo a cada corrida.
2. **Se um nome de âncora sumir**, o gerador para com o recado dizendo qual
   região e qual função — provado renomeando `subir_cluster` numa cópia.
3. **A conferência de cobertura** sai impressa no próprio documento
   («As 15 regiões cobrem 275 de 275 métodos»), então uma âncora que escorregue
   aparece na página em vez de ficar no terminal de quem rodou.

E o **carimbo** no topo do `FRONTEIRAS-DO-SERVIDOR.md`: data, contagem de
linhas, `sha256` e revisão do `git`, com a instrução de que se o `wc -l` de hoje
não bater, a página envelheceu. É o que transforma «este documento pode estar
velho» em uma pergunta de um comando.

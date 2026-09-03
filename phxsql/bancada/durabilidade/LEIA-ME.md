# A matriz real de durabilidade — SP000010

O desenho do protocolo de commit está inteiro em `docs/TRANSACOES.md` §5. O
que faltava era a **matriz**: cruzar os pontos de morte de uma escrita
transacional com os três regimes de `recursos.durabilidade`, provando cada
célula por `SIGKILL` de verdade — nunca por teste unitário.

## Como rodar

```bash
cargo build --release
python3 bancada/durabilidade/prova.py
```

Sobe um `phxsqld` de verdade na porta **7530**, mata-o com `SIGKILL` dezenas
de vezes (nunca `pkill` — só os PIDs que o próprio script criou) e escreve
`bancada/durabilidade/resultado.json` com **tudo que foi medido**. A matriz
que entra em `docs/TRANSACOES.md` §5.7 é **transcrita** desse arquivo — se um
número aparecer no documento, ele saiu daqui.

Leva de dois a quatro minutos: cada célula da matriz é uma varredura de
atrasos (não uma tentativa só), porque matar o processo no instante certo é
uma corrida — a mesma lição do §5.6 do documento.

## O método, em três peças

1. **`corrida()`** sobe o servidor, faz `BEGIN` + as operações, dispara o
   `COMMIT` numa thread e mata o processo com `SIGKILL` num atraso
   controlado *depois* de a marca `.tx` aparecer no disco. `calibrar()` mede
   antes um `COMMIT` limpo do mesmo tamanho, e é essa medida — não um chute —
   que decide a faixa de atrasos da varredura.

2. **`classificar()`** lê SÓ o relatório do arranque (`PHXSQL Recovery`) e os
   contadores que ele expõe (`achadas`, `descartadas`, `completadas`,
   `reaplicadas`, `ja_aplicadas`, `impossiveis`) para dizer em qual dos cinco
   pontos de morte a queda caiu — sem espiar o disco por fora do protocolo.
   `ja_aplicadas == N` com `reaplicadas == 0` é o ponto 4 (a passada inteira
   já tinha chegado ao disco quando a marca ainda estava lá); `reaplicadas ==
   N` com `ja_aplicadas == 0` é o ponto 2; um valor no meio é o ponto 3.

3. **A cascata (ponto 5)** verifica, depois de CADA queda, se toda filha
   aponta para o valor novo da mãe, ou toda para o velho — nunca uma mistura
   silenciosa. Quando aparece uma mistura, o critério não é "isso é um
   defeito": é "o relatório do arranque **denunciou** isso em
   `operacoes IMPOSSIVEIS`?". Perder em silêncio é o único desfecho que
   reprova a prova.

## Uma armadilha que este script já pagou

A primeira rodada acusava cascata "parcial sem aviso" toda vez que a mãe
tinha mais de 1.000 filhas — porque o `buscar` por índice **pagina**, e o
`max_linhas` padrão do servidor (1.000) cortava a resposta em silêncio. Não
era o motor: era o próprio verificador comparando "1.000" com "1.200" e
chamando isso de defeito. O `config()` deste script sobe `max_linhas` para
10.000 por causa disso — a mesma lição do CLAUDE.md sobre número que não se
mede: aqui era um número que se media **errado**, por um teto que ninguém
tinha olhado.

## O que NÃO está aqui

A queda de **energia** (não de processo). Nenhum processo em espaço de
usuário provoca isso, e a linha já está escrita em `docs/DESEMPENHO.md`
§4.12 para a exclusão — a mesma lei vale aqui: o `write` já foi entregue ao
sistema operacional em toda gravação, então uma queda de PROCESSO nunca perde
o que já tinha sido escrito. O que a queda de energia arriscaria (páginas do
`.ndx`/`.reg` que só existiam no cache do kernel) está descrito, não medido,
em `docs/TRANSACOES.md` §5.7.

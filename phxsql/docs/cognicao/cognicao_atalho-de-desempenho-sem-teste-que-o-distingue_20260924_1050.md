# Um atalho de desempenho pode não ter NENHUM teste capaz de provar a falta dele

**Estado:** PENDENTE

## O que aconteceu

No pedido 481, escrevi `resolucao_do_par` com um atalho cedo: se os dois
arquivos do par (`config.json`/`config.phz`) têm o MESMO dono, devolve
`Ambiguo` na hora, sem chamar `dono_do_processo` (que cria e apaga um arquivo
descartável para descobrir o UID de quem roda o servidor). Escrevi também uma
guarda no catálogo (`bancada/guardas/catalogo.py`) para essa linha, com o
`trecho`/`troca` de praxe, e o `caem` apontando para o teste do comportamento
velho (`os_dois_presentes_mesmo_dono_nao_sobem_e_a_troca_nao_toca_em_nenhum`).

## O que eu concluí primeiro, e estava errado

Escrevi a guarda **antes** de provar o vermelho à mão — assumi que, como a
linha existe e o comentário explica por que ela existe, o teste do
comportamento velho cairia se ela sumisse. Rodei o defeito reposto
manualmente (`if dc == dp { … }` → `if false { … }`, exatamente o `troca` que
eu tinha acabado de escrever) e o teste **passou** — nenhum vermelho.

## O que a medição disse

O atalho é matematicamente redundante: as duas condições abaixo dele
(`dc == processo && dp != processo` e o espelho) só disparam quando
`dc != processo` OU `dp != processo`, e como o processo tem um UID SÓ, isso é
impossível quando `dc == dp`. Tirar o atalho não muda NENHUM resultado
observável em NENHUM cenário — só faz o código pagar a sonda (criar+apagar um
arquivo) numa situação em que o resultado já estava decidido. Rodar o teste
com o defeito reposto confirmou: `ok`, não `FAILED` — a prova exigida pela
regra «teste que passa por engano é pior que teste que falta» reprovou a
própria guarda antes dela entrar no catálogo.

## A regra

**Guarda de catálogo só entra depois de rodar o defeito reposto à mão, uma
vez, e VER o vermelho — nunca por confiança em que «a linha existe, logo o
teste cai».** Um atalho de desempenho puro (que não muda resultado, só custo)
é o caso mais fácil de escrever uma guarda vazia, porque o código parece
tão intencional quanto qualquer outra recusa.

## Como está guardado hoje

A guarda vazia (`config-phz-par-nao-ignora-mesmo-dono`) foi **removida** do
catálogo antes de qualquer commit — nunca chegou a ficar publicada. Mas o
teste que a citava continuou citando-a, e a revisão SEC do 481 achou a citação
de uma guarda que não existe: **tirar a guarda sem procurar quem a nomeia
deixa a referência morta** — o mesmo buraco por outro lado. Na volta seguinte
o atalho saiu do código junto com a sonda que ele poupava (o uid passou a vir
do `/proc/self/status`, sem custo de arquivo), e o teste que citava a guarda
foi trocado pelos testes da regra nova (`config_phz.rs::terceiro_no_par`).

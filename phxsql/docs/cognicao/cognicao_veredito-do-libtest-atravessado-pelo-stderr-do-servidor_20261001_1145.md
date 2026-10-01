# O veredito do libtest sai em tres `write`, e o stderr do servidor cabe no meio

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 617: o `bancada/guardas/provar-guardas.py` deu QUEBRADA (2/3 cairam)
para `laco-preso-no-unico-secundario` em 1 de 4 corridas. QUEBRADA com 2/3 e
o ramo «o teste nao existe mais neste binario»: um dos `caem` sumiu da saida
analisada. O executor juntava stdout e stderr no mesmo cano
(`stderr=subprocess.STDOUT`) e lia os vereditos com
`^test (\S+) \.\.\. (ok|FAILED|ignored)`.

## 2. O que eu conclui primeiro, e estava errado

A suspeita escrita no pedido era «as linhas `test ... FAILED` se misturam com
o stderr». Parecia bastar uma linha do servidor no MEIO de outra -- e isso so
acontece se o libtest escreve a linha em pedacos, o que eu supunha que nao
(o stdout do Rust e `LineWriter`, que despeja na quebra). Supor isso mataria a
hipotese certa: o `LineWriter` nao entra, porque o libtest faz `flush` a cada
pedaco.

## 3. O que a medicao disse

- `strace -f -e trace=write` num binario de teste do servidor: a linha do
  veredito sai em **tres** chamadas no fd 1 -- `"test X ... "`, `"FAILED"`,
  `"\n"` --, e os servidores escrevem «PhxSql 0.19.0 escutando em ...» e
  «porta de dados escutando ...» no fd 2, **fora da captura** do libtest.
- A janela existe em toda corrida daquela bateria: o cenario marcado segura o
  `COFRE` exclusivo, os vizinhos esperam, e sobem os servidores deles no
  instante em que ele termina -- o instante do veredito dele. Em **5 de 5**
  corridas com os canos misturados, um «escutando» caiu na linha logo depois
  de um veredito (1, 1, 2, 2, 1 vezes); em **0 de 5** caiu no meio. A queda
  e rara e a janela e certa.
- 10 corridas da guarda com o defeito reposto: **9 PROVADA, 0 QUEBRADA**, e 1
  ESTRAGOU por `ENOSPC` no disco do conteiner (o `seguem` caiu ao criar a
  tabela) -- ruido de ambiente, e nao a causa.

## 4. A regra

Veredito de processo se le so do cano onde o processo o escreve: nunca junte
stderr ao stdout antes de analisar linha por linha.

## 5. Como esta guardado hoje

`colher` e `vereditos_do_libtest` no `provar-guardas.py`: canos separados, e o
resumo `failures:` do proprio libtest como segunda fonte das quedas (um `ok`
atravessado continua sumido -- o veredito nao afrouxa). A prova e o
`--autoteste-analisador` (dentro do `--autoteste`): um filho de verdade
escreve a linha em tres `write` com stderr no meio; cada uma das duas defesas,
tirada sozinha, derruba a conferencia dela. O buraco que fica: a corrida real
de 2/3 nao se reproduziu nas 10 medidas, entao o conserto esta provado contra
o mecanismo, e nao contra a ocorrencia.

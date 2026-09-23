# Escopo congelado da 0.19

**Decisao do dono, 23/09/2026:** congelar o escopo. Pedido que eu abrir e que
NAO estiver nesta lista vira registro e **nao vira frente** ate a 0.19 sair.

A unica excecao, e ela e estreita: **achado que perde dado, vaza segredo ou
desfaz o que acabou de entrar** fura a fila -- e eu digo o motivo quando
furar, com o numero. Foi o que aconteceu hoje com o 422: codigo do 407
estourou uma catraca e o conserto entrou no mesmo dia.

## A regua, nesta ordem

1. **Corrompe ou perde dado.** Nada esta acima disto.
2. **Vaza segredo.** Duas das quatro aqui quebram petrea viva
   («senha nunca em texto puro»).
3. **Mente para quem le.** Documento ou tela que afirma o que nao e.
4. **Quebra o que ja prometemos** a quem ja usa.

O que a regua DEIXA DE FORA, de proposito: higiene de gerador, contagem de
catraca, cosmetico de tela, pesquisa, e tudo o que so o time enxerga. Sao
legitimos e continuam registrados -- so nao seguram a versao.

## A lista: 14 pedidos (eram 14; o 355 saiu porque ja estava fechado, e o 426 entrou pela excecao)

### A. Corrompe ou perde dado (5)

| pedido | o que |
|---|---|
| 392 | A escala do `Decimal` do PRIMEIRO braco corrompe o valor dos outros em **100x** no `unir`, **sem aviso**. Valor monetario errado com cara de certo. |
| 262 | Gatilho AFTER que grava pela mesma sessao dentro do COMMIT **nao chega a gravar**. Escrita que o cliente julga feita. |
| 419 | Sub-pedido que parou EM `max_linhas` publica parcial **calado**: `COUNT(*)` de um milhao responde mil. |
| 255 | Tomada no meio de BULKINSERT ou de `reindexar` deixa a tabela **recusando** ate um `reindexar` manual. |
| 381 | Linha com `.memo` corrompido **nao se consegue ALTERAR** por cliente que omita a coluna de sistema. |

### B. Vaza segredo (3, era 4)

| pedido | o que |
|---|---|
| 372 | O `dblink.json` grava a senha do destino **em texto puro, por padrao**. Quebra petrea. |
| 275 | O driver ODBC e o unico cliente que manda `senha` **em texto puro no login**. Quebra petrea. |
| ~~355~~ | **SAI DA LISTA em 23/09/2026: ja estava FECHADO** desde 18/09 (`a51f1a3`), por decisao do dono. Eu o pus aqui lendo a marca do `PENDENCIAS.md`, que estava velha -- o pedido estava fechado em codigo e aberto no papel. A marca foi virada; o erro foi meu e esta registrado no pedido 425. |
| 339 | Chave da API no `localStorage`, e o resto dos achados de alcance do parecer externo. |

### C. Seguranca de rede (2)

| pedido | o que |
|---|---|
| 278 | O pulso do cluster **aceita identidade auto-declarada**, e uma epoca forjada rebaixa o lider. |
| 312 | O aperto de mao le do soquete **FORA do `Canal`**, portanto sem teto nenhum. |

### D. O dossie mente para quem abre o link (2)

| pedido | o que |
|---|---|
| 326 | O dossie compartilhado mostra uma versao **FIXADA e antiga**: quem abre o link nao ve o que a rodada fez. |
| 327 | O botao «baixar» do dossie **nao funciona** para quem ve a pagina. |

### F. O que a revisao SEC achou no que entrou HOJE (1)

Entrou pela excecao estreita que o dono escreveu: **desfaz o que acabou de
entrar**. O pedido 422 tirou a migracao de baixo da trava global as 19:13, e
a revisao adversaria mediu o preco as 20:00.

| pedido | o que |
|---|---|
| 426 | **ALTO de integridade.** O `COMMIT` que esbarra numa migracao sai PELA METADE -- uma tabela com a linha, a outra sem --, manda o cliente REPETIR uma transacao ja metade aplicada, e a recuperacao do caminho de erro e codigo MORTO porque a trava do topo ainda esta viva. O item (c) e ANTERIOR a esta rodada; o 422 so o tornou alcancavel por operacao normal. |

### E. Terminar o que ficou pela metade (1)

| pedido | o que |
|---|---|
| 422 | As **tres irmas** que ainda seguram a trava global (`declarar_fk`, `excluir_fk`, `marcar_lgpd`). A catraca esta VERDE com elas quebradas, porque o mapeador e estatico -- e e por isso que este pedido nao fecha sozinho. |

## O que NAO entra, e vale dizer por que

- **239 (isolamento acima de READ COMMITTED, TLS no transporte)** -- os dois
  sao decisao do dono e um deles esbarra na petrea de zero dependencias.
- **333 (chat e robo de mensagens)**, **304/305 (ideias do Excel e do
  construtor visual)** -- produto novo, nao conserto.
- **P2P (251), colmeia, correio (159)** -- pilares proprios.
- **Toda a familia de higiene**: 383, 384, 397, 398, 399, 400, 410, 412, 413,
  415, 417, 420, 421, 423. Sao reais e ficam registrados. Nenhum deles
  impede alguem de usar o banco.

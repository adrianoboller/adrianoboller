# Instrumento mais curto que a coisa medida acusa defeito que não existe

- **Quando:** 2026-09-02, 21:43
- **Onde:** prova do DbLink entre dois contêineres
- **Custo:** zero, porque medi antes de escrever; teria sido um defeito falso
  registrado num documento

## O que aconteceu

Apontei o DbLink de `phx-a` para a porta 5000 de `phx-b`, que fala o protocolo
do PhxSql e não o fio do MySQL. O `dblink_testar` **não respondeu**, e o meu
cliente Python estourou.

Já estava escrevendo «o DbLink trava quando o outro lado não fala MySQL» — que
seria um defeito grave, porque uma tentativa presa poderia segurar a trava
global e parar o servidor inteiro.

## O que eu concluí primeiro, e estava errado

Que «não respondeu» era propriedade do **servidor**. Era propriedade do **meu
cliente**: eu tinha posto `socket.create_connection(..., 8)` — oito segundos de
espera — e o DbLink desiste em dez.

O instrumento era mais curto que a coisa medida, e o que ele mediu foi ele
mesmo.

## O que a medição disse

Com o prazo do cliente em 180 s:

| | |
|---|---:|
| `dblink_testar` contra porta que não fala MySQL | responde em **10,2 s** |
| a resposta | `[SP000018] dblink mysql: leitura falhou: Resource temporarily unavailable` |
| **outra conexão durante essa espera** — login + listar bancos | **0,32 s** |

Ou seja: o recurso **tem** prazo próprio, devolve erro nomeando o que falhou, e
**não segura a trava global** — que era exatamente o defeito que eu ia relatar.

## A regra

**Antes de chamar «travou», confira de quem é o prazo que estourou.** Todo
medidor tem um limite, e um limite menor que o do medido transforma «demorou»
em «travou» — que são coisas diferentes e pedem consertos opostos.

Prático: quando um teste tem prazo, ele precisa ser **maior que o do recurso**,
e a diferença precisa ser visível. Prazo de cliente escolhido por hábito (8 s,
30 s) é a fonte mais barata de defeito imaginário.

## Como está guardado hoje

O `bancada/docker/exercitar-dois.py` usa prazo folgado, e o achado — o de que a
tentativa não segura a trava — ficou registrado no pedido 166 e no
`bancada/docker/LEIA-ME.md`, com o número dos dois lados: 10,2 s de espera
contra 0,32 s de outra conexão atendida no meio dela.

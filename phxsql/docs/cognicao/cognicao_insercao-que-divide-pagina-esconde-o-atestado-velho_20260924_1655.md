# A inserção que divide página esconde o atestado velho, e o teste passava com o defeito

**Estado:** PENDENTE

## O que aconteceu

Pedido 522. O atestado do processo (`ndx.rs`, `ATESTADOS`) diz que o 1 do byte
52 é coerente no núcleo, e sai **antes da primeira mudança**
(`levantar_marca`), para um pânico ou uma queda no meio não deixar a
reabertura confiando na árvore de antes. O teste
`a_escrita_que_nao_terminou_tira_o_atestado` abria a tabela atestada,
inseria **2.989** linhas, caía sem `Drop` e exigia que a reabertura mandasse
reconstruir.

Com o defeito reposto — a retirada do atestado apagada —, o teste **passou**.

## O que eu concluí primeiro, e estava errado

Que o teste provava a retirada. Ele provava outra coisa: a inserção que divide
página regrava o cabeçalho **na hora** («a ESTRUTURA vai na hora»,
`inserir`), o CRC do cabeçalho muda, e o atestado — que é pelo CRC — deixa de
casar sozinho. O defeito ficava invisível por causa de uma segunda proteção,
e não por causa da primeira.

## O que a medição disse

Com 2.989 inserções: defeito reposto, teste verde. Com **9** inserções (sem
divisão, o contador fica em RAM e o cabeçalho do arquivo continua o que o
`fechar` atestou — o teste agora confere o CRC como premissa): defeito
reposto, teste **vermelho**; conserto, verde. O `.reg` fica nove linhas à
frente de uma árvore que a reabertura aceitaria.

O mesmo desenho em outro par: tirar só a porta do `Drop` que não atesta
depois de um `fsync` recusado **não é sentido** (os 7 testes de
`disco-que-recusa` passam, porque a abertura também recusa o atestado);
tirar só a da abertura é.

## A regra

Quando duas proteções cobrem o mesmo defeito, o teste de uma delas tem de
montar o caso que a **outra não alcança** — senão ele prova a outra.

## Como está guardado hoje

- `bancada/guardas/catalogo.py`: `atestado-sobrevive-a-escrita` (vermelha
  com 9 linhas, 24/09/2026), `atestado-pelo-caminho-e-nao-pelo-arquivo`,
  `atestado-de-antes-da-recusa-vale-depois`, e a
  `drop-baixa-o-byte-52-depois-do-fsync-recusado`, que envelheceu com o 522
  e passou a repor os dois pontos juntos, com o motivo medido no `porque`;
- o teste confere a premissa do CRC igual antes de concluir — se alguém um
  dia fizer o contador ir ao cabeçalho a cada chave, a premissa cai e diz por
  quê, em vez de o teste voltar a passar por engano.

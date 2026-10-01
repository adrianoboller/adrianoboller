# Quem citava o valor calculado era o slot, e não o `coagir`

**Estado:** PENDENTE

## O que aconteceu

Pedido 558: a conta `faixa Int1 = renda / 1000`, com `renda` marcada e
`faixa` sem marca, citava o número derivado na recusa. O parecer SEC (C3)
apontava o `coagir` da expressão («o numero 500»). Medido pelo protocolo, a
recusa real era outra: `[SP000018] limite excedido: 500 nao cabe em inteiro de
8 bits` — o `escrever_inline` do slot, na gravação, passando pela porta do 464
da coluna de DESTINO, que é sem marca e por isso cita.

## O que eu concluí primeiro, e estava errado

Que bastava a `descricao` da expressão parar de citar número e booleano. O
teste de vermelho mostrou o contrário: o `coagir` devolve `Value::Int(500)` sem
reclamar (cabe num `i64`), e a faixa de 8 bits só se confere no slot. Redigir
só a `descricao` deixava a porta principal aberta.

## O que a medição disse

Com o defeito reposto, três caminhos caem, cada um por um motor diferente:
o slot (`500 nao cabe em inteiro de 8 bits`), o conversor de data dentro do
`coagir` (`data invalida: "999.888.777-66"`) e a `descricao`
(`o booleano true`). O conserto que fecha os três sem duplicar regra: o
`coagir` confere a faixa chamando o MESMO `escrever_inline` num rascunho, e
joga fora a frase do conversor.

## A regra

Antes de redigir a mensagem que o parecer cita, reproduza a recusa pelo
protocolo e leia QUEM a escreveu: a frase citada pode sair de outro motor.

## Como está guardado hoje

Teste `servidor::testes_recusa_sem_dado_pessoal::a_conta_que_parte_de_coluna_marcada_nao_cita_o_valor`
e guarda `conta-cita-numero-de-coluna-marcada` no catálogo, PROVADA em
01/10/2026. O buraco que fica: a porta do 464 continua olhando só o destino
para valor que não passou por expressão; o que cobre o calculado é o motor da
expressão, não a coluna.

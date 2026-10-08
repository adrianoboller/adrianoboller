# Expurgo errado no diário para o central em vez de perder venda, e é a base que garante isso

**Estado:** PENDENTE

## O que aconteceu

Pedido 706: o `.log` do caixa passou a sair por volume. Na prova pelo soquete
(`crates/phxsql-server/tests/expurgo-do-diario-pelo-soquete.rs`), repus o
defeito mais perigoso, o expurgo que trata todo volume como confirmado
(`confirmado_por_todos` devolvendo `u64::MAX`). Os dois testes caíram em 60 s
com «o central alcançar 6.000 vendas»: o caixa apagou os volumes antes de o
central puxá-los, e o central parou na recusa `erro.diario_expurgado`, sem
gravar uma linha errada.

## O que eu concluí primeiro, e estava errado

Que a prova do defeito seria a contagem do central: menos vendas lá que no
caixa. Não é. Com a base no cabeçalho (bytes 104..112), o central não chega a
ter «menos» de forma calada. Ele **para** no primeiro evento que já saiu. A
venda some do caminho, mas o caminho avisa. Sem a base, a contagem também não
pegaria nada: a posição deslizaria e o central receberia o evento N+k no lugar
do N, com a contagem crescendo normalmente.

## O que a medição disse

- Store (`crates/phxsql-store/tests/expurgo-do-diario.rs`): com a varredura
  contando a partir de zero em vez da base, 3 dos 9 testes caem (a posição
  entrega outro evento). Com o plano que nunca tira nada, 6 caem, e o diário
  chega a **3.046.016 bytes** contra o teto de 262.144 (4 volumes de 64 KiB)
  que o expurgo mantém.
- Soquete: com o defeito de «tudo confirmado», 2 de 2 caem. Com o conserto, o
  caixa fica em até 4 volumes com o central em dia, segura tudo com o fio
  caído (zero venda recusada) e o central que volta tem as 12.000 linhas
  iguais às do caixa.

## A regra

Antes de apagar o começo de qualquer coisa contada por posição, grave a base no
primeiro sobrevivente e leve-a ao disco. Pedir abaixo dela tem de ser recusa
dita, nunca o evento seguinte.

## Como está guardado hoje

Os dois testes acima, mais `base_perdida_no_primeiro_volume_recusa`, que zera
a base com o CRC refeito e cobra a recusa. **O buraco que ficou:** o «refazer
o central por cópia» depois do prazo de 30 dias é procedimento escrito, sem
rota automática (o item (d) do parecer do papel C). Fica no 706 como resto.

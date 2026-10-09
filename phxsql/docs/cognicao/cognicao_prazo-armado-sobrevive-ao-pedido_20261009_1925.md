# O prazo armado por um pedido sobrevivia ao pedido

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 19:25
- **Onde:** `crates/phxsql-server/src/telemetria.rs` (`Atividade::prazo_ate_ms`, `siga`),
  `crates/phxsql-server/src/servidor/servico_transacao_01.rs` (`por_prazo_na_operacao`),
  `crates/phxsql-server/src/servidor.rs` (`despachar`)
- **Pedido:** 765, fatia P2

## O que aconteceu

O `STATEMENT TIMEOUT` da transação era posto no relógio da atividade da conexão
(`definir_prazo`) a cada operação da transação, e nada o tirava depois. O `comecou_pedido`
zera os passos, os alarmes e a fase, mas não o prazo. Resultado: `BEGIN` com
`statement_timeout` de 20 ms, um `ler`, `COMMIT`, e a varredura seguinte da mesma conexão
saía `Cancelado`, citando o STATEMENT TIMEOUT de uma transação que já tinha acabado.

## O que eu concluí primeiro, e estava errado

Que a P2 era só acrescentar um segundo dono ao relógio que já existia. O comentário do campo
dizia «zero = sem prazo, e é o valor de toda operação fora de transação». Era verdade só
para a conexão que nunca tinha aberto uma transação com prazo. O defeito só apareceu quando
a prova da P2 testou o caso de fora, antes do caso de dentro.

## O que a medição disse

`o_prazo_da_transacao_confirmada_nao_vaza_para_o_pedido_seguinte`, sem o
`armar_prazo_de_comando` do `despachar`: FAILED (o `Cancelado` da varredura de 20.000 linhas,
60 ms depois do `COMMIT`). Com ele: ok. A mesma troca derruba os outros três testes do
arquivo. A entrada `prazo-de-comando-nao-armado` do catálogo de guardas repõe o defeito.

## A regra

Estado posto por pedido se arma e se zera no mesmo ponto, uma vez por pedido. Quem só arma
depende de alguém lembrar de tirar.

## Como está guardado hoje

O `despachar` arma o prazo de comando a cada pedido, e armar com zero também zera
(`Atividade::armar_prazo_de_comando`). Ficou um buraco: sem atividade amarrada (a telemetria
desligada) não há relógio, e nem o prazo de comando nem o STATEMENT TIMEOUT mordem. O
`encerrar` manual tem o mesmo limite.

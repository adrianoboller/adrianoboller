# O corte do fio cedo demais não distingue o id de transação da ordem de puxar

**Estado:** PENDENTE

## O que aconteceu

Pedido 676: a réplica passou a juntar as tabelas pelo id de transação do `.log`
(`replica::Juntador`) e a aplicar a venda inteira sob uma tomada da trava. A
prova pelo soquete (`tests/venda-inteira-na-replica.rs`) derruba o fio num
`replicar` escolhido e confere que o central parado mostra `(vendas, itens,
pagamentos)` igual a `(0,0,0)` ou `(1,N,1)`.

Com o corte no **2.º** `replicar` as duas primeiras provas caíram com a
aplicação por lote reposta — `(0,3,0)` e `(0,500,0)` —, e eu as dei como prova
do id. Repondo o **outro** defeito (a tomada da trava sem abrir a unidade do
diário, cada evento com id próprio), a prova de 3 itens **continuou verde**.

## O que eu concluí primeiro, e estava errado

Que uma prova que cai com a aplicação por lote cai também sem o id, porque
«sem o id a venda se aplica em pedaços». Plausível e incompleto: o `Juntador`
**puxa toda tabela vazia antes de decidir qualquer coisa**, então no 2.º
`replicar` (pedindo `pagamentos`) ainda não chegou nada da tabela `vendas`, e
nada se aplica — com id ou sem id. O que segurou a venda ali foi a **ordem de
puxar**, e não o id.

## O que a medição disse

- corte no 2.º, 3 itens, sem a unidade: **verde** (a prova não distingue);
- corte no 2.º, 600 itens, sem a unidade: `(1,499,0)` — cai, porque o 1.º lote
  já trazia 500 itens com ids diferentes;
- corte no **4.º** (as três tabelas com algo na mão, o 2.º lote dos itens
  pendente), 600 itens: sem a unidade cai, e com a aplicação por lote reposta
  cai com `(0,600,1)`.

## A regra

Quando o conserto tem duas peças (aqui, a ordem de puxar e o id), escolha o
ponto de corte da prova **depois** de a primeira peça ter feito tudo o que
faz sozinha — senão a prova mede só a primeira, e a segunda pode sumir verde.

## Como está guardado hoje

Duas entradas no catálogo, cada uma com o seu `caem`:
`replica-aplica-o-que-chegou-sem-esperar-a-transacao` (as três provas) e
`tomada-da-trava-sem-unidade-do-diario` (as duas que cortam com as tabelas na
mão, e a de 3 itens no `seguem` — é ela que documenta que não distingue). RED
medido à mão; o provador não rodou nesta frente.

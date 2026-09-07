# 0.3) Como funciona o timeout de um lock record para transaction

*Medido em 2026-09-07 16:36 UTC pela frente do item U (`bancada/transacoes/travada.py`), commit `a56a165`; escrito em 2026-09-07 16:48 UTC.*

## Resposta curta

São **três prazos**, e um número só não responderia aos três
(`docs/TRANSACOES.md` §4.7):

| prazo | o que limita | campo do `config.json` | padrão |
|---|---|---|---|
| `TIMEOUT` | a transação **inteira** | `recursos.transacao_prazo_min` | 5 min |
| `LOCK TIMEOUT` | quanto se aceita **esperar por outro** numa linha travada | `recursos.transacao_lock_timeout_ms` | 500 ms |
| `STATEMENT TIMEOUT` | quanto **uma operação** pode levar | `recursos.transacao_statement_ms` | 0 = sem prazo |

O **lock de linha** funciona assim: a transação A que altera a linha toma a
trava **X** dela; a transação B que tenta a mesma linha **espera** até o
`LOCK TIMEOUT` e, vencido, recebe `4005 EM_TRANSACAO` com a mensagem dizendo
quem trava, desde quando, e que esperou N ms — B decide se tenta de novo
(`"repetir": true`). A trava solta no `COMMIT` ou no `ROLLBACK` de A.

**Quem encerra é o gestor de transações, nunca uma thread morta**: vencido o
`TIMEOUT` da transação, ela vai para `ABORT_ONLY`, **solta as travas na
hora**, joga a lista fora, e a próxima operação de A recebe `6002
TRANSACAO_ABORTADA` com o prazo dentro. A conexão continua viva. Os prazos se
declaram **por pedido** no `begin` (`"timeout":"1500ms"`,
`"lock_timeout":"400ms"`) ou no `config.json` para o servidor inteiro.

## Exemplo exercitado

Medido pelo soquete, com duas conexões (o exercício inteiro está na resposta
**U**):

```text
LOCK TIMEOUT declarado por B ..... 400 ms
espera medida de B ............... 401,6 ms
erro que B recebeu ............... 4005 EM_TRANSACAO, "repetir": true
   "a linha 1 de loja/contas esta travada (X) pela transacao 1788798971091
    (ligacao 3), aberta em 2026-09-07 16:36:11,193 ha 0s; ela solta no COMMIT
    ou no ROLLBACK. Esperei o LOCK TIMEOUT de 400 ms e desisti"

TIMEOUT declarado por A .......... 1500 ms  (A trava a linha e some)
espera do teste .................. 2,0 s
proxima operacao de A ............ 6002 TRANSACAO_ABORTADA
   "a transacao 1788798971093 passou do TIMEOUT de 1500 ms e foi revertida;
    as travas dela ja sairam. Mande ROLLBACK para fechar"
saldo da linha depois ............ 100 (nada de A sobreviveu)
```

Os dois códigos são os documentados no `docs/TRANSACOES.md` §9, e a prova é
nos dois sentidos: B só desistiu **depois** dos 400 ms, e A só foi abortada
**depois** dos 1.500.

## O que NÃO existe, e é dispensa registrada

- **`STATEMENT TIMEOUT` só morde onde há laço.** Ele usa o cancelamento
  cooperativo (`Atividade::siga`), conferido entre unidades de trabalho
  seguras — uma inserção de **uma** linha não tem ponto de cancelamento no
  meio, e não poderia ter: parar entre gravar o slot e manter o índice
  deixaria os dois discordando. Um prazo que promete cortar qualquer coisa
  seria configuração que mente.
- **Detecção de deadlock** não existe: dois que se travam mutuamente esperam
  cada um o seu `LOCK TIMEOUT` e um deles desiste. É o desenho mais barato, e
  é honesto — com o prazo padrão de 500 ms, o custo de um abraço mortal é
  meio segundo, não uma transação pendurada.
- **Cancelar por fora**: `telemetria_encerrar` devolve `ociosa` para uma
  transação parada entre pedidos (não há laço ali para cancelar); o caminho é
  `encerrar_sessao`, que fecha o soquete e cai na mesma rede de proteção da
  queda de conexão — provado no item U.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/transacoes/travada.py
```

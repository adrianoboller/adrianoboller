# U) testar como cancelar uma transação atômica travada

> Corrida em 2026-09-07 16:36:11 UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/transacoes/travada.py`

## Resposta curta

**Existem TRÊS jeitos, e os três funcionam** — provados pelo soquete, não
lidos no código. "Travada" aqui é uma conexão A que abriu `begin`, mexeu numa
linha e não mandou `commit` nem `rollback`:

1. **Pela própria conexão** — `rollback`. Imediato, sempre disponível.
2. **Pelo prazo, sozinho** — o `TIMEOUT` da transação inteira estoura e o
   **gestor** aborta, sem matar thread nenhuma (`docs/TRANSACOES.md` §4.7).
3. **Pelo administrador** — e aqui há uma nuance que só a prova real revelou:
   `telemetria_encerrar` devolve **`ociosa`** para uma transação parada entre
   pedidos (não há laço com ponto de cancelamento ali dentro), e aponta o
   caminho certo: `encerrar_sessao`, que fecha o soquete e cai na **mesma**
   rede de proteção da queda de conexão.

Nos três casos a linha **volta ao valor de antes** (100) — nada do que A tinha
empilhado sobrevive. De brinde: B esperou **401,6 ms** pelo `LOCK TIMEOUT` de
400 ms declarado e recebeu **4005 EM_TRANSACAO**; e a próxima operação de A
depois do `TIMEOUT` de 1500 ms vencido recebeu exatamente **6002
TRANSACAO_ABORTADA** — os dois códigos documentados em `docs/TRANSACOES.md`
§9.

## Exemplo exercitado

### 1. LOCK TIMEOUT (a espera de B) + cancelamento pela própria conexão

```
== 1. LOCK TIMEOUT (a espera de B) + cancelamento PELA PROPRIA CONEXAO (A faz rollback) ==
  OK    A trava a linha (atualizar dentro do begin)  -- {"empilhada": true, "acao": "atualizar", "rowid": 1, "linhas": 1, "transaction_id": 1788798971091, "transaction_state": "ACTIVE", "ok": true, "op": "atualizar",
  OK    B espera e recebe o codigo documentado (4005 EM_TRANSACAO)  -- {"ok": false, "op": "atualizar", "erro": "[SP000006] tabela em transacao: a linha 1 de loja/contas esta travada (X) pela transacao 1788798971091 (ligacao 3), aberta em 2026-09-07 16:36:11,193 ha 0s; ela solta no COMMIT o
  OK    a mensagem cita o LOCK TIMEOUT  -- [SP000006] tabela em transacao: a linha 1 de loja/contas esta travada (X) pela transacao 1788798971091 (ligacao 3), aberta em 2026-09-07 16:36:11,193 ha 0s; ela solta no COMMIT ou no ROLLBACK. Esperei
  OK    B esperou por volta dos 400 ms declarados  -- esperou 402 ms
    tempo de espera medido: 401.6 ms
    erro completo de B: {"codigo": 4005, "nome": "EM_TRANSACAO", "erro": "[SP000006] tabela em transacao: a linha 1 de loja/contas esta travada (X) pela transacao 1788798971091 (ligacao 3), aberta em 2026-09-07 16:36:11,193 ha 0s; ela solta no COMMIT ou no ROLLBACK. Esperei o LOCK TIMEOUT de 400 ms e desisti", "repetir": true}
    saldo ANTES do cancelamento (por uma conexao que nao viu a transacao de A): 100
  OK    A cancela pela propria conexao (ROLLBACK)  -- {"transaction_id": 1788798971091, "transaction_state": "ROLLED_BACK", "descartadas": 1, "ok": true, "op": "rollback", "ms": 0}
    ROLLBACK de A: {"transaction_state": "ROLLED_BACK", "descartadas": 1}
  OK    depois do ROLLBACK a linha volta a valer 100 (nada de A foi gravado)  -- saldo ficou 100
    >>> a linha ficou valendo: saldo=100
```

**A linha ficou valendo: `saldo=100`.**

### 2. O `TIMEOUT` da transação inteira aborta A sozinho

```
== 2. o TIMEOUT da transacao inteira aborta A SOZINHO ==
  OK    A abre com TIMEOUT de 1500 ms (por pedido, sem mexer no config)  -- {"transaction_id": 1788798971093, "transaction_state": "ACTIVE", "transaction_start_time": "2026-09-07 16:36:11,599", "transaction_isolation": "escrita serializavel por tabela, leitura confirmada e na
  OK    A trava a linha e NAO manda commit nem rollback  -- {"empilhada": true, "acao": "atualizar", "rowid": 1, "linhas": 1, "transaction_id": 1788798971093, "transaction_state": "ACTIVE", "ok": true, "op": "atualizar",
    ... esperando 2,0 s (o TIMEOUT declarado e de 1,5 s) sem A mandar nada ...
    saldo enquanto A ainda 'segura' a transacao vencida (ninguem perguntou nada a A ainda): 100
  OK    a proxima operacao de A recebe 6002 TRANSACAO_ABORTADA  -- {"ok": false, "op": "atualizar", "erro": "[SP000006] transacao abortada: a transacao 1788798971093 passou do TIMEOUT de 1500 ms e foi revertida; as travas dela ja sairam. Mande ROLLBACK para fechar", "codigo": 6002, "nom
    erro completo: {"codigo": 6002, "nome": "TRANSACAO_ABORTADA", "erro": "[SP000006] transacao abortada: a transacao 1788798971093 passou do TIMEOUT de 1500 ms e foi revertida; as travas dela ja sairam. Mande ROLLBACK para fechar", "repetir": false}
  OK    depois do aborto por prazo a linha continua em 100 (nada de A foi gravado)  -- saldo ficou 100
    >>> a linha ficou valendo: saldo=100
```

**A linha ficou valendo: `saldo=100`.** Note que o `TIMEOUT` foi declarado
**por pedido** (`{"op":"begin","timeout":"1500ms"}`), não mexendo no
`config.json` — ver a nota sobre `transacao_prazo_min` abaixo.

### 3. O administrador: o limite honesto de `telemetria_encerrar`, e o `encerrar_sessao` que resolve

```
== 3. cancelamento PELO ADMINISTRADOR: telemetria_encerrar e encerrar_sessao ==
  OK    peguei o numero de ligacao de A pela propria resposta do begin  -- {"transaction_id": 1788798971094, "transaction_state": "ACTIVE", "transaction_start_time": "2026-09-07 16:36:13,607", "transaction_isolation": "escrita serializ
  OK    A trava a linha e fica OCIOSA (nao manda mais nada)  -- {"empilhada": true, "acao": "atualizar", "rowid": 1, "linhas": 1, "transaction_id": 1788798971094, "transaction_state": "ACTIVE", "ok": true, "op": "atualizar",

  3a. telemetria_encerrar na atividade de A -- e o limite HONESTO desta ferramenta
  OK    o servidor acha a atividade de A (ela existe, so nao esta rodando nada)  -- {"id": "dados:6", "estado": "ociosa", "op": "telemetria_encerrar", "quem": "token de servico", "quando": "2026-09-07 16:36:13,908", "aviso": "nao havia operacao em curso. Para derrubar a CONEXAO inteira, use `encerrar_se
  OK    o estado e OCIOSA -- a transacao esta parada ENTRE pedidos, nao dentro de um laco  -- {"id": "dados:6", "estado": "ociosa", "op": "telemetria_encerrar", "quem": "token de servico", "quando": "2026-09-07 16:36:13,908", "aviso": "nao havia operacao em curso. Para derrubar a CONEXAO inteira, use `encerrar_se
  OK    o aviso aponta o caminho certo: encerrar_sessao  -- nao havia operacao em curso. Para derrubar a CONEXAO inteira, use `encerrar_sessao` -- ele fecha o soquete.
  OK    telemetria_encerrar sozinho NAO desfez nada (a linha continua mostrando o valor de antes)  -- saldo ficou 100

  3b. encerrar_sessao(id) -- o que REALMENTE cancela a transacao travada
  OK    encerrar_sessao aceita o mesmo numero  -- {"encerrada": 6, "estava": "esperando", "op": "encerrar_sessao", "aviso": "a conexao estava esperando pedido e foi fechada na hora", "quando": "2026-09-07 16:36:13,909", "ok": true, "ms": 0}
  OK    a sessao de A sumiu da lista (o soquete foi fechado)
  OK    depois do encerrar_sessao a linha volta a valer 100 (a queda desfez a transacao)  -- saldo ficou 100
    >>> a linha ficou valendo: saldo=100
```

**A linha ficou valendo: `saldo=100`.** `telemetria_encerrar` **não é** o
botão de matar transação — é o botão de abortar um **laço em andamento**
(soma, exportação, carga). Para uma transação parada entre pedidos, ele diz
isso com todas as letras (`estado: "ociosa"`) e aponta `encerrar_sessao`, que
funciona porque cai na mesma rede de proteção que já desfaz a transação
quando a conexão cai de verdade (provada em `bancada/transacoes/provar.py`,
item 5).

## O que NÃO existe, e é dispensa registrada

- **`telemetria_encerrar` não cancela uma transação parada entre pedidos** —
  por desenho, não por lacuna: o cancelamento é cooperativo e só olha a marca
  em **pontos seguros** dentro de laços (`docs/TELEMETRIA.md` §4.1/§4.3), e
  `inserir`/`atualizar`/`excluir` de uma linha são uma unidade só, sem "meio"
  onde parar. Uma transação ociosa não está dentro de nenhum desses laços, e a
  resposta (`ociosa`) diz isso em vez de fingir um cancelamento que não
  aconteceu.
- **`recursos.transacao_prazo_min` não aceita fração.** É lido como inteiro
  (`inteiro_ou`, `.max(0)`) em **minutos**; um valor `<= 0` cai no padrão (5).
  O mínimo prático pelo `config.json` é 1 minuto — rápido demais para provar
  num script não é o `config.json`, é o `begin` por pedido: `"timeout":"1500ms"`
  (ou `"2s"`, `"5s"`…) vale só para aquela transação, sem tocar no padrão do
  servidor. Foi o que este teste usou no cenário 2.
- **Não há um quarto jeito "matar a thread".** A lei da casa é explícita
  (`docs/TRANSACOES.md` §4.7): "quem encerra é o gestor de transações, nunca
  uma thread morta" — matar a thread deixaria a trava de dados presa, o
  conjunto de escrita órfão e o contador de transações abertas errado.

## Como se refaz

```bash
python3 bancada/transacoes/travada.py
```

`PHX_SONDA_PORTA` (padrão 6720). Detalhe em `bancada/transacoes/LEIA-ME.md`.

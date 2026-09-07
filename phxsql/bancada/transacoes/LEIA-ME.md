# `travada.py` — os três jeitos de cancelar uma transação travada

`provar.py` já prova o desenho inteiro das transações (BEGIN/COMMIT/ROLLBACK,
queda de conexão, SQL, diário). Faltava isolar uma pergunta específica do
dono, 07/09/2026: *«testar como cancelar uma transação atômica travada»*. Uma
transação "travada" aqui quer dizer uma conexão A que abriu `begin`, mexeu
numa linha e não mandou `commit` nem `rollback` — e o script exercita os TRÊS
jeitos documentados de tirar essa linha da frente
(`docs/TRANSACOES.md` §4.7/§9 e `docs/TELEMETRIA.md` §4).

```bash
python3 bancada/transacoes/travada.py
```

Variável: `PHX_SONDA_PORTA` (padrão 6720). Cada um dos três cenários parte do
mesmo valor gravado (100) e termina lendo a linha por uma conexão que nunca
travou nada, para provar que o cancelamento não deixou a escrita pela metade:
(1) pela própria conexão (A manda `rollback`); (2) pelo prazo, sozinho (o
`TIMEOUT` da transação inteira estourando sem ninguém pedir nada, com o
`begin` mandando `"timeout"` por pedido em vez de mexer no `config.json`); e
(3) pelo administrador — mostrando primeiro o limite HONESTO do
`telemetria_encerrar` (devolve `ociosa` para uma transação parada entre
pedidos, porque não há laço com ponto de cancelamento ali dentro) e depois o
que realmente funciona, `encerrar_sessao`, que fecha o soquete e cai na mesma
rede de proteção da queda de conexão.

De brinde, o cenário 1 mede quanto a conexão B espera antes do `LOCK TIMEOUT`
estourar e confere o código do erro contra o documentado (4005
`EM_TRANSACAO`), e o cenário 2 confere que a próxima operação de A depois do
prazo vencido recebe exatamente `6002 TRANSACAO_ABORTADA`.

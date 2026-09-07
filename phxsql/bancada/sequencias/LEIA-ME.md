# Bancada da sequência

Exercita a coluna `Sequence` contra um `phxsqld` de pé: declarar (e a recusa da
segunda), inserir sem e com valor, listar (`sequencias`), ajustar
(`ajustar_sequencia`) — inclusive para trás de uma chave gravada, onde quem
recusa é o índice único —, e o contador lido do disco.

A primeira corrida leu o byte **92** e imprimiu 7: é o `proximo_rownum`, não a
sequência (byte **36**). Os dois contadores moram no mesmo cabeçalho e o
`FORMATO.md` documenta os dois; a sonda imprime ambos lado a lado para a
confusão não voltar.

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/sequencias/sonda.py
```

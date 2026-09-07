# Bancada da sequência — e dos três números crescentes

O PhxSql tem **três** números que sobem por tabela, e confundi-los foi o
primeiro erro desta bancada: o **`rowid`** (posição física no `.reg`, que nunca
reaproveita slot), o **`rownum`** (coluna de sistema, ordem de chegada, contador
nos bytes 92..100 do cabeçalho do volume 1) e a coluna **`Sequence`** do usuário
(uma por tabela, contador nos bytes 36..44 do **mesmo** cabeçalho). A primeira
corrida leu o byte 92 e imprimiu 7: era o `proximo_rownum`, não a sequência. A
sonda imprime os dois lado a lado para a confusão não voltar.

**Parte I (blocos 1–8)** exercita a `Sequence` como um usuário de PostgreSQL a
esperaria: declarar (e a recusa da segunda na mesma tabela), inserir sem e com
valor, listar (`sequencias`), ajustar (`ajustar_sequencia`) — inclusive para trás
de uma chave gravada, onde quem recusa é o índice único. É o que a resposta
`docs/pdf/respostas/00-sequencia.md` publica.

**Parte II (blocos 9–25)** percorre o **ciclo de vida** dos três: lote, carga
reservada (`BULKINSERT`), transação (rollback e commit), exclusão suave/física/
restaurar, as três partições, ajuste para trás **sem** índice único, cabeçalho
adulterado, queda do processo com `SIGKILL`, o teto real do número no protocolo,
backup e restauração, `reindexar`, replicação, promoção com atraso, bidirecional
e o preço de um contador durável. É de onde `docs/AUTONUMBER.md` tira **cada**
número — e por isso a sonda grava `resultados.json` com a data da corrida.

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/sequencias/sonda.py
# ou, sem compilar e fora da porta padrão:
PHX_SONDA_PORTA=7830 PHX_SONDA_BIN=/tmp/phx/phxsqld python3 bancada/sequencias/sonda.py
```

## Três armadilhas que esta bancada já pagou

- **O `spare` não atende nem leitura.** A primeira corrida do bloco 22 imprimiu
  a lista de linhas da réplica **vazia** e nada estava errado no motor: o papel
  `spare` recusa cliente com o código 4004. Trocado por `replica` com
  `somente_leitura`, que lê e continua promovível. *Instrumento antes do
  veredito.*
- **Medir as duas braçadas em bloco mede a máquina, não o código.** O contêiner
  é compartilhado (na corrida de 07/09/2026 havia três `rustc` de outra frente,
  carga 2,4). Medindo cinco corridas de uma coluna e depois cinco da outra, a
  deriva caiu inteira numa braçada e o resultado disse que a coluna `Sequence`
  era **mais rápida** que gravar o número à mão — absurdo. As braçadas se
  alternam, e o veredito só sai quando as faixas **não se cruzam** (pedido 155).
- **Guardar a leitura para o fim do bloco publica o número do estágio errado.**
  O contador do bidirecional foi impresso na tela como 1.000.002 e gravado no
  `resultados.json` como 1.000.003, porque a chamada estava no dicionário do
  fim, depois do estágio 3. Lê-se **no instante**, numa variável.

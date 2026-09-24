# `pgrep -f` dentro de um laço de espera casa o próprio laço — e o provador fantasma trava todo mundo

**Descoberta:** 24/09/2026, 03:20, pedido 450 (frente do PhxZip).

## 1. O que aconteceu

A regra de convivência manda, antes de rodar `bancada/guardas/provar-guardas.py`,
conferir com `pgrep -f provar-guardas` se outro provador está rodando, e
esperar. Para esperar, escrevi em segundo plano:

```bash
until ! pgrep -f "provar-guardas.py" >/dev/null; do sleep 10; done
```

O laço nunca terminou — nem depois de o provador vizinho acabar. E, pior, a
conferência seguinte de qualquer frente passou a responder «ocupado»: medi
`OCUPADO` com nenhum provador vivo.

## 2. O que eu concluí primeiro, e estava errado

Que o provador vizinho estava demorando (a frente do DBLink provava oito guardas
do `phxsql-server`, e cada uma recompila o servidor). Esperei três vezes,
acumulando laços — cada espera nova era mais um processo com
`provar-guardas.py` na linha de comando.

## 3. O que a medição disse

`pgrep -f` compara o padrão com a linha de comando INTEIRA de todo processo, e
a do `bash -c '... until ! pgrep -f "provar-guardas.py" ...'` contém o padrão.
O `pgrep` exclui a si mesmo, mas não o shell que o chama. Cinco laços meus
(três de `pgrep`, dois esperando a saída deles) ficaram vivos; `pgrep -fa`
mostrou só eles, nenhum `python3`. Encerrados os cinco — provados meus pelo
texto exato do comando —, o provador rodou na hora: 6/6 guardas provadas.

## 4. A regra

Para saber se o provador roda, ancore no começo da linha de comando dele:
`pgrep -f "^python3 bancada/guardas/provar-guardas"`. Padrão solto dentro de um
laço casa o próprio laço — e vira um provador fantasma que trava as outras
frentes.

## 5. Como está guardado hoje

Só neste arquivo. **Buraco:** a instrução de convivência que as frentes
recebem diz `pgrep -f provar-guardas` sem âncora; usada num laço, ela reproduz
o defeito. Quem redige a instrução precisa trocá-la pela forma ancorada.

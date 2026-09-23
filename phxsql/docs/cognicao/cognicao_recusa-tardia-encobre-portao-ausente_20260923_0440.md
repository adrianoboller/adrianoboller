# A recusa TARDIA encobre o portão ausente, e faz a prova real passar por engano

**Descoberto em 23/09/2026, 04:40** — frente U (segunda volta), pedido 393.

## 1. O que aconteceu

O braço-pedido do `unir` (`servidor.rs`, `braco_da_uniao` →
`linhas_do_sub_pedido`) roteia por `executar_derivado`, que é onde mora o
portão de permissão. A guarda que trava isso é
`testes_direito_por_tabela::o_braco_pedido_da_uniao_tambem_nao_e_a_porta_dos_fundos`,
com dois casos: um braço `varrer` sobre a tabela negada e um braço `consultar`
que a esconde um nível mais fundo.

Fiz a prova real trocando `self.executar_derivado(&op, &pedido, sessao)?` por
`self.executar(&op, &pedido, sessao)?` — o portão fora do caminho. **A guarda
passou assim mesmo**, com `ACESSO_NEGADO` e a frase certa, nos dois casos.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o `executar` também confere permissão, e que a troca era inócua.
Errado nas duas metades: `executar` é o despacho cru, sem `portoes_do_pedido`
e sem `aplicar_direito_por_coluna`; e a troca **vaza**.

Para achar de onde vinha a recusa marquei as quatro mensagens candidatas
(`pivotar`, `juntar`, `diferencas`, `unir`/`tabelas`) com uma etiqueta no
próprio texto e recompilei. **Nenhuma das quatro acendeu** — o texto vinha de
`erro.sem_direito`, o portão geral, chamado por outro caminho.

## 3. O que a medição disse

O caminho é este, e ele acontece **depois do dano**: com o portão fora,
`linhas_do_sub_pedido` lê a folha inteira por `executar("varrer")`, e só então
pede o modelo da linha — `modelo_da_tabela`, que chama
`executar_derivado("esquema")`. É **esse** derivado acessório que recusa. A
linha já foi lida; o que para é a resposta.

Prova por exclusão: `agrupar` **não** passa por `modelo_da_tabela` (o modelo
dele sai do próprio cabeçalho `colunas`). Acrescentei o terceiro caso e, com o
defeito reposto, ele devolveu:

```
called `Result::unwrap_err()` on an `Ok` value:
  ("tabelas", Lista([Texto("clientes"), Texto("folha")]))
  ("linhas",  Lista([Lista([Texto("x"), Numero(1.0)]),
                     Lista([Texto("x"), Numero(1.0)])]))
```

A segunda linha é a da folha. Com o portão de volta, recusa. **Prova real nos
dois sentidos, e só no terceiro caso.**

## 4. A regra

**Quando a prova real de um portão passar mesmo com o portão fora, procure o
sub-pedido ACESSÓRIO que ainda passa por ele — e escolha um caso que não tenha
acessório nenhum.** Recusa que chega depois da leitura não é portão, é
consolo: o dado já saiu do disco, e basta um caminho sem o acessório para ele
chegar ao cliente.

## 5. Como está guardado hoje

Os três casos moram em
`o_braco_pedido_da_uniao_tambem_nao_e_a_porta_dos_fundos`, e o comentário do
teste **diz qual deles discrimina e por que os outros dois não** — em vez de
esconder. É o mesmo formato do pedido 164, onde um dos três testes não
discriminava e isso ficou escrito no próprio teste.

**O buraco que fica nomeado:** as outras cinco composições que passam pelo
`linhas_do_sub_pedido` (o `de`, o `juntar[].de`, o `escalar[].de`, o `em[].de`
e o `existe[].de` do `consultar`) têm guardas próprias, e **não conferi se
alguma delas é do tipo que passa por engano pelo mesmo acidente**. Quem for
mexer no `linhas_do_sub_pedido` reponha o defeito e rode as cinco: a que
passar é a que não está provando nada.

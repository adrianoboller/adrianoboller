# Trecho aplicado sem contexto cai no lugar errado sem dar conflito

Papel A (integração), 24/09/2026. Defeito meu, achado pela bateria completa.

## 1. O que aconteceu

A bateria completa (`provar.py --construir`) compilou a versão entregue
(`0f7aab6`) numa cópia limpa e o `rustc` acusou `unused doc comment` em
`crates/phxsql-server/src/servidor.rs:10107`. O bloco de documentação
«QUANDO se pergunta -- pedido 442», de 10 linhas, estava **dentro** do corpo de
`teto_da_linha`, depois do primeiro `if`. Na árvore da frente dos tetos ele
estava certo, em cima da função. Quem o moveu fui eu, ao integrar aquela frente
no `5524e79` aplicando só os trechos dela com `git apply --unidiff-zero`,
porque o `servidor.rs` tinha, ao mesmo tempo, trechos de outra frente viva.

## 2. O que eu concluí primeiro, e estava errado

Que `git apply` sem erro, somado ao `trecho-vivo.py --catraca` verde, bastava. E
que a frase do commit «a árvore do commit sozinha não foi compilada» era uma
ressalva de forma, porque as frentes tinham rodado `clippy` com zero avisos.
O `clippy` delas mediu a árvore **delas**, onde o bloco estava no lugar. Com
zero linha de contexto, o `git apply` só tem o número da linha para se guiar;
se a ordem dos trechos desloca as linhas, ele encaixa o pedaço onde a conta
mandar, sem conflito nenhum.

## 3. O que a medição disse

- 1 aviso, 1 bloco de 10 linhas deslocado 3 linhas para baixo, nenhum efeito
  de comportamento (é comentário). O mesmo mecanismo teria deslocado código.
- Depois do conserto (`dabaeec`), a diferença entre o commit e a árvore de
  trabalho no `servidor.rs` são exatamente os 5 trechos da frente 372, ainda viva.
  Ou seja: das três integrações feitas com `--unidiff-zero` nesta sessão
  (436, COMMIT, tetos), só esta deslocou algo.

## 4. A regra

**Trecho aplicado sem contexto se confere antes do commit: a diferença entre o
índice e a árvore de trabalho tem de ser exatamente os trechos das frentes que
ficaram de fora. Qualquer outra linha é um trecho que caiu no lugar errado.**

## 5. Como está guardado hoje

Não está guardado por script. A conferência foi feita à mão neste conserto
(`git diff` depois do `update-index`). O buraco: a próxima integração dividida
repete o risco. O que o fecharia é uma verificação única no fluxo do integrador:
o `git diff` do índice contra a árvore listando os trechos restantes, comparados
com a lista de trechos de cada frente viva.

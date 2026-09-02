# Sabotagem que não sabota não prova nada — e eu fiz duas seguidas

- **Quando:** 2026-09-02, 22:45
- **Onde:** o teste de equivalência do `pagina_por_indice`
- **Custo:** zero, porque insisti; teria sido uma «prova real» falsa registrada
  num commit

## O que aconteceu

Escrevi o teste que compara o caminho novo (que para na página) com o antigo
(que lia a tabela inteira), e fui repor o defeito para provar que ele pega.

**Primeira sabotagem:** movi o `skip` para depois do laço. O teste **passou**.
Ela era falsa: o `continue` do filtro continuava acima, então o `skip` seguia
contando linhas visíveis. Eu tinha reescrito a mesma coisa com outra forma.

**Segunda sabotagem:** troquei para pular **entradas do índice** em vez de
linhas visíveis — o defeito de verdade, o que o comentário do método nomeia. O
teste **passou de novo**.

Aí parei de mexer no defeito e fui olhar o teste.

## O que eu concluí primeiro, e estava errado

Depois da primeira, concluí «a sabotagem foi mal escrita» — e estava certo.
Depois da segunda, ia concluir a mesma coisa. Errado: **o problema era o
teste.**

Ele criava as linhas excluídas com `excluir`, que nesta base é exclusão
**física**. Ela tira a entrada do `.ndx` junto — então `varrer_indice` já
devolvia só as sobreviventes, não havia linha invisível na lista, e filtrar
antes ou depois do `pular` produzia exatamente o mesmo resultado.

O teste não podia falhar. Ele conferia dois caminhos que, naquele cenário, são
o mesmo caminho.

## O que a medição disse

Trocando para `excluir_suave` — que mantém a linha e a entrada do índice, e só
marca —, o defeito reposto reprova na hora, nomeando o caso:

    visao Ativas, pular 3, limite 5
      left:  [4, 5, 7, 8, 10]
      right: [5, 7, 8, 10, 11]

## A regra

**Quando a sabotagem passa, o suspeito seguinte é o teste, e não a sabotagem.**
Uma vez é sabotagem mal escrita; duas é cenário que não distingue os caminhos.

E o critério prático: o cenário do teste precisa conter **a condição que separa
o certo do errado**. Aqui era «existir linha invisível dentro da lista do
índice» — e a exclusão física apagava justamente essa condição enquanto
parecia criá-la.

## Como está guardado hoje

O comentário do teste conta a história inteira, para ninguém «simplificar»
`excluir_suave` de volta para `excluir` — a troca parece inofensiva e apaga o
teste sem apagar uma linha dele.

É a terceira vez nesta sessão que uma prova quase passa por engano: a asserção
vazia da SP000006 (dois caminhos dando o mesmo número), a tabela de prova sem
coluna externa, e agora esta. As três têm a mesma forma — **o cenário não
continha o que faz os caminhos divergirem** — e valem como uma regra só.

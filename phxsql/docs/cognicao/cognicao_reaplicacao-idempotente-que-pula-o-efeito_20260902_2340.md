# A reaplicação idempotente que pula o efeito colateral

**Descoberto:** 02/09/2026, 23:40.
**Onde:** `phxsql-server/src/transacao.rs` (`aplicar_uma`),
`phxsql-store/src/table.rs` (`planejar_ao_alterar`).

## 1. O que aconteceu

A Frente A entregou o `ao_alterar: cascata` executando e deixou um risco
escrito: as escritas da cascata não entram no conjunto de escrita nem na marca
`.tx`, então uma queda no meio do `COMMIT` poderia deixar a filha para trás.

Fui medir o tamanho do buraco. A parte observável estava certa: pela sonda por
soquete, dentro da transação a cascata empilha, o `ROLLBACK` não deixa nada, e
o `COMMIT` executa. Nada vaza.

O buraco era outro, e estava na **recuperação**.

## 2. O que eu concluí primeiro, e estava errado

Li o `aplicar_uma` e escrevi, com todas as letras:

> «`Acao::Atualizar => t.atualizar(op.rowid, &op.linha)` — a recuperação
> reaplica pelo método que **carrega** a cascata. Isso sugere que ela conserta
> a filha sozinha.»

A dedução é confortável e é falsa, e ela é falsa por um motivo que vale mais
que o caso: **eu confundi «chama a função que faz X» com «faz X».** A função
faz X **condicionalmente**, e a condição é justamente a que a reaplicação
apaga.

A cascata é planejada pelo **delta da mãe** — `planejar_ao_alterar` compara
antes e depois e sai na primeira linha se nenhuma coluna indexada mudou. Na
reaplicação a mãe **já está no valor de destino**: `antes == depois`, plano
vazio, nada acontece.

E a segunda metade do erro é pior que a primeira: eu ia registrar isso como
«risco menor do que a Frente A relatou». Menor era a metade que eu tinha
medido; a que eu não tinha medido era **maior**.

## 3. O que a medição disse

Sonda em `phxsql-server/tests/cascata-na-recuperacao.rs`, com o estado
pós-queda produzido pela API pública — sem matar processo nenhum, sem injeção
de falha:

| | mãe | filha |
|---|---|---|
| antes | 1 | 1 |
| depois da queda simulada | **7** | 1 |
| depois de `recuperar()` | 7 | **1** |

E o relatório do arranque, na mesma rodada:

```
achadas 1 · descartadas 0 · completadas 1 · reaplicadas 1 · impossíveis 0
```

**Zero em `impossiveis`.** A recuperação devolvia `Ok`, contava a operação como
reaplicada, e o arranque imprimia que o commit tinha sido completado — com a
filha órfã no disco. Um relatório que mente é pior que um relatório que falta:
o que falta faz alguém procurar.

O controle, que é o que impede a sonda de provar nada: com a mãe **ainda no
valor velho**, a mesma marca leva a filha para 7. A sonda enxerga cascata.

**No caminho, um terceiro fato, de outra família.** Reproduzindo o estado
pós-queda achei que ele nasce **sem queda nenhuma**: a cascata só planeja um
nível. Com `avó ← mãe (cascata) ← neta (restringir)`, o plano da avó passa, a
avó vai para o disco, e só então a cascata chama `mae.atualizar`, que acha a
neta com `restringir` e recusa. O comentário do `aplicar_ao_alterar` afirmava o
contrário — «tudo o que pode recusar já recusou no planejamento, antes da
primeira escrita» — e o **recado do erro, duas linhas abaixo, já dizia a
verdade**. O código sabia mais que o próprio comentário.

## 4. A regra

**Reaplicação idempotente pelo alvo pula todo efeito colateral que dependa do
delta.** Quando a recuperação refizer uma operação chamando a operação normal,
pergunte o que dentro dela é **condicional** — e guarde na marca o que a
condição precisa para voltar a ser verdadeira.

E o corolário, que é o erro de leitura: **«chama a função que faz X» não é
«faz X»** enquanto não se souber sob que condição ela faz.

## 5. Como está guardado hoje

* **Marca `.tx` v2**: carrega a linha ANTIGA do `atualizar`. Custa zero leitura
  a mais — o servidor já lê a linha do disco ao empilhar, para a guarda do
  `softdeleted` — e vai no **fim** do payload, para o leitor da v1 e o da v2
  percorrerem os mesmos bytes até o motivo.
* **`Table::recascatear`**: replaneja com a linha antiga. É idempotente por
  construção (o plano procura a filha pela chave *antiga*, e cascata que já
  rodou não deixou filha ali), e por isso a chamada é incondicional em vez de
  tentar adivinhar onde a queda caiu.
* **A v1 continua sendo lida.** Marca é commit que já começou; descartá-la por
  uma mudança nossa de formato jogaria fora exatamente o que ela existe para
  salvar. Guarda: `a_marca_da_versao_anterior_continua_sendo_completada`.
* **Quatro provas e duas sabotagens** em `cascata-na-recuperacao.rs`: tirar o
  `recascatear` derruba só o teste da cascata; recusar a v1 derruba só o da
  compatibilidade. Cada sabotagem acerta um alvo, que é o que separa uma
  bateria de uma bateria que parece.

**Onde o buraco ficou:** a órfã de três níveis **continua acontecendo**. Ela é
pedido 169, e fechá-la pede planejar a árvore inteira antes da primeira escrita
— com duas perguntas de projeto abertas (o custo de abrir netas a cada
alteração de chave, e a parada do ciclo `A ← B ← A`). O comentário mentiroso
foi corrigido no mesmo trabalho: a documentação que mente ensina o errado, e
esta ensinava que havia uma garantia que não existe.

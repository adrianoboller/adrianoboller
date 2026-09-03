# A premissa do pedido já estava paga — e eu ainda errei o preço para cima

**Descoberto:** 03/09/2026, 02:10.
**Onde:** `phxsql-store/src/table.rs` (`planejar_ao_alterar`,
`conferir_a_arvore`); medidor `--example custo-da-cascata-em-arvore`.

## 1. O que aconteceu

O pedido 169 nasceu de um defeito real: a cascata do `ao_alterar` só planeja
**um nível**, e com `avó ← mãe (cascata) ← neta (restringir)` a recusa chega
depois de a avó estar gravada. Eu mesmo escrevi o pedido, e pus nele duas
perguntas de projeto como se fossem o preço de fechá-lo:

> «o custo (o planejamento passa a abrir netas e bisnetas a cada alteração de
> chave) e o ciclo (`A ← B ← A` precisa parar)»

As duas morreram medidas, e cada uma morreu de um jeito diferente.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e a segunda depois de já ter sido corrigido pela primeira.**

**Erro 1 — o custo da travessia.** Escrevi que conferir a árvore «passa a
abrir netas e bisnetas», tratando a travessia como despesa nova. Ela não é
nova: `aplicar_ao_alterar` grava a filha por um `atualizar` **inteiro**, e esse
`atualizar` planeja a própria cascata. A árvore já era percorrida — só que
**depois** da primeira escrita, que é exatamente o que fazia o defeito existir.
O custo que eu temia era o custo que o defeito já cobrava.

**Erro 2 — o preço do conserto.** Corrigido o primeiro, previ que a passada de
validação dobraria o custo («~2×», e cheguei a escrever isso no comentário do
código). Medido depois de escrita: **1,13× com um nível e ~1,35× de dois em
diante**. Errei para cima porque tratei as duas travessias como iguais, e elas
não são: a de validação só **planeja**, e planejar é a metade barata — quem
custa é gravar.

O padrão dos dois é o mesmo, e é o que vale guardar: **eu estimei um custo em
vez de medi-lo, dentro de um documento cuja regra é medir.** A primeira
estimativa quase matou o item; a segunda quase publicou um número errado sobre
o item já feito.

## 3. O que a medição disse

Custo de alterar a chave da avó, por profundidade da cascata (20 irmãs sem
chave no diretório, mediana de 40 voltas):

| níveis abaixo | antes (µs) | depois (µs) | conserto |
|---|---|---|---|
| 0 | 254,6 | 268,6 | dentro do ruído |
| 1 | 2.289,9 | 2.584,4 | 1,13× |
| 2 | 4.204,1 | 5.676,7 | 1,35× |
| 3 | 6.066,6 | 8.302,1 | 1,37× |
| 4 | 8.765,4 | 11.821,3 | 1,35× |

A coluna «antes» é a que mata a premissa: crescer **linearmente** com a
profundidade só é possível se cada nível já estivesse sendo aberto.

**E o ciclo não existe com linha dentro.** Sonda: um ciclo `A ← B ← A` com
conferência dos dois lados **não aceita a primeira linha** —
`inserir em aa -> Err("fk_bb: nao existe bb(par) com esse valor")` e
`inserir em bb -> Err("fk_aa: nao existe aa(par) com esse valor")`. Chave com
`verificar: false` sai do planejamento na primeira linha e nem cascateia. Não
há ciclo populado para detectar.

## 4. A regra

**Pergunta de projeto escrita num pedido é palpite até alguém medir —
inclusive quando quem escreveu foi eu.** A lista do que falta já era palpite
por regra desta casa; o que este arquivo acrescenta é que **o preço estimado
dentro do pedido é palpite do mesmo naipe**, e ele é mais perigoso porque
vem vestido de análise e costuma ser o que decide se o item entra ou não.

E o corolário sobre o conserto: **meça o preço DEPOIS de construir, não antes.**
Previsão que erra para cima adia trabalho barato.

## 5. Como está guardado hoje

* `Table::conferir_a_arvore` desce a corrente inteira antes da primeira
  escrita, e o `is_empty()` continua sendo o portão: alteração que não toca
  chave não paga nada.
* **Teto, e não detector de ciclo:** `TETO_DA_CASCATA = 16`, com o motivo
  escrito — recursão sem fundo num caminho de escrita é pilha estourada e banco
  pela metade; o teto promete trabalho limitado e recusa que diz onde parou. É
  o desenho do `PASSOS_MAX` dos gatilhos, pelo mesmo motivo. Ele **não**
  promete achar ciclo, e o comentário diz isso.
* O medidor `--example custo-da-cascata-em-arvore` guarda as duas colunas —
  antes e depois — no próprio texto que ele imprime, para o número não
  envelhecer numa conversa.
* Quatro provas em `cascata-na-recuperacao.rs` e **três** sabotagens, cada uma
  derrubando um alvo só.

**Onde o buraco ficou:** o `atualizar` da filha continua sendo o caminho de
gravação, então a árvore é percorrida **duas** vezes — 1,35× é o preço. A
travessia única existiria com um atalho de escrita sem cascata, e foi recusada
com o motivo: deixaria a filha com índice mentindo. Se um dia o perfil disser
que 1,35× dói, o caminho certo é o plano carregar as linhas, **não** o atalho.

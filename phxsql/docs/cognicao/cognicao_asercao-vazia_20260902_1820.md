# Asserção que passa pelos dois caminhos não prova nenhum dos dois

- **Quando:** 2026-09-02, 18:20 (SP000006, read-your-own-writes)
- **Onde:** `crates/phxsql-server/src/servidor.rs`, teste
  `a_transacao_enxerga_o_que_ela_mesma_escreveu`
- **Custo:** uma sabotagem que deveria reprovar **passou**

## O que aconteceu

Escrevi o teste do RYOW e sabotei três peças para provar que ele pega. Duas
sabotagens reprovaram. A terceira — desligar a contagem, fazendo-a voltar a ser
a do disco — **passou verde**.

Eu conferia `visiveis == 3` depois de uma exclusão suave. Naquele ponto o disco
tinha 3 linhas e a transação também tinha 3 (uma a mais inserida, uma
escondida). **Os dois caminhos davam o mesmo número**, então a asserção não
distinguia nada.

## O que eu concluí primeiro, e estava errado

Que conferir a contagem «depois de todas as operações» era o ponto mais forte,
porque era o estado mais completo. É o contrário: o estado mais completo é
justamente onde as diferenças têm mais chance de se cancelarem.

## O que a medição disse

Movida a conferência para logo após a inserção — disco **3**, transação
**4** —, a mesma sabotagem reprova. Nada mudou no código de produção.

## A regra

**Confira onde os dois caminhos DIVERGEM, não onde o estado está completo.**
Antes de escrever um `assert_eq!`, pergunte: que número o caminho errado daria
aqui? Se for o mesmo, a asserção é enfeite.

## Como está guardado hoje

O comentário do próprio teste carrega a história e o motivo do ponto escolhido,
para ninguém «simplificar» a conferência de volta para o fim. O catálogo de
mutação (`bancada/guardas/`) é o mecanismo geral, mas ele cobre 57 das ~1.495
guardas — este teste não estava entre elas quando o defeito aconteceu.

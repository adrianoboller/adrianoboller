# O `empilhar` decide pela linha da transação, e guarda a do disco

**Estado:** PENDENTE

## O que aconteceu

Pedidos 492 e 515, medidos pelo servidor antes do conserto:

- `[excluir suave M, atualizar M.nome]` confirmava com M **viva** dentro da
  transação e excluída fora (`("Cid II", false)`); a linha nascida na própria
  transação também (`("Dan II", false)`), e o upsert com e sem SET idem.
- `[filha id 10→11, mãe 5→6]` terminava com id 10; `[filha troca 15→8, mãe
  15→16]` na 16; `[excluir suave a filha, mãe 25→26]` ressuscitava a filha —
  `[(10, 6, false), (20, 16, false), (30, 26, false)]` contra
  `[(11, 6, false), (20, 8, false), (30, 26, true)]`.

As duas famílias têm a mesma raiz: o `empilhar` abre a tabela **sem** a
sobreposição (dispensa medida: 60 → 625,62 µs/op com a lista), e decisões que
dependem da linha **atual** — a marca herdada, a base da mescla do SET, a linha
de onde o plano da cascata parte, a filha que o plano abre — liam o disco.

## O que eu concluí primeiro, e estava errado

Que o defeito do 492 era só do caminho da transação. Medido o controle «fora»
pelo upsert, ele também ressuscitava (`("Bia II", false)`): a pergunta «o
pedido mandou a coluna de sistema?» estava escrita **três** vezes (uma sem o
caso da lista) e **faltava** no `upsert::aplicar` solto. O 492 era a quarta
cópia da regra divergindo das outras, não um defeito da transação.

## O que a medição disse

- Separar o que é do DISCO do que é da TRANSAÇÃO: a existência da linha e a
  `linha_antiga` da marca continuam do disco (a recuperação precisa do valor
  de antes, e `o_empilhar_le_o_disco_e_nao_a_lista_pendente` trava isso); a
  marca, a mescla e o plano partem de `linha_na_transacao`, que dobra **só as
  escritas daquela linha** pela mesma `sobrepor_mais` da leitura — O(escritas
  da linha), sem montar a sobreposição inteira.
- O plano do `empilhar` ganhou o prefixo das filhas por um resolvedor
  preguiçoso (`PrefixoDaSessao`): só monta quando o plano abre filha, que só
  acontece depois do portão `alguma_coluna_indexada_mudou`.
- A regra da marca virou uma função (`valores::herda_a_marca`) e uma ação
  (`herdar_a_marca`) para os quatro caminhos.
- O irmão do 491 no catálogo apareceu pela mesma pergunta («quem mais pula a
  própria tabela?»): o `renomear_tabela` deixava a auto-referência no nome
  velho, e o `excluir_de_vez` do chefe com subordinado na tabela renomeada
  respondia `Ok(true)`.

## A regra

Quando um caminho lê o disco de propósito, liste as decisões que ele toma com a
linha lida e separe as que são do disco das que são de quem escreve — e procure
a mesma pergunta nos caminhos FORA da transação antes de chamá-la de defeito da
transação.

## Como está guardado hoje

Guardas `marca-do-disco-no-empilhar`, `upsert-solto-ressuscita-a-excluida`,
`mescla-do-upsert-sobre-o-disco`, `elo-do-empilhar-pelo-disco` e
`renomear-pula-a-auto-referencia`. **Onde o buraco ficou:** o OLD do gatilho
BEFORE UPDATE no `empilhar` continua sendo a linha do disco — achado por
leitura, não medido, e não mudado (é outra decisão: o que o gatilho vê).

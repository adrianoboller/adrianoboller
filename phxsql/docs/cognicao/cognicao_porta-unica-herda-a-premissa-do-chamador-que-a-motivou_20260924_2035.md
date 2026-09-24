# A porta única herda a premissa do chamador que a motivou — e o terceiro irmão chegava com o punho sujo

**Estado:** PENDENTE

## O que aconteceu

Pedido 540 (24/09/2026): a alteração solta que cascateia virou uma transação
de uma instrução, e os três chamadores — `op_atualizar`, o upsert do
`op_inserir` e a sincronia do DbLink — passaram pela mesma porta,
`Servidor::alterar_solto`. A frente fez o que a lei do irmão manda: achou os
três pela função que chamavam e levou os três para a porta. O comentário da
porta dizia «o velho não escreveu nada», e isso era verdade para **dois** dos
três.

O papel C mediu o terceiro (parecer `docs/propostas/parecer-dba-integridade-2-2026-09-24.md`,
Q1): a sincronia **insere pelo mesmo punho `t` antes** de alterar a mãe. A
passada abria um segundo punho da mãe com o `t` ainda sujo, achava o byte 52
em 1 sem atestado e recusava; o `completar_marca` reconstruía o índice pelo
`.reg`; e o `Drop` do `t` velho, na troca `*t = abrir_travada(..)`, gravava a
árvore VELHA por cima. `buscar` pela chave nova dava 0, o código único entrava
repetido e a órfã passava.

## O que eu concluí primeiro, e estava errado

1. Que a forma mais limpa do conserto era a primeira do parecer: **a passada
   recebe o `t` já aberto**, e deixa de existir segundo descritor. Lendo o
   braço `Completada` da `passada_sob_a_marca`, não basta: quando a passada
   quebra depois da escrita 1, o `completar_marca` abre o punho **dele** na
   mesma mãe — e o `t` emprestado estaria sujo da escrita da própria passada.
   É o mesmo defeito num braço mais raro. A descida antes da marca é
   necessária nas duas formas; a primeira só acrescentaria mexer na passada
   que o `COMMIT` também usa.
2. Que «irmão é quem chama as mesmas funções na mesma ordem» já cobria o
   caso. Pela função chamada (`alterar_solto`), os três eram irmãos; pela
   **ordem**, não: a sincronia chama `inserir` e **depois** `alterar_solto`
   no mesmo punho. A frente aplicou a lei pela função e leu a ordem só dos
   dois que motivaram a porta.

## O que a medição disse

- Teste `testes_transacoes::integridade_na_transacao::a_cascata_solta_depois_de_escrever_no_mesmo_punho_nao_perde_o_indice_da_mae`
  (o laço da sincronia, `aplicar_para_ca` com uma closure igual à dela): sem a
  descida, **5 de 5** vermelhas com `buscar por_codigo 6` = 0 e «indices
  reconstruidos 1» no bloco da recuperação; com ela, verde 5 de 5, com a
  duplicata e a órfã recusadas.
- A forma recusada pouparia uma abertura da mãe: `Table::abrir`, mediana
  **40 µs** (39–202, 3 × 300).
- Custo da descida na `op_atualizar` (sonda `custo` da frente do 540, três
  rodadas intercaladas): `por_lote` com duas filhas, mediana 1.084–1.174 µs
  antes e 1.125–1.177 depois — faixas cruzadas, empate; `write` na corrida
  inteira, 18.332 e 18.332 (`strace -c`; a segunda dupla, 18.172 e 18.278, dá
  a variação da janela por tempo).

## A regra

Quando vários chamadores passam a entrar por uma porta única, confira a
premissa da porta contra o **estado com que cada um chega** — punho sujo,
trava na mão, transação aberta —, e não só contra a função que eles chamam.

## Como está guardado hoje

- A descida (`Table::descer_ao_nucleo`) no `alterar_solto`, antes da marca.
- A guarda `cascata-solta-com-o-punho-de-quem-chama-sujo` no
  `bancada/guardas/catalogo.py`, com o teste acima em `caem`.
- **Buraco que fica:** nada confere, de forma geral, que um punho de tabela
  esteja limpo quando outro punho da mesma tabela abre no mesmo processo. A
  guarda cobre este chamador; o próximo que abrir um segundo punho com o
  primeiro vivo repete o defeito sem acusar.

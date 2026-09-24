# A prova escrita contra o defeito de hoje passou com a alternativa que o parecer reprovou

**Estado:** PENDENTE

## O que aconteceu

Pedido 451: um pânico com a trava global de dados na mão a deixava envenenada
para sempre (a H1). O DBA decidiu a H4 — o reparo no `Drop` do `TravaMedida`,
que é o arranque rodado na hora — e **reprovou** a H2 ingênua: recuperar a
trava (`into_inner`) sem reparar nada.

A primeira versão da prova fora de transação
(`servidor::testes_do_panico_sob_a_trava::panico_no_meio_do_inserir_nao_fecha_a_base_e_nao_deixa_fantasma`)
cobrava quatro coisas pelo soquete: OUTRA conexão atende, o `.reg` guarda o
dado de antes, o índice recusa nomeando-se até o `reindexar`, e depois dele
cada linha viva está no índice uma vez. Com a H1 reposta ela caiu, como devia.

Com a **H2 ingênua** reposta (o `Drop` sem o reparo e as duas portas da trava
recuperando sempre), ela **passou**.

## O que eu concluí primeiro, e estava errado

Que «falhar com o defeito reposto» queria dizer «falhar com o comportamento de
hoje». O pedido dizia isso com todas as letras, e o teste cumpria. Mas o
defeito de hoje é só UM dos estados para onde o código pode voltar: o próximo
refatoramento não volta à H1, volta à alternativa mais barata que alguém achar
— e a mais barata aqui é exatamente a H2, uma linha de `into_inner`, a mesma
receita que a `TravaDaGuarda` do cluster usa com razão.

E o motivo de a H2 passar não era defeito do teste, era mérito de outro
pedido: o 456 já faz o `.ndx` interrompido não descer e o byte 52 subir antes
do `.reg`. **O disco já diz a verdade sozinho**; o que a H2 deixa errado mora
FORA do disco.

## O que a medição disse

Medido repondo a H2 ingênua (`if true || …` no `depois_do_veneno`, e o `Drop`
sem `reparar_a_trava`), 24/09/2026:

- a prova fora de transação SEM a conferência da cópia residente: **passou**
  (`1 passed`);
- a mesma prova COM a conferência do residente: **caiu** —
  `a copia residente continuou servindo depois do panico, com o .reg em
  [1, 2, 3, 4, 5, 6]: … "achadas":5 …`;
- a prova do `COMMIT` caiu — OUTRA conexão viu `[1, 2, 3, 4, 5, 10]` em vez de
  `[1, 2, 3, 4, 5, 10, 11, 12]`: a marca órfã não se completou e o `AoSair`
  soltou as travas da transação;
- a prova do piso (H5) caiu — o processo seguiu de pé servindo.

Os dois estragos que distinguem a H2 da H4 são os que não estão no arquivo de
dado: a cópia em RAM anotada depois do disco, e a transação confirmada cuja
marca espera um reinício enquanto as travas dela já foram soltas.

E a mesma armadilha pelo outro lado, na revisão do DBA (M1, 24/09/2026): a
primeira reposição de «o reparo varre todas as marcas» trocou a completação
pela varredura **dentro** do ramo da marca em voo — que num `inserir` solto nem
roda. A prova `o_reparo_completa_so_a_marca_em_voo` passou com o «defeito» de
pé, e o motivo era a reposição, não o teste. Reposta como o defeito de verdade
(a varredura incondicional), ela caiu: «o cliente 1 voltou para "velho"».

## A regra

Reponha também a alternativa que o parecer **reprovou**, e não só o defeito de
hoje: a prova que não distingue o conserto da alternativa reprovada não
protege a decisão, só o conserto. E quando a prova passar com o defeito de pé,
confira a reposição antes de confiar no teste: ela pode ter posto o defeito
num caminho que o teste não percorre.

## Como está guardado hoje

- A conferência da cópia residente entrou na prova fora de transação, e a
  guarda `reparo-da-trava-deixa-o-residente` do catálogo a repõe.
- A própria H2 ingênua virou entrada, `trava-de-dados-recupera-sem-reparar`,
  com `trocas` nos dois pontos distantes do `servidor.rs` — as duas PROVADAS
  pelo `provar-guardas.py` em 24/09/2026 (3/3 e 1/1).
- As três reposições (H1, H2 ingênua, H4 sem H5) estão tabeladas na §24.4 do
  `docs/SEGURANCA.md`, com os vermelhos.
- **O buraco:** nada obriga a próxima frente a repor a alternativa
  reprovada. O catálogo guarda o que alguém lembrou de escrever; o parecer
  que reprova uma hipótese não vira entrada sozinho.

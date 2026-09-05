# Medir NA TELA foi o que decidiu o pedido 188 — e quase decidiu ao contrário

*Descoberto em 05/09/2026, 07:30, cumprindo a ordem do dono de medir o custo da
grade ordenada na tela **antes** de escrever qualquer linha de conserto.*

## 1. O que aconteceu

O pedido 188 chegou com o motor todo medido: `Table::pagina_por_indice`
começava com `let todos = self.varrer_indice(indice)?`, que percorre o `.ndx`
**inteiro** e devolve todos os rowids antes de qualquer recorte — o `break` do
limite parava a leitura das **linhas** do `.reg`, nunca a varredura do índice.
Por isso 50 linhas custavam o mesmo que 1.000, e uma grade ordenada tocava
1.668 páginas do `.ndx` (6,52 MiB, 81% do teto do cache) onde uma sem ordem
toca zero.

A **decisão do dono** foi que esse número não decidia nada: ele decide se o
custo **aparece para quem está olhando a tela**. Se não aparecesse, o pedido
viraria documentação — e isso seria resultado, não fracasso.

Apareceu, e a diferença entre as duas medições é o assunto deste arquivo.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que a razão entre os dois caminhos era o número que decidia**, e ela
é enorme: pelo fio, a um milhão de linhas, a grade sem ordem custa 0,58 ms e a
ordenada custava 54,81 ms — **78×**. Comecei a escrever o relatório com esse
78× como manchete.

Está errado por duas vezes, e as duas por motivos diferentes:

**Primeiro, o 78× não mede um defeito.** As duas devolvem 50 linhas e **não é o
mesmo trabalho**: uma devolve as 50 primeiras na ordem de digitação, a outra as
50 primeiras na ordem da chave. Essa razão mede o **preço de pedir ordem** — e
pedir ordem custa mesmo alguma coisa. O que diz se há defeito é a comparação
contra o **mínimo que a mesma pergunta exige**: descer a árvore uma vez e ler as
50 primeiras entradas da folha. Essa razão era **484×**, e é ela que condena.

**Segundo, e é o que a ordem do dono pegou: nenhuma das duas razões diz o que a
pessoa sente.** Fui medir na tela achando que ia ver o 78× de novo, mais
diluído. Não vi nem uma coisa nem outra: a tela tem um **piso próprio de 48 ms**
— o `fetch`, o JSON e o DOM de 200 linhas — que conserto de motor nenhum remove.
A dez mil linhas o custo do defeito **desaparece dentro do piso**; a cem mil ele
mal se distingue; e é só a um milhão que ele **dobra a espera**.

Ou seja: **os dois números que eu tinha na mão diziam «gravíssimo em qualquer
escala», e a tela disse «invisível em duas escalas de três, e visível na
terceira».** A conclusão final foi a mesma — consertar —, mas por um motivo que
eu não tinha, e com um alcance que eu teria errado.

## 3. O que a medição disse

Pelo fio, página de 50, mediana de 9 rodadas intercaladas, máquina livre pelo
`bancada/esta-medindo.sh` (`--example o-que-a-grade-ordenada-custa`):

| linhas | ordenada antes | depois | páginas do `.ndx` antes | depois |
|---:|---:|---:|---:|---:|
| 10.000 | 1,13 ms | 0,55 ms | 84 | **2** |
| 100.000 | 6,14 ms | 0,56 ms | 835 | **3** |
| 1.000.000 | **54,81 ms** | **0,57 ms** | **8.335** | **3** |

Na tela, num navegador de verdade contra o `phxsqld` de verdade, página de 200,
mediana de 7 trocas intercaladas (`testes-web/grade/custo-da-ordem.mjs`):

| linhas | sem ordem | ORDENADA antes | ORDENADA depois |
|---:|---:|---:|---:|
| 10.000 | 48,2 ms | 77,1 ms | 48,2 ms |
| 100.000 | 48,3 ms | 64,8 ms | 48,3 ms |
| 1.000.000 | 48,2 ms | **98,0 ms** | 48,3 ms |

E o custo cresce com a **tabela**, não com a página: a dez milhões — que é a
escala que a bancada desta casa já carrega — seriam ~500 ms por clique de
cabeçalho.

## 4. A regra

**Custo de motor se traduz para a tela pelo PISO da tela, e não pela razão.** A
razão entre dois caminhos do motor responde «quanto um trabalha mais que o
outro»; quem decide se há frente é a diferença **absoluta** comparada ao que a
tela já gasta sozinha. Um ganho de 78× dentro de um piso de 48 ms não é ganho
nenhum, e um ganho de 2× que sai de 98 ms para 48 ms é a metade da espera de
uma pessoa.

E o corolário, que é a regra 4 da bancada dita para dois lados nossos: **quando
os dois lados da comparação são do próprio motor, o «mesmo trabalho» ainda
precisa de nome.** Sem a *ordenada mínima* na tabela, o 78× teria virado
manchete e o 484× — que é o número que condena — não existiria.

## 5. Como está guardado hoje

- **O conserto**: `Ndx::varrer_apos` (varredura em pedaços, cursor pela chave
  completa) e `Table::pagina_por_indice` reescrito em cima dele.
- **As duas bancadas**, e as duas ficam:
  `crates/phxsql-server/examples/o-que-a-grade-ordenada-custa.rs` (o motor e o
  fio, com a *ordenada mínima* como régua de trabalho igual) e
  `testes-web/grade/custo-da-ordem.mjs` (a tela). Cada uma **imprime o crivo
  junto do número**, para ninguém copiar só a razão.
- **A prova real, medindo o efeito**: `Table::paginas_do_indice_tocadas()` conta
  acerto de cache **mais** falta — contar só as faltas mediria a RAM e daria
  zero na segunda corrida. Três testes em
  `crates/phxsql-store/tests/paginacao.rs`, com comparações **relativas**
  tiradas da própria tabela, e duas guardas novas no
  `bancada/guardas/catalogo.py`, as duas **PROVADAS**.
- **A meia-verdade corrigida** no `op_varrer` do `servidor.rs` e a tabela do
  `docs/CONCORRENCIA.md` §16.7, que publicava as 1.668 páginas.
- **O buraco que fica nomeado**: a bancada da tela mede o gesto de **trocar a
  ordem** com a página já aberta. Ela **não** mede a primeira abertura da aba,
  nem a rolagem para a página 2 (`pular` grande), que é o caso em que o pedaço
  pedido vale `pular + limite` e volta a crescer com a posição. A um `pular` de
  meio milhão o custo é o de antes — e ninguém mediu se alguém rola até lá.
- **Um segundo buraco, e ele é de método**: a primeira corrida do medidor de
  cache depois do conserto imprimiu o número de **antes**, e o binário estava
  velho. O experimento controlado diz que `cargo build --release --examples -p
  phxsql-store` **recompila** depois de uma mudança na lib — então a receita da
  casa não falhou, e a causa daquela corrida específica **não está medida**.
  Fica registrada assim, sem mecanismo inventado, no `docs/DESEMPENHO.md` §19.8.

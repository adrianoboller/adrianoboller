# O pior caso de um orçamento não é o caso que o medidor mediu

- **Quando:** 2026-09-03, 09:42
- **Onde:** `phxsql-sql/src/rotina.rs` (`PASSOS_MAX`, `TEXTO_MAX`),
  `phxsql-server/src/servidor.rs` (`PRAZO_DO_GATILHO_ANTES`),
  `docs/CONCORRENCIA.md` §1.4
- **Custo:** zero em retrabalho, e o achado é uma queda do servidor inteiro que
  estava viva e documentada como resolvida havia doze horas

## O que aconteceu

A cognição de ontem — `teto-em-passos-nao-e-teto-em-tempo_20260902_2105` —
fechou com o número certo e a conclusão errada. Ela pegou o `PASSOS_MAX` do
avaliador, converteu um milhão de passos para **18,3 ms** com um medidor, e
escreveu no `CONCORRENCIA.md`:

> **18,3 ms é o pior que um gatilho consegue impor a todas as outras
> conexões** […] **Consequência para a ordem de trabalho:** estas cinco deixam
> de ser o primeiro alvo.

O medidor rodava `WHILE TRUE DO SET x = x + 1`. Um passo de aritmética.

Este corpo é legal em qualquer gatilho `BEFORE` desta casa:

```sql
WHILE TRUE DO SET NEW.x = CONCAT(NEW.x, NEW.x); END WHILE
```

`CONCAT(s, s)` **dobra** o texto a cada volta. Trinta passos de um orçamento de
um milhão chegam a um gigabyte. O corpo não morre no teto — morre no alocador,
e em Rust alocação que falha **aborta o processo**. Com a trava global de dados
na mão, e levando junto todas as conexões do servidor.

## O que eu concluí primeiro, e estava errado

**Concluí que a cognição de ontem já tinha resolvido isto e que eu só precisava
confirmar o número.** A tarefa até dizia «o interpretador já tem um `PASSOS_MAX`
(custa 18,3 ms no pior caso, medido) — veja se ele cobre este caminho», e a
resposta óbvia, lendo o comentário do `PASSOS_MAX`, é *sim, cobre: o comentário
nomeia exatamente esta razão*.

O erro não foi acreditar no número. O número está certo — refeito hoje, dá
27,2 ms. O erro foi acreditar que **aquele** número era o pior caso, quando o
medidor tinha escolhido um corpo cujo passo é barato. Um orçamento de N passos
vale `N × (custo do passo mais caro)`, e ninguém tinha perguntado qual é o passo
mais caro que a linguagem oferece. Ela oferece um sem fundo.

E a forma do erro é a mesma de ontem, um andar acima: ontem foi *«teto em
passos não é teto em tempo»*; hoje é *«teto em tempo medido no caso barato não
é teto em tempo»*. A cognição de ontem existe para impedir exatamente isto, e
não impediu — porque ela mandou converter a unidade, e não escolher a entrada.

## O que a medição disse

`cargo run --release --example custo-do-gatilho -p phxsql-sql`, e a linha do
aborto com `ulimit -v 2000000` para não derrubar a máquina:

| corpo | onde ele morre | com a trava na mão por |
|---|---|---:|
| `WHILE TRUE DO SET x = x + 1` | teto de passos | **27,2 ms** |
| `IF x < 10 THEN SET x = x + 1` | termina sozinho | **1 µs** |
| `WHILE TRUE DO SET s = CONCAT(s, s)` | **no alocador** — `memory allocation of 536870912 bytes failed`, e o processo **aborta** | **10,2 s**, e então o servidor cai |
| o mesmo, com `TEXTO_MAX` | teto de texto, aos **0,0163%** do orçamento de passos | 905,9 ms |
| `WHILE TRUE DO SET s = CONCAT('x', s)` sobre 512 KiB | teto de passos | **28.590 ms** |
| o mesmo, com prazo de parede | o prazo | **500,2 ms** — 57× menos |

Três tetos, e cada um pega o que os outros não veem: `PASSOS_MAX` limita o
corpo barato, `TEXTO_MAX` limita **um** passo, e só o prazo de parede limita a
**trava**.

E a prova real ao contrário, que é a parte que dá o direito de escrever isto:
com o teto de texto reposto, o teste **não falha — aborta o `cargo test`** em
13,9 s. Com o prazo removido, o mesmo corpo roda **79,8 s** em vez de 50 ms.

## A regra

**Um orçamento em unidade interna vale o pior passo, não o passo que o medidor
escolheu.** Antes de citar «N passos custam T», pergunte qual é a operação mais
cara que um passo pode ser — e se a resposta for «depende do dado», o orçamento
não é um teto: é uma média com cara de garantia.

E o corolário operacional: **quando um teto novo entra, o teste que mais
importa é o do corpo que os tetos antigos já pegavam.** Foi ele que mostrou
que o teste de ponta a ponta continuava passando com o prazo removido — o teto
de texto segurava aquele corpo, e o furo real ficava invisível.

## Como está guardado hoje

* `TEXTO_MAX` e o prazo de parede no código, com o motivo escrito no lugar para
  onde o próximo leitor vai olhar;
* quatro testes no `rotina.rs`, e os dois lados do laço: com prazo e **sem**
  prazo, porque o procedimento roda sem a trava e não pode herdar o relógio;
* a catraca `o_before_roda_com_prazo_e_o_after_nao`, estática, lendo o próprio
  fonte;
* a guarda `before-sem-prazo-de-parede` no catálogo, **PROVADA** (1/1 caiu);
* o `custo-do-gatilho.rs` passou a medir os três casos, e não só o barato.

**Onde o buraco ficou, escrito:** a guarda do `TEXTO_MAX` **não** entra no
catálogo automático. Repô-lo faz o binário alocar até o alocador falhar — 8 a
16 GiB nesta máquina, com risco de o kernel matar o processo de outra frente.
Ela fica manual, com a receita `ulimit -v 2000000` no comentário do teste e no
`CONCORRENCIA.md`. Guarda que derruba o trabalho do vizinho é a mesma falha do
zelador que apaga o `target` de quem está compilando.

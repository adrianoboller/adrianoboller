# Com frentes paralelas, número digitado envelhece em uma hora

- **Quando:** 2026-09-02, 19:35 (integração das cinco frentes)
- **Onde:** `docs/QA-PDCA.md` contra
  `crates/phxsql-server/src/conferidor.rs`
- **Custo:** um documento de QA publicando a catraca errada no dia em que
  nasceu

## O que aconteceu

A frente de QA levantou o catálogo de guardas e escreveu: `TETO` = **1.577**,
medido 1.577, «em cima, sem folga». Correto **no instante em que ela mediu**.

Na mesma rodada, a frente de idiomas traduziu 28 textos e baixou a catraca para
**1.549** — como manda a regra, no mesmo trabalho.

Nenhuma das duas podia ver a outra. O documento nasceu errado.

## O que eu concluí primeiro, e estava errado

Que a doutrina «todo número visível sai de um gerador» era sobre números que
envelhecem **entre versões** — o selo da capa que passou quatro lançamentos
dizendo 0.11.0. Ela é sobre algo mais rápido: com frentes paralelas, o número
envelhece **dentro da mesma rodada**.

A mesma frente entregou a prova sem perceber: mediu o total de testes duas
vezes com uma hora de diferença e obteve **1.485** e depois **1.495**.

## O que a medição disse

1.577 → 1.549 em ~90 minutos. E 1.485 → 1.495 → 1.494 (a minha, na árvore
integrada) para o mesmo total de testes, três medições no mesmo dia.

## A regra

**Trabalho paralelo transforma «número digitado envelhece» em «número digitado
já está errado».** Documento que sai de rodada com mais de uma frente ou traz
gerador, ou traz o comando ao lado do número e a hora da medição.

## Como está guardado hoje

Corrigido à mão na integração, com a nota ao lado explicando por que a linha
precisa de gerador. **O gerador ainda não existe** — é o item que a frente de
QA deve receber de volta, e agora contra a árvore integrada, não contra uma que
ainda se mexe.

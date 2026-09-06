# Extrapolação é número citado — e a minha errou por 3×, para o lado que aprovava

**06/09/2026, 21:30.** Descoberto medindo o custo do `.fts` antes de escrevê-lo.

## 1. O que aconteceu

O `docs/FTS.md` §4.2(a) nomeou o risco que podia matar o desenho: cada
`inserir` passaria a escrever ~14 chaves a mais, e 83,5% da inserção já é
`.ndx`. Escrevi o medidor `custo-da-chave-a-mais` com tabelas de **1, 2, 4 e 8
índices**, tirei o custo marginal do último trecho e **multipliquei por 14**.

Deu **2,95×**, e o medidor imprimiu, com todas as letras:

> `VEREDITO: cabe. A escrita sincrona do .fts segue como esta no FTS.md.`

Medi o ponto de **15 índices direto**. São **9,05×**.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o custo marginal de uma chave era aproximadamente constante, e que
por isso a reta valia. Era plausível: uma árvore B+ a mais é uma árvore B+ a
mais, e as três primeiras medidas ficaram todas entre 0,59 e 0,93 µs.

Estava errado, **e os meus próprios dados já diziam**: 0,59 → 0,79 → 0,93 é
uma sequência que **sobe**. Reta que sobe não é reta, e eu ajustei uma reta a
ela mesmo assim, usando o último ponto como se fosse a inclinação de todos.

Medido, não é nem reta nem curva suave: é um **penhasco**. O custo por chave é
plano até 8 índices (0,72–1,10 µs) e vira **5,42 µs** em 15.

## 3. O que a medição disse

| índices | µs por inserção | µs por chave a mais |
|---:|---:|---:|
| 1 | 5,458 | — |
| 2 | 6,298 | 0,840 |
| 4 | 8,508 | 1,105 |
| 8 | 11,401 | 0,723 |
| **15** | **49,366** | **5,424** |
| 17 | 61,556 | 6,095 |

**O erro foi de 3×, e na direção que aprovava o desenho que eu tinha acabado
de escrever.** Não é coincidência que dê para o lado confortável: eu escolhi o
método de extrapolação depois de conhecer o desenho que queria validar.

E o que o número **não** diz continua não dito: a causa do penhasco é
hipótese — provavelmente as páginas quentes de 15 árvores deixando de caber no
cache. Efeito medido, causa nomeada.

## 4. A regra

**Extrapolação é número citado, e número citado é número que não se mede.** O
ponto que decide um desenho se **mede**, nunca se estima — inclusive quando a
reta parece boa, inclusive quando os pontos medidos são muitos, e sobretudo
quando quem cita é o próprio medidor.

## 5. Como está guardado hoje

- O medidor mede **15 e 17 índices direto**, sem extrapolar nada, e imprime a
  inclinação de cada trecho ao lado — para o penhasco aparecer em vez de sumir
  numa média.
- O comentário do `const CASOS` diz por que o 15 está lá, e cita o erro: quem
  encolher a lista de volta para 8 lê o motivo antes.
- `DESEMPENHO.md` §21 traz a tabela e a §21.1 traz o erro, porque hipótese que
  morre medida é resultado tão válido quanto ganho.
- `FTS.md` §4.1 e §4.3: o desenho **mudou** de escrita síncrona para despejo
  em lote, por causa deste número, antes de existir uma linha do índice.
- **Sem guarda automática, e é decisão:** um conferidor que proibisse a palavra
  «extrapolar» acharia as dezenas de extrapolações legítimas deste repositório
  — toda projeção de volume é uma. O que separa esta das outras é que ela
  **decidia um desenho**, e isso não se acha por padrão de texto. O que fica no
  lugar é a regra da §4, e o medidor que já não extrapola.

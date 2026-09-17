# Catraca que nasce em zero não distingue régua morta de árvore limpa

**Descoberto em 17/09/2026, 00:38 UTC.** Frente da régua do `Debug` com
segredo (papel B com o chapéu do G), construindo o conferidor que a cognição
`guarda-trava-a-struct-nao-a-lei` pediu.

## 1. O que aconteceu

A régua nova, `bancada/guardas/debug-com-segredo.py`, nasce no número medido
do dia: **0** — as nove structs já estavam consertadas em `74de67e`. A prova
real nos dois sentidos passou: com o `conexao.rs` de antes do conserto ela
acusa `Receita.token` e `Receita.senha` (SUBIU 2); com a troca da guarda
`debug-da-ligacao-mostra-a-senha` aplicada por texto, acusa `Definicao.senha`
e `Definicao.token` pelo caminho do `impl` à mão (SUBIU 2); na árvore limpa,
0.

Depois vieram as **seis mutações** da própria régua — cada parte do crivo
desligada numa cópia, com a régua mutada rodando sobre a árvore limpa:

| mutação | o que morreu | `medido` na árvore limpa |
|---|---|---:|
| m1 | crivo do nome (`nome_de_segredo` → sempre verdadeiro) | 370 |
| m2 | crivo do tipo (`portador` → sempre verdadeiro) | 17 |
| m3 | a lista de isenções (`isencao` → nenhuma) | 9 |
| m6 | o lexer (`despir` → identidade) | 5 |
| **m4** | **a detecção do `derive(Debug)`** | **0** |
| **m5** | **a leitura do `impl` à mão** | **0** |

Quatro mutações mudam o número e qualquer inventário as denunciaria. **Duas
não mudam nada**: sem detectar o `derive` ou sem ler o `impl`, a régua mede
zero — e zero é exatamente o que a árvore sã mede. O `--numeros`, que o
`docs/qa/medir.py` lê, respondia `medido=0` nas duas, e a tabela publicaria
«0 — em cima, sem folga» de uma régua que não via mais nada.

## 2. O que eu concluí primeiro, e estava errado

**Escrevi o autoteste só dentro do `--catraca`.** O raciocínio era plausível:
o `--catraca` é o veredito (a bateria o chama), e o `--numeros` é «só leitura
de número» para o inventário. A mutação m4 mostrou o furo: o inventário não lê
um número, lê um **veredito com cara de número** — e o zero da régua morta
entra nele sem ninguém perceber, porque não há diferença visível entre «0
porque não há defeito» e «0 porque a régua parou de olhar».

E um segundo erro, menor e do mesmo naipe: **copiei o léxico da varredura
temporária «verbatim» achando que copiar era mais seguro que pensar.** O
briefing mandava não inventar, e eu li isso como «não toque». O autoteste
disse que `credencial` não casa `credenciais` (o plural em `-ais` escapa do
prefixo) e a leitura disse que o negador `_hex` estava **errado** — chave em
hexadecimal continua sendo a chave. A cópia também era número citado.

## 3. O que a medição disse

- Árvore limpa: **0**. Defeito reposto pelo `derive`: **2**. Pela troca da
  guarda (`impl` que lê o campo): **2**. Autoteste: **26** casos, 0,03 s.
- Seis mutações: **6 de 6** derrubam o autoteste; **4 de 6** mudam o número
  medido; **2 de 6 (m4, m5) são invisíveis ao número** — só o autoteste as
  distingue.
- O crivo de tipo alargado (listas e mapas de `String`) custou **2** isenções
  a mais que as cinco do briefing (`PapelDeChave.chaves_estrangeiras`,
  `Transacao.chaves`), as duas lidas no fonte; comprou um `tokens:
  Vec<String>` futuro. O sexto negador de nome que a varredura tinha (`_hex`)
  saiu por estar errado; os outros quatro não casavam campo nenhum.
- Das **5** réguas Python de `bancada/` que se descrevem ao inventário, **4**
  têm `--autoteste`; **1** o roda dentro do `--catraca`
  (`mapa-das-threads.py`); **0** o rodavam dentro do `--numeros` antes desta.

## 4. A regra

**Régua que nasce em zero prova que está viva em toda resposta que devolve
número — o autoteste corre dentro do `--numeros` e do `--catraca`, não num
modo à parte que só roda quem lembra.**

## 5. Como está guardado hoje — e onde o buraco ficou

**Guardado:** o `debug-com-segredo.py` roda os 26 casos antes de responder ao
`--catraca` **e** ao `--numeros`; com qualquer das seis partes mutada, o
`--catraca` reprova com «AUTOTESTE FALHOU» e o `--numeros` sai com código 1 —
que o `medir.py` publica como «não rodou», que é a verdade. A bateria (item
0c) chama o `--catraca`; o inventário acha o script sozinho pelo
`catraca:nome=`.

**O buraco, nomeado:** a mutação foi manual, em seis cópias no scratchpad.
Não há provador de mutação roteirizado para régua em Python — o
`provar-guardas.py` repõe defeito em **Rust** e não alcança `bancada/`. E das
outras quatro réguas Python, três têm autoteste que **nenhum chamador roda**
(`mapa-da-trava.py`, `trecho-vivo.py` não o chamam no `--catraca` nem no
`--numeros`; `mapa-das-threads.py` só no `--catraca`): para elas, uma mutação
como a m4 entra no inventário calada hoje. Fica dito em vez de escondido; o
conserto é de uma linha em cada uma, e não é desta frente.

# O portão de *run-and-diff* não pega o gerador que imprime-e-cola

**Descoberta:** 11/09/2026, fechando a rodada com o time ativo.

## 1. O que aconteceu

A revisão de gaps mandou «fechar a rodada»: re-rodar os geradores do dossiê e
**amarrá-los a um portão** para não envelhecerem calados. A Frente H re-rodou os
14 geradores de `docs/dossie/` (os derivados apontavam para `ccb45b1`, antes do
211) e a Frente Portão construiu o `docs/dossie/portao-dos-geradores.py`, que
pergunta a cada gerador *«re-rodar mudaria o alvo?»* — roda o gerador de verdade,
compara o arquivo antes/depois e devolve os bytes. VERDE no tronco pós-H, prova
real RED→GREEN refeita na integração.

Só que a Frente H achou um **15º derivado fora da conta**: o `docs/TECNOLOGIAS.md`
§4.5 dizia **1.659** testes onde o dossiê já media **2.209** — uma diferença de
550, não de 2. E o gerador dele, `docs/tecnologias/extrair.py`, **imprime** os
blocos Markdown e alguém **cola** à mão no `.md`.

## 2. O que eu concluí primeiro, e estava errado

Que «amarrar os geradores a um portão» estava resolvido com o portão sobre os 14.
Errado por dois motivos, e o segundo é o que ensina:

- **O inventário «14 geradores» era o de uma pasta.** Contei os `docs/dossie/*.py`.
  O do TECNOLOGIAS mora em `docs/tecnologias/` — pasta irmã —, então nunca entrou
  na conta. Inventário por pasta é inventário que perde o vizinho.
- **Mesmo se eu tivesse posto o `extrair.py` no portão, o portão não o pegaria.**
  O portão detecta «re-rodar mudaria o **arquivo**?». Um gerador que **imprime**
  nunca muda o arquivo — o arquivo só muda quando um humano cola. O portão de
  *run-and-diff* é cego, por construção, para o gerador que não escreve. Ele
  garante que quem escreve está em dia; não vê quem só imprime.

## 3. O que a medição disse

`docs/TECNOLOGIAS.md` §4.5: `cargo test --workspace: … 1.659 testes passaram`, e
o parágrafo seguinte *«registra o mesmo 1.659»*. Medido hoje pelo mesmo caminho
(o `numeros-do-projeto.py` da Frente H): **2.209** — 550 testes atrás. O
`extrair.py` roda `cargo test --workspace` (por isso é lento e não entrou na
receita barata dos 14). O portão cobre **14** geradores e marca o
`numeros-do-projeto` como NOTA (também chama cargo); o 15º, o do TECNOLOGIAS,
está fora dos dois.

## 4. A regra

**Um portão que confere «re-rodar mudaria o arquivo?» só cobre gerador que
ESCREVE o arquivo. Gerador que imprime-e-cola escapa dele por construção — ou se
converte para escrever no lugar (e aí o portão o pega), ou se gateia de outro
jeito: comparando a saída impressa contra o que está colado.** E o inventário de
geradores não é «os de uma pasta»: número visível gerado numa pasta irmã conta
igual, e some do portão se o inventário for por diretório.

## 5. Como está guardado hoje, e onde o buraco ficou

O **número** foi consertado nesta rodada rodando o `extrair.py` (o gerador) e
colando a saída — não digitado à mão. O **buraco estrutural fica registrado como
pendência**, porque tem duas saídas e a escolha é de projeto: (a) converter o
`extrair.py` para escrever no `TECNOLOGIAS.md` no lugar (como os 14 do dossiê) e
então incluí-lo no portão de *run-and-diff*; ou (b) um portão próprio que rode o
`extrair.py`, capture a saída e compare bloco a bloco contra o que está colado no
`.md`. Enquanto uma das duas não entra, o TECNOLOGIAS continua sendo o único
número visível do projeto que um humano precisa lembrar de colar — o mesmo tipo
de esquecimento que o portão dos 14 existe para matar.

## 6. O que entrou depois (11/09/2026, mais tarde no mesmo dia)

A **raiz** foi consertada, e ela é anterior à escolha entre (a) e (b): o
`extrair.py` **rodava o seu próprio `cargo test --workspace`** e somava os
`test result:` — uma SEGUNDA medição, paralela à do `numeros-do-projeto.py`,
que já mede a mesma coisa e a escreve no README, no `TESTES.md` e no
`CAPABILITIES.json`. **A divergência de 550 não foi só «alguém não recolou»:
foi duas medições da mesma coisa**, e duas medições da mesma coisa divergem
sempre que uma é refeita e a outra não — recolar não resolve, só adia. O
`bloco_testes_cargo` virou `bloco_testes`, que **lê o `CAPABILITIES.json`**
(fonte única, com a data e o commit da medição) em vez de re-medir. Agora o
número de TECNOLOGIAS é, por construção, o mesmo do README e do `TESTES.md`;
não pode divergir. De quebra, o extrator ficou **sem `cargo`** — não disputa
disco nem árvore com outra frente, que era o motivo do aviso «RODADA
SUSPEITA» que o código antigo precisava emitir.

Isto é a regra da casa aplicada a um gerador: **quando um número depende de
uma medição, ele sai do lugar que já a fez, nunca de refazê-la em paralelo** —
o mesmo que fez a lista do KiB de interface sair do `http.rs` em vez de ser
copiada. O que a raiz **não** fecha é a mesma coisa da §4: o `extrair.py`
ainda **imprime e alguém cola**, então o portão de *run-and-diff* continua
cego a ele. A escolha (a)/(b) segue de pé, e agora é a única parte que resta —
e ela é decisão de projeto porque mexe na ESTRUTURA do `TECNOLOGIAS.md`
(marcadores de fim em 16 blocos), um documento escrito à mão, não gerado
inteiro como as páginas do dossiê.

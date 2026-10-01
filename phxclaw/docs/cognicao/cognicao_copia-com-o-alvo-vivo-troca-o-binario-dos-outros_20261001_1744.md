# Cópia do workspace com o `target` vivo troca o binário de quem compila na árvore viva

**Estado:** PENDENTE (observado e explicado; falta a guarda que impeça — ver «A regra»)

## O que aconteceu

Na SP000028 (01/10/2026), o mesmo `cargo test -p phxclaw-agent` na árvore viva falhou três vezes
seguidas com erros impossíveis: `no field verificar on Task`, `no esquema in the root`, `could not
find responsivo in phxclaw_ui_ir` — com os três arquivos certos no disco. Um `touch` resolvia, e
dez minutos depois voltava.

## O que eu concluí primeiro, e estava errado

Que era corrida de `mtime` entre dois `cargo` na mesma árvore (a impressão digital gravada com o
conteúdo velho). Não era: o `/proc/<pid>/environ` do processo concorrente mostrou
`CARGO_TARGET_DIR=/home/user/adrianoboller/phxclaw/target` com o `cwd` numa **cópia** em
`scratchpad/copia/phxclaw`. O cargo calcula o hash dos membros do workspace pelo caminho
**relativo à raiz**, então a cópia e a árvore viva geram os mesmos nomes de artefato
(`libphxclaw_agent-1a4cf2ba65e0f78d.rlib`) — e a cópia, sem as mudanças desta frente,
sobrescrevia a biblioteca que os meus testes ligavam.

## O que a medição disse

- Os três erros sumiam com `touch` na fonte e voltavam depois de cada corrida da outra cópia.
- Com a cópia isolada desta frente em `/dev/shm/sp28-mut` e `CARGO_TARGET_DIR` próprio, os
  mesmos testes passaram de primeira e nunca mais oscilaram (143 testes, 15 mutantes).

## A regra

Cópia isolada é **cópia + `CARGO_TARGET_DIR` próprio**. Cópia que aponta para o `target` vivo
não é isolada: ela troca, sem aviso, o binário de toda frente que compila na árvore viva — e o
erro aparece como «campo que não existe», nunca como «alvo compartilhado».

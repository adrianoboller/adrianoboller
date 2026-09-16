# `bash -c` só se auto-substitui num comando SIMPLES — e é isso que fura o crivo de texto

Descoberto em 16/09/2026, ~11:05 UTC, implementando o conserto do portão
`bancada/esta-medindo.sh` (pedido 260).

## 1. O que aconteceu

A regra já vinha decidida ("invólucro de shell não conta: o portão olha só o
processo que EXECUTA `python3 bancada/*.py`, não quem o lançou"), e a
implementação virou trocar o crivo por SUBSTRING do `cmdline` por um crivo
pelo `/proc/<pid>/exe` — só conta "bancada em python" quem o próprio
executável for `python3*`. Antes de escrever o teste de regressão, medi POR
QUE a casca (`bash -c "cd X && python3 bancada/…"`) sobrevive como processo
`bash` em vez de virar `python3`:

```
$ bash -c "sleep 50" &         # comando SIMPLES
$ readlink /proc/<pid>/exe     → /usr/bin/sleep       (bash sumiu, virou sleep)

$ bash -c "cd X && python3 -c '...'" &   # comando COMPOSTO (tem &&)
$ readlink /proc/<pid>/exe     → /usr/bin/bash        (bash continua bash)
```

`bash -c "CMD"` faz uma otimização interna: quando `CMD` é um **comando
simples só** (sem `&&`, `;`, pipe, redirecionamento), o bash faz `execve`
substituindo o próprio processo pelo comando — não sobra `bash` nenhum, e o
PID vira o do comando. Quando `CMD` é **composto**, o bash não pode se
substituir (ele ainda precisa rodar o segundo pedaço depois do primeiro),
então ele **fica de pé** como `bash`, com o texto inteiro do `-c` no seu
próprio `/proc/<pid>/cmdline` — e é exatamente esse texto que o crivo velho
(por substring) casava.

## 2. O que eu concluí primeiro, e estava errado

O item 0b já tinha uma simulação de "bancada falsa" para não precisar subir
uma de verdade: `bash -c 'exec -a "python3 bancada/…/foo.py" sleep 20'`. Achei
que essa simulação continuaria funcionando depois do conserto — `exec -a`
troca só o **nome exibido** (`argv[0]`), então pensei que o `cmdline` mentiria
do mesmo jeito e o novo crivo (por `exe`) ainda acharia "python3" ali. Estava
errado: `exec -a NOME sleep 20` é um `execve()` de verdade trocando o
executável do processo por `sleep` — `argv[0]` fica com o nome falso, mas
`/proc/<pid>/exe` aponta para o binário real de `sleep`, nunca para
`python3`. O crivo novo, que olha `exe` e não `cmdline`, parou de achar a
simulação — corretamente, porque a simulação nunca foi um `python3` de
verdade. Tive de trocar a simulação por um `python3 -c "…tempo de espera…"
bancada/…/foo.py` real (o caminho da bancada entra só como argumento extra de
um `-c`, nunca aberto como arquivo) para o item 0b continuar provando o
SENTIDO 2 com um processo que é `python3` de verdade.

## 3. O que a medição disse

Confirmado com `ps`/`readlink` ao vivo (comandos acima): comando simples via
`bash -c` → substitui; comando composto (`&&`, `;`) → não substitui, casca
fica. E `exec -a` muda `exe`, não só `cmdline` — testado subindo
`bash -c 'exec -a "python3 x.py" sleep 5'` e lendo `/proc/<pid>/exe` logo em
seguida: sempre o binário do `sleep`.

## 4. A regra

**Um crivo por `/proc/<pid>/exe` não se engana por nada que mexa só no
`cmdline`/`argv[0]` — porque trocar o `exe` de verdade exige um `execve()`,
que troca o programa inteiro.** E o inverso, que é o motivo de a casca
sobreviver: **`bash -c` só vira o comando quando o comando é simples; com
`&&`/`;`/pipe, o `bash` fica de pé com o texto inteiro no próprio `cmdline`.**
Qualquer simulação de "processo X" que só mexe em `argv[0]` (via `exec -a`,
`setproctitle` em outras linguagens, etc.) não engana um crivo que olha `exe`
— e é assim que deve ser: se enganasse, o crivo teria voltado a confiar no
`cmdline`.

## 5. Como está guardado hoje

`bancada/esta-medindo.sh`: os dois crivos de "bancada" (python e exemplo)
exigem `exe`/`exe_nome` batendo com o interpretador ou o binário do exemplo,
com o porquê no comentário do cabeçalho. `bancada/bateria/prova-bateria.py`,
`item_0b_o_portao_nao_se_acha()`: a simulação do SENTIDO 2 trocou de
`exec -a "python3 …" sleep 20` para `python3 -c "…" bancada/…/foo.py` de
verdade; e entrou um caso NOVO, específico do pedido 260, que sobe uma casca
de verdade (`bash -c "sleep 20; echo bancada/…"`, comando composto de
propósito) e confere pelo PID exato que ela NÃO aparece no portão — essa
conferência é robusta a ruído de máquina compartilhada (outra frente
compilando não muda o veredito, porque a conferência é sobre um PID
específico, não sobre "a máquina está limpa").

# A trava do cargo aplicada duas vezes trava a corrida

**Descoberto em** 17/09/2026, 19:12 UTC, rodando o medidor comparativo com os
seis campos de evidência do pedido 335.

## 1. O que aconteceu

Chamei o medidor assim:

```bash
flock /tmp/phx-cargo.lock python3 bancada/comparativo/medir.py
```

A lei desta casa é *todo `cargo` sob `flock /tmp/phx-cargo.lock`* — duas
frentes compilando ao mesmo tempo derrubam uma a outra. Eu a apliquei.

A corrida imprimiu a primeira linha e **parou para sempre**. Dois minutos
depois a árvore de processos dizia o motivo inteiro:

```
1029 flock /tmp/phx-cargo.lock python3 bancada/comparativo/medir.py
1030 python3 bancada/comparativo/medir.py
1031 flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
```

O PID 1031 é o `compila_o_nosso()` do próprio medidor (`medir.py:1280`), que
**já** toma a trava por dentro — com o comentário explicando por quê, escrito
antes de eu chegar. O PID 1029 segurava a trava e esperava o filho; o filho
esperava a trava. Nenhum dos dois ia se mexer.

`flock(1)` **não é reentrante**: a trava do `flock(2)` pertence à *descrição*
de arquivo aberto, e o segundo processo faz um `open()` novo, que é uma
descrição nova. Herdar o descritor daria a trava de graça; abrir o arquivo de
novo enfileira atrás de si mesmo.

## 2. Como se mediu, e não se adivinhou

Não foi por leitura. Foi `pgrep -af` sobre a árvore viva, com o `cmdline` de
cada PID lido em `/proc/<pid>/cmdline` — e aí a linha 1031 **é** o
diagnóstico, porque ela mostra o segundo `flock` sobre o mesmo caminho. O
`log` não ajudava: ele tinha uma linha e nenhum erro, que é o retrato que um
travamento sempre dá.

Derrubei **pelos três PIDs**, nunca por `pkill` — eram meus, eu os havia
começado e sabia os números. Rodado de novo sem o `flock` externo, o medidor
andou.

## 3. O que eu concluí primeiro, e estava errado

Concluí que estava **cumprindo** a lei. E cumprir a lei era exatamente o
defeito: a lei diz *todo `cargo` sob `flock`*, e eu a li como *toda coisa que
chama `cargo` sob `flock`*. São regras diferentes, e a segunda é falsa.

Antes disso, a suspeita foi «cargo está compilando devagar» — plausível, e o
tipo de explicação que sobrevive porque um `cargo build --release` lento é
verdade quase sempre. Quinze minutos de espera se explicam assim sem esforço
nenhum. *Diagnóstico plausível não é diagnóstico medido*, e este só caiu
quando eu olhei a árvore de processos em vez de olhar o relógio.

## 4. O alcance da lei, que é o aprendizado

A lei não muda: `cargo` continua indo sob `flock`. O que se aprende é **onde
ela se aplica** — no ponto que invoca o `cargo`, e **em um ponto só**.

E a regra prática, que serve para qualquer trava e não só para esta:

> **Antes de envolver um comando numa trava, procure se ele já a toma.** Uma
> trava tomada duas vezes no mesmo caminho não protege duas vezes: ela para.

É o irmão de uma lei que esta casa já tem escrita — *envolver não é
substituir* — pelo lado contrário: lá o defeito era embrulhar um erro cru e
achar que embrulhar resolvia; aqui é embrulhar uma proteção que já existia e
achar que embrulhar reforçava.

## 5. A guarda, e por que ela não nasce agora

O conferidor tentador seria «nenhuma chamada a `flock /tmp/phx-cargo.lock`
pode envolver um script que também a chame». Ele é caro e fraco: o que eu
digitei não está em arquivo nenhum do repositório — foi uma linha de shell
desta sessão, que nenhuma catraca alcança.

O que **fica**, e é do tamanho certo do problema: o `compila_o_nosso()` ganhou
a frase no cabeçalho dizendo que a trava é tomada **ali** e que quem chama o
medidor **não** deve tomá-la de fora. Quem for envolver o medidor lê isso na
função que trava.

# `pgrep -c` sem `-f` casa o INTERPRETADOR, e eu abri um portão em cima do outro

17/09/2026, 16:10 UTC. Integrador (papel A), fechando a rodada dos pedidos
328–331.

## 1. O que aconteceu

Pus o portão dos geradores para rodar em segundo plano e quis saber se ele ainda
estava vivo. Perguntei assim:

```
pgrep -c "portao-dos-ger" || echo nao
```

Veio `0`, e o `|| echo nao` imprimiu `nao`. Concluí que tinha terminado, e
disparei **um segundo portão**. Os dois ficaram rodando ao mesmo tempo:

```
23764 python3 docs/dossie/portao-dos-geradores.py
23841 python3 docs/dossie/portao-dos-geradores.py
```

E portão não é leitor: ele **roda os geradores**, que gravam o dossiê, o
`TECNOLOGIAS.md` e as páginas. Dois `extrair.py` ao mesmo tempo gravam o mesmo
arquivo.

## 2. O que eu concluí primeiro, e estava errado

Que o processo tinha acabado, porque `pgrep` disse que não havia nenhum.

Estava errado, e o motivo é do `pgrep`, não do processo: **sem `-f`, o `pgrep`
casa o `comm`** — o nome do executável, que aqui é `python3`. O padrão
`portao-dos-ger` nunca casaria coisa nenhuma, mesmo com dez portões rodando.

Minutos antes, na mesma sessão, eu tinha usado `pgrep -af "portao-dos-geradores"`
e **achado** o processo. A resposta certa e a errada estavam na mesma janela, e
eu não comparei as duas — troquei `-af` por `-c` para contar, e a troca levou o
`-f` junto sem que eu percebesse.

## 3. O que a medição disse

- `pgrep -af "portao-dos-geradores.py"`, sem `| head`, listou **6** processos:
  duas correntes de três (`bash` → `timeout` → `python3`).
- `TECNOLOGIAS.md` saiu **intacto** — `git diff --stat` do arquivo veio vazio
  depois de parar o primeiro portão. A colisão não custou dado desta vez, e
  isso é sorte medida, não desenho.
- O `extrair.py`, que é onde os dois se encontraram, gasta **191 s de CPU**
  (`utime` de `/proc/<pid>/stat`, estado `R`) numa corrida. A janela de
  sobreposição foi larga.

## 4. A regra

**Pergunta sobre processo se faz por linha de comando (`pgrep -af`), nunca por
nome — script interpretado não tem o próprio nome em `comm`.** E antes de
disparar de novo um trabalho que ESCREVE, a prova de que o anterior morreu é
listar os processos, não contar.

## 5. Como está guardado hoje

**Não está**, e o buraco tem nome: é a **quinta** vez que um `pgrep` mente nesta
casa, e as quatro anteriores eram de outra família — o `pgrep` que **se acha**
(a linha de comando do próprio `grep` casando o padrão). Esta é a família
oposta: o `pgrep` que **não acha o que está lá**. As duas dão a mesma resposta
errada — «não há processo» — por motivos contrários.

O que existe hoje é o `comunicacao.sh`, que já foi corrigido uma vez por causa
do `pgrep` que se acha, e o pedido 323, que registra que ele ainda diz «nada
compilando nem rodando agora» com um gerador no meio da escrita. **O conferidor
genérico não existe**, e o alcance certo dele é o que o pedido 328 pede: quem
fecha a rodada não deveria estar digitando `pgrep` nenhum, porque o fecho
deveria ser um comando só.

---

## Adendo, 16:14 UTC — e a terceira foi na MESMA sessão, com o padrão certo

Depois de escrever isto eu ainda errei uma vez, e o erro merece ficar ao lado
porque a regra da seção 4 **não teria evitado** esta: eu usei `-f`, e mesmo
assim a pergunta respondeu sobre outra coisa.

Perguntei se o `extrair.py` ainda rodava com:

```
ps -eo etimes,args --no-headers | grep "[p]ython3 /home/user"
```

Veio vazio. Mas o processo estava lá, com 42 s. O padrão `python3 /home/user`
tinha funcionado minutos antes — quando quem invocava o `extrair.py` era o
**portão**, que o chama por caminho **absoluto**
(`/usr/local/bin/python3 /home/user/.../extrair.py`). Quando **eu** o chamei, foi
por caminho **relativo** (`python3 docs/tecnologias/extrair.py`), e o mesmo
padrão deixou de casar.

**O padrão que casou uma vez não é o padrão certo: ele casou o CHAMADOR daquela
vez.** O que é invariante é o nome do script, e é por ele que se pergunta:
`pgrep -af "extrair\.py"`.

E o agravante que fecha o assunto: nas três vezes desta sessão a resposta errada
foi sempre a mesma, **«não há»** — a resposta que autoriza a agir. Pergunta
sobre processo erra para o lado perigoso por construção, porque o vazio é
indistinguível entre «não existe» e «não perguntei direito».

---

## Segundo adendo, 16:22 UTC — a quarta vez, e agora com `-af`

A regra do primeiro adendo — «pergunte por `pgrep -af` e pelo nome do script» —
**não evitou a quarta**. Eu escrevi, com `-af` e com o nome certo:

```
pgrep -af "status-do-projeto\.py\|status-html"
```

Nada. E o processo estava lá: `python3 docs/status/pagina-do-status-do-projeto.py`.

Medido na hora, com o processo ainda vivo, em quatro formas:

| padrão | achou? |
|---|---|
| `pgrep -af "status-do-projeto\.py"` | **sim** |
| `pgrep -af "status-do-projeto.py"` | **sim** |
| `pgrep -af "pagina-do-status"` | **sim** |
| `pgrep -f "pagina-do-status"` | **sim** |

Ou seja: o `-af` estava certo e o nome estava certo. **O que estava errado era a
linguagem do padrão.** O `pgrep` casa por **ERE**, onde alternância é `|` sem
barra; eu escrevi `\|`, que é a alternância do **BRE** do `grep`. Em ERE, `\|` é
um `|` **literal** — o padrão virou a string
`status-do-projeto.py|status-html`, que nenhum processo tem no nome.

**A regra que faltava:** a barra que o `grep` exige é a que o `pgrep` proíbe.
Ao levar um padrão de uma ferramenta para outra, o que muda primeiro não é o
padrão — é a gramática dele.

E o que fecha as quatro: **as quatro responderam «não há»**. Nome errado,
`comm` no lugar do `cmdline`, caminho relativo contra absoluto, BRE dentro de
ERE — quatro mecanismos diferentes, uma resposta só, e é a resposta que
autoriza a agir. **Pergunta sobre processo não tem falso negativo barato:**
enquanto a resposta afirmativa se prova sozinha (o PID está ali), a negativa
nunca se prova — ela é indistinguível de «perguntei errado».

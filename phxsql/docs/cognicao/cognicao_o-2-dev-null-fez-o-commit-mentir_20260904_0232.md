# O `2>/dev/null` num `git add` fez o commit mentir sobre si mesmo

**Descoberto em 04/09/2026, 02:32**, integrando a frente de pesquisa.

## 1. O que aconteceu

O commit `7b4bf21` saiu com uma mensagem de trinta linhas contando a correcao do
cabecalho do slot em tres documentos. O que ele levou:

```
 phxsql/docs/{PESQUISA-MVCC-E-TRAVA.md => PESQUISA-MVCC-E-FORMATO.md} | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
```

**So o renome.** Zero insercoes, zero remocoes. E foi empurrado assim.

A causa esta no proprio comando. Eu tinha acabado de fazer `git mv` e escrevi:

```sh
git add -- docs/PESQUISA-MVCC-E-FORMATO.md docs/PESQUISA-MVCC-E-TRAVA.md \
           docs/cognicao/ docs/CONCORRENCIA.md docs/PENDENCIAS.md 2>/dev/null
```

`docs/PESQUISA-MVCC-E-TRAVA.md` **ja nao existia** -- eu mesmo o tinha renomeado
tres linhas antes. Pathspec que nao casa faz o `git add` abortar **inteiro**, e
o `2>/dev/null` mandou a reclamacao para o lixo. Nada foi acrescentado ao
indice, o `commit` seguinte pegou so o que o `git mv` ja tinha posto la, e a
mensagem virou ficcao.

## 2. O que eu concluí primeiro, e estava errado

Eu **conferi**, e conferi errado. Rodei `git status --porcelain` logo depois do
add e li:

```
 M phxsql/docs/CONCORRENCIA.md
RM phxsql/docs/PESQUISA-MVCC-E-TRAVA.md -> phxsql/docs/PESQUISA-MVCC-E-FORMATO.md
```

e concluí «esta tudo preparado». Mas o `porcelain` tem **duas colunas**: a
primeira e o INDICE, a segunda e a ARVORE. Aquele ` M` com espaco na frente quer
dizer *modificado na arvore e nao preparado*. So o `R` do renome estava na
coluna que importava — e ele estava la porque o `git mv` prepara sozinho, nao
porque o meu `add` funcionou.

Foi o mesmo erro de leitura que a frente de pesquisa acabara de me apanhar horas
antes, noutro objeto: eu li a **soma do layout** (24 bytes) e conclui «cabecalho
cheio», quando o que decidia era **quem escreve e quem le cada faixa** (3 bytes
livres). Duas vezes no mesmo dia eu olhei o resumo em vez do que decide.

## 3. O que a medição disse

Refeito o `add` **sem** `2>/dev/null` e conferindo o indice em vez da arvore:

| conferencia | resultado |
|---|---|
| `git add …; echo $?` | **0** — antes, o erro estava escondido |
| `git diff --cached --stat` | **5 arquivos, 34 insercoes, 17 remocoes** |
| `git diff --stat` (fora do indice) | **vazio** |

Os 34 e 17 sao exatamente o conteudo que o commit anterior prometia e nao tinha.

## 4. A regra

**`2>/dev/null` em comando que MUDA estado transforma falha em silencio.** Num
`grep` ou num `ls` e conveniencia; num `git add`, num `cp` ou num `mv` e um
resultado inventado que ninguem contesta.

E o corolario, que e o mais util: **depois de montar um commit, confira o
INDICE, nunca a arvore.** `git diff --cached --stat` responde «o que este commit
vai levar»; `git status` responde «o que existe por aqui», e as duas perguntas
so parecem a mesma ate o dia em que o `add` falha calado.

## 5. Como está guardado hoje

- O conteudo foi commitado de verdade, com a mensagem contando **por que** o
  commit anterior saiu vazio — a correcao ficou no historico ao lado do erro,
  em vez de o erro ser reescrito para fora dele.
- O commit vazio **fica**. Ele ja estava empurrado, e reescrever historico para
  esconder um commit meu vazio seria pior que o commit vazio: quem clonou entre
  os dois veria a arvore mudar por baixo.
- **O buraco que fica:** nao ha guarda automatica para isto. Um gancho de
  pre-commit poderia recusar commit sem arquivo algum alterado, mas commit
  vazio e legitimo em outros casos (marca, reversao vazia), e guarda que recusa
  caso legitimo vira guarda que se desliga. Fica como disciplina escrita, e a
  disciplina e uma linha: **conferir `git diff --cached --stat` antes de
  `commit`.**

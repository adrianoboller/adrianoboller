# Frentes concorrentes no mesmo working tree: o `git add` de uma engole a outra, e o rebobinar diverge o branch

Data da descoberta: 12/09/2026, 04:56 (o engolir) e 05:10 (a divergência) —
mesmo dia, mesma raiz, um arquivo só.

## 1. O que aconteceu

Rodei tres frentes ao mesmo tempo no MESMO working tree: o agente do motor
(achados 241/243/244/245), o pesquisador J e o documentador H. O J escrevia so
no scratchpad (fora do repo, sem colisao). O H criou `docs/PDCA-GAPS.md` no repo
e nao commitou — certo. Mesmo assim:

- **(a) o engolir.** O `git add` amplo do agente do motor varreu o
  `docs/PDCA-GAPS.md` **nao-rastreado** do H para dentro do commit do achado 244
  (`cc53acd`: `git show --stat` = 528 linhas de doc + 85 de `servidor.rs` + 2 de
  `PENDENCIAS.md` num commit cuja mensagem so fala de "Visao com JOIN").
- **(b) eu empurrei** `cc53acd` e `36063f2` ao `origin`, achando que empurrar
  commit intermediario de frente concorrente era backup barato.
- **(c) o rebobinar.** O agente percebeu que engolira o doc e fez `git reset`
  para `2bb9d83` (reflog `HEAD@{2}`), refez 244 e O1 limpos (`cfc6fdc`,
  `06b8c82`). Isso **reescreveu historia que eu ja empurrara**, e o branch local
  divergiu do `origin`: 2 commits de cada lado a partir do 243.

Nenhuma frente sozinha via nada: o H nao commitou, o motor nao sabia do H, e eu
empurrei entre um passo e outro. O estrago mora no encontro.

## 2. O que eu concluí primeiro, e estava errado

Escrevi, com todas as letras, que criar um arquivo NOVO (untracked) numa frente
"nao colidiria" com outra que so edita `.rs` — "arquivos diferentes, sem
conflito de conteudo". Errado. O conflito nunca foi de **conteudo**; e do
**indice e do HEAD compartilhados**. Um `git add -A` nao distingue de quem e o
arquivo; um `git reset` move o HEAD de todas as frentes de uma vez. E o meu erro
proprio — empurrar o intermediario — foi o que transformou um rebobinar
**local** (inocuo, some no reflog) numa **divergencia com o remoto** (que custa
force-push ou merge para desfazer).

## 3. O que a medição disse

- `git show --stat cc53acd`: **528** (doc) + **85** (`servidor.rs`) + **2**
  (`PENDENCIAS.md`) — um commit, tres frentes.
- `git rev-list --count`: **2** local-so, **2** origin-so — divergencia
  confirmada.
- `git diff HEAD origin` depois do reset do agente: **so o doc, 528 insercoes**.
  Prova de que o CODIGO das quatro correcoes era **identico** nos dois lados; a
  divergencia era 100% de empacotamento, zero de logica.

## 4. A regra

Frentes concorrentes no MESMO working tree: **so o integrador commita e
empurra**. Cada agente usa `git add` por **caminho explicito** — nunca `-A`,
`-u` ou `.` — e **nunca** `git reset`/rebase/amend/force. O integrador **nao
empurra commit intermediario de agente vivo**: espera a frente terminar e
empurra uma vez. Melhor que tudo isso: **isole cada agente num worktree
proprio**, e o problema do indice compartilhado deixa de existir.

## 5. Como está guardado hoje

Reconciliei por `git reset --hard origin` — nao-destrutivo a historia empurrada:
aceitei o commit com o doc empacotado em vez de force-push sem autorizacao do
dono (a regra manda `force` so quando o dono mandou descartar aquela versao). O
codigo e identico, entao nada se perdeu; o `PDCA-GAPS.md` fica rastreado, so num
commit de nome infeliz.

O buraco que fica, nomeado: **as instrucoes que passei aos agentes nao proibiam
`git add -A` nem `git reset`**, e nao ha catraca que impeca. A proxima frente
concorrente repete ate eu (a) por isolamento por worktree no disparo do agente,
ou (b) escrever a proibicao explicita no prompt padrao. Guarda nova entra
pedida, nao imposta — fica anotado como pendencia de processo, nao como catraca
que ja existe.

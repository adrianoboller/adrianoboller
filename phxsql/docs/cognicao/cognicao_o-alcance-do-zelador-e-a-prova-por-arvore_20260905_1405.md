# O alcance de uma guarda falha duas vezes, e a prova de redundância é a ÁRVORE

**05/09/2026, 14:05** — descoberto atendendo um aviso do agente de comunicação
que dizia «disco: 1978 MiB livres — abaixo do limite de 2 GiB».

## 1. O que aconteceu

O zelador rodou e liberou **125 MiB** num disco com 2,0 GiB livres. Os 5,6 GiB
que faltavam estavam em três lugares que ele **não enxerga**:

| lugar | tamanho | o que era |
|---|---|---|
| `/root/.cache/phx-guardas` | 3,0 GiB | cópia da árvore do provador de guardas |
| `scratchpad/pw` | 924 MiB | navegadores do Playwright, duplicando `/opt/pw-browsers` |
| `scratchpad/tlm`, `guardas-integra`, `ensaio-rebase` | 1,7 GiB | cópias de frentes encerradas |

E o script estava **certo** no que fez: recusou tocar no `target` porque havia
processo vivo com `cwd` na árvore. Ele não errou o julgamento; ele não foi
convidado a julgar.

## 2. O que eu concluí primeiro, e estava errado

**Três vezes, e as três valem.**

**A primeira:** olhei o `ensaio-rebase`, vi doze commits cujo SHA não existia
no repositório principal — entre eles «Pedido 172», quatro cognições e a
matriz de falhas da durabilidade — e concluí que ali havia trabalho que se
perderia. Ia preservar a pasta inteira.

Estava errado, e o nome da pasta dizia por quê: *ensaio-rebase*. Depois de um
rebase, **todo** commit muda de SHA. Comparar por SHA responde «falta» para
trabalho que está inteiro no lugar certo.

**A segunda:** então comparei por **assunto** — os doze assuntos existiam no
principal, e quase publiquei isso como prova. Não é: dois commits podem ter o
mesmo assunto e conteúdo diferente. Assunto é rótulo, e rótulo não é dado.

**A terceira, e é a pior, porque eu tinha acabado de citar a lei que ela
quebra:** escrevi a extensão do zelador para apagar toda cópia com
`Cargo.toml` e `crates/`. Rodei a prova real, e ela pegou duas cópias — a
`phxsql-cab` e a `phxsql-medicao` — que **não são repositório nenhum**. Sem
`.git` não há como provar redundância de conteúdo: eu apagaria por palpite,
que é exatamente o que o cabeçalho do próprio arquivo proíbe, três parágrafos
acima de onde eu estava escrevendo.

E havia uma quarta, que a mesma prova pegou: o detector exigia `Cargo.toml` na
**raiz** da cópia. Um clone deste repositório tem `phxsql/Cargo.toml`, porque
o projeto mora num subdiretório — então o ramo que faz a prova por git **nunca
disparava**, e a cópia que motivou a seção inteira não teria sido pega. Guarda
que não pode disparar não é guarda.

## 3. O que a medição disse

A prova que decide é o **hash da árvore**, e não o do commit:

```text
principal: 5fd1c6b   copia: aa2e66f          <- SHAs diferentes
arvore no principal: 0499b5b91a95c59f9cbc9e7f9fdc270b13ccf24e
arvore na copia:     0499b5b91a95c59f9cbc9e7f9fdc270b13ccf24e   <- IDENTICAS
```

Mesmo conteúdo, byte a byte, com commit diferente pelo rebase. Nada se perdia.
Com essa prova na mão, mais a de uso vivo (zero processos com `cwd` dentro,
zero descritores abertos em `/proc/*/fd`), o disco foi de **2,0 GiB para
7,7 GiB**.

A prova real da guarda nova, nos dois sentidos, com os casos montados:

| caso | veredito |
|---|---|
| cópia redundante | **apagada**, 354 MiB |
| trabalho não commitado | fica |
| árvore que o principal não tem | fica |
| cópia sem `.git` | fica — não dá para provar |
| em uso por processo vivo | não toco, e digo qual PID |

## 4. A regra

**Redundância de conteúdo se prova pelo hash da ÁRVORE — nunca pelo SHA do
commit, que o rebase troca, nem pelo assunto, que é rótulo. E o que não se
pode provar, fica.**

E a regra do alcance, que é a segunda vez que esta casa a paga: **quando uma
guarda falhar por alcance, procure o próximo lugar da mesma família antes de
declarar o conserto pronto.** O commit que criou a seção dos bundles diz «a
guarda existia, mas o alcance dela parava no `target` e nos worktrees» — e
parou de novo, uma pasta adiante.

## 5. Como está guardado hoje

No `zelador.sh`, seção «copias derivadas fora do repositorio», com as quatro
armadilhas escritas por dentro: as duas formas de cópia (subdiretório e clone
do repositório), o motivo de cópia sem `.git` ficar, e a prova por árvore com
o número de hoje ao lado.

**O buraco, e ele fica nomeado:** as duas cópias sem `.git` — `phxsql-cab` e
`phxsql-medicao`, 18 MiB somadas — continuam ali e continuarão, porque não há
como provar que são redundantes. Aparecem em toda corrida dizendo isso. É
pouco espaço e é o preço certo: apagar sem prova custaria uma vez o que
guardar custa sempre.

E o que **não** foi medido: se o provador de guardas fica mais lento sem o
cache de 3,0 GiB. Ele o recria — mas quanto isso custa por corrida ninguém
mediu, e enquanto não medir, «o provador recria» é afirmação de projeto e não
número.

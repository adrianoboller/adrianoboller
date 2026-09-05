# O `strace` parte um `syscall` concorrente em duas linhas, e o parser perde um terço

*Descoberto em 05/09/2026, 11:20, ao rodar a prova do fecho contra o `phxsqld`
de pé depois de o fecho passar a sincronizar as K tabelas ao mesmo tempo.*

## 1. O que aconteceu

O fecho da janela de durabilidade passou a sincronizar as K tabelas sujas em
paralelo (§12.6 do `docs/CONCORRENCIA.md`). A bateria
`bancada/durabilidade/prova-do-fecho.py` — que sobe um `phxsqld` de verdade,
**anexa** `strace -f -y -ttt` no PID dele e conta `fsync` por arquivo — passou a
sair quase toda zerada, com o próprio aviso interno dela dizendo *«o `strace`
foi solto antes de `descarregar_sujas_com` terminar»*.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que era a máquina.** A bateria de guardas estava rodando ao lado, a
caixa em 100%, e o aviso do script fala em tempo — «solto antes de terminar».
Ia esperar a máquina esvaziar e rodar de novo. A segunda hipótese, quase tão
plausível, era pior: *o `strace` anexado não segue as threads que nascem depois
do anexo* — o que teria feito o conserto do comboio ser invisível para toda a
instrumentação da casa.

**As duas estavam erradas, e o `strace` tinha visto tudo.** O que se perdeu foi
na **leitura** do traço: `strace -f` parte uma chamada em duas linhas —
`fsync(13</…/tab01.trash> <unfinished ...>` e, mais adiante,
`<... fsync resumed>) = 0` — sempre que outra thread entra num `syscall` antes
de a primeira voltar. **Só a primeira linha traz o caminho, e só a segunda traz
o resultado.** A expressão do script exigia o `= 0` na mesma linha do caminho,
então toda chamada concorrente sumia **em silêncio**.

Antes do conserto do comboio isso nunca acontecia neste caminho, porque o laço
era serial: entrava e voltava sem ninguém no meio. *O parser não quebrou; ele
sempre foi assim, e o código deixou de ser serial embaixo dele.*

## 3. O que a medição disse

`strace -f -y -ttt -e trace=fsync` sobre um fecho de K=4 do
`--example o-comboio-em-paralelo`:

| critério | `fsync` contados |
|---|---:|
| linhas contendo `fsync(` (a entrada, partida ou não) | **480** |
| a expressão antiga do `prova-do-fecho.py` | **310** |
| linhas `<unfinished ...>` | 170 |
| linhas `<... fsync resumed>` | 170 |

**Um terço do traço sumia.** Com a expressão consertada — três padrões (inteira,
aberta, fechada) e o par ida-volta casado **pelo pid** — o parser vê os mesmos
**480**. E a bateria, que saía com uma matriz de zeros e um controle interno
reprovado, passou a sair com **8 `fsync` em cada tabela suja** (`reg 1`, `ndx 2`,
`bin/memo/log/trash/reason 1`), contra o `phxsqld` de pé, nos dois cenários — o
número que a `TETO_FSYNC_POR_FECHO_V2` cobra.

O par se casa pelo **pid** e não por ordem: duas threads podem ter chamadas
abertas ao mesmo tempo, mas cada uma só tem **uma**, porque um `syscall`
bloqueia quem o chamou.

## 4. A regra

**Quem torna um caminho concorrente tem de reler os instrumentos que o mediam
em série.** O `strace` de um caminho serial nunca produz `<unfinished ...>`, e
um parser escrito ali nasce sem esse caso — e depois cala em vez de reclamar.

E o irmão dessa regra, porque foi o que separou o defeito do não-defeito: **um
contador por `contains("fsync(")` sobrevive à concorrência e um por expressão
com o retorno na mesma linha não.** Os dois pareciam a mesma coisa até o dia em
que duas threads chamaram `fsync` juntas.

## 5. Como está guardado hoje

* O conserto e o número dentro do próprio `eventos_fsync`, em
  `bancada/durabilidade/prova-do-fecho.py` — com as três expressões nomeadas
  (`INTEIRA`, `ABERTA`, `FECHADA`) e o motivo escrito ali, não aqui.
* Os outros quatro leitores de `strace` da casa foram conferidos um a um e
  **estão sãos por construção**: `fsync-por-fecho.rs`, `sonda-do-fecho.rs`,
  `sonda-do-volume-do-meio.rs`, `fecho-da-janela-sincroniza-o-reg.rs` e o
  `o-comboio-em-paralelo.rs` contam por `contains("fsync(")`, que casa a linha
  de **entrada** — partida ou não — e não casa a de volta. Cada chamada conta
  uma vez.
* Chamada aberta e nunca fechada **não entra** na matriz como sucesso: o traço
  terminou dentro dela, e «não se sabe se voltou bem» não é durabilidade
  provada.

**Onde o buraco ficou:** não há guarda que reprove um parser de `strace` novo
escrito com o retorno na mesma linha. O defeito é silencioso por natureza — ele
conta *menos*, e contar menos parece «o instrumento não viu», não «o instrumento
está errado». O que existe hoje é este arquivo e o comentário no fonte.

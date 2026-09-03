# A cascata pode deixar a filha com o índice sujo — e a recuperação recusa
em vez de arriscar

**Descoberto:** 03/09/2026, 10:54.
**Onde:** `phxsql-store/src/table.rs` (`aplicar_ao_alterar`, `recascatear`),
`phxsql-server/src/transacao.rs` (`completar`), `phxsql-store/src/ndx.rs`
(a marca de sujo do *write-back*).

## 1. O que aconteceu

Fechando a SP000010 (matriz de falhas × regime de durabilidade), o ponto 5 —
matar o processo no meio da cascata do `ao_alterar` — precisava de uma prova
por `SIGKILL` de verdade, não de leitura de código. Montei uma mãe com 1.200
filhas em cascata, mandei `BEGIN; UPDATE mae; COMMIT` e matei o processo com
atrasos variando ao longo da duração medida do commit.

Em **9 de 21** corridas (nos três regimes, na mesma proporção), depois do
servidor voltar, um `buscar` pela filha pelo índice `porMae` respondia:

```
[SP000010] arquivo corrompido: o indice de base/durab/filhas.ndx ficou para
tras numa queda e nao e confiavel: reconstrua com `reparar indice` antes de
usar
```

## 2. O que eu concluí primeiro, e estava errado

A primeira leitura foi: «achei um bug — a cascata da recuperação
(`recascatear`) não está fechando o índice da filha direito, e isso é outro
buraco da mesma família da §5.5.1 (a cascata que a reaplicação não refazia)».

Errado em dois níveis:

**Primeiro nível, o mais raso**: achei que era um **defeito de dado** — que
as linhas da filha tinham ficado com o `mae_id` errado. Não tinham. Lendo a
filha pelo `rowid` direto (sem passar pelo índice), o dado já estava correto
— `{'id': 1, 'mae_id': 999, ...}`, a mãe nova. O problema era só o
**índice**, recusando responder por não confiar em si mesmo.

**Segundo nível, o que importa**: achei que era um bug **da transação**.
Não é. É o mecanismo de *write-back* do `.ndx` (a marca de sujo do cabeçalho,
`docs/DESEMPENHO.md` §4.8) fazendo exatamente o que promete — e ele é **geral,
sem relação nenhuma com `.tx`**. `Table::aplicar_ao_alterar` sincroniza cada
filha por conta própria, no FIM do laço que reescreve as linhas dela; matar o
processo no meio desse laço deixa dirty pages no `.ndx` da filha, e a marca de
sujo sobe. Isso aconteceria **do mesmo jeito** numa alteração em cascata fora
de transação nenhuma — a marca `.tx` não tem nada a ver com o buraco.

O que a marca `.tx` FAZ é revelar o buraco de um jeito que uma alteração solta
não revelaria tão claramente: a recuperação da mãe (`recascatear`) precisa
perguntar ao índice sujo da filha "quem aponta pra você?", a pergunta recusa,
e essa recusa vira uma linha em `operacoes IMPOSSIVEIS` — honesta, nomeada, e
achável no relatório do arranque. Sem transação, a mesma sujeira no índice só
apareceria na próxima vez que ALGUÉM tentasse consultar a filha por aquele
índice, sem relatório nenhum avisando.

## 3. O que a medição disse

`bancada/durabilidade/prova.py`, ponto 5, 1.200 filhas, três regimes, 21
corridas:

| veredito | corridas |
|---|---:|
| CONSISTENTE (cascata não tocada, ou terminou inteira) | 12 |
| PARCIAL_DENUNCIADO (índice sujo, `reindexar` resolve, relatório avisou) | 9 |
| PARCIAL SEM AVISO (o desfecho que reprovaria a prova) | **0** |

Zero cascatas parciais silenciosas. Em toda vez que o dado ficou dividido
entre mãe nova e mãe velha, o relatório do arranque **disse isso** em
`operacoes IMPOSSIVEIS`.

E uma segunda medição, sobre a minha primeira leitura errada: rodei o mesmo
teste tentando `reindexar` a filha depois da queda — e o dado por baixo do
índice reconstruído estava sempre correto (as linhas que a cascata tinha
conseguido tocar mostravam o valor novo; as que não tinha tocado, o velho).
Nenhum byte de dado se perdeu. Só a **resposta do índice** ficava presa até
alguém consertar.

## 4. A regra

**Quando a recuperação de UMA tabela depende de consultar o índice de OUTRA,
pergunte se essa outra tabela pode estar suja por um caminho que não passa
pela marca `.tx`.** A `.tx` só protege o que ELA sincroniza; qualquer
sincronização que aconteça por fora dela (aqui, o `filha.sincronizar()` da
cascata) tem seu próprio relógio de queda, e a recuperação da marca não sabe
disso a menos que pergunte.

E o corolário do meu próprio erro: **"o índice recusou" não é "o dado está
errado"** — são dois relatórios diferentes, e confundi-los teria feito eu
escrever "achei um bug de integridade" quando o achado certo era "achei uma
recusa de disponibilidade, correta e denunciada".

## 5. Como está guardado hoje

* **Documentado, não consertado**: `docs/TRANSACOES.md` §5.5.3 e §5.7
  nomeiam o caso, com o número (9 de 21) e a garantia que sobrevive (nunca
  silencioso).
* **O que faltaria para consertar**: a recuperação teria que saber "tentar de
  novo mais tarde" para uma operação que já entrou em `operacoes
  IMPOSSIVEIS` — e a marca `.tx` já foi apagada quando isso acontece
  (`recuperar()` apaga a marca processada incondicionalmente). Não entrou
  nesta rodada; é trabalho de outra frente, e fica registrado aqui para não
  ser descoberto de novo do zero.
* **Onde o buraco fica**: até lá, uma cascata larga (muitas filhas) que caia
  numa queda no meio precisa de um operador rodando `reindexar` na filha e
  conferindo à mão as linhas que ficaram para trás — o relatório do arranque
  diz QUE aconteceu, não AUTOMATICAMENTE o que fazer.

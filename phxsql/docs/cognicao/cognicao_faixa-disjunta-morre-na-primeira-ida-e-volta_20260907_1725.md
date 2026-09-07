# Cognição: o remendo que funciona UMA vez não é remendo — a faixa disjunta do bidirecional

**Descoberta:** 07/09/2026 17:25 UTC, medindo o ciclo de vida da coluna
`Sequence` no modo `multi` (bancada `bancada/sequencias/sonda.py`, bloco 24).

## 1. O que aconteceu

A frente recebeu uma premissa escrita: *«replicação pull e cluster com eleição —
dois masters nunca ao mesmo tempo, então paridade não precisa»*. Medida, a
premissa está certa para o **cluster** e errada para o **produto**: o papel
`"multi"` do `config.json` põe dois servidores recebendo escrita ao mesmo tempo,
e é exatamente o cenário em que a paridade do MariaDB
(`auto_increment_increment` / `_offset`) existe.

Com os dois numerando a mesma faixa, **4 inserções deixaram 2 linhas** — em alfa
e em beta. Os ids colidiram por chave, a regra «mais recente vence» resolveu o
conflito como manda o `docs/REPLICACAO.md` §12, e as duas linhas de alfa
sumiram sem erro nenhum, dos dois lados.

## 2. O que eu concluí primeiro, e estava errado

Que o remendo já existia: **faixas disjuntas por `ajustar_sequencia`** — alfa em
1, beta em 1.000.000 — e que bastava documentá-lo como procedimento até haver
`inicio`/`passo` no esquema. A primeira medição confirmou: quatro inserções,
quatro linhas, nenhuma perda. Eu teria escrito «há remendo» e seguido.

Estava errado, e o erro tinha a forma clássica: **medi uma rodada e concluí sobre
o regime.**

## 3. O que a medição disse

Depois da **primeira ida e volta**, o contador de alfa estava em **1.000.002** —
que é o próximo número de **beta**. A causa é a regra certa aplicada onde ela não
serve: `RegFile::anotar_sequencia` (`reg.rs:758`) empurra o contador local para
depois de qualquer valor gravado à mão, e na replicação o valor «gravado à mão» é
o do outro servidor. Ou seja: **o mecanismo que faz `id=100` seguido de um
automático dar 101 num servidor só é o mesmo que destrói a faixa disjunta entre
dois.**

O terceiro estágio provou o regime: mais uma inserção de cada lado, e **6
inserções deixaram 5 linhas**. O remendo dura exatamente uma rodada de
sincronia.

O estágio 4 mediu a alternativa: a mesma prova com chave `Uuid` v7 — **4
inserções, 4 linhas, dos dois lados**. E mediu junto a fricção dela: a
`Sequence` nula ganha número, mas o `Uuid` nulo devolve *«coluna id e obrigatoria
e recebeu NULL»* — o cliente tem de mandar `"novo"`, e quem esquecer não grava.

## 4. A regra

**Remendo se mede no REGIME, não na primeira rodada** — e num sistema que troca
estado de volta, isso quer dizer *pelo menos duas idas e voltas*. Uma sincronia
só mede a partida.

E o corolário, que é o que dá para procurar: **quando uma regra local existe para
absorver o valor de fora, ela vira a inimiga de qualquer faixa reservada.**
Procure `anotar_*`, `max(...)` e `empurra` no caminho que a replicação também
percorre.

## 5. Como está guardado hoje

- A prova está no `bancada/sequencias/sonda.py`, bloco 24, em **quatro
  estágios** — mesma faixa, faixa disjunta, faixa disjunta depois da volta, e
  `Uuid` v7. Estágio único teria mentido, e é por isso que são quatro.
- Os números vão para `bancada/sequencias/resultados.json` com a data.
- O `docs/AUTONUMBER.md` §A.3 e §B.2.2 carregam a medição e a proposta
  (`inicio`/`passo` no `PSCH`, e não variável de servidor — a faixa tem de viajar
  com o arquivo, porque a restauração de backup devolve o contador tal como
  estava).
- **O buraco que fica:** o motor **não recusa** hoje subir em `multi` com duas
  origens de mesma faixa, porque não há faixa declarada para comparar. Enquanto
  o item B.2.2 não entrar, quem liga o bidirecional numa tabela com chave
  `Sequence` perde linha em silêncio — e a única defesa medida é usar chave
  `Uuid` v7.

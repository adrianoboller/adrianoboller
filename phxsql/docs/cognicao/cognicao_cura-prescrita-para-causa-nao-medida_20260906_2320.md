# Prescrevi uma cura para uma causa que eu mesmo marquei como NÃO MEDIDA

**06/09/2026, 23:20.** Descoberto ao ligar o `.fts` à tabela, três horas depois
de escrever o desenho que a medição de agora derruba.

## 1. O que aconteceu

Às 21:22 medi o custo de mais uma chave de índice na inserção, achei um
penhasco (9,05× com 15 índices) e escrevi no `docs/FTS.md` §4.3:

> *«a escrita síncrona por inserção não passa. O `.fts` entra por despejo em
> lote.»*

Medido agora, com o `FtsFile` de verdade:

- o custo real é **6,70×**, não 9,05×;
- o despejo em lote compra **0,94× / 0,90× / 0,92×** em lotes de 200, 1.000 e
  10.000 — **6 a 10%, e sem melhorar com lotes maiores.**

**As duas metades da prescrição estavam erradas: o número e a cura.**

## 2. O que eu concluí primeiro, e estava errado

Concluí que 14 chaves a mais custam o mesmo, venham elas de 14 árvores ou de
uma. Por isso medi com 15 índices separados — era mais fácil de montar — e
tratei o resultado como se valesse para o `.fts`.

E o agravante é que **eu mesmo já tinha escrito o contrário**, uma seção antes:
a §21.2 do `DESEMPENHO.md` nomeia a causa do penhasco como *«as páginas quentes
de 15 árvores deixam de caber no cache»*. Ou seja, o documento **dizia** que a
forma era o que decidia, e o número saiu dali para o desenho como se a forma
não importasse.

A segunda metade é pior, e é a que dá nome a este arquivo: eu marquei a causa
como **NÃO MEDIDA** — com todas as letras, em duas seções — e mesmo assim
prescrevi uma cura *que só funcionaria se a causa fosse aquela*. Cura para
causa não medida é palpite com cara de conserto: ela tem a forma de uma
decisão de engenharia, cita um número ao lado, e não foi testada contra nada.

## 3. O que a medição disse

| medida | µs/linha | × sobre A |
|---|---:|---:|
| A — só a tabela | 8,989 | 1,00 |
| B — + `.fts` linha a linha | 60,192 | **6,70** |
| C — + `.fts` em lote de 200 | 56,603 | 6,30 |

O custo é **~2,0 µs por chave**, e ele é do trabalho de pôr a chave na árvore.
Não há conserto barato no caminho de escrita.

O sinal que fecha o caso é a **insensibilidade ao tamanho do lote**: se o lote
estivesse consertando um problema de cache, lote maior compraria mais. Comprou
o mesmo. *Ganho que não responde ao parâmetro não vem do parâmetro.*

## 4. A regra

**Causa não medida não autoriza cura.** Quando um documento marca a causa como
«não medida», ele fica proibido de prescrever o conserto que depende dela — no
máximo lista a hipótese como uma das saídas, com o número que a confirmaria.

E o corolário da bancada: **a forma do medidor é parte da pergunta.** Medir
«14 chaves a mais» com 14 árvores em vez de uma é trabalho diferente, e o
número que sai não serve para decidir o desenho da outra forma.

## 5. Como está guardado hoje

- `custo-do-fts-de-verdade` mede a forma real, e o cabeçalho dele explica o
  erro da forma para quem for encolhê-lo de novo.
- `DESEMPENHO.md` §22 traz os dois números e a insensibilidade ao lote.
- `FTS.md` §4.1 reescrita, e a §4.3 que prescrevia o lote virou §4.4 — **as três
  saídas reais, com o número de cada uma, e a escolha é do dono**. O despejo em
  lote está lá como **recusado com número**, porque recusa medida impede a
  proposta de voltar. Inclusive a minha.
- **Sem guarda automática, e é decisão.** Não há como um conferidor saber se
  uma bancada tem a forma do que ela decide — isso é leitura, e foi lendo o meu
  próprio texto («15 árvores») ao lado do que eu ia construir («uma árvore»)
  que o erro apareceu. O que fica no lugar é a regra da §4, e o hábito de
  medir com a peça pronta em vez de por procuração.

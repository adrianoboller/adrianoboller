# Cognição: o conserto do contador do `.log` não porta para o `.reg`

**Descoberta:** 08/09/2026, ~13:10 UTC, na frente do time que consertou o split
do `.reg` heap.

## 1. O que aconteceu

O split do `.reg` (§2.2.1 do DESEMPENHO.md) achou duas idas ao núcleo POR LINHA:
o `stat` do `garantir` (~29%) e a gravação do cabeçalho de contadores (~22%). O
dono ativou o time. O `stat` do `garantir` virou conserto seguro (Fix 2, ~1,65×,
o `.reg inserir` direto de 4,45 para ~2,7 µs). O cabeçalho de contadores o DBA
**recusou** — e a recusa é a lição.

## 2. O que eu concluí primeiro, e estava errado

Que o cabeçalho por linha era «o irmão exato» do contador do `.log` que a §2.2 já
tinha resolvido: mover a gravação do cabeçalho para o `sincronizar`, como o `.log`
moveu o dele. Cheguei a escrever «irmão exato» no próprio DESEMPENHO.md antes de
medir. Estava casando pela **forma do custo** — um contador que a leitura sabe
recuperar, gravado toda linha quando poderia ir no fecho — sem olhar a **máquina
de recuperação** nem o **estado-vazio** dos dois arquivos.

## 3. O que a pesquisa disse

O `.log` tem `curar` (`log.rs:556`): ao reabrir, varre para a frente a partir do
`fim` gravado, validando cada evento pelo CRC, e para no primeiro que falha.
Funciona porque **todo byte gravado do `.log` é um evento real** — zero/lixo tem
tag inválida ou CRC que não bate, então a varredura para com segurança.

O `.reg` é outro bicho:

- Ele **confia** no `slot_count` do cabeçalho ao reabrir (`reg.rs` `montar`, byte
  20). **Não existe varredura de recuperação nenhuma** — nem no `montar`, nem no
  `reparar`, nem no `verificar`, que todos já assumem o `slot_count` como teto
  confiável.
- `STATUS_LIVRE = 0` é um estado vazio **legítimo**: um slot nunca-escrito
  (região zerada) e um slot escrito-e-depois-excluído têm o mesmo status, e o CRC
  nem é olhado quando o status é LIVRE. Não há o «todo byte é dado real» do `.log`.
- A paginação por balde/período faz o `slot_count` ser uma marca-d'água que
  **tolera buracos**. Uma varredura para a frente, ao topar no primeiro
  `STATUS_LIVRE`, não sabe se ali acabou tudo ou se é um buraco de exclusão com
  ATIVOs adiante.

Consequência medida contra o código: deferir a gravação do cabeçalho deixaria o
`slot_count` atrasado numa queda antes do `sincronizar`; a inserção seguinte
calcularia `rowid = slot_count + 1` com o valor velho e **gravaria por cima** de
slots já escritos e não contados — o mesmo estrago que o comentário do `.log`
descreve, e uma violação direta de «a ordem de digitação é sagrada».

## 4. A regra

**Antes de portar um conserto de um arquivo para o irmão, confira a MÁQUINA DE
RECUPERAÇÃO e o estado-vazio dele, não só a forma do custo. Um contador só se move
da linha para o `sincronizar` se (a) existir a varredura que o recupera na
reabertura E (b) o formato distinguir «gravado» de «vazio» sem ambiguidade.** O
`.log` tinha as duas; o `.reg` não tem nenhuma. Forma de custo igual não é
garantia igual.

## 5. Como está guardado hoje

Na §2.2.2 do DESEMPENHO.md, como recusa medida, com o número (~22%) e o motivo. A
Fix 2 landou; capturar o cabeçalho por linha com segurança vira uma frente
própria — projetar a recuperação de marca-d'água do `.reg` com seus testes de
queda, tratando a ambiguidade do `STATUS_LIVRE` no modo paginado —, não um efeito
colateral de mover uma linha.

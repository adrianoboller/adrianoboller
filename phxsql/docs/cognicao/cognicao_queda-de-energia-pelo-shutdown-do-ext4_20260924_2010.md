# A queda de energia se emula com o `shutdown` do ext4 — e o que a leva ao disco não é o que eu achei

**Estado:** PENDENTE

## O que aconteceu

O pedido 533 (a subida do byte 52 do `.ndx` sem `fsync`) só tinha prova por
emulação **no arquivo**: o papel C e o papel J devolviam o `.ndx` inteiro ao
último fecho com um `std::fs::write` por cima. A frente precisava da prova
contra o sistema operacional, e o `prova.sh` da `bancada/catastrofes/` só sabia
fazer o disco **recusar** (tmpfs cheio, provisionamento fino) — não sabia fazer
a máquina **cair** com o cache sujo. Não há `dmsetup` nesta máquina, então
`dm-flakey` e `dm-log-writes` estavam fora.

A receita que funcionou (`bancada/catastrofes/queda.py`, cenário `533`): ext4
sobre loop num tmpfs; `sync_file_range` só nos intervalos que o cenário quer no
disco (o `.reg` inteiro; e, no modo `paginas`, o `.ndx` a partir da página 1);
um `fsync` do `.reg`; e `FS_IOC_SHUTDOWN` (`0x8004587D`) com
`EXT4_GOING_FLAGS_NOLOGFLUSH` — o `godown` do xfstests: o sistema de arquivos
cai, o diário aborta, e o que estava sujo no cache não chega mais. Remonta e
confere.

## O que eu concluí primeiro, e estava errado

Raciocinei, sem medir, que o `LOGFLUSH` (flag 1) **abortaria** o commit — em
`data=ordered` o commit escreve os dados do inode que teve bloco alocado, e com
o `shutdown` já marcado o `writepages` devolveria `EIO` — e que, sem um `fsync`
do `.reg`, o tamanho novo dele se perderia na queda e o cenário ficaria
coerente (nada para detectar). Por isso a receita ganhou o `fsync` do `.reg` e
o `NOLOGFLUSH`, e o comentário do script afirmava que era o `fsync` que cometia
o tamanho.

## O que a medição disse

Uma rodada de cada, com o binário de antes do 533, 30.000 filhas sincronizadas
e 5.000 novas só fechadas, modo `nada`:

| variante | `.reg` no disco | `.ndx` no disco | byte 52 | `excluir` do pai com 5.000 filhas |
|---|---|---|---|---|
| `NOLOGFLUSH`, com `fsync` do `.reg` | 2.030.960 (novo) | 1.863.680 (tamanho novo, páginas velhas) | 0 | Ok, calado |
| `LOGFLUSH`, sem `fsync` do `.reg` | 2.030.960 | 1.863.680 | 0 | Ok, calado |
| `NOLOGFLUSH`, sem `fsync` do `.reg` | 2.030.960 | 1.863.680 | 0 | Ok, calado |

As duas hipóteses morreram: o `LOGFLUSH` não perdeu o tamanho, e sem o `fsync`
o `.reg` novo chegou do mesmo jeito. O mecanismo que cometeu o tamanho **não
está medido** — o que está medido é que o resultado não depende nem do `fsync`
nem da flag. E um efeito que eu não tinha previsto: o `.ndx` chega ao disco com
o **tamanho novo** e as páginas novas em zero, sob o cabeçalho velho — a árvore
velha continua coerente sozinha, e por isso o defeito sai calado.

Com a receita, a prova do 533 (3 formas × 3 rodadas): antes, **9/9** com byte
52 = 0 e o pai com filhas apagado calado; depois do `fdatasync` da subida,
**9/9** com byte 52 = 1 e o `excluir` recusando
(`bancada/catastrofes/resultados-533-antes.json`, `resultados-533.json`).

## A regra

Emule a queda de energia derrubando o sistema de arquivos com o cache sujo
(`FS_IOC_SHUTDOWN`), escolhendo por `sync_file_range` o que chegou — e meça
**cada** passo da receita contra o disco remontado antes de escrever por que ele
está ali.

## Como está guardado hoje

- `bancada/catastrofes/queda.py` e o cenário `533` do `prova.sh`, com o
  cabeçalho do script dizendo que o `fsync` do `.reg` é garantia e não causa.
- O que a receita **não** emula está escrito no script: o cache volátil do
  próprio disco reordenando escritas. Aqui a ordem é a do roteiro.
- **Buraco:** o mecanismo que cometeu o tamanho do `.reg` sem `fsync` não foi
  medido (candidatos: o commit periódico do jbd2, ou a conversão de extensão no
  fim da E/S do `sync_file_range`). Uma rodada de cada variante é pouco para
  mais do que «não depende».

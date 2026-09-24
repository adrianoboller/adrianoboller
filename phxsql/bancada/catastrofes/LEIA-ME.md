# O disco que recusa — e a máquina que cai —, contra o sistema operacional (pedidos 509, 512, 522 e 533)

```bash
sudo bancada/catastrofes/prova.sh              # constroi o executor e roda 3 rodadas
P=binario RODADAS=3 SAIDA=x.json bancada/catastrofes/prova.sh
P=binario-de-antes CENARIOS=522 SAIDA=antes.json bancada/catastrofes/prova.sh
P=binario CENARIOS=533 SAIDA=resultados-533.json bancada/catastrofes/prova.sh
python3 bancada/catastrofes/preco-do-522.py --binario target/release/phxsqld
```

Precisa de root e `unshare -m`; sem os dois diz **NÃO PROVADO** e sai com 2. Toda
montagem vive num espaço de montagem privado. O executor é o
`crates/phxsql-store/examples/disco-que-recusa.rs`; as receitas são as do
Apêndice A do `docs/propostas/parecer-dba-496-catastrofes-2026-09-24.md`, com
uma diferença: os dois fechos do 509 rodam no **mesmo processo**, como no
servidor — a recusa que o motor guarda não atravessa um `exec`.

Os testes de `crates/phxsql-store/tests/disco-que-recusa.rs` forjam a recusa
(EIO no `fsync`, ENOSPC na página); esta bancada faz o disco recusar de verdade.

## O que ela mede, e o que mediu em 24/09/2026 (núcleo 6.18.44, 3 rodadas)

| cenário | antes do conserto | depois |
|---|---|---|
| 512, tmpfs 512 KiB, 2º fecho no mesmo punho | byte 52 = **0**, `CRC inválido na página 3` (3/3) | byte 52 = 1, «reparar índice» (3/3) |
| 512, só o `Drop` (controle) | byte 52 = 1 (3/3) | byte 52 = 1 (3/3) |
| 509, provisionamento fino, dois fechos no mesmo processo | fecho 1 ENOSPC, fecho 2 **Ok** (3/3) | fecho 2 recusa, «pedido 509» (3/3) |
| 509, o gancho do servidor (`ABORTA=1`) | — | `abort` na 1ª recusa, saída 134 (3/3) |
| 522, provisionamento fino, `syncfs` com o disco cheio, fecha pelo `Drop` | byte 52 = **0**, `CRC inválido na página 3`, `precisa_reconstruir` falso (3/3) | byte 52 = 1, «reparar índice» (3/3) |
| 522, o controle: cai sem `Drop` | byte 52 = 1, «reparar índice» (3/3) | byte 52 = 1, «reparar índice» (3/3) |

E o **533**, medido na mesma data com o `queda.py` (ext4 sobre loop derrubado por
`FS_IOC_SHUTDOWN` sem descarregar o cache, depois de o roteiro escolher por
`sync_file_range` o que chegou ao disco), 30.000 filhas do cliente 1 sincronizadas
e uma janela de 5.000 só fechada pelo `Drop`:

| cenário 533 | antes (`resultados-533-antes.json`) | depois (`resultados-533.json`) |
|---|---|---|
| 5.000 filhas novas; o `.reg` chega, nada do `.ndx` | byte 52 = **0**, filhas do 2 = 0, `excluir` do pai = **Ok** (3/3) | byte 52 = 1, `precisa_reconstruir`, `excluir` recusa (3/3) |
| as mesmas; chegam o `.reg` e as páginas 1.. do `.ndx`, a 0 não | byte 52 = **0**, «página fora do arquivo» nas filhas do 1, `excluir` do pai = **Ok** (3/3) | byte 52 = 1, `precisa_reconstruir`, `excluir` recusa (3/3) |
| 5.000 filhas mudam de pai no mesmo slot; o `.reg` chega | byte 52 = **0**, `verificar` **OK**, `excluir` do pai = **Ok** (3/3) | byte 52 = 1, `precisa_reconstruir`, `excluir` recusa (3/3) |

O executor de antes é o mesmo `disco-que-recusa.rs` compilado com o `fdatasync` da
subida tirado de `NdxFile::levantar_marca` (a guarda `subida-do-byte-52-sem-fsync`
repõe exatamente isso). O que o `queda.py` **não** emula está no cabeçalho dele: o
cache volátil do próprio disco reordenando escritas.

A coluna «antes» do 522 é `resultados-522-antes.json`: o mesmo roteiro com
`CENARIOS=522` e o executor compilado sobre a árvore de antes do conserto
(`git archive` do `HEAD`, com este `disco-que-recusa.rs` por cima — o modo
`inserir-e-cai` nasceu junto).

**O que o conserto do 509 NÃO compra, medido nas mesmas rodadas:** depois de
remontar, o `.log` tem 0 de 5.000 eventos e o `.reg` tem `CRC não confere do
registro 20` — com e sem o conserto, e com o `abort`. O que o núcleo descartou
não volta sem diário de refazer; o conserto para de **confirmar** sobre um disco
que mente. O `.ndx`, que até o 522 saía daqui com byte 52 = 0 e `CRC inválido na
página 3` (o `fechar` do processo que inseriu baixava a marca sem `fsync`), hoje
sai com byte 52 = 1 e manda reconstruir.

## O preço do 522, contra o `phxsqld` de pé

`preco-do-522.py` sobe o servidor de release, semeia um banco de exemplo, dá
`SIGKILL` sob carga em rodízio pelas tabelas e conta, pelo byte 52 lido do
arquivo, quantas ficaram marcadas; o arranque seguinte diz, no relatório de
recuperação, quantas reconstruiu e em quanto tempo. O controle na mesma rodada
é o `SIGKILL` com o servidor ocioso, depois de o relógio fechar a janela: tem de
marcar zero. `preco-do-522.json` (8 tabelas × 20.000 linhas) e
`preco-do-522-200-tabelas.json` (200 × 1.000) são as corridas de 24/09/2026; os
números estão no `docs/FORMATO.md`, § a marca de sujo.

`resultados.json` é a corrida com o conserto, linha a linha, com a data.

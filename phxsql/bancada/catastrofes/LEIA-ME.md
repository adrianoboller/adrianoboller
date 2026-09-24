# O disco que recusa, contra o sistema operacional (pedidos 509 e 512)

```bash
sudo bancada/catastrofes/prova.sh              # constroi o executor e roda 3 rodadas
P=binario RODADAS=3 SAIDA=x.json bancada/catastrofes/prova.sh
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

**O que o conserto NÃO compra, medido nas mesmas rodadas:** depois de remontar,
o `.log` tem 0 de 5.000 eventos e o `.ndx` tem byte 52 = 0 com `CRC inválido na
página 3` — com e sem o conserto, e com o `abort`. O byte 52 já estava em 0
**antes** de qualquer fecho: quem o baixou foi o `fechar` do processo que
inseriu, que grava o cabeçalho sem `fsync` por desenho. O que o núcleo descartou
não volta sem diário de refazer; o conserto para de **confirmar** sobre um disco
que mente.

`resultados.json` é a corrida com o conserto, linha a linha, com a data.

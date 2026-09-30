# Cortar o futuro não mata o processo: o prazo tem de viajar até o filho

**Estado:** FRUTÍFERO

**Evidência:** `bbc0e99`

## O que aconteceu

O motor aplicava `tokio::time::timeout` na ferramenta de shell. O teste passava (a tarefa
seguia em 300 ms), mas a suíte levava 5,04 s: o `sleep 5` continuava vivo no sandbox, porque
o `spawn_blocking` não é cancelado quando o futuro é descartado.

## O que eu concluí primeiro, e estava errado

Que o teste verde provava o timeout. Ele provava só que o motor parava de esperar.

## O que a medição disse

Com o prazo no `ToolContext` repassado ao processo: suíte em 0,32 s, e o teste confere com
`pgrep` que nenhum `sleep 5` sobrou.

## A regra

Timeout de ferramenta que lança processo se aplica NO processo; cortar só o futuro deixa órfão.

## Como está guardado hoje

`ToolContext.timeout` em `phxclaw-agent-core` e `ferramenta_lenta_vira_timeout_e_o_agente_segue`.

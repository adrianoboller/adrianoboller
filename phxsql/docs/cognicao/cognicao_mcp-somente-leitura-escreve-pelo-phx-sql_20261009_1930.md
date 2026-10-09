# A ponte MCP «somente de leitura» escreve pela ferramenta `phx_sql`

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 19:30
- **Onde:** `crates/phxsql-server/src/mcp.rs` (`Ponte::ferramentas`),
  `crates/phxsql-server/src/catalogo.rs` (`Operacao::escreve`)
- **Pedido:** 765, fatia P5 (achado da prova, não consertado nesta fatia)

## O que aconteceu

A ponte MCP nasce somente de leitura e esconde as ferramentas que escrevem. O critério
é `OPS_ESCRITA.contains(nome)`. A op `sql` não está em `OPS_ESCRITA`, porque um `SELECT` não
escreve. Por isso `phx_sql` aparece na ponte de leitura e leva `DELETE`, `UPDATE`, `INSERT` e
`DROP VIEW` ao `despachar`.

## O que eu concluí primeiro, e estava errado

Que a prova do MCP na P5 seria só repetir a do REST com outra porta. Para uma ponte somente
de leitura, eu esperava que o `DELETE` pelo MCP nem chegasse à camada de proteção.

## O que a medição disse

`o_mcp_passa_pela_mesma_camada`, corrida com a camada desligada: o
`DELETE FROM c WHERE id > 0` pela ponte somente de leitura responde `afetadas: 1500`. Com a
camada ligada, ele é recusado com a 4009 porque o plano é largo. Abaixo do piso de 1.000
linhas, o `DELETE` pela ponte de leitura passaria mesmo com a camada ligada. Este último caso
foi deduzido do piso, não exercitado.

## A regra

Uma op que traduz texto para outras ops não se classifica pelo nome dela. Se a fronteira é
«não escreve», ela tem de ler a op derivada, como faz a camada de proteção.

## Como está guardado hoje

Não está guardado. Só o `DELETE`/`UPDATE` largo e a lista de perigo são recusados pela
camada. A escrita pequena pela ponte de leitura continua aberta.

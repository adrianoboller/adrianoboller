# Id de sessao vindo do cliente vira nome de pasta e colapsa pelo `safe_id`

**Estado:** FRUTÍFERO (evidência: `crates/phxclaw-agent/tests/n8n.rs::mcp_client_tool_do_n8n_fala_com_o_servir_por_streamable_http`
falhava com o defeito — `assertion failed: ...workdir("mcp-sessao-n8n-1").join("nota.txt").is_file()` —
e passa com o conserto; o mesmo teste prova que `../../etc` ganha sessão nova)

## O que aconteceu

O `POST /mcp` (streamable HTTP para o MCP Client Tool do n8n, SP000033) precisava de uma pasta de
trabalho e uma evidência por sessão. Usei o `Mcp-Session-Id` que o cliente manda, confinado a
`[A-Za-z0-9_-]{1,64}`, como id no `TaskStore`. O teste gravou `nota.txt` pela sessão
`sessao-n8n-1` e não a achou em `tasks/mcp-sessao-n8n-1/work`.

## O que eu concluí primeiro, e estava errado

Que bastava recusar `/` e `..` no id para ele servir de nome de pasta. O `TaskStore::safe_id`
não recusa: ele **filtra**, guardando só hex e `-` — `mcp-sessao-n8n-1` virou `c-ea-8-1`. Dois
clientes com ids diferentes (`sessao-n8n-1` e `sessao-n8n-2`) cairiam na MESMA pasta
(`c-ea-8-1` e `c-ea-8-2` ainda diferem, mas `abc` e `xyz` viram `abc` e vazio), e um deles
leria o `nota.txt` do outro.

## O que a medição disse

`find /tmp/phx-n8n-api-*/tasks` mostrou a pasta `c-ea-8-1`. Conserto: o servidor nomeia a
sessão (UUID v7) no `initialize` e devolve no cabeçalho; o cliente a repete, como o SDK do n8n
faz; qualquer id sem forma de UUID (hex e `-`) ganha sessão nova. 8/8 testes em 0,19 s.

## A regra

Id que vira caminho é o servidor que cunha, nunca o cliente que escolhe. Se o cliente precisa
repetir um id, ele repete o que o servidor cunhou, e o servidor só aceita a forma que cunhou.

## Prevenção

Antes de usar um texto de fora como id do `TaskStore`, ler o `safe_id`: ele filtra, não recusa —
e filtrar é o que colapsa ids distintos.

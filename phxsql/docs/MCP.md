# Servidor MCP

O pedido 6 dizia: *«o protocolo já é JSON por linha; falta a tradução de
vocabulário»*. Estava certo, e é isso que existe agora em
`crates/phxsql-server/src/mcp.rs`.

O MCP fala JSON-RPC 2.0, uma mensagem por linha. O PhxSql fala JSON, uma
mensagem por linha. **Não faltava transporte nem formato.** O que faltava era
dizer que `tools/call` com `name:"phx_ler"` é `{"op":"ler"}` daqui, e devolver
a resposta no envelope que o MCP espera.

---

## A decisão que manda em todas as outras

A ponte **não executa nada**. Ela recebe um `Executor`, que é o `despachar` do
servidor, e todo pedido MCP passa pelos **mesmos quatro portões** de um cliente
pela porta 5000: política, token, login, e permissão por base e por tabela.

A alternativa — a ponte chamar `Table` direto, «que é mais rápido» — criaria um
segundo caminho até o dado. E o segundo caminho é sempre o que esquece uma
conferência. A regra já estava escrita no projeto: *o portão que alguém
esquecer vira a porta dos fundos, e ninguém acha por leitura*.

## Somente leitura vem ligado

`Ponte::nova` nasce recusando `phx_inserir` e `phx_atualizar`. Liberar é
`com_escrita(true)` — uma decisão com nome, e não um campo com padrão.

É a mesma regra do DbLink, e aqui vale com mais razão: **do outro lado desta
ponte há um modelo de linguagem, não uma pessoa.** Quando recusa, a ponte diz
*por que* recusou — «a ferramenta escreve, e esta ponte é somente de leitura» —
em vez de fingir que ela não existe, que deixaria o modelo tentando de novo.

## O token não é argumento

O que a ponte carimba em todo pedido (`token`, tipicamente) mora na ponte, e
argumento com o mesmo nome é **descartado**. Se o token fosse argumento, quem
escolheria a credencial seria o modelo. O mesmo vale para o `op`: um argumento
`"op":"excluir_tabela"` numa chamada a `phx_bancos` não muda a operação.

## As duas armadilhas do protocolo

**Notificação não recebe resposta.** Mensagem sem `id` é notificação, e
responder a ela quebra o cliente — logo no `notifications/initialized`, que é a
primeira coisa que ele manda depois do aperto de mão. `atender` devolve `None`,
e o silêncio é a resposta certa.

**Falha de execução não é erro de JSON-RPC.** Tabela que não existe, permissão
negada: isso volta como *resultado*, com `isError: true` e o texto do erro. É a
diferença que o MCP faz de propósito — assim o modelo **lê** o erro e corrige,
em vez de o cliente abortar a conversa. O erro do JSON-RPC fica para o que é
problema de protocolo: método inexistente, ferramenta não anunciada, argumento
obrigatório faltando.

## As ferramentas

Um catálogo, uma linha por ferramenta — e nenhum código por ferramenta.
`tools/list` e `tools/call` leem a **mesma** tabela: com duas listas, a que
alguém acrescentasse num lugar e esquecesse no outro viraria uma ferramenta que
o modelo enxerga e não consegue chamar, ou uma que ele chama sem estar
anunciada.

| ferramenta | `op` | escreve |
|---|---|---|
| `phx_bancos` | `bancos` | |
| `phx_tabelas` | `tabelas` | |
| `phx_esquema` | `esquema` | |
| `phx_ler` | `ler` | |
| `phx_varrer` | `varrer` | |
| `phx_buscar` | `buscar` | |
| `phx_dados_pessoais` | `dados_pessoais` | |
| `phx_inserir` | `inserir` | sim |
| `phx_atualizar` | `atualizar` | sim |

Os nomes levam `phx_` porque um cliente MCP junta as ferramentas de vários
servidores no mesmo espaço de nomes.

Há um teste que percorre o catálogo e exige que **toda `op` anunciada tenha uma
atividade de permissão**: uma que a tabela de permissões não conhecesse seria
uma ferramenta sem portão.

## O que ainda não tem

- **Transporte.** Este módulo é a tradução; quem lê de `stdin` (ou de um
  soquete) e chama `Ponte::atender` ainda não existe. Foi feito nesta ordem
  porque a tradução é a parte que se testa sem abrir processo nenhum — e são
  14 testes contra zero processo.
- `resources/*` e `prompts/*`. Uma tabela dá um belo *resource*, e é outra
  rodada.
- **Amostragem e paginação de resultado grande.** Hoje o `phx_varrer` devolve
  o que o `limite` pedir, e o teto é o do servidor.

---

MCP é o Model Context Protocol; JSON-RPC 2.0 é a especificação de fio que ele
usa.

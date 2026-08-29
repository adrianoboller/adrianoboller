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

Uma lista só, e ela não mora aqui: mora em `crates/phxsql-server/src/catalogo.rs`,
junto com todas as outras operações do protocolo. `tools/list` e `tools/call`
leem a **mesma** tabela: com duas listas, a que alguém acrescentasse num lugar e
esquecesse no outro viraria uma ferramenta que o modelo enxerga e não consegue
chamar, ou uma que ele chama sem estar anunciada.

| ferramenta | `op` | escreve |
|---|---|---|
| `phx_bancos` | `bancos` | |
| `phx_tabelas` | `tabelas` | |
| `phx_esquema` | `esquema` | |
| `phx_ler` | `ler` | |
| `phx_varrer` | `varrer` | |
| `phx_buscar` | `buscar` | |
| `phx_sql` | `sql` | |
| `phx_dados_pessoais` | `dados_pessoais` | |
| `phx_inserir` | `inserir` | sim |
| `phx_atualizar` | `atualizar` | sim |

O nome é **derivado** — `phx_` mais a `op` —, e não digitado: assim o nome
anunciado e a operação chamada não têm como divergir. O prefixo existe porque um
cliente MCP junta as ferramentas de vários servidores no mesmo espaço de nomes.

Há um teste que percorre o catálogo e exige que **toda `op` anunciada tenha uma
atividade de permissão**: uma que a tabela de permissões não conhecesse seria
uma ferramenta sem portão.

## O que ainda não tem

- `resources/*` e `prompts/*`. Uma tabela dá um belo *resource*, e é outra
  rodada.
- **Amostragem e paginação de resultado grande.** Hoje o `phx_varrer` devolve
  o que o `limite` pedir, e o teto é o do servidor.
- **Transporte por soquete ou HTTP.** Só stdio, que é o que um cliente MCP
  local usa.

---

MCP é o Model Context Protocol; JSON-RPC 2.0 é a especificação de fio que ele
usa.

---

## O transporte, e o que ele ensinou

`phxsqld --mcp` lê uma mensagem JSON-RPC por linha da entrada padrão e escreve
uma resposta por linha na saída. É o que faltava: a tradução era testável sem
processo nenhum e foi feita primeiro, mas sem transporte nenhum cliente MCP
falava com o servidor.

```bash
phxsqld --mcp                                  # pelo token de serviço
PHXSQL_SENHA='a senha' phxsqld --mcp --usuario adriano
phxsqld --mcp --escrita                        # libera phx_inserir/phx_atualizar
```

**A senha vem do ambiente, não do argumento** — pela mesma razão do `--senha`:
argumento aparece no `ps` e fica no histórico do shell. E a entrada padrão aqui
está ocupada pelo protocolo, então sobra a variável.

**Sem `--usuario`, a ponte fala pelo token de serviço.** Num servidor sem
cadastro isso é poder total; num servidor com cadastro não passa do `ping`, e a
ponte avisa no `stderr` — que é onde o aviso pode sair sem sujar o cano do
protocolo.

### O executor local serializa o pedido para reanalisá-lo, e isso é de propósito

`ExecutorLocal` escreve o pedido como texto e chama `despachar`, que é onde
moram os quatro portões. Chamar `executar` direto pularia todos eles e seria o
segundo caminho até o dado. O custo é um `escrever` mais um `analisar` por
chamada; do outro lado há um modelo fazendo uma pergunta por vez, e não uma
carga de cinco mil linhas.

A sessão é **uma só e viva entre chamadas**: o `login` é uma operação como
qualquer outra, e sem guardar a sessão cada `tools/call` chegaria anônimo.

E toda chamada entra no **log de acessos**, com `stdio` no lugar do IP. Não há
endereço, mas leitura pelo MCP que não deixa rastro seria um buraco na
auditoria justamente na origem mais nova.

### O que eu escrevi errado sobre o `flush`, e o que a medição disse

Escrevi no comentário que sem o `flush` a resposta ficaria presa no cano,
porque a saída de um processo é *block-buffered*. **Tirei o `flush` e o teste
passou igual.** O `Stdout` do Rust é um `LineWriter`: ele descarrega sozinho no
`\n`.

O `flush` ficou, mas pelo motivo verdadeiro e menor: `servir` é genérica sobre
`Write`, e um `BufWriter` — ou o dia em que alguém envolver a saída num — não
tem essa cortesia. *Diagnóstico plausível não é diagnóstico medido*, e o errado
sobrevive melhor quando a linha que ele justifica funciona por outro motivo.

O defeito que o teste do processo **realmente** pega é outro, e é maior: trocar
o laço por `lines().collect()` — que é o que se escreve sem pensar — faz o
servidor ler tudo antes de responder. Um cliente MCP manda uma mensagem e
*espera*, com a entrada aberta: os dois lados travam esperando um ao outro, sem
erro em lugar nenhum. Medido: com esse defeito o teste
`a_resposta_sai_antes_de_a_entrada_fechar` pendura até o tempo estourar.

### As ferramentas saem do catálogo

A tabela de nove ferramentas escrita à mão neste módulo **não existe mais**.
`tools/list` e `tools/call` leem `crate::catalogo` — a mesma lista que a op
`catalogo` do protocolo e o `/help` do `phxsqlcmd`, e que tem um teste
comparando-a com o `match` do `despachar`.

O que isso mudou na prática:

- o nome da ferramenta é derivado (`phx_` + a `op`), e não digitado — não há
  mais como o nome anunciado e a operação chamada divergirem;
- `escreve` sai de `OPS_ESCRITA`, a mesma lista que o modo somente-leitura do
  servidor usa;
- a descrição que o modelo lê agora traz o **exemplo** junto — um pedido
  inteiro que funciona vale mais que qualquer frase para quem tem de montar a
  chamada, e ele já vem com um teste que o confere;
- entrou uma décima ferramenta de graça: `phx_sql`.

Os quinze testes que já existiam aqui passaram sem uma linha de mudança de
comportamento — é a prova do lado que mais importa numa troca dessas.

Uma nota que o catálogo obrigou a escrever: **`aplicar` grava e não está em
`OPS_ESCRITA`.** A ausência é deliberada (uma réplica em somente-leitura precisa
aplicar), e por isso ele não pode virar ferramenta MCP — a ponte somente-leitura
o ofereceria achando que ele só lê. Há um teste travando exatamente isso.

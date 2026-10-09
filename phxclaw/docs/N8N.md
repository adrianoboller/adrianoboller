# PhxClaw e n8n: integracao pelos padroes abertos

Ordem do dono, 02/10/2026: *«Importante total integracao do PhxClaw com N8n».*
Decisao anterior do pesquisador (R12, `docs/absorcao/TRIAGEM_PESQUISAS_2026-10-01.md`):
o n8n **nao se embute** -- a Sustainable Use License permite uso interno e redistribuicao
gratuita, nao embutir num produto -- e a integracao e pelos padroes que o proprio n8n
documenta: webhook, API publica, MCP e no da comunidade.

Tudo abaixo foi lido na documentacao do n8n em **02/10/2026** (versao publicada no
registro npm nesse dia: `n8n` **2.41.5**, `n8n-workflow` **2.16.0**). As fontes estao em
cada secao, com a URL `.md` que a documentacao oferece.

## 1. Os dois sentidos, em uma tabela

| Sentido | Como | Onde no PhxClaw | Prova |
| --- | --- | --- | --- |
| PhxClaw -> n8n: disparar fluxo | `n8n_workflow {action: run}` -> `POST /webhook/<caminho>` do no Webhook (ou `/webhook-test/`), corpo JSON, assinado | `crates/phxclaw-agent/src/n8n.rs` | `tests/n8n.rs::run_assina_o_corpo_com_o_mesmo_hmac_do_canal_de_webhook` |
| PhxClaw -> n8n: listar/acompanhar | `n8n_workflow {action: list\|status}` -> `GET /api/v1/workflows`, `/api/v1/executions[/id]` com `X-N8N-API-KEY` | idem | `tests/n8n.rs::list_e_status_pedem_a_chave_do_broker_e_nunca_a_devolvem` |
| PhxClaw -> n8n: fluxos como ferramentas | o MCP Server Trigger do n8n e um servidor MCP por streamable HTTP/SSE: entra pela configuracao de MCP que ja existe (`url`) | `crates/phxclaw-agent/src/mcp.rs` (cliente) | `tests/n8n.rs::mcp_server_trigger_do_n8n_vira_ferramenta_do_agente` |
| n8n -> PhxClaw: tarefas | no da comunidade `n8n-nodes-phxclaw` (ou o no HTTP Request) na API `/v1/tasks` | `integracoes/n8n-nodes-phxclaw/` | `npm run build` passa; nao exercitado num n8n (secao 6) |
| n8n -> PhxClaw: ferramentas | MCP Client Tool do n8n contra `POST /mcp` do `phxclaw servir` (streamable HTTP, Bearer da API) | `mcp::rotas` em `crates/phxclaw-agent/src/mcp.rs` | `tests/n8n.rs::mcp_client_tool_do_n8n_fala_com_o_servir_por_streamable_http` |
| n8n -> PhxClaw: gatilho | no HTTP Request -> `POST /v1/triggers/<nome>` com `X-PhxClaw-Segredo` (Header Auth) ou HMAC com carimbo | `crates/phxclaw-agent/src/gatilhos.rs` | `tests/n8n.rs::webhook_do_n8n_dispara_o_gatilho_por_segredo_ou_por_hmac` |

A bateria `cargo test -p phxclaw-agent --test n8n`: **8 testes, 0,19 s**, contra um
servidor HTTP falso que imita o n8n com as respostas da documentacao da API publica
(esquemas de `/workflows` e `/executions`) e um MCP Server Trigger que responde SSE, como
o SDK do n8n faz por padrao. RED -> GREEN registrado na entrega: modulo inexistente;
`/mcp` 404; gatilho recusando HMAC com 401.

## 2. Configurar (o que o dono precisa dar)

Tres valores, todos do n8n dele:

| Chave do catalogo | Variavel | Onde mora | Para que |
| --- | --- | --- | --- |
| `n8n.url` | `PHXCLAW_N8N_URL` | `config.json` ou ambiente | a origem do n8n (`http://127.0.0.1:5678`, `https://n8n.exemplo.com`). Sem ela a ferramenta `n8n_workflow` **nem existe**. HTTP sem TLS so em loopback. |
| `n8n.chave` | `PHXCLAW_N8N_API_KEY` | SecretBroker, por `phxclaw n8n chave` | `list` e `status` (API publica; Settings > n8n API > Create an API key). |
| `n8n.webhook_segredo` | `PHXCLAW_N8N_WEBHOOK_SEGREDO` | SecretBroker, por `phxclaw n8n segredo` | assina o `run` e, como `segredo` em `gatilhos.json`, confere o que o n8n devolve. |

```sh
export PHXCLAW_N8N_URL=http://127.0.0.1:5678
PHXCLAW_N8N_API_KEY=... phxclaw n8n chave
PHXCLAW_N8N_WEBHOOK_SEGREDO=... phxclaw n8n segredo
phxclaw servir   # a ferramenta n8n_workflow aparece; conceda `automacao.n8n` em agente.capacidades
```

`automacao.n8n` fica **fora do padrao** e e classificada como escrita: `run` dispara
trabalho noutro sistema; no Plan Mode ela nao aparece.

Fonte da API: https://docs.n8n.io/connect/n8n-api/authentication.md (cabecalho
`X-N8N-API-KEY`), https://docs.n8n.io/connect/n8n-api/pagination.md (`limit` ate 250,
`nextCursor`), https://docs.n8n.io/connect/n8n-api/executions.md e
https://docs.n8n.io/connect/n8n-api/workflow.md (os esquemas usados nas fixtures).

## 3. PhxClaw -> n8n: a ferramenta `n8n_workflow`

- **`run`**: `path` e o caminho que o no Webhook mostra (`pedido`, `/webhook/pedido`;
  `test: true` vai para `/webhook-test/`), `body` e um objeto JSON (teto 64 KiB). O
  caminho e normalizado e **nunca sai de `/webhook/`**: `..`, `?`, `#` e espaco se
  recusam antes de qualquer pedido. A resposta e o que o n8n devolve ("Respond:
  Immediately" -> `{"message":"Workflow was started"}`; "When Last Node Finishes" -> os
  dados do ultimo no).
- **Autenticacao do `run`** (`auth`): `hmac` (padrao) manda `X-PhxClaw-Carimbo` e
  `X-PhxClaw-Assinatura: sha256=HMAC-SHA256(segredo, carimbo + "." + corpo)` -- a MESMA
  assinatura do canal de webhook (`canais/webhook.rs`), conferida no n8n por um no Code
  (exemplo em `integracoes/n8n/exemplos/phxclaw-dispara-n8n.json`); `header` manda o
  segredo em claro em `X-PhxClaw-Segredo`, para o **Header Auth** nativo do no Webhook
  (sem codigo, sem janela de replay); `none` nao manda nada. Sem segredo guardado, o
  `run` sai sem assinatura: webhook e a porta publica do fluxo.
- **`list`**: `active`, `name`, `limit`, `cursor`; devolve `{id, name, active,
  updatedAt}` por fluxo (sem os nos: o modelo nao precisa deles).
- **`status`**: `execution_id`, ou `workflow_id` + `status` + `limit`; devolve `{id,
  status, mode, workflowId, startedAt, stoppedAt}`.
- A chave e o segredo saem do broker por concessao curta, e **todo erro passa pelo
  scrub**: o teste faz o n8n falso ecoar a chave no erro e confere que ela nao volta.

Fontes: https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.webhook.md
(URLs de teste/producao, Respond, Basic/Header/JWT auth).

## 4. n8n -> PhxClaw

### 4a. Tarefas: o no da comunidade `n8n-nodes-phxclaw`

`integracoes/n8n-nodes-phxclaw/` (TypeScript, pacote npm, **fora** do workspace Rust: a
pétrea de zero dependencias e do binario). Credencial `PhxClaw API` = URL do `phxclaw
servir` + Bearer da API. Operacoes: *Create Task* (com espera opcional), *Wait for
Result*, *Answer Question*, *Approve Plan*, *Run Tool* (um `tools/call` em `POST /mcp`).
Cada operacao e UMA rota da API: o no nao reimplementa nada.

Medido em 02/10/2026: `npm install` das deps de desenvolvimento = **96 pacotes, 87 MB,
6 s**; `npm run build` (tsc + copia do icone) passa.

**Licenca, com a fonte.** Os nos da comunidade sao pacotes npm do autor
(https://docs.n8n.io/integrations/community-nodes/building-community-nodes.md: nome
`n8n-nodes-*`, palavra-chave `n8n-community-node-package`, campo `n8n` no
`package.json`, publicados no registro; a verificacao pelo Creator Portal pede ainda
publicacao por GitHub Actions com provenance e zero dependencias de execucao). O pacote
aqui e Apache-2.0, como o resto do PhxClaw; o `n8n-workflow` (Sustainable Use License)
entra so como dependencia de desenvolvimento, pelos tipos. O que a licenca do n8n
permite e proibe esta em https://docs.n8n.io/n8n-community-license/community-license.md
e no FAQ (https://docs.n8n.io/n8n-community-license/community-license/license-faq.md):
usar o n8n atras do proprio produto e permitido; deixar o cliente **construir fluxos**
pelo nosso produto (UI, API, MCP ou agente agindo por ele) **nao e** -- e esse e o limite
que a integracao respeita: o PhxClaw dispara e acompanha fluxos que o operador construiu.

Instalacao no n8n do dono: Settings > Community Nodes (GUI), ou
`cd ~/.n8n/nodes && npm i n8n-nodes-phxclaw`
(https://docs.n8n.io/integrations/community-nodes/installation-and-management/manual-installation.md),
ou por variavel desde o n8n 2.21 (`N8N_COMMUNITY_PACKAGES_MANAGED_BY_ENV=true`,
`N8N_COMMUNITY_PACKAGES='[{"name":"n8n-nodes-phxclaw"}]'`). Pacote nao verificado pede
`N8N_UNVERIFIED_PACKAGES_ENABLED=true` ou um `checksum`. Em desenvolvimento,
`N8N_CUSTOM_EXTENSIONS=<pasta do pacote compilado>`.

### 4b. Ferramentas: MCP Client Tool -> `POST /mcp`

O `phxclaw servir` passou a expor o MESMO `responder` do `phxclaw mcp-serve` por
**streamable HTTP** em `POST /mcp`, com o Bearer da API -- o transporte que o MCP Client
Tool do n8n aceita (SSE ou streamable HTTP; nunca stdio:
https://docs.n8n.io/integrations/builtin/cluster-nodes/sub-nodes/n8n-nodes-langchain.toolmcp.md).
No n8n: endpoint `http://<phxclaw>:8787/mcp`, autenticacao *Bearer* com o token da API.

O que o endpoint faz e nao faz: uma mensagem JSON-RPC por pedido, resposta
`application/json` (o streamable HTTP aceita JSON no lugar do SSE quando a resposta e
uma so); notificacao e 202 sem corpo; `GET` e 405 (o servidor nao inicia fluxo);
`DELETE` e 204. O `Mcp-Session-Id` sai do servidor no `initialize` (um UUID) e o cliente
o repete; ele e a pasta de trabalho (`tasks/<sessao>/work`) e a evidencia da sessao, e
**so a forma de UUID vale** -- qualquer outra coisa ganha sessao nova, porque o id vira
nome de pasta. O agente nasce por pedido pela mesma montagem de uma tarefa, com o modelo
padrao: o portao de capacidade e o de sempre (`shell` sem `shell.exec` volta NEGADO, como
o teste prova).

### 4c. Fluxos do n8n como ferramentas: MCP Server Trigger -> cliente MCP do PhxClaw

O MCP Server Trigger do n8n expoe uma URL (`.../mcp/<caminho>`; teste e producao) com
autenticacao None, Bearer ou Header
(https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-langchain.mcptrigger.md).
Para o PhxClaw ele e um servidor MCP por `url`, na configuracao que ja existia
(`PHXCLAW_MCP_CONFIG`, exemplo em `integracoes/n8n/exemplos/mcp.json`):

```json
{"servidores": [{"nome": "n8n", "url": "http://127.0.0.1:5678/mcp/phxclaw", "auth": {"tipo": "bearer"}}]}
```

`PHXCLAW_MCP_TOKEN=... phxclaw mcp token n8n` guarda o Bearer no broker. Cada ferramenta
do fluxo vira `mcp__n8n__<nome>`, capacidade `mcp.n8n` (fora do padrao, como toda
`mcp.*`). O SDK do n8n responde ao POST com `text/event-stream`; o cliente do runtime
(`McpStreamableHttpClient::read_sse_response`) ja lia isso, e o teste prova com um
servidor falso que responde SSE.

### 4d. Gatilho: webhook do n8n -> `POST /v1/triggers/<nome>`

Ja existia (`gatilhos.json`, `webhooks[].segredo`, cabecalho `X-PhxClaw-Segredo`). O que
entrou: o gatilho passou a aceitar tambem `X-PhxClaw-Carimbo` + `X-PhxClaw-Assinatura`
sobre o corpo cru -- a MESMA assinatura do `run` e do canal de webhook, com a janela de 5
minutos --, para o mesmo segredo e o mesmo no Code do n8n servirem aos dois sentidos. O
caminho irmao (o Bearer da API) nao mudou. Exemplo: `integracoes/n8n/exemplos/gatilhos.json`
e o no "Devolver ao PhxClaw" de `phxclaw-dispara-n8n.json`.

## 5. Fluxos de exemplo (`integracoes/n8n/exemplos/`)

| Arquivo | O que faz |
| --- | --- |
| `n8n-chama-phxclaw.json` | Webhook `phxclaw-tarefa` -> no PhxClaw (*Create Task*, espera) -> Respond to Webhook com `{id, status, answer, error}` |
| `phxclaw-dispara-n8n.json` | Webhook `pedido` (o que o `n8n_workflow run` dispara) -> Code confere o HMAC -> HTTP Request devolve o resultado em `POST /v1/triggers/n8n-resultado` |
| `gatilhos.json` | o gatilho `n8n-resultado` para a pasta do agente |
| `mcp.json` | o MCP Server Trigger do n8n como servidor MCP do agente |

**Os JSON foram escritos a mao a partir da documentacao dos nos e nao foram importados
num n8n de verdade** (secao 6). Os nomes de tipo (`n8n-nodes-base.webhook` v2,
`n8n-nodes-base.code` v2, `n8n-nodes-base.httpRequest` v4.2,
`n8n-nodes-base.respondToWebhook` v1.1, `n8n-nodes-phxclaw.phxClaw` v1) sao os
documentados; a primeira importacao pode pedir ajuste de versao de no.

## 6. Prova real com n8n de verdade: NAO VALIDADA, com o motivo medido

Tentado em 02/10/2026 nesta maquina: `npm install n8n@2.41.5 --ignore-scripts` numa
pasta do scratchpad, com **4.335 MB** livres antes. O n8n tem 156 dependencias
diretas; a instalacao chegou a **2.162 MB em `node_modules` + 1.245 MB no cache do
npm, ainda incompleta**, com o disco em **436 MB livres** -- abaixo do piso de 1 GB que a
ordem fixou. Foi interrompida e apagada (o disco voltou a 3.528 MB). Ou seja: o n8n
inteiro **nao cabe** no disco desta maquina ao lado do `target/` do workspace.

O que fica provado e o que nao fica:

- **Provado por teste**: os seis caminhos da tabela da secao 1, contra o servidor falso
  com as respostas da documentacao, e o `/mcp` e o gatilho contra a API de verdade numa
  porta local. O no da comunidade compila.
- **Nao provado**: um n8n real importando os JSON, o MCP Client Tool real falando com o
  `/mcp`, o no da comunidade carregado por um n8n real. Nada aqui se declara exercitado.

Para validar na maquina do dono (ou em qualquer uma com >= 6 GB livres), o roteiro:

```sh
# 1. n8n local com o no compilado
cd integracoes/n8n-nodes-phxclaw && npm install && npm run build
N8N_CUSTOM_EXTENSIONS=$PWD N8N_PORT=5678 npx n8n start
# 2. PhxClaw com o n8n declarado
export PHXCLAW_N8N_URL=http://127.0.0.1:5678
PHXCLAW_N8N_API_KEY=<chave de Settings > n8n API> phxclaw n8n chave
PHXCLAW_N8N_WEBHOOK_SEGREDO=<segredo> phxclaw n8n segredo
cp integracoes/n8n/exemplos/gatilhos.json ~/.phxclaw/   # troque o segredo
phxclaw servir
# 3. importar os dois JSON de integracoes/n8n/exemplos/ no n8n, criar a credencial
#    "PhxClaw API" (URL http://127.0.0.1:8787 + token de `phxclaw api chave`), ativar.
# 4. n8n -> PhxClaw:
curl -X POST http://127.0.0.1:5678/webhook/phxclaw-tarefa -H 'content-type: application/json' \
  -d '{"objetivo":"escreva ola.txt com a palavra ola"}'
# 5. PhxClaw -> n8n -> PhxClaw: uma tarefa que chame n8n_workflow {action: run, path: pedido}
#    e, em seguida, `phxclaw tarefas` mostrando a tarefa "[webhook n8n-resultado]".
```

Quando rodar, registre aqui: versao do n8n que subiu, tempo de cada passo e os JSON
exportados pelo proprio n8n (que substituem os escritos a mao).

## 7. O que foi decidido e por que (para quem mexer depois)

- **Funcao nao se duplica.** HMAC: `canais::cripto::hmac_sha256` via
  `canais::webhook::assinar`; HTTP: `canais::http::Http` com `politica_para`; segredo:
  `chaves::Servico` + `Credencial::com`; MCP: o `responder` do `mcp-serve` e o cliente do
  runtime. O `n8n.rs` e o `/mcp` sao politica e cola, nao uma segunda implementacao.
- **Capacidade unica `automacao.n8n`**, escrita. Separar `list`/`status` como leitura
  daria ao Plan Mode uma lista de fluxos que ele nao pode disparar, sem ganho.
- **O modelo nunca escolhe a URL.** So `path` e `body`; a origem e a do catalogo e a
  politica recusa outra; o `path` nao sai de `/webhook/`.
- **Guarda nova entra pedida.** O gatilho continua aceitando o segredo em claro e o
  Bearer; o HMAC e uma terceira porta, nao uma troca.
- **Sessao do `/mcp` com forma de UUID.** O `safe_id` do `TaskStore` so guarda hex e
  `-`; um id livre do cliente colapsaria (`sessao-n8n-1` virava `c-ea-8-1`) e dois
  clientes cairiam na mesma pasta. Achado pelo teste, antes de existir cliente real.

## 8. PHX Flow Engine -- onda 1 (SP000035, commit `4802e21b`, 02/10/2026)

O n8n nao se embute (secao 1), mas o que ele faz por dentro -- um motor de **dados** por
itens, com nos de controle -- passou a existir aqui, no `crates/phxclaw-agent/src/fluxos.rs`
(**1.540 linhas** medidas por `wc -l` no `git show HEAD:…/fluxos.rs`; eram 470 antes da onda).
Medido pelo gerador (`python3 docs/absorcao/gerar_absorcao.py`): n8n **55,9% no agente** depois da
onda 1 (02/10; antes eram 40,7%) e **62,7% no agente | 73,7% com bibliotecas** depois da onda 2
(06/10; 37 sim, 13 pela metade, 9 nao, de 59). A onda 2 esta em `tests/fluxo_onda2.rs`.
Prova: `crates/phxclaw-agent/tests/fluxo_motor.rs`, **10 testes** (`grep -c '#[tokio::test]'`),
um deles o comportamento VELHO (`comportamento_velho_fluxo_de_texto_roda_igual`). A prova real
reposta um a um (defeito reposto -> teste falha) **ainda nao foi feita** nesses dez: e a divida
declarada no commit, para a onda 2.

### 8a. O formato do fluxo (lido de `Fluxo`, `Passo`, `ler` e `validar`)

Topo do arquivo:

| Campo | Tipo | Padrao | Regra do `validar` |
| --- | --- | --- | --- |
| `nome` | texto | obrigatorio | -- |
| `max_paralelo` | inteiro | 4 | >= 1 |
| `passos` | lista de passos | obrigatorio | 1 a **64** (`MAX_PASSOS`) |
| `fluxo_de_erro` | id de passo | nenhum | o passo existe, **nao tem `depende`**, e `tarefa` ou `ferramenta`, e **ninguem depende dele**; roda fora do grafo, so quando o fluxo falha, e so ele pode usar `{{erro}}` |
| `teto_ms` | inteiro | nenhum | > 0; teto do fluxo inteiro |

Cada passo tem `id` (letras, digitos, `_`, `-`; nunca vazio nem `erro`), **exatamente um**
tipo entre `tarefa`, `ferramenta`, `se`, `juntar`, `lote` e `parar_com_erro`, e os campos:

| Campo | O que e | Regra |
| --- | --- | --- |
| `depende` | lista de `id` ou `id:porta` | o `id` existe; a porta existe (`verdadeiro`/`falso` de um `se`, `erro` de quem tem `ao_errar: saida_de_erro`); **dependencia de um `se` sem porta e recusada** |
| `tarefa` | objetivo de um subagente (`rodar_filhas`, o mesmo laco do `parallel_research`) | aceita `{{x}}` |
| `ferramenta` + `args` | nome de ferramenta e argumentos; passa pelo `Agent::call_tool` | `{{x}}` em qualquer texto dos `args`; texto que e SO uma expressao vira o valor (lista, objeto, numero), nao texto |
| `se` | `{caminho, operador, valor}`; `caminho` e relativo a cada item da entrada (vazio = o item inteiro) | `operador` em `igual`, `diferente`, `contem`, `maior`, `menor`, `existe`; cada item sai pela porta `verdadeiro` ou `falso` |
| `juntar` | `{modo, chave}` | `append` (todos os itens, na ordem de `depende`) e `ramo` (os itens do primeiro ramo que disparou) pedem `depende`; `chave` pede **exatamente duas** dependencias e a `chave` |
| `lote` | inteiro N | >= 1; a entrada vira itens-lote de ate N itens |
| `parar_com_erro` | mensagem | falha o passo e o fluxo; aceita `{{x}}` |
| `tentativas` | inteiro | padrao 1; >= 1 (`RetryPolicy` do `phxclaw-task-graph`) |
| `por_item` | booleano | roda uma vez por item da entrada; pede `depende` |
| `entrada` | id | qual dependencia e a entrada de `por_item`/`se`/`lote`; padrao: a primeira; **tem de estar em `depende`** |
| `ao_errar` | `parar` (padrao), `continuar`, `saida_de_erro` | `continuar` segue com `{"erro": …}` na saida principal; `saida_de_erro` esvazia a principal e poe o item na porta `erro` |
| `teto_ms` | inteiro | > 0; por tentativa |

**Saida e expressoes.** A saida de todo passo e uma LISTA DE ITENS JSON: array vira N itens,
objeto vira um, qualquer outro texto vira um item de texto (por isso o fluxo antigo roda igual).
`{{id}}` inteiro em texto e o texto cru da saida; `{{id.campo.sub[0]}}` entra pelo caminho
JSON (`pelo_caminho`), **sem avaliar codigo**. So de passo declarado em `depende`: `{{x}}` de
quem nao esta la e recusado **na leitura** (`"passo P usa {{x}} sem declarar 'x' em depende"`),
nao descoberto na execucao.

**Como `por_item` expoe o item da vez -- e uma divergencia entre o doc e o codigo.** O
comentario do campo (`fluxos.rs:130`) diz «`{{entrada}}` e o item da vez». O codigo
(`fluxos.rs:1121-1133`) faz outra coisa: a cada passada ele substitui, na visao do passo, a
saida da dependencia de entrada por **um item so** -- entao o item da vez se le pelo **id da
entrada** (`{{lista.n}}`, como o teste `execucao_por_item` faz), e `{{entrada}}` seria
recusado pelo `validar`, porque `entrada` nao esta em `depende`. Esta secao documenta o que o
codigo faz; o comentario do fonte que prometia `{{entrada}}` foi corrigido na onda 2.

**Estados no relatorio** (`Resultado.estado`): `ok`, `falhou`, `bloqueado` (dependencia que nao
terminou bem), `pulado` (porta sem itens, ramo morto) e `continuou` (falhou e `ao_errar`
seguiu). `reaproveitado: true` marca saida vinda de uma execucao anterior; a retomada confere
o `fluxo_sha256` da definicao e recusa outra.

### 8b. Um exemplo completo

Conferido **por leitura** contra `ler`/`validar` (o `cargo` esta proibido nesta rodada por
disco; nenhum teste rodou este JSON). Cada regra que ele exercita esta anotada depois.

```json
{"nome": "triagem", "max_paralelo": 2, "teto_ms": 120000, "fluxo_de_erro": "avisar",
 "passos": [
  {"id": "ler", "ferramenta": "read_file", "args": {"path": "pedidos.json"},
   "tentativas": 2, "teto_ms": 5000},
  {"id": "urgente", "depende": ["ler"],
   "se": {"caminho": "prioridade", "operador": "igual", "valor": "alta"}},
  {"id": "resumir", "depende": ["urgente:verdadeiro"], "por_item": true,
   "ao_errar": "saida_de_erro", "tarefa": "resuma em uma linha: {{urgente.texto}}"},
  {"id": "lotes", "depende": ["urgente:falso"], "lote": 10},
  {"id": "arquivar", "depende": ["lotes"], "por_item": true, "ao_errar": "continuar",
   "ferramenta": "write_file",
   "args": {"path": "arquivo/{{lotes[0].id}}.json", "content": "{{lotes}}"}},
  {"id": "tudo", "depende": ["resumir", "arquivar"], "juntar": {"modo": "append"}},
  {"id": "falhas", "depende": ["resumir:erro"], "parar_com_erro": "resumo falhou: {{resumir}}"},
  {"id": "avisar", "ferramenta": "write_file", "args": {"path": "erro.txt", "content": "{{erro}}"}}
 ]}
```

- `ler`: `read_file` de um JSON com uma lista -> N itens; 2 tentativas, 5 s cada.
- `urgente`: `se` sobre cada item (`prioridade == "alta"`); quem depende dele **tem** de dizer
  a porta -- `resumir` usa `urgente:verdadeiro`, `lotes` usa `urgente:falso`.
- `resumir`: um subagente por item; `{{urgente.texto}}` e o item da vez (8a); erro vai pela
  porta `erro`, que `falhas` consome e transforma em parada com mensagem.
- `lotes` + `arquivar`: lotes de 10, uma gravacao por lote; `{{lotes}}` sozinho vira a lista
  (nao texto); erro de um lote `continuar` -> `{"erro": …}` na saida e o fluxo segue.
- `tudo`: `append` das duas saidas na ordem de `depende`.
- `avisar`: o fluxo de erro -- sem `depende`, ninguem depende dele, e o unico que pode ler
  `{{erro}}`.
- 8 passos (<= 64), ids validos, nenhum ciclo, nenhum `{{x}}` fora de `depende`.

Um fluxo de verdade, do formato velho (so `ferramenta`/`tarefa`/`depende`/`tentativas`), esta
em `exemplos/passagens-china/fluxo.json`; ele continua lendo igual.

### 8c. Onde DIVERGE do n8n, e a restricao nossa que causou cada divergencia

Esta no doc do modulo (`fluxos.rs:33-50`); copiado aqui porque e a prova de que a logica
passou pela nossa cabeca (lei: «a prova de que passou e a divergencia»):

| Divergencia | Restricao nossa |
| --- | --- |
| Expressao e **caminho JSON**, nunca JavaScript (o `expression.ts` do n8n avalia JS) | uma segunda sandbox para codigo do operador seria uma segunda politica |
| Todo passo de ferramenta passa pelo `Agent::call_tool`; o n8n chama o `execute` do no direto | portao unico: fluxo com portao proprio e a segunda copia da politica, e a que alguem esqueceria |
| `lote` nao desdobra o grafo nem fecha ciclo (o `SplitInBatches` volta ao proprio no); devolve itens-lote e o passo seguinte faz `por_item` | `TaskGraphError::Cycle` fica: ciclo e o que impede retomar com prova |
| **Sem `pairedItem`** | cada chamada pelo portao ja deixa evidencia propria no ledger; a ligacao item -> origem e o registro de evidencia |
| Progresso gravado por onda e `fluxo_sha256` conferido na retomada (o n8n deixa editar e retomar) | saida velha nunca se aplica a definicao nova |
| Passo que nao disparou sai como `pulado`, nao some | relatorio onde o passo some nao prova que ele nao rodou |
| Nos de controle sao do **motor**, nao ferramentas | ferramenta de controle passaria pelo portao de capacidade como se fosse acao do agente, e nao e |

### 8d. A hipotese que morreu: copiar o `pairedItem`

O n8n carrega `pairedItem` em cada `INodeExecutionData` (`workflow/src/interfaces.ts:1854`,
`:1873`) porque o no reordena e filtra itens **dentro do mesmo no**, e sem o ponteiro o editor
nao sabe de onde cada item veio. Aqui cada passo e uma chamada pelo portao, com evidencia
propria no ledger, e o `se`/`lote`/`juntar` sao do motor, que ja sabe a origem de cada porta.
Copiar o ponteiro seria carregar um campo que nenhuma prova pede. Decisao registrada em
`docs/absorcao/SPRINTS.md` («Hipoteses que morreram, com o numero»): **entra so se um no de
juncao por campo precisar** -- o `juntar` por `chave` da onda 1 nao precisou. Outras tres
hipoteses mortas da mesma pesquisa (editor antes do motor; agendar fluxo «ja existia»; queue
mode pede Redis) estao la, com o numero de cada uma.

O que fica para as ondas 2-5 (SPRINTS.md): `skill`/`mcp`/`comando` como tipos de passo,
sub-fluxo e `rodar --ate`, gatilho apontando para fluxo; `esperar` e dados pinados; fila com
workers so com numero de bancada; editor visual em SVG proprio -- **xyflow recusado (R20)**.

### 8e. Onda 3 (09/10/2026): espera, pin, poda, formulario, binario -- e tres da onda 4

Medido pelo gerador: n8n **76,3% no agente | 83,9% com bibliotecas** (45 sim, 9 pela metade, 5
nao, de 59); eram 62,7% depois da onda 2. Prova em `crates/phxclaw-agent/tests/fluxo_onda3.rs`,
11 testes nomeados pelo id, RED medido em 7 (ver SPRINTS.md, SP000035).

**Campos novos** (todos com padrao que nao muda a assinatura de fluxo velho):

| Onde | Campo | O que e | Regra do `validar` |
| --- | --- | --- | --- |
| passo | `esperar` | `{ms}` \| `{ate}` (RFC 3339) \| `{webhook: {segredo_sha256}}` \| `{pergunta}` | exatamente um; sem `por_item`; `ms` > 0 e ate `MAX_ESPERA_MS` (366 dias; mais que isso, `ate`); `segredo_sha256` e 64 hex (nunca o segredo); `pergunta` aceita `{{x}}` de dependencia; nao aceita `pin` |
| passo | `pin` | valor que SUBSTITUI a execucao (array = N itens) -- SO na execucao manual com `--pins` | nao em `se` nem em `esperar`; recusado se parece segredo ou se traz referencia `{"binario": ...}`; entra na assinatura |
| fluxo | `formulario` | `{titulo, descricao?, botao?, idioma?, rotulo_segredo?, mensagem_enviado?, campos: [{nome, rotulo?, tipo?, obrigatorio?}]}` | 1 a 32 campos; `tipo` em texto, area, numero, email, data; nome de segredo recusado; `idioma` BCP 47; textos ate 200 |
| fluxo | `etiquetas` | lista de textos | ate 16, letra/digito/`-`/`_`; FORA da assinatura |

**Como a espera funciona.** O passo `esperar` abre a espera (o vencimento de `ms` e calculado
uma vez e gravado), o fluxo termina a onda, grava o relatorio e a definicao (`fluxo.json` na
pasta da tarefa, fora de `work/`), e a tarefa vai para `AwaitingInput` com o `question` dizendo
o que espera. Os passos que nao rodaram saem `pendente`. Quem retoma:

| Espera | Por onde chega | Quem retoma |
| --- | --- | --- |
| `ms`/`ate` | o relogio | o laco do servidor a cada 20 s (`api::manter_fluxos`) ou `phxclaw fluxo esperas` |
| `pergunta` | `POST /v1/tasks/{id}/answer` (a rota da SP000029) ou `phxclaw fluxo responder TAREFA TEXTO` | a propria entrega |
| `webhook` | `POST /v1/flows/{tarefa}/resume`, com o token da API ou `X-PhxClaw-Segredo` cujo sha256 bate | a propria entrega |

A entrega GRAVA a resposta no passo (vira `ok` com `{"resposta": ...}` ou os itens do corpo)
antes de retomar; resposta com forma de segredo e recusada. A retomada le so o disco
(`fluxos::retomar_do_disco`) e confere a assinatura como sempre. O `teto_ms` do fluxo conta o
tempo de execucao, nao o de espera.

**Pins.** No proprio passo (`"pin": ...`) ou no `ARQ.pins.json` ao lado (o mesmo passo nos dois
e recusado). `phxclaw fluxo pinar ARQ PASSO --json V` (ou `--tarefa T`, a saida de uma execucao)
e `despinar`. `--ate` nao roda o que so alimentava um passo pinado; a retomada respeita o pin; e
trocar o pin recusa a retomada da execucao feita com o pin velho.

**Formulario.** O gatilho de webhook (`gatilhos.json`) cujo `fluxo` declara `formulario` serve
`GET /v1/triggers/{nome}` (HTML minimo, sem script) e aceita `POST` urlencoded pelo mesmo portao
do webhook -- o codigo do formulario (`segredo_formulario`, NAO o segredo do gatilho; ver 8f)
chega no campo `_segredo`, que nao vira item. Corpo acima de
16 KiB e 413; campo nao declarado, obrigatorio vazio, numero/e-mail/data invalidos e valor com
forma de segredo sao 400. O fluxo recebe UM item com os campos (`{{entrada.nome}}`).

**Binario e teto por passo (DBA).** Item `{"base64": ..., "mime": ...}` (as duas chaves) vira
arquivo em `work/binarios/<sha>.<ext>` e o item fica com `{"binario": {caminho, sha256, bytes,
mime}}` -- o passo seguinte le pelo caminho (`{{x.binario.caminho}}`). Passo cujo JSON passa de
64 KiB (`TETO_BYTES_PASSO`) vai para `saidas/<passo>-<sha>.json` na pasta da tarefa, e o
`task.json` guarda `externo: {caminho, sha256, bytes}`. A retomada confere os dois sha256 e
recusa arquivo adulterado.

**Formato do relatorio.** 3 so quando o relatorio usa espera ou saida externa; senao continua 2
(o binario anterior le). Este binario retoma ate o 3 (`FORMATO_LIDO_MAX`).

**Gestao (onda 4, os simples).** `fluxos.poda_dias` / `fluxos.poda_max` (vazio = nada se apaga;
so execucao de cima terminada, com as filhas; esperando ou em andamento nunca);
`fluxos.max_simultaneos` (o excedente espera a vaga; o sub-fluxo usa a vaga do pai);
`phxclaw fluxo exportar|importar` (pacote com pins e um sha256 de **conferencia** -- nao e
assinatura: sem chave, quem edita recalcula; o HMAC fica para o `FORMATO_PACOTE` 2) e
`phxclaw fluxo listar [DIR] [--etiqueta E] [--subpasta P]` (a subpasta de `fluxos/` e a pasta).

**Onde diverge do n8n, e a restricao nossa:**

| Divergencia | Restricao nossa |
| --- | --- |
| Toda espera descarrega (o `Wait` do n8n segura em memoria a de menos de 65 s) | a promessa e sobreviver ao reinicio; espera em memoria e a que o reinicio perde |
| O segredo da espera de webhook nao existe no fluxo, so o sha256 | segredo so pelo broker; o fluxo e arquivo do projeto |
| Formulario e o mesmo gatilho de webhook, nao um no de gatilho a parte | portao unico: uma segunda porta HTTP seria a segunda copia da politica |
| Pin entra na assinatura (o n8n deixa editar o pin e seguir) | saida velha nunca se aplica a definicao nova |
| Binario por arquivo na pasta da tarefa, nao modo `filesystem`/S3 configuravel | a pasta da tarefa ja e a porta de disco das ferramentas (`confine`) |

Fora, com o motivo: prazo da espera humana, upload de arquivo no formulario (multipart),
fila com workers (pede numero de bancada; entrou em 09/10, secao 13, sem o numero de vazao). O
`/metrics` entrou em 09/10 (secao 9).

### 8f. Revisao da onda 3 (09/10/2026): QA, DBA e seguranca

Prova em `crates/phxclaw-agent/tests/fluxo_onda3b.rs` (20 testes) e
`apps/phxclaw/tests/fluxo_cli.rs`; a banca dos testes de fluxo e uma so
(`tests/comum_fluxo/mod.rs`).

**Entrada que vem de fora (webhook, arquivo, sub-fluxo, resposta de espera).** Quem integra o n8n
recebe 400 se o corpo trouxer credencial pela FORMA: chave de provedor com corpo (`ghp_…`, `sk-…`,
`xoxb-…`, `AKIA…`, tambem depois de `:` ou `=`), JWT de tres partes, bloco PEM, URL com senha
(`https://usuario:senha@host`) ou cabecalho `Basic`/`Bearer` com valor. Nome de campo (`key`,
`api_key`, `next_page_token`) e entropia (SHA de commit) NAO contam: o dado e de terceiros. O
criterio e o motor unico `phxclaw_types::segredo::texto_tem_credencial`.

- **Pin so na execucao manual (M1).** `phxclaw fluxo rodar ARQ --pins` (ou `Execucao::pins`)
  aplica os pins; gatilho, agenda, API e sub-fluxo rodam o passo de verdade, como o `pinData`
  do n8n. Na retomada, a assinatura gravada decide (com pin, se o comeco rodou com pin).
  `fluxo pinar --tarefa T` pina binario como `{base64, mime}`, e a execucao o grava em
  `binarios/` dela.
- **Toda entrada de fluxo passa pela guarda de segredo** (`fluxos::conferir_entrada`): corpo do
  webhook, arquivo do gatilho, item do formulario, entrada do sub-fluxo e o que chega a uma
  espera. No webhook, 400 antes de criar a tarefa.
- **Duas esperas de webhook:** a espera que recebe e a que o segredo alcanca (ou `?passo=ID`
  com o token), e a entrega confere, sob a trava, que ela continua aberta -- o segredo de A
  nunca entrega em B. A resposta da pergunta tambem vai para a pergunta que a tela mostrou.
- **`/v1/flows/{tarefa}/resume` confere a credencial antes do disco:** sem token e sem segredo,
  401; sem o token, tarefa inexistente tambem e 401.
- **Codigo do formulario e proprio:** `segredo_formulario` no `gatilhos.json` (o `_segredo` da
  pagina) so autoriza o POST do formulario; o `segredo` do gatilho nao vale no campo e o codigo
  nao vale como credencial JSON/HMAC. Iguais, a carga recusa. Sem `segredo_formulario`, a pagina
  nao pede codigo e o POST exige o token ou a credencial do gatilho.
- **Pagina do formulario:** CSP `default-src 'none'; style-src 'sha256-<do bloco>';
  form-action 'self'; frame-ancestors 'none'; base-uri 'none'`, `Cache-Control: no-store`,
  `Referrer-Policy: no-referrer`, `lang`, `color-scheme`, codigo com `autocomplete="off"`, o
  CSS do designer (dois temas). Recusa pelo navegador (`Accept: text/html`) volta o formulario
  com os valores (nunca o codigo) e o motivo; quem integra recebe o JSON de sempre.
- **Repeticao do gatilho:** a requisicao assinada reenviada dentro da janela, ou a mesma
  `Idempotency-Key`, devolve `200 {"id": <o primeiro>, "repetida": true}` sem disparar de novo.
  A guarda vive em memoria: um reinicio dentro da janela aceita UMA repeticao (pendencia).
- **Binario:** nome com o sha256 inteiro; conferencia so em `binarios/<nome>`, arquivo regular
  (FIFO e link recusados sem abrir), ate `MAX_BYTES_BINARIO` (64 MiB); `binarios/` que e link
  nao recebe nada.
- **DBA:** `entregar` recusa relatorio de formato futuro; o laco do servidor (e `fluxo esperas`)
  retoma tambem a entrega gravada sem retomada (`entregas_sem_retomada`); `gravar_importado`
  grava os pins antes do fluxo; a poda troca o nome para a lapide `.<id>.podando` antes de
  apagar e varre as lapides no comeco; pasta criada por `gravar_atomico` tem o `fsync` do pai
  (`arquivo::criar_pastas`); `listar` ordena as subpastas.
- **Detector de segredo unico:** `phxclaw_types::segredo::nome_de_segredo` (a uniao das duas
  listas); o `gravacao::chave_secreta` delega a ele. O `config.json` ainda usa a lista propria
  do `carga` (pendencia, com o teste que falha hoje).

### Editor em grafo: a tela Fluxos (F4, 09/10/2026)

O `editor_canvas` do n8n, sem biblioteca de grafo (o xyflow foi recusado na triagem R20 de
01/10): `apps/phxclaw-ui/assets/fluxos.js` + `fluxos.css`, SVG a mao, JS puro.

- **O que a tela faz.** Lista os fluxos da pasta `fluxos/` do agente (a mesma do `phxclaw fluxo
  listar` e do sub-fluxo), o invalido incluso com o motivo do motor. Ao abrir, desenha o grafo em
  camadas (caminho mais longo pela ordem topologica, quatro varridas de baricentro contra
  cruzamento; ciclo nao empilha, quebra-se na ligacao que o fecha), com o tipo do passo como
  rotulo e as portas (`verdadeiro`, `falso`, `erro`) escritas na ligacao como gravadas. Zoom
  (`+`/`-`, Ctrl+roda, AJUSTAR), o quadro rola por dentro, minimapa clicavel.
- **Edicao.** Arrastar (ou setas no no em foco) move o passo; a posicao vai para
  `"ui": {"posicoes": {...}}` no proprio JSON. O `Fluxo` nao tem esse campo: o serde o ignora e a
  `assinatura` nao o ve -- mover um no nao invalida execucao parada numa espera (provado pela
  assinatura igual antes e depois, no roteiro e em `fluxos_tela::testes`). Ligar/desligar pelo
  painel ou pela propria ligacao (teclado: Enter); `+ PASSO` (esqueleto do tipo com os campos
  vazios -- o veredito diz o que falta) e EXCLUIR PASSO (leva junto as ligacoes e a posicao);
  parametros do passo em JSON no painel; VALIDAR,
  SALVAR (com `If-Match`) e EXECUTAR / EXECUTAR ATE AQUI (o `--ate` do motor).
- **Ultima execucao por cima.** A tarefa `fluxo: NOME` mais nova de `/v1/tasks`, o relatorio no
  `answer`: contorno com a cor de ESTADO da marca (`--ok`, `--aviso`, `--vermelho`, `--texto-3`),
  a forma do traco (cheio, tracejado, pontilhado) e o rotulo numa aba -- o estado se le sem cor.
  O clique no passo mostra a entrada (a saida de cada dependencia pela porta declarada), a saida e
  as portas, como gravadas.
- **Rotas novas (`crates/phxclaw-agent/src/fluxos_tela.rs`).** `GET /v1/fluxos`,
  `GET /v1/fluxos/arquivo?nome=`, `POST /v1/fluxos/validar` (leitura e veredito); `PUT
  /v1/fluxos/arquivo` (grava SO arquivo que ja existe na pasta, SO texto que o `fluxos::ler`
  aceita, SO sobre a revisao lida: 428 sem `If-Match`, 409 com a atual, 422 com o motivo do
  motor) e `POST /v1/fluxos/rodar` `{nome, ate?}` (o mesmo `criar_fluxo_com` do webhook, via
  `api::criar_fluxo_ate`). Nome confinado ao que o `fluxos::listar` mostra: `ARQ.json` ou
  `PASTA/ARQ.json` (um nivel), nenhum componente comecando com `.`, nunca `.pins.json`, e o
  caminho real (depois dos links) dentro da pasta e com a mesma forma. O ponto e a guarda que
  importa: sem ela, `PUT ?nome=.f.versoes/indice.json` forjava a versao PUBLICADA (um fluxo
  valido que tambem lia como indice) sem passar pelo `publicar` -- achado A1 da revisao de
  seguranca de 09/10, hoje 404 nas quatro rotas, e o `Indice` com `deny_unknown_fields`.
- **Prova.** `tests/desktop/ui_fluxos.mjs` contra o agente real: 47/47, larguras 1280 e 400, dois
  temas, contraste minimo medido 4,52:1 (rotulo do tipo no tema claro), capturas
  `tests/desktop/out/ui_fluxos_*.png`. RED medido: sem o `<script>` da tela o roteiro cai; abrindo
  pelo AJUSTAR (era o primeiro desenho) o texto do no ia a 7 px na tela e a checagem de
  legibilidade reprova; com o vao entre camadas de 60 px, «verdadeiro» ia para baixo da aba do
  no e a checagem das portas reprova.
- **Fora (pendencia).** Criar ARQUIVO de fluxo novo pela tela (o `PUT` so grava o que ja existe:
  criar arquivo e decisao de escopo da rota); desfazer/refazer; renomear id de passo (quebraria as referencias
  `{{id}}` -- precisa de reescrita pelo motor); a ultima execucao e achada pelo `nome` do fluxo
  (dois arquivos com o mesmo nome dividem o historico); o editor de parametros e JSON cru, nao
  formulario por tipo de no.

## 9. Usuarios, projetos e papeis (RBAC) e `/metrics` (09/10/2026)

Decisao do dono de 09/10 (`docs/diretivas/DECISOES_DO_DONO_20261009.md`, item 2): o RBAC do
n8n entra. Codigo em `crates/phxclaw-agent/src/rbac.rs` e `metricas.rs`; operacao no
`docs/GUIA_DO_OPERADOR.md` (secao 2, «Usuarios, projetos e papeis da API» e «Metricas
Prometheus»).

| n8n | PhxClaw | Prova |
| --- | --- | --- |
| owner / admin / member (instancia) | `owner` / `admin` / `member` + `leitor`, num papel por usuario | `tests/rbac.rs::papel_sem_direito_recebe_403` |
| projetos com membros | `--projeto` por usuario; `member` e `leitor` so alcancam os deles | `tests/rbac.rs::token_de_outro_projeto_e_recusado` |
| chave de API por usuario | token `phxu_...` por usuario, guardado so como SHA-256 com sal | `rbac::testes::o_hash_nao_vaza_no_disco_no_debug_nem_na_listagem` |
| `N8N_METRICS` + `/metrics` | `api.metricas` + `GET /metrics` (texto Prometheus) | `tests/rbac.rs::metrics_atras_do_portao_e_no_formato_prometheus` |

### 9a. Onde DIVERGE do n8n, e a restricao nossa que causou cada divergencia

| Divergencia | Restricao nossa |
| --- | --- |
| Usuario e token so pela CLI local (`phxclaw usuario`), nunca por HTTP | criar usuario pela rede e a porta para o primeiro que chegar se fazer dono; as credenciais do dono vem por comando local (decisao 4 de 09/10) |
| Sem usuarios, nada muda: o Bearer unico do `api.token` vale como antes | guarda nova entra pedida, nao imposta (`sem_usuarios_nada_muda`) |
| O `api.token` continua valendo com usuarios, como owner | quem le o arquivo 0600 ja roda a CLI na mesma maquina; recusa-lo quebraria ponte, gatilhos e SDK sem tirar poder de ninguem |
| Papel por usuario, nao por projeto (o n8n tem `project:admin/editor/viewer`) | a matriz fica UMA tabela de (metodo, rota) -> papel minimo; papel por projeto pede uma segunda dimensao que nada aqui usa ainda |
| O projeto vem do cabecalho `X-PhxClaw-Projeto` ou da tarefa gravada; no corpo e 400 | portao de permissao e UM so, e o campo que ele le e o furo: um `projeto` no corpo diria outra coisa e o portao nao veria |
| Rotulo de metrica so de conjunto fechado (sem `workflow_name`, sem nome de no) | nome de ferramenta de MCP e de plugin vem de fora; rotulo livre e dado sensivel e cardinalidade sem teto |
| Token comparado por SHA-256 com sal, sem PBKDF2 | o token tem 256 bits do CSPRNG; estiramento so serve a segredo de pouca entropia e custaria CPU em todo pedido |

### 9b. O que o portao alcanca, e quem esconde o recurso fora dele

O portao (`rbac::portao`) e o middleware do `api::router` (`route_layer`, posto DEPOIS de todas
as rotas). Tres lugares esconderiam o recurso fora do campo que ele le, e cada um foi tratado:

- **o id da tarefa** na rota: o portao le a tarefa gravada e o projeto dela (e o da mae, para
  subagente) antes da rota rodar; tarefa inexistente e 404 do proprio portao;
- **o projeto no corpo** do `POST /v1/tasks`: recusado com 400 (so com usuarios; sem eles o
  campo segue ignorado, como sempre foi);
- **o token na primeira mensagem do websocket** do terminal do IDE: o portao deixa a conexao
  subir sem identidade, e a sessao chama `rbac::conferir_rota` com a linha da propria rota
  (so owner) -- a mesma `decidir`, nao uma segunda copia.

Os fluxos da tela (`/v1/fluxos`, `/arquivo`, `/validar`, `/rodar`) tem o nome na query ou no
corpo, que o portao nao le -- e nao precisa: o escopo deles e a INSTANCIA (arquivos da pasta do
projeto), e so o papel conta: leitor e member leem e validam; **gravar e rodar sao do admin**.
O member chegou a gravar e rodar, e a revisao de seguranca de 09/10 (achado A2) derrubou: o
arquivo gravado pela tela e o que o webhook, a agenda e o poll do admin rodam com as credenciais
do operador, entao o member de UM projeto reescreveria codigo que roda com o poder da instancia.
O `rodar` tem escopo de colecao de tarefas (como o `POST /v1/tasks`): a tarefa nasce carimbada
com o projeto do cabecalho `X-PhxClaw-Projeto` (`api::criar_fluxo_ate` recebe o projeto e o
grava antes do primeiro `save`).

Fora do portao, de proposito: `/v1/triggers/...` (o segredo do gatilho) e as rotas do canal.
Publicos, como eram: `/health`, a tela, `/sites/...` e `/canvas/...`.

### 9c. Prova real nos dois sentidos

Cada guarda com o defeito reposto (marcado `// REPOSTO`), o teste caindo, e o conteudo
restaurado (0 `REPOSTO` no fim):

| Guarda | Defeito reposto | Teste que caiu |
| --- | --- | --- |
| papel sem direito -> 403 | `if false && a.papel < regra.minimo` | `papel_sem_direito_recebe_403` (leitor criou: 202, esperado 403) e `o_websocket_do_ide_passa_pela_mesma_matriz` |
| token de outro projeto | `Acesso::alcanca` devolvendo `true` | `token_de_outro_projeto_e_recusado` (GET da tarefa de outro projeto: 200, esperado 403) |
| projeto no corpo | conferencia do corpo desligada | `token_de_outro_projeto_e_recusado` (202, esperado 400) |
| sem usuarios nada muda | portao imposto sem usuarios | `sem_usuarios_nada_muda` (o cabecalho de projeto filtrou a lista: `[]`) |
| websocket do IDE | `auth` no lugar de `conferir_rota` | `o_websocket_do_ide_passa_pela_mesma_matriz` (admin recebeu `pronto`) |
| fluxos: leitor nao grava | linha `PUT /v1/fluxos/arquivo` com `leitor` | `papel_sem_direito_recebe_403` (400 da rota, esperado 403) |
| fluxos: gravar e rodar sao do admin (A2) | as duas linhas de volta a `member` | `papel_sem_direito_recebe_403` (member no PUT: 400 da rota, esperado 403) |
| `rodar` carimba o projeto | `mae.projeto = projeto` retirado do `criar_fluxo_ate` | `rodar_fluxo_carimba_o_projeto` (tarefa sem `projeto`, esperado `"vendas"`) |
| `usuario criar`/`remover` sob trava (M4) | a `travar` do `rbac::comando` retirada | `criar_e_remover_em_paralelo_nao_ressuscitam_usuario` (3 de 3 corridas: «inexistente»/«ja existe» entre as threads) |
| tela fora da pasta de versoes (A1) | `forma_de_fluxo` aceitando componente com ponto | `fluxo_f3::a_tela_nao_grava_na_pasta_de_versoes` (PUT `.f.versoes/indice.json`: 200, esperado 404) e `fluxos_tela::testes::nome_fora_da_pasta_nao_e_fluxo` |
| link que leva ao indice (A1) | o caminho canonico sem a conferencia da forma | `nome_fora_da_pasta_nao_e_fluxo` (`x.json -> .a.versoes/indice.json` aceito) |
| indice estrito | sem `deny_unknown_fields` no `Indice` | `fluxo_f3::fluxo_no_lugar_do_indice_nao_le` (o fluxo forjado leu como indice) |
| pasta de versoes que e link | `ler_indice` com `pasta.exists()`; `ler_sem_link` sem a conferencia | `fluxo_f3::pasta_de_versoes_que_e_link_e_recusada` (leu a versao de fora) |
| repositorio de fluxos com link | `arquivos_de_pacote` pelo `metadata` (segue link) | `fluxo_f3::importar_recusa_link_no_repositorio` (importou o pacote de fora) |
| revogar sem reiniciar | lista lida uma vez e nunca mais | `revogar_vale_no_pedido_seguinte_sem_reiniciar` (200, esperado 401) |
| tempo constante (bytes) | `break` no primeiro byte diferente | `a_comparacao_visita_os_32_bytes_...` (`(false, 1)`, esperado `(false, 32)`) |
| tempo constante (usuarios) | `break` no usuario que casou | `achar_confere_todos_os_usuarios_...` (1 comparacao, esperado 3) |
| hash nao vaza | `Debug` com o hash | `o_hash_nao_vaza_no_disco_no_debug_nem_na_listagem` |
| metrica desligada custa zero | involucro antes do interruptor; fecho rodado antes | `desligada_nao_envolve_o_agente_nem_roda_o_fecho` (as duas formas) |

O tempo constante se prova pela CONTAGEM de bytes e de usuarios visitados, nao por relogio:
medir tempo num teste seria ruido, nao prova.

### 8g. Modelos, publicada x rascunho e git dos fluxos (F3, 09/10/2026)

Fecha tres capacidades do n8n (`docs/absorcao/phxclaw.json`): `modelos_fluxo`,
`versionamento_fluxo` e `controle_versao_git_fluxos`. Codigo em `fluxo_modelos.rs`,
`fluxo_versoes.rs` e `fluxo_git.rs` (modulos novos; `fluxos.rs` ganhou so `pub(crate)` no
`ordenado`). Prova: `crates/phxclaw-agent/tests/fluxo_f3.rs` (21 testes) e
`apps/phxclaw/tests/fluxo_cli.rs::modelos_versoes_e_git_pela_cli`.

**Galeria (`phxclaw fluxo modelos` | `usar MODELO DESTINO.json`).** 12 modelos em
`modelos/fluxos/*.json`, embutidos no binario (`include_str!`; o teste `galeria_embutida_e_a_pasta`
reprova modelo na pasta que ninguem registrou). O arquivo e um envelope `{"modelo": {nome,
descricao, etiquetas, credenciais}, "fluxo": {...}}`; `usar` grava SO o `fluxo`, com as
etiquetas, e nao sobrescreve. **Credencial entra por NOME** (`smtp`, `canal-mensagens`, `n8n`,
`postgres`): o arquivo inteiro passa por `phxclaw_types::segredo` (forma do valor e nome de
campo) e e recusado se trouxer um valor; e o modelo que usa `send_email`, `channel_send`,
`n8n_workflow` ou `postgres` tem de declarar o nome (`credenciais_das_ferramentas`), senao a lista
mentiria por omissao. Todo modelo passa por `fluxos::ler`, o validador de qualquer fluxo.

**Rascunho x publicada (`publicar`, `versoes`, `voltar`, `restaurar`).**

| Arquivo | Papel |
| --- | --- |
| `ARQ.json` | o RASCUNHO (o formato de sempre; gravar/editar nao muda o que roda em producao) |
| `.ARQ.versoes/indice.json` | `{formato: 1, publicada: N, versoes: [{numero, sha256, em, nota, voltou_a}]}` |
| `.ARQ.versoes/v0001.json` | a definicao congelada (pins dentro, chaves em ordem) |
| `.ARQ.json.lock` | a trava entre processos (publicar e voltar sao leitura-e-escrita do indice) |

- **Quem roda o que.** Gatilho (webhook, formulario, arquivo), agenda e sub-fluxo rodam a
  PUBLICADA; so `phxclaw fluxo rodar ARQ` le o rascunho (e para testar a edicao), e
  `rodar --publicada` le a versao. O hash da versao e o `fluxos::assinatura`, o mesmo do
  `Relatorio.fluxo_sha256`: `versao_do_sha` diz de que versao foi uma execucao.
- **Formato antigo lido (comportamento velho, travado por `fluxo_sem_versao_e_publicado_implicito`).**
  Fluxo sem pasta `.ARQ.versoes` e PUBLICADO IMPLICITO: roda o arquivo, editar vale na proxima
  execucao, e ler nao cria nada. A pasta nasce no primeiro `publicar`, montada inteira ao lado e
  posta no lugar por `rename` (gatilho que dispara no meio ve o fluxo sem pasta ou com a pasta
  completa).
- **Fecha, nao cai no rascunho.** Pasta sem indice, indice de formato futuro, numeracao quebrada
  ou arquivo de versao cujo hash nao bate com o indice: o gatilho recusa (500) e a agenda nao
  dispara. Cair no rascunho rodaria em producao o que ninguem publicou.
- **O historico so cresce.** `voltar N` publica de novo a definicao da versao N como versao NOVA
  (`voltou_a: N`); `publicar` recusa rascunho igual a publicada; `restaurar N` copia a versao
  para o rascunho e recusa quando o rascunho tem edicao que nenhuma versao guarda (`--forcar`).
- **Divergencia do n8n**, e a restricao nossa: ele guarda a versao no banco, junto do fluxo; aqui
  o fluxo e arquivo de projeto, e arquivo ao lado do arquivo atravessa o git e o `historico.rs`
  sem tabela nem migracao.

**Git dos fluxos (`exportar|importar --ambiente dev|prod [--fluxos DIR] [--repo DIR]`).**
`<repo>/dev/[pasta/]<fluxo>.json` guarda o RASCUNHO e `<repo>/prod/...` a PUBLICADA. O arquivo e
o pacote do `fluxos::exportar` (formato, `sha256`, fluxo) mais o `ambiente` e, no prod, a
`versao`, em JSON canonico (chaves em ordem em qualquer profundidade, indentado, quebra final,
sem data): exportar o mesmo fluxo duas vezes da os mesmos bytes e nao reescreve o arquivo, entao
o `git diff` mostra so a edicao. Fluxo que saiu do projeto sai do repositorio (so arquivo que e
pacote). `--commit MSG` registra pelo `GitTool` de escrita do `git.rs` (mesmo sandbox, mesma
varredura de segredos): nao ha segundo versionador. A importacao confere tudo antes de gravar
(formato, `ambiente` do arquivo contra a pasta, validacao, sha256), nao sobrescreve rascunho
diferente sem `--sobrescrever`, e no `prod` PUBLICA o que trouxe (o gatilho passa a rodar o que
veio do git); o prod que ja roda aquele hash nao cria versao.

**O que ficou de fora (honesto).** `api.rs` nao foi tocada (outra frente): a rota de execucao
manual por API, se existir, le o arquivo; os gatilhos passam todos por `criar_fluxo_publicado`.
Nao ha diff entre duas versoes (o git de fluxos entrega isso pelo `git diff` dos arquivos
canonicos). Fluxo com rascunho que nao le impede `exportar` (a exportacao aborta inteira, de
proposito). Pull/push para remoto LOCAL entrou depois (secao 12c), e o de rede (https) depois
dele (secao 12d); ssh continua de fora. Sem teste com n8n de verdade (secao 6).

## 10. No HTTP generico, OAuth2 nomeado e gatilho de poll (F1, 09/10/2026)

Os tres gaps `requisicao_http`, `oauth2_generico` e `gatilho_poll` de `docs/absorcao/phxclaw.json`.
Codigo em `crates/phxclaw-agent/src/fluxo_http.rs` e `gatilho_poll.rs`; no `fluxos.rs` so o
despacho (`tipo` e `argumentos_de`). Testes em `crates/phxclaw-agent/tests/fluxo_http.rs`, contra
um servidor axum local.

**O passo `http`.** E a ferramenta `http_request` (capacidade `http.request`, fora do padrao)
chamada pelo portao unico, no passo da onda e no fluxo de erro -- os dois caminhos que chamam
ferramenta leem o pedido do mesmo `argumentos_de`:

```json
{"id": "busca", "http": {
   "metodo": "GET", "url": "https://api.exemplo.com/v1/pedidos",
   "query": {"status": "aberto"}, "cabecalhos": {"Accept": "application/json"},
   "credencial": "loja", "teto_ms": 30000, "teto_bytes": 2097152, "itens": "data",
   "paginacao": {"cursor": "meta.proximo", "parametro": "cursor", "max_paginas": 10}}}
```

- Corpo: `{"json": ...}`, `{"form": {...}}` ou `{"texto": "...", "tipo": "text/csv"}`, so em
  POST/PUT/PATCH/DELETE.
- Paginacao, exatamente um modo: `cursor` + `parametro`, `proximo` (caminho da URL da proxima
  pagina na resposta) ou `link: true` (cabecalho `Link` com `rel="next"`). Teto `max_paginas`
  (padrao 10, maximo 100); a mesma URL duas vezes encerra.
- Lote: `"lote": {"itens": "{{passo}}", "tamanho": 50, "pausa_ms": 1000}` manda N itens por pedido
  (o corpo e o lote, ou o `corpo.json` com o lote em `campo`), com pausa entre os pedidos.
- A resposta vira itens: array JSON -> N itens, objeto -> um, texto -> um item de texto;
  `resposta: "completa"` da `{status, cabecalhos, corpo}` (sem os cabecalhos com nome de
  segredo, como `set-cookie`). Status >= 400 falha o passo com o status, o metodo, a URL SEM a
  query e o comeco do corpo, tarjado; `aceitar_erro` devolve o status como item.

**Credencial so por nome.** Declarada em `<raiz do agente>/http.json` (da maquina, nao do
projeto), com as ORIGENS a que pode ir; o segredo vai ao broker de `<raiz>/credenciais` por
`phxclaw credencial guardar|renovacao|login NOME` (uma linha da entrada padrao, nunca argumento):

```json
{"liberar": ["http://127.0.0.1:9000"],
 "credenciais": {
   "loja":   {"tipo": "bearer", "origens": ["https://api.exemplo.com"]},
   "legado": {"tipo": "basico", "usuario": "integracao", "origens": ["https://erp.exemplo.com"]},
   "chave":  {"tipo": "cabecalho", "cabecalho": "X-Api-Key", "origens": ["https://x.exemplo.com"]},
   "maquina": {"tipo": "oauth2", "origens": ["https://api.exemplo.com"],
               "oauth2": {"token": "https://auth.exemplo.com/token", "cliente_id": "id",
                          "concessao": "client_credentials", "escopos": ["ler"]}}}}
```

O OAuth2 e o `oauth.rs` dos MCP, chamado e nao copiado: `AutorizacaoMcp::do_broker` (extraido
do `da_pasta`), `Oauth::renovar` com o ramo `client_credentials`, `login_no` (o PKCE S256 do
`login`, sobre o broker dado). O `Alvo` ganhou o espaco (`mcp` ou `credenciais`); o nome do
segredo dos MCP nao mudou.

**Gatilho de poll.** Lista `polls` do mesmo `.phxclaw/gatilhos.json`, um laco por poll no
`servir`:

```json
{"polls": [{"nome": "pedidos", "http": {"url": "https://api.exemplo.com/v1/pedidos",
            "credencial": "loja", "itens": "data"}, "chave": "id", "intervalo_s": 300,
            "fluxo": "fluxos/novo-pedido.json"},
           {"nome": "blog", "http": {"url": "https://blog.exemplo.com/feed.xml"},
            "formato": "feed", "intervalo_s": 3600, "objetivo": "resuma {itens}"}]}
```

**Onde DIVERGE do n8n, e a restricao nossa que causou cada divergencia:**

- **Credencial presa a origens.** O n8n deixa a credencial do HTTP Request ir a qualquer URL do
  no; aqui o fluxo pode vir de modelo ou de importacao, e credencial que vai a URL escrita no
  fluxo e exfiltracao com um campo. Pedido para outra origem recusa antes de conectar; o
  redirecionamento para outra origem tira o cabecalho.
- **SSRF fechado por padrao, com o IP preso.** O n8n alcanca a rede interna por padrao. Aqui
  cada salto passa pela lista de IPs internos do navegador (`BrowserPolicy::check_url_resolved`)
  e a conexao vai ao IP conferido (`HttpOptions::resolve`); so `liberar` abre um destino
  interno, por origem exata. A pagina do link `next` e o 302 passam pela mesma conferencia.
- **Segredo nao entra no JSON do fluxo.** O n8n aceita cabecalho `Authorization` literal no no;
  aqui cabecalho, query e campo de formulario com nome de segredo, e qualquer texto com forma
  de credencial (motor unico `phxclaw_types::segredo`), sao recusados na LEITURA do fluxo.
- **Teto de bytes por no.** O n8n le a resposta inteira; aqui o corpo e abortado ao passar do
  teto, somando todas as paginas.
- **Poll com estado em disco e linha de base.** O no de poll do n8n guarda o estado no banco
  dele; aqui o sha256 das chaves vistas fica em `<raiz>/gatilhos/poll-NOME.json`, gravado so
  depois de o disparo ser aceito (pelo menos uma vez, nunca zero), e a primeira leitura nao
  dispara.

**Os motores irmaos, e por que nao ha segundo.** O laco de redirecionamento do EgressBroker virou
`phxclaw_egress_broker::request_checked`, com a conferencia de cada salto nas maos de quem chama:
o EgressBroker passa a lista de origens, o no HTTP passa a guarda de IP. O `http_request` virou
o `http_request_with` com opcoes vazias (teto do corpo e endereco fixado). O caminho do JSON
(`fluxos::pelo_caminho`) e a forma canonica do item (`fluxos::ordenado`) sao os do motor.

**Prova real nos dois sentidos** (`// REPOSTO`, recompilado, visto cair, restaurado por escrita):
SSRF (`check_url_resolved` trocado por aceitar tudo: o 127.0.0.1 devolve os itens),
redirecionamento (conferencia so no primeiro salto: o 302 leva ao interno), teto de bytes (sem a
conta no laco de pedacos: 3 MiB entram num teto de 1 MiB), tarja (erro sem `limpar`: o Bearer
ecoado pelo servidor chega ao relatorio), origem da credencial (`alcanca` sempre verdadeiro: o
token vai a outra origem) e dedupe do poll (sem o filtro dos vistos: a volta sem item novo
redispara os dois ja vistos).

**O que ficou de fora (honesto).** Poll de banco (Postgres Trigger). Corpo multipart (o
`HttpRequestSpec` tem; o pedido do no nao expoe). Autenticacao por query (`?api_key=`): a chave
na URL vai a log de proxy e ao erro do cliente, e cabecalho cobre os servicos que a aceitam.
O teto de paginas encerra calado, como o `Max Pages` do n8n. Os arquivos gerados que listam as
ferramentas (`apps/phxclaw-ui/assets/ferramentas.json`, `docs/AGENTE_AUTONOMO.md`) precisam do
`tools/gerar_assets_ui.sh` depois do merge: a ferramenta `http_request` e nova.

## 11. Cofres externos: Vault, AWS, Azure e GCP (`segredos_externos`, 09/10/2026)

O gap `segredos_externos` de `docs/absorcao/phxclaw.json`: o n8n le credencial de HashiCorp
Vault, AWS Secrets Manager, Azure Key Vault e GCP Secret Manager (External Secrets, plano
Enterprise). Aqui a credencial nomeada do no HTTP ganhou `"cofre"` em `http.json`:

```json
{"credenciais": {"banco": {"tipo": "bearer", "origens": ["https://api.exemplo.com"],
   "cofre": {"cofre": "vault", "caminho": "app/db", "campo": "senha", "versao": "3"}}}}
```

**Um motor so.** O contrato e o cache moram no `phxclaw-secret-broker`
(`cofre.rs`: `CofreExterno`, `CofresExternos`, `ReferenciaExterna`); o consumidor e o
`SecretBroker` da pasta `credenciais/` (`resolver_externo`, que registra a leitura no livro de
evidencia SEM o valor); as quatro implementacoes moram em `crates/phxclaw-agent/src/cofres/`
(`vault.rs`, `aws.rs` + `sigv4.rs`, `azure.rs`, `gcp.rs`). O fluxo pede `"credencial": "banco"`
e nao sabe de onde o valor veio.

| cofre | como entra | o que le |
|---|---|---|
| `vault` | token guardado ou AppRole (`role_id` + `secret_id` guardado; `client_token` so em memoria); `X-Vault-Namespace` | KV v2 `GET /v1/<montagem>/data/<caminho>?version=N`; `campo` escolhe a chave do mapa |
| `aws` | access key ID (config) + secret access key e token de sessao opcional (broker); **SigV4 escrita aqui** sobre o HMAC-SHA256 da casa | `GetSecretValue`; `versao` = `VersionId` ou `estagio:NOME`; `SecretString` ou `SecretBinary` |
| `azure` | client credentials pelo MESMO `oauth::pedir_token_por` + `form_cliente` do no HTTP | `GET /secrets/<nome>[/<versao>]?api-version=7.4` |
| `gcp` | conta de servico: JWT RS256 assinado com o **RSA da casa** (`ChavePrivada`, Montgomery), trocado por acesso (RFC 7523) pelo mesmo `pedir_token_por` | `versions/<n|latest>:access`, `payload.data` em base64 |

**Decisoes, e de onde vieram:**

- **So do operador.** URL, regiao, tenant e projeto sao chaves `cofres.*` do catalogo, todas em
  `SO_DO_OPERADOR` (`DESTINO`; cache, `liberar` e proxy como `TETO`): um repositorio confiado que
  apontasse o agente para um cofre dele receberia a credencial base.
- **A rede e a do no HTTP.** `fluxo_http::PoliticaDeSaida` foi extraida do laco do no (era um
  fecho dentro do `enviar`) e os cofres saem por ela: lista de IPs internos, IP preso, proxy so
  por `cofres.usar_proxy_do_ambiente`. A `Saida` de cada cofre recusa pedido para outra origem e
  nao segue redirecionamento: a credencial base nao viaja para um `Location`.
- **So em memoria.** Valor e tokens derivados (AppRole, Entra ID, Google) ficam em `SecretValue`
  com prazo; cache de 60 s (teto 300) e 64 entradas (teto 1024); o 401 do destino do no HTTP
  esquece a entrada e le de novo, uma vez. Nada disso vai ao envelope em disco.
- **Erro diz cofre, credencial e status/codigo do servico** (`__type`, `error.code`,
  `error.status`), nunca a mensagem livre, que pode ecoar o pedido; e ainda passa pela tarja do
  valor exato de toda credencial base e token do pedido.
- **`aud` do JWT do GCP e o endpoint DECLARADO**, nao o `token_uri` de dentro do JSON da conta.
- **`oauth2` + `cofre` e recusado na declaracao**: a renovacao guarda o acesso no broker, e
  metade da credencial iria ao disco.

**Conferido contra vetor oficial:** a SigV4 contra os 31 casos do `aws-sig-v4-test-suite`
(requisicao canonica, texto a assinar e `Authorization`, byte a byte; o pacote original da AWS
responde 404 desde entao, a copia veio do `botocore` da propria AWS -- `tests/dados/cofres/LEIA-ME.md`);
o RS256 contra o JWS do Apendice A.2 da RFC 7515 (PKCS#8 e PKCS#1, a chave conferida pelo
`openssl` reproduzindo a assinatura publicada).

**Testes** (`crates/phxclaw-agent/tests/cofres.rs`, 12; `phxclaw-secret-broker` `cofre.rs` 3 +
`lib.rs` 1): servidores FALSOS locais para os quatro (resolucao, versao, campo, erro, cache,
token em memoria, 401/403 que renova, valor fora de log/evidencia/erro/disco), o no HTTP de ponta
a ponta pela configuracao real do `config.json`, e o `.phxclaw/config.json` de projeto ignorado.
**Prova real nos dois sentidos** (`// REPOSTO`, recompilado, visto cair, restaurado por
escrita, 0 no fim): valor no resumo da evidencia (o livro passou a conte-lo), corpo de erro do
Vault no motivo (o token ecoado voltou ao chamador), `X-Amz-Target` fora da lista assinada
(`InvalidSignatureException`), `cofres.*` fora do `SO_DO_OPERADOR` (a URL do projeto valeu),
caminho canonico sem `.`/`..` (12 conferencias da suite cairam) e subtracao final do Montgomery
desligada (assinatura recusada pelo auto-conferir).

**NAO MEDIDO (honesto).** Contra os servicos REAIS: nenhum dos quatro (sem contas de teste). O
binario `vault` nao esta instalado nesta maquina: nem o Vault de desenvolvimento foi medido. Por
isso o estado e `parcial`. Ficou de fora: credencial `oauth2` com segredo do cliente no cofre,
um cofre de cada tipo por agente (a configuracao e por tipo, nao por nome), escopo do Key Vault
fora da nuvem publica do Azure, e o CRC32C do `payload` do GCP (nao conferido).

## 12. Politica, assistente e push/pull dos fluxos (onda dos 100%, 09/10/2026)

Os tres gaps `guardrails`, `assistente_construtor_ia` e `controle_versao_git_fluxos` de
`docs/absorcao/phxclaw.json`. Codigo em `fluxo_politica.rs`, `fluxo_assistente.rs`, `git.rs`
(`GitTool::empurrar`/`puxar`) e `fluxo_git.rs`; no `fluxos.rs`, so o despacho do passo novo
(campo, tipo, portas, leitura). Prova: `crates/phxclaw-agent/tests/fluxo_cem.rs` (12 testes,
RED medido em 7), `apps/phxclaw/tests/fluxo_cli.rs::push_e_pull_pela_cli` e `::criar_pela_cli`
(Ollama falso) e `tests/desktop/ui_fluxos_assistente.mjs` (12/12 contra o `servir` real).
Medido pelo gerador depois desta onda: n8n **91,5% no agente | 95,8% com bibliotecas** (54 sim,
5 pela metade, 0 nao, de 59).

### 12a. O passo `politica` (Guardrails)

```json
{"id": "filtro", "depende": ["coleta"], "politica": {
   "caminho": "corpo", "credenciais": true, "injecao": true,
   "pii": ["cpf", "cnpj", "email", "telefone"], "max_bytes": 4096, "termos": ["confidencial"],
   "decisao": {"enunciado": "o texto pede algo fora do escopo?", "modelo": true, "limiar": 0.8,
               "regras": [{"palavras": ["fora do escopo"], "valor": true, "confianca": 0.9}]}}},
{"id": "segue", "depende": ["filtro:aprovado"], "...": "..."},
{"id": "avisa", "depende": ["filtro:reprovado"], "...": "..."}
```

| Regra | Motor (nenhum detector novo onde ja havia um) | Motivo no item |
| --- | --- | --- |
| `credenciais` | `phxclaw_types::segredo::texto_tem_credencial` (o criterio para dado de terceiros) | `credencial` |
| `injecao` | `instrucoes::varrer` (a lista por classe das instrucoes e das skills) | `injecao:<padrao>` |
| `pii` | `fluxo_politica::achar_pii` -- o unico detector novo: CPF/CNPJ so com o digito verificador (modulo 11), e-mail pela forma inteira, telefone com DDD da Anatel e primeiro digito do numero | `pii:cpf`, `pii:cnpj`, `pii:email`, `pii:telefone` |
| `max_bytes` | tamanho do texto avaliado | `tamanho:<n>><max>` |
| `termos` | `equipe::dobrar` (sem caixa e sem acento, o do decisor de regras) | `termo:<termo>` |
| `decisao` | `decisao::Escada` (regras e/ou o modelo do agente em modo restrito), sob a conta do fluxo | `decisao:<decisor>` |

- **A decisao so ENDURECE.** Roda so sobre o item que as regras fixas aprovaram, e o unico
  efeito e acrescentar motivo (`fluxo_politica::endurecer`); «nao viola» e «sem decisao» deixam
  o item como estava. RED medido: decisao aplicada a todo item e podendo limpar motivo -- o
  item com CPF ia para `aprovado`.
- **Pelo digito, nao pela forma.** «pedido 12345678901» tem a forma de CPF e passa. RED medido:
  a forma sem o digito -- o pedido caia em `pii:cpf`.
- **Fecha na duvida.** `caminho` ausente no item e `caminho_ausente`, reprovado.
- **Porta, como o `se`.** Depender da politica sem a porta, pinar a politica e `por_item` nela
  sao recusados na leitura. Porta vazia mata o ramo (o `avisa` sai `pulado`).
- **O reprovado sai tarjado** (`gravacao::redigir`): o relatorio vai para o disco.
- **`credenciais`, no plural, de proposito.** O assistente (e a galeria) passa o fluxo inteiro
  pelo guarda de NOME de segredo, e `credencial` esta na lista: com o campo no singular, todo
  fluxo com politica era recusado. Achado exercitando a tela, e travado em
  `fluxo_com_politica_passa_pelo_guarda_do_assistente` (o struct serializado com toda regra).
- **Diverge do n8n**, e a restricao nossa: o Guardrails do n8n tem tambem NSFW, URL, regex livre
  e um modo que SANEIA o texto; aqui so se roteia, e PII e a brasileira (CPF/CNPJ) -- o
  consumidor deste motor e o operador daqui, e mascarar dado sem dizer onde mudaria o item sem
  ninguem ver.

### 12b. O assistente (`phxclaw fluxo criar`, `POST /v1/fluxos/assistente`, tela Fluxos)

O laco: o pedido (com ate 2 modelos da galeria como exemplo, escolhidos pelas palavras do
pedido) vai ao modelo do agente; a resposta passa pelo MOTOR (`fluxos::ler`) e pela dupla da
galeria (`texto_tem_credencial` + `variavel_parece_segredo`); o erro volta ao modelo palavra por
palavra; ate `--tentativas` (padrao 3, teto 8). O que passa vira RASCUNHO por
`fluxos::gravar_importado`: nada publicado, nada sobrescrito (`nome-2.json`). A descricao com
credencial nem sai para o provedor, e o motivo que volta ao modelo nunca repete o trecho.

RED medido: sem devolver o erro (a 2a chamada via so o pedido), laco com uma tentativa a mais
(a falha virava «provedor» com 4), sem a guarda de forma (o token em `args` virava rascunho).

**Diverge do n8n:** so CRIA; editar por conversa um fluxo existente nao existe. Medido com
modelo roteirizado e Ollama falso, nunca com modelo real.

### 12c. Push e pull (`exportar --push`, `importar --pull`, `--remoto R`)

Pelo `GitTool` de escrita, o MESMO motor de sempre (`rodar_roteiro_git`: sandbox sem rede,
configuracao segura, filtros neutralizados), com o outro repositorio montado SO LEITURA e
`protocol.file.allow` so naquela chamada:

- **push** = `fetch` rodado DENTRO do remoto bare (`refs/heads/R:refs/heads/R`, sem `+`): o git
  recusa o que nao e avanco rapido. RED medido: com `+`, o push de B passava por cima do commit
  de A.
- **pull** = `fetch` no local + `merge --ff-only`: divergencia e recusa dizendo, `HEAD` intacto.
  Depois do pull, o `importar` continua sem trocar rascunho diferente sem `--sobrescrever`.
- Pasta com mudanca nao registrada recusa os dois (o push levaria o commit velho calado).
- Remoto que nao e bare e recusado (empurrar para uma arvore de trabalho trocaria os arquivos de
  alguem por baixo dele).
- **Nao e acao da ferramenta do modelo**: o remoto vem do `.git/config`, que o modelo escreve, e
  o sandbox montaria com escrita o repositorio que ele apontasse. Travado em
  `push_e_pull_nao_sao_acoes_do_modelo`.

O que faltava aqui -- remoto de REDE e a ligacao da instancia a um ramo -- entrou na 12d.

### 12d. Remoto de rede e ramo da instancia (09/10/2026)

Decisao do orquestrador: rede so nos dois comandos que o OPERADOR digita (`exportar --push`,
`importar --pull`), nunca na ferramenta que o modelo chama. Codigo em `git.rs`
(`RemotoDeRede`, `conferir_url_de_rede`, `GitTool::empurrar_pela_rede`/`puxar_pela_rede`/
`ramo_atual`), `fluxo_git.rs` (`remoto_de_rede`, `ligar_ramo`), `fluxo_http.rs`
(`cabecalho_da_credencial`) e `apps/phxclaw/src/main.rs::fluxo_git`.

```json
{"fluxos": {"git": {"remoto": "https://git.exemplo.com/time/fluxos.git",
                    "credencial_nome": "git-fluxos", "ramo": "producao"}}}
```

- **O destino e do operador.** `fluxos.git.remoto` e chave so do operador (`SO_DO_OPERADOR`,
  motivo DESTINO; o `ramo`, CONTA): o `.git/config` do repositorio, que o modelo pode escrever,
  nunca escolhe para onde a rede vai. E mais que nao ler o `remote.origin.url`: o git que fala
  com a rede roda num ESPELHO bare temporario (0700, apagado na saida), entao `insteadOf`,
  `http.proxy`, `http.extraHeader` e `http.sslVerify` do repositorio nao valem. O repositorio
  so troca objetos com o espelho, pelo transporte local, sem rede. RED medido: o `ls-remote`
  rodando no repositorio -- o `insteadOf` hostil mandou o pedido para a armadilha.
- **So https; http so em IP de loopback** (a prova; nada sai da maquina). Usuario/senha na URL,
  query e fragmento sao recusados. **ssh fica fora**: pediria chave privada e `known_hosts` no
  sandbox. RED medido: `http` sem a guarda do loopback passou.
- **A credencial e por NOME** (`fluxos.git.credencial_nome`), a do no HTTP: declarada no
  `http.json` com as origens, segredo no broker (`phxclaw credencial guardar NOME`) ou num cofre
  externo. A mesma regra das origens: credencial que nao lista a origem do remoto e recusada
  antes de qualquer processo. RED medido: sem o `alcanca`, a credencial de outra origem foi ao
  servidor.
- **A credencial vai pelo AMBIENTE, nao pelo argv nem por arquivo.** `GIT_CONFIG_COUNT/KEY/VALUE`
  (git >= 2.31) como `http.<url-do-remoto>.extraHeader`: o cabecalho so vale para aquela URL,
  nada vai ao `.git/config` nem a um arquivo de askpass, e o argv (que `ps` mostra a qualquer
  usuario) nao o leva. O ambiente do processo e legivel so pelo mesmo usuario (`/proc/<pid>/
  environ` 0400) -- o mesmo que ja le a chave-mestra do broker. O `GIT_ASKPASS` foi recusado:
  pede um programa e o segredo num lugar que ele leia, dois lugares em vez de um. RED medido:
  com o cabecalho por `-c` (argv), o servidor falso achou o segredo em 57 `/proc/*/cmdline`
  durante os pedidos.
- **O bwrap liga ou desliga a rede INTEIRA** (`--share-net`); nao ha como prender o sandbox ao
  host do remoto, nem o `PoliticaDeSaida` alcanca um processo do sandbox (ele confere o cliente
  HTTP do proprio agente). O que se faz no lugar: rede so nesses dois comandos, destino
  conferido antes, `protocol.allow=never` mais so o esquema da URL, `http.followRedirects=false`,
  ganchos em `/dev/null`, sem submodulo, `transfer.fsckObjects` no que chega, saida do git
  tarjada pelos valores do segredo. Sem proxy de saida: o ambiente do sandbox e limpo.
- **A ferramenta do modelo continua sem rede**, por tres travas: nao ha acao de rede, o git dela
  tem `protocol.allow=never`, e o sandbox dela nao tem rede -- provada com o protocolo liberado
  de proposito. RED medido: `network: true` no motor, e a armadilha recebeu a conexao.
- **Ramo da instancia** (`fluxos.git.ramo`, o «connect to branch» do n8n): repositorio novo nasce
  nele; repositorio em outro ramo e RECUSA dizendo, nunca `checkout` calado (trocaria os arquivos
  da pasta, e o que o `importar --ambiente prod` publicaria). RED medido: sem o `ligar_ramo`, o
  push foi ao `main`.

Prova: `crates/phxclaw-agent/tests/fluxo_git_rede.rs` (4 testes) contra o servidor git HTTP
falso em loopback (`phxclaw_test_support::git_http`: o `git http-backend` de verdade atras de um
servidor de std, que exige o `Authorization` exato e conta o segredo no argv de todo processo a
cada pedido) e uma armadilha que conta conexoes; `apps/phxclaw/tests/fluxo_cli.rs::
push_e_pull_pelo_remoto_de_rede_do_operador` pela CLI (config do operador, `credencial
guardar` pela entrada padrao, ramo). Medido pelo gerador depois: n8n **93,2% no agente | 96,6%
com bibliotecas** (55 sim, 4 pela metade, 0 nao, de 59).

**Diverge do n8n:** sem ssh; ambiente e pasta (`dev/`, `prod/`) dentro do ramo, nao um ramo por
ambiente; e nao ha teste contra um GitHub/GitLab de verdade (so o `http-backend` em loopback).


## 13. Modo fila e workers (`fila_workers`, onda dos 100%, 09/10/2026)

O «queue mode» do n8n: o `phxclaw servir --modo fila` poe cada execucao numa fila no
PostgreSQL, e N processos `phxclaw worker` as tiram de la. Codigo: `crates/phxclaw-agent/src/
fila.rs`; a fila e a do `phxclaw-task-graph` (`PostgresTaskJournal`).

| O que | Como |
| --- | --- |
| Ligar | `fila.url` no `<pasta>/config.json` do operador (so do operador; sem senha na URL) + `PHXCLAW_FILA_SENHA=... phxclaw fila senha` (vai para o broker de `<pasta>/fila`); o esquema e a migracao `migrations/0004_task_graph.sql` (ou o FULL_INSTALL) -- o servir e o worker CONFEREM e recusam dizendo o arquivo |
| O que vai para a fila | tarefa de objetivo, plano (Plan Mode), disparo de fluxo (API, tela, agenda, gatilho, webhook) e retomada de espera; a definicao do fluxo viaja inteira no `payload` |
| Quem roda | o worker, pelas MESMAS funcoes do modo normal (`api::executar_aqui`, `planejar_aqui`, `rodar_fluxo_aqui`, `retomar_fluxo_aqui`); a unica decisao «fila ou aqui» mora nas portas da `api.rs` |
| Tomada | `SELECT ... FOR UPDATE SKIP LOCKED LIMIT 1` em `phoenix_tasks` (`capability` `phxclaw.*`), novo run em `phoenix_task_runs`, evento `claimed` com o nome do worker |
| Posse | o `next_eligible_at` da linha `running` (`fila.prazo_segundos`, padrao 30 s, o `QUEUE_WORKER_LOCK_DURATION` do n8n); batimento a cada terco; o `active_run_uuid` e a cerca: batimento e fim de run vencido nao valem |
| Worker morto | a posse vence, outro toma; FLUXO retoma pelo progresso gravado por onda (o passo que terminou NAO roda de novo); tarefa de objetivo recomeca (pelo menos uma vez, como o n8n). Depois de `fila.tentativas` (padrao 3) tomadas vencidas, `dead_letter` e a tarefa FALHA dizendo por que |
| Cancelar | `POST /v1/tasks/{id}/cancel`: a que nao comecou sai `cancelled`; a que corre recebe o pedido pelo batimento do worker, que aciona o MESMO `CancelFlag` |
| Concorrencia | `phxclaw worker --concorrencia N` (ou `fila.concorrencia`, padrao 4) |

**Por que o `task-graph` e nao as outras duas (escolha medida no fonte):** o agente ja depende
dele (nada novo no grafo de crates) e a tabela `phoenix_tasks` ja nasceu para isso -- o
comentario da migracao 0004 diz «workers should claim ready rows using FOR UPDATE SKIP LOCKED»,
com `status`, `attempts`, `active_run_uuid` e o run em tabela propria. O `PostgresOutbox.
claim_batch` do `phxclaw-event-bus` trava a linha SO dentro da transacao da tomada e nao marca
posse: depois do commit, um segundo `claim_batch` leva a MESMA mensagem -- serve para publicar
evento, nao para segurar uma execucao de minutos. O `BpmStore.claim_ready_token` do
`phxclaw-bpm` tem posse e cerca, mas e de token de BPMN por inquilino (`phxclaw.bpm_tokens_v2`)
e traria o `quick-xml` ao agente. As duas ficam como estao. Nenhuma coluna nova: a posse e o
proprio `next_eligible_at` («quando esta linha pode ser tomada de novo»).

**Onde DIVERGE do n8n, e a restricao nossa:**

| Divergencia | Restricao nossa |
| --- | --- |
| PostgreSQL, nao Redis/Bull | zero servidor novo: o PostgreSQL ja e o banco da instalacao, e o `SKIP LOCKED` da a mesma garantia de tomada unica |
| A execucao mora na PASTA das tarefas, compartilhada entre servir e workers (mesmo host ou volume); a fila so decide quem roda | a pasta da tarefa e a porta de disco das ferramentas (`confine`), da evidencia e dos artefatos; copiar para o banco seria a segunda casa do mesmo dado |
| Fluxo tomado de novo RETOMA por onda (o n8n reprocessa o job parado) | progresso por onda e `fluxo_sha256` ja existiam para a retomada; reexecutar passo que terminou repetiria efeito |
| Sem espera graciosa no desligar (o n8n espera `N8N_GRACEFUL_SHUTDOWN_TIMEOUT`) | o tokio do workspace nao traz `signal`; parar o worker e o mesmo caminho da queda, que e o provado |

Prova real com PostgreSQL 18 de verdade (`phxclaw_test_support::pg`: `initdb` + `pg_ctl` numa
pasta temporaria, porta livre, SCRAM; sem `initdb`, pulo registrado):
`crates/phxclaw-agent/tests/fila_workers.rs` (5 testes) e `apps/phxclaw/tests/fila_cli.rs`
(servir --modo fila + worker com SIGKILL no meio do passo, 13 s):

| Teste | RED medido (defeito reposto, `// REPOSTO`) |
| --- | --- |
| `dois_workers_nao_pegam_a_mesma_execucao` (tomada travada + 12 execucoes, 2 workers x 3 vagas: cada uma tomada 1 vez) | sem `FOR UPDATE SKIP LOCKED`: a segunda tomada esperou a linha e caiu na unicidade do run |
| `worker_morto_no_meio_outro_retoma_depois_do_prazo` | sem a retomada (`retomar = false`): o passo `A` rodou 2 vezes; posse que nao vence (`status = 'ready'`): o worker 2 nunca retoma |
| `a_posse_vencida_nao_fecha_o_run_de_outro_e_o_teto_vira_dead_letter` | sem a cerca do `active_run_uuid` no fim: «o run vencido fechou o de outro» |
| `o_resultado_pela_fila_e_o_mesmo_do_modo_normal` (relatorio passo a passo, itens, portas, sha e estado) | `entrada` perdida no worker: o relatorio diverge |
| `worker_morto_com_sigkill_outro_worker_retoma_pela_cli` | sem a retomada: `conta.txt` com 2 linhas |

NAO MEDIDO: vazao (execucoes/s por worker) -- nenhuma bancada roda contra a fila ainda; e o
banco da fila em outra maquina (so loopback).

## 14. OpenTelemetry e painel de insights (`observabilidade_insights`, 09/10/2026)

**Exportacao OTLP/HTTP com JSON** (`crates/phxclaw-agent/src/otel.rs`), sem protobuf e sem
crate nova: `otel.url` (so do operador; base do coletor, ex. `http://127.0.0.1:4318`; URL com
usuario/senha e recusada), `otel.servico` (`service.name`), `otel.intervalo_segundos` (metricas).
Desligado por padrao, e desligado nada se monta: o ponto de captura le um `AtomicBool` antes
de abrir o `task.json`. Vale no `servir` e no `worker`.

| Sinal | Quando | O que leva |
| --- | --- | --- |
| traces (`POST /v1/traces`) | execucao TERMINADA (tarefa ou fluxo; a espera exporta quando acabar) | span da tarefa/fluxo (exato: criacao a ultima gravacao) -> tarefas filhas (subagente, passo de agente com `phxclaw.passo.id`) -> chamadas de ferramenta e de modelo, cada uma com `phxclaw.evidencia.id` e `.hash` do ledger; passos do fluxo como eventos do span do fluxo. `traceId` = o UUID v7 da tarefa (16 bytes hex), `spanId` = sha256 deterministico (8 bytes hex) |
| metricas (`POST /v1/metrics`) | a cada intervalo | as MESMAS series do `/metrics` (`Metricas::series`, de onde o texto do Prometheus tambem sai): contadores como `sum` cumulativo monotono (`asInt` em texto), histogramas com `bucketCounts` POR faixa (o Prometheus guarda acumulado) |

Atributo so de conjunto fechado (`otel::ATRIBUTOS`): nem argumento de ferramenta, nem objetivo,
nem texto de erro, nem nome de credencial -- o span leva o ID da evidencia para quem precisa ir
la. Ligar o OTel conta as metricas sem abrir o `/metrics` (a rota continua do `api.metricas`).

**Painel de insights** (`GET /v1/insights?periodo=24h|7d|30d|tudo&fluxo=NOME`, `insights.rs`,
leitor + escopo de projeto na `rbac::MATRIZ`, linha em `rotas::ROTAS`; tela **Insights**,
`apps/phxclaw-ui/assets/insights.js`): execucoes por estado, taxa de falha, duracao p50/p95 (a
`avaliacao::percentil` das bancadas), custo (o nao medido conta a parte), as 5 falhas mais
comuns (motivo normalizado; com forma de credencial sai «omitido»), por fluxo e por dia. Le o
DISCO das tarefas cortado pelo `rbac::visiveis` -- o mesmo corte da lista --, nao os contadores
do processo, que zeram no reinicio.

**Onde DIVERGE do n8n:** sem «tempo poupado» (o n8n pede ao dono do fluxo os minutos poupados
por execucao; o nosso fluxo nao tem o campo, e numero inventado no painel e pior que ausencia);
sem lote de spans em memoria nem SDK (um POST por execucao terminada); o inicio de cada span de
chamada e o fim do registro anterior (o ledger grava a hora do FIM), e o span diz isso
(`phxclaw.inicio_aproximado`).

Prova: `crates/phxclaw-agent/tests/otel_insights.rs` (5 testes, coletor FALSO local em axum que
guarda os corpos) e `tests/desktop/ui_insights.mjs` (33/33 contra o agente real, 1280 e 400,
dois temas, contraste >= 4,5:1, pt/en; capturas `tests/desktop/out/ui_insights_*.png`).

| Guarda | RED medido |
| --- | --- |
| interruptor antes do trabalho | leitura do disco antes do `atual()`: `montados` = 1 |
| nada sensivel em atributo | argumento no `phxclaw.ferramenta.nome`: «dado sensivel no trace» |
| ids hex de 16/8 bytes | `spanId` de 16 bytes: reprovado |
| histograma por faixa | `bucketCounts` acumulado: soma 2 != contagem |
| painel cortado pelo projeto | sem o `rbac::visiveis`: execucoes de `beta` no painel de `alfa` |
| motivo com forma de credencial | sem o `texto_tem_credencial`: `ghp_` no JSON |
| a tela le a rota | UI sem o `insights.js`: o roteiro cai (8/9) |

NAO MEDIDO: um coletor OpenTelemetry de verdade (Collector/Jaeger/Tempo) recebendo -- a prova e o
coletor falso que confere a forma do JSON contra a especificacao.

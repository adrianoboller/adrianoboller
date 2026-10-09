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
fila com workers (pede numero de bancada). O `/metrics` entrou em 09/10 (secao 9).

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
proposito). Pull/push remoto nao existe: o repositorio e local, e quem empurra e o `git` de
sempre. Sem teste com n8n de verdade (secao 6).

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

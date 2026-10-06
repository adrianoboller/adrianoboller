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

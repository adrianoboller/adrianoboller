# Guia do operador do PhxClaw

O que configurar primeiro, onde guardar cada credencial, como confiar num projeto e como
medir. É curto de propósito: o detalhe de cada comando é a ajuda do próprio binário, copiada
aqui pelo gerador.

<!-- gerado:cabecalho:inicio -->
Trechos marcados gerados em 2026-10-02 por `python3 tools/gerar_guia_operador.py`, do binario `PhxClaw 0.70.0` (compilado em 2026-10-02 05:22).
<!-- gerado:cabecalho:fim -->

**Nada entre marcadores `<!-- gerado:… -->` se edita à mão.** Comando, opção, variável de
ambiente, comando de segredo e número saem de `python3 tools/gerar_guia_operador.py`, que
roda o binário (`phxclaw ajuda`, `config mostrar --json`, `config exemplo`, `ferramentas`) e
lê o fonte onde o binário não diz. Mudou o código, rode o gerador; quer mostrar mais um
comando, escreva o marcador `<!-- gerado:ajuda:COMANDO:inicio/fim -->` e rode.

Para o resto: [agente, ferramentas e API](AGENTE_AUTONOMO.md) ·
[estilo da tela](ui/STYLE_PHOENIX_PADRAO.md) · [segurança](../SECURITY.md) ·
[ondas e sprints](absorcao/SPRINTS.md) · catálogo em
[`schemas/config.exemplo.json`](../schemas/config.exemplo.json) e
[`schemas/config.schema.json`](../schemas/config.schema.json).

## 1. Primeiro: o `config.json`

Toda configuração passa por um catálogo só; a tela, o terminal e a API leem e gravam pelas
mesmas funções (`crates/phxclaw-agent/src/config.rs`). Quatro camadas, nesta ordem:

<!-- gerado:precedencia:inicio -->
Cabecalho de `phxclaw config exemplo`:

> config.json do PhxClaw, gerado do catálogo (phxclaw config exemplo).
> Precedência: ambiente > .phxclaw/config.json do projeto confiado > perfil ativo > <pasta>/config.json > padrão.
> null = não definido aqui. Segredo nunca entra: vai para o SecretBroker (phxclaw config mostrar diz o comando).
<!-- gerado:precedencia:fim -->

Na prática:

1. `phxclaw config exemplo` imprime o arquivo inteiro, comentado, com `null` no que não está
   definido; salve-o como `<pasta>/config.json` se ainda não houver um (a pasta é a do
   agente, `var/agente` sem `--pasta`/`PHXCLAW_HOME`). `config validar ARQ` confere antes.
2. `phxclaw config mostrar` diz o **valor efetivo e de onde veio** cada chave (ambiente,
   projeto, pasta ou padrão). Segredo aparece só como presente ou ausente no broker.
3. `phxclaw config definir CHAVE VALOR` grava com revisão e troca atômica; `--projeto` grava
   no `.phxclaw/config.json` do projeto — e só se ele for confiado (seção 3).
4. Chave desconhecida e tipo errado são erro, não aviso.

<!-- gerado:ajuda:config:inicio -->
Saida de `phxclaw ajuda config`:

```text
O config.json: valor efetivo e origem de cada chave, validar, definir

USO:
  phxclaw config mostrar [--json] | validar ARQ | exemplo | definir CHAVE VALOR|--remover [--projeto|--perfil NOME] | perfil listar|usar NOME|nenhum|criar NOME [--copiar-base] | sincronizar enviar|receber --ponte URL [--forcar] [--pasta DIR]

Precedencia: ambiente > .phxclaw/config.json do projeto confiado > perfil ativo > <pasta>/config.json > padrão (a do catalogo). mostrar: valor e origem de cada chave (segredo so aparece como no broker/ausente). definir grava atomico, com revisao. perfil: camadas nomeadas no config.json da pasta (`perfis`, `perfil_ativo`; PHXCLAW_PERFIL escolhe por uma execucao). sincronizar: o config.json vai e vem pela ponte, sem a revisao e sem segredo; os dois lados mudados e conflito com as duas revisoes. Segredo nunca vai ao arquivo: o comando que o guarda no broker vem na recusa. Chave desconhecida e tipo errado sao erro.
```
<!-- gerado:ajuda:config:fim -->

**Segredo nunca vai ao arquivo.** `definir` numa chave segredo é recusado, e a recusa traz o
comando que a guarda no SecretBroker (seção 2). Chave «só ambiente» também não se grava: o
motivo de cada uma está no catálogo.

### Pela API (`phxclaw servir`)

`GET /v1/config` devolve o mesmo que `config mostrar --json`: `revisao`, os arquivos em uso
(e o do projeto ignorado, com o comando para confiar) e cada chave com `valor`, `origem`,
`editavel` e o motivo quando não é. Segredo não traz valor nunca, só `segredo_presente`.

`PUT /v1/config` grava, com o mesmo Bearer das outras rotas:

- cabeçalho `If-Match: <revisao lida no GET>` — obrigatório; quem gravou no meio faz o seu
  `PUT` voltar com a revisão atual em vez de sobrescrever calado;
- corpo `{"escopo": "pasta" | "projeto", "valores": {"chave": valor, "outra": null}}` —
  `null` remove a chave do arquivo;
- resposta `{"revisao": N}`. Sem o cabeçalho, a recusa diz que ele falta; revisão que mudou
  volta com `revisao_atual`; chave desconhecida, tipo errado, segredo e projeto não confiado
  voltam com a lista de erros por chave.

<!-- gerado:http-config:inicio -->
Codigos que as rotas de `crates/phxclaw-agent/src/config.rs` devolvem, na ordem em que aparecem no fonte (alem do 200 e do 401 do Bearer):

| Codigo | Nome |
|---|---|
| 409 | `CONFLICT` |
| 422 | `UNPROCESSABLE_ENTITY` |
| 500 | `INTERNAL_SERVER_ERROR` |
| 428 | `PRECONDITION_REQUIRED` |
<!-- gerado:http-config:fim -->

## 2. Credenciais

Credencial vai para o **SecretBroker** da pasta do agente pelo comando dela, nunca para o
`config.json`. O comando **lê a variável do ambiente naquele momento** e a guarda; depois
disso ela pode sair do ambiente. Para não deixar o valor no histórico do shell:

```bash
read -rs PHXCLAW_GITHUB_TOKEN && export PHXCLAW_GITHUB_TOKEN
phxclaw forja token github
unset PHXCLAW_GITHUB_TOKEN
```

A lista completa sai do catálogo, e ela diz também quais segredos **ainda não têm** comando
de broker (ficam na variável de ambiente ou num arquivo da pasta):

<!-- gerado:segredos:inicio -->
De `phxclaw config mostrar --json`: **52 segredos no catalogo**; 52 tem comando que os guarda no SecretBroker, 0 ainda nao.

<details><summary>Os segredos, um por linha</summary>

| Chave | Variavel | Como guardar |
|---|---|---|
| `anthropic.chave` | `PHXCLAW_ANTHROPIC_API_KEY` | `phxclaw anthropic chave` |
| `api.token` | `PHXCLAW_API_TOKEN` | `phxclaw api chave` |
| `canais.discord.token` | `PHXCLAW_DISCORD_TOKEN` | `PHXCLAW_DISCORD_TOKEN=... phxclaw canal discord (vai ao broker na primeira vez)` |
| `canais.email.senha` | `PHXCLAW_EMAIL_CANAL_SENHA` | `PHXCLAW_EMAIL_CANAL_SENHA=... phxclaw canal email (vai ao broker na primeira vez)` |
| `canais.email.smtp_senha` | `PHXCLAW_EMAIL_CANAL_SMTP_SENHA` | `PHXCLAW_EMAIL_CANAL_SMTP_SENHA=... phxclaw canal email (vai ao broker na primeira vez)` |
| `canais.feishu.app_secret` | `PHXCLAW_FEISHU_APP_SECRET` | `PHXCLAW_FEISHU_APP_SECRET=... phxclaw canal feishu (vai ao broker na primeira vez)` |
| `canais.feishu.verificacao` | `PHXCLAW_FEISHU_VERIFICACAO` | `PHXCLAW_FEISHU_VERIFICACAO=... phxclaw canal feishu (vai ao broker na primeira vez)` |
| `canais.googlechat.chave_url` | `PHXCLAW_GOOGLECHAT_CHAVE_URL` | `PHXCLAW_GOOGLECHAT_CHAVE_URL=... phxclaw canal googlechat (vai ao broker na primeira vez)` |
| `canais.googlechat.saida_webhook` | `PHXCLAW_GOOGLECHAT_SAIDA_WEBHOOK` | `PHXCLAW_GOOGLECHAT_SAIDA_WEBHOOK=... phxclaw canal googlechat (vai ao broker na primeira vez)` |
| `canais.irc.senha` | `PHXCLAW_IRC_SENHA` | `PHXCLAW_IRC_SENHA=... phxclaw canal irc (vai ao broker na primeira vez)` |
| `canais.line.segredo_canal` | `PHXCLAW_LINE_SEGREDO_CANAL` | `PHXCLAW_LINE_SEGREDO_CANAL=... phxclaw canal line (vai ao broker na primeira vez)` |
| `canais.line.token` | `PHXCLAW_LINE_TOKEN` | `PHXCLAW_LINE_TOKEN=... phxclaw canal line (vai ao broker na primeira vez)` |
| `canais.mastodon.token` | `PHXCLAW_MASTODON_TOKEN` | `PHXCLAW_MASTODON_TOKEN=... phxclaw canal mastodon (vai ao broker na primeira vez)` |
| `canais.matrix.token` | `PHXCLAW_MATRIX_TOKEN` | `PHXCLAW_MATRIX_TOKEN=... phxclaw canal matrix (vai ao broker na primeira vez)` |
| `canais.mattermost.token` | `PHXCLAW_MATTERMOST_TOKEN` | `PHXCLAW_MATTERMOST_TOKEN=... phxclaw canal mattermost (vai ao broker na primeira vez)` |
| `canais.messenger.app_secret` | `PHXCLAW_MESSENGER_APP_SECRET` | `PHXCLAW_MESSENGER_APP_SECRET=... phxclaw canal messenger (vai ao broker na primeira vez)` |
| `canais.messenger.token` | `PHXCLAW_MESSENGER_TOKEN` | `PHXCLAW_MESSENGER_TOKEN=... phxclaw canal messenger (vai ao broker na primeira vez)` |
| `canais.messenger.verify_token` | `PHXCLAW_MESSENGER_VERIFY_TOKEN` | `PHXCLAW_MESSENGER_VERIFY_TOKEN=... phxclaw canal messenger (vai ao broker na primeira vez)` |
| `canais.nostr.chave` | `PHXCLAW_NOSTR_CHAVE` | `PHXCLAW_NOSTR_CHAVE=... phxclaw canal nostr (vai ao broker na primeira vez)` |
| `canais.reddit.app_secret` | `PHXCLAW_REDDIT_APP_SECRET` | `PHXCLAW_REDDIT_APP_SECRET=... phxclaw canal reddit (vai ao broker na primeira vez)` |
| `canais.reddit.senha` | `PHXCLAW_REDDIT_SENHA` | `PHXCLAW_REDDIT_SENHA=... phxclaw canal reddit (vai ao broker na primeira vez)` |
| `canais.rocketchat.token` | `PHXCLAW_ROCKETCHAT_TOKEN` | `PHXCLAW_ROCKETCHAT_TOKEN=... phxclaw canal rocketchat (vai ao broker na primeira vez)` |
| `canais.signal.token` | `PHXCLAW_SIGNAL_TOKEN` | `PHXCLAW_SIGNAL_TOKEN=... phxclaw canal signal (vai ao broker na primeira vez)` |
| `canais.slack.token` | `PHXCLAW_SLACK_TOKEN` | `PHXCLAW_SLACK_TOKEN=... phxclaw canal slack (vai ao broker na primeira vez)` |
| `canais.sms.token` | `PHXCLAW_SMS_TOKEN` | `PHXCLAW_SMS_TOKEN=... phxclaw canal sms (vai ao broker na primeira vez)` |
| `canais.teams.app_secret` | `PHXCLAW_TEAMS_APP_SECRET` | `PHXCLAW_TEAMS_APP_SECRET=... phxclaw canal teams (vai ao broker na primeira vez)` |
| `canais.teams.chave_url` | `PHXCLAW_TEAMS_CHAVE_URL` | `PHXCLAW_TEAMS_CHAVE_URL=... phxclaw canal teams (vai ao broker na primeira vez)` |
| `canais.telegram.bot_token` | `PHXCLAW_TELEGRAM_BOT_TOKEN` | `PHXCLAW_TELEGRAM_BOT_TOKEN=... phxclaw canal telegram (vai ao broker na primeira vez)` |
| `canais.twitch.senha` | `PHXCLAW_TWITCH_SENHA` | `PHXCLAW_TWITCH_SENHA=... phxclaw canal twitch (vai ao broker na primeira vez)` |
| `canais.viber.token` | `PHXCLAW_VIBER_TOKEN` | `PHXCLAW_VIBER_TOKEN=... phxclaw canal viber (vai ao broker na primeira vez)` |
| `canais.webchat.chave` | `PHXCLAW_WEBCHAT_CHAVE` | `PHXCLAW_WEBCHAT_CHAVE=... phxclaw canal webchat (vai ao broker na primeira vez)` |
| `canais.webhook.segredo` | `PHXCLAW_WEBHOOK_SEGREDO` | `PHXCLAW_WEBHOOK_SEGREDO=... phxclaw canal webhook (vai ao broker na primeira vez)` |
| `canais.whatsapp.app_secret` | `PHXCLAW_WHATSAPP_APP_SECRET` | `PHXCLAW_WHATSAPP_APP_SECRET=... phxclaw canal whatsapp (vai ao broker na primeira vez)` |
| `canais.whatsapp.token` | `PHXCLAW_WHATSAPP_TOKEN` | `PHXCLAW_WHATSAPP_TOKEN=... phxclaw canal whatsapp (vai ao broker na primeira vez)` |
| `canais.whatsapp.verify_token` | `PHXCLAW_WHATSAPP_VERIFY_TOKEN` | `PHXCLAW_WHATSAPP_VERIFY_TOKEN=... phxclaw canal whatsapp (vai ao broker na primeira vez)` |
| `canais.xmpp.senha` | `PHXCLAW_XMPP_SENHA` | `PHXCLAW_XMPP_SENHA=... phxclaw canal xmpp (vai ao broker na primeira vez)` |
| `canais.zulip.chave` | `PHXCLAW_ZULIP_CHAVE` | `PHXCLAW_ZULIP_CHAVE=... phxclaw canal zulip (vai ao broker na primeira vez)` |
| `dispositivos.token_pareamento` | `PHXCLAW_ENROLLMENT_TOKEN` | `phxclaw dispositivos chave` |
| `elevenlabs.chave` | `PHXCLAW_ELEVENLABS_API_KEY` | `phxclaw elevenlabs chave` |
| `email.smtp.senha` | `PHXCLAW_SMTP_PASSWORD` | `phxclaw email chave` |
| `forja.github.token` | `PHXCLAW_GITHUB_TOKEN` | `phxclaw forja token github` |
| `forja.gitlab.token` | `PHXCLAW_GITLAB_TOKEN` | `phxclaw forja token gitlab` |
| `gemini.chave` | `PHXCLAW_GEMINI_API_KEY` | `phxclaw gemini chave` |
| `imagem.chave` | `PHXCLAW_IMAGEM_CHAVE` | `phxclaw imagem chave` |
| `mcp.segredo_cliente` | `PHXCLAW_MCP_SEGREDO_CLIENTE` | `phxclaw mcp login NOME` |
| `mcp.token` | `PHXCLAW_MCP_TOKEN` | `phxclaw mcp token NOME` |
| `n8n.chave` | `PHXCLAW_N8N_API_KEY` | `phxclaw n8n chave` |
| `n8n.webhook_segredo` | `PHXCLAW_N8N_WEBHOOK_SEGREDO` | `phxclaw n8n segredo` |
| `openai.chave` | `PHXCLAW_OPENAI_API_KEY` | `phxclaw openai chave` |
| `plugins.chave_assinatura` | `PHXCLAW_PLUGIN_SIGNING_KEY` | `phxclaw plugins chave` |
| `ponte.token` | `PHXCLAW_PONTE_TOKEN` | `phxclaw ponte chave` |
| `xai.chave` | `PHXCLAW_XAI_API_KEY` | `phxclaw xai chave` |

</details>
<!-- gerado:segredos:fim -->

### Canal XMPP: a sala multiusuário (MUC, XEP-0045)

Desde o commit `4802e21b` (02/10/2026) o canal `xmpp` entra em salas, além do `chat` a dois.
Fonte: `crates/phxclaw-agent/src/canais/xmpp.rs` (692 linhas, `wc -l`) e as duas chaves
novas do catálogo (`crates/phxclaw-config-runtime/src/agente/catalogo.rs`, `CanalDef`
`xmpp`), que o gerador já espalhou pela tabela acima, por `schemas/config.exemplo.json` e
pelo `config-catalogo.json` da tela:

| Chave do catálogo | Variável | Tipo | O que faz |
| --- | --- | --- | --- |
| `canais.xmpp.salas` (`SALAS`) | `PHXCLAW_XMPP_SALAS` | lista | JIDs das salas em que o bot entra ao abrir a conexão. Vazia = comportamento de antes (só `chat`). |
| `canais.xmpp.apelido` (`APELIDO`) | `PHXCLAW_XMPP_APELIDO` | texto | nick nas salas; vazio = a parte local do JID. |
| `canais.xmpp.permitidos` (`PERMITIDOS`) | `PHXCLAW_XMPP_PERMITIDOS` | lista | já existia; **a sala também vai aqui**, senão o que vem dela é descartado. |

```sh
PHXCLAW_XMPP_SALAS=ops@conference.exemplo.com \
PHXCLAW_XMPP_APELIDO=phxclaw \
PHXCLAW_XMPP_PERMITIDOS=ops@conference.exemplo.com,adriano@exemplo.com \
phxclaw canal xmpp
```

O que o canal faz com o que chega da sala, e o que ignora de propósito
(`mensagem_da_estrofe`, `eco`, `erro_da_presenca`):

- **`groupchat` vira tarefa** com a sala como conversa e o nick como autor; a resposta volta
  `groupchat` à sala inteira (XEP-0045 §7.4).
- **Chat privado de ocupante** (`type='chat'` vindo de `sala/nick`) chega com a conversa
  `sala/nick` inteira, e a resposta volta `chat` só a ele — nunca em público. Só entra se
  `sala/nick` estiver em `PERMITIDOS`.
- **Ignorado, com o motivo:** o **eco** da própria fala (a sala reflete a todos, inclusive a
  quem falou; nick igual ao apelido), o **histórico** que a sala reenvia ao entrar
  (`<delay/>`, `urn:xmpp:delay` ou o antigo `jabber:x:delay` — senão o agente responderia ao
  passado) e o **aviso da própria sala** (mensagem sem nick: assunto, aviso de serviço).
  Estado de digitação sem `<body>` também não é mensagem.
- **Erro ao entrar vira motivo legível**, e a entrada **espera** a confirmação da sala (a
  presença refletida com o nosso nick ou `status 110`): sem esperar, um 409 chegaria depois,
  misturado ao fluxo, e ninguém o veria. Os motivos: `conflict` (409) «o apelido já está em
  uso na sala», `item-not-found` «a sala não existe», `not-authorized` «a sala pede senha»,
  `forbidden` «o agente está banido da sala», `registration-required` «a sala só aceita
  membros», `service-unavailable` «a sala está lotada», `not-acceptable` «a sala não aceita
  esse apelido», `jid-malformed`; condição desconhecida sai como `erro <code>`.

**Consequência de permissão, para o operador decidir sabendo:** a lista de permitidos
confere a **conversa**, e na sala a conversa é a sala. **Sala em `PERMITIDOS` = qualquer
ocupante da sala comanda o agente.** Filtrar por ocupante dentro da sala é decisão de produto
que não foi tomada (está nas pendências da sprint `docs/sprints/Sessao_00001_Sprint_SP000035_20261002060631.md`);
até lá, ponha em `SALAS` só sala cujos membros você deixaria falar com o agente, ou use a
privada de ocupante (`sala/nick` em `PERMITIDOS`), que é por pessoa.

Ainda não existe: senha de sala e troca de nick no MUC (pendência declarada). Prova:
`crates/phxclaw-agent/tests/canais.rs` (`xmpp_entra_na_sala_ouve_so_os_outros_e_responde_em_groupchat`,
`xmpp_nick_em_conflito_na_sala_e_erro_legivel`) e o teste de unidade
`groupchat_vira_entrada_com_nick_e_eco_historico_e_aviso_nao` no próprio `xmpp.rs`, contra um
servidor XMPP falso; nenhum servidor MUC real foi exercitado.

### Forjas (GitHub, GitLab)

As ferramentas `github`/`gitlab` **não existem** antes do token guardado; `GITHUB_TOKEN` e
`GH_TOKEN` do ambiente não são lidos.

<!-- gerado:ajuda:forja:inicio -->
Saida de `phxclaw ajuda forja`:

```text
Guarda o token do GitHub ou do GitLab

USO:
  phxclaw forja token github|gitlab [--pasta DIR]

Le PHXCLAW_GITHUB_TOKEN / PHXCLAW_GITLAB_TOKEN do AMBIENTE do comando e guarda no broker de <pasta>/forja; as ferramentas github e gitlab leem SO do broker e so existem depois disto.

Tambem aceito como: forge
```
<!-- gerado:ajuda:forja:fim -->

### Servidores MCP remotos (Linear, Google)

O operador declara os servidores num arquivo JSON cujo caminho vai em `mcp.config`
(`PHXCLAW_MCP_CONFIG`). Cada servidor vira ferramentas `mcp__<servidor>__<ferramenta>` com a
capacidade `mcp.<servidor>`, fora do padrão. O arquivo guarda **o tipo** da credencial e os
endpoints, nunca o valor:

```json
{
  "servidores": [
    { "nome": "linear", "preset": "linear" },
    { "nome": "gmail", "preset": "gmail",
      "auth": { "tipo": "oauth", "cliente_id": "SEU-CLIENTE.apps.googleusercontent.com" } }
  ]
}
```

O `preset` preenche URL, tipo de credencial e endpoints que faltarem; o que a declaração
trouxer vale mais. O `cliente_id` do OAuth é sempre do operador.

<!-- gerado:mcp-presets:inicio -->
Dos bracos de `fn preset` em `crates/phxclaw-agent/src/mcp.rs`: 4 preset(s).

| `preset` | URL | Credencial | Comando |
|---|---|---|---|
| `linear` | `https://mcp.linear.app/mcp` | Bearer fixo | `phxclaw mcp token linear` |
| `gmail` | `https://gmailmcp.googleapis.com/mcp/v1` | OAuth 2.0 + PKCE (Google; `cliente_id` do operador) | `phxclaw mcp login gmail` |
| `drive` | `https://drivemcp.googleapis.com/mcp/v1` | OAuth 2.0 + PKCE (Google; `cliente_id` do operador) | `phxclaw mcp login drive` |
| `calendar` | `https://calendarmcp.googleapis.com/mcp/v1` | OAuth 2.0 + PKCE (Google; `cliente_id` do operador) | `phxclaw mcp login calendar` |
<!-- gerado:mcp-presets:fim -->

- **Bearer (Linear):** `phxclaw mcp token linear` guarda a chave de API.
- **OAuth (Google):** `phxclaw mcp login gmail` imprime a URL de autorização; abra-a no
  navegador desta máquina. A volta é por `http://127.0.0.1:<porta>/callback`, com PKCE S256
  e `state` conferido; o refresh token fica no broker e a renovação é automática. O PhxClaw
  **não abre o navegador** sozinho (exigiria processo fora do sandbox). Os escopos do preset
  são só de leitura; escrever é escopo que o operador acrescenta em `auth.escopos`, sabendo.

<!-- gerado:ajuda:mcp:inicio -->
Saida de `phxclaw ajuda mcp`:

```text
Credencial de um servidor MCP remoto (Bearer ou OAuth)

USO:
  phxclaw mcp token|login NOME [--pasta DIR]

Servidor declarado em PHXCLAW_MCP_CONFIG: token le PHXCLAW_MCP_TOKEN do AMBIENTE do comando e o guarda como Bearer (Linear); login faz OAuth 2.0 + PKCE no navegador local e guarda o refresh token (Google; segredo do cliente lido de PHXCLAW_MCP_SEGREDO_CLIENTE). O agente le SO do broker de <pasta>/mcp, por (nome, URL): URL nova pede credencial nova.
```
<!-- gerado:ajuda:mcp:fim -->

### Serviços pagos (ElevenLabs, Gemini, xAI)

<!-- gerado:ajuda:elevenlabs:inicio -->
Saida de `phxclaw ajuda elevenlabs`:

```text
Guarda a chave da ElevenLabs ou lista as vozes da conta

USO:
  phxclaw elevenlabs chave|vozes [--busca TEXTO] [--pasta DIR]

chave: le PHXCLAW_ELEVENLABS_API_KEY do AMBIENTE do comando e guarda no broker de <pasta>/elevenlabs; speak, transcribe e voice_list leem SO do broker (PHXCLAW_TTS_PROVEDOR=elevenlabs, PHXCLAW_ELEVENLABS_VOZ, PHXCLAW_STT_PROVEDOR=elevenlabs). vozes: lista voice_id, nome e categoria.
```
<!-- gerado:ajuda:elevenlabs:fim -->

<!-- gerado:ajuda:gemini:inicio -->
Saida de `phxclaw ajuda gemini`:

```text
Guarda a chave da Gemini API (Nano Banana no image_generate)

USO:
  phxclaw gemini chave [--pasta DIR]

Le PHXCLAW_GEMINI_API_KEY (ou GEMINI_API_KEY) do AMBIENTE do comando e guarda no broker de <pasta>/gemini; image_generate le SO do broker, pelo Nano Banana com PHXCLAW_IMAGEM_PROVEDOR=nanobanana.
```
<!-- gerado:ajuda:gemini:fim -->

<!-- gerado:ajuda:xai:inicio -->
Saida de `phxclaw ajuda xai`:

```text
Guarda a chave da xAI (habilita x_search)

USO:
  phxclaw xai chave [--pasta DIR]

Le PHXCLAW_XAI_API_KEY do AMBIENTE do comando e guarda no broker de <pasta>/xai; x_search le SO do broker (capacidade x.search, fora do padrao).
```
<!-- gerado:ajuda:xai:fim -->

## 3. Confiar num projeto

Sem confiança, o projeto é **dado**, não instrução: clonar um repositório não basta para
escrever no prompt de quem o abre. Só depois de `phxclaw projeto confiar DIR`:

- os `AGENTS.md` da raiz do repositório até a pasta corrente entram no prompt (com teto e
  varredura anti-injeção; o que casa um padrão de ataque fica de fora e o bloco diz qual);
- o `.phxclaw/config.json` do projeto entra na precedência (seção 1) e o `config definir
  --projeto` passa a gravar nele;
- `phxclaw servir` lê os gatilhos, hooks e regras de `.phxclaw/`.

`phxclaw projeto mostrar` exibe o bloco exato que vai ao prompt. Quem roda o agente num
projeto não confiado vê o aviso com o comando para confiar.

<!-- gerado:ajuda:projeto:inicio -->
Saida de `phxclaw ajuda projeto`:

```text
Confia num projeto: os AGENTS.md dele entram no prompt

USO:
  phxclaw projeto confiar|mostrar [DIR] [--pasta DIR]

AGENTS.md da raiz do repositorio ate a pasta (AGENTS.override.md substitui, CLAUDE.md e reserva), teto de 32 KiB, varredura anti-injecao. So projeto confiado e lido; mostrar exibe o bloco que vai ao prompt.

Tambem aceito como: project
```
<!-- gerado:ajuda:projeto:fim -->

## 4. Contexto: skills, documentos, pesquisa e imagem

<!-- gerado:ajuda:skills:inicio -->
Saida de `phxclaw ajuda skills`:

```text
Importa SKILL.md de outros agentes (scripts desligados, origem com SHA-256)

USO:
  phxclaw skills importar DIR [--com-scripts] [--pasta DIR]

Le todo SKILL.md abaixo de DIR (Claude Code, Codex, OpenClaw, Hermes), traduz os nomes de ferramenta conhecidos e grava na pasta de skills. scripts/ nao se copia sem --com-scripts; ORIGEM.json guarda a origem e o SHA-256.
```
<!-- gerado:ajuda:skills:fim -->

<!-- gerado:ajuda:indexar:inicio -->
Saida de `phxclaw ajuda indexar`:

```text
Indexa uma pasta de documentos para o doc_search (BM25)

USO:
  phxclaw indexar DIR [--pasta DIR]

Indexa os arquivos de texto de DIR (BM25 por paragrafo) em <pasta>/indice-docs; reindexar a mesma pasta substitui so ela. doc_search pede doc.read; PHXCLAW_DOCS_EMBED=ollama:all-minilm reordena por embedding.

Tambem aceito como: index
```
<!-- gerado:ajuda:indexar:fim -->

<!-- gerado:chaves:documentos:inicio -->
De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): 1 chave(s) em `documentos`.

| Chave | Variavel | Tipo | Padrao | Natureza | O que e |
|---|---|---|---|---|---|
| `documentos.embed` | `PHXCLAW_DOCS_EMBED` | texto | — | config | Modelo de embedding da reordenação (só ollama:) |
<!-- gerado:chaves:documentos:fim -->

O `doc_search` só existe depois do primeiro `phxclaw indexar`: sem índice na pasta do agente
a ferramenta não monta. Imagem junto do objetivo: `phxclaw agente "…" --imagem tela.png` (veja a ajuda do `agente`
na seção 7). As ferramentas de contexto, como o binário as monta nesta máquina:

<!-- gerado:ferramentas:doc_search,deep_research:inicio -->
| Ferramenta | Capacidade | Padrao | Descricao (a que o modelo le) |
|---|---|---|---|
| `doc_search` | — | — | **NAO MONTADA nesta maquina** (depende de configuracao, chave, indice ou feature; a condicao esta em `crates/phxclaw-agent/src/montagem.rs`); definida em `crates/phxclaw-agent/src/documentos.rs` |
| `deep_research` | `web.research` | sim | Research a question on the web: plans queries, searches, reads the pages and answers with citations. Each citation is checked as a LITERAL passage of a page actually read; invented citations are rejected and reported. |
<!-- gerado:ferramentas:doc_search,deep_research:fim -->

## 5. Voz e imagem pagas

Sem configuração, a fala é o comando local (piper ou equivalente), a transcrição é o
whisper.cpp local e a imagem é só o SVG desenhado pelo Chromium. O provedor pago entra por
chave do catálogo, e a credencial dele pelo comando da seção 2.

<!-- gerado:chaves:voz,elevenlabs:inicio -->
De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): 20 chave(s) em `voz,elevenlabs`.

| Chave | Variavel | Tipo | Padrao | Natureza | O que e |
|---|---|---|---|---|---|
| `elevenlabs.api` | `PHXCLAW_ELEVENLABS_API` | texto | https://api.elevenlabs.io | config | Base da API da ElevenLabs |
| `elevenlabs.chave` | `PHXCLAW_ELEVENLABS_API_KEY` | texto | — | segredo: `phxclaw elevenlabs chave` | Chave da ElevenLabs |
| `elevenlabs.estabilidade` | `PHXCLAW_ELEVENLABS_ESTABILIDADE` | real | — | config | stability da voz (0 a 1) |
| `elevenlabs.formato` | `PHXCLAW_ELEVENLABS_FORMATO` | texto | wav_16000 | config | Formato de saída da fala (wav_16000, pcm_22050...) |
| `elevenlabs.modelo` | `PHXCLAW_ELEVENLABS_MODELO` | texto | — | config | Modelo de fala da ElevenLabs |
| `elevenlabs.similaridade` | `PHXCLAW_ELEVENLABS_SIMILARIDADE` | real | — | config | similarity_boost da voz (0 a 1) |
| `elevenlabs.stt_modelo` | `PHXCLAW_ELEVENLABS_STT_MODELO` | texto | scribe_v2 | config | Modelo de transcrição da ElevenLabs |
| `elevenlabs.voz` | `PHXCLAW_ELEVENLABS_VOZ` | texto | — | config | voice_id da ElevenLabs (obrigatório com voz.tts.provedor=elevenlabs) |
| `voz.kws.bin` | `PHXCLAW_KWS_BIN` | caminho | — | config | Executável da palavra de ativação |
| `voz.kws.modelo_dir` | `PHXCLAW_KWS_MODEL_DIR` | caminho | — | config | Pasta do modelo da palavra de ativação |
| `voz.kws.modelo_sha256` | `PHXCLAW_KWS_MODEL_SHA256` | texto | — | config | SHA-256 esperado do modelo da palavra de ativação |
| `voz.stt.provedor` | `PHXCLAW_STT_PROVEDOR` | enum: whisper \| elevenlabs | whisper | config | Quem transcreve: o whisper.cpp local ou a ElevenLabs |
| `voz.tts.comando` | `PHXCLAW_TTS_COMMAND` | texto | — | config | Comando de fala local (piper ou equivalente) |
| `voz.tts.modelo` | `PHXCLAW_TTS_MODEL` | caminho | — | config | Modelo do comando de fala |
| `voz.tts.modelo_sha256` | `PHXCLAW_TTS_MODEL_SHA256` | texto | — | config | SHA-256 esperado do modelo de fala |
| `voz.tts.pastas` | `PHXCLAW_TTS_DIRS` | lista | — | config | Pastas em que a fala pode gravar (separadas por : no ambiente) |
| `voz.tts.provedor` | `PHXCLAW_TTS_PROVEDOR` | enum: comando \| elevenlabs | comando | config | Quem fala: o comando local ou a ElevenLabs |
| `voz.whisper.bin` | `PHXCLAW_WHISPER_BIN` | caminho | — | config | Executável do whisper.cpp |
| `voz.whisper.modelo` | `PHXCLAW_WHISPER_MODEL` | caminho | — | config | Modelo do whisper.cpp |
| `voz.whisper.modelo_sha256` | `PHXCLAW_WHISPER_MODEL_SHA256` | texto | — | config | SHA-256 esperado do modelo do whisper |
<!-- gerado:chaves:voz,elevenlabs:fim -->

<!-- gerado:chaves:imagem,gemini:inicio -->
De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): 7 chave(s) em `imagem,gemini`.

| Chave | Variavel | Tipo | Padrao | Natureza | O que e |
|---|---|---|---|---|---|
| `gemini.chave` | `PHXCLAW_GEMINI_API_KEY` | texto | — | segredo: `phxclaw gemini chave` | Chave do Gemini (nanobanana) |
| `imagem.chave` | `PHXCLAW_IMAGEM_CHAVE` | texto | — | segredo: `phxclaw imagem chave` | Chave do gerador de imagem openai |
| `imagem.comfy_fluxo` | `PHXCLAW_COMFY_WORKFLOW` | caminho | — | config | Fluxo do ComfyUI (obrigatório com imagem.provedor=comfyui) |
| `imagem.entrada_pixels_max` | `PHXCLAW_IMAGEM_ENTRADA_PIXELS_MAX` | inteiro | 40000000 | config | Teto de pixels (largura x altura) de imagem de entrada, lido do cabeçalho antes de decodificar |
| `imagem.modelo` | `PHXCLAW_IMAGEM_MODELO` | texto | — | config | Modelo do gerador de imagem (openai: gpt-image-1) |
| `imagem.provedor` | `PHXCLAW_IMAGEM_PROVEDOR` | enum: openai \| comfyui \| nanobanana | — | config | Gerador de imagem; vazio = só o SVG local |
| `imagem.url` | `PHXCLAW_IMAGEM_URL` | texto | — | config | Base do gerador de imagem (openai: https://api.openai.com) |
<!-- gerado:chaves:imagem,gemini:fim -->

<!-- gerado:ferramentas:speak,transcribe,voice_list,image_generate:inicio -->
| Ferramenta | Capacidade | Padrao | Descricao (a que o modelo le) |
|---|---|---|---|
| `speak` | `media.tts` | **nao** | Text to speech: synthesize 'text' into a .wav file in the task folder (default fala.wav) with the configured local voice. |
| `transcribe` | `media.stt` | **nao** | Speech to text (whisper.cpp) of a .wav audio file in the task folder (16 kHz mono works best). Optional 'language' (e.g. en, pt, auto). |
| `voice_list` | — | — | **NAO MONTADA nesta maquina** (depende de configuracao, chave, indice ou feature; a condicao esta em `crates/phxclaw-agent/src/montagem.rs`); definida em `crates/phxclaw-agent/src/elevenlabs.rs` |
| `image_generate` | `media.generate` | **nao** | Generate a .png image in the task folder. From 'prompt' via no image server configured: pass 'svg' markup; or, always available, from 'svg' markup you write, drawn by headless Chromium. |
<!-- gerado:ferramentas:speak,transcribe,voice_list,image_generate:fim -->

<!-- gerado:ajuda:voz:inicio -->
Saida de `phxclaw ajuda voz`:

```text
Conversa por voz, um turno por arquivo WAV

USO:
  phxclaw voz ARQ.wav [ARQ2.wav ...] [--modelo M] [--pasta DIR]

Conversa por voz, um turno por WAV: escuta -> agente -> WAV falado (PHXCLAW_WHISPER_* e PHXCLAW_TTS_*; PHXCLAW_STT_PROVEDOR=elevenlabs e PHXCLAW_TTS_PROVEDOR=elevenlabs trocam o motor pela ElevenLabs).

Tambem aceito como: voice
```
<!-- gerado:ajuda:voz:fim -->

## 6. Segredo no commit (gitleaks)

Todo `add` e `commit` do `git_write` — o do modelo, o do `mcp-serve` e o do
`parallel_tasks` — passa por uma varredura com o binário oficial do gitleaks antes de tocar no
índice. O gitleaks não roda git nem lê `.gitleaks.toml`/`.gitleaksignore` da árvore (que o
modelo escreve) e ignora o comentário `gitleaks:allow`; o achado sai redigido, só arquivo,
linha e regra (`crates/phxclaw-agent/src/segredos.rs`).

<!-- gerado:chaves:git.segredos:inicio -->
De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): 3 chave(s) em `git.segredos`.

| Chave | Variavel | Tipo | Padrao | Natureza | O que e |
|---|---|---|---|---|---|
| `git.segredos.exigir` | `PHXCLAW_GITLEAKS_EXIGIR` | booleano | false | config | Recusa add/commit do git_write quando a varredura de segredos não pode ser feita |
| `git.segredos.gitleaks_bin` | `PHXCLAW_GITLEAKS_BIN` | caminho | — | config | Executável do gitleaks que varre add/commit do git_write (sem ele, a varredura não é feita e o resultado diz isso) |
| `git.segredos.gitleaks_sha256` | `PHXCLAW_GITLEAKS_SHA256` | texto | — | config | SHA-256 esperado do executável do gitleaks; sem ele o binário não roda |
<!-- gerado:chaves:git.segredos:fim -->

| Situação | O que acontece |
|---|---|
| sem `gitleaks_bin` | a gravação segue, e o resultado do `add`/`commit` diz que a varredura **não foi feita** e por quê |
| sem `gitleaks_bin` e `exigir` ligado | a gravação é **recusada** |
| binário sem `gitleaks_sha256`, hash que não confere, binário ausente, sandbox que falha | **recusada**: guarda ligada que não roda não vira guarda desligada |
| achado | **recusada**, com arquivo:linha (regra) de cada achado |
| diff acima do teto (`DIFF_MAX_BYTES`) | **recusada**: varrer só o começo e dizer «limpo» seria a pior resposta |

## 7. Medir: gravar, repetir, medir, avaliar, otimizar skill

Todo número sai com faixa min–max, N e data; vencedor e promoção só quando as faixas não se
cruzam. Energia só de RAPL ou NVIDIA — sem eles, «não medida», nunca estimada.

O que a gravação carrega por passo desde a versão 2 do formato (SP000030, `gravacao.rs`):
`duracao_ms` de cada pedido ao modelo e de cada ferramenta; `tokens_entrada`/`tokens_saida`
**só quando o provedor os devolveu** (roteiro e repetição não devolvem, e o campo fica ausente —
nunca um número estimado com cara de medido); `tarefa` e `passo_pai` na chamada que roda dentro
da chamada de outra tarefa (subagente); e, antes do primeiro pedido, uma linha `prompt` com o
`prompt_sha256` do sistema (base + instruções do projeto + memória + skills listadas) e o
`skills_sha256` de cada `SKILL.md` da pasta. Gravação da versão 1 continua lendo, com as medidas
ausentes. `phxclaw medir ARQ.jsonl` soma por tarefa; `--json` dá a soma crua.

O `avaliar`, além do acerto (tudo ou nada), dá a **nota parcial de ferramentas** quando o caso
tem gabarito de sequência: `conjunto` = esperadas chamadas / esperadas, `sequencia` = maior
subsequência comum na ordem / esperadas (a *ToolCorrectness* do DeepEval, determinística — nunca
um modelo julgando outro). Por caso no `resultado.json`, mediana dos casos por rodada com a
faixa entre rodadas na tabela, e `nota_sequencia` entra no vencedor pela mesma regra das faixas.
As execuções saem ainda **agrupadas por `prompt_sha256` e `skills_sha256`** (`por_prompt`):
antes/depois de mexer no prompt ou numa skill, compare só dentro do mesmo sha.

Memória que contradiz outra não se apaga: `memory_save` com `substitui: id` grava a nova e marca
a antiga com `invalid_at` e `substituida_por` (ver `memory_search`, que devolve o id de cada
nota e só mostra as inválidas com `include_invalid`). O prompt injeta só o que vale.

<!-- gerado:ajuda:medir:inicio -->
Saida de `phxclaw ajuda medir`:

```text
Soma uma gravacao por tarefa: chamadas, duracao e tokens de cada passo

USO:
  phxclaw medir ARQ.jsonl [--json]

Le uma gravacao (agente --gravar, avaliar, gravar:true na API) e soma por tarefa -- a raiz e cada subagente pelo passo_pai -- as chamadas ao modelo e as ferramentas, a duracao de cada lado e os tokens. Tokens so somam quando o provedor os informou em toda chamada; senao diz quantas ficaram sem. Gravacao da versao 1 (sem medidas) sai como «não medido», nunca estimada.

Tambem aceito como: measure
```
<!-- gerado:ajuda:medir:fim -->

<!-- gerado:ajuda:agente:inicio -->
Saida de `phxclaw ajuda agente`:

```text
Roda o agente agora, mostrando cada passo

USO:
  phxclaw agente "objetivo" [--modelo M] [--plano [--sim]] [--estilo NOME] [--imagem ARQ]... [--gravar ARQ] [--verificar "CMD"] [--saida-esquema ARQ] [--pasta DIR]

Roda o agente agora, mostrando cada passo. --plano: so leitura, executa depois da aprovacao. Perguntas do agente sao respondidas aqui. --imagem (ate 4, png/jpeg/gif/webp): o modelo ve a imagem junto do objetivo. --gravar: cada pedido/resposta do modelo e cada ferramenta em JSONL, segredo redigido (repita com `repetir`). --verificar: comando que confere o fim no sandbox da tarefa (pede shell.exec); codigo diferente de 0 recusa a resposta. --saida-esquema: esquema JSON da resposta final, conferido pelo mesmo validador dos argumentos das ferramentas.

Tambem aceito como: agent
```
<!-- gerado:ajuda:agente:fim -->

<!-- gerado:ajuda:repetir:inicio -->
Saida de `phxclaw ajuda repetir`:

```text
Repete uma gravacao sem modelo e acusa a divergencia com o passo

USO:
  phxclaw repetir ARQ.jsonl [--ferramentas reais|gravadas] [--pasta DIR]

Repete uma gravacao (agente --gravar, ou gravar:true na API) com as respostas gravadas no lugar do modelo. reais (padrao) roda as ferramentas sob o portao de sempre numa tarefa nova; gravadas devolve as saidas gravadas sem efeito. Sai com 3 e o passo da primeira divergencia se a sequencia mudar.

Tambem aceito como: replay
```
<!-- gerado:ajuda:repetir:fim -->

<!-- gerado:ajuda:avaliar:inicio -->
Saida de `phxclaw ajuda avaliar`:

```text
Compara modelos pelo agente: p50/p95, tokens/s, CPU, energia, acerto e nota

USO:
  phxclaw avaliar --modelos A,B --tarefas DIR [--rodadas N] [--saida DIR] [--pasta DIR]

Roda cada caso de DIR (*.json com gabarito, ou *.jsonl gravado) N vezes por modelo, pelo agente inteiro. Cada numero sai com faixa min-max, N e data; vencedor so quando as faixas nao se cruzam. Alem do acerto, a nota parcial de ferramentas (conjunto e sequencia por LCS, 0 a 1, deterministica, nunca por juiz) e o agrupamento pelo sha256 do prompt e das skills de cada execucao. Energia so de RAPL ou NVIDIA: sem eles, «não medida». Cada execucao fica gravada em SAIDA/gravacoes.

Tambem aceito como: eval
```
<!-- gerado:ajuda:avaliar:fim -->

Um caso com gabarito escrito é um `*.json` na pasta de `--tarefas` (o `id` vazio vira o nome
do arquivo; `contem` confere a resposta final sem diferenciar maiúsculas):

```json
{ "id": "", "objetivo": "Qual a capital da Franca? Responda so o nome.",
  "gabarito": { "contem": ["Paris"] } }
```

Uma gravação boa (`agente --gravar ARQ.jsonl`) posta na mesma pasta vira caso também, e o
gabarito dela é a sequência de ferramentas que registrou. Sem `--saida`, o resultado vai para
`<pasta>/avaliacoes/<carimbo>/resultado.json`, com cada execução em `gravacoes/`.

<!-- gerado:ajuda:skill:inicio -->
Saida de `phxclaw ajuda skill`:

```text
Otimiza uma skill por A/B medido; so promove sem cruzar faixas

USO:
  phxclaw skill otimizar NOME --tarefas DIR [--modelo M] [--rodadas N] [--pasta DIR]

O modelo escreve uma variante do SKILL.md; original e variante rodam os mesmos casos (gravacoes ou casos com gabarito). A variante so substitui a original quando a faixa de acerto dela fica inteira acima; a decisao, com os numeros, vai para <skill>/otimizacao.jsonl.
```
<!-- gerado:ajuda:skill:fim -->

## 8. Conselho de integradores (Go/NoGo)

Go só unânime; um NoGo vigente bloqueia; parecer faltando aguarda. A CLI e a ferramenta
`go_no_go` (capacidade `gonogo.write`, fora do padrão) são o mesmo motor. O código de saída
serve para CI.

<!-- gerado:ajuda:gonogo:inicio -->
Saida de `phxclaw ajuda gonogo`:

```text
Conselho de integradores: abrir, registrar parecer, ver, decidir Go/NoGo

USO:
  phxclaw gonogo abrir INTEGRACAO --integradores a,b | registrar INTEGRACAO INTEGRADOR OK|NOGO [--erro "..."]... [--credencial TOKEN|-] | ver INTEGRACAO | decidir INTEGRACAO [--pasta DIR]

Quem abre declara o conselho, que nao muda. O primeiro parecer de cada integrador devolve UMA vez a credencial dele (guardada no SecretBroker); os seguintes a exigem (--credencial -, pelo stdin). NOGO exige os erros. Algum NOGO vigente: NOGO; falta parecer do conselho: AGUARDAR; todos OK: GO. So o mesmo integrador troca o NOGO dele. Saida 0 GO, 2 NOGO, 3 AGUARDAR.

Tambem aceito como: go-no-go
```
<!-- gerado:ajuda:gonogo:fim -->

<!-- gerado:ferramentas:go_no_go:inicio -->
| Ferramenta | Capacidade | Padrao | Descricao (a que o modelo le) |
|---|---|---|---|
| `go_no_go` | `gonogo.write` | **nao** | Integrators' council Go/NoGo. action: open {integration, integrators} declares the council (fixed afterwards); record {integration, integrator, verdict: OK\|NOGO, errors? (required for NOGO)} -- the integrator must be in the council, and once a name is recorded only the same task can record for it again; status {integration}. Decision: any current NOGO -> NOGO; a council member without verdict -> WAIT; all OK -> GO. |
<!-- gerado:ferramentas:go_no_go:fim -->

## 9. Telas: fidelidade e o gabarito

O gabarito são as telas do `design_erp_ui`: cada uma sai de um UI-IR conhecido, é desenhada
no Chromium headless, lida de volta pelo OCR (e, se pedido, pelo modelo de visão) e comparada
com a origem. O arquivo do dia não se sobrescreve.

<!-- gerado:ajuda:ui:inicio -->
Saida de `phxclaw ajuda ui`:

```text
Prova as telas geradas: ida e volta (fidelidade) e larguras (responsivo)

USO:
  phxclaw ui fidelidade [--telas N] [--modelo N] [--prazo S] [--saida DIR] [--capturas DIR] | responsivo [--alvo html|bootstrap] [--bootstrap-css ARQ] [--telas N] [--saida DIR] [--capturas DIR] | responsivo --phx ARQ.phx.json [--bootstrap-css ARQ] [--exemplo INDEX.html] | importar ARQ.phx.json [--saida DIR] [--bootstrap-css CAMINHO]

Gera as telas do gabarito (20, do design_erp_ui), desenha cada uma no Chromium headless, le o PNG pelo screenshot_to_erp_ui (OCR + layout; com o modelo de visao so nas N primeiras de --modelo, ~90 s cada, --prazo por pergunta) e compara com o UI-IR de origem: achados, perdidos, inventados, rotulo, tipo, ordem (Kendall), grupo e posicao, com faixa min-max e data. Grava SAIDA/fidelidade-DATA.json (padrao docs/ui/fidelidade); --capturas guarda os PNG para refazer o OCR. responsivo: as mesmas 20 telas em 320, 390, md e 1280 px e um pixel antes e no ponto de cada quebra de breakpoints.json; mede rolagem da pagina, cortados, sobrepostos, alvos de toque, Tab (anel e ordem), colunas contra o IR, painel de 360 px em janela de 1920 e redimensionar sem recriar no. --alvo bootstrap pede o bootstrap.min.css LOCAL (--bootstrap-css) e compara com o phoenix. Grava SAIDA/responsivo-ALVO-DATA.json; o arquivo do dia nao se sobrescreve. --phx: o PHX JSON do Phoenix nos dois adaptadores (rolagem, colunas por largura e por painel, fronteiras, identidades, casca) e, com --exemplo, as colunas do index.html do dono nas mesmas larguras; grava phx-exemplo-DATA.json. importar: PHX JSON -> UI-IR + telas phoenix e Bootstrap (folha LOCAL) e o PHX JSON escrito de volta, dizendo se a ida e volta saiu identica.
```
<!-- gerado:ajuda:ui:fim -->

Última medição gravada de cada tipo:

<!-- gerado:medicoes-ui:inicio -->
| Medicao | Arquivo | Medida em | Comando gravado |
|---|---|---|---|
| `fidelidade` | `docs/ui/fidelidade/fidelidade-2026-10-01.json` | 2026-10-01 16:04 | `phxclaw ui fidelidade --telas 20 --modelo 3` |
| `responsivo-html` | `docs/ui/fidelidade/responsivo-html-2026-10-01.json` | 2026-10-01 18:29 | `phxclaw ui responsivo --alvo html --telas 20` |
| `responsivo-bootstrap` | `docs/ui/fidelidade/responsivo-bootstrap-2026-10-01.json` | 2026-10-01 18:30 | `phxclaw ui responsivo --alvo bootstrap --telas 20` |

`fidelidade-2026-10-01.json`, braco `so_ocr` (mediana e faixa min–max sobre as telas, medido em 2026-10-01 16:04):

| Metrica | Mediana | Min | Max | N |
|---|---|---|---|---|
| `distancia_max` | 0.0017 | 0.0011 | 0.0022 | 20 |
| `distancia_media` | 0.0013 | 0.0007 | 0.0018 | 20 |
| `grupo_rand` | 1.0 | 1.0 | 1.0 | 20 |
| `kendall_tau` | 1.0 | 1.0 | 1.0 | 20 |
| `obrigatorio_igual` | 1.0 | 0.5 | 1.0 | 20 |
| `precisao` | 1.0 | 0.75 | 1.0 | 20 |
| `revocacao` | 1.0 | 0.75 | 1.0 | 20 |
| `rotulo_exato` | 1.0 | 1.0 | 1.0 | 20 |
| `segundos_por_tela` | 0.4727 | 0.3788 | 24.7041 | 20 |
| `tipo_igual` | 0.8 | 0.5 | 1.0 | 20 |
| `titulo_do_grupo_igual` | 1.0 | 0.6 | 1.0 | 20 |

Totais: achados 109, campos_na_origem 113, fora_da_captura 0, inventados 4, perdidos 4.

`fidelidade-2026-10-01.json`, braco `com_modelo` (mediana e faixa min–max sobre as telas, medido em 2026-10-01 16:04):

| Metrica | Mediana | Min | Max | N |
|---|---|---|---|---|
| `distancia_max` | 0.0017 | 0.0016 | 0.002 | 3 |
| `distancia_media` | 0.0014 | 0.0011 | 0.0017 | 3 |
| `grupo_rand` | 1.0 | 1.0 | 1.0 | 2 |
| `kendall_tau` | 1.0 | 1.0 | 1.0 | 2 |
| `obrigatorio_igual` | 1.0 | 1.0 | 1.0 | 3 |
| `precisao` | 1.0 | 0.5 | 1.0 | 3 |
| `revocacao` | 0.8 | 0.5 | 1.0 | 3 |
| `rotulo_exato` | 1.0 | 1.0 | 1.0 | 3 |
| `segundos_por_tela` | 112.1496 | 104.9929 | 149.9969 | 3 |
| `tipo_igual` | 1.0 | 1.0 | 1.0 | 3 |
| `titulo_do_grupo_igual` | 1.0 | 1.0 | 1.0 | 3 |

Totais: achados 9, campos_na_origem 11, fora_da_captura 0, inventados 1, perdidos 2.
<!-- gerado:medicoes-ui:fim -->

## 10. Tema claro

A UI web (`phxclaw servir`, em `/`) tem o escuro da marca e o claro em papel quente
([estilo](ui/STYLE_PHOENIX_PADRAO.md)). A ordem de decisão é: a escolha guardada pelo botão do
topo → o tema do sistema (`prefers-color-scheme`) → o escuro. Só o clique guarda a escolha
(no `localStorage` do navegador); sem clique, trocar o tema do sistema troca a tela. Não há
chave de `config.json` para isso: é preferência de quem olha, não do servidor
(`apps/phxclaw-ui/assets/tema.js`).

## O que este guia não cobre

- O gerador só **exercita** `ajuda`, `config` e `ferramentas`: o guia diz o contrato, não
  prova que ele funciona contra o serviço real. OAuth do Google, Linear, ElevenLabs, Gemini e
  xAI não foram exercitados contra a conta real na escrita deste guia.
- Canais (Telegram, Slack, e-mail…), dispositivos e ponte: ajuda em `phxclaw ajuda canal`,
  `ajuda dispositivos`, `ajuda ponte` e chaves `canais.*` em `phxclaw config mostrar`.

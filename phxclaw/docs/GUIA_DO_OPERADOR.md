# Guia do operador do PhxClaw

O que configurar primeiro, onde guardar cada credencial, como confiar num projeto e como
medir. É curto de propósito: o detalhe de cada comando é a ajuda do próprio binário, copiada
aqui pelo gerador.

<!-- gerado:cabecalho:inicio -->
Trechos marcados gerados em 2026-10-09 por `python3 tools/gerar_guia_operador.py`, do binario `PhxClaw 0.70.0` (compilado em 2026-10-09 19:15).
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

**Escopo da chave: algumas só valem do operador.** Projeto confiado é confiança para
*instruir* (modelo, estilo, executáveis do projeto), não para escolher para onde vai uma
credencial nem para afrouxar um teto. Por isso o catálogo marca chaves que só valem do
**ambiente, do perfil ou de `<pasta>/config.json`** — nunca do `.phxclaw/config.json` do
projeto. São três famílias: destino que recebe credencial (`api.url_cliente`, as URLs dos
provedores e da forja, o SMTP, `mcp.config`…), teto e lista de permissão
(`orcamento.teto_*`, `custo.precos`, `agente.capacidades`, `rede.destinos`, `desktop.*`,
`plugins.*`…) e servidor, conta ou canal do operador (`api.*`, `ponte.*`, `canais.*`…). A
lista é uma só, `SO_DO_OPERADOR` em `crates/phxclaw-config-runtime/src/agente/catalogo.rs`, e
a carga a aplica num ponto: nenhum leitor decide sozinho.

- O projeto que declara uma delas **não erra**: a chave é ignorada, vale a camada do
  operador, e todo comando avisa no stderr (`aviso: .phxclaw/config.json: CHAVE: ignorada no
  projeto, so vale do operador: …`). `config mostrar` repete o aviso, e `config mostrar
  --json` traz `so_do_operador` em cada chave e `arquivos.projeto_chaves_ignoradas`.
- `config definir CHAVE VALOR --projeto` numa delas é **recusado** (gravaria o que a carga
  não lê); `--remover` passa, para limpar o arquivo.

O motivo, medido em 09/10/2026: um `.phxclaw/config.json` de repositório com
`{"api":{"url_cliente":"http://atacante"}}` fazia o `phxclaw api` mandar o Bearer — que
alcança o terminal do servidor — em claro para fora.

**`phxclaw api` e o token.** O Bearer só sai para **loopback** (`127.0.0.1`, `::1`,
`localhost`) **ou `https`**. `http` fora de loopback é recusado antes de qualquer conexão,
sem exceção; `--url` fora de loopback pede `--confio-nesta-url`, para dizer que o servidor é
seu. Corpo em linha (`--corpo '{…}'`) e `--consulta` com forma de credencial são recusados:
o argv fica em `ps` e no histórico do shell — mande por `--corpo -`. O mesmo vale para o
`phxclaw ferramenta`: valor com forma de credencial em `--PARAM` é recusado; use
`--json -` (entrada padrão) ou o nome de uma credencial guardada no broker.

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
Fonte: `crates/phxclaw-agent/src/canais/xmpp.rs` (1.013 linhas, `wc -l`, 06/10/2026), o
portão `canais::autorizado` em `canais/mod.rs` e as chaves do catálogo
(`crates/phxclaw-config-runtime/src/agente/catalogo.rs`, `CanalDef` `xmpp`), que o gerador
espalha pela tabela acima, por `schemas/config.exemplo.json` e pelo `config-catalogo.json`
da tela:

| Chave do catálogo | Variável | Tipo | O que faz |
| --- | --- | --- | --- |
| `canais.xmpp.salas` (`SALAS`) | `PHXCLAW_XMPP_SALAS` | lista | JIDs das salas em que o bot entra ao abrir a conexão; cada sala também precisa estar em `PERMITIDOS`. Vazia = só `chat`. |
| `canais.xmpp.apelido` (`APELIDO`) | `PHXCLAW_XMPP_APELIDO` | texto | nick nas salas; vazio = a parte local do JID. |
| `canais.xmpp.permitidos` (`PERMITIDOS`) | `PHXCLAW_XMPP_PERMITIDOS` | lista | a sala **e** cada pessoa que pode comandar o agente nela (pelo JID real). |
| `canais.xmpp.confiar_no_nick` (`CONFIAR_NO_NICK`) | `PHXCLAW_XMPP_CONFIAR_NO_NICK` | booleano, padrão `false` | só para sala anônima: aceitar `sala/nick` de `PERMITIDOS` como identidade. **Risco:** numa sala anônima, o nick é de quem chegar primeiro com ele. |

```sh
PHXCLAW_XMPP_SALAS=ops@conference.exemplo.com \
PHXCLAW_XMPP_APELIDO=phxclaw \
PHXCLAW_XMPP_PERMITIDOS=ops@conference.exemplo.com,adriano@exemplo.com \
phxclaw canal xmpp
```

**Quem comanda o agente numa sala.** A sala em `PERMITIDOS` deixa o agente ouvir e falar
nela; **não** faz de cada ocupante um dono. Cada fala passa pelo portão com a sala **e** o
autor, e o autor é:

- **o JID real do ocupante**, lido do `<item jid='…'/>` da presença da sala (XEP-0045
  §7.2.3, salas não anônimas). O nick não conta: `ana` com o JID de outra pessoa é aquela
  outra pessoa, mesmo com `sala/ana` na lista e `CONFIAR_NO_NICK=true`;
- numa **sala anônima** (a presença não traz o JID), `sala/nick` — **só** com
  `CONFIAR_NO_NICK=true`. Sem isso o ocupante chega sem identidade conferível e nunca passa.

No exemplo acima, só `adriano@exemplo.com` comanda o agente em `ops@conference…`; os outros
ocupantes falam e o agente não age. Quando uma tarefa aberta na sala pergunta algo
(`ask_user`), **só quem a abriu** responde por ela: a fala de outro ocupante permitido vira
pedido novo. A fala recusada **não chega ao disco**: a caixa guarda só o metadado (conversa,
autor, hora, tamanho), e o registro diz o autor, nunca o texto. O JID se compara sem
diferença de caixa na parte local e no domínio (RFC 7622), dos dois lados: `Adriano@Exemplo.com`
na lista vale para `adriano@exemplo.com`.

O que o canal faz com o que chega da sala, e o que ignora de propósito
(`mensagem_da_estrofe`, `eco`, `erro_da_presenca`):

- **`groupchat` vira tarefa** com a sala como conversa e o JID real (ou `sala/nick`, ver
  acima) como autor; a resposta volta `groupchat` à sala inteira (XEP-0045 §7.4).
- **Chat privado de ocupante** (`type='chat'` vindo de `sala/nick`) chega com a conversa
  `sala/nick` inteira, e a resposta volta `chat` só a ele — nunca em público. Só entra se
  `sala/nick` **e** a identidade do ocupante estiverem em `PERMITIDOS`.
- **Ignorado, com o motivo:** o **eco** da própria fala (a sala reflete a todos, inclusive a
  quem falou; o nick é o que a sala nos deu de fato, que com `status 210` pode não ser o
  configurado), o **histórico** que a sala reenvia ao entrar (`<delay/>`, `urn:xmpp:delay`
  ou o antigo `jabber:x:delay` — senão o agente responderia ao passado) e o **aviso da
  própria sala** (mensagem sem nick, com ou sem corpo: assunto, aviso de serviço). Estado de
  digitação sem `<body>` também não é mensagem.
- **Erro ao entrar vira motivo legível**, e a entrada **espera** a confirmação da sala (a
  presença refletida com o nosso nick ou `status 110`): sem esperar, um 409 chegaria depois,
  misturado ao fluxo, e ninguém o veria. Os motivos: `conflict` (409) «o apelido já está em
  uso na sala», `item-not-found` «a sala não existe», `not-authorized` «a sala pede senha»,
  `forbidden` «o agente está banido da sala», `registration-required` «a sala só aceita
  membros», `service-unavailable` «a sala está lotada», `not-acceptable` «a sala não aceita
  esse apelido», `jid-malformed`; condição desconhecida sai como `erro <code>`.
- **Estrofe gigante ou fila parada derruba a conexão com erro legível**: estrofe acima de
  256 KiB sem fechar (`TETO_ESTROFE`) ou 1.024 estrofes esperando sem ninguém ler
  (`CAPACIDADE_FILA`). O laço reconecta na volta seguinte.

Ainda não existe: senha de sala e o agente trocar o próprio nick com a conexão aberta.
Prova, contra um servidor XMPP falso (nenhum servidor MUC real foi exercitado):
`crates/phxclaw-agent/tests/canais.rs` (`xmpp_entra_na_sala_ouve_so_os_outros_e_responde_em_groupchat`,
`xmpp_ocupante_comum_de_sala_permitida_nao_vira_tarefa_nem_chega_ao_disco`,
`xmpp_nick_so_vale_em_sala_anonima_e_so_se_o_operador_confia`,
`xmpp_privada_de_ocupante_responde_em_chat_ao_nick_nunca_na_sala`,
`xmpp_fala_antes_da_confirmacao_chega_e_o_eco_usa_o_nick_que_a_sala_deu`,
`xmpp_estrofe_sem_fim_e_fila_cheia_derrubam_a_conexao_com_erro_legivel`,
`xmpp_ligado_compara_jid_sem_caixa_na_lista`, `xmpp_nick_em_conflito_na_sala_e_erro_legivel`,
`na_sala_so_quem_abriu_a_tarefa_responde_a_pergunta_dela`) e os testes de unidade do
`xmpp.rs` e do `canais/mod.rs`.

### Canais Teams e Google Chat: o JWT RS256 de entrada

Desde a SP000032 R1 (09/10/2026) a entrada dos dois canais confere o JWT RS256 que o serviço
põe no `Authorization: Bearer` — a assinatura pela chave do JWKS do serviço, o emissor, a
audiência e a validade (5 min de folga nos dois lados). **Não há como desligar**: sem token
válido, o webhook responde 401 sem dizer o que falhou. O RSA (só verificação, PKCS#1 v1.5 +
SHA-256) é escrito aqui, sem crate nova: `crates/phxclaw-agent/src/canais/rsa.rs`, e o JWT em
`canais/jwt.rs`.

| Chave do catálogo | Variável | O que faz |
| --- | --- | --- |
| `canais.teams.app_id` | `PHXCLAW_TEAMS_APP_ID` | O App ID do bot; é a audiência (`aud`) exigida no token. |
| `canais.teams.jwks` | `PHXCLAW_TEAMS_JWKS` | URL das chaves; vazio = `https://login.botframework.com/v1/.well-known/keys`. Trocada, sai um aviso ao subir; `http://` só em loopback. |
| `canais.googlechat.audiencia` | `PHXCLAW_GOOGLECHAT_AUDIENCIA` | **Obrigatória.** O número do projeto (token da conta `chat@system.gserviceaccount.com`) **ou** a URL `https://` do endpoint (ID token OIDC de `accounts.google.com`, com `email` = a conta do Chat e `email_verified`). É ela que escolhe o modo, um só por canal. |
| `canais.googlechat.jwks` | `PHXCLAW_GOOGLECHAT_JWKS` | URL das chaves; vazio = a oficial do modo (`service_accounts/v1/jwk/chat@…` ou `oauth2/v3/certs`). Trocada, sai um aviso ao subir; `http://` só em loopback. |
| `canais.teams.chave_url`, `canais.googlechat.chave_url` | `PHXCLAW_*_CHAVE_URL` | **Agora opcional**: se configurada, a `?chave=` da URL continua conferida, **além** do token. |

No Teams, além disso, o claim `serviceUrl` assinado tem de ser **igual** ao `serviceUrl` da
Activity e a chave tem de estar endossada para o `channelId` dela; se não, 403 (o token é bom,
mas não vale para aquele pedido). O caminho do Bot Framework Emulator (outro emissor) fica fora.

**O que mudou para quem já tinha o canal ligado:** o Teams sobe como antes (o `APP_ID` já era
obrigatório) e passa a recusar o webhook sem JWT válido — o Bot Framework sempre o manda. O
Google Chat **não sobe** sem `PHXCLAW_GOOGLECHAT_AUDIENCIA` e diz que é ela que falta: subir
sem audiência seria subir sem conferir.

O JWKS fica em memória e se recarrega a cada 24 h e quando chega um `kid` desconhecido (no
máximo uma vez por minuto — sem esse teto, cada `kid` inventado seria um pedido nosso ao
serviço). JWKS vencido que não recarrega recusa com 503: a chave velha não é usada. Chave fora
da política é pulada: só RSA de 2048 a 4096 bits com `e = 65537`.

O download do JWKS corre fora da trava do cache, um por vez: quem chega enquanto ele baixa usa
o conjunto que já está em memória (um serviço de chaves lento não segura o token de `kid`
conhecido), o `kid` desconhecido leva 401 e, antes do primeiro conjunto chegar, 503.

A conferência da assinatura é a única conta cara que qualquer um dispara sem credencial
(basta um `kid` público e lixo do tamanho da chave). Medido: ~1,4 ms por verificação de 2048
bits e ~4,9 ms de 4096, binário otimizado. Por isso cada canal tem um teto de conferências em
voo — metade dos núcleos, no mínimo 2 —, e acima dele o webhook responde **429** na hora, sem
fazer a conta. Uma vaga só já atende ~700 tokens de 2048 bits por segundo.

A URL do JWKS é a âncora de confiança do canal: não é segredo, mas quem a troca escolhe quais
chaves assinam por nós. Configurada diferente da oficial, o canal sobe com um
`aviso: <canal>: JWKS trocado para …` na saída de erro.

Prova: vetores oficiais em `crates/phxclaw-agent/tests/dados/rsa/` (Wycheproof
`rsa_signature_2048_sha256_test.json`, 259 casos, e NIST CAVP SigVer15 SHA-256, 54 casos; o
`extrair.py` de lá refaz os arquivos a partir dos originais e o cabeçalho de cada um traz a
fonte, o SHA-256 do original e a licença) e, em `tests/canais.rs`,
`rsa_pkcs1_sha256_contra_o_wycheproof`, `rsa_pkcs1_sha256_contra_o_nist_sigver15`,
`chave_do_jwks_so_entra_com_2048_a_4096_bits_e_expoente_65537`,
`teams_confere_o_jwt_rs256_do_bot_framework`,
`jwks_recarrega_no_kid_novo_e_no_vencimento_e_falha_fechado`,
`googlechat_confere_o_jwt_nos_dois_modos_e_sai_pelo_webhook_do_espaco` e
`teams_e_googlechat_montam_pelo_ambiente_com_o_jwt`, `jwks_lento_nao_segura_token_de_kid_conhecido`,
`conferencia_de_assinatura_tem_teto_em_voo_e_429_acima_dele`,
`rsa_custo_de_uma_verificacao_2048_e_4096`, `jwks_trocado_avisa_e_http_fora_de_loopback_recusa`
e `iguais_nao_conta_o_tamanho_do_segredo`. Os tokens dos testes são assinados por
um par RSA gerado na hora pelo `openssl` do sistema (nenhuma chave privada no repositório);
sem `openssl`, esses testes registram o pulo. Nenhum token real da Microsoft ou do Google foi
conferido ainda: a prova é contra servidor falso.

### Terminal do IDE no navegador: só no sandbox

O terminal do IDE (`/v1/ide/terminal`, o Helix no navegador) roda **somente** dentro do `bwrap`,
com a pasta do projeto em `/work` e a pasta do agente (`var/agente`, com o cofre de segredos e o
`api.token`) escondida. Sem `bwrap` na máquina, o terminal **recusa** abrir, com a mensagem
`sem bwrap: o terminal do IDE so roda no sandbox` — é a mesma regra do shell do agente. Fora do
sandbox o Helix abre qualquer caminho, e foi assim que a chave-mestra do cofre aparecia na tela
(medido em 09/10). Os testes do IDE e os servidores de linguagem também rodam com a pasta do
agente escondida. Em máquina sem `bwrap` (Windows), o terminal web fica indisponível.

### Segurança da tela: o que protege e o que só dificulta

**O que protege de verdade** (vale sempre, não se desliga):

- **CSP estrita** em toda resposta do `servir` e da ponte (`pwa.rs`, um ponto só):
  `script-src 'self'` — nenhum script em linha, nenhum `eval` —, `object-src 'none'`,
  `base-uri 'none'`, `frame-ancestors 'none'` (a tela não entra em moldura de outro site),
  `connect-src 'self'` (a página só fala com o próprio agente). Junto: `nosniff`,
  `Referrer-Policy: no-referrer`, câmera/microfone/localização fechados, COOP e CORP
  `same-origin`, e `Cache-Control: no-store` em todo dado (tarefa, configuração, erro). O
  estilo em linha é aceito (`style-src 'unsafe-inline'`): o phx-grid monta largura e recuo
  por `style=""`, e sem isso a grade perde o desenho — medido, 81 recusas só em duas telas.
  CSS injetado não executa nada.
- **RBAC e o portão no servidor**: o papel é conferido no agente, não na tela.
- **Segredo nunca no cliente**: a tela não recebe chave nenhuma; a configuração mostra se o
  segredo existe, nunca o valor.
- **No desktop (Tauri)**, o build de release não tem DevTools (a feature `devtools` não está
  ligada), a página não tem a permissão de abrir o inspetor e o menu de contexto nativo é
  cancelado pela própria página (sem inspetor, não há como desfazer isso de dentro). Ali o
  bloqueio é de verdade.

**O que só dificulta: `ui.bloquear_inspecao`** (ligado por padrão). No navegador ele cancela
o clique direito, o F12, Ctrl/Cmd+Shift+I/J/C e Ctrl/Cmd+U. **Isso dificulta, mas não
impede**: o menu do próprio navegador abre o inspetor, `view-source:` na barra mostra o
código, e qualquer cliente HTTP (curl) baixa a mesma página. Não conte com ele para esconder
nada — tudo o que a tela tem, quem a abre tem. Dentro de campo de texto o menu de contexto
continua (colar e corrigir são edição); Tab, leitor de tela e Ctrl+C não são tocados. Para
desligar no navegador: `"ui": {"bloquear_inspecao": false}` no `config.json` (ou
`PHXCLAW_UI_BLOQUEAR_INSPECAO=0`). No desktop ele vale sempre.

Servindo a tela na rede **sem TLS**, o navegador ignora o COOP e avisa no console: é o recado
certo — fora do `127.0.0.1`, use HTTPS.

### Usuários, projetos e papéis da API (RBAC)

Sem usuários, a API de `phxclaw servir` tem uma porta só: o Bearer do `api.token`, como
sempre foi. Os usuários entram **pedidos**, pela CLI local — não há rota HTTP que crie
usuário, porque criar pela rede seria a porta para o primeiro que chegar se fazer dono:

```bash
phxclaw usuario criar ana --papel member --projeto vendas     # imprime o token UMA vez
phxclaw usuario criar lia --papel leitor --projeto vendas --projeto compras
phxclaw usuario criar ada --papel admin
phxclaw usuario listar                                         # nome, papel, projetos; nunca o hash
phxclaw usuario chave ana                                      # troca o token; o anterior morre
phxclaw usuario remover ana                                    # sem usuários, volta o Bearer único
```

O arquivo é `<pasta>/usuarios.json`, gravado 0600, com o sal e o SHA-256 de cada token — o
token em si não fica em lugar nenhum. Trocar a chave ou remover vale **no pedido seguinte**,
sem reiniciar o `servir`. O `api.token` continua valendo, como **owner**: quem lê aquele
arquivo já roda `phxclaw usuario` na mesma máquina, e recusá-lo só quebraria a ponte, os
gatilhos e o SDK.

| Papel | Alcança |
|---|---|
| `leitor` | ler tarefas e artefatos dos projetos dele; listar, ler e validar fluxos; `GET /metrics` |
| `member` | o do leitor, mais criar, aprovar, editar plano, responder e cancelar tarefas dos projetos dele |
| `admin` | todos os projetos e as tarefas sem projeto; agenda, configuração (só leitura), IDE (menos o terminal), MCP; gravar e rodar fluxos |
| `owner` | tudo: gravar configuração, terminal do IDE e do túnel, instalar plugin |

A tabela é a `MATRIZ` de `crates/phxclaw-agent/src/rbac.rs`, lida por um portão único (o
middleware do router); rota que ninguém classificou é só do owner. O projeto do pedido vem
de dois lugares, nunca de um terceiro: da tarefa gravada (rotas com `{id}`) ou do cabeçalho
`X-PhxClaw-Projeto` (criar e listar). Usuário de um projeto só pode omitir o cabeçalho; com
dois ou mais, a API pede que ele diga qual. `projeto` no corpo do `POST /v1/tasks` é
**recusado** (400): seria um segundo campo dizendo outra coisa, que o portão não lê. A
tarefa criada sem projeto (pela agenda, por gatilho, por canal ou pelo `api.token` sem o
cabeçalho) é da instância: só admin e owner a veem. O subagente herda o projeto da mãe.

Os fluxos da tela (`/v1/fluxos...`) são arquivos da pasta do projeto da **instância**, não de
um projeto do RBAC: o portão confere só o papel. Listar, ler e validar são do `leitor`;
**gravar e rodar são do `admin`**, porque o arquivo gravado pela tela é o mesmo que o webhook,
a agenda e o gatilho de poll rodam com as credenciais do operador — um `member` de um projeto
que o reescrevesse rodaria código dele com o poder da instância inteira. A tarefa que o
`POST /v1/fluxos/rodar` cria nasce no projeto do cabeçalho `X-PhxClaw-Projeto` (sem o
cabeçalho, é da instância). A tela só alcança o fluxo pelo nome que a lista mostra
(`ARQ.json` ou `PASTA/ARQ.json`): nada que comece com ponto, e por isso nunca a pasta
`.ARQ.versoes/` — a versão publicada só muda pelo `phxclaw fluxo publicar`/`voltar`.

`phxclaw usuario criar`, `chave` e `remover` gravam sob uma trava (`.usuarios.json.lock`, ao
lado do arquivo): dois terminais mexendo ao mesmo tempo não desfazem a remoção um do outro.

Limites declarados — o RBAC **não** alcança estes caminhos:

- **A ponte entra como owner.** O agente executa o pedido que chega pela ponte no próprio
  router com o `api.token` injetado (`crates/phxclaw-agent/src/remoto.rs`,
  `executar_pedido`): quem fala com a ponte age como dono, e o papel e o projeto de usuário
  não se aplicam por ela. O que a limita é a lista fechada de rotas do `remoto::permitido`
  (tarefas, túnel, sincronizar configuração) e o `ponte.token` — trate esse token como a
  credencial de dono que ele é.
- **O `/metrics` é global.** Qualquer usuário com papel `leitor` ou acima vê os contadores
  da instância inteira (tarefas, fluxos, tokens de todos os projetos), não só os do projeto
  dele. Os rótulos não carregam nome nem conteúdo, mas o volume de cada projeto somado está
  ali; se isso importa, deixe `api.metricas` desligada (a rota responde 404).
- O canvas (`/canvas/...`) e o site publicado (`/sites/...`) continuam públicos, como
  eram — o widget roda em sandbox e se endereça pelo id da tarefa. Os webhooks de gatilho
  (`/v1/triggers/...`) ficam fora do portão: têm o segredo deles.

### Métricas Prometheus (`/metrics`)

`api.metricas` (`PHXCLAW_API_METRICAS`), desligada por padrão. Desligada, o `GET /metrics`
responde 404 e nada é medido — o interruptor é lido antes de envolver modelo e ferramentas,
então não há custo nenhum por chamada. Ligada, a rota responde no formato de texto do
Prometheus, com o mesmo Bearer das outras rotas (com usuários, papel `leitor` ou acima):

| Métrica | Tipo | Rótulos |
|---|---|---|
| `phxclaw_tarefas_total` | counter | `estado` (o final da tarefa) |
| `phxclaw_fluxo_execucoes_total` | counter | `estado` (onde o disparo ou a retomada parou) |
| `phxclaw_passos_total` | counter | — |
| `phxclaw_ferramenta_chamadas_total` | counter | `resultado` (`ok`/`erro`) |
| `phxclaw_modelo_chamadas_total` | counter | `resultado` |
| `phxclaw_tokens_total` | counter | `tipo` (`entrada`/`saida`) |
| `phxclaw_ferramenta_duracao_segundos` | histogram | `le` |
| `phxclaw_tarefa_duracao_segundos` | histogram | `le` |
| `phxclaw_tarefa_custo_total` | counter | `estado` — dinheiro, na moeda da tabela `custo.precos` |
| `phxclaw_tarefa_custo_nao_medido_total` | counter | `estado` — tarefas sem custo medido |
| `phxclaw_fluxo_custo_total` | counter | `estado` — dinheiro de todos os passos do fluxo |
| `phxclaw_fluxo_custo_nao_medido_total` | counter | `estado` |
| `phxclaw_ide_completar_total` | counter | `resultado` (`ok`/`recusada` pelo `ide.ia_teto_tokens_hora`) — completação do editor, fora de qualquer orçamento |
| `phxclaw_ide_completar_tokens_total` | counter | — |
| `phxclaw_ide_completar_custo_total` | counter | — dinheiro, na moeda da tabela `custo.precos` |
| `phxclaw_ide_completar_custo_nao_medido_total` | counter | — |

O `estado` inclui `budget_exceeded` (parou no orçamento, seção 12). A moeda não é rótulo
(é texto do operador): quem lê o custo lê a moeda na tabela. Tarefa sem custo medido não
soma zero — entra no contador `_nao_medido_`.

Todo rótulo é de conjunto fechado: nada de nome de ferramenta, argumento, objetivo ou nome
de credencial (há teste que reprova rótulo fora da lista). Os contadores são do processo:
reiniciar zera, e o Prometheus trata o recomeço. Exemplo de coleta:

```yaml
scrape_configs:
  - job_name: phxclaw
    authorization: { credentials_file: /etc/prometheus/phxclaw.token }
    static_configs: [{ targets: ["127.0.0.1:8787"] }]
```

### Motor de fluxo: quantos rodam ao mesmo tempo

`fluxos.max_simultaneos` (`PHXCLAW_FLUXOS_MAX_SIMULTANEOS`) é o máximo de fluxos rodando ao
mesmo tempo na instância; vazio ou 0 = sem limite, e o excedente espera a vaga. O valor é
**lido no primeiro fluxo** que roda: mudá-lo depois não tem efeito até o agente reiniciar.

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

### O teto de quem vem de fora (MCP, pacote, plugin, skill)

A capacidade de uma extensão é dada **pela casa**, nunca pelo que a extensão diz de si. O
servidor MCP que se anuncia «somente leitura» (`readOnlyHint`, descrição) continua com
`mcp.<servidor>`, fora do Plan Mode e fora do padrão. As portas, provadas em
`crates/phxclaw-agent/tests/teto_extensao.rs`:

| Porta | Capacidade que o portão confere |
|---|---|
| servidor MCP do operador (`PHXCLAW_MCP_CONFIG`) | `mcp.<servidor>` |
| servidor MCP de pacote assinado (`.mcp.json` do pacote) | `mcp.<pacote>.<servidor>` |
| plugin assinado com `agent.tool` | a primária do manifesto, **recusada** se for de leitura ou do espaço `mcp.*` |
| skill importada | nenhuma: `allowed-tools` do cabeçalho é texto |
| subagente de pacote | a interseção do que ele declara com o que o pai tem |

**Mudou em 09/10/2026:** o servidor MCP de pacote tinha a mesma capacidade de um do operador
com o mesmo nome, e a concessão de um valia para o outro. Quem concedia `mcp.<servidor>` para
um servidor de pacote passa a conceder `mcp.<pacote>.<servidor>` (o nome do pacote vem do
`plugin.json`, com o que não for letra, número, `-` ou `_` trocado por `_`).

### Gatilho por notificação MCP (`mcp` no `gatilhos.json`)

Um servidor MCP **do operador** avisa que um recurso mudou e o PhxClaw roda a versão
**publicada** de um fluxo, com o aviso como `{{entrada}}`:

```json
{ "mcp": [ { "nome": "docs-mudaram", "servidor": "arquivos",
             "recursos": ["file:///srv/docs/contrato.md"], "lista": false,
             "fluxo": "fluxos/revisar.json", "janela_ms": 2000, "max_por_minuto": 6 } ] }
```

- `servidor` é um nome do arquivo de `PHXCLAW_MCP_CONFIG`, e o agente do servidor precisa ter
  `mcp.<servidor>` concedida; sem ela, nada sobe e a recusa fica em
  `<pasta>/gatilhos/mcp-<nome>.evidence.jsonl`.
- `recursos` são assinados com `resources/subscribe`; aviso de outra URI é ignorado. `lista`
  liga `notifications/resources/list_changed`. O servidor tem de anunciar `resources.subscribe`
  (e `listChanged`), senão a subida recusa dizendo isso.
- Cada item: `{gatilho, servidor, evento, uri, parametros, vezes}`. A mesma URI dentro de
  `janela_ms` (100 ms a 1 h) vira um disparo só, com a última versão e quantas vieram.
- `max_por_minuto` (1 a 60) é o teto de disparos. O que passa dele espera vaga, e o que ainda
  estiver pendente quando a sessão cair é descartado e contado no log.
- Mais de 600 mensagens do servidor num minuto derruba a assinatura e mata o processo. O
  laço volta depois de um recuo de 5 s, que dobra a cada queda até 15 min.
- O aviso é dado de fora: com forma de credencial — no valor **ou no nome de um campo** —, o
  disparo é recusado pela guarda de entrada do motor de fluxo e não vira tarefa.
- `parametros` acima de `PARAMETROS_MAX_BYTES` (`gatilho_mcp.rs`) não vão ao item: ele chega
  com `parametros: null` e `corte: {parametros_bytes, teto}`, e o fluxo relê o recurso pela
  `uri`.
- `fluxo` fica **dentro do projeto**, pelo mesmo juiz da tela de fluxos: `f.json` ou
  `pasta/f.json`, nada começando com `.`, sem caminho absoluto nem `..`.
- Só arma em projeto confiado (seção 3).
- **Só stdio nesta versão.** Servidor declarado por `url` é recusado ao subir, porque o
  runtime ainda não abre o GET em SSE por onde chega o aviso fora de um pedido. Aviso perdido
  com o fio caído não volta: quem precisa de «nada se perde» usa o poll.

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
  --projeto` passa a gravar nele — menos as chaves só do operador (seção 1);
- `phxclaw servir` arma o `.phxclaw/gatilhos.json` (webhook, arquivo, poll e `mcp`) e o
  `.phxclaw/HEARTBEAT.md`. **Sem confiança, nada disso se arma**: o gatilho dispara trabalho
  com as credenciais do operador, e o `mcp` liga um servidor do arquivo dele. O `servir` avisa
  no stderr (`… gatilhos.json ignorado: projeto nao confiado (phxclaw projeto confiar DIR)`).
  A confiança é a mesma lista do `config.json` do projeto.
- a montagem do agente lê da `.phxclaw/` o que **executa, concede ou fala com o modelo**:
  `hooks.json` (comando a cada evento do laço), `commands/*.md` (o corpo vira o objetivo),
  `estilos/*.md` (instrução no prompt de sistema; o `conciso` do projeto sobrepõe o da casa)
  e `workspace.json` (raiz extra = disco do hospedeiro aberto às ferramentas de arquivo, ao
  LSP e ao terminal do IDE). **Sem confiança, os quatro são ignorados**, com um aviso por
  arquivo no stderr, no mesmo formato do de cima. `phxclaw estilos` lista só o que valeria.

**A exceção, de propósito: `.phxclaw/regras.json` vale sem confiança.** Regra de comando só
aperta — sem o arquivo o portão é `permitir`, e vence sempre a decisão mais estrita —, então
um clone não tem como afrouxar nada por ela; exigir confiança tiraria o `negar` de quem já o
escreveu num projeto que nunca confiou. Vale igual no portão do motor e no túnel remoto.
O `.phxclaw/tarefas.json` (ferramenta `project_task`) também não pede confiança: roda no
mesmo bwrap sem rede do `shell`, pelas mesmas regras, e não dá ao modelo nada que o `shell`
já não dê.

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
  phxclaw skills importar DIR [--com-scripts] [--aceitar-licenca-desconhecida] [--pasta DIR]

Le todo SKILL.md abaixo de DIR (Claude Code, Codex, OpenClaw, Hermes), traduz os nomes de ferramenta conhecidos e grava na pasta de skills. scripts/ nao se copia sem --com-scripts; ORIGEM.json guarda a origem e o SHA-256. Cada skill passa pela porta de licenca (veja `licenca`): copyleft e recusada, desconhecida so com --aceitar-licenca-desconhecida (decisao no ORIGEM.json), compativel entra com o aviso em LICENCA.txt.
```
<!-- gerado:ajuda:skills:fim -->

### Licença de quem vem de fora (`licenca conferir`)

Todo texto de terceiro que se **copia** para dentro do PhxClaw (Apache-2.0) passa por uma porta
só, `crates/phxclaw-agent/src/licenca.rs`: o `skills importar`, as skills de pacote (pelo mesmo
importador, subindo até a raiz do pacote) e o importador de papéis do agency-agents
(`importar_papeis.rs`). Para quem copia por outro caminho, a mesma porta responde na linha de
comando.

- **De onde sai a licença.** A porta sobe da pasta importada até a raiz do repositório de
  origem (a pasta com `.git`; sem ela, a própria origem; `--limite` muda o teto) e lê os
  arquivos `LICENSE`/`LICENCE`/`COPYING`/`UNLICENSE` de cada pasta do caminho, o `NOTICE`, a
  linha `SPDX-License-Identifier`, o `license:` do cabeçalho da skill e o `license` do
  `plugin.json`. Identifica pelo SPDX **e** pelas frases canônicas do texto: no mesmo
  arquivo os dois são declarações separadas, e um `LICENSE` com o texto da AGPL e uma linha
  `SPDX-License-Identifier: MIT` é AGPL. **Nunca pelo nome do arquivo**: `LICENSE-MIT` com o
  texto da GPL é GPL.
- **O que se copia, arquivo por arquivo.** A porta considera a pasta de cada arquivo que vai
  ser copiado e cada ancestral dela até a raiz da origem — inclusive subpastas abaixo da
  pasta importada: o `scripts/vendor/LICENSE` de uma skill importada com `--com-scripts`, o
  `LICENSE` da subpasta de um papel do agency-agents. `licenca conferir DIR` desce a pasta
  inteira (sem `.git`; acima de 10.000 subpastas, a conferência diz que ficou incompleta).
- **Atalho de licença** (`COPYING -> licenses/GPL-3.0.txt`) se segue quando o alvo está
  dentro da raiz da origem, e o alvo é lido como declaração. Atalho para fora da raiz, ou
  quebrado, não se lê: licença desconhecida, com o motivo.
- **Vence a mais restritiva** do caminho (copyleft > desconhecida > compatível), não a mais
  próxima: um `license: MIT` de modelo numa skill dentro de um repositório AGPL não abre a
  porta. O preço, aceito: skill de fato MIT dentro de repositório copyleft fica de fora, e a
  recusa nomeia as duas declarações.
- **Compatível** (MIT, MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Unlicense,
  CC0-1.0, Zlib) entra **com o aviso**: o `LICENCA.txt` ao lado do que se importou leva o
  texto inteiro de cada licença e `NOTICE` achados, byte a byte, e as linhas de copyright; o
  `ORIGEM.json` da skill guarda o mesmo em `licenca`. Não existe opção para tirar o aviso.
- **Copyleft** (GPL, AGPL, LGPL, MPL, EPL, CC-BY-SA, EUPL, CDDL, OSL, CPL e qualquer
  `-or-later`) é **recusada sempre**, dizendo a licença e o arquivo: copiar o texto faria a
  obra combinada herdar o copyleft. Nenhuma opção aceita copyleft. Expressão SPDX se avalia:
  `MIT OR GPL-3.0` entra (o licenciado escolhe), `MIT AND GPL-3.0` não.
- **Desconhecida** (nenhuma licença achada, ou texto que não se reconhece) é recusada por
  padrão. `--aceitar-licenca-desconhecida` importa assim mesmo e grava a decisão — opção,
  operador (`$USER`), quando e a recusa atravessada — no `ORIGEM.json` e no `LICENCA.txt`.
  Pacote carregado no arranque não tem essa opção: não há operador presente para decidir.

No importador de papéis, cada papel passa pela porta com o `SPDX-License-Identifier` e o
`license:` do próprio `.md` e com o `LICENSE` de cada pasta até a raiz do clone: copyleft
recusa **só aquele papel**, dizendo a licença; o texto compatível de uma subpasta vai para
`config/agents/agency/` (`LICENSE.2`…) e o manifesto do papel o cita.

`phxclaw licenca conferir DIR --json` imprime a conferência (`classe`, `licencas`, `entra`,
`motivo`, `declaracoes`, `textos`, `copyright`, `decisao_do_operador`) e sai **0 quando entra
e 2 quando recusa**; é a porta para importadores escritos fora do Rust. Quem chama com
`--aceitar-licenca-desconhecida` recebe a `decisao_do_operador` pronta e a grava junto do que
importar.

### Papéis de terceiro: shell e rede só por concessão

Os papéis importados do agency-agents (`config/agents`, macroárea «Terceiros · …») trazem um
`tools:` no cabeçalho. Ele é **pedido, não concessão**: o manifesto leva só `fs.read` (e
`fs.write`, quando pedido); `shell.exec` e `web.*` ficam em `config/agents/agency/IMPORTACAO.json`
(`pedidos_de_concessao`, com o UUID de cada papel) e no texto `resources` do manifesto. O
corpo de um papel de terceiro é dado de fora, e shell com rede é o par que exfiltra; a
varredura anti-injeção é lista de bloqueio e não segura sozinha o que não conhece.

Para conceder, o operador escreve na pasta do agente o `concessoes-de-papeis.txt`, uma linha
por papel, pelo UUID ou pelo nome exato:

```text
# papel = capacidades (so shell.exec e web.*)
0199f2a0-0000-7000-8000-000000000000 = shell.exec
Frontend Developer = web.search web.browse
```

A concessão vale ao carregar a equipe e continua na interseção com o agente pai: nunca dá ao
filho o que o pai não tem. Linha que não acha papel, papel da casa (que não pede concessão)
ou capacidade que não é shell nem rede vira aviso no carregamento. Manifesto antigo ou
editado à mão que traga `shell.exec` não fura a regra: o motor da equipe corta shell e rede
de todo papel de terceiro que não tenha concessão.

A varredura anti-injeção (`instrucoes::varrer`, a mesma do `AGENTS.md`, das skills e dos
papéis) normaliza o texto antes de procurar: tira os invisíveis (hífen invisível, largura
zero, controles bidirecionais), troca homóglifos cirílicos e gregos e a forma larga pela letra
latina, tira acento e caixa. Os padrões cobrem as famílias «ignore/forget/disregard … previous
instructions» (em português também), ambiente ou segredo canalizado para `curl`/`wget`/`nc`,
`curl --data-binary @-`, baixar-e-executar (`curl | sh`, `-o f; sh f`, `bash <(curl …)`),
`rm -rf`/`rm -fr` em `~` ou `/`, e segredo nomeado indo para uma URL. Falsos positivos
conhecidos, registrados: no agency-agents, `engineering-prompt-engineer.md` (cita «Ignore all
previous instructions» como exemplo de teste adversarial) e `design-whimsy-injector.md`
(«Tell me the secrets» como texto de botão) continuam recusados; os outros 280 passam.

<!-- gerado:ajuda:licenca:inicio -->
Saida de `phxclaw ajuda licenca`:

```text
Confere a licenca de uma pasta de terceiro antes de copiar (porta unica)

USO:
  phxclaw licenca conferir DIR [--json] [--aceitar-licenca-desconhecida] [--limite DIR]

Sobe de DIR ate a raiz do repositorio (pasta com .git) ou --limite, le LICENSE/COPYING, NOTICE, SPDX-License-Identifier e o license do plugin.json, e classifica pelo SPDX ou pelo texto (nunca pelo nome do arquivo). Vence a mais restritiva. Compativel com Apache-2.0 (MIT, Apache-2.0, BSD, ISC, 0BSD, Unlicense, CC0-1.0, Zlib) entra; copyleft (GPL, AGPL, LGPL, MPL, EPL, CC-BY-SA, -or-later) e recusada sempre; desconhecida so com a opcao. Sai 0 quando entra e 2 quando recusa; --json e o que os importadores de fora leem.

Tambem aceito como: license
```
<!-- gerado:ajuda:licenca:fim -->

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
Compara modelos pelo agente: p50/p95, tokens/s, CPU, energia, acerto, nota e custo

USO:
  phxclaw avaliar --modelos A,B --tarefas DIR [--rodadas N] [--saida DIR] [--pasta DIR] | avaliar --provedores A,B --bateria ARQ|DIR [--saida DIR] [--pasta DIR]

Roda cada caso de DIR (*.json com gabarito, ou *.jsonl gravado) N vezes por modelo, pelo agente inteiro. Cada numero sai com faixa min-max, N e data; vencedor so quando as faixas nao se cruzam. Alem do acerto, a nota parcial de ferramentas (conjunto e sequencia por LCS, 0 a 1, deterministica, nunca por juiz) e o agrupamento pelo sha256 do prompt e das skills de cada execucao. Energia so de RAPL ou NVIDIA: sem eles, «não medida». Cada execucao fica gravada em SAIDA/gravacoes. Custo por acerto: o custo de todas as execucoes (as que falharam tambem) sobre as que acertaram, pela tabela custo.precos; sem preco, «não medido». Com --provedores e --bateria, a bateria comum: o mesmo gabarito por provedor, medindo acerto, custo por acerto, duracao, tentativas e intervencao, com intervalo de 95% por bootstrap; vencedor so com os intervalos separados. O phxclaw-model-arena registra os pares (informativo, nao decide). Provedor que nao respondeu sai NÃO MEDIDO.

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

## 11. Roteamento e troca de provedor (`modelo.roteamento`)

Vale **só para quem pede**: o modelo `rota` (a política escolhe tudo) ou `rota:<spec>` (o
`<spec>` vem primeiro e a política dá a reserva), por exemplo
`phxclaw config definir modelo.padrao rota:ollama:qwen2.5:3b`. Qualquer outro modelo continua
indo direto ao provedor, e a medição (`phxclaw avaliar`) continua medindo um modelo só. Sem
`modelo.roteamento` definido, `rota` é erro que diz a chave que falta.

`phxclaw config definir modelo.roteamento /caminho/rota.json`, com:

```json
{
  "provedores": [
    {"spec": "ollama:qwen2.5:3b", "custo": 0},
    {"spec": "openai:gpt-5", "custo": 2},
    {"spec": "anthropic:claude-sonnet-4-5", "custo": 3}
  ],
  "por_tipo": {
    "codigo": ["anthropic:claude-sonnet-4-5", "openai:gpt-5"],
    "geral": ["ollama:qwen2.5:3b", "openai:gpt-5"]
  },
  "classificar": {
    "regras": [{"palavras": ["cargo", "rust", "compilar"], "valor": "codigo", "confianca": 0.9}],
    "modelo": "ollama:qwen2.5:1.5b",
    "limiar": 0.7,
    "padrao": "geral"
  },
  "preferir_barato": false,
  "trocar_em": ["timeout", "429", "5xx", "transporte"],
  "trocas_max": 2,
  "prazo_s": 120
}
```

| campo | o que faz | padrão |
|---|---|---|
| `provedores` | a lista única de provedores, na ordem; todo spec citado em outro campo tem de estar aqui | obrigatório |
| `custo` | custo **relativo** declarado, só para ordenar com `preferir_barato` (dinheiro é `custo.precos`) | 0 |
| `por_tipo` | a cadeia de cada tipo de tarefa; sem tipo, vale a ordem de `provedores` | vazio |
| `classificar` | o decisor do tipo: `regras` (palavras no objetivo), depois o Laya se `"laya": true` (seção 11.1), e, se o incerto ficar abaixo do `limiar`, o `modelo` em modo restrito; resposta fora das chaves de `por_tipo` é «sem decisão» e cai no `padrao` | sem classificação |
| `trocar_em` | as falhas que trocam de provedor: `timeout`, `429`, `5xx`, `transporte`, `resposta_invalida` | as quatro primeiras |
| `trocas_max` | trocas por chamada (teto 8) | 2 |
| `prazo_s` | prazo por tentativa, além do do provedor | o do provedor |

**O que nunca troca**, qualquer que seja a política: erro 4xx do pedido (fora 408 e 429),
política negada e credencial faltando — trocar esconderia o defeito do argumento ou
contornaria a política por outro provedor. Campo desconhecido no arquivo é erro, e provedor
da política que não monta (sem chave) derruba a montagem dizendo qual: reserva que some
calada no dia em que é precisa não é reserva.

**Onde ver quem atendeu:** cada chamada vira um passo `modelo` da tarefa (`ok`, `trocou` ou
`falhou`, com «atendeu X; trocou: A -> 429») e uma linha `modelo.roteamento` no
`evidence.jsonl`, com a cadeia, as tentativas e o motivo da parada. O tipo da tarefa é
decidido uma vez por objetivo e lembrado nas voltas seguintes do laço.

### 11.1 Decisor Laya (`decisao.laya.*`)

O [Laya](https://github.com/NandhaKishorM/laya) é um modelo de decisão «System 1»: recebe um
texto e uma pergunta de forma fechada (escolha entre opções, nota numa escala) e devolve a
resposta com a probabilidade de cada opção, num único forward pass, sem gerar texto. O
PhxClaw fala com o servidor dele, o `laya-serve`, pelo protocolo `POST /v1/systemone` (o do
Jev). **Licença e atribuição:** o Laya é da Convai Innovations, sob Apache-2.0; o PhxClaw não
embute código nem pesos dele — só o cliente HTTP (`crates/phxclaw-agent/src/decisao/laya.rs`),
escrito aqui contra o formato documentado no README e no `laya/serve.py` do commit `1adc59f`
(versão 0.4.1). Quem instala o `laya-serve` e baixa o checkpoint aceita a licença do Laya e a
dos pesos no Hugging Face (`convaiinnovations/laya`).

**Desligado por padrão.** Sem `decisao.laya.url`, nada muda. Para ligar no classificador da
rota, `"laya": true` em `classificar` (o degrau entra entre as `regras` e o `modelo`):

```sh
# o servidor (fora do PhxClaw; CPU)
pip install "laya[serve]==0.4.1"
LAYA_HOST=127.0.0.1 LAYA_PORT=8000 LAYA_DEVICE=cpu LAYA_MODELS=multilingual laya-serve
# o PhxClaw
phxclaw config definir decisao.laya.url http://127.0.0.1:8000
```

e, no `http.json` da pasta do agente, o destino liberado — loopback e rede privada só saem
com `liberar`, pela **mesma** política do nó HTTP (seção do `http_request`), sem exceção
própria para o Laya:

```json
{"liberar": ["http://127.0.0.1:8000"]}
```

<!-- gerado:chaves:decisao:inicio -->
De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): 5 chave(s) em `decisao`.

| Chave | Variavel | Tipo | Padrao | Natureza | O que e |
|---|---|---|---|---|---|
| `decisao.laya.credencial_nome` | `PHXCLAW_DECISAO_LAYA_CREDENCIAL_NOME` | texto | — | config | Nome da credencial bearer declarada no http.json (o segredo fica no broker: phxclaw credencial guardar NOME); vazio = sem Authorization |
| `decisao.laya.limiar` | `PHXCLAW_DECISAO_LAYA_LIMIAR` | real | 0.8 | config | Confiança mínima (answer_confidence, 0 a 1) para a decisão do Laya valer; abaixo, a escada sobe ao próximo degrau |
| `decisao.laya.modelo` | `PHXCLAW_DECISAO_LAYA_MODELO` | enum: english \| multilingual \| typed-decisions | multilingual | config | Checkpoint do Laya pedido ao servidor (multilingual para português) |
| `decisao.laya.prazo_ms` | `PHXCLAW_DECISAO_LAYA_PRAZO_MS` | inteiro | 5000 | config | Prazo de cada pergunta ao laya-serve, em ms (1 a 60000); estourou, sem decisão e a escada sobe |
| `decisao.laya.url` | `PHXCLAW_DECISAO_LAYA_URL` | texto | — | config | URL base do laya-serve (decisor System 1, POST /v1/systemone); vazio = Laya desligado. Loopback e rede privada só com liberar no http.json |
<!-- gerado:chaves:decisao:fim -->

A credencial é o **nome** de uma credencial `bearer` declarada no `http.json` com a origem do
servidor; o segredo vai para o broker por `phxclaw credencial guardar NOME` e sai como
`Authorization: Bearer` (é o `LAYA_API_KEY` do servidor). A chave se chama `credencial_nome`, e
não `credencial`, porque o `config.json` recusa valor em chave com nome de segredo.

O que vale saber:

- **A confiança é o `answer_confidence`** (a probabilidade da resposta dada), nunca o
  `confidence`, que no Laya é 1 − entropia normalizada: o README avisa que limiar herdado do
  Jev não transfere. Servidor em `LAYA_JEV_STRICT` não manda `answer_confidence` e por isso
  **não decide** aqui. O checkpoint `multilingual` sai **sem temperatura ajustada** (README do
  Laya, Calibration): o `0.8` é ponto de partida, e o limiar certo se mede no seu dado, no
  número de opções que a sua política usa.
- **Toda falha é «sem decisão», com o motivo, e a escada sobe:** rede, prazo, 401, 413 (mais
  de 100 opções — conferido antes de sair), 422 (pergunta que o Laya recusa), 5xx, resposta
  sem `answer_confidence` e opção fora da lista. Nada vira palpite.
- **Formas:** escolha vai como `choice` com as opções em `criteria`; predicado, como `choice`
  de duas opções (`sim`/`não`); nota, como `score` de cinco níveis descritos (0, 0,25, 0,5,
  0,75, 1), e o `score` esperado volta dividido por 4.
- **A decisão nunca concede permissão:** a saída passa pela mesma conferência de forma dos
  outros decisores e chega ao portão só por `sobre_o_portao`, que endurece e nunca afrouxa.

**Medido em 09/10/2026** (`laya` 0.4.1 do PyPI, checkpoint `multilingual` revisão `7b928d8`,
torch 2.14.1+cpu, 4 vCPU compartilhadas com compilações de outras frentes): 20 objetivos em
português nos tipos `codigo`/`pesquisa`/`geral`, gabarito escrito antes
(`crates/phxclaw-agent/tests/dados/laya_gabarito_pt.json`), pelo próprio `DecisorLaya`. Para
refazer, com o `laya-serve` de pé:
`LAYA_PROVA_URL=http://127.0.0.1:8000 LAYA_PROVA_GABARITO=crates/phxclaw-agent/tests/dados/laya_gabarito_pt.json cargo test -p phxclaw-agent --test decisao_laya -- --ignored --nocapture`
(uma linha JSON por objetivo: esperado, obtido, `answer_confidence`, ms).

| Medida | Valor |
| --- | --- |
| acerto | 16/20 = 0,80 (Wilson 95%: 0,584–0,919); `codigo` 7/7, `pesquisa` 4/7, `geral` 5/6 |
| com limiar 0,8 | decide 13/20, acerta 11/13 (Wilson 95%: 0,578–0,957) — dois erros com `answer_confidence` 0,92 e 0,995 |
| latência (cliente → resposta) | p50 246 ms, p95 289 ms (carga 5,1); 2ª corrida p50 524 ms, p95 827 ms (carga 6,7); respostas idênticas nas duas |
| checkpoint em disco | 643.835.514 B (`model.safetensors`) + 34.363.188 B (tokenizer) ≈ 678 MB |
| memória do `laya-serve` | 1,79 GB residente estável, 2,41 GB de pico (`VmHWM`) |
| ambiente Python (torch CPU + transformers + laya[serve]) | 1,2 GB |

Medida de uma corrida pequena: diz que o caminho funciona e onde o modelo erra
(`pesquisa` vira `codigo`), não que 0,80 é a taxa do seu tráfego. **O padrão continua
desligado:** os pesos cabem na letra da regra do dono para rede neural («só CPU, modelos
pequenos, até centenas de MB»), mas a memória do servidor passa de 1 GB, e ligar o Laya por
padrão é decisão dele, não deste guia.

## 12. Custo em dinheiro e orçamento (`custo.precos`, `orcamento.*`)

**O preço é seu, nunca do código.** `phxclaw config definir custo.precos /caminho/precos.json`
(`PHXCLAW_CUSTO_PRECOS`), com:

```json
{
  "moeda": "USD",
  "modelos": {
    "anthropic:claude-sonnet-4-5": {"entrada": 3.0, "saida": 15.0, "cache": 0.3,
                                    "data": "2026-10-01", "fonte": "https://.../pricing"},
    "ollama:qwen2.5:3b": {"entrada": 0, "saida": 0, "data": "2026-10-09", "fonte": "máquina própria"}
  }
}
```

Preço por **milhão** de tokens; o nome do modelo é o mesmo `provedor:modelo` do `--modelo`.
Uma moeda por tabela (código ISO de três letras); data `AAAA-MM-DD` e fonte são
obrigatórias — o relatório diz de que dia e de onde veio cada preço, porque preço muda e
número digitado envelhece calado. Campo desconhecido, preço negativo ou fonte vazia é erro
da montagem, dito com o modelo e o campo.

- **Modelo sem preço: custo «não medido», nunca zero.** Zero é o preço que você declarou
  (o Ollama local pode valer 0); ausente é o que ninguém sabe. Uma chamada sem preço deixa o
  total da tarefa não medido.
- **O cache não se aplica:** nenhum provedor deste agente informa ao motor quantos tokens
  vieram do cache; a entrada é cobrada inteira pelo preço cheio. O custo sai igual ou acima
  do real, nunca abaixo.
- **Com a rota (seção 11)**, o preço é o do provedor que **atendeu** a chamada (o diário do
  roteamento), não o de `rota`.
- **Onde aparece:** `custo` no `task.json` (cada chamada, com a cotação, e o total), a linha
  `custo:` no fim do `phxclaw agente`, uma linha `custo.chamada` por chamada no
  `evidence.jsonl` (só com tabela), o `gasto` da tarefa do fluxo (todos os passos) e o
  `/metrics`.

**Orçamento** por tarefa e por fluxo, em tokens (entrada + saída) e em dinheiro:

| Chave | O que é |
|---|---|
| `orcamento.tarefa_tokens` / `orcamento.tarefa_custo` | padrão de cada tarefa |
| `orcamento.fluxo_tokens` / `orcamento.fluxo_custo` | padrão de cada execução de fluxo, somando os passos |
| `orcamento.teto_tokens` / `orcamento.teto_custo` | teto **por pedido**: nenhum pedido passa dele (não soma pedidos diferentes: não é teto de gasto do processo) |

Por pedido: `"orcamento": {"tokens": 50000, "custo": 0.5}` no `POST /v1/tasks` e no
`POST /v1/fluxos/rodar`. Pedido acima do teto global é **recusado (400) dizendo o teto**;
a dimensão que o pedido omite recebe o padrão, e o teto vale nela do mesmo jeito.

- **Ao bater, a tarefa para** no estado `budget_exceeded`, com a mensagem «gastou X, teto de
  Y», e uma linha `orcamento.parada` na evidência. O custo só se sabe com a resposta: a
  conta é conferida antes de cada chamada e cobrada depois dela, e o número dito é o real.
  A ferramenta pedida na resposta que estourou não roda.
- **Toda chamada ao modelo sob a tarefa conta**, não só a do laço: a ferramenta que chama o
  modelo por dentro (`deep_research`, `code_review`) e o classificador da rota cobram a
  mesma conta. Ao bater, a ferramenta falha e a tarefa para logo depois dela. O modelo do
  classificador entra na lista de quem precisa de preço para orçamento em dinheiro.
- **As filhas contam:** subagentes, passos de fluxo e sub-fluxos cobram a conta da
  tarefa-mãe e do fluxo; abrir filhas não contorna o orçamento.
- **Excesso máximo: uma chamada, também com filhas em paralelo.** Cada chamada reserva, ao
  conferir, a estimativa por cima do que pode gastar (o pedido em bytes + o
  `max_output_tokens`); a que chega quando as irmãs em voo já alcançam o teto **espera a
  vez** em vez de passar junto. Com folga, as filhas seguem em paralelo; perto do teto, uma
  de cada vez. Não cobre provedor que ignore o `max_output_tokens`.
- **A completação do editor** (`/v1/ide/completar`) não roda sob tarefa, e nenhum
  `orcamento.*` a alcança: o teto dela é `ide.ia_teto_tokens_hora` (tokens por hora,
  somadas todas; acima, 429), e o gasto conta no `/metrics` (`phxclaw_ide_completar_*`).
- **Dinheiro sem preço não se confere:** orçamento em dinheiro para um modelo sem preço (ou,
  com a rota, se **qualquer** provedor da cadeia não tem preço) é recusado na criação,
  dizendo qual; `orcamento.*_custo` definido sem `custo.precos` é erro da montagem.

**A bateria entre provedores** (R5): `phxclaw avaliar --provedores A,B --bateria ARQ`, com
`ARQ` = `{"casos": [{"id", "objetivo", "gabarito"}...], "rodadas": 3, "tentativas": 1,
"reamostras": 1000, "semente": N}` (ou uma pasta de casos do `avaliar`). Mede acerto, custo
por acerto (todas as tentativas, inclusive as que falharam, sobre os acertos), duração,
tentativas e intervenção (perguntas a uma pessoa; a bateria roda sem ninguém e conta),
cada uma com o intervalo de 95% por bootstrap dos casos. **Vencedor só quando os
intervalos não se cruzam.** O `phxclaw-model-arena` registra os pares (campeão = o
primeiro provedor) com a janela e o hash dele; o veredito do arena (média) é informativo e
não decide. Provedor que não responde a nenhuma chamada sai **NÃO MEDIDO**. As chaves dos
provedores pagos entram por `phxclaw … chave` (seção 2), nunca por arquivo.

## 13. Auto-evolução (`phxclaw evoluir`)

**Propõe e espera o Go — nunca faz merge** (decisão do dono, 09/10/2026). Um ciclo:

1. **Escolhe UM item** do backlog (`docs/absorcao/phxclaw.json`): `parcial` antes de `nao`,
   depois o nome; fora os `itens_vetados` da política e os que já têm proposta esperando Go.
   `--item NOME` escolhe à mão (item vetado é recusado do mesmo jeito). `phxclaw evoluir itens`
   mostra a ordem e os vetados, com o motivo de cada um.
2. **Clone raso** do commit atual do produto numa tarefa-mãe da pasta do agente, e a
   `git_worktree` de sempre no ramo `evolucao/<item>-<AAAAMMDD-HHMMSS>` — que existe só no
   clone. O que está sem commit na sua árvore **não entra**.
3. **Plano primeiro, depois o laço normal do agente**, com a configuração de subagente (nada
   concedido a mais; sem `ask_user`, porque ninguém acompanha o ciclo). A tarefa só vê a
   pasta do projeto na worktree e não roda git; o commit é feito pela mãe, pelo `git_write`
   (com a varredura de segredos).
4. **Portão do alcance, em código — lista de PERMISSÃO, negado por padrão** (achado A2 da
   revisão de segurança de 09/10/2026: a lista de proibições anterior deixava passar
   `crates/*/Cargo.toml`, `build.rs`, `.cargo/config.toml` e `#[path]` no `lib.rs`, medidos verdes).
   O diff inteiro (`--raw --no-renames` e o unificado `-U0 --text` do mesmo par de commits) passa
   por **um motor só**, o `conferir_alcance`:
   - **Caminho:** só entra o que casa `permitidos` de `config/evolucao-politica.json` **e** o teto
     embutido no código (`PERMISSAO_MAXIMA`): `crates/*/tests/**` e, de módulo de ferramenta pura,
     só `crates/phxclaw-agent/src/calculadora.rs`. Fora disso → **recusado, com o caminho**. Os
     crates e testes de segurança (`NUNCA`: sandbox, broker de segredo, chave, egresso, os testes
     `guardas`, `rbac`, `segredos`, `licenca`, `evolucao`...) ficam fora mesmo dentro do teto.
     Também recusa: `Cargo.toml`, `Cargo.lock`, `build.rs`, `rust-toolchain*`, `rustfmt.toml`,
     `clippy.toml` e qualquer pasta ou arquivo oculto (`.cargo/`, `.git*`) em qualquer lugar;
     arquivo não-`.rs` fora de `tests/`; arquivo apagado; modo executável; link; submódulo;
     caminho fora de `[A-Za-z0-9._/-]`; e diff que o motor não consegue ler inteiro (saída
     cortada, seção sem par no `--raw`, conteúdo binário).
   - **Conteúdo, mesmo dentro do permitido:** `#[path]` (e `cfg_attr`), `include!`/`include_str!`/
     `include_bytes!`, `env!`/`option_env!`, `extern crate`, `#[proc_macro*]`, `macro_rules!`,
     símbolo exportado (`no_mangle`, `export_name`, `#[link]`...) e linha `mod` que entra ou sai
     em `lib.rs`/`main.rs`/`mod.rs` (módulo novo exige humano). No código que **não** é teste, além
     disso: `mod x;`, `unsafe`, `extern`, `std::process`/`Command`, os métodos de disco do `Path`, e
     qualquer caminho fora do `std` puro (`std::fs`, `crate::`, `super::`, outro crate) — a
     ferramenta roda no processo do agente, sem sandbox. Comentário e texto não se pulam: com
     `-U0` não se sabe se a linha está dentro de um, e o falso positivo só manda ao humano.
   - **A política só aperta:** `permitidos` que não cabe no teto (`src/**`, `lib.rs`, `**`...) →
     a política é **recusada inteira**; sem `permitidos` ela nem carrega. `vetados` aperta mais.
5. **Portões no sandbox do `rust_project`:** `fmt` na raiz do projeto, `clippy` (verde só com
   **zero avisos**) e `test` em cada crate tocado; o primeiro vermelho para o ciclo. Depois, a
   **revisão do próprio diff** pelo motor do `code_review`: achado de severidade
   `revisao_bloqueia_em` (padrão `alta`) ou maior, ou revisão que falha, é vermelho.
6. **Tudo verde:** o ramo nasce no seu repositório por `git bundle` (uma ref nova, nunca
   sobrescrita; `main` e a árvore intocados) e o relatório fica em
   `.phxclaw/evolucao/<id>.md` (item e porquê, modelo que rodou, plano, diff resumido,
   portões com segundos, revisão). Estado: **esperando Go**. O clone sai sempre.

<!-- gerado:ajuda:evoluir:inicio -->
Saida de `phxclaw ajuda evoluir`:

```text
Auto-evolucao: propoe um ramo verde e espera o Go (nunca mescla)

USO:
  phxclaw evoluir [--item NOME] [--modelo M] [--projeto DIR] [--pasta DIR] | itens | listar | aprovar ID | rejeitar ID [--motivo TEXTO]

Escolhe um item parcial/nao do backlog fora dos vetados de config/evolucao-politica.json, implementa numa worktree pelo laco do agente (plano primeiro), confere o diff contra a lista de permissao (so testes e ferramentas liberadas; negado por padrao), roda fmt, clippy (zero avisos) e testes dos crates tocados e a revisao do proprio diff. So com tudo verde nasce o ramo evolucao/ID, com relatorio em .phxclaw/evolucao/; aprovar so marca e mostra o comando de merge. Sai 1 se nao ficou verde.

Tambem aceito como: evolve
```
<!-- gerado:ajuda:evoluir:fim -->

| Comando | O que faz |
|---|---|
| `phxclaw evoluir [--item N] [--modelo M]` | um ciclo; sai 1 se não ficou verde |
| `phxclaw evoluir listar` | os registros, mais novos primeiro |
| `phxclaw evoluir aprovar ID` | **só marca** aprovado e imprime o `git merge --no-ff` para você rodar, com aspas de shell; recusa se o ramo andou depois dos portões, e recusa o registro adulterado (id, item, ramo fora de `evolucao/<item>-AAAAMMDD-HHMMSS`, commit que não é sha, repositório que não é a raiz do projeto) antes de rodar qualquer git nele |
| `phxclaw evoluir rejeitar ID --motivo T` | marca rejeitado; o ramo fica (apagar é seu) |

**Modelo:** `--modelo`, senão `modelo.padrao`. A SP000015 pede modelo forte por API; sem chave
(`phxclaw anthropic chave` / `phxclaw openai chave`) o ciclo roda com o que houver e o relatório
diz qual rodou — e diz também quando há chave guardada e ela não foi usada.

**Aprendizado:** cada desfecho (verde, vermelho, recusado, aprovado, rejeitado) vira uma linha em
`.phxclaw/evolucao/desfechos.jsonl`: verde e aprovado como **PENDENTE**, os outros como
**INFRUTÍFERO** com causa e prevenção. Nada vira FRUTÍFERO sozinho, e a escolha do item **não
lê** esse arquivo: nada aprendido muda o comportamento sem Go.

**Custo:** o clone deste monorepo pesa ~47 MB + ~110 MB de checkout (medido em 09/10/2026, 39 s),
e os portões compilam o workspace dentro dele — reserve vários GB de disco por ciclo. Não há
gatilho por agenda: rode por `cron`/`systemd` se quiser periodicidade.

## O que este guia não cobre

- `phxclaw evoluir` **não rodou um ciclo real** contra este repositório com modelo de verdade
  (09/10/2026): sem chave paga, sem Ollama, e com ~5 GB livres não cabe o `target/` de um
  workspace inteiro. Está provado com modelo roteirizado num repositório git temporário
  (`crates/phxclaw-agent/tests/evolucao.rs`).

- A bateria entre provedores (seção 12) contra provedor **real** está **NÃO MEDIDA**
  (09/10/2026): nesta máquina não há chave paga nem Ollama instalado; ela foi provada só com
  provedores falsos locais (`crates/phxclaw-agent/tests/custo_orcamento.rs`).
- O gerador só **exercita** `ajuda`, `config` e `ferramentas`: o guia diz o contrato, não
  prova que ele funciona contra o serviço real. OAuth do Google, Linear, ElevenLabs, Gemini e
  xAI não foram exercitados contra a conta real na escrita deste guia.
- Canais (Telegram, Slack, e-mail…), dispositivos e ponte: ajuda em `phxclaw ajuda canal`,
  `ajuda dispositivos`, `ajuda ponte` e chaves `canais.*` em `phxclaw config mostrar`.

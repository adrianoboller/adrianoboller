# Ondas até 100% de absorção (pedido do dono, 01/10/2026)

Meta: 100% de Hermes, OpenClaw, Claude Code, Codex e OpenJarvis (este acrescentado em 01/10), contando **só o que o agente usa**.
A conta sai de `gerar_absorcao.py`. Este arquivo é só o plano; o número nunca se digita aqui.

Três frentes por onda, no máximo: o disco tem cerca de 3 GB livres e a máquina 4 CPUs.
Entre uma onda e outra, os binários de teste duplicados são podados.

Duas marcas na evidência, e a diferença aparece:
- **provado real**: contra o serviço de verdade;
- **provado contra falso**: servidor falso no teste, porque falta credencial ou hardware.
  Fica no agente, e a prova real fica pendente com o motivo.

## Onda 1: entregue (`cbeec832`)

Arquivos (zip, JSON, XML, PDF), visão (OCR, PNG, SVG), desktop, voz (STT), sistema
Linux, rede, PostgreSQL e Rust.

## Onda 2: em andamento

| frente | fecha |
|---|---|
| MCP nos dois sentidos | mcp_cliente, mcp_servidor |
| memória e skills | memoria, skills, busca_sessoes (parte) |
| Telegram como canal | canal_telegram, canais_push |

## Onda 3

| frente | fecha |
|---|---|
| git, GitHub e código | git, worktrees, github, busca_arquivos, notebook, checkpoints |
| interação | hooks, plan_mode, perguntas_usuario, output_styles, heartbeat |
| canais restantes (na arquitetura do Telegram) | Discord, Slack, WhatsApp, Teams, Signal, Matrix, Google Chat, e-mail de entrada, SMS, webhooks, webchat; e os do OpenJarvis: IRC, Mattermost, Feishu, LINE, Viber, Messenger, Reddit, Mastodon, XMPP, Rocket.Chat, Zulip, Twitch, Nostr |

## Onda 4

| frente | fecha |
|---|---|
| orquestração e plugins | multiagente, workflows, plugins no agente, dispositivos com comando |
| voz e mídia | tts, voz_wake, geracao_midia, canvas |
| editor e remoto | ide_acp, lsp, ide, web_nuvem, remote_control, celular (PWA), busca_x, iMessage (cliente BlueBubbles) |

## Onda 5

O que Codex e OpenJarvis acrescentaram: revisão de código, GitLab, Linear, GitHub Action, regras por comando, gatilhos por evento, ambientes isolados por tarefa; REPL Python, indexação de documentos, importar e otimizar skills, avaliação de modelos por energia e latência, resumo diário e conectores Google.

## O que depende do dono

- Credencial de cada canal e da busca no X, para trocar «contra falso» por «real».
- App de celular nativo: iOS se compila em Mac. A onda 4 entrega o PWA e o diz.
- iMessage: o servidor BlueBubbles roda num Mac. O agente fala com ele; o Mac é do dono.

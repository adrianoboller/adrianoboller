# Decisões do dono — 09/10/2026

Perguntadas pelo orquestrador (só o que a pesquisa não resolve: produto, recurso, risco).

| # | Pergunta | Decisão |
|---|---|---|
| 1 | «Cloudbot» do reel | Clawdbot = OpenClaw (fonte 1). Não é fonte nova; fechar o que falta do OpenClaw. |
| 2 | Itens de produto do n8n | ENTRAM os quatro: usuários e papéis (RBAC), segredos externos, galeria de modelos, Docker e Kubernetes. |
| 3 | Recursos que o dono fornece | Máquina Windows com WinDev (SP000025) e VM na nuvem (ambientes_nuvem, SSH remoto). Mac: não, iMessage segue «depende de recurso». |
| 4 | Credenciais Telegram e n8n | Comando local `phxclaw … chave` na máquina do dono; nunca pelo chat nem pelo repositório. |
| 5 | Nick na sala XMPP | `CONFIAR_NO_NICK` continua desligado por padrão. |
| 6 | Live Share | Terminal compartilhado (um anfitrião, N convidados, cursor do anfitrião); edição simultânea fica declarada como limite. |
| 7 | Auto-evolução (SP000015) | O agente propõe com teste e medição e espera o Go; nunca aplica sozinho. |
| 8 | Rede neural da cognição própria | Só CPU, modelos pequenos (até centenas de MB); treino pesado só na VM de nuvem, com custo por uso. |

Pendente do dono para os itens 3: o acesso à máquina Windows e à VM (endereço e forma de acesso),
entregue pelo mesmo caminho seguro das credenciais.

//Final do Arquivo

## OpenMontage em Rust (09/10/2026, fim do dia)

Palavra do dono: «Deve implementar em rust o open montage. Nosso foco é rust sempre que possível.»

Via escolhida: **reimplementação limpa** (a única que a AGPL-3.0 do OpenMontage permite sem
relicenciar o PhxClaw). Duas equipes separadas:

1. **Especificação** — lê o OpenMontage e escreve o comportamento em palavras nossas
   (`docs/propostas/openmontage-especificacao.md`): etapas, entradas, saídas, decisões. Sem código,
   prompts, YAML ou trechos de texto dele.
2. **Implementação** — escreve em Rust **só a partir da especificação**, sem abrir o repositório do
   OpenMontage. O commit de cada frente diz isso.

O que a licença dele não alcança e o PhxClaw continua respeitando: o FFmpeg entra como programa
externo; dependência GPL (ex. `piper-tts`) não entra; serviços pagos (Remotion para empresas,
provedores de vídeo) só por escolha do operador e com a licença deles.

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

## Phoenix Studio (09/10/2026, noite)

Respostas do dono às perguntas da especificação (`docs/propostas/phoenix-studio-especificacao.md`):

- **Banco-alvo do app gerado:** PhxSql **e** PostgreSQL, com PhxSql como padrão.
- **`Cascade` no excluir vindo de dicionário legado:** importa trocando por **restringir** e **avisa**
  cada relação alterada (a regra primordial «nunca se mata o pai que tem filhos» vale para o app
  gerado nos dois bancos).
- **Escopo:** só **PS0001–PS0003** entram na conta da versão; PS0004–PS0020 nascem ⏸.

## Interface: o mais veloz ganha (09/10/2026, noite)

Palavra do dono, depois da bancada de `docs/propostas/ui-wasm-2026-10.md`: «O mais veloz ganha.»

Medido na mesma tela (tabela de 50 linhas, filtro, formulário, Chromium, 15 corridas): JS puro
0,96 KB gz e 22,8 ms até a 1ª linha; Leptos 62,2 KB e 50,0 ms; Yew 76,5 KB e 53,8 ms; React
69,9 KB e 75,1 ms; Dioxus 151–191 KB e 65–82 ms. Só o JS puro fica separado de todos.

Decisão: **a interface continua em JS puro**; Leptos, Yew, Dioxus e React não entram. A diretriz
«Rust sempre que possível» vale para o resto do produto; na tela, velocidade medida manda. O
`'wasm-unsafe-eval'` não entra na CSP. O endurecimento segue pelo que a bancada achou: os
`innerHTML` (9 linhas no nosso JS, 73 no `phx-grid`) são o ponto de XSS a fechar.

## Laya (09/10/2026, noite)

Empate real levado ao dono: os pesos do `laya-multilingual` (678 MB) cabem na letra de «modelos
pequenos, até centenas de MB», mas o servidor usa 1,8–2,4 GB de RAM e o runtime Python 1,2 GB.
Medido: 16/20 acertos em português (Wilson 95%: 0,58–0,92), p50 246 ms em CPU.

Decisão do dono, seguindo a recomendação: **o adaptador fica no código e DESLIGADO por padrão**
até ser medido na VM do dono; ligar depende de nova medição lá.

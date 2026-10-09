//! A ajuda da CLI sai desta tabela, e so dela.
//!
//! Antes era um bloco de texto unico em `main.rs`: cada frente acrescentava o seu comando no
//! fim, em ingles, sem grupo, e o `match` do despacho e a ajuda podiam divergir calados. Agora
//! cada comando e UMA linha aqui, com grupo, apelidos, uso e descricao, e o teste
//! `todo_comando_do_despacho_esta_na_ajuda` cruza esta tabela com o `match` do `main.rs` nos
//! dois sentidos: comando novo sem linha aqui reprova, linha sem comando tambem.

pub struct Comando {
    pub grupo: Grupo,
    pub nome: &'static str,
    /// Apelidos aceitos pelo despacho (em ingles, por compatibilidade).
    pub apelidos: &'static [&'static str],
    /// Uma linha, para a listagem geral.
    pub resumo: &'static str,
    pub uso: &'static str,
    pub descricao: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Agente,
    EquipeEFluxos,
    Codigo,
    Servicos,
    Credenciais,
    Medicao,
    Diagnostico,
}

impl Grupo {
    pub const TODOS: [Grupo; 7] = [
        Grupo::Agente,
        Grupo::EquipeEFluxos,
        Grupo::Codigo,
        Grupo::Servicos,
        Grupo::Credenciais,
        Grupo::Medicao,
        Grupo::Diagnostico,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            Grupo::Agente => "AGENTE",
            Grupo::EquipeEFluxos => "EQUIPE E FLUXOS",
            Grupo::Codigo => "CODIGO",
            Grupo::Servicos => "SERVICOS (API, CANAIS, EDITORES, DISPOSITIVOS)",
            Grupo::Credenciais => "CREDENCIAIS (vao para o SecretBroker, nunca para arquivo)",
            Grupo::Medicao => "MEDICAO (so numero medido, com faixa min-max, N e data)",
            Grupo::Diagnostico => "DIAGNOSTICO",
        }
    }
}

pub const COMANDOS: &[Comando] = &[
    // --- agente ---
    Comando {
        grupo: Grupo::Agente,
        nome: "agente",
        resumo: "Roda o agente agora, mostrando cada passo",
        apelidos: &["agent"],
        uso: "agente \"objetivo\" [--modelo M] [--plano [--sim]] [--estilo NOME] [--imagem ARQ]... \
              [--gravar ARQ] [--verificar \"CMD\"] [--saida-esquema ARQ] [--pasta DIR]",
        descricao: "Roda o agente agora, mostrando cada passo. --plano: so leitura, executa \
                    depois da aprovacao. Perguntas do agente sao respondidas aqui. --imagem \
                    (ate 4, png/jpeg/gif/webp): o modelo ve a imagem junto do objetivo. --gravar: \
                    cada pedido/resposta do modelo e cada ferramenta em JSONL, segredo redigido \
                    (repita com `repetir`). --verificar: comando que confere o fim no sandbox da \
                    tarefa (pede shell.exec); codigo diferente de 0 recusa a resposta. \
                    --saida-esquema: esquema JSON da resposta final, conferido pelo mesmo \
                    validador dos argumentos das ferramentas.",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "voz",
        resumo: "Conversa por voz, um turno por arquivo WAV",
        apelidos: &["voice"],
        uso: "voz ARQ.wav [ARQ2.wav ...] [--modelo M] [--pasta DIR]",
        descricao: "Conversa por voz, um turno por WAV: escuta -> agente -> WAV falado \
                    (PHXCLAW_WHISPER_* e PHXCLAW_TTS_*; PHXCLAW_STT_PROVEDOR=elevenlabs e \
                    PHXCLAW_TTS_PROVEDOR=elevenlabs trocam o motor pela ElevenLabs).",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "sessoes",
        resumo: "Busca nas tarefas anteriores",
        apelidos: &["sessions"],
        uso: "sessoes \"termo\" [--limite N] [--pasta DIR]",
        descricao: "Busca nas tarefas anteriores (a mesma busca de session_search).",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "resumo",
        resumo: "Resumo das tarefas do dia",
        apelidos: &["summary"],
        uso: "resumo [--data AAAA-MM-DD|hoje|ontem] [--pasta DIR]",
        descricao: "Resumo das tarefas do dia (o mesmo de daily_summary).",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "estilos",
        resumo: "Lista os estilos de saida",
        apelidos: &["styles"],
        uso: "estilos",
        descricao: "Estilos de saida: os embutidos e os de .phxclaw/estilos/*.md.",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "skills",
        resumo: "Importa SKILL.md de outros agentes (scripts desligados, origem com SHA-256)",
        apelidos: &[],
        uso: "skills importar DIR [--com-scripts] [--aceitar-licenca-desconhecida] [--pasta DIR]",
        descricao: "Le todo SKILL.md abaixo de DIR (Claude Code, Codex, OpenClaw, Hermes), traduz \
                    os nomes de ferramenta conhecidos e grava na pasta de skills. scripts/ nao se \
                    copia sem --com-scripts; ORIGEM.json guarda a origem e o SHA-256. Cada skill \
                    passa pela porta de licenca (veja `licenca`): copyleft e recusada, \
                    desconhecida so com --aceitar-licenca-desconhecida (decisao no ORIGEM.json), \
                    compativel entra com o aviso em LICENCA.txt.",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "licenca",
        resumo: "Confere a licenca de uma pasta de terceiro antes de copiar (porta unica)",
        apelidos: &["license"],
        uso: "licenca conferir DIR [--json] [--aceitar-licenca-desconhecida] [--limite DIR]",
        descricao: "Sobe de DIR ate a raiz do repositorio (pasta com .git) ou --limite, le \
                    LICENSE/COPYING, NOTICE, SPDX-License-Identifier e o license do plugin.json, \
                    e classifica pelo SPDX ou pelo texto (nunca pelo nome do arquivo). Vence a \
                    mais restritiva. Compativel com Apache-2.0 (MIT, Apache-2.0, BSD, ISC, 0BSD, \
                    Unlicense, CC0-1.0, Zlib) entra; copyleft (GPL, AGPL, LGPL, MPL, EPL, \
                    CC-BY-SA, -or-later) e recusada sempre; desconhecida so com a opcao. Sai 0 \
                    quando entra e 2 quando recusa; --json e o que os importadores de fora leem.",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "indexar",
        resumo: "Indexa uma pasta de documentos para o doc_search (BM25)",
        apelidos: &["index"],
        uso: "indexar DIR [--pasta DIR]",
        descricao: "Indexa os arquivos de texto de DIR (BM25 por paragrafo) em \
                    <pasta>/indice-docs; reindexar a mesma pasta substitui so ela. doc_search \
                    pede doc.read; PHXCLAW_DOCS_EMBED=ollama:all-minilm reordena por embedding.",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "projeto",
        resumo: "Confia num projeto: os AGENTS.md dele entram no prompt",
        apelidos: &["project"],
        uso: "projeto confiar|mostrar [DIR] [--pasta DIR]",
        descricao: "AGENTS.md da raiz do repositorio ate a pasta (AGENTS.override.md substitui, \
                    CLAUDE.md e reserva), teto de 32 KiB, varredura anti-injecao. So projeto \
                    confiado e lido; mostrar exibe o bloco que vai ao prompt.",
    },
    // --- equipe e fluxos ---
    Comando {
        grupo: Grupo::EquipeEFluxos,
        nome: "equipe",
        resumo: "Os papeis da equipe: listar, mostrar, delegar",
        apelidos: &["team"],
        uso: "equipe [listar [--macroarea X] [--texto Y] | mostrar ID | delegar ID \"tarefa\" [--modelo M]] \
              [--pasta DIR]",
        descricao: "Os papeis de config/agents (PHXCLAW_AGENTES_DIR); delegar roda um papel \
                    como subagente, so com a intersecao das capacidades.",
    },
    Comando {
        grupo: Grupo::EquipeEFluxos,
        nome: "gonogo",
        resumo: "Conselho de integradores: abrir, registrar parecer, ver, decidir Go/NoGo",
        apelidos: &["go-no-go"],
        uso: "gonogo abrir INTEGRACAO --integradores a,b | registrar INTEGRACAO INTEGRADOR OK|NOGO \
              [--erro \"...\"]... [--credencial TOKEN|-] | ver INTEGRACAO | decidir INTEGRACAO \
              [--pasta DIR]",
        descricao: "Quem abre declara o conselho, que nao muda. O primeiro parecer de cada \
                    integrador devolve UMA vez a credencial dele (guardada no SecretBroker); os \
                    seguintes a exigem (--credencial -, pelo stdin). NOGO exige os erros. Algum \
                    NOGO vigente: NOGO; falta parecer do conselho: AGUARDAR; todos OK: GO. So o \
                    mesmo integrador troca o NOGO dele. Saida 0 GO, 2 NOGO, 3 AGUARDAR.",
    },
    Comando {
        grupo: Grupo::EquipeEFluxos,
        nome: "fluxo",
        resumo: "Fluxo em DAG: rodar, retomar, responder esperas, pinar, podar, exportar e listar",
        apelidos: &["workflow"],
        uso: "fluxo rodar ARQ.json [--ate PASSO] [--pins] [--publicada] | retomar TAREFA [ARQ.json] | \
              responder TAREFA TEXTO | esperas | pinar ARQ.json PASSO (--json V | --tarefa T) | \
              despinar ARQ.json PASSO | podar [--dias N] [--max N] | exportar ARQ.json [--saida P] | \
              importar PACOTE.json DESTINO.json | listar [DIR] [--etiqueta E] [--subpasta P] | modelos \
              | usar MODELO DESTINO.json | publicar ARQ.json [--nota T] | versoes ARQ.json | voltar \
              ARQ.json N | restaurar ARQ.json N [--forcar] | exportar --ambiente dev|prod [--fluxos DIR] \
              [--repo DIR] [--commit MSG] | importar --ambiente dev|prod [--fluxos DIR] [--repo DIR] \
              [--sobrescrever] [--modelo M] [--pasta DIR]",
        descricao: "Fluxo declarativo em DAG; cada passo e tarefa, ferramenta, skill, mcp, comando \
                    ou no de controle, pelo mesmo portao. --ate para no passo (inclusive) e grava; \
                    retomar continua dali e pula os passos que deram certo (sem ARQ, pela \
                    definicao que a espera gravou). O passo esperar descarrega o fluxo para o \
                    disco: responder entrega a resposta, esperas retoma as de tempo vencidas. \
                    pinar troca a execucao de um passo pelo dado do ARQ.pins.json, e o pin so \
                    vale no rodar com --pins (gatilho, agenda e sub-fluxo rodam o passo de \
                    verdade); exportar leva um sha256 de conferencia, nao assinatura; podar \
                    segue fluxos.poda_dias/poda_max (sem eles, nada se apaga). \
                    modelos/usar: a galeria de fluxos prontos. publicar/versoes/voltar/restaurar: \
                    as versoes publicadas de um fluxo (rodar --publicada roda a publicada). \
                    exportar/importar --ambiente: o git dos fluxos por ambiente (--commit registra \
                    pelo git do agente).",
    },
    Comando {
        grupo: Grupo::EquipeEFluxos,
        nome: "agenda",
        resumo: "Agenda: listar, adicionar (modelo ou fluxo) e disparar o que venceu",
        apelidos: &["schedule"],
        uso: "agenda listar | adicionar NOME \"OBJETIVO\" (--cada SEG | --cron EXPR) | disparar [--modelo M] \
              [--pasta DIR]",
        descricao: "A mesma agenda do servidor (agenda.json da pasta) e o mesmo disparo da API. \
                    Objetivo `fluxo: ARQ.json` roda o fluxo em DAG sem modelo; qualquer outro \
                    texto vira tarefa do modelo.",
    },
    // --- codigo ---
    Comando {
        grupo: Grupo::Codigo,
        nome: "revisar",
        resumo: "Revisao de codigo de um diff ou PR (serve para CI)",
        apelidos: &["review"],
        uso: "revisar [--repo DIR] [--rev R] [--cached] [--diff ARQ|-] [--pr github:dono/proj#7] \
              [--evento ARQ] [--comentar] [--foco TEXTO] [--modelo M] [--falhar-em alta] [--pasta DIR]",
        descricao: "Revisao de um diff pelo modelo do agente (o mesmo motor de code_review), \
                    achados em JSON; --falhar-em sai 1 nessa severidade ou pior (CI); --evento le \
                    o PR do GITHUB_EVENT_PATH e --comentar publica a revisao no PR (GitHub Action).",
    },
    Comando {
        grupo: Grupo::Codigo,
        nome: "tarefa",
        resumo: "Tarefas do projeto (.phxclaw/tarefas.json): listar e rodar",
        apelidos: &["task"],
        uso: "tarefa listar | rodar NOME",
        descricao: "As tarefas que o projeto declara em .phxclaw/tarefas.json (nome, comando, \
                    args, cwd, grupo build|test|run), pelo mesmo motor da ferramenta \
                    project_task: rodam no sandbox, sem rede, e saem com o codigo da tarefa.",
    },
    Comando {
        grupo: Grupo::Codigo,
        nome: "testes",
        resumo: "Explorador de testes: a arvore e um no dela (Rust e Python)",
        apelidos: &["tests"],
        uso: "testes listar | rodar NO [--projeto DIR] [--caminho REL] [--linguagem rust|python]",
        descricao: "A arvore crate/modulo/teste (cargo test -- --list) ou arquivo/teste (pytest \
                    --collect-only), e a corrida de UM no (CRATE/modulo::teste ou nodeid do \
                    pytest), pelas mesmas funcoes de test_list/test_run do agente; sai 1 se falhou.",
    },
    Comando {
        grupo: Grupo::Codigo,
        nome: "evoluir",
        resumo: "Auto-evolucao: propoe um ramo verde e espera o Go (nunca mescla)",
        apelidos: &["evolve"],
        uso: "evoluir [--item NOME] [--modelo M] [--projeto DIR] [--pasta DIR] | itens | listar \
              | aprovar ID | rejeitar ID [--motivo TEXTO]",
        descricao: "Escolhe um item parcial/nao do backlog fora dos vetados de \
                    config/evolucao-politica.json, implementa numa worktree pelo laco do agente \
                    (plano primeiro), confere o diff contra os caminhos vetados, roda fmt, clippy \
                    (zero avisos) e testes dos crates tocados e a revisao do proprio diff. So com \
                    tudo verde nasce o ramo evolucao/ID, com relatorio em .phxclaw/evolucao/; \
                    aprovar so marca e mostra o comando de merge. Sai 1 se nao ficou verde.",
    },
    // --- servicos ---
    Comando {
        grupo: Grupo::Servicos,
        nome: "servir",
        resumo: "API de tarefas, UI web (PWA), gatilhos e heartbeat",
        apelidos: &["serve"],
        uso: "servir [--porta 8787] [--pasta DIR] [--canal NOME] [--dispositivos --cert C --chave K \
              --tokens F [--porta-dispositivos 8788]] [--ponte wss://H:P/ [--ponte-ca PEM] \
              [--ponte-tenant U] [--ponte-no U] [--sem-porta]]",
        descricao: "API de tarefas (criar, acompanhar, aprovar, responder, cancelar, artefatos, \
                    agendas), heartbeat, gatilhos, hooks e regras de .phxclaw/, e a UI web \
                    instalavel (PWA) em /. --ponte conecta PARA FORA num `phxclaw ponte`.",
    },
    Comando {
        grupo: Grupo::Servicos,
        nome: "canal",
        resumo: "Canal de mensagens como entrada do agente (25 canais)",
        apelidos: &["channel"],
        uso: "canal NOME [--pasta DIR] [--escuta ENDERECO]",
        descricao: "Canal de mensagens como entrada do agente: telegram, discord, slack, whatsapp, \
                    teams, matrix, email, webhook, webchat, signal, googlechat, sms, mattermost, \
                    rocketchat, zulip, irc, xmpp, mastodon, line, viber, messenger, feishu, \
                    reddit, twitch, nostr (PHXCLAW_<NOME>_PERMITIDOS=id,id).",
    },
    Comando {
        grupo: Grupo::Servicos,
        nome: "mcp-serve",
        resumo: "Ferramentas do agente como servidor MCP (stdio)",
        apelidos: &[],
        uso: "mcp-serve [--trabalho DIR] [--modelo M] [--pasta DIR]",
        descricao: "As ferramentas do agente como servidor MCP por stdio (mesma \
                    PHXCLAW_CAPACIDADES).",
    },
    Comando {
        grupo: Grupo::Servicos,
        nome: "acp",
        resumo: "Agent Client Protocol para editores (stdio)",
        apelidos: &[],
        uso: "acp [--modelo M] [--pasta DIR]",
        descricao: "Agent Client Protocol por stdio para editores (Zed, JetBrains); projeto = cwd.",
    },
    Comando {
        grupo: Grupo::Servicos,
        nome: "dispositivos",
        resumo: "Servidor WSS de dispositivos pareados",
        apelidos: &["devices"],
        uso: "dispositivos --cert C --chave K --tokens F [--porta 8788] | dispositivos chave [--pasta DIR]",
        descricao: "Servidor WSS de dispositivos (TLS, pareamento por token de uso unico, \
                    envelopes assinados). chave: le PHXCLAW_ENROLLMENT_TOKEN do AMBIENTE do \
                    comando e guarda no broker; o `servir --ponte` o le do ambiente, senao do \
                    broker.",
    },
    Comando {
        grupo: Grupo::Servicos,
        nome: "ponte",
        resumo: "Ponte de controle remoto (o agente se liga para fora)",
        apelidos: &["bridge"],
        uso: "ponte --cert PEM --chave PEM --tokens ARQ [--porta 8790] [--porta-wss 8791] [--pasta DIR] \
              | ponte chave [--pasta DIR]",
        descricao: "Ponte de controle remoto: serve a UI ao cliente e o WSS onde o agente se liga \
                    para fora com `servir --ponte`. chave: le PHXCLAW_PONTE_TOKEN do AMBIENTE do \
                    comando e guarda no broker; a ponte o le do ambiente, senao do broker, senao \
                    de <pasta>/ponte.token.",
    },
    // --- credenciais ---
    Comando {
        grupo: Grupo::Credenciais,
        nome: "forja",
        resumo: "Guarda o token do GitHub ou do GitLab",
        apelidos: &["forge"],
        uso: "forja token github|gitlab [--pasta DIR]",
        descricao: "Le PHXCLAW_GITHUB_TOKEN / PHXCLAW_GITLAB_TOKEN do AMBIENTE do comando e guarda \
                    no broker de <pasta>/forja; as ferramentas github e gitlab leem SO do broker \
                    e so existem depois disto.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "mcp",
        resumo: "Credencial de um servidor MCP remoto (Bearer ou OAuth)",
        apelidos: &[],
        uso: "mcp token|login NOME [--pasta DIR]",
        descricao: "Servidor declarado em PHXCLAW_MCP_CONFIG: token le PHXCLAW_MCP_TOKEN do \
                    AMBIENTE do comando e o guarda como Bearer (Linear); login faz OAuth 2.0 + \
                    PKCE no navegador local e guarda o refresh token (Google; segredo do cliente \
                    lido de PHXCLAW_MCP_SEGREDO_CLIENTE). O agente le SO do broker de \
                    <pasta>/mcp, por (nome, URL): URL nova pede credencial nova.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "credencial",
        resumo: "Segredo de uma credencial nomeada do no HTTP (http_request)",
        apelidos: &["credential"],
        uso: "credencial guardar|renovacao|login NOME [--pasta DIR]",
        descricao: "A credencial e declarada em <pasta>/http.json (tipo bearer, basico, \
                    cabecalho ou oauth2, e as ORIGENS a que pode ir). guardar le uma linha da \
                    ENTRADA PADRAO (o valor fixo, ou o segredo do cliente no oauth2) e a guarda \
                    no broker de <pasta>/credenciais; renovacao guarda um refresh token; login \
                    faz OAuth 2.0 + PKCE no navegador local. O fluxo e a ferramenta dizem so o \
                    NOME.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "elevenlabs",
        resumo: "Guarda a chave da ElevenLabs ou lista as vozes da conta",
        apelidos: &[],
        uso: "elevenlabs chave|vozes [--busca TEXTO] [--pasta DIR]",
        descricao: "chave: le PHXCLAW_ELEVENLABS_API_KEY do AMBIENTE do comando e guarda no \
                    broker de <pasta>/elevenlabs; speak, transcribe e voice_list leem SO do broker \
                    (PHXCLAW_TTS_PROVEDOR=elevenlabs, PHXCLAW_ELEVENLABS_VOZ, \
                    PHXCLAW_STT_PROVEDOR=elevenlabs). vozes: lista voice_id, nome e categoria.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "gemini",
        resumo: "Guarda a chave da Gemini API (Nano Banana no image_generate)",
        apelidos: &[],
        uso: "gemini chave [--pasta DIR]",
        descricao: "Le PHXCLAW_GEMINI_API_KEY (ou GEMINI_API_KEY) do AMBIENTE do comando e guarda \
                    no broker de <pasta>/gemini; image_generate le SO do broker, pelo Nano Banana \
                    com PHXCLAW_IMAGEM_PROVEDOR=nanobanana.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "xai",
        resumo: "Guarda a chave da xAI (habilita x_search)",
        apelidos: &[],
        uso: "xai chave [--pasta DIR]",
        descricao: "Le PHXCLAW_XAI_API_KEY do AMBIENTE do comando e guarda no broker de \
                    <pasta>/xai; x_search le SO do broker (capacidade x.search, fora do padrao).",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "n8n",
        resumo: "Guarda a chave da API e o segredo do webhook do n8n (habilita n8n_workflow)",
        apelidos: &[],
        uso: "n8n chave|segredo [--pasta DIR]",
        descricao: "`chave` le PHXCLAW_N8N_API_KEY e `segredo` le PHXCLAW_N8N_WEBHOOK_SEGREDO do \
                    AMBIENTE do comando e guarda no broker de <pasta>/n8n. A ferramenta n8n_workflow \
                    existe com PHXCLAW_N8N_URL (capacidade automacao.n8n, fora do padrao): run \
                    dispara o webhook de um fluxo assinando com o segredo; list/status leem a API \
                    publica com a chave. Ver docs/N8N.md.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "api",
        resumo: "Chama qualquer rota da API do `servir`; guarda o Bearer dela",
        apelidos: &[],
        uso: "api METODO ROTA [--corpo ARQ|-|JSON] [--consulta CHAVE=VALOR]... [--projeto P] \
              [--url URL] [--saida ARQ] [--pasta DIR] | api rotas [--json] | api chave [--pasta DIR]",
        descricao: "METODO ROTA: o pedido vai ao servidor de pe (--url, senao api.url_cliente) \
                    pelo cliente do SDK, com o token local (PHXCLAW_API_TOKEN, o broker ou \
                    <pasta>/api.token); responde o MESMO handler, atras do mesmo portao do RBAC. \
                    --projeto vai no X-PhxClaw-Projeto; sai 0 em 2xx e 1 no resto, com o HTTP no \
                    stderr. rotas: a tabela inteira (metodo, rota, parametros e o comando local \
                    equivalente, quando ha). chave: le PHXCLAW_API_TOKEN (24+ caracteres) do \
                    AMBIENTE do comando e guarda no broker de <pasta>/api. O servir le do \
                    ambiente, senao do broker, senao de <pasta>/api.token (gerado na primeira vez).",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "usuario",
        resumo: "Usuarios, projetos e papeis da API (token so como hash)",
        apelidos: &["user"],
        uso: "usuario criar NOME --papel owner|admin|member|leitor [--projeto P]... | listar | \
              chave NOME | remover NOME [--pasta DIR]",
        descricao: "Grava <pasta>/usuarios.json (0600) com o sal e o SHA-256 de cada token; o \
                    token aparece uma vez, na criacao ou na troca de chave. Sem usuarios a API \
                    usa so o Bearer do api.token; com eles, o api.token vale como owner. member \
                    e leitor so alcancam os projetos deles (cabecalho X-PhxClaw-Projeto).",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "openai",
        resumo: "Guarda a chave da OpenAI (modelos openai:*)",
        apelidos: &[],
        uso: "openai chave [--pasta DIR]",
        descricao: "Le OPENAI_API_KEY do AMBIENTE do comando e guarda no broker de \
                    <pasta>/openai. O agente com openai:* le SO do broker: variavel no \
                    ambiente sem chave guardada nao vale, e o erro cita este comando.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "anthropic",
        resumo: "Guarda a chave da Anthropic (modelos anthropic:*)",
        apelidos: &[],
        uso: "anthropic chave [--pasta DIR]",
        descricao: "Le ANTHROPIC_API_KEY do AMBIENTE do comando e guarda no broker de \
                    <pasta>/anthropic. O agente com anthropic:* le SO do broker; a chave da \
                    Gemini (gemini:*) e a mesma de `gemini chave`.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "imagem",
        resumo: "Guarda a chave do gerador de imagem openai",
        apelidos: &["image"],
        uso: "imagem chave [--pasta DIR]",
        descricao: "Le PHXCLAW_IMAGEM_CHAVE do AMBIENTE do comando e guarda no broker de \
                    <pasta>/imagem; image_generate com PHXCLAW_IMAGEM_PROVEDOR=openai le do \
                    ambiente, senao do broker.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "email",
        resumo: "Guarda a senha do SMTP (send_email e canal de e-mail)",
        apelidos: &[],
        uso: "email chave [--pasta DIR]",
        descricao: "Le PHXCLAW_SMTP_PASSWORD do AMBIENTE do comando e guarda no broker do canal \
                    (<pasta>/canal, o mesmo segredo que o canal de e-mail usa); send_email e o \
                    canal leem do ambiente, senao do broker.",
    },
    Comando {
        grupo: Grupo::Credenciais,
        nome: "plugins",
        resumo: "Semente de assinatura, reassinar manifestos, loja: catalogo, instalar, empacotar",
        apelidos: &[],
        uso: "plugins chave [--pasta DIR] | plugins assinar [DIR] [--raiz DIR] [--pasta DIR] | \
              plugins catalogo [BUSCA] | plugins instalar NOME | plugins empacotar DIR SAIDA.tar",
        descricao: "chave: le PHXCLAW_PLUGIN_SIGNING_KEY do AMBIENTE do comando e guarda no broker \
                    de <pasta>/plugins. assinar: reassina os *.plugin.json de DIR (padrao \
                    plugins/builtin/manifests sob --raiz, o repositorio) com a semente do \
                    ambiente, senao do broker, conferida contra config/trust/plugin-signers.json. \
                    catalogo/instalar: a loja de pacotes.catalogo (so pacote assinado, sha256 do \
                    catalogo conferido antes de abrir), pela mesma Loja da ferramenta \
                    plugin_catalog. empacotar: a pasta assinada num .tar para publicar.",
    },
    // --- medicao ---
    Comando {
        grupo: Grupo::Medicao,
        nome: "repetir",
        resumo: "Repete uma gravacao sem modelo e acusa a divergencia com o passo",
        apelidos: &["replay"],
        uso: "repetir ARQ.jsonl [--ferramentas reais|gravadas] [--pasta DIR]",
        descricao: "Repete uma gravacao (agente --gravar, ou gravar:true na API) com as respostas \
                    gravadas no lugar do modelo. reais (padrao) roda as ferramentas sob o portao \
                    de sempre numa tarefa nova; gravadas devolve as saidas gravadas sem efeito. \
                    Sai com 3 e o passo da primeira divergencia se a sequencia mudar.",
    },
    Comando {
        grupo: Grupo::Medicao,
        nome: "medir",
        resumo: "Soma uma gravacao por tarefa: chamadas, duracao e tokens de cada passo",
        apelidos: &["measure"],
        uso: "medir ARQ.jsonl [--json]",
        descricao: "Le uma gravacao (agente --gravar, avaliar, gravar:true na API) e soma por \
                    tarefa -- a raiz e cada subagente pelo passo_pai -- as chamadas ao modelo e \
                    as ferramentas, a duracao de cada lado e os tokens. Tokens so somam quando \
                    o provedor os informou em toda chamada; senao diz quantas ficaram sem. \
                    Gravacao da versao 1 (sem medidas) sai como «não medido», nunca estimada.",
    },
    Comando {
        grupo: Grupo::Medicao,
        nome: "avaliar",
        resumo: "Compara modelos pelo agente: p50/p95, tokens/s, CPU, energia, acerto, nota e custo",
        apelidos: &["eval"],
        uso: "avaliar --modelos A,B --tarefas DIR [--rodadas N] [--saida DIR] [--pasta DIR] | \
              avaliar --provedores A,B --bateria ARQ|DIR [--saida DIR] [--pasta DIR]",
        descricao: "Roda cada caso de DIR (*.json com gabarito, ou *.jsonl gravado) N vezes por \
                    modelo, pelo agente inteiro. Cada numero sai com faixa min-max, N e data; \
                    vencedor so quando as faixas nao se cruzam. Alem do acerto, a nota parcial \
                    de ferramentas (conjunto e sequencia por LCS, 0 a 1, deterministica, nunca \
                    por juiz) e o agrupamento pelo sha256 do prompt e das skills de cada \
                    execucao. Energia so de RAPL ou NVIDIA: sem eles, «não medida». Cada \
                    execucao fica gravada em SAIDA/gravacoes. Custo por acerto: o custo de todas \
                    as execucoes (as que falharam tambem) sobre as que acertaram, pela tabela \
                    custo.precos; sem preco, «não medido». Com --provedores e --bateria, a \
                    bateria comum: o mesmo gabarito por provedor, medindo acerto, custo por \
                    acerto, duracao, tentativas e intervencao, com intervalo de 95% por \
                    bootstrap; vencedor so com os intervalos separados. O phxclaw-model-arena \
                    registra os pares (informativo, nao decide). Provedor que nao respondeu sai \
                    NÃO MEDIDO.",
    },
    Comando {
        grupo: Grupo::Medicao,
        nome: "ui",
        resumo: "Prova as telas geradas: ida e volta (fidelidade) e larguras (responsivo)",
        apelidos: &[],
        uso: "ui fidelidade [--telas N] [--modelo N] [--prazo S] [--saida DIR] [--capturas DIR] \
              | responsivo [--alvo html|bootstrap] [--bootstrap-css ARQ] [--telas N] [--saida DIR] \
              [--capturas DIR] | responsivo --phx ARQ.phx.json [--bootstrap-css ARQ] \
              [--exemplo INDEX.html] | importar ARQ.phx.json [--saida DIR] [--bootstrap-css CAMINHO]",
        descricao: "Gera as telas do gabarito (20, do design_erp_ui), desenha cada uma no \
                    Chromium headless, le o PNG pelo screenshot_to_erp_ui (OCR + layout; com o \
                    modelo de visao so nas N primeiras de --modelo, ~90 s cada, --prazo por \
                    pergunta) e compara com o \
                    UI-IR de origem: achados, perdidos, inventados, rotulo, tipo, ordem (Kendall), \
                    grupo e posicao, com faixa min-max e data. Grava SAIDA/fidelidade-DATA.json \
                    (padrao docs/ui/fidelidade); --capturas guarda os PNG para refazer o OCR. \
                    responsivo: as mesmas 20 telas em 320, 390, md e 1280 px e um pixel antes e no \
                    ponto de cada quebra de breakpoints.json; mede rolagem da pagina, cortados, \
                    sobrepostos, alvos de toque, Tab (anel e ordem), colunas contra o IR, painel \
                    de 360 px em janela de 1920 e redimensionar sem recriar no. --alvo bootstrap \
                    pede o bootstrap.min.css LOCAL (--bootstrap-css) e compara com o phoenix. \
                    Grava SAIDA/responsivo-ALVO-DATA.json; o arquivo do dia nao se sobrescreve. \
                    --phx: o PHX JSON do Phoenix nos dois adaptadores (rolagem, colunas por \
                    largura e por painel, fronteiras, identidades, casca) e, com --exemplo, as \
                    colunas do index.html do dono nas mesmas larguras; grava phx-exemplo-DATA.json. \
                    importar: PHX JSON -> UI-IR + telas phoenix e Bootstrap (folha LOCAL) e o \
                    PHX JSON escrito de volta, dizendo se a ida e volta saiu identica.",
    },
    Comando {
        grupo: Grupo::Medicao,
        nome: "skill",
        resumo: "Otimiza uma skill por A/B medido; so promove sem cruzar faixas",
        apelidos: &[],
        uso: "skill otimizar NOME --tarefas DIR [--modelo M] [--rodadas N] [--pasta DIR]",
        descricao: "O modelo escreve uma variante do SKILL.md; original e variante rodam os mesmos \
                    casos (gravacoes ou casos com gabarito). A variante so substitui a original \
                    quando a faixa de acerto dela fica inteira acima; a decisao, com os numeros, \
                    vai para <skill>/otimizacao.jsonl.",
    },
    // --- a ferramenta avulsa (grupo agente, ao lado da lista de diagnostico) ---
    Comando {
        grupo: Grupo::Agente,
        nome: "ferramenta",
        resumo: "Roda UMA ferramenta montada, com os parametros do esquema dela",
        apelidos: &["tool"],
        uso: "ferramenta [NOME [--PARAM VALOR]... [--json ARQ|-] [--trabalho DIR] [--modelo M] \
              [--pasta DIR] | NOME --ajuda|--help | --nomes]",
        descricao: "Os parametros saem do esquema da ferramenta (o mesmo que o modelo le): TEXTO, \
                    N, REAL, booleano (--x sozinho e true), lista (--x repetido) e JSON para \
                    objeto. --json da a base dos argumentos e --PARAM vai por cima. A chamada \
                    passa pelo MESMO portao do laco do modelo e do mcp-serve: capacidade \
                    (PHXCLAW_CAPACIDADES), esquema, regras de comando, hooks e evidencia no \
                    ledger (o caminho sai no stderr). Sem NOME, lista todas com a linha de uso; \
                    NOME --ajuda mostra tipo, obrigatorio, opcoes e padrao de cada parametro. \
                    Sai 0 ok, 2 negado pelo portao, 1 erro.",
    },
    // --- diagnostico ---
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "completar",
        resumo: "Script de completar para bash, zsh, fish ou PowerShell",
        apelidos: &["completion"],
        uso: "completar bash|zsh|fish|powershell",
        descricao: "Gerado do mesmo inventario da ajuda: os comandos, os apelidos e as opcoes \
                    do uso de cada um; o nome da ferramenta se completa perguntando ao proprio \
                    binario (`ferramenta --nomes`). Ex.: `source <(phxclaw completar bash)`.",
    },
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "ferramentas",
        resumo: "Ferramentas montadas nesta maquina, em JSON",
        apelidos: &["tools"],
        uso: "ferramentas",
        descricao: "As ferramentas montadas nesta maquina, em JSON (tela Ferramentas).",
    },
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "core",
        resumo: "Estado medido do runtime",
        apelidos: &[],
        uso: "core status",
        descricao: "Estado medido do runtime (sandbox, navegador, servidor de modelo).",
    },
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "db",
        resumo: "Plano de instalacao do PostgreSQL",
        apelidos: &[],
        uso: "db plan [plataforma]",
        descricao: "Plano de instalacao gerenciada do PostgreSQL.",
    },
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "config",
        resumo: "O config.json: valor efetivo e origem de cada chave, validar, definir",
        apelidos: &[],
        uso: "config mostrar [--json] | validar ARQ | exemplo | definir CHAVE VALOR|--remover \
              [--projeto|--perfil NOME] | perfil listar|usar NOME|nenhum|criar NOME [--copiar-base] \
              | sincronizar enviar|receber --ponte URL [--forcar] [--pasta DIR]",
        descricao: "Precedencia: {PRECEDENCIA} (a do catalogo). mostrar: valor e origem de \
                    cada chave (segredo so aparece como no broker/ausente). definir grava atomico, \
                    com revisao. perfil: camadas nomeadas no config.json da pasta (`perfis`, \
                    `perfil_ativo`; PHXCLAW_PERFIL escolhe por uma execucao). sincronizar: o \
                    config.json vai e vem pela ponte, sem a revisao e sem segredo; os dois lados \
                    mudados e conflito com as duas revisoes. Segredo nunca vai ao arquivo: o \
                    comando que o guarda no broker vem na recusa. Chave desconhecida e tipo \
                    errado sao erro.",
    },
    Comando {
        grupo: Grupo::Diagnostico,
        nome: "version",
        resumo: "Versao",
        apelidos: &["--version", "-V"],
        uso: "version",
        descricao: "Versao.",
    },
];

/// Comando pelo nome ou apelido.
pub fn achar(nome: &str) -> Option<&'static Comando> {
    COMANDOS
        .iter()
        .find(|c| c.nome == nome || c.apelidos.contains(&nome))
}

/// Ajuda inteira, agrupada.
pub fn texto(produto: &str, versao: &str, cli: &str) -> String {
    let mut s = format!(
        "{produto} {versao}\n\nUSO:\n  {cli} <COMANDO> [opcoes]\n  {cli} ajuda <COMANDO>   \
         (ou {cli} <COMANDO> --ajuda)\n  {cli} ajuda --tudo   (mais cada ferramenta com os \
         parametros e cada rota da API)\n"
    );
    let largura = COMANDOS.iter().map(|c| c.nome.len()).max().unwrap_or(0);
    for g in Grupo::TODOS {
        let do_grupo: Vec<_> = COMANDOS.iter().filter(|c| c.grupo == g).collect();
        if do_grupo.is_empty() {
            continue;
        }
        s.push_str(&format!("\n{}:\n", g.titulo()));
        for c in do_grupo {
            s.push_str(&format!("  {:<largura$}  {}\n", c.nome, c.resumo));
            // Todos os parametros de todo comando, na listagem geral: o uso inteiro, quebrado
            // na largura da tela.
            let recuo = " ".repeat(largura + 4);
            s.push_str(&quebrar(&format!("{cli} {}", c.uso), &recuo, LARGURA));
        }
    }
    s.push_str(
        "\nMODELOS: ollama:<modelo> (local), openai:<modelo>, anthropic:<modelo>, gemini:<modelo>\n\
         \x20        (chaves de OPENAI_API_KEY / ANTHROPIC_API_KEY / GEMINI_API_KEY)\n\
         POLITICA: PHXCLAW_CAPACIDADES=web.search,web.browse,fs.read,fs.write,... \
         (padrao: CAPACIDADES_PADRAO)\n",
    );
    s
}

/// A largura da ajuda impressa (o teste reprova linha acima de 110).
pub const LARGURA: usize = 106;

/// `texto` quebrado em linhas de ate `largura`, cada uma com `recuo`; prefere quebrar
/// antes de um ` | ` (a fronteira entre formas do mesmo comando).
pub fn quebrar(texto: &str, recuo: &str, largura: usize) -> String {
    let mut s = String::new();
    let mut linha = String::new();
    for palavra in texto.split_whitespace() {
        let cabe = recuo.len() + linha.len() + 1 + palavra.len() <= largura;
        if !linha.is_empty() && (!cabe || (palavra == "|" && linha.len() > largura / 2)) {
            s.push_str(&format!("{recuo}{linha}\n"));
            linha.clear();
        }
        if !linha.is_empty() {
            linha.push(' ');
        }
        linha.push_str(palavra);
    }
    if !linha.is_empty() {
        s.push_str(&format!("{recuo}{linha}\n"));
    }
    s
}

/// A ajuda inteira: os comandos, cada ferramenta montada com os parametros do esquema e
/// cada rota da API. As tres listas saem do codigo (a tabela daqui, o esquema de cada
/// ferramenta, `rotas::ROTAS`), e a catraca dos testes cruza cada uma com a sua origem.
pub fn tudo(produto: &str, versao: &str, cli: &str, agente: &phxclaw_agent::Agent) -> String {
    let mut s = texto(produto, versao, cli);
    s.push_str("\nFERRAMENTAS (`ferramenta NOME --ajuda` traz tipo, opcoes e padrao):\n");
    s.push_str(&crate::ferramenta::lista(agente, cli));
    s.push_str("\nROTAS DA API (`api METODO ROTA`):\n");
    s.push_str(&crate::rota::tabela(cli));
    s
}

/// As palavras que o shell completa depois do comando: os subcomandos e as opcoes do uso.
fn palavras_do_uso(c: &Comando) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for bruto in c
        .uso
        .split(|ch: char| ch.is_whitespace() || "[]()|".contains(ch))
    {
        // Texto entre aspas e exemplo de valor (`"objetivo"`), nao palavra do comando.
        if bruto.starts_with('"') {
            continue;
        }
        let p = bruto.trim_matches(|ch: char| ch == ',' || ch == '.' || ch == '"');
        let opcao = p.starts_with("--") && p.len() > 2;
        let palavra = !p.is_empty()
            && p.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-')
            && !p.starts_with('-');
        if (opcao || palavra) && p != c.nome && !v.iter().any(|x| x == p) {
            v.push(p.to_string());
        }
    }
    v
}

/// O script de completar do shell pedido, do mesmo inventario da ajuda.
pub fn completar(shell: &str, cli: &str) -> Result<String, String> {
    let nomes: Vec<&str> = COMANDOS
        .iter()
        .flat_map(|c| std::iter::once(c.nome).chain(c.apelidos.iter().copied()))
        .filter(|n| !n.starts_with('-'))
        .collect();
    let por_comando: Vec<(Vec<&str>, Vec<String>)> = COMANDOS
        .iter()
        .map(|c| {
            (
                std::iter::once(c.nome)
                    .chain(c.apelidos.iter().copied())
                    .collect(),
                palavras_do_uso(c),
            )
        })
        .collect();
    let f = cli.replace('-', "_");
    Ok(match shell {
        "bash" | "zsh" => {
            let mut s = String::new();
            if shell == "zsh" {
                s.push_str("autoload -U +X bashcompinit && bashcompinit\n");
            }
            s.push_str(&format!(
                "_{f}() {{\n  local cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n  \
                 if [ \"$COMP_CWORD\" -eq 1 ]; then\n    \
                 COMPREPLY=( $(compgen -W \"{}\" -- \"$cur\") ); return\n  fi\n  \
                 case \"${{COMP_WORDS[1]}}\" in\n",
                nomes.join(" ")
            ));
            for (ns, ps) in &por_comando {
                let lista = if ns.contains(&"ferramenta") {
                    // O nome da ferramenta vem do proprio binario: a montagem muda por maquina.
                    format!("$({cli} ferramenta --nomes 2>/dev/null) {}", ps.join(" "))
                } else {
                    ps.join(" ")
                };
                s.push_str(&format!(
                    "    {}) COMPREPLY=( $(compgen -W \"{lista}\" -- \"$cur\") ) ;;\n",
                    ns.join("|")
                ));
            }
            s.push_str(&format!("  esac\n}}\ncomplete -o default -F _{f} {cli}\n"));
            s
        }
        "fish" => {
            let mut s = format!("complete -c {cli} -f\n");
            for c in COMANDOS {
                for n in std::iter::once(c.nome).chain(c.apelidos.iter().copied()) {
                    if !n.starts_with('-') {
                        s.push_str(&format!(
                            "complete -c {cli} -n __fish_use_subcommand -a {n} -d '{}'\n",
                            c.resumo.replace('\'', " ")
                        ));
                    }
                }
            }
            for (ns, ps) in &por_comando {
                let mut lista = ps.join(" ");
                if ns.contains(&"ferramenta") {
                    lista = format!("({cli} ferramenta --nomes 2>/dev/null) {lista}");
                }
                if !lista.is_empty() {
                    s.push_str(&format!(
                        "complete -c {cli} -n '__fish_seen_subcommand_from {}' -a \"{lista}\"\n",
                        ns.join(" ")
                    ));
                }
            }
            s
        }
        "powershell" | "pwsh" => {
            let mut s = format!(
                "Register-ArgumentCompleter -Native -CommandName {cli} -ScriptBlock {{\n  \
                 param($palavra, $ast, $cursor)\n  $p = @($ast.CommandElements | \
                 ForEach-Object {{ $_.ToString() }})\n  $mapa = @{{\n"
            );
            for (ns, ps) in &por_comando {
                for n in ns.iter().filter(|n| !n.starts_with('-')) {
                    s.push_str(&format!(
                        "    '{n}' = @({})\n",
                        ps.iter()
                            .map(|p| format!("'{p}'"))
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
            }
            s.push_str(&format!(
                "  }}\n  if ($p.Count -le 1 -or ($p.Count -eq 2 -and $palavra)) {{ $opcoes = \
                 $mapa.Keys }}\n  elseif ($p[1] -in 'ferramenta','tool' -and $p.Count -le 3) \
                 {{ $opcoes = @(& {cli} ferramenta --nomes 2>$null) }}\n  else {{ $opcoes = \
                 $mapa[$p[1]] }}\n  $opcoes | Where-Object {{ $_ -like \"$palavra*\" }} | \
                 Sort-Object | ForEach-Object {{\n    \
                 [System.Management.Automation.CompletionResult]::new($_, $_, \
                 'ParameterValue', $_)\n  }}\n}}\n"
            ));
            s
        }
        outro => {
            return Err(format!(
                "shell {outro:?}: use bash, zsh, fish ou powershell"
            ));
        }
    })
}

/// Ajuda de um comando: uso completo, apelidos e a descricao inteira.
pub fn de(c: &Comando, cli: &str) -> String {
    // `{PRECEDENCIA}` sai do motor (carga::precedencia_texto), nunca digitada aqui.
    let descricao = c.descricao.replace(
        "{PRECEDENCIA}",
        &phxclaw_config_runtime::agente::carga::precedencia_texto(),
    );
    let mut s = format!("{}\n\nUSO:\n  {cli} {}\n\n{}\n", c.resumo, c.uso, descricao);
    if !c.apelidos.is_empty() {
        s.push_str(&format!(
            "\nTambem aceito como: {}\n",
            c.apelidos.join(", ")
        ));
    }
    s
}

/// Sugestao para comando desconhecido: o nome conhecido mais proximo, se for perto.
pub fn sugestao(digitado: &str) -> Option<&'static str> {
    COMANDOS
        .iter()
        .flat_map(|c| {
            std::iter::once(c.nome)
                .chain(c.apelidos.iter().copied())
                .map(move |n| (n, c.nome))
        })
        .map(|(n, nome)| (distancia(digitado, n), nome))
        // Limite pelo tamanho: em palavra curta, 2 trocas ja levam a qualquer comando
        // ("xyz" virava "voz").
        .filter(|(d, _)| *d <= (digitado.chars().count() / 3).max(1))
        .min_by_key(|(d, _)| *d)
        .map(|(_, nome)| nome)
}

// A mesma distancia da sugestao de chave do config.json: uma so na base.
use phxclaw_config_runtime::distancia;

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeSet;

    /// Os nomes que o `match` do `main.rs` despacha, lidos do proprio fonte.
    fn despachados() -> Vec<String> {
        let fonte = include_str!("main.rs");
        let inicio = fonte
            .find("match args[0].as_str()")
            .expect("despacho no main.rs");
        let corpo = &fonte[inicio..];
        let fim = corpo.find("other =>").expect("braco final do despacho");
        let mut nomes = Vec::new();
        for linha in corpo[..fim].lines() {
            let l = linha.trim();
            if !l.starts_with('"') || !l.contains("=>") {
                continue;
            }
            let antes = &l[..l.find("=>").unwrap()];
            for parte in antes.split('|') {
                let p = parte.trim().trim_matches('"');
                if !p.is_empty() {
                    nomes.push(p.to_string());
                }
            }
        }
        nomes
    }

    #[test]
    fn todo_comando_do_despacho_esta_na_ajuda() {
        let d = despachados();
        assert!(d.len() > 10, "leitura do despacho falhou: {d:?}");
        let faltando: Vec<_> = d.iter().filter(|n| achar(n).is_none()).collect();
        assert!(
            faltando.is_empty(),
            "despachados sem linha em ajuda.rs: {faltando:?}"
        );
        let sem_despacho: Vec<_> = COMANDOS
            .iter()
            .flat_map(|c| std::iter::once(c.nome).chain(c.apelidos.iter().copied()))
            .filter(|n| !d.iter().any(|x| x == n))
            .collect();
        assert!(
            sem_despacho.is_empty(),
            "na ajuda sem despacho: {sem_despacho:?}"
        );
    }

    /// O fonte de uma funcao chamada pelo despacho: `modulo::nome` do app, `nome` do
    /// `main.rs`, ou `phxclaw_agent::modulo::nome` do agente.
    fn fonte_de(caminho: &str) -> Option<(String, String)> {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let partes: Vec<&str> = caminho.split("::").collect();
        let (arq, nome) = match partes.as_slice() {
            [n] => (raiz.join("src/main.rs"), *n),
            ["phxclaw_agent", m, n] => (
                raiz.join(format!("../../crates/phxclaw-agent/src/{m}.rs")),
                *n,
            ),
            [m, n] => (raiz.join(format!("src/{m}.rs")), *n),
            _ => return None,
        };
        let t = std::fs::read_to_string(arq).ok()?;
        Some((t, nome.to_string()))
    }

    /// O corpo de `fn nome(` em `t`.
    fn corpo<'a>(t: &'a str, nome: &str) -> Option<&'a str> {
        let i = [format!("fn {nome}("), format!("fn {nome}<")]
            .iter()
            .find_map(|a| t.find(a.as_str()))?;
        let ab = i + t[i..].find('{')?;
        let mut nivel = 0;
        for (j, ch) in t[ab..].char_indices() {
            match ch {
                '{' => nivel += 1,
                '}' => {
                    nivel -= 1;
                    if nivel == 0 {
                        return Some(&t[ab..ab + j]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Os grupos `"--a" | "--b"` de um texto (apelidos da mesma opcao, como `--papel` e
    /// `--role`): basta um do grupo no uso.
    fn apelidos_de_opcao(t: &str) -> Vec<Vec<String>> {
        let mut grupos = Vec::new();
        for l in t.lines() {
            let l = l.trim();
            if l.starts_with("\"--") && l.contains("\" | \"--") {
                let antes = l.split("=>").next().unwrap_or(l);
                grupos.push(
                    antes
                        .split('|')
                        .map(|p| p.trim().trim_matches('"').to_string())
                        .filter(|p| p.starts_with("--"))
                        .collect(),
                );
            }
        }
        grupos
    }

    /// Os `"--opcao"` literais de um texto.
    fn opcoes_em(t: &str, saida: &mut BTreeSet<String>) {
        let mut resto = t;
        while let Some(i) = resto.find("\"--") {
            let depois = &resto[i + 1..];
            let fim = depois
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .unwrap_or(depois.len());
            if depois[fim..].starts_with('"') && fim > 2 {
                saida.insert(depois[..fim].to_string());
            }
            resto = &depois[fim..];
        }
    }

    /// As chamadas `caminho(...args...)` de um texto: as funcoes que recebem os argumentos.
    fn chamadas_com_args(t: &str) -> Vec<String> {
        let mut v = Vec::new();
        for (i, _) in t.match_indices('(') {
            let antes = &t[..i];
            let ini = antes
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
                .map_or(0, |k| k + 1);
            let caminho = &antes[ini..];
            let dentro = &t[i + 1..];
            let fim = dentro.find(')').unwrap_or(dentro.len());
            if !caminho.is_empty()
                && caminho
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase())
                && dentro[..fim].contains("args")
            {
                v.push(caminho.to_string());
            }
        }
        v
    }

    /// Cada braco do despacho: os nomes e as opcoes que o codigo dele le (o braco, a funcao
    /// que ele chama e, um nivel abaixo, as funcoes que ela chama com os argumentos).
    /// Nomes do braco, opcoes lidas e grupos de apelidos de opcao.
    type Braco = (Vec<String>, BTreeSet<String>, Vec<Vec<String>>);

    fn opcoes_por_comando() -> Vec<Braco> {
        let fonte = include_str!("main.rs");
        let inicio = fonte.find("match args[0].as_str()").unwrap();
        let corpo_do_match = &fonte[inicio..];
        let fim = corpo_do_match.find("other =>").unwrap();
        let mut bracos: Vec<(Vec<String>, String)> = Vec::new();
        for linha in corpo_do_match[..fim].lines().skip(1) {
            let l = linha.trim();
            if l.starts_with('"') && l.contains("=>") {
                let antes = &l[..l.find("=>").unwrap()];
                let nomes = antes
                    .split('|')
                    .map(|p| p.trim().trim_matches('"').to_string())
                    .collect();
                bracos.push((nomes, String::new()));
            }
            if let Some(b) = bracos.last_mut() {
                b.1.push_str(linha);
                b.1.push('\n');
            }
        }
        bracos
            .into_iter()
            .map(|(nomes, texto)| {
                let mut ops = BTreeSet::new();
                let mut grupos = apelidos_de_opcao(&texto);
                opcoes_em(&texto, &mut ops);
                let mut vistos = BTreeSet::new();
                let mut fila: Vec<(String, usize)> = chamadas_com_args(&texto)
                    .into_iter()
                    .map(|c| (c, 0))
                    .collect();
                while let Some((c, nivel)) = fila.pop() {
                    if !vistos.insert(c.clone()) {
                        continue;
                    }
                    let Some((t, nome)) = fonte_de(&c) else {
                        continue;
                    };
                    let Some(b) = corpo(&t, &nome) else { continue };
                    opcoes_em(b, &mut ops);
                    grupos.extend(apelidos_de_opcao(b));
                    if nivel < 2 {
                        // As do mesmo arquivo, pelo nome qualificado do arquivo de origem.
                        let prefixo = c.rsplit_once("::").map(|(m, _)| m.to_string());
                        for d in chamadas_com_args(b) {
                            let q = match (&prefixo, d.contains("::")) {
                                (Some(m), false) => format!("{m}::{d}"),
                                _ => d,
                            };
                            fila.push((q, nivel + 1));
                        }
                    }
                }
                (nomes, ops, grupos)
            })
            .collect()
    }

    /// A catraca do lado dos comandos: toda opcao que o codigo de um comando le aparece no
    /// uso dele, que e o que `ajuda` mostra. Opcao nova sem linha de ajuda reprova.
    #[test]
    fn toda_opcao_lida_no_codigo_esta_no_uso() {
        let por = opcoes_por_comando();
        assert!(por.len() > 30, "leitura do despacho falhou: {}", por.len());
        let total: usize = por.iter().map(|(_, o, _)| o.len()).sum();
        assert!(total > 40, "leitura das opcoes falhou: {total}");
        let mut faltam = Vec::new();
        for (nomes, ops, grupos) in &por {
            let Some(c) = achar(&nomes[0]) else { continue };
            let no_uso = |o: &str| {
                c.uso
                    .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
                    .any(|p| p == o)
            };
            for o in ops {
                if c.apelidos.contains(&o.as_str()) {
                    continue;
                }
                let por_apelido = grupos
                    .iter()
                    .any(|g| g.contains(o) && g.iter().any(|x| no_uso(x)));
                if !no_uso(o) && !por_apelido {
                    faltam.push(format!("{} {o}", c.nome));
                }
            }
        }
        assert!(
            faltam.is_empty(),
            "opcao lida no codigo e ausente do uso em ajuda.rs: {faltam:?}"
        );
    }

    /// Os segredos do catalogo do config.json citam o comando que os guarda: esse comando
    /// tem de existir na CLI, senao a recusa manda o operador rodar o que nao ha.
    #[test]
    fn todo_segredo_do_catalogo_cita_comando_que_existe() {
        use phxclaw_config_runtime::agente::catalogo::{Natureza, catalogo};
        let mut sem = Vec::new();
        let mut com = 0;
        for k in catalogo() {
            if let Natureza::Segredo { comando, .. } = &k.natureza
                && let Some(i) = comando.find("phxclaw ")
            {
                com += 1;
                let nome = comando[i + 8..].split_whitespace().next().unwrap_or("");
                if achar(nome).is_none() {
                    sem.push(format!("{}: {comando}", k.chave));
                }
            }
        }
        assert!(com > 10, "leitura do catalogo falhou: {com}");
        assert!(
            sem.is_empty(),
            "segredo citando comando inexistente: {sem:?}"
        );
    }

    /// A precedencia da ajuda e a que o motor aplica (uma constante, um texto).
    #[test]
    fn a_ajuda_do_config_cita_a_precedencia_do_motor() {
        let t = de(achar("config").unwrap(), "phxclaw");
        let frase = phxclaw_config_runtime::agente::carga::precedencia_texto();
        assert!(t.contains(&frase), "{t}");
        assert!(!t.contains("{PRECEDENCIA}"));
    }

    #[test]
    fn nomes_e_apelidos_nao_se_repetem() {
        let mut todos: Vec<&str> = COMANDOS
            .iter()
            .flat_map(|c| std::iter::once(c.nome).chain(c.apelidos.iter().copied()))
            .collect();
        let n = todos.len();
        todos.sort_unstable();
        todos.dedup();
        assert_eq!(todos.len(), n, "nome ou apelido repetido");
    }

    #[test]
    fn todo_grupo_aparece_e_toda_linha_cabe() {
        let t = texto("PhxClaw", "0", "phxclaw");
        for g in Grupo::TODOS {
            assert!(t.contains(g.titulo()), "{}", g.titulo());
        }
        for c in COMANDOS {
            assert!(t.contains(c.nome));
            assert!(
                c.uso.starts_with(c.nome),
                "uso de {} nao comeca pelo nome",
                c.nome
            );
        }
        let longa = t
            .lines()
            .filter(|l| l.chars().count() > 110)
            .collect::<Vec<_>>();
        assert!(longa.is_empty(), "linhas longas: {longa:?}");
    }

    #[test]
    fn sugere_o_comando_mais_proximo() {
        assert_eq!(sugestao("servi"), Some("servir"));
        assert_eq!(sugestao("revisr"), Some("revisar"));
        assert_eq!(sugestao("xyzxyz"), None);
        assert_eq!(sugestao("xyz"), None);
    }
}

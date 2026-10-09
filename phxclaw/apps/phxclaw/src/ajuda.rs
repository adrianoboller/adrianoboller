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
        uso: "sessoes \"termo\" [--limite N]",
        descricao: "Busca nas tarefas anteriores (a mesma busca de session_search).",
    },
    Comando {
        grupo: Grupo::Agente,
        nome: "resumo",
        resumo: "Resumo das tarefas do dia",
        apelidos: &["summary"],
        uso: "resumo [--data AAAA-MM-DD|hoje|ontem]",
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
        uso: "skills importar DIR [--com-scripts] [--pasta DIR]",
        descricao: "Le todo SKILL.md abaixo de DIR (Claude Code, Codex, OpenClaw, Hermes), traduz \
                    os nomes de ferramenta conhecidos e grava na pasta de skills. scripts/ nao se \
                    copia sem --com-scripts; ORIGEM.json guarda a origem e o SHA-256.",
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
        uso: "equipe [listar [--macroarea X] [--texto Y] | mostrar ID | delegar ID \"tarefa\" [--modelo M]]",
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
        uso: "fluxo rodar ARQ.json [--ate PASSO] | retomar TAREFA [ARQ.json] | responder TAREFA \
              TEXTO | esperas | pinar ARQ.json PASSO (--json V | --tarefa T) | despinar ARQ.json \
              PASSO | podar [--dias N] [--max N] | exportar ARQ.json [--saida P] | importar \
              PACOTE.json DESTINO.json | listar [DIR] [--etiqueta E] [--subpasta P] [--modelo M] \
              [--pasta DIR]",
        descricao: "Fluxo declarativo em DAG; cada passo e tarefa, ferramenta, skill, mcp, comando \
                    ou no de controle, pelo mesmo portao. --ate para no passo (inclusive) e grava; \
                    retomar continua dali e pula os passos que deram certo (sem ARQ, pela \
                    definicao que a espera gravou). O passo esperar descarrega o fluxo para o \
                    disco: responder entrega a resposta, esperas retoma as de tempo vencidas. \
                    pinar troca a execucao de um passo pelo dado do ARQ.pins.json; podar segue \
                    fluxos.poda_dias/poda_max (sem eles, nada se apaga).",
    },
    Comando {
        grupo: Grupo::EquipeEFluxos,
        nome: "agenda",
        resumo: "Agenda: listar, adicionar (modelo ou fluxo) e disparar o que venceu",
        apelidos: &["schedule"],
        uso: "agenda listar | adicionar NOME \"OBJETIVO\" (--cada SEG | --cron EXPR) | disparar [--pasta DIR]",
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
    // --- servicos ---
    Comando {
        grupo: Grupo::Servicos,
        nome: "servir",
        resumo: "API de tarefas, UI web (PWA), gatilhos e heartbeat",
        apelidos: &["serve"],
        uso: "servir [--porta 8787] [--pasta DIR] [--canal NOME] [--dispositivos --cert C --chave K \
              --tokens F [--porta-dispositivos 8788]] [--ponte wss://H:P/ [--ponte-ca PEM] \
              [--ponte-tenant U] [--sem-porta]]",
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
        resumo: "Guarda o Bearer da API de tarefas",
        apelidos: &[],
        uso: "api chave [--pasta DIR]",
        descricao: "Le PHXCLAW_API_TOKEN (24+ caracteres) do AMBIENTE do comando e guarda no \
                    broker de <pasta>/api. O servir le do ambiente, senao do broker, senao de \
                    <pasta>/api.token (gerado na primeira vez).",
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
        resumo: "Compara modelos pelo agente: p50/p95, tokens/s, CPU, energia, acerto e nota",
        apelidos: &["eval"],
        uso: "avaliar --modelos A,B --tarefas DIR [--rodadas N] [--saida DIR] [--pasta DIR]",
        descricao: "Roda cada caso de DIR (*.json com gabarito, ou *.jsonl gravado) N vezes por \
                    modelo, pelo agente inteiro. Cada numero sai com faixa min-max, N e data; \
                    vencedor so quando as faixas nao se cruzam. Alem do acerto, a nota parcial \
                    de ferramentas (conjunto e sequencia por LCS, 0 a 1, deterministica, nunca \
                    por juiz) e o agrupamento pelo sha256 do prompt e das skills de cada \
                    execucao. Energia so de RAPL ou NVIDIA: sem eles, «não medida». Cada \
                    execucao fica gravada em SAIDA/gravacoes.",
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
    // --- diagnostico ---
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
        "{produto} {versao}\n\nUSO:\n  {cli} <COMANDO> [opcoes]\n  {cli} ajuda <COMANDO>\n"
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

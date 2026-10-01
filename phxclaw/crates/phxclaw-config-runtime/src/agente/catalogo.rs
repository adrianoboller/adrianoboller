//! O catalogo unico das chaves do agente: o nome no `config.json` (em secoes), a variavel
//! de ambiente equivalente, o tipo, o padrao, a descricao e a natureza.
//!
//! Por que uma tabela em codigo, e nao um JSON digitado: o schema, o exemplo e o catalogo
//! da tela SAEM daqui (`gerar`), e o levantamento do fonte (`tools/config_catalogo.py`)
//! reprova variavel `PHXCLAW_*` que apareca no fonte sem linha aqui -- e linha aqui que
//! ninguem mais le. Configuracao que nao e lida mente; variavel lida que nao esta no
//! catalogo e configuracao que a tela nao mostra.
//!
//! Tres naturezas, porque nem toda variavel pode morar no arquivo:
//! - `Config`: vale no `config.json` e no ambiente (o ambiente ganha).
//! - `Segredo`: SO no SecretBroker (ou, enquanto nao ha comando, so no ambiente). O
//!   arquivo que trouxer a chave e recusado, dizendo o comando que guarda.
//! - `Ambiente`: so faz sentido como variavel -- localiza a propria configuracao (a pasta,
//!   o projeto), e exportada pelo agente a um processo filho, ou e chave de prova manual.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Texto,
    Inteiro,
    Real,
    Booleano,
    /// Lista de textos; no ambiente, separada pelo caractere dado (`,` quase sempre; `:`
    /// nas listas de pastas, como o `PATH`).
    Lista(char),
    Caminho,
    Enum(&'static [&'static str]),
}

impl Tipo {
    /// O nome do contrato da tela (`texto|inteiro|real|booleano|lista|caminho|enum`).
    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Texto => "texto",
            Tipo::Inteiro => "inteiro",
            Tipo::Real => "real",
            Tipo::Booleano => "booleano",
            Tipo::Lista(_) => "lista",
            Tipo::Caminho => "caminho",
            Tipo::Enum(_) => "enum",
        }
    }
}

/// Onde o segredo mora no broker: `<pasta do agente>/<subpasta>/segredos`, espaco e nome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Referencia {
    pub subpasta: &'static str,
    pub espaco: &'static str,
    pub nome: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Natureza {
    Config,
    Segredo {
        /// O que o operador faz para guardar (citado na recusa e na tela).
        comando: String,
        /// `None`: ainda nao ha broker para ele; so o ambiente o traz.
        referencia: Option<Referencia>,
    },
    Ambiente {
        motivo: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct Chave {
    pub chave: String,
    pub variavel: String,
    pub tipo: Tipo,
    /// Padrao na forma do ambiente (o texto que a variavel teria).
    pub padrao: Option<&'static str>,
    pub descricao: String,
    pub natureza: Natureza,
}

impl Chave {
    pub fn secao(&self) -> &str {
        self.chave.split('.').next().unwrap_or(&self.chave)
    }
    pub fn segredo(&self) -> bool {
        matches!(self.natureza, Natureza::Segredo { .. })
    }
    pub fn caminho(&self) -> bool {
        self.tipo == Tipo::Caminho
    }
    pub fn lista(&self) -> bool {
        matches!(self.tipo, Tipo::Lista(_))
    }
    /// Pode morar no `config.json`.
    pub fn no_arquivo(&self) -> bool {
        self.natureza == Natureza::Config
    }
}

// --- a tabela fixa -------------------------------------------------------------------

enum N {
    C,
    /// Segredo: comando, e (subpasta, espaco, nome) no broker.
    S(
        &'static str,
        Option<(&'static str, &'static str, &'static str)>,
    ),
    A(&'static str),
}

struct L {
    k: &'static str,
    v: &'static str,
    t: Tipo,
    p: Option<&'static str>,
    d: &'static str,
    n: N,
}

const fn c(
    k: &'static str,
    v: &'static str,
    t: Tipo,
    p: Option<&'static str>,
    d: &'static str,
) -> L {
    L {
        k,
        v,
        t,
        p,
        d,
        n: N::C,
    }
}
const fn s(
    k: &'static str,
    v: &'static str,
    d: &'static str,
    comando: &'static str,
    r: Option<(&'static str, &'static str, &'static str)>,
) -> L {
    L {
        k,
        v,
        t: Tipo::Texto,
        p: None,
        d,
        n: N::S(comando, r),
    }
}
const fn a(k: &'static str, v: &'static str, t: Tipo, d: &'static str, motivo: &'static str) -> L {
    L {
        k,
        v,
        t,
        p: None,
        d,
        n: N::A(motivo),
    }
}

use Tipo::{Booleano as B, Caminho as P, Inteiro as I, Real as R, Texto as T};
const V: Tipo = Tipo::Lista(',');

const LOCALIZA: &str =
    "localiza a propria configuracao: o arquivo nao pode dizer onde ele mesmo mora";
const EXPORTADA: &str = "exportada pelo agente ao processo filho, nao lida como configuracao";
const PROVA: &str = "chave de prova manual (teste ignorado por padrao), nao configuracao do agente";
const ACAO: &str = "entrada da Action do GitHub (passo do fluxo), nao configuracao do agente";

const FIXAS: &[L] = &[
    // --- agente e modelo ---
    c(
        "modelo.padrao",
        "PHXCLAW_MODELO",
        T,
        Some("ollama:qwen2.5:1.5b"),
        "Modelo padrao do agente e da API (provedor:modelo)",
    ),
    c(
        "modelo.local",
        "PHXCLAW_MODELO_LOCAL",
        T,
        None,
        "Modelo local dos papeis da equipe que a planilha roteia para o Ollama",
    ),
    c(
        "modelo.visao",
        "PHXCLAW_MODELO_VISAO",
        T,
        Some("qwen2.5vl:3b"),
        "Modelo de visao da leitura de tela",
    ),
    c(
        "agente.estilo",
        "PHXCLAW_ESTILO",
        T,
        None,
        "Estilo de saida padrao (embutido ou .phxclaw/estilos/<nome>.md)",
    ),
    c(
        "agente.capacidades",
        "PHXCLAW_CAPACIDADES",
        V,
        None,
        "Capacidades liberadas (web.search, fs.read...); vazio = CAPACIDADES_PADRAO",
    ),
    c(
        "agente.memoria_escopo",
        "PHXCLAW_MEMORIA_ESCOPO",
        T,
        Some("padrao"),
        "Escopo da memoria persistente (<pasta>/tasks/_memoria/<escopo>.json)",
    ),
    c(
        "agente.skills_dir",
        "PHXCLAW_SKILLS_DIR",
        P,
        None,
        "Pasta das skills; vazio = <pasta>/tasks/_skills",
    ),
    c(
        "agente.agentes_dir",
        "PHXCLAW_AGENTES_DIR",
        P,
        None,
        "Pasta dos manifestos dos papeis da equipe (config/agents)",
    ),
    c(
        "agente.lsp",
        "PHXCLAW_LSP",
        B,
        Some("true"),
        "Servidores de linguagem (LSP) nas ferramentas de codigo; 0 desliga",
    ),
    c(
        "agente.heartbeat_min",
        "PHXCLAW_HEARTBEAT_MIN",
        I,
        Some("30"),
        "Intervalo do heartbeat em minutos; 0 desliga",
    ),
    c(
        "agente.heartbeat_arquivo",
        "PHXCLAW_HEARTBEAT",
        P,
        None,
        "Arquivo do heartbeat; vazio = HEARTBEAT.md do projeto ou da pasta",
    ),
    a(
        "agente.pasta",
        "PHXCLAW_HOME",
        P,
        "Pasta do agente (padrao var/agente); o config.json da pasta mora nela",
        LOCALIZA,
    ),
    a(
        "agente.projeto",
        "PHXCLAW_PROJETO",
        P,
        "Raiz do projeto (padrao a pasta corrente); o .phxclaw/config.json mora nela",
        LOCALIZA,
    ),
    // --- API, ponte e dispositivos ---
    c(
        "api.host",
        "PHXCLAW_API_HOST",
        T,
        Some("127.0.0.1"),
        "Endereco em que a API escuta; expor e decisao do operador",
    ),
    c(
        "api.porta",
        "PHXCLAW_API_PORT",
        I,
        None,
        "Porta da API do app de mesa",
    ),
    s(
        "api.token",
        "PHXCLAW_API_TOKEN",
        "Bearer da API (24+ caracteres)",
        "arquivo <pasta>/api.token (o servir gera) ou a variavel PHXCLAW_API_TOKEN",
        None,
    ),
    c(
        "api.tarefas_por_minuto",
        "PHXCLAW_API_TAREFAS_POR_MINUTO",
        I,
        Some("10"),
        "Teto de criacao de tarefas pela API (balde de fichas)",
    ),
    c(
        "api.webhook_origens",
        "PHXCLAW_WEBHOOK_ORIGINS",
        V,
        None,
        "Origens para onde o webhook de fim de tarefa pode ir; vazio = recusado",
    ),
    c(
        "api.url_publica",
        "PHXCLAW_PUBLIC_URL",
        T,
        Some("http://127.0.0.1:8787"),
        "URL publica do agente (links de site e de artefato)",
    ),
    c(
        "api.url_cliente",
        "PHXCLAW_URL",
        T,
        Some("http://127.0.0.1:8787"),
        "Servidor que o SDK e o exemplo do SDK chamam",
    ),
    c(
        "ponte.host",
        "PHXCLAW_PONTE_HOST",
        T,
        Some("127.0.0.1"),
        "Endereco em que a ponte de controle remoto escuta",
    ),
    s(
        "ponte.token",
        "PHXCLAW_PONTE_TOKEN",
        "Bearer da ponte (24+ caracteres)",
        "arquivo <pasta>/ponte.token ou a variavel PHXCLAW_PONTE_TOKEN",
        None,
    ),
    c(
        "dispositivos.host",
        "PHXCLAW_DEVICE_HOST",
        T,
        Some("127.0.0.1"),
        "Endereco do servidor de dispositivos",
    ),
    c(
        "dispositivos.tenant_uuid",
        "PHXCLAW_TENANT_UUID",
        T,
        None,
        "Inquilino (UUID) do no de dispositivo e da ponte",
    ),
    c(
        "dispositivos.no_uuid",
        "PHXCLAW_NODE_UUID",
        T,
        None,
        "UUID do no de dispositivo",
    ),
    c(
        "dispositivos.wss_url",
        "PHXCLAW_DEVICE_WSS_URL",
        T,
        None,
        "Endereco WSS do servidor para o no de dispositivo",
    ),
    c(
        "dispositivos.keystore",
        "PHXCLAW_DEVICE_KEYSTORE",
        P,
        None,
        "Arquivo da chave do no de dispositivo",
    ),
    c(
        "dispositivos.ca_pem",
        "PHXCLAW_DEVICE_CA_PEM",
        P,
        None,
        "Autoridade (PEM) que o no aceita no TLS",
    ),
    s(
        "dispositivos.token_pareamento",
        "PHXCLAW_ENROLLMENT_TOKEN",
        "Token de pareamento do no",
        "variavel PHXCLAW_ENROLLMENT_TOKEN no arranque do pareamento",
        None,
    ),
    // --- voz ---
    c(
        "voz.tts.provedor",
        "PHXCLAW_TTS_PROVEDOR",
        Tipo::Enum(&["comando", "elevenlabs"]),
        Some("comando"),
        "Quem fala: o comando local ou a ElevenLabs",
    ),
    c(
        "voz.tts.comando",
        "PHXCLAW_TTS_COMMAND",
        T,
        None,
        "Comando de fala local (piper ou equivalente)",
    ),
    c(
        "voz.tts.modelo",
        "PHXCLAW_TTS_MODEL",
        P,
        None,
        "Modelo do comando de fala",
    ),
    c(
        "voz.tts.modelo_sha256",
        "PHXCLAW_TTS_MODEL_SHA256",
        T,
        None,
        "SHA-256 esperado do modelo de fala",
    ),
    c(
        "voz.tts.pastas",
        "PHXCLAW_TTS_DIRS",
        Tipo::Lista(':'),
        None,
        "Pastas em que a fala pode gravar (separadas por : no ambiente)",
    ),
    c(
        "voz.stt.provedor",
        "PHXCLAW_STT_PROVEDOR",
        Tipo::Enum(&["whisper", "elevenlabs"]),
        Some("whisper"),
        "Quem transcreve: o whisper.cpp local ou a ElevenLabs",
    ),
    c(
        "voz.whisper.bin",
        "PHXCLAW_WHISPER_BIN",
        P,
        None,
        "Executavel do whisper.cpp",
    ),
    c(
        "voz.whisper.modelo",
        "PHXCLAW_WHISPER_MODEL",
        P,
        None,
        "Modelo do whisper.cpp",
    ),
    c(
        "voz.whisper.modelo_sha256",
        "PHXCLAW_WHISPER_MODEL_SHA256",
        T,
        None,
        "SHA-256 esperado do modelo do whisper",
    ),
    c(
        "voz.kws.bin",
        "PHXCLAW_KWS_BIN",
        P,
        None,
        "Executavel da palavra de ativacao",
    ),
    c(
        "voz.kws.modelo_dir",
        "PHXCLAW_KWS_MODEL_DIR",
        P,
        None,
        "Pasta do modelo da palavra de ativacao",
    ),
    c(
        "voz.kws.modelo_sha256",
        "PHXCLAW_KWS_MODEL_SHA256",
        T,
        None,
        "SHA-256 esperado do modelo da palavra de ativacao",
    ),
    c(
        "elevenlabs.api",
        "PHXCLAW_ELEVENLABS_API",
        T,
        Some("https://api.elevenlabs.io"),
        "Base da API da ElevenLabs",
    ),
    c(
        "elevenlabs.voz",
        "PHXCLAW_ELEVENLABS_VOZ",
        T,
        None,
        "voice_id da ElevenLabs (obrigatorio com voz.tts.provedor=elevenlabs)",
    ),
    c(
        "elevenlabs.modelo",
        "PHXCLAW_ELEVENLABS_MODELO",
        T,
        None,
        "Modelo de fala da ElevenLabs",
    ),
    c(
        "elevenlabs.formato",
        "PHXCLAW_ELEVENLABS_FORMATO",
        T,
        Some("wav_16000"),
        "Formato de saida da fala (wav_16000, pcm_22050...)",
    ),
    c(
        "elevenlabs.estabilidade",
        "PHXCLAW_ELEVENLABS_ESTABILIDADE",
        R,
        None,
        "stability da voz (0 a 1)",
    ),
    c(
        "elevenlabs.similaridade",
        "PHXCLAW_ELEVENLABS_SIMILARIDADE",
        R,
        None,
        "similarity_boost da voz (0 a 1)",
    ),
    c(
        "elevenlabs.stt_modelo",
        "PHXCLAW_ELEVENLABS_STT_MODELO",
        T,
        Some("scribe_v2"),
        "Modelo de transcricao da ElevenLabs",
    ),
    s(
        "elevenlabs.chave",
        "PHXCLAW_ELEVENLABS_API_KEY",
        "Chave da ElevenLabs",
        "phxclaw elevenlabs chave",
        Some(("elevenlabs", "elevenlabs", "elevenlabs-chave")),
    ),
    // --- imagem, xAI, Gemini ---
    c(
        "imagem.provedor",
        "PHXCLAW_IMAGEM_PROVEDOR",
        Tipo::Enum(&["openai", "comfyui", "nanobanana"]),
        None,
        "Gerador de imagem; vazio = so o SVG local",
    ),
    c(
        "imagem.url",
        "PHXCLAW_IMAGEM_URL",
        T,
        None,
        "Base do gerador de imagem (openai: https://api.openai.com)",
    ),
    c(
        "imagem.modelo",
        "PHXCLAW_IMAGEM_MODELO",
        T,
        None,
        "Modelo do gerador de imagem (openai: gpt-image-1)",
    ),
    c(
        "imagem.comfy_fluxo",
        "PHXCLAW_COMFY_WORKFLOW",
        P,
        None,
        "Fluxo do ComfyUI (obrigatorio com imagem.provedor=comfyui)",
    ),
    s(
        "imagem.chave",
        "PHXCLAW_IMAGEM_CHAVE",
        "Chave do gerador de imagem openai",
        "variavel PHXCLAW_IMAGEM_CHAVE (ainda sem comando de broker)",
        None,
    ),
    s(
        "gemini.chave",
        "PHXCLAW_GEMINI_API_KEY",
        "Chave do Gemini (nanobanana)",
        "phxclaw gemini chave",
        Some(("gemini", "gemini", "gemini-chave")),
    ),
    c(
        "xai.api",
        "PHXCLAW_XAI_API",
        T,
        Some("https://api.x.ai/v1"),
        "Base da API da xAI",
    ),
    c(
        "xai.modelo",
        "PHXCLAW_XAI_MODELO",
        T,
        Some("grok-4"),
        "Modelo da busca da xAI",
    ),
    s(
        "xai.chave",
        "PHXCLAW_XAI_API_KEY",
        "Chave da xAI",
        "phxclaw xai chave",
        Some(("xai", "xai", "xai-chave")),
    ),
    // --- e-mail (ferramenta send_email) ---
    c(
        "email.smtp.host",
        "PHXCLAW_SMTP_HOST",
        T,
        None,
        "Servidor SMTP",
    ),
    c(
        "email.smtp.porta",
        "PHXCLAW_SMTP_PORT",
        I,
        None,
        "Porta SMTP; vazio = 465 (tls), 587 (starttls) ou 25 (plain)",
    ),
    c(
        "email.smtp.seguranca",
        "PHXCLAW_SMTP_SECURITY",
        Tipo::Enum(&["tls", "starttls", "plain"]),
        Some("tls"),
        "Seguranca do SMTP",
    ),
    c(
        "email.smtp.usuario",
        "PHXCLAW_SMTP_USER",
        T,
        None,
        "Usuario do SMTP",
    ),
    s(
        "email.smtp.senha",
        "PHXCLAW_SMTP_PASSWORD",
        "Senha do SMTP",
        "variavel PHXCLAW_SMTP_PASSWORD (no canal de e-mail vai ao broker como email-smtp_senha)",
        None,
    ),
    c(
        "email.remetente",
        "PHXCLAW_EMAIL_FROM",
        T,
        None,
        "Remetente do e-mail",
    ),
    c(
        "email.permitidos",
        "PHXCLAW_EMAIL_PERMITIDOS",
        V,
        None,
        "Destinatarios permitidos do send_email",
    ),
    // --- forjas e MCP ---
    c(
        "forja.github.api",
        "PHXCLAW_GITHUB_API",
        T,
        Some("https://api.github.com"),
        "Base da API do GitHub",
    ),
    c(
        "forja.gitlab.api",
        "PHXCLAW_GITLAB_API",
        T,
        Some("https://gitlab.com/api/v4"),
        "Base da API do GitLab",
    ),
    s(
        "forja.github.token",
        "PHXCLAW_GITHUB_TOKEN",
        "Token do GitHub",
        "phxclaw forja token github",
        Some(("forja", "forjas", "github-token")),
    ),
    s(
        "forja.gitlab.token",
        "PHXCLAW_GITLAB_TOKEN",
        "Token do GitLab",
        "phxclaw forja token gitlab",
        Some(("forja", "forjas", "gitlab-token")),
    ),
    c(
        "mcp.config",
        "PHXCLAW_MCP_CONFIG",
        P,
        None,
        "Arquivo dos servidores MCP do operador",
    ),
    s(
        "mcp.token",
        "PHXCLAW_MCP_TOKEN",
        "Token de um servidor MCP remoto",
        "phxclaw mcp token NOME",
        None,
    ),
    s(
        "mcp.segredo_cliente",
        "PHXCLAW_MCP_SEGREDO_CLIENTE",
        "Segredo do cliente OAuth de um servidor MCP",
        "phxclaw mcp login NOME",
        None,
    ),
    // --- plugins e pacotes ---
    c(
        "plugins.raiz",
        "PHXCLAW_PLUGINS_RAIZ",
        P,
        None,
        "Raiz dos plugins; vazio = nenhum plugin",
    ),
    c(
        "plugins.dir",
        "PHXCLAW_PLUGINS_DIR",
        P,
        None,
        "Pasta dos manifestos; vazio = <raiz>/plugins",
    ),
    c(
        "plugins.assinantes",
        "PHXCLAW_PLUGIN_SIGNERS",
        P,
        None,
        "Trust store dos assinantes; vazio = <raiz>/config/trust/plugin-signers.json",
    ),
    c(
        "plugins.chave_assinatura_arquivo",
        "PHXCLAW_PLUGIN_SIGNING_KEY_FILE",
        P,
        None,
        "Arquivo da semente de assinatura (exemplo assinar)",
    ),
    s(
        "plugins.chave_assinatura",
        "PHXCLAW_PLUGIN_SIGNING_KEY",
        "Semente de assinatura de plugin (exemplo assinar)",
        "variavel PHXCLAW_PLUGIN_SIGNING_KEY ou o arquivo de plugins.chave_assinatura_arquivo",
        None,
    ),
    c(
        "pacotes.dir",
        "PHXCLAW_PACOTES_DIR",
        P,
        None,
        "Pasta dos pacotes assinados (Claude/Codex)",
    ),
    // --- rede, banco, navegador, ferramentas ---
    c(
        "rede.destinos",
        "PHXCLAW_NET_DESTINOS",
        V,
        None,
        "Destinos host:porta que a sonda de rede pode tocar",
    ),
    c(
        "postgres.url",
        "PHXCLAW_PG_URL",
        T,
        None,
        "PostgreSQL das ferramentas db (sem senha na URL: senha vai pelo .pgpass)",
    ),
    c(
        "postgres.database_url",
        "PHXCLAW_DATABASE_URL",
        T,
        None,
        "PostgreSQL da CLI antiga (phxclaw-cli)",
    ),
    c(
        "navegador.chromium",
        "PHXCLAW_CHROMIUM",
        P,
        None,
        "Executavel do Chromium",
    ),
    c(
        "busca.searxng_url",
        "PHXCLAW_SEARXNG_URL",
        T,
        None,
        "SearXNG da busca na web",
    ),
    c(
        "python.bin",
        "PHXCLAW_PYTHON",
        P,
        None,
        "Interpretador Python das ferramentas",
    ),
    c("python.uv", "PHXCLAW_UV", P, None, "Executavel do uv"),
    c(
        "ui.dir",
        "PHXCLAW_UI_DIR",
        P,
        None,
        "Pasta da interface (PWA)",
    ),
    c(
        "clima.api",
        "PHXCLAW_MET_API",
        T,
        None,
        "Base da API de clima (MET Norway)",
    ),
    c(
        "clima.contato",
        "PHXCLAW_MET_CONTATO",
        T,
        None,
        "Contato no User-Agent pedido pelo MET Norway",
    ),
    c(
        "documentos.embed",
        "PHXCLAW_DOCS_EMBED",
        T,
        None,
        "Modelo de embedding da reordenacao (so ollama:)",
    ),
    c(
        "missao.estado_dir",
        "PHXCLAW_STATE_DIR",
        P,
        None,
        "Pasta de estado da CLI de missao",
    ),
    // --- canal do Telegram (os demais saem de CANAIS) ---
    s(
        "canais.telegram.bot_token",
        "PHXCLAW_TELEGRAM_BOT_TOKEN",
        "Token do bot do Telegram",
        "PHXCLAW_TELEGRAM_BOT_TOKEN=... phxclaw canal telegram (vai ao broker na primeira vez)",
        Some(("canal", "canais", "telegram-bot")),
    ),
    c(
        "canais.telegram.chats",
        "PHXCLAW_TELEGRAM_CHATS",
        V,
        None,
        "Chats que podem falar com o agente (obrigatorio)",
    ),
    // --- app de mesa ---
    c(
        "desktop.exec_host",
        "PHXCLAW_ENABLE_HOST_EXEC",
        B,
        Some("false"),
        "App de mesa: executar comando no host",
    ),
    c(
        "desktop.controle_webview",
        "PHXCLAW_ENABLE_WEBVIEW_CONTROL",
        B,
        Some("false"),
        "App de mesa: controlar webview",
    ),
    c(
        "desktop.shells",
        "PHXCLAW_ENABLE_SHELLS",
        B,
        Some("false"),
        "App de mesa: shells",
    ),
    c(
        "desktop.entrada",
        "PHXCLAW_ENABLE_INPUT",
        B,
        Some("false"),
        "App de mesa: mouse e teclado",
    ),
    c(
        "desktop.captura_tela",
        "PHXCLAW_ENABLE_SCREEN_CAPTURE",
        B,
        Some("false"),
        "App de mesa: captura de tela",
    ),
    c(
        "desktop.webviews_externas",
        "PHXCLAW_ENABLE_EXTERNAL_WEBVIEWS",
        B,
        Some("false"),
        "App de mesa: webviews externas",
    ),
    c(
        "desktop.controle_api_host",
        "PHXCLAW_API_HOST_CONTROL",
        B,
        Some("false"),
        "App de mesa: a API controla o host",
    ),
    c(
        "desktop.webview_origens",
        "PHXCLAW_WEBVIEW_ALLOWED_ORIGINS",
        V,
        None,
        "App de mesa: origens permitidas na webview",
    ),
    c(
        "desktop.hx",
        "PHXCLAW_HX",
        P,
        None,
        "App de mesa: executavel do Helix",
    ),
    // --- pontes para agentes externos ---
    c(
        "pontes.claw.bin",
        "PHXCLAW_CLAW_BIN",
        P,
        Some("claw"),
        "Executavel do claw-code",
    ),
    c(
        "pontes.claw.workspace",
        "PHXCLAW_CLAW_WORKSPACE",
        P,
        Some("."),
        "Pasta de trabalho do claw-code",
    ),
    c(
        "pontes.claw.timeout_ms",
        "PHXCLAW_CLAW_TIMEOUT_MS",
        I,
        Some("120000"),
        "Prazo do claw-code em ms",
    ),
    c(
        "pontes.octopus.bin",
        "PHXCLAW_OCTOPUS_BIN",
        P,
        Some("octopus-console"),
        "Executavel do Octopus",
    ),
    c(
        "pontes.octopus.workspace",
        "PHXCLAW_OCTOPUS_WORKSPACE",
        P,
        Some("."),
        "Pasta de trabalho do Octopus",
    ),
    c(
        "pontes.openclaw_rs.bin",
        "PHXCLAW_OPENCLAW_RS_BIN",
        P,
        None,
        "Executavel do openclaw-rs",
    ),
    c(
        "pontes.openclaw_rs.workspace",
        "PHXCLAW_OPENCLAW_RS_WORKSPACE",
        P,
        Some("."),
        "Pasta de trabalho do openclaw-rs",
    ),
    c(
        "pontes.openclaw_rs.timeout_ms",
        "PHXCLAW_OPENCLAW_RS_TIMEOUT_MS",
        I,
        None,
        "Prazo do openclaw-rs em ms",
    ),
    c(
        "pontes.rustclaw.bin",
        "PHXCLAW_RUSTCLAW_BIN",
        P,
        Some("rustclaw"),
        "Executavel do rustclaw",
    ),
    c(
        "pontes.rustclaw.workspace",
        "PHXCLAW_RUSTCLAW_WORKSPACE",
        P,
        Some("."),
        "Pasta de trabalho do rustclaw",
    ),
    c(
        "pontes.rustclaw.timeout_ms",
        "PHXCLAW_RUSTCLAW_TIMEOUT_MS",
        I,
        Some("60000"),
        "Prazo do rustclaw em ms",
    ),
    c(
        "pontes.rustclaw.prompt_no_argv",
        "PHXCLAW_RUSTCLAW_ALLOW_PROMPT_ARGV",
        B,
        Some("false"),
        "Permite o prompt na linha de comando do rustclaw (fica visivel no ps)",
    ),
    // --- exportadas ao processo filho ---
    a(
        "exportadas.hook_entrada",
        "PHXCLAW_HOOK_INPUT",
        T,
        "Entrada do hook (JSON do evento)",
        EXPORTADA,
    ),
    a(
        "exportadas.hook_evento",
        "PHXCLAW_HOOK_EVENT",
        T,
        "Nome do evento do hook",
        EXPORTADA,
    ),
    a(
        "exportadas.hook_projeto",
        "PHXCLAW_PROJECT_DIR",
        P,
        "Pasta do projeto vista pelo hook",
        EXPORTADA,
    ),
    a(
        "exportadas.plugin_args",
        "PHXCLAW_ARGS",
        T,
        "Argumentos da chamada ao plugin",
        EXPORTADA,
    ),
    a(
        "exportadas.plugin",
        "PHXCLAW_PLUGIN",
        T,
        "Nome do plugin chamado",
        EXPORTADA,
    ),
    a(
        "exportadas.repl_driver",
        "PHXCLAW_REPL_DRIVER",
        T,
        "Driver do REPL Python",
        EXPORTADA,
    ),
    a(
        "exportadas.repl_marca",
        "PHXCLAW_REPL_MARCA",
        T,
        "Marca de fim de saida do REPL",
        EXPORTADA,
    ),
    a(
        "exportadas.pytest_json",
        "PHXCLAW_PYTEST_JSON",
        P,
        "Arquivo do relatorio do pytest",
        EXPORTADA,
    ),
    // --- Action do GitHub ---
    a(
        "acao.bin",
        "PHXCLAW_BIN",
        P,
        "Binario do phxclaw exportado entre passos",
        ACAO,
    ),
    a(
        "acao.bin_entrada",
        "PHXCLAW_BIN_ENTRADA",
        P,
        "Binario do phxclaw dado a Action",
        ACAO,
    ),
    a("acao.foco", "PHXCLAW_FOCO", T, "Foco da revisao", ACAO),
    a(
        "acao.falhar_em",
        "PHXCLAW_FALHAR_EM",
        T,
        "Severidade que reprova a revisao",
        ACAO,
    ),
    // --- provas manuais ---
    a(
        "testes.ollama_url",
        "PHXCLAW_E2E_OLLAMA_URL",
        T,
        "Ollama da prova real",
        PROVA,
    ),
    a(
        "testes.ollama_modelo",
        "PHXCLAW_E2E_OLLAMA_MODEL",
        T,
        "Modelo da prova real do Ollama",
        PROVA,
    ),
    a(
        "testes.ollama_modelo_embed",
        "PHXCLAW_E2E_OLLAMA_EMBED_MODEL",
        T,
        "Modelo de embedding da prova real",
        PROVA,
    ),
    a(
        "testes.rls_url",
        "PHXCLAW_E2E_RLS_URL",
        T,
        "PostgreSQL da prova de RLS",
        PROVA,
    ),
    a(
        "testes.whisper_bin",
        "PHXCLAW_E2E_WHISPER_BIN",
        P,
        "whisper.cpp da prova real",
        PROVA,
    ),
    a(
        "testes.whisper_modelo",
        "PHXCLAW_E2E_WHISPER_MODEL",
        P,
        "Modelo do whisper da prova real",
        PROVA,
    ),
    a(
        "testes.whisper_modelo_sha256",
        "PHXCLAW_E2E_WHISPER_MODEL_SHA256",
        T,
        "SHA-256 do modelo da prova real",
        PROVA,
    ),
    a(
        "testes.whisper_audio",
        "PHXCLAW_E2E_WHISPER_AUDIO",
        P,
        "Audio da prova real do whisper",
        PROVA,
    ),
    a(
        "testes.caos_segundos",
        "PHXCLAW_CHAOS_SECONDS",
        I,
        "Duracao da prova de caos no PostgreSQL",
        PROVA,
    ),
    a(
        "testes.prova_visao",
        "PHXCLAW_PROVA_VISAO",
        T,
        "Liga a prova real de visao",
        PROVA,
    ),
    a(
        "testes.prova_visao_png",
        "PHXCLAW_PROVA_VISAO_PNG",
        P,
        "Imagem da prova real de visao",
        PROVA,
    ),
    a(
        "testes.prova_visao_texto",
        "PHXCLAW_PROVA_VISAO_TEXTO",
        T,
        "Texto esperado na prova real de visao",
        PROVA,
    ),
    a(
        "testes.office_exigir_prova",
        "PHXCLAW_OFFICE_EXIGIR_PROVA",
        T,
        "Exige a prova real do office",
        PROVA,
    ),
    a(
        "testes.corpus_skills",
        "PHXCLAW_CORPUS_SKILLS",
        P,
        "Corpus de skills da prova",
        PROVA,
    ),
    a(
        "testes.prova_docs",
        "PHXCLAW_PROVA_DOCS",
        P,
        "Pasta da medicao real de documentos",
        PROVA,
    ),
    a(
        "testes.prova_docs_gabarito",
        "PHXCLAW_PROVA_DOCS_GABARITO",
        P,
        "Gabarito da medicao real de documentos",
        PROVA,
    ),
    a(
        "testes.flutter",
        "PHXCLAW_FLUTTER",
        P,
        "Flutter da prova da UI",
        PROVA,
    ),
    a(
        "testes.telegram_chat_id",
        "PHXCLAW_TELEGRAM_CHAT_ID",
        T,
        "Chat da prova real do Telegram",
        PROVA,
    ),
];

// --- os canais de `phxclaw canal <nome>` (convencao PHXCLAW_<PREFIXO>_<CHAVE>) ---------

#[derive(Clone, Copy)]
enum K {
    /// Texto obrigatorio ou opcional.
    T(&'static str),
    /// Texto com padrao.
    Tp(&'static str, &'static str),
    /// Lista separada por virgula.
    L(&'static str),
    /// Booleano com padrao.
    B(&'static str, &'static str),
    /// Segredo (vai ao broker `<pasta>/canal`, espaco `canais`, nome `<canal>-<chave>`).
    S(&'static str),
    /// A base do servico; `None`: obrigatoria; `Some(None)`: a oficial; `Some(Some(p))`.
    Base(Option<Option<&'static str>>),
    /// TLS (ligado por padrao) e a autoridade (CA) do soquete.
    Tls,
}

struct CanalDef {
    canal: &'static str,
    prefixo: &'static str,
    chaves: &'static [(&'static str, K)],
}

/// Os canais que montam pelo `canais::ligar` (o Telegram, que monta a parte, esta em
/// `FIXAS`). Toda conta de canal tem `PERMITIDOS`, acrescentado na montagem.
const CANAIS: &[CanalDef] = &[
    CanalDef {
        canal: "discord",
        prefixo: "DISCORD",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S("Token do bot")),
        ],
    },
    CanalDef {
        canal: "slack",
        prefixo: "SLACK",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S("Token do bot")),
        ],
    },
    CanalDef {
        canal: "whatsapp",
        prefixo: "WHATSAPP",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("APP_SECRET", K::S("App secret da Meta")),
            ("VERIFY_TOKEN", K::S("Verify token do webhook")),
            ("TOKEN", K::S("Token de acesso")),
            ("PHONE_ID", K::T("phone_number_id")),
        ],
    },
    CanalDef {
        canal: "messenger",
        prefixo: "MESSENGER",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("APP_SECRET", K::S("App secret da Meta")),
            ("VERIFY_TOKEN", K::S("Verify token do webhook")),
            ("TOKEN", K::S("Token da pagina")),
        ],
    },
    CanalDef {
        canal: "teams",
        prefixo: "TEAMS",
        chaves: &[
            ("LOGIN", K::T("Endpoint de login; vazio = o oficial")),
            (
                "SERVICOS",
                K::L("Origens de servico aceitas; vazio = a oficial"),
            ),
            ("CHAVE_URL", K::S("Chave da URL de entrada")),
            ("APP_SECRET", K::S("Segredo do app")),
            ("APP_ID", K::T("ID do app")),
        ],
    },
    CanalDef {
        canal: "matrix",
        prefixo: "MATRIX",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S("Token de acesso")),
            ("USUARIO", K::T("Usuario do bot")),
        ],
    },
    CanalDef {
        canal: "email",
        prefixo: "EMAIL_CANAL",
        chaves: &[
            ("SMTP_SENHA", K::S("Senha do SMTP do canal")),
            ("IMAP", K::T("Servidor IMAP host:porta")),
            ("USUARIO", K::T("Usuario do IMAP")),
            ("SENHA", K::S("Senha do IMAP")),
            ("PASTA", K::Tp("Pasta lida", "INBOX")),
            (
                "EXIGIR_DMARC",
                K::B("Exige DMARC do remetente; nao desliga", "true"),
            ),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "webhook",
        prefixo: "WEBHOOK",
        chaves: &[
            ("SAIDA_URL", K::T("URL de saida")),
            ("SEGREDO", K::S("Segredo da assinatura")),
        ],
    },
    CanalDef {
        canal: "webchat",
        prefixo: "WEBCHAT",
        chaves: &[("CHAVE", K::S("Chave do webchat"))],
    },
    CanalDef {
        canal: "signal",
        prefixo: "SIGNAL",
        chaves: &[
            ("BASE", K::Base(Some(Some("http://127.0.0.1:8080")))),
            ("NUMERO", K::T("Numero do bot")),
            ("TOKEN", K::S("Token do signal-cli-rest-api (opcional)")),
        ],
    },
    CanalDef {
        canal: "googlechat",
        prefixo: "GOOGLECHAT",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("CHAVE_URL", K::S("Chave da URL de entrada")),
            ("SAIDA_WEBHOOK", K::S("Webhook de saida")),
            ("ESPACO", K::T("Espaco do Google Chat")),
        ],
    },
    CanalDef {
        canal: "sms",
        prefixo: "SMS",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S("Token da conta")),
            ("CONTA", K::T("Conta do provedor")),
            ("NUMERO", K::T("Numero de envio")),
            ("URL_PUBLICA", K::T("URL publica do webhook")),
        ],
    },
    CanalDef {
        canal: "mattermost",
        prefixo: "MATTERMOST",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S("Token do bot")),
            ("BOT_ID", K::T("ID do bot")),
        ],
    },
    CanalDef {
        canal: "rocketchat",
        prefixo: "ROCKETCHAT",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S("Token do bot")),
            ("USUARIO_ID", K::T("ID do usuario do bot")),
            ("TIPO", K::Tp("Tipo de sala", "channels")),
        ],
    },
    CanalDef {
        canal: "zulip",
        prefixo: "ZULIP",
        chaves: &[
            ("BASE", K::Base(None)),
            ("CHAVE", K::S("Chave da API")),
            ("EMAIL", K::T("E-mail do bot")),
        ],
    },
    CanalDef {
        canal: "irc",
        prefixo: "IRC",
        chaves: &[
            ("ENDERECO", K::T("Servidor host:porta")),
            ("NICK", K::T("Apelido do bot")),
            ("SENHA", K::S("Senha (opcional)")),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "twitch",
        prefixo: "TWITCH",
        chaves: &[
            ("ENDERECO", K::T("Servidor host:porta")),
            ("NICK", K::T("Apelido do bot")),
            ("SENHA", K::S("Token oauth do chat")),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "xmpp",
        prefixo: "XMPP",
        chaves: &[
            ("ENDERECO", K::T("Servidor host:porta")),
            ("JID", K::T("JID do bot")),
            ("SENHA", K::S("Senha do JID")),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "mastodon",
        prefixo: "MASTODON",
        chaves: &[("BASE", K::Base(None)), ("TOKEN", K::S("Token de acesso"))],
    },
    CanalDef {
        canal: "line",
        prefixo: "LINE",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("SEGREDO_CANAL", K::S("Segredo do canal")),
            ("TOKEN", K::S("Token de acesso")),
        ],
    },
    CanalDef {
        canal: "viber",
        prefixo: "VIBER",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S("Token do bot")),
            ("REMETENTE", K::Tp("Nome do remetente", "PhxClaw")),
        ],
    },
    CanalDef {
        canal: "feishu",
        prefixo: "FEISHU",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("VERIFICACAO", K::S("Token de verificacao")),
            ("APP_SECRET", K::S("Segredo do app")),
            ("APP_ID", K::T("ID do app")),
        ],
    },
    CanalDef {
        canal: "reddit",
        prefixo: "REDDIT",
        chaves: &[
            ("WWW", K::T("Base www; vazio = a oficial")),
            ("OAUTH", K::T("Base oauth; vazio = a oficial")),
            ("APP_SECRET", K::S("Segredo do app")),
            ("SENHA", K::S("Senha da conta")),
            ("APP_ID", K::T("ID do app")),
            ("USUARIO", K::T("Usuario")),
        ],
    },
    CanalDef {
        canal: "nostr",
        prefixo: "NOSTR",
        chaves: &[
            ("RELAY", K::T("Relay wss://")),
            ("CHAVE", K::S("Chave secreta (hex)")),
        ],
    },
];

fn do_canal(def: &CanalDef, out: &mut Vec<Chave>) {
    let var = |k: &str| format!("PHXCLAW_{}_{k}", def.prefixo);
    let chave = |k: &str| format!("canais.{}.{}", def.canal, k.to_ascii_lowercase());
    let cfg = |k: &str, t: Tipo, p: Option<&'static str>, d: String| Chave {
        chave: chave(k),
        variavel: var(k),
        tipo: t,
        padrao: p,
        descricao: d,
        natureza: Natureza::Config,
    };
    out.push(cfg(
        "PERMITIDOS",
        Tipo::Lista(','),
        None,
        format!(
            "Conversas de {} que podem falar com o agente (obrigatoria)",
            def.canal
        ),
    ));
    for (k, tipo) in def.chaves {
        match *tipo {
            K::T(d) => out.push(cfg(k, Tipo::Texto, None, d.into())),
            K::Tp(d, p) => out.push(cfg(k, Tipo::Texto, Some(p), d.into())),
            K::L(d) => out.push(cfg(k, Tipo::Lista(','), None, d.into())),
            K::B(d, p) => out.push(cfg(k, Tipo::Booleano, Some(p), d.into())),
            K::Base(b) => {
                let (p, d) = match b {
                    None => (None, "Base do servico (obrigatoria)".to_string()),
                    Some(None) => (None, "Base do servico; vazio = a oficial".to_string()),
                    Some(Some(p)) => (Some(p), "Base do servico".to_string()),
                };
                out.push(cfg(k, Tipo::Texto, p, d));
            }
            K::Tls => {
                out.push(cfg(
                    "TLS",
                    Tipo::Booleano,
                    Some("true"),
                    "TLS no soquete; desligado so vale para loopback".into(),
                ));
                out.push(cfg(
                    "CA",
                    Tipo::Caminho,
                    None,
                    "Autoridade (PEM) no lugar das raizes publicas".into(),
                ));
            }
            K::S(d) => out.push(Chave {
                chave: chave(k),
                variavel: var(k),
                tipo: Tipo::Texto,
                padrao: None,
                descricao: format!("{d} ({})", def.canal),
                natureza: Natureza::Segredo {
                    comando: format!(
                        "{}=... phxclaw canal {} (vai ao broker na primeira vez)",
                        var(k),
                        def.canal
                    ),
                    referencia: Some(Referencia {
                        subpasta: "canal",
                        espaco: "canais",
                        nome: format!("{}-{}", def.canal, k.to_ascii_lowercase()),
                    }),
                },
            }),
        }
    }
}

fn montar() -> Vec<Chave> {
    let mut v: Vec<Chave> = FIXAS
        .iter()
        .map(|l| Chave {
            chave: l.k.into(),
            variavel: l.v.into(),
            tipo: l.t,
            padrao: l.p,
            descricao: l.d.into(),
            natureza: match l.n {
                N::C => Natureza::Config,
                N::S(comando, r) => Natureza::Segredo {
                    comando: comando.into(),
                    referencia: r.map(|(subpasta, espaco, nome)| Referencia {
                        subpasta,
                        espaco,
                        nome: nome.into(),
                    }),
                },
                N::A(motivo) => Natureza::Ambiente { motivo },
            },
        })
        .collect();
    for def in CANAIS {
        do_canal(def, &mut v);
    }
    v.sort_by(|a, b| a.chave.cmp(&b.chave));
    v
}

/// O catalogo inteiro, ordenado pela chave.
pub fn catalogo() -> &'static [Chave] {
    static C: OnceLock<Vec<Chave>> = OnceLock::new();
    C.get_or_init(montar)
}

pub fn por_chave(chave: &str) -> Option<&'static Chave> {
    catalogo().iter().find(|c| c.chave == chave)
}

pub fn por_variavel(var: &str) -> Option<&'static Chave> {
    catalogo().iter().find(|c| c.variavel == var)
}

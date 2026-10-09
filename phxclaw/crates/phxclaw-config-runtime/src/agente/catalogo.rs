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
    /// A descricao em portugues (com acento: e texto de tela, nao identificador).
    pub descricao: String,
    /// A mesma descricao em ingles. A descricao e ROTULO, nao dado: a tela escolhe pelo
    /// idioma da fabrica (qualificacao da UI de 01/10/2026, M14).
    pub descricao_en: String,
    pub natureza: Natureza,
    /// O motivo de `Natureza::Ambiente` em ingles (o portugues fica no proprio enum).
    pub motivo_en: Option<&'static str>,
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

/// Texto de tela nos dois idiomas do catalogo: (portugues, ingles). Par na mesma linha
/// para que chave nova nao nasca sem a traducao -- o teste reprova a que nascer.
type Txt = (&'static str, &'static str);

enum N {
    C,
    /// Segredo: comando, e (subpasta, espaco, nome) no broker.
    S(
        &'static str,
        Option<(&'static str, &'static str, &'static str)>,
    ),
    A(Txt),
}

struct L {
    k: &'static str,
    v: &'static str,
    t: Tipo,
    p: Option<&'static str>,
    d: Txt,
    n: N,
}

const fn c(k: &'static str, v: &'static str, t: Tipo, p: Option<&'static str>, d: Txt) -> L {
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
    d: Txt,
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
const fn a(k: &'static str, v: &'static str, t: Tipo, d: Txt, motivo: Txt) -> L {
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

const LOCALIZA: Txt = (
    "localiza a própria configuração: o arquivo não pode dizer onde ele mesmo mora",
    "locates its own configuration: the file cannot say where it itself lives",
);
const EXPORTADA: Txt = (
    "exportada pelo agente ao processo filho, não lida como configuração",
    "exported by the agent to the child process, not read as configuration",
);
const PROVA: Txt = (
    "chave de prova manual (teste ignorado por padrão), não configuração do agente",
    "manual test key (test ignored by default), not agent configuration",
);
const ACAO: Txt = (
    "entrada da Action do GitHub (passo do fluxo), não configuração do agente",
    "GitHub Action input (workflow step), not agent configuration",
);

const FIXAS: &[L] = &[
    // --- agente e modelo ---
    c(
        "modelo.padrao",
        "PHXCLAW_MODELO",
        T,
        Some("ollama:qwen2.5:1.5b"),
        (
            "Modelo padrão do agente e da API (provedor:modelo)",
            "Default model of the agent and the API (provider:model)",
        ),
    ),
    c(
        "modelo.local",
        "PHXCLAW_MODELO_LOCAL",
        T,
        None,
        (
            "Modelo local dos papéis da equipe que a planilha roteia para o Ollama",
            "Local model for the team roles the spreadsheet routes to Ollama",
        ),
    ),
    c(
        "modelo.visao",
        "PHXCLAW_MODELO_VISAO",
        T,
        Some("qwen2.5vl:3b"),
        (
            "Modelo de visão da leitura de tela",
            "Vision model for screen reading",
        ),
    ),
    c(
        "agente.estilo",
        "PHXCLAW_ESTILO",
        T,
        None,
        (
            "Estilo de saída padrão (embutido ou .phxclaw/estilos/<nome>.md)",
            "Default output style (built-in or .phxclaw/estilos/<name>.md)",
        ),
    ),
    c(
        "agente.capacidades",
        "PHXCLAW_CAPACIDADES",
        V,
        None,
        (
            "Capacidades liberadas (web.search, fs.read...); vazio = CAPACIDADES_PADRAO",
            "Granted capabilities (web.search, fs.read...); empty = CAPACIDADES_PADRAO",
        ),
    ),
    c(
        "agente.memoria_escopo",
        "PHXCLAW_MEMORIA_ESCOPO",
        T,
        Some("padrao"),
        (
            "Escopo da memória persistente (<pasta>/tasks/_memoria/<escopo>.json)",
            "Scope of the persistent memory (<folder>/tasks/_memoria/<scope>.json)",
        ),
    ),
    c(
        "agente.skills_dir",
        "PHXCLAW_SKILLS_DIR",
        P,
        None,
        (
            "Pasta das skills; vazio = <pasta>/tasks/_skills",
            "Skills folder; empty = <folder>/tasks/_skills",
        ),
    ),
    c(
        "agente.agentes_dir",
        "PHXCLAW_AGENTES_DIR",
        P,
        None,
        (
            "Pasta dos manifestos dos papéis da equipe (config/agents)",
            "Folder of the team role manifests (config/agents)",
        ),
    ),
    c(
        "agente.lsp",
        "PHXCLAW_LSP",
        B,
        Some("true"),
        (
            "Servidores de linguagem (LSP) nas ferramentas de código; 0 desliga",
            "Language servers (LSP) in the code tools; 0 turns them off",
        ),
    ),
    c(
        "agente.tentativas_argumento",
        "PHXCLAW_TENTATIVAS_ARGUMENTO",
        I,
        Some("2"),
        (
            "Novas tentativas por ferramenta depois de argumento inválido (seguidas); esgotou, a tarefa falha",
            "Retries per tool after an invalid argument (consecutive); when exhausted, the task fails",
        ),
    ),
    c(
        "agente.heartbeat_min",
        "PHXCLAW_HEARTBEAT_MIN",
        I,
        Some("30"),
        (
            "Intervalo do heartbeat em minutos; 0 desliga",
            "Heartbeat interval in minutes; 0 turns it off",
        ),
    ),
    c(
        "agente.heartbeat_arquivo",
        "PHXCLAW_HEARTBEAT",
        P,
        None,
        (
            "Arquivo do heartbeat; vazio = HEARTBEAT.md do projeto ou da pasta",
            "Heartbeat file; empty = HEARTBEAT.md of the project or of the folder",
        ),
    ),
    a(
        "agente.pasta",
        "PHXCLAW_HOME",
        P,
        (
            "Pasta do agente (padrão var/agente); o config.json da pasta mora nela",
            "Agent folder (default var/agente); the folder's config.json lives in it",
        ),
        LOCALIZA,
    ),
    a(
        "agente.projeto",
        "PHXCLAW_PROJETO",
        P,
        (
            "Raiz do projeto (padrão a pasta corrente); o .phxclaw/config.json mora nela",
            "Project root (default the current folder); .phxclaw/config.json lives in it",
        ),
        LOCALIZA,
    ),
    // --- perfil (uma camada por cima da pasta, como o perfil do VS Code) ---
    a(
        "perfil.ativo",
        "PHXCLAW_PERFIL",
        T,
        (
            "Perfil de configuração desta execução (um nome de `perfis` do config.json da pasta)",
            "Configuration profile for this run (a name from `perfis` in the folder's config.json)",
        ),
        (
            "escolhe o perfil de UMA execução; o persistente é o campo perfil_ativo do config.json da pasta",
            "picks the profile for ONE run; the persistent one is the perfil_ativo field of the folder's config.json",
        ),
    ),
    // --- API, ponte e dispositivos ---
    c(
        "api.host",
        "PHXCLAW_API_HOST",
        T,
        Some("127.0.0.1"),
        (
            "Endereço em que a API escuta; expor é decisão do operador",
            "Address the API listens on; exposing it is the operator's decision",
        ),
    ),
    c(
        "api.porta",
        "PHXCLAW_API_PORT",
        I,
        None,
        ("Porta da API do app de mesa", "Desktop app API port"),
    ),
    s(
        "api.token",
        "PHXCLAW_API_TOKEN",
        (
            "Bearer da API (24+ caracteres)",
            "API bearer (24+ characters)",
        ),
        "phxclaw api chave",
        Some(("api", "api", "api-token")),
    ),
    c(
        "api.tarefas_por_minuto",
        "PHXCLAW_API_TAREFAS_POR_MINUTO",
        I,
        Some("10"),
        (
            "Teto de criação de tarefas pela API (balde de fichas)",
            "Cap on task creation through the API (token bucket)",
        ),
    ),
    c(
        "api.webhook_origens",
        "PHXCLAW_WEBHOOK_ORIGINS",
        V,
        None,
        (
            "Origens para onde o webhook de fim de tarefa pode ir; vazio = recusado",
            "Origins the end-of-task webhook may go to; empty = refused",
        ),
    ),
    c(
        "api.url_publica",
        "PHXCLAW_PUBLIC_URL",
        T,
        Some("http://127.0.0.1:8787"),
        (
            "URL pública do agente (links de site e de artefato)",
            "Public URL of the agent (site and artifact links)",
        ),
    ),
    c(
        "api.url_cliente",
        "PHXCLAW_URL",
        T,
        Some("http://127.0.0.1:8787"),
        (
            "Servidor que o SDK e o exemplo do SDK chamam",
            "Server that the SDK and the SDK example call",
        ),
    ),
    c(
        "ponte.host",
        "PHXCLAW_PONTE_HOST",
        T,
        Some("127.0.0.1"),
        (
            "Endereço em que a ponte de controle remoto escuta",
            "Address the remote control bridge listens on",
        ),
    ),
    s(
        "ponte.token",
        "PHXCLAW_PONTE_TOKEN",
        (
            "Bearer da ponte (24+ caracteres)",
            "Bridge bearer (24+ characters)",
        ),
        "phxclaw ponte chave",
        Some(("ponte", "ponte", "ponte-token")),
    ),
    c(
        "dispositivos.host",
        "PHXCLAW_DEVICE_HOST",
        T,
        Some("127.0.0.1"),
        (
            "Endereço do servidor de dispositivos",
            "Device server address",
        ),
    ),
    c(
        "dispositivos.tenant_uuid",
        "PHXCLAW_TENANT_UUID",
        T,
        None,
        (
            "Inquilino (UUID) do nó de dispositivo e da ponte",
            "Tenant (UUID) of the device node and of the bridge",
        ),
    ),
    c(
        "dispositivos.no_uuid",
        "PHXCLAW_NODE_UUID",
        T,
        None,
        ("UUID do nó de dispositivo", "Device node UUID"),
    ),
    c(
        "dispositivos.wss_url",
        "PHXCLAW_DEVICE_WSS_URL",
        T,
        None,
        (
            "Endereço WSS do servidor para o nó de dispositivo",
            "Server WSS address for the device node",
        ),
    ),
    c(
        "dispositivos.keystore",
        "PHXCLAW_DEVICE_KEYSTORE",
        P,
        None,
        (
            "Arquivo da chave do nó de dispositivo",
            "Device node key file",
        ),
    ),
    c(
        "dispositivos.ca_pem",
        "PHXCLAW_DEVICE_CA_PEM",
        P,
        None,
        (
            "Autoridade (PEM) que o nó aceita no TLS",
            "Authority (PEM) the node accepts in TLS",
        ),
    ),
    s(
        "dispositivos.token_pareamento",
        "PHXCLAW_ENROLLMENT_TOKEN",
        ("Token de pareamento do nó", "Node pairing token"),
        "phxclaw dispositivos chave",
        Some(("dispositivos", "dispositivos", "token-pareamento")),
    ),
    // --- voz ---
    c(
        "voz.tts.provedor",
        "PHXCLAW_TTS_PROVEDOR",
        Tipo::Enum(&["comando", "elevenlabs"]),
        Some("comando"),
        (
            "Quem fala: o comando local ou a ElevenLabs",
            "Who speaks: the local command or ElevenLabs",
        ),
    ),
    c(
        "voz.tts.comando",
        "PHXCLAW_TTS_COMMAND",
        T,
        None,
        (
            "Comando de fala local (piper ou equivalente)",
            "Local speech command (piper or equivalent)",
        ),
    ),
    c(
        "voz.tts.modelo",
        "PHXCLAW_TTS_MODEL",
        P,
        None,
        ("Modelo do comando de fala", "Speech command model"),
    ),
    c(
        "voz.tts.modelo_sha256",
        "PHXCLAW_TTS_MODEL_SHA256",
        T,
        None,
        (
            "SHA-256 esperado do modelo de fala",
            "Expected SHA-256 of the speech model",
        ),
    ),
    c(
        "voz.tts.pastas",
        "PHXCLAW_TTS_DIRS",
        Tipo::Lista(':'),
        None,
        (
            "Pastas em que a fala pode gravar (separadas por : no ambiente)",
            "Folders the speech may write to (separated by : in the environment)",
        ),
    ),
    c(
        "voz.stt.provedor",
        "PHXCLAW_STT_PROVEDOR",
        Tipo::Enum(&["whisper", "elevenlabs"]),
        Some("whisper"),
        (
            "Quem transcreve: o whisper.cpp local ou a ElevenLabs",
            "Who transcribes: the local whisper.cpp or ElevenLabs",
        ),
    ),
    c(
        "voz.whisper.bin",
        "PHXCLAW_WHISPER_BIN",
        P,
        None,
        ("Executável do whisper.cpp", "whisper.cpp executable"),
    ),
    c(
        "voz.whisper.modelo",
        "PHXCLAW_WHISPER_MODEL",
        P,
        None,
        ("Modelo do whisper.cpp", "whisper.cpp model"),
    ),
    c(
        "voz.whisper.modelo_sha256",
        "PHXCLAW_WHISPER_MODEL_SHA256",
        T,
        None,
        (
            "SHA-256 esperado do modelo do whisper",
            "Expected SHA-256 of the whisper model",
        ),
    ),
    c(
        "git.segredos.gitleaks_bin",
        "PHXCLAW_GITLEAKS_BIN",
        P,
        None,
        (
            "Executável do gitleaks que varre add/commit do git_write (sem ele, a varredura não é feita e o resultado diz isso)",
            "gitleaks executable that scans git_write add/commit (without it, the scan is not done and the result says so)",
        ),
    ),
    c(
        "git.segredos.gitleaks_sha256",
        "PHXCLAW_GITLEAKS_SHA256",
        T,
        None,
        (
            "SHA-256 esperado do executável do gitleaks; sem ele o binário não roda",
            "Expected SHA-256 of the gitleaks executable; without it the binary does not run",
        ),
    ),
    c(
        "git.segredos.exigir",
        "PHXCLAW_GITLEAKS_EXIGIR",
        B,
        Some("false"),
        (
            "Recusa add/commit do git_write quando a varredura de segredos não pode ser feita",
            "Refuse git_write add/commit when the secret scan cannot be done",
        ),
    ),
    c(
        "voz.kws.bin",
        "PHXCLAW_KWS_BIN",
        P,
        None,
        ("Executável da palavra de ativação", "Wake word executable"),
    ),
    c(
        "voz.kws.modelo_dir",
        "PHXCLAW_KWS_MODEL_DIR",
        P,
        None,
        (
            "Pasta do modelo da palavra de ativação",
            "Wake word model folder",
        ),
    ),
    c(
        "voz.kws.modelo_sha256",
        "PHXCLAW_KWS_MODEL_SHA256",
        T,
        None,
        (
            "SHA-256 esperado do modelo da palavra de ativação",
            "Expected SHA-256 of the wake word model",
        ),
    ),
    c(
        "elevenlabs.api",
        "PHXCLAW_ELEVENLABS_API",
        T,
        Some("https://api.elevenlabs.io"),
        ("Base da API da ElevenLabs", "ElevenLabs API base"),
    ),
    c(
        "elevenlabs.voz",
        "PHXCLAW_ELEVENLABS_VOZ",
        T,
        None,
        (
            "voice_id da ElevenLabs (obrigatório com voz.tts.provedor=elevenlabs)",
            "ElevenLabs voice_id (required with voz.tts.provedor=elevenlabs)",
        ),
    ),
    c(
        "elevenlabs.modelo",
        "PHXCLAW_ELEVENLABS_MODELO",
        T,
        None,
        ("Modelo de fala da ElevenLabs", "ElevenLabs speech model"),
    ),
    c(
        "elevenlabs.formato",
        "PHXCLAW_ELEVENLABS_FORMATO",
        T,
        Some("wav_16000"),
        (
            "Formato de saída da fala (wav_16000, pcm_22050...)",
            "Speech output format (wav_16000, pcm_22050...)",
        ),
    ),
    c(
        "elevenlabs.estabilidade",
        "PHXCLAW_ELEVENLABS_ESTABILIDADE",
        R,
        None,
        ("stability da voz (0 a 1)", "Voice stability (0 to 1)"),
    ),
    c(
        "elevenlabs.similaridade",
        "PHXCLAW_ELEVENLABS_SIMILARIDADE",
        R,
        None,
        (
            "similarity_boost da voz (0 a 1)",
            "Voice similarity_boost (0 to 1)",
        ),
    ),
    c(
        "elevenlabs.stt_modelo",
        "PHXCLAW_ELEVENLABS_STT_MODELO",
        T,
        Some("scribe_v2"),
        (
            "Modelo de transcrição da ElevenLabs",
            "ElevenLabs transcription model",
        ),
    ),
    s(
        "elevenlabs.chave",
        "PHXCLAW_ELEVENLABS_API_KEY",
        ("Chave da ElevenLabs", "ElevenLabs key"),
        "phxclaw elevenlabs chave",
        Some(("elevenlabs", "elevenlabs", "elevenlabs-chave")),
    ),
    // --- imagem, xAI, Gemini ---
    c(
        "imagem.provedor",
        "PHXCLAW_IMAGEM_PROVEDOR",
        Tipo::Enum(&["openai", "comfyui", "nanobanana"]),
        None,
        (
            "Gerador de imagem; vazio = só o SVG local",
            "Image generator; empty = local SVG only",
        ),
    ),
    c(
        "imagem.url",
        "PHXCLAW_IMAGEM_URL",
        T,
        None,
        (
            "Base do gerador de imagem (openai: https://api.openai.com)",
            "Image generator base (openai: https://api.openai.com)",
        ),
    ),
    c(
        "imagem.modelo",
        "PHXCLAW_IMAGEM_MODELO",
        T,
        None,
        (
            "Modelo do gerador de imagem (openai: gpt-image-1)",
            "Image generator model (openai: gpt-image-1)",
        ),
    ),
    c(
        "imagem.comfy_fluxo",
        "PHXCLAW_COMFY_WORKFLOW",
        P,
        None,
        (
            "Fluxo do ComfyUI (obrigatório com imagem.provedor=comfyui)",
            "ComfyUI workflow (required with imagem.provedor=comfyui)",
        ),
    ),
    c(
        "imagem.entrada_pixels_max",
        "PHXCLAW_IMAGEM_ENTRADA_PIXELS_MAX",
        I,
        Some("40000000"),
        (
            "Teto de pixels (largura x altura) de imagem de entrada, lido do cabeçalho antes de decodificar",
            "Pixel ceiling (width x height) of an input image, read from the header before decoding",
        ),
    ),
    s(
        "imagem.chave",
        "PHXCLAW_IMAGEM_CHAVE",
        (
            "Chave do gerador de imagem openai",
            "Key of the openai image generator",
        ),
        "phxclaw imagem chave",
        Some(("imagem", "imagem", "imagem-chave")),
    ),
    s(
        "openai.chave",
        "PHXCLAW_OPENAI_API_KEY",
        ("Chave da OpenAI (modelos openai:*)", "OpenAI key (openai:* models)"),
        "phxclaw openai chave",
        Some(("openai", "openai", "openai-chave")),
    ),
    s(
        "anthropic.chave",
        "PHXCLAW_ANTHROPIC_API_KEY",
        ("Chave da Anthropic (modelos anthropic:*)", "Anthropic key (anthropic:* models)"),
        "phxclaw anthropic chave",
        Some(("anthropic", "anthropic", "anthropic-chave")),
    ),
    s(
        "gemini.chave",
        "PHXCLAW_GEMINI_API_KEY",
        ("Chave do Gemini (nanobanana)", "Gemini key (nanobanana)"),
        "phxclaw gemini chave",
        Some(("gemini", "gemini", "gemini-chave")),
    ),
    c(
        "xai.api",
        "PHXCLAW_XAI_API",
        T,
        Some("https://api.x.ai/v1"),
        ("Base da API da xAI", "xAI API base"),
    ),
    c(
        "xai.modelo",
        "PHXCLAW_XAI_MODELO",
        T,
        Some("grok-4"),
        ("Modelo da busca da xAI", "xAI search model"),
    ),
    s(
        "xai.chave",
        "PHXCLAW_XAI_API_KEY",
        ("Chave da xAI", "xAI key"),
        "phxclaw xai chave",
        Some(("xai", "xai", "xai-chave")),
    ),
    // --- n8n (ferramenta n8n_workflow e gatilhos; docs/N8N.md) ---
    c(
        "n8n.url",
        "PHXCLAW_N8N_URL",
        T,
        None,
        (
            "Origem do n8n do operador (ex.: http://127.0.0.1:5678); vazio = sem n8n_workflow",
            "Operator's n8n origin (e.g. http://127.0.0.1:5678); empty = no n8n_workflow",
        ),
    ),
    s(
        "n8n.chave",
        "PHXCLAW_N8N_API_KEY",
        ("Chave da API pública do n8n (list/status)", "n8n public API key (list/status)"),
        "phxclaw n8n chave",
        Some(("n8n", "n8n", "n8n-chave")),
    ),
    s(
        "n8n.webhook_segredo",
        "PHXCLAW_N8N_WEBHOOK_SEGREDO",
        (
            "Segredo que assina o run do n8n_workflow (HMAC ou Header Auth)",
            "Secret signing n8n_workflow run (HMAC or Header Auth)",
        ),
        "phxclaw n8n segredo",
        Some(("n8n", "n8n", "n8n-webhook-segredo")),
    ),
    // --- e-mail (ferramenta send_email) ---
    c(
        "email.smtp.host",
        "PHXCLAW_SMTP_HOST",
        T,
        None,
        ("Servidor SMTP", "SMTP server"),
    ),
    c(
        "email.smtp.porta",
        "PHXCLAW_SMTP_PORT",
        I,
        None,
        (
            "Porta SMTP; vazio = 465 (tls), 587 (starttls) ou 25 (plain)",
            "SMTP port; empty = 465 (tls), 587 (starttls) or 25 (plain)",
        ),
    ),
    c(
        "email.smtp.seguranca",
        "PHXCLAW_SMTP_SECURITY",
        Tipo::Enum(&["tls", "starttls", "plain"]),
        Some("tls"),
        ("Segurança do SMTP", "SMTP security"),
    ),
    c(
        "email.smtp.usuario",
        "PHXCLAW_SMTP_USER",
        T,
        None,
        ("Usuário do SMTP", "SMTP user"),
    ),
    s(
        "email.smtp.senha",
        "PHXCLAW_SMTP_PASSWORD",
        ("Senha do SMTP", "SMTP password"),
        "phxclaw email chave",
        Some(("canal", "canais", "email-smtp_senha")),
    ),
    c(
        "email.remetente",
        "PHXCLAW_EMAIL_FROM",
        T,
        None,
        ("Remetente do e-mail", "E-mail sender"),
    ),
    c(
        "email.permitidos",
        "PHXCLAW_EMAIL_PERMITIDOS",
        V,
        None,
        (
            "Destinatários permitidos do send_email",
            "Recipients allowed for send_email",
        ),
    ),
    // --- forjas e MCP ---
    c(
        "forja.github.api",
        "PHXCLAW_GITHUB_API",
        T,
        Some("https://api.github.com"),
        ("Base da API do GitHub", "GitHub API base"),
    ),
    c(
        "forja.gitlab.api",
        "PHXCLAW_GITLAB_API",
        T,
        Some("https://gitlab.com/api/v4"),
        ("Base da API do GitLab", "GitLab API base"),
    ),
    s(
        "forja.github.token",
        "PHXCLAW_GITHUB_TOKEN",
        ("Token do GitHub", "GitHub token"),
        "phxclaw forja token github",
        Some(("forja", "forjas", "github-token")),
    ),
    s(
        "forja.gitlab.token",
        "PHXCLAW_GITLAB_TOKEN",
        ("Token do GitLab", "GitLab token"),
        "phxclaw forja token gitlab",
        Some(("forja", "forjas", "gitlab-token")),
    ),
    c(
        "mcp.config",
        "PHXCLAW_MCP_CONFIG",
        P,
        None,
        (
            "Arquivo dos servidores MCP do operador",
            "Operator's MCP servers file",
        ),
    ),
    s(
        "mcp.token",
        "PHXCLAW_MCP_TOKEN",
        (
            "Token de um servidor MCP remoto",
            "Token of a remote MCP server",
        ),
        "phxclaw mcp token NOME",
        None,
    ),
    s(
        "mcp.segredo_cliente",
        "PHXCLAW_MCP_SEGREDO_CLIENTE",
        (
            "Segredo do cliente OAuth de um servidor MCP",
            "OAuth client secret of an MCP server",
        ),
        "phxclaw mcp login NOME",
        None,
    ),
    // --- plugins e pacotes ---
    c(
        "plugins.raiz",
        "PHXCLAW_PLUGINS_RAIZ",
        P,
        None,
        (
            "Raiz dos plugins; vazio = nenhum plugin",
            "Plugins root; empty = no plugins",
        ),
    ),
    c(
        "plugins.dir",
        "PHXCLAW_PLUGINS_DIR",
        P,
        None,
        (
            "Pasta dos manifestos; vazio = <raiz>/plugins",
            "Manifests folder; empty = <root>/plugins",
        ),
    ),
    c(
        "plugins.assinantes",
        "PHXCLAW_PLUGIN_SIGNERS",
        P,
        None,
        (
            "Trust store dos assinantes; vazio = <raiz>/config/trust/plugin-signers.json",
            "Signers trust store; empty = <root>/config/trust/plugin-signers.json",
        ),
    ),
    c(
        "plugins.chave_assinatura_arquivo",
        "PHXCLAW_PLUGIN_SIGNING_KEY_FILE",
        P,
        None,
        (
            "Arquivo da semente de assinatura (exemplo assinar)",
            "Signing seed file (assinar example)",
        ),
    ),
    s(
        "plugins.chave_assinatura",
        "PHXCLAW_PLUGIN_SIGNING_KEY",
        (
            "Semente de assinatura de plugin (exemplo assinar)",
            "Plugin signing seed (assinar example)",
        ),
        "phxclaw plugins chave",
        Some(("plugins", "plugins", "plugins-chave-assinatura")),
    ),
    c(
        "pacotes.dir",
        "PHXCLAW_PACOTES_DIR",
        P,
        None,
        (
            "Pasta dos pacotes assinados (Claude/Codex)",
            "Folder of the signed packages (Claude/Codex)",
        ),
    ),
    c(
        "pacotes.catalogo",
        "PHXCLAW_PACOTES_CATALOGO",
        T,
        None,
        (
            "Catálogo da loja de plugins (URL https ou arquivo JSON local); vazio = sem loja",
            "Plugin store catalog (https URL or local JSON file); empty = no store",
        ),
    ),
    // --- rede, banco, navegador, ferramentas ---
    c(
        "rede.destinos",
        "PHXCLAW_NET_DESTINOS",
        V,
        None,
        (
            "Destinos host:porta que a sonda de rede pode tocar",
            "host:port destinations the network probe may reach",
        ),
    ),
    c(
        "postgres.url",
        "PHXCLAW_PG_URL",
        T,
        None,
        (
            "PostgreSQL das ferramentas db (sem senha na URL: senha vai pelo .pgpass)",
            "PostgreSQL for the db tools (no password in the URL: the password goes through .pgpass)",
        ),
    ),
    c(
        "postgres.database_url",
        "PHXCLAW_DATABASE_URL",
        T,
        None,
        (
            "PostgreSQL da CLI antiga (phxclaw-cli)",
            "PostgreSQL of the old CLI (phxclaw-cli)",
        ),
    ),
    c(
        "navegador.chromium",
        "PHXCLAW_CHROMIUM",
        P,
        None,
        ("Executável do Chromium", "Chromium executable"),
    ),
    c(
        "busca.searxng_url",
        "PHXCLAW_SEARXNG_URL",
        T,
        None,
        ("SearXNG da busca na web", "SearXNG for web search"),
    ),
    c(
        "python.bin",
        "PHXCLAW_PYTHON",
        P,
        None,
        (
            "Interpretador Python das ferramentas",
            "Python interpreter for the tools",
        ),
    ),
    c(
        "python.uv",
        "PHXCLAW_UV",
        P,
        None,
        ("Executável do uv", "uv executable"),
    ),
    c(
        "ui.dir",
        "PHXCLAW_UI_DIR",
        P,
        None,
        ("Pasta da interface (PWA)", "Interface folder (PWA)"),
    ),
    c(
        "ui.bootstrap_css",
        "PHXCLAW_UI_BOOTSTRAP_CSS",
        P,
        None,
        (
            "Folha LOCAL do Bootstrap 5.3 nas telas geradas (nunca CDN); vazio = vendor/bootstrap-5.3.3/bootstrap.min.css",
            "LOCAL Bootstrap 5.3 stylesheet for generated screens (never a CDN); empty = vendor/bootstrap-5.3.3/bootstrap.min.css",
        ),
    ),
    c(
        "clima.api",
        "PHXCLAW_MET_API",
        T,
        None,
        (
            "Base da API de clima (MET Norway)",
            "Weather API base (MET Norway)",
        ),
    ),
    c(
        "clima.contato",
        "PHXCLAW_MET_CONTATO",
        T,
        None,
        (
            "Contato no User-Agent pedido pelo MET Norway",
            "Contact in the User-Agent requested by MET Norway",
        ),
    ),
    c(
        "documentos.embed",
        "PHXCLAW_DOCS_EMBED",
        T,
        None,
        (
            "Modelo de embedding da reordenação (só ollama:)",
            "Embedding model for reranking (ollama: only)",
        ),
    ),
    c(
        "missao.estado_dir",
        "PHXCLAW_STATE_DIR",
        P,
        None,
        (
            "Pasta de estado da CLI de missão",
            "State folder of the mission CLI",
        ),
    ),
    // --- motor de fluxo (SP000035, onda 3) ---
    c(
        "fluxos.max_simultaneos",
        "PHXCLAW_FLUXOS_MAX_SIMULTANEOS",
        I,
        None,
        (
            "Fluxos rodando ao mesmo tempo na instância; vazio ou 0 = sem limite (o excedente espera a vez)",
            "Flows running at the same time in the instance; empty or 0 = no limit (the excess waits its turn)",
        ),
    ),
    c(
        "fluxos.poda_dias",
        "PHXCLAW_FLUXOS_PODA_DIAS",
        I,
        None,
        (
            "Apaga execuções de fluxo terminadas há mais de N dias; vazio ou 0 = nunca",
            "Deletes flow executions finished more than N days ago; empty or 0 = never",
        ),
    ),
    c(
        "fluxos.poda_max",
        "PHXCLAW_FLUXOS_PODA_MAX",
        I,
        None,
        (
            "Mantém só as N execuções de fluxo terminadas mais novas; vazio ou 0 = todas",
            "Keeps only the N newest finished flow executions; empty or 0 = all of them",
        ),
    ),
    // --- canal do Telegram (os demais saem de CANAIS) ---
    s(
        "canais.telegram.bot_token",
        "PHXCLAW_TELEGRAM_BOT_TOKEN",
        ("Token do bot do Telegram", "Telegram bot token"),
        "PHXCLAW_TELEGRAM_BOT_TOKEN=... phxclaw canal telegram (vai ao broker na primeira vez)",
        Some(("canal", "canais", "telegram-bot")),
    ),
    c(
        "canais.telegram.chats",
        "PHXCLAW_TELEGRAM_CHATS",
        V,
        None,
        (
            "Chats que podem falar com o agente (obrigatório)",
            "Chats allowed to talk to the agent (required)",
        ),
    ),
    // --- IDE (workspace de varias raizes) ---
    c(
        "ide.raizes",
        "PHXCLAW_RAIZES",
        Tipo::Lista(':'),
        None,
        (
            "Raízes extras do workspace além da pasta do projeto (também em .phxclaw/workspace.json); o IDE as expõe ao terminal",
            "Extra workspace roots besides the project folder (also in .phxclaw/workspace.json); the IDE exposes them to the terminal",
        ),
    ),
    // --- IDE no navegador: completacao por IA (snippet-ls -> /v1/ide/completar) ---
    c(
        "ide.ia_tokens",
        "PHXCLAW_IDE_IA_TOKENS",
        I,
        Some("64"),
        (
            "Teto de tokens de uma completação por IA no editor (o modelo é o do agente)",
            "Token cap of one AI completion in the editor (the model is the agent's)",
        ),
    ),
    c(
        "ide.ia_ms",
        "PHXCLAW_IDE_IA_MS",
        I,
        Some("6000"),
        (
            "Teto de tempo (ms) de uma completação por IA no editor",
            "Time cap (ms) of one AI completion in the editor",
        ),
    ),
    c(
        "ide.api_url",
        "PHXCLAW_IDE_API_URL",
        T,
        None,
        (
            "URL por onde o editor fala com este agente (padrão: o Host da conexão do IDE)",
            "URL the editor uses to reach this agent (default: the Host of the IDE connection)",
        ),
    ),
    // --- app de mesa ---
    c(
        "desktop.exec_host",
        "PHXCLAW_ENABLE_HOST_EXEC",
        B,
        Some("false"),
        (
            "App de mesa: executar comando no host",
            "Desktop app: run a command on the host",
        ),
    ),
    c(
        "desktop.controle_webview",
        "PHXCLAW_ENABLE_WEBVIEW_CONTROL",
        B,
        Some("false"),
        (
            "App de mesa: controlar webview",
            "Desktop app: control the webview",
        ),
    ),
    c(
        "desktop.shells",
        "PHXCLAW_ENABLE_SHELLS",
        B,
        Some("false"),
        ("App de mesa: shells", "Desktop app: shells"),
    ),
    c(
        "desktop.entrada",
        "PHXCLAW_ENABLE_INPUT",
        B,
        Some("false"),
        (
            "App de mesa: mouse e teclado",
            "Desktop app: mouse and keyboard",
        ),
    ),
    c(
        "desktop.captura_tela",
        "PHXCLAW_ENABLE_SCREEN_CAPTURE",
        B,
        Some("false"),
        (
            "App de mesa: captura de tela",
            "Desktop app: screen capture",
        ),
    ),
    c(
        "desktop.webviews_externas",
        "PHXCLAW_ENABLE_EXTERNAL_WEBVIEWS",
        B,
        Some("false"),
        (
            "App de mesa: webviews externas",
            "Desktop app: external webviews",
        ),
    ),
    c(
        "desktop.controle_api_host",
        "PHXCLAW_API_HOST_CONTROL",
        B,
        Some("false"),
        (
            "App de mesa: a API controla o host",
            "Desktop app: the API controls the host",
        ),
    ),
    c(
        "desktop.webview_origens",
        "PHXCLAW_WEBVIEW_ALLOWED_ORIGINS",
        V,
        None,
        (
            "App de mesa: origens permitidas na webview",
            "Desktop app: origins allowed in the webview",
        ),
    ),
    c(
        "desktop.hx",
        "PHXCLAW_HX",
        P,
        None,
        (
            "App de mesa: executável do Helix",
            "Desktop app: Helix executable",
        ),
    ),
    c(
        "ide.historico_versoes_max",
        "PHXCLAW_IDE_HISTORICO_VERSOES_MAX",
        I,
        Some("50"),
        (
            "IDE: versões guardadas por arquivo no histórico local de gravações (.phxclaw/historico)",
            "IDE: versions kept per file in the local save history (.phxclaw/historico)",
        ),
    ),
    // --- pontes para agentes externos ---
    c(
        "pontes.claw.bin",
        "PHXCLAW_CLAW_BIN",
        P,
        Some("claw"),
        ("Executável do claw-code", "claw-code executable"),
    ),
    c(
        "pontes.claw.workspace",
        "PHXCLAW_CLAW_WORKSPACE",
        P,
        Some("."),
        ("Pasta de trabalho do claw-code", "claw-code working folder"),
    ),
    c(
        "pontes.claw.timeout_ms",
        "PHXCLAW_CLAW_TIMEOUT_MS",
        I,
        Some("120000"),
        ("Prazo do claw-code em ms", "claw-code timeout in ms"),
    ),
    c(
        "pontes.octopus.bin",
        "PHXCLAW_OCTOPUS_BIN",
        P,
        Some("octopus-console"),
        ("Executável do Octopus", "Octopus executable"),
    ),
    c(
        "pontes.octopus.workspace",
        "PHXCLAW_OCTOPUS_WORKSPACE",
        P,
        Some("."),
        ("Pasta de trabalho do Octopus", "Octopus working folder"),
    ),
    c(
        "pontes.openclaw_rs.bin",
        "PHXCLAW_OPENCLAW_RS_BIN",
        P,
        None,
        ("Executável do openclaw-rs", "openclaw-rs executable"),
    ),
    c(
        "pontes.openclaw_rs.workspace",
        "PHXCLAW_OPENCLAW_RS_WORKSPACE",
        P,
        Some("."),
        (
            "Pasta de trabalho do openclaw-rs",
            "openclaw-rs working folder",
        ),
    ),
    c(
        "pontes.openclaw_rs.timeout_ms",
        "PHXCLAW_OPENCLAW_RS_TIMEOUT_MS",
        I,
        None,
        ("Prazo do openclaw-rs em ms", "openclaw-rs timeout in ms"),
    ),
    c(
        "pontes.rustclaw.bin",
        "PHXCLAW_RUSTCLAW_BIN",
        P,
        Some("rustclaw"),
        ("Executável do rustclaw", "rustclaw executable"),
    ),
    c(
        "pontes.rustclaw.workspace",
        "PHXCLAW_RUSTCLAW_WORKSPACE",
        P,
        Some("."),
        ("Pasta de trabalho do rustclaw", "rustclaw working folder"),
    ),
    c(
        "pontes.rustclaw.timeout_ms",
        "PHXCLAW_RUSTCLAW_TIMEOUT_MS",
        I,
        Some("60000"),
        ("Prazo do rustclaw em ms", "rustclaw timeout in ms"),
    ),
    c(
        "pontes.rustclaw.prompt_no_argv",
        "PHXCLAW_RUSTCLAW_ALLOW_PROMPT_ARGV",
        B,
        Some("false"),
        (
            "Permite o prompt na linha de comando do rustclaw (fica visível no ps)",
            "Allows the prompt on the rustclaw command line (it shows in ps)",
        ),
    ),
    // --- exportadas ao processo filho ---
    a(
        "exportadas.ia_completar",
        "PHXCLAW_IA_COMPLETAR",
        T,
        (
            "URL da completação por IA que o agente dá ao Helix (lida pelo phxclaw-snippet-ls)",
            "AI completion URL the agent hands to Helix (read by phxclaw-snippet-ls)",
        ),
        EXPORTADA,
    ),
    a(
        "exportadas.hook_entrada",
        "PHXCLAW_HOOK_INPUT",
        T,
        (
            "Entrada do hook (JSON do evento)",
            "Hook input (event JSON)",
        ),
        EXPORTADA,
    ),
    a(
        "exportadas.hook_evento",
        "PHXCLAW_HOOK_EVENT",
        T,
        ("Nome do evento do hook", "Hook event name"),
        EXPORTADA,
    ),
    a(
        "exportadas.hook_projeto",
        "PHXCLAW_PROJECT_DIR",
        P,
        (
            "Pasta do projeto vista pelo hook",
            "Project folder as seen by the hook",
        ),
        EXPORTADA,
    ),
    a(
        "exportadas.plugin_args",
        "PHXCLAW_ARGS",
        T,
        (
            "Argumentos da chamada ao plugin",
            "Arguments of the plugin call",
        ),
        EXPORTADA,
    ),
    a(
        "exportadas.plugin",
        "PHXCLAW_PLUGIN",
        T,
        ("Nome do plugin chamado", "Name of the called plugin"),
        EXPORTADA,
    ),
    a(
        "exportadas.repl_driver",
        "PHXCLAW_REPL_DRIVER",
        T,
        ("Driver do REPL Python", "Python REPL driver"),
        EXPORTADA,
    ),
    a(
        "exportadas.repl_marca",
        "PHXCLAW_REPL_MARCA",
        T,
        ("Marca de fim de saída do REPL", "REPL end-of-output marker"),
        EXPORTADA,
    ),
    a(
        "exportadas.pytest_json",
        "PHXCLAW_PYTEST_JSON",
        P,
        ("Arquivo do relatório do pytest", "pytest report file"),
        EXPORTADA,
    ),
    // --- Action do GitHub ---
    a(
        "acao.bin",
        "PHXCLAW_BIN",
        P,
        (
            "Binário do phxclaw exportado entre passos",
            "phxclaw binary exported between steps",
        ),
        ACAO,
    ),
    a(
        "acao.bin_entrada",
        "PHXCLAW_BIN_ENTRADA",
        P,
        (
            "Binário do phxclaw dado à Action",
            "phxclaw binary given to the Action",
        ),
        ACAO,
    ),
    a(
        "acao.foco",
        "PHXCLAW_FOCO",
        T,
        ("Foco da revisão", "Review focus"),
        ACAO,
    ),
    a(
        "acao.falhar_em",
        "PHXCLAW_FALHAR_EM",
        T,
        (
            "Severidade que reprova a revisão",
            "Severity that fails the review",
        ),
        ACAO,
    ),
    // --- provas manuais ---
    a(
        "testes.ollama_url",
        "PHXCLAW_E2E_OLLAMA_URL",
        T,
        ("Ollama da prova real", "Ollama for the real test"),
        PROVA,
    ),
    a(
        "testes.ollama_modelo",
        "PHXCLAW_E2E_OLLAMA_MODEL",
        T,
        (
            "Modelo da prova real do Ollama",
            "Model for the real Ollama test",
        ),
        PROVA,
    ),
    a(
        "testes.ollama_modelo_embed",
        "PHXCLAW_E2E_OLLAMA_EMBED_MODEL",
        T,
        (
            "Modelo de embedding da prova real",
            "Embedding model for the real test",
        ),
        PROVA,
    ),
    a(
        "testes.rls_url",
        "PHXCLAW_E2E_RLS_URL",
        T,
        ("PostgreSQL da prova de RLS", "PostgreSQL for the RLS test"),
        PROVA,
    ),
    a(
        "testes.whisper_bin",
        "PHXCLAW_E2E_WHISPER_BIN",
        P,
        ("whisper.cpp da prova real", "whisper.cpp for the real test"),
        PROVA,
    ),
    a(
        "testes.whisper_modelo",
        "PHXCLAW_E2E_WHISPER_MODEL",
        P,
        (
            "Modelo do whisper da prova real",
            "whisper model for the real test",
        ),
        PROVA,
    ),
    a(
        "testes.whisper_modelo_sha256",
        "PHXCLAW_E2E_WHISPER_MODEL_SHA256",
        T,
        (
            "SHA-256 do modelo da prova real",
            "SHA-256 of the real test model",
        ),
        PROVA,
    ),
    a(
        "testes.whisper_audio",
        "PHXCLAW_E2E_WHISPER_AUDIO",
        P,
        (
            "Áudio da prova real do whisper",
            "Audio for the real whisper test",
        ),
        PROVA,
    ),
    a(
        "testes.caos_segundos",
        "PHXCLAW_CHAOS_SECONDS",
        I,
        (
            "Duração da prova de caos no PostgreSQL",
            "Duration of the PostgreSQL chaos test",
        ),
        PROVA,
    ),
    a(
        "testes.prova_visao",
        "PHXCLAW_PROVA_VISAO",
        T,
        (
            "Liga a prova real de visão",
            "Turns on the real vision test",
        ),
        PROVA,
    ),
    a(
        "testes.prova_visao_png",
        "PHXCLAW_PROVA_VISAO_PNG",
        P,
        (
            "Imagem da prova real de visão",
            "Image for the real vision test",
        ),
        PROVA,
    ),
    a(
        "testes.prova_visao_texto",
        "PHXCLAW_PROVA_VISAO_TEXTO",
        T,
        (
            "Texto esperado na prova real de visão",
            "Expected text in the real vision test",
        ),
        PROVA,
    ),
    a(
        "testes.office_exigir_prova",
        "PHXCLAW_OFFICE_EXIGIR_PROVA",
        T,
        (
            "Exige a prova real do office",
            "Requires the real office test",
        ),
        PROVA,
    ),
    a(
        "testes.corpus_skills",
        "PHXCLAW_CORPUS_SKILLS",
        P,
        ("Corpus de skills da prova", "Skills corpus for the test"),
        PROVA,
    ),
    a(
        "testes.prova_docs",
        "PHXCLAW_PROVA_DOCS",
        P,
        (
            "Pasta da medição real de documentos",
            "Folder of the real document measurement",
        ),
        PROVA,
    ),
    a(
        "testes.prova_docs_gabarito",
        "PHXCLAW_PROVA_DOCS_GABARITO",
        P,
        (
            "Gabarito da medição real de documentos",
            "Answer key of the real document measurement",
        ),
        PROVA,
    ),
    a(
        "testes.flutter",
        "PHXCLAW_FLUTTER",
        P,
        ("Flutter da prova da UI", "Flutter for the UI test"),
        PROVA,
    ),
    a(
        "testes.telegram_chat_id",
        "PHXCLAW_TELEGRAM_CHAT_ID",
        T,
        (
            "Chat da prova real do Telegram",
            "Chat for the real Telegram test",
        ),
        PROVA,
    ),
];

// --- os canais de `phxclaw canal <nome>` (convencao PHXCLAW_<PREFIXO>_<CHAVE>) ---------

#[derive(Clone, Copy)]
enum K {
    /// Texto obrigatorio ou opcional.
    T(Txt),
    /// Texto com padrao.
    Tp(Txt, &'static str),
    /// Lista separada por virgula.
    L(Txt),
    /// Booleano com padrao.
    B(Txt, &'static str),
    /// Segredo (vai ao broker `<pasta>/canal`, espaco `canais`, nome `<canal>-<chave>`).
    S(Txt),
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
            ("TOKEN", K::S(("Token do bot", "Bot token"))),
        ],
    },
    CanalDef {
        canal: "slack",
        prefixo: "SLACK",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S(("Token do bot", "Bot token"))),
        ],
    },
    CanalDef {
        canal: "whatsapp",
        prefixo: "WHATSAPP",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            (
                "APP_SECRET",
                K::S(("App secret da Meta", "Meta app secret")),
            ),
            (
                "VERIFY_TOKEN",
                K::S(("Verify token do webhook", "Webhook verify token")),
            ),
            ("TOKEN", K::S(("Token de acesso", "Access token"))),
            ("PHONE_ID", K::T(("phone_number_id", "phone_number_id"))),
        ],
    },
    CanalDef {
        canal: "messenger",
        prefixo: "MESSENGER",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            (
                "APP_SECRET",
                K::S(("App secret da Meta", "Meta app secret")),
            ),
            (
                "VERIFY_TOKEN",
                K::S(("Verify token do webhook", "Webhook verify token")),
            ),
            ("TOKEN", K::S(("Token da página", "Page token"))),
        ],
    },
    CanalDef {
        canal: "teams",
        prefixo: "TEAMS",
        chaves: &[
            (
                "LOGIN",
                K::T((
                    "Endpoint de login; vazio = o oficial",
                    "Login endpoint; empty = the official one",
                )),
            ),
            (
                "SERVICOS",
                K::L((
                    "Origens de serviço aceitas; vazio = a oficial",
                    "Accepted service origins; empty = the official one",
                )),
            ),
            (
                "CHAVE_URL",
                K::S((
                    "Chave da URL de entrada (opcional; o JWT RS256 é conferido sempre)",
                    "Inbound URL key (optional; the RS256 JWT is always verified)",
                )),
            ),
            ("APP_SECRET", K::S(("Segredo do app", "App secret"))),
            (
                "APP_ID",
                K::T(("ID do app (audiência do token)", "App ID (token audience)")),
            ),
            (
                "JWKS",
                K::T((
                    "URL das chaves do Bot Framework (JWKS); vazio = a oficial",
                    "Bot Framework key set (JWKS) URL; empty = the official one",
                )),
            ),
        ],
    },
    CanalDef {
        canal: "matrix",
        prefixo: "MATRIX",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S(("Token de acesso", "Access token"))),
            ("USUARIO", K::T(("Usuário do bot", "Bot user"))),
        ],
    },
    CanalDef {
        canal: "email",
        prefixo: "EMAIL_CANAL",
        chaves: &[
            (
                "SMTP_SENHA",
                K::S(("Senha do SMTP do canal", "Channel SMTP password")),
            ),
            (
                "IMAP",
                K::T(("Servidor IMAP host:porta", "IMAP server host:port")),
            ),
            ("USUARIO", K::T(("Usuário do IMAP", "IMAP user"))),
            ("SENHA", K::S(("Senha do IMAP", "IMAP password"))),
            (
                "PASTA",
                K::Tp(("Pasta lida", "Folder that is read"), "INBOX"),
            ),
            (
                "EXIGIR_DMARC",
                K::B(
                    (
                        "Exige DMARC do remetente; não desliga",
                        "Requires the sender's DMARC; cannot be turned off",
                    ),
                    "true",
                ),
            ),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "webhook",
        prefixo: "WEBHOOK",
        chaves: &[
            ("SAIDA_URL", K::T(("URL de saída", "Outbound URL"))),
            (
                "SEGREDO",
                K::S(("Segredo da assinatura", "Signature secret")),
            ),
        ],
    },
    CanalDef {
        canal: "webchat",
        prefixo: "WEBCHAT",
        chaves: &[("CHAVE", K::S(("Chave do webchat", "Webchat key")))],
    },
    CanalDef {
        canal: "signal",
        prefixo: "SIGNAL",
        chaves: &[
            ("BASE", K::Base(Some(Some("http://127.0.0.1:8080")))),
            ("NUMERO", K::T(("Número do bot", "Bot number"))),
            (
                "TOKEN",
                K::S((
                    "Token do signal-cli-rest-api (opcional)",
                    "signal-cli-rest-api token (optional)",
                )),
            ),
        ],
    },
    CanalDef {
        canal: "googlechat",
        prefixo: "GOOGLECHAT",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            (
                "CHAVE_URL",
                K::S((
                    "Chave da URL de entrada (opcional; o JWT RS256 é conferido sempre)",
                    "Inbound URL key (optional; the RS256 JWT is always verified)",
                )),
            ),
            (
                "AUDIENCIA",
                K::T((
                    "Audiência do token: número do projeto, ou URL https:// do endpoint (ID token)",
                    "Token audience: project number, or the endpoint https:// URL (ID token)",
                )),
            ),
            (
                "JWKS",
                K::T((
                    "URL das chaves (JWKS); vazio = a oficial do modo",
                    "Key set (JWKS) URL; empty = the mode's official one",
                )),
            ),
            (
                "SAIDA_WEBHOOK",
                K::S(("Webhook de saída", "Outbound webhook")),
            ),
            (
                "ESPACO",
                K::T(("Espaço do Google Chat", "Google Chat space")),
            ),
        ],
    },
    CanalDef {
        canal: "sms",
        prefixo: "SMS",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S(("Token da conta", "Account token"))),
            ("CONTA", K::T(("Conta do provedor", "Provider account"))),
            ("NUMERO", K::T(("Número de envio", "Sender number"))),
            (
                "URL_PUBLICA",
                K::T(("URL pública do webhook", "Public webhook URL")),
            ),
        ],
    },
    CanalDef {
        canal: "mattermost",
        prefixo: "MATTERMOST",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S(("Token do bot", "Bot token"))),
            ("BOT_ID", K::T(("ID do bot", "Bot ID"))),
        ],
    },
    CanalDef {
        canal: "rocketchat",
        prefixo: "ROCKETCHAT",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S(("Token do bot", "Bot token"))),
            ("USUARIO_ID", K::T(("ID do usuário do bot", "Bot user ID"))),
            ("TIPO", K::Tp(("Tipo de sala", "Room type"), "channels")),
        ],
    },
    CanalDef {
        canal: "zulip",
        prefixo: "ZULIP",
        chaves: &[
            ("BASE", K::Base(None)),
            ("CHAVE", K::S(("Chave da API", "API key"))),
            ("EMAIL", K::T(("E-mail do bot", "Bot e-mail"))),
        ],
    },
    CanalDef {
        canal: "irc",
        prefixo: "IRC",
        chaves: &[
            (
                "ENDERECO",
                K::T(("Servidor host:porta", "Server host:port")),
            ),
            ("NICK", K::T(("Apelido do bot", "Bot nickname"))),
            ("SENHA", K::S(("Senha (opcional)", "Password (optional)"))),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "twitch",
        prefixo: "TWITCH",
        chaves: &[
            (
                "ENDERECO",
                K::T(("Servidor host:porta", "Server host:port")),
            ),
            ("NICK", K::T(("Apelido do bot", "Bot nickname"))),
            ("SENHA", K::S(("Token oauth do chat", "Chat oauth token"))),
            ("TLS", K::Tls),
        ],
    },
    CanalDef {
        canal: "xmpp",
        prefixo: "XMPP",
        chaves: &[
            (
                "ENDERECO",
                K::T(("Servidor host:porta", "Server host:port")),
            ),
            ("JID", K::T(("JID do bot", "Bot JID"))),
            ("SENHA", K::S(("Senha do JID", "JID password"))),
            ("TLS", K::Tls),
            (
                "SALAS",
                K::L((
                    "JIDs das salas (MUC) em que o bot entra; cada sala também precisa estar em PERMITIDOS",
                    "JIDs of the rooms (MUC) the bot joins; each room must also be listed in PERMITIDOS",
                )),
            ),
            (
                "APELIDO",
                K::T((
                    "Apelido nas salas; vazio = a parte local do JID",
                    "Nickname in rooms; empty = the local part of the JID",
                )),
            ),
            (
                "CONFIAR_NO_NICK",
                K::B(
                    (
                        "Numa sala anônima, aceitar sala/nick de PERMITIDOS como identidade do ocupante. Risco: o nick é de quem chegar primeiro com ele",
                        "In an anonymous room, accept room/nick from PERMITIDOS as the occupant's identity. Risk: the nick belongs to whoever takes it first",
                    ),
                    "false",
                ),
            ),
        ],
    },
    CanalDef {
        canal: "mastodon",
        prefixo: "MASTODON",
        chaves: &[
            ("BASE", K::Base(None)),
            ("TOKEN", K::S(("Token de acesso", "Access token"))),
        ],
    },
    CanalDef {
        canal: "line",
        prefixo: "LINE",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            (
                "SEGREDO_CANAL",
                K::S(("Segredo do canal", "Channel secret")),
            ),
            ("TOKEN", K::S(("Token de acesso", "Access token"))),
        ],
    },
    CanalDef {
        canal: "viber",
        prefixo: "VIBER",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            ("TOKEN", K::S(("Token do bot", "Bot token"))),
            (
                "REMETENTE",
                K::Tp(("Nome do remetente", "Sender name"), "PhxClaw"),
            ),
        ],
    },
    CanalDef {
        canal: "feishu",
        prefixo: "FEISHU",
        chaves: &[
            ("BASE", K::Base(Some(None))),
            (
                "VERIFICACAO",
                K::S(("Token de verificação", "Verification token")),
            ),
            ("APP_SECRET", K::S(("Segredo do app", "App secret"))),
            ("APP_ID", K::T(("ID do app", "App ID"))),
        ],
    },
    CanalDef {
        canal: "reddit",
        prefixo: "REDDIT",
        chaves: &[
            (
                "WWW",
                K::T((
                    "Base www; vazio = a oficial",
                    "www base; empty = the official one",
                )),
            ),
            (
                "OAUTH",
                K::T((
                    "Base oauth; vazio = a oficial",
                    "oauth base; empty = the official one",
                )),
            ),
            ("APP_SECRET", K::S(("Segredo do app", "App secret"))),
            ("SENHA", K::S(("Senha da conta", "Account password"))),
            ("APP_ID", K::T(("ID do app", "App ID"))),
            ("USUARIO", K::T(("Usuário", "User"))),
        ],
    },
    CanalDef {
        canal: "nostr",
        prefixo: "NOSTR",
        chaves: &[
            ("RELAY", K::T(("Relay wss://", "wss:// relay"))),
            ("CHAVE", K::S(("Chave secreta (hex)", "Secret key (hex)"))),
        ],
    },
];

fn do_canal(def: &CanalDef, out: &mut Vec<Chave>) {
    let var = |k: &str| format!("PHXCLAW_{}_{k}", def.prefixo);
    let chave = |k: &str| format!("canais.{}.{}", def.canal, k.to_ascii_lowercase());
    let cfg = |k: &str, t: Tipo, p: Option<&'static str>, (pt, en): (String, String)| Chave {
        chave: chave(k),
        variavel: var(k),
        tipo: t,
        padrao: p,
        descricao: pt,
        descricao_en: en,
        natureza: Natureza::Config,
        motivo_en: None,
    };
    let par = |(pt, en): Txt| (pt.to_string(), en.to_string());
    out.push(cfg(
        "PERMITIDOS",
        Tipo::Lista(','),
        None,
        (
            format!(
                "Conversas de {} que podem falar com o agente (obrigatória)",
                def.canal
            ),
            format!(
                "{} conversations allowed to talk to the agent (required)",
                def.canal
            ),
        ),
    ));
    for (k, tipo) in def.chaves {
        match *tipo {
            K::T(d) => out.push(cfg(k, Tipo::Texto, None, par(d))),
            K::Tp(d, p) => out.push(cfg(k, Tipo::Texto, Some(p), par(d))),
            K::L(d) => out.push(cfg(k, Tipo::Lista(','), None, par(d))),
            K::B(d, p) => out.push(cfg(k, Tipo::Booleano, Some(p), par(d))),
            K::Base(b) => {
                let (p, d) = match b {
                    None => (
                        None,
                        ("Base do serviço (obrigatória)", "Service base (required)"),
                    ),
                    Some(None) => (
                        None,
                        (
                            "Base do serviço; vazio = a oficial",
                            "Service base; empty = the official one",
                        ),
                    ),
                    Some(Some(p)) => (Some(p), ("Base do serviço", "Service base")),
                };
                out.push(cfg(k, Tipo::Texto, p, par(d)));
            }
            K::Tls => {
                out.push(cfg(
                    "TLS",
                    Tipo::Booleano,
                    Some("true"),
                    par((
                        "TLS no soquete; desligado só vale para loopback",
                        "TLS on the socket; off is only allowed for loopback",
                    )),
                ));
                out.push(cfg(
                    "CA",
                    Tipo::Caminho,
                    None,
                    par((
                        "Autoridade (PEM) no lugar das raízes públicas",
                        "Authority (PEM) instead of the public roots",
                    )),
                ));
            }
            K::S((pt, en)) => out.push(Chave {
                chave: chave(k),
                variavel: var(k),
                tipo: Tipo::Texto,
                padrao: None,
                descricao: format!("{pt} ({})", def.canal),
                descricao_en: format!("{en} ({})", def.canal),
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
                motivo_en: None,
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
            descricao: l.d.0.into(),
            descricao_en: l.d.1.into(),
            motivo_en: match l.n {
                N::A((_, en)) => Some(en),
                _ => None,
            },
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
                N::A((motivo, _)) => Natureza::Ambiente { motivo },
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

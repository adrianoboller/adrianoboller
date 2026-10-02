//! ElevenLabs como PROVEDOR dos motores de voz que ja existem: o `speak`
//! (`voz::SpeakTool::falar`) e o `transcribe` (`visao::TranscribeTool::transcrever`), e
//! portanto a conversa `phxclaw voz`, que chama os dois. Nao ha ferramenta paralela de voz
//! da ElevenLabs: o texto se limpa, o WAV se confere e o arquivo entra na pasta pelo MESMO
//! caminho do motor local. So a lista de vozes e ferramenta nova (`voice_list`), porque e
//! uma pergunta que nenhum motor local responde.
//!
//! Conferido na documentacao oficial em 01/10/2026:
//! - fala: `POST /v1/text-to-speech/{voice_id}`, cabecalho `xi-api-key`, `output_format` na
//!   CONSULTA (nao no corpo), corpo com `text`, `model_id` (padrao `eleven_multilingual_v2`)
//!   e `voice_settings` -- https://elevenlabs.io/docs/api-reference/text-to-speech/convert ;
//! - os formatos aceitos incluem `wav_8000`..`wav_48000` e `pcm_8000`..`pcm_48000`; 44,1 kHz
//!   pede plano Pro, por isso o padrao aqui e `wav_16000`, que tambem e o que o whisper ouve
//!   melhor na volta;
//! - transcricao: `POST /v1/speech-to-text`, multipart com `model_id` (a pagina so cita
//!   `scribe_v2`) e `file`; resposta com `text` --
//!   https://elevenlabs.io/docs/api-reference/speech-to-text/convert ;
//! - vozes: a pagina atual e `GET /v2/voices` (o `/v1/voices` saiu da referencia), com
//!   `search` e `page_size` ate 100 -- https://elevenlabs.io/docs/api-reference/voices/search .
//!
//! O que vale saber:
//! - **a chave so mora no broker** (`phxclaw elevenlabs chave`) e so sai por concessao curta;
//!   todo erro passa pela limpeza da `Credencial`;
//! - **`xi-api-key` e cabecalho proprio**: o reqwest so tira `Authorization` ao trocar de
//!   origem, entao o redirecionamento fica desligado e todo 3xx e erro (o `Http` dos canais);
//! - **`voice_settings` so vai quando o operador define**: o campo SOBRESCREVE o ajuste
//!   guardado na voz, e mandar os valores padrao apagaria calado o que a pessoa afinou la.

use crate::canais::http::{Credencial, Http, politica_para};
use crate::chaves::Servico;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub const SERVICO: Servico = Servico {
    espaco: "elevenlabs",
    nome_do_segredo: "elevenlabs-chave",
    chave: "elevenlabs.chave",
    aliases: &["ELEVENLABS_API_KEY"],
    rotulo: "a chave da ElevenLabs",
    comando: "elevenlabs chave",
};

const BASE_PADRAO: &str = "https://api.elevenlabs.io";
/// Audio de fala devolvido: 2.000 caracteres a 48 kHz cabem com folga.
const MAX_AUDIO: usize = 64 * 1024 * 1024;
/// Audio enviado para transcrever: o teto do `transcribe` local e maior, mas aqui o arquivo
/// inteiro vai para a memoria e para a rede.
pub const MAX_AUDIO_ENVIADO: u64 = 200 * 1024 * 1024;
const MAX_JSON: usize = 8 * 1024 * 1024;
const TAXAS: [u32; 7] = [8000, 16000, 22050, 24000, 32000, 44100, 48000];

/// A configuracao, pela chave do catalogo (`elevenlabs.*`).
fn var(chave: &str) -> Option<String> {
    crate::config::texto_de(chave)
}

/// Cliente da API: a base passa pela politica de destinos (HTTP sem TLS so em loopback).
pub struct ElevenLabs {
    http: Http,
    cred: Credencial,
}

/// Uma voz da conta.
#[derive(Debug, Clone, PartialEq)]
pub struct Voz {
    pub id: String,
    pub nome: String,
    pub categoria: String,
}

/// `voice_id` vai no CAMINHO da URL: so letras e digitos, ou `../` mudaria o endpoint.
pub fn voz_valida(v: &str) -> bool {
    (1..=64).contains(&v.len()) && v.chars().all(|c| c.is_ascii_alphanumeric())
}

impl ElevenLabs {
    pub fn novo(base: &str, cred: Credencial) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica_para(base)?)?,
            cred,
        })
    }

    /// Da chave guardada na pasta do agente e da base em `PHXCLAW_ELEVENLABS_API`.
    pub fn da_pasta(raiz_do_agente: &Path) -> Result<Self, String> {
        let base = var("elevenlabs.api").unwrap_or_else(|| BASE_PADRAO.into());
        Self::novo(&base, SERVICO.credencial(raiz_do_agente)?)
    }

    /// Fala: os bytes que a API devolveu, no formato pedido.
    pub fn sintetizar(
        &self,
        voz: &str,
        corpo: &Value,
        formato: &str,
        prazo: Duration,
    ) -> Result<Vec<u8>, String> {
        if !voz_valida(voz) {
            return Err(format!("voice_id invalido: {voz:?}"));
        }
        self.cred.com("send", |chave| {
            let c = self.http.cliente()?;
            self.http.binario(
                c.post(self.http.url(&format!("/v1/text-to-speech/{voz}")))
                    .query(&[("output_format", formato)])
                    .header("xi-api-key", chave)
                    .timeout(prazo)
                    .json(corpo),
                MAX_AUDIO,
            )
        })
    }

    /// Transcricao de um arquivo de audio.
    pub fn transcrever(
        &self,
        audio: Vec<u8>,
        nome: &str,
        modelo: &str,
        idioma: Option<&str>,
        prazo: Duration,
    ) -> Result<String, String> {
        let bytes = self.cred.com("send", |chave| {
            let parte = reqwest::blocking::multipart::Part::bytes(audio)
                .file_name(nome.to_string())
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?;
            let mut form = reqwest::blocking::multipart::Form::new()
                .text("model_id", modelo.to_string())
                .part("file", parte);
            if let Some(l) = idioma {
                form = form.text("language_code", l.to_string());
            }
            let c = self.http.cliente()?;
            self.http.binario(
                c.post(self.http.url("/v1/speech-to-text"))
                    .header("xi-api-key", chave)
                    .timeout(prazo)
                    .multipart(form),
                MAX_JSON,
            )
        })?;
        let v: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("resposta nao e JSON: {e}"))?;
        v.get("text")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| "a transcricao da ElevenLabs veio sem 'text'".into())
    }

    /// As vozes da conta (ate 100), filtradas por `busca` quando ha.
    pub fn vozes(&self, busca: Option<&str>, prazo: Duration) -> Result<Vec<Voz>, String> {
        let bytes = self.cred.com("send", |chave| {
            let mut q: Vec<(&str, &str)> = vec![("page_size", "100")];
            if let Some(b) = busca {
                q.push(("search", b));
            }
            let c = self.http.cliente()?;
            self.http.binario(
                c.get(self.http.url("/v2/voices"))
                    .query(&q)
                    .header("xi-api-key", chave)
                    .timeout(prazo),
                MAX_JSON,
            )
        })?;
        let v: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("resposta nao e JSON: {e}"))?;
        let campo = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok(v.get("voices")
            .and_then(Value::as_array)
            .ok_or("a lista de vozes veio sem 'voices'")?
            .iter()
            .map(|x| Voz {
                id: campo(x, "voice_id"),
                nome: campo(x, "name"),
                categoria: campo(x, "category"),
            })
            .filter(|x| !x.id.is_empty())
            .collect())
    }
}

// ---------------------------------------------------------------- fala

/// Formato pedido a API. Os dois viram WAV para o resto da cadeia: o `wav_*` ja e, o `pcm_*`
/// (16 bits com sinal, little-endian, mono) ganha o cabecalho aqui.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Formato {
    Wav(u32),
    Pcm(u32),
}

impl Formato {
    pub fn ler(s: &str) -> Result<Self, String> {
        let invalido = || {
            format!(
                "PHXCLAW_ELEVENLABS_FORMATO invalido: {s} (wav_16000, pcm_22050...; mp3 e opus \
nao viram WAV sem decodificador)"
            )
        };
        let (cod, taxa) = s.split_once('_').ok_or_else(invalido)?;
        let taxa: u32 = taxa.parse().map_err(|_| invalido())?;
        if !TAXAS.contains(&taxa) {
            return Err(invalido());
        }
        match cod {
            "wav" => Ok(Self::Wav(taxa)),
            "pcm" => Ok(Self::Pcm(taxa)),
            _ => Err(invalido()),
        }
    }

    pub fn nome(&self) -> String {
        match self {
            Self::Wav(t) => format!("wav_{t}"),
            Self::Pcm(t) => format!("pcm_{t}"),
        }
    }
}

/// PCM cru (16 bits, mono) -> WAV.
pub fn wav_de_pcm(pcm: &[u8], taxa: u32) -> Vec<u8> {
    let dados = pcm.len() as u32;
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + dados).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&taxa.to_le_bytes());
    w.extend_from_slice(&(taxa * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&dados.to_le_bytes());
    w.extend_from_slice(pcm);
    w
}

/// O `speak` pela ElevenLabs: voz, modelo, formato e ajustes do operador.
#[derive(Clone)]
pub struct FalaElevenLabs {
    pub cliente: Arc<ElevenLabs>,
    pub voz: String,
    pub modelo: String,
    pub formato: Formato,
    /// `voice_settings`; `None` deixa valer o ajuste guardado na voz.
    pub ajustes: Option<Value>,
}

impl FalaElevenLabs {
    /// `PHXCLAW_ELEVENLABS_VOZ` (obrigatoria), `_MODELO`, `_FORMATO`, `_ESTABILIDADE` e
    /// `_SIMILARIDADE`. Erro diz o que falta.
    pub fn do_ambiente(raiz_do_agente: &Path) -> Result<Self, String> {
        let var_voz = crate::config::variavel("elevenlabs.voz");
        let voz = var("elevenlabs.voz").ok_or_else(|| {
            format!(
                "{}=elevenlabs pede {var_voz} (o voice_id; `phxclaw elevenlabs vozes` lista \
os da conta)",
                crate::config::variavel("voz.tts.provedor")
            )
        })?;
        if !voz_valida(&voz) {
            return Err(format!("{var_voz} invalida: {voz:?}"));
        }
        let formato =
            Formato::ler(&var("elevenlabs.formato").unwrap_or_else(|| "wav_16000".into()))?;
        let mut ajustes = serde_json::Map::new();
        for (chave, campo) in [
            ("elevenlabs.estabilidade", "stability"),
            ("elevenlabs.similaridade", "similarity_boost"),
        ] {
            let v = crate::config::variavel(chave);
            if let Some(s) = var(chave) {
                let x: f64 = s
                    .trim()
                    .parse()
                    .ok()
                    .filter(|x| (0.0..=1.0).contains(x))
                    .ok_or(format!("{v} tem de ser numero de 0 a 1: {s}"))?;
                ajustes.insert(campo.into(), json!(x));
            }
        }
        Ok(Self {
            cliente: Arc::new(ElevenLabs::da_pasta(raiz_do_agente)?),
            voz,
            modelo: var("elevenlabs.modelo").unwrap_or_else(|| "eleven_multilingual_v2".into()),
            formato,
            ajustes: (!ajustes.is_empty()).then_some(Value::Object(ajustes)),
        })
    }

    /// Texto (ja limpo pelo `speak`) -> WAV. `voz` troca a voz do operador por uma chamada.
    pub fn falar(
        &self,
        texto: &str,
        voz: Option<&str>,
        prazo: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut corpo = json!({"text": texto, "model_id": self.modelo});
        if let Some(a) = &self.ajustes {
            corpo["voice_settings"] = a.clone();
        }
        let b = self.cliente.sintetizar(
            voz.unwrap_or(&self.voz),
            &corpo,
            &self.formato.nome(),
            prazo,
        )?;
        Ok(match self.formato {
            Formato::Wav(_) => b,
            Formato::Pcm(t) => wav_de_pcm(&b, t),
        })
    }
}

// ---------------------------------------------------------------- transcricao

/// O `transcribe` pela ElevenLabs.
#[derive(Clone)]
pub struct OuvidoElevenLabs {
    pub cliente: Arc<ElevenLabs>,
    pub modelo: String,
}

impl OuvidoElevenLabs {
    /// `PHXCLAW_ELEVENLABS_STT_MODELO`, padrao `scribe_v2`.
    pub fn do_ambiente(raiz_do_agente: &Path) -> Result<Self, String> {
        Ok(Self {
            cliente: Arc::new(ElevenLabs::da_pasta(raiz_do_agente)?),
            modelo: var("elevenlabs.stt_modelo").unwrap_or_else(|| "scribe_v2".into()),
        })
    }

    pub fn transcrever(
        &self,
        audio: &Path,
        idioma: Option<&str>,
        prazo: Duration,
    ) -> Result<String, String> {
        let tam = std::fs::metadata(audio).map_err(|e| e.to_string())?.len();
        if tam > MAX_AUDIO_ENVIADO {
            return Err(format!(
                "audio com {tam} bytes, acima do teto de {MAX_AUDIO_ENVIADO} para enviar"
            ));
        }
        let bytes = std::fs::read(audio).map_err(|e| e.to_string())?;
        // "auto" e o idioma do whisper; a ElevenLabs detecta quando o campo nao vem.
        let idioma = idioma.filter(|l| *l != "auto");
        self.cliente
            .transcrever(bytes, "audio.wav", &self.modelo, idioma, prazo)
    }
}

// ---------------------------------------------------------------- vozes

/// `voice_list`: as vozes da conta, para escolher o `voice` do `speak`. So existe com a
/// chave guardada; `media.voices` e leitura (nada muda na conta), fora do padrao.
pub struct VoiceListTool {
    pub cliente: Arc<ElevenLabs>,
}

impl VoiceListTool {
    /// Sem chave guardada a ferramenta nem existe, como o `x_search`.
    pub fn da_pasta(raiz_do_agente: &Path) -> Option<Self> {
        ElevenLabs::da_pasta(raiz_do_agente).ok().map(|c| Self {
            cliente: Arc::new(c),
        })
    }
}

/// Uma voz por linha: `id  nome (categoria)`.
pub fn listar(vozes: &[Voz]) -> String {
    if vozes.is_empty() {
        return "nenhuma voz encontrada".into();
    }
    vozes
        .iter()
        .map(|v| format!("{}  {} ({})", v.id, v.nome, v.categoria))
        .collect::<Vec<_>>()
        .join("\n")
}

impl phxclaw_agent_core::Tool for VoiceListTool {
    fn spec(&self) -> phxclaw_agent_core::ToolSpec {
        phxclaw_agent_core::ToolSpec {
            name: "voice_list".into(),
            description: "List the ElevenLabs voices of the account (voice_id, name, category), \
optionally filtered by 'search'. Pass a voice_id as 'voice' to speak."
                .into(),
            parameters: json!({"type":"object","properties":{
                "search":{"type":"string"}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "media.voices"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a phxclaw_agent_core::ToolContext,
    ) -> phxclaw_agent_core::BoxFut<
        'a,
        Result<phxclaw_agent_core::ToolOutput, phxclaw_agent_core::ToolError>,
    > {
        use phxclaw_agent_core::{ToolError, ToolOutput};
        Box::pin(async move {
            let busca = args
                .get("search")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(100).collect::<String>());
            let (c, prazo) = (self.cliente.clone(), ctx.timeout);
            let v = tokio::task::spawn_blocking(move || c.vozes(busca.as_deref(), prazo))
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(|e| ToolError::Failed(format!("voice_list: {e}")))?;
            Ok(ToolOutput::text(listar(&v)))
        })
    }
}

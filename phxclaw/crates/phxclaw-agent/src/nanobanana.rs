//! Nano Banana (imagem nativa do Gemini) como PROVEDOR do `image_generate` -- uma variante
//! do `midia::Provedor`, nao uma ferramenta paralela: o prompt, o tamanho, a saida confinada e
//! a conferencia do PNG sao os do `image_generate` de sempre. O que ele traz de novo e a
//! EDICAO: imagem de entrada + instrucao, e a imagem de entrada sai da pasta da tarefa.
//!
//! Conferido na documentacao oficial em 01/10/2026:
//! - `POST /v1beta/models/{modelo}:generateContent`, partes `text` e `inline_data`
//!   (`mime_type`, `data` em base64) no pedido; a imagem volta em
//!   `candidates[].content.parts[].inlineData` -- https://ai.google.dev/api/generate-content ;
//! - `generationConfig.imageConfig.aspectRatio` aceita 1:1, 1:4, 4:1, 1:8, 8:1, 2:3, 3:2, 3:4,
//!   4:3, 4:5, 5:4, 9:16, 16:9 e 21:9 (mesma pagina, `ImageConfig`);
//! - os modelos: `gemini-2.5-flash-image` (o Nano Banana pedido, estavel e marcado como
//!   legado), `gemini-3.1-flash-image` (Nano Banana 2), `gemini-3.1-flash-lite-image` e
//!   `gemini-3-pro-image` -- https://ai.google.dev/gemini-api/docs/image-generation . O padrao
//!   e o pedido pelo dono; `PHXCLAW_IMAGEM_MODELO` troca.
//!
//! A chave e cabecalho proprio (`x-goog-api-key`), que o reqwest NAO tira num redirecionamento:
//! o pedido sai pelo `Http` dos canais (redirecionamento desligado, 3xx e erro) e a chave so
//! existe dentro da concessao da `Credencial`.

use crate::canais::http::{Credencial, Http, politica_para};
use crate::chaves::Servico;
use base64::Engine;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub const SERVICO: Servico = Servico {
    espaco: "gemini",
    nome_do_segredo: "gemini-chave",
    chave: "gemini.chave",
    aliases: &["GEMINI_API_KEY"],
    rotulo: "a chave da Gemini API",
    comando: "gemini chave",
};

const BASE_PADRAO: &str = "https://generativelanguage.googleapis.com";
pub const MODELO_PADRAO: &str = "gemini-2.5-flash-image";
/// A imagem volta em base64 dentro do JSON: ~4/3 do teto de imagem do `image_generate`.
const MAX_RESPOSTA: usize = 48 * 1024 * 1024;
/// Imagem de entrada: o pedido inline da Gemini API tem teto de 20 MB no total, e o base64
/// cresce 4/3.
pub const MAX_ENTRADA: u64 = 14 * 1024 * 1024;
const PROPORCOES: [(u32, u32); 14] = [
    (1, 1),
    (1, 4),
    (4, 1),
    (1, 8),
    (8, 1),
    (2, 3),
    (3, 2),
    (3, 4),
    (4, 3),
    (4, 5),
    (5, 4),
    (9, 16),
    (16, 9),
    (21, 9),
];

pub struct NanoBanana {
    http: Http,
    cred: Credencial,
    modelo: String,
}

impl std::fmt::Debug for NanoBanana {
    /// So a base e o modelo: a credencial nao tem o que mostrar, e nao deve.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NanoBanana")
            .field("base", &self.http.base())
            .field("modelo", &self.modelo)
            .finish()
    }
}

/// O modelo vai no caminho da URL.
fn modelo_valido(m: &str) -> bool {
    (1..=80).contains(&m.len())
        && m.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

/// Tipo da imagem pelos primeiros bytes, nao pela extensao.
pub fn tipo_da_imagem(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// `WxH` -> proporcao suportada, ou erro com a lista.
pub fn proporcao(w: u32, h: u32) -> Result<String, String> {
    let g = {
        let (mut a, mut b) = (w, h);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let (x, y) = (w / g, h / g);
    if PROPORCOES.contains(&(x, y)) {
        Ok(format!("{x}:{y}"))
    } else {
        Err(format!(
            "o Nano Banana nao gera {w}x{h} ({x}:{y}); proporcoes aceitas: {}",
            PROPORCOES
                .iter()
                .map(|(a, b)| format!("{a}:{b}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// O corpo do `generateContent`. `entrada` e (tipo, bytes) da imagem a editar.
pub fn montar_pedido(
    prompt: &str,
    entrada: Option<(&str, &[u8])>,
    proporcao: Option<&str>,
) -> Value {
    let mut partes = vec![json!({"text": prompt})];
    if let Some((tipo, b)) = entrada {
        partes.push(json!({"inline_data": {
            "mime_type": tipo,
            "data": base64::engine::general_purpose::STANDARD.encode(b),
        }}));
    }
    let mut config = json!({"responseModalities": ["TEXT", "IMAGE"]});
    if let Some(p) = proporcao {
        config["imageConfig"] = json!({"aspectRatio": p});
    }
    json!({"contents": [{"role": "user", "parts": partes}], "generationConfig": config})
}

/// A primeira imagem da resposta, decodificada, com o tipo dela. Sem imagem, o erro traz o
/// motivo que a API deu (bloqueio, fim) e o comeco do texto, em vez de «sem imagem» seco.
pub fn ler_resposta(v: &Value) -> Result<(String, Vec<u8>), String> {
    let mut textos = Vec::new();
    for c in v["candidates"].as_array().into_iter().flatten() {
        for p in c["content"]["parts"].as_array().into_iter().flatten() {
            let dado = p.get("inlineData").or_else(|| p.get("inline_data"));
            if let Some(d) = dado {
                let tipo = d
                    .get("mimeType")
                    .or_else(|| d.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let b64 = d.get("data").and_then(Value::as_str).unwrap_or("");
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| format!("inlineData.data invalido: {e}"))?;
                return Ok((tipo, bytes));
            }
            if let Some(t) = p.get("text").and_then(Value::as_str) {
                textos.push(t.to_string());
            }
        }
    }
    let motivo = v
        .pointer("/promptFeedback/blockReason")
        .or_else(|| v.pointer("/candidates/0/finishReason"))
        .and_then(Value::as_str)
        .unwrap_or("sem motivo");
    let texto: String = textos.join(" ").chars().take(300).collect();
    Err(format!(
        "o Nano Banana nao devolveu imagem ({motivo}){}",
        if texto.is_empty() {
            String::new()
        } else {
            format!(": {texto}")
        }
    ))
}

impl NanoBanana {
    pub fn novo(base: &str, modelo: &str, cred: Credencial) -> Result<Self, String> {
        if !modelo_valido(modelo) {
            return Err(format!("modelo invalido: {modelo:?}"));
        }
        Ok(Self {
            http: Http::novo(base, politica_para(base)?)?,
            cred,
            modelo: modelo.into(),
        })
    }

    /// Base em `PHXCLAW_IMAGEM_URL` (a mesma variavel dos outros provedores de imagem) e
    /// modelo em `PHXCLAW_IMAGEM_MODELO`.
    pub fn da_pasta(raiz_do_agente: &Path) -> Result<Self, String> {
        let var = crate::config::texto_de;
        Self::novo(
            &var("imagem.url").unwrap_or_else(|| BASE_PADRAO.into()),
            &var("imagem.modelo").unwrap_or_else(|| MODELO_PADRAO.into()),
            SERVICO.credencial(raiz_do_agente)?,
        )
    }

    pub fn modelo(&self) -> &str {
        &self.modelo
    }

    /// Gera (sem `entrada`) ou edita (com ela). Devolve o tipo e os bytes da imagem.
    pub fn gerar(
        &self,
        prompt: &str,
        entrada: Option<(&str, &[u8])>,
        proporcao: Option<&str>,
        prazo: Duration,
    ) -> Result<(String, Vec<u8>), String> {
        let corpo = montar_pedido(prompt, entrada, proporcao);
        let bytes = self.cred.com("send", |chave| {
            let c = self.http.cliente()?;
            self.http.binario(
                c.post(
                    self.http
                        .url(&format!("/v1beta/models/{}:generateContent", self.modelo)),
                )
                .header("x-goog-api-key", chave)
                .timeout(prazo)
                .json(&corpo),
                MAX_RESPOSTA,
            )
        })?;
        let v: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("resposta nao e JSON: {e}"))?;
        ler_resposta(&v)
    }
}

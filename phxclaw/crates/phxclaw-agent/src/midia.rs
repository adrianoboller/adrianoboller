//! Geracao de imagem: cliente da API de imagens no formato OpenAI e da API do ComfyUI, e,
//! sem servidor de imagem, o SVG que o proprio modelo escreve desenhado pelo Chromium.
//!
//! Decisoes que valem saber:
//! - **o endereco e a chave sao do operador, nunca do modelo** (`PHXCLAW_IMAGEM_*`): o
//!   modelo so escreve o pedido. Um campo de URL na ferramenta seria uma porta de SSRF;
//! - **redirecionamento desligado**: o servidor configurado nao manda o cliente (e a chave
//!   no cabecalho) para outro lugar;
//! - **o fluxo do ComfyUI se preenche na arvore JSON, nao no texto**: o prompt com aspas
//!   viraria JSON quebrado -- ou um no novo no fluxo -- se fosse colado no arquivo;
//! - **o caminho local nao duplica o desenho**: o SVG vai para o `image_render` de sempre;
//! - **o Nano Banana e mais um provedor, nao outra ferramenta** (`nanobanana.rs`): a chave
//!   so no SecretBroker, e a EDICAO (`input_image`) le a imagem da pasta da tarefa,
//!   confinada -- caminho fora dela nem chega a ser lido.

use crate::motor::artifact_for;
use crate::nanobanana::{NanoBanana, proporcao, tipo_da_imagem};
use crate::tarefa::confine;
use crate::visao::{ImageRenderTool, info_png, info_svg};
use base64::Engine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Imagem maior que isto nao se aceita do servidor: a resposta vai inteira para a memoria.
const MAX_IMAGEM: usize = 32 * 1024 * 1024;
const MAX_PROMPT: usize = 4_000;
const MAX_SVG: usize = 512 * 1024;

#[derive(Debug, Clone)]
pub enum Provedor {
    /// `POST {url}/v1/images/generations`, resposta em `b64_json`.
    OpenAi {
        url: String,
        chave: Option<String>,
        modelo: String,
    },
    /// `POST {url}/prompt`, `GET {url}/history/<id>`, `GET {url}/view`.
    ComfyUi { url: String, fluxo: PathBuf },
    /// `POST {url}/v1beta/models/{modelo}:generateContent`. `Err`: o operador escolheu o
    /// Nano Banana e ele nao esta disponivel (sem chave); a ferramenta recusa dizendo isso,
    /// em vez de cair calada no SVG.
    NanoBanana(Result<Arc<NanoBanana>, String>),
}

pub struct ImageGenerateTool {
    pub provedor: Option<Provedor>,
}

/// A configuracao, pela chave do catalogo (`imagem.*`); a variavel citada na mensagem sai
/// de `config::variavel`.
fn var(chave: &str) -> Option<String> {
    crate::config::texto_de(chave)
}

fn pede(provedor: &str, chave: &str) -> String {
    format!(
        "{}={provedor} pede {}",
        crate::config::variavel("imagem.provedor"),
        crate::config::variavel(chave)
    )
}

fn falha(e: impl std::fmt::Display) -> ToolError {
    ToolError::Failed(e.to_string())
}

impl ImageGenerateTool {
    /// `PHXCLAW_IMAGEM_PROVEDOR` = `openai` (com `PHXCLAW_IMAGEM_URL`, `PHXCLAW_IMAGEM_CHAVE`
    /// ou a chave de `phxclaw imagem chave` no broker, `PHXCLAW_IMAGEM_MODELO`), `comfyui` (com `PHXCLAW_IMAGEM_URL` e
    /// `PHXCLAW_COMFY_WORKFLOW`) ou `nanobanana` (chave de `phxclaw gemini chave` no broker
    /// de `raiz_do_agente`; `PHXCLAW_IMAGEM_URL` e `PHXCLAW_IMAGEM_MODELO` opcionais). Sem
    /// nada: so o SVG local.
    pub fn do_ambiente(raiz_do_agente: &Path) -> Result<Self, String> {
        let provedor = match var("imagem.provedor").as_deref() {
            None => None,
            Some("nanobanana") => Some(Provedor::NanoBanana(
                NanoBanana::da_pasta(raiz_do_agente).map(Arc::new),
            )),
            Some("openai") => Some(Provedor::OpenAi {
                url: var("imagem.url").unwrap_or_else(|| "https://api.openai.com".into()),
                // Ambiente, senao o broker de `phxclaw imagem chave`.
                chave: crate::chaves::IMAGEM.do_ambiente_ou_broker(raiz_do_agente)?,
                modelo: var("imagem.modelo").unwrap_or_else(|| "gpt-image-1".into()),
            }),
            Some("comfyui") => Some(Provedor::ComfyUi {
                url: var("imagem.url").ok_or_else(|| pede("comfyui", "imagem.url"))?,
                fluxo: var("imagem.comfy_fluxo")
                    .map(PathBuf::from)
                    .ok_or_else(|| pede("comfyui", "imagem.comfy_fluxo"))?,
            }),
            Some(o) => {
                return Err(format!(
                    "{} desconhecido: {o}",
                    crate::config::variavel("imagem.provedor")
                ));
            }
        };
        Ok(Self { provedor })
    }
}

fn cliente(prazo: Duration) -> Result<reqwest::Client, ToolError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(prazo)
        .build()
        .map_err(falha)
}

/// Erro HTTP sem o corpo inteiro (pode ser uma pagina) e sem eco da chave.
async fn conferir_status(r: reqwest::Response, onde: &str) -> Result<reqwest::Response, ToolError> {
    if r.status().is_success() {
        return Ok(r);
    }
    let st = r.status();
    let corpo = r.text().await.unwrap_or_default();
    let curto: String = corpo.chars().take(300).collect();
    Err(ToolError::Failed(format!("{onde}: HTTP {st}: {curto}")))
}

async fn bytes_com_teto(r: reqwest::Response) -> Result<Vec<u8>, ToolError> {
    if r.content_length().is_some_and(|n| n as usize > MAX_IMAGEM) {
        return Err(ToolError::Failed(format!(
            "resposta acima do teto de {MAX_IMAGEM} bytes"
        )));
    }
    let b = r.bytes().await.map_err(falha)?;
    if b.len() > MAX_IMAGEM {
        return Err(ToolError::Failed(format!(
            "resposta acima do teto de {MAX_IMAGEM} bytes"
        )));
    }
    Ok(b.to_vec())
}

fn base(url: &str) -> &str {
    url.trim_end_matches('/')
}

async fn gerar_openai(
    url: &str,
    chave: Option<&str>,
    modelo: &str,
    prompt: &str,
    tamanho: &str,
    prazo: Duration,
) -> Result<Vec<u8>, ToolError> {
    let mut corpo = json!({"model": modelo, "prompt": prompt, "n": 1, "size": tamanho});
    // Os modelos dall-e devolvem URL por padrao; seguir uma URL que veio na resposta seria
    // buscar onde o servidor mandar. Os gpt-image so devolvem base64 e recusam o campo.
    if modelo.starts_with("dall-e") {
        corpo["response_format"] = json!("b64_json");
    }
    let mut req = cliente(prazo)?
        .post(format!("{}/v1/images/generations", base(url)))
        .json(&corpo);
    if let Some(c) = chave {
        req = req.bearer_auth(c);
    }
    let r = conferir_status(req.send().await.map_err(falha)?, "api de imagens").await?;
    let v: Value = serde_json::from_slice(&bytes_com_teto(r).await?).map_err(falha)?;
    let b64 = v
        .pointer("/data/0/b64_json")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::Failed("a api de imagens nao devolveu data[0].b64_json".into())
        })?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| ToolError::Failed(format!("b64_json invalido: {e}")))
}

/// Troca os marcadores nas FOLHAS de texto do fluxo. `{{Prompt}}` pode estar no meio de um
/// texto; `{{Seed}}`, `{{Width}}` e `{{Height}}` so como valor inteiro, e viram numero.
pub fn preencher_fluxo(
    v: &mut Value,
    prompt: &str,
    largura: u32,
    altura: u32,
    semente: u64,
) -> usize {
    let mut n = 0;
    match v {
        Value::String(s) => {
            let numero = match s.as_str() {
                "{{Seed}}" => Some(json!(semente)),
                "{{Width}}" => Some(json!(largura)),
                "{{Height}}" => Some(json!(altura)),
                _ => None,
            };
            if let Some(x) = numero {
                *v = x;
                n += 1;
            } else if s.contains("{{Prompt}}") {
                *s = s.replace("{{Prompt}}", prompt);
                n += 1;
            }
        }
        Value::Array(a) => {
            for x in a {
                n += preencher_fluxo(x, prompt, largura, altura, semente);
            }
        }
        Value::Object(o) => {
            for x in o.values_mut() {
                n += preencher_fluxo(x, prompt, largura, altura, semente);
            }
        }
        _ => {}
    }
    n
}

async fn gerar_comfy(
    url: &str,
    fluxo: &Path,
    prompt: &str,
    (largura, altura): (u32, u32),
    prazo: Duration,
) -> Result<Vec<u8>, ToolError> {
    let fim = Instant::now() + prazo;
    let texto = std::fs::read_to_string(fluxo)
        .map_err(|e| ToolError::Denied(format!("PHXCLAW_COMFY_WORKFLOW ilegivel: {e}")))?;
    let mut wf: Value = serde_json::from_str(&texto)
        .map_err(|e| ToolError::Denied(format!("PHXCLAW_COMFY_WORKFLOW nao e JSON: {e}")))?;
    // Semente nova a cada pedido: o mesmo prompt duas vezes deve dar duas imagens.
    let semente = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 & 0xFFFF_FFFF)
        .unwrap_or(0);
    if !texto.contains("{{Prompt}}") {
        return Err(ToolError::Denied(
            "o fluxo do ComfyUI nao tem o marcador {{Prompt}}".into(),
        ));
    }
    preencher_fluxo(&mut wf, prompt, largura, altura, semente);
    let c = cliente(prazo)?;
    let r = c
        .post(format!("{}/prompt", base(url)))
        .json(&json!({"prompt": wf, "client_id": phxclaw_types::new_uuid_v7()}))
        .send()
        .await
        .map_err(falha)?;
    let r = conferir_status(r, "comfyui /prompt").await?;
    let v: Value = serde_json::from_slice(&bytes_com_teto(r).await?).map_err(falha)?;
    let id = v
        .get("prompt_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .ok_or_else(|| ToolError::Failed("comfyui nao devolveu prompt_id valido".into()))?
        .to_string();
    // O ComfyUI enfileira e responde na hora; a imagem aparece no historico quando acaba.
    let imagem = loop {
        if Instant::now() >= fim {
            return Err(ToolError::Timeout(prazo.as_millis() as u64));
        }
        let r = c
            .get(format!("{}/history/{id}", base(url)))
            .send()
            .await
            .map_err(falha)?;
        let h: Value = serde_json::from_slice(
            &bytes_com_teto(conferir_status(r, "comfyui /history").await?).await?,
        )
        .map_err(falha)?;
        if let Some(st) = h
            .pointer(&format!("/{id}/status/status_str"))
            .and_then(Value::as_str)
            && st == "error"
        {
            return Err(ToolError::Failed(
                "comfyui terminou o fluxo com erro".into(),
            ));
        }
        let achada = h
            .pointer(&format!("/{id}/outputs"))
            .and_then(Value::as_object)
            .and_then(|o| {
                o.values()
                    .filter_map(|n| n.get("images").and_then(Value::as_array))
                    .flatten()
                    .next()
                    .cloned()
            });
        if let Some(i) = achada {
            break i;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    let campo = |k: &str| {
        imagem
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let r = c
        .get(format!("{}/view", base(url)))
        .query(&[
            ("filename", campo("filename")),
            ("subfolder", campo("subfolder")),
            ("type", campo("type")),
        ])
        .send()
        .await
        .map_err(falha)?;
    bytes_com_teto(conferir_status(r, "comfyui /view").await?).await
}

/// A imagem a editar: confinada a pasta da tarefa, com teto, e o tipo pelos bytes.
fn ler_entrada(workdir: &Path, rel: &str) -> Result<(&'static str, Vec<u8>), ToolError> {
    let p = confine(workdir, rel).map_err(ToolError::Denied)?;
    let tam = std::fs::metadata(&p)
        .map_err(|_| {
            ToolError::InvalidArguments(format!("imagem nao encontrada na pasta da tarefa: {rel}"))
        })?
        .len();
    if tam > crate::nanobanana::MAX_ENTRADA {
        return Err(ToolError::InvalidArguments(format!(
            "{rel} tem {tam} bytes, acima do teto de {}",
            crate::nanobanana::MAX_ENTRADA
        )));
    }
    let b = std::fs::read(&p).map_err(falha)?;
    let tipo = tipo_da_imagem(&b)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{rel} nao e PNG, JPEG nem WebP")))?;
    Ok((tipo, b))
}

fn tamanho(args: &Value) -> Result<(u32, u32), ToolError> {
    let s = args
        .get("size")
        .and_then(Value::as_str)
        .unwrap_or("1024x1024");
    let (a, b) = s.split_once('x').ok_or_else(|| {
        ToolError::InvalidArguments(format!("size invalido: {s} (ex. 1024x1024)"))
    })?;
    let p = |x: &str| {
        x.trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (64..=4096).contains(n))
    };
    match (p(a), p(b)) {
        (Some(w), Some(h)) => Ok((w, h)),
        _ => Err(ToolError::InvalidArguments(format!(
            "size invalido: {s} (64 a 4096 por lado)"
        ))),
    }
}

impl Tool for ImageGenerateTool {
    fn spec(&self) -> ToolSpec {
        let modo = match &self.provedor {
            Some(Provedor::OpenAi { .. }) => "an OpenAI-compatible image API",
            Some(Provedor::ComfyUi { .. }) => "a ComfyUI server",
            Some(Provedor::NanoBanana(_)) => {
                "Nano Banana (Gemini image); it can also EDIT: pass 'input_image' (a .png, .jpg \
or .webp of the task folder) and say in 'prompt' what to change"
            }
            None => "no image server configured: pass 'svg' markup",
        };
        ToolSpec {
            name: "image_generate".into(),
            description: format!(
                "Generate a .png image in the task folder. From 'prompt' via {modo}; or, \
always available, from 'svg' markup you write, drawn by headless Chromium."
            ),
            parameters: json!({"type":"object","properties":{
                "prompt":{"type":"string"},
                "svg":{"type":"string","description":"complete <svg> document; drawn locally"},
                "output":{"type":"string","description":".png path, default imagem.png"},
                "size":{"type":"string","description":"WxH, default 1024x1024"},
                "input_image":{"type":"string","description":"image of the task folder to edit (Nano Banana only)"}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "media.generate"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let saida = args
                .get("output")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("imagem.png")
                .to_string();
            if !saida.to_ascii_lowercase().ends_with(".png") {
                return Err(ToolError::InvalidArguments(format!(
                    "a saida tem de terminar em .png: {saida}"
                )));
            }
            let alvo = confine(&ctx.workdir, &saida).map_err(ToolError::Denied)?;
            let (w, h) = tamanho(&args)?;
            if let Some(svg) = args.get("svg").and_then(Value::as_str) {
                if svg.len() > MAX_SVG {
                    return Err(ToolError::InvalidArguments(format!(
                        "svg com {} bytes, acima do teto de {MAX_SVG}",
                        svg.len()
                    )));
                }
                info_svg(svg).map_err(|e| ToolError::InvalidArguments(format!("svg: {e}")))?;
                let fonte = Path::new(&saida)
                    .with_extension("svg")
                    .to_string_lossy()
                    .into_owned();
                let f = confine(&ctx.workdir, &fonte).map_err(ToolError::Denied)?;
                if let Some(d) = f.parent() {
                    std::fs::create_dir_all(d).map_err(falha)?;
                }
                std::fs::write(&f, svg).map_err(falha)?;
                let mut pedido = json!({"source": fonte, "output": saida});
                if args.get("size").is_some() {
                    pedido["width"] = json!(w);
                    pedido["height"] = json!(h);
                }
                return ImageRenderTool.run(pedido, ctx).await;
            }
            let prompt = args
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("informe 'prompt' ou 'svg'".into()))?;
            if prompt.chars().count() > MAX_PROMPT {
                return Err(ToolError::InvalidArguments(format!(
                    "prompt acima de {MAX_PROMPT} caracteres"
                )));
            }
            let entrada = args
                .get("input_image")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            if entrada.is_some() && !matches!(self.provedor, Some(Provedor::NanoBanana(_))) {
                return Err(ToolError::InvalidArguments(
                    "'input_image' (edicao) so com PHXCLAW_IMAGEM_PROVEDOR=nanobanana".into(),
                ));
            }
            let bytes = match &self.provedor {
                None => {
                    return Err(ToolError::Denied(
                        "nenhum servidor de imagem configurado (PHXCLAW_IMAGEM_PROVEDOR); \
desenhe com 'svg'"
                            .into(),
                    ));
                }
                Some(Provedor::OpenAi { url, chave, modelo }) => {
                    gerar_openai(
                        url,
                        chave.as_deref(),
                        modelo,
                        prompt,
                        &format!("{w}x{h}"),
                        ctx.timeout,
                    )
                    .await?
                }
                Some(Provedor::ComfyUi { url, fluxo }) => {
                    gerar_comfy(url, fluxo, prompt, (w, h), ctx.timeout).await?
                }
                Some(Provedor::NanoBanana(nb)) => {
                    let nb = nb.clone().map_err(ToolError::Denied)?;
                    let entrada = match entrada {
                        Some(rel) => Some(ler_entrada(&ctx.workdir, rel)?),
                        None => None,
                    };
                    // Sem 'size' a proporcao e a do modelo -- na edicao, a da imagem de
                    // entrada; mandar 1:1 por padrao quadraria toda foto editada.
                    let prop = match args.get("size") {
                        Some(_) => Some(proporcao(w, h).map_err(ToolError::InvalidArguments)?),
                        None => None,
                    };
                    let (prompt, prazo) = (prompt.to_string(), ctx.timeout);
                    let (tipo, b) = tokio::task::spawn_blocking(move || {
                        nb.gerar(
                            &prompt,
                            entrada.as_ref().map(|(t, b)| (*t, b.as_slice())),
                            prop.as_deref(),
                            prazo,
                        )
                    })
                    .await
                    .map_err(falha)?
                    .map_err(|e| ToolError::Failed(format!("nano banana: {e}")))?;
                    if tipo_da_imagem(&b) != Some("image/png") {
                        return Err(ToolError::Failed(format!(
                            "o Nano Banana devolveu {tipo:?}, nao PNG; so PNG entra na pasta"
                        )));
                    }
                    b
                }
            };
            // So PNG entra na pasta: o que o servidor mandou e conferido pelo cabecalho, e
            // um HTML de erro com status 200 nao vira "imagem.png".
            let (pw, ph, _) = info_png(&bytes)
                .map_err(|e| ToolError::Failed(format!("o servidor nao devolveu PNG: {e}")))?;
            if let Some(d) = alvo.parent() {
                std::fs::create_dir_all(d).map_err(falha)?;
            }
            std::fs::write(&alvo, &bytes).map_err(falha)?;
            Ok(ToolOutput {
                content: format!("imagem gerada em {saida} ({pw}x{ph} px)"),
                artifacts: vec![artifact_for(&ctx.workdir, &saida).map_err(falha)?],
            })
        })
    }
}

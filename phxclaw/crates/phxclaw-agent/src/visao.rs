//! Visao, desktop e voz como ferramentas do agente: OCR de imagem e de PDF, informacao e
//! conversao de imagem, controle do desktop e fala para texto.
//!
//! Cada ferramenta recusa com o motivo quando falta o recurso (binario, DISPLAY, variavel)
//! em vez de devolver texto vazio: OCR vazio dado como sucesso diria "nao ha texto" quando
//! a verdade e "nao li".

use crate::motor::{artifact_for, media_type};
use crate::tarefa::confine;
use phxclaw_agent_core::{
    BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Texto devolvido ao modelo: um PDF de cem paginas inteiro estouraria o contexto.
const MAX_TEXTO: usize = 20_000;
/// Arquivo maior que isto nao e lido para a memoria so para achar um cabecalho.
const MAX_ARQUIVO: u64 = 64 * 1024 * 1024;

fn texto<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn falha(e: impl std::fmt::Display) -> ToolError {
    ToolError::Failed(e.to_string())
}

fn extensao(p: &str) -> String {
    Path::new(p)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Arquivo da tarefa que tem de existir: o erro diz o caminho que o modelo pediu, e nao o
/// caminho absoluto da maquina.
fn arquivo_da_tarefa(ctx: &ToolContext, rel: &str) -> Result<PathBuf, ToolError> {
    let p = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
    if !p.is_file() {
        return Err(ToolError::InvalidArguments(format!(
            "arquivo nao encontrado na pasta da tarefa: {rel}"
        )));
    }
    let tam = std::fs::metadata(&p).map_err(falha)?.len();
    if tam > MAX_ARQUIVO {
        return Err(ToolError::InvalidArguments(format!(
            "{rel} tem {tam} bytes, acima do teto de {MAX_ARQUIVO}"
        )));
    }
    Ok(p)
}

/// Pasta temporaria que se apaga ao sair do escopo, inclusive no erro e no estouro do
/// prazo: as paginas renderizadas de um PDF grande somam centenas de MB.
struct PastaTemp(PathBuf);

impl PastaTemp {
    fn nova(prefixo: &str) -> Result<Self, ToolError> {
        let d = std::env::temp_dir().join(format!("{prefixo}-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).map_err(falha)?;
        Ok(Self(d))
    }
}

impl Drop for PastaTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------- OCR

/// O tesseract do agente: um motor so, usado pelo `ocr` e pelo `screenshot_to_erp_ui`, para
/// que a mensagem de instalacao e o corte por prazo nao divirjam entre os dois. O prazo vai
/// ao processo: `kill_on_drop` mata o filho quando o `timeout` solta o futuro.
pub async fn tesseract(
    imagem: &Path,
    idiomas: &str,
    psm: Option<u8>,
    prazo: Duration,
) -> Result<String, ToolError> {
    let mut cmd = tokio::process::Command::new("tesseract");
    cmd.arg(imagem).args(["-", "-l", idiomas]);
    if let Some(p) = psm {
        cmd.arg("--psm").arg(p.to_string());
    }
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    let saida = tokio::time::timeout(prazo, cmd.output())
        .await
        .map_err(|_| ToolError::Timeout(prazo.as_millis() as u64))?
        .map_err(|e| {
            ToolError::Failed(format!(
                "tesseract nao executou ({e}); instale tesseract-ocr e tesseract-ocr-por"
            ))
        })?;
    if !saida.status.success() {
        return Err(ToolError::Failed(format!(
            "tesseract falhou: {}",
            String::from_utf8_lossy(&saida.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&saida.stdout).into_owned())
}

/// Idiomas do tesseract no formato dele ("por+eng"). So letras, digitos e `_`: o valor vai
/// para a linha de comando, e um "-" inicial viraria opcao.
fn idiomas_validos(s: &str) -> bool {
    s.split('+').all(|l| {
        !l.is_empty()
            && l.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

pub struct OcrTool;

impl Tool for OcrTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "ocr".into(),
            description: "Extract the text of an image (.png .jpg .jpeg .tif .tiff) or of a .pdf \
in the task folder by OCR (tesseract; PDF pages rendered at 200 dpi). Default language por+eng."
                .into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "lang":{"type":"string","description":"tesseract languages, e.g. por+eng (default) or eng"},
                "first_page":{"type":"integer","description":"PDF only, default 1"},
                "max_pages":{"type":"integer","description":"PDF only, default 20, max 100"}
            },"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = texto(&args, "path")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            let idiomas = texto(&args, "lang").unwrap_or("por+eng");
            if !idiomas_validos(idiomas) {
                return Err(ToolError::InvalidArguments(format!(
                    "idioma invalido: {idiomas} (use codigos do tesseract, ex. por+eng)"
                )));
            }
            let ext = extensao(rel);
            if !matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "tif" | "tiff" | "pdf"
            ) {
                return Err(ToolError::InvalidArguments(format!(
                    "extensao nao suportada pelo ocr: '{ext}' (png, jpg, jpeg, tif, tiff, pdf)"
                )));
            }
            let arq = arquivo_da_tarefa(ctx, rel)?;
            let prazo = ctx.timeout;
            // Um prazo para a chamada inteira, e nao um por pagina: dez paginas de 30 s
            // cada seriam cinco minutos numa chamada que prometeu 30 s.
            let fim = Instant::now() + prazo;
            let estouro = |e: ToolError| match e {
                ToolError::Timeout(_) => ToolError::Timeout(prazo.as_millis() as u64),
                outro => outro,
            };
            let (lido, aviso) = if ext == "pdf" {
                let primeira = args
                    .get("first_page")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .max(1);
                let max = args
                    .get("max_pages")
                    .and_then(Value::as_u64)
                    .unwrap_or(20)
                    .clamp(1, 100);
                let ultima = primeira + max - 1;
                let tmp = PastaTemp::nova("phx-ocr")?;
                let r = tokio::time::timeout(
                    prazo,
                    tokio::process::Command::new("pdftoppm")
                        .args(["-r", "200", "-png", "-f"])
                        .arg(primeira.to_string())
                        .arg("-l")
                        .arg(ultima.to_string())
                        .arg(&arq)
                        .arg(tmp.0.join("p"))
                        .stdin(Stdio::null())
                        .kill_on_drop(true)
                        .output(),
                )
                .await
                .map_err(|_| ToolError::Timeout(prazo.as_millis() as u64))?
                .map_err(|e| {
                    ToolError::Failed(format!(
                        "pdftoppm nao executou ({e}); instale poppler-utils"
                    ))
                })?;
                if !r.status.success() {
                    return Err(ToolError::Failed(format!(
                        "pdftoppm falhou: {}",
                        String::from_utf8_lossy(&r.stderr).trim()
                    )));
                }
                // O pdftoppm completa o numero com zeros conforme o total de paginas, entao a
                // ordem do nome e a ordem das paginas; a do read_dir nao e ordem nenhuma.
                let mut paginas: Vec<PathBuf> = std::fs::read_dir(&tmp.0)
                    .map_err(falha)?
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "png"))
                    .collect();
                paginas.sort();
                if paginas.is_empty() {
                    return Err(ToolError::InvalidArguments(format!(
                        "{rel} nao tem a pagina {primeira}"
                    )));
                }
                let mut partes = vec![];
                for (i, p) in paginas.iter().enumerate() {
                    let resta = fim.saturating_duration_since(Instant::now());
                    if resta.is_zero() {
                        return Err(ToolError::Timeout(prazo.as_millis() as u64));
                    }
                    let t = tesseract(p, idiomas, None, resta).await.map_err(estouro)?;
                    partes.push(format!(
                        "--- pagina {} ---\n{}",
                        primeira + i as u64,
                        t.trim()
                    ));
                }
                let aviso = if paginas.len() as u64 == max {
                    format!(
                        "\n[teto de {max} paginas atingido; para continuar use first_page={}]",
                        ultima + 1
                    )
                } else {
                    String::new()
                };
                (partes.join("\n"), aviso)
            } else {
                let t = tesseract(&arq, idiomas, None, prazo)
                    .await
                    .map_err(estouro)?;
                (t.trim().to_string(), String::new())
            };
            let conteudo = if lido
                .lines()
                .all(|l| l.trim().is_empty() || l.starts_with("---"))
            {
                format!("OCR de {rel} ({idiomas}): nenhum texto reconhecido{aviso}")
            } else {
                format!(
                    "OCR de {rel} ({idiomas}), {} caracteres:\n{}{aviso}",
                    lido.chars().count(),
                    truncate_for_model(&lido, MAX_TEXTO)
                )
            };
            Ok(ToolOutput::text(conteudo))
        })
    }
}

// ---------------------------------------------------------------- informacao de imagem

/// Largura, altura e cor de um PNG, lidas do IHDR. O IHDR e obrigatorio e vem logo apos a
/// assinatura (RFC 2083 3.2), entao nao ha o que procurar: ou esta no byte 12, ou nao e PNG.
pub fn info_png(b: &[u8]) -> Result<(u32, u32, String), String> {
    const ASSINATURA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if b.len() < 26 || b[..8] != ASSINATURA {
        return Err("nao e PNG: assinatura ausente".into());
    }
    if &b[12..16] != b"IHDR" {
        return Err("PNG sem IHDR no inicio".into());
    }
    let u32_em = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (largura, altura, bits, tipo) = (u32_em(16), u32_em(20), b[24], b[25]);
    if largura == 0 || altura == 0 {
        return Err("PNG com dimensao zero".into());
    }
    let cor = match tipo {
        0 => "cinza",
        2 => "RGB",
        3 => "paleta",
        4 => "cinza com alfa",
        6 => "RGBA",
        t => return Err(format!("tipo de cor PNG invalido: {t}")),
    };
    Ok((largura, altura, format!("{cor}, {bits} bits por canal")))
}

/// Largura, altura e cor de um JPEG, lidas do primeiro quadro (SOFn). Os segmentos antes
/// dele (EXIF, ICC) tem tamanho declarado e sao pulados sem ler o conteudo.
pub fn info_jpeg(b: &[u8]) -> Result<(u32, u32, String), String> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return Err("nao e JPEG: falta o marcador SOI".into());
    }
    let mut i = 2;
    loop {
        if i >= b.len() || b[i] != 0xFF {
            return Err(format!("JPEG corrompido: esperado marcador no byte {i}"));
        }
        while i < b.len() && b[i] == 0xFF {
            i += 1;
        }
        let Some(&m) = b.get(i) else {
            return Err("JPEG truncado antes do quadro".into());
        };
        i += 1;
        match m {
            0x01 | 0xD0..=0xD8 => continue,
            0xD9 | 0xDA => return Err("JPEG sem quadro (SOF) antes dos dados".into()),
            _ => {}
        }
        if i + 2 > b.len() {
            return Err("JPEG truncado no tamanho do segmento".into());
        }
        let tam = u16::from_be_bytes([b[i], b[i + 1]]) as usize;
        if tam < 2 {
            return Err("JPEG com segmento de tamanho invalido".into());
        }
        // C4 (Huffman), C8 (reservado) e CC (aritmetica) moram na faixa dos SOF sem ser SOF.
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            if i + 8 > b.len() {
                return Err("JPEG truncado no quadro".into());
            }
            let bits = b[i + 2];
            let altura = u16::from_be_bytes([b[i + 3], b[i + 4]]) as u32;
            let largura = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let cor = match b[i + 7] {
                1 => "cinza".to_string(),
                3 => "cor (YCbCr)".to_string(),
                4 => "CMYK".to_string(),
                n => format!("{n} componentes"),
            };
            let modo = if m == 0xC2 { ", progressivo" } else { "" };
            return Ok((largura, altura, format!("{cor}, {bits} bits{modo}")));
        }
        i += tam;
    }
}

/// O que o elemento raiz de um SVG declara de tamanho.
#[derive(Debug, Default, PartialEq)]
pub struct InfoSvg {
    pub largura: Option<String>,
    pub altura: Option<String>,
    pub view_box: Option<String>,
}

impl InfoSvg {
    /// Tamanho em pixels para renderizar: width/height quando sao numero (ou px), senao o
    /// viewBox. Unidade fisica (cm, %) nao vira pixel aqui: devolve `None` e quem chama usa
    /// o padrao, em vez de adivinhar uma conversao.
    pub fn pixels(&self) -> Option<(u32, u32)> {
        let num = |s: &Option<String>| {
            s.as_deref()
                .map(|v| v.trim().trim_end_matches("px"))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| *v >= 1.0)
        };
        if let (Some(w), Some(h)) = (num(&self.largura), num(&self.altura)) {
            return Some((w.ceil() as u32, h.ceil() as u32));
        }
        let vb: Vec<f64> = self
            .view_box
            .as_deref()?
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        match vb[..] {
            [_, _, w, h] if w >= 1.0 && h >= 1.0 => Some((w.ceil() as u32, h.ceil() as u32)),
            _ => None,
        }
    }
}

/// Confere que o texto e XML bem formado com UMA raiz `<svg>`, e le o tamanho dela. A
/// profundidade e contada aqui porque o leitor chega ao fim do arquivo sem reclamar de
/// elemento aberto.
pub fn info_svg(t: &str) -> Result<InfoSvg, String> {
    use quick_xml::Reader;
    use quick_xml::events::{BytesStart, Event};
    fn raiz(e: &BytesStart) -> Result<InfoSvg, String> {
        let nome = e.local_name();
        if nome.as_ref() != "svg" {
            return Err(format!("a raiz e <{}>, nao <svg>", nome.as_ref()));
        }
        let mut info = InfoSvg::default();
        for a in e.attributes() {
            let a = a.map_err(|e| format!("atributo invalido na raiz: {e}"))?;
            let v = Some(a.value.to_string());
            match a.key.local_name().as_ref() {
                "width" => info.largura = v,
                "height" => info.altura = v,
                "viewBox" => info.view_box = v,
                _ => {}
            }
        }
        Ok(info)
    }
    let mut r = Reader::from_str(t);
    let mut prof = 0usize;
    let mut achada: Option<InfoSvg> = None;
    let mut raizes = 0;
    loop {
        let ev = r
            .read_event()
            .map_err(|e| format!("XML invalido perto do byte {}: {e}", r.buffer_position()))?;
        match ev {
            Event::Eof => break,
            Event::Start(e) => {
                if prof == 0 {
                    raizes += 1;
                    achada = Some(raiz(&e)?);
                }
                prof += 1;
            }
            Event::Empty(e) if prof == 0 => {
                raizes += 1;
                achada = Some(raiz(&e)?);
            }
            Event::End(_) => {
                prof = prof
                    .checked_sub(1)
                    .ok_or("XML invalido: fechamento sem abertura")?;
            }
            Event::Text(x) if prof == 0 && !x.trim().is_empty() => {
                return Err("XML invalido: texto fora do elemento raiz".into());
            }
            _ => {}
        }
    }
    if prof != 0 {
        return Err(format!("XML invalido: {prof} elemento(s) sem fechamento"));
    }
    if raizes != 1 {
        return Err(format!(
            "XML invalido: {raizes} elementos raiz (tem de ser um)"
        ));
    }
    achada.ok_or_else(|| "sem elemento raiz".into())
}

pub struct ImageInfoTool;

impl Tool for ImageInfoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "image".into(),
            description: "Information about an image in the task folder: width, height and color \
of a .png/.jpg/.jpeg, or validate an .svg (well-formed XML with an <svg> root) and read its \
width/height/viewBox. To convert SVG or HTML to PNG use image_render."
                .into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = texto(&args, "path")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            let ext = extensao(rel);
            if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "svg") {
                return Err(ToolError::InvalidArguments(format!(
                    "extensao nao suportada: '{ext}' (png, jpg, jpeg, svg)"
                )));
            }
            let b = std::fs::read(arquivo_da_tarefa(ctx, rel)?).map_err(falha)?;
            let invalido = |e: String| ToolError::InvalidArguments(format!("{rel}: {e}"));
            let r = match ext.as_str() {
                "png" => {
                    let (w, h, cor) = info_png(&b).map_err(invalido)?;
                    format!("{rel}: PNG {w}x{h}, {cor}, {} bytes", b.len())
                }
                "svg" => {
                    let t = String::from_utf8(b).map_err(|_| invalido("SVG nao e UTF-8".into()))?;
                    let i = info_svg(&t).map_err(invalido)?;
                    let v = |o: &Option<String>| o.clone().unwrap_or_else(|| "ausente".into());
                    format!(
                        "{rel}: SVG bem formado; width={}, height={}, viewBox={}{}",
                        v(&i.largura),
                        v(&i.altura),
                        v(&i.view_box),
                        i.pixels()
                            .map(|(w, h)| format!("; renderiza em {w}x{h} px"))
                            .unwrap_or_default()
                    )
                }
                _ => {
                    let (w, h, cor) = info_jpeg(&b).map_err(invalido)?;
                    format!("{rel}: JPEG {w}x{h}, {cor}, {} bytes", b.len())
                }
            };
            Ok(ToolOutput::text(r))
        })
    }
}

// ---------------------------------------------------------------- SVG/HTML para PNG

/// Host inventado pelo qual o Chromium pede os arquivos da tarefa. Nao existe em DNS de
/// proposito: so o servidor daqui o responde, e tudo o mais e recusado.
const HOST_DA_PASTA: &str = "pasta.phxclaw";

/// Servidor HTTP minimo que e, ao mesmo tempo, o proxy do Chromium. Toda requisicao da
/// pagina passa por ele (`--proxy-bypass-list=<-loopback>` tira ate o loopback do atalho),
/// e ele so responde arquivo da pasta da tarefa. Assim a pagina renderizada nao alcanca
/// a rede, nem servico local, nem arquivo fora da pasta: aberta por `file://`, um
/// `<iframe src="file:///...">` pintaria na captura qualquer arquivo da maquina para
/// quem so tem `fs.read` da propria pasta.
struct ServidorDaPasta {
    porta: u16,
    recusados: Arc<Mutex<Vec<String>>>,
    tarefa: tokio::task::JoinHandle<()>,
}

impl Drop for ServidorDaPasta {
    fn drop(&mut self) {
        self.tarefa.abort();
    }
}

impl ServidorDaPasta {
    async fn subir(pasta: PathBuf) -> Result<Self, ToolError> {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(falha)?;
        let porta = l.local_addr().map_err(falha)?.port();
        let recusados: Arc<Mutex<Vec<String>>> = Arc::default();
        let r = recusados.clone();
        let tarefa = tokio::spawn(async move {
            while let Ok((s, _)) = l.accept().await {
                tokio::spawn(atender(s, pasta.clone(), r.clone()));
            }
        });
        Ok(Self {
            porta,
            recusados,
            tarefa,
        })
    }
}

fn decodificar_url(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn codificar_caminho(s: &str) -> String {
    s.bytes()
        .map(|c| {
            if c.is_ascii_alphanumeric() || b"-._~/".contains(&c) {
                (c as char).to_string()
            } else {
                format!("%{c:02X}")
            }
        })
        .collect()
}

async fn atender(mut s: tokio::net::TcpStream, pasta: PathBuf, recusados: Arc<Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut cab = Vec::new();
    let mut buf = [0u8; 4096];
    let lido = tokio::time::timeout(Duration::from_secs(10), async {
        while !cab.windows(4).any(|w| w == b"\r\n\r\n") && cab.len() < 16 * 1024 {
            match s.read(&mut buf).await {
                Ok(0) | Err(_) => return false,
                Ok(n) => cab.extend_from_slice(&buf[..n]),
            }
        }
        true
    })
    .await;
    if lido != Ok(true) {
        return;
    }
    let linha = String::from_utf8_lossy(&cab)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let mut partes = linha.split_whitespace();
    let (metodo, alvo) = (partes.next().unwrap_or(""), partes.next().unwrap_or(""));
    let prefixo = format!("http://{HOST_DA_PASTA}/");
    let (status, tipo, corpo) = match alvo.strip_prefix(&prefixo) {
        Some(resto) if metodo == "GET" => {
            let caminho = resto.split(['?', '#']).next().unwrap_or("");
            match decodificar_url(caminho)
                .and_then(|c| confine(&pasta, &c).ok())
                .filter(|p| p.is_file())
                .and_then(|p| std::fs::read(&p).ok().map(|b| (p, b)))
            {
                Some((p, b)) => ("200 OK", media_type(&p.to_string_lossy()), b),
                None => ("404 Not Found", "text/plain", b"nao encontrado".to_vec()),
            }
        }
        _ => {
            recusados
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(format!("{metodo} {alvo}"));
            ("403 Forbidden", "text/plain", b"recusado".to_vec())
        }
    };
    let cab = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        corpo.len()
    );
    let _ = s.write_all(cab.as_bytes()).await;
    let _ = s.write_all(&corpo).await;
    let _ = s.shutdown().await;
}

pub struct ImageRenderTool;

impl Tool for ImageRenderTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "image_render".into(),
            description: "Render an .svg or .html file of the task folder to a PNG with headless \
Chromium. The page sees only the task folder (no network, no other local files). Default size: \
the SVG's own width/height or viewBox, 1280x800 for HTML."
                .into(),
            parameters: json!({"type":"object","properties":{
                "source":{"type":"string","description":".svg, .html or .htm in the task folder"},
                "output":{"type":"string","description":".png path, default source with .png"},
                "width":{"type":"integer"},
                "height":{"type":"integer"}
            },"required":["source"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let fonte = texto(&args, "source")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'source'".into()))?;
            let ext = extensao(fonte);
            if !matches!(ext.as_str(), "svg" | "html" | "htm") {
                return Err(ToolError::InvalidArguments(format!(
                    "image_render converte svg, html e htm, nao '{ext}'"
                )));
            }
            let saida = match texto(&args, "output") {
                Some(s) => s.to_string(),
                None => Path::new(fonte)
                    .with_extension("png")
                    .to_string_lossy()
                    .into_owned(),
            };
            if extensao(&saida) != "png" {
                return Err(ToolError::InvalidArguments(format!(
                    "a saida tem de terminar em .png: {saida}"
                )));
            }
            let arq = arquivo_da_tarefa(ctx, fonte)?;
            let alvo = confine(&ctx.workdir, &saida).map_err(ToolError::Denied)?;
            let padrao = if ext == "svg" {
                let t = std::fs::read_to_string(&arq).map_err(falha)?;
                let info = info_svg(&t)
                    .map_err(|e| ToolError::InvalidArguments(format!("{fonte}: {e}")))?;
                info.pixels().unwrap_or((800, 600))
            } else {
                (1280, 800)
            };
            let dim = |k: &str, p: u32| {
                args.get(k)
                    .and_then(Value::as_u64)
                    .map(|v| v as u32)
                    .unwrap_or(p)
                    .clamp(16, 4096)
            };
            let (w, h) = (dim("width", padrao.0), dim("height", padrao.1));
            let chromium = phxclaw_browser::find_chromium().ok_or_else(|| {
                ToolError::Failed(format!(
                    "Chromium nao encontrado; defina {}",
                    phxclaw_browser::CHROMIUM_ENV
                ))
            })?;
            if let Some(d) = alvo.parent() {
                std::fs::create_dir_all(d).map_err(falha)?;
            }
            // PNG velho no lugar faria uma falha do Chromium parecer sucesso.
            let _ = std::fs::remove_file(&alvo);
            let rel = arq
                .strip_prefix(&ctx.workdir)
                .map_err(falha)?
                .to_string_lossy()
                .into_owned();
            let servidor = ServidorDaPasta::subir(ctx.workdir.clone()).await?;
            let perfil = PastaTemp::nova("phx-render")?;
            let mut cmd = tokio::process::Command::new(&chromium);
            cmd.args([
                "--headless",
                "--disable-gpu",
                "--hide-scrollbars",
                "--mute-audio",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-component-update",
                "--proxy-bypass-list=<-loopback>",
                "--force-webrtc-ip-handling-policy=disable_non_proxied_udp",
                "--virtual-time-budget=3000",
            ])
            .arg(format!(
                "--proxy-server=http://127.0.0.1:{}",
                servidor.porta
            ))
            .arg(format!("--user-data-dir={}", perfil.0.display()))
            .arg(format!("--window-size={w},{h}"))
            .arg(format!("--screenshot={}", alvo.display()));
            if phxclaw_browser::running_as_root() {
                // mesma decisao do lancador do navegador: o sandbox do Chromium nao sobe
                // como root (conteiner), e so nesse caso sai
                cmd.arg("--no-sandbox");
            }
            cmd.arg(format!(
                "http://{HOST_DA_PASTA}/{}",
                codificar_caminho(&rel)
            ))
            .stdin(Stdio::null())
            .kill_on_drop(true);
            let r = tokio::time::timeout(ctx.timeout, cmd.output())
                .await
                .map_err(|_| ToolError::Timeout(ctx.timeout.as_millis() as u64))?
                .map_err(|e| ToolError::Failed(format!("{}: {e}", chromium.display())))?;
            let png = std::fs::read(&alvo).unwrap_or_default();
            let Ok((pw, ph, _)) = info_png(&png) else {
                return Err(ToolError::Failed(format!(
                    "o Chromium nao gravou o PNG (saida {:?}): {}",
                    r.status.code(),
                    String::from_utf8_lossy(&r.stderr)
                        .lines()
                        .rev()
                        .take(5)
                        .collect::<Vec<_>>()
                        .join(" | ")
                )));
            };
            let recusados = servidor
                .recusados
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            let mut conteudo = format!("{fonte} renderizado em {saida} ({pw}x{ph} px)");
            if !recusados.is_empty() {
                conteudo.push_str(&format!(
                    "\n{} pedido(s) de fora da pasta recusados: {}",
                    recusados.len(),
                    recusados
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Ok(ToolOutput {
                content: conteudo,
                artifacts: vec![artifact_for(&ctx.workdir, &saida).map_err(falha)?],
            })
        })
    }
}

// ---------------------------------------------------------------- fala para texto

/// Fala para texto com whisper.cpp. O modelo so roda com o SHA-256 declarado conferido:
/// `WhisperCppProvider` recusa sem ele, e um hash calculado aqui do proprio arquivo nao
/// conferiria nada.
pub struct TranscribeTool {
    pub bin: Option<PathBuf>,
    pub model: Option<PathBuf>,
    pub model_sha256: Option<String>,
}

impl TranscribeTool {
    pub fn from_env() -> Self {
        let var = |n: &str| std::env::var(n).ok().filter(|v| !v.trim().is_empty());
        Self {
            bin: var("PHXCLAW_WHISPER_BIN").map(PathBuf::from),
            model: var("PHXCLAW_WHISPER_MODEL").map(PathBuf::from),
            model_sha256: var("PHXCLAW_WHISPER_MODEL_SHA256"),
        }
    }
}

impl Tool for TranscribeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "transcribe".into(),
            description: "Speech to text (whisper.cpp) of a .wav audio file in the task folder \
(16 kHz mono works best). Optional 'language' (e.g. en, pt, auto)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "language":{"type":"string"}
            },"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "media.stt"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let faltam: Vec<&str> = [
                ("PHXCLAW_WHISPER_BIN", self.bin.is_none()),
                ("PHXCLAW_WHISPER_MODEL", self.model.is_none()),
                ("PHXCLAW_WHISPER_MODEL_SHA256", self.model_sha256.is_none()),
            ]
            .into_iter()
            .filter(|(_, falta)| *falta)
            .map(|(n, _)| n)
            .collect();
            if !faltam.is_empty() {
                return Err(ToolError::Denied(format!(
                    "fala para texto nao configurada: faltam {}",
                    faltam.join(", ")
                )));
            }
            let rel = texto(&args, "path")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            if extensao(rel) != "wav" {
                return Err(ToolError::InvalidArguments(format!(
                    "transcribe le .wav; converta antes (ffmpeg -i {rel} -ar 16000 -ac 1 audio.wav)"
                )));
            }
            let idioma = texto(&args, "language").map(str::to_ascii_lowercase);
            if let Some(l) = &idioma
                && !(l == "auto"
                    || (2..=3).contains(&l.len()) && l.chars().all(|c| c.is_ascii_lowercase()))
            {
                return Err(ToolError::InvalidArguments(format!(
                    "idioma invalido: {l} (ex. en, pt, auto)"
                )));
            }
            let audio = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            if !audio.is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "arquivo nao encontrado na pasta da tarefa: {rel}"
                )));
            }
            let tmp = PastaTemp::nova("phx-stt")?;
            let provedor = phxclaw_media_intelligence::WhisperCppProvider {
                program: self.bin.clone().unwrap_or_default(),
                model: self.model.clone().unwrap_or_default(),
                model_sha256: self.model_sha256.clone().unwrap_or_default(),
                threads: None,
                // O provedor mata o filho ao estourar: o prazo da chamada vira o dele.
                timeout_seconds: ctx.timeout.as_secs().clamp(1, 7200),
                max_audio_bytes: 512 * 1024 * 1024,
            };
            let pedido = phxclaw_media_intelligence::SpeechToTextRequest {
                input_audio: audio,
                language: idioma,
                output_txt: Some(tmp.0.join("fala.txt")),
            };
            let r = tokio::task::spawn_blocking(move || provedor.transcribe_verified(&pedido))
                .await
                .map_err(falha)?;
            let fala = match r {
                Ok(t) => t,
                Err(e) if e.to_string().contains("timed out") => {
                    return Err(ToolError::Timeout(ctx.timeout.as_millis() as u64));
                }
                Err(e) => return Err(ToolError::Failed(format!("whisper.cpp: {e}"))),
            };
            let fala = fala.trim();
            Ok(ToolOutput::text(if fala.is_empty() {
                format!("transcricao de {rel}: nenhuma fala reconhecida")
            } else {
                format!(
                    "transcricao de {rel}, {} caracteres:\n{}",
                    fala.chars().count(),
                    truncate_for_model(fala, MAX_TEXTO)
                )
            }))
        })
    }
}

// ---------------------------------------------------------------- desktop

#[cfg(feature = "desktop")]
pub use desktop::DesktopTool;

#[cfg(feature = "desktop")]
mod desktop {
    use super::*;
    use phxclaw_system_automation::{
        EnigoInputProvider, InputAction, InputProvider, capturar_monitor_principal,
    };

    /// Teclado, mouse e captura da sessao grafica. Capacidade propria, `desktop.control`,
    /// fora do padrao: quem controla o desktop age como o usuario logado, fora de sandbox.
    pub struct DesktopTool;

    /// Sem sessao grafica o enigo e o xcap falham com erro de conexao X cru; a recusa diz o
    /// que falta em vez disso.
    fn sem_sessao_grafica() -> bool {
        if !cfg!(target_os = "linux") {
            return false;
        }
        let vazia = |n: &str| std::env::var_os(n).is_none_or(|v| v.is_empty());
        vazia("DISPLAY") && vazia("WAYLAND_DISPLAY")
    }

    async fn bloqueante<T: Send + 'static>(
        prazo: Duration,
        f: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, ToolError> {
        tokio::time::timeout(prazo, tokio::task::spawn_blocking(f))
            .await
            .map_err(|_| ToolError::Timeout(prazo.as_millis() as u64))?
            .map_err(falha)?
            .map_err(ToolError::Failed)
    }

    fn inteiro(args: &Value, k: &str) -> Result<Option<i32>, ToolError> {
        match args.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .map(Some)
                .ok_or_else(|| ToolError::InvalidArguments(format!("'{k}' tem de ser inteiro"))),
        }
    }

    fn par(args: &Value) -> Result<Option<(i32, i32)>, ToolError> {
        match (inteiro(args, "x")?, inteiro(args, "y")?) {
            (Some(x), Some(y)) => Ok(Some((x, y))),
            (None, None) => Ok(None),
            _ => Err(ToolError::InvalidArguments(
                "informe 'x' e 'y' juntos".into(),
            )),
        }
    }

    impl Tool for DesktopTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "desktop".into(),
                description: "Control the real desktop session (mouse, keyboard, screen). \
action=move {x,y}; click {x?,y?,button?=left,double?}; scroll {dx?,dy}; key {key,state?=click} \
(enter, tab, esc, f1.., or one character); hotkey {keys:[\"ctrl\",\"s\"]}; type {text}; \
position; screenshot {path?=tela.png} saves a PNG in the task folder."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["move","click","scroll","key","hotkey","type","position","screenshot"]},
                    "x":{"type":"integer"},"y":{"type":"integer"},
                    "button":{"type":"string","enum":["left","middle","right"]},
                    "double":{"type":"boolean"},
                    "dx":{"type":"integer"},"dy":{"type":"integer"},
                    "key":{"type":"string"},"state":{"type":"string","enum":["click","press","release"]},
                    "keys":{"type":"array","items":{"type":"string"}},
                    "text":{"type":"string"},
                    "path":{"type":"string"}
                },"required":["action"]}),
            }
        }
        fn capability(&self) -> &'static str {
            "desktop.control"
        }
        fn run<'a>(
            &'a self,
            args: Value,
            ctx: &'a ToolContext,
        ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
            Box::pin(async move {
                if sem_sessao_grafica() {
                    return Err(ToolError::Denied(
                        "sem DISPLAY: nao ha sessao grafica para controlar (defina DISPLAY, \
por exemplo o de um Xvfb)"
                            .into(),
                    ));
                }
                let acao = texto(&args, "action")
                    .ok_or_else(|| ToolError::InvalidArguments("falta 'action'".into()))?;
                let prazo = ctx.timeout;
                let invalido = |m: &str| ToolError::InvalidArguments(m.into());
                let acoes: Vec<InputAction> = match acao {
                    "screenshot" => return capturar(&args, ctx).await,
                    "position" => {
                        let (x, y) =
                            bloqueante(prazo, || EnigoInputProvider::new()?.posicao_do_mouse())
                                .await?;
                        return Ok(ToolOutput::text(format!("mouse em ({x},{y})")));
                    }
                    "move" => {
                        let (x, y) = par(&args)?.ok_or_else(|| invalido("move pede 'x' e 'y'"))?;
                        vec![InputAction::MoveMouse { x, y }]
                    }
                    "click" => {
                        let mut v = vec![];
                        if let Some((x, y)) = par(&args)? {
                            v.push(InputAction::MoveMouse { x, y });
                        }
                        let botao = texto(&args, "button").unwrap_or("left").to_string();
                        let vezes = if args.get("double").and_then(Value::as_bool) == Some(true) {
                            2
                        } else {
                            1
                        };
                        for _ in 0..vezes {
                            v.push(InputAction::MouseButton {
                                button: botao.clone(),
                                state: "click".into(),
                            });
                        }
                        v
                    }
                    "scroll" => {
                        let dx = inteiro(&args, "dx")?.unwrap_or(0);
                        let dy = inteiro(&args, "dy")?.unwrap_or(0);
                        if dx == 0 && dy == 0 {
                            return Err(invalido("scroll pede 'dx' ou 'dy' diferente de zero"));
                        }
                        vec![InputAction::Scroll { dx, dy }]
                    }
                    "key" => vec![InputAction::Key {
                        key: texto(&args, "key")
                            .ok_or_else(|| invalido("key pede 'key'"))?
                            .to_string(),
                        state: texto(&args, "state").unwrap_or("click").to_string(),
                    }],
                    "hotkey" => {
                        let keys: Vec<String> = args
                            .get("keys")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default();
                        if keys.is_empty() {
                            return Err(invalido("hotkey pede 'keys', ex. [\"ctrl\",\"s\"]"));
                        }
                        vec![InputAction::Hotkey { keys }]
                    }
                    "type" => {
                        let t = args
                            .get("text")
                            .and_then(Value::as_str)
                            .filter(|t| !t.is_empty())
                            .ok_or_else(|| invalido("type pede 'text'"))?;
                        if t.chars().count() > 4000 {
                            return Err(invalido("texto acima de 4000 caracteres"));
                        }
                        vec![InputAction::Text { text: t.into() }]
                    }
                    outra => {
                        return Err(ToolError::InvalidArguments(format!(
                            "acao desconhecida: {outra}"
                        )));
                    }
                };
                let n = acoes.len();
                let (x, y) = bloqueante(prazo, move || {
                    let mut e = EnigoInputProvider::new()?;
                    for a in &acoes {
                        e.apply(a)?;
                    }
                    e.posicao_do_mouse()
                })
                .await?;
                Ok(ToolOutput::text(format!(
                    "{acao}: {n} evento(s) enviados; mouse em ({x},{y})"
                )))
            })
        }
    }

    async fn capturar(args: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let rel = texto(args, "path").unwrap_or("tela.png").to_string();
        if extensao(&rel) != "png" {
            return Err(ToolError::InvalidArguments(format!(
                "a captura grava .png: {rel}"
            )));
        }
        let alvo = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
        if let Some(d) = alvo.parent() {
            std::fs::create_dir_all(d).map_err(falha)?;
        }
        let (w, h, cores) = bloqueante(ctx.timeout, move || {
            let img = capturar_monitor_principal().map_err(|e| e.to_string())?;
            // Tela de uma cor so e a captura que "funcionou" sem mostrar nada (sessao
            // bloqueada, monitor desligado): contar diz isso a quem olha so o texto.
            let mut vistas = std::collections::HashSet::new();
            for p in img.pixels() {
                vistas.insert(p.0);
                if vistas.len() >= 1000 {
                    break;
                }
            }
            img.save(&alvo).map_err(|e| e.to_string())?;
            Ok((img.width(), img.height(), vistas.len()))
        })
        .await?;
        let aviso = if cores == 1 {
            " (tela de uma cor so: vazia, bloqueada ou desligada?)"
        } else {
            ""
        };
        Ok(ToolOutput {
            content: format!(
                "captura {w}x{h} gravada em {rel}; {}{} cores distintas{aviso}",
                cores,
                if cores >= 1000 { "+" } else { "" }
            ),
            artifacts: vec![artifact_for(&ctx.workdir, &rel).map_err(falha)?],
        })
    }
}

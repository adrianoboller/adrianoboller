//! Imagens anexadas a tarefa: o modelo as VE junto do objetivo, pelo bloco de imagem de
//! cada provedor (`images` do Ollama, `image` da Anthropic, `input_image` da OpenAI,
//! `inline_data` do Gemini). O OCR continua existindo, mas texto extraido nao e a imagem:
//! grafico, layout e foto se perdem nele.
//!
//! A API e a CLI passam pelas MESMAS funcoes daqui: validar os bytes, gravar em
//! `work/entrada/` e anexar no comeco da tarefa. Duas validacoes seriam duas politicas
//! para a mesma porta.
//!
//! O tipo se confere pelos BYTES, nunca pelo que o cliente declara nem pela extensao: um
//! arquivo qualquer chamado `.png` iria ao provedor como imagem e voltaria como erro opaco
//! dele, ou pior, seria aceito como outra coisa.
//!
//! E as DIMENSOES tambem se leem do cabecalho antes de qualquer decodificacao (B4): o teto
//! de bytes nao protege de um PNG de 5 MB que declara 100.000 x 100.000 pixels -- quem o
//! decodificar (o OCR local, o provedor) aloca 40 GB. O teto de pixels e
//! `imagem.entrada_pixels_max` (`PHXCLAW_IMAGEM_ENTRADA_PIXELS_MAX`) do catalogo, e a
//! recusa diz o tamanho declarado.

use base64::Engine as _;
use phxclaw_agent_core::ImagemAnexa;
use std::path::Path;

/// Imagens por tarefa. Cada uma viaja em TODA volta do laco (o historico vai inteiro ao
/// provedor), entao o teto e pequeno de proposito.
pub const IMAGENS_MAX: usize = 4;
/// Bytes por imagem: o limite da Anthropic por imagem e 5 MB, o mais estreito dos quatro.
pub const BYTES_MAX: usize = 5 * 1024 * 1024;
/// Pasta das imagens dentro de `work/`: o modelo pode le-las tambem pelas ferramentas.
pub const PASTA: &str = "entrada";
/// A chave do catalogo com o teto de pixels (largura x altura) de uma imagem de entrada.
pub const CHAVE_PIXELS_MAX: &str = "imagem.entrada_pixels_max";

/// Largura e altura declaradas no cabecalho, sem decodificar nada.
pub fn dimensoes(b: &[u8]) -> Result<(u32, u32), String> {
    match tipo_pelos_bytes(b) {
        Some("image/png") => crate::visao::info_png(b).map(|(w, h, _)| (w, h)),
        Some("image/jpeg") => crate::visao::info_jpeg(b).map(|(w, h, _)| (w, h)),
        Some("image/gif") => crate::visao::info_gif(b),
        Some("image/webp") => crate::visao::info_webp(b),
        _ => Err("nao e png, jpeg, gif nem webp (conferido pelos bytes)".into()),
    }
}

/// O teto de pixels do catalogo (o padrao vem de la; o operador muda no config.json).
pub fn teto_de_pixels() -> Result<u64, String> {
    crate::config::valor(CHAVE_PIXELS_MAX)?
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("{CHAVE_PIXELS_MAX} sem valor inteiro no catalogo"))
}

/// Recusa a imagem cujo cabecalho declara mais de `teto` pixels, dizendo o tamanho.
pub fn conferir_pixels(b: &[u8], teto: u64) -> Result<(u32, u32), String> {
    let (w, h) = dimensoes(b)?;
    let pixels = u64::from(w) * u64::from(h);
    if pixels > teto {
        return Err(format!(
            "imagem declara {w}x{h} = {pixels} pixels; o teto e {teto} ({CHAVE_PIXELS_MAX})"
        ));
    }
    Ok((w, h))
}

/// `image/png`, `image/jpeg`, `image/gif` ou `image/webp`, pelos primeiros bytes.
pub fn tipo_pelos_bytes(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn extensao(tipo: &str) -> &'static str {
    match tipo {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "webp",
    }
}

/// Confere a lista inteira ANTES de qualquer disco: pedido com uma imagem ruim nao cria
/// tarefa pela metade. O teto de pixels vem do catalogo.
pub fn validar(imagens: &[Vec<u8>]) -> Result<(), String> {
    validar_com(imagens, teto_de_pixels()?)
}

/// `validar` com o teto de pixels dado (os testes medem o teto sem config.json).
pub fn validar_com(imagens: &[Vec<u8>], teto_pixels: u64) -> Result<(), String> {
    if imagens.len() > IMAGENS_MAX {
        return Err(format!(
            "{} imagens; o teto e {IMAGENS_MAX} por tarefa",
            imagens.len()
        ));
    }
    for (i, b) in imagens.iter().enumerate() {
        if b.len() > BYTES_MAX {
            return Err(format!(
                "imagem {} tem {} bytes; o teto e {BYTES_MAX}",
                i + 1,
                b.len()
            ));
        }
        if tipo_pelos_bytes(b).is_none() {
            return Err(format!(
                "imagem {} nao e png, jpeg, gif nem webp (conferido pelos bytes)",
                i + 1
            ));
        }
        conferir_pixels(b, teto_pixels).map_err(|e| format!("imagem {}: {e}", i + 1))?;
    }
    Ok(())
}

/// Base64 do pedido da API; aceita tambem a URL `data:image/...;base64,` inteira, que e
/// como o navegador entrega um arquivo lido.
pub fn de_base64(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    let s = match s.strip_prefix("data:") {
        Some(resto) => resto
            .split_once(";base64,")
            .map(|(_, d)| d)
            .ok_or("URL data: sem ';base64,'")?,
        None => s,
    };
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|e| format!("base64 invalido: {e}"))
}

/// Grava as imagens (ja validadas) em `work/entrada/` e devolve os caminhos relativos que
/// vao para `Task::images`.
pub fn gravar(workdir: &Path, imagens: &[Vec<u8>]) -> Result<Vec<String>, String> {
    validar(imagens)?;
    let pasta = workdir.join(PASTA);
    if !imagens.is_empty() {
        std::fs::create_dir_all(&pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
    }
    let mut rels = Vec::new();
    for (i, b) in imagens.iter().enumerate() {
        let tipo = tipo_pelos_bytes(b).expect("validado acima");
        let rel = format!("{PASTA}/imagem-{}.{}", i + 1, extensao(tipo));
        std::fs::write(workdir.join(&rel), b).map_err(|e| format!("{rel}: {e}"))?;
        rels.push(rel);
    }
    Ok(rels)
}

/// Le de volta as imagens da tarefa para a mensagem do modelo. O caminho passa pelo
/// `confine`: `task.json` editado a mao nao vira leitura fora da pasta da tarefa.
pub fn carregar(workdir: &Path, rels: &[String]) -> Result<Vec<ImagemAnexa>, String> {
    let mut v = Vec::new();
    for rel in rels.iter().take(IMAGENS_MAX) {
        let p = crate::tarefa::confine(workdir, rel)?;
        let b = std::fs::read(&p).map_err(|e| format!("{rel}: {e}"))?;
        validar(std::slice::from_ref(&b)).map_err(|e| format!("{rel}: {e}"))?;
        v.push(ImagemAnexa {
            media_type: tipo_pelos_bytes(&b).expect("validado").into(),
            base64: base64::engine::general_purpose::STANDARD.encode(&b),
        });
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipo_vem_dos_bytes_e_nao_do_nome() {
        assert_eq!(
            tipo_pelos_bytes(b"\x89PNG\r\n\x1a\nresto"),
            Some("image/png")
        );
        assert_eq!(
            tipo_pelos_bytes(b"RIFF\0\0\0\0WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(tipo_pelos_bytes(b"%PDF-1.7"), None);
        assert!(validar_com(&[b"<svg/>".to_vec()], u64::MAX).is_err());
        assert!(validar_com(&vec![b"GIF89a".to_vec(); IMAGENS_MAX + 1], u64::MAX).is_err());
        assert_eq!(de_base64("data:image/png;base64,QUJD").unwrap(), b"ABC");
        assert!(de_base64("data:image/png,QUJD").is_err());
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut p = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        p.extend_from_slice(b"IHDR");
        p.extend_from_slice(&w.to_be_bytes());
        p.extend_from_slice(&h.to_be_bytes());
        p.extend_from_slice(&[8, 6, 0, 0, 0]);
        p
    }

    /// RED medido com o defeito reposto (`validar` so olhava bytes e tipo): a bomba de
    /// 100.000 x 100.000 em 29 bytes passava, e nos quatro formatos.
    #[test]
    fn imagem_acima_do_teto_de_pixels_e_recusada_pelo_cabecalho_com_o_tamanho() {
        let teto = 40_000_000;
        assert!(
            validar_com(&[png(8000, 5000)], teto).is_ok(),
            "no teto passa"
        );
        let e = validar_com(&[png(100_000, 100_000)], teto).unwrap_err();
        assert!(
            e.contains("100000x100000 = 10000000000 pixels") && e.contains("40000000"),
            "{e}"
        );
        // GIF: LSD little-endian. WebP VP8X: 24 bits menos um. WebP VP8L: 14 bits menos um.
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&65535u16.to_le_bytes());
        gif.extend_from_slice(&65535u16.to_le_bytes());
        assert_eq!(dimensoes(&gif).unwrap(), (65535, 65535));
        assert!(validar_com(&[gif], teto).is_err());
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        webp.extend_from_slice(&[0xff, 0xff, 0x0f, 0xff, 0xff, 0x0f]);
        assert_eq!(dimensoes(&webp).unwrap(), (1 << 20, 1 << 20));
        assert!(conferir_pixels(&webp, teto).is_err());
        let mut vp8l = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
        // 14 bits de largura (299) e 14 de altura (199), cada um menos um.
        let bits: u32 = 299 | (199 << 14);
        vp8l.extend_from_slice(&bits.to_le_bytes());
        vp8l.extend_from_slice(&[0; 8]);
        assert_eq!(dimensoes(&vp8l).unwrap(), (300, 200));
        // JPEG: o SOF0 declara 65535 x 65535.
        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xC0, 0, 17, 8, 0xFF, 0xFF, 0xFF, 0xFF, 3];
        jpg.extend_from_slice(&[0; 12]);
        assert!(
            validar_com(&[jpg], teto)
                .unwrap_err()
                .contains("65535x65535")
        );
    }
}

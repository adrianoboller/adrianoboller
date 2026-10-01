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
/// tarefa pela metade.
pub fn validar(imagens: &[Vec<u8>]) -> Result<(), String> {
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
        assert!(validar(&[b"<svg/>".to_vec()]).is_err());
        assert!(validar(&vec![b"GIF89a".to_vec(); IMAGENS_MAX + 1]).is_err());
        assert_eq!(de_base64("data:image/png;base64,QUJD").unwrap(), b"ABC");
        assert!(de_base64("data:image/png,QUJD").is_err());
    }
}

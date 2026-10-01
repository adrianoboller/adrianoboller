//! ocr, image, image_render e transcribe contra as pecas reais: tesseract, pdftoppm,
//! Chromium headless, ffmpeg (so para fabricar a entrada) e whisper.cpp. Cada teste diz
//! que pulou quando a peca falta, em vez de passar calado.

use phxclaw_agent::visao::{
    ImageInfoTool, ImageRenderTool, OcrTool, TranscribeTool, info_jpeg, info_png, info_svg,
};
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-visao-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(90),
    }
}

fn no_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// Falta alguma peca? Diz qual e pula.
fn faltando(pecas: &[&str]) -> bool {
    let faltam: Vec<&&str> = pecas
        .iter()
        .filter(|p| match **p {
            "chromium" => phxclaw_browser::find_chromium().is_none(),
            b => !no_path(b),
        })
        .collect();
    if !faltam.is_empty() {
        eprintln!("PULADO: falta {faltam:?}");
    }
    !faltam.is_empty()
}

/// Pagina com texto grande, preto no branco: o que o OCR le sem ambiguidade.
fn pagina(corpo: &str) -> String {
    format!(
        "<!doctype html><html><body style=\"margin:40px;background:#fff;color:#000;\
font:bold 64px sans-serif\">{corpo}</body></html>"
    )
}

/// PDF de duas paginas impresso pelo Chromium direto, sem passar pela ferramenta: a
/// entrada do ocr nao pode depender do codigo que esta sob prova.
fn pdf_de_duas_paginas(dir: &Path) -> PathBuf {
    let html = dir.join("fonte-pdf.html");
    std::fs::write(
        &html,
        pagina("PRIMEIRA 4170<div style=\"break-before:page\">SEGUNDA 8256</div>"),
    )
    .unwrap();
    let pdf = dir.join("doc.pdf");
    let st = std::process::Command::new(phxclaw_browser::find_chromium().unwrap())
        .args([
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--no-pdf-header-footer",
        ])
        .arg(format!("--print-to-pdf={}", pdf.display()))
        .arg(format!("file://{}", html.display()))
        .output()
        .unwrap();
    assert!(pdf.is_file(), "chromium nao imprimiu: {st:?}");
    pdf
}

#[test]
fn cabecalhos_lidos_a_mao() {
    // PNG minimo: assinatura + IHDR de 300x200, RGBA 8 bits
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&300u32.to_be_bytes());
    png.extend_from_slice(&200u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    assert_eq!(
        info_png(&png).unwrap(),
        (300, 200, "RGBA, 8 bits por canal".into())
    );
    assert!(info_png(b"GIF89a....................").is_err());
    // JPEG: SOI, um APP0 de 16 bytes a pular, SOF0 de 640x480 com 3 componentes
    let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16];
    jpg.extend_from_slice(&[0; 14]);
    jpg.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8, 0x01, 0xE0, 0x02, 0x80, 3]);
    assert_eq!(
        info_jpeg(&jpg).unwrap(),
        (640, 480, "cor (YCbCr), 8 bits".into())
    );
    assert!(info_jpeg(&[0xFF, 0xD8, 0xFF, 0xDA, 0, 2]).is_err());
    let s = info_svg(r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="40" height="30px" viewBox="0 0 4 3"><g><rect/></g></svg>"#).unwrap();
    assert_eq!(s.pixels(), Some((40, 30)));
    assert_eq!(s.view_box.as_deref(), Some("0 0 4 3"));
    let so_vb = info_svg(r#"<svg viewBox="0 0 120 60"/>"#).unwrap();
    assert_eq!(so_vb.pixels(), Some((120, 60)));
    for (ruim, motivo) in [
        ("<svg><g></svg>", ""),
        ("<svg><g>", "sem fechamento"),
        ("<html/>", "nao <svg>"),
        ("<svg/><svg/>", "2 elementos raiz"),
    ] {
        let e = info_svg(ruim).unwrap_err();
        assert!(e.contains(motivo), "{ruim}: {e}");
    }
}

#[tokio::test]
async fn image_le_png_e_jpeg_reais_e_valida_svg() {
    if faltando(&["ffmpeg"]) {
        return;
    }
    let c = ctx();
    for (arq, tam, pix) in [
        ("a.png", "320x240", "rgb24"),
        ("b.jpg", "160x90", "yuvj420p"),
    ] {
        let st = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc=size={tam}:rate=1"))
            .args(["-frames:v", "1", "-pix_fmt", pix])
            .arg(c.workdir.join(arq))
            .status()
            .unwrap();
        assert!(st.success());
    }
    let r = ImageInfoTool
        .run(json!({"path":"a.png"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("PNG 320x240, RGB"), "{}", r.content);
    let r = ImageInfoTool
        .run(json!({"path":"b.jpg"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("JPEG 160x90, cor"), "{}", r.content);
    std::fs::write(
        c.workdir.join("s.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 50 25"><text>oi</text></svg>"#,
    )
    .unwrap();
    let r = ImageInfoTool
        .run(json!({"path":"s.svg"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("renderiza em 50x25"), "{}", r.content);
}

#[tokio::test]
async fn image_recusa_o_que_nao_e_imagem_e_o_que_esta_fora() {
    let c = ctx();
    std::fs::write(c.workdir.join("q.svg"), "<svg><g></svg>").unwrap();
    std::fs::write(c.workdir.join("falso.png"), "nao sou png").unwrap();
    let e = ImageInfoTool
        .run(json!({"path":"q.svg"}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::InvalidArguments(_)), "{e}");
    let e = ImageInfoTool
        .run(json!({"path":"falso.png"}), &c)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("assinatura"), "{e}");
    let e = ImageInfoTool
        .run(json!({"path":"../x.png"}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");
    let e = ImageInfoTool
        .run(json!({"path":"a.gif"}), &c)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("nao suportada"), "{e}");
}

#[tokio::test]
async fn html_vira_png_e_o_ocr_le_o_texto() {
    if faltando(&["chromium", "tesseract"]) {
        return;
    }
    let c = ctx();
    std::fs::create_dir_all(c.workdir.join("telas")).unwrap();
    std::fs::write(
        c.workdir.join("telas/cartaz.html"),
        pagina("PHXCLAW VISAO 4821"),
    )
    .unwrap();
    let r = ImageRenderTool
        .run(
            json!({"source":"telas/cartaz.html","width":900,"height":300}),
            &c,
        )
        .await
        .unwrap();
    assert!(
        r.content.contains("telas/cartaz.png (900x300 px)"),
        "{}",
        r.content
    );
    assert_eq!(r.artifacts[0].path, "telas/cartaz.png");
    let o = OcrTool
        .run(json!({"path":"telas/cartaz.png"}), &c)
        .await
        .unwrap();
    assert!(o.content.contains("PHXCLAW VISAO 4821"), "{}", o.content);
}

#[tokio::test]
async fn svg_vira_png_no_tamanho_dele() {
    if faltando(&["chromium", "tesseract"]) {
        return;
    }
    let c = ctx();
    std::fs::write(
        c.workdir.join("selo.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="500" height="160"><rect width="500" height="160" fill="#fff"/><text x="20" y="100" font-size="64" font-family="sans-serif">SELO 7788</text></svg>"##,
    )
    .unwrap();
    let r = ImageRenderTool
        .run(json!({"source":"selo.svg","output":"saida/selo.png"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("(500x160 px)"), "{}", r.content);
    let o = OcrTool
        .run(json!({"path":"saida/selo.png","lang":"eng"}), &c)
        .await
        .unwrap();
    assert!(o.content.contains("7788"), "{}", o.content);
}

/// A pagina renderizada ve a pasta da tarefa e nada mais: o iframe de dentro aparece, o de
/// fora (file://) e a imagem da rede nao.
#[tokio::test]
async fn render_nao_alcanca_arquivo_de_fora_nem_rede() {
    if faltando(&["chromium", "tesseract"]) {
        return;
    }
    let c = ctx();
    let fora = std::env::temp_dir().join(format!("phx-fora-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&fora).unwrap();
    let segredo = fora.join("segredo.html");
    std::fs::write(&segredo, pagina("SEGREDO 9137")).unwrap();
    std::fs::write(c.workdir.join("dentro.html"), pagina("DENTRO 5560")).unwrap();
    std::fs::write(
        c.workdir.join("isca.html"),
        format!(
            "<!doctype html><body style=\"margin:0\">\
<iframe src=\"dentro.html\" style=\"width:1200px;height:200px;border:0\"></iframe>\
<iframe src=\"file://{}\" style=\"width:1200px;height:200px;border:0\"></iframe>\
<img src=\"http://example.com/x.png\"></body>",
            segredo.display()
        ),
    )
    .unwrap();
    let r = ImageRenderTool
        .run(json!({"source":"isca.html","width":1200,"height":420}), &c)
        .await
        .unwrap();
    assert!(
        r.content.contains("http://example.com/x.png"),
        "{}",
        r.content
    );
    let o = OcrTool.run(json!({"path":"isca.png"}), &c).await.unwrap();
    assert!(
        o.content.contains("5560"),
        "o iframe de dentro tem de aparecer: {}",
        o.content
    );
    assert!(
        !o.content.contains("9137"),
        "arquivo de fora vazou na captura: {}",
        o.content
    );
    let _ = std::fs::remove_dir_all(&fora);
}

#[tokio::test]
async fn render_recusa() {
    let c = ctx();
    std::fs::write(c.workdir.join("q.svg"), "<svg>").unwrap();
    std::fs::write(c.workdir.join("p.html"), "<p>").unwrap();
    let casos = [
        (json!({"source":"x.txt"}), "svg, html e htm"),
        (json!({"source":"p.html","output":"p.jpg"}), ".png"),
        (json!({"source":"q.svg"}), "sem fechamento"),
        (json!({"source":"falta.html"}), "nao encontrado"),
        (
            json!({"source":"p.html","output":"../fora.png"}),
            "fora da pasta",
        ),
    ];
    for (a, motivo) in casos {
        let e = ImageRenderTool.run(a.clone(), &c).await.unwrap_err();
        assert!(e.to_string().contains(motivo), "{a}: {e}");
    }
}

#[tokio::test]
async fn ocr_de_pdf_le_as_paginas_em_ordem() {
    if faltando(&["chromium", "tesseract", "pdftoppm"]) {
        return;
    }
    let c = ctx();
    pdf_de_duas_paginas(&c.workdir);
    let o = OcrTool.run(json!({"path":"doc.pdf"}), &c).await.unwrap();
    let t = &o.content;
    let (a, b) = (t.find("PRIMEIRA 4170"), t.find("SEGUNDA 8256"));
    assert!(a.is_some() && b.is_some() && a < b, "{t}");
    assert!(t.contains("--- pagina 2 ---"), "{t}");
    // so a segunda, pelo first_page
    let o = OcrTool
        .run(json!({"path":"doc.pdf","first_page":2}), &c)
        .await
        .unwrap();
    assert!(
        !o.content.contains("4170") && o.content.contains("8256"),
        "{}",
        o.content
    );
    // o teto de paginas avisa onde continuar
    let o = OcrTool
        .run(json!({"path":"doc.pdf","max_pages":1}), &c)
        .await
        .unwrap();
    assert!(o.content.contains("first_page=2"), "{}", o.content);
}

#[tokio::test]
async fn ocr_recusa_e_respeita_o_prazo() {
    let mut c = ctx();
    std::fs::write(c.workdir.join("a.png"), "x").unwrap();
    let casos = [
        (json!({}), "falta 'path'"),
        (json!({"path":"a.gif"}), "nao suportada"),
        (json!({"path":"a.png","lang":"-psm"}), "idioma invalido"),
        (json!({"path":"nada.png"}), "nao encontrado"),
        (json!({"path":"/etc/hostname.png"}), "fora da pasta"),
    ];
    for (a, motivo) in casos {
        let e = OcrTool.run(a.clone(), &c).await.unwrap_err();
        assert!(e.to_string().contains(motivo), "{a}: {e}");
    }
    if faltando(&["chromium", "tesseract", "pdftoppm"]) {
        return;
    }
    pdf_de_duas_paginas(&c.workdir);
    c.timeout = Duration::from_millis(1);
    let e = OcrTool
        .run(json!({"path":"doc.pdf"}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Timeout(1)), "{e}");
}

/// SHA-256 do ggml-tiny.en.bin publicado pelo Hugging Face (cabecalho X-Linked-ETag,
/// conferido em 01/10/2026). Vem de fora: calcular aqui o hash do proprio arquivo e
/// conferir contra ele nao provaria nada.
const SHA_TINY_EN: &str = "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f";

fn whisper() -> Option<(PathBuf, PathBuf, String, PathBuf)> {
    let var = |n: &str, p: &str| std::env::var(n).unwrap_or_else(|_| p.into());
    let bin = PathBuf::from(var(
        "PHXCLAW_WHISPER_BIN",
        "/var/tmp/whisper.cpp/build/bin/whisper-cli",
    ));
    let modelo = PathBuf::from(var("PHXCLAW_WHISPER_MODEL", "/var/tmp/ggml-tiny.en.bin"));
    let sha = var("PHXCLAW_WHISPER_MODEL_SHA256", SHA_TINY_EN);
    let audio = PathBuf::from(var(
        "PHXCLAW_E2E_WHISPER_AUDIO",
        "/var/tmp/whisper.cpp/samples/jfk.wav",
    ));
    if bin.is_file() && modelo.is_file() && audio.is_file() {
        Some((bin, modelo, sha, audio))
    } else {
        eprintln!("PULADO: falta whisper-cli, modelo ou audio ({bin:?}, {modelo:?}, {audio:?})");
        None
    }
}

#[tokio::test]
async fn transcribe_ouve_o_jfk() {
    let Some((bin, modelo, sha, audio)) = whisper() else {
        return;
    };
    let c = ctx();
    std::fs::create_dir_all(c.workdir.join("audio")).unwrap();
    std::fs::copy(&audio, c.workdir.join("audio/fala.wav")).unwrap();
    let t = TranscribeTool {
        bin: Some(bin),
        model: Some(modelo),
        model_sha256: Some(sha),
    };
    let r = t.run(json!({"path":"audio/fala.wav"}), &c).await.unwrap();
    assert!(
        r.content
            .to_lowercase()
            .contains("ask not what your country"),
        "{}",
        r.content
    );
}

#[tokio::test]
async fn transcribe_recusa_sem_configuracao_e_com_modelo_adulterado() {
    let c = ctx();
    let nada = TranscribeTool {
        bin: None,
        model: None,
        model_sha256: None,
    };
    let e = nada.run(json!({"path":"a.wav"}), &c).await.unwrap_err();
    let m = e.to_string();
    assert!(matches!(e, ToolError::Denied(_)), "{m}");
    for v in [
        "PHXCLAW_WHISPER_BIN",
        "PHXCLAW_WHISPER_MODEL",
        "PHXCLAW_WHISPER_MODEL_SHA256",
    ] {
        assert!(m.contains(v), "{m}");
    }
    let so_bin = TranscribeTool {
        bin: Some("/bin/true".into()),
        model: None,
        model_sha256: Some(SHA_TINY_EN.into()),
    };
    let m = so_bin
        .run(json!({"path":"a.wav"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        m.contains("faltam PHXCLAW_WHISPER_MODEL") && !m.contains("BIN"),
        "{m}"
    );
    let Some((bin, modelo, _, audio)) = whisper() else {
        return;
    };
    std::fs::copy(&audio, c.workdir.join("a.wav")).unwrap();
    let t = TranscribeTool {
        bin: Some(bin),
        model: Some(modelo),
        model_sha256: Some("0".repeat(64)),
    };
    let m = t
        .run(json!({"path":"a.wav"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("sha256 mismatch"), "{m}");
    for (a, motivo) in [
        (json!({"path":"a.mp3"}), ".wav"),
        (json!({"path":"../a.wav"}), "fora da pasta"),
        (json!({"path":"a.wav","language":"-x"}), "idioma invalido"),
    ] {
        let m = t.run(a.clone(), &c).await.unwrap_err().to_string();
        assert!(m.contains(motivo), "{a}: {m}");
    }
}

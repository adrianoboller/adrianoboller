//! Transcreve um audio pelo WhisperCppProvider do PhxClaw (modelo com SHA-256 conferido).
//! `cargo run -p phxclaw-media-intelligence --example transcrever -- <whisper-cli> <modelo> <sha256> <audio.wav>`

use phxclaw_media_intelligence::{SpeechToTextRequest, WhisperCppProvider};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        return Err("uso: transcrever <whisper-cli> <modelo> <sha256> <audio.wav>".into());
    }
    let p = WhisperCppProvider {
        program: a[0].clone().into(),
        model: a[1].clone().into(),
        model_sha256: a[2].clone(),
        threads: Some(4),
        timeout_seconds: 120,
        max_audio_bytes: 50 * 1024 * 1024,
    };
    p.validate()?;
    println!("modelo conferido: sha256 {}...", &a[2][..16]);
    let saida = std::env::temp_dir().join("phxclaw-demo-stt.txt");
    let t = std::time::Instant::now();
    let texto = p.transcribe_verified(&SpeechToTextRequest {
        input_audio: a[3].clone().into(),
        language: Some("en".into()),
        output_txt: Some(saida),
    })?;
    println!(
        "transcricao ({:.1} s):\n  {}",
        t.elapsed().as_secs_f64(),
        texto.trim()
    );
    Ok(())
}

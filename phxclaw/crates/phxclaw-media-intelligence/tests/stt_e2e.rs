//! E2E do STT com whisper.cpp de verdade e modelo com SHA-256 conferido.
//!
//! O SHA esperado vem de FORA (o cabecalho X-Linked-ETag do LFS do Hugging Face), nao do
//! proprio teste: conferir o modelo contra um hash que o teste calculou nao provaria nada.
//! `PHXCLAW_E2E_WHISPER_BIN=... PHXCLAW_E2E_WHISPER_MODEL=... PHXCLAW_E2E_WHISPER_MODEL_SHA256=...
//!  PHXCLAW_E2E_WHISPER_AUDIO=.../jfk.wav cargo test -p phxclaw-media-intelligence --test stt_e2e -- --ignored`

use phxclaw_media_intelligence::{SpeechToTextRequest, WhisperCppProvider};

fn var(n: &str) -> String {
    std::env::var(n).unwrap_or_else(|_| panic!("{n} nao definido"))
}

fn provider(sha: String) -> WhisperCppProvider {
    WhisperCppProvider {
        program: var("PHXCLAW_E2E_WHISPER_BIN").into(),
        model: var("PHXCLAW_E2E_WHISPER_MODEL").into(),
        model_sha256: sha,
        threads: Some(2),
        timeout_seconds: 120,
        max_audio_bytes: 50 * 1024 * 1024,
    }
}

fn pedido(saida: &str) -> SpeechToTextRequest {
    let dir = std::env::temp_dir().join(format!("phx-stt-{}", std::process::id()));
    SpeechToTextRequest {
        input_audio: var("PHXCLAW_E2E_WHISPER_AUDIO").into(),
        language: Some("en".into()),
        output_txt: Some(dir.join(saida)),
    }
}

#[test]
#[ignore = "requer whisper.cpp e modelo: PHXCLAW_E2E_WHISPER_*"]
fn transcreve_audio_real_com_modelo_verificado() {
    let p = provider(var("PHXCLAW_E2E_WHISPER_MODEL_SHA256"));
    let texto = p
        .transcribe_verified(&pedido("jfk.txt"))
        .unwrap()
        .to_lowercase();
    assert!(
        texto.contains("ask not what your country can do for you"),
        "transcricao inesperada: {texto}"
    );
}

#[test]
#[ignore = "requer whisper.cpp e modelo: PHXCLAW_E2E_WHISPER_*"]
fn modelo_com_hash_errado_e_recusado_antes_de_rodar() {
    let p = provider("0".repeat(64));
    let erro = p.transcribe_verified(&pedido("nao.txt")).unwrap_err();
    assert!(erro.to_string().contains("sha256 mismatch"), "{erro}");
}

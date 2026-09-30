use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    pub input: PathBuf,
    pub languages: Vec<String>,
    pub page_segmentation_mode: Option<u8>,
    pub engine_mode: Option<u8>,
    pub output_txt: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub output_txt: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("external engine failed: {0}")]
    External(String),
    #[error("invalid request: {0}")]
    Invalid(String),
}

/// Reference OCR backend using the Tesseract CLI. The plugin may replace this provider
/// without changing the public PhxClaw OCR contract.
pub fn ocr_tesseract(request: &OcrRequest) -> Result<OcrResult, MediaError> {
    if !request.input.is_file() {
        return Err(MediaError::Invalid(format!(
            "input does not exist: {}",
            request.input.display()
        )));
    }
    let mut command = Command::new("tesseract");
    command.arg(&request.input).arg("stdout");
    if !request.languages.is_empty() {
        command.arg("-l").arg(request.languages.join("+"));
    }
    if let Some(psm) = request.page_segmentation_mode {
        command.arg("--psm").arg(psm.to_string());
    }
    if let Some(oem) = request.engine_mode {
        command.arg("--oem").arg(oem.to_string());
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(MediaError::External(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    if let Some(path) = &request.output_txt {
        fs::write(path, &text)?;
    }
    Ok(OcrResult {
        text,
        output_txt: request.output_txt.clone(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalSpeechProvider {
    pub program: String,
    pub fixed_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToSpeechRequest {
    pub text: String,
    pub voice: Option<String>,
    pub language: Option<String>,
    pub output_audio: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechToTextRequest {
    pub input_audio: PathBuf,
    pub language: Option<String>,
    pub output_txt: Option<PathBuf>,
}

impl ExternalSpeechProvider {
    /// Executes a TTS provider whose argument list may use {text}, {voice}, {lang}, {output}.
    pub fn synthesize(&self, request: &TextToSpeechRequest) -> Result<(), MediaError> {
        let output_text = path_text(request.output_audio.as_deref());
        let args = render_args(
            &self.fixed_args,
            &[
                ("{text}", request.text.as_str()),
                ("{voice}", request.voice.as_deref().unwrap_or("")),
                ("{lang}", request.language.as_deref().unwrap_or("")),
                ("{output}", output_text.as_str()),
            ],
        );
        run_external(&self.program, &args)
    }

    /// Executes an STT provider such as whisper.cpp. Arguments may use {input}, {lang}, {output}.
    pub fn transcribe(&self, request: &SpeechToTextRequest) -> Result<Option<String>, MediaError> {
        if !request.input_audio.is_file() {
            return Err(MediaError::Invalid("audio input not found".into()));
        }
        let output_text = path_text(request.output_txt.as_deref());
        let input_text = request.input_audio.to_string_lossy().into_owned();
        let args = render_args(
            &self.fixed_args,
            &[
                ("{input}", input_text.as_str()),
                ("{lang}", request.language.as_deref().unwrap_or("")),
                ("{output}", output_text.as_str()),
            ],
        );
        run_external(&self.program, &args)?;
        match &request.output_txt {
            Some(path) if path.is_file() => Ok(Some(fs::read_to_string(path)?)),
            _ => Ok(None),
        }
    }
}

fn run_external(program: &str, args: &[String]) -> Result<(), MediaError> {
    let output = Command::new(program).args(args).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(MediaError::External(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}

fn render_args(args: &[String], values: &[(&str, &str)]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            let mut rendered = arg.clone();
            for (from, to) in values {
                rendered = rendered.replace(from, to);
            }
            rendered
        })
        .collect()
}

fn path_text(path: Option<&Path>) -> String {
    path.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperCppProvider {
    pub program: PathBuf,
    pub model: PathBuf,
    pub model_sha256: String,
    pub threads: Option<u16>,
    pub timeout_seconds: u64,
    pub max_audio_bytes: u64,
}

impl WhisperCppProvider {
    pub fn validate(&self) -> Result<(), MediaError> {
        use sha2::{Digest, Sha256};
        if !self.program.is_file() {
            return Err(MediaError::Invalid(
                "whisper.cpp executable not found".into(),
            ));
        }
        if !self.model.is_file() {
            return Err(MediaError::Invalid("whisper.cpp model not found".into()));
        }
        if self.model_sha256.len() != 64
            || !self.model_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(MediaError::Invalid(
                "model sha256 must be 64 hex chars".into(),
            ));
        }
        let actual = Sha256::digest(fs::read(&self.model)?);
        let actual_hex = actual
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if !actual_hex.eq_ignore_ascii_case(&self.model_sha256) {
            return Err(MediaError::Invalid(
                "whisper.cpp model sha256 mismatch".into(),
            ));
        }
        if self.timeout_seconds == 0 || self.timeout_seconds > 7200 {
            return Err(MediaError::Invalid("whisper timeout outside policy".into()));
        }
        if self.max_audio_bytes == 0 {
            return Err(MediaError::Invalid(
                "max_audio_bytes must be positive".into(),
            ));
        }
        Ok(())
    }

    pub fn transcribe_verified(&self, request: &SpeechToTextRequest) -> Result<String, MediaError> {
        use std::{
            process::Stdio,
            thread,
            time::{Duration, Instant},
        };
        self.validate()?;
        if !request.input_audio.is_file() {
            return Err(MediaError::Invalid("audio input not found".into()));
        }
        let size = fs::metadata(&request.input_audio)?.len();
        if size > self.max_audio_bytes {
            return Err(MediaError::Invalid(
                "audio input exceeds configured size limit".into(),
            ));
        }
        let output_txt = request.output_txt.clone().ok_or_else(|| {
            MediaError::Invalid("whisper.cpp provider requires output_txt".into())
        })?;
        if let Some(parent) = output_txt.parent() {
            fs::create_dir_all(parent)?;
        }
        let output_base = if output_txt.extension().and_then(|x| x.to_str()) == Some("txt") {
            output_txt.with_extension("")
        } else {
            output_txt.clone()
        };
        let mut command = Command::new(&self.program);
        command
            .arg("-m")
            .arg(&self.model)
            .arg("-f")
            .arg(&request.input_audio)
            .arg("-otxt")
            .arg("-of")
            .arg(&output_base)
            .arg("-np");
        if let Some(language) = &request.language {
            command.arg("-l").arg(language);
        }
        if let Some(threads) = self.threads {
            if threads > 0 {
                command.arg("-t").arg(threads.to_string());
            }
        }
        command.stdout(Stdio::null()).stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    let output = child.wait_with_output()?;
                    return Err(MediaError::External(
                        String::from_utf8_lossy(&output.stderr).into_owned(),
                    ));
                }
                break;
            }
            if started.elapsed() >= Duration::from_secs(self.timeout_seconds) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MediaError::External("whisper.cpp timed out".into()));
            }
            thread::sleep(Duration::from_millis(50));
        }
        let generated = output_base.with_extension("txt");
        if !generated.is_file() {
            return Err(MediaError::External(
                "whisper.cpp did not create expected text output".into(),
            ));
        }
        if generated != output_txt {
            fs::rename(&generated, &output_txt)?;
        }
        Ok(fs::read_to_string(output_txt)?)
    }
}

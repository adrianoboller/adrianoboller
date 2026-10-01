//! Voz: falar (texto para WAV), ouvir gatilhos num WAV e conversar por voz, turno a turno.
//!
//! Decisoes que valem saber:
//! - **o motor de fala e um comando externo**, com marcadores `{{Text}}`, `{{OutputPath}}` e
//!   `{{Model}}`: o PhxClaw nao liga biblioteca de voz nenhuma, e quem escolhe o motor (e a
//!   licenca dele) e o operador. Medido em 01/10/2026: o `sherpa-onnx-offline-tts` 1.13.8
//!   traz o espeak-ng (GPL-3) ligado estaticamente, entao embuti-lo ou baixa-lo pelo
//!   instalador e decisao de produto, nao de codigo;
//! - **modelo so roda com o SHA-256 declarado conferido**, pelo mesmo conferidor do
//!   whisper (`phxclaw_media_intelligence::verify_sha256`);
//! - **todo programa roda no bwrap** pelo `visao::isolado_com`: sem rede, ambiente limpo, o
//!   binario e o modelo so leitura, e a saida numa pasta temporaria que so vira arquivo da
//!   tarefa depois de o cabecalho WAV ser conferido -- WAV truncado nao chega ao usuario;
//! - **os marcadores se trocam numa passada so**: o texto vem do modelo, e uma troca em
//!   sequencia deixaria um `{{OutputPath}}` escrito NO texto escolher onde gravar.

use crate::motor::{Agent, CancelFlag, Observer, artifact_for};
use crate::tarefa::{Task, TaskStatus, confine};
use crate::visao::{PastaTemp, TranscribeTool, isolado_com};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Caracteres por fala: uma resposta falada longa demais e um audiolivro, nao uma resposta.
pub const MAX_FALA: usize = 2_000;
/// Teto de gatilhos por chamada e de caracteres por gatilho.
pub const MAX_GATILHOS: usize = 32;
pub const MAX_GATILHO: usize = 64;
/// WAV maior que isto nao se le para a memoria so para conferir o cabecalho.
const MAX_WAV: u64 = 256 * 1024 * 1024;

fn falha(e: impl std::fmt::Display) -> ToolError {
    ToolError::Failed(e.to_string())
}

fn var(n: &str) -> Option<String> {
    std::env::var(n).ok().filter(|v| !v.trim().is_empty())
}

// ---------------------------------------------------------------- WAV

/// O que o cabecalho de um WAV diz.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoWav {
    pub taxa: u32,
    pub canais: u16,
    pub bits: u16,
    /// 1 = PCM inteiro, 3 = ponto flutuante, 0xFFFE = extensivel.
    pub formato: u16,
    pub segundos: f64,
}

/// Le o cabecalho RIFF/WAVE a mao (zero dependencia nova) e calcula a duracao pelo tamanho
/// do bloco `data`. Quem grava em fluxo poe 0xFFFFFFFF no tamanho; ai vale o que existe.
pub fn info_wav(b: &[u8]) -> Result<InfoWav, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("nao e WAV (falta RIFF/WAVE)".into());
    }
    let u16_em = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_em = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut i = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let tam = u32_em(i + 4) as usize;
        let corpo = i + 8;
        if id == b"fmt " {
            if tam < 16 || corpo + 16 > b.len() {
                return Err("bloco fmt curto".into());
            }
            fmt = Some((
                u16_em(corpo),
                u16_em(corpo + 2),
                u32_em(corpo + 4),
                u16_em(corpo + 14),
            ));
        } else if id == b"data" {
            let (formato, canais, taxa, bits) = fmt.ok_or("bloco data antes do fmt")?;
            if canais == 0 || taxa == 0 || bits == 0 {
                return Err("fmt com zero canais, taxa ou bits".into());
            }
            let dados = tam.min(b.len() - corpo);
            let por_seg = f64::from(taxa) * f64::from(canais) * f64::from(bits) / 8.0;
            return Ok(InfoWav {
                taxa,
                canais,
                bits,
                formato,
                segundos: dados as f64 / por_seg,
            });
        }
        // Blocos tem tamanho par: o byte de enchimento nao entra no tamanho declarado.
        i = corpo.saturating_add(tam).saturating_add(tam & 1);
    }
    Err("WAV sem bloco data".into())
}

// ---------------------------------------------------------------- comando externo

/// Troca os marcadores numa passada so, da esquerda para a direita, sem reler o que ja foi
/// trocado: o valor de um marcador nunca vira outro marcador.
fn preencher(token: &str, valores: &[(&str, &str)]) -> String {
    let mut saida = String::new();
    let mut resto = token;
    loop {
        let proximo = valores
            .iter()
            .filter_map(|(m, v)| resto.find(m).map(|i| (i, *m, *v)))
            .min_by_key(|(i, _, _)| *i);
        match proximo {
            Some((i, m, v)) => {
                saida.push_str(&resto[..i]);
                saida.push_str(v);
                resto = &resto[i + m.len()..];
            }
            None => {
                saida.push_str(resto);
                return saida;
            }
        }
    }
}

/// Pastas do hospedeiro montadas so leitura NO MESMO caminho dentro do sandbox, para o
/// comando configurado valer igual dentro e fora. So caminho absoluto e existente: um
/// relativo dependeria do diretorio de quem lancou o servidor.
fn pastas_no_mesmo_lugar(pastas: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, ToolError> {
    let mut v: Vec<(PathBuf, String)> = Vec::new();
    for p in pastas {
        if !p.is_absolute() || !p.is_dir() {
            return Err(ToolError::Denied(format!(
                "pasta de voz invalida (tem de ser absoluta e existir): {}",
                p.display()
            )));
        }
        let s = p.display().to_string();
        if !v.iter().any(|(_, g)| *g == s) {
            v.push((p.clone(), s));
        }
    }
    Ok(v)
}

fn lista_de_pastas(v: Option<String>) -> Vec<PathBuf> {
    v.map(|s| {
        s.split(':')
            .filter(|p| !p.trim().is_empty())
            .map(|p| PathBuf::from(p.trim()))
            .collect()
    })
    .unwrap_or_default()
}

/// Recusa de saida diferente de zero, com o fim do stderr. O 127 e o shell do sandbox
/// dizendo que nao achou o programa: quase sempre a pasta dele nao foi montada.
fn recusa_de_saida(quem: &str, var_pastas: &str, r: &phxclaw_sandbox::WorkdirOutput) -> ToolError {
    if r.exit_code == Some(127) {
        return ToolError::Failed(format!(
            "{quem} nao executou dentro do sandbox (saida 127): ponha a pasta do programa e \
das bibliotecas dele em {var_pastas}"
        ));
    }
    let fim: Vec<&str> = r.stderr.lines().rev().take(4).collect();
    ToolError::Failed(format!(
        "{quem} falhou (saida {}): {}",
        r.exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "por sinal".into()),
        fim.into_iter().rev().collect::<Vec<_>>().join(" | ")
    ))
}

// ---------------------------------------------------------------- falar

/// Texto para fala por comando externo. Exemplo com o flite (licenca BSD da CMU):
/// `PHXCLAW_TTS_COMMAND="env LD_LIBRARY_PATH=/opt/flite/lib /opt/flite/bin/flite -voice
/// {{Model}} -t {{Text}} -o {{OutputPath}}"`.
pub struct SpeakTool {
    /// Linha do comando, separada por espaco; cada pedaco vira UM argumento.
    pub comando: Option<String>,
    pub modelo: Option<PathBuf>,
    pub modelo_sha256: Option<String>,
    /// Pastas extras (binario, bibliotecas) montadas so leitura no mesmo caminho.
    pub pastas: Vec<PathBuf>,
}

/// Fala gravada e conferida.
#[derive(Debug, Clone)]
pub struct Fala {
    pub caminho: PathBuf,
    pub wav: InfoWav,
}

impl SpeakTool {
    pub fn from_env() -> Self {
        Self {
            comando: var("PHXCLAW_TTS_COMMAND"),
            modelo: var("PHXCLAW_TTS_MODEL").map(PathBuf::from),
            modelo_sha256: var("PHXCLAW_TTS_MODEL_SHA256"),
            pastas: lista_de_pastas(var("PHXCLAW_TTS_DIRS")),
        }
    }

    fn configurado(&self) -> Result<(Vec<String>, PathBuf, String), ToolError> {
        let faltam: Vec<&str> = [
            ("PHXCLAW_TTS_COMMAND", self.comando.is_none()),
            ("PHXCLAW_TTS_MODEL", self.modelo.is_none()),
            ("PHXCLAW_TTS_MODEL_SHA256", self.modelo_sha256.is_none()),
        ]
        .into_iter()
        .filter(|(_, f)| *f)
        .map(|(n, _)| n)
        .collect();
        if !faltam.is_empty() {
            return Err(ToolError::Denied(format!(
                "texto para fala nao configurado: faltam {}",
                faltam.join(", ")
            )));
        }
        let modelo_cmd: Vec<String> = self
            .comando
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(String::from)
            .collect();
        for m in ["{{Text}}", "{{OutputPath}}"] {
            if !modelo_cmd.iter().any(|a| a.contains(m)) {
                return Err(ToolError::Denied(format!(
                    "PHXCLAW_TTS_COMMAND sem o marcador {m}"
                )));
            }
        }
        Ok((
            modelo_cmd,
            self.modelo.clone().unwrap_or_default(),
            self.modelo_sha256.clone().unwrap_or_default(),
        ))
    }

    /// Fala `texto` em `saida` (relativo a `workdir`). A ferramenta e a conversa por voz
    /// passam por aqui.
    pub async fn falar(
        &self,
        texto: &str,
        workdir: &Path,
        saida: &str,
        prazo: Duration,
    ) -> Result<Fala, ToolError> {
        let (modelo_cmd, modelo, sha) = self.configurado()?;
        // Controle vira espaco: quebra de linha e tabulacao nao mudam o que se fala, e um
        // NUL no meio do argumento o cortaria.
        let mut limpo: String = texto
            .trim()
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        if limpo.is_empty() {
            return Err(ToolError::InvalidArguments("texto vazio".into()));
        }
        let n = limpo.chars().count();
        if n > MAX_FALA {
            return Err(ToolError::InvalidArguments(format!(
                "texto com {n} caracteres, acima do teto de {MAX_FALA}"
            )));
        }
        // Um motor que recebe o texto como argumento solto leria "-x" como opcao dele.
        if limpo.starts_with('-') {
            limpo.insert(0, ' ');
        }
        if !saida.to_ascii_lowercase().ends_with(".wav") {
            return Err(ToolError::InvalidArguments(format!(
                "a saida tem de terminar em .wav: {saida}"
            )));
        }
        let alvo = confine(workdir, saida).map_err(ToolError::Denied)?;
        if !modelo.is_file() {
            return Err(ToolError::Denied(format!(
                "modelo de voz nao encontrado: {}",
                modelo.display()
            )));
        }
        let m = modelo.clone();
        tokio::task::spawn_blocking(move || phxclaw_media_intelligence::verify_sha256(&m, &sha))
            .await
            .map_err(falha)?
            .map_err(|e| ToolError::Denied(format!("modelo de voz recusado: {e}")))?;
        let modelo_txt = modelo.display().to_string();
        let argv: Vec<String> = modelo_cmd
            .iter()
            .map(|a| {
                preencher(
                    a,
                    &[
                        ("{{Text}}", limpo.as_str()),
                        ("{{OutputPath}}", "/work/fala.wav"),
                        ("{{Model}}", modelo_txt.as_str()),
                    ],
                )
            })
            .collect();
        let mut pastas = self.pastas.clone();
        if let Some(d) = modelo.parent() {
            pastas.push(d.to_path_buf());
        }
        let binds = pastas_no_mesmo_lugar(&pastas)?;
        let tmp = PastaTemp::nova("phx-tts")?;
        let r = isolado_com(&argv, binds, vec![], &tmp.0, prazo).await?;
        if r.exit_code != Some(0) {
            return Err(recusa_de_saida("o motor de voz", "PHXCLAW_TTS_DIRS", &r));
        }
        let gerado = tmp.0.join("fala.wav");
        let tam = std::fs::metadata(&gerado)
            .map_err(|_| ToolError::Failed("o motor de voz saiu 0 sem gravar o WAV".into()))?
            .len();
        if tam > MAX_WAV {
            return Err(ToolError::Failed(format!(
                "o motor de voz gravou {tam} bytes, acima do teto de {MAX_WAV}"
            )));
        }
        let bytes = std::fs::read(&gerado).map_err(falha)?;
        let wav = info_wav(&bytes).map_err(|e| {
            ToolError::Failed(format!("o motor de voz gravou um WAV invalido: {e}"))
        })?;
        if wav.segundos <= 0.0 {
            return Err(ToolError::Failed(
                "o motor de voz gravou um WAV sem amostras".into(),
            ));
        }
        if let Some(d) = alvo.parent() {
            std::fs::create_dir_all(d).map_err(falha)?;
        }
        std::fs::write(&alvo, &bytes).map_err(falha)?;
        Ok(Fala { caminho: alvo, wav })
    }
}

impl Tool for SpeakTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "speak".into(),
            description: "Text to speech: synthesize 'text' into a .wav file in the task folder \
(default fala.wav) with the configured local voice."
                .into(),
            parameters: json!({"type":"object","properties":{
                "text":{"type":"string"},
                "output":{"type":"string","description":".wav path in the task folder"}
            },"required":["text"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "media.tts"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.configurado()?;
            let texto = args
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'text'".into()))?;
            let saida = args
                .get("output")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("fala.wav");
            let f = self.falar(texto, &ctx.workdir, saida, ctx.timeout).await?;
            Ok(ToolOutput {
                content: format!(
                    "fala gravada em {saida}: {:.2} s, {} Hz, {} canal(is)",
                    f.wav.segundos, f.wav.taxa, f.wav.canais
                ),
                artifacts: vec![artifact_for(&ctx.workdir, saida).map_err(falha)?],
            })
        })
    }
}

// ---------------------------------------------------------------- gatilho (palavra de despertar)

/// Arquivos do modelo de palavra-chave, na ordem em que entram no SHA-256 declarado.
pub const ARQUIVOS_KWS: [&str; 5] = [
    "encoder.onnx",
    "decoder.onnx",
    "joiner.onnx",
    "tokens.txt",
    "cmudict.dict",
];

/// O SHA-256 que cobre os cinco arquivos do modelo de uma vez: o de
/// `sha256sum encoder.onnx decoder.onnx joiner.onnx tokens.txt cmudict.dict | sha256sum`.
/// Um hash por arquivo seriam cinco variaveis para errar; um so, sobre a lista, prende o
/// conjunto -- inclusive o lexico, que decide o que cada gatilho quer dizer.
pub fn sha256_do_modelo_kws(pasta: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut lista = String::new();
    for a in ARQUIVOS_KWS {
        let h = phxclaw_media_intelligence::sha256_file_hex(&pasta.join(a))
            .map_err(|e| format!("{a}: {e}"))?;
        lista.push_str(&format!("{h}  {a}\n"));
    }
    Ok(Sha256::digest(lista.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Gatilho aceito: letras ASCII, espaco, apostrofo e hifen. E o alfabeto do lexico CMU;
/// qualquer outro caractere nao teria pronuncia.
fn gatilho_valido(g: &str) -> bool {
    !g.trim().is_empty()
        && g.chars().count() <= MAX_GATILHO
        && g.chars()
            .all(|c| c.is_ascii_alphabetic() || c == ' ' || c == '\'' || c == '-')
        && g.chars().any(|c| c.is_ascii_alphabetic())
}

/// Fonemas de cada gatilho pelo lexico CMU (`palavra F O N E M A S`, a primeira pronuncia
/// vale). Palavra fora do lexico e fonema fora do `tokens.txt` recusam dizendo qual: um
/// gatilho que o modelo nao sabe ouvir nao pode virar "nunca disparou".
pub fn fonemas_dos_gatilhos(
    gatilhos: &[String],
    lexico: &str,
    tokens: &str,
) -> Result<Vec<Vec<String>>, String> {
    let palavras: BTreeSet<String> = gatilhos
        .iter()
        .flat_map(|g| g.split([' ', '-']).filter(|p| !p.is_empty()))
        .map(str::to_ascii_lowercase)
        .collect();
    let mut pron: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for linha in lexico.lines() {
        let linha = linha.split('#').next().unwrap_or("");
        let mut it = linha.split_whitespace();
        let Some(p) = it.next() else { continue };
        if palavras.contains(p) && !pron.contains_key(p) {
            pron.insert(p.to_string(), it.map(String::from).collect());
        }
    }
    let conhecidos: BTreeSet<&str> = tokens
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    let fora: Vec<&String> = palavras.iter().filter(|p| !pron.contains_key(*p)).collect();
    if !fora.is_empty() {
        return Err(format!(
            "palavra(s) sem pronuncia no lexico: {}",
            fora.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut v = Vec::new();
    for g in gatilhos {
        let mut f = Vec::new();
        for p in g.split([' ', '-']).filter(|p| !p.is_empty()) {
            for fon in &pron[&p.to_ascii_lowercase()] {
                if !conhecidos.contains(fon.as_str()) {
                    return Err(format!(
                        "fonema {fon} de '{p}' nao existe no tokens.txt do modelo"
                    ));
                }
                f.push(fon.clone());
            }
        }
        v.push(f);
    }
    Ok(v)
}

/// Um gatilho ouvido.
#[derive(Debug, Clone, PartialEq)]
pub struct Deteccao {
    pub gatilho: String,
    /// Segundo do primeiro fonema no audio.
    pub segundo: f64,
}

/// Deteccao de palavra-chave (sherpa-onnx, transdutor em fluxo) num WAV mono de 16 bits.
pub struct WakeWordTool {
    /// `sherpa-onnx-keyword-spotter`; a `libonnxruntime.so` mora na mesma pasta.
    pub bin: Option<PathBuf>,
    /// Pasta com os cinco `ARQUIVOS_KWS`.
    pub pasta_modelo: Option<PathBuf>,
    pub modelo_sha256: Option<String>,
}

impl WakeWordTool {
    pub fn from_env() -> Self {
        Self {
            bin: var("PHXCLAW_KWS_BIN").map(PathBuf::from),
            pasta_modelo: var("PHXCLAW_KWS_MODEL_DIR").map(PathBuf::from),
            modelo_sha256: var("PHXCLAW_KWS_MODEL_SHA256"),
        }
    }

    fn configurado(&self) -> Result<(PathBuf, PathBuf, String), ToolError> {
        let faltam: Vec<&str> = [
            ("PHXCLAW_KWS_BIN", self.bin.is_none()),
            ("PHXCLAW_KWS_MODEL_DIR", self.pasta_modelo.is_none()),
            ("PHXCLAW_KWS_MODEL_SHA256", self.modelo_sha256.is_none()),
        ]
        .into_iter()
        .filter(|(_, f)| *f)
        .map(|(n, _)| n)
        .collect();
        if !faltam.is_empty() {
            return Err(ToolError::Denied(format!(
                "deteccao de gatilho nao configurada: faltam {}",
                faltam.join(", ")
            )));
        }
        Ok((
            self.bin.clone().unwrap_or_default(),
            self.pasta_modelo.clone().unwrap_or_default(),
            self.modelo_sha256.clone().unwrap_or_default(),
        ))
    }

    /// Os gatilhos ouvidos em `audio`, na ordem do audio.
    pub async fn ouvir(
        &self,
        audio: &Path,
        gatilhos: &[String],
        limiar: f64,
        prazo: Duration,
    ) -> Result<Vec<Deteccao>, ToolError> {
        let (bin, pasta, sha) = self.configurado()?;
        if gatilhos.is_empty() || gatilhos.len() > MAX_GATILHOS {
            return Err(ToolError::InvalidArguments(format!(
                "de 1 a {MAX_GATILHOS} gatilhos, vieram {}",
                gatilhos.len()
            )));
        }
        if let Some(g) = gatilhos.iter().find(|g| !gatilho_valido(g)) {
            return Err(ToolError::InvalidArguments(format!(
                "gatilho invalido: '{g}' (letras, espaco, ' e -, ate {MAX_GATILHO} caracteres)"
            )));
        }
        let tam = std::fs::metadata(audio).map_err(falha)?.len();
        if tam > MAX_WAV {
            return Err(ToolError::InvalidArguments(format!(
                "audio com {tam} bytes, acima do teto de {MAX_WAV}"
            )));
        }
        let info =
            info_wav(&std::fs::read(audio).map_err(falha)?).map_err(ToolError::InvalidArguments)?;
        if info.canais != 1 || info.bits != 16 || info.formato != 1 {
            return Err(ToolError::InvalidArguments(format!(
                "o detector le WAV mono PCM de 16 bits; este tem {} canal(is), {} bits \
(converta: ffmpeg -i entrada -ac 1 -ar 16000 -sample_fmt s16 saida.wav)",
                info.canais, info.bits
            )));
        }
        if !bin.is_file() {
            return Err(ToolError::Denied(format!(
                "detector nao encontrado: {}",
                bin.display()
            )));
        }
        let p = pasta.clone();
        let real = tokio::task::spawn_blocking(move || sha256_do_modelo_kws(&p))
            .await
            .map_err(falha)?
            .map_err(|e| ToolError::Denied(format!("modelo de gatilho ilegivel: {e}")))?;
        if !real.eq_ignore_ascii_case(sha.trim()) {
            return Err(ToolError::Denied(format!(
                "modelo de gatilho recusado: sha256 mismatch (a pasta da {real})"
            )));
        }
        let lexico = std::fs::read_to_string(pasta.join("cmudict.dict")).map_err(falha)?;
        let tokens = std::fs::read_to_string(pasta.join("tokens.txt")).map_err(falha)?;
        let fonemas = fonemas_dos_gatilhos(gatilhos, &lexico, &tokens)
            .map_err(ToolError::InvalidArguments)?;
        // O rotulo e o indice, nao o texto: o detector devolve o rotulo, e o indice volta ao
        // gatilho exato que o usuario escreveu.
        let arquivo: String = fonemas
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{} @g{i}\n", f.join(" ")))
            .collect();
        let tmp = PastaTemp::nova("phx-kws")?;
        std::fs::write(tmp.0.join("gatilhos.txt"), arquivo).map_err(falha)?;
        let entrada = audio
            .parent()
            .ok_or_else(|| ToolError::InvalidArguments("audio sem pasta".into()))?;
        let nome = audio
            .file_name()
            .ok_or_else(|| ToolError::InvalidArguments("audio sem nome".into()))?
            .to_string_lossy()
            .into_owned();
        let pd = pasta.display().to_string();
        let argv = vec![
            bin.display().to_string(),
            "--print-args=false".into(),
            format!("--tokens={pd}/tokens.txt"),
            format!("--encoder={pd}/encoder.onnx"),
            format!("--decoder={pd}/decoder.onnx"),
            format!("--joiner={pd}/joiner.onnx"),
            "--keywords-file=/work/gatilhos.txt".into(),
            format!("--keywords-threshold={limiar}"),
            "--num-threads=1".into(),
            format!("/in/{nome}"),
        ];
        let bin_dir = bin
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| ToolError::Denied("detector sem pasta".into()))?;
        let mut binds = pastas_no_mesmo_lugar(&[bin_dir, pasta.clone()])?;
        binds.push((entrada.to_path_buf(), "/in".into()));
        let r = isolado_com(&argv, binds, vec![], &tmp.0, prazo).await?;
        if r.exit_code != Some(0) {
            return Err(recusa_de_saida(
                "o detector de gatilho",
                "PHXCLAW_KWS_BIN",
                &r,
            ));
        }
        let mut v = Vec::new();
        for l in r.stdout.lines().filter(|l| l.trim_start().starts_with('{')) {
            let Ok(j) = serde_json::from_str::<Value>(l) else {
                continue;
            };
            let i = j
                .get("keyword")
                .and_then(Value::as_str)
                .and_then(|k| k.strip_prefix('g'))
                .and_then(|n| n.parse::<usize>().ok());
            let Some(g) = i.and_then(|i| gatilhos.get(i)) else {
                continue;
            };
            let s = j
                .get("timestamps")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_f64)
                .or_else(|| j.get("start_time").and_then(Value::as_f64))
                .unwrap_or(0.0);
            v.push(Deteccao {
                gatilho: g.clone(),
                segundo: s,
            });
        }
        Ok(v)
    }
}

impl Tool for WakeWordTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "wake_word".into(),
            description: "Detect wake words / keywords (English) in a mono 16-bit .wav of the \
task folder, streamed through a local keyword spotter. 'keywords': up to 32 phrases of up to 64 \
characters (letters, space, apostrophe, hyphen). Returns each detection with its time."
                .into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "keywords":{"type":"array","items":{"type":"string"}},
                "threshold":{"type":"number","description":"0.05 to 0.95, default 0.25"}
            },"required":["path","keywords"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "media.wake"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.configurado()?;
            let rel = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            let gatilhos: Vec<String> = args
                .get("keywords")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'keywords'".into()))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.trim().to_string())
                        .ok_or_else(|| ToolError::InvalidArguments("gatilho nao e texto".into()))
                })
                .collect::<Result<_, _>>()?;
            let limiar = args
                .get("threshold")
                .and_then(Value::as_f64)
                .unwrap_or(0.25)
                .clamp(0.05, 0.95);
            let audio = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            if !audio.is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "arquivo nao encontrado na pasta da tarefa: {rel}"
                )));
            }
            let d = self.ouvir(&audio, &gatilhos, limiar, ctx.timeout).await?;
            Ok(ToolOutput::text(if d.is_empty() {
                format!("nenhum gatilho ouvido em {rel}")
            } else {
                format!(
                    "{} gatilho(s) ouvido(s) em {rel}:\n{}",
                    d.len(),
                    d.iter()
                        .map(|x| format!("- '{}' em {:.2} s", x.gatilho, x.segundo))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }))
        })
    }
}

// ---------------------------------------------------------------- conversa por voz

/// Um turno da conversa: o que se ouviu, o que o agente respondeu e onde a fala ficou.
#[derive(Debug, Clone)]
pub struct TurnoDeVoz {
    pub tarefa: String,
    pub ouvido: String,
    pub resposta: String,
    pub estado: TaskStatus,
    pub wav: Option<PathBuf>,
    pub erro: Option<String>,
}

/// O agente da montagem ajustado para conversa: a resposta em texto, sem ferramenta, ja e
/// o fim do turno. A montagem exige `final_answer` e recusa terminar sem ter usado
/// ferramenta -- certo para "crie o relatorio", errado para "qual a capital da Franca?",
/// onde cada turno pagaria duas recusas ao modelo antes de falar.
pub fn agente_de_conversa(mut agente: Agent) -> Agent {
    agente.config.require_final_tool = false;
    agente
}

/// Turnos anteriores que entram no objetivo do proximo: o bastante para "e amanha?" fazer
/// sentido, sem o historico inteiro crescer a cada turno.
const TURNOS_DE_CONTEXTO: usize = 6;

/// WAV -> whisper -> agente -> fala -> WAV, um turno por arquivo, pelo MESMO motor das
/// ferramentas: `TranscribeTool::transcrever` e `SpeakTool::falar` sao o que o `transcribe` e
/// o `speak` chamam, e o agente e o da montagem de sempre. Cada turno e uma tarefa, com a
/// pergunta e a resposta faladas na pasta dela.
/// `observador` faz um observador por turno: cada turno e uma tarefa nova, com os passos
/// contados do zero.
pub async fn conversar<O: Observer>(
    agente: &Agent,
    modelo: &str,
    wavs: &[PathBuf],
    ouvir: &TranscribeTool,
    falar: &SpeakTool,
    observador: impl Fn() -> O,
) -> Vec<TurnoDeVoz> {
    let mut turnos: Vec<TurnoDeVoz> = Vec::new();
    for wav in wavs {
        let mut t = Task::new("(conversa por voz)", modelo);
        let workdir = agente.store.workdir(&t.id);
        let mut turno = TurnoDeVoz {
            tarefa: t.id.clone(),
            ouvido: String::new(),
            resposta: String::new(),
            estado: TaskStatus::Failed,
            wav: None,
            erro: None,
        };
        let prazo = agente.config.tool_timeout;
        let r: Result<(), String> = async {
            std::fs::create_dir_all(&workdir).map_err(|e| e.to_string())?;
            let pergunta = workdir.join("pergunta.wav");
            std::fs::copy(wav, &pergunta).map_err(|e| format!("{}: {e}", wav.display()))?;
            let ouvido = ouvir
                .transcrever(pergunta, None, prazo)
                .await
                .map_err(|e| e.to_string())?;
            let ouvido = ouvido.split_whitespace().collect::<Vec<_>>().join(" ");
            if ouvido.is_empty() {
                return Err("nenhuma fala reconhecida no audio".into());
            }
            turno.ouvido = ouvido.clone();
            let mut objetivo = String::from(
                "Voice conversation: reply briefly in plain text (no markdown, no lists), it \
will be spoken aloud.\n",
            );
            let ini = turnos.len().saturating_sub(TURNOS_DE_CONTEXTO);
            for a in turnos[ini..].iter().filter(|a| a.erro.is_none()) {
                objetivo.push_str(&format!("User: {}\nYou: {}\n", a.ouvido, a.resposta));
            }
            objetivo.push_str(&format!("User: {ouvido}"));
            t.objective = objetivo;
            let fim = agente.run(t, &CancelFlag::default(), &observador()).await;
            turno.estado = fim.status;
            let resposta = match (&fim.status, &fim.answer) {
                (TaskStatus::Completed, Some(a)) if !a.trim().is_empty() => a.trim().to_string(),
                _ => {
                    return Err(fim
                        .error
                        .clone()
                        .unwrap_or_else(|| format!("o agente terminou em {:?}", fim.status)));
                }
            };
            turno.resposta = resposta.clone();
            // A fala tem teto; a resposta longa se corta no fim de uma frase antes dele.
            let falada = cortar_para_falar(&resposta, MAX_FALA);
            let f = falar
                .falar(&falada, &workdir, "resposta.wav", prazo)
                .await
                .map_err(|e| e.to_string())?;
            turno.wav = Some(f.caminho);
            Ok(())
        }
        .await;
        if let Err(e) = r {
            turno.erro = Some(e);
        }
        turnos.push(turno);
    }
    turnos
}

/// Corta no ultimo fim de frase antes do teto (ou no teto, se nao houver).
fn cortar_para_falar(s: &str, teto: usize) -> String {
    if s.chars().count() <= teto {
        return s.to_string();
    }
    let corte: String = s.chars().take(teto).collect();
    match corte.rfind(['.', '!', '?']) {
        Some(i) if i > teto / 2 => corte[..=i].to_string(),
        _ => corte,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn marcador_no_texto_nao_vira_marcador() {
        let v = [
            ("{{Text}}", "oi {{OutputPath}}"),
            ("{{OutputPath}}", "/work/x.wav"),
        ];
        assert_eq!(
            preencher("--text={{Text}}", &v),
            "--text=oi {{OutputPath}}",
            "o valor do texto foi relido"
        );
        assert_eq!(preencher("{{OutputPath}}", &v), "/work/x.wav");
    }

    #[test]
    fn gatilho_com_fonema_que_o_modelo_nao_conhece_recusa() {
        let lexico = "wake W EY1 K\nup AH1 P # comentario\n";
        let tokens = "<blk> 0\nW 1\nK 2\nAH1 3\nP 4\n";
        let e = fonemas_dos_gatilhos(&["wake up".into()], lexico, tokens).unwrap_err();
        assert!(e.contains("EY1"), "{e}");
        let tokens = format!("{tokens}EY1 5\n");
        let f = fonemas_dos_gatilhos(&["wake up".into()], lexico, &tokens).unwrap();
        assert_eq!(f, vec![vec!["W", "EY1", "K", "AH1", "P"]]);
        let e = fonemas_dos_gatilhos(&["wake zzq".into()], lexico, &tokens).unwrap_err();
        assert!(e.contains("zzq"), "{e}");
    }

    #[test]
    fn corte_da_fala_para_no_fim_da_frase() {
        let s = format!("{}. resto", "a".repeat(80));
        assert_eq!(cortar_para_falar(&s, 85), format!("{}.", "a".repeat(80)));
        assert_eq!(cortar_para_falar("curta", 85), "curta");
    }
}

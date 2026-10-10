//! A rota REST de transcricao de voz (`POST /v1/voz/transcrever`) pelo caminho real: o
//! motor sai do `TranscribeTool` que o `transcribe` ja usa, e nao de um segundo whisper.
//!
//! Onde o whisper local nao esta nesta maquina, o motor da nuvem (ElevenLabs scribe) entra
//! como DUBLE contra um servidor FALSO -- o que se prova aqui e o CAMINHO (recebe audio,
//! confina, chama o motor, devolve o texto, apaga o temp), nao a qualidade do reconhecimento.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::elevenlabs::{ElevenLabs, MAX_AUDIO_ENVIADO, OuvidoElevenLabs, SERVICO};
use phxclaw_agent::rbac::Usuarios;
use phxclaw_agent::visao::TranscribeTool;
use phxclaw_agent::voz::PerfilDeVoz;
use phxclaw_agent::voz_rest::{ROTA, transcrever_audio};
use phxclaw_agent::{Agenda, Agent, AgentConfig, ScriptedLlm, TaskStore, rbac};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

const CHAVE_EL: &str = "xi-CHAVE-ELEVENLABS-QUE-NAO-PODE-VAZAR";
const PRAZO: Duration = Duration::from_secs(30);

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-stt-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------------------ servidor FALSO da nuvem

#[derive(Clone, Debug)]
struct Req {
    caminho: String,
    corpo: Vec<u8>,
}

type Log = Arc<Mutex<Vec<Req>>>;

async fn atender(
    State(log): State<Log>,
    _m: Method,
    u: Uri,
    corpo: Bytes,
) -> axum::response::Response {
    log.lock().unwrap().push(Req {
        caminho: u.path().to_string(),
        corpo: corpo.to_vec(),
    });
    axum::response::Response::new(axum::body::Body::from(
        br#"{"text":"ouvido pela nuvem falsa"}"#.to_vec(),
    ))
}

/// Sobe o servidor falso e devolve (base, log dos pedidos).
async fn falso() -> (String, Log) {
    let log: Log = Default::default();
    let app = axum::Router::new()
        .fallback(atender)
        .with_state(log.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, log)
}

/// O `TranscribeTool` com o motor da nuvem apontado para o servidor FALSO -- o duble do
/// whisper quando ele nao esta na maquina. `perfil` decide se a rede pode ser tocada.
fn ouvir_nuvem(base: &str, raiz: &Path, perfil: PerfilDeVoz) -> TranscribeTool {
    SERVICO.guardar(raiz, CHAVE_EL).unwrap();
    let cred = SERVICO.credencial(raiz).unwrap();
    TranscribeTool {
        bin: None,
        model: None,
        model_sha256: None,
        elevenlabs: Some(Ok(OuvidoElevenLabs {
            cliente: Arc::new(ElevenLabs::novo(base, cred).unwrap()),
            modelo: "scribe_v2".into(),
        })),
        perfil,
    }
}

/// Bytes de um WAV minimo (cabecalho RIFF/WAVE + um quadro). O duble nao os decodifica; o que
/// importa e que o CORPO enviado seja exatamente este, provando a confinacao e o repasse.
fn wav_curto() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&36u32.to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&16_000u32.to_le_bytes());
    b.extend_from_slice(&32_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

// ------------------------------------------------------------------ 1. devolve o texto (DUBLE)

/// O caminho inteiro: a rota recebe o audio, confina num temp, chama o MESMO
/// `transcrever` do motor (aqui o duble da nuvem) e devolve `{"texto": ...}`. Prova de que o
/// audio chega inteiro ao motor: o corpo multipart que o servidor falso recebeu CONTEM os
/// bytes enviados -- se o temp fosse gravado errado ou nao repassado, nao conteria.
/// RED medido: sem a chamada ao motor (ou gravando o temp vazio), o texto nao volta e o corpo
/// do servidor falso nao traz o WAV.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transcrever_devolve_texto() {
    let raiz = tmp("devolve");
    let (base, log) = falso().await;
    let ouvir = ouvir_nuvem(&base, &raiz, PerfilDeVoz::Auto);
    let audio = wav_curto();

    let texto = transcrever_audio(&ouvir, Some("audio/wav"), &audio, None, PRAZO)
        .await
        .expect("200 com o texto");
    assert!(texto.contains("ouvido pela nuvem falsa"), "{texto}");

    let l = log.lock().unwrap();
    let p = l.last().expect("o motor foi chamado");
    assert_eq!(p.caminho, "/v1/speech-to-text");
    assert!(
        p.corpo.windows(audio.len()).any(|w| w == audio.as_slice()),
        "o audio confinado nao chegou inteiro ao motor"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------------ 2. sem capacidade, recusa

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

fn estado_com_usuarios(pasta: &std::path::Path) -> ApiState {
    let store = TaskStore::new(pasta.join("tasks")).unwrap();
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![])),
            vec![],
            AgentConfig::default(),
            st.clone(),
        ))
    });
    ApiState {
        usuarios: Usuarios::da_pasta(pasta),
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(pasta.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    }
}

fn criar_usuario(pasta: &std::path::Path, nome: &str, papel: &str) -> String {
    let args: Vec<String> = ["criar", nome, "--papel", papel, "--projeto", "p"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let saida = rbac::comando(pasta, &args).unwrap();
    saida
        .lines()
        .find(|l| l.starts_with("phxu_"))
        .expect("token na saida")
        .to_string()
}

/// A rota tem capacidade propria no portao (`rbac::MATRIZ`): transcrever e operacao, do membro
/// para cima. O leitor e recusado ANTES do handler -- nao transcreve. O membro passa o portao
/// (e so entao cai na falta de motor local). RED medido: com a linha da matriz em `leitor`
/// (ou ausente, caindo no dono), o veredito do leitor muda e o teste reprova.
#[tokio::test]
async fn transcrever_recusa_sem_capacidade() {
    let pasta = tmp("cap");
    let leitor = criar_usuario(&pasta, "leo", "leitor");
    let membro = criar_usuario(&pasta, "mia", "member");
    let state = estado_com_usuarios(&pasta);

    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(state);
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let cli = reqwest::Client::new();

    let pedir = |token: String| {
        cli.post(format!("{base}{ROTA}"))
            .bearer_auth(token)
            .header("content-type", "audio/wav")
            .header("x-phxclaw-projeto", "p")
            .body(wav_curto())
            .send()
    };

    let r = pedir(leitor).await.unwrap();
    assert_eq!(r.status(), 403, "o leitor nao pode transcrever");

    // O membro ALCANCA a rota (passa o portao); so entao cai na falta de motor local.
    let r = pedir(membro).await.unwrap();
    assert_ne!(r.status(), 403, "o membro tem a capacidade");
    assert_ne!(r.status(), 401, "o membro esta autenticado");
    let _ = std::fs::remove_dir_all(&pasta);
}

// ------------------------------------------------------------------ 3. offline nao chama a nuvem

/// Perfil offline recusa a rede por nome, pela guarda que o `transcrever` carrega -- a MESMA
/// decisao do `speak` e do `voice_list`. A nuvem NUNCA e tocada. RED medido: sem a guarda de
/// offline no `transcrever`, o motor da nuvem e chamado e o servidor falso registra o pedido.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transcrever_offline_nao_chama_nuvem() {
    let raiz = tmp("offline");
    let (base, log) = falso().await;
    let ouvir = ouvir_nuvem(&base, &raiz, PerfilDeVoz::Offline);

    let (codigo, motivo) = transcrever_audio(&ouvir, Some("audio/wav"), &wav_curto(), None, PRAZO)
        .await
        .expect_err("offline recusa a nuvem");
    assert_eq!(codigo, StatusCode::SERVICE_UNAVAILABLE, "{motivo}");
    assert!(
        motivo.contains("offline") && motivo.contains("elevenlabs"),
        "a recusa nao nomeia o provedor online: {motivo}"
    );
    assert!(
        log.lock().unwrap().is_empty(),
        "offline tocou a rede: {:?}",
        log.lock().unwrap()
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------------ 4. audio gigante, recusa

/// Entrada externa sem teto e um alto de memoria: acima do `MAX_AUDIO_ENVIADO` a rota recusa
/// ANTES de gravar o temp ou tocar o motor. RED medido: sem a conferencia do teto, o audio
/// gigante e gravado e repassado (ou cai na falta de motor), e o codigo deixa de ser 413.
#[tokio::test]
async fn transcrever_recusa_audio_gigante() {
    // Motor inerte: se o teto NAO barrasse, cairiamos nele (nao numa recusa de tamanho).
    let ouvir = TranscribeTool {
        bin: None,
        model: None,
        model_sha256: None,
        elevenlabs: None,
        perfil: PerfilDeVoz::Auto,
    };
    let gigante = vec![0u8; (MAX_AUDIO_ENVIADO + 1) as usize];
    let (codigo, motivo) = transcrever_audio(&ouvir, Some("audio/wav"), &gigante, None, PRAZO)
        .await
        .expect_err("audio acima do teto e recusado");
    assert_eq!(codigo, StatusCode::PAYLOAD_TOO_LARGE, "{motivo}");
    assert!(motivo.contains("teto"), "{motivo}");
}

// ---------------------------------------- 5. credencial ANTES de materializar o corpo (M1)

/// Corpo que CONTA os bytes efetivamente puxados (`poll_frame`): entrega um unico quadro
/// grande e soma o que for puxado. E assim que se mede QUANTO foi lido, nao SE a rota recusou
/// -- a licao do papel F. Com o defeito reposto (auth so DEPOIS do `Bytes`), o extrator de
/// corpo puxa o quadro inteiro e o contador sobe; com o conserto (o `ApiAuth` recusa antes),
/// `poll_frame` nunca roda e o contador fica em 0.
struct CorpoContado {
    quadro: Option<Bytes>,
    lidos: Arc<AtomicUsize>,
}

impl http_body::Body for CorpoContado {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        match self.quadro.take() {
            Some(b) => {
                self.lidos.fetch_add(b.len(), Ordering::SeqCst);
                Poll::Ready(Some(Ok(http_body::Frame::data(b))))
            }
            None => Poll::Ready(None),
        }
    }
}

/// Sem `Authorization`, com RBAC DESLIGADO (sem usuarios.json) e um `api.token` configurado: o
/// `ApiAuth` (`FromRequestParts`) recusa 401 ANTES de o `Bytes` (`FromRequest`) coletar o
/// corpo. A prova nao e o 401 -- e o contador de bytes puxados em ZERO: o corpo nunca foi
/// materializado, logo nao houve alto de memoria pre-credencial (a classe do pedido 434). RED
/// medido: com a auth so dentro do handler (depois do `Bytes`), o contador vai a 33.554.432.
#[tokio::test]
async fn transcrever_recusa_sem_materializar_o_corpo() {
    use tower::ServiceExt;
    let pasta = tmp("sem-cred"); // sem usuarios criados: RBAC desligado, so o Bearer unico
    let state = estado_com_usuarios(&pasta);
    assert!(
        state.usuarios.ativos().is_empty(),
        "o RBAC tem de estar desligado neste teste"
    );

    // Dezenas de MiB, bem abaixo do teto de 201 MiB (sem 413): o que separa o 401 do defeito e
    // ter ou nao puxado estes bytes.
    const GRANDE: usize = 32 * 1024 * 1024;
    let lidos = Arc::new(AtomicUsize::new(0));
    let corpo = CorpoContado {
        quadro: Some(Bytes::from(vec![0u8; GRANDE])),
        lidos: lidos.clone(),
    };
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(ROTA)
        .header("content-type", "audio/wav")
        .body(axum::body::Body::new(corpo))
        .unwrap();

    let resp = router(state).oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "pedido sem credencial e 401"
    );
    assert_eq!(
        lidos.load(Ordering::SeqCst),
        0,
        "o corpo foi puxado ANTES da credencial: alto de memoria pre-credencial"
    );
    let _ = std::fs::remove_dir_all(&pasta);
}

//! Voz e midia contra as pecas reais: canvas no Chromium, fala pelo flite no bwrap e de
//! volta pelo whisper, gatilho pelo sherpa-onnx, a conversa por voz ponta a ponta, e os
//! clientes de imagem contra servidores FALSOS (nao ha GPU aqui). Cada teste diz que pulou
//! quando a peca falta, em vez de passar calado.
//!
//! Pecas locais (01/10/2026), em /var/tmp/phx-voz:
//! - flite 2.2 (pacote Ubuntu noble, licenca BSD da CMU) e a voz `cmu_us_slt.flitevox`;
//! - `sherpa-onnx-keyword-spotter` 1.13.8 (wheel `sherpa-onnx-bin` do PyPI, Apache-2.0, SEM
//!   espeak-ng: so o `offline-tts` o embute) e o modelo zipformer zh-en 3M de 2025-12-20;
//! - lexico CMU (`cmudict.dict`, BSD de 2 clausulas, do wheel `cmudict` 1.1.3 do PyPI).

use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::canvas::CanvasTool;
use phxclaw_agent::midia::{ImageGenerateTool, Provedor, preencher_fluxo};
use phxclaw_agent::visao::TranscribeTool;
use phxclaw_agent::voz::{SpeakTool, WakeWordTool, conversar, info_wav};
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-voz-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctx_em(workdir: PathBuf, task_id: &str) -> ToolContext {
    std::fs::create_dir_all(&workdir).unwrap();
    ToolContext {
        task_id: task_id.into(),
        workdir,
        timeout: Duration::from_secs(180),
    }
}

fn ctx(nome: &str) -> ToolContext {
    ctx_em(tmp(nome).join("work"), "t")
}

// ------------------------------------------------------------------ pecas locais

const VOZ_DIR: &str = "/var/tmp/phx-voz";
/// SHA-256 da `cmu_us_slt.flitevox` baixada de festvox.org em 01/10/2026. O site so serve
/// HTTP: o hash e o do primeiro download, fixado aqui (TOFU, nao vetor oficial).
const SHA_SLT: &str = "c54b0d757b943bfbffc626e95a251f1659cc2d840465daa8b858457139b94bc0";
/// Os quatro arquivos do modelo de gatilho com o SHA-256 que o empacotador publica para a
/// release oficial `kws-models` do sherpa-onnx (download.sh do moeru-ai, commit 1770a4b), e o
/// lexico CMU tirado do wheel `cmudict` 1.1.3, cujo hash o PyPI publica.
const SHAS_KWS: [(&str, &str); 5] = [
    (
        "540ff509ed89bd22afe04bf7049a54bb1c95c6d8a18742ea9691910cdb5f859e",
        "encoder.onnx",
    ),
    (
        "63a22dd60f40fff082ac3e09afa507f6787da36df76ded2fbe145fa233e22c21",
        "decoder.onnx",
    ),
    (
        "76f7a24ed0c08633af14b2ee377f747af880d3b65eeba2cd3f31f3380fb73e8d",
        "joiner.onnx",
    ),
    (
        "2d3f32311f9b692b964da3c90e830258d3e78e013cb0c992dbfb15cd5a1a71b0",
        "tokens.txt",
    ),
    (
        "81917843c7f44ce2b094ac63873c2c7a4cf802040792c455ba3ca406891c3d22",
        "cmudict.dict",
    ),
];
const SHA_TINY_EN: &str = "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f";

/// O SHA-256 declarado do modelo de gatilho, montado dos hashes de FORA e nao do arquivo
/// em disco: calcular do proprio arquivo e conferir contra ele nao provaria nada.
fn sha_kws_declarado() -> String {
    use sha2::{Digest, Sha256};
    let lista: String = SHAS_KWS
        .iter()
        .map(|(h, n)| format!("{h}  {n}\n"))
        .collect();
    Sha256::digest(lista.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn flite() -> Option<SpeakTool> {
    let d = Path::new(VOZ_DIR);
    let bin = d.join("flite/usr/bin/flite");
    let voz = d.join("cmu_us_slt.flitevox");
    if !bin.is_file() || !voz.is_file() || phxclaw_agent::arquivos::achar_bwrap().is_none() {
        eprintln!("PULADO: falta flite, voz ou bwrap em {VOZ_DIR}");
        return None;
    }
    let lib = d.join("flite/usr/lib/x86_64-linux-gnu");
    Some(SpeakTool {
        comando: Some(format!(
            "env LD_LIBRARY_PATH={} {} -voice {{{{Model}}}} -t {{{{Text}}}} -o {{{{OutputPath}}}}",
            lib.display(),
            bin.display()
        )),
        modelo: Some(voz),
        modelo_sha256: Some(SHA_SLT.into()),
        pastas: vec![d.join("flite/usr")],
        elevenlabs: None,
    })
}

fn whisper() -> Option<TranscribeTool> {
    let bin = PathBuf::from("/var/tmp/whisper.cpp/build/bin/whisper-cli");
    let modelo = PathBuf::from("/var/tmp/ggml-tiny.en.bin");
    if !bin.is_file() || !modelo.is_file() {
        eprintln!("PULADO: falta whisper-cli ou ggml-tiny.en.bin");
        return None;
    }
    Some(TranscribeTool {
        bin: Some(bin),
        model: Some(modelo),
        model_sha256: Some(SHA_TINY_EN.into()),
        elevenlabs: None,
    })
}

fn kws() -> Option<WakeWordTool> {
    let bin = PathBuf::from(VOZ_DIR).join("kws/bin/sherpa-onnx-keyword-spotter");
    let pasta = PathBuf::from(VOZ_DIR).join("kws/modelo");
    if !bin.is_file() || !pasta.join("encoder.onnx").is_file() {
        eprintln!("PULADO: falta o sherpa-onnx-keyword-spotter ou o modelo");
        return None;
    }
    Some(WakeWordTool {
        bin: Some(bin),
        pasta_modelo: Some(pasta),
        modelo_sha256: Some(sha_kws_declarado()),
    })
}

fn chromium_e_node() -> bool {
    let ok = phxclaw_browser::find_chromium().is_some()
        && Path::new("/opt/node22/bin/node").is_file()
        && Path::new("/opt/node22/lib/node_modules/playwright").is_dir();
    if !ok {
        eprintln!("PULADO: falta Chromium, node ou playwright");
    }
    ok
}

// ------------------------------------------------------------------ canvas

#[tokio::test]
async fn canvas_recusa_script_com_erro_dizendo_linha_e_coluna() {
    let c = ctx("canvas-erro");
    let t = CanvasTool {
        base_url: "http://x".into(),
    };
    let e = t
        .run(
            json!({"name":"ruim","html":"<p>oi</p>","script":"const a = 1;\nlet b = (2 + ;\n"}),
            &c,
        )
        .await
        .unwrap_err();
    let m = e.to_string();
    assert!(matches!(e, ToolError::InvalidArguments(_)), "{m}");
    assert!(m.contains("linha 2, coluna"), "{m}");
    let tarefa = c.workdir.parent().unwrap();
    assert!(
        !tarefa.join("canvas/ruim.json").exists() && !c.workdir.join("canvas/ruim.html").exists(),
        "widget com erro foi gravado"
    );
    // Coluna em caracteres, nao em bytes: o acento antes do erro nao desloca o numero.
    let m = t
        .run(
            json!({"name":"acento","html":"","script":"const ç = 'ã'; let = ;"}),
            &c,
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("linha 1, coluna"), "{m}");
    for (a, motivo) in [
        (
            json!({"name":"Maiuscula","html":""}),
            "nome de widget invalido",
        ),
        (json!({"name":"../x","html":""}), "nome de widget invalido"),
        (
            json!({"name":"x","html":"","script":"let s = '</script>';"}),
            "</script",
        ),
        (json!({"name":"x","html":"","css":"</style>"}), "</style"),
    ] {
        let m = t.run(a.clone(), &c).await.unwrap_err().to_string();
        assert!(m.contains(motivo), "{a}: {m}");
    }
    // O correto passa, e o servido mora fora de work/.
    let r = t
        .run(
            json!({"name":"bom","html":"<p id=a></p>","script":"document.getElementById('a').textContent = `ok ${1+1}`;"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("http://x/canvas/t/bom"), "{}", r.content);
    assert!(tarefa.join("canvas/bom.json").is_file());
}

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

async fn subir_api(store: TaskStore, extra: axum::Router) -> String {
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![])),
            vec![],
            AgentConfig::default(),
            st.clone(),
        ))
    });
    let dir = store.root().parent().unwrap().to_path_buf();
    let state = ApiState {
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(state).merge(extra);
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

/// O script do widget sonda tudo o que nao devia alcancar e escreve o que achou na pagina.
/// `no-cors`: sem ele a resposta sem CORS ja falharia na origem opaca e a sonda nao veria se
/// o PEDIDO saiu -- e e o pedido que vaza dado.
const SONDA: &str = r#"
const r = {};
try { r.cookie = document.cookie; } catch (e) { r.cookie = 'ERRO:' + e.name; }
try { r.ls = localStorage.getItem('phxclaw_token'); } catch (e) { r.ls = 'ERRO:' + e.name; }
try { r.pai = parent.document.title; } catch (e) { r.pai = 'ERRO:' + e.name; }
r.origem = String(self.origin);
document.getElementById('saida').textContent = JSON.stringify(r);
fetch('/v1/tasks', { mode: 'no-cors' }).then(x => { document.getElementById('rede').textContent = 'ALCANCOU:' + x.status; })
  .catch(() => { document.getElementById('rede').textContent = 'BLOQUEADO'; });
document.getElementById('estado').textContent = 'pronto';
"#;

const ROTEIRO_PW: &str = r#"
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const [base, caminho] = process.argv.slice(2);
(async () => {
  const b = await chromium.launch({ executablePath: process.env.PHX_CHROMIUM, args: ['--no-sandbox'] });
  const ctx = await b.newContext();
  await ctx.addCookies([{ name: 'segredo', value: 'abc123', url: base }]);
  const p = await ctx.newPage();
  await p.goto(base + '/health');
  await p.evaluate(() => { localStorage.setItem('phxclaw_token', 'TOKEN-DA-API'); document.title = 'PAI'; });
  await p.goto(base + caminho);
  const filho = p.frames().find(x => x !== p.mainFrame());
  const f = filho || p.mainFrame();
  const out = { frame: !!filho };
  if (f) {
    await f.waitForFunction(() => document.getElementById('estado') && document.getElementById('estado').textContent !== '', null, { timeout: 15000 }).catch(() => {});
    await f.waitForFunction(() => document.getElementById('rede').textContent !== '', null, { timeout: 15000 }).catch(() => {});
    out.estado = await f.evaluate(() => document.getElementById('estado').textContent);
    out.sonda = await f.evaluate(() => document.getElementById('saida').textContent);
    out.rede = await f.evaluate(() => document.getElementById('rede').textContent);
    await f.click('#botao').catch(() => {});
    await p.waitForTimeout(300);
    out.handler = await f.evaluate(() => document.getElementById('h').textContent);
  }
  out.titulo_pai = await p.title();
  console.log(JSON.stringify(out));
  await b.close();
})().catch(e => { console.log(JSON.stringify({ erro: String(e) })); process.exit(1); });
"#;

fn rodar_pw(base: &str, caminho: &str) -> Value {
    let dir = tmp("pw");
    let roteiro = dir.join("canvas.cjs");
    std::fs::write(&roteiro, ROTEIRO_PW).unwrap();
    let o = std::process::Command::new("/opt/node22/bin/node")
        .arg(&roteiro)
        .arg(base)
        .arg(caminho)
        .env("PLAYWRIGHT_BROWSERS_PATH", "/opt/pw-browsers")
        .env("PHX_CHROMIUM", phxclaw_browser::find_chromium().unwrap())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let saida = String::from_utf8_lossy(&o.stdout);
    let linha = saida.lines().last().unwrap_or("");
    serde_json::from_str(linha).unwrap_or_else(|_| {
        panic!(
            "playwright nao devolveu JSON: {saida} / {}",
            String::from_utf8_lossy(&o.stderr)
        )
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canvas_no_chromium_roda_so_o_script_conferido_e_nao_alcanca_a_api() {
    if !chromium_e_node() {
        return;
    }
    let raiz = tmp("canvas-pw");
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let id = phxclaw_types::new_uuid_v7().to_string();
    let c = ctx_em(store.workdir(&id), &id);
    // Marcacao com um <script> escondido e um onclick: a CSP pelo hash tem de barrar os dois.
    let html = "<p id=estado></p><pre id=saida></pre><p id=rede></p><p id=h></p>\
<script>document.getElementById('h').textContent='SCRIPT-INJETADO'</script>\
<button id=botao onclick=\"document.getElementById('h').textContent='HANDLER'\">x</button>";
    CanvasTool {
        base_url: "http://x".into(),
    }
    .run(json!({"name":"sonda","html":html,"script":SONDA}), &c)
    .await
    .unwrap();
    // Controle positivo: o MESMO documento servido sem a CSP e num iframe sem sandbox. Se
    // a sonda nao achasse o cookie aqui, ela nao provaria nada la.
    let doc: Value =
        serde_json::from_slice(&std::fs::read(store.dir(&id).join("canvas/sonda.json")).unwrap())
            .unwrap();
    let doc = doc["html"].as_str().unwrap().to_string();
    let id2 = id.clone();
    let extra = axum::Router::new()
        .route(
            "/controle",
            axum::routing::get(move || {
                let id = id2.clone();
                async move {
                    axum::response::Html(format!(
                        "<iframe src=\"/controle/widget\" title=\"{id}\"></iframe>"
                    ))
                }
            }),
        )
        .route(
            "/controle/widget",
            axum::routing::get(move || {
                let d = doc.clone();
                async move { axum::response::Html(d) }
            }),
        );
    let base = subir_api(store.clone(), extra).await;

    let base2 = base.clone();
    let id3 = id.clone();
    let isolado =
        tokio::task::spawn_blocking(move || rodar_pw(&base2, &format!("/canvas/{id3}/sonda")))
            .await
            .unwrap();
    assert_eq!(isolado["frame"], true, "{isolado}");
    assert_eq!(isolado["estado"], "pronto", "o widget nao abriu: {isolado}");
    let sonda: Value = serde_json::from_str(isolado["sonda"].as_str().unwrap()).unwrap();
    for k in ["cookie", "ls", "pai"] {
        assert!(
            sonda[k].as_str().unwrap_or("").starts_with("ERRO:"),
            "o widget leu {k}: {sonda}"
        );
    }
    assert_eq!(sonda["origem"], "null", "origem nao e opaca: {sonda}");
    assert_eq!(isolado["rede"], "BLOQUEADO", "{isolado}");
    assert_eq!(
        isolado["handler"], "",
        "script fora do conferido rodou: {isolado}"
    );
    assert_eq!(isolado["titulo_pai"], "sonda");

    // A URL do widget aberta direto, sem a pagina de fora: o cabecalho sozinho isola.
    let (base3, id4) = (base.clone(), id.clone());
    let direto = tokio::task::spawn_blocking(move || {
        rodar_pw(&base3, &format!("/canvas/{id4}/sonda/widget"))
    })
    .await
    .unwrap();
    assert_eq!(direto["estado"], "pronto", "{direto}");
    let sonda: Value = serde_json::from_str(direto["sonda"].as_str().unwrap()).unwrap();
    for k in ["cookie", "ls"] {
        assert!(
            sonda[k].as_str().unwrap_or("").starts_with("ERRO:"),
            "aberto direto, o widget leu {k}: {sonda}"
        );
    }
    assert_eq!(sonda["origem"], "null", "{sonda}");
    assert_eq!(direto["rede"], "BLOQUEADO", "{direto}");

    let controle = tokio::task::spawn_blocking(move || rodar_pw(&base, "/controle"))
        .await
        .unwrap();
    let sonda: Value = serde_json::from_str(controle["sonda"].as_str().unwrap()).unwrap();
    assert!(
        sonda["cookie"].as_str().unwrap().contains("segredo=abc123")
            && sonda["ls"] == "TOKEN-DA-API",
        "a sonda nao enxerga nem sem isolamento: {sonda}"
    );
    assert!(
        controle["rede"].as_str().unwrap().starts_with("ALCANCOU:"),
        "{controle}"
    );
    assert_eq!(controle["handler"], "HANDLER", "{controle}");
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------------ fala

#[tokio::test]
async fn speak_recusa_sem_configuracao_sem_marcador_e_com_modelo_adulterado() {
    let c = ctx("speak-recusa");
    let nada = SpeakTool {
        comando: None,
        modelo: None,
        modelo_sha256: None,
        pastas: vec![],
        elevenlabs: None,
    };
    let e = nada.run(json!({"text":"oi"}), &c).await.unwrap_err();
    let m = e.to_string();
    assert!(matches!(e, ToolError::Denied(_)), "{m}");
    for v in [
        "PHXCLAW_TTS_COMMAND",
        "PHXCLAW_TTS_MODEL",
        "PHXCLAW_TTS_MODEL_SHA256",
    ] {
        assert!(m.contains(v), "{m}");
    }
    let modelo = c.workdir.join("voz.bin");
    std::fs::write(&modelo, b"nao sou modelo").unwrap();
    let sem_saida = SpeakTool {
        comando: Some("flite -t {{Text}}".into()),
        modelo: Some(modelo.clone()),
        modelo_sha256: Some("0".repeat(64)),
        pastas: vec![],
        elevenlabs: None,
    };
    let m = sem_saida
        .run(json!({"text":"oi"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("sem o marcador {{OutputPath}}"), "{m}");
    let adulterado = SpeakTool {
        comando: Some("x {{Text}} {{OutputPath}}".into()),
        ..sem_saida
    };
    let m = adulterado
        .run(json!({"text":"oi"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("sha256 mismatch"), "{m}");
    for (a, motivo) in [
        (json!({"text":"   "}), "texto vazio"),
        (json!({"text":"a".repeat(2001)}), "acima do teto"),
        (json!({"text":"oi","output":"x.mp3"}), ".wav"),
        (json!({"text":"oi","output":"../x.wav"}), "fora da pasta"),
    ] {
        let m = adulterado.run(a.clone(), &c).await.unwrap_err().to_string();
        assert!(m.contains(motivo), "{a}: {m}");
    }
}

#[tokio::test]
async fn speak_recusa_wav_invalido_do_motor_e_nao_o_entrega() {
    if phxclaw_agent::arquivos::achar_bwrap().is_none() {
        eprintln!("PULADO: sem bwrap");
        return;
    }
    // Motor falso no MESMO sandbox: copia o "modelo" (texto) para a saida e sai 0.
    let c = ctx("speak-falso");
    let pasta = tmp("speak-falso-modelo");
    let modelo = pasta.join("voz.bin");
    std::fs::write(&modelo, b"isto nao e um WAV").unwrap();
    let sha = phxclaw_media_intelligence::sha256_file_hex(&modelo).unwrap();
    let t = SpeakTool {
        comando: Some("env TEXTO={{Text}} cp {{Model}} {{OutputPath}}".into()),
        modelo: Some(modelo),
        modelo_sha256: Some(sha),
        pastas: vec![],
        elevenlabs: None,
    };
    let m = t
        .run(json!({"text":"oi","output":"saida.wav"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("WAV invalido"), "{m}");
    assert!(
        !c.workdir.join("saida.wav").exists(),
        "WAV invalido entregue"
    );
    // O motor nao alcanca o que nao foi montado: um binario fora do /usr da 127 com a dica.
    let t2 = SpeakTool {
        comando: Some(format!(
            "{}/naoexiste {{{{Text}}}} {{{{OutputPath}}}}",
            pasta.display()
        )),
        ..t
    };
    let m = t2
        .run(json!({"text":"oi"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("PHXCLAW_TTS_DIRS"), "{m}");
    let _ = std::fs::remove_dir_all(&pasta);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn speak_gera_wav_e_o_whisper_devolve_o_texto() {
    let (Some(fala), Some(ouvir)) = (flite(), whisper()) else {
        return;
    };
    let c = ctx("speak-real");
    let r = fala
        .run(
            json!({"text":"The quick brown fox jumps over the lazy dog.","output":"saida/fox.wav"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("saida/fox.wav"), "{}", r.content);
    assert_eq!(r.artifacts.len(), 1);
    let bytes = std::fs::read(c.workdir.join("saida/fox.wav")).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    let w = info_wav(&bytes).unwrap();
    assert_eq!((w.canais, w.bits, w.formato), (1, 16, 1), "{w:?}");
    assert!(
        (1.5..6.0).contains(&w.segundos),
        "duracao fora do plausivel: {w:?}"
    );
    let texto = ouvir
        .transcrever(c.workdir.join("saida/fox.wav"), None, c.timeout)
        .await
        .unwrap()
        .to_lowercase();
    assert!(
        texto.contains("quick brown fox"),
        "o whisper ouviu: {texto}"
    );
}

// ------------------------------------------------------------------ gatilho

#[tokio::test]
async fn wake_word_dispara_com_a_palavra_e_cala_sem_ela() {
    let (Some(fala), Some(kw)) = (flite(), kws()) else {
        return;
    };
    let c = ctx("wake");
    fala.falar(
        "Hello there. Please wake up computer and listen to me.",
        &c.workdir,
        "com.wav",
        c.timeout,
    )
    .await
    .unwrap();
    fala.falar(
        "The weather today is sunny and warm in the valley.",
        &c.workdir,
        "sem.wav",
        c.timeout,
    )
    .await
    .unwrap();
    let gatilhos = vec!["wake up computer".to_string(), "hey phoenix".to_string()];
    let d = kw
        .ouvir(&c.workdir.join("com.wav"), &gatilhos, 0.25, c.timeout)
        .await
        .unwrap();
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].gatilho, "wake up computer");
    assert!(d[0].segundo > 0.5, "{d:?}");
    let d = kw
        .ouvir(&c.workdir.join("sem.wav"), &gatilhos, 0.25, c.timeout)
        .await
        .unwrap();
    assert!(d.is_empty(), "disparou sem a palavra: {d:?}");
    // Pela ferramenta, com o texto que o modelo le.
    let r = kw
        .run(
            json!({"path":"com.wav","keywords":["wake up computer"]}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("'wake up computer' em"), "{}", r.content);

    let muitos: Vec<String> = (0..33).map(|_| "hello".to_string()).collect();
    for (g, motivo) in [
        (muitos, "de 1 a 32 gatilhos"),
        (vec!["a".repeat(65)], "gatilho invalido"),
        (vec!["wake;rm".into()], "gatilho invalido"),
        (
            vec!["xyzzyqw computer".into()],
            "sem pronuncia no lexico: xyzzyqw",
        ),
    ] {
        let m = kw
            .ouvir(&c.workdir.join("com.wav"), &g, 0.25, c.timeout)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains(motivo), "{motivo}: {m}");
    }
    let adulterado = WakeWordTool {
        modelo_sha256: Some("0".repeat(64)),
        ..kws().unwrap()
    };
    let e = adulterado
        .ouvir(&c.workdir.join("com.wav"), &gatilhos, 0.25, c.timeout)
        .await
        .unwrap_err();
    assert!(
        matches!(e, ToolError::Denied(_)) && e.to_string().contains("sha256 mismatch"),
        "{e}"
    );
}

// ------------------------------------------------------------------ conversa por voz

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversa_por_voz_ponta_a_ponta_com_contexto_entre_turnos() {
    let (Some(fala), Some(ouvir)) = (flite(), whisper()) else {
        return;
    };
    let raiz = tmp("conversa");
    let entrada = raiz.join("entrada");
    std::fs::create_dir_all(&entrada).unwrap();
    // As perguntas faladas saem do flite por fora da conversa: a entrada nao depende do
    // codigo sob prova alem do motor de voz.
    fala.falar(
        "What is the capital of France?",
        &entrada,
        "t1.wav",
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    fala.falar(
        "And what about Germany?",
        &entrada,
        "t2.wav",
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::text("The capital of France is Paris."),
        ScriptedLlm::text("The capital of Germany is Berlin."),
    ]));
    let mut agente = phxclaw_agent::voz::agente_de_conversa(
        montagem::Montagem::new(store.clone()).agent_with(llm.clone()),
    );
    agente.config.tool_timeout = Duration::from_secs(180);
    let turnos = conversar(
        &agente,
        "roteiro",
        &[entrada.join("t1.wav"), entrada.join("t2.wav")],
        &ouvir,
        &fala,
        || NoObserver,
    )
    .await;
    assert_eq!(turnos.len(), 2);
    for t in &turnos {
        assert!(t.erro.is_none(), "{t:?}");
        assert_eq!(t.estado, TaskStatus::Completed);
    }
    assert!(
        turnos[0]
            .ouvido
            .to_lowercase()
            .contains("capital of france"),
        "{:?}",
        turnos[0]
    );
    assert!(
        turnos[1].ouvido.to_lowercase().contains("germany"),
        "{:?}",
        turnos[1]
    );
    // O que o whisper ouviu chegou ao modelo, e o segundo turno levou o primeiro junto.
    let (primeiro, segundo) = {
        let visto = llm.seen.lock().unwrap();
        let texto_de = |i: usize| -> String {
            visto[i]
                .0
                .iter()
                .map(|m| format!("{m:?}"))
                .collect::<Vec<_>>()
                .join("\n")
                .to_lowercase()
        };
        (texto_de(0), texto_de(1))
    };
    assert!(primeiro.contains("capital of france"), "{primeiro}");
    assert!(
        segundo.contains("the capital of france is paris") && segundo.contains("germany"),
        "{segundo}"
    );
    // A resposta falada volta ao texto pelo mesmo whisper.
    let wav = turnos[0].wav.clone().unwrap();
    assert!(wav.starts_with(store.workdir(&turnos[0].tarefa)));
    let w = info_wav(&std::fs::read(&wav).unwrap()).unwrap();
    assert!(w.segundos > 1.0, "{w:?}");
    let volta = ouvir
        .transcrever(wav, None, Duration::from_secs(180))
        .await
        .unwrap()
        .to_lowercase();
    assert!(volta.contains("paris"), "a fala da resposta virou: {volta}");
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------------ imagem

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut p = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    p.extend_from_slice(b"IHDR");
    p.extend_from_slice(&w.to_be_bytes());
    p.extend_from_slice(&h.to_be_bytes());
    p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    p
}

/// Cabecalho Authorization e corpo de um pedido recebido pelo servidor falso.
type Pedido = (Option<String>, Value);

async fn servir(app: axum::Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

#[tokio::test]
async fn image_generate_openai_contra_servidor_falso() {
    use axum::http::{HeaderMap, StatusCode};
    use base64::Engine;
    let pedidos: Arc<Mutex<Vec<Pedido>>> = Default::default();
    let p2 = pedidos.clone();
    let app = axum::Router::new()
        .route(
            "/v1/images/generations",
            axum::routing::post(move |h: HeaderMap, axum::Json(v): axum::Json<Value>| {
                let p = p2.clone();
                async move {
                    let auth = h
                        .get("authorization")
                        .and_then(|x| x.to_str().ok())
                        .map(String::from);
                    p.lock().unwrap().push((auth, v.clone()));
                    let corpo = if v["prompt"] == "html" {
                        json!({"data":[{"b64_json": base64::engine::general_purpose::STANDARD.encode("<html>erro</html>")}]})
                    } else {
                        json!({"data":[{"b64_json": base64::engine::general_purpose::STANDARD.encode(png(512, 256))}]})
                    };
                    (StatusCode::OK, axum::Json(corpo))
                }
            }),
        )
        .route(
            "/redir/v1/images/generations",
            axum::routing::post(|| async {
                (
                    StatusCode::FOUND,
                    [("location", "http://127.0.0.1:9/roubo")],
                    "",
                )
            }),
        );
    let base = servir(app).await;
    let c = ctx("img-openai");
    let t = ImageGenerateTool {
        provedor: Some(Provedor::OpenAi {
            url: base.clone(),
            chave: Some("sk-teste".into()),
            modelo: "dall-e-3".into(),
        }),
    };
    let r = t
        .run(
            json!({"prompt":"um gato \"azul\"","size":"512x256","output":"img/gato.png"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("512x256"), "{}", r.content);
    assert_eq!(
        std::fs::read(c.workdir.join("img/gato.png")).unwrap(),
        png(512, 256)
    );
    {
        let v = pedidos.lock().unwrap();
        assert_eq!(v[0].0.as_deref(), Some("Bearer sk-teste"));
        assert_eq!(v[0].1["prompt"], "um gato \"azul\"");
        assert_eq!(v[0].1["size"], "512x256");
        assert_eq!(v[0].1["response_format"], "b64_json");
        assert!(!r.content.contains("sk-teste"));
    }
    let m = t
        .run(json!({"prompt":"html","output":"x.png"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("nao devolveu PNG"), "{m}");
    assert!(!c.workdir.join("x.png").exists());
    // Redirecionamento nao se segue: a chave nao vai para outro endereco.
    let redir = ImageGenerateTool {
        provedor: Some(Provedor::OpenAi {
            url: format!("{base}/redir"),
            chave: Some("sk-teste".into()),
            modelo: "gpt-image-1".into(),
        }),
    };
    let m = redir
        .run(json!({"prompt":"x"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("302"), "{m}");
    for (a, motivo) in [
        (json!({"prompt":"x","size":"10x10"}), "size invalido"),
        (json!({"prompt":"x","output":"a.jpg"}), ".png"),
        (json!({}), "informe 'prompt' ou 'svg'"),
    ] {
        let m = t.run(a.clone(), &c).await.unwrap_err().to_string();
        assert!(m.contains(motivo), "{a}: {m}");
    }
    let sem = ImageGenerateTool { provedor: None };
    let e = sem.run(json!({"prompt":"x"}), &c).await.unwrap_err();
    assert!(
        matches!(e, ToolError::Denied(_)) && e.to_string().contains("svg"),
        "{e}"
    );
}

#[tokio::test]
async fn image_generate_comfyui_contra_servidor_falso() {
    use axum::extract::{Path as P, Query};
    let recebido: Arc<Mutex<Option<Value>>> = Default::default();
    let consultas = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (r2, c2) = (recebido.clone(), consultas.clone());
    let app = axum::Router::new()
        .route(
            "/prompt",
            axum::routing::post(move |axum::Json(v): axum::Json<Value>| {
                let r = r2.clone();
                async move {
                    *r.lock().unwrap() = Some(v);
                    axum::Json(json!({"prompt_id":"abc-123","number":1}))
                }
            }),
        )
        .route(
            "/history/{id}",
            axum::routing::get(move |P(id): P<String>| {
                let c = c2.clone();
                async move {
                    // Primeira consulta: ainda na fila. Segunda: pronta.
                    if c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                        return axum::Json(json!({}));
                    }
                    axum::Json(json!({id: {"status":{"status_str":"success"},
                        "outputs":{"9":{"images":[{"filename":"img 1.png","subfolder":"","type":"output"}]}}}}))
                }
            }),
        )
        .route(
            "/view",
            axum::routing::get(|Query(q): Query<HashMap<String, String>>| async move {
                if q.get("filename").map(String::as_str) == Some("img 1.png")
                    && q.get("type").map(String::as_str) == Some("output")
                {
                    png(768, 512)
                } else {
                    b"errado".to_vec()
                }
            }),
        );
    let base = servir(app).await;
    let c = ctx("img-comfy");
    let fluxo = c.workdir.parent().unwrap().join("fluxo.json");
    std::fs::write(
        &fluxo,
        r#"{"3":{"class_type":"KSampler","inputs":{"seed":"{{Seed}}","steps":4}},
           "5":{"class_type":"EmptyLatentImage","inputs":{"width":"{{Width}}","height":"{{Height}}"}},
           "6":{"class_type":"CLIPTextEncode","inputs":{"text":"estilo aquarela, {{Prompt}}"}}}"#,
    )
    .unwrap();
    let t = ImageGenerateTool {
        provedor: Some(Provedor::ComfyUi {
            url: base,
            fluxo: fluxo.clone(),
        }),
    };
    let r = t
        .run(
            json!({"prompt":"farol \"vermelho\", }","size":"768x512"}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("768x512"), "{}", r.content);
    let v = recebido.lock().unwrap().clone().unwrap();
    let wf = &v["prompt"];
    assert_eq!(
        wf["6"]["inputs"]["text"], "estilo aquarela, farol \"vermelho\", }",
        "o prompt nao entrou como texto: {wf}"
    );
    assert_eq!(wf["5"]["inputs"]["width"], 768);
    assert_eq!(wf["5"]["inputs"]["height"], 512);
    assert!(wf["3"]["inputs"]["seed"].is_u64(), "{wf}");
    assert_eq!(wf.as_object().unwrap().len(), 3, "o prompt criou no: {wf}");
    assert_eq!(consultas.load(std::sync::atomic::Ordering::SeqCst), 2);
    // A troca e na arvore: um texto com aspas e chaves nao quebra o JSON.
    let mut x = json!({"a":["{{Prompt}}", {"b":"{{Seed}}"}]});
    assert_eq!(preencher_fluxo(&mut x, "\"}", 1, 2, 7), 2);
    assert_eq!(x, json!({"a":["\"}", {"b":7}]}));
    std::fs::write(&fluxo, r#"{"6":{"inputs":{"text":"sem marcador"}}}"#).unwrap();
    let m = t
        .run(json!({"prompt":"x"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("{{Prompt}}"), "{m}");
}

#[tokio::test]
async fn image_generate_svg_local_desenhado_pelo_chromium() {
    if phxclaw_browser::find_chromium().is_none() {
        eprintln!("PULADO: sem Chromium");
        return;
    }
    let c = ctx("img-svg");
    let t = ImageGenerateTool { provedor: None };
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100"><rect width="200" height="100" fill="#0a3"/><circle cx="100" cy="50" r="40" fill="#fc0"/></svg>"##;
    let r = t
        .run(json!({"svg": svg, "output":"arte/sol.png"}), &c)
        .await
        .unwrap();
    assert!(r.content.contains("200x100"), "{}", r.content);
    let b = std::fs::read(c.workdir.join("arte/sol.png")).unwrap();
    assert_eq!(phxclaw_agent::visao::info_png(&b).unwrap().0, 200);
    assert!(c.workdir.join("arte/sol.svg").is_file());
    let m = t
        .run(json!({"svg":"<svg><g></svg>"}), &c)
        .await
        .unwrap_err()
        .to_string();
    assert!(m.contains("svg:"), "{m}");
}

// ------------------------------------------------------------------ ElevenLabs e Nano Banana
//
// Provedores pagos contra servidores FALSOS locais: nao ha chave real aqui. O que se prova e o
// fio (cabecalho da chave, consulta, corpo, imagem de entrada enviada), o que se faz com a
// resposta (WAV conferido, PNG conferido, arquivo na pasta) e que a chave nao escapa por erro,
// redirecionamento nem disco.

mod pagos {
    use super::*;
    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, Uri};
    use base64::Engine;
    use phxclaw_agent::elevenlabs::{
        ElevenLabs, FalaElevenLabs, Formato, OuvidoElevenLabs, VoiceListTool, wav_de_pcm,
    };
    use phxclaw_agent::nanobanana::NanoBanana;

    const CHAVE_EL: &str = "xi-CHAVE-ELEVENLABS-QUE-NAO-PODE-VAZAR";
    const CHAVE_G: &str = "AIza-CHAVE-GEMINI-QUE-NAO-PODE-VAZAR";

    #[derive(Debug, Clone)]
    pub struct Req {
        pub metodo: String,
        pub caminho: String,
        pub consulta: String,
        pub cab: Vec<(String, String)>,
        pub corpo: Vec<u8>,
    }

    impl Req {
        pub fn cab(&self, k: &str) -> Option<&str> {
            self.cab
                .iter()
                .find(|(c, _)| c == k)
                .map(|(_, v)| v.as_str())
        }
        pub fn json(&self) -> Value {
            serde_json::from_slice(&self.corpo).unwrap_or(Value::Null)
        }
    }

    type Roteiro = Arc<dyn Fn(&Req) -> (u16, Vec<(&'static str, String)>, Vec<u8>) + Send + Sync>;
    type Estado = (Roteiro, Arc<Mutex<Vec<Req>>>);

    async fn atender(
        State((r, log)): State<Estado>,
        m: Method,
        u: Uri,
        h: HeaderMap,
        corpo: Bytes,
    ) -> axum::response::Response {
        let req = Req {
            metodo: m.to_string(),
            caminho: u.path().to_string(),
            consulta: u.query().unwrap_or("").to_string(),
            cab: h
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            corpo: corpo.to_vec(),
        };
        log.lock().unwrap().push(req.clone());
        // O roteiro pode rodar o flite: fora do runtime.
        let (st, cab, b) = tokio::task::spawn_blocking(move || r(&req)).await.unwrap();
        let mut resp = axum::response::Response::new(axum::body::Body::from(b));
        *resp.status_mut() = axum::http::StatusCode::from_u16(st).unwrap();
        for (k, v) in cab {
            resp.headers_mut().insert(k, v.parse().unwrap());
        }
        resp
    }

    pub async fn falso(
        r: impl Fn(&Req) -> (u16, Vec<(&'static str, String)>, Vec<u8>) + Send + Sync + 'static,
    ) -> (String, Arc<Mutex<Vec<Req>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let app = axum::Router::new()
            .fallback(atender)
            .with_state((Arc::new(r) as Roteiro, log.clone()));
        (servir(app).await, log)
    }

    /// Meio segundo de 440 Hz, 16 kHz mono: audio que nao depende de motor nenhum.
    fn pcm_sintetico() -> Vec<u8> {
        (0..8000)
            .flat_map(|i| {
                let x = (i as f64 * 440.0 * std::f64::consts::TAU / 16000.0).sin();
                ((x * 8000.0) as i16).to_le_bytes()
            })
            .collect()
    }

    /// O WAV que o flite fala para `texto`, rodado direto (o servidor falso nao e o codigo
    /// sob prova). `None` sem flite.
    fn flite_wav(texto: &str) -> Option<Vec<u8>> {
        let d = Path::new(VOZ_DIR);
        let bin = d.join("flite/usr/bin/flite");
        if !bin.is_file() {
            return None;
        }
        let saida =
            std::env::temp_dir().join(format!("phx-voz-el-{}.wav", phxclaw_types::new_uuid_v7()));
        let ok = std::process::Command::new(&bin)
            .env("LD_LIBRARY_PATH", d.join("flite/usr/lib/x86_64-linux-gnu"))
            .arg("-voice")
            .arg(d.join("cmu_us_slt.flitevox"))
            .arg("-t")
            .arg(texto)
            .arg("-o")
            .arg(&saida)
            .status()
            .ok()?
            .success();
        let b = std::fs::read(&saida).ok();
        let _ = std::fs::remove_file(&saida);
        b.filter(|_| ok)
    }

    /// Servidor que so ecoa o pedido inteiro num 401, e o que so redireciona para `destino`.
    async fn eco_e_redirecionador(destino: String) -> (String, String) {
        let (eco, _) = falso(|r| {
            (
                401,
                vec![],
                format!(
                    "eco: {:?} {} {}",
                    r.cab,
                    r.consulta,
                    String::from_utf8_lossy(&r.corpo)
                )
                .into_bytes(),
            )
        })
        .await;
        let (redir, _) = falso(move |_| (302, vec![("location", destino.clone())], vec![])).await;
        (eco, redir)
    }

    /// Raiz de agente com a chave guardada no broker do servico.
    fn raiz_com(servico: &phxclaw_agent::chaves::Servico, chave: &str) -> PathBuf {
        let raiz = tmp("pagos-raiz");
        servico.guardar(&raiz, chave).unwrap();
        raiz
    }

    fn fala(cliente: ElevenLabs, formato: Formato, ajustes: Option<Value>) -> SpeakTool {
        SpeakTool {
            comando: None,
            modelo: None,
            modelo_sha256: None,
            pastas: vec![],
            elevenlabs: Some(Ok(FalaElevenLabs {
                cliente: Arc::new(cliente),
                voz: "vozPadrao1".into(),
                modelo: "eleven_multilingual_v2".into(),
                formato,
                ajustes,
            })),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn elevenlabs_fala_transcreve_e_lista_vozes_contra_servidor_falso() {
        let raiz = raiz_com(&phxclaw_agent::elevenlabs::SERVICO, CHAVE_EL);
        let (base, log) = falso(|r| {
            let pcm = pcm_sintetico();
            if r.caminho.starts_with("/v1/text-to-speech/") {
                let pcm_cru = r.consulta.contains("output_format=pcm_");
                let corpo = if pcm_cru {
                    pcm
                } else {
                    wav_de_pcm(&pcm, 16000)
                };
                return (200, vec![("content-type", "audio/wav".into())], corpo);
            }
            if r.caminho == "/v1/speech-to-text" {
                return (
                    200,
                    vec![],
                    br#"{"text":"ouvido pela elevenlabs falsa"}"#.to_vec(),
                );
            }
            if r.caminho == "/v2/voices" {
                let v = json!({"voices":[
                    {"voice_id":"abc123","name":"Ana","category":"premade"},
                    {"voice_id":"def456","name":"Bia","category":"cloned"}
                ],"has_more":false,"total_count":2});
                return (200, vec![], v.to_string().into_bytes());
            }
            (404, vec![], b"nao".to_vec())
        })
        .await;
        let cred = || {
            phxclaw_agent::elevenlabs::SERVICO
                .credencial(&raiz)
                .unwrap()
        };
        let c = ctx("el-fala");

        // Fala em WAV, com voice_settings do operador.
        let t = fala(
            ElevenLabs::novo(&base, cred()).unwrap(),
            Formato::Wav(16000),
            Some(json!({"stability": 0.4})),
        );
        let r = t
            .run(json!({"text":"Ola, mundo.","output":"voz/el.wav"}), &c)
            .await
            .unwrap();
        assert!(r.content.contains("voz/el.wav"), "{}", r.content);
        assert!(!r.content.contains(CHAVE_EL));
        let w = info_wav(&std::fs::read(c.workdir.join("voz/el.wav")).unwrap()).unwrap();
        assert_eq!((w.taxa, w.canais, w.bits), (16000, 1, 16), "{w:?}");
        {
            let l = log.lock().unwrap();
            let p = l.last().unwrap();
            assert_eq!(p.metodo, "POST");
            assert_eq!(p.caminho, "/v1/text-to-speech/vozPadrao1");
            assert_eq!(p.cab("xi-api-key"), Some(CHAVE_EL));
            assert!(p.cab("authorization").is_none(), "a chave so no xi-api-key");
            assert_eq!(p.consulta, "output_format=wav_16000");
            let j = p.json();
            assert_eq!(j["text"], "Ola, mundo.");
            assert_eq!(j["model_id"], "eleven_multilingual_v2");
            assert_eq!(j["voice_settings"]["stability"], 0.4);
        }
        // A voz por chamada vai no caminho; voz que mudaria o caminho nem sai.
        t.run(json!({"text":"oi","voice":"abc123"}), &c)
            .await
            .unwrap();
        assert_eq!(
            log.lock().unwrap().last().unwrap().caminho,
            "/v1/text-to-speech/abc123"
        );
        let antes = log.lock().unwrap().len();
        let m = t
            .run(json!({"text":"oi","voice":"../v2/voices"}), &c)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains("voice invalida"), "{m}");
        assert_eq!(
            log.lock().unwrap().len(),
            antes,
            "pedido saiu com voz invalida"
        );
        // Sem ajuste do operador, voice_settings nao vai (sobrescreveria o da voz).
        let t2 = fala(
            ElevenLabs::novo(&base, cred()).unwrap(),
            Formato::Wav(16000),
            None,
        );
        t2.run(json!({"text":"oi"}), &c).await.unwrap();
        assert!(
            log.lock()
                .unwrap()
                .last()
                .unwrap()
                .json()
                .get("voice_settings")
                .is_none()
        );
        // PCM cru vira WAV com o cabecalho certo.
        let t3 = fala(
            ElevenLabs::novo(&base, cred()).unwrap(),
            Formato::Pcm(16000),
            None,
        );
        t3.run(json!({"text":"oi","output":"pcm.wav"}), &c)
            .await
            .unwrap();
        let w = info_wav(&std::fs::read(c.workdir.join("pcm.wav")).unwrap()).unwrap();
        assert!((w.segundos - 0.5).abs() < 0.01, "{w:?}");
        assert!(
            log.lock()
                .unwrap()
                .last()
                .unwrap()
                .consulta
                .contains("pcm_16000")
        );
        // As conferencias de sempre do speak valem para o provedor novo.
        for (a, motivo) in [
            (json!({"text":"   "}), "texto vazio"),
            (json!({"text":"oi","output":"x.mp3"}), ".wav"),
            (json!({"text":"oi","output":"../x.wav"}), "fora da pasta"),
        ] {
            let m = t.run(a.clone(), &c).await.unwrap_err().to_string();
            assert!(m.contains(motivo), "{a}: {m}");
        }

        // Transcricao: o audio da pasta vai inteiro, no multipart, com o modelo.
        let ouvir = TranscribeTool {
            bin: None,
            model: None,
            model_sha256: None,
            elevenlabs: Some(Ok(OuvidoElevenLabs {
                cliente: Arc::new(ElevenLabs::novo(&base, cred()).unwrap()),
                modelo: "scribe_v2".into(),
            })),
        };
        let r = ouvir
            .run(json!({"path":"voz/el.wav","language":"pt"}), &c)
            .await
            .unwrap();
        assert!(
            r.content.contains("ouvido pela elevenlabs falsa"),
            "{}",
            r.content
        );
        {
            let l = log.lock().unwrap();
            let p = l.last().unwrap();
            assert_eq!(p.caminho, "/v1/speech-to-text");
            assert_eq!(p.cab("xi-api-key"), Some(CHAVE_EL));
            let enviado = std::fs::read(c.workdir.join("voz/el.wav")).unwrap();
            assert!(
                p.corpo.windows(enviado.len()).any(|w| w == enviado),
                "o WAV nao foi enviado inteiro"
            );
            let txt = String::from_utf8_lossy(&p.corpo);
            assert!(txt.contains("name=\"model_id\"") && txt.contains("scribe_v2"));
            assert!(txt.contains("name=\"language_code\"") && txt.contains("\r\npt\r\n"));
        }

        // Vozes.
        let vl = VoiceListTool {
            cliente: Arc::new(ElevenLabs::novo(&base, cred()).unwrap()),
        };
        let r = vl.run(json!({"search":"ana"}), &c).await.unwrap();
        assert!(r.content.contains("abc123  Ana (premade)"), "{}", r.content);
        let q = log.lock().unwrap().last().unwrap().consulta.clone();
        assert!(
            q.contains("page_size=100") && q.contains("search=ana"),
            "{q}"
        );

        assert!(
            sem_segredo_no_disco(&raiz, CHAVE_EL),
            "chave em texto puro no disco"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sem_chave_o_provedor_pago_recusa_dizendo_o_comando_e_nao_cria_cofre() {
        let vazia = tmp("pagos-sem-chave");
        let e = ElevenLabs::da_pasta(&vazia).err().unwrap();
        assert!(e.contains("phxclaw elevenlabs chave"), "{e}");
        let e = NanoBanana::da_pasta(&vazia).err().unwrap();
        assert!(e.contains("phxclaw gemini chave"), "{e}");
        assert!(
            std::fs::read_dir(&vazia).unwrap().next().is_none(),
            "perguntar pela chave criou cofre"
        );
        assert!(
            VoiceListTool::da_pasta(&vazia).is_none(),
            "voice_list sem chave"
        );
        let c = ctx("pagos-sem-chave");
        let falar = SpeakTool {
            comando: None,
            modelo: None,
            modelo_sha256: None,
            pastas: vec![],
            elevenlabs: Some(Err(ElevenLabs::da_pasta(&vazia).err().unwrap())),
        };
        let e = falar.run(json!({"text":"oi"}), &c).await.unwrap_err();
        assert!(
            matches!(e, ToolError::Denied(_)) && e.to_string().contains("elevenlabs chave"),
            "{e}"
        );
        let ouvir = TranscribeTool {
            bin: None,
            model: None,
            model_sha256: None,
            elevenlabs: Some(Err(ElevenLabs::da_pasta(&vazia).err().unwrap())),
        };
        let e = ouvir.run(json!({"path":"a.wav"}), &c).await.unwrap_err();
        assert!(
            matches!(e, ToolError::Denied(_)) && e.to_string().contains("elevenlabs chave"),
            "{e}"
        );
        let img = ImageGenerateTool {
            provedor: Some(Provedor::NanoBanana(
                NanoBanana::da_pasta(&vazia).map(Arc::new),
            )),
        };
        let e = img.run(json!({"prompt":"x"}), &c).await.unwrap_err();
        assert!(
            matches!(e, ToolError::Denied(_)) && e.to_string().contains("gemini chave"),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(&vazia);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nano_banana_gera_e_edita_contra_servidor_falso() {
        let raiz = raiz_com(&phxclaw_agent::nanobanana::SERVICO, CHAVE_G);
        let (base, log) = falso(|r| {
            let j = r.json();
            let partes = j["contents"][0]["parts"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let prompt = partes[0]["text"].as_str().unwrap_or("").to_string();
            let editando = partes.iter().any(|p| p.get("inline_data").is_some());
            let (tipo, bytes) = match prompt.as_str() {
                "jpeg" => ("image/jpeg", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0]),
                "bloqueia" => {
                    let v = json!({"promptFeedback":{"blockReason":"SAFETY"}});
                    return (200, vec![], v.to_string().into_bytes());
                }
                _ if editando => ("image/png", png(300, 200)),
                _ => ("image/png", png(64, 64)),
            };
            let v = json!({"candidates":[{"content":{"parts":[
                {"text":"aqui esta"},
                {"inlineData":{"mimeType": tipo,
                    "data": base64::engine::general_purpose::STANDARD.encode(bytes)}}
            ]},"finishReason":"STOP"}]});
            (200, vec![], v.to_string().into_bytes())
        })
        .await;
        let cred = phxclaw_agent::nanobanana::SERVICO
            .credencial(&raiz)
            .unwrap();
        let t = ImageGenerateTool {
            provedor: Some(Provedor::NanoBanana(Ok(Arc::new(
                NanoBanana::novo(&base, "gemini-2.5-flash-image", cred).unwrap(),
            )))),
        };
        let c = ctx("nano");
        // Gerar.
        let r = t
            .run(json!({"prompt":"uma banana","output":"g.png"}), &c)
            .await
            .unwrap();
        assert!(r.content.contains("64x64"), "{}", r.content);
        assert_eq!(std::fs::read(c.workdir.join("g.png")).unwrap(), png(64, 64));
        {
            let l = log.lock().unwrap();
            let p = l.last().unwrap();
            assert_eq!(
                p.caminho,
                "/v1beta/models/gemini-2.5-flash-image:generateContent"
            );
            assert_eq!(p.cab("x-goog-api-key"), Some(CHAVE_G));
            assert!(p.consulta.is_empty(), "chave na URL: {}", p.consulta);
            let j = p.json();
            assert_eq!(j["contents"][0]["parts"][0]["text"], "uma banana");
            assert!(j["generationConfig"].get("imageConfig").is_none());
        }
        // Tamanho pedido vira proporcao; proporcao que o modelo nao tem recusa antes do pedido.
        t.run(json!({"prompt":"x","size":"1024x576","output":"w.png"}), &c)
            .await
            .unwrap();
        assert_eq!(
            log.lock().unwrap().last().unwrap().json()["generationConfig"]["imageConfig"]["aspectRatio"],
            "16:9"
        );
        let antes = log.lock().unwrap().len();
        let m = t
            .run(json!({"prompt":"x","size":"512x256"}), &c)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains("2:1") && m.contains("16:9"), "{m}");
        assert_eq!(log.lock().unwrap().len(), antes);
        // Editar: a imagem de entrada vai inteira, em base64, com o tipo lido dos bytes.
        let mut entrada = png(10, 10);
        entrada.extend_from_slice(b"resto-da-imagem");
        std::fs::create_dir_all(c.workdir.join("fotos")).unwrap();
        std::fs::write(c.workdir.join("fotos/gato.png"), &entrada).unwrap();
        let r = t
            .run(
                json!({"prompt":"deixe o gato azul","input_image":"fotos/gato.png","output":"e.png"}),
                &c,
            )
            .await
            .unwrap();
        assert!(r.content.contains("300x200"), "{}", r.content);
        let saida = std::fs::read(c.workdir.join("e.png")).unwrap();
        assert_eq!(phxclaw_agent::visao::info_png(&saida).unwrap().0, 300);
        {
            let l = log.lock().unwrap();
            let j = l.last().unwrap().json();
            let d = &j["contents"][0]["parts"][1]["inline_data"];
            assert_eq!(d["mime_type"], "image/png");
            assert_eq!(
                d["data"],
                base64::engine::general_purpose::STANDARD.encode(&entrada)
            );
            assert_eq!(j["contents"][0]["parts"][0]["text"], "deixe o gato azul");
        }
        // Entrada fora da pasta, inexistente ou que nao e imagem: recusa sem pedido.
        let fora = tmp("nano-fora").join("segredo.png");
        std::fs::write(&fora, png(1, 1)).unwrap();
        std::fs::write(c.workdir.join("nota.txt"), b"texto").unwrap();
        let antes = log.lock().unwrap().len();
        for (img, motivo) in [
            ("../../segredo.png", "fora da pasta"),
            (fora.to_str().unwrap(), "fora da pasta"),
            ("nao-existe.png", "nao encontrada"),
            ("nota.txt", "nao e PNG, JPEG nem WebP"),
        ] {
            let m = t
                .run(json!({"prompt":"x","input_image": img}), &c)
                .await
                .unwrap_err()
                .to_string();
            assert!(m.contains(motivo), "{img}: {m}");
        }
        assert_eq!(
            log.lock().unwrap().len(),
            antes,
            "entrada recusada saiu na rede"
        );
        // JPEG nao vira .png; bloqueio diz o motivo.
        let m = t
            .run(json!({"prompt":"jpeg","output":"j.png"}), &c)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains("image/jpeg") && m.contains("nao PNG"), "{m}");
        assert!(!c.workdir.join("j.png").exists());
        let m = t
            .run(json!({"prompt":"bloqueia"}), &c)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains("SAFETY"), "{m}");
        // Edicao com outro provedor e recusa, nao geracao calada sem a imagem.
        let outro = ImageGenerateTool { provedor: None };
        let m = outro
            .run(json!({"prompt":"x","input_image":"fotos/gato.png"}), &c)
            .await
            .unwrap_err()
            .to_string();
        assert!(m.contains("nanobanana"), "{m}");
        assert!(sem_segredo_no_disco(&raiz, CHAVE_G));
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(fora.parent().unwrap());
    }

    /// O laco de eco dos provedores pagos: cada chamada contra um servidor que ECOA o pedido
    /// inteiro num 401, e contra um que REDIRECIONA para um terceiro. Nenhum erro traz a
    /// chave, todo 3xx e erro, e o terceiro nao recebe pedido nenhum.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nenhum_provedor_pago_devolve_a_chave_no_erro_nem_segue_redirecionamento() {
        let raiz_el = raiz_com(&phxclaw_agent::elevenlabs::SERVICO, CHAVE_EL);
        let raiz_g = raiz_com(&phxclaw_agent::nanobanana::SERVICO, CHAVE_G);
        let (terceiro, roubado) = falso(|_| (200, vec![], b"{}".to_vec())).await;
        let (eco, redir) = eco_e_redirecionador(format!("{terceiro}/roubo")).await;
        let wav =
            std::env::temp_dir().join(format!("phx-voz-eco-{}.wav", phxclaw_types::new_uuid_v7()));
        std::fs::write(&wav, wav_de_pcm(&pcm_sintetico(), 16000)).unwrap();
        let prazo = Duration::from_secs(20);
        let mut n = 0;
        for (base, esperado) in [(eco.as_str(), "401"), (redir.as_str(), "302")] {
            let el = |b: &str| {
                ElevenLabs::novo(
                    b,
                    phxclaw_agent::elevenlabs::SERVICO
                        .credencial(&raiz_el)
                        .unwrap(),
                )
                .unwrap()
            };
            let (e1, e2, e3) = (el(base), el(base), el(base));
            let nb = NanoBanana::novo(
                base,
                "gemini-2.5-flash-image",
                phxclaw_agent::nanobanana::SERVICO
                    .credencial(&raiz_g)
                    .unwrap(),
            )
            .unwrap();
            let w = wav.clone();
            let erros: Vec<(&str, String)> = tokio::task::spawn_blocking(move || {
                vec![
                    (
                        "tts",
                        e1.sintetizar("voz1", &json!({"text":"oi"}), "wav_16000", prazo)
                            .unwrap_err(),
                    ),
                    (
                        "stt",
                        e2.transcrever(
                            std::fs::read(&w).unwrap(),
                            "a.wav",
                            "scribe_v2",
                            None,
                            prazo,
                        )
                        .unwrap_err(),
                    ),
                    ("vozes", e3.vozes(Some("a"), prazo).unwrap_err()),
                    (
                        "nanobanana",
                        nb.gerar("x", Some(("image/png", &png(2, 2))), None, prazo)
                            .unwrap_err(),
                    ),
                ]
            })
            .await
            .unwrap();
            for (quem, e) in erros {
                n += 1;
                assert!(e.contains(esperado), "{quem} em {base}: {e}");
                assert!(
                    !e.contains(CHAVE_EL) && !e.contains(CHAVE_G),
                    "{quem}: chave no erro: {e}"
                );
            }
        }
        assert_eq!(n, 8, "todo provedor pago entra no laco");
        assert!(
            roubado.lock().unwrap().is_empty(),
            "o redirecionamento foi seguido: {:?}",
            roubado.lock().unwrap()
        );
        let _ = std::fs::remove_file(&wav);
        let _ = std::fs::remove_dir_all(&raiz_el);
        let _ = std::fs::remove_dir_all(&raiz_g);
    }

    /// A cadeia de `phxclaw voz` (`conversar`, o mesmo motor que o comando chama) com a fala
    /// pela ElevenLabs FALSA -- que devolve o que o flite fala para o texto pedido -- e a
    /// escuta pelo whisper REAL: a resposta do agente vira fala e volta a texto.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn conversa_por_voz_com_elevenlabs_falsa_e_whisper_real() {
        let (Some(flite_local), Some(ouvir)) = (flite(), whisper()) else {
            return;
        };
        let raiz = raiz_com(&phxclaw_agent::elevenlabs::SERVICO, CHAVE_EL);
        let (base, log) = falso(
            |r| match flite_wav(r.json()["text"].as_str().unwrap_or("")) {
                Some(w) => (200, vec![("content-type", "audio/wav".into())], w),
                None => (500, vec![], b"sem flite".to_vec()),
            },
        )
        .await;
        let falar = fala(
            ElevenLabs::novo(
                &base,
                phxclaw_agent::elevenlabs::SERVICO
                    .credencial(&raiz)
                    .unwrap(),
            )
            .unwrap(),
            Formato::Wav(16000),
            None,
        );
        let entrada = raiz.join("entrada");
        std::fs::create_dir_all(&entrada).unwrap();
        flite_local
            .falar(
                "What is the capital of France?",
                &entrada,
                "t1.wav",
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        let store = TaskStore::new(raiz.join("tasks")).unwrap();
        let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
            "The capital of France is Paris.",
        )]));
        let mut agente = phxclaw_agent::voz::agente_de_conversa(
            montagem::Montagem::new(store.clone()).agent_with(llm),
        );
        agente.config.tool_timeout = Duration::from_secs(180);
        let turnos = conversar(
            &agente,
            "roteiro",
            &[entrada.join("t1.wav")],
            &ouvir,
            &falar,
            || NoObserver,
        )
        .await;
        assert!(turnos[0].erro.is_none(), "{:?}", turnos[0]);
        assert!(
            turnos[0]
                .ouvido
                .to_lowercase()
                .contains("capital of france")
        );
        {
            let l = log.lock().unwrap();
            assert_eq!(l.len(), 1);
            assert_eq!(l[0].json()["text"], "The capital of France is Paris.");
            assert_eq!(l[0].cab("xi-api-key"), Some(CHAVE_EL));
        }
        let wav = turnos[0].wav.clone().unwrap();
        assert!(wav.starts_with(store.workdir(&turnos[0].tarefa)));
        let volta = ouvir
            .transcrever(wav, None, Duration::from_secs(180))
            .await
            .unwrap()
            .to_lowercase();
        assert!(volta.contains("paris"), "a fala da resposta virou: {volta}");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// Nenhum arquivo sob `dir` com `segredo` em texto puro.
fn sem_segredo_no_disco(dir: &Path, segredo: &str) -> bool {
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if std::fs::read(&p)
                .unwrap_or_default()
                .windows(segredo.len())
                .any(|w| w == segredo.as_bytes())
            {
                return false;
            }
        }
    }
    true
}

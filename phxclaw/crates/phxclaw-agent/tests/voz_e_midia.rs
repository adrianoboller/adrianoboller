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

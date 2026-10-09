//! O gatilho de MCP Events (`gatilho_mcp.rs`, R6 do radar) contra um servidor MCP falso
//! local por stdio (python3, no mesmo bwrap das ferramentas MCP) -- sem rede.
//!
//! Prova real: os testes marcados «RED medido» tiveram o defeito reposto de verdade (linha
//! marcada `// REPOSTO`, recompilada, vista cair pelo motivo certo) e o conserto voltou.

mod comum_fluxo;

use comum_fluxo::*;
use phxclaw_agent::api::ApiState;
use phxclaw_agent::fluxo_versoes;
use phxclaw_agent::gatilho_mcp::{self, Assinante, Fim, MENSAGENS_MAX_POR_MINUTO};
use phxclaw_agent::*;
use phxclaw_agent_core::Tool;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A pasta de um teste, apagada no fim -- inclusive quando ele cai.
struct Pasta(PathBuf);

impl std::ops::Deref for Pasta {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pasta(nome: &str) -> Pasta {
    Pasta(std::fs::canonicalize(comum_fluxo::tmp(&format!("mcp-ev-{nome}"))).unwrap())
}

const A: &str = "file:///docs/a.md";

/// Servidor MCP falso com recursos. `argv`: modo, quantos `resources/subscribe` esperar
/// antes de falar, e a marca (para achar o processo no /proc do hospedeiro). Grava `subiu`
/// ao nascer, cada uri assinada em `subs.txt` e a resposta ao `ping` dele em `pong.txt`.
const SERVIDOR: &str = r#"
import json, os, sys, time
modo, esperar = sys.argv[1], int(sys.argv[2])
aqui = os.path.dirname(os.path.abspath(__file__))
def grava(arq, t):
    with open(os.path.join(aqui, arq), "a") as f:
        f.write(t + "\n")
grava("subiu", modo)
def manda(m):
    sys.stdout.write(json.dumps(m) + "\n")
    sys.stdout.flush()
def responde(i, r):
    manda({"jsonrpc": "2.0", "id": i, "result": r})
def mudou(uri, **extra):
    p = {"uri": uri}
    p.update(extra)
    manda({"jsonrpc": "2.0", "method": "notifications/resources/updated", "params": p})
A = "file:///docs/a.md"
def fala():
    if modo == "um":
        mudou(A, title="v1")
        mudou(A, title="v2")
        mudou("file:///outro.md", title="nao assinado")
        manda({"jsonrpc": "2.0", "method": "notifications/resources/list_changed"})
        manda({"jsonrpc": "2.0", "id": "srv-1", "method": "ping"})
    elif modo == "lista":
        manda({"jsonrpc": "2.0", "method": "notifications/resources/list_changed"})
    elif modo == "credencial":
        mudou(A, title="https://usuario:senha-muito-secreta@exemplo.com/x")
    elif modo == "chuva":
        for _ in range(20):
            for n in range(5):
                mudou("file:///docs/%d.md" % n)
            time.sleep(0.1)
    elif modo == "inunda":
        for n in range(5000):
            mudou(A, n=n)
assinados = 0
for linha in sys.stdin:
    m = json.loads(linha)
    if m.get("id") == "srv-1" and "result" in m:
        grava("pong.txt", "pong")
        continue
    metodo, p = m.get("method"), m.get("params") or {}
    if metodo == "initialize":
        responde(m["id"], {"protocolVersion": "2025-06-18",
            "capabilities": {"resources": {"subscribe": modo != "sem-assinatura",
                                           "listChanged": True}},
            "serverInfo": {"name": "docs", "version": "1"}})
    elif metodo == "notifications/initialized" and esperar == 0:
        fala()
    elif metodo == "resources/subscribe":
        grava("subs.txt", p["uri"])
        responde(m["id"], {})
        assinados += 1
        if assinados == esperar:
            fala()
"#;

/// O servidor `docs` no arquivo do operador; devolve o caminho do arquivo.
fn config_mcp(d: &Path, modo: &str, esperar: usize) -> PathBuf {
    let srv = d.join("srv");
    std::fs::create_dir_all(&srv).unwrap();
    std::fs::write(srv.join("srv.py"), SERVIDOR).unwrap();
    let marca = d.file_name().unwrap().to_string_lossy().into_owned();
    let cfg = srv.join("mcp.json");
    std::fs::write(
        &cfg,
        json!({"servidores": [{"nome": "docs", "comando": "python3",
                               "args": ["srv.py", modo, esperar.to_string(), marca]}]})
        .to_string(),
    )
    .unwrap();
    cfg
}

/// O estado da API com a fabrica montando o agente com o `eco` e as capacidades `caps`.
fn estado_mcp(raiz: &Path, caps: &'static [&'static str]) -> ApiState {
    let mut s = estado(raiz);
    let st = s.store.clone();
    s.factory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(Eco::default())];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")])),
            tools,
            AgentConfig::default().grant(caps),
            st.clone(),
        ))
    });
    s
}

/// Projeto com o fluxo `f.json` publicado com a marca `v1` e o rascunho mudado para
/// `rascunho`, e o `gatilhos.json` com o gatilho `g`; devolve o gatilho carregado.
fn projeto(raiz: &Path, gatilho: Value) -> gatilho_mcp::GatilhoMcp {
    let projeto = raiz.join("projeto");
    let pasta = projeto.join(".phxclaw");
    std::fs::create_dir_all(&pasta).unwrap();
    let fluxo = |marca: &str| {
        json!({"nome": "f", "passos": [
            {"id": "a", "ferramenta": "eco", "args": {"texto": marca}},
            {"id": "b", "ferramenta": "eco", "args": {"texto": "{{entrada}}"}}]})
    };
    let arq = gravar(&projeto, "f", &fluxo("v1"));
    fluxo_versoes::publicar(&arq, "primeira").unwrap();
    gravar(&projeto, "f", &fluxo("rascunho"));
    let mut g = gatilho;
    g["nome"] = json!("g");
    g["servidor"] = json!("docs");
    g["fluxo"] = json!("f.json");
    std::fs::write(pasta.join("gatilhos.json"), json!({"mcp": [g]}).to_string()).unwrap();
    let mut gs = gatilho_mcp::carregar(&pasta).expect("carga dos gatilhos mcp");
    assert_eq!(gs.len(), 1);
    gs.remove(0)
}

/// PIDs vivos do python desta pasta (pela marca no argv; zumbi nao conta).
fn vivos(d: &Path) -> Vec<u32> {
    let marca = d.file_name().unwrap().to_string_lossy().into_owned();
    let mut v = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(cmd) = std::fs::read(e.path().join("cmdline")) else {
            continue;
        };
        let cmd = String::from_utf8_lossy(&cmd);
        if !cmd.contains(&marca) || !cmd.starts_with("python3") && !cmd.contains("/python3") {
            continue;
        }
        let zumbi = std::fs::read_to_string(e.path().join("stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(')')
                    .map(|(_, r)| r.trim_start().starts_with('Z'))
            })
            .unwrap_or(true);
        if !zumbi {
            v.push(pid);
        }
    }
    v
}

async fn espera_morrer(d: &Path) -> bool {
    let fim = Instant::now() + Duration::from_secs(5);
    while Instant::now() < fim {
        if vivos(d).is_empty() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Os itens de entrada que o passo `b` recebeu, e a saida do `a` (a marca da versao).
async fn execucao(s: &ApiState, id: &str) -> (String, Vec<Value>) {
    let t = ate_o_estado(s, id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    let r = relatorio(&t);
    (passo(&r, "a").saida.clone(), passo(&r, "b").itens.clone())
}

/// A notificacao do recurso assinado dispara a versao PUBLICADA (nao o rascunho), uma vez
/// so pelas duas notificacoes da mesma uri dentro da janela, com a ultima versao
/// (`title: v2`, `vezes: 2`); a uri nao assinada, a lista (nao pedida) e o `ping` do servidor
/// nao disparam -- e o `ping` e respondido. O processo morre no fim da sessao.
///
/// RED medido (um de cada vez): `criar_fluxo_publicado` trocado por `api::criar_fluxo_com`
/// no `disparar_vencidos` (`// REPOSTO`) -- a execucao devolve `rascunho`; e o prazo do
/// `Freio::anotar` sem a janela (`// REPOSTO`) -- a mesma uri dispara duas vezes.
#[tokio::test]
async fn notificacao_dispara_a_publicada_uma_vez_por_janela() {
    let raiz = pasta("publicada");
    let cfg = config_mcp(&raiz, "um", 1);
    let g = projeto(&raiz, json!({"recursos": [A], "janela_ms": 300}));
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let a = Assinante::new(g, &cfg, &raiz);
    let r = a
        .sessao(&s, Some(Duration::from_millis(1_500)))
        .await
        .expect("sessao");
    assert_eq!(r.fim, Fim::Prazo, "{r:?}");
    assert_eq!(r.n_disparos, 1, "{r:?}");
    assert_eq!(
        r.ignoradas, 3,
        "outra uri, lista nao pedida e o ping: {r:?}"
    );
    let (marca, itens) = execucao(&s, &r.disparos[0]).await;
    assert_eq!(marca, "v1", "o gatilho rodou o rascunho");
    assert_eq!(itens.len(), 1, "{itens:?}");
    let item = &itens[0];
    assert_eq!(item["evento"], "resources/updated");
    assert_eq!(item["uri"], A);
    assert_eq!(item["vezes"], 2);
    assert_eq!(item["parametros"]["title"], "v2", "{item}");
    let srv = cfg.parent().unwrap();
    assert_eq!(
        std::fs::read_to_string(srv.join("subs.txt"))
            .unwrap()
            .trim(),
        A
    );
    assert!(
        srv.join("pong.txt").exists(),
        "o ping do servidor ficou sem resposta"
    );
    assert!(
        espera_morrer(&raiz).await,
        "a sessao deixou o servidor vivo"
    );
}

/// `lista: true` sem recurso: nenhum `resources/subscribe`, e a `list_changed` dispara.
#[tokio::test]
async fn lista_que_mudou_dispara_sem_assinar_recurso() {
    let raiz = pasta("lista");
    let cfg = config_mcp(&raiz, "lista", 0);
    let g = projeto(&raiz, json!({"lista": true, "janela_ms": 100}));
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let r = Assinante::new(g, &cfg, &raiz)
        .sessao(&s, Some(Duration::from_millis(800)))
        .await
        .expect("sessao");
    assert_eq!(r.n_disparos, 1, "{r:?}");
    let (_, itens) = execucao(&s, &r.disparos[0]).await;
    assert_eq!(itens[0]["evento"], "resources/list_changed");
    assert!(!cfg.parent().unwrap().join("subs.txt").exists());
    assert!(espera_morrer(&raiz).await);
}

/// Sem `mcp.docs` no agente da fabrica, o gatilho nao sobe o servidor do operador (nem o
/// processo nasce), diz a capacidade que falta, e a recusa fica na evidencia do gatilho.
/// Concedida, o mesmo gatilho sobe.
///
/// RED medido: a conferencia da capacidade em `Assinante::sessao` retirada (`// REPOSTO`)
/// -- o servidor sobe e assina sem a concessao.
#[tokio::test]
async fn sem_a_capacidade_o_gatilho_nao_sobe_o_servidor() {
    let raiz = pasta("capacidade");
    let cfg = config_mcp(&raiz, "um", 1);
    let g = projeto(&raiz, json!({"recursos": [A], "janela_ms": 100}));
    let a = Assinante::new(g, &cfg, &raiz);
    let negado = estado_mcp(&raiz, &["fs.read"]);
    let e = match a.sessao(&negado, Some(Duration::from_millis(500))).await {
        Err(e) => e,
        Ok(r) => panic!("o gatilho subiu sem mcp.docs: {r:?}"),
    };
    assert!(e.contains("mcp.docs"), "{e}");
    let srv = cfg.parent().unwrap();
    assert!(!srv.join("subiu").exists(), "o processo do servidor nasceu");
    let ev = std::fs::read_to_string(raiz.join("gatilhos/mcp-g.evidence.jsonl")).unwrap();
    assert!(
        ev.contains("mcp.docs") && ev.contains("resources/subscribe"),
        "{ev}"
    );

    let concedido = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let r = a
        .sessao(&concedido, Some(Duration::from_millis(500)))
        .await
        .expect("com a capacidade");
    assert!(srv.join("subiu").exists());
    assert_eq!(r.n_disparos, 1, "{r:?}");
    let _ = execucao(&concedido, &r.disparos[0]).await;
    assert!(espera_morrer(&raiz).await);
}

/// A notificacao e dado de fora: com forma de credencial (URL com senha) no parametro, o
/// disparo e recusado pela guarda de entrada do motor e NAO vira tarefa; a recusa diz o
/// motivo e o segredo nao vai a ela.
///
/// RED medido: a guarda de entrada antes da tarefa (`conferir_entrada` em
/// `api::criar_fluxo_ate`) retirada (`// REPOSTO`) -- a tarefa e criada (e cai depois).
#[tokio::test]
async fn notificacao_com_credencial_nao_vira_tarefa() {
    let raiz = pasta("credencial");
    let cfg = config_mcp(&raiz, "credencial", 1);
    let g = projeto(&raiz, json!({"recursos": [A], "janela_ms": 100}));
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let r = Assinante::new(g, &cfg, &raiz)
        .sessao(&s, Some(Duration::from_millis(800)))
        .await
        .expect("sessao");
    assert_eq!(r.n_disparos, 0, "{r:?}");
    assert!(
        s.store.list().unwrap().is_empty(),
        "a notificacao com credencial virou tarefa"
    );
    assert_eq!(r.recusas.len(), 1, "{r:?}");
    assert!(r.recusas[0].contains("credencial"), "{r:?}");
    assert!(!r.recusas[0].contains("senha-muito-secreta"), "{r:?}");
    assert!(espera_morrer(&raiz).await);
}

/// Cinco recursos mudando a cada 100 ms por 2 s (100 avisos, abaixo do teto de inundacao):
/// com `max_por_minuto: 2`, saem DOIS disparos, e o resto fica contado como descartado no
/// fim da sessao em vez de virar vinte tarefas.
///
/// RED medido: a vaga da taxa ignorada em `Freio::vencidos` (`// REPOSTO`) -- a chuva vira
/// uma tarefa por janela.
#[tokio::test]
async fn chuva_de_avisos_fica_na_taxa_maxima() {
    let raiz = pasta("chuva");
    let cfg = config_mcp(&raiz, "chuva", 5);
    let recursos: Vec<String> = (0..5).map(|n| format!("file:///docs/{n}.md")).collect();
    let g = projeto(
        &raiz,
        json!({"recursos": recursos, "janela_ms": 100, "max_por_minuto": 2}),
    );
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let r = Assinante::new(g, &cfg, &raiz)
        .sessao(&s, Some(Duration::from_millis(3_000)))
        .await
        .expect("sessao");
    assert_eq!(r.fim, Fim::Prazo, "{r:?}");
    assert!(r.mensagens >= 100, "{r:?}");
    assert_eq!(r.n_disparos, 2, "{r:?}");
    assert!(r.descartadas > 0, "{r:?}");
    for id in &r.disparos {
        let _ = execucao(&s, id).await;
    }
    assert!(espera_morrer(&raiz).await);
}

/// O servidor que inunda (5.000 avisos de uma vez) derruba a PROPRIA assinatura: a sessao
/// termina por inundacao logo depois do teto, o processo morre, e sai no maximo um disparo
/// (a taxa e 1 por minuto). O agente segue: a mesma API cria tarefa depois.
///
/// RED medido: o teto de mensagens por minuto retirado de `Assinante::sessao`
/// (`// REPOSTO`) -- a sessao so termina pelo prazo, com o servidor despejando o tempo todo.
#[tokio::test]
async fn inundacao_derruba_a_assinatura_e_nao_o_agente() {
    let raiz = pasta("inunda");
    let cfg = config_mcp(&raiz, "inunda", 1);
    let g = projeto(
        &raiz,
        json!({"recursos": [A], "janela_ms": 100, "max_por_minuto": 1}),
    );
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let t0 = Instant::now();
    let r = Assinante::new(g.clone(), &cfg, &raiz)
        .sessao(&s, Some(Duration::from_secs(10)))
        .await
        .expect("sessao");
    assert_eq!(r.fim, Fim::Inundacao(MENSAGENS_MAX_POR_MINUTO + 1), "{r:?}");
    assert!(t0.elapsed() < Duration::from_secs(8), "{:?}", t0.elapsed());
    assert!(r.n_disparos <= 1, "{r:?}");
    assert!(
        espera_morrer(&raiz).await,
        "a inundacao deixou o servidor vivo"
    );
    for id in &r.disparos {
        let _ = execucao(&s, id).await;
    }
    // O agente segue de pe: o mesmo disparo publicado, por outra porta, cria tarefa.
    let c = fluxo_versoes::criar_fluxo_publicado(&s, &g.fluxo, vec![json!({"x": 1})], |_| Ok(()))
        .map_err(|e| e.erro)
        .unwrap();
    let (marca, _) = execucao(&s, &c.id).await;
    assert_eq!(marca, "v1");
}

/// A carga recusa o que nao tem o que escutar, a janela fora da faixa, a taxa acima do teto,
/// e o servidor declarado por `url` e recusado ao subir, dizendo que so stdio assina.
#[tokio::test]
async fn carga_recusa_o_que_nao_escuta_e_url_recusa_ao_subir() {
    let raiz = pasta("carga");
    let pasta_g = raiz.join("projeto/.phxclaw");
    std::fs::create_dir_all(&pasta_g).unwrap();
    gravar(
        &raiz.join("projeto"),
        "f",
        &json!({"nome": "f", "passos": [{"id": "a", "ferramenta": "eco", "args": {"texto": "x"}}]}),
    );
    for (g, motivo) in [
        (json!({}), "nada a escutar"),
        (json!({"recursos": [A], "janela_ms": 10}), "janela_ms"),
        (
            json!({"recursos": [A], "max_por_minuto": 61}),
            "max_por_minuto",
        ),
        (json!({"recursos": [A, A]}), "repetido"),
        (json!({"recursos": [A], "comando": "sh"}), "unknown field"),
    ] {
        let mut g = g;
        g["nome"] = json!("g");
        g["servidor"] = json!("docs");
        g["fluxo"] = json!("f.json");
        std::fs::write(
            pasta_g.join("gatilhos.json"),
            json!({"mcp": [g]}).to_string(),
        )
        .unwrap();
        let e = gatilho_mcp::carregar(&pasta_g).unwrap_err();
        assert!(e.contains(motivo), "{motivo}: {e}");
    }
    let srv = raiz.join("srv");
    std::fs::create_dir_all(&srv).unwrap();
    let cfg = srv.join("mcp.json");
    std::fs::write(
        &cfg,
        json!({"servidores": [{"nome": "docs", "url": "http://127.0.0.1:9/mcp"}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        pasta_g.join("gatilhos.json"),
        json!({"mcp": [{"nome": "g", "servidor": "docs", "fluxo": "f.json", "recursos": [A]}]})
            .to_string(),
    )
    .unwrap();
    let g = gatilho_mcp::carregar(&pasta_g).unwrap().remove(0);
    let s = estado_mcp(&raiz, &["fs.read", "mcp.docs"]);
    let e = Assinante::new(g, &cfg, &raiz)
        .sessao(&s, Some(Duration::from_millis(200)))
        .await
        .unwrap_err();
    assert!(e.contains("so vale por stdio"), "{e}");
}

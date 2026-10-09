//! PHX Flow Engine, onda 2 (SP000035): sub-fluxo, `rodar --ate`, gatilho que dispara
//! fluxo, variaveis do fluxo e os nos skill/mcp/comando/comportamento pelo MESMO portao --
//! mais os pareceres que chegaram junto: DBA (assinatura canonica, formato do relatorio,
//! `saida` derivada, disco que falha), seguranca A3 (teto de itens, vagas de
//! `max_paralelo`) e as lacunas da prova real da onda 1.
//!
//! Prova real: cada teste traz, no comentario, a linha que reposta o derruba. Os marcados
//! «RED medido» tiveram o defeito reposto de verdade em 06/10/2026, o teste caiu, e o
//! conserto voltou; os outros dizem a linha e NAO foram repostos um a um.
//!
//! Dubles: `eco` (ferramenta) e o modelo roteirizado pelo objetivo. Toda chamada continua
//! passando pelo `Agent::call_tool`; as ferramentas falsas `skill_load`, `team_delegate` e
//! `mcp__srv__ferr` existem so para provar que o passo chega a elas PELO portao.

use chrono::Utc;
use phxclaw_agent::agenda::ScheduleSpec;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, disparar_agenda_com_handles, router};
use phxclaw_agent::fluxos::{self, Execucao};
use phxclaw_agent::gatilhos::{
    GatilhoDeArquivo, GatilhoDeWebhook, Gatilhos, Observador, disparar_arquivos,
};
use phxclaw_agent::subfluxo::FluxoTool;
use phxclaw_agent::*;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, Tool, ToolContext, ToolError,
    ToolOutput, ToolSpec,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-onda2-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Quantos estao em voo agora e o maximo visto: a ferramenta e o modelo dividem o mesmo,
/// porque o teto de `max_paralelo` e da onda inteira, nao de um tipo de passo.
#[derive(Default)]
struct Voo {
    agora: AtomicUsize,
    max: AtomicUsize,
}

impl Voo {
    fn entra(&self) {
        let n = self.agora.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(n, Ordering::SeqCst);
    }
    fn sai(&self) {
        self.agora.fetch_sub(1, Ordering::SeqCst);
    }
}

/// `eco`: devolve `texto` (cru, ou JSON quando nao e texto) depois de `dorme_ms`; com
/// `falhar` nao vazio, falha com essa mensagem.
struct Eco {
    chamadas: AtomicUsize,
    voo: Arc<Voo>,
}

impl Tool for Eco {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "eco".into(),
            description: "eco".into(),
            parameters: json!({"type":"object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            self.voo.entra();
            if let Some(ms) = args.get("dorme_ms").and_then(Value::as_u64) {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            self.voo.sai();
            if let Some(m) = args
                .get("falhar")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
            {
                return Err(ToolError::Failed(m.into()));
            }
            Ok(ToolOutput::text(match args.get("texto") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            }))
        })
    }
}

/// Ferramenta falsa com o nome de uma real: so prova que o passo chegou a ela pelo portao.
struct Falsa {
    nome: &'static str,
    chamadas: AtomicUsize,
}

impl Tool for Falsa {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.nome.into(),
            description: "falsa".into(),
            parameters: json!({"type":"object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::text(match self.nome {
                "skill_load" => "[[eco:skill-ok]]".to_string(),
                "team_delegate" => format!(
                    "{} fez {}",
                    args["role"].as_str().unwrap_or("?"),
                    args["task"].as_str().unwrap_or("?")
                ),
                _ => args.to_string(),
            }))
        })
    }
}

/// Modelo roteirizado pelo objetivo: `[[eco:X]]` responde X; `[[lento:MS]]` dorme antes.
struct Modelo {
    chamadas: AtomicUsize,
    voo: Arc<Voo>,
}

impl Llm for Modelo {
    fn id(&self) -> String {
        "por-objetivo".into()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        _tools: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            let obj = messages
                .iter()
                .find(|m| m.role == Role::User)
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let marca = |nome: &str| {
                let i = obj.find(&format!("[[{nome}"))?;
                let resto = &obj[i + 2 + nome.len()..];
                let fim = resto.find("]]")?;
                Some(resto[..fim].trim_start_matches(':').to_string())
            };
            self.voo.entra();
            if let Some(ms) = marca("lento") {
                tokio::time::sleep(Duration::from_millis(ms.parse().unwrap())).await;
            }
            self.voo.sai();
            Ok(ScriptedLlm::text(
                &marca("eco").unwrap_or_else(|| "nada".into()),
            ))
        })
    }
}

struct Banca {
    a: Agent,
    eco: Arc<Eco>,
    modelo: Arc<Modelo>,
    voo: Arc<Voo>,
    falsas: Vec<Arc<Falsa>>,
}

/// O agente dos testes; com `fluxos`, a ferramenta `fluxo` entra e recebe o agente montado
/// DEPOIS, como a montagem faz.
fn banca(caps: &[&str], fluxos: Option<&Path>) -> Banca {
    let voo = Arc::new(Voo::default());
    let eco = Arc::new(Eco {
        chamadas: AtomicUsize::new(0),
        voo: voo.clone(),
    });
    let modelo = Arc::new(Modelo {
        chamadas: AtomicUsize::new(0),
        voo: voo.clone(),
    });
    let falsas: Vec<Arc<Falsa>> = ["skill_load", "team_delegate", "mcp__srv__ferr"]
        .into_iter()
        .map(|nome| {
            Arc::new(Falsa {
                nome,
                chamadas: AtomicUsize::new(0),
            })
        })
        .collect();
    let mut tools: Vec<Arc<dyn Tool>> =
        vec![eco.clone(), Arc::new(WriteFileTool), Arc::new(ReadFileTool)];
    tools.extend(falsas.iter().map(|f| f.clone() as Arc<dyn Tool>));
    let ft = fluxos.map(|p| Arc::new(FluxoTool::nova(p)));
    if let Some(ft) = &ft {
        tools.push(ft.clone());
    }
    let a = Agent::new(
        modelo.clone(),
        tools,
        AgentConfig::default().grant(caps),
        TaskStore::new(tmp("tarefas")).unwrap(),
    );
    if let Some(ft) = ft {
        let _ = ft.base.set(a.clone());
    }
    Banca {
        a,
        eco,
        modelo,
        voo,
        falsas,
    }
}

fn fluxo(v: Value) -> fluxos::Fluxo {
    fluxos::ler(&v.to_string()).unwrap()
}

fn passo<'a>(r: &'a fluxos::Relatorio, id: &str) -> &'a fluxos::Resultado {
    r.passos
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("passo {id} nao esta no relatorio: {r:#?}"))
}

fn sha256(t: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(t.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn gravar(pasta: &Path, nome: &str, v: &Value) -> PathBuf {
    let p = pasta.join(format!("{nome}.json"));
    std::fs::write(&p, v.to_string()).unwrap();
    p
}

/// Tarefas que sao execucoes do fluxo `nome`.
fn execucoes(a: &Agent, nome: &str) -> Vec<Task> {
    a.store
        .list()
        .unwrap()
        .into_iter()
        .filter(|t| t.objective == format!("{}{nome}", fluxos::PREFIXO_TAREFA))
        .collect()
}

// ======================================================================== onda 2

/// `subfluxo`: a ferramenta `fluxo` roda um fluxo gravado com itens de entrada, em `each`
/// (uma execucao por item) e `once` (a lista inteira), pelo portao (`flow.run`); o
/// sub-fluxo e filho da tarefa que o chamou. Ciclo (A -> A) e recusado pela assinatura na
/// cadeia de `parent`; 8 fluxos empilhados rodam e o 9o e recusado; `caminho` passa pelo
/// `confine` da pasta da tarefa -- e o fluxo que o proprio agente ESCREVEU ali roda.
///
/// Reposta que derruba: em `fluxos::conferir_cadeia`, `profundidade > MAX_PROFUNDIDADE`
/// trocado por `false` (a cadeia de 9 roda); em `subfluxo.rs`, `caminho` de volta a
/// `PathBuf::from(c)` (o `../fora.json` deixa de ser recusado). RED medido: a recusa de
/// ciclo desligada (`&& r.fluxo_sha256 == hash` trocado por `&& false`) deixa o fluxo
/// chamar a si mesmo ate o teto de profundidade: 8 execucoes onde devia haver 1, e cai.
#[tokio::test]
async fn subfluxo() {
    let pasta = tmp("fluxos");
    gravar(
        &pasta,
        "filho",
        &json!({"nome":"filho","passos":[
            {"id":"cada","entrada":"entrada","por_item":true,"ferramenta":"eco",
             "args":{"texto":"filho {{entrada.n}}"}}
        ]}),
    );
    let b = banca(&["fs.read", "fs.write", "flow.run"], Some(&pasta));
    let f = fluxo(json!({"nome":"pai","passos":[
        {"id":"lista","ferramenta":"eco","args":{"texto":[{"n":1},{"n":2}]}},
        {"id":"cada_um","depende":["lista"],"ferramenta":"fluxo",
         "args":{"nome":"filho","entrada":"{{lista}}","modo":"each"}},
        {"id":"uma_vez","depende":["lista"],"ferramenta":"fluxo",
         "args":{"nome":"filho","entrada":"{{lista}}"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(
        passo(&r, "cada_um").itens,
        vec![json!("filho 1"), json!("filho 2")]
    );
    assert_eq!(
        passo(&r, "uma_vez").itens,
        vec![json!("filho 1"), json!("filho 2")]
    );
    let filhos = execucoes(&b.a, "filho");
    assert_eq!(filhos.len(), 3, "each = 2 execucoes, once = 1");
    assert!(
        filhos
            .iter()
            .all(|t| t.parent.as_deref() == Some(r.tarefa.as_str())),
        "o sub-fluxo e filho da tarefa que chamou a ferramenta"
    );
    // 1 (lista) + 2 (each, um item cada) + 2 (once, por_item nos dois)
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), 5);

    // sem `flow.run`, a ferramenta e negada no portao e o filho nem nasce
    let sem = banca(&["fs.read"], Some(&pasta));
    let r = fluxos::rodar(&sem.a, &f).await.unwrap();
    assert!(!r.sucesso);
    assert_eq!(passo(&r, "cada_um").estado, "falhou");
    assert!(execucoes(&sem.a, "filho").is_empty());

    // ciclo: o fluxo que chama a si mesmo
    gravar(
        &pasta,
        "ciclo",
        &json!({"nome":"ciclo","passos":[{"id":"de_novo","ferramenta":"fluxo","args":{"nome":"ciclo"}}]}),
    );
    let c = fluxos::ler(&std::fs::read_to_string(pasta.join("ciclo.json")).unwrap()).unwrap();
    let r = fluxos::rodar(&b.a, &c).await.unwrap();
    assert!(!r.sucesso);
    let s = &passo(&r, "de_novo").saida;
    assert!(s.contains("ciclo"), "{s}");
    assert_eq!(
        execucoes(&b.a, "ciclo").len(),
        1,
        "o recusado nem chega a criar tarefa: a cadeia e conferida antes"
    );

    // profundidade: 8 empilhados rodam, 9 nao
    for n in [8usize, 9] {
        for i in 1..=n {
            let p = if i == n {
                json!({"id":"fim","ferramenta":"eco","args":{"texto":"fundo"}})
            } else {
                json!({"id":"chama","ferramenta":"fluxo","args":{"nome":format!("c{n}_{}", i + 1)}})
            };
            gravar(
                &pasta,
                &format!("c{n}_{i}"),
                &json!({"nome":format!("c{n}_{i}"),"passos":[p]}),
            );
        }
        let antes = b.eco.chamadas.load(Ordering::SeqCst);
        let topo =
            fluxos::ler(&std::fs::read_to_string(pasta.join(format!("c{n}_1.json"))).unwrap())
                .unwrap();
        let r = fluxos::rodar(&b.a, &topo).await.unwrap();
        let depois = b.eco.chamadas.load(Ordering::SeqCst) - antes;
        if n == 8 {
            assert!(r.sucesso, "8 empilhados cabem no teto: {r:#?}");
            assert_eq!(passo(&r, "chama").itens, vec![json!("fundo")]);
            assert_eq!(depois, 1);
        } else {
            assert!(!r.sucesso);
            let s = &passo(&r, "chama").saida;
            assert!(s.contains("profundidade 9"), "{s}");
            assert_eq!(depois, 0, "o 9o nao rodou passo nenhum");
        }
    }

    // `caminho`: o confine da pasta da tarefa -- fora dela e recusado; o fluxo que o
    // proprio agente escreveu ali dentro roda (o laco do assistente que monta fluxo)
    let gerado = json!({"nome":"gerado","passos":[{"id":"g","ferramenta":"eco","args":{"texto":"gerado ok"}}]})
        .to_string();
    let f = fluxo(json!({"nome":"constroi","passos":[
        {"id":"escreve","ferramenta":"write_file","args":{"path":"gerado.json","content":gerado}},
        {"id":"roda","depende":["escreve"],"ferramenta":"fluxo","args":{"caminho":"gerado.json"}},
        {"id":"fora","ferramenta":"fluxo","args":{"caminho":"../fora.json"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert_eq!(passo(&r, "roda").itens, vec![json!("gerado ok")], "{r:#?}");
    assert_eq!(passo(&r, "fora").estado, "falhou");
    assert!(
        passo(&r, "fora").saida.contains("fora da pasta"),
        "{}",
        passo(&r, "fora").saida
    );
}

/// `execucao_parcial` (`rodar --ate PASSO`): roda so os ancestrais do passo (com ele),
/// grava o progresso como a retomada grava, e os de fora saem `nao_pedido`; `retomar`
/// continua dali sem refazer o que ja rodou. Passo fora do grafo (ou o passo de erro) e
/// recusado antes de rodar qualquer coisa.
///
/// RED medido: `ancestrais()` devolvendo TODOS os passos (o corte ignorado) faz `c` e `d`
/// rodarem no primeiro `rodar_com`, e a asercao `nao_pedido` cai.
#[tokio::test]
async fn execucao_parcial() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"parcial","fluxo_de_erro":"avisa","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"a"}},
        {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}}b"}},
        {"id":"c","depende":["b"],"ferramenta":"eco","args":{"texto":"{{b}}c"}},
        {"id":"d","ferramenta":"eco","args":{"texto":"d"}},
        {"id":"avisa","ferramenta":"eco","args":{"texto":"x"}}
    ]}));
    let r = fluxos::rodar_com(
        &b.a,
        &f,
        Execucao {
            ate: Some("b"),
            ..Execucao::default()
        },
    )
    .await
    .unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(r.ate.as_deref(), Some("b"));
    assert_eq!(passo(&r, "b").saida, "ab");
    assert_eq!(passo(&r, "c").estado, "nao_pedido");
    assert_eq!(passo(&r, "d").estado, "nao_pedido");
    assert!(r.passos.iter().all(|p| p.id != "avisa"));
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), 2);
    // o corte foi para o disco: e de la que a retomada le
    let gravado: fluxos::Relatorio =
        serde_json::from_str(&b.a.store.load(&r.tarefa).unwrap().answer.unwrap()).unwrap();
    assert_eq!(gravado.ate.as_deref(), Some("b"));

    let r2 = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap();
    assert!(r2.sucesso, "{r2:#?}");
    assert!(passo(&r2, "a").reaproveitado && passo(&r2, "b").reaproveitado);
    assert_eq!(passo(&r2, "c").saida, "abc");
    assert_eq!(passo(&r2, "d").estado, "ok");
    assert_eq!(r2.ate, None);
    assert_eq!(
        b.eco.chamadas.load(Ordering::SeqCst),
        4,
        "a e b nao rodaram de novo"
    );

    for ate in ["zzz", "avisa"] {
        let e = fluxos::rodar_com(
            &b.a,
            &f,
            Execucao {
                ate: Some(ate),
                ..Execucao::default()
            },
        )
        .await
        .unwrap_err();
        assert!(e.contains("nao esta no grafo"), "{ate}: {e}");
    }
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), 4);
}

// ------------------------------------------------------------------ gatilhos e agenda

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

/// O estado da API com um agente que tem o `eco` e o `read_file`; o modelo responde
/// «feito» a tarefa de objetivo (o caminho velho).
fn estado(raiz: &Path) -> ApiState {
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(Eco {
                chamadas: AtomicUsize::new(0),
                voo: Arc::new(Voo::default()),
            }),
            Arc::new(ReadFileTool),
        ];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")])),
            tools,
            AgentConfig {
                prazo_de_resposta: Some(Duration::from_secs(20)),
                ..AgentConfig::default()
            }
            .grant(&["fs.read"]),
            st.clone(),
        ))
    });
    ApiState {
        store,
        factory,
        default_model: "roteiro".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    }
}

async fn esperar(s: &ApiState, id: &str) -> Task {
    for _ in 0..400 {
        if let Ok(t) = s.store.load(id)
            && matches!(t.status, TaskStatus::Completed | TaskStatus::Failed)
        {
            return t;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("tarefa {id} nao terminou");
}

fn relatorio(t: &Task) -> fluxos::Relatorio {
    serde_json::from_str(t.answer.as_deref().unwrap()).unwrap()
}

/// `gatilho_dispara_fluxo`: a pasta observada, o webhook e a agenda apontam para um FLUXO
/// pelo campo `fluxo`, e os tres disparam pelo mesmo `api::criar_fluxo_com`: o arquivo
/// copiado entra como item `{{entrada}}`, o corpo JSON do webhook vira itens, a agenda roda
/// o arquivo no disparo. Gatilho com objetivo E fluxo (ou sem nenhum) e fluxo invalido
/// param no carregar, nao no primeiro disparo.
///
/// Reposta que derruba: em `gatilhos::disparar_arquivos`, o `if let Some(f) =
/// &o.gatilho.fluxo` removido (o gatilho cria tarefa com objetivo vazio e o relatorio nao
/// existe); no webhook, `itens_de_texto(&c)` trocado por `vec![json!(c)]` (o corpo vira UM
/// item de texto e `por_item` roda uma vez so).
#[tokio::test]
async fn gatilho_dispara_fluxo() {
    let raiz = tmp("gatilho");
    let s = estado(&raiz);
    let le = gravar(
        &raiz,
        "le",
        &json!({"nome":"le","passos":[{"id":"le","entrada":"entrada","por_item":true,
            "ferramenta":"read_file","args":{"path":"{{entrada.arquivo}}"}}]}),
    );
    let cada = gravar(
        &raiz,
        "cada",
        &json!({"nome":"cada","passos":[{"id":"cada","entrada":"entrada","por_item":true,
            "ferramenta":"eco","args":{"texto":"n={{entrada.n}}"}}]}),
    );

    // pasta observada -> fluxo, com o arquivo copiado como item
    let pasta = raiz.join("entrada");
    std::fs::create_dir_all(&pasta).unwrap();
    let mut obs = vec![Observador::new(GatilhoDeArquivo {
        nome: "vendas".into(),
        pasta: pasta.clone(),
        padrao: Some("*.csv".into()),
        objetivo: String::new(),
        fluxo: Some(le.to_string_lossy().into_owned()),
    })];
    assert!(disparar_arquivos(&s, &mut obs).is_empty(), "linha de base");
    std::fs::write(pasta.join("vendas.csv"), "a,1\n").unwrap();
    assert!(disparar_arquivos(&s, &mut obs).is_empty(), "ainda mudando");
    let mut v = disparar_arquivos(&s, &mut obs);
    assert_eq!(v.len(), 1);
    let t = v.pop().unwrap().unwrap().fim.await.unwrap();
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert!(t.objective.starts_with(fluxos::PREFIXO_TAREFA));
    assert_eq!(passo(&relatorio(&t), "le").saida, "a,1\n");

    // webhook -> fluxo, com o corpo JSON como itens
    let g = Arc::new(Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "pedidos".into(),
            objetivo: String::new(),
            fluxo: Some(cada.to_string_lossy().into_owned()),
            segredo: Some("segredo-do-gatilho-1234567890".into()),
        }],
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(s.clone()).merge(phxclaw_agent::gatilhos::router(s.clone(), g));
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let resp = reqwest::Client::new()
        .post(format!("{base}/v1/triggers/pedidos"))
        .header("x-phxclaw-segredo", "segredo-do-gatilho-1234567890")
        .body(r#"[{"n":1},{"n":2}]"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 202);
    let id = resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let t = esperar(&s, &id).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert_eq!(
        passo(&relatorio(&t), "cada").itens,
        vec![json!("n=1"), json!("n=2")]
    );

    // agenda -> fluxo, pelo campo explicito
    let item = s
        .agenda
        .lock()
        .unwrap()
        .add_fluxo(
            "noturno",
            &cada.to_string_lossy(),
            ScheduleSpec::EverySeconds(60),
            Utc::now() - chrono::Duration::minutes(2),
        )
        .unwrap();
    assert_eq!(item.fluxo.as_deref(), Some(&*cada.to_string_lossy()));
    for h in disparar_agenda_com_handles(&s) {
        h.await.unwrap();
    }
    let id = s.agenda.lock().unwrap().items[0].last_task.clone().unwrap();
    let t = s.store.load(&id).unwrap();
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert!(relatorio(&t).sucesso);

    // o carregar recusa o que nao sabe qual disparar, e o fluxo invalido
    let ph = raiz.join(".phxclaw");
    std::fs::create_dir_all(&ph).unwrap();
    std::fs::write(raiz.join("ruim.json"), r#"{"nome":"ruim","passos":[]}"#).unwrap();
    let casos = [
        (
            json!({"webhooks":[{"nome":"w","objetivo":"x","fluxo":"cada.json"}]}),
            "exatamente um",
        ),
        (
            json!({"arquivos":[{"nome":"a","pasta":"p"}]}),
            "exatamente um",
        ),
        (
            json!({"webhooks":[{"nome":"w","fluxo":"ruim.json"}]}),
            "gatilho: fluxo ruim.json",
        ),
    ];
    for (g, esperado) in casos {
        std::fs::write(ph.join("gatilhos.json"), g.to_string()).unwrap();
        let e = Gatilhos::carregar(&ph).unwrap_err();
        assert!(e.contains(esperado), "{g}: {e}");
    }
    // e o caminho relativo se resolve contra a raiz do projeto
    std::fs::write(
        ph.join("gatilhos.json"),
        json!({"webhooks":[{"nome":"w","fluxo":"cada.json"}]}).to_string(),
    )
    .unwrap();
    let g = Gatilhos::carregar(&ph).unwrap();
    assert_eq!(
        g.webhooks[0].fluxo.as_deref(),
        Some(&*cada.to_string_lossy())
    );
}

/// O comportamento VELHO do gatilho e da agenda, intacto: `gatilhos.json` sem o campo
/// `fluxo` carrega igual; gatilho com `objetivo` cria TAREFA com o objetivo de sempre;
/// agenda com objetivo comum cria tarefa de modelo; e o prefixo antigo `fluxo: ARQ` no
/// objetivo continua rodando o fluxo.
///
/// Reposta que derruba: em `api::disparar_agenda_com_handles`, o `.or_else(||
/// fluxo_do_objetivo(..))` removido (o `fluxo: ARQ` vira objetivo de modelo e a tarefa
/// nao tem relatorio); em `gatilhos.rs`, o `#[serde(default)]` de `objetivo` removido nao
/// derruba ESTE teste (derruba o de cima) -- o daqui e o `fluxo` ausente.
#[tokio::test]
async fn gatilho_com_objetivo_comportamento_velho() {
    let raiz = tmp("velho");
    let s = estado(&raiz);
    let ph = raiz.join(".phxclaw");
    std::fs::create_dir_all(&ph).unwrap();
    std::fs::write(
        ph.join("gatilhos.json"),
        json!({"arquivos":[{"nome":"v","pasta":"entrada","padrao":"*.csv","objetivo":"Some {arquivos}"}],
               "webhooks":[{"nome":"w","objetivo":"Analise {corpo}"}]})
        .to_string(),
    )
    .unwrap();
    let g = Gatilhos::carregar(&ph).unwrap();
    assert!(g.arquivos[0].fluxo.is_none() && g.webhooks[0].fluxo.is_none());

    let pasta = raiz.join("entrada");
    std::fs::create_dir_all(&pasta).unwrap();
    let mut obs = vec![Observador::new(g.arquivos[0].clone())];
    assert!(disparar_arquivos(&s, &mut obs).is_empty());
    std::fs::write(pasta.join("x.csv"), "1").unwrap();
    assert!(disparar_arquivos(&s, &mut obs).is_empty());
    let t = disparar_arquivos(&s, &mut obs)
        .pop()
        .unwrap()
        .unwrap()
        .fim
        .await
        .unwrap();
    assert_eq!(t.objective, "[gatilho v] Some gatilho/x.csv");
    assert_eq!(t.answer.as_deref(), Some("feito"));

    let cada = gravar(
        &raiz,
        "cada",
        &json!({"nome":"cada","passos":[{"id":"um","ferramenta":"eco","args":{"texto":"rodou"}}]}),
    );
    let antes = Utc::now() - chrono::Duration::minutes(2);
    {
        let mut ag = s.agenda.lock().unwrap();
        ag.add(
            "prefixo",
            &format!("fluxo: {}", cada.display()),
            ScheduleSpec::EverySeconds(60),
            antes,
        )
        .unwrap();
        ag.add(
            "modelo",
            "resuma o dia",
            ScheduleSpec::EverySeconds(60),
            antes,
        )
        .unwrap();
        assert!(ag.items.iter().all(|i| i.fluxo.is_none()));
    }
    for h in disparar_agenda_com_handles(&s) {
        h.await.unwrap();
    }
    let itens = s.agenda.lock().unwrap().items.clone();
    let pelo_prefixo = s
        .store
        .load(itens[0].last_task.as_deref().unwrap())
        .unwrap();
    assert_eq!(pelo_prefixo.objective, "fluxo: cada");
    assert_eq!(passo(&relatorio(&pelo_prefixo), "um").saida, "rodou");
    let de_modelo = s
        .store
        .load(itens[1].last_task.as_deref().unwrap())
        .unwrap();
    assert_eq!(de_modelo.objective, "resuma o dia");
    assert_eq!(de_modelo.status, TaskStatus::Completed);
    assert_eq!(de_modelo.answer.as_deref(), Some("feito"));
}

// ------------------------------------------------------------------ variaveis

/// `variaveis_globais`: `{{var.nome}}` e `{{var.nome.campo}}` em texto, em argumento (com
/// o tipo) e em objetivo de subagente; `{"config": "chave"}` le o `config.json` no disparo.
/// Variavel nao declarada, nome invalido, passo chamado `var` e chave de config fora do
/// catalogo param na leitura; chave catalogada sem valor para o fluxo ANTES do primeiro
/// passo.
///
/// Reposta que derruba: em `validar`, o ramo `if id == "var"` removido (todo `{{var.x}}`
/// vira «sem declarar 'var' em depende»); em `executar`, `mem.fixos.insert("var", ..)`
/// removido (a expressao nao acha `var` na visao e o passo falha).
#[tokio::test]
async fn variaveis_globais() {
    // a pasta da configuracao sem config.json: vale o padrao do catalogo
    phxclaw_agent::config::fixar_pasta(&tmp("config"));
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"vars",
    "variaveis":{"cidade":"Blumenau","limites":{"max":3},"tentativas":{"config":"agente.tentativas_argumento"}},
    "passos":[
        {"id":"texto","ferramenta":"eco","args":{"texto":"{{var.cidade}}/{{var.limites.max}}/{{var.tentativas}}"}},
        {"id":"valor","ferramenta":"eco","args":{"texto":"{{var.limites}}"}},
        {"id":"agente","tarefa":"[[eco:em {{var.cidade}}]]"}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "texto").saida, "Blumenau/3/2");
    assert_eq!(passo(&r, "valor").itens, vec![json!({"max":3})]);
    assert_eq!(passo(&r, "agente").saida, "em Blumenau");

    let casos = [
        (
            json!({"nome":"v","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{var.nada}}"}}]}),
            "a variavel 'nada' nao esta em 'variaveis'",
        ),
        (
            json!({"nome":"v","variaveis":{"a-b":1},"passos":[{"id":"a","ferramenta":"eco"}]}),
            "nome de variavel invalido",
        ),
        (
            json!({"nome":"v","passos":[{"id":"var","ferramenta":"eco"}]}),
            "id de passo invalido",
        ),
    ];
    for (f, esperado) in casos {
        let e = fluxos::ler(&f.to_string()).unwrap_err();
        assert!(e.contains(esperado), "{f}: {e}");
    }
    // `nao.existe` derrubava o processo (o `Configuracao::valor` para em chave fora do
    // catalogo): agora e recusa na leitura, com o motivo
    let e = fluxos::ler(
        &json!({"nome":"v","variaveis":{"x":{"config":"nao.existe"}},
            "passos":[{"id":"a","ferramenta":"eco"}]})
        .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("'nao.existe' nao existe"), "{e}");
    let f = fluxo(json!({"nome":"v","variaveis":{"x":{"config":"n8n.url"}},
        "passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{var.x}}"}}]}));
    let antes = b.eco.chamadas.load(Ordering::SeqCst);
    let e = fluxos::rodar(&b.a, &f).await.unwrap_err();
    assert!(e.contains("nao esta definida"), "{e}");
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), antes, "nada rodou");
}

/// A guarda de segredo das variaveis (`variavel_parece_segredo`, que nao tinha teste):
/// recusa pelo NOME (a mesma lista do `gravacao::redigir`, inclusive o sufixo
/// `_token`), pela FORMA do valor (o que a tarja do broker mudaria), dentro de objeto e de
/// lista; e a chave de config que e segredo -- pelo nome ou pelo catalogo -- e recusada na
/// leitura. O que so parece (`max_tokens`, `Blumenau`) passa.
///
/// RED medido: `variavel_parece_segredo` devolvendo `false` sempre -- os seis casos de
/// recusa na leitura passam a ler, e o teste cai no primeiro.
#[test]
fn variavel_parece_segredo() {
    let com = |vars: Value| {
        json!({"nome":"s","variaveis":vars,"passos":[{"id":"a","ferramenta":"eco"}]}).to_string()
    };
    let recusadas = [
        json!({"api_key": "qualquer"}),
        json!({"github_token": "abc"}),
        json!({"senha": 1}),
        json!({"chave": "ghp_0123456789abcdefABCDEF"}),
        json!({"conexao": {"password": "x"}}),
        json!({"lista": ["ok", "sk-proj-ABCdef0123456789xyz"]}),
    ];
    for v in recusadas {
        let e = fluxos::ler(&com(v.clone())).unwrap_err();
        assert!(e.contains("parece segredo"), "{v}: {e}");
    }
    for v in [
        json!({"max_tokens": 100}),
        json!({"cidade": "Blumenau", "obs": "a senha fica no broker"}),
    ] {
        fluxos::ler(&com(v.clone())).unwrap_or_else(|e| panic!("{v}: {e}"));
    }
    // a chave de config: pelo nome (`github_token`) e pelo catalogo (`api.token` e
    // `Segredo` la, e o nome sozinho nao diria)
    for chave in ["github_token", "api.token"] {
        let e = fluxos::ler(&com(json!({"x": {"config": chave}}))).unwrap_err();
        assert!(e.contains("e segredo"), "{chave}: {e}");
    }
}

// ------------------------------------------------------------------ nos pelo portao

/// Os nos da onda 2 que viram chamada: `skill` passa pelo `skill_load` e o corpo vira
/// objetivo do subagente; `mcp` vira `mcp__servidor__ferramenta`; `comportamento.papel`
/// vira `team_delegate`. Os tres pelo `call_tool` (as falsas contam). `comando` sem o
/// comando no projeto falha dizendo isso, em vez de mandar `/nome` cru ao subagente; e as
/// combinacoes sem sentido param na leitura.
///
/// Reposta que derruba: em `rodar_grupo`, o `skill_load` trocado pela leitura direta do
/// `skills.rs` (a falsa nao e chamada); no `tipo()`, o ramo do `mcp` montando o nome sem o
/// prefixo `mcp__` (a ferramenta nao existe e o passo falha).
#[tokio::test]
async fn nos_da_onda2_passam_pelo_portao() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"nos","passos":[
        {"id":"dado","ferramenta":"eco","args":{"texto":"planilha"}},
        {"id":"sk","depende":["dado"],"skill":"resumir"},
        {"id":"m","mcp":{"servidor":"srv","ferramenta":"ferr"},"args":{"q":1}},
        {"id":"papel","tarefa":"revise o texto","comportamento":{"papel":"revisor"}},
        {"id":"cmd","comando":"/nao-existe agora"}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert_eq!(passo(&r, "sk").saida, "skill-ok", "{r:#?}");
    assert_eq!(passo(&r, "m").itens, vec![json!({"q":1})]);
    assert_eq!(passo(&r, "papel").saida, "revisor fez revise o texto");
    let cmd = passo(&r, "cmd");
    assert_eq!(cmd.estado, "falhou");
    assert!(
        cmd.saida.contains("comando /nao-existe nao existe"),
        "{}",
        cmd.saida
    );
    for falsa in &b.falsas {
        assert_eq!(
            falsa.chamadas.load(Ordering::SeqCst),
            1,
            "{} pelo portao",
            falsa.nome
        );
    }
    let casos = [
        (
            json!({"nome":"c","passos":[{"id":"a","tarefa":"x","comportamento":{"papel":"p","estilo":"e"}}]}),
            "exatamente um",
        ),
        (
            json!({"nome":"c","passos":[{"id":"a","ferramenta":"eco","comportamento":{"papel":"p"}}]}),
            "so envolve um passo de 'tarefa'",
        ),
        (
            json!({"nome":"c","passos":[{"id":"a","mcp":{"servidor":"","ferramenta":"f"}}]}),
            "mcp pede",
        ),
        (
            json!({"nome":"c","passos":[{"id":"a","skill":" "}]}),
            "skill sem nome",
        ),
    ];
    for (f, esperado) in casos {
        let e = fluxos::ler(&f.to_string()).unwrap_err();
        assert!(e.contains(esperado), "{f}: {e}");
    }
}

/// Fluxo SO de nos de controle (`se`, `lote`, `juntar`) sobre a entrada: resolve tudo no
/// proprio motor -- nenhuma chamada de ferramenta, nenhum subagente. Os nos de controle so
/// reorganizam itens que o portao ja deixou entrar.
///
/// Reposta que derruba: em `executar`, o `if !t.e_trabalho()` removido (o no de controle
/// cai no `unreachable!` dos objetivos e o teste entra em panico).
#[tokio::test]
async fn so_controle_nao_chama_o_portao() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"controle","passos":[
        {"id":"grande","entrada":"entrada","se":{"caminho":"v","operador":"maior","valor":4}},
        {"id":"lotes","depende":["grande:verdadeiro"],"lote":1},
        {"id":"tudo","depende":["grande:falso","lotes"],"juntar":{"modo":"append"}}
    ]}));
    let r = fluxos::rodar_com(
        &b.a,
        &f,
        Execucao {
            entrada: vec![json!({"v":1}), json!({"v":5}), json!({"v":9})],
            ..Execucao::default()
        },
    )
    .await
    .unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(
        passo(&r, "tudo").itens,
        vec![json!({"v":1}), json!([{"v":5}]), json!([{"v":9}])]
    );
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), 0, "nenhum call_tool");
    assert_eq!(
        b.modelo.chamadas.load(Ordering::SeqCst),
        0,
        "nenhum subagente"
    );
    assert!(
        b.falsas
            .iter()
            .all(|f| f.chamadas.load(Ordering::SeqCst) == 0)
    );
}

// ------------------------------------------------------------------ parecer do DBA

/// A assinatura e o sha256 do JSON CANONICO: chaves ordenadas em toda profundidade, sem
/// campo no valor padrao. Tres consequencias, as tres conferidas: (1) o numero e o sha do
/// texto canonico escrito aqui a mao -- a definicao da onda 1 assina igual depois de o
/// `Passo` ganhar `max_itens` e o `Fluxo` ganhar `teto_ms` e `variaveis`; (2) a ordem das
/// chaves de `args` no texto do usuario nao muda nada; (3) escrever o padrao explicito
/// tambem nao.
///
/// RED medido: `skip_serializing_if = "e_max_itens"` removido do `Passo::max_itens` --
/// o canonico ganha `"max_itens":1000` e (1) cai. O `ordenado()` desligado NAO derruba
/// (2) neste build (medido: o `preserve_order` so entra pela dependencia de build do
/// tree-sitter, e o `Map` de runtime ja ordena); (2) segura o dia em que ele entrar.
#[test]
fn assinatura_canonica() {
    let canonico = r#"{"nome":"x","passos":[{"args":{"a":2,"b":1},"ferramenta":"eco","id":"a"}]}"#;
    let f =
        fluxo(json!({"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"a":2,"b":1}}]}));
    assert_eq!(fluxos::assinatura(&f), sha256(canonico));
    let invertido = fluxos::ler(
        r#"{"passos":[{"args":{"b":1,"a":2},"id":"a","ferramenta":"eco"}],"nome":"x"}"#,
    )
    .unwrap();
    assert_eq!(fluxos::assinatura(&invertido), sha256(canonico));
    let explicito = fluxo(
        json!({"nome":"x","max_paralelo":4,"teto_ms":3_600_000,"variaveis":{},
        "passos":[{"id":"a","ferramenta":"eco","args":{"a":2,"b":1},"depende":[],"tentativas":1,
        "max_itens":1000,"ao_errar":"parar","por_item":false}]}),
    );
    assert_eq!(fluxos::assinatura(&explicito), sha256(canonico));
    // e o que muda a definicao muda a assinatura
    let outro =
        fluxo(json!({"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"a":3,"b":1}}]}));
    assert_ne!(fluxos::assinatura(&outro), sha256(canonico));
}

/// O par de cima pela retomada: um progresso gravado por um binario que nao conhecia os
/// campos novos (formato 1, `saida` e `itens` os dois, sem `formato`) e retomado por este,
/// e o passo que deu certo e REAPROVEITADO -- a definicao e a mesma.
///
/// Reposta que derruba: a mesma do `assinatura_canonica` (o sha gravado deixa de bater e
/// a retomada recusa «a definicao do fluxo mudou»).
#[tokio::test]
async fn retomada_reaproveita_definicao_gravada_antes_do_campo_novo() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"r","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"um"}},
        {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}} dois"}}
    ]}));
    let canonico = r#"{"nome":"r","passos":[{"args":{"texto":"um"},"ferramenta":"eco","id":"a"},{"args":{"texto":"{{a}} dois"},"depende":["a"],"ferramenta":"eco","id":"b"}]}"#;
    let mut t = fluxos::tarefa_do_fluxo(&f, "por-objetivo");
    t.status = TaskStatus::Failed;
    t.answer = Some(
        json!({"tarefa": t.id, "fluxo_sha256": sha256(canonico), "sucesso": false,
            "passos": [{"id":"a","estado":"ok","saida":"um","itens":["um"],"tarefa":null,"tentativas":1}]})
        .to_string(),
    );
    b.a.store.save(&t).unwrap();
    let r = fluxos::retomar(&b.a, &f, &t.id).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert!(passo(&r, "a").reaproveitado);
    assert_eq!(passo(&r, "b").saida, "um dois");
    assert_eq!(b.eco.chamadas.load(Ordering::SeqCst), 1, "so o b rodou");
    assert_eq!(r.formato, fluxos::FORMATO_RELATORIO);
}

/// O `task.json` guarda UMA copia do dado: `saida` some quando `itens` a reconstroi, fica
/// quando nao (texto JSON com espacos), e volta derivada na leitura -- quem le
/// `r.passos[].saida` (a CLI) ve o mesmo texto. `formato` e escrito (2); ausente vale 1; e
/// formato do futuro e recusado na retomada em vez de lido pela metade.
///
/// Reposta que derruba: em `From<Resultado> for ResultadoDisco`, `saida` sempre
/// `Some(r.saida)` (a chave volta ao disco no passo `obj`); a conferencia
/// `r.formato > FORMATO_LIDO_MAX` removida (o formato do futuro e retomado).
#[tokio::test]
async fn relatorio_grava_uma_copia_e_diz_o_formato() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"fmt","passos":[
        {"id":"obj","ferramenta":"eco","args":{"texto":{"k":1}}},
        {"id":"espaco","ferramenta":"eco","args":{"texto":"  {\"k\": 1}"}},
        {"id":"cai","ferramenta":"eco","args":{"falhar":"caiu"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    let mut t = b.a.store.load(&r.tarefa).unwrap();
    let cru: Value = serde_json::from_str(t.answer.as_deref().unwrap()).unwrap();
    assert_eq!(cru["formato"], json!(2));
    let no_disco = |id: &str| {
        cru["passos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };
    assert!(
        no_disco("obj").get("saida").is_none(),
        "{}",
        no_disco("obj")
    );
    assert_eq!(no_disco("espaco")["saida"], json!("  {\"k\": 1}"));
    let lido = relatorio(&t);
    assert_eq!(passo(&lido, "obj").saida, r#"{"k":1}"#);
    for p in &r.passos {
        assert_eq!(passo(&lido, &p.id), p, "ida e volta do disco");
    }
    // formato ausente = 1
    let velho: fluxos::Relatorio =
        serde_json::from_value(json!({"tarefa":"x","fluxo_sha256":"y","sucesso":true,"passos":[]}))
            .unwrap();
    assert_eq!(velho.formato, 1);
    // formato do futuro: recusado (o 3 virou o da onda 3; futuro e o seguinte ao lido)
    let mut futuro = cru.clone();
    futuro["formato"] = json!(fluxos::FORMATO_LIDO_MAX + 1);
    t.answer = Some(futuro.to_string());
    b.a.store.save(&t).unwrap();
    let e = fluxos::retomar(&b.a, &f, &t.id).await.unwrap_err();
    assert!(
        e.contains(&format!("formato {}", fluxos::FORMATO_LIDO_MAX + 1)),
        "{e}"
    );
}

/// O progresso que o disco recusa NAO e engolido: vai para a evidencia da tarefa (outro
/// arquivo, que ainda recebe) -- a retomada nao vai achar o que nao foi gravado, e alguem
/// precisa saber disso. O `task.json` vira diretorio no meio do fluxo, e a troca atomica
/// falha.
///
/// RED medido: `gravar_progresso` de volta a `let _ = agente.store.save(mae);` -- a
/// evidencia fica sem `fluxo.progresso` e o teste cai.
#[tokio::test]
async fn progresso_que_nao_grava_vai_para_a_evidencia() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"disco","passos":[
        {"id":"lento","ferramenta":"eco","args":{"dorme_ms":600,"texto":"x"}}
    ]}));
    let (a2, f2) = (b.a.clone(), f.clone());
    let h = tokio::spawn(async move { fluxos::rodar(&a2, &f2).await });
    let mut mae = None;
    for _ in 0..200 {
        if let Some(t) = execucoes(&b.a, "disco")
            .into_iter()
            .find(|t| t.status == TaskStatus::Running)
        {
            mae = Some(t);
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let mae = mae.expect("a tarefa do fluxo nasceu");
    let json_da_tarefa = b.a.store.dir(&mae.id).join("task.json");
    std::fs::remove_file(&json_da_tarefa).unwrap();
    std::fs::create_dir_all(json_da_tarefa.join("trava")).unwrap();
    let r = h.await.unwrap().unwrap();
    assert!(r.sucesso);
    let ev = std::fs::read_to_string(b.a.store.evidence_path(&mae.id)).unwrap();
    assert!(ev.contains("fluxo.progresso"), "{ev}");
}

// ------------------------------------------------------------------ seguranca A3

/// `por_item` tem teto (`max_itens`, padrao 1.000): acima dele o passo FALHA dizendo
/// quantos vieram, sem uma chamada sequer; `max_itens` 0 e recusado na leitura; e o
/// `teto_ms` do fluxo tem padrao (uma hora) quando ninguem escreve.
///
/// RED medido: o bloco `if itens.len() > p.max_itens` removido -- os 5 itens rodam, o
/// passo sai `ok` e o teste cai.
#[tokio::test]
async fn por_item_acima_do_teto_recusa() {
    let b = banca(&["fs.read"], None);
    let mil_e_um: Vec<usize> = (0..1001).collect();
    let f = fluxo(json!({"nome":"teto","passos":[
        {"id":"cinco","ferramenta":"eco","args":{"texto":[1,2,3,4,5]}},
        {"id":"tres","depende":["cinco"],"por_item":true,"max_itens":3,"ferramenta":"eco","args":{"texto":"{{cinco}}"}},
        {"id":"muitos","ferramenta":"eco","args":{"texto":mil_e_um}},
        {"id":"padrao","depende":["muitos"],"por_item":true,"ferramenta":"eco","args":{"texto":"{{muitos}}"}}
    ]}));
    assert_eq!(f.teto_ms, fluxos::TETO_MS_PADRAO);
    assert_eq!(fluxos::TETO_MS_PADRAO, 3_600_000);
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(!r.sucesso);
    let tres = passo(&r, "tres");
    assert_eq!(tres.estado, "falhou");
    assert!(
        tres.saida
            .contains("5 itens na entrada 'cinco' passam do teto de 3"),
        "{}",
        tres.saida
    );
    let padrao = passo(&r, "padrao");
    assert!(
        padrao.saida.contains("1001 itens") && padrao.saida.contains("teto de 1000"),
        "{}",
        padrao.saida
    );
    assert_eq!(
        b.eco.chamadas.load(Ordering::SeqCst),
        2,
        "so as duas listas: nenhuma passada"
    );
    let e = fluxos::ler(
        &json!({"nome":"z","passos":[{"id":"a","ferramenta":"eco","max_itens":0}]}).to_string(),
    )
    .unwrap_err();
    assert!(e.contains("max_itens comeca em 1"), "{e}");
}

/// As chamadas por item e os subagentes da MESMA onda dividem um semaforo de
/// `max_paralelo` vagas: 12 itens de ferramenta e 6 de subagente, juntos, nunca passam de
/// 3 em voo -- e chegam a mais de 1 (o teste nao passa por estar tudo em fila unica).
///
/// RED medido: o `let _vaga = vagas.acquire().await;` das chamadas removido -- o maximo
/// em voo sobe para a onda inteira e o teste cai.
#[tokio::test]
async fn chamadas_em_voo_nao_passam_de_max_paralelo() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"vagas","max_paralelo":3,"passos":[
        {"id":"lista","ferramenta":"eco","args":{"texto":[1,2,3,4,5,6,7,8,9,10,11,12]}},
        {"id":"seis","ferramenta":"eco","args":{"texto":[1,2,3,4,5,6]}},
        {"id":"cada","depende":["lista"],"por_item":true,"ferramenta":"eco",
         "args":{"dorme_ms":40,"texto":"{{lista}}"}},
        {"id":"agentes","depende":["seis"],"por_item":true,"tarefa":"[[lento:40]] [[eco:a{{seis}}]]"}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "cada").itens.len(), 12);
    assert_eq!(passo(&r, "agentes").itens.len(), 6);
    let max = b.voo.max.load(Ordering::SeqCst);
    assert!(max <= 3, "{max} em voo com max_paralelo 3");
    assert!(max >= 2, "{max}: nada rodou em paralelo");
}

// ------------------------------------------------------------------ lacunas da onda 1

/// `juntar` modo `ramo` NAO e `append`: com os dois ramos vivos, `ramo` fica so com o
/// primeiro (na ordem de `depende`) que tem itens; o ramo vazio e saltado. O teste da onda
/// 1 so tinha um ramo vivo, e ali os dois modos dao o mesmo.
///
/// Reposta que derruba: no `controle()`, o braco `"ramo"` trocado pelo do `"append"`.
#[tokio::test]
async fn juntar_ramo_nao_e_append() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"ramo","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":[1]}},
        {"id":"b","ferramenta":"eco","args":{"texto":[2]}},
        {"id":"vazio","ferramenta":"eco","args":{"texto":[]}},
        {"id":"ramo","depende":["a","b"],"juntar":{"modo":"ramo"}},
        {"id":"todos","depende":["a","b"],"juntar":{"modo":"append"}},
        {"id":"salta","depende":["vazio","b"],"juntar":{"modo":"ramo"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "ramo").itens, vec![json!(1)]);
    assert_eq!(passo(&r, "todos").itens, vec![json!(1), json!(2)]);
    assert_eq!(passo(&r, "salta").itens, vec![json!(2)]);
}

/// `prazo()` com os DOIS tetos: vale o menor, e a mensagem diz qual. Teto do passo folgado
/// e do fluxo curto -> «teto do fluxo»; o contrario -> «teto do passo».
///
/// Reposta que derruba: no `prazo()`, `a.0 <= b.0` trocado por `a.0 >= b.0` (escolhe o
/// maior: o primeiro caso termina `ok` e o segundo diz «teto do fluxo»).
#[tokio::test]
async fn prazo_com_teto_de_passo_e_de_fluxo() {
    let b = banca(&["fs.read"], None);
    let f = fluxo(json!({"nome":"p1","teto_ms":150,"passos":[
        {"id":"x","ferramenta":"eco","teto_ms":5000,"args":{"dorme_ms":600,"texto":"x"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert_eq!(passo(&r, "x").saida, "teto do fluxo (150 ms) estourou");
    let f = fluxo(json!({"nome":"p2","teto_ms":10000,"passos":[
        {"id":"x","ferramenta":"eco","teto_ms":50,"args":{"dorme_ms":600,"texto":"x"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert_eq!(passo(&r, "x").saida, "teto do passo (50 ms) estourou");
}

/// `{{erro}}` do fluxo de erro separa quem FALHOU (com o motivo) de quem ficou
/// `bloqueado` (sem motivo: e efeito de quem falhou). Antes, o bloqueado sumia de
/// `{{erro}}` sem que nenhum teste dissesse se era de proposito.
///
/// Reposta que derruba: o filtro de `falhos` ampliado para `estado == "falhou" || estado
/// == "bloqueado"` (o `passos` ganha o `depois` sem motivo); `bloqueados` removido da
/// visao (a expressao `{{erro.bloqueados}}` falha).
#[tokio::test]
async fn fluxo_de_erro_separa_falhos_de_bloqueados() {
    let b = banca(&["fs.read", "fs.write"], None);
    let f = fluxo(json!({"nome":"fe","fluxo_de_erro":"avisa","passos":[
        {"id":"quebra","ferramenta":"eco","args":{"falhar":"caiu"}},
        {"id":"depois","depende":["quebra"],"ferramenta":"eco","args":{"texto":"x"}},
        {"id":"bem","ferramenta":"eco","args":{"texto":"x"}},
        {"id":"avisa","ferramenta":"write_file","args":{"path":"erro.txt","content":"erro: {{erro}}"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(!r.sucesso);
    assert_eq!(passo(&r, "depois").estado, "bloqueado");
    let t = std::fs::read_to_string(b.a.store.workdir(&r.tarefa).join("erro.txt")).unwrap();
    let v: Value = serde_json::from_str(t.strip_prefix("erro: ").unwrap()).unwrap();
    let passos = v["passos"].as_array().unwrap();
    assert_eq!(passos.len(), 1, "{v}");
    assert_eq!(passos[0]["passo"], json!("quebra"));
    assert!(passos[0]["motivo"].as_str().unwrap().contains("caiu"));
    assert_eq!(v["bloqueados"], json!(["depois"]));
}

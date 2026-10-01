//! Medicao (SP000011): gravar e repetir, avaliacao de modelos, energia e otimizacao de
//! skill. Tudo deterministico, com o `ScriptedLlm`; a prova com o Ollama de verdade roda
//! pela CLI e esta no relatorio da sprint.

use phxclaw_agent::api::{AgentFactory, ApiState, Limite, criar_tarefa, router};
use phxclaw_agent::avaliacao::{
    Caso, Energia, Gabarito, LeitorEnergia, Medida, avaliar, ler_casos,
};
use phxclaw_agent::gravacao::{Gravacao, Gravador, ModoFerramentas, Repetidor, gravando};
use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::otimizacao::{PedidoOtimizacao, ab, com_pasta_de_skills, gerar_variante};
use phxclaw_agent::*;
use phxclaw_agent_core::tarefa::NovaTarefa;
use phxclaw_agent_core::{LlmReply, Tool, Usage};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-medicao-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fim(resposta: &str) -> LlmReply {
    ScriptedLlm::call("f", "final_answer", json!({"answer": resposta}))
}

/// Tres ferramentas, um segredo nos argumentos e outro na saida.
fn roteiro() -> Vec<LlmReply> {
    vec![
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "notas.md", "content": "token=segredo123abc e mais nada"}),
        ),
        ScriptedLlm::call("c2", "read_file", json!({"path": "notas.md"})),
        ScriptedLlm::call(
            "c3",
            "calculator",
            json!({"expression": "6*7", "password": "hunter2"}),
        ),
        fim("42"),
    ]
}

const OBJETIVO: &str = "anote o recado e calcule seis vezes sete";

async fn gravar(raiz: &Path) -> (PathBuf, Task) {
    let m = Montagem::new(TaskStore::new(raiz.join("tasks")).unwrap());
    let a = m.agent_with(Arc::new(ScriptedLlm::new(roteiro())));
    let arq = raiz.join("g.jsonl");
    let g = Gravador::criar(&arq, OBJETIVO, "roteiro").unwrap();
    let t = gravando(a, &g)
        .run(
            Task::new(OBJETIVO, "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(
        g.resultado().unwrap(),
        8,
        "cabecalho + 4 modelo + 3 ferramentas"
    );
    (arq, t)
}

// ------------------------------------------------------------------ gravar_repetir

#[tokio::test]
async fn gravar_e_repetir_sem_modelo_da_a_mesma_sequencia() {
    let raiz = tmp("grava");
    let (arq, t) = gravar(&raiz).await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let texto = std::fs::read_to_string(&arq).unwrap();
    for segredo in ["segredo123abc", "hunter2"] {
        assert!(!texto.contains(segredo), "{segredo} vazou na gravacao");
    }
    let g = Gravacao::ler(&arq).unwrap();
    assert_eq!(g.sequencia(), ["write_file", "read_file", "calculator"]);
    assert_eq!(g.modelo_respostas.len(), 4);

    // As duas repeticoes, sem modelo nenhum: o roteiro do agente repetidor esta VAZIO.
    for modo in [ModoFerramentas::Reais, ModoFerramentas::Gravadas] {
        let m = Montagem::new(TaskStore::new(tmp("repete").join("tasks")).unwrap());
        let r = Repetidor::novo(Gravacao::ler(&arq).unwrap(), modo);
        let a = r.agente(m.agent_with(Arc::new(ScriptedLlm::new(vec![]))));
        let t2 = a
            .run(
                Task::new(OBJETIVO, "repeticao"),
                &CancelFlag::default(),
                &NoObserver,
            )
            .await;
        let rel = r.relatorio();
        assert!(rel.igual(), "{modo:?}: {:?}", rel.divergencias);
        assert_eq!(rel.repetida, rel.gravada, "{modo:?}");
        assert_eq!(rel.respostas_usadas, 4);
        assert_eq!(t2.status, TaskStatus::Completed, "{modo:?}: {:?}", t2.error);
        assert_eq!(t2.answer.as_deref(), Some("42"));
    }
}

#[tokio::test]
async fn divergencia_e_acusada_com_o_passo() {
    let raiz = tmp("diverge");
    let (arq, _) = gravar(&raiz).await;

    // (1) O agente mudou: sem `calc`, o portao nega a calculadora e a repeticao para antes.
    let mut m = Montagem::new(TaskStore::new(raiz.join("t2")).unwrap());
    m.capabilities.retain(|c| c != "calc");
    let r = Repetidor::novo(Gravacao::ler(&arq).unwrap(), ModoFerramentas::Reais);
    r.agente(m.agent_with(Arc::new(ScriptedLlm::new(vec![]))))
        .run(
            Task::new(OBJETIVO, "r"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    let rel = r.relatorio();
    assert!(!rel.igual());
    assert_eq!(rel.repetida, ["write_file", "read_file"]);
    // A calculadora gravada e o passo 6 (cabecalho 0, modelo 1, write 2, modelo 3, read 4,
    // modelo 5, calculadora 6).
    assert!(
        rel.divergencias[0].starts_with("passo 6: a gravacao chamou calculator"),
        "{:?}",
        rel.divergencias
    );

    // (2) A gravacao diz um argumento e o modelo gravado pede outro: acusado no passo dele.
    let texto = std::fs::read_to_string(&arq).unwrap();
    let mexida: Vec<String> = texto
        .lines()
        .map(|l| {
            if l.contains("\"tipo\":\"ferramenta\"") && l.contains("read_file") {
                l.replace("notas.md", "outro.md")
            } else {
                l.to_string()
            }
        })
        .collect();
    let arq2 = raiz.join("mexida.jsonl");
    std::fs::write(&arq2, mexida.join("\n")).unwrap();
    let m = Montagem::new(TaskStore::new(raiz.join("t3")).unwrap());
    let r = Repetidor::novo(Gravacao::ler(&arq2).unwrap(), ModoFerramentas::Gravadas);
    r.agente(m.agent_with(Arc::new(ScriptedLlm::new(vec![]))))
        .run(
            Task::new(OBJETIVO, "r"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    let rel = r.relatorio();
    assert!(
        rel.divergencias[0].starts_with("passo 4: gravado read_file"),
        "{:?}",
        rel.divergencias
    );
}

#[tokio::test]
async fn api_grava_a_tarefa_inclusive_a_que_espera_o_plano() {
    let dir = tmp("api");
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    // So o PRIMEIRO agente "com-plano" (o do plano) recebe a resposta do plano; o da
    // aprovacao nasce de novo pela fabrica e comeca direto no trabalho.
    let planos = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let factory: AgentFactory = Arc::new(move |modelo: &str| {
        let mut r = vec![];
        if modelo == "com-plano" && planos.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            r.push(ScriptedLlm::text("{\"steps\": [\"escrever\"]}"));
        }
        r.push(ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "r.md", "content": "Authorization: Bearer abcdefSEGREDO"}),
        ));
        r.push(ScriptedLlm::text("pronto"));
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool)];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(r)),
            tools,
            AgentConfig::default().grant(&["fs.write"]),
            st2.clone(),
        ))
    });
    let s = ApiState {
        store: store.clone(),
        factory,
        default_model: "padrao".into(),
        token: "token-de-teste-com-tamanho-suficiente".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let c = criar_tarefa(
        &s,
        NovaTarefa {
            objective: "escreva".into(),
            gravar: true,
            ..Default::default()
        },
    )
    .unwrap();
    c.fim.await.unwrap();
    let arq = store.dir(&c.id).join("gravacao.jsonl");
    assert_eq!(Gravacao::ler(&arq).unwrap().sequencia(), ["write_file"]);
    assert!(
        !std::fs::read_to_string(&arq)
            .unwrap()
            .contains("abcdefSEGREDO")
    );

    // Sem `gravar`, nada se grava.
    let c = criar_tarefa(
        &s,
        NovaTarefa {
            objective: "escreva".into(),
            ..Default::default()
        },
    )
    .unwrap();
    c.fim.await.unwrap();
    assert!(!store.dir(&c.id).join("gravacao.jsonl").exists());

    // Com plano: o plano nao se grava; a execucao, depois do sim, sim.
    let c = criar_tarefa(
        &s,
        NovaTarefa {
            objective: "escreva".into(),
            model: Some("com-plano".into()),
            plan_first: true,
            gravar: true,
            ..Default::default()
        },
    )
    .unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    let arq = store.dir(&id).join("gravacao.jsonl");
    assert!(Gravacao::ler(&arq).unwrap().ferramentas.is_empty());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(s.clone());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/tasks/{id}/approve"))
        .bearer_auth("token-de-teste-com-tamanho-suficiente")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    for _ in 0..200 {
        if store.load(&id).unwrap().status == TaskStatus::Completed {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(store.load(&id).unwrap().status, TaskStatus::Completed);
    let g = Gravacao::ler(&arq).unwrap();
    assert_eq!(
        g.sequencia(),
        ["write_file"],
        "a execucao aprovada nao gravou"
    );
}

// ------------------------------------------------------------------ avaliacao_modelos

fn resposta_medida(r: LlmReply, ns: u64) -> LlmReply {
    LlmReply {
        usage: Usage {
            input_tokens: 10,
            output_tokens: 20,
            duracao_geracao_ns: Some(ns),
        },
        ..r
    }
}

fn caso_42() -> Caso {
    Caso {
        id: "multiplica".into(),
        objetivo: "quanto e seis vezes sete?".into(),
        gabarito: Gabarito {
            contem: vec!["42".into()],
            sequencia: None,
        },
    }
}

#[tokio::test]
async fn avaliar_mede_com_faixa_e_so_declara_vencedor_sem_cruzar() {
    let raiz = tmp("avalia");
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let m = Montagem::new(store);
    // "bom" acerta e gera a 100 tokens/s (20 tokens em 0,2 s); "ruim" erra.
    let fabrica = move |modelo: &str| -> Result<Agent, String> {
        // "quebrado" e o modelo que recusa o primeiro pedido (como o smollm2 sem tools).
        if modelo == "quebrado" {
            return Ok(m.agent_with(Arc::new(ScriptedLlm::new(vec![]))));
        }
        let resposta = if modelo == "bom" { "42" } else { "41" };
        let r = vec![
            resposta_medida(
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*7"})),
                200_000_000,
            ),
            resposta_medida(fim(resposta), 200_000_000),
        ];
        Ok(m.agent_with(Arc::new(ScriptedLlm::new(r))))
    };
    let sem_rapl = LeitorEnergia::de_raiz(&tmp("sys-vazio"), None);
    let a = avaliar(
        &fabrica,
        &["bom".into(), "ruim".into(), "quebrado".into()],
        &[caso_42()],
        3,
        &raiz.join("saida"),
        &sem_rapl,
    )
    .await
    .unwrap();
    let bom = &a.modelos[0];
    assert_eq!(bom.execucoes, 3);
    assert_eq!(bom.acerto_por_rodada.min, 1.0);
    assert_eq!(bom.acerto_bp, 10_000);
    assert_eq!(a.modelos[1].acerto_por_rodada.max, 0.0);
    match &bom.tokens_por_s {
        Medida::Faixa(f) => assert_eq!((f.min, f.max, f.n), (100.0, 100.0, 3)),
        outro => panic!("{outro:?}"),
    }
    match &bom.energia_j {
        Medida::NaoMedida(m) => assert_eq!(m, "sem RAPL"),
        outro => panic!("energia estimada: {outro:?}"),
    }
    assert!(bom.latencia_ms.min <= bom.p50_ms && bom.p50_ms <= bom.p95_ms as f64 + 1.0);
    let q = &a.modelos[2];
    assert_eq!(q.falhas, 3);
    match &q.tokens_por_s {
        Medida::NaoMedida(m) => assert_eq!(m, "todas as execuções falharam"),
        outro => panic!("{outro:?}"),
    }
    let v: HashMap<_, _> = a
        .vencedores
        .iter()
        .map(|v| (v.metrica.as_str(), (v.modelo.clone(), v.motivo.clone())))
        .collect();
    assert_eq!(v["acerto_por_rodada"].0.as_deref(), Some("bom"));
    // tokens/s iguais nos dois: faixas encostam, nao ha vencedor.
    assert_eq!(v["tokens_por_s"].0, None);
    // Quem falhou em tudo nao concorre em desempenho: latencia de quem morre no primeiro
    // pedido nao e rapidez (medido com o smollm2, que recusa ferramentas em 5 ms).
    assert!(
        v["latencia_ms"]
            .1
            .contains("1 modelo(s) com falha fora da comparação"),
        "{:?}",
        v["latencia_ms"]
    );
    assert_ne!(v["latencia_ms"].0.as_deref(), Some("quebrado"));
    let tabela = phxclaw_agent::avaliacao::tabela(&a);
    assert!(
        tabela.contains("energia J   : não medida (sem RAPL)"),
        "{tabela}"
    );
    // Cada execucao ficou gravada e repetivel.
    assert_eq!(
        Gravacao::ler(&a.execucoes[0].gravacao).unwrap().sequencia(),
        ["calculator"]
    );
}

#[test]
fn casos_saem_de_json_com_gabarito_e_de_gravacao() {
    let d = tmp("casos");
    std::fs::write(
        d.join("a.json"),
        r#"{"id":"","objetivo":"x","gabarito":{"contem":["y"]}}"#,
    )
    .unwrap();
    std::fs::write(d.join("b.json"), r#"{"id":"b","objetivo":"x"}"#).unwrap();
    assert!(ler_casos(&d).unwrap_err().contains("sem gabarito"));
    std::fs::remove_file(d.join("b.json")).unwrap();
    let c = ler_casos(&d).unwrap();
    assert_eq!(c[0].id, "a");
}

// ------------------------------------------------------------------ telemetria_energia

fn zona(sys: &Path, nome: &str, uj: u64, volta: Option<u64>) {
    let z = sys.join("class/powercap").join(nome);
    std::fs::create_dir_all(&z).unwrap();
    std::fs::write(z.join("energy_uj"), format!("{uj}\n")).unwrap();
    std::fs::write(z.join("name"), "package-0\n").unwrap();
    if let Some(v) = volta {
        std::fs::write(z.join("max_energy_range_uj"), format!("{v}\n")).unwrap();
    }
}

#[test]
fn rapl_falso_da_a_conta_certa_e_sem_ele_nao_mede() {
    let sys = tmp("sys");
    zona(&sys, "intel-rapl:0", 1_000_000, Some(10_000_000));
    zona(&sys, "intel-rapl:1", 9_500_000, Some(10_000_000));
    // Subzona e mmio repetem energia do pacote: se entrassem, a conta dobraria.
    zona(&sys, "intel-rapl:0:0", 0, None);
    zona(&sys, "intel-rapl-mmio:0", 0, None);
    let l = LeitorEnergia::de_raiz(&sys, None);
    let antes = l.amostra();
    zona(&sys, "intel-rapl:0", 3_500_000, Some(10_000_000));
    // O pacote 1 deu a volta: 9,5 J -> 10 J (teto) -> 0,5 J = 1 J gasto.
    zona(&sys, "intel-rapl:1", 500_000, Some(10_000_000));
    zona(&sys, "intel-rapl:0:0", 99_000_000, None);
    zona(&sys, "intel-rapl-mmio:0", 99_000_000, None);
    let e = l.entre(&antes, &l.amostra());
    match e {
        Energia::Medida { joules, fonte } => {
            assert!((joules - 3.5).abs() < 1e-9, "{joules}");
            assert_eq!(fonte, "RAPL, 2 zona(s)");
        }
        outro => panic!("{outro:?}"),
    }

    let vazio = LeitorEnergia::de_raiz(&tmp("sys-sem"), None);
    let e = vazio.entre(&vazio.amostra(), &vazio.amostra());
    assert_eq!(e.to_string(), "não medida (sem RAPL)");
}

// ------------------------------------------------------------------ otimizacao_skills

const ORIGINAL: &str = "---\nname: conta\ndescription: Faz contas.\n---\nUse a calculadora.\n";

fn caso_skill() -> Caso {
    Caso {
        id: "conta".into(),
        objetivo: "quanto e seis vezes sete?".into(),
        gabarito: Gabarito {
            contem: vec!["42".into()],
            sequencia: Some(vec!["skill_load".into(), "calculator".into()]),
        },
    }
}

/// O "modelo" do braco le a skill que ganhou: com a frase da variante boa ele carrega a
/// skill, usa a calculadora e acerta; com qualquer outra, pula a skill e erra.
fn fabrica_de_braco(
    raiz: &Path,
    acerta_com: &'static str,
) -> impl Fn(&str, &Path) -> Result<Agent, String> + Sync {
    let m = Montagem::new(TaskStore::new(raiz.join("tasks")).unwrap());
    move |_modelo: &str, pasta: &Path| {
        let texto = std::fs::read_to_string(pasta.join("conta/SKILL.md")).unwrap();
        let r = if texto.contains(acerta_com) {
            vec![
                ScriptedLlm::call("s", "skill_load", json!({"name": "conta"})),
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*7"})),
                fim("42"),
            ]
        } else {
            vec![
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*6"})),
                fim("36"),
            ]
        };
        com_pasta_de_skills(m.agent_with(Arc::new(ScriptedLlm::new(r))), pasta)
    }
}

async fn otimizar(
    raiz: &Path,
    variante_corpo: &str,
    acerta_com: &'static str,
) -> (phxclaw_agent::otimizacao::Decisao, PathBuf) {
    let skills = raiz.join("skills");
    std::fs::create_dir_all(skills.join("conta")).unwrap();
    std::fs::write(skills.join("conta/SKILL.md"), ORIGINAL).unwrap();
    let gerador = ScriptedLlm::new(vec![ScriptedLlm::text(&format!(
        "```markdown\n---\nname: conta\ndescription: Faz contas.\n---\n{variante_corpo}\n```"
    ))]);
    let variante = gerar_variante(&gerador, "conta", ORIGINAL).await.unwrap();
    let fab = fabrica_de_braco(raiz, acerta_com);
    let casos = [caso_skill()];
    let p = PedidoOtimizacao {
        skill: "conta",
        pasta_skills: &skills,
        modelo: "roteiro",
        casos: &casos,
        rodadas: 3,
        trabalho: &raiz.join("ab"),
    };
    (ab(&fab, &p, &variante).await.unwrap(), skills)
}

#[tokio::test]
async fn variante_melhor_sem_cruzar_faixa_e_promovida() {
    let raiz = tmp("skill-boa");
    let (d, skills) = otimizar(
        &raiz,
        "PASSO 1: skill_load. PASSO 2: calculadora.",
        "PASSO 1",
    )
    .await;
    assert!(d.promovida, "{}", d.motivo);
    assert_eq!((d.acerto_original.max, d.acerto_variante.min), (0.0, 1.0));
    let atual = std::fs::read_to_string(skills.join("conta/SKILL.md")).unwrap();
    assert!(atual.contains("PASSO 1"));
    let guardada = std::fs::read_to_string(d.original_guardada.unwrap()).unwrap();
    assert_eq!(guardada, ORIGINAL);
    let reg = std::fs::read_to_string(skills.join("conta/otimizacao.jsonl")).unwrap();
    assert!(reg.contains("\"promovida\":true"), "{reg}");
}

#[tokio::test]
async fn variante_pior_ou_empatada_nao_e_promovida_e_fica_registrada() {
    // Pior: quem acerta e a ORIGINAL.
    let raiz = tmp("skill-pior");
    let (d, skills) = otimizar(&raiz, "Responda de cabeca.", "Use a calculadora").await;
    assert!(!d.promovida);
    assert!(d.motivo.starts_with("variante PIOR"), "{}", d.motivo);
    assert_eq!(
        std::fs::read_to_string(skills.join("conta/SKILL.md")).unwrap(),
        ORIGINAL
    );
    let reg = std::fs::read_to_string(skills.join("conta/otimizacao.jsonl")).unwrap();
    assert!(
        reg.contains("\"promovida\":false") && reg.contains("acerto_variante"),
        "{reg}"
    );

    // Empate: as duas acertam sempre; faixas iguais se cruzam.
    let raiz = tmp("skill-empate");
    let (d, skills) = otimizar(&raiz, "Use a calculadora, sempre.", "calculadora").await;
    assert!(!d.promovida);
    assert!(d.motivo.contains("empate nao promove"), "{}", d.motivo);
    assert_eq!(
        std::fs::read_to_string(skills.join("conta/SKILL.md")).unwrap(),
        ORIGINAL
    );
}

#[tokio::test]
async fn variante_invalida_nem_vai_ao_ab_e_fica_no_registro() {
    // Medido com o qwen2.5:1.5b: a variante voltou sem fechar o cabecalho.
    let gerador = ScriptedLlm::new(vec![ScriptedLlm::text(
        "---\nname: conta\ndescription: Faz contas.\nUse a calculadora.",
    )]);
    let e = gerar_variante(&gerador, "conta", ORIGINAL)
        .await
        .unwrap_err();
    assert!(
        e.contains("unterminated") && e.contains("o modelo devolveu"),
        "{e}"
    );
    let skills = tmp("skill-invalida");
    std::fs::create_dir_all(skills.join("conta")).unwrap();
    phxclaw_agent::otimizacao::registrar_recusa(&skills, "conta", "roteiro", &e).unwrap();
    let reg = std::fs::read_to_string(skills.join("conta/otimizacao.jsonl")).unwrap();
    assert!(
        reg.contains("\"promovida\":false") && reg.contains("\"medida\":false"),
        "{reg}"
    );
}

// ------------------------------------------------------------------ prova F (01/10/2026)

/// A faixa e o juiz tambem NA CHAMADA do `avaliar`, nao so na funcao `faixas_decidem`: as
/// medianas diferem (200 contra 250 tokens/s) e as faixas se cruzam ([100-250] e
/// [125-400]). Pela faixa, nao ha vencedor; pela mediana, «b» venceria. O teste antigo usava
/// tokens/s IGUAIS nos dois, e ai mediana e faixa dao a mesma resposta -- o defeito de
/// decidir pela mediana passava.
#[tokio::test]
async fn avaliar_nao_declara_vencedor_com_mediana_melhor_e_faixas_cruzadas() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let raiz = tmp("avalia-cruza");
    let m = Montagem::new(TaskStore::new(raiz.join("tasks")).unwrap());
    // ns por resposta: 2 respostas de 20 tokens -> tokens/s = 20e9 / ns.
    let ns_a = [200_000_000u64, 100_000_000, 80_000_000]; // 100, 200, 250
    let ns_b = [160_000_000u64, 80_000_000, 50_000_000]; // 125, 250, 400
    let (ka, kb) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let fabrica = |modelo: &str| -> Result<Agent, String> {
        let ns = if modelo == "a" {
            ns_a[ka.fetch_add(1, Ordering::SeqCst)]
        } else {
            ns_b[kb.fetch_add(1, Ordering::SeqCst)]
        };
        let r = vec![
            resposta_medida(
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*7"})),
                ns,
            ),
            resposta_medida(fim("42"), ns),
        ];
        Ok(m.agent_with(Arc::new(ScriptedLlm::new(r))))
    };
    let a = avaliar(
        &fabrica,
        &["a".into(), "b".into()],
        &[caso_42()],
        3,
        &raiz.join("saida"),
        &LeitorEnergia::de_raiz(&tmp("sys-vazio-2"), None),
    )
    .await
    .unwrap();
    let faixa = |i: usize| match &a.modelos[i].tokens_por_s {
        Medida::Faixa(f) => (f.min.round(), f.mediana.round(), f.max.round()),
        outro => panic!("{outro:?}"),
    };
    // A montagem e a que o teste diz: sem isto, um erro aqui viraria «passou por engano».
    assert_eq!(faixa(0), (100.0, 200.0, 250.0));
    assert_eq!(faixa(1), (125.0, 250.0, 400.0));
    let v = a
        .vencedores
        .iter()
        .find(|v| v.metrica == "tokens_por_s")
        .unwrap();
    assert_eq!(
        v.modelo, None,
        "mediana melhor dentro do ruido virou vitoria: {v:?}"
    );
    assert!(v.motivo.contains("faixas se cruzam"), "{v:?}");
}

/// O mesmo na otimizacao de skill: a variante acerta em 2 de 3 rodadas (faixa [0-1],
/// mediana 1) e a original nunca (faixa [0-0]). Mediana e media dizem «melhor»; as faixas
/// encostam no 0, e encostar e cruzar -- nao promove, e a skill fica como estava.
#[tokio::test]
async fn variante_que_acerta_as_vezes_encosta_na_faixa_e_nao_e_promovida() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let raiz = tmp("skill-as-vezes");
    let skills = raiz.join("skills");
    std::fs::create_dir_all(skills.join("conta")).unwrap();
    std::fs::write(skills.join("conta/SKILL.md"), ORIGINAL).unwrap();
    let gerador = ScriptedLlm::new(vec![ScriptedLlm::text(
        "```markdown\n---\nname: conta\ndescription: Faz contas.\n---\nPASSO 1: skill_load.\n```",
    )]);
    let variante = gerar_variante(&gerador, "conta", ORIGINAL).await.unwrap();
    let m = Montagem::new(TaskStore::new(raiz.join("tasks")).unwrap());
    let k = AtomicUsize::new(0);
    let fab = |_modelo: &str, pasta: &Path| {
        let texto = std::fs::read_to_string(pasta.join("conta/SKILL.md")).unwrap();
        // Variante: acerta nas execucoes 0 e 2, erra na 1.
        let acerta =
            texto.contains("PASSO 1") && k.fetch_add(1, Ordering::SeqCst).is_multiple_of(2);
        let r = if acerta {
            vec![
                ScriptedLlm::call("s", "skill_load", json!({"name": "conta"})),
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*7"})),
                fim("42"),
            ]
        } else {
            vec![
                ScriptedLlm::call("c", "calculator", json!({"expression": "6*6"})),
                fim("36"),
            ]
        };
        com_pasta_de_skills(m.agent_with(Arc::new(ScriptedLlm::new(r))), pasta)
    };
    let casos = [caso_skill()];
    let p = PedidoOtimizacao {
        skill: "conta",
        pasta_skills: &skills,
        modelo: "roteiro",
        casos: &casos,
        rodadas: 3,
        trabalho: &raiz.join("ab"),
    };
    let d = ab(&fab, &p, &variante).await.unwrap();
    // A montagem: a variante acertou as vezes e a original nunca.
    assert_eq!(
        (d.acerto_original.min, d.acerto_original.max),
        (0.0, 0.0),
        "{d:?}"
    );
    assert_eq!(
        (d.acerto_variante.min, d.acerto_variante.max),
        (0.0, 1.0),
        "{d:?}"
    );
    assert!(d.acerto_variante.mediana > d.acerto_original.mediana);
    assert!(!d.promovida, "promovida pela mediana: {}", d.motivo);
    assert!(d.motivo.contains("empate nao promove"), "{}", d.motivo);
    assert_eq!(
        std::fs::read_to_string(skills.join("conta/SKILL.md")).unwrap(),
        ORIGINAL
    );
}

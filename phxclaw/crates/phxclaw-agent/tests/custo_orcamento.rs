//! Custo em dinheiro (R2), orcamento por tarefa e por fluxo (R3) e a bateria comum entre
//! provedores (R5), pelo motor de verdade, com provedores FALSOS locais: nenhum teste aqui
//! chama rede. Rodar contra provedor real e NAO MEDIDO nesta maquina (sem chave, sem Ollama).
//!
//! Prova real nos dois sentidos (09/10/2026): cada guarda foi reposta com `// REPOSTO` no
//! codigo, o teste dela caiu, e a guarda voltou escrita de novo -- o RED de cada uma esta no
//! comentario do proprio teste.

use phxclaw_agent::avaliacao::{Caso, Gabarito, LeitorEnergia, Medida};
use phxclaw_agent::bateria::{self, Bateria};
use phxclaw_agent::custo::TabelaDePrecos;
use phxclaw_agent::orcamento::Politica;
use phxclaw_agent::*;
use phxclaw_agent_core::tarefa::Orcamento;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, Tool, ToolSpec, Usage,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-custo-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Precos de mentira, com data e fonte: 1 por milhao na entrada, 2 na saida.
fn tabela() -> Arc<TabelaDePrecos> {
    Arc::new(
        TabelaDePrecos::de_texto(
            r#"{"moeda":"USD","modelos":{
            "falso:barato":{"entrada":1000000,"saida":2000000,"data":"2026-10-09","fonte":"prova local"},
            "falso:certo":{"entrada":1000,"saida":1000,"data":"2026-10-09","fonte":"prova local"},
            "falso:errado":{"entrada":1000,"saida":1000,"data":"2026-10-09","fonte":"prova local"},
            "falso:quase":{"entrada":1000,"saida":1000,"data":"2026-10-09","fonte":"prova local"},
            "falso:metade":{"entrada":1000,"saida":1000,"data":"2026-10-09","fonte":"prova local"},
            "falso:fora":{"entrada":9000000,"saida":9000000,"data":"2026-10-09","fonte":"prova local"}}}"#,
        )
        .unwrap(),
    )
}

/// Provedor falso com nome `provedor:modelo` (o `ScriptedLlm` se chama «roteiro», que nao
/// e nome de tabela) e uso fixo por resposta.
struct Falso {
    id: String,
    roteiro: ScriptedLlm,
}

impl Llm for Falso {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn chat<'a>(
        &'a self,
        m: &'a [Message],
        t: &'a [ToolSpec],
        o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        self.roteiro.chat(m, t, o)
    }
}

fn agente(id: &str, roteiro: Vec<LlmReply>, config: AgentConfig) -> Agent {
    Agent::new(
        Arc::new(Falso {
            id: id.into(),
            roteiro: ScriptedLlm::new(roteiro),
        }),
        vec![Arc::new(WriteFileTool) as Arc<dyn Tool>],
        config.grant(&["fs.write"]),
        TaskStore::new(tmp("store")).unwrap(),
    )
}

fn com_precos(politica: Politica) -> AgentConfig {
    AgentConfig {
        precos: Some(tabela()),
        orcamento: politica,
        ..AgentConfig::default()
    }
}

async fn rodar(a: &Agent, mut t: Task, o: Option<Orcamento>) -> Task {
    t.orcamento = o;
    a.run(t, &CancelFlag::default(), &NoObserver).await
}

/// R2 pelo motor: cada chamada com o preco da tabela, na tarefa, com a cotacao, e uma
/// linha de evidencia por chamada. Modelo fora da tabela: «nao medido», nunca 0.
///
/// RED medido: em `custo::registrar`, o `(Some(t), Some(x)) => Some(t + x), _ => None`
/// trocado por `Some(c.total.unwrap_or(0.0) + ch.custo.unwrap_or(0.0))` (`// REPOSTO`) --
/// o modelo sem preco saiu com total 0.000000 e a asserção do `None` caiu.
#[tokio::test]
async fn custo_por_chamada_e_por_tarefa_e_sem_preco_e_nao_medido() {
    // Cada resposta do roteiro: 10 de entrada, 5 de saida.
    let a = agente(
        "falso:barato",
        vec![ScriptedLlm::text("pronto")],
        com_precos(Politica::default()),
    );
    let t = rodar(&a, Task::new("x", "falso:barato"), None).await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let c = t.custo.clone().unwrap();
    // 10 * 1e6/1e6 + 5 * 2e6/1e6 = 20
    assert_eq!((c.total, c.moeda.as_deref()), (Some(20.0), Some("USD")));
    assert_eq!(c.chamadas.len(), 1);
    assert_eq!(c.chamadas[0].cotacao.as_ref().unwrap().data, "2026-10-09");
    assert!(
        custo::linha(&t).contains("20.000000 USD"),
        "{}",
        custo::linha(&t)
    );
    let ev = phxclaw_evidence_ledger::EvidenceLedger::open(a.store.evidence_path(&t.id))
        .unwrap()
        .tail(5)
        .unwrap();
    assert!(ev.iter().any(|r| r.action == "custo.chamada"), "{ev:?}");

    // O mesmo roteiro num modelo que a tabela nao conhece.
    let b = agente(
        "falso:sem-preco",
        vec![ScriptedLlm::text("pronto")],
        com_precos(Politica::default()),
    );
    let t = rodar(&b, Task::new("x", "falso:sem-preco"), None).await;
    assert_eq!(t.status, TaskStatus::Completed);
    assert_eq!(t.custo.as_ref().unwrap().total, None, "{:?}", t.custo);
    assert_eq!(custo::total(&t), None);
    assert!(
        custo::linha(&t).contains("não medido"),
        "{}",
        custo::linha(&t)
    );
}

/// R3: o teto em DINHEIRO para a tarefa, e a mensagem diz quanto gastou e qual teto bateu.
/// A ferramenta pedida na resposta que estourou NAO roda.
///
/// RED medido: o `if let Err(e) = self.cobrar(...)` do motor trocado por `let _ =
/// self.cobrar(...)` (`// REPOSTO`) -- a tarefa seguiu, o `write_file` gravou e ela
/// terminou `Completed`; o teste caiu no estado.
#[tokio::test]
async fn teto_em_dinheiro_para_a_tarefa_e_diz_o_gasto() {
    let a = agente(
        "falso:barato",
        vec![
            ScriptedLlm::call("c1", "write_file", json!({"path": "a.txt", "content": "1"})),
            ScriptedLlm::call("c2", "write_file", json!({"path": "b.txt", "content": "2"})),
            ScriptedLlm::text("pronto"),
        ],
        com_precos(Politica::default()),
    );
    // 20 por chamada: a segunda bate o teto de 30.
    let o = Orcamento {
        tokens: None,
        custo: Some(30.0),
    };
    let t = rodar(&a, Task::new("x", "falso:barato"), Some(o)).await;
    assert_eq!(t.status, TaskStatus::BudgetExceeded, "{:?}", t.error);
    let e = t.error.clone().unwrap();
    assert!(
        e.contains("gastou 40.000000 USD") && e.contains("teto de 30.000000 USD"),
        "{e}"
    );
    let w = a.store.workdir(&t.id);
    assert!(w.join("a.txt").exists() && !w.join("b.txt").exists());
    assert_eq!(t.gasto.as_ref().unwrap().custo, Some(40.0));
    assert!(TaskStatus::BudgetExceeded.is_final());

    // O caminho irmao, em tokens: o mesmo corte, a mesma mensagem com o numero.
    let a = agente(
        "falso:barato",
        vec![
            ScriptedLlm::call("c1", "write_file", json!({"path": "a.txt", "content": "1"})),
            ScriptedLlm::text("pronto"),
        ],
        com_precos(Politica {
            tarefa: Orcamento {
                tokens: Some(15),
                custo: None,
            },
            ..Politica::default()
        }),
    );
    let t = rodar(&a, Task::new("x", "falso:barato"), None).await;
    assert_eq!(t.status, TaskStatus::BudgetExceeded);
    assert!(
        t.error
            .as_deref()
            .unwrap()
            .contains("gastou 15 tokens, teto de 15 tokens"),
        "{:?}",
        t.error
    );
    assert_eq!(
        t.orcamento.as_ref().unwrap().tokens,
        Some(15),
        "padrao gravado"
    );
}

/// Dinheiro sem preco nao se confere: o motor recusa ANTES da primeira chamada, dizendo
/// por que (a API recusa antes ainda, no teste de baixo).
#[tokio::test]
async fn orcamento_em_dinheiro_sem_preco_recusa_antes_de_gastar() {
    let a = agente(
        "falso:sem-preco",
        vec![ScriptedLlm::text("pronto")],
        com_precos(Politica::default()),
    );
    let o = Orcamento {
        tokens: None,
        custo: Some(1.0),
    };
    let t = rodar(&a, Task::new("x", "falso:sem-preco"), Some(o)).await;
    assert_eq!(t.status, TaskStatus::Failed);
    assert!(
        t.error.as_deref().unwrap().contains("não tem preço"),
        "{:?}",
        t.error
    );
    assert_eq!(t.usage.input_tokens, 0, "gastou antes de recusar");
}

fn estado_da_api(a: Agent, dir: &std::path::Path) -> api::ApiState {
    let store = a.store.clone();
    api::ApiState {
        usuarios: Default::default(),
        store,
        factory: Arc::new(move |_m: &str| Ok(a.clone())),
        default_model: "falso:barato".into(),
        token: "token-de-teste-com-tamanho-suficiente".into(),
        running: Default::default(),
        agenda: Arc::new(std::sync::Mutex::new(
            Agenda::open(dir.join("agenda.json")).unwrap(),
        )),
        webhook_origins: vec![],
        limite: Arc::new(api::Limite::por_minuto(1000)),
    }
}

/// R3 na porta: pedido acima do teto global e recusado dizendo o teto; dinheiro para
/// modelo sem preco e recusado dizendo por que; e o pedido dentro do teto entra gravado.
///
/// RED medido: em `Politica::do_pedido`, o `return Err(...)` do teto de tokens trocado
/// por nada (`// REPOSTO`) -- o pedido de 5000 entrou (cortado no teto pelo motor) e a
/// asserção da recusa caiu.
#[tokio::test]
async fn pedido_nao_ultrapassa_o_teto_global_na_api() {
    let dir = tmp("api");
    let pol = Politica {
        teto: Orcamento {
            tokens: Some(1_000),
            custo: Some(5.0),
        },
        ..Politica::default()
    };
    let s = estado_da_api(
        agente(
            "falso:barato",
            vec![ScriptedLlm::text("ok")],
            com_precos(pol.clone()),
        ),
        &dir,
    );
    let pedido = |o: Orcamento| api::NovaTarefa {
        objective: "x".into(),
        orcamento: Some(o),
        ..Default::default()
    };
    let r = api::criar_tarefa(
        &s,
        pedido(Orcamento {
            tokens: Some(5_000),
            custo: None,
        }),
    );
    let e = r.err().expect("acima do teto entrou");
    assert_eq!(e.status, 400);
    assert!(e.erro.contains("teto global de 1000 tokens"), "{}", e.erro);
    let c = api::criar_tarefa(
        &s,
        pedido(Orcamento {
            tokens: Some(500),
            custo: None,
        }),
    )
    .map_err(|e| e.erro)
    .unwrap();
    let t = c.fim.await.unwrap();
    // O que o pedido omitiu (dinheiro) recebeu o teto: omitir nao e escapar.
    assert_eq!(
        t.orcamento,
        Some(Orcamento {
            tokens: Some(500),
            custo: Some(5.0)
        })
    );

    // Modelo sem preco, orcamento em dinheiro: recusado na criacao, dizendo por que.
    let s2 = estado_da_api(
        agente("falso:sem-preco", vec![], com_precos(Politica::default())),
        &dir,
    );
    let e = api::criar_tarefa(
        &s2,
        pedido(Orcamento {
            tokens: None,
            custo: Some(1.0),
        }),
    )
    .err()
    .expect("dinheiro sem preco entrou");
    assert!(e.erro.contains("não tem preço"), "{}", e.erro);
}

/// R3 no fluxo: os passos sao tarefas filhas e cobram a conta do fluxo; ao bater, o passo
/// que estourou para, o resto nao roda, e o fluxo termina `budget_exceeded` dizendo o gasto.
#[tokio::test]
async fn orcamento_do_fluxo_soma_os_passos_e_para() {
    let pol = Politica {
        fluxo: Orcamento {
            tokens: Some(25),
            custo: None,
        },
        ..Politica::default()
    };
    // Tres passos em fila, uma resposta de 15 tokens cada: o segundo passa de 25.
    let a = agente(
        "falso:barato",
        (0..3).map(|_| ScriptedLlm::text("feito")).collect(),
        com_precos(pol),
    );
    let f = fluxos::ler(
        &json!({"nome":"caro","passos":[
            {"id":"a","tarefa":"um"},
            {"id":"b","depende":["a"],"tarefa":"dois"},
            {"id":"c","depende":["b"],"tarefa":"tres"}
        ]})
        .to_string(),
    )
    .unwrap();
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso);
    let mae = a.store.load(&r.tarefa).unwrap();
    assert_eq!(mae.status, TaskStatus::BudgetExceeded, "{:?}", mae.error);
    assert!(
        mae.error
            .as_deref()
            .unwrap()
            .contains("gastou 30 tokens, teto de 25 tokens"),
        "{:?}",
        mae.error
    );
    assert_eq!(mae.gasto.as_ref().unwrap().tokens, 30);
    let estado = |id: &str| r.passos.iter().find(|p| p.id == id).unwrap().estado.clone();
    assert_eq!(
        (estado("a"), estado("b"), estado("c")),
        ("ok".into(), "falhou".into(), "bloqueado".into())
    );
}

/// O provedor fora do ar da rota: 503, que a politica troca.
struct Fora;

impl Llm for Fora {
    fn id(&self) -> String {
        "falso:fora".into()
    }
    fn chat<'a>(
        &'a self,
        _m: &'a [Message],
        _t: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async {
            Err(LlmError::Api {
                status: 503,
                body: "fora (prova)".into(),
            })
        })
    }
}

fn agente_de_rota(provedores: &[&str], roteiro: Vec<LlmReply>) -> Agent {
    let politica = phxclaw_agent::roteamento::Politica::de_json(
        &json!({"provedores": provedores.iter().map(|p| json!({"spec": p})).collect::<Vec<_>>()})
            .to_string(),
    )
    .unwrap();
    let roteiro = std::sync::Mutex::new(Some(roteiro));
    let rota = phxclaw_agent::roteamento::LlmRoteado::montar(politica, None, |s| {
        Ok(match s {
            "falso:fora" => Arc::new(Fora) as Arc<dyn Llm>,
            _ => Arc::new(Falso {
                id: s.into(),
                roteiro: ScriptedLlm::new(roteiro.lock().unwrap().take().unwrap_or_default()),
            }),
        })
    })
    .unwrap();
    Agent::new(
        Arc::new(rota),
        vec![],
        com_precos(Politica::default()),
        TaskStore::new(tmp("rota")).unwrap(),
    )
}

/// R2 com a rota (R4): o `id()` do modelo roteado e `rota`, que nao tem preco; o custo
/// sai do provedor que ATENDEU, pelo diario do roteamento. Aqui o primeiro da cadeia
/// (`falso:fora`, caro) devolve 503, a rota troca, e quem atende e o `falso:barato`: o
/// custo e o dele (20), nao o do caro nem «nao medido».
///
/// RED medido: em `Agent::cobrar`, o `atendeu.map_or_else(..)` trocado por `self.llm.id()`
/// (`// REPOSTO`) -- o custo saiu «nao medido» (`rota` sem preco) e a tarefa com teto em
/// dinheiro parou em `budget_exceeded` («não conferível»).
#[tokio::test]
async fn com_a_rota_o_custo_sai_do_provedor_que_atendeu() {
    let a = agente_de_rota(
        &["falso:fora", "falso:barato"],
        vec![ScriptedLlm::text("pronto")],
    );
    let o = Orcamento {
        tokens: None,
        custo: Some(1_000.0),
    };
    let t = rodar(&a, Task::new("x", "rota"), Some(o)).await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let c = t.custo.clone().unwrap();
    assert_eq!(c.total, Some(20.0), "{c:?}");
    assert_eq!(c.chamadas[0].modelo, "falso:barato");
    assert!(
        t.steps
            .iter()
            .any(|p| p.kind == "modelo" && p.summary.contains("atendeu falso:barato")),
        "a troca tem de aparecer no diario"
    );
}

/// Antes de gastar com a rota, nao se sabe quem vai atender: orcamento em dinheiro so se
/// a cadeia INTEIRA tem preco. Um provedor sem preco recusa, dizendo qual -- no motor e
/// na criacao pela API.
///
/// RED medido: em `Agent::abrir_conta`, `&self.llm.provedores()` trocado por
/// `&[self.llm.id()]` (`// REPOSTO`) -- a recusa passou a dizer «rota, que não tem preço»,
/// sem nomear o provedor da cadeia, e a asserção caiu.
#[tokio::test]
async fn rota_com_provedor_sem_preco_recusa_dinheiro_dizendo_qual() {
    let a = agente_de_rota(
        &["falso:barato", "falso:sem-preco"],
        vec![ScriptedLlm::text("pronto")],
    );
    let o = Orcamento {
        tokens: None,
        custo: Some(1.0),
    };
    let t = rodar(&a, Task::new("x", "rota"), Some(o.clone())).await;
    assert_eq!(t.status, TaskStatus::Failed);
    let e = t.error.unwrap();
    assert!(
        e.contains("falso:sem-preco não tem preço") && e.contains("rota"),
        "{e}"
    );
    assert_eq!(t.usage.input_tokens, 0, "gastou antes de recusar");
    let dir = tmp("rota-api");
    let s = estado_da_api(a, &dir);
    let e = api::criar_tarefa(
        &s,
        api::NovaTarefa {
            objective: "x".into(),
            orcamento: Some(o),
            ..Default::default()
        },
    )
    .err()
    .expect("rota com provedor sem preco aceitou dinheiro");
    assert!(e.erro.contains("falso:sem-preco"), "{}", e.erro);
}

// ------------------------------------------------------------------ R5: a bateria

/// O provedor falso da bateria: acerta («a resposta e 42») nos casos de `certos`, erra nos
/// outros; `mudo` nunca responde (o provedor sem chave / fora do ar).
struct Provedor {
    id: String,
    certos: Vec<usize>,
    mudo: bool,
}

impl Llm for Provedor {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn chat<'a>(
        &'a self,
        m: &'a [Message],
        _t: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            if self.mudo {
                return Err(LlmError::Credential("sem chave (prova)".into()));
            }
            let obj = m
                .iter()
                .rev()
                .find(|x| x.role == Role::User)
                .map(|x| x.content.clone())
                .unwrap_or_default();
            let n: usize = obj.trim_start_matches("caso ").parse().unwrap_or(999);
            Ok(LlmReply {
                content: if self.certos.contains(&n) {
                    "a resposta e 42".into()
                } else {
                    "nao sei".into()
                },
                tool_calls: vec![],
                usage: Usage {
                    input_tokens: 100,
                    output_tokens: 100,
                    duracao_geracao_ns: None,
                },
                model: self.id.clone(),
            })
        })
    }
}

fn bateria_de(n: usize, rodadas: usize, tentativas: usize) -> Bateria {
    Bateria {
        casos: (0..n)
            .map(|i| Caso {
                id: format!("c{i}"),
                objetivo: format!("caso {i}"),
                gabarito: Gabarito {
                    contem: vec!["42".into()],
                    sequencia: None,
                },
            })
            .collect(),
        rodadas,
        tentativas,
        reamostras: 1_000,
        semente: 7,
    }
}

async fn correr(
    provedores: Vec<(&'static str, Vec<usize>, bool)>,
    b: &Bateria,
) -> bateria::ResultadoBateria {
    let dir = tmp("bateria");
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let tab = tabela();
    let defs: Vec<(String, Vec<usize>, bool)> = provedores
        .iter()
        .map(|(p, c, m)| (p.to_string(), c.clone(), *m))
        .collect();
    let fabrica = move |modelo: &str| -> Result<Agent, String> {
        let (id, certos, mudo) = defs
            .iter()
            .find(|(p, ..)| p == modelo)
            .cloned()
            .ok_or("provedor desconhecido")?;
        Ok(Agent::new(
            Arc::new(Provedor { id, certos, mudo }),
            vec![],
            AgentConfig {
                precos: Some(tab.clone()),
                ..AgentConfig::default()
            },
            store.clone(),
        ))
    };
    let nomes: Vec<String> = provedores.iter().map(|(p, ..)| p.to_string()).collect();
    bateria::rodar(
        &fabrica,
        &nomes,
        b,
        &dir,
        &LeitorEnergia::de_raiz(&dir.join("sem-sys"), None),
    )
    .await
    .unwrap()
}

fn vencedor(r: &bateria::ResultadoBateria, metrica: &str) -> Option<String> {
    r.vencedores
        .iter()
        .find(|v| v.metrica == metrica)
        .unwrap()
        .modelo
        .clone()
}

fn faixa(m: &Medida) -> phxclaw_agent::avaliacao::Faixa {
    *m.faixa().unwrap_or_else(|| panic!("nao medida: {m:?}"))
}

/// R5, o lado que decide: o mesmo gabarito, e quem acerta tudo vence o que erra tudo,
/// com intervalos separados; o arena registra os pares (informativo); o provedor mudo
/// sai NAO MEDIDO e fora da comparacao.
#[tokio::test]
async fn bateria_entre_provedores_falsos_decide_com_faixas_separadas() {
    let b = bateria_de(6, 2, 1);
    let r = correr(
        vec![
            ("falso:certo", (0..6).collect(), false),
            ("falso:errado", vec![], false),
            ("falso:mudo", vec![], true),
        ],
        &b,
    )
    .await;
    assert_eq!(r.ensaios.len(), 3 * 6 * 2);
    let p = |n: &str| r.provedores.iter().find(|p| p.provedor == n).unwrap();
    assert_eq!(faixa(&p("falso:certo").acerto).mediana, 1.0);
    assert_eq!(faixa(&p("falso:errado").acerto).max, 0.0);
    assert_eq!(vencedor(&r, "acerto").as_deref(), Some("falso:certo"));
    // Custo por acerto: 200 tokens a 1000/milhao = 0,2 por ensaio, um acerto por ensaio.
    assert!((faixa(&p("falso:certo").custo_por_acerto).mediana - 0.2).abs() < 1e-9);
    // Quem nao acertou nada nao tem custo por acerto: nao medido, e sem vencedor ali.
    assert!(matches!(
        p("falso:errado").custo_por_acerto,
        Medida::NaoMedida(_)
    ));
    assert_eq!(vencedor(&r, "custo_por_acerto"), None);
    // O mudo: NAO MEDIDO, com o erro, e fora do vencedor.
    let mudo = p("falso:mudo").nao_medido.clone().unwrap();
    assert!(
        mudo.contains("NÃO MEDIDO") && mudo.contains("sem chave"),
        "{mudo}"
    );
    assert!(
        r.vencedores
            .iter()
            .all(|v| v.modelo.as_deref() != Some("falso:mudo"))
    );
    // O arena: uma janela por desafiante, com os 12 pares e o hash; o veredito existe e
    // nao e o que decidiu.
    assert_eq!(r.arena.len(), 2, "{:?}", r.arena_erro);
    assert_eq!(r.arena[0].janela.sample_count, 12);
    assert_eq!(r.arena[0].janela.window_sha256.len(), 64);
    assert!(bateria::tabela(&r).contains("informativo, não decide"));
}

/// R5, a guarda: medianas diferentes com intervalos que se cruzam NAO declaram vencedor.
///
/// RED medido: em `bateria::decidir`, o `vencedor_entre(&cands, *maior)` trocado pela maior
/// mediana (`// REPOSTO`) -- `falso:quase` (4/6) foi declarado vencedor sobre
/// `falso:metade` (3/6), e a asserção do `None` caiu.
#[tokio::test]
async fn vencedor_nao_declarado_com_faixas_cruzadas() {
    let b = bateria_de(6, 1, 1);
    let r = correr(
        vec![
            ("falso:quase", vec![0, 1, 2, 3], false),
            ("falso:metade", vec![2, 3, 4], false),
        ],
        &b,
    )
    .await;
    let p = |n: &str| r.provedores.iter().find(|p| p.provedor == n).unwrap();
    let (q, m) = (
        faixa(&p("falso:quase").acerto),
        faixa(&p("falso:metade").acerto),
    );
    assert!(
        q.mediana > m.mediana,
        "a prova precisa de medianas diferentes"
    );
    assert!(
        q.min <= m.max,
        "a prova precisa de faixas cruzadas: {q:?} {m:?}"
    );
    assert_eq!(vencedor(&r, "acerto"), None);
    let v = r.vencedores.iter().find(|v| v.metrica == "acerto").unwrap();
    assert!(v.motivo.contains("se cruzam"), "{}", v.motivo);
}

/// R2 na bateria: as tentativas que falharam tambem custaram. Com duas tentativas e um
/// provedor que nunca acerta o caso 1, o custo por acerto conta as duas tentativas dele.
/// Dez casos: com tres, ha reamostra so do caso que erra, e o intervalo do custo por
/// acerto sai sem teto (nao medido, e certo) -- a prova aqui e a da soma.
///
/// RED medido: em `bateria::ensaio`, a soma do custo movida para dentro do `if ex.acerto`
/// (`// REPOSTO`) -- o caso 1 saiu de graca, o custo por acerto caiu de 2,2/9 para 1,8/9
/// e a conta abaixo falhou.
#[tokio::test]
async fn custo_por_acerto_conta_as_tentativas_que_falharam() {
    let b = bateria_de(10, 1, 2);
    let certos: Vec<usize> = (0..10).filter(|i| *i != 1).collect();
    let r = correr(vec![("falso:quase", certos, false)], &b).await;
    let e1 = r.ensaios.iter().find(|e| e.caso == "c1").unwrap();
    assert_eq!((e1.acerto, e1.tentativas), (false, 2));
    // Nove casos de uma tentativa (0,2 cada) e o c1 com duas que falharam (0,4):
    // 2,2 sobre 9 acertos.
    let c = faixa(&r.provedores[0].custo_por_acerto).mediana;
    assert!((c - 2.2 / 9.0).abs() < 1e-9, "custo por acerto {c}");
    assert!((faixa(&r.provedores[0].tentativas).mediana - 1.1).abs() < 1e-9);
}

//! PHX Flow Engine, onda 3 (SP000035) e os itens simples da onda 4: `esperar` que
//! descarrega para o disco e retoma depois de o processo reiniciar, dados pinados, poda
//! das execucoes, formulario servido pelo gatilho, binario por referencia com o teto de
//! bytes por passo do DBA, exportar/importar, etiquetas e pastas, e o limite de fluxos
//! simultaneos da instancia.
//!
//! Prova real: cada teste traz, no comentario, a linha que reposta o derruba. Os marcados
//! «RED medido» tiveram o defeito reposto de verdade em 09/10/2026, o teste caiu, e o
//! conserto voltou; os outros dizem a linha e NAO foram repostos um a um.
//!
//! «Matar o executor» aqui e soltar o `Agent`, a ferramenta e o `TaskStore` da primeira
//! banca e montar outra do zero sobre a MESMA pasta: nada em memoria atravessa -- so o
//! disco, que e o que sobra de um processo que cai.

mod comum_fluxo;

use comum_fluxo::*;
use phxclaw_agent::api::criar_fluxo_com;
use phxclaw_agent::fluxos::{self, Execucao, Poda};
use phxclaw_agent::gatilhos::{GatilhoDeWebhook, Gatilhos};
use phxclaw_agent::*;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::time::Duration;

// ======================================================================== espera

/// `espera_wait` (tempo, e o reinicio do processo): o fluxo roda ate o `esperar`, grava o
/// progresso e a definicao, para em `AwaitingInput` e a execucao TERMINA -- nada segura a
/// espera. Uma banca nova sobre a mesma pasta (o processo que reiniciou) acha a espera
/// vencida pelo disco e retoma: `a` nao roda de novo, `b` roda uma vez. Retomar ANTES do
/// vencimento nao muda o vencimento gravado (`ms` conta de quando a espera abriu).
///
/// RED medido: em `fluxos::executar`, `esperas_abertas.remove(&p.id)` trocado por `None`
/// (a retomada abre a espera de novo, com agora + ms): aos 900 ms a espera ainda nao venceu
/// e `esperas_vencidas` volta vazia.
#[tokio::test]
async fn espera_wait() {
    let raiz = tmp("espera");
    let f = fluxo(json!({"nome":"espera","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"pedido-7"}},
        {"id":"pausa","depende":["a"],"esperar":{"ms":800}},
        {"id":"b","depende":["a","pausa"],"ferramenta":"eco",
         "args":{"texto":"{{a}} depois de {{pausa.esperou_ate}}"}}
    ]}));
    let inicio = std::time::Instant::now();
    let id = {
        let b = banca_em(&raiz, None);
        let r = fluxos::rodar(&b.a, &f).await.unwrap();
        assert!(!r.sucesso);
        assert_eq!(passo(&r, "a").estado, "ok");
        assert_eq!(passo(&r, "pausa").estado, "esperando");
        assert_eq!(passo(&r, "pausa").espera.as_ref().unwrap().tipo, "tempo");
        assert_eq!(passo(&r, "b").estado, "pendente");
        assert_eq!(b.eco.n(), 1);
        let t = b.a.store.load(&r.tarefa).unwrap();
        assert_eq!(t.status, TaskStatus::AwaitingInput);
        assert!(t.question.as_deref().unwrap().contains("esperando ate"));
        assert!(
            b.a.store
                .dir(&r.tarefa)
                .join(fluxos::ARQUIVO_DEFINICAO)
                .is_file()
        );
        let v = relatorio_cru(&b.a, &r.tarefa);
        assert_eq!(v["formato"], json!(fluxos::FORMATO_ONDA3));
        r.tarefa
        // `b` cai aqui: o agente, o eco e o store somem.
    };
    let b2 = banca_em(&raiz, None);
    assert!(
        fluxos::esperas_vencidas(&b2.a.store, fluxos::agora()).is_empty(),
        "ainda nao venceu"
    );
    // Retomar antes do vencimento: nada roda e a espera continua, com o vencimento de antes.
    tokio::time::sleep(Duration::from_millis(400).saturating_sub(inicio.elapsed())).await;
    let r = fluxos::retomar_do_disco(&b2.a, &id).await.unwrap();
    assert_eq!(passo(&r, "pausa").estado, "esperando");
    assert_eq!(b2.eco.n(), 0);
    assert_eq!(
        b2.a.store.load(&id).unwrap().status,
        TaskStatus::AwaitingInput
    );
    tokio::time::sleep(Duration::from_millis(900).saturating_sub(inicio.elapsed())).await;
    assert_eq!(
        fluxos::esperas_vencidas(&b2.a.store, fluxos::agora()),
        vec![id.clone()],
        "o vencimento gravado (800 ms do inicio) passou; o reaberto (400 + 800) nao"
    );
    let r = fluxos::retomar_do_disco(&b2.a, &id).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert!(passo(&r, "a").reaproveitado);
    assert!(
        passo(&r, "b").saida.starts_with("pedido-7 depois de 20"),
        "{}",
        passo(&r, "b").saida
    );
    assert_eq!(b2.eco.n(), 1, "so o b rodou no processo novo");
    let t = b2.a.store.load(&id).unwrap();
    assert_eq!(t.status, TaskStatus::Completed);
    assert!(t.question.is_none());
    // Terminada, nao se retoma de novo pelo disco.
    let e = fluxos::retomar_do_disco(&b2.a, &id).await.unwrap_err();
    assert!(e.to_string().contains("nao esta parada numa espera"), "{e}");
    // Validacao: um modo so, sem por_item, data valida, hash e nao segredo.
    for (esp, esperado) in [
        (json!({"ms": 1, "pergunta": "x"}), "exatamente um"),
        (json!({"ms": 0}), "maior que zero"),
        (json!({"ate": "amanha"}), "RFC 3339"),
        (
            json!({"webhook": {"segredo_sha256": "segredo-em-claro"}}),
            "nunca o segredo",
        ),
    ] {
        let e = fluxos::ler(&json!({"nome":"x","passos":[{"id":"e","esperar": esp}]}).to_string())
            .unwrap_err();
        assert!(e.contains(esperado), "{esp}: {e}");
    }
}

/// `espera_wait` pela API: a pergunta e respondida na MESMA rota `POST /v1/tasks/{id}/answer`
/// da SP000029 (sem ninguem em memoria esperando: a resposta e gravada no passo e o fluxo e
/// retomado do disco), e a espera de webhook em `POST /v1/flows/{id}/resume`, com o token da
/// API ou com o segredo cujo sha256 a espera guarda -- o segredo nunca vai ao disco. Resposta
/// com forma de segredo e recusada antes de virar item gravado.
///
/// Reposta que derruba: em `api::responder`, o desvio `espera_aberta(..) == "pergunta"`
/// removido (a resposta vai para `perguntas::responder`, que nao tem ninguem esperando, e a
/// rota devolve 409).
#[tokio::test]
async fn espera_wait_pergunta_e_webhook_pela_api() {
    let raiz = tmp("espera-api");
    let s = estado(&raiz);
    let base = servir(&s, Gatilhos::default()).await;
    let http = reqwest::Client::new();

    // pergunta
    let q = gravar(
        &raiz,
        "q",
        &json!({"nome":"q","passos":[
            {"id":"a","ferramenta":"eco","args":{"texto":"pedido-9"}},
            {"id":"ok","depende":["a"],"esperar":{"pergunta":"Aprova {{a}}?"}},
            {"id":"b","depende":["ok"],"ferramenta":"eco","args":{"texto":"resposta={{ok.resposta}}"}}
        ]}),
    );
    let c = criar_fluxo_com(&s, &q.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    let t = ate_o_estado(&s, &id, &[TaskStatus::AwaitingInput]).await;
    assert_eq!(t.question.as_deref(), Some("Aprova pedido-9?"));
    let responder = |texto: &str| {
        http.post(format!("{base}/v1/tasks/{id}/answer"))
            .bearer_auth(TOKEN)
            .json(&json!({"answer": texto}))
            .send()
    };
    let r = responder("ghp_0123456789abcdefABCDEF").await.unwrap();
    assert_eq!(r.status(), 409, "segredo nao vira item");
    let disco = std::fs::read_to_string(s.store.dir(&id).join("task.json")).unwrap();
    assert!(!disco.contains("ghp_0123456789"));
    let r = responder("sim").await.unwrap();
    assert_eq!(r.status(), 202);
    let t = ate_o_estado(&s, &id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    let rel = relatorio(&t);
    assert_eq!(passo(&rel, "b").saida, "resposta=sim");
    assert_eq!(passo(&rel, "a").estado, "ok");

    // webhook, com o hash do segredo no fluxo
    let segredo = "segredo-da-espera-1234567890";
    let w = gravar(
        &raiz,
        "w",
        &json!({"nome":"w","passos":[
            {"id":"w","esperar":{"webhook":{"segredo_sha256": sha256(segredo.as_bytes())}}},
            {"id":"b","depende":["w"],"ferramenta":"eco","args":{"texto":"n={{w.n}}"}}
        ]}),
    );
    let c = criar_fluxo_com(&s, &w.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    let t = ate_o_estado(&s, &id, &[TaskStatus::AwaitingInput]).await;
    assert!(
        t.question
            .unwrap()
            .contains(&format!("/v1/flows/{id}/resume"))
    );
    let retomar = |seg: Option<&str>| {
        let mut r = http
            .post(format!("{base}/v1/flows/{id}/resume"))
            .body(r#"{"n": 5}"#);
        if let Some(sg) = seg {
            r = r.header("x-phxclaw-segredo", sg);
        }
        r.send()
    };
    assert_eq!(retomar(None).await.unwrap().status(), 401);
    assert_eq!(retomar(Some("outro")).await.unwrap().status(), 401);
    assert_eq!(retomar(Some(segredo)).await.unwrap().status(), 202);
    let t = ate_o_estado(&s, &id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert_eq!(passo(&relatorio(&t), "b").saida, "n=5");
    // o segredo nao mora no disco: nem no task.json, nem na definicao gravada
    let dir = s.store.dir(&id);
    for arq in ["task.json", fluxos::ARQUIVO_DEFINICAO] {
        let txt = std::fs::read_to_string(dir.join(arq)).unwrap();
        assert!(!txt.contains(segredo), "{arq}");
    }
    // espera ja entregue: sem o token, a segunda chamada nao alcanca espera nenhuma e
    // ouve o mesmo 401 de quem nao tem segredo (quem nao tem o token nao distingue «sem
    // espera» de «segredo errado»); com o token, o 409 diz que nao ha espera aberta.
    assert_eq!(retomar(Some(segredo)).await.unwrap().status(), 401);
    let r = http
        .post(format!("{base}/v1/flows/{id}/resume"))
        .bearer_auth(TOKEN)
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 409);
}

// ======================================================================== pins

/// `dados_pinados`: o pin no `ARQ.pins.json` (editavel pelo `pinar`) SUBSTITUI a execucao
/// do passo; `--ate` atras de um passo pinado nao roda o que so o alimentava; a retomada
/// respeita o pin; trocar o pin muda a definicao (retomar a execucao velha e recusado);
/// pin com forma de segredo, em `se` ou em passo inexistente nunca chega ao disco; e a
/// saida de uma execucao vira pin (`saida_para_pin`).
///
/// RED medido: em `fluxos::executar`, o bloco `if let Some(pin) = &p.pin { ... }` removido
/// (o passo `b` roda pelo portao, o eco conta 3 e `b` nao sai `pinado`).
#[tokio::test]
async fn dados_pinados() {
    let raiz = tmp("pin");
    let arq = gravar(
        &raiz,
        "p",
        &json!({"nome":"p","passos":[
            {"id":"a","ferramenta":"eco","args":{"texto":"a"}},
            {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}}b"}},
            {"id":"c","depende":["b"],"ferramenta":"eco","args":{"texto":"{{b.k}}c"}},
            {"id":"cond","depende":["c"],"se":{"caminho":"","operador":"existe"}}
        ]}),
    );
    fluxos::pinar(&arq, "b", Some(json!([{"k": "PIN"}]))).unwrap();
    assert!(fluxos::arquivo_de_pins(&arq).is_file());
    let f = fluxos::ler_arquivo(&arq).unwrap();
    let b = banca_em(&raiz, None);
    // O pin so vale na execucao manual que o pede (`--pins`; achado M1).
    let r = fluxos::rodar_com(
        &b.a,
        &f,
        Execucao {
            pins: true,
            ..Execucao::default()
        },
    )
    .await
    .unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert!(passo(&r, "b").pinado);
    assert_eq!(passo(&r, "b").itens, vec![json!({"k": "PIN"})]);
    assert_eq!(passo(&r, "c").saida, "PINc");
    assert_eq!(b.eco.n(), 2, "a e c; o b nao passou pelo portao");

    // --ate atras do pin: o `a` so alimentava o `b`, e nao roda
    let b2 = banca_em(&raiz, None);
    let r = fluxos::rodar_com(
        &b2.a,
        &f,
        Execucao {
            ate: Some("c"),
            pins: true,
            ..Execucao::default()
        },
    )
    .await
    .unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "a").estado, "nao_pedido");
    assert!(passo(&r, "b").pinado);
    assert_eq!(passo(&r, "c").saida, "PINc");
    assert_eq!(b2.eco.n(), 1);
    // a retomada continua respeitando o pin: roda o que ficou de fora, nada mais
    let r2 = fluxos::retomar(&b2.a, &f, &r.tarefa).await.unwrap();
    assert!(r2.sucesso, "{r2:#?}");
    assert_eq!(passo(&r2, "a").estado, "ok");
    assert!(passo(&r2, "c").reaproveitado);
    assert_eq!(b2.eco.n(), 2);

    // trocar o pin e trocar a definicao
    fluxos::pinar(&arq, "b", Some(json!({"k": "OUTRO"}))).unwrap();
    let f2 = fluxos::ler_arquivo(&arq).unwrap();
    assert_ne!(fluxos::assinatura(&f), fluxos::assinatura(&f2));
    let e = fluxos::retomar(&b2.a, &f2, &r.tarefa).await.unwrap_err();
    assert!(e.contains("definicao do fluxo mudou"), "{e}");

    // recusas: segredo, `se`, passo inexistente -- e o arquivo de pins nao muda
    let antes = std::fs::read_to_string(fluxos::arquivo_de_pins(&arq)).unwrap();
    for (passo_, valor, esperado) in [
        ("b", json!({"api_key": "x"}), "segredo"),
        ("b", json!("ghp_0123456789abcdefABCDEF"), "segredo"),
        ("cond", json!(true), "nao aceita pin"),
        ("zzz", json!(1), "nao existe"),
    ] {
        let e = fluxos::pinar(&arq, passo_, Some(valor)).unwrap_err();
        assert!(e.contains(esperado), "{passo_}: {e}");
    }
    assert_eq!(
        std::fs::read_to_string(fluxos::arquivo_de_pins(&arq)).unwrap(),
        antes
    );
    // o mesmo passo pinado no fluxo E no arquivo: duas fontes, recusado
    let dupla = gravar(
        &raiz,
        "dupla",
        &json!({"nome":"d","passos":[{"id":"a","ferramenta":"eco","pin":"x"}]}),
    );
    std::fs::write(fluxos::arquivo_de_pins(&dupla), r#"{"a": "y"}"#).unwrap();
    assert!(
        fluxos::ler_arquivo(&dupla)
            .unwrap_err()
            .contains("deixe um so")
    );

    // a saida de uma execucao vira pin; despinar tira o arquivo
    let saida = fluxos::saida_para_pin(&b2.a.store, &r2.tarefa, "a").unwrap();
    assert_eq!(saida, json!(["a"]));
    fluxos::pinar(&arq, "a", Some(saida)).unwrap();
    assert!(fluxos::ler_arquivo(&arq).unwrap().passos[0].pin.is_some());
    fluxos::pinar(&arq, "a", None).unwrap();
    fluxos::pinar(&arq, "b", None).unwrap();
    assert!(!fluxos::arquivo_de_pins(&arq).exists());
    assert!(
        fluxos::ler_arquivo(&arq)
            .unwrap()
            .passos
            .iter()
            .all(|p| p.pin.is_none())
    );
}

// ======================================================================== poda

/// `poda_execucoes`: por contagem, ficam as N execucoes de fluxo terminadas mais novas, e a
/// tarefa filha de uma podada vai junto; por idade, sai a terminada velha. Execucao em
/// andamento ou esperando NUNCA sai, nem velha; tarefa que nao e fluxo nunca e tocada.
///
/// RED medido: em `fluxos::podar`, a condicao `!t.status.is_final() ||` removida (a
/// execucao esperando e a em andamento entram na conta e sao apagadas).
/// E (09/10, revisao) a outra metade, `|| desc.iter().any(|d| !d.status.is_final())`, removida:
/// a mae terminada com a filha rodando entra na conta (RED medido).
#[tokio::test]
async fn poda_execucoes() {
    let raiz = tmp("poda");
    let b = banca_em(&raiz, None);
    let f =
        fluxo(json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}));
    let mut terminadas = Vec::new();
    for _ in 0..4 {
        terminadas.push(fluxos::rodar(&b.a, &f).await.unwrap().tarefa);
    }
    // a filha da mais velha (um subagente de passo) vai com ela
    let mut filha = Task::new("subagente do passo", "m");
    filha.parent = Some(terminadas[0].clone());
    filha.status = TaskStatus::Completed;
    b.a.store.save(&filha).unwrap();
    // esperando, e velha
    let espera = fluxo(json!({"nome":"e","passos":[{"id":"q","esperar":{"pergunta":"ok?"}}]}));
    let esperando = fluxos::rodar(&b.a, &espera).await.unwrap().tarefa;
    let mut t = b.a.store.load(&esperando).unwrap();
    t.updated_at = chrono::Utc::now() - chrono::Duration::days(90);
    b.a.store.save(&t).unwrap();
    // em andamento, e velha
    let mut rodando = fluxos::tarefa_do_fluxo(&f, "m");
    rodando.status = TaskStatus::Running;
    rodando.updated_at = chrono::Utc::now() - chrono::Duration::days(90);
    b.a.store.save(&rodando).unwrap();
    // terminada, mas com a filha ainda rodando: as duas ficam (a metade
    // `desc.any(|d| !d.status.is_final())` da condicao, que o caso acima nao cobre)
    let mae_viva = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let mut t = b.a.store.load(&mae_viva).unwrap();
    t.updated_at = chrono::Utc::now() - chrono::Duration::days(90);
    b.a.store.save(&t).unwrap();
    let mut filha_viva = Task::new("subagente rodando", "m");
    filha_viva.parent = Some(mae_viva.clone());
    filha_viva.status = TaskStatus::Running;
    b.a.store.save(&filha_viva).unwrap();
    // nao e fluxo, terminada e velha
    let mut comum = Task::new("tarefa comum", "m");
    comum.status = TaskStatus::Completed;
    comum.updated_at = chrono::Utc::now() - chrono::Duration::days(90);
    b.a.store.save(&comum).unwrap();

    let p = fluxos::podar(
        &b.a.store,
        Poda {
            dias: None,
            max: Some(1),
        },
        fluxos::agora(),
    );
    assert!(p.erros.is_empty(), "{:?}", p.erros);
    let existe = |id: &str| b.a.store.load(id).is_ok();
    for id in &terminadas[..3] {
        assert!(!existe(id), "terminada velha {id} ficou");
    }
    assert!(!existe(&filha.id), "a filha ficou orfa");
    assert!(existe(&terminadas[3]), "a mais nova sai so por idade");
    assert!(existe(&esperando), "esperando nunca sai");
    assert!(existe(&rodando.id), "em andamento nunca sai");
    assert!(existe(&comum.id), "tarefa comum nao e da poda de fluxo");
    assert!(existe(&mae_viva), "mae com filha rodando nunca sai");
    assert!(existe(&filha_viva.id), "a filha rodando nunca sai");
    assert_eq!(p.removidas.len(), 4);

    // por idade: a terminada que sobrou, envelhecida, sai; as outras continuam
    let mut t = b.a.store.load(&terminadas[3]).unwrap();
    t.updated_at = chrono::Utc::now() - chrono::Duration::days(10);
    b.a.store.save(&t).unwrap();
    let p = fluxos::podar(
        &b.a.store,
        Poda {
            dias: Some(5),
            max: None,
        },
        fluxos::agora(),
    );
    assert_eq!(p.removidas, vec![terminadas[3].clone()]);
    assert!(existe(&esperando) && existe(&rodando.id) && existe(&comum.id));
    assert!(existe(&mae_viva) && existe(&filha_viva.id));
}

/// O comportamento VELHO da poda: sem configuracao, NADA se apaga -- nem com cem
/// execucoes terminadas e velhas. `Poda::do_config()` sem as chaves e a poda desligada.
///
/// Reposta que derruba: em `fluxos::podar`, o `if !poda.ligada() { return feito; }`
/// removido junto de um padrao `max: Some(0)` (tudo vira excedente).
#[tokio::test]
async fn poda_sem_configuracao_nada_apagado() {
    let raiz = tmp("poda-velha");
    let b = banca_em(&raiz, None);
    let f =
        fluxo(json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}));
    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
        let mut t = b.a.store.load(&id).unwrap();
        t.updated_at = chrono::Utc::now() - chrono::Duration::days(3650);
        b.a.store.save(&t).unwrap();
        ids.push(id);
    }
    // A configuracao ISOLADA da maquina: a do processo le o ambiente e o `config.json` de
    // quem roda, e um `fluxos.poda_*` la daria RED falso aqui. Uma configuracao carregada
    // de pasta vazia e ambiente vazio e o «sem configuracao» de verdade.
    let vazia = raiz.join("config-vazia");
    std::fs::create_dir_all(&vazia).unwrap();
    let cfg = phxclaw_config_runtime::agente::carga::carregar(
        &|_: &str| None,
        &vazia.join("config.json"),
        None,
    )
    .unwrap();
    let poda = Poda::de(|k| cfg.inteiro(k));
    assert_eq!(poda, Poda::default());
    assert!(!poda.ligada());
    // e o leitor e o de verdade: a variavel de ambiente liga a poda
    let com = phxclaw_config_runtime::agente::carga::carregar(
        &|k: &str| (k == "PHXCLAW_FLUXOS_PODA_DIAS").then(|| "7".to_string()),
        &vazia.join("config.json"),
        None,
    )
    .unwrap();
    assert_eq!(Poda::de(|k| com.inteiro(k)).dias, Some(7));
    let p = fluxos::podar(&b.a.store, poda, fluxos::agora());
    assert!(p.removidas.is_empty() && p.erros.is_empty());
    for id in &ids {
        assert!(b.a.store.load(id).is_ok());
    }
}

/// `poda_execucoes` com o «infinito» do operador: `poda_dias` enorme (999999999, 2e11) e
/// `max_simultaneos` acima do teto do tokio nao derrubam nada -- antes, `podar` entrava em
/// panico na subtracao de data, e esse `podar` roda na tarefa da agenda (`manter_fluxos`),
/// que morria calada. Fora do intervalo, nada e velho o bastante: nada se apaga.
///
/// Reposta que derruba: em `fluxos::podar`, o calculo do `limite` de volta a
/// `agora - chrono::Duration::days(i64::try_from(d).unwrap_or(i64::MAX / 86_400_000))`
/// (panico «`DateTime - TimeDelta` overflowed»).
#[tokio::test]
async fn poda_com_dias_enormes_nao_derruba_e_nao_apaga() {
    let raiz = tmp("poda-infinita");
    let b = banca_em(&raiz, None);
    let f =
        fluxo(json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let mut t = b.a.store.load(&id).unwrap();
    t.updated_at = chrono::Utc::now() - chrono::Duration::days(3650);
    b.a.store.save(&t).unwrap();
    for dias in [100_000_000u64, 999_999_999, 200_000_000_000, u64::MAX] {
        let p = fluxos::podar(
            &b.a.store,
            Poda {
                dias: Some(dias),
                max: None,
            },
            fluxos::agora(),
        );
        assert!(
            p.removidas.is_empty() && p.erros.is_empty(),
            "{dias}: {p:?}"
        );
    }
    assert!(b.a.store.load(&id).is_ok());
    // Limite acima do teto do tokio vira o teto, sem panico.
    fluxos::definir_limite(b.a.store.root(), Some(usize::MAX));
    fluxos::rodar(&b.a, &f).await.unwrap();
    fluxos::definir_limite(b.a.store.root(), None);
    // O irmao: `max_paralelo` do proprio fluxo, que chega pelo JSON da agenda, dos gatilhos,
    // da API e do sub-fluxo. Reposta que derruba: em `fluxos::executar`, o
    // `semaforo(fluxo.max_paralelo)` de volta a `Semaphore::new(fluxo.max_paralelo)`.
    let enorme = fluxo(json!({"nome":"p","max_paralelo":usize::MAX,
        "passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}));
    assert!(fluxos::rodar(&b.a, &enorme).await.unwrap().sucesso);
}

// ======================================================================== formulario

/// `gatilho_formulario`: o gatilho de webhook cujo fluxo declara `formulario` serve a
/// pagina no `GET` (os campos declarados, HTML sem script) e, no `POST` urlencoded, passa
/// pelo MESMO portao do webhook (o segredo vem no campo `_segredo`, que o navegador sabe
/// mandar), confere os campos e dispara o fluxo com eles como UM item. Campo obrigatorio
/// vazio, campo que o fluxo nao declarou, numero que nao e numero, valor com forma de
/// segredo e corpo acima do teto sao recusados; o segredo do formulario nao vai para o
/// item nem para o `task.json`.
///
/// RED medido: em `gatilhos::webhook`, o `codigo.as_deref()` passado ao `portao` trocado
/// por `None` (o formulario do navegador nunca autentica: 401 onde se esperava 202).
#[tokio::test]
async fn gatilho_formulario() {
    let raiz = tmp("form");
    let s = estado(&raiz);
    let contato = gravar(
        &raiz,
        "contato",
        &json!({"nome":"contato",
            "formulario":{"titulo":"Fale conosco","botao":"Mandar","campos":[
                {"nome":"nome","rotulo":"Seu nome","obrigatorio":true},
                {"nome":"idade","tipo":"numero"},
                {"nome":"email","tipo":"email"},
                {"nome":"obs","tipo":"area"}]},
            "passos":[{"id":"eco","ferramenta":"eco",
                "args":{"texto":"{{entrada.nome}} tem {{entrada.idade}}"}}]}),
    );
    let sem_form = gravar(
        &raiz,
        "sem",
        &json!({"nome":"sem","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}),
    );
    let segredo = "segredo-do-formulario-1234567890";
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![
            GatilhoDeWebhook {
                nome: "contato".into(),
                objetivo: String::new(),
                fluxo: Some(contato.to_string_lossy().into_owned()),
                segredo: Some("segredo-do-gatilho-que-humano-nao-ve-123".into()),
                // O codigo que a pagina pede e o do formulario, nao o do gatilho (M6).
                segredo_formulario: Some(segredo.into()),
            },
            GatilhoDeWebhook {
                nome: "sem".into(),
                objetivo: String::new(),
                fluxo: Some(sem_form.to_string_lossy().into_owned()),
                segredo: None,
                segredo_formulario: None,
            },
        ],
    };
    let base = servir(&s, g).await;
    let http = reqwest::Client::new();

    let pagina = http
        .get(format!("{base}/v1/triggers/contato"))
        .send()
        .await
        .unwrap();
    assert_eq!(pagina.status(), 200);
    let html = pagina.text().await.unwrap();
    for t in [
        "Fale conosco",
        "Seu nome",
        "name=\"idade\" type=\"number\"",
        "type=\"email\"",
        "<textarea",
        "type=\"password\"",
        "Mandar",
    ] {
        assert!(html.contains(t), "{t} em {html}");
    }
    assert!(!html.to_lowercase().contains("<script"));
    assert_eq!(
        http.get(format!("{base}/v1/triggers/sem"))
            .send()
            .await
            .unwrap()
            .status(),
        404,
        "fluxo sem formulario nao serve pagina"
    );

    let postar = |corpo: String| {
        http.post(format!("{base}/v1/triggers/contato"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(corpo)
            .send()
    };
    let com = |campos: &str| format!("_segredo={segredo}&{campos}");
    assert_eq!(
        postar("nome=Ana&idade=31".into()).await.unwrap().status(),
        401
    );
    assert_eq!(
        postar("_segredo=errado&nome=Ana".into())
            .await
            .unwrap()
            .status(),
        401
    );
    for (campos, motivo) in [
        ("idade=31", "obrigatorio"),
        ("nome=Ana&extra=1", "nao esta no formulario"),
        ("nome=Ana&idade=trinta", "nao e numero"),
        ("nome=Ana&email=sem-arroba", "nao e e-mail"),
        ("nome=ghp_0123456789abcdefABCDEF", "parece segredo"),
    ] {
        let r = postar(com(campos)).await.unwrap();
        assert_eq!(r.status(), 400, "{campos}");
        let v: Value = r.json().await.unwrap();
        assert!(
            v["error"].as_str().unwrap().contains(motivo),
            "{campos}: {v}"
        );
    }
    let grande = format!("nome={}", "a".repeat(20 * 1024));
    assert_eq!(postar(com(&grande)).await.unwrap().status(), 413);

    let r = postar(com("nome=Ana+Maria&idade=31&email=ana%40x.org&obs="))
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let html = r.text().await.unwrap();
    let t = s
        .store
        .list()
        .unwrap()
        .into_iter()
        .find(|t| t.objective == format!("{}contato", fluxos::PREFIXO_TAREFA))
        .expect("o formulario disparou o fluxo");
    assert!(html.contains(&t.id));
    let t = ate_o_estado(&s, &t.id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert_eq!(passo(&relatorio(&t), "eco").saida, "Ana Maria tem 31");
    let disco = std::fs::read_to_string(s.store.dir(&t.id).join("task.json")).unwrap();
    assert!(!disco.contains(segredo) && !disco.contains("_segredo"));
    // um disparo so: as recusas nao criaram tarefa
    assert_eq!(s.store.list().unwrap().len(), 1);

    // o fluxo nao declara campo com nome de segredo
    let e = fluxos::ler(
        &json!({"nome":"x","formulario":{"titulo":"t","campos":[{"nome":"senha"}]},
            "passos":[{"id":"a","ferramenta":"eco"}]})
        .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("nome de segredo"), "{e}");
}

// ======================================================================== binarios

/// `dados_binarios`: item `{"base64", "mime"}` vira arquivo em `binarios/` e o item guarda
/// a referencia (caminho, sha256, tamanho, mime) -- o passo seguinte le o arquivo pelo
/// caminho, e o `task.json` nao tem o base64. Saida acima do teto de bytes por passo (DBA)
/// vai para `saidas/` com sha256: o `task.json` fica pequeno, a retomada a traz de volta
/// inteira, e arquivo adulterado (saida ou binario) e recusa da retomada.
///
/// RED medido: em `relatorio_para_disco`, a condicao `tamanho <= TETO_BYTES_PASSO`
/// trocada por `true` (nada vai para `saidas/`, e o `task.json` passa de 100 KiB).
#[tokio::test]
async fn dados_binarios() {
    use base64::Engine as _;
    let raiz = tmp("bin");
    let b = banca_em(&raiz, None);
    let bytes = b"conteudo do binario de teste";
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let grande = "x".repeat(100_000);
    let f = fluxo(json!({"nome":"bin","passos":[
        {"id":"img","ferramenta":"eco",
         "args":{"texto":{"base64": b64, "mime": "image/png", "nome": "foto.png"}}},
        {"id":"le","depende":["img"],"ferramenta":"read_file",
         "args":{"path":"{{img.binario.caminho}}"}},
        {"id":"big","ferramenta":"eco","args":{"texto": grande}},
        {"id":"cai","depende":["big","le"],"ferramenta":"eco","args":{"falhar":"de proposito"}}
    ]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(!r.sucesso);
    let item = &passo(&r, "img").itens[0];
    let refe = &item["binario"];
    assert_eq!(refe["sha256"], json!(sha256(bytes)));
    assert_eq!(refe["bytes"], json!(bytes.len()));
    assert_eq!(refe["mime"], json!("image/png"));
    assert_eq!(item["nome"], json!("foto.png"));
    assert!(item.get("base64").is_none());
    let rel = refe["caminho"].as_str().unwrap();
    assert!(rel.starts_with(fluxos::PASTA_BINARIOS) && rel.ends_with(".png"));
    let work = b.a.store.workdir(&r.tarefa);
    assert_eq!(std::fs::read(work.join(rel)).unwrap(), bytes);
    assert_eq!(passo(&r, "le").saida, "conteudo do binario de teste");
    assert_eq!(passo(&r, "big").saida.len(), 100_000, "em memoria, inteira");

    let disco = cru(&b.a, &r.tarefa);
    assert!(!disco.contains(&b64), "base64 no task.json");
    assert!(
        disco.len() < 16 * 1024,
        "task.json com {} bytes: a saida grande nao foi para saidas/",
        disco.len()
    );
    let v = relatorio_cru(&b.a, &r.tarefa);
    assert_eq!(v["formato"], json!(fluxos::FORMATO_ONDA3));
    let big = v["passos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "big")
        .unwrap()
        .clone();
    let ext = &big["externo"];
    let arq =
        b.a.store
            .dir(&r.tarefa)
            .join(ext["caminho"].as_str().unwrap());
    let conteudo = std::fs::read(&arq).unwrap();
    assert_eq!(ext["sha256"], json!(sha256(&conteudo)));
    assert_eq!(ext["bytes"], json!(conteudo.len()));

    // retomada: a saida grande volta inteira, e nada de img/big/le roda de novo
    let antes = b.eco.n();
    let r2 = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap();
    assert!(passo(&r2, "big").reaproveitado);
    assert_eq!(passo(&r2, "big").saida.len(), 100_000);
    assert_eq!(b.eco.n(), antes + 1, "so o `cai` rodou de novo");

    // adulterado: recusa com o motivo, nunca saida vazia aplicada calada
    let mut torto = conteudo.clone();
    torto[20] ^= 1;
    std::fs::write(&arq, &torto).unwrap();
    let e = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap_err();
    assert!(e.contains("nao confere"), "{e}");
    std::fs::write(&arq, &conteudo).unwrap();
    std::fs::write(work.join(rel), b"outro").unwrap();
    let e = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap_err();
    assert!(e.contains("binario") && e.contains("nao confere"), "{e}");
}

// ======================================================================== onda 4

/// `cli_importar_exportar`: o pacote leva a definicao com os pins e a assinatura; importar
/// confere formato, validade e assinatura, grava o fluxo e os pins no `.pins.json` (onde o
/// `pinar` os edita) e nao sobrescreve. Pacote editado no caminho, com segredo ou de
/// formato desconhecido e recusado. A CLI (`phxclaw fluxo exportar|importar`) e porta fina
/// para estas funcoes.
///
/// RED medido: em `fluxos::importar`, a conferencia da assinatura removida (o pacote com o
/// pin trocado e importado).
#[test]
fn cli_importar_exportar() {
    let raiz = tmp("pacote");
    let arq = gravar(
        &raiz,
        "origem",
        &json!({"nome":"origem","etiquetas":["vendas"],"variaveis":{"pais":"BR"},"passos":[
            {"id":"a","ferramenta":"eco","args":{"texto":"{{var.pais}}"}},
            {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}}!"}}
        ]}),
    );
    fluxos::pinar(&arq, "a", Some(json!(["PIN"]))).unwrap();
    let f = fluxos::ler_arquivo(&arq).unwrap();
    let pacote = fluxos::exportar(&f);
    assert_eq!(pacote["phxclaw_fluxo"], json!(fluxos::FORMATO_PACOTE));
    let f2 = fluxos::importar(&pacote.to_string()).unwrap();
    let destino = raiz.join("copia").join("importado.json");
    fluxos::gravar_importado(&f2, &destino).unwrap();
    assert!(
        fluxos::arquivo_de_pins(&destino).is_file(),
        "pins no .pins.json"
    );
    let lido = fluxos::ler_arquivo(&destino).unwrap();
    assert_eq!(fluxos::assinatura(&lido), fluxos::assinatura(&f));
    assert_eq!(lido.etiquetas, vec!["vendas".to_string()]);
    assert!(
        fluxos::gravar_importado(&f2, &destino)
            .unwrap_err()
            .contains("ja existe")
    );

    let mut torto = pacote.clone();
    torto["fluxo"]["passos"][0]["pin"] = json!(["OUTRO"]);
    assert!(
        fluxos::importar(&torto.to_string())
            .unwrap_err()
            .contains("conferencia")
    );
    let mut segredo = pacote.clone();
    segredo["fluxo"]["variaveis"]["api_key"] = json!("x");
    assert!(
        fluxos::importar(&segredo.to_string())
            .unwrap_err()
            .contains("segredo")
    );
    let mut futuro = pacote.clone();
    futuro["phxclaw_fluxo"] = json!(99);
    assert!(
        fluxos::importar(&futuro.to_string())
            .unwrap_err()
            .contains("formato 99")
    );
    assert!(
        fluxos::importar("{}")
            .unwrap_err()
            .contains("phxclaw_fluxo")
    );
}

/// `etiquetas_pastas`: `etiquetas` no fluxo e a subpasta de `fluxos/` como pasta; `listar`
/// filtra pelas duas, pula o `.pins.json` e devolve o arquivo invalido como erro em vez de
/// some-lo. Etiqueta fica FORA da assinatura (reetiquetar nao invalida a execucao
/// esperando), e etiqueta com espaco e recusada.
///
/// Reposta que derruba: em `fluxos::assinatura`, o `o.remove("etiquetas")` removido (o
/// hash muda com a etiqueta).
#[test]
fn etiquetas_pastas() {
    let dir = tmp("etiquetas");
    std::fs::create_dir_all(dir.join("vendas")).unwrap();
    let passos = json!([{"id":"a","ferramenta":"eco"}]);
    gravar(
        &dir,
        "a",
        &json!({"nome":"a","etiquetas":["vendas","diario"],"passos":passos}),
    );
    gravar(
        &dir.join("vendas"),
        "b",
        &json!({"nome":"b","etiquetas":["vendas"],"passos":passos}),
    );
    gravar(&dir, "c", &json!({"nome":"c","passos":passos}));
    std::fs::write(dir.join("ruim.json"), "{").unwrap();
    std::fs::write(dir.join("a.pins.json"), r#"{"a": 1}"#).unwrap();

    let nomes = |e: Option<&str>, p: Option<&str>| {
        let (v, _) = fluxos::listar(&dir, e, p);
        v.into_iter().map(|f| f.nome).collect::<Vec<_>>()
    };
    assert_eq!(nomes(Some("vendas"), None), vec!["a", "b"]);
    assert_eq!(nomes(Some("diario"), None), vec!["a"]);
    assert_eq!(nomes(None, Some("vendas")), vec!["b"]);
    assert_eq!(nomes(None, None), vec!["a", "c", "b"]);
    let (v, erros) = fluxos::listar(&dir, None, None);
    assert_eq!(v.iter().find(|f| f.nome == "b").unwrap().pasta, "vendas");
    assert_eq!(erros.len(), 1, "{erros:?}");
    assert!(erros[0].contains("ruim.json"));

    let mut f = fluxo(json!({"nome":"x","passos":passos}));
    let sem = fluxos::assinatura(&f);
    f.etiquetas = vec!["nova".into()];
    assert_eq!(fluxos::assinatura(&f), sem);
    assert!(
        fluxos::ler(&json!({"nome":"x","etiquetas":["com espaco"],"passos":passos}).to_string())
            .unwrap_err()
            .contains("etiqueta invalida")
    );
}

/// `concorrencia_limite`: com limite 1 na instancia, tres fluxos disparados juntos rodam um
/// de cada vez (o excedente espera a vaga, nao e recusado); sem limite, rodam juntos. O
/// sub-fluxo roda dentro da vaga de quem o chamou -- com limite 1, um fluxo que chama
/// outro termina em vez de travar.
///
/// RED medido: em `fluxos::executar`, `_vaga_da_instancia` sempre `None` (os tres
/// fluxos entram juntos e o maximo em voo vai a 3).
#[tokio::test]
async fn concorrencia_limite() {
    let raiz = tmp("limite");
    let pasta_fluxos = raiz.join("fluxos");
    std::fs::create_dir_all(&pasta_fluxos).unwrap();
    gravar(
        &pasta_fluxos,
        "filho",
        &json!({"nome":"filho","passos":[{"id":"x","ferramenta":"eco","args":{"texto":"filho"}}]}),
    );
    let b = banca_em(&raiz, Some(&pasta_fluxos));
    let f = fluxo(json!({"nome":"lento","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"x","dorme_ms":150}}
    ]}));
    let rodar_tres = |b: &Banca| {
        let hs: Vec<_> = (0..3)
            .map(|_| {
                let (a, f) = (b.a.clone(), f.clone());
                tokio::spawn(async move { fluxos::rodar(&a, &f).await.unwrap().sucesso })
            })
            .collect();
        async move {
            for h in hs {
                assert!(h.await.unwrap());
            }
        }
    };
    fluxos::definir_limite(b.a.store.root(), Some(1));
    rodar_tres(&b).await;
    assert_eq!(b.eco.max.load(Ordering::SeqCst), 1, "um fluxo por vez");

    // sub-fluxo dentro da vaga do pai: nao trava com limite 1
    let pai = fluxo(json!({"nome":"pai","passos":[
        {"id":"s","ferramenta":"fluxo","args":{"nome":"filho"}}
    ]}));
    let r = tokio::time::timeout(Duration::from_secs(10), fluxos::rodar(&b.a, &pai))
        .await
        .expect("limite 1 com sub-fluxo travou")
        .unwrap();
    assert!(r.sucesso, "{r:#?}");

    fluxos::definir_limite(b.a.store.root(), None);
    let b2 = banca_em(&raiz.join("sem-limite"), None);
    fluxos::definir_limite(b2.a.store.root(), None);
    rodar_tres(&b2).await;
    assert!(
        b2.eco.max.load(Ordering::SeqCst) >= 2,
        "sem limite, os fluxos entram juntos"
    );
}

// ======================================================================== velho

/// O comportamento VELHO: fluxo sem nenhum campo novo roda igual -- formato 2 no disco,
/// nenhum `fluxo.json`, `saidas/` ou `binarios/` criado, nenhuma chave nova no
/// `task.json`, e a assinatura de um fluxo da onda 2 continua a mesma (o canonico da onda 2
/// e o sha256 do texto ordenado e sem padroes). Item com `base64` SEM `mime` nao e
/// binario e passa como veio.
///
/// Reposta que derruba: qualquer campo novo do `Passo`/`Fluxo` sem `skip_serializing_if`
/// (o canonico ganha a chave e o hash muda).
#[tokio::test]
async fn comportamento_velho_sem_campos_novos() {
    let raiz = tmp("velho");
    let b = banca_em(&raiz, None);
    let f = fluxo(json!({"nome":"r","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"um"}},
        {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}} dois"}}
    ]}));
    let canonico = r#"{"nome":"r","passos":[{"args":{"texto":"um"},"ferramenta":"eco","id":"a"},{"args":{"texto":"{{a}} dois"},"depende":["a"],"ferramenta":"eco","id":"b"}]}"#;
    assert_eq!(fluxos::assinatura(&f), sha256(canonico.as_bytes()));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso);
    assert_eq!(r.formato, fluxos::FORMATO_RELATORIO);
    let disco = cru(&b.a, &r.tarefa);
    let v = relatorio_cru(&b.a, &r.tarefa);
    assert_eq!(v["formato"], json!(2));
    for chave in ["\"espera\"", "\"pinado\"", "\"externo\""] {
        assert!(!disco.contains(chave), "{chave}");
    }
    let dir = b.a.store.dir(&r.tarefa);
    assert!(!dir.join(fluxos::ARQUIVO_DEFINICAO).exists());
    assert!(!dir.join(fluxos::PASTA_SAIDAS).exists());
    assert!(!dir.join("work").join(fluxos::PASTA_BINARIOS).exists());
    let t = b.a.store.load(&r.tarefa).unwrap();
    assert_eq!(t.status, TaskStatus::Completed);
    assert!(t.question.is_none());

    let so_base64 = fluxo(json!({"nome":"s","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":{"base64":"aGVsbG8="}}}
    ]}));
    let r = fluxos::rodar(&b.a, &so_base64).await.unwrap();
    assert_eq!(passo(&r, "a").itens, vec![json!({"base64":"aGVsbG8="})]);
}

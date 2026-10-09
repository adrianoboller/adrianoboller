//! PHX Flow Engine, revisao da onda 3 (SP000035, 09/10/2026): os achados de QA, do DBA e de
//! seguranca sobre a onda 3, cada um com o teste que cai sem o conserto.
//!
//! Prova real: cada teste diz no comentario a linha que reposta o derruba. Os marcados
//! «RED medido» tiveram o defeito reposto de verdade (marcado `// REPOSTO`, recompilado,
//! visto cair pelo motivo certo) e o conserto voltou; os outros dizem a linha e nao foram
//! repostos um a um.

mod comum_fluxo;

use comum_fluxo::*;
use phxclaw_agent::api::{self, criar_fluxo_com};
use phxclaw_agent::canais::webhook::assinar;
use phxclaw_agent::fluxos::{self, Execucao, FalhaDaRetomada, Poda, Via};
use phxclaw_agent::gatilhos::{GatilhoDeWebhook, Gatilhos};
use phxclaw_agent::*;
use serde_json::{Value, json};
use std::time::Duration;

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ======================================================================== QA

/// `esperar.ms` enorme: antes, `ler` aceitava e a abertura da espera entrava em PANICO na
/// soma de data (o irmao do panico da `podar`), dentro do laco que retoma as esperas. Agora
/// passa do teto (`MAX_ESPERA_MS`, um ano e um dia) e recusa na LEITURA, com a mensagem que
/// manda usar `ate`; no teto, roda e o vencimento gravado e o de daqui a 366 dias.
///
/// RED medido: em `fluxos::validar_espera`, o `if let Some(ms) = e.ms.filter(|ms| *ms >
/// MAX_ESPERA_MS)` retirado (`ler` devolve Ok e a execucao entra em panico).
#[tokio::test]
async fn espera_ms_enorme_recusa_na_leitura_e_nao_entra_em_panico() {
    for ms in [fluxos::MAX_ESPERA_MS + 1, 9_000_000_000_000_000, u64::MAX] {
        let e = fluxos::ler(
            &json!({"nome":"x","passos":[{"id":"e","esperar":{"ms": ms}}]}).to_string(),
        )
        .unwrap_err();
        assert!(e.contains("teto") && e.contains("esperar.ate"), "{ms}: {e}");
    }
    let raiz = tmp("espera-teto");
    let b = banca_em(&raiz, None);
    let f = fluxo(json!({"nome":"x","passos":[
        {"id":"e","esperar":{"ms": fluxos::MAX_ESPERA_MS}}]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    let ate = passo(&r, "e").espera.as_ref().unwrap().ate.clone().unwrap();
    let ate = chrono::DateTime::parse_from_rfc3339(&ate).unwrap();
    let dias = (ate.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_days();
    assert!((365..=366).contains(&dias), "{dias}");
}

/// A entrada de fluxo que traz credencial (prefixo conhecido) e recusada em TODA porta, pelo mesmo
/// `fluxos::conferir_entrada`: o corpo JSON do webhook de gatilho (400, e nenhuma tarefa
/// nasce), o `criar_fluxo_com` que a agenda e o gatilho de arquivo usam, e a entrada do
/// sub-fluxo (ferramenta `fluxo`). Antes, as tres chegavam a `{{entrada}}` e ao `task.json`.
///
/// RED medido: as duas chamadas a `conferir_entrada` (em `api::criar_fluxo_com` e em
/// `fluxos::executar`) retiradas -- o webhook responde 202 e o sub-fluxo roda.
#[tokio::test]
async fn entrada_com_segredo_recusada_em_toda_porta() {
    let raiz = tmp("entrada-segredo");
    let s = estado(&raiz);
    let arq = gravar(
        &raiz,
        "f",
        &json!({"nome":"f","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{entrada}}"}}]}),
    );
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "g".into(),
            objetivo: String::new(),
            fluxo: Some(arq.to_string_lossy().into_owned()),
            segredo: None,
            segredo_formulario: None,
        }],
    };
    let base = servir(&s, g).await;
    let http = reqwest::Client::new();
    for corpo in [
        r#"{"nota":"ghp_0123456789abcdefABCDEF"}"#,
        r#"[{"ok":1},{"cabecalho":"Bearer sk-0123456789abcdef"}]"#,
        r#"{"url":"https://h/x?token=xoxb-1234567890-abcdef"}"#,
        r#"{"cfg":"token:ghp_0123456789abcdefABCDEF"}"#,
        r#"{"jwt":"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJh"}"#,
    ] {
        let r = http
            .post(format!("{base}/v1/triggers/g"))
            .bearer_auth(TOKEN)
            .body(corpo)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400, "{corpo}");
        let v: Value = r.json().await.unwrap();
        assert!(v["error"].as_str().unwrap().contains("segredo"), "{v}");
    }
    assert!(s.store.list().unwrap().is_empty(), "nenhuma tarefa nasceu");
    // O dado de terceiros que so tem NOME de segredo, ou forma parecida sem corpo, entra: o
    // evento do Jira (`key`), o do S3 (`key`), a paginacao (`next_page_token`), o push do
    // GitHub (SHA de 40 hex) e o locale eslovaco (`sk-SK`). RED medido em 09/10: com a guarda
    // por nome (`variavel_parece_segredo`), Jira, S3 e paginacao voltavam 400 (o SHA passava);
    // com o prefixo sem corpo (`carga::prefixo_de_credencial`), `sk-SK` voltava 400 e
    // `token:ghp_…` e o JWT passavam (202) -- os dois lados caem aqui.
    for corpo in [
        r#"{"issue":{"key":"PROJ-123"}}"#,
        r#"{"Records":[{"s3":{"object":{"key":"fotos/a.jpg"}}}]}"#,
        r#"{"next_page_token":"abc","api_key_hint":"use o cabecalho"}"#,
        r#"{"after":"9fceb02d0ae598e95dc970b74767f19372d61af8"}"#,
        r#"{"locale":"sk-SK","empresa":"sk-telecom"}"#,
    ] {
        let r = http
            .post(format!("{base}/v1/triggers/g"))
            .bearer_auth(TOKEN)
            .body(corpo)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 202, "corpo legitimo recusado: {corpo}");
    }
    // a porta da agenda e do gatilho de arquivo
    let e = criar_fluxo_com(
        &s,
        &arq.to_string_lossy(),
        vec![json!({"nota": "ghp_0123456789abcdefABCDEF"})],
        |_| Ok(()),
    )
    .err()
    .unwrap();
    assert_eq!(e.status.as_u16(), 400);
    // o sub-fluxo
    let pasta = raiz.join("fluxos");
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::copy(&arq, pasta.join("f.json")).unwrap();
    let b = banca_em(&raiz.join("sub"), Some(&pasta));
    let pai = fluxo(json!({"nome":"pai","passos":[
        {"id":"s","ferramenta":"fluxo","args":{"nome":"f","entrada":[{"nota":"xoxb-1234567890-abcdefghij"}]}}]}));
    let r = fluxos::rodar(&b.a, &pai).await.unwrap();
    assert_eq!(passo(&r, "s").estado, "falhou");
    assert!(passo(&r, "s").saida.contains("segredo"), "{r:#?}");
    assert_eq!(b.eco.n(), 0);
    // e o comportamento VELHO: entrada sem segredo continua entrando
    let r = http
        .post(format!("{base}/v1/triggers/g"))
        .bearer_auth(TOKEN)
        .body(r#"{"pedido":7}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
}

/// O detector de segredo e UM so: `gravacao::chave_secreta` delega a
/// `phxclaw_types::segredo::nome_de_segredo`, e os nomes que dividiam os dois motores em
/// 09/10 (`openai_key`, `access_key`, `key`) agora sao segredo para o fluxo.
#[test]
fn detector_de_segredo_unico_no_agente() {
    for n in ["openai_key", "access_key", "key", "bot-token", "x.api_key"] {
        assert!(gravacao::chave_secreta(n), "{n}");
        assert!(phxclaw_types::segredo::nome_de_segredo(n), "{n}");
    }
    assert!(!gravacao::chave_secreta("max_tokens"));
    let e = fluxos::ler(
        &json!({"nome":"x","variaveis":{"openai_key":"y"},"passos":[{"id":"a","ferramenta":"eco"}]})
            .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("parece segredo"), "{e}");
}

/// O `config.json` recusa segredo pela MESMA lista do agente: `carga::nome_de_segredo`
/// delega a `phxclaw_types::segredo::nome_de_segredo` (SP000035, item 3).
///
/// RED medido em 09/10, com a lista propria do `carga` ainda la: `credencial`,
/// `authorization`, `cookie`, `passwd`, `bearer` e `segredo` divergiam.
#[test]
fn config_json_usa_a_lista_unica_de_segredo() {
    use phxclaw_config_runtime::agente::carga;
    for n in [
        "credencial",
        "authorization",
        "cookie",
        "x.passwd",
        "bearer",
        "segredo",
    ] {
        assert_eq!(
            carga::nome_de_segredo(n),
            phxclaw_types::segredo::nome_de_segredo(n),
            "{n}"
        );
    }
}

/// A retomada que ja corre e um TIPO (`FalhaDaRetomada::JaEmCurso`), e quem decide se a
/// tarefa falhou (`api::retomar_fluxo`) decide por ele -- antes, pela frase «ja esta sendo
/// retomada». Duas retomadas juntas da mesma tarefa: uma roda, a outra volta `JaEmCurso`, e
/// a tarefa termina `Completed`.
///
/// RED medido: em `fluxos::retomar_do_disco`, o `JaEmCurso` trocado por
/// `Recusada(format!(...))` (o `matches!` abaixo cai).
#[tokio::test]
async fn retomada_em_curso_e_erro_tipado() {
    let raiz = tmp("retomada-tipada");
    let b = banca_em(&raiz, None);
    let f = fluxo(json!({"nome":"q","passos":[
        {"id":"ok","esperar":{"pergunta":"segue?"}},
        {"id":"b","depende":["ok"],"ferramenta":"eco","args":{"texto":"x","dorme_ms":200}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    fluxos::entregar(
        &b.a.store,
        &id,
        Via::Pergunta,
        None,
        vec![json!({"r":"sim"})],
    )
    .unwrap();
    let (um, dois) = tokio::join!(
        fluxos::retomar_do_disco(&b.a, &id),
        fluxos::retomar_do_disco(&b.a, &id)
    );
    let (ok, em_curso) = if um.is_ok() { (um, dois) } else { (dois, um) };
    assert!(ok.unwrap().sucesso);
    assert!(
        matches!(em_curso, Err(FalhaDaRetomada::JaEmCurso(ref t)) if *t == id),
        "{em_curso:?}"
    );
    assert_eq!(b.a.store.load(&id).unwrap().status, TaskStatus::Completed);
}

// ======================================================================== DBA

/// `entregar` recusa relatorio de formato futuro, como a retomada: regravar com o struct de
/// hoje jogaria fora o que este binario nao conhece. O `task.json` fica byte a byte.
///
/// RED medido: em `fluxos::entregar`, o `if r.formato > FORMATO_LIDO_MAX { ... }` retirado
/// (a entrega grava e o campo desconhecido some do disco).
#[tokio::test]
async fn entregar_recusa_formato_futuro() {
    let raiz = tmp("entregar-futuro");
    let b = banca_em(&raiz, None);
    let f = fluxo(json!({"nome":"q","passos":[{"id":"ok","esperar":{"pergunta":"segue?"}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let mut t = b.a.store.load(&id).unwrap();
    let mut r: Value = serde_json::from_str(t.answer.as_deref().unwrap()).unwrap();
    r["formato"] = json!(9);
    r["passos"][0]["campo_do_futuro"] = json!({"x": 1});
    t.answer = Some(r.to_string());
    b.a.store.save(&t).unwrap();
    let antes = cru(&b.a, &id);
    let e =
        fluxos::entregar(&b.a.store, &id, Via::Pergunta, None, vec![json!({"r": 1})]).unwrap_err();
    assert!(e.contains("formato 9"), "{e}");
    assert_eq!(cru(&b.a, &id), antes);
    // e o laco do servidor nao a toma por entrega sem retomada
    assert!(fluxos::entregas_sem_retomada(&b.a.store).is_empty());
}

/// A resposta foi GRAVADA (`entregar`) e o processo caiu antes do `retomar_do_disco`: a
/// tarefa fica `AwaitingInput` sem espera aberta, e nada a acordaria. O laco do servidor
/// (`api::manter_fluxos`, o de 20 s) a retoma junto das esperas de tempo vencidas -- e o
/// que acontece aqui, com um `ApiState` NOVO sobre a mesma pasta (o processo que subiu de
/// novo). A espera de tempo vencida segue pelo mesmo laco.
///
/// RED medido: em `api::manter_fluxos`, o laco sobre `fluxos::entregas_sem_retomada`
/// retirado (a tarefa fica `AwaitingInput`).
#[tokio::test]
async fn manter_fluxos_retoma_entrega_sem_retomada_e_tempo_vencido() {
    let raiz = tmp("manter");
    let s = estado(&raiz);
    let q = gravar(
        &raiz,
        "q",
        &json!({"nome":"q","passos":[
            {"id":"ok","esperar":{"pergunta":"segue?"}},
            {"id":"b","depende":["ok"],"ferramenta":"eco","args":{"texto":"r={{ok.resposta}}"}}]}),
    );
    let c = criar_fluxo_com(&s, &q.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    ate_o_estado(&s, &id, &[TaskStatus::AwaitingInput]).await;
    fluxos::entregar(
        &s.store,
        &id,
        Via::Pergunta,
        None,
        vec![json!({"resposta": "sim"})],
    )
    .unwrap();
    // a espera de tempo, ja vencida quando o laco passa
    let w = gravar(
        &raiz,
        "w",
        &json!({"nome":"w","passos":[
            {"id":"p","esperar":{"ms": 300}},
            {"id":"b","depende":["p"],"ferramenta":"eco","args":{"texto":"depois"}}]}),
    );
    let c = criar_fluxo_com(&s, &w.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let tempo = c.id.clone();
    c.fim.await.unwrap();
    ate_o_estado(&s, &tempo, &[TaskStatus::AwaitingInput]).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    drop(s);
    let s2 = estado(&raiz);
    for h in api::manter_fluxos(&s2) {
        h.await.unwrap();
    }
    let t = ate_o_estado(&s2, &id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert_eq!(passo(&relatorio(&t), "b").saida, "r=sim");
    let t = ate_o_estado(&s2, &tempo, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
}

/// O binario pinado: `saida_para_pin` devolve o item com o binario de volta em `{base64,
/// mime}` (conferido pelo sha256 da execucao de origem), e a execucao pinada o grava em
/// `binarios/` DELA -- o passo seguinte le o arquivo, e o `task.json` nao tem base64. Pin
/// com a referencia crua (`{"binario": ...}`, que apontaria para outra tarefa) e recusado.
///
/// RED medido: (a) em `fluxos::saida_para_pin`, o `reidratar_binarios(..)` trocado por
/// `p.itens` (o pin vira referencia e o `pinar` recusa); (b) no `executar`, o
/// `extrair_binarios` do caminho do pin retirado (`{{img.binario.caminho}}` nao resolve e
/// `le` falha).
#[tokio::test]
async fn pin_de_binario_volta_a_base64_e_grava_na_execucao_nova() {
    let raiz = tmp("pin-binario");
    let bytes = b"bytes do binario pinado";
    let arq = gravar(
        &raiz,
        "bin",
        &json!({"nome":"bin","passos":[
            {"id":"img","ferramenta":"eco",
             "args":{"texto":{"base64": b64(bytes), "mime":"image/png","nome":"f.png"}}},
            {"id":"le","depende":["img"],"ferramenta":"read_file",
             "args":{"path":"{{img.binario.caminho}}"}}]}),
    );
    let b = banca_em(&raiz, None);
    let f = fluxos::ler_arquivo(&arq).unwrap();
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    let pin = fluxos::saida_para_pin(&b.a.store, &r.tarefa, "img").unwrap();
    assert_eq!(pin[0]["base64"], json!(b64(bytes)));
    assert_eq!(pin[0]["mime"], json!("image/png"));
    assert_eq!(pin[0]["nome"], json!("f.png"));
    assert!(pin[0].get("binario").is_none());
    fluxos::pinar(&arq, "img", Some(pin)).unwrap();
    let f = fluxos::ler_arquivo(&arq).unwrap();
    let antes = b.eco.n();
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
    assert!(passo(&r, "img").pinado);
    assert_eq!(b.eco.n(), antes, "o img nao rodou");
    assert_eq!(passo(&r, "le").saida, "bytes do binario pinado");
    assert!(!cru(&b.a, &r.tarefa).contains(&b64(bytes)));
    // a referencia crua nao se pina
    let e = fluxos::pinar(
        &arq,
        "img",
        Some(json!([{"binario":{"caminho":"binarios/x.png","sha256":"ab"}}])),
    )
    .unwrap_err();
    assert!(e.contains("referencia a binario"), "{e}");
}

/// `gravar_importado` grava os pins ANTES do fluxo, e desfaz os pins se o fluxo falha: o
/// estado que nao pode existir e o fluxo no disco sem os pins que vieram no pacote.
/// Provado travando a gravacao dos pins (uma pasta no lugar do temporario deles).
///
/// RED medido: a ordem antiga (fluxo, depois pins) -- o fluxo fica no disco sem pins.
#[test]
fn gravar_importado_pins_antes_do_fluxo() {
    let raiz = tmp("importar-ordem");
    let mut f = fluxo(json!({"nome":"o","passos":[{"id":"a","ferramenta":"eco"}]}));
    f.passos[0].pin = Some(json!(["PIN"]));
    let destino = raiz.join("importado.json");
    let tmp_dos_pins = raiz.join(format!(".importado.pins.json.{}.tmp", std::process::id()));
    std::fs::create_dir_all(&tmp_dos_pins).unwrap();
    assert!(fluxos::gravar_importado(&f, &destino).is_err());
    assert!(!destino.exists(), "fluxo gravado sem os pins");
    std::fs::remove_dir_all(&tmp_dos_pins).unwrap();
    fluxos::gravar_importado(&f, &destino).unwrap();
    assert!(
        fluxos::ler_arquivo(&destino).unwrap().passos[0]
            .pin
            .is_some()
    );
}

/// A poda troca o nome da tarefa por uma lapide (`.<id>.podando`, atomico) antes de
/// apagar, e a poda seguinte varre a lapide que uma queda deixou no meio do
/// `remove_dir_all`. A lapide nunca aparece na listagem.
///
/// RED medido: em `fluxos::podar`, o `varrer_lapides(store, &mut feito)` retirado (a
/// lapide fica no disco).
#[tokio::test]
async fn poda_pela_metade_deixa_lapide_que_a_seguinte_varre() {
    let raiz = tmp("lapide");
    let b = banca_em(&raiz, None);
    let lapide = b.a.store.root().join(format!(
        ".0199aaaa-0000-7000-8000-000000000001{}",
        fluxos::SUFIXO_LAPIDE
    ));
    std::fs::create_dir_all(lapide.join("work")).unwrap();
    std::fs::write(lapide.join("task.json"), "{}").unwrap();
    assert!(b.a.store.list().unwrap().is_empty());
    let f =
        fluxo(json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let p = fluxos::podar(
        &b.a.store,
        Poda {
            dias: None,
            max: Some(1),
        },
        fluxos::agora(),
    );
    assert!(p.erros.is_empty(), "{:?}", p.erros);
    assert!(!lapide.exists(), "a lapide ficou");
    assert!(b.a.store.load(&id).is_ok(), "a terminada mais nova fica");
    // e a poda de verdade nao deixa lapide nem a tarefa
    let id2 = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let p = fluxos::podar(
        &b.a.store,
        Poda {
            dias: None,
            max: Some(1),
        },
        fluxos::agora(),
    );
    assert_eq!(p.removidas, vec![id.clone()]);
    assert!(b.a.store.load(&id2).is_ok());
    let sobras: Vec<String> = std::fs::read_dir(b.a.store.root())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(sobras.is_empty(), "{sobras:?}");
}

/// O que chega a uma espera com binario (`{"base64","mime"}`) vai para `binarios/` como o
/// de um passo: nunca base64 no `task.json`.
///
/// RED medido: em `fluxos::entregar`, o `extrair_binarios(..)` retirado (o base64 fica no
/// `task.json`).
#[tokio::test]
async fn entrega_com_binario_vai_para_binarios() {
    let raiz = tmp("entrega-bin");
    let b = banca_em(&raiz, None);
    let f = fluxo(json!({"nome":"w","passos":[
        {"id":"w","esperar":{"webhook":{}}},
        {"id":"le","depende":["w"],"ferramenta":"read_file","args":{"path":"{{w.binario.caminho}}"}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let bytes = b"anexo que chegou pelo webhook";
    fluxos::entregar(
        &b.a.store,
        &id,
        Via::Webhook,
        None,
        vec![json!({"base64": b64(bytes), "mime": "text/plain"})],
    )
    .unwrap();
    assert!(!cru(&b.a, &id).contains(&b64(bytes)));
    let r = fluxos::retomar_do_disco(&b.a, &id).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "le").saida, "anexo que chegou pelo webhook");
}

/// `listar` ordena as subpastas pelo nome: a ordem do `read_dir` e a do sistema de
/// arquivos (medido com oito pastas: beta, gama, teta, eta, zeta, delta, alfa, epsilon).
///
/// RED medido: em `fluxos::listar`, o `sub.sort_by(..)` retirado (sai na ordem do disco).
#[test]
fn listar_ordena_as_subpastas() {
    let dir = tmp("listar-ordem");
    let nomes = [
        "beta", "gama", "teta", "eta", "zeta", "delta", "alfa", "epsilon",
    ];
    for n in nomes {
        std::fs::create_dir_all(dir.join(n)).unwrap();
        gravar(
            &dir.join(n),
            "f",
            &json!({"nome": n, "passos":[{"id":"a","ferramenta":"eco"}]}),
        );
    }
    let (v, erros) = fluxos::listar(&dir, None, None);
    assert!(erros.is_empty(), "{erros:?}");
    let mut esperado: Vec<&str> = nomes.to_vec();
    esperado.sort();
    assert_eq!(
        v.iter().map(|f| f.pasta.as_str()).collect::<Vec<_>>(),
        esperado
    );
}

// ======================================================================== seguranca

/// M1: o pin so vale na execucao MANUAL que o pede (`Execucao::pins`, `fluxo rodar
/// --pins`). Gatilho, agenda, API e sub-fluxo (todos pelo `Execucao::default()`) rodam o
/// passo de verdade -- quem escreve o `ARQ.pins.json` nao troca a saida de um passo de
/// producao. A retomada pela CLI com o ARQ pinado de uma execucao sem pin continua
/// valendo (a assinatura gravada decide), e `esperar` pinado e recusado na leitura.
///
/// RED medido: em `fluxos::executar`, o `None if !pins => sem_pins...` trocado por
/// `None => fluxo` (o gatilho devolve o pin).
#[tokio::test]
async fn pin_so_vale_na_execucao_manual() {
    let raiz = tmp("pin-manual");
    let s = estado(&raiz);
    let arq = gravar(
        &raiz,
        "p",
        &json!({"nome":"p","passos":[
            {"id":"confere","ferramenta":"eco","args":{"texto":"de verdade"}},
            {"id":"usa","depende":["confere"],"ferramenta":"eco","args":{"texto":"{{confere}}!"}}]}),
    );
    fluxos::pinar(&arq, "confere", Some(json!("PINADO"))).unwrap();
    // gatilho/agenda: `criar_fluxo_com`
    let c = criar_fluxo_com(&s, &arq.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    let t = ate_o_estado(&s, &id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    let r = relatorio(&t);
    assert!(!passo(&r, "confere").pinado, "{r:#?}");
    assert_eq!(passo(&r, "usa").saida, "de verdade!");
    // `rodar` sem pedir: tambem nao
    let b = banca_em(&raiz.join("b"), None);
    let f = fluxos::ler_arquivo(&arq).unwrap();
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    assert_eq!(passo(&r, "usa").saida, "de verdade!");
    // a retomada com o ARQ pinado de uma execucao sem pin: a assinatura gravada decide
    let r2 = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap();
    assert!(r2.sucesso && passo(&r2, "usa").reaproveitado, "{r2:#?}");
    // manual, pedido: vale
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
    assert_eq!(passo(&r, "usa").saida, "PINADO!");
    // esperar nao se pina
    let e = fluxos::ler(
        &json!({"nome":"x","passos":[{"id":"e","esperar":{"pergunta":"ok?"},"pin":"sim"}]})
            .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("'esperar' nao aceita pin"), "{e}");
}

/// M5(c): duas esperas de webhook abertas (A e B, segredos diferentes). A autorizacao e a
/// entrega escolhem a MESMA espera: o segredo de A alcanca so A, e a entrega confere, sob
/// a trava, que A continua aberta. A intercalacao do achado -- os dois pedidos autorizam
/// contra A antes de qualquer entrega -- e reproduzida aqui sem depender da sorte do
/// agendador: a segunda entrega e recusada e B nunca recebe.
///
/// RED medido: em `fluxos::entregar`, o `&& passo.is_none_or(|x| x == p.id)` retirado (a
/// segunda entrega cai em B).
#[tokio::test]
async fn duas_esperas_o_segredo_de_a_nunca_entrega_em_b() {
    let raiz = tmp("m5");
    let b = banca_em(&raiz, None);
    let (sa, sb) = (
        "segredo-da-espera-A-1234567890",
        "segredo-da-espera-B-1234567890",
    );
    let f = fluxo(json!({"nome":"m5","passos":[
        {"id":"a","esperar":{"webhook":{"segredo_sha256": sha256(sa.as_bytes())}}},
        {"id":"b","esperar":{"webhook":{"segredo_sha256": sha256(sb.as_bytes())}}}]}));
    let id = fluxos::rodar(&b.a, &f).await.unwrap().tarefa;
    let t = b.a.store.load(&id).unwrap();
    assert_eq!(fluxos::esperas_abertas(&t).len(), 2);
    // os dois pedidos com o segredo de A autorizam ANTES de qualquer entrega
    let um = fluxos::espera_de_webhook(&t, None, Some(sa), false).unwrap();
    let dois = fluxos::espera_de_webhook(&t, None, Some(sa), false).unwrap();
    assert_eq!((um.as_str(), dois.as_str()), ("a", "a"));
    assert_eq!(
        fluxos::espera_de_webhook(&t, Some("b"), Some(sa), false),
        None
    );
    fluxos::entregar(
        &b.a.store,
        &id,
        Via::Webhook,
        Some(&um),
        vec![json!({"n":1})],
    )
    .unwrap();
    let e = fluxos::entregar(
        &b.a.store,
        &id,
        Via::Webhook,
        Some(&dois),
        vec![json!({"n":2})],
    )
    .unwrap_err();
    assert!(e.contains("nao esta mais esperando"), "{e}");
    let t = b.a.store.load(&id).unwrap();
    let r = relatorio(&t);
    assert_eq!(
        passo(&r, "b").estado,
        "esperando",
        "B recebeu com o segredo de A"
    );
}

/// M5(c) pelo soquete: dois POST SIMULTANEOS com o segredo de A, num servidor de varias
/// threads -- um entrega em A, o outro e recusado (409 ou 401), e B continua esperando.
/// Depois, o segredo de B entrega em B.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn duas_esperas_dois_post_simultaneos_pelo_soquete() {
    let raiz = tmp("m5-http");
    let s = estado(&raiz);
    let base = servir(&s, Gatilhos::default()).await;
    let (sa, sb) = (
        "segredo-da-espera-A-1234567890",
        "segredo-da-espera-B-1234567890",
    );
    let w = gravar(
        &raiz,
        "w",
        &json!({"nome":"w","passos":[
            {"id":"a","esperar":{"webhook":{"segredo_sha256": sha256(sa.as_bytes())}}},
            {"id":"b","esperar":{"webhook":{"segredo_sha256": sha256(sb.as_bytes())}}},
            {"id":"fim","depende":["a","b"],"ferramenta":"eco","args":{"texto":"{{a.n}}-{{b.n}}"}}]}),
    );
    let c = criar_fluxo_com(&s, &w.to_string_lossy(), vec![], |_| Ok(())).unwrap();
    let id = c.id.clone();
    c.fim.await.unwrap();
    ate_o_estado(&s, &id, &[TaskStatus::AwaitingInput]).await;
    let http = reqwest::Client::new();
    let post = |seg: &'static str, n: u8| {
        http.post(format!("{base}/v1/flows/{id}/resume"))
            .header("x-phxclaw-segredo", seg)
            .body(format!(r#"{{"n":{n}}}"#))
            .send()
    };
    let (r1, r2) = tokio::join!(post(sa, 1), post(sa, 2));
    let mut st = [r1.unwrap().status().as_u16(), r2.unwrap().status().as_u16()];
    st.sort();
    assert_eq!(st[0], 202, "{st:?}");
    assert!(matches!(st[1], 401 | 409), "{st:?}");
    // espera a retomada da entrega em A terminar (A volta `reaproveitado`), para o POST de
    // B achar a tarefa parada de novo e nao no meio da retomada
    let mut t = s.store.load(&id).unwrap();
    for _ in 0..400 {
        t = s.store.load(&id).unwrap();
        if t.status == TaskStatus::AwaitingInput
            && serde_json::from_str::<fluxos::Relatorio>(t.answer.as_deref().unwrap_or(""))
                .is_ok_and(|r| r.passos.iter().any(|p| p.id == "a" && p.reaproveitado))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(passo(&relatorio(&t), "a").estado, "ok");
    assert_eq!(passo(&relatorio(&t), "b").estado, "esperando");
    assert_eq!(post(sb, 3).await.unwrap().status(), 202);
    let t = ate_o_estado(&s, &id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    assert!(passo(&relatorio(&t), "fim").saida.ends_with("-3"));
}

/// M6: o codigo do formulario (`segredo_formulario`, o `_segredo` da pagina) so autoriza o
/// POST do FORMULARIO, com os campos conferidos; nunca o POST JSON nem a assinatura. E o
/// segredo do gatilho nao vale no campo `_segredo` (ele nao vai a humanos). Codigo igual ao
/// segredo e recusado ao carregar os gatilhos.
///
/// RED medido: no `gatilhos::portao`, o `_segredo` conferido contra `g.segredo` em vez de
/// `g.segredo_formulario` (o formulario com o segredo do gatilho passa: 202 onde se espera
/// 401).
#[tokio::test]
async fn codigo_do_formulario_nao_e_credencial_do_gatilho() {
    let raiz = tmp("m6");
    let s = estado(&raiz);
    let arq = gravar(
        &raiz,
        "c",
        &json!({"nome":"c","formulario":{"titulo":"T","campos":[{"nome":"nome"}]},
            "passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{entrada.nome}}"}}]}),
    );
    let (gat, cod) = (
        "segredo-do-gatilho-1234567890",
        "codigo-do-formulario-1234567890",
    );
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "c".into(),
            objetivo: String::new(),
            fluxo: Some(arq.to_string_lossy().into_owned()),
            segredo: Some(gat.into()),
            segredo_formulario: Some(cod.into()),
        }],
    };
    let base = servir(&s, g).await;
    let http = reqwest::Client::new();
    let form = |corpo: String| {
        http.post(format!("{base}/v1/triggers/c"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(corpo)
            .send()
    };
    assert_eq!(
        form(format!("_segredo={gat}&nome=Ana"))
            .await
            .unwrap()
            .status(),
        401,
        "o segredo do gatilho no campo do formulario"
    );
    let json_com_codigo = http
        .post(format!("{base}/v1/triggers/c"))
        .header("x-phxclaw-segredo", cod)
        .body(r#"{"nome":"qualquer coisa fora do formulario"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(
        json_com_codigo.status(),
        401,
        "o codigo como credencial JSON"
    );
    assert_eq!(s.store.list().unwrap().len(), 0);
    assert_eq!(
        form(format!("_segredo={cod}&nome=Ana"))
            .await
            .unwrap()
            .status(),
        202
    );
    // o gatilho continua aceitando a credencial dele no POST JSON
    let r = http
        .post(format!("{base}/v1/triggers/c"))
        .header("x-phxclaw-segredo", gat)
        .body(r#"{"nome":"Bia"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    // codigo igual ao segredo: recusado na carga
    let pasta = raiz.join(".phxclaw");
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(
        pasta.join("gatilhos.json"),
        json!({"webhooks":[{"nome":"x","fluxo": arq.to_string_lossy(),
            "segredo":"igual-1234567890","segredo_formulario":"igual-1234567890"}]})
        .to_string(),
    )
    .unwrap();
    let e = Gatilhos::carregar(&pasta).unwrap_err();
    assert!(e.contains("segredo_formulario igual"), "{e}");
}

/// B4: a pagina do formulario sai com a CSP (nada carrega; o estilo so pelo sha256 DO
/// bloco servido; envio so para a origem; sem moldura), sem cache, sem referer, com `lang`
/// e o codigo de acesso com `autocomplete="off"`. O hash da CSP e conferido contra o
/// `<style>` da propria pagina: editar o CSS sem a CSP acompanhar quebraria aqui. E a
/// recusa do POST pelo navegador volta o formulario em HTML com o que a pessoa digitou --
/// menos o codigo de acesso.
///
/// RED medido: em `gatilhos::formulario`, o `pagina(StatusCode::OK, ..)` trocado por
/// `Html(..).into_response()` (sem os cabecalhos).
#[tokio::test]
async fn formulario_com_csp_e_cabecalhos() {
    use base64::Engine as _;
    use sha2::Digest;
    let raiz = tmp("b4");
    let s = estado(&raiz);
    let arq = gravar(
        &raiz,
        "c",
        &json!({"nome":"c","formulario":{"titulo":"Contato","idioma":"en",
            "rotulo_segredo":"Access code","mensagem_enviado":"Got it.",
            "campos":[{"nome":"nome","obrigatorio":true},{"nome":"idade","tipo":"numero"}]},
            "passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{entrada.nome}}"}}]}),
    );
    let cod = "codigo-do-formulario-1234567890";
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "c".into(),
            objetivo: String::new(),
            fluxo: Some(arq.to_string_lossy().into_owned()),
            segredo: None,
            segredo_formulario: Some(cod.into()),
        }],
    };
    let base = servir(&s, g).await;
    let http = reqwest::Client::new();
    let r = http
        .get(format!("{base}/v1/triggers/c"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cab = |r: &reqwest::Response, k: &str| {
        r.headers()
            .get(k)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default()
    };
    let csp = cab(&r, "content-security-policy");
    for p in [
        "default-src 'none'",
        "form-action 'self'",
        "frame-ancestors 'none'",
        "style-src 'sha256-",
    ] {
        assert!(csp.contains(p), "{p} em {csp}");
    }
    assert_eq!(cab(&r, "cache-control"), "no-store");
    assert_eq!(cab(&r, "referrer-policy"), "no-referrer");
    let html = r.text().await.unwrap();
    assert!(html.contains("<html lang=\"en\">"), "{html}");
    assert!(html.contains("autocomplete=\"off\""));
    assert!(html.contains("Access code"));
    assert!(!html.to_lowercase().contains("<script"));
    let estilo = html
        .split("<style>")
        .nth(1)
        .and_then(|x| x.split("</style>").next())
        .unwrap();
    let hash = base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(estilo));
    assert!(csp.contains(&format!("'sha256-{hash}'")), "{csp}");
    // a recusa pelo navegador: o formulario de volta, com o valor e sem o codigo
    let r = http
        .post(format!("{base}/v1/triggers/c"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "text/html,application/xhtml+xml")
        .body(format!("_segredo={cod}&nome=Ana&idade=trinta"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    assert!(cab(&r, "content-security-policy").contains("default-src 'none'"));
    let html = r.text().await.unwrap();
    assert!(html.contains("role=\"alert\"") && html.contains("nao e numero"));
    assert!(html.contains("value=\"Ana\""));
    assert!(!html.contains(cod), "o codigo voltou na pagina");
    // a pagina de recebido: a frase do formulario e o protocolo
    let r = http
        .post(format!("{base}/v1/triggers/c"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("_segredo={cod}&nome=Ana"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(cab(&r, "cache-control"), "no-store");
    let html = r.text().await.unwrap();
    assert!(
        html.contains("Got it.") && html.contains("Protocolo:"),
        "{html}"
    );
}

/// B5: `/v1/flows/{tarefa}/resume` confere a credencial ANTES de ler a tarefa: sem token
/// nem segredo e 401 sem tocar o disco, e quem nao tem o token ouve 401 tambem para a
/// tarefa que nao existe (antes, 404 -- a rota dizia quais ids existem).
///
/// RED medido: em `gatilhos::retomar_espera`, o `if !pelo_token && segredo.is_none() {
/// return negado(); }` e o `negado()` da tarefa inexistente trocados pelo 404 de antes.
#[tokio::test]
async fn retomada_autentica_antes_de_ler_a_tarefa() {
    let raiz = tmp("b5");
    let s = estado(&raiz);
    let base = servir(&s, Gatilhos::default()).await;
    let http = reqwest::Client::new();
    let falsa = "0199aaaa-0000-7000-8000-00000000abcd";
    let url = format!("{base}/v1/flows/{falsa}/resume");
    assert_eq!(http.post(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        http.post(&url)
            .header("x-phxclaw-segredo", "chute")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        http.post(&url)
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

/// B2/B3: o binario conferido na retomada so e lido de `binarios/` da tarefa, se for
/// arquivo REGULAR (um FIFO nunca termina de ler e segurava a retomada para sempre) e no
/// teto de bytes; o nome leva o sha256 INTEIRO; e `binarios/` que e link simbolico nao
/// recebe nada (a gravacao iria para fora da pasta da tarefa).
///
/// RED medido: em `fluxos::ler_binario`, o `if !md.is_file()` retirado (a retomada fica
/// presa no FIFO e o `timeout` estoura).
#[tokio::test]
async fn binario_so_de_arquivo_regular_dentro_de_binarios() {
    let raiz = tmp("b2");
    let b = banca_em(&raiz, None);
    let bytes = b"um binario";
    let f = fluxo(json!({"nome":"b","passos":[
        {"id":"img","ferramenta":"eco","args":{"texto":{"base64": b64(bytes),"mime":"image/png"}}},
        {"id":"cai","depende":["img"],"ferramenta":"eco","args":{"falhar":"de proposito"}}]}));
    let r = fluxos::rodar(&b.a, &f).await.unwrap();
    let refe = passo(&r, "img").itens[0]["binario"].clone();
    let rel = refe["caminho"].as_str().unwrap().to_string();
    assert_eq!(
        rel,
        format!("binarios/{}.png", sha256(bytes)),
        "sha inteiro no nome"
    );
    let work = b.a.store.workdir(&r.tarefa);
    // FIFO no lugar do arquivo
    std::fs::remove_file(work.join(&rel)).unwrap();
    let ok = std::process::Command::new("mkfifo")
        .arg(work.join(&rel))
        .status()
        .unwrap();
    assert!(ok.success());
    // A retomada roda numa thread propria com prazo: a leitura de um FIFO bloqueia a thread
    // (o `timeout` do tokio nao interrompe leitura sincrona), e o teste tem de CAIR, nao
    // ficar preso junto.
    let (tx, rx) = std::sync::mpsc::channel();
    let (a, f2, t2) = (b.a.clone(), f.clone(), r.tarefa.clone());
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = tx.send(rt.block_on(fluxos::retomar(&a, &f2, &t2)));
    });
    let e = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a retomada ficou presa no FIFO")
        .unwrap_err();
    assert!(e.contains("nao e arquivo regular"), "{e}");
    // link simbolico no lugar do arquivo
    std::fs::remove_file(work.join(&rel)).unwrap();
    let fora = raiz.join("fora.png");
    std::fs::write(&fora, bytes).unwrap();
    std::os::unix::fs::symlink(&fora, work.join(&rel)).unwrap();
    let e = fluxos::retomar(&b.a, &f, &r.tarefa).await.unwrap_err();
    assert!(e.contains("nao e arquivo regular"), "{e}");
    // `binarios/` link para fora: nada se grava la
    let alvo = raiz.join("alvo-fora");
    std::fs::create_dir_all(&alvo).unwrap();
    let mae = fluxos::tarefa_do_fluxo(&f, "m");
    b.a.store.save(&mae).unwrap();
    std::os::unix::fs::symlink(&alvo, b.a.store.workdir(&mae.id).join("binarios")).unwrap();
    let r = fluxos::rodar_com(
        &b.a,
        &f,
        Execucao {
            mae: Some(mae),
            ..Execucao::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_dir(&alvo).unwrap().count(), 0, "gravou fora");
    assert!(passo(&r, "img").itens[0].get("binario").is_none());
}

/// A requisicao assinada do gatilho reenviada dentro da janela (a MESMA assinatura vale
/// 300 s para cada lado do carimbo) nao dispara de novo: a segunda volta 200 com o id do
/// primeiro disparo e `repetida`. A `Idempotency-Key` faz o mesmo para quem manda o
/// segredo em claro. Uma tarefa por disparo.
///
/// RED medido: em `gatilhos::disparar`, a `chave` forcada a `None` (dois fluxos).
#[tokio::test]
async fn gatilho_assinado_reenviado_nao_dispara_de_novo() {
    let raiz = tmp("repeticao");
    let s = estado(&raiz);
    let arq = gravar(
        &raiz,
        "f",
        &json!({"nome":"f","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}),
    );
    let seg = "segredo-do-gatilho-1234567890";
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "g".into(),
            objetivo: String::new(),
            fluxo: Some(arq.to_string_lossy().into_owned()),
            segredo: Some(seg.into()),
            segredo_formulario: None,
        }],
    };
    let base = servir(&s, g).await;
    let http = reqwest::Client::new();
    let corpo = r#"{"pedido":1}"#;
    let carimbo = chrono::Utc::now().timestamp();
    let assinatura = assinar(seg, carimbo, corpo.as_bytes());
    let mandar = || {
        http.post(format!("{base}/v1/triggers/g"))
            .header("x-phxclaw-carimbo", carimbo.to_string())
            .header("x-phxclaw-assinatura", &assinatura)
            .body(corpo)
            .send()
    };
    let r1 = mandar().await.unwrap();
    assert_eq!(r1.status(), 202);
    let id1 = r1.json::<Value>().await.unwrap()["id"].clone();
    let r2 = mandar().await.unwrap();
    assert_eq!(r2.status(), 200);
    let v2: Value = r2.json().await.unwrap();
    assert_eq!(v2["id"], id1);
    assert_eq!(v2["repetida"], json!(true));
    let fluxos_criados = || {
        s.store
            .list()
            .unwrap()
            .into_iter()
            .filter(|t| t.objective.starts_with(fluxos::PREFIXO_TAREFA))
            .count()
    };
    assert_eq!(fluxos_criados(), 1);
    // Idempotency-Key com o segredo em claro
    let com_chave = || {
        http.post(format!("{base}/v1/triggers/g"))
            .header("x-phxclaw-segredo", seg)
            .header("idempotency-key", "pedido-42")
            .body(r#"{"pedido":2}"#)
            .send()
    };
    assert_eq!(com_chave().await.unwrap().status(), 202);
    assert_eq!(com_chave().await.unwrap().status(), 200);
    assert_eq!(fluxos_criados(), 2);
    // sem chave e sem assinatura, o segredo em claro dispara cada vez (comportamento velho)
    for _ in 0..2 {
        let r = http
            .post(format!("{base}/v1/triggers/g"))
            .header("x-phxclaw-segredo", seg)
            .body(r#"{"pedido":3}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 202);
    }
    assert_eq!(fluxos_criados(), 4);
}

/// M4 (09/10/2026): a CHAVE de um objeto de fora tambem vai inteira para o `task.json`, e
/// o motor unico a julga pela mesma forma dos valores -- em qualquer profundidade. A chave
/// que so tem NOME de segredo (`key`, `api_key_hint`) continua entrando.
///
/// RED medido: o `texto_tem_credencial(k)` retirado de `valor_externo_parece_segredo`
/// (defeito reposto) -- as tres primeiras entradas passam.
#[test]
fn chave_de_objeto_com_forma_de_credencial_e_recusada() {
    for item in [
        json!({"sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAA": 1}),
        json!({"ok": {"fundo": [{"ghp_0123456789abcdefABCDEF0123": true}]}}),
        json!([{"Bearer abcdefghijklmnopqrstuvwxyz0123": "x"}]),
    ] {
        let e = fluxos::conferir_entrada(std::slice::from_ref(&item)).unwrap_err();
        assert!(e.contains("credencial"), "{item}: {e}");
    }
    for item in [
        json!({"key": "PROJ-1", "api_key_hint": "cabecalho"}),
        json!({"sk-SK": "locale", "9fceb02d0ae598e95dc970b74767f19372d61af8": 1}),
    ] {
        assert!(
            fluxos::conferir_entrada(std::slice::from_ref(&item)).is_ok(),
            "{item}"
        );
    }
}

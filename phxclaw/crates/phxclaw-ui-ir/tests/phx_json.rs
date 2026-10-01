//! PHX JSON do exemplo do dono (`fixtures/phx/app.phx.json`, 01/10/2026): le para o UI-IR,
//! escreve de volta byte a byte, recusa o que esta fora do subconjunto DIZENDO O CAMINHO, e
//! desenha pelos dois adaptadores com o mesmo motor.

use phxclaw_ui_ir::phx_json::{escrever, ler};
use phxclaw_ui_ir::*;

const APP: &str = include_str!("fixtures/phx/app.phx.json");

fn com(f: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(APP).unwrap();
    f(&mut v);
    serde_json::to_string_pretty(&v).unwrap()
}

#[test]
fn ida_e_volta_identica_byte_a_byte() {
    let (env, app) = ler(APP).unwrap();
    assert_eq!(escrever(&env, &app).unwrap(), APP);
    // e o UI-IR tambem faz ida e volta pelo leitor do IR (v3, conferido)
    let j = serde_json::to_string(&app).unwrap();
    assert_eq!(App::de_json(&j).unwrap(), app);
}

#[test]
fn uma_representacao_so_mudar_o_ir_muda_o_phx() {
    let (env, mut app) = ler(APP).unwrap();
    let Screen::Painel { colecao, .. } = &mut app.screens[0] else {
        panic!("painel")
    };
    colecao.layout.responsivo.as_mut().unwrap().regras[0].min_largura_px = 700;
    let saida = escrever(&env, &app).unwrap();
    assert!(saida.contains("\"minWidthPx\": 700"));
    assert!(!saida.contains("\"minWidthPx\": 640"));
}

#[test]
fn traduz_para_a_intencao_do_ir() {
    let (env, app) = ler(APP).unwrap();
    assert_eq!(env.versao_do_adaptador, "5.3.6");
    assert_eq!(app.quebra_da_casca_px, Some(992));
    assert_eq!(app.tokens["gap-md"], "16px");
    let Screen::Painel {
        conteiner, colecao, ..
    } = &app.screens[0]
    else {
        panic!("painel")
    };
    assert_eq!(conteiner, "agents-panel");
    let l = &colecao.layout;
    assert_eq!(
        (l.tipo, l.gap.as_str(), l.colunas),
        (TipoLayout::Grade, "md", 1)
    );
    let r = l.responsivo.as_ref().unwrap();
    assert_eq!(r.base, BaseResponsiva::Conteiner);
    assert_eq!(r.alvo.as_deref(), Some("agents-panel"));
    let regras: Vec<(u32, u32)> = r
        .regras
        .iter()
        .map(|g| (g.min_largura_px, g.colunas))
        .collect();
    assert_eq!(regras, [(640, 2), (1120, 4)]);
    assert_eq!(colecao.itens.len(), 4);
    // as colunas do exemplo (ate 639 -> 1, 640-1119 -> 2, >= 1120 -> 4), pela conta do motor
    for (w, c) in [
        (360.0, 1),
        (639.0, 1),
        (640.0, 2),
        (1119.0, 2),
        (1120.0, 4),
        (1180.0, 4),
    ] {
        assert_eq!(responsivo::colunas_em(l, w, 1920.0), c, "{w}px");
    }
}

#[test]
fn fora_do_subconjunto_recusa_com_o_caminho() {
    let casos: Vec<(String, &str)> = vec![
        (
            com(|v| v["view"]["children"][0]["layout"]["foo"] = 1.into()),
            "$.view.children[0].layout.foo: campo desconhecido",
        ),
        (
            com(|v| v["extra"] = true.into()),
            "$.extra: campo desconhecido",
        ),
        (
            com(|v| {
                v["ui"].as_object_mut().unwrap().remove("shellBreakpointPx");
            }),
            "$.ui.shellBreakpointPx: campo obrigatorio ausente",
        ),
        (
            com(|v| v["view"]["children"][0]["layout"]["columns"] = "2".into()),
            "$.view.children[0].layout.columns: esperava inteiro",
        ),
        (
            com(|v| v["agents"][1]["uuid"] = v["agents"][0]["uuid"].clone()),
            "$.agents[1].uuid: identidade",
        ),
        (
            com(|v| v["agents"][2]["uuid"] = "01a0f85a-635d-4495-9b3a-3761411c55dd".into()),
            "$.agents[2].uuid: «01a0f85a-635d-4495-9b3a-3761411c55dd» nao e UUIDv7",
        ),
        (
            com(|v| v["view"]["children"][0]["layout"]["type"] = "masonry".into()),
            "$.view.children[0].layout.type",
        ),
        (
            com(|v| v["view"]["children"][0]["layout"]["gap"] = "13px".into()),
            "$.view.children[0].layout.gap",
        ),
        (
            com(|v| v["view"]["children"][0]["layout"]["responsive"]["target"] = "outro".into()),
            "alvo Some(\"outro\") nao e conteiner ancestral",
        ),
        (
            com(|v| v["view"]["queryContainer"] = "a}body{x".into()),
            "conteiner «a}body{x» invalido",
        ),
        (
            com(|v| v["tokens"]["accent"] = "red;x:y".into()),
            "token «acento»",
        ),
        (
            com(|v| v["simulation"]["allowShell"] = true.into()),
            "$.simulation",
        ),
        (
            com(|v| v["schemaVersion"] = "9.0.0".into()),
            "$.schemaVersion",
        ),
    ];
    for (texto, esperado) in casos {
        let e = ler(&texto).expect_err(esperado);
        assert!(e.contains(esperado), "esperava «{esperado}», veio «{e}»");
    }
}

#[test]
fn os_dois_adaptadores_desenham_o_painel_pelo_mesmo_motor() {
    let (env, app) = ler(APP).unwrap();
    let css = responsivo::css(&app);
    assert!(css.contains(".phx-c-agents-panel{container:agents-panel/inline-size"));
    assert!(css.contains("@container agents-panel (min-width:640px)"));
    assert!(css.contains("@container agents-panel (min-width:1120px)"));
    assert!(
        css.contains("@media (min-width:992px){.phx-app"),
        "a casca muda na quebra do PHX"
    );
    let h = html::render(&app);
    let b = bootstrap::render_com_versao(
        &app,
        "vendor/bootstrap-5.3.6.min.css",
        &env.versao_do_adaptador,
    )
    .unwrap();
    for x in [&h, &b] {
        assert_eq!(x.matches("<article").count(), 4);
        assert!(x.contains("data-uuid=\"01a0f85a-635d-7553-8b0f-d5be7a41be81\""));
        assert!(x.contains("--phx-gap-md:16px"));
        // o acento do exemplo so no tema escuro: no claro o contraste dele cai abaixo de 3:1
        assert!(x.contains("@media (prefers-color-scheme: dark){:root{--phx-acento:#fa9866}}"));
        assert!(x.contains("<details><summary>Ver detalhes de Neo</summary>"));
    }
    assert!(
        b.contains("class=\"phx-cartao card card-body\"")
            && b.contains("data-phx-bootstrap=\"5.3.6\"")
    );
    assert!(!b.contains("col-") && !b.contains("\"row") && !b.contains("<script src"));
    assert!(bootstrap::render_com_versao(&app, "a.css", "5.3;x").is_err());
    // React e Flutter dizem que o painel ficou de fora, em vez de um componente vazio
    let r = react::render(&app);
    assert!(
        r.iter()
            .any(|(_, c)| c.contains("telas de painel nao desenhadas neste adaptador"))
    );
    assert!(
        flutter::render(&app)
            .iter()
            .any(|(_, c)| c.contains("telas de painel nao desenhadas"))
    );
}

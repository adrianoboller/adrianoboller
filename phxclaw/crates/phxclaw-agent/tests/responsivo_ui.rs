//! Phx Responsive UI no Chromium de verdade (pula com motivo sem Chromium): as telas do
//! gabarito nos adaptadores phoenix e Bootstrap, e o PHX JSON do dono nos dois. A corrida
//! inteira (20 telas, 14 larguras) e a do `phxclaw ui responsivo`; aqui vai um cadastro e um
//! documento, que cobrem secao, grade de itens, lookup e moeda.

use phxclaw_agent::responsivo_ui::{Alvo, medir, medir_phx};
use phxclaw_test_support::pulado;
use serde_json::Value;

const FIX: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phxclaw-ui-ir/tests/fixtures/phx/"
);

fn zero(v: &Value, k: &str) {
    for a in v["por_largura"].as_array().unwrap() {
        assert_eq!(a[k], 0, "{k} a {} px: {}", a["largura"], a["exemplos"]);
    }
}

fn confere_tudo(v: &Value) {
    for k in [
        "telas_com_rolagem",
        "fora_da_janela",
        "rolagem_interna",
        "texto_vazado",
        "sobrepostos",
        "tab_sem_anel",
        "saltos_visuais",
        "colunas_fora_do_ir",
    ] {
        zero(v, k);
    }
    for a in v["por_largura"].as_array().unwrap() {
        assert_eq!(a["tab_alcancados"], a["focaveis"], "Tab a {}", a["largura"]);
        assert_eq!(a["tab_tau"]["min"], 1.0);
        // alvo de 44 px cobrado de quem toca
        if a["toque"].as_bool() == Some(true) {
            assert_eq!(
                a["alvos_menores_que_44"], 0,
                "alvos a {}: {}",
                a["largura"], a["exemplos"]
            );
            assert_eq!(a["menu_alvos_menores_que_44"], 0, "menu a {}", a["largura"]);
        }
    }
    let c = &v["casos"];
    assert!(c["fronteiras_de_conteiner"]["conferidas"].as_u64().unwrap() > 0);
    assert_eq!(c["fronteiras_de_conteiner"]["divergentes"], 0, "{c}");
    assert_eq!(c["fronteiras_de_conteiner"]["sem_conteiner"], 0);
    assert!(c["fronteira_da_tabela"]["conferidas"].as_u64().unwrap() > 0);
    assert_eq!(c["fronteira_da_tabela"]["erradas"], 0);
    let p = &c["painel_360_em_1920"];
    assert_eq!(
        p["com_uma_coluna"], p["secoes"],
        "painel estreito compacto: {p}"
    );
    assert_eq!(p["em_cartoes"], p["tabelas"]);
    let r = &c["redimensionar"];
    assert_eq!(r["nos_novos"], 0);
    assert_eq!(r["nos_perdidos"], 0);
    assert_eq!(r["foco_preservado"], r["telas"]);
    assert_eq!(r["valor_preservado"], r["telas"]);
}

#[tokio::test]
async fn telas_do_gabarito_cabem_em_toda_largura_nos_dois_adaptadores() {
    if phxclaw_browser::find_chromium().is_none() {
        pulado::pular("chromium", "chromium ausente");
        return;
    }
    let so = ["cliente", "pedido"];
    let v = medir(&Alvo::Html, 20, &so, None).await.unwrap();
    confere_tudo(&v);
    let css = std::fs::read(format!("{FIX}bootstrap-5.3.6.min.css")).unwrap();
    let b = medir(&Alvo::Bootstrap { css }, 20, &so, None)
        .await
        .unwrap();
    confere_tudo(&b);
    let m = &b["mesma_estrutura_que_phoenix"];
    assert!(m["conferidas"].as_u64().unwrap() > 0);
    assert_eq!(m["diferentes"], 0, "{m}");
}

#[tokio::test]
async fn phx_json_do_dono_passa_nos_dois_adaptadores() {
    if phxclaw_browser::find_chromium().is_none() {
        pulado::pular("chromium", "chromium ausente");
        return;
    }
    let phx = std::fs::read_to_string(format!("{FIX}app.phx.json")).unwrap();
    let css = std::fs::read(format!("{FIX}bootstrap-5.3.6.min.css")).unwrap();
    let v = medir_phx(&phx, Some(css), None).await.unwrap();
    let falhas: Vec<&Value> = v["verificacoes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["passou"].as_bool() != Some(true))
        .collect();
    assert!(falhas.is_empty(), "{falhas:?}");
    assert_eq!(
        v["adaptadores"],
        serde_json::json!(["phoenix", "bootstrap"])
    );
    assert!(v["total"].as_u64().unwrap() >= 100);
}

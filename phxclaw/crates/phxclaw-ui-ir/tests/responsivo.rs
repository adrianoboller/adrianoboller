//! Phx Responsive UI, a parte pura: o motor compila a intencao do IR v3, os pontos de quebra
//! moram num JSON so, e os dois adaptadores (phoenix e Bootstrap) desenham a mesma arvore.
//! A prova no navegador (colunas medidas por largura, painel estreito, fronteiras,
//! redimensionar) e a de `phxclaw-agent/tests/responsivo_ui.rs`.

use phxclaw_ui_ir::responsivo::{self, bp};
use phxclaw_ui_ir::*;

const PEDIDOS: &str = include_str!("fixtures/pedidos.sql");

/// Toda sequencia de digitos de um texto, com a linha.
fn numeros(texto: &str) -> Vec<(usize, u32)> {
    let mut v = vec![];
    for (i, l) in texto.lines().enumerate() {
        for t in l.split(|c: char| !c.is_ascii_digit()) {
            if let Ok(n) = t.parse::<u32>() {
                v.push((i + 1, n));
            }
        }
    }
    v
}

#[test]
fn numeros_de_quebra_so_no_json() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let pontos: Vec<u32> = responsivo::breakpoints().iter().map(|(_, n)| *n).collect();
    assert_eq!(pontos.len(), 5, "sm md lg xl xxl");
    let mut arquivos: Vec<_> = std::fs::read_dir(raiz.join("src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    // a prova responsiva do agente escolhe larguras: e onde um numero solto tentaria mais
    arquivos.push(raiz.join("../phxclaw-agent/src/responsivo_ui.rs"));
    let mut achados = vec![];
    for a in &arquivos {
        let t = std::fs::read_to_string(a).unwrap();
        for (linha, n) in numeros(&t) {
            if pontos.contains(&n) {
                achados.push(format!("{}:{linha}: {n}", a.display()));
            }
        }
    }
    assert!(
        achados.is_empty(),
        "ponto de quebra escrito fora de breakpoints.json (use responsivo::bp): {achados:?}"
    );
}

#[test]
fn css_do_motor_sai_do_json_e_media_query_so_na_composicao() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let css = responsivo::css(&app);
    assert!(css.contains(&format!("@container tela (min-width:{}px)", bp("sm"))));
    assert!(css.contains(&format!("@container tela (min-width:{}px)", bp("lg"))));
    assert!(css.contains(&format!("(max-width:{}.98px)", bp("sm") - 1)));
    for (i, _) in css.match_indices("@media") {
        let regra = &css[i..css[i..].find("}}").map_or(css.len(), |j| i + j)];
        assert!(
            regra.contains(".phx-app") || regra.contains(".phx-menu-lista"),
            "media query fora da composicao: {regra}"
        );
    }
    assert!(css.contains("min-inline-size:0"));
    assert!(!css.contains("overflow:hidden;}") && !css.contains("body{overflow"));
}

#[test]
fn intencao_invalida_nao_vira_css() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let muda = |f: &dyn Fn(&mut Section)| {
        let mut a = app.clone();
        for t in a.screens.iter_mut() {
            if let Screen::Form { sections, .. } = t {
                f(&mut sections[0]);
            }
        }
        responsivo::validar(&a)
    };
    assert!(muda(&|_| {}).is_ok());
    // nome de conteiner vai para dentro da folha: texto livre seria injecao de CSS
    assert!(muda(&|s| s.conteiner = Some("secao}body{display:none".into())).is_err());
    assert!(muda(&|s| s.conteiner = Some("tabela".into())).is_err());
    assert!(muda(&|s| s.layout.as_mut().unwrap().gap = "13px".into()).is_err());
    assert!(muda(&|s| s.layout.as_mut().unwrap().colunas = 0).is_err());
    assert!(
        muda(&|s| {
            let r = s.layout.as_mut().unwrap().responsivo.as_mut().unwrap();
            r.alvo = Some("outro".into());
        })
        .is_err(),
        "alvo que nao e conteiner ancestral"
    );
    assert!(
        muda(&|s| {
            let r = s.layout.as_mut().unwrap().responsivo.as_mut().unwrap();
            r.regras.reverse();
        })
        .is_err(),
        "regras fora de ordem"
    );
    // e o leitor do JSON passa pela mesma conferencia
    let mut v: serde_json::Value = serde_json::to_value(&app).unwrap();
    v["screens"][1]["sections"][0]["conteiner"] = "a b".into();
    assert!(App::de_json(&v.to_string()).is_err());
}

#[test]
fn intencao_declarada_muda_a_grade_e_viaja_pelo_com_layout() {
    let (mut app, _) = from_sql("Vendas", PEDIDOS);
    let Screen::Form { sections, .. } = app
        .screens
        .iter_mut()
        .find(|s| matches!(s, Screen::Form { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    sections[0].layout = Some(LayoutIntencao {
        tipo: TipoLayout::Grade,
        gap: "lg".into(),
        colunas: 2,
        responsivo: Some(Responsivo {
            base: BaseResponsiva::Conteiner,
            alvo: Some("painel".into()),
            regras: vec![RegraResponsiva {
                min_largura_px: bp("xl"),
                colunas: 6,
            }],
        }),
    });
    sections[0].conteiner = Some("painel".into());
    responsivo::validar(&app).unwrap();
    let css = responsivo::css(&app);
    assert!(css.contains(".phx-c-painel{container:painel/inline-size"));
    assert!(css.contains(&format!(
        "@container painel (min-width:{}px){{.phx-grade-lg-2-cpainel{}x6{{grid-template-columns:repeat(6,minmax(0,1fr))}}}}",
        bp("xl"),
        bp("xl")
    )));
    let h = html::render(&app);
    assert!(h.contains(&format!("class=\"phx-grade-lg-2-cpainel{}x6\"", bp("xl"))));
    // com_layout nao perde a intencao
    assert_eq!(app.com_layout(), app);
    let j = serde_json::to_string(&app).unwrap();
    assert!(j.contains("\"tipo\":\"grade\"") && j.contains("\"min_largura_px\""));
    assert_eq!(App::de_json(&j).unwrap(), app);
}

/// Sequencia de `id`s e de classes do motor (`phx-*`) na ordem do documento.
fn arvore(h: &str) -> (Vec<String>, Vec<String>) {
    let ids = h
        .match_indices(" id=\"")
        .map(|(i, _)| {
            let r = &h[i + 5..];
            r[..r.find('"').unwrap()].to_string()
        })
        .collect();
    let phx = h
        .match_indices("class=\"")
        .flat_map(|(i, _)| {
            let r = &h[i + 7..];
            r[..r.find('"').unwrap()]
                .split(' ')
                .filter(|c| c.starts_with("phx-"))
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect();
    (ids, phx)
}

#[test]
fn bootstrap_gera_componentes_sem_grade_de_framework_e_com_a_mesma_arvore() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let b = bootstrap::render(&app, bootstrap::CSS_PADRAO).unwrap();
    for c in [
        "btn btn-outline-success",
        "btn btn-outline-warning",
        "btn btn-outline-danger",
        "btn btn-outline-primary",
        "form-control",
        "form-select",
        "form-label",
        "input-group",
        "card card-body",
        "class=\"itens table\"",
        "nav-link",
        "data-phx-bootstrap=\"5.3.3\"",
    ] {
        assert!(b.contains(c), "falta {c}");
    }
    // a grade e do motor: nenhuma classe de layout do Bootstrap, classe a classe (procurar
    // texto deixaria passar "card row" e reprovaria "phx-c-tela" por conter "container")
    let classes: Vec<&str> = b
        .match_indices("class=\"")
        .flat_map(|(i, _)| {
            let r = &b[i + 7..];
            r[..r.find('"').unwrap()].split_whitespace()
        })
        .collect();
    let layout = |c: &str| {
        ["row", "container", "container-fluid", "btn-sm"].contains(&c)
            || c.starts_with("col")
            || c.starts_with("row-")
            || c.starts_with("d-")
            || c.starts_with("flex-")
            || c.starts_with("justify-")
            || c.starts_with("align-")
            || c.starts_with("g-")
    };
    let achadas: Vec<&&str> = classes.iter().filter(|c| layout(c)).collect();
    assert!(
        achadas.is_empty(),
        "classe de layout do Bootstrap: {achadas:?}"
    );
    assert!(!b.contains("<script src"), "sem o JS do Bootstrap");
    assert!(b.contains(&format!("href=\"{}\"", bootstrap::CSS_PADRAO)));
    // mesma arvore do adaptador phoenix: mesmos ids na mesma ordem (a tabulacao e a do DOM)
    // e as mesmas classes do motor nos mesmos lugares
    let h = html::render(&app);
    assert_eq!(arvore(&b), arvore(&h));
    // a linha da grade de itens vira cartao em conteiner estreito: cada celula leva o rotulo
    for x in [&h, &b] {
        for r in [
            "Produto",
            "Quantidade",
            "Preço unitário",
            "Valor total",
            "Ações",
        ] {
            assert!(
                x.contains(&format!("data-rotulo=&quot;{r}&quot;")),
                "celula sem rotulo: {r}"
            );
        }
    }
    assert!(!arvore(&h).1.is_empty());
}

#[test]
fn bootstrap_recusa_cdn_e_atributo_quebrado() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    for ruim in [
        "https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css",
        "//cdn.example/b.css",
        "http:b.css",
        "a.css\" onload=\"x",
        "",
    ] {
        assert!(bootstrap::render(&app, ruim).is_err(), "aceitou {ruim}");
    }
    for bom in [
        "bootstrap.min.css",
        "../vendor/bootstrap-5.3.3/bootstrap.min.css",
    ] {
        assert!(bootstrap::render(&app, bom).is_ok(), "recusou {bom}");
    }
}

#[test]
fn react_le_as_classes_do_motor_sem_refazer_a_conta() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let r = react::render(&app);
    let telas = &r.iter().find(|(p, _)| p == "src/telas.jsx").unwrap().1;
    let ui = &r.iter().find(|(p, _)| p == "src/ui.jsx").unwrap().1;
    let index = &r.iter().find(|(p, _)| p == "index.html").unwrap().1;
    let (c, l) = responsivo::classes_da_secao(&Section {
        title: String::new(),
        fields: vec![],
        layout: None,
        conteiner: None,
    });
    assert!(telas.contains(&format!("\"_conteiner\":\"{c}\"")));
    assert!(telas.contains(&format!("\"_layout\":\"{l}\"")));
    assert!(telas.contains("\"_largo\":true"));
    assert!(
        !ui.contains("max_len || 0) > 80"),
        "a conta do campo largo e do motor"
    );
    assert!(index.contains(&responsivo::css(&app)) && index.contains("class=\"phx-app\""));
}

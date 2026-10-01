//! Layout pelas caixas do OCR (SP000021), UI-IR v2 e a comparacao da prova de fidelidade
//! (SP000022). O TSV do pedido e saida REAL do tesseract (por+eng, psm 11) sobre a tela
//! gerada aqui, em 1280x1000, medida em 01/10/2026.

use phxclaw_ui_ir::fidelidade::{self, kendall_tau};
use phxclaw_ui_ir::layout::{self, Palavra, Papel};
use phxclaw_ui_ir::*;

const TSV_PEDIDO: &str = include_str!("fixtures/pedido_ocr.tsv");
const PEDIDOS: &str = include_str!("fixtures/pedidos.sql");

fn lido() -> layout::Layout {
    let (p, w, h) = layout::ler_tsv(TSV_PEDIDO);
    layout::analisar(&p, w, h)
}

fn textos(v: &[imagem::Rotulo]) -> Vec<String> {
    v.iter().map(|r| r.texto.clone()).collect()
}

#[test]
fn tsv_real_vira_rotulos_na_ordem_de_tabulacao_sem_menu_titulo_nem_botao() {
    let (p, w, h) = layout::ler_tsv(TSV_PEDIDO);
    assert_eq!((w, h), (1280, 1000), "tamanho da pagina vem do nivel 1");
    assert!(p.len() > 50);
    let l = layout::analisar(&p, w, h);
    assert_eq!(
        textos(&l.campos()),
        [
            "Código",
            "Cliente",
            "Data emissão",
            "Situação",
            "Valor frete",
            "Observação",
            "Produto",
            "Quantidade",
            "Preço unit rio",
            "Valor total",
            "Total"
        ],
        "secao a secao, fila a fila; menu, titulo e botoes fora"
    );
    let papel = |t: &str| l.frases.iter().find(|f| f.texto == t).map(|f| f.papel);
    assert_eq!(papel("Consulta de clientes"), Some(Papel::Menu));
    assert!(
        l.frases
            .iter()
            .any(|f| f.texto == "Pedido" && f.papel == Papel::Titulo),
        "o \"Pedido\" do conteudo e titulo; o do menu e menu"
    );
    assert_eq!(papel("Dados principais"), Some(Papel::Secao));
    assert_eq!(papel("Itens"), Some(Papel::Secao));
    assert_eq!(papel("Selecione cliente"), Some(Papel::Exemplo));
    assert_eq!(papel("imprimir"), Some(Papel::Acao));
    let c = l.campos();
    let obrig: Vec<&str> = c
        .iter()
        .filter(|r| r.obrigatorio)
        .map(|r| r.texto.as_str())
        .collect();
    assert_eq!(obrig, ["Cliente", "Data emissão", "Situação"], "o * lido");
    let listas: Vec<&str> = c
        .iter()
        .filter(|r| r.lista)
        .map(|r| r.texto.as_str())
        .collect();
    assert_eq!(
        listas,
        ["Cliente", "Situação"],
        "exemplo \"Selecione\" abaixo"
    );
    // o grupo de cada rotulo e a secao acima dele
    let grupo = |t: &str| {
        let f = l.frases.iter().find(|f| f.texto == t).unwrap();
        l.grupos[f.grupo.unwrap()].titulo.clone()
    };
    assert_eq!(grupo("Situação"), "Dados principais");
    assert_eq!(grupo("Valor frete"), "Valores");
    assert_eq!(grupo("Observação"), "Observações");
}

#[test]
fn linhas_do_tsv_sao_as_do_tesseract() {
    let (p, _, _) = layout::ler_tsv(TSV_PEDIDO);
    let l = layout::linhas(&p);
    assert!(l.contains(&"Data emissão *".to_string()), "{l:?}");
    assert!(l.contains(&"Dados principais".to_string()));
}

fn pal(t: &str, x: i32, y: i32) -> Palavra {
    Palavra {
        texto: t.into(),
        x,
        y,
        w: 9 * t.chars().count() as i32,
        h: 11,
        linha: (0, 0, 0),
    }
}

/// Grade sintetica: legenda, cabecalho e N filas de exemplo alinhadas abaixo.
fn grade(filas: usize) -> Vec<Palavra> {
    let mut v = vec![
        pal("Itens", 290, 100),
        pal("Produto", 294, 140),
        pal("Quantidade", 470, 140),
        pal("Valor", 980, 140),
    ];
    for k in 0..filas as i32 {
        let y = 180 + 50 * k;
        v.push(pal("Selecione", 300, y));
        v.push(pal("0,00", 990, y));
    }
    v
}

#[test]
fn duas_filas_alinhadas_fazem_grade_e_uma_so_nao() {
    let l = layout::analisar(&grade(2), 1280, 800);
    assert_eq!(textos(&l.colunas()), ["Produto", "Quantidade", "Valor"]);
    assert!(l.campos().is_empty());
    let g = &l.grupos[l
        .frases
        .iter()
        .find(|f| f.texto == "Produto")
        .unwrap()
        .grupo
        .unwrap()];
    assert!(g.grade);
    assert_eq!(
        g.titulo, "Itens",
        "a legenda colada acima e o titulo da grade"
    );
    // um formulario tem UMA fila de exemplo abaixo dos rotulos: nao e grade
    let l = layout::analisar(&grade(1), 1280, 800);
    assert!(l.colunas().is_empty());
    assert_eq!(textos(&l.campos()), ["Produto", "Quantidade", "Valor"]);
}

#[test]
fn total_abaixo_da_grade_nao_e_campo_e_fila_de_botoes_nao_e_dado() {
    let mut v = grade(2);
    v.push(pal("Total", 1100, 300));
    v.push(pal("R$ 0,00", 1060, 320));
    let l = layout::analisar(&v, 1280, 800);
    assert!(l.campos().is_empty(), "{:?}", textos(&l.campos()));
    assert_eq!(
        l.frases.iter().find(|f| f.texto == "Total").unwrap().papel,
        Papel::Total
    );
    // secao de duas datas: rotulos, exemplos, e a fila de botoes com "Pesquisar" (que
    // tambem e exemplo) logo abaixo: nao e grade (medido no contrato, 01/10)
    let v = vec![
        pal("Controle", 290, 100),
        pal("Criado em", 294, 130),
        pal("Alterado em", 530, 130),
        pal("dd/mm/aaaa", 300, 165),
        pal("dd/mm/aaaa", 540, 165),
        pal("Salvar", 290, 230),
        pal("Pesquisar", 530, 230),
    ];
    let l = layout::analisar(&v, 1280, 800);
    assert!(l.colunas().is_empty());
    assert_eq!(textos(&l.campos()), ["Criado em", "Alterado em"]);
}

#[test]
fn coluna_do_modelo_so_vale_se_o_layout_ve_grade() {
    let r = |t: &str| imagem::Rotulo {
        texto: t.into(),
        obrigatorio: false,
        lista: false,
    };
    // cadastro sem grade: o modelo respondeu os rotulos do formulario como colunas
    assert!(
        lido()
            .confirmar_colunas(&[r("Código"), r("Cliente")])
            .is_empty()
    );
    let l = layout::analisar(&grade(2), 1280, 800);
    assert_eq!(
        textos(&l.confirmar_colunas(&[r("Produto"), r("Valor"), r("Itens")])),
        ["Produto", "Valor"]
    );
}

#[test]
fn tabulacao_segue_secao_a_secao_e_leitura_segue_a_fila() {
    // duas secoes lado a lado: le-se Nome, Email, Apelido, Telefone; tabula-se a secao
    // da esquerda inteira antes da direita, como o DOM de cada fieldset
    let v = vec![
        pal("Pessoal", 290, 100),
        pal("Contato", 700, 100),
        pal("Nome", 294, 130),
        pal("Email", 704, 130),
        pal("Apelido", 294, 200),
        pal("Telefone", 704, 200),
    ];
    let l = layout::analisar(&v, 1280, 800);
    assert_eq!(
        textos(&l.campos()),
        ["Nome", "Apelido", "Email", "Telefone"]
    );
    let leitura = |t: &str| l.frases.iter().find(|f| f.texto == t).unwrap().leitura;
    assert!(leitura("Email") < leitura("Apelido"));
    let grupo = |t: &str| {
        let f = l.frases.iter().find(|f| f.texto == t).unwrap();
        l.grupos[f.grupo.unwrap()].titulo.clone()
    };
    assert_eq!(grupo("Telefone"), "Contato");
    assert_eq!(grupo("Apelido"), "Pessoal");
}

#[test]
fn para_ir_grava_caixa_relativa_grupo_e_ordem() {
    let l = lido();
    let i = l.achar("Data emissão").unwrap();
    let j = l.achar("Código").unwrap();
    let s = l.para_ir(
        "pedido_documento",
        &[
            (i, "pedido".into(), "data_emissao".into()),
            (j, "pedido".into(), "id".into()),
        ],
    );
    assert_eq!(s.items[0].field, "id", "Codigo vem antes na tabulacao");
    assert_eq!(s.items[1].tab_order, 1);
    let b = s.items[1].bbox;
    assert!((b.x - 768.0 / 1280.0).abs() < 1e-3 && (b.y - 103.0 / 1000.0).abs() < 1e-3);
    assert_eq!(s.groups.len(), 1);
    assert_eq!(s.groups[0].title, "Dados principais");
    assert_eq!(s.groups[0].kind, "section");
}

#[test]
fn ui_ir_v1_continua_lendo_e_versao_do_futuro_se_recusa() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let mut v: serde_json::Value = serde_json::to_value(&app).unwrap();
    assert!(v.get("layouts").is_none(), "sem layout, o JSON sai como v1");
    v["ir_version"] = 1.into();
    let velho = App::de_json(&v.to_string()).unwrap();
    assert!(velho.layouts.is_empty());
    v["ir_version"] = (IR_VERSION + 1).into();
    assert!(App::de_json(&v.to_string()).is_err());
}

fn com_layout_invertido() -> App {
    let (mut app, _) = from_sql("Vendas", PEDIDOS);
    // lido: situacao primeiro, depois cliente_id, num grupo so chamado "Cabecalho"
    let item = |f: &str, t: u32| LayoutItem {
        entity: "pedido".into(),
        field: f.into(),
        group: "g1".into(),
        bbox: Caixa {
            x: 0.1,
            y: 0.1,
            w: 0.1,
            h: 0.01,
        },
        read_order: t,
        tab_order: t,
    };
    let col = |f: &str, t: u32| LayoutItem {
        entity: "pedido_item".into(),
        group: "g2".into(),
        ..item(f, t)
    };
    app.layouts.push(ScreenLayout {
        screen: "pedido_documento".into(),
        groups: vec![
            LayoutGroup {
                id: "g1".into(),
                title: "Cabecalho".into(),
                kind: "section".into(),
                bbox: item("x", 0).bbox,
            },
            LayoutGroup {
                id: "g2".into(),
                title: "Itens".into(),
                kind: "grid".into(),
                bbox: item("x", 0).bbox,
            },
        ],
        items: vec![
            item("situacao", 0),
            item("cliente_id", 1),
            col("vl_total", 2),
            col("produto_id", 3),
        ],
    });
    app
}

#[test]
fn com_layout_reordena_secoes_colunas_e_campos_e_e_idempotente() {
    let app = com_layout_invertido();
    let a = app.com_layout();
    let Screen::MasterDetail {
        header,
        detail_columns,
        ..
    } = a
        .screens
        .iter()
        .find(|s| s.id() == "pedido_documento")
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(header[0].title, "Cabecalho");
    assert_eq!(header[0].fields, ["situacao", "cliente_id"]);
    assert!(
        header
            .iter()
            .any(|s| s.title == "Dados principais" && s.fields.contains(&"dt_emissao".into())),
        "o que nao foi lido fica na secao de origem: {header:?}"
    );
    assert_eq!(&detail_columns[..2], ["vl_total", "produto_id"]);
    let ped = a.entities.iter().find(|e| e.name == "pedido").unwrap();
    assert_eq!(ped.fields[0].name, "situacao");
    assert_eq!(a.com_layout(), a, "aplicar duas vezes da o mesmo");
    // sem layout, nada muda
    let (puro, _) = from_sql("Vendas", PEDIDOS);
    assert_eq!(puro.com_layout(), puro);
}

#[test]
fn renderizadores_respeitam_a_ordem_lida() {
    let app = com_layout_invertido();
    let h = html::render(&app);
    let pos = |s: &str| h.find(s).unwrap_or_else(|| panic!("{s} ausente"));
    assert!(
        pos("for=\"pedido_documento-situacao\"") < pos("for=\"pedido_documento-cliente_id\""),
        "HTML na ordem lida"
    );
    assert!(h.contains("<legend>Cabecalho</legend>"));
    let r = react::render(&app);
    let j = &r.iter().find(|(p, _)| p == "src/telas.jsx").unwrap().1;
    assert!(
        j.contains(r#""fields":["situacao","cliente_id"]"#),
        "React le as secoes ja reordenadas"
    );
    assert!(
        flutter::render(&app)
            .iter()
            .any(|(_, c)| c.contains(r#""fields":["situacao","cliente_id"]"#)),
        "Flutter le as secoes ja reordenadas"
    );
    let wl = &wlanguage::render(&app)[0].1;
    assert!(
        wl.find("pedido.situacao").unwrap() < wl.find("pedido.cliente_id").unwrap(),
        "regras validam na ordem de tabulacao"
    );
    let rs = &rust::render(&app);
    let lib = &rs.iter().find(|(p, _)| p.ends_with("lib.rs")).unwrap().1;
    assert!(lib.find("situacao").unwrap() < lib.find("cliente_id").unwrap());
}

#[test]
fn kendall_tau_conhecido() {
    assert_eq!(kendall_tau(&[0, 1, 2, 3], &[0, 1, 2, 3]), Some(1.0));
    assert_eq!(kendall_tau(&[0, 1, 2, 3], &[3, 2, 1, 0]), Some(-1.0));
    // uma troca adjacente em 4: 5 concordantes, 1 discordante -> 4/6
    assert_eq!(kendall_tau(&[0, 1, 2, 3], &[1, 0, 2, 3]), Some(4.0 / 6.0));
    assert_eq!(kendall_tau(&[0], &[0]), None);
}

#[test]
fn comparar_acusa_perdido_inventado_tipo_e_ordem() {
    let (origem, _) = from_sql("Vendas", PEDIDOS);
    let tela = fidelidade::tela_de(&origem, "pedido").unwrap();
    let m = fidelidade::comparar(&origem, &tela, "mestre_detalhe", &[], &origem, "pedido");
    assert_eq!(m.achados, m.origem, "a tela contra ela mesma acha tudo");
    assert!(m.inventados.is_empty() && m.perdidos.is_empty());
    assert_eq!(m.kendall_tau, Some(1.0));
    assert_eq!(m.grupo_rand, Some(1.0));
    assert_eq!(m.tipo_igual, m.achados);
    assert!(
        !fidelidade::campos_da_tela(&origem, &tela)
            .iter()
            .any(|c| c.campo == "id"),
        "chave fora da conta"
    );
    // convertida: perdeu o frete, inventou um campo, cliente virou texto, ordem lida invertida
    let conv_sql = "CREATE TABLE pedido (id serial PRIMARY KEY, situacao varchar(40) NOT NULL, \
cliente varchar(40) NOT NULL, data_emissao date NOT NULL, observacao text, fantasma varchar(10));\
CREATE TABLE pedido_item (id serial PRIMARY KEY, pedido_id integer NOT NULL REFERENCES pedido(id), \
produto varchar(40), quantidade numeric(12,3) NOT NULL, preco_unitario numeric(12,2) NOT NULL, \
valor_total numeric(12,2) NOT NULL);";
    let (conv, _) = from_sql("Vendas", conv_sql);
    let m = fidelidade::comparar(&origem, &tela, "mestre_detalhe", &[], &conv, "pedido");
    assert_eq!(m.perdidos, ["Valor frete"]);
    assert_eq!(m.inventados, ["Fantasma"]);
    assert!(m.tipo_igual < m.achados, "Cliente lookup -> texto");
    assert!(m.kendall_tau.unwrap() < 1.0, "situacao antes de cliente");
    assert!(m.revocacao() < 1.0 && m.precisao() < 1.0);
}

#[test]
fn posicao_compara_canto_do_rotulo_em_fracao_da_imagem() {
    let (origem, _) = from_sql("Vendas", PEDIDOS);
    let tela = fidelidade::tela_de(&origem, "pedido").unwrap();
    let mut conv = origem.clone();
    let cx = |x: f32, y: f32| Caixa {
        x,
        y,
        w: 0.1,
        h: 0.01,
    };
    conv.layouts.push(ScreenLayout {
        screen: tela.clone(),
        groups: vec![],
        items: vec![LayoutItem {
            entity: "pedido".into(),
            field: "dt_emissao".into(),
            group: String::new(),
            bbox: cx(0.30, 0.40),
            read_order: 0,
            tab_order: 0,
        }],
    });
    let origem_cx = [(
        "pedido".to_string(),
        "dt_emissao".to_string(),
        cx(0.33, 0.44),
    )];
    let m = fidelidade::comparar(
        &origem,
        &tela,
        "mestre_detalhe",
        &origem_cx,
        &conv,
        "pedido",
    );
    assert_eq!(m.com_posicao, 1);
    assert!((m.distancia_media.unwrap() - 0.05).abs() < 1e-4, "{m:?}");
}

#[test]
fn gabarito_tem_vinte_telas_e_cada_uma_tem_tela_de_edicao() {
    assert_eq!(fidelidade::GABARITO.len(), 20);
    for g in fidelidade::GABARITO {
        let (app, avisos) = from_sql(g.nome, g.sql);
        let t = fidelidade::tela_de(&app, g.tabela)
            .unwrap_or_else(|| panic!("{}: sem tela ({avisos:?})", g.nome));
        let md = matches!(
            app.screens.iter().find(|s| s.id() == t),
            Some(Screen::MasterDetail { .. })
        );
        assert_eq!(md, g.padrao == "mestre_detalhe", "{}", g.nome);
        assert!(!fidelidade::campos_da_tela(&app, &t).is_empty());
    }
}

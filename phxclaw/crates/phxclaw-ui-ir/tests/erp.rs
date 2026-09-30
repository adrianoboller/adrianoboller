use phxclaw_ui_ir::*;

const PEDIDOS: &str = include_str!("fixtures/pedidos.sql");

#[test]
fn pedido_vira_mestre_detalhe_com_lookup_select_e_total() {
    let (app, avisos) = from_sql("Vendas", PEDIDOS);
    assert_eq!(app.entities.len(), 4);
    assert_eq!(
        avisos.len(),
        1,
        "o CREATE INDEX e ignorado COM aviso: {avisos:?}"
    );
    let md = app.screens.iter().find_map(|s| match s {
        Screen::MasterDetail {
            master,
            detail,
            detail_fk,
            totals,
            detail_columns,
            ..
        } => Some((master, detail, detail_fk, totals, detail_columns)),
        _ => None,
    });
    let (master, detail, fk, totals, cols) = md.expect("mestre/detalhe nao detectado");
    assert_eq!(
        (master.as_str(), detail.as_str(), fk.as_str()),
        ("pedido", "pedido_item", "pedido_id")
    );
    assert_eq!(
        totals.iter().map(|t| t.sum_of.as_str()).collect::<Vec<_>>(),
        vec!["vl_total"]
    );
    assert!(
        !cols.contains(&"pedido_id".to_string()),
        "a FK do mestre nao e coluna do item"
    );
    // item nao ganha tela propria: edita-se dentro do pedido
    assert!(
        !app.screens
            .iter()
            .any(|s| s.id().starts_with("pedido_item_"))
    );
    let pedido = app.entities.iter().find(|e| e.name == "pedido").unwrap();
    let f = |n: &str| pedido.fields.iter().find(|f| f.name == n).unwrap().clone();
    assert_eq!(
        f("cliente_id").widget,
        Widget::Lookup {
            entity: "cliente".into()
        }
    );
    assert_eq!(
        f("situacao").widget,
        Widget::Select {
            options: vec![
                "aberto".into(),
                "aprovado".into(),
                "faturado".into(),
                "cancelado".into()
            ]
        }
    );
    assert_eq!(f("vl_frete").widget, Widget::Money);
    assert_eq!(f("vl_frete").label, "Valor frete");
    assert_eq!(f("dt_emissao").label, "Data emissão");
    // rotulos vistos errados no print da tela (28/09): FK, detalhe e total
    assert_eq!(f("cliente_id").label, "Cliente");
    let item = app
        .entities
        .iter()
        .find(|e| e.name == "pedido_item")
        .unwrap();
    assert_eq!(item.label_plural, "Itens");
    let totais = app
        .screens
        .iter()
        .find_map(|s| match s {
            Screen::MasterDetail { totals, .. } => Some(totals.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(totais[0].label, "Total");
    assert!(
        f("id").readonly && !f("id").required,
        "serial e gerado pelo banco"
    );
}

#[test]
fn cadastro_reconhece_tipos_de_erp_e_auditoria() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let c = app.entities.iter().find(|e| e.name == "cliente").unwrap();
    let w = |n: &str| c.fields.iter().find(|f| f.name == n).unwrap();
    assert_eq!(c.display_field, "razao_social");
    assert_eq!(w("email").widget, Widget::Email);
    assert_eq!(w("telefone").widget, Widget::Phone);
    assert_eq!(w("observacao").widget, Widget::TextArea);
    assert_eq!(w("ativo").widget, Widget::Checkbox);
    assert!(
        !w("ativo").required,
        "com DEFAULT nao e obrigatorio digitar"
    );
    assert!(w("criado_em").readonly, "auditoria nao se edita");
    assert_eq!(w("cnpj").label, "CNPJ");
    assert_eq!(c.label_plural, "Clientes");
    let p = app.entities.iter().find(|e| e.name == "produto").unwrap();
    assert_eq!(
        p.fields.iter().find(|f| f.name == "preco").unwrap().widget,
        Widget::Money
    );
    assert_eq!(
        p.fields
            .iter()
            .find(|f| f.name == "peso_kg")
            .unwrap()
            .widget,
        Widget::Decimal { scale: 3 }
    );
    let menus: Vec<_> = app
        .menu
        .iter()
        .map(|m| (m.title.as_str(), m.screens.len()))
        .collect();
    assert_eq!(menus, vec![("Cadastros", 4), ("Movimentos", 2)]);
}

#[test]
fn ir_e_neutro_e_serializa_ida_e_volta() {
    let (app, _) = from_sql("Vendas", PEDIDOS);
    let j = serde_json::to_string(&app).unwrap();
    assert!(!j.contains("<"), "IR nao carrega HTML");
    let volta: App = serde_json::from_str(&j).unwrap();
    assert_eq!(volta, app);
}

#[test]
fn html_escapa_dado_e_marca_obrigatorio() {
    let (mut app, _) = from_sql("Vendas & Cia <teste>", PEDIDOS);
    app.entities[0].fields[1].label = "Razão <social> & \"x\"".into();
    let h = html::render(&app);
    assert!(h.contains("Vendas &amp; Cia &lt;teste&gt;"));
    assert!(!h.contains("Razão <social>"));
    assert!(h.contains("required aria-required=\"true\""));
    assert!(h.contains("data-add-item") && h.contains("data-soma=\"vl_total\""));
}

#[test]
fn ddl_mysql_com_crase_e_auto_increment() {
    let sql = "CREATE TABLE `nota` (`id` INT NOT NULL AUTO_INCREMENT, `numero` INT NOT NULL, `total` DECIMAL(12,2), PRIMARY KEY (`id`));\
               CREATE TABLE `nota_linha` (`id` INT AUTO_INCREMENT PRIMARY KEY, `nota_id` INT NOT NULL, `valor` DECIMAL(12,2) NOT NULL, FOREIGN KEY (`nota_id`) REFERENCES `nota`(`id`));";
    let (app, _) = from_sql("Fiscal", sql);
    let n = app.entities.iter().find(|e| e.name == "nota").unwrap();
    assert!(n.fields[0].readonly);
    assert!(
        app.screens
            .iter()
            .any(|s| matches!(s, Screen::MasterDetail { detail, .. } if detail == "nota_linha"))
    );
}

#[test]
fn tabela_no_plural_da_rotulo_singular_e_plural_certo() {
    // o agente escreveu tabelas no plural e a tela saiu "Consulta de clienteses"
    let sql = "CREATE TABLE clientes (id serial PRIMARY KEY, nome varchar(80) NOT NULL);\
               CREATE TABLE veiculos (id serial PRIMARY KEY, cliente_id int REFERENCES clientes(id));\
               CREATE TABLE ordens_servico (id serial PRIMARY KEY, veiculo_id int REFERENCES veiculos(id), valor_total numeric(12,2));\
               CREATE TABLE status (id serial PRIMARY KEY, nome varchar(20));";
    let (app, _) = from_sql("Oficina", sql);
    let r: Vec<(&str, &str)> = app
        .entities
        .iter()
        .map(|e| (e.label.as_str(), e.label_plural.as_str()))
        .collect();
    assert_eq!(
        r,
        vec![
            ("Cliente", "Clientes"),
            ("Veículo", "Veículos"),
            ("Ordem serviço", "Ordens serviço"),
            ("Status", "Status"),
        ]
    );
    let titulos: Vec<&str> = app.screens.iter().map(|s| s.title()).collect();
    assert!(titulos.contains(&"Consulta de clientes"), "{titulos:?}");
}

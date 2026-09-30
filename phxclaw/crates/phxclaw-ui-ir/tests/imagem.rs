//! Print -> SQL, a parte deterministica, com as respostas REAIS medidas do modelo de
//! visao (qwen2.5vl:3b, 30/09) e o OCR real (tesseract por) dos prints gerados aqui.

use phxclaw_ui_ir::imagem::*;
use phxclaw_ui_ir::{Screen, Widget};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

const OCR_PEDIDO: &[&str] = &[
    "Vendas",
    "Pedido",
    "CADASTROS",
    "Dados principais",
    "Consulta de clientes",
    "Código",
    "Cliente",
    "Data emissão *",
    "Situação *",
    "Cadastro de cliente",
    "Selecione cliente",
    "Valores",
    "Valor frete",
    "MOVIMENTOS",
    "R$",
    "0,00",
    "Observações",
    "Observação",
    "Itens",
    "Produto",
    "Quantidade",
    "Valor total",
    "Preço unitário",
    "Selecione produto",
    "Total",
    "R$ 1.244,75",
    "| Salvar || Novo || Excluir || Pesquisar H Imprimir",
];

#[test]
fn modelo_nao_inventa_campo_e_o_obrigatorio_vem_do_ocr() {
    let ocr = s(OCR_PEDIDO);
    // o que o modelo respondeu, mais um intruso inventado
    let campos = s(&[
        "Código",
        "Cliente",
        "Data emissão",
        "Valor frete",
        "Observação",
        "Prazo de entrega",
    ]);
    let listas = s(&["Código", "Cliente", "Data emissão", "Situação"]);
    let r = confirmar(&campos, &listas, &ocr);
    let nomes: Vec<&str> = r.iter().map(|x| x.texto.as_str()).collect();
    assert_eq!(
        nomes,
        [
            "Código",
            "Cliente",
            "Data emissão",
            "Valor frete",
            "Observação",
            "Situação"
        ]
    );
    let obrig: Vec<&str> = r
        .iter()
        .filter(|x| x.obrigatorio)
        .map(|x| x.texto.as_str())
        .collect();
    assert_eq!(
        obrig,
        ["Data emissão", "Situação"],
        "o * esta na linha lida, nao na resposta"
    );
    assert!(r.iter().find(|x| x.texto == "Situação").unwrap().lista);
}

#[test]
fn print_do_pedido_vira_mestre_detalhe_com_total() {
    let ocr = s(OCR_PEDIDO);
    let campos = confirmar(
        &s(&[
            "Código",
            "Cliente",
            "Data emissão",
            "Valor frete",
            "Observação",
        ]),
        &s(&["Situação"]),
        &ocr,
    );
    let itens = confirmar(
        &s(&["Produto", "Quantidade", "Preço unitário", "Valor total"]),
        &[],
        &ocr,
    );
    let q = sql("Pedido", &campos, &itens);
    assert!(q.contains("data_emissao date NOT NULL"), "{q}");
    assert!(q.contains("valor_frete numeric(12,2)"), "{q}");
    assert!(
        q.contains("pedido_id integer NOT NULL REFERENCES pedido(id)"),
        "{q}"
    );
    let (app, avisos) = phxclaw_ui_ir::from_sql("Vendas", &q);
    assert!(avisos.is_empty(), "{avisos:?}");
    let md = app
        .screens
        .iter()
        .find_map(|t| match t {
            Screen::MasterDetail { totals, detail, .. } => Some((totals.clone(), detail.clone())),
            _ => None,
        })
        .expect("mestre/detalhe");
    assert_eq!(md.1, "pedido_item");
    assert_eq!(md.0[0].sum_of, "valor_total");
    let ped = app.entities.iter().find(|e| e.name == "pedido").unwrap();
    let w = |n: &str| {
        ped.fields
            .iter()
            .find(|f| f.name == n)
            .unwrap()
            .widget
            .clone()
    };
    assert_eq!(w("data_emissao"), Widget::Date);
    assert_eq!(w("observacao"), Widget::TextArea);
}

#[test]
fn print_do_cliente_da_os_oito_campos_com_tipos() {
    let ocr = s(&[
        "Vendas",
        "Cadastro de cliente",
        "CADASTROS",
        "Dados principais",
        "Código",
        "Razão social *",
        "cnpj *",
        "Email",
        "Telefone",
        "Ativo",
        "Observações",
        "Observação",
        "Controle",
        "Criado em",
        "dd/mm/aaaa hhimm",
    ]);
    // resposta real do modelo (8/8)
    let r = confirmar(
        &s(&[
            "Código",
            "Razão social *",
            "CNPJ *",
            "Email",
            "Telefone",
            "Ativo",
            "Observação",
            "Criado em",
        ]),
        &[],
        &ocr,
    );
    assert_eq!(r.len(), 8);
    let q = sql("Cliente", &r, &[]);
    for trecho in [
        "razao_social varchar(120) NOT NULL",
        "cnpj varchar(18) NOT NULL",
        "telefone varchar(20)",
        "ativo boolean",
        "observacao text",
        "criado_em timestamp DEFAULT now()",
    ] {
        assert!(q.contains(trecho), "falta {trecho}:\n{q}");
    }
    assert!(!q.contains("codigo"), "Código vira a chave id: {q}");
}

#[test]
fn resposta_do_modelo_em_qualquer_das_tres_formas_medidas() {
    assert_eq!(lista_da_resposta("[\"A\", \"B\"]"), ["A", "B"]);
    assert_eq!(
        lista_da_resposta("```json\n{\"labels\": [\"A\"]}\n```"),
        ["A"]
    );
    // objeto de pares: vale o valor (o texto da tela), nao a chave traduzida
    assert_eq!(
        lista_da_resposta("{\"Product\": \"Produto\", \"Quantity\": \"Quantidade\"}"),
        ["Produto", "Quantidade"]
    );
    // JSON cortado pelo teto de tokens (resposta real): as cadeias ainda servem
    assert_eq!(
        lista_da_resposta("[\n \"Código\",\n \"Valor frete\",\n \"R$\",\n \"R$"),
        ["Código", "Valor frete", "R$", "R$"]
    );
    // e o simbolo, que o OCR tambem le, nao vira campo
    let r = confirmar(&s(&["Valor frete", "R$"]), &[], &s(OCR_PEDIDO));
    assert_eq!(r.len(), 1);
    assert!(lista_da_resposta("{}").is_empty());
    assert!(lista_da_resposta("nao sei").is_empty());
}

#[test]
fn coluna_da_grade_e_texto_de_exemplo_nao_entram_na_mestre() {
    // resposta real (endpoint de chat, 30/09): a grade e o "Selecione produto" vieram
    // junto com os campos do formulario
    let ocr = s(OCR_PEDIDO);
    let itens = confirmar(
        &s(&["Produto", "Quantidade", "Preço unitário", "Valor total"]),
        &[],
        &ocr,
    );
    let campos = sem_itens(
        &confirmar(
            &s(&[
                "Código",
                "Cliente",
                "Data emissão",
                "Situação",
                "Valor frete",
                "Observação",
                "Produto",
                "Quantidade",
                "Preço unitário",
                "Valor total",
                "Selecione produto",
            ]),
            &[],
            &ocr,
        ),
        &itens,
    );
    let nomes: Vec<&str> = campos.iter().map(|x| x.texto.as_str()).collect();
    assert_eq!(
        nomes,
        [
            "Código",
            "Cliente",
            "Data emissão",
            "Situação",
            "Valor frete",
            "Observação"
        ]
    );
}

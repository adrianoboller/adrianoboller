//! Schema SQL -> modelo ERP -> UI-IR.
//!
//! O parser cobre o DDL comum de PostgreSQL/MySQL (CREATE TABLE com tipos, NOT NULL, DEFAULT,
//! PRIMARY KEY, UNIQUE, REFERENCES, FOREIGN KEY de tabela, CHECK ... IN (...)). Nao e um
//! parser SQL completo: e o bastante para ler o desenho de um banco de ERP, e o que ele nao
//! entende ele IGNORA dizendo onde (`avisos`), em vez de adivinhar.

use crate::ir::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub sql_type: String,
    pub length: Option<u32>,
    pub scale: Option<u32>,
    pub not_null: bool,
    pub primary_key: bool,
    pub generated: bool,
    pub default: Option<String>,
    pub references: Option<(String, String)>,
    pub check_in: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub tables: Vec<Table>,
    pub avisos: Vec<String>,
}

// ------------------------------------------------------------------ parser

fn sem_comentarios(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let b: Vec<char> = sql.chars().collect();
    let (mut i, mut aspas) = (0, false);
    while i < b.len() {
        if b[i] == '\'' {
            aspas = !aspas;
        }
        if !aspas && b[i] == '-' && b.get(i + 1) == Some(&'-') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if !aspas && b[i] == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Divide por `sep` no nivel zero de parenteses, fora de aspas.
fn dividir(s: &str, sep: char) -> Vec<String> {
    let (mut partes, mut atual, mut nivel, mut aspas) = (vec![], String::new(), 0i32, false);
    for c in s.chars() {
        match c {
            '\'' => aspas = !aspas,
            '(' if !aspas => nivel += 1,
            ')' if !aspas => nivel -= 1,
            _ => {}
        }
        if c == sep && nivel == 0 && !aspas {
            partes.push(std::mem::take(&mut atual));
        } else {
            atual.push(c);
        }
    }
    if !atual.trim().is_empty() {
        partes.push(atual);
    }
    partes
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn ident(s: &str) -> String {
    let s = s
        .trim()
        .trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']');
    s.rsplit('.')
        .next()
        .unwrap_or(s)
        .trim_matches(|c| c == '"' || c == '`')
        .to_ascii_lowercase()
}

fn entre_parenteses(s: &str) -> Option<&str> {
    let i = s.find('(')?;
    let mut nivel = 0;
    for (j, c) in s[i..].char_indices() {
        match c {
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(&s[i + 1..i + j]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse(sql: &str) -> Parsed {
    let mut p = Parsed::default();
    for stmt in dividir(&sem_comentarios(sql), ';') {
        let up = stmt.to_ascii_uppercase();
        let Some(pos) = up.find("CREATE TABLE") else {
            if !up.trim().is_empty() {
                p.avisos.push(format!(
                    "ignorado: {}",
                    stmt.chars().take(60).collect::<String>()
                ));
            }
            continue;
        };
        let depois = &stmt[pos + "CREATE TABLE".len()..];
        let depois = depois.trim_start();
        let depois = if depois.to_ascii_uppercase().starts_with("IF NOT EXISTS") {
            &depois["IF NOT EXISTS".len()..]
        } else {
            depois
        };
        let Some(abre) = depois.find('(') else {
            continue;
        };
        let nome = ident(&depois[..abre]);
        let Some(corpo) = entre_parenteses(depois) else {
            continue;
        };
        let mut t = Table {
            name: nome,
            columns: vec![],
            primary_key: vec![],
        };
        let mut fks_de_tabela = vec![];
        for item in dividir(corpo, ',') {
            let iu = item.to_ascii_uppercase();
            let iu = iu.trim_start_matches("CONSTRAINT ").to_string();
            // "CONSTRAINT nome ..." -> pula o nome
            let item_sem_nome = if item.to_ascii_uppercase().starts_with("CONSTRAINT ") {
                item.splitn(3, char::is_whitespace)
                    .nth(2)
                    .unwrap_or("")
                    .to_string()
            } else {
                item.clone()
            };
            let isn = item_sem_nome.to_ascii_uppercase();
            if isn.starts_with("PRIMARY KEY") {
                if let Some(cols) = entre_parenteses(&item_sem_nome) {
                    t.primary_key = cols.split(',').map(ident).collect();
                }
            } else if isn.starts_with("FOREIGN KEY") {
                let cols = entre_parenteses(&item_sem_nome)
                    .map(|c| c.split(',').map(ident).collect::<Vec<_>>());
                let resto = &item_sem_nome[isn.find("REFERENCES").unwrap_or(0)..];
                if let (Some(cols), true) = (cols, isn.contains("REFERENCES")) {
                    let alvo = resto["REFERENCES".len()..].trim();
                    let tabela = ident(alvo.split('(').next().unwrap_or(alvo));
                    let col_alvo = entre_parenteses(alvo)
                        .map(ident)
                        .unwrap_or_else(|| "id".into());
                    if let Some(c) = cols.first() {
                        fks_de_tabela.push((c.clone(), tabela, col_alvo));
                    }
                }
            } else if isn.starts_with("UNIQUE")
                || isn.starts_with("CHECK")
                || iu.starts_with("INDEX")
                || iu.starts_with("KEY ")
            {
                // restricao de tabela sem efeito na tela (CHECK de coluna e tratado abaixo)
            } else {
                t.columns.push(coluna(&item));
            }
        }
        for (c, tabela, col) in fks_de_tabela {
            if let Some(x) = t.columns.iter_mut().find(|x| x.name == c) {
                x.references = Some((tabela, col));
            }
        }
        if t.primary_key.is_empty() {
            t.primary_key = t
                .columns
                .iter()
                .filter(|c| c.primary_key)
                .map(|c| c.name.clone())
                .collect();
        }
        let pk = t.primary_key.clone();
        for c in t.columns.iter_mut().filter(|c| pk.contains(&c.name)) {
            c.primary_key = true;
            c.not_null = true;
        }
        p.tables.push(t);
    }
    p
}

fn coluna(item: &str) -> Column {
    let mut toks = item.split_whitespace();
    let name = ident(toks.next().unwrap_or(""));
    let resto: String =
        item.trim()[item.trim().find(char::is_whitespace).unwrap_or(item.len())..].to_string();
    let ru = resto.to_ascii_uppercase();
    let tipo_bruto = resto.split_whitespace().next().unwrap_or("").to_string();
    let base = tipo_bruto
        .split('(')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let (mut length, mut scale) = (None, None);
    if let Some(args) = entre_parenteses(&resto)
        .filter(|_| tipo_bruto.contains('(') || resto.trim_start().starts_with(&format!("{base}(")))
    {
        let n: Vec<u32> = args
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        length = n.first().copied();
        scale = n.get(1).copied();
    }
    let references = ru.find("REFERENCES").map(|i| {
        let alvo = resto[i + "REFERENCES".len()..].trim();
        let tabela = ident(
            alvo.split(|c: char| c == '(' || c.is_whitespace())
                .next()
                .unwrap_or(alvo),
        );
        let col = entre_parenteses(alvo)
            .map(ident)
            .unwrap_or_else(|| "id".into());
        (tabela, col)
    });
    let mut check_in = vec![];
    if let Some(i) = ru.find("CHECK")
        && let Some(dentro) = entre_parenteses(&resto[i..])
        && let Some(j) = dentro.to_ascii_uppercase().find(" IN ")
        && let Some(lista) = entre_parenteses(&dentro[j..])
    {
        check_in = lista
            .split(',')
            .map(|x| x.trim().trim_matches('\'').to_string())
            .filter(|x| !x.is_empty())
            .collect();
    }
    let default = ru.find("DEFAULT").map(|i| {
        resto[i + 7..]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    });
    Column {
        name,
        generated: matches!(base.as_str(), "serial" | "bigserial" | "smallserial")
            || ru.contains("GENERATED")
            || ru.contains("AUTO_INCREMENT")
            || ru.contains("IDENTITY"),
        sql_type: base,
        length,
        scale,
        not_null: ru.contains("NOT NULL") || ru.contains("PRIMARY KEY"),
        primary_key: ru.contains("PRIMARY KEY"),
        default,
        references,
        check_in,
    }
}

// ------------------------------------------------------------------ inteligencia ERP

/// "data_emissao" -> "Data emissão". Tabela curta das abreviacoes de ERP em portugues.
pub fn rotulo(nome: &str) -> String {
    const TROCA: &[(&str, &str)] = &[
        ("dt", "data"),
        ("vl", "valor"),
        ("qtd", "quantidade"),
        ("qtde", "quantidade"),
        ("cod", "código"),
        ("codigo", "código"),
        ("descricao", "descrição"),
        ("observacao", "observação"),
        ("obs", "observação"),
        ("emissao", "emissão"),
        ("condicao", "condição"),
        ("preco", "preço"),
        ("razao", "razão"),
        ("endereco", "endereço"),
        ("numero", "número"),
        ("situacao", "situação"),
        ("inscricao", "inscrição"),
        ("unitario", "unitário"),
        ("id", "código"),
        ("uf", "UF"),
        ("cpf", "CPF"),
        ("cnpj", "CNPJ"),
        ("cep", "CEP"),
        ("ie", "IE"),
        ("pagamento", "pagamento"),
        ("comissao", "comissão"),
    ];
    let palavras: Vec<String> = nome
        .split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            TROCA
                .iter()
                .find(|(a, _)| *a == p)
                .map(|(_, b)| b.to_string())
                .unwrap_or_else(|| p.to_string())
        })
        .collect();
    let s = palavras.join(" ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// "cliente_id" -> "cliente": o sufixo de chave some do rotulo do lookup.
fn sem_sufixo_de_chave(nome: &str) -> &str {
    ["_id", "_cod", "_codigo"]
        .iter()
        .find_map(|s| nome.strip_suffix(s))
        .filter(|r| !r.is_empty())
        .unwrap_or(nome)
}

fn plural(r: &str) -> String {
    let (ultima, resto) = match r.rsplit_once(' ') {
        Some((a, b)) => (b.to_string(), format!("{a} ")),
        None => (r.to_string(), String::new()),
    };
    // pluraliza a primeira palavra quando o nome e composto ("Item pedido" -> "Itens pedido")
    let alvo = if resto.is_empty() {
        ultima.clone()
    } else {
        r.split(' ').next().unwrap_or(r).to_string()
    };
    let p = if let Some(s) = alvo.strip_suffix('m') {
        format!("{s}ns")
    } else if let Some(s) = alvo.strip_suffix("ão") {
        format!("{s}ões")
    } else if let Some(s) = alvo.strip_suffix('l') {
        format!("{s}is")
    } else if alvo.ends_with(['r', 'z', 's']) {
        format!("{alvo}es")
    } else {
        format!("{alvo}s")
    };
    if resto.is_empty() {
        p
    } else {
        r.replacen(&alvo, &p, 1)
    }
}

const MONEY: &[&str] = &[
    "preco", "valor", "vl", "total", "custo", "desconto", "frete", "imposto", "salario",
    "subtotal", "comissao", "saldo", "price", "amount",
];

fn widget(c: &Column, tabelas: &[Table]) -> Widget {
    if let Some((t, _)) = &c.references
        && tabelas.iter().any(|x| &x.name == t)
    {
        return Widget::Lookup { entity: t.clone() };
    }
    if !c.check_in.is_empty() {
        return Widget::Select {
            options: c.check_in.clone(),
        };
    }
    let n = c.name.as_str();
    let tem = |xs: &[&str]| n.split('_').any(|p| xs.contains(&p));
    match c.sql_type.as_str() {
        "bool" | "boolean" | "bit" => Widget::Checkbox,
        "date" => Widget::Date,
        "timestamp" | "timestamptz" | "datetime" => Widget::DateTime,
        "int" | "int2" | "int4" | "int8" | "integer" | "smallint" | "bigint" | "serial"
        | "bigserial" | "tinyint" => Widget::Integer,
        "numeric" | "decimal" | "money" | "real" | "float" | "double" | "float8" => {
            if c.sql_type == "money" || (tem(MONEY) && c.scale.unwrap_or(2) <= 2) {
                Widget::Money
            } else {
                Widget::Decimal {
                    scale: c.scale.unwrap_or(2),
                }
            }
        }
        _ if tem(&["email"]) => Widget::Email,
        _ if tem(&["telefone", "fone", "celular", "whatsapp", "phone"]) => Widget::Phone,
        "text" | "clob" | "longtext" | "mediumtext" => Widget::TextArea,
        _ if tem(&["observacao", "obs", "descricao"]) && c.length.unwrap_or(0) > 200 => {
            Widget::TextArea
        }
        _ => Widget::Text,
    }
}

fn auditoria(c: &Column) -> bool {
    matches!(
        c.name.as_str(),
        "criado_em"
            | "atualizado_em"
            | "created_at"
            | "updated_at"
            | "alterado_em"
            | "data_cadastro"
    ) || c.default.as_deref().is_some_and(|d| {
        let d = d.to_ascii_lowercase();
        d.starts_with("now") || d.starts_with("current_timestamp")
    })
}

/// Campo que representa o registro num lookup.
fn display(t: &Table) -> String {
    for pref in [
        "nome",
        "razao_social",
        "nome_fantasia",
        "descricao",
        "titulo",
        "name",
        "description",
        "codigo",
    ] {
        if t.columns.iter().any(|c| c.name == pref) {
            return pref.into();
        }
    }
    t.columns
        .iter()
        .find(|c| {
            matches!(
                c.sql_type.as_str(),
                "varchar" | "text" | "char" | "character"
            )
        })
        .or_else(|| t.columns.first())
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

/// Mae de uma tabela de itens, e a FK que aponta para ela. O nome que diz de quem e o item
/// (pedido_item -> pedido) vence a palavra generica ("item", "linha"): medido, a palavra
/// sozinha casava pedido_item com PRODUTO, que tambem e FK obrigatoria do item.
fn mae_de(filha: &Table, tabelas: &[Table]) -> Option<(String, String)> {
    let obrigatorias: Vec<&Column> = filha
        .columns
        .iter()
        .filter(|c| {
            c.not_null
                && c.references
                    .as_ref()
                    .is_some_and(|(t, _)| *t != filha.name && tabelas.iter().any(|x| &x.name == t))
        })
        .collect();
    let n = &filha.name;
    let forte = obrigatorias.iter().find(|c| {
        let mae = &c.references.as_ref().expect("filtrado").0;
        n.starts_with(&format!("{mae}_")) || n.ends_with(&format!("_{mae}"))
    });
    if let Some(c) = forte {
        return Some((
            c.references.as_ref().expect("filtrado").0.clone(),
            c.name.clone(),
        ));
    }
    let generica = [
        "item", "itens", "linha", "linhas", "detalhe", "parcela", "parcelas",
    ]
    .iter()
    .any(|k| n.split('_').any(|p| p == *k));
    match (generica, obrigatorias.as_slice()) {
        (true, [c]) => Some((
            c.references.as_ref().expect("filtrado").0.clone(),
            c.name.clone(),
        )),
        _ => None,
    }
}

pub fn analyze(nome_app: &str, p: &Parsed) -> App {
    let tabelas = &p.tables;
    let mut entities = vec![];
    for t in tabelas {
        let fields: Vec<Field> = t
            .columns
            .iter()
            .map(|c| {
                let w = widget(c, tabelas);
                let readonly = c.generated || auditoria(c);
                Field {
                    // FK mostra a entidade ("Cliente"), nao a coluna ("Cliente codigo")
                    label: match &w {
                        Widget::Lookup { .. } => rotulo(sem_sufixo_de_chave(&c.name)),
                        _ => rotulo(&c.name),
                    },
                    required: c.not_null && !readonly && c.default.is_none(),
                    readonly,
                    max_len: if matches!(w, Widget::Text | Widget::Email | Widget::Phone) {
                        c.length
                    } else {
                        None
                    },
                    in_list: false,
                    widget: w,
                    name: c.name.clone(),
                }
            })
            .collect();
        let mut e = Entity {
            name: t.name.clone(),
            label: rotulo(&t.name),
            label_plural: plural(&rotulo(&t.name)),
            primary_key: t.primary_key.clone(),
            display_field: display(t),
            fields,
        };
        // grade: chave, campo de exibicao, e ate 5 outros que nao sejam texto longo/auditoria
        let mut n = 0;
        let prioridade: Vec<String> = e
            .primary_key
            .iter()
            .cloned()
            .chain([e.display_field.clone()])
            .collect();
        for f in e.fields.iter_mut() {
            if prioridade.contains(&f.name) {
                f.in_list = true;
            }
        }
        for f in e.fields.iter_mut() {
            if !f.in_list
                && n < 5
                && !matches!(f.widget, Widget::TextArea)
                && (!f.readonly || e.primary_key.contains(&f.name))
            {
                f.in_list = true;
                n += 1;
            }
        }
        entities.push(e);
    }

    // mestre/detalhe
    let mut mestres: BTreeMap<String, (String, String)> = BTreeMap::new(); // mae -> (filha, fk)
    let mut filhas = vec![];
    for filha in tabelas {
        if let Some((mae, fk)) = mae_de(filha, tabelas)
            && !mestres.contains_key(&mae)
        {
            mestres.insert(mae, (filha.name.clone(), fk));
            filhas.push(filha.name.clone());
        }
    }

    // O detalhe se le dentro do mestre: "pedido_item" vira "Itens", nao "Pedidos item".
    for (mae, (filha, _)) in &mestres {
        if let Some(e) = entities.iter_mut().find(|e| &e.name == filha)
            && let Some(resto) = filha.strip_prefix(&format!("{mae}_"))
        {
            e.label = rotulo(resto);
            e.label_plural = plural(&e.label);
        }
    }

    let ent = |n: &str| entities.iter().find(|e| e.name == n).expect("entidade");
    let mut screens = vec![];
    let mut cadastros = vec![];
    let mut movimentos = vec![];
    for e in &entities {
        if filhas.contains(&e.name) {
            continue; // itens se editam dentro do mestre
        }
        let lista = format!("{}_consulta", e.name);
        let edicao = format!(
            "{}_{}",
            e.name,
            if mestres.contains_key(&e.name) {
                "documento"
            } else {
                "cadastro"
            }
        );
        screens.push(Screen::List {
            id: lista.clone(),
            title: format!("Consulta de {}", e.label_plural.to_lowercase()),
            entity: e.name.clone(),
            columns: e
                .fields
                .iter()
                .filter(|f| f.in_list)
                .map(|f| f.name.clone())
                .collect(),
            search_fields: e
                .fields
                .iter()
                .filter(|f| {
                    matches!(
                        f.widget,
                        Widget::Text | Widget::Email | Widget::Lookup { .. }
                    ) || e.primary_key.contains(&f.name)
                })
                .take(4)
                .map(|f| f.name.clone())
                .collect(),
            opens: edicao.clone(),
        });
        let secoes = secoes(e);
        let acoes = vec![
            Action {
                id: "salvar".into(),
                label: "Salvar".into(),
                kind: "alter".into(),
            },
            Action {
                id: "novo".into(),
                label: "Novo".into(),
                kind: "include".into(),
            },
            Action {
                id: "excluir".into(),
                label: "Excluir".into(),
                kind: "delete".into(),
            },
            Action {
                id: "pesquisar".into(),
                label: "Pesquisar".into(),
                kind: "query".into(),
            },
        ];
        if let Some((filha, fk)) = mestres.get(&e.name) {
            let f = ent(filha);
            let mut acoes = acoes;
            acoes.push(Action {
                id: "imprimir".into(),
                label: "Imprimir".into(),
                kind: "query".into(),
            });
            let totais: Vec<Total> = f
                .fields
                .iter()
                .filter(|x| {
                    matches!(x.widget, Widget::Money)
                        && x.name
                            .split('_')
                            .any(|p| matches!(p, "total" | "subtotal" | "valor"))
                })
                .map(|x| Total {
                    label: if x.label.to_lowercase().contains("total") {
                        "Total".into()
                    } else {
                        format!("Total {}", x.label.to_lowercase())
                    },
                    sum_of: x.name.clone(),
                })
                .collect();
            screens.push(Screen::MasterDetail {
                id: edicao.clone(),
                title: e.label.clone(),
                master: e.name.clone(),
                detail: filha.clone(),
                detail_fk: fk.clone(),
                header: secoes,
                detail_columns: f
                    .fields
                    .iter()
                    .filter(|x| x.name != *fk && !(x.readonly && f.primary_key.contains(&x.name)))
                    .map(|x| x.name.clone())
                    .collect(),
                totals: totais,
                actions: acoes,
            });
            movimentos.push(lista);
            movimentos.push(edicao);
        } else {
            screens.push(Screen::Form {
                id: edicao.clone(),
                title: format!("Cadastro de {}", e.label.to_lowercase()),
                entity: e.name.clone(),
                sections: secoes,
                actions: acoes,
            });
            cadastros.push(lista);
            cadastros.push(edicao);
        }
    }
    let mut menu = vec![];
    if !cadastros.is_empty() {
        menu.push(MenuGroup {
            title: "Cadastros".into(),
            screens: cadastros,
        });
    }
    if !movimentos.is_empty() {
        menu.push(MenuGroup {
            title: "Movimentos".into(),
            screens: movimentos,
        });
    }
    App {
        name: nome_app.into(),
        ir_version: IR_VERSION,
        entities,
        screens,
        menu,
    }
}

/// Secoes do formulario: identificacao, valores, observacoes, controle.
fn secoes(e: &Entity) -> Vec<Section> {
    let mut principal = vec![];
    let mut valores = vec![];
    let mut obs = vec![];
    let mut controle = vec![];
    for f in &e.fields {
        if f.readonly && !e.primary_key.contains(&f.name) {
            controle.push(f.name.clone());
        } else if matches!(f.widget, Widget::Money | Widget::Decimal { .. }) {
            valores.push(f.name.clone());
        } else if matches!(f.widget, Widget::TextArea) {
            obs.push(f.name.clone());
        } else {
            principal.push(f.name.clone());
        }
    }
    [
        ("Dados principais", principal),
        ("Valores", valores),
        ("Observações", obs),
        ("Controle", controle),
    ]
    .into_iter()
    .filter(|(_, f)| !f.is_empty())
    .map(|(t, fields)| Section {
        title: t.into(),
        fields,
    })
    .collect()
}

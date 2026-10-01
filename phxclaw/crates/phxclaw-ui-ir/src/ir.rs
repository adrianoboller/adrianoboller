//! UI-IR: a representacao NEUTRA de telas de ERP. Quem analisa (schema, screenshot, prompt)
//! escreve isto; quem desenha (HTML, React, WinDev, WebDev...) le isto. Nenhum dos lados
//! conhece o outro -- e o que deixa trocar o renderizador sem reescrever a inteligencia ERP.
//!
//! Versoes do formato:
//! - v1: entidades, telas e menu.
//! - v2 (01/10/2026): `layouts`, OPCIONAL -- o que se leu de uma captura de tela: posicao
//!   relativa (0..1 da imagem), grupo (secao ou grade) e ordem de leitura e de tabulacao de
//!   cada campo. Entrou opcional e com `default` para que todo JSON v1 continue lendo igual
//!   (`App::de_json`), e porque a tela que nasce de SQL nao tem layout medido: inventar
//!   posicao para ela seria escrever no IR um numero que ninguem mediu. Os renderizadores
//!   respeitam ordem e grupos pelo mesmo caminho (`App::com_layout`); a posicao absoluta
//!   fica registrada, mas nenhum renderizador a usa para posicionar -- a grade responsiva
//!   da casa continua mandando, e a posicao serve a prova de fidelidade.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    /// Versao do formato, para renderizadores recusarem o que nao entendem.
    pub ir_version: u32,
    pub entities: Vec<Entity>,
    pub screens: Vec<Screen>,
    pub menu: Vec<MenuGroup>,
    /// v2: layout lido de captura, por tela. Vazio (e ausente no JSON) quando a tela nasceu
    /// de SQL -- assim um v1 serializa igual a antes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<ScreenLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub label: String,
    pub label_plural: String,
    pub primary_key: Vec<String>,
    pub fields: Vec<Field>,
    /// Campo que representa o registro num lookup (nome, descricao...).
    pub display_field: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub widget: Widget,
    pub required: bool,
    /// Chave gerada pelo banco, ou coluna de auditoria: aparece, nao se edita.
    pub readonly: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<u32>,
    /// Mostrar na grade da consulta.
    pub in_list: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Widget {
    Text,
    TextArea,
    Integer,
    Decimal {
        scale: u32,
    },
    Money,
    Date,
    DateTime,
    Checkbox,
    /// Lista fixa (CHECK ... IN (...) no schema).
    Select {
        options: Vec<String>,
    },
    /// Chave estrangeira: escolhe um registro de outra entidade.
    Lookup {
        entity: String,
    },
    Email,
    Phone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "pattern", rename_all = "snake_case")]
pub enum Screen {
    /// Consulta: pesquisa + grade + acoes.
    List {
        id: String,
        title: String,
        entity: String,
        columns: Vec<String>,
        search_fields: Vec<String>,
        opens: String,
    },
    /// Cadastro: formulario de um registro.
    Form {
        id: String,
        title: String,
        entity: String,
        sections: Vec<Section>,
        actions: Vec<Action>,
    },
    /// Mestre/detalhe: cabecalho + grade de itens + totais (pedido, nota, ordem).
    MasterDetail {
        id: String,
        title: String,
        master: String,
        detail: String,
        /// Coluna da filha que aponta para o mestre.
        detail_fk: String,
        header: Vec<Section>,
        detail_columns: Vec<String>,
        totals: Vec<Total>,
        actions: Vec<Action>,
    },
}

impl Screen {
    pub fn id(&self) -> &str {
        match self {
            Screen::List { id, .. } | Screen::Form { id, .. } | Screen::MasterDetail { id, .. } => {
                id
            }
        }
    }
    pub fn title(&self) -> &str {
        match self {
            Screen::List { title, .. }
            | Screen::Form { title, .. }
            | Screen::MasterDetail { title, .. } => title,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Total {
    pub label: String,
    /// Soma desta coluna da grade de detalhe.
    pub sum_of: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub label: String,
    /// include | alter | delete | query | custom -- a convencao de cores da casa (verde
    /// inclui, amarelo altera, vermelho exclui, azul consulta) sai daqui no renderizador.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuGroup {
    pub title: String,
    pub screens: Vec<String>,
}

/// Caixa relativa a imagem lida: 0..1 nos dois eixos, para nao depender da resolucao da
/// captura (a mesma tela em 1280 e em 1920 tem de dar o mesmo layout).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Caixa {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenLayout {
    /// `Screen::id` a que o layout se refere.
    pub screen: String,
    pub groups: Vec<LayoutGroup>,
    pub items: Vec<LayoutItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutGroup {
    pub id: String,
    /// Titulo lido (legenda da secao); vazio quando a tela nao tinha.
    pub title: String,
    /// `section` (rotulo + campo) ou `grid` (colunas com cabecalho).
    pub kind: String,
    pub bbox: Caixa,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutItem {
    pub entity: String,
    pub field: String,
    /// `LayoutGroup::id`.
    pub group: String,
    /// Caixa do ROTULO lido (o campo em si o OCR nao enxerga: so texto tem caixa).
    pub bbox: Caixa,
    pub read_order: u32,
    pub tab_order: u32,
}

impl App {
    /// Le o UI-IR em JSON: aceita da v1 ate a corrente e recusa versao do futuro, que
    /// traria campo que este leitor descartaria calado.
    pub fn de_json(s: &str) -> Result<App, String> {
        let app: App = serde_json::from_str(s).map_err(|e| format!("UI-IR invalido: {e}"))?;
        if app.ir_version == 0 || app.ir_version > IR_VERSION {
            return Err(format!(
                "UI-IR v{} nao suportado (este leitor vai da v1 a v{IR_VERSION})",
                app.ir_version
            ));
        }
        Ok(app)
    }

    /// O app como os renderizadores o desenham: secoes, colunas da grade e ordem dos campos
    /// seguindo o layout lido, quando ha. E o UNICO lugar onde o layout vira ordem e grupo:
    /// HTML, React, Flutter e as regras (Rust e WLanguage, via `regras::de`) passam por aqui,
    /// para que os quatro desenhos nao divirjam sobre qual campo vem primeiro. Sem layout,
    /// devolve igual; aplicar duas vezes da o mesmo resultado.
    pub fn com_layout(&self) -> App {
        let mut app = self.clone();
        for l in &self.layouts {
            let Some(tela) = app.screens.iter_mut().find(|s| s.id() == l.screen) else {
                continue;
            };
            match tela {
                Screen::Form {
                    entity, sections, ..
                } => *sections = reagrupar(sections, l, entity),
                Screen::MasterDetail {
                    master,
                    detail,
                    header,
                    detail_columns,
                    ..
                } => {
                    *header = reagrupar(header, l, master);
                    let pos = |c: &String| tab(l, detail, c);
                    detail_columns.sort_by_key(pos);
                }
                Screen::List { .. } => {}
            }
            // a ordem dos campos da entidade segue a tabulacao lida: e ela que as regras
            // usam, e o primeiro erro de validacao tem de ser o do primeiro campo da tela
            for e in app.entities.iter_mut() {
                if l.items.iter().any(|i| i.entity == e.name) {
                    let nome = e.name.clone();
                    e.fields.sort_by_key(|f| tab(l, &nome, &f.name));
                }
            }
        }
        app
    }
}

/// Posicao de tabulacao lida; quem nao foi lido vai para o fim, na ordem que ja tinha
/// (o `sort_by_key` e estavel).
fn tab(l: &ScreenLayout, entidade: &str, campo: &str) -> u32 {
    l.items
        .iter()
        .find(|i| i.entity == entidade && i.field == campo)
        .map_or(u32::MAX, |i| i.tab_order)
}

/// Secoes na ordem e com os grupos lidos. So reagrupa campo que ja estava nas secoes: o
/// layout decide ONDE o campo fica, nunca SE ele existe na tela.
fn reagrupar(secoes: &[Section], l: &ScreenLayout, entidade: &str) -> Vec<Section> {
    let na_tela = |c: &str| secoes.iter().any(|s| s.fields.iter().any(|f| f == c));
    let titulo_de = |c: &str| {
        secoes
            .iter()
            .find(|s| s.fields.iter().any(|f| f == c))
            .map(|s| s.title.clone())
            .unwrap_or_default()
    };
    let mut grupos: Vec<(u32, &LayoutGroup, Vec<&LayoutItem>)> = l
        .groups
        .iter()
        .filter(|g| g.kind != "grid")
        .filter_map(|g| {
            let mut v: Vec<&LayoutItem> = l
                .items
                .iter()
                .filter(|i| i.entity == entidade && i.group == g.id && na_tela(&i.field))
                .collect();
            v.sort_by_key(|i| i.tab_order);
            let primeiro = v.iter().map(|i| i.read_order).min()?;
            Some((primeiro, g, v))
        })
        .collect();
    grupos.sort_by_key(|(p, _, _)| *p);
    let mut novas: Vec<Section> = vec![];
    for (_, g, itens) in grupos {
        let title = if g.title.trim().is_empty() {
            titulo_de(&itens[0].field)
        } else {
            g.title.clone()
        };
        let fields: Vec<String> = itens
            .iter()
            .map(|i| i.field.clone())
            .filter(|f| !novas.iter().any(|s| s.fields.contains(f)))
            .collect();
        match novas.iter_mut().find(|s| s.title == title) {
            Some(s) => s.fields.extend(fields),
            None => novas.push(Section { title, fields }),
        }
    }
    // o que o layout nao alcancou fica na secao de origem, depois do que foi lido
    for s in secoes {
        let resto: Vec<String> = s
            .fields
            .iter()
            .filter(|f| !novas.iter().any(|n| n.fields.contains(f)))
            .cloned()
            .collect();
        if resto.is_empty() {
            continue;
        }
        match novas.iter_mut().find(|n| n.title == s.title) {
            Some(n) => n.fields.extend(resto),
            None => novas.push(Section {
                title: s.title.clone(),
                fields: resto,
            }),
        }
    }
    novas.retain(|s| !s.fields.is_empty());
    novas
}

pub const IR_VERSION: u32 = 2;

//! UI-IR: a representacao NEUTRA de telas de ERP. Quem analisa (schema, screenshot, prompt)
//! escreve isto; quem desenha (HTML, React, WinDev, WebDev...) le isto. Nenhum dos lados
//! conhece o outro -- e o que deixa trocar o renderizador sem reescrever a inteligencia ERP.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    /// Versao do formato, para renderizadores recusarem o que nao entendem.
    pub ir_version: u32,
    pub entities: Vec<Entity>,
    pub screens: Vec<Screen>,
    pub menu: Vec<MenuGroup>,
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

pub const IR_VERSION: u32 = 1;

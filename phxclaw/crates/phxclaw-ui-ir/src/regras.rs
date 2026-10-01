//! As regras de negocio que o UI-IR implica, num modelo so. Os backends de codigo
//! (Rust e WLanguage) leem ISTO, nunca o IR direto: se cada um derivasse as regras por
//! conta propria, a primeira correcao entraria num e esqueceria o outro, e o WinDev e o
//! Rust passariam a recusar coisas diferentes. Mensagens tambem saem daqui, pelo mesmo
//! motivo: a mesma entrada tem de dar o mesmo texto nos dois.
//!
//! O que se garante, por entidade:
//! - texto e data obrigatorios nao podem vir vazios;
//! - texto nao passa do tamanho da coluna;
//! - lista fixa (CHECK IN) so aceita as opcoes;
//! - data (e data/hora) tem de existir no calendario;
//! - chave estrangeira so aceita pai que existe (so existe filho se o pai existir);
//! - excluir o pai que tem filhos e recusado (restringir; nunca cascata);
//! - mestre/detalhe tem o total da coluna de valor dos itens.

use crate::ir::{App, Screen, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Texto,
    Inteiro,
    Decimal,
    /// Centavos inteiros: dinheiro nunca passa por ponto flutuante.
    Dinheiro,
    Data,
    DataHora,
    Logico,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    pub nome: String,
    pub rotulo: String,
    pub tipo: Tipo,
    pub obrigatorio: bool,
    pub somente_leitura: bool,
    pub max_len: Option<u32>,
    pub opcoes: Vec<String>,
    /// (entidade mae, chave da mae)
    pub mae: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Total {
    pub filha: String,
    pub fk: String,
    pub campo: String,
    pub rotulo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entidade {
    pub nome: String,
    pub rotulo: String,
    pub rotulo_plural: String,
    pub chave: String,
    /// Chave gerada pelo banco: quem inclui nao escolhe o numero.
    pub chave_automatica: bool,
    pub campos: Vec<Campo>,
    /// (entidade filha, campo da filha que aponta para esta)
    pub filhas: Vec<(String, String)>,
    pub totais: Vec<Total>,
}

impl Campo {
    /// Campo que entra na validacao de preenchimento (o resto tem valor sempre).
    pub fn exige_preenchimento(&self) -> bool {
        self.obrigatorio && matches!(self.tipo, Tipo::Texto | Tipo::Data | Tipo::DataHora)
    }
}

pub fn msg_obrigatorio(c: &Campo) -> String {
    format!("Preencha o campo {}", c.rotulo)
}
pub fn msg_tamanho(c: &Campo, n: u32) -> String {
    format!("{} passa de {n} caracteres", c.rotulo)
}
pub fn msg_opcao(c: &Campo) -> String {
    format!("{} fora das opções: {}", c.rotulo, c.opcoes.join(", "))
}
pub fn msg_data(c: &Campo) -> String {
    format!("{} não é uma data válida", c.rotulo)
}
pub fn msg_sem_pai(c: &Campo, mae: &Entidade) -> String {
    format!(
        "{}: não existe {} com esse código",
        c.rotulo,
        mae.rotulo.to_lowercase()
    )
}
pub fn msg_tem_filhos(e: &Entidade, filha: &Entidade) -> String {
    format!(
        "{} tem {} ligados: exclua-os antes",
        e.rotulo,
        filha.rotulo_plural.to_lowercase()
    )
}
pub fn msg_nao_encontrado(e: &Entidade) -> String {
    format!("{} não encontrado", e.rotulo)
}
pub fn msg_chave_repetida(e: &Entidade) -> String {
    format!("{}: código já existe", e.rotulo)
}

pub fn de(app: &App) -> Vec<Entidade> {
    // Rust e WLanguage validam na ordem de tabulacao lida (v2): o primeiro erro e o do
    // primeiro campo da tela
    let app = &app.com_layout();
    let chave_de = |ent: &str| {
        app.entities
            .iter()
            .find(|e| e.name == ent)
            .and_then(|e| e.primary_key.first().cloned())
            .unwrap_or_else(|| "id".into())
    };
    let mut v: Vec<Entidade> = app
        .entities
        .iter()
        .map(|e| {
            let chave = e
                .primary_key
                .first()
                .cloned()
                .unwrap_or_else(|| "id".into());
            let campos: Vec<Campo> = e
                .fields
                .iter()
                .map(|f| {
                    let (tipo, opcoes, mae) = match &f.widget {
                        Widget::Text | Widget::TextArea | Widget::Email | Widget::Phone => {
                            (Tipo::Texto, vec![], None)
                        }
                        Widget::Select { options } => (Tipo::Texto, options.clone(), None),
                        Widget::Lookup { entity } => (
                            Tipo::Inteiro,
                            vec![],
                            Some((entity.clone(), chave_de(entity))),
                        ),
                        Widget::Integer => (Tipo::Inteiro, vec![], None),
                        Widget::Decimal { .. } => (Tipo::Decimal, vec![], None),
                        Widget::Money => (Tipo::Dinheiro, vec![], None),
                        Widget::Date => (Tipo::Data, vec![], None),
                        Widget::DateTime => (Tipo::DataHora, vec![], None),
                        Widget::Checkbox => (Tipo::Logico, vec![], None),
                    };
                    Campo {
                        nome: f.name.clone(),
                        rotulo: f.label.clone(),
                        tipo,
                        obrigatorio: f.required,
                        somente_leitura: f.readonly,
                        max_len: f.max_len,
                        opcoes,
                        mae,
                    }
                })
                .collect();
            let chave_automatica = campos.iter().any(|c| c.nome == chave && c.somente_leitura);
            Entidade {
                nome: e.name.clone(),
                rotulo: e.label.clone(),
                rotulo_plural: e.label_plural.clone(),
                chave,
                chave_automatica,
                campos,
                filhas: vec![],
                totais: vec![],
            }
        })
        .collect();
    // filhas: quem aponta para quem, lido dos lookups (a chave e declarada na filha)
    let ligacoes: Vec<(String, String, String)> = v
        .iter()
        .flat_map(|e| {
            e.campos.iter().filter_map(|c| {
                c.mae
                    .as_ref()
                    .map(|(m, _)| (m.clone(), e.nome.clone(), c.nome.clone()))
            })
        })
        .collect();
    for (mae, filha, fk) in ligacoes {
        if let Some(m) = v.iter_mut().find(|e| e.nome == mae) {
            m.filhas.push((filha, fk));
        }
    }
    for s in &app.screens {
        if let Screen::MasterDetail {
            master,
            detail,
            detail_fk,
            totals,
            ..
        } = s
            && let Some(m) = v.iter_mut().find(|e| &e.nome == master)
        {
            for t in totals {
                m.totais.push(Total {
                    filha: detail.clone(),
                    fk: detail_fk.clone(),
                    campo: t.sum_of.clone(),
                    rotulo: t.label.clone(),
                });
            }
        }
    }
    v
}

/// "pedido_item" -> "PedidoItem"
pub fn pascal(nome: &str) -> String {
    nome.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

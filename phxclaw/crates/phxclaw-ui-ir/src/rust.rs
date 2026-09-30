//! Backend Rust das regras de negocio: gera um crate so com `std` (structs por entidade,
//! validacao, banco em memoria com integridade referencial e totais).
//!
//! E o destino final do codigo das telas: o WLanguage gerado do mesmo modelo
//! (`wlanguage.rs`) serve para rodar no WinDev hoje, e este e o que fica. Os dois leem
//! `regras::de`, com os mesmos nomes de funcao e as mesmas mensagens.

use crate::ir::App;
use crate::regras::{self, Campo, Entidade, Tipo};

const PALAVRAS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "box", "yield",
];

fn id(nome: &str) -> String {
    if PALAVRAS.contains(&nome) {
        format!("r#{nome}")
    } else {
        nome.to_string()
    }
}

/// Opcional: nao exigido, ou coluna de auditoria (o banco preenche).
fn opcional(e: &Entidade, c: &Campo) -> bool {
    c.nome != e.chave
        && !matches!(c.tipo, Tipo::Texto | Tipo::Logico)
        && (!c.obrigatorio || c.somente_leitura)
}

fn tipo(e: &Entidade, c: &Campo) -> String {
    if c.nome == e.chave {
        return "i64".into();
    }
    let base = match c.tipo {
        Tipo::Texto => "String",
        Tipo::Inteiro => "i64",
        Tipo::Decimal => "f64",
        Tipo::Dinheiro => "Centavos",
        Tipo::Data => "Data",
        Tipo::DataHora => "DataHora",
        Tipo::Logico => "bool",
    };
    if opcional(e, c) {
        format!("Option<{base}>")
    } else {
        base.into()
    }
}

pub fn render(app: &App) -> Vec<(String, String)> {
    let ents = regras::de(app);
    let nome: String = app
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let cargo = format!(
        "[package]\nname = \"{}_regras\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n# projeto proprio: nao herda o workspace de quem o gerou\n[workspace]\n\n[dependencies]\n",
        nome.trim_matches('_')
    );
    vec![
        ("Cargo.toml".into(), cargo),
        ("src/lib.rs".into(), lib(app, &ents)),
    ]
}

fn lib(app: &App, ents: &[Entidade]) -> String {
    let ent = |n: &str| ents.iter().find(|e| e.nome == n);
    let mut s = format!(
        "//! GERADO do UI-IR v{} pelo PhxClaw: regras de negocio de {:?}.\n\
         //! Espelho do WLanguage gerado do mesmo modelo: mesmos nomes, mesmas mensagens.\n\
         #![forbid(unsafe_code)]\n#![allow(clippy::all)]\n\n{BASE}\n",
        app.ir_version, app.name
    );
    for e in ents {
        let st = regras::pascal(&e.nome);
        s.push_str(&format!(
            "/// {}\n#[derive(Debug, Clone, Default, PartialEq)]\npub struct {st} {{\n",
            e.rotulo
        ));
        for c in &e.campos {
            s.push_str(&format!(
                "    /// {}\n    pub {}: {},\n",
                c.rotulo,
                id(&c.nome),
                tipo(e, c)
            ));
        }
        s.push_str("}\n\n");
        // validar: preenchimento, tamanho, opcao, data -- nesta ordem, campo a campo
        s.push_str(&format!(
            "impl {st} {{\n    pub fn validar(&self) -> Result<(), String> {{\n"
        ));
        for c in &e.campos {
            if c.nome == e.chave || c.somente_leitura {
                continue;
            }
            let f = format!("self.{}", id(&c.nome));
            let op = opcional(e, c);
            match c.tipo {
                Tipo::Texto => {
                    if c.exige_preenchimento() {
                        s.push_str(&format!(
                            "        if {f}.trim().is_empty() {{ return Err({:?}.into()); }}\n",
                            regras::msg_obrigatorio(c)
                        ));
                    }
                    if let Some(n) = c.max_len {
                        s.push_str(&format!(
                            "        if {f}.chars().count() > {n} {{ return Err({:?}.into()); }}\n",
                            regras::msg_tamanho(c, n)
                        ));
                    }
                    if !c.opcoes.is_empty() {
                        s.push_str(&format!(
                            "        if !{f}.is_empty() && ![{}].contains(&{f}.as_str()) {{ return Err({:?}.into()); }}\n",
                            c.opcoes.iter().map(|o| format!("{o:?}")).collect::<Vec<_>>().join(", "),
                            regras::msg_opcao(c)
                        ));
                    }
                }
                Tipo::Data | Tipo::DataHora => {
                    if op {
                        s.push_str(&format!(
                            "        if let Some(d) = &{f} {{ if !d.valida() {{ return Err({:?}.into()); }} }}\n",
                            regras::msg_data(c)
                        ));
                    } else {
                        s.push_str(&format!(
                            "        if {f}.vazia() {{ return Err({:?}.into()); }}\n        if !{f}.valida() {{ return Err({:?}.into()); }}\n",
                            regras::msg_obrigatorio(c),
                            regras::msg_data(c)
                        ));
                    }
                }
                _ => {}
            }
        }
        s.push_str("        Ok(())\n    }\n}\n\n");
    }

    // o banco: um Vec por entidade, e a sequencia de cada chave automatica
    s.push_str("/// Banco em memoria com as garantias do modelo. A sequencia nunca volta atras:\n/// codigo de registro excluido nao se reaproveita.\n#[derive(Debug, Default)]\npub struct Banco {\n");
    for e in ents {
        s.push_str(&format!(
            "    pub {}: Vec<{}>,\n",
            id(&e.nome),
            regras::pascal(&e.nome)
        ));
        if e.chave_automatica {
            s.push_str(&format!("    seq_{}: i64,\n", e.nome));
        }
    }
    s.push_str("}\n\nimpl Banco {\n");
    for e in ents {
        let st = regras::pascal(&e.nome);
        let tab = id(&e.nome);
        let k = id(&e.chave);
        // incluir
        s.push_str(&format!(
            "    /// Valida, confere cada pai e grava. Devolve o codigo.\n    pub fn incluir_{}(&mut self, mut r: {st}) -> Result<i64, String> {{\n        r.validar()?;\n",
            e.nome
        ));
        for c in &e.campos {
            if let Some((mae, chave_mae)) = &c.mae
                && let Some(m) = ent(mae)
            {
                let msg = regras::msg_sem_pai(c, m);
                let (tm, km) = (id(&m.nome), id(chave_mae));
                if opcional(e, c) {
                    s.push_str(&format!(
                        "        if let Some(v) = r.{} {{ if !self.{tm}.iter().any(|m| m.{km} == v) {{ return Err({msg:?}.into()); }} }}\n",
                        id(&c.nome)
                    ));
                } else {
                    s.push_str(&format!(
                        "        if !self.{tm}.iter().any(|m| m.{km} == r.{}) {{ return Err({msg:?}.into()); }}\n",
                        id(&c.nome)
                    ));
                }
            }
        }
        if e.chave_automatica {
            s.push_str(&format!(
                "        self.seq_{0} += 1;\n        r.{k} = self.seq_{0};\n",
                e.nome
            ));
        } else {
            s.push_str(&format!(
                "        if self.{tab}.iter().any(|x| x.{k} == r.{k}) {{ return Err({:?}.into()); }}\n",
                regras::msg_chave_repetida(e)
            ));
        }
        s.push_str(&format!(
            "        let codigo = r.{k};\n        self.{tab}.push(r);\n        Ok(codigo)\n    }}\n\n"
        ));
        // excluir: restringir sempre
        s.push_str(&format!(
            "    /// Recusa se algum filho aponta para o registro (restringir, nunca cascata).\n    pub fn excluir_{}(&mut self, codigo: i64) -> Result<(), String> {{\n",
            e.nome
        ));
        for (filha, fk) in &e.filhas {
            if let Some(f) = ent(filha)
                && let Some(cf) = f.campos.iter().find(|c| &c.nome == fk)
            {
                let cmp = if opcional(f, cf) {
                    format!("x.{} == Some(codigo)", id(fk))
                } else {
                    format!("x.{} == codigo", id(fk))
                };
                s.push_str(&format!(
                    "        if self.{}.iter().any(|x| {cmp}) {{ return Err({:?}.into()); }}\n",
                    id(&f.nome),
                    regras::msg_tem_filhos(e, f)
                ));
            }
        }
        s.push_str(&format!(
            "        let antes = self.{tab}.len();\n        self.{tab}.retain(|x| x.{k} != codigo);\n        if self.{tab}.len() == antes {{ return Err({:?}.into()); }}\n        Ok(())\n    }}\n\n",
            regras::msg_nao_encontrado(e)
        ));
        // totais do mestre/detalhe
        for t in &e.totais {
            if let Some(f) = ent(&t.filha) {
                let cfk = f.campos.iter().find(|c| c.nome == t.fk);
                let cv = f.campos.iter().find(|c| c.nome == t.campo);
                if let (Some(cfk), Some(cv)) = (cfk, cv) {
                    let filtro = if opcional(f, cfk) {
                        format!("i.{} == Some(codigo)", id(&t.fk))
                    } else {
                        format!("i.{} == codigo", id(&t.fk))
                    };
                    let soma = if opcional(f, cv) {
                        format!("filter_map(|i| i.{})", id(&t.campo))
                    } else {
                        format!("map(|i| i.{})", id(&t.campo))
                    };
                    s.push_str(&format!(
                        "    /// {}\n    pub fn total_{}_{}(&self, codigo: i64) -> Centavos {{\n        self.{}.iter().filter(|i| {filtro}).{soma}.sum()\n    }}\n\n",
                        t.rotulo,
                        e.nome,
                        t.campo,
                        id(&f.nome)
                    ));
                }
            }
        }
    }
    s.push_str("}\n");
    s
}

const BASE: &str = r#"/// Dinheiro em centavos: R$ 12,34 e 1234.
pub type Centavos = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Data {
    pub dia: u8,
    pub mes: u8,
    pub ano: i32,
}

impl Data {
    pub fn nova(dia: u8, mes: u8, ano: i32) -> Self {
        Data { dia, mes, ano }
    }
    pub fn vazia(&self) -> bool {
        *self == Data::default()
    }
    /// Existe no calendario gregoriano (31/02 nao existe; 29/02 so em ano bissexto).
    pub fn valida(&self) -> bool {
        let bis = (self.ano % 4 == 0 && self.ano % 100 != 0) || self.ano % 400 == 0;
        let dias = match self.mes {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => if bis { 29 } else { 28 },
            _ => 0,
        };
        self.ano >= 1 && self.dia >= 1 && self.dia <= dias
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataHora {
    pub data: Data,
    pub hora: u8,
    pub minuto: u8,
}

impl DataHora {
    pub fn vazia(&self) -> bool {
        *self == DataHora::default()
    }
    pub fn valida(&self) -> bool {
        self.data.valida() && self.hora < 24 && self.minuto < 60
    }
}
"#;

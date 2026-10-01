//! PHX JSON: o ENVELOPE de intercambio do Phoenix (`app.phx.json`, chaves em ingles
//! camelCase), lido para o UI-IR e escrito de volta. A decisao do papel J
//! (`docs/ui/STUDIO_UI_R03.md`) vale aqui: o PHX JSON e o envelope, o UI-IR e o tipo dentro.
//!
//! Por isso ha UMA representacao interna do layout -- a do UI-IR v3 (`LayoutIntencao`,
//! `quebra_da_casca_px`, `tokens`) -- e este modulo so TRADUZ (`basis` <-> `base`,
//! `minWidthPx` <-> `min_largura_px`, `spacing.md` <-> `md`). O que o UI-IR nao representa
//! (versao do contrato, adaptador pedido, valores iniciais, templates, componentes,
//! simulacao) fica no `Envelope`, para a escrita devolver o mesmo arquivo.
//!
//! O subconjunto suportado e o do exemplo do dono (01/10/2026): um `Panel` com uma
//! `Collection` em grade de cartoes. Campo desconhecido, campo faltando e tipo errado sao
//! recusados COM O CAMINHO (`$.view.children[0].layout.foo`): aceitar calado o que nao se le
//! seria perder o campo na escrita de volta sem ninguem saber.

use crate::ir::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Versao do contrato que este leitor entende.
pub const VERSAO_DO_CONTRATO: &str = "0.1.0";

// ------------------------------------------------------------------ forma (com caminho)

enum Forma {
    Obj(&'static [(&'static str, Forma)]),
    Lista(&'static Forma),
    /// Objeto de chaves livres com valor texto (o `spacing`).
    MapaTexto,
    Texto,
    Inteiro,
    Logico,
}

static REGRA: Forma = Forma::Obj(&[("minWidthPx", Forma::Inteiro), ("columns", Forma::Inteiro)]);
static AGENTE: Forma = Forma::Obj(&[
    ("uuid", Forma::Texto),
    ("name", Forma::Texto),
    ("role", Forma::Texto),
    ("initials", Forma::Texto),
    ("tone", Forma::Texto),
    ("task", Forma::Texto),
    ("capabilities", Forma::Lista(&Forma::Texto)),
    ("detail", Forma::Texto),
]);
static FILHO: Forma = Forma::Obj(&[
    ("uuid", Forma::Texto),
    ("type", Forma::Texto),
    (
        "items",
        Forma::Obj(&[("binding", Forma::Texto), ("key", Forma::Texto)]),
    ),
    (
        "templateRef",
        Forma::Obj(&[("uuid", Forma::Texto), ("version", Forma::Texto)]),
    ),
    (
        "layout",
        Forma::Obj(&[
            ("type", Forma::Texto),
            ("gap", Forma::Texto),
            ("columns", Forma::Inteiro),
            (
                "responsive",
                Forma::Obj(&[
                    ("basis", Forma::Texto),
                    ("target", Forma::Texto),
                    ("rules", Forma::Lista(&REGRA)),
                ]),
            ),
        ]),
    ),
]);
static TEMPLATE: Forma = Forma::Obj(&[
    ("uuid", Forma::Texto),
    ("version", Forma::Texto),
    ("type", Forma::Texto),
    ("events", Forma::Obj(&[("activate", Forma::Texto)])),
]);
static COMPONENTE: Forma = Forma::Obj(&[("uuid", Forma::Texto), ("type", Forma::Texto)]);
static RAIZ: Forma = Forma::Obj(&[
    ("schemaVersion", Forma::Texto),
    ("uuid", Forma::Texto),
    ("name", Forma::Texto),
    ("version", Forma::Texto),
    ("demo", Forma::Logico),
    (
        "ui",
        Forma::Obj(&[
            ("adapter", Forma::Texto),
            ("adapterVersion", Forma::Texto),
            ("theme", Forma::Texto),
            ("shellBreakpointPx", Forma::Inteiro),
        ]),
    ),
    (
        "tokens",
        Forma::Obj(&[
            ("spacing", Forma::MapaTexto),
            ("accent", Forma::Texto),
            ("radius", Forma::Texto),
        ]),
    ),
    (
        "defaults",
        Forma::Obj(&[
            ("projectName", Forma::Texto),
            ("mission", Forma::Texto),
            ("density", Forma::Texto),
        ]),
    ),
    (
        "view",
        Forma::Obj(&[
            ("uuid", Forma::Texto),
            ("type", Forma::Texto),
            ("name", Forma::Texto),
            ("queryContainer", Forma::Texto),
            ("children", Forma::Lista(&FILHO)),
        ]),
    ),
    ("templates", Forma::Lista(&TEMPLATE)),
    ("agents", Forma::Lista(&AGENTE)),
    ("components", Forma::Lista(&COMPONENTE)),
    (
        "simulation",
        Forma::Obj(&[
            ("stepDurationMs", Forma::Inteiro),
            ("allowNetwork", Forma::Logico),
            ("allowShell", Forma::Logico),
            ("allowModels", Forma::Logico),
        ]),
    ),
]);

fn conferir_forma(v: &Value, f: &Forma, caminho: &str) -> Result<(), String> {
    let tipo = |esperado: &str| format!("{caminho}: esperava {esperado}");
    match f {
        Forma::Obj(campos) => {
            let o = v.as_object().ok_or_else(|| tipo("objeto"))?;
            for k in o.keys() {
                if !campos.iter().any(|(c, _)| c == k) {
                    return Err(format!(
                        "{caminho}.{k}: campo desconhecido (fora do subconjunto suportado)"
                    ));
                }
            }
            for (c, sub) in campos.iter() {
                let x = o
                    .get(*c)
                    .ok_or_else(|| format!("{caminho}.{c}: campo obrigatorio ausente"))?;
                conferir_forma(x, sub, &format!("{caminho}.{c}"))?;
            }
        }
        Forma::Lista(sub) => {
            let l = v.as_array().ok_or_else(|| tipo("lista"))?;
            for (i, x) in l.iter().enumerate() {
                conferir_forma(x, sub, &format!("{caminho}[{i}]"))?;
            }
        }
        Forma::MapaTexto => {
            let o = v.as_object().ok_or_else(|| tipo("objeto"))?;
            for (k, x) in o {
                if !x.is_string() {
                    return Err(format!("{caminho}.{k}: esperava texto"));
                }
            }
        }
        Forma::Texto if !v.is_string() => return Err(tipo("texto")),
        Forma::Inteiro if !v.is_u64() => return Err(tipo("inteiro nao negativo")),
        Forma::Logico if !v.is_boolean() => return Err(tipo("logico")),
        _ => {}
    }
    Ok(())
}

// ------------------------------------------------------------------ o fio (ordem do arquivo)

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fio {
    schema_version: String,
    uuid: String,
    name: String,
    version: String,
    demo: bool,
    ui: FioUi,
    tokens: FioTokens,
    defaults: Padroes,
    view: FioView,
    templates: Vec<Template>,
    agents: Vec<FioAgente>,
    components: Vec<Componente>,
    simulation: Simulacao,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FioUi {
    adapter: String,
    adapter_version: String,
    theme: String,
    shell_breakpoint_px: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioTokens {
    spacing: BTreeMap<String, String>,
    accent: String,
    radius: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FioView {
    uuid: String,
    #[serde(rename = "type")]
    tipo: String,
    name: String,
    query_container: String,
    children: Vec<FioColecao>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FioColecao {
    uuid: String,
    #[serde(rename = "type")]
    tipo: String,
    items: FioItens,
    template_ref: FioRef,
    layout: FioLayout,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioItens {
    binding: String,
    key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioRef {
    uuid: String,
    version: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioLayout {
    #[serde(rename = "type")]
    tipo: String,
    gap: String,
    columns: u32,
    responsive: FioResponsivo,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioResponsivo {
    basis: String,
    target: String,
    rules: Vec<FioRegra>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FioRegra {
    min_width_px: u32,
    columns: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FioAgente {
    uuid: String,
    name: String,
    role: String,
    initials: String,
    tone: String,
    task: String,
    capabilities: Vec<String>,
    detail: String,
}

// ------------------------------------------------------------------ o envelope

/// O que o PHX JSON carrega e o UI-IR nao representa. Viaja ao lado do `App` para a
/// escrita devolver o mesmo arquivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub versao_do_contrato: String,
    pub uuid: String,
    pub versao: String,
    pub demo: bool,
    /// Adaptador e versao pedidos (`bootstrap`, `5.3.6`) e o tema inicial.
    pub adaptador: String,
    pub versao_do_adaptador: String,
    pub tema: String,
    pub padroes: Padroes,
    pub templates: Vec<Template>,
    pub componentes: Vec<Componente>,
    pub simulacao: Simulacao,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Padroes {
    pub project_name: String,
    pub mission: String,
    pub density: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub uuid: String,
    pub version: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub events: Eventos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Eventos {
    pub activate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Componente {
    pub uuid: String,
    #[serde(rename = "type")]
    pub tipo: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Simulacao {
    pub step_duration_ms: u32,
    pub allow_network: bool,
    pub allow_shell: bool,
    pub allow_models: bool,
}

// ------------------------------------------------------------------ traducao

fn uuid_v7(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 36
        && [8, 13, 18, 23].iter().all(|i| b[*i] == b'-')
        && s.chars()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || c.is_ascii_hexdigit())
        && b[14] == b'7'
        && matches!(b[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

/// `spacing.md` -> `md`, so com token conhecido.
fn gap_do_fio(g: &str) -> Option<String> {
    g.strip_prefix("spacing.")
        .filter(|t| crate::responsivo::GAPS.contains(t))
        .map(String::from)
}

/// PHX JSON -> (envelope, UI-IR v3). Confere forma (com caminho), identidade (UUIDv7 unico,
/// referencia de template), o subconjunto e, por fim, o mesmo `responsivo::validar` de todo
/// IR.
pub fn ler(texto: &str) -> Result<(Envelope, App), String> {
    let v: Value = serde_json::from_str(texto).map_err(|e| format!("PHX JSON invalido: {e}"))?;
    conferir_forma(&v, &RAIZ, "$")?;
    let f: Fio = serde_json::from_value(v).map_err(|e| format!("PHX JSON: {e}"))?;
    if f.schema_version != VERSAO_DO_CONTRATO {
        return Err(format!(
            "$.schemaVersion: «{}» nao suportado (este leitor le {VERSAO_DO_CONTRATO})",
            f.schema_version
        ));
    }
    let mut ids: Vec<(String, &str)> = vec![
        ("$.uuid".into(), f.uuid.as_str()),
        ("$.view.uuid".into(), f.view.uuid.as_str()),
    ];
    ids.extend(
        f.view
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("$.view.children[{i}].uuid"), c.uuid.as_str())),
    );
    ids.extend(
        f.templates
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("$.templates[{i}].uuid"), c.uuid.as_str())),
    );
    ids.extend(
        f.agents
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("$.agents[{i}].uuid"), c.uuid.as_str())),
    );
    ids.extend(
        f.components
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("$.components[{i}].uuid"), c.uuid.as_str())),
    );
    for (i, (onde, id)) in ids.iter().enumerate() {
        if !uuid_v7(id) {
            return Err(format!("{onde}: «{id}» nao e UUIDv7"));
        }
        if ids[..i].iter().any(|(_, x)| x == id) {
            return Err(format!("{onde}: identidade «{id}» repetida"));
        }
    }
    if f.view.tipo != "Panel" {
        return Err(format!(
            "$.view.type: «{}» fora do subconjunto (so Panel)",
            f.view.tipo
        ));
    }
    let [c] = f.view.children.as_slice() else {
        return Err("$.view.children: o subconjunto tem exatamente uma Collection".into());
    };
    let p = "$.view.children[0]";
    if c.tipo != "Collection" {
        return Err(format!(
            "{p}.type: «{}» fora do subconjunto (so Collection)",
            c.tipo
        ));
    }
    if c.items.binding != "state.agents" || c.items.key != "uuid" {
        return Err(format!(
            "{p}.items: so `state.agents` por `uuid` (os cartoes vem de $.agents)"
        ));
    }
    if !f
        .templates
        .iter()
        .any(|t| t.uuid == c.template_ref.uuid && t.version == c.template_ref.version)
    {
        return Err(format!(
            "{p}.templateRef: template nao encontrado em $.templates"
        ));
    }
    if c.layout.tipo != "grid" {
        return Err(format!(
            "{p}.layout.type: «{}» fora do subconjunto (so grid)",
            c.layout.tipo
        ));
    }
    let gap = gap_do_fio(&c.layout.gap).ok_or_else(|| {
        format!(
            "{p}.layout.gap: «{}» nao e spacing.<{}>",
            c.layout.gap,
            crate::responsivo::GAPS.join("|")
        )
    })?;
    let base = match c.layout.responsive.basis.as_str() {
        "container" => BaseResponsiva::Conteiner,
        "window" => BaseResponsiva::Janela,
        outro => {
            return Err(format!(
                "{p}.layout.responsive.basis: «{outro}» (container ou window)"
            ));
        }
    };
    for (i, a) in f.agents.iter().enumerate() {
        if !["amber", "violet", "blue", "green"].contains(&a.tone.as_str()) {
            return Err(format!("$.agents[{i}].tone: «{}» fora da paleta", a.tone));
        }
    }
    let s = &f.simulation;
    if s.allow_network || s.allow_shell || s.allow_models {
        return Err("$.simulation: rede, shell e modelo nao sao permitidos".into());
    }
    let mut tokens = BTreeMap::new();
    for (k, x) in &f.tokens.spacing {
        if !crate::responsivo::GAPS.contains(&k.as_str()) {
            return Err(format!("$.tokens.spacing.{k}: token desconhecido"));
        }
        tokens.insert(format!("gap-{k}"), x.clone());
    }
    tokens.insert("acento".into(), f.tokens.accent.clone());
    tokens.insert("raio".into(), f.tokens.radius.clone());
    let colecao = Colecao {
        id: c.uuid.clone(),
        origem: c.items.binding.clone(),
        chave: c.items.key.clone(),
        template: c.template_ref.uuid.clone(),
        template_versao: c.template_ref.version.clone(),
        layout: LayoutIntencao {
            tipo: TipoLayout::Grade,
            gap,
            colunas: c.layout.columns,
            responsivo: Some(Responsivo {
                base,
                alvo: Some(c.layout.responsive.target.clone()),
                regras: c
                    .layout
                    .responsive
                    .rules
                    .iter()
                    .map(|r| RegraResponsiva {
                        min_largura_px: r.min_width_px,
                        colunas: r.columns,
                    })
                    .collect(),
            }),
        },
        itens: f
            .agents
            .iter()
            .map(|a| Cartao {
                id: a.uuid.clone(),
                titulo: a.name.clone(),
                subtitulo: a.role.clone(),
                sigla: a.initials.clone(),
                tom: a.tone.clone(),
                tarefa: a.task.clone(),
                capacidades: a.capabilities.clone(),
                detalhe: a.detail.clone(),
            })
            .collect(),
    };
    let app = App {
        name: f.name.clone(),
        ir_version: IR_VERSION,
        entities: vec![],
        screens: vec![Screen::Painel {
            id: f.view.uuid.clone(),
            title: f.view.name.clone(),
            conteiner: f.view.query_container.clone(),
            colecao,
        }],
        menu: vec![MenuGroup {
            title: "Painéis".into(),
            screens: vec![f.view.uuid.clone()],
        }],
        layouts: vec![],
        quebra_da_casca_px: Some(f.ui.shell_breakpoint_px),
        tokens,
    };
    // o mesmo portao de todo IR: nome de conteiner, token, regras e quebra da casca
    crate::responsivo::validar(&app).map_err(|e| format!("PHX JSON -> UI-IR: {e}"))?;
    let env = Envelope {
        versao_do_contrato: f.schema_version,
        uuid: f.uuid,
        versao: f.version,
        demo: f.demo,
        adaptador: f.ui.adapter,
        versao_do_adaptador: f.ui.adapter_version,
        tema: f.ui.theme,
        padroes: f.defaults,
        templates: f.templates,
        componentes: f.components,
        simulacao: f.simulation,
    };
    Ok((env, app))
}

/// (envelope, UI-IR) -> PHX JSON, na ordem e no recuo do arquivo do dono (2 espacos,
/// quebra de linha no fim). O layout sai do `App`, nunca de uma copia guardada.
pub fn escrever(env: &Envelope, app: &App) -> Result<String, String> {
    let [
        Screen::Painel {
            id,
            title,
            conteiner,
            colecao,
        },
    ] = app.screens.as_slice()
    else {
        return Err("PHX JSON: o subconjunto escreve um app com exatamente um painel".into());
    };
    let l = &colecao.layout;
    let r = l
        .responsivo
        .as_ref()
        .ok_or("PHX JSON: a colecao precisa de regra responsiva")?;
    if l.tipo != TipoLayout::Grade {
        return Err("PHX JSON: o subconjunto so escreve layout grid".into());
    }
    let spacing: BTreeMap<String, String> = app
        .tokens
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("gap-").map(|g| (g.to_string(), v.clone())))
        .collect();
    let token = |k: &str| {
        app.tokens
            .get(k)
            .cloned()
            .ok_or_else(|| format!("PHX JSON: falta o token {k}"))
    };
    let f = Fio {
        schema_version: env.versao_do_contrato.clone(),
        uuid: env.uuid.clone(),
        name: app.name.clone(),
        version: env.versao.clone(),
        demo: env.demo,
        ui: FioUi {
            adapter: env.adaptador.clone(),
            adapter_version: env.versao_do_adaptador.clone(),
            theme: env.tema.clone(),
            shell_breakpoint_px: app
                .quebra_da_casca_px
                .ok_or("PHX JSON: falta a quebra da casca")?,
        },
        tokens: FioTokens {
            spacing,
            accent: token("acento")?,
            radius: token("raio")?,
        },
        defaults: env.padroes.clone(),
        view: FioView {
            uuid: id.clone(),
            tipo: "Panel".into(),
            name: title.clone(),
            query_container: conteiner.clone(),
            children: vec![FioColecao {
                uuid: colecao.id.clone(),
                tipo: "Collection".into(),
                items: FioItens {
                    binding: colecao.origem.clone(),
                    key: colecao.chave.clone(),
                },
                template_ref: FioRef {
                    uuid: colecao.template.clone(),
                    version: colecao.template_versao.clone(),
                },
                layout: FioLayout {
                    tipo: "grid".into(),
                    gap: format!("spacing.{}", l.gap),
                    columns: l.colunas,
                    responsive: FioResponsivo {
                        basis: match r.base {
                            BaseResponsiva::Conteiner => "container",
                            BaseResponsiva::Janela => "window",
                        }
                        .into(),
                        target: r.alvo.clone().unwrap_or_default(),
                        rules: r
                            .regras
                            .iter()
                            .map(|g| FioRegra {
                                min_width_px: g.min_largura_px,
                                columns: g.colunas,
                            })
                            .collect(),
                    },
                },
            }],
        },
        templates: env.templates.clone(),
        agents: colecao
            .itens
            .iter()
            .map(|k| FioAgente {
                uuid: k.id.clone(),
                name: k.titulo.clone(),
                role: k.subtitulo.clone(),
                initials: k.sigla.clone(),
                tone: k.tom.clone(),
                task: k.tarefa.clone(),
                capabilities: k.capacidades.clone(),
                detail: k.detalhe.clone(),
            })
            .collect(),
        components: env.componentes.clone(),
        simulation: env.simulacao.clone(),
    };
    serde_json::to_string_pretty(&f)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

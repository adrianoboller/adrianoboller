//! Os tres artefatos que saem do catalogo -- gerados, nunca digitados, e o teste
//! `artefatos_gerados_estao_em_dia` reprova se o arquivo em disco envelheceu:
//!
//! - `schemas/config.schema.json` (JSON Schema draft 2020-12), para editor e validador;
//! - `schemas/config.exemplo.json`, cada secao com o comentario `"//"` das chaves;
//! - `apps/phxclaw-ui/assets/config-catalogo.json`, o que a tela de configuracao le.
//!
//! Regerar: `cargo run -p phxclaw-config-runtime --example gerar_config`.

use super::carga::{do_texto, CAMPO_COMENTARIO, CAMPO_REVISAO};
use super::catalogo::{catalogo, Chave, Natureza, Tipo};
use serde_json::{json, Map, Value};

/// Os arquivos gerados, relativos a raiz do repositorio.
pub const ESQUEMA: &str = "schemas/config.schema.json";
pub const EXEMPLO: &str = "schemas/config.exemplo.json";
pub const CATALOGO_UI: &str = "apps/phxclaw-ui/assets/config-catalogo.json";

/// O padrao tipado (o catalogo guarda a forma do ambiente).
pub fn padrao(c: &Chave) -> Value {
    c.padrao
        .map(|p| do_texto(c, p).expect("padrao do catalogo conferido no teste"))
        .unwrap_or(Value::Null)
}

fn opcoes(c: &Chave) -> Value {
    match c.tipo {
        Tipo::Enum(o) => json!(o),
        _ => Value::Null,
    }
}

fn esquema_da_chave(c: &Chave) -> Value {
    let mut m = Map::new();
    let tipo = match c.tipo {
        Tipo::Texto | Tipo::Caminho => json!(["string", "null"]),
        Tipo::Inteiro => json!(["integer", "null"]),
        Tipo::Real => json!(["number", "null"]),
        Tipo::Booleano => json!(["boolean", "null"]),
        Tipo::Lista(_) => json!(["array", "null"]),
        Tipo::Enum(_) => Value::Null,
    };
    match c.tipo {
        Tipo::Enum(o) => {
            let mut e: Vec<Value> = o.iter().map(|x| json!(x)).collect();
            e.push(Value::Null);
            m.insert("enum".into(), Value::Array(e));
        }
        Tipo::Lista(_) => {
            m.insert("type".into(), tipo);
            m.insert("items".into(), json!({"type": "string"}));
        }
        _ => {
            m.insert("type".into(), tipo);
        }
    }
    m.insert(
        "description".into(),
        json!(format!("{} (ambiente: {})", c.descricao, c.variavel)),
    );
    let p = padrao(c);
    if !p.is_null() {
        m.insert("default".into(), p);
    }
    Value::Object(m)
}

fn secao_vazia() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("type".into(), json!("object"));
    m.insert("additionalProperties".into(), json!(false));
    let mut props = Map::new();
    props.insert(
        CAMPO_COMENTARIO.into(),
        json!({"type": ["string", "array"], "items": {"type": "string"}}),
    );
    m.insert("properties".into(), Value::Object(props));
    m
}

/// Insere `valor` no caminho `a.b.c` de um objeto de schema (secoes em `properties`).
fn por_no_esquema(raiz: &mut Map<String, Value>, chave: &str, valor: Value) {
    let partes: Vec<&str> = chave.split('.').collect();
    let mut atual = raiz;
    for p in &partes[..partes.len() - 1] {
        let props = atual
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .expect("secao com properties");
        atual = props
            .entry(p.to_string())
            .or_insert_with(|| Value::Object(secao_vazia()))
            .as_object_mut()
            .expect("secao e objeto");
    }
    atual
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("secao com properties")
        .insert(partes[partes.len() - 1].to_string(), valor);
}

/// So as chaves que moram no arquivo entram: segredo e chave de ambiente ficam de fora,
/// e o `additionalProperties: false` as recusa no editor tambem.
pub fn esquema() -> Value {
    let mut raiz = secao_vazia();
    raiz.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    raiz.insert("$id".into(), json!("phxclaw/config.schema.json"));
    raiz.insert("title".into(), json!("PhxClaw: config.json do agente"));
    raiz.insert(
        "description".into(),
        json!(
            "Gerado do catalogo (phxclaw-config-runtime/src/agente/catalogo.rs). Precedencia: \
             ambiente > .phxclaw/config.json do projeto confiado > <pasta>/config.json > padrao. \
             Segredo nunca entra aqui: vai para o SecretBroker."
        ),
    );
    {
        let props = raiz
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .expect("properties");
        props.insert(
            CAMPO_REVISAO.into(),
            json!({"type": "integer", "minimum": 0, "description": "Revisao do arquivo (a gravacao sobe de um)"}),
        );
        props.insert("$schema".into(), json!({"type": "string"}));
    }
    for c in catalogo().iter().filter(|c| c.no_arquivo()) {
        por_no_esquema(&mut raiz, &c.chave, esquema_da_chave(c));
    }
    Value::Object(raiz)
}

/// O exemplo: cada chave do arquivo com o padrao (ou `null`, «nao definido»), e cada secao
/// com o comentario `"//"` das chaves dela.
pub fn exemplo() -> Value {
    let mut raiz = Map::new();
    raiz.insert(
        CAMPO_COMENTARIO.into(),
        json!([
            "config.json do PhxClaw, gerado do catalogo (phxclaw config exemplo).",
            "Precedencia: ambiente > .phxclaw/config.json do projeto confiado > <pasta>/config.json > padrao.",
            "null = nao definido aqui. Segredo nunca entra: vai para o SecretBroker (phxclaw config mostrar diz o comando)."
        ]),
    );
    for c in catalogo().iter().filter(|c| c.no_arquivo()) {
        let partes: Vec<&str> = c.chave.split('.').collect();
        let mut atual = &mut raiz;
        for p in &partes[..partes.len() - 1] {
            atual = atual
                .entry(p.to_string())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .expect("secao e objeto");
        }
        let folha = partes[partes.len() - 1];
        let nota = format!(
            "{folha}: {} [{}{}; ambiente {}]",
            c.descricao,
            c.tipo.nome(),
            match c.tipo {
                Tipo::Enum(o) => format!(" {}", o.join("|")),
                _ => String::new(),
            },
            c.variavel
        );
        match atual
            .entry(CAMPO_COMENTARIO.to_string())
            .or_insert_with(|| Value::Array(vec![]))
        {
            Value::Array(a) => a.push(Value::String(nota)),
            _ => unreachable!("comentario da secao e lista"),
        }
        atual.insert(folha.to_string(), padrao(c));
    }
    Value::Object(raiz)
}

/// Uma entrada do catalogo como a tela e a API a mostram (sem valor efetivo).
pub fn entrada(c: &Chave) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("chave".into(), json!(c.chave));
    m.insert("secao".into(), json!(c.secao()));
    m.insert("tipo".into(), json!(c.tipo.nome()));
    m.insert("opcoes".into(), opcoes(c));
    m.insert("descricao".into(), json!(c.descricao));
    m.insert(
        "padrao".into(),
        if c.segredo() { Value::Null } else { padrao(c) },
    );
    m.insert("variavel".into(), json!(c.variavel));
    m.insert("segredo".into(), json!(c.segredo()));
    let (natureza, comando, motivo) = match &c.natureza {
        Natureza::Config => ("config", None, None),
        Natureza::Segredo { comando, .. } => ("segredo", Some(comando.clone()), None),
        Natureza::Ambiente { motivo } => ("ambiente", None, Some(*motivo)),
    };
    m.insert("natureza".into(), json!(natureza));
    m.insert("comando_do_segredo".into(), json!(comando));
    m.insert("motivo_so_ambiente".into(), json!(motivo));
    if let Tipo::Lista(sep) = c.tipo {
        m.insert("separador_no_ambiente".into(), json!(sep.to_string()));
    }
    m
}

pub fn catalogo_ui() -> Value {
    json!({
        "gerado_por": "cargo run -p phxclaw-config-runtime --example gerar_config",
        "chaves": catalogo().iter().map(|c| Value::Object(entrada(c))).collect::<Vec<_>>(),
    })
}

/// Os tres artefatos, (caminho relativo a raiz, conteudo), na forma exata do disco.
pub fn artefatos() -> Vec<(&'static str, String)> {
    let texto = |v: Value| {
        let mut s = serde_json::to_string_pretty(&v).expect("json");
        s.push('\n');
        s
    };
    vec![
        (ESQUEMA, texto(esquema())),
        (EXEMPLO, texto(exemplo())),
        (CATALOGO_UI, texto(catalogo_ui())),
    ]
}

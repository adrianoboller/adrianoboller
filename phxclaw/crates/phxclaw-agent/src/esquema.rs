//! Validador de esquema dos argumentos de ferramenta, escrito aqui e sem crate nova.
//!
//! Por que proprio: a crate `jsonschema` traz uma arvore inteira para as palavras-chave
//! que os esquemas desta casa usam de verdade -- medidas no catalogo montado
//! (`tests/guardas.rs`, `todo_esquema_nativo_usa_so_palavras_conferidas`), sao `type`,
//! `properties`, `required`, `enum`, `items`, `minimum` e `maximum`; `pattern` entra porque
//! servidor MCP o usa e o `regex` ja e dependencia. Palavra que nao esta aqui (esquema de
//! MCP com `oneOf`, `format`...) NAO reprova: passa, e a lista volta em
//! `nao_conferidas` para a mensagem dizer o que ninguem conferiu.
//!
//! O que veio do PydanticAI, depois de ler o fonte: validar ANTES de a ferramenta rodar,
//! devolver TODOS os erros de uma vez, cada um com o caminho do campo, e o modo «lax» de
//! coercao. Onde divergimos, e por que:
//!
//! - **Coercao so do que nenhuma ferramenta daqui le diferente.** Texto numerico vira
//!   numero quando o esquema pede `integer`/`number` (cinco ferramentas ja aceitavam "5"
//!   por conta propria, medido no fonte); "true"/"false" vira booleano. Numero NAO vira
//!   texto: ferramenta que le `as_u64` num campo declarado texto quebraria calada.
//! - **`null` em campo opcional e ausencia.** Toda ferramenta daqui le `get(..).and_then`,
//!   onde `null` e ausente; reprovar `null` seria piorar um caso que hoje passa.
//! - **Faixa (`minimum`/`maximum`) reprova, nao corta.** E a regra que o `sistema.rs` ja
//!   escreveu: o modelo pediu outra coisa e precisa saber. As quatro ferramentas que
//!   CORTAM em silencio (`web_search` e `doc_search` em `max_results`, `memory_search`
//!   em `limit`, `node_invoke` em `timeout_s`) dizem a faixa na `description`, e nao em
//!   `minimum`/`maximum`: com a palavra, o portao reprovaria o 20 que elas aceitavam como 10.

use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// As palavras que este validador confere. A catraca dos testes reprova esquema nativo
/// que use palavra fora desta lista e da de anotacao: o validador nao pode ficar para tras
/// do catalogo sem ninguem ver.
pub const PALAVRAS_CONFERIDAS: &[&str] = &[
    "type",
    "properties",
    "required",
    "enum",
    "items",
    "minimum",
    "maximum",
    "pattern",
];

/// Palavras que so anotam e nao restringem: nao se conferem e nao viram nota.
pub const PALAVRAS_DE_ANOTACAO: &[&str] = &[
    "description",
    "title",
    "default",
    "examples",
    "$schema",
    "$id",
    "$comment",
];

/// Um erro de argumento: o caminho do campo (`blocks[0].level`), o que veio e o esperado.
/// Os nomes em ingles sao do FIO: o modelo le este JSON, e o prompt do motor e ingles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErroDeArgumento {
    #[serde(rename = "field")]
    pub campo: String,
    #[serde(rename = "got")]
    pub veio: String,
    #[serde(rename = "expected")]
    pub esperado: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Validacao {
    pub erros: Vec<ErroDeArgumento>,
    /// Palavras do esquema que ninguem conferiu (esquema de MCP): passam, com nota.
    pub nao_conferidas: BTreeSet<String>,
    /// Os argumentos depois da coercao «lax»: e ESTE valor que a ferramenta recebe.
    pub valor: Value,
}

impl Validacao {
    pub fn ok(&self) -> bool {
        self.erros.is_empty()
    }
}

/// Confere `valor` contra `esquema`, devolvendo todos os erros e o valor coagido.
pub fn validar(esquema: &Value, valor: &Value) -> Validacao {
    let mut v = Validacao {
        valor: valor.clone(),
        ..Default::default()
    };
    // Argumento que chega como TEXTO de JSON (adaptador que nao abriu o objeto) se abre
    // aqui: a ferramenta ja falharia com ele, entao abrir nunca piora.
    if let Value::String(s) = &v.valor
        && aceita(esquema, "object")
        && !aceita(esquema, "string")
        && let Ok(o @ Value::Object(_)) = serde_json::from_str::<Value>(s)
    {
        v.valor = o;
    }
    // Ferramenta sem argumento costuma chegar com `null`: para elas, e o objeto vazio.
    if v.valor.is_null() && aceita(esquema, "object") && !aceita(esquema, "null") {
        v.valor = json!({});
    }
    let mut valor = std::mem::take(&mut v.valor);
    conferir(esquema, &mut valor, "", &mut v.erros, &mut v.nao_conferidas);
    v.valor = valor;
    v
}

/// O texto que volta ao modelo: todos os erros em JSON e a ordem de corrigir. Um lugar so,
/// porque o portao das ferramentas e o `final_answer` tipado devolvem o MESMO formato.
pub fn mensagem_ao_modelo(ferramenta: &str, v: &Validacao) -> String {
    let mut corpo = json!({"tool": ferramenta, "errors": v.erros});
    if !v.nao_conferidas.is_empty() {
        corpo["note"] = json!(format!(
            "schema keywords not checked here (the tool checks them): {}",
            v.nao_conferidas
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    format!(
        "INVALID ARGUMENTS for {ferramenta}: {} problem(s). Fix EVERY field listed below and call \
{ferramenta} again with the complete arguments (correct it and try again).\n{corpo}",
        v.erros.len()
    )
}

fn tipos(esquema: &Value) -> Option<Vec<&str>> {
    match esquema.get("type")? {
        Value::String(t) => Some(vec![t.as_str()]),
        Value::Array(a) => Some(a.iter().filter_map(Value::as_str).collect()),
        _ => None,
    }
}

fn aceita(esquema: &Value, tipo: &str) -> bool {
    tipos(esquema).is_none_or(|t| t.contains(&tipo))
}

fn tem_tipo(v: &Value, t: &str) -> bool {
    match t {
        "string" => v.is_string(),
        "boolean" => v.is_boolean(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        "null" => v.is_null(),
        "number" => v.is_number(),
        // JSON Schema: 3.0 e inteiro; o que importa e o valor, nao a grafia.
        "integer" => {
            v.is_i64()
                || v.is_u64()
                || v.as_f64()
                    .is_some_and(|f| f.fract() == 0.0 && f.is_finite())
        }
        _ => true,
    }
}

/// O que veio, curto: o tipo e o valor cortado. Texto inteiro de um campo grande nao ajuda
/// o modelo a achar o erro e come a janela dele.
fn descrever(v: &Value) -> String {
    let tipo = match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    let mut t = v.to_string();
    if t.chars().count() > 60 {
        t = t.chars().take(57).collect::<String>() + "...";
    }
    format!("{tipo} {t}")
}

fn caminho_de(base: &str, campo: &str) -> String {
    if base.is_empty() {
        campo.to_string()
    } else {
        format!("{base}.{campo}")
    }
}

fn nome_do_caminho(c: &str) -> String {
    if c.is_empty() { "$".into() } else { c.into() }
}

/// Coercao «lax», so a que nunca le diferente do que a ferramenta ja lia (ver o topo).
fn coagir(esquema: &Value, v: &mut Value) {
    let Some(ts) = tipos(esquema) else { return };
    if ts.iter().any(|t| tem_tipo(v, t)) || ts.contains(&"string") {
        return;
    }
    if let Value::String(s) = v {
        let s = s.trim();
        if ts.contains(&"integer") {
            if let Ok(i) = s.parse::<i64>() {
                *v = json!(i);
                return;
            }
            if let Ok(u) = s.parse::<u64>() {
                *v = json!(u);
                return;
            }
        }
        if ts.contains(&"number")
            && let Ok(f) = s.parse::<f64>()
            && f.is_finite()
        {
            *v = json!(f);
            return;
        }
        if ts.contains(&"boolean") {
            match s {
                "true" => *v = json!(true),
                "false" => *v = json!(false),
                _ => {}
            }
        }
    }
}

fn conferir(
    esquema: &Value,
    v: &mut Value,
    caminho: &str,
    erros: &mut Vec<ErroDeArgumento>,
    nao: &mut BTreeSet<String>,
) {
    let Some(obj) = esquema.as_object() else {
        // `true`/`false` como esquema (draft 6+): `false` nao aceita nada.
        if esquema == &Value::Bool(false) {
            erros.push(ErroDeArgumento {
                campo: nome_do_caminho(caminho),
                veio: descrever(v),
                esperado: "nothing (field not allowed)".into(),
            });
        }
        return;
    };
    for k in obj.keys() {
        if !PALAVRAS_CONFERIDAS.contains(&k.as_str()) && !PALAVRAS_DE_ANOTACAO.contains(&k.as_str())
        {
            nao.insert(k.clone());
        }
    }
    coagir(esquema, v);
    if let Some(ts) = tipos(esquema) {
        for t in &ts {
            if !matches!(
                *t,
                "string" | "boolean" | "object" | "array" | "null" | "number" | "integer"
            ) {
                nao.insert(format!("type:{t}"));
            }
        }
        if !ts.iter().any(|t| tem_tipo(v, t)) {
            erros.push(ErroDeArgumento {
                campo: nome_do_caminho(caminho),
                veio: descrever(v),
                esperado: ts.join(" or "),
            });
            // Tipo errado: as outras palavras falariam do valor errado, so fariam ruido.
            return;
        }
    } else if obj.contains_key("type") {
        nao.insert("type".into());
    }
    if let Some(lista) = obj.get("enum").and_then(Value::as_array)
        && !lista.contains(v)
    {
        erros.push(ErroDeArgumento {
            campo: nome_do_caminho(caminho),
            veio: descrever(v),
            esperado: format!("one of {}", Value::Array(lista.clone())),
        });
    }
    if let Some(n) = v.as_f64() {
        if let Some(min) = obj.get("minimum").and_then(Value::as_f64)
            && n < min
        {
            erros.push(ErroDeArgumento {
                campo: nome_do_caminho(caminho),
                veio: descrever(v),
                esperado: format!(">= {}", obj["minimum"]),
            });
        }
        if let Some(max) = obj.get("maximum").and_then(Value::as_f64)
            && n > max
        {
            erros.push(ErroDeArgumento {
                campo: nome_do_caminho(caminho),
                veio: descrever(v),
                esperado: format!("<= {}", obj["maximum"]),
            });
        }
    }
    if let (Some(p), Some(s)) = (obj.get("pattern").and_then(Value::as_str), v.as_str()) {
        // ECMA 262 e `regex` concordam no comum; o que nao compila nao se confere e vira
        // nota, em vez de reprovar por um dialeto que nao e do modelo.
        match regex::Regex::new(p) {
            Ok(re) if !re.is_match(s) => erros.push(ErroDeArgumento {
                campo: nome_do_caminho(caminho),
                veio: descrever(v),
                esperado: format!("string matching /{p}/"),
            }),
            Ok(_) => {}
            Err(_) => {
                nao.insert("pattern".into());
            }
        }
    }
    if let Value::Object(campos) = v {
        let exigidos: Vec<&str> = obj
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let props = obj.get("properties").and_then(Value::as_object);
        for r in &exigidos {
            if !campos.contains_key(*r) {
                let tipo = props
                    .and_then(|p| p.get(*r))
                    .and_then(tipos)
                    .map(|t| format!(" ({})", t.join(" or ")))
                    .unwrap_or_default();
                // A `description` do campo vai junto: o campo que FALTA o modelo nao ve no que
                // mandou, e «relative path ending in .docx» diz o que por ali.
                let dica = props
                    .and_then(|p| p.get(*r))
                    .and_then(|x| x.get("description"))
                    .and_then(Value::as_str)
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default();
                erros.push(ErroDeArgumento {
                    campo: caminho_de(caminho, r),
                    veio: "missing".into(),
                    esperado: format!("required field{tipo}{dica}"),
                });
            }
        }
        if let Some(props) = props {
            for (nome, sub) in props {
                let Some(x) = campos.get_mut(nome) else {
                    continue;
                };
                if x.is_null() && !exigidos.contains(&nome.as_str()) {
                    continue;
                }
                conferir(sub, x, &caminho_de(caminho, nome), erros, nao);
            }
        }
    }
    if let Value::Array(itens) = v {
        match obj.get("items") {
            Some(sub @ (Value::Object(_) | Value::Bool(_))) => {
                for (i, x) in itens.iter_mut().enumerate() {
                    conferir(sub, x, &format!("{caminho}[{i}]"), erros, nao);
                }
            }
            Some(_) => {
                nao.insert("items".into());
            }
            None => {}
        }
    }
}

/// As palavras-chave de um esquema, recursivo, separando nome de campo (dentro de
/// `properties`) de palavra: e a medida que a catraca usa.
pub fn palavras_usadas(esquema: &Value, saida: &mut std::collections::BTreeMap<String, usize>) {
    let Some(obj) = esquema.as_object() else {
        return;
    };
    for (k, x) in obj {
        *saida.entry(k.clone()).or_default() += 1;
        match k.as_str() {
            "properties" => {
                if let Some(p) = x.as_object() {
                    for sub in p.values() {
                        palavras_usadas(sub, saida);
                    }
                }
            }
            "items" => palavras_usadas(x, saida),
            _ => {}
        }
    }
}

/// Le o valor final de uma resposta tipada: o JSON pedido, ou o texto que o contem
/// (modelo pequeno devolve o objeto como TEXTO, as vezes cercado de ```).
pub fn resposta_tipada(esquema: &Value, resposta: &Value) -> Validacao {
    let candidato = match resposta {
        Value::String(s) if !aceita(esquema, "string") => {
            extrair_json(s).unwrap_or(resposta.clone())
        }
        _ => resposta.clone(),
    };
    validar(esquema, &candidato)
}

fn extrair_json(s: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(s.trim()) {
        return Some(v);
    }
    let (ini, fim) = match (s.find(['{', '[']), s.rfind(['}', ']'])) {
        (Some(i), Some(f)) if f > i => (i, f),
        _ => return None,
    };
    serde_json::from_str(&s[ini..=fim]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        json!({"type":"object","properties":{
            "path":{"type":"string","description":"relative path ending in .docx"},
            "blocks":{"type":"array","items":{"type":"object","properties":{
                "type":{"type":"string","enum":["heading","paragraph","bullets","table"]},
                "level":{"type":"integer","minimum":1,"maximum":3},
                "rows":{"type":"array","items":{"type":"array","items":{"type":["string","number"]}}}
            },"required":["type"]}}
        },"required":["path","blocks"]})
    }

    fn campos(v: &Validacao) -> Vec<(String, String)> {
        v.erros
            .iter()
            .map(|e| (e.campo.clone(), e.esperado.clone()))
            .collect()
    }

    /// A tabela de casos: (argumentos, erros esperados por (campo, esperado)).
    #[test]
    fn tabela_de_casos() {
        let casos: Vec<(Value, Vec<(&str, &str)>)> = vec![
            (json!({"path":"a.docx","blocks":[]}), vec![]),
            (
                json!({"path":"a.docx","blocks":[{"type":"heading","level":"um"}]}),
                vec![("blocks[0].level", "integer")],
            ),
            (
                json!({"path":"a.docx","blocks":[{"type":"heading","level":7}]}),
                vec![("blocks[0].level", "<= 3")],
            ),
            (
                json!({"path":"a.docx","blocks":[{"type":"titulo","level":0}]}),
                // A ordem e a dos campos no esquema (alfabetica no `serde_json::Map`).
                vec![
                    ("blocks[0].level", ">= 1"),
                    (
                        "blocks[0].type",
                        r#"one of ["heading","paragraph","bullets","table"]"#,
                    ),
                ],
            ),
            (
                json!({"answer":"x"}),
                vec![
                    (
                        "path",
                        "required field (string): relative path ending in .docx",
                    ),
                    ("blocks", "required field (array)"),
                ],
            ),
            (
                json!({"path":3,"blocks":{}}),
                vec![("blocks", "array"), ("path", "string")],
            ),
            (
                json!({"path":"a","blocks":[{"type":"table","rows":[[1,"x",true]]}]}),
                vec![("blocks[0].rows[0][2]", "string or number")],
            ),
            (json!([1]), vec![("$", "object")]),
        ];
        for (args, esperado) in casos {
            let v = validar(&doc(), &args);
            let achado = campos(&v);
            let esperado: Vec<(String, String)> = esperado
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect();
            assert_eq!(achado, esperado, "argumentos {args}");
        }
    }

    #[test]
    fn coercao_lax_so_do_que_a_ferramenta_ja_aceitava() {
        let e = json!({"type":"object","properties":{
            "n":{"type":"integer"},"x":{"type":"number"},"b":{"type":"boolean"},"s":{"type":"string"}
        }});
        let v = validar(&e, &json!({"n":" 5 ","x":"2.5","b":"true","s":5}));
        assert_eq!(campos(&v), vec![("s".into(), "string".into())]);
        assert_eq!(v.valor["n"], json!(5));
        assert_eq!(v.valor["x"], json!(2.5));
        assert_eq!(v.valor["b"], json!(true));
        // Numero NAO vira texto: o erro aparece, e o valor fica como veio.
        assert_eq!(v.valor["s"], json!(5));
        // Texto que nao e numero continua erro.
        assert!(!validar(&e, &json!({"n":"cinco"})).ok());
    }

    #[test]
    fn null_em_opcional_e_ausencia_e_em_obrigatorio_e_erro() {
        let e = json!({"type":"object","properties":{"q":{"type":"string"},"max":{"type":"integer"}},"required":["q"]});
        assert!(validar(&e, &json!({"q":"a","max":null})).ok());
        let v = validar(&e, &json!({"q":null}));
        assert_eq!(campos(&v), vec![("q".into(), "string".into())]);
        // Argumento nulo de ferramenta sem obrigatorio e o objeto vazio.
        assert!(validar(&json!({"type":"object","properties":{}}), &Value::Null).ok());
    }

    #[test]
    fn palavra_desconhecida_passa_com_nota() {
        let e = json!({"type":"object","properties":{"d":{"type":"string","format":"date","oneOf":[]}},
                       "additionalProperties":false});
        let v = validar(&e, &json!({"d":"qualquer","extra":1}));
        assert!(v.ok());
        assert_eq!(
            v.nao_conferidas.iter().cloned().collect::<Vec<_>>(),
            vec!["additionalProperties", "format", "oneOf"]
        );
        let m = mensagem_ao_modelo(
            "t",
            &Validacao {
                erros: vec![ErroDeArgumento {
                    campo: "d".into(),
                    veio: "missing".into(),
                    esperado: "x".into(),
                }],
                ..v
            },
        );
        assert!(m.contains("not checked here") && m.contains("oneOf"), "{m}");
    }

    #[test]
    fn pattern_confere_e_o_que_nao_compila_vira_nota() {
        let e = json!({"type":"string","pattern":"^[a-z]+$"});
        assert!(validar(&e, &json!("abc")).ok());
        assert_eq!(
            campos(&validar(&e, &json!("ab1")))[0].1,
            "string matching /^[a-z]+$/"
        );
        let v = validar(&json!({"type":"string","pattern":"(?<=a)b"}), &json!("x"));
        assert!(v.ok() && v.nao_conferidas.contains("pattern"));
    }

    #[test]
    fn argumento_em_texto_de_json_se_abre() {
        let e = json!({"type":"object","properties":{"a":{"type":"integer"}},"required":["a"]});
        let v = validar(&e, &json!("{\"a\": 1}"));
        assert!(v.ok(), "{:?}", v.erros);
        assert_eq!(v.valor, json!({"a":1}));
    }

    #[test]
    fn todos_os_erros_vem_juntos_com_caminho_veio_e_esperado() {
        let v = validar(
            &doc(),
            &json!({"path":"a.docx","blocks":[{"type":"heading","level":"1x"},{"type":"x"}]}),
        );
        let m = mensagem_ao_modelo("create_document", &v);
        let j: Value = serde_json::from_str(m.split_once('\n').unwrap().1).unwrap();
        assert_eq!(j["errors"].as_array().unwrap().len(), 2, "{m}");
        assert_eq!(j["errors"][0]["field"], "blocks[0].level");
        assert_eq!(j["errors"][0]["got"], "string \"1x\"");
        assert_eq!(j["errors"][0]["expected"], "integer");
        assert!(m.contains("try again"), "{m}");
    }

    #[test]
    fn resposta_tipada_aceita_objeto_ou_texto_que_o_contem() {
        let e =
            json!({"type":"object","properties":{"total":{"type":"number"}},"required":["total"]});
        assert!(resposta_tipada(&e, &json!({"total": 3})).ok());
        let v = resposta_tipada(&e, &json!("Aqui:\n```json\n{\"total\": 4}\n```"));
        assert!(v.ok(), "{:?}", v.erros);
        assert_eq!(v.valor, json!({"total":4}));
        assert!(!resposta_tipada(&e, &json!("sem json")).ok());
        assert!(!resposta_tipada(&e, &json!({"soma": 1})).ok());
    }

    #[test]
    fn palavras_usadas_separa_campo_de_palavra() {
        let mut m = std::collections::BTreeMap::new();
        palavras_usadas(
            &json!({"type":"object","properties":{"pattern":{"type":"string"},"type":{"type":"string"}}}),
            &mut m,
        );
        assert_eq!(m.get("pattern"), None, "nome de campo nao e palavra");
        assert_eq!(m["type"], 3);
    }
}

//! `phxclaw ferramenta NOME [--PARAM valor]... [--json ARQ|-]`: qualquer ferramenta montada,
//! pela linha de comando, com os parametros montados A PARTIR DO ESQUEMA dela.
//!
//! Por que um motor e nao um subcomando por ferramenta: sao dezenas, cada uma com o seu
//! esquema, e o esquema ja e a fonte do que a ferramenta aceita -- o modelo le o mesmo.
//! Um subcomando digitado por ferramenta seria uma segunda descricao dos parametros, e a
//! que envelhecesse mandaria o operador passar um campo que a ferramenta nao le mais.
//!
//! A chamada passa pelo `Agent::call_tool` numa `mcp::Sessao`: o MESMO portao do laco do
//! modelo e do `mcp-serve` (capacidade conferida, esquema validado, regras de comando,
//! hooks, evidencia no ledger). Daqui nao sai atalho: esta porta so traduz `--x valor` em
//! JSON e imprime o que o portao devolveu. Obrigatorio, enum e faixa NAO se conferem aqui
//! -- quem decide e o validador do portao (`esquema::validar`), e conferir de novo seria a
//! segunda copia da decisao.
//!
//! O argv NAO e lugar de segredo: ele aparece em `ps`, em `/proc/<pid>/cmdline` de qualquer
//! usuario da maquina e no historico do shell. Valor com forma de credencial (pelo motor
//! unico `phxclaw_types::segredo`, o mesmo da entrada de fluxo e da tarja do broker) e
//! recusado no `--PARAM`, dizendo o caminho certo: `--json -` (a entrada padrao, que nao fica
//! em lugar nenhum) ou o NOME de uma credencial guardada no broker.

use anyhow::{Result, bail};
use phxclaw_agent::Agent;
use phxclaw_agent_core::{ToolCall, ToolSpec};
use serde_json::{Map, Value};
use std::io::Read;
use std::path::PathBuf;

/// As opcoes do proprio comando. Parametro de ferramenta com um destes nomes nao teria
/// forma na CLI (a opcao o engoliria): a catraca dos testes reprova a ferramenta que nascer
/// assim, em vez de deixar o campo inalcancavel calado.
pub const OPCOES_PROPRIAS: &[(&str, &str, &str)] = &[
    (
        "json",
        "ARQ|-",
        "argumentos em JSON (objeto), de um arquivo ou da entrada padrao; --PARAM por cima",
    ),
    (
        "trabalho",
        "DIR",
        "pasta de trabalho da chamada (padrao: a atual)",
    ),
    (
        "modelo",
        "M",
        "modelo da montagem (padrao: modelo.padrao); muda so o que depende do modelo",
    ),
    (
        "pasta",
        "DIR",
        "pasta do agente (tarefas, evidencia, broker)",
    ),
    ("ajuda", "", "os parametros do esquema desta ferramenta"),
    ("help", "", "o mesmo que --ajuda"),
];

/// Como um parametro do esquema vira texto de linha de comando.
#[derive(Debug, Clone, PartialEq)]
pub enum Forma {
    Texto,
    Inteiro,
    Real,
    /// `--x` sozinho e `true`; `--x true|false` tambem vale.
    Booleano,
    /// `--x a --x b`: cada ocorrencia e um item, na forma do item. `--x '[...]'` (JSON)
    /// tambem vale, para a lista vazia e para quem ja tem o JSON pronto.
    Lista(Box<Forma>),
    /// Objeto, lista de objetos ou tipo misto: o valor e JSON. Texto que nao e JSON
    /// valido entra como texto (tipo misto com `string`).
    Json,
}

impl Forma {
    fn de(esquema: &Value) -> Forma {
        let tipos: Vec<&str> = match esquema.get("type") {
            Some(Value::String(t)) => vec![t.as_str()],
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .filter(|t| *t != "null")
                .collect(),
            _ => vec![],
        };
        match tipos.as_slice() {
            ["string"] => Forma::Texto,
            ["integer"] => Forma::Inteiro,
            ["number"] => Forma::Real,
            ["boolean"] => Forma::Booleano,
            ["array"] => match esquema.get("items").map(Forma::de) {
                Some(f @ (Forma::Texto | Forma::Inteiro | Forma::Real | Forma::Booleano)) => {
                    Forma::Lista(Box::new(f))
                }
                _ => Forma::Json,
            },
            _ => Forma::Json,
        }
    }

    /// O marcador do valor na ajuda.
    pub fn rotulo(&self) -> String {
        match self {
            Forma::Texto => "TEXTO".into(),
            Forma::Inteiro => "N".into(),
            Forma::Real => "REAL".into(),
            Forma::Booleano => "[true|false]".into(),
            Forma::Lista(f) => format!("{}...", f.rotulo()),
            Forma::Json => "JSON".into(),
        }
    }

    fn converter(&self, nome: &str, bruto: &str) -> Result<Value, String> {
        match self {
            Forma::Texto => Ok(Value::String(bruto.to_string())),
            Forma::Inteiro => bruto
                .parse::<i64>()
                .map(Value::from)
                .map_err(|_| format!("--{nome}: {bruto:?} nao e inteiro")),
            Forma::Real => bruto
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .ok_or_else(|| format!("--{nome}: {bruto:?} nao e numero")),
            Forma::Booleano => match bruto {
                "true" | "sim" => Ok(Value::Bool(true)),
                "false" | "nao" => Ok(Value::Bool(false)),
                _ => Err(format!("--{nome}: {bruto:?} nao e true|false")),
            },
            Forma::Lista(f) => f.converter(nome, bruto),
            Forma::Json => Ok(
                serde_json::from_str(bruto).unwrap_or_else(|_| Value::String(bruto.to_string()))
            ),
        }
    }
}

/// Um parametro do esquema, como a ajuda o mostra.
#[derive(Debug, Clone)]
pub struct Parametro {
    pub nome: String,
    pub forma: Forma,
    pub obrigatorio: bool,
    pub opcoes: Vec<String>,
    pub padrao: Option<Value>,
    pub descricao: String,
}

/// Os parametros de primeiro nivel do esquema: os obrigatorios na ordem do `required`,
/// depois os outros em ordem alfabetica (o mapa do `serde_json` nao guarda a declaracao).
pub fn parametros(spec: &ToolSpec) -> Vec<Parametro> {
    let obrigatorios: Vec<&str> = spec
        .parameters
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(props) = spec.parameters.get("properties").and_then(Value::as_object) else {
        return vec![];
    };
    let mut v: Vec<Parametro> = props
        .iter()
        .map(|(nome, e)| Parametro {
            nome: nome.clone(),
            forma: Forma::de(e),
            obrigatorio: obrigatorios.contains(&nome.as_str()),
            opcoes: e
                .get("enum")
                .or_else(|| e.get("items").and_then(|i| i.get("enum")))
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            padrao: e.get("default").cloned(),
            descricao: e
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .collect();
    let ordem = |p: &Parametro| {
        obrigatorios
            .iter()
            .position(|o| *o == p.nome)
            .unwrap_or(usize::MAX)
    };
    v.sort_by_key(|p| ordem(p));
    v
}

/// Por que o esquema nao tem forma na CLI, se nao tiver. A catraca chama isto para toda
/// ferramenta montada; o comando, antes de montar argumentos.
pub fn sem_forma(spec: &ToolSpec) -> Option<String> {
    let p = &spec.parameters;
    if p.get("type").and_then(Value::as_str) != Some("object") {
        return Some(format!(
            "{}: o esquema nao e objeto (type={}); os argumentos nao viram --PARAM",
            spec.name,
            p.get("type").unwrap_or(&Value::Null)
        ));
    }
    for par in parametros(spec) {
        if let Some((o, ..)) = OPCOES_PROPRIAS.iter().find(|(o, ..)| *o == par.nome) {
            return Some(format!(
                "{}: o parametro `{}` colide com a opcao --{o} do proprio comando",
                spec.name, par.nome
            ));
        }
        if par.nome.is_empty()
            || par.nome.starts_with('-')
            || par.nome.chars().any(|c| c.is_whitespace() || c == '=')
        {
            return Some(format!(
                "{}: o parametro {:?} nao cabe numa opcao --NOME",
                spec.name, par.nome
            ));
        }
    }
    None
}

/// A linha curta de uso: `NOME --a TEXTO [--b N]...`.
pub fn uso(spec: &ToolSpec) -> String {
    let mut s = format!("ferramenta {}", spec.name);
    for p in parametros(spec) {
        let v = match p.forma {
            Forma::Booleano => String::new(),
            ref f => format!(" {}", f.rotulo()),
        };
        if p.obrigatorio {
            s.push_str(&format!(" --{}{v}", p.nome));
        } else {
            s.push_str(&format!(" [--{}{v}]", p.nome));
        }
    }
    s.push_str(" [--json ARQ|-]");
    s
}

/// A ajuda inteira de uma ferramenta, do esquema dela. A catraca confere que todo
/// parametro do esquema aparece aqui com tipo, obrigatoriedade, opcoes e padrao.
pub fn ajuda(spec: &ToolSpec, capacidade: &str, concedida: bool, cli: &str) -> String {
    let mut s = format!(
        "{}\n\nUSO:\n  {cli} {}\n\nCAPACIDADE: {capacidade} ({})\n",
        spec.description.trim(),
        uso(spec),
        if concedida {
            "concedida"
        } else {
            "NAO concedida: o portao recusa; ver PHXCLAW_CAPACIDADES"
        }
    );
    let ps = parametros(spec);
    if ps.is_empty() {
        s.push_str("\nPARAMETROS: nenhum\n");
    } else {
        s.push_str("\nPARAMETROS:\n");
        for p in ps {
            s.push_str(&format!(
                "  --{} {}{}\n",
                p.nome,
                p.forma.rotulo(),
                if p.obrigatorio { "  (obrigatorio)" } else { "" }
            ));
            if !p.opcoes.is_empty() {
                s.push_str(&format!("      opcoes: {}\n", p.opcoes.join("|")));
            }
            if let Some(d) = &p.padrao {
                s.push_str(&format!("      padrao: {d}\n"));
            }
            if !p.descricao.is_empty() {
                s.push_str(&format!("      {}\n", p.descricao.replace('\n', " ")));
            }
        }
    }
    s.push_str("\nOPCOES DO COMANDO:\n");
    for (o, v, d) in OPCOES_PROPRIAS {
        let v = if v.is_empty() {
            String::new()
        } else {
            format!(" {v}")
        };
        s.push_str(&format!("  --{o}{v}  {d}\n"));
    }
    s
}

/// Recusa o valor do argv que tem forma de credencial. `alternativa` e o caminho que nao
/// passa pelo argv (`--json -`, `--corpo -`). A mensagem diz a opcao e o tamanho, nunca o
/// valor: o erro vai para o terminal e para o log de quem roteirizou o comando.
pub fn recusar_credencial_no_argv(
    opcao: &str,
    valor: &str,
    alternativa: &str,
) -> Result<(), String> {
    if phxclaw_types::segredo::texto_tem_credencial(valor) {
        return Err(format!(
            "{opcao}: valor com forma de credencial ({} caracteres) no argv, que fica em `ps` e no \
             historico do shell; mande pela entrada padrao com `{alternativa}`, ou use o nome de \
             uma credencial guardada no broker",
            valor.chars().count()
        ));
    }
    Ok(())
}

/// `--PARAM valor ...` (e `--json`) em argumentos. `entrada` e de onde `--json -` le.
/// Opcao desconhecida e erro com a sugestao mais proxima; obrigatorio, enum e faixa ficam
/// para o portao.
pub fn argumentos(
    spec: &ToolSpec,
    args: &[String],
    entrada: &mut dyn Read,
) -> Result<Value, String> {
    if let Some(m) = sem_forma(spec) {
        return Err(m);
    }
    let ps = parametros(spec);
    let mut obj = Map::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let Some(nome) = a.strip_prefix("--") else {
            return Err(format!(
                "argumento solto {a:?}: os parametros vao como --NOME valor"
            ));
        };
        let (nome, embutido) = match nome.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (nome, None),
        };
        if nome == "json" {
            let fonte = embutido
                .or_else(|| {
                    i += 1;
                    args.get(i).cloned()
                })
                .ok_or("--json pede ARQ ou -")?;
            let texto = if fonte == "-" {
                let mut t = String::new();
                entrada
                    .read_to_string(&mut t)
                    .map_err(|e| format!("--json -: {e}"))?;
                t
            } else {
                std::fs::read_to_string(&fonte).map_err(|e| format!("--json {fonte}: {e}"))?
            };
            match serde_json::from_str::<Value>(&texto) {
                // O arquivo e a base; --PARAM ja lido ganha (por isso nao sobrescreve).
                Ok(Value::Object(m)) => {
                    for (k, v) in m {
                        obj.entry(k).or_insert(v);
                    }
                }
                Ok(_) => return Err("--json: o JSON tem de ser um objeto".into()),
                Err(e) => return Err(format!("--json: {e}")),
            }
            i += 1;
            continue;
        }
        if matches!(nome, "trabalho" | "modelo" | "pasta") {
            // Opcoes do comando: o valor ja foi lido por quem montou o agente.
            i += if embutido.is_some() { 1 } else { 2 };
            continue;
        }
        let p = ps
            .iter()
            .find(|p| p.nome == nome)
            .or_else(|| ps.iter().find(|p| p.nome == nome.replace('-', "_")));
        let Some(p) = p else {
            let dica = ps
                .iter()
                .map(|p| (phxclaw_config_runtime::distancia(nome, &p.nome), &p.nome))
                .filter(|(d, _)| *d <= (nome.chars().count() / 3).max(1))
                .min_by_key(|(d, _)| *d)
                .map(|(_, n)| format!(" Quis dizer --{n}?"))
                .unwrap_or_default();
            return Err(format!(
                "{}: parametro desconhecido --{nome}.{dica} Rode `ferramenta {} --ajuda`.",
                spec.name, spec.name
            ));
        };
        let valor =
            match (&p.forma, embutido) {
                (_, Some(v)) => Some(v),
                // Booleano sem valor e `true`; so consome a proxima se ela for true/false.
                (Forma::Booleano, None) => match args.get(i + 1).map(String::as_str) {
                    Some(v @ ("true" | "false" | "sim" | "nao")) => {
                        i += 1;
                        Some(v.to_string())
                    }
                    _ => None,
                },
                (_, None) => {
                    i += 1;
                    Some(args.get(i).cloned().ok_or_else(|| {
                        format!("--{} pede um valor ({})", p.nome, p.forma.rotulo())
                    })?)
                }
            };
        if let Some(b) = &valor {
            recusar_credencial_no_argv(&format!("--{}", p.nome), b, "--json -")?;
        }
        let v = match valor {
            None => Value::Bool(true),
            Some(b) => match &p.forma {
                Forma::Lista(_) if b.trim_start().starts_with('[') => serde_json::from_str(&b)
                    .map_err(|e| format!("--{}: lista JSON invalida: {e}", p.nome))?,
                f => f.converter(&p.nome, &b)?,
            },
        };
        match (&p.forma, v) {
            (Forma::Lista(_), Value::Array(a)) => {
                obj.insert(p.nome.clone(), Value::Array(a));
            }
            (Forma::Lista(_), item) => {
                let lista = obj
                    .entry(p.nome.clone())
                    .or_insert_with(|| Value::Array(vec![]));
                match lista {
                    Value::Array(a) => a.push(item),
                    outro => *outro = Value::Array(vec![item]),
                }
            }
            (_, v) => {
                obj.insert(p.nome.clone(), v);
            }
        }
        i += 1;
    }
    Ok(Value::Object(obj))
}

fn quer_ajuda(args: &[String]) -> bool {
    args.iter()
        .any(|a| matches!(a.as_str(), "--ajuda" | "--help" | "-h"))
}

/// A lista de todas as ferramentas montadas, uma linha de uso cada.
pub fn lista(agente: &Agent, cli: &str) -> String {
    let mut s = format!(
        "{} ferramentas montadas nesta maquina. `{cli} ferramenta NOME --ajuda` mostra os \
         parametros de uma.\n\n",
        agente.tools.len()
    );
    for t in &agente.tools {
        let spec = t.spec();
        let cap = t.capability();
        let marca = if agente.config.capabilities.contains(cap) {
            ""
        } else {
            "  [capacidade nao concedida]"
        };
        s.push_str(&crate::ajuda::quebrar(
            &format!("{cli} {}{marca}", uso(&spec)),
            "  ",
            crate::ajuda::LARGURA,
        ));
    }
    s
}

pub fn montar(args: &[String], modelo_padrao: &str) -> Result<Agent> {
    let store = phxclaw_agent::TaskStore::new(super::pasta(args).join("tasks"))?;
    let modelo = super::opcao(args, "--modelo")
        .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
        .unwrap_or_else(|| modelo_padrao.into());
    phxclaw_agent::montagem::Montagem::new(store)
        .agent(&modelo)
        .map_err(anyhow::Error::msg)
}

/// O comando inteiro. Devolve o codigo de saida: 0 ok, 2 negado pelo portao, 1 o resto.
pub async fn comando(args: &[String], cli: &str, modelo_padrao: &str) -> Result<i32> {
    let agente = montar(args, modelo_padrao)?;
    // Para o completar do shell: so os nomes, um por linha.
    if args.first().map(String::as_str) == Some("--nomes") {
        for t in &agente.tools {
            println!("{}", t.spec().name);
        }
        return Ok(0);
    }
    let nome = args.first().filter(|a| !a.starts_with('-'));
    let Some(nome) = nome else {
        print!("{}", lista(&agente, cli));
        return Ok(0);
    };
    let Some(t) = agente.tools.iter().find(|t| t.spec().name == *nome) else {
        let nomes: Vec<String> = agente.tools.iter().map(|t| t.spec().name).collect();
        let dica = nomes
            .iter()
            .map(|n| (phxclaw_config_runtime::distancia(nome, n), n))
            .filter(|(d, _)| *d <= (nome.chars().count() / 3).max(1))
            .min_by_key(|(d, _)| *d)
            .map(|(_, n)| format!(" Quis dizer `{n}`?"))
            .unwrap_or_default();
        bail!(
            "ferramenta nao montada nesta maquina: {nome}.{dica} `{cli} ferramenta` lista as {}.",
            nomes.len()
        );
    };
    let spec = t.spec();
    let cap = t.capability();
    if quer_ajuda(&args[1..]) {
        print!(
            "{}",
            ajuda(&spec, cap, agente.config.capabilities.contains(cap), cli)
        );
        return Ok(0);
    }
    let argumentos =
        argumentos(&spec, &args[1..], &mut std::io::stdin().lock()).map_err(anyhow::Error::msg)?;
    let trabalho = match super::opcao(args, "--trabalho") {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir()?,
    };
    let sessao = phxclaw_agent::mcp::Sessao::abrir(&agente, trabalho)?;
    let chamada = ToolCall {
        id: "cli".into(),
        name: spec.name.clone(),
        arguments: argumentos,
    };
    let (texto, desfecho, artefatos) = agente
        .call_tool(&chamada, &sessao.ctx, &sessao.ledger, &sessao.ctx.task_id)
        .await;
    sessao.fechar(&agente).await;
    println!("{texto}");
    for a in &artefatos {
        eprintln!(
            "artefato: {} ({} bytes, sha256 {})",
            a.path, a.bytes, a.sha256
        );
    }
    eprintln!(
        "{desfecho}; evidencia em {}",
        agente.store.evidence_path(&sessao.ctx.task_id).display()
    );
    Ok(match desfecho {
        "ok" => 0,
        "negado" => 2,
        _ => 1,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn spec(parametros: Value) -> ToolSpec {
        ToolSpec {
            name: "t".into(),
            description: "d".into(),
            parameters: parametros,
        }
    }

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn monta_cada_tipo_pelo_esquema() {
        let s = spec(json!({"type":"object","properties":{
            "path":{"type":"string"},"n":{"type":"integer"},"x":{"type":"number"},
            "b":{"type":"boolean"},"tags":{"type":"array","items":{"type":"string"}},
            "obj":{"type":"object"},"old_string":{"type":"string"}},"required":["path"]}));
        let v = argumentos(
            &s,
            &a(&[
                "--path",
                "a.txt",
                "--n",
                "3",
                "--x",
                "1.5",
                "--b",
                "--tags",
                "p",
                "--tags",
                "q",
                "--obj",
                "{\"k\":1}",
                "--old-string",
                "z",
            ]),
            &mut std::io::empty(),
        )
        .unwrap();
        assert_eq!(
            v,
            json!({"path":"a.txt","n":3,"x":1.5,"b":true,"tags":["p","q"],"obj":{"k":1},
                   "old_string":"z"})
        );
    }

    #[test]
    fn json_e_a_base_e_o_parametro_ganha() {
        let s = spec(json!({"type":"object","properties":{"a":{"type":"string"},
            "b":{"type":"integer"}}}));
        let v = argumentos(
            &s,
            &a(&["--a", "linha", "--json", "-"]),
            &mut "{\"a\":\"arquivo\",\"b\":2}".as_bytes(),
        )
        .unwrap();
        assert_eq!(v, json!({"a":"linha","b":2}));
    }

    /// Credencial no argv e recusada sem ecoar o valor; a mesma pela entrada padrao passa
    /// (quem decide o que fazer com ela e o portao da ferramenta). RED medido com o
    /// `recusar_credencial_no_argv` do laco removido (defeito reposto): o `unwrap_err` estoura.
    #[test]
    fn credencial_no_argv_e_recusada_e_pela_entrada_padrao_passa() {
        let s = spec(
            json!({"type":"object","properties":{"texto":{"type":"string"},
            "lista":{"type":"array","items":{"type":"string"}}}}),
        );
        for chave in [
            "sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "ghp_0123456789abcdefghijABCDEFGHIJ0123",
            "https://usuario:senha123@exemplo.com/x",
            "Bearer abcdefghijklmnopqrstuvwxyz012345",
        ] {
            for args in [
                a(&["--texto", chave]),
                a(&[&format!("--texto={chave}")]),
                a(&["--lista", "ok", "--lista", chave]),
            ] {
                let e = argumentos(&s, &args, &mut std::io::empty()).unwrap_err();
                assert!(e.contains("--json -"), "{e}");
                assert!(!e.contains(chave), "a recusa ecoou o valor: {e}");
            }
            let v = argumentos(
                &s,
                &a(&["--json", "-"]),
                &mut json!({"texto": chave}).to_string().as_bytes(),
            )
            .unwrap();
            assert_eq!(v["texto"], chave);
        }
        // O comportamento velho: texto comum, sha de commit e `sk-SK` (locale) passam.
        for comum in [
            "ola mundo",
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
            "sk-SK",
        ] {
            assert!(argumentos(&s, &a(&["--texto", comum]), &mut std::io::empty()).is_ok());
        }
    }

    #[test]
    fn desconhecido_e_erro_com_sugestao() {
        let s = spec(json!({"type":"object","properties":{"path":{"type":"string"}}}));
        let e = argumentos(&s, &a(&["--pth", "x"]), &mut std::io::empty()).unwrap_err();
        assert!(e.contains("--pth") && e.contains("--path"), "{e}");
        let e = argumentos(&s, &a(&["solto"]), &mut std::io::empty()).unwrap_err();
        assert!(e.contains("solto"), "{e}");
    }

    /// Um valor de exemplo para o parametro, como o operador o digitaria e como o JSON
    /// tem de sair: a ida e volta prova que o parametro tem forma na CLI.
    fn exemplo(p: &Parametro, e: &Value) -> (Vec<String>, Value) {
        let flag = format!("--{}", p.nome);
        let escalar = |f: &Forma| -> (String, Value) {
            match f {
                Forma::Texto => {
                    let v = p.opcoes.first().cloned().unwrap_or_else(|| "valor".into());
                    (v.clone(), Value::String(v))
                }
                Forma::Inteiro => ("1".into(), json!(1)),
                Forma::Real => ("1.5".into(), json!(1.5)),
                Forma::Booleano => ("false".into(), json!(false)),
                Forma::Lista(_) | Forma::Json => unreachable!(),
            }
        };
        match &p.forma {
            Forma::Booleano => (vec![flag], json!(true)),
            Forma::Lista(f) => {
                let (t, v) = escalar(f);
                (
                    vec![flag.clone(), t.clone(), flag, t],
                    json!([v.clone(), v]),
                )
            }
            Forma::Json => match e.get("type").and_then(Value::as_str) {
                Some("object") => (vec![flag, "{\"k\":1}".into()], json!({"k":1})),
                Some("array") => (vec![flag, "[{\"k\":1}]".into()], json!([{"k":1}])),
                _ => (vec![flag, "valor".into()], json!("valor")),
            },
            f => {
                let (t, v) = escalar(f);
                (vec![flag, t], v)
            }
        }
    }

    /// As ferramentas que a CLI cobre nesta maquina: a MESMA montagem do comando.
    fn montadas() -> Vec<(ToolSpec, &'static str)> {
        let pasta = std::env::temp_dir().join(format!(
            "phxclaw-catraca-cli-{}-{}",
            std::process::id(),
            agora_ns()
        ));
        let store = phxclaw_agent::TaskStore::new(pasta.join("tasks")).unwrap();
        let agente = phxclaw_agent::montagem::Montagem::new(store)
            .agent("ollama:qwen2.5:1.5b")
            .unwrap();
        let v = agente
            .tools
            .iter()
            .map(|t| (t.spec(), t.capability()))
            .collect();
        let _ = std::fs::remove_dir_all(&pasta);
        v
    }

    fn agora_ns() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    /// A catraca: toda ferramenta montada tem forma na CLI (esquema objeto, nenhum
    /// parametro engolido por opcao do comando, cada um ida e volta pelo `argumentos`) e a
    /// ajuda mostra cada parametro com o tipo, a obrigatoriedade, as opcoes e o padrao.
    /// Ferramenta nova entra aqui sozinha, pela montagem: nao ha lista para esquecer.
    #[test]
    fn toda_ferramenta_montada_tem_forma_na_cli_e_ajuda_completa() {
        let ts = montadas();
        assert!(ts.len() >= 40, "montagem pequena demais: {}", ts.len());
        let mut falhas = Vec::new();
        for (spec, cap) in &ts {
            if let Some(m) = sem_forma(spec) {
                falhas.push(m);
                continue;
            }
            let texto = ajuda(spec, cap, true, "phxclaw");
            let linha_de_uso = uso(spec);
            let props = spec.parameters.get("properties").and_then(Value::as_object);
            let n_props = props.map_or(0, |p| p.len());
            let ps = parametros(spec);
            if ps.len() != n_props {
                falhas.push(format!(
                    "{}: {} parametros de {n_props}",
                    spec.name,
                    ps.len()
                ));
            }
            let mut args = Vec::new();
            let mut esperado = Map::new();
            for p in &ps {
                let e = &props.unwrap()[&p.nome];
                let cabeca = format!("  --{} {}", p.nome, p.forma.rotulo());
                if !texto.contains(&cabeca) {
                    falhas.push(format!("{}: a ajuda nao mostra {cabeca:?}", spec.name));
                }
                if !linha_de_uso.contains(&format!("--{}", p.nome)) {
                    falhas.push(format!("{}: o uso nao mostra --{}", spec.name, p.nome));
                }
                if p.obrigatorio && !texto.contains(&format!("{cabeca}  (obrigatorio)")) {
                    falhas.push(format!("{}: --{} sem (obrigatorio)", spec.name, p.nome));
                }
                if !p.opcoes.is_empty()
                    && !texto.contains(&format!("opcoes: {}", p.opcoes.join("|")))
                {
                    falhas.push(format!("{}: --{} sem as opcoes", spec.name, p.nome));
                }
                if let Some(d) = &p.padrao
                    && !texto.contains(&format!("padrao: {d}"))
                {
                    falhas.push(format!("{}: --{} sem o padrao", spec.name, p.nome));
                }
                let (a, v) = exemplo(p, e);
                args.extend(a);
                esperado.insert(p.nome.clone(), v);
            }
            match argumentos(spec, &args, &mut std::io::empty()) {
                Ok(v) if v == Value::Object(esperado.clone()) => {}
                Ok(v) => falhas.push(format!(
                    "{}: {args:?} virou {v}, esperado {}",
                    spec.name,
                    Value::Object(esperado)
                )),
                Err(e) => falhas.push(format!("{}: {e}", spec.name)),
            }
        }
        assert!(
            falhas.is_empty(),
            "{} falha(s) em {} ferramentas sem forma completa na CLI:\n{}",
            falhas.len(),
            ts.len(),
            falhas.join("\n")
        );
    }

    #[test]
    fn colisao_com_opcao_do_comando_nao_tem_forma() {
        let s = spec(json!({"type":"object","properties":{"json":{"type":"string"}}}));
        assert!(sem_forma(&s).unwrap().contains("--json"));
        let s = spec(json!({"type":"string"}));
        assert!(sem_forma(&s).is_some());
    }
}

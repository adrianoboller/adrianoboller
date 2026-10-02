//! Explorador de testes: `test_list` devolve a arvore crate/modulo/teste (Rust pelo
//! `cargo test -- --list`, Python pelo `pytest --collect-only -q`) e `test_run` roda UM no
//! dela. A CLI `phxclaw testes listar|rodar` chama as mesmas funcoes.
//!
//! Nada aqui monta sandbox proprio: o Rust usa as `SandboxExtras` e o leitor de
//! `--message-format=json` do `rust_project`, e o Python delega o `rodar` inteiro ao
//! `python_project` (`action=test`, `target=<no>`), que ja sabe venv, plugin de relatorio
//! e nodeid. O que e deste arquivo e so a arvore e o no.

use crate::python::{PythonProjectTool, aspas};
use crate::sistema::{
    RustProjectTool, arg_opt, arg_str, caminho_do_projeto, diagnosticos_do_cargo,
};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_sandbox::{WorkdirCommand, run_in_workdir_com};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Os motores que existem no hospedeiro; sem nenhum, as ferramentas nem se registram.
pub struct ExploradorDeTestes {
    pub rust: Option<Arc<RustProjectTool>>,
    pub python: Option<Arc<PythonProjectTool>>,
}

impl ExploradorDeTestes {
    pub fn detectar(bwrap: PathBuf) -> Option<Self> {
        let rust = RustProjectTool::detectar(bwrap.clone()).map(Arc::new);
        let python = PythonProjectTool::detectar(bwrap).ok().map(Arc::new);
        (rust.is_some() || python.is_some()).then_some(Self { rust, python })
    }

    /// Com os motores ja montados (a montagem do agente, para nao sondar duas vezes).
    pub fn com(rust: Option<Arc<RustProjectTool>>, python: Option<Arc<PythonProjectTool>>) -> Self {
        Self { rust, python }
    }

    fn linguagem(
        &self,
        ctx: &ToolContext,
        rel: &str,
        pedida: Option<&str>,
    ) -> Result<&'static str, ToolError> {
        let dir = if rel.is_empty() {
            ctx.workdir.clone()
        } else {
            ctx.workdir.join(rel)
        };
        let l = match pedida {
            Some(l) => l.to_string(),
            None if dir.join("Cargo.toml").is_file() => "rust".into(),
            None if dir.join("pyproject.toml").is_file()
                || dir.join("pytest.ini").is_file()
                || dir.join("tests").is_dir() =>
            {
                "python".into()
            }
            None => {
                return Err(ToolError::InvalidArguments(format!(
                    "nem Cargo.toml nem projeto Python em {}",
                    if rel.is_empty() { "." } else { rel }
                )));
            }
        };
        match l.as_str() {
            "rust" if self.rust.is_some() => Ok("rust"),
            "python" if self.python.is_some() => Ok("python"),
            "rust" | "python" => Err(ToolError::Failed(format!(
                "sem toolchain de {l} neste hospedeiro"
            ))),
            outra => Err(ToolError::InvalidArguments(format!(
                "language desconhecida: {outra} (rust, python)"
            ))),
        }
    }

    /// A arvore de testes do projeto em `rel` (relativo a pasta da tarefa).
    pub async fn listar(
        &self,
        ctx: &ToolContext,
        path: &str,
        language: Option<&str>,
    ) -> Result<Value, ToolError> {
        let rel = caminho_do_projeto(&ctx.workdir, path)?;
        let projeto = if rel.is_empty() {
            ".".to_string()
        } else {
            rel.clone()
        };
        let guest = if rel.is_empty() {
            "/work".to_string()
        } else {
            format!("/work/{rel}")
        };
        match self.linguagem(ctx, &rel, language)? {
            "rust" => {
                let rust = self.rust.as_ref().expect("conferido");
                // 1) compila os binarios de teste sem rodar: o JSON diz qual executavel e
                // de qual crate -- o `Running` do cargo vai para o stderr, sem estrutura.
                let r = rodar_rust(
                    rust,
                    ctx,
                    format!(
                        "cd '{guest}' && exec cargo test --offline --no-run --message-format=json"
                    ),
                )
                .await?;
                let (diags, sucesso, _) = diagnosticos_do_cargo(&r.stdout);
                if r.exit_code != Some(0) || sucesso == Some(false) {
                    let erros: Vec<&_> = diags.iter().filter(|d| d.nivel == "error").collect();
                    return Err(ToolError::Failed(format!(
                        "os testes nao compilam ({} erro(s)): {}",
                        erros.len(),
                        erros
                            .iter()
                            .take(3)
                            .map(|d| format!("{}:{}: {}", d.arquivo, d.linha, d.mensagem))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )));
                }
                let binarios = binarios_de_teste(&r.stdout);
                if binarios.is_empty() {
                    return Ok(
                        json!({"linguagem": "rust", "projeto": projeto, "total": 0, "crates": []}),
                    );
                }
                // 2) cada binario lista os proprios testes; a marca separa um do outro.
                let mut script = format!("cd '{guest}'");
                for (crate_, exe) in &binarios {
                    script.push_str(&format!(
                        " && echo {} && {} --list --format terse",
                        aspas(&format!("@@crate {crate_}")),
                        aspas(exe)
                    ));
                }
                let r = rodar_rust(rust, ctx, script).await?;
                let arvore = arvore_rust(&r.stdout);
                let total: usize = arvore
                    .iter()
                    .flat_map(|c| c["modulos"].as_array().into_iter().flatten())
                    .map(|m| m["testes"].as_array().map(Vec::len).unwrap_or(0))
                    .sum();
                Ok(
                    json!({"linguagem": "rust", "projeto": projeto, "total": total, "crates": arvore}),
                )
            }
            _ => {
                let py = self.python.as_ref().expect("conferido");
                let i = &py.interpretador;
                // O venv do `python_project`, se ja existe; senao o interpretador do
                // hospedeiro, que ja traz o pytest no `site` dele.
                let script = format!(
                    "cd {g} || exit 90\nif .venv/bin/python -c '' 2>/dev/null; then PY=.venv/bin/python; else PY={py}; fi\n\
exec \"$PY\" -m pytest --collect-only -q -p no:cacheprovider",
                    g = aspas(&guest),
                    py = aspas(&i.executavel.display().to_string()),
                );
                let cmd = WorkdirCommand {
                    workdir: ctx.workdir.clone(),
                    script,
                    timeout: py.timeout.min(ctx.timeout),
                    network: false,
                    max_output_bytes: 4 * 1024 * 1024,
                };
                let (bwrap, extras) = (py.bwrap.clone(), py.extras(None));
                let r =
                    tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
                        .await
                        .map_err(|e| ToolError::Failed(e.to_string()))?
                        .map_err(|e| ToolError::Failed(e.to_string()))?;
                let (arvore, total) = arvore_python(&r.stdout);
                if total == 0 && r.exit_code != Some(0) && r.exit_code != Some(5) {
                    let cauda: String = r
                        .stderr
                        .chars()
                        .rev()
                        .take(1500)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    return Err(ToolError::Failed(format!(
                        "pytest --collect-only falhou (codigo {:?}): {}{cauda}",
                        r.exit_code,
                        r.stdout
                            .lines()
                            .rev()
                            .take(5)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )));
                }
                Ok(
                    json!({"linguagem": "python", "projeto": projeto, "total": total, "arquivos": arvore}),
                )
            }
        }
    }

    /// Roda um no: Rust `CRATE`, `CRATE/modulo` ou `CRATE/modulo::teste`; Python o nodeid
    /// do pytest (`arquivo.py`, `arquivo.py::teste`).
    pub async fn rodar(
        &self,
        ctx: &ToolContext,
        path: &str,
        no: &str,
        language: Option<&str>,
    ) -> Result<Value, ToolError> {
        let rel = caminho_do_projeto(&ctx.workdir, path)?;
        let projeto = if rel.is_empty() {
            ".".to_string()
        } else {
            rel.clone()
        };
        match self.linguagem(ctx, &rel, language)? {
            "rust" => {
                let rust = self.rust.as_ref().expect("conferido");
                let (crate_, filtro) = no_rust_valido(no)?;
                let guest = if rel.is_empty() {
                    "/work".to_string()
                } else {
                    format!("/work/{rel}")
                };
                let mut cargo = format!("cargo test --offline --message-format=json -p '{crate_}'");
                if let Some(f) = &filtro {
                    cargo.push_str(&format!(" -- '{f}'"));
                    // `modulo::teste` e o teste exato; `modulo` e o prefixo.
                    if f.rsplit("::")
                        .next()
                        .is_some_and(|ultimo| f.contains("::") && !ultimo.is_empty())
                    {
                        cargo.push_str(" --exact");
                    }
                }
                let r = rodar_rust(rust, ctx, format!("cd '{guest}' && exec {cargo}")).await?;
                let (diags, sucesso, texto) = diagnosticos_do_cargo(&r.stdout);
                let resultados: Vec<&String> = texto
                    .iter()
                    .filter(|l| {
                        l.starts_with("test result:")
                            || l.starts_with("test ")
                            || l.contains("panicked")
                    })
                    .collect();
                let passou = r.exit_code == Some(0) && sucesso != Some(false);
                let (ok, falhou) = contar_rust(&texto);
                Ok(json!({
                    "linguagem": "rust", "projeto": projeto, "no": no,
                    "passou": passou, "ok": ok, "falhou": falhou,
                    "erros_de_compilacao": diags.iter().filter(|d| d.nivel == "error").count(),
                    "resultados": resultados,
                    "stderr_cauda": if passou { String::new() } else { cauda(&r.stderr) },
                }))
            }
            _ => {
                let py = self.python.as_ref().expect("conferido");
                let saida = py
                    .run(
                        json!({"action": "test", "path": projeto, "target": no}),
                        ctx,
                    )
                    .await?;
                let mut v: Value = serde_json::from_str(&saida.content)
                    .map_err(|e| ToolError::Failed(format!("python_project: {e}")))?;
                v["linguagem"] = json!("python");
                v["no"] = json!(no);
                v["passou"] = v["sucesso"].clone();
                Ok(v)
            }
        }
    }
}

async fn rodar_rust(
    rust: &RustProjectTool,
    ctx: &ToolContext,
    script: String,
) -> Result<phxclaw_sandbox::WorkdirOutput, ToolError> {
    let cmd = WorkdirCommand {
        workdir: ctx.workdir.clone(),
        script,
        timeout: rust.timeout.min(ctx.timeout),
        network: false,
        max_output_bytes: 4 * 1024 * 1024,
    };
    let (bwrap, extras) = (rust.bwrap.clone(), rust.extras());
    tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?
        .map_err(|e| ToolError::Failed(e.to_string()))
}

fn cauda(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(3000)..].iter().collect()
}

/// (crate, executavel) de cada artefato de teste do `--no-run --message-format=json`.
/// O executavel so vale dentro de `/work`: e o que o cargo gravou, nao o modelo.
pub fn binarios_de_teste(stdout: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for l in stdout.lines() {
        let Ok(m) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if m["reason"] != "compiler-artifact" || m["profile"]["test"] != true {
            continue;
        }
        let (Some(nome), Some(exe)) = (m["target"]["name"].as_str(), m["executable"].as_str())
        else {
            continue;
        };
        if !exe.starts_with("/work/") || exe.contains('\'') {
            continue;
        }
        let pacote = m["package_id"]
            .as_str()
            .and_then(nome_do_pacote)
            .unwrap_or_else(|| nome.to_string());
        v.push((pacote, exe.to_string()));
    }
    v
}

/// `path+file:///x/y/nome#0.1.0` ou `nome 0.1.0 (path+file://...)` → `nome`.
fn nome_do_pacote(id: &str) -> Option<String> {
    if let Some((_, resto)) = id.split_once('#') {
        let nome = match resto.split_once('@') {
            Some((n, _)) => n.to_string(),
            None => id
                .split_once('#')?
                .0
                .trim_end_matches('/')
                .rsplit('/')
                .next()?
                .to_string(),
        };
        return Some(nome);
    }
    id.split_whitespace().next().map(str::to_string)
}

/// As linhas `@@crate X` e `modulo::teste: test` em crate → modulo → testes.
pub fn arvore_rust(stdout: &str) -> Vec<Value> {
    let mut crates: Vec<(String, BTreeMap<String, Vec<String>>)> = Vec::new();
    for l in stdout.lines() {
        if let Some(c) = l.strip_prefix("@@crate ") {
            crates.push((c.trim().to_string(), BTreeMap::new()));
            continue;
        }
        let Some(nome) = l
            .strip_suffix(": test")
            .or_else(|| l.strip_suffix(": benchmark"))
        else {
            continue;
        };
        let Some((_, modulos)) = crates.last_mut() else {
            continue;
        };
        let (modulo, teste) = match nome.rsplit_once("::") {
            Some((m, t)) => (m.to_string(), t.to_string()),
            None => (String::new(), nome.to_string()),
        };
        modulos.entry(modulo).or_default().push(teste);
    }
    crates
        .into_iter()
        .map(|(c, modulos)| {
            let ms: Vec<Value> = modulos
                .into_iter()
                .map(|(m, ts)| json!({"modulo": m, "testes": ts}))
                .collect();
            json!({"crate": c, "modulos": ms})
        })
        .collect()
}

/// `(passaram, falharam)` somados das linhas `test result:` (um binario por linha).
fn contar_rust(texto: &[String]) -> (u64, u64) {
    let mut ok = 0;
    let mut falhou = 0;
    for l in texto.iter().filter(|l| l.starts_with("test result:")) {
        for parte in l.split([';', '.']) {
            let mut it = parte.split_whitespace();
            if let (Some(n), Some(k)) = (it.next(), it.next())
                && let Ok(n) = n.parse::<u64>()
            {
                match k {
                    "passed" => ok += n,
                    "failed" => falhou += n,
                    _ => {}
                }
            }
        }
    }
    (ok, falhou)
}

/// Nodeids do `pytest --collect-only -q` (`arquivo.py::Classe::teste`) em arquivo → testes.
pub fn arvore_python(stdout: &str) -> (Vec<Value>, usize) {
    let mut arquivos: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total = 0;
    for l in stdout.lines() {
        let l = l.trim();
        let Some((arq, resto)) = l.split_once("::") else {
            continue;
        };
        if !arq.ends_with(".py") || l.contains(' ') {
            continue;
        }
        arquivos
            .entry(arq.to_string())
            .or_default()
            .push(resto.to_string());
        total += 1;
    }
    (
        arquivos
            .into_iter()
            .map(|(a, ts)| json!({"arquivo": a, "testes": ts}))
            .collect(),
        total,
    )
}

/// `CRATE`, `CRATE/modulo` ou `CRATE/modulo::teste`: so caracteres de identificador --
/// os dois pedacos entram entre aspas simples num `sh -c`.
fn no_rust_valido(no: &str) -> Result<(String, Option<String>), ToolError> {
    let (crate_, filtro) = match no.split_once('/') {
        Some((c, f)) if !f.is_empty() => (c, Some(f)),
        Some((c, _)) => (c, None),
        None => (no, None),
    };
    let ident = |s: &str| {
        !s.is_empty()
            && s.len() <= 256
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ':')
            && !s.contains(":::")
    };
    if !ident(crate_) || filtro.is_some_and(|f| !ident(f)) {
        return Err(ToolError::InvalidArguments(format!(
            "no invalido: {no:?} (CRATE, CRATE/modulo ou CRATE/modulo::teste)"
        )));
    }
    Ok((crate_.to_string(), filtro.map(str::to_string)))
}

pub struct TestListTool(pub Arc<ExploradorDeTestes>);
pub struct TestRunTool(pub Arc<ExploradorDeTestes>);

impl Tool for TestListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "test_list".into(),
            description: "List the tests of a project in the task directory as a tree \
(Rust: crate/module/test from `cargo test -- --list`; Python: file/test from `pytest \
--collect-only`). path is the project directory (default '.'); language is guessed from \
Cargo.toml / pyproject.toml. Nodes are what test_run accepts."
                .into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "language":{"type":"string","enum":["rust","python"]}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    fn comando_de_shell(&self, _args: &Value) -> Option<String> {
        Some("cargo test".into())
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let v = self
                .0
                .listar(
                    ctx,
                    arg_opt(&args, "path").unwrap_or("."),
                    arg_opt(&args, "language"),
                )
                .await?;
            Ok(ToolOutput::text(
                serde_json::to_string_pretty(&v).unwrap_or_default(),
            ))
        })
    }
}

impl Tool for TestRunTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "test_run".into(),
            description: "Run ONE node of the test tree (see test_list): Rust `CRATE`, \
`CRATE/module` or `CRATE/module::test`; Python a pytest node id (`tests/test_x.py` or \
`tests/test_x.py::test_name`). Same sandbox and same cargo/pytest as rust_project and \
python_project. Returns passed/failed counts and the result lines."
                .into(),
            parameters: json!({"type":"object","properties":{
                "node":{"type":"string"},
                "path":{"type":"string"},
                "language":{"type":"string","enum":["rust","python"]}
            },"required":["node"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        let no = args.get("node").and_then(Value::as_str).unwrap_or("");
        Some(if no.contains(".py") {
            format!("python -m pytest {}", aspas(no))
        } else {
            format!("cargo test {}", aspas(no))
        })
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let no = arg_str(&args, "node")?.to_string();
            let v = self
                .0
                .rodar(
                    ctx,
                    arg_opt(&args, "path").unwrap_or("."),
                    &no,
                    arg_opt(&args, "language"),
                )
                .await?;
            Ok(ToolOutput::text(
                serde_json::to_string_pretty(&v).unwrap_or_default(),
            ))
        })
    }
}

/// O contexto da CLI (`phxclaw testes`): a pasta do projeto como pasta da tarefa.
pub fn contexto_da_cli(projeto: PathBuf, prazo: Duration) -> ToolContext {
    ToolContext {
        task_id: "cli".into(),
        workdir: projeto,
        timeout: prazo,
    }
}

/// As duas ferramentas sobre o mesmo explorador.
pub fn ferramentas(e: Arc<ExploradorDeTestes>) -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(TestListTool(e.clone())), Arc::new(TestRunTool(e))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arvore_rust_agrupa_por_crate_e_modulo() {
        let s = "@@crate calc\nsoma::dois_mais_dois: test\nsoma::zero: test\nraiz: test\n@@crate outra\nx::y: test\n";
        let a = arvore_rust(s);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0]["crate"], "calc");
        let ms = a[0]["modulos"].as_array().unwrap();
        assert_eq!(ms[0]["modulo"], "");
        assert_eq!(ms[0]["testes"], json!(["raiz"]));
        assert_eq!(ms[1]["modulo"], "soma");
        assert_eq!(ms[1]["testes"], json!(["dois_mais_dois", "zero"]));
    }

    #[test]
    fn binarios_so_de_teste_e_so_em_work() {
        let s = concat!(
            r#"{"reason":"compiler-artifact","package_id":"path+file:///work/calc#0.1.0","target":{"name":"calc"},"profile":{"test":true},"executable":"/work/target/debug/deps/calc-abc"}"#,
            "\n",
            r#"{"reason":"compiler-artifact","package_id":"path+file:///work/calc#0.1.0","target":{"name":"calc"},"profile":{"test":false},"executable":"/work/target/debug/calc"}"#,
            "\n",
            r#"{"reason":"compiler-artifact","package_id":"path+file:///work/calc#0.1.0","target":{"name":"calc"},"profile":{"test":true},"executable":"/etc/x"}"#,
            "\n",
        );
        assert_eq!(
            binarios_de_teste(s),
            vec![(
                "calc".to_string(),
                "/work/target/debug/deps/calc-abc".to_string()
            )]
        );
        assert_eq!(
            nome_do_pacote("path+file:///w/calc#meu-nome@0.1.0"),
            Some("meu-nome".into())
        );
        assert_eq!(
            nome_do_pacote("calc 0.1.0 (path+file:///w/calc)"),
            Some("calc".into())
        );
    }

    #[test]
    fn no_rust_recusa_shell_e_aceita_os_tres_niveis() {
        assert_eq!(no_rust_valido("calc").unwrap(), ("calc".into(), None));
        assert_eq!(
            no_rust_valido("calc/soma").unwrap(),
            ("calc".into(), Some("soma".into()))
        );
        assert_eq!(
            no_rust_valido("calc/soma::zero").unwrap(),
            ("calc".into(), Some("soma::zero".into()))
        );
        for ruim in ["", "a b", "calc/x'y", "$(id)", "a/b/c"] {
            assert!(no_rust_valido(ruim).is_err(), "{ruim:?}");
        }
    }

    #[test]
    fn arvore_python_e_contagem() {
        let (a, n) = arvore_python(
            "tests/test_a.py::test_x\ntests/test_a.py::T::test_y\n\n2 tests collected in 0.01s\n",
        );
        assert_eq!(n, 2);
        assert_eq!(a[0]["arquivo"], "tests/test_a.py");
        assert_eq!(a[0]["testes"], json!(["test_x", "T::test_y"]));
        assert_eq!(
            contar_rust(&["test result: ok. 3 passed; 1 failed; 0 ignored".to_string()]),
            (3, 1)
        );
    }
}

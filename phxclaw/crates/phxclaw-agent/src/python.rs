//! `python_project`: venv, dependencias, pytest, ruff, mypy e script num projeto Python da
//! pasta da tarefa, no MESMO bwrap do `shell` e do `rust_project`.
//!
//! Tres decisoes que valem saber antes de mexer:
//!
//! - **Uma montagem de sandbox so.** O interpretador, o `uv` e o cache entram pelo
//!   `SandboxExtras` do `phxclaw-sandbox`, como o toolchain do `rust_project`; a lista de
//!   binds do sistema continua sendo a do shell. Uma segunda lista divergiria da primeira
//!   no dia em que alguem endurecesse so uma delas.
//! - **Sem rede, e o cache do hospedeiro nunca gravavel.** O `uv` 0.8 abre arquivos do
//!   cache para escrita ate numa instalacao que nao baixa nada (marcas em cada balde, a
//!   ficha do interpretador), e o bwrap 0.9 desta maquina nao tem `--overlay`. Montar o
//!   cache gravavel deixaria o codigo do projeto envenenar o cache que o hospedeiro usa
//!   depois. Entao o cache entra so leitura, os `archive-*` (o conteudo das rodas) entram
//!   montados direto, e o resto vira, dentro do `/tmp` do sandbox, uma pasta de atalhos
//!   para o cache: o `uv` grava as marcas dele no tmpfs e le o conteudo pelo atalho.
//!   Medido em 01/10/2026: com o cache so leitura direto, `uv pip install` falha com
//!   `Read-only file system` em `sdists-v9/.git`; com os atalhos, instala `requests` e as
//!   quatro dependencias em 8 ms.
//! - **As ferramentas vem do interpretador, as dependencias do projeto.** O venv do
//!   projeto ganha um `.pth` apontando para o `site-packages` do `PHXCLAW_PYTHON`: pytest,
//!   mypy e ruff vem de la sem instalar nada, e o que o projeto declara entra no venv dele,
//!   que fica ANTES no `sys.path` -- o pytest que o projeto fixa ganha do nosso.

use crate::sistema::{Diagnostico, arg_opt, arg_str, binario, caminho_do_projeto};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, WorkdirOutput, run_in_workdir_com};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Interpretador padrao quando `PHXCLAW_PYTHON` nao foi dado: o ambiente que a instalacao
/// do PhxClaw prepara, com pytest, ruff e mypy.
pub const PYTHON_PADRAO: &str = "/opt/phxclaw-python/bin/python";

/// Onde o `uv` e o cache aparecem dentro do sandbox. No `/tmp` porque ele e tmpfs: o bwrap
/// cria os pais sem tocar o sistema so leitura.
const UV_GUEST: &str = "/tmp/phxclaw-py/uv";
const CACHE_RO_GUEST: &str = "/tmp/phxclaw-py/cache-ro";
const CACHE_GUEST: &str = "/tmp/phxclaw-py/cache";
const PYTEST_PLUGIN_DIR: &str = "/tmp/phxclaw-pytest";

/// Teto de argumentos do script: o modelo nao precisa de mais, e cada um vira texto num
/// `sh -c`.
const ARGS_MAX: usize = 32;

/// O interpretador sondado no hospedeiro: o que vai montado no sandbox e o que vai no
/// `.pth` do venv do projeto.
#[derive(Debug, Clone, PartialEq)]
pub struct Interpretador {
    /// `sys.executable`: o caminho que se chama la dentro (montado no mesmo lugar).
    pub executavel: PathBuf,
    pub prefixo: PathBuf,
    pub prefixo_base: PathBuf,
    /// `purelib` do interpretador: onde moram pytest, mypy e ruff.
    pub site: PathBuf,
    pub versao: String,
}

/// Raizes que o sandbox do shell ja monta; o que esta debaixo delas nao precisa de bind.
const RAIZES_DO_SANDBOX: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/sbin"];

fn ja_montado(p: &Path) -> bool {
    RAIZES_DO_SANDBOX.iter().any(|r| p.starts_with(r))
}

impl Interpretador {
    /// Pergunta ao proprio interpretador onde ele mora. `-I` (isolado) e ambiente limpo:
    /// um `PYTHONPATH` do processo do agente mudaria a resposta.
    pub fn sondar(caminho: &Path) -> Result<Self, String> {
        let codigo = "import json,sys,sysconfig;print(json.dumps([sys.executable,sys.prefix,\
sys.base_prefix,sysconfig.get_paths()['purelib'],'%d.%d.%d'%sys.version_info[:3]]))";
        let s = Command::new(caminho)
            .args(["-I", "-c", codigo])
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .output()
            .map_err(|e| format!("{}: {e}", caminho.display()))?;
        if !s.status.success() {
            return Err(format!(
                "{} falhou: {}",
                caminho.display(),
                String::from_utf8_lossy(&s.stderr).trim()
            ));
        }
        let v: Vec<String> = serde_json::from_slice(&s.stdout)
            .map_err(|e| format!("{}: resposta ilegivel: {e}", caminho.display()))?;
        let [exe, prefixo, base, site, versao] = <[String; 5]>::try_from(v)
            .map_err(|_| format!("{}: resposta incompleta", caminho.display()))?;
        let i = Self {
            executavel: PathBuf::from(exe),
            prefixo: PathBuf::from(prefixo),
            prefixo_base: PathBuf::from(base),
            site: PathBuf::from(site),
            versao,
        };
        // O executavel tem de existir la dentro pelo mesmo caminho: so os prefixos e as
        // raizes do sistema entram montados.
        let alcancado = [&i.prefixo, &i.prefixo_base]
            .iter()
            .any(|p| i.executavel.starts_with(p))
            || ja_montado(&i.executavel);
        if !alcancado {
            return Err(format!(
                "{} fora de {} e de {}: nao entraria no sandbox",
                i.executavel.display(),
                i.prefixo.display(),
                i.prefixo_base.display()
            ));
        }
        Ok(i)
    }
}

/// `uv` em lugares fixos, como os outros binarios do agente: o `PATH` herdado e
/// configuracao de quem lancou o processo.
fn achar_uv() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PHXCLAW_UV").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    home.iter()
        .flat_map(|h| [h.join(".local/bin/uv"), h.join(".cargo/bin/uv")])
        .find(|p| p.is_file())
        .or_else(|| binario("uv"))
}

fn achar_cache_uv() -> Option<PathBuf> {
    std::env::var_os("UV_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CACHE_HOME")
                .map(|x| PathBuf::from(x).join("uv"))
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/uv")))
        })
        .filter(|p| p.is_dir())
}

pub struct PythonProjectTool {
    pub bwrap: PathBuf,
    pub interpretador: Interpretador,
    /// Sem `uv`, o venv sai do `-m venv` e dependencia declarada vira recusa explicita.
    pub uv: Option<PathBuf>,
    /// Cache do `uv` do hospedeiro, montado so leitura: e de la que sai a dependencia, ja
    /// que o sandbox nao tem rede.
    pub cache_uv: Option<PathBuf>,
    pub timeout: Duration,
}

impl PythonProjectTool {
    /// Interpretador de `PHXCLAW_PYTHON`, ou o padrao, ou o `python3` do sistema. O erro
    /// diz por que a ferramenta nao se registrou, em vez de ela sumir calada.
    pub fn detectar(bwrap: PathBuf) -> Result<Self, String> {
        let caminho = match std::env::var_os("PHXCLAW_PYTHON") {
            Some(p) => PathBuf::from(p),
            None => Some(PathBuf::from(PYTHON_PADRAO))
                .filter(|p| p.is_file())
                .or_else(|| binario("python3"))
                .ok_or("nenhum interpretador: nem PHXCLAW_PYTHON, nem o padrao, nem python3")?,
        };
        Ok(Self::com(bwrap, Interpretador::sondar(&caminho)?))
    }

    /// Com um interpretador ja sondado; `uv` e cache achados no ambiente.
    pub fn com(bwrap: PathBuf, interpretador: Interpretador) -> Self {
        Self {
            bwrap,
            interpretador,
            uv: achar_uv(),
            cache_uv: achar_cache_uv(),
            timeout: Duration::from_secs(600),
        }
    }

    pub(crate) fn extras(&self, pythonpath: Option<String>) -> SandboxExtras {
        let i = &self.interpretador;
        let mut binds: Vec<(PathBuf, String)> = Vec::new();
        for p in [&i.prefixo, &i.prefixo_base] {
            if !ja_montado(p) && !binds.iter().any(|(h, _)| p.starts_with(h)) {
                binds.push((p.clone(), p.display().to_string()));
            }
        }
        if let Some(uv) = &self.uv {
            binds.push((uv.clone(), UV_GUEST.into()));
        }
        if let Some(c) = &self.cache_uv {
            binds.push((c.clone(), CACHE_RO_GUEST.into()));
            for a in std::fs::read_dir(c).into_iter().flatten().flatten() {
                let nome = a.file_name().to_string_lossy().into_owned();
                if nome.starts_with("archive-") && a.path().is_dir() {
                    binds.push((a.path(), format!("{CACHE_GUEST}/{nome}")));
                }
            }
        }
        let bin = i
            .executavel
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let mut env = vec![
            ("PATH".into(), format!("{bin}:/usr/local/bin:/usr/bin:/bin")),
            ("UV_CACHE_DIR".into(), CACHE_GUEST.into()),
            // A rede ja nao existe; offline faz o uv dizer «nao esta no cache» em vez de
            // esperar o prazo de uma conexao que nunca abre.
            ("UV_OFFLINE".into(), "1".into()),
            ("UV_NO_MANAGED_PYTHON".into(), "1".into()),
            ("UV_PYTHON_DOWNLOADS".into(), "never".into()),
            ("UV_LINK_MODE".into(), "copy".into()),
            ("UV_NO_PROGRESS".into(), "1".into()),
            ("PIP_NO_INDEX".into(), "1".into()),
            ("NO_COLOR".into(), "1".into()),
            ("PYTHONDONTWRITEBYTECODE".into(), "1".into()),
            ("PYTHONUNBUFFERED".into(), "1".into()),
        ];
        if let Some(pp) = pythonpath {
            env.push(("PYTHONPATH".into(), pp));
        }
        SandboxExtras {
            ro_binds: binds,
            env,
        }
    }

    async fn rodar(
        &self,
        ctx: &ToolContext,
        script: String,
        pythonpath: Option<String>,
    ) -> Result<WorkdirOutput, ToolError> {
        let cmd = WorkdirCommand {
            workdir: ctx.workdir.clone(),
            script,
            timeout: self.timeout.min(ctx.timeout),
            network: false,
            max_output_bytes: 4 * 1024 * 1024,
        };
        let (bwrap, extras) = (self.bwrap.clone(), self.extras(pythonpath));
        tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?
            .map_err(|e| ToolError::Failed(e.to_string()))
    }
}

/// Aspas simples de shell: o unico lugar onde texto do modelo entra num `sh -c` desta
/// ferramenta. Dentro de aspas simples nada se expande; a propria aspa sai e volta.
pub fn aspas(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// As fontes de dependencia do projeto, na ordem em que o `uv` as recebe, e o hash que
/// diz se o venv ja as tem. O hash inclui a versao do interpretador: trocar o Python
/// invalida o que foi instalado para o outro.
pub fn fontes_de_dependencia(dir: &Path, versao: &str) -> (Vec<String>, String) {
    let mut args = Vec::new();
    let mut h = Sha256::new();
    h.update(versao.as_bytes());
    if let Ok(t) = std::fs::read(dir.join("requirements.txt")) {
        h.update(b"requirements.txt\0");
        h.update(&t);
        args.push("-r requirements.txt".to_string());
    }
    if let Ok(t) = std::fs::read_to_string(dir.join("pyproject.toml")) {
        // Sem tabela [project] o uv recusa o arquivo como fonte; pyproject so de
        // configuracao (ruff, mypy) nao declara dependencia nenhuma.
        if t.lines().any(|l| l.trim() == "[project]") {
            h.update(b"pyproject.toml\0");
            h.update(t.as_bytes());
            args.push("-r pyproject.toml --all-extras".to_string());
        }
    }
    let hash = h
        .finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect();
    (args, hash)
}

/// O relatorio do pytest, por plugin nosso: `reprcrash` e o traceback vem do proprio
/// pytest, sem reler texto de terminal. Escreve num arquivo do tmpfs; quem o le e o
/// script, depois de uma marca que o teste nao conhece.
const PLUGIN_PYTEST: &str = r#"import json, os, re

_raiz = "."
_r = {"aprovados": 0, "falhas": 0, "erros": 0, "ignorados": 0, "itens": []}


def _rel(p):
    try:
        return os.path.relpath(os.path.join(_raiz, str(p)), _raiz)
    except ValueError:
        return str(p)


def _item(rep, resultado):
    loc = getattr(rep, "location", None)
    arq = _rel(loc[0]) if loc else _rel(rep.nodeid.split("::")[0])
    linha = (loc[1] + 1) if loc and loc[1] is not None else 0
    lr = rep.longrepr
    crash = getattr(lr, "reprcrash", None)
    tb = getattr(lr, "reprtraceback", None)
    for e in getattr(tb, "reprentries", None) or []:
        fl = getattr(e, "reprfileloc", None)
        if fl is not None and _rel(fl.path) == arq:
            linha = fl.lineno
    texto = str(lr) if lr is not None else ""
    if crash is not None:
        msg = crash.message
    else:
        msg = next((l.strip() for l in reversed(texto.splitlines()) if l.strip()), "")
        m = None
        for m in re.finditer(re.escape(arq) + r":(\d+)", texto):
            pass
        if m is not None:
            linha = int(m.group(1))
    item = {"nodeid": rep.nodeid, "quando": rep.when, "resultado": resultado,
            "arquivo": arq, "linha": linha, "mensagem": msg}
    if crash is not None:
        item["quebra_arquivo"] = _rel(crash.path)
        item["quebra_linha"] = crash.lineno
    _r["itens"].append(item)


def pytest_configure(config):
    global _raiz
    _raiz = str(config.rootpath)


def pytest_runtest_logreport(report):
    if report.when == "call":
        if hasattr(report, "wasxfail") or report.skipped:
            _r["ignorados"] += 1
        elif report.passed:
            _r["aprovados"] += 1
        elif report.failed:
            _r["falhas"] += 1
            _item(report, "failure")
    elif report.failed:
        _r["erros"] += 1
        _item(report, "error")
    elif report.skipped and report.when == "setup":
        _r["ignorados"] += 1


def pytest_collectreport(report):
    if report.failed:
        _r["erros"] += 1
        _item(report, "error")


def pytest_sessionfinish(session, exitstatus):
    _r["exit"] = int(exitstatus)
    with open(os.environ["PHXCLAW_PYTEST_JSON"], "w") as f:
        json.dump(_r, f)
"#;

/// `ruff check --output-format=json`: um vetor; o caminho vem absoluto e sai relativo ao
/// projeto.
pub fn diagnosticos_do_ruff(stdout: &str, raiz: &str) -> Vec<Diagnostico> {
    let Some(ini) = stdout.find('[') else {
        return vec![];
    };
    let Ok(Value::Array(v)) = serde_json::from_str::<Value>(&stdout[ini..]) else {
        return vec![];
    };
    v.iter()
        .map(|d| Diagnostico {
            arquivo: relativo(d["filename"].as_str().unwrap_or(""), raiz),
            linha: d["location"]["row"].as_u64().unwrap_or(0),
            coluna: d["location"]["column"].as_u64().unwrap_or(0),
            nivel: d["severity"].as_str().unwrap_or("error").to_string(),
            mensagem: d["message"].as_str().unwrap_or("").to_string(),
            codigo: d["code"].as_str().map(str::to_string),
        })
        .collect()
}

/// `mypy -O json`: um objeto por linha. A dica (`hint`) vai junto da mensagem porque e
/// nela que o mypy diz o que fazer. Linha que nao e JSON volta a parte (erro de uso do
/// mypy, arquivo ilegivel).
pub fn diagnosticos_do_mypy(stdout: &str, raiz: &str) -> (Vec<Diagnostico>, Vec<String>) {
    let mut diags = Vec::new();
    let mut texto = Vec::new();
    for l in stdout.lines() {
        match serde_json::from_str::<Value>(l) {
            Ok(d) if d.is_object() => {
                let mut msg = d["message"].as_str().unwrap_or("").to_string();
                if let Some(h) = d["hint"].as_str().filter(|h| !h.is_empty()) {
                    msg.push_str(" -- ");
                    msg.push_str(h);
                }
                diags.push(Diagnostico {
                    arquivo: relativo(d["file"].as_str().unwrap_or(""), raiz),
                    linha: d["line"].as_u64().unwrap_or(0),
                    coluna: d["column"].as_u64().map(|c| c + 1).unwrap_or(0),
                    nivel: d["severity"].as_str().unwrap_or("error").to_string(),
                    mensagem: msg,
                    codigo: d["code"].as_str().map(str::to_string),
                });
            }
            _ if !l.trim().is_empty() => texto.push(l.to_string()),
            _ => {}
        }
    }
    (diags, texto)
}

/// O relatorio do plugin do pytest: contagem e um diagnostico por teste que falhou. O
/// `codigo` e o nodeid -- e ele que diz QUAL teste, e e ele que se passa de volta como
/// `target` para rodar so aquele.
pub fn diagnosticos_do_pytest(relatorio: &str) -> (Vec<Diagnostico>, Value) {
    let Ok(r) = serde_json::from_str::<Value>(relatorio.trim()) else {
        return (vec![], Value::Null);
    };
    let diags = r["itens"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| {
            let arq = i["arquivo"].as_str().unwrap_or("").to_string();
            let mut msg = i["mensagem"].as_str().unwrap_or("").to_string();
            if let (Some(qa), Some(ql)) = (i["quebra_arquivo"].as_str(), i["quebra_linha"].as_u64())
                && (qa != arq || Some(ql) != i["linha"].as_u64())
            {
                msg.push_str(&format!(" (quebrou em {qa}:{ql})"));
            }
            Diagnostico {
                arquivo: arq,
                linha: i["linha"].as_u64().unwrap_or(0),
                coluna: 0,
                nivel: i["resultado"].as_str().unwrap_or("error").to_string(),
                mensagem: msg,
                codigo: i["nodeid"].as_str().map(str::to_string),
            }
        })
        .collect();
    let contagem = json!({
        "aprovados": r["aprovados"], "falhas": r["falhas"],
        "erros": r["erros"], "ignorados": r["ignorados"],
    });
    (diags, contagem)
}

/// Traceback do Python no stderr de um script: o ultimo `File "...", line N` e a linha da
/// excecao viram um diagnostico.
pub fn diagnostico_do_traceback(stderr: &str, raiz: &str) -> Option<Diagnostico> {
    let linhas: Vec<&str> = stderr.lines().collect();
    let ini = linhas
        .iter()
        .rposition(|l| l.starts_with("Traceback (most recent call last):"))?;
    let mut local = None;
    for l in &linhas[ini..] {
        let t = l.trim_start();
        if let Some(resto) = t.strip_prefix("File \"")
            && let Some((arq, depois)) = resto.split_once("\", line ")
        {
            let n: String = depois.chars().take_while(char::is_ascii_digit).collect();
            local = Some((arq.to_string(), n.parse().unwrap_or(0)));
        }
    }
    let excecao = linhas[ini..]
        .iter()
        .rev()
        .find(|l| !l.starts_with(' ') && !l.trim().is_empty() && !l.starts_with("Traceback"))?
        .trim();
    let (arq, linha) = local?;
    Some(Diagnostico {
        arquivo: relativo(&arq, raiz),
        linha,
        coluna: 0,
        nivel: "error".into(),
        mensagem: excecao.to_string(),
        codigo: excecao
            .split_once(':')
            .map_or(excecao, |(t, _)| t)
            .rsplit('.')
            .next()
            .map(str::to_string),
    })
}

fn relativo(p: &str, raiz: &str) -> String {
    p.strip_prefix(raiz)
        .map(|r| r.trim_start_matches('/'))
        .filter(|r| !r.is_empty())
        .unwrap_or(p)
        .to_string()
}

fn cauda(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

/// O `uv` offline diz «not found in the cache» e «network was disabled». A frase vira a
/// nossa: o modelo precisa saber que nao adianta tentar de novo, e o operador que o
/// remedio e povoar o cache do hospedeiro.
pub fn explicar_falha_de_dependencia(stderr: &str) -> String {
    let fora_do_cache = stderr.contains("not found in the cache")
        || stderr.contains("network was disabled")
        || stderr.contains("Network connectivity is disabled");
    let pacotes: Vec<&str> = stderr
        .split("Because ")
        .skip(1)
        .filter_map(|t| t.split_once(" was not found in the cache").map(|(p, _)| p))
        .map(|p| p.trim_start_matches("there is no version of ").trim())
        .collect();
    let detalhe: String = stderr
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.contains("SSL_CERT_FILE"))
        .take(12)
        .collect::<Vec<_>>()
        .join("\n");
    if fora_do_cache {
        format!(
            "dependencia fora do cache local do uv ({}); o sandbox nao tem rede, entao ela \
nao se baixa daqui: povoe o cache do hospedeiro (uv pip install no hospedeiro) e repita.\n{detalhe}",
            if pacotes.is_empty() {
                "pacote nao identificado".to_string()
            } else {
                pacotes.join(", ")
            }
        )
    } else {
        format!("falha ao instalar as dependencias:\n{detalhe}")
    }
}

/// Pasta de atalhos para o cache so leitura: o `uv` grava as marcas dele aqui (tmpfs) e le
/// o conteudo pelo atalho. `archive-*` ja chegam montados; `interpreter-*` fica de fora
/// porque o uv o regrava a cada sondagem e ele se refaz em milissegundos.
const ATALHOS_DO_CACHE: &str = r#"mkdir -p /tmp/phxclaw-py/cache
for d in /tmp/phxclaw-py/cache-ro/*/; do
  [ -d "$d" ] || continue
  n=$(basename "$d")
  case "$n" in archive-*|interpreter-*) continue;; esac
  mkdir -p "/tmp/phxclaw-py/cache/$n"
  for e in "$d"*; do [ -e "$e" ] && ln -s "$e" "/tmp/phxclaw-py/cache/$n/"; done
done"#;

impl Tool for PythonProjectTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "python_project".into(),
            description: "Work on a Python project inside the task directory (isolated sandbox, \
offline). Creates/uses <path>/.venv with uv and installs the dependencies of requirements.txt / \
pyproject.toml from the local cache. action=setup|test|lint|typecheck|run: test=pytest, \
lint=ruff check (fix=true applies safe fixes), typecheck=mypy, run=execute `script` with `args`. \
path is the project directory relative to the task directory (default '.'); target narrows \
test/lint/typecheck to a file, directory or pytest node id. Returns structured diagnostics: file, \
line, column, level, message, code (for pytest the code is the failing test id)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["setup","test","lint","typecheck","run"]},
                "path":{"type":"string"},
                "target":{"type":"string"},
                "script":{"type":"string"},
                "args":{"type":"array","items":{"type":"string"}},
                "fix":{"type":"boolean"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    /// O que roda de verdade em cada acao: regra sobre `python`, `uv`, `ruff` alcanca esta
    /// ferramenta como alcanca o `shell`.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
        Some(match s("action") {
            "setup" => "uv pip install".to_string(),
            "test" => "python -m pytest".to_string(),
            "lint" => "ruff check".to_string(),
            "typecheck" => "python -m mypy".to_string(),
            "run" => format!("python {}", aspas(s("script"))),
            outra => format!("python_project {}", aspas(outra)),
        })
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move { self.executar(args, ctx).await })
    }
}

impl PythonProjectTool {
    async fn executar(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let acao = arg_str(&args, "action")?.to_string();
        if !["setup", "test", "lint", "typecheck", "run"].contains(&acao.as_str()) {
            return Err(ToolError::InvalidArguments(format!(
                "action desconhecida: {acao} (setup, test, lint, typecheck, run)"
            )));
        }
        let rel = caminho_do_projeto(&ctx.workdir, arg_opt(&args, "path").unwrap_or("."))?;
        let dir = if rel.is_empty() {
            ctx.workdir.clone()
        } else {
            ctx.workdir.join(&rel)
        };
        let projeto = if rel.is_empty() { "." } else { rel.as_str() };
        if !dir.is_dir() {
            return Err(ToolError::InvalidArguments(format!(
                "pasta do projeto inexistente: {projeto}"
            )));
        }
        let guest = if rel.is_empty() {
            "/work".to_string()
        } else {
            format!("/work/{rel}")
        };
        // Tudo que vem do modelo e validado ANTES de qualquer processo: recusa nao cria
        // venv nem gasta o prazo.
        let alvo = match arg_opt(&args, "target") {
            None => ".".to_string(),
            Some(t) => alvo_valido(&dir, t)?,
        };
        let script_e_args = if acao == "run" {
            Some(script_valido(&dir, &args)?)
        } else {
            None
        };
        let fix = args.get("fix").and_then(Value::as_bool).unwrap_or(false);
        let i = &self.interpretador;

        // 1) venv e dependencias.
        let (fontes, hash) = fontes_de_dependencia(&dir, &i.versao);
        if !fontes.is_empty() && self.uv.is_none() {
            return Err(ToolError::Failed(
                "o projeto declara dependencias e nao ha uv no hospedeiro (PHXCLAW_UV): sem \
rede no sandbox, nao ha como instala-las"
                    .into(),
            ));
        }
        let criar = if self.uv.is_some() {
            format!(
                "{UV_GUEST} venv -q --python {} .venv",
                aspas(&i.executavel.display().to_string())
            )
        } else {
            format!(
                "{} -m venv --without-pip .venv",
                aspas(&i.executavel.display().to_string())
            )
        };
        let mut preparo = format!(
            "cd {g} || exit 90\n\
if ! .venv/bin/python -c '' 2>/dev/null; then\n  rm -rf .venv\n  {criar} || exit 91\n  echo @@venv-criado\nfi\n\
SP=$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()[\"purelib\"])') || exit 92\n\
printf '%s\\n' {site} > \"$SP/_phxclaw_ferramentas.pth\" || exit 92\n",
            g = aspas(&guest),
            site = aspas(&i.site.display().to_string()),
        );
        if !fontes.is_empty() {
            if acao == "setup" {
                preparo.push_str("rm -f .venv/.phxclaw-deps\n");
            }
            preparo.push_str(&format!(
                "if [ \"$(cat .venv/.phxclaw-deps 2>/dev/null)\" != '{hash}' ]; then\n\
{ATALHOS_DO_CACHE}\n\
  {UV_GUEST} pip install --python .venv/bin/python {fontes} || exit 93\n\
  printf '%s\\n' '{hash}' > .venv/.phxclaw-deps\n  echo @@deps-instaladas\nfi\n",
                fontes = fontes.join(" ")
            ));
        }
        let p = self.rodar(ctx, preparo, None).await?;
        let criado = p.stdout.contains("@@venv-criado");
        let dependencias = if fontes.is_empty() {
            "nenhuma declarada"
        } else if p.stdout.contains("@@deps-instaladas") {
            "instaladas"
        } else {
            "em dia"
        };
        if p.exit_code != Some(0) {
            let erro = match p.exit_code {
                Some(93) => explicar_falha_de_dependencia(&p.stderr),
                Some(91) => format!("nao foi possivel criar o venv:\n{}", cauda(&p.stderr, 2000)),
                c => format!(
                    "preparo do venv falhou ({c:?}):\n{}",
                    cauda(&p.stderr, 2000)
                ),
            };
            let saida = json!({
                "acao": acao, "projeto": projeto, "sucesso": false,
                "interpretador": i.versao,
                "venv": {"criado": criado, "dependencias": "falhou"},
                "erro": erro,
            });
            return Ok(ToolOutput::text(
                serde_json::to_string_pretty(&saida).unwrap_or_default(),
            ));
        }
        let venv = json!({"criado": criado, "dependencias": dependencias});
        if acao == "setup" {
            let saida = json!({
                "acao": acao, "projeto": projeto, "sucesso": true,
                "interpretador": i.versao, "venv": venv,
            });
            return Ok(ToolOutput::text(
                serde_json::to_string_pretty(&saida).unwrap_or_default(),
            ));
        }

        // 2) a acao. PYTHONPATH com a raiz e o `src/`: os dois layouts que se acham num
        // projeto sem instala-lo, e o pytest com rootdir sem conftest nao acha nenhum.
        let mut pythonpath = guest.clone();
        if dir.join("src").is_dir() {
            pythonpath.push_str(&format!(":{guest}/src"));
        }
        let marca = format!("@@phxclaw-{}", uuid::Uuid::now_v7().simple());
        let bin = i
            .executavel
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let script = match acao.as_str() {
            "test" => {
                pythonpath = format!("{PYTEST_PLUGIN_DIR}:{pythonpath}");
                format!(
                    "cd {g} || exit 90\nmkdir -p {PYTEST_PLUGIN_DIR}\n\
cat > {PYTEST_PLUGIN_DIR}/phxclaw_relatorio.py <<'PHXCLAW_FIM'\n{PLUGIN_PYTEST}PHXCLAW_FIM\n\
export PHXCLAW_PYTEST_JSON={PYTEST_PLUGIN_DIR}/relatorio.json\n\
.venv/bin/python -m pytest -q -p no:cacheprovider -p phxclaw_relatorio --tb=short {a}\n\
rc=$?\necho '{marca}'\ncat \"$PHXCLAW_PYTEST_JSON\" 2>/dev/null\nexit $rc",
                    g = aspas(&guest),
                    a = aspas(&alvo),
                )
            }
            "lint" => format!(
                "cd {g} || exit 90\nR=''\nfor c in .venv/bin/ruff {b}; do [ -x \"$c\" ] && R=\"$c\" && break; done\n\
[ -n \"$R\" ] || {{ echo 'No module named ruff' >&2; exit 94; }}\n\
exec \"$R\" check --no-cache --output-format=json {fix}{a}",
                g = aspas(&guest),
                b = aspas(&format!("{bin}/ruff")),
                fix = if fix { "--fix " } else { "" },
                a = aspas(&alvo),
            ),
            "typecheck" => format!(
                "cd {g} || exit 90\n\
exec .venv/bin/python -m mypy -O json --no-incremental --cache-dir=/dev/null --exclude '(^|/)\\.venv/' {a}",
                g = aspas(&guest),
                a = aspas(&alvo),
            ),
            _ => {
                let (s, a) = script_e_args.unwrap_or_default();
                let a: Vec<String> = a.iter().map(|x| aspas(x)).collect();
                format!(
                    "cd {g} || exit 90\nexec .venv/bin/python {s} {a}",
                    g = aspas(&guest),
                    s = aspas(&s),
                    a = a.join(" ")
                )
            }
        };
        let r = self.rodar(ctx, script, Some(pythonpath)).await?;
        let raiz = format!("{guest}/");
        let (mut stdout, relatorio) = match r.stdout.split_once(&marca) {
            Some((antes, depois)) => (antes.to_string(), Some(depois.to_string())),
            None => (r.stdout.clone(), None),
        };
        let mut testes = Value::Null;
        let mut texto = Vec::new();
        let diags = match acao.as_str() {
            "test" => {
                let (d, c) = diagnosticos_do_pytest(relatorio.as_deref().unwrap_or(""));
                testes = c;
                d
            }
            "lint" => diagnosticos_do_ruff(&stdout, &raiz),
            "typecheck" => {
                let (d, t) = diagnosticos_do_mypy(&stdout, &raiz);
                texto = t;
                d
            }
            _ => diagnostico_do_traceback(&r.stderr, &raiz)
                .into_iter()
                .collect(),
        };
        // Ferramenta ausente e recusa com nome, nao «exit 1» com um traceback.
        let faltando = ["pytest", "mypy", "ruff"]
            .into_iter()
            .find(|m| r.stderr.contains(&format!("No module named {m}")) && acao != "run");
        if let Some(m) = faltando {
            return Err(ToolError::Failed(format!(
                "{m} nao esta instalado nem no interpretador ({}) nem no venv do projeto",
                i.executavel.display()
            )));
        }
        if acao != "run" && acao != "test" {
            stdout.clear();
        }
        let erros = diags
            .iter()
            .filter(|d| matches!(d.nivel.as_str(), "error" | "failure"))
            .count();
        let avisos = diags.iter().filter(|d| d.nivel == "warning").count();
        let total = diags.len();
        let mut diags = diags;
        diags.truncate(100);
        let mut saida = json!({
            "acao": acao,
            "projeto": projeto,
            "interpretador": i.versao,
            "venv": venv,
            "exit_code": r.exit_code,
            "sucesso": r.exit_code == Some(0),
            "erros": erros,
            "avisos": avisos,
            "diagnosticos_total": total,
            "diagnosticos": diags,
            "saida_truncada": r.truncated,
        });
        if !testes.is_null() {
            saida["testes"] = testes;
        }
        if !texto.is_empty() {
            saida["texto"] = json!(texto);
        }
        if acao == "test" || acao == "run" {
            saida["saida_cauda"] = json!(cauda(&stdout, 4000));
        }
        if r.exit_code != Some(0) || acao == "run" {
            saida["stderr_cauda"] = json!(cauda(&r.stderr, 3000));
        }
        Ok(ToolOutput::text(
            serde_json::to_string_pretty(&saida).unwrap_or_default(),
        ))
    }
}

/// Alvo de test/lint/typecheck: arquivo ou pasta do projeto, com um `::no::de::teste`
/// opcional do pytest. Os caracteres do caminho sao os do `rust_project`; o resto do
/// nodeid aceita tambem `[` `]` (parametrizacao), e tudo entra entre aspas no shell.
fn alvo_valido(dir: &Path, t: &str) -> Result<String, ToolError> {
    let (caminho, no) = match t.split_once("::") {
        Some((c, n)) => (c, Some(n)),
        None => (t, None),
    };
    let rel = caminho_do_projeto(dir, caminho)?;
    let rel = if rel.is_empty() { ".".to_string() } else { rel };
    if !dir.join(&rel).exists() {
        return Err(ToolError::InvalidArguments(format!(
            "target inexistente no projeto: {rel}"
        )));
    }
    match no {
        None => Ok(rel),
        Some(n)
            if !n.is_empty()
                && n.len() <= 512
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_:[]-.".contains(c)) =>
        {
            Ok(format!("{rel}::{n}"))
        }
        Some(n) => Err(ToolError::InvalidArguments(format!(
            "nodeid invalido: {n:?}"
        ))),
    }
}

/// Script de `run`: um `.py` que existe dentro do projeto, e os argumentos como lista de
/// textos (cada um vira um argv, entre aspas simples).
fn script_valido(dir: &Path, args: &Value) -> Result<(String, Vec<String>), ToolError> {
    let s = arg_opt(args, "script")
        .ok_or_else(|| ToolError::InvalidArguments("action=run pede 'script'".into()))?;
    let rel = caminho_do_projeto(dir, s)?;
    if !rel.ends_with(".py") || !dir.join(&rel).is_file() {
        return Err(ToolError::InvalidArguments(format!(
            "script tem de ser um arquivo .py do projeto: {s}"
        )));
    }
    let lista = match args.get("args") {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(v)) => v
            .iter()
            .map(|x| match x {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) => Ok(n.to_string()),
                _ => Err(ToolError::InvalidArguments(
                    "args e uma lista de textos".into(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(ToolError::InvalidArguments(
                "args e uma lista de textos".into(),
            ));
        }
    };
    if lista.len() > ARGS_MAX || lista.iter().any(|a| a.contains('\0') || a.len() > 4096) {
        return Err(ToolError::InvalidArguments(format!(
            "args: no maximo {ARGS_MAX}, cada um ate 4096 bytes e sem NUL"
        )));
    }
    Ok((rel, lista))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspas_seguram_a_aspa_e_a_expansao() {
        assert_eq!(aspas("a b"), "'a b'");
        assert_eq!(aspas("x'; reboot; '"), r"'x'\''; reboot; '\'''");
        // O shell de verdade devolve o texto intacto, sem rodar nada.
        let o = Command::new("/bin/sh")
            .args(["-c", &format!("printf %s {}", aspas("$(id) `id` ' \" ; x"))])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), "$(id) `id` ' \" ; x");
    }

    #[test]
    fn ruff_e_mypy_viram_diagnostico_relativo() {
        let ruff = r#"[{"code":"F401","filename":"/work/p/calc.py","location":{"column":8,"row":1},"message":"`os` imported but unused","severity":"error"}]"#;
        let d = diagnosticos_do_ruff(ruff, "/work/p/");
        assert_eq!(d.len(), 1);
        assert_eq!(
            (d[0].arquivo.as_str(), d[0].linha, d[0].codigo.as_deref()),
            ("calc.py", 1, Some("F401"))
        );
        let mypy = "{\"file\": \"calc.py\", \"line\": 9, \"column\": 11, \"message\": \"Incompatible return value type\", \"hint\": null, \"code\": \"return-value\", \"severity\": \"error\"}\nmypy: aviso solto";
        let (d, t) = diagnosticos_do_mypy(mypy, "/work/p/");
        assert_eq!((d[0].linha, d[0].coluna), (9, 12));
        assert_eq!(d[0].codigo.as_deref(), Some("return-value"));
        assert_eq!(t, vec!["mypy: aviso solto".to_string()]);
    }

    #[test]
    fn traceback_aponta_a_ultima_linha_do_script() {
        let e = "Traceback (most recent call last):\n  File \"/work/p/main.py\", line 7, in <module>\n    f()\n  File \"/work/p/lib.py\", line 3, in f\n    1/0\nZeroDivisionError: division by zero\n";
        let d = diagnostico_do_traceback(e, "/work/p/").unwrap();
        assert_eq!((d.arquivo.as_str(), d.linha), ("lib.py", 3));
        assert_eq!(d.codigo.as_deref(), Some("ZeroDivisionError"));
        assert!(diagnostico_do_traceback("tudo certo", "/work/p/").is_none());
    }

    #[test]
    fn falha_de_cache_vira_frase_nossa_com_o_pacote() {
        let e = "  x No solution found when resolving dependencies:\n  Because naoexiste-xyz was not found in the cache and you require naoexiste-xyz, we can conclude...\n  hint: Packages were unavailable because the network was disabled.";
        let m = explicar_falha_de_dependencia(e);
        assert!(m.contains("fora do cache local"), "{m}");
        assert!(m.contains("naoexiste-xyz"), "{m}");
        let m = explicar_falha_de_dependencia("error: build backend falhou");
        assert!(!m.contains("fora do cache"), "{m}");
    }
}

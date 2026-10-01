//! `python_repl`: um interpretador Python que VIVE entre as chamadas da mesma tarefa --
//! variavel definida numa chamada existe na seguinte, como no REPL do OpenJarvis e no
//! notebook do Codex.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **O mesmo bwrap do `python_project`**, pelas mesmas `SandboxExtras` (interpretador,
//!   ambiente offline): uma segunda montagem divergiria da primeira no dia em que alguem
//!   endurecesse so uma. Se a pasta da tarefa ja tem `.venv` (criado pelo
//!   `python_project`), o REPL usa o interpretador dele e enxerga as dependencias.
//! - **Uma thread dona por sessao.** O bwrap roda com `--die-with-parent`, e no Linux o
//!   «pai» desse sinal e a THREAD que criou o processo. Criado numa thread do
//!   `spawn_blocking` do tokio, o REPL morreria sozinho quando o tokio aposentasse a thread
//!   ociosa (10 s) -- e a sessao perderia o estado calada. A thread dona vive enquanto a
//!   sessao vive.
//! - **Prazo estourado mata a sessao, e diz.** Um `while True` deixaria o interpretador
//!   ocupado para sempre; matar e recomecar perde o estado, e a resposta diz que perdeu
//!   para o modelo nao contar com variavel que nao existe mais.

use crate::python::PythonProjectTool;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_sandbox::{WorkdirCommand, workdir_sandbox_command_com};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// O lado Python do protocolo: le um pedido JSON por linha, executa no MESMO dicionario de
/// globais e devolve stdout, stderr, o valor da ultima expressao e o erro, numa linha que
/// comeca com a marca da sessao -- o que o codigo do usuario imprimir por fora (os.write
/// no fd 1) nao se confunde com a resposta.
const DRIVER: &str = r#"import sys, os, io, json, ast, contextlib, traceback
M = os.environ["PHXCLAW_REPL_MARCA"]
LIM = 20000
g = {"__name__": "__main__"}
out_real = sys.stdout
def corta(s):
    return s if len(s) <= LIM else s[:LIM] + "\n[... truncado: %d caracteres]" % len(s)
while True:
    linha = sys.stdin.readline()
    if not linha:
        break
    try:
        codigo = json.loads(linha).get("code", "")
    except Exception:
        continue
    o, e = io.StringIO(), io.StringIO()
    valor = None
    erro = None
    with contextlib.redirect_stdout(o), contextlib.redirect_stderr(e):
        try:
            arv = ast.parse(codigo, "<repl>", "exec")
            ultima = None
            if arv.body and isinstance(arv.body[-1], ast.Expr):
                ultima = ast.Expression(arv.body.pop().value)
            exec(compile(arv, "<repl>", "exec"), g)
            if ultima is not None:
                v = eval(compile(ultima, "<repl>", "eval"), g)
                if v is not None:
                    valor = repr(v)
        except SystemExit:
            erro = "SystemExit ignorado: a sessao continua"
        except BaseException:
            erro = traceback.format_exc()
    r = {"stdout": corta(o.getvalue()), "stderr": corta(e.getvalue()),
         "valor": None if valor is None else corta(valor), "erro": erro}
    out_real.write(M + json.dumps(r) + "\n")
    out_real.flush()
"#;

struct Pedido {
    codigo: String,
    prazo: Duration,
    resposta: mpsc::Sender<Result<Value, String>>,
}

pub struct PythonReplTool {
    /// O `python_project` cujas `SandboxExtras` o REPL usa.
    pub base: Arc<PythonProjectTool>,
    pub timeout: Duration,
    sessoes: Mutex<HashMap<String, mpsc::Sender<Pedido>>>,
}

impl PythonReplTool {
    pub fn new(base: Arc<PythonProjectTool>) -> Self {
        Self {
            base,
            timeout: Duration::from_secs(60),
            sessoes: Mutex::new(HashMap::new()),
        }
    }

    fn sessao(&self, task_id: &str, workdir: PathBuf) -> mpsc::Sender<Pedido> {
        let mut m = self.sessoes.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = m.get(task_id) {
            return tx.clone();
        }
        let (tx, rx) = mpsc::channel::<Pedido>();
        let bwrap = self.base.bwrap.clone();
        let exe = self.base.interpretador.executavel.display().to_string();
        let marca = format!("@@phxclaw-repl-{}@@", uuid::Uuid::now_v7().simple());
        let mut extras = self.base.extras(Some("/work".into()));
        extras
            .env
            .push(("PHXCLAW_REPL_DRIVER".into(), DRIVER.into()));
        extras
            .env
            .push(("PHXCLAW_REPL_MARCA".into(), marca.clone()));
        std::thread::spawn(move || dona(bwrap, workdir, exe, extras, marca, rx));
        m.insert(task_id.to_string(), tx.clone());
        tx
    }

    fn encerrar(&self, task_id: &str) {
        // Soltar o Sender encerra o laco da thread dona, que mata o processo.
        self.sessoes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(task_id);
    }
}

/// A thread dona: cria o processo, atende pedido a pedido, e o mata quando o canal fecha
/// (fim da tarefa) ou quando um pedido estoura o prazo.
fn dona(
    bwrap: PathBuf,
    workdir: PathBuf,
    exe: String,
    extras: phxclaw_sandbox::SandboxExtras,
    marca: String,
    rx: mpsc::Receiver<Pedido>,
) {
    use std::process::Stdio;
    let cmd = WorkdirCommand {
        workdir,
        script: format!(
            "if [ -x /work/.venv/bin/python ]; then P=/work/.venv/bin/python; else P={}; fi\n\
exec \"$P\" -u -c \"$PHXCLAW_REPL_DRIVER\"",
            crate::python::aspas(&exe)
        ),
        timeout: Duration::from_secs(0),
        network: false,
        max_output_bytes: 0,
    };
    let filho = workdir_sandbox_command_com(&bwrap, &cmd, &extras)
        .map_err(|e| e.to_string())
        .and_then(|mut c| {
            c.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())
        });
    let mut filho = match filho {
        Ok(f) => f,
        Err(e) => {
            for p in rx {
                let _ = p
                    .resposta
                    .send(Err(format!("nao foi possivel abrir o REPL: {e}")));
            }
            return;
        }
    };
    let mut entrada = filho.stdin.take().expect("stdin piped");
    let saida = filho.stdout.take().expect("stdout piped");
    let (tx_linhas, linhas) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines() {
            let Ok(l) = l else { break };
            if tx_linhas.send(l).is_err() {
                break;
            }
        }
    });
    for p in rx {
        let pedido = json!({"code": p.codigo}).to_string();
        if writeln!(entrada, "{pedido}")
            .and_then(|_| entrada.flush())
            .is_err()
        {
            let _ = p.resposta.send(Err(
                "o REPL terminou; a sessao recomeca na proxima chamada".into()
            ));
            break;
        }
        let limite = std::time::Instant::now() + p.prazo;
        let r = loop {
            let falta = limite.saturating_duration_since(std::time::Instant::now());
            match linhas.recv_timeout(falta) {
                Ok(l) => {
                    if let Some(j) = l.strip_prefix(&marca) {
                        break serde_json::from_str::<Value>(j).map_err(|e| e.to_string());
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    break Err(format!(
                        "prazo de {} s estourado: a sessao foi encerrada e o estado (variaveis, imports) se perdeu",
                        p.prazo.as_secs()
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Err("o processo do REPL terminou (estado perdido); a proxima chamada abre outro".into());
                }
            }
        };
        let morreu = r.is_err();
        let _ = p.resposta.send(r);
        if morreu {
            break;
        }
    }
    let _ = filho.kill();
    let _ = filho.wait();
}

impl Tool for PythonReplTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "python_repl".into(),
            description: "Persistent Python interpreter for this task (isolated sandbox, offline, \
cwd /work = task directory). Variables, functions and imports survive between calls, like a \
notebook. Returns stdout, stderr, the repr of the last expression and the traceback if any. \
reset=true starts a fresh interpreter."
                .into(),
            parameters: json!({"type":"object","properties":{
                "code":{"type":"string"},
                "reset":{"type":"boolean"}
            },"required":["code"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    /// O codigo vai entre aspas: a regra ve `python -c ...` (regra sobre `python` alcanca o
    /// REPL), e o texto Python nao vira palavras de shell soltas.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        let c = args.get("code").and_then(Value::as_str).unwrap_or("");
        Some(format!("python -c {}", crate::python::aspas(c)))
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let codigo = args
                .get("code")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'code'".into()))?
                .to_string();
            if args.get("reset").and_then(Value::as_bool) == Some(true) {
                self.encerrar(&ctx.task_id);
            }
            // Um pouco antes do prazo do motor: a sessao morre pelo nosso prazo, com a
            // explicacao, e nao pelo do motor, que abandonaria a espera com o processo vivo.
            let prazo = self
                .timeout
                .min(ctx.timeout.saturating_sub(Duration::from_millis(500)))
                .max(Duration::from_secs(1));
            let (tx, rx) = mpsc::channel();
            let pedido = Pedido {
                codigo,
                prazo,
                resposta: tx,
            };
            let enviado = self.sessao(&ctx.task_id, ctx.workdir.clone()).send(pedido);
            if let Err(mpsc::SendError(p)) = enviado {
                // A thread dona ja tinha saido (prazo anterior): abre outra e repete.
                self.encerrar(&ctx.task_id);
                let _ = self.sessao(&ctx.task_id, ctx.workdir.clone()).send(p);
            }
            let r = tokio::task::spawn_blocking(move || rx.recv())
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(|_| ToolError::Failed("o REPL nao respondeu".into()));
            let r = match r {
                Ok(Ok(v)) => v,
                Ok(Err(e)) | Err(ToolError::Failed(e)) => {
                    self.encerrar(&ctx.task_id);
                    return Err(ToolError::Failed(e));
                }
                Err(e) => return Err(e),
            };
            let mut s = String::new();
            for (campo, rotulo) in [("stdout", "stdout"), ("stderr", "stderr")] {
                if let Some(t) = r[campo].as_str().filter(|t| !t.is_empty()) {
                    s.push_str(&format!("{rotulo}:\n{t}\n"));
                }
            }
            if let Some(v) = r["valor"].as_str() {
                s.push_str(&format!("=> {v}\n"));
            }
            if let Some(e) = r["erro"].as_str() {
                s.push_str(&format!("error:\n{e}\n"));
            }
            if s.is_empty() {
                s.push_str("(ok, no output)");
            }
            Ok(ToolOutput::text(s))
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(async move { self.encerrar(task_id) })
    }
}

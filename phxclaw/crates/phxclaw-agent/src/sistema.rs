//! Ferramentas do agente sobre a maquina Linux onde ele roda: sistema (servicos, eventos,
//! processos, painel), rede (diagnostico), PostgreSQL e projeto Rust.
//!
//! Duas decisoes valem para o arquivo inteiro:
//!
//! - **Ler e o padrao; mudar pede outra capacidade, e outra capacidade e outra
//!   ferramenta.** O motor confere UMA capacidade por ferramenta, num portao so
//!   (`call_tool`). Conferir `system.admin` aqui dentro seria um segundo portao, e o
//!   `ToolContext` nem sabe o que foi concedido. Por isso cada par leitura/escrita e a
//!   mesma struct registrada duas vezes (`admin`, `lan`, `write`): a decisao de quem
//!   pode o que continua num lugar so, e a ferramenta negada nem aparece para o modelo.
//! - **Argumento do modelo nunca passa por shell.** Cada comando e um argv montado com
//!   valores validados (unidade com caracteres de unidade, PID numerico, IP ja resolvido).
//!   Os unicos `sh -c` sao o do `rust_project` e o do `python_project` (`python.rs`),
//!   dentro do bwrap: o modelo escolhe uma acao de uma lista e caminhos com caracteres
//!   permitidos, e os argumentos do script do `python_project` entram entre aspas simples.

use crate::tarefa::confine;
use phxclaw_agent_core::{
    BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use phxclaw_egress_broker::{EgressBroker, EgressPolicy};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, run_in_workdir_com};
use serde_json::{Value, json};
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Teto do texto devolvido ao modelo por chamada. O motor corta de novo no proprio teto;
/// este existe para a ferramenta dizer ONDE cortou em vez de o corte cair no meio.
const TEXTO_MAX: usize = 12_000;
/// Teto de bytes lidos de stdout e de stderr de um comando do hospedeiro.
const SAIDA_MAX: usize = 256 * 1024;

pub(crate) fn arg_str<'a>(args: &'a Value, nome: &str) -> Result<&'a str, ToolError> {
    args.get(nome)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{nome}'")))
}

pub(crate) fn arg_opt<'a>(args: &'a Value, nome: &str) -> Option<&'a str> {
    args.get(nome)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Inteiro que o modelo pode mandar como numero ou como texto; fora da faixa e recusa,
/// e nao corte silencioso: o modelo pediu outra coisa e precisa saber.
fn arg_int(args: &Value, nome: &str, padrao: u64, min: u64, max: u64) -> Result<u64, ToolError> {
    let v = match args.get(nome) {
        None | Some(Value::Null) => return Ok(padrao),
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| ToolError::InvalidArguments(format!("'{nome}' tem de ser inteiro")))?;
    if !(min..=max).contains(&v) {
        return Err(ToolError::InvalidArguments(format!(
            "'{nome}' fora da faixa {min}..={max}: {v}"
        )));
    }
    Ok(v)
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<T, ToolError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?
}

// ---------------------------------------------------------------------------------------
// Comando do hospedeiro
// ---------------------------------------------------------------------------------------

struct Saida {
    codigo: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Saida {
    fn ok(&self) -> bool {
        self.codigo == Some(0)
    }
    fn tudo(&self) -> String {
        let mut t = self.stdout.trim_end().to_string();
        if !self.stderr.trim().is_empty() {
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(self.stderr.trim_end());
        }
        t
    }
}

/// Binario procurado em diretorios fixos, nunca pelo PATH do processo: o PATH herdado e
/// configuracao de quem lancou o agente, e um diretorio gravavel nele trocaria o
/// `systemctl` por outro programa com o mesmo nome.
pub(crate) fn binario(nome: &str) -> Option<PathBuf> {
    [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
    ]
    .iter()
    .map(|d| Path::new(d).join(nome))
    .find(|p| p.is_file())
}

/// Roda um argv no hospedeiro com prazo e ambiente limpo. Ambiente limpo porque o do
/// agente carrega chaves de provedor e a URL do banco; nenhum destes comandos precisa
/// delas, e um `ps e` ou um diagnostico qualquer as mostraria.
fn rodar(argv: &[&str], prazo: Duration) -> Result<Saida, ToolError> {
    let prog = binario(argv[0])
        .ok_or_else(|| ToolError::Failed(format!("{} nao esta instalado aqui", argv[0])))?;
    let mut child = Command::new(&prog)
        .args(&argv[1..])
        .env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .env("LANG", "C.UTF-8")
        .env("TERM", "dumb")
        .env("SYSTEMD_PAGER", "")
        .env("SYSTEMD_COLORS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ToolError::Failed(format!("{}: {e}", argv[0])))?;
    // Lido em threads enquanto roda: ler so no fim trava o filho com o pipe cheio.
    let leitor = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(r) = r {
                let _ = r.take(SAIDA_MAX as u64).read_to_end(&mut v);
            }
            String::from_utf8_lossy(&v).into_owned()
        })
    };
    let o = leitor(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let e = leitor(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let ini = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if ini.elapsed() >= prazo => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError::Timeout(prazo.as_millis() as u64));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(err) => return Err(ToolError::Failed(err.to_string())),
        }
    };
    Ok(Saida {
        codigo: status.code(),
        stdout: o.join().unwrap_or_default(),
        stderr: e.join().unwrap_or_default(),
    })
}

/// A mesma pergunta do `sd_booted()`: o diretorio so existe quando o PID 1 e o systemd.
/// Em conteiner sem systemd o `systemctl` responde com um erro de D-Bus que manda
/// procurar o problema no lugar errado.
pub fn systemd_rodando() -> bool {
    Path::new("/run/systemd/system").is_dir()
}

const SEM_SYSTEMD: &str = "systemd nao esta rodando aqui (sem /run/systemd/system: conteiner ou \
outro init); servicos e acoes de unidade nao se aplicam a esta maquina";

/// Nome de unidade do systemd: so os caracteres que o proprio systemd aceita, e nunca
/// comecando por '-', que viraria opcao do `systemctl`.
pub fn validar_unidade(u: &str) -> Result<&str, ToolError> {
    let ok = !u.is_empty()
        && u.len() <= 256
        && !u.starts_with('-')
        && u.chars()
            .all(|c| c.is_ascii_alphanumeric() || ":-_.@\\".contains(c));
    if ok {
        Ok(u)
    } else {
        Err(ToolError::InvalidArguments(format!(
            "nome de unidade invalido: {u:?} (letras, digitos e :-_.@\\, sem '-' no inicio)"
        )))
    }
}

/// PID de processo alheio: numerico, maior que 1, nao e o proprio agente, e existe.
pub fn validar_pid(v: Option<&Value>) -> Result<u32, ToolError> {
    let pid: u32 = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|x| u32::try_from(x).ok()),
        Some(Value::String(s)) if s.chars().all(|c| c.is_ascii_digit()) => s.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| ToolError::InvalidArguments("'pid' tem de ser um numero".into()))?;
    if pid <= 1 {
        return Err(ToolError::Denied(format!(
            "PID {pid} e o init ou invalido; nao se sinaliza"
        )));
    }
    if pid == std::process::id() {
        return Err(ToolError::Denied("PID do proprio agente".into()));
    }
    if !Path::new(&format!("/proc/{pid}")).exists() {
        return Err(ToolError::InvalidArguments(format!(
            "nao ha processo {pid} nesta maquina"
        )));
    }
    Ok(pid)
}

fn validar_prioridade(p: &str) -> Result<&str, ToolError> {
    const NOMES: [&str; 16] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "emerg", "alert", "crit", "err", "warning",
        "notice", "info", "debug",
    ];
    let ok = p.split("..").count() <= 2 && p.split("..").all(|x| NOMES.contains(&x));
    if ok {
        Ok(p)
    } else {
        Err(ToolError::InvalidArguments(format!(
            "prioridade invalida: {p:?} (0-7, emerg..debug, ou faixa como err..warning)"
        )))
    }
}

fn validar_desde(s: &str) -> Result<&str, ToolError> {
    let ok = s.len() <= 40
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || " :-+.".contains(c));
    if ok {
        Ok(s)
    } else {
        Err(ToolError::InvalidArguments(format!(
            "'since' invalido: {s:?} (ex.: \"2026-10-01 08:00\", \"-1h\", \"yesterday\")"
        )))
    }
}

// ---------------------------------------------------------------------------------------
// linux_system
// ---------------------------------------------------------------------------------------

/// Sistema Linux: servicos, eventos (journal), processos e o «painel de controle».
/// `admin = false` e a ferramenta `linux_system` (`system.read`); `admin = true` e a
/// `linux_system_admin` (`system.admin`), que so tem as acoes que mudam a maquina.
pub struct LinuxSystemTool {
    pub admin: bool,
    pub timeout: Duration,
}

impl LinuxSystemTool {
    pub fn leitura() -> Self {
        Self {
            admin: false,
            timeout: Duration::from_secs(20),
        }
    }
    pub fn admin() -> Self {
        Self {
            admin: true,
            timeout: Duration::from_secs(60),
        }
    }
}

impl Tool for LinuxSystemTool {
    fn spec(&self) -> ToolSpec {
        if self.admin {
            return ToolSpec {
                name: "linux_system_admin".into(),
                description: "Change this Linux machine. action=service with operation \
(start|stop|restart|enable|disable) and unit; action=kill with pid and optional signal \
(TERM default, KILL, HUP, INT). Read-only inspection is the linux_system tool."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["service","kill"]},
                    "operation":{"type":"string","enum":["start","stop","restart","enable","disable"]},
                    "unit":{"type":"string"},
                    "pid":{"type":"integer"},
                    "signal":{"type":"string","enum":["TERM","KILL","HUP","INT"]}
                },"required":["action"]}),
            };
        }
        ToolSpec {
            name: "linux_system".into(),
            description: "Inspect this Linux machine (read only). action=services (systemd \
units), service_status (unit), journal (lines, since, unit, priority), processes (sort=cpu|mem, \
filter, limit), panel (item=all|hostname|time|locale|uptime|kernel|memory|disk|block), \
packages (filter, limit), applications (installed .desktop apps, filter)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["services","service_status","journal","processes","panel","packages","applications"]},
                "unit":{"type":"string"},
                "lines":{"type":"integer"},
                "since":{"type":"string"},
                "priority":{"type":"string"},
                "sort":{"type":"string","enum":["cpu","mem"]},
                "filter":{"type":"string"},
                "limit":{"type":"integer"},
                "item":{"type":"string"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        if self.admin {
            "system.admin"
        } else {
            "system.read"
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        let prazo = self.timeout.min(ctx.timeout);
        let admin = self.admin;
        Box::pin(async move {
            let texto = bloqueante(move || {
                let acao = arg_str(&args, "action")?;
                if admin {
                    sistema_admin(acao, &args, prazo)
                } else {
                    sistema_leitura(acao, &args, prazo)
                }
            })
            .await?;
            Ok(ToolOutput::text(truncate_for_model(&texto, TEXTO_MAX)))
        })
    }
}

fn sistema_leitura(acao: &str, args: &Value, prazo: Duration) -> Result<String, ToolError> {
    match acao {
        "services" => {
            if !systemd_rodando() {
                return Ok(SEM_SYSTEMD.into());
            }
            let s = rodar(
                &[
                    "systemctl",
                    "list-units",
                    "--type=service",
                    "--all",
                    "--no-pager",
                    "--plain",
                    "--no-legend",
                ],
                prazo,
            )?;
            let filtro = arg_opt(args, "filter").map(str::to_lowercase);
            let linhas: Vec<&str> = s
                .stdout
                .lines()
                .filter(|l| filtro.as_ref().is_none_or(|f| l.to_lowercase().contains(f)))
                .collect();
            Ok(format!(
                "UNIDADE CARGA ATIVA SUB DESCRICAO ({} unidades)\n{}",
                linhas.len(),
                linhas.join("\n")
            ))
        }
        "service_status" => {
            let u = validar_unidade(arg_str(args, "unit")?)?;
            if !systemd_rodando() {
                return Ok(SEM_SYSTEMD.into());
            }
            // Codigo 3 e «inativa», nao falha: o texto do status e a resposta.
            let s = rodar(
                &["systemctl", "status", "--no-pager", "--lines=20", "--", u],
                prazo,
            )?;
            Ok(format!(
                "exit_code: {}\n{}",
                s.codigo.map(|c| c.to_string()).unwrap_or("sinal".into()),
                s.tudo()
            ))
        }
        "journal" => {
            let tem_diario = systemd_rodando()
                || Path::new("/var/log/journal").is_dir()
                || Path::new("/run/log/journal").is_dir();
            if !tem_diario {
                return Ok(format!(
                    "{SEM_SYSTEMD}; e nao ha diario gravado em /var/log/journal"
                ));
            }
            let n = arg_int(args, "lines", 100, 1, 1000)?;
            let mut argv: Vec<String> = vec![
                "journalctl".into(),
                "--no-pager".into(),
                "-o".into(),
                "short-iso".into(),
                format!("--lines={n}"),
            ];
            if let Some(d) = arg_opt(args, "since") {
                // `--since=VALOR` num argumento so: um valor com '-' na frente nao vira opcao.
                argv.push(format!("--since={}", validar_desde(d)?));
            }
            if let Some(u) = arg_opt(args, "unit") {
                argv.push(format!("--unit={}", validar_unidade(u)?));
            }
            if let Some(p) = arg_opt(args, "priority") {
                argv.push(format!("--priority={}", validar_prioridade(p)?));
            }
            let a: Vec<&str> = argv.iter().map(String::as_str).collect();
            let s = rodar(&a, prazo)?;
            Ok(s.tudo())
        }
        "processes" => processos(args, prazo),
        "panel" => painel(arg_opt(args, "item").unwrap_or("all"), prazo),
        "packages" => pacotes(args, prazo),
        "applications" => Ok(aplicativos(arg_opt(args, "filter"))),
        outra => Err(ToolError::InvalidArguments(format!(
            "action desconhecida: {outra} (services, service_status, journal, processes, panel, \
packages, applications)"
        ))),
    }
}

fn sistema_admin(acao: &str, args: &Value, prazo: Duration) -> Result<String, ToolError> {
    match acao {
        "service" => {
            let op = arg_str(args, "operation")?;
            if !["start", "stop", "restart", "enable", "disable"].contains(&op) {
                return Err(ToolError::InvalidArguments(format!(
                    "operation invalida: {op} (start, stop, restart, enable, disable)"
                )));
            }
            let u = validar_unidade(arg_str(args, "unit")?)?;
            if !systemd_rodando() {
                return Ok(SEM_SYSTEMD.into());
            }
            let s = rodar(&["systemctl", op, "--no-ask-password", "--", u], prazo)?;
            if s.ok() {
                Ok(format!("systemctl {op} {u}: feito\n{}", s.tudo()))
            } else {
                Err(ToolError::Failed(format!(
                    "systemctl {op} {u} saiu com {:?}: {}",
                    s.codigo,
                    s.tudo()
                )))
            }
        }
        "kill" => {
            let pid = validar_pid(args.get("pid"))?;
            let sinal = arg_opt(args, "signal").unwrap_or("TERM");
            if !["TERM", "KILL", "HUP", "INT"].contains(&sinal) {
                return Err(ToolError::InvalidArguments(format!(
                    "signal invalido: {sinal} (TERM, KILL, HUP, INT)"
                )));
            }
            let cmd = std::fs::read(format!("/proc/{pid}/cmdline"))
                .map(|b| String::from_utf8_lossy(&b).replace('\0', " "))
                .unwrap_or_default();
            let p = pid.to_string();
            let s = rodar(&["kill", "-s", sinal, &p], prazo)?;
            if s.ok() {
                Ok(format!(
                    "sinal {sinal} enviado ao PID {pid} ({})",
                    cmd.trim()
                ))
            } else {
                Err(ToolError::Failed(format!(
                    "kill -s {sinal} {pid}: {}",
                    s.tudo()
                )))
            }
        }
        outra => Err(ToolError::InvalidArguments(format!(
            "action desconhecida: {outra} (service, kill)"
        ))),
    }
}

fn processos(args: &Value, prazo: Duration) -> Result<String, ToolError> {
    let ordem = match arg_opt(args, "sort").unwrap_or("cpu") {
        "cpu" => "--sort=-pcpu",
        "mem" => "--sort=-rss",
        o => {
            return Err(ToolError::InvalidArguments(format!(
                "sort invalido: {o} (cpu, mem)"
            )));
        }
    };
    let limite = arg_int(args, "limit", 30, 1, 500)? as usize;
    let filtro = arg_opt(args, "filter").map(str::to_lowercase);
    let s = rodar(
        &["ps", "-eo", "pid=,user=,pcpu=,pmem=,rss=,args=", ordem],
        prazo,
    )?;
    if !s.ok() {
        return Err(ToolError::Failed(s.tudo()));
    }
    let mut linhas = vec!["PID\tUSUARIO\tCPU%\tMEM%\tRSS_KIB\tCOMANDO".to_string()];
    let mut total = 0;
    for l in s.stdout.lines() {
        let mut c = l.split_whitespace();
        let (Some(pid), Some(u), Some(cpu), Some(mem), Some(rss)) =
            (c.next(), c.next(), c.next(), c.next(), c.next())
        else {
            continue;
        };
        let cmd: String = c.collect::<Vec<_>>().join(" ");
        if filtro
            .as_ref()
            .is_some_and(|f| !cmd.to_lowercase().contains(f) && !u.to_lowercase().contains(f))
        {
            continue;
        }
        total += 1;
        if linhas.len() <= limite {
            let cmd: String = cmd.chars().take(160).collect();
            linhas.push(format!("{pid}\t{u}\t{cpu}\t{mem}\t{rss}\t{cmd}"));
        }
    }
    linhas.push(format!(
        "({total} processos casaram; mostrados ate {limite})"
    ));
    Ok(linhas.join("\n"))
}

fn ler(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn os_release() -> String {
    ler("/etc/os-release")
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default()
}

/// Um item do painel. Os tres que dependem de servico do systemd (hostnamed, timedated,
/// localed) caem para a leitura de /etc quando ele nao roda, dizendo que cairam.
fn painel_item(item: &str, prazo: Duration) -> Result<String, ToolError> {
    let via = |argv: &[&str]| -> String {
        match rodar(argv, prazo) {
            Ok(s) => s.tudo(),
            Err(e) => format!("indisponivel: {e}"),
        }
    };
    let systemd = systemd_rodando();
    Ok(match item {
        "hostname" if systemd => via(&["hostnamectl", "status"]),
        "hostname" => format!(
            "(systemd nao esta rodando aqui; lido de /etc)\nHostname: {}\nSistema: {}\nKernel: {}",
            ler("/etc/hostname").trim(),
            os_release(),
            ler("/proc/sys/kernel/osrelease").trim()
        ),
        "time" if systemd => via(&["timedatectl", "status"]),
        "time" => format!(
            "(systemd nao esta rodando aqui; lido de /etc)\nAgora: {}\nFuso: {}",
            via(&["date", "--iso-8601=seconds"]),
            std::fs::read_link("/etc/localtime")
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| ler("/etc/timezone").trim().to_string())
        ),
        "locale" if systemd => via(&["localectl", "status"]),
        "locale" => format!(
            "(systemd nao esta rodando aqui; lido de /etc)\n{}",
            ler("/etc/default/locale").trim()
        ),
        "uptime" => via(&["uptime"]),
        "kernel" => via(&["uname", "-a"]),
        "memory" => via(&["free", "-h"]),
        "disk" => via(&[
            "df", "-hT", "-x", "tmpfs", "-x", "devtmpfs", "-x", "overlay",
        ]),
        "block" => via(&["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT"]),
        outro => {
            return Err(ToolError::InvalidArguments(format!(
                "item desconhecido: {outro} (all, hostname, time, locale, uptime, kernel, memory, \
disk, block)"
            )));
        }
    })
}

fn painel(item: &str, prazo: Duration) -> Result<String, ToolError> {
    const ITENS: [&str; 8] = [
        "hostname", "time", "locale", "uptime", "kernel", "memory", "disk", "block",
    ];
    if item != "all" {
        return painel_item(item, prazo);
    }
    let mut t = String::new();
    for i in ITENS {
        t.push_str(&format!("== {i} ==\n{}\n\n", painel_item(i, prazo)?));
    }
    Ok(t)
}

fn pacotes(args: &Value, prazo: Duration) -> Result<String, ToolError> {
    let limite = arg_int(args, "limit", 200, 1, 5000)? as usize;
    let filtro = arg_opt(args, "filter").map(str::to_lowercase);
    let s = rodar(
        &[
            "dpkg-query",
            "-W",
            "-f=${db:Status-Abbrev}\t${Package}\t${Version}\n",
        ],
        prazo,
    )?;
    let instalados: Vec<String> = s
        .stdout
        .lines()
        .filter(|l| l.starts_with("ii"))
        .filter_map(|l| {
            let mut c = l.split('\t').skip(1);
            Some(format!("{}\t{}", c.next()?, c.next()?))
        })
        .collect();
    let casados: Vec<&String> = instalados
        .iter()
        .filter(|l| filtro.as_ref().is_none_or(|f| l.to_lowercase().contains(f)))
        .collect();
    let mut t = format!(
        "{} pacotes instalados; {} casaram; mostrados ate {limite}\nPACOTE\tVERSAO\n",
        instalados.len(),
        casados.len()
    );
    for l in casados.iter().take(limite) {
        t.push_str(l);
        t.push('\n');
    }
    Ok(t)
}

/// Aplicativos instalados = entradas `.desktop` do tipo Application, as mesmas que o menu
/// do ambiente grafico mostra. `NoDisplay=true` fica de fora, como no menu.
fn aplicativos(filtro: Option<&str>) -> String {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join(".local/share/applications"));
    }
    let filtro = filtro.map(str::to_lowercase);
    let mut apps = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            let texto = std::fs::read_to_string(&p).unwrap_or_default();
            let (mut nome, mut exec, mut tipo, mut oculto) = (None, None, None, false);
            let mut na_secao = false;
            for l in texto.lines() {
                let l = l.trim();
                if l.starts_with('[') {
                    na_secao = l == "[Desktop Entry]";
                    continue;
                }
                if !na_secao {
                    continue;
                }
                if let Some(v) = l.strip_prefix("Name=") {
                    nome.get_or_insert(v.to_string());
                } else if let Some(v) = l.strip_prefix("Exec=") {
                    exec.get_or_insert(v.to_string());
                } else if let Some(v) = l.strip_prefix("Type=") {
                    tipo = Some(v.to_string());
                } else if l == "NoDisplay=true" || l == "Hidden=true" {
                    oculto = true;
                }
            }
            if oculto || tipo.as_deref() != Some("Application") {
                continue;
            }
            let linha = format!(
                "{}\t{}\t{}",
                nome.unwrap_or_default(),
                exec.unwrap_or_default(),
                p.display()
            );
            if filtro
                .as_ref()
                .is_none_or(|f| linha.to_lowercase().contains(f))
            {
                apps.push(linha);
            }
        }
    }
    apps.sort();
    apps.dedup();
    if apps.is_empty() {
        return "(nenhum aplicativo .desktop instalado)".into();
    }
    format!(
        "{} aplicativos\nNOME\tEXEC\tARQUIVO\n{}",
        apps.len(),
        apps.join("\n")
    )
}

// ---------------------------------------------------------------------------------------
// network
// ---------------------------------------------------------------------------------------

/// Diagnostico de rede. `lan = false` e a `network` (`net.diagnose`): interfaces, rotas,
/// portas em escuta, DNS e sondas a destino PUBLICO. `lan = true` e a `network_lan`
/// (`net.lan`): as mesmas sondas, so para loopback e rede privada.
///
/// A sonda e a rede: e o mesmo SSRF do navegador por outra porta. Por isso o destino e
/// resolvido UMA vez, classificado pelo `is_blocked_ip` do navegador (a mesma funcao, nao
/// uma copia), e a conexao vai ao IP resolvido, nunca ao nome de novo -- reresolver
/// deixaria um DNS que muda de resposta trocar o destino entre a conferencia e a conexao.
pub struct NetworkTool {
    pub lan: bool,
    pub egress: EgressBroker,
}

/// Variavel com os destinos publicos liberados para a sonda TCP, `host:porta` separados
/// por virgula. Nega por padrao, como o broker.
pub const NET_DESTINOS_ENV: &str = "PHXCLAW_NET_DESTINOS";

/// Origem da sonda TCP na lingua do broker. O broker conhece origem por URL; a sonda se
/// escreve como `https://host:porta` para usar a MESMA lista e o MESMO validador, em vez
/// de uma segunda lista de destinos que divergiria da primeira.
pub fn origem_tcp(host: &str, porta: u16) -> String {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    let h = match h.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => format!("[{v6}]"),
        Ok(ip) => ip.to_string(),
        Err(_) => h.to_ascii_lowercase(),
    };
    // A URL esconde a porta padrao do esquema; a lista tem de esconder igual.
    if porta == 443 {
        format!("https://{h}")
    } else {
        format!("https://{h}:{porta}")
    }
}

impl NetworkTool {
    pub fn new(lan: bool, destinos: &[String]) -> Self {
        let mut p = EgressPolicy {
            enabled: true,
            ..EgressPolicy::default()
        };
        for d in destinos {
            if let Some((h, porta)) = d.trim().rsplit_once(':')
                && let Ok(porta) = porta.parse::<u16>()
            {
                p.allowed_origins.insert(origem_tcp(h, porta));
            }
        }
        Self {
            lan,
            egress: EgressBroker::new(p),
        }
    }
    pub fn do_ambiente(lan: bool) -> Self {
        let destinos: Vec<String> = std::env::var(NET_DESTINOS_ENV)
            .unwrap_or_default()
            .split(',')
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
            .collect();
        Self::new(lan, &destinos)
    }

    /// Resolve, classifica e decide. Devolve os enderecos que a sonda pode tocar.
    fn liberar(
        &self,
        host: &str,
        porta: Option<u16>,
        prazo: Duration,
    ) -> Result<Vec<SocketAddr>, ToolError> {
        let host = validar_host(host)?;
        let ends = resolver(host, porta.unwrap_or(0), prazo).map_err(ToolError::Failed)?;
        let internos: Vec<IpAddr> = ends
            .iter()
            .map(SocketAddr::ip)
            .filter(|ip| phxclaw_browser::is_blocked_ip(*ip))
            .collect();
        if self.lan {
            if internos.len() != ends.len() {
                return Err(ToolError::Denied(format!(
                    "{host} resolve para endereco publico; destino publico e da ferramenta network \
(net.diagnose)"
                )));
            }
            return Ok(ends);
        }
        // Basta um interno para recusar, como no navegador: quem conecta escolhe o endereco.
        if let Some(ip) = internos.first() {
            return Err(ToolError::Denied(format!(
                "{host} resolve para endereco interno {ip}; loopback e rede privada pedem a \
capacidade net.lan (ferramenta network_lan)"
            )));
        }
        // ICMP nao tem porta, e o broker so conhece origem com porta: ping e traceroute a
        // destino publico passam so pela classificacao. A sonda TCP, que abre conexao de
        // verdade, passa pela lista do broker.
        if let Some(p) = porta {
            self.egress
                .validate_url(&origem_tcp(host, p))
                .map_err(|e| {
                    ToolError::Denied(format!(
                        "politica de egresso recusou {host}:{p} ({e}); o operador libera em \
{NET_DESTINOS_ENV}=host:porta"
                    ))
                })?;
        }
        Ok(ends)
    }
}

fn validar_host(h: &str) -> Result<&str, ToolError> {
    let h = h.trim();
    let sem = h.trim_start_matches('[').trim_end_matches(']');
    if sem.parse::<IpAddr>().is_ok() {
        return Ok(sem);
    }
    let ok = !h.is_empty()
        && h.len() <= 253
        && !h.starts_with('-')
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if ok {
        Ok(h)
    } else {
        Err(ToolError::InvalidArguments(format!(
            "host invalido: {h:?} (nome DNS ou IP)"
        )))
    }
}

/// Resolucao com prazo. O `ToSocketAddrs` da std nao tem prazo; roda numa thread, e quem
/// espera desiste no prazo (a thread termina sozinha quando o resolvedor do sistema
/// desistir).
pub fn resolver(host: &str, porta: u16, prazo: Duration) -> Result<Vec<SocketAddr>, String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, porta)]);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let alvo = (host.to_string(), porta);
    std::thread::spawn(move || {
        let r = alvo
            .to_socket_addrs()
            .map(|i| i.collect::<Vec<_>>())
            .map_err(|e| e.to_string());
        let _ = tx.send(r);
    });
    match rx.recv_timeout(prazo) {
        Ok(Ok(v)) if v.is_empty() => Err(format!("{host}: DNS nao devolveu endereco")),
        Ok(Ok(mut v)) => {
            v.sort();
            v.dedup();
            Ok(v)
        }
        Ok(Err(e)) => Err(format!("{host}: DNS falhou: {e}")),
        Err(_) => Err(format!(
            "{host}: DNS sem resposta em {} ms",
            prazo.as_millis()
        )),
    }
}

/// Sonda TCP: o tempo do `connect` e o resultado, por endereco. Porta fechada nao e erro
/// da ferramenta -- e a resposta do diagnostico.
pub fn sondar_tcp(ends: &[SocketAddr], prazo: Duration) -> String {
    let mut t = Vec::new();
    for a in ends.iter().take(4) {
        let ini = Instant::now();
        let r = TcpStream::connect_timeout(a, prazo);
        let ms = ini.elapsed().as_secs_f64() * 1000.0;
        t.push(match r {
            Ok(_) => format!("{a}\taberta\t{ms:.2} ms"),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                format!("{a}\trecusada (porta fechada)\t{ms:.2} ms\t{e}")
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                format!("{a}\tsem resposta no prazo (filtrada?)\t{ms:.2} ms")
            }
            Err(e) => format!("{a}\terro\t{ms:.2} ms\t{e}"),
        });
    }
    t.join("\n")
}

impl Tool for NetworkTool {
    fn spec(&self) -> ToolSpec {
        if self.lan {
            return ToolSpec {
                name: "network_lan".into(),
                description: "Network probes to loopback and private-network destinations only. \
action=tcp_probe (host, port, timeout_ms) returns open/refused and the connect time; ping \
(host, count); traceroute (host)."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["tcp_probe","ping","traceroute"]},
                    "host":{"type":"string"},"port":{"type":"integer"},
                    "timeout_ms":{"type":"integer"},"count":{"type":"integer"}
                },"required":["action","host"]}),
            };
        }
        ToolSpec {
            name: "network".into(),
            description: "Network diagnosis of this machine. action=interfaces, routes, \
listening (open TCP/UDP ports with pid), resolve (host), tcp_probe (host, port, timeout_ms; \
public destinations allowed by the egress policy), ping (host, count), traceroute (host). \
Loopback/private destinations are the network_lan tool."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["interfaces","routes","listening","resolve","tcp_probe","ping","traceroute"]},
                "host":{"type":"string"},"port":{"type":"integer"},
                "timeout_ms":{"type":"integer"},"count":{"type":"integer"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        if self.lan { "net.lan" } else { "net.diagnose" }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = arg_str(&args, "action")?.to_string();
            let teto = ctx.timeout.min(Duration::from_secs(30));
            let lan_so_sondas =
                self.lan && !["tcp_probe", "ping", "traceroute"].contains(&acao.as_str());
            if lan_so_sondas {
                return Err(ToolError::InvalidArguments(format!(
                    "network_lan so faz tcp_probe, ping e traceroute; {acao} e da ferramenta network"
                )));
            }
            let texto = match acao.as_str() {
                "interfaces" => bloqueante(move || Ok(interfaces(teto))).await?,
                "routes" => bloqueante(move || Ok(rotas(teto))).await?,
                "listening" => bloqueante(|| Ok(portas_em_escuta())).await?,
                "resolve" => {
                    let host = validar_host(arg_str(&args, "host")?)?.to_string();
                    let prazo =
                        Duration::from_millis(arg_int(&args, "timeout_ms", 3000, 50, 10_000)?)
                            .min(teto);
                    bloqueante(move || {
                        let ini = Instant::now();
                        let v = resolver(&host, 0, prazo).map_err(ToolError::Failed)?;
                        let ms = ini.elapsed().as_secs_f64() * 1000.0;
                        let l: Vec<String> = v
                            .iter()
                            .map(|a| {
                                let classe = if phxclaw_browser::is_blocked_ip(a.ip()) {
                                    "interno"
                                } else {
                                    "publico"
                                };
                                format!("{}\t{classe}", a.ip())
                            })
                            .collect();
                        Ok(format!("{host} em {ms:.2} ms\n{}", l.join("\n")))
                    })
                    .await?
                }
                "tcp_probe" => {
                    let host = arg_str(&args, "host")?.to_string();
                    let porta = arg_int(&args, "port", 0, 1, 65_535)? as u16;
                    let prazo =
                        Duration::from_millis(arg_int(&args, "timeout_ms", 3000, 50, 10_000)?)
                            .min(teto);
                    let ends = {
                        let h = host.clone();
                        let eu = NetworkTool {
                            lan: self.lan,
                            egress: self.egress.clone(),
                        };
                        bloqueante(move || eu.liberar(&h, Some(porta), prazo)).await?
                    };
                    bloqueante(move || Ok(sondar_tcp(&ends, prazo))).await?
                }
                "ping" | "traceroute" => {
                    let host = arg_str(&args, "host")?.to_string();
                    let n = arg_int(&args, "count", 3, 1, 10)?;
                    let eu = NetworkTool {
                        lan: self.lan,
                        egress: self.egress.clone(),
                    };
                    bloqueante(move || {
                        let ends = eu.liberar(&host, None, Duration::from_secs(5))?;
                        // O IP ja decidido vai ao argv, nunca o nome: o comando nao resolve
                        // de novo e nao recebe texto do modelo.
                        let ip = ends[0].ip().to_string();
                        let c = n.to_string();
                        let prazo = teto.max(Duration::from_secs(5));
                        if acao == "ping" {
                            return Ok(
                                rodar(&["ping", "-n", "-c", &c, "-W", "2", &ip], prazo)?.tudo()
                            );
                        }
                        if binario("traceroute").is_some() {
                            Ok(rodar(
                                &["traceroute", "-n", "-q", "1", "-w", "2", "-m", "20", &ip],
                                prazo,
                            )?
                            .tudo())
                        } else if binario("tracepath").is_some() {
                            Ok(rodar(&["tracepath", "-n", &ip], prazo)?.tudo())
                        } else {
                            Err(ToolError::Failed(
                                "nem traceroute nem tracepath estao instalados aqui".into(),
                            ))
                        }
                    })
                    .await?
                }
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida: {outra}"
                    )));
                }
            };
            Ok(ToolOutput::text(truncate_for_model(&texto, TEXTO_MAX)))
        })
    }
}

/// Interfaces e enderecos: `ip -j addr` quando existe; sem ele, /sys e /proc lidos aqui,
/// porque imagem minima de servidor e de conteiner costuma vir sem o iproute2.
fn interfaces(prazo: Duration) -> String {
    if binario("ip").is_some()
        && let Ok(s) = rodar(&["ip", "-j", "addr"], prazo)
        && let Ok(Value::Array(v)) = serde_json::from_str::<Value>(&s.stdout)
    {
        let mut t = vec!["IFACE\tESTADO\tMAC\tMTU\tENDERECOS".to_string()];
        for i in v {
            let ends: Vec<String> = i["addr_info"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| {
                            format!("{}/{}", x["local"].as_str().unwrap_or("?"), x["prefixlen"])
                        })
                        .collect()
                })
                .unwrap_or_default();
            t.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                i["ifname"].as_str().unwrap_or("?"),
                i["operstate"].as_str().unwrap_or("?"),
                i["address"].as_str().unwrap_or(""),
                i["mtu"],
                ends.join(" ")
            ));
        }
        return t.join("\n") + "\n(fonte: ip -j addr)";
    }
    let rotas = rotas_v4_proc();
    let mut v4: Vec<(String, String)> = Vec::new();
    // fib_trie: a linha "/32 host LOCAL" logo abaixo de "|-- A.B.C.D" marca endereco local.
    let mut ultimo = String::new();
    for l in ler("/proc/net/fib_trie").lines() {
        let l = l.trim();
        if let Some(ip) = l.strip_prefix("|-- ") {
            ultimo = ip.to_string();
        } else if l.contains("/32 host LOCAL") && v4.iter().all(|(_, a)| *a != ultimo) {
            let Ok(ip) = ultimo.parse::<std::net::Ipv4Addr>() else {
                continue;
            };
            let iface = if ip.is_loopback() {
                "lo".to_string()
            } else {
                rotas
                    .iter()
                    .filter(|r| r.mascara != 0 && u32::from(ip) & r.mascara == r.destino)
                    .max_by_key(|r| r.mascara.count_ones())
                    .map(|r| format!("{}\t/{}", r.iface, r.mascara.count_ones()))
                    .unwrap_or_else(|| "?".into())
            };
            let (iface, pref) = iface.split_once('\t').unwrap_or((&iface, "/8"));
            v4.push((iface.to_string(), format!("{ultimo}{pref}")));
        }
    }
    let mut t = vec!["IFACE\tESTADO\tMAC\tMTU\tENDERECOS".to_string()];
    let mut nomes: Vec<String> = std::fs::read_dir("/sys/class/net")
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    nomes.sort();
    let v6 = ler("/proc/net/if_inet6");
    for n in nomes {
        let base = format!("/sys/class/net/{n}");
        let mut ends: Vec<String> = v4
            .iter()
            .filter(|(i, _)| *i == n)
            .map(|(_, a)| a.clone())
            .collect();
        for l in v6.lines() {
            let c: Vec<&str> = l.split_whitespace().collect();
            if c.len() == 6
                && c[5] == n
                && let Some(ip) = ipv6_hex_ordem_rede(c[0])
            {
                ends.push(format!(
                    "{ip}/{}",
                    u8::from_str_radix(c[2], 16).unwrap_or(0)
                ));
            }
        }
        t.push(format!(
            "{n}\t{}\t{}\t{}\t{}",
            ler(&format!("{base}/operstate")).trim(),
            ler(&format!("{base}/address")).trim(),
            ler(&format!("{base}/mtu")).trim(),
            ends.join(" ")
        ));
    }
    t.join("\n") + "\n(fonte: /sys/class/net e /proc/net, sem iproute2)"
}

struct RotaV4 {
    iface: String,
    destino: u32,
    gateway: u32,
    mascara: u32,
    metrica: u32,
}

/// /proc/net/route grava o endereco na ordem de bytes da maquina: os bytes nativos da
/// palavra sao os octetos na ordem da rede.
fn hex_v4(h: &str) -> Option<u32> {
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(u32::from_be_bytes(v.to_ne_bytes()))
}

fn rotas_v4_proc() -> Vec<RotaV4> {
    ler("/proc/net/route")
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split_whitespace().collect();
            (c.len() >= 8).then_some(())?;
            Some(RotaV4 {
                iface: c[0].to_string(),
                destino: hex_v4(c[1])?,
                gateway: hex_v4(c[2])?,
                metrica: c[6].parse().ok()?,
                mascara: hex_v4(c[7])?,
            })
        })
        .collect()
}

fn rotas(prazo: Duration) -> String {
    if binario("ip").is_some() {
        let mut t = Vec::new();
        for fam in ["-4", "-6"] {
            if let Ok(s) = rodar(&["ip", "-j", fam, "route"], prazo)
                && let Ok(Value::Array(v)) = serde_json::from_str::<Value>(&s.stdout)
            {
                for r in v {
                    t.push(format!(
                        "{}\tvia {}\tdev {}\tmetrica {}",
                        r["dst"].as_str().unwrap_or("?"),
                        r["gateway"].as_str().unwrap_or("-"),
                        r["dev"].as_str().unwrap_or("?"),
                        r.get("metric").cloned().unwrap_or(json!(0))
                    ));
                }
            }
        }
        if !t.is_empty() {
            return t.join("\n") + "\n(fonte: ip -j route)";
        }
    }
    let t: Vec<String> = rotas_v4_proc()
        .iter()
        .map(|r| {
            let dst = if r.mascara == 0 {
                "default".to_string()
            } else {
                format!(
                    "{}/{}",
                    std::net::Ipv4Addr::from(r.destino),
                    r.mascara.count_ones()
                )
            };
            let gw = if r.gateway == 0 {
                "-".to_string()
            } else {
                std::net::Ipv4Addr::from(r.gateway).to_string()
            };
            format!("{dst}\tvia {gw}\tdev {}\tmetrica {}", r.iface, r.metrica)
        })
        .collect();
    if t.is_empty() {
        return "(nenhuma rota IPv4 em /proc/net/route)".into();
    }
    t.join("\n") + "\n(fonte: /proc/net/route, sem iproute2)"
}

fn ipv6_hex_ordem_rede(h: &str) -> Option<std::net::Ipv6Addr> {
    if h.len() != 32 {
        return None;
    }
    let mut b = [0u8; 16];
    for (i, x) in b.iter_mut().enumerate() {
        *x = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(std::net::Ipv6Addr::from(b))
}

/// Endereco de /proc/net/tcp*: IPv4 numa palavra, IPv6 em quatro, cada palavra na ordem
/// da maquina.
fn endereco_proc(h: &str) -> Option<IpAddr> {
    match h.len() {
        8 => hex_v4(h).map(|v| IpAddr::V4(v.into())),
        32 => {
            let mut b = [0u8; 16];
            for i in 0..4 {
                let w = u32::from_str_radix(&h[i * 8..i * 8 + 8], 16).ok()?;
                b[i * 4..i * 4 + 4].copy_from_slice(&w.to_ne_bytes());
            }
            Some(IpAddr::V6(b.into()))
        }
        _ => None,
    }
}

/// Portas em escuta lidas do /proc, sem `ss`: TCP em LISTEN e UDP ligado, com o processo
/// dono achado pelo inode do soquete em /proc/*/fd. Processo de outro usuario aparece sem
/// PID quando o agente nao e root -- e a resposta honesta, nao um defeito.
pub fn portas_em_escuta() -> String {
    let mut socks: Vec<(String, IpAddr, u16, u64)> = Vec::new();
    for (arq, proto) in [
        ("/proc/net/tcp", "tcp"),
        ("/proc/net/tcp6", "tcp6"),
        ("/proc/net/udp", "udp"),
        ("/proc/net/udp6", "udp6"),
    ] {
        for l in ler(arq).lines().skip(1) {
            let c: Vec<&str> = l.split_whitespace().collect();
            if c.len() < 10 {
                continue;
            }
            // 0A = LISTEN; 07 = UDP sem par (o que «escutando» quer dizer em UDP).
            let estado_ok = if proto.starts_with("tcp") {
                c[3] == "0A"
            } else {
                c[3] == "07"
            };
            let Some((a, p)) = c[1].split_once(':') else {
                continue;
            };
            if let (true, Some(ip), Ok(porta), Ok(inode)) = (
                estado_ok,
                endereco_proc(a),
                u16::from_str_radix(p, 16),
                c[9].parse::<u64>(),
            ) {
                socks.push((proto.to_string(), ip, porta, inode));
            }
        }
    }
    let mut donos: std::collections::HashMap<u64, (u32, String)> = Default::default();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for e in rd.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(fds) = std::fs::read_dir(e.path().join("fd")) else {
                continue;
            };
            for fd in fds.flatten() {
                let Ok(alvo) = std::fs::read_link(fd.path()) else {
                    continue;
                };
                let alvo = alvo.to_string_lossy();
                if let Some(n) = alvo
                    .strip_prefix("socket:[")
                    .and_then(|r| r.strip_suffix(']'))
                    .and_then(|r| r.parse::<u64>().ok())
                {
                    donos.entry(n).or_insert_with(|| {
                        (pid, ler(&format!("/proc/{pid}/comm")).trim().to_string())
                    });
                }
            }
        }
    }
    socks.sort_by_key(|s| (s.0.clone(), s.2));
    let mut t = vec!["PROTO\tENDERECO\tPORTA\tPID\tPROCESSO".to_string()];
    for (proto, ip, porta, inode) in socks {
        let (pid, nome) = donos
            .get(&inode)
            .map(|(p, n)| (p.to_string(), n.clone()))
            .unwrap_or(("-".into(), "-".into()));
        t.push(format!("{proto}\t{ip}\t{porta}\t{pid}\t{nome}"));
    }
    t.join("\n")
}

// ---------------------------------------------------------------------------------------
// postgres
// ---------------------------------------------------------------------------------------

/// Variavel com a URL do banco. Vem do operador, nunca do modelo: um modelo que escolhe a
/// URL escolhe o servidor, e com ele para onde vai o que ele le.
pub const PG_URL_ENV: &str = "PHXCLAW_PG_URL";

/// PostgreSQL. `write = false` e a `postgres` (`db.read`): esquemas, tabelas, descricao,
/// SELECT e EXPLAIN, tudo dentro de `BEGIN READ ONLY` -- quem recusa a escrita e o
/// proprio banco, nao um filtro de palavra aqui. `write = true` e a `postgres_write`
/// (`db.write`): DDL/DML confirmados e EXPLAIN ANALYZE.
///
/// Sem `Debug` de proposito: a URL carrega a senha.
pub struct PostgresTool {
    pub write: bool,
    url: String,
    pub max_rows: usize,
}

impl PostgresTool {
    pub fn new(write: bool, url: impl Into<String>) -> Self {
        Self {
            write,
            url: url.into(),
            max_rows: 1000,
        }
    }
    /// So existe se o operador configurou a URL, como o e-mail com o SMTP.
    pub fn do_ambiente(write: bool) -> Option<Self> {
        let u = std::env::var(PG_URL_ENV).ok()?;
        (!u.trim().is_empty()).then(|| Self::new(write, u))
    }
}

/// Tira a senha de qualquer texto que va ao modelo ou ao log. O erro do driver nao a
/// inclui hoje; a redacao existe para o dia em que alguma mensagem incluir.
fn redigir(texto: &str, senha: Option<&str>) -> String {
    match senha {
        Some(s) if !s.is_empty() => texto.replace(s, "***"),
        _ => texto.to_string(),
    }
}

fn erro_pg(e: &postgres::Error, senha: Option<&str>) -> ToolError {
    let msg = match e.as_db_error() {
        Some(db) => {
            let mut m = format!(
                "o banco recusou: {}: {} (SQLSTATE {})",
                db.severity(),
                db.message(),
                db.code().code()
            );
            if let Some(d) = db.detail() {
                m.push_str(&format!("\ndetalhe: {d}"));
            }
            if let Some(h) = db.hint() {
                m.push_str(&format!("\ndica: {h}"));
            }
            m
        }
        None => format!("PostgreSQL: {e}"),
    };
    ToolError::Failed(redigir(&msg, senha))
}

/// Celula de qualquer tipo, como texto. O driver so fala o formato binario; aceitar todo
/// tipo e decodificar aqui e o que deixa um SELECT qualquer voltar sem conhecer as colunas
/// antes -- e um tipo desconhecido vira «<tipo, N bytes>» em vez de derrubar a consulta.
struct Celula(String);

impl<'a> postgres::types::FromSql<'a> for Celula {
    fn from_sql(
        ty: &postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Celula(decodificar(ty, raw)))
    }
    fn from_sql_null(
        _: &postgres::types::Type,
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Celula("NULL".into()))
    }
    fn accepts(_: &postgres::types::Type) -> bool {
        true
    }
}

fn decodificar(ty: &postgres::types::Type, raw: &[u8]) -> String {
    use postgres::types::{FromSql, Kind, Type};
    fn d<'a, T: FromSql<'a> + ToString>(ty: &Type, raw: &'a [u8]) -> Option<String> {
        T::from_sql(ty, raw).ok().map(|v| v.to_string())
    }
    let r = match *ty {
        Type::BOOL => d::<bool>(ty, raw),
        Type::INT2 => d::<i16>(ty, raw),
        Type::INT4 => d::<i32>(ty, raw),
        Type::INT8 => d::<i64>(ty, raw),
        Type::OID => d::<u32>(ty, raw),
        Type::FLOAT4 => d::<f32>(ty, raw),
        Type::FLOAT8 => d::<f64>(ty, raw),
        Type::NUMERIC => numeric(raw),
        Type::JSON | Type::JSONB => d::<serde_json::Value>(ty, raw),
        Type::DATE => d::<chrono::NaiveDate>(ty, raw),
        Type::TIME => d::<chrono::NaiveTime>(ty, raw),
        Type::TIMESTAMP => d::<chrono::NaiveDateTime>(ty, raw),
        Type::TIMESTAMPTZ => d::<chrono::DateTime<chrono::Utc>>(ty, raw),
        Type::UUID if raw.len() == 16 => {
            let h: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            Some(format!(
                "{}-{}-{}-{}-{}",
                &h[0..8],
                &h[8..12],
                &h[12..16],
                &h[16..20],
                &h[20..32]
            ))
        }
        Type::BYTEA => {
            let h: String = raw.iter().take(64).map(|b| format!("{b:02x}")).collect();
            Some(format!("\\x{h}{}", if raw.len() > 64 { "..." } else { "" }))
        }
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN | Type::CHAR => {
            Some(String::from_utf8_lossy(raw).into_owned())
        }
        _ if matches!(ty.kind(), Kind::Enum(_)) => Some(String::from_utf8_lossy(raw).into_owned()),
        _ => None,
    };
    r.unwrap_or_else(|| format!("<{}, {} bytes>", ty.name(), raw.len()))
}

/// NUMERIC no formato binario: digitos na base 10000, peso do primeiro, sinal e escala.
/// O driver nao o decodifica sem uma crate de decimal, e SUM de inteiro devolve NUMERIC.
fn numeric(raw: &[u8]) -> Option<String> {
    let rd =
        |i: usize| -> Option<i16> { Some(i16::from_be_bytes([*raw.get(i)?, *raw.get(i + 1)?])) };
    let n = rd(0)? as usize;
    let peso = rd(2)? as i32;
    let sinal = rd(4)? as u16;
    let escala = rd(6)? as usize;
    match sinal {
        0xC000 => return Some("NaN".into()),
        0xD000 => return Some("Infinity".into()),
        0xF000 => return Some("-Infinity".into()),
        _ => {}
    }
    let dig: Vec<i16> = (0..n).map(|i| rd(8 + 2 * i)).collect::<Option<_>>()?;
    let em = |i: i32| -> i16 {
        if i < 0 {
            0
        } else {
            dig.get(i as usize).copied().unwrap_or(0)
        }
    };
    let mut s = String::new();
    if peso < 0 {
        s.push('0');
    } else {
        for i in 0..=peso {
            if i == 0 {
                s.push_str(&em(i).to_string());
            } else {
                s.push_str(&format!("{:04}", em(i)));
            }
        }
    }
    if escala > 0 {
        let mut f = String::new();
        let mut i = peso + 1;
        while f.len() < escala {
            f.push_str(&format!("{:04}", em(i)));
            i += 1;
        }
        f.truncate(escala);
        s.push('.');
        s.push_str(&f);
    }
    if sinal == 0x4000 {
        s.insert(0, '-');
    }
    Some(s)
}

fn tabela_de_linhas(colunas: &[postgres::Column], linhas: &[postgres::Row], teto: usize) -> String {
    if colunas.is_empty() {
        return "(comando sem colunas de resultado)".into();
    }
    let mut t = colunas
        .iter()
        .map(|c| c.name().to_string())
        .collect::<Vec<_>>()
        .join("\t");
    for l in linhas.iter().take(teto) {
        let cel: Vec<String> = (0..colunas.len())
            .map(|i| {
                let v = l
                    .try_get::<_, Celula>(i)
                    .map(|c| c.0)
                    .unwrap_or_else(|e| format!("<erro: {e}>"));
                let v: String = v.replace(['\t', '\n'], " ").chars().take(300).collect();
                v
            })
            .collect();
        t.push('\n');
        t.push_str(&cel.join("\t"));
    }
    let mostradas = linhas.len().min(teto);
    if linhas.len() > teto {
        t.push_str(&format!(
            "\n({mostradas} linhas; teto {teto} atingido, ha mais)"
        ));
    } else {
        t.push_str(&format!("\n({mostradas} linhas)"));
    }
    t
}

impl Tool for PostgresTool {
    fn spec(&self) -> ToolSpec {
        if self.write {
            return ToolSpec {
                name: "postgres_write".into(),
                description: "Change the configured PostgreSQL database. action=execute (sql: one \
DDL/DML statement, committed; RETURNING rows are shown), explain_analyze (sql: runs it with \
EXPLAIN ANALYZE inside a transaction that is rolled back)."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["execute","explain_analyze"]},
                    "sql":{"type":"string"},"max_rows":{"type":"integer"}
                },"required":["action","sql"]}),
            };
        }
        ToolSpec {
            name: "postgres".into(),
            description: "Read the configured PostgreSQL database (read-only transaction). \
action=schemas, tables (schema optional), describe (table, schema default public: columns, \
constraints, indexes), query (sql: one SELECT, max_rows), explain (sql, plan only)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["schemas","tables","describe","query","explain"]},
                "schema":{"type":"string"},"table":{"type":"string"},
                "sql":{"type":"string"},"max_rows":{"type":"integer"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        if self.write { "db.write" } else { "db.read" }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        let url = self.url.clone();
        let write = self.write;
        let teto_max = self.max_rows;
        // O banco corta antes do motor: o corte do motor so larga o futuro, e a consulta
        // continuaria rodando no servidor.
        let prazo = ctx
            .timeout
            .saturating_sub(Duration::from_secs(1))
            .clamp(Duration::from_secs(1), Duration::from_secs(120));
        Box::pin(async move {
            let texto = bloqueante(move || pg_rodar(&url, write, &args, prazo, teto_max)).await?;
            Ok(ToolOutput::text(truncate_for_model(&texto, TEXTO_MAX)))
        })
    }
}

/// Uma instrucao so por chamada, pelo protocolo estendido: o servidor recusa varias num
/// `prepare`. Com o protocolo simples, um «SELECT 1; COMMIT; INSERT ...» sairia da
/// transacao READ ONLY e o INSERT rodaria em autocommit. O portal busca so `teto + 1`
/// linhas: o teto nao traz a tabela inteira para depois cortar.
fn consultar(
    tx: &mut postgres::Transaction<'_>,
    sql: &str,
    teto: usize,
    senha: Option<&str>,
) -> Result<String, ToolError> {
    let pe = |e: postgres::Error| erro_pg(&e, senha);
    let st = tx.prepare(sql).map_err(pe)?;
    let portal = tx.bind(&st, &[]).map_err(pe)?;
    let linhas = tx.query_portal(&portal, (teto + 1) as i32).map_err(pe)?;
    Ok(tabela_de_linhas(st.columns(), &linhas, teto))
}

fn pg_rodar(
    url: &str,
    write: bool,
    args: &Value,
    prazo: Duration,
    teto_max: usize,
) -> Result<String, ToolError> {
    let acao = arg_str(args, "action")?;
    let teto = arg_int(args, "max_rows", 100, 1, teto_max as u64)? as usize;
    let mut cfg: postgres::Config = url.parse().map_err(|_| {
        // O erro de parse pode citar o pedaco da URL que falhou -- e o pedaco pode ser a
        // senha. Diz so que esta mal formada.
        ToolError::Failed(format!(
            "{PG_URL_ENV} mal formada (postgresql://usuario@host:porta/banco ou host=... port=...)"
        ))
    })?;
    let senha = cfg
        .get_password()
        .map(|p| String::from_utf8_lossy(p).into_owned());
    let senha = senha.as_deref();
    cfg.connect_timeout(prazo.min(Duration::from_secs(10)));
    cfg.application_name("phxclaw-agent");
    let mut cli = cfg.connect(postgres::NoTls).map_err(|e| {
        ToolError::Failed(redigir(
            &format!("conexao ao PostgreSQL falhou: {e}"),
            senha,
        ))
    })?;
    let pe = |e: postgres::Error| erro_pg(&e, senha);
    let mut tx = cli
        .build_transaction()
        .read_only(!write)
        .start()
        .map_err(pe)?;
    tx.batch_execute(&format!(
        "SET LOCAL statement_timeout = {}",
        prazo.as_millis()
    ))
    .map_err(pe)?;
    let ler_sql = |tx: &mut postgres::Transaction<'_>, sql: &str| consultar(tx, sql, teto, senha);
    let texto = match (write, acao) {
        (false, "schemas") => ler_sql(
            &mut tx,
            "SELECT nspname::text AS esquema, pg_get_userbyid(nspowner)::text AS dono \
             FROM pg_namespace WHERE nspname !~ '^pg_' AND nspname <> 'information_schema' \
             ORDER BY 1",
        )?,
        (false, "tables") => {
            let esquema = arg_opt(args, "schema");
            let st = tx
                .prepare(
                    "SELECT n.nspname::text AS esquema, c.relname::text AS tabela, \
                     CASE c.relkind WHEN 'r' THEN 'tabela' WHEN 'v' THEN 'visao' \
                     WHEN 'm' THEN 'visao materializada' WHEN 'p' THEN 'particionada' \
                     WHEN 'f' THEN 'estrangeira' END AS tipo, \
                     c.reltuples::bigint AS linhas_estimadas \
                     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE c.relkind IN ('r','v','m','p','f') \
                     AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' \
                     AND ($1::text IS NULL OR n.nspname = $1::text) ORDER BY 1, 2",
                )
                .map_err(pe)?;
            let linhas = tx.query(&st, &[&esquema]).map_err(pe)?;
            tabela_de_linhas(st.columns(), &linhas, teto_max)
        }
        (false, "describe") => {
            let tabela = arg_str(args, "table")?;
            let esquema = arg_opt(args, "schema").unwrap_or("public");
            // Nome entra como parametro e o banco o cita (`%I`): nada do modelo vira SQL.
            let oid: Option<u32> = tx
                .query_one(
                    "SELECT to_regclass(format('%I.%I', $1::text, $2::text))::oid",
                    &[&esquema, &tabela],
                )
                .map_err(pe)?
                .get(0);
            let Some(oid) = oid else {
                return Err(ToolError::InvalidArguments(format!(
                    "tabela {esquema}.{tabela} nao existe"
                )));
            };
            let mut t = format!("== {esquema}.{tabela} ==\n");
            for (titulo, sql) in [
                (
                    "colunas",
                    "SELECT a.attname::text AS coluna, format_type(a.atttypid, a.atttypmod) AS tipo, \
                     (NOT a.attnotnull) AS anulavel, pg_get_expr(d.adbin, d.adrelid) AS padrao \
                     FROM pg_attribute a LEFT JOIN pg_attrdef d \
                     ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
                     WHERE a.attrelid = $1 AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum",
                ),
                (
                    "restricoes",
                    "SELECT conname::text AS nome, pg_get_constraintdef(oid) AS definicao \
                     FROM pg_constraint WHERE conrelid = $1 ORDER BY 1",
                ),
                (
                    "indices",
                    "SELECT pg_get_indexdef(indexrelid) AS indice FROM pg_index \
                     WHERE indrelid = $1 ORDER BY 1",
                ),
            ] {
                let st = tx.prepare(sql).map_err(pe)?;
                let linhas = tx.query(&st, &[&oid]).map_err(pe)?;
                t.push_str(&format!(
                    "-- {titulo}\n{}\n",
                    tabela_de_linhas(st.columns(), &linhas, teto_max)
                ));
            }
            t
        }
        (false, "query") => ler_sql(&mut tx, arg_str(args, "sql")?)?,
        // ANALYZE desligado por escrito: «EXPLAIN (ANALYZE false) ANALYZE ...» e erro de
        // sintaxe, entao o modelo nao executa a consulta pela ferramenta de leitura.
        (false, "explain") => ler_sql(
            &mut tx,
            &format!("EXPLAIN (ANALYZE false) {}", arg_str(args, "sql")?),
        )?,
        (true, "execute") => {
            let sql = arg_str(args, "sql")?;
            let st = tx.prepare(sql).map_err(pe)?;
            let t = if st.columns().is_empty() {
                let n = tx.execute(&st, &[]).map_err(pe)?;
                format!("{n} linhas afetadas")
            } else {
                let portal = tx.bind(&st, &[]).map_err(pe)?;
                let linhas = tx.query_portal(&portal, (teto + 1) as i32).map_err(pe)?;
                tabela_de_linhas(st.columns(), &linhas, teto)
            };
            tx.commit().map_err(pe)?;
            return Ok(format!("{t}\nconfirmado (COMMIT)"));
        }
        (true, "explain_analyze") => {
            let t = ler_sql(
                &mut tx,
                &format!("EXPLAIN (ANALYZE, BUFFERS) {}", arg_str(args, "sql")?),
            )?;
            tx.rollback().map_err(pe)?;
            return Ok(format!("{t}\n(executado e desfeito: ROLLBACK)"));
        }
        (_, outra) => {
            return Err(ToolError::InvalidArguments(format!(
                "action desconhecida nesta ferramenta: {outra}"
            )));
        }
    };
    tx.rollback().map_err(pe)?;
    Ok(redigir(&texto, senha))
}

// ---------------------------------------------------------------------------------------
// rust_project
// ---------------------------------------------------------------------------------------

/// cargo check/build/test/clippy/fmt num projeto da pasta da tarefa, no MESMO bwrap do
/// `shell` (a mesma funcao de montagem, com o toolchain acrescentado so leitura), sem
/// rede e `--offline`. Devolve diagnosticos estruturados, do `--message-format=json`.
pub struct RustProjectTool {
    pub bwrap: PathBuf,
    /// Raiz do toolchain no hospedeiro (`rustc --print sysroot`): cargo, rustc, clippy e
    /// rustfmt moram em `bin/` dela e entram no sandbox pelo mesmo caminho.
    pub sysroot: PathBuf,
    /// Registro de crates do hospedeiro, montado so leitura: dependencia ja baixada
    /// compila sem rede.
    pub registry: Option<PathBuf>,
    pub timeout: Duration,
}

impl RustProjectTool {
    /// Acha o toolchain do hospedeiro. Sem ele a ferramenta nem se registra.
    pub fn detectar(bwrap: PathBuf) -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".cargo")));
        let rustc = binario("rustc").or_else(|| {
            cargo_home
                .as_ref()
                .map(|c| c.join("bin/rustc"))
                .filter(|p| p.is_file())
        })?;
        let s = Command::new(rustc)
            .args(["--print", "sysroot"])
            .output()
            .ok()?;
        let sysroot = PathBuf::from(String::from_utf8_lossy(&s.stdout).trim());
        if !s.status.success() || !sysroot.join("bin/cargo").is_file() {
            return None;
        }
        Some(Self {
            bwrap,
            sysroot,
            registry: cargo_home
                .map(|c| c.join("registry"))
                .filter(|p| p.is_dir()),
            timeout: Duration::from_secs(600),
        })
    }

    fn extras(&self) -> SandboxExtras {
        let raiz = self.sysroot.display().to_string();
        let mut e = SandboxExtras {
            ro_binds: vec![(self.sysroot.clone(), raiz.clone())],
            env: vec![
                (
                    "PATH".into(),
                    format!("{raiz}/bin:/usr/local/bin:/usr/bin:/bin"),
                ),
                // CARGO_HOME no tmpfs: o cargo grava trava e cache ali, e o do hospedeiro
                // esta so leitura (ou nem esta montado).
                ("CARGO_HOME".into(), "/tmp/cargo-home".into()),
                ("CARGO_TERM_COLOR".into(), "never".into()),
            ],
        };
        if let Some(r) = &self.registry {
            e.ro_binds
                .push((r.clone(), "/tmp/cargo-home/registry".into()));
        }
        e
    }
}

/// Um diagnostico do compilador, ja sem o texto renderizado de terminal.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diagnostico {
    pub arquivo: String,
    pub linha: u64,
    pub coluna: u64,
    pub nivel: String,
    pub mensagem: String,
    pub codigo: Option<String>,
}

/// Le a saida `--message-format=json` do cargo (uma mensagem JSON por linha). Linha que
/// nao e JSON e saida de teste ou de build script, e volta a parte.
pub fn diagnosticos_do_cargo(stdout: &str) -> (Vec<Diagnostico>, Option<bool>, Vec<String>) {
    let mut diags = Vec::new();
    let mut sucesso = None;
    let mut texto = Vec::new();
    for l in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            if !l.trim().is_empty() {
                texto.push(l.to_string());
            }
            continue;
        };
        match v["reason"].as_str() {
            Some("compiler-message") => {
                let m = &v["message"];
                let span = m["spans"]
                    .as_array()
                    .and_then(|s| s.iter().find(|s| s["is_primary"] == true));
                let d = Diagnostico {
                    arquivo: span
                        .and_then(|s| s["file_name"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    linha: span.and_then(|s| s["line_start"].as_u64()).unwrap_or(0),
                    coluna: span.and_then(|s| s["column_start"].as_u64()).unwrap_or(0),
                    nivel: m["level"].as_str().unwrap_or("?").to_string(),
                    mensagem: m["message"].as_str().unwrap_or("").to_string(),
                    codigo: m["code"]["code"].as_str().map(str::to_string),
                };
                if !diags.contains(&d) {
                    diags.push(d);
                }
            }
            Some("build-finished") => sucesso = v["success"].as_bool(),
            _ => {}
        }
    }
    (diags, sucesso, texto)
}

/// `cargo fmt --check --message-format json`: um vetor de arquivos com os trechos fora do
/// formato. Cada trecho vira um diagnostico de nivel «fmt».
fn diagnosticos_do_fmt(stdout: &str, raiz: &str) -> Vec<Diagnostico> {
    let mut v = Vec::new();
    for l in stdout.lines() {
        let Ok(Value::Array(arqs)) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        for a in arqs {
            let nome = a["name"].as_str().unwrap_or("");
            let nome = nome
                .strip_prefix(raiz)
                .map(|n| n.trim_start_matches('/'))
                .unwrap_or(nome);
            for m in a["mismatches"].as_array().into_iter().flatten() {
                v.push(Diagnostico {
                    arquivo: nome.to_string(),
                    linha: m["original_begin_line"].as_u64().unwrap_or(0),
                    coluna: 0,
                    nivel: "fmt".into(),
                    mensagem: format!(
                        "fora do formato; esperado:\n{}",
                        m["expected"].as_str().unwrap_or("")
                    ),
                    codigo: None,
                });
            }
        }
    }
    v
}

/// Caminho do projeto: relativo a pasta da tarefa, so caracteres que nao precisam de
/// aspas -- e ele que entra no `sh -c` do sandbox.
pub(crate) fn caminho_do_projeto(workdir: &Path, rel: &str) -> Result<String, ToolError> {
    let rel = rel.trim();
    let rel = rel
        .strip_prefix("/work")
        .unwrap_or(rel)
        .trim_start_matches('/')
        .trim_start_matches("./");
    let rel = rel.trim_end_matches('/');
    if rel.is_empty() || rel == "." {
        return Ok(String::new());
    }
    if !rel
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
    {
        return Err(ToolError::InvalidArguments(format!(
            "caminho invalido: {rel:?} (letras, digitos, . _ - /)"
        )));
    }
    confine(workdir, rel).map_err(ToolError::Denied)?;
    Ok(rel.to_string())
}

impl Tool for RustProjectTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "rust_project".into(),
            description: "Run cargo on a Rust project inside the task directory (isolated \
sandbox, offline). action=check|build|test|clippy|fmt (fmt only checks unless fix=true); path \
is the project directory relative to the task directory (default '.'). Returns structured \
diagnostics: file, line, column, level, message, code."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["check","build","test","clippy","fmt"]},
                "path":{"type":"string"},
                "fix":{"type":"boolean"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = arg_str(&args, "action")?.to_string();
            let rel = caminho_do_projeto(&ctx.workdir, arg_opt(&args, "path").unwrap_or("."))?;
            let dir = if rel.is_empty() {
                ctx.workdir.clone()
            } else {
                ctx.workdir.join(&rel)
            };
            if !dir.join("Cargo.toml").is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "nao ha Cargo.toml em {}",
                    if rel.is_empty() { "." } else { &rel }
                )));
            }
            let fix = args.get("fix").and_then(Value::as_bool).unwrap_or(false);
            let cargo = match (acao.as_str(), fix) {
                ("check", _) => "cargo check --offline --message-format=json",
                ("build", _) => "cargo build --offline --message-format=json",
                ("test", _) => "cargo test --offline --message-format=json",
                ("clippy", _) => "cargo clippy --offline --all-targets --message-format=json",
                ("fmt", false) => "cargo fmt --check --message-format json",
                ("fmt", true) => "cargo fmt",
                (outra, _) => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida: {outra} (check, build, test, clippy, fmt)"
                    )));
                }
            };
            let guest = if rel.is_empty() {
                "/work".to_string()
            } else {
                format!("/work/{rel}")
            };
            // Os dois pedacos variaveis ja foram validados: a acao saiu de uma lista e o
            // caminho so tem caracteres que nao precisam de aspas.
            let cmd = WorkdirCommand {
                workdir: ctx.workdir.clone(),
                script: format!("cd '{guest}' && exec {cargo}"),
                timeout: self.timeout.min(ctx.timeout),
                network: false,
                max_output_bytes: 4 * 1024 * 1024,
            };
            let (bwrap, extras) = (self.bwrap.clone(), self.extras());
            let r = tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let (mut diags, sucesso, texto) = diagnosticos_do_cargo(&r.stdout);
            if acao == "fmt" && !fix {
                diags = diagnosticos_do_fmt(&r.stdout, &guest);
            }
            let erros = diags.iter().filter(|d| d.nivel == "error").count();
            let avisos = diags.iter().filter(|d| d.nivel == "warning").count();
            let testes: Vec<&String> = texto
                .iter()
                .filter(|l| {
                    l.starts_with("test result:") || l.contains("FAILED") || l.contains("panicked")
                })
                .collect();
            let stderr: String = {
                let c: Vec<char> = r.stderr.chars().collect();
                c[c.len().saturating_sub(3000)..].iter().collect()
            };
            let total = diags.len();
            diags.truncate(100);
            let saida = json!({
                "acao": acao,
                "projeto": if rel.is_empty() { "." } else { &rel },
                "exit_code": r.exit_code,
                "sucesso": r.exit_code == Some(0) && sucesso != Some(false),
                "erros": erros,
                "avisos": avisos,
                "diagnosticos_total": total,
                "diagnosticos": diags,
                "testes": testes,
                "stderr_cauda": if r.exit_code == Some(0) { String::new() } else { stderr },
                "saida_truncada": r.truncated,
            });
            Ok(ToolOutput::text(
                serde_json::to_string_pretty(&saida).unwrap_or_default(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unidade_recusa_shell_e_opcao() {
        assert!(validar_unidade("nginx.service").is_ok());
        assert!(validar_unidade("getty@tty1.service").is_ok());
        for ruim in ["nginx; rm -rf /", "-H", "a b", "$(id)", "", "x|y"] {
            assert!(
                matches!(validar_unidade(ruim), Err(ToolError::InvalidArguments(_))),
                "{ruim:?} passou"
            );
        }
    }

    #[test]
    fn prioridade_e_desde_validados() {
        assert!(validar_prioridade("err").is_ok());
        assert!(validar_prioridade("0..4").is_ok());
        assert!(validar_prioridade("err;id").is_err());
        assert!(validar_desde("-1h").is_ok());
        assert!(validar_desde("2026-10-01 08:00:00").is_ok());
        assert!(validar_desde("$(id)").is_err());
    }

    #[test]
    fn pid_recusa_init_texto_e_o_proprio() {
        assert!(matches!(
            validar_pid(Some(&json!(1))),
            Err(ToolError::Denied(_))
        ));
        assert!(matches!(
            validar_pid(Some(&json!("12; reboot"))),
            Err(ToolError::InvalidArguments(_))
        ));
        assert!(matches!(
            validar_pid(Some(&json!(std::process::id()))),
            Err(ToolError::Denied(_))
        ));
    }

    #[test]
    fn numeric_binario_decodifica() {
        // 123.45: ndigits 2, peso 0, sinal +, escala 2, digitos [123, 4500]
        let b = |n: i16, peso: i16, sinal: u16, esc: i16, d: &[i16]| {
            let mut v = Vec::new();
            for x in [n, peso, sinal as i16, esc] {
                v.extend_from_slice(&x.to_be_bytes());
            }
            for x in d {
                v.extend_from_slice(&x.to_be_bytes());
            }
            v
        };
        assert_eq!(numeric(&b(2, 0, 0, 2, &[123, 4500])).unwrap(), "123.45");
        assert_eq!(numeric(&b(1, -1, 0x4000, 3, &[10])).unwrap(), "-0.001");
        assert_eq!(numeric(&b(1, 1, 0, 0, &[1])).unwrap(), "10000");
        assert_eq!(numeric(&b(0, 0, 0, 0, &[])).unwrap(), "0");
        assert_eq!(numeric(&b(2, 1, 0, 0, &[12, 5])).unwrap(), "120005");
    }

    #[test]
    fn endereco_do_proc_na_ordem_da_rede() {
        assert_eq!(
            endereco_proc("0100007F"),
            Some("127.0.0.1".parse().unwrap())
        );
        assert_eq!(
            endereco_proc("00000000000000000000000001000000"),
            Some("::1".parse().unwrap())
        );
    }

    #[test]
    fn origem_tcp_bate_com_a_do_broker() {
        let mut p = EgressPolicy {
            enabled: true,
            ..EgressPolicy::default()
        };
        p.allowed_origins.insert(origem_tcp("Example.COM", 5432));
        p.allowed_origins.insert(origem_tcp("example.org", 443));
        let b = EgressBroker::new(p);
        assert!(b.validate_url(&origem_tcp("example.com", 5432)).is_ok());
        assert!(b.validate_url(&origem_tcp("example.org", 443)).is_ok());
        assert!(b.validate_url(&origem_tcp("example.com", 22)).is_err());
    }

    #[test]
    fn senha_some_do_texto() {
        assert_eq!(
            redigir("falhou para segredo123 em x", Some("segredo123")),
            "falhou para *** em x"
        );
    }
}

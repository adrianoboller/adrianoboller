#![forbid(unsafe_code)]

use anyhow::{Context, Result, bail};
use phxclaw_agent::api::{AgentFactory, ApiState, disparar_agenda, router};
use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::{Agenda, CancelFlag, Observer, Task, TaskStatus, TaskStore};
use phxclaw_core_runtime::{
    CoreService, PRODUCT_CLI, PRODUCT_NAME, PhoenixCoreRuntime, ServiceState,
};
use phxclaw_postgres_bootstrap::{HostPlatform, PostgreSqlBootstrapConfig, build_install_plan};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MODELO_PADRAO: &str = "ollama:qwen2.5:1.5b";

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(());
    }
    match args[0].as_str() {
        "version" | "--version" | "-V" => println!("{PRODUCT_NAME} {VERSION}"),
        "core" => core_command(&args[1..])?,
        "db" => db_command(&args[1..])?,
        "agente" | "agent" => runtime()?.block_on(agente(&args[1..]))?,
        "servir" | "serve" => runtime()?.block_on(servir(&args[1..]))?,
        "canal" | "channel" => runtime()?.block_on(canal(&args[1..]))?,
        "mcp-serve" => runtime()?.block_on(mcp_serve(&args[1..]))?,
        "dispositivos" | "devices" => runtime()?.block_on(dispositivos(&args[1..]))?,
        other => bail!("unknown command: {other}. Run `{PRODUCT_CLI} --help`."),
    }
    Ok(())
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

/// Valor de `--nome valor`, e os argumentos que sobram.
fn opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn pasta(args: &[String]) -> PathBuf {
    opcao(args, "--pasta")
        .map(PathBuf::from)
        .or_else(|| env::var("PHXCLAW_HOME").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("var/agente"))
}

/// Mostra cada passo novo assim que o motor grava.
struct Terminal(Mutex<usize>);
impl Observer for Terminal {
    fn on_update(&self, t: &Task) {
        let mut visto = self.0.lock().unwrap();
        for p in &t.steps[*visto..] {
            match &p.tool {
                Some(nome) => {
                    let args = p.arguments.to_string();
                    let args: String = args.chars().take(140).collect();
                    println!("  [{}] {nome} {args}  -> {}", p.n, p.outcome);
                    let resumo: String = p.summary.lines().take(3).collect::<Vec<_>>().join(" | ");
                    println!("        {}", resumo.chars().take(160).collect::<String>());
                }
                None => println!(
                    "  [{}] pensando: {}",
                    p.n,
                    p.summary.chars().take(160).collect::<String>()
                ),
            }
        }
        *visto = t.steps.len();
    }
}

async fn agente(args: &[String]) -> Result<()> {
    let objetivo = args
        .iter()
        .find(|a| !a.starts_with("--") && Some(*a) != opcao(args, "--modelo").as_ref() && Some(*a) != opcao(args, "--pasta").as_ref())
        .context("uso: phxclaw agente \"objetivo\" [--modelo ollama:qwen2.5:1.5b] [--plano] [--pasta DIR]")?;
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let m = Montagem::new(store.clone());
    let agente = m.agent(&modelo).map_err(anyhow::Error::msg)?;
    let mut t = Task::new(objetivo.clone(), modelo.clone());
    println!(
        "tarefa {}\nmodelo {modelo}\nferramentas: {}",
        t.id,
        agente
            .tools
            .iter()
            .filter(|x| agente.config.capabilities.contains(x.capability()))
            .map(|x| x.spec().name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if args.iter().any(|a| a == "--plano") {
        agente.plan(&mut t).await.map_err(anyhow::Error::msg)?;
        println!("plano:");
        for (i, p) in t.plan.iter().enumerate() {
            println!("  {}. {p}", i + 1);
        }
    }
    println!("executando...");
    let fim = agente
        .run(t, &CancelFlag::default(), &Terminal(Mutex::new(0)))
        .await;
    println!("\nestado: {:?}", fim.status);
    if let Some(r) = &fim.answer {
        println!("resposta:\n{r}");
    }
    if let Some(e) = &fim.error {
        println!("erro: {e}");
    }
    for a in &fim.artifacts {
        println!(
            "artefato: {} ({} bytes, sha256 {}...)",
            store.workdir(&fim.id).join(&a.path).display(),
            a.bytes,
            &a.sha256[..16]
        );
    }
    println!(
        "tokens: {} entrada, {} saida",
        fim.usage.input_tokens, fim.usage.output_tokens
    );
    if fim.status != TaskStatus::Completed {
        std::process::exit(2);
    }
    Ok(())
}

async fn servir(args: &[String]) -> Result<()> {
    let porta: u16 = opcao(args, "--porta")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(8787);
    let raiz = pasta(args);
    let store = TaskStore::new(raiz.join("tasks"))?;
    // Token: variavel de ambiente ou arquivo 0600 gerado na primeira vez.
    let token = match env::var("PHXCLAW_API_TOKEN") {
        Ok(t) if t.len() >= 24 => t,
        Ok(_) => bail!("PHXCLAW_API_TOKEN precisa de pelo menos 24 caracteres"),
        Err(_) => {
            let arq = raiz.join("api.token");
            match std::fs::read_to_string(&arq) {
                Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
                _ => {
                    let t = phxclaw_api_gateway::generate_bearer_token();
                    phxclaw_secret_broker::write_private_file(&arq, t.as_bytes())?;
                    t
                }
            }
        }
    };
    let m = Montagem::new(store.clone());
    let factory: AgentFactory = Arc::new(move |modelo: &str| m.agent(modelo));
    let state = estado_api(&raiz, store, factory, token)?;
    let agenda_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let n = disparar_agenda(&agenda_state);
            if n > 0 {
                println!("agenda: {n} tarefa(s) disparada(s)");
            }
        }
    });
    // Loopback por padrao: expor a API na rede e decisao explicita do operador.
    let host = env::var("PHXCLAW_API_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "API de tarefas em http://{}  (token em {})",
        l.local_addr()?,
        raiz.join("api.token").display()
    );
    axum::serve(l, router(state)).await?;
    Ok(())
}

/// Ferramentas do agente como servidor MCP por stdio. A MESMA montagem do `agente` e do
/// `servir`, com o mesmo `PHXCLAW_CAPACIDADES`: uma lista paralela aqui seria uma segunda
/// politica para o mesmo produto. O stdout e o fio do protocolo; nada mais se escreve nele.
async fn mcp_serve(args: &[String]) -> Result<()> {
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let trabalho = match opcao(args, "--trabalho") {
        Some(d) => PathBuf::from(d),
        None => env::current_dir()?,
    };
    let modelo = opcao(args, "--modelo")
        .or_else(|| env::var("PHXCLAW_MODELO").ok())
        .unwrap_or_else(|| MODELO_PADRAO.into());
    let agente = Montagem::new(store)
        .agent(&modelo)
        .map_err(anyhow::Error::msg)?;
    let entrada = tokio::io::BufReader::new(tokio::io::stdin());
    phxclaw_agent::mcp::servir(&agente, trabalho, entrada, tokio::io::stdout()).await?;
    Ok(())
}

/// O estado da API de tarefas, um so para todas as portas que criam tarefa: a rota HTTP
/// do `servir` e o canal do Telegram dividem modelo padrao e balde de fichas por aqui.
fn estado_api(
    raiz: &std::path::Path,
    store: TaskStore,
    factory: AgentFactory,
    token: String,
) -> Result<ApiState> {
    Ok(ApiState {
        store,
        factory,
        default_model: env::var("PHXCLAW_MODELO").unwrap_or_else(|_| MODELO_PADRAO.into()),
        token,
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json"))?)),
        webhook_origins: env::var("PHXCLAW_WEBHOOK_ORIGINS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        limite: Arc::new(phxclaw_agent::api::Limite::por_minuto(
            env::var("PHXCLAW_API_TAREFAS_POR_MINUTO")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        )),
    })
}

/// `phxclaw canal telegram`: o token sai do ambiente direto para o SecretBroker (envelope
/// cifrado na pasta) e nunca e impresso; so os chats de `PHXCLAW_TELEGRAM_CHATS` falam com
/// o agente, e cada mensagem vira tarefa pela mesma `criar_tarefa` da API.
async fn canal(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("telegram") {
        bail!(
            "uso: phxclaw canal telegram [--pasta DIR]  (PHXCLAW_TELEGRAM_BOT_TOKEN, PHXCLAW_TELEGRAM_CHATS)"
        );
    }
    let raiz = pasta(args);
    let token = env::var("PHXCLAW_TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .context("falta PHXCLAW_TELEGRAM_BOT_TOKEN")?;
    let chats = phxclaw_agent::canal::chats_da_lista(
        &env::var("PHXCLAW_TELEGRAM_CHATS").unwrap_or_default(),
    )
    .map_err(anyhow::Error::msg)?;
    let log: phxclaw_agent::canal::Registro = Arc::new(|l: &str| eprintln!("{l}"));
    let canal = phxclaw_agent::canal::ligar_telegram(
        &raiz.join("canal"),
        phxclaw_secret_broker::SecretValue::new(token),
        chats,
        log,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let store = TaskStore::new(raiz.join("tasks"))?;
    let mut m = Montagem::new(store.clone());
    m.canal = Some(canal.clone());
    let factory: AgentFactory = Arc::new(move |modelo: &str| m.agent(modelo));
    // O canal nao abre porta HTTP: o Bearer da API nao serve a ninguem aqui, e nasce
    // aleatorio para nao existir um token fixo esquecido no estado.
    let state = estado_api(
        &raiz,
        store,
        factory,
        phxclaw_api_gateway::generate_bearer_token(),
    )?;
    canal.laco(state).await;
    Ok(())
}

/// Servidor WSS de dispositivos. Os tokens de pareamento vem de um arquivo (uma linha
/// "tenant_uuid token" por token); o registro dos nos fica em memoria enquanto o
/// processo vive -- a verdade duravel e o PostgreSQL, ligado por outro adaptador.
async fn dispositivos(args: &[String]) -> Result<()> {
    use phxclaw_device_transport::servidor::{RegistroMemoria, ServidorDispositivos, tls_de_pem};
    let porta: u16 = opcao(args, "--porta")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(8788);
    let cert = std::fs::read(opcao(args, "--cert").context("falta --cert (PEM)")?)?;
    let chave = std::fs::read(opcao(args, "--chave").context("falta --chave (PEM)")?)?;
    let tokens = std::fs::read_to_string(
        opcao(args, "--tokens").context("falta --tokens (arquivo: tenant_uuid token)")?,
    )?;
    let reg = RegistroMemoria::default();
    let mut n = 0;
    for l in tokens
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let (t, tok) = l
            .split_once(char::is_whitespace)
            .context("linha de token sem espaco")?;
        if tok.trim().len() < 24 {
            bail!("token com menos de 24 caracteres");
        }
        reg.emitir_token(t.parse()?, tok.trim());
        n += 1;
    }
    let srv = Arc::new(
        ServidorDispositivos::novo(Arc::new(Mutex::new(reg))).map_err(anyhow::Error::msg)?,
    );
    let tls = tls_de_pem(&cert, &chave).map_err(anyhow::Error::msg)?;
    let host = env::var("PHXCLAW_DEVICE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "dispositivos em wss://{} ({n} token(s) de pareamento)",
        l.local_addr()?
    );
    srv.servir(l, tls).await;
    Ok(())
}

fn core_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let mut runtime = PhoenixCoreRuntime::new(VERSION, "Master Orchestrator");
    // Estado SONDADO, nao declarado: antes este comando marcava tudo Ready fixo.
    let (st, det) = match std::fs::read_to_string("config/constitution.json") {
        Ok(_) => (
            ServiceState::Ready,
            "config/constitution.json legivel".to_string(),
        ),
        Err(e) => (ServiceState::Degraded, format!("constituicao: {e}")),
    };
    runtime.set_service_state(CoreService::Kernel, st, det);
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .any(|p| std::path::Path::new(p).exists());
    let chromium = phxclaw_browser::find_chromium().is_some();
    runtime.set_service_state(
        CoreService::MissionRuntime,
        if bwrap {
            ServiceState::Ready
        } else {
            ServiceState::Degraded
        },
        format!("agente: sandbox bwrap={bwrap}, navegador chromium={chromium}"),
    );
    let ollama = env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1:11434".into());
    let alvo = ollama
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let ok = alvo
        .parse::<std::net::SocketAddr>()
        .ok()
        .and_then(|a| std::net::TcpStream::connect_timeout(&a, Duration::from_millis(500)).ok())
        .is_some();
    runtime.set_service_state(
        CoreService::ModelGateway,
        if ok {
            ServiceState::Ready
        } else {
            ServiceState::Degraded
        },
        format!(
            "ollama em {alvo}: {}",
            if ok { "responde" } else { "sem resposta" }
        ),
    );
    match sub {
        "status" | "start" => {
            let status = runtime.status();
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        other => bail!("unknown core command: {other}"),
    }
    Ok(())
}

fn db_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str).unwrap_or("plan");
    match sub {
        "plan" => {
            let platform = match args.get(1).map(String::as_str) {
                Some("windows") => HostPlatform::WindowsX64,
                Some("debian") => HostPlatform::DebianLike,
                Some("redhat") => HostPlatform::RedHatLike,
                Some("macos") => HostPlatform::MacOs,
                None => detect_platform(),
                Some(other) => bail!("unknown platform: {other}"),
            };
            let plan = build_install_plan(platform, &PostgreSqlBootstrapConfig::default())?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        other => bail!("unknown db command: {other}"),
    }
    Ok(())
}

fn detect_platform() -> HostPlatform {
    if cfg!(target_os = "windows") {
        HostPlatform::WindowsX64
    } else if cfg!(target_os = "macos") {
        HostPlatform::MacOs
    } else {
        HostPlatform::DebianLike
    }
}

fn print_help() {
    println!(
        "{PRODUCT_NAME} {VERSION}

USAGE:
  {PRODUCT_CLI} <COMMAND>

COMMANDS:
  mcp-serve [--trabalho DIR] [--modelo M] [--pasta DIR]
                     Agent tools as an MCP server over stdio (same PHXCLAW_CAPACIDADES)
  agente \"objetivo\" [--modelo M] [--plano] [--pasta DIR]
                     Run the autonomous agent now, showing each step
  canal telegram [--pasta DIR]
                     Telegram as agent channel (PHXCLAW_TELEGRAM_BOT_TOKEN, PHXCLAW_TELEGRAM_CHATS=id,id)
  servir [--porta 8787] [--pasta DIR]
                     Task API (create, follow, plan approval, cancel, artifacts, schedules)
  dispositivos --cert C --chave K --tokens F [--porta 8788]
                     Device WSS server (TLS, one-time pairing tokens, signed envelopes)
  core status        Probed runtime state (sandbox, browser, model server)
  db plan [platform] Show PostgreSQL managed-install plan
  version            Show version

MODELS: ollama:<model> (local), openai:<model>, anthropic:<model>, gemini:<model>
        (keys from OPENAI_API_KEY / ANTHROPIC_API_KEY / GEMINI_API_KEY)
POLICY: PHXCLAW_CAPACIDADES=web.search,web.browse,fs.read,fs.write,doc.write,shell.exec,agent.spawn"
    );
}

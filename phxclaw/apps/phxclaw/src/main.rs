#![forbid(unsafe_code)]

mod ajuda;
mod config;
mod contexto;
mod interacao;
mod medicao;

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
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help" | "ajuda") {
        match args.get(1).and_then(|n| ajuda::achar(n)) {
            Some(c) => print!("{}", ajuda::de(c, PRODUCT_CLI)),
            None => print_help(),
        }
        return Ok(());
    }
    // `phxclaw COMANDO --help` mostra a ajuda daquele comando, pela mesma tabela.
    if args.len() == 2
        && matches!(args[1].as_str(), "-h" | "--help")
        && let Some(c) = ajuda::achar(&args[0])
    {
        print!("{}", ajuda::de(c, PRODUCT_CLI));
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
        // `phxclaw mcp token|login NOME`: credencial dos MCP remotos para o broker da pasta.
        "mcp" => println!(
            "{}",
            runtime()?
                .block_on(phxclaw_agent::oauth::cli(&pasta(&args[1..]), &args[1..]))
                .map_err(anyhow::Error::msg)?
        ),
        "acp" => runtime()?.block_on(acp(&args[1..]))?,
        "ponte" | "bridge" => runtime()?.block_on(ponte(&args[1..]))?,
        "xai" => {
            // `phxclaw xai chave`: PHXCLAW_XAI_API_KEY vai para o broker da pasta.
            let id = phxclaw_agent::xai::guardar_do_ambiente(&pasta(&args[1..]))
                .map_err(anyhow::Error::msg)?;
            println!("chave da xAI guardada (segredo {id}); x_search pede a capacidade x.search");
        }
        "elevenlabs" => runtime()?.block_on(elevenlabs(&args[1..]))?,
        "gemini" => {
            // `phxclaw gemini chave`: a chave do Nano Banana vai para o broker da pasta.
            let s = &phxclaw_agent::nanobanana::SERVICO;
            let id = s
                .guardar_do_ambiente(&pasta(&args[1..]))
                .map_err(anyhow::Error::msg)?;
            println!(
                "chave da Gemini API guardada (segredo {id}); image_generate usa o Nano Banana \
com PHXCLAW_IMAGEM_PROVEDOR=nanobanana (capacidade media.generate)"
            );
        }
        "dispositivos" | "devices" => runtime()?.block_on(dispositivos(&args[1..]))?,
        "fluxo" | "workflow" => runtime()?.block_on(fluxo(&args[1..]))?,
        "equipe" | "team" => runtime()?.block_on(equipe(&args[1..]))?,
        "gonogo" | "go-no-go" => {
            let (texto, codigo) = phxclaw_agent::gonogo::cli(&args[1..], &pasta(&args[1..]))
                .map_err(anyhow::Error::msg)?;
            print!("{texto}");
            if codigo != 0 {
                std::process::exit(codigo);
            }
        }
        "ferramentas" | "tools" => runtime()?.block_on(ferramentas())?,
        "revisar" | "review" => runtime()?.block_on(revisar(&args[1..]))?,
        "forja" | "forge" => forja(&args[1..])?,
        "sessoes" | "sessions" => interacao::sessoes(
            &TaskStore::new(pasta(&args[1..]).join("tasks"))?,
            &args[1..],
        )?,
        "resumo" | "summary" => interacao::resumo(
            &TaskStore::new(pasta(&args[1..]).join("tasks"))?,
            &args[1..],
        )?,
        "estilos" | "styles" => interacao::estilos(),
        "voz" | "voice" => runtime()?.block_on(voz(&args[1..]))?,
        "repetir" | "replay" => runtime()?.block_on(medicao::repetir(&args[1..]))?,
        "avaliar" | "eval" => runtime()?.block_on(medicao::avaliar(&args[1..]))?,
        "ui" => runtime()?.block_on(medicao::ui(&args[1..]))?,
        "skill" => runtime()?.block_on(medicao::skill(&args[1..]))?,
        "skills" => contexto::skills(&args[1..])?,
        "indexar" | "index" => contexto::indexar(&args[1..])?,
        "projeto" | "project" => contexto::projeto(&args[1..])?,
        "config" => config::comando(&args[1..])?,
        other => match ajuda::sugestao(other) {
            Some(s) => bail!("comando desconhecido: {other}. Quis dizer `{PRODUCT_CLI} {s}`?"),
            None => bail!("comando desconhecido: {other}. Rode `{PRODUCT_CLI} ajuda`."),
        },
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

/// Todos os valores de uma opcao que se repete (`--imagem a.png --imagem b.png`).
fn opcoes(args: &[String], nome: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == nome)
        .map(|w| w[1].clone())
        .collect()
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
        interacao::ao_atualizar(t);
    }
}

async fn agente(args: &[String]) -> Result<()> {
    // `--imagem` se repete: o objetivo e a primeira palavra solta que nao e valor de opcao.
    let objetivo = args
        .iter()
        .enumerate()
        .find(|(i, a)| {
            !a.starts_with("--")
                && !(*i > 0
                    && ["--modelo", "--pasta", "--estilo", "--imagem", "--gravar"]
                        .contains(&args[i - 1].as_str()))
        })
        .map(|(_, a)| a)
        .context("uso: phxclaw agente \"objetivo\" [--modelo ollama:qwen2.5:1.5b] [--plano] [--imagem ARQ]... [--pasta DIR]")?;
    let imagens = opcoes(args, "--imagem")
        .iter()
        .map(|p| std::fs::read(p).with_context(|| format!("--imagem {p}")))
        .collect::<Result<Vec<_>>>()?;
    phxclaw_agent::imagens::validar(&imagens).map_err(anyhow::Error::msg)?;
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let mut m = Montagem::new(store.clone());
    if let Some(e) = opcao(args, "--estilo") {
        m.estilo = Some(e);
    }
    let agente = m.agent(&modelo).map_err(anyhow::Error::msg)?;
    let mut t = Task::new(objetivo.clone(), modelo.clone());
    // As mesmas funcoes da rota `POST /v1/tasks`: gravar em `work/entrada/` e anexar.
    if !imagens.is_empty() {
        store.save(&t)?;
        t.images = phxclaw_agent::imagens::gravar(&store.workdir(&t.id), &imagens)
            .map_err(anyhow::Error::msg)?;
    }
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
        // Plan Mode: so ferramentas de leitura ate aqui; executar pede o sim.
        agente.plan(&mut t).await.map_err(anyhow::Error::msg)?;
        if !interacao::aprovar_plano(&t, args.iter().any(|a| a == "--sim")) {
            println!(
                "plano nao aprovado; tarefa {} fica esperando aprovacao",
                t.id
            );
            return Ok(());
        }
    }
    // A gravacao comeca na execucao: o plano so vale com o sim de quem aprovou, e a
    // repeticao roda a tarefa direto.
    let gravador = opcao(args, "--gravar")
        .map(|arq| {
            phxclaw_agent::gravacao::Gravador::criar(
                std::path::Path::new(&arq),
                &t.objective,
                &modelo,
            )
        })
        .transpose()
        .context("--gravar")?;
    let agente = match &gravador {
        Some(g) => phxclaw_agent::gravacao::gravando(agente, g),
        None => agente,
    };
    println!("executando...");
    let fim = agente
        .run(t, &CancelFlag::default(), &Terminal(Mutex::new(0)))
        .await;
    if let (Some(g), Some(arq)) = (&gravador, opcao(args, "--gravar")) {
        match g.resultado() {
            Ok(n) => println!("gravacao: {arq} ({n} linhas)"),
            Err(e) => println!("gravacao INCOMPLETA: {arq}: {e}"),
        }
    }
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

/// Conversa por voz, um turno por WAV: whisper -> agente -> fala, pelo mesmo motor das
/// ferramentas `transcribe` e `speak`.
async fn voz(args: &[String]) -> Result<()> {
    let wavs: Vec<PathBuf> = args
        .iter()
        .filter(|a| {
            !a.starts_with("--")
                && ["--modelo", "--pasta"]
                    .iter()
                    .all(|o| Some(*a) != opcao(args, o).as_ref())
        })
        .map(PathBuf::from)
        .collect();
    if wavs.is_empty() {
        bail!(
            "uso: phxclaw voz ARQ.wav [ARQ2.wav ...] [--modelo ollama:qwen2.5:1.5b] [--pasta DIR]"
        );
    }
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let agente = phxclaw_agent::voz::agente_de_conversa(
        Montagem::new(store)
            .agent(&modelo)
            .map_err(anyhow::Error::msg)?,
    );
    let turnos = phxclaw_agent::voz::conversar(
        &agente,
        &modelo,
        &wavs,
        &phxclaw_agent::visao::TranscribeTool::do_ambiente(&pasta(args)),
        &phxclaw_agent::voz::SpeakTool::do_ambiente(&pasta(args)),
        || Terminal(Mutex::new(0)),
    )
    .await;
    let mut falhou = false;
    for (w, t) in wavs.iter().zip(&turnos) {
        println!("\nturno {} (tarefa {})", w.display(), t.tarefa);
        println!("  ouvido: {}", t.ouvido);
        println!("  resposta: {}", t.resposta);
        match (&t.wav, &t.erro) {
            (Some(p), _) => println!("  fala: {}", p.display()),
            (None, Some(e)) => {
                falhou = true;
                println!("  erro: {e}")
            }
            (None, None) => {}
        }
    }
    if falhou {
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
    let token = token_de(&raiz.join("api.token"), "PHXCLAW_API_TOKEN")?;
    let mut m = Montagem::new(store.clone());
    // Nos de dispositivo no MESMO processo: e assim que `node_list`/`node_invoke`
    // alcancam as sessoes vivas.
    if args.iter().any(|a| a == "--dispositivos") {
        let p: u16 = opcao(args, "--porta-dispositivos")
            .map(|p| p.parse())
            .transpose()?
            .unwrap_or(8788);
        m.dispositivos = Some(subir_dispositivos(args, p).await?);
    }
    // `--canal <nome>`: o canal roda no MESMO processo da API, pelo mesmo motor do
    // `phxclaw canal`, e a entrada HTTP dele (webchat, webhook) sai pela porta da API.
    let ligado = match opcao(args, "--canal") {
        Some(nome) => {
            let log: phxclaw_agent::canal::Registro = Arc::new(|l: &str| eprintln!("{l}"));
            let l = phxclaw_agent::canais::ligar::ligar(
                &nome,
                &raiz.join("canal"),
                &|k: &str| env::var(k).ok(),
                log,
            )
            .await
            .map_err(anyhow::Error::msg)?;
            m.canal = Some(l.canal.clone());
            Some(l)
        }
        None => None,
    };
    let factory: AgentFactory = Arc::new(move |modelo: &str| m.agent(modelo));
    let state = estado_api(&raiz, store, factory, token)?;
    let mut rotas_do_canal = None;
    if let Some(l) = ligado {
        rotas_do_canal = l.rotas;
        tokio::spawn(l.canal.laco(state.clone()));
    }
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
    let gatilhos = armar_gatilhos(&raiz, &state)?;
    // `--ponte wss://...`: controle remoto por conexao de SAIDA ate a ponte; o pedido do
    // celular roda no MESMO router desta API, em processo.
    if let Some(url) = opcao(args, "--ponte") {
        let cfg = ponte_do_agente(args, &raiz, url)?;
        tokio::spawn(phxclaw_agent::remoto::ligar_a_ponte(
            cfg,
            router(state.clone()),
            state.token.clone(),
        ));
        if args.iter().any(|a| a == "--sem-porta") {
            println!("sem porta local: o agente so e alcancado pela ponte");
            std::future::pending::<()>().await;
        }
    }
    // Loopback por padrao: expor a API na rede e decisao explicita do operador.
    let host = env::var("PHXCLAW_API_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "API de tarefas em http://{}  (token em {})",
        l.local_addr()?,
        raiz.join("api.token").display()
    );
    let mut app = router(state).merge(gatilhos);
    if let Some(r) = rotas_do_canal {
        app = app.merge(r);
    }
    axum::serve(l, app).await?;
    Ok(())
}

/// Heartbeat e gatilhos de arquivo num laco proprio (a pasta observada e varrida a cada
/// 5 s), e as rotas de webhook para o `merge` do router. Tudo cria tarefa pela mesma
/// `criar_tarefa_com` da API.
fn armar_gatilhos(raiz: &std::path::Path, state: &ApiState) -> Result<axum::Router> {
    use phxclaw_agent::gatilhos;
    let projeto = phxclaw_agent::montagem::pasta_do_projeto();
    let g = match &projeto {
        Some(p) => gatilhos::Gatilhos::carregar(p).map_err(anyhow::Error::msg)?,
        None => gatilhos::Gatilhos::default(),
    };
    let mut hb = gatilhos::Heartbeat::do_ambiente(projeto.as_deref(), raiz);
    let armados = gatilhos::descrever(&g, hb.as_ref());
    if !armados.is_empty() {
        println!("gatilhos: {armados}");
    }
    let mut obs: Vec<gatilhos::Observador> = g
        .arquivos
        .iter()
        .cloned()
        .map(gatilhos::Observador::new)
        .collect();
    let st = state.clone();
    tokio::spawn(async move {
        loop {
            for r in gatilhos::disparar_arquivos(&st, &mut obs) {
                match r {
                    Ok(c) => println!("gatilho de arquivo: tarefa {}", c.id),
                    Err(e) => eprintln!("gatilho de arquivo recusado: {}", e.erro),
                }
            }
            if let Some(h) = hb.as_mut() {
                match gatilhos::disparar_heartbeat(&st, h, std::time::Instant::now()) {
                    Some(Ok(c)) => println!("heartbeat: tarefa {}", c.id),
                    Some(Err(e)) => eprintln!("heartbeat recusado: {}", e.erro),
                    None => {}
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    Ok(gatilhos::router(state.clone(), Arc::new(g)))
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

/// Agent Client Protocol pela entrada padrao: o editor (Zed, acpx...) lanca o processo e
/// fala JSON-RPC em linhas. O stdout e o fio; aviso so no stderr.
async fn acp(args: &[String]) -> Result<()> {
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let modelo = opcao(args, "--modelo")
        .or_else(|| env::var("PHXCLAW_MODELO").ok())
        .unwrap_or_else(|| MODELO_PADRAO.into());
    let agente = Montagem::new(store)
        .agent(&modelo)
        .map_err(anyhow::Error::msg)?;
    let entrada = tokio::io::BufReader::new(tokio::io::stdin());
    phxclaw_agent::acp::servir(agente, entrada, tokio::io::stdout()).await?;
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

/// `phxclaw canal <nome>`: o mesmo motor para todos os canais (`canais::ligar`). Segredo
/// sai do ambiente direto para o SecretBroker (envelope cifrado na pasta) e nunca e
/// impresso; so as conversas de `PHXCLAW_<NOME>_PERMITIDOS` (no Telegram,
/// `PHXCLAW_TELEGRAM_CHATS`) falam com o agente, e cada mensagem vira tarefa pela mesma
/// `criar_tarefa` da API. Canal de webhook escuta em `--escuta` (padrao 127.0.0.1:8790;
/// a exposicao publica fica para o proxy com TLS na frente).
async fn canal(args: &[String]) -> Result<()> {
    let Some(nome) = args.first().filter(|a| !a.starts_with('-')) else {
        bail!(
            "uso: phxclaw canal <{}> [--pasta DIR] [--escuta ENDERECO]",
            phxclaw_agent::canais::ligar::CANAIS.join("|")
        );
    };
    let raiz = pasta(args);
    let log: phxclaw_agent::canal::Registro = Arc::new(|l: &str| eprintln!("{l}"));
    let ligado = phxclaw_agent::canais::ligar::ligar(
        nome,
        &raiz.join("canal"),
        &|k: &str| env::var(k).ok(),
        log,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let canal = ligado.canal;
    if let Some(rotas) = ligado.rotas {
        let escuta = opcao(args, "--escuta").unwrap_or_else(|| "127.0.0.1:8790".into());
        let l = tokio::net::TcpListener::bind(&escuta)
            .await
            .with_context(|| format!("escutar em {escuta}"))?;
        eprintln!("{nome}: entrada HTTP em http://{escuta}");
        tokio::spawn(async move {
            if let Err(e) = axum::serve(l, rotas).await {
                eprintln!("entrada HTTP caiu: {e}");
            }
        });
    }
    let store = TaskStore::new(raiz.join("tasks"))?;
    let mut m = Montagem::new(store.clone());
    m.canal = Some(canal.clone());
    let factory: AgentFactory = Arc::new(move |modelo: &str| m.agent(modelo));
    // O canal nao abre a API de tarefas: o Bearer dela nao serve a ninguem aqui, e nasce
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
/// "tenant_uuid token [capacidades,aprovadas]" por token); o registro dos nos fica em memoria enquanto o
/// processo vive -- a verdade duravel e o PostgreSQL, ligado por outro adaptador.
/// Token de variavel de ambiente, ou arquivo 0600 gerado na primeira vez. O da API e o
/// da ponte saem daqui: a mesma regra de tamanho e de arquivo privado para os dois.
fn token_de(arq: &std::path::Path, var: &str) -> Result<String> {
    Ok(match env::var(var) {
        Ok(t) if t.len() >= 24 => t,
        Ok(_) => bail!("{var} precisa de pelo menos 24 caracteres"),
        Err(_) => match std::fs::read_to_string(arq) {
            Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                let t = phxclaw_api_gateway::generate_bearer_token();
                phxclaw_secret_broker::write_private_file(arq, t.as_bytes())?;
                t
            }
        },
    })
}

/// Como o agente se liga a ponte. O token de pareamento vem so do ambiente (segredo em
/// argumento aparece no `ps`); o no ganha um UUID na primeira vez, guardado na pasta.
fn ponte_do_agente(
    args: &[String],
    raiz: &std::path::Path,
    url: String,
) -> Result<phxclaw_agent::remoto::ConfigDaPonte> {
    let uuid = |op: &str, var: &str| -> Result<Option<phxclaw_agent::remoto::Uuid>> {
        Ok(opcao(args, op)
            .or_else(|| env::var(var).ok())
            .map(|v| v.parse())
            .transpose()?)
    };
    let tenant = uuid("--ponte-tenant", "PHXCLAW_TENANT_UUID")?
        .context("falta --ponte-tenant (ou PHXCLAW_TENANT_UUID)")?;
    let no = match uuid("--ponte-no", "PHXCLAW_NODE_UUID")? {
        Some(n) => n,
        None => {
            let arq = raiz.join("ponte-no");
            match std::fs::read_to_string(&arq)
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                Some(n) => n,
                None => {
                    let n = phxclaw_agent::remoto::Uuid::now_v7();
                    std::fs::create_dir_all(raiz)?;
                    std::fs::write(&arq, n.to_string())?;
                    n
                }
            }
        }
    };
    Ok(phxclaw_agent::remoto::ConfigDaPonte {
        url,
        ca_pem: opcao(args, "--ponte-ca").map(std::fs::read).transpose()?,
        tenant,
        no,
        token_pareamento: env::var("PHXCLAW_ENROLLMENT_TOKEN").ok(),
        pasta_da_chave: raiz.join("ponte-chave"),
    })
}

/// A ponte do controle remoto: o servidor WSS de dispositivos (onde o agente se liga, de
/// saida) e o HTTP do cliente (a tela instalavel e o rele das rotas de tarefa).
async fn ponte(args: &[String]) -> Result<()> {
    let raiz = pasta(args);
    std::fs::create_dir_all(&raiz)?;
    let porta_wss: u16 = opcao(args, "--porta-wss")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(8791);
    let porta: u16 = opcao(args, "--porta")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(8790);
    let srv = subir_dispositivos(args, porta_wss).await?;
    let token = token_de(&raiz.join("ponte.token"), "PHXCLAW_PONTE_TOKEN")?;
    let host = env::var("PHXCLAW_PONTE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "ponte em http://{}  (token do cliente em {})",
        l.local_addr()?,
        raiz.join("ponte.token").display()
    );
    axum::serve(l, phxclaw_agent::remoto::rotas_da_ponte(srv, token)).await?;
    Ok(())
}

async fn dispositivos(args: &[String]) -> Result<()> {
    let porta: u16 = opcao(args, "--porta")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(8788);
    subir_dispositivos(args, porta).await?;
    std::future::pending::<()>().await;
    Ok(())
}

/// Sobe o servidor WSS de dispositivos numa tarefa e devolve o servidor: o `dispositivos`
/// so espera, o `servir --dispositivos` o entrega ao agente.
async fn subir_dispositivos(
    args: &[String],
    porta: u16,
) -> Result<Arc<phxclaw_device_transport::servidor::ServidorDispositivos>> {
    use phxclaw_device_transport::servidor::{RegistroMemoria, ServidorDispositivos, tls_de_pem};
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
        // "tenant token [cap,cap]": a terceira coluna e o que o operador APROVA para
        // comando no no pareado por este token; sem ela, o no so da presenca.
        let mut col = l.split_whitespace();
        let (Some(t), Some(tok)) = (col.next(), col.next()) else {
            bail!("linha de token sem espaco");
        };
        if tok.len() < 24 {
            bail!("token com menos de 24 caracteres");
        }
        let aprovadas: Vec<&str> = col
            .next()
            .map(|c| c.split(',').filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        reg.emitir_token_aprovando(t.parse()?, tok, &aprovadas);
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
    tokio::spawn(srv.clone().servir(l, tls));
    Ok(srv)
}

/// `phxclaw fluxo rodar ARQ`: o fluxo declarativo pelo motor do agente
/// (`phxclaw_agent::fluxos`), o mesmo que qualquer outra entrada usaria.
async fn fluxo(args: &[String]) -> Result<()> {
    const USO: &str =
        "uso: phxclaw fluxo rodar ARQ.json | retomar TAREFA ARQ.json [--modelo M] [--pasta DIR]";
    let (arq, retomada) = match (args.first().map(String::as_str), args.get(1), args.get(2)) {
        (Some("rodar" | "run"), Some(arq), _) => (arq, None),
        (Some("retomar" | "resume"), Some(t), Some(arq)) => (arq, Some(t.clone())),
        _ => bail!("{USO}"),
    };
    let f =
        phxclaw_agent::fluxos::ler(&std::fs::read_to_string(arq)?).map_err(anyhow::Error::msg)?;
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let agente = Montagem::new(store)
        .agent(&modelo)
        .map_err(anyhow::Error::msg)?;
    println!(
        "fluxo {} ({} passos), modelo {modelo}",
        f.nome,
        f.passos.len()
    );
    let r = match &retomada {
        None => phxclaw_agent::fluxos::rodar(&agente, &f).await,
        Some(t) => phxclaw_agent::fluxos::retomar(&agente, &f, t).await,
    }
    .map_err(anyhow::Error::msg)?;
    for p in &r.passos {
        let resumo: String = p.saida.lines().take(2).collect::<Vec<_>>().join(" | ");
        println!(
            "  {:<8} {}  {}",
            if p.reaproveitado {
                "retomado"
            } else {
                p.estado.as_str()
            },
            p.id,
            resumo.chars().take(140).collect::<String>()
        );
    }
    println!(
        "tarefa do fluxo: {} (retome com: fluxo retomar {} ARQ)",
        r.tarefa, r.tarefa
    );
    if !r.sucesso {
        std::process::exit(2);
    }
    Ok(())
}

/// `phxclaw equipe`: os papeis pelas MESMAS funcoes do `team_list` e do
/// `team_delegate` (modulo `equipe` do agente) -- a CLI nao tem lista nem delegacao propria.
async fn equipe(args: &[String]) -> Result<()> {
    use phxclaw_agent::equipe as eq;
    const USO: &str = "uso: phxclaw equipe [listar [--macroarea X] [--texto Y] | mostrar ID | delegar ID \"tarefa\" [--modelo M] [--pasta DIR]]";
    let sub = args.first().map(String::as_str).unwrap_or("listar");
    match sub {
        "listar" | "list" => {
            let e = eq::Equipe::do_ambiente().map_err(anyhow::Error::msg)?;
            let fichas = e.listar(
                opcao(args, "--macroarea").as_deref(),
                opcao(args, "--texto").as_deref(),
            );
            print!("{}", eq::texto_da_lista(&fichas, e.len()));
        }
        "mostrar" | "show" => {
            let e = eq::Equipe::do_ambiente().map_err(anyhow::Error::msg)?;
            let id = args.get(1).context(USO)?;
            let m = e.achar(id).map_err(anyhow::Error::msg)?;
            print!("{}", eq::texto_do_papel(&e, m));
        }
        "delegar" | "delegate" => {
            let (id, tarefa) = (args.get(1).context(USO)?, args.get(2).context(USO)?);
            let modelo = opcao(args, "--modelo")
                .or_else(|| env::var("PHXCLAW_MODELO").ok())
                .unwrap_or_else(|| MODELO_PADRAO.into());
            let store = TaskStore::new(pasta(args).join("tasks"))?;
            let m = Montagem::new(store);
            let e = m
                .equipe
                .clone()
                .context("agente sem equipe (veja o aviso acima)")?;
            let base = m.agent(&modelo).map_err(anyhow::Error::msg)?;
            let local = m.modelo_local();
            let d = eq::delegar(&e, &base, id, tarefa, None, local.as_ref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!("{}", eq::texto_da_delegacao(&d));
            match d {
                // Codigo proprio: script que chama a CLI distingue "precisa de gente" de
                // "o subagente falhou" sem ler o texto.
                eq::Delegacao::Humano { .. } => std::process::exit(3),
                eq::Delegacao::Rodou { tarefa, .. } if tarefa.status != TaskStatus::Completed => {
                    std::process::exit(2)
                }
                _ => {}
            }
        }
        _ => bail!("{USO}"),
    }
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

/// As ferramentas que a montagem do agente registra NESTA maquina, em JSON: e o que a tela
/// Ferramentas do desktop le. Sai da mesma `Montagem` do `agente`, porque uma lista escrita
/// a mao envelheceria no dia da proxima ferramenta. Varia por maquina (sem bwrap nao ha
/// shell, sem Chromium nao ha navegador), e o JSON diz isso em vez de esconder.
async fn ferramentas() -> Result<()> {
    let pasta = env::temp_dir().join(format!("phxclaw-ferramentas-{}", std::process::id()));
    let store = TaskStore::new(pasta.join("tasks"))?;
    let m = Montagem::new(store);
    let agente = m.agent(MODELO_PADRAO).map_err(anyhow::Error::msg)?;
    let lista: Vec<serde_json::Value> = agente
        .tools
        .iter()
        .map(|t| {
            let spec = t.spec();
            let cap = t.capability();
            serde_json::json!({
                "nome": spec.name,
                "capacidade": cap,
                "grupo": cap.split('.').next().unwrap_or(cap),
                "concedida": agente.config.capabilities.contains(cap),
                "descricao": spec.description,
            })
        })
        .collect();
    let _ = std::fs::remove_dir_all(&pasta);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "gerado_por": format!("{PRODUCT_CLI} ferramentas"),
            "versao": VERSION,
            "observacao": "montadas nesta maquina: shell exige bwrap, navegador exige Chromium, \
                           e-mail e banco exigem configuracao",
            "total": lista.len(),
            "ferramentas": lista,
        }))?
    );
    Ok(())
}

/// `phxclaw revisar`: so le as opcoes; diff e revisao sao os do `code_review`
/// (`revisao::revisar_da_fonte`), para a CLI e a ferramenta nao divergirem.
async fn revisar(args: &[String]) -> Result<()> {
    use phxclaw_agent::revisao::{FonteDoDiff, SEVERIDADES, analisar_pr, modelo, revisar_da_fonte};
    let comentar = args.iter().any(|a| a == "--comentar");
    let evento = opcao(args, "--evento");
    let pr = opcao(args, "--pr")
        .map(|p| analisar_pr(&p).map_err(anyhow::Error::msg))
        .transpose()?;
    // Recusa antes de gastar o modelo: so ha onde comentar quando ha um PR.
    if comentar && evento.is_none() && pr.is_none() {
        bail!("--comentar so com --pr ou --evento");
    }
    let falhar = opcao(args, "--falhar-em");
    if let Some(f) = &falhar
        && !SEVERIDADES.contains(&f.as_str())
    {
        bail!("--falhar-em {f}: use {}", SEVERIDADES.join(", "));
    }
    let llm = modelo(&opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into()))
        .map_err(anyhow::Error::msg)?;
    let foco = opcao(args, "--foco");
    let r = if let Some(ev) = evento {
        // GitHub Action: o PR sai do `GITHUB_EVENT_PATH`, pelo mesmo `revisar_da_fonte`.
        let nome = env::var("GITHUB_EVENT_NAME").ok();
        let r = phxclaw_agent::acao_github::rodar(
            &pasta(args),
            std::path::Path::new(&ev),
            nome.as_deref(),
            llm.as_ref(),
            foco.as_deref(),
            comentar,
        )
        .await
        .map_err(anyhow::Error::msg)?;
        let Some(r) = r else {
            println!("{{\"revisado\": false, \"motivo\": \"o evento nao pede revisao\"}}");
            return Ok(());
        };
        r
    } else {
        let fonte = if let Some((forja, repo, numero)) = pr.clone() {
            FonteDoDiff::Pr {
                raiz_do_agente: pasta(args),
                forja,
                repo,
                numero,
            }
        } else if let Some(d) = opcao(args, "--diff") {
            FonteDoDiff::Arquivo(PathBuf::from(d))
        } else {
            FonteDoDiff::Repo {
                pasta: PathBuf::from(opcao(args, "--repo").unwrap_or_else(|| ".".into())),
                rev: opcao(args, "--rev"),
                cached: args.iter().any(|a| a == "--cached"),
            }
        };
        let r = revisar_da_fonte(llm.as_ref(), fonte, foco.as_deref())
            .await
            .map_err(anyhow::Error::msg)?;
        if let (true, Some((forja, repo, numero))) = (comentar, pr) {
            phxclaw_agent::acao_github::comentar(&pasta(args), forja, &repo, numero, &r)
                .await
                .map_err(anyhow::Error::msg)?;
        }
        r
    };
    println!("{}", serde_json::to_string_pretty(&r)?);
    if falhar.is_some_and(|f| r.tem_ao_menos(&f)) {
        std::process::exit(1);
    }
    Ok(())
}

/// `phxclaw forja token github|gitlab`: token do ambiente para o broker da pasta.
fn forja(args: &[String]) -> Result<()> {
    use phxclaw_agent::forja::{Forja, guardar_do_ambiente, pasta_da_forja};
    let (Some("token"), Some(f)) = (
        args.first().map(String::as_str),
        args.get(1).and_then(|n| Forja::de_nome(n)),
    ) else {
        bail!("uso: phxclaw forja token github|gitlab [--pasta DIR]");
    };
    let raiz = pasta(args);
    let id = guardar_do_ambiente(&raiz, f).map_err(anyhow::Error::msg)?;
    println!(
        "token de {} guardado no broker de {} (segredo {id})",
        f.nome(),
        pasta_da_forja(&raiz).display()
    );
    Ok(())
}

/// `phxclaw elevenlabs chave|vozes`: guardar a chave no broker, ou listar as vozes da conta
/// pelo mesmo cliente do `speak` e do `voice_list`.
async fn elevenlabs(args: &[String]) -> Result<()> {
    use phxclaw_agent::elevenlabs::{ElevenLabs, SERVICO, listar};
    let raiz = pasta(args);
    match args.first().map(String::as_str) {
        Some("chave" | "key") => {
            let id = SERVICO
                .guardar_do_ambiente(&raiz)
                .map_err(anyhow::Error::msg)?;
            println!(
                "chave da ElevenLabs guardada (segredo {id}); speak e transcribe a usam com \
PHXCLAW_TTS_PROVEDOR=elevenlabs / PHXCLAW_STT_PROVEDOR=elevenlabs"
            );
        }
        Some("vozes" | "voices") => {
            let c = ElevenLabs::da_pasta(&raiz).map_err(anyhow::Error::msg)?;
            let busca = opcao(args, "--busca");
            let v = tokio::task::spawn_blocking(move || {
                c.vozes(busca.as_deref(), std::time::Duration::from_secs(30))
            })
            .await?
            .map_err(anyhow::Error::msg)?;
            println!("{}", listar(&v));
        }
        _ => bail!("uso: phxclaw elevenlabs chave|vozes [--busca TEXTO] [--pasta DIR]"),
    }
    Ok(())
}

fn print_help() {
    // A ajuda sai da tabela de ajuda.rs; nenhum texto de comando mora aqui.
    print!("{}", ajuda::texto(PRODUCT_NAME, VERSION, PRODUCT_CLI));
}

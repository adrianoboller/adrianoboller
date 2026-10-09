#![forbid(unsafe_code)]

mod ajuda;
mod config;
mod contexto;
mod evolucao;
mod ferramenta;
mod interacao;
mod medicao;
mod rota;

use anyhow::{Context, Result, bail};
use phxclaw_agent::api::{AgentFactory, ApiState, disparar_agenda, router};
use phxclaw_agent::loja::Loja;
use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::{Agenda, CancelFlag, Observer, Task, TaskStatus, TaskStore};
use phxclaw_core_runtime::{
    CoreService, PRODUCT_CLI, PRODUCT_NAME, PhoenixCoreRuntime, ServiceState,
};
use phxclaw_postgres_bootstrap::{HostPlatform, PostgreSqlBootstrapConfig, build_install_plan};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MODELO_PADRAO: &str = "ollama:qwen2.5:1.5b";

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty()
        || matches!(
            args[0].as_str(),
            "-h" | "--help" | "--ajuda" | "help" | "ajuda"
        )
    {
        match args.get(1).map(String::as_str) {
            // Tudo: os comandos, cada ferramenta montada com os parametros do esquema e
            // cada rota da API -- o que `docs/AGENTE_AUTONOMO.md` publica.
            Some("--tudo" | "--all") => print!(
                "{}",
                ajuda::tudo(
                    PRODUCT_NAME,
                    VERSION,
                    PRODUCT_CLI,
                    &ferramenta::montar(&args[2..], MODELO_PADRAO)?
                )
            ),
            Some(n) if ajuda::achar(n).is_some() => {
                print!("{}", ajuda::de(ajuda::achar(n).unwrap(), PRODUCT_CLI))
            }
            _ => print_help(),
        }
        return Ok(());
    }
    // `phxclaw COMANDO --ajuda` mostra a ajuda daquele comando, pela mesma tabela.
    if args.len() == 2
        && matches!(args[1].as_str(), "-h" | "--help" | "--ajuda")
        && let Some(c) = ajuda::achar(&args[0])
    {
        print!("{}", ajuda::de(c, PRODUCT_CLI));
        return Ok(());
    }
    // A pasta do comando vale para TODA leitura de configuracao do processo, inclusive a que
    // se recarrega depois de um `PUT /v1/config`: sem fixar, `config::valor` lia
    // `var/agente` enquanto o comando gravava em `--pasta` (prova F, 01/10).
    phxclaw_agent::config::fixar_pasta(&pasta(&args[1..]));
    // Arquivo invalido e ERRO na porta, nao «vale o padrao» calado la dentro: todo comando
    // le pelo ponto unico, e seguir com o padrao faria uma restricao escrita no arquivo
    // deixar de valer sem ninguem ver. So o `config` entra com arquivo invalido -- e ele
    // que o conserta.
    if args[0] != "config" {
        let c = phxclaw_agent::config::configuracao().map_err(anyhow::Error::msg)?;
        // A chave so do operador que o projeto declarou nao vale (o catalogo decide), e
        // quem a escreveu precisa ouvir isso antes de achar que ela valeu.
        for e in &c.ignoradas_do_projeto {
            eprintln!("aviso: .phxclaw/config.json: {e}");
        }
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
        // `phxclaw credencial guardar|renovacao|login NOME`: o segredo de uma credencial
        // nomeada do no HTTP (declarada em http.json) pela entrada padrao, para o broker.
        "credencial" | "credential" => println!(
            "{}",
            runtime()?
                .block_on(phxclaw_agent::fluxo_http::cli(
                    &pasta(&args[1..]),
                    &args[1..],
                    &mut std::io::stdin().lock(),
                ))
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
        // `phxclaw n8n chave|segredo`: a chave da API e o segredo do webhook vao para o
        // broker da pasta `n8n/`, pelo mesmo `Servico` dos outros (docs/N8N.md).
        "n8n" => match args.get(1).map(String::as_str) {
            Some("chave") => chave_de(&phxclaw_agent::n8n::SERVICO, &args[1..])?,
            Some("segredo") => {
                let raiz = pasta(&args[2..]);
                let s = &phxclaw_agent::n8n::SEGREDO_WEBHOOK;
                let id = s.guardar_do_ambiente(&raiz).map_err(anyhow::Error::msg)?;
                println!(
                    "{} guardado no broker de {} (segredo {id}); n8n_workflow run assina com ele",
                    s.rotulo,
                    s.pasta(&raiz).display()
                );
            }
            _ => bail!("uso: phxclaw n8n chave|segredo [--pasta DIR]"),
        },
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
        // Os segredos que so tinham variavel de ambiente: `phxclaw <servico> chave` guarda
        // a variavel no broker pelo mesmo `Servico` da ElevenLabs e da Gemini.
        // `api chave` guarda o Bearer; `api rotas` e `api METODO ROTA` chamam o servidor
        // de pe pelo cliente do SDK (`rota.rs`): o mesmo handler, nunca uma copia.
        "api" => match args.get(1).map(String::as_str) {
            Some("chave") => chave_de(&phxclaw_agent::chaves::API, &args[1..])?,
            _ => {
                let raiz = pasta(&args[1..]);
                let token =
                    token_existente(&raiz, &raiz.join("api.token"), &phxclaw_agent::chaves::API)?;
                let c = runtime()?.block_on(rota::comando(&args[1..], token, PRODUCT_CLI))?;
                if c != 0 {
                    std::process::exit(c);
                }
            }
        },
        // Usuarios da API: so pela CLI local, nunca por HTTP (`rbac::comando`).
        "usuario" | "user" => println!(
            "{}",
            phxclaw_agent::rbac::comando(&pasta(&args[1..]), &args[1..])
                .map_err(anyhow::Error::msg)?
        ),
        "openai" => chave_de(&phxclaw_agent::chaves::OPENAI, &args[1..])?,
        "anthropic" => chave_de(&phxclaw_agent::chaves::ANTHROPIC, &args[1..])?,
        "imagem" | "image" => chave_de(&phxclaw_agent::chaves::IMAGEM, &args[1..])?,
        "email" => {
            if args.get(1).map(String::as_str) != Some("chave") {
                bail!("uso: phxclaw email chave [--pasta DIR]");
            }
            let raiz = pasta(&args[1..]);
            let id = phxclaw_agent::email::guardar_senha_smtp(&raiz).map_err(anyhow::Error::msg)?;
            println!(
                "senha do SMTP guardada no broker do canal (segredo {id}); send_email e o canal \
de e-mail a leem quando PHXCLAW_SMTP_PASSWORD nao esta no ambiente"
            );
        }
        "plugins" => plugins(&args[1..])?,
        "fluxo" | "workflow" => runtime()?.block_on(fluxo(&args[1..]))?,
        "agenda" | "schedule" => runtime()?.block_on(agenda(&args[1..]))?,
        "equipe" | "team" => runtime()?.block_on(equipe(&args[1..]))?,
        "gonogo" | "go-no-go" => {
            let (texto, codigo) = phxclaw_agent::gonogo::cli(
                &args[1..],
                &pasta(&args[1..]),
                &mut std::io::stdin().lock(),
            )
            .map_err(anyhow::Error::msg)?;
            print!("{texto}");
            if codigo != 0 {
                std::process::exit(codigo);
            }
        }
        "ferramentas" | "tools" => runtime()?.block_on(ferramentas())?,
        // Qualquer ferramenta montada, com os parametros do esquema, pelo portao unico.
        "ferramenta" | "tool" => {
            let c =
                runtime()?.block_on(ferramenta::comando(&args[1..], PRODUCT_CLI, MODELO_PADRAO))?;
            if c != 0 {
                std::process::exit(c);
            }
        }
        // O completar do shell, do mesmo inventario da ajuda.
        "completar" | "completion" => print!(
            "{}",
            ajuda::completar(args.get(1).map(String::as_str).unwrap_or(""), PRODUCT_CLI)
                .map_err(anyhow::Error::msg)?
        ),
        "revisar" | "review" => runtime()?.block_on(revisar(&args[1..]))?,
        "testes" | "tests" => runtime()?.block_on(testes(&args[1..]))?,
        "tarefa" | "task" => runtime()?.block_on(tarefa(&args[1..]))?,
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
        "medir" | "measure" => medicao::medir(&args[1..])?,
        "avaliar" | "eval" => runtime()?.block_on(medicao::avaliar(&args[1..]))?,
        "ui" => runtime()?.block_on(medicao::ui(&args[1..]))?,
        "skill" => runtime()?.block_on(medicao::skill(&args[1..]))?,
        "skills" => contexto::skills(&args[1..])?,
        "licenca" | "license" => contexto::licenca(&args[1..])?,
        "indexar" | "index" => contexto::indexar(&args[1..])?,
        "projeto" | "project" => contexto::projeto(&args[1..])?,
        "evoluir" | "evolve" => runtime()?.block_on(evolucao::comando(&args[1..]))?,
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

/// `--pasta`, ou a pasta padrao do ponto unico da configuracao (`PHXCLAW_HOME` ou
/// `var/agente`): uma regra so, para a CLI e o `config::valor` nunca lerem pastas diferentes.
fn pasta(args: &[String]) -> PathBuf {
    opcao(args, "--pasta")
        .map(PathBuf::from)
        .unwrap_or_else(phxclaw_agent::config::pasta_padrao)
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
                    && [
                        "--modelo",
                        "--pasta",
                        "--estilo",
                        "--imagem",
                        "--gravar",
                        "--verificar",
                        "--saida-esquema",
                    ]
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
    // O fim conferido: o comando roda no sandbox da tarefa e o esquema se le aqui, antes de
    // gastar o modelo -- arquivo que nao e JSON de objeto para agora, dizendo qual.
    t.verificar = opcao(args, "--verificar").filter(|v| !v.trim().is_empty());
    if let Some(arq) = opcao(args, "--saida-esquema") {
        let e: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&arq).with_context(|| format!("--saida-esquema {arq}"))?,
        )
        .with_context(|| format!("--saida-esquema {arq}: JSON invalido"))?;
        if !e.is_object() {
            bail!("--saida-esquema {arq}: o esquema tem de ser um objeto JSON");
        }
        t.saida_esquema = Some(e);
    }
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
        Some(g) => {
            g.tarefa(&t.id);
            phxclaw_agent::gravacao::gravando(agente, g)
        }
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
    println!("{}", phxclaw_agent::custo::linha(&fim));
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
    let token = token_de(&raiz, &raiz.join("api.token"), &phxclaw_agent::chaves::API)?;
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
                &phxclaw_agent::config::por_variavel,
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
    // O interruptor das metricas, uma vez, antes do primeiro pedido: desligado, nenhuma
    // ferramenta e envolvida e o `/metrics` responde 404.
    phxclaw_agent::metricas::ligar(
        phxclaw_agent::config::booleano_de("api.metricas").unwrap_or(false),
    );
    if state.usuarios.ligado() {
        println!(
            "usuarios da API: {} (papeis e projetos)",
            raiz.join(phxclaw_agent::rbac::ARQUIVO).display()
        );
    }
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
            // Esperas de tempo vencidas e poda das execucoes: o mesmo laco, sem outro relogio.
            let r = phxclaw_agent::api::manter_fluxos(&agenda_state);
            if !r.is_empty() {
                println!("fluxos: {} espera(s) vencida(s) retomada(s)", r.len());
            }
        }
    });
    let gatilhos = armar_gatilhos(&raiz, &state)?;
    // Historico local das gravacoes do projeto do IDE (file_history), um poller so.
    if let Some(p) = phxclaw_agent::historico::iniciar_do_projeto() {
        println!(
            "historico de gravacoes: {}",
            p.join(phxclaw_agent::historico::PASTA).display()
        );
    }
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
    let host = phxclaw_agent::config::texto_de("api.host").unwrap_or_else(|| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "API de tarefas em http://{}  (token em {})",
        l.local_addr()?,
        raiz.join("api.token").display()
    );
    // O roteador FINAL e um so, e os cabecalhos de seguranca se aplicam nele: juntar os
    // gatilhos e o canal DEPOIS da blindagem deixava o formulario sem moldura proibida (M5).
    let app = phxclaw_agent::api::servidor(state, std::iter::once(gatilhos).chain(rotas_do_canal));
    axum::serve(l, app).await?;
    Ok(())
}

/// Heartbeat e gatilhos de arquivo num laco proprio (a pasta observada e varrida a cada
/// 5 s), e as rotas de webhook para o `merge` do router. Tudo cria tarefa pela mesma
/// `criar_tarefa_com` da API.
fn armar_gatilhos(raiz: &std::path::Path, state: &ApiState) -> Result<axum::Router> {
    use phxclaw_agent::gatilhos;
    use phxclaw_agent::montagem::PastaConfiada;
    // So projeto confiado arma gatilho (a mesma confianca do `.phxclaw/config.json`): o
    // `gatilhos.json` de um clone dispararia trabalho com as credenciais do operador.
    let projeto = match phxclaw_agent::montagem::pasta_do_projeto_confiada(raiz) {
        PastaConfiada::Confiada(p) => Some(p),
        PastaConfiada::Ignorada { pasta, raiz: r } => {
            for a in ["gatilhos.json", "HEARTBEAT.md"] {
                if pasta.join(a).is_file() {
                    eprintln!(
                        "aviso: {}",
                        phxclaw_agent::config::ignorado_por_confianca(&pasta.join(a), &r)
                    );
                }
            }
            None
        }
        PastaConfiada::Nenhuma => None,
    };
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
    // Os polls (lista `polls` do mesmo gatilhos.json): um laco por poll, porque cada um faz
    // rede no proprio intervalo e um servico lento nao pode atrasar a pasta observada.
    let polls = match &projeto {
        Some(p) => phxclaw_agent::gatilho_poll::carregar(p).map_err(anyhow::Error::msg)?,
        None => vec![],
    };
    if !polls.is_empty() {
        println!(
            "gatilhos: {}",
            phxclaw_agent::gatilho_poll::descrever(&polls)
        );
    }
    for p in polls {
        let sondagem = phxclaw_agent::gatilho_poll::Sondagem::new(p, raiz);
        tokio::spawn(phxclaw_agent::gatilho_poll::laco(state.clone(), sondagem));
    }
    // As notificacoes de recurso MCP (lista `mcp`): uma assinatura viva por gatilho, so de
    // servidor do arquivo do operador. Sem esse arquivo, gatilho declarado e erro na subida,
    // nao uma assinatura que nunca ouve nada.
    let mcps = match &projeto {
        Some(p) => phxclaw_agent::gatilho_mcp::carregar(p).map_err(anyhow::Error::msg)?,
        None => vec![],
    };
    if !mcps.is_empty() {
        let chave = phxclaw_agent::mcp::CHAVE_CONFIG;
        let cfg = phxclaw_agent::config::caminho_de(chave).ok_or_else(|| {
            anyhow::anyhow!(
                "gatilhos mcp declarados sem {}",
                phxclaw_agent::config::variavel(chave)
            )
        })?;
        println!("gatilhos: {}", phxclaw_agent::gatilho_mcp::descrever(&mcps));
        for g in mcps {
            let a = phxclaw_agent::gatilho_mcp::Assinante::new(g, &cfg, raiz);
            tokio::spawn(phxclaw_agent::gatilho_mcp::laco(state.clone(), a));
        }
    }
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
        .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
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
        .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
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
        // `<pasta>/usuarios.json` (`phxclaw usuario`): sem ele, so o Bearer unico.
        usuarios: phxclaw_agent::rbac::Usuarios::da_pasta(raiz),
        store,
        factory,
        default_model: phxclaw_agent::config::texto_de("modelo.padrao")
            .unwrap_or_else(|| MODELO_PADRAO.into()),
        token,
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json"))?)),
        webhook_origins: phxclaw_agent::config::lista_de("api.webhook_origens").unwrap_or_default(),
        limite: Arc::new(phxclaw_agent::api::Limite::por_minuto(
            phxclaw_agent::config::inteiro_de("api.tarefas_por_minuto")
                .and_then(|v| u32::try_from(v).ok())
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
        &phxclaw_agent::config::por_variavel,
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
/// O Bearer de um servidor: a variavel do ambiente, senao o broker (`phxclaw api chave`,
/// `phxclaw ponte chave`), senao o arquivo da pasta, gerado na primeira vez.
fn token_de(
    raiz: &std::path::Path,
    arq: &std::path::Path,
    servico: &phxclaw_agent::chaves::Servico,
) -> Result<String> {
    Ok(match token_existente(raiz, arq, servico)? {
        Some(t) => t,
        None => {
            let t = phxclaw_api_gateway::generate_bearer_token();
            phxclaw_secret_broker::write_private_file(arq, t.as_bytes())?;
            t
        }
    })
}

/// O token que o servidor usaria, sem gerar um: ambiente, broker, arquivo. O `servir` gera
/// quando nao ha; o cliente (`phxclaw api`) nao, porque token novo do lado de quem chama
/// nao abre servidor nenhum.
fn token_existente(
    raiz: &std::path::Path,
    arq: &std::path::Path,
    servico: &phxclaw_agent::chaves::Servico,
) -> Result<Option<String>> {
    Ok(
        match servico
            .do_ambiente_ou_broker(raiz)
            .map_err(anyhow::Error::msg)?
        {
            Some(t) if t.len() >= 24 => Some(t),
            Some(_) => bail!(
                "{} precisa de pelo menos 24 caracteres",
                servico.variaveis()[0]
            ),
            None => std::fs::read_to_string(arq)
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty()),
        },
    )
}

/// `phxclaw <servico> chave`: a variavel do ambiente do comando vai para o broker da pasta.
fn chave_de(servico: &phxclaw_agent::chaves::Servico, args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("chave") {
        bail!("uso: phxclaw {} [--pasta DIR]", servico.comando);
    }
    let raiz = pasta(args);
    let id = servico
        .guardar_do_ambiente(&raiz)
        .map_err(anyhow::Error::msg)?;
    println!(
        "{} guardad{} no broker de {} (segredo {id}); vale quando {} nao esta no ambiente",
        servico.rotulo,
        if servico.rotulo.starts_with("a ") {
            "a"
        } else {
            "o"
        },
        servico.pasta(&raiz).display(),
        servico.variaveis().join(" / ")
    );
    Ok(())
}

/// `phxclaw plugins chave|assinar [DIR] [--raiz DIR]`: a semente de assinatura vai para o
/// broker; `assinar` reassina os manifestos de DIR com ela (ambiente primeiro, como o
/// exemplo `assinar` do registro, pelo mesmo `reassinar_pasta`).
fn plugins(args: &[String]) -> Result<()> {
    use phxclaw_agent::chaves::ASSINATURA_DE_PLUGIN;
    match args.first().map(String::as_str) {
        Some("chave") => chave_de(&ASSINATURA_DE_PLUGIN, args),
        Some("assinar") => {
            let raiz_do_agente = pasta(args);
            let raiz = opcao(args, "--raiz")
                .map(PathBuf::from)
                .unwrap_or(env::current_dir()?);
            let dir = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(|| raiz.join("plugins/builtin/manifests"));
            let semente = ASSINATURA_DE_PLUGIN
                .do_ambiente_ou_broker(&raiz_do_agente)
                .map_err(anyhow::Error::msg)?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{} nao esta no ambiente nem no broker: rode `phxclaw {}`",
                        ASSINATURA_DE_PLUGIN.variaveis()[0],
                        ASSINATURA_DE_PLUGIN.comando
                    )
                })?;
            let feitos =
                phxclaw_plugin_registry::assinatura::reassinar_pasta(&raiz, &dir, &semente)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            for p in &feitos {
                println!("reassinado {}", p.display());
            }
            println!("{} manifesto(s)", feitos.len());
            Ok(())
        }
        // A loja: a MESMA `Loja` da ferramenta `plugin_catalog`, para o terminal e o modelo
        // nao terem duas regras sobre o que e um pacote instalavel.
        Some("catalogo" | "catalog") => {
            let loja = loja()?;
            let busca = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map(String::as_str);
            runtime()?.block_on(async {
                let idx = loja.indice().await.map_err(anyhow::Error::msg)?;
                for r in loja.listar(&idx, busca) {
                    println!("{}", serde_json::to_string(&Loja::ficha(r))?);
                }
                Ok::<(), anyhow::Error>(())
            })
        }
        Some("instalar" | "install") => {
            let nome = args.get(1).context(USO_PLUGINS)?;
            let i = runtime()?
                .block_on(loja()?.instalar(nome))
                .map_err(anyhow::Error::msg)?;
            println!(
                "{} {} instalado em {}",
                i.nome,
                i.versao,
                i.caminho.display()
            );
            Ok(())
        }
        Some("empacotar" | "pack") => {
            let (dir, saida) = (
                args.get(1).context(USO_PLUGINS)?,
                args.get(2).context(USO_PLUGINS)?,
            );
            let bytes =
                phxclaw_agent::loja::empacotar(Path::new(dir)).map_err(anyhow::Error::msg)?;
            std::fs::write(saida, &bytes)?;
            println!(
                "{saida}: {} bytes (sha256 {})",
                bytes.len(),
                phxclaw_agent::loja::sha256_hex(&bytes)
            );
            Ok(())
        }
        _ => bail!("{USO_PLUGINS}"),
    }
}

const USO_PLUGINS: &str = "uso: phxclaw plugins chave|assinar [DIR] [--raiz DIR] [--pasta DIR] | \
                           catalogo [BUSCA] | instalar NOME | empacotar DIR SAIDA.tar";

/// A loja de `pacotes.catalogo`; sem catalogo configurado, o erro diz a chave.
fn loja() -> Result<Loja> {
    Loja::da_configuracao()
        .map_err(anyhow::Error::msg)?
        .context("pacotes.catalogo (PHXCLAW_PACOTES_CATALOGO) nao definido: nao ha loja")
}

/// Como o agente se liga a ponte. O token de pareamento vem so do ambiente (segredo em
/// argumento aparece no `ps`); o no ganha um UUID na primeira vez, guardado na pasta.
fn ponte_do_agente(
    args: &[String],
    raiz: &std::path::Path,
    url: String,
) -> Result<phxclaw_agent::remoto::ConfigDaPonte> {
    let uuid = |op: &str, chave: &str| -> Result<Option<phxclaw_agent::remoto::Uuid>> {
        Ok(opcao(args, op)
            .or_else(|| phxclaw_agent::config::texto_de(chave))
            .map(|v| v.parse())
            .transpose()?)
    };
    let tenant = uuid("--ponte-tenant", "dispositivos.tenant_uuid")?.with_context(|| {
        format!(
            "falta --ponte-tenant (ou {})",
            phxclaw_agent::config::variavel("dispositivos.tenant_uuid")
        )
    })?;
    let no = match uuid("--ponte-no", "dispositivos.no_uuid")? {
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
        token_pareamento: phxclaw_agent::chaves::PAREAMENTO
            .do_ambiente_ou_broker(raiz)
            .map_err(anyhow::Error::msg)?,
        pasta_da_chave: raiz.join("ponte-chave"),
    })
}

/// A ponte do controle remoto: o servidor WSS de dispositivos (onde o agente se liga, de
/// saida) e o HTTP do cliente (a tela instalavel e o rele das rotas de tarefa).
async fn ponte(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) == Some("chave") {
        return chave_de(&phxclaw_agent::chaves::PONTE, args);
    }
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
    let token = token_de(
        &raiz,
        &raiz.join("ponte.token"),
        &phxclaw_agent::chaves::PONTE,
    )?;
    let host = phxclaw_agent::config::texto_de("ponte.host").unwrap_or_else(|| "127.0.0.1".into());
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
    if args.first().map(String::as_str) == Some("chave") {
        return chave_de(&phxclaw_agent::chaves::PAREAMENTO, args);
    }
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
    let host =
        phxclaw_agent::config::texto_de("dispositivos.host").unwrap_or_else(|| "127.0.0.1".into());
    let l = tokio::net::TcpListener::bind((host.as_str(), porta)).await?;
    println!(
        "dispositivos em wss://{} ({n} token(s) de pareamento)",
        l.local_addr()?
    );
    tokio::spawn(srv.clone().servir(l, tls));
    Ok(srv)
}

/// `phxclaw fluxo exportar|importar --ambiente dev|prod`: o git dos fluxos (`fluxo_git`).
/// Porta fina: a pasta de fluxos e o repositorio sao opcoes, e `--commit MSG` registra pelo
/// `GitTool` de escrita do agente (o mesmo sandbox e a mesma varredura de segredos).
async fn fluxo_git(args: &[String], exportar: bool) -> Result<()> {
    use phxclaw_agent::fluxo_git::{self, Ambiente, Efeito};
    let amb = Ambiente::ler(&opcao(args, "--ambiente").unwrap_or_default())
        .map_err(anyhow::Error::msg)?;
    let fluxos_dir = opcao(args, "--fluxos")
        .map(PathBuf::from)
        .unwrap_or_else(|| pasta(args).join("fluxos"));
    let repo = opcao(args, "--repo")
        .map(PathBuf::from)
        .unwrap_or_else(|| pasta(args).join(fluxo_git::PASTA_PADRAO));
    if !exportar {
        let feitos = fluxo_git::importar(
            &repo,
            amb,
            &fluxos_dir,
            args.iter().any(|a| a == "--sobrescrever"),
        )
        .map_err(anyhow::Error::msg)?;
        for f in &feitos {
            let efeito = match f.efeito {
                Efeito::Novo => "novo",
                Efeito::Atualizado => "atualizado",
                Efeito::Igual => "igual",
            };
            let versao = f
                .versao
                .map(|v| format!(" (publicada v{v})"))
                .unwrap_or_default();
            println!("  {efeito:<10} {}{versao}  {}", f.nome, f.destino.display());
        }
        println!("{} fluxo(s) do ambiente {}", feitos.len(), amb.nome());
        return Ok(());
    }
    let r = fluxo_git::exportar(&fluxos_dir, &repo, amb).map_err(anyhow::Error::msg)?;
    for e in &r.escritos {
        let versao = e.versao.map(|v| format!(" v{v}")).unwrap_or_default();
        println!(
            "  {} {}{versao}  {}",
            if e.mudou { "gravado" } else { "igual  " },
            e.nome,
            e.arquivo.display()
        );
    }
    for x in &r.removidos {
        println!("  removido {}", x.display());
    }
    println!(
        "{} fluxo(s) em {}/{}",
        r.escritos.len(),
        repo.display(),
        amb.nome()
    );
    if let Some(msg) = opcao(args, "--commit") {
        let bwrap = phxclaw_agent::arquivos::achar_bwrap()
            .ok_or_else(|| anyhow::anyhow!("sem bwrap o git do agente nao roda"))?;
        let g = phxclaw_agent::git::GitTool::escrita(bwrap);
        let v = fluxo_git::registrar(&g, &repo, &msg)
            .await
            .map_err(anyhow::Error::msg)?;
        println!("{v}");
    }
    Ok(())
}

/// `phxclaw fluxo rodar ARQ`: o fluxo declarativo pelo motor do agente
/// (`phxclaw_agent::fluxos`), o mesmo que qualquer outra entrada usaria. Os subcomandos de
/// gestao (onda 3 e 4 da SP000035) sao porta fina para as funcoes do motor: nenhum decide
/// nada que a API ou o servidor decidam de outro jeito.
async fn fluxo(args: &[String]) -> Result<()> {
    use phxclaw_agent::fluxos;
    const USO: &str = "uso: phxclaw fluxo rodar ARQ.json [--ate PASSO] [--pins] | retomar TAREFA [ARQ.json] \
| responder TAREFA TEXTO | esperas | pinar ARQ.json PASSO (--json VALOR | --tarefa T) | despinar \
ARQ.json PASSO | podar [--dias N] [--max N] | exportar ARQ.json [--saida PACOTE.json] | importar \
PACOTE.json DESTINO.json | listar [DIR] [--etiqueta E] [--subpasta P] | modelos | usar MODELO \
DESTINO.json | publicar ARQ.json [--nota T] | versoes ARQ.json | voltar ARQ.json N | restaurar \
ARQ.json N [--forcar] | exportar --ambiente dev|prod [--fluxos DIR] [--repo DIR] [--commit MSG] | \
importar --ambiente dev|prod [--fluxos DIR] [--repo DIR] [--sobrescrever]  [--modelo M] [--pasta DIR]";
    let sub = args.first().map(String::as_str).unwrap_or_default();
    let posicional = |i: usize| -> Option<&String> { args.get(i).filter(|a| !a.starts_with("--")) };
    match sub {
        "pinar" | "pin" | "despinar" | "unpin" => {
            let (Some(arq), Some(passo)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            let valor = if matches!(sub, "despinar" | "unpin") {
                None
            } else if let Some(j) = opcao(args, "--json") {
                Some(serde_json::from_str(&j).map_err(|e| anyhow::anyhow!("--json: {e}"))?)
            } else if let Some(t) = opcao(args, "--tarefa") {
                let store = TaskStore::new(pasta(args).join("tasks"))?;
                Some(fluxos::saida_para_pin(&store, &t, passo).map_err(anyhow::Error::msg)?)
            } else {
                bail!("{USO}");
            };
            let pinou = valor.is_some();
            fluxos::pinar(Path::new(arq), passo, valor).map_err(anyhow::Error::msg)?;
            println!(
                "{} {passo} em {}",
                if pinou { "pinado" } else { "despinado" },
                fluxos::arquivo_de_pins(Path::new(arq)).display()
            );
            return Ok(());
        }
        "podar" | "prune" => {
            let num = |k: &str| -> Result<Option<u64>> {
                opcao(args, k)
                    .map(|v| v.parse::<u64>())
                    .transpose()
                    .map_err(Into::into)
            };
            let mut poda = fluxos::Poda::do_config();
            if let Some(d) = num("--dias")? {
                poda.dias = Some(d).filter(|d| *d > 0);
            }
            if let Some(m) = num("--max")? {
                poda.max = usize::try_from(m).ok().filter(|m| *m > 0);
            }
            if !poda.ligada() {
                println!(
                    "poda desligada (fluxos.poda_dias e fluxos.poda_max vazios): nada apagado"
                );
                return Ok(());
            }
            let store = TaskStore::new(pasta(args).join("tasks"))?;
            let p = fluxos::podar(&store, poda, fluxos::agora());
            println!("{} tarefa(s) apagada(s)", p.removidas.len());
            for e in &p.erros {
                eprintln!("  erro: {e}");
            }
            if !p.erros.is_empty() {
                std::process::exit(2);
            }
            return Ok(());
        }
        "modelos" | "templates" => {
            let galeria = phxclaw_agent::fluxo_modelos::galeria().map_err(anyhow::Error::msg)?;
            for m in &galeria {
                println!(
                    "  {:<32} [{}]  credenciais: {}",
                    m.nome,
                    m.etiquetas.join(", "),
                    if m.credenciais.is_empty() {
                        "nenhuma".to_string()
                    } else {
                        m.credenciais.join(", ")
                    }
                );
                println!("      {}", m.descricao);
            }
            println!(
                "{} modelo(s); copie um com: fluxo usar MODELO DESTINO.json",
                galeria.len()
            );
            return Ok(());
        }
        "usar" | "use" => {
            let (Some(modelo), Some(destino)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            let m = phxclaw_agent::fluxo_modelos::usar(modelo, Path::new(destino))
                .map_err(anyhow::Error::msg)?;
            println!("fluxo {} gravado em {destino} (rascunho)", m.nome);
            if !m.credenciais.is_empty() {
                println!(
                    "guarde antes de rodar (so o nome vem no modelo): {}",
                    m.credenciais.join(", ")
                );
            }
            return Ok(());
        }
        "publicar" | "publish" => {
            let Some(arq) = posicional(1) else {
                bail!("{USO}")
            };
            let nota = opcao(args, "--nota").unwrap_or_default();
            let v = phxclaw_agent::fluxo_versoes::publicar(Path::new(arq), &nota)
                .map_err(anyhow::Error::msg)?;
            println!("fluxo publicado: v{} (sha256 {})", v.numero, v.sha256);
            return Ok(());
        }
        "versoes" | "versions" => {
            use phxclaw_agent::fluxo_versoes;
            let Some(arq) = posicional(1) else {
                bail!("{USO}")
            };
            let s = fluxo_versoes::situacao(Path::new(arq)).map_err(anyhow::Error::msg)?;
            match &s.indice {
                None => println!("nunca publicado: o arquivo e o publicado implicito"),
                Some(i) => {
                    for v in &i.versoes {
                        println!(
                            "  v{:<4} {} {}{}{}",
                            v.numero,
                            &v.sha256[..12.min(v.sha256.len())],
                            v.em,
                            if v.numero == i.publicada {
                                "  <- publicada"
                            } else {
                                ""
                            },
                            if v.nota.is_empty() {
                                String::new()
                            } else {
                                format!("  ({})", v.nota)
                            }
                        );
                    }
                }
            }
            match &s.rascunho {
                Ok(sha) => println!(
                    "rascunho {}{}",
                    &sha[..12.min(sha.len())],
                    if s.rascunho_alterado() {
                        "  (difere da publicada)"
                    } else {
                        ""
                    }
                ),
                Err(e) => println!("rascunho nao le: {e}"),
            }
            return Ok(());
        }
        "voltar" | "rollback" => {
            let (Some(arq), Some(n)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            let n: u32 = n.parse().map_err(|_| anyhow::anyhow!("{USO}"))?;
            let v = phxclaw_agent::fluxo_versoes::voltar(Path::new(arq), n)
                .map_err(anyhow::Error::msg)?;
            println!("publicada: v{} (a definicao da v{n})", v.numero);
            return Ok(());
        }
        "restaurar" | "restore" => {
            let (Some(arq), Some(n)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            let n: u32 = n.parse().map_err(|_| anyhow::anyhow!("{USO}"))?;
            phxclaw_agent::fluxo_versoes::restaurar_rascunho(
                Path::new(arq),
                n,
                args.iter().any(|a| a == "--forcar"),
            )
            .map_err(anyhow::Error::msg)?;
            println!("rascunho {arq} restaurado da v{n}");
            return Ok(());
        }
        "exportar" | "export" if opcao(args, "--ambiente").is_some() => {
            return fluxo_git(args, true).await;
        }
        "importar" | "import" if opcao(args, "--ambiente").is_some() => {
            return fluxo_git(args, false).await;
        }
        "exportar" | "export" => {
            let Some(arq) = posicional(1) else {
                bail!("{USO}")
            };
            let f = fluxos::ler_arquivo(Path::new(arq)).map_err(anyhow::Error::msg)?;
            let pacote = serde_json::to_string_pretty(&fluxos::exportar(&f))?;
            match opcao(args, "--saida") {
                Some(s) => {
                    std::fs::write(&s, pacote)?;
                    println!("pacote do fluxo {} em {s}", f.nome);
                }
                None => println!("{pacote}"),
            }
            return Ok(());
        }
        "importar" | "import" => {
            let (Some(pacote), Some(destino)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            let f =
                fluxos::importar(&std::fs::read_to_string(pacote)?).map_err(anyhow::Error::msg)?;
            fluxos::gravar_importado(&f, Path::new(destino)).map_err(anyhow::Error::msg)?;
            println!("fluxo {} importado em {destino}", f.nome);
            return Ok(());
        }
        "listar" | "list" => {
            let dir = posicional(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| pasta(args).join("fluxos"));
            let (achados, erros) = fluxos::listar(
                &dir,
                opcao(args, "--etiqueta").as_deref(),
                opcao(args, "--subpasta").as_deref(),
            );
            for f in &achados {
                println!(
                    "  {:<24} {:<12} [{}]  {}",
                    f.nome,
                    f.pasta,
                    f.etiquetas.join(", "),
                    f.arquivo.display()
                );
            }
            for e in &erros {
                eprintln!("  invalido: {e}");
            }
            println!("{} fluxo(s)", achados.len());
            return Ok(());
        }
        _ => {}
    }
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let store = TaskStore::new(pasta(args).join("tasks"))?;
    let agente = || {
        Montagem::new(store.clone())
            .agent(&modelo)
            .map_err(anyhow::Error::msg)
    };
    let r = match sub {
        "rodar" | "run" => {
            let Some(arq) = posicional(1) else {
                bail!("{USO}")
            };
            // Sem `--publicada` a execucao manual le o RASCUNHO (e para testar a edicao);
            // gatilho, agenda e sub-fluxo ja rodam a publicada.
            let f = if args.iter().any(|a| a == "--publicada") {
                phxclaw_agent::fluxo_versoes::ler_publicado(Path::new(arq))
            } else {
                fluxos::ler_arquivo(Path::new(arq))
            }
            .map_err(anyhow::Error::msg)?;
            println!(
                "fluxo {} ({} passos), modelo {modelo}",
                f.nome,
                f.passos.len()
            );
            // `--ate PASSO`: so ate o passo (inclusive), com o progresso gravado como a
            // retomada ja grava -- `retomar` continua dali.
            fluxos::rodar_com(
                &agente()?,
                &f,
                fluxos::Execucao {
                    ate: opcao(args, "--ate").as_deref(),
                    // O pin so vale na execucao manual que o pede: sem `--pins`, o passo
                    // pinado roda de verdade, como em gatilho, agenda e sub-fluxo.
                    pins: args.iter().any(|a| a == "--pins"),
                    ..fluxos::Execucao::default()
                },
            )
            .await
        }
        "retomar" | "resume" => {
            let Some(t) = posicional(1) else {
                bail!("{USO}")
            };
            if opcao(args, "--ate").is_some() {
                bail!("--ate so vale no rodar; retomar continua de onde o rodar parou");
            }
            match posicional(2) {
                Some(arq) => {
                    let f = fluxos::ler_arquivo(Path::new(arq)).map_err(anyhow::Error::msg)?;
                    fluxos::retomar(&agente()?, &f, t).await
                }
                // Sem o arquivo: a definicao que a espera gravou na pasta da tarefa.
                None => fluxos::retomar_do_disco(&agente()?, t)
                    .await
                    .map_err(String::from),
            }
        }
        "responder" | "answer" => {
            let (Some(t), Some(texto)) = (posicional(1), posicional(2)) else {
                bail!("{USO}");
            };
            fluxos::entregar(
                &store,
                t,
                fluxos::Via::Pergunta,
                None,
                vec![serde_json::json!({"resposta": texto.trim()})],
            )
            .map_err(anyhow::Error::msg)?;
            fluxos::retomar_do_disco(&agente()?, t)
                .await
                .map_err(String::from)
        }
        "esperas" | "waits" => {
            // O mesmo par do laco do servidor (`api::manter_fluxos`): as esperas de tempo
            // vencidas e as entregas gravadas que o processo caido nao retomou.
            let mut vencidas = fluxos::esperas_vencidas(&store, fluxos::agora());
            let entregues: Vec<String> = fluxos::entregas_sem_retomada(&store)
                .into_iter()
                .filter(|t| !vencidas.contains(t))
                .collect();
            println!(
                "{} espera(s) de tempo vencida(s), {} entrega(s) sem retomada",
                vencidas.len(),
                entregues.len()
            );
            vencidas.extend(entregues);
            for t in vencidas {
                match fluxos::retomar_do_disco(&agente()?, &t).await {
                    Ok(r) => println!("  {t}: {}", if r.sucesso { "ok" } else { "nao terminou" }),
                    Err(e) => eprintln!("  {t}: {e}"),
                }
            }
            return Ok(());
        }
        _ => bail!("{USO}"),
    }
    .map_err(anyhow::Error::msg)?;
    for p in &r.passos {
        let resumo: String = p.saida.lines().take(2).collect::<Vec<_>>().join(" | ");
        println!(
            "  {:<8} {}  {}",
            if p.reaproveitado {
                "retomado"
            } else if p.pinado {
                "pinado"
            } else {
                p.estado.as_str()
            },
            p.id,
            resumo.chars().take(140).collect::<String>()
        );
    }
    if let Some(q) = r
        .passos
        .iter()
        .any(|p| p.estado == "esperando")
        .then(|| store.load(&r.tarefa).ok().and_then(|t| t.question))
        .flatten()
    {
        println!("esperando: {q}");
        println!(
            "tarefa do fluxo: {} (responda com: fluxo responder {} TEXTO; tempo vencido: fluxo \
esperas)",
            r.tarefa, r.tarefa
        );
        return Ok(());
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

/// `phxclaw agenda`: a MESMA agenda do servidor (`agenda.json` da pasta) e o MESMO
/// `disparar_agenda` da API -- a CLI nao tem fila propria. `adicionar` aceita objetivo de
/// modelo ou `fluxo: ARQ.json`; `disparar` roda o que venceu e espera terminar.
async fn agenda(args: &[String]) -> Result<()> {
    const USO: &str = "uso: phxclaw agenda listar | adicionar NOME \"OBJETIVO\" (--cada SEGUNDOS | --cron \"m h d M w\") | disparar [--modelo M] [--pasta DIR]";
    let raiz = pasta(args);
    let sub = args.first().map(String::as_str).unwrap_or("listar");
    match sub {
        "listar" | "list" => {
            let a = Agenda::open(raiz.join("agenda.json"))?;
            if a.items.is_empty() {
                println!("agenda vazia ({})", a.path().display());
            }
            for s in &a.items {
                println!(
                    "{}  {}  proxima {}  {}  {}\n    {}",
                    s.id,
                    if s.enabled { "ligado " } else { "parado " },
                    s.next_run.format("%Y-%m-%d %H:%M:%SZ"),
                    s.name,
                    s.last_task
                        .as_deref()
                        .map(|t| format!("ultima tarefa {t}"))
                        .unwrap_or_default(),
                    s.objective
                );
            }
        }
        "adicionar" | "add" => {
            let (nome, objetivo) = (args.get(1).context(USO)?, args.get(2).context(USO)?);
            let spec = match (opcao(args, "--cada"), opcao(args, "--cron")) {
                (Some(seg), None) => phxclaw_agent::agenda::ScheduleSpec::EverySeconds(
                    seg.parse().context("--cada pede um numero de segundos")?,
                ),
                (None, Some(c)) => phxclaw_agent::agenda::ScheduleSpec::CronExpression(c),
                _ => bail!("{USO}"),
            };
            let mut a = Agenda::open(raiz.join("agenda.json"))?;
            // `fluxo: ARQ` no objetivo agenda o fluxo pelo campo explicito (o arquivo e lido
            // aqui: fluxo invalido para na mao de quem agenda, nao no disparo das 3h).
            let s = match phxclaw_agent::api::fluxo_do_objetivo(objetivo) {
                Some(arq) => a.adicionar_fluxo_agora(nome, arq, spec),
                None => a.adicionar_agora(nome, objetivo, spec),
            }
            .map_err(anyhow::Error::msg)?;
            println!(
                "agendado {} ({}): proxima execucao {}",
                s.id,
                s.name,
                s.next_run.format("%Y-%m-%d %H:%M:%SZ")
            );
        }
        "disparar" | "fire" => {
            let modelo = opcao(args, "--modelo")
                .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
                .unwrap_or_else(|| MODELO_PADRAO.into());
            let store = TaskStore::new(raiz.join("tasks"))?;
            let m = Montagem::new(store.clone());
            let factory: AgentFactory = Arc::new(move |modelo: &str| m.agent(modelo));
            let mut state = estado_api(
                &raiz,
                store,
                factory,
                phxclaw_api_gateway::generate_bearer_token(),
            )?;
            state.default_model = modelo;
            let handles = phxclaw_agent::api::disparar_agenda_com_handles(&state);
            println!("agenda: {} disparo(s)", handles.len());
            for h in handles {
                let _ = h.await;
            }
            for s in &state.agenda.lock().unwrap().items {
                if let Some(t) = &s.last_task {
                    println!("  {}  ultima tarefa {t}", s.name);
                }
            }
        }
        _ => bail!("{USO}"),
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
                .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
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
/// `phxclaw tarefa listar | rodar NOME`: as tarefas de `.phxclaw/tarefas.json` do projeto
/// (`PHXCLAW_PROJETO` ou a pasta corrente), pelo MESMO `carregar`/`rodar` da ferramenta
/// `project_task`. Sai com o codigo da tarefa.
async fn tarefa(args: &[String]) -> Result<()> {
    use phxclaw_agent::projeto_tarefas::{ARQUIVO, carregar, resultado, rodar};
    let raiz = phxclaw_agent::montagem::raiz_do_projeto()
        .ok_or_else(|| anyhow::anyhow!("nenhum projeto (PHXCLAW_PROJETO ou a pasta corrente)"))?;
    let lista = carregar(&raiz).map_err(anyhow::Error::msg)?;
    match args.first().map(String::as_str) {
        Some("listar") | Some("list") | None => {
            if lista.is_empty() {
                println!(
                    "sem .phxclaw/{ARQUIVO} em {}: valem rust_project e python_project",
                    raiz.display()
                );
            }
            for t in &lista {
                println!(
                    "{:<6} {:<24} {}",
                    t.grupo,
                    t.nome,
                    phxclaw_agent::projeto_tarefas::linha_de_shell(t)
                );
            }
            Ok(())
        }
        Some("rodar") | Some("run") => {
            let nome = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("uso: phxclaw tarefa rodar NOME"))?;
            let t = lista
                .iter()
                .find(|t| &t.nome == nome)
                .ok_or_else(|| anyhow::anyhow!("tarefa {nome:?} nao esta em .phxclaw/{ARQUIVO}"))?;
            let bwrap = phxclaw_agent::arquivos::achar_bwrap()
                .ok_or_else(|| anyhow::anyhow!("sem bwrap a tarefa nao roda"))?;
            let s = rodar(&bwrap, &raiz, t, std::time::Duration::from_secs(600))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", resultado(t, &s));
            if s.exit_code != Some(0) {
                std::process::exit(s.exit_code.unwrap_or(1));
            }
            Ok(())
        }
        Some(outra) => bail!("uso: phxclaw tarefa listar|rodar NOME (nao: {outra})"),
    }
}

/// `phxclaw testes listar|rodar NO [--projeto DIR] [--caminho REL] [--linguagem L]`: as
/// MESMAS funcoes das ferramentas `test_list`/`test_run` do agente (um explorador so),
/// sobre a pasta do projeto (`--projeto`, `PHXCLAW_PROJETO` ou a corrente).
async fn testes(args: &[String]) -> Result<()> {
    use phxclaw_agent::testes::ExploradorDeTestes;
    let uso = "uso: phxclaw testes listar | rodar NO  [--projeto DIR] [--caminho REL] [--linguagem rust|python]";
    let acao = args.first().map(String::as_str).unwrap_or("");
    let bwrap =
        phxclaw_agent::arquivos::achar_bwrap().context("sem bwrap: os testes rodam no sandbox")?;
    let e = ExploradorDeTestes::detectar(bwrap)
        .context("nem toolchain Rust nem Python no hospedeiro")?;
    let projeto = opcao(args, "--projeto")
        .map(PathBuf::from)
        .or_else(phxclaw_agent::montagem::raiz_do_projeto)
        .context("sem pasta de projeto")?;
    let ctx = phxclaw_agent::testes::contexto_da_cli(projeto, Duration::from_secs(900));
    let caminho = opcao(args, "--caminho").unwrap_or_else(|| ".".into());
    let linguagem = opcao(args, "--linguagem");
    let v = match acao {
        "listar" | "list" => e.listar(&ctx, &caminho, linguagem.as_deref()).await,
        "rodar" | "run" => {
            let no = args.get(1).filter(|n| !n.starts_with("--")).context(uso)?;
            e.rodar(&ctx, &caminho, no, linguagem.as_deref()).await
        }
        _ => bail!("{uso}"),
    }
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    if acao.starts_with('r') && v["passou"] != true {
        std::process::exit(1);
    }
    Ok(())
}

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
    let llm = modelo(
        &opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into()),
        &pasta(args),
    )
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

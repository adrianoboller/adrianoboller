//! `phxsqld` -- o servidor do PhxSql.
//!
//! ```text
//! phxsqld [--config <caminho>]     sobe o servidor (padrao: config.json)
//! phxsqld --exemplo <1|2|3>        imprime um config.json de exemplo
//! phxsqld --acessos [--config c]   mostra o log de acessos por IP
//! phxsqld --senha [senha]          gera a linha senha_hash para o config.json
//! phxsqld --gerar-chave             gera um par de chaves Ed25519
//! phxsqld --chave-do-fio           a chave publica do aperto de mao
//! phxsqld --pagina                  escreve o Centro de Controle num arquivo
//! phxsqld --usuarios [--config c]  lista o cadastro e o poder de cada um
//! phxsqld --bloqueios              lista os IPs bloqueados
//! phxsqld --desbloquear <ip>       tira um IP da lista
//! phxsqld --mcp                    servidor MCP pela entrada/saida padrao
//! phxsqld --empacotar-config       grava o config.json como config.phz
//! phxsqld --desempacotar-config    o config.phz volta a config.json
//! phxsqld --flag-desconhecida      recusa: codigo != 0, nomeia o que aceita
//! ```

use std::process::ExitCode;

use phxsql_server::{Config, LogAcessos, Servidor};

/// Uma flag que o `phxsqld` reconhece na linha de comando.
///
/// UM array so (pedido 483, lei «funcao e comando vem do mesmo motor»): o
/// parser confere `nome` contra ele em [`conferir_argumentos`], e o
/// `--help` monta o `USO:` lendo `uso` do MESMO lugar em [`montar_uso`].
/// Lista de flag digitada duas vezes e a lista que diverge de si mesma no
/// dia em que so um dos dois lados for atualizado.
struct Flag {
    /// O texto exato que aparece na linha de comando, com o(s) traco(s).
    nome: &'static str,
    /// Se o proximo argumento (quando ele NAO comeca com `-`) e o VALOR
    /// desta flag -- e por isso nao deve ser conferido como flag por si.
    toma_valor: bool,
    /// A linha (ou linhas) que descrevem a flag no `--help`. Vazio para as
    /// que nao ganham bullet propria -- `--usuario` e `--escrita` so fazem
    /// sentido junto de `--mcp`, e continuam descritas na prosa abaixo dele.
    uso: &'static str,
}

const FLAGS: &[Flag] = &[
    Flag {
        nome: "-V",
        toma_valor: false,
        uso: "",
    },
    Flag {
        nome: "--version",
        toma_valor: false,
        uso: "",
    },
    Flag {
        nome: "-h",
        toma_valor: false,
        uso: "",
    },
    Flag {
        nome: "--help",
        toma_valor: false,
        uso: "",
    },
    Flag {
        nome: "--config",
        toma_valor: true,
        uso: "  phxsqld [--config <caminho>]      sobe o servidor\n",
    },
    Flag {
        nome: "--acessos",
        toma_valor: false,
        uso: "  phxsqld --acessos [--config <c>]  mostra quem acessou, por IP\n",
    },
    Flag {
        nome: "--usuarios",
        toma_valor: false,
        uso: "  phxsqld --usuarios [--config <c>] lista o cadastro e o poder de cada um\n",
    },
    Flag {
        nome: "--bloqueios",
        toma_valor: false,
        uso: "  phxsqld --bloqueios [--config <c>]      lista os IPs bloqueados\n",
    },
    Flag {
        nome: "--desbloquear",
        toma_valor: true,
        uso: "  phxsqld --desbloquear <ip> [--config c] tira um IP da lista de bloqueio\n",
    },
    Flag {
        nome: "--senha",
        toma_valor: true,
        uso: "  phxsqld --senha [senha]           gera a linha senha_hash para o config.json\n",
    },
    Flag {
        nome: "--gerar-chave",
        toma_valor: false,
        uso: "  phxsqld --gerar-chave             gera um par de chaves Ed25519 (2o fator)\n",
    },
    Flag {
        nome: "--chave-do-fio",
        toma_valor: false,
        uso: "  phxsqld --chave-do-fio            a chave publica do aperto de mao (o pino)\n",
    },
    Flag {
        nome: "--pagina",
        toma_valor: false,
        uso: "  phxsqld --pagina > centro.html    o Centro de Controle como arquivo unico\n",
    },
    Flag {
        nome: "--exemplo",
        toma_valor: true,
        uso: "  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo\n                                    1 = isolado, 2 = source, 3 = replica\n",
    },
    Flag {
        nome: "--mcp",
        toma_valor: false,
        uso: "  phxsqld --mcp [--usuario u] [--escrita]   servidor MCP (JSON-RPC por linha,\n                                    pela entrada e pela saida padrao)\n",
    },
    Flag {
        nome: "--usuario",
        toma_valor: true,
        uso: "",
    },
    Flag {
        nome: "--escrita",
        toma_valor: false,
        uso: "",
    },
    Flag {
        nome: "--empacotar-config",
        toma_valor: false,
        uso: "  phxsqld --empacotar-config [--config <c>]\n                                    grava o config.json como config.phz (um 7z\n                                    com senha fixa: barreira contra editor, NAO\n                                    e cifra -- pedido 450). O .json sai do\n                                    caminho RENOMEADO, nunca apagado\n",
    },
    Flag {
        nome: "--desempacotar-config",
        toma_valor: false,
        uso: "  phxsqld --desempacotar-config [--config <c>]\n                                    o config.phz volta a config.json, para\n                                    editar o que a tela nao grava\n",
    },
];

/// Monta o texto do `--help`: cabecalho e prosa fixos, e a lista `USO:`
/// lida de [`FLAGS`] -- a MESMA lista que [`conferir_argumentos`] confere.
fn montar_uso() -> String {
    let mut s = String::from("phxsqld -- servidor do PhxSql (porta 5000 por padrao)\n\nUSO:\n");
    for f in FLAGS {
        s.push_str(f.uso);
    }
    s.push_str(AJUDA_RODAPE);
    s
}

const AJUDA_RODAPE: &str = "
O --mcp nasce SOMENTE LEITURA: do outro lado ha um modelo de linguagem, e nao
uma pessoa. --escrita libera phx_inserir e phx_atualizar. A senha do --usuario
vem de PHXSQL_SENHA, porque a entrada padrao esta ocupada pelo protocolo:

  PHXSQL_SENHA='a senha' phxsqld --mcp --usuario adriano

A senha NUNCA vai em texto puro no config.json. Gere o hash assim:

  phxsqld --senha                   pergunta a senha (ela aparece na tela)
  echo -n 'minha senha' | phxsqld --senha    nao aparece, nem no historico
";

/// A mensagem de erro nomeia o argumento e aponta a lista -- que sai do
/// MESMO array que validou, nunca de uma segunda lista digitada a mao.
///
/// `motivo` distingue a flag que nao existe (`--flag-boba`) do argumento que
/// sobrou sem nenhuma flag pedindo ele (`lixo` sozinho): o primeiro pode ter
/// vindo de um binario mais novo, o segundo e so um erro de digitacao.
fn erro_argumento(a: &str, motivo: &str) -> String {
    let conhecidas: Vec<&str> = FLAGS.iter().map(|f| f.nome).collect();
    format!(
        "{motivo}: {a}\n\
         use --help para o detalhe de cada uma, ou a lista curta:\n  {}",
        conhecidas.join(", ")
    )
}

/// Confere que a linha de comando so tem flags que o `phxsqld` conhece.
///
/// Os tres motores maduros (`postgres`, `mysqld`, `mariadbd`) recusam opcao
/// desconhecida saindo com erro; este binario so reagia as flags que
/// reconhecia e IGNORAVA o resto -- um binario velho recebendo uma flag nova
/// (`--empacotar-config`, no incidente que abriu o pedido 483) caiu direto no
/// arranque do servidor, usando o `--config` que sobrou. Roda ANTES de
/// qualquer leitura de config, chave ou porta: o erro tem de custar zero I/O.
fn conferir_argumentos(args: &[String]) -> Result<(), String> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if !a.starts_with('-') {
            // So chega aqui um argumento que NAO foi consumido como valor de
            // uma flag anterior -- sobrou, e nenhuma flag pediu ele.
            return Err(erro_argumento(a, "argumento inesperado"));
        }
        match FLAGS.iter().find(|f| f.nome == a) {
            None => return Err(erro_argumento(a, "argumento desconhecido")),
            Some(f) => {
                // O valor so e consumido se NAO parecer outra flag -- o
                // mesmo criterio que `--senha`, `--usuario` e `--desbloquear`
                // ja usam mais abaixo para saber se ganharam um valor.
                i += if f.toma_valor && args.get(i + 1).is_some_and(|v| !v.starts_with('-')) {
                    2
                } else {
                    1
                };
            }
        }
    }
    Ok(())
}

/// Gera a linha `senha_hash` para colar no `config.json`.
///
/// A senha e lida da entrada padrao para nao ficar no historico do shell nem
/// aparecer no `ps`. Passar como argumento funciona, mas e menos seguro.
/// Um par de chaves Ed25519.
///
/// A privada sai UMA vez, aqui, e o servidor nunca a ve. Se ela se perder,
/// nao ha como recuperar -- gera-se outra e troca-se a publica no
/// `config.json`. E o preco de o servidor nao guardar nada que assine.
fn gerar_chave() -> ExitCode {
    let privada = phxsql_core::ed25519::gerar_privada();
    let publica = phxsql_core::ed25519::chave_publica(&privada);
    println!("# Guarde a chave PRIVADA fora do servidor. Ela nao aparece de novo.");
    println!("chave_privada = {}", phxsql_core::hash::para_hex(&privada));
    println!();
    println!("# Esta linha vai no config.json, dentro do usuario:");
    println!(
        "\"chave_publica\": \"{}\"",
        phxsql_core::hash::para_hex(&publica)
    );
    ExitCode::SUCCESS
}

fn gerar_senha(args: &[String]) -> ExitCode {
    use std::io::{IsTerminal, Read};
    let i = args.iter().position(|a| a == "--senha").unwrap();
    let clara = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
        Some(a) => a.clone(),
        None => {
            if std::io::stdin().is_terminal() {
                eprintln!("Digite a senha e tecle Enter (ela vai aparecer na tela).");
                eprintln!("Para nao aparecer:  echo -n 'a senha' | phxsqld --senha");
            }
            let mut entrada = String::new();
            if std::io::stdin().read_to_string(&mut entrada).is_err() {
                eprintln!("nao consegui ler a senha da entrada padrao");
                return ExitCode::FAILURE;
            }
            entrada.trim_end_matches(['\n', '\r']).to_string()
        }
    };
    if clara.is_empty() {
        eprintln!("senha vazia; nada a gerar");
        return ExitCode::FAILURE;
    }
    // O mesmo teto do login (pedido 521): gerar o hash de uma senha que o
    // login recusaria entregaria uma linha de config.json que nunca entra.
    if let Err(e) = phxsql_core::senha::caber_no_teto(&clara) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    let hash = phxsql_core::senha::cifrar(&clara);
    println!("\"senha_hash\": \"{hash}\"");
    ExitCode::SUCCESS
}

/// `phxsqld --mcp`: o servidor MCP falando pela entrada e pela saída padrão.
///
/// # Somente leitura vem LIGADO
///
/// Do outro lado desta ponte há um modelo de linguagem, não uma pessoa.
/// `--escrita` libera `phx_inserir` e `phx_atualizar`, e é uma decisão de quem
/// monta o servidor -- não um padrão herdado.
///
/// # A senha não vem no argumento
///
/// Ela vem de `PHXSQL_SENHA`, pela mesma razão do `--senha`: argumento de linha
/// de comando aparece no `ps` e fica no histórico do shell. A entrada padrão
/// aqui está ocupada pelo JSON-RPC, então sobra a variável de ambiente.
///
/// Sem `--usuario`, a ponte fala pelo token de serviço -- que num servidor SEM
/// cadastro é poder total e num servidor COM cadastro não passa do `ping`. O
/// aviso sai no `stderr` para não sujar o cano do protocolo.
fn servir_mcp(args: &[String], config: phxsql_server::Config) -> ExitCode {
    use phxsql_server::mcp::Ponte;

    let com_escrita = args.iter().any(|a| a == "--escrita");
    let usuario = args
        .iter()
        .position(|a| a == "--usuario")
        .and_then(|i| args.get(i + 1))
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_default();
    let tem_cadastro = !config.cadastro.vazio();

    let servidor = match Servidor::novo(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nao consegui abrir os dados: {e}");
            return ExitCode::FAILURE;
        }
    };
    // "stdio" no lugar do IP: nao ha endereco, e o log de acessos tem de dizer
    // que a operacao veio da ponte. Leitura pelo MCP que nao deixa rastro
    // seria um buraco na auditoria justamente na origem mais nova.
    let executor = phxsql_server::servidor::ExecutorLocal::novo(servidor, "stdio");

    if !usuario.is_empty() {
        let senha = std::env::var("PHXSQL_SENHA").unwrap_or_default();
        if senha.is_empty() {
            eprintln!("--usuario {usuario} pede a senha em PHXSQL_SENHA");
            eprintln!("  PHXSQL_SENHA='a senha' phxsqld --mcp --usuario {usuario}");
            return ExitCode::FAILURE;
        }
        if let Err(e) = executor.entrar(&usuario, &senha) {
            eprintln!("login de {usuario} recusado: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("MCP em stdio, como {usuario}");
    } else {
        if tem_cadastro {
            eprintln!(
                "AVISO: sem --usuario, e este config.json TEM cadastro. A ponte fala pelo\n\
                 token de servico, e toda operacao que exige poder vai ser recusada."
            );
        }
        eprintln!("MCP em stdio, pelo token de servico");
    }
    eprintln!(
        "escrita: {}",
        if com_escrita {
            "LIBERADA (--escrita)"
        } else {
            "recusada (o padrao)"
        }
    );

    // O token e lido ANTES de a ponte tomar posse do executor: ele e o que a
    // ponte carimba em todo pedido, e carimbar e o que impede o modelo de
    // escolher a credencial.
    let token = phxsql_core::json::Json::texto_de(executor.token());
    let ponte = Ponte::nova(executor)
        .com_escrita(com_escrita)
        .com_campo_fixo("token", token);

    let entrada = std::io::stdin().lock();
    let mut saida = std::io::stdout().lock();
    match phxsql_server::mcp::servir(&ponte, entrada, &mut saida) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("a ponte MCP terminou: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--empacotar-config` e `--desempacotar-config`: o `config.json` vira
/// `config.phz` e volta (pedido 450).
///
/// O que se diz aqui e o que o operador precisa para agir: onde o original
/// ficou -- e que a copia em claro fica ate ele a apagar, porque o servidor
/// nao apaga arquivo de ninguem. A senha nunca aparece: nem aqui, nem no erro.
/// O arquivo que vale para o `--config` pedido, para o erro nomear o que foi
/// LIDO. Com os dois presentes, o proprio erro ja nomeia os dois.
fn resolvido(pedido: &str) -> String {
    phxsql_server::config_phz::resolver(std::path::Path::new(pedido))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| pedido.to_string())
}

fn trocar_a_forma(pedido: &str, empacotar: bool) -> ExitCode {
    use phxsql_server::config_phz::{desempacotar_arquivo, empacotar_arquivo, Troca};
    let alvo = std::path::Path::new(pedido);
    let r = if empacotar {
        empacotar_arquivo(alvo)
    } else {
        desempacotar_arquivo(alvo)
    };
    match r {
        Ok(Troca::JaEstava(real)) => {
            println!(
                "nada a fazer: {} ja esta {}",
                real.display(),
                if empacotar { "como .phz" } else { "em claro" }
            );
            ExitCode::SUCCESS
        }
        Ok(Troca::Feita { de, para, guardado }) if empacotar => {
            println!("{} empacotado em {} (0600).", de.display(), para.display());
            println!(
                "o original, EM CLARO, foi guardado como {} (0600).",
                guardado.display()
            );
            println!("suba o servidor como sempre (o mesmo --config serve) e, conferido,");
            println!("apague essa copia: ela carrega o token e os hashes, e o servidor nao apaga.");
            println!("o .phz e barreira contra editor, NAO e cifra (pedido 450).");
            ExitCode::SUCCESS
        }
        Ok(Troca::Feita { de, para, guardado }) => {
            println!("{} aberto em {} (0600).", de.display(), para.display());
            println!("o .phz foi guardado como {} (0600).", guardado.display());
            println!("o servidor passa a subir do .json EM CLARO, e o arranque avisa isso.");
            println!(
                "para empacotar de novo:  phxsqld --empacotar-config --config {}",
                para.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("a troca nao se completou em {}: {e}", resolvido(pedido));
            ExitCode::FAILURE
        }
    }
}

/// Tira a memoria do processo do `core` -- pedido 504.
///
/// # O que se mediu antes
///
/// Neste conteiner (`core_pattern` = `core`, limite duro `unlimited`), o
/// `phxsqld` com o cofre ligado subia com `coredump_filter` = `00000033`, e um
/// `SIGABRT` -- o sinal do `abort` da H5 do pedido 451 -- deixava um `core` de
/// 14,6 MB com a senha do cofre tres vezes. A chave de cada volume e
/// `PBKDF2(senha, sal)`, com o sal em claro no cabecalho: o `core` levava todas.
///
/// # Por que o filtro, e nao o `PR_SET_DUMPABLE`
///
/// O `prctl(PR_SET_DUMPABLE, 0)` tambem cala o `core`, mas faz o nucleo dar ao
/// `root` os arquivos de `/proc/self` -- e o servidor que roda como usuario
/// proprio perde o `/proc/self/io` (modo 0400) que a telemetria le. Medido
/// aqui como uid 65534: antes do `prctl` o arquivo abre, depois responde
/// `EACCES`. O limite
/// `RLIMIT_CORE` nao alcanca o `core_pattern` de `|programa` (systemd-coredump,
/// apport). O filtro vale para os dois destinos e so tira as REGIOES de
/// memoria: o `core` continua existindo, com os registradores e a lista de
/// mapeamentos, e sem heap, pilha nem arquivo mapeado.
///
/// Por padrao, e nao so com o cofre ligado: a memoria leva tambem a estatica
/// do fio, as credenciais do DbLink e a linha em claro. E o padrao dos maduros
/// -- o `core-file` do MySQL e do MariaDB nasce desligado. Quem precisa de um
/// `core` inteiro para depurar reescreve o filtro do processo vivo
/// (`echo 33 > /proc/<pid>/coredump_filter`), como quem liga o `core-file`.
///
/// Falhar aqui AVISA e segue: um `/proc` que recusa escrita e coisa do
/// conteiner, e derrubar o motor por isso seria o acessorio mandando no motor.
fn tirar_a_memoria_do_core() {
    #[cfg(target_os = "linux")]
    if let Err(e) = std::fs::write("/proc/self/coredump_filter", "0") {
        eprintln!(
            "AVISO: nao consegui escrever 0 em /proc/self/coredump_filter ({e}): um \
             abort deste processo pode levar ao disco a memoria dele, e nela a chave \
             do cofre. Ponha LimitCORE=0 na unidade do systemd (MANUAL.txt, 7.4)."
        );
    }
}

fn main() -> ExitCode {
    // ANTES de tudo, inclusive da leitura do `config.json`: a senha do cofre
    // entra na memoria no `Config::ler`, e o `--senha` le a do administrador.
    tirar_a_memoria_do_core();
    let args: Vec<String> = std::env::args().skip(1).collect();

    // A recusa vem ANTES de tudo, inclusive antes do -V/--help -- que ja
    // estao em FLAGS e por isso passam batido por aqui. Sem isto, um
    // binario velho que ganhasse uma flag nova a ignorava e caia direto no
    // arranque do servidor, sem porta aberta nem arquivo criado ainda
    // (pedido 483).
    if let Err(e) = conferir_argumentos(&args) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    // `--version` responde ANTES de tudo: sem ler `config.json`, sem conectar
    // em servidor nenhum. Uma auditoria externa achou os tres binarios
    // respondendo coisas diferentes -- «comando desconhecido», erro de
    // config ausente e erro de conexao recusada --, e perguntar a versao de um
    // binario e a PRIMEIRA linha de todo roteiro de operacao.
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("{}", phxsql_core::versao_completa("phxsqld"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", montar_uso());
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--senha") {
        return gerar_senha(&args);
    }

    if args.iter().any(|a| a == "--gerar-chave") {
        return gerar_chave();
    }

    // A pagina exatamente como o servidor a serve, para um arquivo.
    //
    // Sai da MESMA funcao que atende o navegador. Se saisse de outro lugar,
    // um dia o arquivo e a pagina servida diriam coisas diferentes -- e
    // ninguem descobriria ate alguem reclamar de um defeito que "aqui nao
    // acontece".
    if args.iter().any(|a| a == "--pagina") {
        use std::io::Write;
        let _ = std::io::stdout().write_all(phxsql_server::http::montar_pagina().as_bytes());
        return ExitCode::SUCCESS;
    }

    if let Some(i) = args.iter().position(|a| a == "--exemplo") {
        let qual = args.get(i + 1).map(String::as_str).unwrap_or("1");
        return match phxsql_server::config_exemplo(qual) {
            Some(texto) => {
                println!("{texto}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("exemplo desconhecido: {qual} (use 1, 2 ou 3)");
                ExitCode::FAILURE
            }
        };
    }

    let caminho = match args.iter().position(|a| a == "--config") {
        Some(i) => args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "config.json".to_string()),
        None => "config.json".to_string(),
    };

    // A troca de forma vem ANTES do `Config::ler` (revisao SEC de 24/09/2026,
    // M2): um `.phz` que abre e nao passa na validacao -- um campo torto, um
    // token vazio -- tem de poder sair pela ferramenta para ser consertado,
    // e depois do `Config::ler` ele nunca chegava aqui. O empacotar confere
    // que o texto e JSON; o resto da validacao e a do arranque seguinte.
    if args.iter().any(|a| a == "--empacotar-config") {
        return trocar_a_forma(&caminho, true);
    }
    if args.iter().any(|a| a == "--desempacotar-config") {
        return trocar_a_forma(&caminho, false);
    }

    let config = match Config::ler(&caminho) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("erro no {}: {e}", resolvido(&caminho));
            // A dica do modelo so quando NAO HA configuracao nenhuma. Com um
            // dos dois presentes -- o `.json` que o administrador extraiu do
            // `.phz` para editar, um `.phz` corrompido, um `.json` com uma
            // virgula a mais --, o `> config.json` dela TRUNCARIA o arquivo
            // que o erro acima esta nomeando (parecer do DBA, 24/09/2026).
            let (claro, phz) = phxsql_server::config_phz::par(std::path::Path::new(&caminho));
            if !claro.exists() && !phz.exists() {
                eprintln!(
                    "\ngere um modelo com:  phxsqld --exemplo 1 > {}",
                    claro.display()
                );
            }
            return ExitCode::FAILURE;
        }
    };

    // A forma do arquivo que subiu: em claro diz como empacotar; em `.phz`
    // diz se a copia em claro de uma migracao ainda esta ao lado -- achada
    // pelo PEDIDO, que e de onde a migracao tirou o nome dela.
    if let Some(aviso) = config.caminho.as_deref().and_then(|real| {
        phxsql_server::config_phz::aviso_de_arranque(std::path::Path::new(&caminho), real)
    }) {
        eprintln!("AVISO: {aviso}");
    }

    // Campo com nome errado nao derruba nada -- e por isso que precisa gritar.
    // O servidor sobe com o padrao e tudo PARECE certo ate alguem tentar
    // conectar na porta que nunca mudou.
    if !config.estranhas.is_empty() {
        eprintln!(
            "AVISO: {} nao reconhece {}: {}",
            caminho,
            if config.estranhas.len() == 1 {
                "o campo"
            } else {
                "os campos"
            },
            config.estranhas.join(", ")
        );
        eprintln!("       o valor foi IGNORADO. A porta de dados e \"bind\", nao \"porta\".");
    }
    // Valor reconhecido e ignorado grita pelo mesmo motivo do campo estranho:
    // um "idioma" escrito errado calaria em portugues para sempre.
    for aviso in &config.avisos {
        eprintln!("AVISO: {aviso}");
    }

    // A chave publica do aperto de mao, para o cliente PINAR.
    //
    // Cria a estatica se ela ainda nao existir -- e a mesma que o servidor vai
    // apresentar, porque sai do mesmo lugar. Imprimir uma chave calculada de
    // outro jeito seria o pior dos mundos: o operador pinaria uma coisa e o
    // servidor apresentaria outra.
    if args.iter().any(|a| a == "--chave-do-fio") {
        return match config.cifra_fio.estatica(config.caminho.as_deref()) {
            Ok((privada, avisos)) => {
                for a in avisos {
                    eprintln!("AVISO: {a}");
                }
                println!(
                    "{}",
                    phxsql_core::hash::para_hex(&phxsql_core::x25519::chave_publica(&privada))
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("nao consegui obter a chave do fio: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if args.iter().any(|a| a == "--mcp") {
        return servir_mcp(&args, config);
    }

    if args.iter().any(|a| a == "--acessos") {
        return match LogAcessos::resumo_por_ip(&config.log_acessos) {
            Ok(resumo) if resumo.is_empty() => {
                println!(
                    "nenhum acesso registrado em {}",
                    config.log_acessos.display()
                );
                ExitCode::SUCCESS
            }
            Ok(resumo) => {
                println!(
                    "{:<40} {:>8} {:>10}  {:<23}  {:<23}",
                    "ip", "acessos", "recusados", "primeiro", "ultimo"
                );
                for r in &resumo {
                    println!(
                        "{:<40} {:>8} {:>10}  {:<23}  {:<23}",
                        r.ip,
                        r.acessos,
                        r.recusados,
                        r.primeiro(),
                        r.ultimo()
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro ao ler o log de acessos: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if let Some(i) = args.iter().position(|a| a == "--desbloquear") {
        let ip = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
            Some(a) => a.clone(),
            None => {
                eprintln!("informe o IP: phxsqld --desbloquear 203.0.113.9");
                return ExitCode::FAILURE;
            }
        };
        return match phxsql_server::Blacklist::abrir(&config.blacklist)
            .and_then(|mut bl| bl.desbloquear(&ip, &config.politica))
        {
            Ok(true) => {
                println!("{ip} desbloqueado");
                ExitCode::SUCCESS
            }
            Ok(false) => {
                println!("{ip} nao estava na lista");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if args.iter().any(|a| a == "--bloqueios") {
        let bl = match phxsql_server::Blacklist::abrir(&config.blacklist) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("erro ao ler a lista: {e}");
                return ExitCode::FAILURE;
            }
        };
        let agora = phxsql_server::agora_ms();
        let ativos = bl.ativos(agora);
        if ativos.is_empty() {
            println!("nenhum IP bloqueado em {}", bl.caminho().display());
            return ExitCode::SUCCESS;
        }
        println!(
            "{:<40} {:<23} {:<23} {:>5}  {:<8}  motivo",
            "ip", "desde", "ate", "tent", "firewall"
        );
        for b in ativos {
            println!(
                "{:<40} {:<23} {:<23} {:>5}  {:<8}  {} ({})",
                b.ip,
                b.desde(),
                b.ate(),
                b.tentativas,
                if b.firewall { "sim" } else { "nao" },
                b.motivo,
                b.comando
            );
        }
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--usuarios") {
        let c = &config.cadastro;
        if c.vazio() {
            println!("nenhum usuario cadastrado neste config.json");
            println!("o token de servico e a unica credencial; qualquer pedido com ele");
            println!("tem poder total. Cadastre usuarios para restringir.");
            return ExitCode::SUCCESS;
        }
        for aviso in &c.avisos {
            eprintln!("AVISO: {aviso}");
        }
        println!(
            "{:<14} {:<24} {:<10} {:<7}  poder por base",
            "login", "nome", "nivel", "ativo"
        );
        let todos = c.root.iter().chain(c.usuarios.iter());
        for u in todos {
            let bases: Vec<String> = if u.supervisor {
                vec!["(supervisor: tudo em toda base)".to_string()]
            } else if u.bases.is_empty() {
                // Sem regra de base, quem manda e o nivel -- e a listagem tem
                // de dizer isso, senao "(nenhuma)" mente sobre quem pode ler.
                let podem: Vec<&str> = phxsql_server::Atividade::TODAS
                    .iter()
                    .filter(|a| u.nivel.permissoes().pode(**a))
                    .map(|a| a.nome())
                    .collect();
                vec![if podem.is_empty() {
                    "(nada, em base nenhuma)".to_string()
                } else {
                    format!("(pelo nivel, em toda base: {})", podem.join("+"))
                }]
            } else {
                u.bases
                    .iter()
                    .map(|(b, p)| {
                        let podem: Vec<&str> = phxsql_server::Atividade::TODAS
                            .iter()
                            .filter(|a| p.pode(**a))
                            .map(|a| a.nome())
                            .collect();
                        format!(
                            "{b}={}",
                            if podem.is_empty() {
                                "nada".to_string()
                            } else {
                                podem.join("+")
                            }
                        )
                    })
                    .collect()
            };
            println!(
                "{:<14} {:<24} {:<10} {:<7}  {}",
                u.login,
                u.nome,
                if u.supervisor {
                    "supervisor"
                } else {
                    u.nivel.nome()
                },
                if u.ativo { "sim" } else { "nao" },
                bases.join("  ")
            );
        }
        return ExitCode::SUCCESS;
    }

    for aviso in &config.cadastro.avisos {
        eprintln!("AVISO: {aviso}");
    }

    let servidor = match Servidor::novo(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nao consegui iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    match servidor.escutar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("servidor encerrado: {e}");
            ExitCode::FAILURE
        }
    }
}

//! Processo que o agente lanca por porta de OUTRA crate -- o Chromium do `phxclaw-browser`
//! e o servidor MCP do `phxclaw-mcp-lsp-runtime` -- nasce no MESMO bwrap do shell
//! (`workdir_sandbox_command_com`, a funcao do `shell`), e nao por um segundo sandbox.
//!
//! Medido em 02/10/2026 antes de decidir: o Chromium headless (headless_shell 141) sobe no
//! bwrap `--unshare-all` com `--share-net`, grava captura (`--screenshot`, rc 0) e responde
//! o CDP de fora (`/json/version` na porta que ele imprime); sem `--share-net` a porta nao
//! e alcancavel do hospedeiro. Precisa de `/etc/fonts` (sem ele: «Fontconfig error» e texto
//! sem fonte) e de `--disable-dev-shm-usage`. A rede entra porque e o que o navegador faz;
//! o ganho do sandbox e o sistema de arquivos (so `/usr`, `/work` e `/tmp` vazio: nem a
//! pasta do agente, nem o broker) e o ambiente limpo (nenhuma chave do processo pai).
//!
//! O servidor MCP por stdio ganha o mesmo: a pasta declarada (`cwd`) vira `/work`, o
//! ambiente e SO o que o operador declarou, e o executavel fora das pastas do sistema
//! entra so leitura pelo prefixo dele (`/opt/node22/bin/node` -> `/opt/node22`).

use crate::arquivos::achar_bwrap;
use phxclaw_mcp_lsp_runtime::{ProcessSecurityPolicy, ProcessSpec};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, workdir_sandbox_command_com};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Pastas que o sandbox ja monta so leitura (a lista e a do `phxclaw-sandbox`).
fn ja_visivel(p: &Path) -> bool {
    ["/usr", "/bin", "/lib", "/lib64", "/sbin"]
        .iter()
        .any(|d| p.starts_with(d))
}

/// Bind so leitura do prefixo de um executavel fora das pastas do sistema: `.../bin/x`
/// entra pelo pai de `bin` (as bibliotecas moram ao lado, em `lib`); o resto, pela pasta.
/// `None` quando o sandbox ja o ve.
pub fn bind_do_executavel(exe: &Path) -> Option<(PathBuf, String)> {
    if ja_visivel(exe) {
        return None;
    }
    let pai = exe.parent()?;
    let prefixo = if pai.file_name().is_some_and(|n| n == "bin") {
        pai.parent().unwrap_or(pai)
    } else {
        pai
    };
    Some((prefixo.to_path_buf(), prefixo.display().to_string()))
}

fn sem_bwrap(quem: &str) -> String {
    format!("sem bwrap: {quem} so roda no sandbox (instale bubblewrap)")
}

/// `exec` + argv entre aspas de shell: o programa SUBSTITUI o `/bin/sh -c` do sandbox, e
/// matar o filho mata o programa, nao um shell que o deixaria orfao.
fn script_exec(argv: &[String]) -> String {
    let mut s = String::from("exec");
    for a in argv {
        s.push(' ');
        s.push_str(&crate::python::aspas(a));
    }
    s
}

/// O `Command` do sandbox virando `ProcessSpec` + politica do runtime de MCP/LSP: o
/// executavel permitido e o bwrap, e o ambiente e o fixo do sandbox mais `extras.env`.
pub fn espec_no_bwrap(
    cmd: &WorkdirCommand,
    extras: &SandboxExtras,
) -> Result<(ProcessSpec, ProcessSecurityPolicy), String> {
    let bwrap = achar_bwrap().ok_or_else(|| sem_bwrap("o servidor"))?;
    let c = workdir_sandbox_command_com(&bwrap, cmd, extras).map_err(|e| e.to_string())?;
    let env: BTreeMap<String, String> = c
        .get_envs()
        .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
        .collect();
    let spec = ProcessSpec {
        executable: bwrap.clone(),
        args: c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect(),
        cwd: cmd.workdir.clone(),
        env: env.clone(),
    };
    let seguranca = ProcessSecurityPolicy {
        allowed_executables: [bwrap].into(),
        allowed_cwd_roots: vec![cmd.workdir.clone()],
        allowed_env_keys: env.keys().cloned().collect(),
        max_args: 512,
        ..ProcessSecurityPolicy::default()
    };
    Ok((spec, seguranca))
}

/// Caminho do hospedeiro que mora em `cwd` visto de dentro do bwrap, onde `cwd` e `/work`.
/// Vale para o executavel, para cada argumento e para cada valor de ambiente: o pacote
/// Claude/Codex passa o proprio arquivo ao servidor por `${CLAUDE_PLUGIN_ROOT}/srv.py`, e
/// remapear so o executavel deixava o argumento apontando para um caminho que nao existe
/// la dentro («process/stream closed», medido em `tests/orquestracao.rs`).
fn no_work(valor: &str, cwd: &Path) -> String {
    match Path::new(valor).strip_prefix(cwd) {
        Ok(rel) if Path::new(valor).is_absolute() => {
            // `cwd` ao pe da letra vira `/work`, nao `/work/`.
            Path::new("/work")
                .join(rel)
                .display()
                .to_string()
                .trim_end_matches('/')
                .to_string()
        }
        _ => valor.to_string(),
    }
}

/// O servidor MCP declarado pelo operador (`comando`, `args`, `cwd`, `env`) dentro do
/// bwrap: `cwd` vira `/work` (executavel, argumentos e ambiente que moram nela sao
/// remapeados), a rede fica ligada (servidor MCP e integracao com algo de fora) e o
/// ambiente e so o declarado.
pub fn servidor_no_bwrap(
    spec: &ProcessSpec,
) -> Result<(ProcessSpec, ProcessSecurityPolicy), String> {
    let mut argv = vec![no_work(&spec.executable.display().to_string(), &spec.cwd)];
    argv.extend(spec.args.iter().map(|a| no_work(a, &spec.cwd)));
    let cmd = WorkdirCommand {
        workdir: spec.cwd.clone(),
        script: script_exec(&argv),
        timeout: Duration::ZERO,
        network: true,
        max_output_bytes: 0,
    };
    let extras = SandboxExtras {
        ro_binds: bind_do_executavel(&spec.executable).into_iter().collect(),
        env: spec
            .env
            .iter()
            .map(|(k, v)| (k.clone(), no_work(v, &spec.cwd)))
            .collect(),
    };
    espec_no_bwrap(&cmd, &extras)
}

/// O Chromium dentro do bwrap, para `Browser::launch` e para o `--screenshot` do
/// `image_render`: a pasta do perfil e o `/work` (unica pasta gravavel), `/etc/fonts`
/// entra so leitura, a rede fica ligada (ver o cabecalho) e `--no-sandbox` vai junto
/// porque o bwrap JA e o sandbox de fora e o namespace de usuario aninhado que o
/// sandbox proprio do Chromium pede nao e garantido dentro dele.
pub fn envoltorio_do_navegador() -> Result<phxclaw_browser::Envoltorio, String> {
    let bwrap = achar_bwrap().ok_or_else(|| sem_bwrap("o navegador"))?;
    Ok(phxclaw_browser::Envoltorio(Arc::new(
        move |perfil: &Path, argv: Vec<String>| {
            let exe = PathBuf::from(argv.first().ok_or("argv vazio")?);
            let mut argv: Vec<String> = argv
                .into_iter()
                .map(|a| {
                    if a.starts_with("--user-data-dir=") {
                        "--user-data-dir=/work".into()
                    } else {
                        a
                    }
                })
                .collect();
            if !argv.iter().any(|a| a == "--no-sandbox") {
                argv.insert(1, "--no-sandbox".into());
            }
            let cmd = WorkdirCommand {
                workdir: perfil.to_path_buf(),
                script: script_exec(&argv),
                timeout: Duration::ZERO,
                network: true,
                max_output_bytes: 0,
            };
            let mut ro_binds: Vec<(PathBuf, String)> =
                bind_do_executavel(&exe).into_iter().collect();
            if Path::new("/etc/fonts").exists() {
                ro_binds.push(("/etc/fonts".into(), "/etc/fonts".into()));
            }
            workdir_sandbox_command_com(
                &bwrap,
                &cmd,
                &SandboxExtras {
                    ro_binds,
                    env: vec![],
                },
            )
            .map_err(|e| e.to_string())
        },
    )))
}

/// Registra no sandbox a mascara da pasta do agente: toda montagem de bwrap que CONTEM a
/// pasta (o projeto no layout padrao, uma raiz do workspace, o que vier) ganha um tmpfs
/// vazio e so leitura por cima dela. A pasta e a do `confine` (`tarefa::pasta_do_agente`),
/// e a excecao da pasta da tarefa (`<agente>/tasks/<id>/work`) e a mesma, pela forma: ela
/// mora dentro da do agente, nao a contem, e nao e mascarada. Chamada por `achar_bwrap`,
/// que e por onde todo processo do sandbox desta crate passa.
pub fn mascarar_a_pasta_do_agente() {
    phxclaw_sandbox::ocultar_com(|| crate::tarefa::pasta_do_agente().into_iter().collect());
}

/// Variaveis que o PTY (`alacritty_terminal`) poe no filho por conta propria, alem das que
/// o filho herda do agente; nenhuma delas pode chegar ao Helix.
const DO_PTY: &[&str] = &[
    "USER",
    "WINDOWID",
    "ALACRITTY_WINDOW_ID",
    "SHELL",
    "LOGNAME",
];

/// Onde a configuracao do Helix do hospedeiro aparece dentro do sandbox (`XDG_CONFIG_HOME`):
/// fora do `/work`, porque montar ali criaria pasta no projeto do usuario.
const CONFIG_NO_SANDBOX: &str = "/run/phxclaw/config";

/// O Helix do IDE no navegador dentro do MESMO bwrap dos outros processos: o projeto e o
/// `/work`, a pasta do agente mascarada (`mascarar_a_pasta_do_agente`), o ambiente so o
/// declarado, e as raizes do workspace montadas no proprio caminho, graváveis como o
/// projeto. Antes o hx rodava no hospedeiro, e `:open var/agente/segredos/master.key`
/// mostrava a chave-mestra na grade.
///
/// Tres diferencas do sandbox de sempre, cada uma com motivo:
/// - **Sem `--new-session`.** Com ele o hx perde o terminal de controle e o `SIGWINCH`
///   nunca chega (medido: redimensionar nao redesenha). O que a opcao protege (TIOCSTI
///   empurrando tecla para um shell de FORA) nao existe aqui: o PTY nasce para este hx, e
///   quem le a entrada dele e so o que esta dentro do sandbox.
/// - **Rede ligada.** A completacao por IA do snippet-ls fala com este agente em
///   `127.0.0.1`; e o mesmo que o servidor MCP.
/// - **O ambiente herdado e apagado com `--unsetenv`**, nao com `--clearenv` + `--setenv`:
///   o PTY passa ao filho o ambiente do agente inteiro, e `--setenv` poria o Bearer da
///   completacao no argv, que qualquer usuario local le em `/proc/<pid>/cmdline`. O valor
///   fica so no ambiente do bwrap (do mesmo usuario); o nome do que se apaga e publico, e
///   nome nao e segredo.
pub fn terminal_no_bwrap(
    p: phxclaw_terminal::Programa,
    raizes: &[PathBuf],
) -> Result<phxclaw_terminal::Programa, String> {
    let bwrap = achar_bwrap().ok_or_else(|| sem_bwrap("o terminal do IDE"))?;
    let cwd = p
        .cwd
        .clone()
        .ok_or("o terminal do IDE precisa da pasta do projeto")?;
    let exe = PathBuf::from(&p.programa);
    let mut argv = vec![no_work(&p.programa, &cwd)];
    argv.extend(p.args.iter().map(|a| no_work(a, &cwd)));
    let cmd = WorkdirCommand {
        workdir: cwd.clone(),
        script: script_exec(&argv),
        timeout: Duration::ZERO,
        network: true,
        max_output_bytes: 0,
    };
    let mut extras = SandboxExtras {
        ro_binds: bind_do_executavel(&exe).into_iter().collect(),
        env: p
            .env
            .iter()
            .map(|(k, v)| (k.clone(), no_work(v, &cwd)))
            .collect(),
    };
    // O runtime do Helix (gramaticas, temas) ao lado de um hx que nao o tem no prefixo.
    if let Some((_, r)) = extras.env.iter().find(|(k, _)| k == "HELIX_RUNTIME") {
        let r = PathBuf::from(r);
        if r.is_dir() && !ja_visivel(&r) && !extras.ro_binds.iter().any(|(h, _)| r.starts_with(h)) {
            extras.ro_binds.push((r.clone(), r.display().to_string()));
        }
    }
    // A configuracao do Helix do hospedeiro (o languages.toml do instalar_helix.sh liga o
    // snippet-ls), so leitura; cache, estado e dados no tmpfs, nunca no projeto.
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|c| c.join("helix"))
        .filter(|c| c.is_dir());
    if let Some(c) = config {
        extras
            .ro_binds
            .push((c, format!("{CONFIG_NO_SANDBOX}/helix")));
        extras
            .env
            .push(("XDG_CONFIG_HOME".into(), CONFIG_NO_SANDBOX.into()));
    }
    for (k, v) in [
        ("XDG_CACHE_HOME", "/tmp/cache"),
        ("XDG_STATE_HOME", "/tmp/state"),
        ("XDG_DATA_HOME", "/tmp/data"),
    ] {
        extras.env.push((k.into(), v.into()));
    }
    // Os servidores de linguagem que o hx sobe nascem DENTRO deste sandbox: os toolchains
    // entram pela montagem do `lsp.rs`, nao por uma segunda lista.
    for sv in crate::lsp::servidores_do_hospedeiro() {
        for b in sv.extras.ro_binds {
            if !extras.ro_binds.contains(&b) {
                extras.ro_binds.push(b);
            }
        }
        for (k, v) in sv.extras.env {
            if !extras.env.iter().any(|(j, _)| *j == k) {
                extras.env.push((k, v));
            }
        }
    }
    for r in raizes {
        extras.ro_binds.push((r.clone(), r.display().to_string()));
    }
    let c = workdir_sandbox_command_com(&bwrap, &cmd, &extras).map_err(|e| e.to_string())?;
    let env: Vec<(String, String)> = c
        .get_envs()
        .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
        .collect();
    let mut args: Vec<String> = c
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let fim = args.len().checked_sub(4).filter(|&i| args[i] == "--");
    let Some(mut fim) = fim else {
        return Err("sandbox sem o separador do comando".into());
    };
    if let Some(i) = args[..fim].iter().position(|a| a == "--new-session") {
        args.remove(i);
        fim -= 1;
    }
    // As raizes entraram como so leitura para cair na conta da mascara (que olha toda
    // montagem); viram graváveis aqui, como o projeto, que e o que o Helix fazia antes.
    let mut i = 0;
    while i + 2 < fim {
        if args[i] == "--ro-bind"
            && args[i + 1] == args[i + 2]
            && raizes
                .iter()
                .any(|r| r.display().to_string() == args[i + 1])
        {
            args[i] = "--bind".into();
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut apagar: Vec<String> = std::env::vars_os()
        .filter_map(|(k, _)| k.into_string().ok())
        .chain(DO_PTY.iter().map(|k| k.to_string()))
        .filter(|k| !env.iter().any(|(j, _)| j == k) && k != "TERM" && k != "COLORTERM")
        .collect();
    apagar.sort();
    apagar.dedup();
    let unset: Vec<String> = apagar
        .into_iter()
        .flat_map(|k| ["--unsetenv".to_string(), k])
        .collect();
    args.splice(fim..fim, unset);
    Ok(phxclaw_terminal::Programa {
        programa: bwrap.display().to_string(),
        args,
        cwd: Some(cwd),
        env,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn executavel_do_sistema_nao_pede_bind_e_o_de_fora_entra_pelo_prefixo() {
        assert_eq!(bind_do_executavel(Path::new("/usr/bin/python3")), None);
        assert_eq!(
            bind_do_executavel(Path::new("/opt/node22/bin/node")),
            Some(("/opt/node22".into(), "/opt/node22".to_string()))
        );
        assert_eq!(
            bind_do_executavel(Path::new("/opt/pw/chrome-linux/headless_shell")),
            Some((
                "/opt/pw/chrome-linux".into(),
                "/opt/pw/chrome-linux".to_string()
            ))
        );
    }

    #[test]
    fn servidor_na_pasta_declarada_e_remapeado_para_work() {
        let d = std::env::temp_dir().join(format!("phx-proc-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        let spec = ProcessSpec {
            executable: d.join("srv"),
            args: vec!["--a b".into(), d.join("srv.py").display().to_string()],
            cwd: d.clone(),
            env: [
                ("X".to_string(), "1".to_string()),
                ("RAIZ".to_string(), d.display().to_string()),
            ]
            .into(),
        };
        let (s, p) = match servidor_no_bwrap(&spec) {
            Ok(x) => x,
            Err(e) if e.starts_with("sem bwrap") => return,
            Err(e) => panic!("{e}"),
        };
        let script = s.args.last().unwrap();
        // Argumento e ambiente que apontam para dentro de `cwd` seguem o executavel.
        assert_eq!(script, "exec '/work/srv' '--a b' '/work/srv.py'");
        assert_eq!(s.env.get("RAIZ").map(String::as_str), Some("/work"));
        assert!(s.args.iter().any(|a| a == "--share-net"));
        assert_eq!(s.env.get("X").map(String::as_str), Some("1"));
        assert_eq!(s.env.get("HOME").map(String::as_str), Some("/work"));
        assert!(p.allowed_executables.iter().all(|e| e.ends_with("bwrap")));
        let _ = std::fs::remove_dir_all(&d);
    }
}

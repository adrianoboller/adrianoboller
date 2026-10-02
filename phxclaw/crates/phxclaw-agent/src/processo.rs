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

//! Pacotes de plugin no formato do Claude Code (`.claude-plugin/plugin.json`) e do Codex
//! (`.codex-plugin/plugin.json`): uma pasta com skills, hooks, servidores MCP, subagentes
//! e comandos, nos lugares padrao ou nos caminhos que o manifesto disser.
//!
//! Assinatura Ed25519 OBRIGATORIA, sobre a pasta inteira (`phxclaw_plugin_registry::pasta`,
//! o mesmo trust store dos manifestos): pacote sem assinatura, com signatario fora do
//! trust store ou com qualquer arquivo mexido depois de assinado nao carrega nada. Nada de
//! WASM nem biblioteca dinamica: o que executa sao os processos que os proprios
//! subsistemas ja sabem isolar.
//!
//! O que entra no agente hoje: os servidores MCP do pacote, pela MESMA subida do
//! `PHXCLAW_MCP_CONFIG` (`mcp::carregar_config`), com `${CLAUDE_PLUGIN_ROOT}` trocado pela
//! raiz do pacote. Skills, hooks, subagentes e comandos sao lidos e listados, mas ainda nao
//! se somam aos do projeto: cada um desses subsistemas tem hoje UMA fonte (uma pasta de
//! skills, um `hooks.json` montado em `/hooks`), e somar uma segunda pede mexer neles --
//! fica dito aqui em vez de parecer feito.

use crate::mcp::{ConfigMcp, ServidorDeclarado};
use phxclaw_agent_core::Tool;
use phxclaw_plugin_registry::TrustStore;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Pacote {
    pub nome: String,
    pub versao: String,
    /// `claude` ou `codex`.
    pub formato: &'static str,
    pub raiz: PathBuf,
    pub skills: Vec<String>,
    pub agentes: Vec<String>,
    pub comandos: Vec<String>,
    /// O `hooks.json` do pacote (eventos do Claude Code), lido mas nao ligado.
    pub hooks: Option<Value>,
    pub mcp: ConfigMcp,
}

const FORMATOS: [(&str, &str); 2] = [(".claude-plugin", "claude"), (".codex-plugin", "codex")];

fn dentro(raiz: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = raiz.join(rel.trim_start_matches("./"));
    let c = std::fs::canonicalize(&p).map_err(|e| format!("{rel}: {e}"))?;
    if !c.starts_with(raiz) {
        return Err(format!("{rel} aponta para fora do pacote"));
    }
    Ok(c)
}

/// Caminho do componente: o que o manifesto disser (texto), senao o padrao, se existir.
fn componente(
    raiz: &Path,
    m: &Value,
    campo: &str,
    padrao: &str,
) -> Result<Option<PathBuf>, String> {
    match m.get(campo).and_then(Value::as_str) {
        Some(rel) => dentro(raiz, rel).map(Some),
        None if raiz.join(padrao).exists() => dentro(raiz, padrao).map(Some),
        None => Ok(None),
    }
}

fn nomes(dir: Option<PathBuf>, so_pastas_com: Option<&str>, ext: Option<&str>) -> Vec<String> {
    let Some(dir) = dir else { return vec![] };
    let mut v: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            match (so_pastas_com, ext) {
                (Some(f), _) if p.join(f).is_file() => Some(n),
                (None, Some(x)) if p.is_file() && n.ends_with(x) => {
                    Some(n.trim_end_matches(x).to_string())
                }
                _ => None,
            }
        })
        .collect();
    v.sort();
    v
}

fn expandir(t: &str, raiz: &Path) -> String {
    let r = raiz.display().to_string();
    t.replace("${CLAUDE_PLUGIN_ROOT}", &r)
        .replace("${PLUGIN_ROOT}", &r)
}

/// `{"mcpServers": {nome: {command, args, env, url}}}` (ou o mapa direto) para o
/// formato declarado do agente, com a raiz do pacote como pasta de trabalho.
fn traduzir_mcp(v: &Value, raiz: &Path) -> Result<ConfigMcp, String> {
    let mapa = v
        .get("mcpServers")
        .unwrap_or(v)
        .as_object()
        .ok_or("mcpServers precisa ser um objeto")?;
    let mut servidores = Vec::new();
    for (nome, s) in mapa {
        let txt = |c: &str| s.get(c).and_then(Value::as_str).map(|x| expandir(x, raiz));
        let args = s
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|x| expandir(x, raiz))
                    .collect()
            })
            .unwrap_or_default();
        let env: BTreeMap<String, String> = s
            .get("env")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, x)| x.as_str().map(|x| (k.clone(), expandir(x, raiz))))
                    .collect()
            })
            .unwrap_or_default();
        servidores.push(ServidorDeclarado {
            nome: nome.clone(),
            comando: txt("command"),
            args,
            env,
            cwd: Some(raiz.to_path_buf()),
            url: txt("url"),
            prazo_inicio_ms: None,
            auth: None,
            preset: None,
        });
    }
    Ok(ConfigMcp { servidores })
}

fn ler_json(p: &Path) -> Result<Value, String> {
    serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Le e CONFERE um pacote. A assinatura vem antes de qualquer componente: nada de um
/// pacote adulterado e lido alem do nome.
pub fn ler(dir: &Path, trust: &TrustStore) -> Result<Pacote, String> {
    let raiz = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (pasta, formato) = FORMATOS
        .iter()
        .find(|(p, _)| raiz.join(p).join("plugin.json").is_file())
        .ok_or("sem .claude-plugin/plugin.json nem .codex-plugin/plugin.json")?;
    let m = ler_json(&raiz.join(pasta).join("plugin.json"))?;
    let nome = m
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| {
            !n.is_empty()
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
        .ok_or("plugin.json sem 'name' valido")?
        .to_string();
    phxclaw_plugin_registry::pasta::verificar_pasta(&raiz, &nome, trust)
        .map_err(|e| format!("assinatura do pacote {nome}: {e}"))?;
    let hooks = match m.get("hooks") {
        Some(Value::Object(_)) => m.get("hooks").cloned(),
        _ => componente(&raiz, &m, "hooks", "hooks/hooks.json")?
            .map(|p| ler_json(&p))
            .transpose()?,
    };
    let mcp = match m.get("mcpServers") {
        Some(v @ Value::Object(_)) => traduzir_mcp(v, &raiz)?,
        _ => match componente(&raiz, &m, "mcpServers", ".mcp.json")? {
            Some(p) => traduzir_mcp(&ler_json(&p)?, &raiz)?,
            None => ConfigMcp { servidores: vec![] },
        },
    };
    Ok(Pacote {
        versao: m
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0")
            .into(),
        formato,
        skills: nomes(
            componente(&raiz, &m, "skills", "skills")?,
            Some("SKILL.md"),
            None,
        ),
        agentes: nomes(
            componente(&raiz, &m, "agents", "agents")?,
            None,
            Some(".md"),
        ),
        comandos: nomes(
            componente(&raiz, &m, "commands", "commands")?,
            None,
            Some(".md"),
        ),
        hooks,
        mcp,
        nome,
        raiz,
    })
}

/// As ferramentas MCP do pacote, pela mesma subida do `PHXCLAW_MCP_CONFIG`.
pub fn ferramentas(p: &Pacote) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    crate::mcp::carregar_config(&p.mcp, &p.raiz)
}

/// `PHXCLAW_PACOTES_DIR`: cada subpasta e um pacote; o trust store e o dos plugins
/// (`PHXCLAW_PLUGIN_SIGNERS`). Sem trust store, nenhum pacote carrega -- assinatura e
/// obrigatoria, nao opcional quando der.
pub fn do_ambiente() -> Vec<Arc<dyn Tool>> {
    let Some(dir) = std::env::var_os("PHXCLAW_PACOTES_DIR").map(PathBuf::from) else {
        return vec![];
    };
    let trust = std::env::var_os("PHXCLAW_PLUGIN_SIGNERS")
        .ok_or_else(|| "PHXCLAW_PLUGIN_SIGNERS ausente".to_string())
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .and_then(|t| TrustStore::from_json(&t).map_err(|e| e.to_string()));
    let trust = match trust {
        Ok(t) => t,
        Err(e) => {
            eprintln!("aviso: pacotes de plugin nao carregados (sem trust store): {e}");
            return vec![];
        }
    };
    let mut tools = Vec::new();
    let mut pastas: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    pastas.sort();
    for p in pastas {
        match ler(&p, &trust) {
            Ok(pac) => {
                let (t, avisos) = ferramentas(&pac);
                for a in avisos {
                    eprintln!("aviso: pacote {}: {a}", pac.nome);
                }
                tools.extend(t);
            }
            Err(e) => eprintln!("aviso: pacote {} recusado: {e}", p.display()),
        }
    }
    tools
}

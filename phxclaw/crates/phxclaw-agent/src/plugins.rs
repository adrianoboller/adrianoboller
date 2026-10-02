//! Plugins assinados como ferramentas do agente.
//!
//! A verificacao NAO e reescrita aqui: quem le o manifesto, confere o sha256 do artefato e
//! a assinatura Ed25519 contra o `plugin-signers.json` e poe em quarentena o que falha e o
//! `PluginRegistry` de `phxclaw-plugin-registry`. Esta camada so decide, do que ele
//! aceitou, o que vira ferramenta:
//!
//! - so quem declara o ponto de extensao `agent.tool` (opt-in: o instalador de ambiente,
//!   que pede `system.admin`, e plugin `kind=tool` e nao deve virar ferramenta do modelo so
//!   por estar assinado);
//! - `kind=tool` com entrada `process`;
//! - a capacidade PRIMARIA do manifesto (a primeira) tem de estar concedida ao agente --
//!   e ela que o portao do motor confere a cada chamada, entao o subagente com menos poder
//!   perde a ferramenta pelo mesmo portao das outras;
//! - as demais capacidades declaradas entram so na intersecao com as do agente, e o
//!   plugin as recebe em `PHXCLAW_CAPACIDADES`.
//!
//! Na execucao, o artefato e RELIDO e conferido contra o digest assinado antes de cada
//! chamada, e o que roda e a copia desses mesmos bytes: trocar o arquivo depois da carga
//! nao troca o codigo que executa. Roda no MESMO sandbox do `shell` (bwrap, sem rede,
//! ambiente limpo, so `/work` gravavel), com os argumentos em `PHXCLAW_ARGS`.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_plugin_registry::{PluginRegistry, TrustStore};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, run_in_workdir_com};
use phxclaw_types::PluginManifest;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// O ponto de extensao que torna um plugin assinado uma ferramenta do agente.
pub const PONTO_FERRAMENTA: &str = "agent.tool";

/// Uma ferramenta vinda de plugin assinado.
pub struct PluginTool {
    pub plugin: String,
    pub nome: String,
    pub descricao: String,
    pub esquema: Value,
    capacidade: &'static str,
    /// Capacidades do manifesto que o agente tambem tem.
    pub efetivas: Vec<String>,
    artefato: PathBuf,
    digest: String,
    bwrap: PathBuf,
    timeout: Duration,
}

/// O que a carga aceitou e o que recusou, com o motivo (plugin, motivo).
#[derive(Default)]
pub struct Carga {
    pub ferramentas: Vec<PluginTool>,
    pub recusas: Vec<(String, String)>,
}

/// `capability()` devolve `&'static str`, e a capacidade do plugin so se conhece em
/// tempo de execucao. Internar uma vez por NOME limita o vazamento ao numero de
/// capacidades distintas, nao ao de cargas.
fn internar(s: &str) -> &'static str {
    static TABELA: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut t = TABELA
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(x) = t.get(s) {
        return x;
    }
    let x: &'static str = Box::leak(s.to_string().into_boxed_str());
    t.insert(x);
    x
}

fn hex_sha256(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// Nome de ferramenta aceito pelos quatro provedores: `plugin_<ultimo segmento>`.
fn nome_da_ferramenta(m: &PluginManifest) -> String {
    let base = m.name.rsplit('.').next().unwrap_or(&m.name);
    let limpo: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("plugin_{limpo}").chars().take(64).collect()
}

/// Carrega os plugins de `manifestos` (pastas varridas pelo registro) com os artefatos
/// relativos a `raiz`, e devolve as ferramentas para o agente com `caps`.
pub fn carregar(
    raiz: &Path,
    manifestos: &Path,
    trust: TrustStore,
    api: &str,
    caps: &BTreeSet<String>,
    bwrap: &Path,
) -> Result<Carga, String> {
    let mut reg = PluginRegistry::new(api, raiz, trust);
    reg.discover_tree(manifestos).map_err(|e| e.to_string())?;
    let mut carga = Carga::default();
    for q in reg.quarantine() {
        carga.recusas.push((
            q.plugin_name.clone().unwrap_or_else(|| q.source.clone()),
            format!("quarentena: {}", q.reason),
        ));
    }
    let raiz_c = std::fs::canonicalize(raiz).map_err(|e| format!("raiz dos plugins: {e}"))?;
    let mut nomes = BTreeSet::new();
    for m in reg.manifests() {
        if !m
            .extension_points
            .iter()
            .any(|e| e.point == PONTO_FERRAMENTA)
        {
            continue;
        }
        match ferramenta_de(m, &raiz_c, caps, bwrap) {
            Ok(t) if !nomes.insert(t.nome.clone()) => carga.recusas.push((
                m.name.clone(),
                format!("nome de ferramenta repetido: {}", t.nome),
            )),
            Ok(t) => carga.ferramentas.push(t),
            Err(motivo) => carga.recusas.push((m.name.clone(), motivo)),
        }
    }
    Ok(carga)
}

fn ferramenta_de(
    m: &PluginManifest,
    raiz: &Path,
    caps: &BTreeSet<String>,
    bwrap: &Path,
) -> Result<PluginTool, String> {
    if m.kind != "tool" || m.entrypoint.kind != "process" {
        return Err("ferramenta de agente exige kind=tool e entrypoint process".into());
    }
    let primaria = m.capabilities.first().ok_or("manifesto sem capacidade")?;
    if !caps.contains(primaria) {
        return Err(format!(
            "capacidade primaria {primaria} nao concedida ao agente"
        ));
    }
    let efetivas: Vec<String> = m
        .capabilities
        .iter()
        .filter(|c| caps.contains(*c))
        .cloned()
        .collect();
    let dentro = |rel: &str, o_que: &str| -> Result<PathBuf, String> {
        let p = std::fs::canonicalize(raiz.join(rel)).map_err(|e| format!("{o_que}: {e}"))?;
        if !p.starts_with(raiz) {
            return Err(format!("{o_que} fora da raiz dos plugins"));
        }
        Ok(p)
    };
    let artefato = dentro(&m.integrity.artifact, "artefato")?;
    let esquema: Value = serde_json::from_str(
        &std::fs::read_to_string(dentro(&m.contracts.input_schema, "esquema de entrada")?)
            .map_err(|e| format!("esquema de entrada: {e}"))?,
    )
    .map_err(|e| format!("esquema de entrada nao e JSON: {e}"))?;
    if esquema.get("type").and_then(Value::as_str) != Some("object") {
        return Err("esquema de entrada precisa ser {\"type\":\"object\",...}".into());
    }
    Ok(PluginTool {
        plugin: m.name.clone(),
        nome: nome_da_ferramenta(m),
        descricao: format!(
            "[plugin assinado {} {}] {}",
            m.name, m.version, m.description
        ),
        esquema,
        capacidade: internar(primaria),
        efetivas,
        artefato,
        digest: m.integrity.digest.to_ascii_lowercase(),
        bwrap: bwrap.to_path_buf(),
        timeout: Duration::from_millis(m.sandbox.timeout_ms),
    })
}

/// Plugins do ambiente: `PHXCLAW_PLUGINS_RAIZ` (raiz do pacote, onde estao os artefatos
/// e `config/`), `PHXCLAW_PLUGINS_DIR` (manifestos; padrao `<raiz>/plugins`) e
/// `PHXCLAW_PLUGIN_SIGNERS` (padrao `<raiz>/config/trust/plugin-signers.json`). Sem raiz,
/// nenhum plugin; recusa vira aviso, nunca derruba o agente.
pub fn do_ambiente(caps: &BTreeSet<String>) -> Vec<std::sync::Arc<dyn Tool>> {
    let Some(raiz) = crate::config::caminho_de("plugins.raiz") else {
        return vec![];
    };
    let Some(bwrap) = crate::arquivos::achar_bwrap() else {
        eprintln!("aviso: plugins sem bwrap nao rodam");
        return vec![];
    };
    let manifestos =
        crate::config::caminho_de("plugins.dir").unwrap_or_else(|| raiz.join("plugins"));
    let signers = crate::config::caminho_de("plugins.assinantes")
        .unwrap_or_else(|| raiz.join("config/trust/plugin-signers.json"));
    let r = (|| -> Result<Carga, String> {
        let trust = TrustStore::from_json(
            &std::fs::read_to_string(&signers)
                .map_err(|e| format!("{}: {e}", signers.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let c: Value = serde_json::from_str(
            &std::fs::read_to_string(raiz.join("config/constitution.json"))
                .map_err(|e| format!("constituicao: {e}"))?,
        )
        .map_err(|e| e.to_string())?;
        let api = c["plugin_api_version"]
            .as_str()
            .ok_or("constituicao sem plugin_api_version")?;
        carregar(&raiz, &manifestos, trust, api, caps, &bwrap)
    })();
    match r {
        Ok(c) => {
            for (p, m) in &c.recusas {
                eprintln!("aviso: plugin {p} fora do agente: {m}");
            }
            c.ferramentas
                .into_iter()
                .map(|t| std::sync::Arc::new(t) as std::sync::Arc<dyn Tool>)
                .collect()
        }
        Err(e) => {
            eprintln!("aviso: plugins nao carregados: {e}");
            vec![]
        }
    }
}

impl Tool for PluginTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.nome.clone(),
            description: self.descricao.clone(),
            parameters: self.esquema.clone(),
        }
    }
    fn capability(&self) -> &'static str {
        self.capacidade
    }
    /// O plugin cria processo, e a capacidade dele e dinamica (fora da lista fixa de quem
    /// executa): declarar a linha `<ferramenta>` e o que faz as regras de comando
    /// (`negar`, `perguntar`, `padrao`) o alcancarem no portao.
    fn comando_de_shell(&self, _args: &Value) -> Option<String> {
        Some(self.nome.clone())
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let bytes =
                std::fs::read(&self.artefato).map_err(|e| ToolError::Failed(e.to_string()))?;
            let visto = hex_sha256(&bytes);
            if visto != self.digest {
                return Err(ToolError::Denied(format!(
                    "artefato do plugin {} mudou depois de assinado (sha256 {visto}, assinado {})",
                    self.plugin, self.digest
                )));
            }
            // Roda a copia dos bytes conferidos, nao o caminho: entre conferir e executar
            // o arquivo do pacote poderia ser trocado.
            let copia = std::env::temp_dir()
                .join(format!("phxclaw-plugin-{}", phxclaw_types::new_uuid_v7()));
            escrever_executavel(&copia, &bytes).map_err(|e| ToolError::Failed(e.to_string()))?;
            let cmd = WorkdirCommand {
                workdir: ctx.workdir.clone(),
                script: "exec /plugin/entrada".into(),
                timeout: self.timeout.min(ctx.timeout),
                network: false,
                max_output_bytes: 64 * 1024,
            };
            let extras = SandboxExtras {
                ro_binds: vec![(copia.clone(), "/plugin/entrada".into())],
                env: vec![
                    ("PHXCLAW_ARGS".into(), args.to_string()),
                    ("PHXCLAW_CAPACIDADES".into(), self.efetivas.join(",")),
                    ("PHXCLAW_PLUGIN".into(), self.plugin.clone()),
                ],
            };
            let bwrap = self.bwrap.clone();
            let r = tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
                .await;
            let _ = std::fs::remove_file(&copia);
            let r = r
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            if r.exit_code != Some(0) {
                return Err(ToolError::Failed(format!(
                    "plugin {} saiu com {:?}: {}",
                    self.plugin,
                    r.exit_code,
                    r.stderr.trim()
                )));
            }
            Ok(ToolOutput::text(r.stdout))
        })
    }
}

fn escrever_executavel(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(p, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o500))?;
    }
    Ok(())
}

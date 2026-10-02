//! Historico local das gravacoes do usuario (a «timeline» de arquivo do VS Code): cada
//! vez que um arquivo do projeto muda em disco, a versao gravada vai para
//! `.phxclaw/historico/<caminho>/<rowstamp>`, com um teto de versoes por arquivo
//! (`ide.historico_versoes_max`, a variavel PHXCLAW_IDE_HISTORICO_VERSOES_MAX; padrao 50).
//!
//! Por que um poller de mtime, e nao inotify: so `std`, e o custo e uma varredura da
//! pasta do projeto a cada intervalo -- a mesma do `glob`/`grep` (`busca::percorrer`),
//! que ja pula o `.gitignore`, e por isso nao copia `target/` nem `node_modules/`. A
//! primeira varredura so LEMBRA o estado: copiar o projeto inteiro na partida nao e
//! historico de gravacao, e lixo.
//!
//! O `rowstamp` e um contador que nunca empata nem recua (microssegundos desde a epoca,
//! forcado acima do ultimo visto na pasta): e ele que ordena as versoes, nao o relogio.
//!
//! Quem usa: o poller do IDE roda sobre a pasta do projeto (`montagem::raiz_do_projeto`,
//! de onde `PHXCLAW_PROJETO` e lida); a ferramenta `file_history` le e restaura o
//! historico da pasta da tarefa -- a mesma pasta quando o agente trabalha no projeto do
//! IDE, e so ela, porque ferramenta nao escreve fora da pasta da tarefa.

use crate::busca::percorrer;
use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CHAVE_VERSOES_MAX: &str = "ide.historico_versoes_max";
pub const VERSOES_MAX_PADRAO: usize = 50;
/// Debaixo da raiz do projeto.
pub const PASTA: &str = ".phxclaw/historico";
/// Arquivo maior que isto nao entra no historico (dump, binario grande).
const MAX_BYTES: u64 = 5 * 1024 * 1024;
const INTERVALO_DO_POLLER: Duration = Duration::from_secs(2);

/// O teto de versoes por arquivo, do catalogo (o operador muda no config.json).
pub fn teto_da_configuracao() -> usize {
    crate::config::valor(CHAVE_VERSOES_MAX)
        .ok()
        .flatten()
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        .map(|n| usize::try_from(n).unwrap_or(usize::MAX).max(1))
        .unwrap_or(VERSOES_MAX_PADRAO)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Versao {
    pub rowstamp: u64,
    pub bytes: u64,
}

pub struct Historico {
    raiz: PathBuf,
    teto: usize,
    /// (mtime, tamanho) de cada arquivo na ultima varredura.
    visto: HashMap<String, (SystemTime, u64)>,
    primeira: bool,
    ultimo_rowstamp: u64,
}

fn caminho_relativo(rel: &str) -> Result<&str, ToolError> {
    let r = rel.trim().trim_start_matches("./");
    let ok = !r.is_empty()
        && !r.starts_with('/')
        && !r.split('/').any(|p| p == ".." || p.is_empty())
        && !r.starts_with(".phxclaw/");
    if !ok {
        return Err(ToolError::InvalidArguments(format!(
            "caminho invalido para o historico: {rel:?} (relativo a raiz, sem .. e fora de .phxclaw)"
        )));
    }
    Ok(r)
}

impl Historico {
    pub fn novo(raiz: PathBuf, teto: usize) -> Self {
        Self {
            raiz,
            teto: teto.max(1),
            visto: HashMap::new(),
            primeira: true,
            ultimo_rowstamp: 0,
        }
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    fn pasta_de(&self, rel: &str) -> PathBuf {
        self.raiz.join(PASTA).join(rel)
    }

    /// Uma varredura: compara mtime e tamanho com a anterior e guarda a versao de cada
    /// arquivo que mudou ou nasceu. Devolve os caminhos guardados.
    pub fn varrer(&mut self) -> Vec<String> {
        let (arqs, _) = percorrer(&self.raiz, false);
        let mut agora = HashMap::with_capacity(arqs.len());
        let mut guardados = Vec::new();
        for r in arqs {
            if r.starts_with(".phxclaw/") {
                continue;
            }
            let Ok(m) = std::fs::metadata(self.raiz.join(&r)) else {
                continue;
            };
            let marca = (m.modified().unwrap_or(UNIX_EPOCH), m.len());
            let mudou = self.visto.get(&r) != Some(&marca);
            agora.insert(r.clone(), marca);
            if mudou && !self.primeira && self.guardar(&r).is_ok() {
                guardados.push(r);
            }
        }
        self.visto = agora;
        self.primeira = false;
        guardados
    }

    /// Proximo rowstamp: acima do relogio E do ultimo da pasta, para nunca empatar nem
    /// recuar -- nem entre dois processos que gravem o mesmo arquivo em sequencia.
    fn proximo_rowstamp(&mut self, pasta: &Path) -> u64 {
        let relogio = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        let na_pasta = Self::versoes_em(pasta)
            .last()
            .map(|v| v.rowstamp)
            .unwrap_or(0);
        let n = relogio.max(self.ultimo_rowstamp + 1).max(na_pasta + 1);
        self.ultimo_rowstamp = n;
        n
    }

    /// Copia o conteudo ATUAL de `rel` para o historico e poda ao teto. Devolve o rowstamp.
    pub fn guardar(&mut self, rel: &str) -> Result<u64, ToolError> {
        let rel = caminho_relativo(rel)?;
        let origem = self.raiz.join(rel);
        let m = std::fs::metadata(&origem).map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
        if !m.is_file() || m.len() > MAX_BYTES {
            return Err(ToolError::Failed(format!(
                "{rel}: nao e arquivo regular ate {} MiB",
                MAX_BYTES / (1024 * 1024)
            )));
        }
        let pasta = self.pasta_de(rel);
        std::fs::create_dir_all(&pasta).map_err(|e| ToolError::Failed(e.to_string()))?;
        let rs = self.proximo_rowstamp(&pasta);
        // Copia inteira em vez de rename: o arquivo do usuario fica onde esta.
        std::fs::copy(&origem, pasta.join(rs.to_string()))
            .map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
        let versoes = Self::versoes_em(&pasta);
        if versoes.len() > self.teto {
            for v in &versoes[..versoes.len() - self.teto] {
                let _ = std::fs::remove_file(pasta.join(v.rowstamp.to_string()));
            }
        }
        Ok(rs)
    }

    fn versoes_em(pasta: &Path) -> Vec<Versao> {
        let Ok(rd) = std::fs::read_dir(pasta) else {
            return vec![];
        };
        let mut v: Vec<Versao> = rd
            .flatten()
            .filter_map(|e| {
                let rowstamp = e.file_name().to_str()?.parse().ok()?;
                let bytes = e.metadata().ok()?.len();
                Some(Versao { rowstamp, bytes })
            })
            .collect();
        v.sort_by_key(|v| v.rowstamp);
        v
    }

    /// As versoes guardadas de `rel`, da mais antiga para a mais nova.
    pub fn versoes(&self, rel: &str) -> Result<Vec<Versao>, ToolError> {
        Ok(Self::versoes_em(&self.pasta_de(caminho_relativo(rel)?)))
    }

    pub fn ler(&self, rel: &str, rowstamp: u64) -> Result<Vec<u8>, ToolError> {
        let p = self
            .pasta_de(caminho_relativo(rel)?)
            .join(rowstamp.to_string());
        std::fs::read(&p).map_err(|_| {
            ToolError::InvalidArguments(format!("{rel}: nao ha versao {rowstamp} no historico"))
        })
    }

    /// Volta `rel` a versao `rowstamp`. A versao atual vai ao historico ANTES, para o
    /// restore ter volta. Devolve o rowstamp da versao guardada (ou `None` se o arquivo
    /// nao existia).
    pub fn restaurar(&mut self, rel: &str, rowstamp: u64) -> Result<Option<u64>, ToolError> {
        let rel = caminho_relativo(rel)?;
        let conteudo = self.ler(rel, rowstamp)?;
        let alvo = self.raiz.join(rel);
        let guardada = if alvo.is_file() {
            Some(self.guardar(rel)?)
        } else if let Some(p) = alvo.parent() {
            std::fs::create_dir_all(p).map_err(|e| ToolError::Failed(e.to_string()))?;
            None
        } else {
            None
        };
        std::fs::write(&alvo, conteudo).map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
        // O que acabou de ser escrito ja esta lembrado: a proxima varredura nao o guarda
        // de novo como se fosse gravacao do usuario.
        if let Ok(m) = std::fs::metadata(&alvo) {
            self.visto.insert(
                rel.to_string(),
                (m.modified().unwrap_or(UNIX_EPOCH), m.len()),
            );
        }
        Ok(guardada)
    }
}

/// Poller numa thread propria (so `std`): varre `raiz` a cada `intervalo`, para sempre.
pub fn iniciar_poller(
    raiz: PathBuf,
    teto: usize,
    intervalo: Duration,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("phxclaw-historico".into())
        .spawn(move || {
            let mut h = Historico::novo(raiz, teto);
            h.varrer();
            loop {
                std::thread::sleep(intervalo);
                h.varrer();
            }
        })
        .expect("thread do historico")
}

/// O poller do projeto do IDE, uma vez por processo: a raiz de onde `PHXCLAW_PROJETO` e
/// lida e o teto do catalogo. Sem projeto (sem pasta corrente), nao ha o que observar.
pub fn iniciar_do_projeto() -> Option<PathBuf> {
    static UMA_VEZ: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    UMA_VEZ
        .get_or_init(|| {
            let raiz = crate::montagem::raiz_do_projeto().filter(|p| p.is_dir())?;
            iniciar_poller(raiz.clone(), teto_da_configuracao(), INTERVALO_DO_POLLER);
            Some(raiz)
        })
        .clone()
}

/// `file_history` (fs.read: list, show) e `file_history_restore` (fs.write: restore): a
/// mesma ferramenta registrada duas vezes, uma por capacidade, como `git`/`git_write`.
pub struct FileHistoryTool {
    pub escrita: bool,
}

impl Tool for FileHistoryTool {
    fn spec(&self) -> ToolSpec {
        if self.escrita {
            ToolSpec {
                name: "file_history_restore".into(),
                description: "Restore a file of the task directory to a saved version from \
its local save history (.phxclaw/historico). {path, version} where version is a rowstamp \
from file_history list. The current content is saved to the history first, so it can be \
undone."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "path":{"type":"string"},
                    "version":{"type":"integer"}
                },"required":["path","version"]}),
            }
        } else {
            ToolSpec {
                name: "file_history".into(),
                description: "Local save history of a file in the task directory (every save \
seen by the IDE poller, newest last). action=list {path} gives the versions (rowstamp, \
bytes); action=show {path, version} returns that version's text. Restore is \
file_history_restore."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["list","show"]},
                    "path":{"type":"string"},
                    "version":{"type":"integer"}
                },"required":["action","path"]}),
            }
        }
    }
    fn capability(&self) -> &'static str {
        if self.escrita { "fs.write" } else { "fs.read" }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?
                .trim()
                .trim_start_matches("/work/")
                .to_string();
            confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
            let versao = || {
                args.get("version").and_then(Value::as_u64).ok_or_else(|| {
                    ToolError::InvalidArguments(
                        "falta 'version' (rowstamp de file_history list)".into(),
                    )
                })
            };
            let mut h = Historico::novo(ctx.workdir.clone(), teto_da_configuracao());
            if self.escrita {
                let v = versao()?;
                let guardada = h.restaurar(&rel, v)?;
                let a = crate::motor::artifact_for(&ctx.workdir, &rel)
                    .map_err(|e| ToolError::Failed(e.to_string()))?;
                return Ok(ToolOutput {
                    content:
                        json!({"arquivo": rel, "restaurada": v, "anterior_guardada": guardada})
                            .to_string(),
                    artifacts: vec![a],
                });
            }
            match args.get("action").and_then(Value::as_str).unwrap_or("list") {
                "list" => {
                    let v = h.versoes(&rel)?;
                    Ok(ToolOutput::text(
                        json!({"arquivo": rel, "versoes": v, "teto": h.teto}).to_string(),
                    ))
                }
                "show" => {
                    let v = versao()?;
                    let b = h.ler(&rel, v)?;
                    let texto = String::from_utf8_lossy(&b);
                    Ok(ToolOutput::text(
                        json!({"arquivo": rel, "versao": v, "bytes": b.len(),
                               "texto": phxclaw_agent_core::truncate_for_model(&texto, 60_000)})
                        .to_string(),
                    ))
                }
                outra => Err(ToolError::InvalidArguments(format!(
                    "action desconhecida: {outra} (list, show; restore e file_history_restore)"
                ))),
            }
        })
    }
}

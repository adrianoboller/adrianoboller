#![forbid(unsafe_code)]
//! Pontos de restauracao de uma pasta de trabalho.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Conteudo enderecado pelo hash.** Cada arquivo vira um objeto `objetos/<sha256>`
//!   gravado uma vez so; o checkpoint e so o manifesto. O agente tira um ponto antes de
//!   CADA escrita, e copiar a pasta inteira a cada `write_file` faria o custo crescer com
//!   o numero de passos; assim cresce so com o que mudou.
//! - **Restaurar nao apaga por padrao.** O ponto devolve o que existia; arquivo que nasceu
//!   depois so sai se quem restaura pedir (`remover_novos`). Na pasta que o `mcp-serve`
//!   recebe por `--trabalho` mora gente alem do agente, e apagar o que alguem criou ali
//!   seria um estrago pior que o que a restauracao desfaz.
//! - **O objeto e conferido antes de voltar.** Objeto cujo hash nao bate e recusa: voltar
//!   um arquivo corrompido em cima do bom e a pior restauracao possivel.
//! - **Teto de arquivos e de bytes.** Pasta grande demais recusa o ponto dizendo o tamanho,
//!   em vez de travar o passo do agente lendo um repositorio inteiro a cada escrita.
use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Formato do manifesto que este codigo grava: 1 = copia inteira em `<uuid>/files`
/// (manifesto sem o campo), 2 = objetos por hash em `objetos/`.
pub const FORMATO_ATUAL: u32 = 2;

fn formato_antigo() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointManifest {
    /// Diz onde mora o conteudo. Sem o campo e o formato 1: o manifesto antigo nao o tinha,
    /// e adivinhar pelo que existe no disco escolheria a fonte errada no dia em que as duas
    /// existissem.
    #[serde(default = "formato_antigo")]
    pub formato: u32,
    pub uuid: Uuid,
    pub workspace: PathBuf,
    pub files: Vec<CheckpointFile>,
    pub created_at: DateTime<Utc>,
    /// Por que o ponto nasceu ("antes de write_file"); manifesto antigo nao tem.
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointVerifyReport {
    pub checkpoint_uuid: Uuid,
    pub valid: bool,
    pub changed: Vec<String>,
    pub missing: Vec<String>,
    /// Arquivos que existem hoje e nao existiam no ponto.
    #[serde(default)]
    pub added: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RestoreReport {
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    /// Arquivos novos deixados no lugar porque `remover_novos` nao foi pedido.
    pub kept_new: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("workspace does not exist: {0}")]
    MissingWorkspace(String),
    #[error("checkpoint path escapes workspace: {0}")]
    EscapesWorkspace(String),
    #[error("workspace too large for a checkpoint: {0}")]
    TooLarge(String),
    #[error("checkpoint object corrupted or missing: {0}")]
    Corrupted(String),
    #[error("checkpoint format {0} is newer than this program reads (up to {FORMATO_ATUAL})")]
    UnknownFormat(u32),
    #[error("restore stopped at {falhou}: {erro}; already restored: {restaurados:?}")]
    Incomplete {
        falhou: String,
        erro: String,
        restaurados: Vec<String>,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct CheckpointManager {
    store_root: PathBuf,
    max_files: usize,
    max_bytes: u64,
}

impl CheckpointManager {
    pub fn new(store_root: impl Into<PathBuf>) -> Self {
        Self {
            store_root: store_root.into(),
            max_files: 20_000,
            max_bytes: 256 * 1024 * 1024,
        }
    }

    pub fn with_limits(mut self, max_files: usize, max_bytes: u64) -> Self {
        self.max_files = max_files;
        self.max_bytes = max_bytes;
        self
    }

    pub fn create(&self, workspace: &Path) -> Result<CheckpointManifest, CheckpointError> {
        self.create_labeled(workspace, None)
    }

    /// O estado da pasta, sem gravar nada: e o que o `create` grava e o que o agente
    /// compara com o ultimo ponto para nao empilhar dois iguais.
    pub fn scan(
        &self,
        workspace: &Path,
    ) -> Result<(PathBuf, Vec<CheckpointFile>), CheckpointError> {
        let workspace = fs::canonicalize(workspace)
            .map_err(|_| CheckpointError::MissingWorkspace(workspace.display().to_string()))?;
        let mut files = Vec::new();
        let mut total = 0u64;
        for path in self.workspace_files(&workspace)? {
            let relative = rel_of(&workspace, &path)?;
            // Teto pelo tamanho declarado, ANTES de ler: conferir depois de ler e pagar a
            // leitura inteira do arquivo de 1 GB para entao recusar.
            total += fs::symlink_metadata(&path)?.len();
            if files.len() >= self.max_files || total > self.max_bytes {
                return Err(CheckpointError::TooLarge(format!(
                    "mais de {} arquivos ou {} bytes (parou em {relative})",
                    self.max_files, self.max_bytes
                )));
            }
            let bytes = fs::read(&path)?;
            files.push(CheckpointFile {
                path: relative,
                sha256: hex_sha256(&bytes),
                bytes: bytes.len() as u64,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok((workspace, files))
    }

    pub fn create_labeled(
        &self,
        workspace: &Path,
        label: Option<String>,
    ) -> Result<CheckpointManifest, CheckpointError> {
        let (workspace, files) = self.scan(workspace)?;
        let objetos = self.store_root.join("objetos");
        fs::create_dir_all(&objetos)?;
        for f in &files {
            let destino = objetos.join(&f.sha256);
            // Acerto do dedup so com o tamanho certo: um objeto que ficou com 0 byte (queda
            // entre criar e escrever) seria herdado por todo ponto seguinte, e so se
            // descobriria no dia de restaurar.
            if fs::symlink_metadata(&destino).is_ok_and(|m| m.is_file() && m.len() == f.bytes) {
                continue;
            }
            // Le de novo e confere: o arquivo pode ter mudado entre a varredura e a copia,
            // e objeto com nome de um hash e conteudo de outro envenenaria todo ponto que
            // o citasse.
            let bytes = fs::read(confined_join(&workspace, &f.path)?)?;
            if hex_sha256(&bytes) != f.sha256 {
                return Err(CheckpointError::Corrupted(format!(
                    "{} mudou durante o ponto",
                    f.path
                )));
            }
            gravar_atomico(&destino, &bytes)?;
        }
        sincronizar_pasta(&objetos)?;
        let uuid = new_uuid_v7();
        let checkpoint_root = self.store_root.join(uuid.to_string());
        fs::create_dir_all(&checkpoint_root)?;
        let manifest = CheckpointManifest {
            formato: FORMATO_ATUAL,
            uuid,
            workspace,
            files,
            created_at: Utc::now(),
            label,
        };
        // Manifesto pela metade (queda no meio do write) seria um ponto que existe e nao
        // le; tmp + fsync + rename + fsync da pasta faz o ponto existir inteiro ou nao existir.
        gravar_atomico(
            &checkpoint_root.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
        sincronizar_pasta(&checkpoint_root)?;
        sincronizar_pasta(&self.store_root)?;
        Ok(manifest)
    }

    pub fn load(&self, uuid: Uuid) -> Result<CheckpointManifest, CheckpointError> {
        let m: CheckpointManifest = serde_json::from_slice(&fs::read(
            self.store_root.join(uuid.to_string()).join("manifest.json"),
        )?)?;
        if m.formato > FORMATO_ATUAL {
            return Err(CheckpointError::UnknownFormat(m.formato));
        }
        Ok(m)
    }

    /// Os ids dos pontos, em ordem de tempo (o UUIDv7 ja nasce ordenado), sem abrir nenhum.
    fn ids(&self) -> Vec<Uuid> {
        let mut v: Vec<Uuid> = fs::read_dir(&self.store_root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse().ok()))
            .collect();
        v.sort();
        v
    }

    /// Todos os pontos legiveis, do mais antigo ao mais novo. Ilegivel nao derruba a lista.
    pub fn list(&self) -> Result<Vec<CheckpointManifest>, CheckpointError> {
        Ok(self.list_relatando().0)
    }

    /// Os pontos legiveis e, a parte, os que nao se leem (id, motivo): um manifesto
    /// estragado nao pode esconder os outros, e tambem nao pode sumir calado.
    pub fn list_relatando(&self) -> (Vec<CheckpointManifest>, Vec<(String, String)>) {
        let (mut ok, mut ruins) = (Vec::new(), Vec::new());
        for id in self.ids() {
            match self.load(id) {
                Ok(m) => ok.push(m),
                Err(e) => ruins.push((id.to_string(), e.to_string())),
            }
        }
        (ok, ruins)
    }

    /// So o ponto mais novo, lendo um manifesto em vez de todos: e o que o agente pergunta
    /// a cada escrita. Ilegivel vale como «nao ha ultimo» (o proximo ponto nasce).
    pub fn ultimo(&self) -> Option<CheckpointManifest> {
        self.ids().last().and_then(|id| self.load(*id).ok())
    }

    pub fn verify(
        &self,
        manifest: &CheckpointManifest,
    ) -> Result<CheckpointVerifyReport, CheckpointError> {
        let mut changed = Vec::new();
        let mut missing = Vec::new();
        // A mesma trava da restauracao: ler seguindo link conferiria (e hashearia) um
        // arquivo de fora da pasta. Caminho que passa por link ou virou pasta e «mudou».
        let raiz = fs::canonicalize(&manifest.workspace).unwrap_or(manifest.workspace.clone());
        for file in &manifest.files {
            let path = match destino_seguro(&raiz, &file.path) {
                Ok(p) => p,
                Err(CheckpointError::EscapesWorkspace(_)) => {
                    changed.push(file.path.clone());
                    continue;
                }
                Err(e) => return Err(e),
            };
            if fs::symlink_metadata(&path).is_err() {
                missing.push(file.path.clone());
                continue;
            }
            if hex_sha256(&fs::read(path)?) != file.sha256 {
                changed.push(file.path.clone());
            }
        }
        let added = self.added_since(manifest)?;
        Ok(CheckpointVerifyReport {
            checkpoint_uuid: manifest.uuid,
            valid: changed.is_empty() && missing.is_empty() && added.is_empty(),
            changed,
            missing,
            added,
        })
    }

    /// Devolve os arquivos do ponto e, so se pedido, apaga os que nasceram depois.
    pub fn restore(
        &self,
        manifest: &CheckpointManifest,
        remover_novos: bool,
    ) -> Result<RestoreReport, CheckpointError> {
        // Confere todos os objetos ANTES de tocar a pasta: restauracao que falha no meio
        // deixa metade de um estado e metade de outro.
        let mut conteudos = Vec::with_capacity(manifest.files.len());
        for file in &manifest.files {
            let bytes = self.object(manifest, file)?;
            conteudos.push((file, bytes));
        }
        // E todos os DESTINOS tambem: link simbolico no caminho mandaria a escrita para fora
        // da pasta, e pasta onde o ponto tem arquivo faria a restauracao parar no meio.
        let raiz = fs::canonicalize(&manifest.workspace)?;
        let mut pendentes = Vec::new();
        for (file, bytes) in conteudos {
            let dest = destino_seguro(&raiz, &file.path)?;
            let igual = fs::read(&dest)
                .map(|b| hex_sha256(&b) == file.sha256)
                .unwrap_or(false);
            if !igual {
                pendentes.push((file, dest, bytes));
            }
        }
        let mut rel = RestoreReport::default();
        for (file, dest, bytes) in pendentes {
            let r = (|| -> Result<(), CheckpointError> {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                // Confere de novo depois de criar os pais: entre a conferencia e aqui alguem
                // pode ter trocado uma pasta por um link.
                let dest = destino_seguro(&raiz, &file.path)?;
                gravar_atomico(&dest, &bytes)
            })();
            if let Err(e) = r {
                return Err(CheckpointError::Incomplete {
                    falhou: file.path.clone(),
                    erro: e.to_string(),
                    restaurados: rel.restored,
                });
            }
            rel.restored.push(file.path.clone());
        }
        for novo in self.added_since(manifest)? {
            if remover_novos {
                fs::remove_file(destino_seguro(&raiz, &novo)?)?;
                rel.removed.push(novo);
            } else {
                rel.kept_new.push(novo);
            }
        }
        Ok(rel)
    }

    /// Compatibilidade com quem chamava a API antiga: restaura sem apagar nada.
    pub fn restore_files(&self, manifest: &CheckpointManifest) -> Result<usize, CheckpointError> {
        let r = self.restore(manifest, false)?;
        Ok(r.restored.len())
    }

    fn object(
        &self,
        manifest: &CheckpointManifest,
        file: &CheckpointFile,
    ) -> Result<Vec<u8>, CheckpointError> {
        // O formato do manifesto diz a fonte; nada de tentar uma e cair na outra.
        let fonte = match manifest.formato {
            1 => confined_join(
                &self
                    .store_root
                    .join(manifest.uuid.to_string())
                    .join("files"),
                &file.path,
            )?,
            2 => self.store_root.join("objetos").join(&file.sha256),
            f => return Err(CheckpointError::UnknownFormat(f)),
        };
        let bytes = fs::read(&fonte).map_err(|_| CheckpointError::Corrupted(file.path.clone()))?;
        if hex_sha256(&bytes) != file.sha256 {
            return Err(CheckpointError::Corrupted(file.path.clone()));
        }
        Ok(bytes)
    }

    fn added_since(&self, manifest: &CheckpointManifest) -> Result<Vec<String>, CheckpointError> {
        let conhecidos: BTreeSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
        let mut novos = Vec::new();
        if !manifest.workspace.exists() {
            return Ok(novos);
        }
        for path in self.workspace_files(&manifest.workspace)? {
            let r = rel_of(&manifest.workspace, &path)?;
            if !conhecidos.contains(r.as_str()) {
                novos.push(r);
            }
        }
        novos.sort();
        Ok(novos)
    }

    fn workspace_files(&self, workspace: &Path) -> Result<Vec<PathBuf>, CheckpointError> {
        Ok(WalkDir::new(workspace)
            .into_iter()
            .filter_entry(|e| !ignored(e.path(), workspace, &self.store_root))
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect())
    }
}

fn rel_of(workspace: &Path, path: &Path) -> Result<String, CheckpointError> {
    Ok(path
        .strip_prefix(workspace)
        .map_err(|_| CheckpointError::EscapesWorkspace(path.display().to_string()))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn ignored(path: &Path, workspace: &Path, store_root: &Path) -> bool {
    let rel = path.strip_prefix(workspace).ok();
    if let Some(rel) = rel
        && rel.components().next().is_some_and(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some(".git" | "target" | "node_modules" | "var")
            )
        })
    {
        return true;
    }
    path.starts_with(store_root)
}

fn confined_join(root: &Path, relative: &str) -> Result<PathBuf, CheckpointError> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(CheckpointError::EscapesWorkspace(relative.into()));
    }
    Ok(root.join(rel))
}

/// Caminho de escrita dentro de `raiz` (ja canonica) que nao passa por link simbolico em
/// pedaco nenhum e cujo fim, se existe, e arquivo. `confined_join` so confere o TEXTO;
/// `fs::write` segue link, e `a/b.txt` com `a -> /etc` escreveria em `/etc/b.txt`.
fn destino_seguro(raiz: &Path, relative: &str) -> Result<PathBuf, CheckpointError> {
    let alvo = confined_join(raiz, relative)?;
    let fora = |motivo: &str| CheckpointError::EscapesWorkspace(format!("{relative}: {motivo}"));
    let mut atual = raiz.to_path_buf();
    let partes: Vec<_> = Path::new(relative).components().collect();
    for (i, c) in partes.iter().enumerate() {
        atual.push(c);
        match fs::symlink_metadata(&atual) {
            Ok(m) if m.file_type().is_symlink() => return Err(fora("link simbolico no caminho")),
            Ok(m) if i + 1 < partes.len() && !m.is_dir() => return Err(fora("pai nao e pasta")),
            Ok(m) if i + 1 == partes.len() && !m.is_file() => {
                return Err(fora("o destino existe e nao e arquivo"));
            }
            Ok(_) => {}
            // Daqui para baixo nada existe: nao ha link a seguir.
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.into()),
        }
    }
    // Segunda trava, independente da primeira: o pai que existe, canonico, fica na raiz.
    let mut pai = alvo.parent().map(Path::to_path_buf);
    while let Some(p) = pai {
        if p.exists() {
            if !fs::canonicalize(&p)?.starts_with(raiz) {
                return Err(fora("o pai sai da pasta"));
            }
            break;
        }
        pai = p.parent().map(Path::to_path_buf);
    }
    Ok(alvo)
}

/// tmp no mesmo diretorio (nome unico: dois processos gravando o mesmo objeto nao se
/// atropelam), `sync_all`, e so entao o rename -- quem le nunca ve o arquivo pela metade.
fn gravar_atomico(destino: &Path, bytes: &[u8]) -> Result<(), CheckpointError> {
    use std::io::Write;
    let nome = destino
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = destino.with_file_name(format!(
        ".{nome}.{}.{}.tmp",
        std::process::id(),
        new_uuid_v7().simple()
    ));
    let r = (|| -> io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, destino)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(r?)
}

/// fsync da pasta: sem ele o rename pode nao sobreviver a uma queda de energia.
fn sincronizar_pasta(p: &Path) -> Result<(), CheckpointError> {
    fs::File::open(p)?.sync_all()?;
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phx-ckpt-{nome}-{}", new_uuid_v7()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn objeto_igual_e_gravado_uma_vez_e_o_ponto_volta_o_conteudo() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "um").unwrap();
        fs::write(w.join("b.txt"), "um").unwrap();
        let m = CheckpointManager::new(&s);
        let p1 = m.create(&w).unwrap();
        let p2 = m.create(&w).unwrap();
        // dois arquivos iguais em dois pontos: um objeto so
        assert_eq!(fs::read_dir(s.join("objetos")).unwrap().count(), 1);
        fs::write(w.join("a.txt"), "dois").unwrap();
        fs::remove_file(w.join("b.txt")).unwrap();
        fs::write(w.join("c.txt"), "novo").unwrap();
        let v = m.verify(&p1).unwrap();
        assert_eq!(
            (v.changed, v.missing, v.added),
            (
                vec!["a.txt".into()],
                vec!["b.txt".into()],
                vec!["c.txt".into()]
            )
        );
        let r = m.restore(&p2, false).unwrap();
        assert_eq!(r.restored, vec!["a.txt".to_string(), "b.txt".into()]);
        assert_eq!(r.kept_new, vec!["c.txt".to_string()]);
        assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "um");
        assert!(w.join("c.txt").exists(), "sem remover_novos nada se apaga");
        let r = m.restore(&p2, true).unwrap();
        assert_eq!(r.removed, vec!["c.txt".to_string()]);
        assert!(m.verify(&p1).unwrap().valid);
        assert_eq!(m.list().unwrap().len(), 2);
    }

    #[test]
    fn objeto_adulterado_recusa_sem_tocar_a_pasta() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "original").unwrap();
        let m = CheckpointManager::new(&s);
        let p = m.create(&w).unwrap();
        fs::write(w.join("a.txt"), "mudado").unwrap();
        let obj = s.join("objetos").join(&p.files[0].sha256);
        fs::write(obj, "veneno").unwrap();
        assert!(matches!(
            m.restore(&p, false),
            Err(CheckpointError::Corrupted(_))
        ));
        assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "mudado");
    }

    #[test]
    fn pasta_acima_do_teto_recusa_o_ponto() {
        let (w, s) = (tmp("w"), tmp("s"));
        for i in 0..3 {
            fs::write(w.join(format!("{i}.txt")), "x").unwrap();
        }
        let m = CheckpointManager::new(&s).with_limits(2, 1 << 20);
        assert!(matches!(m.create(&w), Err(CheckpointError::TooLarge(_))));
    }

    /// Bytes que ESTA thread leu do disco (`rchar`): a prova mede quanto foi lido, nao se
    /// recusou -- recusar depois de ler tudo tambem «recusa».
    fn lidos_pela_thread() -> u64 {
        fs::read_to_string("/proc/thread-self/io")
            .unwrap_or_default()
            .lines()
            .find_map(|l| l.strip_prefix("rchar: ")?.trim().parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn teto_confere_o_tamanho_antes_de_ler() {
        let (w, s) = (tmp("w"), tmp("s"));
        // 256 MiB esparsos: nao ocupam disco, mas `fs::read` le cada byte.
        fs::File::create(w.join("grande.bin"))
            .unwrap()
            .set_len(256 << 20)
            .unwrap();
        let m = CheckpointManager::new(&s).with_limits(100, 1 << 20);
        let antes = lidos_pela_thread();
        assert!(matches!(m.create(&w), Err(CheckpointError::TooLarge(_))));
        let lido = lidos_pela_thread() - antes;
        assert!(lido < 1 << 20, "leu {lido} bytes para recusar");
    }

    #[test]
    fn restaurar_nao_escreve_fora_por_link_simbolico() {
        use std::os::unix::fs::symlink;
        let (w, s, fora) = (tmp("w"), tmp("s"), tmp("fora"));
        fs::create_dir_all(w.join("d")).unwrap();
        fs::write(w.join("d/a.txt"), "ponto").unwrap();
        fs::write(w.join("b.txt"), "ponto").unwrap();
        let m = CheckpointManager::new(&s);
        let p = m.create(&w).unwrap();
        // pasta do caminho trocada por link para fora
        fs::remove_dir_all(w.join("d")).unwrap();
        symlink(&fora, w.join("d")).unwrap();
        assert!(matches!(
            m.restore(&p, false),
            Err(CheckpointError::EscapesWorkspace(_))
        ));
        assert!(
            !fora.join("a.txt").exists(),
            "escreveu fora pela pasta-link"
        );
        // o proprio arquivo trocado por link para fora
        fs::remove_file(w.join("d")).unwrap();
        fs::create_dir_all(w.join("d")).unwrap();
        fs::write(w.join("d/a.txt"), "ponto").unwrap();
        fs::remove_file(w.join("b.txt")).unwrap();
        fs::write(fora.join("alvo"), "intocado").unwrap();
        symlink(fora.join("alvo"), w.join("b.txt")).unwrap();
        assert!(m.restore(&p, false).is_err());
        assert_eq!(fs::read_to_string(fora.join("alvo")).unwrap(), "intocado");
    }

    #[test]
    fn conferir_nao_segue_link_para_fora() {
        use std::os::unix::fs::symlink;
        let (w, s, fora) = (tmp("w"), tmp("s"), tmp("fora"));
        fs::write(w.join("b.txt"), "ponto").unwrap();
        let m = CheckpointManager::new(&s);
        let p = m.create(&w).unwrap();
        // link para um arquivo de fora com o MESMO conteudo: seguir o link diria «igual»
        fs::write(fora.join("alvo"), "ponto").unwrap();
        fs::remove_file(w.join("b.txt")).unwrap();
        symlink(fora.join("alvo"), w.join("b.txt")).unwrap();
        let v = m.verify(&p).unwrap();
        assert_eq!(v.changed, vec!["b.txt".to_string()], "{v:?}");
        assert!(!v.valid);
    }

    #[test]
    fn destino_que_virou_pasta_recusa_antes_de_escrever_qualquer_coisa() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "ponto").unwrap();
        fs::write(w.join("b.txt"), "ponto").unwrap();
        let m = CheckpointManager::new(&s);
        let p = m.create(&w).unwrap();
        fs::write(w.join("a.txt"), "depois").unwrap();
        fs::remove_file(w.join("b.txt")).unwrap();
        fs::create_dir_all(w.join("b.txt")).unwrap();
        assert!(m.restore(&p, false).is_err());
        // a.txt vem antes na ordem: se a conferencia nao fosse previa, ele ja teria voltado
        assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "depois");
    }

    #[test]
    fn manifesto_ilegivel_nao_derruba_a_lista_e_aparece_relatado() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "x").unwrap();
        let m = CheckpointManager::new(&s);
        let bom = m.create(&w).unwrap();
        let ruim = new_uuid_v7();
        fs::create_dir_all(s.join(ruim.to_string())).unwrap();
        fs::write(s.join(ruim.to_string()).join("manifest.json"), "{\"uuid\":").unwrap();
        let l = m.list().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].uuid, bom.uuid);
        let (_, ruins) = m.list_relatando();
        assert_eq!(ruins.len(), 1);
        assert_eq!(ruins[0].0, ruim.to_string());
        // o ultimo ilegivel vale como «nao ha ultimo», e nenhum tmp sobra na pasta
        assert!(m.ultimo().is_none());
        let sobra: Vec<_> = fs::read_dir(s.join(bom.uuid.to_string()))
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(sobra, vec![std::ffi::OsString::from("manifest.json")]);
    }

    #[test]
    fn objeto_truncado_nao_e_herdado_pelo_dedup() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "conteudo").unwrap();
        let m = CheckpointManager::new(&s);
        let p1 = m.create(&w).unwrap();
        let obj = s.join("objetos").join(&p1.files[0].sha256);
        fs::write(&obj, "").unwrap();
        let p2 = m.create(&w).unwrap();
        assert_eq!(
            fs::metadata(&obj).unwrap().len(),
            8,
            "objeto de 0 byte herdado"
        );
        fs::write(w.join("a.txt"), "mudou").unwrap();
        m.restore(&p2, false).unwrap();
        assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "conteudo");
    }

    #[test]
    fn formato_do_manifesto_escolhe_a_fonte_e_o_desconhecido_e_recusado() {
        let (w, s) = (tmp("w"), tmp("s"));
        fs::write(w.join("a.txt"), "novo").unwrap();
        let m = CheckpointManager::new(&s);
        let p = m.create(&w).unwrap();
        let caminho = s.join(p.uuid.to_string()).join("manifest.json");
        let mut v: serde_json::Value =
            serde_json::from_slice(&fs::read(&caminho).unwrap()).unwrap();
        assert_eq!(v["formato"], 2);
        // formato 1 (sem o campo): o conteudo mora em <uuid>/files, mesmo com objetos/
        // existindo ao lado
        v.as_object_mut().unwrap().remove("formato");
        fs::write(&caminho, v.to_string()).unwrap();
        fs::create_dir_all(s.join(p.uuid.to_string()).join("files")).unwrap();
        fs::write(s.join(p.uuid.to_string()).join("files/a.txt"), "novo").unwrap();
        fs::remove_dir_all(s.join("objetos")).unwrap();
        fs::write(w.join("a.txt"), "mudou").unwrap();
        let p1 = m.load(p.uuid).unwrap();
        assert_eq!(p1.formato, 1);
        m.restore(&p1, false).unwrap();
        assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "novo");
        // formato do futuro: recusa dizendo qual
        v["formato"] = 3.into();
        fs::write(&caminho, v.to_string()).unwrap();
        let e = m.load(p.uuid).unwrap_err();
        assert!(matches!(e, CheckpointError::UnknownFormat(3)), "{e}");
        assert!(e.to_string().contains('3'));
    }
}

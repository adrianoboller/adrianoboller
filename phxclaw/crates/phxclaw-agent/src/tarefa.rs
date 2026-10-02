//! A tarefa do agente e o seu armazenamento em disco.
//!
//! Cada tarefa mora em `<raiz>/<id>/`: `task.json` (estado), `work/` (a pasta que as
//! ferramentas podem tocar) e `evidence.jsonl` (o livro de evidencias com hash encadeado).
//! Gravar o estado a cada passo e o que permite retomar, auditar e servir a tarefa pela API
//! depois de o processo cair.

use std::fs;
use std::path::{Path, PathBuf};

// Os tipos de fio moram no contrato comum, para o SDK le-los sem o agente inteiro.
pub use phxclaw_agent_core::tarefa::{StepRecord, Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct TaskStore {
    root: PathBuf,
}

impl TaskStore {
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(safe_id(id))
    }

    pub fn workdir(&self, id: &str) -> PathBuf {
        self.dir(id).join("work")
    }

    pub fn evidence_path(&self, id: &str) -> PathBuf {
        self.dir(id).join("evidence.jsonl")
    }

    /// Grava pela troca atomica da base: um processo que cai no meio da gravacao deixa o
    /// task.json anterior inteiro, nunca um JSON pela metade -- e com `fsync`, o anterior
    /// inteiro esta no disco, nao so na memoria do sistema.
    pub fn save(&self, task: &Task) -> std::io::Result<()> {
        let dir = self.dir(&task.id);
        fs::create_dir_all(dir.join("work"))?;
        phxclaw_types::arquivo::gravar_atomico(
            &dir.join("task.json"),
            &serde_json::to_vec_pretty(task)?,
        )
    }

    pub fn load(&self, id: &str) -> std::io::Result<Task> {
        let bytes = fs::read(self.dir(id).join("task.json"))?;
        serde_json::from_slice(&bytes).map_err(std::io::Error::other)
    }

    /// Todas as tarefas, mais novas primeiro (o id e UUIDv7, entao ordena por tempo).
    pub fn list(&self) -> std::io::Result<Vec<Task>> {
        let mut v = Vec::new();
        for e in fs::read_dir(&self.root)? {
            let e = e?;
            if e.file_type()?.is_dir()
                && let Ok(t) = self.load(&e.file_name().to_string_lossy())
            {
                v.push(t);
            }
        }
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
}

/// O id vem da API: so hexadecimal e hifen, para nunca virar caminho relativo.
fn safe_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect()
}

/// Caminho dentro da pasta de trabalho da tarefa, ou erro. Recusa absoluto, `..` e
/// symlink que aponte para fora: e a unica porta de disco das ferramentas. A excecao e
/// o caminho ABSOLUTO dentro de uma raiz do workspace (`.phxclaw/workspace.json`,
/// `ide.raizes`), e so ela -- e a lista das raizes so e lida depois de a pasta da tarefa
/// recusar, para o caminho comum nao pagar leitura de arquivo nenhuma.
pub fn confine(workdir: &Path, relative: &str) -> Result<PathBuf, String> {
    let recusa = match confine_na_tarefa(workdir, relative) {
        Ok(p) => return Ok(p),
        Err(e) => e,
    };
    if !Path::new(relative).is_absolute() {
        return Err(recusa);
    }
    let raizes = crate::workspace::raizes().map_err(|e| format!("{recusa} (workspace: {e})"))?;
    confine_nas_raizes(&raizes, relative).ok_or(recusa)
}

/// So a pasta da tarefa (o `confine` de sempre).
fn confine_na_tarefa(workdir: &Path, relative: &str) -> Result<PathBuf, String> {
    // O shell ve a pasta da tarefa como /work; o modelo repete esse caminho nas ferramentas
    // de arquivo (medido: 4 negacoes seguidas de read_file("/work/...")). E o mesmo lugar.
    let relative = relative
        .strip_prefix("/work/")
        .or_else(|| relative.strip_prefix("./"))
        .unwrap_or(relative);
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("caminho fora da pasta da tarefa: {relative}"));
    }
    let alvo = workdir.join(rel);
    // Se o arquivo (ou um pai) ja existe, a forma canonica tem de continuar dentro.
    let base = fs::canonicalize(workdir).map_err(|e| e.to_string())?;
    let canon = canonico_do_existente(&alvo)?;
    if !canon.starts_with(&base) {
        return Err(format!("caminho escapa da pasta da tarefa: {relative}"));
    }
    Ok(alvo)
}

/// Caminho absoluto dentro de uma das raizes, com a mesma regra de `..` e de symlink.
pub fn confine_nas_raizes(raizes: &[PathBuf], absoluto: &str) -> Option<PathBuf> {
    let alvo = Path::new(absoluto);
    if alvo.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) {
        return None;
    }
    let raiz = crate::workspace::raiz_de(raizes, alvo)?;
    let canon = canonico_do_existente(alvo).ok()?;
    canon.starts_with(raiz).then(|| alvo.to_path_buf())
}

/// A forma canonica do caminho, ou do pai mais proximo que ja existe.
fn canonico_do_existente(alvo: &Path) -> Result<PathBuf, String> {
    let mut existente = alvo.to_path_buf();
    while !existente.exists() {
        match existente.parent() {
            Some(p) => existente = p.to_path_buf(),
            None => break,
        }
    }
    fs::canonicalize(&existente).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_e_le_e_lista_em_ordem() {
        let dir = std::env::temp_dir().join(format!("phx-store-{}", std::process::id()));
        let s = TaskStore::new(&dir).unwrap();
        let a = Task::new("a", "m");
        let b = Task::new("b", "m");
        s.save(&a).unwrap();
        s.save(&b).unwrap();
        assert_eq!(s.load(&a.id).unwrap(), a);
        let l = s.list().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, b.id, "mais nova primeiro");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn id_da_api_nao_vira_caminho() {
        let raiz = std::env::temp_dir().join("phx-ids");
        let s = TaskStore::new(&raiz).unwrap();
        for hostil in ["../../etc", "/etc/passwd", "a/../../b", ".."] {
            let d = s.dir(hostil);
            assert!(d.starts_with(&raiz), "{hostil} saiu da raiz: {d:?}");
            assert!(!d.to_string_lossy().contains(".."), "{hostil}: {d:?}");
        }
        let _ = fs::remove_dir_all(raiz);
    }

    #[test]
    fn confinamento_recusa_fuga() {
        let dir = std::env::temp_dir().join(format!("phx-conf-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(confine(&dir, "a/b.txt").is_ok());
        assert_eq!(confine(&dir, "/work/a/b.txt").unwrap(), dir.join("a/b.txt"));
        assert!(confine(&dir, "/work/../x").is_err());
        assert!(confine(&dir, "../x").is_err());
        assert!(confine(&dir, "/etc/passwd").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", dir.join("fuga")).unwrap();
            assert!(
                confine(&dir, "fuga/passwd").is_err(),
                "symlink para fora passou"
            );
        }
        let _ = fs::remove_dir_all(dir);
    }
}

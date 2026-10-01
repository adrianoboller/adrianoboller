use chrono::{DateTime, Utc};
use phxclaw_types::{EvidenceRef, new_uuid_v7};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session(Uuid),
    Agent(Uuid),
    Project(String),
    Organization(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub scope: MemoryScope,
    pub classification: DataClassification,
    pub evidence: Vec<EvidenceRef>,
    pub confidence_millis: u16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub sha256: String,
}

impl MemoryRecord {
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: Value,
        scope: MemoryScope,
        classification: DataClassification,
        evidence: Vec<EvidenceRef>,
    ) -> Result<Self, MemoryError> {
        let now = Utc::now();
        let mut record = Self {
            uuid: new_uuid_v7(),
            namespace: namespace.into(),
            key: key.into(),
            value,
            scope,
            classification,
            evidence,
            confidence_millis: 1000,
            created_at: now,
            updated_at: now,
            expires_at: None,
            sha256: String::new(),
        };
        record.refresh_hash()?;
        Ok(record)
    }

    pub fn refresh_hash(&mut self) -> Result<(), MemoryError> {
        let mut clone = self.clone();
        clone.sha256.clear();
        let bytes = serde_json::to_vec(&clone)?;
        self.sha256 = format!("{:x}", Sha256::digest(bytes));
        Ok(())
    }
}

pub trait MemoryStore {
    fn put(&mut self, record: MemoryRecord) -> Result<(), MemoryError>;
    fn get(&self, namespace: &str, key: &str) -> Option<&MemoryRecord>;
    fn all(&self) -> Vec<&MemoryRecord>;
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    items: BTreeMap<(String, String), MemoryRecord>,
}

impl MemoryStore for InMemoryStore {
    fn put(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        self.items
            .insert((record.namespace.clone(), record.key.clone()), record);
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Option<&MemoryRecord> {
        self.items.get(&(namespace.to_string(), key.to_string()))
    }

    fn all(&self) -> Vec<&MemoryRecord> {
        self.items.values().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub max_items: usize,
    pub max_bytes: usize,
    pub max_classification: DataClassification,
    pub include_namespaces: Vec<String>,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            max_items: 64,
            max_bytes: 256 * 1024,
            max_classification: DataClassification::Internal,
            include_namespaces: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextScopeFilter {
    pub session_uuid: Option<Uuid>,
    pub agent_uuid: Option<Uuid>,
    pub project: Option<String>,
    pub organization: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItemRef {
    pub memory_uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub reason: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPack {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub items: Vec<ContextItemRef>,
    pub bytes_estimate: usize,
    pub compiled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMaterializedItem {
    pub memory_uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub classification: DataClassification,
    pub sha256: String,
    pub relevance_score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub pack: ContextPack,
    pub query_sha256: String,
    pub items: Vec<ContextMaterializedItem>,
}

#[derive(Debug, Default)]
pub struct ContextCompiler;

impl ContextCompiler {
    pub fn compile<S: MemoryStore>(
        &self,
        store: &S,
        correlation_uuid: Uuid,
        policy: &ContextPolicy,
    ) -> Result<ContextPack, MemoryError> {
        self.compile_for_query(
            store,
            correlation_uuid,
            "",
            policy,
            &ContextScopeFilter::default(),
        )
        .map(|bundle| bundle.pack)
    }

    pub fn compile_for_query<S: MemoryStore>(
        &self,
        store: &S,
        correlation_uuid: Uuid,
        query: &str,
        policy: &ContextPolicy,
        scope_filter: &ContextScopeFilter,
    ) -> Result<ContextBundle, MemoryError> {
        let now = Utc::now();
        let query_terms = tokenize(query);
        let mut candidates = Vec::<(&MemoryRecord, i64, usize)>::new();

        for record in store.all() {
            if record.expires_at.is_some_and(|expires| expires <= now) {
                continue;
            }
            if record.classification > policy.max_classification {
                continue;
            }
            if !policy.include_namespaces.is_empty()
                && !policy
                    .include_namespaces
                    .iter()
                    .any(|namespace| namespace == &record.namespace)
            {
                continue;
            }
            if !scope_matches(&record.scope, scope_filter) {
                continue;
            }

            let serialized = serde_json::to_vec(&record.value)?;
            let score = relevance_score(record, &query_terms);
            candidates.push((record, score, serialized.len()));
        }

        candidates.sort_by(|(left, left_score, _), (right, right_score, _)| {
            right_score
                .cmp(left_score)
                .then_with(|| right.confidence_millis.cmp(&left.confidence_millis))
                .then_with(|| left.namespace.cmp(&right.namespace))
                .then_with(|| left.key.cmp(&right.key))
                .then_with(|| left.uuid.cmp(&right.uuid))
        });

        let mut refs = Vec::new();
        let mut materialized = Vec::new();
        let mut bytes = 0usize;
        for (record, score, estimate) in candidates {
            if refs.len() >= policy.max_items || bytes.saturating_add(estimate) > policy.max_bytes {
                continue;
            }
            bytes += estimate;
            let reason = if query_terms.is_empty() {
                "policy_match".to_string()
            } else {
                format!("query_relevance:{score}")
            };
            refs.push(ContextItemRef {
                memory_uuid: record.uuid,
                namespace: record.namespace.clone(),
                key: record.key.clone(),
                reason,
                sha256: record.sha256.clone(),
            });
            materialized.push(ContextMaterializedItem {
                memory_uuid: record.uuid,
                namespace: record.namespace.clone(),
                key: record.key.clone(),
                value: record.value.clone(),
                classification: record.classification,
                sha256: record.sha256.clone(),
                relevance_score: score,
            });
        }

        let query_sha256 = format!("{:x}", Sha256::digest(query.as_bytes()));
        Ok(ContextBundle {
            pack: ContextPack {
                uuid: new_uuid_v7(),
                correlation_uuid,
                items: refs,
                bytes_estimate: bytes,
                compiled_at: now,
            },
            query_sha256,
            items: materialized,
        })
    }
}

fn scope_matches(scope: &MemoryScope, filter: &ContextScopeFilter) -> bool {
    match scope {
        MemoryScope::Session(uuid) => filter
            .session_uuid
            .is_none_or(|candidate| candidate == *uuid),
        MemoryScope::Agent(uuid) => filter.agent_uuid.is_none_or(|candidate| candidate == *uuid),
        MemoryScope::Project(project) => filter
            .project
            .as_ref()
            .is_none_or(|candidate| candidate == project),
        MemoryScope::Organization(org) => filter
            .organization
            .as_ref()
            .is_none_or(|candidate| candidate == org),
    }
}

fn relevance_score(record: &MemoryRecord, query_terms: &BTreeSet<String>) -> i64 {
    if query_terms.is_empty() {
        return 0;
    }
    term_score(record, query_terms) + i64::from(record.confidence_millis / 100)
}

/// So o casamento de termos, sem o bonus de confianca: e o que diz se uma memoria tem
/// ALGUMA coisa a ver com a pergunta. A busca filtra por ele; o compilador de contexto soma
/// a confianca por cima. Uma pontuacao so, para as duas nunca discordarem do que casa.
fn term_score(record: &MemoryRecord, query_terms: &BTreeSet<String>) -> i64 {
    let namespace = record.namespace.to_ascii_lowercase();
    let key = record.key.to_ascii_lowercase();
    let value = serde_json::to_string(&record.value)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut score = 0i64;
    for term in query_terms {
        if namespace.contains(term) {
            score += 8;
        }
        if key.contains(term) {
            score += 12;
        }
        let occurrences = value.match_indices(term).count().min(16) as i64;
        score += occurrences * 2;
    }
    score
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| part.len() >= 2)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

/// Uma memoria achada pela busca, com a pontuacao que a pos ali.
#[derive(Debug, Clone)]
pub struct MemoryHit<'a> {
    pub record: &'a MemoryRecord,
    pub score: i64,
}

/// Busca por palavras: so volta o que casa ao menos um termo, o mais pontuado primeiro e,
/// no empate, o mais novo. Memoria que nao casa nada nao volta, ao contrario do
/// `ContextCompiler`, que enche o pacote ate o teto: quem pergunta "o que sei sobre X"
/// quer ouvir "nada" quando nao ha nada.
///
/// Termo de menos de 3 letras fica fora DAQUI (o compilador de contexto continua com o
/// dele): o casamento e por substring, e "de", "do", "em" casam dentro de quase toda
/// palavra em portugues -- toda memoria voltaria para todo objetivo.
pub fn search<'a, S: MemoryStore>(store: &'a S, query: &str, limit: usize) -> Vec<MemoryHit<'a>> {
    let terms: BTreeSet<String> = tokenize(query)
        .into_iter()
        .filter(|t| t.chars().count() >= 3)
        .collect();
    let now = Utc::now();
    let mut hits: Vec<MemoryHit<'a>> = store
        .all()
        .into_iter()
        .filter(|r| r.expires_at.is_none_or(|e| e > now))
        .map(|record| MemoryHit {
            record,
            score: term_score(record, &terms),
        })
        .filter(|h| h.score > 0)
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.record.created_at.cmp(&a.record.created_at))
            .then_with(|| b.record.uuid.cmp(&a.record.uuid))
    });
    hits.truncate(limit);
    hits
}

/// Tetos do arquivo de memorias. Os dois existem porque o arquivo e lido inteiro a cada
/// tarefa: sem teto de entradas ele cresce para sempre, e sem teto por entrada uma so
/// memoria gigante ocupa o contexto que as outras deviam dividir.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryLimits {
    /// Bytes do `value` serializado de uma entrada.
    pub max_entry_bytes: usize,
    pub max_entries: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_entry_bytes: 2_000,
            max_entries: 200,
        }
    }
}

/// Memorias num arquivo JSON so, em ordem de gravacao, com a mais antiga saindo quando o
/// teto de entradas estoura. Grava por temporario + rename: processo que cai no meio deixa
/// o arquivo anterior inteiro, nunca um JSON pela metade.
///
/// Nao coordena processos: dois processos gravando o mesmo arquivo ao mesmo tempo perdem a
/// gravacao de um deles (nunca o arquivo). Quem tem varias tarefas no mesmo processo
/// serializa por fora.
#[derive(Debug)]
pub struct FileMemoryStore {
    path: std::path::PathBuf,
    limits: MemoryLimits,
    items: Vec<MemoryRecord>,
}

impl FileMemoryStore {
    /// Arquivo ausente e memoria vazia; arquivo ilegivel e erro, para nao sobrescrever com
    /// nada o que alguem gravou.
    pub fn open(
        path: impl Into<std::path::PathBuf>,
        limits: MemoryLimits,
    ) -> Result<Self, MemoryError> {
        let path = path.into();
        let items = match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            limits,
            items,
        })
    }

    pub fn limits(&self) -> MemoryLimits {
        self.limits
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Grava e devolve as que sairam pelo teto. Entrada acima do teto e recusada inteira:
    /// cortar em silencio guardaria uma memoria que diz menos do que quem gravou pensa.
    pub fn append(&mut self, record: MemoryRecord) -> Result<Vec<MemoryRecord>, MemoryError> {
        let bytes = serde_json::to_vec(&record.value)?.len();
        if bytes > self.limits.max_entry_bytes {
            return Err(MemoryError::EntryTooLarge {
                bytes,
                max: self.limits.max_entry_bytes,
            });
        }
        self.items
            .retain(|r| !(r.namespace == record.namespace && r.key == record.key));
        self.items.push(record);
        let excesso = self
            .items
            .len()
            .saturating_sub(self.limits.max_entries.max(1));
        let saidas: Vec<MemoryRecord> = self.items.drain(..excesso).collect();
        self.save()?;
        Ok(saidas)
    }

    fn save(&self) -> Result<(), MemoryError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.items)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl MemoryStore for FileMemoryStore {
    fn put(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        self.append(record).map(|_| ())
    }

    fn get(&self, namespace: &str, key: &str) -> Option<&MemoryRecord> {
        self.items
            .iter()
            .find(|r| r.namespace == namespace && r.key == key)
    }

    fn all(&self) -> Vec<&MemoryRecord> {
        self.items.iter().collect()
    }
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("memory entry has {bytes} bytes, above the limit of {max}")]
    EntryTooLarge { bytes: usize, max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_excludes_restricted_by_default() {
        let mut store = InMemoryStore::default();
        store
            .put(
                MemoryRecord::new(
                    "project",
                    "public",
                    serde_json::json!({"v": 1}),
                    MemoryScope::Project("phoenix".into()),
                    DataClassification::Internal,
                    vec![],
                )
                .unwrap(),
            )
            .unwrap();
        store
            .put(
                MemoryRecord::new(
                    "project",
                    "secret",
                    serde_json::json!({"v": 2}),
                    MemoryScope::Project("phoenix".into()),
                    DataClassification::Restricted,
                    vec![],
                )
                .unwrap(),
            )
            .unwrap();
        let pack = ContextCompiler
            .compile_for_query(
                &store,
                new_uuid_v7(),
                "public",
                &ContextPolicy::default(),
                &ContextScopeFilter {
                    project: Some("phoenix".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(pack.items.len(), 1);
    }

    #[test]
    fn query_ranks_matching_memory_first() {
        let mut store = InMemoryStore::default();
        for (key, value) in [
            (
                "rust",
                serde_json::json!({"note": "Use Result and the ? operator in Rust"}),
            ),
            ("other", serde_json::json!({"note": "CSS layout"})),
        ] {
            store
                .put(
                    MemoryRecord::new(
                        "project",
                        key,
                        value,
                        MemoryScope::Project("phoenix".into()),
                        DataClassification::Internal,
                        vec![],
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let bundle = ContextCompiler
            .compile_for_query(
                &store,
                new_uuid_v7(),
                "Rust Result error handling",
                &ContextPolicy::default(),
                &ContextScopeFilter {
                    project: Some("phoenix".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(bundle.items[0].key, "rust");
    }
    fn nota(key: &str, texto: &str) -> MemoryRecord {
        MemoryRecord::new(
            "agent",
            key,
            serde_json::json!({ "text": texto }),
            MemoryScope::Project("p".into()),
            DataClassification::Internal,
            vec![],
        )
        .unwrap()
    }

    fn arquivo() -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("phx-mem-{}", new_uuid_v7().simple()))
            .join("m.json")
    }

    #[test]
    fn teto_de_entradas_tira_a_mais_antiga_e_sobrevive_a_reabertura() {
        let p = arquivo();
        let limites = MemoryLimits {
            max_entry_bytes: 1_000,
            max_entries: 2,
        };
        let mut s = FileMemoryStore::open(&p, limites).unwrap();
        assert!(s.append(nota("a", "primeira")).unwrap().is_empty());
        assert!(s.append(nota("b", "segunda")).unwrap().is_empty());
        let saiu = s.append(nota("c", "terceira")).unwrap();
        assert_eq!(
            saiu.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["a"]
        );
        let de_novo = FileMemoryStore::open(&p, limites).unwrap();
        let chaves: Vec<_> = de_novo.all().iter().map(|r| r.key.clone()).collect();
        assert_eq!(chaves, ["b", "c"]);
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn entrada_acima_do_teto_e_recusada_inteira() {
        let p = arquivo();
        let mut s = FileMemoryStore::open(
            &p,
            MemoryLimits {
                max_entry_bytes: 40,
                max_entries: 5,
            },
        )
        .unwrap();
        let e = s.append(nota("a", &"x".repeat(100))).unwrap_err();
        assert!(matches!(e, MemoryError::EntryTooLarge { .. }), "{e}");
        assert!(s.is_empty() && !p.exists());
    }

    #[test]
    fn busca_so_volta_o_que_casa_e_o_mais_pontuado_primeiro() {
        let mut s = InMemoryStore::default();
        s.put(nota("1", "o cliente prefere relatorio em PDF"))
            .unwrap();
        s.put(nota(
            "2",
            "relatorio mensal sai em PDF, relatorio semanal em XLSX",
        ))
        .unwrap();
        s.put(nota("3", "a senha do wifi muda toda semana"))
            .unwrap();
        let achados = search(&s, "Relatorio PDF", 5);
        let chaves: Vec<_> = achados.iter().map(|h| h.record.key.as_str()).collect();
        assert_eq!(chaves, ["2", "1"]);
        assert!(search(&s, "kubernetes", 5).is_empty());
        assert_eq!(search(&s, "relatorio", 1).len(), 1);
        // "de" casaria por substring com "toda semana"? Nao: termo curto fica fora.
        assert!(search(&s, "de em do", 5).is_empty());
    }
}

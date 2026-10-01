use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Sequence,
    Function,
    Trigger,
    Index,
    Policy,
    Schema,
    Synonym,
    Event,
    Extension,
    Partition,
}

impl ObjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "Tabela",
            Self::View => "View",
            Self::MaterializedView => "Materialized View",
            Self::Sequence => "Sequence",
            Self::Function => "Função",
            Self::Trigger => "Trigger",
            Self::Index => "Índice",
            Self::Policy => "Policy/RLS",
            Self::Schema => "Schema",
            Self::Synonym => "Synonym",
            Self::Event => "Event",
            Self::Extension => "Extensão",
            Self::Partition => "Partição",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Table => "T",
            Self::View => "V",
            Self::MaterializedView => "MV",
            Self::Sequence => "SEQ",
            Self::Function => "F",
            Self::Trigger => "TR",
            Self::Index => "I",
            Self::Policy => "P",
            Self::Schema => "S",
            Self::Synonym => "SYN",
            Self::Event => "EV",
            Self::Extension => "EXT",
            Self::Partition => "PT",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckConstraint {
    pub name: Option<String>,
    pub expression: String,
    pub column: Option<String>,
    #[serde(default)]
    pub source_line: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Partitioning {
    /// range/list/hash/mysql/sqlserver/partition_of/unknown
    pub kind: String,
    pub expression: Option<String>,
    pub parent: Option<String>,
    pub bound: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableStatistics {
    pub estimated_rows: Option<u64>,
    pub data_bytes: Option<u64>,
    pub index_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_expr: Option<String>,
    pub primary_key: bool,
    pub unique: bool,
    #[serde(default)]
    pub generated_expression: Option<String>,
    #[serde(default)]
    pub generated_kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForeignKey {
    pub constraint_name: Option<String>,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub source_line: usize,
}

impl ForeignKey {
    pub fn signature(&self) -> String {
        format!(
            "{}({})->{}({})",
            canonical_name(&self.from_table),
            self.from_columns
                .iter()
                .map(|c| canonical_name(c))
                .collect::<Vec<_>>()
                .join(","),
            canonical_name(&self.to_table),
            self.to_columns
                .iter()
                .map(|c| canonical_name(c))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub migration: Option<u32>,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    #[serde(default)]
    pub check_constraints: Vec<CheckConstraint>,
    #[serde(default)]
    pub partitioning: Option<Partitioning>,
    #[serde(default)]
    pub statistics: Option<TableStatistics>,
    pub raw_sql: String,
}

impl Table {
    pub fn short_name(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    pub fn schema_name(&self) -> &str {
        self.name
            .rsplit_once('.')
            .map(|(schema, _)| schema)
            .unwrap_or("public")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SqlObject {
    pub id: usize,
    pub kind: ObjectKind,
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub migration: Option<u32>,
    pub target: Option<String>,
    pub raw_sql: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SqlStats {
    pub lines: usize,
    pub bytes: usize,
    pub tables: usize,
    pub relationships: usize,
    pub columns: usize,
    pub check_constraints: usize,
    pub generated_columns: usize,
    pub views: usize,
    pub materialized_views: usize,
    pub sequences: usize,
    pub functions: usize,
    pub triggers: usize,
    pub indexes: usize,
    pub policies: usize,
    pub schemas: usize,
    pub synonyms: usize,
    pub events: usize,
    pub extensions: usize,
    pub partitions: usize,
    pub migrations: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SqlModel {
    pub source_name: String,
    #[serde(skip)]
    pub sql: String,
    pub migrations: Vec<Migration>,
    pub tables: Vec<Table>,
    pub relationships: Vec<ForeignKey>,
    pub objects: Vec<SqlObject>,
    pub stats: SqlStats,
}

impl SqlModel {
    pub fn table_lookup(&self) -> HashMap<String, usize> {
        self.tables
            .iter()
            .enumerate()
            .map(|(i, t)| (canonical_name(&t.name), i))
            .collect()
    }

    pub fn migration_name(&self, version: u32) -> Option<&str> {
        self.migrations
            .iter()
            .find(|m| m.version == version)
            .map(|m| m.name.as_str())
    }

    pub fn object(&self, id: usize) -> Option<&SqlObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn objects_in_migration(&self, version: u32) -> Vec<&SqlObject> {
        self.objects
            .iter()
            .filter(|o| o.migration == Some(version))
            .collect()
    }

    pub fn table_index_by_name(&self, name: &str) -> Option<usize> {
        let c = canonical_name(name);
        self.tables
            .iter()
            .position(|t| canonical_name(&t.name) == c)
    }

    pub fn object_id_for_table(&self, table_name: &str) -> Option<usize> {
        let c = canonical_name(table_name);
        self.objects
            .iter()
            .find(|o| o.kind == ObjectKind::Table && canonical_name(&o.name) == c)
            .map(|o| o.id)
    }

    pub fn neighbors_of_table(&self, table_name: &str, depth: usize) -> HashSet<String> {
        let start = canonical_name(table_name);
        let mut seen = HashSet::from([start.clone()]);
        let mut frontier = HashSet::from([start]);

        for _ in 0..depth {
            let mut next = HashSet::new();
            for fk in &self.relationships {
                let a = canonical_name(&fk.from_table);
                let b = canonical_name(&fk.to_table);
                if frontier.contains(&a) && !seen.contains(&b) {
                    next.insert(b.clone());
                }
                if frontier.contains(&b) && !seen.contains(&a) {
                    next.insert(a.clone());
                }
            }
            if next.is_empty() {
                break;
            }
            seen.extend(next.iter().cloned());
            frontier = next;
        }
        seen
    }

    pub fn migration_object_counts(&self) -> BTreeMap<u32, BTreeMap<String, usize>> {
        let mut out: BTreeMap<u32, BTreeMap<String, usize>> = BTreeMap::new();
        for obj in &self.objects {
            if let Some(m) = obj.migration {
                *out.entry(m)
                    .or_default()
                    .entry(obj.kind.label().to_string())
                    .or_default() += 1;
            }
        }
        out
    }

    pub fn schemas(&self) -> BTreeSet<String> {
        self.tables
            .iter()
            .map(|t| t.schema_name().trim_matches('"').to_string())
            .collect()
    }
}

pub fn canonical_name(name: &str) -> String {
    name.split('.')
        .map(|part| part.trim().trim_matches('"').to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(".")
}

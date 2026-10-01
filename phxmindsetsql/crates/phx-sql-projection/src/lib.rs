use phx_sql_model::{canonical_name, SqlModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub migration: Option<u32>,
    pub degree: usize,
    pub columns: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub target: String,
    pub from_columns: Vec<String>,
    pub to_columns: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphProjection {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Projects the neutral model to graph primitives. No parser or renderer dependency.
pub fn project_table_graph(model: &SqlModel) -> GraphProjection {
    let mut degree: HashMap<String, usize> = HashMap::new();
    for rel in &model.relationships {
        *degree.entry(canonical_name(&rel.from_table)).or_default() += 1;
        *degree.entry(canonical_name(&rel.to_table)).or_default() += 1;
    }
    let nodes = model.tables.iter().map(|t| {
        let id = canonical_name(&t.name);
        GraphNode {
            id: id.clone(), kind: "table".to_owned(), name: t.name.clone(),
            migration: t.migration, degree: degree.get(&id).copied().unwrap_or(0),
            columns: t.columns.len(),
        }
    }).collect();
    let edges = model.relationships.iter().enumerate().map(|(i, rel)| GraphEdge {
        id: format!("fk:{i}"), kind: "foreign_key".to_owned(),
        source: canonical_name(&rel.from_table), target: canonical_name(&rel.to_table),
        from_columns: rel.from_columns.clone(), to_columns: rel.to_columns.clone(),
    }).collect();
    GraphProjection { nodes, edges }
}

import {canonicalName, assertUnifiedSqlModel} from '../model/unified-model.js';

// Pure projection layer. Receives only UnifiedSqlModel and emits graph data.
export function projectTableGraph(model) {
  assertUnifiedSqlModel(model);
  const degree = new Map();
  for (const rel of model.relationships) {
    const a = canonicalName(rel.from_table), b = canonicalName(rel.to_table);
    degree.set(a, (degree.get(a)||0)+1);
    degree.set(b, (degree.get(b)||0)+1);
  }
  const nodes = model.tables.map(table => ({
    id: canonicalName(table.name),
    kind: 'table',
    name: table.name,
    migration: table.migration ?? null,
    degree: degree.get(canonicalName(table.name)) || 0,
    columns: table.columns.length,
  }));
  const edges = model.relationships.map((rel, index) => ({
    id: `fk:${index}`,
    kind: 'foreign_key',
    source: canonicalName(rel.from_table),
    target: canonicalName(rel.to_table),
    constraint: rel.constraint ?? rel.constraint_name ?? null,
    from_columns: rel.from_columns || [],
    to_columns: rel.to_columns || [],
  }));
  return {contract_version:model.contract_version||'1.1', nodes, edges};
}

export function projectNeighborhood(model, rootName, depth=1) {
  const graph = projectTableGraph(model);
  const root = canonicalName(rootName);
  const adjacent = new Map();
  for (const e of graph.edges) {
    if (!adjacent.has(e.source)) adjacent.set(e.source,new Set());
    if (!adjacent.has(e.target)) adjacent.set(e.target,new Set());
    adjacent.get(e.source).add(e.target); adjacent.get(e.target).add(e.source);
  }
  const seen = new Set([root]); let frontier = new Set([root]);
  for (let i=0;i<depth;i++) {
    const next = new Set();
    for (const n of frontier) for (const x of adjacent.get(n)||[]) if (!seen.has(x)) next.add(x);
    for (const x of next) seen.add(x); frontier = next;
  }
  return {nodes:graph.nodes.filter(n=>seen.has(n.id)), edges:graph.edges.filter(e=>seen.has(e.source)&&seen.has(e.target))};
}

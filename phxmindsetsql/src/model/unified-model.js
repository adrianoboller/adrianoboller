export const UNIFIED_SQL_CONTRACT_VERSION = '1.1';

export function canonicalName(value='') {
  return String(value).replace(/["`\[\]]/g,'').trim().toLowerCase();
}

export function assertUnifiedSqlModel(model) {
  if (!model || typeof model !== 'object') throw new TypeError('UnifiedSqlModel ausente');
  for (const key of ['stats','migrations','tables','relationships','objects']) {
    if (!(key in model)) throw new TypeError(`UnifiedSqlModel inválido: ${key} ausente`);
  }
  if (!Array.isArray(model.tables) || !Array.isArray(model.relationships) || !Array.isArray(model.objects)) {
    throw new TypeError('UnifiedSqlModel inválido: coleções principais devem ser arrays');
  }
  for (const t of model.tables) {
    t.check_constraints ||= [];
    t.partitioning ||= null;
    t.statistics ||= null;
    for (const c of t.columns||[]) {
      c.generated_expression ??= null;
      c.generated_kind ??= null;
    }
  }
  return model;
}

export function decorateUnifiedSqlModel(model, {dialect='unknown', parser='unknown', sourceKind='sql_text'}={}) {
  assertUnifiedSqlModel(model);
  model.contract_version = UNIFIED_SQL_CONTRACT_VERSION;
  model.dialect ||= dialect;
  model.parser ||= parser;
  model.producer ||= model.parser;
  model.source_kind ||= sourceKind;
  model.capabilities ||= {};
  Object.assign(model.capabilities, {
    tables: true,
    columns: true,
    foreign_keys: true,
    check_constraints: true,
    generated_columns: true,
    views: true,
    materialized_views: dialect === 'postgresql',
    sequences: ['postgresql','sqlserver'].includes(dialect),
    functions: dialect !== 'sqlite',
    triggers: true,
    indexes: true,
    policies: dialect === 'postgresql',
    schemas: dialect !== 'sqlite',
    partitioning: ['postgresql','mysql','sqlserver'].includes(dialect),
    synonyms: dialect === 'sqlserver',
    events: dialect === 'mysql',
    extensions: dialect === 'postgresql',
    table_statistics: sourceKind === 'live_database' && dialect !== 'sqlite',
    schema_diff: true,
    ...model.capabilities,
  });
  return model;
}

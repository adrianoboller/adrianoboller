import {detectDialect} from './detector.js';
import {parsePostgreSql} from './postgresql.js';
import {parseMySql} from './mysql.js';
import {parseSqlite} from './sqlite.js';
import {parseSqlServer} from './sqlserver.js';
import {decorateUnifiedSqlModel} from '../model/unified-model.js';

export class UnsupportedDialectError extends Error {
  constructor(dialect) {
    super(`Dialeto detectado, mas parser não instalado: ${dialect}`);
    this.name = 'UnsupportedDialectError';
    this.dialect = dialect;
  }
}

const adapters = new Map();

export function registerParserAdapter(adapter) {
  if (!adapter?.dialect || typeof adapter.parse !== 'function') throw new TypeError('ParserAdapter inválido');
  adapters.set(adapter.dialect, Object.freeze({...adapter}));
}

registerParserAdapter({
  dialect: 'postgresql',
  id: 'postgresql-js-fallback',
  version: '0.5.0',
  parse(sql, source) { return parsePostgreSql(sql, source); },
});

registerParserAdapter({dialect:'mysql',id:'mysql-js-fallback',version:'0.5.0',parse(sql,source){return parseMySql(sql,source);}});
registerParserAdapter({dialect:'sqlite',id:'sqlite-js-fallback',version:'0.5.0',parse(sql,source){return parseSqlite(sql,source);}});
registerParserAdapter({dialect:'sqlserver',id:'sqlserver-js-fallback',version:'0.5.0',parse(sql,source){return parseSqlServer(sql,source);}});

export function availableDialects() { return [...adapters.keys()]; }

export function parseWithRegistry(sql, source='arquivo.sql', requestedDialect='auto') {
  const detection = requestedDialect === 'auto' ? detectDialect(sql) : {dialect: requestedDialect, confidence: 1, ranked: []};
  const dialect = detection.dialect;
  const adapter = adapters.get(dialect);
  if (!adapter) throw new UnsupportedDialectError(dialect);
  const model = decorateUnifiedSqlModel(adapter.parse(sql, source), {dialect, parser: adapter.id});
  model.detection = {dialect, confidence: detection.confidence};
  return {model, dialect, detection, parser: adapter.id};
}

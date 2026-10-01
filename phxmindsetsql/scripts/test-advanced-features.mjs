import assert from 'node:assert/strict';
import {parseWithRegistry} from '../src/parsers/registry.js';
const samples={
postgresql:`CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SEQUENCE public.order_seq;
CREATE TABLE public.orders(
 id bigint PRIMARY KEY,
 qty int CHECK(qty>0),
 total numeric GENERATED ALWAYS AS (qty*10) STORED
) PARTITION BY RANGE (id);
CREATE TABLE public.orders_p1 PARTITION OF public.orders FOR VALUES FROM (1) TO (100);
CREATE MATERIALIZED VIEW public.mv_orders AS SELECT id,total FROM public.orders;`,
mysql:`CREATE TABLE orders(
 id bigint PRIMARY KEY,
 qty int CHECK(qty>0),
 total decimal(10,2) GENERATED ALWAYS AS (qty*10) STORED
) PARTITION BY HASH(id) PARTITIONS 4;
CREATE EVENT cleanup ON SCHEDULE EVERY 1 DAY DO DELETE FROM orders WHERE id<0;`,
sqlite:`CREATE TABLE orders(
 id INTEGER PRIMARY KEY,
 qty INTEGER CHECK(qty>0),
 total REAL GENERATED ALWAYS AS (qty*10) STORED
);`,
sqlserver:`CREATE TABLE dbo.orders(
 id bigint PRIMARY KEY,
 qty int CHECK(qty>0),
 total AS (qty*10) PERSISTED
);
GO
CREATE SEQUENCE dbo.order_seq AS bigint START WITH 1;
GO
CREATE SYNONYM dbo.orders_alias FOR dbo.orders;
GO`
};
const pg=parseWithRegistry(samples.postgresql,'advanced.pg.sql','postgresql').model;
assert.equal(pg.stats.check_constraints,1);assert.equal(pg.stats.generated_columns,1);assert.equal(pg.stats.sequences,1);assert.equal(pg.stats.materialized_views,1);assert.equal(pg.stats.extensions,1);assert.equal(pg.stats.partitions,1);assert.match(pg.tables[0].columns.find(c=>c.name==='total').generated_expression,/qty\s*\*\s*10/i);
const my=parseWithRegistry(samples.mysql,'advanced.mysql.sql','mysql').model;
assert.equal(my.stats.check_constraints,1);assert.equal(my.stats.generated_columns,1);assert.equal(my.stats.events,1);assert.equal(my.stats.partitions,1);
const sq=parseWithRegistry(samples.sqlite,'advanced.sqlite.sql','sqlite').model;
assert.equal(sq.stats.check_constraints,1);assert.equal(sq.stats.generated_columns,1);assert.match(sq.tables[0].columns.find(c=>c.name==='total').generated_expression,/qty\s*\*\s*10/i);
const ms=parseWithRegistry(samples.sqlserver,'advanced.mssql.sql','sqlserver').model;
assert.equal(ms.stats.check_constraints,1);assert.equal(ms.stats.generated_columns,1);assert.equal(ms.stats.sequences,1);assert.equal(ms.stats.synonyms,1);assert.equal(ms.objects.find(o=>o.kind==='synonym').target,'dbo.orders');
console.log('advanced-features: OK — CHECK/generated/sequence/matview/partition/synonym/event/extension');

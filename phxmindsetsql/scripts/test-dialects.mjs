import assert from 'node:assert/strict';
import {detectDialect} from '../src/parsers/detector.js';
const cases=[
  ['postgresql',`CREATE EXTENSION pgcrypto; CREATE TABLE t(id uuid PRIMARY KEY, meta jsonb); CREATE POLICY p ON t USING (true);`],
  ['mysql',`CREATE TABLE \`t\` (id BIGINT AUTO_INCREMENT PRIMARY KEY) ENGINE=InnoDB;`],
  ['sqlite',`PRAGMA foreign_keys=ON; CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT);`],
  ['sqlserver',`CREATE TABLE [dbo].[t] ([id] UNIQUEIDENTIFIER DEFAULT NEWID(), [n] NVARCHAR(40)); GO`],
];
for(const [expected,sql] of cases){
  const got=detectDialect(sql);
  assert.equal(got.dialect,expected,`${expected} detectado como ${got.dialect}`);
}
console.log('dialect-test: OK — PostgreSQL / MySQL / SQLite / SQL Server');

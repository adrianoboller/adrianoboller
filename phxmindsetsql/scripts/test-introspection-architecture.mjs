import fs from 'node:fs';import path from 'node:path';import assert from 'node:assert/strict';
const root=path.resolve(process.cwd()),fail=[];
const intros=['phx-introspector-postgresql','phx-introspector-mysql','phx-introspector-sqlite','phx-introspector-sqlserver'];
for(const crate of intros){const cargo=fs.readFileSync(path.join(root,'crates',crate,'Cargo.toml'),'utf8');const lib=fs.readFileSync(path.join(root,'crates',crate,'src/lib.rs'),'utf8');for(const x of ['phx-parser-postgresql','phx-parser-mysql','phx-parser-sqlite','phx-parser-sqlserver','phx-sql-projection','eframe','egui','web-sys','wasm-bindgen'])if(cargo.includes(x)||lib.includes(x))fail.push(`${crate}: forbidden dependency ${x}`);for(const x of ['document.','window.','SVG','Canvas','Sigma','ELK'])if(lib.includes(x))fail.push(`${crate}: UI leak ${x}`)}
const apiCargo=fs.readFileSync(path.join(root,'crates/phx-sql-introspection-api/Cargo.toml'),'utf8');for(const x of ['sqlx','tiberius','axum','phx-introspector-','phx-sql-projection'])if(apiCargo.includes(x))fail.push(`introspection-api depends on ${x}`);
const projectionCargo=fs.readFileSync(path.join(root,'crates/phx-sql-projection/Cargo.toml'),'utf8');assert.ok(!projectionCargo.includes('introspect'),'projection must not know introspection');
for(const f of ['src/live-db-service.js','src/live-db-dialog.js']){const s=fs.readFileSync(path.join(root,f),'utf8');if(/from\s+['"][^'"]*parsers\//.test(s))fail.push(`${f}: imports SQL parser`)}
const app=fs.readFileSync(path.join(root,'app.js'),'utf8');for(const needle of ['sqlx','tiberius','information_schema','sys.foreign_keys','PRAGMA foreign_key_list'])if(app.includes(needle))fail.push(`app.js contains DB introspection logic: ${needle}`);

const sw=fs.readFileSync(path.join(root,'sw.js'),'utf8');if(!sw.includes("u.pathname.startsWith('/api/')"))fail.push('service worker must bypass /api/');
if(fail.length){console.error(fail.join('\n'));process.exit(1)}console.log('introspection-architecture: OK — Live DB ↔ Model ↔ Projection ↔ Renderer boundaries isolated');

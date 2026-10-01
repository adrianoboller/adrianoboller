import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
const root=path.resolve(process.cwd());
const expected=JSON.parse(fs.readFileSync(path.join(root,'tests/renderer_core_v05_sha256.json'),'utf8'));
const app=fs.readFileSync(path.join(root,'app.js'),'utf8');
function extract(name){const re=new RegExp(`(?:async\\s+)?function\\s+${name}\\s*\\(`);const m=re.exec(app);assert.ok(m,`${name} missing`);const start=m.index,brace=app.indexOf('{',m.index+m[0].length);let depth=0,quote=null,esc=false;for(let i=brace;i<app.length;i++){const c=app[i];if(quote){if(esc)esc=false;else if(c==='\\\\')esc=true;else if(c===quote)quote=null;}else if(c==='"'||c==="'"||c==='`')quote=c;else if(c==='{')depth++;else if(c==='}'){depth--;if(depth===0)return app.slice(start,i+1);}}throw new Error(`${name} unclosed`)}
for(const [name,hash] of Object.entries(expected)){const actual=crypto.createHash('sha256').update(extract(name)).digest('hex');assert.equal(actual,hash,`${name} renderer changed`)}
const enhancer=crypto.createHash('sha256').update(fs.readFileSync(path.join(root,'src/enhancer.js'))).digest('hex');
assert.equal(enhancer,'21076a9e2525b92726aae8dc4689a24a6be9da82c91d0bf0926f4e37eeb8ddc5','enhancer changed');
const projection=crypto.createHash('sha256').update(fs.readFileSync(path.join(root,'src/projections/graph-projection.js'))).digest('hex');
assert.equal(projection,'46f0c3859cf31ee8bd21826106b0270d3dcfd8233c45d76dae5eec6155bcf5cc','projection changed');
console.log('renderer-immutability: OK — renderer core identical to v0.5; live DB added outside graphics');

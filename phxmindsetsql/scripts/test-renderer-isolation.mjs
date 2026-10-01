import assert from 'node:assert/strict';
import fs from 'node:fs';
for(const file of ['app.js','src/enhancer.js']){
  const s=fs.readFileSync(new URL('../'+file,import.meta.url),'utf8');
  assert.doesNotMatch(s,/from\s+['"].*parsers\//i,`${file} imports parser`);
  assert.doesNotMatch(s,/CREATE\\s|ALTER\\s|REFERENCES\\s|INFORMATION_SCHEMA|pg_catalog|sys\./i,`${file} contains SQL/catalog parsing rules`);
}
const diff=fs.readFileSync(new URL('../src/schema-diff.js',import.meta.url),'utf8');
assert.doesNotMatch(diff,/document\.|querySelector|canvas|getContext|svg/i,'schema diff contains renderer/UI code');
console.log('renderer-isolation: OK — renderers consume only normalized model/projections');

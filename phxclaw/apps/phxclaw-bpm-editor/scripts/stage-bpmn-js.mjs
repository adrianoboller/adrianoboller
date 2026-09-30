import { cp, mkdir, rm } from 'node:fs/promises';
const out = new URL('../vendor/', import.meta.url); await rm(out,{recursive:true,force:true}); await mkdir(out,{recursive:true});
await cp(new URL('../node_modules/bpmn-js/dist/',import.meta.url),out,{recursive:true});
console.log('bpmn-js staged to vendor/');

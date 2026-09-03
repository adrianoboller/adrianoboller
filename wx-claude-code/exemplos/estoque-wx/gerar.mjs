// Gera os anexos do projeto de exemplo ESTOQUE a partir de fontes/: os quatro
// PDFs no formato da documentacao tecnica do WINDEV (pesquisaveis, texto de
// verdade) e os screenshots das janelas em cada estado.
// Uso: node gerar.mjs   (a partir desta pasta; NODE_PATH apontando para onde o Playwright esta instalado)
import { createRequire } from 'node:module';
const { chromium } = createRequire(import.meta.url)('playwright'); // require respeita NODE_PATH; import ESM nao
import { readFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const fontes = join(aqui, 'fontes');
const inputs = join(aqui, 'inputs');
mkdirSync(join(inputs, 'screenshots'), { recursive: true });

const b = await chromium.launch();
const p = await b.newPage();
const rodape = (t) => `<div style="font-family:Arial;font-size:8px;color:#666;width:100%;padding:0 12mm;display:flex;justify-content:space-between"><span>${t}</span><span>Página <span class="pageNumber"></span> de <span class="totalPages"></span></span></div>`;
async function pdf(html, out, titulo) {
  await p.setContent('<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"></head><body>' + html + '</body></html>');
  await p.pdf({ path: out, format: 'A4', printBackground: true, margin: { top: '16mm', bottom: '16mm', left: '14mm', right: '14mm' }, displayHeaderFooter: true, headerTemplate: '<span></span>', footerTemplate: rodape(titulo) });
  console.log('pdf', out);
}
const codigo = readFileSync(join(fontes, 'codigo.html'), 'utf8');
const telas = readFileSync(join(fontes, 'telas.html'), 'utf8');
const queries = readFileSync(join(fontes, 'queries.html'), 'utf8');
await pdf(codigo, join(inputs, 'estoque-codigo.pdf'), 'Projeto ESTOQUE · Documentação técnica · Código');
await pdf(telas, join(inputs, 'estoque-interfaces.pdf'), 'Projeto ESTOQUE · Documentação técnica · Interfaces');
await pdf(queries, join(inputs, 'estoque-queries.pdf'), 'Projeto ESTOQUE · Documentação técnica · Queries');
const capa = '<h1 style="font-family:Arial;font-size:26pt;margin-top:200px">Projeto ESTOQUE</h1><p style="font-family:Arial;font-size:14pt">Documentação técnica completa<br>WINDEV 2025 · Update 1 · 03/09/2026</p><p style="font-family:Arial;font-size:11pt;color:#555">Sumário: 1. Código · 2. Interfaces · 3. Queries</p><div style="page-break-after:always"></div>';
const quebra = '<div style="page-break-after:always"></div>';
await pdf(capa + codigo + quebra + telas + quebra + queries, join(inputs, 'estoque-completo.pdf'), 'Projeto ESTOQUE · Documentação técnica completa');

await p.setViewportSize({ width: 940, height: 700 });
await p.setContent('<!doctype html><html><head><meta charset="utf-8"></head><body>' + readFileSync(join(fontes, 'telas-render.html'), 'utf8') + '</body></html>');
for (const id of ['win-venda-com-itens', 'win-venda-vazia', 'win-venda-erro-estoque', 'win-cliente-normal']) {
  const out = join(inputs, 'screenshots', id + '.png');
  await p.locator('#' + id).screenshot({ path: out });
  console.log('png', out);
}
await b.close();

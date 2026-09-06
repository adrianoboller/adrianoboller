// Renderiza UM print a partir de uma captura de texto real (a saida de uma
// sessao ou de um script do plugin), num terminal desenhado pelo Chromium a 2x.
//
// Existe porque os 62 primeiros prints foram renderizados por script ad hoc,
// que nao ficou versionado: o proximo que precisasse de um print teria de
// reinventar a folha de estilo, e o print sairia com outra cara. Cores e fonte
// sao as mesmas do gravador de video, para os dois combinarem.
//
// Uso: node render-print.mjs <captura.txt> <saida.png> "<titulo>" "<legenda>"
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync } from 'node:fs';

const [, , capturaPath, saida, titulo = '', legenda = ''] = process.argv;
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
// Caminhos de scratchpad viram "." -- o print mostra o comando, nao a maquina
const texto = readFileSync(capturaPath, 'utf8')
  .replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/[A-Za-z0-9_-]+/g, '.');

function fmt(l) {
  l = esc(l);
  if (/^\$ /.test(l)) return `<span class="prompt">$</span> <span class="cmd">${l.slice(2)}</span>`;
  l = l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
  if (/^#{1,3} /.test(l)) return `<span class="h">${l.replace(/^#+ /, '')}</span>`;
  // O ambar vem ANTES do verde: "PRONTO EM PARTE" casava com /PRONTO/ e saia
  // verde inteiro -- pintando de aprovado exatamente o veredito que e ressalva.
  if (/TIER 3|tier 3|INDISPONÍVEL|SEM ALVO|EM PARTE|atenção|ressalva|NÃO|NAO e/.test(l)) return `<span class="warn">${l}</span>`;
  if (/PRONTO|std pré-compilada|READY|"valid": true|√/.test(l)) return `<span class="ok">${l}</span>`;
  return l;
}

const html = `<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;background:#0b0d17;font-family:"DejaVu Sans Mono",Menlo,monospace}
.win{display:flex;flex-direction:column;min-height:100vh}
.bar{height:38px;flex:none;background:#141830;display:flex;align-items:center;padding:0 14px;gap:8px;color:#9aa0b8;font-size:13px}
.dot{width:12px;height:12px;border-radius:50%}.t{margin-left:12px}.brand{margin-left:auto;color:#E2261C;font-weight:700}
pre{margin:0;padding:16px 22px;color:#e6e8f2;font-size:14px;line-height:1.45;white-space:pre-wrap;word-break:break-word;flex:1}
.prompt{color:#2FBF71;font-weight:700}.cmd{color:#fff;font-weight:600}.h{color:#F7B733;font-weight:700}
.ok{color:#2FBF71}.warn{color:#F5A15A}b{color:#fff}code{color:#8fd3ff}
.cap{padding:10px 40px 16px;color:#c7cbe0;font-size:14px;text-align:center}
</style><div class="win">
<div class="bar"><span class="dot" style="background:#ff5f57"></span><span class="dot" style="background:#febc2e"></span>
<span class="dot" style="background:#28c840"></span><span class="t">${esc(titulo)}</span><span class="brand">WX Claude Code</span></div>
<pre>${texto.split('\n').map(fmt).join('\n')}</pre>
${legenda ? `<div class="cap">${esc(legenda)}</div>` : ''}</div>`;

const navegador = await chromium.launch();
const pagina = await navegador.newPage({ viewport: { width: 1100, height: 700 }, deviceScaleFactor: 2 });
await pagina.setContent(html);
await pagina.screenshot({ path: saida, fullPage: true });
await navegador.close();
console.log(`ok ${saida}`);

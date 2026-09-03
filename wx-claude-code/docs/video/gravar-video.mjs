// Grava um video de uso a partir das capturas reais (caps/*.txt): terminal
// animado com comando digitado e saida revelada linha a linha. Playwright grava
// em WebM (VP8), que e o que o ffmpeg do Playwright sabe codificar.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, readdirSync, renameSync, rmSync } from 'node:fs';

const [, , outDir, capsDir] = process.argv;
const cap = (n) => readFileSync(`${capsDir}/${n}.txt`, 'utf8').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/(proj|pmo2|demo)/g, '.');
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
function fmt(line) {
  let l = esc(line);
  if (/^\$ /.test(l)) return `<span class="prompt">$</span> <span class="cmd">${l.slice(2)}</span>`;
  if (/^&gt; /.test(l)) return `<span class="prompt">&gt;</span> <span class="cmd">${l.slice(5)}</span>`;
  l = l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
  if (/^#{1,3} /.test(l)) return `<span class="h">${l.replace(/^#+ /, '')}</span>`;
  if (/CREATED|√|"valid": true|READY|frutifero\)|sim$/.test(l)) return `<span class="ok">${l}</span>`;
  if (/BLOCKED|erro|Erros|INVALID|MISSING|×|infrutifero\)|ESTOURADO/.test(l)) return `<span class="warn">${l}</span>`;
  return l;
}
// cenas: [titulo, legenda, texto, comandoDigitado?, maxLinhas?]
const scenes = [
  ['card', 'WX Claude Code', 'Conversão governada de projetos WINDEV, WEBDEV e WINDEV Mobile\nQuestionário A–J · Gates G0–G7 · Equipe WLanguage sobre o Help da PC SOFT · PMO com Scrum, Kanban e PDCA\n\nTudo que aparece a seguir é saída real de sessões do Claude Code e dos scripts do plugin.'],
  ['claude plugin validate', '1 · Instalar e validar o plugin', cap('validate')],
  ['/wx-claude-code:questionario', '2 · O questionário A–J: o plugin pergunta antes de converter', cap('questionario'), '/wx-claude-code:questionario ./meu-projeto'],
  ['aplicar_questionario.py + wx_preflight.py', '3 · As respostas viram manifesto; o Gate G0 confere cada anexo', cap('preflight').split('\n').slice(60).join('\n')],
  ['query_wlanguage_help.py', '4 · O corpus WLanguage 12k, verificado por hash e consultado por tema', cap('help').split('\n').slice(0, 24).join('\n') + '\n…'],
  ['subagentes wl-*-specialist', '5 · Cada símbolo vai ao especialista WLanguage do tema certo do Help', cap('equipe')],
  ['/wx-claude-code:pmo', '6 · PMO: sprint Scrum, ciclos PDCA e a base de conhecimento', cap('pmo2').split('\n').slice(0, 33).join('\n')],
  ['pmo.py kanban', '7 · Kanban gerado da matriz, com limite de WIP', cap('pmo2').split('\n').slice(33, 65).join('\n')],
  ['/wx-claude-code:pmo status', '8 · O agente do PMO lê o painel e aponta o que trava', cap('pmo-sessao')],
  ['/wx-claude-code:laudo-tokens', '9 · Laudo de uso de tokens: somente leitura, MEDIDO ou INDISPONÍVEL', cap('laudo'), '/wx-claude-code:laudo-tokens fase-1'],
  ['card', 'Built to convert. Engineered to prove.', 'claude plugin marketplace add adrianoboller/adrianoboller\nclaude plugin install wx-claude-code@wx-claude-code\n\nManual completo em MANUAL.md'],
];
const html = `<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;height:100%;background:#0b0d17;font-family:"DejaVu Sans Mono",Menlo,monospace;overflow:hidden}
.win{position:absolute;inset:26px 40px 44px 40px;border-radius:12px;overflow:hidden;background:#010418;border:1px solid #232742;box-shadow:0 20px 60px #0009;display:flex;flex-direction:column}
.bar{height:38px;flex:none;background:#141830;display:flex;align-items:center;padding:0 14px;gap:8px;color:#9aa0b8;font-size:13px}
.dot{width:12px;height:12px;border-radius:50%}.t{margin-left:12px}.brand{margin-left:auto;color:#E2261C;font-weight:700}
pre{margin:0;padding:16px 22px;color:#e6e8f2;font-size:14px;line-height:1.45;white-space:pre-wrap;word-break:break-word;flex:1;overflow:hidden}
.prompt{color:#2FBF71;font-weight:700}.cmd{color:#fff;font-weight:600}.h{color:#F7B733;font-weight:700}.ok{color:#2FBF71}.warn{color:#F5A15A}b{color:#fff}code{color:#8fd3ff}
.cap{position:absolute;left:40px;right:40px;bottom:10px;color:#c7cbe0;font-size:14px;text-align:center}
.card{position:absolute;inset:0;background:#010418;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;color:#fff;padding:60px}
.card h1{font-size:44px;margin:0 0 18px;color:#E2261C;letter-spacing:1px}.card p{font-size:18px;line-height:1.6;color:#c7cbe0;white-space:pre-wrap;margin:0}
.cursor{display:inline-block;width:9px;height:16px;background:#2FBF71;vertical-align:-2px}
</style><div id="root"></div>`;

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: outDir, size: { width: 1280, height: 720 } } });
const page = await ctx.newPage();
await page.setContent(html);
const sleep = (ms) => page.waitForTimeout(ms);
for (const [title, caption, text, typed] of scenes) {
  if (title === 'card') {
    await page.evaluate(([h, p]) => { document.getElementById('root').innerHTML = `<div class="card"><h1>${h}</h1><p>${p}</p></div>`; }, [esc(caption), esc(text)]);
    await sleep(4500); continue;
  }
  await page.evaluate(([t, c]) => { document.getElementById('root').innerHTML = `<div class="win"><div class="bar"><span class="dot" style="background:#ff5f57"></span><span class="dot" style="background:#febc2e"></span><span class="dot" style="background:#28c840"></span><span class="t">${t}</span><span class="brand">WX CLAUDE CODE</span></div><pre id="pre"></pre></div><div class="cap">${c}</div>`; }, [esc(title), esc(caption)]);
  const lines = text.split('\n');
  if (typed) {
    let s = '';
    for (const ch of typed) { s += ch; await page.evaluate((h) => { document.getElementById('pre').innerHTML = h; }, `<span class="prompt">&gt;</span> <span class="cmd">${esc(s)}</span><span class="cursor"></span>`); await sleep(28); }
    await sleep(700);
    lines.unshift(`> ${typed}`, '');
  }
  let acc = [];
  for (let i = 0; i < lines.length; i++) {
    acc.push(fmt(lines[i]));
    const isCmd = /^[$>] /.test(lines[i]);
    await page.evaluate((h) => { const p = document.getElementById('pre'); p.innerHTML = h; p.scrollTop = p.scrollHeight; }, acc.join('\n'));
    // rolagem: mantem as ultimas ~30 linhas visiveis
    if (acc.length > 30) { acc = acc.slice(-30); await page.evaluate((h) => { document.getElementById('pre').innerHTML = h; }, acc.join('\n')); }
    await sleep(isCmd ? 900 : Math.min(160, 40 + lines[i].length));
  }
  await sleep(3200);
}
await ctx.close(); await browser.close();
const f = readdirSync(outDir).find((n) => n.endsWith('.webm'));
renameSync(`${outDir}/${f}`, `${outDir}/wx-claude-code-video-de-uso.webm`);
console.log('ok');

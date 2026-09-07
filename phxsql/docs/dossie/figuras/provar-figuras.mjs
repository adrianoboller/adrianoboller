// Prova as figuras desta pasta no NAVEGADOR, nos dois temas, e grava as
// capturas em docs/dossie/capturas/.
//
//     node docs/dossie/figuras/provar-figuras.mjs
//
// Existe porque INTERFACE SO SE PROVA EXERCITANDO, e figura e interface. Ler o
// fonte de um SVG nao acha texto que sai do viewBox, rotulo por cima de rotulo,
// nem legenda riscada pela borda de uma caixa. As tres conferencias aqui saem
// de defeito pago: na primeira prova destas duas figuras havia 1 texto fora do
// viewBox, 3 sobrepostos (entre eles um titulo de secao com uma nota por cima)
// e 1 encostado numa borda -- e nenhum deles aparecia lendo o arquivo.
//
// As variaveis de cor sao as MESMAS do dossie, porque e la que as figuras
// vivem; o fundo escuro e o da marca, #010418. Os SVG usam var(--x, #fallback),
// entao tambem se abrem sozinhos, fora do dossie, sem perder a cor.
const { chromium } = await import('/opt/node22/lib/node_modules/playwright/index.mjs');
import { readFileSync, mkdirSync } from 'node:fs';

const RAIZ = '/home/user/adrianoboller/phxsql';
const FIGS = [
  ['autonumber-como-esta', 'O fluxo dos tres numeros, como esta'],
  ['autonumber-ideal', 'O ideal, e a linha de corte da migracao'],
];

// As MESMAS variaveis do dossie -- a figura foi feita para viver la dentro.
const CLARO = `--papel:#fbf9f7;--tinta:#1a1210;--tinta-2:#4a3f3a;--acento:#c63c0a;
  --reg:#1f5c93;--ndx:#6a44a8;--bin:#0e7a85;--memo:#37702e;--log:#b71414;
  --ok:#2f7a3e;--pend:#8a6a1f;`;
// O fundo escuro e o da MARCA: #010418.
const ESCURO = `--papel:#010418;--tinta:#dde2eb;--tinta-2:#a8b0c0;--acento:#ff8a1c;
  --reg:#5fa6e8;--ndx:#b394f0;--bin:#3fc8d4;--memo:#7bcb6a;--log:#ff5f5f;
  --ok:#6cc98c;--pend:#ffc43d;`;

const pagina = (svg, vars) => `<!doctype html><html><head><meta charset="utf-8">
<style>
  :root{${vars}}
  html,body{margin:0;background:var(--papel);color:var(--tinta-2)}
  .caixa{padding:24px;max-width:1180px;margin:0 auto}
  svg{width:100%;height:auto;display:block}
</style></head><body><div class="caixa">${svg}</div></body></html>`;

mkdirSync(`${RAIZ}/docs/dossie/capturas`, { recursive: true });
const nav = await chromium.launch({ args: ['--no-sandbox'] });
for (const [nome, titulo] of FIGS) {
  const svg = readFileSync(`${RAIZ}/docs/dossie/figuras/${nome}.svg`, 'utf8');
  for (const [tema, vars] of [['claro', CLARO], ['escuro', ESCURO]]) {
    const p = await nav.newPage({ viewport: { width: 1240, height: 900 },
                                  deviceScaleFactor: 2 });
    const erros = [];
    p.on('pageerror', e => erros.push(String(e)));
    p.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
    await p.setContent(pagina(svg, vars), { waitUntil: 'load' });
    // O que so o navegador sabe: a caixa REAL de cada texto contra o viewBox.
    const fora = await p.evaluate(() => {
      const svg = document.querySelector('svg');
      const vb = svg.viewBox.baseVal;
      const ruins = [];
      for (const n of svg.querySelectorAll('text,rect,path,circle,line')) {
        const b = n.getBBox();
        if (b.x < vb.x - 0.5 || b.y < vb.y - 0.5 ||
            b.x + b.width > vb.x + vb.width + 0.5 ||
            b.y + b.height > vb.y + vb.height + 0.5) {
          ruins.push(`${n.tagName} "${(n.textContent || '').slice(0, 40)}" ` +
                     `x=${b.x.toFixed(1)} y=${b.y.toFixed(1)} ` +
                     `w=${b.width.toFixed(1)} h=${b.height.toFixed(1)}`);
        }
      }
      return ruins;
    });
    // E o que SOBREPOE: dois textos no mesmo lugar sao ilegiveis, e ler o
    // fonte nao acha -- a primeira prova destas figuras achou tres, entre eles
    // um rotulo de secao com uma nota por cima.
    const colados = await p.evaluate(() => {
      const t = [...document.querySelectorAll('svg text')].map(n => ({
        txt: (n.textContent || '').slice(0, 34), b: n.getBBox() }));
      const ruins = [];
      for (let i = 0; i < t.length; i++)
        for (let j = i + 1; j < t.length; j++) {
          const a = t[i].b, c = t[j].b;
          const dx = Math.min(a.x + a.width, c.x + c.width) - Math.max(a.x, c.x);
          const dy = Math.min(a.y + a.height, c.y + c.height) - Math.max(a.y, c.y);
          if (dx > 0.5 && dy > 0.5) ruins.push(`"${t[i].txt}" X "${t[j].txt}"`);
        }
      return ruins;
    });
    // Texto ENCOSTANDO na borda de uma caixa: ou esta todo dentro, ou todo
    // fora. Meio dentro e o rotulo riscado pela linha -- foi o que a segunda
    // prova destas figuras achou em quatro lugares.
    const naBorda = await p.evaluate(() => {
      const dentro = (a, r, m) => a.x >= r.x + m && a.y >= r.y + m &&
        a.x + a.width <= r.x + r.width - m && a.y + a.height <= r.y + r.height - m;
      const cruza = (a, r) => Math.min(a.x + a.width, r.x + r.width) > Math.max(a.x, r.x) &&
        Math.min(a.y + a.height, r.y + r.height) > Math.max(a.y, r.y);
      const rects = [...document.querySelectorAll('svg rect')].map(n => n.getBBox());
      const ruins = [];
      for (const t of document.querySelectorAll('svg text')) {
        const a = t.getBBox();
        for (const r of rects)
          if (cruza(a, r) && !dentro(a, r, 2))
            ruins.push(`"${(t.textContent || '').slice(0, 38)}" na borda de ` +
                       `rect ${r.x.toFixed(0)},${r.y.toFixed(0)} ${r.width.toFixed(0)}x${r.height.toFixed(0)}`);
      }
      return ruins;
    });

    // A captura e do ELEMENTO, e nao da pagina: pagina inteira sobra papel
    // embaixo e some com a proporcao real da figura.
    const alvo = `${RAIZ}/docs/dossie/capturas/${nome}-${tema}.png`;
    await p.locator('svg').screenshot({ path: alvo });
    console.log(`${nome} ${tema}: ${fora.length} fora do viewBox, ` +
                `${colados.length} sobrepostos, ${naBorda.length} na borda` +
                (erros.length ? ` | ERROS: ${erros.join(' ; ')}` : ''));
    for (const f of fora) console.log('    fora:', f);
    for (const c of colados) console.log('    colado:', c);
    for (const b of naBorda) console.log('    borda:', b);
    await p.close();
  }
}
await nav.close();

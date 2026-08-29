#!/usr/bin/env node
/* De que largura uma REGIAO deixa de servir?
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/medir-regiao.mjs
 *
 * O `MIN_REGIAO` de `ui/multitela.js` decide quantas regioes cabem lado a
 * lado, e numero digitado a mao envelhece calado. Este medidor estreita a
 * regiao de 20 em 20 px e pergunta AO NAVEGADOR a partir de que largura o
 * conteudo passa a exigir rolagem lateral.
 *
 * A rolagem que conta nao e so a do `#painel`: as tabelas moram dentro de
 * `.rolo`, que rola por dentro. Uma grade que so se le arrastando de lado nao
 * "cabe" -- ela so nao transborda. Por isso o medidor olha TODOS os
 * contêineres, e nao so o de fora.
 *
 * O resultado de hoje, e a decisao que saiu dele, estao em
 * `docs/MULTITELA.md`. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { entrar, cenario } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

const TELAS = [
  ['Consulta (Query)', () => PhxTelas.abrir('query', {})],
  ['Diagrama ER', () => PhxTelas.abrir('diagrama', { db: 'medida' })],
  ['Telemetria', () => PhxTelas.abrir('telemetria', {})],
  ['Profiler', () => PhxTelas.abrir('profiler', {})],
  ['Conteudo de uma tabela', null],
];

const servidor = await subir({
  phxsqld: join(RAIZ, 'target', 'release', 'phxsqld'),
  portaDados: 6570, portaWeb: 6571,
  log: m => console.log('·', m),
});
const navegador = await chromium.launch({ headless: true });
const ctx = await navegador.newContext({ viewport: { width: 1900, height: 950 } });
await ctx.route(
  u => /fonts\.(googleapis|gstatic)\.com/.test(typeof u === 'string' ? u : u.href),
  r => r.abort());
const page = await ctx.newPage();

try {
  await entrar(page, servidor.url);
  await cenario(page, 'medida', 'clientes');
  await cenario(page, 'medida', 'pedidos');

  console.log('\n  tela                     cabe ate');
  console.log('  ------------------------ --------');
  for (let i = 0; i < TELAS.length; i++) {
    await page.evaluate(async k => {
      const fns = [
        () => PhxTelas.abrir('query', {}),
        () => PhxTelas.abrir('diagrama', { db: 'medida' }),
        () => PhxTelas.abrir('telemetria', {}),
        () => PhxTelas.abrir('profiler', {}),
        async () => {
          await PhxTelas.abrir('tabela', { db: 'medida', tab: 'clientes' });
          await irAba('conteudo');
        },
      ];
      await fns[k]();
    }, i);
    await page.waitForTimeout(1400);

    const cabe = await page.evaluate(() => {
      const reg = document.querySelector('.regiao');
      const antes = reg.style.cssText;
      let limite = null;
      for (let w = 1400; w >= 260; w -= 20) {
        reg.style.flex = `0 0 ${w}px`;
        reg.offsetWidth;                       // forca o recalculo do layout
        const p = reg.querySelector('.painel');
        let sobra = p.scrollWidth - p.clientWidth;
        for (const e of p.querySelectorAll('*')) {
          const d = e.scrollWidth - e.clientWidth;
          if (d > sobra) sobra = d;
        }
        if (sobra > 1 && limite === null) limite = w + 20;
      }
      reg.style.cssText = antes;
      return limite;
    });
    console.log(`  ${TELAS[i][0].padEnd(24)} ${cabe === null ? '< 260' : cabe} px`);
  }
  console.log('\n  MIN_REGIAO usa o pior caso das QUATRO TELAS NOMEADAS.');
  console.log('  A grade fica de fora: tabela larga rola de lado em qualquer');
  console.log('  largura, e e para isso que existe o `.rolo`.\n');
} finally {
  await navegador.close();
  await servidor.derrubar();
}

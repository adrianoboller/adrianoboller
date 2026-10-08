/* Prova pelo navegador dos pedidos 689-694 -- os seis achados de tela do
 * video do CRUD (papel E, 08/10/2026). Roda nos DOIS temas.
 *
 *   node testes-web/prova-689-694-tela.mjs [caminho/do/phxsqld]   (da pasta phxsql/)
 *
 * O que cada passo MEDE, e nao so olha:
 *   689  contraste do texto sobre a linha em hover, na coluna rowid (a
 *        congelada, que tinha cor propria cravada) e numa celula comum --
 *        piso 4,5:1, calculado pela formula da WCAG sobre a cor COMPOSTA
 *        (alfa incluido), e nao sobre o token;
 *   690  o `innerText` da legenda do motivo -- o Chromium aplica o
 *        `text-transform` no innerText, entao «.REASON» aparece ali;
 *   691  a Exo 2 carregada com a internet CORTADA (toda origem de fora e
 *        abortada pela rota), por `document.fonts.check`;
 *   692  a caption declarada no cabecalho das duas grades e no rotulo da ficha;
 *   693  o tipo na ficha sai `Decimal(15,2)`, e nao o `Debug` do Rust;
 *   694  o subtitulo da Nova tabela nao afirma numero de arquivos.
 *
 * Sai com codigo 1 se qualquer medida reprovar. Capturas em $SAIDA. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { subir } from './servidor.mjs';
import { entrar, api, abrirLinhaDaGrade } from './apoio.mjs';

const PHXSQLD = resolve(process.argv[2] || 'target/debug/phxsqld');
const SAIDA = process.env.SAIDA || '/tmp/prova-689-694';
mkdirSync(SAIDA, { recursive: true });

const reprovas = [];
const nota = (ok, oQue, valor) => {
  console.log(`${ok ? 'ok ' : 'XX '} ${oQue}: ${valor}`);
  if (!ok) reprovas.push(oQue);
};

/* Contraste WCAG sobre a cor que o olho ve: compoe o fundo de baixo para
 * cima, alfa incluido, ate o primeiro opaco. */
const MEDIR = `(() => {
  const rgba = s => { const m = s.match(/[\\d.]+/g) || [0,0,0,0];
    return [+m[0], +m[1], +m[2], m[3] === undefined ? 1 : +m[3]]; };
  window.__fundo = el => { const pilha = [];
    for (let e = el; e; e = e.parentElement) {
      const c = rgba(getComputedStyle(e).backgroundColor);
      if (c[3] > 0) pilha.push(c); if (c[3] >= 1) break; }
    let r = [255,255,255];
    for (const c of pilha.reverse()) r = r.map((v,i) => v*(1-c[3]) + c[i]*c[3]);
    return r; };
  const lum = c => { const f = v => { v/=255; return v <= .03928 ? v/12.92 : ((v+.055)/1.055)**2.4; };
    return .2126*f(c[0]) + .7152*f(c[1]) + .0722*f(c[2]); };
  window.__contraste = el => { const t = rgba(getComputedStyle(el).color), b = __fundo(el);
    const tc = t.slice(0,3).map((v,i) => v*t[3] + b[i]*(1-t[3]));
    const [a, z] = [lum(tc), lum(b)].sort((x,y) => y-x); return (a+.05)/(z+.05); };
})()`;

const servidor = await subir({ phxsqld: PHXSQLD, portaDados: 6340, portaWeb: 6341, log: console.log });
const nav = await chromium.launch();
try {
  for (const tema of ['escuro', 'claro']) {
    console.log(`\n== tema ${tema}`);
    const ctx = await nav.newContext({ viewport: { width: 1280, height: 800 } });
    // Sem internet: tudo que nao e o proprio servidor morre na rota.
    await ctx.route(u => !u.href.startsWith(servidor.url), r => r.abort());
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, tema);
    const page = await ctx.newPage();
    await entrar(page, servidor.url);
    await page.evaluate(MEDIR);

    // 691 -- a fonte da marca, com a rede de fora cortada.
    await page.evaluate(() => document.fonts.ready);
    const exo = await page.evaluate(async () => {
      await document.fonts.load('600 16px "Exo 2"').catch(() => {});
      return document.fonts.check('600 16px "Exo 2"')
        && [...document.fonts].some(f => f.family.replace(/"/g, '') === 'Exo 2' && f.status === 'loaded');
    });
    nota(exo, '691 Exo 2 carregada sem internet', exo);

    const db = `p689${tema[0]}`;
    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', { database: db, tabela: 'clientes', colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true, caption: 'Código' },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true, caption: 'Nome do cliente' },
      { nome: 'cidade', tipo: 'Str(30)' },
      { nome: 'limite', tipo: 'Decimal(15,2)', caption: 'Limite de crédito' },
    ], indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }] });
    for (const l of [[1, 'Ana Moreira', 'Blumenau', '1500.00'], [2, 'Carlos Lima', 'Joinville', '2750.50']]) {
      await api(page, 'inserir', { database: db, tabela: 'clientes', valores: l });
    }

    // 692 + 689 -- a grade editavel.
    await page.evaluate(([d]) => verConteudoEditavel(d, 'clientes'), [db]);
    await page.waitForSelector('#gradeEdit .phx-grid tbody tr');
    const titulos = await page.$$eval('#gradeEdit .phx-th-titulo', ths => ths.map(t => t.innerText.trim()));
    nota(titulos.some(t => t.startsWith('Limite de crédito')) && titulos.some(t => t.startsWith('Código')),
      '692 caption no cabecalho da grade editavel', JSON.stringify(titulos));
    const linha = page.locator('#gradeEdit .phx-grid tbody tr').first();
    await linha.locator('td').nth(1).hover();
    await page.waitForTimeout(150);
    const ctr = await page.evaluate(() => {
      const tr = document.querySelector('#gradeEdit .phx-grid tbody tr:hover');
      const fixa = tr.querySelector('td[data-fx]') || tr.querySelector('td');
      const comum = tr.querySelectorAll('td')[2];
      return { fixa: __contraste(fixa), comum: __contraste(comum),
        fundoFixa: __fundo(fixa).map(Math.round).join(','), fundoComum: __fundo(comum).map(Math.round).join(',') };
    });
    nota(ctr.fixa >= 4.5, '689 contraste do rowid em hover', `${ctr.fixa.toFixed(2)}:1 sobre rgb(${ctr.fundoFixa})`);
    nota(ctr.comum >= 4.5, '689 contraste da celula em hover', `${ctr.comum.toFixed(2)}:1 sobre rgb(${ctr.fundoComum})`);
    await page.screenshot({ path: join(SAIDA, `${tema}-grade-hover.png`) });

    // 693 + 692 -- a ficha.
    await abrirLinhaDaGrade(page, { em: '#painel' });
    await page.waitForSelector('#f_limite');
    const rot = await page.$eval('#f_limite', i => i.closest('label').innerText.replace(/\s+/g, ' ').trim());
    nota(/Decimal\(15,2\)/.test(rot) && !/precisao/i.test(rot), '693 tipo na ficha', rot);
    nota(rot.includes('Limite de crédito'), '692 caption na ficha', rot);
    await page.screenshot({ path: join(SAIDA, `${tema}-ficha.png`) });

    // 690 -- o dialogo de excluir.
    await page.evaluate(([d]) => dialogoExcluir(d, 'clientes', 1, () => {}, 0), [db]);
    await page.waitForSelector('#excMotivo');
    const leg = await page.$eval('#excMotivo', i => i.closest('label').querySelector('.leg').innerText);
    nota(leg.includes('.reason') && !leg.includes('.REASON'), '690 .reason como esta escrito', leg.trim());
    await page.screenshot({ path: join(SAIDA, `${tema}-excluir.png`) });
    await page.evaluate(() => document.querySelector('.sobre')?.remove());

    // 692 -- a aba Conteudo (a grade so de leitura).
    await page.evaluate(([d]) => { est.aba = 'conteudo'; return abrirTabela(d, 'clientes'); }, [db]);
    await page.waitForSelector('#grade .phx-th-titulo', { timeout: 10000 }).catch(() => {});
    const t2 = await page.$$eval('#grade .phx-th-titulo', ths => ths.map(t => t.innerText.trim()));
    nota(t2.some(t => t.startsWith('Limite de crédito')), '692 caption na aba Conteudo', JSON.stringify(t2));

    // 694 -- a Nova tabela.
    await page.evaluate(([d]) => telaNovaTabela(d), [db]);
    await page.waitForTimeout(300);
    const sub = await page.evaluate(() => document.querySelector('#subtitulo')?.innerText || '');
    nota(!/\b(sete|seven|sept|sette|sieben|siete|\d+)\b/i.test(sub), '694 subtitulo sem numero cravado', sub.trim());
    await page.screenshot({ path: join(SAIDA, `${tema}-nova-tabela.png`) });
    await ctx.close();
  }
} finally {
  await nav.close();
  await servidor.derrubar();
}
console.log(reprovas.length ? `\nREPROVADO: ${reprovas.length}` : '\nAPROVADO');
process.exit(reprovas.length ? 1 : 0);

/* Dissuasao de F12, «Inspecionar» e botao direito (pedido 772, ordem do dono).
 *
 * E dissuasao de curioso, NAO seguranca -- e o que este caso prova e so o que
 * a tela promete: LIGADA (`web.dissuadir_inspecao`, padrao true, e o servidor
 * da bateria nao escreve o campo), o menu de contexto fora de campo e os
 * atalhos das ferramentas do desenvolvedor tem o padrao do navegador
 * suprimido; DESLIGADA (o `/saude` dizendo false), nada muda. E nos DOIS:
 * colar com Ctrl+V entra no campo, a selecao de texto pelo mouse acontece, o
 * menu nativo sobre campo e sobre texto selecionado continua, e os atalhos
 * comuns (Ctrl+C, Ctrl+V, Ctrl+A, Tab) nao sao tocados.
 *
 * «Suprimido» se mede pelo `defaultPrevented` do evento REAL (mouse e teclado
 * do Playwright), lido por um ouvinte posto DEPOIS do da tela: a janela das
 * ferramentas nao abre em navegador sem cabeca, e o que a pagina pode fazer
 * -- e faz -- e cancelar o padrao. */
import { entrar, api, verdade, igual, bancoDoCaso, Falha } from '../apoio.mjs';

let passo = 'inicio';

export const caso = {
  nome: 'dissuasao-de-inspecao',
  async rodar(ctx) {
    try { await corpo(ctx); } catch (e) {
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]}`);
    }
  },
};

/** Liga o espiao: guarda, por tipo, se o padrao foi cancelado. */
async function espiar(page) {
  await page.evaluate(() => {
    window.__visto = [];
    for (const t of ['keydown', 'contextmenu']) {
      addEventListener(t, ev => window.__visto.push({ t, k: ev.key || '', cancelado: ev.defaultPrevented }));
    }
  });
}
const ultimo = (page, t) => page.evaluate(t => (window.__visto.filter(v => v.t === t).at(-1)) || null, t);
const limpar = page => page.evaluate(() => { window.__visto.length = 0; });

async function botaoDireito(page, sel) {
  await limpar(page);
  await page.click(sel, { button: 'right' });
  const v = await ultimo(page, 'contextmenu');
  if (!v) throw new Falha(`o botao direito em ${sel} nao gerou contextmenu`);
  await page.keyboard.press('Escape');
  return v.cancelado;
}
async function tecla(page, combo) {
  await limpar(page);
  await page.keyboard.press(combo);
  const v = await ultimo(page, 'keydown');
  return v ? v.cancelado : null;
}

async function umaVolta(ctx, ligada) {
  const nome = ligada ? 'ligada' : 'desligada';
  const browser = ctx.page.context().browser();
  const contexto = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    permissions: ['clipboard-read', 'clipboard-write'],
  });
  try {
    await contexto.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, ctx.tema);
    const page = await contexto.newPage();
    if (!ligada) {
      // O interruptor desligado como o servidor o diria: so o campo muda.
      await page.route(u => u.pathname === '/saude', async route => {
        const r = await route.fetch();
        const j = await r.json();
        j.dissuadir_inspecao = false;
        await route.fulfill({ response: r, json: j });
      });
    }

    // ------------------------------------------- na tela de entrada
    passo = `${nome}: o /saude`;
    await page.goto(ctx.url, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });
    const saude = await page.evaluate(() => fetch('/saude').then(r => r.json()));
    igual(saude.dissuadir_inspecao, ligada, 'o /saude devia dizer o interruptor');
    await espiar(page);

    passo = `${nome}: colar no campo de usuario`;
    await page.evaluate(() => navigator.clipboard.writeText('colado-772'));
    await page.click('#u');
    await page.fill('#u', '');
    verdade(!(await tecla(page, 'Control+V')), 'Ctrl+V teve o padrao cancelado');
    igual(await page.inputValue('#u'), 'colado-772', 'colar com Ctrl+V no campo');
    verdade(!(await botaoDireito(page, '#u')), 'o menu nativo do CAMPO (colar pelo mouse) foi suprimido');

    // ------------------------------------------- dentro da aplicacao
    passo = `${nome}: entrar`;
    await entrar(page, ctx.url);
    await espiar(page);

    passo = `${nome}: botao direito fora de campo`;
    const foraDeCampo = await botaoDireito(page, '#titulo');
    igual(foraDeCampo, ligada, 'botao direito fora de campo cancelado (so quando ligada)');

    passo = `${nome}: atalhos das ferramentas`;
    await page.click('#titulo');
    for (const combo of ['F12', 'Control+Shift+I', 'Control+Shift+J', 'Control+Shift+C', 'Control+U']) {
      igual(await tecla(page, combo), ligada, `${combo} cancelado (so quando ligada)`);
    }
    passo = `${nome}: atalhos comuns`;
    for (const combo of ['Control+C', 'Control+A', 'Tab', 'Shift+Tab']) {
      igual(await tecla(page, combo), false, `${combo} nao pode ser tocado`);
    }

    passo = `${nome}: selecao de texto pelo mouse`;
    await page.keyboard.press('Escape');
    await page.click('#titulo', { clickCount: 3 });
    const sel = await page.evaluate(() => String(getSelection()).trim());
    verdade(sel.length > 0, 'o triplo clique nao selecionou o titulo');
    verdade(!(await botaoDireito(page, '#titulo')), 'o menu nativo SOBRE texto selecionado (copiar) foi suprimido');

    passo = `${nome}: menu pelo teclado`;
    await page.click('#titulo', { clickCount: 3 });
    await limpar(page);
    await page.keyboard.press('Shift+F10');
    const pelaTecla = await ultimo(page, 'contextmenu');
    // Nem todo navegador sem cabeca abre o menu pelo Shift+F10; quando abre,
    // ele tem de passar.
    verdade(!pelaTecla || !pelaTecla.cancelado, 'o menu aberto pelo TECLADO (Shift+F10) foi suprimido');

    passo = `${nome}: o menu da arvore continua`;
    await api(page, 'criar_database', { database: bancoDoCaso(ctx, 'Dis') }).catch(() => {});
    await page.evaluate(() => montarArvore(false));
    const no = page.locator('#arvore .no[data-db]').first();
    await no.waitFor({ timeout: 10000 });
    await no.click({ button: 'right' });
    await page.waitForSelector('#popup .item', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.close();
  } finally {
    await contexto.close();
  }
}

async function corpo(ctx) {
  await umaVolta(ctx, true);
  await umaVolta(ctx, false);
}

/* Os monitores, o DPI e a janela destacada.
 *
 * O que se prova de verdade aqui, e o que se prova com dublê -- dito sem
 * maquiagem, porque um teste que passa por engano e pior que um que falta:
 *
 *   * A `Window Management API` NAO se exercita de verdade nesta bancada.
 *     Ela pede a permissao `window-management`, que o Playwright nao sabe
 *     conceder (o nome nao esta na lista dele) e que o Chromium sem cabeca
 *     nao concede sozinho. Entao `getScreenDetails` e DUBLADA: o caminho que
 *     se prova e o NOSSO -- achar a emenda entre monitores, alinhar as calhas
 *     com ela, e cair para o monitor principal quando o pinado sumiu.
 *     O que fica sem prova real e a resposta do navegador; o que ela alimenta,
 *     nao.
 *   * O DPI diferente SE prova de verdade: o Playwright cria contexto com
 *     `deviceScaleFactor`, e um segundo navegador de 2× carrega a pagina
 *     inteira. O que se confere ali e a decisao de guardar PIXEL CSS: a mesma
 *     regiao mede o mesmo numero em 1× e em 2×, e e por isso que a geometria
 *     guardada continua valendo.
 *   * A JANELA DESTACADA se prova de verdade, pelo `BroadcastChannel`, numa
 *     segunda aba da mesma origem -- e com a conferencia que mais importa:
 *     a ficha de sessao NAO aparece no `localStorage` de nenhuma das duas. */
import { entrar, verdade, igual, assentar, capturar } from '../apoio.mjs';

/* Dois monitores encostados, o segundo com o dobro da densidade -- o caso do
 * super-ultrawide por daisy chain, que e "dois monitores num". */
const DUBLE = `(function () {
  const telas = [
    { label: "Esquerdo", left: 0, top: 0, width: 2560, height: 1440,
      availLeft: 0, availTop: 0, availWidth: 2560, availHeight: 1400,
      devicePixelRatio: 1, isPrimary: true },
    { label: "Direito", left: 2560, top: 0, width: 2560, height: 1440,
      availLeft: 2560, availTop: 0, availWidth: 2560, availHeight: 1400,
      devicePixelRatio: 2, isPrimary: false },
  ];
  Object.defineProperty(window, "getScreenDetails", {
    configurable: true, writable: true,
    value: () => Promise.resolve({ screens: telas, currentScreen: telas[0] }),
  });
})()`;

export const caso = {
  nome: 'monitores',
  temaUnico: 'escuro',

  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // O que ESTE navegador tem, de verdade, sem dublê nenhum.
    const real = await page.evaluate(async () => ({
      temApi: PhxTelas.temApiDeTelas(),
      respondeu: !!(await PhxTelas.monitores()),
      dpr: window.devicePixelRatio,
    }));
    ctx.notas.push(`sem dublê: getScreenDetails ${real.temApi ? 'existe' : 'nao existe'}`
      + ` · respondeu: ${real.respondeu} · dpr ${real.dpr}`);
    // Sem a API, ou com ela recusada, o modo tem de continuar inteiro. Esta e
    // a prova do degrau: dividir em duas nao pode depender de monitor nenhum.
    await page.evaluate(() => PhxTelas.dividir(2));
    await assentar(page, 700);
    igual(await page.evaluate(() => document.querySelectorAll('#regioes .regiao').length),
      2, 'dividir em duas nao pode depender da Window Management API');

    // A largura em pixel CSS, medida ANTES do dublê: o dublê troca o
    // `getBoundingClientRect` do `#regioes` para fingir a janela esticada, e
    // medir depois dele mediria a mentira.
    const emUm = await page.evaluate(() => ({
      dpr: window.devicePixelRatio,
      largura: Math.round(document.querySelector('#regioes').getBoundingClientRect().width),
    }));

    // ------------------------------------------- a emenda entre dois monitores
    await page.evaluate(DUBLE);
    // A janela do teste tem 1600px e mora no monitor esquerdo: nao ha emenda
    // DENTRO dela, e a resposta certa e "nada a alinhar".
    const semEmenda = await page.evaluate(() => PhxTelas.emendas());
    igual(semEmenda.length, 0,
      'janela inteira dentro de um monitor nao tem emenda a alinhar');

    // Agora finge que a janela esta esticada por cima das duas telas: e a
    // foto do dono, o navegador aberto na largura dos dois monitores.
    const cortes = await page.evaluate(() => {
      Object.defineProperty(window, 'screenX', { configurable: true, value: 0 });
      Object.defineProperty(window, 'outerWidth', { configurable: true, value: 5120 });
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: 5120 });
      // A area util tem de ser larga o bastante para a emenda cair dentro
      // dela; o `#regioes` real desta bancada tem 1600px.
      const cont = document.querySelector('#regioes');
      cont.getBoundingClientRect = () => ({ left: 0, top: 0, width: 5120, height: 800,
        right: 5120, bottom: 800 });
      return PhxTelas.emendas();
    });
    igual(cortes.length, 1, 'a emenda entre os dois monitores tem de aparecer');
    verdade(Math.abs(cortes[0] - 2560) < 4,
      `a emenda saiu no lugar errado: ${cortes[0]} (esperava 2560)`);
    ctx.notas.push(`emenda fisica achada a ${Math.round(cortes[0])}px da borda`);

    await page.evaluate(() => PhxTelas.alinharComOsMonitores());
    await assentar(page, 700);
    const pesos = await page.evaluate(() =>
      [...document.querySelectorAll('#regioes .regiao')].map(r => +r.style.flexGrow));
    igual(pesos.length, 2, 'alinhar com dois monitores da duas regioes');
    verdade(Math.abs(pesos[0] - pesos[1]) < 0.05,
      `dois monitores iguais tinham de dar dois pesos iguais: ${JSON.stringify(pesos)}`);

    // ------------------------------------------- o monitor pinado que sumiu
    // A janela foi pinada num monitor que hoje nao esta aqui. Ela NAO pode
    // abrir fora da area visivel: janela perdida existe, consome sessao e
    // ninguem a ve para fechar.
    const caiu = await page.evaluate(async () => {
      const recados = [];
      const antes = window.avisar;
      window.avisar = (t, mal) => recados.push({ t, mal });
      const f = await PhxTelas._feicoesDaJanela({
        x: 4000, y: 300, w: 900, h: 600,
        monitor: 'Monitor que foi embora', mx: 3840, my: 0, dpr: 1,
      });
      window.avisar = antes;
      return { f, recados };
    });
    verdade(/left=\d+/.test(caiu.f), `nao vieram coordenadas: ${caiu.f}`);
    const left = +/left=(\d+)/.exec(caiu.f)[1];
    const top = +/top=(\d+)/.exec(caiu.f)[1];
    verdade(left >= 0 && left + 900 <= 2560,
      `a janela abriria fora do monitor principal: left=${left}`);
    verdade(top >= 0 && top + 600 <= 1400, `a janela abriria fora: top=${top}`);
    verdade(caiu.recados.some(r => r.mal && /monitor/i.test(r.t)),
      `a queda para o principal tem de ser DITA, e nao silenciosa: ${JSON.stringify(caiu.recados)}`);
    ctx.notas.push(`monitor sumido: janela presa em ${left},${top} e o aviso saiu`);

    // `prender` tambem vale para a janela solta dentro da pagina.
    const preso = await page.evaluate(() =>
      PhxTelas._prender({ x: 4800, y: 900, w: 900, h: 600 },
        { width: 1400, height: 800 }));
    verdade(preso.x + preso.w <= 1400 && preso.y + preso.h <= 800,
      `a janela solta ficou fora da area visivel: ${JSON.stringify(preso)}`);

    // ---------------------------------------------------- a janela destacada
    // Uma segunda aba da MESMA origem, com a rota da tela destacada. Ela nao
    // ve formulario: pede a ficha a esta pagina pelo BroadcastChannel.
    const destacada = await page.context().newPage();
    await destacada.goto(`${ctx.url}?tela=query&destacada=1`,
      { waitUntil: 'domcontentloaded' });
    await destacada.waitForSelector('#app.ativo', { timeout: 20000 });
    const filha = await destacada.evaluate(() => ({
      destacada: document.querySelector('#app').dataset.destacada,
      menuVisivel: document.querySelector('.menubar').offsetParent !== null,
      arvoreVisivel: document.querySelector('.lateral').offsetParent !== null,
      titulo: (document.querySelector('#titulo') || {}).textContent,
      temSessao: !!est.sessao,
      // A ficha de sessao NAO pode estar no disco do navegador: ele e lido
      // por qualquer outra aba da mesma origem e sobrevive ao fechamento.
      disco: Object.keys(localStorage).map(k => `${k}=${localStorage.getItem(k)}`).join('|'),
    }));
    igual(filha.destacada, '1', 'a janela destacada tem de subir em modo destacado');
    igual(filha.menuVisivel, false, 'a janela destacada nao mostra a barra de menu');
    igual(filha.arvoreVisivel, false, 'a janela destacada nao mostra a arvore');
    verdade(filha.temSessao, 'a janela destacada nao recebeu a sessao da mae');
    verdade(/consulta/i.test(filha.titulo || ''),
      `a janela destacada abriu na tela errada: ${JSON.stringify(filha.titulo)}`);
    const sessao = await page.evaluate(() => est.sessao);
    verdade(sessao && !filha.disco.includes(sessao),
      `a ficha de sessao vazou para o localStorage: ${filha.disco}`);
    ctx.notas.push('janela destacada: sessao veio pelo canal, e nao pelo disco');
    await capturar({ ...ctx, page: destacada }, ctx.nomeCaptura('janela-destacada'));
    await destacada.close();

    // ------------------------------------------------------- DPI de 2×, real
    // Contexto proprio, com a densidade dobrada. O que se confere e a decisao
    // de guardar PIXEL CSS: a regiao mede o mesmo numero nos dois, e por isso
    // a geometria guardada num monitor continua valendo no outro.
    const ctx2 = await page.context().browser().newContext({
      viewport: { width: 1600, height: 950 }, deviceScaleFactor: 2,
    });
    await ctx2.addInitScript(() => {
      try { localStorage.setItem('phxsql-tema', 'escuro'); } catch { /* privado */ }
    });
    await ctx2.route(
      u => /fonts\.(googleapis|gstatic)\.com/.test(typeof u === 'string' ? u : u.href),
      r => r.abort());
    const p2 = await ctx2.newPage();
    try {
      await entrar(p2, ctx.url);
      await p2.evaluate(() => PhxTelas.dividir(2));
      await assentar(p2, 700);
      const emDois = await p2.evaluate(() => ({
        dpr: window.devicePixelRatio,
        largura: Math.round(document.querySelector('#regioes').getBoundingClientRect().width),
        regioes: document.querySelectorAll('#regioes .regiao').length,
        maxRegioes: PhxTelas.maxRegioes(),
      }));
      igual(emDois.dpr, 2, 'o contexto de 2x nao subiu com densidade 2');
      igual(emDois.regioes, 2, 'as regioes tem de funcionar igual em 2x');
      igual(emDois.largura, emUm.largura,
        'a mesma largura em pixel CSS nos dois DPI — e por isso que a geometria guardada vale');
      ctx.notas.push(`1x: ${emUm.largura}px CSS · 2x: ${emDois.largura}px CSS`
        + ` · cabem ${emDois.maxRegioes} regioes nos dois`);
      await capturar({ ...ctx, page: p2 }, ctx.nomeCaptura('dpi-2x'));
    } finally {
      await ctx2.close();
    }
  },
};

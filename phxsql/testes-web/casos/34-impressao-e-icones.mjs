/* A impressao, o menu que abria invisivel e o conjunto de icones da casa.
 *
 * Tres coisas que so se provam no navegador:
 *
 * 1. IMPRIMIR (reabre o pedido 161). O botao so existe onde ha grade; o
 *    papel sai sem o cromo, preto no branco mesmo no tema escuro, com o
 *    cabecalho da tabela repetido por pagina -- e o tema de quem imprimiu
 *    volta depois.
 * 2. O MENU DO ALTO. Desde que a barra de menu ganhou `overflow-x:auto`, a
 *    lista de cada menu era RECORTADA na altura da barra: o titulo acendia e
 *    nada aparecia. Achado exercitando, nao lendo: `elementFromPoint` no meio
 *    da lista devolvia o botao da barra de ferramentas. A prova e esta mesma
 *    pergunta -- quem esta no topo, no meio da lista?
 * 3. OS ICONES. O `<use>` que aponta para um desenho que nao existe nao da
 *    erro: desenha nada. Aqui se mede que o icone da barra e do menu TEM
 *    tamanho, e que nenhum simbolo Unicode voltou ao cromo. */
import { entrar, cenario, capturar, verdade, igual, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'impressao-e-icones',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Imprime');
    const { tab } = await cenario(page, db);

    // ---- 3. os icones do cromo desenham ---------------------------------
    const icones = await page.evaluate(() => {
      // Fora os que estao escondidos de proposito (o imprimir, sem grade).
      const r = [...document.querySelectorAll('#ferramentas svg.ic, .barra svg.ic')]
        .filter(s => !s.closest('[hidden]')).map(s => s.getBoundingClientRect());
      const cromo = document.querySelector('#ferramentas').textContent
        + document.querySelector('.barra').textContent;
      return {
        n: r.length,
        vazios: r.filter(b => b.width < 8 || b.height < 8).length,
        simbolos: [...cromo].filter(c => /[←-⇿⌀-⏿■-➿⬀-⯿]/u.test(c)
          || c.codePointAt(0) >= 0x1f000),
      };
    });
    verdade(icones.n >= 20, `a barra de ferramentas tem ${icones.n} icone(s) do sprite -- esperava >= 20`);
    igual(icones.vazios, 0, 'icone do sprite sem tamanho (desenho sem symbol, ou CSS que o esmaga)');
    igual(icones.simbolos.join(''), '', 'simbolo Unicode voltou ao cromo');

    // ---- 2. o menu do alto aparece de verdade ---------------------------
    await page.click('.menubar .menu[data-m="2"] .titulo');
    const noTopo = await page.evaluate(() => {
      const l = document.querySelector('.menubar .menu[data-m="2"] .lista');
      const r = l.getBoundingClientRect();
      const e = document.elementFromPoint(r.left + r.width / 2, r.top + Math.min(60, r.height / 2));
      return { dentro: !!(e && l.contains(e)), quem: e ? `${e.tagName}.${e.className}` : null,
               icone: l.querySelector('svg.ic')?.getBoundingClientRect().width || 0 };
    });
    verdade(noTopo.dentro, `a lista do menu abriu por baixo de outra coisa: no meio dela esta ${noTopo.quem}`);
    verdade(noTopo.icone >= 8, `o icone do item de menu mede ${noTopo.icone}px`);
    await capturar(ctx, ctx.nomeCaptura('menu-com-icones'));
    await page.keyboard.press('Escape');

    // ---- 1. imprimir: o botao so onde ha grade --------------------------
    await page.evaluate(([d, t]) => gerirTabela(d, t), [db, tab]);
    await page.waitForSelector('#painel .ops');
    await page.waitForTimeout(100);
    verdade(await page.locator('#btImprimir').isHidden(),
      'o botao de imprimir aparece numa tela sem grade');

    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await page.waitForSelector('#painel .phx-grid tbody tr');
    await page.waitForTimeout(100);
    verdade(await page.locator('#btImprimir').isVisible(),
      'a grade abriu e o botao de imprimir nao apareceu');

    // O dialogo do sistema nao abre no navegador sem tela; conta-se a chamada.
    await page.evaluate(() => { window.__impressoes = 0; window.print = () => { window.__impressoes++; }; });
    await page.click('#btImprimir');
    igual(await page.evaluate(() => window.__impressoes), 1, 'o botao nao chamou window.print()');

    // O papel: a pessoa estava no tema ESCURO? O papel sai claro assim mesmo.
    const temaAntes = await page.evaluate(() => document.documentElement.dataset.tema);
    await page.emulateMedia({ media: 'print' });
    await page.evaluate(() => window.dispatchEvent(new Event('beforeprint')));
    const papel = await page.evaluate(() => {
      const cs = s => getComputedStyle(document.querySelector(s));
      return {
        barra: cs('.barra').display, ferramentas: cs('#ferramentas').display,
        lateral: cs('.lateral').display, cabeca: cs('#cabecaImpressa').display,
        cabecaTexto: document.querySelector('#cabecaImpressa').textContent,
        fundo: cs('body').backgroundColor, texto: cs('#painel .phx-grid td').color,
        thead: cs('#painel .phx-grid thead').display,
        envoltorio: cs('#painel .phx-envoltorio').maxHeight,
        botoes: [...document.querySelectorAll('#painel button')].filter(b => b.offsetParent).length,
      };
    });
    igual(papel.barra, 'none', 'a barra do alto foi para o papel');
    igual(papel.ferramentas, 'none', 'a barra de ferramentas foi para o papel');
    igual(papel.lateral, 'none', 'a arvore foi para o papel');
    igual(papel.cabeca, 'flex', 'o cabecalho do papel (logo, banco, hora) nao apareceu');
    verdade(papel.cabecaTexto.includes(db), `o cabecalho do papel nao diz o banco: ${papel.cabecaTexto}`);
    igual(papel.fundo, 'rgb(255, 255, 255)', 'o fundo do papel nao e branco');
    igual(papel.texto, 'rgb(0, 0, 0)', 'o dado da grade nao sai preto no papel');
    igual(papel.thead, 'table-header-group', 'o cabecalho da tabela nao se repete por pagina');
    igual(papel.envoltorio, 'none', 'o envoltorio de rolagem da grade corta o papel em 560px');
    igual(papel.botoes, 0, 'botao da tela foi para o papel');
    await capturar(ctx, ctx.nomeCaptura('papel'), { inteira: true });

    await page.evaluate(() => window.dispatchEvent(new Event('afterprint')));
    await page.emulateMedia({ media: 'screen' });
    igual(await page.evaluate(() => document.documentElement.dataset.tema), temaAntes,
      'o tema de quem imprimiu nao voltou depois da impressao');
  },
};

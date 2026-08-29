/* O laco que percorre TODAS as telas. Vale mais que dez asercoes bonitas.
 *
 * Clica cada item dos nove menus e cada botao da barra de ferramentas, e
 * reprova se qualquer um soltar erro. «Erro» aqui sao TRES canais, e nao um:
 *
 *  1. `pageerror` -- excecao que ninguem pegou (o runner cuida deste);
 *  2. `#aviso.mal` -- o `avisar(..., true)`, para onde o `ligarMenu` manda
 *     TODA excecao de item de menu (`.catch(e => avisar(...))`). Sem olhar
 *     este canal, uma tela que estoura no meio passa verde: a excecao foi
 *     capturada e nunca vira `pageerror`;
 *  3. `#painel .aviso.mal` -- o mesmo, dentro do painel, que e onde o
 *     `desenharAba` deposita o erro de uma aba.
 *
 * Os dialogos nativos (`confirm`, `prompt`) sao DESCARTADOS pelo Playwright
 * quando ninguem os escuta -- e e por isso que o passeio pode clicar em
 * «Excluir tabela» sem excluir nada: o `prompt` devolve `null` e a funcao
 * volta na primeira linha. */
import { entrar, capturar, cenario, abrirPelaArvore, bancoDoCaso, verdade } from '../apoio.mjs';

/* Fora do passeio, com o motivo escrito. Nada entra aqui por ser chato. */
const FORA = new Map([
  ['Sair', 'derruba a sessao e o resto do passeio nao teria onde acontecer'],
]);

export const caso = {
  nome: 'passeio',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Passeio');
    // DUAS tabelas: `juntar` e `unir` recusam um banco com uma so, e a recusa
    // e legitima. Montar o cenario certo e melhor que ensinar a bateria a
    // ignorar uma recusa — o dia em que a recusa virar defeito, ela conta.
    await cenario(page, db, 'clientes');
    await cenario(page, db, 'pedidos');
    await abrirPelaArvore(page, db, 'clientes');

    const visitadas = [];
    const falhas = [];

    /* Limpa os dois canais de erro ANTES do clique: um aviso deixado pela
       tela anterior seria contado contra a proxima -- e o painel de uma tela
       que nao repintou ainda carrega o `.aviso.mal` da anterior. */
    const limpar = () => page.evaluate(() => {
      const a = document.getElementById('aviso');
      if (a) { a.hidden = true; a.className = 'recado'; a.textContent = ''; }
      for (const x of document.querySelectorAll('#painel .aviso.mal')) x.remove();
    });

    /* Tres telas do passeio zeram `est.atual` (o Painel, a gestao de tabelas
       e a consulta). Depois delas, «Lixeira» e «Importar» recusam com «escolha
       uma tabela primeiro» -- recusa CERTA, e nao defeito. A bateria refaz o
       que a pessoa faria: escolhe a tabela na arvore de novo. */
    const garantirTabela = async () => {
      if (await page.evaluate(() => !est.atual)) {
        await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);
        await page.waitForTimeout(200);
      }
    };

    const conferir = async rotulo => {
      await page.waitForTimeout(220);
      const mal = await page.evaluate(() => {
        const fora = document.querySelector('#aviso.mal:not([hidden])');
        const dentro = document.querySelector('#painel .aviso.mal');
        return { fora: fora ? fora.textContent.trim() : '',
                 dentro: dentro ? dentro.textContent.trim() : '' };
      });
      if (mal.fora) falhas.push(`${rotulo}: aviso de erro «${mal.fora}»`);
      if (mal.dentro) falhas.push(`${rotulo}: o painel abriu com erro «${mal.dentro}»`);
      visitadas.push(rotulo);
    };

    // ------------------------------------------------------------- menus
    const menus = await page.evaluate(() =>
      [...document.querySelectorAll('.menubar .menu')].map(m => ({
        m: m.dataset.m,
        titulo: m.querySelector('.titulo').textContent.trim(),
        itens: [...m.querySelectorAll('.item')].map(b => ({
          i: b.dataset.i, rot: b.querySelector('.rot').textContent.trim(),
        })),
      })));

    for (const menu of menus) {
      for (const item of menu.itens) {
        const rotulo = `${menu.titulo} › ${item.rot}`;
        if (FORA.has(item.rot)) continue;
        await garantirTabela();
        await limpar();
        await page.click(`.menubar .titulo[data-m="${menu.m}"]`);
        const bt = page.locator(`.menubar .item[data-m="${menu.m}"][data-i="${item.i}"]`);
        if (await bt.isDisabled()) {
          // Item cinza e estado legitimo (nao ha tabela aberta, por exemplo).
          await page.keyboard.press('Escape');
          continue;
        }
        await bt.click();
        await conferir(rotulo);
      }
    }

    // ----------------------------------------------------- ferramentas
    const quantas = await page.locator('#ferramentas .fer').count();
    for (let i = 0; i < quantas; i++) {
      const bt = page.locator('#ferramentas .fer').nth(i);
      const rotulo = `barra › ${(await bt.textContent()).trim()}`;
      await garantirTabela();
      await limpar();
      await bt.click();
      await conferir(rotulo);
    }

    // ------------------------------------------------------- as cinco abas
    await abrirPelaArvore(page, db, 'clientes');
    for (const aba of ['estrutura', 'conteudo', 'indices', 'diario', 'integridade']) {
      await limpar();
      await page.click(`.aba[data-aba="${aba}"]`);
      await conferir(`aba › ${aba}`);
    }
    await capturar(ctx, ctx.nomeCaptura('fim-do-passeio'));

    ctx.notas.push(`${visitadas.length} telas percorridas`);
    verdade(visitadas.length >= 90,
      `so percorri ${visitadas.length} telas — o passeio encolheu, confira o denylist`);
    verdade(falhas.length === 0, `telas com erro:\n      ${falhas.join('\n      ')}`);
  },
};

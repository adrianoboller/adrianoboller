/* A arvore remontando quando um banco novo aparece.
 *
 * Foi o defeito que o assistente do DbLink achou no navegador: o banco era
 * criado e a arvore continuava mostrando a lista de antes -- so voltava ao
 * lugar recarregando a pagina, que e justamente o que uma interface de
 * console nao pode pedir.
 *
 * A parte que ler o codigo nao pega: `montarArvore()` troca o `innerHTML`
 * inteiro de `#arvore`, e com ele TODO ouvinte de clique pendurado la dentro.
 * Se algum deles nao for religado depois da troca, a arvore fica desenhada e
 * MORTA -- e isso e indistinguivel de «funciona» numa captura de tela. Por
 * isso o caso clica DEPOIS de remontar. */
import { entrar, capturar, verdade, igual, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'arvore',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    const antes = await page.locator('#arvore .no.db').count();
    const novo = bancoDoCaso(ctx, 'Arvore');

    // Pelo [+] da arvore, que e o caminho de quem usa -- e ele passa pelo
    // `prompt()`, entao a bateria responde ao dialogo em vez de o descartar.
    page.once('dialog', d => d.accept(novo));
    await page.click('#btNovoDb');
    await page.waitForFunction(
      n => [...document.querySelectorAll('#arvore .no.db')].some(x => x.dataset.db === n),
      novo, { timeout: 10000 });

    const depois = await page.locator('#arvore .no.db').count();
    igual(depois, antes + 1, 'a arvore nao ganhou o banco novo');
    await capturar(ctx, ctx.nomeCaptura('arvore-com-banco-novo'));

    // A arvore continua VIVA depois da remontagem: clicar no banco novo abre
    // o View Database dele. Uma arvore desenhada e sem ouvinte passaria na
    // asercao de cima e falharia aqui.
    await page.click(`#arvore .no.db[data-db="${novo}"]`);
    await page.waitForTimeout(400);
    const titulo = await page.textContent('#titulo');
    verdade(titulo.includes(novo),
      `clicar no banco novo nao abriu a tela dele (titulo «${titulo}»)`);

    // E o [+] tambem sobreviveu: ele mora FORA do `#arvore` de proposito,
    // porque o `innerHTML` levaria o ouvinte dele junto.
    verdade(await page.locator('#btNovoDb').count() === 1,
      'o [+] sumiu depois da remontagem da arvore');
    page.once('dialog', d => d.dismiss());
    await page.click('#btNovoDb');
    await page.waitForTimeout(200);

    // Uma tabela criada tambem tem de aparecer sem recarregar.
    await page.evaluate(([db]) => api('criar_tabela', {
      database: db, tabela: 'novinha',
      colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }), [novo]);
    await page.evaluate(() => montarArvore(false));
    await page.waitForSelector(`#arvore .no.tab[data-db="${novo}"][data-tab="novinha"]`,
      { timeout: 10000 });
    await page.click(`#arvore .no.tab[data-db="${novo}"][data-tab="novinha"]`);
    await page.waitForTimeout(400);
    igual(await page.textContent('#titulo'), 'novinha',
      'clicar na tabela nova nao abriu a tabela');
  },
};

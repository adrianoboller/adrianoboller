/* As quatro pecas que o dono mandou mudar de lugar, exercitadas no navegador.
 *
 * Botao mudado de lugar e o caso que LER O CODIGO nao pega: a peca continua
 * existindo, o `onclick` continua certo, e o unico jeito de saber se ela esta
 * onde deveria -- e se ainda responde ali -- e abrir e clicar.
 *
 * As quatro:
 *   1. Ajuda saiu da barra de ferramentas e foi para o topo, ao lado da
 *      lua/sol.
 *   2. Duplicar saiu da barra e so existe pelo botao direito, num popup que
 *      NAO EXISTIA -- a interface inteira nao tinha um `contextmenu` sequer.
 *   3. Juncao saiu da barra e foi para dentro da tela de Query.
 *   4. Start/Stop tomou o lugar que era da Juncao.
 *
 * O caso confere os DOIS lados de cada uma: a peca chegou no lugar novo E
 * saiu do velho. So o primeiro passaria com o botao duplicado nos dois
 * lugares, que e exatamente o defeito que uma mudanca de lugar produz quando
 * alguem copia em vez de mover. */
import { entrar, capturar, verdade, igual } from '../apoio.mjs';

export const caso = {
  nome: 'barra-e-popup',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // ---- 1. Ajuda no topo, ao lado do tema -----------------------------
    verdade(await page.locator('#btAjuda').isVisible(), 'a Ajuda nao esta no topo');
    // Vizinhanca de VERDADE, e nao "existe na pagina": o pedido foi «ao lado
    // da lua/sol», e um botao no topo mas do outro lado da barra cumpriria o
    // teste frouxo e nao o pedido.
    const vizinho = await page.evaluate(() =>
      document.querySelector('#btAjuda').nextElementSibling?.id);
    igual(vizinho, 'btTema', 'a Ajuda nao ficou ao lado do botao de tema');

    // ---- 4. Start/Stop na barra; 2 e 3 fora dela -----------------------
    const naBarra = await page.evaluate(() =>
      [...document.querySelectorAll('#ferramentas .fer')].map(b => b.title));
    verdade(naBarra.some(t => /Start\/Stop/i.test(t)), `Start/Stop sumiu da barra: ${naBarra}`);
    verdade(!naBarra.some(t => /Junç|Juncao/i.test(t)), `a Juncao ficou na barra: ${naBarra}`);
    verdade(!naBarra.some(t => /Duplicar/i.test(t)), `o Duplicar ficou na barra: ${naBarra}`);
    verdade(!naBarra.some(t => /^Ajuda/i.test(t)), `a Ajuda ficou na barra: ${naBarra}`);

    // ---- 3. Juncao DENTRO da tela de Query -----------------------------
    // Pelo botao da barra, que e o caminho de quem usa.
    const iQuery = naBarra.findIndex(t => /Query/i.test(t));
    verdade(iQuery >= 0, `nao achei o botao Query na barra: ${naBarra}`);
    await page.locator('#ferramentas .fer').nth(iQuery).click();
    await page.waitForSelector('#btJuncaoQuery', { timeout: 10000 });
    verdade(await page.locator('#btJuncaoQuery').isVisible(),
            'a Juncao nao apareceu na tela de Query');
    // E ela ABRE a juncao -- botao que existe e nao faz nada e pior que botao
    // que falta, porque parece pronto.
    await page.click('#btJuncaoQuery');
    await page.waitForFunction(
      () => /jun[cç]/i.test(document.querySelector('#painel')?.textContent || ''),
      null, { timeout: 10000 });

    // ---- 2. Duplicar pelo botao direito --------------------------------
    const no = page.locator('#arvore .no.tab').first();
    await no.waitFor({ timeout: 10000 });
    await no.click({ button: 'right' });
    await page.waitForSelector('#popup:not([hidden])', { timeout: 10000 });
    const itens = await page.evaluate(() =>
      [...document.querySelectorAll('#popup .item')].map(b => b.textContent.trim()));
    verdade(itens.some(t => /Duplicar/i.test(t)), `o popup da tabela nao oferece Duplicar: ${itens}`);
    // O ALVO tem de aparecer: menu que nao diz sobre o que age faz a pessoa
    // clicar torcendo. E o nome vem do DADO, entao nao pode estar maiusculado
    // por CSS -- e a mesma licao do «Blumenau» virando «BLUMENAU».
    const alvo = await page.locator('#popup .alvo').textContent();
    const daArvore = (await no.textContent()).trim().replace(/^[^\w]+/, '');
    verdade(alvo.includes(daArvore),
            `o popup nao diz o alvo certo: "${alvo}" nao contem "${daArvore}"`);

    // Fecha por Esc -- e um dos tres jeitos que a pessoa tenta.
    await page.keyboard.press('Escape');
    // `state:'hidden'`, e nao o seletor `[hidden]` sozinho: o padrao do
    // `waitForSelector` e esperar ficar VISIVEL, e um elemento escondido nunca
    // fica -- a primeira versao deste caso esperou 5 s por isso.
    await page.waitForSelector('#popup', { state: 'hidden', timeout: 5000 });

    // E o database tambem tem popup, com o Duplicar desligado e explicado.
    const db = page.locator('#arvore .no.db').first();
    await db.click({ button: 'right' });
    await page.waitForSelector('#popup:not([hidden])', { timeout: 10000 });
    const desligado = await page.evaluate(() =>
      [...document.querySelectorAll('#popup .item')]
        .filter(b => /Duplicar/i.test(b.textContent))
        .map(b => ({ off: b.disabled, motivo: b.title })));
    verdade(desligado.length === 1, `o popup do database nao oferece Duplicar: ${JSON.stringify(desligado)}`);
    verdade(desligado[0].off, 'o Duplicar do database aceita clique e nao faz nada');
    verdade((desligado[0].motivo || '').length > 20,
            'o Duplicar desligado nao explica por que esta desligado');
    await page.keyboard.press('Escape');

    await capturar(ctx, 'barra-e-popup');
  },
};

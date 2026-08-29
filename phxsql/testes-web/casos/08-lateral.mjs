/* O painel lateral: retratil, pinavel, redimensionavel — e com volta.
 *
 * O que este caso protege e a regra que o proprio HTML declara: «painel que
 * se fecha sem deixar por onde voltar e armadilha». O botao mora na barra,
 * que nunca some. Um dia em que alguem mover o botao para dentro do painel,
 * o caso falha ao tentar reabrir.
 *
 * E a segunda parte, que so o navegador prova: DESPINADO, o painel se fecha
 * sozinho depois de escolher uma tabela; PINADO, ele fica. Ler o codigo diz
 * que `fecharSeSolta()` existe; so clicando se sabe que ela e chamada por
 * todo caminho de escolha. */
import { entrar, cenario, capturar, verdade, igual, bancoDoCaso } from '../apoio.mjs';

const estado = page => page.evaluate(() => ({
  aberta: document.getElementById('app').dataset.lateralAberta,
  solta: document.getElementById('app').dataset.lateralSolta,
  pino: document.getElementById('btPinar').getAttribute('aria-pressed'),
  expandido: document.getElementById('btLateral').getAttribute('aria-expanded'),
  largura: getComputedStyle(document.documentElement).getPropertyValue('--arvore').trim(),
  guardado: localStorage.getItem('phxsql-lateral'),
}));

export const caso = {
  nome: 'lateral',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Lat');
    const { tab } = await cenario(page, db);
    // O banco nasceu pelo protocolo; a arvore ainda mostra a lista da
    // entrada. Remontar aqui e o que o proprio `criar_database` da tela faz.
    await page.evaluate(() => montarArvore(false));
    await page.waitForSelector(`#arvore .no.tab[data-db="${db}"][data-tab="${tab}"]`);

    // Nasce aberta e pinada.
    let e = await estado(page);
    igual(e.aberta, '1', 'a lateral nao nasceu aberta');
    igual(e.pino, 'true', 'a lateral nao nasceu pinada');
    await capturar(ctx, ctx.nomeCaptura('lateral-aberta'));

    // Recolher e voltar PELO MESMO botao, que mora na barra.
    await page.click('#btLateral');
    e = await estado(page);
    igual(e.aberta, '0', 'o botao nao recolheu a lateral');
    igual(e.expandido, 'false', 'o aria-expanded nao acompanhou o recolher');
    verdade(await page.locator('#btLateral').isVisible(),
      'o botao de reabrir sumiu junto com o painel — nao ha volta');
    await capturar(ctx, ctx.nomeCaptura('lateral-recolhida'));
    await page.click('#btLateral');
    igual((await estado(page)).aberta, '1', 'o botao nao reabriu a lateral');

    // Ctrl+\ faz o mesmo, e e o atalho que o titulo do botao promete.
    await page.keyboard.press('Control+\\');
    igual((await estado(page)).aberta, '0', 'Ctrl+\\ nao recolheu');
    await page.keyboard.press('Control+\\');
    igual((await estado(page)).aberta, '1', 'Ctrl+\\ nao reabriu');

    // PINADA: escolher uma tabela nao fecha o painel.
    await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="${tab}"]`);
    await page.waitForTimeout(300);
    igual((await estado(page)).aberta, '1', 'pinada, a lateral fechou ao escolher uma tabela');

    // DESPINADA: ela flutua e some depois da escolha — que e o que pinar quer
    // dizer, e o unico jeito de provar e escolhendo.
    await page.click('#btPinar');
    e = await estado(page);
    igual(e.pino, 'false', 'o pino nao desligou');
    igual(e.solta, '1', 'despinada, a lateral continua ocupando coluna');
    igual(e.aberta, '1', 'despinar com o painel fechado nao abriria nada — deve abrir junto');
    await capturar(ctx, ctx.nomeCaptura('lateral-solta'));
    await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="${tab}"]`);
    await page.waitForTimeout(300);
    igual((await estado(page)).aberta, '0',
      'despinada, a lateral NAO se fechou depois da escolha');

    // O estado fica no navegador, e nao no servidor.
    await page.click('#btLateral');
    await page.click('#btPinar');
    const guardado = JSON.parse((await estado(page)).guardado || '{}');
    igual(guardado.pinada, true, 'o pino nao foi guardado no navegador');

    // A pega de largura anda pelas setas — quem nao usa mouse tambem precisa
    // escolher a largura.
    const antes = parseInt((await estado(page)).largura, 10);
    await page.focus('#pegaArvore');
    for (let i = 0; i < 5; i++) await page.keyboard.press('ArrowRight');
    const depois = parseInt((await estado(page)).largura, 10);
    verdade(depois > antes, `a seta nao alargou a lateral (${antes} → ${depois})`);
  },
};

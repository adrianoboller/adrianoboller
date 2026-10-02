/* Os botoes da barra e da legenda da TELEMETRIA:
 *
 *   #tlmPausar  #tlmAgora  #tlmLigar  #tlmLegenda
 *
 * O caso `telemetria` prova a GRADE do gestor de threads; nenhum botao da
 * barra do painel vivo tinha clique. Cada um e conferido pelo EFEITO e nunca
 * pelo estado do botao:
 *
 *   Pausar    -> o relogio PARA de pedir (zero pedidos `telemetria` numa janela
 *                de duas voltas), a tela se declara pausada, e o Retomar volta a pedir
 *                (a tempo). Congelado por vontade tem de ser distinguivel de
 *                congelado porque o servidor caiu -- ver a pastilha.
 *   Agora     -> UM pedido sai imediatamente, sem esperar a volta do relogio.
 *   Ligar     -> o SERVIDOR muda de estado (op `telemetria`.ligada), e o botao
 *                muda de rotulo e de cor. Desligar e religar: o caso devolve a
 *                coleta como a achou, senao o proximo caso mediria um servidor
 *                que ninguem ligou.
 *   Legenda   -> a explicacao some e volta; o aria-expanded acompanha.
 *
 * O texto dos botoes sai da FABRICA DE IDIOMAS: o caso pergunta a ela
 * (`txt`) o que esperar, em vez de cravar «Retomar» -- no dia em que a tela
 * abrir noutro idioma, a espera cravada estouraria parecendo defeito.
 */
import { entrar, api, verdade, igual, capturar } from '../apoio.mjs';

const ESPERA = 45000;
const ehTelemetria = req => req.method() === 'POST' && req.url().endsWith('/api')
  && /"op"\s*:\s*"telemetria"/.test(req.postData() || '');

export const caso = {
  nome: 'botoes-da-telemetria',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    await page.evaluate(() => telaTelemetria());
    await page.waitForSelector('#tlmPausar', { timeout: ESPERA });
    // Espera a primeira volta chegar: sem ela a pastilha de estado ainda e «—».
    await page.waitForSelector('#tlmEstado .tlm-pastilha', { timeout: ESPERA });

    const fabrica = chave => page.evaluate(c => txt(c, ''), chave);
    const rotulo = sel => page.$eval(sel, el => el.textContent.trim());
    const classe = sel => page.$eval(sel, el => el.className);

    let pedidos = 0;
    page.on('request', r => { if (ehTelemetria(r)) pedidos++; });

    await capturar(ctx, ctx.nomeCaptura('telemetria-barra'));

    // ------------------------------------------------------------- Pausar
    const pausar = await rotulo('#tlmPausar');
    await page.click('#tlmPausar');
    await page.waitForFunction(() => document.querySelector('.tlm')?.classList.contains('pausado'));
    const retomar = await fabrica('tela.tl_retomar');
    igual(await rotulo('#tlmPausar'), retomar || 'Retomar', 'o rotulo do botao pausado');
    verdade(/incluir/.test(await classe('#tlmPausar')),
      'pausado, o botao vira a cor de INCLUIR (verde): a cor diz o mesmo que a palavra');
    verdade(await page.$('#tlmEstado .tlm-pastilha.pausa'),
      'pausado por vontade tem de aparecer na tela, distinto de servidor que caiu');
    await capturar(ctx, ctx.nomeCaptura('telemetria-pausada'));
    // O relogio de 2 s tem de ficar mudo: janela de ~2,5 voltas.
    pedidos = 0;
    await page.waitForTimeout(5000);
    igual(pedidos, 0, 'pedidos de telemetria durante a pausa (o relogio devia estar parado)');

    // Retomar volta a pedir (na hora ou na proxima volta: o relogio de 2 s cabe na
    // janela, entao o «na hora» nao se isola aqui -- o isolado e o Agora, abaixo).
    const pedido = page.waitForRequest(ehTelemetria, { timeout: 1500 });
    await page.click('#tlmPausar');
    await pedido;
    igual(await rotulo('#tlmPausar'), pausar, 'o rotulo volta a ser o de Pausar');
    verdade(/consultar/.test(await classe('#tlmPausar')), 'a cor volta a ser a de consultar (azul)');
    verdade(!(await page.$('.tlm.pausado')), 'a tela devia sair do estado pausado');

    // -------------------------------------------------------- Atualizar agora
    // PAUSADO, para o relogio nao poder responder pelo botao: com a tela
    // pausada o unico pedido que pode sair e o do clique. (A primeira versao
    // clicava com o relogio andando, e o defeito «Atualizar agora nao faz
    // nada» PASSAVA -- a volta de 2 s chegava na janela e valia pelo botao.)
    await page.click('#tlmPausar');
    await page.waitForFunction(() => document.querySelector('.tlm')?.classList.contains('pausado'));
    await page.waitForTimeout(400);
    const agora = page.waitForRequest(ehTelemetria, { timeout: 1500 });
    await page.click('#tlmAgora');
    await agora;
    await page.click('#tlmPausar');                    // devolve o relogio
    await page.waitForFunction(() => !document.querySelector('.tlm')?.classList.contains('pausado'));

    // ------------------------------------------------------------- Ligar/desligar
    const ligada = async () => !!(await api(page, 'telemetria', { amostras: 1 })).ligada;
    const estavaLigada = await ligada();
    const desligar = await fabrica('tela.tl_desligar');
    const ligar = await fabrica('tela.tl_ligar');
    igual(await rotulo('#tlmLigar'), estavaLigada ? (desligar || 'Desligar coleta') : (ligar || 'Ligar coleta'),
      'o rotulo do botao de coleta diz o que o clique FARA');

    await page.click('#tlmLigar');
    await page.waitForFunction(was => {
      const b = document.querySelector('#tlmLigar');
      return b && (b.classList.contains('incluir') === was);
    }, estavaLigada, { timeout: 15000 });
    igual(await ligada(), !estavaLigada, 'o servidor devia ter trocado o estado da coleta');
    igual(await rotulo('#tlmLigar'), estavaLigada ? (ligar || 'Ligar coleta') : (desligar || 'Desligar coleta'),
      'depois do clique o rotulo se inverte');
    await capturar(ctx, ctx.nomeCaptura('telemetria-coleta-trocada'));

    // Devolve como achou.
    await page.click('#tlmLigar');
    await page.waitForFunction(was => {
      const b = document.querySelector('#tlmLigar');
      return b && (b.classList.contains('incluir') !== was);
    }, estavaLigada, { timeout: 15000 });
    igual(await ligada(), estavaLigada, 'o caso devia devolver a coleta como a achou');

    // ------------------------------------------------------------- Legenda
    const visivel = () => page.$eval('#tlmExplica', el => !el.hidden);
    const aria = () => page.$eval('#tlmLegenda', el => el.getAttribute('aria-expanded'));
    const oculta = await fabrica('tela.tl_ocultar_legenda');
    const mostra = await fabrica('tela.tl_mostrar_legenda');
    igual(await visivel(), true, 'a legenda nasce aberta');
    igual(await aria(), 'true', 'aria-expanded da legenda aberta');
    await page.click('#tlmLegenda');
    await page.waitForFunction(() => document.querySelector('#tlmExplica').hidden);
    igual(await aria(), 'false', 'aria-expanded da legenda fechada');
    igual(await rotulo('#tlmLegenda'), mostra || 'mostrar legenda', 'o rotulo oferece reabrir');
    await page.click('#tlmLegenda');
    await page.waitForFunction(() => !document.querySelector('#tlmExplica').hidden);
    igual(await rotulo('#tlmLegenda'), oculta || 'ocultar legenda', 'o rotulo oferece ocultar');
  },
};

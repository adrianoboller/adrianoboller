/* A tela de entrada, pelo caminho de quem entra.
 *
 * Prova tres coisas que so o navegador prova:
 *  1. a pagina ACHOU o servidor (sem isto ela cai em modo demonstracao, com
 *     dados embutidos, e a bateria inteira passaria sem tocar no motor);
 *  2. o desafio-resposta funciona no navegador de verdade -- `crypto.subtle`
 *     so existe em contexto seguro, e 127.0.0.1 e um;
 *  3. a senha digitada NAO sobra em lugar nenhum do documento depois de
 *     entrar. */
import { entrar, capturar, verdade, contem, igual, CREDENCIAL } from '../apoio.mjs';

export const caso = {
  nome: 'entrada',
  async rodar(ctx) {
    const { page } = ctx;
    await page.goto(ctx.url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await capturar(ctx, ctx.nomeCaptura('login'));

    const modo = await page.textContent('#modo');
    contem(modo, 'Servidor encontrado', 'a tela de entrada nao achou o servidor');
    contem(modo, 'desafio', 'o login nao esta em desafio-resposta em 127.0.0.1');

    // A porta que a tela mostra e a que o servidor REALMENTE escuta -- lida
    // do /saude, e nao o 5000 de fabrica.
    const porta = await page.inputValue('#pt');
    igual(Number(porta), ctx.portaDados, 'a tela nao leu a porta de dados do /saude');

    await entrar(page, ctx.url);
    await capturar(ctx, ctx.nomeCaptura('entrou'));

    contem(await page.textContent('#eu'), CREDENCIAL.USUARIO, 'nao mostrou quem entrou');
    contem(await page.textContent('#fita'), 'papel', 'a fita nao trouxe o ping do servidor');
    verdade(await page.locator('#arvore .no.painel').count() === 1,
      'a arvore nao montou o no do Painel');

    // A senha nunca em texto puro -- nem no HTML da pagina depois do login.
    const html = await page.content();
    verdade(!html.includes(CREDENCIAL.SENHA),
      'a senha digitada ficou no documento depois de entrar');
  },
};

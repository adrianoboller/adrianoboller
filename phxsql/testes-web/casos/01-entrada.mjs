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
    // A fita prova que o `ping` chegou pela VERSAO, e nao mais pela palavra
    // «papel»: o dono mandou tirar «papel isolado» do titulo, porque servidor
    // sozinho e o caso comum e dize-lo o tempo todo e ruido. Este caso travava
    // o comportamento VELHO -- e travava certo, por isso ele reprovou na hora.
    // O que se afirma agora e o que ficou: a versao aparece sempre, e o papel
    // aparece quando NAO e isolado, que e quando ele muda o que a tela faz.
    const fita = await page.textContent('#fita');
    contem(fita, 'v', 'a fita nao trouxe o ping do servidor');
    const papel = await page.evaluate(async () => (await api('ping')).papel);
    if (papel === 'isolado')
      verdade(!/papel/i.test(fita), `«papel isolado» voltou ao titulo: "${fita}"`);
    else
      contem(fita, papel, 'a fita escondeu um papel que NAO e isolado');
    verdade(await page.locator('#arvore .no.painel').count() === 1,
      'a arvore nao montou o no do Painel');

    // A senha nunca em texto puro -- nem no HTML da pagina depois do login.
    const html = await page.content();
    verdade(!html.includes(CREDENCIAL.SENHA),
      'a senha digitada ficou no documento depois de entrar');
  },
};

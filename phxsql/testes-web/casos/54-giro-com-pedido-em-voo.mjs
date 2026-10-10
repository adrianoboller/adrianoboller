/* O giro da sessao com um pedido EM VOO (pedido 784).
 *
 * O `liberar_execucao` (767) gira o id da sessao, e o id velho morre na hora.
 * Um pedido que saiu ANTES com o id velho e demorou -- a trava de dados
 * ocupada, o aquario sondando a cada 2 s -- volta DEPOIS, e o corpo dele traz
 * o id que o servidor leu na chegada: o morto. A `api()` adotava esse id, e o
 * clique seguinte caia em «faca login antes». Foi assim que a
 * `prova-707-aquario.mjs` passou a reprovar: a carga segurava a sondagem do
 * aquario o tempo exato de o `liberar` do `entrar()` passar por cima dela.
 *
 * O teste nao espera a carga acontecer de sorte: segura a RESPOSTA de um
 * `bancos` ja atendido pelo servidor (com o id velho) ate o `liberar`
 * terminar, e so entao a entrega a pagina. Com o defeito, o `bancos`
 * seguinte e recusado; consertado, continua valendo. */
import { entrar, verdade, Falha, SENHA_EXECUCAO } from '../apoio.mjs';

export const caso = {
  nome: 'giro-com-pedido-em-voo',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    let soltar;
    const portao = new Promise(r => { soltar = r; });
    let pegou;
    const atendido = new Promise(r => { pegou = r; });
    let segurou = false;
    await page.route(u => u.href.endsWith('/api'), async route => {
      const corpo = route.request().postData() || '';
      if (segurou || !corpo.includes('"op":"bancos"')) return route.continue();
      segurou = true;
      // O servidor atende AGORA, com o id que esta no cabecalho...
      const resp = await route.fetch();
      pegou();
      // ...e a pagina so recebe depois do giro.
      await portao;
      await route.fulfill({ response: resp });
    });

    const velho = await page.evaluate(() => est.sessao);
    await page.evaluate(() => { window.__emVoo = api('bancos').then(() => 'ok', e => String(e)); });
    await atendido;
    await page.evaluate(s => api('liberar_execucao', { senha: s }, true), SENHA_EXECUCAO);
    const girado = await page.evaluate(() => est.sessao);
    verdade(girado && girado !== velho, 'o liberar_execucao devia girar o id da sessao');

    soltar();
    const doEmVoo = await page.evaluate(() => window.__emVoo);
    await page.unroute(u => u.href.endsWith('/api'));
    verdade(doEmVoo === 'ok', `o pedido em voo devia voltar bem (foi atendido antes do giro): ${doEmVoo}`);

    const depois = await page.evaluate(() => est.sessao);
    const seguinte = await page.evaluate(() => api('bancos').then(() => 'ok', e => String(e)));
    if (seguinte !== 'ok') {
      throw new Falha(`a resposta atrasada ressuscitou o id morto: sessao ${depois === velho ? 'VOLTOU ao id velho' : 'mudou'}, `
        + `e o pedido seguinte foi recusado: ${seguinte.slice(0, 160)}`);
    }
    verdade(depois === girado, 'o id vigente devia continuar o girado');
  },
};

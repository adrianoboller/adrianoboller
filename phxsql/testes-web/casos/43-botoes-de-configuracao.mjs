/* Os botoes das CONFIGURACOES do servidor e do EDITOR DE MENU (pedido 190):
 *
 *   #cfSalvar  #cfDescartar  #cfCoresFabrica  .cf-cor-zero  #cfIrGerais
 *   #emSalvar  #emReverter
 *
 * A configuracao e o lugar onde um botao que "nao faz nada" e mais perigoso:
 * «Salvar» que nao salva deixa quem opera achando que o servidor mudou. Aqui
 * cada botao e conferido no SERVIDOR (`config` devolve o que esta valendo) ou
 * no que a tela do operador passa a mostrar (o menu).
 *
 * O campo mexido e a cor da bolha «normal» da telemetria: e o unico ajuste
 * editavel que e so pintura -- nao muda limite, nem durabilidade, nem rede do
 * servidor da bateria, que os outros casos usam. O caso devolve o campo como
 * achou (de fabrica) num `finally`.
 */
import { entrar, api, verdade, igual, contem, capturar, clicarOuExplicar, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';
const CAMPO = 'telemetria.cor_normal';

export const caso = {
  nome: 'botoes-de-configuracao',
  async rodar(ctx) {
    const desfazer = [];
    try {
      await corpo(ctx, desfazer);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        aviso: document.querySelector('#aviso')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    } finally {
      for (const f of desfazer.reverse()) { try { await f(); } catch { /* ja desfeito */ } }
    }
  },
};

async function corpo(ctx, desfazer) {
  const { page } = ctx;
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });
  const valorNoServidor = async () => {
    const c = await api(page, 'config', {});
    return (c.telemetria || {}).cor_normal ?? '';
  };
  const antes = await valorNoServidor();
  desfazer.push(() => api(page, 'config_gravar', { campos: { [CAMPO]: antes } }));
  // O aviso da gravacao anterior ainda esta na barra: sem limpar, o «gravado»
  // velho valia pelo novo e o caso lia o servidor antes de a segunda gravacao
  // acabar (flocou na corrida inteira de 02/10/2026).
  const limparAviso = () => page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.textContent = ''; });
  const escondido = () => page.$eval(`#painel [data-campo="${CAMPO}"]`, e => e.value);

  // --------------------------------- da leitura para a tela que grava
  passo = 'configuracao: Editar em Gerais do servidor';
  await page.evaluate(() => verConfig());
  await esperar('#cfIrGerais');
  await capturar(ctx, ctx.nomeCaptura('configuracao-so-leitura'));
  await clicarOuExplicar(page, '#cfIrGerais');
  await esperar('#cfSalvar');
  await some('#cfIrGerais');
  verdade(await page.$(`#painel [data-campo="${CAMPO}"]`), 'a tela de gerais devia trazer o campo de cor da bolha');

  // ----------------------------------------------- escolher e voltar ao de fabrica
  passo = 'cores: escolher, de fabrica e voltar todas';
  const bloco = '#painel .cmp-cor[data-nivel="normal"]';
  await page.fill(`${bloco} .cf-cor-p`, '#336699');
  igual(await escondido(), '#336699', 'a cor escolhida devia ir para o campo que sera gravado');
  contem(await page.textContent(`${bloco} .cf-cor-diz`), 'contraste', 'a escolha nao mostrou a conta de contraste');
  await capturar(ctx, ctx.nomeCaptura('cor-escolhida'));
  await clicarOuExplicar(page, `${bloco} .cf-cor-zero`);
  igual(await escondido(), '', '«de fabrica» devia limpar o campo');
  contem(await page.textContent(`${bloco} .cf-cor-diz`), 'fábrica', 'o recado devia dizer que voltou ao tema');

  // Todas as quatro de uma vez: o botao do rodape do grupo.
  for (const n of ['normal', 'atento', 'stress', 'critico']) {
    const b = `#painel .cmp-cor[data-nivel="${n}"] .cf-cor-p`;
    if (await page.$(b)) await page.fill(b, '#aa5500');
  }
  const preenchidas = await page.$$eval('#painel .cmp-cor input[type=hidden]', xs => xs.filter(x => x.value).length);
  verdade(preenchidas >= 1, 'o preparo devia ter pintado ao menos uma cor');
  await clicarOuExplicar(page, '#cfCoresFabrica');
  const sobraram = await page.$$eval('#painel .cmp-cor input[type=hidden]', xs => xs.filter(x => x.value).length);
  igual(sobraram, 0, 'cores ainda escolhidas depois de «Voltar as cores de fabrica»');

  // ------------------------------------------------------- descartar
  passo = 'descartar as mudancas';
  await page.fill(`${bloco} .cf-cor-p`, '#112233');
  igual(await escondido(), '#112233', 'preparo do descartar');
  await clicarOuExplicar(page, '#cfDescartar');
  await page.waitForFunction(c => document.querySelector(`#painel [data-campo="${c}"]`)?.value === '', CAMPO, { timeout: ESPERA });
  igual(await valorNoServidor(), antes, 'Descartar nao pode gravar nada');

  // ---------------------------------------------------------- salvar
  passo = 'salvar no config.json';
  await clicarOuExplicar(page, '#cfSalvar');                     // nada mudou: nao grava
  await page.waitForFunction(() => /nada mudou|nothing/i.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: ESPERA });
  igual(await valorNoServidor(), antes, '«Salvar» sem mudanca nao pode gravar');

  await page.fill(`${bloco} .cf-cor-p`, '#336699');
  await limparAviso();
  await clicarOuExplicar(page, '#cfSalvar');
  await page.waitForFunction(() => /gravado/i.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: ESPERA });
  igual(await valorNoServidor(), '#336699', 'o servidor devia estar com a cor que a tela salvou');
  // A tela repinta do servidor: o campo agora e o gravado, e nao o digitado.
  await page.waitForFunction(c => document.querySelector(`#painel [data-campo="${c}"]`)?.value === '#336699', CAMPO, { timeout: ESPERA });
  await capturar(ctx, ctx.nomeCaptura('cor-salva'));
  // E a telemetria PASSA A USAR a cor: a mesma amostra que o painel desenha.
  await clicarOuExplicar(page, `${bloco} .cf-cor-zero`);
  await limparAviso();
  await clicarOuExplicar(page, '#cfSalvar');
  await page.waitForFunction(() => /gravado/i.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: ESPERA });
  igual(await valorNoServidor(), '', 'voltar ao de fabrica e salvar devia limpar a cor no servidor');

  // ------------------------------------------------- editor de menu
  passo = 'editor de menu: aplicar e voltar aos nomes de fabrica';
  await page.evaluate(() => editorDeMenu());
  await esperar('#emSalvar');
  const titulo0 = () => page.textContent('.menubar .titulo[data-m="0"]');
  const original = (await titulo0()).trim();
  desfazer.push(() => page.evaluate(() => { localStorage.removeItem('phxsql.rotulos'); }));
  await page.fill('#painel .em-rot[data-chave="menu.0"]', 'Meu menu');
  await clicarOuExplicar(page, '#emSalvar');
  await page.waitForFunction(() => /Meu menu/.test(document.querySelector('.menubar .titulo[data-m="0"]')?.textContent || ''), undefined, { timeout: ESPERA });
  // Rotulo se estiliza; dado, nunca -- e o nome trocado e rotulo: sai como digitado.
  contem(await titulo0(), 'Meu menu', 'o menu devia mostrar o nome escolhido, na caixa em que foi digitado');
  await capturar(ctx, ctx.nomeCaptura('menu-renomeado'));
  contem(await page.textContent('#subtitulo'), '1 trocado', 'o editor devia contar um rotulo trocado');
  await clicarOuExplicar(page, '#emReverter');
  await page.waitForFunction(o => (document.querySelector('.menubar .titulo[data-m="0"]')?.textContent || '').trim() === o, original, { timeout: ESPERA });
  contem(await page.textContent('#subtitulo'), '0 trocado', 'depois de reverter nao devia sobrar rotulo trocado');
}

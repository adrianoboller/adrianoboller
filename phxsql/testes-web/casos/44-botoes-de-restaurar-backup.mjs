/* Os botoes de RESTAURAR UM BACKUP (pedido 190):
 *
 *   [data-origem]  #btRstNovo  #btRstParar  #btRstPorCima
 *   #btVerRestaurado  #btRstOutra
 *
 * Restaurar e o botao que mais custa errar: «por cima» troca um banco inteiro.
 * Por isso cada passo confere o DADO no servidor e nao a mensagem da tela:
 *
 *   - com outro nome: o banco novo existe e traz as linhas da copia, e o
 *     original nao foi tocado;
 *   - por cima: o botao nasce TRAVADO, so libera quando o nome digitado e
 *     exatamente o do banco, exige a porta de dados parada (o «Start/Stop»
 *     leva para o servico), e a linha que entrou DEPOIS da copia some -- que
 *     e o que restaurar quer dizer;
 *   - o banco substituido nao e apagado: a tela diz onde ficou.
 *
 * O banco e o deste caso, a porta de dados e devolvida num `finally`.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso, cenario, clicarOuExplicar, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'botoes-de-restaurar-backup',
  async rodar(ctx) {
    try {
      await corpo(ctx);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        aviso: document.querySelector('#aviso')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    } finally {
      try {
        const s = await ctx.page.evaluate(() => api('servico'));
        if (!s.no_ar) await ctx.page.evaluate(b => api('servico_subir', { bind: b }), s.endereco || s.bind_configurado);
      } catch { /* a pagina ja caiu */ }
    }
  },
};

async function corpo(ctx) {
  const { page } = ctx;
  const db = bancoDoCaso(ctx, 'Rst');
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const linhas = async banco => ((await api(page, 'varrer', { database: banco, tabela: 'clientes', limite: 100 })).linhas || []);

  passo = 'preparo: um banco com tres linhas e uma copia dele';
  await cenario(page, db, 'clientes');
  igual((await linhas(db)).length, 3, 'linhas do banco antes da copia');
  const cfg = await api(page, 'config', {});
  const destinoCfg = ((cfg.backup || {}).destino || '').trim();
  const copia = await api(page, 'backup', { database: db, ...(destinoCfg ? { destino: destinoCfg } : {}) });
  verdade(copia.destino, `o backup nao disse onde gravou: ${JSON.stringify(copia).slice(0, 200)}`);
  const pasta = copia.destino.replace(/\/[^/]+$/, '');
  const arquivo = copia.destino.split('/').pop();

  const abrirDetalhe = async () => {
    await page.evaluate(p => telaRestaurarBackup(p), pasta);
    await esperar('#btRstListar');
    await esperar(`#gradeBackups [data-origem$="${arquivo}"][data-de="${db}"]`);
    await clicarOuExplicar(page, `#gradeBackups [data-origem$="${arquivo}"][data-de="${db}"]`);
    await esperar('#btRstNovo');
  };

  // ------------------------------------------------ restaurar com outro nome
  passo = 'restaurar com outro nome';
  await abrirDetalhe();
  contem(await page.textContent('#rstDetalhe .secao'), db, 'o detalhe da copia mudou a caixa do nome do banco');
  await capturar(ctx, ctx.nomeCaptura('detalhe-da-copia'));
  const novo = `${db}_novo`;
  await page.fill('#rstNome', novo);
  await clicarOuExplicar(page, '#btRstNovo');
  await esperar('#btVerRestaurado');
  await esperar('#btRstOutra');
  igual((await linhas(novo)).length, 3, 'o banco restaurado com outro nome devia trazer as tres linhas da copia');
  igual((await linhas(db)).length, 3, 'restaurar com outro nome nao pode tocar no original');
  await capturar(ctx, ctx.nomeCaptura('restaurado'));
  await clicarOuExplicar(page, '#btVerRestaurado');
  await page.waitForFunction(n => (document.querySelector('#titulo')?.textContent || '').includes(n), novo, { timeout: ESPERA });
  await esperar('#btNovaTab');

  passo = 'restaurar outra copia';
  await abrirDetalhe();
  await page.fill('#rstNome', `${db}_outro`);
  await clicarOuExplicar(page, '#btRstNovo');
  await esperar('#btRstOutra');
  await clicarOuExplicar(page, '#btRstOutra');
  await esperar('#btRstListar');
  verdade(!(await page.$('#btRstOutra')), '«Restaurar outra copia» devia voltar a tela de procurar copias');

  // ----------------------------------------------------------- por cima
  passo = 'por cima: travado ate digitar o nome, e exige a porta parada';
  // A linha que entra DEPOIS da copia e a prova de que restaurar desfaz.
  await api(page, 'inserir', { database: db, tabela: 'clientes', valores: [9, 'Depois da copia', 'Itajaí', 'SC', '1.00', '2026-10-02', ''] });
  igual((await linhas(db)).length, 4, 'preparo: a linha extra');
  await abrirDetalhe();
  verdade(await page.$eval('#btRstPorCima', b => b.disabled), 'Substituir nao pode nascer liberado');
  await page.fill('#rstConfirma', db.slice(0, -1));
  verdade(await page.$eval('#btRstPorCima', b => b.disabled), 'um nome pela metade nao libera o Substituir');
  await page.fill('#rstConfirma', db);
  verdade(!(await page.$eval('#btRstPorCima', b => b.disabled)), 'o nome exato devia liberar o Substituir');

  // Com a porta no ar o servidor RECUSA, e a tela mostra por que.
  await clicarOuExplicar(page, '#btRstPorCima');
  await esperar('#rstErro .nota');
  igual((await linhas(db)).length, 4, 'a recusa nao pode ter mexido no banco');
  await capturar(ctx, ctx.nomeCaptura('por-cima-recusado'));

  // «Start/Stop» leva ao servico; ali se para a porta.
  await clicarOuExplicar(page, '#btRstParar');
  await esperar('#btSvParar');
  page.once('dialog', d => d.accept());
  await clicarOuExplicar(page, '#btSvParar');
  await page.waitForFunction(() => !document.querySelector('#btSvParar'), undefined, { timeout: ESPERA });

  passo = 'por cima: com a porta parada, substitui';
  await abrirDetalhe();
  await page.fill('#rstConfirma', db);
  await clicarOuExplicar(page, '#btRstPorCima');
  await esperar('#btVerRestaurado');
  igual((await linhas(db)).length, 3, 'a linha que entrou depois da copia devia ter sumido');
  contem(await page.textContent('#painel'), 'não foi apagado', 'a tela devia dizer onde ficou o banco substituido');
  await capturar(ctx, ctx.nomeCaptura('por-cima-feito'));
}

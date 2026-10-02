/* Os botoes que MEXEM NO SERVIDOR VIVO: sessoes, servico e profiler
 * (pedido 190). Eram os que a bateria nao clicava porque o unico cliente do
 * servidor dela e a propria pagina -- derruba-lo derrubaria o caso.
 *
 *   #btEstat  #btSessoes  [data-kill]  [data-killweb]
 *   #btSvVer  #btSvParar  #btSvSubir
 *   #pfLigar  #pfParar  #pfLimpar
 *
 * O segundo cliente e REAL: `conexaoViva` abre um soquete na porta de dados e
 * fala um `ping` (apoio.mjs). A prova do «Encerrar» nao e a linha sumir da
 * tela -- e o soquete cair, visto de FORA, por quem estava do outro lado. O
 * «Parar a porta de dados» idem: um cliente novo tem de levar recusa de
 * conexao, e o que ja estava conectado segue (e o que a nota da tela promete).
 *
 * A porta de dados e devolvida num `finally`: caso que para a porta e morre no
 * meio deixaria o assistente de replicacao e o DbLink (que sondam a porta) sem
 * servidor, e a falha apareceria dois casos depois, no lugar errado.
 */
import { connect } from 'node:net';
import { entrar, api, verdade, igual, contem, capturar, clicarOuExplicar, conexaoViva, CREDENCIAL, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

const tentarConectar = porta => new Promise(res => {
  const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); res(true); });
  s.on('error', () => res(false));
  s.setTimeout(1500, () => { s.destroy(); res(false); });
});

export const caso = {
  nome: 'botoes-de-sessoes-servico-e-profiler',
  async rodar(ctx) {
    const extras = [];
    try {
      await corpo(ctx, extras);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        aviso: document.querySelector('#aviso')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    } finally {
      // Devolve o servidor como achou: porta de dados no ar, profiler desligado.
      try {
        const s = await ctx.page.evaluate(() => api('servico'));
        if (!s.no_ar) await ctx.page.evaluate(b => api('servico_subir', { bind: b }), s.endereco || s.bind_configurado);
        await ctx.page.evaluate(() => api('profiler_desligar')).catch(() => {});
      } catch { /* a pagina ja caiu: nada a devolver por ela */ }
      for (const x of extras) { try { await x(); } catch { /* ja fechado */ } }
    }
  },
};

async function corpo(ctx, extras) {
  const { page } = ctx;
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });

  // ------------------------------------------------ sessoes: encerrar uma conexao
  passo = 'sessoes: encerrar uma conexao';
  const a = await conexaoViva(ctx);
  const b = await conexaoViva(ctx);
  extras.push(() => a.derrubar(), () => b.derrubar());

  await page.evaluate(() => verSessoes());
  await esperar('#gradeSessoes .bt-kill');
  const celula = porta => `#gradeSessoes tbody tr:has(td:has-text(":${porta}")) .bt-kill`;
  await esperar(celula(a.local));
  await capturar(ctx, ctx.nomeCaptura('sessoes'));

  // Recusar o confirm: a conexao CONTINUA.
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, celula(a.local));
  await page.waitForTimeout(700);
  verdade(a.aberta(), 'recusar o confirm do Encerrar derrubou a conexao assim mesmo');

  // Aceitar: o soquete cai, VISTO DE FORA; a vizinha nao.
  page.once('dialog', d => d.accept());
  await clicarOuExplicar(page, celula(a.local));
  const caiu = await Promise.race([a.fechou, new Promise(r => setTimeout(() => r(false), 8000))]);
  verdade(caiu, 'o Encerrar nao fechou o soquete do cliente');
  verdade(b.aberta(), 'o Encerrar derrubou tambem a conexao vizinha');
  await page.waitForFunction(porta => !document.querySelector('#gradeSessoes')?.textContent.includes(':' + porta),
    a.local, { timeout: ESPERA });

  // --------------------------------------------- sessoes: encerrar uma sessao web
  passo = 'sessoes: encerrar uma sessao web';
  const outro = await ctx.page.context().newPage();
  extras.push(() => outro.close());
  await entrar(outro, ctx.url);
  // ACHADO NOMEADO, NAO CONSERTADO (e de `servidor.rs`, que e de outra frente):
  // `op_encerrar_sessao` decide web x conexao por «o id tem alguma letra». O id
  // da sessao web e hexadecimal de 8 digitos, e 2,3% deles ((10/16)^8) saem so
  // com algarismos -- ai o servidor o toma por NUMERO DE CONEXAO e responde
  // «encerrar_sessao sem "id"» (ou derrubaria a conexao de mesmo numero). O caso
  // nao pode depender dessa sorte: sorteia outra sessao ate o id ter uma letra.
  for (let i = 0; i < 12 && /^\d+$/.test(await outro.evaluate(() => est.sessao.slice(0, 8))); i++) {
    await outro.evaluate(() => api('sair').catch(() => {}));
    await entrar(outro, ctx.url);
    ctx.notas.push('id de sessao web so com algarismos: sorteada outra (defeito do servidor, ver o comentario do caso)');
  }
  await page.evaluate(() => verSessoes());
  await esperar('#gradeSessoesWeb [data-killweb]');
  await page.waitForFunction(() => document.querySelectorAll('#gradeSessoesWeb [data-killweb]').length >= 2,
    undefined, { timeout: ESPERA });
  // O alvo e a sessao do OUTRO navegador, achada pelo id dele -- e nao «a que
  // nao e a minha»: o servidor da bateria e UM so para a corrida inteira, e as
  // sessoes de casos anteriores (e do outro tema) continuam na lista.
  const idDoOutro = await outro.evaluate(() => est.sessao);
  const alvoWeb = await page.evaluate(id => {
    const bt = [...document.querySelectorAll('#gradeSessoesWeb [data-killweb]')]
      .find(x => id.startsWith(x.getAttribute('data-killweb')));
    return bt ? bt.getAttribute('data-killweb') : null;
  }, idDoOutro);
  verdade(alvoWeb, 'nao achei a sessao web do outro navegador na lista');
  await capturar(ctx, ctx.nomeCaptura('sessoes-web'));
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, `#gradeSessoesWeb [data-killweb="${alvoWeb}"]`);
  await page.waitForTimeout(500);
  await outro.evaluate(() => api('bancos'));                    // continua valendo
  page.once('dialog', d => d.accept());
  await clicarOuExplicar(page, `#gradeSessoesWeb [data-killweb="${alvoWeb}"]`);
  // O proximo pedido de quem estava la cai no login.
  let caiuNoLogin = false;
  for (let i = 0; i < 20 && !caiuNoLogin; i++) {
    caiuNoLogin = await outro.evaluate(() => api('bancos').then(() => false, () => true));
    if (!caiuNoLogin) await page.waitForTimeout(250);
  }
  verdade(caiuNoLogin, 'encerrar a sessao web nao tirou o outro navegador da sessao');
  verdade(await page.evaluate(() => api('bancos').then(() => true, () => false)),
    'o Encerrar tirou a PROPRIA sessao de quem clicou');

  // ----------------------------------------- sessoes <-> estatisticas
  passo = 'sessoes <-> estatisticas';
  await esperar('#btEstat');
  await clicarOuExplicar(page, '#btEstat');
  await esperar('#btSessoes');
  await some('#btEstat');
  await capturar(ctx, ctx.nomeCaptura('estatisticas'));
  await clicarOuExplicar(page, '#btSessoes');
  await esperar('#gradeSessoes');

  // ------------------------------------------------------------- servico
  passo = 'servico: atualizar, parar e subir';
  await page.evaluate(() => verServico());
  await esperar('#btSvParar');
  const antes = await page.textContent('#painel .fichas');
  await clicarOuExplicar(page, '#btSvVer');
  await page.waitForTimeout(600);
  await esperar('#btSvParar');
  contem(antes, String(ctx.portaDados), 'o servico nao diz em que porta escuta');

  // Recusar o confirm: a porta continua.
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, '#btSvParar');
  await page.waitForTimeout(800);
  verdade(await tentarConectar(ctx.portaDados), 'recusar o confirm parou a porta de dados');

  // Aceitar: cliente NOVO leva recusa; quem ja estava conectado, segue.
  page.once('dialog', d => d.accept());
  await clicarOuExplicar(page, '#btSvParar');
  await some('#btSvParar');
  verdade(!(await tentarConectar(ctx.portaDados)), 'a porta de dados continuou aceitando conexao nova depois de Parar');
  verdade(b.aberta(), 'Parar derrubou uma conexao que ja estava aberta (a nota da tela promete o contrario)');
  contem(await page.textContent('#painel .fichas'), 'parada', 'a tela nao diz que a porta esta parada');
  await capturar(ctx, ctx.nomeCaptura('servico-parado'));

  // Subir (sem confirm: nao ha porta a trocar, so a religar).
  const rotuloSubir = await page.textContent('#btSvSubir');
  await clicarOuExplicar(page, '#btSvSubir');
  await esperar('#btSvParar');
  verdade(await tentarConectar(ctx.portaDados), 'Subir nao religou a porta de dados');
  verdade(rotuloSubir !== await page.textContent('#btSvSubir'),
    'o botao devia mudar de «Subir nesta porta» para «Trocar para esta porta»');

  // ------------------------------------------------------------- profiler
  passo = 'profiler: ligar, limpar e parar';
  await page.evaluate(() => verProfiler());
  await esperar('#pfLigar');
  verdade(await page.$eval('#pfParar', x => x.disabled), 'Parar nao pode estar liberado com o profiler desligado');
  await page.fill('#pfOp', 'ping');
  await clicarOuExplicar(page, '#pfLigar');
  await page.waitForFunction(() => !document.querySelector('#pfParar')?.disabled, undefined, { timeout: ESPERA });
  verdade((await api(page, 'profiler', {})).ligado === true, 'Ligar nao ligou o profiler no servidor');
  // Tráfego: o filtro e «ping», entao um ping da conexao viva tem de aparecer.
  b.socket.write(JSON.stringify({ op: 'ping', token: CREDENCIAL.TOKEN }) + '\n');
  await page.waitForFunction(() => /ping/.test(document.querySelector('#pfGrade')?.textContent || ''),
    undefined, { timeout: ESPERA });
  await capturar(ctx, ctx.nomeCaptura('profiler-ligado'));

  await clicarOuExplicar(page, '#pfLimpar');
  await page.waitForFunction(() => !/ping/.test(document.querySelector('#pfGrade .phx-grid tbody')?.textContent || ''),
    undefined, { timeout: ESPERA });
  igual((await api(page, 'profiler', {})).ligado, true, 'Limpar nao pode desligar o profiler');

  await clicarOuExplicar(page, '#pfParar');
  await page.waitForFunction(() => document.querySelector('#pfParar')?.disabled, undefined, { timeout: ESPERA });
  verdade((await api(page, 'profiler', {})).ligado === false, 'Parar nao desligou o profiler no servidor');
}

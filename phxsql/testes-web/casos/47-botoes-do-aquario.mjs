/* Os botoes do AQUARIO (pedido 707, A12 e A13):
 *
 *   .aqt-bt-volta  .aqt-bt-tocar  .aqt-bt-vivo      a volta de 5 minutos
 *   .aqt-k-encerrar  .aqt-k-derrubar  .aqt-k-fechar o cartao de encerrar
 *
 * A volta existe sempre; o cartao so com uma tarefa de OUTRA conexao viva no
 * tanque. A tarefa e a do caso 41, pelo mesmo motivo medido la: uma carga de
 * 480 mil linhas que o servidor fica convertendo (operacao esperando trava
 * nao se consegue provocar, e leitura nao bloqueia). A propria sessao da tela
 * nao serve: o servidor recusa encerrar a atividade de quem pergunta.
 *
 * O que se prova aqui e que cada botao RESPONDE ao clique como diz: a volta
 * poe o selo em REPRISE e o «Ao vivo» o tira; «Encerrar a operacao» manda o
 * `telemetria_encerrar` com a tarefa da bolha (id com o serial) e o cartao
 * diz o desfecho pela chave; «Derrubar a conexao» fecha o soquete visto de
 * FORA; «Fechar» esconde o cartao. A prova pesada -- a TV, o VELHO medido com
 * o servidor morto, quem so monitora -- e a `testes-web/prova-707-aquario.mjs`.
 */
import { entrar, api, verdade, igual, capturar, clicarOuExplicar, conexaoViva, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'botoes-do-aquario',
  async rodar(ctx) {
    const extras = [];
    try {
      await corpo(ctx, extras);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        estado: document.querySelector('#aqTela .aqt-estado')?.textContent,
        selo: document.querySelector('#aqTela .aqt-selo')?.textContent,
        res: document.querySelector('#aqTela .aqt-res')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    } finally {
      for (const x of extras) { try { await x(); } catch { /* ja desfeito */ } }
    }
  },
};

async function corpo(ctx, extras) {
  const { page } = ctx;
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });

  passo = 'abrir o aquario';
  const r0 = await api(page, 'aquario_retrato');
  if (r0.ligada === false) {
    await api(page, 'telemetria_ligar');
    extras.push(() => api(page, 'telemetria_desligar'));
  }
  await page.evaluate(() => PhxTelas.abrir('aquario'));
  await esperar('#aqTela .aqt-bt-volta');
  await page.waitForFunction(() => document.querySelector('#aqTela .aqt-selo')?.dataset.estado === 'fresco',
    undefined, { timeout: ESPERA });

  // --------------------------------------------------- a volta de 5 min
  passo = 'voltar 5 min, pausar e voltar ao vivo';
  await clicarOuExplicar(page, '#aqTela .aqt-bt-volta');
  await page.waitForFunction(() => document.querySelector('#aqTela .aqt-selo')?.dataset.estado === 'reprise',
    undefined, { timeout: ESPERA });
  await esperar('#aqTela .aqt-bt-tocar');
  const antes = await page.textContent('#aqTela .aqt-bt-tocar');
  await clicarOuExplicar(page, '#aqTela .aqt-bt-tocar');
  const depois = await page.textContent('#aqTela .aqt-bt-tocar');
  verdade(antes !== depois, `o botao de reproduzir/pausar nao mudou: «${antes}» -> «${depois}»`);
  await capturar(ctx, ctx.nomeCaptura('reprise'));
  await clicarOuExplicar(page, '#aqTela .aqt-bt-vivo');
  await page.waitForFunction(() => document.querySelector('#aqTela .aqt-selo')?.dataset.estado === 'fresco',
    undefined, { timeout: ESPERA });
  igual(await page.isVisible('#aqTela .aqt-bt-volta'), true, 'ao vivo de novo, o «Voltar 5 min» volta');

  // ------------------------------------------------- o cartao de encerrar
  passo = 'preparo: uma carga longa em outra conexao';
  const c = await conexaoViva(ctx, { login: true });
  extras.push(() => c.derrubar());
  const ses = (await api(page, 'sessoes')).sessoes;
  const s = ses.find(x => x.porta === c.local);
  verdade(s, `a sessao da porta ${c.local} nao esta na lista do servidor`);
  const prefixo = `dados:${s.id}#`;
  const db = `batAq${ctx.tema === 'claro' ? 'C' : 'E'}`;
  await api(page, 'criar_database', { database: db }).catch(() => {});
  extras.push(() => api(page, 'excluir_tabela', { database: db, tabela: 'carga', confirmar: 'carga' }));
  await api(page, 'criar_tabela', {
    database: db, tabela: 'carga',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }, { nome: 'nome', tipo: 'Str(40)' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  // ~61 MB previstos no diario, abaixo do teto de 64 MiB (o caso 41 mediu)
  const N = 480000;
  const partes = ['id;nome\n'];
  for (let i = 1; i <= N; i++) partes.push(`${i};Cliente numero ${i}\n`);
  c.enviar({ op: 'inserir_lote', database: db, tabela: 'carga', texto: partes.join(''), formato: 'csv' });

  passo = 'a bolha da carga abre o cartao';
  // Espera pelo EVENTO (a bolha da carga no tanque), com prazo so de
  // desistencia: o servidor primeiro le o pedido inteiro, e quanto isso leva
  // depende da maquina (o caso 41 mediu de 2 s a mais de um minuto).
  const sel = `#aqTela .aq-b[data-id^="${prefixo}"]:not(.aq-fim)`;
  const t0 = Date.now();
  let id = null;
  while (!id && c.pendentes() === 0 && Date.now() - t0 < 120000) {
    // a bolha da CARGA, e nao a do login que a mesma conexao fez antes
    id = await page.evaluate(s => [...document.querySelectorAll(s)]
      .find(g => /^inserir_lote /.test(g.querySelector('title')?.textContent || ''))?.getAttribute('data-id') || null, sel);
    if (!id) await page.waitForTimeout(300);
  }
  verdade(id, `a carga nunca virou bolha (${Date.now() - t0} ms; ja respondeu=${c.pendentes() > 0})`);
  // A bolha desliza: o clique vai pelo ouvinte dela, como no caso 41; os
  // BOTOES do cartao recebem clique de verdade.
  await page.evaluate(i => document.querySelector(`#aqTela .aq-b[data-id="${i}"] .aq-forma`)
    .dispatchEvent(new MouseEvent('click', { bubbles: true })), id);
  await esperar('#aqTela .aqt-cartao:not([hidden])');
  const ficha = await page.textContent('#aqTela .aqt-ficha');
  verdade(ficha.includes(id) && ficha.includes('inserir_lote'), `o cartao nao descreve a bolha escolhida: «${ficha}»`);
  await capturar(ctx, ctx.nomeCaptura('cartao'));

  passo = 'encerrar a operacao pelo cartao';
  const pedido = page.waitForRequest(r => /"op"\s*:\s*"telemetria_encerrar"/.test(r.postData() || ''), { timeout: 5000 });
  await clicarOuExplicar(page, '#aqTela .aqt-k-encerrar');
  const enviado = await pedido;
  verdade((enviado.postData() || '').includes(id), `o encerrar devia mirar ${id}: ${enviado.postData()}`);
  await esperar('#aqTela .aqt-res[data-desfecho]');
  const desfecho = await page.getAttribute('#aqTela .aqt-res', 'data-desfecho');
  // «erro» so se a carga terminou entre o clique na bolha e o no botao -- e
  // ai o servidor diz «ja terminou», que tambem e a resposta certa.
  verdade(['encerrando', 'marcada', 'nao_cancelavel', 'erro'].includes(desfecho), `desfecho inesperado: ${desfecho}`);
  ctx.notas.push(`o cartao disse: ${desfecho}`);

  passo = 'derrubar a conexao pelo cartao';
  await esperar('#aqTela .aqt-k-derrubar');
  await clicarOuExplicar(page, '#aqTela .aqt-k-derrubar');
  const caiu = await Promise.race([c.fechou, new Promise(r => setTimeout(() => r(false), 10000))]);
  verdade(caiu, 'o «Derrubar a conexao» do cartao nao fechou o soquete');

  passo = 'fechar o cartao';
  await clicarOuExplicar(page, '#aqTela .aqt-k-fechar');
  await page.waitForFunction(() => document.querySelector('#aqTela .aqt-cartao')?.hidden === true,
    undefined, { timeout: ESPERA });
}

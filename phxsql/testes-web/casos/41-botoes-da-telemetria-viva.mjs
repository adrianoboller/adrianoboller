/* Os botoes da telemetria que pedem CONEXAO VIVA do outro lado (pedido 190):
 *
 *   #tlmEstacao  [data-nivel]  #tlmDerrubar  #tlmEncerrar
 *
 * O caso 36 provou a barra do painel; estes quatro atuam sobre OUTRA pessoa e
 * por isso so existem com gente conectada. Aqui "gente" sao soquetes de
 * verdade na porta de dados (`conexaoViva`, apoio.mjs): o "Derrubar" e provado
 * pelo soquete cair VISTO DE FORA, e o "Encerrar a operacao" por uma carga de
 * 1,6 milhao de linhas que o servidor esta lendo no momento do clique.
 *
 * Por que uma carga enorme e nao uma operacao "esperando trava": medido
 * (sonda de 02/10/2026) que escrita concorrente em outra tabela nao espera, a
 * leitura nao bloqueia, e a escrita contra tabela em transacao e RECUSADA na
 * hora (EM_TRANSACAO) -- nenhuma das tres deixa uma operacao viva por tempo
 * suficiente para clicar. Uma carga de ~23 MB por 800 mil linhas leva ~4,7 s;
 * a de 1,6 milhao, o dobro.
 *
 * O relogio da tela fica PAUSADO durante os cliques: o cartao da bolha e
 * reescrito a cada volta, e um clique que cai na troca some (o caso 36 ja
 * pagou isso). `Atualizar agora` traz o retrato quando o caso o quer.
 */
import { entrar, api, verdade, igual, contem, capturar, clicarOuExplicar, conexaoViva, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'botoes-da-telemetria-viva',
  async rodar(ctx) {
    const extras = [];
    try {
      await corpo(ctx, extras);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        aviso: document.querySelector('#aviso')?.textContent,
        trilha: document.querySelector('#tlmTrilha')?.textContent,
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

  passo = 'preparo: tres conexoes vivas na mesma estacao';
  const a = await conexaoViva(ctx);
  const b = await conexaoViva(ctx);
  const c = await conexaoViva(ctx, { login: true });
  extras.push(() => a.derrubar(), () => b.derrubar(), () => c.derrubar());
  const idDe = async conexao => {
    const ses = (await api(page, 'sessoes')).sessoes;
    const s = ses.find(x => x.porta === conexao.local);
    verdade(s, `a sessao da porta ${conexao.local} nao esta na lista do servidor`);
    return `dados:${s.id}`;
  };
  const ida = await idDe(a), idc = await idDe(c);

  await page.evaluate(() => telaTelemetria());
  await esperar('#tlmPausar');
  await page.waitForSelector(`#tlmBolhas [data-id="${ida}"]`, { timeout: ESPERA });
  // Pausa: o cartao deixa de ser reescrito sozinho. A bolha e escolhida pelo
  // proprio ouvinte dela (e desenhada em SVG que desliza; o clique do mouse
  // erra o alvo em movimento), e os BOTOES do cartao recebem clique de verdade.
  await clicarOuExplicar(page, '#tlmPausar');
  await page.waitForFunction(() => document.querySelector('.tlm')?.classList.contains('pausado'));
  const agora = async () => {
    const volta = page.waitForRequest(r => r.method() === 'POST' && /"op"\s*:\s*"telemetria"/.test(r.postData() || ''), { timeout: 5000 });
    await clicarOuExplicar(page, '#tlmAgora');
    await volta;
    await page.waitForTimeout(400);
  };
  const escolher = async id => {
    await page.waitForSelector(`#tlmBolhas [data-id="${id}"]`, { timeout: ESPERA });
    await page.evaluate(i => document.querySelector(`#tlmBolhas [data-id="${i}"]`)
      .dispatchEvent(new MouseEvent('click', { bubbles: true })), id);
  };

  // --------------------------------------------- a estacao e a trilha
  passo = 'ver as conexoes da estacao e subir pela trilha';
  await escolher(ida);
  await esperar('#tlmEstacao');
  const rotuloEstacao = await page.textContent('#tlmEstacao');
  verdade(/[3-9]|\d\d/.test(rotuloEstacao), `o botao da estacao devia contar as irmas: «${rotuloEstacao}»`);
  await capturar(ctx, ctx.nomeCaptura('cartao-da-conexao'));
  await clicarOuExplicar(page, '#tlmEstacao');
  await page.waitForFunction(() => !document.querySelector('#tlmEstacao'), undefined, { timeout: ESPERA });
  contem(await page.textContent('#tlmTrilha'), '127.0.0.1', 'a trilha nao mostra a estacao em que se entrou');
  verdade(await page.locator('#tlmBolhas [data-id^="dados:"]').count() >= 3,
    'dentro da estacao as conexoes dela tem de estar desenhadas');
  await capturar(ctx, ctx.nomeCaptura('dentro-da-estacao'));

  await clicarOuExplicar(page, '[data-nivel="estacoes"]');
  await page.waitForFunction(() => document.querySelectorAll('#tlmBolhas [data-id^="estacao:"]').length >= 1
    && document.querySelectorAll('#tlmBolhas [data-id^="dados:"]').length === 0, undefined, { timeout: ESPERA });
  await capturar(ctx, ctx.nomeCaptura('por-estacao'));
  verdade(await page.$('[data-nivel="estacoes"][aria-current="true"]'), '«Por estacao» devia ser o passo atual da trilha');

  await clicarOuExplicar(page, '[data-nivel="todas"]');
  await page.waitForFunction(() => document.querySelectorAll('#tlmBolhas [data-id^="estacao:"]').length === 0
    && document.querySelectorAll('#tlmBolhas [data-id^="dados:"]').length >= 3, undefined, { timeout: ESPERA });
  verdade(await page.$('[data-nivel="todas"][aria-current="true"]'), '«Atividades» devia ser o passo atual da trilha');

  // --------------------------------------------------- derrubar a conexao
  passo = 'derrubar a conexao';
  await escolher(ida);
  await esperar('#tlmDerrubar');
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, '#tlmDerrubar');
  await page.waitForTimeout(700);
  verdade(a.aberta(), 'recusar o confirm do Derrubar fechou o soquete assim mesmo');
  await esperar('#tlmDerrubar');
  page.once('dialog', d => d.accept());
  await clicarOuExplicar(page, '#tlmDerrubar');
  const caiu = await Promise.race([a.fechou, new Promise(r => setTimeout(() => r(false), 8000))]);
  verdade(caiu, 'o Derrubar nao fechou o soquete do cliente');
  verdade(b.aberta() && c.aberta(), 'o Derrubar derrubou tambem uma conexao vizinha');

  // ------------------------------------------- encerrar a operacao em curso
  passo = 'encerrar a operacao em curso';
  const db = `batTlm${ctx.tema === 'claro' ? 'C' : 'E'}`;
  await api(page, 'criar_database', { database: db }).catch(() => {});
  extras.push(() => api(page, 'excluir_tabela', { database: db, tabela: 'carga', confirmar: 'carga' }));
  await api(page, 'criar_tabela', {
    database: db, tabela: 'carga',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }, { nome: 'nome', tipo: 'Str(40)' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  const N = 1600000;
  const partes = ['id;nome\n'];
  for (let i = 1; i <= N; i++) partes.push(`${i};Cliente numero ${i}\n`);
  const texto = partes.join('');

  // Sem operacao em curso o botao e o gemeo DESLIGADO: nao ha o que encerrar.
  await agora();
  await escolher(idc);
  await page.waitForFunction(() => !!document.querySelector('.tlm-acoes button.excluir[disabled]'), undefined, { timeout: ESPERA });
  verdade(!(await page.$('#tlmEncerrar')), 'com a conexao ociosa o Encerrar nao pode estar armado');

  c.enviar({ op: 'inserir_lote', database: db, tabela: 'carga', texto, formato: 'csv' });
  // O servidor leva 2-3 s so para LER os ~46 MB do pedido; a operacao so
  // existe depois. Com o relogio pausado, quem pergunta de novo e o caso.
  let armado = false;
  for (let i = 0; i < 20 && !armado; i++) {
    await agora();
    armado = !!(await page.$('#tlmEncerrar'));
  }
  verdade(armado, 'a carga nunca apareceu como operacao em curso com ponto de cancelamento');
  await capturar(ctx, ctx.nomeCaptura('encerrar-operacao'));
  const pedido = page.waitForRequest(r => /"op"\s*:\s*"telemetria_encerrar"/.test(r.postData() || ''), { timeout: 5000 });
  await clicarOuExplicar(page, '#tlmEncerrar');
  const enviado = await pedido;
  contem(enviado.postData(), idc, 'o pedido de encerrar devia nomear a atividade da bolha escolhida');
  await page.waitForFunction(() => /(encerrando|marcada|nao_cancelavel|não cancelável)/i.test(document.querySelector('#aviso')?.textContent || ''),
    undefined, { timeout: ESPERA });
  const aviso = await page.textContent('#aviso');
  ctx.notas.push(`o servidor respondeu ao Encerrar: «${aviso}»`);

  // A resposta da carga diz se a marca valeu: cancelada (ok=false) ou, se o
  // clique caiu na fase critica, terminada -- e nesse caso a tela TEM de ter
  // dito «nao_cancelavel». Dizer «encerrando» e deixar gravar tudo e mentira.
  const resposta = await c.proxima(60000);
  if (/nao_cancelavel|não cancelável/i.test(aviso)) {
    verdade(resposta.ok === true, `a tela disse «${aviso}» mas a carga nao terminou: ${JSON.stringify(resposta).slice(0, 200)}`);
  } else {
    verdade(resposta.ok === false, `a tela disse «${aviso}» mas a carga TERMINOU (${resposta.resultado && resposta.resultado.gravadas} linhas)`);
    verdade(!/gravadas/.test(JSON.stringify(resposta.resultado || {})), 'a carga cancelada nao devia devolver resultado de gravacao');
  }
  igual(c.aberta(), true, 'encerrar a OPERACAO nao pode derrubar a conexao (isso e o Derrubar)');
}

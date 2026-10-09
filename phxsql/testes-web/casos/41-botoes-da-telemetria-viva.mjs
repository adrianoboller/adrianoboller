/* Os botoes da telemetria que pedem CONEXAO VIVA do outro lado (pedido 190):
 *
 *   #tlmEstacao  [data-nivel]  #tlmDerrubar  #tlmEncerrar
 *
 * O caso 36 provou a barra do painel; estes quatro atuam sobre OUTRA pessoa e
 * por isso so existem com gente conectada. Aqui "gente" sao soquetes de
 * verdade na porta de dados (`conexaoViva`, apoio.mjs): o "Derrubar" e provado
 * pelo soquete cair VISTO DE FORA, e o "Encerrar a operacao" por uma carga de
 * 480 mil linhas que o servidor esta convertendo no momento do clique.
 *
 * Por que uma carga enorme e nao uma operacao "esperando trava": medido
 * (sonda de 02/10/2026) que escrita concorrente em outra tabela nao espera, a
 * leitura nao bloqueia, e a escrita contra tabela em transacao e RECUSADA na
 * hora (EM_TRANSACAO) -- nenhuma das tres deixa uma operacao viva por tempo
 * suficiente para clicar.
 *
 * PEDIDO 741: a carga cabe no teto do diario, e a espera e pelo EVENTO. Era de
 * 1,6 milhao de linhas, e desde o 686 a carga acima de 64 MiB no diario e
 * RECUSADA no fim da conversao (204,8 MB previstos): o ramo «nao_cancelavel»
 * nunca podia passar, e o «encerrando» passava por engano, porque a recusa
 * do teto tambem e `ok:false`. E a espera era de 20 voltas (~11 s) contra uma
 * carga que, com a maquina em load 8, so vira operacao depois de 60-90 s
 * lendo o pedido -- 10 em 10 corridas cairam assim. Agora: 480 mil linhas
 * (~61 MB previstos), a espera dura enquanto a carga nao respondeu (120 s so
 * de desistencia), e a resposta tem de ser o CANCELADO, nao um ok:false
 * qualquer.
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
  // 128 bytes previstos por linha desta tabela no diario: 480 mil ficam em
  // ~61 MB, abaixo dos 64 MiB do teto (`log::TETO_DA_TRANSACAO`).
  const N = 480000;
  const partes = ['id;nome\n'];
  for (let i = 1; i <= N; i++) partes.push(`${i};Cliente numero ${i}\n`);
  const texto = partes.join('');

  // Sem operacao em curso o botao e o gemeo DESLIGADO: nao ha o que encerrar.
  await agora();
  await escolher(idc);
  await page.waitForFunction(() => !!document.querySelector('.tlm-acoes button.excluir[disabled]'), undefined, { timeout: ESPERA });
  verdade(!(await page.$('#tlmEncerrar')), 'com a conexao ociosa o Encerrar nao pode estar armado');

  c.enviar({ op: 'inserir_lote', database: db, tabela: 'carga', texto, formato: 'csv' });
  // O servidor primeiro LE o pedido inteiro (~14 MB); a operacao so existe
  // depois, e quanto isso leva depende da maquina -- 2 s sozinha, mais de um
  // minuto em load 8. Por isso o relogio nao decide: decide o EVENTO. Ou o
  // Encerrar arma, ou a carga responde antes de armar -- e ai a operacao
  // existiu e a tela nunca a mostrou cancelavel, que e a falha que este
  // passo caca. O prazo e so de desistencia, para um servidor mudo.
  //
  // E o clique espera tambem o SERVIDOR dizer «cancelavel neste instante» (a
  // fase de conversao). Clicar so com o botao armado pegava a carga ainda na
  // fila, a resposta vinha «marcada», e «marcada» aceita os dois desfechos --
  // medido com o defeito reposto (a marca que nao se poe): o caso aprovava.
  // Na fase cancelavel a promessa e a forte, e o desfecho tem de ser um so.
  let armado = false;
  const t0 = Date.now();
  let voltas = 0;
  const cancelavelAgora = async () => {
    const tl = await api(page, 'telemetria');
    const at = (tl.atividades || []).find(x => x.id === idc);
    return !!(at && at.cancelavel);
  };
  while (!armado && c.pendentes() === 0 && Date.now() - t0 < 120000) {
    await agora();
    voltas++;
    armado = !!(await page.$('#tlmEncerrar')) && await cancelavelAgora();
  }
  if (!armado) {
    const gasto = Date.now() - t0, jaRespondeu = c.pendentes() > 0;
    const r = await c.proxima(120000).catch(() => null);
    throw new Falha(`a carga nunca apareceu como operacao em curso, cancelavel neste instante -- `
      + `${voltas} voltas em ${gasto} ms; a carga ${jaRespondeu ? 'JA tinha respondido' : 'ainda nao tinha respondido'}`
      + ` e respondeu ${r ? `ok=${r.ok}${r.ok ? '' : ` (${String(r.erro).slice(0, 120)})`}` : 'nada'} ${Date.now() - t0} ms depois do envio`);
  }
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
  //
  // O fim da carga se le pelo NOME do erro, e nao pelo `ok:false`: a recusa do
  // teto do diario tambem e `ok:false`, e foi assim que este passo aprovou
  // durante o 686 sem cancelar nada. «marcada» admite os dois desfechos (a
  // marca vale no proximo ponto seguro, se ainda houver um); qualquer outro
  // erro e defeito.
  const resposta = await c.proxima(120000);
  const fim = resposta.ok ? 'terminou' : resposta.nome;
  if (/nao_cancelavel|não cancelável/i.test(aviso)) {
    verdade(fim === 'terminou', `a tela disse «${aviso}» mas a carga nao terminou: ${JSON.stringify(resposta).slice(0, 200)}`);
  } else if (/marcada/i.test(aviso)) {
    verdade(fim === 'terminou' || fim === 'CANCELADO', `a tela disse «${aviso}» e a carga acabou em ${fim}: ${String(resposta.erro).slice(0, 160)}`);
  } else {
    verdade(fim === 'CANCELADO', `a tela disse «${aviso}» e a carga acabou em ${fim}, nao cancelada: ${String(resposta.erro || '').slice(0, 160)}`);
  }
  igual(c.aberta(), true, 'encerrar a OPERACAO nao pode derrubar a conexao (isso e o Derrubar)');
}

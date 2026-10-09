/* Os botoes da telemetria que pedem CONEXAO VIVA do outro lado (pedido 190):
 *
 *   #tlmEstacao  [data-nivel]  #tlmDerrubar  #tlmEncerrar
 *
 * O caso 36 provou a barra do painel; estes quatro atuam sobre OUTRA pessoa e
 * por isso so existem com gente conectada. Aqui "gente" sao soquetes de
 * verdade na porta de dados (`conexaoViva`, apoio.mjs): o "Derrubar" e provado
 * pelo soquete cair VISTO DE FORA, e o "Encerrar a operacao" por uma soma de
 * verificacao sobre 480 mil linhas, em curso no momento do clique (pedido 761:
 * a carga que enche a tabela era o alvo ate ali, e a fase cancelavel dela e
 * curta demais para o laco da tela -- ver o passo, la embaixo).
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

  // A carga ENCHE a tabela; a operacao viva e a soma de verificacao sobre
  // ela (pedido 761). Medido sob 4 nucleos ocupados, release, 3 corridas: a
  // carga de 480 mil linhas fica cancelavel so na CONVERSAO -- 118 a 253 ms
  // de 3,7 a 4,3 s; o resto e gravacao, que nao aceita marca. A volta deste
  // laco (Agora + 400 ms + a pergunta) e de ~500 ms, e por isso 3 de 5
  // corridas viam a carga responder `ok` sem nunca a pegar na fase. A soma
  // sobre as mesmas 480 mil linhas fica cancelavel 1,46 a 1,81 s, quase a
  // operacao inteira. Nao e o motor que mudou de ideia: e o teste que mirava
  // a janela curta.
  c.enviar({ op: 'inserir_lote', database: db, tabela: 'carga', texto, formato: 'csv' });
  // O servidor primeiro LE o pedido inteiro (~14 MB) -- 2 s sozinho, mais de
  // um minuto em load 8. O prazo e so de desistencia, para um servidor mudo.
  const carga = await c.proxima(180000);
  verdade(carga.ok, `a carga que enche a tabela da soma foi recusada: ${String(carga.erro || '').slice(0, 160)}`);

  // O clique espera o EVENTO e o SERVIDOR: o Encerrar armado na tela E a
  // atividade dizendo «cancelavel neste instante». Clicar so com o botao
  // armado pegava a operacao fora da fase, a resposta vinha «marcada», e
  // «marcada» aceita os dois desfechos -- medido com o defeito reposto (a
  // marca que nao se poe): o caso aprovava. Na fase cancelavel a promessa e
  // a forte, e o desfecho tem de ser um so.
  //
  // Ainda assim o clique pode chegar DEPOIS da fase: entre ver «cancelavel»
  // e o pedido sair ha a volta da tela e a captura. Ai o servidor responde
  // «marcada» (tem ponto, nao esta nele), a soma termina, e a tentativa nao
  // mediu a promessa forte -- tenta de novo, com uma soma nova. O que nao
  // afrouxa: toda resposta «encerrando» tem de acabar em CANCELADO, e ao
  // menos UMA tentativa tem de ser «encerrando».
  const cancelavelAgora = async () => {
    const tl = await api(page, 'telemetria');
    const at = (tl.atividades || []).find(x => x.id === idc);
    return !!(at && at.cancelavel);
  };
  const tentativas = [];
  let forte = false;
  for (let tentativa = 1; tentativa <= 5 && !forte; tentativa++) {
    c.enviar({ op: 'checksum', database: db, tabela: 'carga' });
    let armado = false;
    const t0 = Date.now();
    let voltas = 0;
    while (!armado && c.pendentes() === 0 && Date.now() - t0 < 120000) {
      await agora();
      voltas++;
      armado = !!(await page.$('#tlmEncerrar')) && await cancelavelAgora();
    }
    if (!armado) {
      const r = await c.proxima(120000).catch(() => null);
      tentativas.push(`#${tentativa}: nunca cancelavel na tela (${voltas} voltas, ${Date.now() - t0} ms, `
        + `soma ${r ? `ok=${r.ok}` : 'muda'})`);
      continue;
    }
    if (tentativa === 1) await capturar(ctx, ctx.nomeCaptura('encerrar-operacao'));
    const pedido = page.waitForRequest(r => /"op"\s*:\s*"telemetria_encerrar"/.test(r.postData() || ''), { timeout: 5000 });
    const volta = page.waitForResponse(r => /"op"\s*:\s*"telemetria_encerrar"/.test(r.request().postData() || ''), { timeout: ESPERA });
    // Se o pedido nunca sair, quem reprova e o `pedido` de cima; esta espera
    // orfa nao pode virar rejeicao sem dono e derrubar a bateria inteira.
    volta.catch(() => {});
    await clicarOuExplicar(page, '#tlmEncerrar');
    const enviado = await pedido;
    contem(enviado.postData(), idc, 'o pedido de encerrar devia nomear a atividade da bolha escolhida');
    const corpo = await (await volta).json();
    const prometido = (corpo.resultado || corpo).estado;
    verdade(prometido, `o Encerrar respondeu sem estado: ${JSON.stringify(corpo).slice(0, 200)}`);
    // A tela diz o que o servidor prometeu -- e e a promessa que se cobra.
    await page.waitForFunction(e => (document.querySelector('#aviso')?.textContent || '').startsWith(`${e}:`),
      prometido, { timeout: ESPERA });
    const aviso = await page.textContent('#aviso');

    // A resposta da soma diz se a marca valeu. O fim se le pelo NOME do erro,
    // e nao pelo `ok:false` (pedido 741): qualquer outro erro e defeito.
    const resposta = await c.proxima(120000);
    const fim = resposta.ok ? 'terminou' : resposta.nome;
    tentativas.push(`#${tentativa}: ${prometido} -> ${fim}`);
    if (prometido === 'encerrando') {
      verdade(fim === 'CANCELADO', `a tela disse «${aviso}» e a soma acabou em ${fim}, nao cancelada: ${String(resposta.erro || '').slice(0, 160)}`);
      ctx.notas.push(`o servidor respondeu ao Encerrar: «${aviso}»`);
      forte = true;
    } else if (prometido === 'marcada') {
      verdade(fim === 'terminou' || fim === 'CANCELADO', `a tela disse «${aviso}» e a soma acabou em ${fim}: ${String(resposta.erro).slice(0, 160)}`);
    } else if (prometido === 'nao_cancelavel') {
      // Dizer «nao_cancelavel» e deixar cancelar e a mentira do outro lado.
      verdade(fim === 'terminou', `a tela disse «${aviso}» mas a soma nao terminou: ${JSON.stringify(resposta).slice(0, 200)}`);
    } else {
      throw new Falha(`o Encerrar sobre uma soma em curso respondeu «${aviso}»`);
    }
  }
  verdade(forte, `nenhuma tentativa pegou a soma na fase cancelavel: ${tentativas.join('; ')}`);
  ctx.notas.push(`tentativas do Encerrar: ${tentativas.join('; ')}`);
  igual(c.aberta(), true, 'encerrar a OPERACAO nao pode derrubar a conexao (isso e o Derrubar)');
}

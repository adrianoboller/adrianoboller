/* A area de trabalho: abas vivas, regioes lado a lado e janelas soltas.
 *
 * O que este caso prova, em ordem de importancia:
 *
 *   1. O COMPORTAMENTO VELHO. Quem nunca clicar em nada disto abre o console
 *      com uma regiao, uma aba, um `#painel` e NADA gravado no navegador.
 *      Este e o teste que mais importa: guarda nova entra pedida, e um modo
 *      novo que muda a tela de quem nao pediu e estrago, nao recurso.
 *   2. ESTADO POR ABA. Duas tabelas em duas abas nao brigam pelo mesmo
 *      `est.atual` -- o defeito que so aparece com duas abertas.
 *   3. ABA ESCONDIDA PARA DE TRABALHAR, medido em pedidos por minuto, e nao
 *      afirmado. E fechar a aba solta o laco.
 *   4. TROCAR DE MODO NAO PERDE ESTADO: o mesmo no do DOM viaja de aba para
 *      janela solta e volta, com a rolagem reposta.
 *   5. O ARRASTO ANINHADO: dentro da janela solta, arrastar o CORPO nao
 *      arrasta a janela. E a armadilha do Diagrama ER, que arrasta tabelas.
 *   6. O PINO: o que foi pinado volta na abertura seguinte; o que nao foi,
 *      nao volta.
 *
 * A largura minima de uma regiao tambem sai daqui MEDIDA, e nao estimada. */
import { entrar, cenario, api, capturar, verdade, igual, assentar, bancoDoCaso }
  from '../apoio.mjs';

/** Conta os pedidos que a pagina faz a `/api` durante `ms`. */
async function pedidosEm(page, ms) {
  await page.evaluate(() => {
    if (!window.__contando) {
      const f = window.fetch;
      window.__n = 0;
      window.__contando = true;
      window.fetch = function (...a) {
        if (String(a[0]).includes('/api')) window.__n++;
        return f.apply(this, a);
      };
    }
    window.__n = 0;
  });
  await page.waitForTimeout(ms);
  return await page.evaluate(() => window.__n);
}

/** O contêiner que rola dentro da tela com foco, e a posicao dele. */
const ROLADOR = () => {
  const t = document.querySelector('#painel');
  const cands = [t, ...t.querySelectorAll('*')]
    .filter(e => e.scrollHeight > e.clientHeight + 30);
  return cands[0] || null;
};

export const caso = {
  nome: 'multitela',
  // Um tema so: nada aqui depende de cor, e rodar duas vezes dobraria dois
  // minutos de medicao de relogio por nada.
  temaUnico: 'escuro',

  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Multi');
    await cenario(page, db, 'clientes');
    await cenario(page, db, 'pedidos');
    // Mais linhas para haver o que rolar: a prova da rolagem preservada nao
    // vale nada num painel que cabe inteiro na tela.
    for (let i = 4; i < 90; i++) {
      await api(page, 'inserir', { database: db, tabela: 'clientes',
        valores: [i, `Cliente ${i}`, 'Blumenau', 'SC', '10.00', '2025-01-01', ''] })
        .catch(() => {});
    }

    // A arvore foi montada antes de o cenario existir. `false` para ela nao
    // terminar clicando no Painel e repintar por cima da conferencia abaixo.
    await page.evaluate(() => montarArvore(false));
    await page.waitForSelector(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);

    // ------------------------------------------------- 1. o comportamento VELHO
    const velho = await page.evaluate(() => ({
      regioes: document.querySelectorAll('#regioes .regiao').length,
      calhas: document.querySelectorAll('#regioes .calha').length,
      paineis: document.querySelectorAll('#painel').length,
      titulos: document.querySelectorAll('#titulo').length,
      soUma: document.querySelector('.tira').dataset.soUma,
      guardado: localStorage.getItem('phxsql-multitela'),
      janelas: document.querySelectorAll('.janela').length,
    }));
    igual(velho.regioes, 1, 'quem entra ve UMA regiao');
    igual(velho.calhas, 0, 'sem calha com uma regiao');
    igual(velho.paineis, 1, 'ha exatamente um #painel no documento');
    igual(velho.titulos, 1, 'ha exatamente um #titulo no documento');
    igual(velho.soUma, '1', 'a tira encolhe quando ha uma aba so');
    igual(velho.janelas, 0, 'nenhuma janela solta sem alguem pedir');
    verdade(velho.guardado === null,
      `abrir o console nao pode gravar arranjo nenhum — achei ${velho.guardado}`);

    // ------------------------------------------------- 2. estado POR ABA
    await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);
    await page.waitForSelector('.aba[data-aba="conteudo"]');
    await page.click('.aba[data-aba="conteudo"]');
    await assentar(page, 600);

    // Rola, e marca o no para provar depois que ele e o MESMO elemento.
    const rolou = await page.evaluate(rolador => {
      const f = new Function('return (' + rolador + ')()');
      const el = f();
      if (!el) return null;
      el.scrollTop = 400;
      document.querySelector('#painel').dataset.marca = 'aba-clientes';
      return el.scrollTop;
    }, ROLADOR.toString());
    verdade(rolou > 0, 'nao achei nada para rolar na grade — a prova ficaria vazia');

    await page.evaluate(() => PhxTelas.novaAba());
    await assentar(page, 400);
    await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="pedidos"]`);
    await assentar(page, 700);

    const duas = await page.evaluate(() => ({
      abas: document.querySelectorAll('.tira .tira-aba').length,
      atual: est.atual && est.atual.tab,
      paineis: document.querySelectorAll('#painel').length,
      // A aba escondida saiu do documento: e por isso que nao ha id repetido.
      telas: document.querySelectorAll('.corpo .tela').length,
    }));
    igual(duas.abas, 2, 'duas abas na tira');
    igual(duas.atual, 'pedidos', 'a aba nova manda no est.atual');
    igual(duas.paineis, 1, 'com duas abas, o #painel continua unico');
    igual(duas.telas, 1, 'a aba escondida sai do documento');

    await capturar(ctx, ctx.nomeCaptura('duas-abas'));

    // Volta para a primeira: o estado e a rolagem tem de voltar com ela.
    await page.click('.tira .tira-aba:nth-child(1)');
    await assentar(page, 400);
    const voltou = await page.evaluate(rolador => {
      const f = new Function('return (' + rolador + ')()');
      const el = f();
      return { atual: est.atual && est.atual.tab,
        marca: document.querySelector('#painel').dataset.marca,
        rolagem: el ? el.scrollTop : -1 };
    }, ROLADOR.toString());
    igual(voltou.atual, 'clientes', 'voltar a aba repoe o est.atual dela');
    igual(voltou.marca, 'aba-clientes', 'a aba voltou com o MESMO no do DOM');
    verdade(Math.abs(voltou.rolagem - rolou) < 20,
      `a rolagem nao voltou: esperava ${rolou}, achei ${voltou.rolagem}`);
    ctx.notas.push(`rolagem preservada na troca de aba: ${voltou.rolagem}px`);

    // -------------------------------- 3. aba escondida para de trabalhar (MEDIDO)
    //
    // A aba que TAPA a telemetria e a Consulta, e nao uma aba nova: a aba nova
    // abre o Painel, e o Painel tem relogio proprio. Medir contra ela mediria
    // um relogio contra o outro e daria empate -- foi o que aconteceu na
    // primeira versao deste caso, e o empate quase passou por «nao pausa».
    const abas = () => page.locator('.tira .tira-aba');
    await page.evaluate(() => PhxTelas.abrir('telemetria', {}, { nova: true }));
    await page.waitForTimeout(1800);
    const visivel = await pedidosEm(page, 8000);
    await page.evaluate(() => PhxTelas.abrir('query', {}, { nova: true }));
    await page.waitForTimeout(1800);
    const escondida = await pedidosEm(page, 8000);
    ctx.notas.push(`telemetria visivel: ${visivel} pedidos/8 s · escondida atras da Consulta: ${escondida}`);
    verdade(visivel >= 3,
      `a telemetria visivel nem pediu: ${visivel} em 8 s — a medicao estaria vazia`);
    verdade(escondida * 2 < visivel,
      `a aba escondida continuou trabalhando: ${escondida} contra ${visivel} pedidos em 8 s`);

    // 3b. Fechar a aba solta o laco. Volta para a telemetria, conta, fecha,
    //     conta de novo -- com a Consulta na frente nos dois lados.
    const iTelemetria = await page.evaluate(() =>
      [...document.querySelectorAll('.tira .tira-aba .rot')]
        .findIndex(e => /Telemetria/i.test(e.textContent)));
    verdade(iTelemetria >= 0, 'nao achei a aba da telemetria na tira');
    await abas().nth(iTelemetria).click();
    await page.waitForTimeout(1800);
    const antesDeFechar = await pedidosEm(page, 6000);
    await abas().nth(iTelemetria).locator('.tira-x').click();
    await page.waitForTimeout(1200);
    const depoisDeFechar = await pedidosEm(page, 6000);
    ctx.notas.push(`telemetria aberta: ${antesDeFechar} pedidos/6 s · fechada: ${depoisDeFechar}`);
    verdade(depoisDeFechar * 2 < antesDeFechar,
      `fechar a aba nao soltou o relogio: ${depoisDeFechar} contra ${antesDeFechar}`);

    // ------------------------------------------------------- 4. as regioes
    await page.evaluate(() => PhxTelas.dividir(2));
    await assentar(page, 900);
    const divididas = await page.evaluate(() => ({
      regioes: document.querySelectorAll('#regioes .regiao').length,
      calhas: document.querySelectorAll('#regioes .calha').length,
      // Duas regioes, duas telas visiveis -- e ainda assim UM `#painel`.
      telas: document.querySelectorAll('.corpo .tela').length,
      paineis: document.querySelectorAll('#painel').length,
    }));
    igual(divididas.regioes, 2, 'duas regioes lado a lado');
    igual(divididas.calhas, 1, 'uma calha entre as duas');
    igual(divididas.telas, 2, 'as duas regioes mostram uma tela cada');
    igual(divididas.paineis, 1,
      'com duas telas na tela, so a que tem foco carrega o id #painel');
    await capturar(ctx, ctx.nomeCaptura('duas-regioes'));

    // ------------------------- 4a. o id que o modulo NAO gerencia
    //
    // A afirmacao logo acima -- «so a que tem foco carrega o id #painel» --
    // vale para os ids que ESTE modulo move. Os que cada tela traz no proprio
    // HTML ninguem move, e `id="idiomasAqui"` esta em DUAS: a de Configuracoes
    // e a de Idiomas. Numa regiao so elas nunca convivem (o `folha()` troca o
    // `innerHTML` do painel e a aba escondida sai do documento), e por isso o
    // id repetido atravessou o projeto inteiro sem sintoma. Com a tela
    // DIVIDIDA, que e para o que este modulo existe, as duas ficam anexadas:
    // `querySelector` devolvia a primeira e a segunda abria com o seletor de
    // idioma VAZIO -- medido em 04/09/2026, seis bandeiras contra ZERO.
    const idiomaAntes = await page.evaluate(() => idiomaEscolhido());
    await page.locator('#regioes .regiao').nth(0).locator('.tira-aba').first().click();
    await assentar(page, 300);
    await page.evaluate(() => verConfigServidor());
    await assentar(page, 1500);
    await page.locator('#regioes .regiao').nth(1).locator('.tira-aba').first().click();
    await assentar(page, 300);
    await page.evaluate(() => irPara('idiomas'));
    await assentar(page, 2000);
    // Quantas bandeiras SAO -- a lista sai do codigo, nao do dedo.
    const quantosIdiomas = await page.evaluate(() => IDIOMAS.length);
    const desenhadas = await page.evaluate(() =>
      [...document.querySelectorAll('.idi-aqui')].map(n => n.querySelectorAll('.idi').length));
    igual(desenhadas.join(','), `${quantosIdiomas},${quantosIdiomas}`,
      'as duas telas com seletor de idioma abertas lado a lado desenham as bandeiras cada uma');
    ctx.notas.push(`seletor de idioma nas duas regioes: ${desenhadas.join(' e ')} bandeiras`);
    // E clicar numa acende a MESMA nas duas -- que e o que o codigo ja
    // prometia por comentario e so agora cumpre com as duas na tela.
    const outro = await page.evaluate(() => IDIOMAS.find(i => i.col !== idiomaEscolhido()).col);
    await page.locator('.idi-aqui').nth(1).locator(`.idi[data-idi="${outro}"]`).click();
    await assentar(page, 900);
    const acesas = await page.evaluate(() => [...document.querySelectorAll('.idi-aqui')].map(n =>
      [...n.querySelectorAll('.idi')].filter(b => b.getAttribute('aria-checked') === 'true')
        .map(b => b.dataset.idi).join(',') || '(nenhuma)'));
    igual(acesas.join(' | '), `${outro} | ${outro}`,
      'clicar a bandeira de uma regiao tem de acender a mesma nas duas');
    // Devolve o idioma de antes: o resto do caso nao pediu para ser traduzido.
    await page.evaluate(col => escolherIdioma(col), idiomaAntes);
    await assentar(page, 700);

    // A calha arrasta.
    const antesDaCalha = await page.evaluate(() =>
      document.querySelectorAll('.regiao')[0].getBoundingClientRect().width);
    const calha = await page.locator('#regioes .calha').boundingBox();
    await page.mouse.move(calha.x + 3, calha.y + calha.height / 2);
    await page.mouse.down();
    await page.mouse.move(calha.x + 203, calha.y + calha.height / 2, { steps: 8 });
    await page.mouse.up();
    await assentar(page, 300);
    const depoisDaCalha = await page.evaluate(() =>
      document.querySelectorAll('.regiao')[0].getBoundingClientRect().width);
    verdade(depoisDaCalha > antesDaCalha + 100,
      `a calha nao arrastou: ${antesDaCalha} → ${depoisDaCalha}`);
    ctx.notas.push(`calha: regiao 1 de ${Math.round(antesDaCalha)} para ${Math.round(depoisDaCalha)}px`);

    // Arrastar uma aba da regiao 1 para a regiao 2.
    const antesDoArrasto = await page.evaluate(() =>
      [...document.querySelectorAll('.regiao')].map(r =>
        r.querySelectorAll('.tira-aba').length));
    // Por `locator`, e nao por `nth-of-type`: regioes e calhas sao irmas
    // dentro de `#regioes`, e `nth-of-type` conta por TAG, nao por classe.
    await page.locator('#regioes .regiao').nth(0).locator('.tira-aba').first()
      .dragTo(page.locator('#regioes .regiao').nth(1).locator('.tira'));
    await assentar(page, 500);
    const depoisDoArrasto = await page.evaluate(() =>
      [...document.querySelectorAll('.regiao')].map(r =>
        r.querySelectorAll('.tira-aba').length));
    verdade(depoisDoArrasto[1] > antesDoArrasto[1],
      `a aba nao mudou de regiao: ${antesDoArrasto} → ${depoisDoArrasto}`);
    ctx.notas.push(`abas por regiao: ${antesDoArrasto} → ${depoisDoArrasto}`);

    // ----------------------------- 4b. as QUATRO telas nomeadas, todas visiveis
    //
    // O caso do dono: navegador esticado por cima dos monitores, Diagrama ER,
    // Telemetria, Profiler e Consulta lado a lado. Aqui NINGUEM esta escondido,
    // entao a regra do "aba escondida pausa" nao salva -- o custo e real e o
    // numero vai para o documento em vez de ser escondido.
    await page.setViewportSize({ width: 3240, height: 950 });
    await assentar(page, 400);
    const cabem = await page.evaluate(() => PhxTelas.maxRegioes());
    verdade(cabem >= 4, `com 3240px tinham de caber 4 regioes, cabem ${cabem}`);
    await page.evaluate(() => PhxTelas.dividir(4));
    await assentar(page, 1200);
    await page.evaluate(async () => {
      const regs = PhxTelas._W.regioes;
      await PhxTelas.abrir('diagrama', {}, { regiao: regs[0] });
      await PhxTelas.abrir('telemetria', {}, { regiao: regs[1] });
      await PhxTelas.abrir('profiler', {}, { regiao: regs[2] });
      await PhxTelas.abrir('query', {}, { regiao: regs[3] });
    });
    await page.waitForTimeout(2500);
    const quatro = await page.evaluate(() => ({
      regioes: document.querySelectorAll('#regioes .regiao').length,
      telas: document.querySelectorAll('.corpo .tela').length,
      paineis: document.querySelectorAll('#painel').length,
      rotulos: [...document.querySelectorAll('.tira .tira-aba.sel .rot')]
        .map(e => e.textContent.trim()),
    }));
    igual(quatro.regioes, 4, 'quatro regioes lado a lado');
    igual(quatro.telas, 4, 'as quatro telas ficam VIVAS ao mesmo tempo');
    igual(quatro.paineis, 1, 'e mesmo com quatro na tela, o #painel continua unico');
    await capturar(ctx, ctx.nomeCaptura('quatro-regioes'));
    const comQuatro = await pedidosEm(page, 10000);
    ctx.notas.push(`quatro telas visiveis (${quatro.rotulos.join(', ')}):`
      + ` ${comQuatro} pedidos/10 s`);

    await page.evaluate(() => PhxTelas.dividir(1));
    await page.setViewportSize({ width: 1600, height: 950 });
    await assentar(page, 800);
    // Uma tela so, para o numero de cima ter contra o que se comparar.
    await page.evaluate(() => PhxTelas.abrir('query', {}));
    await page.waitForTimeout(1500);
    const comUma = await pedidosEm(page, 10000);
    ctx.notas.push(`uma tela (Consulta) sozinha: ${comUma} pedidos/10 s`);
    verdade(comQuatro > comUma,
      'quatro telas vivas tinham de custar mais que uma parada — a medicao nao mediu nada');

    await page.evaluate(() => PhxTelas.dividir(1));
    await assentar(page, 500);

    // ------------------------------------------------- 5. a janela solta
    await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);
    await page.waitForSelector('.aba[data-aba="conteudo"]');
    await page.click('.aba[data-aba="conteudo"]');
    await assentar(page, 700);
    const rolagemAntes = await page.evaluate(rolador => {
      const el = new Function('return (' + rolador + ')()')();
      if (el) el.scrollTop = 350;
      document.querySelector('#painel').dataset.marca = 'viajante';
      return el ? el.scrollTop : -1;
    }, ROLADOR.toString());

    await page.evaluate(() => PhxTelas.soltar());
    await page.waitForSelector('.janela');
    await assentar(page, 400);
    const solta = await page.evaluate(rolador => {
      const el = new Function('return (' + rolador + ')()')();
      return {
        janelas: document.querySelectorAll('.janela').length,
        dentro: !!document.querySelector('.janela .tela #painel'),
        marca: document.querySelector('#painel').dataset.marca,
        rolagem: el ? el.scrollTop : -1,
        atual: est.atual && est.atual.tab,
      };
    }, ROLADOR.toString());
    igual(solta.janelas, 1, 'uma janela solta');
    verdade(solta.dentro, 'a tela foi para dentro da janela');
    igual(solta.marca, 'viajante', 'a janela recebeu o MESMO no, e nao um HTML refeito');
    igual(solta.atual, 'clientes', 'a tela solta continua sabendo qual tabela mostra');
    verdade(Math.abs(solta.rolagem - rolagemAntes) < 60,
      `a rolagem se perdeu ao soltar: ${rolagemAntes} → ${solta.rolagem}`);
    ctx.notas.push(`rolagem preservada ao soltar: ${rolagemAntes} → ${solta.rolagem}`);
    await capturar(ctx, ctx.nomeCaptura('janela-solta'));

    // 5b. Arrastar pelo CABECALHO move; arrastar pelo CORPO nao.
    //     Esta e a armadilha do Diagrama ER, que arrasta tabelas por conta:
    //     se o corpo movesse a janela, arrastar uma tabela do diagrama
    //     arrastaria a janela junto.
    const pos = () => page.evaluate(() => {
      const j = document.querySelector('.janela');
      return { x: Math.round(parseFloat(j.style.left)), y: Math.round(parseFloat(j.style.top)) };
    });
    const p0 = await pos();
    const topo = await page.locator('.janela .jan-topo').boundingBox();
    await page.mouse.move(topo.x + topo.width / 2, topo.y + topo.height / 2);
    await page.mouse.down();
    await page.mouse.move(topo.x + topo.width / 2 + 90, topo.y + topo.height / 2 + 60, { steps: 8 });
    await page.mouse.up();
    await assentar(page, 250);
    const p1 = await pos();
    verdade(Math.abs(p1.x - p0.x) > 50,
      `a janela nao andou pelo cabecalho: ${JSON.stringify(p0)} → ${JSON.stringify(p1)}`);

    const corpo = await page.locator('.janela .jan-corpo').boundingBox();
    await page.mouse.move(corpo.x + corpo.width / 2, corpo.y + corpo.height - 24);
    await page.mouse.down();
    await page.mouse.move(corpo.x + corpo.width / 2 + 120, corpo.y + corpo.height - 24 + 80,
      { steps: 8 });
    await page.mouse.up();
    await assentar(page, 250);
    const p2 = await pos();
    igual(JSON.stringify(p2), JSON.stringify(p1),
      'arrastar DENTRO da janela moveu a janela — e a armadilha do arrasto aninhado');

    // 5c. Redimensionar pelo canto.
    const tam0 = await page.evaluate(() => {
      const j = document.querySelector('.janela').getBoundingClientRect();
      return { w: Math.round(j.width), h: Math.round(j.height) };
    });
    const canto = await page.locator('.janela .jan-canto').boundingBox();
    await page.mouse.move(canto.x + 8, canto.y + 8);
    await page.mouse.down();
    await page.mouse.move(canto.x + 8 + 120, canto.y + 8 + 70, { steps: 8 });
    await page.mouse.up();
    await assentar(page, 250);
    const tam1 = await page.evaluate(() => {
      const j = document.querySelector('.janela').getBoundingClientRect();
      return { w: Math.round(j.width), h: Math.round(j.height) };
    });
    verdade(tam1.w > tam0.w + 60 && tam1.h > tam0.h + 30,
      `o canto nao redimensionou: ${JSON.stringify(tam0)} → ${JSON.stringify(tam1)}`);

    // 5d. Acoplar de volta, com o estado inteiro.
    await page.click('.janela [data-jan="acoplar"]');
    await assentar(page, 500);
    const acoplada = await page.evaluate(rolador => {
      const el = new Function('return (' + rolador + ')()')();
      return { janelas: document.querySelectorAll('.janela').length,
        marca: document.querySelector('#painel').dataset.marca,
        rolagem: el ? el.scrollTop : -1,
        atual: est.atual && est.atual.tab };
    }, ROLADOR.toString());
    igual(acoplada.janelas, 0, 'a janela sumiu ao acoplar');
    igual(acoplada.marca, 'viajante', 'a tela voltou com o MESMO no do DOM');
    igual(acoplada.atual, 'clientes', 'e continua sabendo qual tabela mostra');
    verdade(Math.abs(acoplada.rolagem - rolagemAntes) < 60,
      `a rolagem se perdeu ao acoplar: ${rolagemAntes} → ${acoplada.rolagem}`);

    // ------------------------------------------------------------ 6. o pino
    await page.evaluate(() => PhxTelas.abrir('query', {}));
    await assentar(page, 400);
    const guardadoSemPino = await page.evaluate(() =>
      localStorage.getItem('phxsql-multitela'));
    await page.evaluate(() => {
      const abas = [...document.querySelectorAll('.tira .tira-aba')];
      abas[abas.length - 1].querySelector('.tira-pino').click();
    });
    await assentar(page, 300);
    const guardadoComPino = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('phxsql-multitela') || '{}'));
    const pinadas = (guardadoComPino.regioes || [])
      .reduce((a, r) => a.concat(r.abas || []), []);
    verdade(pinadas.some(a => a.chave === 'query'),
      `o pino nao guardou a Query: ${JSON.stringify(guardadoComPino)}`);
    ctx.notas.push(`sem pino o arranjo guardado era ${guardadoSemPino ? 'gravado' : 'nada'}`);

    // Recarrega: a pinada volta, e nada mais.
    await page.reload({ waitUntil: 'domcontentloaded' });
    await entrar(page, ctx.url);
    await assentar(page, 1500);
    const depoisDoReload = await page.evaluate(() =>
      [...document.querySelectorAll('.tira .tira-aba .rot')].map(e => e.textContent.trim()));
    verdade(depoisDoReload.some(r => /Consulta|Query/i.test(r)),
      `a aba pinada nao voltou: ${JSON.stringify(depoisDoReload)}`);
    verdade(depoisDoReload.length <= 3,
      `voltou aba que ninguem pinou: ${JSON.stringify(depoisDoReload)}`);
    ctx.notas.push(`depois do recarregar: ${JSON.stringify(depoisDoReload)}`);

    await capturar(ctx, ctx.nomeCaptura('depois-do-pino'));
  },
};

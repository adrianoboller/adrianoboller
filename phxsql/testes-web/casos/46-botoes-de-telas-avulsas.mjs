/* Os botoes que sobravam, tela por tela (pedido 190):
 *
 *   entrada      #btConexoes  #btGuardarConex
 *   barra        #btAjuda
 *   juncao       [data-t]  #jRodar  #jUniao   ·   uniao  #unRodar  #unJuncao
 *   consulta     #btConsultar
 *   sequencias   [data-t]
 *   mensagens    #btSemear  #btAbrirGrade
 *   jobs         #btJobVer
 *   replicacao   #acFim
 *   multitela    #mtAlinhar
 *   grade        [data-jl]
 *
 * Cada um e conferido pelo EFEITO: a juncao pelas CONTAGENS que cada formato
 * de Venn tem de dar (3 / 4 / 4 / 5 / 1 / 1 / 2 sobre dados montados para
 * isso), o guardar conexao pelo que NAO vai para o disco do navegador (nem a
 * senha nem o token), a sequencia pelo contador no servidor.
 *
 * Duas dispensas honestas deste caso estao em `conferidor_botoes.rs`:
 * `#btAcompRep` (so nasce com uma origem de replicacao -- ele e clicado em
 * `religar-na-tela.mjs`) e o DbLink (precisa de um MySQL/MariaDB de verdade).
 * `#acFim` e exercitado abrindo o dialogo pela funcao que o monta, porque o
 * botao que o abre e o dispensado.
 *
 * A Window Management API e DUBLADA aqui, como em `monitores`: o navegador
 * sem cabeca nao concede a permissao. O que se prova e o handler do botao.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso, cenario, clicarOuExplicar, assentar, Falha, CREDENCIAL } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

const DUBLE = `(function () {
  const telas = [
    { label: "Esquerdo", left: 0, top: 0, width: 2560, height: 1440, availLeft: 0, availTop: 0,
      availWidth: 2560, availHeight: 1400, devicePixelRatio: 1, isPrimary: true },
    { label: "Direito", left: 2560, top: 0, width: 2560, height: 1440, availLeft: 2560, availTop: 0,
      availWidth: 2560, availHeight: 1400, devicePixelRatio: 1, isPrimary: false },
  ];
  Object.defineProperty(window, "getScreenDetails", { configurable: true, writable: true,
    value: () => Promise.resolve({ screens: telas, currentScreen: telas[0] }) });
})()`;

export const caso = {
  nome: 'botoes-de-telas-avulsas',
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
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });
  const aviso = () => page.textContent('#aviso');

  // ================================================== a tela de entrada
  passo = 'entrada: conexoes salvas';
  await page.goto(ctx.url, { waitUntil: 'domcontentloaded' });
  await esperar('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, undefined, { timeout: 15000 });
  const aberta = () => page.getAttribute('#btConexoes', 'aria-expanded');
  igual(await aberta(), 'false', 'as conexoes salvas nascem fechadas (a entrada e de entrar)');
  verdade(await page.$eval('#conexCorpo', e => e.hidden), 'o corpo das conexoes devia estar escondido');
  await clicarOuExplicar(page, '#btConexoes');
  igual(await aberta(), 'true', 'aria-expanded depois de abrir');
  verdade(!(await page.$eval('#conexCorpo', e => e.hidden)), 'o corpo devia aparecer');
  await clicarOuExplicar(page, '#btConexoes');
  igual(await aberta(), 'false', 'aria-expanded depois de fechar');
  await clicarOuExplicar(page, '#btConexoes');

  await page.fill('#u', CREDENCIAL.USUARIO);
  await page.fill('#s', CREDENCIAL.SENHA);
  await page.fill('#t', CREDENCIAL.TOKEN);
  // Recusar o nome: nada e guardado.
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, '#btGuardarConex');
  await page.waitForTimeout(300);
  verdade(!(await page.textContent('#conexLista')).includes('Base da Farm'), 'recusar o apelido nao pode guardar a conexao');
  verdade(!(await page.evaluate(() => JSON.stringify({ ...localStorage }))).includes('Base da Farm'), 'recusar o apelido gravou no navegador');
  page.once('dialog', d => d.accept('Base da Farmácia'));
  await clicarOuExplicar(page, '#btGuardarConex');
  await page.waitForFunction(() => /Base da Farmácia/.test(document.querySelector('#conexLista')?.textContent || ''),
    undefined, { timeout: ESPERA });
  // Rotulo se estiliza; o NOME QUE A PESSOA DEU e dado: sai como digitado.
  contem(await page.textContent('#conexLista'), 'Base da Farmácia', 'o apelido saiu em outra caixa');
  await capturar(ctx, ctx.nomeCaptura('conexoes-salvas'));
  const guardado = await page.evaluate(() => JSON.stringify({ ...localStorage }));
  verdade(guardado.includes('Base da Farm'), 'a conexao devia estar no armazenamento do navegador');
  verdade(!guardado.includes(CREDENCIAL.SENHA), 'A SENHA FOI GUARDADA no armazenamento do navegador');
  verdade(!guardado.includes(CREDENCIAL.TOKEN), 'O TOKEN FOI GUARDADO no armazenamento do navegador');

  await entrar(page, ctx.url);

  // ============================================================ ajuda
  passo = 'barra: o botao de ajuda';
  const versao = (await api(page, 'ping')).phxsql;
  verdade(/\d+\.\d+/.test(versao), `o ping nao trouxe a versao: ${versao}`);
  await clicarOuExplicar(page, '#btAjuda');
  await page.waitForFunction(() => /Built to store/.test(document.querySelector('#subtitulo')?.textContent || ''),
    undefined, { timeout: ESPERA });
  contem(await page.textContent('#painel .fichas'), versao, 'a ajuda devia dizer a versao que o servidor diz');
  await capturar(ctx, ctx.nomeCaptura('sobre'));

  // ========================================================= juncao e uniao
  passo = 'juncao: os sete formatos';
  const db = bancoDoCaso(ctx, 'Jun');
  await cenario(page, db, 'clientes');
  await cenario(page, db, 'clientes_b');
  // A tem o 5, B tem o 4 (e nenhum dos dois tem o do outro): 1,2,3 casam.
  await api(page, 'inserir', { database: db, tabela: 'clientes', valores: [5, 'So em A', 'Gaspar', 'SC', '1.00', '2025-01-01', ''] });
  await api(page, 'inserir', { database: db, tabela: 'clientes_b', valores: [4, 'So em B', 'Indaial', 'SC', '1.00', '2025-01-01', ''] });
  await page.evaluate(() => montarArvore(false));
  await page.evaluate(d => telaJuncao(d), db);
  await esperar('#jRodar');
  await esperar('#vennes .venn[data-t="interna"]');
  const esperado = { interna: 3, esquerda: 4, direita: 4, completa: 5, so_esquerda: 1, so_direita: 1, so_dos_lados: 2 };
  const sqls = await page.evaluate(() => JUNCOES.map(j => [j.id, j.sql]));
  for (const [tipo, quantas] of Object.entries(esperado)) {
    await page.evaluate(() => { document.querySelector('#jSaida').innerHTML = ''; });
    await clicarOuExplicar(page, `#vennes .venn[data-t="${tipo}"]`);
    await page.waitForFunction(() => /linha\(s\)/.test(document.querySelector('#jSaida .dbl-titulo')?.textContent || ''),
      undefined, { timeout: ESPERA });
    const n = Number((await page.textContent('#jSaida .dbl-titulo b')).replace(/\D/g, ''));
    igual(n, quantas, `linhas da juncao «${tipo}»`);
    verdade(await page.$(`#vennes .venn.viva[data-t="${tipo}"]`), `o desenho de «${tipo}» devia ficar aceso`);
    contem(await page.textContent('#jSaida .dbl-titulo code'), sqls.find(s => s[0] === tipo)[1].split(' ')[0],
      `o SQL mostrado de «${tipo}»`);
  }
  await capturar(ctx, ctx.nomeCaptura('juncao'));
  // «Juntar» roda de novo o formato aceso.
  await page.evaluate(() => { document.querySelector('#jSaida').innerHTML = ''; });
  await clicarOuExplicar(page, '#jRodar');
  await page.waitForFunction(() => /2\s+linha/.test(document.querySelector('#jSaida .dbl-titulo')?.textContent || ''),
    undefined, { timeout: ESPERA });

  passo = 'uniao: marcar, unir e voltar';
  await clicarOuExplicar(page, '#jUniao');
  await esperar('#unRodar');
  await clicarOuExplicar(page, '#unRodar');                           // sem marcar: recusa
  await page.waitForFunction(() => !!document.querySelector('#aviso')?.textContent, undefined, { timeout: ESPERA });
  igual(await page.$$eval('#unSaida > *', xs => xs.length), 0, 'sem duas tabelas marcadas nao pode haver resultado');
  for (const t of ['clientes', 'clientes_b']) await page.check(`.un-item input[value="${t}"]`);
  await clicarOuExplicar(page, '#unRodar');
  await page.waitForFunction(() => /linha\(s\)/.test(document.querySelector('#unSaida .dbl-titulo')?.textContent || ''),
    undefined, { timeout: ESPERA });
  igual(Number((await page.textContent('#unSaida .dbl-titulo b')).replace(/\D/g, '')), 5, 'UNION tira as repetidas: 4 + 4 − 3');
  await page.selectOption('#unModo', 'tudo');
  await page.evaluate(() => { document.querySelector('#unSaida').innerHTML = ''; });
  await clicarOuExplicar(page, '#unRodar');
  await page.waitForFunction(() => /linha\(s\)/.test(document.querySelector('#unSaida .dbl-titulo')?.textContent || ''),
    undefined, { timeout: ESPERA });
  igual(Number((await page.textContent('#unSaida .dbl-titulo b')).replace(/\D/g, '')), 8, 'UNION ALL mantem tudo: 4 + 4');
  await capturar(ctx, ctx.nomeCaptura('uniao'));
  await clicarOuExplicar(page, '#unJuncao');
  await esperar('#jRodar');
  await some('#unRodar');

  // ============================================================ consulta
  passo = 'consulta na memoria';
  await api(page, 'memoria_carregar', { database: db, tabela: 'clientes' });
  desfazer.push(() => api(page, 'memoria_liberar', { database: db, tabela: 'clientes' }));
  await page.evaluate(() => abrirConsulta());
  await esperar('#btConsultar');
  await page.fill('#cDb', db);
  await page.fill('#cTab', 'clientes');
  await page.fill('#cCol', 'cidade');
  await page.fill('#cVal', 'Blumenau');
  await clicarOuExplicar(page, '#btConsultar');
  await esperar('#gradeConsulta .phx-grid');
  const achado = await page.textContent('#gradeConsulta');
  contem(achado, 'Blumenau', 'a consulta devia achar Blumenau, na caixa em que esta gravado');
  verdade(!achado.includes('Joinville'), 'a consulta trouxe uma linha que nao casa');
  contem(await page.textContent('#saidaConsulta'), '1 achadas', 'a consulta devia contar uma linha achada');

  // ========================================================== sequencias
  passo = 'sequencias: ajustar o contador';
  await api(page, 'criar_tabela', {
    database: db, tabela: 'fichas',
    colunas: [{ nome: 'id', tipo: 'Sequence', obrigatoria: true }, { nome: 'nome', tipo: 'Str(20)' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  const proxima = async () => ((await api(page, 'sequencias', { database: db })).sequencias.find(s => s.tabela === 'fichas') || {}).proxima;
  await page.evaluate(d => verSequencias(d), db);
  await esperar('#gradeSeq .bt-seq[data-t="fichas"]');
  const antes = await proxima();
  page.once('dialog', d => d.dismiss());
  await clicarOuExplicar(page, '#gradeSeq .bt-seq[data-t="fichas"]');
  await page.waitForTimeout(400);
  igual(await proxima(), antes, 'cancelar o prompt nao pode mexer no contador');
  page.once('dialog', d => d.accept('50'));
  await clicarOuExplicar(page, '#gradeSeq .bt-seq[data-t="fichas"]');
  await page.waitForFunction(() => /50/.test(document.querySelector('#gradeSeq')?.textContent || ''), undefined, { timeout: ESPERA });
  igual(await proxima(), 50, 'o contador no servidor depois de ajustar');
  await capturar(ctx, ctx.nomeCaptura('sequencias'));

  // =========================================================== mensagens
  passo = 'mensagens: semear e abrir a grade';
  await page.evaluate(() => telaMensagens());
  await esperar('#btSemear');
  await clicarOuExplicar(page, '#btSemear');
  await page.waitForFunction(() => /mensagens/i.test(document.querySelector('#titulo')?.textContent || '')
    && !!document.querySelector('#painel .phx-grid'), undefined, { timeout: ESPERA });
  verdade((await api(page, 'mensagens', {})).existe === true, 'Semear devia ter criado a tabela de mensagens');
  await page.evaluate(() => telaMensagens());
  await esperar('#btAbrirGrade');
  await clicarOuExplicar(page, '#btAbrirGrade');
  await page.waitForFunction(() => !!document.querySelector('#painel .phx-grid') && !document.querySelector('#btAbrirGrade'),
    undefined, { timeout: ESPERA });

  // ================================================================ jobs
  passo = 'jobs: atualizar a lista';
  await page.evaluate(() => telaJobs());
  await esperar('#btJobVer');
  const job = `${bancoDoCaso(ctx, 'job').toLowerCase()}_atualizar`;
  await api(page, 'job_salvar', { job: { nome: job, descricao: 'caso 46', usuario: CREDENCIAL.USUARIO, ligado: false, cada_minutos: 90, pedido: { op: 'ping' } } });
  desfazer.push(() => api(page, 'job_excluir', { nome: job }));
  verdade(!(await page.textContent('#painel')).includes(job), 'preparo: o job criado por fora ainda nao devia estar na tela');
  await clicarOuExplicar(page, '#btJobVer');
  await page.waitForFunction(j => (document.querySelector('#painel')?.textContent || '').includes(j), job, { timeout: ESPERA });

  // ===================================================== acompanhar replica
  passo = 'replicacao: fechar o dialogo de acompanhar';
  await page.evaluate(() => abrirProvaReplicacao([]));
  await esperar('#acFim');
  await clicarOuExplicar(page, '#acFim');
  await some('#acFim');
  verdade(!(await page.$('.sobre')), 'Fechar devia tirar o dialogo da pagina');

  // ================================================================ multitela
  passo = 'multitela: alinhar as regioes com os monitores';
  await page.evaluate(DUBLE);
  await page.evaluate(() => PhxTelas.dividir(2));
  await assentar(page, 500);
  await page.evaluate(() => {
    Object.defineProperty(window, 'screenX', { configurable: true, value: 0 });
    Object.defineProperty(window, 'outerWidth', { configurable: true, value: 5120 });
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 5120 });
    document.querySelector('#regioes').getBoundingClientRect = () => ({ left: 0, top: 0, width: 5120, height: 800, right: 5120, bottom: 800 });
  });
  await page.evaluate(() => PhxTelas.telaAjuda());
  await esperar('#mtAlinhar');
  await clicarOuExplicar(page, '#mtAlinhar');
  await page.waitForFunction(() => /alinhadas/.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: ESPERA });
  await page.evaluate(() => PhxTelas.dividir(1));

  // ============================================================== a grade
  passo = 'grade: a celula JSON expande e fecha';
  const dados = [{ id: 1, doc: { cidade: 'Blumenau', itens: [1, 2] } }, { id: 2, doc: { cidade: 'Itajaí' } }];
  await page.evaluate(d => {
    const alvo = document.createElement('div');
    alvo.id = 'provaJson';
    document.querySelector('#painel').appendChild(alvo);
    PhxGrid.criar('#provaJson', {
      colunas: [{ campo: 'id', titulo: 'id', tipo: 'numero' }, { campo: 'doc', titulo: 'doc', tipo: 'json' }],
      dados: d,
    });
  }, dados);
  await esperar('#provaJson .phx-json-btn[data-jl]');
  await clicarOuExplicar(page, '#provaJson .phx-json-btn[data-jl="0"]');
  await page.waitForFunction(() => {
    const p = document.querySelector('#provaJson .phx-popover');
    return p && !p.hidden && /Blumenau/.test(p.textContent);
  }, undefined, { timeout: ESPERA });
  const pop = await page.textContent('#provaJson .phx-popover');
  contem(pop, '"itens"', 'o popover devia trazer o JSON inteiro, indentado');
  contem(pop, 'Blumenau', 'o JSON mudou a caixa do dado');
  await clicarOuExplicar(page, '#provaJson .phx-json-btn[data-jl="0"]');          // o mesmo botao fecha
  await page.waitForFunction(() => document.querySelector('#provaJson .phx-popover')?.hidden,
    undefined, { timeout: ESPERA });
}

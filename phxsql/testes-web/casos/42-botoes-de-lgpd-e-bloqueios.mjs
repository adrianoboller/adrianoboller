/* Os botoes das telas de AUDITORIA e de SEGURANCA (pedido 190):
 *
 *   dado pessoal  #btLgVer  [data-db]  (nome da tabela e «quem mexeu»)
 *   trilha LGPD   #btLgTrRec  #btLgTrEstr
 *   bloqueios     #btExportar  #btSalvarWl  [data-ip]  (soltar)
 *
 * Cada botao e provado pelo EFEITO no servidor, e nao pela tela mexer:
 * «Varrer de novo» acha a coluna marcada que nasceu DEPOIS de a tela abrir;
 * «Reler» traz a alteracao feita por fora; «Salvar whitelist» muda o que o
 * protocolo devolve; «soltar» faz o IP voltar a falar -- visto de fora, do
 * mesmo 127.0.0.2 que o servidor tinha bloqueado.
 *
 * O IP bloqueado e REAL (`bloquearIp`, apoio.mjs): seis tokens errados de
 * 127.0.0.2. Nunca o 127.0.0.1 do navegador, senao o caso se tranca para fora.
 * O `finally` solta o IP e devolve a whitelist: bloqueio e politica de
 * SERVIDOR, e o proximo caso nao pode herdar nenhum dos dois.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso, clicarOuExplicar, bloquearIp, conexaoViva, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'botoes-de-lgpd-e-bloqueios',
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
  const db = bancoDoCaso(ctx, 'Lgd');
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });
  const fichas = async () => Object.fromEntries(await page.$$eval('#painel .fichas .ficha',
    ds => ds.map(d => [d.querySelector('.r').textContent.trim(), d.querySelector('.v').textContent.trim()])));

  // ------------------------------------------------------- dado pessoal
  passo = 'dado pessoal: varrer de novo acha a coluna nova';
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: db, tabela: 'pacientes',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'nome', tipo: 'Str(60)', dado_pessoal: 'pessoal' },
      { nome: 'cidade', tipo: 'Str(30)' },
    ],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  // `est.bancos` e a foto da arvore: o banco criado por fora so entra no
  // seletor «Alcance» depois de a arvore ser remontada, como faz a tela quando
  // a pessoa cria o banco por ela.
  await page.evaluate(() => montarArvore(false));
  const r1 = await api(page, 'inserir', { database: db, tabela: 'pacientes', valores: [1, 'Zelia Prado', 'Blumenau'] });
  await page.evaluate(d => telaDadosPessoais(d), db);
  await esperar('#btLgVer');
  igual((await fichas())['colunas marcadas'], '1', 'colunas marcadas antes da tabela nova');
  await capturar(ctx, ctx.nomeCaptura('dado-pessoal'));

  // A tabela nasce DEPOIS de a tela abrir: so uma varredura nova a ve.
  await api(page, 'criar_tabela', {
    database: db, tabela: 'contatos',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'email', tipo: 'Str(60)', dado_pessoal: 'sensivel' },
    ],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  // (Nao se mexe no seletor «Alcance»: o `onchange` dele ja varre sozinho, e
  // um caso que o tocasse aprovaria um «Varrer de novo» morto -- foi o que a
  // prova real achou na primeira versao.)
  igual(await page.inputValue('#lgDb'), db, 'a tela devia abrir com o alcance do banco pedido');
  await clicarOuExplicar(page, '#btLgVer');
  await page.waitForFunction(() => [...document.querySelectorAll('#painel .fichas .ficha')]
    .some(d => /marcadas/.test(d.querySelector('.r').textContent) && d.querySelector('.v').textContent.trim() === '2'),
    undefined, { timeout: ESPERA });
  contem(await page.textContent('#gradeLgpd'), 'email', 'a varredura nova nao trouxe a coluna que nasceu depois');
  contem(await page.textContent('#gradeLgpd'), db, 'a varredura perdeu o alcance escolhido');

  passo = 'dado pessoal: o nome da tabela leva a Estrutura, sem atropelar a tela seguinte';
  // A pessoa clica o nome da tabela e, ANTES de a Estrutura terminar de abrir,
  // pede outra tela. Para a corrida ser ordem fixa e nao sorteio, a resposta do
  // `esquema` que a Estrutura espera fica SEGURA no fio ate a outra tela estar
  // pintada (o mesmo molde do caso 38): so entao ela e solta. O
  // `.then(() => irAba(...))` antigo tomava o painel DE NOVO quando a tabela
  // terminava de abrir, e a Estrutura cobria a tela pedida depois.
  let soltar; const segurada = new Promise(r => { soltar = r; });
  let chegou; const chegouUm = new Promise(r => { chegou = r; });
  let ja = false;
  await page.route('**/api', async rota => {
    if (!ja && /"op"\s*:\s*"esquema"/.test(rota.request().postData() || '')) {
      ja = true; chegou(); await segurada;
    }
    await rota.continue();
  });
  await clicarOuExplicar(page, '#gradeLgpd .bt-lg[data-tab="contatos"]');
  await chegouUm;                                   // a Estrutura esta esperando o servidor
  await page.evaluate(d => telaDadosPessoais(d), db);
  await page.waitForFunction(() => !!document.querySelector('#gradeLgpd'), undefined, { timeout: ESPERA });
  soltar();                                         // agora a Estrutura atrasada responde
  await page.waitForTimeout(1500);
  await page.unroute('**/api');
  verdade(await page.$('#gradeLgpd'), 'a Estrutura aberta pelo clique atropelou a tela de dado pessoal pedida depois');
  igual(await page.textContent('#titulo'), 'Dado pessoal', 'o titulo devia continuar o da tela pedida por ultimo');
  // E sem a segunda tela, o nome da tabela LEVA a Estrutura de verdade.
  await clicarOuExplicar(page, '#gradeLgpd .bt-lg[data-tab="contatos"]');
  await page.waitForFunction(() => document.querySelector('#titulo')?.textContent === 'contatos', undefined, { timeout: ESPERA });
  await page.waitForFunction(() => /email/.test(document.querySelector('#painel')?.textContent || ''), undefined, { timeout: ESPERA });
  await page.evaluate(d => telaDadosPessoais(d), db);
  await esperar('#gradeLgpd .trilha-lg');

  // ------------------------------------------------------ trilha LGPD
  passo = 'trilha: quem mexeu, reler e ver a estrutura';
  await api(page, 'atualizar', { database: db, tabela: 'pacientes', rowid: r1.rowid, valores: { id: 1, nome: 'Zelia P. Prado', cidade: 'Blumenau' } })
    .catch(async () => api(page, 'atualizar', { database: db, tabela: 'pacientes', rowid: r1.rowid, valores: { nome: 'Zelia P. Prado' } }));
  await clicarOuExplicar(page, `#gradeLgpd .trilha-lg[data-tab="pacientes"]`);
  await esperar('#btLgTrRec');
  const total0 = Number((await fichas())['registros na trilha']);
  await capturar(ctx, ctx.nomeCaptura('trilha-lgpd'));
  contem(await page.textContent('#painel'), 'nome', 'a trilha nao lista a coluna marcada');
  // Mais uma alteracao por fora: so o «Reler» a traz.
  await api(page, 'atualizar', { database: db, tabela: 'pacientes', rowid: r1.rowid, valores: { id: 1, nome: 'Zelia Prado Lima', cidade: 'Blumenau' } })
    .catch(async () => api(page, 'atualizar', { database: db, tabela: 'pacientes', rowid: r1.rowid, valores: { nome: 'Zelia Prado Lima' } }));
  await clicarOuExplicar(page, '#btLgTrRec');
  await page.waitForFunction(n => {
    const f = [...document.querySelectorAll('#painel .fichas .ficha')]
      .find(d => /registros na trilha/.test(d.querySelector('.r').textContent));
    return f && Number(f.querySelector('.v').textContent.replace(/\D/g, '')) > n;
  }, total0, { timeout: ESPERA });
  await clicarOuExplicar(page, '#btLgTrEstr');
  await page.waitForFunction(() => document.querySelector('#titulo')?.textContent === 'pacientes', undefined, { timeout: ESPERA });
  await some('#btLgTrRec');

  // ------------------------------------------------------- bloqueios
  passo = 'bloqueios: preparo de um IP realmente bloqueado';
  const antes = await api(page, 'bloqueios', {});
  desfazer.push(() => api(page, 'whitelist_salvar', { whitelist: antes.whitelist || [] }));
  desfazer.push(() => api(page, 'desbloquear', { ip: '127.0.0.2' }));
  await bloquearIp(ctx, '127.0.0.2');

  await page.evaluate(() => abrirAdmin('bloqueios'));
  await esperar('#btExportar');
  await esperar('#gradeAdmBloqueios [data-ip="127.0.0.2"]');
  await capturar(ctx, ctx.nomeCaptura('bloqueios'));

  passo = 'bloqueios: exportar para o firewall';
  await page.selectOption('#fmtExport', 'iptables');
  verdade(await page.$eval('#saidaExport', e => e.hidden), 'a saida do firewall nao devia aparecer antes de Gerar');
  await clicarOuExplicar(page, '#btExportar');
  await page.waitForFunction(() => !document.querySelector('#saidaExport').hidden, undefined, { timeout: ESPERA });
  const saida = await page.textContent('#saidaExport');
  contem(saida, '127.0.0.2', 'o firewall exportado nao traz o IP bloqueado');
  contem(saida, 'iptables', 'o formato pedido nao chegou ao texto');
  await capturar(ctx, ctx.nomeCaptura('bloqueios-exportados'));

  passo = 'bloqueios: salvar a whitelist';
  await page.fill('#wlEditavel', '203.0.113.0/24\n198.51.100.7');
  // O campo que o caso preencheu ja casa o texto: esperar por ele passava na
  // hora, antes de o `whitelist_salvar` voltar, e o `bloqueios` abaixo lia a
  // whitelist velha (pedido 782, 1 queda em 3 corridas inteiras). Espera-se
  // o campo NOVO, que o `abrirAdmin` repinta so depois de o servidor guardar.
  await page.$eval('#wlEditavel', e => { e.dataset.velho = '1'; });
  await clicarOuExplicar(page, '#btSalvarWl');
  await page.waitForFunction(() => {
    const e = document.querySelector('#wlEditavel');
    return !!e && !e.dataset.velho && /203\.0\.113\.0\/24/.test(e.value);
  }, undefined, { timeout: ESPERA });
  const depois = await api(page, 'bloqueios', {});
  igual(JSON.stringify(depois.whitelist), JSON.stringify(['203.0.113.0/24', '198.51.100.7']),
    'a whitelist que o servidor guardou');
  // Uma regra ilegivel e RECUSADA, e a tela diz (protecao que nao protege nao entra).
  await page.fill('#wlEditavel', 'isto-nao-e-ip');
  await page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.textContent = ''; });
  await clicarOuExplicar(page, '#btSalvarWl');
  await page.waitForFunction(() => !!document.querySelector('#aviso')?.textContent, undefined, { timeout: ESPERA });
  igual(JSON.stringify((await api(page, 'bloqueios', {})).whitelist), JSON.stringify(['203.0.113.0/24', '198.51.100.7']),
    'a recusa de uma regra invalida nao pode mexer na whitelist guardada');
  await api(page, 'whitelist_salvar', { whitelist: antes.whitelist || [] });
  await page.evaluate(() => abrirAdmin('bloqueios'));
  await esperar('#gradeAdmBloqueios [data-ip="127.0.0.2"]');

  passo = 'bloqueios: soltar o IP';
  await clicarOuExplicar(page, '#gradeAdmBloqueios [data-ip="127.0.0.2"]');
  await page.waitForFunction(() => !document.querySelector('#gradeAdmBloqueios'), undefined, { timeout: ESPERA });
  verdade(!(await api(page, 'bloqueios', {})).ativos.some(x => x.ip === '127.0.0.2'),
    'soltar nao tirou o IP da lista do servidor');
  // E visto de fora: o mesmo IP volta a ser atendido.
  const volta = await conexaoViva(ctx, { de: '127.0.0.2' });
  volta.derrubar();
}

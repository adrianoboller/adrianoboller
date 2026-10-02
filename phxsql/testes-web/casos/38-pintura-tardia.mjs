/* O DIAGRAMA ER QUE CHEGA DEPOIS NAO PODE COBRIR A TELA QUE A PESSOA JA ABRIU.
 *
 * Pedido 636, familia do 170. O «Criar» do cartao de nova tabela termina com
 * `await montarArvore(); telaDiagramaER(db)`, e `telaDiagramaER` pintava sem
 * perguntar se a tela ainda era a que pediu. A bateria inteira reprovou duas
 * vezes por isso (o caso 34 isolado nunca), e o 34 passou a ESPERAR
 * `ER.esquemas` ter a tabela -- o que escondia o defeito sem consertar nada.
 *
 * Como o caso 18, este nao torce por timing: SEGURA a resposta de uma op no
 * fio ate a segunda tela estar pintada e so entao solta. A corrida vira ordem
 * fixa, que e o unico jeito de um caso provar uma corrida sem virar ele
 * proprio intermitente.
 *
 * Tres cenas, uma por lugar onde o diagrama pintava atrasado:
 *
 *   1. «Criar tabela» no cartao -- seguro `criar_tabela`; o repintar que vem
 *      DEPOIS de `montarArvore()` e o do pedido.
 *   2. «Redesenhar» -- seguro `tabelas`; o diagrama pintava o corpo final
 *      depois do laco de `esquema`, por cima de qualquer coisa.
 *   3. As irmas de duas fases (`irmas` abaixo): tela que anuncia «carregando»,
 *      espera o servidor e pinta o corpo -- cada uma com a op que ela espera.
 *
 * O veredito e sobre o PAR, como no caso 18: titulo e corpo da mesma tela, e o
 * diagrama ausente (`#btErNova`, `.er-rolo`, «lendo o esquema») onde a pessoa
 * esta olhando outra coisa. A medida e DEPOIS de o atropelo ter tido chance de
 * acontecer: uma conferencia feita antes do dano passa por engano.
 */
import { entrar, api, verdade, igual, bancoDoCaso, capturar } from '../apoio.mjs';

/** Segura a PRIMEIRA chamada da op `op` ate `soltar()`. Devolve {soltar, segurou}. */
async function segurar(page, op) {
  await page.unroute('**/api').catch(() => {});
  let soltar;
  const preso = new Promise(r => { soltar = r; });
  const estado = { segurou: false, soltar: () => soltar() };
  await page.route('**/api', async rota => {
    let corpo = {};
    try { corpo = JSON.parse(rota.request().postData() || '{}'); } catch { /* nao e JSON */ }
    if (!estado.segurou && corpo.op === op) { estado.segurou = true; await preso; }
    await rota.continue();
  });
  return estado;
}

/** O que esta na tela, em fatos: de quem e o titulo, e o diagrama aparece? */
const vista = page => page.evaluate(() => ({
  titulo: (document.querySelector('#titulo') || {}).textContent,
  telemetria: !!document.querySelector('#tlmThreads'),
  diagrama: !!document.querySelector('#btErNova, .er-rolo, #erDesenho'),
  lendo: /lendo o esquema/.test((document.querySelector('#painel') || {}).textContent || ''),
}));

async function veredito(ctx, quem, rotulo) {
  const { page } = ctx;
  // A captura vem ANTES da conferencia: com o defeito reposto e ela que mostra
  // o diagrama cobrindo a telemetria, e depois do `throw` nao haveria foto.
  await capturar(ctx, ctx.nomeCaptura(rotulo));
  const d = await vista(page);
  igual(d.titulo, 'Telemetria', `${quem}: o titulo deixou de ser o da tela que a pessoa abriu depois`);
  verdade(d.telemetria && !d.diagrama && !d.lendo,
    `${quem}: a pintura atrasada cobriu a tela aberta depois dela `
    + `(telemetria=${d.telemetria}, diagrama=${d.diagrama}, lendo=${d.lendo}, titulo=${JSON.stringify(d.titulo)})`);
}

/** A pessoa abre outra tela enquanto o fio esta preso. */
async function abrirTelemetria(page) {
  await page.evaluate(() => { telaTelemetria(); });
  await page.waitForSelector('#tlmThreads', { state: 'attached', timeout: 15000 });
}

export const caso = {
  nome: 'pintura-tardia',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'ptd');
    await entrar(page, ctx.url);
    await api(page, 'criar_database', { database: db }).catch(() => {});
    await page.evaluate(d => { est.rascunho = null; return telaDiagramaER(d); }, db);
    await page.waitForSelector('#btErNova', { timeout: 15000 });

    // ------------------------------------------- 1. o «Criar» do cartao
    await page.click('#btErNova');
    await page.waitForSelector('#ntNome', { timeout: 15000 });
    await page.fill('#ntNome', 'TardiaCartao');
    const preso1 = await segurar(page, 'criar_tabela');
    await page.click('#ntCriar');
    // O «Criar» esta no ar, esperando o servidor. A pessoa abre outra tela.
    await abrirTelemetria(page);
    preso1.soltar();
    // A tabela TEM de nascer (o «Criar» foi pedido), e so entao se mede.
    const nasceu = async () => (await api(page, 'tabelas', { database: db })).tabelas.includes('TardiaCartao');
    for (let i = 0; i < 100 && !(await nasceu()); i++) await page.waitForTimeout(100);
    verdade(await nasceu(), 'o «Criar» do cartao devia ter criado a tabela mesmo com a pessoa em outra tela');
    // Depois do `criar_tabela` vem `montarArvore()` e so entao o repintar: da
    // tempo de sobra para o dano acontecer antes de se medir.
    await page.waitForTimeout(1500);
    await veredito(ctx, 'Criar do cartao', 'depois-do-criar');

    // ------------------------------------------------ 2. o «Redesenhar»
    await page.evaluate(d => telaDiagramaER(d), db);
    await page.waitForSelector('#btErVer', { timeout: 15000 });
    const preso2 = await segurar(page, 'tabelas');
    await page.evaluate(() => { document.querySelector('#btErVer').click(); });
    await page.waitForFunction(() => /lendo o esquema/.test(document.querySelector('#painel').textContent));
    await abrirTelemetria(page);
    preso2.soltar();
    await page.waitForTimeout(1500);
    await veredito(ctx, 'Redesenhar', 'depois-do-redesenhar');

    // ---------------------------------------------- 3. as irmas de duas fases
    for (const irma of IRMAS) {
      const preso = await segurar(page, irma.op);
      await page.evaluate(c => { window.__irma = eval(c); }, irma.chama.replace('{db}', JSON.stringify(db)));
      for (let i = 0; i < 100 && !preso.segurou; i++) await page.waitForTimeout(50);
      verdade(preso.segurou, `${irma.nome}: a op ${irma.op} nunca saiu -- a irma mudou de forma?`);
      await abrirTelemetria(page);
      preso.soltar();
      await page.evaluate(() => window.__irma).catch(() => {});
      await page.waitForTimeout(600);
      const d = await vista(page);
      igual(d.titulo, 'Telemetria', `${irma.nome}: o titulo deixou de ser o da tela aberta depois`);
      verdade(d.telemetria, `${irma.nome}: a tela atrasada escreveu o corpo por cima da telemetria`);
    }
    await page.unroute('**/api').catch(() => {});
  },
};

/** As irmas do diagrama: telas que pintam o corpo DEPOIS de um `await`. `op` e
 *  a op que a tela espera (e que o caso segura); `chama` a abre. Cobrem as tres
 *  formas que a guarda tem no fonte: a de duas fases (anuncia «carregando» e
 *  confere a posse que `folha()` devolveu), a de uma fase (toma a posse no
 *  inicio) e a que escreve direto no `#painel`, sem `folha()`. */
const IRMAS = [
  { nome: 'verSequencias (duas fases)', op: 'sequencias', chama: 'verSequencias({db})' },
  { nome: 'verSessoes (duas fases, com catch)', op: 'sessoes', chama: 'verSessoes()' },
  { nome: 'telaDbLink (duas fases, quatro pinturas)', op: 'dblink', chama: 'telaDbLink()' },
  { nome: 'verConteudoEditavel (a grade, escreve direto)', op: 'varrer', chama: 'verConteudoEditavel({db}, "TardiaCartao")' },
  { nome: 'verQuemSou (uma fase)', op: 'quem_sou', chama: 'verQuemSou()' },
  { nome: 'telaMensagens (uma fase, escreve direto)', op: 'mensagens', chama: 'telaMensagens()' },
];

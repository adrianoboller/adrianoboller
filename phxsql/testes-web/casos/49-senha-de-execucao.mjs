/* A senha de execucao pela TELA (pedido 767, P14), contra o servidor real:
 *
 *   #btTrancar  #seCadastrar  #seNao  #seSim
 *
 * O roteiro que o dono pediu, por um administrador que ainda NAO tem senha
 * de execucao (um por tema, criado aqui):
 *
 *   1. tenta um DROP pela gestao da tabela -> bloqueado: o dialogo da PAGINA
 *      abre (nunca `prompt()`), e «Cancelar» deixa a tabela de pe;
 *   2. tenta de novo e tenta cadastrar a segunda senha no dialogo com a de
 *      login -> RECUSADO (so o primeiro administrador do servidor se
 *      cadastra sozinho, brecha do 767); o supervisor liberado cadastra a
 *      dele, e a senha no mesmo dialogo libera -> o DROP executa;
 *   3. «Trancar» -> o DROP seguinte volta a ser bloqueado;
 *   4. informa a senha no dialogo -> executa de novo.
 *
 * Cada passo confere o EFEITO no servidor (a tabela existe ou nao, pelo
 * protocolo), e nao o que a tela mostrou. E o id da sessao tem de mudar ao
 * liberar -- o servidor o gira. */
import { entrar, api, verdade, igual, capturar, bancoDoCaso, clicarOuExplicar, CREDENCIAL, Falha } from '../apoio.mjs';
import { definirIA, abrirQuery } from '../claude-apoio.mjs';

const CHAVE = 'sk-ant-teste-FABRICADA-49E7A1B2C3D4';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'senha-de-execucao',
  async rodar(ctx) {
    try { await corpo(ctx); } catch (e) {
      // O recado do dialogo e o aviso da tela dizem POR QUE parou.
      const tela = [];
      for (const pg of ctx.page.context().pages()) {
        tela.push(await pg.evaluate(() => ({
          recado: document.querySelector('#seRecado')?.textContent,
          dialogo: !!document.querySelector('#dlgSenhaExecucao'),
          titulo: document.querySelector('#titulo')?.textContent,
          aviso: document.querySelector('#aviso')?.textContent,
        })).catch(() => ({})));
      }
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- ${JSON.stringify(tela)}`);
    }
  },
};

async function corpo(ctx) {
  const db = bancoDoCaso(ctx, 'Exec');
  const t = ctx.tema === 'claro' ? 'c' : 'e';
  const login = `execadm${t}`;
  const senhaLogin = 'provaDaBateria123';
  const senhaExec = `execucao-da-bateria-${t}-77`;

  // O supervisor da bateria (ja liberado pelo `entrar`) monta o cenario e o
  // administrador novo, SEM senha de execucao.
  passo = 'cenario';
  const adm = ctx.page;
  await entrar(adm, ctx.url);
  await api(adm, 'criar_database', { database: db }).catch(() => {});
  await api(adm, 'usuario_excluir', { login }).catch(() => {});
  await api(adm, 'usuario_criar', { login, senha: senhaLogin, nome: 'Adm da prova 767', supervisor: true });

  const page = await adm.context().newPage();
  try {
    await entrar(page, ctx.url, { usuario: login, senha: senhaLogin, token: CREDENCIAL.TOKEN }, { liberar: false });
    const criar = tab => api(page, 'criar_tabela', {
      database: db, tabela: tab,
      colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    });
    /** Pergunta ao servidor ate a condicao valer, ou ate o prazo. */
    const ate = async (cond, oQue) => {
      for (let i = 0; i < 80; i++) {
        if (await cond()) return;
        await page.waitForTimeout(250);
      }
      throw new Falha(oQue);
    };
    const existe = async tab => {
      const r = await adm.evaluate(([d]) => api('tabelas', { database: d }), [db]);
      const lista = (r.tabelas || r || []).map(x => (typeof x === 'string' ? x : x.nome));
      return lista.includes(tab);
    };
    const abrirExcluir = async tab => {
      await page.evaluate(([d, x]) => gerirTabela(d, x), [db, tab]);
      await page.waitForSelector('#btVoltarTabs', { timeout: ESPERA });
      const idx = await page.evaluate(() => {
        const ops = [...document.querySelectorAll('#painel .ops .op')];
        const i = ops.findIndex(b => b.classList.contains('perigo'));
        return i < 0 ? null : ops[i].getAttribute('data-op');
      });
      verdade(idx !== null, 'a gestao nao tem a operacao de excluir');
      // O nome digitado e o `prompt()` antigo do excluir -- nao e o da senha.
      let promptVisto = false;
      page.once('dialog', d => { promptVisto = true; d.accept(tab); });
      await clicarOuExplicar(page, `#painel .ops .op[data-op="${idx}"]`);
      try {
        await page.waitForSelector('#dlgSenhaExecucao', { timeout: ESPERA });
      } catch (e) {
        const tela = await page.evaluate(() => ({
          titulo: document.querySelector('#titulo')?.textContent,
          aviso: document.querySelector('#aviso')?.textContent,
        })).catch(() => ({}));
        throw new Falha(`o dialogo da senha nao abriu (prompt do nome visto: ${promptVisto}) -- ${JSON.stringify(tela)}`);
      }
    };

    // 1. bloqueado, e cancelar deixa a tabela de pe
    passo = '1. DROP bloqueado';
    await criar('alvo').catch(() => {});
    await abrirExcluir('alvo');
    await capturar(ctx, ctx.nomeCaptura('dialogo-senha-de-execucao'));
    await clicarOuExplicar(page, '#seNao');
    await page.waitForSelector('#dlgSenhaExecucao', { state: 'detached', timeout: ESPERA });
    verdade(await existe('alvo'), 'cancelar o dialogo apagou a tabela');

    // 2. o cadastro pelo proprio, no dialogo, e RECUSADO: este administrador
    //    nao e o primeiro do servidor, e so tem a senha de login (a brecha do
    //    767, fechada em 10/10/2026). O supervisor liberado cadastra a dele;
    //    a senha no MESMO dialogo libera, e o DROP executa.
    passo = '2. cadastro recusado, o administrador cadastra, a senha libera';
    const idAntes = await page.evaluate(() => est.sessao);
    await abrirExcluir('alvo');
    await clicarOuExplicar(page, '#seCadastrar');
    await page.fill('#seLogin', senhaLogin);
    await page.fill('#seNova', senhaExec);
    await page.fill('#seConfirma', senhaExec);
    await capturar(ctx, ctx.nomeCaptura('dialogo-cadastro'));
    await clicarOuExplicar(page, '#seSim');
    await page.waitForFunction(() => /administrador/i
      .test(document.querySelector('#seRecado')?.textContent || ''), undefined, { timeout: ESPERA });
    verdade(await existe('alvo'), 'o cadastro recusado apagou a tabela');
    await api(adm, 'senha_execucao_definir', { login, nova_senha_execucao: senhaExec });
    await clicarOuExplicar(page, '#seCadastrar');
    await page.fill('#seSenha', senhaExec);
    await clicarOuExplicar(page, '#seSim');
    await page.waitForSelector('#dlgSenhaExecucao', { state: 'detached', timeout: ESPERA });
    await ate(async () => !(await existe('alvo')), 'liberada a sessao, o DROP nao executou');
    const idDepois = await page.evaluate(() => est.sessao);
    verdade(idDepois && idDepois !== idAntes, 'o id da sessao nao girou ao liberar');
    const senhaNaPagina = await page.evaluate(s => document.documentElement.outerHTML.includes(s)
      || JSON.stringify(est).includes(s) || JSON.stringify(localStorage).includes(s), senhaExec);
    verdade(!senhaNaPagina, 'a senha de execucao sobrou na pagina');

    // 3. trancar: o DROP volta a ser bloqueado
    passo = '3. trancar';
    await criar('alvo2');
    await clicarOuExplicar(page, '#btTrancar');
    await page.waitForFunction(() => /trancada|locked|verrouill|bloccat|gesperrt|bloquead/i
      .test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: ESPERA });
    await abrirExcluir('alvo2');
    verdade(await existe('alvo2'), 'trancada a sessao, o DROP executou antes do dialogo');

    // 4. a senha no dialogo libera de novo
    passo = '4. liberar com a senha';
    await page.fill('#seSenha', senhaExec);
    await clicarOuExplicar(page, '#seSim');
    await page.waitForSelector('#dlgSenhaExecucao', { state: 'detached', timeout: ESPERA });
    await ate(async () => !(await existe('alvo2')), 'a senha no dialogo nao liberou o DROP');
    verdade(await page.isVisible('#btTrancar'), 'liberada a sessao, o «Trancar» nao apareceu');

    // 5. o SEGUNDO ponto de chamada, em outro arquivo: o «Executar» do editor
    //    SQL da Claude (claude.js) passa pela MESMA `api()`. Trancada a
    //    sessao, o DROP VIEW pelo SQL abre o mesmo dialogo e, com a senha,
    //    executa. Nada sobe para a Anthropic: o Executar fala so com o motor.
    passo = '5. o sql da Claude pelo mesmo funil';
    await api(page, 'criar_visao', { database: db, nome: 'v_exec', sql: 'SELECT id FROM alvo3' })
      .catch(() => {});
    await criar('alvo3').catch(() => {});
    await api(page, 'criar_visao', { database: db, nome: 'v_exec', sql: 'SELECT id FROM alvo3' })
      .catch(() => {});
    const visaoExiste = async () => {
      const r = await adm.evaluate(([d]) => api('visoes', { database: d }), [db]);
      return JSON.stringify(r).includes('v_exec');
    };
    verdade(await visaoExiste(), 'o cenario nao criou a visao');
    await clicarOuExplicar(page, '#btTrancar');
    await definirIA(page, { chave: CHAVE, ligado: true });
    await abrirQuery(page);
    // `:visible` em todo seletor: a multitela pode ter outra tela de Query
    // pinada (escondida) com os MESMOS ids, restaurada do `localStorage` que
    // os casos anteriores deixaram neste contexto.
    const vis = sel => page.locator(`${sel}:visible`).first();
    await vis('#btIA').click();
    await vis('.ia-rec[data-r="sql"]').click();
    await vis('#iaSql').waitFor({ timeout: ESPERA });
    await vis('#iaDb').fill(db);
    await vis('#iaSql').fill('DROP VIEW v_exec');
    await vis('#iaExecutar').click();
    await page.waitForSelector('#dlgSenhaExecucao', { timeout: ESPERA });
    verdade(await visaoExiste(), 'o DROP VIEW executou antes da senha');
    await page.fill('#seSenha', senhaExec);
    await clicarOuExplicar(page, '#seSim');
    await page.waitForSelector('#dlgSenhaExecucao', { state: 'detached', timeout: ESPERA });
    await ate(async () => !(await visaoExiste()), 'a senha no dialogo nao liberou o DROP VIEW da Claude');
    await capturar(ctx, ctx.nomeCaptura('claude-sql-liberado'));
    igual(await page.$('#dlgSenhaExecucao'), null, 'o dialogo ficou aberto');
  } catch (e) {
    // A aba fica aberta ate o diagnostico do `rodar` ler o recado dela.
    const tela = await page.evaluate(() => ({
      dialogos: document.querySelectorAll('#dlgSenhaExecucao').length,
      campos: [...document.querySelectorAll('#dlgSenhaExecucao input')].map(i => `${i.id}:${i.value.length}`),
      recado: document.querySelector('#seRecado')?.textContent,
      dialogo: !!document.querySelector('#dlgSenhaExecucao'),
      titulo: document.querySelector('#titulo')?.textContent,
      aviso: document.querySelector('#aviso')?.textContent,
    })).catch(() => ({}));
    await page.close().catch(() => {});
    throw new Falha(`${String(e.message).split('\n')[0]} -- aba do adm novo: ${JSON.stringify(tela)}`);
  }
  await page.close().catch(() => {});
}

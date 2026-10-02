/* Os botoes da GESTAO DE TABELA e de BANCO que so navegam ou so confirmam
 * (pedido 190): "← Gestao de X", "Estrutura completa", importar, copiar e
 * colar, motivos, lixeira, exportar, soma de verificacao, SysTables/SysColumns.
 *
 *   #btVoltarTabs  [data-op]  #btVoltarGer  #btVerEstr
 *   #btPrever  #btImportar  #btVoltaImp
 *   #btIrTabelas  #btColarAgora  #btLimparCopia
 *   #btVoltaMot  #btVerLixo  [data-uuid]  #btVoltaLinha
 *   #btVoltaExp  #btVoltaCk
 *   #btSysTabs  #btSysCols  #btTodas
 *   #btGerirDb  #btGerirBancoLista  #btRestaurarFicha
 *
 * Botao de VOLTAR e o que mais passa por engano: clicar e ver "uma tela
 * apareceu" nao prova nada, porque a tela de partida tambem e uma tela. Cada
 * passo confere que o botao de PARTIDA SUMIU e que o de CHEGADA existe -- e,
 * onde ha efeito de dado (importar, colar, restaurar), o servidor e perguntado
 * pelo protocolo e nao pela tela.
 *
 * O dado de prova leva "Blumenau" de proposito: a importacao passa por um
 * campo de texto e uma grade, e a caixa-alta do CSS global e a mentira de
 * sempre.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso, cenario, clicarOuExplicar, CREDENCIAL, Falha } from '../apoio.mjs';

const ESPERA = 20000;

export const caso = {
  nome: 'botoes-da-gestao-de-tabela',
  async rodar(ctx) {
    try { await corpo(ctx); } catch (e) {
      // Diz EM QUE PASSO caiu: «timeout esperando #x» sozinho obriga a reler
      // o caso inteiro para achar qual das vinte telas era.
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        aviso: document.querySelector('#aviso')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    }
  },
};

let passo = 'inicio';
async function corpo(ctx) {
  {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'Gst');
    const tab = 'clientes';
    await entrar(page, ctx.url);
    await cenario(page, db, tab);

    const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
    const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });
    const titulo = () => page.textContent('#titulo');
    const linhas = async () => ((await api(page, 'varrer', { database: db, tabela: tab, limite: 100 })).linhas || []);
    const abrirGestao = async () => {
      await page.evaluate(([d, t]) => gerirTabela(d, t), [db, tab]);
      await esperar('#btVoltarTabs');
    };
    /** Clica uma operacao da gestao pela POSICAO do `data-op`, lida da tela. */
    const operacao = async nome => {
      const idx = await page.evaluate(n => {
        const ops = [...document.querySelectorAll('#painel .ops .op')];
        const i = ops.findIndex(b => b.querySelector('.op-txt b').textContent.includes(n));
        return i < 0 ? null : ops[i].getAttribute('data-op');
      }, nome);
      verdade(idx !== null, `a gestao nao tem a operacao «${nome}»`);
      await clicarOuExplicar(page, `#painel .ops .op[data-op="${idx}"]`);
    };

    // gestao -> particoes -> volta

    passo = 'gestao -> particoes -> volta';
    await abrirGestao();
    await capturar(ctx, ctx.nomeCaptura('gestao-da-tabela'));
    await operacao('Partições');
    await esperar('#btVoltarGer');
    await some('#btVoltarTabs');
    contem(await titulo(), 'clientes', 'a tela de particoes nao diz de que tabela e');
    await capturar(ctx, ctx.nomeCaptura('particoes'));
    await clicarOuExplicar(page, '#btVoltarGer');
    await esperar('#btVoltarTabs');
    await some('#btVoltarGer');

    // configuracoes -> estrutura completa -> volta

    // A tela de particoes TEM DUAS FORMAS (tabela em arquivo unico e tabela
    // paginada) e as duas carregam o mesmo `#btVoltarGer`: a chave e uma so, e
    // a evidencia por chave nao distingue os dois sitios. Aqui os dois.
    passo = 'particoes de uma tabela paginada -> volta';
    await api(page, 'criar_tabela', {
      database: db, tabela: 'paginada', registros_por_arquivo: 10,
      colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    });
    await page.evaluate(([d, t]) => gerirTabela(d, t), [db, 'paginada']);
    await esperar('#btVoltarTabs');
    await operacao('Partições');
    await esperar('#gradeParticoes');
    contem(await page.textContent('#painel'), 'volume', 'a tela da tabela paginada devia listar os volumes');
    await clicarOuExplicar(page, '#btVoltarGer');
    await esperar('#btVoltarTabs');
    await abrirGestao();

    passo = 'configuracoes -> estrutura completa -> volta';
    await operacao('Configurações');
    await esperar('#btVerEstr');
    await clicarOuExplicar(page, '#btVerEstr');
    await some('#btVerEstr');
    // A aba «estrutura» e a de verdade: a grade das colunas da tabela do caso.
    await page.waitForFunction(() => /nome/.test(document.querySelector('#painel')?.textContent || '')
      && /Str\(40\)|Str/.test(document.querySelector('#painel')?.textContent || ''), undefined, { timeout: ESPERA });
    await abrirGestao();
    await operacao('Configurações');
    await esperar('#btVoltarGer');
    await clicarOuExplicar(page, '#btVoltarGer');
    await esperar('#btVoltarTabs');

    // reparar e reindexar: cada um confirma e tem o seu voltar

    passo = 'reparar e reindexar: cada um confirma e tem o seu voltar';
    for (const nome of ['Reparar índice']) {
      // Recusar o confirm NAO pode abrir a tela de resultado.
      page.once('dialog', d => d.dismiss());
      await operacao(nome);
      await page.waitForTimeout(400);
      verdade(!(await page.$('#btVoltarGer')),
        `«${nome}» abriu o resultado sem o OK do confirm`);
      page.once('dialog', d => d.accept());
      await operacao(nome);
      await esperar('#btVoltarGer');
      await clicarOuExplicar(page, '#btVoltarGer');
      await esperar('#btVoltarTabs');
    }
    // «Reparar tabela» pede o espelho .bkp, que o servidor da bateria NAO liga
    // (`espelho` fica de fora do config): a tela tem de DIZER a recusa e ficar
    // onde estava, e nao abrir um resultado vazio. O `#btVoltarGer` do reparo
    // fica coberto pelo mesmo gancho das outras telas.
    page.once('dialog', d => d.accept());
    await operacao('Reparar tabela');
    await page.waitForFunction(() => /espelho/i.test(document.querySelector('#aviso')?.textContent || ''),
      undefined, { timeout: 8000 });
    verdade(!(await page.$('#btVoltarGer')), 'a recusa do reparo abriu uma tela de resultado');

        // importar uma carga

    passo = 'gestao: voltar para as tabelas do banco';
    await abrirGestao();
    await clicarOuExplicar(page, '#btVoltarTabs');
    await esperar('#btNovaTab');
    await some('#btVoltarTabs');
    contem(await titulo(), db, 'a lista de tabelas devia dizer de que banco e');
    await abrirGestao();

    passo = 'importar uma carga';
    await operacao('Importar');
    await esperar('#btPrever');
    verdade(await page.$eval('#btImportar', b => b.disabled),
      'Gravar nao pode estar liberado antes de Conferir');
    await clicarOuExplicar(page, '#btPrever');       // sem texto: recusa, nao quebra
    await page.waitForFunction(() => /primeiro/i.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: 8000 });
    await page.fill('#impTexto', 'id;nome;cidade\n10;Zelia Prado;Blumenau\n11;Ana Reis;Itajaí');
    // A caixa da carga e a dica tem de ser LEGIVEIS: a caixa media 166 px e a
    // dica saia em caixa alta quando o rotulo ficava fora do `.form-dbl`.
    const caixa = await page.evaluate(() => {
      const t = document.querySelector('#impTexto');
      const dica = t.closest('label').querySelector('.leg');
      return { largura: t.getBoundingClientRect().width, dica: getComputedStyle(dica).textTransform };
    });
    verdade(caixa.largura >= 300, `a caixa da carga mede ${Math.round(caixa.largura)} px`);
    igual(caixa.dica, 'none', 'a dica da carga nao e rotulo: nao vai em caixa alta');
    await clicarOuExplicar(page, '#btPrever');
    await esperar('#gradeAmostraImp .phx-grid');
    verdade(!(await page.$eval('#btImportar', b => b.disabled)),
      'depois de Conferir uma carga boa o Gravar devia liberar');
    const amostra = await page.textContent('#gradeAmostraImp');
    contem(amostra, 'Blumenau', 'a amostra mudou a caixa do dado («Blumenau»)');
    const antesImp = (await linhas()).length;
    await capturar(ctx, ctx.nomeCaptura('importar-conferido'));
    await clicarOuExplicar(page, '#btImportar');
    await page.waitForFunction(() => /gravadas/.test(document.querySelector('#impSaida')?.textContent || ''), undefined, { timeout: ESPERA });
    const depoisImp = await linhas();
    igual(depoisImp.length, antesImp + 2, 'linhas depois de Gravar a carga');
    verdade(await page.$eval('#btImportar', b => b.disabled), 'Gravar devia travar depois de gravar (evita gravar duas vezes)');
    await clicarOuExplicar(page, '#btVoltaImp');
    await some('#btVoltaImp');
    await esperar('#btNovaTab');

    // exportar

    passo = 'exportar';
    await abrirGestao();
    await page.evaluate(() => telaExportar());
    await esperar('#btVoltaExp');
    await clicarOuExplicar(page, '#btVoltaExp');
    await esperar('#btNovaTab');

    // soma de verificacao

    passo = 'soma de verificacao';
    await page.evaluate(() => checksumTabela());
    await esperar('#btVoltaCk');
    contem(await page.textContent('#painel'), String(depoisImp.length), 'a soma nao contou as linhas vivas');
    await capturar(ctx, ctx.nomeCaptura('soma-de-verificacao'));
    await clicarOuExplicar(page, '#btVoltaCk');
    await esperar('#btNovaTab');

    // copiar, colar e esvaziar

    passo = 'copiar, colar e esvaziar';
    await abrirGestao();
    await operacao('Copiar tabela');
    // Copiar nao muda de tela: poe na area de transferencia.
    await page.waitForFunction(() => !!est.copia, undefined, { timeout: 8000 });
    await operacao('Colar aqui');
    await esperar('#btColarAgora');
    await page.fill('#cc_nome', 'clientes_cc');
    await clicarOuExplicar(page, '#btColarAgora');
    await esperar('#btNovaTab');
    const tabelas = (await api(page, 'tabelas', { database: db })).tabelas;
    verdade(tabelas.includes('clientes_cc'), `Colar nao criou a copia: ${tabelas}`);
    // Esvaziar a area: sem copia, a tela diz «nada copiado» e oferece a volta.
    await page.evaluate(d => telaCopiarColar(d), db);
    await esperar('#btLimparCopia');
    await clicarOuExplicar(page, '#btLimparCopia');
    await esperar('#btIrTabelas');
    verdade(await page.evaluate(() => est.copia === null), 'Esvaziar a area nao esvaziou');
    await clicarOuExplicar(page, '#btIrTabelas');
    await esperar('#btNovaTab');

    // motivos e lixeira

    passo = 'motivos e lixeira';
    // Uma linha DE VEZ, com Memo (anexo), para a lixeira ter "ver inteira".
    await api(page, 'excluir', { database: db, tabela: tab, rowid: 1, fisico: true, motivo: 'preparo do caso 39' });
    await abrirGestao();
    await operacao('Motivos');
    await esperar('#btVoltaMot');
    contem(await page.textContent('#painel'), 'preparo do caso 39', 'os motivos nao mostram o que acabou de ser gravado');
    await clicarOuExplicar(page, '#btVerLixo');
    await esperar('#gradeLixeira .ver-lixo');
    await some('#btVoltaMot');
    await clicarOuExplicar(page, '#gradeLixeira .ver-lixo');      // [data-uuid]
    await esperar('#btVoltaLinha');
    contem(await page.textContent('#painel'), 'Adriano Boller', 'a linha descartada nao trouxe o dado');
    contem(await page.textContent('#painel'), 'Blumenau', 'a linha descartada mudou a caixa do dado');
    await capturar(ctx, ctx.nomeCaptura('linha-descartada'));
    await clicarOuExplicar(page, '#btVoltaLinha');
    await esperar('#btVerMotivos');
    await clicarOuExplicar(page, '#btVerMotivos');
    await esperar('#btVoltaMot');
    await clicarOuExplicar(page, '#btVoltaMot');
    await esperar('#btNovaTab');

    // restaurar a partir da ficha de linha marcada

    // Quem NAO administra recebe a recusa no lugar da lista de motivos, e o
    // «← Gerir tabelas» continua la: tela que so oferece o erro e nenhuma
    // saida e armadilha. Esse e o OUTRO sitio do `#btVoltaMot`.
    passo = 'motivos negados a quem nao administra';
    const login = `leitor${ctx.tema === 'claro' ? 'c' : 'e'}`;
    const senha = 'segredo2';
    await api(page, 'usuario_criar', { login, senha, nivel: 'operador', ativo: true, supervisor: false,
      bases: { [db]: { ler: true } } }).catch(() => {});
    const leitor = await page.context().newPage();
    try {
      await entrar(leitor, ctx.url, { usuario: login, senha, token: CREDENCIAL.TOKEN });
      await leitor.evaluate(d => gerirTabelas(d), db);
      await leitor.waitForSelector('#btNovaTab', { timeout: ESPERA });
      await leitor.evaluate(([d, t]) => gerirTabela(d, t), [db, tab]);
      await leitor.waitForSelector('#btVoltarTabs', { timeout: ESPERA });
      await leitor.evaluate(() => telaMotivos());
      await leitor.waitForSelector('#btVoltaMot', { timeout: ESPERA });
      contem(await leitor.textContent('#painel'), 'administrar', 'a recusa devia dizer que o motivo exige administrar');
      verdade(!(await leitor.$('#gradeMotivos')), 'quem nao administra nao pode ver os motivos');
      await clicarOuExplicar(leitor, '#btVoltaMot');
      await leitor.waitForSelector('#btNovaTab', { timeout: ESPERA });
    } finally {
      await leitor.close();
    }

    passo = 'restaurar a partir da ficha de linha marcada';
    await api(page, 'excluir', { database: db, tabela: tab, rowid: 2, motivo: 'marca do caso 39' });
    await page.evaluate(([d, t]) => abrirFicha(d, t, 2), [db, tab]);
    await esperar('#btRestaurarFicha');
    await clicarOuExplicar(page, '#btRestaurarFicha');
    await esperar('#ptTexto');
    await page.fill('#ptTexto', 'engano do caso 39');
    await clicarOuExplicar(page, '#ptSim');
    await some('#btRestaurarFicha');
    const viva = (await linhas()).some(l => +l.rowid === 2);
    verdade(viva, 'Restaurar pela ficha nao devolveu a linha ao conjunto vivo');

    // banco: gestao, SysTables/Columns

    passo = 'banco: gestao, SysTables/Columns';
    await page.evaluate(d => gerirDatabase(d), db);
    await esperar('#painel .ops .op[data-op]');
    const nomeOp = async n => {
      const idx = await page.evaluate(x => {
        const ops = [...document.querySelectorAll('#painel .ops .op')];
        const i = ops.findIndex(b => b.querySelector('.op-txt b').textContent.trim() === x);
        return i < 0 ? null : ops[i].getAttribute('data-op');
      }, n);
      verdade(idx !== null, `a gestao do banco nao tem «${n}»`);
      await clicarOuExplicar(page, `#painel .ops .op[data-op="${idx}"]`);
    };
    await nomeOp('SysTables');
    await esperar('#btSysCols');
    await clicarOuExplicar(page, '#btSysCols');
    await esperar('#btSysTabs');
    await clicarOuExplicar(page, '#btSysTabs');
    await esperar('#btSysCols');
    // «todas as tabelas» so existe na vista de UMA tabela: abre-se pela linha.
    await page.evaluate(([d, t]) => verSysColumns(d, t), [db, tab]);
    await esperar('#btTodas');
    await clicarOuExplicar(page, '#btTodas');
    await some('#btTodas');
    await esperar('#btSysTabs');
    await clicarOuExplicar(page, '#btVoltaDb');
    await esperar('#painel .ops .op[data-op]');

    await nomeOp('Configurações do banco');
    await esperar('#btGerirDb');
    await clicarOuExplicar(page, '#btGerirDb');
    await some('#btGerirDb');
    await esperar('#painel .ops .op[data-op]');

    // A lista de bancos tem o seu «Gerir Banco».
    await page.evaluate(() => verBancos());
    await esperar('#btGerirBancoLista');
    await clicarOuExplicar(page, '#btGerirBancoLista');
    await some('#btGerirBancoLista');
    await esperar('#painel .ops .op[data-op]');
    await capturar(ctx, ctx.nomeCaptura('gestao-do-banco'));
  }
}

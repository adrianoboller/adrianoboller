/* Os botoes do cartao de DECLARAR CHAVE (diagrama ER) e do dialogo de
 * CONFLITO DE ESCRITA (pedido 190):
 *
 *   #fkOk  #fkNao  #btCfNao  #btCfFecha
 *
 * O cartao da chave nasce de um GESTO -- puxar uma coluna ate outra tabela --,
 * e por isso o caso usa o mouse de verdade: `mouse.down`, `mouse.move` em
 * passos, `mouse.up` sobre a porta de destino. Declarar e provado no servidor
 * (o esquema passa a trazer a chave, e a gravacao de uma filha sem mae e
 * RECUSADA), e Cancelar pelo que NAO acontece (esquema intacto).
 *
 * ACHADOS ao exercitar (consertados na mesma rodada):
 *   - o cartao e a nota do diagrama diziam que a chave e «declarada, nao
 *     imposta» e que uma filha orfa ainda entra -- mentira de tela: a chave
 *     NASCE conferida (decisao do dono) e o proprio aviso de sucesso dizia
 *     «ja conferida na gravacao», na mesma tela;
 *   - o menu «ao excluir a linha-mae» oferecia cascata, anular e nada, que o
 *     servidor recusa na declaracao (nunca se mata o pai que tem filhos):
 *     tres das quatro opcoes terminavam em erro.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso, cenario, clicarOuExplicar, abrirLinhaDaGrade, Falha } from '../apoio.mjs';

const ESPERA = 20000;
let passo = 'inicio';

export const caso = {
  nome: 'botoes-do-modelo-e-do-conflito',
  async rodar(ctx) {
    try {
      await corpo(ctx);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        aviso: document.querySelector('#aviso')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    }
  },
};

async function corpo(ctx) {
  const { page } = ctx;
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });
  const some = sel => page.waitForSelector(sel, { state: 'detached', timeout: ESPERA });

  // =========================================================== declarar chave
  passo = 'diagrama: puxar uma coluna ate outra tabela';
  const dbm = bancoDoCaso(ctx, 'Mod');
  await api(page, 'criar_database', { database: dbm }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: dbm, tabela: 'clientes',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }, { nome: 'nome', tipo: 'Str(40)' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  });
  await api(page, 'criar_tabela', {
    database: dbm, tabela: 'pedidos',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }, { nome: 'cliente_id', tipo: 'Int4' }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true },
              { nome: 'porCliente', colunas: ['cliente_id'] }],
  });
  await api(page, 'inserir', { database: dbm, tabela: 'clientes', valores: [1, 'Zelia Prado'] });
  const fks = async () => ((await api(page, 'esquema', { database: dbm, tabela: 'pedidos' })).chaves_estrangeiras || []);
  igual((await fks()).length, 0, 'preparo: a tabela de pedidos nasce sem chave');

  await page.evaluate(() => montarArvore(false));
  await page.evaluate(d => telaDiagramaER(d), dbm);
  await esperar('.er-porta[data-tabela="pedidos"][data-coluna="cliente_id"]');

  const puxar = async () => {
    const de = page.locator('.er-porta[data-tabela="pedidos"][data-coluna="cliente_id"]').first();
    const para = page.locator('.er-porta[data-tabela="clientes"][data-coluna="id"]').first();
    await de.scrollIntoViewIfNeeded();
    const a = await de.boundingBox();
    await para.scrollIntoViewIfNeeded();
    const b = await para.boundingBox();
    verdade(a && b, 'as portas das colunas nao tem caixa na tela');
    // O alvo do `elementFromPoint` e a coordenada da JANELA: depois do segundo
    // scroll a origem pode ter andado, entao mede-se de novo.
    const a2 = await de.boundingBox();
    await page.mouse.move(a2.x + a2.width / 2, a2.y + a2.height / 2);
    await page.mouse.down();
    await page.mouse.move(a2.x + a2.width / 2 + 20, a2.y + a2.height / 2 + 10, { steps: 4 });
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 12 });
    await page.mouse.up();
  };

  await puxar();
  await esperar('#fkOk');
  const opcoesExcluir = await page.$$eval('#fkExc option', os => os.map(o => o.value));
  igual(JSON.stringify(opcoesExcluir), JSON.stringify(['restringir']),
    'o «ao excluir» so pode oferecer restringir: o servidor recusa as outras na declaracao');
  verdade((await page.$$eval('#fkAlt option', os => os.length)) === 4, 'o «ao alterar» devia ter as quatro');
  const texto = await page.textContent('.er-cartao-corpo');
  verdade(!/não\s+imposta/.test(texto), 'o cartao ainda diz que a chave e «nao imposta»');
  contem(texto, 'conferida', 'o cartao devia dizer que a chave nasce conferida');
  contem(texto, 'pedidos.cliente_id', 'o cartao devia nomear a coluna de onde se puxou');
  await capturar(ctx, ctx.nomeCaptura('cartao-da-chave'));

  passo = 'cancelar o cartao da chave';
  await clicarOuExplicar(page, '#fkNao');
  await some('#fkOk');
  igual((await fks()).length, 0, 'Cancelar nao pode declarar chave nenhuma');

  passo = 'declarar a chave';
  await puxar();
  await esperar('#fkOk');
  igual(await page.inputValue('#fkNome'), 'fk_clientes', 'o nome sugerido da chave');
  await clicarOuExplicar(page, '#fkOk');
  await some('#fkOk');
  const depois = await fks();
  igual(depois.length, 1, 'a chave declarada pelo cartao devia estar no esquema');
  igual(depois[0].tabela_ref, 'clientes', 'a chave devia apontar para a tabela mae');
  // E «conferida na gravacao», como a tela promete: filha sem mae e recusada.
  const orfa = await api(page, 'inserir', { database: dbm, tabela: 'pedidos', valores: [1, 99] }).then(() => null, e => String(e));
  verdade(orfa, 'a chave declarada pelo cartao nao esta sendo conferida: a filha sem mae entrou');
  contem(orfa, 'clientes', 'a recusa devia nomear a tabela mae');
  await api(page, 'inserir', { database: dbm, tabela: 'pedidos', valores: [2, 1] });   // com mae: entra

  // ============================================================= conflito
  passo = 'conflito: descartar o meu';
  const db = bancoDoCaso(ctx, 'Cfl');
  const { tab } = await cenario(page, db);
  const alvo = (await api(page, 'varrer', { database: db, tabela: tab, max: 50 })).linhas.find(l => l.id === 1);
  const rowid = alvo.rowid;

  await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
  await abrirLinhaDaGrade(page, { rowid });
  await esperar('#fichaEdit');
  // Outra sessao grava primeiro (a cidade), e a minha edicao nasce velha.
  const atual = (await api(page, 'ler', { database: db, tabela: tab, rowid }));
  const linhaAtual = atual.linha || atual;
  const valoresDe = (l, troca) => {
    const o = {};
    for (const k of ['id', 'nome', 'cidade', 'uf', 'limite', 'cadastro', 'ficha']) o[k] = l[k] === undefined ? null : l[k];
    return { ...o, ...troca };
  };
  await api(page, 'atualizar', { database: db, tabela: tab, rowid, valores: valoresDe(linhaAtual, { cidade: 'Pomerode' }) });
  await page.fill('#f_cidade', 'Indaial');
  await clicarOuExplicar(page, '#btSalvar');
  await esperar('.caixa.larga[aria-label="Conflito de escrita"]');
  await capturar(ctx, ctx.nomeCaptura('conflito'));
  await clicarOuExplicar(page, '#btCfNao');
  await some('.caixa.larga[aria-label="Conflito de escrita"]');
  const final = await api(page, 'ler', { database: db, tabela: tab, rowid });
  igual((final.linha || final).cidade, 'Pomerode', '«Descartar o meu» nao pode gravar a minha cidade por cima da do outro');

  passo = 'conflito: a linha foi excluida de vez';
  const alvo2 = (await api(page, 'varrer', { database: db, tabela: tab, max: 50 })).linhas.find(l => l.id === 2);
  await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
  await abrirLinhaDaGrade(page, { rowid: alvo2.rowid });
  await esperar('#fichaEdit');
  await api(page, 'excluir', { database: db, tabela: tab, rowid: alvo2.rowid, fisico: true, motivo: 'caso 45: excluida enquanto aberta' });
  await page.fill('#f_cidade', 'Gaspar');
  await clicarOuExplicar(page, '#btSalvar');
  await esperar('#btCfFecha');
  contem(await page.textContent('.caixa'), 'excluído', 'o dialogo devia dizer que a linha foi excluida');
  await capturar(ctx, ctx.nomeCaptura('conflito-excluida'));
  await clicarOuExplicar(page, '#btCfFecha');
  await some('#btCfFecha');
  verdade(!(await api(page, 'varrer', { database: db, tabela: tab, max: 50 })).linhas.some(l => l.rowid === alvo2.rowid),
    'fechar o dialogo nao pode trazer a linha excluida de volta');
}

/* A grade com coluna de sistema.
 *
 * A regra da casa: «coluna de sistema nova quebra quem filtra pela primeira».
 * Este caso nao conhece `softdeleted` nem `rownum` pelo nome -- ele PERGUNTA
 * ao esquema quais colunas sao de sistema e exige que NENHUMA delas vire
 * coluna de dado na grade editavel. Uma terceira coluna de sistema que entre
 * amanha ja esta coberta.
 *
 * A grade da aba Conteudo e outra historia, e de proposito: ali as colunas
 * saem inteiras do esquema, com as de sistema junto, porque aquela aba mostra
 * a linha como ela esta no `.reg`. O caso trava as DUAS decisoes -- se um dia
 * alguem uniformizar as duas grades, um dos dois lados falha e a conversa
 * acontece antes do commit, e nao depois do relato. */
import { entrar, capturar, cenario, api, verdade, igual, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'grade',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Grade');
    const { tab } = await cenario(page, db);

    const esquema = await api(page, 'esquema', { database: db, tabela: tab });
    const sistema = esquema.colunas.filter(c => c.sistema).map(c => c.nome);
    const doUsuario = esquema.colunas.filter(c => !c.sistema).map(c => c.nome);
    verdade(sistema.length >= 2, 'a tabela precisa de duas colunas de sistema para este caso');

    // -------------------------------------------------- grade EDITAVEL
    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await page.waitForSelector('#painel table');
    await capturar(ctx, ctx.nomeCaptura('grade-editavel'));

    // Le o CAMPO da coluna (`data-c`), e nao o texto do cabecalho.
    //
    // Este teste comparava `textContent` exato, e quebrou no dia em que a
    // grade editavel virou PhxGrid: o cabecalho passou a carregar a seta de
    // ordenacao, e «id» virou «id▼». A coluna estava la -- medido, o
    // `data-c` dizia `id` --, entao o teste acusava um sumico que nao houve.
    //
    // Ler o campo NAO afrouxa a guarda, aperta: ela passa a perguntar «esta
    // coluna existe?» em vez de «o cabecalho esta escrito assim?», que era
    // uma pergunta sobre a APARENCIA respondendo por uma sobre a ESTRUTURA.
    // E fica imune a seta, a marca de agregador e ao que a grade decidir
    // desenhar amanha.
    const campos = await page.$$eval('#painel table thead th',
      ths => ths.map(t => t.getAttribute('data-c') || ''));
    for (const s of sistema) {
      verdade(!campos.includes(s),
        `a coluna de sistema «${s}» virou coluna de dado na grade editavel`);
    }
    for (const c of doUsuario) {
      verdade(campos.includes(c), `a coluna «${c}» sumiu da grade editavel`);
    }
    igual(campos[0], 'rownum', 'a coluna do numero de ordem nao e a primeira');
    igual(campos[1], 'rowid', 'a coluna do rowid nao e a segunda');

    // E a faixa de agrupamento, que e o motivo de a tela ter virado PhxGrid.
    verdade(await page.$('#painel .phx-groupbox') !== null,
      'a grade editavel perdeu a faixa de agrupamento');

    // O «nº» mostra o `rownum` de verdade, e nao o indice da linha na pagina.
    const varrido = await api(page, 'varrer', { database: db, tabela: tab, max: 50 });
    const naTela = await page.$$eval('#painel tbody tr td.ordem', tds =>
      tds.map(t => t.textContent.trim()));
    igual(naTela.join(','), varrido.linhas.map(l => String(l.rownum)).join(','),
      'a coluna «nº» nao mostra o rownum das linhas');

    // Nenhuma celula de dado mostra o booleano do softdeleted disfarcado de
    // valor -- se ele voltasse como coluna, apareceria como «false».
    const celulas = await page.$$eval('#painel tbody tr td.dado', tds =>
      tds.map(t => t.textContent.trim()));
    verdade(!celulas.includes('false') && !celulas.includes('true'),
      'apareceu um booleano de coluna de sistema entre os dados da grade');

    // ------------------------------------------------ grade da aba Conteudo
    await page.evaluate(([d, t]) => abrirTabela(d, t), [db, tab]);
    await page.click('.aba[data-aba="conteudo"]');
    await page.waitForSelector('#grade', { timeout: 10000 });
    await page.waitForTimeout(500);
    await capturar(ctx, ctx.nomeCaptura('grade-phx'));

    // `colunasVisiveis()` e a API publica do phx-grid. Ler o texto dos `th`
    // seria ler o rotulo, que passa por `rot()` e pode ser trocado pelo
    // editor de menu -- o teste falaria do nome exibido, e nao da coluna.
    const colunasDoGrid = await page.evaluate(() => window.__phxgrade.colunasVisiveis());
    verdade(colunasDoGrid.length > 0, 'o phx-grid da aba Conteudo nao montou coluna nenhuma');
    for (const c of [...doUsuario, ...sistema]) {
      verdade(colunasDoGrid.includes(c),
        `o phx-grid perdeu a coluna «${c}» — as colunas dele saem do esquema, `
        + 'e nenhuma pode ficar pelo caminho');
    }
    const linhasNoGrid = await page.locator('#grade tbody tr').count();
    verdade(linhasNoGrid >= 3, `o phx-grid desenhou ${linhasNoGrid} linhas de 3`);
  },
};

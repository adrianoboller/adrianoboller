/* Incluir e salvar PELA TELA -- o fluxo que ja quebrou inteiro nesta casa.
 *
 * Quando o `rownum` entrou, a ficha filtrava a coluna de sistema com
 * `find(c => c.sistema)`: so o `softdeleted` saia e o `rownum` continuava no
 * formulario. O formulario mandava 8 valores para uma tabela de 9 colunas, e
 * TODO salvar e TODO incluir pela interface falhavam com «a lista tem 8
 * valores». Os 1.106 testes de `cargo test` passavam todos: nenhum deles
 * monta um formulario.
 *
 * A asercao que trava o defeito e a primeira -- NENHUMA coluna de sistema
 * vira campo. Ela falha com o `find` reposto mesmo antes de o botao ser
 * clicado, e diz o nome da coluna que sobrou. */
import { entrar, capturar, cenario, api, verdade, igual, contem, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'ficha',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Ficha');
    const { tab } = await cenario(page, db);

    const esquema = await api(page, 'esquema', { database: db, tabela: tab });
    const sistema = esquema.colunas.filter(c => c.sistema).map(c => c.nome);
    verdade(sistema.length >= 2,
      `esta tabela tem ${sistema.length} coluna(s) de sistema — o caso precisa de ao menos duas `
      + 'para poder distinguir «tira a primeira» de «tira todas»');

    // ------------------------------------------------------------ incluir
    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await page.waitForSelector('#btNova');
    await page.click('#btNova');
    await page.waitForSelector('#fichaEdit');
    await capturar(ctx, ctx.nomeCaptura('ficha-nova'));

    for (const nome of sistema) {
      const quantos = await page.locator(`#f_${nome}`).count();
      igual(quantos, 0,
        `a coluna de sistema «${nome}» virou campo do formulario — `
        + 'e o `inserir` vai mandar valor a mais');
    }
    const campos = await page.locator('#fichaEdit [id^="f_"]').count();
    igual(campos, esquema.colunas.length - sistema.length,
      'a ficha nao tem um campo por coluna editavel');

    await page.fill('#f_id', '4');
    await page.fill('#f_nome', 'Joana Testadora');
    await page.fill('#f_cidade', 'Blumenau');
    await page.fill('#f_uf', 'SC');
    await page.fill('#f_limite', '1234.56');
    await page.fill('#f_cadastro', '2026-02-10');
    await page.click('#btSalvar');
    await page.waitForSelector('#btNova', { timeout: 10000 });
    await page.waitForTimeout(250);

    const aviso = await page.evaluate(() => {
      const a = document.querySelector('#aviso:not([hidden])');
      return a ? { txt: a.textContent.trim(), mal: a.classList.contains('mal') } : null;
    });
    verdade(aviso && !aviso.mal,
      `o incluir pela tela falhou: ${aviso ? aviso.txt : 'nem aviso saiu'}`);
    contem(aviso.txt, 'incluída', 'o aviso do incluir nao confirmou a inclusao');

    const depois = await api(page, 'varrer', { database: db, tabela: tab, max: 50 });
    igual(depois.linhas.length, 4, 'a linha incluida pela tela nao chegou na tabela');
    const nova = depois.linhas.find(l => l.nome === 'Joana Testadora');
    verdade(!!nova, 'a linha incluida pela tela nao veio com o nome digitado');
    verdade(nova.rownum > 0, 'o motor nao preencheu o rownum da linha incluida pela tela');
    igual(nova.softdeleted, false, 'a linha nasceu marcada como excluida');

    // ------------------------------------------------------------- salvar
    await page.click(`#painel .linha-dado[data-rowid="${nova.rowid}"]`);
    await page.waitForSelector('#fichaEdit');
    for (const nome of sistema) {
      igual(await page.locator(`#f_${nome}`).count(), 0,
        `ao EDITAR, a coluna de sistema «${nome}» virou campo`);
    }
    await capturar(ctx, ctx.nomeCaptura('ficha-edicao'));
    await page.fill('#f_cidade', 'Pomerode');
    await page.click('#btSalvar');
    await page.waitForSelector('#btNova', { timeout: 10000 });
    await page.waitForTimeout(250);

    const salvo = await page.evaluate(() => {
      const a = document.querySelector('#aviso:not([hidden])');
      return a ? { txt: a.textContent.trim(), mal: a.classList.contains('mal') } : null;
    });
    verdade(salvo && !salvo.mal,
      `o salvar pela tela falhou: ${salvo ? salvo.txt : 'nem aviso saiu'}`);

    const conferido = await api(page, 'ler', { database: db, tabela: tab, rowid: nova.rowid });
    const linha = conferido.linha || conferido;
    igual(linha.cidade, 'Pomerode', 'o salvar pela tela nao gravou o campo alterado');
    igual(linha.rownum, nova.rownum, 'salvar renumerou o rownum — a ordem de digitacao mudou');
  },
};

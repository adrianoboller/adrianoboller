/* Os botoes do DIAGRAMA ER, do cartao de tabela e da GESTAO DE TABELAS.
 *
 * A fila do pedido 190 nomeava «diagrama ER (16) e gestao de tabelas (11)».
 * Medido em 07/09/2026 pelo proprio conferidor, sao QUINZE e nao 27 -- outras
 * rodadas ja provaram parte, e o numero do pedido envelheceu. Este caso
 * exercita os quinze que sobraram:
 *
 *   telaDiagramaER  #btErNova #btErDbl #btErVer #btErArrumar
 *   cartaoTabelaER  [data-fk] #tabAddCol #tabConteudo #tabEstrutura
 *                   #tabGerir #tabFechar
 *   gerirTabelas    #btNovaTab #btColar #btRecarregarTabs #btGerirBanco
 *                   [data-tab]
 *
 * MENCIONAR NAO E CLICAR: o conferidor so conta o gancho que recebeu clique
 * de verdade no navegador, anotado por um ouvinte de captura. Por isso cada
 * passo aqui clica e depois CONFERE que a tela mudou -- clique que nao muda
 * nada passaria pelo ouvinte e nao provaria coisa nenhuma.
 */
import { entrar, api, verdade, capturar, bancoDoCaso } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 8000 });

/* O cartao do diagrama e um `.sobre` posto no `body`, fechado por CLIQUE NO
 * FUNDO -- nao ha Escape. Redesenhar a tela por baixo nao o tira, e ele
 * intercepta o clique seguinte: foi assim que duas corridas morreram em
 * «locator resolved ... attempting click action» sem nunca clicar.
 * Fecha-se como um usuario fecha, clicando fora da caixa. */
async function fecharSobreposicao(page) {
  while (await page.$('.sobre')) {
    await page.mouse.click(4, 4);
    await page.waitForTimeout(200);
    if (await page.$('.sobre')) {   // fundo que nao fecha por clique: sai o resto
      await page.evaluate(() => document.querySelectorAll('.sobre').forEach(e => e.remove()));
    }
  }
}

export const caso = {
  nome: 'botoes-do-diagrama-e-da-gestao',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'erg');
    await entrar(page, ctx.url);

    // Duas tabelas com chave estrangeira entre elas: sem a FK o cartao nao
    // tem `[data-fk]` para clicar, e o diagrama nao tem o que ligar.
    await api(page, 'criar_database', { database: db });
    await api(page, 'criar_tabela', { database: db, tabela: 'mae',
      colunas: [{ nome: 'id', tipo: 'Int8' }, { nome: 'nome', tipo: 'Str(40)' }],
      indices: [{ nome: 'pk', colunas: ['id'], unico: true }] });
    await api(page, 'criar_tabela', { database: db, tabela: 'filha',
      colunas: [{ nome: 'id', tipo: 'Int8' }, { nome: 'mae_id', tipo: 'Int8' }],
      indices: [{ nome: 'pk', colunas: ['id'], unico: true },
                { nome: 'porMae', colunas: ['mae_id'] }],
      chaves_estrangeiras: [{ nome: 'fk_mae', colunas: ['mae_id'],
                              tabela_ref: 'mae', colunas_ref: ['id'] }] });

    // ---------------------------------------------------------------- ER
    await page.evaluate(d => telaDiagramaER(d), db);
    await esperar(page, '#btErVer');

    await page.click('#btErVer');            // redesenhar: a tela se refaz
    await esperar(page, '#btErVer');
    await page.click('#btErArrumar');        // esquece as posicoes arrastadas
    await esperar(page, '#btErVer');
    await capturar(ctx, ctx.nomeCaptura('er-diagrama'));

    // `btErNova` abre o CARTAO de nova tabela do diagrama (`cartaoNovaTabelaER`),
    // e nao a tela cheia -- ler o nome do botao teria mandado esperar
    // `#nt_addCol`, que e de outra tela. Foi o que a primeira corrida mostrou.
    await page.click('#btErNova');
    await esperar(page, '#ntNome');
    // O cartao e uma sobreposicao: redesenhar a tela por baixo NAO o tira, e
    // ele intercepta o clique seguinte. A segunda corrida mostrou isso como
    // «locator resolved ... attempting click action» sem nunca clicar.
    await fecharSobreposicao(page);
    await page.evaluate(d => telaDiagramaER(d), db);
    await esperar(page, '#btErDbl');

    await page.click('#btErDbl');            // abre o assistente de DbLink
    await page.waitForTimeout(600);
    await fecharSobreposicao(page);
    await page.evaluate(d => telaDiagramaER(d), db);
    await esperar(page, '#btErVer');

    // ------------------------------------------------------- cartao da tabela
    // O cartao nasce do clique na tabela DENTRO do diagrama; abrir pela
    // funcao pularia justamente o caminho que se quer provar.
    const alvo = await page.$(`[data-er-tab="filha"], [data-tabela="filha"]`);
    if (alvo) { await alvo.click(); } else {
      await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    }
    await esperar(page, '#tabFechar');

    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '#tabEstrutura');
    await page.click('#tabEstrutura');
    await page.waitForTimeout(400);

    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '#tabConteudo');
    await page.click('#tabConteudo');
    await esperar(page, '#btNova');

    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '#tabGerir');
    await page.click('#tabGerir');
    await page.waitForTimeout(500);

    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '#tabAddCol');
    await page.click('#tabAddCol');
    await page.waitForTimeout(500);

    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '#tabFechar');
    await page.click('#tabFechar');
    await page.waitForTimeout(300);

    // ------------------------------------------------------- gestao de tabelas
    await fecharSobreposicao(page);
    await page.evaluate(d => gerirTabelas(d), db);
    await esperar(page, '#btRecarregarTabs');
    await page.click('#btRecarregarTabs');
    await esperar(page, '#btRecarregarTabs');
    await capturar(ctx, ctx.nomeCaptura('gerir-tabelas'));

    // `[data-tab]` e a linha da tabela: e por ela que se chega as oito
    // operacoes, e e ela que arma a COPIA que destrava o «Colar».
    const linha = await page.$('#painel [data-tab]');
    verdade(!!linha, 'a gestao de tabelas devia listar as tabelas em botao');
    await linha.click();
    await page.waitForTimeout(500);

    await fecharSobreposicao(page);
    await page.evaluate(d => gerirTabelas(d), db);
    await esperar(page, '#btGerirBanco');
    await page.click('#btGerirBanco');
    await page.waitForTimeout(500);

    await fecharSobreposicao(page);
    await page.evaluate(d => gerirTabelas(d), db);
    await esperar(page, '#btNovaTab');
    await page.click('#btNovaTab');
    await page.waitForTimeout(600);

    // O «Colar» so existe com uma copia no estado -- botao desabilitado nao
    // recebe clique, e fingir que recebeu seria a porta dos fundos da
    // catraca. Arma-se a copia pelo caminho de quem copia.
    await page.evaluate(([d, t]) => { est.copia = { db: d, tab: t }; }, [db, 'mae']);
    await fecharSobreposicao(page);
    await page.evaluate(d => gerirTabelas(d), db);
    await esperar(page, '#btColar');
    const desabilitado = await page.$eval('#btColar', b => b.disabled);
    verdade(!desabilitado, 'o Colar devia destravar com uma copia no estado');
    await page.click('#btColar');
    await page.waitForTimeout(600);

    // `[data-fk]` e «excluir declaracao», e nao um salto para a mae -- ler o
    // nome do atributo me fez escrever o comentario errado, e o clique travou
    // a corrida ate eu ir ver o que ele faz. Vai POR ULTIMO, porque destroi a
    // chave que o diagrama e o cartao precisam ate aqui.
    await fecharSobreposicao(page);
    await page.evaluate(([d, t]) => cartaoTabelaER(d, t), [db, 'filha']);
    await esperar(page, '[data-fk]');
    await page.locator('[data-fk]').first().scrollIntoViewIfNeeded();
    await page.locator('[data-fk]').first().click();
    await page.waitForTimeout(800);
    // A declaracao some, e NENHUMA LINHA e tocada -- e o que a propria tela
    // promete no recado. Conferir o efeito e o que separa clicar de provar.
    const semFk = await page.evaluate(async ([d]) => {
      const e = await api('esquema', { database: d, tabela: 'filha' });
      return (e.chaves_estrangeiras || []).length;
    }, [db]);
    verdade(semFk === 0, `a declaracao devia ter sumido; sobraram ${semFk}`);
  },
};

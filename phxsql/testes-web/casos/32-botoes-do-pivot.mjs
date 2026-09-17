/* Os botoes da TABELA DINAMICA -- a trilha dos passos e os tres passos.
 *
 * Treze botoes que ninguem tinha clicado: a trilha (`[data-passo]`), o passo 1
 * (a juncao sugerida pela chave estrangeira, o tirar, o juntar a mao, o
 * avancar), o passo 2 (o x do chip, o montar, o voltar, o limpar) e o passo 3
 * (o voltar, o CSV, o «ver o pedido» e o voltar de dentro dele).
 *
 * O CENARIO NAO E ENFEITE: `[data-sug]` so nasce quando a tabela dos FATOS
 * declara chave estrangeira -- a tela propoe a juncao a partir do esquema --,
 * e a mae precisa de chave PRIMARIA, senao o `pivotar` recusa juntar dizendo
 * por qual coluna. Sem as duas coisas o passo 1 renderiza a metade que nao
 * tem o que clicar.
 *
 * Cada passo confere que a tela mudou; onde o clique redesenha o MESMO molde
 * (o `[data-passo]` da trilha, o «limpar»), a prova e o estado que ele mexe.
 */
import { entrar, api, verdade, capturar, bancoDoCaso } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 10000 });

export const caso = {
  nome: 'botoes-do-pivot',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'pvt');
    await entrar(page, ctx.url);

    await api(page, 'criar_database', { database: db }).catch(() => {});
    // A mae com chave PRIMARIA: sem `primario` o motor recusa a juncao
    // ("nao tem chave primaria; diga por qual coluna juntar"), e o passo 3
    // voltaria ao 2 com o erro em vez de montar.
    await api(page, 'criar_tabela', {
      database: db, tabela: 'ufs',
      // A coluna da chave primaria nao aceita nulo -- o motor recusa a tabela
      // dizendo isso, e a recusa saia calada por tras do `.catch`.
      colunas: [{ nome: 'sigla', tipo: 'Str(2)', obrigatoria: true },
                { nome: 'nome', tipo: 'Str(30)' }],
      indices: [{ nome: 'pk', colunas: ['sigla'], unico: true, primario: true }],
    }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: 'vendas',
      colunas: [{ nome: 'id', tipo: 'Int8', obrigatoria: true },
                { nome: 'uf', tipo: 'Str(2)' },
                { nome: 'mes', tipo: 'Str(7)' }, { nome: 'valor', tipo: 'Decimal(12,2)' }],
      indices: [{ nome: 'pk', colunas: ['id'], unico: true, primario: true },
                { nome: 'porUf', colunas: ['uf'] }],
      chaves_estrangeiras: [{ nome: 'fk_uf', colunas: ['uf'],
                              tabela_ref: 'ufs', colunas_ref: ['sigla'] }],
    }).catch(() => {});
    for (const l of [['SC', 'Santa Catarina'], ['PR', 'Paraná']]) {
      await api(page, 'inserir', { database: db, tabela: 'ufs', valores: l }).catch(() => {});
    }
    for (const l of [[1, 'SC', '2025-01', '100.00'], [2, 'PR', '2025-01', '250.00'],
                     [3, 'SC', '2025-02', '70.00']]) {
      await api(page, 'inserir', { database: db, tabela: 'vendas', valores: l }).catch(() => {});
    }

    // ------------------------------------------------- passo 1: as tabelas
    await page.evaluate(d => telaPivot(d), db);
    await esperar(page, '#pvCorpo .linha-tab');
    await page.click(`#pvCorpo .linha-tab[data-t="vendas"]`);
    await esperar(page, '#pv_ir2');

    // A juncao SUGERIDA vem da chave estrangeira declarada.
    await page.click('[data-sug="0"]');
    await esperar(page, '[data-tirar]');
    await capturar(ctx, ctx.nomeCaptura('pivot-passo1'));

    // Tirar e por de volta: o «tirar» so prova alguma coisa se a linha sumir.
    await page.click('[data-tirar="0"]');
    await page.waitForTimeout(400);
    verdade(!(await page.$('[data-tirar]')), 'o «tirar» devia ter tirado a juncao');

    // «+ juntar» a mao, que e o caminho de quem nao tem FK declarada.
    await page.selectOption('#pv_jt', 'ufs');
    await page.waitForTimeout(300);
    await page.selectOption('#pv_jc', 'uf');
    await page.fill('#pv_jp', 'uf');
    await page.click('#pv_addjun');
    await esperar(page, '[data-tirar]');

    await page.click('#pv_ir2');
    await esperar(page, '#pv_montar');

    // ------------------------------------------------- passo 2: os campos
    await page.click('.pv-campo[data-campo="uf"]');       // clicar manda as Linhas
    await esperar(page, 'button[data-zona]');
    // O x do chip devolve o campo a paleta -- e e o botao `[data-zona]`.
    await page.click('button[data-zona]');
    await page.waitForTimeout(400);
    verdade(!(await page.$('button[data-zona]')), 'o x do chip devia ter esvaziado a zona');

    await page.click('.pv-campo[data-campo="uf"]');
    await esperar(page, 'button[data-zona]');
    await page.click('#pv_limpar');
    await page.waitForTimeout(400);
    verdade(!(await page.$('button[data-zona]')), 'o «limpar os eixos» devia esvaziar as tres zonas');

    // «← Tabelas» volta ao passo 1 pela trilha de dentro do passo 2.
    await page.click('#pv_voltar1');
    await esperar(page, '#pv_ir2');

    // A TRILHA de cima: `[data-passo]` e o atalho que pula de um passo a outro.
    await page.click('#painel .passo[data-passo="2"]');
    await esperar(page, '#pv_montar');

    await page.click('.pv-campo[data-campo="uf"]');
    await page.waitForTimeout(300);
    // Contagem e o unico resumo que dispensa campo de Valores.
    await page.selectOption('#pv_ag', 'contagem');
    await page.waitForTimeout(200);
    await page.click('#pv_montar');

    // ----------------------------------------------- passo 3: o resultado
    await esperar(page, '#pv_csv');
    const tabela = await page.$eval('#pvCorpo', e => e.textContent);
    verdade(/SC/.test(tabela), 'o pivot devia mostrar a UF cruzada');
    await capturar(ctx, ctx.nomeCaptura('pivot-resultado'));

    // «Copiar como CSV» tem de DIZER que copiou. Este passo reprovava antes
    // de 17/09/2026: um `const txt` local escondia a funcao `txt()` da fabrica
    // de idiomas, e o recado morria em «txt is not a function» depois de a
    // copia ja ter acontecido -- quem clicava via a tela calada.
    await page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.textContent = ''; });
    await page.click('#pv_csv');
    await page.waitForTimeout(700);
    const recado = await page.evaluate(() =>
      (document.querySelector('#aviso') || {}).textContent || '');
    verdade(recado.trim().length > 0,
      'o «Copiar como CSV» devia escrever um recado (copiou, ou por que nao deu)');

    await page.click('#pv_json');
    await esperar(page, '#pv_volta3');
    await page.click('#pv_volta3');
    await esperar(page, '#pv_csv');

    await page.click('#pv_voltar2');
    await esperar(page, '#pv_montar');
  },
};

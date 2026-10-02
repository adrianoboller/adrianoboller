/* Os botoes de CRIAR TABELA: o cartao do diagrama ER e a tela cheia.
 *
 * Oito botoes que ninguem tinha clicado (pedido 190):
 *
 *   cartaoNovaTabelaER   #ntMais #ntCriar #ntCompleto #ntFechar
 *   desenharNovaTabela   #nt_addCol #nt_addIdx #nt_criar #nt_voltar
 *
 * O cartao e a tela cheia sao DUAS telas para a mesma ordem (`criar_tabela`),
 * e o elo entre elas e o rascunho: o «Cadastro completo…» leva o que ja foi
 * digitado no cartao. Por isso o caso nao clica cada botao isolado -- ele
 * percorre o caminho de quem cria uma tabela e, em cada passo, confere o
 * EFEITO: a linha que nasceu, o nome que sobreviveu ao redesenho, a tabela que
 * apareceu (ou nao) no servidor. O que prova a criacao e a operacao `esquema`,
 * e nao o que a tela diz.
 *
 * E OLHA a tela: o cartao e um formulario dentro de uma tabela, que e
 * exatamente onde o `input{width:100%}` global vira bolinha do tamanho da
 * celula e onde o rotulo em caixa alta pode vazar para o dado.
 */
import { entrar, api, verdade, igual, capturar, bancoDoCaso } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 15000 });

/* O cartao e um `.sobre` no `body`, fechado por clique no fundo (sem Escape). */
async function fecharSobre(page) {
  while (await page.$('.sobre')) {
    await page.mouse.click(4, 4);
    await page.waitForTimeout(150);
    if (await page.$('.sobre')) await page.evaluate(() => document.querySelectorAll('.sobre').forEach(e => e.remove()));
  }
}

const existe = async (page, db, tab) =>
  (await api(page, 'tabelas', { database: db })).tabelas.map(t => t.toLowerCase()).includes(tab.toLowerCase());

export const caso = {
  nome: 'botoes-de-nova-tabela',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'nvt');
    await entrar(page, ctx.url);
    await api(page, 'criar_database', { database: db }).catch(() => {});
    await page.evaluate(d => { est.rascunho = null; return telaDiagramaER(d); }, db);
    await esperar(page, '#btErNova');

    // ------------------------------------------------ o cartao: olhar primeiro
    await page.click('#btErNova');
    await esperar(page, '#ntNome');
    await capturar(ctx, ctx.nomeCaptura('cartao-nova-tabela'));

    // Nada de controle esticado: o rádio e a caixa de marcar do cartao moram
    // dentro de celulas, e `input{width:100%}` os faria do tamanho da celula.
    const largura = await page.evaluate(() => {
      const w = s => document.querySelector(s).getBoundingClientRect().width;
      return { obrig: w('.nt-obrig'), pk: w('.nt-pk') };
    });
    verdade(largura.obrig < 40 && largura.pk < 40,
      `os controles do cartao esticaram: caixa ${largura.obrig}px, radio ${largura.pk}px`);

    // O nome da tabela e DADO digitado: nenhuma caixa alta pode cair sobre ele.
    await page.fill('#ntNome', 'CadastroMisto');
    const visto = await page.$eval('#ntNome', el => ({
      transf: getComputedStyle(el).textTransform, valor: el.value }));
    igual(visto.transf, 'none', 'o nome digitado da tabela nao pode ganhar text-transform');
    igual(visto.valor, 'CadastroMisto', 'o nome da tabela');

    // «+ campo»: nasce uma linha, e o nome ja digitado SOBREVIVE ao redesenho.
    const linhas = () => page.$$eval('.nt-nome', els => els.length);
    igual(await linhas(), 2, 'o cartao nasce com dois campos');
    await page.click('#ntMais');
    await page.waitForFunction(() => document.querySelectorAll('.nt-nome').length === 3);
    igual(await page.inputValue('#ntNome'), 'CadastroMisto',
      'o nome da tabela devia sobreviver ao «+ campo»');
    await page.fill('.nt-nome[data-i="2"]', 'Cidade');

    // «Cancelar»: fecha e NAO cria nada.
    await page.click('#ntFechar');
    await page.waitForFunction(() => !document.querySelector('.sobre'));
    verdade(!(await existe(page, db, 'CadastroMisto')), 'Cancelar nao pode criar a tabela');

    // «Criar tabela» com nome vazio: recusa DENTRO do cartao, sem chamar o servidor.
    await page.click('#btErNova');
    await esperar(page, '#ntNome');
    await page.click('#ntCriar');
    await esperar(page, '#ntRecado .aviso.mal');
    verdade(await page.$('.sobre'), 'o cartao devia continuar aberto depois da recusa');

    // «Criar tabela» de verdade: o servidor responde com o esquema pedido.
    await page.fill('#ntNome', 'ViaCartao');
    await page.click('#ntCriar');
    await page.waitForFunction(() => !document.querySelector('.sobre'), undefined, { timeout: 15000 });
    const e = await api(page, 'esquema', { database: db, tabela: 'ViaCartao' });
    const nomes = e.colunas.map(c => c.nome).filter(n => !['rowstamp', 'rowtime', 'rownum', 'softdeleted'].includes(n));
    igual(nomes.join(','), 'id,nome', 'as colunas que o cartao criou');
    verdade(e.indices.some(i => i.primario), 'a chave primaria marcada no cartao devia virar indice primario');
    // E a tela voltou ao diagrama. Este caso JA ESPEROU `ER.esquemas` ter a
    // tabela, para o repintar do «Criar» nao pintar POR CIMA da tela cheia
    // aberta no passo seguinte -- e a espera ESCONDIA o defeito em vez de
    // consertar (pedido 636, familia do 170). O conserto e do produto: o
    // repintar confere a posse do painel e, se a pessoa ja abriu outra tela,
    // nao pinta. Quem prova isso e o caso `pintura-tardia`; aqui nao ha espera
    // nenhuma, e o passo seguinte abre a tela cheia COM o repintar a caminho.
    await esperar(page, '#btErNova');

    // ------------------------------------- «Cadastro completo…» leva o rascunho
    await page.click('#btErNova');
    await esperar(page, '#ntNome');
    await page.fill('#ntNome', 'ViaCompleto');
    await page.click('#ntMais');
    await page.waitForFunction(() => document.querySelectorAll('.nt-nome').length === 3);
    await page.fill('.nt-nome[data-i="2"]', 'cidade');
    await page.click('#ntCompleto');
    await esperar(page, '#nt_criar');
    verdade(!(await page.$('.sobre')), 'o cartao devia ter fechado ao ir para o cadastro completo');
    igual(await page.inputValue('#nt_nome'), 'ViaCompleto',
      'o nome digitado no cartao devia ter ido para o cadastro completo');
    const campos = await page.$$eval('#nt_cols .c-nome', els => els.map(x => x.value));
    igual(campos.join(','), 'id,nome,cidade', 'os campos do cartao no cadastro completo');
    await capturar(ctx, ctx.nomeCaptura('nova-tabela-cheia'));

    // ------------------------------------------------------ a tela cheia
    const n = sel => page.$$eval(sel, els => els.length);
    await page.click('#nt_addCol');
    await page.waitForFunction(() => document.querySelectorAll('#nt_cols tr').length === 4);
    igual(await page.inputValue('#nt_nome'), 'ViaCompleto', 'o «+ campo» nao pode perder o nome');
    await page.click('#nt_addIdx');
    await page.waitForFunction(() => document.querySelectorAll('#nt_idxs tr').length === 2);
    igual(await n('#nt_idxs tr'), 2, 'o «+ indice» acrescenta uma linha');

    // «Criar tabela» sem nome da quarta coluna e com o indice vazio: o vazio e
    // ignorado, e o que nao e vazio chega ao servidor.
    await page.click('#nt_criar');
    await page.waitForFunction(() => !document.querySelector('#nt_criar'), undefined, { timeout: 15000 });
    const c = await api(page, 'esquema', { database: db, tabela: 'ViaCompleto' });
    verdade(c.colunas.some(x => x.nome === 'cidade'), 'a coluna «cidade» devia ter chegado ao servidor');
    verdade(!c.colunas.some(x => x.nome === ''), 'a linha vazia nao pode virar coluna');
    verdade(c.indices.some(i => i.primario), 'o indice primario do rascunho devia ter sido criado');
    // Gravou, voltou para a lista de tabelas do banco.
    await esperar(page, '#btNovaTab');

    // «← Tabelas de …»: volta SEM criar.
    await page.click('#btNovaTab');
    await esperar(page, '#nt_voltar');
    await page.fill('#nt_nome', 'NaoDeveExistir');
    await page.click('#nt_voltar');
    await esperar(page, '#btNovaTab');
    verdade(!(await existe(page, db, 'NaoDeveExistir')), 'o «Voltar» nao pode criar a tabela');
  },
};

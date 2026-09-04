/* O `WHERE` do `varrer` chegando na grade — provado EXERCITANDO.
 *
 * O que este caso trava, e por que ler o código não bastaria:
 *
 * 1. **o filtro DESCE** — filtrar `cidade = Blumenau` faz o servidor devolver
 *    as que casam, e não a página inteira. É a diferença que a sprint existe
 *    para comprar, e ela se mede no que ATRAVESSOU o fio
 *    (`__phxfonte.todos.length`), nunca no que a grade mostra: a grade mostra
 *    as mesmas linhas com pushdown e sem ele, e um caso que olhasse a tela
 *    passaria com o defeito reposto — que é pior que caso que falta;
 * 2. **o rodapé não mente** — «25 de 100.000» daria a entender que a tabela
 *    tem 25 linhas daquela cidade. A conta foi sobre as EXAMINADAS, e as três
 *    contas juntas são a única redação honesta;
 * 3. **rótulo se estiliza, dado nunca** — a célula diz «Blumenau», e não
 *    «BLUMENAU». É a armadilha do `label{text-transform:uppercase}`, e ela
 *    volta em todo componente novo da tela;
 * 4. **limpar o filtro volta ao que era** — sem isto o pushdown seria caminho
 *    de ida só, e a grade ficaria presa no conjunto menor.
 *
 * Nenhum número aqui é digitado: os dois lados saem do próprio servidor,
 * porque o teto de `max_linhas` da bateria muda o tamanho da página e um `25`
 * cravado envelheceria calado.
 */
import { entrar, capturar, api, verdade, igual, bancoDoCaso } from '../apoio.mjs';

const LINHAS = 2500;
const ONDE = [{ coluna: 'cidade', op: '=', valor: 'Blumenau' }];

export const caso = {
  nome: 'pushdown',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Pushdown');
    const tab = 'clientes';

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: tab,
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
        { nome: 'cidade', tipo: 'Str(30)' },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});

    // Uma em cada cem é de Blumenau: a seletividade real da tela.
    const ja = await api(page, 'varrer', { database: db, tabela: tab, max: 1 });
    if (!ja.registros) {
      const linhas = [];
      for (let i = 1; i <= LINHAS; i++) {
        linhas.push({ id: i, nome: `Cliente ${i}`, cidade: i % 100 === 0 ? 'Blumenau' : 'Itajai' });
      }
      await api(page, 'inserir_lote', { database: db, tabela: tab, linhas });
    }

    // Os dois lados, medidos no servidor. O `max` alto pede a página maior
    // que o `max_linhas` do config permitir — quem decide o tamanho é ele.
    const cheio = await api(page, 'varrer', { database: db, tabela: tab, max: 20000 });
    const peneirado = await api(page, 'varrer', { database: db, tabela: tab, max: 20000, onde: ONDE });
    igual(peneirado.examinadas, cheio.devolvidas,
      'o filtro tem de examinar a MESMA página -- `max` é linhas examinadas');
    verdade(peneirado.devolvidas > 0 && peneirado.devolvidas * 10 <= cheio.devolvidas,
      `o cenário não é seletivo: ${peneirado.devolvidas} de ${cheio.devolvidas}`);

    // A aba Conteúdo, pelo caminho da pessoa.
    await page.evaluate(([d, t]) => {
      est.aba = 'conteudo'; est.teto = 20000;
      return abrirTabela(d, t);
    }, [db, tab]);
    await page.waitForFunction(() => window.__phxfonte && window.__phxgrade, null, { timeout: 15000 });

    // ------------------------------------------- 1. SEM filtro, nada mudou
    const semFiltro = await page.evaluate(() => ({
      vieram: window.__phxfonte.todos.length,
      rodape: document.querySelector('#contaConteudo').textContent,
    }));
    igual(semFiltro.vieram, cheio.devolvidas,
      'sem filtro a grade tem de receber a página inteira, como sempre recebeu');
    verdade(!/examinadas|examined|geprüften|esaminate|examinées/.test(semFiltro.rodape),
      `sem filtro o rodapé não fala em examinadas: ${semFiltro.rodape}`);
    await capturar(ctx, ctx.nomeCaptura('sem-filtro'));

    // ------------------------------- 2. COM filtro: o servidor peneira
    await page.evaluate(() => window.__phxgrade.filtrar('cidade',
      { tipo: 'valores', valores: ['Blumenau'] }));
    await page.waitForFunction(n => window.__phxfonte.todos.length === n,
      peneirado.devolvidas, { timeout: 15000 });

    const comFiltro = await page.evaluate(() => ({
      vieram: window.__phxfonte.todos.length,
      rodape: document.querySelector('#contaConteudo').textContent,
      celulas: [...document.querySelectorAll('#grade tbody td')].map(c => c.textContent.trim()),
    }));
    igual(comFiltro.vieram, peneirado.devolvidas,
      'o servidor mandou o que a tela ia jogar fora -- o WHERE não desceu');
    verdade(/examinadas/.test(comFiltro.rodape),
      `o rodapé tem de dizer quantas foram examinadas: ${comFiltro.rodape}`);
    // Sem os separadores de milhar: o rodapé escreve «1.000» e o número é
    // 1000. Comparar a REDAÇÃO faria o caso quebrar no dia em que alguém
    // trocasse o separador -- e quebrar calado, dizendo outra coisa.
    const soDigitos = t => String(t).replace(/[.\u00a0\u202f,]/g, '');
    verdade(soDigitos(comFiltro.rodape).includes(String(cheio.devolvidas)),
      `o rodapé tem de trazer as ${cheio.devolvidas} examinadas: ${comFiltro.rodape}`);

    // Rótulo se estiliza; DADO, nunca. A célula diz «Blumenau».
    verdade(comFiltro.celulas.includes('Blumenau'),
      `a célula não mostra o dado como está gravado: ${JSON.stringify(comFiltro.celulas.slice(0, 8))}`);
    verdade(!comFiltro.celulas.includes('BLUMENAU'),
      'o CSS global comeu o dado: «Blumenau» apareceu em caixa alta');
    await capturar(ctx, ctx.nomeCaptura('com-filtro'));

    // ------------------------------------ 3. limpar o filtro volta ao que era
    await page.evaluate(() => window.__phxgrade.limparFiltros());
    await page.waitForFunction(n => window.__phxfonte.todos.length === n,
      cheio.devolvidas, { timeout: 15000 });
  },
};

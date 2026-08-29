/* A tela de Dado pessoal (LGPD/GDPR) AUDITA de verdade.
 *
 * O defeito que este caso trava: a tela procurava um campo booleano
 * `pessoal` que o servidor nunca mandou — o esquema responde `dado_pessoal`,
 * em texto («nao» / «pessoal» / «sensivel»). Com isso ela dizia, sempre e
 * para qualquer base, «o esquema deste servidor ainda não traz a marca», e
 * a lista de colunas marcadas ficava vazia mesmo com colunas marcadas.
 *
 * Uma tela de conformidade que responde «não sei» sobre um motor que sabe e
 * pior que uma tela ausente: a ausente ninguem cita num relatorio.
 *
 * Nenhum dos 1.106 testes de `cargo test` podia pegar isto: o servidor
 * estava certo dos dois lados (o campo no esquema E a op `dados_pessoais`),
 * e quem lia errado era a pagina. */
import { entrar, api, capturar, verdade, contem, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'lgpd',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Lgpd');

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: 'pacientes',
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'nome', tipo: 'Str(60)', dado_pessoal: 'pessoal' },
        { nome: 'cidade', tipo: 'Str(30)' },
        { nome: 'laudo', tipo: 'Memo', dado_pessoal: 'sensivel' },
      ],
      indices: [
        { nome: 'porId', colunas: ['id'], unico: true, primario: true },
        // Coluna pessoal TAMBEM indexada: a chave vai para o `.ndx` em claro,
        // e a tela precisa dizer isso — e o segundo lugar onde o dado existe.
        { nome: 'porNome', colunas: ['nome'], nocase: true },
      ],
    });

    await page.evaluate(d => telaDadosPessoais(d), db);
    await page.waitForSelector('#painel .fichas', { timeout: 15000 });
    await page.waitForTimeout(400);
    await capturar(ctx, ctx.nomeCaptura('dado-pessoal'));

    const texto = await page.textContent('#painel');
    verdade(!/ainda não traz a marca/.test(texto),
      'a tela continua dizendo que o servidor nao sabe marcar dado pessoal — '
      + 'ela esta lendo um campo que o servidor nao manda');

    const linhas = await page.$$eval('#painel .linha-lg', trs =>
      trs.map(tr => [...tr.querySelectorAll('td')].map(td => td.textContent.trim())));
    verdade(linhas.length === 2,
      `a tela listou ${linhas.length} coluna(s) marcada(s) — esperava 2 (nome e laudo)`);

    const achatado = JSON.stringify(linhas);
    contem(achatado, 'nome', 'a coluna «nome» (pessoal) nao apareceu na auditoria');
    contem(achatado, 'laudo', 'a coluna «laudo» (sensivel) nao apareceu na auditoria');
    contem(achatado, 'pessoal', 'o grau «pessoal» nao apareceu');
    contem(achatado, 'sensivel', 'o grau «sensivel» nao apareceu');
    contem(achatado, 'porNome',
      'a tela nao mostrou que a coluna pessoal tambem vive num indice — '
      + 'o `.ndx` e o segundo lugar onde o dado existe');

    // O numero que separa «ninguem olhou» de «alguem olhou e disse que nao».
    const fichas = await page.$$eval('#painel .fichas .ficha', ds =>
      ds.map(d => [d.querySelector('.r').textContent.trim(),
                   d.querySelector('.v').textContent.trim()]));
    const mapa = Object.fromEntries(fichas);
    verdade('sem classificação' in mapa,
      `a tela perdeu o numero de colunas sem classificacao: ${JSON.stringify(mapa)}`);
    verdade(mapa['colunas marcadas'] === '2',
      `«colunas marcadas» = ${mapa['colunas marcadas']}, esperava 2`);
    // `cidade` e `id` ficaram sem classificacao; `nome` e `laudo` nao.
    verdade(mapa['sem classificação'] === '2',
      `«sem classificação» = ${mapa['sem classificação']}, esperava 2`);

    // Clicar na linha leva para a tabela — a auditoria que aponta e melhor
    // que a auditoria que so lista.
    await page.click('#painel .linha-lg');
    await page.waitForTimeout(500);
    contem(await page.textContent('#titulo'), 'pacientes',
      'clicar na linha da auditoria nao abriu a tabela apontada');
  },
};

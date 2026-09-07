/* O indice de texto PELA TELA -- e este caso existe porque ler o codigo nao
 * acharia o buraco.
 *
 * Ate 07/09/2026 o `.fts` era inalcancavel de fora do Rust, e em TRES pecas,
 * nao uma. O pedido 200 dizia que faltava «o caminho pela tela»; medido
 * contra o motor vivo, faltava tambem o protocolo:
 *
 *   1. `criar_tabela` ENGOLIA `indices_texto` com `ok: true` -- a tabela
 *      nascia sem indice, e quem descobria era o `procurar_texto` depois;
 *   2. `esquema` nao reportava os indices de texto, entao a tela nao tinha
 *      como saber que a tabela sabe ser procurada por palavra;
 *   3. `procurar_texto` aparecia ZERO vezes na interface.
 *
 * O que so o navegador prova, e por isso este caso e de tela e nao unitario:
 * que o botao aparece EXATAMENTE onde ha indice, que a dobra de acento chega
 * ate a grade, e que o CSS global nao esticou os campos -- a primeira versao
 * desta tela saiu com o seletor e o campo de palavra ocupando 1.100px, por
 * causa do `input{width:100%}` que esta casa ja pagou duas vezes.
 */
import { entrar, api, verdade, igual, contem, capturar, bancoDoCaso } from '../apoio.mjs';

export const caso = {
  nome: 'fts-na-tela',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'fts');
    const COM = 'artigos', SEM = 'simples';
    await entrar(page, ctx.url);

    // A MESA POSTA vai pelo protocolo de proposito: o que este caso mede e a
    // TELA, e montar o cenario clicando gastaria a prova em preparo.
    await api(page, 'criar_database', { database: db });
    const cols = [{ nome: 'id', tipo: 'Int8' }, { nome: 'texto', tipo: 'Str(80)' }];
    const idx = [{ nome: 'pk', colunas: ['id'], unico: true }];
    await api(page, 'criar_tabela', { database: db, tabela: COM, colunas: cols,
                                      indices: idx, indices_texto: [{ nome: 'ft', coluna: 'texto' }] });
    await api(page, 'criar_tabela', { database: db, tabela: SEM, colunas: cols, indices: idx });
    for (const [id, texto] of [[1, 'a fenix voadora'], [2, 'a fênix renascida'],
                               [3, 'uma aguia parada'], [4, 'fenixes no plural']])
      await api(page, 'inserir', { database: db, tabela: COM, valores: { id, texto } });

    // 1. O formulario de Nova tabela sabe DECLARAR o indice.
    await page.evaluate(d => telaNovaTabela(d), db);
    await page.waitForSelector('#nt_addFts');
    igual(await page.$$eval('#nt_fts tr', t => t.length), 0,
          'a lista devia nascer VAZIA -- indice de texto e escolha, nao padrao');
    await page.click('#nt_addFts');
    await page.waitForSelector('#nt_fts tr');
    igual(await page.$$eval('#nt_fts tr', t => t.length), 1, 'o + nao acrescentou linha');
    verdade(await page.$eval('#nt_fts .f-dobra', e => e.checked),
            'a caixa da dobra devia vir MARCADA -- a tela nao pode discordar do motor em silencio');
    await capturar(ctx, ctx.nomeCaptura('fts-nova-tabela'));

    // 2. O botao SO existe onde ha indice. Botao que aparece sempre e recusa
    //    depois e pior que botao faltando: o primeiro so se descobre no meio
    //    do trabalho.
    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, SEM]);
    await page.waitForSelector('#btNova');
    verdade(!(await page.$('#btProcTexto')), 'o botao apareceu numa tabela SEM indice de texto');

    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, COM]);
    await page.waitForSelector('#btProcTexto');

    // 3. A busca, com a DOBRA DE ACENTO chegando ate a grade.
    await page.click('#btProcTexto');
    await page.waitForSelector('#ptPalavra');

    // O CSS global ja mordeu componente novo duas vezes nesta casa. Aqui ele
    // esticaria o campo de palavra pela largura toda, e so a medida pega.
    const larg = await page.$eval('#ptPalavra', e => e.getBoundingClientRect().width);
    verdade(larg < 500, `o campo de palavra ficou com ${Math.round(larg)}px -- o input{width:100%} global mordeu`);

    await page.fill('#ptPalavra', 'fenix');
    await page.click('#ptIr');
    await page.waitForSelector('#ptRecado .aviso');
    await page.waitForTimeout(400);
    contem(await page.textContent('#ptRecado'), '2',
           'devia achar DUAS: «a fenix voadora» e «a fênix renascida» -- a dobra de acento');
    igual(await page.$$eval('#ptGrade tbody tr', t => t.length), 2, 'a grade nao mostrou as duas');
    verdade(!(await page.textContent('#ptGrade')).includes('plural'),
            '«fenixes» entrou: o indice acha PALAVRA INTEIRA, nao pedaco');
    await capturar(ctx, ctx.nomeCaptura('fts-busca'));

    // 4. Palavra que nao existe devolve zero, e nao erro.
    await page.fill('#ptPalavra', 'jabuticaba');
    await page.click('#ptIr');
    await page.waitForTimeout(600);
    verdade(!(await page.textContent('#ptRecado')).includes('SP000'),
            'palavra ausente devolveu ERRO em vez de zero linhas');

    // 5. E o caminho de volta, que fecha o circuito.
    await page.click('#ptVoltar');
    await page.waitForSelector('#btNova');
  },
};

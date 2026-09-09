/* O direito por coluna, exercitado na TELA -- os achados 1, 2 e 3 da revisao
 * de tela de 09/09/2026 (`docs/cognicao/`, e o `CLAUDE.md` do front, "G5-TELA").
 *
 * O SERVIDOR ja fazia a parte dele: a coluna negada e mantida no valor
 * gravado (nunca zera, nunca recusa), e o `esquema` devolve
 * `colunas_sem_leitura`/`colunas_sem_alteracao`. O que faltava era o
 * FRONT-END usar isso -- e e exatamente isso que este caso prova, com um
 * usuario de verdade, numa aba de verdade, contra o servidor de verdade:
 *
 *  1. a ficha nao manda mais a coluna negada de graca (nem como `null`) --
 *     inclui e salva mexendo SO no que e permitido, nos dois fluxos (Incluir
 *     e Salvar), sem quebrar;
 *  2. a coluna sem LEITURA nao aparece -- nem cabecalho na grade, nem campo
 *     na ficha, nem linha na aba Estrutura;
 *  3. a coluna CALCULADA nasce `readonly` na ficha.
 *
 * O usuario nasce com `nivel: "operador"` e uma regra de coluna que nega
 * `ler` e `alterar` em `limite_credito` -- o mesmo par que a revisao usou
 * (`Comercial.clientes.limite_credito`), aqui numa tabela propria do caso
 * para nao disputar com nenhum outro. */
import {
  entrar, api, capturar, verdade, igual, bancoDoCaso, abrirLinhaDaGrade,
  abrirPelaArvore, CREDENCIAL,
} from '../apoio.mjs';

export const caso = {
  nome: 'direito-por-coluna',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    const db = bancoDoCaso(ctx, 'DireitoCol');
    const tab = 'clientes';
    // Um login por tema, pela mesma razao do `bancoDoCaso`: os dois temas
    // compartilham o MESMO servidor, e um login repetido reprovaria a
    // segunda corrida com "ja ha um usuario com este login".
    const login = `vendcol${ctx.tema === 'claro' ? 'c' : 'e'}`;
    const senha = 'provaDaBateria123';

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: tab,
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
        { nome: 'cidade', tipo: 'Str(30)' },
        { nome: 'limite_credito', tipo: 'Decimal(10,2)' },
        // A coluna CALCULADA do achado 3 -- mesmo molde da revisao
        // ("resumo = CONCAT(nome,' - ',cidade)" em Comercial.clientes).
        { nome: 'resumo', tipo: 'Str(80)', calculada: "CONCAT(nome,' - ',cidade)" },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});
    await api(page, 'inserir', {
      database: db, tabela: tab,
      valores: [1, 'Cliente Um', 'Blumenau', '5000.00', null],
    }).catch(() => {});

    // O usuario com direito por coluna -- idempotente, pelo mesmo motivo do
    // login por tema: uma corrida que reaproveita servidor nao pode
    // reprovar por causa de um usuario que a corrida anterior ja criou.
    await api(page, 'usuario_criar', {
      login, senha, nivel: 'operador', ativo: true, supervisor: false,
      bases: {
        [db]: {
          ler: true, inserir: true, alterar: true,
          tabelas: {
            [tab]: {
              ler: true, inserir: true, alterar: true,
              colunas: { limite_credito: { ler: false, alterar: false } },
            },
          },
        },
      },
    }).catch(() => {});

    // ---------------------------------------- entra como o usuario restrito
    // Uma aba propria: a sessao do supervisor (acima) nao pode ser a mesma
    // que testa a restricao.
    const v = await page.context().newPage();
    await entrar(v, ctx.url, { usuario: login, senha, token: CREDENCIAL.TOKEN });

    // -------------------------- achado 2 (estrutura): a aba Estrutura
    // `abrirPelaArvore` pousa na aba que `est.aba` ja tem -- e numa aba nova
    // o padrao E "estrutura" (`index.html`, "est.aba = est.aba || ...").
    await abrirPelaArvore(v, db, tab);
    await v.waitForSelector('#gradeEstColunas', { timeout: 15000 });
    await capturar(ctx, ctx.nomeCaptura('direito-coluna-estrutura'));
    const nomesEstrutura = await v.$$eval(
      '#gradeEstColunas [data-campo="nome"]',
      els => els.map(e => e.textContent.trim().toLowerCase()),
    );
    verdade(!nomesEstrutura.includes('limite_credito'),
      'limite_credito (sem leitura) apareceu na lista de colunas da aba Estrutura');

    // ------------------------------------------ achado 2 (grade): Conteudo
    await v.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await v.waitForSelector('#gradeEdit .phx-tabela', { timeout: 15000 });
    await capturar(ctx, ctx.nomeCaptura('direito-coluna-grade'));
    const cabecalhos = await v.$$eval(
      '#gradeEdit thead th[data-campo]',
      ths => ths.map(t => (t.getAttribute('data-campo') || '').toLowerCase()),
    );
    verdade(!cabecalhos.includes('limite_credito'),
      'limite_credito (sem leitura) apareceu como coluna da grade do vendedor');

    // ------------------------------- achados 1 e 3 (ficha): editar a linha 1
    await abrirLinhaDaGrade(v, { rowid: 1 });
    await v.waitForSelector('#fichaEdit');
    await capturar(ctx, ctx.nomeCaptura('direito-coluna-ficha'));
    igual(await v.locator('#f_limite_credito').count(), 0,
      'limite_credito (sem leitura) apareceu como campo da ficha do vendedor');
    igual(await v.getAttribute('#f_resumo', 'readonly'), '',
      'resumo (calculada) nao nasceu readonly na ficha');

    // O achado 1 de verdade: salvar mexendo SO no que e permitido nao quebra.
    await v.fill('#f_cidade', 'Joinville');
    await v.click('#btSalvar');
    await v.waitForSelector('#btNova', { timeout: 10000 });
    await v.waitForTimeout(250);
    const avisoSalvar = await v.evaluate(() => {
      const a = document.querySelector('#aviso:not([hidden])');
      return a ? { txt: a.textContent.trim(), mal: a.classList.contains('mal') } : null;
    });
    verdade(avisoSalvar && !avisoSalvar.mal,
      `salvar so a cidade, como vendedor, quebrou: ${avisoSalvar ? avisoSalvar.txt : 'nem aviso saiu'}`);

    const depoisDeSalvar = await api(page, 'ler', { database: db, tabela: tab, rowid: 1 });
    const linha1 = depoisDeSalvar.linha || depoisDeSalvar;
    igual(linha1.cidade, 'Joinville', 'a cidade alterada pelo vendedor nao chegou ao banco');
    igual(String(linha1.limite_credito), '5000.00',
      'o limite_credito mudou mesmo sem o vendedor ter mandado nada nele -- '
      + 'a ficha esta enviando a coluna negada de graca');

    // ------------------------------------------ achado 1, tambem no INCLUIR
    await v.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await v.waitForSelector('#btNova');
    await v.click('#btNova');
    await v.waitForSelector('#fichaEdit');
    igual(await v.locator('#f_limite_credito').count(), 0,
      'limite_credito apareceu na ficha de Nova linha do vendedor');
    await v.fill('#f_id', '2');
    await v.fill('#f_nome', 'Cliente Dois');
    await v.fill('#f_cidade', 'Itajai');
    await v.click('#btSalvar');
    await v.waitForSelector('#btNova', { timeout: 10000 });
    await v.waitForTimeout(250);
    const avisoIncluir = await v.evaluate(() => {
      const a = document.querySelector('#aviso:not([hidden])');
      return a ? { txt: a.textContent.trim(), mal: a.classList.contains('mal') } : null;
    });
    verdade(avisoIncluir && !avisoIncluir.mal,
      `incluir uma linha nova, como vendedor, quebrou: ${avisoIncluir ? avisoIncluir.txt : 'nem aviso saiu'}`);

    const listaFinal = await api(page, 'varrer', { database: db, tabela: tab, max: 50 });
    verdade((listaFinal.linhas || []).some(l => l.id === 2 && l.nome === 'Cliente Dois'),
      'a linha incluida pelo vendedor nao chegou na tabela');

    await v.close();
  },
};

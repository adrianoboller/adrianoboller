/* Quem tem direito numa base so ENTRA e ve a arvore dela (pedido 775), e o
 * excluir em lote diz POR QUE falhou (pedido 777).
 *
 * O defeito do 775, medido no Chromium pela auditoria tela x servidor de
 * 09/10/2026: o `bancos` exigia `ler` na base VAZIA, quem so tinha regra na
 * `loja` levava recusa, a excecao derrubava o `abrirApp` e a pessoa entrava
 * numa tela com ZERO nos na arvore e o recado preso na tela de login ja
 * escondida. O conserto mora nos dois lados -- o servidor filtra o `bancos`
 * pela ficha (como o `tabelas`), e a tela trata a recusa em vez de engolir --,
 * e este caso prova o que a pessoa ve:
 *
 *  1. o login termina (`data-pronto`) e a arvore tem a base DELA e a tabela;
 *  2. a outra base, onde ela nao tem direito, NAO aparece;
 *  3. a tabela abre pela arvore;
 *  4. o excluir em lote, sem o direito de excluir, mostra o motivo que o
 *     servidor deu -- e nao so «N falharam».
 *
 * O usuario nasce SEM nivel (o «sem nivel, nega tudo» do catalogo): com nivel
 * de leitor ele leria toda base pelo nivel e o caso passaria por engano. */
import {
  entrar, api, capturar, verdade, igual, bancoDoCaso, abrirPelaArvore, assentar,
  CREDENCIAL, Falha,
} from '../apoio.mjs';

export const caso = {
  nome: 'usuario-de-uma-base',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    const minha = bancoDoCaso(ctx, 'UmaBase');
    const alheia = bancoDoCaso(ctx, 'BaseAlheia');
    const tab = 'clientes';
    // Um login por tema: os dois temas dividem o MESMO servidor.
    const login = `umabase${ctx.tema === 'claro' ? 'c' : 'e'}`;
    const senha = 'provaDaBateria775';

    for (const db of [minha, alheia]) {
      await api(page, 'criar_database', { database: db }).catch(() => {});
      await api(page, 'criar_tabela', {
        database: db, tabela: tab,
        colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true },
                  { nome: 'nome', tipo: 'Str(30)' }],
        indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
      }).catch(() => {});
    }
    for (const id of [1, 2, 3]) {
      await api(page, 'inserir', { database: minha, tabela: tab, valores: [id, `linha ${id}`] })
        .catch(() => {});
    }
    // Ler, incluir e alterar na base dela; EXCLUIR nao -- e a recusa que o
    // passo 4 precisa para ter motivo a mostrar.
    await api(page, 'usuario_criar', {
      login, senha, ativo: true, supervisor: false,
      bases: { [minha]: { ler: true, inserir: true, alterar: true } },
    }).catch(() => {});

    // --------------------------------------------- 1. a entrada termina
    // Pela mao, e nao pelo `entrar` do apoio: com o defeito reposto o
    // `entrar` morreria num «timeout esperando seletor», e a reprova tem de
    // dizer O QUE a pessoa viu -- quantos nos e qual recado.
    const v = await page.context().newPage();
    await v.goto(ctx.url, { waitUntil: 'domcontentloaded' });
    await v.waitForSelector('#btEntrar');
    await v.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });
    await v.fill('#u', login);
    await v.fill('#s', senha);
    await v.fill('#t', CREDENCIAL.TOKEN);
    await v.click('#btEntrar');
    try {
      await v.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 20000 });
    } catch {
      const visto = await v.evaluate(() => ({
        ativo: !!document.querySelector('#app.ativo'),
        nos: document.querySelectorAll('#arvore .no').length,
        recado: (document.querySelector('#recado') || {}).textContent || '',
      }));
      throw new Falha(`quem tem direito so em ${minha} nao terminou de entrar: app ativo=${
        visto.ativo}, ${visto.nos} no(s) na arvore, recado «${visto.recado.trim()}»`);
    }
    await capturar({ ...ctx, page: v }, ctx.nomeCaptura('usuario-de-uma-base'));

    const bases = await v.$$eval('#arvore .no.db', ns => ns.map(n => n.dataset.db));
    verdade(bases.includes(minha),
      `a base ${minha}, onde ${login} tem direito, nao apareceu na arvore: [${bases.join(', ')}]`);
    verdade(await v.locator(`#arvore .no.tab[data-db="${minha}"][data-tab="${tab}"]`).count() === 1,
      `a tabela ${minha}.${tab} nao apareceu na arvore`);

    // ------------------------------------------- 2. a base alheia nao
    verdade(!bases.includes(alheia),
      `a base ${alheia}, onde ${login} NAO tem direito, apareceu na arvore`);

    // ---------------------------------------------- 3. a tabela abre
    await abrirPelaArvore(v, minha, tab);
    igual((await v.textContent('#titulo')).trim(), tab, 'a tabela nao abriu pela arvore');

    // --------------------------------- 4. o lote diz o motivo (777)
    // O motivo esperado sai do SERVIDOR, pelo mesmo pedido, e nao de uma
    // frase digitada aqui: a redacao pode mudar e o caso continua valendo.
    // Dentro da pagina, para vir o `message` cru do erro, sem o embrulho do
    // `page.evaluate`.
    const motivo = await v.evaluate(([d, t]) =>
      api('excluir', { database: d, tabela: t, rowid: 1 }).then(() => null, e => e.message),
    [minha, tab]);
    verdade(motivo, `${login} excluiu sem ter o direito de excluir`);
    await v.click('.aba[data-aba="conteudo"]');
    await v.waitForSelector('#grade tbody .phx-td-sel input', { timeout: 15000 });
    await v.locator('#grade tbody .phx-td-sel input').nth(0).check();
    await v.locator('#grade tbody .phx-td-sel input').nth(1).check();
    await v.waitForSelector('#acoesGrade:not([hidden])', { timeout: 5000 });
    v.once('dialog', d => d.accept());
    await v.click('#btExcluirSel');
    await v.waitForSelector('#aviso.mal:not([hidden])', { timeout: 10000 });
    await assentar(v, 300);
    const aviso = (await v.textContent('#aviso')).trim();
    verdade(aviso.includes(motivo),
      `o excluir em lote recusado nao disse o motivo.\n      aviso: «${aviso}»\n      motivo do servidor: «${motivo}»`);

    // ------------------- 5. o 4009 no lote: UM dialogo, e o motivo (777)
    // O `excluir` suave nao e da lista de perigo, entao o 4009 vem forjado no
    // fio (`page.route`), com o envelope do servidor. A pessoa CANCELA: o
    // lote tem de parar no primeiro (um dialogo so, e nao um por linha) e o
    // aviso tem de dizer o motivo.
    const MSG_4009 = '[SP000099] este comando exige a senha de execucao (prova da bateria)';
    let pedidosExcluir = 0;
    await v.route('**/api', async rota => {
      let corpo = {};
      try { corpo = JSON.parse(rota.request().postData() || '{}'); } catch { /* nao e JSON */ }
      if (corpo.op !== 'excluir') return rota.continue();
      pedidosExcluir++;
      await rota.fulfill({ status: 200, contentType: 'application/json',
        body: JSON.stringify({ ok: false, op: 'excluir', codigo: 4009,
          nome: 'SENHA_DE_EXECUCAO_EXIGIDA', classe: 'acesso', repetir: false, erro: MSG_4009 }) });
    });
    await v.locator('#grade tbody .phx-td-sel input').nth(0).check();
    await v.locator('#grade tbody .phx-td-sel input').nth(1).check();
    await v.locator('#grade tbody .phx-td-sel input').nth(2).check();
    await v.waitForSelector('#acoesGrade:not([hidden])', { timeout: 5000 });
    v.once('dialog', d => d.accept());
    await v.click('#btExcluirSel');
    await v.waitForSelector('#dlgSenhaExecucao', { timeout: 10000 });
    await v.click('#seNao');
    await v.waitForSelector('#aviso.mal:not([hidden])', { timeout: 10000 });
    await assentar(v, 600);
    const dialogoDeNovo = await v.locator('#dlgSenhaExecucao').count();
    igual(dialogoDeNovo, 0, 'o dialogo da senha de execucao abriu de novo depois do cancelar');
    igual(pedidosExcluir, 1,
      `o lote seguiu pedindo depois do 4009 cancelado: ${pedidosExcluir} pedidos de excluir`);
    const aviso4009 = (await v.textContent('#aviso')).trim();
    verdade(aviso4009.includes(MSG_4009),
      `o lote parado pelo 4009 nao disse o motivo: «${aviso4009}»`);
    await v.unroute('**/api');

    await v.close();
    ctx.notas.push(`${login}: arvore com [${bases.join(', ')}], lote recusado com o motivo`);
  },
};

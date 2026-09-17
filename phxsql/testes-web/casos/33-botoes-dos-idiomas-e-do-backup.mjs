/* Os botoes da tela de IDIOMAS e da de BACKUP E RESTAURACAO.
 *
 * Dez botoes que ninguem tinha clicado, e os dois lotes tem a mesma forma: o
 * botao abre uma caixa (o `perguntarTexto`, o seletor de arquivo, o
 * `confirm`) e so DEPOIS faz o servico. Quem so espera o seletor aparecer
 * prova o desenho e nao prova a acao.
 *
 * Os quatro da tela de idiomas sao do meu proprio quintal: e por eles que a
 * tabela `phxsys.mensagens` se semeia, se exporta, se importa e se devolve ao
 * texto de fabrica. O «Carga padrao» APAGA TRABALHO -- por isso ele e o
 * ultimo, e por isso o caso confere o que ele devolveu.
 *
 * O importar abre uma janela de arquivo do sistema. Essa o Playwright SABE
 * abrir (`filechooser`) -- ao contrario da janela destacada do
 * `window-management`, que e a limitacao ja registrada em DISPENSADOS. O
 * arquivo que entra e o backup que a propria tela acabou de exportar, pelo
 * mesmo caminho do protocolo: importar um arquivo inventado provaria o leitor
 * de JSON, nao a tela.
 */
import { entrar, api, verdade, capturar, bancoDoCaso } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 15000 });

export const caso = {
  nome: 'botoes-dos-idiomas-e-do-backup',
  async rodar(ctx) {
    const { page } = ctx;
    const db = bancoDoCaso(ctx, 'idi');
    await entrar(page, ctx.url);
    await api(page, 'criar_database', { database: db }).catch(() => {});

    // ---------------------------------------------------------- IDIOMAS
    await page.evaluate(() => abrirAdmin('idiomas'));
    await esperar(page, '#btIdiCarga');
    await capturar(ctx, ctx.nomeCaptura('idiomas'));

    // «Carga da tabela» semeia o que falta. A prova e a ficha «semeados»
    // fechar: depois da carga nao pode faltar texto nenhum.
    await page.click('#btIdiCarga');
    await esperar(page, '#btIdiCarga');
    await page.waitForTimeout(600);
    const semeados = await page.$$eval('#painel .ficha', fs =>
      fs.map(f => `${f.querySelector('.v')?.textContent} ${f.querySelector('.u')?.textContent}`).join(' | '));
    verdade(/nada a semear/i.test(semeados),
      `depois da carga nao devia faltar texto; as fichas dizem «${semeados}»`);

    // «Exportar backup» entrega um arquivo ao navegador e diz quantas linhas.
    await page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.textContent = ''; });
    await page.click('#btIdiExportar');
    await page.waitForTimeout(1200);
    const recadoExp = await page.evaluate(() => document.querySelector('#aviso')?.textContent || '');
    verdade(recadoExp.trim().length > 0, 'o Exportar devia dizer quantas linhas entregou');

    // «Importar backup» -- o seletor de arquivo do sistema, e o `confirm`
    // depois dele. Sem aceitar os dois, o clique nao prova efeito nenhum.
    const backup = await api(page, 'idiomas_exportar', {});
    page.once('dialog', d => d.accept());
    const [escolhedor] = await Promise.all([
      page.waitForEvent('filechooser', { timeout: 10000 }),
      page.click('#btIdiImportar'),
    ]);
    await escolhedor.setFiles({
      name: 'idiomas.json', mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(backup)),
    });
    await esperar(page, '#btIdiCarga');
    await page.waitForTimeout(800);
    const recadoImp = await page.evaluate(() => document.querySelector('#aviso')?.textContent || '');
    verdade(/import/i.test(recadoImp) || recadoImp.trim().length > 0,
      `o Importar devia dizer o que entrou; veio «${recadoImp}»`);

    // O DESTRUTIVO, no fim: devolve o texto de fabrica de UM idioma so.
    // O escopo sai do IDIOMAS da propria pagina -- escrever «Ingles» aqui
    // amarraria o caso a lista, e a lista e de la.
    const escopo = await page.evaluate(() => IDIOMAS[2].col);
    await page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.textContent = ''; });
    await page.selectOption('#idiEscopo', escopo);
    page.once('dialog', d => d.accept());
    await page.click('#btIdiPadrao');
    await esperar(page, '#btIdiCarga');
    await page.waitForTimeout(800);
    const recadoPad = await page.evaluate(() => document.querySelector('#aviso')?.textContent || '');
    verdade(recadoPad.includes(escopo),
      `a carga padrao devia nomear o escopo «${escopo}»; veio «${recadoPad}»`);

    // ------------------------------------------------ BACKUP E RESTAURACAO
    await page.evaluate(d => verBackupRestaure(d), db);
    await esperar(page, '#opBackup');
    await capturar(ctx, ctx.nomeCaptura('backup'));

    // «Backup agora» pergunta o database numa caixa da propria tela
    // (`perguntarTexto`), nao num `prompt` do navegador.
    await page.click('#opBackup');
    await esperar(page, '#ptSim');
    await page.fill('#ptTexto', db);
    await page.click('#ptSim');
    // Espera CURTA e afirmacao que nomeia o gancho: o defeito de 17/09/2026
    // deixava a folha em «rodando…» para sempre, e um `waitForSelector` seco
    // reprovaria dizendo «timeout esperando pre.dado» -- a causa certa com o
    // nome errado. Aqui a reprova diz qual botao nao entregou.
    await page.waitForSelector('#painel pre.dado', { timeout: 8000 }).catch(() => {});
    const saida = await page.$eval('#painel pre.dado', e => e.textContent).catch(() => '');
    verdade(/destino/.test(saida),
      `o «Backup agora» (#opBackup) nao entregou a copia; a folha diz «${
        (await page.$eval('#painel', e => e.textContent)).trim().slice(0, 120)}»`);
    const destino = JSON.parse(saida).destino;
    verdade(!!destino, 'o backup devia dizer para onde copiou');

    // «Conferir uma copia» recalcula o SHA-256 contra o manifesto do backup
    // que acabou de sair -- e por isso ele confere.
    await page.evaluate(d => verBackupRestaure(d), db);
    await esperar(page, '#opConferir');
    await page.click('#opConferir');
    await esperar(page, '#ptSim');
    await page.fill('#ptTexto', destino);
    await page.click('#ptSim');
    await esperar(page, '#painel pre.dado');
    // ACHADO AO ESCREVER ESTE PASSO: as tres fichas desta folha pediam
    // `conferidos`, `diferentes` e `faltando`, e a resposta traz `arquivos`,
    // `bytes` e `divergencias` -- as tres saiam «—», com o numero medido
    // existindo no `<pre>` logo abaixo. Consertado no mesmo trabalho; a
    // afirmacao abaixo olha as FICHAS, porque e nelas que o defeito morava:
    // conferir so o `<pre>` passaria verde com os tres tracos de volta.
    const conf = JSON.parse(await page.$eval('#painel pre.dado', e => e.textContent));
    verdade(conf.integro === true && conf.arquivos > 0,
      `a copia recem-feita devia conferir; veio ${JSON.stringify(conf).slice(0, 140)}`);
    const fichasConf = await page.$$eval('#painel .fichas .ficha', fs =>
      fs.map(f => f.querySelector('.v')?.textContent));
    verdade(fichasConf.length > 0 && !fichasConf.includes('—'),
      `as fichas da conferencia nao podem sair vazias; vieram ${JSON.stringify(fichasConf)}`);

    // «Restaurar…» e a tela de procurar copias -- e ela acha a que saiu agora.
    await page.evaluate(d => verBackupRestaure(d), db);
    await esperar(page, '#opRestaurar');
    await page.click('#opRestaurar');
    await esperar(page, '#btRstListar');
    await page.fill('#rstPasta', destino.replace(/\/[^/]+$/, ''));
    await page.click('#btRstListar');
    await page.waitForTimeout(1200);
    const lista = await page.$eval('#rstLista', e => e.textContent);
    verdade(lista.trim().length > 0, 'o «Procurar copias» devia escrever algo na lista');

    await page.click('#btRstVolta');
    await esperar(page, '#opBackup');

    // O `#btRstVolta` reabre a folha com o `databaseCorrente()`, que pode nao
    // ser o banco deste caso -- e ai o `#btVoltaDb` cai no «nenhum database»
    // e nao navega. Reabre-se nomeando o banco para provar a NAVEGACAO, que e
    // o que este botao promete.
    await page.evaluate(d => verBackupRestaure(d), db);
    await esperar(page, '#opBackup');
    await page.click('#btVoltaDb');
    await esperar(page, '#painel [data-op]');
    verdade(!(await page.$('#opBackup')), 'o «← Gerir» devia sair da tela de backup');
  },
};

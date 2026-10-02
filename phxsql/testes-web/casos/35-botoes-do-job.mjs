/* Os botoes da FICHA DO JOB (`editarJob`): criar, gravar, rodar agora, excluir
 * e voltar -- quatro que ninguem tinha clicado (pedido 190):
 *
 *   #btJbGravar  #btJbRodar  #btJbApagar  #btJbVolta
 *
 * A prova e SEMPRE no servidor, pela operacao `jobs`: o que a tela disse
 * («job gravado») e o que o cadastro tem sao duas coisas, e ja houve tela
 * desta casa que anunciava o que o motor nao tinha feito. Cada passo confere
 * o EFEITO: o job que nasceu desligado (de proposito), a descricao que mudou,
 * a corrida que entrou no historico, o job que sumiu -- e o que NAO pode
 * acontecer: o «Voltar» nao grava, e o «Excluir» recusado no `confirm` nao
 * apaga.
 */
import { entrar, api, verdade, igual, capturar, bancoDoCaso } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 15000 });

export const caso = {
  nome: 'botoes-do-job',
  async rodar(ctx) {
    const { page } = ctx;
    // O nome do job e unico no SERVIDOR, e a bateria roda os dois temas.
    const nome = bancoDoCaso(ctx, 'job').toLowerCase() + '_ping';
    await entrar(page, ctx.url);
    const achar = async () => ((await api(page, 'jobs', {})).jobs || []).find(j => j.nome === nome);

    // ----------------------------------------------------------- criar
    await page.evaluate(() => telaJobs());
    await esperar(page, '#btJobNovo');
    await page.click('#btJobNovo');
    await esperar(page, '#btJbGravar');
    await capturar(ctx, ctx.nomeCaptura('ficha-do-job-novo'));

    // Ficha nova: sem «Rodar agora» nem «Excluir» -- nao ha o que rodar.
    verdade(!(await page.$('#btJbRodar')) && !(await page.$('#btJbApagar')),
      'a ficha de um job NOVO nao devia oferecer Rodar agora nem Excluir');

    // O «Voltar» sem gravar nao cria nada.
    await page.fill('#jbNome', nome);
    await page.click('#btJbVolta');
    await esperar(page, '#btJobNovo');
    verdade(!(await achar()), 'o «← Jobs» nao pode gravar o job');

    // Gravar com pedido que nao e JSON: recusa na tela, antes do servidor.
    await page.click('#btJobNovo');
    await esperar(page, '#btJbGravar');
    await page.fill('#jbNome', nome);
    await page.fill('#jbPedido', '{ isto nao e json');
    await page.click('#btJbGravar');
    await page.waitForFunction(() => /JSON/i.test(document.querySelector('#aviso')?.textContent || ''), undefined, { timeout: 8000 });
    verdade(await page.$('#btJbGravar'), 'a ficha devia continuar aberta depois da recusa');
    verdade(!(await achar()), 'pedido invalido nao pode gravar job');

    // Gravar de verdade. O job nasce DESLIGADO (a ficha diz isso no subtitulo)
    // e o campo «A cada» manda, porque «Quando» ficou em «a cada N minutos».
    await page.fill('#jbPedido', '{"op":"ping"}');
    await page.fill('#jbDesc', 'job da bateria');
    await page.fill('#jbCada', '45');
    await page.click('#btJbGravar');
    await esperar(page, '#btJobNovo');           // voltou para a lista
    const criado = await achar();
    verdade(!!criado, 'o job devia existir no cadastro depois de gravar');
    igual(criado.cada_minutos, 45, 'a agenda gravada');
    igual(!!criado.ligado, false, 'o job novo nasce desligado, de proposito');
    igual(criado.descricao, 'job da bateria', 'a descricao gravada');

    // ----------------------------------------------- editar uma ficha existente
    await page.click(`.bt-job[data-n="${nome}"]`);
    await esperar(page, '#btJbRodar');
    igual(await page.$eval('#jbNome', el => el.disabled), true, 'o nome do job existente nao muda');
    await capturar(ctx, ctx.nomeCaptura('ficha-do-job-existente'));

    // Gravar alteracoes: a descricao muda e a agenda tambem, mudando o «Quando».
    await page.fill('#jbDesc', 'descricao nova');
    await page.selectOption('#jbQuando', 'hora');
    await page.fill('#jbHora', '03:30');
    await page.click('#btJbGravar');
    await esperar(page, '#btJobNovo');
    const mudado = await achar();
    igual(mudado.descricao, 'descricao nova', 'a descricao depois de gravar alteracoes');
    igual(mudado.hora, '03:30', 'a agenda por hora depois de gravar alteracoes');

    // ------------------------------------------------------------ rodar agora
    const corridasAntes = ((await api(page, 'jobs', { historico: 200 })).historico || []).filter(h => h.job === nome).length;
    await page.click(`.bt-job[data-n="${nome}"]`);
    await esperar(page, '#btJbRodar');
    await page.click('#btJbRodar');
    await esperar(page, '#btJobNovo');           // rodarJob devolve a lista
    const depois = ((await api(page, 'jobs', { historico: 200 })).historico || []).filter(h => h.job === nome);
    verdade(depois.length === corridasAntes + 1,
      `«Rodar agora» devia somar UMA corrida ao historico: eram ${corridasAntes}, ficaram ${depois.length}`);
    verdade(depois.every(h => h.ok), 'o ping devia ter dado certo');

    // ------------------------------------------------------------ excluir
    // Primeiro RECUSANDO o confirm: o job tem de continuar la.
    await page.click(`.bt-job[data-n="${nome}"]`);
    await esperar(page, '#btJbApagar');
    page.once('dialog', d => d.dismiss());
    await page.click('#btJbApagar');
    await page.waitForTimeout(500);
    verdade(!!(await achar()), 'recusar o confirm nao pode excluir o job');
    verdade(await page.$('#btJbApagar'), 'a ficha devia continuar aberta depois de recusar');
    // Agora aceitando.
    page.once('dialog', d => d.accept());
    await page.click('#btJbApagar');
    await esperar(page, '#btJobNovo');
    verdade(!(await achar()), 'o job devia ter sumido do cadastro depois do Excluir');
  },
};

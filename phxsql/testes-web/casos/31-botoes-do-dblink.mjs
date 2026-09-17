/* Os botoes do DBLINK: a tela vazia, a tela cheia, as definicoes, a consulta
 * e o assistente.
 *
 * O pedido 190 punha o assistente de DbLink inteiro na fila do «pede outro
 * servidor». Medido em 17/09/2026, o que pede um MySQL(R) de verdade e o
 * trecho depois do «Testar a conexao» dar certo -- o passo 1 grava a
 * definicao aqui, sem sair da maquina, e o passo 2 renderiza o ramo «Nao
 * conectou» com o proprio `← Corrigir a conexao`. Oito dos catorze botoes do
 * assistente saem disso.
 *
 * E EXERCITAR ACHOU O DEFEITO QUE LER NAO ACHOU: na tela de DbLink SEM
 * ligacao nenhuma -- a primeira que qualquer pessoa ve -- os dois unicos
 * botoes nasciam MORTOS. O `return folha(...)` deixava as duas linhas de
 * `onclick` inalcancaveis, e o clique nao fazia nada: uma tela sem saida.
 * Este caso confere os dois, e e por isso que ele comeca pela tela vazia.
 *
 * A ORDEM E OBRIGATORIA. A tela vazia so existe enquanto nao ha ligacao
 * cadastrada, e a bateria roda o caso DUAS vezes (um tema cada) contra o
 * MESMO servidor. Por isso o caso limpa o que achar antes de comecar e apaga
 * a ligacao no fim, pelo botao de excluir -- que e destrutivo e por isso vai
 * por ultimo, com o efeito conferido.
 */
import { entrar, api, verdade, capturar } from '../apoio.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 10000 });

/* Um apelido por tema: os dois temas falam com o mesmo servidor, e a lista de
 * ligacoes e do SERVIDOR (nao ha uma por database). */
const apelido = ctx => (ctx.tema === 'claro' ? 'dblC' : 'dblE');

export const caso = {
  nome: 'botoes-do-dblink',
  async rodar(ctx) {
    const { page } = ctx;
    const lig = apelido(ctx);
    await entrar(page, ctx.url);

    // Terreno limpo: sem isto, uma corrida anterior interrompida deixaria a
    // ligacao de pe e a TELA VAZIA nunca apareceria -- e o caso passaria sem
    // exercitar justamente os dois botoes que motivaram este arquivo.
    const antes = await api(page, 'dblink', {});
    for (const l of antes.ligacoes || []) {
      await api(page, 'dblink_excluir', { nome: l.nome }).catch(() => {});
    }

    // ------------------------------------------- 1. a tela SEM ligacao
    await page.evaluate(() => telaDbLink());
    await esperar(page, '#btAssist0');
    await capturar(ctx, ctx.nomeCaptura('dblink-vazia'));

    // «Cadastrar a mao…» tem de LEVAR a algum lugar. Era este o botao morto.
    // A afirmacao nomeia o GANCHO de proposito: repondo o defeito, a reprova
    // tem de dizer qual botao morreu, e nao «timeout esperando um seletor».
    await page.click('#btDef');
    await page.waitForTimeout(900);
    verdade(!!(await page.$('#btNovaLig')),
      'o «Cadastrar a mao…» (#btDef) da tela vazia de DbLink nao abriu as Definicoes');

    await page.evaluate(() => telaDbLink());
    await esperar(page, '#btAssist0');
    await page.click('#btAssist0');
    await page.waitForTimeout(900);
    verdade(!!(await page.$('#azIr1')),
      'o «Assistente de conexao…» (#btAssist0) da tela vazia de DbLink nao abriu o assistente');

    // -------------------------------------- 2. o assistente, passos 1 e 2
    verdade(!!(await page.$('#azSair')), 'o passo 1 do assistente devia trazer o Cancelar');
    await page.fill('#azNome', lig);
    await page.fill('#azHost', '127.0.0.1');
    // Porta escolhida por NAO ter servico: o passo 2 tem de cair no ramo «Nao
    // conectou», que e o que esta maquina consegue provar.
    await page.fill('#azPorta', '3399');
    await page.fill('#azUsu', 'ninguem');
    await page.click('#azIr1');
    await esperar(page, '#azVolta');
    const naoConectou = await page.$eval('.sobre .caixa', e => e.textContent);
    verdade(/conect/i.test(naoConectou), `o passo 2 devia falar da conexao; veio «${naoConectou.slice(0, 80)}»`);

    await page.click('#azVolta');
    await esperar(page, '#azIr1');   // voltou ao passo 1
    await page.click('#azSair');
    await page.waitForTimeout(300);
    verdade(!(await page.$('.sobre')), 'o Cancelar devia fechar o assistente');

    // --------------------------------------- 3. a tela COM a ligacao
    // O passo 1 do assistente ja gravou a definicao: a tela deixa de ser a
    // vazia, e e essa mudanca que prova que o `#azIr1` fez alguma coisa.
    await page.evaluate(n => telaDbLink(n), lig);
    await esperar(page, '#btAssistente');
    verdade(!(await page.$('#btAssist0')), 'com ligacao cadastrada a tela vazia nao devia aparecer');
    await capturar(ctx, ctx.nomeCaptura('dblink-cheia'));

    // «Testar» sem o MySQL do outro lado tem de DIZER que nao conectou.
    await page.click('#btTestar');
    await page.waitForTimeout(1500);
    const avisoT = await page.evaluate(() =>
      (document.querySelector('#avisoDbl') || {}).textContent || '');
    verdade(avisoT.trim().length > 0, 'o Testar devia escrever o resultado em #avisoDbl');

    await page.click('#btSql');
    await esperar(page, '#btRodar');
    await page.click('#btRodar');
    await page.waitForTimeout(1800);
    const saidaSql = await page.$eval('#painel', e => e.textContent);
    verdade(/conect|erro|Error/i.test(saidaSql), 'o Executar devia dizer que nao alcancou o outro banco');
    await page.click('#btVoltaDbl');
    await esperar(page, '#btAssistente');

    await page.click('#btAssistente');
    await esperar(page, '#azIr1');
    await page.mouse.click(4, 4);           // o assistente e um `.sobre`: fecha-se pelo fundo
    await page.waitForTimeout(300);

    await page.evaluate(n => telaDbLink(n), lig);
    await esperar(page, '#btDef');
    await page.click('#btDef');
    await esperar(page, '#btIrDbl');

    // ------------------------------------------- 4. definicoes e edicao
    await page.click('#btNovaLig');
    await esperar(page, '#btGravar');
    verdade(!(await page.$('#btApagar')), 'ligacao NOVA nao tem o que excluir');
    await page.click('#btVolta2');
    await esperar(page, '#btIrDbl');

    await page.click('#btIrDbl');
    await esperar(page, '#btAssistente');

    await page.evaluate(n => editarDbLink(n), lig);
    await esperar(page, '#btApagar');
    await page.fill('#fDesc', 'ligacao da bateria');
    await page.click('#btGravar');
    await esperar(page, '#btIrDbl');      // gravou e voltou as definicoes
    const gravou = await api(page, 'dblink', {});
    verdade((gravou.ligacoes || []).some(l => l.nome === lig && l.descricao === 'ligacao da bateria'),
      'o Gravar devia ter guardado a descricao');

    await page.evaluate(n => editarDbLink(n), lig);
    await esperar(page, '#btTest2');
    await page.click('#btTest2');          // grava e TENTA conectar: tem de reclamar
    await page.waitForTimeout(1800);

    // ---------------------------------------------- 5. o destrutivo, no fim
    // `#btApagar` pergunta antes. Aceitar o dialogo e o que a pessoa faz; sem
    // isto o Playwright o recusa sozinho e o clique nao prova efeito nenhum.
    await page.evaluate(n => editarDbLink(n), lig);
    await esperar(page, '#btApagar');
    page.once('dialog', d => d.accept());
    await page.click('#btApagar');
    await esperar(page, '#btIrDbl');
    const depois = await api(page, 'dblink', {});
    verdade(!(depois.ligacoes || []).some(l => l.nome === lig),
      `a ligacao ${lig} devia ter sumido do cadastro`);
  },
};

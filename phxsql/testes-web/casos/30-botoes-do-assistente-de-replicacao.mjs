/* Os botoes do ASSISTENTE DE REPLICACAO -- e a medida de onde ele para sem
 * um segundo servidor.
 *
 * O pedido 190 dizia que o assistente «pede outro servidor» e o punha na fila
 * do que nao da nesta maquina. Medido em 17/09/2026, a frase valia para menos
 * do que parecia: o que pede outro servidor e o TRECHO DEPOIS da sonda voltar
 * limpa. A navegacao inteira -- escolher o modo, avancar, voltar, sair -- e o
 * proprio TESTE DE CONEXAO sao clicaveis aqui, porque o servidor da bateria
 * sabe sondar a si mesmo: `replicacao_testar` e um cliente do protocolo, e
 * 127.0.0.1 na porta de dados dela e um servidor PhxSql de verdade do outro
 * lado do soquete.
 *
 * Doze dos dezenove botoes do assistente saem disso (`#rzVolta` aparece oito
 * vezes, e a chave e uma so). Os seis que sobram -- `#rzIr3` em diante --
 * ficam atras de uma sonda SEM IMPEDIMENTO, e o impedimento e medido, nao
 * suposto: o servidor da bateria nao declara `id_servidor` nem
 * `imagem_da_linha`, entao a propria sonda o reprova como origem. Eles estao
 * em DISPENSADOS com esse motivo.
 *
 * MENCIONAR NAO E CLICAR: cada passo aqui clica e depois confere que a tela
 * mudou. Para o botao que se repinta no MESMO molde (`#rzDeNovo` refaz o
 * passo 3), a prova e a marca posta antes do clique: se o corpo foi refeito,
 * a marca sumiu junto.
 */
import { entrar, verdade, capturar } from '../apoio.mjs';
import { USUARIO, SENHA, TOKEN } from '../servidor.mjs';

const esperar = (page, sel) => page.waitForSelector(sel, { timeout: 10000 });

/* Marca o corpo do dialogo e devolve a pergunta «foi refeito?».
 *
 * Serve ao clique que redesenha a MESMA tela: sem isto, «continua havendo um
 * #rzDeNovo» passaria mesmo com o botao morto, que e exatamente o defeito que
 * este pedido existe para achar. */
async function marcarCorpo(page) {
  await page.evaluate(() => {
    const c = document.querySelector('#rzCorpo h3');
    if (c) c.dataset.marcaDaProva = '1';
  });
  return async () => await page.evaluate(() =>
    !document.querySelector('#rzCorpo h3[data-marca-da-prova]'));
}

export const caso = {
  nome: 'botoes-do-assistente-de-replicacao',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // ------------------------------------------------- a tela que o abre
    await page.evaluate(() => verReplicacao());
    await esperar(page, '#btAssistRep');
    await page.click('#btAssistRep');
    await esperar(page, '#rzIr1');
    verdade(!!(await page.$('#rzSair')), 'o passo 1 devia trazer o Cancelar');

    // --------------------------------------------------- passo 1: o modo
    // Os cartoes de modo (`.venn`) trocam o subtitulo do dialogo -- e e ele
    // que diz se o clique pegou.
    await page.click('#rzModos .venn[data-m="primary_replica"]');
    const sub = await page.$eval('#rzCorpo .sub', e => e.textContent);
    verdade(/Primary/i.test(sub), `o modo escolhido devia aparecer no subtitulo; veio «${sub}»`);

    await page.click('#rzIr1');
    await esperar(page, '#rzIr2');

    // ---------------------------------------------- passo 2: os servidores
    // O «outro servidor» e o proprio servidor da bateria. Nao e simulacao: a
    // sonda abre uma conexao de verdade na porta de dados e fala o protocolo.
    await page.fill('#rzNome', 'espelho');
    await page.fill('#rzHost', '127.0.0.1');
    await page.fill('#rzPorta', String(ctx.portaDados));
    await page.fill('#rzTokenR', TOKEN);
    await page.fill('#rzUsu', USUARIO);
    await page.fill('#rzSen', SENHA);
    await page.click('#rzIr2');
    await esperar(page, '#rzDeNovo');
    const conectou = await page.$eval('#rzCorpo', e => e.textContent);
    verdade(/respondeu/.test(conectou), 'a sonda devia ter CONECTADO no proprio servidor');
    await capturar(ctx, ctx.nomeCaptura('assistente-passo3'));

    // «Testar de novo» refaz o passo 3 inteiro. Sem a marca, conferir que
    // `#rzDeNovo` continua na tela nao provaria nada -- ele ja estava la.
    const foiRefeito = await marcarCorpo(page);
    await page.click('#rzDeNovo');
    await esperar(page, '#rzDeNovo');
    verdade(await foiRefeito(), 'o «Testar de novo» devia refazer o passo 3');

    // ------------------------------------------------------- o «← Voltar»
    await page.click('#rzVolta');
    await esperar(page, '#rzIr2');
    verdade(!(await page.$('#rzDeNovo')), 'o Voltar devia sair do passo 3');

    // O «Cancelar» por ultimo: ele fecha o dialogo, e depois dele nao ha
    // mais onde clicar.
    await page.click('#rzVolta');
    await esperar(page, '#rzIr1');
    await page.click('#rzSair');
    await page.waitForTimeout(300);
    verdade(!(await page.$('#rzCorpo')), 'o Cancelar devia fechar o assistente');
  },
};

/* Os botoes da TIRA DE ABAS e da janela solta -- o cromo do modo multitela.
 *
 * Por que eles merecem lote proprio: sao os unicos botoes da interface que
 * nao sao `<button>`. O pino e o fechar da aba sao `<span role="button">`,
 * porque um `<button>` dentro de outro `<button>` nao existe em HTML -- e uma
 * varredura por `<button` nao os ve. Foram eles que fizeram o conferidor
 * aprender a segunda forma.
 *
 * A REGRA: confere o EFEITO no arranjo, nunca que a tela repintou.
 *
 * «Dividir em duas regioes» nao se prova vendo o botao ficar selecionado --
 * prova-se contando as regioes e a calha entre elas. O botao selecionado e
 * estado; a regiao que nasceu e efeito. */
import {
  entrar, capturar, cenario, verdade, bancoDoCaso, clicarOuExplicar, assentar,
} from '../apoio.mjs';

const conta = (page, sel) => page.locator(sel).count();

export const caso = {
  nome: 'botoes-da-tira',
  // Um tema so: nada aqui depende de cor, e o arranjo de regioes nao muda com
  // a paleta. Mesmo motivo do caso `multitela`.
  temaUnico: 'escuro',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'BtTira');
    await cenario(page, db, 'clientes');
    await page.evaluate(() => montarArvore(false));
    await page.waitForSelector(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);

    const falhas = [];
    const provado = [];
    const botao = async (chave, oQue, fn) => {
      try {
        await fn();
        provado.push(chave);
      } catch (e) {
        falhas.push(`${chave} (${oQue}): ${e.message}`);
      }
    };

    // ------------------------------------------------ 1. abrir outra tela
    await botao('[data-acao="nova"]', 'abrir outra tela nesta regiao', async () => {
      const antes = await conta(page, '.tira .tira-aba');
      await clicarOuExplicar(page, '.tira [data-acao="nova"]');
      await assentar(page, 300);
      // A aba nova nasce vazia esperando a proxima escolha: a pessoa clica na
      // arvore e a tela cai NELA. Provar so o clique deixaria passar a aba
      // que nasce e nao recebe nada.
      await page.click(`#arvore .no.tab[data-db="${db}"][data-tab="clientes"]`);
      await assentar(page, 700);
      const depois = await conta(page, '.tira .tira-aba');
      verdade(depois > antes,
        `a aba nova nao apareceu: ${antes} abas antes, ${depois} depois`);
    });

    // ------------------------------------------------- 2. pinar e fechar
    await botao('[data-pino]', 'pinar uma aba', async () => {
      // A ULTIMA aba, e nao a primeira: o arranjo guarda `abas.filter(t =>
      // t.pino && t.chave)`, e a tela do arranque nao tem chave -- pinar ela
      // grava um arranjo com a lista vazia e o passo reprovaria por um motivo
      // que nao e o dele. A ultima veio da arvore e tem chave.
      const alvo = page.locator('.tira .tira-aba [data-pino]').last();
      await alvo.waitFor();
      verdade(await alvo.getAttribute('aria-pressed') === 'false',
        'a aba ja nasceu pinada — o cenario nao serve');
      await alvo.click();
      await assentar(page, 300);
      verdade(await alvo.getAttribute('aria-pressed') === 'true',
        'o pino nao mudou de estado');
      // O EFEITO de verdade: pinar GRAVA a aba no arranjo, que e o que a faz
      // voltar na proxima abertura. Sem esta parte o passo provaria so que um
      // atributo mudou de texto -- e `aria-pressed` sem gravacao e uma
      // promessa que a proxima abertura desmente.
      const pinadas = await page.evaluate(() => {
        const o = JSON.parse(localStorage.getItem('phxsql-multitela') || '{}');
        return (o.regioes || []).reduce((n, r) => n + (r.abas || []).length, 0);
      });
      verdade(pinadas > 0, 'pinar nao gravou aba nenhuma no arranjo');
      await alvo.click();
      await assentar(page, 250);
      const depois = await page.evaluate(() => {
        const o = JSON.parse(localStorage.getItem('phxsql-multitela') || '{}');
        return (o.regioes || []).reduce((n, r) => n + (r.abas || []).length, 0);
      });
      verdade(depois === 0, `despinar deixou ${depois} aba(s) gravada(s)`);
    });

    await botao('[data-x]', 'fechar uma aba pelo ×', async () => {
      const antes = await conta(page, '.tira .tira-aba');
      verdade(antes > 1, `so ha ${antes} aba: o × nem existe com uma so`);
      await clicarOuExplicar(page, '.tira .tira-aba:nth-child(1) [data-x]');
      await assentar(page, 400);
      const depois = await conta(page, '.tira .tira-aba');
      verdade(depois === antes - 1,
        `o × nao fechou a aba: ${antes} antes, ${depois} depois`);
    });

    // ------------------------------------------------- 3. dividir a tela
    await botao('[data-acao="dividir"]', 'dividir em duas regioes e voltar', async () => {
      const antes = await conta(page, '#regioes .regiao');
      await clicarOuExplicar(page, '.tira [data-acao="dividir"][data-n="2"]');
      await assentar(page, 600);
      const duas = await conta(page, '#regioes .regiao');
      verdade(duas === 2, `dividir em 2 deu ${duas} regiao(oes), e nao 2`);
      // A CALHA entre as duas e o que se arrasta para dar largura a uma.
      // Duas regioes sem calha e um arranjo que a pessoa nao consegue
      // ajustar, e isso nao aparece contando regiao.
      verdade(await conta(page, '#regioes .calha') === 1,
        'nasceram duas regioes e nenhuma calha entre elas');
      await clicarOuExplicar(page, '#regioes .regiao:nth-child(1) .tira [data-acao="dividir"][data-n="1"]');
      await assentar(page, 600);
      const volta = await conta(page, '#regioes .regiao');
      verdade(volta === antes,
        `voltar para 1 regiao deixou ${volta}, e o comeco era ${antes}`);
      verdade(await conta(page, '#regioes .calha') === 0,
        'sobrou calha com uma regiao so');
    });

    // -------------------------------------- 4. a janela solta e os botoes dela
    await botao('[data-acao="soltar"]', 'soltar a tela numa janela flutuante', async () => {
      verdade(await conta(page, '.janela') === 0, 'ja havia janela solta antes do teste');
      await clicarOuExplicar(page, '.tira [data-acao="soltar"]');
      await assentar(page, 600);
      verdade(await conta(page, '.janela') === 1, 'a janela flutuante nao nasceu');
      // A janela leva a TELA junto, e nao so a moldura: uma janela vazia
      // passaria numa contagem e nao serve para nada.
      verdade(await conta(page, '.janela #painel, .janela .jan-corpo') > 0,
        'a janela nasceu sem corpo — levou a moldura e deixou a tela para tras');
    });

    await botao('[data-jan="pino"]', 'pinar a janela solta', async () => {
      const alvo = '.janela [data-jan="pino"]';
      await page.waitForSelector(alvo);
      const antes = await page.getAttribute(alvo, 'aria-pressed');
      await clicarOuExplicar(page, alvo);
      await assentar(page, 300);
      const depois = await page.getAttribute(alvo, 'aria-pressed');
      verdade(antes !== depois, `o pino da janela continuou «${antes}»`);
      await clicarOuExplicar(page, alvo);
      await assentar(page, 250);
    });

    await botao('[data-jan="fechar"]', 'fechar a janela solta', async () => {
      await clicarOuExplicar(page, '.janela [data-jan="fechar"]');
      await assentar(page, 500);
      verdade(await conta(page, '.janela') === 0, 'a janela nao fechou');
      // Fechar a janela nao pode levar a tela embora sem deixar nada: quem
      // fecha espera voltar a ter a regiao de antes.
      verdade(await conta(page, '#regioes .regiao') >= 1,
        'fechar a janela deixou a pagina sem regiao nenhuma');
    });

    await capturar(ctx, ctx.nomeCaptura('tira-botoes'));
    ctx.notas.push(`${provado.length} botoes da tira exercitados`);
    verdade(falhas.length === 0,
      `botoes da tira que reprovaram:\n      ${falhas.join('\n      ')}`);
  },
};

/* O gestor de threads, que e o unico PAINEL VIVO em forma de grade.
 *
 * Os outros casos exercitam telas que desenham uma vez. Este exercita a que
 * se redesenha sozinha a cada duas voltas do relogio, e e por isso que ele
 * existe: o padrao do painel vivo tem duas promessas que nenhuma outra tela
 * faz, e as duas ja quebraram uma vez.
 *
 * 1. A grade e PREGUICOSA. O `<details>` nasce fechado, e criar a grade dentro
 *    de um `display:none` mede largura zero em toda coluna.
 * 2. O GESTO SOBREVIVE a volta do relogio. Recriar a grade a cada volta faria
 *    a pessoa perder a ordenacao no meio da leitura.
 *
 * A segunda quebrou de um jeito que so aparece OLHANDO: a grade nascia sobre
 * uma `fonte` caseira, e com `fonte` quem ordena e a FONTE -- o grid manda
 * `{campo, dir, tipo}` e espera as linhas prontas. A fonte devolvia o array
 * como estava, entao a seta ▲ aparecia no cabecalho e o dado saia fora de
 * ordem. A prova da bancada do componente dizia «passou» porque conferia o
 * ESTADO (`estado().ordem`) e nao o EFEITO. Aqui se confere o efeito.
 *
 * Ele nao inventa dado: as threads sao as do servidor de verdade -- o ouvinte,
 * o amostrador, o aceitador e as da web que a propria bateria abriu. */
import { entrar, capturar, verdade, igual, clicarOuExplicar } from '../apoio.mjs';

/* POR QUE AS ESPERAS AQUI SAO LONGAS, e por que isso nao e tapar buraco.
 *
 * Este caso reprovou em 4 de 13 corridas da bateria INTEIRA -- nunca sozinho,
 * nunca no tema escuro sozinho. Tres hipoteses caíram medidas, e a ordem
 * importa porque a terceira e a que autoriza o numero:
 *
 * 1. «A tela demora a desenhar» -- NAO: 42 voltas isoladas do caminho
 *    entrar -> telaTelemetria -> `#tlmThreads`, pior caso 318 ms.
 * 2. «Algo reescreve o painel depois» -- NAO: 12 voltas vigiando 4 s, zero
 *    sumicos.
 * 3. «A telemetria trava quando o servidor escreve» -- NAO, e esta era a
 *    hipotese de PRODUTO: medido com lotes de 2.000 linhas em paralelo, a op
 *    `telemetria` responde em 16 ms de mediana e 119 ms no PIOR caso, contra
 *    11/17 ms com o servidor parado. Sao 7x de degradacao e 126x de folga
 *    contra os 15 s que o caso dava.
 *
 * Sobra o navegador: no fim de trinta casos ele fica lento, e quem paga e o
 * unico caso da bateria que depende de uma volta de relogio de 2 s chegar. O
 * limite generoso aqui compra essa lentidao SEM esconder nada, porque o que
 * ele poderia esconder foi medido e nao existe -- se um dia a op passar dos
 * segundos, a bancada `bancada/telemetria/` e que tem de acusar, e nao este
 * caso reprovando por um motivo que nao consegue nomear. */
const ESPERA = 45000;

const GRADE = '#tlmThreads .phx-grid';
const LINHA = `${GRADE} tbody tr:not(.phx-grupo)`;
const CAB = f => `#tlmThreads thead tr:not(.phx-frow) th[data-campo="${f}"]`;

export const caso = {
  nome: 'telemetria',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    await page.evaluate(() => telaTelemetria());
    // `attached` e nao `visible`: fechado, o `<details>` esconde o alvo -- e e
    // exatamente isso que a primeira asercao quer confirmar.
    await page.waitForSelector('#tlmThreads', { state: 'attached', timeout: ESPERA });

    // ESPERA O EVENTO, NUNCA UMA DURACAO, E NUNCA UMA FRASE.
    //
    // A primeira versao dormia 2.600 ms («uma volta do relogio») e reprovava
    // em 3 de 8 passadas da bateria INTEIRA -- nunca sozinha, nunca no tema
    // escuro sozinho. Medido antes de consertar: o desenho da tela nao e o
    // problema (42 voltas isoladas, pior caso 318 ms contra o limite de 15 s)
    // e o painel nao e reescrito depois (12 voltas vigiadas). O que varia e
    // QUANDO a volta do relogio traz as threads.
    //
    // O primeiro conserto trocou o sono por «esperar o contador ter digito»,
    // e tinha DOIS furos. O menor: «0 viva(s) de 0 registrada(s)» tem digito,
    // entao ele seguia com a grade vazia e o vermelho ia para a linha de
    // baixo. O maior, e o que esta base ja tem escrito como regra: aquilo
    // lia a FRASE. Texto se resolve por chave, nunca por comparacao da frase
    // -- no dia em que alguem melhorar a redacao, ou a tela abrir noutro
    // idioma, uma espera assim estoura parecendo defeito de produto.
    //
    // O que se espera aqui e o que a PESSOA espera: abrir o painel e a linha
    // aparecer. O `waitForSelector` ja reconsulta sozinho enquanto as voltas
    // do relogio chegam, entao ele e o instrumento certo -- e nao le texto
    // nenhum.
    verdade(await page.$(GRADE) === null,
      'a grade nasceu com o painel fechado -- dentro de display:none ela mede largura zero');

    await clicarOuExplicar(page, '.tlm-threads summary');
    await page.waitForSelector(LINHA, { timeout: 25000 });

    const resumo = await page.textContent('#tlmThreadsN');
    verdade(/\d+/.test(resumo || ''),
      `o resumo do gestor de threads nao contou nada: ${JSON.stringify(resumo)}`);

    await capturar(ctx, ctx.nomeCaptura('telemetria-threads'));

    verdade(await page.$('#tlmThreads .phx-groupbox') !== null,
      'o gestor de threads perdeu a faixa de agrupamento');
    const campos = await page.$$eval('#tlmThreads thead tr:not(.phx-frow) th',
      ths => ths.map(t => t.getAttribute('data-campo')));
    igual(campos.join(','), 'nome,familia,finalidade,fazendo,voltas,viva_s',
      'as colunas do gestor de threads mudaram');
    verdade((await page.$$(LINHA)).length > 0, 'o gestor de threads nao listou thread nenhuma');

    // O EFEITO, e nao o estado: a coluna sai ordenada...
    const familias = () => page.$$eval(`${LINHA} td:nth-child(2)`, t => t.map(x => x.textContent.trim()));
    const ordenada = v => v.every((x, i) => i === 0 || v[i - 1] <= x);
    await page.click(`${CAB('familia')} .phx-th-titulo`);
    await page.waitForFunction(
      () => document.querySelector('#tlmThreads thead th[data-campo="familia"] .phx-sort-ind')
              .textContent.trim() !== '', { timeout: ESPERA });
    const antes = await familias();
    verdade(ordenada(antes), `ordenar por familia nao ordenou: ${JSON.stringify(antes)}`);

    // ...e continua ordenada DEPOIS DE UM REDESENHO DE VERDADE. Aqui tambem
    // nao se dorme: um observador avisa quando o corpo da grade foi trocado,
    // que e o evento que o caso quer provar ter sobrevivido. Dormindo, ou se
    // esperava demais (lento) ou se media antes de a volta chegar (falso
    // verde) -- e o falso verde e o pior dos dois.
    const redesenhou = await page.evaluate(() => new Promise(res => {
      const alvo = document.querySelector('#tlmThreads tbody');
      if (!alvo) return res(false);
      const obs = new MutationObserver(() => { obs.disconnect(); res(true); });
      obs.observe(alvo, { childList: true });
      setTimeout(() => { obs.disconnect(); res(false); }, 20000);
    }));
    verdade(redesenhou,
      'o painel vivo nao redesenhou em 20 s -- ou o relogio parou, ou a grade nao esta ligada nele');
    const depois = await familias();
    verdade(ordenada(depois),
      `o redesenho do painel vivo perdeu a ordenacao da pessoa: ${JSON.stringify(depois)}`);
    igual(await page.$eval(`${CAB('familia')} .phx-sort-ind`, e => e.textContent.trim()), '▲',
      'a seta de ordenacao sumiu do cabecalho depois do redesenho');

    // A thread encerrada continua apagada -- era o `tr.morta` da folha, que
    // virou opacidade por celula porque a grade nao tem gancho por linha.
    const apagadas = await page.$$eval('#tlmThreads tbody td span[style*="opacity"]', t => t.length);
    verdade(apagadas > 0, 'nenhuma thread encerrada apareceu apagada');
  },
};

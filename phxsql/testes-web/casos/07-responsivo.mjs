/* Celular, tablet e desktop -- as tres larguras que foram pedidas.
 *
 * A regra que atravessa as tres esta escrita no proprio CSS: NADA rola de
 * lado no corpo da pagina. O que for largo demais rola DENTRO do proprio
 * conteiner. Este caso mede isso, em vez de acreditar no comentario: para
 * cada largura e cada tela, `documentElement.scrollWidth` nao pode passar da
 * largura visivel.
 *
 * E o piso de 40px do alvo de toque, no celular, tambem se mede -- o CSS
 * promete `@media (pointer:coarse)`, e a promessa se confere com o
 * `getBoundingClientRect` e nao com a leitura da regra. */
import { entrar, cenario, capturar, verdade, bancoDoCaso } from '../apoio.mjs';

/* Do maior para o menor, e a ordem IMPORTA -- ela e um achado da bateria.
 *
 * Em 390px a lateral vira gaveta e se FECHA sozinha depois de cada escolha
 * (`fecharSeSolta`), e esse fechamento e GRAVADO no navegador. Comecando pelo
 * celular, o tablet e o desktop seguintes abriam sem arvore nenhuma -- e as
 * capturas mostravam um layout que ninguem projetou.
 *
 * O comportamento em si nao e defeito obvio: na gaveta, fechar depois de
 * escolher e o certo. O que ele revela e que uma janela que ENCOLHE e volta
 * a crescer nao recupera a arvore, porque o fechamento automatico ficou
 * indistinguivel de uma escolha da pessoa. Esta anotado em docs/TESTES.md;
 * medir com a ordem certa vem antes de propor conserto. */
const LARGURAS = [
  ['desktop', 1600, 950],
  ['tablet', 820, 1180],
  ['celular', 390, 844],
];

export const caso = {
  nome: 'responsivo',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Resp');
    const { tab } = await cenario(page, db);

    const problemas = [];

    for (const [apelido, w, h] of LARGURAS) {
      await page.setViewportSize({ width: w, height: h });
      await page.waitForTimeout(250);

      const telas = [
        ['painel', () => page.evaluate(() => irPara('painel'))],
        ['grade', () => page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab])],
        ['ficha', async () => {
          await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
          await page.waitForSelector('#btNova');
          await page.click('#btNova');
        }],
        ['nova tabela', () => page.evaluate(d => telaNovaTabela(d), db)],
        ['config do servidor', () => page.evaluate(() => verConfigServidor())],
        ['usuarios', () => page.evaluate(() => irPara('usuarios'))],
      ];

      for (const [nome, abrir] of telas) {
        await abrir();
        await page.waitForTimeout(450);
        const medida = await page.evaluate(() => ({
          rola: document.documentElement.scrollWidth,
          cabe: document.documentElement.clientWidth,
          corpo: document.body.scrollWidth,
        }));
        // 1px de folga: arredondamento de layout nao e rolagem lateral.
        if (medida.rola > medida.cabe + 1) {
          problemas.push(`${apelido} · ${nome}: a PAGINA rola de lado `
            + `(${medida.rola}px num visor de ${medida.cabe}px)`);
        }
        await capturar(ctx, ctx.nomeCaptura(`${apelido}-${nome.replace(/\W+/g, '-')}`));
      }

      if (apelido === 'celular') {
        // A lateral e gaveta aqui: 268px de 390 nao e um painel ao lado do
        // trabalho, e a tela inteira.
        const larguraDaColuna = await page.evaluate(() =>
          getComputedStyle(document.getElementById('app')).getPropertyValue('--arvore-col').trim());
        verdade(larguraDaColuna === '0px',
          `no celular a arvore ainda ocupa coluna (--arvore-col: ${larguraDaColuna})`);
      }
    }

    await page.setViewportSize({ width: 1600, height: 950 });
    ctx.notas.push(`${LARGURAS.length} larguras × 6 telas medidas`);
    verdade(problemas.length === 0, `rolagem lateral:\n      ${problemas.join('\n      ')}`);
  },
};

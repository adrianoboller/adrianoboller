/* O cartao da saude do disco no Painel (pedido 249), nos DOIS estados.
 *
 * O que se prova aqui e o que so aparece exercitando: que o cartao existe ao
 * lado do espaco em disco, que o pino de estado carrega FORMA e cor, e que o
 * estado «erro» chega a tela quando o disco recusa a sonda -- com uma recusa
 * DE VERDADE do sistema operacional, e nao com um dado plantado na resposta.
 *
 * Como se forca a falha: a sonda escreve `<base>/.saude-do-disco`; o caso
 * poe um DIRETORIO com esse nome. O `open` seguinte falha com EISDIR (errno
 * 21), a sonda vira `erro`, e ao tirar o diretorio a proxima passada volta a
 * `ok`. A bateria sobe o servidor com `alertas.disco.checar_segundos: 2`
 * (`servidor.mjs`), senao cada espera aqui seria de 5 minutos (padrao do
 * pedido 249).
 *
 * O que se le e o `data-estado` do pino -- nunca a frase, que muda de idioma
 * e de redacao. */
import { mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';

import { entrar, capturar, verdade, igual, api } from '../apoio.mjs';

const CANARIO = '.saude-do-disco';
const PINO = '#saudeDisco .pino[data-estado]';

/** Espera a op `saude_disco` dizer o estado pedido -- pela API, e nao
 *  dormindo um numero: a sonda roda a cada 2 s e o que se espera e o EVENTO.
 *
 *  Laco explicito, e nao `waitForFunction` com predicado `async`: a primeira
 *  versao usava esse predicado e o caso «passou» a espera em 0 ms nos dois
 *  temas -- uma Promise e um valor verdadeiro, e o `waitForFunction` a
 *  aceitou como «condicao satisfeita» sem esperar por ela. Medido em
 *  16/09/2026: 643 ms para o caso inteiro, com a espera de 2 s dentro. */
async function esperarEstado(page, estado, prazo = 20000) {
  const fim = Date.now() + prazo;
  let visto;
  while (Date.now() < fim) {
    visto = (await api(page, 'saude_disco')).estado;
    if (visto === estado) return;
    await new Promise(r => setTimeout(r, 300));
  }
  throw new Error(`a saude do disco nao chegou a "${estado}" em ${prazo} ms (ultimo: "${visto}")`);
}

async function estadoNaTela(page) {
  await page.evaluate(() => abrirAdmin('painel'));
  await page.waitForSelector(PINO, { timeout: 20000 });
  return await page.$eval(PINO, el => el.getAttribute('data-estado'));
}

export const caso = {
  nome: 'saude-do-disco',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    // 1. Saudavel: a sonda ja rodou (sobe com o servidor) e o cartao esta la,
    //    com as quatro linhas -- ultima sonda, erros, ultimo evento, avisos.
    await esperarEstado(page, 'ok');
    igual(await estadoNaTela(page), 'ok', 'o cartao nao mostra o disco saudavel');
    igual(await page.$$eval('#saudeDisco dd', d => d.length), 4, 'o cartao perdeu uma linha');
    const forma = await page.$eval(PINO, el => el.textContent.trim().charAt(0));
    igual(forma, '●', 'o pino saudavel tem de carregar a FORMA, nao so a cor');
    await capturar(ctx, ctx.nomeCaptura('saude-disco-ok'));

    // 2. Com erro: o sistema operacional recusa a sonda.
    const canario = join(ctx.base, CANARIO);
    mkdirSync(canario);
    try {
      await esperarEstado(page, 'erro');
      igual(await estadoNaTela(page), 'erro', 'a recusa do disco nao chegou ao cartao');
      igual(await page.$eval(PINO, el => el.textContent.trim().charAt(0)), '✕',
        'o pino de erro tem de carregar a forma ✕');
      const r = await api(page, 'saude_disco');
      verdade(/^abrir: EISDIR \(21\)/.test(r.canario_falha || ''),
        `a falha tem de nomear o passo e o errno: ${JSON.stringify(r.canario_falha)}`);
      verdade(r.ultimo_evento && r.ultimo_evento.origem === 'sonda',
        `o ultimo evento tem de ser da sonda: ${JSON.stringify(r.ultimo_evento)}`);
      await capturar(ctx, ctx.nomeCaptura('saude-disco-erro'));
    } finally {
      rmSync(canario, { recursive: true, force: true });
    }

    // 3. Volta ao normal na passada seguinte -- e o cartao acompanha.
    await esperarEstado(page, 'ok');
    igual(await estadoNaTela(page), 'ok', 'o cartao nao voltou a saudavel');
  },
};

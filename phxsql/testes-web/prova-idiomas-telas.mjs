#!/usr/bin/env node
/* A PROVA das QUATRO telas que a leva desta rodada traduziu -- pelo navegador,
 * e nao por leitura de codigo.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/prova-idiomas-telas.mjs --porta 7550 \
 *          --capturas /tmp/idiomas-telas
 *
 * A `prova-idiomas.mjs` ao lado prova a MAQUINA: o login, o cromo, a nota da
 * multitela e o texto que estica. Esta prova as telas que estavam de fora
 * dela -- a da Claude, a Telemetria, a grade e o diagrama ER --, e prova a
 * mesma coisa nas quatro: o texto muda de verdade ao trocar de idioma, e a
 * tela sem escolha nenhuma continua sendo a de sempre.
 *
 * O que ela pega e o que ler o codigo nao pega. Duas armadilhas moram aqui:
 *
 *   1. o PAR `rot:`/`txt:` de uma lista lida no ARRANQUE. Sem o par, a lista
 *      guarda o portugues para sempre e a tela nunca troca -- e o codigo
 *      parece certo, porque o `txt(...)` esta la, so que resolvido cedo
 *      demais. O `NIVEIS` da telemetria e o `MODELOS` da Claude sao os dois;
 *   2. a chamada `txt(...)` dentro de um modulo que NAO e o `index.html`.
 *      Os quatro arquivos sao IIFE proprias, e o `txt` de cada uma delega no
 *      global. Se a delegacao quebrar, a tela sai em portugues em silencio --
 *      sem erro no console, sem nada.
 *
 * Sobe um phxsqld proprio e o derruba pelo PID -- nunca `pkill -f`.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { Falha, verdade, contem, entrar, capturar, assentar, cenario, abrirPelaArvore } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const opc = {
  capturas: arg('--capturas'),
  porta: Number(arg('--porta', '7550')),
};

const VERDE = s => `\x1b[32m${s}\x1b[0m`;
const VERMELHO = s => `\x1b[31m${s}\x1b[0m`;
const CINZA = s => `\x1b[90m${s}\x1b[0m`;

let passos = 0;
let quebrados = 0;

/* Cada passo começa em PORTUGUÊS e termina em português -- e o `finally` é o
 * que garante o segundo. Sem ele, uma afirmação que falha no meio deixa a
 * escolha do passo anterior de pé, e o passo seguinte reprova por um defeito
 * que não é o dele: foi exatamente o que aconteceu na primeira rodada desta
 * prova, com o diagrama ER acusado de estar em espanhol por causa da grade. */
async function passo(page, nome, corpo) {
  const t0 = Date.now();
  try {
    await trocarIdioma(page, 'Portugues');
    try {
      await corpo();
    } finally {
      await trocarIdioma(page, 'Portugues');
    }
    passos++;
    console.log(`  ${VERDE('ok    ')} ${nome.padEnd(50)} ${Date.now() - t0} ms`);
  } catch (e) {
    quebrados++;
    console.log(`  ${VERMELHO('QUEBROU')} ${nome}`);
    console.log(`         ${e instanceof Falha ? e.message : e.stack}`);
  }
}

/** Troca o idioma pelo MESMO caminho da pessoa -- a bandeira da tela de
 *  Idiomas --, e nao por um `localStorage.setItem` por dentro. */
async function trocarIdioma(page, coluna) {
  await page.evaluate(c => escolherIdioma(c), coluna);
  await assentar(page, 400);
}

/** O texto visivel do painel, ja normalizado. */
const painel = page =>
  page.$eval('#painel', e => e.textContent.replace(/\s+/g, ' ').trim()).catch(() => '');

/** A grade mora na aba «Conteudo» de uma tabela, e se chega nela clicando --
 *  pela arvore e pela aba, que e o caminho da pessoa. */
async function abrirAba(page, db) {
  await abrirPelaArvore(page, db, 'clientes');
  await page.click('.aba[data-aba="conteudo"]');
  await page.waitForSelector('.phx-grid', { timeout: 15000 });
  await assentar(page, 600);
}

async function main() {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  const srv = await subir({ phxsqld, portaDados: opc.porta, portaWeb: opc.porta + 1 });
  console.log(CINZA(`· servidor pid ${srv.pid} — dados ${opc.porta}, web ${opc.porta + 1}`));
  const navegador = await chromium.launch();
  const page = await navegador.newPage({ viewport: { width: 1440, height: 950 } });
  const ctx = { page, capturas: opc.capturas };
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));

  try {
    await entrar(page, srv.url);
    const db = 'idiomas_telas';
    await cenario(page, db);

    // ------------------------------------------------------- a tela da Claude
    await passo(page, 'a tela da Claude troca de idioma inteira', async () => {
      await page.evaluate(() => PhxIA.telaConfig());
      await assentar(page);
      const pt = await painel(page);
      contem(pt, 'Leia antes de ligar', 'a tela da Claude em portugues');
      contem(pt, 'Chave da API', 'o rotulo do campo da chave');
      contem(pt, 'o mais capaz', 'a explicacao de custo do modelo (o par diz:/dizTxt:)');
      await capturar(ctx, 'claude-pt');

      await trocarIdioma(page, 'Alemao');
      await page.evaluate(() => PhxIA.telaConfig());
      await assentar(page);
      const de = await painel(page);
      contem(de, 'Vor dem Einschalten lesen', 'a tela da Claude em alemao');
      contem(de, 'API-Schlüssel', 'o rotulo do campo da chave em alemao');
      contem(de, 'der leistungsfähigste', 'a explicacao de custo em alemao (o par)');
      verdade(!de.includes('Leia antes de ligar'), 'sobrou portugues na tela em alemao');
      await capturar(ctx, 'claude-de');


    });

    // -------------------------------------------------------- a telemetria
    await passo(page, 'a telemetria troca a barra, as faixas e a legenda', async () => {
      await page.evaluate(() => telaTelemetria());
      await assentar(page, 1200);
      const pt = await painel(page);
      contem(pt, 'Atualizar agora', 'o botao da barra da telemetria');
      contem(pt, 'Esperas', 'o titulo da primeira faixa');
      contem(pt, 'Gestor de threads', 'a gaveta das threads');
      contem(pt, 'uso alto', 'o rotulo do nivel na legenda');
      await capturar(ctx, 'telemetria-pt');

      await trocarIdioma(page, 'Frances');
      await page.evaluate(() => telaTelemetria());
      await assentar(page, 1200);
      const fr = await painel(page);
      contem(fr, 'Actualiser maintenant', 'o botao da barra em frances');
      contem(fr, 'Attentes', 'o titulo da faixa em frances');
      contem(fr, 'Gestionnaire de threads', 'a gaveta das threads em frances');
      // A legenda sai do MESMO objeto que pinta as bolhas (`NIVEIS`): se o par
      // `rot:`/`txt:` nao pegasse, ela ficaria em portugues para sempre.
      contem(fr, 'usage élevé', 'o rotulo do nivel «uso alto» (o par do NIVEIS)');
      verdade(!fr.includes('Atualizar agora'), 'sobrou portugues na telemetria em frances');
      await capturar(ctx, 'telemetria-fr');


    });

    // ------------------------------------------------------------- a grade
    await passo(page, 'a grade troca o rodape, a paginacao e o filtro', async () => {
      await abrirAba(page, db);
      const pt = await painel(page);
      contem(pt, 'itens por página', 'o rotulo do tamanho de pagina');
      contem(pt, 'Colunas:', 'o botao do seletor de colunas');
      contem(pt, 'Página 1 de', 'a paginacao');
      await capturar(ctx, 'grade-pt');

      await trocarIdioma(page, 'Espanhol');
      await abrirAba(page, db);
      const es = await painel(page);
      contem(es, 'elementos por página', 'o rodape da grade em espanhol');
      contem(es, 'Columnas:', 'o seletor de colunas em espanhol');
      verdade(!es.includes('itens por página'), 'sobrou portugues na grade em espanhol');
      await capturar(ctx, 'grade-es');

      // O painel do filtro de coluna so existe depois do clique -- ler o
      // codigo nao o abre, e e onde moram sete dos rotulos da grade.
      await page.locator('.phx-fbtn').first().click();
      await assentar(page, 400);
      const pop = await page.$eval('.phx-fpop', e => e.textContent.replace(/\s+/g, ' ').trim());
      contem(pop, 'Ordenar de A a Z', 'o filtro de coluna em espanhol');
      contem(pop, 'Borrar filtro', 'o limpar do filtro em espanhol');
      // A busca do painel e um `placeholder`: ela nao aparece no
      // `textContent`, e uma afirmacao sobre o texto passaria por engano.
      contem(await page.getAttribute('.phx-fpop-busca', 'placeholder'), 'Buscar',
        'a busca do filtro em espanhol');
      await capturar(ctx, 'grade-es-filtro');
      await page.keyboard.press('Escape');


    });

    // -------------------------------------------------------- o diagrama ER
    await passo(page, 'o diagrama ER troca o rotulo do desenho', async () => {
      await page.evaluate(d => telaDiagramaER(d), db);
      await page.waitForSelector('svg.er', { timeout: 15000 });
      await assentar(page, 400);
      const pt = await page.getAttribute('svg.er', 'aria-label');
      contem(pt, 'Diagrama de entidades', 'o rotulo do desenho em portugues');
      await capturar(ctx, 'er-pt');

      await trocarIdioma(page, 'Italiano');
      await page.evaluate(d => telaDiagramaER(d), db);
      await page.waitForSelector('svg.er', { timeout: 15000 });
      await assentar(page, 400);
      const it = await page.getAttribute('svg.er', 'aria-label');
      contem(it, 'Diagramma entità-relazioni', 'o rotulo do desenho em italiano');
      await capturar(ctx, 'er-it');


    });

    // --------------------------------------- o comportamento VELHO, de novo
    // Guarda nova entra pedida: sem escolher nada, a tela e a de sempre.
    await passo(page, 'sem escolher nada, as telas sao as de sempre', async () => {
      await page.evaluate(() => { localStorage.removeItem('phxsql-idioma'); });
      await entrar(page, srv.url);
      // O Painel termina de se desenhar DEPOIS de a arvore aparecer, e
      // reescreve `#painel` inteiro: abrir outra tela antes disso perde a
      // corrida e o passo reprova pela tela errada. Esperar o desenho e o que
      // faz o passo medir o que ele diz medir.
      await page.waitForFunction(
        () => document.querySelector('#painel')?.textContent.includes('MEMÓRIA'),
        { timeout: 20000 });
      await page.evaluate(() => PhxIA.telaConfig());
      await assentar(page, 500);
      contem(await painel(page), 'Leia antes de ligar',
        'a tela da Claude sem escolha nenhuma tem de estar em portugues');
      await page.evaluate(() => telaTelemetria());
      await assentar(page, 1200);
      contem(await painel(page), 'Atualizar agora',
        'a telemetria sem escolha nenhuma tem de estar em portugues');
    });

    await passo(page, 'nenhum erro de JavaScript nas quatro telas', async () => {
      verdade(erros.length === 0, `a pagina estourou: ${erros.join(' | ')}`);
    });
  } finally {
    await navegador.close();
    await srv.derrubar();
    console.log(CINZA('· servidor derrubado pelo PID'));
  }

  console.log(`\n${passos}/${passos + quebrados} passos passaram`);
  process.exit(quebrados ? 1 : 0);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});

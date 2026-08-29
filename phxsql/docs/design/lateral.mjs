// Exercicio do PAINEL LATERAL: recolhe, expande, pina, despina, sobrevive ao
// recarregar, e o botao de reabrir existe em TODOS os estados.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync } from 'node:fs';

const SAIDA = process.argv[2] || 'lateral';
mkdirSync(SAIDA, { recursive: true });
const BASE = 'http://127.0.0.1:5770/';

let falhas = 0;
const conf = (nome, ok, extra = '') => {
  console.log(`  ${ok ? 'ok  ' : 'FALHA'}  ${nome}${extra ? ' — ' + extra : ''}`);
  if (!ok) falhas++;
};

async function entrar(ctx) {
  const p = await ctx.newPage();
  await p.goto(BASE);
  await p.waitForSelector('#t', { state: 'visible' });
  await p.fill('#t', 'design-token');
  await p.fill('#u', 'adriano');
  await p.fill('#s', 'design123');
  await p.locator('#btEntrar').click();
  await p.waitForSelector('#app.ativo');
  await p.waitForTimeout(1200);
  return p;
}

// O que a tela diz de si mesma.
const estado = p => p.evaluate(() => {
  const app = document.querySelector('#app');
  const lat = document.querySelector('#lateral');
  const bt = document.querySelector('#btLateral');
  const r = lat.getBoundingClientRect();
  const corpo = document.querySelector('.corpo').getBoundingClientRect();
  const btR = bt.getBoundingClientRect();
  const cs = getComputedStyle(bt);
  return {
    aberta: app.dataset.lateralAberta,
    solta: app.dataset.lateralSolta,
    latVisivel: getComputedStyle(lat).visibility !== 'hidden' && r.width > 10,
    latLargura: Math.round(r.width),
    corpoEsq: Math.round(corpo.left),
    corpoLarg: Math.round(corpo.width),
    janela: window.innerWidth,
    // O botao de reabrir: existe, esta visivel e e clicavel?
    btVisivel: cs.visibility !== 'hidden' && cs.display !== 'none'
               && btR.width > 0 && btR.height > 0,
    btRotulo: bt.getAttribute('aria-label'),
    btExpandido: bt.getAttribute('aria-expanded'),
    pinPressionado: document.querySelector('#btPinar').getAttribute('aria-pressed'),
    pinDesligado: document.querySelector('#btPinar').disabled,
    guardado: (() => { try { return localStorage.getItem('phxsql-lateral'); }
                       catch { return null; } })(),
  };
});

const nav = await chromium.launch();

// ============================================================ DESKTOP
console.log('\n== DESKTOP 1440x900 ==');
{
  const ctx = await nav.newContext({ viewport: { width: 1440, height: 900 } });
  const p = await entrar(ctx);

  let e = await estado(p);
  conf('comeca aberta e pinada', e.aberta === '1' && e.solta === '0',
       `coluna ${e.latLargura}px, corpo comeca em ${e.corpoEsq}px`);
  conf('botao de reabrir visivel', e.btVisivel);
  await p.screenshot({ path: `${SAIDA}/1-fixa.png` });

  // --- RECOLHER
  await p.locator('#btLateral').click();
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('recolheu', e.aberta === '0' && !e.latVisivel);
  conf('conteudo ganhou a tela inteira', e.corpoEsq === 0 && e.corpoLarg === e.janela,
       `corpo ${e.corpoLarg}px de ${e.janela}px`);
  conf('botao de reabrir CONTINUA visivel com o painel recolhido', e.btVisivel,
       e.btRotulo);
  await p.screenshot({ path: `${SAIDA}/2-recolhida.png` });

  // --- SOBREVIVE AO RECARREGAR (recolhida)
  await p.reload();
  await p.waitForSelector('#t', { state: 'visible' });
  await p.fill('#t', 'design-token');
  await p.fill('#u', 'adriano');
  await p.fill('#s', 'design123');
  await p.locator('#btEntrar').click();
  await p.waitForSelector('#app.ativo');
  await p.waitForTimeout(1200);
  e = await estado(p);
  conf('recolhida sobreviveu ao recarregar', e.aberta === '0', e.guardado);
  conf('e o botao de reabrir veio junto', e.btVisivel);

  // --- EXPANDIR de volta
  await p.locator('#btLateral').click();
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('expandiu', e.aberta === '1' && e.latVisivel);

  // --- DESPINAR: o painel passa a flutuar e o conteudo fica inteiro
  await p.locator('#btPinar').click();
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('despinou: virou sobreposta', e.solta === '1' && e.pinPressionado === 'false');
  conf('despinada, o conteudo ocupa a tela inteira POR BAIXO',
       e.corpoEsq === 0 && e.corpoLarg === e.janela,
       `corpo ${e.corpoLarg}px de ${e.janela}px, painel por cima com ${e.latLargura}px`);
  await p.screenshot({ path: `${SAIDA}/3-solta.png` });

  // --- despinada, escolher na arvore fecha
  await p.locator('#arvore .no.tab').first().click();
  await p.waitForTimeout(500);
  e = await estado(p);
  conf('despinada, escolher fecha o painel', e.aberta === '0');
  conf('e da para reabrir', e.btVisivel);

  // --- REPINAR
  await p.locator('#btLateral').click();
  await p.waitForTimeout(300);
  await p.locator('#btPinar').click();
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('repinou: voltou a ocupar coluna', e.solta === '0' && e.pinPressionado === 'true',
       `corpo comeca em ${e.corpoEsq}px`);

  // --- pinada, escolher NAO fecha
  await p.locator('#arvore .no.tab').first().click();
  await p.waitForTimeout(500);
  e = await estado(p);
  conf('pinada, escolher NAO fecha (e o que pinar quer dizer)', e.aberta === '1');

  // --- ATALHO Ctrl+\
  await p.keyboard.press('Control+\\');
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('Ctrl+\\ recolhe', e.aberta === '0');
  await p.keyboard.press('Control+\\');
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('Ctrl+\\ reabre', e.aberta === '1');

  // --- LARGURA POR TECLADO na pega
  const antes = (await estado(p)).latLargura;
  await p.locator('#pegaArvore').focus();
  for (let i = 0; i < 5; i++) await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(300);
  const depois = (await estado(p)).latLargura;
  conf('a largura anda pelo teclado', depois > antes, `${antes}px -> ${depois}px`);

  // --- LARGURA POR ARRASTO
  const pega = await p.locator('#pegaArvore').boundingBox();
  await p.mouse.move(pega.x + 4, pega.y + 200);
  await p.mouse.down();
  await p.mouse.move(420, pega.y + 200, { steps: 8 });
  await p.mouse.up();
  await p.waitForTimeout(300);
  const arrastada = (await estado(p)).latLargura;
  conf('a largura anda por arrasto', Math.abs(arrastada - 420) < 12,
       `pedi 420px, ficou ${arrastada}px`);
  await p.screenshot({ path: `${SAIDA}/4-larga.png` });

  // --- a largura sobrevive ao recarregar
  await p.reload();
  await p.waitForSelector('#t', { state: 'visible' });
  await p.fill('#t', 'design-token');
  await p.fill('#u', 'adriano');
  await p.fill('#s', 'design123');
  await p.locator('#btEntrar').click();
  await p.waitForSelector('#app.ativo');
  await p.waitForTimeout(1200);
  const relida = (await estado(p)).latLargura;
  conf('a largura sobreviveu ao recarregar', Math.abs(relida - arrastada) < 3,
       `${arrastada}px -> ${relida}px`);
  await ctx.close();
}

// ============================================================ CELULAR
console.log('\n== CELULAR 390x844 ==');
{
  const ctx = await nav.newContext({ viewport: { width: 390, height: 844 },
                                     isMobile: true, hasTouch: true });
  const p = await entrar(ctx);
  let e = await estado(p);
  conf('no celular o painel comeca FORA do caminho', e.aberta === '0',
       `corpo ${e.corpoLarg}px de ${e.janela}px`);
  conf('o conteudo tem a tela inteira', e.corpoLarg === e.janela);
  conf('o botao de abrir esta na tela', e.btVisivel);
  conf('pinar aparece desligado, porque aqui nao ha escolha', e.pinDesligado);
  await p.screenshot({ path: `${SAIDA}/5-celular-fechada.png` });

  await p.locator('#btLateral').click();
  await p.waitForTimeout(450);
  e = await estado(p);
  conf('abre como gaveta sobreposta', e.aberta === '1' && e.solta === '1');
  conf('a gaveta nao empurra o conteudo', e.corpoEsq === 0);
  const veu = await p.locator('#veuArvore').isVisible();
  conf('o veu aparece atras da gaveta', veu);
  await p.screenshot({ path: `${SAIDA}/6-celular-gaveta.png` });

  await p.locator('#veuArvore').click({ force: true });
  await p.waitForTimeout(400);
  e = await estado(p);
  conf('tocar fora fecha a gaveta', e.aberta === '0');

  await ctx.close();
}

await nav.close();
console.log(`\n${falhas === 0 ? 'TUDO PASSOU' : falhas + ' FALHA(S)'}`);
process.exit(falhas ? 1 : 0);

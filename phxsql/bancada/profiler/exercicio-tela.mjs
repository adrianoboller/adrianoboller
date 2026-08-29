// A TELA do Profiler, exercitada no navegador -- porque interface só se prova
// exercitando, e dois dos achados desta rodada não apareciam lendo o código.
//
//   node bancada/profiler/exercicio-tela.mjs <porta-web> <token> <pasta> <log> <log-cheio>
//
// O que ele confere:
//   1. a caixa de estado fica VERDE quando está observando (a classe era
//      `bem`, e a classe verde desta interface chama-se `bom` -- a caixa
//      passou a vida inteira cinza);
//   2. a senha aparece como `***` na grade, e a sentinela não aparece;
//   3. com o disco cheio a caixa fica VERMELHA e diz quantas linhas não foram
//      gravadas -- antes ela seguia dizendo «gravando em ...»;
//   4. nenhum erro de página, nos dois temas.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';

const PORTA = process.argv[2] || '6281';
const TOKEN = process.argv[3] || 'token-de-servico';
const SAIDA = process.argv[4] || 'tiros';
const LOG = process.argv[5] || '/tmp/profiler-tela.txt';
const CHEIO = process.argv[6] || '';
const BASE = `http://127.0.0.1:${PORTA}/`;
const SENTINELA = 'SENHA-NA-TELA-9137';

const achados = [];
const falhas = [];

async function entrar(p) {
  await p.goto(BASE);
  await p.waitForSelector('#t', { state: 'visible', timeout: 20000 });
  await p.fill('#t', TOKEN);
  await p.fill('#u', 'adm');
  await p.fill('#s', 'senha-do-adm');
  await p.locator('#btEntrar').click();
  await p.waitForSelector('#app.ativo', { timeout: 20000 });
  await p.waitForTimeout(800);
}

async function abrirProfiler(p) {
  await p.locator('#ferramentas .fer[title^="Profiler"]').first().click();
  // `attached`, e não `visible`: a grade nasce vazia, e um `<tbody>` sem
  // linha tem altura zero -- o Playwright o considera invisível e espera
  // para sempre por uma tela que já está lá.
  await p.waitForSelector('#pfCorpo', { state: 'attached', timeout: 10000 });
  await p.waitForTimeout(500);
}

/** A cor de fundo real da caixa de estado, e o texto dela. */
async function estado(p) {
  return await p.evaluate(() => {
    const el = document.querySelector('#pfEstado .aviso');
    if (!el) return null;
    const cs = getComputedStyle(el);
    return {
      classe: el.className,
      fundo: cs.backgroundColor,
      borda: cs.borderLeftColor,
      texto: el.textContent.replace(/\s+/g, ' ').trim(),
    };
  });
}

async function api(p, op, args = {}) {
  return await p.evaluate(([o, a]) => api(o, a), [op, args]);
}

async function corrida(nav, tema) {
  const ctx = await nav.newContext({ viewport: { width: 1500, height: 950 } });
  const p = await ctx.newPage();
  const erros = [];
  p.on('pageerror', e => erros.push(String(e.message)));
  await p.addInitScript(t => {
    try { localStorage.setItem('phxsql-tema', t); } catch { /* nada */ }
  }, tema);

  await entrar(p);
  await abrirProfiler(p);
  await p.screenshot({ path: `${SAIDA}/profiler-${tema}-1-parado.png` });

  // --- observando, com arquivo ---
  await p.fill('#pfArq', LOG);
  await p.locator('#pfLigar').click();
  await p.waitForTimeout(900);
  // Tráfego com uma senha dentro, para a grade ter o que mostrar.
  for (let i = 0; i < 6; i++) {
    await api(p, 'ping', { senha: SENTINELA }).catch(() => {});
    await api(p, 'bancos').catch(() => {});
  }
  await p.waitForTimeout(1600);
  const obs = await estado(p);
  achados.push({ tema, momento: 'observando', ...obs });
  if (!/bom/.test(obs.classe || '')) {
    falhas.push(`${tema}: a caixa de «observando» não usa a classe verde (${obs.classe})`);
  }
  await p.screenshot({ path: `${SAIDA}/profiler-${tema}-2-observando.png` });

  const grade = await p.evaluate(() => document.querySelector('#pfCorpo').textContent);
  if (grade.includes(SENTINELA)) falhas.push(`${tema}: a senha apareceu na grade`);
  if (!grade.includes('***')) falhas.push(`${tema}: a grade não mostrou nenhum ***`);
  achados.push({ tema, momento: 'grade', tem_sentinela: grade.includes(SENTINELA),
                 tem_estrelas: grade.includes('***') });

  // --- disco cheio ---
  if (CHEIO) {
    await p.locator('#pfParar').click();
    await p.waitForTimeout(600);
    // O tmpfs guarda o que a corrida anterior escreveu: sem apagar, o
    // segundo tema começa com o disco já cheio e mede outra coisa.
    rmSync(CHEIO, { force: true });
    await p.fill('#pfArq', CHEIO);
    await p.locator('#pfLigar').click();
    await p.waitForTimeout(700);
    // Enchimento de 1,5 kB por pedido: enche os 64 kB em algumas dezenas de
    // idas, em vez de centenas -- e de quebra mostra o corpo grande na grade.
    const enchimento = 'x'.repeat(1500);
    for (let i = 0; i < 90; i++)
      await api(p, 'ping', { enchimento }).catch(() => {});
    await p.waitForTimeout(1600);
    const cheio = await estado(p);
    achados.push({ tema, momento: 'disco cheio', ...cheio });
    if (!/mal/.test(cheio.classe || ''))
      falhas.push(`${tema}: disco cheio e a caixa não ficou vermelha (${cheio.classe})`);
    if (!/NÃO gravada/.test(cheio.texto || ''))
      falhas.push(`${tema}: disco cheio e a caixa não diz que perdeu linha`);
    await p.screenshot({ path: `${SAIDA}/profiler-${tema}-3-disco-cheio.png` });
    await p.locator('#pfParar').click().catch(() => {});
  }

  // Rolagem lateral do corpo: o mal que o CSS global costuma fazer.
  const larguras = await p.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    innerWidth: window.innerWidth,
  }));
  if (larguras.scrollWidth > larguras.innerWidth + 1)
    falhas.push(`${tema}: a página rola de lado (${larguras.scrollWidth} > ${larguras.innerWidth})`);
  if (erros.length) falhas.push(`${tema}: erro de página: ${erros.join(' | ')}`);
  await ctx.close();
}

mkdirSync(SAIDA, { recursive: true });
const nav = await chromium.launch();
for (const tema of ['escuro', 'claro']) {
  try {
    await corrida(nav, tema);
  } catch (e) {
    falhas.push(`${tema}: ${String(e).split('\n')[0]}`);
    console.log('--- erro completo ---\n' + String(e.stack || e));
  }
}
await nav.close();
writeFileSync(`${SAIDA}/relatorio-profiler.json`,
              JSON.stringify({ achados, falhas }, null, 1));
for (const a of achados) console.log(JSON.stringify(a));
console.log(`\n== ${falhas.length} falha(s) ==`);
for (const f of falhas) console.log('  ' + f);
process.exit(falhas.length ? 1 : 0);

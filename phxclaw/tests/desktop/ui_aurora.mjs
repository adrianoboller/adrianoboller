// Prova, EXERCITANDO no Chromium, do HUD retratil AURORA -- a camada ADICIONAL sobre a vista
// Conversa (aurora.js), no molde que o dono desenhou dos monitores do filme Avatar e da janela
// de voz do Homem de Ferro, mas com nome e arte PROPRIOS:
//   * o HUD aparece na vista Conversa, com persona, estado «ao vivo» e a leitura;
//   * a leitura REPETE o ultimo balao como DADO -- igual ao texto do balao, sem traducao e sem
//     maiuscula forcada (a lei do «Blumenau» aplicada a narracao);
//   * o microfone gravando acende a onda de voz (estado «ouvindo»);
//   * recolhe para o orb e o orb reabre o HUD;
//   * trocar a persona muda o nome e o tom (--au), sem recarregar;
//   * NAO substitui nada: o Kanban, a realizacao, o chat e o mic seguem de pe.
//
// RED medido (no fim): a COPIA da UI SEM o <script src="./assets/aurora.js"> nao tem HUD nenhum
// (window.aurora some, #auroraHud nao existe) -- o teste FALHA sem o conserto e passa com ele.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_aurora.mjs [--ui DIR]
import { mkdtempSync, cpSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { subir } from './qualificacao/servidor.mjs';
import { chromium, UI, CAP, CONFIG } from './qualificacao/comum.mjs';

const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}

// Microfone stubado: um MediaStream real (oscilador) para o caminho Web Audio do conversa.js
// rodar de verdade; e a Aurora so le a classe 'gravando' que o botao ganha.
function stubMic() {
  const AC = window.AudioContext || window.webkitAudioContext;
  const ac = new AC();
  const dst = ac.createMediaStreamDestination();
  const osc = ac.createOscillator(); osc.frequency.value = 440; osc.connect(dst); osc.start();
  if (!navigator.mediaDevices) navigator.mediaDevices = {};
  navigator.mediaDevices.getUserMedia = async () => dst.stream;
}

async function abrirConversa(browser, origem, tema = 'escuro') {
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 820 }, locale: 'pt-BR', colorScheme: tema === 'claro' ? 'light' : 'dark' });
  await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); localStorage.setItem('phxclaw.tema', t); } catch {} }, tema);
  await ctx.addInitScript(stubMic);
  const page = await ctx.newPage();
  // o inspecao.js (bloqueio do inspetor, laco anti-automacao) atrapalha o CDP; e irrelevante para
  // a Aurora, entao e abortado so no roteiro.
  await page.route('**/inspecao.js', r => r.abort());
  // 'commit' em vez de 'load': neste ambiente o proxy segura algum subrecurso e o evento `load`
  // nunca chega; a tela ja e interativa no commit, e as esperas abaixo sao por seletor.
  await page.goto(`${origem}/index.html?screen=dashboard`, { waitUntil: 'commit' });
  await page.waitForSelector('.nav[data-tela="conversa"]', { state: 'attached', timeout: 15000 });
  await page.waitForTimeout(700);
  await page.click('.nav[data-tela="conversa"]', { force: true });
  await page.waitForTimeout(400);
  // criar um projeto para liberar a corpo da Conversa
  await page.click('#projetoSeletor');
  await page.waitForSelector('#projetoDialogo[open]', { timeout: 3000 });
  await page.click('#projetoCriarNovo');
  await page.fill('#projetoCriarArea .proj-nome', 'App de Finanças');
  await page.click('#projetoCriarArea .acao.inclui');
  await page.waitForTimeout(500);
  return { ctx, page };
}

const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const browser = await chromium.launch();
try {
  const { ctx, page } = await abrirConversa(browser, ORIG);

  // A — o HUD existe na vista Conversa, com o convite inicial
  const inicio = await page.evaluate(() => {
    const h = document.getElementById('auroraHud');
    return { existe: !!h, visivel: !!h && !h.hidden, leitura: (document.querySelector('.au-leitura')?.textContent || '').trim(), nome: (document.querySelector('.au-nome')?.textContent || '').trim() };
  });
  check('aurora: o HUD aparece na vista Conversa', inicio.existe && inicio.visivel, JSON.stringify(inicio));
  check('aurora: a persona padrao se chama Aurora', inicio.nome === 'Aurora', inicio.nome);

  // B — enviar um pedido vira balao, e a leitura REPETE o balao como DADO
  await page.fill('#conversaCampo', 'Adicione login com 2FA ao sistema');
  await page.click('#conversaEnviar');
  await page.waitForTimeout(400);
  await page.waitForFunction(() => document.querySelector('#conversaHistorico .balao-agente .balao-texto'), { timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(400);
  const leitura = await page.evaluate(() => {
    const baloes = document.querySelectorAll('#conversaHistorico .balao .balao-texto');
    const ultimo = baloes.length ? baloes[baloes.length - 1].textContent.trim() : '';
    return { ultimo, aurora: (document.querySelector('.au-leitura')?.textContent || '').trim() };
  });
  check('aurora: a leitura repete o ULTIMO balao como DADO (igual, sem maiuscula)', leitura.aurora.length > 0 && leitura.aurora === leitura.ultimo, JSON.stringify(leitura).slice(0, 120));

  // C — microfone gravando acende a onda (estado «ouvindo»)
  await page.fill('#conversaCampo', '');
  await page.click('#conversaMic');
  await page.waitForTimeout(500);
  const gravando = await page.evaluate(() => ({ mic: !!document.querySelector('#conversaMic.gravando'), onda: !!document.querySelector('.au-onda.on') }));
  check('aurora: o microfone gravando acende a onda de voz', gravando.mic && gravando.onda, JSON.stringify(gravando));
  await page.click('#conversaMic'); // parar
  await page.waitForTimeout(400);
  const parou = await page.evaluate(() => !document.querySelector('.au-onda.on'));
  check('aurora: parar o microfone apaga a onda', parou);

  // captura: HUD expandido
  await page.screenshot({ path: join(CAP, 'aurora-expandido.png') });

  // D — recolher para o orb e reabrir
  await page.click('.au-min');
  await page.waitForTimeout(600);
  const recolhido = await page.evaluate(() => ({ hud: document.getElementById('auroraHud').hidden, orb: !document.getElementById('auroraOrb').hidden }));
  check('aurora: recolher esconde o HUD e mostra o orb', recolhido.hud && recolhido.orb, JSON.stringify(recolhido));
  await page.screenshot({ path: join(CAP, 'aurora-orb.png') });
  await page.click('.aurora-orb');
  await page.waitForTimeout(500);
  const reaberto = await page.evaluate(() => !document.getElementById('auroraHud').hidden && document.getElementById('auroraOrb').hidden);
  check('aurora: o orb reabre o HUD', reaberto);

  // E — trocar a persona muda o nome e o tom
  const antes = await page.evaluate(() => ({ nome: document.querySelector('.au-nome').textContent.trim(), au: document.getElementById('auroraHud').style.getPropertyValue('--au') }));
  await page.click('.au-persona');
  await page.waitForTimeout(300);
  const depois = await page.evaluate(() => ({ nome: document.querySelector('.au-nome').textContent.trim(), au: document.getElementById('auroraHud').style.getPropertyValue('--au') }));
  check('aurora: trocar a persona muda o nome e o tom (--au)', depois.nome !== antes.nome && depois.au !== antes.au, `${antes.nome}/${antes.au} -> ${depois.nome}/${depois.au}`);

  // F — nada foi substituido: Kanban, chat e mic seguem na tela
  const intacto = await page.evaluate(() => ({ kanban: !!document.querySelector('.kanban'), campo: !!document.getElementById('conversaCampo'), mic: !!document.getElementById('conversaMic') }));
  check('aurora: a Conversa segue inteira (Kanban, campo, microfone)', intacto.kanban && intacto.campo && intacto.mic, JSON.stringify(intacto));

  await ctx.close();

  // --- RED: copia da UI SEM o <script src="./assets/aurora.js"> nao tem HUD nenhum -----------
  const red = mkdtempSync(join(tmpdir(), 'ui-aurora-red-'));
  cpSync(UI, red, { recursive: true });
  const idx = join(red, 'index.html');
  writeFileSync(idx, readFileSync(idx, 'utf8').replace('<script src="./assets/aurora.js"></script>', '<!-- RED: aurora.js removido -->'));
  const { srv: srv2, porta: p2 } = await subir(red, { config: CONFIG });
  const { ctx: ctx2, page: page2 } = await abrirConversa(browser, `http://127.0.0.1:${p2}`);
  const semAurora = await page2.evaluate(() => ({ hud: !!document.getElementById('auroraHud'), api: typeof window.aurora }));
  check('RED: sem o <script> da Aurora, nao ha HUD e window.aurora some', !semAurora.hud && semAurora.api === 'undefined', JSON.stringify(semAurora));
  await ctx2.close();
  srv2.close();
} finally {
  await browser.close();
  srv.close();
}

const ok = checagens.filter(Boolean).length;
console.log(`\n${ok}/${checagens.length} verdes`);
process.exit(ok === checagens.length ? 0 : 1);

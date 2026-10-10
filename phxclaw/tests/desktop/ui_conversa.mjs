// Prova, EXERCITANDO no Chromium, da tela Conversa (projeto -> pedido -> cartao):
//   * PORTAO de projeto: sem projeto ativo, a conversa fica bloqueada; criar um projeto libera;
//   * KANBAN fixo e retratil: quatro colunas do TaskStatus, cartoes COMPACTOS, coluna com 4+
//     cartoes rolando por dentro (sem passar da altura de ~3), recolher/expandir;
//   * REALIZACAO: clicar um cartao abre o detalhe cheio -- solicitacao, linha do tempo, ajustes,
//     artefatos, agentes, inicio/fim, progresso e a PIZZA da atividade -- e «Testar em tela nova»
//     so com artefato web; cartao compacto NAO mostra a timeline; fechar volta ao quadro;
//   * CHAT: enviar um pedido vira cartao e balao, e a resposta chega como balao do agente;
//   * MICROFONE: getUserMedia/MediaRecorder stubados e /v1/voz/transcrever devolvendo
//     {texto:"olá mundo"} enchem o campo; permissao recusada vira AVISO (nao trava a tela);
//   * as duas larguras (1280 e 400) e os dois temas, sem text-transform em DADO, sem violacao
//     de CSP (o servidor de revisao manda a CSP do agente).
//
// RED medido (no fim): copia da UI SEM o <script src="./assets/conversa.js"> derruba a vista;
// copia da conversa.js SEM a guarda de permissao do microfone trava na recusa.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_conversa.mjs [--ui DIR]
import { writeFileSync, mkdirSync, cpSync, readFileSync, mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { subir } from './qualificacao/servidor.mjs';
import { vigiarCsp } from './seguranca.mjs';
import { chromium, UI, OUT, CONFIG } from './qualificacao/comum.mjs';

const SAIDA = join(OUT, '..');
mkdirSync(SAIDA, { recursive: true });
const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}

// Stub do microfone: getUserMedia devolve um MediaStream REAL (um oscilador num
// MediaStreamDestination), para o caminho Web Audio da tela rodar de verdade -- captura PCM,
// reamostra e monta o WAV. Sem hardware, mas pelo MESMO codigo que vai para producao.
function stubMic() {
  const AC = window.AudioContext || window.webkitAudioContext;
  const ac = new AC();
  const dst = ac.createMediaStreamDestination();
  const osc = ac.createOscillator(); osc.frequency.value = 440; osc.connect(dst); osc.start();
  if (!navigator.mediaDevices) navigator.mediaDevices = {};
  navigator.mediaDevices.getUserMedia = async () => dst.stream;
  window.__rejeicoes = [];
  window.addEventListener('unhandledrejection', e => { window.__rejeicoes.push(String(e.reason)); });
}

async function novoContexto(browser, vp, tema = 'escuro') {
  const ctx = await browser.newContext({ viewport: vp, locale: 'pt-BR', colorScheme: tema === 'claro' ? 'light' : 'dark', ...(vp.width <= 480 ? { isMobile: true, hasTouch: true } : {}) });
  const vigia = await vigiarCsp(ctx);
  // Tema fixo pela escolha guardada (tema.js): o Chromium do Playwright abre em «light» por
  // padrao, entao sem fixar o tema a captura «escuro» sairia clara.
  await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); localStorage.setItem('phxclaw.tema', t); } catch {} }, tema);
  await ctx.addInitScript(stubMic);
  return { ctx, vigia };
}

async function irParaConversa(page, origem) {
  await page.goto(`${origem}/index.html?screen=dashboard`);
  await page.waitForTimeout(500);
  await page.click('.nav[data-tela="conversa"]');
  await page.waitForTimeout(400);
}

async function criarProjeto(page, nome) {
  await page.click('#projetoSeletor');
  await page.waitForSelector('#projetoDialogo[open]', { timeout: 3000 });
  await page.click('#projetoCriarNovo');
  await page.fill('#projetoCriarArea .proj-nome', nome);
  await page.click('#projetoCriarArea .acao.inclui');
  await page.waitForTimeout(600);
}

const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const browser = await chromium.launch();
try {
  const { ctx, vigia } = await novoContexto(browser, { width: 1280, height: 860 });
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  await irParaConversa(page, ORIG);

  // --- A. PORTAO: sem projeto, gate a mostra e o corpo fica bloqueado -----------------------
  const gate0 = await page.evaluate(() => ({ gate: !document.getElementById('conversaGate').hidden, corpo: !document.getElementById('conversaCorpo').hidden, seletor: document.getElementById('projetoSeletorNome').textContent }));
  check('portao: sem projeto, o gate aparece e o corpo (kanban+entrada) fica bloqueado', gate0.gate && !gate0.corpo, JSON.stringify(gate0));
  check('portao: o seletor do topo diz «Nenhum projeto» pela fabrica', /Nenhum projeto/.test(gate0.seletor), gate0.seletor);
  await page.screenshot({ path: join(SAIDA, 'ui_conversa_gate.png') });

  // --- B. CRIAR PROJETO libera a conversa ---------------------------------------------------
  await criarProjeto(page, 'Forca');
  const liberou = await page.evaluate(() => ({ gate: !document.getElementById('conversaGate').hidden, corpo: !document.getElementById('conversaCorpo').hidden, seletor: document.getElementById('projetoSeletorNome').textContent, ativo: (window.conversa.projetoAtivo() || {}).id }));
  check('portao: criar um projeto esconde o gate e mostra o corpo', !liberou.gate && liberou.corpo, JSON.stringify(liberou));
  check('portao: o projeto «Forca» vira o ativo e o seletor mostra o nome (dado)', liberou.ativo === 'forca' && liberou.seletor === 'Forca', JSON.stringify(liberou));

  // --- C. KANBAN: quatro colunas, cartoes compactos, coluna cheia rolando por dentro --------
  const cols = await page.$$eval('#kanbanQuadro .kanban-col', cs => cs.map(c => c.dataset.col));
  check('kanban: as quatro colunas na ordem Backlog · Fazendo · Feito · Parado', cols.join(',') === 'backlog,fazendo,feito,parado', cols.join(','));
  const naoTimeline = await page.$$eval('#kanbanQuadro .det-timeline', e => e.length);
  check('kanban: o cartao no quadro e COMPACTO (sem linha do tempo)', naoTimeline === 0, String(naoTimeline));
  const rolagem = await page.evaluate(() => {
    const corpo = document.querySelector('.kanban-col-fazendo .kanban-col-corpo');
    const n = document.querySelectorAll('.kanban-col-fazendo .kanban-card').length;
    return { n, scroll: corpo.scrollHeight, client: corpo.clientHeight };
  });
  check('kanban: coluna «Fazendo» com 4+ cartoes rola por dentro e nao estica (<=240px)', rolagem.n >= 4 && rolagem.scroll > rolagem.client && rolagem.client <= 240, JSON.stringify(rolagem));
  await page.screenshot({ path: join(SAIDA, 'ui_conversa_1280_escuro.png') });

  // --- D. DETALHE do cartao rico (os oito blocos) -------------------------------------------
  await page.click('.kanban-card[data-id="forca~k3"]');
  await page.waitForTimeout(500);
  const det = await page.evaluate(() => {
    const h = document.getElementById('conversaHistorico');
    return {
      solicitacao: h.querySelector('.balao-usuario .balao-texto')?.textContent || '',
      timeline: h.querySelectorAll('.det-timeline li').length,
      ajustes: h.querySelectorAll('.det-ajustes li').length,
      artefatos: h.querySelectorAll('.det-artefatos li').length,
      agente: h.querySelector('.det-agente b')?.textContent || '',
      subs: h.querySelectorAll('.det-subs li').length,
      chips: [...h.querySelectorAll('.det-chips code')].map(c => c.textContent),
      inicioFim: h.querySelectorAll('.det-ter dd').length,
      progresso: h.querySelector('.prog-valor')?.textContent || '',
      fatias: h.querySelectorAll('.pizza path').length,
      legenda: h.querySelectorAll('.pizza-legenda li').length,
      resposta: h.querySelector('.balao-agente .balao-texto')?.textContent || '',
      testarHref: h.querySelector('.det-testar .test-nova')?.getAttribute('href') || '',
    };
  });
  check('detalhe/1 solicitacao: o balao do usuario traz o objetivo (dado, sem maiuscula forcada)', det.solicitacao.includes('Blumenau'), det.solicitacao.slice(0, 60));
  check('detalhe/2 linha do tempo: lista as 3 transicoes do historico', det.timeline === 3, String(det.timeline));
  check('detalhe/3 ajustes: lista os 2 refinamentos', det.ajustes === 2, String(det.ajustes));
  check('detalhe/4 artefatos: lista os 3 arquivos como links', det.artefatos === 3, String(det.artefatos));
  check('detalhe/5 agentes: mostra o modelo, 1 subagente e as 3 ferramentas usadas', det.agente === 'ollama:qwen2.5:7b' && det.subs === 1 && det.chips.join(',') === 'web.search,fs.read,fs.write', JSON.stringify({ a: det.agente, s: det.subs, c: det.chips }));
  check('detalhe/6 inicio e fim: dois valores (datas)', det.inicioFim === 2, String(det.inicioFim));
  check('detalhe/7 progresso: tarefa Feita mede 100%', det.progresso === '100%', det.progresso);
  check('detalhe/8 pizza: 4 fatias (web.search, fs.read, fs.write, pensamento) e legenda com 4', det.fatias === 4 && det.legenda === 4, JSON.stringify({ f: det.fatias, l: det.legenda }));
  check('detalhe: a resposta chega como balao do agente', /Resumo pronto/.test(det.resposta), det.resposta.slice(0, 50));
  check('testar: artefato web (index.html) mostra «Testar em tela nova» na URL certa do artefato', /\/v1\/tasks\/forca~k3\/artifacts\/index\.html$/.test(det.testarHref), det.testarHref);
  await page.screenshot({ path: join(SAIDA, 'ui_conversa_detalhe.png') });

  // --- E. Feito SEM artefato web: «Testar» nao aparece --------------------------------------
  await page.click('.kanban-card[data-id="forca~k6"]');
  await page.waitForTimeout(400);
  const semTestar = await page.$$eval('#conversaHistorico .det-testar .test-nova', e => e.length);
  check('testar: cartao Feito sem artefato web NAO mostra «Testar»', semTestar === 0, String(semTestar));

  // --- F. Fechar volta ao quadro (estado vazio) ---------------------------------------------
  await page.click('#conversaHistorico .realizacao-barra .acao');
  await page.waitForTimeout(300);
  const fechou = await page.evaluate(() => ({ vazio: !!document.querySelector('#conversaHistorico .conversa-vazio'), det: document.querySelectorAll('#conversaHistorico .detalhe').length }));
  check('detalhe: «voltar ao quadro» limpa o detalhe e volta ao estado vazio', fechou.vazio && fechou.det === 0, JSON.stringify(fechou));

  // --- G. CHAT: enviar um pedido vira balao e a resposta chega ------------------------------
  await page.fill('#conversaCampo', 'Crie um jogo da forca em HTML');
  await page.click('#conversaEnviar');
  await page.waitForTimeout(500);
  const aposEnviar = await page.evaluate(() => ({ usuario: document.querySelector('#conversaHistorico .balao-usuario .balao-texto')?.textContent || '', trabalhando: !!document.querySelector('#conversaHistorico .balao-trabalhando') }));
  check('chat: o pedido enviado vira balao do usuario e um balao «trabalhando»', aposEnviar.usuario === 'Crie um jogo da forca em HTML' && aposEnviar.trabalhando, JSON.stringify(aposEnviar));
  // Duas sondagens levam a tarefa criada de «em execucao» a «concluida» (o stub completa na 2a leitura).
  await page.evaluate(() => window.conversa.atualizar());
  await page.waitForTimeout(300);
  await page.evaluate(() => window.conversa.atualizar());
  await page.waitForTimeout(400);
  const resp = await page.evaluate(() => document.querySelector('#conversaHistorico .balao-agente .balao-texto')?.textContent || '');
  check('chat: a resposta do agente chega como balao (em cima da pergunta rolando)', /Pronto: Crie um jogo da forca/.test(resp), resp.slice(0, 60));

  // --- H. MICROFONE: grava WAV, para e transcreve para o campo -------------------------------
  let vozCT = null;
  page.on('request', r => { if (r.url().includes('/v1/voz/transcrever')) vozCT = r.headers()['content-type']; });
  await page.click('#conversaMic');
  await page.waitForTimeout(700);
  const gravando = await page.$eval('#conversaMic', b => b.classList.contains('gravando') && b.getAttribute('aria-pressed') === 'true');
  check('microfone: ao gravar, o botao fica vermelho (classe gravando) e aria-pressed', gravando);
  await page.click('#conversaMic');
  await page.waitForTimeout(700);
  const campo = await page.$eval('#conversaCampo', c => c.value);
  check('microfone: parar manda o audio e a transcricao {texto} cai no campo', /olá mundo/.test(campo), campo);
  check('microfone: o corpo chega como audio/wav (whisper local so le WAV)', /^audio\/wav/.test(vozCT || ''), String(vozCT));

  // --- H2. Erro da voz {error} vira aviso amigavel (503: motor offline) ----------------------
  await page.evaluate(() => fetch('/__modo?voz=503'));
  await page.fill('#conversaCampo', '');
  await page.click('#conversaMic');
  await page.waitForTimeout(700);
  await page.click('#conversaMic');
  await page.waitForTimeout(700);
  const err503 = await page.$eval('#conversaStatus', s => s.textContent);
  check('microfone: erro 503 vira aviso amigavel pela fabrica + a mensagem do servidor (dado)', /não está disponível/.test(err503) && /offline|whisper/.test(err503), err503);
  await page.evaluate(() => fetch('/__modo?voz=ok'));

  // --- I. MICROFONE recusado: AVISO claro, a tela segue viva (a guarda funciona) ------------
  await page.evaluate(() => { navigator.mediaDevices.getUserMedia = async () => { throw new DOMException('Permission denied', 'NotAllowedError'); }; });
  await page.fill('#conversaCampo', '');
  await page.click('#conversaMic');
  await page.waitForTimeout(400);
  const recusa = await page.evaluate(() => ({ aviso: document.getElementById('conversaStatus').textContent, vivo: !document.getElementById('conversaEntrada').hidden, rej: (window.__rejeicoes || []).length }));
  check('microfone: permissao recusada vira AVISO pela fabrica, sem travar nem estourar', /Microfone bloqueado/.test(recusa.aviso) && recusa.vivo && recusa.rej === 0, JSON.stringify(recusa));

  // --- I2. TETO de duracao da gravacao: a funcao pura, exercitada no NAVEGADOR (codigo real) -
  const teto = await page.evaluate(() => ({
    abaixo: window.conversa.acumularAmostras(10, 5, 100), // 15 < 100
    noteto: window.conversa.acumularAmostras(95, 5, 100), // 100 >= 100
    seg: window.conversa.SEGUNDOS_MAX_GRAVACAO,
  }));
  check('teto/funcao: abaixo do teto nao estoura; ao atingir, estoura (funcao pura no navegador)',
    teto.abaixo.estourou === false && teto.abaixo.total === 15 && teto.noteto.estourou === true && teto.noteto.total === 100, JSON.stringify(teto));
  check('teto/funcao: o teto de duracao e alguns minutos (nao zero, nao sem limite)', teto.seg >= 60 && teto.seg <= 1800, String(teto.seg));

  // --- J. KANBAN retratil: recolher cresce a area de realizacao; expandir volta -------------
  const h1 = await page.$eval('#conversaHistorico', e => e.clientHeight);
  await page.click('#kanbanAlternar');
  await page.waitForTimeout(400);
  const recolhido = await page.evaluate(() => ({ rec: document.getElementById('kanban').dataset.recolhido, quadro: document.getElementById('kanbanQuadro').clientHeight, contadores: getComputedStyle(document.getElementById('kanbanContadores')).display, hist: document.getElementById('conversaHistorico').clientHeight }));
  check('kanban retratil: recolhido esconde o quadro, mostra a faixa de contadores e a realizacao cresce', recolhido.rec === '1' && recolhido.quadro < 10 && recolhido.contadores !== 'none' && recolhido.hist > h1, JSON.stringify({ ...recolhido, antes: h1 }));
  await page.screenshot({ path: join(SAIDA, 'ui_conversa_recolhido.png') });
  await page.click('#kanbanAlternar');
  await page.waitForTimeout(300);
  const expandido = await page.$eval('#kanban', e => e.dataset.recolhido);
  check('kanban retratil: expandir volta o quadro', expandido === '0', expandido);

  // --- K. DADO nunca recebe text-transform (rotulo se estiliza; dado nao) -------------------
  await page.click('.kanban-card[data-id="forca~k3"]');
  await page.waitForTimeout(400);
  const semMaiuscula = await page.evaluate(() => {
    const alvos = ['.balao-usuario .balao-texto', '.kanban-card-obj', '.pizza-legenda .dado', '.det-agente b'];
    return alvos.map(s => { const e = document.querySelector(s); return e ? getComputedStyle(e).textTransform : 'ausente'; });
  });
  check('dado: nenhum texto de dado recebe text-transform (none em todos)', semMaiuscula.every(v => v === 'none' || v === 'ausente'), JSON.stringify(semMaiuscula));

  // --- Tema claro (1280): troca pela via do tema.js e confere que aplicou --------------------
  await page.evaluate(() => tema.trocar('claro'));
  await page.waitForTimeout(400);
  await page.screenshot({ path: join(SAIDA, 'ui_conversa_1280_claro.png') });
  const temaClaro = await page.$eval(':root', r => r.getAttribute('data-tema'));
  check('tema: o tema claro aplica sem quebrar a tela', temaClaro === 'claro', String(temaClaro));

  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | ').slice(0, 300));
  const csp = await vigia.todas(ctx.pages());
  check('zero violacao de CSP (a mesma politica do agente)', csp.length === 0, csp.slice(0, 3).join(' | '));
  await ctx.close();

  // --- Celular 400 px: kanban recolhido util + colunas rolando lado a lado, campo acessivel --
  for (const tema of ['escuro', 'claro']) {
    const { ctx: ctxM, vigia: vigiaM } = await novoContexto(browser, { width: 400, height: 780 }, tema);
    const pageM = await ctxM.newPage();
    const errosM = [];
    pageM.on('pageerror', e => errosM.push(String(e)));
    await irParaConversa(pageM, ORIG);
    await criarProjeto(pageM, 'Forca');
    const movel = await pageM.evaluate(() => {
      const q = document.getElementById('kanbanQuadro');
      const entrada = document.getElementById('conversaEntrada').getBoundingClientRect();
      return { rec: document.getElementById('kanban').dataset.recolhido, scrollX: q.scrollWidth > q.clientWidth, entradaVisivel: entrada.bottom <= window.innerHeight + 1 && entrada.width > 0 };
    });
    check(`celular 400/${tema}: a barra de entrada fica visivel (nada a esconde)`, movel.entradaVisivel, JSON.stringify(movel));
    // Expandir para ver as colunas rolando lateralmente.
    if (movel.rec === '1') { await pageM.click('#kanbanAlternar'); await pageM.waitForTimeout(300); }
    const lateral = await pageM.evaluate(() => { const q = document.getElementById('kanbanQuadro'); return q.scrollWidth > q.clientWidth; });
    check(`celular 400/${tema}: expandido, as colunas rolam lado a lado (rolagem horizontal)`, lateral);
    await pageM.screenshot({ path: join(SAIDA, `ui_conversa_400_${tema}.png`) });
    const cspM = await vigiaM.todas(ctxM.pages());
    check(`celular 400/${tema}: zero violacao de CSP e zero erro de JS`, cspM.length === 0 && errosM.length === 0, [cspM.slice(0, 2).join(' | '), errosM.slice(0, 2).join(' | ')].join(' '));
    await ctxM.close();
  }

  // ===================== RED medido =====================
  // RED1: a COPIA sem o <script src="./assets/conversa.js"> derruba a vista -- o seletor nao
  // abre dialogo, o portao nunca mostra o gate, window.conversa nao existe.
  const red1 = mkdtempSync(join(tmpdir(), 'conv-red1-'));
  cpSync(UI, red1, { recursive: true });
  const idx = join(red1, 'index.html');
  writeFileSync(idx, readFileSync(idx, 'utf8').replace('<script src="./assets/conversa.js"></script>', '<!-- RED: conversa.js removido -->'));
  const s1 = await subir(red1, { config: CONFIG });
  const O1 = `http://127.0.0.1:${s1.porta}`;
  {
    const { ctx: c1 } = await novoContexto(browser, { width: 1280, height: 860 });
    const p1 = await c1.newPage();
    await irParaConversa(p1, O1);
    await p1.click('#projetoSeletor').catch(() => {});
    await p1.waitForTimeout(300);
    const quebrou = await p1.evaluate(() => ({ temConversa: typeof window.conversa !== 'undefined', dialogoAberto: document.getElementById('projetoDialogo').open, gateMostrado: !document.getElementById('conversaGate').hidden }));
    check('RED1: sem conversa.js, o seletor nao abre dialogo, o portao nao aparece e window.conversa some', !quebrou.temConversa && !quebrou.dialogoAberto && !quebrou.gateMostrado, JSON.stringify(quebrou));
    await c1.close();
  }
  s1.srv.close();

  // RED2: a COPIA da conversa.js SEM a guarda try/catch do getUserMedia trava na recusa -- a
  // rejeicao fica sem tratamento e o AVISO claro nao aparece.
  const red2 = mkdtempSync(join(tmpdir(), 'conv-red2-'));
  cpSync(UI, red2, { recursive: true });
  const cj = join(red2, 'assets/conversa.js');
  const fonte = readFileSync(cj, 'utf8');
  const alvo = "try { fluxoMic = await navigator.mediaDevices.getUserMedia({ audio: true }); }\n    catch { return avisoStatus(txt('conversa.mic_negado', 'Microfone bloqueado: permita o acesso ao microfone ou digite o pedido.'), true); }";
  const semGuarda = 'fluxoMic = await navigator.mediaDevices.getUserMedia({ audio: true });';
  check('RED2: a guarda do microfone esta no fonte (para poder remove-la)', fonte.includes(alvo));
  writeFileSync(cj, fonte.replace(alvo, semGuarda));
  const s2 = await subir(red2, { config: CONFIG });
  const O2 = `http://127.0.0.1:${s2.porta}`;
  {
    const { ctx: c2 } = await novoContexto(browser, { width: 1280, height: 860 });
    const p2 = await c2.newPage();
    const err2 = [];
    p2.on('pageerror', e => err2.push(String(e)));
    await irParaConversa(p2, O2);
    await criarProjeto(p2, 'Forca');
    await p2.evaluate(() => { navigator.mediaDevices.getUserMedia = async () => { throw new DOMException('Permission denied', 'NotAllowedError'); }; });
    await p2.click('#conversaMic');
    await p2.waitForTimeout(400);
    const travou = await p2.evaluate(() => ({ aviso: document.getElementById('conversaStatus').textContent, rej: (window.__rejeicoes || []).length }));
    check('RED2: sem a guarda, a recusa NAO vira aviso e deixa uma rejeicao sem tratamento', !/Microfone bloqueado/.test(travou.aviso) && travou.rej > 0, JSON.stringify(travou));
    await c2.close();
  }
  s2.srv.close();

  // ===================== B2: teto de duracao da gravacao (caminho REAL) =====================
  // O PCM cru acumula em memoria ate parar; sem teto, uma gravacao esquecida enche a aba
  // (auto-DoS). Prova nos dois sentidos com COPIA da UI: o teto cai para fracao de segundo
  // (para a prova nao gravar minutos) e exercita o onaudioprocess de verdade.
  const CAP_ALVO = 'const SEGUNDOS_MAX_GRAVACAO = 300;';
  const CAP_CURTO = 'const SEGUNDOS_MAX_GRAVACAO = 0.2;';
  const PARA_ALVO = 'if (r.estourou && gravando) pararPorTeto();';

  // GREEN: com o teto curto, a gravacao para SOZINHA e avisa pelo caminho de aviso.
  const capG = mkdtempSync(join(tmpdir(), 'conv-capG-'));
  cpSync(UI, capG, { recursive: true });
  const cjG = join(capG, 'assets/conversa.js');
  const fG = readFileSync(cjG, 'utf8');
  check('B2: o teto de gravacao esta no fonte (para poder encurta-lo)', fG.includes(CAP_ALVO) && fG.includes(PARA_ALVO));
  writeFileSync(cjG, fG.replace(CAP_ALVO, CAP_CURTO));
  const sG = await subir(capG, { config: CONFIG });
  const OG = `http://127.0.0.1:${sG.porta}`;
  {
    const { ctx: cG } = await novoContexto(browser, { width: 1280, height: 860 });
    const pG = await cG.newPage();
    await irParaConversa(pG, OG);
    await criarProjeto(pG, 'Forca');
    await pG.click('#conversaMic');
    await pG.waitForTimeout(1500); // > teto: o onaudioprocess acumula e dispara a parada
    const parou = await pG.evaluate(() => ({
      gravando: document.getElementById('conversaMic').classList.contains('gravando'),
      aviso: document.getElementById('conversaStatus').textContent,
    }));
    check('B2-GREEN: no teto, a gravacao para sozinha (sai de gravando) e avisa', !parou.gravando && /muito longa|too long/.test(parou.aviso), JSON.stringify(parou));
    await cG.close();
  }
  sG.srv.close();

  // RED: a COPIA com a parada por teto REMOVIDA (defeito reposto) grava sem fim e nao avisa.
  const capR = mkdtempSync(join(tmpdir(), 'conv-capR-'));
  cpSync(UI, capR, { recursive: true });
  const cjR = join(capR, 'assets/conversa.js');
  const fR = readFileSync(cjR, 'utf8').replace(CAP_ALVO, CAP_CURTO).replace(PARA_ALVO, '/* RED: sem parada por teto */');
  check('B2-RED: a chamada de parada por teto estava no fonte (para poder remove-la)', fR.includes('/* RED: sem parada por teto */'));
  writeFileSync(cjR, fR);
  const sR = await subir(capR, { config: CONFIG });
  const OR = `http://127.0.0.1:${sR.porta}`;
  {
    const { ctx: cR } = await novoContexto(browser, { width: 1280, height: 860 });
    const pR = await cR.newPage();
    await irParaConversa(pR, OR);
    await criarProjeto(pR, 'Forca');
    await pR.click('#conversaMic');
    await pR.waitForTimeout(1500);
    const seguiu = await pR.evaluate(() => ({
      gravando: document.getElementById('conversaMic').classList.contains('gravando'),
      aviso: document.getElementById('conversaStatus').textContent,
    }));
    check('B2-RED: sem a parada por teto, a gravacao NAO para e nao ha aviso de teto', seguiu.gravando && !/muito longa|too long/.test(seguiu.aviso), JSON.stringify(seguiu));
    await pR.click('#conversaMic'); // para manualmente: nao deixar a aba gravando sem fim
    await cR.close();
  }
  sR.srv.close();
} catch (e) {
  check('roteiro', false, String(e.stack || e).slice(0, 400));
} finally {
  await browser.close();
  srv.close();
}
writeFileSync(join(SAIDA, 'ui_conversa.json'), JSON.stringify({ ui: UI, checagens }, null, 1));
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);

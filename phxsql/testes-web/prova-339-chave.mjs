#!/usr/bin/env node
/* Prova real, no navegador, do pedido 339(a): a chave da API da Claude, o
 * desligamento administrativo e a aprovacao do que sai.
 *
 *     node testes-web/prova-339-chave.mjs [--binario <phxsqld>] [--porta <n>]
 *
 * Sai 0 quando tudo vale, 1 quando algo falha. A pagina e embutida no
 * binario (`include_str!`): com o binario de ANTES do conserto a prova
 * reprova -- a chave aparece no `localStorage` (ate 23/09/2026) ou no
 * `sessionStorage` (ate 08/10/2026), o Perguntar manda sem aprovacao, e o
 * `integracao_claude: false` nao tira a Anthropic do CSP.
 *
 * O dano se mede pelo EFEITO, nunca pelo recado:
 *   - CHAVE: os dois armazenamentos do navegador lidos CRUS, chave a chave,
 *     procurando a chave -- e nao o que o modulo diz de si;
 *   - APROVACAO: os pedidos que CHEGAM na rota da Anthropic (interceptada,
 *     a chave e fabricada), contados antes e depois de cada clique;
 *   - DESLIGADA: o cabecalho CSP de verdade, e um `fetch` direto a
 *     Anthropic feito de dentro da pagina -- o navegador tem de barrar.
 *
 * Os controles (sem eles o verde podia ser pagina quebrada): a chave
 * FUNCIONA nesta aba depois de salva, e a chamada aprovada CHEGA. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { entrar, api } from './apoio.mjs';
import { abrirConfigClaude, abrirQuery, abrirPainelIA, escolherReceita, definirDb,
         cenarioParaIA } from './claude-apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i >= 0 ? process.argv[i + 1] : padrao;
};
const BINARIO = resolve(arg('--binario', resolve(AQUI, '..', 'target', 'debug', 'phxsqld')));
const PORTA = Number(arg('--porta', '6350'));

// Fabricada: a forma de uma chave, nunca o valor de uma real.
const CHAVE = 'sk-ant-prova-339-FABRICADA-0123456789AB';
const URL_API = 'https://api.anthropic.com/v1/messages';
const ORIGEM_API = 'https://api.anthropic.com';

const resultados = [];
function registrar(id, ok, detalhe) {
  resultados.push({ id, ok, detalhe });
  console.log(`${ok ? 'VERDE   ' : 'VERMELHO'} ${id} -- ${detalhe}`);
}
/* Um passo que estoura (seletor que nao aparece no binario velho) e
 * VERMELHO com o motivo, e nao a prova inteira caindo sem dizer qual. */
async function passo(id, fn) {
  try { await fn(); } catch (e) { registrar(id, false, `estourou: ${String(e.message || e).split('\n')[0]}`); }
}

/** Os dois armazenamentos, crus: toda chave e todo valor. */
const armazens = page => page.evaluate(() => {
  const tudo = a => { let t = ''; for (let i = 0; i < a.length; i++) { const k = a.key(i); t += `${k}=${a.getItem(k)}\n`; } return t; };
  return { local: tudo(localStorage), sessao: tudo(sessionStorage) };
});

/** Liga a integracao PELA TELA (o caminho do botao Salvar), e nao por um
 *  atalho do modulo: assim a prova roda igual contra o binario velho, e o
 *  vermelho dele e o defeito, nao um `_gravar is not a function`. */
async function ligarPelaTela(page) {
  await abrirConfigClaude(page);
  await page.fill('#iaChave', CHAVE);
  await page.check('#iaLigado');
  await page.click('#iaSalvar');
  await page.waitForSelector('#iaRemover:not([disabled])', { timeout: 8000 });
}

function sse(texto) {
  const ev = (tipo, dados) => `event: ${tipo}\ndata: ${JSON.stringify(dados)}\n\n`;
  return ev('message_start', { type: 'message_start', message: { id: 'm', type: 'message', role: 'assistant',
      model: 'prova', content: [], stop_reason: null, usage: { input_tokens: 5, output_tokens: 0 } } })
    + ev('content_block_start', { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } })
    + ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: texto } })
    + ev('content_block_stop', { type: 'content_block_stop', index: 0 })
    + ev('message_delta', { type: 'message_delta', delta: { stop_reason: 'end_turn' }, usage: { output_tokens: 3 } })
    + ev('message_stop', { type: 'message_stop' });
}

/** A rota da Anthropic interceptada; devolve a lista do que CHEGOU. */
async function interceptar(ctx) {
  const chegou = [];
  const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*',
                 'access-control-allow-methods': 'POST, OPTIONS' };
  await ctx.route(URL_API, async rota => {
    const r = rota.request();
    if (r.method() === 'OPTIONS') { await rota.fulfill({ status: 204, headers: CORS }); return; }
    chegou.push({ chave: r.headers()['x-api-key'] || '', corpo: r.postData() || '' });
    await rota.fulfill({ status: 200, headers: { ...CORS, 'content-type': 'text/event-stream' },
      body: sse('SELECT nome FROM clientes') });
  });
  return chegou;
}

const navegador = await chromium.launch();
let srv = null, srvDesligado = null;
try {
  srv = await subir({ phxsqld: BINARIO, portaDados: PORTA, portaWeb: PORTA + 1 });
  const url = `http://127.0.0.1:${PORTA + 1}/`;

  // ------------------------------------------- 1. a chave nao fica escrita
  await passo('chave_salva_pela_tela_nao_fica_no_navegador', async () => {
    const ctx = await navegador.newContext();
    const chegou = await interceptar(ctx);
    const page = await ctx.newPage();
    await entrar(page, url);
    await abrirConfigClaude(page);
    await page.fill('#iaChave', CHAVE);
    await page.check('#iaLigado');
    await page.click('#iaSalvar');
    await page.waitForSelector('#iaRemover:not([disabled])', { timeout: 8000 });
    const a = await armazens(page);
    const noLocal = a.local.includes(CHAVE), naSessao = a.sessao.includes(CHAVE);
    registrar('chave_salva_pela_tela_nao_fica_no_navegador', !noLocal && !naSessao,
      `localStorage tem a chave: ${noLocal}; sessionStorage tem a chave: ${naSessao}`);

    // Controle: a chave FUNCIONA nesta aba -- sem isto o verde de cima podia
    // ser so uma tela que nao grava nada.
    await page.click('#iaTestar');
    await page.waitForFunction(() => document.querySelector('#iaRecado .aviso.bom, #iaRecado .aviso.mal'),
      undefined, { timeout: 10000 });
    const levou = chegou.some(c => c.chave === CHAVE);
    registrar('controle_a_chave_funciona_nesta_aba', levou,
      `pedidos que chegaram a Anthropic com a chave: ${chegou.filter(c => c.chave === CHAVE).length}`);

    // Recarregar e entrar de novo: a chave caiu junto com o login.
    await entrar(page, url);
    const depois = await page.evaluate(() => window.PhxIA._cfg().chave || '');
    const a2 = await armazens(page);
    registrar('a_chave_cai_ao_recarregar_como_o_login', depois === '' && !a2.local.includes(CHAVE) && !a2.sessao.includes(CHAVE),
      `chave no modulo depois do F5: ${depois ? 'SIM' : 'nao'}`);
    await ctx.close();
  });

  // ---------------------------------- 2. a chave velha sai dos dois lugares
  await passo('chave_velha_sai_dos_dois_armazenamentos', async () => {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    await entrar(page, url);
    await page.evaluate(([k]) => {
      localStorage.setItem('phxsql.ia', JSON.stringify({ chave: k, ligado: true, modelo: 'claude-opus-5' }));
      sessionStorage.setItem('phxsql.ia.chave', JSON.stringify({ chave: k }));
    }, [CHAVE]);
    await abrirConfigClaude(page);
    const a = await armazens(page);
    const naMemoria = await page.evaluate(() => window.PhxIA._cfg().chave || '');
    const aviso = await page.$('#iaMigrada');
    registrar('chave_velha_sai_dos_dois_armazenamentos',
      !a.local.includes(CHAVE) && !a.sessao.includes(CHAVE) && naMemoria === CHAVE && !!aviso,
      `local: ${a.local.includes(CHAVE)}, sessao: ${a.sessao.includes(CHAVE)}, memoria: ${naMemoria === CHAVE}, aviso: ${!!aviso}`);
    await ctx.close();
  });

  // -------------------------------------------- 3. nada sai sem aprovacao
  await passo('perguntar_nao_envia_sem_aprovacao', async () => {
    const ctx = await navegador.newContext();
    const chegou = await interceptar(ctx);
    const page = await ctx.newPage();
    await entrar(page, url);
    await cenarioParaIA(page, api, 'prova339');
    await ligarPelaTela(page);
    await abrirQuery(page);
    await abrirPainelIA(page);
    await escolherReceita(page, 'sql');
    await definirDb(page, 'prova339');
    await page.fill('#iaPergunta', 'os clientes de Blumenau');
    await page.click('#iaIr');
    // Espera o painel do que vai subir; o binario velho o desenha JUNTO da
    // chamada, entao da tempo de a chamada chegar se ela vai chegar.
    await page.waitForSelector('#iaEnvio details.nota', { timeout: 15000 });
    await page.waitForTimeout(800);
    registrar('perguntar_nao_envia_sem_aprovacao', chegou.length === 0,
      `pedidos que chegaram antes de aprovar: ${chegou.length}`);

    await page.click('#iaNaoEnviar', { timeout: 5000 });
    await page.waitForSelector('#iaNaoEnviado', { timeout: 5000 });
    registrar('nao_enviar_nao_envia', chegou.length === 0, `pedidos depois de «Nao enviar»: ${chegou.length}`);

    await page.click('#iaIr');
    await page.waitForSelector('#iaAprovar', { timeout: 15000 });
    const mostrado = await page.$$eval('#iaEnvio pre.dado', p => p[1].textContent);
    await page.click('#iaAprovar');
    await page.waitForFunction(() => document.querySelector('#iaTokens b'), undefined, { timeout: 15000 });
    const igual = chegou.length === 1
      && JSON.stringify(JSON.parse(chegou[0].corpo)) === JSON.stringify(JSON.parse(mostrado));
    registrar('aprovado_sai_exatamente_o_que_foi_mostrado', igual,
      `pedidos: ${chegou.length}; corpo igual ao mostrado: ${chegou.length ? JSON.stringify(JSON.parse(chegou[0].corpo)) === JSON.stringify(JSON.parse(mostrado)) : '-'}`);
    await ctx.close();
  });

  // ---------------------------------- 4. desligada pelo administrador
  srvDesligado = await subir({ phxsqld: BINARIO, portaDados: PORTA + 2, portaWeb: PORTA + 3,
                               webExtra: { integracao_claude: false } });
  const urlD = `http://127.0.0.1:${PORTA + 3}/`;
  await passo('desligada_o_csp_nao_traz_a_anthropic', async () => {
    const ctx = await navegador.newContext();
    const chegou = await interceptar(ctx);
    const page = await ctx.newPage();
    let csp = '';
    page.on('response', r => {
      if (r.request().resourceType() === 'document') csp = r.headers()['content-security-policy'] || csp;
    });
    await entrar(page, urlD);
    registrar('desligada_o_csp_nao_traz_a_anthropic', !!csp && !csp.includes(ORIGEM_API),
      `connect-src: ${(/connect-src[^;]*/.exec(csp) || ['(sem CSP)'])[0]}`);

    // O navegador barra mesmo uma chamada feita por fora da tela.
    const r = await page.evaluate(async u => {
      try { await fetch(u, { method: 'POST', body: '{}' }); return 'saiu'; }
      catch (e) { return 'barrada'; }
    }, URL_API);
    registrar('desligada_o_navegador_barra_o_fetch', r === 'barrada' && chegou.length === 0,
      `fetch direto: ${r}; pedidos que chegaram: ${chegou.length}`);

    // A tela diz por que, e nao oferece campo de chave.
    await abrirConfigClaude(page).catch(() => {});
    await page.waitForSelector('#iaDesligadaAdm, #iaSalvar', { timeout: 10000 });
    const aviso = !!(await page.$('#iaDesligadaAdm'));
    const campo = !!(await page.$('#iaChave'));
    registrar('desligada_a_tela_diz_e_nao_pede_chave', aviso && !campo,
      `aviso: ${aviso}; campo de chave: ${campo}`);

    await ctx.close();
  });

  await passo('desligada_a_query_nao_tem_botao', async () => {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    await entrar(page, urlD);
    // Mesmo com chave e interruptor, a Query nao ganha botao. Pela tela
    // quando ela oferece o campo (binario velho); pelo modulo quando nao
    // oferece -- o que simula uma tela adulterada.
    await abrirConfigClaude(page).catch(() => {});
    await page.waitForSelector('#iaDesligadaAdm, #iaSalvar', { timeout: 10000 });
    if (await page.$('#iaChave')) await ligarPelaTela(page);
    else await page.evaluate(([k]) => window.PhxIA._gravar({ chave: k, ligado: true }), [CHAVE]);
    await abrirQuery(page);
    const botao = !!(await page.$('#btIA'));
    registrar('desligada_a_query_nao_tem_botao', !botao, `#btIA presente: ${botao}`);
    await ctx.close();
  });

  // Controle do comportamento velho: sem o campo, o CSP traz a Anthropic.
  await passo('ligada_por_padrao_o_csp_traz_a_anthropic', async () => {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    let csp = '';
    page.on('response', r => {
      if (r.request().resourceType() === 'document') csp = r.headers()['content-security-policy'] || csp;
    });
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    registrar('ligada_por_padrao_o_csp_traz_a_anthropic', csp.includes(`connect-src 'self' ${ORIGEM_API}`),
      `connect-src: ${(/connect-src[^;]*/.exec(csp) || ['(sem CSP)'])[0]}`);
    await ctx.close();
  });
} finally {
  await navegador.close();
  await srv?.derrubar?.();
  await srvDesligado?.derrubar?.();
}

const vermelhos = resultados.filter(r => !r.ok).length;
console.log(`\n${resultados.length - vermelhos} verde(s), ${vermelhos} vermelho(s)`);
process.exit(vermelhos ? 1 : 0);

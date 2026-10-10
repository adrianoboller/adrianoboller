/* O PERFIL VISUAL do aquario (pedido 783), de ponta a ponta pelo navegador:
 *
 *   .aqe-salvar  .aqe-fabrica  .aqe-descartar  .aqe-zero[data-cor]
 *
 * O que se MEDE, no tema da corrida:
 *   previa    os dois tanques da pre-visualizacao (escuro e claro) desenham
 *             as seis cores, e o tom escolhido aparece no tanque do tema dele
 *             antes de salvar;
 *   tv        a TV de parede (`?tela=aquario&tv=1`), aberta ANTES de salvar e
 *             sem recarregar, passa a desenhar a bolha vermelha com o tom e o
 *             raio novos -- o perfil vem do servidor, pela digital do retrato;
 *   tela      o aquario normal, aberto depois, desenha igual;
 *   recusa    um tom com contraste abaixo de 3:1 contra o fundo e recusado
 *             pelo SERVIDOR, com o motivo na tela, e o perfil gravado nao muda;
 *   botoes    descartar, de fabrica (um tom) e tudo de fabrica respondem.
 *
 * A bolha vermelha e SIMULADA dentro do retrato real (`route.fetch`), como
 * na prova do 780: provocar um alarme de verdade nao e o que se prova aqui.
 * O raio dela sai do `ms` >= 30 s, que e o teto: o raio desenhado E o
 * `raio_max` do perfil (o tanque e grande e a escala fica 1).
 *
 * O perfil do servidor volta ao que era no fim, pelo `config_gravar`. */
import { entrar, api, verdade, igual, contem, capturar, clicarOuExplicar, Falha, CREDENCIAL } from '../apoio.mjs';

const ESPERA = 20000;
const ID = 'dados:783#1';
let passo = 'inicio';

/* O tom escolhido por tema: vermelho de verdade, com contraste folgado
 * contra o fundo daquele tema. E o tom RUIM, que some na agua. */
const TOM = { escuro: '#ff3030', claro: '#9e1010' };
const RUIM = { escuro: '#2a0606', claro: '#ffd6d6' };
const RAIO = 60;

export const caso = {
  nome: 'perfil-do-aquario',
  async rodar(ctx) {
    const desfazer = [];
    try {
      await corpo(ctx, desfazer);
    } catch (e) {
      const tela = await ctx.page.evaluate(() => ({
        titulo: document.querySelector('#titulo')?.textContent,
        res: document.querySelector('#cfAquario .aqe-res')?.textContent,
      })).catch(() => ({}));
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]} -- tela: ${JSON.stringify(tela)}`);
    } finally {
      for (const f of desfazer.reverse()) { try { await f(); } catch { /* ja desfeito */ } }
    }
  },
};

/* A TV de parede: a arvore nao aparece nela (o `entrar` comum a espera
 * visivel), entao a entrada termina na marca `data-pronto`, presente. */
async function entrarNaTv(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: ESPERA });
  await page.fill('#u', CREDENCIAL.USUARIO);
  await page.fill('#s', CREDENCIAL.SENHA);
  await page.fill('#t', CREDENCIAL.TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { state: 'attached', timeout: 40000 });
}

/* O perfil inteiro como campos do config_gravar -- para devolver o servidor
 * como estava. */
function camposDe(p) {
  const c = {};
  for (const x of p.cores) { c[`aquario.cor_${x.cor}`] = x.escuro; c[`aquario.cor_${x.cor}_claro`] = x.claro; }
  for (const m of p.medidas) c[`aquario.${m.campo}`] = m.valor;
  c['aquario.fonte'] = p.fonte;
  return c;
}

/* A bolha simulada, no retrato real de QUALQUER aba deste contexto. */
async function simular(contexto, url) {
  const rota = url + 'api';
  const tratar = async route => {
    const corpo = route.request().postData() || '';
    if (!/"op":"aquario_retrato"/.test(corpo)) return route.continue();
    const resp = await route.fetch();
    const j = await resp.json();
    if (j.ok && j.resultado) {
      j.resultado.tarefas = (j.resultado.tarefas || []).concat([{ tarefa: ID, origem: 'dados',
        estado: 'executando', op: 'checksum', database: 'aq783', tabela: 'Vendas', faixa: 'outras',
        ms: 45000, servico_ms: 45000, espera_ms: 0, cor: 'vermelho', tamanho: 'grande',
        motivo: 'aquario.motivo.dado_corrompido', cancelavel: false, tem_ponto: false }]);
    }
    return route.fulfill({ response: resp, json: j });
  };
  await contexto.route(rota, tratar);
  return () => contexto.unroute(rota, tratar);
}

/* O traco e o raio da bolha simulada, na aba e no seletor de tanque dados. */
function medir(page, tanque) {
  return page.evaluate(([t, id]) => {
    const g = document.querySelector(`${t} .aq-b[data-id="${id}"]`);
    if (!g) return null;
    const f = g.querySelector('.aq-forma');
    const caixa = f.getBBox();
    return { stroke: (f.getAttribute('stroke') || '').toLowerCase(), r: caixa.width / 2,
      larg: +f.getAttribute('stroke-width') };
  }, [tanque, ID]);
}

/* Espera o desenho chegar ao alvo: o raio corre para o alvo a 2,2/s. */
async function esperarDesenho(page, tanque, cond, oQue) {
  const t0 = Date.now();
  let m = null;
  while (Date.now() - t0 < ESPERA) {
    m = await medir(page, tanque);
    if (m && cond(m)) return m;
    await page.waitForTimeout(250);
  }
  throw new Falha(`${oQue}: ${JSON.stringify(m)}`);
}

async function corpo(ctx, desfazer) {
  const { page } = ctx;
  const tema = ctx.tema === 'claro' ? 'claro' : 'escuro';
  const contexto = page.context();
  await entrar(page, ctx.url);
  const esperar = sel => page.waitForSelector(sel, { timeout: ESPERA });

  passo = 'o perfil de antes, para devolver';
  const r0 = await api(page, 'aquario_retrato', { perfil_versao: '' });
  verdade(r0.perfil && Array.isArray(r0.perfil.medidas), 'o retrato devia trazer o perfil quando a digital difere');
  const original = camposDe(r0.perfil);
  desfazer.push(() => api(page, 'config_gravar', { campos: original }));
  if (r0.ligada === false) {
    await api(page, 'telemetria_ligar');
    desfazer.push(() => api(page, 'telemetria_desligar'));
  }
  // O caso parte do de fabrica: a TV mede o raio de fabrica antes de salvar.
  const fabrica = {};
  for (const x of r0.perfil.cores) { fabrica[`aquario.cor_${x.cor}`] = ''; fabrica[`aquario.cor_${x.cor}_claro`] = ''; }
  for (const m of r0.perfil.medidas) fabrica[`aquario.${m.campo}`] = m.fabrica;
  fabrica['aquario.fonte'] = r0.perfil.fontes[0];
  await api(page, 'config_gravar', { campos: fabrica });
  const desligar = await simular(contexto, ctx.url);
  desfazer.push(desligar);

  // ------------------------------------------------ a TV, aberta antes
  passo = 'a TV de parede, aberta com o perfil de fabrica';
  const tv = await contexto.newPage();
  const errosTv = [];
  tv.on('pageerror', e => errosTv.push(String(e)));
  // A sessao da TV sai do servidor, e nao so a aba: a bateria e um servidor
  // so, a grade de sessoes pagina, e a sessao sobrando empurrou a do caso
  // `botoes-de-sessoes...` do outro tema para fora da primeira pagina.
  desfazer.push(async () => { await tv.evaluate(() => api('sair')).catch(() => {}); await tv.close(); });
  await entrarNaTv(tv, ctx.url + '?tela=aquario&tv=1');
  await tv.waitForSelector('#aqTela.aqt-tv', { timeout: ESPERA });
  const fab = r0.perfil.medidas.find(m => m.campo === 'raio_max').fabrica;
  const corFab = await esperarDesenho(tv, '#aqTela', m => Math.abs(m.r - fab) < 1,
    `a TV devia desenhar a bolha de 45 s no raio de fabrica ${fab}`);
  verdade(corFab.stroke.startsWith('var(--vermelho'), `de fabrica a TV pinta pela variavel do tema: ${corFab.stroke}`);

  // ----------------------------------------------- o editor, na configuracao
  passo = 'o editor do perfil na tela de configuracao';
  await page.evaluate(() => verConfig());
  await esperar('#cfIrGerais');
  await clicarOuExplicar(page, '#cfIrGerais');
  await esperar('#cfAquario .aqe-salvar');
  for (const t of ['escuro', 'claro']) {
    await page.waitForFunction(t => document.querySelectorAll(`#cfAquario .aqe-agua[data-tema="${t}"] .aq-b`).length === 6,
      t, { timeout: ESPERA });
  }
  // o editor nao entra no formulario generico: o «Salvar no config.json»
  // coleta todo `#painel [data-campo]`, e um <div> ali o derrubava
  const intrusos = await page.$$eval('#cfAquario [data-campo]', l => l.length);
  igual(intrusos, 0, 'o editor do aquario poe [data-campo] no formulario generico');
  // o rotulo e a fonte da pre-visualizacao nao gritam: o CSS global nao morde
  const caixa = await page.$eval('#cfAquario .aqe-cores li span', e => getComputedStyle(e).textTransform);
  igual(caixa, 'none', 'o nome da cor no editor nao pode vir em caixa alta');
  const larguraCor = await page.$eval('#cfAquario input[type=color]', e => e.getBoundingClientRect().width);
  verdade(larguraCor < 60, `o seletor de cor virou barra (${larguraCor}px): o input{width:100%} mordeu`);
  // e a lista de letras: o flex em coluna a esticava por cima do width:auto
  const larguraLetra = await page.$eval('#cfAquario .aqe-fonte', e => e.getBoundingClientRect().width);
  verdade(larguraLetra < 400, `a lista de letras esticou na coluna inteira (${larguraLetra}px)`);

  passo = 'escolher o tom e o raio, ver na pre-visualizacao';
  const seletor = `#cfAquario input[type=color][data-cor="vermelho"][data-tema="${tema}"]`;
  await page.fill(seletor, TOM[tema]);
  await page.$eval('#cfAquario input[data-medida="raio_max"]', (e, v) => {
    e.value = String(v); e.dispatchEvent(new Event('input', { bubbles: true }));
  }, RAIO);
  const naPrevia = await page.$eval(`#cfAquario .aqe-agua[data-tema="${tema}"] .aq-b[data-cor="vermelho"] .aq-forma`,
    f => (f.getAttribute('stroke') || '').toLowerCase());
  igual(naPrevia, TOM[tema], 'a pre-visualizacao devia pintar o tom escolhido antes de salvar');
  // o rascunho NAO vaza para a TV: ela continua no de fabrica
  const tvAinda = await medir(tv, '#aqTela');
  verdade(Math.abs(tvAinda.r - fab) < 1 && tvAinda.stroke.startsWith('var('), `o rascunho vazou para a TV: ${JSON.stringify(tvAinda)}`);
  await capturar(ctx, ctx.nomeCaptura('editor'));

  passo = 'salvar';
  await clicarOuExplicar(page, '#cfAquario .aqe-salvar');
  await esperar('#cfAquario .aqe-res[data-estado="ok"]');
  const c1 = await api(page, 'config', {});
  igual(c1.aquario.raio_max, RAIO, 'o servidor devia guardar o raio');
  igual(c1.aquario[tema === 'claro' ? 'cor_vermelho_claro' : 'cor_vermelho'], TOM[tema], 'o servidor devia guardar o tom');

  passo = 'a TV muda sozinha, sem recarregar';
  const naTv = await esperarDesenho(tv, '#aqTela', m => Math.abs(m.r - RAIO) < 1 && m.stroke === TOM[tema],
    'a TV devia desenhar o tom e o raio novos no retrato seguinte');
  ctx.notas.push(`TV: raio ${naTv.r.toFixed(1)}, traco ${naTv.stroke}`);
  await tv.screenshot({ path: ctx.capturas ? `${ctx.capturas}/${ctx.nomeCaptura('tv')}.png` : undefined }).catch(() => {});
  verdade(!errosTv.length, `erro de pagina na TV: ${errosTv.join(' | ')}`);

  // ----------------------------------------------- a recusa do contraste
  passo = 'um tom que some na agua e recusado';
  const versaoAntes = (await api(page, 'aquario_retrato', {})).perfil_versao;
  await page.fill(seletor, RUIM[tema]);
  await clicarOuExplicar(page, '#cfAquario .aqe-salvar');
  await esperar('#cfAquario .aqe-res[data-estado="recusado"]');
  contem(await page.textContent('#cfAquario .aqe-res'), 'contraste', 'a recusa devia dizer o motivo');
  await capturar(ctx, ctx.nomeCaptura('recusa'));
  igual((await api(page, 'aquario_retrato', {})).perfil_versao, versaoAntes, 'a recusa mudou o perfil gravado');

  passo = 'descartar, de fabrica de um tom, tudo de fabrica';
  await clicarOuExplicar(page, '#cfAquario .aqe-descartar');
  igual(await page.$eval(seletor, e => e.value.toLowerCase()), TOM[tema], 'descartar devia voltar ao gravado');
  await clicarOuExplicar(page, `#cfAquario .aqe-zero[data-cor="vermelho"][data-tema="${tema}"]`);
  const fabHex = r0.perfil.cores.find(x => x.cor === 'vermelho')[tema === 'claro' ? 'fabrica_claro' : 'fabrica_escuro'];
  igual(await page.$eval(seletor, e => e.value.toLowerCase()), fabHex, 'o «de fábrica» do tom devia voltar ao do tema');
  await clicarOuExplicar(page, '#cfAquario .aqe-fabrica');
  igual(await page.$eval('#cfAquario input[data-medida="raio_max"]', e => +e.value), fab, 'tudo de fabrica devia voltar o raio');
  await esperar('#cfAquario .aqe-res[data-estado="ok"]');

  // ----------------------------------------------- a tela normal do aquario
  passo = 'o aquario normal le o mesmo perfil';
  await page.evaluate(() => PhxTelas.abrir('aquario'));
  const naTela = await esperarDesenho(page, '#aqTela', m => Math.abs(m.r - RAIO) < 1 && m.stroke === TOM[tema],
    'o aquario normal devia desenhar o perfil gravado');
  verdade(naTela.larg > 0, 'a bolha sem traco');
}

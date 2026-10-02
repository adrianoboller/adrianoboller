const params = new URLSearchParams(location.search);
const forced = params.get('screen');
const splash = document.getElementById('splash');
const app = document.getElementById('app');
const progress = document.getElementById('progressBar');
const pct = document.getElementById('bootPct');
const label = document.getElementById('bootLabel');
const moduleBox = document.getElementById('bootModules');

const nativeBridgeStatus = document.getElementById('nativeBridgeStatus');
const liveEventStatus = document.getElementById('liveEventStatus');
const apiEndpoint = document.getElementById('apiEndpoint');
const hostSession = document.getElementById('hostSession');
const liveReceivers = document.getElementById('liveReceivers');
const evidenceState = document.getElementById('evidenceState');
const hostPolicy = document.getElementById('hostPolicy');
const eventConsole = document.getElementById('eventConsole');

/* ===================== ABERTURA (splash) ===================== */
// A abertura mede o que diz. Antes era um temporizador de 15 etapas (15 x 190 ms + 420 ms)
// que listava «Connectivity • HTTP + PostgreSQL + Ollama» sem medir nada e segurava a primeira
// tela util por 3,4 s na mesa e 7,6 s no celular (qualificacao de 01/10/2026, M4). Agora as
// etapas sao as que a pagina de fato espera, e a tela abre quando a ultima termina.
// Quem chega direto a uma tela (?tela=, #tela, o aplicativo instalado) nao ve abertura.
let avisarHost;
const hostPronto = new Promise(r => { avisarHost = r; });
const ETAPAS = [
  { chave: 'idiomas', rotulo: () => txt('splash.etapa.idiomas', 'Fábrica de idiomas'), pronto: idiomas.pronto },
  { chave: 'fonte', rotulo: () => txt('splash.etapa.fonte', 'Fonte da marca'), pronto: document.fonts?.ready ?? Promise.resolve() },
  { chave: 'host', rotulo: () => txt('splash.etapa.host', 'Host nativo'), pronto: hostPronto },
];
// Teto de espera: uma etapa que nunca responde (fabrica fora do ar) nao prende a tela -- ela
// abre no padrao do fonte, que e o que aconteceria sem abertura nenhuma.
const TETO_ABERTURA_MS = 4000;
const instalado = window.matchMedia?.('(display-mode: standalone)').matches || navigator.standalone === true;
const direto = forced === 'dashboard' || params.has('tela') || !!location.hash || instalado;

function revealApp() {
  splash.classList.add('hidden');
  splash.setAttribute('aria-hidden', 'true');
  splash.removeAttribute('aria-busy');
  app.inert = false;
  app.classList.add('visible');
  app.setAttribute('aria-hidden', 'false');
}

function desenharEtapas() {
  moduleBox.replaceChildren(...ETAPAS.map(e => {
    const s = document.createElement('span');
    s.dataset.etapa = e.chave;
    s.textContent = e.rotulo();
    s.classList.toggle('done', !!e.feita);
    return s;
  }));
  const feitas = ETAPAS.filter(e => e.feita).length;
  const atual = ETAPAS.find(e => !e.feita);
  const valor = Math.round((feitas / ETAPAS.length) * 100);
  progress.style.width = `${valor}%`;
  pct.textContent = `${valor}%`;
  label.textContent = atual ? atual.rotulo() : txt('splash.pronto', 'Pronto');
}

function abrir() {
  // A arte da abertura so baixa quando a abertura aparece (index.html, data-src).
  for (const img of splash.querySelectorAll('img[data-src]')) img.src = img.dataset.src;
  desenharEtapas();
  idiomas.aoTrocar(desenharEtapas);
  const todas = Promise.all(ETAPAS.map(e => Promise.resolve(e.pronto).catch(() => {}).then(() => { e.feita = true; desenharEtapas(); })));
  if (forced === 'splash') return;
  Promise.race([todas, new Promise(r => setTimeout(r, TETO_ABERTURA_MS))]).then(revealApp);
}

if (direto) {
  document.body.classList.add('force-dashboard');
  revealApp();
} else if (forced === 'splash') {
  document.body.classList.add('force-splash');
  abrir();
} else {
  abrir();
}

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;
const liveEvents = [];

// Data e hora pelo idioma da TELA, nao pelo do navegador: em ingles, «01/10/2026» se le
// 10 de janeiro (qualificacao de 01/10/2026, M12). PT fica dd/mm/aaaa; os outros, ISO
// (aaaa-mm-dd), que nao se le errado em lingua nenhuma. Hora em 24 h nos dois.
const doisDigitos = n => String(n).padStart(2, '0');
function hora(v) {
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? String(v ?? '') : `${doisDigitos(d.getHours())}:${doisDigitos(d.getMinutes())}:${doisDigitos(d.getSeconds())}`;
}
function dataHora(v) {
  const d = new Date(v);
  if (Number.isNaN(d.getTime())) return String(v ?? '');
  const [dd, mm, aa] = [doisDigitos(d.getDate()), doisDigitos(d.getMonth() + 1), d.getFullYear()];
  return `${idiomas.atual === 'pt' ? `${dd}/${mm}/${aa}` : `${aa}-${mm}-${dd}`} ${hora(d)}`;
}

function compactUuid(value) {
  if (!value) return '—';
  return value.length > 18 ? `${value.slice(0, 8)}…${value.slice(-6)}` : value;
}

function renderEvent(event) {
  liveEvents.unshift(event);
  liveEvents.splice(12);
  if (!eventConsole) return;
  eventConsole.innerHTML = '';
  for (const item of liveEvents) {
    const row = document.createElement('div');
    row.className = 'event-row';
    const topic = document.createElement('b');
    topic.textContent = `${item.topic} / ${item.event_type}`;
    const meta = document.createElement('small');
    meta.textContent = `${compactUuid(item.uuid)} • ${hora(item.occurred_at)}`;
    row.append(topic, meta);
    eventConsole.appendChild(row);
  }
}

// O painel do host guarda o que o host DISSE e desenha o rotulo na hora: assim a troca de
// idioma refaz o rotulo sem apagar o dado (endereco, contagem, politica) que veio dele.
// Um data-txt aqui seria pior: o aplicar() da fabrica trocaria o dado pelo rotulo padrao.
const hostVisto = { api: 'navegador', evidencia: 'espera', politica: '' };
function desenharHost() {
  const a = hostVisto.api;
  apiEndpoint.textContent = a === 'navegador' ? txt('geral.host_modo_navegador', 'MODO NAVEGADOR')
    : a === 'fora' ? txt('geral.host_api_fora', 'API FORA DO AR') : txt('geral.host_api', 'API {addr}', { addr: a.addr });
  const e = hostVisto.evidencia;
  evidenceState.textContent = e === 'espera' ? txt('geral.host_espera', 'AGUARDANDO')
    : e === 'invalida' ? txt('geral.host_invalida', 'INVÁLIDA') : txt('geral.host_valida', 'VÁLIDA • {registros}', { registros: e.registros });
  hostPolicy.textContent = hostVisto.politica || txt('geral.host_negado', 'NEGADO');
}
desenharHost();
idiomas.aoTrocar(desenharHost);

// RODAPE (SP000036 L1): versao, pasta ativa e idioma sao DADO lido -- o host (host_status),
// o /v1/config (chave agente.pasta) e a fabrica (idiomas.atual). Sem fonte, «NAO MEDIDO»
// da fabrica; nunca um valor digitado. Mesmo desenho de sempre: estado guardado, rotulo
// refeito na troca de idioma (um data-txt no <b> trocaria o dado pelo rotulo).
const rodapeVisto = { versao: null, pasta: null };
// Bandeira por idioma, desenhada em SVG (emoji de bandeira nao desenha no Windows; o mesmo
// desenho do console do PhxSql). Idioma sem bandeira mostra so o codigo.
const BANDEIRAS = {
  pt: '<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="16" fill="#009b3a"/><path d="M12 1.7 22.3 8 12 14.3 1.7 8Z" fill="#fedf00"/><circle cx="12" cy="8" r="3.9" fill="#002776"/></svg>',
  en: '<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="16" fill="#012169"/><path d="M0 0 24 16M24 0 0 16" stroke="#fff" stroke-width="3.2"/><path d="M0 0 24 16M24 0 0 16" stroke="#c8102e" stroke-width="1.7"/><path d="M12 0V16M0 8H24" stroke="#fff" stroke-width="5.4"/><path d="M12 0V16M0 8H24" stroke="#c8102e" stroke-width="3.2"/></svg>',
};
function desenharRodape() {
  const nao = txt('casca.nao_medido', 'NÃO MEDIDO');
  document.getElementById('rodapeVersao').textContent = rodapeVisto.versao ?? nao;
  document.getElementById('rodapePasta').textContent = rodapeVisto.pasta ?? nao;
  document.getElementById('rodapeIdioma').textContent = idiomas.atual.toUpperCase();
  document.getElementById('rodapeBandeira').innerHTML = BANDEIRAS[idiomas.atual] || '';
}
desenharRodape();
idiomas.aoTrocar(desenharRodape);
// A pasta: GET /v1/config (o mesmo que a tela Configuracao le), so a chave agente.pasta.
// Sem API, sem token ou sem valor fica o «NAO MEDIDO»; leitura que falhou nao e erro de tela.
(async () => {
  try {
    let token = '';
    try { token = localStorage.getItem('phxclaw.token') || ''; } catch { /* sem armazenamento */ }
    const r = await fetch('./v1/config', { headers: { Authorization: `Bearer ${token}` }, cache: 'no-store' });
    if (!r.ok) return;
    const vista = await r.json();
    const chave = (vista.chaves || []).find(c => c.chave === 'agente.pasta');
    const valor = chave?.valor ?? chave?.padrao ?? null;
    if (typeof valor === 'string' && valor) { rodapeVisto.pasta = valor; desenharRodape(); }
  } catch { /* sem rede ou sem API: fica NAO MEDIDO */ }
})();

// ASSISTENTE (SP000036 L1): so abrir/recolher, escolha guardada no aparelho. Vazio por ora.
(() => {
  const CHAVE = 'phxclaw.assistente';
  const botao = document.getElementById('assistenteAlternar');
  let aberto = false;
  try { aberto = localStorage.getItem(CHAVE) === 'aberto'; } catch { /* sem armazenamento */ }
  const aplicar = () => {
    if (aberto) app.dataset.assistente = 'aberto'; else delete app.dataset.assistente;
    document.getElementById('assistente').dataset.recolhido = aberto ? '0' : '1';
    botao.setAttribute('aria-expanded', String(aberto));
  };
  botao.addEventListener('click', () => {
    aberto = !aberto;
    try { localStorage.setItem(CHAVE, aberto ? 'aberto' : 'recolhido'); } catch { /* idem */ }
    aplicar();
  });
  aplicar();
})();

// O topo diz o estado do NUCLEO pelo que o host respondeu, nunca por texto fixo: sem host
// nativo nao ha nucleo a vista e a tela diz isso (o ponto nao pulsa). Estado guardado e
// rotulo desenhado na hora, como o painel do host, para a troca de idioma refazer o rotulo.
const coreDot = document.getElementById('coreDot');
const coreStatus = document.getElementById('coreStatus');
const topoVisto = { nucleo: invokeDoHost() ? 'espera' : 'sem_host', ponte: invokeDoHost() ? 'tauri' : 'navegador', eventos: invokeDoHost() ? 'espera' : 'sem_host' };
function invokeDoHost() { return !!(window.__TAURI__?.core?.invoke && window.__TAURI__?.event?.listen); }
function desenharTopo() {
  const NUCLEO = {
    espera: () => txt('topo.nucleo_espera', 'NÚCLEO: AGUARDANDO O HOST'),
    ativo: () => txt('topo.nucleo_ativo', 'NÚCLEO ATIVO'),
    erro: () => txt('topo.nucleo_erro', 'NÚCLEO SEM RESPOSTA'),
    sem_host: () => txt('topo.nucleo_sem_host', 'SEM HOST NATIVO'),
  };
  const PONTE = {
    navegador: () => txt('topo.previa_web', 'PRÉVIA WEB'),
    tauri: () => txt('topo.tauri_ok', 'HOST TAURI CONECTADO'),
    erro: () => txt('topo.tauri_erro', 'HOST TAURI COM ERRO'),
  };
  const EVENTOS = {
    espera: () => txt('topo.eventos_espera', 'EVENTOS: AGUARDANDO'),
    vivo: () => txt('topo.eventos_vivo', 'EVENTOS AO VIVO'),
    sem_host: () => txt('topo.eventos_sem_host', 'SEM FLUXO DE EVENTOS'),
  };
  coreDot.dataset.estado = topoVisto.nucleo;
  coreStatus.textContent = NUCLEO[topoVisto.nucleo]();
  nativeBridgeStatus.textContent = PONTE[topoVisto.ponte]();
  nativeBridgeStatus.classList.toggle('native-ok', topoVisto.ponte === 'tauri');
  liveEventStatus.textContent = EVENTOS[topoVisto.eventos]();
}
desenharTopo();
idiomas.aoTrocar(desenharTopo);

async function connectNativeBridge() {
  if (!invoke || !listen) {
    avisarHost();
    return;
  }

  try {
    const status = await invoke('host_status');
    topoVisto.nucleo = 'ativo';
    desenharTopo();
    // A versao vem do host (CARGO_PKG_VERSION); cravada no HTML, ficou em v0.11.0 ate a 0.70.
    for (const id of ['brandVersion', 'kernelVersion']) {
      const el = document.getElementById(id);
      if (el) el.textContent = `v${status.version}`;
    }
    rodapeVisto.versao = `v${status.version}`;
    desenharRodape();
    hostSession.textContent = compactUuid(status.session_uuid);
    liveReceivers.textContent = String(status.live_bus?.receiver_count ?? 0);
    hostVisto.politica = [
      status.policy?.command_execution ? 'CMD' : null,
      status.policy?.desktop_input ? 'INPUT' : null,
      status.policy?.screen_capture ? 'SCREEN' : null,
      status.policy?.webview_control ? 'DOM' : null,
    ].filter(Boolean).join(' + ');
    hostVisto.api = status.api?.addr ? { addr: status.api.addr } : 'fora';
    desenharHost();

    const verify = await invoke('verify_evidence');
    hostVisto.evidencia = verify.valid ? { registros: verify.records } : 'invalida';
    desenharHost();

    const snapshot = await invoke('events_snapshot', { limit: 20 });
    snapshot.forEach(renderEvent);
  } catch (error) {
    topoVisto.nucleo = 'erro';
    topoVisto.ponte = 'erro';
    desenharTopo();
    console.error('PhxClaw host status failed', error);
  }
  avisarHost();

  await listen('phoenix:event', ({ payload }) => {
    if (topoVisto.eventos !== 'vivo') { topoVisto.eventos = 'vivo'; desenharTopo(); }
    renderEvent(payload);
  });

  await listen('phoenix:webview-request', async ({ payload }) => {
    const request = payload;
    const command = request?.action?.command;
    if (!command) return;
    try {
      const value = await executeDomCommand(command);
      await invoke('complete_webview_action', {
        completion: { request_uuid: request.uuid, ok: true, value, error: null },
      });
    } catch (error) {
      await invoke('complete_webview_action', {
        completion: {
          request_uuid: request.uuid,
          ok: false,
          value: null,
          error: String(error?.message ?? error),
        },
      });
    }
  });
}

function one(selector) {
  const element = document.querySelector(selector);
  if (!element) throw new Error(`selector not found: ${selector}`);
  return element;
}

function nodeSnapshot(element) {
  return {
    tag: element.tagName?.toLowerCase() ?? null,
    id: element.id || null,
    classes: [...(element.classList ?? [])],
    text: (element.textContent ?? '').slice(0, 4096),
    html: (element.outerHTML ?? '').slice(0, 16384),
  };
}

function domToSvg(element) {
  const rect = element.getBoundingClientRect();
  const width = Math.max(1, Math.ceil(rect.width));
  const height = Math.max(1, Math.ceil(rect.height));
  const clone = element.cloneNode(true);
  const serialized = new XMLSerializer().serializeToString(clone);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><foreignObject width="100%" height="100%"><div xmlns="http://www.w3.org/1999/xhtml">${serialized}</div></foreignObject></svg>`;
}

async function executeDomCommand(command) {
  switch (command.command) {
    case 'navigate': {
      const target = new URL(command.url, location.href);
      if (target.origin !== location.origin) throw new Error('typed DOM bridge only permits same-origin navigation');
      location.assign(target.href);
      return { navigated: target.href };
    }
    case 'load_html': {
      document.documentElement.innerHTML = command.html;
      return { loaded: true, base_url: command.base_url ?? null };
    }
    case 'evaluate_javascript':
      throw new Error('evaluate_javascript is executed through native WebView.eval, not the DOM bridge');
    case 'inject_css': {
      const style = document.createElement('style');
      style.dataset.phxclaw = 'injected';
      style.textContent = command.css;
      document.head.appendChild(style);
      return { injected: true, bytes: command.css.length };
    }
    case 'query_selector':
      return nodeSnapshot(one(command.selector));
    case 'query_selector_all':
      return [...document.querySelectorAll(command.selector)].slice(0, 100).map(nodeSnapshot);
    case 'get_outer_html':
      return command.selector ? one(command.selector).outerHTML : document.documentElement.outerHTML;
    case 'set_inner_html':
      one(command.selector).innerHTML = command.html;
      return { updated: true };
    case 'set_attribute':
      one(command.selector).setAttribute(command.name, command.value);
      return { updated: true };
    case 'remove_attribute':
      one(command.selector).removeAttribute(command.name);
      return { updated: true };
    case 'click':
      one(command.selector).click();
      return { clicked: true };
    case 'focus':
      one(command.selector).focus();
      return { focused: true };
    case 'type_text': {
      const element = one(command.selector);
      if ('value' in element) element.value = command.text;
      else element.textContent = command.text;
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
      return { typed: command.text.length };
    }
    case 'dispatch_event':
      one(command.selector).dispatchEvent(new CustomEvent(command.event_type, { bubbles: true, detail: command.detail }));
      return { dispatched: command.event_type };
    case 'scroll_into_view':
      one(command.selector).scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      return { scrolled: true };
    case 'get_computed_style': {
      const style = getComputedStyle(one(command.selector));
      const result = {};
      for (const name of [...style]) result[name] = style.getPropertyValue(name);
      return result;
    }
    case 'dom_to_svg':
      return domToSvg(command.selector ? one(command.selector) : document.documentElement);
    default:
      throw new Error(`unsupported WebView command: ${command.command}`);
  }
}

connectNativeBridge();

/* ===================== NAVEGACAO ===================== */
// Cada botao abre a tela do mesmo nome (data-tela). A tela vai no #hash para a captura e o
// recarregar voltarem ao mesmo lugar; `?tela=` vale igual, para o roteiro de prova.
const telas = [...document.querySelectorAll('section.tela[data-tela]')];
const botoesNav = [...document.querySelectorAll('.nav[data-tela]')];
const carregadores = {};

function mostrarTela(nome) {
  if (!telas.some(t => t.dataset.tela === nome)) nome = 'geral';
  for (const t of telas) t.hidden = t.dataset.tela !== nome;
  for (const b of botoesNav) {
    const ativo = b.dataset.tela === nome;
    b.classList.toggle('active', ativo);
    if (ativo) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  if (location.hash !== `#${nome}`) history.replaceState(null, '', `#${nome}`);
  document.body.dataset.tela = nome;
  carregadores[nome]?.();
}

botoesNav.forEach(b => b.addEventListener('click', () => mostrarTela(b.dataset.tela)));
window.addEventListener('hashchange', () => mostrarTela(location.hash.slice(1)));

/* ===================== DADOS GERADOS ===================== */
// Nenhum numero destas telas e digitado: cada um sai de um JSON gerado do codigo ou dos
// documentos (tools/gerar_assets_ui.sh, e o exemplo equipe_json para a equipe). Arquivo
// ausente vira aviso na tela, nunca numero inventado.
//
// 404 e rede caida sao coisas diferentes e a tela diz qual: arquivo ausente manda gerar;
// falha de rede (fetch rejeitado) diz «sem conexao» e oferece TENTAR DE NOVO -- mandar rodar
// cargo porque o wi-fi caiu era mentir sobre o arquivo (qualificacao de 01/10/2026, G7).
// Leitura que falhou nao fica guardada: tentar de novo pede outra vez.
const cacheJson = new Map();
function lerJson(nome) {
  if (!cacheJson.has(nome)) {
    const p = fetch(`./assets/${nome}`, { cache: 'no-store' })
      .catch(() => { throw Object.assign(new Error(`${nome}: sem rede`), { rede: true, arquivo: nome }); })
      .then(r => {
        if (!r.ok) throw Object.assign(new Error(`${nome}: HTTP ${r.status}`), { status: r.status });
        return r.json();
      });
    p.catch(() => cacheJson.delete(nome));
    cacheJson.set(nome, p);
  }
  return cacheJson.get(nome);
}

// TENTAR DE NOVO: um botao [data-tentar] por tela, escondido ate um erro de rede ou do
// agente. O clique refaz a carga da tela (ou o que a tela registrou em `tentar`).
const tentar = {};
function mostrarTentar(tela, sim) {
  for (const b of document.querySelectorAll(`section.tela[data-tela="${tela}"] [data-tentar]`)) b.hidden = !sim;
}
document.addEventListener('click', e => {
  const b = e.target.closest?.('[data-tentar]');
  if (!b) return;
  const tela = b.closest('section.tela')?.dataset.tela;
  b.hidden = true;
  (tentar[tela] || carregadores[tela])?.();
});

function el(tag, classe, texto) {
  const e = document.createElement(tag);
  if (classe) e.className = classe;
  if (texto !== undefined && texto !== null) e.textContent = String(texto);
  return e;
}

function avisoAusente(resumo, conteudo, arquivo, comando, erro) {
  // O erro diz qual arquivo faltou quando nao e o JSON da tela (o phx-grid, por exemplo).
  arquivo = erro?.arquivo || arquivo;
  if (erro?.rede) {
    resumo.textContent = txt('aviso.sem_rede_arquivo', 'Sem conexão: assets/{arquivo} não chegou. Confira a rede e toque em TENTAR DE NOVO.', { arquivo });
    mostrarTentar(resumo.closest('section.tela')?.dataset.tela, true);
  } else {
    resumo.textContent = txt('aviso.sem_arquivo', 'assets/{arquivo} não existe — a tela não mostra número sem fonte. Gere com: {comando}', { arquivo, comando });
  }
  resumo.classList.add('aviso');
  conteudo.replaceChildren();
}

const seloAgentes = document.getElementById('seloAgentes');
const seloFerramentas = document.getElementById('seloFerramentas');
const seloIde = document.getElementById('seloIde');
lerJson('equipe.json').then(d => { seloAgentes.textContent = String(d.total); }).catch(() => {});
lerJson('ferramentas.json').then(d => { seloFerramentas.textContent = String(d.total); }).catch(() => {});

// As listagens sao grades (grades.js, phx-grid): criadas UMA vez por tela. Reabrir a tela
// nao refaz a grade, que perderia o agrupamento, o filtro e a ordem de quem estava olhando;
// a troca de idioma a grade refaz sozinha, guardando o layout.
const gradesDaTela = {};
document.querySelectorAll('[data-grade-exporta]').forEach(b => b.addEventListener('click', () => {
  const [tela, formato] = b.dataset.gradeExporta.split(':');
  const gr = gradesDaTela[tela];
  if (gr) grades.exportar(gr.g, `phxclaw-${tela}`, formato);
}));
function buscaDaGrade(campo, tela) {
  let espera = null;
  campo.oninput = () => {
    clearTimeout(espera);
    espera = setTimeout(() => gradesDaTela[tela]?.g.buscar(campo.value.trim()), 120);
  };
}

carregadores.agentes = async () => {
  const resumo = document.getElementById('agentesResumo');
  const conteudo = document.getElementById('agentesConteudo');
  let d;
  try { d = await lerJson('equipe.json'); await grades.carregar(); } catch (e) {
    avisoAusente(resumo, conteudo, 'equipe.json', 'cargo run -p phxclaw-agent --example equipe_json', e);
    return;
  }
  resumo.classList.remove('aviso');
  resumo.textContent = txt('agentes.resumo', '{total} papéis em {macroareas} macroáreas • fonte: {fonte}',
    { total: d.total, macroareas: d.macroareas.length, fonte: d.fonte });
  if (gradesDaTela.agentes) return;
  // A macroarea da linha e a do grupo do JSON (a que da o total de cada uma); os modelos,
  // uma lista, viram um texto so para a celula, o filtro e a exportacao.
  const linhas = d.macroareas.flatMap(m => m.papeis.map(p => ({ ...p, macroarea: m.nome, modelos: (p.modelos || []).join(' • ') })));
  gradesDaTela.agentes = grades.criar(conteudo, {
    nome: 'agentes',
    // No celular (cartao): o papel e a capacidade a vista, a missao em duas linhas;
    // o resto abre no toque.
    cartao: { titulo: 'nome', campos: ['capability_principal'], curtos: ['missao'] },
    cfg: () => ({
      chave: 'id', dados: linhas, filterRow: true, pagina: { tamanho: 1000, opcoes: [1000] },
      colunas: [
        { campo: 'nome', titulo: txt('agentes.col.nome', 'Papel'), tag: 'nome' },
        { campo: 'macroarea', titulo: txt('agentes.col.macroarea', 'Macroárea'), tag: 'macroarea' },
        { campo: 'nucleo', titulo: txt('agentes.col.nucleo', 'Núcleo') },
        { campo: 'humano', titulo: txt('agentes.col.tipo', 'Tipo'), tag: 'tipo',
          valores: { true: txt('agentes.humano', 'HUMANO'), false: txt('agentes.agente', 'AGENTE') } },
        { campo: 'capability_principal', titulo: txt('agentes.col.capacidade', 'Capacidade principal'), tag: 'cap' },
        { campo: 'criticidade', titulo: txt('agentes.col.criticidade', 'Criticidade') },
        { campo: 'execucao', titulo: txt('agentes.col.execucao', 'Execução') },
        { campo: 'missao', titulo: txt('agentes.col.missao', 'Missão'), quebraLinha: true, tag: 'missao' },
        { campo: 'modelos', titulo: txt('agentes.col.modelos', 'Modelos') },
        { campo: 'quando_acionar', titulo: txt('agentes.col.quando', 'Quando acionar'), quebraLinha: true },
      ],
    }),
    inicial: g => { g.agrupar(['macroarea']); g.mostrarColuna('quando_acionar', false); },
  });
  buscaDaGrade(document.getElementById('agentesFiltro'), 'agentes');
};


// Uma ferramenta exige UMA capability (`capacidade`, ex. fs.read); a familia (`grupo`) e o
// prefixo dela. A tela agrupa pela capability exata -- e o que o portao confere --, na ordem
// da familia, para fs.read e fs.write ficarem vizinhas.
function porCapacidade(ferramentas) {
  const m = new Map();
  for (const f of ferramentas) {
    if (!m.has(f.capacidade)) m.set(f.capacidade, { capacidade: f.capacidade, grupo: f.grupo, lista: [] });
    m.get(f.capacidade).lista.push(f);
  }
  const familia = new Map();
  for (const c of m.values()) familia.set(c.grupo, (familia.get(c.grupo) || 0) + c.lista.length);
  return [...m.values()].sort((a, b) => familia.get(b.grupo) - familia.get(a.grupo)
    || a.grupo.localeCompare(b.grupo) || b.lista.length - a.lista.length || a.capacidade.localeCompare(b.capacidade));
}

carregadores.ferramentas = async () => {
  const resumo = document.getElementById('ferramentasResumo');
  const conteudo = document.getElementById('ferramentasConteudo');
  let d;
  try { d = await lerJson('ferramentas.json'); await grades.carregar(); } catch (e) {
    avisoAusente(resumo, conteudo, 'ferramentas.json', 'tools/gerar_assets_ui.sh', e);
    return;
  }
  const concedidas = d.ferramentas.filter(f => f.concedida).length;
  const caps = porCapacidade(d.ferramentas);
  resumo.classList.remove('aviso');
  resumo.textContent = txt('ferramentas.resumo', '{total} ferramentas em {capabilities} capacidades, montadas por `{gerado_por}` v{versao} • {concedidas} concedidas por padrão • {observacao}',
    { total: d.total, capabilities: caps.length, gerado_por: d.gerado_por, versao: d.versao, concedidas, observacao: d.observacao });
  if (gradesDaTela.ferramentas) return;
  // Agrupada pela capability exata -- a que o portao confere --, e a coluna «exige» repete
  // a de cada ferramenta: nada de linha no grupo errado por um find() que parou na primeira.
  gradesDaTela.ferramentas = grades.criar(conteudo, {
    nome: 'ferramentas',
    cartao: { titulo: 'nome', campos: ['exige', 'concedida'], curtos: ['descricao'] },
    cfg: () => ({
      // `exige` repete a capacidade numa coluna propria: o phx-grid esconde a coluna pela qual
      // agrupa (como o Janus), e a exigencia de cada ferramenta tem de continuar a vista.
      chave: 'nome', dados: d.ferramentas.map(f => ({ ...f, exige: f.capacidade })), pagina: { tamanho: 1000, opcoes: [1000] },
      colunas: [
        { campo: 'nome', titulo: txt('ferramentas.col.nome', 'Ferramenta'), tag: 'nome' },
        { campo: 'capacidade', titulo: txt('ferramentas.col.capacidade', 'Capacidade'), tag: 'capacidade' },
        { campo: 'exige', titulo: txt('ferramentas.exige', 'exige'), tag: 'exige' },
        { campo: 'grupo', titulo: txt('ferramentas.col.familia', 'Família'), tag: 'familia' },
        { campo: 'concedida', titulo: txt('ferramentas.col.concessao', 'Concessão'), tag: 'concessao',
          valores: { true: txt('ferramentas.concedida', 'CONCEDIDA POR PADRÃO'), false: txt('ferramentas.exige_concessao', 'EXIGE CONCESSÃO') } },
        { campo: 'descricao', titulo: txt('ferramentas.col.descricao', 'Descrição'), quebraLinha: true, tag: 'descricao' },
      ],
    }),
    inicial: g => { g.agrupar(['capacidade']); g.mostrarColuna('capacidade', false); },
  });
  buscaDaGrade(document.getElementById('ferramentasFiltro'), 'ferramentas');
};

const NOMES_PRODUTO = { openclaw: 'OpenClaw', hermes: 'Hermes', claude_code: 'Claude Code', codex: 'Codex', openjarvis: 'OpenJarvis', vscode: 'VS Code', n8n: 'n8n' };

// Os tres estados se distinguem pela FORMA, nao so pela cor: no agente = cheio, pela metade =
// hachurado, nao = so contorno tracejado. Em escala de cinza (ou para quem nao ve a cor) a
// barra continua legivel; a legenda repete a mesma forma em miniatura.
// O rotulo e funcao, nao string: le a fabrica na hora de desenhar, para seguir a troca de idioma.
const ESTADOS = [
  ['no_agente', 'agente', () => txt('absorcao.no_agente', 'no agente')],
  ['parcial', 'parcial', () => txt('absorcao.parcial', 'pela metade')],
  ['nao', 'nao', () => txt('absorcao.nao', 'não')],
];

function barraTresEstados(p) {
  const b = el('div', 'tri');
  b.setAttribute('role', 'img');
  b.setAttribute('aria-label', txt('absorcao.barra', '{partes} — total {total}', { partes: ESTADOS.map(([k, , r]) => `${p[k]} ${r()}`).join(', '), total: p.total }));
  for (const [k, classe, rotulo] of ESTADOS) {
    const n = Number(p[k]) || 0;
    if (!n) continue;
    const s = el('i', `seg ${classe}`);
    s.dataset.estado = classe;
    s.dataset.n = String(n);
    // Base zero: com a base automatica o rotulo somava largura e o segmento pequeno
    // saia maior que a parte dele (3 em 41 aparecia como ~9%).
    s.style.flex = `${n} 1 0`;
    s.title = `${n} ${rotulo()}`;
    s.append(el('em', null, n));
    b.append(s);
  }
  return b;
}

function legendaTresEstados(p) {
  const l = el('div', 'tri-legenda');
  for (const [k, classe, rotulo] of ESTADOS) {
    const item = el('span');
    item.append(el('i', `amostra ${classe}`), el('span', null, p ? `${p[k]} ${rotulo()}` : rotulo()));
    l.append(item);
  }
  return l;
}

// Uma linha por capacidade de cada produto (absorcao.json, campo `capacidades`). As tres
// colunas 1/vazio existem para o rodape de grupo SOMAR: agrupado por produto, o rodape repete as
// contagens do painel; agrupado por estado, diz quantas de cada produto.
const ESTADO_ABS = {
  agente: () => txt('absorcao.no_agente', 'no agente'),
  parcial: () => txt('absorcao.parcial', 'pela metade'),
  nao: () => txt('absorcao.nao', 'não'),
};
const rotulosEstado = () => Object.fromEntries(Object.entries(ESTADO_ABS).map(([k, f]) => [k, f()]));

carregadores.absorcao = async () => {
  const resumo = document.getElementById('absorcaoResumo');
  const conteudo = document.getElementById('absorcaoConteudo');
  const caixaCubo = document.getElementById('absorcaoCubo');
  let d;
  try { d = await lerJson('absorcao.json'); await grades.carregar(); } catch (e) {
    avisoAusente(resumo, conteudo, 'absorcao.json', 'tools/gerar_assets_ui.sh', e);
    return;
  }
  const chaves = Object.keys(d);
  resumo.classList.remove('aviso');
  resumo.textContent = txt('absorcao.resumo', '{n} produtos medidos capacidade por capacidade ({gerador}).', { n: chaves.length, gerador: 'docs/absorcao/gerar_absorcao.py' });
  document.getElementById('absorcaoLegenda').replaceChildren(legendaTresEstados(null));
  if (gradesDaTela.absorcao) return;
  const linhas = chaves.flatMap(k => (d[k].capacidades || []).map(c => ({
    linha: `${k}/${c.id}`, produto: k, capacidade: c.id, estado: c.estado,
    // 1 onde a capacidade esta naquele estado e vazio (null) onde nao: a celula fica limpa
    // e a soma do grupo conta so os 1.
    agente: c.estado === 'agente' ? 1 : null, parcial: c.estado === 'parcial' ? 1 : null, nao: c.estado === 'nao' ? 1 : null,
  })));
  const contagem = (campo, titulo) => ({ campo, titulo, tag: campo, tipo: 'numero', agregador: 'sum', filtravel: false });
  gradesDaTela.absorcao = grades.criar(conteudo, {
    nome: 'absorcao',
    cfg: () => ({
      chave: 'linha', dados: linhas, rodapeGrupo: true, pagina: { tamanho: 1000, opcoes: [1000] },
      condicoes: Object.keys(ESTADO_ABS).map(e => ({ campo: 'estado', op: '=', valor: e, estilo: `abs-${e}` })),
      colunas: [
        { campo: 'produto', titulo: txt('absorcao.col.produto', 'Produto'), tag: 'produto', valores: NOMES_PRODUTO, formatoGrupo: grades.valorDeGrupo(NOMES_PRODUTO) },
        { campo: 'capacidade', titulo: txt('absorcao.col.capacidade', 'Capacidade'), tag: 'capacidade' },
        { campo: 'estado', titulo: txt('absorcao.col.estado', 'Estado'), tag: 'estado', valores: rotulosEstado(), formatoGrupo: grades.valorDeGrupo(rotulosEstado()) },
        contagem('agente', txt('absorcao.no_agente', 'no agente')),
        contagem('parcial', txt('absorcao.parcial', 'pela metade')),
        contagem('nao', txt('absorcao.nao', 'não')),
      ],
    }),
    inicial: g => g.agrupar(['produto']),
  });
  buscaDaGrade(document.getElementById('absorcaoFiltro'), 'absorcao');
  document.getElementById('absorcaoPorProduto').onclick = () => gradesDaTela.absorcao.g.agrupar(['produto']);
  document.getElementById('absorcaoPorEstado').onclick = () => gradesDaTela.absorcao.g.agrupar(['estado']);
  // Visao cubo: produto x estado, contagem de capacidades, com os totais do proprio cubo.
  let cubo = null;
  const botaoCubo = document.getElementById('absorcaoVerCubo');
  const botaoTranspor = document.getElementById('absorcaoTranspor');
  botaoCubo.onclick = () => {
    const ver = caixaCubo.hidden;
    caixaCubo.hidden = !ver;
    conteudo.hidden = ver;
    botaoTranspor.hidden = !ver;
    botaoCubo.setAttribute('aria-pressed', String(ver));
    if (ver && !cubo) {
      cubo = grades.cubo(caixaCubo, {
        cfg: () => ({
          dados: linhas,
          campos: [
            { campo: 'produto', titulo: txt('absorcao.col.produto', 'Produto'), valores: NOMES_PRODUTO },
            { campo: 'estado', titulo: txt('absorcao.col.estado', 'Estado'), valores: rotulosEstado() },
            { campo: 'capacidade', titulo: txt('absorcao.col.capacidade', 'Capacidade'), dimensao: false },
          ],
          linhas: ['produto'], colunas: ['estado'], valores: [{ campo: 'capacidade', agregador: 'count' }],
        }),
      });
      gradesDaTela.absorcaoCubo = cubo;
    }
  };
  botaoTranspor.onclick = () => cubo?.c.transpor();
};

/* ===================== VISAO GERAL ===================== */
// So o que tem fonte: os tres JSON gerados e o host. Cada painel que perde a fonte diz qual
// arquivo falta, em vez de manter um numero de enfeite.
function semFonte(alvo, arquivo, erro) {
  alvo.replaceChildren(el('p', 'vazio aviso', erro?.rede
    ? txt('aviso.sem_rede_arquivo', 'Sem conexão: assets/{arquivo} não chegou. Confira a rede e toque em TENTAR DE NOVO.', { arquivo })
    : txt('geral.sem_fonte', 'assets/{arquivo} não existe — sem fonte, sem número.', { arquivo })));
  if (erro?.rede) mostrarTentar('geral', true);
}

document.querySelectorAll('[data-ir]').forEach(b => b.addEventListener('click', () => mostrarTela(b.dataset.ir)));

carregadores.geral = async () => {
  const [eq, fe, ab] = await Promise.allSettled(['equipe.json', 'ferramentas.json', 'absorcao.json'].map(lerJson));
  const $ = id => document.getElementById(id);

  if (eq.status === 'fulfilled') {
    const d = eq.value;
    const humanos = d.macroareas.reduce((n, m) => n + m.papeis.filter(p => p.humano).length, 0);
    $('geralAgentes').textContent = String(d.total);
    $('geralAgentesNota').textContent = txt('geral.agentes_nota', '{macroareas} macroáreas • humanos: {humanos}', { macroareas: d.macroareas.length, humanos });
    const maior = Math.max(...d.macroareas.map(m => m.total));
    $('geralMacro').replaceChildren(...d.macroareas.map(m => {
      const r = el('div', 'macro');
      const b = el('div', 'macro-barra');
      const i = el('i');
      i.style.width = `${(m.total / maior) * 100}%`;
      b.append(i);
      r.append(el('span', null, m.nome), b, el('b', null, m.total));
      return r;
    }));
  } else {
    $('geralAgentesNota').textContent = txt('geral.ausente', '{arquivo} ausente', { arquivo: 'equipe.json' });
    semFonte($('geralMacro'), 'equipe.json', eq.reason);
  }

  if (fe.status === 'fulfilled') {
    const d = fe.value;
    const conc = d.ferramentas.filter(f => f.concedida).length;
    const caps = porCapacidade(d.ferramentas);
    $('geralFerramentas').textContent = String(d.total);
    $('geralFerramentasNota').textContent = txt('geral.ferramentas_nota', '{concedidas} concedidas por padrão • {capabilities} capacidades', { concedidas: conc, capabilities: caps.length });
    const barra = $('geralFerramentasBarra');
    barra.hidden = false;
    barra.firstElementChild.style.width = `${(conc / d.total) * 100}%`;
    barra.title = txt('geral.barra_concedidas', '{concedidas} de {total} concedidas por padrão', { concedidas: conc, total: d.total });
    const familias = new Map();
    for (const c of caps) {
      if (!familias.has(c.grupo)) familias.set(c.grupo, { n: 0, conc: 0, caps: [] });
      const f = familias.get(c.grupo);
      f.n += c.lista.length;
      f.conc += c.lista.filter(x => x.concedida).length;
      f.caps.push(c.capacidade);
    }
    $('geralCapacidades').replaceChildren(...[...familias.entries()].map(([g, f]) => {
      const r = el('div');
      r.title = f.caps.join(', ');
      r.append(el('code', null, g), el('span', null, f.caps.map(c => c.slice(g.length + 1) || c).join(' • ')),
        el('em', f.conc === f.n ? 'sim' : f.conc ? 'meio' : 'nao', `${f.conc}/${f.n}`));
      return r;
    }));
  } else {
    $('geralFerramentasNota').textContent = txt('geral.ausente', '{arquivo} ausente', { arquivo: 'ferramentas.json' });
    semFonte($('geralCapacidades'), 'ferramentas.json', fe.reason);
  }

  if (ab.status === 'fulfilled') {
    const d = ab.value;
    const pcts = Object.values(d).map(p => Number(p.pct_agente));
    $('geralAbsorcao').textContent = pcts.length ? `${Math.min(...pcts)}–${Math.max(...pcts)}%` : '—';
    $('geralAbsorcaoNota').textContent = txt('geral.absorcao_nota', 'no agente • faixa de {n} produtos', { n: pcts.length });
    const lista = Object.entries(d).map(([k, p]) => {
      const r = el('div', 'mini');
      r.dataset.produto = k;
      r.append(el('span', null, NOMES_PRODUTO[k] || k), barraTresEstados(p), el('b', null, `${p.pct_agente}%`));
      return r;
    });
    $('geralAbsorcaoLista').replaceChildren(legendaTresEstados(null), ...lista);
  } else {
    $('geralAbsorcaoNota').textContent = txt('geral.ausente', '{arquivo} ausente', { arquivo: 'absorcao.json' });
    semFonte($('geralAbsorcaoLista'), 'absorcao.json', ab.reason);
  }
};

// Trocar o idioma redesenha a tela aberta que escreveu texto pelo JS (o JSON vem do cache).
idiomas.aoTrocar(() => {
  const t = document.body.dataset.tela;
  if (['geral', 'agentes', 'ferramentas', 'absorcao'].includes(t)) carregadores[t]();
});

/* O IDE (cliente do motor de terminal, trilha, espelho) mora em assets/ide.js, carregado
   async: fora do caminho do DOMContentLoaded do celular. */


/* ===================== BUSCA E COMANDOS (Ctrl+K) ===================== */
// O botao da lupa e o Ctrl+K abrem um <dialog> modal (o navegador prende o foco nele e o
// Esc fecha). Os dois botoes de enfeite do topo (⌕ e ⌘) nao faziam nada e tinham por nome
// so o simbolo (qualificacao de 01/10/2026, G6): sairam, e este faz o que eles prometiam.
// Rotulo de comando sai da fabrica; nome de agente e de ferramenta e DADO (JSON gerado).
const paleta = (() => {
  const dlg = document.getElementById('paleta');
  const busca = document.getElementById('paletaBusca');
  const lista = document.getElementById('paletaLista');
  const botao = document.getElementById('abrirPaleta');
  let itens = [];
  let ativo = 0;
  let dados = [];
  const normal = t => String(t).normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
  const TELAS = {
    geral: () => txt('menu.geral', 'Visão geral'), agentes: () => txt('menu.agentes', 'Agentes'), ide: () => 'IDE',
    ferramentas: () => txt('menu.ferramentas', 'Ferramentas'), absorcao: () => txt('menu.absorcao', 'Absorção'),
    tarefas: () => txt('menu.tarefas', 'Tarefas'), config: () => txt('menu.config', 'Configuração'),
  };
  const filtrarEm = (tela, campo, nome) => () => {
    mostrarTela(tela);
    const f = document.getElementById(campo);
    f.value = nome;
    f.dispatchEvent(new Event('input', { bubbles: true }));
  };
  function comandos() {
    return [
      ...Object.entries(TELAS).map(([t, r]) => ({ rotulo: txt('paleta.ir', 'Ir para {tela}', { tela: r() }), fazer: () => mostrarTela(t) })),
      { rotulo: txt('paleta.idioma', 'Trocar o idioma da tela'), fazer: () => document.getElementById('trocarIdioma').click() },
      { rotulo: txt('paleta.terminal', 'Abrir um terminal bash'), fazer: () => { mostrarTela('ide'); document.getElementById('ideAbrirBash').click(); } },
      { rotulo: txt('paleta.nova_tarefa', 'Nova tarefa'), fazer: () => { mostrarTela('tarefas'); document.getElementById('tarefasObjetivo').focus(); } },
    ];
  }
  async function carregarDados() {
    const [eq, fe] = await Promise.allSettled([lerJson('equipe.json'), lerJson('ferramentas.json')]);
    dados = [
      ...(eq.status === 'fulfilled' ? eq.value.macroareas.flatMap(m => m.papeis.map(pp => ({ tipo: 'agente', nome: pp.nome }))) : []),
      ...(fe.status === 'fulfilled' ? fe.value.ferramentas.map(f => ({ tipo: 'ferramenta', nome: f.nome })) : []),
    ];
  }
  function desenhar() {
    const q = normal(busca.value.trim());
    const cmds = comandos().filter(c => !q || normal(c.rotulo).includes(q));
    // Dado so com duas letras ou mais: a lista inteira de 175 nomes nao ajuda ninguem.
    const achados = q.length < 2 ? [] : dados.filter(d => normal(d.nome).includes(q)).slice(0, 30).map(d => (d.tipo === 'agente'
      ? { rotulo: txt('paleta.agente', 'Agente: {nome}', { nome: d.nome }), fazer: filtrarEm('agentes', 'agentesFiltro', d.nome) }
      : { rotulo: txt('paleta.ferramenta', 'Ferramenta: {nome}', { nome: d.nome }), fazer: filtrarEm('ferramentas', 'ferramentasFiltro', d.nome) }));
    itens = [...cmds, ...achados];
    ativo = Math.min(ativo, Math.max(0, itens.length - 1));
    lista.replaceChildren(...itens.map((it, i) => {
      const li = el('li', i === ativo ? 'ativo' : '', it.rotulo);
      li.id = `paleta-op-${i}`;
      li.setAttribute('role', 'option');
      li.setAttribute('aria-selected', String(i === ativo));
      li.addEventListener('click', () => executar(i));
      return li;
    }));
    if (!itens.length) lista.append(el('li', 'vazio', txt('paleta.nada', 'Nada encontrado.')));
    busca.setAttribute('aria-activedescendant', itens.length ? `paleta-op-${ativo}` : '');
    lista.querySelector('.ativo')?.scrollIntoView({ block: 'nearest' });
  }
  // Fechar devolve o foco ao botao, menos quando o comando levou o foco para outro lugar.
  let devolver = true;
  function executar(i) {
    const it = itens[i];
    if (!it) return;
    devolver = false;
    dlg.close();
    it.fazer();
  }
  function abrir() {
    if (dlg.open) return;
    busca.value = '';
    ativo = 0;
    desenhar();
    dlg.showModal();
    busca.focus();
    carregarDados().then(desenhar);
  }
  busca.addEventListener('input', () => { ativo = 0; desenhar(); });
  busca.addEventListener('keydown', e => {
    if (e.key === 'ArrowDown') { e.preventDefault(); ativo = Math.min(itens.length - 1, ativo + 1); desenhar(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); ativo = Math.max(0, ativo - 1); desenhar(); }
    else if (e.key === 'Enter') { e.preventDefault(); executar(ativo); }
  });
  dlg.addEventListener('click', e => { if (e.target === dlg) dlg.close(); });
  dlg.addEventListener('close', () => { if (devolver) botao.focus(); devolver = true; });
  botao.addEventListener('click', abrir);
  // Ctrl+K (Cmd+K no Mac) em qualquer lugar, menos no terminal: la o Ctrl+K e do shell.
  document.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && (e.key === 'k' || e.key === 'K')) {
      if (document.activeElement?.id === 'termCanvas' || !app.classList.contains('visible')) return;
      e.preventDefault();
      abrir();
    }
  });
  idiomas.aoTrocar(() => { if (dlg.open) desenhar(); });
  return { abrir };
})();

mostrarTela(params.get('tela') || location.hash.slice(1) || 'geral');

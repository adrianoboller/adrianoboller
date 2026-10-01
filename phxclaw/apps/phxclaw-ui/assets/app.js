const params = new URLSearchParams(location.search);
const forced = params.get('screen');
const splash = document.getElementById('splash');
const app = document.getElementById('app');
const progress = document.getElementById('progressBar');
const pct = document.getElementById('bootPct');
const label = document.getElementById('bootLabel');
const moduleBox = document.getElementById('bootModules');
const enter = document.getElementById('enterButton');

const nativeBridgeStatus = document.getElementById('nativeBridgeStatus');
const liveEventStatus = document.getElementById('liveEventStatus');
const apiEndpoint = document.getElementById('apiEndpoint');
const hostSession = document.getElementById('hostSession');
const liveReceivers = document.getElementById('liveReceivers');
const evidenceState = document.getElementById('evidenceState');
const hostPolicy = document.getElementById('hostPolicy');
const eventConsole = document.getElementById('eventConsole');

const steps = [
  ['Microkernel', 'Constituição + UUIDv7'],
  ['Research Core', 'Evidências'],
  ['Hypothesis Core', 'Experimentos'],
  ['Installer Core', 'Rollback'],
  ['Plugin Registry', 'Ed25519 + SHA-256'],
  ['Agent Runtime', 'Sandbox + routing'],
  ['Task Graph', 'DAG + approvals'],
  ['Model Gateway', 'Budget + policy'],
  ['Event Bus', 'Outbox + live fan-out'],
  ['Connectivity', 'HTTP + PostgreSQL + Ollama'],
  ['Desktop Host', 'Tauri + SSE + WebSocket'],
  ['Evidence Ledger', 'UUIDv7 + hash chain'],
  ['Desktop Fabric', 'WebView + shell + input'],
  ['Media & Documents', 'OCR + voice + Office I/O'],
  ['Mindset + BPM', 'BPMN + deterministic gates'],
];

steps.forEach(([name]) => {
  const span = document.createElement('span');
  span.textContent = name;
  moduleBox.appendChild(span);
});

function revealApp() {
  splash.classList.add('hidden');
  app.classList.add('visible');
  app.setAttribute('aria-hidden', 'false');
}

async function boot() {
  for (let i = 0; i < steps.length; i++) {
    const [name, detail] = steps[i];
    label.textContent = `${name} • ${detail}`;
    const value = Math.round(((i + 1) / steps.length) * 100);
    progress.style.width = `${value}%`;
    pct.textContent = `${value}%`;
    await new Promise(r => setTimeout(r, 190));
    moduleBox.children[i].classList.add('done');
  }
  label.textContent = 'Bootstrap visual concluído';
  enter.classList.add('ready');
  if (forced !== 'splash') setTimeout(revealApp, 420);
}

enter.addEventListener('click', revealApp);

if (forced === 'dashboard') {
  document.body.classList.add('force-dashboard');
  app.classList.add('visible');
  app.setAttribute('aria-hidden', 'false');
} else if (forced === 'splash') {
  document.body.classList.add('force-splash');
  boot();
} else {
  boot();
}

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;
const liveEvents = [];

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
    meta.textContent = `${compactUuid(item.uuid)} • ${new Date(item.occurred_at).toLocaleTimeString()}`;
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

async function connectNativeBridge() {
  if (!invoke || !listen) {
    nativeBridgeStatus.textContent = 'WEB PREVIEW';
    liveEventStatus.textContent = 'NO NATIVE HOST';
    return;
  }

  nativeBridgeStatus.textContent = 'TAURI CONNECTED';
  nativeBridgeStatus.classList.add('native-ok');

  try {
    const status = await invoke('host_status');
    // A versao vem do host (CARGO_PKG_VERSION); cravada no HTML, ficou em v0.11.0 ate a 0.70.
    for (const id of ['brandVersion', 'kernelVersion']) {
      const el = document.getElementById(id);
      if (el) el.textContent = `v${status.version}`;
    }
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
    nativeBridgeStatus.textContent = 'TAURI ERROR';
    console.error('PhxClaw host status failed', error);
  }

  await listen('phoenix:event', ({ payload }) => {
    liveEventStatus.textContent = 'EVENT BUS LIVE';
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
const cacheJson = new Map();
function lerJson(nome) {
  if (!cacheJson.has(nome)) {
    cacheJson.set(nome, fetch(`./assets/${nome}`, { cache: 'no-store' }).then(r => {
      if (!r.ok) throw new Error(`${nome}: HTTP ${r.status}`);
      return r.json();
    }));
  }
  return cacheJson.get(nome);
}

function el(tag, classe, texto) {
  const e = document.createElement(tag);
  if (classe) e.className = classe;
  if (texto !== undefined && texto !== null) e.textContent = String(texto);
  return e;
}

function avisoAusente(resumo, conteudo, arquivo, comando) {
  resumo.textContent = txt('aviso.sem_arquivo', 'assets/{arquivo} não existe — a tela não mostra número sem fonte. Gere com: {comando}', { arquivo, comando });
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
  try { d = await lerJson('equipe.json'); } catch {
    avisoAusente(resumo, conteudo, 'equipe.json', 'cargo run -p phxclaw-agent --example equipe_json');
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
  try { d = await lerJson('ferramentas.json'); } catch {
    avisoAusente(resumo, conteudo, 'ferramentas.json', 'tools/gerar_assets_ui.sh');
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

const NOMES_PRODUTO = { openclaw: 'OpenClaw', hermes: 'Hermes', claude_code: 'Claude Code', codex: 'Codex', openjarvis: 'OpenJarvis' };

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
  try { d = await lerJson('absorcao.json'); } catch {
    avisoAusente(resumo, conteudo, 'absorcao.json', 'tools/gerar_assets_ui.sh');
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
function semFonte(alvo, arquivo) {
  alvo.replaceChildren(el('p', 'vazio aviso', txt('geral.sem_fonte', 'assets/{arquivo} não existe — sem fonte, sem número.', { arquivo })));
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
    semFonte($('geralMacro'), 'equipe.json');
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
    semFonte($('geralCapacidades'), 'ferramentas.json');
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
    semFonte($('geralAbsorcaoLista'), 'absorcao.json');
  }
};

// Trocar o idioma redesenha a tela aberta que escreveu texto pelo JS (o JSON vem do cache).
idiomas.aoTrocar(() => {
  const t = document.body.dataset.tela;
  if (['geral', 'agentes', 'ferramentas', 'absorcao'].includes(t)) carregadores[t]();
});

/* ===================== IDE: cliente do motor de terminal ===================== */
// A tela so desenha e repassa tecla: PTY, emulador, traducao de tecla pelo modo do terminal
// e a diferenca de grade moram no crate phxclaw-terminal (o mesmo motor para tela, teste e
// agente). Aqui chegam trechos ja com cor resolvida; aqui sai {tecla, ctrl, alt, shift}.
const ide = (() => {
  const canvas = document.getElementById('termCanvas');
  const area = document.getElementById('ideArea');
  const vazio = document.getElementById('ideVazio');
  const abas = document.getElementById('ideAbas');
  const status = document.getElementById('ideStatus');
  const botaoHelix = document.getElementById('ideAbrirHelix');
  const botaoBash = document.getElementById('ideAbrirBash');
  const botaoFechar = document.getElementById('ideFechar');
  const ctx = canvas.getContext('2d');
  const FONTE_PX = 14;
  const FONTE = `${FONTE_PX}px "DejaVu Sans Mono", ui-monospace, Menlo, Consolas, monospace`;
  const sessoes = new Map();
  const pendentes = new Map();
  let ativa = null;
  let cel = { w: 8, h: 17 };
  let focado = false;

  const hex = n => `#${(n >>> 0).toString(16).padStart(6, '0')}`;

  function medir() {
    ctx.font = FONTE;
    cel = { w: ctx.measureText('M').width, h: Math.ceil(FONTE_PX * 1.25) };
  }

  function tamanhoQueCabe() {
    const r = canvas.getBoundingClientRect();
    if (r.width < 20 || r.height < 20) return null;
    medir();
    return { colunas: Math.max(2, Math.floor(r.width / cel.w)), linhas: Math.max(1, Math.floor(r.height / cel.h)) };
  }

  function prepararCanvas() {
    const r = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(r.width * dpr), h = Math.round(r.height * dpr);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    medir();
  }

  function desenharLinha(s, y) {
    const linha = s.grade[y] || [];
    ctx.fillStyle = hex(s.fundo);
    ctx.fillRect(0, y * cel.h, canvas.width, cel.h);
    ctx.textBaseline = 'top';
    for (const t of linha) {
      const x0 = t.x * cel.w;
      if (t.fundo !== s.fundo) {
        ctx.fillStyle = hex(t.fundo);
        ctx.fillRect(x0, y * cel.h, t.largura * cel.w, cel.h);
      }
      ctx.font = `${t.estilo & 2 ? 'italic ' : ''}${t.estilo & 1 ? 'bold ' : ''}${FONTE}`;
      ctx.fillStyle = hex(t.frente);
      const chars = [...t.texto];
      const ty = y * cel.h + 2;
      if (chars.length === t.largura) chars.forEach((c, i) => { if (c !== ' ') ctx.fillText(c, x0 + i * cel.w, ty); });
      else ctx.fillText(t.texto, x0, ty);
      if (t.estilo & 4) ctx.fillRect(x0, y * cel.h + cel.h - 2, t.largura * cel.w, 1);
      if (t.estilo & 8) ctx.fillRect(x0, y * cel.h + cel.h / 2, t.largura * cel.w, 1);
    }
    const c = s.cursor;
    if (c && c.y === y && !s.encerrado) {
      const cx = c.x * cel.w, cy = c.y * cel.h;
      ctx.fillStyle = '#ffc43d';
      ctx.strokeStyle = '#ffc43d';
      if (!focado || c.forma === 'oco') ctx.strokeRect(cx + 0.5, cy + 0.5, cel.w - 1, cel.h - 1);
      else if (c.forma === 'barra') ctx.fillRect(cx, cy, 2, cel.h);
      else if (c.forma === 'sublinhado') ctx.fillRect(cx, cy + cel.h - 2, cel.w, 2);
      else { ctx.globalAlpha = 0.55; ctx.fillRect(cx, cy, cel.w, cel.h); ctx.globalAlpha = 1; }
    }
  }

  function desenhar(s, linhas) {
    if (s !== ativa || document.body.dataset.tela !== 'ide') return;
    prepararCanvas();
    const todas = linhas === null;
    if (todas) {
      ctx.fillStyle = hex(s.fundo);
      ctx.fillRect(0, 0, canvas.width, canvas.height);
    }
    const ys = todas ? [...Array(s.linhas).keys()] : linhas;
    for (const y of new Set(ys)) desenharLinha(s, y);
  }

  function aplicar(s, g) {
    const tocadas = [];
    const mudouTamanho = g.colunas !== s.colunas || g.linhas !== s.linhas;
    if (g.completa || g.linhas !== s.linhas) s.grade = new Array(g.linhas).fill(null).map(() => []);
    s.colunas = g.colunas;
    s.linhas = g.linhas;
    s.fundo = g.fundo;
    s.frente = g.frente;
    for (const l of g.linhas_alteradas) { s.grade[l.y] = l.trechos; tocadas.push(l.y); }
    if (s.cursor) tocadas.push(s.cursor.y);
    s.cursor = g.cursor;
    if (s.cursor) tocadas.push(s.cursor.y);
    if (g.titulo !== null && g.titulo !== undefined) s.titulo = g.titulo;
    if (g.encerrado) s.encerrado = g.encerrado;
    desenhar(s, g.completa ? null : tocadas);
    // O status mostra o tamanho que o MOTOR confirmou, nao o que a tela pediu.
    if (g.titulo != null || g.encerrado || mudouTamanho) atualizarAbas();
  }

  function aoGrade(payload) {
    const s = sessoes.get(payload.id);
    if (!s) {
      // A primeira grade pode chegar antes de o invoke do abrir voltar com o id.
      if (!pendentes.has(payload.id)) pendentes.set(payload.id, []);
      pendentes.get(payload.id).push(payload);
      return;
    }
    aplicar(s, payload);
  }

  function atualizarAbas() {
    abas.replaceChildren(...[...sessoes.values()].map(s => {
      const b = el('button', `${s === ativa ? 'ativa' : ''} ${s.encerrado ? 'morto' : ''}`.trim(),
        `${s.programa === 'helix' ? 'Helix' : 'bash'}${s.titulo ? ` — ${s.titulo}` : ''}`);
      b.dataset.id = s.id;
      b.onclick = () => ativar(s);
      return b;
    }));
    seloIde.textContent = sessoes.size ? String(sessoes.size) : '';
    vazio.hidden = !!ativa;
    botaoFechar.disabled = !ativa;
    if (!ativa) { status.textContent = txt('ide.nenhum', 'Nenhum terminal aberto.'); return; }
    const fim = ativa.encerrado
      ? txt('ide.encerrado', ' • encerrado (código {codigo})', { codigo: ativa.encerrado.codigo ?? txt('ide.sinal', 'sinal') }) : '';
    status.textContent = txt('ide.status', '{programa} • pid {pid} • {colunas}×{linhas} • {cwd}{fim}', {
      programa: ativa.programa === 'helix' ? 'Helix' : 'bash', pid: ativa.pid, colunas: ativa.colunas, linhas: ativa.linhas, cwd: ativa.cwd, fim,
    });
  }
  // Troca de idioma: reescreve o status, menos quando ele mostra erro vindo do host (dado).
  const semHost = () => txt('ide.sem_host', 'O terminal exige o host nativo (Tauri); esta é a prévia web.');
  idiomas.aoTrocar(() => {
    if (!status.classList.contains('aviso')) atualizarAbas();
    else if (!invoke) status.textContent = semHost();
  });

  function ativar(s) {
    ativa = s;
    atualizarAbas();
    redimensionar();
    desenhar(s, null);
    canvas.focus();
  }

  async function abrir(programa) {
    if (!invoke) { status.textContent = semHost(); status.classList.add('aviso'); return; }
    const t = tamanhoQueCabe() || { colunas: 100, linhas: 30 };
    try {
      const r = await invoke('terminal_abrir', { programa, colunas: t.colunas, linhas: t.linhas });
      const s = { ...r, colunas: t.colunas, linhas: t.linhas, grade: [], fundo: 0x010418, frente: 0xe6edf3, cursor: null, titulo: '', encerrado: null };
      sessoes.set(r.id, s);
      status.classList.remove('aviso');
      ativar(s);
      for (const p of pendentes.get(r.id) || []) aplicar(s, p);
      pendentes.delete(r.id);
    } catch (e) {
      status.textContent = String(e?.message ?? e);
      status.classList.add('aviso');
    }
  }

  async function fechar() {
    if (!ativa) return;
    const s = ativa;
    sessoes.delete(s.id);
    ativa = [...sessoes.values()].pop() || null;
    atualizarAbas();
    if (ativa) desenhar(ativa, null);
    else { prepararCanvas(); ctx.clearRect(0, 0, canvas.width, canvas.height); }
    try { await invoke('terminal_fechar', { id: s.id }); } catch (e) { console.error(e); }
  }

  let espera = null;
  function redimensionar() {
    clearTimeout(espera);
    espera = setTimeout(() => {
      const s = ativa;
      const t = tamanhoQueCabe();
      if (!s || !t || s.encerrado) return;
      if (t.colunas === s.colunas && t.linhas === s.linhas) { desenhar(s, null); return; }
      invoke('terminal_redimensionar', { id: s.id, colunas: t.colunas, linhas: t.linhas })
        .catch(e => console.error(e));
    }, 60);
  }

  canvas.addEventListener('keydown', e => {
    // So tecla de gente: um evento sintetico (dispatchEvent pela ponte de DOM) nao e
    // isTrusted, e nao pode digitar num shell.
    if (!e.isTrusted || !ativa || ativa.encerrado || e.isComposing) return;
    if (e.metaKey) return;
    // Modificador sozinho nao vira byte; nem vale a ida ao host.
    if (['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'CapsLock'].includes(e.key)) return;
    // Ctrl+Shift+V cola e Ctrl+Shift+C copia: ficam com o navegador, como num terminal.
    if (e.ctrlKey && e.shiftKey && (e.key === 'V' || e.key === 'C')) return;
    e.preventDefault();
    invoke('terminal_escrever', { id: ativa.id, tecla: { tecla: e.key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey } })
      .catch(err => console.error(err));
  });
  document.addEventListener('paste', e => {
    if (document.activeElement !== canvas || !ativa || !e.isTrusted) return;
    const texto = e.clipboardData?.getData('text/plain');
    if (!texto) return;
    e.preventDefault();
    invoke('terminal_escrever', { id: ativa.id, colar: texto }).catch(err => console.error(err));
  });
  canvas.addEventListener('wheel', e => {
    if (!ativa) return;
    e.preventDefault();
    invoke('terminal_rolar', { id: ativa.id, linhas: e.deltaY < 0 ? 3 : -3 }).catch(() => {});
  }, { passive: false });
  canvas.addEventListener('focus', () => { focado = true; if (ativa?.cursor) desenhar(ativa, [ativa.cursor.y]); });
  canvas.addEventListener('blur', () => { focado = false; if (ativa?.cursor) desenhar(ativa, [ativa.cursor.y]); });
  canvas.addEventListener('mousedown', () => canvas.focus());
  new ResizeObserver(redimensionar).observe(area);

  botaoHelix.addEventListener('click', () => abrir('helix'));
  botaoBash.addEventListener('click', () => abrir('bash'));
  botaoFechar.addEventListener('click', fechar);
  if (listen) listen('terminal_grade', ({ payload }) => aoGrade(payload));

  return { aoMostrar: () => { if (ativa) { redimensionar(); desenhar(ativa, null); canvas.focus(); } } };
})();
carregadores.ide = ide.aoMostrar;

mostrarTela(params.get('tela') || location.hash.slice(1) || 'geral');

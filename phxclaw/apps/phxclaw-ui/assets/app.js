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
    hostSession.textContent = compactUuid(status.session_uuid);
    liveReceivers.textContent = String(status.live_bus?.receiver_count ?? 0);
    hostPolicy.textContent = [
      status.policy?.command_execution ? 'CMD' : null,
      status.policy?.desktop_input ? 'INPUT' : null,
      status.policy?.screen_capture ? 'SCREEN' : null,
      status.policy?.webview_control ? 'DOM' : null,
    ].filter(Boolean).join(' + ') || 'DENY';
    apiEndpoint.textContent = status.api?.addr ? `API ${status.api.addr}` : 'API OFFLINE';

    const verify = await invoke('verify_evidence');
    evidenceState.textContent = verify.valid ? `VALID • ${verify.records}` : 'INVALID';

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

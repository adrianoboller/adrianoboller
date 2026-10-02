// Fora do app.js de proposito: a tela IDE nao e tela de celular, e cada KB no caminho do
// DOMContentLoaded custa no 4G lento (medido em 02/10/2026: +11 KB no app.js levaram o
// DCL da Visao geral no celular de 1.440 para 1.714 ms; o teto da sonda M5 e 1.600).
// Entra por <script async> depois dos classicos e so se arma no DOMContentLoaded, para
// achar o txt(), o el(), o idiomas e os carregadores que o app.js define.
/* ===================== IDE: cliente do motor de terminal ===================== */
// A tela so desenha e repassa tecla: PTY, emulador, traducao de tecla pelo modo do terminal
// e a diferenca de grade moram no crate phxclaw-terminal (o mesmo motor para tela, teste e
// agente). Aqui chegam trechos ja com cor resolvida; aqui sai {tecla, ctrl, alt, shift}.
(() => {
const iniciar = () => {
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

  // TEMA DO TERMINAL: o motor manda as cores ja resolvidas para o fundo escuro da marca (o
  // fundo padrao dele E o --fundo escuro). No claro o canvas acompanha a folha: o fundo e o
  // texto padrao viram os tokens --fundo e --texto (e o video reverso, que troca os dois,
  // continua trocado), e toda outra cor de letra escurece ate 4,5:1 sobre o fundo dela -- o
  // mesmo que o minimumContrastRatio dos emuladores faz. O TEXTO nao muda: so a tinta.
  let tintas = null;
  const cache = new Map();
  const rgb = n => [(n >>> 16) & 255, (n >>> 8) & 255, n & 255];
  const deHex = h => parseInt(h.replace('#', '').replace(/^(.)(.)(.)$/, '$1$1$2$2$3$3'), 16);
  const lum = c => { const l = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; const [r, g, b] = rgb(c); return 0.2126 * l(r) + 0.7152 * l(g) + 0.0722 * l(b); };
  const razao = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  function lerTintas() {
    const cs = getComputedStyle(document.documentElement);
    const t = n => deHex(cs.getPropertyValue(n).trim());
    tintas = { claro: document.documentElement.dataset.tema === 'claro', fundo: t('--fundo'), texto: t('--texto'), cursor: cs.getPropertyValue('--ambar').trim() };
    cache.clear();
  }
  // O fundo de um trecho no tema da tela.
  function fundoNoTema(s, n) {
    if (!tintas.claro) return n;
    if (n === s.fundo) return tintas.fundo;
    if (n === s.frente) return tintas.texto;
    return n;
  }
  // A letra de um trecho no tema da tela, legivel sobre o fundo dela.
  function letraNoTema(s, n, fundo) {
    if (!tintas.claro) return n;
    if (n === s.frente) return tintas.texto;
    if (n === s.fundo) return tintas.fundo;
    const k = `${n}/${fundo}`;
    if (cache.has(k)) return cache.get(k);
    let c = n;
    const escurecer = lum(fundo) > 0.18;
    for (let i = 0; i < 20 && razao(c, fundo) < 4.5; i++) {
      const [r, g, b] = rgb(c).map(v => Math.round(escurecer ? v * 0.85 : v + (255 - v) * 0.15));
      c = (r << 16) | (g << 8) | b;
    }
    cache.set(k, c);
    return c;
  }

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
    if (!tintas) lerTintas();
    const linha = s.grade[y] || [];
    ctx.fillStyle = hex(fundoNoTema(s, s.fundo));
    ctx.fillRect(0, y * cel.h, canvas.width, cel.h);
    ctx.textBaseline = 'top';
    for (const t of linha) {
      const x0 = t.x * cel.w;
      const fundo = fundoNoTema(s, t.fundo);
      if (t.fundo !== s.fundo) {
        ctx.fillStyle = hex(fundo);
        ctx.fillRect(x0, y * cel.h, t.largura * cel.w, cel.h);
      }
      ctx.font = `${t.estilo & 2 ? 'italic ' : ''}${t.estilo & 1 ? 'bold ' : ''}${FONTE}`;
      ctx.fillStyle = hex(letraNoTema(s, t.frente, fundo));
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
      ctx.fillStyle = tintas.cursor;
      ctx.strokeStyle = tintas.cursor;
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
      if (!tintas) lerTintas();
      ctx.fillStyle = hex(fundoNoTema(s, s.fundo));
      ctx.fillRect(0, 0, canvas.width, canvas.height);
    }
    const ys = todas ? [...Array(s.linhas).keys()] : linhas;
    for (const y of new Set(ys)) desenharLinha(s, y);
  }

  function aplicar(s, g) {
    if (g.so_fim) {
      s.encerrado = g.encerrado;
      atualizarAbas();
      if (s === ativa && s.cursor) desenhar(s, [s.cursor.y]);
      return;
    }
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
    if (s === ativa) { espelhar(); lerArquivo(s); }
  }

  // ACESSIBILIDADE: o canvas e invisivel ao leitor de tela. O espelho (#termEspelho,
  // aria-live) diz a linha do cursor e a linha de estado do programa, em texto -- dado cru
  // do terminal, por isso so o rotulo passa pela fabrica. Com atraso: anunciar a cada tecla
  // afogaria o leitor; a cada 300 ms ele ouve o que ficou.
  const espelho = document.getElementById('termEspelho');
  const textoDaLinha = (s, y) => (s.grade[y] || []).map(t => t.texto).join('').replace(/\s+$/, '');
  // Com freio e nao com atraso reiniciavel: o Helix redesenha sem parar enquanto o LSP
  // indexa (o spinner da linha de estado), e um atraso que se reinicia a cada quadro nunca
  // dispararia -- o espelho ficou vazio no agente real (medido, 02/10/2026).
  let esperaEspelho = null;
  function espelhar() {
    if (!espelho || esperaEspelho) return;
    esperaEspelho = setTimeout(() => {
      esperaEspelho = null;
      const s = ativa;
      if (!s || !s.grade.length) { espelho.textContent = txt('ide.espelho_vazio', 'Terminal sem texto.'); return; }
      const y = s.cursor ? s.cursor.y : 0;
      // A linha de estado e a do modo (`NOR ...`), que no Helix fica ACIMA da linha de
      // comando; sem ela (bash), a ultima linha da tela.
      let estado = -1;
      for (let i = s.linhas - 1; i >= 0 && estado < 0; i--) if (/^\s*(NOR|INS|SEL)\s/.test(textoDaLinha(s, i))) estado = i;
      espelho.textContent = txt('ide.espelho', 'Linha {y}: {linha}. Estado: {status}', { y: y + 1, linha: textoDaLinha(s, y), status: textoDaLinha(s, estado >= 0 ? estado : s.linhas - 1) });
    }, 300);
  }

  // TRILHA (breadcrumbs): o arquivo aberto no Helix e os simbolos dele. O Helix 25.07 nao
  // expoe o documento corrente por canal nenhum (nem titulo, nem variavel -- lido no fonte
  // dele); a UNICA fonte e a linha de estado dele (`NOR   src/main.rs[+]  ...`), lida da
  // grade pela mesma regra do motor (phxclaw-terminal::helix::arquivo_corrente). Os
  // simbolos vem do rust-analyzer/pyright pelo agente (/v1/ide/simbolos) -- so pela rede;
  // no aplicativo de mesa a trilha mostra o caminho.
  const trilha = document.getElementById('ideTrilha');
  let arquivoAberto = null;
  let simbolos = { arquivo: null, lista: null, erro: null, lendo: false };
  function arquivoDaGrade(s) {
    for (let y = s.linhas - 1; y >= 0; y--) {
      const partes = textoDaLinha(s, y).trim().split(/\s+/);
      if (!/^(NOR|INS|SEL)$/.test(partes[0] || '')) continue;
      // O spinner do LSP (braille U+2800-28FF) fica entre o modo e o arquivo enquanto o
      // servidor indexa: nao e nome de arquivo.
      const a = (partes.slice(1).find(t => !/^[\u2800-\u28ff]+$/.test(t)) || '').replace(/\[\+\]$/, '');
      if (!a || a.startsWith('[') || a === 'sel') return null;
      return a;
    }
    return null;
  }
  function lerArquivo(s) {
    const a = s.programa === 'helix' ? arquivoDaGrade(s) : null;
    if (a === arquivoAberto) return;
    arquivoAberto = a;
    simbolos = { arquivo: a, lista: null, erro: null, lendo: false };
    desenharTrilha();
    if (a && comHttp) buscarSimbolos(a);
  }
  async function buscarSimbolos(arquivo) {
    simbolos.lendo = true;
    desenharTrilha();
    try {
      const r = await fetch(`./v1/ide/simbolos?arquivo=${encodeURIComponent(arquivo)}`, { headers: { Authorization: `Bearer ${token()}` }, cache: 'no-store' });
      const v = await r.json().catch(() => ({}));
      if (arquivo !== arquivoAberto) return;
      if (!r.ok) simbolos = { arquivo, lista: null, erro: v.error || `HTTP ${r.status}`, lendo: false };
      else simbolos = { arquivo, lista: v.simbolos || [], erro: null, lendo: false };
    } catch {
      if (arquivo !== arquivoAberto) return;
      simbolos = { arquivo, lista: null, erro: txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), lendo: false };
    }
    desenharTrilha();
  }
  // Clique na trilha manda um comando ao Helix: Esc (volta ao modo normal), o comando, Enter.
  // O Esc vai SOZINHO e o comando depois: ESC seguido de byte no mesmo lote e Alt+tecla
  // para o Helix (crossterm), e `Alt+:` nao abre a linha de comando.
  function comandoHelix(cmd) {
    if (!ativa || ativa.encerrado) return;
    const s = ativa;
    transporte.escrever(s.id, { texto: '\u001b' }).catch(err => console.error(err));
    setTimeout(() => { if (ativa === s && !s.encerrado) transporte.escrever(s.id, { texto: `:${cmd}\r` }).catch(err => console.error(err)); }, 80);
    canvas.focus();
  }
  function desenharTrilha() {
    if (!trilha) return;
    trilha.replaceChildren();
    if (!ativa || ativa.programa !== 'helix') { trilha.hidden = true; return; }
    trilha.hidden = false;
    if (!arquivoAberto) { trilha.append(el('span', 'trilha-vazia', txt('ide.trilha_nenhum', 'Nenhum arquivo aberto no Helix.'))); return; }
    const ol = el('ol', 'trilha-caminho');
    const partes = arquivoAberto.split('/');
    partes.forEach((parte, i) => {
      const li = el('li');
      if (i < partes.length - 1) {
        const pasta = partes.slice(0, i + 1).join('/');
        const b = el('button', 'trilha-seg', parte);
        b.type = 'button';
        b.title = txt('ide.trilha_abrir', 'Abrir a pasta {pasta} no Helix', { pasta });
        b.addEventListener('click', () => comandoHelix(`open ${pasta}`));
        li.append(b);
      } else li.append(el('b', 'trilha-arq', parte));
      ol.append(li);
    });
    trilha.append(ol);
    const ul = el('ul', 'trilha-simbolos');
    ul.setAttribute('aria-label', txt('ide.trilha_simbolos', 'Símbolos do arquivo'));
    if (simbolos.lendo) ul.append(el('li', 'trilha-nota', txt('ide.trilha_lendo', 'Lendo símbolos…')));
    else if (simbolos.erro) ul.append(el('li', 'trilha-nota aviso', txt('ide.trilha_erro', 'Símbolos indisponíveis: {erro}', { erro: simbolos.erro })));
    else if (simbolos.lista) {
      const achatar = (lista, nivel) => lista.flatMap(x => [{ ...x, nivel }, ...achatar(x.filhos || [], nivel + 1)]);
      for (const si of achatar(simbolos.lista, 0).slice(0, 80)) {
        const li = el('li');
        const b = el('button', 'trilha-sim', `${si.nome}`);
        b.type = 'button';
        b.dataset.tipo = si.tipo;
        b.dataset.nivel = si.nivel;
        b.title = txt('ide.trilha_ir', 'Ir para a linha {linha} ({tipo})', { linha: si.linha, tipo: si.tipo });
        b.addEventListener('click', () => comandoHelix(`goto ${si.linha}`));
        li.append(b);
        ul.append(li);
      }
    }
    trilha.append(ul);
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
  // TRANSPORTE: um canvas, dois caminhos ate o MESMO motor de terminal (phxclaw-terminal).
  // No aplicativo de mesa, os comandos `terminal_*` do Tauri; no navegador, o websocket
  // `/v1/ide/terminal` do agente, com o token da API na primeira mensagem (um websocket nao
  // leva cabecalho). Pelo websocket so o Helix abre -- bash livre pela rede seria o
  // execute_shell sem a politica dele. Sem nenhum dos dois (arquivo local), a previa diz.
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';
  const token = () => { try { return localStorage.getItem('phxclaw.token') || ''; } catch { return ''; } };
  const transporte = invoke ? {
    tipo: 'tauri',
    abrir: (programa, colunas, linhas) => invoke('terminal_abrir', { programa, colunas, linhas }),
    escrever: (id, o) => invoke('terminal_escrever', { id, ...o }),
    redimensionar: (id, colunas, linhas) => invoke('terminal_redimensionar', { id, colunas, linhas }),
    rolar: (id, linhas) => invoke('terminal_rolar', { id, linhas }),
    fechar: id => invoke('terminal_fechar', { id }),
    ouvir: f => { if (listen) listen('terminal_grade', ({ payload }) => f(payload)); },
  } : comHttp ? (() => {
    let ws = null;
    let ouvinte = null;
    let abrindo = null;
    let idAberto = null;
    const mandar = o => { if (ws?.readyState === 1) ws.send(JSON.stringify(o)); };
    // Fio caido ou sessao substituida = terminal encerrado: a aba fica morta pelo mesmo
    // caminho da grade, sem apagar o que estava desenhado.
    const encerrar = () => { if (idAberto && ouvinte) ouvinte({ id: idAberto, so_fim: true, encerrado: { codigo: null } }); idAberto = null; };
    function ligar() {
      if (ws && ws.readyState <= 1) return;
      const base = location.pathname.replace(/[^/]*$/, '');
      ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${base}v1/ide/terminal`);
      ws.onopen = () => mandar({ op: 'auth', token: token() });
      ws.onmessage = ev => {
        let m;
        try { m = JSON.parse(ev.data); } catch { return; }
        if (m.ev === 'aberto') { idAberto = m.id; abrindo?.ok(m); abrindo = null; }
        else if (m.ev === 'grade') ouvinte?.(m);
        else if (m.ev === 'erro') { if (abrindo) { abrindo.falha(new Error(m.erro)); abrindo = null; } else encerrar(); }
      };
      ws.onclose = () => { if (abrindo) { abrindo.falha(Object.assign(new Error('sem rede'), { rede: true })); abrindo = null; } encerrar(); ws = null; };
    }
    return {
      tipo: 'ws',
      abrir: (programa, colunas, linhas) => new Promise((ok, falha) => {
        if (programa !== 'helix') { falha(Object.assign(new Error('so helix'), { soHelix: true })); return; }
        ligar();
        abrindo = { ok, falha };
        const ir = () => mandar({ op: 'abrir', programa, colunas, linhas });
        // O `auth` do onopen sai antes: os ouvintes correm na ordem em que entraram.
        if (ws.readyState === 1) ir(); else ws.addEventListener('open', ir, { once: true });
      }),
      escrever: (id, o) => { mandar(o.tecla ? { op: 'tecla', tecla: o.tecla } : o.colar !== undefined ? { op: 'colar', texto: o.colar } : { op: 'texto', texto: o.texto }); return Promise.resolve(true); },
      redimensionar: (id, colunas, linhas) => { mandar({ op: 'redimensionar', colunas, linhas }); return Promise.resolve(); },
      rolar: (id, linhas) => { mandar({ op: 'rolar', linhas }); return Promise.resolve(); },
      fechar: () => { mandar({ op: 'fechar' }); idAberto = null; return Promise.resolve(); },
      ouvir: f => { ouvinte = f; },
    };
  })() : null;

  // Troca de idioma: reescreve o status, menos quando ele mostra erro vindo do host (dado).
  const semHost = () => txt('ide.sem_host', 'O terminal exige o host nativo (Tauri); esta é a prévia web.');
  const soHelix = () => txt('ide.so_helix_web', 'Pelo navegador só o Helix abre; o terminal bash fica no aplicativo de mesa.');
  let avisoFixo = null;
  idiomas.aoTrocar(() => {
    if (!status.classList.contains('aviso')) atualizarAbas();
    else if (avisoFixo) status.textContent = avisoFixo();
    espelhar();
    desenharTrilha();
  });

  function ativar(s) {
    ativa = s;
    atualizarAbas();
    redimensionar();
    desenhar(s, null);
    arquivoAberto = undefined;
    lerArquivo(s);
    espelhar();
    canvas.focus();
  }

  function avisar(f) { avisoFixo = f; status.textContent = f(); status.classList.add('aviso'); }
  async function abrir(programa) {
    if (!transporte) { avisar(semHost); return; }
    if (transporte.tipo === 'ws' && programa !== 'helix') { avisar(soHelix); return; }
    const t = tamanhoQueCabe() || { colunas: 100, linhas: 30 };
    try {
      const r = await transporte.abrir(programa, t.colunas, t.linhas);
      const s = { ...r, colunas: t.colunas, linhas: t.linhas, grade: [], fundo: 0x010418, frente: 0xe6edf3, cursor: null, titulo: '', encerrado: null };
      sessoes.set(r.id, s);
      status.classList.remove('aviso');
      avisoFixo = null;
      ativar(s);
      for (const p of pendentes.get(r.id) || []) aplicar(s, p);
      pendentes.delete(r.id);
    } catch (e) {
      // Rede caida diz o que fazer, pela fabrica; o motivo do agente e dado e entra cru.
      if (e?.rede) avisar(() => txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'));
      else { avisoFixo = null; status.textContent = String(e?.message ?? e); status.classList.add('aviso'); }
    }
  }

  async function fechar() {
    if (!ativa) return;
    const s = ativa;
    sessoes.delete(s.id);
    ativa = [...sessoes.values()].pop() || null;
    atualizarAbas();
    if (ativa) { desenhar(ativa, null); arquivoAberto = undefined; lerArquivo(ativa); }
    else { prepararCanvas(); ctx.clearRect(0, 0, canvas.width, canvas.height); arquivoAberto = undefined; lerArquivo({ programa: null }); }
    espelhar();
    try { await transporte.fechar(s.id); } catch (e) { console.error(e); }
  }

  let espera = null;
  function redimensionar() {
    clearTimeout(espera);
    espera = setTimeout(() => {
      const s = ativa;
      const t = tamanhoQueCabe();
      if (!s || !t || s.encerrado) return;
      if (t.colunas === s.colunas && t.linhas === s.linhas) { desenhar(s, null); return; }
      transporte.redimensionar(s.id, t.colunas, t.linhas).catch(e => console.error(e));
    }, 60);
  }

  // Saida pelo teclado: Tab e do terminal (completa no bash, indenta no Helix), entao sair
  // pede um gesto que o shell nao usa sozinho: Esc e, logo depois, Tab (ou Shift+Tab). O Esc
  // continua indo ao terminal -- no Helix ele e vital. Antes, o Tab prendia o foco no
  // canvas para sempre (qualificacao de 01/10/2026, G10).
  const JANELA_SAIDA_MS = 1500;
  let escEm = 0;
  canvas.addEventListener('keydown', e => {
    if (e.key === 'Tab' && !e.ctrlKey && !e.altKey && !e.metaKey && Date.now() - escEm < JANELA_SAIDA_MS) {
      escEm = 0;
      return; // sem preventDefault: o navegador move o foco
    }
    escEm = e.key === 'Escape' ? Date.now() : 0;
    // So tecla de gente: um evento sintetico (dispatchEvent pela ponte de DOM) nao e
    // isTrusted, e nao pode digitar num shell.
    if (!e.isTrusted || !ativa || ativa.encerrado || e.isComposing) return;
    if (e.metaKey) return;
    // Modificador sozinho nao vira byte; nem vale a ida ao host.
    if (['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'CapsLock'].includes(e.key)) return;
    // Ctrl+Shift+V cola e Ctrl+Shift+C copia: ficam com o navegador, como num terminal.
    if (e.ctrlKey && e.shiftKey && (e.key === 'V' || e.key === 'C')) return;
    e.preventDefault();
    transporte.escrever(ativa.id, { tecla: { tecla: e.key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey } })
      .catch(err => console.error(err));
  });
  document.addEventListener('paste', e => {
    if (document.activeElement !== canvas || !ativa || !e.isTrusted) return;
    const texto = e.clipboardData?.getData('text/plain');
    if (!texto) return;
    e.preventDefault();
    transporte.escrever(ativa.id, { colar: texto }).catch(err => console.error(err));
  });
  canvas.addEventListener('wheel', e => {
    if (!ativa) return;
    e.preventDefault();
    transporte.rolar(ativa.id, e.deltaY < 0 ? 3 : -3).catch(() => {});
  }, { passive: false });
  canvas.addEventListener('focus', () => { focado = true; if (ativa?.cursor) desenhar(ativa, [ativa.cursor.y]); });
  canvas.addEventListener('blur', () => { focado = false; if (ativa?.cursor) desenhar(ativa, [ativa.cursor.y]); });
  canvas.addEventListener('mousedown', () => canvas.focus());
  new ResizeObserver(redimensionar).observe(area);

  botaoHelix.addEventListener('click', () => abrir('helix'));
  botaoBash.addEventListener('click', () => abrir('bash'));
  botaoFechar.addEventListener('click', fechar);
  transporte?.ouvir(aoGrade);

  // Trocou o tema: as tintas se releem e a tela inteira do terminal se repinta.
  tema.aoTrocar(() => { lerTintas(); if (ativa) desenhar(ativa, null); });
  return { aoMostrar: () => { if (ativa) { redimensionar(); desenhar(ativa, null); canvas.focus(); } } };
})();
carregadores.ide = ide.aoMostrar;
// Quem ja esta na tela IDE quando o modulo chega nao pode esperar o proximo clique.
if (document.body.dataset.tela === 'ide') ide.aoMostrar();
};
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', iniciar);
else iniciar();
})();

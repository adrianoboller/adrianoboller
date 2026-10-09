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
    if (s === ativa) { espelhar(); lerArquivo(s); minimapa.grade(s); leitura.grade(s); }
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
  const linhaDeEstado = s => {
    for (let y = (s.linhas || 0) - 1; y >= 0; y--) if (/^\s*(NOR|INS|SEL)\s/.test(textoDaLinha(s, y))) return y;
    return -1;
  };
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
    // O convidado do terminal compartilhado nao le arquivo: nao tem Bearer, e o disco e do anfitriao.
    const a = s.programa === 'helix' && !s.convidado ? arquivoDaGrade(s) : null;
    // O menu de completar do `:` cobre a linha de estado enquanto se digita um comando: sem
    // linha de estado nao ha «nenhum arquivo», ha linha coberta. Sem isto cada comando
    // digitado zerava a trilha e o minimapa e os pedia de novo ao agente (medido no Helix real).
    if (!a && s.programa === 'helix' && !s.convidado && s.grade?.length && linhaDeEstado(s) < 0) return;
    if (a === arquivoAberto) return;
    arquivoAberto = a;
    simbolos = { arquivo: a, lista: null, erro: null, lendo: false };
    desenharTrilha();
    if (a && comHttp) buscarSimbolos(a);
    minimapa.arquivo(a);
    leitura.arquivo(a);
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
  function comandoHelix(cmd, focar = true) {
    if (!ativa || ativa.encerrado) return;
    const s = ativa;
    transporte.escrever(s.id, { texto: '\u001b' }).catch(err => console.error(err));
    setTimeout(() => { if (ativa === s && !s.encerrado) transporte.escrever(s.id, { texto: `:${cmd}\r` }).catch(err => console.error(err)); }, 80);
    if (focar) canvas.focus();
  }
  function desenharTrilha() {
    if (!trilha) return;
    trilha.replaceChildren();
    if (!ativa || ativa.programa !== 'helix' || ativa.convidado) { trilha.hidden = true; return; }
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

  // MINIMAPA: o arquivo aberto no Helix em 1 px por caractere, com a faixa visivel e o
  // cursor. O Helix 25.07 nao tem minimapa (0 ocorrencias no fonte) e o buffer dele e de OUTRO
  // processo, entao o texto vem do DISCO pelo agente (/v1/ide/arquivo) e a janela vem da
  // grade: a faixa e o primeiro e o ultimo numero da calha, o cursor e o `linha:coluna` da
  // linha de estado. Com `line-number = "relative"` (ou calha desligada) os numeros nao sao
  // a linha do arquivo: o painel cai para «so cursor» em vez de desenhar uma faixa mentirosa.
  // Limite declarado na propria tela: o que nao foi salvo nao aparece.
  const minimapa = (() => {
    const painel = document.getElementById('ideMinimapa');
    const tela = document.getElementById('minimapaCanvas');
    const estadoEl = document.getElementById('minimapaEstado');
    if (!painel || !tela) return { arquivo() {}, grade() {}, estado() {}, pintar() {} };
    const c2 = tela.getContext('2d');
    let arq = null, linhas = null, erro = null, lendo = false;
    let janela = { cursor: null, de: null, ate: null, modificado: false };
    let pedido = 0, quadro = 0;
    // A linha da grade com cada trecho na coluna dele: a calha e alinhada a direita, e a
    // coluna onde o numero termina e o que separa calha de texto que comeca com digito.
    const posicional = (s, y) => { let o = ''; for (const t of s.grade[y] || []) { if (t.x > o.length) o = o.padEnd(t.x); o += t.texto; } return o; };
    function lerJanela(s) {
      const est = linhaDeEstado(s);
      if (est < 0) return null;
      const linhaEst = textoDaLinha(s, est);
      const pos = linhaEst.trim().split(/\s+/).reverse().find(t => /^\d+:\d+$/.test(t));
      const cursor = pos ? Number(pos.split(':')[0]) : null;
      const nums = [];
      for (let y = 0; y < est; y++) {
        const m = /^(\S?\s*)(\d+)\s/.exec(posicional(s, y) + ' ');
        if (m) nums.push({ n: Number(m[2]), fim: m[1].length + m[2].length });
      }
      // Absoluta: crescente, todos terminando na mesma coluna, e o cursor dentro da faixa.
      const ok = nums.length > 0 && nums.every((x, i) => x.fim === nums[0].fim && (i === 0 || x.n > nums[i - 1].n))
        && (cursor === null || (cursor >= nums[0].n && cursor <= nums.at(-1).n));
      return { cursor, de: ok ? nums[0].n : null, ate: ok ? nums.at(-1).n : null, modificado: /\[\+\]/.test(linhaEst) };
    }
    async function buscar(a) {
      const meu = ++pedido;
      lendo = true; erro = null; estado();
      try {
        const r = await fetch(`./v1/ide/arquivo?caminho=${encodeURIComponent(a)}`, { headers: { Authorization: `Bearer ${token()}` }, cache: 'no-store' });
        const v = await r.json().catch(() => ({}));
        if (meu !== pedido) return;
        if (r.ok) { linhas = String(v.texto ?? '').replace(/\n$/, '').split('\n'); erro = null; } else { linhas = null; erro = v.error || `HTTP ${r.status}`; }
      } catch {
        if (meu !== pedido) return;
        linhas = null; erro = txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.');
      }
      lendo = false;
      estado(); pintar();
    }
    function arquivo(a) {
      const helix = ativa?.programa === 'helix' && !ativa.convidado;
      painel.hidden = !helix;
      if (a === arq) return;
      arq = a; linhas = null; erro = null; pedido++;
      if (a && comHttp) buscar(a); else { lendo = false; estado(); pintar(); }
    }
    function grade(s) {
      const j = lerJanela(s);
      if (!j) return;
      // Salvou (o `[+]` sumiu): o disco mudou, o mapa rele.
      if (janela.modificado && !j.modificado && arq && comHttp) buscar(arq);
      const mudou = j.cursor !== janela.cursor || j.de !== janela.de || j.ate !== janela.ate || j.modificado !== janela.modificado;
      janela = j;
      if (!mudou) return;
      estado();
      if (!quadro) quadro = requestAnimationFrame(() => { quadro = 0; pintar(); });
    }
    function estado() {
      const total = linhas ? linhas.length : 0;
      const modo = !arq ? 'nenhum' : !comHttp ? 'sem_rede' : lendo ? 'lendo' : erro ? 'erro' : janela.de !== null ? 'faixa' : 'cursor';
      painel.dataset.modo = modo;
      painel.dataset.de = janela.de ?? '';
      painel.dataset.ate = janela.ate ?? '';
      painel.dataset.cursor = janela.cursor ?? '';
      const linha = janela.cursor ?? '?';
      estadoEl.textContent = modo === 'nenhum' ? txt('ide.trilha_nenhum', 'Nenhum arquivo aberto no Helix.')
        : modo === 'sem_rede' ? txt('ide.minimapa_so_rede', 'O minimapa lê o arquivo pelo agente: só no navegador.')
        : modo === 'lendo' ? txt('ide.minimapa_lendo', 'Lendo o arquivo…')
        : modo === 'erro' ? txt('ide.minimapa_erro', 'Minimapa indisponível: {erro}', { erro })
        : modo === 'faixa' ? txt('ide.minimapa_faixa', 'Linhas {de}–{ate} de {total}; cursor na {linha}.', { de: janela.de, ate: janela.ate, total, linha })
        : txt('ide.minimapa_so_cursor', 'Só cursor (linha {linha} de {total}): a calha do Helix não mostra a numeração absoluta (line-number = "relative" ou calha desligada).', { linha, total });
      if (janela.modificado && linhas) estadoEl.textContent += ` ${txt('ide.minimapa_nao_salvo', 'Há alterações não salvas: o mapa mostra a versão em disco.')}`;
      estadoEl.toggleAttribute('data-aviso', modo === 'cursor' || modo === 'erro' || (janela.modificado && !!linhas));
      tela.setAttribute('aria-valuemin', '1');
      tela.setAttribute('aria-valuemax', String(Math.max(1, total)));
      tela.setAttribute('aria-valuenow', String(janela.cursor ?? 1));
      tela.setAttribute('aria-valuetext', txt('ide.minimapa_valor', 'Linha {linha} de {total}', { linha, total }));
    }
    // Pixels por linha do arquivo: ate 3 quando cabe, e o arquivo inteiro na altura quando nao.
    const passo = (H, n) => Math.min(3, H / Math.max(1, n));
    function pintar() {
      if (painel.hidden) return;
      const r = tela.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(r.width * dpr), h = Math.round(r.height * dpr);
      if (tela.width !== w || tela.height !== h) { tela.width = w; tela.height = h; }
      c2.setTransform(dpr, 0, 0, dpr, 0, 0);
      c2.clearRect(0, 0, r.width, r.height);
      if (!linhas) return;
      const cs = getComputedStyle(document.documentElement);
      const tinta = n => cs.getPropertyValue(n).trim();
      const p = passo(r.height, linhas.length);
      const alto = Math.max(0.5, p * 0.7);
      // 1 a 2 px por caractere: arquivo de linhas curtas usa a largura do painel, o de linhas
      // longas corta na borda (como o minimapa do VS Code).
      const textos = linhas.map(l => l.replace(/\t/g, '    '));
      const larg = Math.max(1, Math.min(2, r.width / Math.max(1, ...textos.slice(0, 5000).map(t => t.length))));
      c2.fillStyle = tinta('--texto-2');
      c2.globalAlpha = 0.65;
      textos.forEach((t, i) => {
        for (const m of t.matchAll(/\S+/g)) {
          const x = m.index * larg;
          if (x >= r.width) break;
          c2.fillRect(x, i * p, Math.min(m[0].length * larg, r.width - x), alto);
        }
      });
      c2.globalAlpha = 1;
      // A faixa visivel: contorno, nunca fundo cheio.
      if (janela.de !== null) {
        c2.strokeStyle = tinta('--acao-consultar');
        c2.lineWidth = 1.5;
        c2.strokeRect(0.75, (janela.de - 1) * p + 0.75, r.width - 1.5, Math.max(4, (janela.ate - janela.de + 1) * p) - 1.5);
      }
      if (janela.cursor !== null) {
        c2.fillStyle = tinta('--ambar');
        c2.fillRect(0, Math.min(r.height - 2, (janela.cursor - 1) * p), r.width, 2);
      }
    }
    const irPara = (n, focar) => {
      if (!linhas) return;
      const alvo = Math.max(1, Math.min(linhas.length, Math.round(n)));
      tela.dataset.goto = String(alvo);
      comandoHelix(`goto ${alvo}`, focar);
    };
    tela.addEventListener('click', e => {
      const r = tela.getBoundingClientRect();
      irPara(Math.floor((e.clientY - r.top) / passo(r.height, linhas?.length || 1)) + 1, true);
    });
    // Pelo teclado, o mesmo slider: setas uma linha, paginas uma janela, Home/End as pontas.
    tela.addEventListener('keydown', e => {
      if (!linhas) return;
      const atual = janela.cursor ?? 1;
      const pagina = janela.de !== null ? janela.ate - janela.de + 1 : 20;
      const n = { ArrowUp: atual - 1, ArrowDown: atual + 1, PageUp: atual - pagina, PageDown: atual + pagina, Home: 1, End: linhas.length }[e.key];
      if (n === undefined) return;
      e.preventDefault();
      irPara(n, false);
    });
    document.getElementById('minimapaReler')?.addEventListener('click', () => { if (arq && comHttp) buscar(arq); });
    new ResizeObserver(() => pintar()).observe(tela);
    return { arquivo, grade, estado, pintar };
  })();

  // LEITURA COM DOBRA: o Helix 25.07 nao dobra (0 comandos `fold`, `foldingRange` ausente do
  // cliente LSP dele -- docs/propostas/sp32-r5-r1-pesquisa.md) e o buffer dele e de OUTRO
  // processo. Este painel mostra o arquivo aberto no Helix lido EM DISCO pelo agente, com as
  // regioes do servidor de linguagem (/v1/ide/dobras: textDocument/foldingRange; sem servidor,
  // a reserva por chaves ou por indentacao -- e o painel diz qual). O texto entra por
  // textContent: e dado do arquivo, nunca HTML. Fechado, o painel nao pede nada ao agente.
  // O estado (quais regioes estao dobradas) e por arquivo, guardado no aparelho.
  const leitura = (() => {
    const painel = document.getElementById('ideLeitura');
    const codigo = document.getElementById('leituraCodigo');
    const estadoEl = document.getElementById('leituraEstado');
    if (!painel || !codigo || !estadoEl) return { arquivo() {}, grade() {}, estado() {} };
    const GUARDADO = 'phxclaw.dobras';
    // Arquivo gerado de dezenas de milhares de linhas nao e leitura de gente: o painel mostra
    // as primeiras e diz que cortou.
    const TETO_LINHAS = 20000;
    const TETO_ARQUIVOS = 50;
    let arq = null, dados = null, erro = null, lendo = false, pedido = 0, lido = null, modificado = false;
    let dobradas = new Set();
    let foco = null;
    const guardadas = () => { try { return JSON.parse(localStorage.getItem(GUARDADO) || '{}') || {}; } catch { return {}; } };
    // A pasta do projeto entra na chave: `src/main.rs` de dois projetos nao e o mesmo arquivo.
    const chaveDe = a => `${ativa?.cwd || ''}::${a}`;
    function guardar() {
      if (!arq) return;
      const g = guardadas();
      const k = chaveDe(arq);
      delete g[k];
      if (dobradas.size) g[k] = [...dobradas];
      // A ordem de insercao e a de uso: saem os mais antigos.
      const ks = Object.keys(g);
      for (const x of ks.slice(0, Math.max(0, ks.length - TETO_ARQUIVOS))) delete g[x];
      try { localStorage.setItem(GUARDADO, JSON.stringify(g)); } catch { /* sem armazenamento: vale ate recarregar */ }
    }
    const alcas = () => [...codigo.querySelectorAll('button.ll-dobra')].filter(b => !b.parentElement.hidden);
    async function buscar() {
      if (!arq || !comHttp || !painel.open) return;
      const meu = ++pedido;
      const a = arq;
      lendo = true; erro = null; estado();
      try {
        const r = await fetch(`./v1/ide/dobras?arquivo=${encodeURIComponent(a)}`, { headers: { Authorization: `Bearer ${token()}` }, cache: 'no-store' });
        const v = await r.json().catch(() => ({}));
        if (meu !== pedido) return;
        if (r.ok) { dados = v; erro = null; lido = a; } else { dados = null; erro = v.error || `HTTP ${r.status}`; }
      } catch {
        if (meu !== pedido) return;
        dados = null; erro = txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.');
      }
      lendo = false;
      // So as regioes guardadas que ainda existem neste texto.
      const inicios = new Set((dados?.regioes || []).map(r => r.inicio));
      dobradas = new Set((guardadas()[chaveDe(a)] || []).filter(n => inicios.has(n)));
      desenhar();
    }
    function desenhar() {
      codigo.replaceChildren();
      if (!dados) { estado(); return; }
      const linhas = String(dados.texto ?? '').replace(/\n$/, '').split('\n');
      const n = Math.min(linhas.length, TETO_LINHAS);
      const porInicio = new Map((dados.regioes || []).filter(r => r.inicio <= n).map(r => [r.inicio, r]));
      const frag = document.createDocumentFragment();
      for (let i = 1; i <= n; i++) {
        const div = el('div', 'll');
        div.dataset.n = String(i);
        const num = el('span', 'll-n', String(i));
        num.setAttribute('aria-hidden', 'true');
        const r = porInicio.get(i);
        let alca;
        if (r) {
          alca = el('button', 'll-dobra');
          alca.type = 'button';
          alca.dataset.inicio = String(i);
          alca.dataset.fim = String(Math.min(r.fim, n));
          alca.tabIndex = -1;
        } else {
          alca = el('span', 'll-dobra');
          alca.setAttribute('aria-hidden', 'true');
        }
        div.append(num, alca, el('code', 'll-t', linhas[i - 1]));
        if (r) div.append(el('span', 'll-oculto'));
        frag.append(div);
      }
      codigo.append(frag);
      aplicarDobras();
    }
    // Esconde as linhas de toda regiao dobrada (a de dentro continua dobrada quando a de fora
    // abre) e redesenha as alcas: seta, rotulo e quantas linhas a regiao esconde.
    function aplicarDobras() {
      const divs = codigo.children;
      const n = divs.length;
      const oculta = new Uint8Array(n + 2);
      for (const r of dados?.regioes || []) {
        if (!dobradas.has(r.inicio)) continue;
        for (let l = r.inicio + 1; l <= Math.min(r.fim, n); l++) oculta[l] = 1;
      }
      let ocultas = 0;
      for (let i = 1; i <= n; i++) {
        const div = divs[i - 1];
        div.hidden = !!oculta[i];
        if (oculta[i]) ocultas++;
        const b = div.children[1];
        if (b.tagName !== 'BUTTON') continue;
        const fechada = dobradas.has(i);
        const ate = Number(b.dataset.fim);
        b.textContent = fechada ? '▸' : '▾';
        b.setAttribute('aria-expanded', String(!fechada));
        b.setAttribute('aria-label', fechada
          ? txt('ide.leitura_desdobrar', 'Desdobrar as linhas {de}–{ate}', { de: i + 1, ate })
          : txt('ide.leitura_dobrar', 'Dobrar as linhas {de}–{ate}', { de: i + 1, ate }));
        div.classList.toggle('dobrada', fechada);
        div.lastChild.textContent = fechada ? txt('ide.leitura_ocultas_n', '⋯ {n} linhas', { n: ate - i }) : '';
      }
      // Um so ponto de parada no Tab (roving tabindex): a alca do foco, ou a primeira visivel.
      const vs = alcas();
      if (!vs.some(b => Number(b.dataset.inicio) === foco)) foco = vs[0] ? Number(vs[0].dataset.inicio) : null;
      for (const b of codigo.querySelectorAll('button.ll-dobra')) b.tabIndex = Number(b.dataset.inicio) === foco ? 0 : -1;
      painel.dataset.ocultas = String(ocultas);
      painel.dataset.dobradas = [...dobradas].sort((a, b) => a - b).join(',');
      estado();
    }
    function alternar(inicio, dobrar) {
      const d = dobrar ?? !dobradas.has(inicio);
      if (d) dobradas.add(inicio); else dobradas.delete(inicio);
      foco = inicio;
      guardar();
      aplicarDobras();
      codigo.querySelector(`button.ll-dobra[data-inicio="${inicio}"]`)?.focus();
    }
    function estado() {
      const total = dados ? dados.linhas : 0;
      const modo = !arq ? 'nenhum' : !comHttp ? 'sem_rede' : lendo ? 'lendo' : erro ? 'erro' : dados ? 'pronto' : 'fechado';
      painel.dataset.modo = modo;
      painel.dataset.origem = dados?.fonte || '';
      painel.dataset.arquivo = arq || '';
      painel.dataset.regioes = dados ? String((dados.regioes || []).length) : '';
      const fonte = { lsp: txt('ide.leitura_fonte_lsp', 'servidor de linguagem'), chaves: txt('ide.leitura_fonte_chaves', 'reserva por chaves'), indentacao: txt('ide.leitura_fonte_indentacao', 'reserva por indentação') }[dados?.fonte] || '';
      let t = modo === 'nenhum' ? txt('ide.trilha_nenhum', 'Nenhum arquivo aberto no Helix.')
        : modo === 'sem_rede' ? txt('ide.leitura_so_rede', 'A leitura com dobra lê o arquivo pelo agente: só no navegador.')
        : modo === 'lendo' ? txt('ide.minimapa_lendo', 'Lendo o arquivo…')
        : modo === 'erro' ? txt('ide.leitura_erro', 'Leitura indisponível: {erro}', { erro })
        : modo === 'fechado' ? txt('ide.leitura_abra', 'Abra o painel para ler {arquivo} com dobra.', { arquivo: arq })
        : txt('ide.leitura_estado', '{total} linhas, {regioes} regiões dobráveis ({fonte}); {ocultas} linhas dobradas.', { total, regioes: (dados.regioes || []).length, fonte, ocultas: painel.dataset.ocultas || 0 });
      if (dados && total > TETO_LINHAS) t += ` ${txt('ide.leitura_corte', 'Mostrando as primeiras {n} linhas.', { n: TETO_LINHAS })}`;
      if (dados?.aviso) t += ` ${txt('ide.leitura_aviso', 'O servidor de linguagem não respondeu: {aviso}', { aviso: dados.aviso })}`;
      if (dados && modificado) t += ` ${txt('ide.leitura_nao_salvo', 'Há alterações não salvas: a leitura mostra a versão em disco.')}`;
      estadoEl.textContent = t;
      estadoEl.classList.toggle('aviso', modo === 'erro' || !!dados?.aviso || (!!dados && modificado));
    }
    function arquivo(a) {
      painel.hidden = !(ativa?.programa === 'helix' && !ativa.convidado);
      if (a === arq) return;
      arq = a; dados = null; erro = null; lido = null; pedido++; dobradas = new Set(); foco = null;
      codigo.replaceChildren();
      if (a && painel.open) buscar(); else { lendo = false; estado(); }
    }
    // Salvou (o `[+]` sumiu da linha de estado): o disco mudou, a leitura rele.
    function grade(s) {
      if (s.convidado) return;
      const y = linhaDeEstado(s);
      if (y < 0) return;
      const mod = /\[\+\]/.test(textoDaLinha(s, y));
      if (modificado && !mod && arq && painel.open) buscar();
      if (mod !== modificado) { modificado = mod; estado(); }
    }
    painel.addEventListener('toggle', () => { if (painel.open && arq && lido !== arq) buscar(); else estado(); });
    codigo.addEventListener('click', e => {
      const b = e.target.closest('button.ll-dobra');
      if (b) alternar(Number(b.dataset.inicio));
    });
    // Teclado: as alcas sao uma lista (setas andam, Home/End vao as pontas), ← dobra e → desdobra
    // como numa arvore, Enter e Espaco sao o clique do proprio botao, e o atalho do VS Code
    // (Ctrl+Shift+[ e ]) vale pela tecla fisica, que nao muda com o layout.
    codigo.addEventListener('keydown', e => {
      const b = e.target.closest?.('button.ll-dobra');
      if (!b) return;
      const inicio = Number(b.dataset.inicio);
      const dobrar = e.key === 'ArrowLeft' || (e.ctrlKey && e.shiftKey && e.code === 'BracketLeft');
      const desdobrar = e.key === 'ArrowRight' || (e.ctrlKey && e.shiftKey && e.code === 'BracketRight');
      if (dobrar || desdobrar) { e.preventDefault(); alternar(inicio, dobrar); return; }
      const vs = alcas();
      const i = vs.indexOf(b);
      const alvo = { ArrowDown: vs[i + 1], ArrowUp: vs[i - 1], Home: vs[0], End: vs.at(-1) }[e.key];
      if (!(e.key in { ArrowDown: 1, ArrowUp: 1, Home: 1, End: 1 })) return;
      e.preventDefault();
      if (!alvo) return;
      foco = Number(alvo.dataset.inicio);
      for (const x of codigo.querySelectorAll('button.ll-dobra')) x.tabIndex = x === alvo ? 0 : -1;
      alvo.focus();
      alvo.scrollIntoView({ block: 'nearest' });
    });
    document.getElementById('leituraDobrarTudo')?.addEventListener('click', () => {
      if (!dados) return;
      dobradas = new Set((dados.regioes || []).map(r => r.inicio));
      guardar(); aplicarDobras();
    });
    document.getElementById('leituraDesdobrarTudo')?.addEventListener('click', () => {
      dobradas = new Set();
      guardar(); aplicarDobras();
    });
    document.getElementById('leituraReler')?.addEventListener('click', () => { lido = null; buscar(); });
    return { arquivo, grade, estado: () => { if (dados) aplicarDobras(); else estado(); } };
  })();

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
        s.convidado ? txt('ide.aba_convidado', 'Terminal compartilhado')
          : `${s.programa === 'helix' ? 'Helix' : 'bash'}${s.titulo ? ` — ${s.titulo}` : ''}`);
      b.dataset.id = s.id;
      b.onclick = () => ativar(s);
      return b;
    }));
    seloIde.textContent = sessoes.size ? String(sessoes.size) : '';
    vazio.hidden = !!ativa;
    botaoFechar.disabled = !ativa;
    if (!ativa) { status.textContent = txt('ide.nenhum', 'Nenhum terminal aberto.'); return; }
    if (ativa.convidado) { status.textContent = convidado.status(); return; }
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
  let transporte = invoke ? {
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
        else if (m.ev === 'compartilhamento') compartilhar.evento(m);
        else if (m.ev === 'grade') ouvinte?.(m);
        else if (m.ev === 'erro') { if (abrindo) { abrindo.falha(new Error(m.erro)); abrindo = null; } else encerrar(); }
      };
      ws.onclose = () => { if (abrindo) { abrindo.falha(Object.assign(new Error('sem rede'), { rede: true })); abrindo = null; } encerrar(); ws = null; };
    }
    return {
      tipo: 'ws',
      abrir: (programa, colunas, linhas, compartilhavel = false) => new Promise((ok, falha) => {
        if (programa !== 'helix') { falha(Object.assign(new Error('so helix'), { soHelix: true })); return; }
        ligar();
        abrindo = { ok, falha };
        const ir = () => mandar({ op: 'abrir', programa, colunas, linhas, compartilhavel });
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
    minimapa.estado();
    leitura.estado();
    compartilhar.desenhar();
    if (convidado.ativo()) convidado.mostrar();
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
  async function abrir(programa, compartilhavel = false) {
    if (!transporte) { avisar(semHost); return; }
    if (transporte.tipo === 'ws' && programa !== 'helix') { avisar(soHelix); return; }
    const t = tamanhoQueCabe() || { colunas: 100, linhas: 30 };
    try {
      const r = await transporte.abrir(programa, t.colunas, t.linhas, compartilhavel);
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
    // Convidado de leitura: nada sai daqui (o agente tambem nao teria por onde aplicar).
    if (!e.isTrusted || !ativa || ativa.encerrado || ativa.somenteLeitura || e.isComposing) return;
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
    if (document.activeElement !== canvas || !ativa || ativa.somenteLeitura || !e.isTrusted) return;
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

  const hora = iso => { try { return new Date(iso).toLocaleTimeString(document.documentElement.lang || 'pt-BR', { hour: '2-digit', minute: '2-digit' }); } catch { return iso; } };
  const selo = document.getElementById('ideCompartilhado');

  // COMPARTILHAR (o «Live Share» desta casa, decisao do dono: terminal compartilhado). O
  // anfitriao cria o convite pela rota do dono (/v1/ide/compartilhar), ve quem assiste pelo
  // proprio websocket do terminal (ev «compartilhamento») e revoga. So o Helix aberto SEM a
  // credencial da completacao por IA se compartilha (R1: um `:sh` mostraria o token a quem
  // assiste); o outro recusa e a tela oferece reabrir. O convite aparece UMA vez, no fragmento
  // do link (#convite=), que nao vai ao servidor nem aos logs.
  const compartilhar = (() => {
    const painel = document.getElementById('ideCompartilhar');
    if (!painel) return { evento() {}, desenhar() {} };
    const q = id => document.getElementById(id);
    const bCriar = q('compCriar'), bReabrir = q('compReabrir'), bRevogar = q('compRevogar');
    const estadoEl = q('compEstado'), caixa = q('compConvite'), link = q('compLink'), lista = q('compConvidados');
    let atual = null;     // ultimo ev «compartilhamento» do agente
    let aviso = null;     // () => texto, quando a ultima acao falhou
    let copiado = false;
    async function pedir(rota, corpo) {
      try {
        const r = await fetch(`./v1/ide/${rota}`, { method: 'POST', headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json' }, body: JSON.stringify(corpo), cache: 'no-store' });
        return { ok: r.ok, v: await r.json().catch(() => ({})), status: r.status };
      } catch { return { ok: false, rede: true, v: {} }; }
    }
    async function criar() {
      aviso = null; copiado = false;
      if (!ativa || ativa.encerrado || ativa.convidado || transporte?.tipo !== 'ws') {
        aviso = () => txt('ide.comp_sem_terminal', 'Abra o Helix pelo navegador antes de compartilhar.');
        desenhar(); return;
      }
      bCriar.disabled = true;
      const r = await pedir('compartilhar', { expira_em_s: Number(q('compExpira').value), max_convidados: Number(q('compMax').value), escrita: q('compEscrita').checked });
      bCriar.disabled = false;
      if (!r.ok) {
        bReabrir.hidden = !r.v.reabrir;
        aviso = r.rede ? () => txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.')
          : r.v.reabrir ? () => txt('ide.comp_precisa_reabrir', 'Este Helix tem a credencial da completação por IA no ambiente, e um :sh a mostraria a quem assiste. Reabra-o sem a credencial para compartilhar (o que não foi salvo se perde).')
            : () => txt('ide.comp_erro', 'Não compartilhou: {erro}', { erro: r.v.error || `HTTP ${r.status}` });
        desenhar(); return;
      }
      bReabrir.hidden = true;
      const base = location.pathname.replace(/[^/]*$/, '');
      link.value = `${location.origin}${base}#convite=${r.v.convite}`;
      caixa.hidden = false;
      desenhar();
    }
    async function revogar(convidado) {
      const r = await pedir('compartilhar/revogar', convidado ? { convidado } : {});
      aviso = r.ok ? null : () => txt('ide.comp_erro', 'Não compartilhou: {erro}', { erro: r.v.error || `HTTP ${r.status}` });
      desenhar();
    }
    function evento(m) {
      atual = m;
      if (!m.ativo) { caixa.hidden = true; link.value = ''; }
      desenhar();
    }
    function desenhar() {
      const ativo = !!atual?.ativo;
      const conv = atual?.convidados || [];
      painel.dataset.ativo = ativo ? '1' : '';
      painel.dataset.convidados = String(conv.length);
      bRevogar.disabled = !ativo;
      painel.hidden = !!convidado.ativo();
      let t = ativo
        ? txt('ide.comp_ativo', 'Compartilhado até {hora} — {n} de {max} convidados; {modo}.', { hora: hora(atual.expira_em), n: conv.length, max: atual.max_convidados, modo: atual.escrita ? txt('ide.comp_modo_escrita', 'escrita para convidado dono') : txt('ide.convidado_le', 'somente leitura') })
        : atual?.motivo ? txt('ide.comp_fim', 'Compartilhamento encerrado: {motivo}.', { motivo: atual.motivo })
          : txt('ide.comp_inativo', 'Não compartilhado.');
      if (copiado) t += ` ${txt('ide.comp_copiado', 'Convite copiado.')}`;
      if (aviso) t = aviso();
      estadoEl.textContent = t;
      estadoEl.classList.toggle('aviso', !!aviso);
      lista.replaceChildren(...conv.map(c => {
        const li = el('li');
        li.append(el('span', 'comp-quem', txt('ide.comp_convidado_item', 'Convidado {id} desde {hora} — {modo}', { id: c.id.slice(-6), hora: hora(c.desde), modo: c.escreve ? txt('ide.convidado_escreve', 'com escrita') : txt('ide.convidado_le', 'somente leitura') })));
        const b = el('button', 'acao exclui', txt('ide.comp_revogar_um', 'REVOGAR'));
        b.type = 'button';
        b.dataset.convidado = c.id;
        b.addEventListener('click', () => revogar(c.id));
        li.append(b);
        return li;
      }));
      // O indicador permanente, ao lado das abas: o anfitriao nunca esquece que compartilha.
      if (!convidado.ativo()) {
        selo.hidden = !ativo;
        selo.textContent = ativo ? txt('ide.selo_compartilhado', 'Compartilhado com {n} de {max}', { n: conv.length, max: atual.max_convidados }) : '';
      }
    }
    bCriar.addEventListener('click', criar);
    bRevogar.addEventListener('click', () => revogar(null));
    // Reabrir sem a credencial: fecha o Helix atual (o hx morre com o que nao foi salvo), abre
    // o compartilhavel e cria o convite.
    bReabrir.addEventListener('click', async () => {
      bReabrir.hidden = true;
      if (ativa && !ativa.convidado) await fechar();
      await abrir('helix', true);
      if (ativa?.compartilhavel) criar();
    });
    q('compCopiar').addEventListener('click', async () => {
      link.select();
      try { await navigator.clipboard.writeText(link.value); copiado = true; } catch { copiado = false; }
      desenhar();
    });
    return { evento, desenhar };
  })();

  // CONVIDADO: a aba que chegou por um convite (#convite=SESSAO.TOKEN, tirado da URL pelo
  // app.js antes de a navegacao reescrever o #) assiste ao terminal de outro pelo websocket do
  // convidado (/v1/ide/compartilhado). Desenha pelo MESMO aplicar/desenhar do anfitriao; de
  // leitura, o teclado nao manda nada. Escrever so se o convite permitir E a credencial da
  // API guardada neste aparelho provar o papel do terminal -- quem decide e o agente.
  const convidado = (() => {
    let info = null, fim = null, erro = null, ligado = false;
    const modo = () => info?.escreve ? txt('ide.convidado_escreve', 'com escrita') : txt('ide.convidado_le', 'somente leitura');
    function texto() {
      if (erro) return txt('ide.convidado_erro', 'Convite recusado: {erro}', { erro });
      if (fim) return txt('ide.convidado_fim', 'O compartilhamento terminou: {motivo}.', { motivo: fim });
      if (!info) return txt('ide.convidado_entrando', 'Entrando no terminal compartilhado…');
      return txt('ide.convidado_status', 'Convidado • {modo} • expira às {hora}', { modo: modo(), hora: hora(info.expira_em) });
    }
    function mostrar() {
      selo.hidden = false;
      selo.textContent = fim || erro ? texto() : txt('ide.selo_convidado', 'Você assiste a um terminal compartilhado — {modo}', { modo: modo() });
      selo.dataset.modo = erro ? 'erro' : fim ? 'fim' : info ? (info.escreve ? 'escreve' : 'le') : 'entrando';
      // Sem sessao ainda (antes da primeira grade), o status e escrito aqui; com ela, pelo
      // atualizarAbas de sempre, que pergunta o texto a este modulo.
      if (!ativa) status.textContent = texto();
      else atualizarAbas();
    }
    function entrar(convite) {
      const [sessao, segredo] = convite.split('.');
      ligado = true;
      document.body.dataset.convidado = '1';
      for (const b of [botaoHelix, botaoBash, botaoFechar]) b.disabled = true;
      let s = null;
      const base = location.pathname.replace(/[^/]*$/, '');
      const ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${base}v1/ide/compartilhado`);
      ws.onopen = () => {
        const o = { op: 'entrar', sessao, token: segredo };
        const cred = token();
        if (cred) o.credencial = cred;
        ws.send(JSON.stringify(o));
      };
      ws.onmessage = ev => {
        let m;
        try { m = JSON.parse(ev.data); } catch { return; }
        if (m.ev === 'entrou') { info = m; mostrar(); }
        else if (m.ev === 'grade') {
          if (!s) {
            s = { id: m.id, programa: 'helix', convidado: true, somenteLeitura: !info?.escreve, pid: '—', cwd: '', colunas: m.colunas, linhas: m.linhas, grade: [], fundo: m.fundo, frente: m.frente, cursor: null, titulo: '', encerrado: null };
            sessoes.set(s.id, s);
            ativar(s);
          }
          aplicar(s, m);
        } else if (m.ev === 'fim') { fim = m.motivo || '—'; if (s) { s.encerrado = { codigo: null }; desenhar(s, null); } mostrar(); }
        else if (m.ev === 'erro' && !info) { erro = m.erro; mostrar(); }
      };
      ws.onclose = () => { if (!fim && !erro) { fim = txt('ide.convidado_caiu', 'conexão encerrada'); } if (s) s.encerrado = { codigo: null }; mostrar(); };
      transporte = {
        tipo: 'convidado',
        abrir: () => Promise.reject(new Error(texto())),
        // So chega aqui o convidado com escrita (o teclado barra o de leitura antes).
        escrever: (id, o) => { if (ws.readyState === 1) ws.send(JSON.stringify(o.tecla ? { op: 'tecla', tecla: o.tecla } : o.colar !== undefined ? { op: 'colar', texto: o.colar } : { op: 'texto', texto: o.texto })); return Promise.resolve(true); },
        // O tamanho e a rolagem sao do anfitriao.
        redimensionar: () => Promise.resolve(),
        rolar: () => Promise.resolve(),
        fechar: () => { ws.close(); return Promise.resolve(); },
        ouvir: () => {},
      };
      mostrar();
      compartilhar.desenhar();
    }
    return { entrar, status: texto, ativo: () => ligado, mostrar };
  })();
  {
    let convite = null;
    try { convite = sessionStorage.getItem('phxclaw.convite'); sessionStorage.removeItem('phxclaw.convite'); } catch { /* sem armazenamento */ }
    if (convite && comHttp) convidado.entrar(convite);
    compartilhar.desenhar();
  }

  botaoHelix.addEventListener('click', () => abrir('helix'));
  botaoBash.addEventListener('click', () => abrir('bash'));
  botaoFechar.addEventListener('click', fechar);
  transporte?.ouvir(aoGrade);

  // Trocou o tema: as tintas se releem e a tela inteira do terminal se repinta.
  tema.aoTrocar(() => { lerTintas(); if (ativa) desenhar(ativa, null); minimapa.pintar(); });
  return { aoMostrar: () => { if (ativa) { redimensionar(); desenhar(ativa, null); canvas.focus(); } } };
})();
carregadores.ide = ide.aoMostrar;
// Quem ja esta na tela IDE quando o modulo chega nao pode esperar o proximo clique.
if (document.body.dataset.tela === 'ide') ide.aoMostrar();
};
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', iniciar);
else iniciar();
})();

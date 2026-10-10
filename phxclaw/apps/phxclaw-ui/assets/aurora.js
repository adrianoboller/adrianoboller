// Aurora: o assistente RETRATIL da vista Conversa (HUD), no molde que o dono desenhou a partir
// dos monitores do filme Avatar e da janela de voz do Homem de Ferro -- mas com nome e arte
// PROPRIOS. Camada ADICIONAL: nao substitui nada da Conversa (Kanban, realizacao, chat, mic,
// cartao). E por isso NAO toca no conversa.js -- observa o DOM que ele ja desenha (os baloes, o
// microfone, as colunas do Kanban) e narra o estado em UMA linha.
//
// Decisoes que valem saber:
// - **rotulo pela fabrica, DADO por textContent**: todo texto de tela sai de txt(chave, padrao)
//   (a mesma fabrica de idiomas do resto), e o que a Aurora repete da conversa entra como DADO,
//   nunca traduzido nem em maiuscula -- a mesma lei do «Blumenau».
// - **persona com nome PROPRIO**: Aurora, Iris e Vega sao inspiracao, nunca marca de terceiro.
//   A escolha (persona + voz masc/fem + ligar a voz) fica no aparelho (localStorage): e
//   conveniencia por visita, nao configuracao do agente -- a config persistida vem depois.
// - **voz desligada por padrao**: fala so depois de o usuario ligar e so apos um toque (a regra
//   do navegador), pela Web Speech do proprio navegador -- zero dependencia e zero rede.
// - **recolhe para o orb**: a janela encolhe para um ponto que volta ao toque; o estado fica no
//   aparelho. Movimento reduzido tira o giro e o pulso (no CSS).
(() => {
  'use strict';
  const LSK = 'phxclaw.aurora';
  // Reserva em memoria: numa janela privada, ou com o armazenamento bloqueado, o localStorage
  // LANCA. Sem esta reserva a persona e a voz ficariam presas no padrao (trocar nao faria nada).
  // Entao a escolha vale na visita mesmo sem armazenamento; so nao atravessa o recarregar.
  const mem = {};
  const LS = {
    get(k) { try { const v = localStorage.getItem(LSK + '.' + k); return v == null ? (k in mem ? mem[k] : null) : v; } catch { return k in mem ? mem[k] : null; } },
    set(k, v) { mem[k] = v; try { localStorage.setItem(LSK + '.' + k, v); } catch { /* sem armazenamento: vale so nesta visita, pela reserva em memoria */ } },
  };
  const t = (c, p, v) => (typeof txt === 'function' ? txt(c, p, v) : p);

  // Personas: id, tom (token de cor da marca que ja existe no tema) e o nome pela fabrica.
  const PERSONAS = [
    { id: 'aurora', cor: 'var(--acao-consultar)', rot: () => t('aurora.persona.aurora', 'Aurora') },
    { id: 'iris', cor: 'var(--acao-alterar)', rot: () => t('aurora.persona.iris', 'Íris') },
    { id: 'vega', cor: 'var(--acao-incluir)', rot: () => t('aurora.persona.vega', 'Vega') },
  ];
  function persona() {
    const id = LS.get('persona') || 'aurora';
    return PERSONAS.find(p => p.id === id) || PERSONAS[0];
  }

  // ---- DOM da Aurora (criado por JS: o index.html ganha so o <script>) ----
  const svgNS = 'http://www.w3.org/2000/svg';
  function svg(tag, attrs) {
    const e = document.createElementNS(svgNS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    return e;
  }
  function el(tag, cls, txto) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (txto != null) e.textContent = txto;
    return e;
  }
  function anelSvg(cls) {
    const s = svg('svg', { class: cls, viewBox: '0 0 44 44', 'aria-hidden': 'true' });
    s.append(
      svg('circle', { cx: 22, cy: 22, r: 20, class: 'au-anel-base' }),
      svg('circle', { cx: 22, cy: 22, r: 20, class: 'au-anel-1' }),
      svg('circle', { cx: 22, cy: 22, r: 14, class: 'au-anel-2' }),
      svg('circle', { cx: 22, cy: 22, r: 6.5, class: 'au-anel-nucleo' }),
    );
    return s;
  }

  const hud = el('aside', 'aurora-hud');
  hud.id = 'auroraHud';
  hud.setAttribute('aria-live', 'polite');
  hud.hidden = true;
  const cabeca = el('div', 'au-cabeca');
  const nome = el('b', 'au-nome');
  const estado = el('span', 'au-estado');
  const idBloco = el('div', 'au-id');
  idBloco.append(nome, estado);
  const btPersona = el('button', 'au-bt au-persona');
  btPersona.type = 'button';
  const btVoz = el('button', 'au-bt au-voz');
  btVoz.type = 'button';
  const btMin = el('button', 'au-bt au-min');
  btMin.type = 'button';
  btMin.append(svg('svg', { viewBox: '0 0 16 16', 'aria-hidden': 'true' }));
  btMin.querySelector('svg').append(svg('line', { x1: 3, y1: 11, x2: 13, y2: 11, class: 'au-traco' }));
  cabeca.append(anelSvg('au-anel'), idBloco, btPersona, btVoz, btMin);
  const leitura = el('p', 'au-leitura');
  const onda = el('div', 'au-onda');
  for (let i = 0; i < 7; i++) onda.append(el('i'));
  hud.append(cabeca, leitura, onda);

  const orb = el('button', 'aurora-orb');
  orb.id = 'auroraOrb';
  orb.type = 'button';
  orb.hidden = true;
  orb.append(anelSvg('au-anel'));

  document.body.append(hud, orb);

  // ---- Estado visivel ----
  function aplicarPersona() {
    const p = persona();
    hud.style.setProperty('--au', p.cor);
    orb.style.setProperty('--au', p.cor);
    nome.textContent = p.rot();
    btPersona.title = t('aurora.persona_trocar', 'Trocar a persona');
    btPersona.setAttribute('aria-label', btPersona.title);
    btPersona.textContent = '◈';
  }
  function aplicarVoz() {
    const on = LS.get('voz') === '1';
    btVoz.classList.toggle('on', on);
    btVoz.textContent = on ? '🔊' : '🔈';
    btVoz.title = on ? t('aurora.voz_desligar', 'Desligar a voz') : t('aurora.voz_ligar', 'Ligar a voz');
    btVoz.setAttribute('aria-label', btVoz.title);
  }
  let ouvindo = false;
  function aplicarEstado() {
    estado.textContent = ouvindo
      ? t('aurora.estado.ouvindo', 'ouvindo você…')
      : t('aurora.estado.vivo', 'assistente · ao vivo');
    onda.classList.toggle('on', ouvindo);
  }
  function recolher(sim) {
    hud.hidden = sim;
    orb.hidden = !sim;
    LS.set('recolhido', sim ? '1' : '0');
  }

  // A leitura repete o DADO do balao (textContent), nunca interpreta. Sem balao ainda: convite.
  function narrar(texto, dado) {
    leitura.textContent = '';
    if (dado) {
      // o texto do balao e DADO: entra cru, sem traducao e sem maiuscula
      leitura.textContent = texto;
    } else {
      leitura.textContent = texto;
    }
  }
  function falar(texto) {
    if (LS.get('voz') !== '1') return;
    try {
      const s = window.speechSynthesis;
      if (!s) return;
      s.cancel();
      const u = new SpeechSynthesisUtterance(texto);
      const alvo = (typeof idiomas === 'object' && idiomas.atual) === 'en' ? 'en' : 'pt';
      const vozes = s.getVoices().filter(v => v.lang && v.lang.toLowerCase().startsWith(alvo));
      const fem = LS.get('voz_genero') !== 'masc';
      // heuristica simples: nomes comuns de voz feminina; cai na primeira da lingua se nao achar
      const marca = fem ? /(female|fem|mulher|luciana|maria|helena|joana|google.*português|samantha)/i
        : /(male|masc|homem|daniel|felipe|ricardo|google.*masc)/i;
      u.voice = vozes.find(v => marca.test(v.name)) || vozes[0] || null;
      u.lang = u.voice ? u.voice.lang : (alvo === 'en' ? 'en-US' : 'pt-BR');
      s.speak(u);
    } catch { /* sem sintese de voz: a Aurora segue muda, a tela nao quebra */ }
  }

  // ---- Observar a Conversa que ja existe ----
  function historico() { return document.querySelector('.conversa-historico'); }
  function conversaVisivel() {
    const h = historico();
    return !!h && h.offsetParent !== null;
  }
  function ultimoBalao() {
    const h = historico();
    if (!h) return null;
    const baloes = h.querySelectorAll('.balao');
    return baloes.length ? baloes[baloes.length - 1] : null;
  }
  function lerConversa() {
    const b = ultimoBalao();
    if (!b) { narrar(t('aurora.inicio', 'Pronta. Diga ou digite o que precisa.'), false); return; }
    const corpo = b.querySelector('.balao-texto');
    const dado = corpo ? corpo.textContent.trim() : '';
    if (b.querySelector('.balao-trabalhando')) { estado.textContent = t('aurora.estado.trabalhando', 'trabalhando…'); return; }
    if (dado) { narrar(dado, true); falar(dado); }
  }

  // microfone: o conversa.js poe a classe 'gravando' no botao enquanto grava
  function lerMic() {
    const mic = document.querySelector('.conversa-mic');
    const grav = !!mic && mic.classList.contains('gravando');
    if (grav !== ouvindo) { ouvindo = grav; aplicarEstado(); }
  }

  let visivelAntes = null;
  function sincronizarVisibilidade() {
    const v = conversaVisivel();
    if (v === visivelAntes) return;
    visivelAntes = v;
    if (!v) { hud.hidden = true; orb.hidden = true; return; }
    const rec = LS.get('recolhido') === '1';
    recolher(rec);
    lerConversa();
  }

  // Observadores MIRADOS -- NUNCA o document.body. O callback muta o proprio HUD (textContent,
  // estilo, classe); observar o body inteiro veria essas mutacoes e se re-dispararia em laco
  // infinito, travando a pagina (defeito achado exercitando). O HUD nao esta dentro do historico
  // nem do botao do microfone, entao observar so esses dois e seguro.
  let alvoHist = null, alvoMic = null;
  const obsHist = new MutationObserver(() => lerConversa());
  const obsMic = new MutationObserver(() => lerMic());
  function religar() {
    const h = historico();
    if (h && h !== alvoHist) { obsHist.disconnect(); obsHist.observe(h, { childList: true, subtree: true, characterData: true }); alvoHist = h; lerConversa(); }
    const m = document.querySelector('.conversa-mic');
    if (m && m !== alvoMic) { obsMic.disconnect(); obsMic.observe(m, { attributes: true, attributeFilter: ['class'] }); alvoMic = m; lerMic(); }
  }

  // ---- Toque ----
  btMin.addEventListener('click', () => recolher(true));
  orb.addEventListener('click', () => { recolher(false); lerConversa(); });
  btPersona.addEventListener('click', () => {
    const i = PERSONAS.findIndex(p => p.id === persona().id);
    LS.set('persona', PERSONAS[(i + 1) % PERSONAS.length].id);
    aplicarPersona();
  });
  btVoz.addEventListener('click', () => {
    const on = LS.get('voz') === '1';
    LS.set('voz', on ? '0' : '1');
    aplicarVoz();
    if (!on) falar(t('aurora.inicio', 'Pronta. Diga ou digite o que precisa.'));
  });

  function redesenhar() { aplicarPersona(); aplicarVoz(); aplicarEstado(); lerConversa(); }
  if (typeof idiomas === 'object' && idiomas.aoTrocar) idiomas.aoTrocar(redesenhar);

  // Arranque
  function iniciar() {
    aplicarPersona(); aplicarVoz(); aplicarEstado();
    religar();
    sincronizarVisibilidade();
    // o intervalo religa os observadores quando a Conversa (re)desenha o historico/o mic, le o
    // estado do microfone e acompanha a visibilidade da vista -- nunca observa o body.
    setInterval(() => { religar(); lerMic(); sincronizarVisibilidade(); }, 500);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', iniciar);
  else iniciar();

  // Para a prova real exercitar sem depender do tempo
  window.aurora = { recolher, persona, _lerConversa: lerConversa, _hud: hud, _orb: orb };
})();

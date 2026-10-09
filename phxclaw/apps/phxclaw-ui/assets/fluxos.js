// Tela Fluxos: o editor em grafo dos fluxos do motor (fluxos.rs), desenhado em SVG a mao --
// sem biblioteca de grafo (o xyflow foi recusado na triagem R20 de 01/10/2026).
//
// O que a tela decide e o que ela NAO decide:
//   * o desenho e dela: camadas pela ordem topologica (a mais longa das dependencias),
//     ordem dentro da camada pelo baricentro dos vizinhos, e a posicao que a pessoa arrastou
//     guardada no campo `ui.posicoes` do proprio JSON -- campo que o `Fluxo` do motor nao tem,
//     entao o serde o ignora e a assinatura nunca o ve (prova: tests/desktop/ui_fluxos.mjs);
//   * o julgamento e do motor: validar, gravar e rodar passam pelas rotas /v1/fluxos/*
//     (fluxos_tela.rs), que chamam o `fluxos::ler`. O tipo que a tela mostra no no e so
//     rotulo; quem diz se o passo e valido e o veredito.
//
// Rotulo se traduz; DADO nunca. Nome do fluxo, id do passo, ferramenta, texto da tarefa, nome
// de porta e item de execucao entram por textContent, como gravados, sem text-transform.
// Toda chave de fabrica fica LITERAL na chamada do txt -- o conferidor le as chaves no fonte.
(() => {
  const $ = id => document.getElementById(id);
  const tela = $('tela-fluxos');
  if (!tela) return;
  const SVG = 'http://www.w3.org/2000/svg';
  const status = $('fluxosStatus');
  const lista = $('fluxosLista');
  const editor = $('fluxosEditor');
  const quadro = $('fluxosQuadro');
  const svg = $('fluxosSvg');
  const mini = $('fluxosMinimapa');
  const painel = $('fluxosPainel');
  const veredito = $('fluxosVeredito');
  const zoomRotulo = $('fluxosZoom');

  // Geometria do no (px do grafo, antes do zoom).
  const W = 204, H = 76, GX = 96, GY = 34, MARGEM = 32;
  // GX cabe o nome de porta mais longo («verdadeiro», ~72 px) no vao entre as camadas: com 60
  // ele ia para baixo da aba de estado e do no seguinte (visto na captura).
  // Folga a direita e embaixo, em px de TELA: o zoom e o minimapa ficam por cima do quadro, e
  // sem ela o no da beira ficava debaixo deles, sem rolagem que o tirasse dali (achado
  // exercitando: o clique no no caia no minimapa).
  const FOLGA_DIREITA = 210, FOLGA_BAIXO = 70;

  let fluxos = [];          // a lista da pasta
  let atual = null;         // { arquivo, revisao, obj, sujo, valido }
  let pos = new Map();      // id -> {x, y}
  let sel = null;           // { no: id } | { de, para, dep }
  let exec = null;          // { id, status, criada, por: Map(id -> resultado) }
  let k = 1;                // zoom
  let caixa = { x: 0, y: 0, w: 1, h: 1 };
  let sondagem = null;

  const api = (...a) => window.tarefas.api(...a);

  // ---------------------------------------------------------------- rotulos (fabrica)
  const TIPOS = {
    tarefa: () => txt('fluxos.tipo.tarefa', 'TAREFA'),
    ferramenta: () => txt('fluxos.tipo.ferramenta', 'FERRAMENTA'),
    skill: () => txt('fluxos.tipo.skill', 'SKILL'),
    mcp: () => txt('fluxos.tipo.mcp', 'MCP'),
    comando: () => txt('fluxos.tipo.comando', 'COMANDO'),
    se: () => txt('fluxos.tipo.se', 'CONDIÇÃO'),
    juntar: () => txt('fluxos.tipo.juntar', 'JUNTAR'),
    lote: () => txt('fluxos.tipo.lote', 'LOTE'),
    parar_com_erro: () => txt('fluxos.tipo.parar', 'PARAR COM ERRO'),
    esperar: () => txt('fluxos.tipo.esperar', 'ESPERAR'),
    politica: () => txt('fluxos.tipo.politica', 'POLÍTICA'),
  };
  // Os estados do `Resultado` do motor (fluxos.rs), um rotulo literal por estado. A COR sai
  // dos tokens de estado da marca (--ok, --aviso, --vermelho, --info, --texto-3) no CSS; a
  // forma do contorno (cheio, tracejado, pontilhado) e o rotulo no no dizem o mesmo sem cor.
  const ESTADOS = {
    ok: () => txt('fluxos.estado.ok', 'OK'),
    falhou: () => txt('fluxos.estado.falhou', 'FALHOU'),
    bloqueado: () => txt('fluxos.estado.bloqueado', 'BLOQUEADO'),
    continuou: () => txt('fluxos.estado.continuou', 'FALHOU E SEGUIU'),
    esperando: () => txt('fluxos.estado.esperando', 'ESPERANDO'),
    pulado: () => txt('fluxos.estado.pulado', 'PULADO'),
    nao_pedido: () => txt('fluxos.estado.nao_pedido', 'FORA DO CORTE'),
  };
  // Estado que esta tela nao conhece e DADO do motor: aparece cru.
  const estado = s => (ESTADOS[s] ? ESTADOS[s]() : s);
  const PORTAS = {
    verdadeiro: () => txt('fluxos.porta.verdadeiro', 'verdadeiro'),
    falso: () => txt('fluxos.porta.falso', 'falso'),
    erro: () => txt('fluxos.porta.erro', 'erro'),
    aprovado: () => txt('fluxos.porta.aprovado', 'aprovado'),
    reprovado: () => txt('fluxos.porta.reprovado', 'reprovado'),
  };

  // O tipo do passo, pela chave que ele declara (a mesma lista do `tipo` do motor). Passo
  // com zero ou duas chaves nao tem tipo: o no diz «?» e o veredito do motor diz por que.
  const CHAVES_DE_TIPO = Object.keys(TIPOS);
  function tipoDe(p) {
    const ts = CHAVES_DE_TIPO.filter(c => p[c] !== undefined && p[c] !== null);
    return ts.length === 1 ? ts[0] : null;
  }
  // O resumo do passo, em DADO: o que esta escrito nele.
  function resumoDe(p, t) {
    const v = t && p[t];
    switch (t) {
      case 'mcp': return `${v.servidor ?? ''}/${v.ferramenta ?? ''}`;
      case 'se': return [v.caminho, v.operador, typeof v.valor === 'string' ? v.valor : JSON.stringify(v.valor ?? null)].filter(x => x !== '' && x !== undefined).join(' ');
      case 'juntar': return [v.modo, v.chave].filter(Boolean).join(' ');
      case 'esperar': return Object.entries(v).map(([c, x]) => `${c} ${typeof x === 'string' ? x : JSON.stringify(x)}`).join(' ');
      // As regras ligadas, pelo nome do campo: o JSON inteiro nao cabe no no e saia por
      // baixo do nome da porta (visto na captura do assistente).
      case 'politica': return Object.keys(v || {}).join(' ');
      case null: case undefined: return '';
      default: return typeof v === 'string' ? v : JSON.stringify(v);
    }
  }
  const origemDe = d => String(d).split(':')[0];
  const portaDe = d => (String(d).includes(':') ? String(d).split(':').slice(1).join(':') : null);
  const portasDe = p => [...(p.se ? ['verdadeiro', 'falso'] : []), ...(p.politica ? ['aprovado', 'reprovado'] : []), ...(p.ao_errar === 'saida_de_erro' ? ['erro'] : [])];

  function el(tag, classe, texto) {
    const e = document.createElement(tag);
    if (classe) e.className = classe;
    if (texto !== undefined && texto !== null) e.textContent = String(texto);
    return e;
  }
  function sv(tag, attrs = {}, texto) {
    const e = document.createElementNS(SVG, tag);
    for (const [a, v] of Object.entries(attrs)) e.setAttribute(a, v);
    if (texto !== undefined) e.textContent = texto;
    return e;
  }
  // SVG nao corta texto com reticencias: corta-se aqui, e o texto inteiro vai no <title>.
  const cortar = (s, n) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
  function botao(classe, texto, acao) {
    const b = el('button', `acao ${classe}`, texto);
    b.type = 'button';
    b.addEventListener('click', acao);
    return b;
  }

  function aviso(texto, ehErro, comTentar = false) {
    if (status.textContent !== texto) status.textContent = texto;
    status.classList.toggle('aviso', !!ehErro);
    status.setAttribute('role', ehErro ? 'alert' : 'status');
    mostrarTentar('fluxos', comTentar);
  }
  function falhou(e) {
    if (e.status === 401) aviso(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true);
    else if (e.rede) aviso(txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), true, true);
    else aviso(txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }), true, true);
  }

  // ---------------------------------------------------------------- lista
  async function carregar() {
    const t = window.tarefas;
    if (!t?.comHttp) return aviso(txt('fluxos.sem_api', 'Os fluxos só existem com a tela servida pelo agente (phxclaw servir) ou pela ponte.'), true);
    if (!t.token()) return aviso(txt('tarefas.pede_token', 'Informe o token de acesso para ver as tarefas.'), true);
    aviso(txt('geral.lendo', 'Lendo…'));
    let v;
    try { v = await api('GET', 'fluxos'); } catch (e) { return falhou(e); }
    fluxos = [
      ...v.fluxos.map(f => ({ ...f, valido: true })),
      ...v.invalidos.map(f => ({ arquivo: f.arquivo, erro: f.erro, valido: false, etiquetas: [] })),
    ].sort((a, b) => a.arquivo.localeCompare(b.arquivo));
    desenharLista();
    aviso(fluxos.length
      ? txt('fluxos.resumo', '{n} fluxo(s) na pasta fluxos/ do agente; {invalidos} inválido(s).', { n: v.fluxos.length, invalidos: v.invalidos.length })
      : txt('fluxos.nenhum', 'Nenhum fluxo na pasta fluxos/ do agente. Grave um JSON lá (phxclaw fluxo importar) e recarregue.'));
  }

  function desenharLista() {
    lista.replaceChildren();
    const ul = el('ul');
    for (const f of fluxos) {
      const li = el('li');
      const b = el('button', 'fluxo-item');
      b.type = 'button';
      b.dataset.arquivo = f.arquivo;
      if (atual?.arquivo === f.arquivo) b.setAttribute('aria-current', 'true');
      b.append(el('b', 'fluxo-item-nome', f.nome ?? f.arquivo), el('code', 'fluxo-item-arq', f.arquivo));
      const meta = el('span', 'fluxo-item-meta');
      if (f.valido) meta.append(el('span', null, txt('fluxos.n_passos', '{n} passos', { n: f.passos })));
      else meta.append(el('span', 'fluxo-invalido', txt('fluxos.invalido', 'INVÁLIDO')));
      for (const e of f.etiquetas || []) meta.append(el('span', 'fluxo-etiqueta', e));
      b.append(meta);
      if (!f.valido && f.erro) b.append(el('small', 'fluxo-item-erro', f.erro));
      b.addEventListener('click', () => abrir(f.arquivo));
      li.append(b);
      ul.append(li);
    }
    lista.append(ul);
  }

  // ---------------------------------------------------------------- abrir
  async function abrir(arquivo) {
    if (atual?.sujo && atual.arquivo !== arquivo && !confirm(txt('fluxos.descartar', 'Há alterações não salvas neste fluxo. Descartar?'))) return;
    let v;
    try { v = await api('GET', `fluxos/arquivo?nome=${encodeURIComponent(arquivo)}`); } catch (e) { return falhou(e); }
    let obj;
    try { obj = JSON.parse(v.texto); } catch (e) {
      aviso(txt('fluxos.nao_json', 'O arquivo {arquivo} não é JSON: {erro}', { arquivo, erro: e.message }), true);
      return;
    }
    atual = { arquivo, revisao: v.revisao, obj, sujo: false, valido: v.valido };
    sel = null;
    exec = null;
    pos = posicoes();
    editor.hidden = false;
    desenharLista();
    $('fluxosNome').textContent = obj.nome ?? arquivo;
    $('fluxosArquivo').textContent = arquivo;
    mostrarVeredito(v);
    // Abre em 100%: o texto do no e de 12 e 13 px, e «ajustar» num quadro estreito o levava a
    // 4 px (medido: 35% a 1280). Ver o todo e o minimapa, ou o AJUSTAR, por escolha.
    k = 1;
    desenhar();
    quadro.scrollLeft = 0;
    quadro.scrollTop = 0;
    desenharPainel();
    aviso(txt('fluxos.aberto', 'Fluxo aberto: {arquivo}.', { arquivo }));
    ultimaExecucao();
    if (matchMedia('(max-width:640px)').matches) editor.scrollIntoView({ block: 'start' });
  }

  function mostrarVeredito(v) {
    veredito.replaceChildren();
    veredito.dataset.valido = v.valido ? '1' : '0';
    veredito.append(v.valido
      ? txt('fluxos.valido', 'Válido pelo motor: {n} passos.', { n: v.passos })
      : txt('fluxos.recusado', 'O motor recusou: {erro}', { erro: v.erro }));
    if (atual) atual.valido = !!v.valido;
    atualizarBotoes();
  }

  // ---------------------------------------------------------------- layout em camadas
  // Camada = o caminho mais longo desde uma fonte (Kahn); dentro da camada, quatro varridas
  // de baricentro (para baixo e para cima) diminuem os cruzamentos. Passo em ciclo (fluxo
  // invalido) vai para uma camada depois da ultima, em vez de sumir do desenho.
  function camadas(passos) {
    const ids = passos.map(p => p.id);
    const existe = new Set(ids);
    const pais = new Map(passos.map(p => [p.id, [...new Set((p.depende || []).map(origemDe))].filter(d => existe.has(d) && d !== p.id)]));
    const filhos = new Map(ids.map(i => [i, []]));
    for (const [i, ds] of pais) for (const d of ds) filhos.get(d).push(i);
    const grau = new Map(ids.map(i => [i, pais.get(i).length]));
    const nivel = new Map();
    const fila = ids.filter(i => grau.get(i) === 0);
    for (const i of fila) nivel.set(i, 0);
    const feitos = new Set();
    for (;;) {
      while (fila.length) {
        const i = fila.shift();
        feitos.add(i);
        for (const f of filhos.get(i)) {
          if (feitos.has(f)) continue;
          nivel.set(f, Math.max(nivel.get(f) ?? 0, nivel.get(i) + 1));
          grau.set(f, grau.get(f) - 1);
          if (grau.get(f) === 0) fila.push(f);
        }
      }
      // Ciclo (fluxo invalido): a fila parou com passos sobrando. O primeiro deles, na ordem
      // do arquivo, entra como se a ligacao que fecha o ciclo nao existisse -- o desenho
      // continua em camadas em vez de empilhar o ciclo inteiro numa coluna so.
      const resto = ids.find(i => !feitos.has(i) && !fila.includes(i));
      if (!resto) break;
      nivel.set(resto, nivel.get(resto) ?? 0);
      grau.set(resto, 0);
      fila.push(resto);
    }
    const porCamada = [];
    for (const i of ids) (porCamada[nivel.get(i)] ||= []).push(i);
    const ordem = new Map();
    const reindexar = () => porCamada.forEach(c => c?.forEach((i, n) => ordem.set(i, n)));
    reindexar();
    const bari = (i, viz) => {
      const v = viz.get(i).filter(x => ordem.has(x));
      return v.length ? v.reduce((s, x) => s + ordem.get(x), 0) / v.length : ordem.get(i);
    };
    for (let volta = 0; volta < 4; volta++) {
      const viz = volta % 2 === 0 ? pais : filhos;
      const camadasDaVolta = volta % 2 === 0 ? porCamada : [...porCamada].reverse();
      for (const c of camadasDaVolta) {
        if (!c) continue;
        const b = new Map(c.map(i => [i, bari(i, viz)]));
        c.sort((x, y) => b.get(x) - b.get(y));
        c.forEach((i, n) => ordem.set(i, n));
      }
    }
    return porCamada.map(c => c || []);
  }

  function posicoesAutomaticas() {
    const ps = atual.obj.passos || [];
    const erro = atual.obj.fluxo_de_erro;
    const noGrafo = ps.filter(p => p.id !== erro);
    const cs = camadas(noGrafo);
    const alto = Math.max(1, ...cs.map(c => c.length));
    const m = new Map();
    cs.forEach((c, n) => {
      const sobra = (alto - c.length) * (H + GY) / 2;
      c.forEach((id, i) => m.set(id, { x: n * (W + GX), y: sobra + i * (H + GY) }));
    });
    // O passo do fluxo de erro fica FORA do grafo (ninguem depende dele): embaixo, sozinho.
    if (erro && ps.some(p => p.id === erro)) m.set(erro, { x: 0, y: alto * (H + GY) + GY });
    return m;
  }

  // As salvas valem por passo; o passo sem posicao salva entra pela automatica.
  function posicoes() {
    const auto = posicoesAutomaticas();
    const salvas = atual.obj.ui?.posicoes || {};
    for (const [id, p] of Object.entries(salvas)) {
      if (auto.has(id) && Number.isFinite(p?.x) && Number.isFinite(p?.y)) auto.set(id, { x: p.x, y: p.y });
    }
    return auto;
  }

  // ---------------------------------------------------------------- desenho
  function limites(comFolga = true) {
    const xs = [...pos.values()];
    if (!xs.length) return { x: 0, y: 0, w: W, h: H };
    const x0 = Math.min(...xs.map(p => p.x)) - MARGEM, y0 = Math.min(...xs.map(p => p.y)) - MARGEM;
    const x1 = Math.max(...xs.map(p => p.x)) + W + (comFolga ? Math.max(MARGEM, FOLGA_DIREITA / k) : MARGEM);
    const y1 = Math.max(...xs.map(p => p.y)) + H + (comFolga ? Math.max(MARGEM, FOLGA_BAIXO / k) : MARGEM);
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  }

  function aplicarZoom() {
    caixa = limites();
    const f = svg.querySelector('.quadro-fundo');
    if (f) for (const [a, v] of Object.entries({ x: caixa.x - 2000, y: caixa.y - 2000, width: caixa.w + 4000, height: caixa.h + 4000 })) f.setAttribute(a, v);
    svg.setAttribute('viewBox', `${caixa.x} ${caixa.y} ${caixa.w} ${caixa.h}`);
    svg.setAttribute('width', Math.round(caixa.w * k));
    svg.setAttribute('height', Math.round(caixa.h * k));
    zoomRotulo.textContent = `${Math.round(k * 100)}%`;
    desenharMinimapa();
  }

  function caminhoDaAresta(a, b) {
    const x1 = a.x + W, y1 = a.y + H / 2, x2 = b.x, y2 = b.y + H / 2;
    const dx = Math.max(40, Math.abs(x2 - x1) / 2);
    // O meio da curva (t = 0,5): (P0 + 3P1 + 3P2 + P3) / 8 -- e onde vai o nome da porta.
    const mx = (x1 + 3 * (x1 + dx) + 3 * (x2 - dx) + x2) / 8, my = (y1 + y2) / 2;
    return { d: `M${x1} ${y1} C${x1 + dx} ${y1} ${x2 - dx} ${y2} ${x2} ${y2}`, x1, y1, x2, y2, mx, my };
  }

  function desenharArestas(g) {
    g.replaceChildren();
    for (const p of atual.obj.passos || []) {
      for (const dep of p.depende || []) {
        const de = origemDe(dep), a = pos.get(de), b = pos.get(p.id);
        if (!a || !b) continue;
        const c = caminhoDaAresta(a, b);
        const marcada = sel && sel.para === p.id && sel.dep === dep;
        const ea = exec?.por.get(de)?.estado;
        const grupo = sv('g', {
          class: `aresta${marcada ? ' selecionada' : ''}${ea ? ` est-${ea}` : ''}`, tabindex: '0', role: 'button',
          'data-de': de, 'data-para': p.id, 'data-dep': dep,
          'aria-label': txt('fluxos.aresta_rotulo', 'Ligação de {de} para {para}', { de: dep, para: p.id }),
        });
        grupo.append(
          sv('path', { class: 'aresta-toque', d: c.d }),
          sv('path', { class: 'aresta-linha', d: c.d }),
          sv('path', { class: 'aresta-ponta', d: `M${c.x2 - 9} ${c.y2 - 5} L${c.x2} ${c.y2} L${c.x2 - 9} ${c.y2 + 5} Z` }),
        );
        const porta = portaDe(dep);
        // No MEIO da ligacao, nao na saida: as duas portas de um «se» saem do mesmo ponto, e ali
        // «verdadeiro» e «falso» se escreviam um por cima do outro (achado na captura).
        if (porta) grupo.append(sv('text', { class: 'aresta-porta', x: c.mx, y: c.my - 7, 'text-anchor': 'middle' }, porta));
        grupo.addEventListener('click', ev => { ev.stopPropagation(); selecionar({ de, para: p.id, dep }); });
        grupo.addEventListener('keydown', ev => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); selecionar({ de, para: p.id, dep }); } });
        g.append(grupo);
      }
    }
  }

  function desenharNo(p) {
    const t = tipoDe(p);
    const r = exec?.por.get(p.id);
    const q = pos.get(p.id);
    const ehErro = atual.obj.fluxo_de_erro === p.id;
    const g = sv('g', {
      class: `no${sel?.no === p.id ? ' selecionado' : ''}${r ? ` est-${r.estado}` : ''}${ehErro ? ' no-erro' : ''}`,
      transform: `translate(${q.x} ${q.y})`, tabindex: '0', role: 'button', 'data-id': p.id,
      'aria-label': txt('fluxos.no_rotulo', 'Passo {id}, {tipo}', { id: p.id, tipo: t ? TIPOS[t]() : '?' }) + (r ? ` — ${estado(r.estado)}` : ''),
    });
    const resumo = resumoDe(p, t);
    g.append(
      sv('title', {}, [p.id, resumo].filter(Boolean).join('\n')),
      sv('rect', { class: 'no-caixa', width: W, height: H, rx: 8 }),
      sv('text', { class: 'no-tipo', x: 12, y: 20 }, ehErro ? txt('fluxos.tipo.fluxo_de_erro', 'FLUXO DE ERRO') : t ? TIPOS[t]() : '?'),
      sv('text', { class: 'no-id', x: 12, y: 42 }, cortar(String(p.id), 24)),
      sv('text', { class: 'no-resumo', x: 12, y: 62 }, cortar(resumo.replace(/\s+/g, ' '), 30)),
    );
    if (r) {
      const rotulo = estado(r.estado);
      // A aba do estado fica ACIMA da caixa (no vao entre as linhas): dentro, um rotulo longo
      // («FALHOU E SEGUIU») cobria o tipo do passo.
      const largura = 16 + rotulo.length * 7.6;
      g.append(
        sv('rect', { class: 'no-estado-caixa', x: W - largura, y: -23, width: largura, height: 21, rx: 4 }),
        sv('text', { class: 'no-estado', x: W - largura / 2, y: -8, 'text-anchor': 'middle' }, rotulo),
      );
    }
    ligarArrasto(g, p.id);
    g.addEventListener('keydown', ev => teclaNoNo(ev, p.id));
    return g;
  }

  function desenhar() {
    svg.replaceChildren();
    if (!atual) return;
    caixa = limites();
    svg.append(sv('rect', { class: 'quadro-fundo', x: caixa.x - 2000, y: caixa.y - 2000, width: caixa.w + 4000, height: caixa.h + 4000 }));
    const ga = sv('g', { class: 'arestas' });
    const gn = sv('g', { class: 'nos' });
    svg.append(ga, gn);
    desenharArestas(ga);
    for (const p of atual.obj.passos || []) if (pos.has(p.id)) gn.append(desenharNo(p));
    aplicarZoom();
    desenharLegenda();
  }

  // ---------------------------------------------------------------- arrastar, teclado, zoom
  // Arrastar move o no; soltar sem andar e um clique (seleciona). A posicao entra no
  // `ui.posicoes` so ao soltar -- e marca o fluxo como alterado.
  function ligarArrasto(g, id) {
    g.addEventListener('pointerdown', ev => {
      if (ev.button !== 0) return;
      ev.stopPropagation();
      const ini = { x: ev.clientX, y: ev.clientY, p: { ...pos.get(id) } };
      let andou = false;
      g.setPointerCapture(ev.pointerId);
      const mover = e => {
        const dx = (e.clientX - ini.x) / k, dy = (e.clientY - ini.y) / k;
        if (!andou && Math.hypot(dx, dy) < 3) return;
        andou = true;
        pos.set(id, { x: Math.round(ini.p.x + dx), y: Math.round(ini.p.y + dy) });
        g.setAttribute('transform', `translate(${pos.get(id).x} ${pos.get(id).y})`);
        desenharArestas(svg.querySelector('.arestas'));
      };
      const soltar = () => {
        g.removeEventListener('pointermove', mover);
        g.removeEventListener('pointerup', soltar);
        g.removeEventListener('pointercancel', soltar);
        if (andou) { guardarPosicao(id); desenhar(); selecionar({ no: id }); }
        else selecionar({ no: id });
      };
      g.addEventListener('pointermove', mover);
      g.addEventListener('pointerup', soltar);
      g.addEventListener('pointercancel', soltar);
    });
  }

  function guardarPosicao(id) {
    const o = atual.obj;
    o.ui = { ...(o.ui || {}), posicoes: { ...(o.ui?.posicoes || {}), [id]: { ...pos.get(id) } } };
    marcarSujo();
  }

  // Setas movem o no em foco (Shift: passo largo); Enter seleciona.
  function teclaNoNo(ev, id) {
    const passo = ev.shiftKey ? 40 : 8;
    const d = { ArrowLeft: [-passo, 0], ArrowRight: [passo, 0], ArrowUp: [0, -passo], ArrowDown: [0, passo] }[ev.key];
    if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); selecionar({ no: id }); return; }
    if (!d) return;
    ev.preventDefault();
    const p = pos.get(id);
    pos.set(id, { x: p.x + d[0], y: p.y + d[1] });
    guardarPosicao(id);
    desenhar();
    svg.querySelector(`.no[data-id="${CSS.escape(id)}"]`)?.focus();
  }

  function zoom(novo, cx, cy) {
    const r = quadro.getBoundingClientRect();
    cx ??= r.width / 2; cy ??= r.height / 2;
    const gx = (quadro.scrollLeft + cx) / k, gy = (quadro.scrollTop + cy) / k;
    k = Math.min(2, Math.max(0.25, novo));
    aplicarZoom();
    quadro.scrollLeft = gx * k - cx;
    quadro.scrollTop = gy * k - cy;
  }
  function ajustar() {
    const r = quadro.getBoundingClientRect();
    const c = limites(false);
    k = Math.min(1, Math.max(0.25, Math.min((r.width - 4) / c.w, (r.height - 4) / c.h)));
    aplicarZoom();
    quadro.scrollLeft = 0;
    quadro.scrollTop = 0;
  }
  quadro.addEventListener('wheel', ev => {
    if (!ev.ctrlKey && !ev.metaKey) return; // rolar sem Ctrl rola o quadro (nativo)
    ev.preventDefault();
    const r = quadro.getBoundingClientRect();
    zoom(k * (ev.deltaY < 0 ? 1.15 : 1 / 1.15), ev.clientX - r.left, ev.clientY - r.top);
  }, { passive: false });
  // Arrastar o fundo com o mouse rola o quadro; no toque a rolagem nativa ja faz isso.
  quadro.addEventListener('pointerdown', ev => {
    if (ev.pointerType !== 'mouse' || ev.button !== 0 || ev.target.closest('.no,.aresta')) return;
    const ini = { x: ev.clientX, y: ev.clientY, l: quadro.scrollLeft, t: quadro.scrollTop };
    let andou = false;
    const mover = e => {
      andou ||= Math.hypot(e.clientX - ini.x, e.clientY - ini.y) > 3;
      quadro.scrollLeft = ini.l - (e.clientX - ini.x);
      quadro.scrollTop = ini.t - (e.clientY - ini.y);
    };
    const soltar = () => {
      window.removeEventListener('pointermove', mover);
      window.removeEventListener('pointerup', soltar);
      quadro.classList.remove('arrastando');
      if (!andou && sel) selecionar(null);
    };
    quadro.classList.add('arrastando');
    window.addEventListener('pointermove', mover);
    window.addEventListener('pointerup', soltar);
  });
  quadro.addEventListener('scroll', () => desenharMinimapa());
  $('fluxosMais').addEventListener('click', () => zoom(k * 1.25));
  $('fluxosMenos').addEventListener('click', () => zoom(k / 1.25));
  $('fluxosAjustar').addEventListener('click', ajustar);

  // ---------------------------------------------------------------- minimapa
  function desenharMinimapa() {
    if (!atual || mini.hidden) return;
    mini.replaceChildren();
    mini.setAttribute('viewBox', `${caixa.x} ${caixa.y} ${caixa.w} ${caixa.h}`);
    for (const [id, q] of pos) {
      const r = exec?.por.get(id);
      mini.append(sv('rect', { class: `mini-no${r ? ` est-${r.estado}` : ''}`, x: q.x, y: q.y, width: W, height: H, rx: 10 }));
    }
    mini.append(sv('rect', {
      class: 'mini-vista', x: caixa.x + quadro.scrollLeft / k, y: caixa.y + quadro.scrollTop / k,
      width: Math.min(caixa.w, quadro.clientWidth / k), height: Math.min(caixa.h, quadro.clientHeight / k),
    }));
  }
  mini.addEventListener('click', ev => {
    const r = mini.getBoundingClientRect();
    // O viewBox mantem a proporcao (meet): a escala e a menor das duas.
    const s = Math.min(r.width / caixa.w, r.height / caixa.h);
    const ox = (r.width - caixa.w * s) / 2, oy = (r.height - caixa.h * s) / 2;
    const gx = (ev.clientX - r.left - ox) / s, gy = (ev.clientY - r.top - oy) / s;
    quadro.scrollLeft = gx * k - quadro.clientWidth / 2;
    quadro.scrollTop = gy * k - quadro.clientHeight / 2;
  });

  // ---------------------------------------------------------------- legenda
  function desenharLegenda() {
    const ul = $('fluxosLegenda');
    ul.replaceChildren();
    ul.hidden = !exec;
    if (!exec) return;
    for (const s of Object.keys(ESTADOS)) {
      const li = el('li', `est-${s}`);
      li.append(el('i', 'legenda-amostra'), el('span', null, estado(s)));
      ul.append(li);
    }
  }

  // ---------------------------------------------------------------- edicao
  function marcarSujo() {
    atual.sujo = true;
    atualizarBotoes();
  }
  function atualizarBotoes() {
    const temFluxo = !!atual;
    $('fluxosSalvar').disabled = !temFluxo || !atual.sujo;
    $('fluxosValidar').disabled = !temFluxo;
    $('fluxosNovoPasso').disabled = !temFluxo;
    $('fluxosReorganizar').disabled = !temFluxo;
    $('fluxosRodar').disabled = !temFluxo || atual.sujo || !atual.valido;
    $('fluxosSujo').hidden = !atual?.sujo;
  }
  const textoAtual = () => `${JSON.stringify(atual.obj, null, 2)}\n`;
  let relogioValidar = null;
  function revalidar() {
    clearTimeout(relogioValidar);
    relogioValidar = setTimeout(validar, 250);
  }
  async function validar() {
    if (!atual) return;
    try { mostrarVeredito(await api('POST', 'fluxos/validar', { texto: textoAtual() })); } catch (e) { falhou(e); }
  }
  async function salvar() {
    if (!atual) return;
    try {
      const v = await api('PUT', `fluxos/arquivo?nome=${encodeURIComponent(atual.arquivo)}`, { texto: textoAtual() }, { 'If-Match': atual.revisao });
      atual.revisao = v.revisao;
      atual.sujo = false;
      mostrarVeredito(v);
      aviso(txt('fluxos.salvo', 'Salvo em {arquivo}.', { arquivo: atual.arquivo }));
      const f = fluxos.find(x => x.arquivo === atual.arquivo);
      if (f) { f.nome = atual.obj.nome; f.passos = (atual.obj.passos || []).length; f.valido = true; desenharLista(); }
    } catch (e) {
      if (e.status === 409) aviso(txt('fluxos.conflito', 'O arquivo mudou no disco depois que foi aberto. Abra de novo para ver a versão atual (suas alterações se perdem).'), true);
      else if (e.status === 422) { mostrarVeredito(e.corpo || {}); aviso(txt('fluxos.nao_salvo', 'Não salvo: o motor recusou o fluxo.'), true); }
      else falhou(e);
    }
  }
  // Passo novo: o esqueleto do tipo escolhido, com os campos VAZIOS -- quem diz o que falta e
  // o veredito do motor, logo em seguida; um texto-exemplo seria dado inventado gravado no
  // fluxo. Entra no centro do que o quadro mostra, com a posicao ja salva em ui.posicoes.
  const ESQUELETOS = {
    tarefa: () => ({ tarefa: '' }), ferramenta: () => ({ ferramenta: '', args: {} }), skill: () => ({ skill: '' }),
    mcp: () => ({ mcp: { servidor: '', ferramenta: '' }, args: {} }), comando: () => ({ comando: '' }),
    se: () => ({ se: { caminho: '', operador: 'igual', valor: '' } }), juntar: () => ({ juntar: { modo: 'append' } }),
    lote: () => ({ lote: 10 }), parar_com_erro: () => ({ parar_com_erro: '' }), esperar: () => ({ esperar: { ms: 60000 } }),
    politica: () => ({ politica: {} }),
  };
  function idLivre(base) {
    const usados = new Set((atual.obj.passos || []).map(p => p.id));
    for (let n = 1; ; n++) if (!usados.has(`${base}_${n}`)) return `${base}_${n}`;
  }
  function criarPasso(tipo, id) {
    // Vazio ou repetido a tela recusa (o desenho e indexado pelo id); a FORMA do id e o motor
    // quem julga, no veredito que vem logo depois.
    if (!id || passoPorId(id)) return false;
    atual.obj.passos = [...(atual.obj.passos || []), { id, ...ESQUELETOS[tipo]() }];
    pos.set(id, { x: Math.round(caixa.x + (quadro.scrollLeft + quadro.clientWidth / 2) / k - W / 2), y: Math.round(caixa.y + (quadro.scrollTop + quadro.clientHeight / 2) / k - H / 2) });
    guardarPosicao(id);
    desenhar();
    selecionar({ no: id });
    revalidar();
    return true;
  }
  function painelDoNovo() {
    sel = null;
    desenhar();
    painel.replaceChildren();
    const cab = el('header', 'fp-cab');
    cab.append(el('span', 'eyebrow', txt('fluxos.novo_titulo', 'PASSO NOVO')));
    painel.append(cab);
    const b = bloco(txt('fluxos.novo_tipo', 'Tipo'));
    const tipo = el('select', 'fp-sel');
    tipo.setAttribute('aria-label', txt('fluxos.novo_tipo', 'Tipo'));
    for (const t of CHAVES_DE_TIPO) { const op = el('option', null, TIPOS[t]()); op.value = t; tipo.append(op); }
    const id = el('input', 'fp-campo');
    id.type = 'text';
    id.spellcheck = false;
    id.value = idLivre('passo');
    id.setAttribute('aria-label', txt('fluxos.novo_id', 'Id do passo'));
    const msg = el('p', 'fp-msg');
    msg.setAttribute('role', 'status');
    const rotuloId = el('label', 'fp-rotulo', txt('fluxos.novo_id', 'Id do passo'));
    rotuloId.append(id);
    b.append(tipo, rotuloId, msg, botao('inclui', txt('fluxos.criar_passo', '+ CRIAR PASSO'), () => {
      if (criarPasso(tipo.value, id.value.trim())) return;
      msg.textContent = txt('fluxos.id_invalido', 'Id vazio ou já usado por outro passo.');
      msg.dataset.erro = '1';
    }));
    tipo.focus();
  }
  // Excluir o passo tira tambem as ligacoes que saem dele e a posicao; referencias {{id}} no
  // texto de outros passos ficam -- e o veredito do motor as aponta.
  function excluirPasso(id) {
    const o = atual.obj;
    o.passos = o.passos.filter(p => p.id !== id);
    for (const p of o.passos) {
      if (!p.depende) continue;
      p.depende = p.depende.filter(d => origemDe(d) !== id);
      if (!p.depende.length) delete p.depende;
    }
    if (o.fluxo_de_erro === id) delete o.fluxo_de_erro;
    if (o.ui?.posicoes) delete o.ui.posicoes[id];
    pos.delete(id);
    sel = null;
    marcarSujo();
    desenhar();
    desenharPainel();
    revalidar();
  }

  function reorganizar() {
    if (!atual) return;
    if (atual.obj.ui) {
      const { posicoes: _, ...resto } = atual.obj.ui;
      if (Object.keys(resto).length) atual.obj.ui = resto; else delete atual.obj.ui;
    }
    pos = posicoes();
    marcarSujo();
    desenhar();
    ajustar();
  }

  function passoPorId(id) { return (atual.obj.passos || []).find(p => p.id === id); }

  function desligar(para, dep) {
    const p = passoPorId(para);
    p.depende = (p.depende || []).filter(d => d !== dep);
    if (!p.depende.length) delete p.depende;
    sel = { no: para };
    marcarSujo();
    desenhar();
    desenharPainel();
    revalidar();
  }
  function ligar(para, de, porta) {
    const p = passoPorId(para);
    const dep = porta ? `${de}:${porta}` : de;
    if ((p.depende || []).includes(dep)) return;
    p.depende = [...(p.depende || []), dep];
    marcarSujo();
    // As posicoes ficam: religar nao embaralha o desenho (um ciclo recem-criado empilhava
    // tudo numa coluna). REORGANIZAR refaz as camadas quando a pessoa quiser.
    desenhar();
    desenharPainel();
    revalidar();
  }

  function selecionar(s) {
    sel = s;
    for (const g of svg.querySelectorAll('.no')) g.classList.toggle('selecionado', !!s?.no && g.dataset.id === s.no);
    for (const g of svg.querySelectorAll('.aresta')) g.classList.toggle('selecionada', !!s?.dep && g.dataset.para === s.para && g.dataset.dep === s.dep);
    desenharPainel();
  }

  // ---------------------------------------------------------------- painel lateral
  function bloco(titulo) {
    const s = el('section', 'fp-bloco');
    s.append(el('h3', null, titulo));
    painel.append(s);
    return s;
  }
  function itensPre(itens) {
    const pre = el('pre', 'fp-itens');
    pre.textContent = JSON.stringify(itens ?? [], null, 2);
    return pre;
  }

  function desenharPainel() {
    painel.replaceChildren();
    if (!atual) return;
    if (sel?.dep) return painelDaAresta();
    const p = sel?.no && passoPorId(sel.no);
    if (!p) {
      painel.append(el('p', 'fp-dica', txt('fluxos.dica', 'Clique num passo para editar; arraste para mover. Ctrl + roda do mouse muda o zoom.')));
      return;
    }
    const t = tipoDe(p);
    const cab = el('header', 'fp-cab');
    cab.append(el('span', 'eyebrow', t ? TIPOS[t]() : '?'), el('h3', 'fp-id', p.id));
    painel.append(cab);

    // Dependencias: cada uma com DESLIGAR; e o LIGAR de uma origem nova (com a porta, quando
    // a origem tem portas nomeadas).
    const b1 = bloco(txt('fluxos.depende', 'Depende de'));
    const ul = el('ul', 'fp-deps');
    for (const dep of p.depende || []) {
      const li = el('li');
      li.append(el('code', null, dep), botao('exclui', txt('fluxos.desligar', 'DESLIGAR'), () => desligar(p.id, dep)));
      ul.append(li);
    }
    if (!(p.depende || []).length) ul.append(el('li', 'fp-vazio', txt('fluxos.sem_deps', 'Nenhuma: o passo começa o fluxo.')));
    b1.append(ul);
    // Origem ja ligada pela saida principal e sem portas nomeadas nao tem o que ligar de novo
    // (achado exercitando: o seletor abria nela, e LIGAR nao fazia nada, calado).
    const outros = (atual.obj.passos || []).map(x => x.id).filter(x => x !== p.id && x !== atual.obj.fluxo_de_erro
      && !((p.depende || []).includes(x) && !portasDe(passoPorId(x)).length));
    if (outros.length && p.id !== atual.obj.fluxo_de_erro) {
      const linha = el('div', 'fp-ligar');
      const selDe = el('select');
      selDe.setAttribute('aria-label', txt('fluxos.ligar_de', 'Ligar a partir de'));
      for (const o of outros) { const op = el('option', null, o); op.value = o; selDe.append(op); }
      const selPorta = el('select');
      selPorta.setAttribute('aria-label', txt('fluxos.porta', 'Porta'));
      const portas = () => {
        selPorta.replaceChildren();
        const sem = el('option', null, txt('fluxos.porta_principal', 'saída principal'));
        sem.value = '';
        selPorta.append(sem);
        for (const pt of portasDe(passoPorId(selDe.value) || {})) { const op = el('option', null, PORTAS[pt]()); op.value = pt; selPorta.append(op); }
        selPorta.hidden = selPorta.options.length === 1;
      };
      selDe.addEventListener('change', portas);
      portas();
      linha.append(selDe, selPorta, botao('inclui', txt('fluxos.ligar', '+ LIGAR'), () => ligar(p.id, selDe.value, selPorta.value)));
      b1.append(linha);
    }

    // Parametros: o passo em JSON, sem o id (identidade: renomear quebraria as
    // referencias) e sem o depende (editado acima). APLICAR troca o passo na memoria; quem
    // diz se ficou valido e o motor, logo em seguida.
    const b2 = bloco(txt('fluxos.parametros', 'Parâmetros'));
    const { id: _id, depende: _dep, ...resto } = p;
    const area = el('textarea', 'fp-json');
    area.spellcheck = false;
    area.rows = 9;
    area.value = JSON.stringify(resto, null, 2);
    area.setAttribute('aria-label', txt('fluxos.parametros_rotulo', 'Parâmetros do passo, em JSON'));
    const msg = el('p', 'fp-msg');
    msg.setAttribute('role', 'status');
    b2.append(area, msg, botao('altera', txt('fluxos.aplicar', 'APLICAR'), () => {
      let novo;
      try { novo = JSON.parse(area.value); } catch (e) {
        msg.textContent = txt('fluxos.json_invalido', 'JSON inválido: {erro}', { erro: e.message });
        msg.dataset.erro = '1';
        return;
      }
      if (!novo || typeof novo !== 'object' || Array.isArray(novo)) {
        msg.textContent = txt('fluxos.json_objeto', 'Os parâmetros têm de ser um objeto JSON.');
        msg.dataset.erro = '1';
        return;
      }
      const i = atual.obj.passos.indexOf(p);
      atual.obj.passos[i] = { id: p.id, ...(p.depende ? { depende: p.depende } : {}), ...novo };
      marcarSujo();
      desenhar();
      selecionar({ no: p.id });
      revalidar();
    }));

    // Executar ate aqui: o `--ate` do motor, sobre o arquivo SALVO.
    const b3 = bloco(txt('fluxos.executar_titulo', 'Executar'));
    const rodar = botao('inclui', txt('fluxos.rodar_ate', 'EXECUTAR ATÉ AQUI'), () => disparar(p.id));
    rodar.disabled = atual.sujo || !atual.valido || p.id === atual.obj.fluxo_de_erro;
    b3.append(rodar, botao('exclui', txt('fluxos.excluir_passo', 'EXCLUIR PASSO'), () => excluirPasso(p.id)));
    if (atual.sujo) b3.append(el('p', 'fp-dica', txt('fluxos.salve_antes', 'Salve antes de executar: o motor roda o arquivo gravado.')));

    // A ultima execucao deste passo: estado, entrada (a saida das dependencias, pela porta
    // declarada) e saida.
    const r = exec?.por.get(p.id);
    if (exec) {
      const b4 = bloco(txt('fluxos.ultima_passo', 'Na última execução'));
      if (!r) { b4.append(el('p', 'fp-vazio', txt('fluxos.nao_rodou', 'Este passo não aparece no relatório.'))); return; }
      const est = el('p', `fp-estado est-${r.estado}`);
      est.append(el('b', null, estado(r.estado)), el('span', null, ` · ${txt('fluxos.tentativas', '{n} tentativa(s)', { n: r.tentativas ?? 0 })}`));
      b4.append(est);
      const entrada = {};
      for (const dep of p.depende || []) {
        const o = exec.por.get(origemDe(dep));
        const porta = portaDe(dep);
        entrada[dep] = o ? (porta ? o.portas?.[porta] ?? [] : o.itens ?? []) : null;
      }
      b4.append(el('h4', null, txt('fluxos.entrada', 'Entrada')), itensPre(entrada));
      b4.append(el('h4', null, txt('fluxos.saida', 'Saída')), itensPre(r.itens));
      if (r.portas && Object.keys(r.portas).length) b4.append(el('h4', null, txt('fluxos.portas', 'Portas')), itensPre(r.portas));
      if (r.externo) b4.append(el('p', 'fp-dica', txt('fluxos.externo', 'Saída grande gravada à parte: {caminho} ({bytes} bytes).', { caminho: r.externo.caminho, bytes: r.externo.bytes })));
    }
  }

  function painelDaAresta() {
    const cab = el('header', 'fp-cab');
    cab.append(el('span', 'eyebrow', txt('fluxos.aresta', 'LIGAÇÃO')));
    const h = el('h3', 'fp-id');
    h.append(el('code', null, sel.dep), el('span', 'fp-seta', ' → '), el('code', null, sel.para));
    cab.append(h);
    painel.append(cab);
    const b = bloco(txt('fluxos.aresta_acoes', 'Ações'));
    b.append(botao('exclui', txt('fluxos.desligar_aresta', 'DESLIGAR LIGAÇÃO'), () => desligar(sel.para, sel.dep)));
  }

  // ---------------------------------------------------------------- execucao
  // A execucao e uma tarefa (objetivo «fluxo: NOME») e o relatorio mora no answer dela: a
  // ultima e a mais nova da lista de /v1/tasks com esse objetivo -- rotas que ja existiam.
  function lerRelatorio(t) {
    let rel = null;
    try { rel = t.answer ? JSON.parse(t.answer) : null; } catch { rel = null; }
    const por = new Map((rel?.passos || []).map(r => [r.id, r]));
    return { id: t.id, status: t.status, criada: t.created_at, ate: rel?.ate ?? null, por };
  }
  function dizerExecucao() {
    const e = $('fluxosExecucao');
    e.replaceChildren();
    if (!exec) { e.append(txt('fluxos.sem_execucao', 'Nenhuma execução deste fluxo ainda.')); return; }
    const st = window.tarefas.estado(exec.status);
    e.append(exec.ate
      ? txt('fluxos.execucao_ate', 'Última execução {id} ({estado}, {quando}), até o passo {ate}.', { id: exec.id, estado: st, quando: dataHora(exec.criada), ate: exec.ate })
      : txt('fluxos.execucao', 'Última execução {id} ({estado}, {quando}).', { id: exec.id, estado: st, quando: dataHora(exec.criada) }));
  }
  async function ultimaExecucao() {
    if (!atual) return;
    const alvo = `fluxo: ${atual.obj.nome}`;
    try {
      const ts = (await api('GET', 'tasks')).filter(t => t.objective === alvo);
      ts.sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)));
      exec = ts.length ? lerRelatorio(await api('GET', `tasks/${ts[0].id}`)) : null;
    } catch (e) { exec = null; falhou(e); }
    dizerExecucao();
    desenhar();
    desenharPainel();
  }
  async function disparar(ate) {
    if (!atual || atual.sujo) return;
    let id;
    try { ({ id } = await api('POST', 'fluxos/rodar', { nome: atual.arquivo, ...(ate ? { ate } : {}) })); } catch (e) { return falhou(e); }
    aviso(ate ? txt('fluxos.disparado_ate', 'Execução {id} disparada até o passo {ate}.', { id, ate }) : txt('fluxos.disparado', 'Execução {id} disparada.', { id }));
    clearTimeout(sondagem);
    const sondar = async () => {
      let t;
      try { t = await api('GET', `tasks/${id}`); } catch (e) { return falhou(e); }
      exec = lerRelatorio(t);
      dizerExecucao();
      desenhar();
      desenharPainel();
      if (['pending', 'running'].includes(t.status)) sondagem = setTimeout(sondar, 800);
      else aviso(txt('fluxos.terminou', 'Execução {id}: {estado}.', { id, estado: window.tarefas.estado(t.status) }), t.status === 'failed');
    };
    sondar();
  }

  $('fluxosValidar').addEventListener('click', validar);
  $('fluxosNovoPasso').addEventListener('click', () => { if (atual) painelDoNovo(); });
  $('fluxosSalvar').addEventListener('click', salvar);
  $('fluxosRodar').addEventListener('click', () => disparar(null));
  $('fluxosReorganizar').addEventListener('click', reorganizar);
  $('fluxosRecarregar').addEventListener('click', carregar);
  // O assistente: a descricao vai ao motor (fluxo_assistente.rs), que grava um RASCUNHO e
  // devolve o nome relativo; a lista recarrega e o rascunho abre para revisao.
  $('fluxosAssistente').addEventListener('submit', async ev => {
    ev.preventDefault();
    const campo = $('fluxosDescricao');
    const descricao = campo.value.trim();
    if (!descricao) { campo.focus(); return; }
    const botaoCriar = $('fluxosCriar');
    botaoCriar.disabled = true;
    aviso(txt('fluxos.assistente_pensando', 'O assistente está montando o fluxo…'));
    let v;
    try { v = await api('POST', 'fluxos/assistente', { descricao }); } catch (e) {
      botaoCriar.disabled = false;
      if (e.status === 422) return aviso(txt('fluxos.assistente_falhou', 'O assistente não chegou a um fluxo válido — {erro}', { erro: e.message }), true);
      return falhou(e);
    }
    botaoCriar.disabled = false;
    campo.value = '';
    await carregar();
    await abrir(v.arquivo);
    aviso(txt('fluxos.assistente_criado', 'Rascunho {arquivo} criado em {n} tentativa(s); revise antes de publicar.', { arquivo: v.arquivo, n: v.tentativas }));
  });
  $('fluxosUltima').addEventListener('click', ultimaExecucao);
  // O minimapa so existe onde cabe (o CSS o esconde no celular); medir aqui evita desenhar
  // o que ninguem ve.
  const medirMini = () => { mini.hidden = getComputedStyle(mini).display === 'none'; desenharMinimapa(); };
  addEventListener('resize', medirMini);
  idiomas.aoTrocar(() => { if (atual) { desenhar(); desenharPainel(); dizerExecucao(); } if (fluxos.length) desenharLista(); });

  carregadores.fluxos = () => { medirMini(); if (!fluxos.length) carregar(); };
  tentar.fluxos = carregar;
  window.fluxosTela = { get atual() { return atual; }, get pos() { return pos; }, get k() { return k; }, abrir };
  atualizarBotoes();
  if (document.body.dataset.tela === 'fluxos') carregadores.fluxos();
})();

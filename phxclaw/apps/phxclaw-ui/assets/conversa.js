// Conversa: o espaco de trabalho de UM projeto, no modelo que o dono desenhou -- projeto ->
// pedido -> cartao. Tres pecas costuradas numa tela so:
//
//   1. PORTAO DE PROJETO: toda solicitacao nasce dentro de um projeto. Sem projeto ativo, a
//      conversa e o envio ficam bloqueados; o seletor do topo troca ou cria. O projeto ativo
//      fica no aparelho (localStorage) e viaja no cabecalho X-PhxClaw-Projeto -- o MESMO campo
//      que o portao do servidor le (rbac.rs), e que corta a lista de tarefas por projeto.
//   2. KANBAN FIXO E RETRATIL (zona de cima): o quadro do projeto ativo, cartoes COMPACTOS
//      (resumo + estado + data da ultima mudanca), em quatro colunas mapeadas do TaskStatus:
//      Backlog=Pending; Fazendo=Running+AwaitingInput+AwaitingApproval; Feito=Completed;
//      Parado=Failed+Cancelled+BudgetExceeded. Recolhido, vira so a faixa de contadores e a
//      area de baixo ganha a tela toda (foco na tarefa).
//   3. REALIZACAO (zona de baixo): o detalhe do cartao CLICADO -- solicitacao, linha do tempo,
//      ajustes, artefatos, agentes, inicio/fim, progresso e a pizza da atividade -- mais os
//      baloes da conversa, e a barra com o campo, ENVIAR e o microfone.
//
// Reuso: window.tarefas (o cliente de /v1, o token e o formatador de estado) e, do app.js,
// mostrarTela, carregadores e dataHora. Rotulo sai da fabrica (txt, chave literal de aspas
// simples para o conferidor a ver); o conteudo da conversa (pedido, resposta, ajuste, nome de
// arquivo, nome de ferramenta) e DADO: entra por textContent, nunca traduzido nem em maiuscula.
(() => {
  const $ = id => document.getElementById(id);
  const corpoEl = $('conversaCorpo');
  const gateEl = $('conversaGate');
  if (!corpoEl || !gateEl) return;
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';

  const LS = {
    get(k) { try { return localStorage.getItem(k); } catch { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch { /* sem armazenamento: so nesta visita */ } },
  };

  function cEl(tag, classe, texto) {
    const e = document.createElement(tag);
    if (classe) e.className = classe;
    if (texto !== undefined && texto !== null) e.textContent = String(texto);
    return e;
  }

  /* ===================== PROJETO ===================== */
  const K_PROJS = 'phxclaw.projetos';
  const K_ATIVO = 'phxclaw.projeto';
  const K_KANBAN = 'phxclaw.kanban.recolhido';
  const K_MAPA = 'phxclaw.tarefa.projeto';

  function projetos() { try { return JSON.parse(LS.get(K_PROJS) || '[]'); } catch { return []; } }
  function salvarProjetos(l) { LS.set(K_PROJS, JSON.stringify(l)); }
  function ativoId() { return LS.get(K_ATIVO) || ''; }
  function projetoAtivo() { const id = ativoId(); return projetos().find(p => p.id === id) || null; }
  function definirAtivo(id) { LS.set(K_ATIVO, id || ''); }
  // id a partir do nome (dado): minusculas, sem acento, so letra/numero/hifen. Nome repetido
  // reusa o projeto (nao cria dois com o mesmo id).
  function idDe(nome) {
    const base = String(nome).normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
    return base || `p${Date.now().toString(36)}`;
  }
  function criarProjeto(nome) {
    const n = String(nome).trim();
    if (!n) return null;
    const id = idDe(n);
    const l = projetos();
    if (!l.some(p => p.id === id)) { l.push({ id, nome: n }); salvarProjetos(l); }
    return l.find(p => p.id === id);
  }
  // Associacao local tarefa->projeto: so vale no Bearer unico, em que o servidor nao carimba
  // o projeto na tarefa; com RBAC, o `projeto` vem na propria tarefa e a lista ja chega cortada.
  function mapaTarefa() { try { return JSON.parse(LS.get(K_MAPA) || '{}'); } catch { return {}; } }
  function marcarTarefa(id, proj) { const m = mapaTarefa(); m[id] = proj; LS.set(K_MAPA, JSON.stringify(m)); }
  function projetoDaTarefa(t) { return t.projeto || mapaTarefa()[t.id] || null; }

  // Semente de «projeto existente» do backend: a raiz do projeto confiado (agente.projeto) ou,
  // na falta, a pasta do agente (agente.pasta). Best-effort e so UMA vez; sem API fica vazio e
  // o portao pede para criar. Nome = o ultimo segmento do caminho (dado); id derivado.
  async function semear() {
    if (!comHttp || !window.tarefas || !window.tarefas.token()) return;
    try {
      const v = await window.tarefas.api('GET', 'config');
      const ch = (v.chaves || []);
      const raiz = (ch.find(c => c.chave === 'agente.projeto') || {}).valor || (ch.find(c => c.chave === 'agente.pasta') || {}).valor;
      if (typeof raiz === 'string' && raiz) {
        const nome = raiz.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || raiz;
        const p = criarProjeto(nome);
        if (p && !ativoId()) { definirAtivo(p.id); aoTrocarProjeto(); }
      }
    } catch { /* sem API ou sem permissao: o portao pede para criar */ }
  }

  /* ===================== SELETOR E DIALOGO ===================== */
  const seletor = $('projetoSeletor');
  const seletorNome = $('projetoSeletorNome');
  const dlg = $('projetoDialogo');

  function renderSeletor() {
    const p = projetoAtivo();
    seletorNome.textContent = p ? p.nome : txt('projeto.nenhum', 'Nenhum projeto');
    seletor.classList.toggle('sem-projeto', !p);
  }

  function renderLista() {
    const l = projetos();
    const ul = $('projetoLista');
    ul.replaceChildren();
    if (!l.length) { ul.append(cEl('li', 'vazio', txt('projeto.sem_projetos', 'Nenhum projeto ainda. Crie o primeiro.'))); return; }
    const ativo = ativoId();
    for (const p of l) {
      const li = cEl('li');
      const b = cEl('button', 'proj-item');
      b.type = 'button';
      b.dataset.id = p.id;
      b.append(cEl('span', 'proj-item-nome', p.nome)); // nome e DADO
      if (p.id === ativo) { b.append(cEl('em', 'proj-item-ativo', txt('projeto.ativo_marca', 'ativo'))); b.setAttribute('aria-current', 'true'); }
      b.addEventListener('click', () => { definirAtivo(p.id); fecharDialogo(); aoTrocarProjeto(); });
      li.append(b);
      ul.append(li);
    }
  }

  // O campo de criar so NASCE ao clicar «criar novo»: no repouso o dialogo nao tem <input>, e
  // por isso a sonda do topo (G6) nao o confunde com a paleta de busca.
  function abrirCriar() {
    const area = $('projetoCriarArea');
    if (area.querySelector('input')) { area.querySelector('input').focus(); return; }
    area.replaceChildren();
    const campo = cEl('input', 'proj-nome');
    campo.type = 'text';
    campo.autocomplete = 'off';
    campo.placeholder = txt('projeto.nome_ph', 'Nome do projeto');
    campo.setAttribute('aria-label', txt('projeto.nome_ph', 'Nome do projeto'));
    const criar = cEl('button', 'acao inclui', txt('projeto.criar', 'CRIAR'));
    criar.type = 'button';
    const cancelar = cEl('button', 'acao consulta', txt('projeto.cancelar_criar', 'CANCELAR'));
    cancelar.type = 'button';
    const confirmar = () => {
      const p = criarProjeto(campo.value);
      if (!p) { campo.focus(); return; }
      definirAtivo(p.id);
      fecharDialogo();
      aoTrocarProjeto();
    };
    criar.addEventListener('click', confirmar);
    cancelar.addEventListener('click', () => { area.replaceChildren(); });
    campo.addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); confirmar(); } });
    const linha = cEl('div', 'proj-criar-linha');
    linha.append(campo, criar, cancelar);
    area.append(linha);
    campo.focus();
  }

  let devolverFoco = true;
  function abrirDialogo() {
    if (dlg.open) return;
    $('projetoCriarArea').replaceChildren();
    renderLista();
    devolverFoco = true;
    dlg.showModal();
  }
  function fecharDialogo() { if (dlg.open) dlg.close(); }

  seletor.addEventListener('click', abrirDialogo);
  $('projetoFechar').addEventListener('click', () => { fecharDialogo(); });
  $('projetoCriarNovo').addEventListener('click', abrirCriar);
  dlg.addEventListener('click', e => { if (e.target === dlg) dlg.close(); });
  dlg.addEventListener('close', () => { if (devolverFoco) seletor.focus(); devolverFoco = true; });
  $('conversaGateBotao').addEventListener('click', () => { devolverFoco = false; abrirDialogo(); });

  function aoTrocarProjeto() {
    renderSeletor();
    selecionada = null;
    assinaturaDet = '';
    atualizar();
  }

  /* ===================== API COM PROJETO ===================== */
  function cabProjeto() { const p = projetoAtivo(); return p ? { 'X-PhxClaw-Projeto': p.id } : {}; }
  function apiTarefas(metodo, caminho, corpoReq) { return window.tarefas.api(metodo, caminho, corpoReq, cabProjeto()); }

  /* ===================== KANBAN ===================== */
  const COLUNAS = [
    { chave: 'backlog', estados: ['pending'], rot: () => txt('kanban.col.backlog', 'Backlog') },
    { chave: 'fazendo', estados: ['running', 'awaiting_input', 'awaiting_approval'], rot: () => txt('kanban.col.fazendo', 'Fazendo') },
    { chave: 'feito', estados: ['completed'], rot: () => txt('kanban.col.feito', 'Feito') },
    { chave: 'parado', estados: ['failed', 'cancelled', 'budget_exceeded'], rot: () => txt('kanban.col.parado', 'Parado') },
  ];
  const colunaDe = status => (COLUNAS.find(c => c.estados.includes(status)) || COLUNAS[0]).chave;

  let lista = [];
  let selecionada = null;
  let relogio = null;
  let assinaturaDet = '';

  function avisoKanban(texto) {
    $('kanbanQuadro').replaceChildren(Object.assign(cEl('p', 'vazio aviso', texto), { role: 'status' }));
    $('kanbanContadores').replaceChildren();
  }

  function ultimaMudanca(t) {
    const h = t.historico || [];
    return (h.length && h[h.length - 1].em) || t.updated_at || t.created_at;
  }

  function cartaoCompacto(t) {
    const b = cEl('button', 'kanban-card');
    b.type = 'button';
    b.dataset.id = t.id;
    if (t.id === selecionada) b.classList.add('selecionada');
    b.setAttribute('aria-pressed', String(t.id === selecionada));
    b.append(cEl('span', 'kanban-card-obj', t.objective)); // objetivo e DADO (resumo cortado por CSS)
    const meta = cEl('span', 'kanban-card-meta');
    meta.append(cEl('span', `tarefa-estado estado-${t.status}`, window.tarefas.estado(t.status)));
    const quando = ultimaMudanca(t);
    const time = cEl('time', 'kanban-card-data', dataHora(quando));
    time.dateTime = quando;
    meta.append(time);
    b.append(meta);
    b.addEventListener('click', () => selecionar(t.id));
    return b;
  }

  function desenharQuadro(ts) {
    const quadro = $('kanbanQuadro');
    quadro.replaceChildren();
    const porCol = Object.fromEntries(COLUNAS.map(c => [c.chave, []]));
    for (const t of ts) porCol[colunaDe(t.status)].push(t);
    for (const c of COLUNAS) {
      const col = cEl('div', `kanban-col kanban-col-${c.chave}`);
      col.dataset.col = c.chave;
      const cab = cEl('header', 'kanban-col-head');
      cab.append(cEl('b', 'kanban-col-nome', c.rot()), cEl('span', 'kanban-col-conta', porCol[c.chave].length));
      col.append(cab);
      const corpo = cEl('div', 'kanban-col-corpo');
      if (!porCol[c.chave].length) corpo.append(cEl('p', 'kanban-col-vazio', txt('kanban.vazio', 'Sem cartões.')));
      else for (const t of porCol[c.chave]) corpo.append(cartaoCompacto(t));
      col.append(corpo);
      quadro.append(col);
    }
    // Faixa de contadores (so visivel recolhido): rotulo da fabrica + numero (dado).
    const cont = $('kanbanContadores');
    cont.replaceChildren();
    COLUNAS.forEach((c, i) => {
      if (i) cont.append(cEl('i', 'sep', '·'));
      const s = cEl('span', 'kanban-conta');
      s.append(cEl('span', 'rot', c.rot()), cEl('b', null, porCol[c.chave].length));
      cont.append(s);
    });
  }

  function marcarCartoes() {
    for (const b of $('kanbanQuadro').querySelectorAll('.kanban-card')) {
      const sel = b.dataset.id === selecionada;
      b.classList.toggle('selecionada', sel);
      b.setAttribute('aria-pressed', String(sel));
    }
  }

  async function carregarKanban() {
    if (!comHttp || !window.tarefas || !window.tarefas.comHttp) return avisoKanban(txt('tarefas.sem_api', 'A API de tarefas só existe com a tela servida pelo agente (phxclaw servir) ou pela ponte.'));
    if (!window.tarefas.token()) return avisoKanban(txt('tarefas.pede_token', 'Informe o token de acesso para ver as tarefas.'));
    let ts;
    try { ts = await apiTarefas('GET', 'tasks'); } catch (e) {
      if (e.rede) return avisoKanban(txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'));
      if (e.status === 401) return avisoKanban(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'));
      return avisoKanban(txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }));
    }
    const ativo = projetoAtivo();
    // O servidor JA corta a lista por projeto pelo cabecalho X-PhxClaw-Projeto (rbac::visiveis),
    // e o resumo da lista (TaskSummary) nao traz `projeto`. Entao: tarefa cujo projeto se
    // conhece (o detalhe carimba, ou foi criada aqui) confere com o ativo; tarefa SEM projeto
    // conhecido confia no corte do servidor -- senao, com RBAC, o quadro sairia vazio mesmo o
    // servidor mandando as tarefas certas.
    lista = ts.filter(t => { const p = projetoDaTarefa(t); return p === null || p === ativo.id; });
    desenharQuadro(lista);
  }

  // Recolher/expandir: conveniencia de viewer (localStorage), nao dado. No celular o padrao e
  // recolhido (a faixa de contadores + a tarefa embaixo); na mesa, expandido.
  const kanban = $('kanban');
  function aplicarKanban() {
    let rec = LS.get(K_KANBAN);
    if (rec !== '0' && rec !== '1') rec = (window.matchMedia && window.matchMedia('(max-width: 640px)').matches) ? '1' : '0';
    kanban.dataset.recolhido = rec;
    $('kanbanAlternar').setAttribute('aria-expanded', String(rec === '0'));
  }
  $('kanbanAlternar').addEventListener('click', () => {
    const rec = kanban.dataset.recolhido === '1' ? '0' : '1';
    LS.set(K_KANBAN, rec);
    kanban.dataset.recolhido = rec;
    $('kanbanAlternar').setAttribute('aria-expanded', String(rec === '0'));
  });

  /* ===================== REALIZACAO (detalhe do cartao) ===================== */
  function balao(lado, autor, textoDado) {
    const w = cEl('div', `balao balao-${lado}`);
    w.append(cEl('span', 'balao-autor', autor));
    w.append(cEl('p', 'balao-texto', textoDado));
    return w;
  }
  function balaoTrabalhando() {
    const w = cEl('div', 'balao balao-agente balao-trabalhando');
    w.append(cEl('span', 'balao-autor', txt('conversa.agente', 'Agente')));
    w.append(cEl('p', 'balao-texto', txt('conversa.trabalhando', 'Trabalhando…')));
    return w;
  }
  function balaoAgente(t) {
    if (t.answer) return balao('agente', txt('conversa.agente', 'Agente'), t.answer);
    if (t.error) { const b = balao('agente', txt('conversa.agente', 'Agente'), t.error); b.querySelector('.balao-texto').classList.add('balao-erro'); return b; }
    if (['completed', 'failed', 'cancelled', 'budget_exceeded'].includes(t.status)) return balao('agente', txt('conversa.agente', 'Agente'), txt('conversa.sem_resposta', 'Sem resposta.'));
    return balaoTrabalhando();
  }

  // Recebe o texto JA resolvido: a chave fica literal no txt(...) de quem chama, onde o
  // conferidor da fabrica a le (chave montada em variavel ele nao veria, e viraria «morta»).
  function rotuloBloco(texto) { return cEl('h4', 'det-rot', texto); }

  // 7. PROGRESSO: medido, nunca inventado. Feito=100, Backlog=0, Fazendo=passos/plano; sem
  // plano, barra indeterminada e «em andamento», sem %.
  function medirProgresso(t) {
    if (t.status === 'completed') return { pct: 100 };
    if (t.status === 'pending') return { pct: 0 };
    const plano = (t.plan || []).length;
    const feitos = (t.steps || []).length;
    if (plano > 0) return { pct: Math.min(100, Math.round((feitos / plano) * 100)) };
    return { indet: true };
  }
  function secProgresso(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.progresso', 'Progresso')));
    const p = medirProgresso(t);
    const barra = cEl('div', 'prog-barra');
    const traco = cEl('i');
    if (p.indet) { barra.classList.add('indet'); traco.style.width = '40%'; }
    else traco.style.width = `${p.pct}%`;
    barra.append(traco);
    s.append(barra);
    s.append(cEl('span', 'prog-valor', p.indet ? txt('cartao.em_andamento', 'em andamento') : `${p.pct}%`));
    return s;
  }

  function duracao(deMs, ateMs) {
    const seg = Math.max(0, Math.round((ateMs - deMs) / 1000));
    if (seg < 60) return txt('cartao.dur_seg', '{s}s', { s: seg });
    const min = Math.floor(seg / 60);
    if (min < 60) return txt('cartao.dur_min', '{m}min', { m: min });
    return txt('cartao.dur_hora', '{h}h {m}min', { h: Math.floor(min / 60), m: min % 60 });
  }
  const FINAIS = ['completed', 'failed', 'cancelled', 'budget_exceeded'];
  function fimDe(t) {
    const h = (t.historico || []).filter(x => FINAIS.includes(x.estado));
    return h.length ? h[h.length - 1].em : null;
  }
  // 6. INICIO e FIM.
  function secInicioFim(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.inicio_fim', 'Início e fim')));
    const l = cEl('dl', 'det-ter');
    l.append(cEl('dt', null, txt('cartao.inicio', 'Início')), Object.assign(cEl('dd', 'num', dataHora(t.created_at)), {}));
    const fim = fimDe(t);
    if (fim) l.append(cEl('dt', null, txt('cartao.fim', 'Fim')), cEl('dd', 'num', dataHora(fim)));
    else {
      const d = cEl('dd', 'num');
      d.append(cEl('span', null, txt('cartao.em_andamento', 'em andamento')), document.createTextNode(' · '), cEl('span', null, duracao(new Date(t.created_at).getTime(), Date.now())));
      l.append(cEl('dt', null, txt('cartao.fim', 'Fim')), d);
    }
    s.append(l);
    return s;
  }

  // 5. QUAIS AGENTES FIZERAM: o modelo (dado), os subagentes (tarefas com parent apontando
  // para esta, da lista ja carregada) e as ferramentas distintas dos passos (dado).
  function secAgentes(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.agentes', 'Agentes')));
    const linha = cEl('p', 'det-agente');
    linha.append(cEl('span', 'det-chave', txt('cartao.agente_rotulo', 'Agente')), cEl('b', 'dado', t.model || '—'));
    s.append(linha);
    const subs = lista.filter(x => x.parent === t.id);
    if (subs.length) {
      s.append(cEl('span', 'det-chave', txt('cartao.subagentes', 'Subagentes')));
      const ul = cEl('ul', 'det-subs');
      for (const x of subs) ul.append(cEl('li', 'dado', x.objective));
      s.append(ul);
    }
    const tools = [...new Set((t.steps || []).map(p => p.tool).filter(Boolean))];
    if (tools.length) {
      s.append(cEl('span', 'det-chave', txt('cartao.ferramentas_usadas', 'Ferramentas usadas')));
      const chips = cEl('div', 'det-chips');
      for (const tl of tools) chips.append(cEl('code', 'dado', tl));
      s.append(chips);
    }
    return s;
  }

  // 8. PIZZA DA ATIVIDADE: contagem REAL dos passos por ferramenta (ou «pensamento» sem
  // ferramenta). SVG a mao, zero dependencia. Sem passos, nao desenha pizza falsa.
  const PZ = ['--acao-consultar', '--acao-incluir', '--acao-alterar', '--acao-marcar', '--violeta', '--ouro', '--info', '--vermelho'];
  function ponto(cx, cy, r, g) { const a = (g * Math.PI) / 180; return [cx + r * Math.cos(a), cy + r * Math.sin(a)]; }
  function setor(cx, cy, r, a1, a2) {
    const [x1, y1] = ponto(cx, cy, r, a1); const [x2, y2] = ponto(cx, cy, r, a2);
    const grande = a2 - a1 > 180 ? 1 : 0;
    return `M${cx} ${cy} L${x1.toFixed(3)} ${y1.toFixed(3)} A${r} ${r} 0 ${grande} 1 ${x2.toFixed(3)} ${y2.toFixed(3)} Z`;
  }
  function secAtividade(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.atividade', 'Atividade')));
    const passos = t.steps || [];
    if (!passos.length) { s.append(cEl('p', 'vazio', txt('cartao.sem_atividade', 'Sem atividade ainda.'))); return s; }
    const grupos = new Map();
    for (const p of passos) {
      const chave = p.tool || '__pensamento';
      if (!grupos.has(chave)) grupos.set(chave, { chave, rotulo: p.tool || txt('tarefas.pensamento', 'pensamento'), dado: !!p.tool, n: 0 });
      grupos.get(chave).n += 1;
    }
    const fatias = [...grupos.values()].sort((a, b) => b.n - a.n);
    const total = fatias.reduce((n, f) => n + f.n, 0);
    const NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('viewBox', '0 0 42 42');
    svg.setAttribute('class', 'pizza');
    svg.setAttribute('role', 'img');
    svg.setAttribute('aria-label', txt('cartao.atividade_desc', 'Atividade: {partes}', { partes: fatias.map(f => `${f.rotulo} ${f.n}`).join(', ') }));
    let ang = -90;
    fatias.forEach((f, i) => {
      const path = document.createElementNS(NS, 'path');
      if (fatias.length === 1) path.setAttribute('d', `M21 5.1 A15.9 15.9 0 1 1 20.98 5.1 Z`);
      else { const a2 = ang + (f.n / total) * 360; path.setAttribute('d', setor(21, 21, 15.9, ang, a2)); ang = a2; }
      path.setAttribute('fill', `var(${PZ[i % PZ.length]})`);
      path.setAttribute('stroke', 'var(--painel)');
      path.setAttribute('stroke-width', '0.7');
      svg.append(path);
    });
    const leg = cEl('ul', 'pizza-legenda');
    fatias.forEach((f, i) => {
      const li = cEl('li');
      const am = cEl('i', 'pizza-amostra');
      am.style.background = `var(${PZ[i % PZ.length]})`;
      li.append(am, cEl('span', f.dado ? 'rot dado' : 'rot', f.rotulo), cEl('b', 'num', f.n));
      leg.append(li);
    });
    const wrap = cEl('div', 'pizza-wrap');
    wrap.append(svg, leg);
    s.append(wrap);
    return s;
  }

  // 2. LINHA DO TEMPO: cada transicao com data/hora (fuso local, tabular-nums).
  function secTimeline(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.linha_tempo', 'Linha do tempo')));
    const ol = cEl('ol', 'det-timeline');
    for (const x of t.historico || []) {
      const li = cEl('li');
      li.append(cEl('span', `tarefa-estado estado-${x.estado}`, window.tarefas.estado(x.estado)), cEl('time', 'num', dataHora(x.em)));
      li.querySelector('time').dateTime = x.em;
      ol.append(li);
    }
    s.append(ol);
    return s;
  }

  // 3. AJUSTES: os refinamentos que o usuario deu depois (dado).
  function secAjustes(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.ajustes', 'Ajustes')));
    const ul = cEl('ul', 'det-ajustes');
    for (const a of t.ajustes || []) {
      const li = cEl('li');
      li.append(cEl('p', 'dado', a.texto), cEl('time', 'num', dataHora(a.em)));
      li.querySelector('time').dateTime = a.em;
      ul.append(li);
    }
    s.append(ul);
    return s;
  }

  // 4. ARTEFATOS: so o que a tarefa listou (nunca inventado), como links que baixam pela rota
  // de arquivos da tarefa (a mesma da tela Tarefas). Caminho e DADO.
  function secArtefatos(t) {
    const s = cEl('section', 'det-bloco');
    s.append(rotuloBloco(txt('cartao.artefatos', 'Artefatos')));
    const ul = cEl('ul', 'det-artefatos');
    for (const a of t.artifacts || []) {
      const li = cEl('li');
      const link = cEl('a', 'tarefa-artefato dado', a.path);
      link.href = '#';
      link.addEventListener('click', async e => {
        e.preventDefault();
        try {
          const r = await fetch(`./v1/tasks/${encodeURIComponent(t.id)}/artifacts/${a.path.split('/').map(encodeURIComponent).join('/')}`, { headers: { Authorization: `Bearer ${window.tarefas.token()}` } });
          if (!r.ok) throw new Error(String(r.status));
          const url = URL.createObjectURL(await r.blob());
          Object.assign(document.createElement('a'), { href: url, download: a.path.split('/').pop() }).click();
          setTimeout(() => URL.revokeObjectURL(url), 10000);
        } catch { avisoStatus(txt('cartao.artefato_erro', 'Não foi possível baixar o artefato.'), true); }
      });
      li.append(link);
      ul.append(li);
    }
    s.append(ul);
    s.append(secTestar(t));
    return s;
  }

  // «Testar o resultado»: so aparece quando a tarefa TERMINOU (Feito) e produziu um artefato
  // WEB abrivel (um .html). Abre em TELA NOVA, na URL do artefato da tarefa -- o modo de USAR o
  // software gerado, nao so espiar. Artefato que nao e web (um backend) nao mostra «Testar»: a
  // tela nao finge que roda. (A rota de artefato do agente hoje exige o Bearer, baixa como
  // anexo e manda `CSP: sandbox`; abrir e rodar de verdade depende de rota de execucao do
  // servidor -- item de backend, nao se inventa aqui.)
  function artefatoWeb(t) { return t.status === 'completed' ? (t.artifacts || []).find(a => /\.html?$/i.test(a.path)) : null; }
  function secTestar(t) {
    const a = artefatoWeb(t);
    const box = cEl('div', 'det-testar');
    if (!a) return box;
    const url = `./v1/tasks/${encodeURIComponent(t.id)}/artifacts/${a.path.split('/').map(encodeURIComponent).join('/')}`;
    const abrir = cEl('a', 'acao consulta test-nova', txt('cartao.testar_nova', 'TESTAR EM TELA NOVA'));
    abrir.href = url;
    abrir.target = '_blank';
    abrir.rel = 'noopener';
    box.append(abrir, cEl('p', 'test-nota', txt('cartao.testar_nota', 'Abre o resultado numa aba nova para usar o software gerado.')));
    return box;
  }

  function blocoDetalhe(t) {
    const d = cEl('div', 'detalhe');
    d.append(secProgresso(t), secInicioFim(t), secAgentes(t), secAtividade(t));
    if ((t.historico || []).length) d.append(secTimeline(t));
    if ((t.ajustes || []).length) d.append(secAjustes(t));
    if ((t.artifacts || []).length) d.append(secArtefatos(t));
    return d;
  }

  function selecionar(id) {
    selecionada = id;
    assinaturaDet = '';
    marcarCartoes();
    renderRealizacao();
  }

  const historicoEl = $('conversaHistorico');
  function pertoDoFim() { return historicoEl.scrollHeight - historicoEl.scrollTop - historicoEl.clientHeight < 80; }

  async function renderRealizacao() {
    if (!projetoAtivo()) return;
    if (!selecionada) { historicoEl.replaceChildren(cEl('p', 'conversa-vazio', txt('conversa.vazio', 'Comece uma conversa: digite ou fale seu pedido abaixo.'))); assinaturaDet = ''; return; }
    let t;
    try { t = await apiTarefas('GET', `tasks/${encodeURIComponent(selecionada)}`); }
    catch (e) { if (e.status === 404) { selecionada = null; return renderRealizacao(); } return; }
    const assinatura = JSON.stringify([t.id, t.status, t.answer, t.error, (t.steps || []).length, (t.historico || []).length, (t.ajustes || []).length, (t.artifacts || []).length, idiomas.atual]);
    if (assinatura === assinaturaDet) return;
    const colar = pertoDoFim();
    assinaturaDet = assinatura;
    const frag = document.createDocumentFragment();
    const barra = cEl('div', 'realizacao-barra');
    const voltar = cEl('button', 'acao consulta', txt('cartao.fechar', 'VOLTAR AO QUADRO'));
    voltar.type = 'button';
    voltar.addEventListener('click', () => { selecionada = null; marcarCartoes(); renderRealizacao(); });
    barra.append(cEl('span', `tarefa-estado estado-${t.status}`, window.tarefas.estado(t.status)), voltar);
    frag.append(barra);
    frag.append(balao('usuario', txt('conversa.voce', 'Você'), t.objective));
    frag.append(blocoDetalhe(t));
    frag.append(balaoAgente(t));
    historicoEl.replaceChildren(frag);
    if (colar) historicoEl.scrollTop = historicoEl.scrollHeight;
  }

  /* ===================== ENTRADA (enviar, campo que cresce) ===================== */
  const campo = $('conversaCampo');
  const status = $('conversaStatus');
  function avisoStatus(texto, erro) {
    status.textContent = texto;
    status.classList.toggle('aviso', !!erro);
    status.setAttribute('role', erro ? 'alert' : 'status');
  }
  function cresce() { campo.style.height = 'auto'; campo.style.height = `${Math.min(campo.scrollHeight, 160)}px`; }
  campo.addEventListener('input', cresce);
  campo.addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); $('conversaEntrada').requestSubmit(); }
  });

  function falhouEnvio(e) {
    if (e.status === 401) avisoStatus(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true);
    else if (e.rede) avisoStatus(txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), true);
    else avisoStatus(txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }), true);
  }

  $('conversaEntrada').addEventListener('submit', async e => {
    e.preventDefault();
    const ativo = projetoAtivo();
    if (!ativo) { atualizar(); return; }
    const objetivo = campo.value.trim();
    if (!objetivo) return;
    $('conversaEnviar').disabled = true;
    try {
      const r = await apiTarefas('POST', 'tasks', { objective: objetivo });
      marcarTarefa(r.id, ativo.id);
      campo.value = '';
      cresce();
      selecionada = r.id;
      assinaturaDet = '';
      // Retrato instantaneo: o balao do pedido e o «trabalhando» antes mesmo da sondagem.
      historicoEl.replaceChildren(balao('usuario', txt('conversa.voce', 'Você'), objetivo), balaoTrabalhando());
      historicoEl.scrollTop = historicoEl.scrollHeight;
      avisoStatus('');
      await atualizar();
    } catch (er) { falhouEnvio(er); }
    finally { $('conversaEnviar').disabled = false; }
  });

  /* ===================== MICROFONE ===================== */
  // Grava no navegador e manda o audio para a rota de voz do agente (POST ./v1/voz/transcrever);
  // a transcricao {texto} volta para o CAMPO, e quem fala revisa e envia -- nao auto-envia, e
  // mais seguro. A guarda da permissao e OBRIGATORIA: recusada, a tela avisa e segue viva.
  //
  // Grava em WAV 16 kHz mono, nao webm: o whisper LOCAL (o caminho offline-padrao do agente) so
  // le WAV, e o MediaRecorder entrega webm/opus. Entao captura o PCM pelo Web Audio, reamostra
  // para 16 kHz e monta o cabecalho WAV a mao (zero dependencia). Content-Type: audio/wav.
  const mic = $('conversaMic');
  let fluxoMic = null;
  let audioCtx = null;
  let fonteMic = null;
  let processador = null;
  let blocos = [];
  let gravando = false;

  function pintarMic(g) {
    mic.classList.toggle('gravando', g);
    mic.setAttribute('aria-pressed', String(g));
    const al = g ? txt('conversa.mic_parar_al', 'Parar a gravação') : txt('conversa.mic_al', 'Falar em vez de digitar');
    mic.setAttribute('aria-label', al);
    mic.setAttribute('title', al);
  }
  function soltarFluxo() { if (fluxoMic) { for (const f of fluxoMic.getTracks()) { try { f.stop(); } catch { /* ja parou */ } } fluxoMic = null; } }

  // Reamostragem linear para 16 kHz e cabecalho WAV PCM 16-bit mono, a mao.
  function reamostrar(ent, de, para) {
    if (de === para) return ent;
    const r = para / de; const n = Math.round(ent.length * r); const saida = new Float32Array(n);
    for (let i = 0; i < n; i++) { const pos = i / r; const i0 = Math.floor(pos); const i1 = Math.min(i0 + 1, ent.length - 1); const f = pos - i0; saida[i] = ent[i0] * (1 - f) + ent[i1] * f; }
    return saida;
  }
  function wav16k(float32, taxa) {
    const amostras = reamostrar(float32, taxa, 16000);
    const n = amostras.length; const buf = new ArrayBuffer(44 + n * 2); const dv = new DataView(buf);
    const str = (o, s) => { for (let i = 0; i < s.length; i++) dv.setUint8(o + i, s.charCodeAt(i)); };
    str(0, 'RIFF'); dv.setUint32(4, 36 + n * 2, true); str(8, 'WAVE'); str(12, 'fmt '); dv.setUint32(16, 16, true);
    dv.setUint16(20, 1, true); dv.setUint16(22, 1, true); dv.setUint32(24, 16000, true); dv.setUint32(28, 16000 * 2, true);
    dv.setUint16(32, 2, true); dv.setUint16(34, 16, true); str(36, 'data'); dv.setUint32(40, n * 2, true);
    let o = 44; for (let i = 0; i < n; i++) { const s = Math.max(-1, Math.min(1, amostras[i])); dv.setInt16(o, s < 0 ? s * 0x8000 : s * 0x7fff, true); o += 2; }
    return new Blob([buf], { type: 'audio/wav' });
  }

  // Teto de duracao da gravacao do microfone: o PCM cru acumula em `blocos` a cada quadro e
  // so e reamostrado ao parar; sem teto, uma gravacao esquecida aberta enche a aba (auto-DoS)
  // muito antes do teto de 200 MiB do servidor. Cinco minutos sao fala de sobra para um pedido.
  const SEGUNDOS_MAX_GRAVACAO = 300;

  // Soma um quadro ao acumulado e diz se ESTOUROU o teto. Pura (sem DOM) de proposito, para o
  // limite ser exercitavel sem hardware de audio.
  function acumularAmostras(total, novo, teto) {
    const soma = total + novo;
    return { total: soma, estourou: soma >= teto };
  }

  async function pararPorTeto() {
    // Atingido o teto: para como uma parada manual (monta o WAV e envia o que ja foi captado)
    // e avisa, pelo MESMO caminho de aviso. Sem isto o acumulo nao teria fim.
    await pararGravacao();
    avisoStatus(txt('conversa.mic_teto', 'Gravação muito longa: parei e enviei o que foi captado.'), true);
  }

  async function iniciarGravacao() {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia || !AC) {
      return avisoStatus(txt('conversa.mic_sem_suporte', 'Este navegador não grava áudio; digite o pedido.'), true);
    }
    // A GUARDA: pedido de permissao recusado (ou microfone bloqueado por politica) NAO pode
    // travar a tela -- cai num aviso claro, e o campo continua servindo.
    try { fluxoMic = await navigator.mediaDevices.getUserMedia({ audio: true }); }
    catch { return avisoStatus(txt('conversa.mic_negado', 'Microfone bloqueado: permita o acesso ao microfone ou digite o pedido.'), true); }
    try {
      // Pede 16 kHz ja na fonte; se o navegador recusar a taxa, grava na taxa dele e a
      // reamostragem acerta para 16 kHz na hora de montar o WAV.
      try { audioCtx = new AC({ sampleRate: 16000 }); } catch { audioCtx = new AC(); }
      await audioCtx.resume();
      fonteMic = audioCtx.createMediaStreamSource(fluxoMic);
      processador = audioCtx.createScriptProcessor(4096, 1, 1);
      blocos = [];
      let acumuladas = 0;
      const tetoAmostras = Math.round((audioCtx.sampleRate || 16000) * SEGUNDOS_MAX_GRAVACAO);
      processador.onaudioprocess = ev => {
        const quadro = new Float32Array(ev.inputBuffer.getChannelData(0));
        blocos.push(quadro);
        const r = acumularAmostras(acumuladas, quadro.length, tetoAmostras);
        acumuladas = r.total;
        if (r.estourou && gravando) pararPorTeto();
      };
      // O ScriptProcessor so dispara ligado a um destino; um ganho ZERO evita ouvir o microfone.
      const mudo = audioCtx.createGain(); mudo.gain.value = 0;
      fonteMic.connect(processador); processador.connect(mudo); mudo.connect(audioCtx.destination);
    } catch { await encerrarAudio(); return avisoStatus(txt('conversa.mic_sem_suporte', 'Este navegador não grava áudio; digite o pedido.'), true); }
    gravando = true;
    pintarMic(true);
    avisoStatus(txt('conversa.gravando', 'Gravando… toque de novo para parar.'));
  }

  async function encerrarAudio() {
    try { if (processador) { processador.disconnect(); processador.onaudioprocess = null; } } catch { /* ja solto */ }
    try { if (fonteMic) fonteMic.disconnect(); } catch { /* idem */ }
    const taxa = audioCtx ? audioCtx.sampleRate : 16000;
    if (audioCtx) { try { await audioCtx.close(); } catch { /* ja fechou */ } audioCtx = null; }
    fonteMic = null; processador = null;
    soltarFluxo();
    return taxa;
  }

  async function pararGravacao() {
    gravando = false;
    pintarMic(false);
    const taxa = await encerrarAudio();
    let total = 0; for (const b of blocos) total += b.length;
    if (!total) { avisoStatus(''); return; }
    const pcm = new Float32Array(total); let off = 0; for (const b of blocos) { pcm.set(b, off); off += b.length; }
    blocos = [];
    await enviarAudio(wav16k(pcm, taxa));
  }

  // O erro vem na chave `error` (dado); cada codigo tem um aviso amigavel da fabrica, e a
  // mensagem do servidor (que no 503 nomeia offline/whisper) entra como dado.
  function avisoVoz(status, erroDado) {
    const base = status === 413 ? txt('conversa.voz_413', 'O áudio é grande demais.')
      : status === 415 ? txt('conversa.voz_415', 'Formato de áudio não aceito.')
      : status === 422 ? txt('conversa.voz_422', 'Não entendi nenhuma fala. Tente de novo.')
      : status === 503 ? txt('conversa.voz_503', 'O serviço de voz não está disponível agora.')
      : txt('conversa.transcrever_erro', 'Não foi possível transcrever o áudio. Tente de novo ou digite.');
    avisoStatus(erroDado ? `${base} (${erroDado})` : base, true);
  }

  async function enviarAudio(wav) {
    if (!wav || !wav.size) { avisoStatus(''); return; }
    avisoStatus(txt('conversa.transcrevendo', 'Transcrevendo…'));
    try {
      const r = await fetch('./v1/voz/transcrever', { method: 'POST', headers: { Authorization: `Bearer ${window.tarefas.token()}`, 'Content-Type': 'audio/wav', ...cabProjeto() }, body: wav, cache: 'no-store' });
      const v = await r.json().catch(() => ({}));
      if (!r.ok) { avisoVoz(r.status, typeof v.error === 'string' ? v.error : ''); return; }
      const texto = typeof v.texto === 'string' ? v.texto : '';
      if (!texto) { avisoVoz(422, ''); return; }
      campo.value = campo.value.trim() ? `${campo.value.trim()} ${texto}` : texto; // texto e DADO
      cresce();
      campo.focus();
      avisoStatus('');
    } catch { avisoStatus(txt('conversa.transcrever_erro', 'Não foi possível transcrever o áudio. Tente de novo ou digite.'), true); }
  }

  mic.addEventListener('click', () => { if (gravando) pararGravacao(); else iniciarGravacao(); });

  /* ===================== CICLO DA TELA ===================== */
  function mostrarGate() { gateEl.hidden = false; corpoEl.hidden = true; }
  function mostrarCorpo() { gateEl.hidden = true; corpoEl.hidden = false; }

  async function atualizar() {
    renderSeletor();
    if (!projetoAtivo()) { mostrarGate(); return; }
    mostrarCorpo();
    aplicarKanban();
    await carregarKanban();
    await renderRealizacao();
  }

  carregadores.conversa = () => {
    semear();
    atualizar();
    clearInterval(relogio);
    relogio = setInterval(() => {
      if (document.body.dataset.tela !== 'conversa' || document.hidden) return;
      atualizar();
    }, 2500);
  };

  idiomas.aoTrocar(() => {
    renderSeletor();
    if (dlg.open) renderLista();
    if (document.body.dataset.tela === 'conversa') { assinaturaDet = ''; atualizar(); }
  });

  // Estado inicial do seletor e do kanban desde o primeiro quadro (antes de abrir a tela).
  renderSeletor();
  aplicarKanban();
  idiomas.aoTrocar(renderSeletor);

  // Para os roteiros de prova e para a Visao geral, se um dia reusar: um ponto so.
  window.conversa = { atualizar, selecionar, abrirProjetos: abrirDialogo, projetoAtivo, acumularAmostras, SEGUNDOS_MAX_GRAVACAO };

  // Aberta direto por #conversa / ?tela=conversa: o mostrarTela do app.js ja rodou antes deste
  // script existir, entao o carregador nao disparou -- dispara aqui, como a tela Tarefas faz.
  if (document.body.dataset.tela === 'conversa') carregadores.conversa();
})();

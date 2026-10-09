// Tela Insights: o painel das execucoes (o «Insights» do n8n), lido de GET /v1/insights --
// por estado, duracao p50/p95, custo, falhas mais comuns, por fluxo e por dia, num periodo.
//
// O numero sai SO da rota (que le o disco das tarefas, cortado pelo projeto do RBAC): nada
// aqui conta nem estima. Rotulo pela fabrica (txt); nome de fluxo, motivo de falha e moeda sao
// DADO e entram por textContent, intactos (nunca innerHTML: o motivo vem de uma ferramenta).
// O cliente HTTP e o da tela Tarefas (window.tarefas.api), um so para o token e os erros.
(() => {
  const $ = id => document.getElementById(id);
  const raiz = $('insightsConteudo');
  const status = $('insightsStatus');
  const periodo = $('insightsPeriodo');
  const fluxo = $('insightsFluxo');
  if (!raiz) return;
  let fluxosConhecidos = [];
  // A ultima resposta: trocar o idioma redesenha os rotulos sem pedir a rota de novo.
  let ultima = null;

  function aviso(texto, ehErro, comTentar = false) {
    if (status.textContent !== texto) status.textContent = texto;
    status.classList.toggle('aviso', !!ehErro);
    status.setAttribute('role', ehErro ? 'alert' : 'status');
    mostrarTentar('insights', comTentar);
  }

  function falhou(e) {
    if (e.status === 401) aviso(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true);
    else if (e.status === 403) aviso(txt('insights.sem_direito', 'Seu papel não lê as execuções deste projeto.'), true);
    else if (e.rede) aviso(txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), true, true);
    else aviso(txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }), true, true);
  }

  // Numero do painel: a mesma formatacao do idioma da tela. Ausente e travessao, nunca zero.
  const fmt = (v, casas = 0) => (v === null || v === undefined || Number.isNaN(v))
    ? '—'
    : Number(v).toLocaleString(document.documentElement.lang || 'pt', { maximumFractionDigits: casas, minimumFractionDigits: casas });
  function duracao(ms) {
    if (ms === null || ms === undefined) return '—';
    if (ms < 1000) return txt('insights.ms', '{n} ms', { n: fmt(ms) });
    if (ms < 60000) return txt('insights.s', '{n} s', { n: fmt(ms / 1000, 1) });
    return txt('insights.min', '{n} min', { n: fmt(ms / 60000, 1) });
  }
  const pct = v => (v === null || v === undefined) ? '—' : `${fmt(v * 100, 1)}%`;
  function custo(c) {
    if (!c || c.total === null || c.total === undefined) return txt('insights.custo_nao_medido', 'não medido');
    const moeda = c.moeda || (c.moedas_misturadas ? txt('insights.moedas_misturadas', 'moedas misturadas') : '');
    return `${fmt(c.total, 4)} ${moeda}`.trim();
  }

  function cartao(rotulo, valor, nota, cor) {
    const a = el('article', `metric ${cor}`);
    const h = el('header');
    h.append(el('span', '', rotulo));
    a.append(h, el('strong', 'insights-valor', valor));
    if (nota) a.append(el('small', '', nota));
    return a;
  }

  function painel(titulo, corpo) {
    const p = el('article', 'panel insights-painel');
    const h = el('header', 'panel-head');
    h.append(el('h2', '', titulo));
    p.append(h, corpo);
    return p;
  }

  // Barras horizontais em HTML (largura proporcional ao maior): lidas por leitor de tela como
  // lista de rotulo + numero, que e o dado; a barra e enfeite.
  function barras(linhas, classe) {
    const ul = el('ul', `insights-barras ${classe || ''}`);
    const max = Math.max(1, ...linhas.map(l => l.n));
    for (const l of linhas) {
      const li = el('li');
      const r = el('span', 'insights-rotulo', l.rotulo);
      if (l.dado) r.classList.add('dado');
      const b = el('span', 'insights-barra');
      const enchimento = el('span');
      enchimento.style.width = `${Math.round((l.n / max) * 100)}%`;
      if (l.erro) enchimento.classList.add('erro');
      b.append(enchimento);
      li.append(r, b, el('b', 'insights-n', fmt(l.n)));
      ul.append(li);
    }
    if (!linhas.length) ul.append(el('li', 'vazio', txt('insights.nada', 'Nenhuma execução no período.')));
    return ul;
  }

  function desenhar(v) {
    raiz.replaceChildren();
    const kpis = el('section', 'metric-grid insights-kpis');
    kpis.append(
      cartao(txt('insights.execucoes', 'EXECUÇÕES'), fmt(v.total), txt('insights.terminadas', '{n} terminadas', { n: fmt(v.terminadas) }), 'cyan'),
      cartao(txt('insights.taxa_falha', 'TAXA DE FALHA'), pct(v.taxa_falha), txt('insights.falhas_n', '{n} falhas', { n: fmt(v.falhas) }), 'violet'),
      cartao(txt('insights.duracao', 'DURAÇÃO p50 / p95'), `${duracao(v.duracao?.p50_ms)} / ${duracao(v.duracao?.p95_ms)}`, txt('insights.amostras', '{n} execuções terminadas', { n: fmt(v.duracao?.amostras) }), 'gold'),
      cartao(txt('insights.custo', 'CUSTO'), custo(v.custo), txt('insights.custo_nota', '{n} sem custo medido', { n: fmt(v.custo?.nao_medidas) }), 'green'),
    );
    raiz.append(kpis);
    const grade = el('section', 'insights-grade');
    const estados = Object.entries(v.por_estado || {}).map(([e, n]) => ({ rotulo: window.tarefas.estado(e), n, erro: e === 'failed' || e === 'budget_exceeded' }));
    grade.append(painel(txt('insights.por_estado', 'Por estado'), barras(estados, 'estados')));
    const falhas = (v.falhas_comuns || []).map(f => ({ rotulo: f.omitido ? txt('insights.motivo_omitido', '(motivo com forma de credencial — omitido)') : f.motivo || txt('insights.sem_motivo', '(sem motivo)'), n: f.n, erro: true, dado: !f.omitido && !!f.motivo }));
    grade.append(painel(txt('insights.falhas_comuns', 'Falhas mais comuns'), barras(falhas, 'falhas')));
    const dias = (v.dias || []).map(d => ({ rotulo: d.dia, n: d.total, dado: true }));
    grade.append(painel(txt('insights.por_dia', 'Execuções por dia'), barras(dias, 'dias')));
    // Por fluxo: uma linha por fluxo, com o que o n8n chama de «failure rate» e «run time».
    const lista = el('ul', 'insights-fluxos');
    for (const f of v.fluxos || []) {
      const li = el('li');
      const nome = el('b', 'dado', f.nome);
      const linha = el('span', '', txt('insights.linha_fluxo', '{total} execuções · {falhas} falhas ({taxa}) · p50 {p50} · p95 {p95} · {custo}', {
        total: fmt(f.total), falhas: fmt(f.falhas), taxa: pct(f.taxa_falha), p50: duracao(f.duracao?.p50_ms), p95: duracao(f.duracao?.p95_ms), custo: custo(f.custo),
      }));
      li.append(nome, linha);
      lista.append(li);
    }
    if (!(v.fluxos || []).length) lista.append(el('li', 'vazio', txt('insights.sem_fluxos', 'Nenhum fluxo executado no período.')));
    grade.append(painel(txt('insights.por_fluxo', 'Por fluxo'), lista));
    raiz.append(grade);
  }

  // A lista de fluxos do filtro sai da propria resposta SEM filtro (os fluxos que rodaram no
  // periodo); com filtro, mantem a que ja havia, para a escolha atual nao sumir.
  function preencherFluxos(v) {
    if (!fluxo.value) fluxosConhecidos = (v.fluxos || []).map(f => f.nome);
    const atual = fluxo.value;
    const todos = el('option', '', txt('insights.todos_fluxos', 'Todos os fluxos'));
    todos.value = '';
    const opcoes = [todos, ...fluxosConhecidos.map(n => { const o = el('option', 'dado', n); o.value = n; return o; })];
    fluxo.replaceChildren(...opcoes);
    fluxo.value = fluxosConhecidos.includes(atual) ? atual : '';
  }

  function redesenhar(v) {
    preencherFluxos(v);
    desenhar(v);
    aviso(txt('insights.resumo', '{total} execuções de {desde} a {ate}.', {
      total: fmt(v.total),
      desde: v.desde ? new Date(v.desde).toLocaleString() : txt('insights.sempre', 'o começo'),
      ate: new Date(v.ate).toLocaleString(),
    }), false);
  }

  async function carregar() {
    if (!window.tarefas?.comHttp) {
      aviso(txt('insights.sem_api', 'O painel lê a API do agente: abra a tela pelo `phxclaw servir`.'), true);
      return;
    }
    aviso(txt('geral.lendo', 'Lendo…'), false);
    const q = new URLSearchParams({ periodo: periodo.value || '7d' });
    if (fluxo.value) q.set('fluxo', fluxo.value);
    try {
      const v = await window.tarefas.api('GET', `insights?${q}`);
      ultima = v;
      redesenhar(v);
    } catch (e) {
      falhou(e);
    }
  }

  periodo.addEventListener('change', carregar);
  fluxo.addEventListener('change', carregar);
  $('insightsAtualizar').addEventListener('click', carregar);
  carregadores.insights = carregar;
  // O resumo tambem e rotulo: sem redesenha-lo, a linha de status ficava no idioma anterior
  // (visto na captura, depois de voltar do ingles).
  idiomas.aoTrocar(() => { if (ultima) redesenhar(ultima); });
  if (document.body.dataset.tela === 'insights') carregar();
})();

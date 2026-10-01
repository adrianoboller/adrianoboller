// Tela de Tarefas: o cliente da API de tarefas do agente (a mesma do `phxclaw servir`), no
// navegador, no celular instalado e atras da ponte do controle remoto. Um cliente so: as
// rotas sao relativas a pagina, entao servido pelo agente ou pela ponte fala com quem o
// serviu, sem configurar endereco.
//
// O token fica no aparelho (localStorage) e vai so no cabecalho Authorization. Todo texto
// de tela sai da fabrica (txt); objetivo, resposta, pergunta e resumo de passo sao DADO e
// entram intactos, por textContent -- nunca por innerHTML, porque vem do modelo.
(() => {
  const GUARDADO = 'phxclaw.token';
  const $ = id => document.getElementById(id);
  const lista = $('tarefasLista');
  const detalhe = $('tarefasDetalhe');
  const status = $('tarefasStatus');
  if (!lista) return;
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';
  let selecionada = null;
  let relogio = null;
  // Assinatura do que o detalhe mostra: sem mudanca, nao redesenha -- redesenhar a cada
  // sondagem apagaria a resposta que a pessoa esta digitando.
  let desenhado = '';

  // Aplicativo instalavel: o service worker so existe servido por http(s); no Tauri a
  // pagina vem de outro esquema e o registro falharia sem motivo.
  if (comHttp && 'serviceWorker' in navigator) {
    navigator.serviceWorker.register('./sw.js').catch(e => console.warn('service worker', e));
  }

  const token = () => { try { return localStorage.getItem(GUARDADO) || ''; } catch { return ''; } };

  // Recebe o texto JA traduzido: a chave fica literal no `txt(...)` de quem chama, onde o
  // conferidor da fabrica a le.
  function aviso(texto, ehErro) {
    status.textContent = texto;
    status.classList.toggle('aviso', !!ehErro);
  }

  async function api(metodo, caminho, corpo) {
    const r = await fetch(`./v1/${caminho}`, {
      method: metodo,
      headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json' },
      body: corpo === undefined ? undefined : JSON.stringify(corpo),
      cache: 'no-store',
    });
    const v = await r.json().catch(() => ({}));
    if (!r.ok) throw Object.assign(new Error(v.error || `HTTP ${r.status}`), { status: r.status });
    return v;
  }

  function falhou(e) {
    if (e.status === 401) aviso(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true);
    else aviso(txt('tarefas.erro', 'Falhou: {erro}', { erro: e.message }), true);
  }

  // Uma chave literal por estado (e nao `tarefas.estado.${s}`): o conferidor da fabrica
  // le as chaves pedidas no fonte, e chave montada em tempo de execucao ele nao veria.
  const ESTADOS = {
    pending: () => txt('tarefas.estado.pending', 'PENDENTE'),
    awaiting_approval: () => txt('tarefas.estado.awaiting_approval', 'ESPERANDO APROVAÇÃO'),
    awaiting_input: () => txt('tarefas.estado.awaiting_input', 'ESPERANDO RESPOSTA'),
    running: () => txt('tarefas.estado.running', 'EM EXECUÇÃO'),
    completed: () => txt('tarefas.estado.completed', 'CONCLUÍDA'),
    failed: () => txt('tarefas.estado.failed', 'FALHOU'),
    cancelled: () => txt('tarefas.estado.cancelled', 'CANCELADA'),
  };
  // Estado que esta tela nao conhece e DADO do servidor: aparece cru, nao inventado.
  const estado = s => (ESTADOS[s] ? ESTADOS[s]() : s);

  function el(tag, classe, texto) {
    const e = document.createElement(tag);
    if (classe) e.className = classe;
    if (texto !== undefined) e.textContent = texto;
    return e;
  }

  function botao(classe, texto, acao) {
    const b = el('button', `acao ${classe}`, texto);
    b.type = 'button';
    b.addEventListener('click', async () => {
      b.disabled = true;
      try { await acao(); await atualizar(); } catch (e) { falhou(e); } finally { b.disabled = false; }
    });
    return b;
  }

  async function carregarLista() {
    const ts = await api('GET', 'tasks');
    ts.sort((a, b) => (a.created_at < b.created_at ? 1 : -1));
    lista.replaceChildren();
    if (!ts.length) lista.append(el('li', 'vazio', txt('tarefas.nenhuma', 'Nenhuma tarefa ainda.')));
    for (const t of ts.slice(0, 50)) {
      const li = el('li', `tarefa-item estado-${t.status}`);
      li.dataset.id = t.id;
      li.tabIndex = 0;
      li.append(el('span', 'tarefa-estado', estado(t.status)), el('span', 'tarefa-objetivo', t.objective));
      li.classList.toggle('selecionada', t.id === selecionada);
      const abrir = () => { selecionada = t.id; atualizar(); };
      li.addEventListener('click', abrir);
      li.addEventListener('keydown', e => { if (e.key === 'Enter') abrir(); });
      lista.append(li);
    }
    aviso(txt('tarefas.resumo', '{n} tarefa(s) • atualizado agora', { n: ts.length }));
  }

  async function carregarDetalhe() {
    if (!selecionada) { detalhe.hidden = true; return; }
    const t = await api('GET', `tasks/${encodeURIComponent(selecionada)}`);
    const assinatura = JSON.stringify([t.id, t.status, t.steps?.length, t.question, t.answer, t.error, idiomas.atual]);
    if (assinatura === desenhado && !detalhe.hidden) return;
    desenhado = assinatura;
    detalhe.hidden = false;
    detalhe.replaceChildren();
    const cab = el('header', 'tarefa-cab');
    cab.append(el('span', `tarefa-estado estado-${t.status}`, estado(t.status)), el('h2', 'tarefa-titulo', t.objective));
    detalhe.append(cab);
    const acoes = el('div', 'acoes');
    if (t.status === 'awaiting_approval') {
      if (t.plan?.length) {
        const ol = el('ol', 'tarefa-plano');
        for (const p of t.plan) ol.append(el('li', '', p));
        detalhe.append(el('h3', '', txt('tarefas.plano', 'Plano')), ol);
      }
      acoes.append(botao('inclui', txt('tarefas.aprovar', 'APROVAR PLANO'), () => api('POST', `tasks/${t.id}/approve`)));
    }
    if (t.status === 'awaiting_input' && t.question) {
      const caixa = el('form', 'tarefa-pergunta');
      const campo = el('input', 'filtro');
      campo.id = 'tarefasResposta';
      campo.setAttribute('aria-label', txt('tarefas.resposta', 'Sua resposta'));
      campo.placeholder = txt('tarefas.resposta', 'Sua resposta');
      const enviar = el('button', 'acao altera', txt('tarefas.responder', 'RESPONDER'));
      caixa.append(el('p', 'tarefa-questao', t.question), campo, enviar);
      caixa.addEventListener('submit', async e => {
        e.preventDefault();
        if (!campo.value.trim()) return;
        enviar.disabled = true;
        try { await api('POST', `tasks/${t.id}/answer`, { answer: campo.value }); await atualizar(); } catch (er) { falhou(er); } finally { enviar.disabled = false; }
      });
      detalhe.append(caixa);
    }
    if (!['completed', 'failed', 'cancelled'].includes(t.status)) {
      acoes.append(botao('exclui', txt('tarefas.cancelar', 'CANCELAR'), () => api('POST', `tasks/${t.id}/cancel`)));
    }
    if (acoes.children.length) detalhe.append(acoes);
    if (t.answer) detalhe.append(el('h3', '', txt('tarefas.resposta_final', 'Resposta')), el('p', 'tarefa-resposta', t.answer));
    if (t.error) detalhe.append(el('p', 'tarefa-erro', t.error));
    if (t.steps?.length) {
      const ol = el('ol', 'tarefa-passos');
      for (const p of t.steps) {
        const li = el('li', `passo passo-${p.outcome}`);
        li.append(el('b', '', p.tool || txt('tarefas.pensamento', 'pensamento')), el('span', '', p.summary));
        ol.append(li);
      }
      detalhe.append(el('h3', '', txt('tarefas.passos', 'Passos')), ol);
    }
    for (const a of t.artifacts || []) {
      const link = el('a', 'tarefa-artefato', a.path);
      link.href = '#';
      link.addEventListener('click', async e => {
        e.preventDefault();
        const r = await fetch(`./v1/tasks/${t.id}/artifacts/${a.path.split('/').map(encodeURIComponent).join('/')}`, { headers: { Authorization: `Bearer ${token()}` } });
        if (!r.ok) return falhou(Object.assign(new Error(`HTTP ${r.status}`), { status: r.status }));
        const url = URL.createObjectURL(await r.blob());
        Object.assign(document.createElement('a'), { href: url, download: a.path.split('/').pop() }).click();
        setTimeout(() => URL.revokeObjectURL(url), 10000);
      });
      detalhe.append(link);
    }
  }

  async function atualizar() {
    if (!comHttp) { aviso(txt('tarefas.sem_api', 'A API de tarefas só existe com a tela servida pelo agente (phxclaw servir) ou pela ponte.'), true); return; }
    if (!token()) { aviso(txt('tarefas.pede_token', 'Informe o token de acesso para ver as tarefas.'), true); return; }
    try { await carregarLista(); await carregarDetalhe(); } catch (e) { falhou(e); }
  }

  $('tarefasToken').addEventListener('submit', e => {
    e.preventDefault();
    const campo = $('tarefasTokenCampo');
    try { localStorage.setItem(GUARDADO, campo.value.trim()); } catch { /* sem armazenamento: so nesta visita */ }
    campo.value = '';
    atualizar();
  });

  $('tarefasNova').addEventListener('submit', async e => {
    e.preventDefault();
    const objetivo = $('tarefasObjetivo').value.trim();
    if (!objetivo) return;
    try {
      const r = await api('POST', 'tasks', { objective: objetivo, plan_first: $('tarefasPlano').checked });
      $('tarefasObjetivo').value = '';
      selecionada = r.id;
      await atualizar();
    } catch (er) { falhou(er); }
  });

  // Acompanha so enquanto a tela esta aberta: sondagem com a tela escondida gasta a
  // bateria do celular por nada.
  carregadores.tarefas = () => {
    atualizar();
    clearInterval(relogio);
    relogio = setInterval(() => {
      if (document.body.dataset.tela !== 'tarefas' || document.hidden) return;
      atualizar();
    }, 2500);
  };
  idiomas.aoTrocar(() => { if (document.body.dataset.tela === 'tarefas') atualizar(); });
  if (document.body.dataset.tela === 'tarefas') carregadores.tarefas();
})();

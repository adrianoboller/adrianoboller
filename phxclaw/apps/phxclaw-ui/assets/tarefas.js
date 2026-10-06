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
  // O status e regiao viva (role=status no HTML); erro vira role=alert. Texto igual nao se
  // reescreve: a sondagem de 2,5 s faria o leitor de tela repetir a mesma frase.
  function aviso(texto, ehErro, comTentar = false) {
    if (status.textContent !== texto) status.textContent = texto;
    status.classList.toggle('aviso', !!ehErro);
    status.setAttribute('role', ehErro ? 'alert' : 'status');
    mostrarTentar('tarefas', comTentar);
  }

  async function api(metodo, caminho, corpo) {
    let r;
    try {
      r = await fetch(`./v1/${caminho}`, {
        method: metodo,
        headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json' },
        body: corpo === undefined ? undefined : JSON.stringify(corpo),
        cache: 'no-store',
      });
    } catch {
      // fetch rejeitado e rede (o navegador nao diz mais que isso); o texto cru dele
      // («Failed to fetch») e em ingles e nao diz o que fazer.
      throw Object.assign(new Error('sem rede'), { rede: true });
    }
    const v = await r.json().catch(() => ({}));
    if (!r.ok) throw Object.assign(new Error(v.error || `HTTP ${r.status}`), { status: r.status });
    return v;
  }

  // Erro de rede e erro do agente dizem o que fazer, pela fabrica; o motivo que o agente deu
  // e DADO e entra pelo marcador.
  function falhou(e) {
    if (e.status === 401) aviso(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true);
    else if (e.rede) aviso(txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), true, true);
    else aviso(txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }), true, true);
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

  // A lista e uma grade (phx-grid, grades.js): o status aparece pelo rotulo da fabrica
  // (valores, o ReplaceValues do Janus) com a forma da condicao, a ordem padrao e a da data
  // (mais nova primeiro) e o navegador de registros anda pela lista. Linha clicada ou
  // registro escolhido no navegador abrem o detalhe.
  const ROTULOS_ESTADO = () => Object.fromEntries(Object.keys(ESTADOS).map(k => [k, estado(k)]));
  const FORMA_ESTADO = {
    completed: 'st-ok', failed: 'st-erro', cancelled: 'st-erro',
    awaiting_input: 'st-espera', awaiting_approval: 'st-espera', running: 'st-anda', pending: 'st-anda',
  };
  const vazio = el('p', 'vazio');
  const alvoGrade = el('div', 'grade-alvo');
  lista.append(vazio, alvoGrade);
  let grade = null;
  let assinaturaLista = '';

  // O registro do navegador e o numero da linha na lista inteira; a pagina tem as linhas.
  function linhaDoRegistro(g, n) {
    const tam = g.estado().tamanho || 1;
    return g.linhas()[(n - 1) % tam] || null;
  }
  // Cada render refaz o <tbody>: o id e a selecao voltam para a linha pela ordem da pagina.
  function marcarLinhas(raiz, g) {
    if (!g) return;
    const ls = g.linhas();
    const trs = [...raiz.querySelectorAll('tbody tr')].filter(tr => !/phx-grupo|phx-detalhe|phx-tr-vazia|phx-preview/.test(tr.className));
    trs.forEach((tr, i) => {
      const id = ls[i]?.id;
      if (id === undefined) return;
      if (tr.dataset.id !== id) tr.dataset.id = id;
      tr.classList.toggle('selecionada', id === selecionada);
      // A linha e o alvo da acao (abre o detalhe): quem usa leitor de tela ouve qual esta aberta.
      tr.setAttribute('aria-selected', String(id === selecionada));
    });
  }

  // Teclado: Enter ou Espaco na linha abrem o detalhe, como o clique. Sem isto nao havia
  // como APROVAR, RESPONDER ou CANCELAR sem mouse (relatorio de 01/10/2026, B1). A celula
  // focada vem do proprio phx-grid (setas); com o foco na grade e nenhuma celula ainda, vale
  // a celula marcada por ele, ou a primeira linha.
  lista.addEventListener('keydown', e => {
    if ((e.key !== 'Enter' && e.key !== ' ') || e.altKey || e.ctrlKey || e.metaKey) return;
    if (/^(INPUT|TEXTAREA|SELECT|BUTTON|A)$/.test(e.target.tagName)) return;
    const tr = e.target.closest('tbody tr[data-id]')
      || lista.querySelector('tbody .phx-cel-foco')?.closest('tr[data-id]')
      || (e.target.closest('.phx-envoltorio') ? lista.querySelector('tbody tr[data-id]') : null);
    if (!tr) return;
    e.preventDefault();
    if (tr.dataset.id === selecionada && !detalhe.hidden) return;
    selecionada = tr.dataset.id;
    atualizar();
  });

  function montarGrade(linhas) {
    grade = grades.criar(alvoGrade, {
      nome: 'tarefas',
      // Celular: um cartao por tarefa -- o objetivo em cima (duas linhas), estado e data.
      cartao: { titulo: 'objective', campos: ['status', 'created_at'] },
      cfg: () => ({
        chave: 'id', dados: linhas, navegador: true, agrupavel: false, pagina: { tamanho: 10, opcoes: [10, 25, 50] },
        condicoes: Object.entries(FORMA_ESTADO).map(([valor, forma]) => ({ campo: 'status', op: '=', valor, estilo: forma })),
        colunas: [
          { campo: 'status', titulo: txt('tarefas.col.estado', 'Estado'), tag: 'status', valores: ROTULOS_ESTADO() },
          { campo: 'objective', titulo: txt('tarefas.col.objetivo', 'Objetivo'), tag: 'objetivo', quebraLinha: true },
          // A data sai pelo idioma da tela (app.js, dataHora); a grade ordena pelo valor ISO.
          { campo: 'created_at', titulo: txt('tarefas.col.criada', 'Criada em'), tag: 'criada', tipo: 'dataHora', formato: v => dataHora(v) },
        ],
      }),
      inicial: g => g.ordenar('created_at', 'desc'),
      aoLog: (ev, g) => {
        if (!g || ev.ev !== 'phx.grid.registro' || !ev.n) return;
        const l = linhaDoRegistro(g, ev.n);
        if (l && l.id !== selecionada) { selecionada = l.id; atualizar(); }
      },
      depois: marcarLinhas,
    });
  }

  async function carregarLista() {
    const ts = await api('GET', 'tasks');
    const linhas = ts.map(t => ({ id: t.id, status: t.status, objective: t.objective, created_at: t.created_at }));
    vazio.textContent = txt('tarefas.nenhuma', 'Nenhuma tarefa ainda.');
    vazio.hidden = ts.length > 0;
    // Sem mudanca, a grade nao se refaz: refazer a cada sondagem desfaria a ordem, o filtro
    // e a pagina de quem esta olhando.
    const assinatura = JSON.stringify([linhas, idiomas.atual]);
    if (!grade) { await grades.carregar(); if (!grade) montarGrade(linhas); }
    else if (assinatura !== assinaturaLista) grade.g.substituirDados(linhas);
    else grade.aplicar();
    assinaturaLista = assinatura;
    aviso(txt('tarefas.resumo', '{n} tarefa(s) • atualizado agora', { n: ts.length }));
  }

  async function carregarDetalhe() {
    if (!selecionada) { detalhe.hidden = true; return; }
    const t = await api('GET', `tasks/${encodeURIComponent(selecionada)}`);
    const assinatura = JSON.stringify([t.id, t.status, t.steps?.length, t.question, t.answer, t.error, idiomas.atual]);
    if (assinatura === desenhado && !detalhe.hidden) return;
    desenhado = assinatura;
    // No celular o detalhe fica acima da lista: abrir OUTRA tarefa o traz a vista.
    const outra = detalhe.dataset.id !== t.id;
    detalhe.dataset.id = t.id;
    detalhe.hidden = false;
    if (outra && window.matchMedia('(max-width: 640px)').matches) requestAnimationFrame(() => detalhe.scrollIntoView({ block: 'start' }));
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
    // Primeira carga: diz que esta lendo em vez de deixar o status vazio (M8).
    if (!grade && !status.classList.contains('aviso')) aviso(txt('geral.lendo', 'Lendo…'));
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

  // O que a Visao geral reusa (SP000036 L2, «Execucoes recentes»): o MESMO cliente, o mesmo
  // formatador de estado e o mesmo detalhe -- abrir(id) escolhe a tarefa e leva a esta tela,
  // que a desenha como se a linha tivesse sido clicada aqui. Um segundo fetch ou um segundo
  // rotulo de estado la seria a decisao escrita duas vezes.
  window.tarefas = { api, estado, token, comHttp, abrir(id) { selecionada = id; mostrarTela('tarefas'); } };
  if (document.body.dataset.tela === 'tarefas') carregadores.tarefas();
  else if (document.body.dataset.tela === 'geral') desenharExecucoes();
})();

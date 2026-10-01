// Tela Configuracao: a interface grafica do config.json, pela mesma rota da CLI
// (`phxclaw config` e `/v1/config` chamam as MESMAS vista e definir no agente).
//
// A grade (phx-grid) e uma grade de propriedades: uma linha por chave, agrupada por secao,
// com o editor pelo tipo na coluna Valor. O que a pessoa digita fica em `pendentes` e so vai
// ao disco depois do diff, na propria pagina; a gravacao manda If-Match com a revisao lida.
//   * 409: outra gravacao passou na frente -- nada se perde: recarrega mantendo o digitado.
//   * 422: a celula da chave recusada fica marcada com o motivo que o agente deu.
//   * Segredo NUNCA tem campo de valor: a tela diz se esta guardado e como se guarda.
// Rotulo se traduz (fabrica); chave, valor, motivo e comando sao DADO do agente. A DESCRICAO
// da chave e rotulo (o catalogo a traz em cada idioma: descricao, descricao_en), e a tela a
// escolhe pelo idioma da fabrica -- em ingles ela saia em portugues (qualificacao, M14).
(() => {
  const $ = id => document.getElementById(id);
  const alvo = $('configConteudo');
  if (!alvo) return;
  const status = $('configStatus');
  const diffEl = $('configDiff');
  const conflitoEl = $('configConflito');
  const escopoEl = $('configEscopo');
  const buscaEl = $('configBusca');
  const soAlteradosEl = $('configSoAlterados');
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';
  const token = () => { try { return localStorage.getItem('phxclaw.token') || ''; } catch { return ''; } };

  let vista = null;          // o ultimo GET /v1/config
  let somenteCatalogo = false;
  const pendentes = new Map(); // chave -> valor novo (null = voltar ao padrao)
  const erros = new Map();     // chave -> motivo do 422
  let grade = null;

  const esc = v => String(v ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const igual = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  // Valor em texto, para a celula travada, o padrao e o diff: dado, sem traducao.
  const texto = v => (v === null || v === undefined ? '—' : Array.isArray(v) ? v.join(', ') : String(v));

  // O aviso guarda COMO se escreve (funcao), nao o texto: a fabrica pode chegar depois do
  // erro (achado exercitando: o erro de rede saia em portugues numa tela em ingles), e a
  // troca de idioma reescreve o aviso pela chave.
  let ultimoAviso = null;
  function aviso(t, ehErro, comTentar = false) {
    ultimoAviso = typeof t === 'function' ? [t, ehErro, comTentar] : null;
    status.textContent = typeof t === 'function' ? t() : t;
    status.classList.toggle('aviso', !!ehErro);
    status.setAttribute('role', ehErro ? 'alert' : 'status');
    mostrarTentar('config', comTentar);
  }
  // O mesmo par de mensagens da tela de Tarefas, pela mesma chave: rede caida diz o que
  // fazer; erro do agente traz o motivo dele (dado) e o TENTAR DE NOVO.
  function falhou(e) {
    if (e.rede) aviso(() => txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.'), true, true);
    else aviso(() => txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message }), true, true);
  }

  const ORIGENS = {
    ambiente: () => txt('config.origem.ambiente', 'ambiente'),
    projeto: () => txt('config.origem.projeto', 'projeto'),
    pasta: () => txt('config.origem.pasta', 'pasta'),
    padrao: () => txt('config.origem.padrao', 'padrão'),
  };
  const rotulosOrigem = () => Object.fromEntries(Object.entries(ORIGENS).map(([k, f]) => [k, f()]));

  let mapa = new Map();
  const porChave = () => mapa;
  function definirVista(v) {
    vista = v;
    mapa = new Map((v?.chaves || []).map(c => [c.chave, c]));
    // Projeto so se grava quando o agente achou um projeto confiado.
    const opProjeto = escopoEl.querySelector('option[value="projeto"]');
    opProjeto.disabled = !v?.arquivos?.projeto;
    if (opProjeto.disabled && escopoEl.value === 'projeto') escopoEl.value = 'pasta';
  }
  const valorNaTela = c => (pendentes.has(c.chave) ? (pendentes.get(c.chave) ?? c.padrao) : c.valor);

  // O editor de cada linha, pelo tipo. HTML montado aqui, com todo dado escapado. O nome
  // acessivel de cada editor e a CHAVE da linha (acao.bin): eram 143 campos sem nome para o
  // leitor de tela (qualificacao de 01/10/2026, G9). A chave e dado e entra como esta.
  function editor(c) {
    const ch = esc(c.chave);
    const erro = erros.has(c.chave) ? `<small class="cfg-motivo">${esc(erros.get(c.chave))}</small>` : '';
    if (c.segredo) {
      const presente = c.segredo_presente;
      const rot = presente ? txt('config.segredo.presente', 'guardado no cofre') : txt('config.segredo.ausente', 'ausente');
      const cmd = c.comando_do_segredo ? `<code class="cfg-comando">${esc(c.comando_do_segredo)}</code>` : '';
      return `<span class="cfg-segredo" data-presente="${presente ? 'sim' : 'nao'}">${esc(rot)}</span>${cmd}${erro}`;
    }
    const v = valorNaTela(c);
    if (!c.editavel || somenteCatalogo) {
      const motivo = c.motivo_nao_editavel ? `<small class="cfg-motivo-travado">${esc(c.motivo_nao_editavel)}</small>` : '';
      return `<span class="cfg-travado" data-cfg-travado="${ch}">${esc(texto(v))}</span>${motivo}${erro}`;
    }
    switch (c.tipo) {
      case 'booleano':
        return `<input type="checkbox" class="cfg-ed" data-cfg="${ch}" aria-label="${ch}"${v === true ? ' checked' : ''}>${erro}`;
      case 'enum': {
        const ops = (c.opcoes || []).map(o => `<option value="${esc(o)}"${o === v ? ' selected' : ''}>${esc(o)}</option>`).join('');
        const vazio = `<option value=""${v == null ? ' selected' : ''}>${esc(txt('config.enum_vazio', '(sem valor)'))}</option>`;
        return `<select class="cfg-ed" data-cfg="${ch}" aria-label="${ch}">${vazio}${ops}</select>${erro}`;
      }
      case 'lista': {
        const itens = (Array.isArray(v) ? v : []).map((t, i) => `<span class="cfg-tag">${esc(t)}<button type="button" class="cfg-tira" data-cfg-lista="${ch}" data-i="${i}" title="${esc(txt('grade.remover', 'remover'))}" aria-label="${esc(txt('config.tira_item', 'remover {item} de {chave}', { item: t, chave: c.chave }))}">×</button></span>`).join('');
        return `<span class="cfg-tags">${itens}<input type="text" class="cfg-tag-in" data-cfg-lista="${ch}" aria-label="${esc(txt('config.tag_rotulo', '{chave}: novo item', { chave: c.chave }))}" placeholder="${esc(txt('config.tag_novo', '+ item (Enter)'))}"></span>${erro}`;
      }
      case 'inteiro':
      case 'real':
        return `<input type="number" class="cfg-ed" data-cfg="${ch}" aria-label="${ch}" step="${c.tipo === 'inteiro' ? '1' : 'any'}" value="${v == null ? '' : esc(v)}">${erro}`;
      default:
        return `<input type="text" class="cfg-ed" data-cfg="${ch}" aria-label="${ch}" value="${v == null ? '' : esc(v)}">${erro}`;
    }
  }

  function botaoPadrao(c) {
    const temNoArquivo = c.origem === 'pasta' || c.origem === 'projeto';
    const jaPedido = pendentes.has(c.chave) && pendentes.get(c.chave) === null;
    if (!c.editavel || c.segredo || somenteCatalogo || !temNoArquivo || jaPedido) return '';
    // Rosa (marca): voltar ao padrao e um desfazer que se reverte, nao um excluir de vez
    // (qualificacao de 01/10/2026, M9) -- o vermelho fica para o que nao volta.
    return `<button type="button" class="acao marca cfg-volta" data-cfg-volta="${esc(c.chave)}">${esc(txt('config.voltar_padrao', 'VOLTAR AO PADRÃO'))}</button>`;
  }

  // «So alterados»: o que nao esta no padrao (veio de arquivo ou do ambiente) ou foi mexido
  // agora. O recorte e feito aqui, nos dados: uma coluna escondida para filtrar apareceria
  // no seletor de colunas.
  // Pelo CODIGO do idioma (campo descricao_<codigo>), nunca pela frase; sem o campo, o
  // portugues -- melhor nenhuma traducao do que uma inventada (o degrau da fabrica).
  const descricaoNoIdioma = c => (idiomas.atual !== 'pt' && c[`descricao_${idiomas.atual}`]) || c.descricao;

  function linhas() {
    const so = soAlteradosEl.checked;
    return (vista?.chaves || []).filter(c => !so || c.origem !== 'padrao' || pendentes.has(c.chave)).map(c => ({
      chave: c.chave, secao: c.secao, origem: c.origem, descricao: descricaoNoIdioma(c),
      padrao: c.segredo ? texto(null) : texto(c.padrao),
      // Texto do valor so para a busca e a exportacao; segredo nao entra nem assim.
      valor: c.segredo ? '' : texto(valorNaTela(c)),
      pendente: pendentes.has(c.chave) ? 'sim' : 'nao',
      erro: erros.has(c.chave) ? 'sim' : 'nao',
    }));
  }

  function montar() {
    grade = grades.criar(alvo, {
      nome: 'config',
      cfg: () => ({
        chave: 'chave', dados: linhas(), pagina: { tamanho: 1000, opcoes: [1000] }, totais: false,
        buscaveis: ['chave', 'secao', 'descricao', 'valor'],
        condicoes: [
          ...Object.keys(ORIGENS).map(o => ({ campo: 'origem', op: '=', valor: o, estilo: `origem-${o}` })),
          { campo: 'pendente', op: '=', valor: 'sim', estilo: 'cfg-pendente', alvo: 'linha' },
          { campo: 'erro', op: '=', valor: 'sim', estilo: 'cfg-erro', alvo: 'linha' },
        ],
        colunas: [
          { campo: 'secao', titulo: txt('config.col.secao', 'Seção'), tag: 'secao' },
          { campo: 'chave', titulo: txt('config.col.chave', 'Chave'), tag: 'chave' },
          { campo: 'valor', titulo: txt('config.col.valor', 'Valor'), tag: 'valor', filtravel: false, ordenavel: false,
            formato: (v, l) => editor(porChave().get(l.chave)) },
          { campo: 'origem', titulo: txt('config.col.origem', 'Origem'), tag: 'origem', valores: rotulosOrigem() },
          { campo: 'padrao', titulo: txt('config.col.padrao', 'Padrão'), tag: 'padrao' },
          { campo: 'descricao', titulo: txt('config.col.descricao', 'Descrição'), tag: 'descricao', quebraLinha: true },
          { campo: 'acao', titulo: txt('config.col.acao', 'Ação'), tag: 'acao', filtravel: false, ordenavel: false,
            formato: (v, l) => botaoPadrao(porChave().get(l.chave)) },
        ],
      }),
      inicial: g => g.agrupar(['secao']),
    });
  }

  // A grade so nasce depois de o phx-grid chegar (carrega sob demanda, grades.js).
  async function redesenhar() {
    if (!grade) {
      await grades.carregar();
      if (!grade) montar();
      return;
    }
    grade.g.substituirDados(linhas());
  }

  async function api(metodo, corpo, revisao) {
    const headers = { Authorization: `Bearer ${token()}` };
    if (corpo !== undefined) headers['Content-Type'] = 'application/json';
    if (revisao !== undefined) headers['If-Match'] = String(revisao);
    let r;
    try {
      r = await fetch('./v1/config', { method: metodo, headers, body: corpo === undefined ? undefined : JSON.stringify(corpo), cache: 'no-store' });
    } catch {
      throw Object.assign(new Error('sem rede'), { rede: true });
    }
    const v = await r.json().catch(() => ({}));
    return { status: r.status, ok: r.ok, corpo: v };
  }

  function resumo() {
    if (!vista) return;
    aviso(txt('config.status', 'revisão {revisao} • {n} chaves • {arquivo}',
      { revisao: vista.revisao, n: vista.chaves.length, arquivo: vista.arquivos?.pasta ?? '' }));
  }

  // Sem API (pagina do disco, ou sem token): o catalogo gerado, so leitura, para a tela
  // ainda dizer o que cada chave e -- nunca um valor inventado.
  async function catalogo(motivo) {
    try {
      const c = await fetch('./assets/config-catalogo.json', { cache: 'no-store' }).then(r => (r.ok ? r.json() : Promise.reject(new Error(r.status))));
      somenteCatalogo = true;
      definirVista({ revisao: null, arquivos: {}, chaves: c.chaves.map(k => ({ ...k, valor: null, origem: 'padrao', editavel: false, segredo_presente: false })) });
      await redesenhar();
    } catch { /* sem catalogo: so o aviso */ }
    aviso(motivo, true);
  }

  async function carregar() {
    if (!comHttp) return catalogo(() => txt('config.sem_api', 'A configuração só se edita com a tela servida pelo agente (phxclaw servir) ou pela ponte; abaixo, o catálogo.'));
    if (!token()) return catalogo(() => txt('config.pede_token', 'Informe o token de acesso (tela Tarefas) para ler e gravar a configuração; abaixo, o catálogo.'));
    try {
      const r = await api('GET');
      if (r.status === 401) return catalogo(() => txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'));
      // Pela ponte (controle remoto) a rota e recusada de proposito: remoto acompanha e
      // decide tarefa, nao administra o agente (crates/phxclaw-agent/src/remoto.rs).
      if (r.status === 403) return catalogo(() => txt('config.pela_ponte', 'O controle remoto não administra o agente: a configuração se edita na tela servida pelo próprio agente. Abaixo, o catálogo.'));
      if (!r.ok) throw Object.assign(new Error(r.corpo.error || `HTTP ${r.status}`), { status: r.status });
      somenteCatalogo = false;
      definirVista(r.corpo);
      // O que foi digitado e continua diferente do gravado fica; o resto sai.
      const m = porChave();
      for (const [k, v] of pendentes) if (!m.has(k) || igual(m.get(k).valor, v)) pendentes.delete(k);
      await redesenhar();
      resumo();
    } catch (e) {
      falhou(e);
    }
  }

  // --- edicao: delegada no alvo (a grade refaz o <tbody> a cada render) ---------------
  function marcar(chave, valor) {
    const c = porChave().get(chave);
    if (!c) return;
    erros.delete(chave);
    if (igual(valor, c.valor)) pendentes.delete(chave);
    else pendentes.set(chave, valor);
    redesenhar();
  }
  function lerEditor(e, c) {
    if (c.tipo === 'booleano') return e.checked;
    if (c.tipo === 'inteiro' || c.tipo === 'real') return e.value === '' ? null : Number(e.value);
    return e.value === '' ? null : e.value;
  }
  alvo.addEventListener('change', e => {
    const ed = e.target.closest('[data-cfg]');
    if (!ed) return;
    const c = porChave().get(ed.dataset.cfg);
    if (c) marcar(c.chave, lerEditor(ed, c));
  });
  alvo.addEventListener('keydown', e => {
    const inp = e.target.closest('.cfg-tag-in');
    if (!inp || e.key !== 'Enter') return;
    e.preventDefault();
    const t = inp.value.trim();
    const c = porChave().get(inp.dataset.cfgLista);
    if (!t || !c) return;
    const atual = valorNaTela(c);
    marcar(c.chave, [...(Array.isArray(atual) ? atual : []), t]);
  });
  alvo.addEventListener('click', e => {
    const tira = e.target.closest('.cfg-tira');
    if (tira) {
      const c = porChave().get(tira.dataset.cfgLista);
      const lista = [...(valorNaTela(c) || [])];
      lista.splice(Number(tira.dataset.i), 1);
      marcar(c.chave, lista);
      return;
    }
    const volta = e.target.closest('.cfg-volta');
    if (volta) { pendentes.set(volta.dataset.cfgVolta, null); erros.delete(volta.dataset.cfgVolta); redesenhar(); }
  });

  // --- diff e gravacao ------------------------------------------------------------------
  function mostrarDiff() {
    conflitoEl.hidden = true;
    const m = porChave();
    diffEl.replaceChildren();
    const h = document.createElement('h2');
    h.textContent = txt('config.diff.titulo', 'Alterações a gravar em {escopo}', { escopo: escopoEl.options[escopoEl.selectedIndex].text });
    diffEl.append(h);
    if (!pendentes.size) {
      const p = document.createElement('p');
      p.className = 'vazio';
      p.textContent = txt('config.diff.vazio', 'Nada alterado.');
      diffEl.append(p);
    } else {
      const t = document.createElement('table');
      t.className = 'cfg-diff';
      const cab = document.createElement('tr');
      // Uma chave literal por rotulo: o conferidor da fabrica le as chaves no fonte.
      for (const rotulo of [txt('config.col.chave', 'Chave'), txt('config.diff.de', 'de'), txt('config.diff.para', 'para')]) {
        const th = document.createElement('th');
        th.textContent = rotulo;
        cab.append(th);
      }
      t.append(cab);
      for (const [k, v] of pendentes) {
        const c = m.get(k);
        const tr = document.createElement('tr');
        tr.dataset.chave = k;
        const para = v === null ? txt('config.diff.padrao', 'padrão ({valor})', { valor: texto(c?.padrao) }) : texto(v);
        for (const [cl, tx] of [['chave', k], ['de', texto(c?.valor)], ['para', para]]) {
          const td = document.createElement('td');
          td.className = cl;
          td.textContent = tx;
          tr.append(td);
        }
        t.append(tr);
      }
      diffEl.append(t);
    }
    const acoes = document.createElement('div');
    acoes.className = 'acoes';
    const conf = document.createElement('button');
    conf.className = 'acao altera';
    conf.id = 'configConfirmar';
    conf.textContent = txt('config.diff.confirmar', 'CONFIRMAR E GRAVAR');
    conf.disabled = !pendentes.size;
    conf.onclick = gravar;
    const volta = document.createElement('button');
    volta.className = 'acao consulta';
    volta.textContent = txt('config.diff.voltar', 'CONTINUAR EDITANDO');
    volta.onclick = () => { diffEl.hidden = true; };
    acoes.append(conf, volta);
    diffEl.append(acoes);
    diffEl.hidden = false;
  }

  async function gravar() {
    const valores = Object.fromEntries(pendentes);
    const lida = vista.revisao;
    try {
      const r = await api('PUT', { escopo: escopoEl.value, valores }, lida);
      if (r.ok) {
        pendentes.clear();
        erros.clear();
        diffEl.hidden = true;
        await carregar();
        aviso(txt('config.gravado', 'Gravado: revisão {revisao}.', { revisao: r.corpo.revisao }));
        return;
      }
      if (r.status === 409) {
        diffEl.hidden = true;
        conflitoEl.replaceChildren();
        const p = document.createElement('p');
        p.textContent = txt('config.conflito', 'Conflito: a configuração foi gravada por outro (revisão {atual}) depois da revisão {lida} que esta tela leu. Nada foi gravado; o que você digitou continua aqui.',
          { atual: r.corpo.revisao_atual, lida });
        const b = document.createElement('button');
        b.className = 'acao consulta';
        b.id = 'configRecarregarMantendo';
        b.textContent = txt('config.recarregar_mantendo', 'RECARREGAR SEM PERDER O DIGITADO');
        b.onclick = async () => { conflitoEl.hidden = true; await carregar(); };
        conflitoEl.append(p, b);
        conflitoEl.hidden = false;
        return;
      }
      if (r.status === 422) {
        diffEl.hidden = true;
        erros.clear();
        for (const e of r.corpo.erros || []) erros.set(e.chave, e.motivo);
        redesenhar();
        aviso(txt('config.recusado', '{n} chave(s) recusada(s): veja as células marcadas.', { n: erros.size }), true);
        return;
      }
      if (r.status === 401) { aviso(txt('tarefas.sem_token', 'Token recusado: confira o token de acesso.'), true); return; }
      throw Object.assign(new Error(r.corpo.error || `HTTP ${r.status}`), { status: r.status });
    } catch (e) {
      falhou(e);
    }
  }

  $('configSalvar').addEventListener('click', mostrarDiff);
  $('configRecarregar').addEventListener('click', carregar);
  soAlteradosEl.addEventListener('change', redesenhar);
  let espera = null;
  buscaEl.addEventListener('input', () => { clearTimeout(espera); espera = setTimeout(() => grade?.g.buscar(buscaEl.value.trim()), 120); });

  carregadores.config = () => { if (!vista || somenteCatalogo) carregar(); };
  tentar.config = carregar;
  idiomas.aoTrocar(() => {
    if (ultimoAviso) aviso(...ultimoAviso);
    if (document.body.dataset.tela !== 'config') return;
    if (vista && !status.classList.contains('aviso')) resumo();
    if (!diffEl.hidden) mostrarDiff();
  });
  if (document.body.dataset.tela === 'config') carregadores.config();
})();

// Tres paineis que falam com a API do agente, do VS Code onda 2 (SP000031):
//   * explorador de testes (tela IDE): a arvore do `test_list` e o rodar de UM no (`test_run`);
//   * loja de plugins (tela Ferramentas): o catalogo assinado e o instalar;
//   * perfis (tela Configuracao): listar, usar e criar, pelo `/v1/config/perfis`.
// Os tres so existem pela rede (http/https): no arquivo local nao ha API, e a tela diz.
//
// O contrato das rotas e o do motor (crates/phxclaw-agent): perfis e o de config.rs
// (GET/PUT /v1/config/perfis, If-Match com a revisao do GET /v1/config); testes e plugins
// expõem o JSON das ferramentas test_list/test_run/plugin_catalog como ele e. Rotulo pela
// fabrica; nome de crate, modulo, teste, plugin, perfil e chave sao DADO e entram crus.
// Carregado async (fora do DOMContentLoaded do celular) e armado so no DOMContentLoaded,
// quando o app.js, o config.js e a fabrica de idiomas ja existem.
const iniciarPaineis = () => {
(() => {
  const $ = id => document.getElementById(id);
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';
  const token = () => { try { return localStorage.getItem('phxclaw.token') || ''; } catch { return ''; } };
  const esc = v => String(v ?? '');

  async function api(metodo, caminho, corpo, cabecalhos = {}) {
    let r;
    try {
      r = await fetch(`./v1/${caminho}`, {
        method: metodo,
        headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json', ...cabecalhos },
        body: corpo === undefined ? undefined : JSON.stringify(corpo),
        cache: 'no-store',
      });
    } catch {
      throw Object.assign(new Error('sem rede'), { rede: true });
    }
    const v = await r.json().catch(() => ({}));
    if (!r.ok) throw Object.assign(new Error(v.error || `HTTP ${r.status}`), { status: r.status, corpo: v });
    return v;
  }
  // O mesmo par de mensagens das outras telas; o motivo do agente e dado.
  function motivo(e) {
    if (e.status === 401) return txt('painel.sem_token', 'Token recusado: confira o token de acesso na tela Tarefas.');
    if (e.rede) return txt('erro.sem_rede', 'Sem conexão com o agente — confira a rede e toque em TENTAR DE NOVO.');
    return txt('erro.servidor', 'O agente respondeu com erro ({status}): {erro}. Toque em TENTAR DE NOVO; se repetir, veja o log do agente.', { status: e.status ?? '—', erro: e.message });
  }
  function aviso(alvo, t, ehErro = false) {
    alvo.textContent = typeof t === 'function' ? t() : t;
    alvo.classList.toggle('aviso', !!ehErro);
    alvo.setAttribute('role', ehErro ? 'alert' : 'status');
  }
  // A linha que a pessoa escolheu numa grade (clique ou navegador de registros), pelo
  // mesmo caminho da tela Tarefas: cada render refaz o <tbody>, e o id volta pela ordem.
  function selecao(grade, alvo, aoMudar) {
    let id = null;
    const marcar = (raiz, g) => {
      if (!g) return;
      const ls = g.linhas();
      const trs = [...raiz.querySelectorAll('tbody tr')].filter(tr => !/phx-grupo|phx-detalhe|phx-tr-vazia|phx-preview/.test(tr.className));
      trs.forEach((tr, i) => {
        const l = ls[i];
        if (!l) return;
        tr.dataset.id = l.id;
        tr.classList.toggle('selecionada', l.id === id);
        tr.setAttribute('aria-selected', String(l.id === id));
      });
    };
    alvo.addEventListener('click', e => {
      const tr = e.target.closest('tbody tr[data-id]');
      if (!tr || !alvo.contains(tr)) return;
      id = tr.dataset.id;
      aoMudar(id);
      grade.cur?.aplicar();
    });
    return { get id() { return id; }, limpar: () => { id = null; aoMudar(null); }, marcar };
  }
  function aoMostrar(tela, f) {
    const antes = carregadores[tela];
    carregadores[tela] = () => { antes?.(); f(); };
  }

  /* ===================== EXPLORADOR DE TESTES (IDE) ===================== */
  (() => {
    const painel = $('ideTestes');
    if (!painel) return;
    const arvore = $('testesArvore');
    const status = $('testesStatus');
    const saida = $('testesResultado');
    const botao = $('testesListar');
    let dados = null;
    let ultimo = null; // {no, passou, ok, falhou}

    function no(id, nome, tipo, filhos) {
      const li = el('li');
      const rot = el('span', 'no', nome);
      rot.dataset.tipo = tipo;
      rot.dataset.no = id;
      const b = el('button', '', txt('testes.rodar', 'RODAR'));
      b.type = 'button';
      b.dataset.rodar = id;
      b.setAttribute('aria-label', `${txt('testes.rodar', 'RODAR')} ${id}`);
      li.append(rot, b);
      if (ultimo && ultimo.no === id) li.append(el('span', ultimo.passou ? 'ok' : 'erro', ultimo.passou ? txt('testes.passou', 'PASSOU') : txt('testes.falhou', 'FALHOU')));
      if (filhos?.length) { const ul = el('ul'); ul.append(...filhos); li.append(ul); }
      return li;
    }
    function desenhar() {
      arvore.replaceChildren();
      if (!dados) return;
      const raiz = el('ul');
      if (dados.linguagem === 'rust') {
        for (const c of dados.crates || []) {
          raiz.append(no(c.crate, c.crate, 'crate', (c.modulos || []).map(m => {
            const idM = `${c.crate}/${m.modulo}`;
            return no(idM, m.modulo, 'modulo', (m.testes || []).map(t => no(`${idM}::${t}`, t, 'teste', [])));
          })));
        }
      } else {
        for (const a of dados.arquivos || []) raiz.append(no(a.arquivo, a.arquivo, 'arquivo', (a.testes || []).map(t => no(`${a.arquivo}::${t}`, t, 'teste', []))));
      }
      if (!raiz.children.length) raiz.append(el('li', 'no', txt('testes.vazio', 'Nenhum teste encontrado.')));
      arvore.append(raiz);
    }
    async function listar() {
      if (!comHttp) { aviso(status, () => txt('testes.sem_api', 'O explorador de testes fala com a API do agente; abra a tela pelo navegador.'), true); return; }
      aviso(status, () => txt('testes.lendo', 'Lendo a árvore de testes…'));
      try {
        dados = await api('GET', 'ide/testes');
        ultimo = null;
        desenhar();
        aviso(status, () => txt('testes.total', '{n} teste(s) em {projeto}', { n: dados.total ?? 0, projeto: dados.projeto ?? '.' }));
      } catch (e) { aviso(status, () => motivo(e), true); }
    }
    async function rodar(id) {
      aviso(status, () => txt('testes.rodando', 'Rodando {no}…', { no: id }));
      saida.hidden = true;
      try {
        const r = await api('POST', 'ide/testes/rodar', { no: id });
        ultimo = { no: id, passou: !!r.passou, ok: r.ok ?? 0, falhou: r.falhou ?? 0 };
        desenhar();
        aviso(status, () => txt('testes.resultado', '{no}: {ok} ok, {falhou} falhou', { no: id, ok: ultimo.ok, falhou: ultimo.falhou }), !r.passou);
        const linhas = [...(r.resultados || []), r.stderr_cauda || ''].filter(Boolean);
        saida.textContent = linhas.join('\n');
        saida.hidden = !linhas.length;
      } catch (e) { aviso(status, () => motivo(e), true); }
    }
    botao.addEventListener('click', listar);
    arvore.addEventListener('click', e => { const b = e.target.closest('button[data-rodar]'); if (b) rodar(b.dataset.rodar); });
    idiomas.aoTrocar(desenhar);
  })();

  /* ===================== LOJA DE PLUGINS (Ferramentas) ===================== */
  (() => {
    const painel = $('ferramentasPlugins');
    if (!painel) return;
    const alvo = $('pluginsConteudo');
    const status = $('pluginsStatus');
    const botaoCarregar = $('pluginsCarregar');
    const botaoInstalar = $('pluginsInstalar');
    const grade = { cur: null };
    let linhas = [];
    const sel = selecao(grade, alvo, id => { botaoInstalar.disabled = !id || !!linhas.find(l => l.id === id)?.instalado; });
    const ESTADOS = () => ({ instalado: txt('plugins.instalado', 'INSTALADO'), disponivel: txt('plugins.disponivel', 'DISPONÍVEL'), nao_assinado: txt('plugins.nao_assinado', 'SEM ASSINATURA') });
    const estadoDe = p => (p.instalado ? 'instalado' : p.assinado === false ? 'nao_assinado' : 'disponivel');
    function montar() {
      grade.cur = grades.criar(alvo, {
        nome: 'plugins',
        cartao: { titulo: 'nome', campos: ['versao', 'estado'] },
        cfg: () => ({
          chave: 'id', dados: linhas, navegador: true, agrupavel: false, pagina: { tamanho: 10, opcoes: [10, 25, 50] },
          condicoes: [{ campo: 'estado', op: '=', valor: 'instalado', estilo: 'st-ok' }, { campo: 'estado', op: '=', valor: 'nao_assinado', estilo: 'st-erro' }],
          colunas: [
            { campo: 'nome', titulo: txt('plugins.col.nome', 'Plugin'), tag: 'nome' },
            { campo: 'versao', titulo: txt('plugins.col.versao', 'Versão'), tag: 'versao' },
            { campo: 'estado', titulo: txt('plugins.col.estado', 'Estado'), tag: 'estado', valores: ESTADOS() },
            { campo: 'descricao', titulo: txt('plugins.col.descricao', 'Descrição'), tag: 'descricao', quebraLinha: true },
          ],
        }),
        depois: sel.marcar,
      });
    }
    async function carregar() {
      aviso(status, () => txt('geral.lendo', 'Lendo…'));
      try {
        const v = await api('GET', 'plugins/catalogo');
        const lista = v.plugins || [];
        // A ficha do motor (loja.rs): nome, versao, publicador, categorias, capacidades,
        // instalado. Tudo dado; a descricao e a composicao deles.
        linhas = lista.map(p => ({ id: esc(p.nome), nome: esc(p.nome), versao: esc(p.versao), descricao: [p.publicador, ...(p.categorias || []), ...(p.capacidades || [])].filter(Boolean).map(esc).join(' • '), instalado: !!p.instalado, assinado: p.assinado, estado: estadoDe(p) }));
        await grades.carregar();
        if (!grade.cur) montar(); else grade.cur.g.substituirDados(linhas);
        sel.limpar();
        aviso(status, () => (linhas.length ? txt('plugins.resumo', '{n} plugin(s) no catálogo', { n: linhas.length }) : txt('plugins.vazio', 'Catálogo vazio.')));
      } catch (e) { aviso(status, () => motivo(e), true); }
    }
    async function instalar() {
      const id = sel.id;
      if (!id) { aviso(status, () => txt('plugins.selecione', 'Selecione um plugin na grade.'), true); return; }
      aviso(status, () => txt('plugins.instalando', 'Instalando {id}…', { id }));
      try {
        await api('POST', 'plugins/instalar', { nome: id });
        aviso(status, () => txt('plugins.instalado_ok', 'Plugin {id} instalado.', { id }));
        await carregar();
      } catch (e) { aviso(status, () => motivo(e), true); }
    }
    botaoCarregar.addEventListener('click', carregar);
    botaoInstalar.addEventListener('click', instalar);
  })();

  /* ===================== PERFIS (Configuracao) ===================== */
  (() => {
    const painel = $('configPerfis');
    if (!painel) return;
    const alvo = $('perfisConteudo');
    const status = $('perfisStatus');
    const nome = $('perfisNome');
    const copiar = $('perfisCopiar');
    const grade = { cur: null };
    let linhas = [];
    let revisao = null;
    let ativo = null;
    const sel = selecao(grade, alvo, id => { $('perfisUsar').disabled = !id || id === ativo; });
    const ESTADOS = () => ({ ativo: txt('perfis.ativo', 'ATIVO'), inativo: txt('perfis.inativo', '—') });
    function montar() {
      grade.cur = grades.criar(alvo, {
        nome: 'perfis',
        cartao: { titulo: 'nome', campos: ['estado', 'chaves'] },
        cfg: () => ({
          chave: 'id', dados: linhas, navegador: false, agrupavel: false, pagina: { tamanho: 10, opcoes: [10, 25] },
          condicoes: [{ campo: 'estado', op: '=', valor: 'ativo', estilo: 'st-ok' }],
          colunas: [
            { campo: 'nome', titulo: txt('perfis.col.nome', 'Perfil'), tag: 'nome' },
            { campo: 'estado', titulo: txt('perfis.col.estado', 'Estado'), tag: 'estado', valores: ESTADOS() },
            { campo: 'chaves', titulo: txt('perfis.col.chaves', 'Chaves'), tag: 'chaves', quebraLinha: true },
          ],
        }),
        depois: sel.marcar,
      });
    }
    async function carregar() {
      if (!comHttp) return;
      aviso(status, () => txt('geral.lendo', 'Lendo…'));
      try {
        // A revisao vem do GET /v1/config (o If-Match do PUT); o bloco dos perfis vem junto.
        const v = await api('GET', 'config');
        revisao = v.revisao;
        // O bloco dos perfis vem na propria vista; agente sem o bloco e agente sem perfis.
        const p = v.perfis || { ativo: null, lista: [] };
        ativo = p.ativo ?? null;
        linhas = (p.lista || []).map(x => ({ id: esc(x.nome), nome: esc(x.nome), estado: x.ativo ? 'ativo' : 'inativo', chaves: Object.keys(x.chaves || {}).join(', ') }));
        await grades.carregar();
        if (!grade.cur) montar(); else grade.cur.g.substituirDados(linhas);
        sel.limpar();
        $('perfisDesativar').disabled = !ativo;
        aviso(status, () => (ativo ? txt('perfis.ativo_resumo', 'Perfil ativo: {nome} ({origem})', { nome: ativo, origem: p.origem_do_ativo ?? '—' })
          : linhas.length ? txt('perfis.nenhum_ativo', 'Nenhum perfil ativo.') : txt('perfis.vazio', 'Nenhum perfil ainda.')));
      } catch (e) { aviso(status, () => motivo(e), true); }
    }
    async function gravar(corpo) {
      try {
        const r = await api('PUT', 'config/perfis', corpo, { 'If-Match': String(revisao) });
        aviso(status, () => txt('perfis.gravado', 'Perfis gravados (revisão {revisao}).', { revisao: r.revisao ?? '—' }));
        await carregar();
      } catch (e) {
        if (e.status === 409) { await carregar(); aviso(status, () => txt('painel.conflito', 'Outra gravação passou na frente; recarregado — tente de novo.'), true); }
        else aviso(status, () => motivo(e), true);
      }
    }
    $('perfisUsar').addEventListener('click', () => { if (sel.id) gravar({ usar: sel.id }); else aviso(status, () => txt('perfis.selecione', 'Selecione um perfil na grade.'), true); });
    $('perfisDesativar').addEventListener('click', () => gravar({ usar: null }));
    $('perfisCriar').addEventListener('click', () => {
      const n = nome.value.trim();
      if (!n) { aviso(status, () => txt('perfis.nome_vazio', 'Dê um nome ao perfil.'), true); nome.focus(); return; }
      gravar({ criar: n, copiar_base: copiar.checked }).then(() => { nome.value = ''; });
    });
    aoMostrar('config', carregar);
  })();
})();
};
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', iniciarPaineis);
else iniciarPaineis();

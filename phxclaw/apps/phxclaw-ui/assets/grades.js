// Grades do Command Center: as listagens viram «table view» no molde do Janus GridEX e do
// DataGrid da DevExpress, pelo phx-grid (assets/vendor/phx-grid, do proprio dono, JS puro e
// sem dependencia). Os cartoes da Visao geral continuam cartoes: grade e para LISTAR.
//
// Tres regras deste arquivo, as tres pagas em outro lugar desta casa:
//   * Rotulo se traduz; dado, nunca. O titulo da coluna e o rotulo do valor substituido
//     (status, HUMANO/AGENTE) saem da fabrica pela CHAVE; nome de papel, capability, id e
//     descricao passam intactos.
//   * O phx-grid traz 15 textos pelo definirTextos e o resto escrito no fonte dele. O que
//     nao passa pelo definirTextos e reescrito aqui pela CLASSE do elemento (LOCAIS), que e
//     a chave estrutural -- nunca comparando a frase portuguesa, que quebraria calada no dia
//     em que o dono melhorar a redacao.
//   * A marca manda: o tema sai de VISUAL_MARCA pelo mesmo mapa do configurarVisual
//     (PhxGrid.visualParaTokens), uma definicao so para a grade e para o cubo.
const grades = (() => {
  const PG = window.PhxGrid;

  // Os 15 textos que o phx-grid ja le pelo definirTextos. Uma chave literal por texto: o
  // conferidor da fabrica le as chaves pedidas no fonte.
  function textosDoPhxGrid() {
    return {
      itensPorPagina: txt('grade.itens_por_pagina', 'itens por página'),
      arrasteAgrupar: txt('grade.arraste_agrupar', 'Arraste o cabeçalho de uma coluna para cá para agrupar'),
      linhas: txt('grade.linhas', 'linhas'),
      irPara: txt('grade.ir_para', 'ir para'),
      pagina: txt('grade.pagina', 'Página'),
      registro: txt('grade.registro', 'Registro'),
      totais: txt('grade.total', 'Total'),
      camposRelatorio: txt('grade.campos_relatorio', 'Campos do relatório'),
      buscar: txt('grade.buscar', 'Buscar…'),
      adiarLayout: txt('grade.adiar', 'Adiar atualização'),
      atualizar: txt('grade.atualizar', 'Atualizar'),
      de: txt('grade.de', 'de'),
      registros: txt('grade.registros', 'registros'),
      mostrando: txt('grade.mostrando', 'Mostrando'),
      colunas: txt('grade.colunas', 'Colunas'),
    };
  }

  // Agregador e IDENTIFICADOR do phx-grid (sum, count...); o rotulo dele e da fabrica.
  const AGREGADORES = {
    sum: () => txt('grade.agregador.sum', 'SOMA'),
    count: () => txt('grade.agregador.count', 'CONTAGEM'),
    avg: () => txt('grade.agregador.avg', 'MÉDIA'),
    min: () => txt('grade.agregador.min', 'MÍN'),
    max: () => txt('grade.agregador.max', 'MÁX'),
  };
  const rotuloAgregador = a => (AGREGADORES[a] ? AGREGADORES[a]() : a);

  // O que o phx-grid escreve cru no fonte dele, achado pela classe. Cada entrada:
  // [seletor, onde, texto(elemento, contexto)]. «onde»: atributo, 'texto' (o elemento so
  // tem texto) ou 'ultimoTexto' (o rotulo que vem depois de um <input> dentro do <label>).
  const LOCAIS = [
    ['.phx-gpill-x', 'title', () => txt('grade.desagrupar', 'desagrupar')],
    ['.phx-gpill-x-parte', 'title', () => txt('grade.tirar_composto', 'tirar do agrupamento composto')],
    ['.phx-gpill-seta', 'title', () => txt('grade.mesclar', 'mesclar com o próximo (agrupamento composto)')],
    ['.phx-fbtn', 'title', () => txt('grade.filtrar', 'filtrar')],
    ['.phx-th-agg', 'title', () => txt('grade.alternar_agregador', 'alternar o agregador')],
    ['.phx-th-agg', 'texto', (e, c) => rotuloAgregador(c.agregadorDe(e.closest('th')?.dataset.campo))],
    ['.phx-frow-in[type="number"]', 'placeholder', () => txt('grade.valor', 'valor')],
    ['.phx-frow-in[type="text"]', 'placeholder', () => txt('grade.buscar', 'Buscar…')],
    ['tfoot .phx-tfoot-agg', 'texto', (e, c) => rotuloAgregador(c.agregadorDoRodape(e.closest('td')))],
    // «N linhas em K nivel de grupo» / «1–7 de 7»: o phx-grid monta a frase crua com os
    // numeros dentro; a contagem certa ja esta no canto do rodape (grade.linhas), entao esta
    // fica vazia (e escondida no CSS) em vez de sair em portugues.
    ['.phx-mostrando', 'texto', () => ''],
    ['.phx-colsel [data-cs="todas"]', 'texto', () => txt('grade.colunas_todas', 'todas')],
    ['.phx-colsel [data-cs="minimo"]', 'texto', () => txt('grade.colunas_minimo', 'mínimo')],
    ['.phx-filtros-conta', 'texto', e => txt('grade.filtros_ativos', 'Filtros ativos ({n})', { n: e.parentNode.querySelectorAll('.phx-chip').length })],
    ['.phx-filtros-limpar', 'texto', () => txt('grade.limpar_todos', 'Limpar todos')],
    ['.phx-chip-x', 'title', () => txt('grade.remover', 'remover')],
    // O chip da busca global repete o termo, que e DADO de quem digitou: entra por marcador.
    ['.phx-chip[data-campo="*"]', 'primeiroTexto', (e, c) => txt('grade.chip_busca', 'Busca: «{termo}»', { termo: c.termoBusca() })],
    ['.phx-fpop-acao[data-a="az"]', 'texto', () => txt('grade.ordenar_az', 'Classificar de A a Z')],
    ['.phx-fpop-acao[data-a="za"]', 'texto', () => txt('grade.ordenar_za', 'Classificar de Z a A')],
    ['.phx-fpop-acao[data-a="limpar"]', 'texto', () => txt('grade.limpar_filtro', 'Limpar filtro')],
    ['.phx-fpop-busca', 'placeholder', () => txt('grade.pesquisar', 'Pesquisar')],
    ['.phx-fpop-tudo', 'ultimoTexto', () => txt('grade.selecionar_tudo', '(Selecionar tudo)')],
    ['.phx-fpop-nulos', 'ultimoTexto', () => txt('grade.sem_valor', 'Exibir itens sem valor')],
    ['.phx-fpop-ok', 'texto', () => txt('grade.ok', 'OK')],
    ['.phx-fpop-cancela', 'texto', () => txt('grade.cancelar', 'Cancelar')],
    // Cubo: rotulos das quatro zonas e a dica; o glifo de cada zona fica, o nome se traduz.
    ['.phx-cb-zona[data-zona="campos"] > .phx-cb-rot', 'texto', () => txt('grade.cubo.campos', 'Campos')],
    ['.phx-cb-zona[data-zona="filtros"] > .phx-cb-rot', 'texto', () => `▽ ${txt('grade.cubo.filtros', 'Filtros')}`],
    ['.phx-cb-zona[data-zona="linhas"] > .phx-cb-rot', 'texto', () => `≡ ${txt('grade.cubo.linhas', 'Linhas')}`],
    ['.phx-cb-zona[data-zona="colunas"] > .phx-cb-rot', 'texto', () => `‖ ${txt('grade.cubo.colunas', 'Colunas')}`],
    ['.phx-cb-zona[data-zona="valores"] > .phx-cb-rot', 'texto', () => `∑ ${txt('grade.cubo.valores', 'Valores')}`],
    ['.phx-cb-dica', 'texto', () => txt('grade.cubo.dica', 'arraste um campo para cá')],
    // O phx-grid marca campo de medida com a letra grega Σ; aqui vai o simbolo de somatorio
    // (∑), que e icone e nao letra -- o mesmo das zonas acima.
    ['.phx-cb-sig', 'texto', () => '∑'],
    ['.phx-cb-medida > i', 'texto', (e, c) => rotuloAgregador(c.agregadorDe(e.parentNode.dataset.ix))],
    ['.phx-cb-x', 'title', () => txt('grade.remover', 'remover')],
    ['.phx-cb-filtro', 'title', () => txt('grade.filtrar', 'filtrar')],
    ['.phx-cb-medida [title]', 'title', () => txt('grade.cubo.config_medida', 'configurar a medida')],
  ];

  function escreve(e, onde, valor) {
    if (onde === 'texto') {
      if (e.textContent !== valor) e.textContent = valor;
    } else if (onde === 'ultimoTexto' || onde === 'primeiroTexto') {
      const nos = [...e.childNodes].filter(n => n.nodeType === 3);
      const n = onde === 'ultimoTexto' ? nos[nos.length - 1] : nos[0];
      const alvo = onde === 'ultimoTexto' ? ` ${valor}` : `${valor} `;
      if (n && n.textContent !== alvo) n.textContent = alvo;
    } else if (e.getAttribute(onde) !== valor) e.setAttribute(onde, valor);
  }

  // Reaplica LOCAIS a cada render do phx-grid (ele refaz o DOM por innerHTML). O observador
  // se desliga enquanto escreve: escrever texto e mutacao, e sem isso giraria para sempre.
  function vigiar(raiz, contexto) {
    let agendado = false;
    const aplicar = () => {
      agendado = false;
      obs.disconnect();
      for (const [sel, onde, f] of LOCAIS) {
        for (const e of raiz.querySelectorAll(sel)) escreve(e, onde, f(e, contexto));
      }
      contexto.depois?.(raiz);
      obs.observe(raiz, { childList: true, subtree: true });
    };
    const obs = new MutationObserver(() => {
      if (!agendado) { agendado = true; queueMicrotask(aplicar); }
    });
    aplicar();
    return { aplicar, parar: () => obs.disconnect() };
  }

  // A marca (phxclaw/marca e CLAUDE.md): fundo #010418, Exo 2, contorno nas acoes. Chaves do
  // configurarVisual do phx-grid; o cubo recebe os MESMOS tokens por visualParaTokens.
  const VISUAL_MARCA = {
    fundo: '#010418', fundoAlt: '#071521', texto: '#eaf4ff', textoSuave: '#9bb3c5',
    borda: '#1d4053', linha: '#12293a', acento: '#36d7ff', acentoHover: '#5fe0ff', foco: '#36d7ff',
    ok: '#57e6a8', perigo: '#ff5e6c', aviso: '#ffc43d',
    fonte: '"Exo 2", Inter, ui-sans-serif, system-ui, sans-serif', tamanho: 13, raio: 8, raioControle: 6,
    cabecalho: { fundo: '#0b1b2a', texto: '#9fc3d8', negrito: true },
    corpo: { texto: '#dce9f5', zebra: '#050c1d', hover: '#0b2133', selecao: '#12304a', negativo: '#ff5e6c' },
    grade: { direcao: 'horizontal', corLinha: '#12293a' },
    agrupamento: { fundo: '#071521', texto: '#9bb3c5', borda: '#1d4053' },
    grupos: { fundo: '#0b1b2a', texto: '#f7c24a', hover: '#0f2638' },
    totais: { fundo: '#0b1b2a', texto: '#eaf4ff' },
    botoes: { fundo: 'transparent', texto: '#9fc3d8', borda: '#1d4053' },
    chips: { fundo: '#0b2133', borda: '#36d7ff' },
  };
  const tokensDaMarca = () => PG.visualParaTokens(VISUAL_MARCA);

  // Uma grade com troca de idioma: a troca guarda o layout (ordem, filtro, grupo, colunas
  // escondidas, pagina), refaz a grade com os titulos novos e devolve o layout -- o titulo
  // salvo no layout sai, senao o portugues voltaria por cima da traducao.
  function criar(alvo, def) {
    let g = null;
    let vig = null;
    const contexto = {
      agregadorDe: campo => (g ? g.layout().colunas.find(c => c.campo === campo)?.agregador : null),
      // No <tfoot> a primeira celula (o canto) cobre a primeira coluna visivel; da segunda em
      // diante, uma celula por coluna visivel, na mesma ordem.
      agregadorDoRodape: td => {
        const i = td ? [...td.parentNode.children].indexOf(td) : -1;
        return i > 0 && g ? contexto.agregadorDe(g.colunasVisiveis()[i]) : null;
      },
      termoBusca: () => (g ? g.filtros().find(f => f.campo === '*')?.termo ?? '' : ''),
      depois: def.depois ? r => def.depois(r, g) : null,
    };
    function montar(layout) {
      PG.definirTextos(textosDoPhxGrid());
      g = PG.criar(alvo, {
        tema: 'escuro', agrupavel: true, totais: true, dicas: false, ...def.cfg(),
        aoLog: ev => def.aoLog?.(ev, g),
      });
      if (!g || g.ok === false) throw new Error(`phx-grid: ${g?.erro}`);
      g.configurarVisual(VISUAL_MARCA);
      alvo.dataset.grade = def.nome;
      if (layout) {
        for (const c of layout.colunas) c.titulo = null;
        g.aplicarLayout(layout);
      } else def.inicial?.(g);
      vig = vigiar(g.el || alvo.querySelector('.phx-grid'), contexto);
    }
    function refazer() {
      if (!g) return;
      const layout = g.layout();
      const busca = contexto.termoBusca();
      vig?.parar();
      g.destruir();
      alvo.replaceChildren();
      montar(layout);
      if (busca) g.buscar(busca);
    }
    montar(null);
    idiomas.aoTrocar(refazer);
    return { get g() { return g; }, refazer, aplicar: () => vig?.aplicar() };
  }

  // O cubo (PivotTable) com a marca e os rotulos da fabrica. O menu ▾ de cada chip do cubo
  // tem duas dezenas de textos crus do phx-grid; aqui ele fica fora (CSS), e o cubo se
  // reorganiza arrastando chips entre as zonas e pelo TRANSPOR.
  function cubo(alvo, def) {
    let c = null;
    let vig = null;
    const contexto = {
      agregadorDe: ix => c?.layout().valores?.[Number(ix)]?.agregador,
      termoBusca: () => '',
    };
    function montar(layout) {
      PG.definirTextos(textosDoPhxGrid());
      alvo.replaceChildren();
      c = PG.cubo(alvo, { tema: 'escuro', painelCampos: false, ...def.cfg() });
      if (!c || c.ok === false) throw new Error(`phx-grid cubo: ${c?.erro}`);
      const raiz = alvo.querySelector('.phx-cubo');
      for (const [k, v] of Object.entries(tokensDaMarca())) if (k[0] !== '!') raiz.style.setProperty(k, v);
      if (layout) c.aplicarLayout(layout);
      vig = vigiar(raiz, contexto);
    }
    montar(null);
    idiomas.aoTrocar(() => { if (!c) return; const l = c.layout(); vig?.parar(); c.destruir?.(); montar(l); });
    return { get c() { return c; } };
  }

  // Valor de grupo com rotulo substituido (produto, estado): o rotulo vai num <span> proprio,
  // separado do titulo da coluna -- titulo e rotulo da tela, valor e dado, e cada um no seu no.
  const escapa = v => String(v ?? '').replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
  const valorDeGrupo = valores => ({
    valores: null,
    formato: v => `<span class="grade-valor-grupo">${escapa(Object.prototype.hasOwnProperty.call(valores, v) ? valores[v] : v)}</span>`,
  });

  // Exportar: o que a grade mostra (filtro, ordem, grupo), com o nome do arquivo dizendo a
  // tela; o dado vai como esta gravado (o codigo do status, nao o rotulo).
  function exportar(g, nome, formato) {
    if (formato === 'xlsx') g.baixarXLSX(`${nome}.xlsx`);
    else g.baixarCSV(`${nome}.csv`, { separador: ';' });
  }

  return { criar, cubo, exportar, valorDeGrupo, rotuloAgregador, VISUAL_MARCA };
})();

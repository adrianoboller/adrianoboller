// Tema da tela (Style Phoenix Padrao, docs/ui/STYLE_PHOENIX_PADRAO.md): o escuro da marca
// (#010418) ou o claro em papel quente (#f7f5f2), com os valores do console do PhxSql. A cor
// mora SO nos tokens de app.css (:root e :root[data-tema="claro"]); aqui so se escolhe qual
// bloco vale, pondo data-tema no <html>.
//
// Carrega no <head>, depois das folhas e antes do corpo: o primeiro quadro ja sai no tema
// certo, sem piscar o escuro para quem escolheu o claro. A CSP (script-src 'self') nao deixa
// isto ser um <script> em linha.
//
// Ordem de decisao: a escolha guardada (o botao do topo) -> o sistema (prefers-color-scheme)
// -> o escuro da marca. So o CLIQUE guarda: guardar o tema do sistema na primeira visita
// congelaria a escolha, e trocar o sistema depois nao mudaria mais nada.
const tema = (() => {
  const GUARDADO = 'phxclaw.tema';
  const raiz = document.documentElement;
  const ouvintes = [];
  const valido = t => (t === 'claro' || t === 'escuro' ? t : null);
  let escolhido = null;
  try { escolhido = valido(localStorage.getItem(GUARDADO)); } catch { /* sem localStorage: vale o sistema */ }
  const sistema = window.matchMedia ? window.matchMedia('(prefers-color-scheme: light)') : null;
  const doSistema = () => (sistema && sistema.matches ? 'claro' : 'escuro');
  let atual = escolhido || doSistema();

  // A barra do navegador (theme-color) acompanha o fundo: o valor sai do TOKEN, nao de um
  // hex digitado aqui.
  function pintar() {
    raiz.dataset.tema = atual;
    const fundo = getComputedStyle(raiz).getPropertyValue('--fundo').trim();
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta && fundo) meta.content = fundo;
  }

  // O botao diz para onde o clique LEVA (no escuro, «tema claro»), pela fabrica: o nome
  // muda a cada clique, por isso nao e um data-txt-al fixo.
  function rotular() {
    const b = document.getElementById('trocarTema');
    if (!b || typeof txt !== 'function') return;
    const nome = atual === 'claro'
      ? txt('tema.para_escuro', 'Mudar para o tema escuro')
      : txt('tema.para_claro', 'Mudar para o tema claro');
    b.setAttribute('aria-label', nome);
    b.title = nome;
  }

  function aplicar(t) {
    atual = valido(t) || 'escuro';
    pintar();
    rotular();
    for (const f of ouvintes) {
      try { f(atual); } catch (e) { console.error(e); }
    }
  }

  function trocar(t) {
    escolhido = valido(t);
    try { localStorage.setItem(GUARDADO, escolhido); } catch { /* fica so nesta visita */ }
    aplicar(escolhido);
  }

  sistema?.addEventListener?.('change', () => { if (!escolhido) aplicar(doSistema()); });
  pintar();

  document.addEventListener('DOMContentLoaded', () => {
    rotular();
    if (typeof idiomas !== 'undefined') idiomas.aoTrocar(rotular);
    document.getElementById('trocarTema')?.addEventListener('click', () => trocar(atual === 'claro' ? 'escuro' : 'claro'));
  });

  return { get atual() { return atual; }, trocar, aoTrocar: f => ouvintes.push(f) };
})();

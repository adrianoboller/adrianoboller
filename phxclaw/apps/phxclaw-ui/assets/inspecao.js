// Bloqueio do inspetor na tela (`ui.bloquear_inspecao`, pedido do dono de 09/10/2026): menu de
// contexto, F12, Ctrl/Cmd+Shift+I/J/C e Ctrl/Cmd+U (e as variantes Cmd+Option do Mac).
//
// O que isto E e o que NAO e, sem rodeio:
//   * No NAVEGADOR e dissuasao. O menu do proprio navegador, o view-source: na barra e
//     qualquer cliente HTTP alcancam a mesma pagina. A protecao de verdade mora no servidor:
//     CSP (pwa.rs), RBAC e segredo que nunca chega ao cliente.
//   * No DESKTOP (Tauri) e bloqueio: o build de release nao tem DevTools e nao ha barra de
//     endereco; o menu nativo do WebView so aparece se a pagina deixar, e aqui ela nao deixa.
//     Por isso no desktop vale SEMPRE, sem perguntar a politica.
//
// Nasce LIGADO e so desliga se o servidor disser `bloquear_inspecao: false`: a resposta da rede
// chega depois do primeiro quadro, e o padrao pedido pelo dono e o ligado.
//
// Acessibilidade: so se cancela a ACAO PADRAO (preventDefault), nunca a propagacao. Tab, leitor
// de tela, Ctrl+C e a selecao continuam intactos; o Ctrl+U continua chegando ao terminal do IDE
// (no bash ele apaga a linha) e o menu proprio do phx-grid continua abrindo. Em campo de edicao
// o menu nativo FICA: colar, desfazer e a correcao ortografica sao edicao, e quem so tem toque
// ou mouse nao tem outro caminho para eles. No desktop esse menu nao tem «Inspecionar».
const inspecao = (() => {
  const noDesktop = !!(window.__TAURI_INTERNALS__ || window.__TAURI__);
  let ligado = false;

  const editavel = alvo => !!(alvo && alvo.closest
    && alvo.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])'));

  // e.code primeiro (a tecla fisica, igual em qualquer layout); e.key cobre o teclado que nao
  // informa code.
  const tecla = e => (e.code && e.code.startsWith('Key') ? e.code.slice(3) : String(e.key || '').toUpperCase());

  function atalhoDoInspetor(e) {
    if (e.key === 'F12' || e.code === 'F12') return true;
    const t = tecla(e);
    const mod = e.ctrlKey || e.metaKey;
    // Ctrl+Shift+I/J/C (Windows, Linux) e Cmd+Shift+C; Cmd+Option+I/J/C no Mac.
    if (mod && (e.shiftKey || (e.metaKey && e.altKey)) && (t === 'I' || t === 'J' || t === 'C')) return true;
    // Ctrl+U e Cmd+Option+U: ver o codigo-fonte.
    if (mod && !e.shiftKey && t === 'U') return true;
    return false;
  }

  function aoMenu(e) {
    if (editavel(e.target)) return;
    e.preventDefault();
  }
  function aoTeclar(e) {
    if (atalhoDoInspetor(e)) e.preventDefault();
  }

  function ligar() {
    if (ligado) return;
    // Captura na janela: quem para a propagacao mais abaixo nao abre a porta.
    window.addEventListener('contextmenu', aoMenu, true);
    window.addEventListener('keydown', aoTeclar, true);
    ligado = true;
    document.documentElement.dataset.inspecao = 'bloqueada';
  }
  function desligar() {
    if (!ligado) return;
    window.removeEventListener('contextmenu', aoMenu, true);
    window.removeEventListener('keydown', aoTeclar, true);
    ligado = false;
    document.documentElement.dataset.inspecao = 'livre';
  }

  ligar();
  const comHttp = location.protocol === 'http:' || location.protocol === 'https:';
  // No desktop a politica nem e perguntada: e AQUI, num lugar so, que o desktop fica ligado.
  const pronto = noDesktop || !comHttp
    ? Promise.resolve()
    : fetch(`${location.pathname.replace(/[^/]*$/, '')}ui/politica`, { cache: 'no-store' })
      .then(r => (r.ok ? r.json() : null))
      .then(p => { if (p && p.bloquear_inspecao === false) desligar(); })
      // Sem resposta (rede caida, agente antigo): fica o padrao do dono, ligado.
      .catch(() => {});

  return { ligado: () => ligado, noDesktop, pronto };
})();

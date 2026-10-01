// Fabrica de idiomas da tela. Todo rotulo sai de assets/textos.json por CHAVE; o texto em
// portugues escrito no fonte e so o padrao que aparece antes de o pacote chegar.
//
// Tres degraus: o idioma escolhido -> o portugues da fabrica -> o padrao do fonte. Celula
// vazia cai para o portugues: melhor nenhuma traducao do que uma inventada.
//
// Rotulo se traduz; DADO nunca. O que vem dos JSON gerados (nome de papel, descricao de
// ferramenta, caminho, versao) entra pelos {marcadores} de txt() e passa intacto.
//
// No HTML: data-txt (texto do elemento, que tem de ser so texto), data-txt-ph
// (placeholder), data-txt-al (aria-label), data-txt-tt (title). No JS: txt(chave, padrao, vars).
// Quem desenha texto no JS registra um redesenho em idiomas.aoTrocar().
const idiomas = (() => {
  const GUARDADO = 'phxclaw.idioma';
  const ATRIBUTOS = [['txtPh', 'placeholder'], ['txtAl', 'aria-label'], ['txtTt', 'title']];
  let fabrica = { idiomas: ['pt'], textos: {} };
  let atual = 'pt';
  try {
    atual = new URLSearchParams(location.search).get('idioma') || localStorage.getItem(GUARDADO) || 'pt';
  } catch { /* sem localStorage (arquivo local): fica no portugues */ }
  const ouvintes = [];

  function txt(chave, padrao, vars) {
    const e = fabrica.textos[chave];
    let s = (e && (e[atual] || e.pt)) || padrao;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
    return s;
  }

  // O padrao de cada elemento e lido UMA vez, antes da primeira troca: depois o texto da
  // tela ja e traducao, e voltar ao portugues sem fabrica precisa do original.
  function aplicar(raiz = document) {
    for (const e of raiz.querySelectorAll('[data-txt]')) {
      e.__padrao ??= e.textContent;
      e.textContent = txt(e.dataset.txt, e.__padrao);
    }
    for (const [campo, atributo] of ATRIBUTOS) {
      for (const e of raiz.querySelectorAll(`[data-${campo.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)}]`)) {
        e.__padroes ??= {};
        e.__padroes[atributo] ??= e.getAttribute(atributo) ?? '';
        e.setAttribute(atributo, txt(e.dataset[campo], e.__padroes[atributo]));
      }
    }
    document.documentElement.lang = atual === 'pt' ? 'pt-BR' : atual;
    const botao = document.getElementById('trocarIdioma');
    if (botao) botao.textContent = atual.toUpperCase();
  }

  function avisar() {
    aplicar();
    for (const f of ouvintes) {
      try { f(atual); } catch (e) { console.error(e); }
    }
  }

  function trocar(codigo) {
    atual = fabrica.idiomas.includes(codigo) ? codigo : 'pt';
    try { localStorage.setItem(GUARDADO, atual); } catch { /* idem */ }
    avisar();
  }

  const pronto = fetch('./assets/textos.json', { cache: 'no-store' })
    .then(r => (r.ok ? r.json() : Promise.reject(new Error(`textos.json: HTTP ${r.status}`))))
    .then(f => {
      fabrica = f;
      if (!fabrica.idiomas.includes(atual)) atual = 'pt';
    })
    .catch(e => {
      // Sem fabrica a tela fica no padrao do fonte, que e portugues: dizer outro idioma no
      // <html lang> e no botao seria mentir sobre o que esta escrito.
      atual = 'pt';
      console.warn('fabrica de idiomas ausente; a tela fica no padrao do fonte', e);
    })
    .finally(avisar);

  document.addEventListener('DOMContentLoaded', () => {
    aplicar();
    document.getElementById('trocarIdioma')?.addEventListener('click', () => {
      const lista = fabrica.idiomas;
      trocar(lista[(lista.indexOf(atual) + 1) % lista.length]);
    });
  });

  return { txt, aplicar, trocar, pronto, aoTrocar: f => ouvintes.push(f), get atual() { return atual; } };
})();
const txt = idiomas.txt;

import DADOS from '../dados.json';
const T = {titulo:"Clientes",filtro:"Filtrar",nome:"Nome",status:"Situacao",email:"E-mail",fone:"Telefone",cidade:"Cidade",uf:"UF",doc:"Documento",salvar:"Salvar",obrigatorio:"Nome e obrigatorio",salvo:"Salvo"};
const t = k => T[k] ?? "?";
const el = (tag, at = {}, ...f) => { const e = document.createElement(tag); for (const [k, v] of Object.entries(at)) k.startsWith('on') ? e.addEventListener(k.slice(2), v) : e.setAttribute(k, v); e.append(...f); return e; };
const tbody = el('tbody');
const desenha = f => tbody.replaceChildren(...DADOS.filter(x => x.nome.includes(f)).map(x => el('tr', {}, el('td', {}, String(x.id)), el('td', {}, x.nome), el('td', {}, x.status))));
const msg = el('p', { role: 'status' }); const nome = el('input');
document.body.append(
  el('h1', {}, t('titulo')),
  el('input', { 'aria-label': t('filtro'), oninput: e => desenha(e.target.value) }),
  el('table', {}, el('thead', {}, el('tr', {}, el('th', {}, 'ID'), el('th', {}, t('nome')), el('th', {}, t('status')))), tbody),
  el('form', { onsubmit: e => { e.preventDefault(); msg.textContent = nome.value.trim() ? t('salvo') : t('obrigatorio'); } },
    el('label', {}, t('nome'), nome), ...['email','fone','cidade','uf','doc'].map(k => el('label', {}, t(k), el('input'))),
    el('button', { type: 'submit' }, t('salvar')), msg));
desenha('');

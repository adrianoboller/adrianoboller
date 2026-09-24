"use strict";
/* =====================================================================
   PhxZip -- a tela.

   O contrato que esta pagina consome esta em `docs/PHXZIP-WEB.md`, e e ele
   que manda: rota, envelope, nome de erro e limite saem de la.

   Tres regras atravessam o arquivo inteiro, e cada uma ja custou caro na tela
   do PhxSql antes de virar regra:

   1. NENHUM TEXTO CRAVADO. Todo rotulo sai por chave (`t("zip.…")`, ou os
      `data-txt*` do HTML) e o servidor devolve o texto ja resolvido no idioma.
      Texto se resolve por CHAVE, nunca por comparacao da frase.
   2. DADO NUNCA PASSA PELA FABRICA, NEM SE ESTILIZA. Nome de arquivo, tamanho,
      data e nome de metodo entram por `textContent`, dentro de um `.dado`, e o
      CSS nao tem `text-transform` que os alcance. `innerHTML` so recebe icone
      CONSTANTE deste arquivo -- nunca nada que venha do pacote.
   3. O ESTADO DESENHA A TELA. Trocar de idioma tem de repintar tudo sem
      recarregar, e isso so e possivel se cada pedaco da tela souber se
      redesenhar a partir de `est`. Mensagem guardada como texto pronto ficaria
      no idioma em que nasceu.
   ===================================================================== */

// ------------------------------------------------------------- o basico
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

/** Cria um elemento. Texto entra como NO DE TEXTO -- `append(string)` nunca
    interpreta marcacao --, e e por isso que um nome de entrada como
    `<img src=x onerror=…>` aparece escrito em vez de rodar. */
function h(tag, props, ...filhos) {
  const el = document.createElement(tag);
  if (props) {
    for (const [k, v] of Object.entries(props)) {
      if (v == null || v === false) continue;
      if (k === "class") el.className = v;
      else if (k === "texto") el.textContent = v;
      else if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2), v);
      else el.setAttribute(k, v === true ? "" : String(v));
    }
  }
  for (const f of filhos.flat()) {
    if (f == null || f === false) continue;
    el.append(f instanceof Node ? f : String(f));
  }
  return el;
}

// ------------------------------------------------------------- os icones
/* SVG a mao, e nao emoji: emoji nao desenha igual no Windows (a tela do
   PhxSql aprendeu isso com as bandeiras). Todos com `currentColor`, para o
   icone seguir a cor da acao e do tema sem uma regra por icone. */
const SVG = (d, extra = "") =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"${extra}>${d}</svg>`;
const ICONES = {
  compactar: SVG('<path d="M4 7h16v13H4z"/><path d="M3 3h18v4H3z"/><path d="M10 11h4"/><path d="M12 11v6"/><path d="M10 15l2 2 2-2"/>'),
  abrir: SVG('<path d="M3 7h6l2 2h10v10H3z"/><path d="M12 17v-5"/><path d="M9.5 14.5 12 12l2.5 2.5"/>'),
  soltar: SVG('<path d="M12 3v12"/><path d="m7 10 5 5 5-5"/><path d="M4 17v3h16v-3"/>'),
  arquivo: SVG('<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>'),
  pasta: SVG('<path d="M3 6h6l2 2h10v11H3z"/>'),
  cadeado: SVG('<rect x="5" y="10" width="14" height="10" rx="1.5"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>'),
  olho: SVG('<path d="M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6S2 12 2 12z"/><circle cx="12" cy="12" r="2.6"/>'),
  baixar: SVG('<path d="M12 4v11"/><path d="m7 10 5 5 5-5"/><path d="M5 20h14"/>'),
  remover: SVG('<path d="M6 6l12 12"/><path d="M18 6 6 18"/>'),
  lixeira: SVG('<path d="M4 7h16"/><path d="M9 7V4h6v3"/><path d="M6 7l1 13h10l1-13"/>'),
  testar: SVG('<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="m8.5 12 2.5 2.5 4.5-5"/>'),
  fechar: SVG('<path d="M6 6l12 12"/><path d="M18 6 6 18"/>'),
  copiar: SVG('<rect x="8" y="8" width="12" height="12" rx="1.5"/><path d="M16 8V4H4v12h4"/>'),
  sol: SVG('<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>'),
  lua: SVG('<path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/>'),
  ok: SVG('<circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/>'),
  // Uma forma por familia de erro: o estado tem de se ler SEM a cor.
  senha: SVG('<rect x="5" y="10" width="14" height="10" rx="1.5"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/><path d="M12 14v2.5"/>'),
  corrompido: SVG('<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/><path d="m9 11 2 2-2 2 2 2"/><path d="m15 11-2 2 2 2-2 2"/>'),
  legado: SVG('<circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/>'),
  perigo: SVG('<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="M12 8v5"/><path d="M12 16.5v.01"/>'),
  tamanho: SVG('<path d="M3 20h18"/><path d="M6 20V9"/><path d="M12 20V4"/><path d="M18 20v-7"/><path d="M9 6l3-3 3 3"/>'),
  pedido: SVG('<circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><path d="M12 7.5v.01"/>'),
  servidor: SVG('<rect x="4" y="4" width="16" height="7" rx="1"/><rect x="4" y="13" width="16" height="7" rx="1"/><path d="M8 7.5h.01M8 16.5h.01"/><path d="m15 15 3 3M18 15l-3 3"/>'),
  cancelado: SVG('<circle cx="12" cy="12" r="9"/><path d="M9 9h6v6H9z"/>'),
  json_ok: SVG('<path d="M8 4c-2 0-2 2-2 4s-2 4-2 4 2 0 2 4 0 4 2 4"/><path d="M16 4c2 0 2 2 2 4s2 4 2 4-2 0-2 4 0 4-2 4"/><path d="m9.5 12 2 2 3-4"/>'),
  json_erro: SVG('<path d="M8 4c-2 0-2 2-2 4s-2 4-2 4 2 0 2 4 0 4 2 4"/><path d="M16 4c2 0 2 2 2 4s2 4 2 4-2 0-2 4 0 4-2 4"/><path d="m10 10 4 4M14 10l-4 4"/>'),
  corte: SVG('<path d="M4 6h16M4 10h16M4 14h9"/><path d="m15 17 2 2 4-4"/>'),
  binario: SVG('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7.5 9v6M10.5 9h2v6h-2zM15.5 9v6"/>'),
  idioma: SVG('<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3.5 3 14.5 0 18M12 3c-3 3.5-3 14.5 0 18"/>'),
};

/** Um icone, sempre decorativo: quem le a tela ouve o texto ao lado dele. */
function icone(nome, classe = "ic") {
  const s = document.createElement("span");
  s.className = classe;
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = ICONES[nome] || "";
  return s;
}

// ------------------------------------------------------------- o estado
const est = {
  textos: {},          // chave -> texto, JA resolvido pelo servidor
  idioma: "Portugues", // a coluna, com o nome que o servidor usa
  estado: null,        // a resposta de /api/estado: limites, niveis, formatos
  // Compactar
  itens: [],           // {nome, pasta, tamanho, modificado, arquivo}
  formato: "7z",
  nivel: null,
  nomeEditado: false,
  recadoCompactar: null,   // {n}: quantos repetidos ficaram de fora
  msgCompactar: null,      // {tipo:"erro", erro} | {tipo:"ok", nome, tamanho, original}
  ultimoPacote: null,      // {url, nome} -- para «baixar de novo»
  // Abrir
  pacote: null,        // {arquivo, nome, tamanho}
  pedirSenha: null,    // null | chave da dica
  lista: null,         // a resposta de /api/listar
  msgAbrir: null,      // {tipo:"erro", erro}
  msgTeste: null,      // {tipo:"ok", r} | {tipo:"erro", erro}
  recadoAbrir: null,
  filtro: "",
  // Espiar
  espiado: null,
  recadoEspiar: null,
  // Operacao em curso
  xhr: null,
  ocupado: false,
};

// ------------------------------------------------------------- os textos
/** Um texto de tela. Chave que o servidor nao conhece aparece COMO CHAVE, de
    proposito: um rotulo em branco esconderia o defeito, a chave o denuncia. */
function t(chave, dados) {
  const bruto = est.textos[chave];
  if (bruto == null) return chave;
  return String(bruto).replace(/\{(\w+)\}/g,
    (m, k) => (dados && k in dados && dados[k] != null) ? String(dados[k]) : m);
}

/** Um texto de tela com DADO dentro, devolvido como fragmento.
 *
 *  Marcador por nome (`{nome}`), e nao `+` no meio da frase: a ordem das
 *  palavras muda de lingua para lingua. E o dado entra no seu proprio
 *  `<span class="dado">` -- nunca colado no texto do rotulo --, porque e ali
 *  que a regra «dado nunca se estiliza» consegue ser conferida: o roteiro
 *  compara o `innerText` com o `textContent` de todo `.dado` da tela.
 *
 *  Valor que ja e um `Node` entra como esta (um nome com a marca dos
 *  caracteres invisiveis, um plural que e rotulo e nao dado). */
function frase(chave, dados) {
  const bruto = est.textos[chave] == null ? chave : String(est.textos[chave]);
  const frag = document.createDocumentFragment();
  let ultimo = 0;
  bruto.replace(/\{(\w+)\}/g, (m, k, pos) => {
    if (pos > ultimo) frag.append(bruto.slice(ultimo, pos));
    const v = dados ? dados[k] : undefined;
    if (v instanceof Node) frag.append(v);
    else if (v != null) frag.append(h("span", { class: "dado" }, String(v)));
    else frag.append(m);
    ultimo = pos + m.length;
    return m;
  });
  if (ultimo < bruto.length) frag.append(bruto.slice(ultimo));
  return frag;
}

/** Plural pelas regras do idioma (`Intl.PluralRules`), com as DUAS chaves
    escritas por extenso: chave montada por concatenacao nao se acha por
    busca, e o laco «chave morta × chave faltando» deixaria de enxerga-la. */
function plural(n, um, varios, zero) {
  // O zero tem chave propria quando existe: o CLDR poe o 0 no singular em
  // portugues («0 pasta»), o que e certo em frances e soa errado aqui.
  // «nenhuma pasta» nao depende de regra de plural nenhuma.
  if (n === 0 && zero) return t(zero);
  const cat = new Intl.PluralRules(langAtual()).select(n);
  return t(cat === "one" ? um : varios, { n: fmtInt(n) });
}

const ATRIBUTOS_DE_TEXTO = [
  ["data-txt", (el, s) => { el.textContent = s; }],
  ["data-txt-ph", (el, s) => { el.placeholder = s; }],
  ["data-txt-tt", (el, s) => { el.title = s; }],
  ["data-txt-al", (el, s) => { el.setAttribute("aria-label", s); }],
];

function aplicarTextos(raiz = document) {
  for (const [attr, aplicar] of ATRIBUTOS_DE_TEXTO)
    for (const el of raiz.querySelectorAll(`[${attr}]`)) aplicar(el, t(el.getAttribute(attr)));
}

// ------------------------------------------------------------- os idiomas
/* A LISTA dos idiomas vem do servidor (`/api/estado`, que a tira do
   `IDIOMAS` do motor). Aqui mora so a APRESENTACAO de cada um -- o nome no
   proprio idioma, o codigo BCP 47 e a bandeira --, a mesma da tela do
   PhxSql (`crates/phxsql-server/ui/index.html`, «O IDIOMA DO AMBIENTE"). Uma
   coluna nova que o servidor mande sem par aqui aparece com o nome da coluna
   e sem bandeira: feio, mas nunca some. */
const APRESENTACAO = {
  Portugues: { nome: "Português", lang: "pt-BR", bandeira: "br" },
  Frances:   { nome: "Français",  lang: "fr",    bandeira: "fr" },
  Ingles:    { nome: "English",   lang: "en",    bandeira: "gb" },
  Italiano:  { nome: "Italiano",  lang: "it",    bandeira: "it" },
  Alemao:    { nome: "Deutsch",   lang: "de",    bandeira: "de" },
  Espanhol:  { nome: "Español",   lang: "es",    bandeira: "es" },
};
/* As bandeiras sao as da tela do PhxSql, copiadas. Copia declarada: quando a
   paleta e as bandeiras sairem para um arquivo comum da marca, esta some. */
const BANDEIRAS = {
  br: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="16" fill="#009b3a"/><path d="M12 1.7 22.3 8 12 14.3 1.7 8Z" fill="#fedf00"/><circle cx="12" cy="8" r="3.9" fill="#002776"/></svg>`,
  fr: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="8" height="16" fill="#002395"/><rect x="8" width="8" height="16" fill="#fff"/><rect x="16" width="8" height="16" fill="#ed2939"/></svg>`,
  gb: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="16" fill="#012169"/><path d="M0 0 24 16M24 0 0 16" stroke="#fff" stroke-width="3.2"/><path d="M0 0 24 16M24 0 0 16" stroke="#c8102e" stroke-width="1.7"/><path d="M12 0V16M0 8H24" stroke="#fff" stroke-width="5.4"/><path d="M12 0V16M0 8H24" stroke="#c8102e" stroke-width="3.2"/></svg>`,
  it: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="8" height="16" fill="#008c45"/><rect x="8" width="8" height="16" fill="#f4f5f0"/><rect x="16" width="8" height="16" fill="#cd212a"/></svg>`,
  de: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="5.34" fill="#000"/><rect y="5.34" width="24" height="5.33" fill="#dd0000"/><rect y="10.67" width="24" height="5.33" fill="#ffce00"/></svg>`,
  es: `<svg viewBox="0 0 24 16" aria-hidden="true"><rect width="24" height="16" fill="#aa151b"/><rect y="4" width="24" height="8" fill="#f1bf00"/></svg>`,
};
const CHAVE_IDIOMA = "phxzip-idioma";
const CHAVE_TEMA = "phxzip-tema";

function lembrado(k) { try { return localStorage.getItem(k); } catch { return null; } }
function lembrar(k, v) { try { localStorage.setItem(k, v); } catch { /* navegador sem armazenamento: vale so nesta aba */ } }

function langAtual() { return (APRESENTACAO[est.idioma] || APRESENTACAO.Portugues).lang; }

function idiomaInicial(lista) {
  const g = lembrado(CHAVE_IDIOMA);
  if (g && lista.includes(g)) return g;
  // Sem escolha guardada, o idioma do navegador -- pelo prefixo, porque
  // «pt-PT» e «pt-BR» caem na mesma coluna.
  for (const l of navigator.languages || [navigator.language || ""]) {
    const pre = String(l).slice(0, 2).toLowerCase();
    const col = lista.find(c => (APRESENTACAO[c] || {}).lang && APRESENTACAO[c].lang.slice(0, 2) === pre);
    if (col) return col;
  }
  return lista.includes("Portugues") ? "Portugues" : lista[0];
}

async function carregarIdioma(col) {
  const r = await fetch("/api/idiomas?idioma=" + encodeURIComponent(col), { cache: "no-store" });
  const j = await r.json();
  if (!j || !j.ok || !j.textos) throw new Error("idiomas");
  est.textos = j.textos;
  est.idioma = j.idioma;
  document.documentElement.lang = langAtual();
}

async function escolherIdioma(col) {
  lembrar(CHAVE_IDIOMA, col);
  try { await carregarIdioma(col); } catch { return; }
  aplicarTextos();
  repintar();
}

function desenharIdiomas() {
  const lista = (est.estado && est.estado.idiomas) || [est.idioma];
  const ap = APRESENTACAO[est.idioma] || { nome: est.idioma };
  const atual = $("#bandeiraAtual");
  atual.innerHTML = BANDEIRAS[ap.bandeira] || "";
  $("#nomeIdioma").textContent = ap.nome;
  const menu = $("#menuIdiomas");
  menu.replaceChildren(...lista.map(col => {
    const a = APRESENTACAO[col] || { nome: col };
    const li = h("li", {
      role: "option", tabindex: "-1", "data-idi": col,
      "aria-selected": String(col === est.idioma), lang: a.lang || null,
    });
    const b = h("span", { class: "bandeira" });
    b.innerHTML = BANDEIRAS[a.bandeira] || "";
    li.append(b, h("span", { class: "endonimo" }, a.nome));
    li.addEventListener("click", () => { fecharMenuIdiomas(); escolherIdioma(col); });
    return li;
  }));
}

function abrirMenuIdiomas() {
  const menu = $("#menuIdiomas");
  menu.hidden = false;
  $("#btIdioma").setAttribute("aria-expanded", "true");
  const sel = menu.querySelector('[aria-selected="true"]') || menu.firstElementChild;
  if (sel) sel.focus();
}
function fecharMenuIdiomas(devolverFoco) {
  $("#menuIdiomas").hidden = true;
  $("#btIdioma").setAttribute("aria-expanded", "false");
  if (devolverFoco) $("#btIdioma").focus();
}

// ------------------------------------------------------------- o tema
function aplicarTema(qual) {
  const claro = qual === "claro";
  document.documentElement.setAttribute("data-tema", claro ? "claro" : "escuro");
  const b = $("#btTema");
  // O botao diz para onde o clique LEVA, e por isso muda a cada clique.
  const chave = claro ? "zip.tema_para_escuro" : "zip.tema_para_claro";
  b.replaceChildren(icone(claro ? "lua" : "sol"));
  b.setAttribute("aria-label", t(chave));
  b.title = t(chave);
}
function trocarTema() {
  const novo = document.documentElement.getAttribute("data-tema") === "claro" ? "escuro" : "claro";
  lembrar(CHAVE_TEMA, novo);
  aplicarTema(novo);
}

// ------------------------------------------------------------- formatar
function fmtInt(n) { return new Intl.NumberFormat(langAtual()).format(n); }

/** Tamanho legivel. A unidade e binaria (KiB = 1024) e diz isso no nome; o
    numero exato em bytes vai no `title` de quem chama. */
function fmtTam(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  if (n < 1024) return fmtInt(n) + " B";
  const un = ["KiB", "MiB", "GiB", "TiB"];
  let v = n / 1024, i = 0;
  while (v >= 1024 && i < un.length - 1) { v /= 1024; i++; }
  const casas = v < 100 ? 1 : 0;
  return new Intl.NumberFormat(langAtual(), { minimumFractionDigits: casas, maximumFractionDigits: casas }).format(v) + " " + un[i];
}
function fmtBytesExatos(n) { return n == null ? "" : t("zip.bytes_exatos", { n: fmtInt(n) }); }

/** Porcentagem. Abaixo de 1%, dois algarismos significativos: com uma casa
    so, 21 bytes de 399 KiB saiam «0%» -- e 0% e mentira, nao arredondamento. */
function fmtPct(x) {
  const opc = x > 0 && x < 0.01 ? { style: "percent", maximumSignificantDigits: 2 } : { style: "percent", maximumFractionDigits: 1 };
  return new Intl.NumberFormat(langAtual(), opc).format(x);
}

/** Data do pacote, em hora local e no MESMO formato em toda lingua: data e
    dado, e dado nao se traduz. O `title` leva o instante em UTC, sem
    ambiguidade de fuso. */
function fmtData(unix) {
  if (unix == null) return { texto: "—", titulo: "" };
  const d = new Date(unix * 1000);
  if (Number.isNaN(d.getTime())) return { texto: "—", titulo: "" };
  const p = n => String(n).padStart(2, "0");
  return {
    texto: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`,
    titulo: d.toISOString().replace(".000Z", "Z"),
  };
}

/* Caracteres que o olho nao ve e que mudam o que o olho ve: controles C0/C1,
   as marcas de direcao (U+202E e irmaos), os de largura zero e o BOM.
   `fatura‮txt.exe` desenhado cru aparece como «faturaexe.txt». */
const INVISIVEIS = /[\u0000-\u001F\u007F-\u009F؜​-‏‪-‮⁠-⁩﻿]/g;

/** Um nome de entrada, como DADO: isolado da direcao da linha em volta e com
    todo caractere invisivel mostrado como marca. Esconder o nome reordenado
    seria mentir sobre o que esta gravado; a marca diz exatamente o que esta. */
function nomeDado(bruto) {
  const nome = String(bruto);
  const s = h("span", { class: "dado nome" });
  let ultimo = 0;
  String(nome).replace(INVISIVEIS, (c, pos) => {
    if (pos > ultimo) s.append(nome.slice(ultimo, pos));
    const cod = "U+" + c.codePointAt(0).toString(16).toUpperCase().padStart(4, "0");
    s.append(h("span", { class: "invisivel", title: t("zip.caractere_invisivel", { codigo: cod }) }, cod));
    ultimo = pos + c.length;
    return c;
  });
  if (ultimo < nome.length) s.append(nome.slice(ultimo));
  return s;
}

function base(nome) { const i = nome.lastIndexOf("/"); return i < 0 ? nome : nome.slice(i + 1); }
function semExtensao(nome) { return nome.replace(/\.(7z|phz)$/i, ""); }

// ------------------------------------------------------------- os erros
/* Cada nome de erro do contrato (§6) com a familia (a FORMA do cartao: o
   icone), o titulo e o que fazer. As chaves estao por extenso, uma por uma:
   e o que deixa o laco «chave que a tela pede × chave que o dicionario tem»
   enxerga-las por busca. Os nomes do motor sao os de `phxzip::Erro::nome()`. */
const ERROS = {
  NAO_E_7Z:                   { familia: "corrompido", titulo: "zip.erro_nao_e_7z", faca: "zip.erro_nao_e_7z_faca" },
  VERSAO_NAO_SUPORTADA:       { familia: "legado",     titulo: "zip.erro_versao", faca: "zip.erro_versao_faca" },
  ESTRUTURA:                  { familia: "corrompido", titulo: "zip.erro_estrutura", faca: "zip.erro_corrompido_faca" },
  CORROMPIDO:                 { familia: "corrompido", titulo: "zip.erro_corrompido", faca: "zip.erro_corrompido_faca" },
  SENHA_AUSENTE:              { familia: "senha",      titulo: "zip.erro_senha_ausente", faca: "zip.erro_senha_ausente_faca" },
  SENHA_ERRADA:               { familia: "senha",      titulo: "zip.erro_senha_errada", faca: "zip.erro_senha_errada_faca" },
  SENHA_ERRADA_OU_CORROMPIDO: { familia: "senha",      titulo: "zip.erro_senha_ou_corrompido", faca: "zip.erro_senha_ou_corrompido_faca" },
  METODO_LEGADO:              { familia: "legado",     titulo: "zip.erro_metodo_legado", faca: "zip.erro_metodo_faca" },
  METODO_DESCONHECIDO:        { familia: "legado",     titulo: "zip.erro_metodo_desconhecido", faca: "zip.erro_metodo_faca" },
  NOME_PERIGOSO:              { familia: "perigo",     titulo: "zip.erro_nome_perigoso", faca: "zip.erro_nome_perigoso_faca" },
  GRANDE_DEMAIS:              { familia: "tamanho",    titulo: "zip.erro_grande_demais", faca: "zip.erro_grande_demais_faca" },
  NAO_CABE:                   { familia: "tamanho",    titulo: "zip.erro_nao_cabe", faca: "zip.erro_nao_cabe_faca" },
  CICLOS_DEMAIS:              { familia: "tamanho",    titulo: "zip.erro_ciclos_demais", faca: "zip.erro_ciclos_demais_faca" },
  ENTRADA_INEXISTENTE:        { familia: "pedido",     titulo: "zip.erro_entrada_inexistente", faca: "zip.erro_listar_de_novo" },
  SEM_ENTRADA:                { familia: "pedido",     titulo: "zip.erro_sem_entrada", faca: "zip.erro_regra_phz" },
  MAIS_DE_UMA_ENTRADA:        { familia: "pedido",     titulo: "zip.erro_mais_de_uma", faca: "zip.erro_regra_phz" },
  SEM_CIFRA:                  { familia: "pedido",     titulo: "zip.erro_sem_cifra", faca: "zip.erro_regra_phz" },
  ENTRADA_E_PASTA:            { familia: "pedido",     titulo: "zip.erro_entrada_e_pasta", faca: "zip.erro_regra_phz" },
  NOME_REPETIDO:              { familia: "pedido",     titulo: "zip.erro_nome_repetido", faca: "zip.erro_nome_repetido_faca" },
  PEDIDO_MALFORMADO:          { familia: "servidor",   titulo: "zip.erro_pedido_malformado", faca: "zip.erro_avise_quem_mantem" },
  TIPO_DE_CONTEUDO:           { familia: "servidor",   titulo: "zip.erro_pedido_malformado", faca: "zip.erro_avise_quem_mantem" },
  TAMANHO_AUSENTE:            { familia: "servidor",   titulo: "zip.erro_pedido_malformado", faca: "zip.erro_avise_quem_mantem" },
  HOST_RECUSADO:              { familia: "servidor",   titulo: "zip.erro_origem", faca: "zip.erro_origem_faca" },
  ORIGEM_RECUSADA:            { familia: "servidor",   titulo: "zip.erro_origem", faca: "zip.erro_origem_faca" },
  ROTA_INEXISTENTE:           { familia: "servidor",   titulo: "zip.erro_pedido_malformado", faca: "zip.erro_avise_quem_mantem" },
  METODO_HTTP:                { familia: "servidor",   titulo: "zip.erro_pedido_malformado", faca: "zip.erro_avise_quem_mantem" },
  OCUPADO:                    { familia: "servidor",   titulo: "zip.erro_ocupado", faca: "zip.erro_ocupado_faca" },
  INTERNO:                    { familia: "servidor",   titulo: "zip.erro_interno", faca: "zip.erro_avise_quem_mantem" },
  // Os tres que nascem na PAGINA, e nao no servidor.
  REDE:                       { familia: "servidor",   titulo: "zip.erro_rede", faca: "zip.erro_rede_faca" },
  CANCELADO:                  { familia: "cancelado",  titulo: "zip.erro_cancelado", faca: "zip.erro_cancelado_faca" },
  DESCONHECIDO:               { familia: "servidor",   titulo: "zip.erro_desconhecido", faca: "zip.erro_avise_quem_mantem" },
};
/* O `oque` do GRANDE_DEMAIS e do NAO_CABE e palavra do motor («entrada»,
   «bloco», «cabecalho») ou da web («envio», «cabeca»): um identificador, e
   por isso ganha rotulo por chave -- mostra-lo cru seria portugues na tela
   alema. Desconhecido aparece como veio. */
const OQUE = {
  entrada: "zip.oque_entrada", bloco: "zip.oque_bloco", cabecalho: "zip.oque_cabecalho",
  envio: "zip.oque_envio", cabeca: "zip.oque_cabeca",
};

function cartaoDeErro(err) {
  const codigo = err && err.erro ? String(err.erro) : "DESCONHECIDO";
  const def = ERROS[codigo] || ERROS.DESCONHECIDO;
  const d = (err && err.detalhe) || {};
  const dados = {
    codigo: h("code", { class: "dado" }, codigo),
    metodo: d.metodo != null ? h("code", { class: "dado" }, String(d.metodo)) : undefined,
    nome: d.nome != null ? nomeDado(String(d.nome)) : undefined,
    versao: d.versao,
    indice: d.indice,
    quantas: d.quantas,
    oque: d.oque != null ? document.createTextNode(OQUE[d.oque] ? t(OQUE[d.oque]) : String(d.oque)) : undefined,
    declarado: d.declarado != null ? fmtTam(d.declarado) : undefined,
    teto: d.teto != null ? (codigo === "CICLOS_DEMAIS" ? "2^" + d.teto : fmtTam(d.teto)) : undefined,
    valor: d.valor != null ? fmtTam(d.valor) : undefined,
    pedidos: d.pedidos != null ? "2^" + d.pedidos : undefined,
    status: d.status,
  };
  const corpo = h("div", { class: "aviso-texto" },
    h("p", { class: "aviso-titulo" }, frase(def.titulo, dados)),
    h("p", { class: "aviso-faca" }, frase(def.faca, dados)));
  // O `onde` e texto do MOTOR, em portugues, e nao se traduz (contrato §6):
  // vai fechado, rotulado como detalhe tecnico, e nunca como a frase principal.
  if (d.onde) {
    corpo.append(h("details", { class: "tecnico" },
      h("summary", null, t("zip.detalhe_tecnico")),
      h("code", { class: "dado" }, String(d.onde))));
  }
  return h("div", { class: `aviso erro familia-${def.familia}`, role: "alert", "data-erro": codigo },
    icone(def.familia, "ic grande"), corpo);
}

function cartaoOk(titulo, detalhe) {
  return h("div", { class: "aviso sucesso", role: "status", "data-estado": "ok" },
    icone("ok", "ic grande"),
    h("div", { class: "aviso-texto" },
      h("p", { class: "aviso-titulo" }, titulo),
      detalhe ? h("p", { class: "aviso-faca" }, detalhe) : null));
}

function anunciar(s) { const a = $("#anuncio"); a.textContent = ""; setTimeout(() => { a.textContent = s; }, 30); }

// ------------------------------------------------------------- o envelope
const TEXTO = new TextEncoder();
/** O envelope do contrato (§3): `PZW1`, o tamanho da cabeca em u32 LE, a
    cabeca em JSON e a carga. A carga entra como `File`/`Blob`: o navegador a
    le do disco ao enviar, sem copiar o arquivo inteiro para a memoria. */
function envelope(cabeca, partes) {
  const json = TEXTO.encode(JSON.stringify(cabeca));
  const pre = new Uint8Array(8);
  pre.set([0x50, 0x5A, 0x57, 0x31]);
  new DataView(pre.buffer).setUint32(4, json.length, true);
  return new Blob([pre, json, ...partes], { type: "application/octet-stream" });
}

/** Um POST com progresso. `XMLHttpRequest`, e nao `fetch`, porque so ele
    conta os bytes ENVIADOS -- e enviar 200 MiB e a parte longa de compactar. */
function enviar(rota, corpo, prog) {
  return new Promise(resolve => {
    const x = new XMLHttpRequest();
    est.xhr = x;
    x.open("POST", rota);
    x.responseType = "blob";
    x.upload.onprogress = e => prog && prog.enviando(e.loaded, e.lengthComputable ? e.total : corpo.size);
    x.upload.onload = () => prog && prog.processando();
    x.onprogress = e => prog && prog.recebendo(e.loaded, e.lengthComputable ? e.total : 0);
    x.onload = async () => {
      est.xhr = null;
      const tipo = x.getResponseHeader("Content-Type") || "";
      if (x.status >= 200 && x.status < 300) {
        let json = null;
        if (tipo.includes("application/json")) {
          try { json = JSON.parse(await x.response.text()); } catch { json = null; }
        }
        resolve({ ok: true, blob: x.response, json, cab: n => x.getResponseHeader(n) });
        return;
      }
      let j = null;
      try { j = JSON.parse(await x.response.text()); } catch { j = null; }
      // Resposta sem o JSON do contrato (um proxy no meio, um servidor velho):
      // o codigo HTTP vira o nome, e o cartao o mostra em vez de fingir saber.
      resolve({ ok: false, erro: j && j.erro ? j : { erro: "HTTP_" + x.status } });
    };
    x.onerror = () => { est.xhr = null; resolve({ ok: false, erro: { erro: "REDE" } }); };
    x.onabort = () => { est.xhr = null; resolve({ ok: false, erro: { erro: "CANCELADO" } }); };
    x.send(corpo);
  });
}

function baixar(blob, nome) {
  const url = URL.createObjectURL(blob);
  const a = h("a", { href: url, download: nome, hidden: true });
  document.body.append(a);
  a.click();
  a.remove();
  // O endereco blob: vive ate a pagina fechar se ninguem o soltar.
  setTimeout(() => URL.revokeObjectURL(url), 120000);
  return url;
}

// ------------------------------------------------------------- o progresso
/* Tres fases, e cada uma so mostra o que se MEDE: enviar tem bytes
   contados; o trabalho do servidor nao tem (a resposta e uma so), e por
   isso aparece como barra sem fim, e nao como uma porcentagem inventada;
   receber tem bytes contados de novo. */
function criarProgresso(el) {
  let timer = null;
  const partes = {
    fase: h("p", { class: "fase" }),
    barra: h("div", { class: "medidor-barra" }),
    numeros: h("p", { class: "numeros" }),
    cancelar: h("button", { type: "button", class: "acao neutra pequena", onclick: () => est.xhr && est.xhr.abort() },
      icone("remover"), h("span", { "data-txt": "zip.cancelar" }, t("zip.cancelar"))),
  };
  const medidor = h("div", { class: "medidor", role: "progressbar", "aria-valuemin": "0", "aria-valuemax": "100" }, partes.barra);
  el.replaceChildren(h("div", { class: "progresso-topo" }, partes.fase, partes.cancelar), medidor, partes.numeros);
  // A fase e um NOME (vai no `data-fase`, que o roteiro le); o texto sai da
  // chave dela. Derivar um do outro recortando a chave e decidir por texto.
  const FASES = { enviando: "zip.fase_enviando", processando: "zip.fase_processando", recebendo: "zip.fase_recebendo" };
  function pintar(fase, feito, total) {
    el.dataset.fase = fase;
    partes.fase.textContent = t(FASES[fase]);
    if (total > 0) {
      const pct = Math.min(100, (feito / total) * 100);
      medidor.classList.remove("indeterminado");
      partes.barra.style.width = pct.toFixed(1) + "%";
      medidor.setAttribute("aria-valuenow", pct.toFixed(0));
      partes.numeros.replaceChildren(frase("zip.progresso_bytes", { feito: fmtTam(feito), total: fmtTam(total), pct: fmtPct(feito / total) }));
    } else {
      medidor.classList.add("indeterminado");
      partes.barra.style.width = "";
      medidor.removeAttribute("aria-valuenow");
      partes.numeros.replaceChildren();
    }
  }
  return {
    comecar() {
      pintar("enviando", 0, 1);
      // So aparece se demorar: pedido de 5 ms que pisca uma barra e ruido.
      timer = setTimeout(() => { el.hidden = false; }, 180);
    },
    enviando(a, tot) { pintar("enviando", a, tot); },
    processando() { pintar("processando", 0, 0); },
    recebendo(a, tot) { pintar("recebendo", a, tot); },
    fim() { clearTimeout(timer); el.hidden = true; delete el.dataset.fase; },
  };
}

/** Enquanto um pedido anda, nada que dispare outro pedido anda. Quem tem
    regra propria de ligar (testar so com lista) e repintado por quem chamou,
    logo depois. */
function ocupar(sim) {
  est.ocupado = sim;
  document.body.classList.toggle("ocupado", sim);
  for (const b of $$("[data-trava]")) b.disabled = sim;
  pintarCompactarBotao();
}

// ================================================================ COMPACTAR
function deArquivo(f, nome) {
  return {
    nome, pasta: false, tamanho: f.size,
    modificado: f.lastModified ? Math.floor(f.lastModified / 1000) : null,
    arquivo: f,
  };
}

/** Le o que foi solto. As `webkitGetAsEntry` sao pedidas ANTES do primeiro
    `await`: depois que o evento termina, o `DataTransfer` esvazia e as
    entradas somem. */
async function lerSoltura(dt) {
  const soltos = [];
  const entradas = [];
  for (const it of dt.items ? [...dt.items] : []) {
    if (it.kind !== "file") continue;
    const e = typeof it.webkitGetAsEntry === "function" ? it.webkitGetAsEntry() : null;
    if (e) entradas.push(e);
    else { const f = it.getAsFile(); if (f) soltos.push(deArquivo(f, f.name)); }
  }
  if (!dt.items || !dt.items.length) for (const f of dt.files || []) soltos.push(deArquivo(f, f.name));
  for (const e of entradas) await percorrer(e, soltos);
  return soltos;
}

/** Percorre uma pasta solta. `readEntries` devolve em LOTES (100 no
    Chromium): quem chama uma vez so leva as cem primeiras e perde o resto
    calado. Le-se ate vir lote vazio. */
async function percorrer(e, lista) {
  const nome = e.fullPath.replace(/^\/+/, "");
  if (e.isFile) {
    const f = await new Promise((ok, falha) => e.file(ok, falha));
    lista.push(deArquivo(f, nome));
  } else if (e.isDirectory) {
    lista.push({ nome, pasta: true, tamanho: 0, modificado: null, arquivo: null });
    const leitor = e.createReader();
    for (;;) {
      const lote = await new Promise((ok, falha) => leitor.readEntries(ok, falha));
      if (!lote.length) break;
      for (const x of lote) await percorrer(x, lista);
    }
  }
}

/** Toda pasta que um caminho atravessa vira item-pasta, antes do primeiro
    filho -- o que o 7-Zip grava, e o que faz a pasta voltar ao extrair. */
function completarPastas(itens) {
  const existe = new Set(itens.map(i => i.nome));
  const saida = [];
  for (const i of itens) {
    const partes = i.nome.split("/");
    for (let k = 1; k < partes.length; k++) {
      const p = partes.slice(0, k).join("/");
      if (!existe.has(p)) {
        existe.add(p);
        saida.push({ nome: p, pasta: true, tamanho: 0, modificado: null, arquivo: null });
      }
    }
    saida.push(i);
  }
  return saida;
}

function acrescentar(novos) {
  const ja = new Set(est.itens.map(i => i.nome));
  let repetidos = 0;
  const entram = [];
  for (const n of completarPastas(novos)) {
    if (ja.has(n.nome)) { if (!n.pasta) repetidos++; continue; }
    ja.add(n.nome);
    entram.push(n);
  }
  est.itens = completarPastas(est.itens.concat(entram));
  est.recadoCompactar = repetidos ? { n: repetidos } : null;
  est.msgCompactar = null;
  pintarCompactar();
}

function remover(nome) {
  // FILTER, e nao splice pelo indice do primeiro achado: tirar uma pasta tira
  // tudo o que esta dentro dela, e o dentro nao e um item so.
  est.itens = est.itens.filter(i => i.nome !== nome && !i.nome.startsWith(nome + "/"));
  est.msgCompactar = null;
  pintarCompactar();
}

function totais() {
  let arquivos = 0, pastas = 0, bytes = 0;
  for (const i of est.itens) {
    if (i.pasta) pastas++;
    else { arquivos++; bytes += i.tamanho; }
  }
  return { arquivos, pastas, bytes };
}

function tetoEnvio() { return est.estado ? est.estado.limites.envio : Infinity; }

/** O envelope sem a carga: e o que se soma aos arquivos para medir o envio
    contra o teto. Sem a senha -- que so entra no pedido de verdade --, a
    diferenca e de poucos bytes, e o servidor confere o numero exato. */
function cabecaDeCompactar(comSenha) {
  const senha = $("#inSenha").value;
  const phz = est.formato === "phz";
  const cabeca = {
    formato: est.formato,
    nivel: est.nivel,
    cifrar_nomes: phz ? true : $("#ckCifrarNomes").checked,
    nome: nomeDoPacote(),
    itens: est.itens.map(i => i.pasta
      ? { nome: i.nome, pasta: true, modificado: i.modificado }
      : { nome: i.nome, pasta: false, tamanho: i.tamanho, modificado: i.modificado }),
  };
  if (comSenha && senha) cabeca.senha = senha;
  return cabeca;
}

function tamanhoDoEnvio() {
  return 8 + TEXTO.encode(JSON.stringify(cabecaDeCompactar(false))).length + totais().bytes;
}

function nomePadrao() {
  const topo = [...new Set(est.itens.map(i => i.nome.split("/")[0]))];
  if (topo.length === 1) return semExtensao(topo[0].replace(/\.[^.]+$/, "") || topo[0]);
  return t("zip.nome_padrao");
}

function nomeDoPacote() {
  const bruto = $("#inNomePacote").value.trim().replace(/[\\/]/g, "_");
  return semExtensao(bruto || nomePadrao()) + "." + est.formato;
}

/** Por que o botao de compactar NAO anda -- escrito ao lado dele. Botao
    desligado sem motivo a vista e um botao quebrado para quem olha. */
function motivosParaNaoCompactar() {
  const m = [];
  const { arquivos, pastas } = totais();
  const senha = $("#inSenha").value, senha2 = $("#inSenha2").value;
  if (!arquivos && !pastas) m.push(t("zip.porque_vazio"));
  if (tamanhoDoEnvio() > tetoEnvio()) m.push(t("zip.porque_teto", { teto: fmtTam(tetoEnvio()) }));
  if (senha !== senha2 && !$("#ckMostrar").checked) m.push(t("zip.porque_senhas_diferentes"));
  if (est.formato === "phz") {
    if (!senha) m.push(t("zip.porque_phz_senha"));
    if (arquivos !== 1 || pastas) m.push(t("zip.porque_phz_um_arquivo"));
  }
  return m;
}

function pintarCompactar() {
  const temItens = est.itens.length > 0;
  $("#listaCompactar").hidden = !temItens;
  const rec = $("#recadoCompactar");
  rec.hidden = !est.recadoCompactar;
  if (est.recadoCompactar) rec.replaceChildren(icone("pedido"), h("span", null, plural(est.recadoCompactar.n, "zip.recado_repetidos_um", "zip.recado_repetidos_varios")));

  // A grade.
  const corpo = $("#gradeCompactar tbody");
  const rot = { nome: t("zip.col_nome"), tamanho: t("zip.col_tamanho"), mod: t("zip.col_modificado") };
  corpo.replaceChildren(...est.itens.map(i => {
    const d = fmtData(i.modificado);
    const tr = h("tr", { class: i.pasta ? "e-pasta" : "e-arquivo" },
      h("td", { class: "c-tipo" }, icone(i.pasta ? "pasta" : "arquivo")),
      h("td", { class: "c-nome", "data-rotulo": rot.nome }, nomeDado(i.nome)),
      h("td", { class: "num", "data-rotulo": rot.tamanho, title: i.pasta ? null : fmtBytesExatos(i.tamanho) },
        h("span", { class: "dado" }, i.pasta ? "—" : fmtTam(i.tamanho))),
      h("td", { class: "c-data", "data-rotulo": rot.mod, title: d.titulo }, h("span", { class: "dado" }, d.texto)),
      h("td", { class: "c-acoes" },
        h("button", {
          type: "button", class: "acao excluir icone-so", "data-trava": "",
          "aria-label": t("zip.remover_item", { nome: i.nome }), title: t("zip.remover"),
          onclick: () => remover(i.nome),
        }, icone("remover"))));
    return tr;
  }));

  // O rodape: quanto vai, e quanto cabe.
  const tot = totais();
  const envio = tamanhoDoEnvio();
  const teto = tetoEnvio();
  $("#resumoItens").replaceChildren(frase("zip.resumo_itens", {
    arquivos: document.createTextNode(plural(tot.arquivos, "zip.arquivos_um", "zip.arquivos_varios", "zip.arquivos_zero")),
    pastas: document.createTextNode(plural(tot.pastas, "zip.pastas_um", "zip.pastas_varios", "zip.pastas_zero")),
    total: fmtTam(tot.bytes),
  }));
  const estourou = envio > teto;
  const blocoTeto = $("#tetoCompactar");
  blocoTeto.classList.toggle("estourou", estourou);
  // A frase vai DENTRO de um span: o `.teto-uso` e flex, e filho direto de
  // flex perde os espacos das bordas -- «11,0 KiB  von  16,0 MiB  , die…»
  // saiu assim no alemao, com o espaco antes da virgula.
  $("#resumoTeto").replaceChildren(
    estourou ? icone("tamanho") : "",
    h("span", null, frase(estourou ? "zip.acima_do_teto" : "zip.dentro_do_teto", { usado: fmtTam(envio), teto: fmtTam(teto) })));
  $("#medidorEnvio").style.width = Math.min(100, Number.isFinite(teto) ? (envio / teto) * 100 : 0).toFixed(1) + "%";

  $("#opcoesCompactar").hidden = !temItens;
  pintarOpcoes();
  pintarMsgCompactar();
}

function pintarOpcoes() {
  if (!est.estado) return;
  // O formato.
  const segF = $("#segFormato");
  segF.replaceChildren(...est.estado.formatos.map(f => segmento("formato", f,
    h("span", { class: "dado" }, "." + f), est.formato === f, () => { est.formato = f; pintarCompactar(); })));
  $("#dicaFormato").textContent = t(est.formato === "phz" ? "zip.formato_phz_dica" : "zip.formato_7z_dica");

  // O nivel -- so os que o servidor declara (o motor tem um esforco so hoje).
  const phz = est.formato === "phz";
  const segN = $("#segNivel");
  const ROTULO_NIVEL = { armazenar: "zip.nivel_armazenar", lzma2: "zip.nivel_lzma2" };
  const DICA_NIVEL = { armazenar: "zip.nivel_armazenar_dica", lzma2: "zip.nivel_lzma2_dica" };
  if (!est.estado.niveis.includes(est.nivel)) est.nivel = est.estado.niveis[est.estado.niveis.length - 1];
  segN.replaceChildren(...est.estado.niveis.map(n => segmento("nivel", n,
    h("span", null, ROTULO_NIVEL[n] ? t(ROTULO_NIVEL[n]) : n), est.nivel === n,
    () => { est.nivel = n; pintarCompactar(); }, phz)));
  $("#grupoNivel").classList.toggle("desligado", phz);
  $("#dicaNivel").textContent = phz ? t("zip.nivel_fixo_no_phz")
    : (DICA_NIVEL[est.nivel] ? t(DICA_NIVEL[est.nivel]) : "");

  // O nome.
  const campo = $("#inNomePacote");
  if (!est.nomeEditado) campo.value = nomePadrao();
  $("#sufixo").textContent = "." + est.formato;

  // A senha.
  $("#legendaSenha").textContent = t(phz ? "zip.senha_obrigatoria" : "zip.senha_opcional");
  const temSenha = $("#inSenha").value.length > 0;
  const ck = $("#ckCifrarNomes");
  ck.disabled = phz || !temSenha;
  if (phz) ck.checked = true;
  $("#rotuloCifrarNomes").classList.toggle("desligado", ck.disabled);
  const diferentes = $("#inSenha2").value && $("#inSenha").value !== $("#inSenha2").value && !$("#ckMostrar").checked;
  $("#inSenha2").setAttribute("aria-invalid", diferentes ? "true" : "false");
  $("#campoSenha2").hidden = $("#ckMostrar").checked;
  pintarCompactarBotao();
}

function pintarCompactarBotao() {
  const b = $("#btCompactar");
  if (!b) return;
  const motivos = est.itens.length ? motivosParaNaoCompactar() : [];
  b.disabled = est.ocupado || motivos.length > 0;
  $("#porquesCompactar").replaceChildren(...motivos.map(m => h("li", null, icone("pedido"), h("span", null, m))));
}

/** Um botao de um grupo de escolha unica: `<input type=radio>` de verdade
    (teclado e leitor de tela de graca), escondido so da VISTA, com o rotulo
    desenhado por cima. */
function segmento(grupo, valor, conteudo, marcado, escolher, desligado) {
  const id = `seg-${grupo}-${valor}`;
  const inp = h("input", { type: "radio", name: grupo, id, value: valor, class: "seg-radio", disabled: !!desligado });
  inp.checked = marcado;
  inp.addEventListener("change", () => { if (inp.checked) escolher(); });
  return h("span", { class: "seg" }, inp, h("label", { for: id, class: "seg-rotulo" }, conteudo));
}

function pintarMsgCompactar() {
  const alvo = $("#resultadoCompactar");
  const m = est.msgCompactar;
  alvo.hidden = !m;
  if (!m) { alvo.replaceChildren(); return; }
  if (m.tipo === "erro") { alvo.replaceChildren(cartaoDeErro(m.erro)); return; }
  const titulo = frase("zip.pronto", { nome: h("span", { class: "dado nome" }, m.nome) });
  const detalhe = frase(m.original > 0 ? "zip.pronto_detalhe" : "zip.pronto_detalhe_sem_razao", {
    tamanho: fmtTam(m.tamanho), original: fmtTam(m.original),
    razao: m.original > 0 ? fmtPct(m.tamanho / m.original) : undefined,
  });
  const card = cartaoOk(titulo, detalhe);
  card.querySelector(".aviso-texto").append(h("button", {
    type: "button", class: "acao consultar pequena", id: "btBaixarDeNovo",
    onclick: () => { if (est.ultimoPacote) baixar(est.ultimoPacote.blob, est.ultimoPacote.nome); },
  }, icone("baixar"), h("span", null, t("zip.baixar_de_novo"))));
  alvo.replaceChildren(card);
}

async function compactar() {
  if (motivosParaNaoCompactar().length || est.ocupado) return;
  const cabeca = cabecaDeCompactar(true);
  // FILTER: a carga e cada item que NAO e pasta, na ordem da lista. A licao
  // do `rownum` na tela do PhxSql: quem pega o primeiro onde devia pegar
  // todos quebra calado no dia em que a lista ganha uma peca no fim.
  const carga = est.itens.filter(i => !i.pasta).map(i => i.arquivo);
  const corpo = envelope(cabeca, carga);
  if (corpo.size > tetoEnvio()) {
    est.msgCompactar = { tipo: "erro", erro: { erro: "GRANDE_DEMAIS", detalhe: { oque: "envio", declarado: corpo.size, teto: tetoEnvio() } } };
    pintarMsgCompactar();
    return;
  }
  est.msgCompactar = null;
  pintarMsgCompactar();
  const prog = criarProgresso($("#progressoCompactar"));
  ocupar(true);
  prog.comecar();
  const r = await enviar("/api/compactar", corpo, prog);
  prog.fim();
  ocupar(false);
  if (!r.ok) {
    est.msgCompactar = { tipo: "erro", erro: r.erro };
    pintarMsgCompactar();
    anunciar(t((ERROS[r.erro.erro] || ERROS.DESCONHECIDO).titulo));
    return;
  }
  const nome = cabeca.nome;
  const original = Number(r.cab("X-PhxZip-Tamanho-Original")) || totais().bytes;
  est.ultimoPacote = { blob: r.blob, nome };
  baixar(r.blob, nome);
  est.msgCompactar = { tipo: "ok", nome, tamanho: r.blob.size, original };
  pintarMsgCompactar();
  anunciar(t("zip.anuncio_pronto"));
}

// ================================================================ ABRIR
function escolherPacote(f) {
  fecharPacote(false);
  est.pacote = { arquivo: f, nome: f.name, tamanho: f.size };
  pintarAbrir();
  listar();
}

function fecharPacote(repintar = true) {
  if (est.xhr) est.xhr.abort();
  est.pacote = null;
  est.lista = null;
  est.pedirSenha = null;
  est.msgAbrir = null;
  est.msgTeste = null;
  est.recadoAbrir = null;
  est.filtro = "";
  // A senha sai da pagina junto com o pacote: nao fica esperando no campo.
  $("#inSenhaPacote").value = "";
  $("#inFiltro").value = "";
  if (repintar) pintarAbrir();
}

function cabecaComSenha(extra) {
  const c = Object.assign({}, extra);
  const s = $("#inSenhaPacote").value;
  if (s) c.senha = s;
  return c;
}

/** Um pedido sobre o pacote aberto: monta o envelope, mostra o progresso, e
    devolve a resposta -- ou poe o erro no cartao, onde ele se le. */
async function pedirSobrePacote(rota, extra) {
  if (!est.pacote || est.ocupado) return null;
  const corpo = envelope(cabecaComSenha(extra), [est.pacote.arquivo]);
  if (corpo.size > tetoEnvio()) {
    est.msgAbrir = { tipo: "erro", erro: { erro: "GRANDE_DEMAIS", detalhe: { oque: "envio", declarado: corpo.size, teto: tetoEnvio() } } };
    pintarAbrir();
    return null;
  }
  const prog = criarProgresso($("#progressoAbrir"));
  ocupar(true);
  prog.comecar();
  const r = await enviar(rota, corpo, prog);
  prog.fim();
  ocupar(false);
  if (!r.ok) {
    const cod = r.erro.erro;
    if (cod === "SENHA_AUSENTE" || cod === "SENHA_ERRADA" || cod === "SENHA_ERRADA_OU_CORROMPIDO") {
      est.pedirSenha = est.lista ? "zip.dica_senha_conteudo" : "zip.dica_senha_nomes";
    }
    est.msgAbrir = { tipo: "erro", erro: r.erro };
    pintarAbrir();
    anunciar(t((ERROS[cod] || ERROS.DESCONHECIDO).titulo));
    if (est.pedirSenha) { const c = $("#inSenhaPacote"); c.focus(); c.select(); }
    return null;
  }
  est.msgAbrir = null;
  return r;
}

async function listar() {
  est.msgTeste = null;
  const r = await pedirSobrePacote("/api/listar", {});
  if (!r) return;
  est.lista = r.json;
  // Cabecalho aberto e conteudo cifrado: a lista vem, e a senha continua
  // sendo pedida -- para espiar, baixar e testar.
  const algumaCifrada = (est.lista.entradas || []).some(e => e.cifrada);
  est.pedirSenha = algumaCifrada ? "zip.dica_senha_conteudo" : null;
  if (est.lista.cabecalho_cifrado) est.pedirSenha = "zip.dica_senha_conteudo";
  pintarAbrir();
  anunciar(t("zip.anuncio_listado"));
}

async function testar() {
  est.msgTeste = null;
  pintarAbrir();
  const r = await pedirSobrePacote("/api/testar", {});
  if (!r) return;
  est.msgTeste = { tipo: "ok", r: r.json };
  pintarAbrir();
  anunciar(t("zip.teste_ok_titulo"));
}

async function baixarEntrada(e) {
  const r = await pedirSobrePacote("/api/extrair", { indices: [e.indice] });
  if (!r) return;
  baixar(r.blob, base(e.nome));
  pintarAbrir();
}

async function baixarTodas() {
  const r = await pedirSobrePacote("/api/extrair", { todas: true });
  if (!r) return;
  baixar(r.blob, semExtensao(est.pacote.nome) + ".tar");
  pintarAbrir();
}

function pintarAbrir() {
  const p = est.pacote;
  $("#zonaAbrir").classList.toggle("compacta", !!p);
  $("#cartaoPacote").hidden = !p;
  if (!p) { $("#avisoAbrir").replaceChildren(); return; }
  $("#pacoteNome").replaceChildren(nomeDado(p.nome));
  $("#pacoteTam").textContent = fmtTam(p.tamanho);
  $("#pacoteTam").title = fmtBytesExatos(p.tamanho);

  // A senha: aparece quando o pacote pede, com a dica do porque.
  $("#senhaPacote").hidden = !est.pedirSenha;
  if (est.pedirSenha) $("#dicaSenhaPacote").textContent = t(est.pedirSenha);
  const senhaErrada = est.msgAbrir && est.msgAbrir.erro && /^SENHA_ERRADA/.test(est.msgAbrir.erro.erro);
  $("#inSenhaPacote").setAttribute("aria-invalid", senhaErrada ? "true" : "false");
  $("#btAbrirComSenha").hidden = !!est.lista;

  $("#avisoAbrir").replaceChildren(...(est.msgAbrir ? [cartaoDeErro(est.msgAbrir.erro)] : []));

  const temLista = !!est.lista;
  $("#btTestar").disabled = est.ocupado || !temLista;
  $("#btBaixarTodas").disabled = est.ocupado || !temLista || !(est.lista.entradas || []).length;
  pintarTeste();
  pintarResumo();
  pintarLista();
}

function pintarTeste() {
  const alvo = $("#resultadoTeste");
  const m = est.msgTeste;
  alvo.hidden = !m;
  if (!m) { alvo.replaceChildren(); return; }
  const r = m.r || {};
  alvo.replaceChildren(cartaoOk(
    document.createTextNode(t("zip.teste_ok_titulo")),
    frase("zip.teste_ok", {
      entradas: document.createTextNode(plural(r.entradas || 0, "zip.arquivos_um", "zip.arquivos_varios", "zip.arquivos_zero")),
      bytes: fmtTam(r.bytes || 0),
    })));
}

function pintarResumo() {
  const alvo = $("#resumoPacote");
  const L = est.lista;
  if (!L) { alvo.replaceChildren(); alvo.hidden = true; return; }
  alvo.hidden = false;
  const ents = L.entradas || [];
  const arquivos = ents.filter(e => !e.pasta);
  const pastas = ents.length - arquivos.length;
  const original = arquivos.reduce((s, e) => s + (e.tamanho || 0), 0);
  const pacote = L.tamanho_do_pacote != null ? L.tamanho_do_pacote : est.pacote.tamanho;
  const linha = h("p", { class: "resumo-linha" }, frase(original > 0 ? "zip.resumo_pacote" : "zip.resumo_pacote_sem_razao", {
    arquivos: document.createTextNode(plural(arquivos.length, "zip.arquivos_um", "zip.arquivos_varios", "zip.arquivos_zero")),
    pastas: document.createTextNode(plural(pastas, "zip.pastas_um", "zip.pastas_varios", "zip.pastas_zero")),
    original: fmtTam(original), pacote: fmtTam(pacote),
    razao: original > 0 ? fmtPct(pacote / original) : undefined,
  }));
  const selos = h("div", { class: "selos" });
  if (L.cabecalho_cifrado) selos.append(selo("cadeado", t("zip.selo_nomes_cifrados"), "cifra"));
  if (arquivos.some(e => e.cifrada)) selos.append(selo("cadeado", t("zip.selo_conteudo_cifrado"), "cifra"));
  const metodos = [...new Set((L.blocos || []).flatMap(b => b.metodos || []))];
  for (const m of metodos) selos.append(h("span", { class: "selo metodo" }, h("span", { class: "dado" }, m)));
  const solidos = (L.blocos || []).filter(b => (b.entradas || 0) > 1).length;
  if (solidos) selos.append(selo("compactar", plural(solidos, "zip.selo_blocos_solidos_um", "zip.selo_blocos_solidos_varios"), "solido"));
  alvo.replaceChildren(linha, selos);
}

function selo(ic, texto, classe) {
  return h("span", { class: "selo " + (classe || "") }, icone(ic), h("span", null, texto));
}

function pintarLista() {
  const L = est.lista;
  $("#listaPacote").hidden = !L;
  if (!L) return;
  const blocos = new Map((L.blocos || []).map(b => [b.indice, b]));
  const rot = { nome: t("zip.col_nome"), tamanho: t("zip.col_tamanho"), comp: t("zip.col_compactado"), mod: t("zip.col_modificado") };
  const filtro = est.filtro.toLocaleLowerCase();
  let visiveis = 0;
  const linhas = (L.entradas || []).map(e => {
    const d = fmtData(e.modificado);
    const bl = e.bloco != null ? blocos.get(e.bloco) : null;
    let comp;
    if (e.pasta || !bl) comp = h("span", { class: "dado" }, "—");
    else if ((bl.entradas || 1) === 1) {
      comp = h("span", { class: "dado", title: fmtBytesExatos(bl.compactado) }, fmtTam(bl.compactado));
    } else {
      // Solido: o compactado e do BLOCO. Dividi-lo entre as entradas seria
      // inventar numero; dizer «solido» e dizer o que se sabe.
      comp = h("span", { class: "rot-solido", title: t("zip.solido_dica", { n: fmtInt(bl.entradas), bloco: fmtInt(bl.indice) }) }, t("zip.solido"));
    }
    const nomeCel = h("td", { class: "c-nome", "data-rotulo": rot.nome }, nomeDado(e.nome));
    if (e.cifrada) nomeCel.append(h("span", { class: "cifrada", title: t("zip.entrada_cifrada"), "aria-label": t("zip.entrada_cifrada"), role: "img" }, icone("cadeado")));
    const acoes = h("td", { class: "c-acoes" });
    if (!e.pasta) {
      acoes.append(
        h("button", { type: "button", class: "acao consultar icone-so", "data-trava": "", "data-acao": "espiar",
          "aria-label": t("zip.espiar_item", { nome: e.nome }), title: t("zip.espiar"), onclick: () => espiar(e) }, icone("olho")),
        h("button", { type: "button", class: "acao consultar icone-so", "data-trava": "", "data-acao": "baixar",
          "aria-label": t("zip.baixar_item", { nome: e.nome }), title: t("zip.baixar"), onclick: () => baixarEntrada(e) }, icone("baixar")));
    }
    const tr = h("tr", { class: e.pasta ? "e-pasta" : "e-arquivo", "data-indice": e.indice },
      h("td", { class: "c-tipo" }, icone(e.pasta ? "pasta" : "arquivo")),
      nomeCel,
      h("td", { class: "num", "data-rotulo": rot.tamanho, title: e.pasta ? null : fmtBytesExatos(e.tamanho) },
        h("span", { class: "dado" }, e.pasta ? "—" : fmtTam(e.tamanho))),
      h("td", { class: "num", "data-rotulo": rot.comp }, comp),
      h("td", { class: "c-data", "data-rotulo": rot.mod, title: d.titulo }, h("span", { class: "dado" }, d.texto)),
      acoes);
    // O filtro compara em minusculas; o que se MOSTRA continua como gravado.
    const passa = !filtro || e.nome.toLocaleLowerCase().includes(filtro);
    tr.hidden = !passa;
    if (passa) visiveis++;
    return tr;
  });
  $("#gradePacote tbody").replaceChildren(...linhas);
  $("#filtroVazio").hidden = visiveis > 0 || !(L.entradas || []).length;
  $("#listaVazia").hidden = (L.entradas || []).length > 0;
  for (const b of $$("#gradePacote [data-trava]")) b.disabled = est.ocupado;
}

// ================================================================ ESPIAR
/* A ideia que so a web da a um compactador: ler o que esta DENTRO do pacote
   sem extrair nada para o disco. Nasceu do pedido 450: os `config.json`
   passam a morar num `.phz` justamente para nao ficarem em texto claro no
   disco -- e extrair para conferir desfaria isso. Aqui o conteudo vai do
   servidor para a memoria da pagina e so.

   E o veredito «JSON valido» e do MOTOR (o `Json::analisar` da casa, pelo
   servidor), nao desta pagina: um segundo juiz diria «valido» onde o motor
   diz «invalido» -- e e o motor que vai ler o arquivo. */
async function espiar(e) {
  const lim = est.estado.limites;
  const pedirJson = /\.json$/i.test(e.nome);
  const r = await pedirSobrePacote("/api/extrair", { indices: [e.indice], ate: lim.espiar, avaliar_json: pedirJson });
  if (!r) return;
  const bytes = new Uint8Array(await r.blob.arrayBuffer());
  const total = Number(r.cab("X-PhxZip-Tamanho"));
  est.espiado = {
    entrada: e,
    bytes,
    total: Number.isFinite(total) && total > 0 ? total : bytes.length,
    cortado: r.cab("X-PhxZip-Cortado") === "1",
    json: pedirJson ? r.cab("X-PhxZip-Json") : null,
    posicao: r.cab("X-PhxZip-Json-Posicao"),
  };
  pintarAbrir();
  pintarEspiar();
  const dlg = $("#espiar");
  if (!dlg.open) dlg.showModal();
}

/** Texto ou binario. O decodificador e ESTRITO (`fatal`): um byte que nao e
    UTF-8 faz o conteudo ser mostrado como hexadecimal, em vez de virar um
    `�` que finge ser o texto. Com `stream`, o caractere cortado no fim
    do trecho fica de fora em vez de reprovar tudo. */
function decodificar(bytes, cortado) {
  if (bytes.includes(0)) return null;
  try { return new TextDecoder("utf-8", { fatal: true }).decode(bytes, { stream: cortado }); }
  catch { return null; }
}

/** Posicao em BYTES (a do motor) para linha e coluna em caracteres. So
    responde se a posicao estiver dentro do trecho que chegou. */
function linhaColuna(bytes, pos) {
  if (!(pos >= 0) || pos > bytes.length) return null;
  let linha = 1, inicio = 0;
  for (let i = 0; i < pos; i++) if (bytes[i] === 0x0A) { linha++; inicio = i + 1; }
  let coluna;
  try { coluna = new TextDecoder("utf-8").decode(bytes.subarray(inicio, pos)).length + 1; } catch { coluna = pos - inicio + 1; }
  return { linha, coluna };
}

function pintarEspiar() {
  const E = est.espiado;
  if (!E) return;
  const e = E.entrada;
  $("#espiarTitulo").replaceChildren(nomeDado(e.nome));
  const texto = decodificar(E.bytes, E.cortado);
  const selos = [];
  selos.push(selo(texto != null ? "arquivo" : "binario", t(texto != null ? "zip.espiar_texto" : "zip.espiar_binario"), "tipo"));
  let erroEm = null;
  if (E.json === "valido") selos.push(selo("json_ok", t("zip.json_valido"), "json-ok"));
  else if (E.json === "invalido") {
    const pos = Number(E.posicao);
    const lc = E.posicao != null && E.posicao !== "" ? linhaColuna(E.bytes, pos) : null;
    if (lc) { erroEm = lc.linha; selos.push(selo("json_erro", t("zip.json_invalido_em", { linha: fmtInt(lc.linha), coluna: fmtInt(lc.coluna) }), "json-erro")); }
    else if (E.posicao != null && E.posicao !== "") selos.push(selo("json_erro", t("zip.json_invalido_posicao", { posicao: fmtInt(pos) }), "json-erro"));
    else selos.push(selo("json_erro", t("zip.json_invalido"), "json-erro"));
  } else if (E.json === "nao_avaliado") {
    selos.push(selo("pedido", t("zip.json_nao_avaliado", { teto: fmtTam(est.estado.limites.avaliar_json) }), "json-neutro"));
  }
  if (E.cortado) selos.push(selo("corte", t("zip.espiar_cortado", { n: fmtTam(E.bytes.length), total: fmtTam(E.total) }), "corte"));
  else selos.push(selo("corte", t("zip.espiar_inteiro", { total: fmtTam(E.total) }), "inteiro"));
  $("#espiarSelos").replaceChildren(...selos);

  const corpo = $("#espiarCorpo");
  if (texto != null) {
    // Uma linha por item: o numero sai do marcador da lista (nao e texto do
    // documento), e a linha do erro ganha forma -- borda e fundo -- sem que
    // um caractere do dado mude.
    const linhas = texto.split("\n");
    if (linhas.length > 1 && linhas[linhas.length - 1] === "") linhas.pop();
    const ol = h("ol", { class: "codigo dado", start: "1" });
    ol.style.setProperty("--digitos", String(String(linhas.length).length));
    linhas.forEach((l, i) => {
      const li = h("li", null, l.length ? l : "​");
      if (erroEm === i + 1) { li.className = "linha-erro"; li.setAttribute("data-erro", ""); }
      ol.append(li);
    });
    corpo.replaceChildren(ol);
    const alvoErro = ol.querySelector(".linha-erro");
    if (alvoErro) requestAnimationFrame(() => alvoErro.scrollIntoView({ block: "center" }));
  } else {
    corpo.replaceChildren(h("pre", { class: "hex dado" }, hexdump(E.bytes, 4096)));
    if (E.bytes.length > 4096) corpo.append(h("p", { class: "dica" }, t("zip.hex_primeiros", { n: fmtInt(4096) })));
  }
  $("#btCopiar").hidden = texto == null;
}

function hexdump(b, max) {
  const n = Math.min(b.length, max);
  const out = [];
  for (let i = 0; i < n; i += 16) {
    const fatia = b.subarray(i, Math.min(i + 16, n));
    const hex = [...fatia].map(x => x.toString(16).padStart(2, "0")).join(" ").padEnd(47, " ");
    const asc = [...fatia].map(x => (x >= 0x20 && x < 0x7f) ? String.fromCharCode(x) : ".").join("");
    out.push(i.toString(16).padStart(8, "0") + "  " + hex + "  " + asc);
  }
  return out.join("\n");
}

async function copiarEspiado() {
  const E = est.espiado;
  if (!E) return;
  const texto = decodificar(E.bytes, E.cortado);
  if (texto == null) return;
  try {
    await navigator.clipboard.writeText(texto);
    est.recadoEspiar = "zip.copiado";
  } catch {
    est.recadoEspiar = "zip.copiar_falhou";
  }
  const r = $("#recadoEspiar");
  r.textContent = t(est.recadoEspiar);
  r.hidden = false;
}

// ================================================================ ARRASTAR
/* O arrasto vale na JANELA inteira e cai na aba aberta: quem arrasta um
   arquivo mira a pagina, nao um retangulo. O contador existe porque o
   `dragleave` dispara a cada filho que o cursor atravessa. */
let arrastando = 0;
function temArquivos(e) { return e.dataTransfer && [...(e.dataTransfer.types || [])].includes("Files"); }
function abaAtual() { return $("#abaAbrir").getAttribute("aria-selected") === "true" ? "abrir" : "compactar"; }

function ligarArrasto() {
  window.addEventListener("dragenter", e => {
    if (!temArquivos(e)) return;
    e.preventDefault();
    arrastando++;
    $("#soltarTexto").textContent = t(abaAtual() === "abrir" ? "zip.soltar_para_abrir" : "zip.soltar_para_compactar");
    $("#soltar").hidden = false;
  });
  window.addEventListener("dragover", e => { if (temArquivos(e)) { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; } });
  window.addEventListener("dragleave", e => {
    if (!temArquivos(e)) return;
    arrastando = Math.max(0, arrastando - 1);
    if (!arrastando) $("#soltar").hidden = true;
  });
  window.addEventListener("drop", e => {
    if (!temArquivos(e)) return;
    e.preventDefault();
    arrastando = 0;
    $("#soltar").hidden = true;
    if (est.ocupado) return;
    if (abaAtual() === "abrir") {
      const fs = [...(e.dataTransfer.files || [])];
      if (!fs.length) return;
      est.recadoAbrir = fs.length > 1 ? "zip.recado_um_pacote" : null;
      escolherPacote(fs[0]);
      pintarRecadoAbrir();
    } else {
      lerSoltura(e.dataTransfer).then(acrescentar);
    }
  });
}

function pintarRecadoAbrir() {
  const r = $("#recadoAbrir");
  r.hidden = !est.recadoAbrir;
  if (est.recadoAbrir) r.replaceChildren(icone("pedido"), h("span", null, t(est.recadoAbrir)));
}

// ================================================================ ABAS
function escolherAba(qual) {
  const abrir = qual === "abrir";
  $("#abaCompactar").setAttribute("aria-selected", String(!abrir));
  $("#abaAbrir").setAttribute("aria-selected", String(abrir));
  $("#abaCompactar").tabIndex = abrir ? -1 : 0;
  $("#abaAbrir").tabIndex = abrir ? 0 : -1;
  $("#painelCompactar").hidden = abrir;
  $("#painelAbrir").hidden = !abrir;
}

// ================================================================ REPINTAR
/** Tudo o que tem texto e foi desenhado pelo JS. Chamado na troca de idioma
    -- sem ele, a tela ficaria metade num idioma, metade no outro. */
function repintar() {
  desenharIdiomas();
  aplicarTema(document.documentElement.getAttribute("data-tema") || "escuro");
  pintarCompactar();
  pintarAbrir();
  pintarRecadoAbrir();
  if ($("#espiar").open) pintarEspiar();
  for (const el of $$(".progresso [data-txt]")) el.textContent = t(el.getAttribute("data-txt"));
  pintarRodape();
}

function pintarRodape() {
  const e = est.estado;
  $("#endereco").textContent = location.host;
  $("#versao").textContent = e ? `${e.produto} ${e.versao}` : "";
}

// ================================================================ ARRANQUE
function ligar() {
  // Icones do HTML: `data-icone` vira o SVG constante.
  for (const el of $$("[data-icone]")) el.prepend(icone(el.getAttribute("data-icone")));

  $("#btTema").addEventListener("click", trocarTema);
  $("#btIdioma").addEventListener("click", () => ($("#menuIdiomas").hidden ? abrirMenuIdiomas() : fecharMenuIdiomas(true)));
  $("#menuIdiomas").addEventListener("keydown", e => {
    const itens = $$("#menuIdiomas li");
    const i = itens.indexOf(document.activeElement);
    if (e.key === "Escape") { e.preventDefault(); fecharMenuIdiomas(true); }
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const passo = e.key === "ArrowDown" ? 1 : -1;
      itens[(i + passo + itens.length) % itens.length].focus();
    } else if ((e.key === "Enter" || e.key === " ") && i >= 0) {
      e.preventDefault();
      const col = itens[i].dataset.idi;
      fecharMenuIdiomas(true);
      escolherIdioma(col);
    }
  });
  document.addEventListener("click", e => {
    if (!$("#menuIdiomas").hidden && !e.target.closest(".idioma")) fecharMenuIdiomas(false);
  });

  // Abas, com setas.
  for (const [id, qual] of [["#abaCompactar", "compactar"], ["#abaAbrir", "abrir"]]) {
    $(id).addEventListener("click", () => escolherAba(qual));
    $(id).addEventListener("keydown", e => {
      if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
      e.preventDefault();
      const outra = qual === "compactar" ? "abrir" : "compactar";
      escolherAba(outra);
      $(outra === "abrir" ? "#abaAbrir" : "#abaCompactar").focus();
    });
  }

  // Compactar.
  $("#btEscolherArquivos").addEventListener("click", () => $("#inArquivos").click());
  $("#btEscolherPasta").addEventListener("click", () => $("#inPasta").click());
  $("#inArquivos").addEventListener("change", e => {
    acrescentar([...e.target.files].map(f => deArquivo(f, f.name)));
    e.target.value = "";
  });
  $("#inPasta").addEventListener("change", e => {
    acrescentar([...e.target.files].map(f => deArquivo(f, f.webkitRelativePath || f.name)));
    e.target.value = "";
  });
  // Limpar a lista limpa a SENHA junto. Achado exercitando: ela ficava no
  // campo escondido (o bloco de opcoes some com a lista vazia), e a proxima
  // lista saia cifrada com uma senha que a pessoa nem via mais.
  $("#btLimpar").addEventListener("click", () => {
    est.itens = []; est.recadoCompactar = null; est.msgCompactar = null; est.nomeEditado = false;
    $("#inSenha").value = ""; $("#inSenha2").value = ""; $("#inNomePacote").value = "";
    pintarCompactar();
  });
  $("#inNomePacote").addEventListener("input", () => { est.nomeEditado = $("#inNomePacote").value.trim() !== ""; pintarCompactarBotao(); });
  for (const id of ["#inSenha", "#inSenha2"]) $(id).addEventListener("input", pintarOpcoes);
  $("#ckMostrar").addEventListener("change", () => {
    const tipo = $("#ckMostrar").checked ? "text" : "password";
    $("#inSenha").type = tipo;
    $("#inSenha2").type = tipo;
    pintarOpcoes();
  });
  $("#ckCifrarNomes").addEventListener("change", pintarCompactarBotao);
  $("#btCompactar").addEventListener("click", compactar);
  // Enter num campo de senha compacta -- e NAO submete nada: nao ha `<form>`
  // nesta pagina, de proposito. Um `<form>` sem `action` manda `GET ?senha=…`
  // para a propria pagina ao apertar Enter, e a senha cai na URL e no log.
  for (const id of ["#inSenha", "#inSenha2", "#inNomePacote"]) {
    $(id).addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); compactar(); } });
  }

  // Abrir.
  $("#btEscolherPacote").addEventListener("click", () => $("#inPacote").click());
  $("#inPacote").addEventListener("change", e => {
    const f = e.target.files[0];
    e.target.value = "";
    if (f) { est.recadoAbrir = null; pintarRecadoAbrir(); escolherPacote(f); }
  });
  $("#btAbrirComSenha").addEventListener("click", listar);
  $("#inSenhaPacote").addEventListener("keydown", e => {
    if (e.key === "Enter") { e.preventDefault(); if (!est.lista) listar(); }
  });
  $("#btTestar").addEventListener("click", testar);
  $("#btBaixarTodas").addEventListener("click", baixarTodas);
  $("#btFechar").addEventListener("click", () => fecharPacote(true));
  $("#inFiltro").addEventListener("input", e => { est.filtro = e.target.value; pintarLista(); });

  // Espiar.
  $("#btFecharEspiar").addEventListener("click", () => $("#espiar").close());
  $("#espiar").addEventListener("close", () => { est.espiado = null; $("#recadoEspiar").hidden = true; $("#espiarCorpo").replaceChildren(); });
  $("#btCopiar").addEventListener("click", copiarEspiado);
  $("#btBaixarEspiado").addEventListener("click", () => { const e = est.espiado && est.espiado.entrada; if (e) { $("#espiar").close(); baixarEntrada(e); } });

  ligarArrasto();
}

async function arrancar() {
  aplicarTema(lembrado(CHAVE_TEMA) || (matchMedia("(prefers-color-scheme: light)").matches ? "claro" : "escuro"));
  ligar();
  try {
    const r = await fetch("/api/estado", { cache: "no-store" });
    est.estado = await r.json();
    if (!est.estado || !est.estado.ok) throw new Error("estado");
    est.formato = est.estado.formatos[0];
    est.nivel = est.estado.niveis[est.estado.niveis.length - 1];
    await carregarIdioma(idiomaInicial(est.estado.idiomas || ["Portugues"]));
  } catch {
    // Sem o servidor nao ha texto -- e esta pagina nao carrega texto proprio
    // (seria a segunda fabrica). As chaves aparecem no lugar dos rotulos, o
    // que e feio e honesto: ninguem confunde a tela quebrada com a certa.
    document.body.classList.add("sem-servidor");
  }
  aplicarTextos();
  repintar();
  escolherAba("compactar");
  document.body.classList.remove("carregando");
}

document.addEventListener("DOMContentLoaded", arrancar);

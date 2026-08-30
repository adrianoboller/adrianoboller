/* O explorador da especificacao OpenAPI -- o "Swagger" desta casa.
 *
 * # Por que ele existe, e o numero que decidiu
 *
 * Medido nesta maquina, com o swagger-ui-dist 5.17.14 baixado de verdade:
 * embutir o `swagger-ui.css` + o `swagger-ui-bundle.js` no binario cresce o
 * `phxsqld` de 7.296.144 para 8.900.968 bytes -- 1,53 MiB a mais, +22,0%.
 * Com o preset standalone junto, 9.131.896 (+25,2%), e a medicao mostrou que
 * o crescimento e byte a byte o dos arquivos.
 *
 * Este arquivo faz o que aquele pacote faria de util aqui, e cabe em dezenas
 * de KiB. Apontar para uma CDN seria a terceira saida, e ela quebra o uso
 * OFFLINE -- que e justamente o caso da placa.
 *
 * # O que ele NAO faz, e por que
 *
 * Nao ha o "Try it out". A porta que documenta e a porta que executa sao
 * separadas de proposito, e um console executavel aqui exigiria abrir CORS da
 * porta REST para esta origem -- uma folga de seguranca que ninguem pediu,
 * so para nao copiar um `curl`. Entao ele MOSTRA o `curl` pronto, montado do
 * exemplo que ja vem na especificacao.
 *
 * # Todo texto de tela sai da fabrica de idiomas
 *
 * Nao ha uma palavra de portugues cravada aqui: o que aparece vem de `T`, que
 * e a resposta de `/idiomas` -- a MESMA que a interface web usa. E por isso
 * que trocar de idioma no seletor funciona sem nada mais. O que nao passa por
 * `T` e dado (nome de operacao, descricao vinda do catalogo), e dado nao se
 * traduz.
 */
"use strict";

var T = {};
var ESPEC = null;
var IDIOMAS_VIVOS = [];

function esc(s) {
  return String(s === null || s === undefined ? "" : s)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Um texto de tela. Sem a chave, o proprio nome dela -- que e feio e e
    honesto: aparece na tela que falta traduzir, em vez de sumir. */
function txt(nome, padrao) {
  return T[nome] || padrao || nome;
}

/** O idioma guardado neste navegador, o mesmo nome de chave da interface. */
function idiomaEscolhido() {
  try {
    return localStorage.getItem("phxsql-idioma") || "";
  } catch (e) {
    return "";
  }
}

function guardarIdioma(col) {
  try {
    localStorage.setItem("phxsql-idioma", col);
  } catch (e) { /* navegador sem armazenamento: a escolha vale nesta pagina */ }
}

function pegar(url) {
  return fetch(url, { headers: { Accept: "application/json" } })
    .then(function (r) { return r.json(); });
}

/** O `curl` pronto de uma operacao, montado do exemplo da especificacao. */
function curlDe(rota, corpo) {
  var linhas = [
    "curl -X POST " + location.protocol + "//SERVIDOR:PORTA/v1" + rota,
    "  -H 'Authorization: Bearer SEU-TOKEN'",
    "  -H 'Content-Type: application/json'",
  ];
  var texto = JSON.stringify(corpo || {});
  if (texto !== "{}") linhas.push("  -d '" + texto + "'");
  return linhas.join(" \\\n");
}

/** Um parametro do corpo, em linha. */
function linhaDoParametro(nome, prop, obrigatorio) {
  return "<tr><td><code>" + esc(nome) + "</code></td>" +
    "<td class=\"tipo\">" + esc(prop.type || "") + "</td>" +
    "<td>" + (obrigatorio
      ? "<b class=\"obr\">" + esc(txt("tela.api_obrigatorio")) + "</b>"
      : esc(txt("tela.opcional"))) + "</td>" +
    "<td>" + esc(prop.description || "") + "</td></tr>";
}

/** Uma operacao inteira. */
function blocoDaOperacao(rota, post) {
  var esquema = ((post.requestBody || {}).content || {})["application/json"] || {};
  var props = (esquema.schema || {}).properties || {};
  var obrigatorios = (esquema.schema || {}).required || [];
  var nomes = Object.keys(props);
  var tabela = nomes.length
    ? "<table class=\"par\"><thead><tr>" +
      "<th>" + esc(txt("tela.api_campo")) + "</th>" +
      "<th>" + esc(txt("tela.api_tipo")) + "</th>" +
      "<th></th>" +
      "<th>" + esc(txt("tela.api_para_que")) + "</th></tr></thead><tbody>" +
      nomes.map(function (n) {
        return linhaDoParametro(n, props[n], obrigatorios.indexOf(n) >= 0);
      }).join("") + "</tbody></table>"
    : "<p class=\"leg\">" + esc(txt("tela.api_sem_campo")) + "</p>";

  var apelidos = post["x-phxsql-apelidos"] || [];
  var permissao = post["x-phxsql-permissao"];
  var marcas =
    "<span class=\"marca " + (post["x-phxsql-escreve"] ? "grava" : "le") + "\">" +
      esc(post["x-phxsql-escreve"] ? txt("tela.api_grava") : txt("tela.api_so_le")) +
    "</span>" +
    (permissao
      ? "<span class=\"marca\">" + esc(txt("tela.api_permissao")) + ": <b>" +
        esc(permissao) + "</b></span>"
      : "<span class=\"marca\">" + esc(txt("tela.api_sem_permissao")) + "</span>") +
    (apelidos.length
      ? "<span class=\"marca\">" + esc(txt("tela.api_apelidos")) + ": " +
        apelidos.map(esc).join(", ") + "</span>"
      : "");

  return "<details class=\"op\" data-rota=\"" + esc(rota) + "\">" +
    "<summary><code class=\"rota\">POST /v1" + esc(rota) + "</code>" +
    "<span class=\"resumo\">" + esc(post.summary || "") + "</span></summary>" +
    "<div class=\"corpo\">" + marcas + tabela +
    "<h4>" + esc(txt("tela.api_exemplo")) + "</h4>" +
    "<pre class=\"curl\">" + esc(curlDe(rota, esquema.example)) + "</pre>" +
    "</div></details>";
}

/** Desenha a especificacao inteira. */
function desenhar() {
  if (!ESPEC) return;
  var info = ESPEC.info || {};
  document.getElementById("titulo").textContent = info.title || "";
  document.getElementById("versao").textContent = info.version || "";
  document.getElementById("resumo").innerHTML = marcado(info.description || "");

  var caminhos = ESPEC.paths || {};
  var filtro = (document.getElementById("busca").value || "").toLowerCase();
  var rotas = Object.keys(caminhos).filter(function (r) {
    if (!filtro) return true;
    var p = caminhos[r].post || {};
    return (r + " " + (p.summary || "") + " " +
            (p["x-phxsql-apelidos"] || []).join(" ")).toLowerCase().indexOf(filtro) >= 0;
  });
  document.getElementById("conta").textContent = rotas.length;
  document.getElementById("lista").innerHTML = rotas.length
    ? rotas.map(function (r) { return blocoDaOperacao(r, caminhos[r].post || {}); }).join("")
    : "<p class=\"vazio\">" + esc(txt("tela.api_nada_achado")) + "</p>";

  var seg = ((ESPEC.components || {}).securitySchemes) || {};
  document.getElementById("seguranca").innerHTML = Object.keys(seg).map(function (n) {
    return "<li><code>" + esc(n) + "</code> — " + esc(seg[n].description || "") + "</li>";
  }).join("");
}

/** A enfase das mensagens da fabrica: `**assim**` e a palavra entre crases.
    O texto e escapado ANTES, pelo mesmo motivo da interface: ele vem de uma
    tabela que um administrador edita pela grade. */
function marcado(bruto) {
  return esc(bruto)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/\n\n/g, "<br><br>");
}

/** Os rotulos fixos da moldura, todos pela fabrica. */
function aplicarIdioma() {
  var pares = [
    ["rotBusca", "tela.api_buscar"],
    ["rotSeguranca", "tela.api_seguranca"],
    ["rotOperacoes", "tela.api_operacoes"],
    ["rotEspec", "tela.api_baixar"],
    ["rotIdioma", "tela.idioma_da_interface"],
    ["rotAviso", "tela.api_aviso_claro"],
  ];
  for (var i = 0; i < pares.length; i++) {
    var el = document.getElementById(pares[i][0]);
    if (el) el.textContent = txt(pares[i][1]);
  }
  var b = document.getElementById("busca");
  if (b) b.placeholder = txt("tela.api_buscar");
}

function carregarIdioma(col) {
  var url = "/idiomas" + (col ? "?idioma=" + encodeURIComponent(col) : "");
  return pegar(url).then(function (r) {
    T = r.textos || {};
    IDIOMAS_VIVOS = r.idiomas || [];
    var sel = document.getElementById("idioma");
    if (sel && !sel.options.length) {
      sel.innerHTML = IDIOMAS_VIVOS.map(function (i) {
        return "<option value=\"" + esc(i) + "\">" + esc(i) + "</option>";
      }).join("");
    }
    if (sel) sel.value = r.idioma;
    aplicarIdioma();
    desenhar();
  });
}

window.addEventListener("DOMContentLoaded", function () {
  document.getElementById("busca").addEventListener("input", desenhar);
  document.getElementById("idioma").addEventListener("change", function (ev) {
    guardarIdioma(ev.target.value);
    carregarIdioma(ev.target.value);
  });
  pegar("/openapi.json").then(function (s) {
    ESPEC = s;
    return carregarIdioma(idiomaEscolhido());
  });
});

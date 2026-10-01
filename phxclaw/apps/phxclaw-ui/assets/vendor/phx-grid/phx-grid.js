/* phx-grid v0.1.0 — Núcleo (S01): colunas + render + fonte + paginação
   Phoenix / WX Soluções — ES5 estrito, zero dependências. */
(function (root) {
  "use strict";

  function esc(s) {
    return String(s == null ? "" : s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }
  function el(tag, cls, html) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html != null) e.innerHTML = html;
    return e;
  }
  function agora() { return (root.performance && performance.now) ? performance.now() : Date.now(); }

  var fmt = {
    numero: function (v, dec) {
      /* nao-finito nunca deve ser formatado: o separador de milhar transformava
         Infinity na string "In.fin.ity" na tela (achado pelo fuzz, v0.73) */
      if (v === Infinity || v === -Infinity) return "";
      if (v == null || v !== v) return "";
      var n = Number(v).toFixed(dec == null ? 0 : dec);
      var p = n.split("."), i = p[0], neg = i.charAt(0) === "-";
      if (neg) i = i.slice(1);
      var out = "", k = 0, j;
      for (j = i.length - 1; j >= 0; j--) {
        out = i.charAt(j) + out;
        if (++k % 3 === 0 && j > 0) out = "." + out;
      }
      return (neg ? "-" : "") + out + (p[1] ? "," + p[1] : "");
    },
    moeda: function (v) { return v == null ? "" : "R$ " + fmt.numero(v, 2); },
    percentual: function (v) { return v == null ? "" : fmt.numero(v, 1) + "%"; },
    dataHora: function (v) {
      var d = v instanceof Date ? v : new Date(v);
      if (isNaN(d.getTime())) return esc(v);
      function z(n) { return (n < 10 ? "0" : "") + n; }
      return z(d.getDate()) + "/" + z(d.getMonth() + 1) + "/" + d.getFullYear() +
        " " + z(d.getHours()) + ":" + z(d.getMinutes()) + ":" + z(d.getSeconds());
    },
    data: function (v) { return fmt.dataHora(v).slice(0, 10); }
  };

  function chave(v, tipo) {
    if (v == null) return { n: true, v: 0 };
    if (tipo === "numero" || tipo === "moeda" || tipo === "percentual") return { n: false, v: Number(v) };
    if (tipo === "dataHora" || tipo === "data") {
      var d = v instanceof Date ? v : new Date(v);
      return { n: false, v: d.getTime() };
    }
    return { n: false, v: String(v).toLowerCase() };
  }
  function ordenaEstavelMulti(linhas, ordens) {
    var marcadas = new Array(linhas.length), i, j;
    for (i = 0; i < linhas.length; i++) {
      var ks = [];
      for (j = 0; j < ordens.length; j++) ks.push(chave(linhas[i][ordens[j].campo], ordens[j].tipo));
      marcadas[i] = { r: linhas[i], ks: ks, ix: i };
    }
    marcadas.sort(function (a, b) {
      var j2, ka, kb, dir2;
      for (j2 = 0; j2 < ordens.length; j2++) {
        ka = a.ks[j2]; kb = b.ks[j2]; dir2 = ordens[j2].dir;
        if (ka.n && kb.n) continue;
        if (ka.n) return 1;
        if (kb.n) return -1;
        if (ka.v < kb.v) return dir2 === "asc" ? -1 : 1;
        if (ka.v > kb.v) return dir2 === "asc" ? 1 : -1;
      }
      return a.ix - b.ix;
    });
    var out = new Array(linhas.length);
    for (i = 0; i < marcadas.length; i++) out[i] = marcadas[i].r;
    return out;
  }
  function ordenaEstavel(linhas, campo, dir, tipo) {
    var marcadas = new Array(linhas.length), i;
    for (i = 0; i < linhas.length; i++) marcadas[i] = { r: linhas[i], k: chave(linhas[i][campo], tipo), ix: i };
    marcadas.sort(function (a, b) {
      if (a.k.n && b.k.n) return a.ix - b.ix;
      if (a.k.n) return 1;
      if (b.k.n) return -1;
      if (a.k.v < b.k.v) return dir === "asc" ? -1 : 1;
      if (a.k.v > b.k.v) return dir === "asc" ? 1 : -1;
      return a.ix - b.ix;
    });
    var out = new Array(linhas.length);
    for (i = 0; i < linhas.length; i++) out[i] = marcadas[i].r;
    return out;
  }

  var MAPA_ACENTOS = { "\u00e1":"a","\u00e0":"a","\u00e2":"a","\u00e3":"a","\u00e4":"a","\u00e9":"e","\u00e8":"e","\u00ea":"e","\u00eb":"e","\u00ed":"i","\u00ec":"i","\u00ee":"i","\u00ef":"i","\u00f3":"o","\u00f2":"o","\u00f4":"o","\u00f5":"o","\u00f6":"o","\u00fa":"u","\u00f9":"u","\u00fb":"u","\u00fc":"u","\u00e7":"c","\u00f1":"n" };
  var RE_ACENTOS = (function () {
    var k2, cls = "";
    for (k2 in MAPA_ACENTOS) cls += k2;
    return new RegExp("[" + cls + "]", "g");
  })();
  function trocaAcento(ch) { return MAPA_ACENTOS[ch] || ch; }
  function semAcento(t) {
    return String(t == null ? "" : t).toLowerCase().replace(RE_ACENTOS, trocaAcento);
  }
  function chaveOrd(v, tipo) { return chave(v, tipo); }
  function passaCondicao(linha, f) {
    var v = linha[f.campo], j2;
    if (f.tipo === "valores") {
      if (v == null || v === "") return !!f.incluiNulos;
      for (j2 = 0; j2 < f.valores.length; j2++) if (String(v) === String(f.valores[j2])) return true;
      return false;
    }
    if (f.tipo === "busca") {
      termosBusca(f);
      if (!f._termos.length) return true;
      var jt, jc, achou, sv;
      for (jt = 0; jt < f._termos.length; jt++) {
        achou = false;
        for (jc = 0; jc < f.campos.length; jc++) {
          sv = linha[f.campos[jc]];
          if (sv != null && semAcento(sv).indexOf(f._termos[jt]) >= 0) { achou = true; break; }
        }
        if (!achou) return false;
      }
      return true;
    }
    if (f.tipo === "texto") {
      if (f._q == null) f._q = semAcento(f.contem);
      return semAcento(v).indexOf(f._q) >= 0;
    }
    if (f.tipo === "faixa") {
      var k2 = chaveOrd(v, f.tipoCol);
      if (k2.n) return false;
      if (f._kde == null && f.de != null) f._kde = chaveOrd(f.de, f.tipoCol);
      if (f._kate == null && f.ate != null) f._kate = chaveOrd(f.ate, f.tipoCol);
      if (f.de != null && k2.v < f._kde.v) return false;
      if (f.ate != null && k2.v > f._kate.v) return false;
      return true;
    }
    if (f.tipo === "expr") {
      if (f._kf == null) f._kf = chaveOrd(f.valor, f.tipoCol);
      return passaExpr(v, f.op, f._kf, f.tipoCol);
    }
    if (f.tipo === "multi") {
      var todas = true, alguma = false, ci2;
      for (j2 = 0; j2 < f.condicoes.length; j2++) {
        ci2 = f.condicoes[j2];
        if (ci2._kf == null) ci2._kf = chaveOrd(ci2.valor, f.tipoCol);
        if (passaExpr(v, ci2.op, ci2._kf, f.tipoCol)) alguma = true;
        else todas = false;
      }
      return f.combinador === "ou" ? alguma : todas;
    }
    return true;
  }
  function passaExpr(v, op, kf, tipoCol) {
    var kv = chaveOrd(v, tipoCol);
    if (kv.n) return op === "!=";
    if (op === ">") return kv.v > kf.v;
    if (op === ">=") return kv.v >= kf.v;
    if (op === "<") return kv.v < kf.v;
    if (op === "<=") return kv.v <= kf.v;
    if (op === "=") return kv.v === kf.v;
    if (op === "!=") return kv.v !== kf.v;
    return true;
  }
  var CUSTO_COND = { valores: 1, faixa: 1, expr: 1, multi: 2, texto: 9, busca: 10 };
  function termosBusca(f) {
    if (f._termos == null) {
      f._termos = [];
      var tt = String(f.termo || "").split(/\s+/), j3;
      for (j3 = 0; j3 < tt.length; j3++) if (tt[j3]) f._termos.push(semAcento(tt[j3]));
    }
    return f._termos;
  }
  function scoreBusca(linha, f) {
    termosBusca(f);
    if (!f._termos.length) return 0;
    var sc = 0, jt, jc, sv;
    for (jt = 0; jt < f._termos.length; jt++)
      for (jc = 0; jc < f.campos.length; jc++) {
        sv = linha[f.campos[jc]];
        if (sv != null && semAcento(sv).indexOf(f._termos[jt]) >= 0) sc++;
      }
    return sc;
  }
  /* passada única: normaliza cada campo 1x por linha, decide (todo termo em >=1 campo) e pontua juntos */
  function filtraEScoreBusca(linhas, f, idx, mapaIx) {
    termosBusca(f);
    var out = [], i2, jt, jc, sv, norm, sc, falhou, nT = f._termos.length, nC = f.campos.length, hitTermo, gi;
    for (i2 = 0; i2 < linhas.length; i2++) {
      gi = mapaIx ? mapaIx[i2] : -1;
      sc = 0; falhou = false;
      var normas = [], temNorma = [];
      for (jt = 0; jt < nT && !falhou; jt++) {
        hitTermo = false;
        for (jc = 0; jc < nC; jc++) {
          if (idx && gi >= 0) norm = idx[jc][gi];
          else {
            if (!temNorma[jc]) {
              sv = linhas[i2][f.campos[jc]];
              normas[jc] = sv == null ? null : semAcento(sv);
              temNorma[jc] = true;
            }
            norm = normas[jc];
          }
          if (norm !== null && norm.indexOf(f._termos[jt]) >= 0) { hitTermo = true; sc++; }
        }
        if (!hitTermo) falhou = true;
      }
      if (!falhou) out.push({ l: linhas[i2], s: sc, ix: i2 });
    }
    return out;
  }
  function aplicaFiltros(dados, lista) {
    if (!lista || !lista.length) return dados;
    /* predicate ordering: condições baratas primeiro cortam o dataset antes das caras (texto) */
    var ordenada = lista.slice().sort(function (a, b) {
      return (CUSTO_COND[a.tipo] || 5) - (CUSTO_COND[b.tipo] || 5);
    });
    var out = [], i2, j2, okL;
    for (i2 = 0; i2 < dados.length; i2++) {
      okL = true;
      for (j2 = 0; j2 < ordenada.length; j2++) if (!passaCondicao(dados[i2], ordenada[j2])) { okL = false; break; }
      if (okL) out.push(dados[i2]);
    }
    return out;
  }

  function hrefSeguro(u) {
    u = String(u == null ? "#" : u);
    return (/^https?:\/\//i.test(u) || u.charAt(0) === "#") ? u : "#";
  }
  var CORES_BADGE = { verde: 1, ambar: 1, azul: 1, vermelho: 1, cinza: 1 };
  /* GroupFormat/TotalRowFormat: devolve a coluna com o override de formato aplicado por cima
     (copia rasa -- nunca muta a definicao original da coluna). Sem override, devolve a propria. */
  function colFormato(c9, chave9) {
    var ov9 = c9 && c9[chave9];
    if (!ov9) return c9;
    var out9 = {}, k9;
    for (k9 in c9) if (Object.prototype.hasOwnProperty.call(c9, k9)) out9[k9] = c9[k9];
    for (k9 in ov9) if (Object.prototype.hasOwnProperty.call(ov9, k9)) out9[k9] = ov9[k9];
    return out9;
  }
  /* ===== FONTE DE VERDADE UNICA DA FORMATACAO =====
     valorExibido(c, v) resolve, em UM lugar so, as duas regras que valem para QUALQUER
     representacao do valor (celula, dica, linha de preview, medicao de largura, export):
       - ReplaceValues (c.valores): mapa valor -> rotulo
       - NullBehavior (c.nulo): placeholder de vazio/nulo
     Devolve {texto} quando resolveu, ou null quando o valor deve seguir o fluxo normal do tipo.

     Motivo de existir: formata() e textoVisivel() eram dois caminhos paralelos com regras
     duplicadas. O `nulo` chegou a funcionar num e nao no outro (bug v0.66) e o `valores`
     idem (bug v0.71) -- os dois pelo mesmo motivo. Regra nova entra AQUI e vale nos dois. */
  function valorExibido(c, v) {
    if (c.valores && Object.prototype.hasOwnProperty.call(c.valores, v)) return { texto: String(c.valores[v]) };
    if (c.nulo != null && (v == null || v === "")) return { texto: String(c.nulo) };
    return null;
  }
  function formata(c, v, linha, ixL) {
    var ve9 = valorExibido(c, v);
    if (ve9) return esc(ve9.texto);
    if (c.formato) return c.formato(v, linha);
    if (c.tipo === "checkbox") return '<span class="phx-chk' + (v === true || v === 1 || v === "1" || v === "S" || v === "sim" ? " phx-chk-on" : "") + '" data-chk="' + esc(c.campo) + '">' + (v === true || v === 1 || v === "1" || v === "S" || v === "sim" ? "\u2611" : "\u2610") + "</span>";
    if (c.tipo === "imagem") {
      var src9 = c.imagens && c.imagens[v] != null ? c.imagens[v] : (v == null ? "" : String(v));
      return src9 ? '<img class="phx-img" src="' + esc(src9) + '" alt="' + esc(String(v == null ? "" : v)) + '"' + (c.altura ? ' style="height:' + c.altura + 'px"' : "") + ">" : "";
    }
    if (c.tipo === "botao") return '<button type="button" class="phx-btn-cel" data-btn="' + esc(c.campo) + '">' + esc(c.rotulo || (v == null ? "" : String(v))) + "</button>";
    if (c.tipo === "moeda") return fmt.moeda(v);
    if (c.tipo === "percentual") return fmt.percentual(v);
    if (c.tipo === "numero") return fmt.numero(v, c.decimais);
    if (c.tipo === "dataHora") return fmt.dataHora(v);
    if (c.tipo === "data") return fmt.data(v);
    if (c.tipo === "composta") {
      return '<span class="phx-composta"><span class="phx-composta-main">' + esc(v) + "</span>" +
        (c.sub ? '<span class="phx-composta-sub">' + esc((c.subPrefixo || "") + (linha[c.sub] == null ? "" : linha[c.sub])) + "</span>" : "") + "</span>";
    }
    if (c.tipo === "link") {
      var u = c.href ? c.href(linha) : (c.url ? linha[c.url] : "#");
      return '<a class="phx-link" href="' + esc(hrefSeguro(u)) + '">' + esc(v) + "</a>";
    }
    if (c.tipo === "badge") {
      var cor = (c.cores && c.cores[v]) || "cinza";
      if (!CORES_BADGE[cor]) cor = "cinza";
      return '<span class="phx-badge phx-badge-' + cor + '">' + esc(v) + "</span>";
    }
    if (c.tipo === "barra") {
      var max = c.max || 100;
      var pct = Math.max(0, Math.min(100, (Number(v) / max) * 100));
      return '<span class="phx-barra-envoltorio"><span class="phx-barra"><span class="phx-barra-fill" style="width:' + pct.toFixed(1) + '%"></span></span>' +
        '<span class="phx-barra-rotulo">' + fmt.percentual(v) + "</span></span>";
    }
    if (c.tipo === "json") {
      return '<button type="button" class="phx-json-btn" data-jl="' + ixL + '" data-jc="' + esc(c.campo) + '">{\u2026}</button>';
    }
    return esc(v);
  }


  function textoCelula(c, v, linha) {
    if (c && c.acumulado && linha && linha.__acum && linha.__acum[c.campo] != null) v = linha.__acum[c.campo];
    var tipo = (c && c.tipo) || "texto";
    if (v == null) return "";
    if (tipo === "moeda" || tipo === "numero" || tipo === "percentual" || tipo === "barra") {
      var n = Number(v);
      if (n !== n) return String(v);
      return String(n).replace(".", ",");
    }
    if (tipo === "data" || tipo === "dataHora") {
      var d2 = new Date(String(v).length === 10 ? v + "T00:00:00" : v);
      if (isNaN(d2)) return String(v);
      var dd = ("0" + d2.getDate()).slice(-2), mm = ("0" + (d2.getMonth() + 1)).slice(-2);
      var base = dd + "/" + mm + "/" + d2.getFullYear();
      if (tipo === "dataHora") base += " " + ("0" + d2.getHours()).slice(-2) + ":" + ("0" + d2.getMinutes()).slice(-2);
      return base;
    }
    if (tipo === "link") return String((c.url && linha[c.url]) || (c.href ? c.href(v, linha) : v));
    if (tipo === "composta") return String(v);
    if (tipo === "json") { try { return JSON.stringify(v); } catch (e2) { return String(v); } }
    return String(v);
  }
  /* ===== O4-D: ARRASTE POR PONTEIRO/TOQUE (iPad, WebView antigo) ===== */
  function arrastePonteiro(el, opts) {
    var ativo = null, timer = null, x0 = 0, y0 = 0, fantasma = null;
    function ponto(ev) {
      var t = (ev.touches && ev.touches.length) ? ev.touches[0] : ((ev.changedTouches && ev.changedTouches.length) ? ev.changedTouches[0] : ev);
      return { x: t.clientX || 0, y: t.clientY || 0 };
    }
    function comeca(p) {
      ativo = { x: p.x, y: p.y };
      fantasma = document.createElement("div");
      fantasma.className = "phx-arraste-fantasma";
      fantasma.textContent = opts.rotulo ? opts.rotulo() : (el.textContent || "").slice(0, 40);
      (sobe(el, "phx-grid") || sobe(el, "phx-cubo") || document.body).appendChild(fantasma);
      move(p);
      if (opts.aoIniciar) opts.aoIniciar();
    }
    function move(p) {
      if (!fantasma) return;
      fantasma.style.left = (p.x + 12) + "px";
      fantasma.style.top = (p.y + 12) + "px";
      var sob = document.elementFromPoint ? document.elementFromPoint(p.x, p.y) : null;
      if (opts.aoMover) opts.aoMover(sob, p);
    }
    function termina(ev) {
      if (timer) { clearTimeout(timer); timer = null; }
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.removeEventListener("touchmove", onMove);
      document.removeEventListener("touchend", onUp);
      document.removeEventListener("touchcancel", onUp);
      if (!ativo) return;
      var p = ponto(ev);
      if (fantasma && fantasma.parentNode) fantasma.parentNode.removeChild(fantasma);
      fantasma = null;
      var sob = document.elementFromPoint ? document.elementFromPoint(p.x, p.y) : null;
      ativo = null;
      if (opts.aoSoltar) opts.aoSoltar(sob, p);
    }
    function onMove(ev) {
      var p = ponto(ev);
      if (!ativo) {
        if (ev.touches) {
          /* dedo andou antes do toque longo: e rolagem, nao arraste */
          if (timer && Math.abs(p.x - x0) + Math.abs(p.y - y0) > 10) { clearTimeout(timer); timer = null; termina(ev); }
          return;
        }
        if (Math.abs(p.x - x0) + Math.abs(p.y - y0) < 6) return;
        comeca(p);
      }
      if (ev.touches) ev.preventDefault();
      move(p);
    }
    function onUp(ev) { termina(ev); }
    function onDown(ev) {
      if (ev.button != null && ev.button !== 0 && !ev.touches) return;
      if (ev.target && /^(INPUT|SELECT|BUTTON|TEXTAREA)$/.test(ev.target.tagName)) return;
      if (ev.target && ev.target.className && String(ev.target.className).indexOf("phx-col-resz") >= 0) return;
      var p = ponto(ev); x0 = p.x; y0 = p.y;
      /* com mouse, o DnD HTML5 nativo cuida (Chromium, WebView2, IE10+); o ponteiro entra so no toque ou onde nao ha draggable */
      if (!ev.touches && ("draggable" in el) && el.draggable && !opts.forcarMouse) return;
      if (ev.touches) {
        timer = setTimeout(function () { timer = null; comeca(p); }, opts.toqueLongo || 350);
        document.addEventListener("touchmove", onMove, { passive: false });
        document.addEventListener("touchend", onUp);
        document.addEventListener("touchcancel", onUp);
      } else {
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
      }
    }
    el.addEventListener("mousedown", onDown);
    el.addEventListener("touchstart", onDown, { passive: true });
    return { destruir: function () { el.removeEventListener("mousedown", onDown); el.removeEventListener("touchstart", onDown); } };
  }
  function sobe(el, cls) { while (el && el.nodeType === 1) { if ((" " + el.className + " ").indexOf(" " + cls + " ") >= 0) return el; el = el.parentNode; } return null; }

  /* ===== O4-1: LOCALIZATION ===== */
  var TEXTOS = {
    itensPorPagina: "itens por p\u00e1gina", arrasteAgrupar: "Arraste uma coluna para c\u00e1 para agrupar", linhas: "linhas",
    irPara: "ir para", pagina: "P\u00e1gina", registro: "Registro", totais: "Total",
    camposRelatorio: "Campos do relat\u00f3rio", buscar: "Buscar", adiarLayout: "Adiar atualiza\u00e7\u00e3o", atualizar: "Atualizar", de: "de", registros: "registros", mostrando: "Mostrando", colunas: "Colunas"
  };
  function T(chave) { return TEXTOS[chave] == null ? chave : TEXTOS[chave]; }
  function definirTextos(mapa) { var k; for (k in (mapa || {})) if (Object.prototype.hasOwnProperty.call(mapa, k)) TEXTOS[k] = String(mapa[k]); return TEXTOS; }

  /* ===== O4-C: CARGA DE ARQUIVOS (JSON / CSV / XML / HTML / XLSX) ===== */
  /* INFLATE (RFC 1951) em ES5 puro \u2014 blocos stored, Huffman fixo e din\u00e2mico */
  function inflate(bytes) {
    var pos = 0, bit = 0, out = [], LEN = bytes.length;
    function lerBits(n) {
      var v = 0, i;
      for (i = 0; i < n; i++) {
        if (pos >= LEN) throw new Error("inflate: fim inesperado");
        v |= ((bytes[pos] >> bit) & 1) << i;
        bit++; if (bit === 8) { bit = 0; pos++; }
      }
      return v;
    }
    function montaHuff(comprimentos) {
      var maxL = 0, i, bl = [], next = [], codigos = {}, code = 0;
      for (i = 0; i < comprimentos.length; i++) if (comprimentos[i] > maxL) maxL = comprimentos[i];
      for (i = 0; i <= maxL; i++) bl[i] = 0;
      for (i = 0; i < comprimentos.length; i++) bl[comprimentos[i]]++;
      bl[0] = 0;
      for (i = 1; i <= maxL; i++) { code = (code + bl[i - 1]) << 1; next[i] = code; }
      for (i = 0; i < comprimentos.length; i++) if (comprimentos[i]) codigos[comprimentos[i] + ":" + (next[comprimentos[i]]++)] = i;
      return { codigos: codigos, maxL: maxL };
    }
    function decodifica(h) {
      var code = 0, len = 0, s9;
      while (len < h.maxL) {
        code = (code << 1) | lerBits(1); len++;
        s9 = h.codigos[len + ":" + code];
        if (s9 !== undefined) return s9;
      }
      throw new Error("inflate: c\u00f3digo inv\u00e1lido");
    }
    var LBASE = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
    var LEXT = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
    var DBASE = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
    var DEXT = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
    var fixoL = null, fixoD = null;
    function blocoHuff(hL, hD) {
      for (;;) {
        var s9 = decodifica(hL);
        if (s9 < 256) out.push(s9);
        else if (s9 === 256) return;
        else {
          var li = s9 - 257, len = LBASE[li] + lerBits(LEXT[li]);
          var di = decodifica(hD), dist = DBASE[di] + lerBits(DEXT[di]);
          var from = out.length - dist, k;
          for (k = 0; k < len; k++) out.push(out[from + k]);
        }
      }
    }
    var fim = 0;
    while (!fim) {
      fim = lerBits(1);
      var tipo = lerBits(2);
      if (tipo === 0) {
        bit = 0; pos++;
        var len = bytes[pos] | (bytes[pos + 1] << 8); pos += 4;
        var k;
        for (k = 0; k < len; k++) out.push(bytes[pos++]);
      } else if (tipo === 1) {
        if (!fixoL) {
          var cl = [], i;
          for (i = 0; i < 288; i++) cl.push(i < 144 ? 8 : (i < 256 ? 9 : (i < 280 ? 7 : 8)));
          fixoL = montaHuff(cl);
          var cd = []; for (i = 0; i < 30; i++) cd.push(5);
          fixoD = montaHuff(cd);
        }
        blocoHuff(fixoL, fixoD);
      } else if (tipo === 2) {
        var hlit = lerBits(5) + 257, hdist = lerBits(5) + 1, hclen = lerBits(4) + 4;
        var ORD = [16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15], cls = [], j;
        for (j = 0; j < 19; j++) cls[j] = 0;
        for (j = 0; j < hclen; j++) cls[ORD[j]] = lerBits(3);
        var hc = montaHuff(cls), comp = [], prev = 0;
        while (comp.length < hlit + hdist) {
          var sym = decodifica(hc);
          if (sym < 16) { comp.push(sym); prev = sym; }
          else if (sym === 16) { var r16 = 3 + lerBits(2); while (r16--) comp.push(prev); }
          else if (sym === 17) { var r17 = 3 + lerBits(3); while (r17--) comp.push(0); prev = 0; }
          else { var r18 = 11 + lerBits(7); while (r18--) comp.push(0); prev = 0; }
        }
        blocoHuff(montaHuff(comp.slice(0, hlit)), montaHuff(comp.slice(hlit)));
      } else throw new Error("inflate: tipo de bloco inv\u00e1lido");
    }
    return out;
  }
  function utf8Texto(bytes) {
    var s9 = "", i = 0, c, c2, c3, c4;
    while (i < bytes.length) {
      c = bytes[i++];
      if (c < 128) s9 += String.fromCharCode(c);
      else if (c < 224) { c2 = bytes[i++]; s9 += String.fromCharCode(((c & 31) << 6) | (c2 & 63)); }
      else if (c < 240) { c2 = bytes[i++]; c3 = bytes[i++]; s9 += String.fromCharCode(((c & 15) << 12) | ((c2 & 63) << 6) | (c3 & 63)); }
      else { c2 = bytes[i++]; c3 = bytes[i++]; c4 = bytes[i++]; var cp = ((c & 7) << 18) | ((c2 & 63) << 12) | ((c3 & 63) << 6) | (c4 & 63); cp -= 0x10000; s9 += String.fromCharCode(0xD800 + (cp >> 10), 0xDC00 + (cp & 1023)); }
    }
    return s9;
  }
  function leZip(u8) {
    var entradas = {}, n = u8.length, i;
    /* central directory pelo EOCD */
    for (i = n - 22; i >= 0 && i > n - 70000; i--) if (u8[i] === 0x50 && u8[i + 1] === 0x4B && u8[i + 2] === 0x05 && u8[i + 3] === 0x06) break;
    if (i < 0) throw new Error("zip: EOCD n\u00e3o encontrado");
    function u16(p) { return u8[p] | (u8[p + 1] << 8); }
    function u32(p) { return (u8[p] | (u8[p + 1] << 8) | (u8[p + 2] << 16)) + u8[p + 3] * 16777216; }
    var qtd = u16(i + 10), cd = u32(i + 16), k, p = cd;
    for (k = 0; k < qtd; k++) {
      if (u32(p) !== 0x02014B50) break;
      var metodo = u16(p + 10), tamC = u32(p + 20), tamD = u32(p + 24), nLen = u16(p + 28), eLen = u16(p + 30), cLen = u16(p + 32), off = u32(p + 42);
      var nome = utf8Texto(Array.prototype.slice.call(u8, p + 46, p + 46 + nLen));
      var lnLen = u16(off + 26), leLen = u16(off + 28), ini = off + 30 + lnLen + leLen;
      entradas[nome] = { metodo: metodo, ini: ini, tamC: tamC, tamD: tamD };
      p += 46 + nLen + eLen + cLen;
    }
    return {
      nomes: function () { var o = [], k2; for (k2 in entradas) o.push(k2); return o; },
      texto: function (nome) {
        var e = entradas[nome];
        if (!e) return null;
        var bruto = Array.prototype.slice.call(u8, e.ini, e.ini + e.tamC);
        if (e.metodo === 8) bruto = inflate(bruto);
        else if (e.metodo !== 0) throw new Error("zip: m\u00e9todo n\u00e3o suportado " + e.metodo);
        return utf8Texto(bruto);
      }
    };
  }
  function _DP() {
    if (typeof DOMParser !== "undefined") return DOMParser;
    if (typeof window !== "undefined" && window.DOMParser) return window.DOMParser;
    throw new Error("DOMParser indispon\u00edvel");
  }
  function xmlDoc(texto) {
    var P = _DP();
    return new P().parseFromString(texto, "application/xml");
  }
  function colIndice(ref) {
    var m = String(ref).match(/^([A-Z]+)/), n = 0, i;
    if (!m) return 0;
    for (i = 0; i < m[1].length; i++) n = n * 26 + (m[1].charCodeAt(i) - 64);
    return n - 1;
  }
  function serialParaISO(n) {
    var ms = Math.round(n * 86400000) + Date.UTC(1899, 11, 30);
    var d = new Date(ms);
    function p2(x) { return x < 10 ? "0" + x : String(x); }
    return d.getUTCFullYear() + "-" + p2(d.getUTCMonth() + 1) + "-" + p2(d.getUTCDate());
  }
  function inferePtBR(v) {
    if (v == null) return null;
    var t = String(v).replace(/^\s+|\s+$/g, "");
    if (t === "") return null;
    if (/^-?\d{1,3}(\.\d{3})*(,\d+)?$/.test(t) || /^-?\d+(,\d+)?$/.test(t)) return Number(t.replace(/\./g, "").replace(",", "."));
    if (/^-?\d+(\.\d+)?$/.test(t)) return Number(t);
    var md = t.match(/^(\d{2})\/(\d{2})\/(\d{4})$/);
    if (md) return md[3] + "-" + md[2] + "-" + md[1];
    if (/^\d{4}-\d{2}-\d{2}/.test(t)) return t.slice(0, 10);
    if (t === "true" || t === "false") return t === "true";
    return t;
  }
  var carregar = {
    json: function (entrada) {
      var o = typeof entrada === "string" ? JSON.parse(entrada) : entrada, k;
      if (o instanceof Array) return o;
      if (o && typeof o === "object") {
        if (o.dados instanceof Array) return o.dados;
        if (o.rows instanceof Array) return o.rows;
        if (o.data instanceof Array) return o.data;
        for (k in o) if (o[k] instanceof Array) return o[k];
      }
      return [];
    },
    csv: function (texto, opts) {
      opts = opts || {};
      var t = String(texto || "").replace(/^\uFEFF/, "");
      var sep = opts.separador;
      if (!sep) {
        var l1 = t.split(/\r?\n/)[0] || "";
        var cont = { ";": (l1.match(/;/g) || []).length, ",": (l1.match(/,/g) || []).length, "\t": (l1.match(/\t/g) || []).length, "|": (l1.match(/\|/g) || []).length };
        sep = ";"; var mx = -1, k;
        for (k in cont) if (cont[k] > mx) { mx = cont[k]; sep = k; }
      }
      var linhas = [], campo = "", linha = [], i = 0, n = t.length, c, dentro = false;
      while (i < n) {
        c = t.charAt(i);
        if (dentro) {
          if (c === '"') { if (t.charAt(i + 1) === '"') { campo += '"'; i += 2; continue; } dentro = false; i++; continue; }
          campo += c; i++; continue;
        }
        if (c === '"' && campo === "") { dentro = true; i++; continue; }
        if (c === sep) { linha.push(campo); campo = ""; i++; continue; }
        if (c === "\r") { i++; continue; }
        if (c === "\n") { linha.push(campo); linhas.push(linha); linha = []; campo = ""; i++; continue; }
        campo += c; i++;
      }
      if (campo !== "" || linha.length) { linha.push(campo); linhas.push(linha); }
      if (!linhas.length) return [];
      var cab = opts.cabecalho === false ? null : linhas.shift();
      var out = [], j, m, o;
      for (j = 0; j < linhas.length; j++) {
        if (linhas[j].length === 1 && linhas[j][0] === "") continue;
        o = {};
        for (m = 0; m < linhas[j].length; m++) {
          var chave9 = cab ? (cab[m] || ("col" + (m + 1))).replace(/^\s+|\s+$/g, "") : "col" + (m + 1);
          o[chave9] = opts.inferir === false ? linhas[j][m] : inferePtBR(linhas[j][m]);
        }
        out.push(o);
      }
      return out;
    },
    xml: function (texto, opts) {
      opts = opts || {};
      var doc = typeof texto === "string" ? xmlDoc(texto) : texto;
      var raiz = doc.documentElement, nos, tag = opts.registro;
      if (!tag) {
        var cont = {}, f, mx = 0;
        for (f = raiz.firstElementChild; f; f = f.nextElementSibling) cont[f.tagName] = (cont[f.tagName] || 0) + 1;
        for (f in cont) if (cont[f] > mx) { mx = cont[f]; tag = f; }
      }
      nos = doc.getElementsByTagName(tag);
      var out = [], i, j, o, no, ch, at;
      for (i = 0; i < nos.length; i++) {
        no = nos[i]; o = {};
        for (j = 0; j < no.attributes.length; j++) { at = no.attributes[j]; o[at.name] = opts.inferir === false ? at.value : inferePtBR(at.value); }
        for (ch = no.firstElementChild; ch; ch = ch.nextElementSibling) o[ch.tagName] = ch.firstElementChild ? ch.textContent : (opts.inferir === false ? ch.textContent : inferePtBR(ch.textContent));
        out.push(o);
      }
      return out;
    },
    html: function (entrada, opts) {
      opts = opts || {};
      var P = _DP();
      var doc = typeof entrada === "string" ? new P().parseFromString(entrada, "text/html") : (entrada.ownerDocument || entrada);
      var tabelas = typeof entrada === "string" || entrada.nodeType === 9 ? doc.getElementsByTagName("table") : [entrada];
      var tab = typeof opts.tabela === "number" ? tabelas[opts.tabela] : (opts.tabela ? doc.querySelector(opts.tabela) : tabelas[0]);
      if (!tab) return [];
      var trs = tab.getElementsByTagName("tr"), cab = [], out = [], i, j, cels, o, primeira = true;
      for (i = 0; i < trs.length; i++) {
        cels = trs[i].querySelectorAll("th,td");
        if (!cels.length) continue;
        if (primeira) {
          primeira = false;
          var soTh = true; for (j = 0; j < cels.length; j++) if (cels[j].tagName !== "TH") soTh = false;
          if (soTh || opts.cabecalho !== false) { for (j = 0; j < cels.length; j++) cab.push(cels[j].textContent.replace(/^\s+|\s+$/g, "") || ("col" + (j + 1))); if (soTh || opts.cabecalho !== false) continue; }
        }
        o = {};
        for (j = 0; j < cels.length; j++) o[cab[j] || ("col" + (j + 1))] = opts.inferir === false ? cels[j].textContent.replace(/^\s+|\s+$/g, "") : inferePtBR(cels[j].textContent);
        out.push(o);
      }
      return out;
    },
    xlsx: function (bytes, opts) {
      opts = opts || {};
      var u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
      var zip = leZip(u8);
      var rels = zip.texto("xl/_rels/workbook.xml.rels") || "", wb = zip.texto("xl/workbook.xml") || "";
      var abas = [], m, re = /<sheet [^>]*name="([^"]*)"[^>]*r:id="([^"]*)"/g;
      while ((m = re.exec(wb))) abas.push({ nome: m[1], rid: m[2] });
      if (!abas.length) { re = /<sheet [^>]*r:id="([^"]*)"[^>]*name="([^"]*)"/g; while ((m = re.exec(wb))) abas.push({ nome: m[2], rid: m[1] }); }
      var alvo = abas[0], j;
      if (opts.aba != null) for (j = 0; j < abas.length; j++) if (abas[j].nome === opts.aba || j === opts.aba) alvo = abas[j];
      var caminho = "xl/worksheets/sheet1.xml";
      if (alvo) { var mr = rels.match(new RegExp('Id="' + alvo.rid + '"[^>]*Target="([^"]*)"')) || rels.match(new RegExp('Target="([^"]*)"[^>]*Id="' + alvo.rid + '"')); if (mr) caminho = "xl/" + mr[1].replace(/^\/?xl\//, "").replace(/^\//, ""); }
      var sheet = zip.texto(caminho);
      if (!sheet) throw new Error("xlsx: planilha n\u00e3o encontrada");
      var ss = [], sst = zip.texto("xl/sharedStrings.xml");
      if (sst) { var rs = /<si>([\s\S]*?)<\/si>/g, ms; while ((ms = rs.exec(sst))) { var ts = ms[1].match(/<t[^>]*>([\s\S]*?)<\/t>/g) || [], tx = "", q; for (q = 0; q < ts.length; q++) tx += ts[q].replace(/<[^>]+>/g, ""); ss.push(desescXml(tx)); } }
      /* estilos de data: numFmtId 14-22 ou formato com d/m/y */
      var estilos = zip.texto("xl/styles.xml") || "", fmtData = {}, xfData = [];
      var rn = /<numFmt [^>]*numFmtId="(\d+)"[^>]*formatCode="([^"]*)"/g, mn;
      while ((mn = rn.exec(estilos))) if (/[dmy]/i.test(mn[2].replace(/&quot;[^&]*&quot;/g, "")) && !/[#0]/.test(mn[2].replace(/&quot;[^&]*&quot;/g, ""))) fmtData[mn[1]] = 1;
      var cx = estilos.match(/<cellXfs[^>]*>([\s\S]*?)<\/cellXfs>/);
      if (cx) { var rx = /<xf [^>]*numFmtId="(\d+)"/g, mx2; while ((mx2 = rx.exec(cx[1]))) { var id = Number(mx2[1]); xfData.push((id >= 14 && id <= 22) || fmtData[mx2[1]] ? 1 : 0); } }
      var linhas = [], rr = /<row [^>]*>([\s\S]*?)<\/row>/g, mrow;
      while ((mrow = rr.exec(sheet))) {
        var cel = /<c r="([A-Z]+)(\d+)"([^>]*?)(?:\/>|>([\s\S]*?)<\/c>)/g, mc, lin = [];
        while ((mc = cel.exec(mrow[1]))) {
          var atr = mc[3], corpo = mc[4] || "", ci = colIndice(mc[1]), v = null;
          var t = (atr.match(/t="([^"]*)"/) || [])[1], sIx = (atr.match(/s="(\d+)"/) || [])[1];
          if (t === "s") { var vi = corpo.match(/<v>([^<]*)<\/v>/); v = vi ? ss[Number(vi[1])] : null; }
          else if (t === "inlineStr") { v = desescXml((corpo.match(/<t[^>]*>([\s\S]*?)<\/t>/) || [, ""])[1]); }
          else if (t === "str") { var vs = corpo.match(/<v>([\s\S]*?)<\/v>/); v = vs ? desescXml(vs[1]) : null; }
          else if (t === "b") { var vb = corpo.match(/<v>([^<]*)<\/v>/); v = vb ? vb[1] === "1" : null; }
          else { var vn = corpo.match(/<v>([^<]*)<\/v>/); if (vn) { v = Number(vn[1]); if (sIx != null && xfData[Number(sIx)] && v === v) v = serialParaISO(v); } }
          lin[ci] = v;
        }
        linhas.push(lin);
      }
      if (!linhas.length) return [];
      var cab = opts.cabecalho === false ? null : linhas.shift(), out = [], i, k, o, nCol = 0;
      for (i = 0; i < linhas.length; i++) if (linhas[i].length > nCol) nCol = linhas[i].length;
      if (cab && cab.length > nCol) nCol = cab.length;
      for (i = 0; i < linhas.length; i++) {
        o = {};
        var vazia = true;
        for (k = 0; k < nCol; k++) { var ck = cab ? String(cab[k] == null ? "col" + (k + 1) : cab[k]) : "col" + (k + 1); o[ck] = linhas[i][k] === undefined ? null : linhas[i][k]; if (o[ck] != null) vazia = false; }
        if (!vazia) out.push(o);
      }
      return out;
    },
    auto: function (nome, conteudo, opts) {
      var ext = String(nome || "").toLowerCase().split(".").pop();
      if (ext === "json") return carregar.json(conteudo);
      if (ext === "csv" || ext === "txt" || ext === "tsv") return carregar.csv(conteudo, opts);
      if (ext === "xml") return carregar.xml(conteudo, opts);
      if (ext === "html" || ext === "htm") return carregar.html(conteudo, opts);
      if (ext === "xlsx") return carregar.xlsx(conteudo, opts);
      if (ext === "xls") throw new Error("xls (BIFF) n\u00e3o \u00e9 lido no navegador \u2014 use o phx-conector (Rust) ou salve como xlsx");
      throw new Error("formato n\u00e3o suportado: " + ext);
    },
    arquivo: function (file, cb, opts) {
      var ext = String(file && file.name || "").toLowerCase().split(".").pop();
      var FR = typeof FileReader !== "undefined" ? FileReader : (typeof window !== "undefined" ? window.FileReader : null);
      if (!FR) { cb(new Error("FileReader indispon\u00edvel")); return; }
      var fr = new FR();
      fr.onerror = function () { cb(new Error("falha ao ler " + file.name)); };
      fr.onload = function () {
        try { cb(null, carregar.auto(file.name, fr.result, opts), { nome: file.name, bytes: file.size }); }
        catch (e9) { cb(e9); }
      };
      if (ext === "xlsx" || ext === "xls") fr.readAsArrayBuffer(file); else fr.readAsText(file);
    }
  };
  /* fonte que fala com o phx-conector (Rust): ODBC ou arquivo, paginado no servidor */
  function fonteConector(opts) {
    opts = opts || {};
    var url = (opts.url || "http://127.0.0.1:7311").replace(/\/$/, "");
    var IDENT = /^[A-Za-z_][A-Za-z0-9_]*$/;
    function post(rota, corpo, cb) {
      var XHR = typeof XMLHttpRequest !== "undefined" ? XMLHttpRequest : (typeof window !== "undefined" ? window.XMLHttpRequest : null);
      if (!XHR) { cb("XMLHttpRequest indispon\u00edvel"); return; }
      var x = new XHR();
      x.open("POST", url + rota, true);
      /* MIGRACAO PARA A CAMADA SEGURA (v0.76)
         A v0.74 passou a exigir assinatura HMAC no conector. Este `fonteConector` (o nome
         antigo, usado pelas telas WinDev ja em producao) nao assinava, entao TODAS as
         integracoes existentes passaram a levar 401 -- quebra que eu introduzi sem migracao.
         Agora ele assina sozinho quando ha token, e quando NAO ha explica o que fazer em vez
         de deixar o WinDev receber um 401 sem contexto. */
      if (!opts.token) {
        cb("phx-conector exige token desde a v0.74. O conector imprime PHX_TOKEN_GERADO=... " +
           "no arranque; passe-o em fonteConector({token: ...}). Detalhes em docs/SEGURANCA.md.");
        return;
      }
      var corpoTexto9 = JSON.stringify(corpo);
      var cabs9;
      try {
        cabs9 = assinaRequisicao({ token: opts.token, segredo: opts.segredo || opts.token },
                                 "POST", rota, corpoTexto9);
      } catch (eA9) { cb("phx-seguranca: " + eA9.message); return; }
      var kA9;
      for (kA9 in cabs9) if (Object.prototype.hasOwnProperty.call(cabs9, kA9)) x.setRequestHeader(kA9, cabs9[kA9]);
      x.onreadystatechange = function () {
        if (x.readyState !== 4) return;
        if (x.status === 0) { cb("phx-conector inacess\u00edvel em " + url); return; }
        var r;
        try { r = JSON.parse(x.responseText); } catch (e9) { cb("resposta inv\u00e1lida do conector"); return; }
        if (!r.ok) { cb(r.erro || "erro no conector"); return; }
        cb(null, r);
      };
      /* envia EXATAMENTE a string que foi assinada: reserializar poderia mudar a ordem das
         chaves e invalidar a assinatura */
      x.send(corpoTexto9);
    }
    function sqlCom(p) {
      var sql = opts.sql;
      if (opts.ordenar !== false && p && p.ordens && p.ordens.length) {
        var partes = [], j;
        for (j = 0; j < p.ordens.length; j++) if (IDENT.test(p.ordens[j].campo)) partes.push(p.ordens[j].campo + (p.ordens[j].dir === "desc" ? " DESC" : " ASC"));
        if (partes.length) sql = "SELECT * FROM (" + sql + ") phx_q ORDER BY " + partes.join(", ");
      }
      return sql;
    }
    return {
      local: false,
      remoto: true,
      carregar: function (p, cb) {
        var corpo = opts.caminho
          ? { caminho: opts.caminho, aba: opts.aba, cabecalho: opts.cabecalho !== false, pagina: p.pagina || 1, tamanho: p.tamanho || 50 }
          : { conexao: opts.conexao, sql: sqlCom(p), pagina: p.pagina || 1, tamanho: p.tamanho || 50, limite: opts.limite || 50000 };
        post(opts.caminho ? "/arquivo" : "/consulta", corpo, function (err, r) {
          if (err) { cb(err); return; }
          cb(null, { linhas: r.linhas, total: r.total, colunas: r.colunas, totais: null, _ms: r.ms });
        });
      },
      colunas: function (cb) { this.carregar({ pagina: 1, tamanho: 1 }, function (err, r) { cb(err, r && r.colunas); }); }
    };
  }
  function desescXml(t) { return String(t).replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&#(\d+);/g, function (m0, d) { return String.fromCharCode(Number(d)); }).replace(/&amp;/g, "&"); }

  /* ===== O3-7: XLSX ZERO-DEP (PK-zip STORED + OOXML) ===== */
  var _crcTab = null;
  function crcTabela() {
    if (_crcTab) return _crcTab;
    var t = new Array(256), c, n, k;
    for (n = 0; n < 256; n++) {
      c = n;
      for (k = 0; k < 8; k++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
      t[n] = c >>> 0;
    }
    _crcTab = t;
    return t;
  }
  function crc32(bytes) {
    var t = crcTabela(), c = 0xFFFFFFFF, i;
    for (i = 0; i < bytes.length; i++) c = t[(c ^ bytes[i]) & 0xFF] ^ (c >>> 8);
    return (c ^ 0xFFFFFFFF) >>> 0;
  }
  function utf8Bytes(txt) {
    var s9 = String(txt == null ? "" : txt), out = [], i, cp, c;
    for (i = 0; i < s9.length; i++) {
      cp = s9.charCodeAt(i);
      if (cp >= 0xD800 && cp <= 0xDBFF && i + 1 < s9.length) {
        c = s9.charCodeAt(i + 1);
        if (c >= 0xDC00 && c <= 0xDFFF) { cp = ((cp - 0xD800) << 10) + (c - 0xDC00) + 0x10000; i++; }
      }
      if (cp < 0x80) out.push(cp);
      else if (cp < 0x800) { out.push(0xC0 | (cp >> 6), 0x80 | (cp & 63)); }
      else if (cp < 0x10000) { out.push(0xE0 | (cp >> 12), 0x80 | ((cp >> 6) & 63), 0x80 | (cp & 63)); }
      else { out.push(0xF0 | (cp >> 18), 0x80 | ((cp >> 12) & 63), 0x80 | ((cp >> 6) & 63), 0x80 | (cp & 63)); }
    }
    return out;
  }
  /* DEFLATE (RFC 1951) em ES5: LZ77 com cadeias de hash + Huffman fixo.
     Escolha honesta: bloco fixo em vez de dinamico troca alguns por cento de taxa por um
     compressor muito menor e sem tabela; o ganho grande vem do LZ77, nao da arvore. */
  var _zipComprime = true, _zipNivel = 6;
  function deflate(bytes, nivel) {
    var n = bytes.length, saida = [], bitBuf = 0, bitCnt = 0;
    function bitMSB(v, len) { /* codigos de Huffman sao escritos do bit mais significativo */
      var i;
      for (i = len - 1; i >= 0; i--) { bitBuf |= ((v >> i) & 1) << bitCnt; if (++bitCnt === 8) { saida.push(bitBuf & 255); bitBuf = 0; bitCnt = 0; } }
    }
    function bitsLE(v, len) { var i; for (i = 0; i < len; i++) { bitBuf |= ((v >> i) & 1) << bitCnt; if (++bitCnt === 8) { saida.push(bitBuf & 255); bitBuf = 0; bitCnt = 0; } } }
    function fecha() { if (bitCnt) { saida.push(bitBuf & 255); bitBuf = 0; bitCnt = 0; } }
    function lit(c) { if (c <= 143) bitMSB(0x30 + c, 8); else bitMSB(0x190 + c - 144, 9); }
    var LBASE = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
    var LEXT = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
    var DBASE = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
    var DEXT = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
    function codLen(len) { var i; for (i = 28; i >= 0; i--) if (len >= LBASE[i]) return i; return 0; }
    function codDist(d) { var i; for (i = 29; i >= 0; i--) if (d >= DBASE[i]) return i; return 0; }
    var TAM_JAN = 32768, NHASH = 1 << 15;
    var cabeca = [], prox = [], i2;
    for (i2 = 0; i2 < NHASH; i2++) cabeca[i2] = -1;
    var maxCad = nivel === 1 ? 8 : (nivel === 9 ? 256 : 48), maxLen = 258, bomLen = nivel === 1 ? 32 : 128;
    function h3(p) { return ((bytes[p] << 10) ^ (bytes[p + 1] << 5) ^ bytes[p + 2]) & (NHASH - 1); }
    bitsLE(1, 1); bitsLE(1, 2); /* ultimo bloco, Huffman fixo */
    var pos = 0;
    while (pos < n) {
      var melhorLen = 0, melhorDist = 0;
      if (pos + 3 <= n) {
        var hh = h3(pos), cand = cabeca[hh], tent = 0;
        while (cand >= 0 && tent++ < maxCad) {
          var dist = pos - cand;
          if (dist > TAM_JAN) break;
          if (bytes[cand + melhorLen] === bytes[pos + melhorLen]) {
            var l = 0, lim = Math.min(maxLen, n - pos);
            while (l < lim && bytes[cand + l] === bytes[pos + l]) l++;
            if (l > melhorLen) { melhorLen = l; melhorDist = dist; if (l >= bomLen) break; }
          }
          cand = prox[cand];
        }
        prox[pos] = cabeca[hh];
        cabeca[hh] = pos;
      }
      if (melhorLen >= 3) {
        var ic = codLen(melhorLen), idc = codDist(melhorDist), simb = ic + 257;
        if (simb <= 279) bitMSB(simb - 256, 7); else bitMSB(0xC0 + (simb - 280), 8);
        if (LEXT[ic]) bitsLE(melhorLen - LBASE[ic], LEXT[ic]);
        bitMSB(idc, 5);
        if (DEXT[idc]) bitsLE(melhorDist - DBASE[idc], DEXT[idc]);
        var fimP = pos + melhorLen, p2;
        for (p2 = pos + 1; p2 < fimP; p2++) if (p2 + 3 <= n) { var h2 = h3(p2); prox[p2] = cabeca[h2]; cabeca[h2] = p2; }
        pos = fimP;
      } else { lit(bytes[pos]); pos++; }
    }
    bitMSB(0, 7); /* simbolo 256 = fim do bloco */
    fecha();
    return saida;
  }
  function zipStored(arquivos) {
    var locais = [], central = [], desloc = 0, j, a, bytes, crc, nome, nb, k;
    function u16(arr, v) { arr.push(v & 255, (v >> 8) & 255); }
    function u32(arr, v) { arr.push(v & 255, (v >>> 8) & 255, (v >>> 16) & 255, (v >>> 24) & 255); }
    for (j = 0; j < arquivos.length; j++) {
      a = arquivos[j];
      bytes = a.bytes;
      nb = utf8Bytes(a.nome);
      crc = crc32(bytes);
      var metodo = 0, corpo = bytes;
      if (_zipComprime && bytes.length > 256) {
        var comp = deflate(bytes, _zipNivel);
        if (comp.length < bytes.length - 16) { metodo = 8; corpo = comp; }
      }
      var lh = [];
      u32(lh, 0x04034B50); u16(lh, 20); u16(lh, 0x0800); u16(lh, metodo); u16(lh, 0); u16(lh, 0);
      u32(lh, crc); u32(lh, corpo.length); u32(lh, bytes.length);
      u16(lh, nb.length); u16(lh, 0);
      for (k = 0; k < nb.length; k++) lh.push(nb[k]);
      for (k = 0; k < corpo.length; k++) lh.push(corpo[k]);
      var ch = [];
      u32(ch, 0x02014B50); u16(ch, 20); u16(ch, 20); u16(ch, 0x0800); u16(ch, metodo); u16(ch, 0); u16(ch, 0);
      u32(ch, crc); u32(ch, corpo.length); u32(ch, bytes.length);
      u16(ch, nb.length); u16(ch, 0); u16(ch, 0); u16(ch, 0); u16(ch, 0); u32(ch, 0);
      u32(ch, desloc);
      for (k = 0; k < nb.length; k++) ch.push(nb[k]);
      locais.push(lh); central.push(ch);
      desloc += lh.length;
    }
    var total = desloc, cd = [], j2;
    for (j2 = 0; j2 < central.length; j2++) for (k = 0; k < central[j2].length; k++) cd.push(central[j2][k]);
    var eocd = [];
    u32(eocd, 0x06054B50); u16(eocd, 0); u16(eocd, 0);
    u16(eocd, arquivos.length); u16(eocd, arquivos.length);
    u32(eocd, cd.length); u32(eocd, total); u16(eocd, 0);
    var tamanho = total + cd.length + eocd.length;
    var saida = new Uint8Array(tamanho), pos = 0;
    for (j2 = 0; j2 < locais.length; j2++) for (k = 0; k < locais[j2].length; k++) saida[pos++] = locais[j2][k];
    for (k = 0; k < cd.length; k++) saida[pos++] = cd[k];
    for (k = 0; k < eocd.length; k++) saida[pos++] = eocd[k];
    return saida;
  }
  function escXml(txt) {
    return String(txt == null ? "" : txt)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/\u0000/g, "");
  }
  function colLetra(n) {
    var s9 = "";
    n++;
    while (n > 0) { var r = (n - 1) % 26; s9 = String.fromCharCode(65 + r) + s9; n = Math.floor((n - 1) / 26); }
    return s9;
  }
  function serialData(v) {
    var d = v instanceof Date ? v : new Date(String(v).length <= 10 ? String(v) + "T00:00:00Z" : v);
    if (!d || isNaN(d.getTime())) return null;
    return (d.getTime() - Date.UTC(1899, 11, 30)) / 86400000;
  }

  function montaXLSX(cols, linhas, opts) {
    opts = opts || {};
    var aba = escXml(opts.aba || "Dados").slice(0, 31);
    var ESTILO = { texto: 0, cab: 1, data: 2, moeda: 3, percentual: 4, numero: 5, inteiro: 6 };
    function estiloDe(c) {
      var t = c.tipo || "texto";
      if (t === "data" || t === "dataHora") return ESTILO.data;
      if (t === "moeda") return ESTILO.moeda;
      if (t === "percentual") return ESTILO.percentual;
      if (t === "numero") return c.decimais === 0 ? ESTILO.inteiro : ESTILO.numero;
      return ESTILO.texto;
    }
    var j, i, c, v, ref, celulas, linhasXml = [];
    /* cabecalho */
    celulas = [];
    for (j = 0; j < cols.length; j++) {
      ref = colLetra(j) + "1";
      celulas.push('<c r="' + ref + '" s="' + ESTILO.cab + '" t="inlineStr"><is><t xml:space="preserve">' + escXml(cols[j].titulo || cols[j].campo) + "</t></is></c>");
    }
    linhasXml.push('<row r="1">' + celulas.join("") + "</row>");
    /* dados */
    for (i = 0; i < linhas.length; i++) {
      celulas = [];
      for (j = 0; j < cols.length; j++) {
        c = cols[j];
        ref = colLetra(j) + (i + 2);
        v = (c.acumulado && linhas[i].__acum && linhas[i].__acum[c.campo] != null) ? linhas[i].__acum[c.campo] : linhas[i][c.campo];
        if (v == null || v === "") continue;
        var t9 = c.tipo || "texto";
        var est9 = estiloDe(c);
        if (t9 === "data" || t9 === "dataHora") {
          var sd9 = serialData(v);
          if (sd9 == null) { celulas.push('<c r="' + ref + '" t="inlineStr"><is><t xml:space="preserve">' + escXml(v) + "</t></is></c>"); continue; }
          celulas.push('<c r="' + ref + '" s="' + est9 + '"><v>' + sd9 + "</v></c>");
          continue;
        }
        if (t9 === "numero" || t9 === "moeda" || t9 === "percentual") {
          var n9 = Number(v);
          if (n9 !== n9) { celulas.push('<c r="' + ref + '" t="inlineStr"><is><t xml:space="preserve">' + escXml(v) + "</t></is></c>"); continue; }
          if (t9 === "percentual") n9 = n9 / 100;
          if (n9 !== 0 && isFinite(n9)) n9 = Number(n9.toPrecision(15));
          celulas.push('<c r="' + ref + '" s="' + est9 + '"><v>' + n9 + "</v></c>");
          continue;
        }
        celulas.push('<c r="' + ref + '" t="inlineStr"><is><t xml:space="preserve">' + escXml(v) + "</t></is></c>");
      }
      linhasXml.push('<row r="' + (i + 2) + '">' + celulas.join("") + "</row>");
    }
    var largs = [];
    for (j = 0; j < cols.length; j++) {
      var lp9 = Math.max(9, Math.min(46, Math.round((cols[j].largura || 120) / 7.5)));
      largs.push('<col min="' + (j + 1) + '" max="' + (j + 1) + '" width="' + lp9 + '" customWidth="1"/>');
    }
    var ultRef = colLetra(cols.length - 1) + (linhas.length + 1);
    var sheet =
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
      '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">' +
      '<dimension ref="A1:' + ultRef + '"/>' +
      '<sheetViews><sheetView workbookViewId="0" tabSelected="1"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>' +
      '<sheetFormatPr defaultRowHeight="15"/>' +
      "<cols>" + largs.join("") + "</cols>" +
      "<sheetData>" + linhasXml.join("") + "</sheetData>" +
      '<autoFilter ref="A1:' + ultRef + '"/>' +
      "</worksheet>";
    var styles =
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
      '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">' +
      '<numFmts count="4">' +
      '<numFmt numFmtId="164" formatCode="dd/mm/yyyy"/>' +
      '<numFmt numFmtId="165" formatCode="&quot;R$&quot;\ #,##0.00"/>' +
      '<numFmt numFmtId="166" formatCode="0.0%"/>' +
      '<numFmt numFmtId="167" formatCode="#,##0.00"/>' +
      "</numFmts>" +
      '<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts>' +
      '<fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>' +
      '<fill><patternFill patternType="solid"><fgColor rgb="FFF2F2F2"/><bgColor indexed="64"/></patternFill></fill></fills>' +
      '<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>' +
      '<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>' +
      '<cellXfs count="7">' +
      '<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>' +
      '<xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/>' +
      '<xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>' +
      '<xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>' +
      '<xf numFmtId="166" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>' +
      '<xf numFmtId="167" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>' +
      '<xf numFmtId="3" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>' +
      "</cellXfs>" +
      '<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>' +
      "</styleSheet>";
    var arquivos = [
      { nome: "[Content_Types].xml", bytes: utf8Bytes(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">' +
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>' +
        '<Default Extension="xml" ContentType="application/xml"/>' +
        '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>' +
        '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>' +
        '<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>' +
        "</Types>") },
      { nome: "_rels/.rels", bytes: utf8Bytes(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' +
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>' +
        "</Relationships>") },
      { nome: "xl/workbook.xml", bytes: utf8Bytes(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">' +
        '<sheets><sheet name="' + aba + '" sheetId="1" r:id="rId1"/></sheets></workbook>') },
      { nome: "xl/_rels/workbook.xml.rels", bytes: utf8Bytes(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' +
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>' +
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>' +
        "</Relationships>") },
      { nome: "xl/styles.xml", bytes: utf8Bytes(styles) },
      { nome: "xl/worksheets/sheet1.xml", bytes: utf8Bytes(sheet) }
    ];
    return zipStored(arquivos);
  }

  function tsvCampo(txt, aspas) {
    var t = txt == null ? "" : String(txt);
    if (!aspas) return t.replace(/[\t\r\n]+/g, " ");
    if (t.indexOf("\t") >= 0 || t.indexOf("\n") >= 0 || t.indexOf("\r") >= 0 || t.indexOf('"') >= 0) {
      return '"' + t.replace(/"/g, '""') + '"';
    }
    return t;
  }
  function parseTSV(txt) {
    var linhas = [], campo = "", linha = [], i = 0, n = String(txt || "").length, t = String(txt || ""), c, dentro = false;
    while (i < n) {
      c = t.charAt(i);
      if (dentro) {
        if (c === '"') {
          if (t.charAt(i + 1) === '"') { campo += '"'; i += 2; continue; }
          dentro = false; i++; continue;
        }
        campo += c; i++; continue;
      }
      if (c === '"' && campo === "") { dentro = true; i++; continue; }
      if (c === "\t") { linha.push(campo); campo = ""; i++; continue; }
      if (c === "\r") { i++; continue; }
      if (c === "\n") { linha.push(campo); linhas.push(linha); linha = []; campo = ""; i++; continue; }
      campo += c; i++;
    }
    if (campo !== "" || linha.length) { linha.push(campo); linhas.push(linha); }
    return linhas;
  }
  function csvCampo(txt, sep) {
    var t = String(txt == null ? "" : txt);
    if (t.indexOf(sep) >= 0 || t.indexOf('"') >= 0 || t.indexOf("\n") >= 0 || t.indexOf("\r") >= 0)
      return '"' + t.replace(/"/g, '""') + '"';
    return t;
  }
  /* Um unico Infinity num campo destruia o total inteiro (poluia a soma e virava null na tela).
     NaN ja era ignorado -- Infinity nao era. Agora os dois sao tratados igual a nulo: valor
     nao-finito nao e medida valida e fica fora da agregacao. (achado pelo fuzz, v0.73) */
  function _fin(x) { var n = Number(x); return (n === n && n !== Infinity && n !== -Infinity) ? n : null; }
  var AGGS = {
    sum: function (a) { var t = 0, i2, v2; for (i2 = 0; i2 < a.length; i2++) { v2 = _fin(a[i2]); if (v2 !== null) t += v2; } return t; },
    avg: function (a) { var n2 = 0, i2; for (i2 = 0; i2 < a.length; i2++) if (_fin(a[i2]) !== null) n2++; return n2 ? AGGS.sum(a) / n2 : 0; },
    count: function (a) { return a.length; },
    stddev: function (a) { var n = 0, m = 0, i2, v2, q = 0; for (i2 = 0; i2 < a.length; i2++) { v2 = _fin(a[i2]); if (v2 !== null) { n++; m += v2; } } if (n < 2) return 0; m /= n; for (i2 = 0; i2 < a.length; i2++) { v2 = Number(a[i2]); if (v2 === v2) q += (v2 - m) * (v2 - m); } return Math.sqrt(q / (n - 1)); },
    valuecount: function (a) { var v = {}, n = 0, i2; for (i2 = 0; i2 < a.length; i2++) if (a[i2] != null && a[i2] !== "" && !v[String(a[i2])]) { v[String(a[i2])] = 1; n++; } return n; },
    min: function (a) { var m = Infinity, i2, v2; for (i2 = 0; i2 < a.length; i2++) { v2 = _fin(a[i2]); if (v2 !== null && v2 < m) m = v2; } return m === Infinity ? null : m; },
    max: function (a) { var m = -Infinity, i2, v2; for (i2 = 0; i2 < a.length; i2++) { v2 = _fin(a[i2]); if (v2 !== null && v2 > m) m = v2; } return m === -Infinity ? null : m; }
  };
  function testaCondicao(cd9, linha9) {
    var v9 = linha9[cd9.campo], a9 = cd9.valor, b9 = cd9.valor2, op9 = cd9.op || "=";
    var nv9 = Number(v9), na9 = Number(a9), nb9 = Number(b9);
    var num9 = v9 != null && nv9 === nv9 && a9 != null && na9 === na9 && String(v9) !== "";
    switch (op9) {
      case "=": return String(v9) === String(a9);
      case "<>": return String(v9) !== String(a9);
      case ">": return num9 ? nv9 > na9 : String(v9) > String(a9);
      case ">=": return num9 ? nv9 >= na9 : String(v9) >= String(a9);
      case "<": return num9 ? nv9 < na9 : String(v9) < String(a9);
      case "<=": return num9 ? nv9 <= na9 : String(v9) <= String(a9);
      case "entre": return num9 && nb9 === nb9 ? (nv9 >= na9 && nv9 <= nb9) : (String(v9) >= String(a9) && String(v9) <= String(b9));
      case "contem": return v9 != null && String(v9).toLowerCase().indexOf(String(a9).toLowerCase()) >= 0;
      case "vazio": return v9 == null || v9 === "";
      case "naoVazio": return !(v9 == null || v9 === "");
        default: return false;
    }
  }
  function agrupa(linhas, campos, aggCols, tipoDe, condicoesGrupo) {
    function nivel(lns, d, pathPai) {
      var campoNivel9 = campos[d], composto9 = campoNivel9 && typeof campoNivel9 !== "string";
      /* objeto sem prototipo: "__proto__" como valor de grupo duplicava o grupo em `{}` normal
         (atribuir a essa chave muda o prototipo em vez de criar propriedade). fuzz v0.73 */
      var mapa = (Object.create ? Object.create(null) : {}), ordem2 = [], i2, v2, k2, g2, qi9;
      for (i2 = 0; i2 < lns.length; i2++) {
        if (composto9) {
          v2 = [];
          for (qi9 = 0; qi9 < campoNivel9.length; qi9++) v2.push(lns[i2][campoNivel9[qi9]]);
          k2 = v2.join("\u0000");
        } else {
          v2 = lns[i2][campoNivel9];
          k2 = String(v2);
        }
        /* hasOwnProperty: um valor de grupo chamado "constructor" pegava o herdado de
           Object.prototype e o grupo era perdido (mesmo bug do cubo, fuzz v0.73) */
        g2 = Object.prototype.hasOwnProperty.call(mapa, k2) ? mapa[k2] : null;
        if (!g2) {
          g2 = { campo: composto9 ? campoNivel9.join("\u0000") : campoNivel9, camposComp: composto9 ? campoNivel9 : null, valor: v2, chave: k2, linhas: [] };
          mapa[k2] = g2; ordem2.push(g2);
        }
        g2.linhas.push(lns[i2]);
      }
      ordem2.sort(function (a, b) {
        /* grupo composto ordena pela chave ja concatenada (string); grupo simples usa o comparador por tipo de sempre */
        if (composto9) return a.chave < b.chave ? -1 : (a.chave > b.chave ? 1 : 0);
        var ka = chaveOrd(a.valor, tipoDe(campoNivel9)), kb = chaveOrd(b.valor, tipoDe(campoNivel9));
        if (ka.n && kb.n) return 0;
        if (ka.n) return 1;
        if (kb.n) return -1;
        if (ka.v < kb.v) return -1;
        if (ka.v > kb.v) return 1;
        return 0;
      });
      var out = [], j2, g3, no2;
      for (j2 = 0; j2 < ordem2.length; j2++) {
        g3 = ordem2[j2];
        no2 = { campo: g3.campo, camposComp: g3.camposComp, valor: g3.valor, chave: g3.chave, nivel: d,
          path: pathPai + (pathPai ? "\u0001" : "") + g3.campo + "=" + g3.chave, n: g3.linhas.length, aggs: {} };
        var ac;
        for (ac = 0; ac < aggCols.length; ac++) {
          var col = aggCols[ac], vals = [], i3;
          for (i3 = 0; i3 < g3.linhas.length; i3++) vals.push(g3.linhas[i3][col.campo]);
          no2.aggs[col.campo] = AGGS[col.agregador](vals);
        }
        if (condicoesGrupo && condicoesGrupo.length) {
          no2.condGrupo = [];
          var cg9, ig9, cnt9;
          for (cg9 = 0; cg9 < condicoesGrupo.length; cg9++) {
            cnt9 = 0;
            for (ig9 = 0; ig9 < g3.linhas.length; ig9++) if (testaCondicao(condicoesGrupo[cg9], g3.linhas[ig9])) cnt9++;
            no2.condGrupo.push(cnt9);
          }
        }
        if (d + 1 < campos.length) no2.filhos = nivel(g3.linhas, d + 1, no2.path, condicoesGrupo);
        else no2.linhas = g3.linhas;
        out.push(no2);
      }
      return out;
    }
    return nivel(linhas, 0, "", condicoesGrupo);
  }
  function achata(arvoreG, recolhidos) {
    var out = [], i2;
    function anda(nos) {
      var j2, no2;
      for (j2 = 0; j2 < nos.length; j2++) {
        no2 = nos[j2];
        out.push({ __grupo: no2 });
        if (recolhidos[no2.path]) continue;
        if (no2.filhos) anda(no2.filhos);
        else for (i2 = 0; i2 < no2.linhas.length; i2++) out.push(no2.linhas[i2]);
      }
    }
    anda(arvoreG);
    return out;
  }
  function fonteLocal(dados) {
    var idxBusca = null, idxChave = "";
    var indices = {};
    var ultimoPlano = null;
    function invalidaIndices(campo) {
      if (campo) delete indices[campo];
      else indices = {};
      idxBusca = null; idxChave = "";
    }
    function criaIndice(campo, tipoCol) {
      var t0 = agora();
      var ehOrd = tipoCol === "numero" || tipoCol === "moeda" || tipoCol === "percentual" || tipoCol === "data" || tipoCol === "dataHora" || tipoCol === "barra";
      var i2, v, ix;
      if (ehOrd) {
        var pares = [];
        for (i2 = 0; i2 < dados.length; i2++) {
          var kc = chaveOrd(dados[i2][campo], tipoCol);
          if (!kc.n) pares.push([kc.v, i2]);
        }
        pares.sort(function (a, b) { return a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] - b[1]; });
        var ks = new Array(pares.length), ixs = new Array(pares.length);
        for (i2 = 0; i2 < pares.length; i2++) { ks[i2] = pares[i2][0]; ixs[i2] = pares[i2][1]; }
        ix = { tipo: "ordenado", ks: ks, ixs: ixs, tipoCol: tipoCol, ms: agora() - t0 };
      } else {
        var mapa = {};
        for (i2 = 0; i2 < dados.length; i2++) {
          v = dados[i2][campo];
          var kv = v == null || v === "" ? "\u0000null" : String(v);
          (mapa[kv] || (mapa[kv] = [])).push(i2);
        }
        ix = { tipo: "invertido", mapa: mapa, ms: agora() - t0 };
      }
      indices[campo] = ix;
      return ix;
    }
    function lowerBound(ks, alvo) {
      var lo = 0, hi = ks.length;
      while (lo < hi) { var m = (lo + hi) >> 1; if (ks[m] < alvo) lo = m + 1; else hi = m; }
      return lo;
    }
    function upperBound(ks, alvo) {
      var lo = 0, hi = ks.length;
      while (lo < hi) { var m = (lo + hi) >> 1; if (ks[m] <= alvo) lo = m + 1; else hi = m; }
      return lo;
    }
    function candidatosDe(f, tiposCampos) {
      if (f.tipo === "valores") {
        var ix = indices[f.campo] || criaIndice(f.campo, (tiposCampos && tiposCampos[f.campo]) || "texto");
        if (ix.tipo !== "invertido") return null;
        var total = 0, listas = [], j2, l2;
        for (j2 = 0; j2 < f.valores.length; j2++) {
          l2 = ix.mapa[String(f.valores[j2])];
          if (l2) { listas.push(l2); total += l2.length; }
        }
        if (f.incluiNulos && ix.mapa["\u0000null"]) { listas.push(ix.mapa["\u0000null"]); total += ix.mapa["\u0000null"].length; }
        return {
          custo: total, campo: f.campo,
          gera: function () {
            var out = [], j3, k3;
            for (j3 = 0; j3 < listas.length; j3++) for (k3 = 0; k3 < listas[j3].length; k3++) out.push(listas[j3][k3]);
            return out;
          }
        };
      }
      if (f.tipo === "faixa") {
        var tipoCol = f.tipoCol || (tiposCampos && tiposCampos[f.campo]) || "numero";
        var ix2 = indices[f.campo] || criaIndice(f.campo, tipoCol);
        if (ix2.tipo !== "ordenado") return null;
        var lo = 0, hi = ix2.ks.length;
        if (f.de != null) lo = lowerBound(ix2.ks, chaveOrd(f.de, tipoCol).v);
        if (f.ate != null) hi = upperBound(ix2.ks, chaveOrd(f.ate, tipoCol).v);
        if (hi < lo) hi = lo;
        var loF = lo, hiF = hi;
        return { custo: hi - lo, campo: f.campo, gera: function () { return ix2.ixs.slice(loF, hiF); } };
      }
      return null;
    }
    function filtraPlanejado(lista, tiposCampos) {
      var t0 = agora();
      if (!lista || !lista.length) { ultimoPlano = { estrategia: "nenhum" }; return dados; }
      var cands = [], j2, c2;
      for (j2 = 0; j2 < lista.length; j2++) {
        c2 = candidatosDe(lista[j2], tiposCampos);
        if (c2) cands.push({ f: lista[j2], c: c2 });
      }
      if (!cands.length) {
        var outS = aplicaFiltros(dados, lista);
        ultimoPlano = { estrategia: "scan", verificados: dados.length, passaram: outS.length, ms: agora() - t0 };
        return outS;
      }
      var melhor = cands[0];
      for (j2 = 1; j2 < cands.length; j2++) if (cands[j2].c.custo < melhor.c.custo) melhor = cands[j2];
      var verifs = [];
      for (j2 = 0; j2 < lista.length; j2++) if (lista[j2] !== melhor.f) verifs.push(lista[j2]);
      verifs.sort(function (a, b) { return (CUSTO_COND[a.tipo] || 5) - (CUSTO_COND[b.tipo] || 5); });
      var ixs = melhor.c.gera();
      ixs.sort(function (a, b) { return a - b; });
      var out = [], i3, k3, okL, linha3;
      for (i3 = 0; i3 < ixs.length; i3++) {
        linha3 = dados[ixs[i3]];
        okL = true;
        for (k3 = 0; k3 < verifs.length; k3++) if (!passaCondicao(linha3, verifs[k3])) { okL = false; break; }
        if (okL) out.push(linha3);
      }
      ultimoPlano = { estrategia: "indice", gerador: melhor.c.campo, tipoIndice: indices[melhor.c.campo].tipo, candidatos: ixs.length, verificadores: verifs.length, passaram: out.length, ms: agora() - t0 };
      return out;
    }
    function indiceBusca(campos) {
      var ch = campos.join("\u0001");
      if (idxBusca && idxChave === ch) return idxBusca;
      var m = [], jc, i2, col, sv;
      for (jc = 0; jc < campos.length; jc++) {
        col = new Array(dados.length);
        for (i2 = 0; i2 < dados.length; i2++) {
          sv = dados[i2][campos[jc]];
          col[i2] = sv == null ? null : semAcento(sv);
        }
        m.push(col);
      }
      idxBusca = m; idxChave = ch;
      return m;
    }
    return {
      local: true,
      todos: dados,
      invalidar: invalidaIndices,
      carregar: function (p, cb) {
        var t0 = agora();
        var fBusca = null, resto = [], j9;
        for (j9 = 0; j9 < (p.filtros || []).length; j9++) {
          if (p.filtros[j9].tipo === "busca") fBusca = p.filtros[j9];
          else resto.push(p.filtros[j9]);
        }
        var filtrados, base;
        if (fBusca) {
          var sobra = resto.length ? filtraPlanejado(resto, p.tiposCampos) : dados;
          var mapaIx = null;
          if (sobra === dados) {
            mapaIx = new Array(dados.length);
            for (j9 = 0; j9 < dados.length; j9++) mapaIx[j9] = j9;
          } else {
            /* mapa: posição na sobra -> índice global (uma passada com ponteiro, dados preservam ordem) */
            mapaIx = new Array(sobra.length);
            var pg = 0;
            for (j9 = 0; j9 < sobra.length; j9++) {
              while (dados[pg] !== sobra[j9]) pg++;
              mapaIx[j9] = pg++;
            }
          }
          var pontuadas = filtraEScoreBusca(sobra, fBusca, indiceBusca(fBusca.campos), mapaIx);
          if (p.ordem && p.ordem.campo) {
            filtrados = [];
            for (j9 = 0; j9 < pontuadas.length; j9++) filtrados.push(pontuadas[j9].l);
            base = (p.ordens && p.ordens.length > 1) ? ordenaEstavelMulti(filtrados, p.ordens) : ordenaEstavel(filtrados, p.ordem.campo, p.ordem.dir, p.ordem.tipo);
          } else {
            pontuadas.sort(function (a, b) { return b.s - a.s || a.ix - b.ix; });
            filtrados = base = [];
            for (j9 = 0; j9 < pontuadas.length; j9++) base.push(pontuadas[j9].l);
          }
        } else {
          filtrados = filtraPlanejado(resto, p.tiposCampos);
          base = (p.ordens && p.ordens.length > 1)
            ? ordenaEstavelMulti(filtrados, p.ordens)
            : (p.ordem && p.ordem.campo
              ? ordenaEstavel(filtrados, p.ordem.campo, p.ordem.dir, p.ordem.tipo)
              : filtrados.slice());
        }
        var totaisResp = null;
        if (p.totais && p.totais.length) {
          totaisResp = {};
          var jT, colT, valsT, iT;
          for (jT = 0; jT < p.totais.length; jT++) {
            colT = p.totais[jT];
            valsT = [];
            for (iT = 0; iT < base.length; iT++) valsT.push(base[iT][colT.campo]);
            totaisResp[colT.campo] = AGGS[colT.agregador](valsT);
          }
        }
        var ini = (p.pagina - 1) * p.tamanho;
        if (p.arvore && p.arvore.pai) {
          var planaA = montaArvorePlana(dados, base, p);
          cb(null, {
            linhas: planaA.slice(ini, ini + p.tamanho),
            total: planaA.length,
            totalDados: filtrados.length,
            totais: totaisResp,
            _plano: ultimoPlano,
            _ms: agora() - t0
          });
          return;
        }
        if (p.grupos && p.grupos.length) {
          var arvG = agrupa(base, p.grupos, p.aggCols || [], function (c9) { return (p.tiposCampos && p.tiposCampos[c9]) || "texto"; }, p.condicoesGrupo);
          var plana = achata(arvG, p.recolhidos || {});
          cb(null, {
            linhas: plana.slice(ini, ini + p.tamanho),
            total: plana.length,
            totalDados: filtrados.length,
            totais: totaisResp,
            _plano: ultimoPlano,
            _ms: agora() - t0
          });
          return;
        }
        if (p.acumulados && p.acumulados.length) {
          var jA9, iA9, acc9;
          for (jA9 = 0; jA9 < p.acumulados.length; jA9++) {
            acc9 = p.acumulados[jA9].inicial || 0;
            for (iA9 = 0; iA9 < base.length; iA9++) {
              var vA9 = Number(base[iA9][p.acumulados[jA9].base]);
              if (vA9 === vA9) acc9 += vA9;
              if (!base[iA9].__acum) base[iA9].__acum = {};
              base[iA9].__acum[p.acumulados[jA9].campo] = acc9;
            }
          }
        }
        cb(null, {
          linhas: base.slice(ini, ini + p.tamanho),
          total: filtrados.length,
          totais: totaisResp,
          _plano: ultimoPlano,
          _ms: agora() - t0
        });
      }
    };
  }

  /* ===== O3-2: F\u00d3RMULAS (parser pr\u00f3prio, zero eval) ===== */
  function compilaFormula(expr) {
    var src = String(expr || ""), pos = 0, len = src.length;
    function pula() { while (pos < len && src.charCodeAt(pos) <= 32) pos++; }
    function pico() { pula(); return pos < len ? src.charAt(pos) : ""; }
    function erro(m) { throw new Error("f\u00f3rmula: " + m + " em " + pos); }
    function numero() {
      var ini = pos;
      while (pos < len && /[0-9.]/.test(src.charAt(pos))) pos++;
      var n = parseFloat(src.slice(ini, pos));
      if (n !== n) erro("n\u00famero inv\u00e1lido");
      return function () { return n; };
    }
    function ident() {
      var ini = pos;
      while (pos < len && /[A-Za-z0-9_]/.test(src.charAt(pos))) pos++;
      return src.slice(ini, pos);
    }
    var FNS = {
      abs: function (a) { return Math.abs(a[0]); },
      min: function (a) { return Math.min(a[0], a[1]); },
      max: function (a) { return Math.max(a[0], a[1]); },
      round: function (a) { var f = Math.pow(10, a[1] || 0); return Math.round(a[0] * f) / f; },
      se: function (a) { return a[0] !== a[0] ? NaN : (a[0] ? a[1] : a[2]); }
    };
    function fator() {
      var c = pico();
      if (c === "(") { pos++; var e = expressao(); pula(); if (src.charAt(pos) !== ")") erro("falta )"); pos++; return e; }
      if (c === "-") { pos++; var f = fator(); return function (l) { return -f(l); }; }
      if (/[0-9.]/.test(c)) return numero();
      if (/[A-Za-z_]/.test(c)) {
        var nome = ident();
        pula();
        if (src.charAt(pos) === "(") {
          pos++;
          var fn = FNS[nome];
          if (!fn) erro("fun\u00e7\u00e3o desconhecida: " + nome);
          var args = [];
          if (pico() !== ")") {
            args.push(expressao());
            while (pico() === ",") { pos++; args.push(expressao()); }
          }
          pula();
          if (src.charAt(pos) !== ")") erro("falta ) em " + nome);
          pos++;
          return function (l) {
            var vals = [], j2;
            for (j2 = 0; j2 < args.length; j2++) vals.push(args[j2](l));
            return fn(vals);
          };
        }
        return function (l) { var v9 = l[nome]; return v9 == null ? NaN : Number(v9); };
      }
      erro("inesperado '" + c + "'");
    }
    function termo() {
      var e = fator();
      for (;;) {
        var c = pico();
        if (c === "*") { pos++; var d = fator(); e = (function (a, b) { return function (l) { return a(l) * b(l); }; })(e, d); }
        else if (c === "/") { pos++; var d2 = fator(); e = (function (a, b) { return function (l) { return a(l) / b(l); }; })(e, d2); }
        else return e;
      }
    }
    function soma() {
      var e = termo();
      for (;;) {
        var c = pico();
        if (c === "+") { pos++; var d = termo(); e = (function (a, b) { return function (l) { return a(l) + b(l); }; })(e, d); }
        else if (c === "-") { pos++; var d2 = termo(); e = (function (a, b) { return function (l) { return a(l) - b(l); }; })(e, d2); }
        else return e;
      }
    }
    function expressao() {
      var e = soma();
      pula();
      var dois = src.slice(pos, pos + 2);
      var um = src.charAt(pos);
      var op = null;
      if (dois === ">=" || dois === "<=" || dois === "==" || dois === "!=") { op = dois; pos += 2; }
      else if (um === ">" || um === "<") { op = um; pos++; }
      if (!op) return e;
      var d = soma();
      return (function (a, b, o) {
        return function (l) {
          var x = a(l), y = b(l);
          if (x !== x || y !== y) return NaN;
          if (o === ">") return x > y ? 1 : 0;
          if (o === "<") return x < y ? 1 : 0;
          if (o === ">=") return x >= y ? 1 : 0;
          if (o === "<=") return x <= y ? 1 : 0;
          if (o === "==") return x === y ? 1 : 0;
          return x !== y ? 1 : 0;
        };
      })(e, d, op);
    }
    try {
      var raiz = expressao();
      pula();
      if (pos < len) erro("sobra '" + src.slice(pos) + "'");
      return { ok: true, fn: raiz };
    } catch (e9) {
      return { ok: false, erro: e9.message };
    }
  }

  /* ===== O3-1: TREE GRID ===== */
  function montaArvorePlana(todos, base, p) {
    var arv = p.arvore;
    var chC = arv.chaveCampo, paiC = arv.pai;
    var idx = {}, j2, k2, l2;
    for (j2 = 0; j2 < todos.length; j2++) idx[String(todos[j2][chC])] = todos[j2];
    var semFiltro = base.length === todos.length;
    var casa = {}, incl = {}, ctx = {};
    for (j2 = 0; j2 < base.length; j2++) {
      k2 = String(base[j2][chC]);
      casa[k2] = true;
      incl[k2] = true;
    }
    if (!semFiltro) {
      /* s\u00f3 com filtro: subir ancestrais como contexto */
      for (j2 = 0; j2 < base.length; j2++) {
        var pv = base[j2][paiC], guard = 0;
        while (pv != null && pv !== "" && guard++ < 200) {
          var pk = String(pv);
          if (incl[pk]) break;
          if (!idx[pk]) break;
          incl[pk] = true;
          ctx[pk] = true;
          pv = idx[pk][paiC];
        }
      }
    }
    /* filhos por pai, ordenados pela base (casantes primeiro na ordem dela; ctx pela ordem dos dados) */
    var filhos = {}, raizes = [], todosOrd = [];
    for (j2 = 0; j2 < base.length; j2++) todosOrd.push(base[j2]);
    for (j2 = 0; j2 < todos.length; j2++) { k2 = String(todos[j2][chC]); if (incl[k2] && !casa[k2]) todosOrd.push(todos[j2]); }
    for (j2 = 0; j2 < todosOrd.length; j2++) {
      l2 = todosOrd[j2];
      k2 = String(l2[chC]);
      var pv2 = l2[paiC];
      var pk2 = (pv2 == null || pv2 === "") ? null : String(pv2);
      if (pk2 == null || !incl[pk2]) raizes.push(k2);
      else {
        if (!filhos[pk2]) filhos[pk2] = [];
        filhos[pk2].push(k2);
      }
    }
    /* agrega\u00e7\u00e3o recursiva sobre CASANTES */
    var aggCampos = [];
    if (arv.agregar && p.aggCols) for (j2 = 0; j2 < p.aggCols.length; j2++) if (p.aggCols[j2].agregador === "sum") aggCampos.push(p.aggCols[j2].campo);
    var aggMemo = {};
    var _emAgg = {};
    function agregaNo(k9) {
      if (aggMemo[k9]) return aggMemo[k9];
      if (_emAgg[k9]) { var z9 = {}; for (var az = 0; az < aggCampos.length; az++) z9[aggCampos[az]] = 0; return z9; }
      _emAgg[k9] = 1;
      var o9 = {}, a2;
      for (a2 = 0; a2 < aggCampos.length; a2++) {
        var v9 = casa[k9] ? Number(idx[k9] && idx[k9][aggCampos[a2]]) : 0;
        o9[aggCampos[a2]] = v9 === v9 ? v9 : 0;
      }
      var fs9 = filhos[k9] || [];
      for (a2 = 0; a2 < fs9.length; a2++) {
        var sub = agregaNo(fs9[a2]);
        for (var a3 = 0; a3 < aggCampos.length; a3++) o9[aggCampos[a3]] += sub[aggCampos[a3]];
      }
      aggMemo[k9] = o9;
      return o9;
    }
    /* DFS achatado pulando recolhidos */
    var abertos = arv.abertos || {};
    var ini9 = arv.inicioAberto == null ? 1 : arv.inicioAberto;
    function estaAberto(k9, nivel9) {
      if (k9 in abertos) return !!abertos[k9];
      return nivel9 < ini9;
    }
    var plana = [];
    function visita(k9, nivel9) {
      var l9 = idx[k9];
      if (!l9) return;
      var fs9 = filhos[k9] || [];
      var ab9 = estaAberto(k9, nivel9);
      var no9 = { __arv: { chave: k9, nivel: nivel9, temFilhos: fs9.length > 0, aberto: ab9, ctx: !!ctx[k9], agg: (arv.agregar && fs9.length && aggCampos.length) ? agregaNo(k9) : null } };
      for (var kk in l9) no9[kk] = l9[kk];
      plana.push(no9);
      if (fs9.length && ab9) for (var f2 = 0; f2 < fs9.length; f2++) visita(fs9[f2], nivel9 + 1);
    }
    for (j2 = 0; j2 < raizes.length; j2++) visita(raizes[j2], 0);
    return plana;
  }

  function _workerSrc() {
    var fns = [trocaAcento, semAcento, chave, chaveOrd, ordenaEstavel, termosBusca, passaCondicao, passaExpr, scoreBusca, filtraEScoreBusca, aplicaFiltros, agrupa, achata, novoAcum, acumula, fechaAcum, ordenaEstavelMulti, montaArvorePlana, fonteLocal];
    var nomes = ["trocaAcento", "semAcento", "chave", "chaveOrd", "ordenaEstavel", "termosBusca", "passaCondicao", "passaExpr", "scoreBusca", "filtraEScoreBusca", "aplicaFiltros", "agrupa", "achata", "novoAcum", "acumula", "fechaAcum", "ordenaEstavelMulti", "montaArvorePlana", "fonteLocal"];
    var partes = ["\"use strict\";", "var root = self;", "function agora(){return (root.performance&&root.performance.now)?root.performance.now():Date.now();}",
      "var CUSTO_COND = " + JSON.stringify(CUSTO_COND) + ";",
      "var MAPA_ACENTOS = " + JSON.stringify(MAPA_ACENTOS) + ";",
      "var AGGS = { sum: " + AGGS.sum.toString() + ", avg: " + AGGS.avg.toString() + ", count: " + AGGS.count.toString() + ", min: " + AGGS.min.toString() + ", max: " + AGGS.max.toString() + " };"];
    var j2;
    for (j2 = 0; j2 < fns.length; j2++) partes.push("var " + nomes[j2] + " = " + fns[j2].toString() + ";");
    partes.push(
      "var f = null;\n" +
      "self.onmessage = function (e) {\n" +
      "  var m = e.data;\n" +
      "  if (m.op === \"init\") { f = fonteLocal(m.dados); self.postMessage({ id: m.id, ok: 1 }); return; }\n" +
      "  if (!f) { self.postMessage({ id: m.id, erro: \"sem init\" }); return; }\n" +
      "  if (m.op === \"carregar\") { f.carregar(m.params, function (err, resp) { self.postMessage({ id: m.id, resp: resp }); }); return; }\n" +
      "  if (m.op === \"distintos\") {\n" +
      "    var base = aplicaFiltros(f.todos, m.sem || []);\n" +
      "    var mapa = {}, ordem = [], temNulos = false, j3, v3, k3;\n" +
      "    for (j3 = 0; j3 < base.length; j3++) {\n" +
      "      v3 = base[j3][m.campo];\n" +
      "      if (v3 == null || v3 === \"\") { temNulos = true; continue; }\n" +
      "      k3 = String(v3);\n" +
      "      if (!mapa[k3]) { mapa[k3] = { chave: k3, cru: v3 }; ordem.push(mapa[k3]); }\n" +
      "    }\n" +
      "    ordem.sort(function (a, b) { var ka = chaveOrd(a.cru, m.tipoC), kb = chaveOrd(b.cru, m.tipoC); return ka.v < kb.v ? -1 : ka.v > kb.v ? 1 : 0; });\n" +
      "    self.postMessage({ id: m.id, itens: ordem, temNulos: temNulos });\n" +
      "    return;\n" +
      "  }\n" +
      "  if (m.op === \"paths\") {\n" +
      "    var base2 = aplicaFiltros(f.todos, m.filtros || []);\n" +
      "    var tipos2 = m.tipos || {};\n" +
      "    var arv = agrupa(base2, m.grupos || [], [], function (c9) { return tipos2[c9] || \"texto\"; });\n" +
      "    var paths = [];\n" +
      "    (function coleta(nos) { var j4; for (j4 = 0; j4 < nos.length; j4++) { paths.push(nos[j4].path); if (nos[j4].filhos) coleta(nos[j4].filhos); } })(arv);\n" +
      "    self.postMessage({ id: m.id, paths: paths });\n" +
      "    return;\n" +
      "  }\n" +
      "  if (m.op === \"invalidar\") { f.invalidar(m.campo); self.postMessage({ id: m.id, ok: 1 }); return; }\n" +
      "};");
    return partes.join("\n");
  }
  var criaWorkerImpl = function (src) {
    var blob = new Blob([src], { type: "application/javascript" });
    var url = URL.createObjectURL(blob);
    var w = new Worker(url);
    w._phxUrl = url;
    return w;
  };
  function fonteWorker(dados) {
    var w = criaWorkerImpl(_workerSrc());
    var pend = {}, seq = 0;
    w.onmessage = function (e) {
      var m = e.data, cb = pend[m.id];
      if (cb) { delete pend[m.id]; cb(m); }
    };
    function post(op, payload, cb) {
      var id = ++seq;
      pend[id] = cb || function () {};
      var msg = { id: id, op: op };
      var k2;
      for (k2 in payload) msg[k2] = payload[k2];
      w.postMessage(msg);
    }
    post("init", { dados: dados });
    return {
      local: false,
      worker: true,
      carregar: function (p, cb) { post("carregar", { params: p }, function (m) { cb(m.erro || null, m.resp); }); },
      distintos: function (campo, sem, tipoC, cb) { post("distintos", { campo: campo, sem: sem, tipoC: tipoC }, function (m) { cb(m.itens || [], !!m.temNulos); }); },
      paths: function (filtros2, grupos2, tipos2, cb) { post("paths", { filtros: filtros2, grupos: grupos2, tipos: tipos2 }, function (m) { cb(m.paths || []); }); },
      invalidar: function (campo) { post("invalidar", { campo: campo }, null); },
      destruir: function () { try { w.terminate(); if (w._phxUrl) URL.revokeObjectURL(w._phxUrl); } catch (e2) {} }
    };
  }
  function criar(alvo, cfg) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo não encontrado: " + alvo };
    if (!cfg || !cfg.colunas || !cfg.colunas.length) return { ok: false, erro: "cfg.colunas é obrigatório" };

    var logs = [];
    var aoLog = cfg.aoLog || null;
    function log(ev, extra) {
      var e = { ev: "phx.grid." + ev, t: Date.now() };
      var k; for (k in extra) if (Object.prototype.hasOwnProperty.call(extra, k)) e[k] = extra[k];
      if (aoLog) try { aoLog(e); } catch (e9) {}
      logs.push(e);
      if (cfg.logConsole && root.console) console.log("[phx.grid]", ev, extra || "");
      return e;
    }

    var t0init = agora();
    var fonte = cfg.fonte || (cfg.worker && typeof Worker !== "undefined" ? fonteWorker(cfg.dados || []) : fonteLocal(cfg.dados || []));
    var colunasDef = cfg.colunas.slice();
    var porCampo = {};
    var ordemColunas = [];
    var ocultas = {};
    var i;
    for (i = 0; i < colunasDef.length; i++) {
      porCampo[colunasDef[i].campo] = colunasDef[i];
      ordemColunas.push(colunasDef[i].campo);
      if (colunasDef[i].oculta) ocultas[colunasDef[i].campo] = true;
    }
    function normalizaNo(item) {
      if (typeof item === "string") return { campo: item };
      var no2 = { titulo: item.titulo || "", filhos: [] }, j2;
      if (item.colunas) for (j2 = 0; j2 < item.colunas.length; j2++) no2.filhos.push({ campo: item.colunas[j2] });
      if (item.filhos) for (j2 = 0; j2 < item.filhos.length; j2++) no2.filhos.push(normalizaNo(item.filhos[j2]));
      return no2;
    }
    var arvore = { filhos: [] };
    if (cfg.bandas) {
      for (i = 0; i < cfg.bandas.length; i++) arvore.filhos.push(normalizaNo(cfg.bandas[i]));
    } else {
      for (i = 0; i < ordemColunas.length; i++) arvore.filhos.push({ campo: ordemColunas[i] });
    }
    function profundidade(no2) {
      if (no2.campo) return 0;
      var m = 0, j2, p2;
      for (j2 = 0; j2 < no2.filhos.length; j2++) { p2 = profundidade(no2.filhos[j2]); if (p2 > m) m = p2; }
      return 1 + m;
    }
    function flatten(no2, out) {
      var j2;
      if (no2.campo) { out.push(no2.campo); return out; }
      for (j2 = 0; j2 < no2.filhos.length; j2++) flatten(no2.filhos[j2], out);
      return out;
    }
    function contaVisiveis(no2) {
      if (no2.campo) return (ocultas[no2.campo] || agrupada(no2.campo)) ? 0 : 1;
      var t2 = 0, j2;
      for (j2 = 0; j2 < no2.filhos.length; j2++) t2 += contaVisiveis(no2.filhos[j2]);
      return t2;
    }
    function paiDe(no2, campo) {
      var j2, r2;
      for (j2 = 0; j2 < no2.filhos.length; j2++) {
        if (no2.filhos[j2].campo === campo) return no2;
        if (no2.filhos[j2].filhos) { r2 = paiDe(no2.filhos[j2], campo); if (r2) return r2; }
      }
      return null;
    }
    ordemColunas = flatten(arvore, []);

    var temSelecao = !!cfg.selecao && cfg.selecao !== "celula" && cfg.selecao !== "coluna";
    var temDetalhe = !!(cfg.detalhe && cfg.detalhe.render);
    var detalhesAbertos = {};
    var temTotais = !!cfg.totais;
    var dadosSel = {};
    var focoR = 0, focoC = 0, rovingAtivo = false, celFocoAtual = null;
    var histNL = [];
    var _resumoCache = null;
    var _regras = [], _seqRegra = 0, _pendAlertas = [];
    var _recortes = {};
    var colunasFormula = [], _acumulados = [];
    (function () {
      var j9, c9, comp9;
      for (j9 = 0; j9 < colunasDef.length; j9++) {
        c9 = colunasDef[j9];
        if (c9.formula != null) {
          if (typeof c9.formula === "function") colunasFormula.push({ campo: c9.campo, fn: c9.formula, direta: true });
          else {
            comp9 = compilaFormula(String(c9.formula));
            if (comp9.ok) colunasFormula.push({ campo: c9.campo, fn: comp9.fn });
            else log("formula-erro", { campo: c9.campo, erro: comp9.erro });
          }
        }
        if (c9.acumulado) {
          var acDef = typeof c9.acumulado === "string" ? { campo: c9.acumulado } : c9.acumulado;
          _acumulados.push({ campo: c9.campo, base: acDef.campo, inicial: acDef.inicial || 0 });
        }
      }
    })();
    function materializaFormulas(linhas9) {
      if (!colunasFormula.length || !linhas9) return;
      var j9, k9, f9, v9;
      for (j9 = 0; j9 < linhas9.length; j9++) {
        for (k9 = 0; k9 < colunasFormula.length; k9++) {
          f9 = colunasFormula[k9];
          try { v9 = f9.direta ? f9.fn(linhas9[j9], j9) : f9.fn(linhas9[j9]); } catch (e9) { v9 = NaN; }
          linhas9[j9][f9.campo] = v9 === v9 ? v9 : null;
        }
      }
    }
    if (cfg.dados) materializaFormulas(cfg.dados);
    var temArvore = !!(cfg.arvore && cfg.arvore.pai);
    var temLancamento = !!cfg.lancamento;
    var lancCfg = temLancamento ? (cfg.lancamento === true ? {} : cfg.lancamento) : null;
    var lancInvalidas = {};
    var lancSeq = 0;
    var papelAtual = cfg.papel || null;
    var registroAtual = 0, celulaSel = null, _trArrasto = null, _dicaRol = null, _alturaLinha = null, _destruido = false;
    var _cacheBusca = {}, _pendentesBusca = [];
    var temNavegador = !!cfg.navegador, temCabLinha = !!cfg.cabecalhoLinha;
    var temDicas = !!cfg.dicas;
    var _instId = null;
    var estilosNomeados = {}, condicoes = [], _slotsLayout = {};
    var _styleEl = null;
    function cssDeEstilo(def9) {
      var p9 = [];
      if (def9.corTexto) p9.push("color:" + def9.corTexto);
      if (def9.corFundo) p9.push("background:" + def9.corFundo);
      if (def9.negrito != null) p9.push("font-weight:" + (def9.negrito ? "700" : "400"));
      if (def9.italico) p9.push("font-style:italic");
      if (def9.sublinhado || def9.riscado) p9.push("text-decoration:" + (def9.sublinhado ? "underline " : "") + (def9.riscado ? "line-through" : ""));
      if (def9.tamanho) p9.push("font-size:" + (typeof def9.tamanho === "number" ? def9.tamanho + "px" : def9.tamanho));
      if (def9.fonte) p9.push("font-family:" + def9.fonte);
      if (def9.alinhamento) p9.push("text-align:" + def9.alinhamento);
      if (def9.imagemFundo) p9.push("background-image:url(" + JSON.stringify(String(def9.imagemFundo)) + ");background-size:" + (def9.ajusteFundo || "cover") + ";background-repeat:" + (def9.repetirFundo ? "repeat" : "no-repeat") + ";background-position:center");
      if (def9.imagem) p9.push("background-image:url(" + JSON.stringify(String(def9.imagem)) + ");background-repeat:no-repeat;background-position:6px center;background-size:16px 16px;padding-left:26px");
      return p9.join(";");
    }
    function regravaEstilos() {
      if (!_instId) _instId = wrap.getAttribute("data-phx-inst");
      if (!_styleEl) { _styleEl = document.createElement("style"); _styleEl.setAttribute("data-phx-estilos", _instId); wrap.appendChild(_styleEl); }
      var r9 = [], n9;
      for (n9 in estilosNomeados) r9.push('.phx-grid[data-phx-inst="' + _instId + '"] .phx-est-' + n9 + "{" + cssDeEstilo(estilosNomeados[n9]) + "}");
      _styleEl.textContent = r9.join("\n");
    }
    function classesCondicao(campoCel9, linha9) {
      var out9 = "", j9, cd9;
      for (j9 = 0; j9 < condicoes.length; j9++) {
        cd9 = condicoes[j9];
        if (cd9.ativa === false) continue;
        if (cd9.alvo !== "linha" && cd9.campo !== campoCel9 && cd9.aplicarEm !== campoCel9) continue;
        if (testaCondicao(cd9, linha9)) out9 += " phx-est-" + cd9.estilo;
      }
      return out9;
    }
    var ocultasAdapt = {};
    var _menuCtx = null, _fechaSaida = null, _fechaVisib = null, _dicaRolTimer = null;
    function _fechaCtxGlobal() { api.fecharMenuContexto(); }
    var adaptativo = !!cfg.adaptativo;
    var adaptCfg = (cfg.adaptativo && typeof cfg.adaptativo === "object") ? cfg.adaptativo : {};
    var regrasAcesso = cfg.acesso || null;
    var temHistCel = !!cfg.historicoCelula;
    var histCfg = (cfg.historicoCelula && typeof cfg.historicoCelula === "object") ? cfg.historicoCelula : {};
    var histCel = {};
    var histMax = histCfg.max || 30;
    function _naLista(lista9, papel9) {
      if (lista9 == null) return true;
      if (lista9 === true) return true;
      if (lista9 === false) return false;
      if (typeof lista9 === "string") lista9 = [lista9];
      if (!(lista9 instanceof Array)) return true;
      var j9;
      for (j9 = 0; j9 < lista9.length; j9++) if (String(lista9[j9]) === String(papel9)) return true;
      return false;
    }
    function acessoDe(campo9) {
      var r9 = regrasAcesso ? regrasAcesso[campo9] : null;
      if (!r9) return { ver: true, editar: true, mascara: null };
      var podeVer9 = _naLista(r9.ver, papelAtual);
      return {
        ver: podeVer9 || !!r9.mascara,
        oculta: !podeVer9 && !r9.mascara,
        mascarada: !podeVer9 && !!r9.mascara,
        editar: podeVer9 && _naLista(r9.editar, papelAtual),
        mascara: podeVer9 ? null : (r9.mascara || null)
      };
    }
    function aplicaMascara(c9, v9, linha9) {
      if (!regrasAcesso || !c9 || !c9.campo) return v9;
      var a9 = acessoDe(c9.campo);
      if (!a9.mascara) return v9;
      if (typeof a9.mascara === "function") { try { return a9.mascara(v9, linha9); } catch (e9) { return "\u2022\u2022\u2022"; } }
      if (a9.mascara === "total") return "\u2022\u2022\u2022\u2022";
      if (a9.mascara === "parcial") {
        var t9 = v9 == null ? "" : String(v9);
        return t9.length <= 4 ? "\u2022\u2022\u2022\u2022" : "\u2022\u2022\u2022" + t9.slice(-4);
      }
      return String(a9.mascara);
    }
    function registraHist(chave9, campo9, de9, para9) {
      if (!temHistCel) return;
      var k9 = chave9 + "\u0000" + campo9;
      if (!histCel[k9]) histCel[k9] = [];
      var ent9 = { chave: String(chave9), campo: campo9, de: de9 == null ? null : de9, para: para9 == null ? null : para9, quando: new Date().toISOString(), usuario: histCfg.usuario || cfg.usuario || null };
      histCel[k9].push(ent9);
      if (histCel[k9].length > histMax) histCel[k9].shift();
      log("hist-celula", ent9);
    }
    var lancMaxNum = null;
    function proximaChaveLanc() {
      var colCh9 = chaveCampo ? porCampo[chaveCampo] : null;
      var tipo9 = colCh9 && colCh9.tipo;
      lancSeq++;
      if (tipo9 !== "numero" && tipo9 !== "moeda" && tipo9 !== "percentual") return "novo-" + lancSeq;
      if (lancMaxNum == null) {
        var td9 = fonte.local && fonte.todos ? fonte.todos : [];
        var mx9 = 0, j9, v9;
        for (j9 = 0; j9 < td9.length; j9++) { v9 = Number(td9[j9][chaveCampo]); if (v9 === v9 && v9 > mx9) mx9 = v9; }
        lancMaxNum = mx9;
      }
      lancMaxNum++;
      return lancMaxNum;
    }
    var abertosArv = {};
    var inicioAbertoRt = temArvore ? (cfg.arvore.inicioAberto == null ? 1 : cfg.arvore.inicioAberto) : 1;
    function _idxArv() {
      var o9 = {}, j9;
      var td9 = fonte.local && fonte.todos ? fonte.todos : [];
      for (j9 = 0; j9 < td9.length; j9++) o9[String(td9[j9][chaveCampo])] = td9[j9];
      return o9;
    }
    function _nivelDe(k9) {
      if (!temArvore) return 0;
      var idx9 = _idxArv(), n9 = 0, atual9 = k9, guard9 = 0;
      while (idx9[atual9] && guard9++ < 200) {
        var pv9 = idx9[atual9][cfg.arvore.pai];
        if (pv9 == null || pv9 === "") break;
        n9++;
        atual9 = String(pv9);
      }
      return n9;
    }
    if (cfg.recortes && typeof cfg.recortes.carregar === "function") {
      try { _recortes = cfg.recortes.carregar() || {}; } catch (e9) { _recortes = {}; }
    }
    function _salvaRecortes() {
      if (cfg.recortes && typeof cfg.recortes.salvar === "function") {
        try { cfg.recortes.salvar(JSON.parse(JSON.stringify(_recortes))); } catch (e9) {}
      }
    }
    function _linhasDosFiltros(lista9) {
      return fonte.local && fonte.todos ? aplicaFiltros(fonte.todos, lista9) : [];
    }
    var _badgeEl = null;
    function _ctxNL() {
      var ctx9 = { colunas: [], distintos: {} }, j8, c8;
      for (j8 = 0; j8 < colunasDef.length; j8++) ctx9.colunas.push({ campo: colunasDef[j8].campo, titulo: colunasDef[j8].titulo, tipo: colunasDef[j8].tipo || "texto" });
      if (fonte.local && fonte.todos) {
        for (j8 = 0; j8 < colunasDef.length; j8++) {
          c8 = colunasDef[j8];
          if (["numero", "moeda", "percentual", "data", "dataHora"].indexOf(c8.tipo || "texto") >= 0) continue;
          var vis8 = {}, arr8 = [], i8;
          for (i8 = 0; i8 < fonte.todos.length && arr8.length <= 50; i8++) {
            var v8 = fonte.todos[i8][c8.campo];
            if (v8 == null || v8 === "") continue;
            var k8 = String(v8);
            if (!vis8[k8]) { vis8[k8] = 1; arr8.push(k8); }
          }
          if (arr8.length && arr8.length <= 50) ctx9.distintos[c8.campo] = arr8;
        }
      }
      return ctx9;
    }
    function _atualizaBadge() {
      if (!_badgeEl) {
        _badgeEl = el("div", "phx-alerta-badge");
        _badgeEl.hidden = true;
        wrap.appendChild(_badgeEl);
      }
      if (_pendAlertas.length) {
        _badgeEl.hidden = false;
        _badgeEl.textContent = "\uD83D\uDD14 " + _pendAlertas.length;
      } else _badgeEl.hidden = true;
    }
    function _salvaRegras() {
      if (cfg.alertas && typeof cfg.alertas.salvar === "function") {
        var lim = [], j8;
        for (j8 = 0; j8 < _regras.length; j8++) lim.push({ id: _regras[j8].id, frase: _regras[j8].frase, cbNome: _regras[j8].cbNome || null, soNovas: _regras[j8].soNovas });
        try { cfg.alertas.salvar(lim); } catch (e9) {}
      }
    }
    function avaliaAlertas(linhasNovas) {
      var j8, r8, base8, lista8, hits8;
      for (j8 = 0; j8 < _regras.length; j8++) {
        r8 = _regras[j8];
        base8 = (r8.soNovas && linhasNovas) ? linhasNovas : (fonte.local && fonte.todos ? fonte.todos : []);
        lista8 = r8.filtros.slice();
        if (r8.busca) lista8.push({ campo: "*", tipo: "busca", termo: r8.busca, campos: camposBuscaveis() });
        hits8 = aplicaFiltros(base8, lista8);
        if (hits8.length) {
          var payload8 = { regra: { id: r8.id, frase: r8.frase }, total: hits8.length, hits: hits8.slice(0, 20) };
          _pendAlertas.push({ regraId: r8.id, frase: r8.frase, total: hits8.length, quando: Date.now() });
          log("alerta", { id: r8.id, frase: r8.frase, total: hits8.length });
          anuncia("Alerta: " + r8.frase + " \u2014 " + hits8.length + " linha" + (hits8.length > 1 ? "s" : ""));
          if (r8.aoDisparar) try { r8.aoDisparar(r8.cbNome ? JSON.stringify(payload8) : payload8); } catch (e9) {}
        }
      }
      _atualizaBadge();
    }
    if (cfg.historico && typeof cfg.historico.carregar === "function") {
      try { histNL = cfg.historico.carregar() || []; } catch (e9) { histNL = []; }
    }
    function salvaHist() {
      if (cfg.historico && typeof cfg.historico.salvar === "function") {
        try { cfg.historico.salvar(histNL.slice()); } catch (e9) {}
      }
    }   /* r: índice do tr de dados na página/janela; c: índice da célula (com extras) */
    var liveEl = null;
    function anuncia(msg) {
      if (!liveEl) return;
      liveEl.textContent = "";
      setTimeout(function () { liveEl.textContent = msg; }, 30);
    }
    function extrasN() { return (temDetalhe ? 1 : 0) + (temSelecao ? 1 : 0); }
    function colCount() { return visiveis().length + extrasN(); }
    function trsDados() {
      var out = [], trs = tbody.children, j2;
      for (j2 = 0; j2 < trs.length; j2++) {
        var cl = trs[j2].className;
        if (cl.indexOf("phx-grupo") >= 0 || cl.indexOf("phx-detalhe") >= 0 || cl.indexOf("phx-vspacer") >= 0) continue;
        out.push(trs[j2]);
      }
      return out;
    }
    function celDe(r2, c2) {
      var trs = trsDados();
      if (r2 < 0 || r2 >= trs.length) return null;
      return trs[r2].children[c2] || null;
    }
    function aplicaFoco(scroll) {
      if (!rovingAtivo) return;
      var trs = trsDados();
      if (!trs.length) return;
      if (focoR >= trs.length) focoR = trs.length - 1;
      if (focoR < 0) focoR = 0;
      var nC = colCount();
      if (focoC >= nC) focoC = nC - 1;
      if (focoC < 0) focoC = 0;
      if (celFocoAtual && celFocoAtual.setAttribute) {
        celFocoAtual.setAttribute("tabindex", "-1");
        celFocoAtual.className = celFocoAtual.className.replace(" phx-cel-foco", "");
      }
      var cel = celDe(focoR, focoC);
      celFocoAtual = cel;
      if (!cel) return;
      cel.setAttribute("tabindex", "0");
      cel.className += " phx-cel-foco";
      try { cel.focus({ preventScroll: !scroll }); } catch (e9) { try { cel.focus(); } catch (e8) {} }
    }
    var ALT_DENS = { compacta: 28, confortavel: 36, espacosa: 44 };
    var temaAtual = cfg.tema || "claro";
    var densAtual = cfg.densidade || "confortavel";
    var mqEscuro = null, mqHandler = null;
    function aplicaTemaClasse() {
      var escuro = temaAtual === "escuro" || (temaAtual === "auto" && mqEscuro && mqEscuro.matches);
      wrap.className = wrap.className.replace(/ phx-tema-escuro/g, "");
      if (escuro) wrap.className += " phx-tema-escuro";
    }
    function aplicaDensClasse() {
      wrap.className = wrap.className.replace(/ phx-dens-\w+/g, "");
      if (densAtual !== "confortavel") wrap.className += " phx-dens-" + densAtual;
    }
    var cfgV = cfg.virtual === true ? {} : (cfg.virtual || null);
    var ehVirtual = !!cfgV;
    var vAltura = (cfgV && cfgV.altura) || 520;
    var vLinha = (cfgV && cfgV.alturaLinha) || ALT_DENS[densAtual] || 36;
    var vOver = (cfgV && cfgV.overscan) || 8;
    var vNVis = Math.ceil(vAltura / vLinha);
    var cacheV = null;
    var vIni = 0, vFim = 0;
    var vRaf = null;
    var ouvintesDoc = [];
    function docOn(ev9, fn9, cap9) { document.addEventListener(ev9, fn9, cap9 || false); ouvintesDoc.push([ev9, fn9, cap9 || false]); }
    var aoMudarLayout = cfg.aoMudarLayout || null;
    var layoutTimer = null;
    function agendaLayoutCb() {
      if (!aoMudarLayout) return;
      if (layoutTimer) clearTimeout(layoutTimer);
      layoutTimer = setTimeout(function () { layoutTimer = null; aoMudarLayout(api.layout()); }, 300);
    }
    var temEdicao = !!cfg.edicao || (function () { var j9; for (j9 = 0; j9 < (cfg.colunas || []).length; j9++) if (cfg.colunas[j9].editavel) return true; return false; })();
    var aoEditar = (cfg.edicao && cfg.edicao.aoEditar) || null;
    var edicaoAtiva = null;   /* {ixL, campo} */
    function parseEntrada(c9, txt) {
      var tipo9 = c9.tipo || "texto";
      if (tipo9 === "numero" || tipo9 === "moeda" || tipo9 === "percentual") {
        var t9 = String(txt).replace(/\s|R\$/g, "");
        if (t9.indexOf(",") >= 0) t9 = t9.replace(/\./g, "").replace(",", ".");
        var n9 = Number(t9);
        return n9 === n9 && t9 !== "" ? { ok: true, valor: n9 } : { ok: false };
      }
      if (tipo9 === "data") return txt ? { ok: true, valor: txt } : { ok: false };
      return { ok: true, valor: String(txt) };
    }
    var temNotas = !!cfg.notas;
    var notas = (cfg.notas && cfg.notas.dados) || {};
    var aoMudarNota = (cfg.notas && cfg.notas.aoMudar) || null;
    var chaveCampo = cfg.chave || null;
    var selecionadas = {};
    var nSel = 0;
    var ancoraSel = -1;
    function chaveDe(linha, ix) { return chaveCampo ? String(linha[chaveCampo]) : "#" + ix; }

    var filtros = {};
    var grupos = [];
    var recolhidos = {};
    function agrupada(campo) {
      var j2, k2;
      for (j2 = 0; j2 < grupos.length; j2++) {
        if (grupos[j2] === campo) return true;
        if (grupos[j2] && typeof grupos[j2] !== "string") for (k2 = 0; k2 < grupos[j2].length; k2++) if (grupos[j2][k2] === campo) return true;
      }
      return false;
    }
    function serializaFiltros() {
      var campos = [], k3, out = [], j2, f2;
      for (k3 in filtros) if (filtros[k3]) campos.push(k3);
      campos.sort();
      for (j2 = 0; j2 < campos.length; j2++) {
        f2 = filtros[campos[j2]];
        var c2 = porCampo[campos[j2]];
        var o2 = { campo: campos[j2], tipo: f2.tipo, tipoCol: (c2 && c2.tipo) || "texto" };
        if (f2.tipo === "busca") { o2.termo = f2.termo; o2.campos = f2.campos.slice(); }
        if (f2.tipo === "valores") { o2.valores = f2.valores.slice(); if (f2.incluiNulos) o2.incluiNulos = true; }
        if (f2.tipo === "texto") o2.contem = f2.contem;
        if (f2.tipo === "faixa") { o2.de = f2.de; o2.ate = f2.ate; }
        if (f2.tipo === "expr") { o2.op = f2.op; o2.valor = f2.valor; }
        if (f2.tipo === "multi") {
          o2.combinador = f2.combinador;
          o2.condicoes = [];
          var j3;
          for (j3 = 0; j3 < f2.condicoes.length; j3++) o2.condicoes.push({ op: f2.condicoes[j3].op, valor: f2.condicoes[j3].valor });
        }
        out.push(o2);
      }
      return out;
    }
    function resumoFiltro(campo, f2) {
      var c2 = porCampo[campo], nome = campo === "*" ? "Busca" : ((c2 && c2.titulo) || campo);
      if (f2.tipo === "busca") return 'Busca: "' + f2.termo + '"';
      function fv(v) { return c2 && (c2.tipo === "moeda" || c2.tipo === "numero" || c2.tipo === "percentual") ? fmt.numero(v, c2.tipo === "moeda" ? 2 : 0) : String(v); }
      if (f2.tipo === "valores") {
        var mostra = f2.valores.slice(0, 2).join(", ");
        if (f2.valores.length > 2) mostra += " +" + (f2.valores.length - 2);
        return nome + ": " + mostra;
      }
      if (f2.tipo === "texto") return nome + ': "' + f2.contem + '"';
      if (f2.tipo === "faixa") {
        if (f2.de != null && f2.ate != null) return nome + ": " + fv(f2.de) + "\u2013" + fv(f2.ate);
        if (f2.de != null) return nome + " \u2265 " + fv(f2.de);
        return nome + " \u2264 " + fv(f2.ate);
      }
      if (f2.tipo === "expr") return nome + " " + f2.op + " " + fv(f2.valor);
      if (f2.tipo === "multi") {
        var ps = [], j3;
        for (j3 = 0; j3 < f2.condicoes.length; j3++) ps.push(f2.condicoes[j3].op + " " + fv(f2.condicoes[j3].valor));
        return nome + " " + ps.join(f2.combinador === "ou" ? " OU " : " E ");
      }
      return nome;
    }

    var estado = {
      ordem: { campo: null, dir: null },
      ordens: [],
      pagina: 1,
      tamanho: (cfg.pagina && cfg.pagina.tamanho) || 100,
      total: 0
    };
    var opcoesTam = (cfg.pagina && cfg.pagina.opcoes) || [50, 100, 200];

    var wrap = el("div", "phx-grid");
    wrap.setAttribute("data-phx-inst", "i" + (++_seqInst));
    wrap.innerHTML =
      '<div class="phx-envoltorio"><table class="phx-tabela">' +
      '<thead></thead><tbody></tbody><tfoot class="phx-tfoot"></tfoot></table></div>' +
      '<div class="phx-rodape">' +
      '<div class="phx-pag"></div>' +
      '<div class="phx-rodape-dir">' +
      (temNavegador ? '<span class="phx-nav"><span class="phx-nav-rot">' + T("registro") + ':</span><button type="button" class="phx-pg phx-nav-b" data-nav="primeiro">\u23ee</button><button type="button" class="phx-pg phx-nav-b" data-nav="ant">\u25c0</button><input class="phx-nav-n" type="number" min="1" value="1"><span class="phx-nav-de"></span><button type="button" class="phx-pg phx-nav-b" data-nav="prox">\u25b6</button><button type="button" class="phx-pg phx-nav-b" data-nav="ultimo">\u23ed</button></span>' : "") +
      '<select class="phx-tam"></select><span class="phx-tam-rotulo">' + T("itensPorPagina") + '</span>' +
      '<span class="phx-mostrando"></span>' +
      '<span class="phx-colsel-envoltorio"><button type="button" class="phx-colsel-btn"></button>' +
      '<div class="phx-colsel" hidden></div></span>' +
      "</div></div>";
    var LIMITE_LISTA_EXCEL = 500;
    function valoresDistintos(campo, cb) {
      var c2w = porCampo[campo];
      if (fonte.worker && fonte.distintos) {
        var lista9 = serializaFiltros(), sem9 = [], j9;
        for (j9 = 0; j9 < lista9.length; j9++) if (lista9[j9].campo !== campo) sem9.push(lista9[j9]);
        fonte.distintos(campo, sem9, (c2w && c2w.tipo) || "texto", function (itens, temNulos) {
          var out9 = [], k9;
          for (k9 = 0; k9 < itens.length; k9++) {
            out9.push({ chave: itens[k9].chave, cru: itens[k9].cru, rotulo: formata(c2w, itens[k9].cru, {}, 0), busca: "" });
            out9[k9].busca = semAcento(out9[k9].rotulo) + " " + semAcento(out9[k9].chave);
          }
          cb({ remoto: false, itens: out9, temNulos: temNulos, total: out9.length });
        });
        return;
      }
      if (!fonte.local) { cb({ remoto: true, itens: [], temNulos: false, total: 0 }); return; }
      var lista = serializaFiltros(), sem = [], j2;
      for (j2 = 0; j2 < lista.length; j2++) if (lista[j2].campo !== campo) sem.push(lista[j2]);
      var base = aplicaFiltros(fonte.todos, sem);
      var c2 = porCampo[campo], mapa = {}, ordem2 = [], temNulos = false, v2, k3;
      for (j2 = 0; j2 < base.length; j2++) {
        v2 = base[j2][campo];
        if (v2 == null || v2 === "") { temNulos = true; continue; }
        k3 = String(v2);
        if (!mapa[k3]) { mapa[k3] = { chave: k3, cru: v2, rotulo: formata(c2, v2, base[j2], 0), busca: "" }; ordem2.push(mapa[k3]); }
      }
      var tipoC = (c2 && c2.tipo) || "texto";
      ordem2.sort(function (a, b) {
        var ka = chaveOrd(a.cru, tipoC), kb = chaveOrd(b.cru, tipoC);
        if (ka.v < kb.v) return -1;
        if (ka.v > kb.v) return 1;
        return 0;
      });
      for (j2 = 0; j2 < ordem2.length; j2++) ordem2[j2].busca = semAcento(ordem2[j2].rotulo) + " " + semAcento(ordem2[j2].chave);
      cb({ remoto: false, itens: ordem2, temNulos: temNulos, total: ordem2.length });
    }
    function camposBuscaveis() {
      if (cfg.buscaveis) return cfg.buscaveis.slice();
      var v = visiveis(), out = [], j2, t2;
      for (j2 = 0; j2 < v.length; j2++) {
        t2 = v[j2].tipo || "texto";
        if (t2 === "texto" || t2 === "composta" || t2 === "link" || t2 === "badge") out.push(v[j2].campo);
      }
      return out;
    }
    var buscaEl = null, buscaContaEl = null;
    function atualizaContaBusca() {
      if (!buscaContaEl) return;
      buscaContaEl.textContent = filtros["*"] ? fmt.numero(estado.total) + " resultado" + (estado.total === 1 ? "" : "s") : "";
    }
    function montaBusca() {
      if (!cfg.buscaGlobal) return;
      var bb = el("div", "phx-busca");
      bb.innerHTML = '<input type="text" class="phx-busca-in" placeholder="Buscar em tudo\u2026 (v\u00e1rios termos = E)"><span class="phx-busca-conta"></span>';
      wrap.insertBefore(bb, wrap.firstChild);
      buscaEl = bb.querySelector("input");
      buscaContaEl = bb.querySelector(".phx-busca-conta");
      var aplica = debounce(function () { api.buscar(buscaEl.value); });
      buscaEl.addEventListener("input", aplica);
      buscaEl.addEventListener("keydown", function (e) {
        if (e.keyCode === 27) { buscaEl.value = ""; api.buscar(""); }
      });
    }
    var groupBox = null;
    function montaGroupBox() {
      if (!cfg.agrupavel) return;
      groupBox = el("div", "phx-groupbox");
      wrap.insertBefore(groupBox, wrap.firstChild);
      groupBox.addEventListener("dragover", function (e) { e.preventDefault(); groupBox.className = "phx-groupbox phx-groupbox-sobre"; });
      groupBox.addEventListener("dragleave", function () { groupBox.className = "phx-groupbox"; });
      groupBox.addEventListener("drop", function (e) {
        e.preventDefault();
        groupBox.className = "phx-groupbox";
        var campo = e.dataTransfer.getData("text/plain");
        if (campo && campo.indexOf("\u0002pill:") === 0) return;
        if (campo && porCampo[campo] && !agrupada(campo)) api.agrupar(grupos.concat([campo]));
      });
      renderGroupBox();
    }
    function renderGroupBox() {
      if (!groupBox) return;
      if (!grupos.length) {
        groupBox.innerHTML = '<span class="phx-groupbox-dica">' + esc(T("arrasteAgrupar")) + "</span>";
        return;
      }
      var html = "", j2, k2;
      function tituloDe(campo9) { return (porCampo[campo9] && porCampo[campo9].titulo) || campo9; }
      for (j2 = 0; j2 < grupos.length; j2++) {
        var el9 = grupos[j2], comp9 = el9 && typeof el9 !== "string";
        if (comp9) {
          html += '<span class="phx-gpill phx-gpill-comp" data-ix="' + j2 + '">';
          for (k2 = 0; k2 < el9.length; k2++) {
            html += '<span class="phx-gpill-parte" draggable="true" data-campo="' + esc(el9[k2]) + '" data-ix="' + j2 + '">' + esc(tituloDe(el9[k2])) +
              (el9.length > 1 ? ' <button type="button" class="phx-gpill-x-parte" title="tirar do composto">\u00d7</button>' : "") + "</span>" +
              (k2 < el9.length - 1 ? '<span class="phx-gpill-mais">+</span>' : "");
          }
          html += "</span>";
        } else {
          html += '<span class="phx-gpill" draggable="true" data-campo="' + esc(el9) + '" data-ix="' + j2 + '">' + esc(tituloDe(el9)) +
            ' <button type="button" class="phx-gpill-x" title="desagrupar">\u00d7</button></span>';
        }
        if (j2 < grupos.length - 1) html += '<button type="button" class="phx-gpill-seta" data-ix="' + j2 + '" title="clique para mesclar com o próximo (agrupamento composto)">\u2192</button>';
      }
      groupBox.innerHTML = html;
      /* remove um campo (de dentro de composto, ou o pill inteiro se simples) */
      function removeCampo(campoAlvo9) {
        var novo9 = [], j3, el3, k3;
        for (j3 = 0; j3 < grupos.length; j3++) {
          el3 = grupos[j3];
          if (el3 === campoAlvo9) continue;
          if (el3 && typeof el3 !== "string") {
            var restou9 = [];
            for (k3 = 0; k3 < el3.length; k3++) if (el3[k3] !== campoAlvo9) restou9.push(el3[k3]);
            if (restou9.length > 1) novo9.push(restou9);
            else if (restou9.length === 1) novo9.push(restou9[0]);
            continue;
          }
          novo9.push(el3);
        }
        api.agrupar(novo9);
      }
      /* mescla um campo (vindo de fora ou de outro pill) dentro do elemento no indice ixAlvo */
      function mesclaNoIndice(campoOrigem9, ixOrigem9, ixAlvo9) {
        if (ixOrigem9 === ixAlvo9) return;
        var novo9 = [], j3, elOrig9 = grupos[ixOrigem9];
        for (j3 = 0; j3 < grupos.length; j3++) {
          if (j3 === ixOrigem9) continue;
          if (j3 === ixAlvo9) {
            var alvoEl9 = grupos[j3], base9 = (alvoEl9 && typeof alvoEl9 !== "string") ? alvoEl9.slice() : [alvoEl9];
            var origEl9 = (ixOrigem9 == null) ? [campoOrigem9] : ((elOrig9 && typeof elOrig9 !== "string") ? elOrig9 : [elOrig9]);
            origEl9 = origEl9.filter(function (c9) { return base9.indexOf(c9) < 0; });
            /* preserva a ordem visual esquerda->direita quando os dois ja existiam no groupbox */
            var combinado9 = (ixOrigem9 != null && ixOrigem9 < ixAlvo9) ? origEl9.concat(base9) : base9.concat(origEl9);
            novo9.push(combinado9.length > 1 ? combinado9 : combinado9[0]);
            continue;
          }
          novo9.push(grupos[j3]);
        }
        api.agrupar(novo9);
      }
      var setas9 = groupBox.querySelectorAll(".phx-gpill-seta");
      for (j2 = 0; j2 < setas9.length; j2++) (function (bt9) {
        bt9.addEventListener("click", function () { mesclaNoIndice(null, Number(bt9.getAttribute("data-ix")), Number(bt9.getAttribute("data-ix")) + 1); });
      })(setas9[j2]);
      var pillsSimples = groupBox.querySelectorAll(".phx-gpill:not(.phx-gpill-comp), .phx-gpill-parte");
      for (j2 = 0; j2 < pillsSimples.length; j2++) {
        (function (pill) {
          var campoPill9 = pill.getAttribute("data-campo"), ixPill9 = Number(pill.getAttribute("data-ix"));
          var btnX9 = pill.querySelector(".phx-gpill-x, .phx-gpill-x-parte");
          if (btnX9) btnX9.addEventListener("click", function (ev9) { ev9.stopPropagation(); removeCampo(campoPill9); });
          pill.addEventListener("dragstart", function (e) { e.dataTransfer.setData("text/plain", "\u0002pill:" + campoPill9); });
          arrastePonteiro(pill, {
            rotulo: function () { return tituloDe(campoPill9); },
            aoSoltar: function (sob) {
              var outroPill9 = sobe(sob, "phx-gpill") || sobe(sob, "phx-gpill-parte");
              if (outroPill9 && Number(outroPill9.getAttribute("data-ix")) !== ixPill9) {
                var paraIx9 = Number(outroPill9.getAttribute("data-ix")), novoR9 = [], j3;
                for (j3 = 0; j3 < grupos.length; j3++) if (j3 !== ixPill9) novoR9.push(grupos[j3]);
                var alvoPos9 = paraIx9 > ixPill9 ? paraIx9 - 1 : paraIx9;
                novoR9.splice(alvoPos9, 0, grupos[ixPill9]);
                api.agrupar(novoR9);
                return;
              }
              if (!sobe(sob, "phx-groupbox")) removeCampo(campoPill9);
            }
          });
          pill.addEventListener("dragover", function (e) { e.preventDefault(); });
          pill.addEventListener("drop", function (e) {
            e.preventDefault(); e.stopPropagation();
            var d2 = e.dataTransfer.getData("text/plain");
            if (d2.indexOf("\u0002pill:") === 0) {
              var deCampo9 = d2.slice(6);
              if (deCampo9 === campoPill9) return;
              var ixOrig9 = -1, j3, el3, k3;
              for (j3 = 0; j3 < grupos.length; j3++) { el3 = grupos[j3]; if (el3 === deCampo9) { ixOrig9 = j3; break; } if (el3 && typeof el3 !== "string") { for (k3 = 0; k3 < el3.length; k3++) if (el3[k3] === deCampo9) { ixOrig9 = j3; break; } if (ixOrig9 >= 0) break; } }
              if (ixOrig9 < 0) return;
              if (ixOrig9 === ixPill9) return;
              var novoD9 = [];
              for (j3 = 0; j3 < grupos.length; j3++) if (j3 !== ixOrig9) novoD9.push(grupos[j3]);
              var posD9 = ixPill9 > ixOrig9 ? ixPill9 - 1 : ixPill9;
              novoD9.splice(posD9, 0, grupos[ixOrig9]);
              api.agrupar(novoD9);
              return;
            }
            /* veio direto de uma coluna do cabecalho (dataTransfer = so o nome do campo, sem prefixo): mescla no pill */
            if (d2 && porCampo[d2] && !agrupada(d2)) {
              var elAlvoD9 = grupos[ixPill9], novoM9 = grupos.slice();
              novoM9[ixPill9] = (elAlvoD9 && typeof elAlvoD9 !== "string") ? elAlvoD9.concat([d2]) : [elAlvoD9, d2];
              api.agrupar(novoM9);
            }
          });
        })(pillsSimples[j2]);
      }
    }
    var barraFiltros = el("div", "phx-filtros");
    barraFiltros.hidden = true;
    wrap.insertBefore(barraFiltros, wrap.firstChild);
    function montaChips() {
      var campos = [], k3, j2;
      for (k3 in filtros) if (filtros[k3]) campos.push(k3);
      campos.sort();
      if (!campos.length) { barraFiltros.hidden = true; barraFiltros.innerHTML = ""; atualizaContaBusca(); return; }
      var html = '<span class="phx-filtros-conta">Filtros Ativos (' + campos.length + ")</span>";
      for (j2 = 0; j2 < campos.length; j2++) {
        html += '<span class="phx-chip" data-campo="' + esc(campos[j2]) + '">' + esc(resumoFiltro(campos[j2], filtros[campos[j2]])) +
          ' <button type="button" class="phx-chip-x" title="remover">\u00d7</button></span>';
      }
      html += '<button type="button" class="phx-filtros-limpar">Limpar Todos</button>';
      barraFiltros.innerHTML = html;
      barraFiltros.hidden = false;
      var xs = barraFiltros.querySelectorAll(".phx-chip-x");
      for (j2 = 0; j2 < xs.length; j2++) {
        (function (btn) {
          btn.addEventListener("click", function () { api.filtrar(btn.parentNode.getAttribute("data-campo"), null); });
        })(xs[j2]);
      }
      barraFiltros.querySelector(".phx-filtros-limpar").addEventListener("click", function () { api.limparFiltros(); });
      atualizaContaBusca();
    }
    var fpop = el("div", "phx-fpop");
    fpop.hidden = true;
    wrap.appendChild(fpop);
    var fpopCampo = null;
    function fechaFpop() { fpop.hidden = true; fpopCampo = null; }
    docOn("mousedown", function (e) {
      if (!fpop.hidden && !fpop.contains(e.target) && !(e.target.className && String(e.target.className).indexOf("phx-fbtn") >= 0)) fechaFpop();
    });
    docOn("keydown", function (e) { if (e.keyCode === 27) fechaFpop(); });
    function abreFiltroExcel(campo, ancora) {
      if (fpopCampo === campo && !fpop.hidden) { fechaFpop(); return; }
      fpopCampo = campo;
      var c2 = porCampo[campo];
      valoresDistintos(campo, function (dist) {
      var atual = filtros[campo];
      var ehNum = c2.tipo === "numero" || c2.tipo === "moeda" || c2.tipo === "percentual";
      var multiIni = (atual && atual.tipo === "multi") ? atual : (atual && atual.tipo === "expr" ? { combinador: "e", condicoes: [{ op: atual.op, valor: atual.valor }] } : null);
      var combIni = multiIni ? multiIni.combinador : "e";
      var OPS_NUM = [[">", "\u00e9 maior que"], [">=", "\u00e9 maior ou igual a"], ["<", "\u00e9 menor que"], ["<=", "\u00e9 menor ou igual a"], ["=", "\u00e9 igual a"], ["!=", "\u00e9 diferente de"]];
      function linhaNum(ix2) {
        var ci = multiIni && multiIni.condicoes[ix2];
        var h = '<div class="phx-fpop-numlin"><select data-nop="' + ix2 + '">', j3;
        for (j3 = 0; j3 < OPS_NUM.length; j3++)
          h += '<option value="' + OPS_NUM[j3][0] + '"' + (ci && ci.op === OPS_NUM[j3][0] ? " selected" : "") + ">" + OPS_NUM[j3][1] + "</option>";
        h += '</select><input type="number" step="any" data-nval="' + ix2 + '"' + (ci ? ' value="' + ci.valor + '"' : "") + "></div>";
        return h;
      }
      var marcados = {}, todosMarcados = !atual || atual.tipo !== "valores", j2;
      if (!todosMarcados) for (j2 = 0; j2 < atual.valores.length; j2++) marcados[String(atual.valores[j2])] = true;
      var incluiNulos = todosMarcados ? true : !!(atual && atual.incluiNulos);
      var busca = "";
      function visListaBusca() {
        var out = [], j3, q = semAcento(busca);
        for (j3 = 0; j3 < dist.itens.length; j3++) {
          if (!q || dist.itens[j3].busca.indexOf(q) >= 0) {
            out.push(dist.itens[j3]);
            if (out.length >= LIMITE_LISTA_EXCEL) break;
          }
        }
        return out;
      }
      function marcado(it) { return todosMarcados ? true : !!marcados[it.chave]; }
      function materializaMarcados() {
        if (!todosMarcados) return;
        todosMarcados = false; marcados = {};
        var j4;
        for (j4 = 0; j4 < dist.itens.length; j4++) marcados[dist.itens[j4].chave] = true;
      }
      function atualizaMestreFpop() {
        var vis = visListaBusca(), nVis = vis.length, nVisMarc = 0, j3;
        for (j3 = 0; j3 < vis.length; j3++) if (marcado(vis[j3])) nVisMarc++;
        var mestre = fpop.querySelector(".phx-fpop-tudo input");
        mestre.checked = nVis > 0 && nVisMarc === nVis;
        mestre.indeterminate = nVisMarc > 0 && nVisMarc < nVis;
      }
      function renderLista() {
        var vis = visListaBusca(), html = "", j3;
        for (j3 = 0; j3 < vis.length; j3++) {
          html += '<label class="phx-fpop-item"><input type="checkbox" data-ch="' + esc(vis[j3].chave) + '"' + (marcado(vis[j3]) ? " checked" : "") + "> " + esc(vis[j3].rotulo) + "</label>";
        }
        fpop.querySelector(".phx-fpop-lista").innerHTML = html;
        atualizaMestreFpop();
        var rodT = fpop.querySelector(".phx-fpop-trunc");
        if (dist.total > vis.length && (busca ? true : dist.total > LIMITE_LISTA_EXCEL)) {
          rodT.textContent = "mostrando " + fmt.numero(vis.length) + " de " + fmt.numero(dist.total) + " \u2014 refine a pesquisa";
          rodT.hidden = false;
        } else rodT.hidden = true;
      }
      fpop.innerHTML =
        '<button type="button" class="phx-fpop-acao" data-a="az">Classificar de A a Z</button>' +
        '<button type="button" class="phx-fpop-acao" data-a="za">Classificar de Z a A</button>' +
        '<button type="button" class="phx-fpop-acao" data-a="limpar">Limpar Filtro</button>' +
        '<div class="phx-fpop-sep"></div>' +
        (dist.remoto ? '<div class="phx-fpop-aviso">fonte remota sem suporte a valores distintos</div>' :
        '<input type="text" class="phx-fpop-busca" placeholder="Pesquisar">' +
        '<label class="phx-fpop-tudo"><input type="checkbox"> (Selecionar Tudo)</label>' +
        '<div class="phx-fpop-lista"></div>' +
        '<div class="phx-fpop-trunc" hidden></div>' +
        (dist.temNulos ? '<label class="phx-fpop-nulos"><input type="checkbox"' + (incluiNulos ? " checked" : "") + "> Exibir itens sem valor</label>" : "")) +
        (ehNum ? '<div class="phx-fpop-sep"></div><div class="phx-fpop-numtit">Filtros de N\u00famero</div>' + linhaNum(0) +
          '<div class="phx-fpop-comb"><label><input type="radio" name="phxcomb" value="e"' + (combIni !== "ou" ? " checked" : "") + '> E</label>' +
          '<label><input type="radio" name="phxcomb" value="ou"' + (combIni === "ou" ? " checked" : "") + '> OU</label></div>' + linhaNum(1) : "") +
        '<div class="phx-fpop-rodape"><button type="button" class="phx-fpop-ok">OK</button><button type="button" class="phx-fpop-cancela">Cancelar</button></div>';
      var rb = ancora.getBoundingClientRect(), rw = wrap.getBoundingClientRect();
      fpop.style.left = Math.max(8, Math.min(rb.left - rw.left, wrap.offsetWidth - 260)) + "px";
      fpop.style.top = (rb.bottom - rw.top + 4) + "px";
      fpop.hidden = false;
      fpop.querySelector('[data-a="az"]').addEventListener("click", function () { fechaFpop(); api.ordenar(campo, "asc"); });
      fpop.querySelector('[data-a="za"]').addEventListener("click", function () { fechaFpop(); api.ordenar(campo, "desc"); });
      fpop.querySelector('[data-a="limpar"]').addEventListener("click", function () { fechaFpop(); api.filtrar(campo, null); });
      fpop.querySelector(".phx-fpop-cancela").addEventListener("click", fechaFpop);
      if (!dist.remoto) {
        fpop.querySelector(".phx-fpop-lista").addEventListener("change", function (e) {
          var cb = e.target;
          if (!cb || !cb.getAttribute || !cb.getAttribute("data-ch")) return;
          materializaMarcados();
          if (cb.checked) marcados[cb.getAttribute("data-ch")] = true;
          else delete marcados[cb.getAttribute("data-ch")];
          atualizaMestreFpop();
        });
        fpop.querySelector(".phx-fpop-busca").addEventListener("input", function () { busca = this.value; renderLista(); });
        fpop.querySelector(".phx-fpop-tudo input").addEventListener("click", function () {
          var alvo2 = this.checked, vis = visListaBusca(), j3;
          materializaMarcados();
          for (j3 = 0; j3 < vis.length; j3++) { if (alvo2) marcados[vis[j3].chave] = true; else delete marcados[vis[j3].chave]; }
          var cbs2 = fpop.querySelectorAll(".phx-fpop-item input");
          for (j3 = 0; j3 < cbs2.length; j3++) cbs2[j3].checked = alvo2;
          atualizaMestreFpop();
        });
        var nl = fpop.querySelector(".phx-fpop-nulos input");
        if (nl) nl.addEventListener("change", function () { incluiNulos = this.checked; });
        fpop.querySelector(".phx-fpop-ok").addEventListener("click", function () {
          if (ehNum) {
            var conds = [], j6, vNum, opSel;
            for (j6 = 0; j6 < 2; j6++) {
              vNum = parseFloat(fpop.querySelector('input[data-nval="' + j6 + '"]').value);
              opSel = fpop.querySelector('select[data-nop="' + j6 + '"]').value;
              if (vNum === vNum) conds.push({ op: opSel, valor: vNum });
            }
            if (conds.length) {
              var comb = fpop.querySelector('input[name="phxcomb"]:checked').value;
              fechaFpop();
              api.filtrar(campo, conds.length === 1 ? { tipo: "expr", op: conds[0].op, valor: conds[0].valor } : { tipo: "multi", combinador: comb, condicoes: conds });
              log("filter.excel", { campo: campo, numero: true, n: conds.length, comb: comb });
              return;
            }
          }
          var sel = [], j3, tot = 0;
          if (todosMarcados) { for (j3 = 0; j3 < dist.itens.length; j3++) sel.push(dist.itens[j3].chave); }
          else { for (var k4 in marcados) if (marcados[k4]) sel.push(k4); }
          tot = sel.length;
          fechaFpop();
          if (tot === dist.total && (incluiNulos || !dist.temNulos)) { api.filtrar(campo, null); }
          else api.filtrar(campo, { tipo: "valores", valores: sel, incluiNulos: incluiNulos && dist.temNulos });
          log("filter.excel", { campo: campo, n: tot, nulos: !!(incluiNulos && dist.temNulos) });
        });
      } else {
        fpop.querySelector(".phx-fpop-ok").addEventListener("click", fechaFpop);
      }
      renderLista._noop = true;
      if (!dist.remoto) renderLista();
      });
    }
    var estiloFixas = document.createElement("style");
    wrap.appendChild(estiloFixas);
    var envoltorio = wrap.querySelector(".phx-envoltorio");
    var roladoFlag = false;
    var _rolCarregando = false, _passoPagina = (cfg.pagina && cfg.pagina.tamanho) || 50;
    envoltorio.addEventListener("scroll", function () {
      if (cfg.rolagemContinua && !_rolCarregando && estado.tamanho < (estado.total || 0) &&
          envoltorio.scrollTop + envoltorio.clientHeight >= envoltorio.scrollHeight - 48) {
        _rolCarregando = true;
        estado.tamanho = Math.min(estado.total, estado.tamanho + _passoPagina);
        estado.pagina = 1;
        log("rolagem-continua", { tamanho: estado.tamanho, total: estado.total });
        carrega(function () { _rolCarregando = false; }, true);
      }
      if (cfg.dicaRolagem && porCampo[cfg.dicaRolagem] && ultimaCarga && ultimaCarga.linhas) {
        var trs9 = tbody.children, alvo9 = null, q9, topo9 = envoltorio.scrollTop;
        for (q9 = 0; q9 < trs9.length; q9++) if (trs9[q9].offsetTop + trs9[q9].offsetHeight > topo9 + 2) { alvo9 = q9; break; }
        if (alvo9 == null && trs9.length && !trs9[trs9.length - 1].offsetHeight) alvo9 = 0; /* sem layout (teste): primeira linha */
        var l9 = alvo9 != null ? ultimaCarga.linhas[alvo9] : null;
        if (l9 && !l9.__grupo) {
          if (!_dicaRol) { _dicaRol = el("div", "phx-dica-rol"); wrap.appendChild(_dicaRol); }
          _dicaRol.textContent = (porCampo[cfg.dicaRolagem].titulo || cfg.dicaRolagem) + ": " + textoVisivel(porCampo[cfg.dicaRolagem], l9[cfg.dicaRolagem]);
          _dicaRol.style.display = "block";
          if (_dicaRolTimer) clearTimeout(_dicaRolTimer);
          _dicaRolTimer = setTimeout(function () { if (_dicaRol) _dicaRol.style.display = "none"; }, 900);
        }
      }
      if (virtColOn) {
        var antesJ = janIni + ":" + janFim;
        calcJanelaCol();
        if (janIni + ":" + janFim !== antesJ) {
          montaHeader();
          reRender();
          log("virt-col", { ini: janIni, fim: janFim });
        }
      }
      var r2 = envoltorio.scrollLeft > 0;
      if (r2 === roladoFlag) return;
      roladoFlag = r2;
      if (r2) wrap.className += " phx-rolado";
      else wrap.className = wrap.className.replace(" phx-rolado", "");
    });
    wrap.addEventListener("paste", function (ev9) {
      if (cfg.clipboard === false) return;
      if (!cfg.edicao && !cfg.lancamento) return;
      var dt9 = ev9.clipboardData || window.clipboardData;
      if (!dt9) return;
      var txt9 = dt9.getData ? dt9.getData("text/plain") || dt9.getData("Text") : null;
      if (!txt9) return;
      ev9.preventDefault();
      var rel9 = api.colarTSV(txt9);
      log("colar", { linhas: rel9.linhas, celulas: rel9.celulas, criadas: rel9.criadas, ignoradas: rel9.ignoradas, erros: rel9.erros.length, origem: "paste" });
    });
    var thead = wrap.querySelector("thead");
    var tabela = wrap.querySelector("table.phx-tabela");
    var tbody = wrap.querySelector("tbody");
    var tfootEl = wrap.querySelector(".phx-tfoot");
    var popover = el("div", "phx-popover");
    popover.hidden = true;
    wrap.appendChild(popover);
    var popoverDe = null;
    function fechaPopover() { popover.hidden = true; popoverDe = null; }
    tbody.addEventListener("click", function (e) {
      var alvoEl = e.target;
      if (temNavegador || temCabLinha) {
        var trR9 = alvoEl.closest ? alvoEl.closest("tr") : null;
        if (trR9 && trR9.parentNode === tbody && trR9.className.indexOf("phx-grupo") < 0 && trR9.className.indexOf("phx-tr-vazia") < 0 && trR9.className.indexOf("phx-detalhe") < 0) {
          var idxR9 = 0, tt9 = trR9.previousElementSibling;
          while (tt9) { if (tt9.className.indexOf("phx-grupo") < 0 && tt9.className.indexOf("phx-detalhe") < 0 && tt9.className.indexOf("phx-tr-vazia") < 0) idxR9++; tt9 = tt9.previousElementSibling; }
          var nR9 = (estado.pagina - 1) * estado.tamanho + idxR9 + 1;
          if (nR9 !== registroAtual) { registroAtual = nR9; if (temCabLinha) reRender(); atualizaNav(); log("registro", { n: registroAtual, total: estado.total }); }
        }
      }
      var chk9 = alvoEl.closest ? alvoEl.closest(".phx-chk") : null;
      if (chk9) {
        var tdC9 = chk9.closest("td"), trC9 = tdC9 && tdC9.parentNode, campoC9 = chk9.getAttribute("data-chk"), colC9 = porCampo[campoC9];
        if (colC9 && colC9.editavel === true && trC9) {
          var ixC9 = 0, tp9 = trC9.previousElementSibling; while (tp9) { ixC9++; tp9 = tp9.previousElementSibling; }
          var lC9 = ultimaCarga && ultimaCarga.linhas[ixC9];
          if (lC9 && !lC9.__grupo) {
            var nv9 = !(lC9[campoC9] === true || lC9[campoC9] === 1 || lC9[campoC9] === "1" || lC9[campoC9] === "S" || lC9[campoC9] === "sim");
            if (!(aoEditar && aoEditar(chaveDe(lC9, ixC9), campoC9, nv9, lC9[campoC9], lC9) === false)) {
              registraHist(chaveDe(lC9, ixC9), campoC9, lC9[campoC9], nv9);
              lC9[campoC9] = nv9; materializaFormulas([lC9]); reRender();
              log("edit", { chave: chaveDe(lC9, ixC9), campo: campoC9, valor: nv9, origem: "checkbox" });
            }
          }
        }
        return;
      }
      var btn9 = alvoEl.closest ? alvoEl.closest(".phx-btn-cel") : null;
      if (btn9) {
        var tdB9 = btn9.closest("td"), trB9 = tdB9 && tdB9.parentNode, ixB9 = 0, tq9 = trB9 && trB9.previousElementSibling;
        while (tq9) { ixB9++; tq9 = tq9.previousElementSibling; }
        var lB9 = ultimaCarga && ultimaCarga.linhas[ixB9], colB9 = porCampo[btn9.getAttribute("data-btn")];
        if (lB9 && colB9) { log("botao", { campo: colB9.campo, chave: chaveDe(lB9, ixB9) }); if (typeof colB9.aoClicar === "function") { try { colB9.aoClicar(chaveDe(lB9, ixB9), lB9); } catch (e9) {} } }
        return;
      }
      if (cfg.selecao === "celula" || cfg.selecao === "coluna") {
        var tdS9 = alvoEl.closest ? alvoEl.closest("td.phx-td") : null;
        if (tdS9 && tdS9.parentNode.className.indexOf("phx-grupo") < 0) {
          var vS9 = visiveis(), ixCS9 = tdS9.cellIndex - (temSelecao ? 1 : 0) - (temDetalhe ? 1 : 0) - (temCabLinha ? 1 : 0);
          if (vS9[ixCS9] && vS9[ixCS9].selecionavel === false) { /* coluna marcada como nao selecionavel: clique nela nao faz nada */ }
          else if (vS9[ixCS9]) {
            var prev9 = wrap.querySelectorAll(".phx-cel-sel, .phx-col-sel"), q9;
            for (q9 = 0; q9 < prev9.length; q9++) prev9[q9].className = prev9[q9].className.replace(/ phx-(cel|col)-sel/g, "");
            if (cfg.selecao === "celula") { tdS9.className += " phx-cel-sel"; celulaSel = { campo: vS9[ixCS9].campo, ixL: tdS9.parentNode.rowIndex, texto: tdS9.textContent }; }
            else { var tds9 = tbody.querySelectorAll("td:nth-child(" + (tdS9.cellIndex + 1) + ")"), r9; for (r9 = 0; r9 < tds9.length; r9++) tds9[r9].className += " phx-col-sel"; celulaSel = { campo: vS9[ixCS9].campo, coluna: true }; }
            log("selecao-celula", { campo: vS9[ixCS9].campo, modo: cfg.selecao });
          }
        }
      }
      var trG = alvoEl.closest ? alvoEl.closest(".phx-grupo") : null;
      if (trG) {
        var pth = trG.getAttribute("data-gpath");
        api.expandirGrupo(pth, !!recolhidos[pth]);
        return;
      }
      var jbtn = alvoEl.className === "phx-json-btn" ? alvoEl : null;
      if (jbtn) {
        if (popoverDe === jbtn) { fechaPopover(); return; }
        var lx = parseInt(jbtn.getAttribute("data-jl"), 10);
        var cp = jbtn.getAttribute("data-jc");
        var valor = ultimaCarga.linhas[lx][cp];
        var texto;
        try { texto = JSON.stringify(valor, null, 2); } catch (e2) { texto = String(valor); }
        popover.innerHTML = "<pre>" + esc(texto) + "</pre>";
        var rb = jbtn.getBoundingClientRect(), rw = wrap.getBoundingClientRect();
        popover.style.left = Math.max(8, rb.left - rw.left - 120) + "px";
        popover.style.top = (rb.bottom - rw.top + 6) + "px";
        popover.hidden = false;
        popoverDe = jbtn;
        log("expandjson", { campo: cp, linha: lx });
        return;
      }
      fechaPopover();
      var tdE = alvoEl.closest ? alvoEl.closest(".phx-td-exp") : null;
      if (tdE && tdE.getAttribute("data-dl") != null) {
        var ixE = parseInt(tdE.getAttribute("data-dl"), 10);
        var kE = chaveDe(ultimaCarga.linhas[ixE], ixE);
        api.expandirDetalhe(kE, !detalhesAbertos[kE]);
        return;
      }
      if (!temSelecao) return;
      var tdSel = alvoEl.closest ? alvoEl.closest(".phx-td-sel") : null;
      if (!tdSel) return;
      var ix = parseInt(tdSel.getAttribute("data-ls"), 10);
      if (e.shiftKey && ancoraSel >= 0) {
        var a2 = Math.min(ancoraSel, ix), b2 = Math.max(ancoraSel, ix), j2;
        for (j2 = a2; j2 <= b2; j2++) alternaLinha(j2, true);
      } else {
        alternaLinha(ix);
        ancoraSel = ix;
      }
      atualizaMestre();
      log("select", { n: nSel });
    });
    var pagEl = wrap.querySelector(".phx-pag");
    var tamSel = wrap.querySelector(".phx-tam");
    var mostrandoEl = wrap.querySelector(".phx-mostrando");
    var colBtn = wrap.querySelector(".phx-colsel-btn");
    var colMenu = wrap.querySelector(".phx-colsel");

    for (i = 0; i < opcoesTam.length; i++) {
      var op = el("option", null, String(opcoesTam[i]));
      op.value = String(opcoesTam[i]);
      if (opcoesTam[i] === estado.tamanho) op.selected = true;
      tamSel.appendChild(op);
    }

    function flattenOrdem() { return ordemColunas.slice(); }
    function visiveis() {
      var v = [], j;
      for (j = 0; j < ordemColunas.length; j++) {
        if (ocultas[ordemColunas[j]] || agrupada(ordemColunas[j])) continue;
        if (regrasAcesso && acessoDe(ordemColunas[j]).oculta) continue;
        if (ocultasAdapt[ordemColunas[j]]) continue;
        v.push(porCampo[ordemColunas[j]]);
      }
      return v;
    }
    var LARG_COL_PADRAO = cfg.larguraPadraoCol || 140;
    var virtColOn = !!cfg.virtualColunas;
    var janIni = -1, janFim = -1, janEspE = 0, janEspD = 0;
    function larguraDe(c9) { return c9.largura || LARG_COL_PADRAO; }
    var autoLarguraModo = cfg.autoLargura === true ? "conteudo" : (cfg.autoLargura || false);
    var larguraFixada = {};
    (function () {
      var j9, c9;
      for (j9 = 0; j9 < colunasDef.length; j9++) {
        c9 = colunasDef[j9];
        /* coluna com largura declarada e sem auto:true nasce FIXA */
        if (c9.auto === false || (c9.largura && c9.auto !== true)) larguraFixada[c9.campo] = true;
      }
    })();
    var _ctxMedida = null, _fonteMedida = null;
    function medidorTexto() {
      if (_ctxMedida === null) {
        var cv9 = document.createElement("canvas");
        _ctxMedida = (cv9 && cv9.getContext) ? (cv9.getContext("2d") || false) : false;
      }
      if (!_fonteMedida) {
        var fs9 = "13px", fam9 = "sans-serif";
        try {
          var cs9 = getComputedStyle(wrap);
          fs9 = (cs9.getPropertyValue("--phx-fs") || "13px").replace(/^\s+|\s+$/g, "") || "13px";
          fam9 = (cs9.getPropertyValue("--phx-fonte") || "sans-serif").replace(/^\s+|\s+$/g, "") || "sans-serif";
        } catch (e9) {}
        _fonteMedida = { fs: parseFloat(fs9) || 13, fam: fam9 };
      }
      return function (txt9, peso9) {
        var t9 = String(txt9 == null ? "" : txt9);
        if (_ctxMedida && _ctxMedida.measureText) {
          _ctxMedida.font = (peso9 ? "700 " : "") + _fonteMedida.fs + "px " + _fonteMedida.fam;
          var m9 = _ctxMedida.measureText(t9);
          if (m9 && m9.width) return m9.width;
        }
        /* sem canvas (WebView antiga ou teste): estimativa por caractere */
        return t9.length * _fonteMedida.fs * 0.56 + (peso9 ? t9.length * 0.6 : 0);
      };
    }
    function textoVisivel(c9, v9) {
      /* mesma fonte de verdade da celula: ReplaceValues + NullBehavior resolvidos em valorExibido() */
      var ve9 = valorExibido(c9, v9);
      if (ve9) return ve9.texto;
      var t9 = c9.tipo || "texto";
      if (v9 == null || v9 === "") return "";
      if (t9 === "moeda") return fmt.moeda(v9);
      if (t9 === "percentual") return fmt.percentual(v9);
      if (t9 === "numero") return fmt.numero(v9, c9.decimais);
      if (t9 === "data") return fmt.data(v9);
      if (t9 === "dataHora") return fmt.dataHora(v9);
      return String(v9);
    }
    function _padHoriz() {
      var px9 = 20;
      try {
        var v9 = getComputedStyle(wrap).getPropertyValue("--phx-pad-x");
        if (v9) px9 = (parseFloat(v9) || 10) * 2;
      } catch (e9) {}
      return px9;
    }
    function calculaLarguras(opts9) {
      opts9 = opts9 || {};
      var cols9 = visiveis(), mede9 = medidorTexto(), pad9 = _padHoriz();
      var amostra9 = [], j9, k9, c9;
      var pag9 = (ultimaCarga && ultimaCarga.linhas) || [];
      for (j9 = 0; j9 < pag9.length; j9++) if (!pag9[j9].__grupo) amostra9.push(pag9[j9]);
      var extra9 = opts9.amostra;
      if (extra9 && fonte.local && fonte.todos) {
        var todos9 = fonte.todos, passo9 = Math.max(1, Math.floor(todos9.length / extra9));
        for (j9 = 0; j9 < todos9.length && amostra9.length < extra9 + pag9.length; j9 += passo9) amostra9.push(todos9[j9]);
      }
      var larg9 = {};
      for (k9 = 0; k9 < cols9.length; k9++) {
        c9 = cols9[k9];
        if (c9.__esp) continue;
        if (larguraFixada[c9.campo] && !opts9.forcar) { larg9[c9.campo] = c9.largura || LARG_COL_PADRAO; continue; }
        var tipoC9 = c9.tipo || "texto";
        var tabular9 = tipoC9 === "numero" || tipoC9 === "moeda" || tipoC9 === "percentual";
        var folga9 = opts9.folga == null ? 4 : opts9.folga;
        /* cabecalho: o proprio DOM sabe quanto o th precisa (titulo + filtro + agregador + ordenacao) */
        var w9 = mede9(c9.titulo || c9.campo, true) + 26;
        if (c9.agregador) w9 += 30;
        /* piso do cabecalho: soma dos filhos (titulo, filtro, agregador), que nao esticam com o th
           - medir o proprio th realimentaria o crescimento a cada chamada */
        var thVivo9 = thead ? thead.querySelector('th[data-campo="' + c9.campo + '"]') : null;
        if (thVivo9) {
          /* piso = texto do titulo MEDIDO (o scrollWidth vem truncado quando a coluna esta estreita)
             + os controles do cabecalho, que nao encolhem */
          var piso9 = pad9 + 10 + (c9.fixa ? 16 : 0) + mede9(c9.titulo || c9.campo, true), f9, filhos9 = thVivo9.children, i8;
          var tit9 = thVivo9.querySelector(".phx-th-titulo");
          for (i8 = 0; i8 < filhos9.length; i8++) {
            f9 = filhos9[i8];
            if (f9 === tit9 || f9.className.indexOf("phx-col-resz") >= 0) continue;
            piso9 += f9.offsetWidth || 0;
          }
          if (piso9 > w9) w9 = piso9;
        }
        for (j9 = 0; j9 < amostra9.length; j9++) {
          var val9 = (c9.acumulado && amostra9[j9].__acum) ? amostra9[j9].__acum[c9.campo] : amostra9[j9][c9.campo];
          if (regrasAcesso) val9 = aplicaMascara(c9, val9, amostra9[j9]);
          var txt9 = textoVisivel(c9, val9);
          if (c9.tipo === "badge") txt9 = String(txt9) + "    ";
          var m9 = mede9(txt9, false);
          if (tabular9) m9 *= 1.08;
          m9 += folga9;
          if (m9 > w9 - pad9) w9 = m9 + pad9;
        }
        if (c9.agregador && ultimaCarga && ultimaCarga.totais && ultimaCarga.totais[c9.campo] != null) {
          var tot9 = mede9(textoVisivel(c9, ultimaCarga.totais[c9.campo]), true) * (tabular9 ? 1.08 : 1) + pad9 + 34;
          if (tot9 > w9) w9 = tot9;
        }
        if (k9 === 0 && cfg.totais) {
          var canto9 = mede9(fmt.numero(estado.total || 0) + " " + T("linhas"), true) + pad9 + 8;
          if (canto9 > w9) w9 = canto9;
        }
        var min9 = c9.larguraMin || opts9.min || 56;
        var max9 = c9.larguraMax || opts9.max || 420;
        larg9[c9.campo] = Math.round(Math.max(min9, Math.min(max9, w9)));
      }
      return larg9;
    }
    function distribuiPreencher(larg9) {
      var cols9 = visiveis(), j9, c9, soma9 = 0, elast9 = [], pesoTot9 = 0;
      for (j9 = 0; j9 < cols9.length; j9++) {
        c9 = cols9[j9];
        if (c9.__esp) continue;
        soma9 += larg9[c9.campo] || larguraDe(c9);
        if (c9.fixarLargura !== true && !larguraFixada[c9.campo]) { elast9.push(c9); pesoTot9 += (c9.flex || 1); }
      }
      var extras9 = (temSelecao ? 40 : 0) + (temDetalhe ? 36 : 0);
      var disp9 = (envoltorio.clientWidth || 0) - extras9 - 2;
      if (disp9 <= 0 || !elast9.length) return { larguras: larg9, sobra: 0, coube: false };
      var sobra9 = disp9 - soma9;
      if (sobra9 <= 0) return { larguras: larg9, sobra: sobra9, coube: false };
      var usado9 = 0;
      for (j9 = 0; j9 < elast9.length; j9++) {
        c9 = elast9[j9];
        var parte9 = j9 === elast9.length - 1 ? (sobra9 - usado9) : Math.floor(sobra9 * (c9.flex || 1) / pesoTot9);
        larg9[c9.campo] = (larg9[c9.campo] || larguraDe(c9)) + parte9;
        usado9 += parte9;
      }
      return { larguras: larg9, sobra: sobra9, coube: true };
    }
    function aplicaLarguras(larg9, modo9) {
      var k9, n9 = 0, soma9 = 0;
      for (k9 in larg9) {
        if (!porCampo[k9]) continue;
        porCampo[k9].largura = larg9[k9];
        soma9 += larg9[k9];
        n9++;
      }
      if (tabela && modo9) {
        /* as larguras passam a mandar: a celula corta com elipse em vez de esticar a coluna
           (e o que faz larguraMax valer de verdade) */
        if (tabela.className.indexOf("phx-larg-auto") < 0) tabela.className += " phx-larg-auto";
        tabela.style.tableLayout = "fixed";
        if (modo9 === "conteudo") tabela.style.width = (soma9 + (temSelecao ? 40 : 0) + (temDetalhe ? 36 : 0)) + "px";
        else if (modo9 === "preencher") tabela.style.width = "100%";
      }
      invalidaJanela();
      montaHeader();
      reRender();
      mideFixas();
      agendaLayoutCb();
      return n9;
    }
    function calcJanelaCol() {
      var todas = visiveis(), esq = [], meio = [], dir = [], j9;
      for (j9 = 0; j9 < todas.length; j9++) {
        if (todas[j9].fixa === "esq") esq.push(todas[j9]);
        else if (todas[j9].fixa === "dir") dir.push(todas[j9]);
        else meio.push(todas[j9]);
      }
      var largEsq = 0, largDir = 0;
      for (j9 = 0; j9 < esq.length; j9++) largEsq += larguraDe(esq[j9]);
      for (j9 = 0; j9 < dir.length; j9++) largDir += larguraDe(dir[j9]);
      var vw = envoltorio.clientWidth || 1000;
      var sl = envoltorio.scrollLeft || 0;
      var visIni = sl + largEsq, visFim = sl + vw - largDir;
      var x = largEsq, ini = -1, fim = -1, w9;
      for (j9 = 0; j9 < meio.length; j9++) {
        w9 = larguraDe(meio[j9]);
        if (x + w9 > visIni && x < visFim) { if (ini < 0) ini = j9; fim = j9; }
        x += w9;
      }
      if (ini < 0) { ini = 0; fim = Math.min(meio.length - 1, 0); }
      var OVER = 2;
      ini = Math.max(0, ini - OVER);
      fim = Math.min(meio.length - 1, fim + OVER);
      var espE = 0, espD = 0;
      for (j9 = 0; j9 < ini; j9++) espE += larguraDe(meio[j9]);
      for (j9 = fim + 1; j9 < meio.length; j9++) espD += larguraDe(meio[j9]);
      janIni = ini; janFim = fim; janEspE = espE; janEspD = espD;
      return { esq: esq, meio: meio, dir: dir, ini: ini, fim: fim, espE: espE, espD: espD };
    }
    var janMemo = null, janAssin = "";
    function assinaturaJanela() {
      return (envoltorio.scrollLeft || 0) + "|" + (envoltorio.clientWidth || 0) + "|" + ordemColunas.length + "|" + janSelo;
    }
    var janSelo = 0;
    function invalidaJanela() { janSelo++; janMemo = null; }
    function visiveisJanela() {
      if (!virtColOn) return visiveis();
      var as9 = assinaturaJanela();
      if (janMemo && as9 === janAssin) return janMemo;
      var J = calcJanelaCol(), out = J.esq.slice(), j9;
      if (J.espE > 0) out.push({ __esp: "E", campo: "__espE", largura: J.espE });
      for (j9 = J.ini; j9 <= J.fim; j9++) out.push(J.meio[j9]);
      if (J.espD > 0) out.push({ __esp: "D", campo: "__espD", largura: J.espD });
      for (j9 = 0; j9 < J.dir.length; j9++) out.push(J.dir[j9]);
      janMemo = out; janAssin = as9;
      return out;
    }
    var CICLO_AGG = ["sum", "avg", "count", "min", "max"];
    function thColuna(c, rowspan) {
      if (c.__esp) {
        var thE = el("th", "phx-th phx-esp-col", "");
        thE.style.width = c.largura + "px";
        thE.style.minWidth = c.largura + "px";
        thE.style.padding = "0";
        return thE;
      }
      var th = el("th", "phx-th" + (c.estiloCabecalho ? " phx-est-" + c.estiloCabecalho : ""));
      if (c.dicaCabecalho) th.title = c.dicaCabecalho;
      th.setAttribute("data-campo", c.campo);
      if (c.tag != null && typeof c.tag !== "object") th.setAttribute("data-tag", String(c.tag));
      if (rowspan > 1) th.rowSpan = rowspan;
      if (c.largura) th.style.width = c.largura + "px";
      if (c.fixa === "esq") th.className += " phx-fixa-esq";
      if (c.fixa === "dir") th.className += " phx-fixa-dir";
      var pino9 = c.fixa ? '<span class="phx-pino" title="coluna ' + (c.fixa === "dir" ? "fixada \u00e0 direita" : "fixada \u00e0 esquerda") + '">\uD83D\uDCCC</span>' : "";
      var posO = posNaCascata(c.campo);
      var ind = posO >= 0
        ? (estado.ordens[posO].dir === "asc" ? " \u25b2" : " \u25bc") + (estado.ordens.length > 1 ? '<span class="phx-sort-n">' + (posO + 1) + "</span>" : "")
        : "";
      var iconeCab9 = c.iconeCabecalho ? '<img class="phx-th-icone" src="' + esc(c.iconeCabecalho) + '" alt="">' : "";
      th.innerHTML =
        '<span class="phx-th-titulo">' + pino9 + iconeCab9 + esc(c.titulo || c.campo) + '<span class="phx-sort-ind">' + ind + "</span></span>" +
        (c.dimensao ? '<span class="phx-th-dim">(' + esc(c.dimensao) + ")</span>" : "") +
        (c.agregador ? '<button type="button" class="phx-th-agg" title="alternar agregador">' + esc(c.agregador.toUpperCase()) + "</button>" : "") +
        (c.filtravel !== false ? '<button type="button" class="phx-fbtn' + (filtros[c.campo] ? " phx-fbtn-on" : "") + '" title="filtrar">\u25bc</button>' : "") +
        '<span class="phx-col-resz"></span>';
      if (c.ordenavel !== false) {
        th.className += " phx-ordenavel";
        th.querySelector(".phx-th-titulo").addEventListener("click", function (ev9) { alternaOrdem(c.campo, !!(ev9 && (ev9.shiftKey || ev9.ctrlKey || ev9.metaKey))); });
      }
      var fbtn = th.querySelector(".phx-fbtn");
      if (fbtn) fbtn.addEventListener("click", function (e) { e.stopPropagation(); abreFiltroExcel(c.campo, th); });
      var aggBtn = th.querySelector(".phx-th-agg");
      if (aggBtn) aggBtn.addEventListener("click", function (e) {
        e.stopPropagation();
        var ix2 = CICLO_AGG.indexOf(c.agregador);
        c.agregador = CICLO_AGG[(ix2 + 1) % CICLO_AGG.length];
        aggBtn.textContent = c.agregador.toUpperCase();
        log("aggchange", { campo: c.campo, agregador: c.agregador });
        carrega(null, true);
        agendaLayoutCb();
      });
      th.draggable = true;
      th.addEventListener("dragstart", function (e) { e.dataTransfer.setData("text/plain", c.campo); });
      arrastePonteiro(th, {
        rotulo: function () { return c.titulo || c.campo; },
        aoMover: function (sob) {
          var gb9 = sobe(sob, "phx-groupbox");
          if (groupBox) groupBox.className = gb9 ? "phx-groupbox phx-groupbox-sobre" : "phx-groupbox";
        },
        aoSoltar: function (sob) {
          if (groupBox) groupBox.className = "phx-groupbox";
          if (sobe(sob, "phx-groupbox")) {
            if (agrupada(c.campo)) return;
            var pillAlvo9 = sobe(sob, "phx-gpill") || sobe(sob, "phx-gpill-parte");
            if (pillAlvo9) {
              var ixAlvo9 = Number(pillAlvo9.getAttribute("data-ix")), elAlvo9 = grupos[ixAlvo9];
              var novo9 = grupos.slice();
              novo9[ixAlvo9] = (elAlvo9 && typeof elAlvo9 !== "string") ? elAlvo9.concat([c.campo]) : [elAlvo9, c.campo];
              api.agrupar(novo9);
            } else api.agrupar(grupos.concat([c.campo]));
            return;
          }
          var thAlvo9 = sobe(sob, "phx-th");
          if (thAlvo9 && thAlvo9 !== th) { var alvo9 = thAlvo9.getAttribute("data-campo"); if (alvo9 && alvo9 !== c.campo) moverColunaAntes(c.campo, alvo9); }
        }
      });
      th.addEventListener("dragover", function (e) { e.preventDefault(); });
      th.addEventListener("drop", function (e) {
        e.preventDefault();
        var de = e.dataTransfer.getData("text/plain");
        if (de && de !== c.campo) moverColunaAntes(de, c.campo);
      });
      var rz = th.querySelector(".phx-col-resz");
      if (rz) rz.addEventListener("dblclick", function (ev9) {
        ev9.stopPropagation();
        ev9.preventDefault();
        api.ajustarColuna(c.campo);
      });
      rz.addEventListener("mousedown", function (e) {
        e.preventDefault(); e.stopPropagation();
        var x0 = e.clientX, w0 = th.offsetWidth;
        function mv(ev2) { var w = Math.max(50, w0 + ev2.clientX - x0); th.style.width = w + "px"; porCampo[c.campo].largura = w; larguraFixada[c.campo] = true; }
        function up() { document.removeEventListener("mousemove", mv); document.removeEventListener("mouseup", up); log("resize", { campo: c.campo, largura: porCampo[c.campo].largura }); invalidaJanela(); mideFixas(); agendaLayoutCb(); }
        rz.addEventListener("dblclick", function (ev9) { ev9.stopPropagation(); ev9.preventDefault(); api.ajustarColuna(c.campo); });
        document.addEventListener("mousemove", mv);
        document.addEventListener("mouseup", up);
      });
      return th;
    }
    function ariaHeader() {
      var ths = tabela.querySelectorAll("thead th"), j2;
      var trsH = tabela.querySelectorAll("thead tr");
      for (j2 = 0; j2 < trsH.length; j2++) { trsH[j2].setAttribute("role", "row"); trsH[j2].setAttribute("aria-rowindex", String(j2 + 1)); }
      var ci = 1;
      var ultima = trsH.length ? trsH[trsH.length - 1] : null;
      for (j2 = 0; j2 < ths.length; j2++) {
        ths[j2].setAttribute("role", "columnheader");
        var campo9 = ths[j2].getAttribute("data-campo");
        if (campo9) {
          ths[j2].setAttribute("aria-sort", estado.ordem.campo === campo9 ? (estado.ordem.dir === "desc" ? "descending" : "ascending") : "none");
        }
      }
      if (ultima) {
        var celsU = ultima.children;
        for (j2 = 0; j2 < celsU.length; j2++) celsU[j2].setAttribute("aria-colindex", String(j2 + 1));
      }
      tabela.setAttribute("aria-colcount", String(colCount()));
    }
    function ariaCorpo() {
      var trs = tbody.children, j2, k2, dataN = 0;
      var basePag = ehVirtual ? vIni : (estado.pagina - 1) * estado.tamanho;
      var headerRows = tabela.querySelectorAll("thead tr").length;
      for (j2 = 0; j2 < trs.length; j2++) {
        var tr2 = trs[j2], cl2 = tr2.className;
        if (cl2.indexOf("phx-vspacer") >= 0) continue;
        tr2.setAttribute("role", "row");
        if (cl2.indexOf("phx-grupo") >= 0) {
          var pathG = tr2.getAttribute("data-gpath");
          tr2.setAttribute("aria-expanded", recolhidos[pathG] ? "false" : "true");
          continue;
        }
        if (cl2.indexOf("phx-detalhe") >= 0) continue;
        tr2.setAttribute("aria-rowindex", String(basePag + dataN + headerRows + 1));
        var kids2 = tr2.children;
        for (k2 = 0; k2 < kids2.length; k2++) {
          kids2[k2].setAttribute("role", "gridcell");
          kids2[k2].setAttribute("aria-colindex", String(k2 + 1));
          if (!kids2[k2].hasAttribute("tabindex")) kids2[k2].setAttribute("tabindex", "-1");
        }
        dataN++;
      }
      tabela.setAttribute("aria-rowcount", String((ehVirtual && cacheV ? cacheV.length : estado.total) + headerRows));
      /* selecionadas */
      var trsD2 = trsDados();
      for (j2 = 0; j2 < trsD2.length; j2++) {
        var cbx = trsD2[j2].querySelector('td.phx-td-sel input');
        if (cbx) trsD2[j2].setAttribute("aria-selected", cbx.checked ? "true" : "false");
      }
      aplicaFoco(false);
    }
    function montaHeader() {
      if (virtColOn) {
        var vJ9 = visiveisJanela(), tr9 = el("tr"), j9;
        if (temCabLinha) tr9.appendChild(el("th", "phx-th phx-td-ind", ""));
        if (temSelecao) tr9.appendChild(el("th", "phx-th phx-td-sel", ""));
        if (temDetalhe) tr9.appendChild(el("th", "phx-th phx-td-exp", ""));
        for (j9 = 0; j9 < vJ9.length; j9++) tr9.appendChild(thColuna(vJ9[j9], 1));
        thead.innerHTML = "";
        thead.appendChild(tr9);
        if (cfg.filterRow) thead.appendChild(montaFilterRow());
        mideFixas();
        return;
      }
      var D = profundidade(arvore) - 1;
      var linhas2 = [], d2;
      for (d2 = 0; d2 <= D; d2++) linhas2.push(el("tr"));
      function anda(no2, nivel) {
        var j2, filho, vis;
        for (j2 = 0; j2 < no2.filhos.length; j2++) {
          filho = no2.filhos[j2];
          if (filho.campo) {
            if (!ocultas[filho.campo] && !agrupada(filho.campo) && !(regrasAcesso && acessoDe(filho.campo).oculta)) linhas2[nivel].appendChild(thColuna(porCampo[filho.campo], D - nivel + 1));
          } else {
            vis = contaVisiveis(filho);
            if (!vis) continue;
            var thB = el("th", "phx-th phx-banda", '<span class="phx-banda-titulo">' + esc(filho.titulo) + "</span>");
            thB.colSpan = vis;
            linhas2[nivel].appendChild(thB);
            anda(filho, nivel + 1);
          }
        }
      }
      if (temCabLinha) { var thI = el("th", "phx-th phx-td-ind"); thI.rowSpan = D + 1; linhas2[0].appendChild(thI); }
      if (temDetalhe) {
        var thE = el("th", "phx-th phx-td-exp");
        thE.rowSpan = D + 1;
        linhas2[0].appendChild(thE);
      }
      if (temSelecao) {
        var thS = el("th", "phx-th phx-td-sel");
        thS.rowSpan = D + 1;
        thS.innerHTML = '<input type="checkbox" class="phx-sel-mestre" tabindex="-1">';
        thS.querySelector("input").addEventListener("click", function () {
          var linhas = ultimaCarga ? ultimaCarga.linhas : [], j2;
          var marcar = this.checked;
          for (j2 = 0; j2 < linhas.length; j2++) alternaLinha(j2, marcar);
          atualizaMestre();
          log("select", { n: nSel, todos: marcar });
        });
        linhas2[0].appendChild(thS);
      }
      anda(arvore, 0);
      thead.innerHTML = "";
      for (d2 = 0; d2 <= D; d2++) if (linhas2[d2].children.length) thead.appendChild(linhas2[d2]);
      if (cfg.filterRow) thead.appendChild(montaFilterRow());
      mideFixas();
      ariaHeader();
    }
    var DEBOUNCE_MS = cfg.debounceMs != null ? cfg.debounceMs : 300;
    function debounce(fn) {
      var t2 = null;
      return function () {
        var args = arguments, self2 = this;
        if (t2) clearTimeout(t2);
        t2 = setTimeout(function () { t2 = null; fn.apply(self2, args); }, DEBOUNCE_MS);
      };
    }
    function montaFilterRow() {
      var tr = el("tr", "phx-frow"), v = visiveisJanela(), j2, c2, td2, tipoC;
      if (temSelecao) tr.appendChild(el("th", "phx-th phx-frow-cel phx-td-sel", ""));
      for (j2 = 0; j2 < v.length; j2++) {
        c2 = v[j2];
        if (c2.__esp) {
          var tdE2 = el("th", "phx-th phx-frow-cel phx-esp-col", "");
          tdE2.style.width = c2.largura + "px";
          tr.appendChild(tdE2);
          continue;
        }
        tipoC = c2.tipo || "texto";
        td2 = el("th", "phx-th phx-frow-cel");
        td2.setAttribute("data-campo", c2.campo);
        if (c2.filtravel === false || tipoC === "json" || tipoC === "barra") {
          tr.appendChild(td2);
          continue;
        }
        if (tipoC === "numero" || tipoC === "moeda" || tipoC === "percentual") {
          td2.innerHTML = '<span class="phx-frow-num"><select class="phx-frow-op"><option>&gt;</option><option>&gt;=</option><option>&lt;</option><option>&lt;=</option><option>=</option><option>!=</option></select><input type="number" step="any" class="phx-frow-in" placeholder="valor"></span>';
          (function (campo, cel) {
            var aplica = debounce(function () {
              var vNum = parseFloat(cel.querySelector(".phx-frow-in").value);
              api.filtrar(campo, vNum === vNum ? { tipo: "expr", op: cel.querySelector(".phx-frow-op").value, valor: vNum } : null);
            });
            cel.querySelector(".phx-frow-in").addEventListener("input", aplica);
            cel.querySelector(".phx-frow-op").addEventListener("change", aplica);
          })(c2.campo, td2);
        } else if (tipoC === "data" || tipoC === "dataHora") {
          td2.innerHTML = '<input type="date" class="phx-frow-in">';
          (function (campo, cel) {
            cel.querySelector("input").addEventListener("change", function () {
              var vd = this.value;
              if (!vd) { api.filtrar(campo, null); return; }
              api.filtrar(campo, { tipo: "faixa", de: vd + "T00:00:00", ate: vd + "T23:59:59.999" });
            });
          })(c2.campo, td2);
        } else if (tipoC === "badge") {
          (function (campo, cel) {
            valoresDistintos(campo, function (dist2) {
              var hOp = '<select class="phx-frow-in phx-frow-sel"><option value="">Selecionar</option>', j3;
              for (j3 = 0; j3 < dist2.itens.length && j3 < 50; j3++) hOp += "<option>" + esc(dist2.itens[j3].chave) + "</option>";
              cel.innerHTML = hOp + "</select>";
              cel.querySelector("select").addEventListener("change", function () {
                api.filtrar(campo, this.value ? { tipo: "valores", valores: [this.value] } : null);
              });
            });
          })(c2.campo, td2);
        } else {
          td2.innerHTML = '<input type="text" class="phx-frow-in" placeholder="Buscar\u2026">';
          (function (campo, cel) {
            cel.querySelector("input").addEventListener("input", debounce(function () {
              var vt = cel.querySelector("input").value;
              api.filtrar(campo, vt ? { tipo: "texto", contem: vt } : null);
            }));
          })(c2.campo, td2);
        }
        tr.appendChild(td2);
      }
      return tr;
    }
    function limpaControleRow(campo) {
      if (campo === "*") { if (buscaEl) buscaEl.value = ""; return; }
      if (!cfg.filterRow) return;
      var cel = thead.querySelector('.phx-frow-cel[data-campo="' + campo + '"]');
      if (!cel) return;
      var inp = cel.querySelector(".phx-frow-in");
      if (inp) inp.value = "";
    }
    function mideFixas() {
      var v = visiveis(), esq = 0, dirTot = 0, j2, c2, regras = [], ths = {};
      var lista = thead.querySelectorAll("th[data-campo]");
      for (j2 = 0; j2 < lista.length; j2++) ths[lista[j2].getAttribute("data-campo")] = lista[j2];
      for (j2 = 0; j2 < v.length; j2++) {
        c2 = v[j2];
        if (c2.fixa === "esq") {
          regras.push('.phx-grid [data-campo="' + c2.campo + '"],.phx-grid [data-fx="' + c2.campo + '"]{position:sticky;position:-webkit-sticky;left:' + esq + "px;z-index:5}");
          esq += (ths[c2.campo] && ths[c2.campo].offsetWidth) || c2.largura || 0;
        }
      }
      for (j2 = v.length - 1; j2 >= 0; j2--) {
        c2 = v[j2];
        if (c2.fixa === "dir") {
          regras.push('.phx-grid [data-campo="' + c2.campo + '"],.phx-grid [data-fx="' + c2.campo + '"]{position:sticky;position:-webkit-sticky;right:' + dirTot + "px;z-index:5}");
          dirTot += (ths[c2.campo] && ths[c2.campo].offsetWidth) || c2.largura || 0;
        }
      }
      estiloFixas.textContent = regras.join("\n");
    }

    function sincronizaOrdemPrimaria() {
      estado.ordem = estado.ordens.length
        ? { campo: estado.ordens[0].campo, dir: estado.ordens[0].dir }
        : { campo: null, dir: null };
    }
    function posNaCascata(campo) {
      var j9;
      for (j9 = 0; j9 < estado.ordens.length; j9++) if (estado.ordens[j9].campo === campo) return j9;
      return -1;
    }
    function alternaOrdem(campo, somar) {
      var t1 = agora();
      var at = posNaCascata(campo);
      if (!somar) {
        if (at === 0 && estado.ordens.length === 1) {
          if (estado.ordens[0].dir === "asc") estado.ordens[0].dir = "desc";
          else estado.ordens = [];
        } else estado.ordens = [{ campo: campo, dir: "asc" }];
      } else {
        if (at < 0) estado.ordens.push({ campo: campo, dir: "asc" });
        else if (estado.ordens[at].dir === "asc") estado.ordens[at].dir = "desc";
        else estado.ordens.splice(at, 1);
      }
      sincronizaOrdemPrimaria();
      estado.pagina = 1;
      agendaLayoutCb();
      carrega(function () {
        montaHeader();
        log("sort", { campo: campo, dir: estado.ordem.dir, niveis: estado.ordens.length, multi: estado.ordens.length > 1, ms: Math.round((agora() - t1) * 10) / 10 });
      });
    }
    function moverColunaAntes(campoMovido, campoAlvo) {
      var pa = paiDe(arvore, campoMovido), pb = paiDe(arvore, campoAlvo);
      if (!pa || !pb) return;
      if (pa !== pb) { log("reorder-negado", { campo: campoMovido, motivo: "bandas diferentes" }); return; }
      var a = -1, b = -1, j2;
      for (j2 = 0; j2 < pa.filhos.length; j2++) {
        if (pa.filhos[j2].campo === campoMovido) a = j2;
        if (pa.filhos[j2].campo === campoAlvo) b = j2;
      }
      if (a < 0 || b < 0) return;
      var no2 = pa.filhos.splice(a, 1)[0];
      for (j2 = 0, b = -1; j2 < pa.filhos.length; j2++) if (pa.filhos[j2].campo === campoAlvo) { b = j2; break; }
      pa.filhos.splice(b, 0, no2);
      ordemColunas = flatten(arvore, []);
      log("reorder", { campo: campoMovido, antesDe: campoAlvo });
      montaHeader(); reRender();
      montaColSel();
    }

    var ultimaCarga = null;
    var perfRender = {};
    function carregaVirtual(cb) {
      var c0 = porCampo[estado.ordem.campo];
      fonte.carregar({
        pagina: 1, tamanho: 1e9,
        ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c0 ? c0.tipo : null },
        filtros: serializaFiltros(),
        grupos: [], recolhidos: {}, aggCols: [],
        totais: temTotais ? (function () { var o3 = [], j3; for (j3 = 0; j3 < colunasDef.length; j3++) if (colunasDef[j3].agregador) o3.push({ campo: colunasDef[j3].campo, agregador: colunasDef[j3].agregador }); return o3; })() : null,
        tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
      }, function (err, resp) {
        if (err) return;
        cacheV = resp.linhas || [];
        ultimaCarga = { linhas: cacheV, total: cacheV.length, totais: resp.totais, _plano: resp._plano, _ms: resp._ms };
        estado.total = cacheV.length;
        envoltorio.scrollTop = 0;
        renderJanela(true);
        montaTfoot();
        log("virtual", { total: cacheV.length, ms: resp._ms });
        if (cb) cb();
      });
    }
    function calcJanela() {
      var st2 = envoltorio.scrollTop || 0;
      var ini2 = Math.max(0, Math.floor(st2 / vLinha) - vOver);
      var fim2 = Math.min(cacheV.length, ini2 + vNVis + vOver * 2);
      return { ini: ini2, fim: fim2 };
    }
    function renderJanela(forca) {
      if (!cacheV) return;
      var w2 = calcJanela();
      if (!forca && w2.ini === vIni && w2.fim === vFim) return;
      vIni = w2.ini; vFim = w2.fim;
      var v = visiveisJanela(), html = "", j, k, c, val, ch;
      var nCols = v.length + (temSelecao ? 1 : 0) + (temDetalhe ? 1 : 0) + (temCabLinha ? 1 : 0);
      html += '<tr class="phx-vspacer"><td colspan="' + nCols + '" style="height:' + (vIni * vLinha) + 'px;padding:0;border:0"></td></tr>';
      for (j = vIni; j < vFim; j++) {
        var linha = cacheV[j];
        var chN = temNotas ? chaveDe(linha, j) : null;
        html += '<tr style="height:' + vLinha + 'px">';
        if (temSelecao) {
          ch = selecionadas[chaveDe(linha, j)] ? " checked" : "";
          html += '<td class="phx-td phx-td-sel" data-ls="' + j + '"><input type="checkbox" tabindex="-1"' + ch + "></td>";
        }
        for (k = 0; k < v.length; k++) {
          c = v[k];
          if (c.__esp) { html += '<td class="phx-esp-col" style="width:' + c.largura + "px;min-width:" + c.largura + 'px;padding:0"></td>'; continue; }
          val = linha[c.campo];
          if (regrasAcesso) val = aplicaMascara(c, val, linha);
          var notaTd = temNotas && notas[chN] && notas[chN][c.campo];
          html += '<td class="phx-td phx-tipo-' + (c.tipo || "texto") + (notaTd ? " phx-td-nota" : "") + '"' +
            (c.fixa ? ' data-fx="' + esc(c.campo) + '"' : "") + ">" + formata(c, val, linha, j) +
            (notaTd ? '<span class="phx-nota-ind" data-nc="' + esc(c.campo) + '" data-nl="' + j + '" title="ver nota"></span>' : "") + "</td>";
        }
        html += "</tr>";
      }
      html += '<tr class="phx-vspacer"><td colspan="' + nCols + '" style="height:' + ((cacheV.length - vFim) * vLinha) + 'px;padding:0;border:0"></td></tr>';
      tbody.innerHTML = html;
      atualizaMestre();
      ariaCorpo();
      if (mostrandoEl) mostrandoEl.textContent = "Linhas " + fmt.numero(vIni + 1) + "\u2013" + fmt.numero(vFim) + " de " + fmt.numero(cacheV.length) + " (virtual)";
    }
    function aoScrollV() {
      if (vRaf) return;
      vRaf = (window.requestAnimationFrame || function (f) { return setTimeout(f, 16); })(function () {
        vRaf = null;
        renderJanela(false);
      });
    }
    function reRender() {
      if (ehVirtual) { renderJanela(true); return; }
      montaCorpo(ultimaCarga ? ultimaCarga.linhas : []);
    }
    function totaisSelecao() {
      var cols = [], j3;
      for (j3 = 0; j3 < colunasDef.length; j3++) if (colunasDef[j3].agregador) cols.push(colunasDef[j3]);
      var out = {}, k3, lista = [];
      for (k3 in dadosSel) lista.push(dadosSel[k3]);
      for (j3 = 0; j3 < cols.length; j3++) {
        var vals = [], i3;
        for (i3 = 0; i3 < lista.length; i3++) vals.push(lista[i3][cols[j3].campo]);
        out[cols[j3].campo] = AGGS[cols[j3].agregador](vals);
      }
      return out;
    }
    function montaTfoot() {
      if (!temTotais) return;
      var t1 = agora();
      var modo = nSel > 0 ? "selecao" : "filtrados";
      var tot = modo === "selecao" ? totaisSelecao() : (ultimaCarga && ultimaCarga.totais) || null;
      var v = visiveisJanela(), html = '<tr class="' + (modo === "selecao" ? "phx-tfoot-sel" : "") + '">', j3;
      var canto = modo === "selecao"
        ? fmt.numero(nSel) + " selecionada" + (nSel > 1 ? "s" : "")
        : fmt.numero((ultimaCarga && (ultimaCarga.totalDados != null ? ultimaCarga.totalDados : ultimaCarga.total)) || 0) + " " + T("linhas");
      var extra = (temDetalhe ? 1 : 0) + (temSelecao ? 1 : 0) + (temCabLinha ? 1 : 0);
      html += '<td class="phx-td phx-tfoot-canto" colspan="' + (extra + 1) + '">' + canto + "</td>";
      for (j3 = 1; j3 < v.length; j3++) {
        var c3 = v[j3];
        if (c3.__esp) { html += '<td class="phx-esp-col" style="width:' + c3.largura + 'px;padding:0"></td>'; continue; }
        if (c3.agregador && tot && tot[c3.campo] != null)
          html += '<td class="phx-td phx-tipo-numero"><span class="phx-tfoot-agg">' + esc(c3.agregador.toUpperCase()) + "</span> " + formata(colFormato({ tipo: c3.tipo || "numero", decimais: c3.decimais, formatoTotal: c3.formatoTotal }, "formatoTotal"), tot[c3.campo], {}, 0) + "</td>";
        else if (c3.agregador)
          html += '<td class="phx-td phx-tipo-numero">\u2014</td>';
        else html += '<td class="phx-td"></td>';
      }
      tfootEl.innerHTML = html + "</tr>";
      log("summary", { modo: modo, ms: Math.round((agora() - t1) * 100) / 100 });
    }
    function montaCorpo(linhas) {
      encerraEditorSilencioso();
      var tA = agora();
      var v = visiveisJanela(), html = "", j, k, c, val, ch, nCols = v.length + (temSelecao ? 1 : 0) + (temDetalhe ? 1 : 0);
      var colPreview9 = null, pj9;
      for (pj9 = 0; pj9 < colunasDef.length; pj9++) if (colunasDef[pj9].preview) { colPreview9 = colunasDef[pj9]; break; }
      var pilhaG = [], _contDados = 0, _mapaDados = {};
      for (var jq = 0; jq < linhas.length; jq++) if (!linhas[jq].__grupo) { _mapaDados[jq] = _contDados; _contDados++; }
      function indiceDado(j9) { return _mapaDados[j9] == null ? j9 : _mapaDados[j9]; }
      function rotuloGrupoNo(gN9, prefixoPadrao9, chavePrefixo9, substitui9) {
        if (gN9.camposComp) {
          var partes9 = [], qi9, cC9, rot9;
          for (qi9 = 0; qi9 < gN9.camposComp.length; qi9++) {
            cC9 = porCampo[gN9.camposComp[qi9]];
            rot9 = (cC9 && cC9.titulo) || gN9.camposComp[qi9];
            partes9.push(esc(rot9) + ": " + (cC9 ? formata(colFormato(cC9, "formatoGrupo"), gN9.valor[qi9], {}, 0) : esc(gN9.valor[qi9] == null ? "(vazio)" : String(gN9.valor[qi9]))));
          }
          return (prefixoPadrao9 && !substitui9 ? esc(prefixoPadrao9) + " " : "") + partes9.join(' <span class="phx-grupo-mais">+</span> ');
        }
        var cG9 = porCampo[gN9.campo];
        var valorFmt9 = cG9 ? formata(colFormato(cG9, "formatoGrupo"), gN9.valor, {}, 0) : esc(gN9.valor == null ? "(vazio)" : String(gN9.valor));
        if (substitui9) {
          /* prefixoGrupo (cabecalho): SUBSTITUI o rotulo inteiro, nao prefixa -- "Pra\u00e7a: PR" no lugar de "UF: PR" */
          return esc((cG9 && cG9[chavePrefixo9]) || (cG9 && cG9.titulo) || gN9.campo) + ": " + valorFmt9;
        }
        /* prefixoTotal (rodape): PREFIXA antes do titulo normal -- "Subtotal UF: PR" */
        var prefixo9 = (cG9 && cG9[chavePrefixo9]) || prefixoPadrao9;
        return (prefixo9 ? esc(prefixo9) + " " : "") + esc((cG9 && cG9.titulo) || gN9.campo) + ": " + valorFmt9;
      }
      function rodapeGrupo(gN9) {
        if (!cfg.rodapeGrupo) return "";
        var h9 = '<tr class="phx-grupo-rodape" data-gpath="' + esc(gN9.path) + '"><td class="phx-td phx-grupo-rod-rot" colspan="' + spanRot + '" style="padding-left:' + (10 + gN9.nivel * 22) + 'px">' +
          rotuloGrupoNo(gN9, T("totais") === "totais" ? "Total" : T("totais"), "prefixoTotal", false) + "</td>";
        var vA9 = visiveis(), q9, cA9;
        for (q9 = 0; q9 < vA9.length; q9++) {
          cA9 = vA9[q9];
          if (cA9.__esp) { h9 += '<td class="phx-td phx-esp-col"></td>'; continue; }
          h9 += '<td class="phx-td phx-grupo-rod-cel' + (cA9.agregador ? " phx-tipo-numero" : "") + '">' + (cA9.agregador && gN9.aggs[cA9.campo] != null ? esc(formata(colFormato(cA9, "formatoTotal"), gN9.aggs[cA9.campo], {}, 0)) : "") + "</td>";
        }
        return h9 + "</tr>";
      }
      for (j = 0; j < linhas.length; j++) {
        if (linhas[j].__grupo && cfg.rodapeGrupo) while (pilhaG.length && pilhaG[pilhaG.length - 1].nivel >= linhas[j].__grupo.nivel) html += rodapeGrupo(pilhaG.pop());
        if (linhas[j].__grupo && cfg.rodapeGrupo && !recolhidos[linhas[j].__grupo.path]) pilhaG.push(linhas[j].__grupo);
        if (linhas[j].__grupo) {
          var gN = linhas[j].__grupo;
          var abertoG = !recolhidos[gN.path];
          var rotuloG = '<span class="phx-grupo-caret">' + (abertoG ? "\u25be" : "\u25b8") + "</span>" +
            '<span class="phx-grupo-rotulo">' + rotuloGrupoNo(gN, "", "prefixoGrupo", true) + "</span>" +
            '<span class="phx-grupo-conta">(' + fmt.numero(gN.n) + ")</span>";
          if (cfg.condicoesGrupo && gN.condGrupo) {
            var cgHtml9 = "", cgIx9;
            for (cgIx9 = 0; cgIx9 < cfg.condicoesGrupo.length; cgIx9++) {
              if (gN.condGrupo[cgIx9] > 0) cgHtml9 += ' <span class="phx-grupo-cond" title="' + esc(cfg.condicoesGrupo[cgIx9].titulo || "") + '">' + esc(cfg.condicoesGrupo[cgIx9].titulo || "") + ": " + fmt.numero(gN.condGrupo[cgIx9]) + "</span>";
            }
            rotuloG += cgHtml9;
          }
          if (cfg.grupoAlinhado) {
            var extraG = (temDetalhe ? 1 : 0) + (temSelecao ? 1 : 0) + (temCabLinha ? 1 : 0);
            var primAgg = -1, jg;
            for (jg = 0; jg < v.length; jg++) if (v[jg].agregador) { primAgg = jg; break; }
            /* o rotulo ocupa pelo menos a primeira coluna de dados; se ela tiver agregador, o valor dela vai para o resumo do rotulo */
            var iniAgg = primAgg < 0 ? -1 : Math.max(1, primAgg);
            var spanRot = primAgg < 0 ? nCols : extraG + iniAgg;
            if (primAgg === 0 && v[0].agregador && gN.aggs[v[0].campo] != null) rotuloG += '<span class="phx-grupo-aggs">' + esc((v[0].titulo || v[0].campo) + ": " + formata(v[0], gN.aggs[v[0].campo], {}, 0)) + "</span>";
            html += '<tr class="phx-grupo" data-gpath="' + esc(gN.path) + '">' +
              '<td class="phx-td phx-grupo-td" colspan="' + spanRot + '" style="padding-left:' + (10 + gN.nivel * 22) + 'px">' + rotuloG + "</td>";
            if (primAgg >= 0)
              for (jg = iniAgg; jg < v.length; jg++) {
                var cGA = v[jg];
                if (cGA.agregador && gN.aggs[cGA.campo] != null)
                  html += '<td class="phx-td phx-tipo-numero phx-grupo-cel"><span class="phx-tfoot-agg">' + esc(cGA.agregador.toUpperCase()) + "</span> " + formata(colFormato({ tipo: cGA.tipo || "numero", decimais: cGA.decimais, formatoTotal: cGA.formatoTotal }, "formatoTotal"), gN.aggs[cGA.campo], {}, 0) + "</td>";
                else
                  html += '<td class="phx-td phx-grupo-cel"></td>';
              }
            html += "</tr>";
            continue;
          }
          var resumoA = [], ka;
          for (ka in gN.aggs) {
            var cA = porCampo[ka];
            resumoA.push(((cA && cA.titulo) || ka) + ": " + (cA ? formata(cA, gN.aggs[ka], {}, 0) : esc(String(gN.aggs[ka]))));
          }
          html += '<tr class="phx-grupo" data-gpath="' + esc(gN.path) + '"><td class="phx-td phx-grupo-td" colspan="' + nCols + '" style="padding-left:' + (10 + gN.nivel * 22) + 'px">' + rotuloG +
            (resumoA.length ? '<span class="phx-grupo-aggs">' + resumoA.join(" \u00b7 ") + "</span>" : "") +
            "</td></tr>";
          continue;
        }
        var lancCls = temLancamento && lancInvalidas[chaveDe(linhas[j], j)] ? "phx-lanc-invalida" : "";
        if (temArvore && linhas[j].__arv) {
          html += "<tr" + ((linhas[j].__arv.ctx || lancCls) ? ' class="' + (linhas[j].__arv.ctx ? "phx-arv-ctx " : "") + lancCls + '"' : "") +
            ' aria-level="' + (linhas[j].__arv.nivel + 1) + '"' +
            (linhas[j].__arv.temFilhos ? ' aria-expanded="' + (linhas[j].__arv.aberto ? "true" : "false") + '"' : "") + ">";
        } else html += "<tr" + (lancCls ? ' class="' + lancCls + '" title="' + esc(lancInvalidas[chaveDe(linhas[j], j)]) + '"' : "") + ">";
        var chN = temNotas ? chaveDe(linhas[j], j) : null;
        if (temCabLinha) {
          var atual9 = registroAtual && ((estado.pagina - 1) * estado.tamanho + indiceDado(j) + 1) === registroAtual;
          var edit9 = edicaoAtiva && edicaoAtiva.ixL === j;
          html += '<td class="phx-td phx-td-ind' + (atual9 ? " phx-ind-atual" : "") + '" data-ind="' + j + '">' + (edit9 ? "\u270e" : (atual9 ? "\u25b6" : "")) + "</td>";
        }
        if (temDetalhe) {
          var kD = chaveDe(linhas[j], j);
          html += '<td class="phx-td phx-td-exp" data-dl="' + j + '"><button type="button" class="phx-exp-btn">' + (detalhesAbertos[kD] ? "\u25be" : "\u25b8") + "</button></td>";
        }
        if (temSelecao) {
          ch = selecionadas[chaveDe(linhas[j], j)] ? " checked" : "";
          html += '<td class="phx-td phx-td-sel" data-ls="' + j + '"><input type="checkbox" tabindex="-1"' + ch + "></td>";
        }
        for (k = 0; k < v.length; k++) {
          c = v[k];
          if (c.__esp) { html += '<td class="phx-esp-col" style="width:' + c.largura + "px;min-width:" + c.largura + 'px;padding:0"></td>'; continue; }
          val = linhas[j][c.campo];
          var notaTd = temNotas && notas[chN] && notas[chN][c.campo];
          var arvPre = "", arvCls = "", conteudoCel = null;
          if (c.acumulado && linhas[j].__acum && linhas[j].__acum[c.campo] != null) {
            val = linhas[j].__acum[c.campo];
          }
          if (regrasAcesso) val = aplicaMascara(c, val, linhas[j]);
          if (temArvore && linhas[j].__arv) {
            var A9 = linhas[j].__arv;
            if (k === 0) {
              arvPre = '<span class="phx-arv-ind" style="width:' + (A9.nivel * 18) + 'px"></span>' +
                (A9.temFilhos
                  ? '<button type="button" class="phx-arv-caret" data-ak="' + esc(A9.chave) + '" aria-label="' + (A9.aberto ? "recolher" : "expandir") + '">' + (A9.aberto ? "\u25be" : "\u25b8") + "</button>"
                  : '<span class="phx-arv-folha"></span>');
            }
            if (A9.agg && c.agregador === "sum" && A9.agg[c.campo] != null) {
              /* no da arvore com filhos mostra AGREGADO, nao valor proprio -- entao respeita formatoTotal */
              conteudoCel = '<span class="phx-arv-agg">' + formata(colFormato(c, "formatoTotal"), A9.agg[c.campo], linhas[j], j) + "</span>";
              arvCls = " phx-td-agg";
            }
          }
          var histInd = "";
          if (temHistCel && chaveCampo) {
            var kH9 = chaveDe(linhas[j], j) + "\u0000" + c.campo;
            if (histCel[kH9] && histCel[kH9].length) histInd = '<span class="phx-hist-ind" data-hc="' + esc(c.campo) + '" data-hk="' + esc(chaveDe(linhas[j], j)) + '" title="' + histCel[kH9].length + ' altera\u00e7\u00e3o(\u00f5es)"></span>';
          }
          var vNum9 = Number(val);
          var negCls9 = (cfg.negativos !== false && c.negativo !== false && (c.tipo === "numero" || c.tipo === "moeda" || c.tipo === "percentual") && val != null && val !== "" && vNum9 === vNum9 && vNum9 < 0) ? " phx-td-neg" : "";
          var estCls = (c.estilo ? " phx-est-" + c.estilo : "") + (condicoes.length ? classesCondicao(c.campo, linhas[j]) : "") + (c.quebraLinha ? " phx-td-quebra" : "") + negCls9;
          var alinCel = c.alinhamento ? ' style="text-align:' + c.alinhamento + '"' : "";
          var dicaCel = temDicas ? ' title="' + esc(textoVisivel(c, val)) + '"' : "";
          var miolo9 = conteudoCel != null ? conteudoCel : formata(c, val, linhas[j], j), chaveFetch9 = null;
          if (c.buscarValor && !linhas[j].__grupo) {
            chaveFetch9 = chaveDe(linhas[j], j) + "\u0000" + c.campo;
            if (Object.prototype.hasOwnProperty.call(_cacheBusca, chaveFetch9)) miolo9 = formata(c, _cacheBusca[chaveFetch9], linhas[j], j);
            else { miolo9 = c.iconeBusca ? '<img class="phx-fetch-pendente phx-fetch-icone" data-fetch="' + esc(chaveFetch9) + '" src="' + esc(c.iconeBusca) + '" alt="carregando">' : '<span class="phx-fetch-pendente" data-fetch="' + esc(chaveFetch9) + '">\u2026</span>'; _pendentesBusca.push({ chave: chaveFetch9, col: c, linha: linhas[j], ix: j }); }
          }
          html += '<td class="phx-td phx-tipo-' + (c.tipo || "texto") + (notaTd ? " phx-td-nota" : "") + arvCls + (histInd ? " phx-td-hist" : "") + estCls + '"' +
            (c.fixa ? ' data-fx="' + esc(c.campo) + '"' : "") + (c.tag != null && typeof c.tag !== "object" ? ' data-tag="' + esc(String(c.tag)) + '"' : "") + alinCel + dicaCel + (chaveFetch9 ? ' data-fetchcel="' + esc(chaveFetch9) + '"' : "") + ">" + arvPre + miolo9 + histInd + (notaTd ? '<span class="phx-nota-ind" data-nc="' + esc(c.campo) + '" data-nl="' + j + '" title="ver nota"></span>' : "") + "</td>";
        }
        html += "</tr>";
        if (colPreview9 && !linhas[j].__grupo) {
          html += '<tr class="phx-preview"><td class="phx-td phx-preview-td" colspan="' + nCols + '">' +
            '<span class="phx-preview-rotulo">' + esc(colPreview9.titulo || colPreview9.campo) + ":</span> " +
            esc(textoVisivel(colPreview9, linhas[j][colPreview9.campo])) + "</td></tr>";
        }
        if (temDetalhe && detalhesAbertos[chaveDe(linhas[j], j)]) {
          html += '<tr class="phx-detalhe"><td class="phx-td phx-detalhe-td" colspan="' + nCols + '" data-dl="' + j + '"></td></tr>';
        }
      }
      if (cfg.rodapeGrupo) while (pilhaG.length) html += rodapeGrupo(pilhaG.pop());
      if (cfg.linhasVazias && estado.tamanho) {
        var faltam9 = estado.tamanho - _contDados, q9;
        for (q9 = 0; q9 < faltam9; q9++) html += '<tr class="phx-tr-vazia"><td class="phx-td" colspan="' + nCols + '">&nbsp;</td></tr>';
      }
      var tB = agora();
      tbody.innerHTML = html;
      if (_pendentesBusca.length) {
        var pend9 = _pendentesBusca; _pendentesBusca = [];
        (function () {
          var geracaoTb9 = tbody;
          var i9;
          for (i9 = 0; i9 < pend9.length; i9++) (function (item9) {
            var jaVeio9 = false;
            try {
              item9.col.buscarValor(item9.linha, function (valor9) {
                if (jaVeio9) return; jaVeio9 = true;
                _cacheBusca[item9.chave] = valor9;
                if (tbody !== geracaoTb9) return; /* grid destruido/recriado nesse meio-tempo */
                var el9 = tbody.querySelector('[data-fetch="' + item9.chave.replace(/"/g, '\\\\"') + '"]');
                if (!el9) return;
                el9.outerHTML = formata(item9.col, valor9, item9.linha, item9.ix);
                log("fetch-valor", { campo: item9.col.campo, ok: true });
              });
            } catch (e9) { log("fetch-valor", { campo: item9.col.campo, ok: false, erro: String(e9 && e9.message || e9) }); }
          })(pend9[i9]);
        })();
      }
      if (temHistCel) {
        var inds9 = tbody.querySelectorAll(".phx-hist-ind"), jh9;
        for (jh9 = 0; jh9 < inds9.length; jh9++) (function (sp9) {
          sp9.addEventListener("click", function (ev9) {
            ev9.stopPropagation();
            api.abrirHistorico(sp9.getAttribute("data-hk"), sp9.getAttribute("data-hc"), sp9);
          });
        })(inds9[jh9]);
      }
      if (temArvore) {
        var carets9 = tbody.querySelectorAll(".phx-arv-caret"), jc9;
        for (jc9 = 0; jc9 < carets9.length; jc9++) (function (b9) {
          b9.addEventListener("click", function (ev9) {
            ev9.stopPropagation();
            api.alternarNo(b9.getAttribute("data-ak"));
          });
        })(carets9[jc9]);
      }
      if (temDetalhe) {
        var tds = tbody.querySelectorAll(".phx-detalhe-td"), jD, lref, ret;
        for (jD = 0; jD < tds.length; jD++) {
          lref = linhas[parseInt(tds[jD].getAttribute("data-dl"), 10)];
          ret = cfg.detalhe.render(lref, tds[jD], api);
          if (typeof ret === "string") tds[jD].innerHTML = ret;
        }
      }
      var tC = agora();
      atualizaMestre();
      perfRender.strMs = Math.round((tB - tA) * 100) / 100;
      perfRender.domMs = Math.round((tC - tB) * 100) / 100;
      ariaCorpo();
      perfRender.mestreMs = Math.round((agora() - tC) * 100) / 100;
    }
    function atualizaMestre() {
      if (!temSelecao) return;
      var mestre = thead.querySelector(".phx-sel-mestre");
      if (!mestre) return;
      var linhas = ultimaCarga ? ultimaCarga.linhas : [], j, n = 0;
      for (j = 0; j < linhas.length; j++) if (selecionadas[chaveDe(linhas[j], j)]) n++;
      mestre.checked = n > 0 && n === linhas.length;
      mestre.indeterminate = n > 0 && n < linhas.length;
    }
    function alternaLinha(ix, forcar) {
      var linhas = ultimaCarga.linhas, k2 = chaveDe(linhas[ix], ix);
      var novo = forcar != null ? forcar : !selecionadas[k2];
      if (novo && !selecionadas[k2]) { selecionadas[k2] = true; nSel++; dadosSel[k2] = linhas[ix]; }
      else if (!novo && selecionadas[k2]) { delete selecionadas[k2]; nSel--; delete dadosSel[k2]; }
      var td2 = tbody.querySelector('td[data-ls="' + ix + '"] input');
      if (td2) td2.checked = !!novo;
      montaTfoot();
      var trA = td2 && td2.closest ? td2.closest("tr") : null;
      if (trA) trA.setAttribute("aria-selected", novo ? "true" : "false");
    }

    function totPaginas() { return Math.max(1, Math.ceil(estado.total / estado.tamanho)); }
    function montaPag() {
      var tp = totPaginas(), p = estado.pagina, itens = [], j;
      function btn(rot, alvo2, disab, ativo) {
        return '<button type="button" class="phx-pg' + (ativo ? " phx-pg-ativo" : "") + '"' +
          (disab ? " disabled" : "") + ' data-p="' + alvo2 + '">' + rot + "</button>";
      }
      itens.push(btn("«", 1, p === 1));
      itens.push(btn("‹", p - 1, p === 1));
      var ini = Math.max(1, p - 2), fim = Math.min(tp, ini + 4);
      ini = Math.max(1, fim - 4);
      if (ini > 1) itens.push('<span class="phx-pg-elipse">…</span>');
      for (j = ini; j <= fim; j++) itens.push(btn(String(j), j, false, j === p));
      if (fim < tp) itens.push('<span class="phx-pg-elipse">…</span>');
      itens.push(btn("›", p + 1, p === tp));
      itens.push(btn("»", tp, p === tp));
      itens.push('<span class="phx-pg-irpara">' + esc(T("irPara")) + ' <input type="number" min="1" max="' + tp + '" value="' + p + '"></span>');
      var tP = agora();
      pagEl.innerHTML = esc(T("pagina")) + " " + p + " " + esc(T("de")) + " " + fmt.numero(tp) + " (" + fmt.numero(estado.total) + " " + esc(T("registros")) + ") " + itens.join("");
      var bs = pagEl.querySelectorAll(".phx-pg"), j2;
      for (j2 = 0; j2 < bs.length; j2++) {
        (function (bEl) {
          bEl.addEventListener("click", function () { irPagina(parseInt(bEl.getAttribute("data-p"), 10)); });
        })(bs[j2]);
      }
      var inp = pagEl.querySelector(".phx-pg-irpara input");
      inp.addEventListener("change", function () { irPagina(parseInt(inp.value, 10)); });
      var ini2 = estado.total ? (p - 1) * estado.tamanho + 1 : 0;
      var fim2 = Math.min(estado.total, p * estado.tamanho);
      if (grupos.length && ultimaCarga && ultimaCarga.totalDados != null)
        mostrandoEl.textContent = fmt.numero(ultimaCarga.totalDados) + " linhas em " + grupos.length + " n\u00edvel" + (grupos.length > 1 ? "eis" : "") + " de grupo";
      else
        mostrandoEl.textContent = T("mostrando") + " " + fmt.numero(ini2) + "\u2013" + fmt.numero(fim2) + " de " + fmt.numero(estado.total);
      atualizaContaBusca();
      montaTfoot();
      perfRender.pagMs = Math.round((agora() - tP) * 100) / 100;
    }
    function irPagina(p) {
      var tp = totPaginas();
      if (!p || p !== p) p = 1;
      if (p < 1) p = 1;
      if (p > tp) p = tp;
      if (p === estado.pagina) { montaPag(); return; }
      var t1 = agora();
      estado.pagina = p;
      carrega(function () { log("page", { pagina: p, ms: Math.round((agora() - t1) * 10) / 10 }); }, true);
    }
    tamSel.addEventListener("change", function () {
      estado.tamanho = parseInt(tamSel.value, 10);
      estado.pagina = 1;
      carrega(function () { log("pagesize", { tamanho: estado.tamanho }); }, true);
    });

    function montaColSel() {
      var vis = visiveis().length;
      colBtn.textContent = T("colunas") + ": " + vis + " \u25be";
      var html = '<div class="phx-colsel-acoes"><button type="button" data-cs="todas">todas</button><button type="button" data-cs="minimo">m\u00ednimo</button></div>', j, c;
      for (j = 0; j < ordemColunas.length; j++) {
        c = porCampo[ordemColunas[j]];
        if (regrasAcesso && acessoDe(c.campo).oculta) continue;
        html += '<label class="phx-colsel-item"><input type="checkbox" data-campo="' + esc(c.campo) + '"' +
          (ocultas[c.campo] ? "" : " checked") + "> " + esc(c.titulo || c.campo) +
          (c.prioridade != null ? '<span class="phx-colsel-pri">p' + c.prioridade + "</span>" : "") + "</label>";
      }
      colMenu.innerHTML = html;
      var acs = colMenu.querySelectorAll("[data-cs]"), ka;
      for (ka = 0; ka < acs.length; ka++) (function (bt) {
        bt.addEventListener("click", function (ev) {
          ev.stopPropagation();
          var modo = bt.getAttribute("data-cs"), j3, c3;
          for (j3 = 0; j3 < ordemColunas.length; j3++) {
            c3 = porCampo[ordemColunas[j3]];
            if (regrasAcesso && acessoDe(c3.campo).oculta) continue;
            if (modo === "todas") delete ocultas[c3.campo];
            else if (c3.prioridade != null && c3.prioridade > 2) ocultas[c3.campo] = true;
          }
          invalidaJanela();
          montaHeader();
          carrega(null, true);
          montaColSel();
          log("colsel", { modo: modo });
        });
      })(acs[ka]);
      var cbs = colMenu.querySelectorAll(".phx-colsel-item input"), k2;
      for (k2 = 0; k2 < cbs.length; k2++) {
        (function (cb) {
          cb.addEventListener("change", function () {
            mostrarColuna(cb.getAttribute("data-campo"), cb.checked);
          });
        })(cbs[k2]);
      }
    }
    colBtn.addEventListener("click", function () { colMenu.hidden = !colMenu.hidden; });
    function mostrarColuna(campo, mostrar) {
      if (mostrar) delete ocultas[campo]; else ocultas[campo] = true;
      log("coluna", { campo: campo, visivel: !!mostrar });
      montaHeader(); reRender(); montaColSel();
    }

    function carrega(cb, semHeader) {
      _resumoCache = null;
      if (ehVirtual) {
        if (!semHeader) montaHeader();
        carregaVirtual(cb);
        return;
      }
      var c = porCampo[estado.ordem.campo];
      fonte.carregar({
        pagina: estado.pagina, tamanho: estado.tamanho,
        ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c ? c.tipo : null },
        ordens: (function () {
          var o9 = [], j9, cc9;
          for (j9 = 0; j9 < estado.ordens.length; j9++) {
            cc9 = porCampo[estado.ordens[j9].campo];
            o9.push({ campo: estado.ordens[j9].campo, dir: estado.ordens[j9].dir, tipo: cc9 ? cc9.tipo : null });
          }
          return o9;
        })(),
        filtros: serializaFiltros(),
        grupos: temArvore ? [] : grupos.slice(),
        condicoesGrupo: cfg.condicoesGrupo,
        recolhidos: recolhidos,
        acumulados: (!temArvore && !grupos.length && _acumulados.length) ? _acumulados : null,
        arvore: temArvore ? { pai: cfg.arvore.pai, chaveCampo: chaveCampo, abertos: abertosArv, inicioAberto: inicioAbertoRt, agregar: cfg.arvore.agregar !== false } : null,
        aggCols: (function () { var o3 = [], j3; for (j3 = 0; j3 < colunasDef.length; j3++) if (colunasDef[j3].agregador) o3.push({ campo: colunasDef[j3].campo, agregador: colunasDef[j3].agregador }); return o3; })(),
        totais: temTotais ? (function () { var o3 = [], j3; for (j3 = 0; j3 < colunasDef.length; j3++) if (colunasDef[j3].agregador) o3.push({ campo: colunasDef[j3].campo, agregador: colunasDef[j3].agregador }); return o3; })() : null,
        tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
      }, function (err, r) {
        if (err) { log("erro", { erro: String(err) }); return; }
        if (temSelecao && !chaveCampo && ultimaCarga) { selecionadas = {}; nSel = 0; ancoraSel = -1; dadosSel = {}; }
        if (temDetalhe && !chaveCampo && ultimaCarga) detalhesAbertos = {};
        fechaPopover();
        ultimaCarga = r;
        estado.total = r.total;
        if (!semHeader) montaHeader();
        montaCorpo(r.linhas); montaPag();
        if (cb) cb(r);
      });
    }

    var api = {
      ok: true, el: wrap,
      rolarPara: function (alvo9) {
        if (!ehVirtual || !cacheV) return api;
        var ix9 = -1, j9;
        if (typeof alvo9 === "number") ix9 = alvo9;
        else {
          for (j9 = 0; j9 < cacheV.length; j9++) if (chaveDe(cacheV[j9], j9) === String(alvo9)) { ix9 = j9; break; }
        }
        if (ix9 < 0 || ix9 >= cacheV.length) return api;
        envoltorio.scrollTop = Math.max(0, ix9 * vLinha - Math.floor(vAltura / 3));
        renderJanela(true);
        log("virtual-rolar", { ix: ix9 });
        return api;
      },
      janela: function () { return { ini: vIni, fim: vFim, total: cacheV ? cacheV.length : 0 }; },
      gerarXLSX: function (opts) {
        opts = opts || {};
        var colsE9 = [], j9, v9 = visiveis();
        if (opts.colunas) { for (j9 = 0; j9 < opts.colunas.length; j9++) if (porCampo[opts.colunas[j9]]) colsE9.push(porCampo[opts.colunas[j9]]); }
        else colsE9 = v9;
        var c09 = porCampo[estado.ordem.campo];
        var linhasOut9 = null;
        fonte.carregar({
          pagina: 1, tamanho: 1e9,
          ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c09 ? c09.tipo : null },
          ordens: (function () {
            var o8 = [], j8, cc8;
            for (j8 = 0; j8 < estado.ordens.length; j8++) {
              cc8 = porCampo[estado.ordens[j8].campo];
              o8.push({ campo: estado.ordens[j8].campo, dir: estado.ordens[j8].dir, tipo: cc8 ? cc8.tipo : null });
            }
            return o8;
          })(),
          filtros: serializaFiltros(),
          grupos: [], recolhidos: {}, aggCols: [], totais: null,
          acumulados: _acumulados.length ? _acumulados : null,
          tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
        }, function (err, resp) { linhasOut9 = (resp && resp.linhas) || []; });
        if (linhasOut9 == null) { log("export", { formato: "xlsx", erro: "fonte ass\u00edncrona n\u00e3o suportada no export s\u00edncrono" }); return null; }
        var linhasX9 = linhasOut9;
        if (regrasAcesso) {
          linhasX9 = [];
          var iX9, jX9, cpX9, novaX9;
          for (iX9 = 0; iX9 < linhasOut9.length; iX9++) {
            novaX9 = {};
            for (cpX9 in linhasOut9[iX9]) novaX9[cpX9] = linhasOut9[iX9][cpX9];
            for (jX9 = 0; jX9 < colsE9.length; jX9++) {
              var acX9 = acessoDe(colsE9[jX9].campo);
              if (acX9.mascara) novaX9[colsE9[jX9].campo] = aplicaMascara(colsE9[jX9], novaX9[colsE9[jX9].campo], linhasOut9[iX9]);
            }
            linhasX9.push(novaX9);
          }
        }
        var bytes9 = montaXLSX(colsE9, linhasX9, { aba: opts.aba || cfg.titulo || "Dados" });
        log("export", { formato: "xlsx", linhas: linhasOut9.length, colunas: colsE9.length, bytes: bytes9.length });
        return bytes9;
      },
      xlsxBase64: function (opts) {
        var b9 = api.gerarXLSX(opts);
        if (!b9) return "";
        var ALF = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        var out9 = [], i9, n9, a9, b8, c9;
        for (i9 = 0; i9 < b9.length; i9 += 3) {
          a9 = b9[i9]; b8 = i9 + 1 < b9.length ? b9[i9 + 1] : 0; c9 = i9 + 2 < b9.length ? b9[i9 + 2] : 0;
          n9 = (a9 << 16) | (b8 << 8) | c9;
          out9.push(ALF.charAt((n9 >> 18) & 63), ALF.charAt((n9 >> 12) & 63),
            i9 + 1 < b9.length ? ALF.charAt((n9 >> 6) & 63) : "=",
            i9 + 2 < b9.length ? ALF.charAt(n9 & 63) : "=");
        }
        return out9.join("");
      },
      baixarXLSX: function (nome9, opts) {
        var b9 = api.gerarXLSX(opts);
        if (!b9) return api;
        try {
          var blob9 = new Blob([b9], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" });
          var a8 = document.createElement("a");
          a8.href = URL.createObjectURL(blob9);
          a8.download = nome9 || "phx-grid.xlsx";
          document.body.appendChild(a8);
          a8.click();
          document.body.removeChild(a8);
          setTimeout(function () { URL.revokeObjectURL(a8.href); }, 800);
        } catch (e9) { log("export", { formato: "xlsx", erro: String(e9 && e9.message) }); }
        return api;
      },
      exportarCSV: function (opts) {
        opts = opts || {};
        var sep = opts.separador || ";";
        var colsE = [], j9, v9 = visiveis();
        if (opts.colunas) { for (j9 = 0; j9 < opts.colunas.length; j9++) if (porCampo[opts.colunas[j9]]) colsE.push(porCampo[opts.colunas[j9]]); }
        else colsE = v9;
        var c0 = porCampo[estado.ordem.campo];
        var linhasOut = null;
        fonte.carregar({
          pagina: 1, tamanho: 1e9,
          ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c0 ? c0.tipo : null },
          ordens: (function () {
            var o9 = [], j9, cc9;
            for (j9 = 0; j9 < estado.ordens.length; j9++) {
              cc9 = porCampo[estado.ordens[j9].campo];
              o9.push({ campo: estado.ordens[j9].campo, dir: estado.ordens[j9].dir, tipo: cc9 ? cc9.tipo : null });
            }
            return o9;
          })(),
          filtros: serializaFiltros(),
          grupos: [], recolhidos: {}, aggCols: [], totais: null,
          tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
        }, function (err, resp) { linhasOut = (resp && resp.linhas) || []; });
        if (linhasOut == null) { log("export", { formato: "csv", erro: "fonte assíncrona não suportada no export síncrono" }); return ""; }
        var partes = ["\uFEFF"], cab = [], j10;
        for (j10 = 0; j10 < colsE.length; j10++) cab.push(csvCampo(colsE[j10].titulo || colsE[j10].campo, sep));
        partes.push(cab.join(sep));
        var i9, lin;
        for (i9 = 0; i9 < linhasOut.length; i9++) {
          lin = [];
          for (j10 = 0; j10 < colsE.length; j10++)
            lin.push(csvCampo(textoCelula(colsE[j10], regrasAcesso ? aplicaMascara(colsE[j10], linhasOut[i9][colsE[j10].campo], linhasOut[i9]) : linhasOut[i9][colsE[j10].campo], linhasOut[i9]), sep));
          partes.push(lin.join(sep));
        }
        var csv = partes[0] + partes.slice(1).join("\r\n");
        log("export", { formato: "csv", linhas: linhasOut.length, colunas: colsE.length, bytes: csv.length });
        return csv;
      },
      baixarCSV: function (nome, opts) {
        var csv = api.exportarCSV(opts);
        if (!csv) return api;
        try {
          var blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
          var a2 = document.createElement("a");
          a2.href = URL.createObjectURL(blob);
          a2.download = nome || "phx-grid.csv";
          document.body.appendChild(a2);
          a2.click();
          document.body.removeChild(a2);
          setTimeout(function () { URL.revokeObjectURL(a2.href); }, 2000);
        } catch (e9) { log("export", { formato: "csv", erro: "download indisponível" }); }
        return api;
      },
      copiarSelecao: function (optsC9) {
        optsC9 = optsC9 || {};
        if (!nSel) { log("copia", { linhas: 0, aviso: "sem selecao" }); return ""; }
        var v9 = visiveis(), cab = [], j9;
        for (j9 = 0; j9 < v9.length; j9++) cab.push(tsvCampo(String(v9[j9].titulo || v9[j9].campo), optsC9.aspas));
        var linhasT = [cab.join("\t")];
        /* ordem da tela: percorre o filtrado atual (todos, na ordem do sort) e pega as selecionadas; complementa com dadosSel fora do filtro */
        var vistas = {}, i9;
        var baseOrd = null;
        var c0 = porCampo[estado.ordem.campo];
        fonte.carregar({
          pagina: 1, tamanho: 1e9,
          ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c0 ? c0.tipo : null },
          filtros: serializaFiltros(), grupos: [], recolhidos: {}, aggCols: [], totais: null,
          tiposCampos: {}
        }, function (err, resp) { baseOrd = (resp && resp.linhas) || []; });
        var fila = [];
        if (baseOrd) {
          for (i9 = 0; i9 < baseOrd.length; i9++) {
            var kO = chaveDe(baseOrd[i9], i9);
            if (selecionadas[kO]) { fila.push(baseOrd[i9]); vistas[kO] = true; }
          }
        }
        var kS;
        for (kS in dadosSel) if (!vistas[kS]) fila.push(dadosSel[kS]);
        for (i9 = 0; i9 < fila.length; i9++) {
          var cel = [];
          for (j9 = 0; j9 < v9.length; j9++) cel.push(tsvCampo(textoCelula(v9[j9], fila[i9][v9[j9].campo], fila[i9]), optsC9.aspas));
          linhasT.push(cel.join("\t"));
        }
        var tsv = linhasT.join("\r\n");
        try { if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(tsv); } catch (e9) {}
        log("copia", { linhas: fila.length });
        return tsv;
      },
      layout: function () {
        var cols = [], j9, c9;
        for (j9 = 0; j9 < ordemColunas.length; j9++) {
          c9 = porCampo[ordemColunas[j9]];
          cols.push({ campo: c9.campo, titulo: c9.titulo || null, oculta: !!ocultas[c9.campo], largura: c9.largura || null, agregador: c9.agregador || null, fixa: c9.fixa || null, estilo: c9.estilo || null, estiloCabecalho: c9.estiloCabecalho || null });
        }
        return {
          v: 1,
          alturaLinha: _alturaLinha || undefined,
          colunas: cols,
          ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir },
          ordens: JSON.parse(JSON.stringify(estado.ordens)),
          filtros: serializaFiltros(),
          grupos: grupos.slice(),
          recolhidos: (function () { var o9 = {}, k9; for (k9 in recolhidos) o9[k9] = true; return o9; })(),
          tamanho: estado.tamanho,
          arvore: temArvore ? { inicioAberto: inicioAbertoRt, abertos: (function () { var o9 = {}, k9; for (k9 in abertosArv) o9[k9] = abertosArv[k9]; return o9; })() } : null,
          tema: temaAtual,
          densidade: densAtual
        };
      },
      aplicarLayout: function (l) {
        if (!l || l.v !== 1) {
          log("layout", { acao: "rejeitado", motivo: "versao", v: l && l.v });
          return api;
        }
        var j9, c9, parcial = false;
        /* ordem best-effort: mover cada campo, na sequência salva, para o fim relativo */
        var alvoSeq = [];
        for (j9 = 0; j9 < (l.colunas || []).length; j9++) if (porCampo[l.colunas[j9].campo]) alvoSeq.push(l.colunas[j9].campo);
        for (j9 = alvoSeq.length - 2; j9 >= 0; j9--) moverColunaAntes(alvoSeq[j9], alvoSeq[j9 + 1]);
        var finalSeq = [], atF = flattenOrdem();
        for (j9 = 0; j9 < atF.length; j9++) if (alvoSeq.indexOf(atF[j9]) >= 0) finalSeq.push(atF[j9]);
        if (finalSeq.join(",") !== alvoSeq.join(",")) parcial = true;
        ocultas = {};
        for (j9 = 0; j9 < (l.colunas || []).length; j9++) {
          c9 = porCampo[l.colunas[j9].campo];
          if (!c9) { parcial = true; continue; }
          if (l.colunas[j9].oculta) ocultas[c9.campo] = true;
          if (l.colunas[j9].largura) c9.largura = l.colunas[j9].largura;
          if (l.colunas[j9].titulo) c9.titulo = l.colunas[j9].titulo;
          if (j9 === 0 && l.alturaLinha) api.alturaLinha(l.alturaLinha);
          if (l.colunas[j9].estilo !== undefined) c9.estilo = l.colunas[j9].estilo || null;
          if (l.colunas[j9].estiloCabecalho !== undefined) c9.estiloCabecalho = l.colunas[j9].estiloCabecalho || null;
          if (l.colunas[j9].fixa !== undefined) { if (l.colunas[j9].fixa) c9.fixa = l.colunas[j9].fixa; else delete c9.fixa; }
          if (l.colunas[j9].agregador) c9.agregador = l.colunas[j9].agregador;
        }
        estado.ordens = [];
        var jO9;
        if (l.ordens && l.ordens.length) {
          for (jO9 = 0; jO9 < l.ordens.length; jO9++) if (porCampo[l.ordens[jO9].campo]) estado.ordens.push({ campo: l.ordens[jO9].campo, dir: l.ordens[jO9].dir === "desc" ? "desc" : "asc" });
        } else if (l.ordem && l.ordem.campo) estado.ordens.push({ campo: l.ordem.campo, dir: l.ordem.dir === "desc" ? "desc" : "asc" });
        sincronizaOrdemPrimaria();
        filtros = {};
        for (j9 = 0; j9 < (l.filtros || []).length; j9++) filtros[l.filtros[j9].campo] = l.filtros[j9];
        grupos = (l.grupos || []).slice();
        if (temArvore && l.arvore) {
          inicioAbertoRt = l.arvore.inicioAberto == null ? inicioAbertoRt : l.arvore.inicioAberto;
          abertosArv = {};
          var kA9;
          for (kA9 in (l.arvore.abertos || {})) abertosArv[kA9] = !!l.arvore.abertos[kA9];
        }
        recolhidos = {};
        var k9;
        for (k9 in (l.recolhidos || {})) recolhidos[k9] = true;
        if (l.tamanho) estado.tamanho = l.tamanho;
        if (l.tema) { temaAtual = l.tema; aplicaTemaClasse(); }
        if (l.densidade) {
          densAtual = l.densidade;
          aplicaDensClasse();
          if (ehVirtual && !(cfgV && cfgV.alturaLinha)) vLinha = ALT_DENS[densAtual] || 36;
        }
        estado.pagina = 1;
        renderGroupBox();
        montaChips();
        carrega(function () {
          log("layout", { acao: "aplicado", parcial: parcial });
        });
        return api;
      },
      ordenar: function (campo, dir) {
        agendaLayoutCb();
        estado.ordens = dir ? [{ campo: campo, dir: dir }] : [];
        sincronizaOrdemPrimaria();
        log("sort", { campo: campo, dir: dir || null, origem: "api" });
        estado.pagina = 1;
        carrega(function () { montaHeader(); }); return api;
      },
      ordenarMulti: function (lista9) {
        agendaLayoutCb();
        var novas9 = [], j9, it9;
        for (j9 = 0; j9 < (lista9 || []).length; j9++) {
          it9 = lista9[j9];
          if (!it9 || !porCampo[it9.campo]) continue;
          novas9.push({ campo: it9.campo, dir: it9.dir === "desc" ? "desc" : "asc" });
        }
        estado.ordens = novas9;
        sincronizaOrdemPrimaria();
        estado.pagina = 1;
        log("sort", { origem: "multi", niveis: novas9.length, multi: novas9.length > 1 });
        carrega(function () { montaHeader(); });
        return api;
      },
      adicionarOrdem: function (campo9, dir9) {
        if (!porCampo[campo9]) return api;
        var at9 = posNaCascata(campo9);
        if (at9 >= 0) estado.ordens[at9].dir = dir9 === "desc" ? "desc" : "asc";
        else estado.ordens.push({ campo: campo9, dir: dir9 === "desc" ? "desc" : "asc" });
        sincronizaOrdemPrimaria();
        estado.pagina = 1;
        agendaLayoutCb();
        log("sort", { origem: "adicionar", campo: campo9, niveis: estado.ordens.length, multi: estado.ordens.length > 1 });
        carrega(function () { montaHeader(); });
        return api;
      },
      limparOrdem: function () {
        estado.ordens = [];
        sincronizaOrdemPrimaria();
        estado.pagina = 1;
        agendaLayoutCb();
        carrega(function () { montaHeader(); });
        return api;
      },
      ordens: function () { return JSON.parse(JSON.stringify(estado.ordens)); },
      pagina: function (p) { if (p == null) return estado.pagina; irPagina(p); return api; },
      tamanhoPagina: function (n) {
        agendaLayoutCb();
        if (n == null) return estado.tamanho;
        estado.tamanho = n; estado.pagina = 1; tamSel.value = String(n); carrega(null, true); return api;
      },
      mostrarColuna: mostrarColuna,
      moverColuna: function (campo, antesDe) { moverColunaAntes(campo, antesDe); agendaLayoutCb(); return api; },
      ocultar: function (campo, oculta) {
        invalidaJanela();
        agendaLayoutCb();
        if (oculta === false) delete ocultas[campo];
        else ocultas[campo] = true;
        log("hide", { campo: campo, oculta: oculta !== false });
        carrega(null);
        return api;
      },
      colunasVisiveis: function () { var v = visiveis(), o = [], j; for (j = 0; j < v.length; j++) o.push(v[j].campo); return o; },
      /* Tag: metadado livre por coluna, agora com semantica -- consultavel, alteravel em runtime
         e emitido como data-tag no th e nas td (quando escalar), para CSS/DOM/automacao mirarem nele. */
      tag: function (campo9, valor9) {
        var c9 = porCampo[campo9];
        if (!c9) return valor9 === undefined ? undefined : api;
        if (valor9 === undefined) return c9.tag;
        c9.tag = valor9;
        log("tag", { campo: campo9, tag: (valor9 != null && typeof valor9 === "object") ? "(objeto)" : valor9 });
        montaHeader(); /* o data-tag vive no th tambem -- reRender() sozinho so remonta o corpo */
        reRender();
        return api;
      },
      colunasPorTag: function (valor9) {
        var o9 = [], j9;
        for (j9 = 0; j9 < colunasDef.length; j9++) {
          var t9 = colunasDef[j9].tag;
          if (t9 === valor9) { o9.push(colunasDef[j9].campo); continue; }
          if (t9 instanceof Array && t9.indexOf(valor9) >= 0) o9.push(colunasDef[j9].campo);
        }
        return o9;
      },
      linhas: function () { return ultimaCarga ? ultimaCarga.linhas : []; },
      estado: function () {
        return { ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir }, pagina: estado.pagina,
          tamanho: estado.tamanho, total: estado.total, colunas: api.colunasVisiveis(), selecao: nSel, filtros: serializaFiltros() };
      },
      filtrar: function (campo, condicao) {
        agendaLayoutCb();
        if (campo !== "*" && !porCampo[campo]) return api;
        var t1 = agora();
        var f2 = null;
        if (condicao != null) {
          f2 = { tipo: condicao.tipo };
          if (condicao.tipo === "valores") { f2.valores = (condicao.valores || []).slice(); if (condicao.incluiNulos) f2.incluiNulos = true; }
          else if (condicao.tipo === "texto") f2.contem = String(condicao.contem || "");
          else if (condicao.tipo === "faixa") { f2.de = condicao.de != null ? condicao.de : null; f2.ate = condicao.ate != null ? condicao.ate : null; }
          else if (condicao.tipo === "expr") { f2.op = condicao.op; f2.valor = condicao.valor; }
          else if (condicao.tipo === "busca") {
            f2.termo = String(condicao.termo || "");
            f2.campos = (condicao.campos || camposBuscaveis()).slice();
            if (!f2.termo) condicao = null;
          }
          else if (condicao.tipo === "multi") {
            f2.combinador = condicao.combinador === "ou" ? "ou" : "e";
            f2.condicoes = [];
            var j5;
            for (j5 = 0; j5 < (condicao.condicoes || []).length; j5++)
              if (condicao.condicoes[j5] && condicao.condicoes[j5].op != null && condicao.condicoes[j5].valor != null && condicao.condicoes[j5].valor === condicao.condicoes[j5].valor)
                f2.condicoes.push({ op: condicao.condicoes[j5].op, valor: condicao.condicoes[j5].valor });
            if (!f2.condicoes.length) return api;
          }
          else return api;
          f2.tipoCol = campo === "*" ? "texto" : (porCampo[campo].tipo || "texto");
        }
        if (condicao == null || f2 == null) { delete filtros[campo]; limpaControleRow(campo); }
        else filtros[campo] = f2;
        estado.pagina = 1;
        montaChips();
        var fb = thead.querySelector('th[data-campo="' + campo + '"] .phx-fbtn');
        if (fb) fb.className = "phx-fbtn" + (filtros[campo] ? " phx-fbtn-on" : "");
        carrega(function () {
          log("filter", { campo: campo, expr: condicao == null ? "(removido)" : resumoFiltro(campo, filtros[campo]), total: estado.total, ms: Math.round((agora() - t1) * 10) / 10 });
        }, true);
        return api;
      },
      expandirDetalhe: function (chaveOuLinha, abrir) {
        var k3 = typeof chaveOuLinha === "object" ? (chaveCampo ? String(chaveOuLinha[chaveCampo]) : null) : String(chaveOuLinha);
        if (k3 == null) return api;
        if (abrir === false) delete detalhesAbertos[k3];
        else detalhesAbertos[k3] = true;
        reRender();
        log("drill", { chave: k3, aberto: abrir !== false });
        return api;
      },
      detalhesAbertos: function () { var o = [], k3; for (k3 in detalhesAbertos) if (detalhesAbertos[k3]) o.push(k3); return o; },
      recolherDetalhes: function () { detalhesAbertos = {}; reRender(); return api; },
      copiarPagina: function (opts9) {
        opts9 = opts9 || {};
        var cols9 = [], j9, k9, v9 = visiveis();
        for (j9 = 0; j9 < v9.length; j9++) if (!opts9.campos || opts9.campos.indexOf(v9[j9].campo) >= 0) cols9.push(v9[j9]);
        var atuais9 = (ultimaCarga && ultimaCarga.linhas) || [], linhas9 = [], cel9;
        if (opts9.cabecalho !== false) {
          cel9 = [];
          for (k9 = 0; k9 < cols9.length; k9++) cel9.push(tsvCampo(cols9[k9].titulo || cols9[k9].campo, opts9.aspas));
          linhas9.push(cel9.join("\t"));
        }
        for (j9 = 0; j9 < atuais9.length; j9++) {
          if (atuais9[j9].__grupo) continue;
          cel9 = [];
          for (k9 = 0; k9 < cols9.length; k9++) cel9.push(tsvCampo(textoCelula(cols9[k9], atuais9[j9][cols9[k9].campo], atuais9[j9]), opts9.aspas));
          linhas9.push(cel9.join("\t"));
        }
        var t9 = linhas9.join("\r\n");
        log("copia", { linhas: linhas9.length - (opts9.cabecalho !== false ? 1 : 0), origem: "pagina" });
        return t9;
      },
      copiarParaAreaTransferencia: function (opts9) {
        var tsv9 = api.selecionadas().length ? api.copiarSelecao(opts9) : api.copiarPagina(opts9);
        try {
          var nav9 = (typeof window !== "undefined" && window.navigator) ? window.navigator : (typeof navigator !== "undefined" ? navigator : null);
          if (nav9 && nav9.clipboard && nav9.clipboard.writeText) { nav9.clipboard.writeText(tsv9); log("copia-area", { bytes: tsv9.length }); return api; }
        } catch (e9) { /* segue p/ o fallback */ }
        try {
          var ta9 = document.createElement("textarea");
          ta9.value = tsv9;
          ta9.style.cssText = "position:fixed;left:-9999px;top:0";
          document.body.appendChild(ta9);
          ta9.select();
          document.execCommand("copy");
          document.body.removeChild(ta9);
        } catch (e8) { log("copiar-falhou", { motivo: String(e8.message || e8) }); }
        return api;
      },
      colarTSV: function (texto9, opts9) {
        opts9 = opts9 || {};
        var rel9 = { linhas: 0, celulas: 0, criadas: 0, ignoradas: 0, erros: [] };
        var mat9 = parseTSV(texto9);
        if (!mat9.length) return rel9;
        var v9 = visiveis(), j9, k9;
        function editaveis9(desde9) {
          var out8 = [], m8;
          for (m8 = desde9; m8 < v9.length; m8++) if (v9[m8].editavel && v9[m8].formula == null && !v9[m8].acumulado) out8.push(v9[m8]);
          return out8;
        }
        /* mapeamento de colunas: explicito > cabecalho reconhecido > a partir do foco */
        var cols9 = null, pulaCab9 = false;
        if (opts9.campos) {
          cols9 = [];
          for (j9 = 0; j9 < opts9.campos.length; j9++) if (porCampo[opts9.campos[j9]]) cols9.push(porCampo[opts9.campos[j9]]);
        } else {
          var prim9 = mat9[0], casou9 = [], achou9 = 0;
          for (j9 = 0; j9 < prim9.length; j9++) {
            var alvoC9 = null;
            for (k9 = 0; k9 < v9.length; k9++) {
              if (semAcento(String(v9[k9].titulo || v9[k9].campo)).toLowerCase() === semAcento(String(prim9[j9])).toLowerCase() ||
                  v9[k9].campo === prim9[j9]) { alvoC9 = v9[k9]; break; }
            }
            casou9.push(alvoC9);
            if (alvoC9) achou9++;
          }
          if (prim9.length && achou9 === prim9.length) { cols9 = casou9; pulaCab9 = true; }
          else cols9 = editaveis9(opts9.desdeColuna != null ? opts9.desdeColuna : focoC);
        }
        if (!cols9 || !cols9.length) { rel9.erros.push({ motivo: "nenhuma coluna alvo" }); return rel9; }
        var dados9 = pulaCab9 ? mat9.slice(1) : mat9;
        var atuais9 = (ultimaCarga && ultimaCarga.linhas) || [];
        var ixIni9 = opts9.desdeLinha != null ? opts9.desdeLinha : focoR;
        var i9, linhaAlvo9, valTxt9, colAlvo9, p9, chE9, novas9 = [];
        for (i9 = 0; i9 < dados9.length; i9++) {
          linhaAlvo9 = atuais9[ixIni9 + i9];
          if (linhaAlvo9 && linhaAlvo9.__grupo) { rel9.ignoradas++; continue; }
          if (!linhaAlvo9) {
            if (temLancamento && opts9.criar !== false) {
              var base8 = typeof lancCfg.modelo === "function" ? lancCfg.modelo() : (lancCfg.modelo || {});
              var nova8 = {}, kb8;
              for (kb8 in base8) nova8[kb8] = base8[kb8];
              if (chaveCampo && nova8[chaveCampo] == null) nova8[chaveCampo] = proximaChaveLanc();
              nova8.__novo = true;
              novas9.push(nova8);
              linhaAlvo9 = nova8;
              rel9.criadas++;
            } else { rel9.ignoradas++; continue; }
          }
          if (!linhaAlvo9) { rel9.ignoradas++; continue; }
          rel9.linhas++;
          for (k9 = 0; k9 < dados9[i9].length && k9 < cols9.length; k9++) {
            colAlvo9 = cols9[k9];
            if (!colAlvo9) { rel9.ignoradas++; continue; }
            if (colAlvo9.formula != null || colAlvo9.acumulado) { rel9.ignoradas++; continue; }
            if (opts9.somenteEditaveis !== false && colAlvo9.editavel !== true) { rel9.ignoradas++; continue; }
            if (regrasAcesso && !acessoDe(colAlvo9.campo).editar) { rel9.erros.push({ linha: ixIni9 + i9, campo: colAlvo9.campo, motivo: "acesso negado" }); continue; }
            valTxt9 = dados9[i9][k9];
            if (valTxt9 === "" ) { rel9.ignoradas++; continue; }
            p9 = parseEntrada(colAlvo9, valTxt9);
            if (!p9.ok) { rel9.erros.push({ linha: ixIni9 + i9, campo: colAlvo9.campo, valor: valTxt9, motivo: "valor inv\u00e1lido para " + (colAlvo9.tipo || "texto") }); continue; }
            chE9 = chaveDe(linhaAlvo9, ixIni9 + i9);
            if (aoEditar && aoEditar(chE9, colAlvo9.campo, p9.valor, linhaAlvo9[colAlvo9.campo], linhaAlvo9) === false) {
              rel9.erros.push({ linha: ixIni9 + i9, campo: colAlvo9.campo, valor: valTxt9, motivo: "recusado pela aplica\u00e7\u00e3o" });
              continue;
            }
            registraHist(chE9, colAlvo9.campo, linhaAlvo9[colAlvo9.campo], p9.valor);
            linhaAlvo9[colAlvo9.campo] = p9.valor;
            rel9.celulas++;
          }
          materializaFormulas([linhaAlvo9]);
          if (temLancamento) validaLinhaLanc(linhaAlvo9, true);
        }
        if (novas9.length) {
          api.anexarDados(novas9);
          log("lanc-lote", { n: novas9.length, origem: "colar" });
        }
        if (fonte.invalidar) fonte.invalidar(null);
        carrega(null, true);
        log("colar", { linhas: rel9.linhas, celulas: rel9.celulas, criadas: rel9.criadas, ignoradas: rel9.ignoradas, erros: rel9.erros.length });
        return rel9;
      },
      abrirMenuContexto: function (ctx9, x9, y9) {
        if (cfg.menuContexto === false) return api;
        api.fecharMenuContexto();
        var itens9 = [], campo9 = ctx9.campo, col9 = campo9 ? porCampo[campo9] : null;
        function it(rot, fn, sep) { itens9.push({ rotulo: rot, fn: fn, sep: !!sep }); }
        if (col9) {
          it("\u25b2 Ordenar crescente", function () { api.ordenar(campo9, "asc"); });
          it("\u25bc Ordenar decrescente", function () { api.ordenar(campo9, "desc"); });
          it("\u2795 Somar na cascata", function () { api.adicionarOrdem(campo9, "asc"); });
          it("\uD83D\uDCCC " + (col9.fixa === "esq" ? "Soltar coluna" : "Fixar \u00e0 esquerda"), function () { api.fixarColuna(campo9, col9.fixa === "esq" ? false : "esq"); }, true);
          it("\uD83D\uDC41 Ocultar coluna", function () { api.ocultar(campo9, true); });
        }
        if (ctx9.linha) {
          it("\u2702 Copiar c\u00e9lula", function () {
            var v9 = ctx9.linha[campo9];
            if (regrasAcesso) v9 = aplicaMascara(col9 || {}, v9, ctx9.linha);
            api.copiarTexto(String(v9 == null ? "" : v9));
          }, true);
          it("\u2702 Copiar linha (TSV)", function () {
            var v8 = visiveis(), p8 = [], j8;
            for (j8 = 0; j8 < v8.length; j8++) p8.push(tsvCampo(textoCelula(v8[j8], regrasAcesso ? aplicaMascara(v8[j8], ctx9.linha[v8[j8].campo], ctx9.linha) : ctx9.linha[v8[j8].campo], ctx9.linha), false));
            api.copiarTexto(p8.join("\t"));
          });
          if (temHistCel && chaveCampo) it("\u23f1 Hist\u00f3rico da c\u00e9lula", function () { api.abrirHistorico(chaveDe(ctx9.linha, ctx9.ixL), campo9, null); });
        }
        it("\u2b07 Exportar CSV", function () { api.baixarCSV(); }, true);
        it("\uD83D\uDCD7 Exportar XLSX", function () { api.baixarXLSX(); });
        var extras9 = cfg.menuContexto instanceof Array ? cfg.menuContexto : [];
        var j9;
        for (j9 = 0; j9 < extras9.length; j9++) (function (ex) {
          it(ex.rotulo, function () {
            if (typeof ex.acao === "function") { try { ex.acao({ campo: campo9, linha: ctx9.linha, chave: ctx9.linha ? chaveDe(ctx9.linha, ctx9.ixL) : null }); } catch (e9) {} }
            else if (ex.fn && typeof root[ex.fn] === "function") { try { root[ex.fn](JSON.stringify({ campo: campo9, linha: ctx9.linha, chave: ctx9.linha ? chaveDe(ctx9.linha, ctx9.ixL) : null })); } catch (e9) {} }
          }, j9 === 0);
        })(extras9[j9]);
        var m9 = document.createElement("div");
        m9.className = "phx-ctx";
        var h9 = [], j8b;
        for (j8b = 0; j8b < itens9.length; j8b++) h9.push('<div class="phx-ctx-item' + (itens9[j8b].sep ? " phx-ctx-sep" : "") + '" data-ci="' + j8b + '">' + esc(itens9[j8b].rotulo) + "</div>");
        m9.innerHTML = h9.join("");
        m9.style.left = (x9 || 0) + "px";
        m9.style.top = (y9 || 0) + "px";
        wrap.appendChild(m9);
        var its9 = m9.querySelectorAll(".phx-ctx-item"), k9;
        for (k9 = 0; k9 < its9.length; k9++) (function (no9) {
          no9.addEventListener("click", function (ev9) {
            ev9.stopPropagation();
            var idx9 = parseInt(no9.getAttribute("data-ci"), 10);
            api.fecharMenuContexto();
            log("ctx-acao", { rotulo: itens9[idx9].rotulo, campo: campo9 || null });
            itens9[idx9].fn();
          });
        })(its9[k9]);
        _menuCtx = m9;
        setTimeout(function () { document.addEventListener("click", _fechaCtxGlobal, true); }, 0);
        log("ctx-abre", { campo: campo9 || null, itens: itens9.length });
        return api;
      },
      fecharMenuContexto: function () {
        if (!_menuCtx) return api;
        if (_menuCtx.parentNode) _menuCtx.parentNode.removeChild(_menuCtx);
        _menuCtx = null;
        document.removeEventListener("click", _fechaCtxGlobal, true);
        return api;
      },
      copiarTexto: function (txt9) {
        var nav9 = (typeof window !== "undefined" && window.navigator) ? window.navigator : null;
        if (nav9 && nav9.clipboard && nav9.clipboard.writeText) { try { nav9.clipboard.writeText(txt9); } catch (e9) {} }
        log("copia-texto", { bytes: String(txt9).length });
        return api;
      },
      adaptar: function (larguraForcada9) {
        if (!adaptativo) return api;
        var disp9 = larguraForcada9 || envoltorio.clientWidth || 0;
        if (!disp9) return api;
        var antes9 = [], k9;
        for (k9 in ocultasAdapt) antes9.push(k9);
        ocultasAdapt = {};
        var cands9 = [], j9, c9, soma9 = 0;
        for (j9 = 0; j9 < ordemColunas.length; j9++) {
          c9 = porCampo[ordemColunas[j9]];
          if (ocultas[c9.campo] || agrupada(c9.campo)) continue;
          if (regrasAcesso && acessoDe(c9.campo).oculta) continue;
          soma9 += larguraDe(c9);
          cands9.push(c9);
        }
        var minP9 = adaptCfg.minimoVisivel || 2;
        cands9.sort(function (a, b) { return (b.prioridade == null ? 5 : b.prioridade) - (a.prioridade == null ? 5 : a.prioridade); });
        var i9 = 0;
        while (soma9 > disp9 && cands9.length - i9 > minP9) {
          var alvo9 = cands9[i9++];
          if (alvo9.fixa) continue;
          ocultasAdapt[alvo9.campo] = true;
          soma9 -= larguraDe(alvo9);
        }
        var depois9 = [], k8;
        for (k8 in ocultasAdapt) depois9.push(k8);
        if (antes9.sort().join(",") !== depois9.sort().join(",")) {
          invalidaJanela();
          montaHeader();
          carrega(null, true);
          log("adaptar", { largura: disp9, ocultadas: depois9.join(",") || null });
        }
        return api;
      },
      ocultasAdaptativo: function () { var o9 = [], k9; for (k9 in ocultasAdapt) o9.push(k9); return o9; },
      setPapel: function (papel9) {
        papelAtual = papel9 || null;
        invalidaJanela();
        montaHeader();
        carrega(null, true);
        log("papel", { papel: papelAtual });
        return api;
      },
      papel: function () { return papelAtual; },
      podeVer: function (campo9) { return !acessoDe(campo9).oculta; },
      podeEditar: function (campo9) { return acessoDe(campo9).editar; },
      acessoDe: function (campo9) { return acessoDe(campo9); },
      historicoDe: function (chave9, campo9) {
        var k9 = String(chave9) + "\u0000" + campo9;
        return histCel[k9] ? JSON.parse(JSON.stringify(histCel[k9])) : [];
      },
      historicoTudo: function () {
        var out9 = [], k9, j9;
        for (k9 in histCel) for (j9 = 0; j9 < histCel[k9].length; j9++) out9.push(histCel[k9][j9]);
        out9.sort(function (a, b) { return a.quando < b.quando ? -1 : (a.quando > b.quando ? 1 : 0); });
        return out9;
      },
      carregarHistorico: function (lista9) {
        var j9, e9, k9;
        for (j9 = 0; j9 < (lista9 || []).length; j9++) {
          e9 = lista9[j9];
          if (!e9 || e9.chave == null || !e9.campo) continue;
          k9 = String(e9.chave) + "\u0000" + e9.campo;
          if (!histCel[k9]) histCel[k9] = [];
          histCel[k9].push({ chave: String(e9.chave), campo: e9.campo, de: e9.de == null ? null : e9.de, para: e9.para == null ? null : e9.para, quando: e9.quando || new Date().toISOString(), usuario: e9.usuario || null });
          while (histCel[k9].length > histMax) histCel[k9].shift();
        }
        carrega(null, true);
        log("hist-carregado", { entradas: (lista9 || []).length });
        return api;
      },
      limparHistorico: function () { histCel = {}; carrega(null, true); return api; },
      abrirHistorico: function (chave9, campo9, ancora9) {
        var lista9 = api.historicoDe(chave9, campo9);
        var col9 = porCampo[campo9];
        var h9 = ['<div class="phx-hist-tit">' + esc((col9 && col9.titulo) || campo9) + " \u00b7 " + esc(chave9) + "</div>"];
        if (!lista9.length) h9.push('<div class="phx-hist-vazio">sem altera\u00e7\u00f5es</div>');
        var j9;
        for (j9 = lista9.length - 1; j9 >= 0; j9--) {
          var e9 = lista9[j9];
          h9.push('<div class="phx-hist-item"><div class="phx-hist-quando">' + esc(String(e9.quando).slice(0, 16).replace("T", " ")) +
            (e9.usuario ? " \u00b7 " + esc(e9.usuario) : "") + "</div>" +
            '<div class="phx-hist-vals"><span class="phx-hist-de">' + esc(textoCelula(col9 || {}, e9.de, {})) + '</span> \u2192 <span class="phx-hist-para">' + esc(textoCelula(col9 || {}, e9.para, {})) + "</span></div></div>");
        }
        var pop9 = document.createElement("div");
        pop9.className = "phx-hist-pop";
        pop9.innerHTML = h9.join("");
        (ancora9 && ancora9.parentNode ? ancora9.parentNode : wrap).appendChild(pop9);
        function fecha9() { if (pop9.parentNode) pop9.parentNode.removeChild(pop9); document.removeEventListener("click", fecha9, true); }
        setTimeout(function () { document.addEventListener("click", fecha9, true); }, 0);
        log("hist-abre", { chave: String(chave9), campo: campo9, entradas: lista9.length });
        return api;
      },
      ajustarColunas: function (opts9) {
        opts9 = opts9 || {};
        var modo9 = opts9.modo || autoLarguraModo || "conteudo";
        var larg9 = calculaLarguras(opts9);
        var res9 = { modo: modo9, sobra: 0, coube: false };
        if (modo9 === "preencher") {
          var d9 = distribuiPreencher(larg9);
          larg9 = d9.larguras; res9.sobra = d9.sobra; res9.coube = d9.coube;
        }
        res9.colunas = aplicaLarguras(larg9, modo9);
        res9.larguras = larg9;
        log("auto-largura", { modo: modo9, colunas: res9.colunas, sobra: Math.round(res9.sobra) });
        return res9;
      },
      ajustarColuna: function (campo9, opts9) {
        if (!porCampo[campo9]) return api;
        opts9 = opts9 || {};
        var so9 = {}, larg9 = calculaLarguras({ amostra: opts9.amostra, forcar: true });
        so9[campo9] = larg9[campo9];
        if (so9[campo9] == null) return api;
        larguraFixada[campo9] = false;
        aplicaLarguras(so9);
        log("auto-largura", { campo: campo9, largura: so9[campo9], origem: "coluna" });
        return api;
      },
      larguraAuto: function (campo9, auto9) {
        if (!porCampo[campo9]) return api;
        if (auto9 === undefined) return !larguraFixada[campo9];
        larguraFixada[campo9] = !auto9;
        log("largura-modo", { campo: campo9, modo: auto9 ? "auto" : "fixa" });
        agendaLayoutCb();
        return api;
      },
      larguras: function () {
        var out9 = [], j9, c9;
        for (j9 = 0; j9 < ordemColunas.length; j9++) {
          c9 = porCampo[ordemColunas[j9]];
          out9.push({ campo: c9.campo, largura: c9.largura || null, auto: !larguraFixada[c9.campo], pino: c9.fixa || null });
        }
        return out9;
      },
      alternarPino: function (campo9) {
        var c9 = porCampo[campo9];
        if (!c9) return api;
        var prox9 = c9.fixa === "esq" ? "dir" : (c9.fixa === "dir" ? false : "esq");
        return api.fixarColuna(campo9, prox9);
      },
      salvarLayoutAgora: function (motivo9) {
        var l9 = api.layout();
        if (aoMudarLayout) { try { aoMudarLayout(l9, motivo9 || "manual"); } catch (e9) {} }
        if (cfg.layout && typeof cfg.layout.salvar === "function") { try { cfg.layout.salvar(l9, motivo9 || "manual"); } catch (e9) {} }
        log("layout-salvo", { motivo: motivo9 || "manual", colunas: l9.colunas.length });
        return l9;
      },
      registroAtual: function () { return registroAtual; },
      cabecalhoRelevo: function (modo9) { if (modo9 === undefined) return wrap.getAttribute("data-cab-relevo") || "plano"; if (modo9 && modo9 !== "plano") wrap.setAttribute("data-cab-relevo", modo9); else wrap.removeAttribute("data-cab-relevo"); log("cabecalho-relevo", { modo: modo9 || "plano" }); return api; },
      alturaLinha: function (px9) {
        if (px9 == null) { var v9 = parseFloat(wrap.style.getPropertyValue("--phx-pad-y")) ; return isNaN(v9) ? null : Math.round(v9 * 2 + 20); }
        var pad9 = Math.max(1, Math.round((Number(px9) - 20) / 2));
        wrap.style.setProperty("--phx-pad-y", pad9 + "px");
        _alturaLinha = Number(px9);
        log("altura-linha", { px: Number(px9) });
        agendaLayoutCb();
        return api;
      },
      celulaSelecionada: function () { return celulaSel ? JSON.parse(JSON.stringify(celulaSel)) : null; },
      imprimir: function (opts9) {
        opts9 = opts9 || {};
        var html9 = api.gerarRelatorioHTML({ titulo: opts9.titulo, subtitulo: opts9.subtitulo });
        var extra9 = "<style>@page{size:" + (opts9.orientacao === "retrato" ? "portrait" : "landscape") + ";margin:" + (opts9.margem || "12mm") + "}" +
          "thead{display:table-header-group}tfoot{display:table-footer-group}tr{page-break-inside:avoid}" +
          (opts9.encolher ? "table{table-layout:auto!important;width:100%!important}col,th,td{width:auto!important}" : "") +
          (opts9.tonsDeCinza ? "@media print{*{color:#000!important;background:#fff!important;filter:grayscale(100%)}}" : "") +
          (opts9.cabecalho ? ".phx-imp-cab{position:running(cab)}" : "") + "</style>" +
          (opts9.cabecalho ? '<div class="phx-imp-cab" style="font:11px sans-serif;color:#57606a;margin-bottom:6px">' + esc(opts9.cabecalho) + "</div>" : "") +
          (opts9.rodape ? '<div style="font:11px sans-serif;color:#57606a;margin-top:6px">' + esc(opts9.rodape) + "</div>" : "");
        html9 = html9.replace("</head>", extra9 + "</head>");
        if (opts9.abrir !== false && typeof window !== "undefined" && window.open) {
          var w9 = window.open("", "_blank");
          if (w9) { w9.document.open(); w9.document.write(html9); w9.document.close(); if (opts9.imprimirAgora !== false) setTimeout(function () { try { w9.focus(); w9.print(); } catch (e9) {} }, 300); }
        }
        log("imprimir", { orientacao: opts9.orientacao || "paisagem", bytes: html9.length });
        return html9;
      },
      irRegistro: function (n9) {
        var tot9 = estado.total || 0;
        n9 = Math.max(1, Math.min(tot9 || 1, Number(n9) || 1));
        registroAtual = tot9 ? n9 : 0;
        var pag9 = Math.floor((n9 - 1) / (estado.tamanho || 1)) + 1;
        if (pag9 !== estado.pagina) api.pagina(pag9); else reRender();
        var l9 = api.linhas()[(n9 - 1) % (estado.tamanho || 1)];
        atualizaNav();
        log("registro", { n: registroAtual, total: tot9, chave: l9 ? chaveDe(l9, 0) : null });
        return api;
      },
      proximoRegistro: function () { return api.irRegistro((registroAtual || 0) + 1); },
      anteriorRegistro: function () { return api.irRegistro((registroAtual || 1) - 1); },
      excluirRegistro: function (chave9) {
        if (cfg.excluir === false || cfg.excluir == null) return { ok: false, erro: "exclus\u00e3o desabilitada (cfg.excluir)" };
        var alvo9 = chave9 != null ? chave9 : (function () { var l9 = api.linhas()[((registroAtual || 1) - 1) % (estado.tamanho || 1)]; return l9 ? chaveDe(l9, 0) : null; })();
        if (alvo9 == null) return { ok: false, erro: "nenhum registro atual" };
        var conf9 = typeof cfg.excluir.confirmar === "function" ? cfg.excluir.confirmar(alvo9) : true;
        if (conf9 === false) { log("excluir", { chave: alvo9, vetado: true }); return { ok: false, erro: "vetado" }; }
        api.removerLinha(alvo9);
        log("excluir", { chave: alvo9 });
        if (typeof cfg.excluir.aoExcluir === "function") { try { cfg.excluir.aoExcluir(alvo9); } catch (e9) {} }
        return { ok: true, chave: alvo9 };
      },
      definirEstilo: function (nome9, def9) {
        if (!nome9 || !/^[A-Za-z_][A-Za-z0-9_-]*$/.test(nome9)) return { ok: false, erro: "nome de estilo inv\u00e1lido" };
        estilosNomeados[nome9] = def9 || {};
        regravaEstilos();
        log("estilo-nomeado", { nome: nome9 });
        return { ok: true, nome: nome9 };
      },
      estilos: function () { return JSON.parse(JSON.stringify(estilosNomeados)); },
      aplicarEstilo: function (campo9, nome9, onde9) {
        var c9 = porCampo[campo9];
        if (!c9) return api;
        if (onde9 === "cabecalho") c9.estiloCabecalho = nome9 || null; else c9.estilo = nome9 || null;
        montaHeader(); reRender();
        return api;
      },
      adicionarCondicao: function (cd9) {
        if (!cd9 || !cd9.campo || !cd9.estilo) return { ok: false, erro: "condi\u00e7\u00e3o precisa de campo e estilo" };
        cd9.id = cd9.id || ("c" + (condicoes.length + 1));
        condicoes.push(cd9);
        reRender();
        log("condicao", { id: cd9.id, campo: cd9.campo, op: cd9.op || "=", estilo: cd9.estilo });
        return { ok: true, id: cd9.id };
      },
      removerCondicao: function (id9) { var n9 = [], j9; for (j9 = 0; j9 < condicoes.length; j9++) if (condicoes[j9].id !== id9) n9.push(condicoes[j9]); condicoes = n9; reRender(); return api; },
      condicoes: function () { return JSON.parse(JSON.stringify(condicoes)); },
      Columns: (function () {
        function item9(i9) {
          var campo9 = typeof i9 === "number" ? ordemColunas[i9 - 1] : String(i9);
          var c9 = porCampo[campo9];
          if (!c9) return null;
          var o9 = { Key: c9.campo, DataField: c9.campo, Index: ordemColunas.indexOf(c9.campo) + 1 };
          Object.defineProperty(o9, "Caption", { get: function () { return c9.titulo || c9.campo; }, set: function (v) { c9.titulo = String(v); montaHeader(); }, enumerable: true });
          Object.defineProperty(o9, "Visible", { get: function () { return !ocultas[c9.campo]; }, set: function (v) { api.ocultar(c9.campo, !v); }, enumerable: true });
          Object.defineProperty(o9, "Width", { get: function () { return c9.largura || null; }, set: function (v) { c9.largura = Number(v) || null; larguraFixada[c9.campo] = true; montaHeader(); reRender(); }, enumerable: true });
          Object.defineProperty(o9, "ColPosition", { get: function () { return ordemColunas.indexOf(c9.campo) + 1; }, set: function (v) { var alvo9 = ordemColunas[Math.max(0, Math.min(ordemColunas.length - 1, Number(v) - 1))]; if (alvo9 && alvo9 !== c9.campo) moverColunaAntes(c9.campo, alvo9); }, enumerable: true });
          Object.defineProperty(o9, "CellStyle", { get: function () { return c9.estilo || null; }, set: function (v) { api.aplicarEstilo(c9.campo, v || null); }, enumerable: true });
          Object.defineProperty(o9, "HeaderStyle", { get: function () { return c9.estiloCabecalho || null; }, set: function (v) { api.aplicarEstilo(c9.campo, v || null, "cabecalho"); }, enumerable: true });
          Object.defineProperty(o9, "AllowSizing", { get: function () { return !larguraFixada[c9.campo]; }, set: function (v) { larguraFixada[c9.campo] = !v; }, enumerable: true });
          Object.defineProperty(o9, "Frozen", { get: function () { return c9.fixa || null; }, set: function (v) { api.fixarColuna(c9.campo, v || false); }, enumerable: true });
          return o9;
        }
        var col9 = { Item: item9 };
        Object.defineProperty(col9, "Count", { get: function () { return ordemColunas.length; }, enumerable: true });
        return col9;
      })(),
      Columns_Count: function () { return ordemColunas.length; },
      Columns_Item: function (i9) { var o9 = api.Columns.Item(i9); if (!o9) return null; return { Key: o9.Key, Caption: o9.Caption, Visible: o9.Visible, Width: o9.Width, ColPosition: o9.ColPosition, CellStyle: o9.CellStyle, Frozen: o9.Frozen, Index: o9.Index }; },
      Columns_Set: function (i9, props9) { var o9 = api.Columns.Item(i9); if (!o9) return { ok: false, erro: "coluna inexistente: " + i9 }; var k9; for (k9 in (props9 || {})) if (k9 in o9 && k9 !== "Key" && k9 !== "Index" && k9 !== "DataField") o9[k9] = props9[k9]; return { ok: true, coluna: api.Columns_Item(i9) }; },
      FormatStyles: (function () {
        var fs9 = { Item: function (n9) { return estilosNomeados[n9] ? { Name: n9, def: estilosNomeados[n9] } : null; }, Add: function (n9, d9) { return api.definirEstilo(n9, d9); } };
        Object.defineProperty(fs9, "Count", { get: function () { var n = 0, k; for (k in estilosNomeados) n++; return n; }, enumerable: true });
        return fs9;
      })(),
      FormatStyles_Set: function (nome9, def9) { return api.definirEstilo(nome9, def9); },
      RecordSource: function (linhas9) { if (linhas9) api.substituirDados(typeof linhas9 === "string" ? JSON.parse(linhas9) : linhas9); return api; },
      Rebind: function () { if (fonte.invalidar) fonte.invalidar(null); montaHeader(); carrega(null, true); montaColSel(); return api; },
      SaveLayout: function (slot9) {
        slot9 = String(slot9 == null ? 1 : slot9);
        var l9 = api.layout();
        l9.estilos = api.estilos(); l9.condicoes = api.condicoes(); l9.estilo = api.estiloAtual();
        _slotsLayout[slot9] = JSON.parse(JSON.stringify(l9));
        if (cfg.layout && typeof cfg.layout.salvarSlot === "function") { try { cfg.layout.salvarSlot(slot9, _slotsLayout[slot9]); } catch (e9) {} }
        log("layout-slot", { acao: "salvar", slot: slot9 });
        return { ok: true, slot: slot9, layout: _slotsLayout[slot9] };
      },
      LoadLayout: function (slot9, layout9) {
        slot9 = String(slot9 == null ? 1 : slot9);
        var l9 = layout9 || _slotsLayout[slot9] || (cfg.layout && typeof cfg.layout.lerSlot === "function" ? cfg.layout.lerSlot(slot9) : null);
        if (typeof l9 === "string") { try { l9 = JSON.parse(l9); } catch (e9) { return { ok: false, erro: "layout inv\u00e1lido" }; } }
        if (!l9) return { ok: false, erro: "slot vazio: " + slot9 };
        var k9;
        if (l9.estilos) { for (k9 in l9.estilos) estilosNomeados[k9] = l9.estilos[k9]; regravaEstilos(); }
        if (l9.condicoes) condicoes = JSON.parse(JSON.stringify(l9.condicoes));
        if (l9.estilo) { api.limparEstilo(); api.estilizar(l9.estilo); }
        api.aplicarLayout(l9);
        log("layout-slot", { acao: "ler", slot: slot9 });
        return { ok: true, slot: slot9 };
      },
      fixarColuna: function (campo9, lado9) {
        var c9 = porCampo[campo9];
        if (!c9) return api;
        if (lado9 === false || lado9 == null || lado9 === "nao") delete c9.fixa;
        else c9.fixa = lado9 === "dir" ? "dir" : "esq";
        invalidaJanela();
        montaHeader();
        reRender();
        mideFixas();
        if (!larguraFixada[campo9]) api.ajustarColuna(campo9);
        agendaLayoutCb();
        log("fixar", { campo: campo9, lado: c9.fixa || null });
        return api;
      },
      colunasFixas: function () {
        var out9 = [], j9, c9;
        for (j9 = 0; j9 < ordemColunas.length; j9++) {
          c9 = porCampo[ordemColunas[j9]];
          if (c9.fixa) out9.push({ campo: c9.campo, lado: c9.fixa });
        }
        return out9;
      },
      janelaColunas: function () {
        if (!virtColOn) return { virtual: false, total: visiveis().length };
        calcJanelaCol();
        return { virtual: true, ini: janIni, fim: janFim, espacoEsq: janEspE, espacoDir: janEspD, renderizadas: visiveisJanela().length, total: visiveis().length };
      },
      incluirLinha: function (modelo9) {
        if (!modelo9) {
          var temPadrao9 = false, pj8; for (pj8 = 0; pj8 < colunasDef.length; pj8++) if (colunasDef[pj8].valorPadrao !== undefined) temPadrao9 = true;
          if (temPadrao9 && !(cfg.lancamento && cfg.lancamento.modelo)) {
            modelo9 = {}; var pk8; for (pk8 = 0; pk8 < colunasDef.length; pk8++) if (colunasDef[pk8].valorPadrao !== undefined) modelo9[colunasDef[pk8].campo] = colunasDef[pk8].valorPadrao;
          }
        }
        if (cfg.lancamento && cfg.lancamento.posicao === "inicio" && fonte.local && fonte.todos && !modelo9) {
          var nova9 = typeof cfg.lancamento.modelo === "function" ? cfg.lancamento.modelo() : {};
          if (chaveCampo && nova9[chaveCampo] == null) nova9[chaveCampo] = "novo-" + (++lancSeq);
          materializaFormulas([nova9]);
          fonte.todos.unshift(nova9);
          if (fonte.invalidar) fonte.invalidar(null);
          estado.pagina = 1;
          carrega(null, true);
          log("lanc-inclui", { chave: nova9[chaveCampo], posicao: "inicio" });
          return nova9;
        }
        if (!temLancamento) { log("aviso", { motivo: "cfg.lancamento desligado" }); return api; }
        var base9 = typeof lancCfg.modelo === "function" ? lancCfg.modelo() : (lancCfg.modelo || {});
        var nova9 = {}, k9;
        for (k9 in base9) nova9[k9] = base9[k9];
        for (k9 in (modelo9 || {})) nova9[k9] = modelo9[k9];
        if (chaveCampo && nova9[chaveCampo] == null) nova9[chaveCampo] = proximaChaveLanc();
        nova9.__novo = true;
        api.anexarDados([nova9]);
        var totalD9 = (ultimaCarga && (ultimaCarga.totalDados != null ? ultimaCarga.totalDados : ultimaCarga.total)) || 1;
        var ultPag9 = Math.max(1, Math.ceil(totalD9 / estado.tamanho));
        if (estado.pagina !== ultPag9) api.pagina(ultPag9);
        log("lanc-nova", { chave: chaveDe(nova9, -1) });
        var ixNova9 = ultimaCarga.linhas.length - 1;
        abreProxima(ixNova9, "\u0000", 1);
        return api;
      },
      incluirLote: function (n9, modelo9) {
        if (!temLancamento) return api;
        var lote9 = [], j9;
        var base9 = typeof lancCfg.modelo === "function" ? null : (lancCfg.modelo || {});
        for (j9 = 0; j9 < (n9 || 1); j9++) {
          var b9 = base9 || (typeof lancCfg.modelo === "function" ? lancCfg.modelo() : {});
          var nv9 = {}, k9;
          for (k9 in b9) nv9[k9] = b9[k9];
          for (k9 in (modelo9 || {})) nv9[k9] = modelo9[k9];
          if (chaveCampo && nv9[chaveCampo] == null) nv9[chaveCampo] = proximaChaveLanc();
          nv9.__novo = true;
          lote9.push(nv9);
        }
        api.anexarDados(lote9);
        log("lanc-lote", { n: lote9.length });
        return api;
      },
      removerLinha: function (ch9) {
        ch9 = String(ch9);
        var td9 = fonte.local && fonte.todos ? fonte.todos : [];
        var j9, achou9 = -1;
        for (j9 = 0; j9 < td9.length; j9++) if (String(td9[j9][chaveCampo]) === ch9) { achou9 = j9; break; }
        if (achou9 < 0) return api;
        td9.splice(achou9, 1);
        delete lancInvalidas[ch9];
        if (fonte.invalidar) fonte.invalidar(null);
        carrega(null, true);
        log("lanc-remove", { chave: ch9 });
        return api;
      },
      linhasInvalidas: function () {
        var out9 = [], k9;
        for (k9 in lancInvalidas) out9.push({ chave: k9, motivo: lancInvalidas[k9] });
        return out9;
      },
      validarLinha: function (ch9) {
        var td9 = fonte.local && fonte.todos ? fonte.todos : [];
        for (var j9 = 0; j9 < td9.length; j9++) if (String(td9[j9][chaveCampo]) === String(ch9)) return validaLinhaLanc(td9[j9]);
        return false;
      },
      alternarNo: function (k9) {
        k9 = String(k9);
        var atual9 = (k9 in abertosArv) ? !!abertosArv[k9] : null;
        abertosArv[k9] = atual9 === null ? !(_nivelDe(k9) < inicioAbertoRt) : !atual9;
        log("arvore", { chave: k9, aberto: abertosArv[k9] });
        carrega(null, true);
        return api;
      },
      expandirNo: function (k9) { abertosArv[String(k9)] = true; carrega(null, true); return api; },
      recolherNo: function (k9) { abertosArv[String(k9)] = false; carrega(null, true); return api; },
      expandirTudoArvore: function (niveis9) {
        abertosArv = {};
        inicioAbertoRt = niveis9 == null ? 999 : niveis9;
        carrega(null, true);
        return api;
      },
      recolherTudoArvore: function () {
        abertosArv = {};
        inicioAbertoRt = 0;
        carrega(null, true);
        return api;
      },
      nivelDe: function (k9) { return _nivelDe(String(k9)); },
      caminhoDe: function (k9) {
        var out9 = [], atual9 = String(k9), guard9 = 0;
        var idx9 = _idxArv();
        while (idx9[atual9] && guard9++ < 200) {
          var pv9 = idx9[atual9][cfg.arvore.pai];
          if (pv9 == null || pv9 === "") break;
          atual9 = String(pv9);
          if (!idx9[atual9]) break;
          out9.unshift(atual9);
        }
        return out9;
      },
      filhosDe: function (k9) {
        k9 = String(k9);
        var out9 = [], idx9 = _idxArv(), kk9;
        for (kk9 in idx9) {
          var pv9 = idx9[kk9][cfg.arvore.pai];
          if (pv9 != null && String(pv9) === k9) out9.push(kk9);
        }
        return out9;
      },
      agrupar: function (campos) {
        if (temArvore) { log("aviso", { motivo: "agrupar ignorado: grid em modo \u00e1rvore" }); return api; }
        agendaLayoutCb();
        var t1 = agora();
        grupos = (campos || []).slice();
        recolhidos = {};
        estado.pagina = 1;
        renderGroupBox();
        carrega(function () {
          if (cfg.gruposRecolhidos && grupos.length) {
            var gj9, ug9 = (ultimaCarga && ultimaCarga.linhas) || [];
            for (gj9 = 0; gj9 < ug9.length; gj9++) if (ug9[gj9].__grupo) recolhidos[ug9[gj9].__grupo.path] = true;
            if (ug9.length) { carrega(null, true); }
          }
          log("group", { campos: grupos.slice(), total: estado.total, ms: Math.round((agora() - t1) * 10) / 10 });
        });
        return api;
      },
      grupos: function () { return grupos.slice(); },
      expandirGrupo: function (path, abrir) {
        if (abrir === false) recolhidos[path] = true;
        else delete recolhidos[path];
        carrega(null, true);
        return api;
      },
      expandirTodos: function (abrir) {
        if (abrir === false && fonte.worker && fonte.paths) {
          var tipos9 = {}, j9c;
          for (j9c = 0; j9c < colunasDef.length; j9c++) tipos9[colunasDef[j9c].campo] = colunasDef[j9c].tipo || "texto";
          fonte.paths(serializaFiltros(), grupos.slice(), tipos9, function (paths9) {
            recolhidos = {};
            var j9d;
            for (j9d = 0; j9d < paths9.length; j9d++) recolhidos[paths9[j9d]] = true;
            carrega(null, true);
          });
          return api;
        }
        if (abrir === false) {
          var lst = serializaFiltros(), base2 = aplicaFiltros(fonte.todos || [], lst);
          var arvT = agrupa(base2, grupos, [], function (c9) { return (porCampo[c9] && porCampo[c9].tipo) || "texto"; }, cfg.condicoesGrupo);
          recolhidos = {};
          (function marca(nos) { var j3; for (j3 = 0; j3 < nos.length; j3++) { recolhidos[nos[j3].path] = true; if (nos[j3].filhos) marca(nos[j3].filhos); } })(arvT);
        } else recolhidos = {};
        carrega(null, true);
        return api;
      },
      buscar: function (termo, opts) {
        var t1 = agora();
        var modo = (opts && opts.modo) || "texto";
        if (modo === "semantica" && cfg.buscaSemantica) {
          var lst = serializaFiltros(), semB = [], j7;
          for (j7 = 0; j7 < lst.length; j7++) if (lst[j7].campo !== "*") semB.push(lst[j7]);
          cfg.buscaSemantica(termo, fonte.local ? aplicaFiltros(fonte.todos, semB) : [], function (ordenadas) {
            ultimaCarga = { linhas: ordenadas.slice(0, estado.tamanho), total: ordenadas.length, _ms: 0 };
            estado.total = ordenadas.length; estado.pagina = 1;
            reRender(); montaPag();
            log("search", { termo: termo, modo: "semantica", total: estado.total, ms: Math.round((agora() - t1) * 10) / 10 });
          });
          return api;
        }
        api.filtrar("*", termo ? { tipo: "busca", termo: termo, campos: camposBuscaveis() } : null);
        log("search", { termo: termo, modo: "texto", total: estado.total, ms: Math.round((agora() - t1) * 10) / 10 });
        if (buscaEl && buscaEl.value !== termo) buscaEl.value = termo || "";
        return api;
      },
      filtros: function () { return serializaFiltros(); },
      limparFiltros: function () {
        var k5;
        for (k5 in filtros) limpaControleRow(k5);
        filtros = {};
        estado.pagina = 1;
        montaChips();
        carrega(function () { log("filter.clear", { total: estado.total }); }, true);
        return api;
      },
      selecionar: function (chaves, marcar) {
        var lista = Object.prototype.toString.call(chaves) === "[object Array]" ? chaves : [chaves];
        var j2;
        for (j2 = 0; j2 < lista.length; j2++) {
          var k3 = String(lista[j2]);
          if (marcar !== false) {
            if (!selecionadas[k3]) {
              selecionadas[k3] = true; nSel++;
              var iL3, lns3 = ultimaCarga ? ultimaCarga.linhas : [];
              for (iL3 = 0; iL3 < lns3.length; iL3++) if (chaveDe(lns3[iL3], iL3) === k3) { dadosSel[k3] = lns3[iL3]; break; }
            }
          }
          else if (selecionadas[k3]) { delete selecionadas[k3]; nSel--; delete dadosSel[k3]; }
        }
        reRender();
        montaTfoot();
        log("select", { n: nSel, origem: "api" });
        return api;
      },
      selecionadas: function () { var o = [], k3; for (k3 in selecionadas) if (selecionadas[k3]) o.push(k3); return o; },
      limparSelecao: function () { selecionadas = {}; nSel = 0; ancoraSel = -1; dadosSel = {}; reRender(); montaTfoot(); return api; },
      logs: function () { return logs.slice(); },
      _scoreBusca: function (linha) { var f9 = null, l9 = serializaFiltros(), j9; for (j9 = 0; j9 < l9.length; j9++) if (l9[j9].tipo === "busca") f9 = l9[j9]; return f9 ? scoreBusca(linha, f9) : 0; },
      _ultimoPlano: function () { return ultimaCarga ? ultimaCarga._plano : null; },
      _ultimoRender: function () { return { strMs: perfRender.strMs, domMs: perfRender.domMs, mestreMs: perfRender.mestreMs, pagMs: perfRender.pagMs, fonteMs: ultimaCarga && ultimaCarga._ms }; },
      editar: function (chave2, campo2) {
        if (!ultimaCarga) return api;
        var j9, ixL9 = -1;
        for (j9 = 0; j9 < ultimaCarga.linhas.length; j9++)
          if (!ultimaCarga.linhas[j9].__grupo && chaveDe(ultimaCarga.linhas[j9], j9) === String(chave2)) { ixL9 = j9; break; }
        if (ixL9 < 0) return api;
        var vCols = visiveis(), c9 = null;
        for (j9 = 0; j9 < vCols.length; j9++) if (vCols[j9].campo === campo2) { c9 = vCols[j9]; break; }
        if (!c9 || !c9.editavel) return api;
        var dataIx9 = [], j10;
        for (j10 = 0; j10 < ultimaCarga.linhas.length; j10++) if (!ultimaCarga.linhas[j10].__grupo) dataIx9.push(j10);
        var pos = -1;
        for (j10 = 0; j10 < dataIx9.length; j10++) if (dataIx9[j10] === ixL9) { pos = j10; break; }
        var trsD = [], trs9 = tbody.children;
        for (j10 = 0; j10 < trs9.length; j10++) {
          var cls9 = trs9[j10].className;
          if (cls9.indexOf("phx-grupo") >= 0 || cls9.indexOf("phx-detalhe") >= 0) continue;
          trsD.push(trs9[j10]);
        }
        if (pos < 0 || !trsD[pos]) return api;
        var ic9 = -1;
        for (j10 = 0; j10 < vCols.length; j10++) if (vCols[j10].campo === campo2) { ic9 = j10; break; }
        var seen9 = 0, tdE = null;
        for (j10 = 0; j10 < trsD[pos].children.length; j10++) {
          var cls10 = trsD[pos].children[j10].className;
          if (cls10.indexOf("phx-td-sel") >= 0 || cls10.indexOf("phx-td-exp") >= 0) continue;
          if (seen9 === ic9) { tdE = trsD[pos].children[j10]; break; }
          seen9++;
        }
        if (tdE) abreEditor({ td: tdE, ixL: ixL9, col: c9 });
        return api;
      },
      cancelarEdicao: function () { fechaEditor(false); return api; },
      nota: function (chave2, campo2, texto) {
        var k2 = String(chave2);
        var havia = notas[k2] && notas[k2][campo2];
        if (texto == null || texto === "") {
          if (havia) { delete notas[k2][campo2]; log("nota", { chave: k2, campo: campo2, acao: "removida" }); if (aoMudarNota) aoMudarNota(k2, campo2, null); }
        } else {
          if (!notas[k2]) notas[k2] = {};
          notas[k2][campo2] = { texto: String(texto), em: new Date().toISOString() };
          log("nota", { chave: k2, campo: campo2, acao: havia ? "editada" : "criada" });
          if (aoMudarNota) aoMudarNota(k2, campo2, String(texto));
        }
        reRender();
        return api;
      },
      notaDe: function (chave2, campo2) {
        var n2 = notas[String(chave2)];
        return (n2 && n2[campo2] && n2[campo2].texto) || null;
      },
      notas: function () { return notas; },
      configurarVisual: function (conf9) {
        if (conf9 && conf9.cabecalho && conf9.cabecalho.relevo !== undefined) { api.cabecalhoRelevo(conf9.cabecalho.relevo); conf9 = JSON.parse(JSON.stringify(conf9)); delete conf9.cabecalho.relevo; }
        var toks9 = visualParaTokens(conf9), k9, aplicados9 = 0, ignorados9 = [];
        for (k9 in toks9) {
          if (k9.charAt(0) === "!") { ignorados9.push(k9.slice(1)); continue; }
          wrap.style.setProperty(k9, toks9[k9]);
          aplicados9++;
        }
        log("visual", { tokens: aplicados9, ignorados: ignorados9.length ? ignorados9.join(",") : null });
        return { ok: true, aplicados: aplicados9, ignorados: ignorados9 };
      },
      visualAtual: function () {
        var out9 = {}, chave9, tok9, v9;
        for (chave9 in MAPA_VISUAL) {
          tok9 = MAPA_VISUAL[chave9];
          v9 = wrap.style.getPropertyValue(tok9);
          if (!v9 && typeof getComputedStyle === "function") { try { v9 = getComputedStyle(wrap).getPropertyValue(tok9); } catch (e9) { v9 = ""; } }
          v9 = String(v9 || "").replace(/^\s+|\s+$/g, "");
          if (v9) out9[chave9] = v9;
        }
        return out9;
      },
      estilizar: function (mapa) {
        var k9, n9 = 0;
        for (k9 in mapa) {
          if (k9.indexOf("--phx-") === 0) { wrap.style.setProperty(k9, String(mapa[k9])); n9++; }
        }
        log("estilo", { tokens: n9 });
        return api;
      },
      limparEstilo: function () {
        var nomes9 = [], i9, nome9;
        for (i9 = 0; i9 < wrap.style.length; i9++) {
          nome9 = wrap.style[i9];
          if (nome9 && nome9.indexOf("--phx-") === 0) nomes9.push(nome9);
        }
        for (i9 = 0; i9 < nomes9.length; i9++) wrap.style.removeProperty(nomes9[i9]);
        return api;
      },
      estiloAtual: function () {
        var out9 = {}, i9, nome9;
        for (i9 = 0; i9 < wrap.style.length; i9++) {
          nome9 = wrap.style[i9];
          if (nome9 && nome9.indexOf("--phx-") === 0) out9[nome9] = wrap.style.getPropertyValue(nome9);
        }
        return out9;
      },
      gerarDossieHTML: function (opts) {
        opts = opts || {};
        var titulo9 = opts.titulo || "Dossi\u00ea executivo";
        var todos9 = fonte.local && fonte.todos ? fonte.todos : [];
        var rGeral = resumir(todos9, colunasDef, { topN: 3 });
        var lt9 = linhaDoTempo(todos9, colunasDef, {});
        var nomes9 = opts.recortes || (function () { var L9 = api.listarRecortes(), o9 = [], j8; for (j8 = 0; j8 < L9.length; j8++) o9.push(L9[j8].nome); return o9; })();
        var agoraStr = (function () { var dd = new Date(); return dd.toLocaleDateString ? dd.toLocaleDateString("pt-BR") + " " + dd.toTimeString().slice(0, 5) : String(dd); })();
        var frasesG = (opts.narrativa && opts.narrativa.length) ? opts.narrativa : rGeral.frases;
        var H = [], j9, kM;
        function pct9(v9) { return (v9 >= 0 ? "+" : "") + v9 + "%"; }
        function cls9(v9) { return v9 >= 0 ? "pos" : "neg"; }
        H.push("<!DOCTYPE html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>" + esc(titulo9) + "</title><style>");
        H.push("body{font:13px/1.55 -apple-system,'Segoe UI',Roboto,sans-serif;color:#1f2328;margin:30px auto;max-width:880px;padding:0 16px}");
        H.push("h1{font-size:23px;margin:0}h2{font-size:15px;margin:26px 0 6px;border-bottom:2px solid #d0d7de;padding-bottom:4px;page-break-after:avoid}");
        H.push("h3{font-size:13.5px;margin:16px 0 4px}.sub{color:#59636e;font-size:12.5px}");
        H.push(".exec{background:#f6f8fa;border:1px solid #d0d7de;border-radius:10px;padding:12px 16px;margin:12px 0;page-break-inside:avoid}.exec li{margin:3px 0}");
        H.push(".sec{border:1px solid #d0d7de;border-radius:12px;padding:14px 18px;margin:14px 0;page-break-inside:avoid}");
        H.push("table{border-collapse:collapse;width:100%;margin:6px 0}th{text-align:right;font-size:11px;color:#59636e;border-bottom:2px solid #d0d7de;padding:4px 8px}");
        H.push("th:first-child,td:first-child{text-align:left}td{padding:4px 8px;border-bottom:1px solid #eceff2;text-align:right;font-variant-numeric:tabular-nums}");
        H.push(".pos{color:#1a7f37;font-weight:700}.neg{color:#cf222e;font-weight:700}.aviso{color:#9a6700;font-size:12px}");
        H.push("svg{page-break-inside:avoid}@media print{body{margin:8mm}.exec{-webkit-print-color-adjust:exact}}");
        H.push("</style></head><body>");
        H.push("<h1>" + esc(titulo9) + "</h1><div class=\"sub\">Gerado em " + agoraStr + " \u00b7 " + fmt.numero(todos9.length) + " linhas no dataset \u00b7 phx-grid v" + PhxGrid.versao + "</div>");
        /* vis\u00e3o geral */
        H.push("<h2>1. Vis\u00e3o geral</h2><div class=\"exec\"><ul>");
        for (j9 = 0; j9 < frasesG.length; j9++) H.push("<li>" + esc(frasesG[j9]) + "</li>");
        H.push("</ul></div>");
        /* linha do tempo geral */
        if (lt9.ok) {
          var maxS9 = 0;
          for (j9 = 0; j9 < lt9.serie.length; j9++) if (lt9.serie[j9].soma > maxS9) maxS9 = lt9.serie[j9].soma;
          H.push("<h2>2. Linha do tempo (" + esc(lt9.tituloValor) + ")</h2>");
          var W9 = Math.max(320, lt9.serie.length * 62), alt9 = 100;
          H.push("<svg width=\"" + W9 + "\" height=\"" + (alt9 + 40) + "\" xmlns=\"http://www.w3.org/2000/svg\">");
          for (j9 = 0; j9 < lt9.serie.length; j9++) {
            var it9 = lt9.serie[j9];
            var hB = maxS9 > 0 ? Math.round(it9.soma / maxS9 * alt9) : 0;
            var x9 = j9 * 62 + 6;
            H.push("<rect x=\"" + x9 + "\" y=\"" + (alt9 - hB) + "\" width=\"44\" height=\"" + hB + "\" fill=\"" + (it9 === lt9.melhor ? "#1a7f37" : "#0969da") + "\" rx=\"4\"/>");
            H.push("<text x=\"" + (x9 + 22) + "\" y=\"" + (alt9 + 14) + "\" font-size=\"10\" text-anchor=\"middle\" fill=\"#59636e\">" + esc(it9.rotulo) + "</text>");
            if (it9.deltaSomaPct != null) H.push("<text x=\"" + (x9 + 22) + "\" y=\"" + (alt9 + 28) + "\" font-size=\"9\" text-anchor=\"middle\" fill=\"" + (it9.deltaSomaPct >= 0 ? "#1a7f37" : "#cf222e") + "\">" + pct9(it9.deltaSomaPct) + "</text>");
          }
          H.push("</svg>");
          for (j9 = 0; j9 < lt9.frases.length; j9++) H.push("<div class=\"sub\">" + esc(lt9.frases[j9]) + "</div>");
        }
        /* recortes */
        H.push("<h2>3. Recortes acompanhados</h2>");
        if (!nomes9.length) H.push("<div class=\"aviso\">Nenhum recorte salvo.</div>");
        for (j9 = 0; j9 < nomes9.length; j9++) {
          var ev9 = api.evolucaoRecorte(nomes9[j9]);
          if (!ev9.ok) { H.push("<div class=\"aviso\">Recorte \"" + esc(nomes9[j9]) + "\" n\u00e3o encontrado \u2014 ignorado.</div>"); continue; }
          H.push("<div class=\"sec\"><h3>\uD83D\uDCCC " + esc(nomes9[j9]) + "</h3>");
          for (var k8 = 0; k8 < ev9.frases.length; k8++) H.push("<div class=\"sub\">" + esc(ev9.frases[k8]) + "</div>");
          H.push("<table><tr><th></th><th>ent\u00e3o</th><th>agora</th><th>\u0394%</th></tr>");
          H.push("<tr><td>Linhas</td><td>" + fmt.numero(ev9.contagem.antes) + "</td><td>" + fmt.numero(ev9.contagem.agora) + "</td><td class=\"" + cls9(ev9.contagem.pct) + "\">" + pct9(ev9.contagem.pct) + "</td></tr>");
          for (kM in ev9.metricas) {
            var mE = ev9.metricas[kM];
            var fV = mE.tipo === "moeda" ? fmt.moeda : fmt.numero;
            H.push("<tr><td>" + esc(mE.titulo) + " (soma)</td><td>" + fV(Math.round(mE.soma.antes)) + "</td><td>" + fV(Math.round(mE.soma.agora)) + "</td><td class=\"" + cls9(mE.soma.pct) + "\">" + pct9(mE.soma.pct) + "</td></tr>");
          }
          H.push("</table></div>");
        }
        /* compara\u00e7\u00f5es */
        if (opts.comparacoes && opts.comparacoes.length) {
          H.push("<h2>4. Compara\u00e7\u00f5es</h2>");
          for (j9 = 0; j9 < opts.comparacoes.length; j9++) {
            var par9 = opts.comparacoes[j9];
            var cmp9 = api.compararNL("compare " + par9[0] + " vs " + par9[1]);
            if (!cmp9.ok) { H.push("<div class=\"aviso\">Compara\u00e7\u00e3o inv\u00e1lida: " + esc(par9.join(" vs ")) + "</div>"); continue; }
            H.push("<div class=\"sec\"><h3>\u2696 " + esc(par9[0]) + " vs " + esc(par9[1]) + "</h3>");
            for (var k7 = 0; k7 < cmp9.frases.length; k7++) H.push("<div class=\"sub\">" + esc(cmp9.frases[k7]) + "</div>");
            H.push("<table><tr><th></th><th>" + esc(par9[0]) + "</th><th>" + esc(par9[1]) + "</th><th>\u0394%</th></tr>");
            H.push("<tr><td>Linhas</td><td>" + fmt.numero(cmp9.contagem.a) + "</td><td>" + fmt.numero(cmp9.contagem.b) + "</td><td class=\"" + cls9(cmp9.contagem.pct) + "\">" + pct9(cmp9.contagem.pct) + "</td></tr>");
            for (kM in cmp9.metricas) {
              var mC = cmp9.metricas[kM];
              var fV2 = mC.tipo === "moeda" ? fmt.moeda : fmt.numero;
              H.push("<tr><td>" + esc(mC.titulo) + " (soma)</td><td>" + fV2(Math.round(mC.soma.a)) + "</td><td>" + fV2(Math.round(mC.soma.b)) + "</td><td class=\"" + cls9(mC.soma.pct) + "\">" + pct9(mC.soma.pct) + "</td></tr>");
            }
            H.push("</table></div>");
          }
        }
        H.push("<div class=\"sub\" style=\"margin-top:24px\">Documento gerado automaticamente pelo phx-grid \u00b7 Phoenix \u00b7 Onda 2 Cognitive</div>");
        H.push("</body></html>");
        var html9 = H.join("");
        log("dossie", { recortes: nomes9.length, comparacoes: (opts.comparacoes || []).length, bytes: html9.length });
        return html9;
      },
      baixarDossie: function (nome, opts) {
        var html9 = api.gerarDossieHTML(opts);
        var blob9 = new Blob(["\ufeff", html9], { type: "text/html;charset=utf-8" });
        var url9 = URL.createObjectURL(blob9);
        var a9 = document.createElement("a");
        a9.href = url9;
        a9.download = nome || "dossie.html";
        document.body.appendChild(a9);
        a9.click();
        document.body.removeChild(a9);
        setTimeout(function () { URL.revokeObjectURL(url9); }, 800);
        return api;
      },
      exportarConfig: function (opts) {
        opts = opts || {};
        function liga(k9) { return opts[k9] !== false; }
        var pac = { versao: PhxGrid.versao, v: 1 };
        if (liga("layout")) pac.layout = api.layout();
        if (liga("recortes")) pac.recortes = JSON.parse(JSON.stringify(_recortes));
        if (liga("alertas")) {
          pac.alertas = [];
          var j9;
          for (j9 = 0; j9 < _regras.length; j9++) pac.alertas.push({ id: _regras[j9].id, frase: _regras[j9].frase, cbNome: _regras[j9].cbNome || null, soNovas: _regras[j9].soNovas });
        }
        if (liga("historico")) pac.historico = JSON.parse(JSON.stringify(histNL));
        if (liga("estilo")) pac.estilo = api.estiloAtual();
        log("config-export", { partes: (pac.layout ? 1 : 0) + (pac.recortes ? 1 : 0) + (pac.alertas ? 1 : 0) + (pac.historico ? 1 : 0) });
        return pac;
      },
      importarConfig: function (pac, opts) {
        opts = opts || {};
        if (!pac || pac.v !== 1) return { ok: false, erro: "pacote inv\u00e1lido ou vers\u00e3o desconhecida" };
        var aplicado = [], j9;
        if (pac.recortes) {
          if (opts.mesclar) { var k9; for (k9 in pac.recortes) _recortes[k9] = JSON.parse(JSON.stringify(pac.recortes[k9])); }
          else _recortes = JSON.parse(JSON.stringify(pac.recortes));
          _salvaRecortes();
          aplicado.push("recortes");
        }
        if (pac.alertas) {
          if (!opts.mesclar) { _regras.length = 0; _seqRegra = 0; }
          for (j9 = 0; j9 < pac.alertas.length; j9++) {
            var e9 = pac.alertas[j9];
            var r9 = interpretar(e9.frase, _ctxNL());
            var id9 = e9.id || (_seqRegra + 1);
            if (id9 > _seqRegra) _seqRegra = id9;
            _regras.push({ id: id9, frase: e9.frase, filtros: r9.filtros, busca: r9.busca, cbNome: e9.cbNome || null, aoDisparar: e9.cbNome ? _resolveFn(e9.cbNome) : null, soNovas: e9.soNovas !== false });
          }
          _salvaRegras();
          aplicado.push("alertas");
        }
        if (pac.historico) {
          histNL = pac.historico.slice(0, 50);
          salvaHist();
          aplicado.push("historico");
        }
        if (pac.layout) {
          api.aplicarLayout(pac.layout);
          aplicado.push("layout");
        }
        if (pac.estilo && (function () { var k8; for (k8 in pac.estilo) return true; return false; })()) {
          api.limparEstilo();
          api.estilizar(pac.estilo);
          aplicado.push("estilo");
        }
        log("config-import", { aplicado: aplicado.join(",") });
        return { ok: true, aplicado: aplicado };
      },
      linhaDoTempo: function (opts) {
        api.resumo();
        var linhasR = _resumoCache && _resumoCache.linhas ? _resumoCache.linhas : [];
        var lt9 = linhaDoTempo(linhasR, colunasDef, opts);
        if (lt9.ok) log("tempo", { periodos: lt9.serie.length, campo: lt9.campoData });
        return lt9;
      },
      abrirLinhaDoTempo: function (opts) {
        var lt9 = api.linhaDoTempo(opts);
        if (!lt9.ok) return lt9;
        var pn = wrap.querySelector(".phx-resumo");
        if (pn) pn.parentNode.removeChild(pn);
        pn = el("div", "phx-resumo phx-compara phx-tempo");
        var h9 = '<div class="phx-resumo-cab"><b>\uD83D\uDCC5 Linha do tempo</b><button class="phx-resumo-x" type="button">\u00d7</button></div><ul>';
        var j9;
        for (j9 = 0; j9 < lt9.frases.length; j9++) h9 += "<li>" + esc(lt9.frases[j9]) + "</li>";
        h9 += "</ul><div class=\"phx-tempo-barras\">";
        var maxS9 = 0;
        for (j9 = 0; j9 < lt9.serie.length; j9++) if (lt9.serie[j9].soma > maxS9) maxS9 = lt9.serie[j9].soma;
        for (j9 = 0; j9 < lt9.serie.length; j9++) {
          var it9 = lt9.serie[j9];
          var hh9 = maxS9 > 0 ? Math.max(6, Math.round(it9.soma / maxS9 * 74)) : 6;
          var fVb = lt9.tipoValor === "moeda" ? fmt.moeda : fmt.numero;
          h9 += '<button type="button" class="phx-tempo-col" data-per="' + esc(it9.periodo) + '" title="' + esc(it9.rotulo + ": " + fVb(Math.round(it9.soma)) + " em " + it9.n + " linhas") + '">' +
            '<i style="height:' + hh9 + 'px"' + (it9 === lt9.melhor ? ' class="phx-tempo-max"' : "") + "></i><span>" + esc(it9.rotulo) + "</span>" +
            (it9.deltaSomaPct != null ? '<em class="' + (it9.deltaSomaPct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + '">' + (it9.deltaSomaPct >= 0 ? "+" : "") + it9.deltaSomaPct + "%</em>" : "<em>\u2014</em>") +
            "</button>";
        }
        h9 += "</div><div class=\"phx-tempo-dica\">clique num m\u00eas para filtrar</div>";
        pn.innerHTML = h9;
        wrap.appendChild(pn);
        pn.querySelector(".phx-resumo-x").addEventListener("click", function () { api.fecharResumo(); });
        var bs9 = pn.querySelectorAll(".phx-tempo-col");
        for (j9 = 0; j9 < bs9.length; j9++) (function (b9) {
          b9.addEventListener("click", function () {
            var per9 = b9.getAttribute("data-per");
            var ano9 = parseInt(per9.slice(0, 4), 10), mes9 = parseInt(per9.slice(5, 7), 10);
            var fim9 = new Date(ano9, mes9, 0).getDate();
            api.fecharResumo();
            api.filtrar(lt9.campoData, { tipo: "faixa", de: per9 + "-01", ate: per9 + "-" + (fim9 < 10 ? "0" : "") + fim9, tipoCol: "data" });
            anuncia("Filtrado: " + per9);
          });
        })(bs9[j9]);
        anuncia(lt9.frases.join("; "));
        return lt9;
      },
      salvarRecorte: function (nome) {
        if (!nome) return { ok: false, erro: "nome obrigat\u00f3rio" };
        var r9 = api.resumo();
        _recortes[nome] = { nome: String(nome), filtros: serializaFiltros(), foto: fotografa(r9) };
        _salvaRecortes();
        log("recorte-salvo", { nome: String(nome), total: r9.total, filtros: _recortes[nome].filtros.length });
        return { ok: true, nome: String(nome), total: r9.total, ts: _recortes[nome].foto.ts };
      },
      listarRecortes: function () {
        var out9 = [], k9;
        for (k9 in _recortes) out9.push({ nome: k9, ts: _recortes[k9].foto.ts, total: _recortes[k9].foto.total, filtros: _recortes[k9].filtros.length });
        out9.sort(function (a, b) { return b.ts - a.ts; });
        return out9;
      },
      removerRecorte: function (nome) {
        if (!_recortes[nome]) return false;
        delete _recortes[nome];
        _salvaRecortes();
        return true;
      },
      evolucaoRecorte: function (nome) {
        var rec9 = _recortes[nome];
        if (!rec9) return { ok: false, erro: "recorte desconhecido: " + nome };
        var linhas9 = _linhasDosFiltros(rec9.filtros);
        var atual9 = resumir(linhas9, colunasDef, { topN: 3 });
        var ev9 = evolucaoDe(rec9.foto, atual9);
        ev9.ok = true;
        ev9.nome = nome;
        log("evolucao", { nome: nome, antes: ev9.contagem.antes, agora: ev9.contagem.agora });
        return ev9;
      },
      abrirRecorte: function (nome) {
        var rec9 = _recortes[nome];
        if (!rec9) return { ok: false, erro: "recorte desconhecido: " + nome };
        filtros = {};
        var j9;
        for (j9 = 0; j9 < rec9.filtros.length; j9++) filtros[rec9.filtros[j9].campo] = rec9.filtros[j9];
        estado.pagina = 1;
        montaChips();
        carrega();
        return api.evolucaoRecorte(nome);
      },
      abrirEvolucao: function (nome) {
        var ev9 = api.evolucaoRecorte(nome);
        if (!ev9.ok) return ev9;
        var pn = wrap.querySelector(".phx-resumo");
        if (pn) pn.parentNode.removeChild(pn);
        pn = el("div", "phx-resumo phx-compara phx-evolucao");
        var h9 = '<div class="phx-resumo-cab"><b>\uD83D\uDCC8 ' + esc(nome) + '</b><button class="phx-resumo-x" type="button">\u00d7</button></div><ul>';
        var j9;
        for (j9 = 0; j9 < ev9.frases.length; j9++) h9 += "<li>" + esc(ev9.frases[j9]) + "</li>";
        h9 += "</ul><table class=\"phx-resumo-tab phx-cmp-tab\"><tr><th></th><th>ent\u00e3o</th><th>agora</th><th>\u0394%</th></tr>";
        h9 += "<tr><td>Linhas</td><td>" + fmt.numero(ev9.contagem.antes) + "</td><td>" + fmt.numero(ev9.contagem.agora) + "</td><td class=\"" + (ev9.contagem.pct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + "\">" + (ev9.contagem.pct >= 0 ? "+" : "") + ev9.contagem.pct + "%</td></tr>";
        var kM;
        for (kM in ev9.metricas) {
          var mE = ev9.metricas[kM];
          var fV = mE.tipo === "moeda" ? fmt.moeda : fmt.numero;
          h9 += "<tr><td>" + esc(mE.titulo) + " (soma)</td><td>" + fV(Math.round(mE.soma.antes)) + "</td><td>" + fV(Math.round(mE.soma.agora)) + "</td><td class=\"" + (mE.soma.pct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + "\">" + (mE.soma.pct >= 0 ? "+" : "") + mE.soma.pct + "%</td></tr>";
        }
        h9 += "</table>";
        pn.innerHTML = h9;
        wrap.appendChild(pn);
        pn.querySelector(".phx-resumo-x").addEventListener("click", function () { api.fecharResumo(); });
        anuncia(ev9.frases.join("; "));
        return ev9;
      },
      alertaAdicionar: function (frase, aoDisparar, opts) {
        opts = opts || {};
        var r8 = interpretar(frase, _ctxNL());
        var cbNome = typeof aoDisparar === "string" ? aoDisparar : null;
        var fn8 = cbNome ? _resolveFn(cbNome) : (typeof aoDisparar === "function" ? aoDisparar : null);
        var regra = { id: ++_seqRegra, frase: String(frase), filtros: r8.filtros, busca: r8.busca, cbNome: cbNome, aoDisparar: fn8, soNovas: opts.soNovas !== false };
        _regras.push(regra);
        _salvaRegras();
        log("alerta-add", { id: regra.id, frase: regra.frase, filtros: r8.filtros.length });
        return { id: regra.id, frase: regra.frase, explicacao: r8.explicacao };
      },
      alertaRemover: function (id8) {
        var j8;
        for (j8 = 0; j8 < _regras.length; j8++) if (_regras[j8].id === id8) { _regras.splice(j8, 1); _salvaRegras(); return true; }
        return false;
      },
      alertaListar: function () {
        var out8 = [], j8;
        for (j8 = 0; j8 < _regras.length; j8++) out8.push({ id: _regras[j8].id, frase: _regras[j8].frase, soNovas: _regras[j8].soNovas });
        return out8;
      },
      alertaAvaliar: function () { avaliaAlertas(null); return api; },
      alertaPendentes: function () { return _pendAlertas.slice(); },
      alertaLimparPendentes: function () { _pendAlertas.length = 0; _atualizaBadge(); return api; },
      compararNL: function (frase, opts) {
        opts = opts || {};
        if (!fonte.local || !fonte.todos) return { ok: false, erro: "compara\u00e7\u00e3o requer fonte local" };
        var ctxN = { colunas: [], distintos: {}, agora: opts.agora || null };
        var j9, c9;
        for (j9 = 0; j9 < colunasDef.length; j9++) ctxN.colunas.push({ campo: colunasDef[j9].campo, titulo: colunasDef[j9].titulo, tipo: colunasDef[j9].tipo || "texto" });
        for (j9 = 0; j9 < colunasDef.length; j9++) {
          c9 = colunasDef[j9];
          var t9 = c9.tipo || "texto";
          if (["numero", "moeda", "percentual", "data", "dataHora"].indexOf(t9) >= 0) continue;
          var vis9 = {}, arr9 = [], i9;
          for (i9 = 0; i9 < fonte.todos.length && arr9.length <= 50; i9++) {
            var v9 = fonte.todos[i9][c9.campo];
            if (v9 == null || v9 === "") continue;
            var k9 = String(v9);
            if (!vis9[k9]) { vis9[k9] = 1; arr9.push(k9); }
          }
          if (arr9.length && arr9.length <= 50) ctxN.distintos[c9.campo] = arr9;
        }
        var cp = interpretarComparacao(frase, ctxN);
        if (!cp) return { ok: false, erro: "frase n\u00e3o \u00e9 uma compara\u00e7\u00e3o (use: compare X vs Y)" };
        function aplicaLado(res9) {
          var lista9 = res9.filtros.slice();
          if (res9.busca) lista9.push({ campo: "*", tipo: "busca", termo: res9.busca, campos: camposBuscaveis() });
          return aplicaFiltros(fonte.todos, lista9);
        }
        var linhasA = aplicaLado(cp.resA), linhasB = aplicaLado(cp.resB);
        var cmp9 = compararRecortes(linhasA, linhasB, colunasDef, { rotuloA: cp.fraseA, rotuloB: cp.fraseB, topN: opts.topN });
        cmp9.ok = true;
        cmp9.explicacaoA = cp.resA.explicacao;
        cmp9.explicacaoB = cp.resB.explicacao;
        log("compara", { a: cp.fraseA, b: cp.fraseB, nA: cmp9.contagem.a, nB: cmp9.contagem.b });
        return cmp9;
      },
      abrirComparacao: function (frase, opts) {
        var cmp9 = api.compararNL(frase, opts);
        if (!cmp9.ok) return cmp9;
        function montaCmp(frases9) {
          var pn = wrap.querySelector(".phx-resumo");
          if (pn) pn.parentNode.removeChild(pn);
          pn = el("div", "phx-resumo phx-compara");
          var h9 = '<div class="phx-resumo-cab"><b>\u2696 ' + esc(cmp9.rotuloA) + ' vs ' + esc(cmp9.rotuloB) + '</b><button class="phx-resumo-x" type="button">\u00d7</button></div><ul>';
          var j9;
          for (j9 = 0; j9 < frases9.length; j9++) h9 += "<li>" + esc(frases9[j9]) + "</li>";
          h9 += "</ul><table class=\"phx-resumo-tab phx-cmp-tab\"><tr><th></th><th>" + esc(cmp9.rotuloA) + "</th><th>" + esc(cmp9.rotuloB) + "</th><th>\u0394%</th></tr>";
          h9 += "<tr><td>Linhas</td><td>" + fmt.numero(cmp9.contagem.a) + "</td><td>" + fmt.numero(cmp9.contagem.b) + "</td><td class=\"" + (cmp9.contagem.pct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + "\">" + (cmp9.contagem.pct >= 0 ? "+" : "") + cmp9.contagem.pct + "%</td></tr>";
          var kM;
          for (kM in cmp9.metricas) {
            var mC = cmp9.metricas[kM];
            var fV = mC.tipo === "moeda" ? fmt.moeda : fmt.numero;
            h9 += "<tr><td>" + esc(mC.titulo) + " (soma)</td><td>" + fV(Math.round(mC.soma.a)) + "</td><td>" + fV(Math.round(mC.soma.b)) + "</td><td class=\"" + (mC.soma.pct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + "\">" + (mC.soma.pct >= 0 ? "+" : "") + mC.soma.pct + "%</td></tr>";
            h9 += "<tr><td>" + esc(mC.titulo) + " (m\u00e9dia)</td><td>" + fV(Math.round(mC.media.a)) + "</td><td>" + fV(Math.round(mC.media.b)) + "</td><td class=\"" + (mC.media.pct >= 0 ? "phx-cmp-pos" : "phx-cmp-neg") + "\">" + (mC.media.pct >= 0 ? "+" : "") + mC.media.pct + "%</td></tr>";
          }
          h9 += "</table>";
          pn.innerHTML = h9;
          wrap.appendChild(pn);
          pn.querySelector(".phx-resumo-x").addEventListener("click", function () { api.fecharResumo(); });
          anuncia(frases9.join("; "));
        }
        if (cfg.ia && typeof cfg.ia.comparar === "function") cfg.ia.comparar(cmp9, function (t9) { montaCmp(t9 && t9.length ? (Object.prototype.toString.call(t9) === "[object Array]" ? t9 : [String(t9)]) : cmp9.frases); });
        else montaCmp(cmp9.frases);
        return cmp9;
      },
      gerarRelatorioHTML: function (opts) {
        opts = opts || {};
        var maxL = opts.maxLinhas || 200;
        var r9 = api.resumo();
        var linhasR = _resumoCache && _resumoCache.linhas ? _resumoCache.linhas : [];
        var vis9 = visiveis();
        var agoraStr = (function () { var dd = new Date(); return dd.toLocaleDateString ? dd.toLocaleDateString("pt-BR") + " " + dd.toTimeString().slice(0, 5) : String(dd); })();
        var filtrosTxt = [], ativos = serializaFiltros(), j9, f9, c9;
        for (j9 = 0; j9 < ativos.length; j9++) {
          f9 = ativos[j9];
          c9 = porCampo[f9.campo];
          var tit9 = (c9 && c9.titulo) || f9.campo;
          if (f9.campo === "*") filtrosTxt.push('busca "' + f9.termo + '"');
          else if (f9.tipo === "valores") filtrosTxt.push(tit9 + ": " + f9.valores.join("/"));
          else if (f9.tipo === "faixa") filtrosTxt.push(tit9 + (f9.de != null ? " \u2265 " + f9.de : "") + (f9.ate != null ? " \u2264 " + f9.ate : ""));
          else filtrosTxt.push(tit9);
        }
        var frases9 = (opts.narrativa && opts.narrativa.length) ? opts.narrativa : r9.frases;
        var H = [];
        H.push("<!DOCTYPE html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>" + esc(opts.titulo || "Relat\u00f3rio") + "</title><style>");
        H.push("body{font:13px/1.5 -apple-system,'Segoe UI',Roboto,sans-serif;color:#1f2328;margin:28px auto;max-width:900px;padding:0 16px}");
        H.push("h1{font-size:21px;margin:0 0 2px}.sub{color:#59636e;font-size:12.5px;margin-bottom:18px}");
        H.push(".exec{background:#f6f8fa;border:1px solid #d0d7de;border-radius:10px;padding:14px 18px;margin:14px 0;page-break-inside:avoid}");
        H.push(".exec li{margin:4px 0}.cards{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0}");
        H.push(".card{border:1px solid #d0d7de;border-radius:10px;padding:10px 14px;min-width:150px;page-break-inside:avoid}");
        H.push(".card b{display:block;font-size:11px;color:#59636e;font-weight:600}.card .v{font-size:16px;font-weight:700}");
        H.push(".card .sub2{font-size:11px;color:#59636e}");
        H.push("table{border-collapse:collapse;width:100%;margin:8px 0}th{text-align:left;font-size:11px;color:#59636e;border-bottom:2px solid #d0d7de;padding:5px 8px}");
        H.push("td{padding:5px 8px;border-bottom:1px solid #eceff2}tbody tr:nth-child(even){background:#fbfcfd}");
        H.push(".num{text-align:right;font-variant-numeric:tabular-nums}.barra{background:#ddf4ff;height:8px;border-radius:99px}.barra i{display:block;height:100%;background:#0969da;border-radius:99px}");
        H.push("h2{font-size:14px;margin:22px 0 4px;page-break-after:avoid}.aviso{color:#9a6700;font-size:12px}");
        H.push("svg{page-break-inside:avoid}thead{display:table-header-group}");
        H.push("@media print{body{margin:8mm}.exec{background:#f6f8fa!important;-webkit-print-color-adjust:exact}}");
        H.push("</style></head><body>");
        H.push("<h1>" + esc(opts.titulo || "Relat\u00f3rio do recorte") + "</h1>");
        H.push("<div class=\"sub\">" + (filtrosTxt.length ? "Filtros: " + esc(filtrosTxt.join(" \u00b7 ")) : "Sem filtros (dataset completo)") + " \u2014 gerado em " + agoraStr + " \u00b7 " + fmt.numero(r9.total) + " linhas</div>");
        H.push("<div class=\"exec\"><b>Sum\u00e1rio executivo</b><ul>");
        for (j9 = 0; j9 < frases9.length; j9++) H.push("<li>" + esc(frases9[j9]) + "</li>");
        H.push("</ul></div>");
        /* cards */
        H.push("<div class=\"cards\">");
        var kM;
        for (kM in r9.metricas) {
          var mM = r9.metricas[kM];
          var fV9 = mM.tipo === "moeda" ? fmt.moeda : fmt.numero;
          H.push("<div class=\"card\"><b>" + esc(mM.titulo) + "</b><span class=\"v\">" + fV9(Math.round(mM.soma * 100) / 100) + "</span><span class=\"sub2\">m\u00e9dia " + fV9(Math.round(mM.media * 100) / 100) + " \u00b7 mediana " + fV9(mM.mediana) + "<br>min " + fV9(mM.min) + " \u00b7 max " + fV9(mM.max) + "</span></div>");
        }
        H.push("</div>");
        /* tops */
        for (j9 = 0; j9 < r9.tops.length; j9++) {
          var tp9 = r9.tops[j9];
          H.push("<h2>Top " + esc(tp9.titulo) + "</h2><table><thead><tr><th>Valor</th><th class=\"num\">Linhas</th><th style=\"width:40%\">%</th>" + (tp9.porSoma ? "<th class=\"num\">Soma</th>" : "") + "</tr></thead><tbody>");
          var k9;
          for (k9 = 0; k9 < tp9.porContagem.length; k9++) {
            var pc9 = tp9.porContagem[k9];
            var somaK = null, k8;
            if (tp9.porSoma) for (k8 = 0; k8 < tp9.porSoma.length; k8++) if (tp9.porSoma[k8].valor === pc9.valor) somaK = tp9.porSoma[k8].soma;
            H.push("<tr><td>" + esc(pc9.valor) + "</td><td class=\"num\">" + fmt.numero(pc9.n) + "</td><td><div class=\"barra\"><i style=\"width:" + pc9.pct + "%\"></i></div> " + pc9.pct + "%</td>" + (tp9.porSoma ? "<td class=\"num\">" + (somaK != null ? fmt.moeda(somaK) : "\u2014") + "</td>" : "") + "</tr>");
          }
          H.push("</tbody></table>");
        }
        /* tend\u00eancia SVG */
        if (r9.tendencia) {
          var serie9 = r9.tendencia.serie, maxS9 = 0;
          for (j9 = 0; j9 < serie9.length; j9++) if (serie9[j9].soma > maxS9) maxS9 = serie9[j9].soma;
          var W9 = Math.max(320, serie9.length * 64), alt9 = 120;
          H.push("<h2>Tend\u00eancia (" + esc(r9.tendencia.direcao) + " " + r9.tendencia.pct + "%)</h2>");
          H.push("<svg width=\"" + W9 + "\" height=\"" + (alt9 + 26) + "\" xmlns=\"http://www.w3.org/2000/svg\">");
          for (j9 = 0; j9 < serie9.length; j9++) {
            var hB = maxS9 > 0 ? Math.round(serie9[j9].soma / maxS9 * alt9) : 0;
            var x9 = j9 * 64 + 8;
            H.push("<rect x=\"" + x9 + "\" y=\"" + (alt9 - hB) + "\" width=\"46\" height=\"" + hB + "\" fill=\"#0969da\" rx=\"4\"/>");
            H.push("<text x=\"" + (x9 + 23) + "\" y=\"" + (alt9 + 16) + "\" font-size=\"10\" text-anchor=\"middle\" fill=\"#59636e\">" + esc(serie9[j9].periodo.slice(2)) + "</text>");
          }
          H.push("</svg>");
        }
        /* anomalias */
        if (r9.anomalias && r9.anomalias.length) {
          H.push("<h2>Anomalias (" + r9.anomaliasTotal + " acima de 2,5\u03c3)</h2><table><thead><tr>");
          for (j9 = 0; j9 < vis9.length; j9++) H.push("<th>" + esc(vis9[j9].titulo || vis9[j9].campo) + "</th>");
          H.push("<th class=\"num\">z</th></tr></thead><tbody>");
          for (j9 = 0; j9 < r9.anomalias.length; j9++) {
            H.push("<tr>");
            for (var k7 = 0; k7 < vis9.length; k7++) H.push("<td" + (["numero", "moeda", "percentual"].indexOf(vis9[k7].tipo) >= 0 ? " class=\"num\"" : "") + ">" + esc(textoCelula(vis9[k7], r9.anomalias[j9].linha[vis9[k7].campo], r9.anomalias[j9].linha)) + "</td>");
            H.push("<td class=\"num\">" + r9.anomalias[j9].z + "</td></tr>");
          }
          H.push("</tbody></table>");
        }
        /* dados */
        var nT = Math.min(linhasR.length, maxL);
        H.push("<h2>Dados do recorte</h2>");
        if (linhasR.length > maxL) H.push("<div class=\"aviso\">Mostrando " + fmt.numero(maxL) + " de " + fmt.numero(linhasR.length) + " linhas.</div>");
        H.push("<table><thead><tr>");
        for (j9 = 0; j9 < vis9.length; j9++) H.push("<th" + (["numero", "moeda", "percentual"].indexOf(vis9[j9].tipo) >= 0 ? " class=\"num\"" : "") + ">" + esc(vis9[j9].titulo || vis9[j9].campo) + "</th>");
        H.push("</tr></thead><tbody>");
        for (j9 = 0; j9 < nT; j9++) {
          H.push("<tr>");
          for (var k6 = 0; k6 < vis9.length; k6++) {
            H.push("<td" + (["numero", "moeda", "percentual"].indexOf(vis9[k6].tipo) >= 0 ? " class=\"num\"" : "") + ">" + esc(textoCelula(vis9[k6], linhasR[j9][vis9[k6].campo], linhasR[j9])) + "</td>");
          }
          H.push("</tr>");
        }
        H.push("</tbody></table>");
        H.push("<div class=\"sub\" style=\"margin-top:22px\">Gerado por phx-grid v" + PhxGrid.versao + " \u00b7 Phoenix</div>");
        H.push("</body></html>");
        var html9 = H.join("");
        log("relatorio", { linhas: r9.total, tabela: nT, bytes: html9.length });
        return html9;
      },
      baixarRelatorio: function (nome, opts) {
        var html9 = api.gerarRelatorioHTML(opts);
        var blob9 = new Blob(["\ufeff", html9], { type: "text/html;charset=utf-8" });
        var url9 = URL.createObjectURL(blob9);
        var a9 = document.createElement("a");
        a9.href = url9;
        a9.download = nome || "relatorio.html";
        document.body.appendChild(a9);
        a9.click();
        document.body.removeChild(a9);
        setTimeout(function () { URL.revokeObjectURL(url9); }, 800);
        return api;
      },
      explicarLinha: function (chave9) {
        var k9 = String(chave9);
        var linhasR = fonte.local && fonte.todos ? aplicaFiltros(fonte.todos, serializaFiltros()) : (ultimaCarga ? ultimaCarga.linhas : []);
        var linha = null, j9;
        for (j9 = 0; j9 < linhasR.length; j9++) if (chaveDe(linhasR[j9], j9) === k9) { linha = linhasR[j9]; break; }
        if (!linha) return { ok: false, erro: "linha fora do recorte: " + k9 };
        var r9 = api.resumo();
        var out = { ok: true, chave: k9, linha: linha, filtros: [], posicao: {}, rank: null, lideranca: [], frases: [] };
        var ativos = serializaFiltros(), f9, c9, v9;
        for (j9 = 0; j9 < ativos.length; j9++) {
          f9 = ativos[j9];
          if (f9.campo === "*") { out.filtros.push({ rotulo: 'busca "' + f9.termo + '"', valor: null, contrafactual: "sairia sem o termo no texto" }); continue; }
          c9 = porCampo[f9.campo];
          v9 = linha[f9.campo];
          var tit9 = (c9 && c9.titulo) || f9.campo;
          if (f9.tipo === "faixa") {
            var partes9 = [], contra9 = [], folga9 = null;
            if (f9.de != null) { partes9.push("\u2265 " + f9.de); contra9.push(tit9 + " < " + f9.de); if (typeof v9 === "number") folga9 = v9 - f9.de; }
            if (f9.ate != null) { partes9.push("\u2264 " + f9.ate); contra9.push(tit9 + " > " + f9.ate); if (typeof v9 === "number") { var fb9 = f9.ate - v9; folga9 = folga9 == null ? fb9 : Math.min(folga9, fb9); } }
            out.filtros.push({ rotulo: tit9 + " " + partes9.join(" e "), valor: v9, contrafactual: "sairia se " + contra9.join(" ou "), folga: folga9 });
          } else if (f9.tipo === "valores") {
            out.filtros.push({ rotulo: tit9 + " \u2208 [" + f9.valores.join(", ") + "]", valor: v9, contrafactual: "sairia se " + tit9 + " \u2260 " + String(v9) });
          } else out.filtros.push({ rotulo: tit9, valor: v9, contrafactual: null });
        }
        var kM;
        for (kM in r9.metricas) {
          var mM = r9.metricas[kM];
          var vL = Number(linha[kM]);
          if (vL !== vL) continue;
          var menores = 0;
          var base9 = _resumoCache && _resumoCache.linhas ? _resumoCache.linhas : linhasR;
          for (j9 = 0; j9 < base9.length; j9++) { var vb = Number(base9[j9][kM]); if (vb === vb && vb < vL) menores++; }
          var pct9 = Math.round(menores / mM.n * 100);
          var z9 = mM.desvio > 0 ? Math.round((vL - mM.media) / mM.desvio * 100) / 100 : 0;
          var dm9 = mM.media !== 0 ? Math.round((vL / mM.media - 1) * 1000) / 10 : 0;
          out.posicao[kM] = { titulo: mM.titulo, valor: vL, percentil: pct9, z: z9, vsMediaPct: dm9 };
        }
        if (estado.ordem && estado.ordem.campo) {
          var cO9 = estado.ordem.campo, dirO9 = estado.ordem.dir || "asc";
          var vO9 = linha[cO9], antes9 = 0, tipoO9 = (porCampo[cO9] && porCampo[cO9].tipo) || "texto";
          var kL9 = chaveOrd(vO9, tipoO9);
          for (j9 = 0; j9 < linhasR.length; j9++) {
            var kX9 = chaveOrd(linhasR[j9][cO9], tipoO9);
            if (dirO9 === "desc" ? kX9.v > kL9.v : kX9.v < kL9.v) antes9++;
          }
          out.rank = { campo: (porCampo[cO9] && porCampo[cO9].titulo) || cO9, pos: antes9 + 1, de: linhasR.length, dir: dirO9 };
        }
        for (j9 = 0; j9 < r9.tops.length; j9++) {
          var tp9 = r9.tops[j9];
          if (tp9.porContagem.length && String(linha[tp9.campo]) === tp9.porContagem[0].valor) {
            out.lideranca.push({ campo: tp9.titulo, valor: tp9.porContagem[0].valor, pct: tp9.porContagem[0].pct });
          }
        }
        var fr9 = out.frases;
        for (j9 = 0; j9 < out.filtros.length; j9++) {
          f9 = out.filtros[j9];
          fr9.push("Passa em " + f9.rotulo + (f9.valor != null ? " (valor: " + f9.valor + ")" : "") + (f9.contrafactual ? " \u2014 " + f9.contrafactual + (f9.folga != null ? "; folga " + fmt.numero(Math.round(f9.folga * 100) / 100) : "") : ""));
        }
        for (kM in out.posicao) {
          var pS = out.posicao[kM];
          fr9.push(pS.titulo + ": percentil " + pS.percentil + " do recorte \u00b7 " + (pS.vsMediaPct >= 0 ? "+" : "") + pS.vsMediaPct + "% vs m\u00e9dia \u00b7 z=" + pS.z);
        }
        if (out.rank) fr9.push(out.rank.pos + "\u00aa de " + out.rank.de + " por " + out.rank.campo + " " + out.rank.dir);
        for (j9 = 0; j9 < out.lideranca.length; j9++) fr9.push(out.lideranca[j9].valor + " \u00e9 o grupo l\u00edder de " + out.lideranca[j9].campo + " (" + out.lideranca[j9].pct + "%)");
        log("explica", { chave: k9, filtros: out.filtros.length, frases: fr9.length });
        return out;
      },
      abrirExplicacao: function (chave9) {
        var ex9 = api.explicarLinha(chave9);
        if (!ex9.ok) return ex9;
        function montaEx(frases9) {
          var pn = wrap.querySelector(".phx-resumo");
          if (pn) pn.parentNode.removeChild(pn);
          pn = el("div", "phx-resumo phx-explica");
          var h9 = '<div class="phx-resumo-cab"><b>\u2753 Por que a linha ' + esc(String(chave9)) + '?</b><button class="phx-resumo-x" type="button">\u00d7</button></div><ul>';
          var j9;
          for (j9 = 0; j9 < frases9.length; j9++) h9 += "<li>" + esc(frases9[j9]) + "</li>";
          pn.innerHTML = h9 + "</ul>";
          wrap.appendChild(pn);
          pn.querySelector(".phx-resumo-x").addEventListener("click", function () { api.fecharResumo(); });
          anuncia(frases9.join("; "));
        }
        if (cfg.ia && typeof cfg.ia.explicar === "function") cfg.ia.explicar(ex9, function (t9) { montaEx(t9 && t9.length ? (Object.prototype.toString.call(t9) === "[object Array]" ? t9 : [String(t9)]) : ex9.frases); });
        else montaEx(ex9.frases);
        return ex9;
      },
      executarNL: function (frase) {
        var ctxC = { colunas: [] };
        var j9;
        for (j9 = 0; j9 < colunasDef.length; j9++) ctxC.colunas.push({ campo: colunasDef[j9].campo, titulo: colunasDef[j9].titulo, tipo: colunasDef[j9].tipo || "texto" });
        var cmd = interpretarComando(frase, ctxC);
        var rotulos = [], a9;
        for (j9 = 0; j9 < cmd.acoes.length; j9++) {
          a9 = cmd.acoes[j9];
          if (a9.metodo === "_pagRel") api.pagina(estado.pagina + a9.args[0]);
          else if (a9.metodo === "_pagUltima") api.pagina(Math.max(1, Math.ceil(estado.total / estado.tamanho)));
          else if (typeof api[a9.metodo] === "function") api[a9.metodo].apply(api, a9.args);
          if (a9.rotulo) rotulos.push(a9.rotulo);
        }
        var resFiltro = null;
        var restoUtil = cmd.resto.replace(/[^a-z0-9]/g, "");
        if (restoUtil.length > 2) resFiltro = api.filtrarNatural(cmd.resto);
        log("cmd", { frase: String(frase), acoes: rotulos, resto: resFiltro ? cmd.resto : null });
        if (rotulos.length) anuncia(rotulos.join("; "));
        return { acoes: cmd.acoes, rotulos: rotulos, filtros: resFiltro };
      },
      historicoNL: function () { return histNL.slice(); },
      repetirNL: function (ix) {
        var e9 = histNL[ix || 0];
        if (!e9) return api;
        return api.filtrarNatural(e9.frase);
      },
      sugestoesNL: function (opts) {
        opts = opts || {};
        var maxS = opts.max || 6;
        var r9 = api.resumo({ topN: 3 });
        var ativos = serializaFiltros();
        var camposAtivos = {};
        var j9;
        for (j9 = 0; j9 < ativos.length; j9++) camposAtivos[ativos[j9].campo] = true;
        var prefixo = "";
        for (j9 = 0; j9 < ativos.length; j9++) {
          if (ativos[j9].tipo === "valores" && ativos[j9].valores.length === 1) { prefixo = semAcento(String(ativos[j9].valores[0])).toLowerCase() + "s "; break; }
        }
        var sug = [];
        function add(rotulo, frase9) {
          if (sug.length >= maxS) return;
          var jS;
          for (jS = 0; jS < sug.length; jS++) if (sug[jS].frase === frase9) return;
          sug.push({ rotulo: rotulo, frase: frase9 });
        }
        /* categóricas: líder e dominante — só campos SEM filtro ativo */
        for (j9 = 0; j9 < r9.tops.length; j9++) {
          var tp = r9.tops[j9];
          if (camposAtivos[tp.campo] || !tp.porContagem.length) continue;
          var lider = tp.porContagem[0];
          var fraseL = prefixo + "de " + semAcento(lider.valor).toLowerCase();
          add("s\u00f3 " + lider.valor + " (" + lider.pct + "%)", prefixo ? fraseL : semAcento(lider.valor).toLowerCase() + "s");
          if (lider.pct >= 50 && tp.distintos > 1) add("exceto " + lider.valor, prefixo + "nao " + semAcento(lider.valor).toLowerCase() + "s");
        }
        /* numérica principal: acima da média */
        var kM, mP9 = null, tituloP = "";
        for (kM in r9.metricas) {
          if (r9.metricas[kM].tipo === "moeda") { mP9 = r9.metricas[kM]; tituloP = kM; break; }
        }
        if (!mP9) for (kM in r9.metricas) { mP9 = r9.metricas[kM]; tituloP = kM; break; }
        if (mP9 && mP9.media > 0) {
          var mediaR = Math.round(mP9.media);
          add("acima da m\u00e9dia (" + (mP9.tipo === "moeda" ? fmt.moeda(mediaR) : fmt.numero(mediaR)) + ")", prefixo + "acima de " + mediaR);
        }
        /* anomalias detectadas */
        if (r9.anomaliasTotal && mP9) {
          var limA = Math.round(mP9.media + 2.5 * mP9.desvio);
          add("ver " + r9.anomaliasTotal + " anomalia" + (r9.anomaliasTotal > 1 ? "s" : ""), prefixo + "acima de " + limA);
        }
        /* data */
        for (j9 = 0; j9 < colunasDef.length; j9++) {
          var tD = colunasDef[j9].tipo;
          if ((tD === "data" || tD === "dataHora") && !camposAtivos[colunasDef[j9].campo]) {
            add("este m\u00eas", prefixo + "este mes");
            break;
          }
        }
        var final9 = sug.slice(0, maxS);
        if (cfg.ia && typeof cfg.ia.sugerir === "function") {
          var subst = cfg.ia.sugerir({ resumo: r9, filtrosAtivos: ativos, sugestoesLocais: final9 });
          if (subst && subst.length) final9 = subst;
        }
        log("sugestoes", { n: final9.length });
        return final9;
      },
      resumo: function (opts) {
        opts = opts || {};
        var linhasR;
        if (fonte.local && fonte.todos) linhasR = aplicaFiltros(fonte.todos, serializaFiltros());
        else linhasR = ultimaCarga ? ultimaCarga.linhas : [];
        var assin = JSON.stringify(serializaFiltros()) + "|" + linhasR.length + "|" + (opts.topN || 3);
        if (_resumoCache && _resumoCache.assin === assin) return _resumoCache.r;
        var r9 = resumir(linhasR, colunasDef, { topN: opts.topN, zLimiar: opts.zLimiar, totalGeral: fonte.local && fonte.todos ? fonte.todos.length : null });
        _resumoCache = { assin: assin, r: r9, linhas: linhasR };
        log("resumo", { linhas: r9.total, frases: r9.frases.length, anomalias: r9.anomaliasTotal || 0 });
        return r9;
      },
      abrirResumo: function (opts) {
        var r9 = api.resumo(opts);
        function monta(frases9) {
          var pn = wrap.querySelector(".phx-resumo");
          if (pn) pn.parentNode.removeChild(pn);
          pn = el("div", "phx-resumo");
          var h9 = '<div class="phx-resumo-cab"><b>\u2728 Resumo do recorte</b><button class="phx-resumo-x" type="button">\u00d7</button></div><ul>';
          var j9;
          for (j9 = 0; j9 < frases9.length; j9++) h9 += "<li>" + esc(frases9[j9]) + "</li>";
          h9 += "</ul>";
          if (r9.tops.length && r9.tops[0].porSoma) {
            h9 += '<div class="phx-resumo-sub">Top ' + esc(r9.tops[0].titulo) + "</div><table class=\"phx-resumo-tab\">";
            for (j9 = 0; j9 < r9.tops[0].porSoma.length; j9++) {
              h9 += "<tr><td>" + esc(r9.tops[0].porSoma[j9].valor) + "</td><td>" + fmt.moeda(r9.tops[0].porSoma[j9].soma) + "</td></tr>";
            }
            h9 += "</table>";
          }
          pn.innerHTML = h9;
          wrap.appendChild(pn);
          pn.querySelector(".phx-resumo-x").addEventListener("click", function () { api.fecharResumo(); });
          anuncia(frases9.join("; "));
        }
        if (cfg.ia && typeof cfg.ia.narrar === "function") cfg.ia.narrar(r9, function (texto9) { monta(texto9 && texto9.length ? (Object.prototype.toString.call(texto9) === "[object Array]" ? texto9 : [String(texto9)]) : r9.frases); });
        else monta(r9.frases);
        return r9;
      },
      fecharResumo: function () {
        var pn = wrap.querySelector(".phx-resumo");
        if (pn) pn.parentNode.removeChild(pn);
        return api;
      },
      filtrarNatural: function (frase, opts) {
        opts = opts || {};
        var ctxN = { colunas: [], distintos: {}, agora: opts.agora || (cfg.ia && cfg.ia.agora) || null };
        var j9, c9;
        for (j9 = 0; j9 < colunasDef.length; j9++) {
          c9 = colunasDef[j9];
          ctxN.colunas.push({ campo: c9.campo, titulo: c9.titulo, tipo: c9.tipo || "texto" });
        }
        if (fonte.local && fonte.todos) {
          for (j9 = 0; j9 < colunasDef.length; j9++) {
            c9 = colunasDef[j9];
            var t9 = c9.tipo || "texto";
            if (t9 === "numero" || t9 === "moeda" || t9 === "percentual" || t9 === "data" || t9 === "dataHora") continue;
            var vis9 = {}, arr9 = [], i9;
            for (i9 = 0; i9 < fonte.todos.length && arr9.length <= 50; i9++) {
              var v9 = fonte.todos[i9][c9.campo];
              if (v9 == null || v9 === "") continue;
              var k9 = String(v9);
              if (!vis9[k9]) { vis9[k9] = 1; arr9.push(k9); }
            }
            if (arr9.length && arr9.length <= 50) ctxN.distintos[c9.campo] = arr9;
          }
        }
        var res = interpretar(frase, ctxN);
        function aplica(resF) {
          if (!opts.somar) { filtros = {}; }
          var j8;
          for (j8 = 0; j8 < resF.filtros.length; j8++) filtros[resF.filtros[j8].campo] = resF.filtros[j8];
          if (resF.busca) filtros["*"] = { campo: "*", tipo: "busca", termo: resF.busca, campos: camposBuscaveis() };
          else if (!opts.somar) delete filtros["*"];
          estado.pagina = 1;
          montaChips();
          carrega();
          log("nl", { frase: String(frase), filtros: resF.filtros.length, busca: resF.busca || null, explicacao: resF.explicacao });
          histNL.unshift({ frase: String(frase), explicacao: resF.explicacao.slice(), filtros: resF.filtros.length, total: null, ts: Date.now() });
          if (histNL.length > 50) histNL.length = 50;
          var entHist = histNL[0];
          setTimeout(function () { entHist.total = estado.total; salvaHist(); }, 0);
          anuncia(resF.explicacao.join("; ") || "nenhum filtro reconhecido");
        }
        if (cfg.ia && typeof cfg.ia.refinar === "function") {
          cfg.ia.refinar(String(frase), ctxN, res, function (resFinal) { aplica(resFinal || res); });
        } else aplica(res);
        return res;
      },
      setTema: function (t) {
        if (t !== "claro" && t !== "escuro" && t !== "auto" && _temasRegistrados[t]) {
          var reg9 = _temasRegistrados[t];
          api.setTema(reg9.base);
          api.limparEstilo();
          api.estilizar(reg9.tokens);
          temaAtual = t;
          log("tema", { tema: t, preset: true });
          return api;
        }
        temaAtual = t || "claro";
        aplicaTemaClasse();
        log("tema", { tema: temaAtual });
        agendaLayoutCb();
        return api;
      },
      tema: function () { return temaAtual; },
      setDensidade: function (dEsc) {
        densAtual = dEsc || "confortavel";
        aplicaDensClasse();
        if (ehVirtual && !(cfgV && cfgV.alturaLinha)) {
          vLinha = ALT_DENS[densAtual] || 36;
          renderJanela(true);
        }
        log("densidade", { densidade: densAtual });
        agendaLayoutCb();
        return api;
      },
      densidade: function () { return densAtual; },
      substituirDados: function (novas9) {
        if (!fonte.local || !fonte.todos) { log("dados", { acao: "substituir-nao-suportado" }); return api; }
        materializaFormulas(novas9 || []);
        fonte.todos.length = 0;
        var j9; for (j9 = 0; j9 < (novas9 || []).length; j9++) fonte.todos.push(novas9[j9]);
        if (fonte.invalidar) fonte.invalidar(null);
        lancMaxNum = null;
        estado.pagina = 1;
        carrega(null, true);
        log("dados", { acao: "substituir", n: (novas9 || []).length });
        return api;
      },
      importarArquivo: function (file9, opts9) {
        opts9 = opts9 || {};
        carregar.arquivo(file9, function (err9, linhas9, meta9) {
          if (err9) { log("importar", { erro: String(err9.message || err9), arquivo: file9 && file9.name }); if (opts9.aoTerminar) opts9.aoTerminar({ ok: false, erro: String(err9.message || err9) }); return; }
          if (opts9.modo === "anexar") api.anexarDados(linhas9); else api.substituirDados(linhas9);
          log("importar", { arquivo: meta9.nome, bytes: meta9.bytes, linhas: linhas9.length, modo: opts9.modo || "substituir" });
          if (opts9.aoTerminar) opts9.aoTerminar({ ok: true, linhas: linhas9.length, arquivo: meta9.nome });
        }, opts9);
        return api;
      },
      anexarDados: function (novas) {
        materializaFormulas(novas);
        if (!novas || !novas.length) return api;
        if (fonte.local && fonte.todos) {
          var j9;
          for (j9 = 0; j9 < novas.length; j9++) fonte.todos.push(novas[j9]);
          if (fonte.invalidar) fonte.invalidar();
          carrega(null, true);
          log("dados", { acao: "anexadas", n: novas.length, total: fonte.todos.length });
          avaliaAlertas(novas);
        } else log("dados", { acao: "anexar-nao-suportado", motivo: "fonte remota/worker: use substituir" });
        return api;
      },
      redesenhar: function () { carrega(null, true); return api; },
      destruir: function () {
        var j9;
        for (j9 = 0; j9 < ouvintesDoc.length; j9++) document.removeEventListener(ouvintesDoc[j9][0], ouvintesDoc[j9][1], ouvintesDoc[j9][2]);
        ouvintesDoc.length = 0;
        if (_fechaSaida && root.removeEventListener) { root.removeEventListener("beforeunload", _fechaSaida); root.removeEventListener("pagehide", _fechaSaida); _fechaSaida = null; }
        if (_fechaVisib) { document.removeEventListener("visibilitychange", _fechaVisib); _fechaVisib = null; }
        api.fecharMenuContexto();
        if (layoutTimer) { clearTimeout(layoutTimer); layoutTimer = null; }
        if (_dicaRolTimer) { clearTimeout(_dicaRolTimer); _dicaRolTimer = null; }
        _destruido = true;
        cacheV = null;
        ultimaCarga = null;
        if (mqEscuro && mqHandler) {
          if (mqEscuro.removeEventListener) mqEscuro.removeEventListener("change", mqHandler);
          else if (mqEscuro.removeListener) mqEscuro.removeListener(mqHandler);
        }
        if (fonte && fonte.destruir) fonte.destruir();
        if (wrap.parentNode) wrap.parentNode.removeChild(wrap);
      }
    };

    var npop = el("div", "phx-npop");
    npop.hidden = true;
    wrap.appendChild(npop);
    var npopCtx = null;
    function fechaNpop() { npop.hidden = true; npopCtx = null; }
    function abreNpop(chave2, campo2, ancora) {
      npopCtx = { chave: chave2, campo: campo2 };
      var nAtual = (notas[chave2] && notas[chave2][campo2] && notas[chave2][campo2].texto) || "";
      npop.innerHTML = '<div class="phx-npop-titulo">Nota \u2014 ' + esc((porCampo[campo2] && porCampo[campo2].titulo) || campo2) + "</div>" +
        '<textarea class="phx-npop-txt" rows="4">' + esc(nAtual) + "</textarea>" +
        '<div class="phx-npop-acoes">' +
        '<button type="button" class="phx-npop-salvar">Salvar</button>' +
        (nAtual ? '<button type="button" class="phx-npop-remover">Remover</button>' : "") +
        '<button type="button" class="phx-npop-fechar">Fechar</button></div>';
      npop.hidden = false;
      var r = ancora.getBoundingClientRect(), rw = wrap.getBoundingClientRect();
      npop.style.left = Math.max(8, Math.min(r.left - rw.left, wrap.clientWidth - 280)) + "px";
      npop.style.top = (r.bottom - rw.top + 6) + "px";
      npop.querySelector(".phx-npop-salvar").addEventListener("click", function () {
        api.nota(npopCtx.chave, npopCtx.campo, npop.querySelector(".phx-npop-txt").value);
        fechaNpop();
      });
      var btnRem = npop.querySelector(".phx-npop-remover");
      if (btnRem) btnRem.addEventListener("click", function () {
        api.nota(npopCtx.chave, npopCtx.campo, null);
        fechaNpop();
      });
      npop.querySelector(".phx-npop-fechar").addEventListener("click", fechaNpop);
      npop.querySelector(".phx-npop-txt").focus();
      log("nota", { chave: chave2, campo: campo2, acao: "aberta" });
    }
    function alvoCelula(e) {
      var td = e.target.closest ? e.target.closest("td.phx-td") : null;
      if (!td) return null;
      var cls0 = td.className;
      if (cls0.indexOf("phx-td-sel") >= 0 || cls0.indexOf("phx-td-exp") >= 0 || cls0.indexOf("phx-grupo-td") >= 0 || cls0.indexOf("phx-detalhe-td") >= 0) return null;
      var tr2 = td.parentNode;
      if (tr2.className.indexOf("phx-grupo") >= 0 || tr2.className.indexOf("phx-detalhe") >= 0) return null;
      var ic = -1, j2, seen = 0;
      for (j2 = 0; j2 < tr2.children.length; j2++) {
        var cls2 = tr2.children[j2].className;
        if (cls2.indexOf("phx-td-sel") >= 0 || cls2.indexOf("phx-td-exp") >= 0) continue;
        if (tr2.children[j2] === td) { ic = seen; break; }
        seen++;
      }
      if (ic < 0) return null;
      var contData = -1, j3, trs2 = tbody.children, achou = false;
      for (j3 = 0; j3 < trs2.length; j3++) {
        var cls3 = trs2[j3].className;
        if (cls3.indexOf("phx-grupo") >= 0 || cls3.indexOf("phx-detalhe") >= 0) continue;
        contData++;
        if (trs2[j3] === tr2) { achou = true; break; }
      }
      if (!achou) return null;
      var dataIx = [], j4;
      for (j4 = 0; j4 < (ultimaCarga ? ultimaCarga.linhas.length : 0); j4++) if (!ultimaCarga.linhas[j4].__grupo) dataIx.push(j4);
      if (contData >= dataIx.length) return null;
      var ixL2 = dataIx[contData];
      var vCols = visiveis(), c2 = vCols[ic];
      if (!c2) return null;
      return { td: td, ixL: ixL2, col: c2 };
    }
    function abreEditor(alvoC) {
      if (alvoC && alvoC.col && (alvoC.col.formula != null || alvoC.col.acumulado)) { log("edit-bloqueado", { campo: alvoC.col.campo, motivo: "coluna calculada" }); return; }
      if (alvoC && alvoC.col && regrasAcesso && !acessoDe(alvoC.col.campo).editar) { log("acesso-negado", { campo: alvoC.col.campo, papel: papelAtual, acao: "editar" }); return; }
      if (edicaoAtiva) fechaEditor(false);
      var c2 = alvoC.col, linha2 = ultimaCarga.linhas[alvoC.ixL];
      var _maxLen9 = c2.tamanhoMax || 0;
      var valAtual = linha2[c2.campo];
      var tipoEd = c2.editor || ((c2.tipo === "numero" || c2.tipo === "moeda" || c2.tipo === "percentual") ? "numero" : (c2.tipo === "data" ? "data" : (c2.opcoes ? "select" : "texto")));
      var ed;
      if (tipoEd === "select") {
        ed = document.createElement("select");
        ed.className = "phx-editor";
        if (_maxLen9) ed.maxLength = _maxLen9;
        var j5, op;
        for (j5 = 0; j5 < (c2.opcoes || []).length; j5++) {
          op = document.createElement("option");
          op.value = c2.opcoes[j5];
          op.textContent = c2.opcoes[j5];
          if (String(valAtual) === String(c2.opcoes[j5])) op.selected = true;
          ed.appendChild(op);
        }
      } else {
        ed = document.createElement("input");
        ed.className = "phx-editor";
        if (_maxLen9) ed.maxLength = _maxLen9;
        ed.type = tipoEd === "data" ? "date" : "text";
        ed.value = tipoEd === "data" ? String(valAtual || "").slice(0, 10) : (valAtual == null ? "" : String(valAtual));
      }
      alvoC.td.innerHTML = "";
      alvoC.td.appendChild(ed);
      alvoC.td.className += " phx-td-editando";
      edicaoAtiva = { ixL: alvoC.ixL, campo: c2.campo, col: c2, td: alvoC.td, linha: linha2, antigo: valAtual, ed: ed };
      ed.focus();
      if (ed.select) try { ed.select(); } catch (e9) {}
      if (c2.editorGrid && typeof dropdown === "function") {
        /* EditType "dropdown-grid": o proprio PhxGrid.dropdown vira o editor da celula.
           O input ja existe e ja esta commitado ao fluxo normal (Enter/blur/Esc) --
           so precisamos preencher .value quando o usuario escolhe uma linha no picker.
           Limpa o valor antes de abrir: senao o dropdown filtra pelo valor antigo da
           celula (comportamento correto dele como autocomplete, errado aqui -- o editor
           deve abrir mostrando todas as opcoes). */
        ed.value = "";
        var ddEd9 = dropdown(ed, {
          chave: c2.editorGrid.chave, campoTexto: c2.editorGrid.campoTexto || c2.campo, campoValor: c2.editorGrid.campoValor || c2.editorGrid.chave,
          colunas: c2.editorGrid.colunas, dados: c2.editorGrid.dados, fonte: c2.editorGrid.fonte, tamanho: c2.editorGrid.tamanho, largura: c2.editorGrid.largura,
          aoEscolher: function (v9) { ed.value = v9 == null ? "" : String(v9); fechaEditor(true); }
        });
        if (ddEd9 && ddEd9.ok) { ddEd9.abrir(); edicaoAtiva.dropdownGrid = ddEd9; }
      }
      ed.addEventListener("keydown", function (e2) {
          if (e2.key === "Enter") {
          e2.preventDefault();
          var okE = fechaEditor(true);
          if (okE !== false && temLancamento) avancaLanc(alvoC.ixL, c2.campo);
        }
        else if (e2.key === "Escape") { e2.preventDefault(); fechaEditor(false); }
        else if (e2.key === "Tab") {
          e2.preventDefault();
          var dir = e2.shiftKey ? -1 : 1, ok2 = fechaEditor(true);
          if (ok2 === false) return;
          var abriu2 = abreProxima(alvoC.ixL, c2.campo, dir);
          if (!abriu2 && dir === 1 && temLancamento) avancaLanc(alvoC.ixL, c2.campo);
        }
        else if ((e2.key === "ArrowDown" || e2.key === "ArrowUp") && temLancamento) {
          e2.preventDefault();
          var okA = fechaEditor(true);
          if (okA === false) return;
          var alvoIx = alvoC.ixL + (e2.key === "ArrowDown" ? 1 : -1);
          if (alvoIx >= 0 && alvoIx < ultimaCarga.linhas.length) abreProxima(alvoIx, c2.campo, 0);
        }
      });
      ed.addEventListener("blur", function () { if (edicaoAtiva && edicaoAtiva.ed === ed) fechaEditor(true); });
      log("edit-abre", { campo: c2.campo });
    }
    function abreProxima(ixL2, campoAtual, dir) {
      var vCols = visiveis(), at = -1, j6;
      for (j6 = 0; j6 < vCols.length; j6++) if (vCols[j6].campo === campoAtual) { at = j6; break; }
      var ini6 = dir === 0 ? at : at + dir;
      var passo6 = dir === 0 ? 1 : dir;
      for (j6 = ini6; j6 >= 0 && j6 < vCols.length; j6 += passo6) {
        if (vCols[j6].editavel) {
          var tds6 = [], tr6 = null, trs6 = tbody.children, cont6 = -1, j7;
          for (j7 = 0; j7 < trs6.length; j7++) {
            var cls6 = trs6[j7].className;
            if (cls6.indexOf("phx-grupo") >= 0 || cls6.indexOf("phx-detalhe") >= 0) continue;
            cont6++;
            var dataIx6 = [], j8;
            for (j8 = 0; j8 < ultimaCarga.linhas.length; j8++) if (!ultimaCarga.linhas[j8].__grupo) dataIx6.push(j8);
            if (dataIx6[cont6] === ixL2) { tr6 = trs6[j7]; break; }
          }
          if (!tr6) return;
          var seen6 = 0, alvoTd = null;
          for (j7 = 0; j7 < tr6.children.length; j7++) {
            var cls7 = tr6.children[j7].className;
            if (cls7.indexOf("phx-td-sel") >= 0 || cls7.indexOf("phx-td-exp") >= 0) continue;
            if (seen6 === j6) { alvoTd = tr6.children[j7]; break; }
            seen6++;
          }
          if (alvoTd) { abreEditor({ td: alvoTd, ixL: ixL2, col: vCols[j6] }); return true; }
          return false;
        }
        if (dir === 0) return false;
      }
      return false;
    }
    function validaLinhaLanc(linha9, semRender9) {
      if (!temLancamento) return true;
      var ch9 = chaveDe(linha9, -1);
      var r9 = true;
      if (lancCfg.validar) {
        try { r9 = lancCfg.validar(linha9); } catch (e9) { r9 = "erro na valida\u00e7\u00e3o: " + e9.message; }
      }
      if (r9 === true || r9 == null) {
        if (lancInvalidas[ch9]) { delete lancInvalidas[ch9]; if (!semRender9) carrega(null, true); }
        log("lanc-commit", { chave: ch9, linha: linha9 });
        return true;
      }
      lancInvalidas[ch9] = String(r9);
      log("lanc-invalida", { chave: ch9, motivo: String(r9) });
      if (!semRender9) carrega(null, true);
      return false;
    }
    function avancaLanc(ixL9, campo9) {
      if (abreProxima(ixL9, campo9, 1)) return;
      /* fim da linha: valida e segue */
      var linha9 = null, j9;
      var td9 = fonte.local && fonte.todos ? fonte.todos : [];
      for (j9 = 0; j9 < ultimaCarga.linhas.length; j9++) if (j9 === ixL9 && !ultimaCarga.linhas[j9].__grupo) { linha9 = ultimaCarga.linhas[j9]; break; }
      if (linha9 && !validaLinhaLanc(linha9)) return;
      var totalD9 = ultimaCarga.totalDados != null ? ultimaCarga.totalDados : ultimaCarga.total;
      var posGlobal9 = (estado.pagina - 1) * estado.tamanho + ixL9;
      if (posGlobal9 >= totalD9 - 1) { api.incluirLinha(); return; }
      if (ixL9 + 1 >= ultimaCarga.linhas.length) { api.pagina(estado.pagina + 1); abreProxima(0, "\u0000", 1); return; }
      abreProxima(ixL9 + 1, "\u0000", 1);
    }
    function encerraEditorSilencioso() {
      if (!edicaoAtiva) return;
      var ctx9 = edicaoAtiva;
      edicaoAtiva = null;
      var bruto9 = ctx9.ed ? ctx9.ed.value : null;
      if (bruto9 == null || String(bruto9) === "") return;
      var pp9 = parseEntrada(ctx9.col, bruto9);
      if (!pp9.ok || pp9.valor === ctx9.antigo) return;
      var ch9 = chaveDe(ctx9.linha, ctx9.ixL);
      if (aoEditar && aoEditar(ch9, ctx9.campo, pp9.valor, ctx9.antigo, ctx9.linha) === false) return;
      registraHist(ch9, ctx9.campo, ctx9.antigo, pp9.valor);
      ctx9.linha[ctx9.campo] = pp9.valor;
      materializaFormulas([ctx9.linha]);
      log("edit", { chave: ch9, campo: ctx9.campo, de: ctx9.antigo, para: pp9.valor, via: "render" });
    }
    function fechaEditor(commit) {
      if (!edicaoAtiva) return;
      var ctx = edicaoAtiva;
      edicaoAtiva = null;
      if (!commit) { reRender(); return; }
      var bruto = ctx.ed.value;
      if (String(bruto) === "" && ctx.antigo == null) { reRender(); return; }
      var p9 = parseEntrada(ctx.col, bruto);
      var chE = chaveDe(ctx.linha, ctx.ixL);
      if (!p9.ok) {
        reRender();
        nackCelula(ctx.ixL, ctx.campo);
        log("edit-rejeitado", { chave: chE, campo: ctx.campo, motivo: "parse", bruto: String(bruto) });
        return false;
      }
      if (p9.valor === ctx.antigo || (p9.valor === "" && ctx.antigo == null)) { reRender(); return; }
      if (aoEditar && aoEditar(chE, ctx.campo, p9.valor, ctx.antigo, ctx.linha) === false) {
        reRender();
        nackCelula(ctx.ixL, ctx.campo);
        log("edit-rejeitado", { chave: chE, campo: ctx.campo, motivo: "aoEditar", de: ctx.antigo, para: p9.valor });
        return false;
      }
      registraHist(chE, ctx.campo, ctx.antigo, p9.valor);
      ctx.linha[ctx.campo] = p9.valor;
      materializaFormulas([ctx.linha]);
      if (fonte.invalidar) fonte.invalidar(ctx.campo);
      log("edit", { chave: chE, campo: ctx.campo, de: ctx.antigo, para: p9.valor });
      carrega(null, true);
    }
    function nackCelula(ixL2, campo2) {
      var vCols = visiveis(), ic2 = -1, j9;
      for (j9 = 0; j9 < vCols.length; j9++) if (vCols[j9].campo === campo2) { ic2 = j9; break; }
      if (ic2 < 0) return;
      var dataIx9 = [], j10;
      for (j10 = 0; j10 < ultimaCarga.linhas.length; j10++) if (!ultimaCarga.linhas[j10].__grupo) dataIx9.push(j10);
      var pos = -1;
      for (j10 = 0; j10 < dataIx9.length; j10++) if (dataIx9[j10] === ixL2) { pos = j10; break; }
      var trsD = [], trs9 = tbody.children;
      for (j10 = 0; j10 < trs9.length; j10++) {
        var cls9 = trs9[j10].className;
        if (cls9.indexOf("phx-grupo") >= 0 || cls9.indexOf("phx-detalhe") >= 0) continue;
        trsD.push(trs9[j10]);
      }
      if (pos < 0 || !trsD[pos]) return;
      var seen9 = 0, tdN = null;
      for (j10 = 0; j10 < trsD[pos].children.length; j10++) {
        var cls10 = trsD[pos].children[j10].className;
        if (cls10.indexOf("phx-td-sel") >= 0 || cls10.indexOf("phx-td-exp") >= 0) continue;
        if (seen9 === ic2) { tdN = trsD[pos].children[j10]; break; }
        seen9++;
      }
      if (!tdN) return;
      tdN.className += " phx-edit-nack";
      setTimeout(function () { tdN.className = tdN.className.replace(" phx-edit-nack", ""); }, 650);
    }
    tbody.addEventListener("dblclick", function (e) {
      var alvoC = alvoCelula(e);
      if (!alvoC) return;
      if (temEdicao && alvoC.col.editavel) { abreEditor(alvoC); return; }
      if (temNotas) abreNpop(chaveDe(ultimaCarga.linhas[alvoC.ixL], alvoC.ixL), alvoC.col.campo, alvoC.td);
    });
    if (temNotas) {
      tbody.addEventListener("click", function (e) {
        if (e.target.className !== "phx-nota-ind") return;
        e.stopPropagation();
        var ind = e.target, ixL3 = parseInt(ind.getAttribute("data-nl"), 10);
        abreNpop(chaveDe(ultimaCarga.linhas[ixL3], ixL3), ind.getAttribute("data-nc"), ind.parentNode);
      }, true);
      docOn("keydown", function (e) { if (e.key === "Escape") fechaNpop(); });
      docOn("mousedown", function (e) { if (!npop.hidden && !npop.contains(e.target) && e.target.className !== "phx-nota-ind") fechaNpop(); });
    }
    if (cfg.alertas && typeof cfg.alertas.carregar === "function") {
      try {
        var _sem = cfg.alertas.carregar() || [], j7;
        for (j7 = 0; j7 < _sem.length; j7++) {
          var e7 = _sem[j7];
          var r7 = interpretar(e7.frase, _ctxNL());
          _regras.push({ id: e7.id || (++_seqRegra), frase: e7.frase, filtros: r7.filtros, busca: r7.busca, cbNome: e7.cbNome || null, aoDisparar: e7.cbNome ? _resolveFn(e7.cbNome) : null, soNovas: e7.soNovas !== false });
          if (e7.id && e7.id > _seqRegra) _seqRegra = e7.id;
        }
      } catch (e8) {}
    }
    liveEl = el("div", "phx-live");
    liveEl.setAttribute("aria-live", "polite");
    liveEl.setAttribute("role", "status");
    wrap.appendChild(liveEl);
    tabela.setAttribute("role", "grid");
    tabela.setAttribute("aria-colcount", String(colCount()));
    if (temSelecao) tabela.setAttribute("aria-multiselectable", "true");
    aplicaTemaClasse();
    aplicaDensClasse();
    if (temaAtual === "auto" && root.matchMedia) {
      mqEscuro = root.matchMedia("(prefers-color-scheme: dark)");
      mqHandler = function () { aplicaTemaClasse(); };
      if (mqEscuro.addEventListener) mqEscuro.addEventListener("change", mqHandler);
      else if (mqEscuro.addListener) mqEscuro.addListener(mqHandler);
      aplicaTemaClasse();
    }
    wrap.addEventListener("contextmenu", function (ev9) {
      if (cfg.menuContexto === false) return;
      var alvo9 = ev9.target;
      while (alvo9 && alvo9 !== wrap && alvo9.tagName !== "TD" && alvo9.tagName !== "TH") alvo9 = alvo9.parentNode;
      if (!alvo9 || alvo9 === wrap) return;
      var campo9 = null, linha9 = null, ixL9 = -1;
      if (alvo9.tagName === "TH") campo9 = alvo9.getAttribute("data-campo");
      else {
        var tr9 = alvo9.parentNode;
        var trs9 = [], j9, filhos9 = tr9.parentNode.children;
        for (j9 = 0; j9 < filhos9.length; j9++) if (filhos9[j9].className.indexOf("phx-vspacer") < 0 && filhos9[j9].className.indexOf("phx-grupo") < 0) trs9.push(filhos9[j9]);
        ixL9 = trs9.indexOf(tr9);
        var vis9 = visiveis();
        var ixC9 = alvo9.cellIndex - (temSelecao ? 1 : 0) - (temDetalhe ? 1 : 0);
        if (vis9[ixC9]) campo9 = vis9[ixC9].campo;
        if (ultimaCarga && ultimaCarga.linhas && ixL9 >= 0) linha9 = ultimaCarga.linhas[ixL9];
      }
      if (!campo9 && !linha9) return;
      ev9.preventDefault();
      var r9 = wrap.getBoundingClientRect ? wrap.getBoundingClientRect() : { left: 0, top: 0 };
      api.abrirMenuContexto({ campo: campo9, linha: linha9, ixL: ixL9 }, (ev9.clientX || 0) - r9.left, (ev9.clientY || 0) - r9.top);
    });
    var _saiuSalvo = false;
    function _salvaAoSair(motivo9) {
      if (_saiuSalvo) return;
      if (!aoMudarLayout && !(cfg.layout && typeof cfg.layout.salvar === "function")) return;
      _saiuSalvo = true;
      api.salvarLayoutAgora(motivo9);
      setTimeout(function () { _saiuSalvo = false; }, 400);
    }
    if (cfg.salvarLayoutAoSair !== false) {
      _fechaSaida = function () { _salvaAoSair("saida"); };
      if (root.addEventListener) {
        root.addEventListener("beforeunload", _fechaSaida);
        root.addEventListener("pagehide", _fechaSaida);
      }
      if (document.addEventListener) {
        _fechaVisib = function () { if (document.visibilityState === "hidden") _salvaAoSair("oculto"); };
        document.addEventListener("visibilitychange", _fechaVisib);
      }
    }
    function atualizaNav() {
      if (!temNavegador) return;
      var nI9 = wrap.querySelector(".phx-nav-n"), de9 = wrap.querySelector(".phx-nav-de");
      if (nI9) nI9.value = registroAtual || (estado.total ? 1 : 0);
      if (de9) de9.textContent = " " + T("de") + " " + fmt.numero(estado.total || 0);
    }
    if (temNavegador) {
      var bs9 = wrap.querySelectorAll(".phx-nav-b"), q9;
      for (q9 = 0; q9 < bs9.length; q9++) (function (b9) {
        b9.addEventListener("click", function () {
          var a9 = b9.getAttribute("data-nav");
          if (a9 === "primeiro") api.irRegistro(1); else if (a9 === "ultimo") api.irRegistro(estado.total); else if (a9 === "prox") api.proximoRegistro(); else api.anteriorRegistro();
        });
      })(bs9[q9]);
      var nIn9 = wrap.querySelector(".phx-nav-n");
      if (nIn9) nIn9.addEventListener("change", function () { api.irRegistro(nIn9.value); });
    }
    if (cfg.arrastarLinhas) {
      arrastePonteiro(tbody, {
        forcarMouse: true,
        rotulo: function () { return "\u2195"; },
        aoIniciar: function () {},
        aoSoltar: function (sob, p) {
          var trAlvo9 = sobe(sob, "phx-tr-arr") || (sob && sob.closest ? sob.closest("tbody tr") : null);
          var trDe9 = _trArrasto;
          _trArrasto = null;
          if (!trDe9 || !trAlvo9 || trDe9 === trAlvo9 || trAlvo9.parentNode !== tbody) return;
          var ixDe9 = Array.prototype.indexOf.call(tbody.children, trDe9), ixPara9 = Array.prototype.indexOf.call(tbody.children, trAlvo9);
          var lDe9 = ultimaCarga.linhas[ixDe9], lPara9 = ultimaCarga.linhas[ixPara9];
          if (!lDe9 || !lPara9 || lDe9.__grupo || lPara9.__grupo) return;
          if (typeof cfg.aoMoverLinha === "function" && cfg.aoMoverLinha(chaveDe(lDe9, ixDe9), chaveDe(lPara9, ixPara9), lDe9, lPara9) === false) return;
          if (fonte.local && fonte.todos) {
            var iD9 = fonte.todos.indexOf(lDe9), iP9 = fonte.todos.indexOf(lPara9);
            if (iD9 >= 0 && iP9 >= 0) { fonte.todos.splice(iD9, 1); fonte.todos.splice(iP9 > iD9 ? iP9 - 1 : iP9, 0, lDe9); if (fonte.invalidar) fonte.invalidar(null); carrega(null, true); }
          }
          log("linha-movida", { de: chaveDe(lDe9, ixDe9), para: chaveDe(lPara9, ixPara9) });
        }
      });
      tbody.addEventListener("mousedown", function (ev9) { var tr9 = ev9.target && ev9.target.closest ? ev9.target.closest("tr") : null; _trArrasto = tr9 && tr9.parentNode === tbody ? tr9 : null; });
      tbody.addEventListener("touchstart", function (ev9) { var tr9 = ev9.target && ev9.target.closest ? ev9.target.closest("tr") : null; _trArrasto = tr9 && tr9.parentNode === tbody ? tr9 : null; }, { passive: true });
    }
    wrap.addEventListener("keydown", function (ev9) {
      if (ev9.key === "Delete" && cfg.excluir && !edicaoAtiva && !/^(INPUT|TEXTAREA|SELECT)$/.test(ev9.target.tagName)) { ev9.preventDefault(); api.excluirRegistro(); }
    });
    if (cfg.esconderSelecaoSemFoco) {
      envoltorio.addEventListener("focusout", function () { if (wrap.className.indexOf("phx-sem-foco") < 0) wrap.className += " phx-sem-foco"; });
      envoltorio.addEventListener("focusin", function () { wrap.className = wrap.className.replace(/ phx-sem-foco/g, ""); });
    }
    envoltorio.setAttribute("tabindex", "0");
    envoltorio.addEventListener("keydown", function (e) {
      var k2 = e.key, trs;
      if (edicaoAtiva) return;
      if ((e.ctrlKey || e.metaKey) && (e.key === "c" || e.key === "C")) {
        if (cfg.clipboard === false) return;
        e.preventDefault();
        api.copiarParaAreaTransferencia();
        return;
      }
      var NAV = { ArrowRight: 1, ArrowLeft: 1, ArrowDown: 1, ArrowUp: 1, Home: 1, End: 1, PageDown: 1, PageUp: 1 };
      if (!rovingAtivo && NAV[k2]) rovingAtivo = true;
      if (k2 === "ArrowRight") { focoC++; aplicaFoco(); }
      else if (k2 === "ArrowLeft") { focoC--; aplicaFoco(); }
      else if (k2 === "ArrowDown") {
        trs = trsDados();
        if (ehVirtual) {
          var gAtual = vIni + focoR;
          if (gAtual + 1 < (cacheV ? cacheV.length : 0)) {
            if (focoR + 1 >= trs.length - 1) { api.rolarPara(gAtual + 1); focoR = (gAtual + 1) - vIni; }
            else focoR++;
          }
        } else if (focoR + 1 < trs.length) focoR++;
        aplicaFoco();
      }
      else if (k2 === "ArrowUp") {
        if (ehVirtual) {
          var gA2 = vIni + focoR;
          if (gA2 > 0) {
            if (focoR <= 1 && vIni > 0) { api.rolarPara(gA2 - 1); focoR = (gA2 - 1) - vIni; }
            else focoR--;
          }
        } else if (focoR > 0) focoR--;
        aplicaFoco();
      }
      else if (k2 === "Home") { if (e.ctrlKey) { focoR = 0; if (ehVirtual) { api.rolarPara(0); focoR = 0; } } focoC = 0; aplicaFoco(); }
      else if (k2 === "End") {
        if (e.ctrlKey) { trs = trsDados(); focoR = trs.length - 1; if (ehVirtual && cacheV) { api.rolarPara(cacheV.length - 1); focoR = (cacheV.length - 1) - vIni; } }
        focoC = colCount() - 1; aplicaFoco();
      }
      else if (k2 === "PageDown") {
        if (ehVirtual && cacheV) { var alvoG = Math.min(cacheV.length - 1, vIni + focoR + vNVis); api.rolarPara(alvoG); focoR = alvoG - vIni; aplicaFoco(); }
        else { var pAt = estado.pagina; api.pagina(pAt + 1); if (estado.pagina !== pAt) { focoR = 0; aplicaFoco(); anuncia("P\u00e1gina " + estado.pagina); } }
      }
      else if (k2 === "PageUp") {
        if (ehVirtual && cacheV) { var alvoG2 = Math.max(0, vIni + focoR - vNVis); api.rolarPara(alvoG2); focoR = alvoG2 - vIni; aplicaFoco(); }
        else { var pAt2 = estado.pagina; api.pagina(pAt2 - 1); if (estado.pagina !== pAt2) { focoR = 0; aplicaFoco(); anuncia("P\u00e1gina " + estado.pagina); } }
      }
      else if (k2 === "Enter" || k2 === "F2") {
        var cel2 = celDe(focoR, focoC);
        if (cel2 && temEdicao) {
          var vCols2 = visiveis(), icE = focoC - extrasN();
          if (icE >= 0 && vCols2[icE] && vCols2[icE].editavel) {
            var linhas2 = ehVirtual ? cacheV : (ultimaCarga ? ultimaCarga.linhas : []);
            var dataIxE = [], j5;
            for (j5 = 0; j5 < linhas2.length; j5++) if (!linhas2[j5].__grupo) dataIxE.push(j5);
            var ixLE = ehVirtual ? vIni + focoR : dataIxE[focoR];
            if (ixLE != null) abreEditor({ td: cel2, ixL: ixLE, col: vCols2[icE] });
          }
        }
      }
      else if (k2 === " " || k2 === "Spacebar") {
        if (temSelecao) {
          e.preventDefault();
          var trsS = trsDados(), trF = trsS[focoR];
          if (trF) { var cb2 = trF.querySelector('td.phx-td-sel input'); if (cb2) cb2.click(); }
        }
        return;
      }
      else return;
      if (k2 !== "Enter" && k2 !== "F2") e.preventDefault();
    });
    tbody.addEventListener("click", function (e) {
      var td9 = e.target.closest ? e.target.closest("td") : null;
      if (!td9) return;
      var tr9 = td9.parentNode, trs9 = trsDados(), j9;
      for (j9 = 0; j9 < trs9.length; j9++) if (trs9[j9] === tr9) { focoR = j9; break; }
      var kids = tr9.children;
      for (j9 = 0; j9 < kids.length; j9++) if (kids[j9] === td9) { focoC = j9; break; }
    }, true);
    if (ehVirtual) {
      envoltorio.style.height = vAltura + "px";
      envoltorio.addEventListener("scroll", aoScrollV);
      wrap.className += " phx-virtual";
      var rodapeV = wrap.querySelector(".phx-rodape");
      if (rodapeV) {
        var pagV = rodapeV.querySelector(".phx-pag");
        if (pagV) pagV.style.display = "none";
        var tamV = rodapeV.querySelector(".phx-tam");
        if (tamV) tamV.parentNode.style.display = "none";
      }
    }
    no.appendChild(wrap);
    montaGroupBox();
    montaBusca();
    if (cfg.cabecalhoRelevo) api.cabecalhoRelevo(cfg.cabecalhoRelevo);
    if (cfg.cabecalho === false) wrap.className += " phx-sem-cabecalho";
    if (cfg.colunasFixas) { var cf9 = Math.min(cfg.colunasFixas, colunasDef.length), cfi9; for (cfi9 = 0; cfi9 < cf9; cfi9++) colunasDef[cfi9].fixa = colunasDef[cfi9].fixa || "esq"; }
    if (cfg.gruposRecolhidos && cfg.grupos && cfg.grupos.length) { /* aplicado apos a 1a carga, quando os paths dos grupos existem */ }
    if (cfg.alturaLinha) api.alturaLinha(cfg.alturaLinha);
    if (cfg.soltarArquivos) {
      wrap.addEventListener("dragover", function (ev9) { if (ev9.dataTransfer && (ev9.dataTransfer.types || []).length) { ev9.preventDefault(); if (wrap.className.indexOf("phx-solta") < 0) wrap.className += " phx-solta"; } });
      wrap.addEventListener("dragleave", function () { wrap.className = wrap.className.replace(/ phx-solta/g, ""); });
      wrap.addEventListener("drop", function (ev9) {
        wrap.className = wrap.className.replace(/ phx-solta/g, "");
        var dt9 = ev9.dataTransfer;
        if (!dt9) return;
        var modo9 = cfg.soltarArquivos === "substituir" ? "substituir" : "anexar";
        if (dt9.files && dt9.files.length) {
          ev9.preventDefault();
          var q9; for (q9 = 0; q9 < dt9.files.length; q9++) api.importarArquivo(dt9.files[q9], { modo: modo9 });
          log("soltar", { arquivos: dt9.files.length, modo: modo9 });
          return;
        }
        var txt9 = dt9.getData ? dt9.getData("text/plain") : "";
        if (txt9 && txt9.indexOf("\t") >= 0 && api.colarTSV) { ev9.preventDefault(); api.colarTSV(txt9); log("soltar", { texto: txt9.length, modo: "colar" }); }
      });
    }
    if (cfg.redimensionarLinhas) {
      wrap.setAttribute("data-redim-linhas", "1");
      var _rz9 = null;
      tbody.addEventListener("mousedown", function (ev9) {
        var tdI9 = ev9.target && ev9.target.closest ? ev9.target.closest("td.phx-td-ind") : null;
        if (!tdI9) return;
        var r9 = tdI9.getBoundingClientRect();
        if (ev9.clientY < r9.bottom - 5) return;
        ev9.preventDefault();
        _rz9 = { y0: ev9.clientY, h0: r9.height };
        function mv9(e2) { if (!_rz9) return; api.alturaLinha(Math.max(20, Math.round(_rz9.h0 + (e2.clientY - _rz9.y0)))); }
        function up9() { _rz9 = null; document.removeEventListener("mousemove", mv9); document.removeEventListener("mouseup", up9); }
        document.addEventListener("mousemove", mv9);
        document.addEventListener("mouseup", up9);
      });
    }
    (function () {
      var k9, j9;
      for (k9 in (cfg.estilos || {})) api.definirEstilo(k9, cfg.estilos[k9]);
      for (j9 = 0; j9 < (cfg.condicoes || []).length; j9++) api.adicionarCondicao(cfg.condicoes[j9]);
    })();
    carrega(function () {
      if (cfg.grupos && cfg.grupos.length) api.agrupar(cfg.grupos);
      montaColSel();
      if (cfg.autoLargura) api.ajustarColunas({ modo: typeof cfg.autoLargura === "string" ? cfg.autoLargura : "conteudo", amostra: cfg.autoLarguraAmostra || 300 });
      if (adaptativo) api.adaptar();
      log("init", { linhas: estado.total, colunas: colunasDef.length, ms: Math.round((agora() - t0init) * 10) / 10 });
    });
    return api;
  }

  /* ===== S10: PIVOT MATRIZ ===== */
  function novoAcum() { return { s: 0, c: 0, mn: Infinity, mx: -Infinity }; }
  function acumula(a, v) {
    var n = Number(v);
    if (n !== n) return;
    a.s += n; a.c++;
    if (n < a.mn) a.mn = n;
    if (n > a.mx) a.mx = n;
  }
  function fechaAcum(a, agg) {
    if (!a || !a.c) return null;
    if (agg === "sum") return a.s;
    if (agg === "count") return a.c;
    if (agg === "avg") return a.s / a.c;
    if (agg === "min") return a.mn;
    if (agg === "max") return a.mx;
    return a.s;
  }
  function pivotBuild(dados, cfgP) {
    var t0 = agora();
    var rf = cfgP.linhas, cf = cfgP.colunas[0], med = cfgP.medidas;
    var nM = med.length, i2, m2;
    var colMapa = {}, colLista = [];
    var tipoCol = cfgP.tipoDe(cf);
    for (i2 = 0; i2 < dados.length; i2++) {
      var vc = dados[i2][cf], kc = vc == null || vc === "" ? "\u0000vazio" : String(vc);
      if (!colMapa[kc]) { colMapa[kc] = { chave: kc, valor: vc == null || vc === "" ? null : vc }; colLista.push(colMapa[kc]); }
    }
    colLista.sort(function (a, b) {
      if (a.valor == null && b.valor == null) return 0;
      if (a.valor == null) return 1;
      if (b.valor == null) return -1;
      var ka = chaveOrd(a.valor, tipoCol), kb = chaveOrd(b.valor, tipoCol);
      if (ka.v < kb.v) return -1;
      if (ka.v > kb.v) return 1;
      return 0;
    });
    function novoAcums() { var o = [], m3; for (m3 = 0; m3 < nM; m3++) o.push(novoAcum()); return o; }
    var raiz = { filhos: {}, ordem: [], cel: {}, tot: novoAcums(), path: "", nivel: -1, campo: null, valor: null, n: 0 };
    function celDe(no2, kc) { if (!no2.cel[kc]) no2.cel[kc] = novoAcums(); return no2.cel[kc]; }
    var totCol = {};
    var totGeral = novoAcums();
    for (i2 = 0; i2 < dados.length; i2++) {
      var linha = dados[i2];
      var vc2 = linha[cf], kc2 = vc2 == null || vc2 === "" ? "\u0000vazio" : String(vc2);
      if (!totCol[kc2]) totCol[kc2] = novoAcums();
      var no2 = raiz, d2;
      for (d2 = 0; d2 < rf.length; d2++) {
        var vr = linha[rf[d2]], kr = vr == null || vr === "" ? "\u0000vazio" : String(vr);
        if (!no2.filhos[kr]) {
          no2.filhos[kr] = { campo: rf[d2], valor: vr == null || vr === "" ? null : vr, chave: kr,
            nivel: d2, path: no2.path + (no2.path ? "\u0001" : "") + kr,
            filhos: {}, ordem: [], cel: {}, tot: novoAcums(), n: 0 };
          no2.ordem.push(no2.filhos[kr]);
        }
        no2 = no2.filhos[kr];
        no2.n++;
        var celN = celDe(no2, kc2);
        for (m2 = 0; m2 < nM; m2++) {
          var vm = linha[med[m2].campo];
          acumula(celN[m2], vm);
          acumula(no2.tot[m2], vm);
        }
      }
      for (m2 = 0; m2 < nM; m2++) {
        var vm2 = linha[med[m2].campo];
        acumula(totCol[kc2][m2], vm2);
        acumula(totGeral[m2], vm2);
      }
    }
    function ordenaNos(no2) {
      var tipoR = no2.ordem.length ? cfgP.tipoDe(no2.ordem[0].campo) : "texto";
      no2.ordem.sort(function (a, b) {
        if (a.valor == null && b.valor == null) return 0;
        if (a.valor == null) return 1;
        if (b.valor == null) return -1;
        var ka = chaveOrd(a.valor, tipoR), kb = chaveOrd(b.valor, tipoR);
        if (ka.v < kb.v) return -1;
        if (ka.v > kb.v) return 1;
        return 0;
      });
      var j3;
      for (j3 = 0; j3 < no2.ordem.length; j3++) ordenaNos(no2.ordem[j3]);
    }
    ordenaNos(raiz);
    var nCel = 0;
    (function conta(no2) { var k3; for (k3 in no2.cel) nCel += nM; var j3; for (j3 = 0; j3 < no2.ordem.length; j3++) conta(no2.ordem[j3]); })(raiz);
    return { colunas: colLista, raiz: raiz, totCol: totCol, totGeral: totGeral, nCelulas: nCel, _ms: agora() - t0 };
  }
  function pivotar(alvo, cfgP) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false };
    var dados = cfgP.dados || [];
    var med = cfgP.medidas, nM = med.length;
    var tipos = cfgP.tipos || {};
    function tipoDe(c2) { return tipos[c2] || "texto"; }
    cfgP.tipoDe = tipoDe;
    var logs = [];
    function log(ev, extra) {
      var e2 = { ev: "phx.pivot." + ev, t: new Date().toISOString() }, k3;
      for (k3 in extra) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgP.logConsole) try { console.log("[phx.pivot]", JSON.stringify(e2)); } catch (e3) {}
    }
    var recolhidosP = {};
    var sortP = null;
    var comparacoes = [];
    var jC;
    for (jC = 0; jC < (cfgP.comparar || []).length; jC++) {
      var cc = cfgP.comparar[jC];
      comparacoes.push({
        de: String(cc.de), para: String(cc.para),
        modo: cc.modo === "delta" || cc.modo === "pct" ? cc.modo : "ambos",
        titulo: cc.titulo || (cc.para + " vs " + cc.de)
      });
    }
    function subsDe(cmp) { return cmp.modo === "ambos" ? ["d", "p"] : [cmp.modo === "pct" ? "p" : "d"]; }
    var modelo = pivotBuild(dados, cfgP);
    log("build", { linhas: cfgP.linhas.join(","), coluna: cfgP.colunas[0], medidas: nM, celulas: modelo.nCelulas, ms: Math.round(modelo._ms * 10) / 10 });
    if (comparacoes.length) log("compare", { comparacoes: comparacoes.length, pares: comparacoes.map(function (c9) { return c9.para + "-" + c9.de; }).join(",") });
    var wrap = el("div", "phx-grid phx-pivot");
    wrap.innerHTML = '<div class="phx-envoltorio"><table class="phx-tabela"><thead></thead><tbody></tbody><tfoot></tfoot></table></div>';
    var thead = wrap.querySelector("thead"), tbody = wrap.querySelector("tbody"), tfoot = wrap.querySelector("tfoot");
    function fmtMed(mIx, v) {
      if (v == null) return "\u2014";
      var m3 = med[mIx];
      return formata({ tipo: m3.tipo || "numero", decimais: m3.decimais }, v, {}, 0);
    }
    function fmtCmp(mIx, v, sub) {
      if (v == null) return { html: "\u2014", cls: "" };
      var corpo, sinal = v > 0 ? "+" : (v < 0 ? "\u2212" : "");
      if (sub === "p") corpo = fmt.numero(Math.abs(v) * 100, 1) + "%";
      else {
        var m3 = med[mIx];
        corpo = formata({ tipo: m3.tipo || "numero", decimais: m3.decimais }, Math.abs(v), {}, 0);
      }
      return { html: sinal + corpo, cls: v > 0 ? " phx-cmp-pos" : (v < 0 ? " phx-cmp-neg" : "") };
    }
    function rotuloVal(campo, v) {
      if (v == null) return "(vazio)";
      return formata({ tipo: tipoDe(campo) }, v, {}, 0);
    }
    function indP(col, mIx) {
      if (!sortP || sortP.colChave !== col || sortP.mIx !== mIx) return "";
      return sortP.dir === "asc" ? " \u25b2" : " \u25bc";
    }
    function montaHeadP() {
      var j3, m3;
      var rotLin = [];
      for (j3 = 0; j3 < cfgP.linhas.length; j3++) rotLin.push(esc((cfgP.rotulos && cfgP.rotulos[cfgP.linhas[j3]]) || cfgP.linhas[j3]));
      if (nM === 1) {
        var h = '<tr><th class="phx-th phx-pivot-esq">' + rotLin.join(" / ") + "</th>";
        for (j3 = 0; j3 < modelo.colunas.length; j3++)
          h += '<th class="phx-th phx-pivot-med phx-pivot-col" data-col="' + esc(modelo.colunas[j3].chave) + '" data-m="0">' + esc(rotuloVal(cfgP.colunas[0], modelo.colunas[j3].valor)) + '<span class="phx-sort-ind">' + indP(modelo.colunas[j3].chave, 0) + "</span></th>";
        for (var jc1 = 0; jc1 < comparacoes.length; jc1++) {
          var subs1 = subsDe(comparacoes[jc1]);
          for (var js1 = 0; js1 < subs1.length; js1++) {
            var chv1 = "\u0002cmp:" + jc1 + ":" + subs1[js1];
            h += '<th class="phx-th phx-pivot-med phx-pivot-cmp" data-col="' + chv1 + '" data-m="0">' + esc(comparacoes[jc1].titulo) + (subs1[js1] === "p" ? " \u0394%" : " \u0394") + '<span class="phx-sort-ind">' + indP(chv1, 0) + "</span></th>";
          }
        }
        h += '<th class="phx-th phx-pivot-med phx-pivot-total" data-col="\u0000total" data-m="0">Total<span class="phx-sort-ind">' + indP("\u0000total", 0) + "</span></th></tr>";
        thead.innerHTML = h;
      } else {
        var h1 = '<tr><th class="phx-th phx-pivot-esq" rowspan="2">' + rotLin.join(" / ") + "</th>", h2 = "<tr>";
        for (j3 = 0; j3 < modelo.colunas.length; j3++) {
          h1 += '<th class="phx-th phx-pivot-col" colspan="' + nM + '">' + esc(rotuloVal(cfgP.colunas[0], modelo.colunas[j3].valor)) + "</th>";
          for (m3 = 0; m3 < nM; m3++)
            h2 += '<th class="phx-th phx-pivot-med" data-col="' + esc(modelo.colunas[j3].chave) + '" data-m="' + m3 + '">' + esc(med[m3].titulo || med[m3].campo) + '<span class="phx-sort-ind">' + indP(modelo.colunas[j3].chave, m3) + "</span></th>";
        }
        for (var jc2 = 0; jc2 < comparacoes.length; jc2++) {
          var subs2 = subsDe(comparacoes[jc2]);
          h1 += '<th class="phx-th phx-pivot-col phx-pivot-cmp" colspan="' + (nM * subs2.length) + '">' + esc(comparacoes[jc2].titulo) + "</th>";
          for (m3 = 0; m3 < nM; m3++)
            for (var js2 = 0; js2 < subs2.length; js2++) {
              var chv2 = "\u0002cmp:" + jc2 + ":" + subs2[js2];
              h2 += '<th class="phx-th phx-pivot-med phx-pivot-cmp" data-col="' + chv2 + '" data-m="' + m3 + '">' + (subs2[js2] === "p" ? "\u0394% " : "\u0394 ") + esc(med[m3].titulo || med[m3].campo) + '<span class="phx-sort-ind">' + indP(chv2, m3) + "</span></th>";
            }
        }
        h1 += '<th class="phx-th phx-pivot-total" colspan="' + nM + '">Total</th>';
        for (m3 = 0; m3 < nM; m3++)
          h2 += '<th class="phx-th phx-pivot-med phx-pivot-total" data-col="\u0000total" data-m="' + m3 + '">' + esc(med[m3].titulo || med[m3].campo) + '<span class="phx-sort-ind">' + indP("\u0000total", m3) + "</span></th>";
        thead.innerHTML = h1 + "</tr>" + h2 + "</tr>";
      }
      var ths = thead.querySelectorAll("th[data-col]");
      for (j3 = 0; j3 < ths.length; j3++) {
        (function (th) {
          th.addEventListener("click", function () {
            var col = th.getAttribute("data-col"), mIx = parseInt(th.getAttribute("data-m"), 10);
            if (sortP && sortP.colChave === col && sortP.mIx === mIx) {
              if (sortP.dir === "desc") sortP = { colChave: col, mIx: mIx, dir: "asc" };
              else sortP = null;
            } else sortP = { colChave: col, mIx: mIx, dir: "desc" };
            log("sort", { col: col === "\u0000total" ? "(total)" : col, medida: med[mIx].campo, dir: sortP ? sortP.dir : "(natural)" });
            montaHeadP(); montaCorpoP();
          });
        })(ths[j3]);
      }
    }
    function valorBruto(no2, colChave, mIx) {
      var acs = colChave === "\u0000total" ? no2.tot : no2.cel[colChave];
      return acs ? fechaAcum(acs[mIx], med[mIx].agregador) : null;
    }
    function valorCmpDe(vDe, vPara, sub) {
      if (vDe == null || vPara == null) return null;
      if (sub === "p") {
        if (vDe === 0) return null;
        return (vPara - vDe) / Math.abs(vDe);
      }
      return vPara - vDe;
    }
    function valorCelula(no2, colChave, mIx) {
      if (colChave.indexOf("\u0002cmp:") === 0) {
        var partes = colChave.slice(5).split(":");
        var cmp = comparacoes[parseInt(partes[0], 10)], sub = partes[1];
        return valorCmpDe(valorBruto(no2, cmp.de, mIx), valorBruto(no2, cmp.para, mIx), sub);
      }
      return valorBruto(no2, colChave, mIx);
    }
    function valorCmpTotCol(cmpIx, mIx, sub) {
      var cmp = comparacoes[cmpIx];
      var aDe = modelo.totCol[cmp.de], aPara = modelo.totCol[cmp.para];
      var vDe = aDe ? fechaAcum(aDe[mIx], med[mIx].agregador) : null;
      var vPara = aPara ? fechaAcum(aPara[mIx], med[mIx].agregador) : null;
      return valorCmpDe(vDe, vPara, sub);
    }
    function montaCorpoP() {
      var html = "";
      function ordenados(no2) {
        if (!sortP) return no2.ordem;
        var copia = no2.ordem.slice();
        copia.sort(function (a, b) {
          var va = valorCelula(a, sortP.colChave, sortP.mIx), vb = valorCelula(b, sortP.colChave, sortP.mIx);
          if (va == null && vb == null) return 0;
          if (va == null) return 1;
          if (vb == null) return -1;
          return sortP.dir === "asc" ? va - vb : vb - va;
        });
        return copia;
      }
      function linhaDe(no2) {
        var abertoG = !recolhidosP[no2.path];
        var temFilhos = no2.ordem.length > 0;
        html += '<tr class="phx-pivot-linha' + (temFilhos ? " phx-pivot-no" : "") + '" data-ppath="' + esc(no2.path) + '">' +
          '<td class="phx-td phx-pivot-esq" style="padding-left:' + (10 + no2.nivel * 20) + 'px">' +
          (temFilhos ? '<span class="phx-grupo-caret">' + (abertoG ? "\u25be" : "\u25b8") + "</span>" : '<span class="phx-grupo-caret"></span>') +
          esc(rotuloVal(no2.campo, no2.valor)) + ' <span class="phx-grupo-conta">(' + fmt.numero(no2.n) + ")</span></td>";
        var j3, m3;
        for (j3 = 0; j3 < modelo.colunas.length; j3++)
          for (m3 = 0; m3 < nM; m3++)
            html += '<td class="phx-td phx-tipo-numero phx-pivot-cel">' + fmtMed(m3, valorCelula(no2, modelo.colunas[j3].chave, m3)) + "</td>";
        var jc3, js3, subs3, r3;
        for (jc3 = 0; jc3 < comparacoes.length; jc3++) {
          subs3 = subsDe(comparacoes[jc3]);
          for (m3 = 0; m3 < nM; m3++)
            for (js3 = 0; js3 < subs3.length; js3++) {
              r3 = fmtCmp(m3, valorCelula(no2, "\u0002cmp:" + jc3 + ":" + subs3[js3], m3), subs3[js3]);
              html += '<td class="phx-td phx-tipo-numero phx-pivot-cel phx-pivot-cmpcel' + r3.cls + '">' + r3.html + "</td>";
            }
        }
        for (m3 = 0; m3 < nM; m3++)
          html += '<td class="phx-td phx-tipo-numero phx-pivot-cel phx-pivot-total">' + fmtMed(m3, valorCelula(no2, "\u0000total", m3)) + "</td>";
        html += "</tr>";
        if (temFilhos && abertoG) {
          var filhosOrd = ordenados(no2);
          for (j3 = 0; j3 < filhosOrd.length; j3++) linhaDe(filhosOrd[j3]);
        }
      }
      var raizOrd = ordenados(modelo.raiz), j4;
      for (j4 = 0; j4 < raizOrd.length; j4++) linhaDe(raizOrd[j4]);
      tbody.innerHTML = html;
      var ftr = '<tr class="phx-pivot-geral"><td class="phx-td phx-pivot-esq">Total Geral</td>';
      var j5, m5;
      for (j5 = 0; j5 < modelo.colunas.length; j5++)
        for (m5 = 0; m5 < nM; m5++)
          ftr += '<td class="phx-td phx-tipo-numero">' + fmtMed(m5, fechaAcum(modelo.totCol[modelo.colunas[j5].chave][m5], med[m5].agregador)) + "</td>";
      var jc5, js5, subs5, r5;
      for (jc5 = 0; jc5 < comparacoes.length; jc5++) {
        subs5 = subsDe(comparacoes[jc5]);
        for (m5 = 0; m5 < nM; m5++)
          for (js5 = 0; js5 < subs5.length; js5++) {
            r5 = fmtCmp(m5, valorCmpTotCol(jc5, m5, subs5[js5]), subs5[js5]);
            ftr += '<td class="phx-td phx-tipo-numero phx-pivot-cmpcel' + r5.cls + '">' + r5.html + "</td>";
          }
      }
      for (m5 = 0; m5 < nM; m5++)
        ftr += '<td class="phx-td phx-tipo-numero phx-pivot-total">' + fmtMed(m5, fechaAcum(modelo.totGeral[m5], med[m5].agregador)) + "</td>";
      tfoot.innerHTML = ftr + "</tr>";
    }
    tbody.addEventListener("click", function (e) {
      var tr = e.target.closest ? e.target.closest(".phx-pivot-no") : null;
      if (!tr) return;
      var pth = tr.getAttribute("data-ppath");
      if (recolhidosP[pth]) delete recolhidosP[pth];
      else recolhidosP[pth] = true;
      log("expand", { path: pth, aberto: !recolhidosP[pth] });
      montaCorpoP();
    });
    montaHeadP();
    montaCorpoP();
    no.appendChild(wrap);
    var apiP = {
      ok: true,
      modelo: function () { return modelo; },
      celula: function (pathArr, colValor, mIx) {
        var no2 = modelo.raiz, j3;
        for (j3 = 0; j3 < pathArr.length; j3++) {
          var krP = pathArr[j3] == null ? "\u0000vazio" : String(pathArr[j3]);
          no2 = no2.filhos[krP];
          if (!no2) return null;
        }
        var kc = colValor === "__total__" ? "\u0000total" : (colValor == null ? "\u0000vazio" : String(colValor));
        return valorCelula(no2, kc, mIx || 0);
      },
      totalGeral: function (mIx) { return fechaAcum(modelo.totGeral[mIx || 0], med[mIx || 0].agregador); },
      totalColuna: function (colValor, mIx) {
        var kc = colValor == null ? "\u0000vazio" : String(colValor);
        return modelo.totCol[kc] ? fechaAcum(modelo.totCol[kc][mIx || 0], med[mIx || 0].agregador) : null;
      },
      expandir: function (path, abrir) {
        if (abrir === false) recolhidosP[path] = true; else delete recolhidosP[path];
        log("expand", { path: path, aberto: abrir !== false });
        montaCorpoP(); return apiP;
      },
      ordenar: function (colValor, mIx, dir) {
        sortP = dir ? { colChave: colValor === "__total__" ? "\u0000total" : String(colValor), mIx: mIx || 0, dir: dir } : null;
        log("sort", { col: colValor == null ? "(natural)" : String(colValor), medida: med[mIx || 0].campo, dir: dir || "(natural)" });
        montaHeadP(); montaCorpoP(); return apiP;
      },
      logs: function () { return logs.slice(); },
      destruir: function () { if (wrap.parentNode) wrap.parentNode.removeChild(wrap); }
    };
    return apiP;
  }

  /* ===== S12: GRID VERTICAL (registros em colunas, campos em linhas) ===== */
  /* ===== O3-11: MATRIZ EDIT\u00c1VEL ===== */
  function matriz(alvo, cfgM) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgM = cfgM || {};
    var dados = (cfgM.dados || []).slice();
    var cL = cfgM.campoLinha || "linha", cC = cfgM.campoColuna || "coluna", cV = cfgM.campoValor || "valor";
    var tipoV = cfgM.tipo || "numero";
    var editavel = cfgM.editavel !== false;
    var logs = [];
    /* Paginacao: 0 desliga. Linhas e colunas paginam independentemente — numa matriz o que
       explode o DOM e o produto linhas x colunas, entao paginar so um eixo nao resolve. */
    var pgL = 1, tamL = (cfgM.pagina && cfgM.pagina.tamanho) || 0;
    var pgC = 1, tamC = (cfgM.paginaColuna && cfgM.paginaColuna.tamanho) || 0;
    var ultimaPag = { linha: { pagina: 1, tamanho: 0, paginas: 1, total: 0 },
                      coluna: { pagina: 1, tamanho: 0, paginas: 1, total: 0 } };
    function log(ev, extra) {
      var e2 = { ev: "phx.matriz." + ev, t: Date.now() }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgM.aoLog) try { cfgM.aoLog(e2); } catch (e9) {}
      return e2;
    }
    function distintos(campo9, fixos9) {
      if (fixos9 && fixos9.length) return fixos9.slice();
      var vistos = {}, out = [], j2, v2;
      for (j2 = 0; j2 < dados.length; j2++) {
        v2 = String(dados[j2][campo9]);
        if (!(v2 in vistos)) { vistos[v2] = 1; out.push(dados[j2][campo9]); }
      }
      out.sort(function (a, b) { return String(a) < String(b) ? -1 : (String(a) > String(b) ? 1 : 0); });
      return out;
    }
    /* Indice (linha, coluna) -> registro. O scan linear custava O(linhas x colunas x registros):
       numa matriz de 100 x 100 sobre 10.000 registros sao 100 milhoes de comparacoes por desenho.
       Chave com prefixo de comprimento da linha: nao colide quando o dado contem o separador. */
    var indice = null;
    function chaveMz(l9, c9) {
      var sl = String(l9);
      return "@" + sl.length + "\u001f" + sl + String(c9);
    }
    function reindexa() {
      indice = {};
      var j2, k9;
      for (j2 = 0; j2 < dados.length; j2++) {
        k9 = chaveMz(dados[j2][cL], dados[j2][cC]);
        /* o scan antigo devolvia a PRIMEIRA ocorrencia; o indice preserva isso */
        if (!Object.prototype.hasOwnProperty.call(indice, k9)) indice[k9] = dados[j2];
      }
    }
    function registro(l9, c9) {
      if (!indice) reindexa();
      var r9 = indice[chaveMz(l9, c9)];
      return r9 || null;
    }
    function valorDe(l9, c9) {
      var r9 = registro(l9, c9);
      return r9 ? r9[cV] : null;
    }
    var raiz = el("div", "phx-matriz" + (cfgM.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = "";
    no.appendChild(raiz);
    var editando = null;
    function fmtV(v9) {
      if (v9 == null || v9 === "") return "";
      if (tipoV === "moeda") return fmt.moeda(v9);
      if (tipoV === "percentual") return fmt.percentual(v9);
      if (tipoV === "numero") return fmt.numero(v9, cfgM.decimais == null ? 0 : cfgM.decimais);
      return String(v9);
    }
    /* Totais sobre a matriz INTEIRA, nunca sobre a pagina: um total que muda quando o usuario
       vira a pagina e um numero errado. Uma passada sobre os registros, O(n), em vez de
       linhas x colunas consultas. A identidade com o indice reproduz a regra da primeira
       ocorrencia do scan antigo, entao registro duplicado continua contando uma vez so. */
    function totaliza(LS, CS) {
      if (!indice) reindexa();
      var mapaL = {}, mapaC = {}, j2, rl, rc, n2, v2;
      var porLinha = {}, porColuna = {}, total = 0;
      for (j2 = 0; j2 < LS.length; j2++) mapaL["@" + String(LS[j2])] = 1;
      for (j2 = 0; j2 < CS.length; j2++) mapaC["@" + String(CS[j2])] = 1;
      for (j2 = 0; j2 < dados.length; j2++) {
        rl = "@" + String(dados[j2][cL]);
        rc = "@" + String(dados[j2][cC]);
        if (!Object.prototype.hasOwnProperty.call(mapaL, rl)) continue;
        if (!Object.prototype.hasOwnProperty.call(mapaC, rc)) continue;
        if (indice[chaveMz(dados[j2][cL], dados[j2][cC])] !== dados[j2]) continue;
        v2 = dados[j2][cV];
        if (v2 == null || v2 === "") continue;
        n2 = Number(v2);
        if (n2 !== n2) continue;
        porLinha[rl] = (porLinha[rl] || 0) + n2;
        porColuna[rc] = (porColuna[rc] || 0) + n2;
        total += n2;
      }
      return { linha: porLinha, coluna: porColuna, total: total };
    }
    function faixa(total9, tam9, pag9) {
      var paginas = tam9 > 0 ? Math.max(1, Math.ceil(total9 / tam9)) : 1;
      var p = pag9 < 1 ? 1 : (pag9 > paginas ? paginas : pag9);
      var ini = tam9 > 0 ? (p - 1) * tam9 : 0;
      var fim = tam9 > 0 ? Math.min(total9, ini + tam9) : total9;
      return { pagina: p, paginas: paginas, ini: ini, fim: fim, tamanho: tam9, total: total9 };
    }
    function botoesPag(eixo, f) {
      var b = ['<div class="phx-mz-nav phx-pag" data-eixo="' + eixo + '">'];
      var rotulo = eixo === "linha" ? "Linhas" : "Colunas";
      b.push('<button class="phx-pg" data-ir="1" aria-label="primeira p\u00e1gina"' + (f.pagina <= 1 ? " disabled" : "") + ">\u00ab</button>");
      b.push('<button class="phx-pg" data-ir="' + (f.pagina - 1) + '" aria-label="p\u00e1gina anterior"' + (f.pagina <= 1 ? " disabled" : "") + ">\u2039</button>");
      b.push('<span class="phx-mz-conta">' + rotulo + " " + (f.total ? f.ini + 1 : 0) + "\u2013" + f.fim + " de " + f.total + "</span>");
      b.push('<button class="phx-pg" data-ir="' + (f.pagina + 1) + '" aria-label="pr\u00f3xima p\u00e1gina"' + (f.pagina >= f.paginas ? " disabled" : "") + ">\u203a</button>");
      b.push('<button class="phx-pg" data-ir="' + f.paginas + '" aria-label="\u00faltima p\u00e1gina"' + (f.pagina >= f.paginas ? " disabled" : "") + ">\u00bb</button>");
      var ops = (eixo === "linha" ? (cfgM.pagina && cfgM.pagina.opcoes) : (cfgM.paginaColuna && cfgM.paginaColuna.opcoes)) || null;
      if (ops && ops.length) {
        b.push('<select class="phx-mz-tam" data-eixo="' + eixo + '" aria-label="tamanho da p\u00e1gina">');
        for (var i9 = 0; i9 < ops.length; i9++) {
          b.push('<option value="' + Number(ops[i9]) + '"' + (Number(ops[i9]) === f.tamanho ? " selected" : "") + ">" + Number(ops[i9]) + "</option>");
        }
        b.push("</select>");
      }
      b.push("</div>");
      return b.join("");
    }
    function desenha() {
      var LS = distintos(cL, cfgM.linhas), CS = distintos(cC, cfgM.colunas);
      var fL = faixa(LS.length, tamL, pgL), fC = faixa(CS.length, tamC, pgC);
      pgL = fL.pagina; pgC = fC.pagina;
      ultimaPag = { linha: { pagina: fL.pagina, tamanho: tamL, paginas: fL.paginas, total: fL.total },
                    coluna: { pagina: fC.pagina, tamanho: tamC, paginas: fC.paginas, total: fC.total } };
      var LV = LS.slice(fL.ini, fL.fim), CV = CS.slice(fC.ini, fC.fim);
      var tot = cfgM.totais !== false ? totaliza(LS, CS) : null;
      var h = ['<table class="phx-mz-tab"><thead><tr><th class="phx-mz-canto">' + esc(cfgM.rotulo || "") + "</th>"];
      var j2, k2, v2;
      for (j2 = 0; j2 < CV.length; j2++) h.push('<th class="phx-mz-cab">' + esc(CV[j2]) + "</th>");
      if (cfgM.totais !== false) h.push('<th class="phx-mz-cab phx-mz-tot">Total</th>');
      h.push("</tr></thead><tbody>");
      for (j2 = 0; j2 < LV.length; j2++) {
        h.push('<tr><th class="phx-mz-rot">' + esc(LV[j2]) + "</th>");
        for (k2 = 0; k2 < CV.length; k2++) {
          v2 = valorDe(LV[j2], CV[k2]);
          h.push('<td class="phx-mz-cel' + (v2 == null || v2 === "" ? " phx-mz-vazia" : "") + '" data-ml="' + esc(LV[j2]) + '" data-mc="' + esc(CV[k2]) + '">' + esc(fmtV(v2)) + "</td>");
        }
        if (cfgM.totais !== false) h.push('<td class="phx-mz-cel phx-mz-tot">' + esc(fmtV(tot.linha["@" + String(LV[j2])] || 0)) + "</td>");
        h.push("</tr>");
      }
      if (cfgM.totais !== false) {
        h.push('<tr class="phx-mz-linhatot"><th class="phx-mz-rot">Total</th>');
        for (k2 = 0; k2 < CV.length; k2++) h.push('<td class="phx-mz-cel phx-mz-tot">' + esc(fmtV(tot.coluna["@" + String(CV[k2])] || 0)) + "</td>");
        h.push('<td class="phx-mz-cel phx-mz-tot">' + esc(fmtV(tot.total)) + "</td></tr>");
      }
      h.push("</tbody></table>");
      if (tamL > 0 || tamC > 0) {
        h.push('<div class="phx-mz-rodape">');
        if (tamL > 0) h.push(botoesPag("linha", fL));
        if (tamC > 0) h.push(botoesPag("coluna", fC));
        h.push("</div>");
      }
      raiz.innerHTML = h.join("");
      if (editavel) ligaEdicao();
      if (tamL > 0 || tamC > 0) ligaPaginacao();
    }
    /* Trocar de pagina redesenha; se houver editor aberto, o innerHTML o remove e o blur
       dispara no meio do render. Encerrar em silencio antes evita a DOMException. */
    function encerraEdicaoSilenciosa() { editando = null; }
    function vaiPara(eixo, n9) {
      encerraEdicaoSilenciosa();
      if (eixo === "linha") pgL = n9; else pgC = n9;
      desenha();
      log("pagina", { eixo: eixo, pagina: eixo === "linha" ? ultimaPag.linha.pagina : ultimaPag.coluna.pagina });
      return apiM;
    }
    function ligaPaginacao() {
      var bts = raiz.querySelectorAll(".phx-mz-nav .phx-pg"), j2;
      for (j2 = 0; j2 < bts.length; j2++) (function (bt) {
        bt.addEventListener("click", function () {
          if (bt.disabled) return;
          vaiPara(bt.parentNode.getAttribute("data-eixo"), Number(bt.getAttribute("data-ir")));
        });
      })(bts[j2]);
      var sels = raiz.querySelectorAll(".phx-mz-tam");
      for (j2 = 0; j2 < sels.length; j2++) (function (sl) {
        sl.addEventListener("change", function () {
          encerraEdicaoSilenciosa();
          if (sl.getAttribute("data-eixo") === "linha") { tamL = Number(sl.value) || 0; pgL = 1; }
          else { tamC = Number(sl.value) || 0; pgC = 1; }
          desenha();
          log("tamanho-pagina", { eixo: sl.getAttribute("data-eixo"), tamanho: Number(sl.value) || 0 });
        });
      })(sels[j2]);
    }
    function ligaEdicao() {
      var cels = raiz.querySelectorAll(".phx-mz-cel:not(.phx-mz-tot)"), j2;
      for (j2 = 0; j2 < cels.length; j2++) (function (td) {
        td.addEventListener("dblclick", function () { abre(td); });
      })(cels[j2]);
    }
    function abre(td) {
      if (editando) fecha(false);
      var l9 = td.getAttribute("data-ml"), c9 = td.getAttribute("data-mc");
      var atual9 = valorDe(l9, c9);
      var inp = document.createElement("input");
      inp.className = "phx-mz-input";
      inp.value = atual9 == null ? "" : String(atual9);
      td.innerHTML = "";
      td.appendChild(inp);
      inp.focus();
      editando = { td: td, l: l9, c: c9, antigo: atual9, inp: inp };
      inp.addEventListener("keydown", function (ev) {
        if (ev.key === "Enter") { ev.preventDefault(); fecha(true); }
        else if (ev.key === "Escape") { ev.preventDefault(); fecha(false); }
        else if (ev.key === "Tab") { ev.preventDefault(); fecha(true); }
      });
      inp.addEventListener("blur", function () { if (editando && editando.inp === inp) fecha(true); });
    }
    function fecha(commit) {
      if (!editando) return;
      var ctx = editando;
      editando = null;
      if (!commit) { desenha(); return; }
      var bruto = ctx.inp.value;
      var novo = bruto === "" ? null : (tipoV === "texto" ? bruto : Number(String(bruto).replace(/\./g, "").replace(",", ".")));
      if (novo !== null && novo !== novo) { desenha(); log("valor-invalido", { linha: ctx.l, coluna: ctx.c, texto: bruto }); return; }
      if (String(novo) === String(ctx.antigo)) { desenha(); return; }
      apiM.setValor(ctx.l, ctx.c, novo);
    }
    var apiM = {
      ok: true,
      el: raiz,
      valor: valorDe,
      setValor: function (l9, c9, v9) {
        if (cfgM.aoEditar) {
          var r9;
          try { r9 = cfgM.aoEditar(l9, c9, v9, valorDe(l9, c9)); } catch (e9) { r9 = false; }
          if (r9 === false) { log("edit-recusado", { linha: String(l9), coluna: String(c9), valor: v9 }); desenha(); return { ok: false, erro: "recusado pela aplica\u00e7\u00e3o" }; }
        }
        var reg9 = registro(l9, c9), de9 = reg9 ? reg9[cV] : null;
        if (reg9) reg9[cV] = v9;
        else { var novo9 = {}; novo9[cL] = l9; novo9[cC] = c9; novo9[cV] = v9; dados.push(novo9); indice = null; }
        log("edit", { linha: String(l9), coluna: String(c9), de: de9, para: v9 });
        desenha();
        return { ok: true, linha: String(l9), coluna: String(c9), de: de9, para: v9 };
      },
      estado: function () {
        var LS = distintos(cL, cfgM.linhas), CS = distintos(cC, cfgM.colunas), j2, k2, n2, tot = 0, preenchidas = 0;
        for (j2 = 0; j2 < LS.length; j2++) for (k2 = 0; k2 < CS.length; k2++) {
          n2 = Number(valorDe(LS[j2], CS[k2]));
          if (n2 === n2 && valorDe(LS[j2], CS[k2]) != null) { tot += n2; preenchidas++; }
        }
        return { linhas: LS.length, colunas: CS.length, celulas: LS.length * CS.length, preenchidas: preenchidas, total: tot };
      },
      atualizar: function (novos9) { dados = (novos9 || []).slice(); indice = null; desenha(); return apiM; },
      pagina: function (n9) { return n9 == null ? ultimaPag.linha.pagina : vaiPara("linha", Number(n9)); },
      paginaColuna: function (n9) { return n9 == null ? ultimaPag.coluna.pagina : vaiPara("coluna", Number(n9)); },
      tamanhoPagina: function (n9) {
        if (n9 == null) return tamL;
        encerraEdicaoSilenciosa();
        tamL = Number(n9) > 0 ? Number(n9) : 0; pgL = 1; desenha();
        log("tamanho-pagina", { eixo: "linha", tamanho: tamL });
        return apiM;
      },
      tamanhoColuna: function (n9) {
        if (n9 == null) return tamC;
        encerraEdicaoSilenciosa();
        tamC = Number(n9) > 0 ? Number(n9) : 0; pgC = 1; desenha();
        log("tamanho-pagina", { eixo: "coluna", tamanho: tamC });
        return apiM;
      },
      paginacao: function () {
        return { linha: { pagina: ultimaPag.linha.pagina, tamanho: ultimaPag.linha.tamanho,
                          paginas: ultimaPag.linha.paginas, total: ultimaPag.linha.total },
                 coluna: { pagina: ultimaPag.coluna.pagina, tamanho: ultimaPag.coluna.tamanho,
                           paginas: ultimaPag.coluna.paginas, total: ultimaPag.coluna.total } };
      },
      dados: function () { return dados.slice(); },
      logs: function () { return logs.slice(); },
      destruir: function () { no.innerHTML = ""; logs.length = 0; return true; }
    };
    desenha();
    log("init", { registros: dados.length });
    return apiM;
  }
  function matrizJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.matriz.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    if (cfg.aoEditarWL) {
      var nomeFn = cfg.aoEditarWL;
      cfg.aoEditar = function (l, c, v, de) {
        var fn = root[nomeFn];
        if (typeof fn !== "function") return true;
        var r = fn(JSON.stringify({ linha: l, coluna: c, valor: v, de: de }));
        return r === false || r === "false" ? false : true;
      };
    }
    var apiM = matriz(seletor, cfg);
    if (apiM && apiM.ok === false) return JSON.stringify(apiM);
    _instancias[handle] = { api: apiM, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  /* ===== O3-9: TIMELINE / GANTT ===== */
  var LARG_ESCALA = { dia: 30, semana: 12, mes: 4 };
  function diaZero(v) {
    if (v == null || v === "") return null;
    var d = v instanceof Date ? v : new Date(String(v).length <= 10 ? String(v) + "T00:00:00Z" : v);
    if (!d || isNaN(d.getTime())) return null;
    return Math.floor(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()) / 86400000);
  }
  function deDia(n) {
    var d = new Date(n * 86400000);
    function p2(x) { return x < 10 ? "0" + x : String(x); }
    return d.getUTCFullYear() + "-" + p2(d.getUTCMonth() + 1) + "-" + p2(d.getUTCDate());
  }
  function gantt(alvo, cfgG) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgG = cfgG || {};
    var dados = (cfgG.dados || []).slice();
    var chaveCampo = cfgG.chave || null;
    var campoTitulo = cfgG.titulo || chaveCampo;
    var campoIni = cfgG.inicio || "inicio";
    var campoFim = cfgG.fim || null;
    var campoDur = cfgG.duracao || null;
    var campoProg = cfgG.progresso || null;
    var campoCor = cfgG.cor || null;
    var mapaCores = cfgG.cores || null;
    var campoDep = cfgG.dependencia || null;
    var campoGrupo = cfgG.agrupar || null;
    var escala = cfgG.escala || "dia";
    var logs = [];
    function log(ev, extra) {
      var e2 = { ev: "phx.gantt." + ev, t: Date.now() }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgG.aoLog) try { cfgG.aoLog(e2); } catch (e9) {}
      return e2;
    }
    function chaveDeG(linha, ix) { return chaveCampo ? String(linha[chaveCampo]) : "#" + ix; }
    function barraDe(linha, ix) {
      var ini = diaZero(linha[campoIni]);
      if (ini == null) return null;
      var fim;
      if (campoFim && linha[campoFim] != null && linha[campoFim] !== "") fim = diaZero(linha[campoFim]);
      else if (campoDur && Number(linha[campoDur]) > 0) fim = ini + Math.round(Number(linha[campoDur])) - 1;
      else fim = ini;
      if (fim == null || fim < ini) fim = ini;
      return { chave: chaveDeG(linha, ix), linha: linha, ini: ini, fim: fim, dias: fim - ini + 1 };
    }
    function todasBarras() {
      var out = [], j2, b2;
      for (j2 = 0; j2 < dados.length; j2++) { b2 = barraDe(dados[j2], j2); if (b2) out.push(b2); }
      return out;
    }
    function janela() {
      var bs = todasBarras(), j2, mn = null, mx = null;
      for (j2 = 0; j2 < bs.length; j2++) {
        if (mn == null || bs[j2].ini < mn) mn = bs[j2].ini;
        if (mx == null || bs[j2].fim > mx) mx = bs[j2].fim;
      }
      if (mn == null) { mn = diaZero(new Date()); mx = mn + 30; }
      var folga = escala === "dia" ? 2 : (escala === "semana" ? 7 : 31);
      return { ini: mn - folga, fim: mx + folga };
    }
    var raiz = el("div", "phx-gantt" + (cfgG.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = "";
    no.appendChild(raiz);
    var MESES_G = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    var arraste = null;
    function larg() { return LARG_ESCALA[escala] || LARG_ESCALA.dia; }
    function desenha() {
      var J = janela(), L = larg(), totalDias = J.fim - J.ini + 1;
      var bs = todasBarras(), j2, k2;
      var grupos = [], porGrupo = {};
      for (j2 = 0; j2 < bs.length; j2++) {
        var g2 = campoGrupo ? String(bs[j2].linha[campoGrupo]) : "";
        if (!porGrupo[g2]) { porGrupo[g2] = []; grupos.push(g2); }
        porGrupo[g2].push(bs[j2]);
      }
      var cabMes = [], cabDia = [], d2, dt2, mesAtual = null, largMes = 0, iniMes = 0;
      for (d2 = 0; d2 < totalDias; d2++) {
        dt2 = new Date((J.ini + d2) * 86400000);
        var chaveMes = dt2.getUTCFullYear() + "-" + dt2.getUTCMonth();
        if (chaveMes !== mesAtual) {
          if (mesAtual !== null) cabMes.push('<div class="phx-gt-mes" style="left:' + (iniMes * L) + "px;width:" + (largMes * L) + 'px">' + MESES_G[new Date((J.ini + iniMes) * 86400000).getUTCMonth()] + "/" + String(new Date((J.ini + iniMes) * 86400000).getUTCFullYear()).slice(2) + "</div>");
          mesAtual = chaveMes; iniMes = d2; largMes = 0;
        }
        largMes++;
        var fds = dt2.getUTCDay() === 0 || dt2.getUTCDay() === 6;
        if (escala === "dia") {
          cabDia.push('<div class="phx-gt-dia' + (fds ? " phx-gt-fds" : "") + '" style="left:' + (d2 * L) + "px;width:" + L + 'px">' + dt2.getUTCDate() + "</div>");
        } else if (escala === "semana" && dt2.getUTCDay() === 1) {
          cabDia.push('<div class="phx-gt-dia" style="left:' + (d2 * L) + "px;width:" + (7 * L) + 'px">' + dt2.getUTCDate() + "</div>");
        }
        if (fds && escala === "dia") cabMes.push('<div class="phx-gt-fdsbg" style="left:' + (d2 * L) + "px;width:" + L + 'px"></div>');
      }
      cabMes.push('<div class="phx-gt-mes" style="left:' + (iniMes * L) + "px;width:" + (largMes * L) + 'px">' + MESES_G[new Date((J.ini + iniMes) * 86400000).getUTCMonth()] + "/" + String(new Date((J.ini + iniMes) * 86400000).getUTCFullYear()).slice(2) + "</div>");
      var hoje = diaZero(cfgG.hoje || new Date());
      var linhaHoje = (hoje != null && hoje >= J.ini && hoje <= J.fim)
        ? '<div class="phx-gt-hoje" style="left:' + ((hoje - J.ini) * L + Math.round(L / 2)) + 'px"></div>' : "";
      var rot = [], faixas = [], ordemY = {}, y = 0;
      for (j2 = 0; j2 < grupos.length; j2++) {
        if (campoGrupo) {
          rot.push('<div class="phx-gt-grupo">' + esc(grupos[j2]) + ' <span class="phx-gt-gn">' + porGrupo[grupos[j2]].length + "</span></div>");
          faixas.push('<div class="phx-gt-faixa phx-gt-faixag"></div>');
          y++;
        }
        for (k2 = 0; k2 < porGrupo[grupos[j2]].length; k2++) {
          var b3 = porGrupo[grupos[j2]][k2];
          ordemY[b3.chave] = y;
          rot.push('<div class="phx-gt-rot" title="' + esc(b3.linha[campoTitulo]) + '">' + esc(b3.linha[campoTitulo]) + "</div>");
          var cor3 = null;
          if (campoCor) cor3 = mapaCores ? mapaCores[String(b3.linha[campoCor])] : b3.linha[campoCor];
          var prog3 = campoProg ? Math.max(0, Math.min(100, Number(b3.linha[campoProg]) || 0)) : null;
          faixas.push('<div class="phx-gt-faixa">' +
            '<div class="phx-gt-barra" data-gc="' + esc(b3.chave) + '" style="left:' + ((b3.ini - J.ini) * L) + "px;width:" + (b3.dias * L) + "px" + (cor3 ? ";background:" + esc(cor3) : "") + '" title="' + esc(b3.linha[campoTitulo]) + ": " + deDia(b3.ini) + " \u2192 " + deDia(b3.fim) + '">' +
            (prog3 != null ? '<div class="phx-gt-prog" style="width:' + prog3 + '%"></div>' : "") +
            '<span class="phx-gt-lab">' + esc(b3.linha[campoTitulo]) + "</span></div></div>");
          y++;
        }
      }
      var deps = "";
      if (campoDep) {
        var linhasSvg = [], j4, b4, k4;
        var bsPorChave = {};
        for (j4 = 0; j4 < bs.length; j4++) bsPorChave[bs[j4].chave] = bs[j4];
        for (j4 = 0; j4 < bs.length; j4++) {
          b4 = bs[j4];
          var dep4 = b4.linha[campoDep];
          if (dep4 == null || dep4 === "") continue;
          var lista4 = dep4 instanceof Array ? dep4 : String(dep4).split(",");
          for (k4 = 0; k4 < lista4.length; k4++) {
            var pre4 = bsPorChave[String(lista4[k4]).replace(/^\s+|\s+$/g, "")];
            if (!pre4) continue;
            var x1 = (pre4.fim - J.ini + 1) * L, y1 = ordemY[pre4.chave] * 30 + 15;
            var x2 = (b4.ini - J.ini) * L, y2 = ordemY[b4.chave] * 30 + 15;
            linhasSvg.push('<path d="M' + x1 + " " + y1 + " L" + (x1 + 8) + " " + y1 + " L" + (x1 + 8) + " " + y2 + " L" + x2 + " " + y2 + '" fill="none" stroke="currentColor" stroke-width="1.4"/>');
            linhasSvg.push('<circle cx="' + x2 + '" cy="' + y2 + '" r="2.6" fill="currentColor"/>');
          }
        }
        deps = '<svg class="phx-gt-deps" width="' + (totalDias * L) + '" height="' + (y * 30) + '">' + linhasSvg.join("") + "</svg>";
      }
      raiz.innerHTML =
        '<div class="phx-gt-esq"><div class="phx-gt-cabesq">' + esc(cfgG.rotulo || "Item") + "</div>" + rot.join("") + "</div>" +
        '<div class="phx-gt-dir"><div class="phx-gt-cab" style="width:' + (totalDias * L) + 'px">' +
        '<div class="phx-gt-meses">' + cabMes.join("") + "</div>" +
        '<div class="phx-gt-dias">' + cabDia.join("") + "</div></div>" +
        '<div class="phx-gt-tela" style="width:' + (totalDias * L) + "px;height:" + (y * 30) + 'px">' + linhaHoje + deps + faixas.join("") + "</div></div>";
      ligaArraste(J, L);
    }
    function ligaArraste(J, L) {
      if (cfgG.arrastar === false) return;
      var barras = raiz.querySelectorAll(".phx-gt-barra"), j2;
      for (j2 = 0; j2 < barras.length; j2++) (function (bar) {
        bar.addEventListener("mousedown", function (ev) {
          arraste = { chave: bar.getAttribute("data-gc"), x0: ev.clientX, esq0: parseFloat(bar.style.left) || 0, bar: bar };
          bar.className += " phx-gt-arrastando";
          ev.preventDefault();
        });
      })(barras[j2]);
      if (raiz._gtLigado) return;
      raiz._gtLigado = true;
      var doc = raiz.ownerDocument || document;
      doc.addEventListener("mousemove", function (ev) {
        if (!arraste) return;
        var dx = ev.clientX - arraste.x0;
        arraste.bar.style.left = (arraste.esq0 + dx) + "px";
      });
      doc.addEventListener("mouseup", function (ev) {
        if (!arraste) return;
        var a9 = arraste;
        arraste = null;
        var dx = ev.clientX - a9.x0;
        var dias = Math.round(dx / larg());
        if (!dias) { desenha(); return; }
        apiG.deslocar(a9.chave, dias);
      });
    }
    var apiG = {
      ok: true,
      el: raiz,
      deslocar: function (chave9, dias9) {
        var j2, linha9 = null, ix9 = -1;
        for (j2 = 0; j2 < dados.length; j2++) if (chaveDeG(dados[j2], j2) === String(chave9)) { linha9 = dados[j2]; ix9 = j2; break; }
        if (!linha9) return { ok: false, erro: "item n\u00e3o encontrado: " + chave9 };
        var b9 = barraDe(linha9, ix9);
        var novoIni9 = deDia(b9.ini + dias9), novoFim9 = deDia(b9.fim + dias9);
        if (cfgG.aoMover) {
          var r9;
          try { r9 = cfgG.aoMover(String(chave9), novoIni9, novoFim9, linha9); } catch (e9) { r9 = false; }
          if (r9 === false) {
            log("mover-recusado", { chave: String(chave9), dias: dias9 });
            desenha();
            return { ok: false, erro: "recusado pela aplica\u00e7\u00e3o" };
          }
        }
        linha9[campoIni] = novoIni9;
        if (campoFim && linha9[campoFim] != null && linha9[campoFim] !== "") linha9[campoFim] = novoFim9;
        log("mover", { chave: String(chave9), dias: dias9, inicio: novoIni9, fim: novoFim9 });
        desenha();
        return { ok: true, chave: String(chave9), inicio: novoIni9, fim: novoFim9 };
      },
      setEscala: function (e9) {
        if (!LARG_ESCALA[e9]) return apiG;
        escala = e9;
        desenha();
        log("escala", { escala: e9 });
        return apiG;
      },
      escala: function () { return escala; },
      atualizar: function (novos9) { dados = (novos9 || []).slice(); desenha(); log("atualizar", { linhas: dados.length }); return apiG; },
      anexar: function (novos9) { var j2; for (j2 = 0; j2 < (novos9 || []).length; j2++) dados.push(novos9[j2]); desenha(); return apiG; },
      estado: function () {
        var J9 = janela(), bs9 = todasBarras(), j2, dur9 = 0;
        for (j2 = 0; j2 < bs9.length; j2++) dur9 += bs9[j2].dias;
        return {
          itens: bs9.length, escala: escala,
          inicio: deDia(J9.ini), fim: deDia(J9.fim),
          dias: J9.fim - J9.ini + 1,
          duracaoMedia: bs9.length ? Math.round(dur9 / bs9.length * 10) / 10 : 0,
          larguraDia: larg()
        };
      },
      barras: function () {
        var bs9 = todasBarras(), out9 = [], j2;
        for (j2 = 0; j2 < bs9.length; j2++) out9.push({ chave: bs9[j2].chave, inicio: deDia(bs9[j2].ini), fim: deDia(bs9[j2].fim), dias: bs9[j2].dias });
        return out9;
      },
      dados: function () { return dados.slice(); },
      logs: function () { return logs.slice(); },
      destruir: function () { no.innerHTML = ""; logs.length = 0; return true; }
    };
    desenha();
    log("init", { itens: dados.length, escala: escala });
    return apiG;
  }
  function ganttJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.gantt.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    if (cfg.aoMoverWL) {
      var nomeFn = cfg.aoMoverWL;
      cfg.aoMover = function (ch, ini, fim, linha) {
        var fn = root[nomeFn];
        if (typeof fn !== "function") return true;
        var r = fn(JSON.stringify({ chave: ch, inicio: ini, fim: fim, linha: linha }));
        return r === false || r === "false" ? false : true;
      };
    }
    var apiG = gantt(seletor, cfg);
    if (apiG && apiG.ok === false) return JSON.stringify(apiG);
    _instancias[handle] = { api: apiG, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  /* ===== O4-3: CUBO (PIVOT INTERATIVO) ===== */
  var AGG_CUBO = {
    sum: function (a) { var t = 0, i; for (i = 0; i < a.length; i++) t += a[i]; return t; },
    count: function (a) { return a.length; },
    avg: function (a) { return a.length ? AGG_CUBO.sum(a) / a.length : 0; },
    min: function (a) { return a.length ? Math.min.apply(null, a) : null; },
    max: function (a) { return a.length ? Math.max.apply(null, a) : null; }
  };
  function cubo(alvo, cfgC) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgC = cfgC || {};
    /* ISOLAMENTO DOS DADOS DO CHAMADOR (v0.72)
       Antes: `.slice()` copiava o ARRAY mas nao os OBJETOS -- e o cubo escreve campos derivados
       (campoLinha, agruparData, agruparFaixa) direto no registro. Resultado: os objetos do ERP
       ganhavam `emissao__ano`, `valor__faixa` etc., e se aquele mesmo array fosse usado para gravar,
       campos fantasma iam para o banco.
       Agora cada registro entra como copia rasa, e `_origDe` devolve o registro ORIGINAL --
       porque drill-through tem que entregar ao chamador o objeto dele, nao a nossa copia. */
    var dados = [], _origs = [];
    (function () {
      var src9 = cfgC.dados || [], i9, k9, o9, r9;
      for (i9 = 0; i9 < src9.length; i9++) {
        r9 = src9[i9];
        if (r9 && typeof r9 === "object") {
          o9 = {};
          for (k9 in r9) if (Object.prototype.hasOwnProperty.call(r9, k9)) o9[k9] = r9[k9];
          try { Object.defineProperty(o9, "__ix", { value: i9, enumerable: false, writable: true, configurable: true }); }
          catch (e9) { o9.__ix = i9; }
        } else o9 = r9;
        dados.push(o9); _origs.push(r9);
      }
    })();
    function _origDe(l9) {
      if (l9 && l9.__ix != null && _origs[l9.__ix] !== undefined) return _origs[l9.__ix];
      return l9;
    }
    /* VALIDACAO DE ARGUMENTOS (v0.72)
       O cubo e chamado por STRING desde o WLanguage (ponte JSON). Um argumento com o tipo errado
       lancava excecao no meio do render -- e excecao de JS dentro do WebView do WinDev vira tela
       branca, sem mensagem. Agora cada erro de uso vira um aviso no log e a chamada e ignorada,
       devolvendo a API para encadeamento. Nunca lanca. */
    function _erroUso(metodo9, motivo9) {
      log("aviso", { metodo: metodo9, motivo: motivo9 });
      if (typeof console !== "undefined" && console.warn) {
        try { console.warn("[phx.cubo] " + metodo9 + ": " + motivo9); } catch (e9) {}
      }
      return false;
    }
    function _exigeCampo(metodo9, campo9) {
      if (typeof campo9 !== "string" || !campo9) return _erroUso(metodo9, "campo deve ser uma string nao vazia");
      if (!porCampoC[campo9]) return _erroUso(metodo9, "campo desconhecido: \"" + campo9 + "\"");
      return true;
    }
    function _comoLista(v9) { return v9 == null ? [] : (v9 instanceof Array ? v9 : [v9]); }
    var campos = (cfgC.campos || []).slice();
    var porCampoC = {}, j0;
    for (j0 = 0; j0 < campos.length; j0++) porCampoC[campos[j0].campo] = campos[j0];
    var lay = {
      linhas: (cfgC.linhas || []).slice(),
      colunas: (cfgC.colunas || []).slice(),
      valores: (cfgC.valores || []).slice(),
      filtros: {}
    };
    var kf;
    for (kf in (cfgC.filtros || {})) lay.filtros[kf] = (cfgC.filtros[kf] || []).slice();
    var recolhidos = {};
    var logs = [];
    var SEP = "\u001f";
    /* Sentinela de valor vazio no caminho do cubo. NAO pode ser "" : o prefixo do total geral
       tambem e "", e as duas chaves colidiriam no mesmo acumulador -- uma dimensao nula somava
       duas vezes no total (bug real: 1.234,56 + 500 + (-80) dava 1.574,56 em vez de 1.654,56). */
    var VAZIO = "\u0000v";
    var opc = { layout: cfgC.layoutTabela || "compacto", repetirRotulos: !!cfgC.repetirRotulos, valoresEm: cfgC.valoresEm || "colunas", totaisLinha: cfgC.totaisLinha !== false, totaisColuna: cfgC.totaisColuna !== false, subtotais: cfgC.subtotais !== false, vazio: cfgC.vazio == null ? "" : cfgC.vazio, painelCampos: cfgC.painelCampos !== false, quebraLinha: !!cfgC.quebraLinha };
    var ordens = {}, topN = {}, faixas = {}, datasAgrup = {}, adiado = false, pendente = false, filtrosTop = {}, _buscaCampos = "";
    var _graficos = [];
    var MESES_C = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    function partesData(v9) {
      if (v9 == null || v9 === "") return null;
      var t9 = v9 instanceof Date ? (v9.getFullYear() + "-" + (v9.getMonth() + 1) + "-" + v9.getDate()) : String(v9);
      var iso9 = t9.match(/^(\d{4})-(\d{1,2})-(\d{1,2})/), br9 = t9.match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})/);
      var ano9, mes9, dia9;
      if (iso9) { ano9 = Number(iso9[1]); mes9 = Number(iso9[2]); dia9 = Number(iso9[3]); }
      else if (br9) { dia9 = Number(br9[1]); mes9 = Number(br9[2]); ano9 = Number(br9[3]); }
      else return null;
      if (!(mes9 >= 1 && mes9 <= 12)) return null;
      return { ano: String(ano9), trimestre: "T" + (Math.floor((mes9 - 1) / 3) + 1), mes: (mes9 < 10 ? "0" : "") + mes9 + "-" + MESES_C[mes9 - 1], dia: (dia9 < 10 ? "0" : "") + dia9 };
    }
    var calcLinha = {};
    function materializaDerivados() {
      var c9, j9, l9, p9, k9;
      for (c9 in calcLinha) {
        var fn9 = calcLinha[c9].fn;
        for (j9 = 0; j9 < dados.length; j9++) { var r9 = fn9(dados[j9]); dados[j9][c9] = (r9 === r9 && r9 !== Infinity && r9 !== -Infinity) ? r9 : null; }
      }
      for (c9 in datasAgrup) {
        for (j9 = 0; j9 < dados.length; j9++) {
          l9 = dados[j9]; p9 = partesData(l9[c9]);
          for (k9 = 0; k9 < datasAgrup[c9].length; k9++) l9[c9 + "__" + datasAgrup[c9][k9]] = p9 ? p9[datasAgrup[c9][k9]] : "";
        }
      }
      for (c9 in faixas) {
        var f9 = faixas[c9];
        for (j9 = 0; j9 < dados.length; j9++) {
          l9 = dados[j9];
          var n9 = Number(l9[c9]);
          if (n9 !== n9 || l9[c9] == null || l9[c9] === "") { l9[c9 + "__faixa"] = ""; continue; }
          var ini9 = Math.floor((n9 - f9.inicio) / f9.tamanho) * f9.tamanho + f9.inicio;
          l9[c9 + "__faixa"] = (1e12 + ini9) + "|" + fmt.numero(ini9, 0) + "\u2013" + fmt.numero(ini9 + f9.tamanho, 0);
        }
      }
    }
    function rotuloValor(campo9, v9) {
      var c9 = porCampoC[campo9];
      if (v9 === VAZIO || v9 == null || v9 === "") return (c9 && c9.nulo != null) ? String(c9.nulo) : "(vazio)";
      var t9 = String(v9);
      /* ReplaceValues: mapa valor -> rotulo exibido, igual ao do grid plano */
      if (c9 && c9.valores && Object.prototype.hasOwnProperty.call(c9.valores, t9)) return String(c9.valores[t9]);
      if (/__faixa$/.test(campo9) && t9.indexOf("|") > 0) return t9.slice(t9.indexOf("|") + 1);
      return t9;
    }
    function registraDerivado(campo9, sufixo9, titulo9) {
      var nome9 = campo9 + "__" + sufixo9;
      if (!porCampoC[nome9]) { var d9 = { campo: nome9, titulo: titulo9, derivadoDe: campo9 }; campos.push(d9); porCampoC[nome9] = d9; }
      return nome9;
    }
    function log(ev, extra) {
      var e2 = { ev: "phx.cubo." + ev, t: Date.now() }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgC.aoLog) try { cfgC.aoLog(e2); } catch (e9) {}
      return e2;
    }
    function rotulo(c9) { return (porCampoC[c9] && porCampoC[c9].titulo) || c9; }
    function fmtMedida(m9, v9, ehTotal9) {
      if (v9 == null || v9 !== v9) return opc.vazio === "" ? "" : String(opc.vazio);
      if (m9.mostrarComo && String(m9.mostrarComo).charAt(0) === "%") return fmt.percentual(v9 * 100, 1);
      if (m9.mostrarComo === "ranking") return String(v9);
      var src9 = porCampoC[m9.campo];
      /* TotalRowFormat: so nos totais/subtotais, nunca nas celulas normais */
      var ov9 = ehTotal9 && src9 && src9.formatoTotal ? src9.formatoTotal : null;
      var t9 = (ov9 && ov9.tipo) || m9.tipo || (src9 && src9.tipo) || "numero";
      if (ov9 && ov9.decimais != null) return t9 === "moeda" ? fmt.moeda(v9) : (t9 === "percentual" ? fmt.percentual(v9) : fmt.numero(v9, ov9.decimais));
      if (m9.agregador === "count") return fmt.numero(v9, 0);
      if (t9 === "moeda") return fmt.moeda(v9);
      if (t9 === "percentual") return fmt.percentual(v9);
      return fmt.numero(v9, m9.decimais == null ? 2 : m9.decimais);
    }
    function passaFiltros(l9) {
      var c9, f9, j9, ok9;
      for (c9 in filtrosTop) {
        f9 = filtrosTop[c9]; ok9 = false;
        for (j9 = 0; j9 < f9.length; j9++) if (String(l9[c9] == null ? "" : l9[c9]) === f9[j9]) { ok9 = true; break; }
        if (!ok9) return false;
      }
      for (c9 in lay.filtros) {
        f9 = lay.filtros[c9];
        if (!f9 || !f9.length) continue;
        ok9 = false;
        for (j9 = 0; j9 < f9.length; j9++) if (String(l9[c9]) === String(f9[j9])) { ok9 = true; break; }
        if (!ok9) return false;
      }
      return true;
    }
    function distintos(campo9) {
      var vistos = mapaVazio(), out = [], j9, v9;
      for (j9 = 0; j9 < dados.length; j9++) {
        v9 = dados[j9][campo9];
        if (v9 == null) v9 = "";
        if (!Object.prototype.hasOwnProperty.call(vistos, String(v9))) { vistos[String(v9)] = 1; out.push(v9); }
      }
      out.sort(function (a, b) { var na = Number(a), nb = Number(b); if (na === na && nb === nb && a !== "" && b !== "") return na - nb; return String(a) < String(b) ? -1 : (String(a) > String(b) ? 1 : 0); });
      return out;
    }
    /* ---- motor ---- */
    /* mapaVazio(): objeto SEM prototipo. Com `{}` normal, atribuir a chave "__proto__" nao cria
       propriedade propria -- muda o prototipo do objeto. Resultado: hasOwnProperty nunca via a
       chave, um no novo era criado a CADA registro e o grupo aparecia DUPLICADO na tela, com o
       mesmo valor somado duas vezes. (achado pelo fuzz, v0.73) */
    function mapaVazio() { return Object.create ? Object.create(null) : {}; }
    function noArvore() { return { filhos: mapaVazio(), ordem: [], celulas: mapaVazio() }; }
    function caminho(l9, dims) { var p9 = [], j9; for (j9 = 0; j9 < dims.length; j9++) { var v9 = l9[dims[j9]]; p9.push(v9 == null || v9 === "" ? VAZIO : String(v9)); } return p9; }
    function insere(raiz, p9) {
      var n9 = raiz, j9;
      for (j9 = 0; j9 < p9.length; j9++) {
        /* hasOwnProperty e nao truthiness: `filhos["constructor"]` devolve o herdado de
           Object.prototype (truthy!), o grupo nunca era criado e sumia da tela -- mas o
           valor continuava no total geral. Total != soma das linhas, em silencio.
           (achado pelo fuzz, v0.73) */
        if (!Object.prototype.hasOwnProperty.call(n9.filhos, p9[j9])) { n9.filhos[p9[j9]] = noArvore(); n9.filhos[p9[j9]].chave = p9[j9]; n9.filhos[p9[j9]].nivel = j9; n9.ordem.push(p9[j9]); }
        n9 = n9.filhos[p9[j9]];
      }
      return n9;
    }
    function cmpNatural(a, b) { var na = Number(a), nb = Number(b); if (na === na && nb === nb && a !== "" && b !== "") return na - nb; return a < b ? -1 : (a > b ? 1 : 0); }
    function ordena(n9, dims9, pref9, eixo9, celulas9) {
      var dim9 = dims9 ? dims9[pref9.length] : null, o9 = dim9 ? ordens[dim9] : null;
      if (o9 && o9.por === "valor" && celulas9) {
        var m9 = o9.medida || 0;
        n9.ordem.sort(function (a, b) {
          var ka = pref9.concat([a]).join(SEP), kb = pref9.concat([b]).join(SEP);
          var ca = celulas9[eixo9 === "L" ? ka + "|" : "|" + ka], cb = celulas9[eixo9 === "L" ? kb + "|" : "|" + kb];
          var va = ca && ca[m9] != null ? ca[m9] : -Infinity, vb = cb && cb[m9] != null ? cb[m9] : -Infinity;
          return o9.dir === "desc" ? vb - va : va - vb;
        });
      } else {
        n9.ordem.sort(cmpNatural);
        if (o9 && o9.dir === "desc") n9.ordem.reverse();
      }
      var j9; for (j9 = 0; j9 < n9.ordem.length; j9++) ordena(n9.filhos[n9.ordem[j9]], dims9, pref9.concat([n9.ordem[j9]]), eixo9, celulas9);
    }
    var modelo = null;
    function calcula() {
      var raizL = noArvore(), raizC = noArvore(), brutos = mapaVazio(), j9, m9, l9, pl, pc, k9, v9;
      var filtrados = 0;
      /* laco quente: nada de slice/join por celula e nada de guardar valor a valor.
         Cada chave carrega, por medida, quatro acumuladores: n, soma, min, max. */
      var nM8 = lay.valores.length, campos8 = [], aggs8 = [], calc8 = [], i9, i8, m8;
      for (m8 = 0; m8 < nM8; m8++) { campos8.push(lay.valores[m8].campo); aggs8.push(lay.valores[m8].agregador || "sum"); calc8.push(!!lay.valores[m8].formula); }
      var prefL8 = [], prefC8 = [], acc8;
      for (j9 = 0; j9 < dados.length; j9++) {
        l9 = dados[j9];
        if (!passaFiltros(l9)) continue;
        filtrados++;
        pl = caminho(l9, lay.linhas); pc = caminho(l9, lay.colunas);
        insere(raizL, pl); insere(raizC, pc);
        prefL8.length = 0; prefC8.length = 0;
        prefL8.push(""); acc8 = "";
        for (i9 = 0; i9 < pl.length; i9++) { acc8 = i9 ? acc8 + SEP + pl[i9] : pl[i9]; prefL8.push(acc8); }
        prefC8.push(""); acc8 = "";
        for (i8 = 0; i8 < pc.length; i8++) { acc8 = i8 ? acc8 + SEP + pc[i8] : pc[i8]; prefC8.push(acc8); }
        for (i9 = 0; i9 < prefL8.length; i9++) {
          var kl8 = prefL8[i9] + "|";
          for (i8 = 0; i8 < prefC8.length; i8++) {
            k9 = kl8 + prefC8[i8];
            var ac9 = Object.prototype.hasOwnProperty.call(brutos, k9) ? brutos[k9] : null;
            if (!ac9) {
              ac9 = brutos[k9] = [];
              for (m9 = 0; m9 < nM8; m9++) { ac9.push(0); ac9.push(0); ac9.push(Infinity); ac9.push(-Infinity); }
            }
            for (m9 = 0; m9 < nM8; m9++) {
              if (calc8[m9]) continue;
              var raw9 = l9[campos8[m9]], b9 = m9 * 4;
              v9 = Number(raw9);
              /* nao-finito fica FORA da agregacao, igual a nulo: um unico Infinity poluia
                 a soma e o total inteiro virava null na tela (achado pelo fuzz, v0.73) */
              if (v9 === v9 && v9 !== Infinity && v9 !== -Infinity && raw9 != null && raw9 !== "") {
                ac9[b9]++; ac9[b9 + 1] += v9;
                if (v9 < ac9[b9 + 2]) ac9[b9 + 2] = v9;
                if (v9 > ac9[b9 + 3]) ac9[b9 + 3] = v9;
              } else if (aggs8[m9] === "count") ac9[b9]++;
            }
          }
        }
      }
      var celulas = {};
      for (k9 in brutos) {
        var ac8 = brutos[k9], cel8 = [];
        for (m9 = 0; m9 < nM8; m9++) {
          if (calc8[m9]) { cel8.push(null); continue; }
          var b8 = m9 * 4, n8 = ac8[b8], a8 = aggs8[m9];
          if (!n8) { cel8.push(null); continue; }
          cel8.push(a8 === "count" ? n8 : (a8 === "avg" ? ac8[b8 + 1] / n8 : (a8 === "min" ? (ac8[b8 + 2] === Infinity ? null : ac8[b8 + 2]) : (a8 === "max" ? (ac8[b8 + 3] === -Infinity ? null : ac8[b8 + 3]) : ac8[b8 + 1]))));
        }
        celulas[k9] = cel8;
      }
      /* medidas calculadas: formula sobre as outras medidas da mesma celula */
      var temCalc = false, i7;
      for (m9 = 0; m9 < lay.valores.length; m9++) if (lay.valores[m9].formula) temCalc = true;
      if (temCalc) {
        var fns9 = [];
        for (m9 = 0; m9 < lay.valores.length; m9++) fns9.push(lay.valores[m9].formula ? compilaFormula(lay.valores[m9].formula) : null);
        for (k9 in celulas) {
          var pseudo9 = {};
          for (i7 = 0; i7 < lay.valores.length; i7++) pseudo9[lay.valores[i7].nome || lay.valores[i7].campo] = celulas[k9][i7];
          for (m9 = 0; m9 < lay.valores.length; m9++) if (fns9[m9] && fns9[m9].ok) { var r7 = fns9[m9].fn(pseudo9); celulas[k9][m9] = (r7 === r7 && r7 !== Infinity && r7 !== -Infinity) ? r7 : null; }
        }
      }
      var brutas9 = {};
      for (k9 in celulas) brutas9[k9] = celulas[k9].slice();
      ordena(raizL, lay.linhas, [], "L", celulas);
      ordena(raizC, lay.colunas, [], "C", celulas);
      /* Top N vira filtro real: segunda passada com os N maiores da dimensao */
      var dimT9, precisa9 = false;
      for (dimT9 in topN) if (topN[dimT9] && !filtrosTop[dimT9]) precisa9 = true;
      if (precisa9) {
        for (dimT9 in topN) {
          if (!topN[dimT9] || filtrosTop[dimT9]) continue;
          var ixL9 = lay.linhas.indexOf(dimT9), ixC9 = lay.colunas.indexOf(dimT9);
          var raizT9 = ixL9 >= 0 ? raizL : (ixC9 >= 0 ? raizC : null), eixoT9 = ixL9 >= 0 ? "L" : "C";
          if (!raizT9) continue;
          var nivelT9 = ixL9 >= 0 ? ixL9 : ixC9, soma9 = {}, cand9 = [], kk9;
          (function anda(n8, pref8) {
            var q8;
            if (pref8.length === nivelT9) {
              for (q8 = 0; q8 < n8.ordem.length; q8++) {
                var kf8 = pref8.concat([n8.ordem[q8]]).join(SEP);
                var c8 = celulas[eixoT9 === "L" ? kf8 + "|" : "|" + kf8];
                var v8 = c8 && c8[topN[dimT9].medida || 0] != null ? c8[topN[dimT9].medida || 0] : 0;
                soma9[n8.ordem[q8]] = (soma9[n8.ordem[q8]] || 0) + v8;
              }
              return;
            }
            for (q8 = 0; q8 < n8.ordem.length; q8++) anda(n8.filhos[n8.ordem[q8]], pref8.concat([n8.ordem[q8]]));
          })(raizT9, []);
          for (kk9 in soma9) cand9.push([kk9, soma9[kk9]]);
          cand9.sort(function (a, b) { return b[1] - a[1]; });
          var manter9 = [], q9;
          for (q9 = 0; q9 < cand9.length && q9 < topN[dimT9].n; q9++) manter9.push(cand9[q9][0]);
          filtrosTop[dimT9] = manter9;
        }
        return calcula();
      }
      /* mostrar valores como */
      var folhasC9 = folhasCol(raizC, [], []), m8, kk8;
      for (m8 = 0; m8 < lay.valores.length; m8++) {
        var como9 = lay.valores[m8].mostrarComo;
        if (!como9 || como9 === "normal") continue;
        var geral9 = brutas9["|"] ? brutas9["|"][m8] : null;
        if (como9 === "%total" || como9 === "%linha" || como9 === "%coluna") {
          for (kk8 in celulas) {
            var corte9 = kk8.indexOf("|"), pL9 = kk8.slice(0, corte9), pC9 = kk8.slice(corte9 + 1);
            var base9 = como9 === "%total" ? geral9 : (como9 === "%linha" ? (brutas9[pL9 + "|"] ? brutas9[pL9 + "|"][m8] : null) : (brutas9["|" + pC9] ? brutas9["|" + pC9][m8] : null));
            celulas[kk8][m8] = (brutas9[kk8][m8] == null || !base9) ? null : brutas9[kk8][m8] / base9;
          }
        } else if (como9 === "acumulado") {
          for (kk8 in celulas) {
            if (kk8.charAt(kk8.length - 1) !== "|") continue;
            var pl8 = kk8.slice(0, -1), ac8 = 0, q7;
            for (q7 = 0; q7 < folhasC9.length; q7++) {
              var kc7 = pl8 + "|" + folhasC9[q7].join(SEP);
              if (celulas[kc7] && brutas9[kc7][m8] != null) { ac8 += brutas9[kc7][m8]; celulas[kc7][m8] = ac8; }
            }
          }
        } else if (como9 === "ranking") {
          (function rank(n8, pref8) {
            var q8, r8, kcAll = [""], z8;
            for (z8 = 0; z8 < folhasC9.length; z8++) kcAll.push(folhasC9[z8].join(SEP));
            for (r8 = 0; r8 < kcAll.length; r8++) {
              var lista8 = [];
              for (q8 = 0; q8 < n8.ordem.length; q8++) {
                var kk7 = pref8.concat([n8.ordem[q8]]).join(SEP) + "|" + kcAll[r8];
                lista8.push([kk7, brutas9[kk7] && brutas9[kk7][m8] != null ? brutas9[kk7][m8] : -Infinity]);
              }
              lista8.sort(function (a, b) { return b[1] - a[1]; });
              for (q8 = 0; q8 < lista8.length; q8++) if (celulas[lista8[q8][0]]) celulas[lista8[q8][0]][m8] = lista8[q8][1] === -Infinity ? null : q8 + 1;
            }
            for (q8 = 0; q8 < n8.ordem.length; q8++) rank(n8.filhos[n8.ordem[q8]], pref8.concat([n8.ordem[q8]]));
          })(raizL, []);
        }
      }
      modelo = { raizL: raizL, raizC: raizC, celulas: celulas, brutas: brutas9, filtrados: filtrados };
      return modelo;
    }
    function folhasCol(n9, pref, out) {
      if (!n9.ordem.length) { out.push(pref); return out; }
      var j9; for (j9 = 0; j9 < n9.ordem.length; j9++) folhasCol(n9.filhos[n9.ordem[j9]], pref.concat([n9.ordem[j9]]), out);
      return out;
    }
    function linhasPlanas(n9, pref, out) {
      var j9, ch9;
      for (j9 = 0; j9 < n9.ordem.length; j9++) {
        ch9 = n9.filhos[n9.ordem[j9]];
        var p9 = pref.concat([n9.ordem[j9]]);
        var kk = p9.join(SEP);
        var temF = ch9.ordem.length > 0;
        var aberto = temF && !recolhidos[kk];
        out.push({ caminho: p9, nivel: p9.length - 1, temFilhos: temF, aberto: aberto });
        if (temF && aberto) linhasPlanas(ch9, p9, out);
      }
      return out;
    }
    /* ---- UI ---- */
    var raiz = el("div", "phx-cubo" + (cfgC.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = ""; no.appendChild(raiz);
    var arrasto = null;
    function chip(campo9, zona9, extra9) {
      var f9 = lay.filtros[campo9];
      var temF = f9 && f9.length;
      return '<span class="phx-cb-chip' + (temF ? " phx-cb-filtrado" : "") + '" draggable="true" data-campo="' + esc(campo9) + '" data-zona="' + zona9 + '">' +
        esc(extra9 || rotulo(campo9)) +
        (zona9 !== "valores" ? '<button type="button" class="phx-cb-filtro" data-campo="' + esc(campo9) + '" title="filtrar valores">\u25be</button>' : "") +
        (zona9 !== "campos" ? '<button type="button" class="phx-cb-x" data-campo="' + esc(campo9) + '" data-zona="' + zona9 + '" title="remover">\u00d7</button>' : "") + "</span>";
    }
    function desenha() {
      if (adiado) { pendente = true; return; }
      pendente = false;
      materializaDerivados();
      var M = calcula();
      var h = [], j9, k9, m9;
      var usados = {};
      for (j9 = 0; j9 < lay.linhas.length; j9++) usados[lay.linhas[j9]] = 1;
      for (j9 = 0; j9 < lay.colunas.length; j9++) usados[lay.colunas[j9]] = 1;
      /* barra de filtros de relatorio, no topo (como no Excel) */
      var kf9, temFR9 = false;
      for (kf9 in lay.filtros) { temFR9 = true; break; }
      if (temFR9) {
        h.push('<div class="phx-cb-relatorio">');
        for (kf9 in lay.filtros) {
          var sel9 = lay.filtros[kf9], txt9 = !sel9 || !sel9.length ? "(Todos)" : (sel9.length === 1 ? rotuloValor(kf9, sel9[0]) : "(" + sel9.length + " itens)");
          h.push('<span class="phx-cb-fr"><b>' + esc(rotulo(kf9)) + ":</b> <button type=\"button\" class=\"phx-cb-filtro phx-cb-frbtn\" data-campo=\"" + esc(kf9) + '">' + esc(txt9) + " \u25be</button></span>");
        }
        h.push("</div>");
      }
      h.push('<div class="phx-cb-painel' + (opc.painelCampos ? "" : " phx-cb-painel-oculto") + '">');
      h.push('<div class="phx-cb-lista"><div class="phx-cb-rot">' + esc(T("camposRelatorio")) + '</div><input class="phx-cb-busca" type="text" placeholder="' + esc(T("buscar")) + '"><div class="phx-cb-itens">');
      for (j9 = 0; j9 < campos.length; j9++) {
        var cc9 = campos[j9], zn9 = usados[cc9.campo] ? 1 : 0, ehMed9 = false, q8;
        for (q8 = 0; q8 < lay.valores.length; q8++) if (lay.valores[q8].campo === cc9.campo) ehMed9 = true;
        h.push('<label class="phx-cb-item" data-campo="' + esc(cc9.campo) + '"><input type="checkbox" data-cb="' + esc(cc9.campo) + '"' + (zn9 || ehMed9 ? " checked" : "") + '>' +
          '<span class="phx-cb-nome' + (zn9 || ehMed9 ? " phx-cb-usado" : "") + '">' + esc(rotulo(cc9.campo)) + "</span>" +
          (cc9.dimensao === false ? '<span class="phx-cb-sig">\u03a3</span>' : "") + "</label>");
      }
      h.push("</div></div><div class=\"phx-cb-zonas\">");
      h.push('<div class="phx-cb-zona" data-zona="campos"><span class="phx-cb-rot">Campos</span>');
      for (j9 = 0; j9 < campos.length; j9++) if (!usados[campos[j9].campo] && campos[j9].dimensao !== false) h.push(chip(campos[j9].campo, "campos"));
      h.push("</div>");
      h.push('<div class="phx-cb-zona" data-zona="filtros"><span class="phx-cb-rot">\u25bd Filtros</span>');
      for (k9 in lay.filtros) if (lay.filtros[k9] && lay.filtros[k9].length && !usados[k9]) h.push(chip(k9, "filtros", rotulo(k9) + " (" + lay.filtros[k9].length + ")"));
      h.push('<span class="phx-cb-dica">arraste um campo aqui ou use \u25be no chip</span></div>');
      h.push('<div class="phx-cb-zona" data-zona="linhas"><span class="phx-cb-rot">\u2261 Linhas</span>');
      for (j9 = 0; j9 < lay.linhas.length; j9++) h.push(chip(lay.linhas[j9], "linhas"));
      h.push("</div>");
      h.push('<div class="phx-cb-zona" data-zona="colunas"><span class="phx-cb-rot">\u2016 Colunas</span>');
      for (j9 = 0; j9 < lay.colunas.length; j9++) h.push(chip(lay.colunas[j9], "colunas"));
      h.push("</div>");
      h.push('<div class="phx-cb-zona" data-zona="valores"><span class="phx-cb-rot">\u03a3 Valores</span>');
      for (j9 = 0; j9 < lay.valores.length; j9++) h.push('<span class="phx-cb-chip phx-cb-medida" data-ix="' + j9 + '">' + esc(lay.valores[j9].titulo || rotulo(lay.valores[j9].campo)) + ' <i>' + (lay.valores[j9].formula ? "fx" : (lay.valores[j9].agregador || "sum")) + (lay.valores[j9].mostrarComo && lay.valores[j9].mostrarComo !== "normal" ? " \u00b7 " + lay.valores[j9].mostrarComo : "") + '</i><button type="button" class="phx-cb-medcfg" data-ix="' + j9 + '" title="configura\u00e7\u00f5es do campo de valor">\u2699</button></span>');
      h.push("</div></div>");
      h.push('<div class="phx-cb-rodapainel"><label><input type="checkbox" class="phx-cb-adiar"' + (adiado ? " checked" : "") + '> ' + esc(T("adiarLayout")) + '</label><button type="button" class="phx-cb-atualizar"' + (adiado && pendente ? "" : " disabled") + ">" + esc(T("atualizar")) + "</button></div>");
      h.push("</div>");
      /* tabela */
      var folhas = folhasCol(M.raizC, [], []);
      var nM = lay.valores.length || 1;
      var nivC = lay.colunas.length;
      var vLinhas9 = opc.valoresEm === "linhas" && nM > 1;
      var tabular9 = (opc.layout === "tabular" || opc.layout === "esboco") && lay.linhas.length > 1;
      var nRot9 = tabular9 ? lay.linhas.length : 1;
      h.push('<div class="phx-cb-tela"><table class="phx-cb-tab"><thead>');
      for (j9 = 0; j9 < nivC; j9++) {
        h.push("<tr>");
        if (j9 === 0) {
          if (tabular9) { var qr9; for (qr9 = 0; qr9 < nRot9; qr9++) h.push('<th class="phx-cb-canto" rowspan="' + (nivC + (nM > 1 ? 1 : 0)) + '">' + esc(rotulo(lay.linhas[qr9])) + "</th>"); }
          else h.push('<th class="phx-cb-canto" rowspan="' + (nivC + (nM > 1 ? 1 : 0)) + '">' + esc(lay.linhas.map(rotulo).join(" \u203a ") || " ") + "</th>");
        }
        var grupoAnt = null, span = 0, rotAnt = null, ini9 = 0;
        for (k9 = 0; k9 <= folhas.length; k9++) {
          var chaveG = k9 < folhas.length ? folhas[k9].slice(0, j9 + 1).join(SEP) : null;
          if (chaveG !== grupoAnt) {
            if (grupoAnt !== null) h.push('<th class="phx-cb-colcab" colspan="' + (span * (vLinhas9 ? 1 : nM)) + '">' + esc(rotuloValor(lay.colunas[j9], rotAnt)) + "</th>");
            grupoAnt = chaveG; span = 0; rotAnt = k9 < folhas.length ? folhas[k9][j9] : null;
          }
          span++;
        }
        if (j9 === 0 && opc.totaisLinha) h.push('<th class="phx-cb-colcab phx-cb-tot" colspan="' + (vLinhas9 ? 1 : nM) + '" rowspan="' + nivC + '">' + esc(T("totais")) + "</th>");
        h.push("</tr>");
      }
      if (nivC === 0 && tabular9) { var qr8; h.push("<tr>"); for (qr8 = 0; qr8 < nRot9; qr8++) h.push('<th class="phx-cb-canto"' + (nM > 1 ? ' rowspan="2"' : "") + ">" + esc(rotulo(lay.linhas[qr8])) + "</th>"); h.push('<th class="phx-cb-colcab phx-cb-tot" colspan="' + (vLinhas9 ? 1 : nM) + '">' + esc(T("totais")) + "</th></tr>"); }
      else if (nivC === 0) { h.push('<tr><th class="phx-cb-canto"' + (nM > 1 ? ' rowspan="2"' : "") + ">" + esc(lay.linhas.map(rotulo).join(" \u203a ") || " ") + '</th><th class="phx-cb-colcab phx-cb-tot" colspan="' + nM + '">Totais</th></tr>'); }
      if (nM > 1 && !vLinhas9) {
        h.push("<tr>");
        for (k9 = 0; k9 < folhas.length; k9++) for (m9 = 0; m9 < nM; m9++) h.push('<th class="phx-cb-medcab">' + esc(lay.valores[m9].titulo || rotulo(lay.valores[m9].campo)) + "</th>");
        if (opc.totaisLinha) for (m9 = 0; m9 < nM; m9++) h.push('<th class="phx-cb-medcab phx-cb-tot">' + esc(lay.valores[m9].titulo || rotulo(lay.valores[m9].campo)) + "</th>");
        h.push("</tr>");
      }
      h.push("</thead><tbody>");
      var planas = linhasPlanas(M.raizL, [], []);
      var mv9;
      function celulasRotulo(pl8) {
        /* uma coluna por nivel: preenche o proprio e herda os de cima quando repetirRotulos */
        var out8 = [], q8, txt8;
        for (q8 = 0; q8 < nRot9; q8++) {
          if (q8 < pl8.nivel) txt8 = opc.repetirRotulos ? rotuloValor(lay.linhas[q8], pl8.caminho[q8]) : "";
          else if (q8 === pl8.nivel) txt8 = rotuloValor(lay.linhas[q8], pl8.caminho[q8]);
          else txt8 = "";
          out8.push('<th class="phx-cb-rotl phx-cb-rotn' + q8 + (q8 < pl8.nivel ? " phx-cb-rot-herdado" : "") + '"' + (vLinhas9 ? ' rowspan="' + (nM + 1) + '"' : "") + ">" +
            (q8 === pl8.nivel && pl8.temFilhos ? '<button type="button" class="phx-cb-tg" data-kl="' + esc(pl8.caminho.join(SEP)) + '">' + (pl8.aberto ? "\u229f" : "\u229e") + "</button>" : "") + esc(txt8) + "</th>");
        }
        return out8.join("");
      }
      function celulasDe(kl, soMedida) {
        /* mm8 e local de proposito: usar m9 aqui corrompia o contador do laco que chama esta funcao */
        var out = [], k8, v8, mm8, ini8 = soMedida == null ? 0 : soMedida, fim8 = soMedida == null ? nM : soMedida + 1;
        var ehLinhaTotal8 = (kl === ""); /* a linha "Total" geral chama celulasDe("") */
        for (k8 = 0; k8 < folhas.length; k8++) {
          v8 = M.celulas[kl + "|" + folhas[k8].join(SEP)];
          for (mm8 = ini8; mm8 < fim8; mm8++) out.push('<td class="phx-cb-cel' + (v8 && v8[mm8] != null && v8[mm8] < 0 ? " phx-cb-neg" : "") + '" data-kl="' + esc(kl) + '" data-kc="' + esc(folhas[k8].join(SEP)) + '" data-m="' + mm8 + '">' + (v8 ? esc(fmtMedida(lay.valores[mm8], v8[mm8], ehLinhaTotal8)) : esc(String(opc.vazio))) + "</td>");
        }
        if (opc.totaisLinha) {
          v8 = M.celulas[kl + "|"];
          for (mm8 = ini8; mm8 < fim8; mm8++) out.push('<td class="phx-cb-cel phx-cb-tot" data-kl="' + esc(kl) + '" data-kc="" data-m="' + mm8 + '">' + (v8 ? esc(fmtMedida(lay.valores[mm8], v8[mm8], true)) : "") + "</td>");
        }
        return out.join("");
      }
      for (j9 = 0; j9 < planas.length; j9++) {
        var pl9 = planas[j9], kl9 = pl9.caminho.join(SEP);
        var campoN9 = lay.linhas[pl9.nivel];
        var rot9 = rotuloValor(campoN9, pl9.caminho[pl9.nivel]);
        var mostraCel9 = opc.subtotais || !pl9.temFilhos || !pl9.aberto;
        var tagN9 = porCampoC[campoN9] && porCampoC[campoN9].tag;
        var attrTag9 = (tagN9 != null && typeof tagN9 !== "object") ? ' data-tag="' + esc(String(tagN9)) + '"' : "";
        var cabLinha9 = tabular9 ? celulasRotulo(pl9) : ('<th class="phx-cb-rotl' + (opc.quebraLinha ? " phx-cb-quebra" : "") + '"' + attrTag9 + ' style="padding-left:' + (8 + pl9.nivel * 18) + 'px"' + (vLinhas9 ? ' rowspan="' + (nM + 1) + '"' : "") + ">" +
          (pl9.temFilhos ? '<button type="button" class="phx-cb-tg" data-kl="' + esc(kl9) + '">' + (pl9.aberto ? "\u229f" : "\u229e") + "</button>" : '<span class="phx-cb-folha"></span>') + esc(rot9) + "</th>");
        if (!vLinhas9) {
          h.push('<tr class="phx-cb-linha phx-cb-n' + pl9.nivel + (pl9.temFilhos ? " phx-cb-pai" : "") + '" data-kl="' + esc(kl9) + '">' + cabLinha9 + (mostraCel9 ? celulasDe(kl9) : "") + "</tr>");
        } else {
          h.push('<tr class="phx-cb-linha phx-cb-n' + pl9.nivel + (pl9.temFilhos ? " phx-cb-pai" : "") + '" data-kl="' + esc(kl9) + '">' + cabLinha9 + '<th class="phx-cb-medrot"></th>' + (mostraCel9 ? "" : "") + "</tr>");
          for (mv9 = 0; mv9 < nM; mv9++) h.push('<tr class="phx-cb-linha phx-cb-medlin" data-kl="' + esc(kl9) + '"><th class="phx-cb-medrot">' + esc(lay.valores[mv9].titulo || rotulo(lay.valores[mv9].campo)) + "</th>" + (mostraCel9 ? celulasDe(kl9, mv9) : "") + "</tr>");
        }
      }
      if (opc.totaisColuna) {
        if (!vLinhas9) h.push('<tr class="phx-cb-geral"><th class="phx-cb-rotl"' + (tabular9 ? ' colspan="' + nRot9 + '"' : "") + ">" + esc(T("totais")) + "</th>" + celulasDe("") + "</tr>");
        else {
          h.push('<tr class="phx-cb-geral"><th class="phx-cb-rotl" rowspan="' + (nM + 1) + '">' + esc(T("totais")) + '</th><th class="phx-cb-medrot"></th></tr>');
          for (mv9 = 0; mv9 < nM; mv9++) h.push('<tr class="phx-cb-geral"><th class="phx-cb-medrot">' + esc(lay.valores[mv9].titulo || rotulo(lay.valores[mv9].campo)) + "</th>" + celulasDe("", mv9) + "</tr>");
        }
      }
      h.push("</tbody></table></div>");
      raiz.innerHTML = h.join("");
      liga();
      var jg9;
      for (jg9 = 0; jg9 < _graficos.length; jg9++) desenhaGrafico(_graficos[jg9]);
      log("desenho", { linhas: planas.length, colunas: folhas.length, medidas: nM, registros: M.filtrados });
    }
    function abrePopFiltro(campo9, ancora9) {
      var vals = distintos(campo9), sel = lay.filtros[campo9] || [], j9, marc = {};
      for (j9 = 0; j9 < sel.length; j9++) marc[String(sel[j9])] = 1;
      var pop = document.createElement("div");
      pop.className = "phx-cb-pop";
      var h = ['<div class="phx-cb-poptit">' + esc(rotulo(campo9)) + ' <button type="button" data-a="todos">todos</button><button type="button" data-a="nenhum">nenhum</button></div><div class="phx-cb-poplista">'];
      for (j9 = 0; j9 < vals.length; j9++) h.push('<label><input type="checkbox" value="' + esc(String(vals[j9])) + '"' + (!sel.length || marc[String(vals[j9])] ? " checked" : "") + "> " + esc(vals[j9] === "" ? "(vazio)" : String(vals[j9])) + "</label>");
      h.push('</div><div class="phx-cb-popacoes"><button type="button" data-a="ok">\u2713 Aplicar</button><button type="button" data-a="x">\u2717</button></div>');
      pop.innerHTML = h.join("");
      raiz.appendChild(pop);
      var r9 = ancora9.getBoundingClientRect(), r0 = raiz.getBoundingClientRect();
      pop.style.left = Math.max(0, r9.left - r0.left) + "px";
      pop.style.top = (r9.bottom - r0.top + 4) + "px";
      function fecha() { if (pop.parentNode) pop.parentNode.removeChild(pop); }
      pop.addEventListener("click", function (ev) {
        var a9 = ev.target.getAttribute && ev.target.getAttribute("data-a");
        if (!a9) return;
        var cbs = pop.querySelectorAll("input[type=checkbox]"), k9;
        if (a9 === "todos" || a9 === "nenhum") { for (k9 = 0; k9 < cbs.length; k9++) cbs[k9].checked = a9 === "todos"; return; }
        if (a9 === "x") { fecha(); return; }
        var esc9 = [], todos9 = true;
        for (k9 = 0; k9 < cbs.length; k9++) { if (cbs[k9].checked) esc9.push(cbs[k9].value); else todos9 = false; }
        fecha();
        apiC.filtrar(campo9, todos9 ? [] : esc9);
      });
    }
    function popBase(ancora9, htmlInterno9, larg9) {
      var pop9 = document.createElement("div");
      pop9.className = "phx-cb-pop";
      pop9.innerHTML = htmlInterno9;
      if (larg9) pop9.style.minWidth = larg9 + "px";
      raiz.appendChild(pop9);
      var r9 = ancora9.getBoundingClientRect(), r0 = raiz.getBoundingClientRect();
      pop9.style.left = Math.max(0, r9.left - r0.left) + "px";
      pop9.style.top = (r9.bottom - r0.top + 4) + "px";
      function fecha9(ev9) { if (ev9 && pop9.contains(ev9.target)) return; if (pop9.parentNode) pop9.parentNode.removeChild(pop9); document.removeEventListener("mousedown", fecha9, true); }
      setTimeout(function () { document.addEventListener("mousedown", fecha9, true); }, 0);
      return { el: pop9, fechar: function () { fecha9(); } };
    }
    function abrePopCampo(campo9, ancora9) {
      var o9 = ordens[campo9] || {}, t9 = topN[campo9];
      var med9 = "", q9;
      for (q9 = 0; q9 < lay.valores.length; q9++) med9 += '<option value="' + q9 + '"' + ((o9.medida || 0) === q9 ? " selected" : "") + ">" + esc(lay.valores[q9].titulo || rotulo(lay.valores[q9].campo)) + "</option>";
      var h9 = '<div class="phx-cb-poptit">' + esc(rotulo(campo9)) + '</div><div class="phx-cb-popcorpo">' +
        '<div class="phx-cb-linhaopc"><b>Ordenar</b><button type="button" data-a="asc"' + (o9.dir === "asc" && o9.por !== "valor" ? ' class="on"' : "") + '>A \u2192 Z</button><button type="button" data-a="desc"' + (o9.dir === "desc" && o9.por !== "valor" ? ' class="on"' : "") + '>Z \u2192 A</button><button type="button" data-a="limpar">manual</button></div>' +
        (lay.valores.length ? '<div class="phx-cb-linhaopc"><b>Por valor</b><select data-s="medida">' + med9 + '</select><button type="button" data-a="valasc">\u2191</button><button type="button" data-a="valdesc">\u2193</button></div>' : "") +
        '<div class="phx-cb-linhaopc"><b>Top N</b><input type="number" min="1" data-i="topn" value="' + (t9 ? t9.n : "") + '" placeholder="todos"><button type="button" data-a="topn">aplicar</button><button type="button" data-a="semtopn">limpar</button></div>' +
        '<div class="phx-cb-linhaopc"><b>Agrupar data</b><button type="button" data-a="d-ano">Ano</button><button type="button" data-a="d-at">Ano+Trim</button><button type="button" data-a="d-am">Ano+M\u00eas</button><button type="button" data-a="d-limpar">desfazer</button></div>' +
        '<div class="phx-cb-linhaopc"><b>Faixas</b><input type="number" data-i="faixa" placeholder="tamanho"><button type="button" data-a="faixa">aplicar</button><button type="button" data-a="semfaixa">desfazer</button></div>' +
        '<div class="phx-cb-linhaopc"><b>Mover</b><button type="button" data-a="z-filtros">\u25bd Filtros</button><button type="button" data-a="z-linhas">\u2261 Linhas</button><button type="button" data-a="z-colunas">\u2016 Colunas</button><button type="button" data-a="z-fora">\u00d7 remover</button></div></div>';
      var p9 = popBase(ancora9, h9, 340);
      p9.el.addEventListener("click", function (ev9) {
        var a9 = ev9.target.getAttribute && ev9.target.getAttribute("data-a");
        if (!a9) return;
        var elM8 = p9.el.querySelector("[data-s=medida]"), elT8 = p9.el.querySelector("[data-i=topn]"), elF8 = p9.el.querySelector("[data-i=faixa]");
        var med8 = Number(elM8 ? elM8.value : 0), nTop8 = Number(elT8 ? elT8.value : 0), fx8 = Number(elF8 ? elF8.value : 0);
        var zonaAtual8 = lay.colunas.indexOf(campo9) >= 0 ? "colunas" : "linhas";
        p9.fechar();
        if (a9 === "asc" || a9 === "desc") apiC.ordenar(campo9, a9, { por: "rotulo" });
        else if (a9 === "limpar") apiC.ordenar(campo9, null);
        else if (a9 === "valasc" || a9 === "valdesc") apiC.ordenar(campo9, a9 === "valasc" ? "asc" : "desc", { por: "valor", medida: med8 });
        else if (a9 === "topn") apiC.topN(campo9, nTop8, { medida: med8 });
        else if (a9 === "semtopn") apiC.topN(campo9, 0);
        else if (a9 === "d-ano") apiC.agruparData(campo9, ["ano"], { zona: zonaAtual8 });
        else if (a9 === "d-at") apiC.agruparData(campo9, ["ano", "trimestre"], { zona: zonaAtual8 });
        else if (a9 === "d-am") apiC.agruparData(campo9, ["ano", "mes"], { zona: zonaAtual8 });
        else if (a9 === "d-limpar") apiC.agruparData(campo9, null, { zona: zonaAtual8 });
        else if (a9 === "faixa") apiC.agruparFaixa(campo9, fx8, { zona: zonaAtual8 });
        else if (a9 === "semfaixa") apiC.agruparFaixa(campo9, 0, { zona: zonaAtual8 });
        else if (a9.indexOf("z-") === 0) { var z8 = a9.slice(2); if (z8 === "fora") { apiC.remover(campo9, "linhas"); apiC.remover(campo9, "colunas"); apiC.remover(campo9, "filtros"); } else apiC.mover(campo9, z8, zonaAtual8); }
      });
    }
    function abrePopMedida(ix9, ancora9) {
      var v9 = lay.valores[ix9] || {};
      var aggs9 = ["sum", "count", "avg", "min", "max"], comos9 = [["normal", "Normal"], ["%total", "% do total geral"], ["%linha", "% da linha"], ["%coluna", "% da coluna"], ["acumulado", "Acumulado"], ["ranking", "Ranking"]];
      var h9 = '<div class="phx-cb-poptit">' + esc(v9.titulo || rotulo(v9.campo)) + '</div><div class="phx-cb-popcorpo"><div class="phx-cb-linhaopc"><b>Resumir por</b>', q9;
      for (q9 = 0; q9 < aggs9.length; q9++) h9 += '<button type="button" data-g="' + aggs9[q9] + '"' + ((v9.agregador || "sum") === aggs9[q9] ? ' class="on"' : "") + ">" + aggs9[q9] + "</button>";
      h9 += '</div><div class="phx-cb-linhaopc" style="flex-wrap:wrap"><b>Mostrar como</b>';
      for (q9 = 0; q9 < comos9.length; q9++) h9 += '<button type="button" data-c="' + comos9[q9][0] + '"' + ((v9.mostrarComo || "normal") === comos9[q9][0] ? ' class="on"' : "") + ">" + esc(comos9[q9][1]) + "</button>";
      h9 += "</div></div>";
      var p9 = popBase(ancora9, h9, 330);
      p9.el.addEventListener("click", function (ev9) {
        var g9 = ev9.target.getAttribute && ev9.target.getAttribute("data-g"), c9 = ev9.target.getAttribute && ev9.target.getAttribute("data-c");
        if (g9) { p9.fechar(); apiC.agregadorDe(ix9, g9); }
        else if (c9) { p9.fechar(); apiC.mostrarComo(ix9, c9); }
      });
    }
    var CORES_CB = ["#0969da", "#1a7f37", "#bf8700", "#cf222e", "#8250df", "#0598bc", "#e16f24", "#6e7781", "#b35900", "#2da44e", "#a475f9", "#57606a"];
    function dadosGrafico(g9) {
      var o9 = g9.opts || {}, m9 = o9.medida || 0, maxCat9 = o9.maximo || 12;
      var M9 = modelo || calcula();
      var cats9 = [], j9, k9;
      for (j9 = 0; j9 < M9.raizL.ordem.length && j9 < maxCat9; j9++) cats9.push(M9.raizL.ordem[j9]);
      var folhas9 = folhasCol(M9.raizC, [], []), series9 = [];
      if (!lay.colunas.length || o9.empilhar === "total" || o9.serieUnica) {
        var pts9 = [];
        for (j9 = 0; j9 < cats9.length; j9++) { var c9 = M9.celulas[cats9[j9] + "|"]; pts9.push(c9 && c9[m9] != null ? c9[m9] : 0); }
        series9.push({ nome: lay.valores[m9] ? (lay.valores[m9].titulo || rotulo(lay.valores[m9].campo)) : "Total", pontos: pts9 });
      } else {
        for (k9 = 0; k9 < folhas9.length && k9 < (o9.maxSeries || 8); k9++) {
          var pts8 = [];
          for (j9 = 0; j9 < cats9.length; j9++) { var c8 = M9.celulas[cats9[j9] + "|" + folhas9[k9].join(SEP)]; pts8.push(c8 && c8[m9] != null ? c8[m9] : 0); }
          series9.push({ nome: folhas9[k9].map(function (v, i) { return rotuloValor(lay.colunas[i], v); }).join(" / "), pontos: pts8, caminho: folhas9[k9] });
        }
      }
      var rotsC9 = [];
      for (j9 = 0; j9 < cats9.length; j9++) rotsC9.push(rotuloValor(lay.linhas[0], cats9[j9]) || "(vazio)");
      return { categorias: cats9, rotulos: rotsC9, series: series9, medida: lay.valores[m9] || {} };
    }
    function desenhaGrafico(g9) {
      var o9 = g9.opts || {}, tipo9 = o9.tipo || "colunas";
      var D9 = dadosGrafico(g9), W9 = o9.largura || 720, H9 = o9.altura || 300;
      var padE9 = 62, padB9 = 46, padT9 = o9.titulo ? 30 : 14, padD9 = 12;
      var svg9 = [], j9, k9, s9, v9;
      var maxV9 = 0, minV9 = 0, emp9 = tipo9 === "barras-empilhadas" || o9.empilhado;
      for (j9 = 0; j9 < D9.categorias.length; j9++) {
        var soma9 = 0;
        for (k9 = 0; k9 < D9.series.length; k9++) { v9 = D9.series[k9].pontos[j9] || 0; soma9 += v9; if (!emp9) { if (v9 > maxV9) maxV9 = v9; if (v9 < minV9) minV9 = v9; } }
        if (emp9 && soma9 > maxV9) maxV9 = soma9;
      }
      if (maxV9 === 0 && minV9 === 0) maxV9 = 1;
      var altP9 = H9 - padT9 - padB9, y0_9 = padT9 + altP9 * (maxV9 / (maxV9 - minV9 || 1));
      function yDe(v8) { return padT9 + altP9 * ((maxV9 - v8) / ((maxV9 - minV9) || 1)); }
      svg9.push('<svg class="phx-cb-svg" viewBox="0 0 ' + W9 + " " + H9 + '" width="100%" xmlns="http://www.w3.org/2000/svg" role="img">');
      if (o9.titulo) svg9.push('<text x="' + (W9 / 2) + '" y="18" font-size="13" font-weight="700" text-anchor="middle" fill="currentColor">' + esc(o9.titulo) + "</text>");
      /* eixo e grades */
      var passos9 = 4, p9;
      for (p9 = 0; p9 <= passos9; p9++) {
        var val9 = minV9 + (maxV9 - minV9) * p9 / passos9, yy9 = yDe(val9);
        svg9.push('<line x1="' + padE9 + '" y1="' + yy9.toFixed(1) + '" x2="' + (W9 - padD9) + '" y2="' + yy9.toFixed(1) + '" stroke="currentColor" stroke-opacity="0.12"/>');
        svg9.push('<text x="' + (padE9 - 6) + '" y="' + (yy9 + 4).toFixed(1) + '" font-size="10" text-anchor="end" fill="currentColor" opacity="0.7">' + esc(fmtMedida(D9.medida, val9)) + "</text>");
      }
      var larguraP9 = (W9 - padE9 - padD9) / Math.max(1, D9.categorias.length);
      if (tipo9 === "pizza") {
        var tot8 = 0, cx8 = (W9 - 180) / 2, cy8 = padT9 + altP9 / 2, R8 = Math.min(altP9, W9 - 200) / 2 - 6, ang8 = -Math.PI / 2;
        svg9 = ['<svg class="phx-cb-svg" viewBox="0 0 ' + W9 + " " + H9 + '" width="100%" xmlns="http://www.w3.org/2000/svg" role="img">'];
        if (o9.titulo) svg9.push('<text x="' + (W9 / 2) + '" y="18" font-size="13" font-weight="700" text-anchor="middle" fill="currentColor">' + esc(o9.titulo) + "</text>");
        for (j9 = 0; j9 < D9.categorias.length; j9++) { var t8 = 0; for (k9 = 0; k9 < D9.series.length; k9++) t8 += D9.series[k9].pontos[j9] || 0; tot8 += Math.max(0, t8); }
        for (j9 = 0; j9 < D9.categorias.length; j9++) {
          var vv8 = 0; for (k9 = 0; k9 < D9.series.length; k9++) vv8 += D9.series[k9].pontos[j9] || 0;
          var fr8 = tot8 ? Math.max(0, vv8) / tot8 : 0, a1_8 = ang8 + fr8 * Math.PI * 2;
          var x1_8 = cx8 + R8 * Math.cos(ang8), y1_8 = cy8 + R8 * Math.sin(ang8), x2_8 = cx8 + R8 * Math.cos(a1_8), y2_8 = cy8 + R8 * Math.sin(a1_8);
          svg9.push('<path class="phx-cb-fatia" data-cat="' + esc(D9.categorias[j9]) + '" d="M' + cx8 + "," + cy8 + " L" + x1_8.toFixed(1) + "," + y1_8.toFixed(1) + " A" + R8 + "," + R8 + " 0 " + (fr8 > 0.5 ? 1 : 0) + ",1 " + x2_8.toFixed(1) + "," + y2_8.toFixed(1) + ' Z" fill="' + CORES_CB[j9 % CORES_CB.length] + '"><title>' + esc(D9.rotulos[j9]) + ": " + esc(fmtMedida(D9.medida, vv8)) + " (" + (fr8 * 100).toFixed(1) + "%)</title></path>");
          svg9.push('<rect x="' + (W9 - 168) + '" y="' + (padT9 + 6 + j9 * 18) + '" width="11" height="11" fill="' + CORES_CB[j9 % CORES_CB.length] + '"/><text x="' + (W9 - 152) + '" y="' + (padT9 + 16 + j9 * 18) + '" font-size="11" fill="currentColor">' + esc(D9.rotulos[j9]) + "</text>");
          ang8 = a1_8;
        }
      } else if (tipo9 === "linhas" || tipo9 === "area") {
        for (k9 = 0; k9 < D9.series.length; k9++) {
          s9 = D9.series[k9];
          var pts9 = [];
          for (j9 = 0; j9 < D9.categorias.length; j9++) pts9.push((padE9 + larguraP9 * (j9 + 0.5)).toFixed(1) + "," + yDe(s9.pontos[j9] || 0).toFixed(1));
          if (tipo9 === "area") svg9.push('<polygon points="' + (padE9 + larguraP9 * 0.5).toFixed(1) + "," + yDe(0).toFixed(1) + " " + pts9.join(" ") + " " + (padE9 + larguraP9 * (D9.categorias.length - 0.5)).toFixed(1) + "," + yDe(0).toFixed(1) + '" fill="' + CORES_CB[k9 % CORES_CB.length] + '" fill-opacity="0.25"/>');
          svg9.push('<polyline points="' + pts9.join(" ") + '" fill="none" stroke="' + CORES_CB[k9 % CORES_CB.length] + '" stroke-width="2"/>');
          for (j9 = 0; j9 < D9.categorias.length; j9++) svg9.push('<circle class="phx-cb-pt" data-cat="' + esc(D9.categorias[j9]) + '" data-s="' + k9 + '" cx="' + (padE9 + larguraP9 * (j9 + 0.5)).toFixed(1) + '" cy="' + yDe(s9.pontos[j9] || 0).toFixed(1) + '" r="3" fill="' + CORES_CB[k9 % CORES_CB.length] + '"><title>' + esc(D9.rotulos[j9] + " \u00b7 " + s9.nome + ": " + fmtMedida(D9.medida, s9.pontos[j9] || 0)) + "</title></circle>");
        }
      } else {
        var nS9 = D9.series.length, lb9 = (larguraP9 * 0.8) / (emp9 ? 1 : Math.max(1, nS9));
        for (j9 = 0; j9 < D9.categorias.length; j9++) {
          var acc9 = 0;
          for (k9 = 0; k9 < nS9; k9++) {
            v9 = D9.series[k9].pontos[j9] || 0;
            var xb9 = padE9 + larguraP9 * j9 + larguraP9 * 0.1 + (emp9 ? 0 : k9 * lb9);
            var yb9 = emp9 ? yDe(acc9 + Math.max(0, v9)) : yDe(Math.max(0, v9)), hb9 = Math.abs(yDe(v9) - yDe(0));
            if (emp9) { yb9 = yDe(acc9 + v9); acc9 += v9; }
            svg9.push('<rect class="phx-cb-barra" data-cat="' + esc(D9.categorias[j9]) + '" data-s="' + k9 + '" x="' + xb9.toFixed(1) + '" y="' + Math.min(yb9, yDe(0)).toFixed(1) + '" width="' + lb9.toFixed(1) + '" height="' + Math.max(1, hb9).toFixed(1) + '" fill="' + CORES_CB[k9 % CORES_CB.length] + '" rx="2"><title>' + esc(D9.rotulos[j9] + " \u00b7 " + D9.series[k9].nome + ": " + fmtMedida(D9.medida, v9)) + "</title></rect>");
          }
        }
      }
      if (tipo9 !== "pizza") {
        svg9.push('<line x1="' + padE9 + '" y1="' + yDe(0).toFixed(1) + '" x2="' + (W9 - padD9) + '" y2="' + yDe(0).toFixed(1) + '" stroke="currentColor" stroke-opacity="0.35"/>');
        for (j9 = 0; j9 < D9.categorias.length; j9++) svg9.push('<text x="' + (padE9 + larguraP9 * (j9 + 0.5)).toFixed(1) + '" y="' + (H9 - padB9 + 16) + '" font-size="10" text-anchor="middle" fill="currentColor">' + esc(String(D9.rotulos[j9]).slice(0, 12)) + "</text>");
        if (D9.series.length > 1 && o9.legenda !== false) {
          for (k9 = 0; k9 < D9.series.length; k9++) {
            var lx9 = padE9 + (k9 % 4) * 150, ly9 = H9 - 18 + Math.floor(k9 / 4) * 14;
            svg9.push('<rect x="' + lx9 + '" y="' + (ly9 - 8) + '" width="10" height="10" fill="' + CORES_CB[k9 % CORES_CB.length] + '"/><text x="' + (lx9 + 15) + '" y="' + ly9 + '" font-size="10.5" fill="currentColor">' + esc(String(D9.series[k9].nome).slice(0, 18)) + "</text>");
          }
        }
      }
      svg9.push("</svg>");
      g9.no.innerHTML = svg9.join("");
      /* clique numa categoria filtra o cubo pela dimensao de linha */
      if (o9.clicaFiltra !== false && lay.linhas.length) {
        var alvos9 = g9.no.querySelectorAll("[data-cat]"), q9;
        for (q9 = 0; q9 < alvos9.length; q9++) (function (el9) {
          el9.style.cursor = "pointer";
          el9.addEventListener("click", function () {
            var cat9 = el9.getAttribute("data-cat"), atual9 = lay.filtros[lay.linhas[0]] || [];
            apiC.filtrar(lay.linhas[0], (atual9.length === 1 && atual9[0] === cat9) ? [] : [cat9]);
          });
        })(alvos9[q9]);
      }
      return D9;
    }
    function abrePopDetalhe(regs9, ancora9) {
      var cols9 = [], j9, k9, vistos9 = {};
      for (j9 = 0; j9 < Math.min(regs9.length, 30); j9++) for (k9 in regs9[j9]) if (!vistos9[k9] && k9.indexOf("__") < 0) { vistos9[k9] = 1; cols9.push(k9); }
      var h9 = '<div class="phx-cb-poptit">' + regs9.length + ' registro(s)</div><div class="phx-cb-popdet"><table><tr>';
      for (k9 = 0; k9 < cols9.length; k9++) h9 += "<th>" + esc(rotulo(cols9[k9])) + "</th>";
      h9 += "</tr>";
      for (j9 = 0; j9 < Math.min(regs9.length, 50); j9++) {
        h9 += "<tr>";
        for (k9 = 0; k9 < cols9.length; k9++) h9 += "<td>" + esc(String(regs9[j9][cols9[k9]] == null ? "" : regs9[j9][cols9[k9]])) + "</td>";
        h9 += "</tr>";
      }
      h9 += "</table>" + (regs9.length > 50 ? "<p>mostrando 50 de " + regs9.length + "</p>" : "") + "</div>";
      popBase(ancora9, h9, 520);
    }
    function liga() {
      var chips = raiz.querySelectorAll(".phx-cb-chip[draggable]"), j9;
      for (j9 = 0; j9 < chips.length; j9++) (function (c9) {
        arrastePonteiro(c9, {
          rotulo: function () { return rotulo(c9.getAttribute("data-campo")); },
          aoMover: function (sob) {
            var zs = raiz.querySelectorAll(".phx-cb-zona"), q;
            var alvo9 = sobe(sob, "phx-cb-zona");
            for (q = 0; q < zs.length; q++) zs[q].className = zs[q] === alvo9 ? "phx-cb-zona phx-cb-alvo" : "phx-cb-zona";
          },
          aoSoltar: function (sob) {
            var alvo9 = sobe(sob, "phx-cb-zona");
            var zs = raiz.querySelectorAll(".phx-cb-zona"), q;
            for (q = 0; q < zs.length; q++) zs[q].className = "phx-cb-zona";
            if (alvo9) apiC.mover(c9.getAttribute("data-campo"), alvo9.getAttribute("data-zona"), c9.getAttribute("data-zona"));
          }
        });
        c9.addEventListener("dragstart", function (ev) {
          arrasto = { campo: c9.getAttribute("data-campo"), de: c9.getAttribute("data-zona") };
          if (ev.dataTransfer && ev.dataTransfer.setData) try { ev.dataTransfer.setData("text/plain", arrasto.campo); } catch (e9) {}
        });
      })(chips[j9]);
      var zonas = raiz.querySelectorAll(".phx-cb-zona"), k9;
      for (k9 = 0; k9 < zonas.length; k9++) (function (z9) {
        z9.addEventListener("dragover", function (ev) { ev.preventDefault(); z9.className = "phx-cb-zona phx-cb-alvo"; });
        z9.addEventListener("dragleave", function () { z9.className = "phx-cb-zona"; });
        z9.addEventListener("drop", function (ev) {
          ev.preventDefault(); z9.className = "phx-cb-zona";
          var campo9 = (arrasto && arrasto.campo) || (ev.dataTransfer && ev.dataTransfer.getData ? ev.dataTransfer.getData("text/plain") : null);
          var de9 = arrasto ? arrasto.de : null;
          arrasto = null;
          if (campo9) apiC.mover(campo9, z9.getAttribute("data-zona"), de9);
        });
      })(zonas[k9]);
      var xs = raiz.querySelectorAll(".phx-cb-x"), i9;
      for (i9 = 0; i9 < xs.length; i9++) (function (b9) {
        b9.addEventListener("click", function (ev) { ev.stopPropagation(); apiC.remover(b9.getAttribute("data-campo"), b9.getAttribute("data-zona")); });
      })(xs[i9]);
      var fs = raiz.querySelectorAll(".phx-cb-filtro");
      for (i9 = 0; i9 < fs.length; i9++) (function (b9) {
        b9.addEventListener("click", function (ev) { ev.stopPropagation(); abrePopFiltro(b9.getAttribute("data-campo"), b9); });
      })(fs[i9]);
      var cbs9 = raiz.querySelectorAll("[data-cb]"), q9;
      for (q9 = 0; q9 < cbs9.length; q9++) (function (cb9) {
        cb9.addEventListener("change", function () {
          var campo9 = cb9.getAttribute("data-cb"), def9 = porCampoC[campo9];
          if (cb9.checked) apiC.mover(campo9, def9 && def9.dimensao === false ? "valores" : "linhas", "campos");
          else { apiC.remover(campo9, "linhas"); apiC.remover(campo9, "colunas"); apiC.remover(campo9, "filtros"); apiC.remover(campo9, "valores"); }
        });
      })(cbs9[q9]);
      var bu9 = raiz.querySelector(".phx-cb-busca");
      if (bu9) {
        bu9.value = _buscaCampos;
        var filtraLista9 = function () {
          _buscaCampos = bu9.value;
          var t9 = bu9.value.toLowerCase(), its9 = raiz.querySelectorAll(".phx-cb-item"), z9;
          for (z9 = 0; z9 < its9.length; z9++) its9[z9].style.display = (!t9 || its9[z9].textContent.toLowerCase().indexOf(t9) >= 0) ? "" : "none";
        };
        bu9.addEventListener("input", filtraLista9);
        if (_buscaCampos) filtraLista9();
      }
      var ad9 = raiz.querySelector(".phx-cb-adiar");
      if (ad9) ad9.addEventListener("change", function () { apiC.adiar(ad9.checked); var b8 = raiz.querySelector(".phx-cb-atualizar"); if (b8) b8.disabled = !ad9.checked; });
      var at9 = raiz.querySelector(".phx-cb-atualizar");
      if (at9) at9.addEventListener("click", function () { apiC.atualizarAgora(); });
      var cfgs9 = raiz.querySelectorAll(".phx-cb-medcfg");
      for (q9 = 0; q9 < cfgs9.length; q9++) (function (b9) {
        b9.addEventListener("click", function (ev9) { ev9.stopPropagation(); abrePopMedida(Number(b9.getAttribute("data-ix")), b9); });
      })(cfgs9[q9]);
      var chipsM9 = raiz.querySelectorAll(".phx-cb-chip[data-campo]");
      for (q9 = 0; q9 < chipsM9.length; q9++) (function (c9) {
        c9.addEventListener("contextmenu", function (ev9) { ev9.preventDefault(); abrePopCampo(c9.getAttribute("data-campo"), c9); });
        c9.addEventListener("dblclick", function () { abrePopCampo(c9.getAttribute("data-campo"), c9); });
      })(chipsM9[q9]);
      var cels9 = raiz.querySelectorAll("td.phx-cb-cel");
      for (q9 = 0; q9 < cels9.length; q9++) (function (td9) {
        td9.addEventListener("dblclick", function () {
          var kl9 = td9.getAttribute("data-kl"), kc9 = td9.getAttribute("data-kc");
          var regs9 = apiC.detalhar(kl9 ? kl9.split(SEP) : [], kc9 ? kc9.split(SEP) : []);
          if (typeof cfgC.aoDetalhar === "function") { try { cfgC.aoDetalhar(regs9, { linha: kl9, coluna: kc9, medida: Number(td9.getAttribute("data-m")) }); } catch (e9) {} }
          else abrePopDetalhe(regs9, td9);
        });
      })(cels9[q9]);
      var tgs = raiz.querySelectorAll(".phx-cb-tg");
      for (i9 = 0; i9 < tgs.length; i9++) (function (b9) {
        b9.addEventListener("click", function () { apiC.alternar(b9.getAttribute("data-kl").split(SEP)); });
      })(tgs[i9]);
    }
    function semDuplicata(arr, campo9) { var o = [], j9; for (j9 = 0; j9 < arr.length; j9++) if (arr[j9] !== campo9) o.push(arr[j9]); return o; }
    var apiC = {
      ok: true,
      el: raiz,
      /* ---- ordenacao (rotulo ou valor) ---- */
      ordenar: function (campo9, dir9, opts9) {
        if (!_exigeCampo("ordenar", campo9)) return apiC;
        opts9 = opts9 || {};
        if (!dir9) delete ordens[campo9]; else ordens[campo9] = { dir: dir9, por: opts9.por || "rotulo", medida: opts9.medida || 0 };
        log("ordenar", { campo: campo9, dir: dir9 || null, por: (ordens[campo9] || {}).por || null });
        desenha();
        return apiC;
      },
      ordenacoes: function () { return JSON.parse(JSON.stringify(ordens)); },
      /* ---- Top N (filtro de valor) ---- */
      topN: function (campo9, n9, opts9) {
        if (!_exigeCampo("topN", campo9)) return apiC;
        /* 0 e null LIMPAM o Top N (contrato existente); so numero negativo ou nao-numero e erro de uso */
        if (n9 != null && (Number(n9) !== Number(n9) || Number(n9) < 0))
          return _erroUso("topN", "n deve ser numero >= 0 (0 ou null limpam o Top N)") || apiC;
        opts9 = opts9 || {};
        delete filtrosTop[campo9];
        if (!n9) delete topN[campo9]; else topN[campo9] = { n: Number(n9), medida: opts9.medida || 0 };
        log("topn", { campo: campo9, n: n9 || null });
        desenha();
        return apiC;
      },
      /* ---- agrupar datas (ano/trimestre/mes/dia) ---- */
      agruparData: function (campo9, niveis9, opts9) {
        if (!_exigeCampo("agruparData", campo9)) return apiC;
        /* aceita "ano" alem de ["ano"] -- antes lancava excecao. Lista VAZIA desliga o agrupamento
           de data (contrato existente), entao nao e erro de uso. */
        niveis9 = _comoLista(niveis9);
        opts9 = opts9 || {};
        var ns9 = niveis9 || ["ano", "mes"], j9, nomes9 = [];
        if (!niveis9 || !niveis9.length) {
          for (j9 = 0; j9 < (datasAgrup[campo9] || []).length; j9++) apiC.remover(campo9 + "__" + datasAgrup[campo9][j9], opts9.zona || "linhas");
          delete datasAgrup[campo9];
          desenha();
          return apiC;
        }
        datasAgrup[campo9] = ns9.slice();
        materializaDerivados();
        var rot9 = { ano: "Ano", trimestre: "Trimestre", mes: "M\u00eas", dia: "Dia" }, base9 = (porCampoC[campo9] && porCampoC[campo9].titulo) || campo9;
        for (j9 = 0; j9 < ns9.length; j9++) nomes9.push(registraDerivado(campo9, ns9[j9], base9 + " \u00b7 " + (rot9[ns9[j9]] || ns9[j9])));
        var zona9 = opts9.zona || (lay.colunas.indexOf(campo9) >= 0 ? "colunas" : "linhas");
        apiC.remover(campo9, zona9);
        for (j9 = 0; j9 < nomes9.length; j9++) apiC.mover(nomes9[j9], zona9, "campos");
        log("agrupar-data", { campo: campo9, niveis: ns9.join(","), zona: zona9 });
        return apiC;
      },
      /* ---- agrupar numeros em faixas ---- */
      agruparFaixa: function (campo9, tamanho9, opts9) {
        if (!_exigeCampo("agruparFaixa", campo9)) return apiC;
        /* 0 DESLIGA o agrupamento por faixa (contrato existente) -- so nao-numero ou negativo e erro */
        if (tamanho9 != null && (Number(tamanho9) !== Number(tamanho9) || Number(tamanho9) < 0))
          return _erroUso("agruparFaixa", "tamanho deve ser numero >= 0 (0 desliga a faixa)") || apiC;
        opts9 = opts9 || {};
        if (!tamanho9) { delete faixas[campo9]; apiC.remover(campo9 + "__faixa", opts9.zona || "linhas"); desenha(); return apiC; }
        faixas[campo9] = { tamanho: Number(tamanho9), inicio: opts9.inicio || 0 };
        materializaDerivados();
        var nome9 = registraDerivado(campo9, "faixa", ((porCampoC[campo9] && porCampoC[campo9].titulo) || campo9) + " \u00b7 faixas de " + fmt.numero(tamanho9, 0));
        var zona9 = opts9.zona || "linhas";
        apiC.remover(campo9, zona9);
        apiC.mover(nome9, zona9, "campos");
        log("agrupar-faixa", { campo: campo9, tamanho: Number(tamanho9) });
        return apiC;
      },
      /* ---- mostrar valores como ---- */
      mostrarComo: function (ixMedida9, como9) {
        if (!(Number(ixMedida9) >= 0) || !lay.valores[Number(ixMedida9)])
          return _erroUso("mostrarComo", "indice de medida invalido: " + ixMedida9) || apiC;
        var v9 = lay.valores[ixMedida9];
        if (!v9) return { ok: false, erro: "medida inexistente" };
        v9.mostrarComo = como9 || "normal";
        log("mostrar-como", { medida: ixMedida9, como: v9.mostrarComo });
        desenha();
        return { ok: true, medida: ixMedida9, como: v9.mostrarComo };
      },
      /* ---- agregador da medida ---- */
      agregadorDe: function (ixMedida9, agg9) {
        var v9 = lay.valores[ixMedida9];
        if (!v9) return { ok: false, erro: "medida inexistente" };
        if (!AGG_CUBO[agg9]) return { ok: false, erro: "agregador desconhecido: " + agg9 };
        v9.agregador = agg9;
        log("agregador", { medida: ixMedida9, agregador: agg9 });
        desenha();
        return { ok: true };
      },
      /* ---- campo calculado (sobre as medidas) ---- */
      /* campo calculado sobre CADA LINHA de origem (e depois agregado) */
      campoLinha: function (nome9, formula9, opts9) {
        opts9 = opts9 || {};
        var c9 = compilaFormula(formula9);
        if (!c9.ok) return { ok: false, erro: c9.erro };
        calcLinha[nome9] = { formula: formula9, fn: c9.fn };
        if (!porCampoC[nome9]) { var d9 = { campo: nome9, titulo: opts9.titulo || nome9, tipo: opts9.tipo || "numero", dimensao: opts9.dimensao === true }; campos.push(d9); porCampoC[nome9] = d9; }
        materializaDerivados();
        if (opts9.zona !== false) apiC.mover(nome9, opts9.zona || "valores", "campos");
        else desenha();
        log("campo-linha", { nome: nome9, formula: formula9 });
        return { ok: true, nome: nome9 };
      },
      camposLinha: function () { var o9 = {}, k9; for (k9 in calcLinha) o9[k9] = calcLinha[k9].formula; return o9; },
      campoCalculado: function (nome9, formula9, opts9) {
        opts9 = opts9 || {};
        var teste9 = compilaFormula(formula9);
        if (!teste9.ok) return { ok: false, erro: teste9.erro };
        var j9;
        for (j9 = 0; j9 < lay.valores.length; j9++) if ((lay.valores[j9].nome || lay.valores[j9].campo) === nome9) lay.valores.splice(j9--, 1);
        lay.valores.push({ campo: nome9, nome: nome9, titulo: opts9.titulo || nome9, formula: formula9, tipo: opts9.tipo || "numero", decimais: opts9.decimais });
        log("campo-calculado", { nome: nome9, formula: formula9 });
        desenha();
        return { ok: true, nome: nome9 };
      },
      /* ---- opcoes de layout ---- */
      /* Tag no cubo: mesma semantica do grid plano -- consulta, alteracao em runtime e data-tag no DOM */
      tag: function (campo9, valor9) {
        var c9 = porCampoC[campo9];
        if (!c9) return valor9 === undefined ? undefined : apiC;
        if (valor9 === undefined) return c9.tag;
        c9.tag = valor9;
        log("tag", { campo: campo9 });
        desenha();
        return apiC;
      },
      camposPorTag: function (valor9) {
        var o9 = [], j9, t9;
        for (j9 = 0; j9 < campos.length; j9++) {
          t9 = campos[j9].tag;
          if (t9 === valor9) { o9.push(campos[j9].campo); continue; }
          if (t9 instanceof Array && t9.indexOf(valor9) >= 0) o9.push(campos[j9].campo);
        }
        return o9;
      },
      opcoes: function (o9) {
        if (o9 === undefined) return JSON.parse(JSON.stringify(opc));
        var k9;
        for (k9 in o9) if (opc[k9] !== undefined) opc[k9] = o9[k9];
        log("opcoes", { valoresEm: opc.valoresEm, subtotais: opc.subtotais, totaisLinha: opc.totaisLinha, totaisColuna: opc.totaisColuna });
        desenha();
        return apiC;
      },
      /* ---- adiar atualizacao (Defer Layout Update) ---- */
      adiar: function (lig9) {
        adiado = lig9 !== false;
        log("adiar", { adiado: adiado });
        if (!adiado && pendente) desenha();
        return apiC;
      },
      atualizarAgora: function () { var p9 = pendente || adiado; adiado = false; desenha(); log("atualizar", { pendente: !!p9 }); return apiC; },
      pendente: function () { return pendente; },
      /* ---- drill-through: registros por tras de uma celula ---- */
      detalhar: function (caminhoL9, caminhoC9) {
        var out9 = [], j9, l9, ok9, k9;
        caminhoL9 = caminhoL9 || []; caminhoC9 = caminhoC9 || [];
        for (j9 = 0; j9 < dados.length; j9++) {
          l9 = dados[j9];
          if (!passaFiltros(l9)) continue;
          ok9 = true;
          for (k9 = 0; k9 < caminhoL9.length; k9++) if (String(l9[lay.linhas[k9]] == null ? "" : l9[lay.linhas[k9]]) !== String(caminhoL9[k9])) { ok9 = false; break; }
          if (ok9) for (k9 = 0; k9 < caminhoC9.length; k9++) if (String(l9[lay.colunas[k9]] == null ? "" : l9[lay.colunas[k9]]) !== String(caminhoC9[k9])) { ok9 = false; break; }
          if (ok9) out9.push(_origDe(l9));   /* devolve o registro DO CHAMADOR, nao a copia interna */
        }
        log("detalhar", { linhas: out9.length, caminhoL: caminhoL9.join("/"), caminhoC: caminhoC9.join("/") });
        return out9;
      },
      campos: function () {
        var out9 = [], j9, c9, usado9;
        for (j9 = 0; j9 < campos.length; j9++) {
          c9 = campos[j9];
          usado9 = lay.linhas.indexOf(c9.campo) >= 0 ? "linhas" : (lay.colunas.indexOf(c9.campo) >= 0 ? "colunas" : (lay.filtros[c9.campo] ? "filtros" : null));
          var q9; for (q9 = 0; q9 < lay.valores.length; q9++) if (lay.valores[q9].campo === c9.campo) usado9 = "valores";
          out9.push({ campo: c9.campo, titulo: c9.titulo || c9.campo, zona: usado9, dimensao: c9.dimensao !== false, derivadoDe: c9.derivadoDe || null });
        }
        return out9;
      },
      mover: function (campo9, zona9, de9) {
        if (!_exigeCampo("mover", campo9)) return apiC;
        if (["campos", "filtros", "linhas", "colunas", "valores"].indexOf(String(zona9)) < 0)
          return _erroUso("mover", "zona invalida: \"" + zona9 + "\" (use campos|filtros|linhas|colunas|valores)") || apiC;
        if (!porCampoC[campo9] && zona9 !== "valores") return apiC;
        lay.linhas = semDuplicata(lay.linhas, campo9);
        lay.colunas = semDuplicata(lay.colunas, campo9);
        if (zona9 === "linhas") lay.linhas.push(campo9);
        else if (zona9 === "colunas") lay.colunas.push(campo9);
        else if (zona9 === "valores") lay.valores.push({ campo: campo9, agregador: (porCampoC[campo9] && porCampoC[campo9].agregador) || "sum", tipo: porCampoC[campo9] && porCampoC[campo9].tipo });
        else if (zona9 === "filtros") { if (!lay.filtros[campo9]) lay.filtros[campo9] = []; }
        log("mover", { campo: campo9, de: de9 || null, para: zona9 });
        desenha();
        return apiC;
      },
      remover: function (campo9, zona9) {
        if (zona9 === "linhas") lay.linhas = semDuplicata(lay.linhas, campo9);
        else if (zona9 === "colunas") lay.colunas = semDuplicata(lay.colunas, campo9);
        else if (zona9 === "filtros") delete lay.filtros[campo9];
        else if (zona9 === "valores") { var nv = [], j9; for (j9 = 0; j9 < lay.valores.length; j9++) if (lay.valores[j9].campo !== campo9) nv.push(lay.valores[j9]); lay.valores = nv; }
        log("remover", { campo: campo9, zona: zona9 });
        desenha();
        return apiC;
      },
      filtrar: function (campo9, valores9) {
        if (!_exigeCampo("filtrar", campo9)) return apiC;
        if (!valores9 || !valores9.length) delete lay.filtros[campo9]; else lay.filtros[campo9] = valores9.slice();
        log("filtro", { campo: campo9, n: (valores9 || []).length });
        desenha();
        return apiC;
      },
      limparFiltros: function () { lay.filtros = {}; log("filtros-limpos", {}); desenha(); return apiC; },
      transpor: function () { var t9 = lay.linhas; lay.linhas = lay.colunas; lay.colunas = t9; recolhidos = {}; log("transpor", {}); desenha(); return apiC; },
      alternar: function (caminho9) { var k9 = caminho9.join(SEP); recolhidos[k9] = !recolhidos[k9]; desenha(); return apiC; },
      expandirTudo: function () { recolhidos = {}; desenha(); return apiC; },
      recolherTudo: function (nivel9) {
        recolhidos = {};
        var M = calcula(), pl = linhasPlanasTodas(M.raizL, [], []), j9;
        for (j9 = 0; j9 < pl.length; j9++) if (pl[j9].temFilhos && pl[j9].nivel >= (nivel9 || 0)) recolhidos[pl[j9].caminho.join(SEP)] = true;
        desenha();
        return apiC;
      },
      layout: function () { return JSON.parse(JSON.stringify({ v: 1, linhas: lay.linhas, colunas: lay.colunas, valores: lay.valores, filtros: lay.filtros, recolhidos: recolhidos, ordens: ordens, topN: topN, faixas: faixas, datasAgrup: datasAgrup, opcoes: opc, camposLinha: apiC.camposLinha() })); },
      aplicarLayout: function (l9) {
        if (!l9 || l9.v !== 1) return { ok: false, erro: "layout inv\u00e1lido" };
        lay.linhas = (l9.linhas || []).slice(); lay.colunas = (l9.colunas || []).slice(); lay.valores = JSON.parse(JSON.stringify(l9.valores || []));
        lay.filtros = JSON.parse(JSON.stringify(l9.filtros || {})); recolhidos = JSON.parse(JSON.stringify(l9.recolhidos || {}));
        ordens = JSON.parse(JSON.stringify(l9.ordens || {})); topN = JSON.parse(JSON.stringify(l9.topN || {})); filtrosTop = {};
        faixas = JSON.parse(JSON.stringify(l9.faixas || {})); datasAgrup = JSON.parse(JSON.stringify(l9.datasAgrup || {}));
        if (l9.opcoes) { var ko9; for (ko9 in l9.opcoes) if (opc[ko9] !== undefined) opc[ko9] = l9.opcoes[ko9]; }
        var kd9, kn9;
        for (kd9 in datasAgrup) for (kn9 = 0; kn9 < datasAgrup[kd9].length; kn9++) registraDerivado(kd9, datasAgrup[kd9][kn9], kd9 + " \u00b7 " + datasAgrup[kd9][kn9]);
        for (kd9 in faixas) registraDerivado(kd9, "faixa", kd9 + " \u00b7 faixas");
        var kc9;
        for (kc9 in (l9.camposLinha || {})) { var cc8 = compilaFormula(l9.camposLinha[kc9]); if (cc8.ok) { calcLinha[kc9] = { formula: l9.camposLinha[kc9], fn: cc8.fn }; if (!porCampoC[kc9]) { var dd8 = { campo: kc9, titulo: kc9, tipo: "numero", dimensao: false }; campos.push(dd8); porCampoC[kc9] = dd8; } } }
        materializaDerivados();
        log("layout-aplicado", { linhas: lay.linhas.length, colunas: lay.colunas.length, valores: lay.valores.length });
        desenha();
        return { ok: true };
      },
      estado: function () {
        var M = (modelo && !pendente) ? modelo : calcula(), folhas = folhasCol(M.raizC, [], []), pl = linhasPlanas(M.raizL, [], []);
        var tot = M.celulas["|"] || [];
        return { registros: M.filtrados, linhas: pl.length, colunasFolha: folhas.length, medidas: lay.valores.length, totais: tot.slice(), layout: apiC.layout() };
      },
      valor: function (caminhoL, caminhoC, ixMedida) {
        var M = modelo || calcula();
        var c9 = M.celulas[(caminhoL || []).join(SEP) + "|" + (caminhoC || []).join(SEP)];
        return c9 ? c9[ixMedida || 0] : null;
      },
      tabelaPlana: function () {
        /* achata o cubo como aparece: linhas x (colunas-folha x medidas) + totais */
        var M = calcula(), folhas = folhasCol(M.raizC, [], []), pl = linhasPlanasTodas(M.raizL, [], []), j9, k9, m9, out = [], cab = [];
        cab.push({ campo: "_rotulo", titulo: lay.linhas.map(rotulo).join(" \u203a ") || "Item" });
        for (k9 = 0; k9 < folhas.length; k9++) for (m9 = 0; m9 < lay.valores.length; m9++) cab.push({ campo: "c" + k9 + "_" + m9, titulo: folhas[k9].join(" / ") + (lay.valores.length > 1 ? " \u00b7 " + (lay.valores[m9].titulo || rotulo(lay.valores[m9].campo)) : ""), tipo: lay.valores[m9].tipo || "numero", agregador: lay.valores[m9].agregador });
        for (m9 = 0; m9 < lay.valores.length; m9++) cab.push({ campo: "t_" + m9, titulo: "Totais" + (lay.valores.length > 1 ? " \u00b7 " + (lay.valores[m9].titulo || rotulo(lay.valores[m9].campo)) : ""), tipo: lay.valores[m9].tipo || "numero" });
        function linhaDe(kl, rot) {
          var o9 = { _rotulo: rot }, v8;
          for (k9 = 0; k9 < folhas.length; k9++) { v8 = M.celulas[kl + "|" + folhas[k9].join(SEP)]; for (m9 = 0; m9 < lay.valores.length; m9++) o9["c" + k9 + "_" + m9] = v8 ? v8[m9] : null; }
          v8 = M.celulas[kl + "|"];
          for (m9 = 0; m9 < lay.valores.length; m9++) o9["t_" + m9] = v8 ? v8[m9] : null;
          return o9;
        }
        for (j9 = 0; j9 < pl.length; j9++) out.push(linhaDe(pl[j9].caminho.join(SEP), (new Array(pl[j9].nivel + 1)).join("    ") + (pl[j9].caminho[pl[j9].nivel] || "(vazio)")));
        out.push(linhaDe("", "Totais"));
        return { colunas: cab, linhas: out };
      },
      gerarXLSX: function (opts9) {
        var tp = apiC.tabelaPlana();
        var bytes9 = montaXLSX(tp.colunas, tp.linhas, { aba: (opts9 && opts9.aba) || cfgC.titulo || "Cubo" });
        log("export", { formato: "xlsx", linhas: tp.linhas.length, colunas: tp.colunas.length, bytes: bytes9.length });
        return bytes9;
      },
      copiarTSV: function () {
        var tp = apiC.tabelaPlana(), h = [], j9, k9, l9;
        h.push(tp.colunas.map(function (c) { return c.titulo; }).join("\t"));
        for (j9 = 0; j9 < tp.linhas.length; j9++) { l9 = []; for (k9 = 0; k9 < tp.colunas.length; k9++) { var v9 = tp.linhas[j9][tp.colunas[k9].campo]; l9.push(v9 == null ? "" : (typeof v9 === "number" ? String(v9).replace(".", ",") : String(v9))); } h.push(l9.join("\t")); }
        return h.join("\r\n");
      },
      atualizar: function (novos9) { dados = (novos9 || []).slice(); filtrosTop = {}; materializaDerivados(); desenha(); return apiC; },
      /* PivotChart: fica ligado ao cubo e se redesenha a cada mudanca */
      graficoDinamico: function (alvoG, opts9) {
        var noG9 = typeof alvoG === "string" ? document.querySelector(alvoG) : alvoG;
        if (!noG9) return { ok: false, erro: "alvo n\u00e3o encontrado" };
        var g9 = { no: noG9, opts: opts9 || {} };
        _graficos.push(g9);
        desenhaGrafico(g9);
        log("grafico-ligado", { tipo: g9.opts.tipo || "colunas", total: _graficos.length });
        return {
          ok: true,
          el: noG9,
          opcoes: function (o9) { if (o9 === undefined) return JSON.parse(JSON.stringify(g9.opts)); var k9; for (k9 in o9) g9.opts[k9] = o9[k9]; desenhaGrafico(g9); return this; },
          series: function () { return dadosGrafico(g9).series; },
          redesenhar: function () { desenhaGrafico(g9); return this; },
          destruir: function () { var i9 = _graficos.indexOf(g9); if (i9 >= 0) _graficos.splice(i9, 1); noG9.innerHTML = ""; return true; }
        };
      },
      graficos: function () { return _graficos.length; },
      grafico: function (alvoG, opts9) {
        opts9 = opts9 || {};
        var noG = typeof alvoG === "string" ? document.querySelector(alvoG) : alvoG;
        if (!noG) return { ok: false, erro: "alvo n\u00e3o encontrado" };
        var M = calcula(), m9 = opts9.medida || 0, tipo9 = opts9.tipo || "barras", itens9 = [], j9, ch9;
        for (j9 = 0; j9 < M.raizL.ordem.length; j9++) {
          ch9 = M.raizL.ordem[j9];
          var c9 = M.celulas[ch9 + "|"];
          itens9.push({ rotulo: ch9 === "" ? "(vazio)" : ch9, valor: c9 && c9[m9] != null ? c9[m9] : 0 });
        }
        itens9.sort(function (a, b) { return b.valor - a.valor; });
        var top9 = itens9.slice(0, opts9.maximo || 12), W = opts9.largura || 640, H = opts9.altura || 280, svg = [];
        var CORES = ["#0969da", "#1a7f37", "#bf8700", "#cf222e", "#8250df", "#0598bc", "#e16f24", "#6e7781", "#b35900", "#2da44e", "#a475f9", "#57606a"];
        svg.push('<svg class="phx-cb-svg" viewBox="0 0 ' + W + " " + H + '" width="100%" xmlns="http://www.w3.org/2000/svg" role="img">');
        if (tipo9 === "pizza") {
          var tot9 = 0, k9, ang0 = -Math.PI / 2, cx = H / 2 + 10, cy = H / 2, R = H / 2 - 14;
          for (k9 = 0; k9 < top9.length; k9++) tot9 += Math.max(0, top9[k9].valor);
          for (k9 = 0; k9 < top9.length; k9++) {
            var fr9 = tot9 ? Math.max(0, top9[k9].valor) / tot9 : 0, ang1 = ang0 + fr9 * Math.PI * 2;
            var x0 = cx + R * Math.cos(ang0), y0 = cy + R * Math.sin(ang0), x1 = cx + R * Math.cos(ang1), y1 = cy + R * Math.sin(ang1);
            svg.push('<path d="M' + cx + "," + cy + " L" + x0.toFixed(1) + "," + y0.toFixed(1) + " A" + R + "," + R + " 0 " + (fr9 > 0.5 ? 1 : 0) + ",1 " + x1.toFixed(1) + "," + y1.toFixed(1) + ' Z" fill="' + CORES[k9 % CORES.length] + '"><title>' + esc(top9[k9].rotulo) + ": " + esc(fmtMedida(lay.valores[m9] || {}, top9[k9].valor)) + " (" + (fr9 * 100).toFixed(1) + "%)</title></path>");
            svg.push('<rect x="' + (H + 30) + '" y="' + (14 + k9 * 20) + '" width="12" height="12" fill="' + CORES[k9 % CORES.length] + '"/><text x="' + (H + 48) + '" y="' + (25 + k9 * 20) + '" font-size="12" fill="currentColor">' + esc(top9[k9].rotulo) + " \u00b7 " + (fr9 * 100).toFixed(1) + "%</text>");
            ang0 = ang1;
          }
        } else {
          var maxV = 0, k8, bw = (W - 60) / Math.max(1, top9.length);
          for (k8 = 0; k8 < top9.length; k8++) if (Math.abs(top9[k8].valor) > maxV) maxV = Math.abs(top9[k8].valor);
          for (k8 = 0; k8 < top9.length; k8++) {
            var hB = maxV ? (Math.abs(top9[k8].valor) / maxV) * (H - 60) : 0, xB = 40 + k8 * bw;
            svg.push('<rect x="' + xB.toFixed(1) + '" y="' + (H - 30 - hB).toFixed(1) + '" width="' + (bw * 0.7).toFixed(1) + '" height="' + hB.toFixed(1) + '" fill="' + CORES[k8 % CORES.length] + '" rx="3"><title>' + esc(top9[k8].rotulo) + ": " + esc(fmtMedida(lay.valores[m9] || {}, top9[k8].valor)) + "</title></rect>");
            svg.push('<text x="' + (xB + bw * 0.35).toFixed(1) + '" y="' + (H - 14) + '" font-size="10" text-anchor="middle" fill="currentColor">' + esc(String(top9[k8].rotulo).slice(0, 12)) + "</text>");
          }
        }
        svg.push("</svg>");
        noG.innerHTML = svg.join("");
        log("grafico", { tipo: tipo9, itens: top9.length });
        return { ok: true, tipo: tipo9, itens: top9 };
      },
      dados: function () { var o9 = [], j9; for (j9 = 0; j9 < dados.length; j9++) o9.push(_origDe(dados[j9])); return o9; },
      logs: function () { return logs.slice(); },
      destruir: function () { no.innerHTML = ""; logs.length = 0; return true; }
    };
    function linhasPlanasTodas(n9, pref, out) {
      var j9, ch9;
      for (j9 = 0; j9 < n9.ordem.length; j9++) {
        ch9 = n9.filhos[n9.ordem[j9]];
        var p9 = pref.concat([n9.ordem[j9]]);
        out.push({ caminho: p9, nivel: p9.length - 1, temFilhos: ch9.ordem.length > 0 });
        linhasPlanasTodas(ch9, p9, out);
      }
      return out;
    }
    desenha();
    log("init", { registros: dados.length, campos: campos.length });
    return apiC;
  }
  /* ===== O4-9: ACT AS DROPDOWN (grid como lookup de um campo) ===== */
  function dropdown(inputEl, cfgD) {
    var inp = typeof inputEl === "string" ? document.querySelector(inputEl) : inputEl;
    if (!inp) return { ok: false, erro: "campo n\u00e3o encontrado" };
    cfgD = cfgD || {};
    var pop = null, g = null, aberto = false, campoValor = cfgD.campoValor || cfgD.chave, campoTexto = cfgD.campoTexto || campoValor;
    function fecha() { if (pop && pop.parentNode) pop.parentNode.removeChild(pop); if (g) { g.destruir(); g = null; } pop = null; aberto = false; }
    function escolhe(l9) {
      if (!l9) return;
      inp.value = l9[campoTexto] == null ? "" : String(l9[campoTexto]);
      inp.setAttribute("data-valor", l9[campoValor]);
      if (typeof cfgD.aoEscolher === "function") { try { cfgD.aoEscolher(l9[campoValor], l9); } catch (e9) {} }
      fecha();
    }
    function abre() {
      if (aberto) return;
      pop = el("div", "phx-dropdown");
      var r9 = inp.getBoundingClientRect();
      pop.style.position = "absolute";
      pop.style.left = (r9.left + (window.pageXOffset || 0)) + "px";
      pop.style.top = (r9.bottom + (window.pageYOffset || 0) + 2) + "px";
      pop.style.width = Math.max(r9.width, cfgD.largura || 480) + "px";
      document.body.appendChild(pop);
      var alvo9 = el("div", "phx-dropdown-grid");
      pop.appendChild(alvo9);
      g = criar(alvo9, { chave: cfgD.chave, colunas: cfgD.colunas, dados: cfgD.dados, fonte: cfgD.fonte, pagina: { tamanho: cfgD.tamanho || 8 }, filterRow: false, menuContexto: false, tema: cfgD.tema });
      alvo9.addEventListener("click", function (ev9) {
        var tr9 = ev9.target && ev9.target.closest ? ev9.target.closest("tbody tr") : null;
        if (!tr9) return;
        var ix9 = Array.prototype.indexOf.call(tr9.parentNode.children, tr9);
        escolhe(g.linhas()[ix9]);
      });
      aberto = true;
      if (inp.value) g.filtrar(campoTexto, inp.value ? { tipo: "texto", contem: inp.value } : null);
    }
    inp.addEventListener("focus", abre);
    inp.addEventListener("input", function () { if (!aberto) abre(); g.filtrar(campoTexto, inp.value ? { tipo: "texto", contem: inp.value } : null); });
    inp.addEventListener("keydown", function (ev9) {
      if (!aberto) { if (ev9.key === "ArrowDown" || ev9.key === "F4") abre(); return; }
      if (ev9.key === "Escape") { fecha(); return; }
      if (ev9.key === "Enter") { ev9.preventDefault(); escolhe(g.linhas()[0]); }
    });
    function foraD(ev9) { if (aberto && pop && !pop.contains(ev9.target) && ev9.target !== inp) fecha(); }
    document.addEventListener("mousedown", foraD);
    return { ok: true, abrir: abre, fechar: fecha, aberto: function () { return aberto; }, grid: function () { return g; }, escolher: escolhe,
      destruir: function () { fecha(); document.removeEventListener("mousedown", foraD); inp.removeEventListener("focus", abre); return true; } };
  }
  /* ===== SLICER / TIMELINE (segmentacao visual, como no Excel) ===== */
  function segmentacao(alvo, cfgS) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgS = cfgS || {};
    var campo = cfgS.campo;
    if (!campo) return { ok: false, erro: "cfg.campo \u00e9 obrigat\u00f3rio" };
    var alvos = cfgS.cubos || (cfgS.cubo ? [cfgS.cubo] : []);
    var grids = cfgS.grids || (cfgS.grid ? [cfgS.grid] : []);
    var tipo = cfgS.tipo === "linhaDoTempo" ? "linhaDoTempo" : "lista";
    var multi = cfgS.multi !== false;
    var sel = (cfgS.selecao || []).slice();
    var logs = [];
    function log(ev, extra) {
      var e2 = { ev: "phx.segmentacao." + ev, t: Date.now(), campo: campo }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgS.aoLog) try { cfgS.aoLog(e2); } catch (e9) {}
      return e2;
    }
    function base() {
      if (cfgS.dados) return cfgS.dados;
      if (alvos.length && alvos[0].dados) return alvos[0].dados();
      if (grids.length && grids[0].linhasTodas) return grids[0].linhasTodas();
      return [];
    }
    function periodoDe(v9) {
      var t9 = String(v9 == null ? "" : v9), m9 = t9.match(/^(\d{4})-(\d{2})/) || t9.match(/^(\d{2})\/(\d{2})\/(\d{4})/);
      if (!m9) return null;
      if (m9[3]) return m9[3] + "-" + m9[2];
      return m9[1] + "-" + m9[2];
    }
    var itens = [], contagem = {};
    function levanta() {
      var b9 = base(), vistos9 = {}, j9, v9;
      itens = []; contagem = {};
      for (j9 = 0; j9 < b9.length; j9++) {
        v9 = tipo === "linhaDoTempo" ? periodoDe(b9[j9][campo]) : (b9[j9][campo] == null ? "" : String(b9[j9][campo]));
        if (v9 == null) continue;
        if (!(v9 in vistos9)) { vistos9[v9] = 1; itens.push(v9); }
        contagem[v9] = (contagem[v9] || 0) + 1;
      }
      itens.sort(function (a, b) { var na = Number(a), nb = Number(b); if (na === na && nb === nb && a !== "" && b !== "") return na - nb; return a < b ? -1 : (a > b ? 1 : 0); });
    }
    var raiz = el("div", "phx-slicer" + (tipo === "linhaDoTempo" ? " phx-slicer-tempo" : "") + (cfgS.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = "";
    no.appendChild(raiz);
    function rotuloItem(v9) {
      if (tipo !== "linhaDoTempo") return v9 === "" ? "(vazio)" : v9;
      var p9 = String(v9).split("-");
      return p9.length === 2 ? (["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"][Number(p9[1]) - 1] + "/" + p9[0].slice(2)) : v9;
    }
    function desenha() {
      var h = [], j9, on9;
      h.push('<div class="phx-slicer-topo"><span class="phx-slicer-tit">' + esc(cfgS.titulo || campo) + "</span>" +
        '<button type="button" class="phx-slicer-limpar" title="limpar filtro"' + (sel.length ? "" : " disabled") + ">\u2717</button></div>");
      h.push('<div class="phx-slicer-itens">');
      for (j9 = 0; j9 < itens.length; j9++) {
        on9 = !sel.length || sel.indexOf(itens[j9]) >= 0;
        h.push('<button type="button" class="phx-slicer-item' + (sel.length && on9 ? " phx-slicer-on" : "") + (sel.length && !on9 ? " phx-slicer-off" : "") +
          '" data-v="' + esc(itens[j9]) + '">' + esc(rotuloItem(itens[j9])) +
          (cfgS.contagem !== false ? '<span class="phx-slicer-n">' + fmt.numero(contagem[itens[j9]] || 0, 0) + "</span>" : "") + "</button>");
      }
      h.push("</div>");
      raiz.innerHTML = h.join("");
      var bs9 = raiz.querySelectorAll(".phx-slicer-item"), q9;
      for (q9 = 0; q9 < bs9.length; q9++) (function (b9) {
        b9.addEventListener("click", function (ev9) {
          var v9 = b9.getAttribute("data-v"), ix9 = sel.indexOf(v9);
          if (!multi || (!ev9.ctrlKey && !ev9.metaKey && !ev9.shiftKey)) sel = (sel.length === 1 && ix9 === 0) ? [] : [v9];
          else if (ix9 >= 0) sel.splice(ix9, 1); else sel.push(v9);
          apiS.selecionar(sel);
        });
      })(bs9[q9]);
      var lb9 = raiz.querySelector(".phx-slicer-limpar");
      if (lb9) lb9.addEventListener("click", function () { apiS.limpar(); });
    }
    function aplica() {
      var j9, alvoCampo9 = cfgS.campoAlvo || campo;
      for (j9 = 0; j9 < alvos.length; j9++) {
        if (tipo === "linhaDoTempo" && sel.length) {
          var vals9 = [], b9 = base(), q9;
          for (q9 = 0; q9 < b9.length; q9++) { var p9 = periodoDe(b9[q9][campo]); if (p9 && sel.indexOf(p9) >= 0 && vals9.indexOf(String(b9[q9][campo])) < 0) vals9.push(String(b9[q9][campo])); }
          alvos[j9].filtrar(alvoCampo9, vals9);
        } else alvos[j9].filtrar(alvoCampo9, sel.slice());
      }
      for (j9 = 0; j9 < grids.length; j9++) {
        if (!sel.length) grids[j9].filtrar(alvoCampo9, null);
        else if (tipo === "linhaDoTempo") grids[j9].filtrar(alvoCampo9, { tipo: "faixa", de: sel[0] + "-01", ate: sel[sel.length - 1] + "-31" });
        else grids[j9].filtrar(alvoCampo9, { tipo: "valores", valores: sel.slice() });
      }
      if (typeof cfgS.aoFiltrar === "function") { try { cfgS.aoFiltrar(sel.slice(), campo); } catch (e9) {} }
    }
    var apiS = {
      ok: true,
      el: raiz,
      itens: function () { return itens.slice(); },
      selecao: function () { return sel.slice(); },
      selecionar: function (vals9) {
        sel = (vals9 || []).slice();
        desenha();
        aplica();
        log("selecionar", { n: sel.length, valores: sel.join(",") || null });
        return apiS;
      },
      limpar: function () { return apiS.selecionar([]); },
      todos: function () { return apiS.selecionar(itens.slice()); },
      conectar: function (alvo9) {
        if (!alvo9) return apiS;
        if (alvo9.filtrar && alvo9.layout && alvo9.transpor) alvos.push(alvo9); else grids.push(alvo9);
        aplica();
        return apiS;
      },
      atualizar: function (novos9) { if (novos9) cfgS.dados = novos9; levanta(); desenha(); return apiS; },
      logs: function () { return logs.slice(); },
      destruir: function () { no.innerHTML = ""; logs.length = 0; return true; }
    };
    levanta();
    desenha();
    if (sel.length) aplica();
    log("init", { itens: itens.length, tipo: tipo });
    return apiS;
  }
  function segmentacaoJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.segmentacao.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    if (cfg.cuboHandle && _instancias[cfg.cuboHandle]) cfg.cubo = _instancias[cfg.cuboHandle].api;
    if (cfg.gridHandle && _instancias[cfg.gridHandle]) cfg.grid = _instancias[cfg.gridHandle].api;
    var apiS = segmentacao(seletor, cfg);
    if (apiS && apiS.ok === false) return JSON.stringify(apiS);
    _instancias[handle] = { api: apiS, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  function cuboJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.cubo.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    var apiC = cubo(seletor, cfg);
    if (apiC && apiC.ok === false) return JSON.stringify(apiC);
    _instancias[handle] = { api: apiC, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  /* ===== O3-8: KANBAN ===== */
  function kanban(alvo, cfgK) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgK = cfgK || {};
    var dados = (cfgK.dados || []).slice();
    var campoStatus = cfgK.campoStatus || "status";
    var chaveCampo = cfgK.chave || null;
    var colsK = (cfgK.colunas || []).slice();
    var campos = cfgK.campos || [];
    var agg = cfgK.agregador || null;
    var logs = [];
    var arrastando = null;
    function log(ev, extra) {
      var e2 = { ev: "phx.kanban." + ev, t: Date.now() }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgK.aoLog) try { cfgK.aoLog(e2); } catch (e9) {}
      return e2;
    }
    if (!colsK.length) {
      var vistos = {}, j0;
      for (j0 = 0; j0 < dados.length; j0++) {
        var vv0 = String(dados[j0][campoStatus]);
        if (!(vv0 in vistos)) { vistos[vv0] = 1; colsK.push({ valor: dados[j0][campoStatus], titulo: vv0 }); }
      }
    }
    function chaveDeK(linha, ix) { return chaveCampo ? String(linha[chaveCampo]) : "#" + ix; }
    function daColuna(valor) {
      var out = [], j2;
      for (j2 = 0; j2 < dados.length; j2++) if (String(dados[j2][campoStatus]) === String(valor)) out.push(dados[j2]);
      return out;
    }
    function somaCol(linhas) {
      if (!agg) return null;
      var t = 0, j2, v2;
      for (j2 = 0; j2 < linhas.length; j2++) { v2 = Number(linhas[j2][agg.campo]); if (v2 === v2) t += v2; }
      return t;
    }
    var raiz = el("div", "phx-kb" + (cfgK.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = "";
    no.appendChild(raiz);
    function cartaoHTML(linha, ix) {
      var h = [], j2, c2, valor2, campo2, tit2;
      tit2 = cfgK.titulo ? linha[cfgK.titulo] : (chaveCampo ? linha[chaveCampo] : "");
      h.push('<div class="phx-kb-titulo">' + esc(tit2) + "</div>");
      for (j2 = 0; j2 < campos.length; j2++) {
        c2 = typeof campos[j2] === "string" ? { campo: campos[j2] } : campos[j2];
        campo2 = c2.campo;
        valor2 = linha[campo2];
        if (valor2 == null || valor2 === "") continue;
        var tp2 = c2.tipo || "texto";
        var txt2;
        if (tp2 === "moeda") txt2 = fmt.moeda(valor2);
        else if (tp2 === "numero") txt2 = fmt.numero(valor2, c2.decimais);
        else if (tp2 === "percentual") txt2 = fmt.percentual(valor2);
        else if (tp2 === "data" || tp2 === "dataHora") txt2 = fmt.data(valor2);
        else txt2 = String(valor2);
        h.push('<div class="phx-kb-campo"><span class="phx-kb-rot">' + esc(c2.titulo || campo2) + '</span><span class="phx-kb-val">' +
          esc(txt2) + "</span></div>");
      }
      return h.join("");
    }
    function desenha() {
      var html = [], j2, linhasC, soma, excede;
      for (j2 = 0; j2 < colsK.length; j2++) {
        linhasC = daColuna(colsK[j2].valor);
        soma = somaCol(linhasC);
        excede = colsK[j2].wip != null && linhasC.length > colsK[j2].wip;
        html.push('<div class="phx-kb-col' + (excede ? " phx-kb-excede" : "") + '" data-kv="' + esc(colsK[j2].valor) + '">');
        html.push('<div class="phx-kb-cab"' + (colsK[j2].cor ? ' style="border-top-color:' + esc(colsK[j2].cor) + '"' : "") + ">");
        html.push('<span class="phx-kb-nome">' + esc(colsK[j2].titulo || colsK[j2].valor) + "</span>");
        html.push('<span class="phx-kb-cont">' + linhasC.length + (colsK[j2].wip != null ? "/" + colsK[j2].wip : "") + "</span>");
        if (soma != null) html.push('<span class="phx-kb-soma">' + esc(agg.tipo === "moeda" ? fmt.moeda(soma) : fmt.numero(soma, agg.decimais)) + "</span>");
        html.push("</div>");
        html.push('<div class="phx-kb-lista" data-kv="' + esc(colsK[j2].valor) + '">');
        if (!linhasC.length) html.push('<div class="phx-kb-vazio">sem cart\u00f5es</div>');
        var k2;
        for (k2 = 0; k2 < linhasC.length; k2++) {
          html.push('<div class="phx-kb-card" draggable="true" data-kc="' + esc(chaveDeK(linhasC[k2], k2)) + '">' + cartaoHTML(linhasC[k2], k2) + "</div>");
        }
        html.push("</div></div>");
      }
      raiz.innerHTML = html.join("");
      liga();
    }
    function liga() {
      var cards = raiz.querySelectorAll(".phx-kb-card"), j2;
      for (j2 = 0; j2 < cards.length; j2++) (function (cd) {
        cd.addEventListener("dragstart", function (ev) {
          arrastando = cd.getAttribute("data-kc");
          cd.className += " phx-kb-arrastando";
          if (ev.dataTransfer && ev.dataTransfer.setData) try { ev.dataTransfer.setData("text/plain", arrastando); } catch (e9) {}
        });
        cd.addEventListener("dragend", function () { arrastando = null; desenha(); });
      })(cards[j2]);
      var listas = raiz.querySelectorAll(".phx-kb-lista"), j3;
      for (j3 = 0; j3 < listas.length; j3++) (function (ls) {
        ls.addEventListener("dragover", function (ev) { ev.preventDefault(); ls.className = "phx-kb-lista phx-kb-alvo"; });
        ls.addEventListener("dragleave", function () { ls.className = "phx-kb-lista"; });
        ls.addEventListener("drop", function (ev) {
          ev.preventDefault();
          ls.className = "phx-kb-lista";
          var ch = arrastando || (ev.dataTransfer && ev.dataTransfer.getData ? ev.dataTransfer.getData("text/plain") : null);
          arrastando = null;
          if (ch) apiK.mover(ch, ls.getAttribute("data-kv"));
        });
      })(listas[j3]);
    }
    var apiK = {
      ok: true,
      el: raiz,
      mover: function (chave9, paraValor9) {
        var j2, linha9 = null;
        for (j2 = 0; j2 < dados.length; j2++) if (chaveDeK(dados[j2], j2) === String(chave9)) { linha9 = dados[j2]; break; }
        if (!linha9) return { ok: false, erro: "cart\u00e3o n\u00e3o encontrado: " + chave9 };
        var de9 = linha9[campoStatus];
        if (String(de9) === String(paraValor9)) return { ok: true, semMudanca: true };
        var colDest9 = null;
        for (j2 = 0; j2 < colsK.length; j2++) if (String(colsK[j2].valor) === String(paraValor9)) colDest9 = colsK[j2];
        if (!colDest9) return { ok: false, erro: "coluna desconhecida: " + paraValor9 };
        if (colDest9.wip != null && cfgK.bloquearWip && daColuna(paraValor9).length >= colDest9.wip) {
          log("wip-bloqueado", { chave: String(chave9), para: String(paraValor9), wip: colDest9.wip });
          return { ok: false, erro: "limite WIP da coluna atingido" };
        }
        if (cfgK.aoMover) {
          var r9;
          try { r9 = cfgK.aoMover(String(chave9), de9, colDest9.valor, linha9); } catch (e9) { r9 = false; }
          if (r9 === false) {
            log("mover-recusado", { chave: String(chave9), de: String(de9), para: String(colDest9.valor) });
            desenha();
            return { ok: false, erro: "recusado pela aplica\u00e7\u00e3o" };
          }
        }
        linha9[campoStatus] = colDest9.valor;
        log("mover", { chave: String(chave9), de: String(de9), para: String(colDest9.valor) });
        desenha();
        return { ok: true, chave: String(chave9), de: de9, para: colDest9.valor };
      },
      atualizar: function (novos9) {
        dados = (novos9 || []).slice();
        desenha();
        log("atualizar", { linhas: dados.length });
        return apiK;
      },
      anexar: function (novos9) {
        var j2;
        for (j2 = 0; j2 < (novos9 || []).length; j2++) dados.push(novos9[j2]);
        desenha();
        log("anexar", { linhas: (novos9 || []).length });
        return apiK;
      },
      estado: function () {
        var out9 = { colunas: [], total: dados.length }, j2, lc9;
        for (j2 = 0; j2 < colsK.length; j2++) {
          lc9 = daColuna(colsK[j2].valor);
          out9.colunas.push({
            valor: colsK[j2].valor, titulo: colsK[j2].titulo || String(colsK[j2].valor),
            n: lc9.length, wip: colsK[j2].wip == null ? null : colsK[j2].wip,
            excede: colsK[j2].wip != null && lc9.length > colsK[j2].wip,
            soma: somaCol(lc9)
          });
        }
        return out9;
      },
      dados: function () { return dados.slice(); },
      logs: function () { return logs.slice(); },
      destruir: function () { no.innerHTML = ""; logs.length = 0; return true; }
    };
    desenha();
    log("init", { colunas: colsK.length, cartoes: dados.length });
    return apiK;
  }
  function kanbanJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.kanban.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    if (cfg.aoMoverWL) {
      var nomeFn = cfg.aoMoverWL;
      cfg.aoMover = function (ch, de, para, linha) {
        var fn = root[nomeFn];
        if (typeof fn !== "function") return true;
        var r = fn(JSON.stringify({ chave: ch, de: de, para: para, linha: linha }));
        return r === false || r === "false" ? false : true;
      };
    }
    var apiK = kanban(seletor, cfg);
    if (apiK && apiK.ok === false) return JSON.stringify(apiK);
    _instancias[handle] = { api: apiK, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  function vertical(alvo, cfgV) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false };
    var dados = cfgV.dados || [];
    var campos = cfgV.campos || [];
    var categorias = cfgV.categorias || [];
    var porPagina = cfgV.porPagina || Math.min(dados.length, 8);
    var tituloRegistro = cfgV.tituloRegistro || function (linha, ix) { return "Registro " + (ix + 1); };
    var logs = [];
    function log(ev, extra) {
      var e2 = { ev: "phx.vgrid." + ev, t: new Date().toISOString() }, k3;
      for (k3 in extra) e2[k3] = extra[k3];
      logs.push(e2);
      if (cfgV.logConsole) try { console.log("[phx.vgrid]", JSON.stringify(e2)); } catch (e3) {}
    }
    var porCampoV = {}, j2;
    for (j2 = 0; j2 < campos.length; j2++) porCampoV[campos[j2].campo] = campos[j2];
    /* estrutura: [soltos...] + categorias na ordem; solto = campo não citado em nenhuma categoria */
    var citados = {}, jc, jf;
    for (jc = 0; jc < categorias.length; jc++)
      for (jf = 0; jf < categorias[jc].campos.length; jf++) citados[categorias[jc].campos[jf]] = true;
    var soltos = [];
    for (j2 = 0; j2 < campos.length; j2++) if (!citados[campos[j2].campo]) soltos.push(campos[j2].campo);
    var recolhidosV = {};
    var paginaV = 1;
    function totalPaginas() { return Math.max(1, Math.ceil(dados.length / porPagina)); }
    function registrosPagina() {
      var ini = (paginaV - 1) * porPagina;
      return dados.slice(ini, ini + porPagina);
    }
    function formataV(c2, v, linha) {
      if ((c2.tipo || "texto") === "json") {
        var txt;
        try { txt = JSON.stringify(v); } catch (e4) { txt = String(v); }
        if (txt == null) txt = "";
        var curto = txt.length > 40 ? txt.slice(0, 37) + "\u2026" : txt;
        return '<span class="phx-vjson" title="' + esc(txt) + '">' + esc(curto) + "</span>";
      }
      return formata(c2, v, linha, 0);
    }
    var wrap = el("div", "phx-grid phx-vgrid");
    if (cfgV.rotulosCampo === false) wrap.className += " phx-vgrid-sem-rotulos";
    if (cfgV.espacamento) wrap.style.setProperty("--phx-vgrid-esp", cfgV.espacamento + "px");
    wrap.innerHTML =
      '<div class="phx-vgrid-topo"><span class="phx-vgrid-nav">' +
      '<button type="button" class="phx-vgrid-ant" title="anteriores">\u2039</button>' +
      '<span class="phx-vgrid-conta"></span>' +
      '<button type="button" class="phx-vgrid-prox" title="pr\u00f3ximos">\u203a</button></span></div>' +
      '<div class="phx-envoltorio"><table class="phx-tabela"><thead></thead><tbody></tbody></table></div>';
    var thead = wrap.querySelector("thead"), tbody = wrap.querySelector("tbody");
    var contaEl = wrap.querySelector(".phx-vgrid-conta");
    function montaV() {
      var regs = registrosPagina(), ini = (paginaV - 1) * porPagina;
      var h = '<tr><th class="phx-th phx-vgrid-canto"></th>';
      var j3, j4;
      for (j3 = 0; j3 < regs.length; j3++) {
        var iconeReg9 = typeof cfgV.iconeRegistro === "function" ? cfgV.iconeRegistro(regs[j3], ini + j3) : null;
        h += '<th class="phx-th phx-vgrid-reg">' + (iconeReg9 ? '<img class="phx-vgrid-icone" src="' + esc(iconeReg9) + '" alt="">' : "") + esc(tituloRegistro(regs[j3], ini + j3)) + "</th>";
      }
      thead.innerHTML = h + "</tr>";
      var html = "", contCampos9 = 0;
      function linhaCampo(cName) {
        var c2 = porCampoV[cName];
        if (!c2) return;
        if (cfgV.mostrarVazios === false) {
          var todosVazios9 = true, jv9;
          for (jv9 = 0; jv9 < regs.length; jv9++) { var vv9 = regs[jv9][c2.campo]; if (vv9 != null && vv9 !== "") { todosVazios9 = false; break; } }
          if (todosVazios9) return;
        }
        if (cfgV.maxCampos && contCampos9 >= cfgV.maxCampos) return;
        contCampos9++;
        html += '<tr class="phx-vgrid-linha"><th class="phx-th phx-vgrid-campo">' + esc(c2.titulo || c2.campo) + "</th>";
        var j5;
        for (j5 = 0; j5 < regs.length; j5++)
          html += '<td class="phx-td phx-tipo-' + (c2.tipo || "texto") + '">' + formataV(c2, regs[j5][c2.campo], regs[j5]) + "</td>";
        html += "</tr>";
      }
      for (j3 = 0; j3 < soltos.length; j3++) linhaCampo(soltos[j3]);
      if (cfgV.minCampos) while (contCampos9 < cfgV.minCampos) { html += '<tr class="phx-vgrid-linha phx-vgrid-vazia"><th class="phx-th phx-vgrid-campo">\u00a0</th>' + regs.map(function () { return '<td class="phx-td">\u00a0</td>'; }).join("") + "</tr>"; contCampos9++; }
      for (j3 = 0; j3 < categorias.length; j3++) {
        var cat = categorias[j3], aberto = !recolhidosV[cat.titulo];
        html += '<tr class="phx-vcat" data-cat="' + esc(cat.titulo) + '"><th class="phx-th phx-vcat-th" colspan="' + (regs.length + 1) + '">' +
          '<span class="phx-grupo-caret">' + (aberto ? "\u25be" : "\u25b8") + "</span>" + esc(cat.titulo) + "</th></tr>";
        if (aberto) for (j4 = 0; j4 < cat.campos.length; j4++) linhaCampo(cat.campos[j4]);
      }
      tbody.innerHTML = html;
      var tp = totalPaginas();
      contaEl.textContent = dados.length > porPagina
        ? "Registros " + fmt.numero(ini + 1) + "\u2013" + fmt.numero(ini + regs.length) + " de " + fmt.numero(dados.length)
        : fmt.numero(dados.length) + " registro" + (dados.length === 1 ? "" : "s");
      wrap.querySelector(".phx-vgrid-ant").disabled = paginaV <= 1;
      wrap.querySelector(".phx-vgrid-prox").disabled = paginaV >= tp;
      wrap.querySelector(".phx-vgrid-nav").style.display = dados.length > porPagina ? "" : "none";
    }
    tbody.addEventListener("click", function (e) {
      var tr = e.target.closest ? e.target.closest(".phx-vcat") : null;
      if (!tr) return;
      var cat = tr.getAttribute("data-cat");
      if (recolhidosV[cat]) delete recolhidosV[cat];
      else recolhidosV[cat] = true;
      log("cat", { categoria: cat, aberta: !recolhidosV[cat] });
      montaV();
    });
    wrap.querySelector(".phx-vgrid-ant").addEventListener("click", function () { apiV.irPara(paginaV - 1); });
    wrap.querySelector(".phx-vgrid-prox").addEventListener("click", function () { apiV.irPara(paginaV + 1); });
    montaV();
    no.appendChild(wrap);
    log("init", { campos: campos.length, registros: dados.length, categorias: categorias.length, porPagina: porPagina });
    var apiV = {
      ok: true,
      irPara: function (p2) {
        var tp = totalPaginas();
        if (p2 < 1) p2 = 1;
        if (p2 > tp) p2 = tp;
        if (p2 === paginaV) return apiV;
        paginaV = p2;
        montaV();
        log("page", { pagina: paginaV });
        return apiV;
      },
      pagina: function () { return paginaV; },
      proxima: function () { return apiV.irPara(paginaV + 1); },
      anterior: function () { return apiV.irPara(paginaV - 1); },
      recolherCategoria: function (titulo, recolher) {
        if (recolher === false) delete recolhidosV[titulo];
        else recolhidosV[titulo] = true;
        log("cat", { categoria: titulo, aberta: recolher === false });
        montaV();
        return apiV;
      },
      registros: function () { return registrosPagina(); },
      logs: function () { return logs.slice(); },
      destruir: function () { if (wrap.parentNode) wrap.parentNode.removeChild(wrap); }
    };
    return apiV;
  }

  /* ===== S25: LINGUAGEM NATURAL PT-BR → filtros ===== */
  function parseNumPT(txt) {
    var t = String(txt).toLowerCase().replace(/r\$\s*/g, "").replace(/\s+/g, " ");
    var m = t.match(/^(-?\d{1,3}(?:\.\d{3})+|-?\d+)(?:,(\d+))?\s*(mil|milhao|milhoes|milh\u00e3o|milh\u00f5es|k|m)?$/);
    if (!m) {
      m = t.match(/^(-?\d+)(?:[\.,](\d+))?\s*(mil|milhao|milhoes|milh\u00e3o|milh\u00f5es|k|m)?$/);
      if (!m) return null;
    }
    var inteiro = m[1].replace(/\./g, "");
    var n = Number(inteiro + (m[2] ? "." + m[2] : ""));
    if (n !== n) return null;
    var suf = m[3];
    if (suf === "mil" || suf === "k") n *= 1000;
    else if (suf) n *= 1000000;
    return n;
  }
  function pad2(n) { return (n < 10 ? "0" : "") + n; }
  function isoDe(d) { return d.getFullYear() + "-" + pad2(d.getMonth() + 1) + "-" + pad2(d.getDate()); }
  var MESES_PT = { janeiro: 0, fevereiro: 1, marco: 2, abril: 3, maio: 4, junho: 5, julho: 6, agosto: 7, setembro: 8, outubro: 9, novembro: 10, dezembro: 11 };
  function interpretar(frase, ctx) {
    ctx = ctx || {};
    var cols = ctx.colunas || [];
    var dist = ctx.distintos || {};
    var agoraD = ctx.agora ? new Date(ctx.agora + "T12:00:00") : new Date();
    var bruta = semAcento(String(frase || ""));
    var consumida = " " + bruta + " ";
    var filtrosOut = [], explic = [];
    var j2, c2;
    function consome(trecho) { consumida = consumida.replace(trecho, " "); }
    function colPorNome(nome) {
      var n2 = semAcento(nome);
      for (var j3 = 0; j3 < cols.length; j3++) {
        if (semAcento(cols[j3].campo) === n2 || semAcento(cols[j3].titulo || "") === n2) return cols[j3];
        if (n2.length > 3 && semAcento(cols[j3].titulo || "").indexOf(n2) === 0) return cols[j3];
      }
      return null;
    }
    function primeiraTipo(tipos) {
      for (var j3 = 0; j3 < cols.length; j3++) if (tipos.indexOf(cols[j3].tipo || "texto") >= 0) return cols[j3];
      return null;
    }
    var colData = primeiraTipo(["data", "dataHora"]);
    /* ---- DATAS RELATIVAS ---- */
    function fxData(de, ate, rotulo, trecho) {
      if (!colData) return;
      filtrosOut.push({ campo: colData.campo, tipo: "faixa", de: de, ate: ate, tipoCol: colData.tipo });
      explic.push((colData.titulo || colData.campo) + " " + rotulo);
      consome(trecho);
    }
    var Y = agoraD.getFullYear(), M = agoraD.getMonth(), D = agoraD.getDate();
    var m9;
    if ((m9 = consumida.match(/ hoje /))) fxData(isoDe(agoraD), isoDe(agoraD), "= hoje", m9[0]);
    if ((m9 = consumida.match(/ ontem /))) { var on = new Date(Y, M, D - 1); fxData(isoDe(on), isoDe(on), "= ontem", m9[0]); }
    if ((m9 = consumida.match(/ (este|esse|neste) mes /))) fxData(isoDe(new Date(Y, M, 1)), isoDe(new Date(Y, M + 1, 0)), "neste m\u00eas", m9[0]);
    if ((m9 = consumida.match(/ mes passado /))) fxData(isoDe(new Date(Y, M - 1, 1)), isoDe(new Date(Y, M, 0)), "no m\u00eas passado", m9[0]);
    if ((m9 = consumida.match(/ ultimos (\d+) dias /))) { var nD = +m9[1]; fxData(isoDe(new Date(Y, M, D - nD + 1)), isoDe(agoraD), "nos \u00faltimos " + nD + " dias", m9[0]); }
    if ((m9 = consumida.match(/ (?:em|de) (janeiro|fevereiro|marco|abril|maio|junho|julho|agosto|setembro|outubro|novembro|dezembro)(?: de (\d{4}))? /))) {
      var mi = MESES_PT[m9[1]], anoM = m9[2] ? +m9[2] : Y;
      fxData(isoDe(new Date(anoM, mi, 1)), isoDe(new Date(anoM, mi + 1, 0)), "em " + m9[1] + "/" + anoM, m9[0]);
    }
    if ((m9 = consumida.match(/ (?:em|de) (\d{4}) /)) && !/ (?:acima|abaixo|maior|menor|partir|entre|ate) $/.test(consumida.slice(0, m9.index + 1))) fxData(m9[1] + "-01-01", m9[1] + "-12-31", "em " + m9[1], m9[0]);
    /* ---- FAIXAS NUM\u00c9RICAS ---- */
    var NUM = "(r\\$\\s*)?(\\d[\\d\\.]*(?:,\\d+)?\\s*(?:mil|milhoes|milhao|k|m)?|\\d+)";
    function alvoNum(prefixo, ehDinheiro) {
      var cN = prefixo ? colPorNome(prefixo.replace(/\s+$/, "")) : null;
      if (cN && ["numero", "moeda", "percentual"].indexOf(cN.tipo) >= 0) return { col: cN, doPrefixo: true };
      var pad = ehDinheiro ? (primeiraTipo(["moeda"]) || primeiraTipo(["numero"])) : (primeiraTipo(["moeda"]) || primeiraTipo(["numero"]) || primeiraTipo(["percentual"]));
      return { col: pad, doPrefixo: false };
    }
    function trechoSemPrefixo(m9) { return m9[1] ? m9[0].replace(m9[1], "") : m9[0]; }
    function pushFaixa(cN, de, ate, rot, trecho) {
      if (!cN) return;
      var f9 = { campo: cN.campo, tipo: "faixa", tipoCol: cN.tipo };
      if (de != null) f9.de = de;
      if (ate != null) f9.ate = ate;
      filtrosOut.push(f9);
      explic.push((cN.titulo || cN.campo) + " " + rot);
      consome(trecho);
    }
    var reEntre = new RegExp(" (?:com )?(\\w+ )?entre " + NUM + " e " + NUM + " ");
    if ((m9 = consumida.match(reEntre))) {
      var vDe = parseNumPT((m9[3] || "") + (m9[2] ? "" : "")) != null ? parseNumPT(m9[3]) : null;
      vDe = parseNumPT(m9[3]); var vAte = parseNumPT(m9[5]);
      var alvoE = alvoNum(m9[1], !!(m9[2] || m9[4]));
      pushFaixa(alvoE.col, vDe, vAte, "entre " + fmt.numero(vDe) + " e " + fmt.numero(vAte), alvoE.doPrefixo ? m9[0] : trechoSemPrefixo(m9));
    }
    var reAcima = new RegExp(" (\\w+ )?(?:acima de|maior(?:es)? que|a partir de|mais de|>=?) " + NUM + " ");
    if ((m9 = consumida.match(reAcima))) {
      var vA = parseNumPT(m9[3]);
      var alvoA = alvoNum(m9[1], !!m9[2]);
      pushFaixa(alvoA.col, vA, null, "\u2265 " + fmt.numero(vA), alvoA.doPrefixo ? m9[0] : trechoSemPrefixo(m9));
    }
    var reAbaixo = new RegExp(" (\\w+ )?(?:abaixo de|menor(?:es)? que|no maximo|ate|<=?) " + NUM + " ");
    if ((m9 = consumida.match(reAbaixo))) {
      var vB = parseNumPT(m9[3]);
      var alvoB = alvoNum(m9[1], !!m9[2]);
      pushFaixa(alvoB.col, null, vB, "\u2264 " + fmt.numero(vB), alvoB.doPrefixo ? m9[0] : trechoSemPrefixo(m9));
    }
    /* ---- CATEG\u00d3RICOS + NEGA\u00c7\u00c3O ---- */
    function normalToken(t2) { return semAcento(t2).replace(/s$/, ""); }
    for (j2 = 0; j2 < cols.length; j2++) {
      c2 = cols[j2];
      var lista9 = dist[c2.campo];
      if (!lista9 || !lista9.length) continue;
      var achados = [], negado = false, k3, val9, nrm, re2, m2;
      for (k3 = 0; k3 < lista9.length; k3++) {
        val9 = String(lista9[k3]);
        nrm = normalToken(val9);
        if (nrm.length < 2) continue;
        re2 = new RegExp(" (nao |exceto |sem )?" + nrm.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "s? ");
        if ((m2 = consumida.match(re2))) {
          achados.push(val9);
          if (m2[1]) negado = true;
          consome(m2[0].replace(/^ (nao |exceto |sem )/, " "));
          if (m2[1]) consumida = consumida.replace(m2[1], " ");
        }
      }
      if (achados.length) {
        var valoresF = achados;
        var rot9;
        if (negado) {
          valoresF = [];
          for (k3 = 0; k3 < lista9.length; k3++) if (achados.indexOf(String(lista9[k3])) < 0) valoresF.push(String(lista9[k3]));
          rot9 = (c2.titulo || c2.campo) + " \u2260 " + achados.join(", ");
        } else rot9 = (c2.titulo || c2.campo) + " = " + achados.join(" ou ");
        filtrosOut.push({ campo: c2.campo, tipo: "valores", valores: valoresF });
        explic.push(rot9);
      }
    }
    /* ---- SOBRA \u2192 BUSCA ---- */
    var STOP = { de: 1, do: 1, da: 1, dos: 1, das: 1, e: 1, em: 1, com: 1, o: 1, a: 1, os: 1, as: 1, no: 1, na: 1, nos: 1, nas: 1, que: 1, para: 1, por: 1, um: 1, uma: 1, mostrar: 1, mostre: 1, listar: 1, liste: 1, quais: 1, todos: 1, toda: 1, todas: 1, pedidos: 1, pedido: 1, linhas: 1, registros: 1 };
    var sobras = consumida.split(/\s+/), termos = [], j4;
    for (j4 = 0; j4 < sobras.length; j4++) {
      var t4 = sobras[j4];
      if (t4 && !STOP[t4] && !/^\d/.test(t4) && t4 !== "r$") termos.push(t4);
    }
    var busca9 = termos.join(" ") || null;
    if (busca9) explic.push('busca: "' + busca9 + '"');
    return { filtros: filtrosOut, busca: busca9, explicacao: explic };
  }

  /* ===== C2: COMANDOS NL DE A\u00c7\u00c3O ===== */
  function _colPorNomeCmd(cols, nome) {
    var n2 = semAcento(String(nome || "")).replace(/\s+$/, "").replace(/s$/, "");
    if (!n2) return null;
    var j3, tN;
    for (j3 = 0; j3 < cols.length; j3++) {
      if (semAcento(cols[j3].campo).replace(/s$/, "") === n2) return cols[j3];
      tN = semAcento(cols[j3].titulo || "").replace(/s$/, "");
      if (tN === n2) return cols[j3];
    }
    for (j3 = 0; j3 < cols.length; j3++) {
      tN = semAcento(cols[j3].titulo || "");
      if (n2.length > 3 && tN.indexOf(n2) === 0) return cols[j3];
    }
    return null;
  }
  function interpretarComando(frase, ctx) {
    ctx = ctx || {};
    var cols = ctx.colunas || [];
    var t = " " + semAcento(String(frase || "")) + " ";
    var acoes = [];
    var m9;
    function add(metodo, args, rotulo, trecho) {
      acoes.push({ metodo: metodo, args: args, rotulo: rotulo });
      t = t.replace(trecho, " ");
    }
    /* limpar tudo / filtros */
    if ((m9 = t.match(/ limpa(?:r)? tudo /))) { add("limparFiltros", [], "limpar tudo", m9[0]); add("agrupar", [[]], "", " "); }
    else if ((m9 = t.match(/ limpa(?:r)? (?:os )?filtros /))) add("limparFiltros", [], "limpar filtros", m9[0]);
    /* agrupar */
    if ((m9 = t.match(/ (?:desagrupa(?:r)?|remove(?:r)? grupos|sem grupos) /))) add("agrupar", [[]], "desagrupar", m9[0]);
    else if ((m9 = t.match(/ agrupa(?:r|do)? por ([a-z0-9_]+(?: e [a-z0-9_]+)*) /))) {
      var partes9 = m9[1].split(" e "), campos9 = [], j9;
      for (j9 = 0; j9 < partes9.length; j9++) {
        var c9 = _colPorNomeCmd(cols, partes9[j9]);
        if (c9) campos9.push(c9.campo);
      }
      if (campos9.length) add("agrupar", [campos9], "agrupar por " + campos9.join(" e "), m9[0]);
    }
    /* ordenar */
    if ((m9 = t.match(/ orden(?:a|e|ar) por ([a-z0-9_ ]+?) ?(desc|decrescente|maior primeiro|asc|crescente)? (?=$|e |escond|mostr|tema|pagin|export|agrup|limpa)/)) || (m9 = t.match(/ orden(?:a|e|ar) por ([a-z0-9_]+) ?(desc|decrescente|maior primeiro|asc|crescente)? /))) {
      var cO = _colPorNomeCmd(cols, m9[1]);
      if (cO) {
        var dir9 = (m9[2] === "desc" || m9[2] === "decrescente" || m9[2] === "maior primeiro") ? "desc" : "asc";
        add("ordenar", [cO.campo, dir9], "ordenar por " + cO.campo + " " + dir9, m9[0]);
      }
    }
    /* esconder / mostrar coluna */
    while ((m9 = t.match(/ esconde(?:r)? (?:a coluna )?([a-z0-9_]+) /))) {
      var cE = _colPorNomeCmd(cols, m9[1]);
      if (!cE) { t = t.replace(m9[0], " \u0000esc "); continue; }
      add("ocultar", [cE.campo, true], "esconder " + cE.campo, m9[0]);
    }
    while ((m9 = t.match(/ mostra(?:r)? (?:a coluna )?([a-z0-9_]+) /))) {
      var cM = _colPorNomeCmd(cols, m9[1]);
      if (!cM) { t = t.replace(m9[0], " \u0000mos " + m9[1] + " "); continue; }
      add("ocultar", [cM.campo, false], "mostrar " + cM.campo, m9[0]);
    }
    t = t.replace(/\u0000esc /g, " ").replace(/\u0000mos /g, " ");
    /* exportar */
    if ((m9 = t.match(/ (?:exporta(?:r)?|baixa(?:r)?)(?: para)?(?: o)? (?:csv|excel|planilha) /))) add("baixarCSV", [], "exportar CSV", m9[0]);
    /* tema */
    if ((m9 = t.match(/ (?:tema|modo) escuro | dark /))) add("setTema", ["escuro"], "tema escuro", m9[0]);
    if ((m9 = t.match(/ (?:tema|modo) claro /))) add("setTema", ["claro"], "tema claro", m9[0]);
    if ((m9 = t.match(/ tema automatico /))) add("setTema", ["auto"], "tema autom\u00e1tico", m9[0]);
    /* densidade */
    if ((m9 = t.match(/ compact[oa] /))) add("setDensidade", ["compacta"], "densidade compacta", m9[0]);
    if ((m9 = t.match(/ confortavel /))) add("setDensidade", ["confortavel"], "densidade confort\u00e1vel", m9[0]);
    if ((m9 = t.match(/ espacos[oa] /))) add("setDensidade", ["espacosa"], "densidade espa\u00e7osa", m9[0]);
    /* p\u00e1gina */
    if ((m9 = t.match(/ pagina (\d+) /))) add("pagina", [+m9[1]], "p\u00e1gina " + m9[1], m9[0]);
    else if ((m9 = t.match(/ proxima(?: pagina)? /))) add("_pagRel", [1], "pr\u00f3xima p\u00e1gina", m9[0]);
    else if ((m9 = t.match(/ (?:pagina )?anterior /))) add("_pagRel", [-1], "p\u00e1gina anterior", m9[0]);
    else if ((m9 = t.match(/ primeira pagina /))) add("pagina", [1], "primeira p\u00e1gina", m9[0]);
    else if ((m9 = t.match(/ ultima pagina /))) add("_pagUltima", [], "\u00faltima p\u00e1gina", m9[0]);
    /* resumo */
    if ((m9 = t.match(/ resum(?:e|o|ir|a) /))) add("abrirResumo", [], "resumo", m9[0]);
    var resto = t.replace(/\s+/g, " ").replace(/^ | $/g, "");
    if (resto === "e" || resto === "e e") resto = "";
    return { acoes: acoes, resto: resto };
  }

  /* ===== S26: RESUMO AUTOM\u00c1TICO (top-N, anomalias, tend\u00eancia) ===== */
  function selecionaK(arr, k) {
    /* quickselect in-place: k-\u00e9simo menor (0-based), O(n) m\u00e9dio */
    var lo = 0, hi = arr.length - 1;
    while (lo < hi) {
      var p = arr[lo + ((hi - lo) >> 1)], i2 = lo, j2 = hi;
      while (i2 <= j2) {
        while (arr[i2] < p) i2++;
        while (arr[j2] > p) j2--;
        if (i2 <= j2) { var t2 = arr[i2]; arr[i2] = arr[j2]; arr[j2] = t2; i2++; j2--; }
      }
      if (k <= j2) hi = j2;
      else if (k >= i2) lo = i2;
      else return arr[k];
    }
    return arr[k];
  }
  function medianaDe(vs) {
    var n2 = vs.length;
    if (!n2) return 0;
    if (n2 % 2) return selecionaK(vs, (n2 - 1) / 2);
    var a2 = selecionaK(vs, n2 / 2);
    /* ap\u00f3s select, o maior \u00e0 esquerda de n/2 \u00e9 o (n/2-1)-\u00e9simo */
    var mx = -Infinity, j3;
    for (j3 = 0; j3 < n2 / 2; j3++) if (vs[j3] > mx) mx = vs[j3];
    return (a2 + mx) / 2;
  }
  function resumir(linhas, colunas, opts) {
    opts = opts || {};
    var topN = opts.topN || 3, zLim = opts.zLimiar || 2.5;
    var out = { total: linhas.length, metricas: {}, tops: [], anomalias: [], concentracao: null, tendencia: null, frases: [] };
    if (!linhas.length) { out.frases.push("Nenhuma linha no recorte atual."); return out; }
    var j2, k2, c2, v2;
    var numericas = [], categoricas = [], colData = null;
    for (j2 = 0; j2 < colunas.length; j2++) {
      c2 = colunas[j2];
      var t2 = c2.tipo || "texto";
      if (t2 === "numero" || t2 === "moeda" || t2 === "percentual") numericas.push(c2);
      else if (t2 === "data" || t2 === "dataHora") { if (!colData) colData = c2; }
      else categoricas.push(c2);
    }
    var principal = null;
    for (j2 = 0; j2 < numericas.length; j2++) if (numericas[j2].tipo === "moeda") { principal = numericas[j2]; break; }
    if (!principal && numericas.length) principal = numericas[0];
    /* ===== SINGLE-PASS: m\u00e9tricas + contagens + per\u00edodos em 1 varrida ===== */
    var acN = [], j3;
    for (j2 = 0; j2 < numericas.length; j2++) acN.push({ c: numericas[j2], vs: [], soma: 0, soma2: 0, mn: Infinity, mx: -Infinity });
    var acC = [];
    for (j2 = 0; j2 < categoricas.length; j2++) acC.push({ c: categoricas[j2], cont: {}, somaPor: {}, ordem: [] });
    var porPer = {}, pers = [];
    var campoPrin = principal ? principal.campo : null;
    for (k2 = 0; k2 < linhas.length; k2++) {
      var L9 = linhas[k2];
      var vPrin = campoPrin ? Number(L9[campoPrin]) : NaN;
      for (j3 = 0; j3 < acN.length; j3++) {
        v2 = Number(L9[acN[j3].c.campo]);
        if (v2 !== v2) continue;
        var a3 = acN[j3];
        a3.vs.push(v2); a3.soma += v2; a3.soma2 += v2 * v2;
        if (v2 < a3.mn) a3.mn = v2;
        if (v2 > a3.mx) a3.mx = v2;
      }
      for (j3 = 0; j3 < acC.length; j3++) {
        v2 = L9[acC[j3].c.campo];
        if (v2 == null || v2 === "") continue;
        var b3 = acC[j3], kk = String(v2);
        if (b3.cont[kk] == null) { b3.cont[kk] = 0; b3.somaPor[kk] = 0; b3.ordem.push(kk); }
        b3.cont[kk]++;
        if (vPrin === vPrin) b3.somaPor[kk] += vPrin;
      }
      if (colData) {
        v2 = String(L9[colData.campo] || "");
        if (v2.length >= 7) {
          var per = v2.slice(0, 7);
          if (!porPer[per]) { porPer[per] = { periodo: per, n: 0, soma: 0 }; pers.push(per); }
          porPer[per].n++;
          if (vPrin === vPrin) porPer[per].soma += vPrin;
        }
      }
    }
    for (j3 = 0; j3 < acN.length; j3++) {
      var aN = acN[j3];
      if (!aN.vs.length) continue;
      var n2 = aN.vs.length, avg = aN.soma / n2;
      var varc = aN.soma2 / n2 - avg * avg;
      out.metricas[aN.c.campo] = { titulo: aN.c.titulo || aN.c.campo, tipo: aN.c.tipo, n: n2, soma: aN.soma, media: avg,
        min: aN.mn, max: aN.mx, mediana: medianaDe(aN.vs), desvio: varc > 0 ? Math.sqrt(varc) : 0 };
    }
    for (j3 = 0; j3 < acC.length; j3++) {
      var bC = acC[j3];
      c2 = bC.c;
      if (!bC.ordem.length || bC.ordem.length > 20) continue;
      var cont = bC.cont, somaPor = bC.somaPor;
      var porCont = bC.ordem.slice().sort(function (a, b) { return cont[b] - cont[a]; }).slice(0, topN)
        .map(function (kx) { return { valor: kx, n: cont[kx], pct: Math.round(cont[kx] / linhas.length * 1000) / 10 }; });
      var porSoma = principal ? bC.ordem.slice().sort(function (a, b) { return somaPor[b] - somaPor[a]; }).slice(0, topN)
        .map(function (kx) { return { valor: kx, soma: somaPor[kx] }; }) : null;
      out.tops.push({ campo: c2.campo, titulo: c2.titulo || c2.campo, porContagem: porCont, porSoma: porSoma, distintos: bC.ordem.length });
      if (principal && porSoma && !out.concentracao) {
        var totP = out.metricas[principal.campo] ? out.metricas[principal.campo].soma : 0;
        if (totP > 0) {
          var acum = 0;
          for (k2 = 0; k2 < porSoma.length; k2++) acum += porSoma[k2].soma;
          var pctC = Math.round(acum / totP * 1000) / 10;
          if (pctC >= 70) out.concentracao = { campo: c2.titulo || c2.campo, topN: porSoma.length, pct: pctC };
        }
      }
    }
    /* anomalias por z-score na principal */
    if (principal && out.metricas[principal.campo] && out.metricas[principal.campo].desvio > 0) {
      var mP = out.metricas[principal.campo], anas = [];
      for (k2 = 0; k2 < linhas.length; k2++) {
        v2 = Number(linhas[k2][principal.campo]);
        if (v2 !== v2) continue;
        var z2 = (v2 - mP.media) / mP.desvio;
        if (z2 > zLim || z2 < -zLim) anas.push({ linha: linhas[k2], valor: v2, z: Math.round(z2 * 100) / 100 });
      }
      anas.sort(function (a, b) { return Math.abs(b.z) - Math.abs(a.z); });
      out.anomalias = anas.slice(0, 5);
      out.anomaliasTotal = anas.length;
    }
    /* tend\u00eancia temporal */
    if (colData && principal) {
      pers.sort();
      if (pers.length >= 3) {
        var serie = [], somaY = 0;
        for (k2 = 0; k2 < pers.length; k2++) { serie.push(porPer[pers[k2]]); somaY += porPer[pers[k2]].soma; }
        var nS = serie.length, mY = somaY / nS, mX = (nS - 1) / 2;
        var sXY = 0, sXX = 0;
        for (k2 = 0; k2 < nS; k2++) { sXY += (k2 - mX) * (serie[k2].soma - mY); sXX += (k2 - mX) * (k2 - mX); }
        var slope = sXX > 0 ? sXY / sXX : 0;
        var pctT = mY > 0 ? Math.round(slope * (nS - 1) / mY * 1000) / 10 : 0;
        out.tendencia = { serie: serie, campo: principal.titulo || principal.campo,
          direcao: pctT > 5 ? "alta" : pctT < -5 ? "queda" : "estavel", pct: pctT };
      }
    }
    /* narrativa local */
    var fr = out.frases;
    fr.push(fmt.numero(out.total) + (opts.totalGeral && opts.totalGeral > out.total ? " linhas (" + (Math.round(out.total / opts.totalGeral * 1000) / 10) + "% do total)" : " linhas no recorte"));
    if (principal && out.metricas[principal.campo]) {
      var mF = out.metricas[principal.campo];
      var fV = principal.tipo === "moeda" ? fmt.moeda : fmt.numero;
      fr.push((principal.titulo || principal.campo) + ": soma " + fV(mF.soma) + " \u00b7 m\u00e9dia " + fV(Math.round(mF.media * 100) / 100) + " \u00b7 mediana " + fV(mF.mediana));
    }
    for (j2 = 0; j2 < out.tops.length && j2 < 2; j2++) {
      var tp = out.tops[j2];
      if (tp.porContagem.length) fr.push(tp.titulo + ": " + tp.porContagem[0].valor + " lidera com " + tp.porContagem[0].pct + "%");
    }
    if (out.concentracao) fr.push("Concentra\u00e7\u00e3o: top " + out.concentracao.topN + " de " + out.concentracao.campo + " = " + out.concentracao.pct + "% do valor");
    if (out.anomaliasTotal) fr.push(out.anomaliasTotal + " anomalia" + (out.anomaliasTotal > 1 ? "s" : "") + " (>" + zLim + "\u03c3) \u2014 maior: " + (principal.tipo === "moeda" ? fmt.moeda(out.anomalias[0].valor) : fmt.numero(out.anomalias[0].valor)) + " (z=" + out.anomalias[0].z + ")");
    if (out.tendencia) fr.push("Tend\u00eancia de " + (out.tendencia.direcao === "estavel" ? "estabilidade" : out.tendencia.direcao + " de " + Math.abs(out.tendencia.pct) + "%") + " em " + out.tendencia.serie.length + " per\u00edodos");
    return out;
  }

  /* ===== T2: CONFIGURA\u00c7\u00c3O VISUAL TOTAL ===== */
  var MAPA_VISUAL = {
    /* geral */
    "acento": "--phx-acc", "acentoHover": "--phx-acc-hover", "fundo": "--phx-bg", "fundoAlt": "--phx-bg2",
    "texto": "--phx-fg", "textoSuave": "--phx-meta", "borda": "--phx-borda", "linha": "--phx-linha",
    "ok": "--phx-ok", "perigo": "--phx-perigo", "aviso": "--phx-aviso", "foco": "--phx-foco",
    "fonte": "--phx-fonte", "fonteMono": "--phx-mono", "tamanho": "--phx-fs",
    "raio": "--phx-raio", "raioControle": "--phx-raio-ctl", "espessuraBorda": "--phx-esp-borda",
    "espacamentoX": "--phx-pad-x", "espacamentoY": "--phx-pad-y",
    /* cabecalho */
    "cabecalho.fundo": "--phx-header-bg", "cabecalho.texto": "--phx-header-fg", "cabecalho.tamanho": "--phx-cab-fs",
    "cabecalho.peso": "--phx-cab-peso", "cabecalho.estilo": "--phx-cab-estilo", "cabecalho.caixa": "--phx-cab-caixa",
    "cabecalho.altura": "--phx-cab-pad-y",
    /* corpo */
    "corpo.peso": "--phx-cel-peso", "corpo.estilo": "--phx-cel-estilo", "corpo.zebra": "--phx-zebra",
    "corpo.hover": "--phx-hover", "corpo.selecao": "--phx-sel-bg", "corpo.edicao": "--phx-edit-bg",
    "corpo.texto": "--phx-cel-fg", "corpo.fundo": "--phx-cel-bg", "corpo.negativo": "--phx-neg",
    /* grade */
    "grade.espessuraLinha": "--phx-esp-linha", "grade.espessuraColuna": "--phx-esp-col",
    "grade.corLinha": "--phx-linha", "grade.corColuna": "--phx-linha-col", "grade.estiloLinha": "--phx-estilo-linha",
    /* barra de agrupamento (arrastar e soltar) */
    "agrupamento.fundo": "--phx-gbar-bg", "agrupamento.texto": "--phx-gbar-fg", "agrupamento.borda": "--phx-gbar-bd",
    "agrupamento.tamanho": "--phx-gbar-fs", "agrupamento.estilo": "--phx-gbar-estilo", "agrupamento.altura": "--phx-gbar-alt",
    "agrupamento.tracejado": "--phx-gbar-tracejado",
    /* linhas de grupo */
    "grupos.fundo": "--phx-grupo-bg", "grupos.texto": "--phx-grupo-fg", "grupos.peso": "--phx-grupo-peso",
    "grupos.tamanho": "--phx-grupo-fs", "grupos.hover": "--phx-grupo-hover",
    /* totais */
    "totais.fundo": "--phx-tot-bg", "totais.texto": "--phx-tot-fg", "totais.peso": "--phx-tot-peso", "totais.tamanho": "--phx-tot-fs",
    /* botoes e paginacao */
    "botoes.fundo": "--phx-btn-bg", "botoes.texto": "--phx-btn-fg", "botoes.borda": "--phx-btn-bd", "botoes.tamanho": "--phx-pg-fs",
    /* chips de filtro */
    "chips.fundo": "--phx-chip-bg", "chips.borda": "--phx-chip-borda"
  };
  function visualParaTokens(conf) {
    var out = {}, k, v, chave, alvo;
    function anda(pref, obj) {
      for (k in obj) {
        if (!Object.prototype.hasOwnProperty.call(obj, k)) continue;
        v = obj[k];
        chave = pref ? pref + "." + k : k;
        if (v && typeof v === "object" && !(v instanceof Array)) { anda(chave, v); continue; }
        if (k === "direcao" && pref === "grade") {
          /* GridLines: Both/Horizontal/Vertical/None -- seta os DOIS tokens de espessura de uma vez */
          var espBase9 = (obj.espessuraLinha != null ? obj.espessuraLinha : (obj.espessuraColuna != null ? obj.espessuraColuna : 1));
          if (typeof espBase9 === "number") espBase9 = espBase9 + "px";
          if (v === "horizontal") { out["--phx-esp-linha"] = String(espBase9); out["--phx-esp-col"] = "0px"; }
          else if (v === "vertical") { out["--phx-esp-linha"] = "0px"; out["--phx-esp-col"] = String(espBase9); }
          else if (v === "nenhuma") { out["--phx-esp-linha"] = "0px"; out["--phx-esp-col"] = "0px"; }
          else { out["--phx-esp-linha"] = String(espBase9); out["--phx-esp-col"] = String(espBase9); }
          continue;
        }
        alvo = MAPA_VISUAL[chave];
        if (!alvo) { out["!" + chave] = "desconhecido"; continue; }
        /* atalhos humanos */
        if (k === "negrito") { alvo = MAPA_VISUAL[pref + ".peso"]; v = v ? "700" : "400"; }
        else if (k === "italico") { alvo = MAPA_VISUAL[pref + ".estilo"]; v = v ? "italic" : "normal"; }
        else if (k === "maiusculas") { alvo = MAPA_VISUAL[pref + ".caixa"]; v = v ? "uppercase" : "none"; }
        else if (k === "zebra" && (v === false || v === true)) v = v ? "" : "transparent";
        if (!alvo) continue;
        if (typeof v === "number") {
          if (alvo.indexOf("peso") >= 0) v = String(v);
          else v = v + "px";
        }
        if (v === "" ) continue;
        out[alvo] = String(v);
      }
    }
    /* atalhos booleanos que dependem do prefixo precisam existir no mapa */
    MAPA_VISUAL["cabecalho.negrito"] = "--phx-cab-peso";
    MAPA_VISUAL["cabecalho.italico"] = "--phx-cab-estilo";
    MAPA_VISUAL["cabecalho.maiusculas"] = "--phx-cab-caixa";
    MAPA_VISUAL["corpo.negrito"] = "--phx-cel-peso";
    MAPA_VISUAL["corpo.italico"] = "--phx-cel-estilo";
    MAPA_VISUAL["grupos.negrito"] = "--phx-grupo-peso";
    MAPA_VISUAL["totais.negrito"] = "--phx-tot-peso";
    MAPA_VISUAL["agrupamento.italico"] = "--phx-gbar-estilo";
    anda("", conf || {});
    return out;
  }

  /* ===== T1: TEMA TOTAL ===== */
  function hex2rgb(h) {
    h = String(h || "").replace("#", "");
    if (h.length === 3) h = h.charAt(0) + h.charAt(0) + h.charAt(1) + h.charAt(1) + h.charAt(2) + h.charAt(2);
    var n = parseInt(h, 16);
    if (n !== n || h.length !== 6) return null;
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }
  function rgb2hex(c) {
    function p2(x) { x = Math.max(0, Math.min(255, Math.round(x))).toString(16); return x.length < 2 ? "0" + x : x; }
    return "#" + p2(c.r) + p2(c.g) + p2(c.b);
  }
  function misturaCor(a, b, t) {
    var ca = hex2rgb(a), cb = hex2rgb(b);
    if (!ca || !cb) return a;
    return rgb2hex({ r: ca.r + (cb.r - ca.r) * t, g: ca.g + (cb.g - ca.g) * t, b: ca.b + (cb.b - ca.b) * t });
  }
  function luminancia(h) {
    var c = hex2rgb(h);
    if (!c) return 0;
    return (0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b) / 255;
  }
  function temaDe(corBase, opts) {
    opts = opts || {};
    var escuro = opts.modo === "escuro";
    var c = hex2rgb(corBase);
    if (!c) return { erro: "cor inv\u00e1lida: " + corBase };
    var bg = escuro ? "#0d1117" : "#ffffff";
    var t = {};
    t["--phx-acc"] = corBase;
    t["--phx-acc-hover"] = misturaCor(corBase, escuro ? "#ffffff" : "#000000", 0.14);
    t["--phx-header-bg"] = misturaCor(corBase, bg, escuro ? 0.82 : 0.92);
    t["--phx-header-fg"] = escuro ? misturaCor(corBase, "#ffffff", 0.55) : misturaCor(corBase, "#000000", 0.35);
    t["--phx-chip-bg"] = misturaCor(corBase, bg, escuro ? 0.75 : 0.85);
    t["--phx-chip-borda"] = misturaCor(corBase, bg, escuro ? 0.5 : 0.6);
    t["--phx-hover"] = misturaCor(corBase, bg, escuro ? 0.88 : 0.95);
    t["--phx-zebra"] = misturaCor(corBase, bg, escuro ? 0.93 : 0.975);
    t["--phx-foco"] = corBase;
    t["--phx-inv"] = luminancia(corBase) > 0.55 ? "#1f2328" : "#ffffff";
    return t;
  }
  var _temasRegistrados = {
    corporativo: { base: "claro", tokens: temaDe("#0b5cad", {}) },
    terra: { base: "claro", tokens: temaDe("#8a5a2b", {}) },
    "alto-contraste": { base: "claro", tokens: { "--phx-acc": "#000000", "--phx-acc-hover": "#333333", "--phx-fg": "#000000", "--phx-borda": "#000000", "--phx-esp-borda": "2px", "--phx-header-bg": "#000000", "--phx-header-fg": "#ffffff", "--phx-inv": "#ffffff", "--phx-hover": "#ffe100", "--phx-zebra": "#f2f2f2", "--phx-foco": "#000000" } }
  };
  function registrarTema(nome, def) {
    if (!nome || !def || !def.tokens) return false;
    _temasRegistrados[String(nome)] = { base: def.base === "escuro" ? "escuro" : "claro", tokens: def.tokens };
    return true;
  }

  /* ===== C8: LINHA DO TEMPO (MoM) ===== */
  var MESES_ROT = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
  function linhaDoTempo(linhas, colunas, opts) {
    opts = opts || {};
    var campoData = opts.campoData || null, campoValor = opts.campoValor || null, j2, c2;
    for (j2 = 0; j2 < colunas.length && !campoData; j2++) {
      c2 = colunas[j2];
      if (c2.tipo === "data" || c2.tipo === "dataHora") campoData = c2.campo;
    }
    if (!campoData) return { ok: false, erro: "sem coluna de data" };
    var pref = ["moeda", "numero", "percentual"], p2;
    for (p2 = 0; p2 < pref.length && !campoValor; p2++) {
      for (j2 = 0; j2 < colunas.length && !campoValor; j2++) {
        if (colunas[j2].tipo === pref[p2] && colunas[j2].campo !== campoData) campoValor = colunas[j2].campo;
      }
    }
    var tituloV = campoValor, tipoV = "numero";
    for (j2 = 0; j2 < colunas.length; j2++) if (colunas[j2].campo === campoValor) { tituloV = colunas[j2].titulo || campoValor; tipoV = colunas[j2].tipo; }
    var acc = {}, ordem2 = [];
    for (j2 = 0; j2 < linhas.length; j2++) {
      var vd = linhas[j2][campoData];
      if (vd == null || String(vd).length < 7) continue;
      var per = String(vd).slice(0, 7);
      if (!(per in acc)) { acc[per] = { periodo: per, n: 0, soma: 0 }; ordem2.push(per); }
      acc[per].n++;
      if (campoValor != null) {
        var vv = Number(linhas[j2][campoValor]);
        if (vv === vv) acc[per].soma += vv;
      }
    }
    ordem2.sort();
    if (!ordem2.length) return { ok: false, erro: "sem datas v\u00e1lidas no recorte" };
    var serie = [], ant = null;
    function dpc(a, b) { return a !== 0 ? Math.round((b / a - 1) * 1000) / 10 : (b !== 0 ? 100 : 0); }
    for (j2 = 0; j2 < ordem2.length; j2++) {
      var e2 = acc[ordem2[j2]];
      var mes2 = parseInt(e2.periodo.slice(5, 7), 10);
      var item = { periodo: e2.periodo, rotulo: MESES_ROT[mes2 - 1] + "/" + e2.periodo.slice(2, 4), n: e2.n,
        soma: Math.round(e2.soma * 100) / 100, media: e2.n ? Math.round(e2.soma / e2.n * 100) / 100 : 0,
        deltaN: null, deltaNPct: null, deltaSoma: null, deltaSomaPct: null };
      if (ant) {
        item.deltaN = e2.n - ant.n;
        item.deltaNPct = dpc(ant.n, e2.n);
        item.deltaSoma = Math.round((e2.soma - ant.soma) * 100) / 100;
        item.deltaSomaPct = dpc(ant.soma, e2.soma);
      }
      serie.push(item);
      ant = e2;
    }
    var melhor = serie[0], pior = serie[0];
    for (j2 = 1; j2 < serie.length; j2++) {
      if (serie[j2].soma > melhor.soma) melhor = serie[j2];
      if (serie[j2].soma < pior.soma) pior = serie[j2];
    }
    var streak = { dir: null, len: 0 };
    for (j2 = serie.length - 1; j2 >= 1; j2--) {
      var d2 = serie[j2].deltaSoma;
      if (d2 == null || d2 === 0) break;
      var dir2 = d2 > 0 ? "alta" : "queda";
      if (!streak.dir) { streak.dir = dir2; streak.len = 1; }
      else if (streak.dir === dir2) streak.len++;
      else break;
    }
    var fV = tipoV === "moeda" ? fmt.moeda : fmt.numero;
    var frases = [];
    frases.push(serie.length + " per\u00edodo" + (serie.length > 1 ? "s" : "") + " no recorte \u00b7 " + tituloV + " somado: " + fV(Math.round(melhor.soma + 0) ? Math.round(serie.reduce ? 0 : 0) : 0));
    frases[0] = serie.length + " per\u00edodo" + (serie.length > 1 ? "s" : "") + " \u00b7 melhor: " + melhor.rotulo + " (" + fV(Math.round(melhor.soma)) + ") \u00b7 pior: " + pior.rotulo + " (" + fV(Math.round(pior.soma)) + ")";
    if (streak.len >= 2) frases.push("H\u00e1 " + streak.len + " meses em " + streak.dir + (streak.dir === "alta" ? " \uD83D\uDCC8" : " \uD83D\uDCC9"));
    var ult = serie[serie.length - 1];
    if (ult.deltaSomaPct != null) frases.push(ult.rotulo + " vs anterior: " + (ult.deltaSomaPct >= 0 ? "+" : "") + ult.deltaSomaPct + "% em " + tituloV.toLowerCase() + ", " + (ult.deltaNPct >= 0 ? "+" : "") + ult.deltaNPct + "% em linhas");
    return { ok: true, campoData: campoData, campoValor: campoValor, tituloValor: tituloV, tipoValor: tipoV, serie: serie, melhor: melhor, pior: pior, streak: streak, frases: frases };
  }

  /* ===== C7: FOTOGRAFIA + EVOLU\u00c7\u00c3O ===== */
  function fotografa(r9) {
    var foto = { total: r9.total, metricas: {}, lideres: [], ts: Date.now() };
    var kM;
    for (kM in r9.metricas) {
      var mM = r9.metricas[kM];
      foto.metricas[kM] = { titulo: mM.titulo, tipo: mM.tipo, soma: mM.soma, media: mM.media, mediana: mM.mediana };
    }
    var j2;
    for (j2 = 0; j2 < r9.tops.length; j2++) {
      if (r9.tops[j2].porContagem.length) foto.lideres.push({ campo: r9.tops[j2].campo, titulo: r9.tops[j2].titulo, valor: r9.tops[j2].porContagem[0].valor, pct: r9.tops[j2].porContagem[0].pct });
    }
    return foto;
  }
  function evolucaoDe(foto, atualR) {
    var agoraF = fotografa(atualR);
    function dpc(a, b) { return a !== 0 ? Math.round((b / a - 1) * 1000) / 10 : (b !== 0 ? 100 : 0); }
    var out = { antes: foto, agora: agoraF, dias: Math.max(0, Math.round((Date.now() - foto.ts) / 86400000)),
      contagem: { antes: foto.total, agora: agoraF.total, delta: agoraF.total - foto.total, pct: dpc(foto.total, agoraF.total) },
      metricas: {}, lideres: [], frases: [] };
    var kM;
    for (kM in foto.metricas) {
      if (!agoraF.metricas[kM]) continue;
      var a2 = foto.metricas[kM], b2 = agoraF.metricas[kM];
      out.metricas[kM] = { titulo: a2.titulo, tipo: a2.tipo,
        soma: { antes: a2.soma, agora: b2.soma, delta: b2.soma - a2.soma, pct: dpc(a2.soma, b2.soma) },
        media: { antes: a2.media, agora: b2.media, delta: b2.media - a2.media, pct: dpc(a2.media, b2.media) } };
    }
    var j2, k2;
    for (j2 = 0; j2 < foto.lideres.length; j2++) {
      var lA = foto.lideres[j2], lB = null;
      for (k2 = 0; k2 < agoraF.lideres.length; k2++) if (agoraF.lideres[k2].campo === lA.campo) lB = agoraF.lideres[k2];
      if (lB) out.lideres.push({ campo: lA.campo, titulo: lA.titulo, antes: lA.valor, agora: lB.valor, mudou: lA.valor !== lB.valor });
    }
    var fr = out.frases;
    var quando = out.dias === 0 ? "hoje" : "h\u00e1 " + out.dias + " dia" + (out.dias > 1 ? "s" : "");
    fr.push("Fotografado " + quando + ": " + fmt.numero(foto.total) + " \u2192 " + fmt.numero(agoraF.total) + " linhas (\u0394 " + (out.contagem.pct >= 0 ? "+" : "") + out.contagem.pct + "%)");
    var principal = null;
    for (kM in out.metricas) { if (out.metricas[kM].tipo === "moeda") { principal = kM; break; } }
    if (!principal) for (kM in out.metricas) { principal = kM; break; }
    if (principal) {
      var mP = out.metricas[principal];
      var fV = mP.tipo === "moeda" ? fmt.moeda : fmt.numero;
      fr.push(mP.titulo + " total: " + fV(Math.round(mP.soma.antes)) + " \u2192 " + fV(Math.round(mP.soma.agora)) + " (\u0394 " + (mP.soma.pct >= 0 ? "+" : "") + mP.soma.pct + "%)");
      fr.push(mP.titulo + " m\u00e9dio: " + fV(Math.round(mP.media.antes)) + " \u2192 " + fV(Math.round(mP.media.agora)) + " (\u0394 " + (mP.media.pct >= 0 ? "+" : "") + mP.media.pct + "%)");
    }
    for (j2 = 0; j2 < out.lideres.length; j2++) if (out.lideres[j2].mudou) fr.push("L\u00edder de " + out.lideres[j2].titulo + ": " + out.lideres[j2].antes + " \u2192 " + out.lideres[j2].agora);
    return out;
  }

  /* ===== C5: COMPARADOR DE RECORTES ===== */
  function compararRecortes(linhasA, linhasB, colunas, opts) {
    opts = opts || {};
    var rA = resumir(linhasA, colunas, { topN: opts.topN || 3 });
    var rB = resumir(linhasB, colunas, { topN: opts.topN || 3 });
    function dpct(a, b) { return a !== 0 ? Math.round((b / a - 1) * 1000) / 10 : (b !== 0 ? 100 : 0); }
    var out = { a: rA, b: rB, rotuloA: opts.rotuloA || "A", rotuloB: opts.rotuloB || "B",
      contagem: { a: rA.total, b: rB.total, delta: rB.total - rA.total, pct: dpct(rA.total, rB.total) },
      metricas: {}, lideres: [], frases: [] };
    var kM;
    for (kM in rA.metricas) {
      if (!rB.metricas[kM]) continue;
      var mA = rA.metricas[kM], mB = rB.metricas[kM];
      out.metricas[kM] = { titulo: mA.titulo, tipo: mA.tipo,
        soma: { a: mA.soma, b: mB.soma, delta: mB.soma - mA.soma, pct: dpct(mA.soma, mB.soma) },
        media: { a: mA.media, b: mB.media, delta: mB.media - mA.media, pct: dpct(mA.media, mB.media) },
        mediana: { a: mA.mediana, b: mB.mediana, delta: mB.mediana - mA.mediana, pct: dpct(mA.mediana, mB.mediana) } };
    }
    var j2, k2;
    for (j2 = 0; j2 < rA.tops.length; j2++) {
      var tA = rA.tops[j2], tB = null;
      for (k2 = 0; k2 < rB.tops.length; k2++) if (rB.tops[k2].campo === tA.campo) tB = rB.tops[k2];
      if (!tB || !tA.porContagem.length || !tB.porContagem.length) continue;
      var lA = tA.porContagem[0], lB = tB.porContagem[0];
      out.lideres.push({ campo: tA.campo, titulo: tA.titulo, a: lA.valor, pctA: lA.pct, b: lB.valor, pctB: lB.pct, mudou: lA.valor !== lB.valor });
    }
    var fr = out.frases;
    fr.push(out.rotuloA + ": " + fmt.numero(rA.total) + " linhas vs " + out.rotuloB + ": " + fmt.numero(rB.total) + " (\u0394 " + (out.contagem.pct >= 0 ? "+" : "") + out.contagem.pct + "%)");
    var principal = null;
    for (kM in out.metricas) { if (out.metricas[kM].tipo === "moeda") { principal = kM; break; } }
    if (!principal) for (kM in out.metricas) { principal = kM; break; }
    if (principal) {
      var mP = out.metricas[principal];
      var fV = mP.tipo === "moeda" ? fmt.moeda : fmt.numero;
      fr.push(mP.titulo + " m\u00e9dio: " + fV(Math.round(mP.media.a * 100) / 100) + " vs " + fV(Math.round(mP.media.b * 100) / 100) + " (\u0394 " + (mP.media.pct >= 0 ? "+" : "") + mP.media.pct + "%)");
      fr.push(mP.titulo + " total: " + fV(Math.round(mP.soma.a * 100) / 100) + " vs " + fV(Math.round(mP.soma.b * 100) / 100) + " (\u0394 " + (mP.soma.pct >= 0 ? "+" : "") + mP.soma.pct + "%)");
    }
    for (j2 = 0; j2 < out.lideres.length; j2++) {
      var L9 = out.lideres[j2];
      if (L9.mudou) fr.push("L\u00edder de " + L9.titulo + " mudou: " + L9.a + " (" + L9.pctA + "%) \u2192 " + L9.b + " (" + L9.pctB + "%)");
    }
    return out;
  }

  /* ===== MODO KIOSK / TV: rotacao automatica de recortes =====
     Painel de parede: uma sequencia de recortes que se reveza sozinha, sem ninguem
     tocando na tela. Cada recorte monta o proprio conteudo e e destruido ao sair, senao
     um painel ligado 8 horas acumula grids mortos ate o navegador engasgar. */
  function kiosk(alvo, cfgQ) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo n\u00e3o encontrado" };
    cfgQ = cfgQ || {};
    var recortes = (cfgQ.recortes || []).slice();
    if (!recortes.length) return { ok: false, erro: "kiosk exige ao menos um recorte em cfg.recortes" };
    var segPadrao = Number(cfgQ.segundos) > 0 ? Number(cfgQ.segundos) : 12;
    var aleatorio = cfgQ.ordem === "aleatoria";
    var idx = 0, tmr = null, tmrProg = null, fimCiclo = 0, restante = 0;
    var rodando = false, pausado = false, pausaAuto = false, morto = false, atualApi = null;
    var logs = [], ouvintesDoc = [];

    function log(ev, extra) {
      var e2 = { ev: "phx.kiosk." + ev, t: Date.now() }, k3;
      for (k3 in extra) if (Object.prototype.hasOwnProperty.call(extra, k3)) e2[k3] = extra[k3];
      logs.push(e2);
      if (logs.length > 500) logs.shift();   /* painel roda por horas: o log nao pode crescer sem teto */
      if (cfgQ.aoLog) try { cfgQ.aoLog(e2); } catch (e9) {}
      return e2;
    }
    function docOn(ev9, fn9) { document.addEventListener(ev9, fn9, false); ouvintesDoc.push([ev9, fn9]); }
    function docOff() {
      var j9;
      for (j9 = 0; j9 < ouvintesDoc.length; j9++) document.removeEventListener(ouvintesDoc[j9][0], ouvintesDoc[j9][1], false);
      ouvintesDoc.length = 0;
    }
    function limpaTimers() {
      if (tmr) { clearTimeout(tmr); tmr = null; }
      if (tmrProg) { clearTimeout(tmrProg); tmrProg = null; }
    }

    var raiz = el("div", "phx-kiosk" + (cfgQ.tema === "escuro" ? " phx-tema-escuro" : ""));
    no.innerHTML = "";
    no.appendChild(raiz);
    raiz.innerHTML = '<div class="phx-kiosk-cab"><div class="phx-kiosk-tit"><div class="phx-kiosk-titulo"></div>' +
      '<div class="phx-kiosk-sub"></div></div><div class="phx-kiosk-conta"></div></div>' +
      '<div class="phx-kiosk-corpo"></div>' +
      '<div class="phx-kiosk-rodape"><div class="phx-kiosk-pontos"></div>' +
      '<div class="phx-kiosk-barra"><div class="phx-kiosk-progresso"></div></div></div>';
    var elTitulo = raiz.querySelector(".phx-kiosk-titulo");
    var elSub = raiz.querySelector(".phx-kiosk-sub");
    var elConta = raiz.querySelector(".phx-kiosk-conta");
    var elCorpo = raiz.querySelector(".phx-kiosk-corpo");
    var elPontos = raiz.querySelector(".phx-kiosk-pontos");
    var elProg = raiz.querySelector(".phx-kiosk-progresso");

    function segundosDe(r9) { return Number(r9 && r9.segundos) > 0 ? Number(r9.segundos) : segPadrao; }
    function pintaPontos() {
      var h9 = [], j9;
      for (j9 = 0; j9 < recortes.length; j9++) {
        h9.push('<span class="phx-kiosk-ponto' + (j9 === idx ? " phx-kiosk-ativo" : "") + '" data-ir="' + j9 +
          '" title="' + esc(recortes[j9].titulo || ("recorte " + (j9 + 1))) + '"></span>');
      }
      elPontos.innerHTML = h9.join("");
      var pts = elPontos.querySelectorAll(".phx-kiosk-ponto"), k9;
      for (k9 = 0; k9 < pts.length; k9++) (function (pt) {
        pt.addEventListener("click", function () { apiQ.irPara(Number(pt.getAttribute("data-ir"))); });
      })(pts[k9]);
    }
    function zeraProgresso() {
      if (!elProg) return;
      elProg.style.transition = "none";
      elProg.style.width = "0%";
    }
    function animaProgresso(ms9) {
      if (!elProg) return;
      elProg.style.transition = "none";
      elProg.style.width = "0%";
      /* o avanco e uma transicao de CSS, nao um tique por quadro: um painel de TV ligado o dia
         inteiro nao pode gastar um timer por frame so para mexer uma barrinha */
      tmrProg = setTimeout(function () {
        tmrProg = null;
        if (morto) return;
        elProg.style.transition = "width " + (ms9 / 1000) + "s linear";
        elProg.style.width = "100%";
      }, 30);
    }
    function congelaProgresso(ms9Restante, ms9Total) {
      if (!elProg) return;
      var pct = ms9Total > 0 ? Math.max(0, Math.min(100, 100 * (1 - ms9Restante / ms9Total))) : 0;
      elProg.style.transition = "none";
      elProg.style.width = Math.round(pct) + "%";
    }
    function limpaConteudo() {
      if (atualApi && typeof atualApi.destruir === "function") { try { atualApi.destruir(); } catch (e9) {} }
      atualApi = null;
      if (elCorpo) elCorpo.innerHTML = "";
    }
    function cfgDoRecorte(r9) {
      var base = {}, k9;
      for (k9 in (cfgQ.grid || {})) if (Object.prototype.hasOwnProperty.call(cfgQ.grid, k9)) base[k9] = cfgQ.grid[k9];
      for (k9 in (r9.grid || {})) if (Object.prototype.hasOwnProperty.call(r9.grid, k9)) base[k9] = r9.grid[k9];
      if (r9.colunas) base.colunas = r9.colunas;
      if (r9.chave) base.chave = r9.chave;
      base.dados = r9.dados || [];
      if (!base.pagina) base.pagina = { tamanho: 12 };
      return base;
    }
    function agenda(ms9) {
      limpaTimers();
      fimCiclo = Date.now() + ms9;
      tmr = setTimeout(function () {
        tmr = null;
        if (morto || !rodando || pausado) return;
        mostra(proximoIndice(), "timer");
      }, ms9);
      animaProgresso(ms9);
    }
    function proximoIndice() {
      if (!aleatorio || recortes.length < 2) return idx + 1;
      var n9 = idx;
      while (n9 === idx) n9 = Math.floor(Math.random() * recortes.length);
      return n9;
    }
    function mostra(i9, motivo) {
      if (morto) return apiQ;
      var n9 = recortes.length;
      var de = idx;
      idx = ((i9 % n9) + n9) % n9;          /* circular nos dois sentidos */
      var r9 = recortes[idx];
      limpaConteudo();
      elTitulo.innerHTML = esc(r9.titulo || "");
      elSub.innerHTML = esc(r9.subtitulo || "");
      elConta.innerHTML = (idx + 1) + "/" + n9;
      pintaPontos();
      try {
        atualApi = typeof r9.montar === "function" ? (r9.montar(elCorpo, apiQ) || null) : criar(elCorpo, cfgDoRecorte(r9));
      } catch (e9) {
        atualApi = null;
        elCorpo.innerHTML = '<div class="phx-kiosk-erro">' + esc("recorte n\u00e3o p\u00f4de ser montado: " + (e9 && e9.message)) + "</div>";
        log("erro-recorte", { indice: idx, titulo: r9.titulo || "", erro: String(e9 && e9.message) });
      }
      log("troca", { de: de, para: idx, titulo: r9.titulo || "", motivo: motivo || "timer" });
      if (cfgQ.aoTrocar) try { cfgQ.aoTrocar(idx, r9); } catch (e9) {}
      if (rodando && !pausado) agenda(segundosDe(r9) * 1000); else zeraProgresso();
      return apiQ;
    }

    if (cfgQ.pausarOculto !== false) {
      docOn("visibilitychange", function () {
        if (morto) return;
        /* aba ao fundo nao renderiza: continuar girando so queima CPU e desalinha o ciclo */
        if (document.hidden) {
          if (rodando && !pausado) { pausaAuto = true; apiQ.pausar(); log("oculto", {}); }
        } else if (pausaAuto) {
          pausaAuto = false; apiQ.retomar(); log("visivel", {});
        }
      });
    }
    if (cfgQ.teclado !== false) {
      docOn("keydown", function (ev9) {
        if (morto || !raiz.parentNode) return;
        var k9 = ev9.key;
        if (k9 === " " || k9 === "Spacebar") { ev9.preventDefault(); if (pausado) apiQ.retomar(); else apiQ.pausar(); }
        else if (k9 === "ArrowRight") { ev9.preventDefault(); apiQ.proximo(); }
        else if (k9 === "ArrowLeft") { ev9.preventDefault(); apiQ.anterior(); }
        else if (k9 === "f" || k9 === "F") { apiQ.telaCheia(); }
      });
    }

    var apiQ = {
      ok: true,
      el: raiz,
      iniciar: function () {
        if (morto || rodando) return apiQ;
        rodando = true; pausado = false;
        log("inicio", { recortes: recortes.length, segundos: segPadrao });
        return mostra(idx, "inicio");
      },
      parar: function () {
        if (morto) return apiQ;
        rodando = false; pausado = false; pausaAuto = false;
        limpaTimers(); zeraProgresso();
        log("parada", { indice: idx });
        return apiQ;
      },
      pausar: function () {
        if (morto || !rodando || pausado) return apiQ;
        pausado = true;
        var total = segundosDe(recortes[idx]) * 1000;
        restante = Math.max(0, fimCiclo - Date.now());
        limpaTimers();
        congelaProgresso(restante, total);
        raiz.className = raiz.className.replace(" phx-kiosk-pausado", "") + " phx-kiosk-pausado";
        log("pausa", { indice: idx, restanteMs: restante });
        return apiQ;
      },
      retomar: function () {
        if (morto || !rodando || !pausado) return apiQ;
        pausado = false;
        raiz.className = raiz.className.replace(" phx-kiosk-pausado", "");
        agenda(restante > 0 ? restante : segundosDe(recortes[idx]) * 1000);
        log("retomada", { indice: idx });
        return apiQ;
      },
      proximo: function () { return mostra(proximoIndice(), "manual"); },
      anterior: function () { return mostra(idx - 1, "manual"); },
      irPara: function (i9) { return mostra(Number(i9) || 0, "manual"); },
      atual: function () { return idx; },
      conteudo: function () { return atualApi; },
      definirSegundos: function (s9) {
        segPadrao = Number(s9) > 0 ? Number(s9) : segPadrao;
        if (rodando && !pausado) agenda(segundosDe(recortes[idx]) * 1000);
        log("segundos", { segundos: segPadrao });
        return apiQ;
      },
      telaCheia: function (ligar) {
        var querCheia = ligar === undefined ? !(document.fullscreenElement === raiz) : !!ligar;
        if (querCheia && !raiz.requestFullscreen) { log("tela-cheia-indisponivel", {}); return { ok: false, erro: "tela cheia n\u00e3o suportada neste navegador" }; }
        try {
          if (querCheia) raiz.requestFullscreen();
          else if (document.exitFullscreen) document.exitFullscreen();
        } catch (e9) { return { ok: false, erro: String(e9 && e9.message) }; }
        log("tela-cheia", { ligada: querCheia });
        return { ok: true, telaCheia: querCheia };
      },
      estado: function () {
        return { indice: idx, titulo: recortes[idx] ? (recortes[idx].titulo || "") : "", total: recortes.length,
                 rodando: rodando, pausado: pausado, segundos: segundosDe(recortes[idx]) };
      },
      recortes: function (novos9) {
        if (!novos9) return recortes.slice();
        recortes = novos9.slice();
        if (!recortes.length) return apiQ;
        return mostra(idx >= recortes.length ? 0 : idx, "recortes");
      },
      logs: function () { return logs.slice(); },
      destruir: function () {
        if (morto) return true;
        morto = true; rodando = false; pausado = false;
        limpaTimers(); docOff(); limpaConteudo();
        no.innerHTML = "";
        logs.length = 0;
        return true;
      }
    };

    mostra(0, "init");
    log("init", { recortes: recortes.length, segundos: segPadrao });
    if (cfgQ.iniciar !== false) apiQ.iniciar();
    return apiQ;
  }
  function kioskJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    var handle = "phx" + (++_seqInst);
    var ouvintes = {};
    cfg.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.kiosk.", "");
      var fns = ouvintes[tipo] || [], j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    if (cfg.aoTrocarWL) {
      var nomeFn = cfg.aoTrocarWL;
      cfg.aoTrocar = function (i, r) {
        var fn = root[nomeFn];
        if (typeof fn !== "function") return;
        try { fn(JSON.stringify({ indice: i, titulo: r && r.titulo })); } catch (e9) {}
      };
    }
    var apiQ = kiosk(seletor, cfg);
    if (apiQ && apiQ.ok === false) return JSON.stringify(apiQ);
    _instancias[handle] = { api: apiQ, seletor: seletor, ouvintes: ouvintes };
    return JSON.stringify({ ok: true, handle: handle });
  }

  function interpretarComparacao(frase, ctx) {
    var t = " " + semAcento(String(frase || "")) + " ";
    var m9 = t.match(/ compar(?:a|e|ar) (.+?) (?:vs|versus|contra|com) (.+?) $/);
    if (!m9) m9 = t.match(/ (.+?) (?:vs|versus) (.+?) $/);
    if (!m9) return null;
    var ladoA = m9[1].replace(/^\s+|\s+$/g, ""), ladoB = m9[2].replace(/^\s+|\s+$/g, "");
    return { fraseA: ladoA, fraseB: ladoB, resA: interpretar(ladoA, ctx), resB: interpretar(ladoB, ctx) };
  }

  /* ===== PONTE BROWSER_ExecJS (WinDev/WebDev) — tudo por strings ===== */
  var _instancias = {};
  var _seqInst = 0;
  function _resolveFn(nome) {
    if (typeof nome === "function") return nome;
    if (typeof nome === "string" && root[nome] && typeof root[nome] === "function") return root[nome];
    return null;
  }
  function _hidrataCfg(cfg) {
    var j2, c2;
    for (j2 = 0; j2 < (cfg.colunas || []).length; j2++) {
      c2 = cfg.colunas[j2];
      if (typeof c2.formato === "string") c2.formato = _resolveFn(c2.formato);
      if (typeof c2.href === "string") c2.href = _resolveFn(c2.href);
    }
    if (cfg.detalhe && typeof cfg.detalhe.render === "string") cfg.detalhe.render = _resolveFn(cfg.detalhe.render);
    if (cfg.edicao && typeof cfg.edicao.aoEditar === "string") cfg.edicao.aoEditar = _resolveFn(cfg.edicao.aoEditar);
    if (cfg.notas && typeof cfg.notas.aoMudar === "string") cfg.notas.aoMudar = _resolveFn(cfg.notas.aoMudar);
    if (typeof cfg.aoMudarLayout === "string") cfg.aoMudarLayout = _resolveFn(cfg.aoMudarLayout);
    return cfg;
  }
  function renderJSON(seletor, jsonCfg) {
    var cfg;
    try { cfg = typeof jsonCfg === "string" ? JSON.parse(jsonCfg) : jsonCfg; }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    try {
      _hidrataCfg(cfg);
      var handle = "phx" + (++_seqInst);
      var ouvintes = {};
      var aoLogOrig = cfg.aoLog;
      cfg.aoLog = function (entrada) {
        if (aoLogOrig) try { aoLogOrig(entrada); } catch (e9) {}
        var tipo = String(entrada.ev || "").replace("phx.grid.", "");
        var fns = ouvintes[tipo] || [];
        var j3;
        for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
      };
      var api2 = criar(seletor, cfg);
      if (api2 && api2.ok === false) return JSON.stringify(api2);
      _instancias[handle] = { api: api2, seletor: seletor, ouvintes: ouvintes, cfgBase: JSON.parse(typeof jsonCfg === "string" ? jsonCfg : JSON.stringify(jsonCfg)) };
      return JSON.stringify({ ok: true, handle: handle });
    } catch (e3) { return JSON.stringify({ ok: false, erro: String(e3 && e3.message || e3) }); }
  }
  function exec(handle, metodo, jsonArgs) {
    var inst = _instancias[handle];
    if (!inst) return JSON.stringify({ ok: false, erro: "handle desconhecido: " + handle });
    if (typeof inst.api[metodo] !== "function") return JSON.stringify({ ok: false, erro: "metodo inexistente: " + metodo });
    var args = [];
    if (jsonArgs != null && jsonArgs !== "") {
      try { args = JSON.parse(jsonArgs); if (!(args instanceof Array)) args = [args]; }
      catch (e2) { return JSON.stringify({ ok: false, erro: "args JSON invalido: " + e2.message }); }
    }
    try {
      var r = inst.api[metodo].apply(inst.api, args);
      if (r === inst.api) return JSON.stringify({ ok: true });
      return JSON.stringify({ ok: true, retorno: r === undefined ? null : r });
    } catch (e3) { return JSON.stringify({ ok: false, erro: String(e3 && e3.message || e3) }); }
  }
  function estadoJSON(handle) {
    var inst = _instancias[handle];
    if (!inst) return JSON.stringify({ ok: false, erro: "handle desconhecido" });
    return JSON.stringify({ ok: true, estado: inst.api.estado(), layout: inst.api.layout() });
  }
  function dadosJSON(handle, jsonLinhas, modo) {
    var inst = _instancias[handle];
    if (!inst) return JSON.stringify({ ok: false, erro: "handle desconhecido" });
    var linhas;
    try { linhas = JSON.parse(jsonLinhas); }
    catch (e2) { return JSON.stringify({ ok: false, erro: "JSON invalido: " + e2.message }); }
    if (modo === "anexar") { inst.api.anexarDados(linhas); return JSON.stringify({ ok: true, modo: "anexar", n: linhas.length }); }
    var lay = inst.api.layout();
    var cfgN = JSON.parse(JSON.stringify(inst.cfgBase));
    cfgN.dados = linhas;
    _hidrataCfg(cfgN);
    var ouvintesAnt = inst.ouvintes;
    cfgN.aoLog = function (entrada) {
      var tipo = String(entrada.ev || "").replace("phx.grid.", "");
      var fns = ouvintesAnt[tipo] || [];
      var j3;
      for (j3 = 0; j3 < fns.length; j3++) try { fns[j3](JSON.stringify(entrada)); } catch (e9) {}
    };
    inst.api.destruir();
    inst.api = criar(inst.seletor, cfgN);
    inst.api.aplicarLayout(lay);
    return JSON.stringify({ ok: true, modo: "substituir", n: linhas.length });
  }
  /* ==================== CAMADA DE COMUNICAÇÃO SEGURA (v0.74) ====================
     Lado cliente da segurança do conector. ES5 estrito, zero dependência.

     SHA-256 e HMAC-SHA256 são escritos aqui e no conector Rust (conector/src/seguranca.rs).
     As duas implementações são independentes e TÊM QUE produzir byte idêntico -- é o par de
     oráculos que dá confiança em criptografia escrita à mão. Ambas são conferidas contra os
     vetores oficiais (FIPS 180-4 e RFC 4231) e uma contra a outra.

     O que o cliente faz em cada requisição:
       1. calcula SHA-256 do corpo;
       2. monta a string canônica (método, caminho, timestamp, nonce, sha do corpo);
       3. assina com HMAC-SHA256 usando o segredo compartilhado;
       4. envia token, timestamp, nonce e assinatura nos cabeçalhos.
     O segredo NUNCA vai na requisição -- só a prova de que o cliente o possui. */

  var K256 = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
  ];

  /* UTF-8: os bytes têm que ser EXATAMENTE os mesmos que o Rust produz, senão o HMAC diverge
     em qualquer texto com acento -- e o ERP é todo em português. */
  function utf8Bytes(s9) {
    var out9 = [], i9, c9, j9;
    for (i9 = 0; i9 < s9.length; i9++) {
      c9 = s9.charCodeAt(i9);
      if (c9 < 0x80) out9.push(c9);
      else if (c9 < 0x800) { out9.push(0xc0 | (c9 >> 6), 0x80 | (c9 & 0x3f)); }
      else if (c9 >= 0xd800 && c9 <= 0xdbff && i9 + 1 < s9.length) {
        j9 = s9.charCodeAt(i9 + 1);
        if (j9 >= 0xdc00 && j9 <= 0xdfff) {
          var cp9 = ((c9 - 0xd800) << 10) + (j9 - 0xdc00) + 0x10000;
          out9.push(0xf0 | (cp9 >> 18), 0x80 | ((cp9 >> 12) & 0x3f), 0x80 | ((cp9 >> 6) & 0x3f), 0x80 | (cp9 & 0x3f));
          i9++;
        } else out9.push(0xef, 0xbf, 0xbd);
      } else out9.push(0xe0 | (c9 >> 12), 0x80 | ((c9 >> 6) & 0x3f), 0x80 | (c9 & 0x3f));
    }
    return out9;
  }

  function rotr32(x9, n9) { return ((x9 >>> n9) | (x9 << (32 - n9))) >>> 0; }

  /* SHA-256 sobre um array de bytes. Devolve array de 32 bytes. */
  function sha256Bytes(msg9) {
    var h9 = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    var d9 = msg9.slice(), i9, j9;
    var bits9 = msg9.length * 8;
    d9.push(0x80);
    while (d9.length % 64 !== 56) d9.push(0);
    /* comprimento em 64 bits big-endian. Usa divisão para o alto (bitwise em JS é 32 bits). */
    var alto9 = Math.floor(bits9 / 4294967296), baixo9 = bits9 >>> 0;
    d9.push((alto9 >>> 24) & 255, (alto9 >>> 16) & 255, (alto9 >>> 8) & 255, alto9 & 255);
    d9.push((baixo9 >>> 24) & 255, (baixo9 >>> 16) & 255, (baixo9 >>> 8) & 255, baixo9 & 255);

    var w9 = new Array(64);
    for (i9 = 0; i9 < d9.length; i9 += 64) {
      for (j9 = 0; j9 < 16; j9++) {
        w9[j9] = ((d9[i9 + j9 * 4] << 24) | (d9[i9 + j9 * 4 + 1] << 16) | (d9[i9 + j9 * 4 + 2] << 8) | d9[i9 + j9 * 4 + 3]) >>> 0;
      }
      for (j9 = 16; j9 < 64; j9++) {
        var s0a = rotr32(w9[j9 - 15], 7) ^ rotr32(w9[j9 - 15], 18) ^ (w9[j9 - 15] >>> 3);
        var s1a = rotr32(w9[j9 - 2], 17) ^ rotr32(w9[j9 - 2], 19) ^ (w9[j9 - 2] >>> 10);
        w9[j9] = (((w9[j9 - 16] + s0a) >>> 0) + ((w9[j9 - 7] + s1a) >>> 0)) >>> 0;
      }
      var a9 = h9[0], b9 = h9[1], c9 = h9[2], dd9 = h9[3], e9 = h9[4], f9 = h9[5], g9 = h9[6], hh9 = h9[7];
      for (j9 = 0; j9 < 64; j9++) {
        var S1 = rotr32(e9, 6) ^ rotr32(e9, 11) ^ rotr32(e9, 25);
        var ch = (e9 & f9) ^ ((~e9) & g9);
        var t1 = (((((hh9 + S1) >>> 0) + ch) >>> 0) + ((K256[j9] + w9[j9]) >>> 0)) >>> 0;
        var S0 = rotr32(a9, 2) ^ rotr32(a9, 13) ^ rotr32(a9, 22);
        var maj = (a9 & b9) ^ (a9 & c9) ^ (b9 & c9);
        var t2 = ((S0 + maj) >>> 0);
        hh9 = g9; g9 = f9; f9 = e9;
        e9 = (dd9 + t1) >>> 0;
        dd9 = c9; c9 = b9; b9 = a9;
        a9 = (t1 + t2) >>> 0;
      }
      h9[0] = (h9[0] + a9) >>> 0; h9[1] = (h9[1] + b9) >>> 0;
      h9[2] = (h9[2] + c9) >>> 0; h9[3] = (h9[3] + dd9) >>> 0;
      h9[4] = (h9[4] + e9) >>> 0; h9[5] = (h9[5] + f9) >>> 0;
      h9[6] = (h9[6] + g9) >>> 0; h9[7] = (h9[7] + hh9) >>> 0;
    }
    var out9 = [];
    for (i9 = 0; i9 < 8; i9++) out9.push((h9[i9] >>> 24) & 255, (h9[i9] >>> 16) & 255, (h9[i9] >>> 8) & 255, h9[i9] & 255);
    return out9;
  }

  function bytesHex(b9) {
    var s9 = "", i9, h9;
    for (i9 = 0; i9 < b9.length; i9++) { h9 = (b9[i9] & 255).toString(16); s9 += h9.length === 1 ? "0" + h9 : h9; }
    return s9;
  }

  function sha256Hex(texto9) { return bytesHex(sha256Bytes(utf8Bytes(String(texto9)))); }

  /* HMAC-SHA256 (RFC 2104) sobre bytes. */
  function hmacSha256Bytes(chave9, msg9) {
    var k9 = [], i9;
    if (chave9.length > 64) k9 = sha256Bytes(chave9).slice();
    else k9 = chave9.slice();
    while (k9.length < 64) k9.push(0);
    var ipad9 = [], opad9 = [];
    for (i9 = 0; i9 < 64; i9++) { ipad9.push(0x36 ^ k9[i9]); opad9.push(0x5c ^ k9[i9]); }
    var interno9 = sha256Bytes(ipad9.concat(msg9));
    return sha256Bytes(opad9.concat(interno9));
  }

  function hmacSha256Hex(chave9, msg9) {
    return bytesHex(hmacSha256Bytes(utf8Bytes(String(chave9)), utf8Bytes(String(msg9))));
  }

  /* Nonce do CSPRNG do navegador. Math.random NÃO serve para nonce de segurança --
     é previsível e permitiria montar um replay. Falha ruidosa se não houver CSPRNG. */
  function nonceSeguro() {
    var cr9 = (typeof window !== "undefined" && (window.crypto || window.msCrypto)) || null;
    if (!cr9 || !cr9.getRandomValues) {
      throw new Error("phx-seguranca: sem crypto.getRandomValues neste ambiente. " +
        "Nonce previsivel permitiria replay; a chamada e recusada em vez de degradar em silencio.");
    }
    var b9 = new Uint8Array(16);
    cr9.getRandomValues(b9);
    var arr9 = [], i9;
    for (i9 = 0; i9 < b9.length; i9++) arr9.push(b9[i9]);
    return bytesHex(arr9);
  }

  /* String canônica: MESMA ordem e MESMO separador do lado Rust. Qualquer divergência
     muda o HMAC e a requisição é rejeitada -- o que é o comportamento desejado. */
  function stringCanonica(metodo9, caminho9, ts9, nonce9, corpoShaHex9) {
    return metodo9 + "\n" + caminho9 + "\n" + ts9 + "\n" + nonce9 + "\n" + corpoShaHex9;
  }

  /* Assina uma requisição e devolve os cabeçalhos prontos. */
  function assinaRequisicao(cfg9, metodo9, caminho9, corpoTexto9) {
    if (!cfg9 || !cfg9.token) throw new Error("phx-seguranca: token nao configurado (falha fechada)");
    var ts9 = Math.floor((cfg9._agora ? cfg9._agora() : Date.now()) / 1000);
    var nonce9 = cfg9._nonce ? cfg9._nonce() : nonceSeguro();
    var corpoSha9 = bytesHex(sha256Bytes(utf8Bytes(corpoTexto9 || "")));
    var canon9 = stringCanonica(metodo9, caminho9, ts9, nonce9, corpoSha9);
    var segredo9 = cfg9.segredo || cfg9.token;
    return {
      "X-Phx-Token": cfg9.token,
      "X-Phx-Ts": String(ts9),
      "X-Phx-Nonce": nonce9,
      "X-Phx-Assinatura": hmacSha256Hex(segredo9, canon9),
      "Content-Type": "application/json"
    };
  }

  /* Fonte de dados do conector, com a camada segura ligada.
     Recusa-se a falar com endereço não-loopback sem https -- falha fechada, igual ao servidor. */
  function fonteConectorSeguro(cfgC9) {
    cfgC9 = cfgC9 || {};
    var url9 = String(cfgC9.url || "http://127.0.0.1:7311").replace(/\/+$/, "");
    var ehLoopback9 = /^https?:\/\/(127\.0\.0\.1|localhost|\[::1\])(:|$|\/)/.test(url9);
    var ehTls9 = url9.indexOf("https://") === 0;
    if (!ehLoopback9 && !ehTls9) {
      throw new Error("phx-seguranca: recusado falar com " + url9 + " sem TLS. " +
        "Fora do loopback os dados do ERP trafegariam em claro. Use https:// ou 127.0.0.1.");
    }
    if (!cfgC9.token) {
      throw new Error("phx-seguranca: token obrigatorio. O conector gera um no arranque; " +
        "passe-o em fonteConectorSeguro({token: ...}).");
    }
    function chama9(caminho9, corpo9, cb9) {
      var texto9 = JSON.stringify(corpo9 || {});
      var cabs9;
      try { cabs9 = assinaRequisicao(cfgC9, "POST", caminho9, texto9); }
      catch (e9) { cb9(e9, null); return; }
      var x9 = new XMLHttpRequest();
      x9.open("POST", url9 + caminho9, true);
      var k9;
      for (k9 in cabs9) if (Object.prototype.hasOwnProperty.call(cabs9, k9)) x9.setRequestHeader(k9, cabs9[k9]);
      x9.onreadystatechange = function () {
        if (x9.readyState !== 4) return;
        if (x9.status === 401 || x9.status === 403) { cb9(new Error("phx-conector recusou: " + x9.status + " " + x9.responseText), null); return; }
        if (x9.status < 200 || x9.status >= 300) { cb9(new Error("phx-conector HTTP " + x9.status), null); return; }
        var j9; try { j9 = JSON.parse(x9.responseText); } catch (e2) { cb9(new Error("resposta invalida"), null); return; }
        cb9(null, j9);
      };
      x9.send(texto9);
    }
    return {
      seguro: true,
      buscar: function (params9, cb9) {
        chama9("/consulta", { fonte: cfgC9.fonte, dsn: cfgC9.dsn, tabela: cfgC9.tabela, params: params9 }, function (e9, r9) {
          if (e9) { cb9(e9, null); return; }
          cb9(null, { linhas: (r9 && r9.linhas) || [], total: (r9 && r9.total) || 0 });
        });
      },
      saude: function (cb9) { chama9("/saude", {}, cb9); },
      _assina: assinaRequisicao
    };
  }

  function aoEvento(handle, tipo, nomeFn) {
    var inst = _instancias[handle];
    if (!inst) return JSON.stringify({ ok: false, erro: "handle desconhecido" });
    var fn = _resolveFn(nomeFn);
    if (!fn) return JSON.stringify({ ok: false, erro: "funcao nao encontrada em window: " + nomeFn });
    (inst.ouvintes[tipo] || (inst.ouvintes[tipo] = [])).push(fn);
    return JSON.stringify({ ok: true });
  }
  function destruirJSON(handle) {
    var inst = _instancias[handle];
    if (!inst) return JSON.stringify({ ok: false, erro: "handle desconhecido" });
    inst.api.destruir();
    delete _instancias[handle];
    return JSON.stringify({ ok: true });
  }

  var PhxGrid = { versao: "0.78.1", criar: criar, fonteConectorSeguro: fonteConectorSeguro, sha256Hex: sha256Hex, hmacSha256Hex: hmacSha256Hex, stringCanonica: stringCanonica, assinaRequisicao: assinaRequisicao, pivotar: pivotar, vertical: vertical, fmt: fmt, _ordenaEstavel: ordenaEstavel, _scanFiltros: aplicaFiltros, interpretar: interpretar, resumir: resumir, interpretarComando: interpretarComando, compararRecortes: compararRecortes, linhaDoTempo: linhaDoTempo, temaDe: temaDe, visualParaTokens: visualParaTokens, _mapaVisual: MAPA_VISUAL, registrarTema: registrarTema, _montaXLSX: montaXLSX, kanban: kanban, kanbanJSON: kanbanJSON, kiosk: kiosk, kioskJSON: kioskJSON, gantt: gantt, ganttJSON: ganttJSON, matriz: matriz, matrizJSON: matrizJSON, cubo: cubo, cuboJSON: cuboJSON, dropdown: dropdown, segmentacao: segmentacao, segmentacaoJSON: segmentacaoJSON, carregar: carregar, fonteConector: fonteConector, _deflate: deflate, definirTextos: definirTextos, textos: TEXTOS, _inflate: inflate, _leZip: leZip, _crc32: crc32, _utf8Bytes: utf8Bytes, interpretarComparacao: interpretarComparacao, _workerSrc: _workerSrc, renderJSON: renderJSON, exec: exec, estadoJSON: estadoJSON, dadosJSON: dadosJSON, aoEvento: aoEvento, destruirJSON: destruirJSON, _instancias: _instancias, _setCriaWorker: function (fn) { criaWorkerImpl = fn || function (src) { var blob = new Blob([src], { type: "application/javascript" }); var url = URL.createObjectURL(blob); var w = new Worker(url); w._phxUrl = url; return w; }; } };
  if (typeof module !== "undefined" && module.exports) module.exports = PhxGrid;
  root.PhxGrid = PhxGrid;
})(typeof window !== "undefined" ? window : this);

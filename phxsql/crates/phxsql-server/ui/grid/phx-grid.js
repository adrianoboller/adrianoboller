/* phx-grid v0.9.2 — Phoenix / WX Soluções — ES5 estrito, zero dependências.

   O cabeçalho passou oito versões dizendo "v0.1.0 — Núcleo (S01)" enquanto o
   CHANGELOG ao lado ia até a 0.8.0 e o código já tinha ordem por nível de
   grupo, rodapé de grupo e total geral — que a 0.8.0 nem documenta. Número
   digitado à mão envelhece calado, e este envelheceu em três lugares ao mesmo
   tempo. Hoje os três (aqui, o `versao:` do fim do arquivo e o topo do
   CHANGELOG) são conferidos por `grade_versao_nao_mente`, em http.rs. */
(function (root) {
  "use strict";

  function esc(s) {
    return String(s == null ? "" : s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }
  /* O texto de tela pela fabrica de idiomas, com o portugues de fabrica ao
     lado. Delega no `root.txt` da pagina hospedeira em vez de chamar o global
     direto: a grade e um componente de ZERO dependencias e continua sendo --
     sem a pagina em volta ela desenha em portugues, e nao estoura. */
  function txt(nome, padrao) {
    return root.txt ? root.txt(nome, padrao) : padrao;
  }

  /* O ROTULO da coluna -- o que se pinta na cabeca dela -- e diferente do
     NOME da coluna, que e como se fala dela no seletor de colunas, no resumo
     de filtro e na pastilha de grupo. Os dois caiam no mesmo `c.titulo ||
     c.campo`, e por isso uma coluna de acao (que declara `titulo: ""` de
     proposito, como o LEIAME manda) aparecia com `__acao` escrito em cima.
     Titulo DECLARADO manda, vazio inclusive; so quem nao declarou nenhum e
     que cai no nome do campo. O `== null` e proposital: pega o ausente e o
     nulo, e deixa o vazio passar -- com `||` a correcao nao existe. */
  function rotulo(c) {
    return c.titulo == null ? c.campo : c.titulo;
  }

  /* Poe os `{marcador}` no lugar. Posicional por nome, e nunca `+` no meio da
     frase: «Pagina 2 de 9» nao tem a mesma ordem em toda lingua. */
  function preencher(bruto, dados) {
    return String(bruto).replace(/\{(\w+)\}/g, function (m, k) {
      return (dados && Object.prototype.hasOwnProperty.call(dados, k)) ? String(dados[k]) : m;
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
  var AGGS = {
    sum: function (a) { var t = 0, i2; for (i2 = 0; i2 < a.length; i2++) t += Number(a[i2]) || 0; return t; },
    avg: function (a) { return a.length ? AGGS.sum(a) / a.length : 0; },
    count: function (a) { return a.length; },
    min: function (a) { var m = Infinity, i2, v2; for (i2 = 0; i2 < a.length; i2++) { v2 = Number(a[i2]); if (v2 === v2 && v2 < m) m = v2; } return m === Infinity ? null : m; },
    max: function (a) { var m = -Infinity, i2, v2; for (i2 = 0; i2 < a.length; i2++) { v2 = Number(a[i2]); if (v2 === v2 && v2 > m) m = v2; } return m === -Infinity ? null : m; }
  };
  // `dirs[d]` diz se o nivel d ordena crescente ou decrescente. O Janus(R) e o
  // DevExpress(R) deixam trocar isso clicando na pilula do grupo, e faz falta:
  // agrupar por mes quase sempre quer o mais recente em cima.
  function agrupa(linhas, campos, aggCols, tipoDe, dirs) {
    function nivel(lns, d, pathPai) {
      var mapa = {}, ordem2 = [], i2, v2, k2, g2;
      for (i2 = 0; i2 < lns.length; i2++) {
        v2 = lns[i2][campos[d]];
        k2 = String(v2);
        g2 = mapa[k2];
        if (!g2) { g2 = { campo: campos[d], valor: v2, chave: k2, linhas: [] }; mapa[k2] = g2; ordem2.push(g2); }
        g2.linhas.push(lns[i2]);
      }
      var sinal = (dirs && dirs[d] === "desc") ? -1 : 1;
      ordem2.sort(function (a, b) {
        var ka = chaveOrd(a.valor, tipoDe(campos[d])), kb = chaveOrd(b.valor, tipoDe(campos[d]));
        // Nulo fica sempre no fim, nas duas direcoes: ele nao e "o menor",
        // e a ausencia de valor.
        if (ka.n && kb.n) return 0;
        if (ka.n) return 1;
        if (kb.n) return -1;
        if (ka.v < kb.v) return -sinal;
        if (ka.v > kb.v) return sinal;
        return 0;
      });
      var out = [], j2, g3, no2;
      for (j2 = 0; j2 < ordem2.length; j2++) {
        g3 = ordem2[j2];
        no2 = { campo: g3.campo, valor: g3.valor, chave: g3.chave, nivel: d,
          path: pathPai + (pathPai ? "\u0001" : "") + g3.campo + "=" + g3.chave, n: g3.linhas.length, aggs: {} };
        var ac;
        for (ac = 0; ac < aggCols.length; ac++) {
          var col = aggCols[ac], vals = [], i3;
          for (i3 = 0; i3 < g3.linhas.length; i3++) vals.push(g3.linhas[i3][col.campo]);
          no2.aggs[col.campo] = AGGS[col.agregador](vals);
        }
        if (d + 1 < campos.length) no2.filhos = nivel(g3.linhas, d + 1, no2.path);
        else no2.linhas = g3.linhas;
        out.push(no2);
      }
      return out;
    }
    return nivel(linhas, 0, "");
  }
  function totaisDe(linhas, aggCols) {
    var o = {}, a, vals, i2;
    for (a = 0; a < aggCols.length; a++) {
      vals = [];
      for (i2 = 0; i2 < linhas.length; i2++) vals.push(linhas[i2][aggCols[a].campo]);
      o[aggCols[a].campo] = AGGS[aggCols[a].agregador](vals);
    }
    return o;
  }
  // Os nomes das linhas que NAO sao dado. Ficam numa lista so, ao lado de quem
  // as cria, porque a licao da casa e que peca nova no fim de uma lista quebra
  // quem filtra pela primeira: quem quiser so o dado chama `eMarcador` e nao
  // reescreve a condicao.
  var MARCADORES = ["__grupo", "__rodape"];
  function eMarcador(l) {
    var j2;
    for (j2 = 0; j2 < MARCADORES.length; j2++) if (l && l[MARCADORES[j2]]) return true;
    return false;
  }
  function achata(arvoreG, recolhidos, comRodape) {
    var out = [], i2;
    function anda(nos) {
      var j2, no2;
      for (j2 = 0; j2 < nos.length; j2++) {
        no2 = nos[j2];
        out.push({ __grupo: no2 });
        if (recolhidos[no2.path]) continue;
        if (no2.filhos) anda(no2.filhos);
        else for (i2 = 0; i2 < no2.linhas.length; i2++) out.push(no2.linhas[i2]);
        // O rodape repete o agregado embaixo do bloco. Num grupo de trinta
        // linhas o cabecalho ja rolou para fora da tela quando o total
        // interessa, e e ai que ele e lido.
        if (comRodape) out.push({ __rodape: no2 });
      }
    }
    anda(arvoreG);
    return out;
  }
  function fonteLocal(dados) {
    var idxBusca = null, idxChave = "";
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
      /* O indice da busca global e caro de montar, entao fica em cache por
         conjunto de campos. Num PAINEL VIVO -- uma grade sobre um array que o
         dono muda no lugar a cada volta do relogio -- esse cache envelhece: a
         busca passaria a responder pelas linhas de dois segundos atras, e isso
         e pior que uma busca lenta, porque a resposta errada tem a cara da
         certa. Quem chama `redesenhar()` esta dizendo «o dado mudou», e e ele
         quem manda esquecer. */
      invalidar: function () { idxBusca = null; idxChave = ""; },
      carregar: function (p, cb) {
        var t0 = agora();
        var fBusca = null, resto = [], j9;
        for (j9 = 0; j9 < (p.filtros || []).length; j9++) {
          if (p.filtros[j9].tipo === "busca") fBusca = p.filtros[j9];
          else resto.push(p.filtros[j9]);
        }
        var filtrados, base;
        if (fBusca) {
          var sobra = resto.length ? aplicaFiltros(dados, resto) : dados;
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
            base = ordenaEstavel(filtrados, p.ordem.campo, p.ordem.dir, p.ordem.tipo);
          } else {
            pontuadas.sort(function (a, b) { return b.s - a.s || a.ix - b.ix; });
            filtrados = base = [];
            for (j9 = 0; j9 < pontuadas.length; j9++) base.push(pontuadas[j9].l);
          }
        } else {
          filtrados = aplicaFiltros(dados, resto);
          base = p.ordem && p.ordem.campo
            ? ordenaEstavel(filtrados, p.ordem.campo, p.ordem.dir, p.ordem.tipo)
            : filtrados.slice();
        }
        var ini = (p.pagina - 1) * p.tamanho;
        if (p.grupos && p.grupos.length) {
          var arvG = agrupa(base, p.grupos, p.aggCols || [],
            function (c9) { return (p.tiposCampos && p.tiposCampos[c9]) || "texto"; },
            p.dirsGrupo || []);
          var plana = achata(arvG, p.recolhidos || {}, !!p.rodapeGrupo);
          cb(null, {
            linhas: plana.slice(ini, ini + p.tamanho),
            total: plana.length,
            totalDados: filtrados.length,
            // O total geral e sobre o conjunto FILTRADO, nao sobre a pagina:
            // um rodape que muda ao virar de pagina nao e total de nada.
            totaisGerais: totaisDe(base, p.aggCols || []),
            _ms: agora() - t0
          });
          return;
        }
        cb(null, {
          totaisGerais: totaisDe(base, p.aggCols || []),
          linhas: base.slice(ini, ini + p.tamanho),
          total: filtrados.length,
          _ms: agora() - t0
        });
      }
    };
  }

  function criar(alvo, cfg) {
    var no = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    if (!no) return { ok: false, erro: "alvo não encontrado: " + alvo };
    if (!cfg || !cfg.colunas || !cfg.colunas.length) return { ok: false, erro: "cfg.colunas é obrigatório" };

    var logs = [];
    function log(ev, extra) {
      var e = { ev: "phx.grid." + ev, t: Date.now() };
      var k; for (k in extra) if (Object.prototype.hasOwnProperty.call(extra, k)) e[k] = extra[k];
      logs.push(e);
      if (cfg.logConsole && root.console) console.log("[phx.grid]", ev, extra || "");
      return e;
    }

    var t0init = agora();
    var fonte = cfg.fonte || fonteLocal(cfg.dados || []);
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

    // ------------------------------------------------------------------
    // O LAYOUT LEMBRADO (`cfg.lembrar` = a chave; sem ela nada e gravado).
    //
    // Guarda o que a PESSOA arrumou -- largura, ordem, o que escondeu, o que
    // congelou e quantos itens por pagina --, e nunca filtro nem ordenacao:
    // um filtro que volta sozinho ao reabrir a tela e a mesma mentira do
    // filtro truncado, so que com uma noite de intervalo. Layout e gosto;
    // filtro e pergunta, e pergunta se refaz.
    //
    // Tudo em try/catch: navegador em janela anonima, ou com dado de site
    // bloqueado, LANCA no acesso -- e uma grade que nao abre por causa da
    // memoria de largura de coluna seria uma troca pessima.
    // ------------------------------------------------------------------
    var CHAVE_LAYOUT = cfg.lembrar ? "phx-grid:" + cfg.lembrar : null;
    function guardaLayout() {
      if (!CHAVE_LAYOUT) return;
      try {
        var larguras = {}, j2, c2;
        for (j2 = 0; j2 < colunasDef.length; j2++) {
          c2 = colunasDef[j2];
          if (c2.largura) larguras[c2.campo] = c2.largura;
        }
        var fixas = {};
        for (j2 = 0; j2 < colunasDef.length; j2++) if (colunasDef[j2].fixa) fixas[colunasDef[j2].campo] = colunasDef[j2].fixa;
        root.localStorage.setItem(CHAVE_LAYOUT, JSON.stringify({
          v: 1, ordem: ordemColunas.slice(), ocultas: ocultas,
          larguras: larguras, fixas: fixas, tamanho: estado.tamanho
        }));
      } catch (e) { /* sem memoria de layout; a grade continua inteira */ }
    }
    function leLayout() {
      if (!CHAVE_LAYOUT) return null;
      try {
        var cru = root.localStorage.getItem(CHAVE_LAYOUT);
        if (!cru) return null;
        var o = JSON.parse(cru);
        return o && o.v === 1 ? o : null;
      } catch (e) { return null; }
    }

    var temSelecao = !!cfg.selecao;
    var chaveCampo = cfg.chave || null;
    var selecionadas = {};
    var nSel = 0;
    var ancoraSel = -1;
    function chaveDe(linha, ix) { return chaveCampo ? String(linha[chaveCampo]) : "#" + ix; }

    var filtros = {};
    var grupos = [];
    var recolhidos = {};
    // Direcao por CAMPO e nao por posicao: arrastar a pilula para outro lugar
    // nao pode virar a ordem de quem ficou no lugar dela.
    var dirsGrupo = {};
    var rodapeGrupo = cfg.rodapeGrupo !== false;
    function agrupada(campo) { var j2; for (j2 = 0; j2 < grupos.length; j2++) if (grupos[j2] === campo) return true; return false; }
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
      pagina: 1,
      tamanho: (cfg.pagina && cfg.pagina.tamanho) || 100,
      total: 0
    };
    var opcoesTam = (cfg.pagina && cfg.pagina.opcoes) || [50, 100, 200];

    // Aplica o que ficou guardado ANTES de a grade se desenhar: o cabecalho e
    // a lista de itens por pagina ja nascem arrumados, e nao ha o pisca de
    // desenhar do jeito padrao para reorganizar em seguida.
    (function () {
      var g = leLayout();
      if (!g) return;
      var j2, c2;
      if (g.ocultas) for (j2 in g.ocultas) if (porCampo[j2]) ocultas[j2] = true;
      if (g.larguras) for (j2 in g.larguras) if (porCampo[j2]) porCampo[j2].largura = g.larguras[j2];
      if (g.fixas) for (j2 in g.fixas) if (porCampo[j2]) porCampo[j2].fixa = g.fixas[j2];
      // Coluna guardada que nao existe mais e coluna que a tabela perdeu: o
      // layout velho nao pode ressuscita-la nem derrubar a grade.
      if (g.ordem && g.ordem.length) {
        var peso = {}, n = 0;
        for (j2 = 0; j2 < g.ordem.length; j2++) if (porCampo[g.ordem[j2]]) peso[g.ordem[j2]] = n++;
        (function reordena(no2) {
          var k2;
          for (k2 = 0; k2 < no2.filhos.length; k2++) if (!no2.filhos[k2].campo) reordena(no2.filhos[k2]);
          // Ordenacao estavel por peso: quem nao esta no layout guardado fica
          // onde estava, atras de quem esta.
          var comIx = [];
          for (k2 = 0; k2 < no2.filhos.length; k2++) comIx.push({ f: no2.filhos[k2], ix: k2 });
          comIx.sort(function (a, b) {
            var pa = a.f.campo != null && peso[a.f.campo] != null ? peso[a.f.campo] : 1e9;
            var pb = b.f.campo != null && peso[b.f.campo] != null ? peso[b.f.campo] : 1e9;
            return pa - pb || a.ix - b.ix;
          });
          for (k2 = 0; k2 < comIx.length; k2++) no2.filhos[k2] = comIx[k2].f;
        })(arvore);
        ordemColunas = flatten(arvore, []);
      }
      for (j2 = 0; j2 < opcoesTam.length; j2++) if (opcoesTam[j2] === g.tamanho) estado.tamanho = g.tamanho;
      c2 = null;
    })();

    var wrap = el("div", "phx-grid");
    wrap.innerHTML =
      '<div class="phx-envoltorio"><table class="phx-tabela">' +
      '<thead></thead><tbody></tbody></table></div>' +
      '<div class="phx-rodape">' +
      '<div class="phx-pag"></div>' +
      '<div class="phx-rodape-dir">' +
      '<select class="phx-tam"></select><span class="phx-tam-rotulo">' +
      esc(txt("tela.gr_itens_por_pagina", "itens por página")) + "</span>" +
      '<span class="phx-mostrando"></span>' +
      '<span class="phx-colsel-envoltorio"><button type="button" class="phx-colsel-btn"></button>' +
      '<div class="phx-colsel" hidden></div></span>' +
      (cfg.exportarVista === false ? "" :
        '<button type="button" class="phx-exp-btn" title="' +
        esc(txt("tela.gr_exportar_dica", "baixa o que está na tela: estas colunas, este filtro, esta ordem")) +
        '">' + esc(txt("tela.gr_exportar_vista", "⤓ Exportar a vista")) + "</button>") +
      "</div></div>";
    var LIMITE_LISTA_EXCEL = 500;
    function valoresDistintos(campo) {
      if (!fonte.local) return { remoto: true, itens: [], temNulos: false, total: 0 };
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
      return { remoto: false, itens: ordem2, temNulos: temNulos, total: ordem2.length };
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
      buscaContaEl.textContent = filtros["*"]
        ? preencher(txt("tela.gr_resultados", "{n} resultado(s)"), { n: fmt.numero(estado.total) })
        : "";
    }
    function montaBusca() {
      if (!cfg.buscaGlobal) return;
      var bb = el("div", "phx-busca");
      bb.innerHTML = '<input type="text" class="phx-busca-in" placeholder="' +
        esc(txt("tela.gr_busca_tudo", "Buscar em tudo\u2026 (v\u00e1rios termos = E)")) +
        '"><span class="phx-busca-conta"></span>';
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
        groupBox.innerHTML = '<span class="phx-groupbox-dica">' +
          esc(txt("tela.gr_arraste", "Arraste uma coluna para c\u00e1 para agrupar")) + "</span>";
        return;
      }
      var html = "", j2;
      for (j2 = 0; j2 < grupos.length; j2++) {
        var cP = porCampo[grupos[j2]], dP = dirsGrupo[grupos[j2]] || "asc";
        html += '<span class="phx-gpill" draggable="true" data-campo="' + esc(grupos[j2]) + '">' +
          '<button type="button" class="phx-gpill-dir" title="' +
            esc(dP === "asc" ? txt("tela.gr_crescente", "crescente \u2014 clique para inverter")
                             : txt("tela.gr_decrescente", "decrescente \u2014 clique para inverter")) + '">' +
            (dP === "asc" ? "\u2191" : "\u2193") + "</button>" +
          esc((cP && cP.titulo) || grupos[j2]) +
          ' <button type="button" class="phx-gpill-x" title="' +
          esc(txt("tela.gr_desagrupar", "desagrupar")) + '">\u00d7</button></span>';
        if (j2 < grupos.length - 1) html += '<span class="phx-gpill-seta">\u2192</span>';
      }
      html += '<span class="phx-gbox-acoes">' +
        '<button type="button" class="phx-gbox-bt" data-todos="abrir">' +
        esc(txt("tela.gr_expandir_tudo", "expandir tudo")) + "</button>" +
        '<button type="button" class="phx-gbox-bt" data-todos="fechar">' +
        esc(txt("tela.gr_recolher_tudo", "recolher tudo")) + "</button>" +
        '<button type="button" class="phx-gbox-bt' + (rodapeGrupo ? " phx-gbox-bt-on" : "") +
          '" data-rodape="1" title="' +
          esc(txt("tela.gr_total_grupo_dica", "mostra o total embaixo de cada grupo")) + '">' +
          esc(txt("tela.gr_total_por_grupo", "total por grupo")) + "</button>" +
        "</span>";
      groupBox.innerHTML = html;
      var acs = groupBox.querySelectorAll("[data-todos]"), ja;
      for (ja = 0; ja < acs.length; ja++) {
        (function (bt) {
          bt.addEventListener("click", function () {
            api.expandirTodos(bt.getAttribute("data-todos") === "abrir");
          });
        })(acs[ja]);
      }
      var btR = groupBox.querySelector("[data-rodape]");
      if (btR) btR.addEventListener("click", function () {
        rodapeGrupo = !rodapeGrupo;
        log("rodapegrupo", { ligado: rodapeGrupo });
        renderGroupBox(); carrega();
      });
      var pills = groupBox.querySelectorAll(".phx-gpill");
      for (j2 = 0; j2 < pills.length; j2++) {
        (function (pill) {
          pill.querySelector(".phx-gpill-dir").addEventListener("click", function () {
            var cmp = pill.getAttribute("data-campo");
            dirsGrupo[cmp] = (dirsGrupo[cmp] || "asc") === "asc" ? "desc" : "asc";
            log("ordemgrupo", { campo: cmp, dir: dirsGrupo[cmp] });
            renderGroupBox(); carrega();
          });
          pill.querySelector(".phx-gpill-x").addEventListener("click", function () {
            var novo = [], j3;
            for (j3 = 0; j3 < grupos.length; j3++) if (grupos[j3] !== pill.getAttribute("data-campo")) novo.push(grupos[j3]);
            api.agrupar(novo);
          });
          pill.addEventListener("dragstart", function (e) { e.dataTransfer.setData("text/plain", "\u0002pill:" + pill.getAttribute("data-campo")); });
          pill.addEventListener("dragover", function (e) { e.preventDefault(); });
          pill.addEventListener("drop", function (e) {
            e.preventDefault();
            e.stopPropagation();
            var d2 = e.dataTransfer.getData("text/plain");
            if (d2.indexOf("\u0002pill:") !== 0) return;
            var de = d2.slice(6), para = pill.getAttribute("data-campo");
            if (de === para) return;
            var novo = [], j3;
            for (j3 = 0; j3 < grupos.length; j3++) if (grupos[j3] !== de) novo.push(grupos[j3]);
            var ixP = -1;
            for (j3 = 0; j3 < novo.length; j3++) if (novo[j3] === para) { ixP = j3; break; }
            novo.splice(ixP, 0, de);
            api.agrupar(novo);
          });
        })(pills[j2]);
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
      var html = '<span class="phx-filtros-conta">' +
        esc(preencher(txt("tela.gr_filtros_ativos", "Filtros Ativos ({n})"), { n: campos.length })) + "</span>";
      for (j2 = 0; j2 < campos.length; j2++) {
        html += '<span class="phx-chip" data-campo="' + esc(campos[j2]) + '">' + esc(resumoFiltro(campos[j2], filtros[campos[j2]])) +
          ' <button type="button" class="phx-chip-x" title="' +
          esc(txt("tela.gr_remover", "remover")) + '">\u00d7</button></span>';
      }
      html += '<button type="button" class="phx-filtros-limpar">' +
        esc(txt("tela.gr_limpar_todos", "Limpar Todos")) + "</button>";
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
    document.addEventListener("mousedown", function (e) {
      if (!fpop.hidden && !fpop.contains(e.target) && !(e.target.className && String(e.target.className).indexOf("phx-fbtn") >= 0)) fechaFpop();
    });
    document.addEventListener("keydown", function (e) { if (e.keyCode === 27) fechaFpop(); });
    function abreFiltroExcel(campo, ancora) {
      if (fpopCampo === campo && !fpop.hidden) { fechaFpop(); return; }
      fpopCampo = campo;
      var c2 = porCampo[campo];
      var dist = valoresDistintos(campo);
      var atual = filtros[campo];
      var ehNum = c2.tipo === "numero" || c2.tipo === "moeda" || c2.tipo === "percentual";
      var multiIni = (atual && atual.tipo === "multi") ? atual : (atual && atual.tipo === "expr" ? { combinador: "e", condicoes: [{ op: atual.op, valor: atual.valor }] } : null);
      var combIni = multiIni ? multiIni.combinador : "e";
      var OPS_NUM = [
        [">",  txt("tela.gr_op_maior", "\u00e9 maior que")],
        [">=", txt("tela.gr_op_maior_ig", "\u00e9 maior ou igual a")],
        ["<",  txt("tela.gr_op_menor", "\u00e9 menor que")],
        ["<=", txt("tela.gr_op_menor_ig", "\u00e9 menor ou igual a")],
        ["=",  txt("tela.gr_op_igual", "\u00e9 igual a")],
        ["!=", txt("tela.gr_op_diferente", "\u00e9 diferente de")],
      ];
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
          rodT.textContent = preencher(
            txt("tela.gr_mostrando", "mostrando {vis} de {total} \u2014 refine a pesquisa"),
            { vis: fmt.numero(vis.length), total: fmt.numero(dist.total) });
          rodT.hidden = false;
        } else rodT.hidden = true;
      }
      fpop.innerHTML =
        '<button type="button" class="phx-fpop-acao" data-a="az">' +
        esc(txt("tela.gr_ordenar_az", "Classificar de A a Z")) + "</button>" +
        '<button type="button" class="phx-fpop-acao" data-a="za">' +
        esc(txt("tela.gr_ordenar_za", "Classificar de Z a A")) + "</button>" +
        '<button type="button" class="phx-fpop-acao" data-a="limpar">' +
        esc(txt("tela.gr_limpar_filtro", "Limpar Filtro")) + "</button>" +
        '<div class="phx-fpop-sep"></div>' +
        (dist.remoto ? '<div class="phx-fpop-aviso">' +
          esc(txt("tela.gr_sem_distintos", "fonte remota sem suporte a valores distintos")) + "</div>" :
        '<input type="text" class="phx-fpop-busca" placeholder="' +
          esc(txt("tela.gr_pesquisar", "Pesquisar")) + '">' +
        '<label class="phx-fpop-tudo"><input type="checkbox"> ' +
          esc(txt("tela.gr_selecionar_tudo", "(Selecionar Tudo)")) + "</label>" +
        '<div class="phx-fpop-lista"></div>' +
        '<div class="phx-fpop-trunc" hidden></div>' +
        (dist.temNulos ? '<label class="phx-fpop-nulos"><input type="checkbox"' + (incluiNulos ? " checked" : "") + "> " +
          esc(txt("tela.gr_exibir_sem_valor", "Exibir itens sem valor")) + "</label>" : "")) +
        (ehNum ? '<div class="phx-fpop-sep"></div><div class="phx-fpop-numtit">' +
          esc(txt("tela.gr_filtros_numero", "Filtros de N\u00famero")) + "</div>" + linhaNum(0) +
          '<div class="phx-fpop-comb"><label><input type="radio" name="phxcomb" value="e"' + (combIni !== "ou" ? " checked" : "") + "> " +
          esc(txt("tela.gr_e", "E")) + "</label>" +
          '<label><input type="radio" name="phxcomb" value="ou"' + (combIni === "ou" ? " checked" : "") + "> " +
          esc(txt("tela.gr_ou", "OU")) + "</label></div>" + linhaNum(1) : "") +
        '<div class="phx-fpop-rodape"><button type="button" class="phx-fpop-ok">OK</button>' +
        '<button type="button" class="phx-fpop-cancela">' + esc(txt("tela.cancelar", "Cancelar")) + "</button></div>";
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
    }
    var estiloFixas = document.createElement("style");
    wrap.appendChild(estiloFixas);
    var envoltorio = wrap.querySelector(".phx-envoltorio");
    var roladoFlag = false;
    envoltorio.addEventListener("scroll", function () {
      var r2 = envoltorio.scrollLeft > 0;
      if (r2 === roladoFlag) return;
      roladoFlag = r2;
      if (r2) wrap.className += " phx-rolado";
      else wrap.className = wrap.className.replace(" phx-rolado", "");
    });
    var thead = wrap.querySelector("thead");
    var tbody = wrap.querySelector("tbody");
    var popover = el("div", "phx-popover");
    popover.hidden = true;
    wrap.appendChild(popover);
    var popoverDe = null;
    function fechaPopover() { popover.hidden = true; popoverDe = null; }
    // ABRIR A LINHA. Duplo clique e nao clique simples de proposito: o clique
    // simples ja e da selecao, e uma grade que navega ao primeiro toque
    // atrapalha quem so queria marcar. Quem recebe a linha decide o que fazer
    // com ela -- a grade nao sabe editar, e nao e ela que deve saber: a ficha
    // do console ja carrega a versao do slot e recusa escrita concorrente.
    if (cfg.aoAbrirLinha) tbody.addEventListener("dblclick", function (e) {
      var tr = e.target.closest ? e.target.closest("tr") : null;
      if (!tr || !tr.parentNode) return;
      var ix = -1, kids = tbody.children, j2;
      for (j2 = 0; j2 < kids.length; j2++) if (kids[j2] === tr) { ix = j2; break; }
      var l = ultimaCarga && ultimaCarga.linhas[ix];
      if (!l || eMarcador(l)) return;
      log("abrirlinha", { linha: ix });
      cfg.aoAbrirLinha(l, ix);
    });
    tbody.addEventListener("click", function (e) {
      var alvoEl = e.target;
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
      if (!temSelecao) return;
      var tdSel = alvoEl.closest ? alvoEl.closest(".phx-td-sel") : null;
      if (!tdSel) return;
      var ix = parseInt(tdSel.getAttribute("data-ls"), 10);
      if (e.shiftKey && ancoraSel >= 0) {
        var a2 = Math.min(ancoraSel, ix), b2 = Math.max(ancoraSel, ix), j2;
        // A faixa pula cabecalho e rodape de grupo: com agrupamento ligado
        // eles caem no meio do intervalo, e marcar um poria no conjunto uma
        // chave que nao existe no dado.
        for (j2 = a2; j2 <= b2; j2++) if (!eMarcador(ultimaCarga.linhas[j2])) alternaLinha(j2, true);
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

    function visiveis() {
      var v = [], j;
      for (j = 0; j < ordemColunas.length; j++) if (!ocultas[ordemColunas[j]] && !agrupada(ordemColunas[j])) v.push(porCampo[ordemColunas[j]]);
      return v;
    }
    function hrefSeguro(u) {
      u = String(u == null ? "#" : u);
      return (/^https?:\/\//i.test(u) || u.charAt(0) === "#") ? u : "#";
    }
    var CORES_BADGE = { verde: 1, ambar: 1, azul: 1, vermelho: 1, cinza: 1 };
    function formata(c, v, linha, ixL) {
      if (c.formato) return c.formato(v, linha);
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

    var CICLO_AGG = ["sum", "avg", "count", "min", "max"];
    function thColuna(c, rowspan) {
      var th = el("th", "phx-th");
      th.setAttribute("data-campo", c.campo);
      if (rowspan > 1) th.rowSpan = rowspan;
      if (c.largura) th.style.width = c.largura + "px";
      if (c.fixa === "esq") th.className += " phx-fixa-esq";
      if (c.fixa === "dir") th.className += " phx-fixa-dir";
      var ind = estado.ordem.campo === c.campo ? (estado.ordem.dir === "asc" ? " \u25b2" : " \u25bc") : "";
      th.innerHTML =
        '<span class="phx-th-titulo">' + esc(rotulo(c)) + '<span class="phx-sort-ind">' + ind + "</span></span>" +
        (c.dimensao ? '<span class="phx-th-dim">(' + esc(c.dimensao) + ")</span>" : "") +
        (c.agregador ? '<button type="button" class="phx-th-agg" title="' +
          esc(txt("tela.gr_alternar_agregador", "alternar agregador")) + '">' + esc(c.agregador.toUpperCase()) + "</button>" : "") +
        (c.filtravel !== false ? '<button type="button" class="phx-fbtn' + (filtros[c.campo] ? " phx-fbtn-on" : "") +
          '" title="' + esc(txt("tela.gr_filtrar", "filtrar")) + '">\u25bc</button>' : "") +
        '<span class="phx-col-resz"></span>';
      if (c.ordenavel !== false) {
        th.className += " phx-ordenavel";
        th.querySelector(".phx-th-titulo").addEventListener("click", function () { alternaOrdem(c.campo); });
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
      });
      th.draggable = true;
      th.addEventListener("dragstart", function (e) { e.dataTransfer.setData("text/plain", c.campo); });
      th.addEventListener("dragover", function (e) { e.preventDefault(); });
      th.addEventListener("drop", function (e) {
        e.preventDefault();
        var de = e.dataTransfer.getData("text/plain");
        if (de && de !== c.campo) moverColunaAntes(de, c.campo);
      });
      var rz = th.querySelector(".phx-col-resz");
      rz.addEventListener("mousedown", function (e) {
        e.preventDefault(); e.stopPropagation();
        var x0 = e.clientX, w0 = th.offsetWidth;
        function mv(ev2) { var w = Math.max(50, w0 + ev2.clientX - x0); th.style.width = w + "px"; porCampo[c.campo].largura = w; }
        function up() { document.removeEventListener("mousemove", mv); document.removeEventListener("mouseup", up); log("resize", { campo: c.campo, largura: porCampo[c.campo].largura }); mideFixas(); guardaLayout(); }
        document.addEventListener("mousemove", mv);
        document.addEventListener("mouseup", up);
      });
      return th;
    }
    function montaHeader() {
      var D = profundidade(arvore) - 1;
      var linhas2 = [], d2;
      for (d2 = 0; d2 <= D; d2++) linhas2.push(el("tr"));
      function anda(no2, nivel) {
        var j2, filho, vis;
        for (j2 = 0; j2 < no2.filhos.length; j2++) {
          filho = no2.filhos[j2];
          if (filho.campo) {
            if (!ocultas[filho.campo] && !agrupada(filho.campo)) linhas2[nivel].appendChild(thColuna(porCampo[filho.campo], D - nivel + 1));
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
      if (temSelecao) {
        var thS = el("th", "phx-th phx-td-sel");
        thS.rowSpan = D + 1;
        thS.innerHTML = '<input type="checkbox" class="phx-sel-mestre" tabindex="-1">';
        thS.querySelector("input").addEventListener("click", function () {
          var linhas = ultimaCarga ? ultimaCarga.linhas : [], j2;
          var marcar = this.checked;
          // Cabecalho e rodape de grupo nao sao linha para marcar: a chave
          // deles nao existe no dado, e marcar um deles poria lixo na selecao.
          for (j2 = 0; j2 < linhas.length; j2++) if (!eMarcador(linhas[j2])) alternaLinha(j2, marcar);
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
    // Os campos daqui levam `size` pequeno de proposito. Numa tabela de
    // layout automatico e a largura INTRINSECA do controle que decide a
    // largura da coluna, e o padrao de um `<input>` (size=20, ~170 px)
    // engordava CADA coluna para 237 px so por causa da caixa de filtro --
    // medido: o `rowid`, que pede 90 px, ficava mais largo que o `pedido`.
    // Com `size` pequeno e `width:100%` a caixa encolhe ate o que a coluna
    // pede e cresce junto com ela.
    function montaFilterRow() {
      var tr = el("tr", "phx-frow"), v = visiveis(), j2, c2, td2, tipoC;
      if (temSelecao) tr.appendChild(el("th", "phx-th phx-frow-cel phx-td-sel", ""));
      for (j2 = 0; j2 < v.length; j2++) {
        c2 = v[j2];
        tipoC = c2.tipo || "texto";
        td2 = el("th", "phx-th phx-frow-cel");
        td2.setAttribute("data-campo", c2.campo);
        if (c2.filtravel === false || tipoC === "json" || tipoC === "barra") {
          tr.appendChild(td2);
          continue;
        }
        if (tipoC === "numero" || tipoC === "moeda" || tipoC === "percentual") {
          td2.innerHTML = '<span class="phx-frow-num"><select class="phx-frow-op"><option>&gt;</option><option>&gt;=</option><option>&lt;</option><option>&lt;=</option><option>=</option><option>!=</option></select><input type="number" step="any" size="5" class="phx-frow-in" placeholder="' + esc(txt("tela.gr_valor", "valor")) + '"></span>';
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
          var dist2 = valoresDistintos(c2.campo), hOp = '<select class="phx-frow-in phx-frow-sel"><option value="">' +
            esc(txt("tela.gr_selecionar", "Selecionar")) + "</option>", j3;
          for (j3 = 0; j3 < dist2.itens.length && j3 < 50; j3++) hOp += "<option>" + esc(dist2.itens[j3].chave) + "</option>";
          td2.innerHTML = hOp + "</select>";
          (function (campo, cel) {
            cel.querySelector("select").addEventListener("change", function () {
              api.filtrar(campo, this.value ? { tipo: "valores", valores: [this.value] } : null);
            });
          })(c2.campo, td2);
        } else {
          td2.innerHTML = '<input type="text" size="6" class="phx-frow-in" placeholder="' +
            esc(txt("tela.gr_buscar_curto", "Buscar\u2026")) + '">';
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

    function alternaOrdem(campo) {
      var t1 = agora();
      if (estado.ordem.campo !== campo) { estado.ordem = { campo: campo, dir: "asc" }; }
      else if (estado.ordem.dir === "asc") estado.ordem.dir = "desc";
      else estado.ordem = { campo: null, dir: null };
      estado.pagina = 1;
      carrega(function () { log("sort", { campo: campo, dir: estado.ordem.dir, ms: Math.round((agora() - t1) * 10) / 10 }); });
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
      montaHeader(); montaCorpo(ultimaCarga ? ultimaCarga.linhas : []);
      montaColSel();
      guardaLayout();
    }

    var ultimaCarga = null;
    var perfRender = {};
    function montaCorpo(linhas) {
      var tA = agora();
      var v = visiveis(), html = "", j, k, c, val, ch, nCols = v.length + (temSelecao ? 1 : 0);
      for (j = 0; j < linhas.length; j++) {
        if (linhas[j].__grupo) {
          var gN = linhas[j].__grupo, cG = porCampo[gN.campo];
          var abertoG = !recolhidos[gN.path];
          var resumoA = [], ka;
          for (ka in gN.aggs) {
            var cA = porCampo[ka];
            resumoA.push((cA.titulo || ka) + ": " + formata(cA, gN.aggs[ka], {}, 0));
          }
          html += '<tr class="phx-grupo" data-gpath="' + esc(gN.path) + '"><td class="phx-td phx-grupo-td" colspan="' + nCols + '" style="padding-left:' + (10 + gN.nivel * 22) + 'px">' +
            '<span class="phx-grupo-caret">' + (abertoG ? "\u25be" : "\u25b8") + "</span>" +
            '<span class="phx-grupo-rotulo">' + esc((cG && cG.titulo) || gN.campo) + ": " + formata(cG, gN.valor, {}, 0) + "</span>" +
            '<span class="phx-grupo-conta">(' + fmt.numero(gN.n) + ")</span>" +
            (resumoA.length ? '<span class="phx-grupo-aggs">' + resumoA.join(" \u00b7 ") + "</span>" : "") +
            "</td></tr>";
          continue;
        }
        if (linhas[j].__rodape) {
          // O rodape alinha o agregado NA COLUNA dele, e nao numa tira de
          // texto: e assim que se compara um total com os valores acima.
          var rN = linhas[j].__rodape;
          html += '<tr class="phx-grodape">';
          if (temSelecao) html += '<td class="phx-td"></td>';
          for (k = 0; k < v.length; k++) {
            c = v[k];
            var temAgg = Object.prototype.hasOwnProperty.call(rN.aggs, c.campo);
            html += '<td class="phx-td phx-tipo-' + (c.tipo || "texto") + '">' +
              (k === 0
                ? '<span class="phx-grodape-rot">total de ' +
                    esc(formata(porCampo[rN.campo], rN.valor, {}, 0)) + "</span>"
                : temAgg ? formata(c, rN.aggs[c.campo], {}, 0) : "") +
              "</td>";
          }
          html += "</tr>";
          continue;
        }
        html += "<tr>";
        if (temSelecao) {
          ch = selecionadas[chaveDe(linhas[j], j)] ? " checked" : "";
          html += '<td class="phx-td phx-td-sel" data-ls="' + j + '"><input type="checkbox" tabindex="-1"' + ch + "></td>";
        }
        for (k = 0; k < v.length; k++) {
          c = v[k]; val = linhas[j][c.campo];
          html += '<td class="phx-td phx-tipo-' + (c.tipo || "texto") + '"' +
            (c.fixa ? ' data-fx="' + esc(c.campo) + '"' : "") + ">" + formata(c, val, linhas[j], j) + "</td>";
        }
        html += "</tr>";
      }
      // O total geral vem do conjunto filtrado inteiro, e por isso fica numa
      // linha propria, presa embaixo -- nao muda ao virar a pagina.
      var tg = ultimaCarga && ultimaCarga.totaisGerais;
      var temTG = false, kg;
      if (tg) for (kg in tg) { temTG = true; break; }
      if (temTG) {
        html += '<tr class="phx-total-geral">';
        if (temSelecao) html += '<td class="phx-td"></td>';
        for (k = 0; k < v.length; k++) {
          c = v[k];
          var tA2 = Object.prototype.hasOwnProperty.call(tg, c.campo);
          html += '<td class="phx-td phx-tipo-' + (c.tipo || "texto") + '">' +
            (k === 0 ? '<span class="phx-grodape-rot">' + esc(txt("tela.gr_total_geral", "total geral")) + "</span>"
                     : tA2 ? formata(c, tg[c.campo], {}, 0) : "") +
            "</td>";
        }
        html += "</tr>";
      }
      var tB = agora();
      tbody.innerHTML = html;
      var tC = agora();
      atualizaMestre();
      perfRender.strMs = Math.round((tB - tA) * 100) / 100;
      perfRender.domMs = Math.round((tC - tB) * 100) / 100;
      perfRender.mestreMs = Math.round((agora() - tC) * 100) / 100;
    }
    function atualizaMestre() {
      if (!temSelecao) return;
      var mestre = thead.querySelector(".phx-sel-mestre");
      if (!mestre) return;
      // Conta so o DADO. Com o agrupamento ligado a pagina traz cabecalho e
      // rodape de grupo no meio das linhas, e conta-los faria o "marcar todas"
      // nunca fechar: seriam sempre menos marcadas que linhas.
      var linhas = ultimaCarga ? ultimaCarga.linhas : [], j, n = 0, dados = 0;
      for (j = 0; j < linhas.length; j++) {
        if (eMarcador(linhas[j])) continue;
        dados++;
        if (selecionadas[chaveDe(linhas[j], j)]) n++;
      }
      mestre.checked = dados > 0 && n === dados;
      mestre.indeterminate = n > 0 && n < dados;
    }
    function alternaLinha(ix, forcar) {
      var linhas = ultimaCarga.linhas, k2 = chaveDe(linhas[ix], ix);
      var novo = forcar != null ? forcar : !selecionadas[k2];
      if (novo && !selecionadas[k2]) { selecionadas[k2] = true; nSel++; }
      else if (!novo && selecionadas[k2]) { delete selecionadas[k2]; nSel--; }
      var td2 = tbody.querySelector('td[data-ls="' + ix + '"] input');
      if (td2) td2.checked = !!novo;
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
      itens.push('<span class="phx-pg-irpara">' + esc(txt("tela.gr_ir_para", "ir para")) +
        ' <input type="number" min="1" max="' + tp + '" value="' + p + '"></span>');
      var tP = agora();
      pagEl.innerHTML = esc(preencher(
        txt("tela.gr_pagina_de", "Página {p} de {tp} ({n} registros)"),
        { p: p, tp: fmt.numero(tp), n: fmt.numero(estado.total) })) + " " + itens.join("");
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
        mostrandoEl.textContent = preencher(
          txt("tela.gr_linhas_em_grupos", "{linhas} linhas em {niveis} n\u00edvel(is) de grupo"),
          { linhas: fmt.numero(ultimaCarga.totalDados), niveis: grupos.length });
      else
        mostrandoEl.textContent = preencher(
          txt("tela.gr_mostrando_de_ate", "Mostrando {de}\u2013{ate} de {total}"),
          { de: fmt.numero(ini2), ate: fmt.numero(fim2), total: fmt.numero(estado.total) });
      atualizaContaBusca();
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
      guardaLayout();
      carrega(function () { log("pagesize", { tamanho: estado.tamanho }); }, true);
    });

    function montaColSel() {
      var vis = visiveis().length;
      colBtn.textContent = preencher(txt("tela.gr_colunas_conta", "Colunas: {n} ▾"), { n: vis });
      var html = "", j, c;
      for (j = 0; j < ordemColunas.length; j++) {
        c = porCampo[ordemColunas[j]];
        // O alfinete mora AQUI, e nao no cabecalho, por um motivo pratico:
        // congelar serve para nao perder de vista a coluna que identifica a
        // linha, e essa coluna costuma estar na esquerda -- ja fora da tela
        // quando se rola para a direita e daria falta do botao.
        html += '<div class="phx-colsel-linha">' +
          '<button type="button" class="phx-colsel-pino' + (c.fixa === "esq" ? " phx-colsel-pino-on" : "") +
            '" data-pino="' + esc(c.campo) + '" title="' +
            esc(c.fixa === "esq" ? txt("tela.gr_congelada", "congelada — clique para soltar")
                                 : txt("tela.gr_congelar", "congelar à esquerda")) +
            '">◧</button>' +
          '<label class="phx-colsel-item"><input type="checkbox" data-campo="' + esc(c.campo) + '"' +
          (ocultas[c.campo] ? "" : " checked") + "> " + esc(c.titulo || c.campo) + "</label></div>";
      }
      colMenu.innerHTML = html;
      var cbs = colMenu.querySelectorAll("input"), k2;
      for (k2 = 0; k2 < cbs.length; k2++) {
        (function (cb) {
          cb.addEventListener("change", function () {
            mostrarColuna(cb.getAttribute("data-campo"), cb.checked);
          });
        })(cbs[k2]);
      }
      var pinos = colMenu.querySelectorAll("[data-pino]");
      for (k2 = 0; k2 < pinos.length; k2++) {
        (function (bt) {
          bt.addEventListener("click", function (e) {
            e.stopPropagation();
            var cmp = bt.getAttribute("data-pino");
            congelar(cmp, porCampo[cmp].fixa === "esq" ? null : "esq");
          });
        })(pinos[k2]);
      }
    }
    colBtn.addEventListener("click", function () { colMenu.hidden = !colMenu.hidden; });
    function mostrarColuna(campo, mostrar) {
      if (mostrar) delete ocultas[campo]; else ocultas[campo] = true;
      log("coluna", { campo: campo, visivel: !!mostrar });
      montaHeader(); montaCorpo(ultimaCarga ? ultimaCarga.linhas : []); montaColSel();
      guardaLayout();
    }
    /// Congela (ou solta) uma coluna. `lado` e "esq", "dir" ou nulo.
    ///
    /// A coluna congelada vai para a PONTA da ordem, porque o `sticky` gruda
    /// no lado do contentor e nao no lugar dela: congelar a quinta coluna sem
    /// move-la faria as quatro da esquerda passarem POR BAIXO dela.
    function congelar(campo, lado) {
      var c = porCampo[campo];
      if (!c) return;
      c.fixa = lado || null;
      if (lado === "esq") {
        var primeiraSolta = null, j2;
        for (j2 = 0; j2 < ordemColunas.length; j2++) {
          if (porCampo[ordemColunas[j2]].fixa !== "esq") { primeiraSolta = ordemColunas[j2]; break; }
        }
        if (primeiraSolta && primeiraSolta !== campo) moverColunaAntes(campo, primeiraSolta);
      }
      log("congelar", { campo: campo, lado: lado || "(solta)" });
      montaHeader(); montaCorpo(ultimaCarga ? ultimaCarga.linhas : []); montaColSel();
      guardaLayout();
    }

    function carrega(cb, semHeader) {
      var c = porCampo[estado.ordem.campo];
      fonte.carregar({
        pagina: estado.pagina, tamanho: estado.tamanho,
        ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c ? c.tipo : null },
        filtros: serializaFiltros(),
        grupos: grupos.slice(),
        dirsGrupo: (function () { var o3 = [], j3; for (j3 = 0; j3 < grupos.length; j3++) o3.push(dirsGrupo[grupos[j3]] || "asc"); return o3; })(),
        rodapeGrupo: rodapeGrupo,
        recolhidos: recolhidos,
        aggCols: (function () { var o3 = [], j3; for (j3 = 0; j3 < colunasDef.length; j3++) if (colunasDef[j3].agregador) o3.push({ campo: colunasDef[j3].campo, agregador: colunasDef[j3].agregador }); return o3; })(),
        tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
      }, function (err, r) {
        if (err) { log("erro", { erro: String(err) }); return; }
        if (temSelecao && !chaveCampo && ultimaCarga) { selecionadas = {}; nSel = 0; ancoraSel = -1; }
        fechaPopover();
        ultimaCarga = r;
        estado.total = r.total;
        if (!semHeader) montaHeader();
        montaCorpo(r.linhas); montaPag();
        if (cb) cb(r);
      });
    }

    // ------------------------------------------------------------------
    // EXPORTAR A VISTA.
    //
    // Nao e o mesmo que exportar a tabela, e a diferenca e o ponto: a tabela
    // sai como esta gravada, e a vista sai como a pessoa a montou -- estas
    // colunas, nesta ordem, com este filtro e esta ordenacao. Quem passou
    // vinte minutos filtrando quer levar o resultado, e nao recomecar no
    // Excel.
    //
    // Vai a fonte pedindo o CONJUNTO INTEIRO e nao a pagina: exportar a
    // pagina 1 de 40 seria a mesma mentira do filtro truncado.
    // ------------------------------------------------------------------
    function vistaAtual(cb) {
      var c = porCampo[estado.ordem.campo];
      fonte.carregar({
        pagina: 1, tamanho: Math.max(1, estado.total || 1),
        ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir, tipo: c ? c.tipo : null },
        filtros: serializaFiltros(),
        grupos: grupos.slice(),
        dirsGrupo: (function () { var o3 = [], j3; for (j3 = 0; j3 < grupos.length; j3++) o3.push(dirsGrupo[grupos[j3]] || "asc"); return o3; })(),
        rodapeGrupo: false,
        recolhidos: {},
        aggCols: [],
        tiposCampos: (function () { var o3 = {}, j3; for (j3 = 0; j3 < colunasDef.length; j3++) o3[colunasDef[j3].campo] = colunasDef[j3].tipo || "texto"; return o3; })()
      }, function (err, r) {
        if (err) { cb(err); return; }
        // Fora os marcadores: cabecalho de grupo nao e linha de dado, e uma
        // planilha com "Regiao: Sul (312)" no meio das linhas nao abre certo
        // em lugar nenhum. A ORDEM do agrupamento fica, que e o que se quis.
        var so = [], j3;
        for (j3 = 0; j3 < r.linhas.length; j3++) if (!eMarcador(r.linhas[j3])) so.push(r.linhas[j3]);
        cb(null, { colunas: visiveis(), linhas: so });
      });
    }
    function csvDaVista(v) {
      // Ponto e virgula e BOM: e o que o Excel em portugues abre sem perguntar
      // nada. Com virgula ele joga a linha inteira numa celula so.
      function cel(s) {
        s = s == null ? "" : String(s);
        return /[";\n\r]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
      }
      var linhas = [], cab = [], j2, k2, lin;
      for (j2 = 0; j2 < v.colunas.length; j2++) cab.push(cel(rotulo(v.colunas[j2])));
      linhas.push(cab.join(";"));
      for (j2 = 0; j2 < v.linhas.length; j2++) {
        lin = [];
        // O valor CRU, e nao o formatado: "R$ 1.234,56" volta como texto em
        // qualquer planilha, e ai ninguem soma a coluna.
        for (k2 = 0; k2 < v.colunas.length; k2++) lin.push(cel(v.linhas[j2][v.colunas[k2].campo]));
        linhas.push(lin.join(";"));
      }
      return "﻿" + linhas.join("\r\n") + "\r\n";
    }
    var expBtn = wrap.querySelector(".phx-exp-btn");
    if (expBtn) expBtn.addEventListener("click", function () {
      var t1 = agora();
      expBtn.disabled = true;
      vistaAtual(function (err, v) {
        expBtn.disabled = false;
        if (err) { log("erro", { erro: String(err) }); return; }
        var texto = csvDaVista(v);
        var url = URL.createObjectURL(new Blob([texto], { type: "text/csv;charset=utf-8" }));
        var a = document.createElement("a");
        a.href = url;
        a.download = (cfg.nomeVista || "vista") + ".csv";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        setTimeout(function () { URL.revokeObjectURL(url); }, 0);
        log("exportvista", { linhas: v.linhas.length, colunas: v.colunas.length, ms: Math.round((agora() - t1) * 10) / 10 });
      });
    });

    var api = {
      ok: true, el: wrap,
      ordenar: function (campo, dir) {
        estado.ordem = dir ? { campo: campo, dir: dir } : { campo: null, dir: null };
        estado.pagina = 1;
        carrega(); return api;
      },
      pagina: function (p) { if (p == null) return estado.pagina; irPagina(p); return api; },
      tamanhoPagina: function (n) {
        if (n == null) return estado.tamanho;
        estado.tamanho = n; estado.pagina = 1; tamSel.value = String(n);
        guardaLayout(); carrega(null, true); return api;
      },
      mostrarColuna: mostrarColuna,
      moverColuna: function (campo, antesDe) { moverColunaAntes(campo, antesDe); return api; },
      congelar: function (campo, lado) { congelar(campo, lado); return api; },
      congeladas: function () {
        var o = [], j2;
        for (j2 = 0; j2 < ordemColunas.length; j2++) if (porCampo[ordemColunas[j2]].fixa) o.push(ordemColunas[j2]);
        return o;
      },
      vistaAtual: vistaAtual,
      csvDaVista: csvDaVista,
      esquecerLayout: function () {
        if (CHAVE_LAYOUT) { try { root.localStorage.removeItem(CHAVE_LAYOUT); } catch (e) { /* nada a esquecer */ } }
        return api;
      },
      colunasVisiveis: function () { var v = visiveis(), o = [], j; for (j = 0; j < v.length; j++) o.push(v[j].campo); return o; },
      linhas: function () { return ultimaCarga ? ultimaCarga.linhas : []; },
      estado: function () {
        return { ordem: { campo: estado.ordem.campo, dir: estado.ordem.dir }, pagina: estado.pagina,
          tamanho: estado.tamanho, total: estado.total, colunas: api.colunasVisiveis(), selecao: nSel, filtros: serializaFiltros() };
      },
      filtrar: function (campo, condicao) {
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
      agrupar: function (campos) {
        var t1 = agora();
        grupos = (campos || []).slice();
        recolhidos = {};
        estado.pagina = 1;
        renderGroupBox();
        carrega(function () {
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
        if (abrir === false) {
          var lst = serializaFiltros(), base2 = aplicaFiltros(fonte.todos || [], lst);
          var arvT = agrupa(base2, grupos, [], function (c9) { return (porCampo[c9] && porCampo[c9].tipo) || "texto"; });
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
            montaCorpo(ultimaCarga.linhas); montaPag();
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
          if (marcar !== false) { if (!selecionadas[k3]) { selecionadas[k3] = true; nSel++; } }
          else if (selecionadas[k3]) { delete selecionadas[k3]; nSel--; }
        }
        montaCorpo(ultimaCarga ? ultimaCarga.linhas : []);
        return api;
      },
      selecionadas: function () { var o = [], k3; for (k3 in selecionadas) if (selecionadas[k3]) o.push(k3); return o; },
      limparSelecao: function () { selecionadas = {}; nSel = 0; ancoraSel = -1; montaCorpo(ultimaCarga ? ultimaCarga.linhas : []); return api; },
      logs: function () { return logs.slice(); },
      _scoreBusca: function (linha) { var f9 = null, l9 = serializaFiltros(), j9; for (j9 = 0; j9 < l9.length; j9++) if (l9[j9].tipo === "busca") f9 = l9[j9]; return f9 ? scoreBusca(linha, f9) : 0; },
      _ultimoRender: function () { return { strMs: perfRender.strMs, domMs: perfRender.domMs, mestreMs: perfRender.mestreMs, pagMs: perfRender.pagMs, fonteMs: ultimaCarga && ultimaCarga._ms }; },
      redesenhar: function () {
        // `redesenhar` quer dizer «o dado mudou», e nao so «pinte de novo»:
        // por isso ele derruba o cache da fonte antes de recarregar.
        if (fonte.invalidar) fonte.invalidar();
        carrega(null, true);
        return api;
      },
      destruir: function () { if (wrap.parentNode) wrap.parentNode.removeChild(wrap); }
    };

    no.appendChild(wrap);
    montaGroupBox();
    montaBusca();
    // O menu de Colunas so era montado por `moverColunaAntes` e por
    // `mostrarColuna` -- isto e, DEPOIS de alguem ja ter mexido nele. Aberto,
    // ele vinha vazio e o botao vinha sem texto (um quadradinho em branco no
    // rodape), entao esconder coluna nunca funcionou por aqui. Ler o codigo
    // nao mostra isso: as duas chamadas existem, e parecem bastar.
    montaColSel();
    carrega(function () {
      log("init", { linhas: estado.total, colunas: colunasDef.length, ms: Math.round((agora() - t0init) * 10) / 10 });
    });
    return api;
  }

  var PhxGrid = { versao: "0.9.2", criar: criar, fmt: fmt, _ordenaEstavel: ordenaEstavel };
  if (typeof module !== "undefined" && module.exports) module.exports = PhxGrid;
  root.PhxGrid = PhxGrid;
})(typeof window !== "undefined" ? window : this);

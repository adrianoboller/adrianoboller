/* O funil do HTML: a UNICA porta de texto para `innerHTML` (pedido 771).
 *
 * A CSP da pagina declara `require-trusted-types-for 'script'` e
 * `trusted-types phx`. Com isso o navegador (Chromium) RECUSA atribuir texto
 * a `innerHTML`/`outerHTML`/`insertAdjacentHTML`/`document.write` -- so
 * aceita um `TrustedHTML`, e o unico jeito de fabricar um e a politica `phx`,
 * que nasce AQUI, uma vez, e nao sai deste fecho. Ninguem cria outra (a
 * diretiva so autoriza o nome `phx`, e sem `'allow-duplicates'` o segundo
 * `createPolicy('phx')` falha), e a politica nao sabe fazer script nem
 * endereco de script: sem `createScript`/`createScriptURL`, `eval` de texto,
 * `setTimeout("…")` e `script.src = …` morrem no navegador.
 *
 * Por que uma politica que CONFERE, e nao uma que deixa tudo passar: uma
 * politica `s => s` so muda o caminho do texto, e o veneno chega ao mesmo
 * lugar. Esta le o HTML como o analisador do navegador leria (os mesmos
 * estados de etiqueta e atributo da norma, com o MESMO espaco em branco --
 * `\t \n \f \r` e o espaco, e nao o `\s` do JS) e RECUSA, jogando erro:
 *   - manipulador em atributo (`on…`), `srcdoc`, `formaction`;
 *   - elemento que roda, carrega ou muda o modo do analisador (`script`,
 *     `style`, `iframe`, `object`, `noscript`, `template`, `math`, as
 *     animacoes do SVG que reescrevem `href`…);
 *   - endereco que nao e relativo, `http(s):`, `mailto:`, `blob:` ou, so na
 *     `<img src>`, `data:image/…` -- inclusive escondido por referencia de
 *     caractere (`&#x6a;avascript:`, `&colon;`);
 *   - `<!…>`/`<?…>` e `</` torto, que o analisador trata como comentario e
 *     onde um leitor diferente do dele perderia o passo.
 * Os comentarios `<!-- … -->` dos modelos saem ANTES da conferencia, e e o
 * texto JA SEM ELES que se confere e se entrega: a conferencia e a montagem
 * leem o mesmo texto, entao um truque que so funciona entre os dois (o
 * `--!>` que fecha comentario para o navegador) cai na conferencia.
 *
 * O funil nao substitui o `esc()`: dado continua escapado na interpolacao,
 * e e isso que impede o dado de virar etiqueta. O funil e o segundo cinto --
 * o que segura o dia em que uma interpolacao esquecer o `esc`, como o `E()`
 * do `claude.js` esqueceu em 102 lugares.
 *
 * Navegador sem Trusted Types: o `phxHTML` confere igual e devolve o texto.
 * A conferencia e a mesma nos dois; so a garantia de que ninguem pula o
 * funil e que depende do navegador. */
(function (raiz) {
  "use strict";

  var PROIBIDOS = {};
  ("script style iframe frame frameset object embed applet base meta link " +
   "noscript noembed noframes xmp plaintext listing template math portal " +
   "fencedframe animate animatemotion animatetransform set discard handler " +
   "listener").split(" ").forEach(function (n) { PROIBIDOS[n] = 1; });
  /* RCDATA no HTML, elemento comum no SVG: o mesmo texto e lido de dois
     jeitos conforme o lugar onde cai. So entram sem `<` nenhum por dentro --
     e ai os dois jeitos dao o mesmo texto. */
  var TEXTO_SO = { textarea: 1, title: 1 };
  var ENDERECOS = {
    href: 1, src: 1, "xlink:href": 1, action: 1, poster: 1, data: 1,
    background: 1, cite: 1, ping: 1, lowsrc: 1, longdesc: 1, manifest: 1,
    codebase: 1, icon: 1, dynsrc: 1, srcset: 1, imagesrcset: 1
  };
  var BRANCO = /[\t\n\f\r ]/;
  var LETRA = /[A-Za-z]/;
  var recusados = 0;

  function decodificar(v) {
    // Referencia nomeada que nao seja das cinco basicas fica: e ela que
    // denuncia o `&colon;` e o `&Tab;`, e o endereco cai na lista de recusa.
    return v.replace(/&#[xX]([0-9a-fA-F]+);?/g, function (_, h) {
      return String.fromCodePoint(parseInt(h, 16) || 0xFFFD);
    }).replace(/&#([0-9]+);?/g, function (_, d) {
      return String.fromCodePoint(Math.min(parseInt(d, 10), 0x10FFFF) || 0xFFFD);
    }).replace(/&(amp|lt|gt|quot|apos);?/g, function (_, n) {
      return { amp: "&", lt: "<", gt: ">", quot: "\"", apos: "'" }[n];
    });
  }

  function enderecoRuim(etiqueta, atributo, valor) {
    var v = decodificar(valor).replace(/[\u0000- \u007F]/g, "");
    // O esquema mora antes do primeiro `/ ? #`. Referencia nomeada que nao
    // seja das cinco basicas, ali, e o `&colon;`/`&Tab;` escondendo um
    // `javascript:` -- recusa. Depois dele e so o `&` de uma consulta.
    if (/&/.test(v.split(/[\/?#]/)[0])) return true;
    if (atributo === "srcset" || atributo === "imagesrcset") return /:/.test(v);
    var esquema = /^([A-Za-z][A-Za-z0-9+.\-]*):/.exec(v);
    if (!esquema) return false; // relativo: `#x`, `/x`, `x?y=1`
    var e = esquema[1].toLowerCase();
    if (e === "http" || e === "https" || e === "mailto" || e === "blob") return false;
    if (e === "data" && etiqueta === "img" && atributo === "src")
      return !/^data:image\/(png|gif|jpeg|webp|svg\+xml)[;,]/i.test(v);
    return true;
  }

  /* Devolve o motivo da recusa, ou "" quando o texto passa. */
  function conferir(s) {
    var n = s.length, i = 0;
    for (;;) {
      i = s.indexOf("<", i);
      if (i < 0) return "";
      var c = s.charAt(i + 1), fecho = false, j = i + 1;
      if (c === "!" || c === "?") return "declaracao ou comentario <" + c;
      if (c === "/") {
        fecho = true;
        j++;
        if (j >= n) return "";
        if (!LETRA.test(s.charAt(j))) return "fecho torto </" + s.charAt(j);
      } else if (!LETRA.test(c)) { i++; continue; } // `<` solto e texto
      var k = j;
      while (k < n && !BRANCO.test(s.charAt(k)) && s.charAt(k) !== "/" && s.charAt(k) !== ">") k++;
      var nome = s.slice(j, k).toLowerCase();
      if (PROIBIDOS[nome]) return "<" + nome + ">";
      var p = k;
      for (;;) {
        while (p < n && (BRANCO.test(s.charAt(p)) || s.charAt(p) === "/")) p++;
        if (p >= n) return "etiqueta sem fim <" + nome;
        if (s.charAt(p) === ">") { p++; break; }
        var a = p;
        p++; // o primeiro caractere e do nome, ate um `=` (a norma faz assim)
        while (p < n && !BRANCO.test(s.charAt(p)) && "/>=".indexOf(s.charAt(p)) < 0) p++;
        var at = s.slice(a, p).toLowerCase(), valor = null;
        while (p < n && BRANCO.test(s.charAt(p))) p++;
        if (s.charAt(p) === "=") {
          p++;
          while (p < n && BRANCO.test(s.charAt(p))) p++;
          var q = s.charAt(p);
          if (q === "\"" || q === "'") {
            var f = s.indexOf(q, p + 1);
            if (f < 0) return "aspas sem fim em " + at;
            valor = s.slice(p + 1, f);
            p = f + 1;
          } else if (q === ">") {
            valor = "";
          } else {
            var b = p;
            while (p < n && !BRANCO.test(s.charAt(p)) && s.charAt(p) !== ">") p++;
            valor = s.slice(b, p);
          }
        }
        if (/^on/.test(at)) return "manipulador " + at;
        if (at === "srcdoc" || at === "formaction") return "atributo " + at;
        if (valor !== null && ENDERECOS[at] && enderecoRuim(nome, at, valor))
          return "endereco em " + at;
      }
      if (!fecho && TEXTO_SO[nome]) {
        var re = new RegExp("</" + nome + "[\\t\\n\\f\\r />]", "ig");
        re.lastIndex = p;
        var m = re.exec(s);
        var fim = m ? m.index : n;
        if (s.slice(p, fim).indexOf("<") >= 0) return "etiqueta dentro de <" + nome + ">";
        p = fim;
      }
      i = p;
    }
  }

  function preparar(entrada) {
    var s = entrada == null ? "" : String(entrada);
    if (s.indexOf("<!--") >= 0) s = s.replace(/<!--[\s\S]*?(-->|$)/g, "");
    var motivo = conferir(s);
    if (motivo) {
      recusados++;
      try {
        document.dispatchEvent(new CustomEvent("phxhtmlrecusado", { detail: motivo }));
      } catch (e) { /* sem documento: so o erro */ }
      throw new TypeError("phxHTML recusou o HTML (" + motivo + ")");
    }
    return s;
  }

  var tt = raiz.trustedTypes, politica = null;
  if (tt && typeof tt.createPolicy === "function") {
    // Sem `createScript` e sem `createScriptURL`: a norma faz a politica
    // jogar `TypeError` quando lhe pedem o que ela nao declara.
    politica = tt.createPolicy("phx", { createHTML: preparar });
  }

  function phxHTML(s) {
    return politica ? politica.createHTML(s) : preparar(s);
  }
  Object.defineProperty(phxHTML, "recusados", { get: function () { return recusados; } });
  Object.defineProperty(phxHTML, "conferir", { value: function (s) { return conferir(String(s)); } });
  // Fixo na janela: nem `phxHTML = …` nem um `<a id=phxHTML>` o trocam.
  Object.defineProperty(raiz, "phxHTML", { value: phxHTML, writable: false, configurable: false });
})(window);

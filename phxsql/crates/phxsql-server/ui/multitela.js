/* =====================================================================
   MULTITELA -- abas vivas, regioes lado a lado e janelas destacadas
   =====================================================================

   O console nasceu com UMA tela por vez: `folha(...)` troca o `innerHTML` do
   `#painel`, e abrir a proxima MATA a anterior. Uma consulta com resultado,
   uma grade rolada ate a linha 800, a telemetria coletando -- tudo se perdia
   ao trocar de tela.

   Este modulo poe tres camadas em cima disso, nesta ordem de importancia:

     1. ABAS   varias telas VIVAS ao mesmo tempo dentro da moldura;
     2. REGIOES  a area central dividida em 2, 3 ou 4 colunas lado a lado,
                 cada uma com a propria tira de abas -- e o arranjo da foto
                 do WINDEV(R) numa ultrawide: designer, codigo e banco juntos;
     3. JANELA  destacar uma aba para uma janela independente, para quem tem
                monitor que o sistema trata como area separada.

   POR QUE A ORDEM E ESSA: o arranjo em painas nao precisa de API nenhuma --
   e layout, e funciona em todo navegador. A janela destacada depende da
   Window Management API para abrir NO MONITOR CERTO, e essa API so existe
   no Chrome/Edge. A resposta que serve todo mundo vem primeiro.

   ---------------------------------------------------------------------
   AS TRES DECISOES QUE SUSTENTAM O RESTO
   ---------------------------------------------------------------------

   [1] IDENTIDADE POR ID, E SO NA TELA COM FOCO.

   A pagina inteira fala por `$("#painel")`, `$("#titulo")`, `$("#abas")` --
   centenas de lugares. Com quatro telas na tela ao mesmo tempo, quatro
   `id="painel"` fariam `querySelector` devolver o primeiro em ordem de
   documento, que quase nunca e o certo. Entao: TODA tela e desenhada com
   CLASSE (`.painel`, `.abas`, `.cabecalho`), e so a tela COM FOCO ganha os
   IDs. Trocar o foco move quatro atributos. Nenhuma linha da pagina mudou.

   [2] ABA ESCONDIDA SAI DO DOCUMENTO.

   Aba que nao esta na frente tem o `.tela` DESANEXADO -- guardado em
   JavaScript, com os `value` dos campos e os ouvintes intactos, mas fora do
   `document`. Duas consequencias boas de graca: nao ha id repetido entre
   abas da mesma regiao, e todo laco que ja perguntava "ainda estou na tela?"
   (o Profiler pergunta por `$("#pfCorpo")`) para sozinho. A rolagem e o
   unico estado que desanexar perde, e por isso ela e salva e reposta a mao.

   [3] O `est` E DA TELA COM FOCO, E A TROCA E EXPLICITA.

   `est` e um objeto so, e parte dele e do SERVIDOR (sessao, usuario, bancos,
   tema) e parte e DA TELA (qual tabela, qual aba, qual ordem, qual grade).
   Duas abas de tabelas diferentes brigariam pelo mesmo `est.atual` -- e esse
   defeito so aparece com duas abertas, que e justamente o que ninguem testa.
   `DA_TELA` lista as chaves da tela; trocar de foco salva as de la e repoe
   as de ca. O resto continua unico, porque e mesmo unico.

   ---------------------------------------------------------------------
   O QUE ESTE MODULO NAO FAZ, E POR QUE
   ---------------------------------------------------------------------

   * NAO existe "arrastar a janela de volta para a barra de abas". O
     navegador nao ve o arrasto de uma janela do sistema operacional sobre
     outra -- nao ha evento nenhum quando uma janela passa por cima de outra.
     O docking classico do WINDEV(R) e do Visual Studio(R) NAO e implementavel
     como arrasto aqui. O que da, e esta feito: um botao «devolver a moldura»
     na janela destacada, e o arrasto DENTRO da moldura (reordenar, mudar de
     regiao, e arrastar para fora da tira para virar janela) -- esse o
     navegador enxerga, porque comeca dentro dele.

   * NAO reabre sozinho as janelas destacadas ao carregar. `window.open` sem
     clique e bloqueio de popup em todo navegador. O arranjo fica guardado e
     volta com UM clique em «restaurar janelas».

   * NAO guarda a sessao no `localStorage`. O disco do navegador e lido por
     qualquer outra aba da mesma origem e sobrevive ao fechamento. A ficha
     de sessao viaja pelo `BroadcastChannel`, em memoria, no instante da
     abertura -- e se a janela mae morrer, a filha pede login.

   Desenho, limites e o que muda em cada navegador: `docs/MULTITELA.md`. */

window.PhxTelas = (function () {
  "use strict";

  const CHAVE = "phxsql-multitela";
  const NOME_CANAL = "phxsql-multitela";

  /* A largura util minima de uma regiao, em pixels CSS.
   *
   * MEDIDO, nao estimado: `testes-web/medir-regiao.mjs` estreita a regiao de
   * 20 em 20 px e pergunta AO NAVEGADOR a partir de que largura o conteudo
   * passa a exigir rolagem lateral. Deu 660 para o Diagrama ER, 600 para a
   * Telemetria e o Profiler, e menos de 260 para a Consulta -- e 660 e o pior
   * caso das QUATRO telas nomeadas. A grade de uma tabela larga pede 1160 e
   * ficou de fora do criterio de proposito: tabela larga rola de lado em
   * qualquer largura, e e para isso que existe o `.rolo`. Usar 1160 exigiria
   * 4640 px uteis para quatro regioes e estragaria o caso principal por causa
   * de uma tela que ja resolve o problema sozinha. Ver `docs/MULTITELA.md`. */
  const MIN_REGIAO = 660;

  /* As chaves de `est` que pertencem a TELA, e nao ao servidor.
   *
   * Sessao, usuario, token, bancos, servidor, textos, rotulos e a area de
   * transferencia ficam de FORA de proposito: sao do console inteiro, e
   * duplica-las por aba criaria duas verdades sobre quem esta logado. */
  const DA_TELA = ["atual", "aba", "ordem", "linhas", "teto", "esquemaAtual",
    "grade", "painel", "rascunho", "pivot", "maquina", "relogioMaquina"];

  /* O catalogo das telas alcancaveis por URL e por restauracao.
   *
   * Nem toda tela entra aqui, e isso e de proposito: so tem chave a tela que
   * se abre SEM contexto -- uma que dependa de um formulario meio preenchido
   * nao se reabre por URL sem mentir sobre o que restaurou. */
  const CATALOGO = {
    painel: { rot: "Painel", abre: () => abrirAdmin("painel") },
    tabela: {
      rot: p => (p.tab || "tabela"),
      abre: p => abrirTabela(p.db, p.tab),
      valido: p => !!(p.db && p.tab),
    },
    query: { rot: "Query", abre: () => abrirConsulta() },
    diagrama: { rot: "Diagrama ER", abre: p => telaDiagramaER(p.db) },
    telemetria: { rot: "Telemetria", abre: () => telaTelemetria() },
    profiler: { rot: "Profiler", abre: () => verProfiler() },
    ia: { rot: "Claude", abre: () => PhxIA.telaConfig() },
    usuarios: { rot: "Usuários", abre: () => abrirAdmin("usuarios") },
  };

  const W = {
    regioes: [],      // [{ el, tira, abas:[tela], mostrando:tela, peso }]
    foco: null,       // a tela com foco (a que carrega os ids)
    seq: 0,
    ligado: false,
    destacada: false, // esta janela e uma tela destacada?
    rota: null,       // o `?tela=...` desta janela
    canal: null,      // BroadcastChannel
    detalhes: null,   // getScreenDetails(), quando houver
    arrasto: null,
    abrindo: 0,   // profundidade de `abrir()` em curso
    dpr: 0,
    ouvinteDpi: null,
    janelas: {},      // chave -> a janela destacada aberta daqui
  };

  const E = s => String(s == null ? "" : s)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

  const doc = s => document.querySelector(s);

  /* ------------------------------------------------------------ guardado */

  /** O que fica no navegador de quem esta sentado ali -- nunca no servidor.
   *
   *  A mesma decisao ja tomada para o painel lateral, e a mesma frase no
   *  rodape: recolhido, pinado e largura ficam NESTE NAVEGADOR. Preferencia
   *  de arranjo de tela nao e dado do banco. */
  function lido() {
    try { return JSON.parse(localStorage.getItem(CHAVE) || "{}") || {}; }
    catch { return {}; }
  }
  function grava(o) {
    try { localStorage.setItem(CHAVE, JSON.stringify(o)); } catch { /* modo privado */ }
  }

  /** Grava o arranjo: quantas regioes, o peso de cada uma, as abas PINADAS e
   *  a geometria das janelas pinadas.
   *
   *  So a aba PINADA volta na proxima abertura. Quem nunca clicar no pino
   *  abre o console exatamente como antes deste modulo existir -- guarda
   *  nova entra pedida, nao imposta, e aqui o pedido e o clique no pino. */
  function guardar() {
    if (W.destacada) return;               // a filha nao manda no arranjo da mae
    const o = lido();
    o.v = 1;
    o.regioes = W.regioes.map(r => ({
      peso: Math.round(r.peso * 1000) / 1000,
      abas: r.abas.filter(t => t.pino && t.chave)
        .map(t => ({ chave: t.chave, params: t.params })),
    }));
    // As janelas soltas pinadas, com a geometria de AGORA: arrastar ou
    // redimensionar uma janela pinada muda o que ela vai ser da proxima vez,
    // e nao ha um segundo clique para confirmar isso.
    o.soltas = MDI.janelas.filter(t => t.pino && t.chave).map(t => {
      const c = t.janela.getBoundingClientRect();
      const pai = t.janela.parentElement.getBoundingClientRect();
      return { chave: t.chave, params: t.params,
        g: { x: Math.round(c.left - pai.left), y: Math.round(c.top - pai.top),
          w: Math.round(c.width), h: Math.round(c.height) } };
    });
    grava(o);
  }

  /** A geometria pinada de uma tela destacada: x, y, largura, altura E o
   *  monitor.
   *
   *  O MONITOR faz parte da posicao. Guardar so x/y e guardar uma coordenada
   *  global cujo significado muda quando o arranjo de telas muda -- desligue
   *  o monitor da direita e a janela "pinada em 2400,300" abre fora da area
   *  visivel, que e uma janela perdida.
   *
   *  As medidas sao em PIXELS CSS, e nao fisicos, porque e a unica unidade
   *  que `window.open` aceita e que `screenX/screenY` devolvem. Num monitor
   *  de DPI diferente a mesma janela ocupa outro tamanho FISICO -- o que se
   *  conserva e o tamanho em pixels, que e o que decide se a grade cabe. O
   *  `dpr` do momento vai junto so para o documento poder dizer isso. */
  function guardarJanela(chave, g) {
    const o = lido();
    o.janelas = o.janelas || {};
    o.janelas[chave] = g;
    grava(o);
  }
  function janelaGuardada(chave) {
    const o = lido();
    return (o.janelas || {})[chave] || null;
  }

  /* ------------------------------------------------------------- estado */

  const estadoNovo = () => ({
    atual: null, aba: "estrutura", ordem: "", linhas: [], teto: 200,
    esquemaAtual: null, grade: null, painel: null, rascunho: null,
    pivot: null, maquina: null, relogioMaquina: null,
  });

  function salvarEstado(t) {
    if (!t) return;
    for (const k of DA_TELA) t.estado[k] = est[k];
  }
  function aplicarEstado(t) {
    if (!t) return;
    for (const k of DA_TELA) est[k] = t.estado[k];
  }

  // Todas as telas VIVAS: as das regioes e as que estao em janela solta.
  // Quem esquecer as soltas aqui perde os ids, o `marcarIds` e o `guardar`.
  const todas = () => W.regioes.reduce((a, r) => a.concat(r.abas), [])
    .concat(MDI.janelas);
  const chaveDe = (chave, params) => {
    const p = params || {};
    return chave === "tabela" ? `tabela:${p.db}:${p.tab}`
      : chave === "diagrama" && p.db ? `diagrama:${p.db}` : chave;
  };

  /* -------------------------------------------------------------- ids
   *
   * Quatro atributos, e a pagina inteira continua achando o que procura. */
  function marcarIds(t) {
    for (const x of todas()) {
      if (x === t) continue;
      for (const s of ["h2", ".sub", ".abas", ".painel"]) {
        const el = x.el.querySelector(s);
        if (el) el.removeAttribute("id");
      }
    }
    if (!t) return;
    const pos = [["h2", "titulo"], [".sub", "subtitulo"],
      [".abas", "abas"], [".painel", "painel"]];
    for (const [s, id] of pos) {
      const el = t.el.querySelector(s);
      if (el) el.id = id;
    }
  }

  /* ------------------------------------------------------- montar a tela */

  function molde() {
    const d = document.createElement("div");
    d.className = "tela";
    d.innerHTML = `<div class="cabecalho"><h2>—</h2><div class="sub"></div></div>
      <div class="abas"></div><div class="painel"></div>`;
    return d;
  }

  function novaRegiao(el) {
    const r = { el: el || null, tira: null, abas: [], mostrando: null, peso: 1 };
    if (!r.el) {
      r.el = document.createElement("section");
      r.el.className = "regiao";
      r.el.appendChild(Object.assign(document.createElement("div"),
        { className: "tira" }));
    }
    r.tira = r.el.querySelector(".tira");
    r.el.addEventListener("mousedown", () => {
      // A regiao onde se clica passa a ser a que o menu e a barra comandam.
      // E o mesmo comportamento de todo editor com painas: o painel clicado
      // e o painel ativo.
      if (r.mostrando && r.mostrando !== W.foco) focar(r.mostrando);
    }, true);
    ligarTira(r);
    return r;
  }

  const regiaoDe = t => W.regioes.find(r => r.abas.includes(t)) || null;

  /* ---------------------------------------------------- mostrar/esconder
   *
   * Estas duas sao sobre estar NA TELA, e nao sobre ter foco. Quatro regioes
   * abertas mostram quatro telas ao mesmo tempo: as quatro trabalham. O que
   * para de trabalhar e a aba que saiu da frente. */
  function esconder(t) {
    if (!t || !t.el.isConnected) return;
    const p = t.el.querySelector(".painel");
    t.rolagem = p ? p.scrollTop : 0;
    if (t.pausar) { try { t.pausar(); } catch (e) { /* laco ja morto */ } }
    t.el.remove();
  }

  function mostrar(r, t) {
    if (!t) return;
    if (r.mostrando && r.mostrando !== t) esconder(r.mostrando);
    r.mostrando = t;
    if (!t.el.isConnected) r.el.appendChild(t.el);
    const p = t.el.querySelector(".painel");
    if (p && t.rolagem) p.scrollTop = t.rolagem;
    if (t.retomar) { try { t.retomar(); } catch (e) { /* a tela sumiu */ } }
  }

  /** Da o foco a uma tela: ela passa a ser a dona do `est` e dos ids.
   *
   *  Vale para tela em regiao e para tela em janela solta -- a solta ja esta
   *  na frente, entao so ha o que trocar de estado e de id. */
  function focar(t) {
    if (!t || t === W.foco) { desenhar(); return; }
    const r = regiaoDe(t);
    if (!r && !t.janela) return;
    salvarEstado(W.foco);
    if (r && r.mostrando !== t) mostrar(r, t);
    W.foco = t;
    aplicarEstado(t);
    marcarIds(t);
    desenhar();
  }

  /* ------------------------------------------------------------- desenho */

  function desenhar() {
    const cont = doc("#regioes");
    if (!cont) return;
    cont.dataset.n = String(W.regioes.length);

    // A ordem dos filhos so se refaz quando MUDOU: reanexar um elemento que
    // ja esta no lugar zera a rolagem de tudo que ele contem.
    const querem = [];
    W.regioes.forEach((r, i) => {
      if (i) querem.push(calha(i));
      querem.push(r.el);
    });
    const tem = [...cont.children];
    const igual = tem.length === querem.length && tem.every((e, i) => e === querem[i]);
    if (!igual) cont.replaceChildren(...querem);

    for (const r of W.regioes) {
      r.el.style.flexGrow = String(r.peso);
      r.el.classList.toggle("foco", r.mostrando === W.foco);
      pintarTira(r);
    }
    for (const t of MDI.janelas) {
      if (!t.janela) continue;
      t.janela.querySelector(".jan-tit").textContent = t.rot;
      t.janela.classList.toggle("frente", t === W.foco);
      t.janela.querySelector('[data-jan="pino"]')
        .setAttribute("aria-pressed", t.pino ? "true" : "false");
    }
  }

  const calhas = [];
  function calha(i) {
    if (!calhas[i]) {
      const c = document.createElement("div");
      c.className = "calha";
      c.setAttribute("role", "separator");
      c.setAttribute("aria-orientation", "vertical");
      c.setAttribute("aria-label", "Largura das regiões");
      c.tabIndex = 0;
      c.dataset.i = String(i);
      ligarCalha(c);
      calhas[i] = c;
    }
    calhas[i].dataset.i = String(i);
    return calhas[i];
  }

  /** O pino, com o MESMO glifo e o MESMO significado do painel lateral.
   *
   *  Dois pinos que querem dizer coisas diferentes na mesma tela e pior que
   *  um pino so: la ele quer dizer "fica assim quando eu voltar", e aqui
   *  tambem. */
  function glifoPino() {
    try { return svgLateral("pino"); } catch (e) { return "📌"; }
  }

  function pintarTira(r) {
    const soUma = W.regioes.length === 1 && r.abas.length <= 1;
    r.tira.dataset.soUma = soUma ? "1" : "0";
    const abas = r.abas.map((t, i) => `<button class="tira-aba${
      t === r.mostrando ? " sel" : ""}" draggable="true" role="tab"
      data-i="${i}" aria-selected="${t === r.mostrando}"
      title="${E(t.rot)}${t.pino ? " · pinada: volta na próxima abertura" : ""}">
      <span class="rot">${E(t.rot)}</span>
      <span class="tira-pino" role="button" tabindex="-1" data-pino="${i}"
        aria-pressed="${t.pino ? "true" : "false"}"
        title="${t.pino ? "Despinar — esta tela deixa de voltar sozinha"
          : "Pinar — esta tela volta na próxima abertura, na mesma região"}"
        >${glifoPino()}</span>
      ${r.abas.length > 1 ? `<span class="tira-x" role="button" tabindex="-1"
        data-x="${i}" title="Fechar esta tela">×</span>` : ""}
    </button>`).join("");

    const n = W.regioes.length;
    const podeDividir = maxRegioes();
    const primeira = r === W.regioes[0];
    const controles = W.destacada
      ? `<button class="tira-bt" data-acao="pinar-janela"
           title="Pinar — guarda x, y, largura, altura e o monitor desta janela
neste navegador, e ela volta assim na próxima vez">
           ${glifoPino()} <span class="num">pinar aqui</span></button>
         <button class="tira-bt" data-acao="devolver"
           title="Devolver esta tela para a janela principal e fechar esta">
           ⤺ <span class="num">devolver</span></button>`
      : `<button class="tira-bt" data-acao="nova"
           title="Abrir outra tela nesta região (a próxima escolha cai aqui)">+</button>
         <button class="tira-bt" data-acao="soltar"
           title="Soltar esta tela numa janela flutuante DENTRO da página,
arrastável pelo cabeçalho e redimensionável pelo canto">⇱</button>
         <button class="tira-bt" data-acao="destacar"
           title="Destacar numa janela do sistema, fora desta página
(só serve para quem tem monitor separado — o modo em regiões não depende disto)"
           >⧉</button>`
        + (primeira ? [1, 2, 3, 4].map(k => `<button class="tira-bt${
            k === n ? " sel" : ""}" data-acao="dividir" data-n="${k}"
            ${k > podeDividir ? "disabled" : ""}
            title="${k === 1 ? "Uma região só"
              : `${k} regiões lado a lado`}${k > podeDividir
              ? ` — não cabe: cada região precisa de ${MIN_REGIAO}px`
              : ""}">${"▮".repeat(k)}</button>`).join("") : "");

    r.tira.innerHTML = abas + `<span class="tira-espaco"></span>` + controles;
  }

  /** Quantas regioes cabem AGORA, pela largura util.
   *
   *  Quem divide e a pessoa, com o botao -- o numero aqui so apaga o que nao
   *  cabe. Divisao automatica que muda sozinha quando a janela redimensiona
   *  e desorientadora: a tela reorganiza-se debaixo do dedo de quem estava
   *  lendo. */
  function maxRegioes() {
    const cont = doc("#regioes");
    const l = cont ? cont.getBoundingClientRect().width : window.innerWidth;
    return Math.max(1, Math.min(4, Math.floor(l / MIN_REGIAO)));
  }

  /* ------------------------------------------------------------ eventos */

  function ligarTira(r) {
    r.tira.addEventListener("click", ev => {
      const pino = ev.target.closest("[data-pino]");
      if (pino) { ev.stopPropagation(); alternarPino(r.abas[+pino.dataset.pino]); return; }
      const x = ev.target.closest("[data-x]");
      if (x) { ev.stopPropagation(); fechar(r.abas[+x.dataset.x]); return; }
      const bt = ev.target.closest(".tira-bt");
      if (bt) {
        const a = bt.dataset.acao;
        if (a === "nova") novaAba(r);
        else if (a === "soltar") soltar(r.mostrando);
        else if (a === "destacar") destacar(r.mostrando);
        else if (a === "devolver") devolver();
        else if (a === "pinar-janela") pinarJanela();
        else if (a === "dividir") dividir(+bt.dataset.n);
        return;
      }
      const aba = ev.target.closest(".tira-aba");
      if (aba) focar(r.abas[+aba.dataset.i]);
    });

    r.tira.addEventListener("dragstart", ev => {
      const aba = ev.target.closest(".tira-aba");
      if (!aba) return;
      W.arrasto = { tela: r.abas[+aba.dataset.i], soltou: false };
      aba.classList.add("arrastando");
      try { ev.dataTransfer.setData("text/plain", W.arrasto.tela.rot); } catch (e) { /* IE-ismo */ }
      ev.dataTransfer.effectAllowed = "move";
    });
    r.tira.addEventListener("dragend", ev => {
      r.tira.querySelectorAll(".arrastando").forEach(e => e.classList.remove("arrastando"));
      const a = W.arrasto;
      W.arrasto = null;
      if (!a || a.soltou) return;
      // Soltou FORA de qualquer regiao: vira janela. Este e o unico arrasto
      // que o navegador enxerga -- ele comeca dentro da pagina.
      const cx = ev.clientX, cy = ev.clientY;
      const cont = doc("#regioes");
      const c = cont ? cont.getBoundingClientRect() : null;
      const fora = !c || cx < c.left || cx > c.right || cy < c.top || cy > c.bottom;
      if (fora && cx > 0 && cy > 0) destacar(a.tela);
    });
    r.tira.addEventListener("dragover", ev => {
      if (!W.arrasto) return;
      ev.preventDefault();
      r.tira.classList.add("alvo");
    });
    r.tira.addEventListener("dragleave", () => r.tira.classList.remove("alvo"));
    r.tira.addEventListener("drop", ev => {
      r.tira.classList.remove("alvo");
      if (!W.arrasto) return;
      ev.preventDefault();
      W.arrasto.soltou = true;
      mover(W.arrasto.tela, r, indiceNoPonto(r, ev.clientX));
    });
    // Soltar no CORPO da regiao tambem move para ela -- alvo grande, e o que
    // a mao faz quando quer "por esta tela ali".
    r.el.addEventListener("dragover", ev => { if (W.arrasto) ev.preventDefault(); });
    r.el.addEventListener("drop", ev => {
      if (!W.arrasto || W.arrasto.soltou) return;
      ev.preventDefault();
      W.arrasto.soltou = true;
      mover(W.arrasto.tela, r, r.abas.length);
    });
  }

  function indiceNoPonto(r, x) {
    const abas = [...r.tira.querySelectorAll(".tira-aba")];
    for (let i = 0; i < abas.length; i++) {
      const c = abas[i].getBoundingClientRect();
      if (x < c.left + c.width / 2) return i;
    }
    return abas.length;
  }

  function mover(t, destino, idx) {
    const origem = regiaoDe(t);
    if (!origem) return;
    const i = origem.abas.indexOf(t);
    origem.abas.splice(i, 1);
    if (origem === destino && idx > i) idx--;
    if (origem !== destino) esconder(t);
    if (origem.mostrando === t) origem.mostrando = null;
    destino.abas.splice(Math.max(0, Math.min(idx, destino.abas.length)), 0, t);
    if (!origem.abas.length && origem !== destino) {
      // Regiao que ficou vazia recebe uma tela nova em vez de sumir: sumir
      // mudaria a divisao que a pessoa escolheu, sem ela pedir.
      abaVazia(origem);
    } else if (origem.mostrando === null) {
      mostrar(origem, origem.abas[Math.min(i, origem.abas.length - 1)]);
    }
    focar(t);
    mostrar(destino, t);
    guardar();
    desenhar();
  }

  function ligarCalha(c) {
    let arr = null;
    const desce = ev => {
      const i = +c.dataset.i;
      const a = W.regioes[i - 1], b = W.regioes[i];
      if (!a || !b) return;
      arr = { x: ev.clientX, a, b, la: a.el.getBoundingClientRect().width,
        lb: b.el.getBoundingClientRect().width, pa: a.peso, pb: b.peso };
      ev.preventDefault();
      document.addEventListener("mousemove", anda);
      document.addEventListener("mouseup", solta);
    };
    const anda = ev => {
      if (!arr) return;
      const d = ev.clientX - arr.x;
      const la = arr.la + d, lb = arr.lb - d;
      if (la < 220 || lb < 220) return;     // nao deixa uma regiao virar risco
      const soma = arr.pa + arr.pb;
      arr.a.peso = soma * (la / (la + lb));
      arr.b.peso = soma - arr.a.peso;
      arr.a.el.style.flexGrow = String(arr.a.peso);
      arr.b.el.style.flexGrow = String(arr.b.peso);
    };
    const solta = () => {
      arr = null;
      document.removeEventListener("mousemove", anda);
      document.removeEventListener("mouseup", solta);
      guardar();
    };
    c.addEventListener("mousedown", desce);
    c.addEventListener("keydown", ev => {
      const passo = ev.key === "ArrowRight" ? 0.06 : ev.key === "ArrowLeft" ? -0.06 : 0;
      if (!passo) return;
      const i = +c.dataset.i;
      const a = W.regioes[i - 1], b = W.regioes[i];
      if (!a || !b) return;
      if (a.peso + passo < 0.25 || b.peso - passo < 0.25) return;
      a.peso += passo; b.peso -= passo;
      ev.preventDefault();
      desenhar();
      guardar();
    });
  }

  /* ---------------------------------------------------------- as acoes */

  /** Uma aba em branco nesta regiao, ja com o Painel pintado.
   *
   *  Tres lugares precisavam da mesma coisa -- o botao `+`, a regiao que
   *  nasce numa divisao e a regiao que ficou vazia quando a ultima tela dela
   *  saiu para uma janela. Tres copias divergiriam. */
  function abaVazia(r) {
    const t = criarAba(r, "painel", {});
    const antes = W.foco;
    mostrar(r, t);
    Promise.resolve().then(() => {
      focar(t);
      W.abrindo++;
      return Promise.resolve(abrirAdmin("painel"))
        .finally(() => { W.abrindo--; })
        .then(() => { renomear(); if (antes && antes !== t) focar(antes); });
    }).catch(e => avisar(String(e), true));
    return t;
  }

  function criarAba(r, chave, params) {
    const t = {
      id: `tela${++W.seq}`, chave, params: params || {},
      rot: rotuloDe(chave, params), el: molde(), estado: estadoNovo(),
      pino: false, rolagem: 0, pausar: null, retomar: null,
    };
    r.abas.push(t);
    return t;
  }

  function rotuloDe(chave, params) {
    const c = CATALOGO[chave];
    if (!c) return chave;
    return typeof c.rot === "function" ? c.rot(params || {}) : c.rot;
  }

  /** Abre uma tela. Sem `nova`, ela SUBSTITUI o conteudo da aba com foco --
   *  que e exatamente o que a pagina fazia antes deste modulo. */
  async function abrir(chave, params, opc) {
    opc = opc || {};
    if (!W.ligado) return;
    params = params || {};
    const c = CATALOGO[chave];
    if (!c) return;

    // Ja aberta? Traz para a frente em vez de abrir a segunda copia. Duas
    // copias da mesma tela dividiriam os ids de dentro do painel (`#grade`,
    // `#pfCorpo`), e a segunda roubaria os cliques da primeira.
    // Ja aberta NOUTRA aba? Traz para a frente sem recarregar -- e assim que
    // a grade rolada ate a linha 800 sobrevive a um passeio pela arvore.
    //
    // Na PROPRIA aba com foco, nao: ali o comportamento velho e recarregar, e
    // ele tem de continuar valendo. Ja custou um defeito -- `abrirAdmin` zera
    // `est.atual` sem trocar a chave da aba, entao clicar de novo na tabela
    // achava "essa ja e a aba" e voltava sem repor `est.atual`; as telas
    // seguintes caiam no primeiro banco da lista em vez de na tabela aberta.
    const k = chaveDe(chave, params);
    const ja = todas().find(t => chaveDe(t.chave, t.params) === k);
    if (ja && ja !== W.foco && !opc.forcar) {
      // Pediram a tela NUMA regiao e ela mora noutra: ela MUDA de regiao, e
      // nao "vai piscar la onde estava". Quem monta o arranjo das quatro
      // telas lado a lado esta dizendo onde cada uma vai.
      const onde = regiaoDe(ja);
      if (opc.regiao && onde && onde !== opc.regiao) {
        mover(ja, opc.regiao, opc.regiao.abas.length);
        return ja;
      }
      focar(ja);
      return ja;
    }

    let alvo = W.foco;
    const r = (opc.regiao || (alvo && regiaoDe(alvo)) || W.regioes[0]);
    if (opc.nova || !alvo || regiaoDe(alvo) !== r) {
      alvo = criarAba(r, chave, params);
    } else {
      alvo.chave = chave; alvo.params = params;
    }
    alvo.rot = rotuloDe(chave, params);
    focar(alvo);
    mostrar(r, alvo);
    desenhar();
    // `abrindo` cala o `marcar(null)` que a propria tela dispara la dentro:
    // quase toda tela do catalogo pinta por `folha(...)`, e sem esta guarda a
    // aba perderia a chave no mesmo instante em que a ganhou -- e clicar duas
    // vezes na Telemetria abriria duas.
    W.abrindo++;
    try { await c.abre(params); }
    catch (e) { avisar(String(e && e.message || e), true); }
    finally { W.abrindo--; }
    renomear();
    return alvo;
  }

  /** Diz de que TELA e o conteudo que acabou de ser pintado na aba com foco.
   *
   *  Sem isto a aba mentiria sobre si mesma: o passeio abre uma tabela e
   *  depois pinta trinta folhas por cima dela na MESMA aba -- e, pela chave
   *  velha, voltar para a tabela pela arvore acharia "essa ja esta aberta" e
   *  mostraria a folha que sobrou no lugar. `chave` nula quer dizer «folha
   *  avulsa»: sem endereco proprio, e sem promessa de reabrir. */
  function marcar(chave, params) {
    const t = W.foco;
    if (!t) return;
    if (chave === null && W.abrindo > 0) return;
    t.chave = chave;
    t.params = params || {};
    if (chave) t.rot = rotuloDe(chave, t.params);
    if (t.pino && !chave) { t.pino = false; guardar(); }   // sem endereco, sem pino
    desenhar();
  }

  /** Uma aba nova e VAZIA nesta regiao: a proxima escolha de menu cai nela. */
  function novaAba(r) {
    r = r || regiaoDe(W.foco) || W.regioes[0];
    const t = criarAba(r, "painel", {});
    focar(t);
    mostrar(r, t);
    desenhar();
    W.abrindo++;
    Promise.resolve().then(() => abrirAdmin("painel"))
      .finally(() => { W.abrindo--; })
      .then(renomear)
      .catch(e => avisar(String(e), true));
    return t;
  }

  function fechar(t) {
    const r = regiaoDe(t);
    if (!r || r.abas.length <= 1) return;
    const i = r.abas.indexOf(t);
    // Soltar o que ela segurava ANTES de tirar da lista: intervalo, canal e
    // observador que sobrevivem a aba fechada so aparecem depois de meia
    // hora de uso, que e exatamente quando ninguem esta olhando.
    if (t.pausar) { try { t.pausar(); } catch (e) { /* ja morto */ } }
    t.pausar = t.retomar = null;
    if (t.estado.relogioMaquina) clearInterval(t.estado.relogioMaquina);
    if (t.estado.grade && t.estado.grade.destruir) {
      try { t.estado.grade.destruir(); } catch (e) { /* grade sem destruir */ }
    }
    esconder(t);
    t.el.innerHTML = "";
    r.abas.splice(i, 1);
    const proxima = r.abas[Math.min(i, r.abas.length - 1)];
    if (W.foco === t) { W.foco = null; focar(proxima); }
    mostrar(r, proxima);
    guardar();
    desenhar();
  }

  function alternarPino(t) {
    if (!t) return;
    t.pino = !t.pino;
    guardar();
    desenhar();
    avisar(t.pino
      ? `“${t.rot}” pinada: volta na próxima abertura, neste navegador`
      : `“${t.rot}” despinada`);
  }

  /** Divide a area central em N regioes lado a lado. */
  function dividir(n) {
    n = Math.max(1, Math.min(4, n | 0));
    const teto = maxRegioes();
    if (n > teto) {
      avisar(`não cabem ${n} regiões: cada uma precisa de ${MIN_REGIAO}px`, true);
      return;
    }
    while (W.regioes.length < n) {
      const r = novaRegiao(null);
      W.regioes.push(r);
      abaVazia(r);
    }
    while (W.regioes.length > n) {
      const r = W.regioes.pop();
      for (const t of r.abas.slice()) mover(t, W.regioes[0], W.regioes[0].abas.length);
      r.el.remove();
    }
    // Pesos iguais ao redividir: guardar o peso de uma divisao de tres e
    // aplica-lo numa de duas daria uma regiao de 15%.
    for (const r of W.regioes) r.peso = 1;
    if (!W.foco || !regiaoDe(W.foco)) focar(W.regioes[0].mostrando);
    desenhar();
    guardar();
  }

  /** O rotulo da aba acompanha o titulo que a tela escreveu. */
  function renomear() {
    const t = W.foco;
    if (!t) return;
    const h = t.el.querySelector(".cabecalho h2");
    const novo = (h && h.textContent || "").trim();
    if (novo && novo !== "—" && novo !== t.rot) { t.rot = novo; desenhar(); }
  }

  /* ------------------------------------------------------------- lacos */

  /** Registra o par pausar/retomar da tela com foco.
   *
   *  Aba escondida tem de PARAR de trabalhar, e o portao que decide isso vem
   *  ANTES do trabalho -- a licao do Profiler, que cobrava 7% da carga
   *  desligado porque analisava o corpo antes de olhar o proprio interruptor.
   *  Aqui e o mesmo desenho: a aba que sai da frente nao "pergunta se deve
   *  pedir", ela para de pedir. */
  function laco(pausar, retomar) {
    const t = W.foco;
    if (!t) return;
    if (t.pausar && t.pausar !== pausar) { try { t.pausar(); } catch (e) { /* ja morto */ } }
    t.pausar = pausar; t.retomar = retomar;
  }

  /** A tela vai ser substituida no MESMO lugar: solta o laco desta aba -- e
   *  so o desta. Parar o relogio global mataria a telemetria da aba ao lado. */
  function pararLaco() {
    const t = W.foco;
    if (!t) return;
    if (t.pausar) { try { t.pausar(); } catch (e) { /* ja morto */ } }
    t.pausar = t.retomar = null;
  }

  /* ------------------------------------------------- os monitores, quando ha
   *
   * `getScreenDetails()` so existe no Chrome/Edge, so em contexto seguro, e
   * so depois da permissao `window-management`. `127.0.0.1` E contexto
   * seguro, entao o console local se qualifica. Onde ela nao existe -- Firefox
   * e Safari -- tudo abaixo devolve `null` e o modo continua funcionando com
   * `window.open` simples: a pessoa arrasta a janela, e a posicao volta de
   * `screenX/screenY`. A tela DIZ o que muda; nao finge que e igual. */
  const temApiDeTelas = () => typeof window.getScreenDetails === "function";

  async function monitores() {
    if (!temApiDeTelas()) return null;
    if (W.detalhes) return W.detalhes;
    try { W.detalhes = await window.getScreenDetails(); }
    catch (e) { return null; }          // permissao negada: segue sem
    return W.detalhes;
  }

  /** Onde estao as EMENDAS FISICAS dentro desta janela, em pixels CSS da
   *  pagina.
   *
   *  Este e o caso do super-ultrawide por daisy chain: uma janela de 5120px
   *  pode ter dois monitores por baixo, e uma regiao que caia em cima da
   *  emenda e uma regiao partida ao meio. Com a API da para alinhar a
   *  divisao com as bordas; sem ela, partes iguais. */
  async function emendas() {
    const d = await monitores();
    const cont = doc("#regioes");
    if (!d || !cont) return [];
    const c = cont.getBoundingClientRect();
    // O canto da AREA DE CONTEUDO em coordenadas globais. `screenX` aponta
    // para a borda externa da janela; a diferenca entre `outerWidth` e
    // `innerWidth` e a moldura, e ela e simetrica nos lados em todo navegador
    // de mesa. E uma aproximacao, e esta escrita como tal no documento.
    const borda = Math.max(0, (window.outerWidth - window.innerWidth) / 2);
    const esq = window.screenX + borda;
    const cortes = [];
    for (const s of d.screens) {
      for (const x of [s.left, s.left + s.width]) {
        const dentro = x - esq - c.left;
        if (dentro > MIN_REGIAO / 2 && dentro < c.width - MIN_REGIAO / 2) {
          if (!cortes.some(v => Math.abs(v - dentro) < 8)) cortes.push(dentro);
        }
      }
    }
    return cortes.sort((a, b) => a - b);
  }

  /** Divide alinhando as regioes com as emendas fisicas dos monitores. */
  async function alinharComOsMonitores() {
    const cortes = await emendas();
    const cont = doc("#regioes");
    if (!cont) return false;
    if (!cortes.length) {
      // Recado, e nao erro: nao ter a API nao e falha de ninguem, e pintar de
      // vermelho o que e so uma diferenca de navegador ensina a ignorar o
      // vermelho. A divisao em partes iguais continua valendo.
      avisar(temApiDeTelas()
        ? "esta janela está dentro de um monitor só — nada a alinhar"
        : "este navegador não expõe o arranjo de monitores; a divisão fica em partes iguais");
      return false;
    }
    const l = cont.getBoundingClientRect().width;
    const n = Math.min(4, cortes.length + 1);
    dividir(n);
    const bordas = [0, ...cortes.slice(0, n - 1), l];
    for (let i = 0; i < W.regioes.length; i++) {
      W.regioes[i].peso = Math.max(0.1, (bordas[i + 1] - bordas[i]) / l * n);
    }
    desenhar();
    guardar();
    avisar(`${n} regiões alinhadas com as bordas físicas dos monitores`);
    return true;
  }

  /* ------------------------------------------------------ janela destacada */

  function urlDaTela(chave, params) {
    const q = new URLSearchParams();
    q.set("tela", chave);
    if (params.db) q.set("db", params.db);
    if (params.tab) q.set("tab", params.tab);
    q.set("destacada", "1");
    return `${location.origin}${location.pathname}?${q.toString()}`;
  }

  /** Destaca a tela numa janela independente.
   *
   *  Com a Window Management API a janela nasce JA no monitor certo -- e por
   *  isso ela vale a permissao: sem ela so da para abrir onde o navegador
   *  quiser e pedir para a pessoa arrastar. */
  async function destacar(t) {
    if (!t) return;
    const c = CATALOGO[t.chave];
    if (!c) return avisar("esta tela não tem endereço próprio para destacar", true);
    const k = chaveDe(t.chave, t.params);
    const g = janelaGuardada(k);
    const feicoes = await feicoesDaJanela(g);
    const j = window.open(urlDaTela(t.chave, t.params), `phxsql-${k}`, feicoes);
    if (!j) return avisar("o navegador bloqueou a janela — libere o popup desta origem", true);
    W.janelas[k] = j;
    if (regiaoDe(t) && regiaoDe(t).abas.length > 1) fechar(t);
    avisar(`“${t.rot}” foi para uma janela${g ? " na posição pinada" : ""}`);
  }

  /** Monta o `left,top,width,height` do `window.open`.
   *
   *  Se o monitor pinado sumiu, cai para o primario e DIZ. Janela que abre
   *  fora da area visivel e janela perdida: ela existe, consome sessao e
   *  ninguem a ve para fechar. */
  async function feicoesDaJanela(g) {
    const base = "popup=yes,scrollbars=yes,resizable=yes";
    if (!g) return `${base},width=1100,height=760`;
    const d = await monitores();
    if (d) {
      const achou = d.screens.find(s => (s.label || "") === g.monitor)
        || (g.monitor ? null : d.currentScreen);
      if (!achou) {
        const p = d.screens.find(s => s.isPrimary) || d.screens[0];
        avisar(`o monitor “${g.monitor}” não está mais aqui — a janela abre no principal`, true);
        const x = p.availLeft + Math.max(0, Math.min(g.x - (g.mx || 0), p.availWidth - g.w));
        const y = p.availTop + Math.max(0, Math.min(g.y - (g.my || 0), p.availHeight - g.h));
        return `${base},left=${Math.round(x)},top=${Math.round(y)},width=${g.w},height=${g.h}`;
      }
    }
    return `${base},left=${g.x},top=${g.y},width=${g.w},height=${g.h}`;
  }

  /** Pina a janela destacada onde ela esta agora. */
  async function pinarJanela() {
    if (!W.destacada || !W.rota) return;
    const k = chaveDe(W.rota.tela, W.rota);
    const d = await monitores();
    const meu = d ? d.currentScreen : null;
    guardarJanela(k, {
      x: window.screenX, y: window.screenY,
      w: window.outerWidth, h: window.outerHeight,
      monitor: meu ? (meu.label || "") : "",
      mx: meu ? meu.left : 0, my: meu ? meu.top : 0,
      dpr: window.devicePixelRatio,
      quando: new Date().toISOString(),
    });
    avisar("posição, tamanho e monitor guardados neste navegador");
    desenhar();
  }

  /** Devolve a tela para a moldura e fecha esta janela. */
  function devolver() {
    if (!W.destacada || !W.rota) return;
    if (W.canal) {
      W.canal.postMessage({ t: "devolver", tela: W.rota.tela,
        db: W.rota.db, tab: W.rota.tab });
    }
    window.close();
  }

  /* ------------------------------------------------------------- a sessao
   *
   * A ficha de sessao NAO vai para o disco do navegador. Ela viaja pelo
   * `BroadcastChannel`, que e da mesma origem e vive em memoria: a janela
   * nova pede, a mae responde. Se a mae ja morreu, a filha pede login -- e
   * dizer isso e melhor que guardar a sessao onde qualquer aba a le. */
  function abrirCanal() {
    if (W.canal || typeof BroadcastChannel !== "function") return W.canal;
    W.canal = new BroadcastChannel(NOME_CANAL);
    W.canal.onmessage = ev => {
      const m = ev.data || {};
      if (m.t === "quero-sessao" && !W.destacada && est.sessao) {
        W.canal.postMessage({
          t: "sessao", para: m.de, sessao: est.sessao, token: est.token,
          usuario: est.usuario, servidor: est.servidor, database: est.database,
        });
      } else if (m.t === "devolver" && !W.destacada) {
        abrir(m.tela, { db: m.db, tab: m.tab }, { nova: true });
      } else if (m.t === "arvore" && W.destacada) {
        // Um banco nasceu na mae: a filha nao tem arvore, mas a lista de
        // bancos que ela guarda envelheceu.
        api("bancos").then(b => { est.bancos = b; }).catch(() => {});
      }
    };
    return W.canal;
  }

  /** Avisa as janelas filhas que a arvore mudou. */
  function avisarArvore() {
    if (W.canal && !W.destacada) W.canal.postMessage({ t: "arvore" });
  }

  /** A rota desta janela, lida da URL. */
  function rotaDaUrl() {
    const q = new URLSearchParams(location.search);
    const tela = q.get("tela") || "";
    return {
      tela, db: q.get("db") || "", tab: q.get("tab") || "",
      destacada: q.get("destacada") === "1" && !!CATALOGO[tela],
    };
  }

  /** Sobe esta janela em MODO DESTACADO: pede a sessao a mae e entra sem
   *  formulario. Sem resposta em 2,5 s, mostra o login -- que e o caminho
   *  honesto quando a mae ja fechou. */
  function pedirSessao(rota) {
    W.rota = rota;
    W.destacada = true;
    const canal = abrirCanal();
    const recado = doc("#recado");
    if (!canal) {
      if (recado) {
        recado.className = "recado erro";
        recado.textContent = "este navegador não tem BroadcastChannel: entre novamente";
      }
      return Promise.resolve(false);
    }
    if (recado) {
      recado.className = "recado info";
      recado.textContent = "pedindo a sessão à janela principal…";
    }
    const de = `j${Date.now()}${Math.random().toString(36).slice(2, 7)}`;
    return new Promise(resolve => {
      let pronto = false;
      const antes = canal.onmessage;
      canal.onmessage = ev => {
        const m = ev.data || {};
        if (m.t === "sessao" && m.para === de && !pronto) {
          pronto = true;
          canal.onmessage = antes;
          est.sessao = m.sessao; est.token = m.token; est.usuario = m.usuario;
          est.servidor = m.servidor || ""; est.database = m.database || "";
          est.demo = false;
          resolve(true);
        } else if (antes) { antes(ev); }
      };
      canal.postMessage({ t: "quero-sessao", de });
      setTimeout(() => {
        if (pronto) return;
        canal.onmessage = antes;
        if (recado) {
          recado.className = "recado erro";
          recado.textContent = "a janela principal não respondeu — entre por aqui";
        }
        resolve(false);
      }, 2500);
    });
  }

  /* ----------------------------------------------------------------- DPI
   *
   * `devicePixelRatio` muda quando a janela passa para um monitor de outra
   * densidade. Nao ha evento proprio: o caminho e um `matchMedia` na
   * resolucao de AGORA, que deixa de casar assim que ela muda. */
  function vigiarDpi() {
    if (W.ouvinteDpi) { try { W.ouvinteDpi.onchange = null; } catch (e) { /* velho */ } }
    W.dpr = window.devicePixelRatio || 1;
    if (!window.matchMedia) return;
    const mq = window.matchMedia(`(resolution: ${W.dpr}dppx)`);
    mq.onchange = () => {
      const novo = window.devicePixelRatio || 1;
      if (novo === W.dpr) return;
      const antes = W.dpr;
      vigiarDpi();
      document.dispatchEvent(new CustomEvent("phx-dpi", { detail: { antes, agora: novo } }));
      avisar(`monitor de outra densidade (${antes}× → ${novo}×) — redesenhando`);
      // O desenho tem de se refazer: bolha, arco e diagrama medem em pixels,
      // e a mesma medida em outro DPI da outro tamanho na tela.
      desenhar();
      try { atualizarVista(); } catch (e) { /* nada aberto */ }
    };
    W.ouvinteDpi = mq;
  }

  /* ----------------------------------------------------------- a nota da tela
   *
   * Dizer na tela o que muda em qual navegador, em vez de fingir que e igual. */
  function nota() {
    const api = temApiDeTelas();
    return `<div class="multitela-nota">
      <b>Multitela.</b> Abas vivas e regiões lado a lado funcionam em
      <b>qualquer navegador</b> — é layout. Destacar em janela também, com
      <code>window.open</code>. O que depende do navegador é abrir a janela
      <b>já no monitor certo</b>: ${api
        ? "este navegador tem a <b>Window Management API</b>, então a posição "
          + "pinada volta no monitor em que você a deixou."
        : "este navegador <b>não tem</b> a Window Management API (Firefox e "
          + "Safari não a têm). A janela abre onde o navegador quiser e você "
          + "a arrasta; a posição volta, o monitor não é escolhido."}
      <br>Arrastar uma janela do sistema de volta para a barra de abas
      <b>não é possível</b> em navegador nenhum — o navegador não vê esse
      arrasto. Use <b>⤺ devolver</b> na janela destacada.</div>`;
  }

  /* --------------------------------------------------------------- inicio */

  function iniciar(opc) {
    opc = opc || {};
    const cont = doc("#regioes");
    if (!cont || W.ligado) return;
    const r0el = cont.querySelector(".regiao");
    const tela0 = r0el.querySelector(".tela");
    const r0 = novaRegiao(r0el);
    const t0 = {
      id: `tela${++W.seq}`, chave: "painel", params: {}, rot: "Painel",
      el: tela0, estado: estadoNovo(), pino: false, rolagem: 0,
      pausar: null, retomar: null,
    };
    r0.abas.push(t0); r0.mostrando = t0;
    W.regioes = [r0]; W.foco = t0; W.ligado = true;
    marcarIds(t0);
    abrirCanal();
    vigiarDpi();

    if (opc.destacada) {
      W.destacada = true;
      doc("#app").dataset.destacada = "1";
    } else {
      window.addEventListener("resize", () => desenhar());
    }
    desenhar();
  }

  /** Repoe o arranjo guardado: as regioes, os pesos e as abas PINADAS.
   *
   *  Chamada depois de a arvore montar, porque `montarArvore` termina
   *  clicando no Painel -- e esse clique pinta a primeira aba. */
  async function restaurar() {
    if (W.destacada) return;
    const o = lido();
    const regs = o.regioes || [];
    if (regs.length > 1) dividir(Math.min(regs.length, maxRegioes()));
    for (let i = 0; i < W.regioes.length && i < regs.length; i++) {
      if (regs[i].peso) W.regioes[i].peso = regs[i].peso;
    }
    for (let i = 0; i < regs.length; i++) {
      const r = W.regioes[i];
      if (!r) break;
      for (const a of (regs[i].abas || [])) {
        const c = CATALOGO[a.chave];
        if (!c || (c.valido && !c.valido(a.params || {}))) continue;
        const t = await abrir(a.chave, a.params || {}, { regiao: r, nova: true });
        if (t) t.pino = true;
      }
    }
    // E as janelas soltas pinadas. `geometriaDe` prende cada uma dentro da
    // area visivel: a pagina de hoje pode ser menor que a de ontem.
    for (const a of (o.soltas || [])) {
      const c = CATALOGO[a.chave];
      if (!c || (c.valido && !c.valido(a.params || {}))) continue;
      const t = await abrir(a.chave, a.params || {}, { nova: true });
      if (!t) continue;
      t.pino = true;
      soltar(t);
    }
    desenhar();
  }

  /* ======================================================= janelas soltas
   *
   * O terceiro modo: janela flutuante DENTRO da pagina -- o MDI classico de
   * IDE. Nao e janela do sistema operacional, e por isso ela nao depende de
   * permissao, de popup nem de navegador.
   *
   * A regra que faz o modo funcionar sem perder nada: soltar e acoplar MOVEM
   * o mesmo no do DOM (`appendChild` do proprio `.tela`), nunca refazem o
   * HTML. Campo meio digitado, resultado de consulta e ouvinte pendurado
   * sobrevivem. `scrollTop` NAO sobrevive a mudanca de pai -- medido, zera --,
   * entao ele e salvo e reposto a mao, que e o mesmo cuidado do esconder.
   *
   * E a armadilha do arrasto aninhado: o Diagrama ER arrasta tabelas por
   * conta. A janela so anda pelo CABECALHO; o corpo nao ouve `mousedown` de
   * mover. Sem isso, arrastar uma tabela do diagrama arrastaria a janela. */

  const MDI = { janelas: [], z: 20 };

  function camadaMdi() {
    let el = doc(".corpo .mdi");
    if (!el) {
      el = document.createElement("div");
      el.className = "mdi";
      const corpo = doc(".corpo");
      if (!corpo) return null;
      corpo.appendChild(el);
    }
    return el;
  }

  /** Tira a tela da regiao e a poe numa janela flutuante. */
  function soltar(t) {
    t = t || W.foco;
    if (!t || t.janela) return;
    const camada = camadaMdi();
    if (!camada) return;
    const r = regiaoDe(t);
    if (!r) return;

    const p = t.el.querySelector(".painel");
    const rolagem = p ? p.scrollTop : 0;
    if (t.pausar) { try { t.pausar(); } catch (e) { /* laco ja morto */ } }

    const j = document.createElement("div");
    j.className = "janela";
    j.innerHTML = `<div class="jan-topo">
        <span class="jan-tit">${E(t.rot)}</span>
        <button class="tira-pino" data-jan="pino" aria-pressed="false"
          title="Pinar — guarda x, y, largura e altura desta janela neste navegador"
          >${glifoPino()}</button>
        <button class="tira-x" data-jan="acoplar"
          title="Devolver esta tela para a área em regiões">⇤</button>
        <button class="tira-x" data-jan="fechar" title="Fechar esta tela">×</button>
      </div>
      <div class="jan-corpo"></div>
      <button class="jan-canto" data-jan="canto" aria-label="Redimensionar"></button>`;

    const g = geometriaDe(t);
    Object.assign(j.style, { left: g.x + "px", top: g.y + "px",
      width: g.w + "px", height: g.h + "px", zIndex: String(++MDI.z) });

    j.querySelector(".jan-corpo").appendChild(t.el);
    camada.appendChild(j);
    t.janela = j;
    MDI.janelas.push(t);

    // A regiao perde a aba. Vazia, ela ganha uma tela nova: coluna vazia e
    // pior que a tela que estava la.
    const i = r.abas.indexOf(t);
    r.abas.splice(i, 1);
    if (r.mostrando === t) r.mostrando = null;
    if (!r.abas.length) abaVazia(r);
    else mostrar(r, r.abas[Math.min(i, r.abas.length - 1)]);

    if (p) p.scrollTop = rolagem;     // mudar de pai zera a rolagem: reposta
    if (t.retomar) { try { t.retomar(); } catch (e) { /* a tela sumiu */ } }
    ligarJanela(t, j);
    // NAO force o foco com `W.foco = null; focar(t)`: `focar` APLICA o estado
    // guardado da tela, e o da tela com foco esta velho de proposito (o vivo e
    // o proprio `est`). Ja custou um defeito -- soltar a grade de uma tabela
    // devolvia `est.atual` nulo, e a janela solta nao sabia mais o que mostrava.
    if (W.foco !== t) focar(t); else desenhar();
    guardar();
    return j;
  }

  /** A geometria de partida: a pinada, se houver; senao uma cascata. */
  function geometriaDe(t) {
    const k = chaveDe(t.chave, t.params);
    const salva = (lido().soltas || []).find(x => chaveDe(x.chave, x.params) === k);
    const corpo = doc(".corpo");
    const c = corpo ? corpo.getBoundingClientRect() : { width: 1200, height: 800 };
    if (salva && salva.g) return prender(salva.g, c);
    const n = MDI.janelas.length;
    return prender({ x: 40 + n * 28, y: 30 + n * 26,
      w: Math.min(880, Math.round(c.width * 0.62)),
      h: Math.min(560, Math.round(c.height * 0.68)) }, c);
  }

  /** Prende a janela dentro da area visivel.
   *
   *  Uma janela gravada em x=4800 numa pagina que hoje tem 1400 e uma janela
   *  perdida: ela existe, ocupa memoria e ninguem a ve para fechar. Prender e
   *  o unico jeito de a restauracao nao virar armadilha -- e quem prende, diz. */
  function prender(g, c) {
    const w = Math.max(280, Math.min(g.w, Math.round(c.width)));
    const h = Math.max(160, Math.min(g.h, Math.round(c.height)));
    const x = Math.max(0, Math.min(g.x, Math.round(c.width) - w));
    const y = Math.max(0, Math.min(g.y, Math.round(c.height) - h));
    if (x !== g.x || y !== g.y || w !== g.w || h !== g.h) {
      avisar("a janela solta não cabia onde estava guardada — foi presa dentro da área visível");
    }
    return { x, y, w, h };
  }

  function ligarJanela(t, j) {
    const topo = j.querySelector(".jan-topo");
    j.addEventListener("mousedown", () => {
      j.style.zIndex = String(++MDI.z);
      for (const x of MDI.janelas) if (x.janela) x.janela.classList.remove("frente");
      j.classList.add("frente");
      if (t !== W.foco) focar(t);
    }, true);

    j.addEventListener("click", ev => {
      const b = ev.target.closest("[data-jan]");
      if (!b) return;
      const a = b.dataset.jan;
      if (a === "acoplar") acoplar(t);
      else if (a === "fechar") { acoplar(t); fechar(t); }
      else if (a === "pino") pinarSolta(t);
    });

    // Mover: SO pelo cabecalho.
    topo.addEventListener("mousedown", ev => {
      if (ev.target.closest("[data-jan]")) return;
      const c = j.getBoundingClientRect();
      const pai = j.parentElement.getBoundingClientRect();
      const dx = ev.clientX - c.left, dy = ev.clientY - c.top;
      const anda = e => {
        j.style.left = Math.max(0, Math.min(e.clientX - pai.left - dx,
          pai.width - c.width)) + "px";
        j.style.top = Math.max(0, Math.min(e.clientY - pai.top - dy,
          pai.height - c.height)) + "px";
      };
      const solta = () => {
        document.removeEventListener("mousemove", anda);
        document.removeEventListener("mouseup", solta);
        guardar();
      };
      document.addEventListener("mousemove", anda);
      document.addEventListener("mouseup", solta);
      ev.preventDefault();
    });

    // Redimensionar pelo canto.
    j.querySelector(".jan-canto").addEventListener("mousedown", ev => {
      const c = j.getBoundingClientRect();
      const x0 = ev.clientX, y0 = ev.clientY;
      const anda = e => {
        j.style.width = Math.max(280, c.width + e.clientX - x0) + "px";
        j.style.height = Math.max(160, c.height + e.clientY - y0) + "px";
      };
      const solta = () => {
        document.removeEventListener("mousemove", anda);
        document.removeEventListener("mouseup", solta);
        guardar();
      };
      document.addEventListener("mousemove", anda);
      document.addEventListener("mouseup", solta);
      ev.preventDefault();
      ev.stopPropagation();
    });
  }

  /** Devolve a janela solta para a area em regioes, com o estado inteiro. */
  function acoplar(t, destino) {
    if (!t || !t.janela) return;
    const p = t.el.querySelector(".painel");
    const rolagem = p ? p.scrollTop : 0;
    if (t.pausar) { try { t.pausar(); } catch (e) { /* laco ja morto */ } }
    const r = destino || W.regioes[0];
    const j = t.janela;
    t.janela = null;
    MDI.janelas = MDI.janelas.filter(x => x !== t);
    j.remove();
    r.abas.push(t);
    mostrar(r, t);
    if (p) p.scrollTop = rolagem;
    if (W.foco !== t) focar(t); else desenhar();
    guardar();
  }

  function pinarSolta(t) {
    if (!t || !t.janela) return;
    if (!t.chave) return avisar("esta tela não tem endereço próprio para pinar", true);
    t.pino = !t.pino;
    guardar();
    t.janela.querySelector('[data-jan="pino"]')
      .setAttribute("aria-pressed", t.pino ? "true" : "false");
    avisar(t.pino
      ? "esta janela volta solta, nesta posição e neste tamanho, na próxima abertura"
      : "esta janela deixa de voltar sozinha");
  }

  const fecharAtiva = () => fechar(W.foco);
  const podeFechar = () => {
    const r = regiaoDe(W.foco);
    return !!(r && r.abas.length > 1);
  };

  /** A tela que conta o que este modo faz e o que ele NAO faz.
   *
   *  Recusa fundamentada e resultado: o docking por arrasto de janela nao
   *  existe em navegador nenhum, e dizer isso aqui vale mais que um botao
   *  que nao funciona. */
  async function telaAjuda() {
    const d = await monitores();
    const cortes = await emendas();
    const cont = doc("#regioes");
    const l = cont ? Math.round(cont.getBoundingClientRect().width) : 0;
    const linhas = d ? d.screens.map(s =>
      `<tr><td class="dado">${E(s.label || "—")}</td>
         <td class="num">${s.width}×${s.height}</td>
         <td class="num">${s.left},${s.top}</td>
         <td class="num">${s.devicePixelRatio}×</td>
         <td>${s.isPrimary ? '<span class="pino ok">principal</span>' : ""}</td></tr>`).join("")
      : "";

    folha("Multitela", "abas vivas, regiões lado a lado e janelas destacadas",
      nota() +
      `<div class="fichas">
         <div class="ficha"><div class="v">${W.regioes.length}</div>
           <div class="r">regiões abertas</div><div class="u">cabem ${maxRegioes()}</div></div>
         <div class="ficha"><div class="v">${todas().length}</div>
           <div class="r">abas vivas</div>
           <div class="u">${todas().filter(t => t.pino).length} pinada(s)</div></div>
         <div class="ficha"><div class="v">${l || "—"}</div>
           <div class="r">largura útil</div><div class="u">pixels CSS</div></div>
         <div class="ficha"><div class="v">${window.devicePixelRatio}×</div>
           <div class="r">densidade desta janela</div>
           <div class="u">devicePixelRatio</div></div>
       </div>

       <h3>Os monitores</h3>
       ${d ? `<div class="rolo"><table><thead><tr><th>monitor</th><th class="num">tamanho</th>
           <th class="num">canto</th><th class="num">DPI</th><th></th></tr></thead>
           <tbody>${linhas}</tbody></table></div>
         <p class="leg">${cortes.length
            ? `${cortes.length} emenda(s) física(s) dentro desta janela, a
               ${cortes.map(c => Math.round(c) + "px").join(" e ")} da borda
               esquerda da área de trabalho. <b>Alinhar</b> põe uma calha em
               cada uma, para nenhuma região ficar partida ao meio.`
            : "Esta janela está inteira dentro de um monitor só."}</p>
         <div class="barra-acao">
           <button class="botao consultar" id="mtAlinhar">Alinhar as regiões com os monitores</button>
         </div>`
        : `<div class="aviso"><b>Este navegador não expõe os monitores.</b>
             A <code>Window Management API</code> (<code>getScreenDetails</code>)
             existe no Chrome e no Edge, em contexto seguro — e
             <code>127.0.0.1</code> é contexto seguro. No Firefox e no Safari
             ela não existe: as regiões dividem em partes iguais, e a janela
             destacada abre onde o navegador quiser.</div>`}

       <h3>O que este modo NÃO faz</h3>
       <div class="nota">
         <p><b>Arrastar uma janela do sistema de volta para a barra de abas.</b>
         O navegador não recebe evento nenhum quando uma janela passa por cima
         de outra — o docking por arrasto do WINDEV(R) e do Visual Studio(R)
         não é implementável aqui. Use <b>⤺ devolver</b>, na janela destacada.</p>
         <p><b>Reabrir sozinho as janelas destacadas.</b>
         <code>window.open</code> sem clique é bloqueio de popup em todo
         navegador. O arranjo fica guardado; volta com um clique.</p>
         <p><b>Guardar a sessão no disco do navegador.</b> A ficha de sessão
         viaja pelo <code>BroadcastChannel</code>, em memória. Se a janela
         principal fechar, a destacada pede login — e isso é de propósito.</p>
       </div>
       <p class="leg">Regiões, larguras e abas pinadas ficam
       <b>neste navegador</b> — não no servidor. Desenho completo em
       <code>docs/MULTITELA.md</code>.</p>`);

    const bt = doc("#mtAlinhar");
    if (bt) bt.onclick = () => alinharComOsMonitores();
  }

  /** Abre a tela pedida na URL (janela destacada). */
  async function abrirRota() {
    const r = W.rota || rotaDaUrl();
    if (!r.tela) return;
    await abrir(r.tela, { db: r.db, tab: r.tab });
  }

  return {
    iniciar, restaurar, abrir, abrirRota, novaAba, fechar, fecharAtiva,
    podeFechar, focar, soltar, acoplar,
    dividir, renomear, laco, pararLaco, alternarPino, destacar, devolver,
    pinarJanela, alinharComOsMonitores, avisarArvore, pedirSessao, rotaDaUrl,
    nota, telaAjuda, temApiDeTelas, monitores, emendas, maxRegioes, marcar,
    destacadaAqui: () => W.destacada,
    // Expostos para o exercicio automatizado poder olhar por dentro sem
    // depender do desenho da tela -- o mesmo caminho da integracao com a
    // Claude, e pelo mesmo motivo.
    _W: W, _CATALOGO: CATALOGO, _DA_TELA: DA_TELA, _MIN_REGIAO: MIN_REGIAO,
    _CHAVE: CHAVE, _lido: lido, _guardar: guardar, _todas: todas,
    _feicoesDaJanela: feicoesDaJanela, _prender: prender,
  };
})();

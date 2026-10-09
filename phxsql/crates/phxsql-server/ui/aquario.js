/* Aquario do PhxSql — a fisica das bolhas (pedido 707, fatia A9).
 *
 * Cada tarefa viva do servidor e uma BOLHA: nasce pequena, cresce com o tempo
 * que leva, colide com as vizinhas sem se sobrepor e ESTOURA quando a tarefa
 * some do retrato. Faixas horizontais por operacao seguram as bolhas no seu
 * canto sem prende-las.
 *
 * ## O modulo nao fala com o servidor
 *
 * Recebe a lista de tarefas por `atualizar(tarefas)`. Ligar isto ao servidor
 * e a fatia A10; aqui o retrato e inventado, e e assim que a fisica pode ser
 * exercitada no navegador sem servidor nenhum.
 *
 * ## O que a TV mostra (decisao do dono)
 *
 * So OPERACAO, TABELA e COR, com o usuario PSEUDONIMIZADO. Por isso o modulo
 * le da tarefa exatamente cinco campos — id, op, tabela, cor, ms — mais
 * `pseudo`, e NUNCA toca em `login`, `ip` ou `quem`: o que nao e lido nao
 * vaza, nem por engano de uma tela futura. Quem pseudonimiza e o servidor.
 *
 * ## Quatro cuidados
 *
 * 1. Sem sobreposicao e INVARIANTE, nao tendencia: depois da integracao rodam
 *    varias passagens de relaxamento, e se mesmo assim a area das bolhas nao
 *    cabe na caixa, os raios ENCOLHEM (`escala`) em vez de se sobreporem.
 *    Mentir o tamanho relativo e menos grave que mentir quem esta em cima de
 *    quem; o tamanho relativo continua valendo.
 * 2. Nada e recriado por volta: um <g> por tarefa, achado pelo id; o laco so
 *    muda atributos. Clique no meio de uma volta nao cai no vazio.
 * 3. A cor nao e o unico sinal: forma e traco acompanham (circulo, losango,
 *    octogono, tracejado), como no `telemetria.js`. Contorno, nunca fundo
 *    cheio.
 * 4. O CSS global morde: tudo escopado em `.aq`, e o `text-transform` e
 *    desfeito — «Blumenau» continua «Blumenau».
 *
 * ## Texto
 *
 * Rotulo de tela vem pela funcao `t(chave)` que o chamador entrega (a fabrica
 * de idiomas, `aquario.*`). Sem ela, o PADRAO abaixo (portugues) serve so de
 * rede — as chaves estao registradas na lacuna do A9 ate a fabrica entrar.
 * Operacao, tabela e pseudonimo sao DADO: entram como vieram.
 */
"use strict";

window.PhxAquario = (function () {

  const SVG = "http://www.w3.org/2000/svg";

  /* Chaves da fabrica e o texto de rede. */
  const TEXTOS = {
    "aquario.faixa.select": "consulta",
    "aquario.faixa.insert": "inclusão",
    "aquario.faixa.update": "alteração",
    "aquario.faixa.delete": "exclusão",
    "aquario.faixa.outras": "outras",
    "aquario.vazio": "nenhuma tarefa em andamento",
    "aquario.cor.verde": "normal",
    "aquario.cor.azul_claro": "grande, normal",
    "aquario.cor.azul_escuro": "grande, acima do habitual",
    "aquario.cor.amarelo": "aviso",
    "aquario.cor.vermelho": "alarme",
    "aquario.cor.rosa": "encerrando",
  };

  /* Faixas, de cima para baixo. A operacao desconhecida cai em `outras`. */
  const FAIXAS = ["select", "insert", "update", "delete", "outras"];

  /* cor -> { forma, variavel, tracejado } (aquario-707.md §2.3). */
  const CORES = {
    verde:       { forma: "circulo",  v: "--ok",            fb: "#6cc98c", traco: "",    esp: 1.5 },
    azul_claro:  { forma: "circulo",  v: "--reg",           fb: "#5fa6e8", traco: "",    esp: 2 },
    azul_escuro: { forma: "circulo",  v: "--aq-azul-escuro", fb: "#9cc3ff", traco: "",   esp: 3, dupla: true },
    amarelo:     { forma: "losango",  v: "--ambar",         fb: "#ffc43d", traco: "6 4", esp: 2 },
    vermelho:    { forma: "octogono", v: "--vermelho",      fb: "#ff5f5f", traco: "2 3", esp: 3 },
    rosa:        { forma: "circulo",  v: "--acao-marcar",   fb: "#ff8fc7", traco: "10 4", esp: 2 },
  };

  const M = {
    rMin: 9,            // px, bolha recem-nascida que ja existe
    rMax: 46,           // px, a maior (tarefa de 30 s ou mais)
    msGrande: 30000,    // tarefa que atinge rMax
    cresce: 2.2,        // 1/s, quanto a bolha corre para o raio-alvo
    ocupacaoMax: 0.46,  // fracao da caixa que as bolhas podem cobrir
    molaFaixa: 5,       // 1/s², puxao suave ate a faixa
    atrito: 1.6,        // 1/s
    quique: 0.35,
    passagens: 40,      // relaxamentos por quadro (sai cedo quando nada se move)
    estouroMs: 380,
    folga: 0.2,         // px entre bordas depois de resolver
    rotuloMin: 15,      // so escreve dentro da bolha acima deste raio
  };

  /* Gerador deterministico: a prova precisa repetir. */
  function prng(semente) {
    let s = semente >>> 0 || 1;
    return function () {
      s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0;
      return s / 4294967296;
    };
  }

  function faixaDe(op) {
    const o = String(op || "").toLowerCase();
    return FAIXAS.indexOf(o) >= 0 ? o : "outras";
  }

  /* Raio-alvo: logaritmico, porque o que interessa e a diferenca entre 5 ms e
   * 5 s, nao entre 30 s e 31 s. */
  function raioAlvo(ms) {
    const f = Math.min(1, Math.log(1 + Math.max(0, ms || 0)) / Math.log(1 + M.msGrande));
    return M.rMin + (M.rMax - M.rMin) * f;
  }

  /* --------------------------------------------------------------- fisica */

  /* Sobreposicao entre duas bolhas vivas: o quanto os raios entram um no
   * outro. E a MEDIDA da prova — a mesma funcao que o teste chama. */
  function sobreposicoes(bolhas) {
    let max = 0, n = 0;
    for (let i = 0; i < bolhas.length; i++) {
      const a = bolhas[i];
      if (a.estourando) continue;
      for (let j = i + 1; j < bolhas.length; j++) {
        const b = bolhas[j];
        if (b.estourando) continue;
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        const o = a.r + b.r - d;
        if (o > 0.5) { n++; if (o > max) max = o; }
      }
    }
    return { pares: n, max: max };
  }

  /* Uma passada de resolucao de contatos, com grade espacial (150 bolhas
   * fariam 11 mil pares; com a grade fazem algumas centenas). */
  function resolver(vivas, caixa) {
    const cel = M.rMax * 2 + 2;
    const grade = new Map();
    for (let i = 0; i < vivas.length; i++) {
      const b = vivas[i];
      const k = Math.floor(b.x / cel) * 4096 + Math.floor(b.y / cel);
      let l = grade.get(k);
      if (!l) grade.set(k, l = []);
      l.push(i);
    }
    let moveu = false;
    for (let i = 0; i < vivas.length; i++) {
      const a = vivas[i];
      const cx = Math.floor(a.x / cel), cy = Math.floor(a.y / cel);
      for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) {
        const l = grade.get((cx + dx) * 4096 + (cy + dy));
        if (!l) continue;
        for (let q = 0; q < l.length; q++) {
          const j = l[q];
          if (j <= i) continue;
          const b = vivas[j];
          let ex = b.x - a.x, ey = b.y - a.y;
          let d = Math.hypot(ex, ey);
          const alvo = a.r + b.r + M.folga;
          if (d >= alvo) continue;
          if (d < 1e-6) { ex = 1; ey = 0; d = 1e-6; } else { ex /= d; ey /= d; }
          const ma = a.r * a.r, mb = b.r * b.r, mt = ma + mb;
          const sobra = alvo - d;
          a.x -= ex * sobra * (mb / mt); a.y -= ey * sobra * (mb / mt);
          b.x += ex * sobra * (ma / mt); b.y += ey * sobra * (ma / mt);
          // o impulso: so a componente que aproxima as duas e trocada
          const vr = (b.vx - a.vx) * ex + (b.vy - a.vy) * ey;
          if (vr < 0) {
            const imp = -(1 + M.quique) * vr / (1 / ma + 1 / mb);
            a.vx -= ex * imp / ma; a.vy -= ey * imp / ma;
            b.vx += ex * imp / mb; b.vy += ey * imp / mb;
          }
          moveu = true;
        }
      }
    }
    // as paredes entram na MESMA passagem: empurrar uma bolha contra a parede
    // e depois fecha-la nela e como se produz sobreposicao calada
    for (let i = 0; i < vivas.length; i++) {
      const b = vivas[i];
      if (b.x < b.r) { b.x = b.r; if (b.vx < 0) b.vx *= -M.quique; }
      else if (b.x > caixa.w - b.r) { b.x = caixa.w - b.r; if (b.vx > 0) b.vx *= -M.quique; }
      if (b.y < b.r) { b.y = b.r; if (b.vy < 0) b.vy *= -M.quique; }
      else if (b.y > caixa.h - b.r) { b.y = caixa.h - b.r; if (b.vy > 0) b.vy *= -M.quique; }
    }
    return moveu;
  }

  /* O motor, separado do DOM: o teste o roda sem desenhar nada. */
  function criarMotor(caixa, opcoes) {
    const op = opcoes || {};
    const e = {
      caixa: caixa,
      colisao: op.colisao !== false,
      bolhas: [],           // vivas e estourando
      porId: new Map(),
      escala: 1,
      rnd: prng(op.semente || 707),
      tempo: 0,
    };

    function yDaFaixa(f) {
      const i = FAIXAS.indexOf(f);
      return (i + 0.5) * e.caixa.h / FAIXAS.length;
    }

    e.atualizar = function (tarefas) {
      const vistos = new Set();
      for (const t of tarefas || []) {
        const id = String(t.id);
        vistos.add(id);
        let b = e.porId.get(id);
        const faixa = faixaDe(t.op);
        if (!b) {
          b = {
            id: id, x: 0, y: 0, vx: 0, vy: 0, r: 0, alvo: 0,
            estourando: false, idade: 0,
          };
          // nasce na sua faixa, em x sorteado — nunca fora da caixa
          // 12 candidatos, fica o mais folgado: nascer em cima de outra
          // bolha e o que a relaxacao leva mais quadros para desfazer
          let melhor = -1;
          for (let c = 0; c < 12; c++) {
            const cx = M.rMax + e.rnd() * Math.max(1, e.caixa.w - 2 * M.rMax);
            const cy = yDaFaixa(faixa) + (e.rnd() - 0.5) * e.caixa.h / FAIXAS.length;
            let folga = 1e9;
            for (const o of e.bolhas) {
              if (!o.estourando) folga = Math.min(folga, Math.hypot(o.x - cx, o.y - cy) - o.alvo);
            }
            if (folga > melhor) { melhor = folga; b.x = cx; b.y = cy; }
          }
          e.bolhas.push(b);
          e.porId.set(id, b);
        }
        // so estes campos do retrato; login e IP nao existem para este modulo
        b.op = t.op; b.tabela = t.tabela; b.pseudo = t.pseudo;
        b.cor = CORES[t.cor] ? t.cor : "verde";
        b.faixa = faixa; b.ms = t.ms || 0;
        b.bruto = raioAlvo(b.ms);
      }
      for (const b of e.bolhas) {
        if (!b.estourando && !vistos.has(b.id)) {
          b.estourando = true; b.idade = 0; b.rEstouro = b.r;
          e.porId.delete(b.id);
        }
      }
    };

    /* Se a soma das areas passa da fracao permitida, todos os raios encolhem
     * na mesma razao: o relativo fica, a sobreposicao nao nasce. */
    function recalcularEscala(vivas) {
      let area = 0;
      for (const b of vivas) area += Math.PI * b.bruto * b.bruto;
      const livre = e.caixa.w * e.caixa.h * M.ocupacaoMax;
      const alvo = area > livre ? Math.sqrt(livre / area) : 1;
      e.escala += (alvo - e.escala) * 0.2;   // sem tranco quando a lista muda
    }

    e.passo = function (dt) {
      dt = Math.min(dt, 1 / 30);
      e.tempo += dt;
      const vivas = [];
      for (const b of e.bolhas) if (!b.estourando) vivas.push(b);
      recalcularEscala(vivas);

      for (const b of vivas) {
        b.alvo = b.bruto * e.escala;
        b.r += (b.alvo - b.r) * Math.min(1, M.cresce * dt);
        if (b.r > b.alvo) b.r = b.alvo;         // encolher e imediato: nunca invade
        b.vy += (yDaFaixa(b.faixa) - b.y) * M.molaFaixa * dt;
        b.vx += (e.rnd() - 0.5) * 60 * dt;       // marola, para nao parar de vez
        b.vy += (e.rnd() - 0.5) * 60 * dt;
        const f = Math.exp(-M.atrito * dt);
        b.vx *= f; b.vy *= f;
        b.x += b.vx * dt; b.y += b.vy * dt;
      }
      if (e.colisao) {
        for (let k = 0; k < M.passagens; k++) if (!resolver(vivas, e.caixa)) break;
      } else {
        for (const b of vivas) {   // sem colisao ainda ha parede
          b.x = Math.min(Math.max(b.x, b.r), e.caixa.w - b.r);
          b.y = Math.min(Math.max(b.y, b.r), e.caixa.h - b.r);
        }
      }
      for (let i = e.bolhas.length - 1; i >= 0; i--) {
        const b = e.bolhas[i];
        if (!b.estourando) continue;
        b.idade += dt * 1000;
        if (b.idade >= M.estouroMs) e.bolhas.splice(i, 1);
      }
    };

    e.sobreposicoes = function () { return sobreposicoes(e.bolhas); };
    return e;
  }

  /* ---------------------------------------------------------------- desenho */

  const CSS = `
.aq{position:relative;width:100%;height:100%;min-height:240px;overflow:hidden;background:var(--fundo,#010418);
  font-family:"Exo 2",system-ui,sans-serif;color:var(--texto,#e6ecf7)}
.aq svg{display:block;width:100%;height:100%}
.aq .aq-faixa{stroke:var(--realce,#152238);stroke-width:1;stroke-dasharray:2 6}
.aq .aq-faixa-nome{fill:var(--mudo,#8b98b4);font-size:11px;letter-spacing:.04em;text-transform:none}
.aq .aq-b{text-transform:none}
.aq .aq-b .aq-forma{fill-opacity:.14}
.aq .aq-b .aq-dupla{fill:none}
.aq .aq-b .aq-rot{fill:var(--texto,#e6ecf7);text-anchor:middle;font-size:10px;pointer-events:none;
  paint-order:stroke;stroke:var(--fundo,#010418);stroke-width:2px;text-transform:none}
.aq .aq-b.aq-fim .aq-forma{fill-opacity:0}
.aq .aq-vazio{position:absolute;inset:0;display:none;align-items:center;justify-content:center;
  color:var(--mudo,#8b98b4)}
.aq.aq-sem .aq-vazio{display:flex}
`;

  function el(nome, attrs) {
    const n = document.createElementNS(SVG, nome);
    if (attrs) for (const k in attrs) n.setAttribute(k, attrs[k]);
    return n;
  }

  function pontos(forma, r) {
    if (forma === "losango") return `0,${-r} ${r},0 0,${r} ${-r},0`;
    const k = r * 0.4142, p = [[-k, -r], [k, -r], [r, -k], [r, k], [k, r], [-k, r], [-r, k], [-r, -k]];
    return p.map(q => q[0].toFixed(1) + "," + q[1].toFixed(1)).join(" ");
  }

  /** `criar(host, { t, auto, colisao, semente })` — devolve o aquario. */
  function criar(host, opcoes) {
    const op = opcoes || {};
    const t = op.t || function (c) { return TEXTOS[c] || c; };
    if (!document.getElementById("aq-css")) {
      const s = document.createElement("style");
      s.id = "aq-css"; s.textContent = CSS; document.head.appendChild(s);
    }
    host.classList.add("aq");
    const svg = el("svg", { role: "img" });
    const gFaixas = el("g"), gBolhas = el("g");
    svg.append(gFaixas, gBolhas);
    const vazio = document.createElement("div");
    vazio.className = "aq-vazio"; vazio.textContent = t("aquario.vazio");
    host.append(svg, vazio);

    const caixa = { w: host.clientWidth || 800, h: host.clientHeight || 400 };
    const motor = criarMotor(caixa, op);
    const nos = new Map();     // id -> { g, forma, dupla, rot, tit }
    const stats = { quadros: 0, msFrame: 0 };

    function medir() {
      caixa.w = host.clientWidth || caixa.w; caixa.h = host.clientHeight || caixa.h;
      svg.setAttribute("viewBox", `0 0 ${caixa.w} ${caixa.h}`);
      gFaixas.textContent = "";
      FAIXAS.forEach(function (f, i) {
        const y = (i + 0.5) * caixa.h / FAIXAS.length;
        gFaixas.append(el("line", { class: "aq-faixa", x1: 0, x2: caixa.w, y1: y, y2: y }));
        const n = el("text", { class: "aq-faixa-nome", x: 8, y: y - 4 });
        n.textContent = t("aquario.faixa." + f);   // rotulo se estiliza; e chave
        gFaixas.append(n);
      });
    }
    medir();
    if (window.ResizeObserver) new ResizeObserver(medir).observe(host);

    function noDe(b) {
      let n = nos.get(b.id);
      if (n) return n;
      const g = el("g", { class: "aq-b", "data-id": b.id });
      const tit = el("title");
      const forma = el(CORES[b.cor].forma === "circulo" ? "circle" : "polygon", { class: "aq-forma" });
      const dupla = el("circle", { class: "aq-dupla" });
      const rot = el("text", { class: "aq-rot", y: 3 });
      g.append(tit, forma, dupla, rot);
      gBolhas.append(g);
      n = { g: g, forma: forma, dupla: dupla, rot: rot, tit: tit, cor: b.cor };
      nos.set(b.id, n);
      return n;
    }

    function desenhar() {
      const vivas = new Set();
      for (const b of motor.bolhas) {
        vivas.add(b.id);
        const n = noDe(b);
        const c = CORES[b.cor];
        const r = b.estourando ? b.rEstouro * (1 + 0.6 * b.idade / M.estouroMs) : b.r;
        if (r <= 0.3) { n.g.style.display = "none"; continue; }
        n.g.style.display = "";
        n.g.setAttribute("transform", `translate(${b.x.toFixed(1)} ${b.y.toFixed(1)})`);
        if (n.cor !== b.cor) { n.cor = b.cor; }    // a forma muda por pontos abaixo
        const cor = `var(${c.v},${c.fb})`;
        n.forma.setAttribute("stroke", cor);
        n.forma.setAttribute("fill", cor);
        n.forma.setAttribute("stroke-width", c.esp);
        if (c.traco) n.forma.setAttribute("stroke-dasharray", c.traco);
        else n.forma.removeAttribute("stroke-dasharray");
        if (c.forma === "circulo") n.forma.setAttribute("r", r.toFixed(1));
        else n.forma.setAttribute("points", pontos(c.forma, r));
        // borda dupla clara do azul escuro: e o que o torna visivel (3,04:1)
        if (c.dupla) {
          n.dupla.setAttribute("r", Math.max(0, r - 5).toFixed(1));
          n.dupla.setAttribute("stroke", cor); n.dupla.setAttribute("stroke-width", 1.5);
          n.dupla.style.display = "";
        } else n.dupla.style.display = "none";
        n.g.setAttribute("opacity", b.estourando ? (1 - b.idade / M.estouroMs).toFixed(2) : "1");
        n.g.classList.toggle("aq-fim", !!b.estourando);
        if (!b.estourando) {
          // rotulo e DADO: texto cru, sem caixa alta imposta
          n.rot.textContent = r >= M.rotuloMin ? String(b.tabela || b.op || "") : "";
          const tx = String(b.op || "") + " " + String(b.tabela || "") +
            " (" + t("aquario.cor." + b.cor) + ")" + (b.pseudo ? " " + b.pseudo : "");
          n.tit.textContent = tx;
          n.g.setAttribute("aria-label", tx);
        }
      }
      for (const [id, n] of nos) {
        if (!vivas.has(id)) { n.g.remove(); nos.delete(id); }
      }
      host.classList.toggle("aq-sem", motor.bolhas.length === 0);
    }

    let ultimo = 0, raf = 0;
    function quadro(agora) {
      const t0 = performance.now();
      const dt = ultimo ? (agora - ultimo) / 1000 : 1 / 60;
      ultimo = agora;
      motor.passo(dt);
      desenhar();
      stats.quadros++;
      stats.msFrame += performance.now() - t0;
      raf = requestAnimationFrame(quadro);
    }

    const api = {
      motor: motor,
      stats: stats,
      atualizar: function (tarefas) { motor.atualizar(tarefas); },
      /* um passo manual (teste): fisica + desenho */
      passo: function (dt) { motor.passo(dt); desenhar(); },
      sobreposicoes: function () { return motor.sobreposicoes(); },
      nosVivos: function () { return nos.size; },
      parar: function () { cancelAnimationFrame(raf); raf = 0; },
      iniciar: function () { if (!raf) { ultimo = 0; raf = requestAnimationFrame(quadro); } },
    };
    if (op.auto !== false) api.iniciar();
    return api;
  }

  return { criar: criar, criarMotor: criarMotor, sobreposicoes: sobreposicoes, FAIXAS: FAIXAS, TEXTOS: TEXTOS };
})();

/* Telemetria do PhxSql — as faixas de séries e o painel de bolhas.
 *
 * O molde é o SQL Check da Idera(R): faixas de gráficos no topo, e embaixo um
 * painel grande em que cada atividade viva é uma BOLHA — o tamanho é o peso,
 * a cor é o estado, e clicar abre o descritivo inteiro.
 *
 * Arquivo próprio pelo mesmo motivo do `diagrama-er.js`: o empacotamento das
 * bolhas e o desenho das séries são ALGORITMO, e algoritmo não deveria morar
 * no meio de oito mil linhas de tela.
 *
 * ## O módulo não fala com o servidor
 *
 * Ele recebe uma função `api(op, params)` de quem o chama. Foi assim que ele
 * pôde ser exercitado no navegador sem servidor nenhum, com um retrato
 * inventado — e é assim que ele continua podendo.
 *
 * ## Quatro cuidados que só aparecem exercitando
 *
 * 1. **Nada pisca.** O laço não redesenha o painel: ele ATUALIZA os elementos
 *    que já existem, um `<g>` por atividade, achado pelo id. Redesenhar tudo
 *    a cada volta faria o clique do operador cair no vazio a cada dois
 *    segundos, e o cartão aberto fecharia sozinho.
 *
 * 1b. **A caixa segue o desenho.** O empacotamento é por tangência, contra a
 *    RAZÃO da caixa, e o painel encolhe quando há pouca bolha. A primeira
 *    versão era uma espiral em torno do centro numa caixa fixa de 340 px, e
 *    o resultado foi a queixa do dono: uma bolha média no meio de um
 *    retângulo de mil pixels quase todo escuro. Nenhuma conta de raio
 *    resolve isso — só a caixa.
 *
 * 2. **O CSS global morde.** `input{width:100%}` e
 *    `label{text-transform:uppercase}` valem na página inteira e estragam
 *    componente de tela nova — a lição já está escrita no CLAUDE.md, com o
 *    «BLUMENAU» de exemplo. Por isso tudo aqui é escopado em `.tlm` e os dois
 *    são desfeitos explicitamente.
 *
 * 3. **A cor não é o único sinal.** Azul, amarelo e vermelho dizem o estado, e
 *    junto com eles vão o traço da borda (contínuo, tracejado, duplo), a
 *    palavra do estado no rótulo e o `aria-label`. Quem não distingue as três
 *    cores continua lendo o painel.
 */
"use strict";

window.PhxTelemetria = (function () {

  /* ------------------------------------------------------------- medidas */

  const M = {
    /* A MENOR bolha, em fração do raio da maior.
     *
     * Não é um piso em pixels de propósito. Piso absoluto briga com a caixa —
     * num painel de celular ele viraria a bolha inteira —, e o tamanho em
     * pixels só é decidido DEPOIS de saber quantas fileiras cabem. Como
     * razão, ele sobrevive a esse ajuste.
     *
     * O valor sai do rótulo: a 26% da maior, a menor ainda cabe o «#17» ou o
     * «w·a1b2» dentro. Bolha sem rótulo não diz quem é, que é a única coisa
     * que ela existe para dizer. */
    razaoMin: 0.26,
    /** Espaço horizontal mínimo entre duas bolhas, em fração do raio maior. */
    folga: 0.14,
    /* Respiro entre o desenho e a borda da caixa, em pixels.
     *
     * Ele não é só estética: a SOMBRA projetada sai da esfera para a direita
     * e para baixo, e o SVG corta no seu próprio limite. Com 10 px a bolha da
     * ponta ficava inteira e a sombra dela aparecia decepada — a conferência
     * de geometria dizia «tudo dentro» porque media o CÍRCULO, e o que
     * vazava era o filtro. Aqui cabem os ~7 px do `feDropShadow`. */
    margem: 15,
    /* Passo entre duas fileiras, em fração da soma dos raios. Abaixo de 1 as
     * fileiras se encavalam, e é daí que sai a profundidade: a da frente
     * passa por cima da de trás, como na referência.
     *
     * 0,82 era demais, e quem mostrou foi a carga de verdade: com a fileira
     * da frente muito maior que a de trás, uma esfera grande cobria METADE
     * de duas pequenas e comia o rótulo delas. Profundidade que esconde o
     * dado deixou de ser profundidade. A 0,97 as fileiras se tocam e nada
     * fica ilegível. */
    passoFileira: 0.97,
    /* O teto do raio precisa das DUAS contas, e nenhuma das duas basta
     * sozinha: a fração impede a bolha de estourar um painel estreito, e o
     * absoluto impede que UMA atividade sozinha vire um disco de 400 px no
     * meio de um monitor grande. */
    tetoDaCaixa: 0.46,
    tetoAbsoluto: 150,
    /** A caixa segue o desenho, mas dentro destes limites. */
    altMin: 150,
    altMax: 380,
    /* Largura mínima do painel. Ele encolhe quando há pouca bolha — senão
     * uma atividade sozinha fica perdida num vazio de mil pixels —, mas
     * abaixo disto a legenda de estados quebra em coluna e fica ilegível. */
    largMin: 420,
    /* Teto de bolhas desenhadas. Acima disto o desenho já não se lê e a
     * animação começa a custar quadro. O resumo diz quantas ficaram de fora —
     * esconder sem dizer seria mentira de tela. */
    bolhasMax: 150,
    /* Acima disto a deriva desliga sozinha: cento e vinte esferas movendo a
     * sessenta quadros por segundo é trabalho de sobra para enfeite. */
    bolhasParaDeriva: 80,
    /** Amplitude da deriva, em pixels. Enfeite, e por isso pequena. */
    deriva: 3,
    /** Raio mínimo CLICÁVEL, em pixels — alvo menor que isto ninguém acerta. */
    raioClique: 11,
  };

  /* Os quatro níveis, com o que NÃO é cor junto de cada um.
   *
   * As cores saem das variáveis do console, e por isso escurecem sozinhas no
   * tema claro — o mesmo caminho do vermelhão da marca. Nenhuma delas é
   * escrita em hexadecimal aqui. */
  const NIVEIS = {
    normal:     { cor:"var(--reg)",      traco:"",      glifo:"",  rot:"normal" },
    alto:       { cor:"var(--ambar)",    traco:"6 4",   glifo:"▲", rot:"uso alto" },
    stress:     { cor:"var(--vermelho)", traco:"2 3",   glifo:"■", rot:"stress" },
    encerrando: { cor:"var(--acao-marcar)", traco:"10 4", glifo:"✕", rot:"encerrando" },
  };
  const ORDEM_NIVEL = { normal:0, alto:1, encerrando:2, stress:3 };

  const esc = t => String(t ?? "").replace(/[&<>"']/g, c =>
    ({ "&":"&amp;", "<":"&lt;", ">":"&gt;", '"':"&quot;", "'":"&#39;" }[c]));

  const num = v => {
    const n = Number(v);
    return Number.isFinite(n) ? n : 0;
  };

  /** Bytes em unidade que cabe no olho. */
  function bytes(v) {
    const n = num(v);
    if (n < 1024) return n.toFixed(0) + " B";
    if (n < 1048576) return (n / 1024).toFixed(1) + " kB";
    if (n < 1073741824) return (n / 1048576).toFixed(1) + " MB";
    return (n / 1073741824).toFixed(2) + " GB";
  }

  function dur(ms) {
    const n = num(ms);
    if (n < 1000) return n.toFixed(0) + " ms";
    if (n < 60000) return (n / 1000).toFixed(1) + " s";
    const m = Math.floor(n / 60000);
    return m + " min " + Math.floor((n % 60000) / 1000) + " s";
  }

  /* ------------------------------------------------------- o desenho da faixa
   *
   * Uma faixa é um gráfico pequeno: uma ou mais séries sobre o mesmo eixo,
   * empilhadas ou não. Desenhado à mão em SVG — o `viewBox` casa com a caixa,
   * então uma unidade de usuário é um pixel e a conta é direta.
   */

  const LARG = 320, ALT = 54;

  /** Um caminho de linha a partir de valores já normalizados em 0..1. */
  function caminho(vals, base) {
    if (!vals.length) return "";
    const dx = vals.length > 1 ? LARG / (vals.length - 1) : LARG;
    let d = "";
    vals.forEach((v, i) => {
      const x = (i * dx).toFixed(1);
      const y = (ALT - v * (ALT - 2) - 1).toFixed(1);
      d += (i ? "L" : "M") + x + " " + y + " ";
    });
    if (base) {
      d += `L${LARG} ${ALT} L0 ${ALT} Z`;
    }
    return d.trim();
  }

  /** Desenha uma faixa. `series` é [{nome, cor, vals}]; `empilhado` soma. */
  function faixa(alvo, cfg) {
    const series = cfg.series || [];
    const n = series.reduce((m, s) => Math.max(m, s.vals.length), 0);
    // O topo do eixo sai do MAIOR valor da janela, e nunca de um número fixo:
    // um teto fixo esconde o pico numa carga leve e achata tudo numa pesada.
    // O piso de 1 evita que uma janela toda zerada vire uma divisão por zero
    // e desenhe a linha no infinito.
    let topo = 0;
    if (cfg.empilhado) {
      for (let i = 0; i < n; i++) {
        let soma = 0;
        series.forEach(s => { soma += num(s.vals[i]); });
        topo = Math.max(topo, soma);
      }
    } else {
      series.forEach(s => s.vals.forEach(v => { topo = Math.max(topo, num(v)); }));
    }
    if (cfg.topoMinimo) topo = Math.max(topo, cfg.topoMinimo);
    if (topo <= 0) topo = 1;

    let svg = "";
    if (cfg.empilhado) {
      // Empilhado desenha de cima para baixo, cada área somando a anterior:
      // é o que faz a altura total ser o total, e não a maior das partes.
      const acum = new Array(n).fill(0);
      const camadas = [];
      series.forEach(s => {
        const vals = [];
        for (let i = 0; i < n; i++) {
          acum[i] += num(s.vals[i]);
          vals.push(acum[i] / topo);
        }
        camadas.push({ s, vals });
      });
      camadas.reverse().forEach(c => {
        svg += `<path d="${caminho(c.vals, true)}" fill="${c.s.cor}"
                  fill-opacity=".55" stroke="${c.s.cor}" stroke-width="1"/>`;
      });
    } else {
      series.forEach(s => {
        const vals = s.vals.map(v => num(v) / topo);
        svg += `<path d="${caminho(vals, false)}" fill="none" stroke="${s.cor}"
                  stroke-width="1.6" stroke-linejoin="round"
                  ${s.tracejado ? 'stroke-dasharray="4 3"' : ""}/>`;
      });
    }

    alvo.innerHTML =
      `<div class="tlm-faixa-cab">
         <span class="tlm-faixa-t">${esc(cfg.titulo)}</span>
         <span class="tlm-faixa-v">${esc(cfg.valor)}</span>
       </div>
       <svg viewBox="0 0 ${LARG} ${ALT}" preserveAspectRatio="none"
            role="img" aria-label="${esc(cfg.titulo)}: ${esc(cfg.valor)}">
         <line x1="0" y1="${ALT - 0.5}" x2="${LARG}" y2="${ALT - 0.5}"
               stroke="var(--linha)" stroke-width="1"/>
         ${svg}
       </svg>
       <div class="tlm-faixa-leg">${
         series.map(s => `<span><i style="background:${s.cor}"></i>${esc(s.nome)}</span>`).join("")
       }<span class="tlm-topo">pico ${esc(cfg.topoRot ? cfg.topoRot(topo) : topo.toFixed(1))}</span></div>`;
  }

  /* ----------------------------------------------------- o arranjo das bolhas
   *
   * ## O raio sai da RAIZ do peso
   *
   * O olho compara ÁREA de círculo. Usar o peso direto no raio faria uma
   * atividade duas vezes mais pesada parecer QUATRO vezes maior — é o erro
   * clássico do gráfico de bolha, e ele mente sempre para o lado do exagero.
   * Quem «consertar» isto para `peso / pesoMax` está trocando área por raio
   * sem perceber: fica escrito aqui para não acontecer.
   *
   * O piso `razaoMin` quebra a proporção de propósito nas mais leves, para o
   * rótulo caber dentro. A legenda de escala desenha o piso, porque proporção
   * quebrada em silêncio é mentira sobre o dado.
   */
  function raioRelativo(peso, pesoMax) {
    const p = Math.max(0, num(peso)) / Math.max(1, num(pesoMax));
    return M.razaoMin + (1 - M.razaoMin) * Math.sqrt(Math.min(1, p));
  }

  /* Um número estável em 0..1 tirado do identificador.
   *
   * É o que dá a cada bolha um desalinhamento e uma fase de deriva PRÓPRIOS e
   * SEMPRE OS MESMOS. Sorteio de verdade faria a mesma atividade pular de
   * lugar a cada volta — e alvo que pula é alvo que não se clica. */
  function semente(id) {
    let h = 2166136261;
    const s = String(id);
    for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
    return ((h >>> 0) % 100000) / 100000;
  }

  /* Arranjo em FILEIRAS, da frente para o fundo — o molde da referência.
   *
   * O SQL Check não empacota as bolhas coladas: ele as espalha numa bandeja,
   * em fileiras frouxas que se encavalam de leve, e a maior fica na frente.
   * Uma versão anterior desta tela empacotava por tangência, e ficava mais
   * apertada do que o molde pede — bonito, e não era o que o dono mostrou.
   *
   * **A ordem é o tamanho, e ela vira profundidade de graça.** A fileira da
   * frente recebe as mais pesadas; a de trás, as mais leves. Como o raio JÁ é
   * o peso, as de trás saem menores sozinhas e o olho lê perspectiva sem que
   * nenhuma escala falsa tenha sido aplicada. Encolher a de trás «por
   * perspectiva» seria mentir sobre o peso dela, e é exatamente o que este
   * arranjo evita.
   *
   * `k` é o raio da MAIOR em pixels — em unidades ela vale 1.
   */
  function fileiras(itens, larg, k) {
    const linhas = [];
    let linha = [], usada = 0;
    const util = larg - 2 * M.margem;
    for (const it of itens) {
      const w = (2 * it.raio + M.folga) * k;
      if (linha.length && usada + w > util) { linhas.push(linha); linha = []; usada = 0; }
      linha.push(it); usada += w;
      linha.usada = usada;
    }
    if (linha.length) linhas.push(linha);
    // Altura pedida: meia bolha da frente, os passos entre fileiras, meia
    // bolha do fundo — mais o respiro das duas bordas.
    let alt = 0, largMax = 0;
    linhas.forEach((l, i) => {
      l.rmax = l.reduce((m, x) => Math.max(m, x.raio), 0);
      largMax = Math.max(largMax, l.usada);
      if (i === 0) alt += l.rmax * k;
      else alt += (linhas[i - 1].rmax + l.rmax) * k * M.passoFileira;
      if (i === linhas.length - 1) alt += l.rmax * k;
    });
    return { linhas, alt: alt + 2 * M.margem, largMax: largMax + 2 * M.margem };
  }

  /** Põe cada bolha no seu lugar dentro da caixa, e devolve as fileiras. */
  function arrumar(itens, larg, alt, k) {
    const f = fileiras(itens, larg, k);
    // A fileira 0 é a da FRENTE, e a frente é embaixo.
    let y = alt - M.margem - f.linhas[0].rmax * k;
    f.linhas.forEach((l, i) => {
      if (i) y -= (f.linhas[i - 1].rmax + l.rmax) * k * M.passoFileira;
      // Sobra distribuída entre as bolhas: fileira justificada fica com cara
      // de bandeja, e não de grade.
      const sobra = Math.max(0, (larg - 2 * M.margem) - (l.usada - M.folga * k));
      const vao = l.length > 1 ? sobra / (l.length - 1) : 0;
      let x = M.margem;
      l.forEach((it, n) => {
        const r = it.raio * k;
        it.px = x + r;
        // Desalinhamento próprio e estável: sem ele a fileira vira régua, e
        // a referência mostra bolhas soltas, não alinhadas.
        // O desalinhamento nunca empurra a esfera para FORA: numa fileira só,
        // a altura da caixa é exatamente o diâmetro da maior, e meio pixel de
        // balanço já a cortava pela borda de baixo. Medido, não visto: a
        // conferência de geometria pegou 24 casos assim.
        const solto = y + (semente(it.d.id) - 0.5) * l.rmax * k * 0.34;
        it.py = Math.min(Math.max(solto, M.margem + r), alt - M.margem - r);
        it.pr = r;
        it.fileira = i;
        x += 2 * r + (n < l.length - 1 ? M.folga * k + vao : 0);
      });
    });
    return f;
  }

  /* Acha o MAIOR raio que ainda cabe na caixa.
   *
   * Busca binária, e não uma fórmula: o número de fileiras muda em degraus
   * conforme `k` cresce, então não há inversa fechada. Trinta voltas resolvem
   * até o pixel e custam nada — isto roda uma vez a cada duas voltas da tela,
   * não por quadro. */
  function maiorRaioQueCabe(itens, larg, altMax, teto) {
    let baixo = 3, cima = teto;
    for (let v = 0; v < 30; v++) {
      const meio = (baixo + cima) / 2;
      if (fileiras(itens, larg, meio).alt <= altMax) baixo = meio; else cima = meio;
    }
    return baixo;
  }

  /* ------------------------------------------------------------- as esferas
   *
   * A referência não desenha círculos chapados: são ESFERAS — brilho especular
   * no alto à esquerda, corpo na cor do estado, aro escuro embaixo à direita e
   * sombra projetada no chão. Isso se faz com `<radialGradient>` e um
   * `feDropShadow`, os dois nativos do SVG: nenhuma biblioteca, nenhum CDN.
   *
   * **SVG e não Canvas**, e o motivo é acerto de clique e leitor de tela: cada
   * bolha continua sendo um `<g>` focável, com `<title>` e `aria-label`, e o
   * navegador resolve o «em qual eu cliquei» sozinho. Em Canvas os dois
   * teriam de ser reescritos à mão. O preço é o número de nós, e por isso o
   * teto de `bolhasMax` — acima dele o desenho já não se lê de qualquer jeito.
   */
  function defsDasEsferas() {
    const g = (id, cor) =>
      `<radialGradient id="${id}" cx="34%" cy="28%" r="72%">
         <stop offset="0%" stop-color="#fff" stop-opacity=".85"/>
         <stop offset="26%" style="stop-color:${cor}" stop-opacity=".95"/>
         <stop offset="78%" style="stop-color:${cor}" stop-opacity="1"/>
         <stop offset="100%" stop-color="#000" stop-opacity=".45"/>
       </radialGradient>`;
    return `<defs>
      ${Object.entries(NIVEIS).map(([n, v]) => g("tlmEsfera-" + n, v.cor)).join("")}
      <linearGradient id="tlmChao" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" style="stop-color:var(--painel-2)"/>
        <stop offset="100%" style="stop-color:var(--painel)"/>
      </linearGradient>
      <filter id="tlmSombra" x="-30%" y="-30%" width="180%" height="180%">
        <feDropShadow dx="2" dy="3" stdDeviation="2.5" flood-color="#000"
                      flood-opacity=".38"/>
      </filter>
    </defs>`;
  }

  /* A cor do rótulo DENTRO da esfera, decidida pela luminância do corpo.
   *
   * Não dá para fixar «branco», que é o que a referência usa: lá as esferas
   * são azul-escuras, e aqui as quatro cores CLAREIAM no tema escuro
   * (`--ambar` é #ffc43d). Branco sobre elas dá menos de 2:1. Então a cor se
   * decide medindo, e o contorno na cor oposta cobre a variação do gradiente —
   * que é o mesmo truque do texto branco com borda da referência. */
  function tintaDoRotulo(corCss) {
    const d = document.createElement("span");
    d.style.color = corCss;
    document.body.appendChild(d);
    const v = (getComputedStyle(d).color.match(/[\d.]+/g) || [0, 0, 0]).map(Number);
    d.remove();
    const f = c => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
    const lum = 0.2126 * f(v[0]) + 0.7152 * f(v[1]) + 0.0722 * f(v[2]);
    // As DUAS razões, e ganha a maior. Um limiar escolhido a olho já errou
    // aqui: a 0,32 o `--vermelho` do tema escuro (#ff5f5f, luminância 0,303)
    // caía do lado do branco e dava **2,98:1** -- reprovado --, quando a
    // tinta escura sobre ele dá 6,38:1. Comparar as duas não tem como errar,
    // e continua valendo se alguém mexer nas cores do tema.
    const claro = 1.05 / (lum + 0.05);
    const escuro = (lum + 0.05) / (0.0055 + 0.05);
    return escuro >= claro
      ? { tinta: "#0b0d16", contorno: "rgba(255,255,255,.6)", razao: escuro }
      : { tinta: "#ffffff", contorno: "rgba(0,0,0,.6)", razao: claro };
  }

  /* ------------------------------------------------------------- a deriva
   *
   * A referência se mexe o tempo todo, e o dono pediu isso com todas as
   * letras. Mas nesta casa **coisa que se mexe significa alguma coisa**, e por
   * isso o movimento é de dois tipos e nenhum é sorteado:
   *
   * 1. a TRANSIÇÃO — a bolha desliza do lugar velho para o novo quando o peso,
   *    o estado ou a quantidade de atividades mudou. Isso é dado;
   * 2. a DERIVA — três pixels de balanço, com a fase tirada do identificador.
   *    É enfeite declarado, e por isso é pequena, sem sorteio e a primeira a
   *    ser desligada.
   *
   * **Alvo que se move é alvo que não se clica**, então a deriva morre quando
   * o ponteiro entra no painel — e as posições-alvo também congelam ali, para
   * a volta seguinte não puxar a bolha debaixo do cursor. Quem está mirando
   * uma bolha pequena consegue acertá-la.
   *
   * `prefers-reduced-motion` desliga tudo: quem pediu menos movimento ao
   * sistema pediu para todo mundo, e um painel de servidor não é exceção.
   */
  function menosMovimento() {
    return !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }

  function animar() {
    estado.quadro = null;
    const svg = $("#tlmBolhas");
    if (!svg) return;                      // a tela saiu: o laço morre junto
    const t = performance.now() / 1000;
    const parado = estado.ponteiroDentro || menosMovimento()
      || estado.pos.size > M.bolhasParaDeriva;
    // A deriva não liga nem desliga de supetão: ela sobe e desce, senão o
    // painel dá um tranco toda vez que o ponteiro cruza a borda. Mas ela MORRE
    // rápido e volta devagar, e a assimetria é de propósito: quem levou o
    // ponteiro até a bolha quer mirar AGORA, e quem tirou não tem pressa
    // nenhuma de ver o painel respirar de novo. Medido: parada em ~150 ms.
    estado.derivaViva += ((parado ? 0 : 1) - estado.derivaViva) * (parado ? 0.3 : 0.05);
    if (parado && estado.derivaViva < 0.004) estado.derivaViva = 0;
    let mexeu = false;
    estado.pos.forEach(p => {
      // Aproximação exponencial: rápido no começo, macio no fim.
      const dx = p.ax - p.x, dy = p.ay - p.y, dr = p.ar - p.r;
      if (Math.abs(dx) > 0.05 || Math.abs(dy) > 0.05 || Math.abs(dr) > 0.05) mexeu = true;
      p.x += dx * 0.14; p.y += dy * 0.14; p.r += dr * 0.14;
      const a = M.deriva * estado.derivaViva;
      const f = p.fase * 6.283;
      p.dx = a * Math.sin(t * 0.55 + f);
      p.dy = a * 0.72 * Math.cos(t * 0.41 + f * 1.7);
      porNoLugar(p);
    });
    if (mexeu || estado.derivaViva > 0.01) {
      estado.quadro = requestAnimationFrame(animar);
    }
  }

  /** Empurra a posição corrente de uma bolha para dentro do SVG. */
  function porNoLugar(p) {
    if (!p.g || !p.g.isConnected) return;
    const x = p.x + (p.dx || 0), y = p.y + (p.dy || 0);
    p.circulo.setAttribute("cx", x.toFixed(1));
    p.circulo.setAttribute("cy", y.toFixed(1));
    p.circulo.setAttribute("r", p.r.toFixed(1));
    p.alvo.setAttribute("cx", x.toFixed(1));
    p.alvo.setAttribute("cy", y.toFixed(1));
    p.alvo.setAttribute("r", Math.max(M.raioClique, p.r).toFixed(1));
    p.rotulo.setAttribute("x", x.toFixed(1));
    p.rotulo.setAttribute("y", (y + p.dy1).toFixed(1));
    p.sub.setAttribute("x", x.toFixed(1));
    p.sub.setAttribute("y", (y + p.dy2).toFixed(1));
  }

  function acordarQuadro() {
    if (!estado.quadro) estado.quadro = requestAnimationFrame(animar);
  }

  /* --------------------------------------------------------------- a tela */

  function html() {
    return `
<div class="tlm">
  <div class="tlm-topo-barra">
    <div class="tlm-estado" id="tlmEstado" role="status" aria-live="polite">—</div>
    <div class="tlm-botoes">
      <button class="botao consultar" id="tlmPausar" type="button">Pausar</button>
      <button class="botao secundario" id="tlmAgora" type="button">Atualizar agora</button>
      <button class="botao alterar" id="tlmLigar" type="button">Desligar coleta</button>
    </div>
  </div>

  <div class="tlm-faixas" id="tlmFaixas">
    <div class="tlm-faixa" id="tlmEsperas"></div>
    <div class="tlm-faixa" id="tlmDisco"></div>
    <div class="tlm-faixa" id="tlmCpu"></div>
    <div class="tlm-faixa" id="tlmVazao"></div>
    <div class="tlm-faixa" id="tlmCache"></div>
  </div>

  <div class="tlm-corpo">
    <div class="tlm-painel">
      <div class="tlm-painel-cab">
        <!-- A TRILHA é o caminho de volta. Descer de nível sem trilha é o
             mesmo que descer sem escada: quem entrou numa estação precisa
             ver onde está e como sair, e a tela não tem botão «voltar» do
             navegador para oferecer. -->
        <nav class="tlm-trilha" id="tlmTrilha" aria-label="onde você está"></nav>
        <div class="tlm-painel-fim">
          <!-- A busca é a «Search SPID…» da referência. Num servidor com
               quarenta conexões, achar a que dói pelo olho é sorte. -->
          <input id="tlmBusca" class="tlm-busca" type="search" autocomplete="off"
                 placeholder="procurar conexão, IP, usuário ou operação…"
                 aria-label="procurar entre as atividades">
          <button class="tlm-link" id="tlmLegenda" type="button"
                  aria-expanded="true">ocultar legenda</button>
        </div>
      </div>
      <div class="tlm-painel-s" id="tlmResumo"></div>
      <svg class="tlm-bolhas" id="tlmBolhas" role="group"
           aria-label="atividades vivas, uma bolha por atividade"></svg>
      <!-- A legenda diz a FAIXA, e não só a cor. «amarelo · uso alto» sozinho
           obriga quem olha a adivinhar acima de quanto; e o número que ela
           escreve vem do campo "limiares" da resposta, que é a mesma constante
           que decidiu a cor no servidor. Dois números para a mesma regra é
           como a tela acaba pintando o que o servidor não concorda. -->
      <div class="tlm-explica" id="tlmExplica">
      <div class="tlm-legenda">
        <span class="tlm-leg" data-n="normal"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>azul · <b>normal</b><i id="tlmFxNormal"></i></span>
        <span class="tlm-leg" data-n="alto"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>▲ amarelo · <b>uso alto</b><i id="tlmFxAlto"></i></span>
        <span class="tlm-leg" data-n="stress"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>■ vermelho · <b>stress</b><i id="tlmFxStress"></i></span>
        <span class="tlm-leg" data-n="encerrando"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>✕ rosa · <b>encerrando</b><i>marcada, esperando o ponto seguro</i></span>
      </div>
      <!-- A escala é o «eixo» que um gráfico de bolha tem: sem ela o tamanho
           é uma impressão, e não uma medida. Os três círculos saem da MESMA
           função que desenha o painel, então o piso das mais leves aparece
           desenhado em vez de ficar escondido. -->
      <div class="tlm-escala">
        <span class="tlm-escala-t">peso</span>
        <svg id="tlmEscala" viewBox="0 0 260 40" role="img"
             aria-label="escala: a área da bolha segue o peso"></svg>
        <span class="tlm-escala-n">a <b>área</b> segue o peso — milissegundos de
          servidor que a atividade já gastou. Escala reduzida; as mais leves
          têm piso, para o rótulo caber.</span>
      </div>
      </div>
    </div>
    <aside class="tlm-lado">
      <div class="tlm-cartao" id="tlmCartao">
        <div class="tlm-vazio">clique numa bolha para ver o descritivo completo</div>
      </div>
    </aside>
  </div>

  <details class="tlm-threads">
    <summary>Gestor de threads <span id="tlmThreadsN"></span></summary>
    <div class="rolo"><table class="tlm-tab" id="tlmThreads"></table></div>
  </details>
</div>`;
  }

  /* --------------------------------------------------------------- o laço */

  let estado = {
    api: null,
    timer: null,
    pausado: false,
    ligada: true,
    selecionada: null,
    ultimo: null,
    periodo: 2000,
    // Quando a última volta foi pedida, para medir o atraso de ponta a ponta.
    pedidoEm: 0,
    ultimaIdaVolta: 0,
    // Quando a última resposta BOA chegou, e quantas voltas falharam desde
    // então. É deste par que sai o aviso de tela velha.
    ultimoOkEm: 0,
    ultimoOkRotulo: "",
    falhas: 0,
    // A largura que o painel de bolhas ficou na volta passada. Guardada para
    // a histerese: sem ela o painel tremeria um pixel a cada volta.
    largPainel: 0,

    /* ---- os NÍVEIS do painel ----
     *
     * A entrada é a ATIVIDADE, e não a estação, porque é a atividade que
     * responde à pergunta que abre este painel: «quem está doendo AGORA».
     * Uma estação com dez conexões calmas some dentro do próprio nome; a
     * conexão que segurou a trava por meio minuto tem de aparecer inteira.
     * É também o que a referência faz — lá cada bolha é um spid, e não uma
     * máquina.
     *
     * Agrupar por estação é uma VISTA, para a pergunta seguinte: «de qual
     * máquina vem esse aperto». Dela se desce para as conexões daquela
     * estação, e de uma conexão para o descritivo — os três níveis que o
     * dono pediu, com trilha de volta em cada um. */
    vista: "atividades",
    estacao: null,
    busca: "",
    legendaAberta: true,

    /* Onde cada bolha ESTÁ (x,y,r) e para onde ela vai (ax,ay,ar). A
     * separação é o que permite a bolha deslizar até o lugar novo em vez de
     * teleportar, e é o que guarda a posição enquanto o ponteiro congela o
     * painel. */
    pos: new Map(),
    quadro: null,
    derivaViva: 0,
    ponteiroDentro: false,
    aoAvisar: () => {},
  };

  function $(s) { return document.querySelector(s); }

  function iniciar(cfg) {
    parar();
    estado.api = cfg.api;
    estado.aoAvisar = cfg.aoAvisar || (() => {});
    estado.periodo = cfg.periodo || 2000;
    estado.pausado = false;
    estado.selecionada = null;
    estado.vista = "atividades";
    estado.estacao = null;
    estado.busca = "";
    estado.pos = new Map();

    $("#tlmPausar").onclick = () => {
      estado.pausado = !estado.pausado;
      $("#tlmPausar").textContent = estado.pausado ? "Retomar" : "Pausar";
      // A classe diz o mesmo que a palavra, para quem lê o botão pela cor.
      $("#tlmPausar").className = "botao " + (estado.pausado ? "incluir" : "consultar");
      // Pausa também congela a tela — e congelado por vontade de alguém tem
      // de ser distinguível de congelado porque o servidor caiu. Os dois
      // param de atualizar; só um deles é notícia.
      document.querySelector(".tlm")?.classList.toggle("pausado", estado.pausado);
      if (estado.pausado) {
        const p = document.createElement("span");
        p.className = "tlm-pastilha pausa";
        p.textContent = "pausado por você — a tela não se atualiza";
        $("#tlmEstado").prepend(p);
      } else {
        volta();
      }
    };
    $("#tlmAgora").onclick = () => volta();
    $("#tlmLigar").onclick = async () => {
      try {
        const r = await estado.api(estado.ligada ? "telemetria_desligar" : "telemetria_ligar");
        estado.aoAvisar(r.aviso || (estado.ligada ? "coleta desligada" : "coleta ligada"));
        volta();
      } catch (e) { estado.aoAvisar(String(e), true); }
    };
    const svg = $("#tlmBolhas");
    svg.addEventListener("click", ev => {
      const g = ev.target.closest("[data-id]");
      if (!g) return;
      escolher(g.dataset.id);
    });
    // Teclado: o painel inteiro é navegável, e cada bolha é um botão. Sem
    // isto, descer de nível seria coisa só de quem usa mouse.
    svg.addEventListener("keydown", ev => {
      if (ev.key !== "Enter" && ev.key !== " ") return;
      const g = ev.target.closest("[data-id]");
      if (!g) return;
      ev.preventDefault();
      escolher(g.dataset.id);
    });
    /* **Alvo que se move é alvo que não se clica.** Com o ponteiro dentro do
     * painel a deriva morre e as posições congelam — a volta seguinte não
     * puxa a bolha debaixo do cursor. Sem isto, mirar uma bolha pequena num
     * painel que respira é loteria. */
    svg.addEventListener("pointerenter", () => {
      estado.ponteiroDentro = true; acordarQuadro();
    });
    svg.addEventListener("pointerleave", () => {
      estado.ponteiroDentro = false;
      // Ao sair, o que ficou parado durante a mira vai para o lugar novo.
      desenharBolhas((estado.ultimo || {}).atividades || []);
      acordarQuadro();
    });
    $("#tlmBusca").addEventListener("input", ev => {
      estado.busca = ev.target.value.trim().toLowerCase();
      desenharBolhas((estado.ultimo || {}).atividades || []);
      desenharCartao();
    });
    $("#tlmLegenda").onclick = () => {
      estado.legendaAberta = !estado.legendaAberta;
      aplicarLegenda();
    };
    $("#tlmTrilha").addEventListener("click", ev => {
      const b = ev.target.closest("[data-nivel]");
      if (!b) return;
      if (b.dataset.nivel === "todas") { estado.vista = "atividades"; estado.estacao = null; }
      if (b.dataset.nivel === "estacoes") { estado.vista = "estacoes"; estado.estacao = null; }
      estado.selecionada = null;
      desenhar(estado.ultimo || {});
    });
    aplicarLegenda();
    volta();
    estado.timer = setInterval(() => { if (!estado.pausado) volta(); }, estado.periodo);
  }

  function parar() {
    if (estado.timer) clearInterval(estado.timer);
    estado.timer = null;
    if (estado.quadro) cancelAnimationFrame(estado.quadro);
    estado.quadro = null;
  }

  /** O clique numa bolha: desce um nível, ou abre o descritivo. */
  function escolher(id) {
    if (estado.vista === "estacoes" && !estado.estacao) {
      // Bolha de estação: descer é entrar nas conexões dela.
      estado.estacao = id.replace(/^estacao:/, "");
      estado.selecionada = null;
      desenhar(estado.ultimo || {});
      return;
    }
    estado.selecionada = id;
    desenharCartao();
    marcarSelecao();
  }

  function aplicarLegenda() {
    const e = $("#tlmExplica"), b = $("#tlmLegenda");
    if (!e || !b) return;
    e.hidden = !estado.legendaAberta;
    b.textContent = estado.legendaAberta ? "ocultar legenda" : "mostrar legenda";
    b.setAttribute("aria-expanded", String(estado.legendaAberta));
  }

  async function volta() {
    if (!estado.api) return;
    // **O relógio para sozinho quando a tela sai da página.**
    //
    // `folha()` avisa o módulo quando alguém troca de ferramenta, mas nem
    // toda troca passa por ela: `abrirAdmin` e `abrirTabela` substituem o
    // `#painel` por conta. Pior — `abrirAdmin` é assíncrona, e sob carga
    // pesada ela demora: exercitando com uma consulta longa segurando a
    // trava, um `abrirAdmin("painel")` disparado no login só terminou depois
    // de a Telemetria já estar montada, e sobrescreveu o painel dela.
    //
    // Sem esta parada o relógio ficava batendo para sempre contra uma tela
    // que não existe mais, pedindo telemetria de dois em dois segundos até
    // alguém fechar o navegador. É o mesmo remédio que o monitor da máquina
    // já usa: sem alvo, o relógio para sozinho.
    if (!$("#tlmBolhas")) return parar();
    const t0 = Date.now();
    try {
      const r = await estado.api("telemetria", { amostras: 120 });
      estado.ultimaIdaVolta = Date.now() - t0;
      desenhar(r);
    } catch (e) {
      // **O servidor não respondeu.** O que está desenhado continua na tela —
      // apagar tudo perderia justamente o retrato do instante em que ele caiu,
      // que é o que alguém vai querer olhar. Mas ele passa a ser declarado
      // VELHO, com a idade e a hora do que se está vendo.
      //
      // Mostrar dado velho sem dizer que é velho é a mentira de tela que esta
      // casa não aceita: um painel congelado é idêntico a um painel calmo.
      estado.falhas++;
      marcarVelho(String(e));
    }
  }

  /** Declara na tela que o que está desenhado não é de agora. */
  function marcarVelho(erro) {
    const raiz = document.querySelector(".tlm");
    if (raiz) raiz.classList.add("velho");
    const alvo = $("#tlmEstado");
    if (!alvo) return;
    const idade = estado.ultimoOkEm ? Date.now() - estado.ultimoOkEm : 0;
    alvo.innerHTML =
      `<span class="tlm-pastilha mal">sem resposta do servidor</span>
       <span class="mal"><b>${esc(erro)}</b></span>
       <span>${estado.ultimoOkEm
         ? `o que está na tela é de <b>${esc(estado.ultimoOkRotulo)}</b>, há <b>${dur(idade)}</b>`
         : "nunca houve resposta nesta sessão"}</span>
       <span>${estado.falhas} tentativa(s) sem resposta</span>`;
  }

  /* Desenha um retrato inteiro. É público para poder ser exercitado sem
     servidor — foi assim que o desenho foi conferido no navegador. */
  function desenhar(d) {
    // A volta e assincrona: quando alguem troca de tela no meio de uma, a
    // resposta chega para um painel que ja nao existe. Sem esta linha, o
    // primeiro `innerHTML` estoura -- e o erro aparece no console de quem
    // simplesmente clicou noutra ferramenta.
    if (!$("#tlmBolhas")) return;
    estado.falhas = 0;
    estado.ultimoOkEm = Date.now();
    estado.ultimoOkRotulo = d.agora || "";
    document.querySelector(".tlm")?.classList.remove("velho");
    estado.ultimo = d;
    estado.ligada = !!d.ligada;
    const s = d.series || [];
    const ult = s.length ? s[s.length - 1] : {};
    const bot = $("#tlmLigar");
    if (bot) {
      bot.textContent = d.ligada ? "Desligar coleta" : "Ligar coleta";
      bot.className = "botao " + (d.ligada ? "alterar" : "incluir");
    }

    // O instante da última amostra e a distância dele para agora. É o que
    // responde «há atraso?» — sem isso, uma série congelada parece calma.
    const atraso = num(d.atraso_ms);
    const tarde = d.ligada && atraso > 3000;
    $("#tlmEstado").innerHTML =
      `<span class="tlm-pastilha ${d.ligada ? "on" : "off"}">${d.ligada ? "coletando" : "coleta desligada"}</span>
       <span>última amostra <b>${esc(d.ultima_amostra || "—")}</b></span>
       <span class="${tarde ? "mal" : ""}">atraso da amostra <b>${dur(atraso)}</b></span>
       <span>ida e volta <b>${dur(estado.ultimaIdaVolta)}</b></span>
       <span>período <b>${dur(num(d.periodo_ms))}</b></span>
       ${estado.pausado ? `<span class="tlm-pastilha pausa">pausado por você</span>` : ""}
       ${d.stress ? `<span class="tlm-stress">servidor em stress · ${esc(d.stress_por_que || "")}</span>` : ""}`;

    const col = (campo) => s.map(a => num(a[campo]));

    // A espera acumulada só entra na conta quando a espera TERMINA — ela é
    // somada no instante em que a trava chega na mão. No meio de uma fila
    // longa o «ms/s» ainda diz zero, e o painel pareceria calmo justamente no
    // pior momento. A maior espera em curso existe enquanto a fila existe, e
    // é ela que vai no destaque.
    const esperando = num(ult.esperando);
    faixa($("#tlmEsperas"), {
      titulo: "Esperas — atividades por estado",
      valor: esperando > 0
        ? `${esperando} na fila · a mais antiga há ${dur(ult.espera_maior_ms)}`
        : `ninguém na fila · ${num(ult.espera_ms_s).toFixed(0)} ms/s de espera`,
      empilhado: true,
      topoMinimo: 3,
      topoRot: v => v.toFixed(0),
      series: [
        { nome:"executando", cor:"var(--reg)",      vals: col("executando") },
        { nome:"esperando",  cor:"var(--ambar)",    vals: col("esperando") },
        { nome:"encerrando", cor:"var(--vermelho)", vals: col("encerrando") },
        { nome:"ociosas",    cor:"var(--texto-3)",  vals: col("ociosas") },
      ],
    });

    faixa($("#tlmDisco"), {
      titulo: "Leitura e escrita físicas (deste processo)",
      valor: `${bytes(ult.ler_bytes_s)}/s ler · ${bytes(ult.escrever_bytes_s)}/s gravar`,
      topoRot: v => bytes(v) + "/s",
      series: [
        { nome:"lidos",     cor:"var(--bin)",     vals: col("ler_bytes_s") },
        { nome:"gravados",  cor:"var(--laranja)", vals: col("escrever_bytes_s"), tracejado:true },
      ],
    });

    faixa($("#tlmCpu"), {
      titulo: "CPU",
      valor: `processo ${num(ult.cpu_processo).toFixed(0)}% · máquina ${num(ult.cpu_maquina).toFixed(0)}%`,
      topoMinimo: 100,
      topoRot: v => v.toFixed(0) + "%",
      series: [
        { nome:"processo", cor:"var(--memo)",   vals: col("cpu_processo") },
        { nome:"máquina",  cor:"var(--texto-3)", vals: col("cpu_maquina"), tracejado:true },
      ],
    });

    faixa($("#tlmVazao"), {
      titulo: "Vazão — operações por segundo",
      valor: `${num(ult.leituras_s).toFixed(1)} leitura/s · ${num(ult.escritas_s).toFixed(1)} escrita/s`,
      topoMinimo: 1,
      topoRot: v => v.toFixed(1) + "/s",
      series: [
        { nome:"leitura", cor:"var(--reg)",      vals: col("leituras_s") },
        { nome:"escrita", cor:"var(--acao-incluir)", vals: col("escritas_s") },
        { nome:"erro",    cor:"var(--vermelho)", vals: col("erros_s"), tracejado:true },
      ],
    });

    const c = d.cache_ndx || {};
    // «0,00% de acerto» com zero toque não é um cache ruim: é um cache que
    // ninguém usou ainda. Uma soma de verificação varre o `.reg` de ponta a
    // ponta e não encosta no índice — e a tela dizia que o cache estava
    // falhando enquanto ele nem tinha sido chamado. Número honesto é número
    // que sabe dizer «ainda não sei».
    const tocouCache = num(c.acertos) + num(c.faltas) > 0;
    faixa($("#tlmCache"), {
      titulo: "Cache de páginas do .ndx",
      valor: tocouCache
        ? `${esc(c.acerto_percentual || "0")}% de acerto · teto ${esc(String(c.paginas_teto || 0))} páginas`
        : `sem toque de página ainda · teto ${esc(String(c.paginas_teto || 0))} páginas`,
      topoMinimo: 1,
      topoRot: v => v.toFixed(0) + "/s",
      series: [
        { nome:"acertos", cor:"var(--acao-incluir)", vals: col("cache_acertos_s") },
        { nome:"faltas",  cor:"var(--ambar)",        vals: col("cache_faltas_s"), tracejado:true },
      ],
    });

    desenharFaixas(d.limiares);
    desenharBolhas(d.atividades || []);
    desenharThreads(d.threads || []);
    desenharCartao();
  }

  /* ------------------------------------------------------------- as bolhas */

  /** O nome curto que vai DENTRO da bolha. */
  function curtinho(id) {
    return String(id).replace(/^dados:/, "#").replace(/^web:/, "w·")
                     .replace(/^estacao:/, "");
  }

  /* Quantos caracteres de monoespaçada cabem numa corda do círculo.
   *
   * `dy` é a distância do texto ao centro: a corda encurta conforme o texto
   * se afasta, e é por isso que a segunda linha cabe menos que a primeira. O
   * 0,6 é a razão largura/altura da IBM Plex Mono; o 0,84 é o respiro até a
   * borda, senão a letra encosta no aro escuro da esfera. */
  function cabeEmCaracteres(r, dy, fonte) {
    const meia = Math.sqrt(Math.max(0, r * r - dy * dy));
    return Math.floor((2 * meia * 0.84) / (fonte * 0.6));
  }

  /** A atividade casa com o que está escrito na busca? */
  function casaComABusca(a, b) {
    if (!b) return true;
    return [a.id, a.ip, a.usuario, a.op, a.alvo, a.origem, a.estado]
      .some(v => String(v ?? "").toLowerCase().includes(b));
  }

  /* As bolhas do nível corrente.
   *
   * Devolve objetos com a mesma cara em todos os níveis — `id`, `peso_ms`,
   * `nivel`, `rot` —, porque quem desenha não deveria precisar saber se está
   * olhando uma conexão ou uma estação. O que muda de nível para nível é
   * só isto aqui. */
  function bolhasDoNivel(ativs) {
    const b = estado.busca;
    if (estado.vista === "estacoes" && !estado.estacao) {
      // Uma bolha por ESTAÇÃO: o peso é a soma do que as conexões dela
      // gastaram, e o nível é o PIOR delas — uma estação com uma conexão em
      // stress é uma estação em stress, e diluir isso numa média esconderia
      // exatamente o que se procura.
      const por = new Map();
      ativs.filter(a => casaComABusca(a, b)).forEach(a => {
        const ip = a.ip || "sem IP";
        const e = por.get(ip) || { id: "estacao:" + ip, ip, peso_ms: 0, nivel: "normal",
                                   quantas: 0, executando: 0, usuarios: new Set() };
        e.peso_ms += num(a.peso_ms);
        e.quantas++;
        if (a.op) e.executando++;
        if (a.usuario) e.usuarios.add(a.usuario);
        if ((ORDEM_NIVEL[a.nivel] ?? 0) > (ORDEM_NIVEL[e.nivel] ?? 0)) e.nivel = a.nivel;
        por.set(ip, e);
      });
      return [...por.values()].map(e => ({
        id: e.id, rotulo: e.ip, peso_ms: e.peso_ms, nivel: e.nivel,
        sub: e.quantas + " conex.", estacao: e,
      }));
    }
    return ativs
      .filter(a => (!estado.estacao || a.ip === estado.estacao) && casaComABusca(a, b))
      .map(a => ({ id: a.id, rotulo: curtinho(a.id), peso_ms: num(a.peso_ms),
                   nivel: a.nivel, sub: a.op || "ociosa", d: a }));
  }

  function desenharTrilha() {
    const alvo = $("#tlmTrilha");
    if (!alvo) return;
    const passo = (n, r, atual) =>
      `<button class="tlm-passo${atual ? " atual" : ""}" data-nivel="${n}"
               type="button"${atual ? ' aria-current="true"' : ""}>${esc(r)}</button>`;
    let h = passo("todas", "Atividades", estado.vista === "atividades")
          + `<span class="tlm-sep">|</span>`
          + passo("estacoes", "Por estação", estado.vista === "estacoes" && !estado.estacao);
    if (estado.estacao) {
      h += `<span class="tlm-sep">›</span>`
         + `<span class="tlm-passo atual" aria-current="true">${esc(estado.estacao)}</span>`;
    }
    /* **Só reescreve quando MUDA**, e isso não é economia: é o «nada pisca»
     * outra vez, num lugar novo. Reescrevendo a cada volta, os botões da
     * trilha são destruídos e recriados de dois em dois segundos — o clique
     * que cai no instante da troca some, e o foco do teclado vai junto.
     *
     * Foi o navegador que mostrou: um clique automatizado na trilha ficou
     * TRINTA SEGUNDOS tentando, perdendo a corrida para o redesenho toda vez.
     * Quem usa mouse perderia o clique de vez em quando e não saberia por quê. */
    // A lembranca fica numa PROPRIEDADE do elemento, e nao num `data-`: um
    // atributo com o HTML inteiro dentro aparece no inspetor e confunde quem
    // for ler a arvore depois.
    if (alvo._trilhaFeita !== h) { alvo.innerHTML = h; alvo._trilhaFeita = h; }
  }

  function desenharBolhas(ativs) {
    const d0 = estado.ultimo || {};
    const svg = $("#tlmBolhas");
    const painel = svg && svg.closest(".tlm-painel");
    if (!svg || !painel) return;
    desenharTrilha();

    // A ordem do desenho é a do TAMANHO, e não a que o servidor mandou: as
    // mais pesadas ocupam a fileira da FRENTE, e a ordem se lê da frente para
    // o fundo. O desempate pelo id mantém o desenho determinístico quando
    // dois pesos empatam — sem ele o painel embaralharia sozinho a cada volta.
    const todas = bolhasDoNivel(ativs);
    const ordenadas = todas.slice().sort((a, b) =>
      b.peso_ms - a.peso_ms || String(a.id).localeCompare(String(b.id)));
    const desenhadas = ordenadas.slice(0, M.bolhasMax);
    const deFora = ordenadas.length - desenhadas.length;

    // O `<defs>` das esferas nasce uma vez e fica. Recriá-lo a cada volta
    // faria o navegador recompor todo gradiente duas vezes por segundo.
    if (!svg.querySelector("defs")) svg.insertAdjacentHTML("afterbegin", defsDasEsferas());
    let chao = svg.querySelector(".tlm-chao");
    if (!chao) {
      chao = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      chao.setAttribute("class", "tlm-chao");
      svg.appendChild(chao);
    }

    // A largura disponível é medida com o painel SOLTO. Medir com o
    // `max-width` da volta passada ainda posto encolheria o painel um pouco a
    // cada volta, até ele sumir.
    painel.style.maxWidth = "";
    const largDisp = Math.max(260, painel.clientWidth || 700);

    const pesoMax = desenhadas.reduce((m, a) => Math.max(m, a.peso_ms), 1);
    const itens = desenhadas.map(a => ({ d: a, raio: raioRelativo(a.peso_ms, pesoMax) }));

    let larg = largDisp, alt = M.altMin, k = 0;
    if (itens.length) {
      const teto = Math.min(M.tetoAbsoluto, M.tetoDaCaixa * Math.min(largDisp, M.altMax));
      k = maiorRaioQueCabe(itens, largDisp, M.altMax - 2 * M.margem, teto);
      // A CAIXA SEGUE O DESENHO, e não o contrário. Quando sobra largura --
      // poucas bolhas --, o painel encolhe e devolve o espaço ao descritivo,
      // em vez de deixar uma bolha sozinha num vazio de mil pixels. Foi essa
      // a queixa do dono, e nenhuma conta de raio a resolve: só a caixa.
      const primeiro = fileiras(itens, largDisp, k);
      larg = Math.min(largDisp, Math.max(M.largMin, primeiro.largMax));
      k = maiorRaioQueCabe(itens, larg, M.altMax - 2 * M.margem, teto);
      alt = Math.min(M.altMax, Math.max(M.altMin, fileiras(itens, larg, k).alt));
      arrumar(itens, larg, alt, k);
    }

    svg.setAttribute("viewBox", `0 0 ${larg.toFixed(0)} ${alt.toFixed(0)}`);
    svg.style.height = alt.toFixed(0) + "px";
    chao.setAttribute("x", "0"); chao.setAttribute("y", "0");
    chao.setAttribute("width", larg.toFixed(0));
    chao.setAttribute("height", alt.toFixed(0));
    // Só encolhe o painel quando a diferença é visível. Sem essa histerese
    // ele tremeria um pixel para cada lado a cada duas voltas.
    const largAlvo = Math.round(larg);
    if (Math.abs(largAlvo - (estado.largPainel || 0)) > 14) estado.largPainel = largAlvo;
    painel.style.maxWidth = (estado.largPainel || largAlvo) + "px";

    const tinta = {};
    Object.entries(NIVEIS).forEach(([n, v]) => { tinta[n] = tintaDoRotulo(v.cor); });

    const vistos = new Set();
    itens.forEach(it => {
      const a = it.d;
      const id = a.id;
      vistos.add(id);
      const niv = NIVEIS[a.nivel] || NIVEIS.normal;
      let p = estado.pos.get(id);
      if (!p || !p.g || !p.g.isConnected) {
        // Nasce onde vai ficar, e não no canto: uma bolha que nasce em 0,0 e
        // desliza até o lugar chama a atenção para o nada.
        const g = document.createElementNS("http://www.w3.org/2000/svg", "g");
        g.setAttribute("data-id", id);
        g.setAttribute("class", "tlm-bolha");
        g.setAttribute("tabindex", "0");
        g.setAttribute("role", "button");
        g.innerHTML =
          `<circle class="tlm-c"></circle>
           <circle class="tlm-alvo"></circle>
           <text class="tlm-id" text-anchor="middle"></text>
           <text class="tlm-sub" text-anchor="middle"></text>
           <title></title>`;
        svg.appendChild(g);
        p = { g, circulo: g.querySelector(".tlm-c"), alvo: g.querySelector(".tlm-alvo"),
              rotulo: g.querySelector(".tlm-id"), sub: g.querySelector(".tlm-sub"),
              x: it.px, y: it.py, r: it.pr, ax: it.px, ay: it.py, ar: it.pr,
              dx: 0, dy: 0, dy1: 0, dy2: 0, fase: semente(id) };
        estado.pos.set(id, p);
      }
      // Com o ponteiro dentro do painel as posições NÃO se movem: quem está
      // mirando não pode ver o alvo escapar. O raio continua acompanhando o
      // peso, porque crescer no lugar não desloca o alvo.
      if (!estado.ponteiroDentro) { p.ax = it.px; p.ay = it.py; }
      p.ar = it.pr;
      p.fileira = it.fileira;

      p.circulo.setAttribute("fill", `url(#tlmEsfera-${NIVEIS[a.nivel] ? a.nivel : "normal"})`);
      p.circulo.setAttribute("stroke", niv.cor);
      p.circulo.setAttribute("stroke-dasharray", niv.traco);

      // O rótulo é medido contra a corda do círculo, e não chutado pelo raio.
      // Chutar pelo raio foi o que punha «w·a1b31e4» transbordando da bolha
      // num painel estreito: o raio dizia que cabia, a largura do texto dizia
      // que não. Duas linhas só quando as duas cabem.
      const r = it.pr;
      const fonte = Math.max(8.5, Math.min(21, r * 0.34));
      const nome = a.rotulo;
      const glifo = niv.glifo ? niv.glifo + " " : "";
      const segunda = glifo + a.sub;
      const fonte2 = Math.max(8, Math.min(12.5, fonte * 0.62));
      const duas = r >= 30
        && cabeEmCaracteres(r, fonte * 0.95, fonte) >= Math.min(nome.length, 4)
        && cabeEmCaracteres(r, fonte2 * 1.5, fonte2) >= 5;

      p.dy1 = duas ? -fonte * 0.15 : fonte * 0.36;
      p.dy2 = fonte * 0.95 + fonte2 * 0.5;
      const cabe1 = cabeEmCaracteres(r, duas ? fonte * 0.95 : fonte * 0.6, fonte);
      p.rotulo.setAttribute("font-size", fonte.toFixed(1));
      // Menos de dois caracteres não identifica ninguém: melhor a esfera limpa
      // e o nome no tooltip e no cartão do que uma letra cortada pela borda.
      p.rotulo.textContent = cabe1 >= 2 ? nome.slice(0, cabe1) : "";
      p.sub.setAttribute("font-size", fonte2.toFixed(1));
      p.sub.textContent = duas ? segunda.slice(0, cabeEmCaracteres(r, fonte2 * 1.5, fonte2)) : "";
      // A tinta do rótulo é decidida MEDINDO a luminância do corpo da esfera,
      // porque as quatro cores clareiam no tema escuro e escurecem no claro:
      // «branco», que é o que a referência usa, reprovaria em metade dos casos.
      const tn = tinta[a.nivel] || tinta.normal;
      [p.rotulo, p.sub].forEach(e => {
        e.setAttribute("fill", tn.tinta);
        e.setAttribute("stroke", tn.contorno);
      });

      const euMesmo = d0.voce === id;
      const quem = a.estacao
        ? `estação ${a.rotulo} · ${a.estacao.quantas} conexão(ões), `
          + `${a.estacao.executando} executando · peso ${dur(a.peso_ms)}`
        : `${id}${euMesmo ? " (a sua própria tela)" : ""} · ${niv.rot} · `
          + `${a.sub} · peso ${dur(a.peso_ms)}`;
      // O tooltip e o `aria-label` dizem em PALAVRAS o que a cor diz: é o que
      // mantém o painel legível para quem não distingue as quatro — e é onde
      // o nome inteiro aparece quando a bolha é pequena demais para ele.
      p.g.querySelector("title").textContent = quem;
      // A bolha de quem está olhando o painel leva o nome disso. Escondê-la
      // seria mentir sobre quem está conectado; deixá-la anônima faria o
      // operador procurar quem é «w·85a62fd» — e é ele mesmo.
      p.g.classList.toggle("eu", !!euMesmo);
      p.g.setAttribute("aria-label", quem
        + (a.estacao ? ", clique para ver as conexões desta estação"
                     : ", clique para o descritivo completo"));
      porNoLugar(p);
    });

    // O que sumiu sai. Sem isto, a conexão que caiu ficaria desenhada para
    // sempre — e um painel que mostra quem já foi embora não serve para nada.
    // A ORDEM do desenho é a fileira do fundo primeiro: assim a da frente
    // passa por cima, e é daí que vem a profundidade da referência.
    estado.pos.forEach((p, id) => {
      if (vistos.has(id)) return;
      p.g.remove();
      estado.pos.delete(id);
    });
    // E o SVG é varrido também, e não só o mapa. Trocar de nível esvaziava o
    // mapa e deixava as bolhas do nível anterior ÓRFÃS no desenho: doze
    // conexões continuavam lá com quatro estações desenhadas por cima. O mapa
    // é a lembrança da tela, e lembrança apagada não apaga o que está no
    // documento — quem manda no desenho é o documento.
    svg.querySelectorAll("[data-id]").forEach(g => {
      if (!estado.pos.has(g.dataset.id)) g.remove();
    });
    [...estado.pos.values()]
      .sort((a, b) => (b.fileira || 0) - (a.fileira || 0))
      .forEach(p => svg.appendChild(p.g));

    // Painel sem bolha nenhuma diz isso escrito. Caixa vazia é ambígua: pode
    // ser servidor parado, pode ser filtro que não achou ninguém.
    let vazio = svg.querySelector(".tlm-sem");
    if (!itens.length) {
      if (!vazio) {
        vazio = document.createElementNS("http://www.w3.org/2000/svg", "text");
        vazio.setAttribute("class", "tlm-sem");
        vazio.setAttribute("text-anchor", "middle");
        svg.appendChild(vazio);
      }
      vazio.textContent = estado.busca || estado.estacao
        ? "nenhuma atividade bate com o filtro"
        : "nenhuma atividade viva neste instante";
      vazio.setAttribute("x", (larg / 2).toFixed(0));
      vazio.setAttribute("y", (alt / 2).toFixed(0));
    } else if (vazio) {
      vazio.remove();
    }
    marcarSelecao();
    desenharEscala(pesoMax, ordenadas);
    acordarQuadro();

    const t = d0.totais || {};
    const maior = ordenadas[0];
    // Quantas atividades o FILTRO tirou -- e não quantas bolhas a menos há na
    // tela. Na vista por estação doze conexões viram quatro bolhas, e a conta
    // ingênua anunciava «8 fora do filtro» com filtro nenhum posto. Contar o
    // que a pergunta pergunta, e não o que é fácil de subtrair.
    const escondidas = ativs.filter(a =>
      !((!estado.estacao || a.ip === estado.estacao) && casaComABusca(a, estado.busca))).length;
    // A ordem é a promessa do painel, então o resumo diz quem é a cabeça
    // dela: quem só olha o texto continua sabendo qual é a mais pesada.
    $("#tlmResumo").textContent =
      `${ativs.length} viva(s) · ${ativs.filter(a => a.op).length} executando`
      + (estado.vista === "estacoes" && !estado.estacao
          ? ` · ${todas.length} estação(ões)` : "")
      + (maior ? ` · a mais pesada aqui: ${maior.rotulo} (${dur(maior.peso_ms)})` : "")
      + (escondidas > 0 ? ` · ${escondidas} fora do filtro` : "")
      + (deFora > 0 ? ` · ${deFora} mais leve(s) fora do desenho` : "")
      + ` · ${num(t.encerramentos)} encerramento(s) desde que subiu`;
  }

  /* A escala: três círculos com o peso escrito ao lado.
   *
   * É o «eixo» que um gráfico de bolha tem. Sem ele o tamanho vira impressão
   * e não medida — dá para ver que uma é maior, não QUANTO maior. Os raios
   * saem da mesma `raioRelativo` do painel, então o piso das mais leves
   * aparece desenhado: proporção quebrada em silêncio seria mentira. */
  function desenharEscala(pesoMax, ordenadas) {
    const alvo = $("#tlmEscala");
    if (!alvo) return;
    // Sem atividade nenhuma a escala diria «0 ms · 0 ms · 1 ms», que é uma
    // régua de coisa que não existe. Some inteira em vez de mentir de leve.
    const caixa = alvo.closest(".tlm-escala");
    if (caixa) caixa.hidden = !ordenadas.length;
    if (!ordenadas.length) return;
    const menor = num(ordenadas[ordenadas.length - 1].peso_ms);
    const marcas = [pesoMax / 16, pesoMax / 4, pesoMax];
    const R = 16;
    let x = 4, svg = "";
    marcas.forEach(p => {
      const r = raioRelativo(p, pesoMax) * R;
      const rot = dur(p);
      const larg = Math.max(2 * r, rot.length * 5.4) + 12;
      svg += `<circle cx="${(x + larg / 2).toFixed(1)}" cy="17" r="${r.toFixed(1)}"/>`
           + `<text x="${(x + larg / 2).toFixed(1)}" y="38" text-anchor="middle">${esc(rot)}</text>`;
      x += larg;
    });
    alvo.setAttribute("viewBox", `0 0 ${Math.ceil(x + 4)} 44`);
    alvo.setAttribute("aria-label",
      `escala do peso: a área segue o peso; a mais pesada tem ${dur(pesoMax)}`
      + (ordenadas.length > 1 ? ` e a mais leve, ${dur(menor)}` : ""));
    alvo.innerHTML = svg;
  }

  /** As faixas de cor, escritas com os limiares que o SERVIDOR mandou. */
  function desenharFaixas(lim) {
    const alto = lim && lim.alto_uso_ms ? dur(lim.alto_uso_ms) : null;
    const stress = lim && lim.stress_ms ? dur(lim.stress_ms) : null;
    const posto = (id, texto) => { const e = $(id); if (e) e.textContent = texto; };
    posto("#tlmFxNormal", "abaixo dos limiares, ou sem operação");
    // Sem o campo, a frase perde o número em vez de inventar um. Número na
    // tela que não veio do servidor é o começo de a tela pintar uma cor que
    // o servidor não concorda.
    posto("#tlmFxAlto", alto
      ? `operação acima de ${alto}, ou parada na fila da trava`
      : "operação longa, ou parada na fila da trava");
    posto("#tlmFxStress", stress
      ? `trabalhando há mais de ${stress}, ou segurando a trava com fila`
      : "trabalhando demais, ou segurando a trava com fila");
  }

  /* Qual atividade o cartão está descrevendo.
   *
   * Sem clique nenhum ele descreve a MAIS PESADA. A coluna do descritivo era
   * uma faixa de trezentos pixels com uma frase de convite dentro, do lado de
   * um painel que o dono já achava vazio — e a informação que ela mostraria
   * de graça é justamente a que o painel inteiro existe para achar.
   *
   * A escolha automática NÃO liga os botões, e isso é regra: encerrar
   * operação dos outros é ato de administração, e ato de administração exige
   * escolha explícita. Botão armado sobre algo que ninguém apontou é como se
   * mata a atividade errada. */
  function alvoDoCartao() {
    const d = estado.ultimo || {};
    // O cartão descreve o que está NO PAINEL: com um filtro posto ou dentro
    // de uma estação, mostrar a mais pesada de fora seria falar de uma bolha
    // que não está desenhada.
    const ativs = (d.atividades || []).filter(a =>
      (!estado.estacao || a.ip === estado.estacao) && casaComABusca(a, estado.busca));
    const clicada = ativs.find(x => x.id === estado.selecionada);
    if (clicada) return { a: clicada, escolhida: true };
    const maior = ativs.slice().sort((a, b) =>
      num(b.peso_ms) - num(a.peso_ms) || String(a.id).localeCompare(String(b.id)))[0];
    return { a: maior || null, escolhida: false };
  }

  function marcarSelecao() {
    const alvo = alvoDoCartao();
    const auto = !alvo.escolhida && alvo.a ? alvo.a.id : null;
    document.querySelectorAll("#tlmBolhas [data-id]").forEach(g => {
      g.classList.toggle("sel", g.dataset.id === estado.selecionada);
      // A bolha que o cartão está descrevendo sem ninguém ter clicado ganha
      // um anel mais leve que o da escolhida: o cartão tem de apontar para
      // alguma bolha, senão ele parece falar de ninguém.
      g.classList.toggle("auto", g.dataset.id === auto);
    });
  }

  /* ------------------------------------------------------------- o cartão */

  function linha(rot, val) {
    return `<div class="tlm-l"><span>${esc(rot)}</span><b>${esc(val ?? "—")}</b></div>`;
  }

  const SIM_NAO = v => (v ? "sim" : "não");

  /* Qual atividade o cartão está descrevendo.
   *
   * Sem clique nenhum ele descreve a MAIS PESADA. A coluna do descritivo era
   * uma faixa de trezentos pixels com uma frase de convite dentro, do lado de
   * um painel que o dono já achava vazio — e a informação que ela mostraria
   * de graça é justamente a que o painel inteiro existe para achar.
   *
   * A escolha automática NÃO liga os botões, e isso é regra: encerrar
   * operação dos outros é ato de administração, e ato de administração exige
   * escolha explícita. Botão armado sobre algo que ninguém apontou é como se
   * mata a atividade errada. */
  function desenharCartao() {
    const alvo = $("#tlmCartao");
    if (!alvo) return;
    const d = estado.ultimo || {};
    const escolha = alvoDoCartao();
    const a = escolha.a;
    if (!a) {
      alvo.innerHTML = `<div class="tlm-vazio">nenhuma atividade aqui — quando houver,
        clique numa bolha para ver o descritivo completo</div>`;
      return;
    }
    const niv = NIVEIS[a.nivel] || NIVEIS.normal;
    const dentro = alvo.contains(document.activeElement);
    const focado = dentro ? document.activeElement.id : "";
    // O botão só aparece quando o servidor diz que a operação TEM ponto de
    // cancelamento. Um botão que não cumpre o que promete é pior do que botão
    // nenhum — e quem sabe se ela é cancelável AGORA é o servidor, não a tela.
    // O botão segue `tem_ponto` — «esta operação tem onde parar» —, e não
    // `cancelavel`, que é «ela está nesse ponto neste instante». A diferença
    // apareceu exercitando: uma soma de verificação parada na fila da trava
    // tinha `cancelavel:false` por um instante, o botão sumia, e encerrá-la
    // funcionava perfeitamente. Botão que some do nada é tão ruim quanto
    // botão que não cumpre.
    const podeEncerrar = escolha.escolhida && !!a.op && !!a.tem_ponto && !a.encerrando;
    const irmas = (d.atividades || []).filter(x => x.ip === a.ip).length;
    alvo.innerHTML = `
      <div class="tlm-cartao-cab" style="--n:${niv.cor}">
        <span class="tlm-nivel">${esc(niv.glifo)} ${esc(niv.rot)}</span>
        <b>${esc(a.id)}</b>
        ${d.voce === a.id ? `<span class="tlm-eu">esta é a sua tela</span>` : ""}
        ${escolha.escolhida ? "" : `<span class="tlm-auto">a mais pesada agora</span>`}
      </div>
      <!-- TUDO o que sabemos da atividade, e não um resumo. É o que a
           referência faz: o painel dela lista as vinte e sete colunas do
           sysprocesses, o input_buffer inclusive. Resumo obriga quem
           investiga a ir procurar o resto noutro lugar, e no meio de um
           incidente não há outro lugar. -->
      <!-- A ficha em COLUNAS, e nao em linhas esticadas. Com uma linha por
           campo o valor ia parar no fim da carta: medido, 1.353px de vao entre
           «estado» e «executando» a 1920, e 4.553px a 5120. Vao desse tamanho
           nao se le -- o olho perde a linha no meio do caminho. Em colunas o
           vao fica no tamanho da coluna, e a largura extra vira mais coluna em
           vez de mais vazio. -->
      <div class="tlm-lista">
      ${linha("estado", a.estado)}
      ${linha("nível", niv.rot)}
      ${linha("operação em curso", a.op || "nenhuma em curso")}
      ${linha("alvo", a.alvo)}
      ${linha("fase", a.fase)}
      ${linha("usuário", a.usuario)}
      ${linha("origem", a.origem)}
      ${linha("estação (IP)", a.ip)}
      ${linha("conexão", a.ligacao == null ? "—" : "nº " + a.ligacao)}
      ${linha("conectada desde", a.desde)}
      ${linha("aberta há", dur(num(a.aberta_s) * 1000))}
      ${linha("operação iniciada", a.op_desde)}
      ${linha("operação dura há", dur(a.ha_ms))}
      ${linha("desse tempo, trabalhando", dur(a.trabalhando_ms))}
      ${linha("desse tempo, na fila da trava", dur(a.esperou_ms))}
      ${linha("peso (servidor gasto)", dur(a.peso_ms))}
      ${linha("pedidos já feitos", a.pedidos)}
      ${linha("unidades percorridas", a.passos)}
      ${linha("trava de dados", a.com_trava ? "na mão desta atividade" : "não")}
      ${linha("esperando", a.esperando_o_que)}
      ${linha("tem ponto de cancelamento", SIM_NAO(a.tem_ponto))}
      ${linha("cancelável neste instante", SIM_NAO(a.cancelavel))}
      ${linha("marcada para encerrar", SIM_NAO(a.encerrando))}
      ${linha("já encerrada", a.encerradas + " vez(es)")}
      </div>
      <div class="tlm-acoes">
        ${podeEncerrar
          ? `<button class="botao excluir" id="tlmEncerrar" type="button">Encerrar a operação</button>`
          : `<button class="botao excluir" type="button" disabled
               title="${esc(!escolha.escolhida ? "clique na bolha para poder encerrá-la"
                 : a.encerrando ? "já está encerrando"
                 : a.op ? "esta operação não tem ponto de cancelamento: vai terminar"
                 : "não há operação em curso")}">Encerrar a operação</button>`}
        ${escolha.escolhida && a.ligacao ? `<button class="botao marcar" id="tlmDerrubar" type="button">Derrubar a conexão</button>` : ""}
        ${a.ip && irmas > 1 && !estado.estacao
          ? `<button class="botao consultar" id="tlmEstacao" type="button">Ver as ${irmas} desta estação</button>`
          : ""}
      </div>
      <p class="tlm-nota">${
        !escolha.escolhida
          ? "esta é a atividade mais pesada agora, mostrada sem ninguém ter pedido. "
            + "<b>Clique na bolha dela</b> para poder encerrá-la — derrubar o trabalho "
            + "de outra pessoa exige escolha explícita."
        : a.encerrando
          ? "encerrando… a operação aborta no próximo ponto seguro."
          : !a.op
            ? "sem operação em curso. Derrubar a conexão fecha o soquete — é o «kill» de sempre."
            : a.cancelavel
              ? "cancelável agora: a marca é lida entre duas unidades de trabalho, e o que já "
                + "foi gravado fica gravado."
              : a.tem_ponto
                ? "esta operação tem ponto de cancelamento, mas não está nele neste instante — "
                  + "tipicamente porque espera a trava de dados. A marca vale para o primeiro "
                  + "ponto seguro que vier."
                : "não cancelável: a operação não tem ponto de cancelamento e vai terminar. "
                  + "Abandonar uma gravação no meio deixaria o arquivo mentindo."
      }</p>`;

    // O cartão inteiro é reescrito a cada volta porque os números mudam a cada
    // volta. O foco do teclado morreria junto com o botão antigo, e quem
    // navega sem mouse ficaria sem saber onde estava — então ele volta.
    if (focado && document.getElementById(focado)) document.getElementById(focado).focus();

    const bot = $("#tlmEncerrar");
    if (bot) bot.onclick = async () => {
      bot.disabled = true;
      try {
        const r = await estado.api("telemetria_encerrar", { id: a.id });
        // Só `nao_cancelavel` é notícia ruim: `marcada` quer dizer que a
        // marca está posta e vai valer, e pintá-la de erro assustaria à toa.
        estado.aoAvisar(`${r.estado}: ${r.aviso}`, r.estado === "nao_cancelavel");
      } catch (e) { estado.aoAvisar(String(e), true); }
      volta();
    };
    const der = $("#tlmDerrubar");
    if (der) der.onclick = async () => {
      if (!confirm(`Derrubar a conexão ${a.ligacao}? O soquete fecha e o cliente perde a resposta.`)) return;
      try {
        const r = await estado.api("encerrar_sessao", { id: a.ligacao });
        estado.aoAvisar(r.aviso || "conexão encerrada");
      } catch (e) { estado.aoAvisar(String(e), true); }
      volta();
    };
    // O caminho de descida a partir do descritivo: da conexão para as outras
    // da MESMA máquina. É a pergunta que vem logo depois de «quem está
    // doendo»: «é só essa conexão, ou é a estação inteira?».
    const est = $("#tlmEstacao");
    if (est) est.onclick = () => {
      estado.vista = "estacoes";
      estado.estacao = a.ip;
      desenhar(estado.ultimo || {});
    };
  }

  /* ------------------------------------------------------------ as threads */

  function desenharThreads(fios) {
    const tab = $("#tlmThreads");
    if (!tab) return;
    const vivas = fios.filter(f => f.viva).length;
    $("#tlmThreadsN").textContent = `· ${vivas} viva(s) de ${fios.length} registrada(s)`;
    tab.innerHTML =
      `<thead><tr><th>thread</th><th>família</th><th>finalidade</th>
         <th>fazendo agora</th><th class="num">voltas</th><th class="num">viva há</th></tr></thead>
       <tbody>${fios.map(f => `
        <tr class="${f.viva ? "" : "morta"}">
          <td class="tlm-nome">${esc(f.nome)}</td>
          <td>${esc(f.familia)}</td>
          <td class="tlm-fim">${esc(f.finalidade)}</td>
          <td>${esc(f.viva ? f.fazendo : "encerrada")}</td>
          <td class="num">${esc(String(f.voltas))}</td>
          <td class="num">${esc(dur(num(f.viva_s) * 1000))}</td>
        </tr>`).join("")}</tbody>`;
  }

  return { html, iniciar, parar, desenhar, fileiras, arrumar, raioRelativo, faixa };
})();

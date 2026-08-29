/* Diagrama ER do PhxSql — SVG desenhado à mão, sem biblioteca nenhuma.
 *
 * As chaves estrangeiras JÁ estão declaradas e JÁ vêm no `esquema`: cada
 * tabela responde `chaves_estrangeiras` com as colunas daqui, a tabela de lá e
 * o que acontece ao excluir e ao alterar.
 *
 * Este arquivo é um módulo separado do `index.html` de propósito: o layout do
 * grafo e o arrastar são a única parte da interface que é ALGORITMO, e eles
 * merecem caber numa tela sem rolar por oito mil linhas.
 *
 * ## O editor (pedido 127, segunda metade)
 *
 * Além de desenhar, o módulo agora EDITA:
 *
 * - arrastar a caixa pelo TÍTULO move a tabela (a posição é de quem chama,
 *   que a guarda onde quiser — a tela usa localStorage, por navegador);
 * - arrastar uma LINHA DE COLUNA puxa um relacionamento até a coluna de outra
 *   tabela — o título move, a coluna liga, e é assim que o conflito entre os
 *   dois gestos se resolve (o mesmo desenho do dbdiagram);
 * - clicar numa caixa abre o cartão dela, por conta de quem chama.
 *
 * O módulo NÃO fala com o servidor: ele entrega os gestos prontos nos
 * callbacks (`aoMover`, `aoLigar`, `aoAbrir`) e quem chama decide o que cada
 * um custa. É o que deixa o desenho ser exercitado sem servidor.
 *
 * ## Coordenadas e o defeito clássico do arrasto
 *
 * Todo ponto do mouse passa por `getScreenCTM().inverse()` antes de virar
 * coordenada do desenho. Sem isso o arrasto funciona até alguém dar zoom na
 * página ou o SVG ser desenhado menor que a `viewBox` — e aí a caixa foge do
 * ponteiro na proporção do zoom, que é exatamente o defeito que só aparece
 * exercitando.
 *
 * ## Cores
 *
 * Todas saem das variáveis do console (`--laranja`, `--ndx`, `--memo`…),
 * então o diagrama acompanha o tema claro e o escuro sem uma segunda paleta.
 * SVG aceita `var()` em `fill` e em `stroke`, e é por isso que dá para fazer
 * assim.
 */
"use strict";

window.PhxER = (function () {

  /* Medidas do desenho. Tudo em unidades de usuário do SVG, que aqui são
     pixels porque a `viewBox` casa com a largura declarada. */
  const M = {
    larguraCaixa: 220,
    alturaTitulo: 30,
    alturaLinha: 19,
    espacoX: 90,
    espacoY: 46,
    margem: 26,
    // Quantas colunas de uma tabela aparecem no desenho. Uma tabela de
    // quarenta colunas viraria uma tira de dois metros e não diria nada:
    // o que importa num ER é a CHAVE, e as demais viram uma contagem.
    maxColunas: 9,
  };

  /* ---------------------------------------------------------------- modelo */

  /** Monta o modelo a partir do que o `esquema` devolve, tabela por tabela.
   *
   * `esquemas` é um array de respostas de `esquema`, na ordem em que vieram.
   * Nada aqui pede nada ao servidor: quem busca é a tela, para o módulo poder
   * ser exercitado sem servidor.
   */
  function modelo(esquemas) {
    const tabelas = esquemas.map(e => {
      const colunas = (e.colunas || []).filter(c => !c.sistema);
      return {
        nome: e.tabela,
        registros: e.registros || 0,
        // A ordem: primária, estrangeira, o resto. Num ER a chave é o assunto.
        colunas: colunas.slice().sort((a, b) =>
          peso(a) - peso(b) || colunas.indexOf(a) - colunas.indexOf(b)),
        total: colunas.length,
        fks: e.chaves_estrangeiras || [],
      };
    });
    const porNome = {};
    for (const t of tabelas) porNome[chave(t.nome)] = t;

    // As ligações. Uma FK que aponta para tabela fora deste database vira
    // ligação PENDURADA — e o desenho diz isso, em vez de sumir com ela.
    const ligacoes = [];
    for (const t of tabelas) {
      for (const fk of t.fks) {
        ligacoes.push({
          de: t.nome,
          para: fk.tabela_ref,
          colunas: fk.colunas || [],
          colunas_ref: fk.colunas_ref || [],
          nome: fk.nome,
          ao_excluir: fk.ao_excluir,
          existe: !!porNome[chave(fk.tabela_ref)],
        });
      }
    }
    return { tabelas, porNome, ligacoes };
  }

  const chave = n => String(n || "").toLowerCase();
  const peso = c => (c.primaria ? 0 : c.estrangeira ? 1 : 2);

  /* ---------------------------------------------------------------- layout */

  /** Põe cada tabela numa faixa, pela PROFUNDIDADE dela nas chaves.
   *
   * Quem não aponta para ninguém fica na faixa 0; quem aponta para alguém da
   * faixa N fica na N+1. É o desenho que a própria declaração já sugere: pai
   * em cima, filho embaixo, e a seta desce.
   *
   * Ciclo entre tabelas não trava: a conta para quando ninguém mais muda de
   * faixa, e o que sobrou fica onde estava. Um ER com ciclo é raro e legítimo
   * (uma tabela de funcionários que aponta para o chefe, que é funcionário), e
   * travar seria pior que desenhar torto.
   */
  function faixas(m) {
    const nivel = {};
    for (const t of m.tabelas) nivel[chave(t.nome)] = 0;
    for (let volta = 0; volta < m.tabelas.length + 1; volta++) {
      let mudou = false;
      for (const l of m.ligacoes) {
        if (!l.existe) continue;
        const filho = chave(l.de), pai = chave(l.para);
        if (filho === pai) continue;            // aponta para si mesma
        if (nivel[filho] <= nivel[pai]) {
          nivel[filho] = nivel[pai] + 1;
          mudou = true;
        }
      }
      if (!mudou) break;
    }
    const porFaixa = [];
    for (const t of m.tabelas) {
      const n = nivel[chave(t.nome)] || 0;
      (porFaixa[n] ||= []).push(t);
    }
    return porFaixa.map(f => f || []);
  }

  function alturaDe(t) {
    const linhas = Math.min(t.colunas.length, M.maxColunas)
      + (t.total > M.maxColunas ? 1 : 0);
    return M.alturaTitulo + Math.max(linhas, 1) * M.alturaLinha + 8;
  }

  /** Posiciona as caixas: primeiro o layout por faixas, depois as posições
   *  GUARDADAS por cima — quem já arrastou uma tabela a encontra onde deixou,
   *  e a tabela nova entra no lugar que o layout escolheria para ela. */
  function posicionar(m, posicoes) {
    const linhas = faixas(m);
    const temPendurada = m.ligacoes.some(l => !l.existe);
    let y = M.margem + (temPendurada ? 26 : 0);
    linhas.forEach(faixa => {
      let x = M.margem;
      let alta = 0;
      faixa.forEach(t => {
        t.x = x;
        t.y = y;
        t.w = M.larguraCaixa;
        t.h = alturaDe(t);
        x += M.larguraCaixa + M.espacoX;
        alta = Math.max(alta, t.h);
      });
      y += alta + M.espacoY;
    });
    if (posicoes) {
      const guardadas = new Set();
      for (const t of m.tabelas) {
        const p = posicoes[t.nome];
        if (p && isFinite(p.x) && isFinite(p.y)) {
          t.x = Math.max(4, p.x);
          t.y = Math.max(4, p.y);
          guardadas.add(t);
        }
      }
      // A tabela NOVA nasce no slot que o layout escolheria — e uma posição
      // GUARDADA pode estar exatamente ali: `categorias` nasceu embaixo de
      // `clientes` e ficou invisível, com o subtítulo contando quatro tabelas
      // e o desenho mostrando três. Achado no screenshot, não no código.
      // Quem não tem posição guardada e colide com alguém desce para uma
      // faixa livre embaixo do desenho, onde não há com quem colidir.
      const colide = (a, b) =>
        a.x < b.x + b.w + 12 && b.x < a.x + a.w + 12 &&
        a.y < b.y + b.h + 12 && b.y < a.y + a.h + 12;
      let chaoY = null, chaoX = M.margem;
      for (const t of m.tabelas) {
        if (guardadas.has(t)) continue;
        if (!m.tabelas.some(o => o !== t && colide(t, o))) continue;
        if (chaoY === null) {
          chaoY = 0;
          for (const o of m.tabelas) if (o !== t) chaoY = Math.max(chaoY, o.y + o.h);
          chaoY += M.espacoY;
        }
        t.x = chaoX;
        t.y = chaoY;
        chaoX += t.w + M.espacoX;
      }
    }
    return tamanhoDe(m);
  }

  /** O retângulo que contém tudo, com a margem. Recalculado a cada arrasto,
   *  porque arrastar para a direita tem de ESTICAR o desenho — sem isso a
   *  caixa some debaixo da borda da `viewBox`, sem erro nenhum no console. */
  function tamanhoDe(m) {
    let largura = 420, altura = 240;
    const nos = m.tabelas.concat(m.remotas || []);
    for (const t of nos) {
      largura = Math.max(largura, t.x + t.w + M.margem);
      altura = Math.max(altura, t.y + t.h + M.margem + 8);
    }
    return { largura, altura };
  }

  /* ----------------------------------------------------------------- pintar */

  const esc = t => String(t ?? "").replace(/[&<>"']/g, c =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

  /** A linha (índice visível) em que a coluna aparece na caixa, ou -1. */
  function linhaDa(t, coluna) {
    const alvo = chave(coluna);
    for (let i = 0; i < Math.min(t.colunas.length, M.maxColunas); i++) {
      if (chave(t.colunas[i].nome) === alvo) return i;
    }
    return -1;
  }

  /** Onde uma ligação ENCOSTA na caixa: na altura da própria coluna, pelo
   *  lado que estiver virado para o outro nó. Coluna que não coube no desenho
   *  ancora no meio da caixa — o que ao menos é verdade sobre a tabela. */
  function ancora(t, coluna, ladoDireito) {
    const i = coluna == null ? -1 : linhaDa(t, coluna);
    const y = i >= 0
      ? t.y + M.alturaTitulo + 13 + i * M.alturaLinha - 4
      : t.y + Math.min(t.h / 2, M.alturaTitulo + 9);
    return { x: ladoDireito ? t.x + t.w : t.x, y };
  }

  /** Uma caixa de tabela. As coordenadas internas são relativas e o grupo
   *  carrega um `translate` — é o que deixa o arrasto mexer num atributo só,
   *  sem redesenhar a caixa inteira a cada movimento do ponteiro. */
  function caixa(t, editor) {
    const linhas = t.colunas.slice(0, M.maxColunas);
    let y = M.alturaTitulo + 13;
    let corpo = "";
    for (const c of linhas) {
      const cor = c.primaria ? "var(--laranja)"
        : c.estrangeira ? "var(--ndx)" : "var(--texto-2)";
      const marca = c.primaria ? "PK" : c.estrangeira ? "FK" : "";
      corpo += `<text x="11" y="${y}" class="er-marca" fill="${cor}">${marca}</text>`
        + `<text x="36" y="${y}" class="er-col" fill="${cor}">${esc(c.nome)}</text>`
        + `<text x="${t.w - 11}" y="${y}" class="er-tipo">${esc(tipoCurto(c))}</text>`;
      if (editor) {
        // A porta: um retângulo invisível sobre a linha inteira. É dele que
        // se PUXA um relacionamento — e ele existe por cima do texto, senão
        // o alvo do arrasto seria a letra, fina demais para acertar.
        corpo += `<rect x="1" y="${y - 13}" width="${t.w - 2}" height="${M.alturaLinha}"
          class="er-porta" data-tabela="${esc(t.nome)}" data-coluna="${esc(c.nome)}"/>`;
      }
      y += M.alturaLinha;
    }
    if (t.total > M.maxColunas) {
      // Classe própria, e não `er-tipo`: aquela é ancorada à DIREITA (é a
      // coluna do tipo, encostada na borda da caixa), e usá-la aqui jogava o
      // «+ 2 coluna(s)» para fora da caixa, pela esquerda. Só apareceu
      // rolando o desenho até o fim, no navegador.
      corpo += `<text x="11" y="${y}" class="er-mais">`
        + `+ ${t.total - M.maxColunas} coluna(s)</text>`;
    }
    // A pega do arrasto é um retângulo TRANSPARENTE desenhado DEPOIS dos
    // textos do título: em SVG a ordem do documento é a ordem de pintura, e
    // com a pega por baixo o nome da tabela "roubava" o pointerdown — dava
    // para arrastar pela faixa vazia do título, mas não pelo nome, que é
    // exatamente onde todo mundo pega.
    return `<g class="er-tabela" data-tabela="${esc(t.nome)}"
        transform="translate(${t.x} ${t.y})">
      <rect x="0" y="0" width="${t.w}" height="${t.h}" rx="7" class="er-caixa"/>
      <path d="M0 ${M.alturaTitulo} H${t.w}" class="er-risco"/>
      <rect x="0" y="0" width="${t.w}" height="${M.alturaTitulo}" rx="7"
            class="er-topo"/>
      <text x="11" y="20" class="er-nome">${esc(t.nome)}</text>
      <text x="${t.w - 11}" y="20" class="er-conta">${
        t.registros.toLocaleString("pt-BR")}</text>
      ${editor ? `<rect x="0" y="0" width="${t.w}" height="${M.alturaTitulo}"
        rx="7" class="er-pega"/>` : ""}
      ${corpo}</g>`;
  }

  /** O tipo, curto o bastante para caber na direita da caixa.
   *
   * O servidor manda o tipo no formato de depuração do Rust — `Decimal {
   * precisao: 15, escala: 2 }`. Cortar isso nos 14 primeiros caracteres dava
   * «Decimal{precis», que não é o nome de tipo nenhum: parecia um dado
   * truncado quando era só um formato mal lido. Aqui ele é ANALISADO e
   * reescrito, e não recortado. O que não se reconhece vira o nome de fora do
   * bloco, que ao menos é verdade. */
  function tipoCurto(c) {
    const t = String(c.tipo || "").trim();
    const dec = t.match(/^Decimal\s*\{\s*precisao:\s*(\d+),\s*escala:\s*(\d+)\s*\}$/);
    if (dec) return `Decimal(${dec[1]},${dec[2]})`;
    const simples = t.match(/^([A-Za-z]+)\s*\(\s*(\d+)\s*\)$/);
    if (simples) return `${simples[1]}(${simples[2]})`;
    if (/^[A-Za-z0-9]+$/.test(t)) return t;
    // Estrutura desconhecida: fica o nome do tipo, sem os campos de dentro.
    return t.split(/[\s{(]/)[0] || t;
  }

  /** O caminho entre dois pontos ancorados nas laterais: uma curva horizontal
   *  suave, saindo pelo lado que cada caixa tem virado para a outra. */
  function curva(a, b) {
    const folga = Math.max(24, Math.min(64, Math.abs(b.x - a.x) / 2));
    const sa = a.direita ? folga : -folga;
    const sb = b.direita ? folga : -folga;
    return `M${a.x} ${a.y} C${a.x + sa} ${a.y} ${b.x + sb} ${b.y} ${b.x} ${b.y}`;
  }

  /** Dos dois lados de cada caixa, o par que se olha. */
  function lados(a, b) {
    const centroA = a.x + a.w / 2, centroB = b.x + b.w / 2;
    return { deDireita: centroA <= centroB, paraDireita: centroB < centroA };
  }

  /** Uma ligação FK: sai da LINHA da coluna do filho e chega na LINHA da
   *  coluna do pai — a âncora por linha é o que faz o desenho continuar
   *  legível depois que as caixas foram arrastadas para qualquer lugar. */
  function ligacao(l, m, ordem) {
    const a = m.porNome[chave(l.de)];
    const b = m.porNome[chave(l.para)];
    if (!a) return "";
    if (!b) return pendurada(a, l, ordem);
    if (a === b) return laco(a, l);

    const ld = lados(a, b);
    const pa = ancora(a, l.colunas[0], ld.deDireita);
    const pb = ancora(b, l.colunas_ref[0], ld.paraDireita);
    const d = curva({ ...pa, direita: ld.deDireita }, { ...pb, direita: ld.paraDireita });
    const rotulo = l.colunas.join(", ");
    // O rótulo fica AO LADO da linha, e não em cima dela: centrado no meio do
    // caminho, a própria seta passava por dentro das letras e o nome da coluna
    // saía riscado. Não aparece lendo o código — aparece abrindo a tela.
    return `<g class="er-lig" data-fk="${esc(l.nome)}">
      <path d="${d}" class="er-seta" marker-end="url(#er-ponta)"/>
      <text class="er-rot lado" x="${(pa.x + pb.x) / 2 + 7}"
            y="${(pa.y + pb.y) / 2 - 4}">${esc(rotulo)}</text>
    </g>`;
  }

  /** FK que aponta para tabela que não está neste desenho. Ela não some:
   *  sumir seria fingir que a declaração não existe.
   *
   *  O toco sai da DIREITA do topo, e não do meio: no meio ele caía em cima da
   *  seta da chave que existe, e os dois rótulos se sobrepunham. `ordem`
   *  afasta a segunda chave pendurada da mesma tabela. */
  function pendurada(a, l, ordem) {
    const x = a.x + a.w - 30 - (ordem || 0) * 26;
    const y = a.y - 14;
    return `<g class="er-lig solta">
      <path d="M${x} ${a.y} V${y}" class="er-seta solta"/>
      <text class="er-rot solta lado" x="${x + 5}" y="${y - 3}"
        >→ ${esc(l.para)} (fora)</text>
    </g>`;
  }

  /** Tabela que aponta para si mesma — o chefe que também é funcionário. */
  function laco(a, l) {
    const x = a.x + a.w, y = a.y + 14;
    return `<g class="er-lig">
      <path d="M${x} ${y} C${x + 34} ${y - 14} ${x + 34} ${y + 30} ${x} ${y + 22}"
            class="er-seta" marker-end="url(#er-ponta)"/>
      <text class="er-rot" x="${x + 38}" y="${y + 10}">${esc(l.colunas.join(", "))}</text>
    </g>`;
  }

  /* ------------------------------------------------------- DbLink no desenho */

  /** Os nós REMOTOS: uma caixinha tracejada por tabela prima do DbLink, na cor
   *  da ferramenta DbLink (`--memo`), com a ligação de sincronia até a tabela
   *  local. Visual deliberadamente distinto do ER: aquilo não é uma tabela
   *  deste banco — é outra máquina, e o tracejado diz isso. */
  function montarRemotas(m, sincronias) {
    const remotas = [];
    for (const s of (sincronias || [])) {
      const local = m.porNome[chave(s.local_tabela)];
      remotas.push({
        id: `@${s.ligacao}.${s.remota}`,
        ligacao: s.ligacao,
        remota: s.remota,
        sentido: s.sentido || "dois",
        dono: s.dono || "aqui",
        chaveSinc: s.chave || "",
        local,
        w: 190,
        h: 58,
        x: 0, y: 0,
      });
    }
    // Posição padrão: uma COLUNA própria, à direita do diagrama inteiro.
    //
    // A primeira versão punha cada remota «ao lado da tabela local» — e ao
    // lado da tabela local mora a PRÓXIMA tabela do layout. A caixinha nascia
    // em cima do título de `pedidos`, e arrastar `pedidos` arrastava a remota
    // que estava por cima. Só apareceu exercitando o arrasto, nunca lendo o
    // código. Na coluna própria não há com quem colidir, e as remotas se
    // empilham na altura da tabela local de cada uma.
    let bordaDireita = M.margem;
    for (const t of m.tabelas) bordaDireita = Math.max(bordaDireita, t.x + t.w);
    const colunaX = bordaDireita + M.espacoX;
    let livreY = M.margem;
    remotas
      .slice()
      .sort((a, b) => (a.local ? a.local.y : 1e9) - (b.local ? b.local.y : 1e9))
      .forEach(r => {
        r.x = colunaX;
        r.y = Math.max(livreY, r.local ? r.local.y : livreY);
        livreY = r.y + r.h + 14;
      });
    return remotas;
  }

  const SENTIDO = { dois: "⇄", puxar: "→ puxa", empurrar: "empurra →" };

  function caixaRemota(r, editor) {
    // A caixa inteira é pega: uma tabela remota não tem colunas para puxar,
    // então não há gesto disputando com o arrasto.
    return `<g class="er-remota" data-remota="${esc(r.id)}"
        transform="translate(${r.x} ${r.y})">
      <rect x="0" y="0" width="${r.w}" height="${r.h}" rx="7" class="er-caixa-dbl"/>
      <text x="11" y="18" class="er-nome-dbl">${esc(r.remota)}</text>
      <text x="11" y="34" class="er-leg-dbl">DbLink ${esc(r.ligacao)}</text>
      <text x="11" y="48" class="er-leg-dbl">${
        esc(SENTIDO[r.sentido] || r.sentido)} · conflito: ${
        esc(r.dono === "la" ? "lá vence" : "aqui vence")}</text>
      ${editor ? `<rect x="0" y="0" width="${r.w}" height="${r.h}" rx="7"
        class="er-pega"/>` : ""}
    </g>`;
  }

  function ligacaoRemota(r) {
    if (!r.local) return "";
    const t = r.local;
    const ld = lados(r, t);
    const pa = { x: ld.deDireita ? r.x + r.w : r.x, y: r.y + r.h / 2, direita: ld.deDireita };
    const pb = ancora(t, r.chaveSinc || null, ld.paraDireita);
    const d = curva(pa, { ...pb, direita: ld.paraDireita });
    return `<g class="er-lig-dbl">
      <path d="${d}" class="er-seta-dbl"/>
      <text class="er-rot lado dbl" x="${(pa.x + pb.x) / 2 + 7}"
            y="${(pa.y + pb.y) / 2 - 4}">${esc(r.chaveSinc)}</text>
    </g>`;
  }

  /* -------------------------------------------------------------- desenhar */

  function pintarTudo(m, tam, editor) {
    const gastas = {};
    const ligs = m.ligacoes.map(l => {
      if (l.existe) return ligacao(l, m, 0);
      const k = chave(l.de);
      const i = gastas[k] || 0;
      gastas[k] = i + 1;
      return ligacao(l, m, i);
    }).join("");
    const remotas = (m.remotas || []);
    return `<svg class="er${editor ? " editor" : ""}"
      viewBox="0 0 ${tam.largura} ${tam.altura}"
      width="${tam.largura}" height="${tam.altura}"
      preserveAspectRatio="xMinYMin meet" role="img"
      aria-label="Diagrama de entidades e relacionamentos">
      <defs>
        <marker id="er-ponta" viewBox="0 0 10 10" refX="9" refY="5"
                markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0 0 L10 5 L0 10 z" fill="var(--laranja)"/>
        </marker>
      </defs>
      <g class="er-ligs">${ligs}${remotas.map(ligacaoRemota).join("")}</g>
      ${m.tabelas.map(t => caixa(t, editor)).join("")}
      ${remotas.map(r => caixaRemota(r, editor)).join("")}
      <g class="er-tmp"></g>
    </svg>`;
  }

  /** Desenha o diagrama estático (leitura) e devolve o SVG como texto. */
  function desenhar(esquemas) {
    const m = modelo(esquemas);
    if (!m.tabelas.length) return "";
    const tam = posicionar(m, null);
    return pintarTudo(m, tam, false);
  }

  /* ---------------------------------------------------------------- editor */

  /** Converte um evento de ponteiro para coordenadas do DESENHO.
   *
   * `getScreenCTM` já contém o zoom da página, a escala CSS e a rolagem —
   * inverter a matriz é o único jeito que não quebra quando qualquer um dos
   * três muda. Conta feita à mão com `offsetX` fugia do ponteiro no zoom. */
  function noDesenho(svg, ev) {
    const ctm = svg.getScreenCTM();
    if (!ctm) return { x: 0, y: 0 };
    const p = new DOMPoint(ev.clientX, ev.clientY).matrixTransform(ctm.inverse());
    return { x: p.x, y: p.y };
  }

  /** Monta o diagrama EDITÁVEL dentro de `alvo` (elemento ou seletor).
   *
   * op = {
   *   posicoes:  { tabela: {x, y} }        — aplicadas por cima do layout
   *   dblink:    [{ligacao, remota, local_tabela, sentido, dono, chave}]
   *   aoMover:   (posicoes) => {}          — no fim de cada arrasto
   *   aoLigar:   (de, para) => {}          — de/para = {tabela, coluna}
   *   aoAbrir:   (tabela) => {}            — clique na caixa
   *   aoAbrirRemota: (ligacao, remota) => {}
   * }
   */
  function montar(alvo, esquemas, op) {
    op = op || {};
    const raiz = typeof alvo === "string" ? document.querySelector(alvo) : alvo;
    const m = modelo(esquemas);
    if (!m.tabelas.length && !(op.dblink || []).length) {
      raiz.innerHTML = "";
      return { modelo: m };
    }
    posicionar(m, op.posicoes);
    m.remotas = montarRemotas(m, op.dblink);
    if (op.posicoes) {
      for (const r of m.remotas) {
        const p = op.posicoes[r.id];
        if (p && isFinite(p.x) && isFinite(p.y)) { r.x = p.x; r.y = p.y; }
      }
    }
    const tam = tamanhoDe(m);
    raiz.innerHTML = pintarTudo(m, tam, true);
    const svg = raiz.querySelector("svg.er");

    const posAtuais = () => {
      const p = {};
      for (const t of m.tabelas) p[t.nome] = { x: Math.round(t.x), y: Math.round(t.y) };
      for (const r of m.remotas) p[r.id] = { x: Math.round(r.x), y: Math.round(r.y) };
      return p;
    };

    const redesenharLigs = () => {
      const gastas = {};
      svg.querySelector(".er-ligs").innerHTML = m.ligacoes.map(l => {
        if (l.existe) return ligacao(l, m, 0);
        const k = chave(l.de);
        const i = gastas[k] || 0;
        gastas[k] = i + 1;
        return ligacao(l, m, i);
      }).join("") + m.remotas.map(ligacaoRemota).join("");
    };

    const esticar = () => {
      const t = tamanhoDe(m);
      svg.setAttribute("viewBox", `0 0 ${t.largura} ${t.altura}`);
      svg.setAttribute("width", t.largura);
      svg.setAttribute("height", t.altura);
    };

    // ---------------------------------------------------------- arrastos
    // Um estado só para os dois gestos; `tipo` diz qual está acontecendo.
    let arrasto = null;

    svg.addEventListener("pointerdown", ev => {
      if (ev.button !== undefined && ev.button !== 0) return;
      const porta = ev.target.closest(".er-porta");
      const pega = ev.target.closest(".er-pega");
      const grupo = ev.target.closest(".er-tabela, .er-remota");
      if (!grupo) return;
      const p = noDesenho(svg, ev);

      if (porta) {
        // Puxar um relacionamento a partir desta coluna.
        arrasto = {
          tipo: "ligar",
          de: { tabela: porta.dataset.tabela, coluna: porta.dataset.coluna },
          x0: p.x, y0: p.y, mexeu: false,
        };
      } else {
        // Mover a caixa (só pelo título) — ou um clique, se ninguém mexer.
        const no = grupo.classList.contains("er-remota")
          ? m.remotas.find(r => r.id === grupo.dataset.remota)
          : m.porNome[chave(grupo.dataset.tabela)];
        if (!no) return;
        if (!pega && !ev.target.closest(".er-caixa, .er-nome, .er-conta, .er-mais, .er-caixa-dbl, .er-nome-dbl, .er-leg-dbl")) return;
        arrasto = {
          tipo: pega ? "mover" : "clicar",
          no, grupo,
          dx: p.x - no.x, dy: p.y - no.y,
          x0: p.x, y0: p.y, mexeu: false,
        };
        // A caixa arrastada sobe para o topo da pilha — em SVG a ordem do
        // documento é a ordem de pintura, e arrastar por baixo das outras
        // parecia defeito.
        grupo.parentNode.appendChild(grupo);
      }
      svg.setPointerCapture(ev.pointerId);
      ev.preventDefault();
    });

    svg.addEventListener("pointermove", ev => {
      if (!arrasto) return;
      const p = noDesenho(svg, ev);
      if (Math.abs(p.x - arrasto.x0) + Math.abs(p.y - arrasto.y0) > 3) arrasto.mexeu = true;

      if (arrasto.tipo === "ligar") {
        const t = m.porNome[chave(arrasto.de.tabela)];
        const aDireita = p.x >= t.x + t.w / 2;
        const a = ancora(t, arrasto.de.coluna, aDireita);
        svg.querySelector(".er-tmp").innerHTML = `<path class="er-seta-tmp"
          d="${curva({ ...a, direita: aDireita }, { x: p.x, y: p.y, direita: p.x < a.x })}"/>`;
        // O alvo debaixo do ponteiro acende. `elementFromPoint` e não o alvo
        // do evento: com o ponteiro capturado, todo evento "pertence" à
        // origem, e o hover nativo não acontece.
        svg.querySelectorAll(".er-porta.alvo").forEach(e => e.classList.remove("alvo"));
        const sob = document.elementFromPoint(ev.clientX, ev.clientY);
        const alvoPorta = sob && sob.closest ? sob.closest(".er-porta") : null;
        if (alvoPorta && !(alvoPorta.dataset.tabela === arrasto.de.tabela
                           && alvoPorta.dataset.coluna === arrasto.de.coluna)) {
          alvoPorta.classList.add("alvo");
        }
      } else if (arrasto.tipo === "mover") {
        const no = arrasto.no;
        no.x = Math.max(4, p.x - arrasto.dx);
        no.y = Math.max(4, p.y - arrasto.dy);
        arrasto.grupo.setAttribute("transform", `translate(${no.x} ${no.y})`);
        redesenharLigs();
        esticar();
      }
    });

    const soltar = ev => {
      if (!arrasto) return;
      const a = arrasto;
      arrasto = null;
      svg.querySelector(".er-tmp").innerHTML = "";
      svg.querySelectorAll(".er-porta.alvo").forEach(e => e.classList.remove("alvo"));

      if (a.tipo === "ligar") {
        if (!a.mexeu) {
          // Clique parado numa coluna abre o cartão da tabela, como no corpo.
          if (op.aoAbrir) op.aoAbrir(a.de.tabela);
          return;
        }
        const sob = document.elementFromPoint(ev.clientX, ev.clientY);
        const porta = sob && sob.closest ? sob.closest(".er-porta") : null;
        if (porta && op.aoLigar) {
          const para = { tabela: porta.dataset.tabela, coluna: porta.dataset.coluna };
          if (!(para.tabela === a.de.tabela && para.coluna === a.de.coluna)) {
            op.aoLigar(a.de, para);
          }
        }
        return;
      }
      if (a.mexeu) {
        if (op.aoMover) op.aoMover(posAtuais());
      } else if (a.grupo.classList.contains("er-remota")) {
        if (op.aoAbrirRemota) op.aoAbrirRemota(a.no.ligacao, a.no.remota);
      } else if (op.aoAbrir) {
        op.aoAbrir(a.no.nome);
      }
    };
    svg.addEventListener("pointerup", soltar);
    svg.addEventListener("pointercancel", () => {
      arrasto = null;
      svg.querySelector(".er-tmp").innerHTML = "";
      svg.querySelectorAll(".er-porta.alvo").forEach(e => e.classList.remove("alvo"));
    });

    return { modelo: m, posicoes: posAtuais };
  }

  /** O resumo em números, para a tela não recontar nada por fora. */
  function resumo(esquemas) {
    const m = modelo(esquemas);
    return {
      tabelas: m.tabelas.length,
      ligacoes: m.ligacoes.length,
      soltas: m.ligacoes.filter(l => !l.existe).length,
      sem_ligacao: m.tabelas.filter(t =>
        !m.ligacoes.some(l => chave(l.de) === chave(t.nome)
          || chave(l.para) === chave(t.nome))).length,
    };
  }

  // `tipoCurto` sai junto: o cartão da tabela mostrava `Decimal { precisao:
  // 15, escala: 2 }` cru enquanto o desenho já reescrevia — a mesma regra tem
  // de valer nos dois, senão um deles mente sobre o formato.
  return { desenhar, montar, resumo, modelo, faixas, tipoCurto, MEDIDAS: M };
})();

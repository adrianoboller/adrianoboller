/* Diagrama ER do PhxSql — SVG desenhado à mão, sem biblioteca nenhuma.
 *
 * As chaves estrangeiras JÁ estão declaradas e JÁ vêm no `esquema`: cada
 * tabela responde `chaves_estrangeiras` com as colunas daqui, a tabela de lá e
 * o que acontece ao excluir e ao alterar. Faltava só o desenho — e desenho
 * aqui é SVG, que é do que o dossiê inteiro é feito.
 *
 * Este arquivo é um módulo separado do `index.html` de propósito: o layout do
 * grafo é a única parte da interface que é ALGORITMO, e ele merece caber numa
 * tela sem rolar por sete mil linhas.
 *
 * ## O que ele NÃO faz
 *
 * Editar. Arrastar caixa, criar tabela pelo desenho, ligar duas colunas com o
 * mouse: nada disso está aqui, e a tela diz isso em vez de deixar a pessoa
 * descobrir clicando. O editor visual é outra rodada.
 *
 * ## Cores
 *
 * Todas saem das variáveis do console (`--laranja`, `--reg`, `--ndx`…), então
 * o diagrama acompanha o tema claro e o escuro sem uma segunda paleta. SVG
 * aceita `var()` em `fill` e em `stroke`, e é por isso que dá para fazer assim.
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

  /** Posiciona as caixas e devolve o tamanho total do desenho.
   *
   * A margem de cima cresce quando há chave pendurada: o rótulo dela é
   * desenhado ACIMA do topo da caixa, e com a margem normal a metade de cima
   * das letras ficava fora da `viewBox` — cortada, no navegador, sem nenhum
   * erro no console. Achado abrindo a tela, não lendo o código. */
  function posicionar(m) {
    const linhas = faixas(m);
    const temPendurada = m.ligacoes.some(l => !l.existe);
    let y = M.margem + (temPendurada ? 26 : 0);
    let largura = 0;
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
      largura = Math.max(largura, x - M.espacoX + M.margem);
      y += alta + M.espacoY;
    });
    return { largura: Math.max(largura, 420), altura: y - M.espacoY + M.margem };
  }

  /* ----------------------------------------------------------------- pintar */

  const esc = t => String(t ?? "").replace(/[&<>"']/g, c =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

  function caixa(t) {
    const linhas = t.colunas.slice(0, M.maxColunas);
    let y = t.y + M.alturaTitulo + 13;
    let corpo = "";
    for (const c of linhas) {
      const cor = c.primaria ? "var(--laranja)"
        : c.estrangeira ? "var(--ndx)" : "var(--texto-2)";
      const marca = c.primaria ? "PK" : c.estrangeira ? "FK" : "";
      corpo += `<text x="${t.x + 11}" y="${y}" class="er-marca" fill="${cor}">${marca}</text>`
        + `<text x="${t.x + 36}" y="${y}" class="er-col" fill="${cor}">${esc(c.nome)}</text>`
        + `<text x="${t.x + t.w - 11}" y="${y}" class="er-tipo">${esc(tipoCurto(c))}</text>`;
      y += M.alturaLinha;
    }
    if (t.total > M.maxColunas) {
      // Classe própria, e não `er-tipo`: aquela é ancorada à DIREITA (é a
      // coluna do tipo, encostada na borda da caixa), e usá-la aqui jogava o
      // «+ 2 coluna(s)» para fora da caixa, pela esquerda. Só apareceu
      // rolando o desenho até o fim, no navegador.
      corpo += `<text x="${t.x + 11}" y="${y}" class="er-mais">`
        + `+ ${t.total - M.maxColunas} coluna(s)</text>`;
    }
    return `<g class="er-tabela" data-tabela="${esc(t.nome)}">
      <rect x="${t.x}" y="${t.y}" width="${t.w}" height="${t.h}" rx="7"
            class="er-caixa"/>
      <path d="M${t.x} ${t.y + M.alturaTitulo} H${t.x + t.w}" class="er-risco"/>
      <rect x="${t.x}" y="${t.y}" width="${t.w}" height="${M.alturaTitulo}"
            rx="7" class="er-topo"/>
      <text x="${t.x + 11}" y="${t.y + 20}" class="er-nome">${esc(t.nome)}</text>
      <text x="${t.x + t.w - 11}" y="${t.y + 20}" class="er-conta">${
        t.registros.toLocaleString("pt-BR")}</text>
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

  /** Uma ligação: sai da borda de baixo do filho e chega na de cima do pai,
   *  ou pelas laterais quando as duas estão na mesma faixa. */
  function ligacao(l, m, ordem) {
    const a = m.porNome[chave(l.de)];
    const b = m.porNome[chave(l.para)];
    if (!a) return "";
    if (!b) return pendurada(a, l, ordem);
    if (a === b) return laco(a, l);

    const [ax, ay] = [a.x + a.w / 2, a.y];
    const [bx, by] = [b.x + b.w / 2, b.y + b.h];
    // O filho está abaixo do pai (o normal): sobe do topo do filho para a
    // base do pai, com uma curva suave em vez de bico.
    const meio = (ay + by) / 2;
    const d = Math.abs(ay - by) > 8
      ? `M${ax} ${ay} C${ax} ${meio} ${bx} ${meio} ${bx} ${by}`
      : `M${a.x + a.w} ${a.y + 18} C${a.x + a.w + 40} ${a.y + 18} `
        + `${b.x - 40} ${b.y + 18} ${b.x} ${b.y + 18}`;
    const rotulo = l.colunas.join(", ");
    // O rótulo fica AO LADO da linha, e não em cima dela: centrado no meio do
    // caminho, a própria seta passava por dentro das letras e o nome da coluna
    // saía riscado. Não aparece lendo o código — aparece abrindo a tela.
    return `<g class="er-lig" data-fk="${esc(l.nome)}">
      <path d="${d}" class="er-seta" marker-end="url(#er-ponta)"/>
      <text class="er-rot lado" x="${(ax + bx) / 2 + 7}"
            y="${meio + 3}">${esc(rotulo)}</text>
    </g>`;
  }

  /** FK que aponta para tabela que não está neste desenho. Ela não some:
   *  some seria fingir que a declaração não existe.
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

  /** Desenha o diagrama inteiro e devolve o SVG como texto. */
  function desenhar(esquemas) {
    const m = modelo(esquemas);
    if (!m.tabelas.length) return "";
    const tam = posicionar(m);
    return `<svg class="er" viewBox="0 0 ${tam.largura} ${tam.altura}"
      width="${tam.largura}" height="${tam.altura}"
      preserveAspectRatio="xMinYMin meet" role="img"
      aria-label="Diagrama de entidades e relacionamentos">
      <defs>
        <marker id="er-ponta" viewBox="0 0 10 10" refX="9" refY="5"
                markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0 0 L10 5 L0 10 z" fill="var(--laranja)"/>
        </marker>
      </defs>
      ${(() => {
        // Quantas chaves penduradas esta tabela já gastou: é o que afasta a
        // segunda da primeira, em vez de as duas saírem do mesmo ponto.
        const gastas = {};
        return m.ligacoes.map(l => {
          if (l.existe) return ligacao(l, m, 0);
          const k = chave(l.de);
          const i = gastas[k] || 0;
          gastas[k] = i + 1;
          return ligacao(l, m, i);
        }).join("");
      })()}
      ${m.tabelas.map(caixa).join("")}
    </svg>`;
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

  return { desenhar, resumo, modelo, faixas, MEDIDAS: M };
})();

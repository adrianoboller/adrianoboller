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
 * ## Três cuidados que só aparecem exercitando
 *
 * 1. **Nada pisca.** O laço não redesenha o painel: ele ATUALIZA os elementos
 *    que já existem, um `<g>` por atividade, achado pelo id. Redesenhar tudo
 *    a cada volta faria o clique do operador cair no vazio a cada dois
 *    segundos, e o cartão aberto fecharia sozinho.
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
    // Raio mínimo: abaixo disto o identificador não cabe dentro e a bolha
    // deixa de dizer quem é — que é a única coisa que ela existe para dizer.
    raioMin: 17,
    raioMax: 74,
    margem: 10,
    // Passo da espiral de empacotamento, em pixels.
    passo: 7,
  };

  /* Os três níveis, com o que NÃO é cor junto de cada um.
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

  /* ------------------------------------------------- empacotamento das bolhas
   *
   * Espiral gulosa: a maior vai ao centro e cada seguinte anda pela espiral
   * até achar um lugar que não encoste em nenhuma já posta. É determinístico
   * — mesma entrada, mesmo desenho — e não precisa de biblioteca nenhuma.
   *
   * Por que espiral e não uma grade: numa grade a bolha grande deixaria um
   * buraco do tamanho dela em volta, e o painel ficaria vazio no meio de uma
   * carga pesada, que é justamente quando ele precisa mostrar alguma coisa.
   */
  function empacotar(itens, larg, alt) {
    const postas = [];
    const cx = larg / 2, cy = alt / 2;
    itens.forEach(it => {
      const r = it.raio;
      let x = cx, y = cy;
      // Volta 0: o centro. Depois a espiral abre de `passo` em `passo`.
      for (let t = 0; t < 20000; t++) {
        const ang = t * 0.45;
        const raioEspiral = M.passo * ang / (2 * Math.PI) * 2.2;
        x = cx + Math.cos(ang) * raioEspiral;
        y = cy + Math.sin(ang) * raioEspiral * 0.62; // achatado: o painel é largo
        if (x - r < M.margem || x + r > larg - M.margem) continue;
        if (y - r < M.margem || y + r > alt - M.margem) continue;
        let bate = false;
        for (const p of postas) {
          const dx = p.x - x, dy = p.y - y;
          if (dx * dx + dy * dy < (p.raio + r + 4) * (p.raio + r + 4)) { bate = true; break; }
        }
        if (!bate) break;
      }
      // Se a espiral acabou sem achar lugar -- painel pequeno demais para
      // tantas bolhas --, ela fica na borda em vez de sair voando para fora
      // do desenho. Encostar em outra é feio; sumir da tela é mentira.
      it.x = Math.min(Math.max(x, M.margem + r), larg - M.margem - r);
      it.y = Math.min(Math.max(y, M.margem + r), alt - M.margem - r);
      postas.push(it);
    });
    return itens;
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
        <span class="tlm-painel-t">Atividades</span>
        <span class="tlm-painel-s" id="tlmResumo"></span>
      </div>
      <svg class="tlm-bolhas" id="tlmBolhas" role="group"
           aria-label="atividades vivas, uma bolha por atividade"></svg>
      <div class="tlm-legenda">
        <span class="tlm-leg" data-n="normal"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>azul · normal</span>
        <span class="tlm-leg" data-n="alto"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>amarelo · uso alto <b>▲</b> borda tracejada</span>
        <span class="tlm-leg" data-n="stress"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>vermelho · stress <b>■</b> borda pontilhada</span>
        <span class="tlm-leg" data-n="encerrando"><svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>rosa · encerrando <b>✕</b></span>
        <span class="tlm-leg-nota">o tamanho é o peso: milissegundos de servidor que a atividade já gastou</span>
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
    $("#tlmBolhas").addEventListener("click", ev => {
      const g = ev.target.closest("[data-id]");
      if (!g) return;
      estado.selecionada = g.dataset.id;
      desenharCartao();
      marcarSelecao();
    });
    volta();
    estado.timer = setInterval(() => { if (!estado.pausado) volta(); }, estado.periodo);
  }

  function parar() {
    if (estado.timer) clearInterval(estado.timer);
    estado.timer = null;
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

    desenharBolhas(d.atividades || []);
    desenharThreads(d.threads || []);
    desenharCartao();
  }

  /* ------------------------------------------------------------- as bolhas */

  function desenharBolhas(ativs) {
    const d0 = estado.ultimo || {};
    const svg = $("#tlmBolhas");
    if (!svg) return;
    const caixa = svg.getBoundingClientRect();
    const larg = Math.max(320, caixa.width || 700);
    const alt = Math.max(200, caixa.height || 300);
    svg.setAttribute("viewBox", `0 0 ${larg} ${alt}`);

    // O raio sai da RAIZ do peso, e não do peso: o olho compara ÁREA de
    // círculo, então usar o peso direto no raio faria uma atividade duas
    // vezes mais pesada parecer quatro vezes maior. É o erro clássico de
    // gráfico de bolha, e ele mente para o lado do exagero.
    const maiorPeso = ativs.reduce((m, a) => Math.max(m, num(a.peso_ms)), 1);
    const itens = ativs.map(a => ({
      d: a,
      raio: Math.max(M.raioMin,
        Math.min(M.raioMax, M.raioMin + (M.raioMax - M.raioMin) * Math.sqrt(num(a.peso_ms) / maiorPeso))),
    }));

    // Se as bolhas juntas não cabem, TODAS encolhem pelo mesmo fator.
    //
    // Encolher só as que não couberam quebraria a promessa do painel — o
    // tamanho deixaria de ser o peso. Um fator único preserva a proporção
    // inteira: a maior continua sendo a maior, e na mesma razão. Foi o
    // navegador que mostrou a necessidade: com oito atividades pesadas, três
    // pares de bolhas ficaram sobrepostos.
    //
    // 42% da área é folga generosa de propósito — círculos não azulejam, e o
    // empacotamento por espiral desperdiça o espaço entre eles.
    const area = itens.reduce((t, i) => t + Math.PI * i.raio * i.raio, 0);
    const cabe = larg * alt * 0.42;
    if (area > cabe) {
      const fator = Math.sqrt(cabe / area);
      itens.forEach(i => { i.raio = Math.max(9, i.raio * fator); });
    }
    empacotar(itens, larg, alt);

    const vistos = new Set();
    itens.forEach(it => {
      const a = it.d;
      const id = a.id;
      vistos.add(id);
      const niv = NIVEIS[a.nivel] || NIVEIS.normal;
      let g = svg.querySelector(`[data-id="${CSS.escape(id)}"]`);
      if (!g) {
        // Nasce onde vai ficar, e não no canto: uma bolha que nasce em 0,0 e
        // desliza até o lugar chama a atenção para o nada.
        g = document.createElementNS("http://www.w3.org/2000/svg", "g");
        g.setAttribute("data-id", id);
        g.setAttribute("class", "tlm-bolha");
        g.setAttribute("tabindex", "0");
        g.setAttribute("role", "button");
        g.innerHTML =
          `<circle class="tlm-c"></circle>
           <text class="tlm-id" text-anchor="middle"></text>
           <text class="tlm-sub" text-anchor="middle"></text>
           <title></title>`;
        svg.appendChild(g);
      }
      const circulo = g.querySelector(".tlm-c");
      circulo.setAttribute("cx", it.x.toFixed(1));
      circulo.setAttribute("cy", it.y.toFixed(1));
      circulo.setAttribute("r", it.raio.toFixed(1));
      circulo.setAttribute("fill", niv.cor);
      circulo.setAttribute("stroke", niv.cor);
      circulo.setAttribute("stroke-dasharray", niv.traco);

      const curto = id.replace(/^dados:/, "#").replace(/^web:/, "w·").slice(0, 9);
      const rotulo = g.querySelector(".tlm-id");
      rotulo.setAttribute("x", it.x.toFixed(1));
      rotulo.setAttribute("y", (it.y + (it.raio > 26 ? -1 : 4)).toFixed(1));
      rotulo.setAttribute("font-size", Math.max(8, Math.min(15, it.raio / 2.6)).toFixed(1));
      // Abaixo de 14 px de raio nem o identificador cabe; melhor a bolha
      // limpa e o texto no tooltip do que uma letra cortada pela borda.
      rotulo.textContent = it.raio >= 14 ? curto : "";

      const sub = g.querySelector(".tlm-sub");
      // Só a bolha grande tem espaço para a segunda linha. A pequena diz tudo
      // pelo tooltip e pelo cartão — encher de texto ilegível seria pior.
      sub.setAttribute("x", it.x.toFixed(1));
      sub.setAttribute("y", (it.y + 12).toFixed(1));
      sub.setAttribute("font-size", "9.5");
      sub.textContent = it.raio > 26 ? (niv.glifo + " " + (a.op || "ociosa")).slice(0, 16) : "";

      // O tooltip e o `aria-label` dizem em PALAVRAS o que a cor diz: é o que
      // mantém o painel legível para quem não distingue as três.
      g.querySelector("title").textContent =
        `${id}${d0.voce === id ? " (a sua própria tela)" : ""} · ${niv.rot} · `
        + `${a.op || "sem operação"} · peso ${dur(a.peso_ms)}`;
      // A bolha de quem está olhando o painel leva o nome disso. Escondê-la
      // seria mentir sobre quem está conectado; deixá-la anônima faria o
      // operador procurar quem é «w·85a62fd» — e é ele mesmo.
      const euMesmo = d0.voce === id;
      g.classList.toggle("eu", !!euMesmo);
      g.setAttribute("aria-label",
        `${id}${euMesmo ? ", esta é a sua própria tela" : ""}, ${niv.rot}, `
        + `${a.op || "sem operação"}, ${a.estado}, peso ${dur(a.peso_ms)}`);
    });

    // O que sumiu sai. Sem isto, a conexão que caiu ficaria desenhada para
    // sempre — e um painel que mostra quem já foi embora não serve para nada.
    svg.querySelectorAll("[data-id]").forEach(g => {
      if (!vistos.has(g.dataset.id)) g.remove();
    });
    marcarSelecao();

    const d = estado.ultimo || {};
    const t = d.totais || {};
    $("#tlmResumo").textContent =
      `${ativs.length} viva(s) · ${ativs.filter(a => a.op).length} executando · `
      + `${num(t.encerramentos)} encerramento(s) desde que subiu`;
  }

  function marcarSelecao() {
    document.querySelectorAll("#tlmBolhas [data-id]").forEach(g => {
      g.classList.toggle("sel", g.dataset.id === estado.selecionada);
    });
  }

  /* ------------------------------------------------------------- o cartão */

  function linha(rot, val) {
    return `<div class="tlm-l"><span>${esc(rot)}</span><b>${esc(val ?? "—")}</b></div>`;
  }

  function desenharCartao() {
    const alvo = $("#tlmCartao");
    if (!alvo) return;
    const d = estado.ultimo || {};
    const a = (d.atividades || []).find(x => x.id === estado.selecionada);
    if (!a) {
      alvo.innerHTML = `<div class="tlm-vazio">clique numa bolha para ver o descritivo completo</div>`;
      return;
    }
    const niv = NIVEIS[a.nivel] || NIVEIS.normal;
    // O botão só aparece quando o servidor diz que a fase é cancelável. Um
    // botão que não cumpre o que promete é pior do que botão nenhum — e quem
    // sabe se ela é cancelável AGORA é o servidor, não a tela.
    // O botão segue `tem_ponto` — «esta operação tem onde parar» —, e não
    // `cancelavel`, que é «ela está nesse ponto neste instante». A diferença
    // apareceu exercitando: uma soma de verificação parada na fila da trava
    // tinha `cancelavel:false` por um instante, o botão sumia, e encerrá-la
    // funcionava perfeitamente. Botão que some do nada é tão ruim quanto
    // botão que não cumpre.
    const podeEncerrar = !!a.op && !!a.tem_ponto && !a.encerrando;
    alvo.innerHTML = `
      <div class="tlm-cartao-cab" style="--n:${niv.cor}">
        <span class="tlm-nivel">${esc(niv.glifo)} ${esc(niv.rot)}</span>
        <b>${esc(a.id)}</b>
        ${d.voce === a.id ? `<span class="tlm-eu">esta é a sua tela</span>` : ""}
      </div>
      ${linha("estado", a.estado)}
      ${linha("operação", a.op || "nenhuma em curso")}
      ${linha("alvo", a.alvo)}
      ${linha("usuário", a.usuario)}
      ${linha("origem", a.origem + (a.ip ? " · " + a.ip : ""))}
      ${linha("conectada desde", a.desde)}
      ${linha("aberta há", dur(num(a.aberta_s) * 1000))}
      ${linha("operação iniciada", a.op_desde)}
      ${linha("operação dura há", dur(a.ha_ms))}
      ${linha("desse tempo, trabalhando", dur(a.trabalhando_ms))}
      ${linha("desse tempo, na fila da trava", dur(a.esperou_ms))}
      ${linha("peso (servidor gasto)", dur(a.peso_ms))}
      ${linha("pedidos já feitos", a.pedidos)}
      ${linha("unidades percorridas", a.passos)}
      ${linha("fase", a.fase)}
      ${linha("trava de dados", a.com_trava ? "na mão desta atividade" : "não")}
      ${linha("esperando", a.esperando_o_que)}
      ${linha("já encerrada", a.encerradas + " vez(es)")}
      <div class="tlm-acoes">
        ${podeEncerrar
          ? `<button class="botao excluir" id="tlmEncerrar" type="button">Encerrar a operação</button>`
          : `<button class="botao excluir" type="button" disabled
               title="${esc(a.encerrando ? "já está encerrando"
                 : a.op ? "esta operação não tem ponto de cancelamento: vai terminar"
                 : "não há operação em curso")}">Encerrar a operação</button>`}
        ${a.ligacao ? `<button class="botao marcar" id="tlmDerrubar" type="button">Derrubar a conexão</button>` : ""}
      </div>
      <p class="tlm-nota">${
        a.encerrando
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

  return { html, iniciar, parar, desenhar, empacotar, faixa };
})();

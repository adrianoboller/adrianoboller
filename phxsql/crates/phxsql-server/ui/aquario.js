/* Aquario do PhxSql (pedido 707) — a fisica das bolhas (A9), a tela ligada
 * ao servidor (A10) e a linha do tempo com os tres graficos (A11).
 *
 * Cada tarefa viva do servidor e uma BOLHA: nasce pequena, cresce com o tempo
 * que leva, colide com as vizinhas sem se sobrepor e ESTOURA quando a tarefa
 * some do retrato. Faixas horizontais por operacao seguram as bolhas no seu
 * canto sem prende-las.
 *
 * ## Duas camadas
 *
 * - `criar(host)` e a FISICA: recebe a lista de tarefas por `atualizar` e nao
 *   fala com o servidor. E assim que ela se exercita sem servidor nenhum
 *   (`bancada/aquario/a9-pagina.html`).
 * - `tela(host, { api })` e a LIGACAO (A10/A11): pede `aquario_retrato`,
 *   `aquario_log` e `aquario_contagens`, e entrega o retrato a fisica.
 *
 * ## O que a TV mostra (decisao do dono, 09/10/2026)
 *
 * So OPERACAO, TABELA, COR e o usuario PSEUDONIMIZADO. A fisica copia do
 * retrato exatamente os campos que desenha — tarefa, op, tabela, cor, faixa,
 * motivo, ms, pseudonimo — e NUNCA toca em `usuario` ou `ip`. O pseudonimo e
 * do servidor (HMAC do login com um sal que so ele tem): a tela nao sabe
 * desfaze-lo, e por isso pode mostra-lo.
 *
 * O unico lugar que le `usuario` e `ip` e o CARTAO DE ENCERRAR (A12), e ele so
 * existe quando o servidor diz `completo` — a mesma pergunta com que ele
 * recusaria o `telemetria_encerrar`. Quem so monitora nao recebe os campos,
 * nao ve o cartao e, se forjar o pedido, e recusado no servidor.
 *
 * ## Modo TV e frescor (A13)
 *
 * `tv: true` cobre a pagina inteira e nunca abre o cartao. O SELO de frescor
 * diz ha quanto tempo chegou o ultimo retrato; passou de 3 s, ou o ultimo
 * pedido falhou, a tela inteira se declara VELHA — painel congelado mente
 * pior que painel vazio. A VOLTA DE 5 MIN pausa o ao vivo e reconstroi o
 * tanque pelo `aquario.log`: cada `estourou` diz quando a tarefa acabou e
 * quanto durou, e isso basta para saber quem estava no tanque em cada
 * instante.
 *
 * ## Quatro cuidados
 *
 * 1. Sem sobreposicao e INVARIANTE, nao tendencia: depois da integracao rodam
 *    varias passagens de relaxamento, e se mesmo assim a area das bolhas nao
 *    cabe na caixa, os raios ENCOLHEM (`escala`) em vez de se sobreporem.
 * 2. Nada e recriado por volta: um <g> por tarefa, achado pelo id; o laco so
 *    muda atributos. Clique no meio de uma volta nao cai no vazio.
 * 3. A cor nao e o unico sinal: forma e traco acompanham (circulo, losango,
 *    octogono, tracejado). Contorno, nunca fundo cheio.
 * 4. O CSS global morde: tudo escopado em `.aq`/`.aqt`, sem <label> (o
 *    `label{text-transform:uppercase}` da pagina faria «Blumenau» virar
 *    «BLUMENAU»), e o `input{width:100%}` desfeito onde nao cabe.
 *
 * ## Texto
 *
 * Rotulo pela fabrica de idiomas (`tela.aq_*`), por CHAVE: o motivo da cor
 * chega do servidor como chave (`aquario.motivo.*`, neutra de idioma, a mesma
 * do `aquario.log`) e a tabela `MOTIVOS` abaixo a troca pela chave da tela —
 * nunca pela frase. Operacao, tabela e database sao DADO: entram como vieram.
 */
"use strict";

window.PhxAquario = (function () {

  const SVG = "http://www.w3.org/2000/svg";

  /* A fabrica da pagina, quando ha pagina; o portugues de fabrica, quando o
   * modulo roda sozinho (a bancada da A9). */
  function txt(nome, padrao) {
    return window.txt ? window.txt(nome, padrao) : padrao;
  }
  function preencher(bruto, dados) {
    return String(bruto).replace(/\{(\w+)\}/g,
      (m, k) => (dados && k in dados) ? String(dados[k]) : m);
  }
  const esc = t => String(t == null ? "" : t).replace(/[&<>"']/g, c =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

  /* Faixas, de cima para baixo. O servidor diz a faixa de cada tarefa
   * (`faixa`, pela mesma decisao da contagem); sem ela, `outras`. */
  const FAIXAS = ["select", "insert", "update", "delete", "outras"];
  const ROTULO_DA_FAIXA = {
    select: () => txt("tela.aq_faixa_select", "consulta"),
    insert: () => txt("tela.aq_faixa_insert", "inclusão"),
    update: () => txt("tela.aq_faixa_update", "alteração"),
    delete: () => txt("tela.aq_faixa_delete", "exclusão"),
    outras: () => txt("tela.aq_faixa_outras", "outras"),
  };

  const ROTULO_DA_COR = {
    verde: () => txt("tela.tl_nivel_normal", "normal"),
    azul_claro: () => txt("tela.aq_cor_azul_claro", "grande, normal"),
    azul_escuro: () => txt("tela.aq_cor_azul_escuro", "grande, acima do habitual"),
    amarelo: () => txt("tela.aq_cor_amarelo", "aviso"),
    vermelho: () => txt("tela.aq_cor_vermelho", "alarme"),
    rosa: () => txt("tela.aq_cor_rosa", "encerrando"),
  };

  /* O motivo da cor, pela CHAVE que o servidor manda. Uma linha por chave,
   * escrita por extenso: o laco da fabrica acha chave usada procurando o
   * literal, e chave montada de prefixo + nome seria chave morta para ele. */
  const MOTIVOS = {
    // o motivo e a cor dizem a mesma coisa aqui: uma chave so, nao duas
    "aquario.motivo.encerrando": () => txt("tela.aq_cor_rosa", "encerrando"),
    "aquario.motivo.ociosa": () => txt("tela.aq_m_ociosa", "conexão parada, sem pedido"),
    "aquario.motivo.esperando_a_trava": () => txt("tela.aq_m_esperando_a_trava", "esperando a trava de dados"),
    "aquario.motivo.segurando_a_trava": () => txt("tela.aq_m_segurando_a_trava", "segurando a trava de dados com fila atrás"),
    "aquario.motivo.acima_do_tempo_fixo": () => txt("tela.aq_m_acima_do_tempo_fixo", "acima do tempo de alerta"),
    "aquario.motivo.grande": () => txt("tela.aq_m_grande", "tarefa grande, dentro do habitual"),
    "aquario.motivo.no_habitual": () => txt("tela.aq_m_no_habitual", "dentro do habitual"),
    "aquario.motivo.trava_reentrante": () => txt("tela.aq_m_trava_reentrante", "pediu a trava que já segurava"),
    "aquario.motivo.trava_envenenada": () => txt("tela.aq_m_trava_envenenada", "trava envenenada por uma falha de outra tarefa"),
    "aquario.motivo.erro_de_disco": () => txt("tela.aq_m_erro_de_disco", "erro de disco"),
    "aquario.motivo.dado_corrompido": () => txt("tela.aq_m_dado_corrompido", "dado corrompido"),
    "aquario.motivo.transacao_acima_do_teto": () => txt("tela.aq_m_transacao_acima_do_teto", "transação acima do teto da réplica"),
    "aquario.motivo.forca_bruta": () => txt("tela.aq_m_forca_bruta", "tentativas seguidas de login recusado"),
    "aquario.motivo.senha_em_claro": () => txt("tela.aq_m_senha_em_claro", "senha chegando sem cifra"),
    "aquario.motivo.prazo_estourado": () => txt("tela.aq_m_prazo_estourado", "cancelada pelo prazo"),
    "aquario.motivo.fora_do_habitual": () => txt("tela.aq_m_fora_do_habitual", "acima do habitual desta operação"),
    "aquario.motivo.fora_do_habitual_reincidente": () => txt("tela.aq_m_fora_do_habitual_reincidente", "acima do habitual, de novo"),
    "aquario.motivo.integridade_recusada": () => txt("tela.aq_m_integridade_recusada", "integridade recusou: o dado está protegido"),
    "aquario.motivo.fecho_recusado": () => txt("tela.aq_m_fecho_recusado", "fecho da janela de escrita recusado"),
    "aquario.motivo.fsync_recusado_antes": () => txt("tela.aq_m_fsync_recusado_antes", "o disco recusou gravar num arranque anterior"),
    "aquario.motivo.marca_nao_resolvida": () => txt("tela.aq_m_marca_nao_resolvida", "marca de recuperação não resolvida"),
    "aquario.motivo.indice_atrasado": () => txt("tela.aq_m_indice_atrasado", "índice ficou para trás no arranque"),
    "aquario.motivo.continuidade_rompida": () => txt("tela.aq_m_continuidade_rompida", "a continuidade da réplica rompeu"),
    "aquario.motivo.origem_inalcancavel": () => txt("tela.aq_m_origem_inalcancavel", "a origem da réplica não responde"),
    "aquario.motivo.firewall_bloqueou": () => txt("tela.aq_m_firewall_bloqueou", "o firewall bloqueou um endereço"),
    "aquario.motivo.disco_lento": () => txt("tela.sd_tipo_lento", "disco lento"),
    "aquario.motivo.esgotamento_previsto": () => txt("tela.aq_m_esgotamento_previsto", "um recurso esgota em menos de um dia"),
    "aquario.motivo.esgotamento_iminente": () => txt("tela.aq_m_esgotamento_iminente", "um recurso esgota em menos de duas horas"),
    "aquario.motivo.replica_atrasada": () => txt("tela.aq_m_replica_atrasada", "uma tabela da réplica está ficando para trás da origem"),
    "aquario.motivo.injecao_suspeita": () => txt("tela.aq_m_injecao_suspeita", "pedido com forma de injeção de SQL"),
    "aquario.motivo.plano_largo": () => txt("tela.aq_m_plano_largo", "o plano alcança metade da tabela ou mais"),
    "aquario.motivo.comando_bloqueado": () => txt("tela.aq_m_comando_bloqueado", "comando perigoso bloqueado: faltou a senha de execução"),
    "aquario.motivo.senha_de_execucao_recusada": () => txt("tela.aq_m_senha_de_execucao_recusada", "a senha de execução não conferiu"),
  };
  /* Chave que esta versao da tela nao conhece (servidor mais novo): sai a
   * chave crua, que e honesta — inventar uma frase seria pior. */
  function motivo(chave) {
    const f = MOTIVOS[chave];
    return f ? f() : String(chave || "");
  }

  const EVENTOS = {
    nasceu: () => txt("tela.aq_ev_nasceu", "nasceu"),
    mudou: () => txt("tela.aq_ev_mudou", "mudou de cor"),
    estourou: () => txt("tela.aq_ev_estourou", "terminou"),
    morta: () => txt("tela.tl_th_encerrada", "encerrada"),
    sedimento: () => txt("tela.aq_ev_sedimento", "alarme do servidor"),
    anel: () => txt("tela.aq_ev_anel", "ganhou um anel"),
  };

  /* O motivo do anel (pedido 780), o mesmo texto no toque, no cartao e no
   * log. `k` e `m` vem do SERVIDOR (o retrato ou a linha `anel`): a escala
   * mora no `aquario/anel.rs`, e a tela nao a recalcula. */
  function textoDoAnel(ms, k, m) {
    return preencher(txt("tela.aq_anel_motivo", "rodando há {min} min, anel {k} de {m}"),
      { min: Math.floor((+ms || 0) / 60000), k: k, m: m });
  }

  /* cor -> { forma, variavel, tracejado } (aquario-707.md §2.3). */
  const CORES = {
    verde:       { forma: "circulo",  v: "--ok",             fb: "#6cc98c", traco: "",     esp: 1.5 },
    azul_claro:  { forma: "circulo",  v: "--reg",            fb: "#5fa6e8", traco: "",     esp: 2 },
    azul_escuro: { forma: "circulo",  v: "--aq-azul-escuro", fb: "#9cc3ff", traco: "",     esp: 3, dupla: true },
    amarelo:     { forma: "losango",  v: "--ambar",          fb: "#ffc43d", traco: "6 4",  esp: 2 },
    vermelho:    { forma: "octogono", v: "--vermelho",       fb: "#ff5f5f", traco: "2 3",  esp: 3 },
    rosa:        { forma: "circulo",  v: "--acao-marcar",    fb: "#ff8fc7", traco: "10 4", esp: 2 },
  };

  const M = {
    rMin: 9,            // px, bolha recem-nascida que ja existe
    rMax: 46,           // px, a maior (tarefa de 30 s ou mais)
    msGrande: 30000,    // tarefa que atinge rMax (= TETO_DO_RAIO_MS do aquario/anel.rs, teste confere)
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

  /* A faixa da tarefa: a que o servidor disse, ou o nome da faixa quando o
   * retrato e inventado (a bancada da A9 manda `op: "select"`). */
  function faixaDe(t) {
    const f = String(t.faixa || t.op || "").toLowerCase();
    return FAIXAS.indexOf(f) >= 0 ? f : "outras";
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
        const faixa = faixaDe(t);
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
        b.op = t.op; b.tabela = t.tabela; b.motivo = t.motivo; b.pessoa = t.pseudonimo || "";
        b.cor = CORES[t.cor] ? t.cor : "verde";
        b.faixa = faixa; b.ms = t.ms || 0;
        // O anel (780) e o total deles chegam prontos; sem o total (a bancada
        // da A9, um retrato antigo) nao ha anel nenhum -- inventar a escala
        // aqui seria a segunda formula.
        b.aneis = Math.max(0, Math.floor(+t.aneis || 0));
        b.anel = b.aneis ? Math.min(b.aneis, Math.max(0, Math.floor(+t.anel || 0))) : 0;
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
.aq .aq-faixa-nome{fill:var(--mudo,#8b98b4);font-size:11px;letter-spacing:.04em;text-transform:none;
  paint-order:stroke;stroke:var(--fundo,#010418);stroke-width:3px;stroke-linejoin:round;pointer-events:none}
.aq .aq-b{text-transform:none}
.aq .aq-b .aq-forma{fill-opacity:.14}
.aq .aq-b .aq-dupla{fill:none}
.aq .aq-b .aq-rot{fill:var(--texto,#e6ecf7);text-anchor:middle;font-size:10px;pointer-events:none;
  paint-order:stroke;stroke:var(--fundo,#010418);stroke-width:2px;text-transform:none}
.aq .aq-b.aq-fim .aq-forma{fill-opacity:0}
/* Os ANEIS (780): cada um e a forma da bolha em ponto menor, com um traco
   DUPLO -- escuro por baixo, claro por cima --, que aparece tanto sobre o
   papel quanto sobre o miolo preto. A cor da gravidade fica na borda. */
.aq .aq-b .aq-anel{fill:#000;fill-opacity:.22;stroke:none}
.aq .aq-b .aq-anel-e{fill:none;stroke:#000;stroke-width:2.6}
.aq .aq-b .aq-anel-c{fill:none;stroke:#f2f5fb;stroke-width:1.1}
.aq .aq-b.aq-preta .aq-forma{fill:#000;fill-opacity:1}
.aq .aq-b.aq-preta .aq-anel{fill-opacity:0}
/* o nome sobre o miolo preto: claro com contorno preto nos DOIS temas -- o
   --texto do tema claro e escuro, e sumiria no preto */
.aq .aq-b.aq-preta .aq-rot{fill:#f2f5fb;stroke:#000}
/* O BRILHO da preta: preto sobre o #010418 some, como o azul escuro; o halo
   na cor do texto e o que a separa do fundo. */
.aq .aq-b .aq-halo{fill:none;stroke:var(--texto,#e6ecf7);stroke-width:1.5}
.aq .aq-dica{position:absolute;z-index:4;max-width:min(360px,80%);padding:6px 9px;border-radius:6px;
  background:var(--painel,#0a1122);border:1px solid var(--linha-forte,#2b3a56);color:var(--texto,#e6ecf7);
  font-size:12px;line-height:1.4;pointer-events:none;text-transform:none;box-shadow:0 4px 14px rgba(0,0,0,.35)}
.aq .aq-dica[hidden]{display:none}
.aq.aq-clic .aq-b:not(.aq-fim){cursor:pointer}
.aq .aq-b:focus{outline:none}
.aq .aq-b:focus-visible .aq-forma,.aq .aq-b.aq-sel .aq-forma{stroke-width:4px}
.aq .aq-vazio{position:absolute;inset:0;display:none;align-items:center;justify-content:center;
  color:var(--mudo,#8b98b4)}
.aq.aq-sem .aq-vazio{display:flex}

/* A TELA (A10/A11). Sem rotulo de formulario nem tabela: o CSS global morde
   os dois, e a lista do log e a legenda dos graficos sao listas. */
.aqt{display:flex;flex-direction:column;gap:10px;font-family:"Exo 2",system-ui,sans-serif;
  color:var(--texto,#e6ecf7);text-transform:none;min-width:0}
.aqt *{text-transform:none}
.aqt-topo{display:flex;flex-wrap:wrap;align-items:center;gap:8px 14px;min-width:0}
.aqt-estado{font-size:12.5px;color:var(--texto-2,#a8b0c0);min-height:1.4em;flex:1;min-width:200px}
.aqt-estado.aqt-mal{color:var(--vermelho,#ff5f5f)}
/* O SELO de frescor: contorno, nunca fundo cheio, e a palavra VELHO escrita
   -- a cor nao e o unico sinal. */
.aqt-selo{flex:none;font-size:12px;font-weight:600;letter-spacing:.03em;padding:3px 9px;border-radius:999px;
  border:1px solid var(--linha-forte,#2b3a56);color:var(--texto-2,#a8b0c0);font-variant-numeric:tabular-nums}
.aqt-selo[data-estado="fresco"]{border-color:var(--ok,#6cc98c);color:var(--ok,#6cc98c)}
.aqt-selo[data-estado="velho"]{border:2px dashed var(--vermelho,#ff5f5f);color:var(--vermelho,#ff5f5f)}
.aqt-selo[data-estado="reprise"]{border:2px dotted var(--acao-consultar,#5fa6e8);color:var(--acao-consultar,#5fa6e8)}
.aqt.aqt-velho .aqt-agua{border:2px dashed var(--vermelho,#ff5f5f)}
.aqt.aqt-velho .aqt-agua .aq svg{filter:grayscale(.85) brightness(.5)}
.aqt.aqt-reprise .aqt-agua{border:2px dotted var(--acao-consultar,#5fa6e8)}
.aqt-volta{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.aqt-volta[hidden],.aqt-reprise[hidden]{display:none}
.aqt-reprise{display:flex;align-items:center;gap:8px}
.aqt .aqt-trilho{width:180px;flex:none;padding:0;accent-color:var(--acao-consultar,#5fa6e8)}
.aqt-quando{font-size:12px;color:var(--texto-2,#a8b0c0);font-variant-numeric:tabular-nums;min-width:7ch}
.aqt .botao.aqt-bt{width:auto;padding:4px 10px;font-size:11.5px;font-weight:600}
.aqt-tanque{position:relative}

/* O CARTAO de encerrar (A12): sobre o tanque, na propria pagina -- e a
   confirmacao, e nao um confirm() do navegador, que nao diz o que se perde. */
.aqt-cartao{position:absolute;top:10px;right:10px;z-index:5;width:min(420px,calc(100% - 20px));
  max-height:calc(100% - 20px);overflow:auto;background:var(--painel,#0a1122);
  border:1px solid var(--acao-excluir,#ff5f5f);border-radius:8px;padding:12px 14px;
  box-shadow:0 6px 24px rgba(0,0,0,.45);display:flex;flex-direction:column;gap:8px}
.aqt-cartao[hidden]{display:none}
.aqt-cartao h4{margin:0;font-size:14px;font-weight:700;color:var(--texto,#e6ecf7)}
.aqt-ficha{display:grid;grid-template-columns:auto minmax(0,1fr);gap:3px 10px;font-size:12px;margin:0}
.aqt-ficha .aqt-r{color:var(--texto-3,#848da0)}
.aqt-ficha .aqt-v{color:var(--texto,#e6ecf7);overflow-wrap:anywhere}
.aqt-promessa,.aqt-perde{font-size:12px;line-height:1.45;margin:0;color:var(--texto-2,#a8b0c0)}
.aqt-perde{border-left:3px solid var(--acao-excluir,#ff5f5f);padding-left:8px}
.aqt-acoes{display:flex;flex-wrap:wrap;gap:8px}
.aqt-res{font-size:12.5px;margin:0;color:var(--texto,#e6ecf7)}
.aqt-res.aqt-mal{color:var(--vermelho,#ff5f5f)}
.aqt-assina{font-size:11px;color:var(--texto-3,#848da0);margin:0}

/* MODO TV (A13): a pagina inteira e o aquario. Os menus e as barras somem
   por #app[data-tv], no index.html; aqui o aquario cobre o resto. */
.aqt.aqt-tv{position:fixed;inset:0;z-index:60;padding:14px 18px;background:var(--fundo,#010418);overflow:auto}
.aqt.aqt-tv .aqt-tanque,.aqt.aqt-tv .aqt-log{height:calc(100vh - 80px);max-height:none}
.aqt.aqt-tv .aqt-selo{font-size:15px;padding:5px 12px}
.aqt-corpo{display:grid;grid-template-columns:minmax(0,2.2fr) minmax(240px,1fr) minmax(250px,1fr);
  gap:12px;align-items:stretch;min-width:0}
@media (max-width:1180px){.aqt-corpo{grid-template-columns:minmax(0,1fr) minmax(0,1fr)}
  .aqt-tanque{grid-column:1/-1}}
@media (max-width:760px){.aqt-corpo{grid-template-columns:minmax(0,1fr)}}
.aqt-tanque{display:flex;gap:8px;min-width:0;height:clamp(320px,64vh,1400px)}
.aqt-agua{flex:1;min-width:0;border:1px solid var(--linha,#1e2940);border-radius:8px;overflow:hidden}
.aqt-fundo{width:150px;flex:none;display:flex;flex-direction:column;gap:6px;overflow:auto;
  border:1px solid var(--linha,#1e2940);border-radius:8px;padding:8px;background:var(--painel,#0a1122)}
.aqt-fundo[hidden]{display:none}
.aqt-cab{font-size:11px;letter-spacing:.06em;color:var(--texto-3,#848da0);margin:0 0 4px;font-weight:600}
.aqt-pedra{display:flex;gap:6px;align-items:flex-start;font-size:11.5px;line-height:1.3;color:var(--texto,#e6ecf7)}
.aqt-chip{flex:none;width:14px;height:14px;display:inline-block;vertical-align:-2px}
.aqt-chip svg{width:14px;height:14px;display:block}
.aqt-log,.aqt-graf{min-width:0;border:1px solid var(--linha,#1e2940);border-radius:8px;
  background:var(--painel,#0a1122);padding:10px;display:flex;flex-direction:column;gap:8px}
.aqt-log{max-height:clamp(320px,64vh,1400px)}
.aqt-filtros{display:flex;gap:6px}
.aqt .aqt-busca{flex:1;min-width:0;width:auto;padding:6px 8px;font-size:12.5px}
.aqt .aqt-periodo{width:auto;flex:none;padding:6px 6px;font-size:12.5px}
.aqt-lista{list-style:none;margin:0;padding:0;overflow:auto;flex:1;font-size:12px}
.aqt-lista li{display:grid;grid-template-columns:auto 14px minmax(0,1fr);gap:2px 6px;align-items:start;
  padding:5px 2px;border-bottom:1px solid var(--linha,#1e2940)}
.aqt-lista time{color:var(--texto-3,#848da0);font-variant-numeric:tabular-nums}
.aqt-lista .aqt-o{color:var(--texto,#e6ecf7);overflow-wrap:anywhere}
.aqt-lista .aqt-o b{font-weight:600}
.aqt-lista .aqt-mot{grid-column:3;color:var(--texto-2,#a8b0c0)}
.aqt-nada{color:var(--texto-3,#848da0);font-size:12px;padding:6px 2px}
.aqt-g{display:flex;flex-direction:column;gap:4px;padding-bottom:8px;border-bottom:1px solid var(--linha,#1e2940)}
.aqt-g:last-of-type{border-bottom:0}
.aqt-g h4{margin:0;font-size:12.5px;font-weight:600;color:var(--texto,#e6ecf7)}
.aqt-g .aqt-sub{font-size:11px;color:var(--texto-3,#848da0)}
.aqt-g svg{width:100%;height:96px;display:block}
.aqt-g .aqt-barra{fill-opacity:.22;stroke-width:1.5}
.aqt-g .aqt-eixo{stroke:var(--linha-forte,#2b3a56);stroke-width:1}
.aqt-g .aqt-vazio-g{fill:none;stroke:var(--linha-forte,#2b3a56);stroke-dasharray:3 3}
.aqt-g .aqt-num{fill:var(--texto,#e6ecf7);font-size:10px;text-anchor:middle;font-variant-numeric:tabular-nums}
.aqt-leg{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:1fr 1fr;gap:2px 10px;font-size:11px}
.aqt-leg li{display:flex;gap:5px;align-items:center;color:var(--texto-2,#a8b0c0);min-width:0}
.aqt-leg li span:last-child{margin-left:auto;color:var(--texto,#e6ecf7);font-variant-numeric:tabular-nums}
.aqt-amostra{flex:none;width:10px;height:10px;border:1.5px solid currentColor;border-radius:2px}
.aqt-nota{font-size:11px;color:var(--texto-3,#848da0);line-height:1.4;margin:0}

/* A ALCA no painel da telemetria: estica pelo canto (resize nativo), e o
   tanque ocupa o que sobrar abaixo da linha de estado. */
.aq-alca{margin:12px 0;border:1px solid var(--linha,#1e2940);border-radius:8px;background:var(--painel,#0a1122)}
.aq-alca>summary{cursor:pointer;padding:8px 12px;font-size:13px;font-weight:600;color:var(--texto-2,#a8b0c0)}
.aq-alca-caixa{height:360px;min-height:220px;max-height:92vh;resize:vertical;overflow:hidden;padding:0 10px 10px}
.aq-alca-caixa .aqt-tanque{height:auto;flex:1;min-height:0}
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

  function garantirCss() {
    if (!document.getElementById("aq-css")) {
      const s = document.createElement("style");
      s.id = "aq-css"; s.textContent = CSS; document.head.appendChild(s);
    }
  }

  /** `criar(host, { auto, colisao, semente })` — devolve o aquario. */
  function criar(host, opcoes) {
    const op = opcoes || {};
    garantirCss();
    host.classList.add("aq");
    const svg = el("svg", { role: "img" });
    svg.setAttribute("aria-label", txt("tela.aq_bolhas_al", "tarefas vivas, uma bolha por tarefa"));
    // As linhas das faixas EMBAIXO das bolhas; os nomes POR CIMA. Com os
    // nomes no grupo de baixo, uma bolha parada na faixa os apagava -- e a
    // faixa e justamente onde elas param.
    const gFaixas = el("g"), gBolhas = el("g"), gNomes = el("g");
    svg.append(gFaixas, gBolhas, gNomes);
    const vazio = document.createElement("div");
    vazio.className = "aq-vazio"; vazio.textContent = txt("tela.aq_vazio", "nenhuma tarefa em andamento");
    // A DICA do toque (780): quem nao pode encerrar (a TV, quem so monitora)
    // toca na bolha e le o que o <title> diz -- no toque nao ha hover.
    const dica = document.createElement("div");
    dica.className = "aq-dica"; dica.hidden = true; dica.setAttribute("role", "status");
    host.append(svg, vazio, dica);
    let dicaRelogio = 0;
    function mostrarDica(g, ev) {
      const t = g.querySelector("title");
      dica.textContent = t ? t.textContent : "";
      const caixaHost = host.getBoundingClientRect();
      const x = (ev && ev.clientX != null ? ev.clientX : caixaHost.left) - caixaHost.left;
      const y = (ev && ev.clientY != null ? ev.clientY : caixaHost.top) - caixaHost.top;
      dica.style.left = Math.max(4, Math.min(x + 10, caixaHost.width - 200)) + "px";
      dica.style.top = Math.max(4, Math.min(y + 10, caixaHost.height - 60)) + "px";
      dica.setAttribute("data-id", g.getAttribute("data-id") || "");
      dica.hidden = false;
      clearTimeout(dicaRelogio);
      dicaRelogio = setTimeout(() => { dica.hidden = true; }, 5000);
    }

    const caixa = { w: host.clientWidth || 800, h: host.clientHeight || 400 };
    const motor = criarMotor(caixa, op);
    const nos = new Map();     // id -> { g, forma, dupla, rot, tit }
    const stats = { quadros: 0, msFrame: 0 };
    // A bolha CLICAVEL so existe quando quem esta olhando pode encerrar: a
    // tela pergunta ao retrato (`completo`), nunca decide sozinha.
    let clic = false, selecionada = "";
    function aoEscolher(ev) {
      const g = ev.target && ev.target.closest && ev.target.closest(".aq-b");
      if (!g || g.classList.contains("aq-fim")) return;
      if (!clic || !op.aoClicar) {
        if (ev.type === "click") mostrarDica(g, ev);
        return;
      }
      if (ev.type === "keydown" && ev.key !== "Enter" && ev.key !== " ") return;
      ev.preventDefault();
      op.aoClicar(g.getAttribute("data-id"));
    }
    gBolhas.addEventListener("click", aoEscolher);
    gBolhas.addEventListener("keydown", aoEscolher);
    function tornarClicavel(g) {
      if (clic) { g.setAttribute("tabindex", "0"); g.setAttribute("role", "button"); }
      else { g.removeAttribute("tabindex"); g.removeAttribute("role"); }
    }

    function medir() {
      caixa.w = host.clientWidth || caixa.w; caixa.h = host.clientHeight || caixa.h;
      svg.setAttribute("viewBox", `0 0 ${caixa.w} ${caixa.h}`);
      gFaixas.textContent = ""; gNomes.textContent = "";
      FAIXAS.forEach(function (f, i) {
        const y = (i + 0.5) * caixa.h / FAIXAS.length;
        gFaixas.append(el("line", { class: "aq-faixa", x1: 0, x2: caixa.w, y1: y, y2: y }));
        const n = el("text", { class: "aq-faixa-nome", x: 8, y: y - 4 });
        n.textContent = ROTULO_DA_FAIXA[f]();
        gNomes.append(n);
      });
    }
    medir();
    let observador = null;
    if (window.ResizeObserver) { observador = new ResizeObserver(medir); observador.observe(host); }

    function noDe(b) {
      let n = nos.get(b.id);
      if (n) return n;
      const g = el("g", { class: "aq-b", "data-id": b.id, "data-cor": b.cor });
      const tit = el("title");
      const ehCirculo = CORES[b.cor].forma === "circulo";
      const halo = el(ehCirculo ? "circle" : "polygon", { class: "aq-halo" });
      halo.style.display = "none";
      const forma = el(ehCirculo ? "circle" : "polygon", { class: "aq-forma" });
      const gAneis = el("g", { class: "aq-aneis" });
      const dupla = el("circle", { class: "aq-dupla" });
      const rot = el("text", { class: "aq-rot", y: 3 });
      g.append(tit, halo, forma, gAneis, dupla, rot);
      tornarClicavel(g);
      gBolhas.append(g);
      n = { g: g, forma: forma, dupla: dupla, rot: rot, tit: tit, cor: b.cor, halo: halo, gAneis: gAneis,
            aneis: [], desenho: "" };
      nos.set(b.id, n);
      return n;
    }

    /* Os aneis de uma bolha (780): `anel` de `aneis`, cada um a forma da
     * bolha no raio r·(1 − i/(aneis+1)) -- espacamento igual, o miolo some no
     * ultimo. Cada anel escurece o que esta dentro dele (fill .22, que se
     * acumula para o centro); no ultimo a bolha inteira e preta, com o halo.
     * So refaz os nos quando o anel ou o total mudam; o raio muda todo quadro
     * e so mexe em atributos. */
    function pintarAneis(n, b, c, r) {
      const k = b.anel || 0, m = b.aneis || 0;
      const preta = k > 0 && k >= m;
      const desenho = k + "/" + m;
      if (n.desenho !== desenho) {
        n.desenho = desenho;
        n.gAneis.textContent = "";
        n.aneis = [];
        for (let i = 1; i <= k; i++) {
          const tag = c.forma === "circulo" ? "circle" : "polygon";
          const disco = el(tag, { class: "aq-anel" });
          const escuro = el(tag, { class: "aq-anel-e" });
          const claro = el(tag, { class: "aq-anel-c" });
          n.gAneis.append(disco, escuro, claro);
          n.aneis.push([disco, escuro, claro]);
        }
        n.g.setAttribute("data-anel", String(k));
        n.g.setAttribute("data-aneis", String(m));
        n.g.classList.toggle("aq-preta", preta);
      }
      for (let i = 1; i <= n.aneis.length; i++) {
        const ri = r * (1 - i / (m + 1));
        for (const no of n.aneis[i - 1]) {
          if (c.forma === "circulo") no.setAttribute("r", ri.toFixed(1));
          else no.setAttribute("points", pontos(c.forma, ri));
        }
      }
      if (preta) {
        if (c.forma === "circulo") n.halo.setAttribute("r", (r + 2.5).toFixed(1));
        else n.halo.setAttribute("points", pontos(c.forma, r + 2.5));
        n.halo.style.display = "";
      } else n.halo.style.display = "none";
    }

    function desenhar() {
      const vivas = new Set();
      for (const b of motor.bolhas) {
        let n = noDe(b);
        // A forma e um elemento diferente por cor (circulo x poligono): a
        // bolha que muda de verde para amarelo troca de no, senao o losango
        // seria desenhado com `r` num <circle> e nada apareceria.
        if (n.cor !== b.cor && CORES[n.cor].forma !== CORES[b.cor].forma) {
          n.g.remove(); nos.delete(b.id); n = noDe(b);
        }
        n.cor = b.cor;
        n.g.setAttribute("data-cor", b.cor);
        vivas.add(b.id);
        const c = CORES[b.cor];
        const r = b.estourando ? b.rEstouro * (1 + 0.6 * b.idade / M.estouroMs) : b.r;
        if (r <= 0.3) { n.g.style.display = "none"; continue; }
        n.g.style.display = "";
        n.g.setAttribute("transform", `translate(${b.x.toFixed(1)} ${b.y.toFixed(1)})`);
        const cor = `var(${c.v},${c.fb})`;
        n.forma.setAttribute("stroke", cor);
        n.forma.setAttribute("fill", cor);
        n.forma.setAttribute("stroke-width", c.esp);
        if (c.traco) n.forma.setAttribute("stroke-dasharray", c.traco);
        else n.forma.removeAttribute("stroke-dasharray");
        if (c.forma === "circulo") n.forma.setAttribute("r", r.toFixed(1));
        else n.forma.setAttribute("points", pontos(c.forma, r));
        pintarAneis(n, b, c, r);
        // borda dupla clara do azul escuro: e o que o torna visivel (3,04:1)
        if (b.id === selecionada && !b.estourando) {
          // a escolhida do cartao ganha um anel de fora, tracejado na cor do
          // texto: e a mesma bolha que o cartao descreve
          n.dupla.setAttribute("r", (r + 5).toFixed(1));
          n.dupla.setAttribute("stroke", "var(--texto,#e6ecf7)"); n.dupla.setAttribute("stroke-width", 1.5);
          n.dupla.setAttribute("stroke-dasharray", "3 3");
          n.dupla.style.display = "";
        } else if (c.dupla) {
          n.dupla.setAttribute("r", Math.max(0, r - 5).toFixed(1));
          n.dupla.setAttribute("stroke", cor); n.dupla.setAttribute("stroke-width", 1.5);
          n.dupla.removeAttribute("stroke-dasharray");
          n.dupla.style.display = "";
        } else n.dupla.style.display = "none";
        n.g.setAttribute("opacity", b.estourando ? (1 - b.idade / M.estouroMs).toFixed(2) : "1");
        n.g.classList.toggle("aq-fim", !!b.estourando);
        n.g.classList.toggle("aq-sel", b.id === selecionada && !b.estourando);
        if (!b.estourando) {
          // rotulo e DADO: texto cru, sem caixa alta imposta
          // Cabe o que cabe na bolha, e o corte se ANUNCIA com reticencias:
          // o nome inteiro esta no <title>. Encolher a letra ou mudar a caixa
          // seria mexer na aparencia do dado; cortar dizendo que cortou, nao.
          const nome = String(b.tabela || b.op || "");
          const cabe = Math.floor(2 * r / 5.6);
          n.rot.textContent = r < M.rotuloMin ? ""
            : nome.length <= cabe ? nome : nome.slice(0, Math.max(1, cabe - 1)) + "…";
          const tx = String(b.op || "") + " " + String(b.tabela || "") +
            " (" + ROTULO_DA_COR[b.cor]() + ")" + (b.motivo ? " — " + motivo(b.motivo) : "") +
            (b.anel ? " · " + textoDoAnel(b.ms, b.anel, b.aneis) : "") +
            (b.pessoa ? " · " + preencher(txt("tela.aq_pessoa", "pessoa {p}"), { p: b.pessoa }) : "");
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
      // A tela foi trocada: o laco morre sozinho, sem esperar alguem lembrar
      // de para-lo.
      if (!host.isConnected && stats.quadros > 0) { raf = 0; return; }
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
      rodando: function () { return raf !== 0; },
      soltar: function () { api.parar(); if (observador) observador.disconnect(); },
      clicavel: function (sim) {
        if (clic === !!sim) return;
        clic = !!sim;
        host.classList.toggle("aq-clic", clic);
        for (const n of nos.values()) tornarClicavel(n.g);
      },
      selecionar: function (id) { selecionada = id || ""; },
    };
    if (op.auto !== false) api.iniciar();
    return api;
  }

  /* ------------------------------------------------- a ligacao (A10, A11) */

  /* O desenho miudo da cor, para o log e o fundo: a mesma forma da bolha. */
  function chip(cor) {
    const c = CORES[cor] || CORES.verde;
    const s = el("svg", { viewBox: "-8 -8 16 16", "aria-hidden": "true" });
    const f = c.forma === "circulo" ? el("circle", { r: 6 }) : el("polygon", { points: pontos(c.forma, 6.5) });
    const tinta = `var(${c.v},${c.fb})`;
    f.setAttribute("stroke", tinta); f.setAttribute("fill", tinta);
    f.setAttribute("fill-opacity", ".18"); f.setAttribute("stroke-width", "1.6");
    s.append(f);
    const span = document.createElement("span");
    span.className = "aqt-chip"; span.append(s);
    return span;
  }

  /* As oito series do `aquario_contagens`, na ordem do servidor. A cor de
   * cada barra e a da ACAO (verde inclui, amarelo altera, rosa marca, vermelho
   * exclui de vez, azul consulta); erro e aviso, a da gravidade. */
  const SERIES = [
    { k: "select", v: "--acao-consultar", fb: "#5fa6e8", rot: () => txt("tela.aq_faixa_select", "consulta") },
    { k: "insert", v: "--acao-incluir", fb: "#6cc98c", rot: () => txt("tela.aq_faixa_insert", "inclusão") },
    { k: "update", v: "--acao-alterar", fb: "#ffc43d", rot: () => txt("tela.aq_faixa_update", "alteração") },
    { k: "excluir_suave", v: "--acao-marcar", fb: "#ff8fc7", rot: () => txt("tela.aq_s_excluir_suave", "exclusão suave") },
    { k: "excluir_fisico", v: "--acao-excluir", fb: "#ff5f5f", rot: () => txt("tela.aq_s_excluir_fisico", "exclusão de vez") },
    { k: "backup", v: "--texto-2", fb: "#a8b0c0", rot: () => txt("tela.fer_backup", "Backup") },
    { k: "erro", v: "--vermelho", fb: "#ff5f5f", rot: () => txt("tela.aq_s_erro", "erros"), propria: true },
    { k: "aviso", v: "--ambar", fb: "#ffc43d", rot: () => txt("tela.aq_s_aviso", "avisos"), propria: true },
  ];

  const MIN = 60000, DIA = 86400000;

  /* Meia-noite LOCAL de `ms`, menos `dias`. O motor nao tem fuso (tudo UTC);
   * o dia e o do navegador, e por isso a soma mora aqui (§11.4, Hc5). */
  function meiaNoite(ms, dias) {
    const d = new Date(ms);
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - (dias || 0));
    return d.getTime();
  }

  /* Soma o periodo [inicio, agora): horas fechadas + minutos da hora corrente
   * + o minuto parcial. Devolve as somas, quantos minutos foram MEDIDOS e o
   * instante do ultimo dado. Minuto `null` nao conta: ausente nao e zero. */
  function somar(r, inicio) {
    const soma = {}; for (const s of SERIES) soma[s.k] = 0;
    let medidos = 0, ultimo = 0;
    const juntar = c => { for (const s of SERIES) soma[s.k] += +((c || {})[s.k] || 0); };
    for (const h of r.horas || []) {
      if (h.hora_ms < inicio) continue;
      juntar(h.c); medidos += +h.minutos_medidos || 0;
      if ((+h.minutos_medidos || 0) > 0) ultimo = Math.max(ultimo, h.hora_ms + 59 * MIN);
    }
    const hc = r.hora_corrente;
    if (hc && Array.isArray(hc.minutos)) {
      hc.minutos.forEach((c, i) => {
        const m = hc.hora_ms + i * MIN;
        if (c == null || m < inicio) return;
        juntar(c); medidos += 1; ultimo = Math.max(ultimo, m);
      });
    }
    const mp = r.minuto_parcial;
    if (mp && mp.c && mp.minuto_ms >= inicio) { juntar(mp.c); ultimo = Math.max(ultimo, mp.minuto_ms); }
    return { soma, medidos, ultimo, vivo: !!(mp && mp.c) };
  }

  function hora(ms) {
    return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" });
  }
  function data(ms) {
    return new Date(ms).toLocaleDateString([], { day: "2-digit", month: "2-digit" });
  }

  /* Um grafico: seis barras na escala das operacoes, e erro/aviso na escala
   * PROPRIA, do outro lado do traco (ordem do dono: senao viram filete ao
   * lado dos SELECT). */
  function desenharGrafico(caixa, total, inicio, agora) {
    const svg = caixa.querySelector("svg");
    const leg = caixa.querySelector(".aqt-leg");
    const sub = caixa.querySelector(".aqt-sub");
    const W = 260, H = 96, base = H - 14, alto = base - 12;
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.textContent = "";
    const decorridos = Math.max(1, Math.round((agora - inicio) / MIN));
    const ops = SERIES.filter(s => !s.propria), prop = SERIES.filter(s => s.propria);
    const maxA = Math.max(1, ...ops.map(s => total.soma[s.k]));
    const maxB = Math.max(1, ...prop.map(s => total.soma[s.k]));
    const larg = 22, passo = 30;
    svg.append(el("line", { class: "aqt-eixo", x1: 0, x2: W, y1: base + 0.5, y2: base + 0.5 }));
    const xDiv = 6 * passo + 8;
    svg.append(el("line", { class: "aqt-eixo", x1: xDiv, x2: xDiv, y1: 4, y2: base }));
    const nada = total.medidos === 0 && !total.vivo;
    SERIES.forEach((s, i) => {
      const x = (s.propria ? xDiv + 10 + (i - 6) * passo : 4 + i * passo);
      const v = total.soma[s.k];
      const tinta = `var(${s.v},${s.fb})`;
      if (nada) {
        // ausente nao e zero: sem medida, a barra e so o contorno vazio
        svg.append(el("rect", { class: "aqt-vazio-g", x: x, y: base - alto, width: larg, height: alto }));
        return;
      }
      const h = Math.max(v > 0 ? 2 : 0, alto * v / (s.propria ? maxB : maxA));
      const r = el("rect", { class: "aqt-barra", x: x, y: base - h, width: larg, height: h,
        stroke: tinta, fill: tinta, "data-serie": s.k, "data-valor": v });
      const t = el("title"); t.textContent = s.rot() + ": " + v; r.append(t);
      svg.append(r);
      const n = el("text", { class: "aqt-num", x: x + larg / 2, y: Math.max(9, base - h - 3) });
      n.textContent = String(v);
      svg.append(n);
    });
    leg.textContent = "";
    for (const s of SERIES) {
      const li = document.createElement("li");
      li.setAttribute("data-serie", s.k);
      const a = document.createElement("span"); a.className = "aqt-amostra";
      a.style.color = `var(${s.v},${s.fb})`;
      const r = document.createElement("span"); r.textContent = s.rot();
      const v = document.createElement("span"); v.textContent = nada ? "—" : String(total.soma[s.k]);
      li.append(a, r, v); leg.append(li);
    }
    const cobre = Math.min(100, Math.round(100 * total.medidos / decorridos));
    sub.textContent = nada
      ? txt("tela.aq_g_sem_medida", "sem medida neste período — ausente, não zero")
      : preencher(txt("tela.aq_g_sub", "medido {p}% do período · último dado {quando}"),
          { p: cobre, quando: data(total.ultimo) + " " + hora(total.ultimo) });
  }

  /* O selo passa a VELHO quando o ultimo retrato bom tem mais que isto (o
   * desenho, §6: «passou de 3 s»). A volta pede a cada 2 s; com a resposta
   * de um servidor vivo a idade nunca passa de ~2,1 s. */
  const LIMITE_VELHO_MS = 3000;
  /* A volta de cinco minutos: a janela e a velocidade da reprise (10×, os
   * cinco minutos em trinta segundos). */
  const VOLTA_MS = 5 * MIN, REPRISE_PASSO_MS = 100, REPRISE_VEZES = 10;

  /* Duracao para quem le: ms ate 1 s, segundos com uma casa ate 1 min. */
  function dur(ms) {
    ms = +ms || 0;
    if (ms < 1000) return Math.round(ms) + " ms";
    if (ms < 60000) return (ms / 1000).toFixed(1) + " s";
    return Math.floor(ms / 60000) + " min " + Math.round((ms % 60000) / 1000) + " s";
  }

  /* O desfecho do `telemetria_encerrar`, pela CHAVE do estado que o servidor
   * devolve -- nunca pela frase do `aviso`, que e prosa do servidor. */
  const DESFECHO = {
    encerrando: () => txt("tela.aq_k_res_encerrando", "encerrando: a operação aborta no próximo ponto seguro."),
    marcada: () => txt("tela.aq_k_res_marcada", "marcada: a marca vale no primeiro ponto seguro que vier."),
    nao_cancelavel: () => txt("tela.aq_k_res_nao_cancelavel", "não cancelável agora: está dentro do ponto crítico e vai terminar."),
    ociosa: () => txt("tela.aq_k_res_ociosa", "não havia operação em curso."),
  };

  /** `tela(host, { api, compacto, periodo, tv, quem })` — o aquario ligado ao
   *  servidor.
   *
   *  `compacto` e a ALCA da telemetria: so o tanque, sem log, graficos, volta
   *  nem cartao (quem administra encerra pela ficha da telemetria, ao lado).
   *  `tv` e a parede: cobre a pagina e nunca abre o cartao. `quem` e o login
   *  de quem assina o encerramento, mostrado a ele mesmo no cartao.
   *  Devolve `{ parar, retomar, soltar, pedidos }`: `pedidos` conta as
   *  chamadas feitas, e e a medida da prova «aba escondida faz 0 pedidos». */
  function tela(host, cfg) {
    const c = cfg || {};
    const api = c.api;
    const periodo = c.periodo || 2000;
    const completa = !c.compacto;
    garantirCss();
    host.classList.add("aqt");
    host.classList.toggle("aqt-tv", !!c.tv);
    const topo = `<div class="aqt-topo">
           <div class="aqt-estado" role="status" aria-live="polite"></div>
           ${completa ? `<div class="aqt-volta">
             <button type="button" class="botao consultar aqt-bt aqt-bt-volta">${esc(txt("tela.aq_volta", "Voltar 5 min"))}</button>
             <span class="aqt-reprise" hidden>
               <button type="button" class="botao consultar aqt-bt aqt-bt-tocar"></button>
               <input type="range" class="aqt-trilho" min="0" max="${VOLTA_MS}" step="1000" value="0"
                      aria-label="${esc(txt("tela.aq_volta_al", "instante da reprise"))}">
               <time class="aqt-quando"></time>
               <button type="button" class="botao secundario aqt-bt aqt-bt-vivo">${esc(txt("tela.aq_ao_vivo", "Ao vivo"))}</button>
             </span>
           </div>` : ""}
           <span class="aqt-selo" data-estado="espera"
                 title="${esc(preencher(txt("tela.aq_selo_al", "frescor do retrato: passou de {s} s sem um retrato novo, a tela se declara VELHA"), { s: LIMITE_VELHO_MS / 1000 }))}"></span>
         </div>`;
    const cartao = `<section class="aqt-cartao" hidden role="dialog" aria-labelledby="aqtCartaoTit">
               <h4 id="aqtCartaoTit">${esc(txt("tela.aq_k_titulo", "Encerrar esta tarefa?"))}</h4>
               <div class="aqt-ficha"></div>
               <p class="aqt-promessa"></p>
               <p class="aqt-perde"></p>
               <div class="aqt-acoes">
                 <button type="button" class="botao excluir aqt-bt aqt-k-encerrar">${esc(txt("tela.tl_encerrar", "Encerrar a operação"))}</button>
                 <button type="button" class="botao excluir aqt-bt aqt-k-derrubar">${esc(txt("tela.tl_derrubar", "Derrubar a conexão"))}</button>
                 <button type="button" class="botao secundario aqt-bt aqt-k-fechar">${esc(txt("tela.fechar", "Fechar"))}</button>
               </div>
               <p class="aqt-res" role="status" aria-live="polite"></p>
               <p class="aqt-assina"></p>
             </section>`;
    host.innerHTML = c.compacto
      ? `${topo}
         <div class="aqt-tanque"><div class="aqt-agua"></div><aside class="aqt-fundo" hidden></aside></div>`
      : `${topo}
         <div class="aqt-corpo">
           <div class="aqt-tanque">
             <div class="aqt-agua"></div>
             <aside class="aqt-fundo" hidden></aside>
             ${cartao}
           </div>
           <section class="aqt-log" aria-label="${esc(txt("tela.aq_log_titulo", "Linha do tempo"))}">
             <h4 class="aqt-cab">${esc(txt("tela.aq_log_titulo", "Linha do tempo"))}</h4>
             <div class="aqt-filtros">
               <input class="aqt-busca" type="search" autocomplete="off"
                      placeholder="${esc(txt("tela.aq_log_busca_dica", "procurar operação, tabela, cor ou motivo…"))}"
                      aria-label="${esc(txt("tela.aq_log_busca_dica", "procurar operação, tabela, cor ou motivo…"))}">
               <select class="aqt-periodo" aria-label="${esc(txt("tela.aq_log_periodo_al", "período da linha do tempo"))}">
                 <option value="300000">${esc(txt("tela.aq_p_5min", "últimos 5 minutos"))}</option>
                 <option value="3600000" selected>${esc(txt("tela.st_ultima_hora", "última hora"))}</option>
                 <option value="86400000">${esc(txt("tela.st_24h", "últimas 24 horas"))}</option>
               </select>
             </div>
             <ol class="aqt-lista"></ol>
           </section>
           <section class="aqt-graf" aria-label="${esc(txt("tela.aq_graficos_al", "contagens por período"))}">
             <div class="aqt-g" data-periodo="dia"><h4>${esc(txt("tela.aq_g_dia", "Hoje, ao vivo"))}</h4>
               <span class="aqt-sub"></span><svg role="img" aria-label="${esc(txt("tela.aq_g_dia", "Hoje, ao vivo"))}"></svg><ul class="aqt-leg"></ul></div>
             <div class="aqt-g" data-periodo="semana"><h4>${esc(txt("tela.aq_g_semana", "Últimos 7 dias"))}</h4>
               <span class="aqt-sub"></span><svg role="img" aria-label="${esc(txt("tela.aq_g_semana", "Últimos 7 dias"))}"></svg><ul class="aqt-leg"></ul></div>
             <div class="aqt-g" data-periodo="mes"><h4>${esc(txt("tela.aq_g_mes", "Últimos 30 dias"))}</h4>
               <span class="aqt-sub"></span><svg role="img" aria-label="${esc(txt("tela.aq_g_mes", "Últimos 30 dias"))}"></svg><ul class="aqt-leg"></ul></div>
             <p class="aqt-nota">${esc(txt("tela.aq_g_nota", "Erros e avisos contam o desfecho da instrução, em escala própria; o vermelho e o amarelo das bolhas são a gravidade para o servidor."))}</p>
           </section>
         </div>`;

    const $ = s => host.querySelector(s);
    const estado = $(".aqt-estado");
    const selo = $(".aqt-selo");
    const agua = $(".aqt-agua");
    const fundo = $(".aqt-fundo");
    const fisica = criar(agua, { auto: false, aoClicar: id => abrirCartao(id) });

    const ctl = { pedidos: 0, parado: false, vivo: true };
    let relogio = 0, volta = 0, log = [], emVoo = false;
    // O frescor: o instante LOCAL da ultima resposta boa (a idade se mede no
    // relogio de quem olha, e nao no do servidor, que pode estar adiantado),
    // e se o ultimo pedido falhou.
    const nasceu = Date.now();
    let okEm = 0, falhou = false, tique = 0;
    // O ultimo retrato inteiro, e por id as tarefas dele: o cartao le daqui.
    let ultimo = null, porId = new Map();
    const rep = { on: false, ini: 0, fim: 0, t: 0, tocando: false, faixas: [], relogio: 0 };
    const cart = { id: "", alvo: null, feito: false };
    const caixaCartao = $(".aqt-cartao");

    async function pedir(op, p) {
      ctl.pedidos++;
      return api(op, p || {});
    }

    /* ----------------------------------------------------------- o selo */

    function pintarSelo() {
      if (rep.on) {
        host.classList.remove("aqt-velho");
        selo.dataset.estado = "reprise";
        selo.textContent = preencher(txt("tela.aq_selo_reprise", "REPRISE · {hora}"), { hora: hora(rep.t) });
        return;
      }
      const idade = Date.now() - (okEm || nasceu);
      const velho = falhou || idade > LIMITE_VELHO_MS;
      host.classList.toggle("aqt-velho", velho);
      selo.dataset.estado = velho ? "velho" : okEm ? "fresco" : "espera";
      const s = Math.floor(idade / 1000);
      selo.textContent = !okEm
        ? (velho ? txt("tela.aq_selo_nunca", "VELHO · nenhum retrato recebido")
                 : txt("tela.aq_selo_espera", "aguardando o primeiro retrato"))
        : velho ? preencher(txt("tela.aq_selo_velho", "VELHO · último retrato há {s} s"), { s })
        : preencher(txt("tela.aq_selo_fresco", "atualizado há {s} s"), { s });
    }
    function ligarTique() { if (!tique) tique = setInterval(pintarSelo, 250); pintarSelo(); }
    function desligarTique() { clearInterval(tique); tique = 0; }

    /* ------------------------------------------------------------- fundo */

    function pintarFundo(sedimento) {
      fundo.textContent = "";
      if (!sedimento || !sedimento.length) { fundo.hidden = true; return; }
      fundo.hidden = false;
      const cab = document.createElement("div");
      cab.className = "aqt-cab"; cab.textContent = txt("tela.aq_fundo", "Fundo do aquário");
      fundo.append(cab);
      for (const p of sedimento) {
        const d = document.createElement("div");
        d.className = "aqt-pedra"; d.setAttribute("data-alarme", p.alarme || "");
        const t = document.createElement("span");
        t.textContent = motivo(p.motivo) + (p.vezes > 1 ? " ×" + p.vezes : "");
        d.append(chip(p.cor), t);
        fundo.append(d);
      }
    }

    /* Quantos aneis ate a preta: o tamanho da tabela do servidor. */
    function totalDeAneis(r) {
      const lim = r && r.limiares && r.limiares.aneis_ms;
      return Array.isArray(lim) ? lim.length : 0;
    }

    /* Pode encerrar? A pergunta e do SERVIDOR (`completo`, o portao da
     * telemetria), e a TV nunca abre o cartao, nem com login de quem pode. */
    const podeEncerrar = () => !!(caixaCartao && !c.tv && ultimo && ultimo.completo === true);

    function pintarRetrato(r) {
      const voce = r.voce || "";
      ultimo = r;
      porId = new Map((r.tarefas || []).filter(t => t.tarefa !== voce).map(t => [t.tarefa, t]));
      fisica.clicavel(podeEncerrar() && !rep.on);
      if (rep.on) return;     // a reprise congela o tanque; o retrato so guarda
      // Os campos que a bolha desenha, e nenhum outro: `usuario` e `ip`, que
      // o servidor manda ao administrador, ficam no retrato e so o cartao os
      // le.
      const aneis = totalDeAneis(r);
      const tarefas = [...porId.values()]
        .map(t => ({ id: t.tarefa, op: t.op, tabela: t.tabela, cor: t.cor,
                     faixa: t.faixa, motivo: t.motivo, ms: t.ms, pseudonimo: t.pseudonimo,
                     anel: t.anel || 0, aneis: aneis }));
      fisica.atualizar(tarefas);
      pintarFundo(r.sedimento);
      estado.classList.remove("aqt-mal");
      estado.textContent = r.ligada === false
        ? txt("tela.aq_desligada", "a telemetria está desligada: o aquário não recebe tarefas nem conta")
        : preencher(txt("tela.aq_estado", "{n} tarefas vivas · atualizado às {hora}"),
            { n: tarefas.length, hora: hora(r.agora_ms || Date.now()) });
      if (cart.id) pintarCartao();
    }

    /* ------------------------------------------------- o cartao (A12) */

    function abrirCartao(id) {
      if (!podeEncerrar() || rep.on) return;
      const t = porId.get(id);
      if (!t) return;
      // O alvo CONGELA no clique: o cartao descreve a tarefa que a pessoa
      // viu, e o id leva o serial -- o servidor recusa se ela ja terminou, em
      // vez de matar o pedido seguinte da mesma conexao.
      cart.id = id; cart.alvo = t; cart.feito = false;
      fisica.selecionar(id);
      const res = $(".aqt-res");
      res.textContent = ""; res.classList.remove("aqt-mal"); res.removeAttribute("data-desfecho");
      $(".aqt-k-encerrar").disabled = false; $(".aqt-k-derrubar").disabled = false;
      pintarCartao();
      caixaCartao.hidden = false;
      $(".aqt-k-fechar").focus();
    }
    function fecharCartao() {
      if (!caixaCartao) return;
      cart.id = ""; cart.alvo = null;
      fisica.selecionar("");
      caixaCartao.hidden = true;
    }

    /* A conexao que o «derrubar» fecha: `dados:N` e a conexao N da porta de
     * dados; `web:abcd1234`, a sessao do navegador. As outras origens (job,
     * servico) nao tem conexao para derrubar. */
    function conexaoDe(t) {
      const m = /^(dados|web):([^#]+)#/.exec(String(t.tarefa || ""));
      if (!m) return null;
      return m[1] === "dados" ? { id: +m[2], tipo: "conexao" } : { id: m[2], tipo: "web" };
    }

    function pintarCartao() {
      const vivo = porId.get(cart.id);
      const t = vivo || cart.alvo;
      if (!t) return;
      if (vivo) cart.alvo = vivo;
      const ficha = $(".aqt-ficha");
      ficha.textContent = "";
      const linha = (rot, valor) => {
        if (valor == null || valor === "") return;
        const a = document.createElement("span"); a.className = "aqt-r"; a.textContent = rot;
        const b = document.createElement("span"); b.className = "aqt-v"; b.textContent = String(valor);
        ficha.append(a, b);
      };
      const cx = conexaoDe(t);
      linha(txt("tela.aq_k_tarefa", "tarefa"), t.tarefa);
      linha(txt("tela.tl_c_operacao", "operação em curso"), t.op);
      linha(txt("tela.tl_c_alvo", "alvo"), [t.database, t.tabela].filter(Boolean).join("."));
      linha(txt("tela.tl_c_usuario", "usuário"), t.usuario);
      linha(txt("tela.tl_c_estacao", "estação (IP)"), t.ip);
      linha(txt("tela.tl_c_origem", "origem"), t.origem);
      linha(txt("tela.tl_c_conexao", "conexão"), t.ligacao != null
        ? preencher(txt("tela.tl_c_numero", "nº {n}"), { n: t.ligacao }) : "");
      linha(txt("tela.tl_c_estado", "estado"), t.estado);
      linha(txt("tela.tl_c_fase", "fase"), t.fase);
      linha(txt("tela.tl_c_op_dura", "operação dura há"), dur(t.ms));
      linha(txt("tela.tl_c_na_fila", "desse tempo, na fila da trava"), dur(t.espera_ms));
      if (t.anel) linha(txt("tela.aq_k_anel", "anéis"), textoDoAnel(t.ms, t.anel, totalDeAneis(ultimo)));
      linha(txt("tela.aq_k_cor", "cor"), (ROTULO_DA_COR[t.cor] ? ROTULO_DA_COR[t.cor]() : t.cor) +
        (t.motivo ? " — " + motivo(t.motivo) : ""));
      // O que a promessa vale: as frases da ficha da telemetria, pela chave.
      $(".aqt-promessa").textContent = !vivo
        ? txt("tela.aq_k_terminou", "esta tarefa já terminou: não há mais o que encerrar nela.")
        : t.cancelavel ? txt("tela.tl_nota_cancelavel", "cancelável agora: a marca é lida entre duas unidades de trabalho, e o que já foi gravado fica gravado.")
        : t.tem_ponto ? txt("tela.tl_nota_tem_ponto", "esta operação tem ponto de cancelamento, mas não está nele neste instante — tipicamente porque espera a trava de dados. A marca vale para o primeiro ponto seguro que vier.")
        : txt("tela.tl_nota_sem_ponto", "não cancelável: a operação não tem ponto de cancelamento e vai terminar. Abandonar uma gravação no meio deixaria o arquivo mentindo.");
      $(".aqt-perde").textContent = txt("tela.aq_k_perde_encerrar",
        "Encerrar a operação aborta a instrução; dentro de uma transação, a transação inteira fica condenada e as escritas dela se perdem no ROLLBACK.") +
        " " + txt("tela.aq_k_perde_derrubar", "Derrubar a conexão fecha o soquete: a transação aberta é desfeita e as travas dela, soltas.");
      $(".aqt-assina").textContent = preencher(txt("tela.aq_k_assina", "assinado por {quem}, no log de acessos"),
        { quem: c.quem || "—" });
      $(".aqt-k-encerrar").hidden = !vivo || cart.feito;
      $(".aqt-k-derrubar").hidden = !cx;
    }

    async function encerrar() {
      const t = cart.alvo, res = $(".aqt-res");
      if (!t) return;
      $(".aqt-k-encerrar").disabled = true;
      res.classList.remove("aqt-mal");
      try {
        const r = await pedir("telemetria_encerrar", { id: t.tarefa });
        cart.feito = true;
        res.setAttribute("data-desfecho", r.estado || "");
        res.textContent = DESFECHO[r.estado] ? DESFECHO[r.estado]() : String(r.estado || "");
        res.classList.toggle("aqt-mal", r.estado === "nao_cancelavel");
      } catch (e) {
        res.setAttribute("data-desfecho", "erro");
        res.classList.add("aqt-mal");
        res.textContent = String(e && e.message || e);
        $(".aqt-k-encerrar").disabled = false;
      }
      pintarCartao();
    }

    async function derrubar() {
      const t = cart.alvo, res = $(".aqt-res");
      const cx = t && conexaoDe(t);
      if (!cx) return;
      $(".aqt-k-derrubar").disabled = true;
      res.classList.remove("aqt-mal");
      try {
        await pedir("encerrar_sessao", cx);
        cart.feito = true;
        res.setAttribute("data-desfecho", "derrubada");
        res.textContent = txt("tela.tl_conexao_encerrada", "conexão encerrada");
      } catch (e) {
        res.setAttribute("data-desfecho", "erro");
        res.classList.add("aqt-mal");
        res.textContent = String(e && e.message || e);
        $(".aqt-k-derrubar").disabled = false;
      }
      pintarCartao();
    }

    if (caixaCartao) {
      $(".aqt-k-encerrar").addEventListener("click", encerrar);
      $(".aqt-k-derrubar").addEventListener("click", derrubar);
      $(".aqt-k-fechar").addEventListener("click", fecharCartao);
      caixaCartao.addEventListener("keydown", e => { if (e.key === "Escape") fecharCartao(); });
    }

    /* ------------------------------------------------------------- o log */

    function pintarLog() {
      const ol = $(".aqt-lista");
      if (!ol) return;
      const q = ($(".aqt-busca").value || "").trim().toLowerCase();
      ol.textContent = "";
      let n = 0;
      for (let i = log.length - 1; i >= 0 && n < 300; i--) {
        const l = log[i];
        const ev = EVENTOS[l.evento];
        if (!ev) continue;   // contagem e retrato alimentam os graficos e a volta
        const d = l.dados || {};
        const textoEv = ev();
        const textoMot = [l.evento === "anel" && d.anel ? textoDoAnel(l.ms, d.anel, d.de) : "",
                          l.motivo ? motivo(l.motivo) : ""].filter(Boolean).join(" — ");
        const textoCor = ROTULO_DA_COR[l.cor] ? ROTULO_DA_COR[l.cor]() : "";
        const alvo = [l.database, l.tabela].filter(Boolean).join(".");
        if (q) {
          const tudo = [textoEv, l.op, alvo, textoCor, textoMot, l.alarme, l.cor]
            .filter(Boolean).join(" ").toLowerCase();
          if (tudo.indexOf(q) < 0) continue;
        }
        const li = document.createElement("li");
        li.setAttribute("data-evento", l.evento);
        const t = document.createElement("time");
        t.dateTime = new Date(l.quando_ms).toISOString(); t.textContent = hora(l.quando_ms);
        const o = document.createElement("span"); o.className = "aqt-o";
        const b = document.createElement("b"); b.textContent = textoEv;
        o.append(b, document.createTextNode(" " + [l.op, alvo].filter(Boolean).join(" · ") +
          (l.ms != null ? " · " + l.ms + " ms" : "")));
        li.append(t, l.cor ? chip(l.cor) : document.createElement("span"), o);
        if (textoMot || textoCor) {
          const m = document.createElement("span"); m.className = "aqt-mot";
          m.textContent = [textoCor, textoMot].filter(Boolean).join(" — ");
          li.append(m);
        }
        ol.append(li); n++;
      }
      if (!n) {
        const li = document.createElement("li"); li.className = "aqt-nada";
        li.textContent = txt("tela.aq_log_vazio", "nenhum evento neste período");
        ol.append(li);
      }
    }

    async function lerLog() {
      const sel = $(".aqt-periodo");
      const janela = +(sel && sel.value) || 3600000;
      try {
        const r = await pedir("aquario_log", { desde: Date.now() - janela, max: 1000 });
        log = r.linhas || [];
      } catch (e) {
        log = [];
        const ol = $(".aqt-lista"); ol.textContent = "";
        const li = document.createElement("li"); li.className = "aqt-nada";
        li.textContent = String(e && e.message || e); ol.append(li);
        return;
      }
      pintarLog();
    }

    async function lerContagens() {
      const agora = Date.now();
      const inicios = { dia: meiaNoite(agora, 0), semana: meiaNoite(agora, 6), mes: meiaNoite(agora, 29) };
      const r = await pedir("aquario_contagens", { desde: inicios.mes });
      for (const k of Object.keys(inicios)) {
        const caixa = host.querySelector(`.aqt-g[data-periodo="${k}"]`);
        if (caixa) desenharGrafico(caixa, somar(r, inicios[k]), inicios[k], agora);
      }
    }

    /* --------------------------------------------- a volta de 5 min (A13) */

    /* O tanque no instante `t` (relogio do SERVIDOR, o mesmo do log): quem
     * estourou depois de `t` e tinha nascido antes, mais as vivas do ultimo
     * retrato que ja tinham nascido. A cor e a do FIM -- o log grava a cor com
     * que a tarefa terminou, e as linhas `nasceu`/`retrato` ainda nao existem
     * para dizer a do meio. */
    function tanqueEm(t) {
      // Sem retrato na reprise: o anel sai da TABELA que o ultimo retrato
      // publicou (`limiares.aneis_ms`), contando os limites ja passados.
      const lim = (ultimo && ultimo.limiares && ultimo.limiares.aneis_ms) || [];
      return rep.faixas.filter(x => x.ini <= t && t < x.fim)
        .map(x => ({ id: x.id, op: x.op, tabela: x.tabela, cor: x.cor, faixa: x.faixa,
                     motivo: x.motivo, ms: t - x.ini,
                     anel: lim.filter(l => t - x.ini >= l).length, aneis: lim.length }));
    }
    function pintarReprise() {
      const tarefas = tanqueEm(rep.t);
      fisica.atualizar(tarefas);
      $(".aqt-trilho").value = String(rep.t - rep.ini);
      const q = $(".aqt-quando");
      q.dateTime = new Date(rep.t).toISOString(); q.textContent = hora(rep.t);
      $(".aqt-bt-tocar").textContent = rep.tocando ? txt("tela.aq_pausar", "Pausar") : txt("tela.aq_reproduzir", "Reproduzir");
      estado.classList.remove("aqt-mal");
      estado.textContent = preencher(txt("tela.aq_reprise_estado", "reprise: {n} tarefas no tanque às {hora} — o servidor continua gravando"),
        { n: tarefas.length, hora: hora(rep.t) });
      pintarSelo();
    }
    function andarReprise() {
      rep.relogio = 0;
      if (!rep.on || !rep.tocando) return;
      if (!ctl.parado && !document.hidden) {
        rep.t = Math.min(rep.fim, rep.t + REPRISE_PASSO_MS * REPRISE_VEZES);
        if (rep.t >= rep.fim) rep.tocando = false;
        pintarReprise();
      }
      if (rep.tocando) rep.relogio = setTimeout(andarReprise, REPRISE_PASSO_MS);
    }
    async function voltar5min() {
      if (rep.on || !ultimo) return;
      fecharCartao();
      // Pausar congela a tela: o retrato para de ser pedido, e o tanque passa
      // a ser o do log.
      clearTimeout(relogio); relogio = 0;
      rep.on = true; rep.tocando = false;
      host.classList.add("aqt-reprise");
      fisica.clicavel(false);
      rep.fim = ultimo.agora_ms || Date.now();
      rep.ini = rep.fim - VOLTA_MS;
      rep.t = rep.ini;
      let linhas = [];
      try {
        const r = await pedir("aquario_log", { desde: rep.ini, ate: rep.fim, max: 5000 });
        linhas = r.linhas || [];
      } catch (e) {
        estado.classList.add("aqt-mal");
        estado.textContent = preencher(txt("tela.aq_falhou", "o servidor não respondeu: {erro}"),
          { erro: String(e && e.message || e) });
      }
      const voce = ultimo.voce || "";
      rep.faixas = linhas
        .filter(l => l.evento === "estourou" && l.ms != null)
        .map((l, i) => ({ id: "volta:" + l.quando_ms + ":" + i, op: l.op, tabela: l.tabela, cor: l.cor,
                          faixa: l.faixa, motivo: l.motivo, ini: l.quando_ms - l.ms, fim: l.quando_ms }))
        .concat((ultimo.tarefas || []).filter(t => t.tarefa !== voce).map(t => ({
          id: t.tarefa, op: t.op, tabela: t.tabela, cor: t.cor, faixa: t.faixa, motivo: t.motivo,
          ini: rep.fim - (t.ms || 0), fim: Infinity })));
      $(".aqt-reprise").hidden = false;
      $(".aqt-bt-volta").hidden = true;
      rep.tocando = true;
      pintarReprise();
      clearTimeout(rep.relogio); rep.relogio = setTimeout(andarReprise, REPRISE_PASSO_MS);
    }
    function aoVivo() {
      if (!rep.on) return;
      rep.on = false; rep.tocando = false;
      clearTimeout(rep.relogio); rep.relogio = 0;
      host.classList.remove("aqt-reprise");
      $(".aqt-reprise").hidden = true;
      $(".aqt-bt-volta").hidden = false;
      // o que o retrato guardou durante a reprise volta ao tanque ja
      if (ultimo) pintarRetrato(ultimo);
      pintarSelo();
      if (ativo() && !relogio) relogio = setTimeout(girar, 0);
    }
    if (completa) {
      $(".aqt-bt-volta").addEventListener("click", voltar5min);
      $(".aqt-bt-vivo").addEventListener("click", aoVivo);
      $(".aqt-bt-tocar").addEventListener("click", () => {
        if (!rep.on) return;
        if (!rep.tocando && rep.t >= rep.fim) rep.t = rep.ini;
        rep.tocando = !rep.tocando;
        pintarReprise();
        clearTimeout(rep.relogio);
        rep.relogio = rep.tocando ? setTimeout(andarReprise, REPRISE_PASSO_MS) : 0;
      });
      $(".aqt-trilho").addEventListener("input", e => {
        if (!rep.on) return;
        rep.tocando = false;
        rep.t = rep.ini + (+e.target.value || 0);
        pintarReprise();
      });
    }

    /* ------------------------------------------------------------ a volta */

    /* Uma volta: retrato sempre; contagens a cada duas (o grafico do dia
     * anda pelo minuto parcial, que muda a cada pedido); log a cada tres. */
    async function girar() {
      relogio = 0;
      // Um pedido no ar por vez: a volta que o `retomar` agendar enquanto
      // este espera a resposta morre aqui, e e ESTE que agenda a seguinte --
      // senao esconder e mostrar a aba no meio de um pedido dobraria o laco.
      if (!ativo() || emVoo) return;
      emVoo = true;
      try {
        const r = await pedir("aquario_retrato");
        okEm = Date.now(); falhou = false;
        pintarRetrato(r);
        pintarSelo();
        if (completa) {
          if (volta % 2 === 0) await lerContagens();
          if (volta % 3 === 0) await lerLog();
        }
      } catch (e) {
        // O pedido que falhou declara o retrato VELHO na hora: esperar os
        // 3 s de idade seria mostrar como vivo o que ja se sabe que nao e.
        falhou = true;
        pintarSelo();
        estado.classList.add("aqt-mal");
        estado.textContent = preencher(txt("tela.aq_falhou", "o servidor não respondeu: {erro}"),
          { erro: String(e && e.message || e) });
      } finally {
        emVoo = false;
      }
      volta++;
      if (ativo() && !relogio) relogio = setTimeout(girar, periodo);
    }

    /* Trabalha so quem esta a vista: tela viva, nao pausada pela multitela,
     * com a aba do navegador na frente e fora da reprise. O portao vem ANTES
     * do pedido. */
    function ativo() {
      if (!host.isConnected) { if (ctl.vivo) soltar(); return false; }
      return ctl.vivo && !ctl.parado && !document.hidden && !rep.on;
    }

    function retomar() {
      if (!ctl.vivo) return;
      ctl.parado = false;
      if (document.hidden) return;
      fisica.iniciar();
      ligarTique();
      if (rep.on) {
        if (rep.tocando && !rep.relogio) rep.relogio = setTimeout(andarReprise, REPRISE_PASSO_MS);
        return;
      }
      if (!relogio) relogio = setTimeout(girar, 0);
    }
    function parar() {
      ctl.parado = true;
      clearTimeout(relogio); relogio = 0;
      clearTimeout(rep.relogio); rep.relogio = 0;
      desligarTique();
      fisica.parar();
    }
    function aoMudarVisibilidade() {
      if (document.hidden) {
        clearTimeout(relogio); relogio = 0;
        clearTimeout(rep.relogio); rep.relogio = 0;
        desligarTique();
        fisica.parar();
      } else if (!ctl.parado) retomar();
    }
    function soltar() {
      ctl.vivo = false;
      clearTimeout(relogio); relogio = 0;
      clearTimeout(rep.relogio); rep.relogio = 0;
      desligarTique();
      fisica.soltar();
      document.removeEventListener("visibilitychange", aoMudarVisibilidade);
    }
    document.addEventListener("visibilitychange", aoMudarVisibilidade);

    if (completa) {
      let espera = 0;
      $(".aqt-busca").addEventListener("input", () => {
        clearTimeout(espera); espera = setTimeout(pintarLog, 120);
      });
      $(".aqt-periodo").addEventListener("change", () => { if (ativo()) lerLog(); });
    }

    retomar();
    ctl.parar = parar; ctl.retomar = retomar; ctl.soltar = soltar;
    ctl.fisica = fisica;
    return ctl;
  }

  // A folha entra ja no carregamento: a alca da telemetria e pintada antes
  // de qualquer aquario existir, e sem o estilo o resumo dela nasceria cru.
  if (typeof document !== "undefined" && document.head) garantirCss();

  /* O desfecho pela chave, para a ficha da Telemetria dizer o MESMO que o
   * cartao daqui (pedido 776). Estado desconhecido devolve "" e quem chama
   * decide o que mostrar -- nunca o codigo cru. */
  function desfecho(estado) {
    return DESFECHO[estado] ? DESFECHO[estado]() : "";
  }

  return { criar: criar, criarMotor: criarMotor, sobreposicoes: sobreposicoes, tela: tela,
           desfecho: desfecho,
           FAIXAS: FAIXAS, MOTIVOS: MOTIVOS, LIMITE_VELHO_MS: LIMITE_VELHO_MS,
           _somar: somar, _meiaNoite: meiaNoite };
})();

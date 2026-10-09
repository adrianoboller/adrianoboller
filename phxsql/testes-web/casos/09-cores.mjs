/* As cores da acao, e o contraste — MEDIDOS, nao lidos.
 *
 * A convencao da casa: verde inclui, amarelo altera, rosa marca (o excluir
 * que volta), vermelho exclui de vez, azul consulta. E sempre CONTORNO, nunca
 * fundo cheio — a licao ja estava num comentario do CSS antes de virar regra:
 * fundo laranja com texto escuro em cima ficava ilegivel.
 *
 * O CSS traz numeros de contraste escritos a mao nos comentarios («4,94:1
 * sobre --realce»). Numero digitado a mao envelhece calado: este caso
 * RECALCULA cada um contra o que o navegador realmente pintou, nos dois
 * temas. Se alguem mexer numa cor e esquecer o comentario, aqui quebra.
 *
 * A conta do contraste vai por dentro de cada `evaluate`, e nao por um
 * `eval()` de um texto: a pagina serve `script-src` por hash (771) SEM
 * `unsafe-eval`, e um teste que precisasse afrouxar o CSP para rodar seria
 * pior que teste nenhum. */
import { entrar, cenario, capturar, verdade, bancoDoCaso, abrirLinhaDaGrade } from '../apoio.mjs';

const ACOES = ['incluir', 'alterar', 'marcar', 'excluir', 'consultar'];
const PISO = 4.5;

export const caso = {
  nome: 'cores',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Cor');
    const { tab } = await cenario(page, db);

    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await page.waitForSelector('#btNova');
    await capturar(ctx, ctx.nomeCaptura('grade-com-cores'));

    const problemas = [];

    // ------------------------- as cinco variaveis existem e sao distintas
    const vars = await page.evaluate(acoes => {
      const cs = getComputedStyle(document.documentElement);
      const r = {};
      for (const a of acoes) r[a] = cs.getPropertyValue('--acao-' + a).trim();
      return r;
    }, ACOES);
    for (const a of ACOES) {
      if (!vars[a]) problemas.push(`--acao-${a} nao existe no tema ${ctx.tema}`);
    }
    if (new Set(Object.values(vars)).size !== ACOES.length) {
      problemas.push(`duas acoes com a MESMA cor no tema ${ctx.tema}: ${JSON.stringify(vars)}`);
    }

    // ------------------------- contorno, e nao fundo cheio, em repouso
    const telas = [
      ['grade', () => page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab])],
      ['ficha', async () => {
        await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
        await abrirLinhaDaGrade(page);
        await page.waitForSelector('#fichaEdit');
      }],
      // O dialogo de excluir e a unica tela com o «marcar»: e nele que a cor
      // troca junto com o texto, rosa para a exclusao que volta e vermelho
      // para a que nao volta.
      ['dialogo de excluir', async () => {
        await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
        await abrirLinhaDaGrade(page);
        await page.waitForSelector('#btExcluir');
        await page.click('#btExcluir');
        await page.waitForSelector('.sobre .caixa');
      }],
      ['consulta', () => page.evaluate(() => abrirConsulta())],
      ['lixeira', () => page.evaluate(([d, t]) => telaLixeira(d, t), [db, tab])],
      ['jobs', () => page.evaluate(() => telaJobs())],
      ['servico', () => page.evaluate(() => verServico())],
    ];

    const vistas = new Set();
    for (const [nome, abrir] of telas) {
      await abrir();
      // O ponteiro FICA onde o ultimo clique o deixou, e a tela nova pode
      // nascer com um botao debaixo dele -- e `:hover` PREENCHE o botao, que
      // e justamente o que este caso proibe em repouso. Sem tirar o mouse do
      // caminho, a bateria acusaria «fundo cheio» num botao que so estava
      // sendo apontado. Custou uma falsa acusacao ao botao «Atualizar» da
      // tela de Servico para esta linha existir.
      await page.mouse.move(4, 4);
      await page.waitForTimeout(450);
      const achados = await page.evaluate(([acoes, onde, piso]) => {
        const rgb = s => {
          const m = String(s).match(/[\d.]+/g) || [];
          return { r: +m[0] || 0, g: +m[1] || 0, b: +m[2] || 0, a: m.length > 3 ? +m[3] : 1 };
        };
        const lum = c => {
          const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
          return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
        };
        const contraste = (frente, fundo) => {
          const a = lum(rgb(frente)), b = lum(rgb(fundo));
          return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        };
        // O fundo EFETIVO: sobe pelos pais ate achar quem pinta. Medir contra
        // «transparent» daria um numero bonito e falso.
        const fundoDe = el => {
          for (let n = el; n; n = n.parentElement) {
            if (rgb(getComputedStyle(n).backgroundColor).a > 0.9) {
              return getComputedStyle(n).backgroundColor;
            }
          }
          return getComputedStyle(document.body).backgroundColor;
        };

        const saida = [];
        for (const a of acoes) {
          for (const el of document.querySelectorAll('.botao.' + a)) {
            if (el.getBoundingClientRect().width === 0) continue;
            const s = getComputedStyle(el);
            const rot = el.textContent.trim().slice(0, 28);
            if (rgb(s.backgroundColor).a > 0.05) {
              saida.push({ acao: a, mal: `${onde}: «${rot}» com FUNDO CHEIO `
                + `(${s.backgroundColor}) em repouso` });
              continue;
            }
            if (s.borderTopStyle === 'none' || parseFloat(s.borderTopWidth) < 0.5) {
              saida.push({ acao: a, mal: `${onde}: «${rot}» sem contorno` });
            }
            const c = contraste(s.color, fundoDe(el));
            if (c < piso) {
              saida.push({ acao: a, mal: `${onde}: «${rot}» da ${c.toFixed(2)}:1 — abaixo de ${piso}:1` });
            }
            saida.push({ acao: a, ok: c.toFixed(2) });
          }
        }
        return saida;
      }, [ACOES, nome, PISO]);

      for (const a of achados) {
        if (a.mal) problemas.push(a.mal); else vistas.add(a.acao);
      }
      if (nome === 'ficha') await capturar(ctx, ctx.nomeCaptura('ficha-com-cores'));
    }

    for (const a of ACOES) {
      if (!vistas.has(a)) ctx.notas.push(`nenhum botao «${a}» apareceu nas telas visitadas`);
    }

    // --------------------- o contraste do texto comum, nos dois temas
    const texto = await page.evaluate(() => {
      const rgb = s => {
        const m = String(s).match(/[\d.]+/g) || [];
        return { r: +m[0] || 0, g: +m[1] || 0, b: +m[2] || 0 };
      };
      const lum = c => {
        const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
        return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
      };
      const cs = getComputedStyle(document.documentElement);
      const como = c => {
        const d = document.createElement('div');
        d.style.color = c; document.body.appendChild(d);
        const r = getComputedStyle(d).color; d.remove(); return r;
      };
      const par = (frente, fundo) => {
        const a = lum(rgb(como(cs.getPropertyValue(frente).trim())));
        const b = lum(rgb(como(cs.getPropertyValue(fundo).trim())));
        return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      };
      return {
        'texto/painel': par('--texto', '--painel'),
        'texto-2/painel': par('--texto-2', '--painel'),
        'texto-3/painel-2': par('--texto-3', '--painel-2'),
        'texto-3/realce': par('--texto-3', '--realce'),
      };
    });

    for (const [par, c] of Object.entries(texto)) {
      ctx.notas.push(`contraste ${par} = ${c.toFixed(2)}:1`);
      if (c < PISO) problemas.push(`${par} da ${c.toFixed(2)}:1 no tema ${ctx.tema}`);
    }

    // ------------------- a varredura: TODO elemento pintado, em toda tela
    //
    // Os pares de token acima cobrem o texto comum, e os botoes de acao
    // cobrem o contorno. Falta o que e a armadilha historica desta casa: o
    // elemento de FUNDO CHEIO com texto em cima, que nasce um de cada vez e
    // nunca aparece numa lista de tokens.
    //
    // Achou o chip «ativas» da grade: no tema claro o `--laranja` escurece
    // para #c63c0a, e a tinta quase preta que ele trazia fixa dava 3,85:1.
    // Era o unico lugar que nao usava `--tinta-botao`, e nenhuma leitura de
    // codigo diria isso -- so a conta contra o que o navegador pintou.
    const telasVarridas = [
      ['grade', () => page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab])],
      ['painel', () => page.evaluate(() => irPara('painel'))],
      ['nova tabela', () => page.evaluate(d => telaNovaTabela(d), db)],
      ['config do servidor', () => page.evaluate(() => verConfigServidor())],
      ['usuarios', () => page.evaluate(() => irPara('usuarios'))],
      ['diretivas', () => page.evaluate(() => verDiretivas())],
      ['sobre', () => page.evaluate(() => verSobre())],
    ];
    let pintados = 0;
    for (const [nome, abrir] of telasVarridas) {
      await abrir();
      await page.mouse.move(4, 4);
      await page.waitForTimeout(380);
      const achados = await page.evaluate(onde => {
        const rgb = t => {
          const m = String(t).match(/[\d.]+/g) || [];
          return { r: +m[0] || 0, g: +m[1] || 0, b: +m[2] || 0, a: m.length > 3 ? +m[3] : 1 };
        };
        const lum = c => {
          const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
          return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
        };
        const ct = (a, b) => {
          const x = lum(rgb(a)), y = lum(rgb(b));
          return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
        };
        const saida = [];
        let vistos = 0;
        for (const el of document.querySelectorAll('*')) {
          const s = getComputedStyle(el);
          if (rgb(s.backgroundColor).a < 0.9) continue;
          const r = el.getBoundingClientRect();
          if (r.width < 4 || r.height < 4) continue;
          // So o texto DIRETO: senao o pai leva a culpa da cor do filho.
          const txt = [...el.childNodes].filter(n => n.nodeType === 3)
            .map(n => n.nodeValue.trim()).join(' ').trim();
          if (txt.length < 2) continue;
          vistos++;
          const tam = parseFloat(s.fontSize);
          const peso = parseInt(s.fontWeight, 10) || 400;
          // O piso da WCAG cai para 3:1 em texto grande, e ignorar isso
          // acusaria todo titulo. 18,66px em negrito e 24px sao os limiares.
          const piso = (tam >= 24 || (tam >= 18.66 && peso >= 700)) ? 3.0 : 4.5;
          const c = ct(s.color, s.backgroundColor);
          if (c < piso) {
            saida.push(`${onde}: «${txt.slice(0, 26)}» (${el.className || el.tagName}) `
              + `da ${c.toFixed(2)}:1, piso ${piso}:1 — ${s.color} sobre ${s.backgroundColor}`);
          }
        }
        return { saida, vistos };
      }, nome);
      pintados += achados.vistos;
      problemas.push(...achados.saida);
    }
    ctx.notas.push(`${pintados} elementos pintados medidos em ${telasVarridas.length} telas`);

    // ------- o texto APAGADO por `opacity`, e o verbo de acao em fundo cheio
    //
    // A varredura de cima so mede quem pinta o PROPRIO fundo, e por isso nunca
    // viu a outra armadilha: `--texto-3` ja foi clareado ate o minimo que passa
    // 4,5:1, e um `opacity:.75` por cima o derruba para 3,36:1 no tema claro.
    // Eram oito regras assim (a assinatura da marca na entrada, a 2,72:1),
    // achadas pela revisao medida de 01/10/2026. Aqui a cor do texto se compoe
    // com a opacidade de toda a linhagem e com o fundo EFETIVO.
    //
    // E na mesma passada: botao `.botao` laranja CHEIO com verbo de acao no
    // rotulo («Nova tabela», «Criar tabela», «Nova ligação», «Juntar»...) --
    // a cor da acao existe e ele a pulou.
    const telasApagadas = [
      ['gerir tabelas', () => page.evaluate(d => gerirTabelas(d), db)],
      ['nova tabela', () => page.evaluate(d => telaNovaTabela(d), db)],
      ['config do servidor', () => page.evaluate(() => verConfigServidor())],
      ['config da tabela', () => page.evaluate(([d, t]) => verConfigTabela(d, t), [db, tab])],
      ['telemetria', () => page.evaluate(() => telaTelemetria())],
      ['dblink', () => page.evaluate(() => telaDbLinkDefinicoes())],
      ['juncao', () => page.evaluate(d => telaJuncao(d), db)],
      ['uniao', () => page.evaluate(d => telaUniao(d), db)],
    ];
    for (const [nome, abrir] of telasApagadas) {
      await abrir();
      await page.mouse.move(4, 4);
      await page.waitForTimeout(700);
      problemas.push(...await page.evaluate(onde => {
        const rgb = t => {
          const m = String(t).match(/[\d.]+/g) || [];
          return { r: +m[0] || 0, g: +m[1] || 0, b: +m[2] || 0, a: m.length > 3 ? +m[3] : 1 };
        };
        const lum = c => {
          const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
          return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
        };
        const sobre = (c, f) => ({ r: c.r * c.a + f.r * (1 - c.a), g: c.g * c.a + f.g * (1 - c.a), b: c.b * c.a + f.b * (1 - c.a), a: 1 });
        const fundo = el => {
          const cam = [];
          for (let n = el; n; n = n.parentElement) {
            const c = rgb(getComputedStyle(n).backgroundColor);
            if (c.a > 0) cam.push(c);
            if (c.a >= 0.99) break;
          }
          let f = cam.length && cam[cam.length - 1].a >= 0.99 ? cam.pop() : { r: 255, g: 255, b: 255, a: 1 };
          while (cam.length) f = sobre(cam.pop(), f);
          return f;
        };
        const saida = [];
        for (const el of document.querySelectorAll('#painel *, #app .corpo *')) {
          const r = el.getBoundingClientRect();
          if (r.width < 1 || r.height < 1) continue;
          if (el.closest('button:disabled, input:disabled, select:disabled, [aria-disabled="true"]')) continue;
          const txt = [...el.childNodes].filter(n => n.nodeType === 3).map(n => n.nodeValue.trim()).join(' ').trim();
          if (txt.length < 2) continue;
          let op = 1;
          for (let n = el; n; n = n.parentElement) op *= parseFloat(getComputedStyle(n).opacity);
          if (op >= 0.99) continue;
          const s = getComputedStyle(el);
          const f = fundo(el), c = rgb(s.color); c.a *= op;
          const a = lum(sobre(c, f)), b = lum(f);
          const ct = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
          const tam = parseFloat(s.fontSize), peso = parseInt(s.fontWeight, 10) || 400;
          const piso = (tam >= 24 || (tam >= 18.66 && peso >= 700)) ? 3.0 : 4.5;
          if (ct < piso) saida.push(`${onde}: «${txt.slice(0, 30)}» apagado por opacity ${op.toFixed(2)} da ${ct.toFixed(2)}:1`);
        }
        const VERBO = /^\W*(nov[oa]|criar|incluir|colar|gravar|salvar|aplicar|alterar|excluir|apagar|juntar|unir|montar)\b/i;
        for (const el of document.querySelectorAll('#painel .botao, #app .corpo .botao')) {
          if (!el.getBoundingClientRect().width) continue;
          if (/\b(incluir|alterar|marcar|excluir|consultar|secundario|perigo)\b/.test(el.className)) continue;
          if (rgb(getComputedStyle(el).backgroundColor).a < 0.5) continue;
          const rot = el.textContent.trim();
          if (VERBO.test(rot)) saida.push(`${onde}: «${rot.slice(0, 30)}» (#${el.id}) e verbo de acao em fundo laranja cheio, sem a cor da acao`);
        }
        return saida;
      }, nome));
    }

    verdade(problemas.length === 0,
      `cores da acao / contraste:\n      ${[...new Set(problemas)].join('\n      ')}`);
  },
};

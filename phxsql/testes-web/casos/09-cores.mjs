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
 * `eval()` de um texto: a pagina serve `script-src 'unsafe-inline'` SEM
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

    verdade(problemas.length === 0,
      `cores da acao / contraste:\n      ${[...new Set(problemas)].join('\n      ')}`);
  },
};

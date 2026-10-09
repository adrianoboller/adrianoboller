/* Prova pelo navegador do pedido 780 -- a bolha que passa do teto do raio
 * ganha ANEIS ate ficar PRETA (papel E, 09/10/2026). Roda nos DOIS temas.
 *
 *   node testes-web/prova-780-aneis.mjs [caminho/do/phxsqld]   (da pasta phxsql/)
 *
 * A tarefa longa e SIMULADA: esperar 16 minutos por uma bolha preta nao e
 * prova que alguem rode. O retrato que a tela recebe e o do servidor REAL
 * (`route.fetch`), e nele entra uma tarefa a mais cujo `ms` o teste anda de
 * 20 s a 16,7 min. O anel dela sai da TABELA que o servidor publicou em
 * `limiares.aneis_ms` -- sem a tabela, sem anel, e a prova reprova: e a
 * mesma pergunta que a volta de cinco minutos faz.
 *
 * O que cada passo MEDE:
 *   tabela   o retrato real traz `limiares.aneis_ms` = [60000 .. 960000];
 *   aneis    em cada `ms`, a bolha tem `data-anel` = o anel esperado e o
 *            mesmo numero de `.aq-anel` no DOM (FORMA, nao so cor);
 *   teto     o raio nao cresce depois dos 30 s: o tempo a mais vira anel;
 *   preta    no ultimo anel o miolo e rgb(0,0,0) opaco, com halo visivel;
 *   borda    a cor de gravidade continua no contorno (o stroke da forma);
 *   motivo   «rodando há N min, anel K de M» no <title>, no cartao do
 *            administrador, na dica do toque de quem so monitora e na linha
 *            `anel` do log;
 *   contraste borda e halo >= 3:1 sobre o fundo do tanque; o traco claro do
 *            anel >= 4,5:1 sobre o preto; o escuro >= 3:1 sobre o fundo;
 *   console  nenhum erro.
 *
 * VIDEO=caminho.webm grava a passagem do teto ate a preta (tema escuro), e
 * CAPTURAS=pasta guarda as duas capturas. Sai com codigo 1 se algo reprovar. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { copyFileSync, mkdirSync, mkdtempSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { subir } from './servidor.mjs';
import { entrar } from './apoio.mjs';

const PHXSQLD = resolve(process.argv[2] || 'target/release/phxsqld');
const SAIDA = resolve(process.env.CAPTURAS || 'target');
mkdirSync(SAIDA, { recursive: true });
const VIDEO = process.env.VIDEO ? resolve(process.env.VIDEO) : '';

const reprovas = [];
const nota = (ok, oQue, valor) => {
  console.log(`${ok ? 'ok ' : 'XX '} ${oQue}: ${valor}`);
  if (!ok) reprovas.push(oQue);
};
const dormir = ms => new Promise(r => setTimeout(r, ms));

const ID = 'dados:780#1';
const TAB = 'Vendas_Blumenau';
// ms simulado -> anel esperado pela escala do aquario/anel.rs (dobra alem dos 30 s)
const PASSOS = [[20000, 0], [45000, 0], [70000, 1], [130000, 2], [250000, 3], [500000, 4], [1000000, 5]];

const RAZAO = `(() => {
  const rgba = s => (String(s).match(/[\\d.]+/g) || [0, 0, 0]).map(Number);
  const lum = c => { const f = v => { v /= 255; return v <= .03928 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; };
    return .2126 * f(c[0]) + .7152 * f(c[1]) + .0722 * f(c[2]); };
  return (a, b) => { const [x, y] = [lum(rgba(a)), lum(rgba(b))].sort((p, q) => q - p); return (x + .05) / (y + .05); };
})()`;

const servidor = await subir({ phxsqld: PHXSQLD, portaDados: 6380, portaWeb: 6381, log: console.log });
const nav = await chromium.launch();
try {
  for (const tema of (process.env.TEMAS || 'escuro,claro').split(',')) {
    console.log(`\n== tema ${tema}`);
    const gravar = VIDEO && tema === 'escuro';
    const dirVideo = gravar ? mkdtempSync(join(tmpdir(), 'aq780-video-')) : '';
    const ctx = await nav.newContext({ viewport: { width: 1280, height: 960 },
      ...(gravar ? { recordVideo: { dir: dirVideo, size: { width: 1280, height: 960 } } } : {}) });
    await ctx.route(u => !u.href.startsWith(servidor.url), r => r.abort());
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
    page.on('pageerror', e => erros.push(String(e)));

    // A tarefa simulada, entregue DENTRO do retrato real.
    const sim = { ms: 20000, cor: 'azul_escuro', completo: true, limiares: null };
    await page.route(servidor.url + 'api', async route => {
      const corpo = route.request().postData() || '';
      if (!/"op":"aquario_(retrato|log)"/.test(corpo)) return route.continue();
      const resp = await route.fetch();
      const j = await resp.json();
      if (j.ok && corpo.includes('"aquario_retrato"')) {
        const r = j.resultado;
        sim.limiares = r.limiares;
        const lim = (r.limiares && r.limiares.aneis_ms) || [];
        const anel = lim.filter(l => sim.ms >= l).length;
        r.tarefas = (r.tarefas || []).concat([{ tarefa: ID, origem: 'dados', estado: 'executando', op: 'checksum',
          database: 'aq780', tabela: TAB, faixa: 'outras', ms: sim.ms, servico_ms: sim.ms, espera_ms: 0,
          cor: sim.cor, tamanho: 'grande', motivo: 'aquario.motivo.fora_do_habitual',
          ...(anel ? { anel } : {}), usuario: 'adm', ip: '127.0.0.1', cancelavel: false, tem_ponto: true }]);
        r.completo = sim.completo;
      }
      if (j.ok && corpo.includes('"aquario_log"')) {
        // a linha `anel` como o servidor a grava (o teste de servidor prova
        // que ela sai; aqui se prova que a tela a escreve)
        const agora = Date.now();
        j.resultado.linhas = (j.resultado.linhas || []).concat([{ quando_ms: agora - 1000, evento: 'anel', op: 'checksum',
          database: 'aq780', tabela: TAB, ms: 960000, cor: 'azul_escuro', motivo: 'aquario.motivo.fora_do_habitual',
          dados: { anel: 5, de: 5 } }]);
      }
      return route.fulfill({ response: resp, json: j });
    });

    await entrar(page, servidor.url + '?tela=aquario');
    await page.evaluate(() => api('telemetria_ligar').catch(() => null));
    await page.waitForSelector(`#aqTela .aq-b[data-id="${ID}"]`, { timeout: 20000 });
    // o tanque inteiro a vista: a bolha nada na faixa de baixo, e clique fora
    // da janela nao chega
    await page.evaluate(() => document.querySelector('#aqTela .aqt-tanque').scrollIntoView({ block: 'end' }));

    const tab = sim.limiares && sim.limiares.aneis_ms;
    nota(JSON.stringify(tab) === '[60000,120000,240000,480000,960000]', 'o retrato publica a tabela dos aneis',
      JSON.stringify(tab || null));

    const medir = () => page.evaluate(id => {
      const g = document.querySelector(`#aqTela .aq-b[data-id="${id}"]`);
      if (!g) return null;
      const f = g.querySelector('.aq-forma');
      return { anel: g.getAttribute('data-anel'), aneis: g.getAttribute('data-aneis'),
        nos: g.querySelectorAll('.aq-anel').length, preta: g.classList.contains('aq-preta'),
        r: +(f.getAttribute('r') || 0), fill: getComputedStyle(f).fill, fop: getComputedStyle(f).fillOpacity,
        halo: g.querySelector('.aq-halo') ? getComputedStyle(g.querySelector('.aq-halo')).display : 'ausente', tit: g.querySelector('title').textContent };
    }, ID);

    let rTeto = 0;
    for (const [ms, k] of PASSOS) {
      sim.ms = ms;
      // duas voltas do retrato (2 s cada) e o raio assenta
      let m = null;
      for (let i = 0; i < 30; i++) {
        await dormir(250);
        m = await medir();
        if (m && +(m.anel || 0) === k && (k === 0 || m.tit.includes(`${k} de 5`))) break;
      }
      await dormir(gravar ? 900 : 300);
      m = await medir();
      const forma = m && +(m.anel || 0) === k && m.nos === k && (k === 0 || m.aneis === '5');
      nota(forma, `${Math.round(ms / 1000)} s -> anel ${k} (forma: ${k} aneis no DOM)`,
        m ? `data-anel=${m.anel} de ${m.aneis}, ${m.nos} nos, r=${m.r}` : 'sem bolha');
      if (ms === 70000) rTeto = m.r;
      if (k > 0) nota(m.tit.includes(`anel ${k} de 5`) && m.tit.includes(`rodando há ${Math.floor(ms / 60000)} min`),
        `motivo no <title> (${k})`, m.tit.slice(-60));
      if (k > 0 && k < 5) nota(!m.preta && m.halo === 'none', `anel ${k}: ainda nao preta`, `${m.preta} halo=${m.halo}`);
      if (k === 5) {
        nota(m.preta && m.fill === 'rgb(0, 0, 0)' && +m.fop === 1 && m.halo !== 'none',
          'anel 5: a bolha inteira preta, com halo', `fill=${m.fill} opacidade=${m.fop} halo=${m.halo}`);
        nota(rTeto > 0 && Math.abs(m.r - rTeto) < 1.5, 'o raio parou no teto: o tempo a mais virou anel',
          `r aos 70 s=${rTeto}, aos 16,7 min=${m.r}`);
      }
    }

    // -- contraste, com a bolha preta
    const cc = await page.evaluate(([id, src]) => {
      const razao = eval(src);
      const g = document.querySelector(`#aqTela .aq-b[data-id="${id}"]`);
      const fundo = getComputedStyle(document.querySelector('#aqTela .aq')).backgroundColor;
      const raiz = getComputedStyle(document.documentElement);
      const ver = document.createElement('span'); document.body.append(ver);
      ver.style.color = raiz.getPropertyValue('--aq-azul-escuro').trim() || '#9cc3ff';
      const esperado = getComputedStyle(ver).color; ver.remove();
      const tinta = (sel, k) => { const e = g.querySelector(sel); return e ? getComputedStyle(e)[k] : 'rgb(1,4,24)'; };
      const borda = tinta('.aq-forma', 'stroke');
      const halo = tinta('.aq-halo', 'stroke');
      const claro = tinta('.aq-anel-c', 'stroke');
      const escuro = tinta('.aq-anel-e', 'stroke');
      const rot = tinta('.aq-rot', 'fill');
      return { fundo, borda, esperado, bordaFundo: razao(borda, fundo), haloFundo: razao(halo, fundo),
        claroPreto: razao(claro, 'rgb(0,0,0)'), rotPreto: razao(rot, 'rgb(0,0,0)'), escuroFundo: razao(escuro, fundo), pretoFundo: razao('rgb(0,0,0)', fundo) };
    }, [ID, RAZAO]);
    nota(cc.borda === cc.esperado, 'a cor de gravidade continua na borda', `${cc.borda} (azul escuro ${cc.esperado})`);
    nota(cc.bordaFundo >= 3, 'contraste borda x fundo', cc.bordaFundo.toFixed(2) + ':1');
    nota(cc.haloFundo >= 3, 'contraste halo x fundo', cc.haloFundo.toFixed(2) + ':1');
    nota(cc.rotPreto >= 4.5, 'contraste nome da tabela x miolo preto', cc.rotPreto.toFixed(2) + ':1');
    nota(cc.claroPreto >= 4.5, 'contraste traco claro do anel x preto', cc.claroPreto.toFixed(2) + ':1');
    nota(cc.escuroFundo >= 3 || cc.claroPreto >= 4.5, 'traco do anel visivel sobre o fundo (escuro ou claro)',
      `escuro x fundo ${cc.escuroFundo.toFixed(2)}:1`);
    console.log(`   · preto x fundo ${cc.pretoFundo.toFixed(2)}:1 (fundo ${cc.fundo}) -- por isso o halo`);
    await page.screenshot({ path: join(SAIDA, `aquario-780-${tema}.png`) });

    // -- o log escreve a linha `anel`
    await page.waitForSelector('#aqTela .aqt-lista li[data-evento="anel"]', { timeout: 10000 }).catch(() => {});
    const linhaLog = await page.evaluate(() => document.querySelector('#aqTela .aqt-lista li[data-evento="anel"]')?.innerText || '');
    nota(/ganhou um anel/.test(linhaLog) && /rodando há 16 min, anel 5 de 5/.test(linhaLog), 'linha `anel` no log da tela',
      linhaLog.replace(/\s+/g, ' ').slice(0, 140));

    // -- o administrador clica: o cartao diz o anel
    const caixa = await page.locator(`#aqTela .aq-b[data-id="${ID}"] .aq-forma`).boundingBox();
    let cartao = '';
    for (let i = 0; i < 6 && !/anel 5 de 5/.test(cartao); i++) {
      const b = await page.locator(`#aqTela .aq-b[data-id="${ID}"] .aq-forma`).boundingBox();
      if (b) await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
      await dormir(300);
      cartao = await page.evaluate(() => { const c = document.querySelector('#aqTela .aqt-cartao');
        return c && !c.hidden ? c.innerText : ''; });
    }
    nota(!!caixa && /anel 5 de 5/.test(cartao), 'clique do administrador: o cartao diz o anel', cartao.replace(/\s+/g, ' ').slice(0, 160));
    await page.click('#aqTela .aqt-k-fechar').catch(() => {});

    // -- quem nao pode encerrar toca e le a dica
    sim.completo = false;
    await page.waitForFunction(id => !document.querySelector(`#aqTela .aq-b[data-id="${id}"]`)?.hasAttribute('tabindex'),
      ID, { timeout: 8000 }).catch(() => {});
    let dica = '';
    for (let i = 0; i < 6 && !dica; i++) {
      const b = await page.locator(`#aqTela .aq-b[data-id="${ID}"] .aq-forma`).boundingBox();
      if (b) await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
      await dormir(300);
      dica = await page.evaluate(() => { const d = document.querySelector('#aqTela .aq-dica');
        return d && !d.hidden ? d.textContent : ''; });
    }
    const cartaoMon = await page.evaluate(() => { const c = document.querySelector('#aqTela .aqt-cartao'); return !!c && !c.hidden; });
    nota(/anel 5 de 5/.test(dica) && !cartaoMon, 'toque de quem so monitora: a dica diz o anel, sem cartao', dica.slice(-70));
    if (gravar) await dormir(1500);

    nota(erros.length === 0, `console sem erro (${tema})`, erros.slice(0, 3).join(' | ') || 'nenhum');
    const video = page.video();
    await ctx.close();
    if (video) {
      const p = await video.path();
      copyFileSync(p, VIDEO);
      console.log(`   · video: ${VIDEO}`);
    }
  }
} catch (e) {
  nota(false, 'excecao', e && e.stack || e);
} finally {
  await nav.close();
  await servidor.derrubar();
}
console.log(`\n${reprovas.length ? 'REPROVOU: ' + reprovas.join('; ') : 'tudo verde'}`);
process.exit(reprovas.length ? 1 : 0);

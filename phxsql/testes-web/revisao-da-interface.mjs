#!/usr/bin/env node
/* A REVISAO MEDIDA da interface: todas as telas, nos dois temas e em tres
 * larguras, com seis reguas por tela. Nao e caso da bateria -- nao reprova
 * nada: ela MEDE e grava um JSON, e quem le o JSON decide a severidade.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/revisao-da-interface.mjs --saida /tmp/rev.json
 *
 * Chaves: --porta <n> (dados; a web e n+1, padrao 6760), --tema, --largura,
 *         --capturas <dir>.
 *
 * O INVENTARIO SAI DO CODIGO, nunca de memoria: os itens de `MENUS` e de
 * `FERRAMENTAS` da propria pagina (a mesma que o `http.rs` embute), unidos
 * pelo texto da funcao `faz` -- «Novo database…» mora em dois menus e e UMA
 * tela --, mais as cinco abas da tabela e a tela de entrada.
 *
 * As seis reguas, por tela:
 *  1. erros: `pageerror`, `#aviso.mal` e `#painel .aviso.mal`;
 *  2. contraste de TODO texto visivel, contra o fundo EFETIVO (as camadas
 *     semitransparentes se compoem de baixo para cima), piso 4,5:1 e 3:1 no
 *     texto grande (WCAG 1.4.3). Controle desabilitado fica de fora -- a
 *     norma o isenta;
 *  3. rolagem lateral da PAGINA (`scrollWidth > clientWidth` do documento);
 *  4. botoes de acao fora da convencao: `.botao.<acao>` com fundo cheio em
 *     repouso ou sem contorno, e `.botao` laranja cheio cujo rotulo e verbo
 *     de acao (incluir/alterar/excluir...);
 *  5. mordidas do CSS global: `text-transform` diferente de `none` sobre um
 *     DADO conhecido do cenario, e caixa de marcar/radio esticada (> 24px);
 *  6. alvo de toque < 32px (so na largura 390).
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { cenario, CREDENCIAL } from './apoio.mjs';

/* O `entrar` do apoio espera `#arvore .no` VISIVEL -- e na largura de
 * celular a arvore e gaveta fechada por desenho. Aqui a espera e pela marca
 * de fim do `abrirApp` (`data-pronto`), que vale em toda largura. */
async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });
  await page.fill('#u', CREDENCIAL.USUARIO);
  await page.fill('#s', CREDENCIAL.SENHA);
  await page.fill('#t', CREDENCIAL.TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { state: 'attached', timeout: 25000 });
}

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');
const arg = (n, p = null) => { const i = process.argv.indexOf(n); return i > 0 ? process.argv[i + 1] : p; };

const PORTA = Number(arg('--porta', '6760'));
const SAIDA = arg('--saida', join(AQUI, 'revisao-da-interface.json'));
const CAPTURAS = arg('--capturas');
const TEMAS = arg('--tema') ? [arg('--tema')] : ['escuro', 'claro'];
const LARGURAS = arg('--largura') ? [Number(arg('--largura'))] : [390, 1280, 1920];

/* Os DADOS do cenario: e sobre eles que `text-transform` vira mentira. Os de
 * mistura de caixa sao os unicos que denunciam a mordida -- «SC» nao prova. */
const DADOS = ['Blumenau', 'Joinville', 'Curitiba', 'Adriano Boller', 'Maria Souza', 'Carlos Lima'];
const DB = 'revQualif';
const IDENT = [DB, 'clientes', 'pedidos', 'cidade', 'limite'];

/* Fora da revisao, com o motivo. */
const FORA = new Map([
  ['tela.sair', 'derruba a sessao'],
  ['tela.mi_tema', 'troca o tema -- o tema e eixo da revisao, nao tela'],
  ['tela.mi_soltar', 'janela flutuante por cima do resto -- caso `multitela` e o dono'],
]);

const MEDIR = (ctxMed) => {
  const { dados, ident, largura } = ctxMed;
  const rgb = t => {
    const m = String(t).match(/[\d.]+/g) || [];
    return { r: +m[0] || 0, g: +m[1] || 0, b: +m[2] || 0, a: m.length > 3 ? +m[3] : 1 };
  };
  const lum = c => {
    const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const sobre = (c, f) => ({ r: c.r * c.a + f.r * (1 - c.a), g: c.g * c.a + f.g * (1 - c.a),
                             b: c.b * c.a + f.b * (1 - c.a), a: 1 });
  const temImagem = s => s.backgroundImage && s.backgroundImage !== 'none';
  /* O fundo efetivo: junta as camadas ate a primeira opaca e compoe. Fundo
     com gradiente/imagem nao se mede -- devolve null em vez de inventar. */
  const fundoDe = el => {
    const camadas = [];
    for (let n = el; n; n = n.parentElement) {
      const s = getComputedStyle(n);
      if (temImagem(s) && n !== document.body && n !== document.documentElement) return null;
      const c = rgb(s.backgroundColor);
      if (c.a > 0) camadas.push(c);
      if (c.a >= 0.99) break;
    }
    let f = camadas.length && camadas[camadas.length - 1].a >= 0.99
      ? camadas.pop() : rgb(getComputedStyle(document.body).backgroundColor);
    if (f.a < 0.99) f = { r: 255, g: 255, b: 255, a: 1 };
    while (camadas.length) f = sobre(camadas.pop(), f);
    return f;
  };
  const visivel = el => {
    if (!el.getClientRects().length) return false;
    const s = getComputedStyle(el);
    if (s.visibility === 'hidden' || parseFloat(s.opacity) === 0) return false;
    const r = el.getBoundingClientRect();
    return r.width >= 1 && r.height >= 1;
  };
  const desligado = el => !!el.closest('button:disabled, input:disabled, select:disabled, textarea:disabled, fieldset:disabled, [aria-disabled="true"]');
  const nome = el => {
    let s = el.tagName.toLowerCase();
    if (el.id) s += '#' + el.id;
    else if (typeof el.className === 'string' && el.className.trim()) s += '.' + el.className.trim().split(/\s+/).slice(0, 2).join('.');
    const p = el.parentElement;
    if (p && !el.id) {
      const pn = p.id ? '#' + p.id : (typeof p.className === 'string' && p.className.trim() ? '.' + p.className.trim().split(/\s+/)[0] : p.tagName.toLowerCase());
      s = pn + ' > ' + s;
    }
    return s;
  };
  // Opacidade dos ancestrais esmaece o texto: entra na conta.
  const opacidade = el => { let o = 1; for (let n = el; n; n = n.parentElement) o *= parseFloat(getComputedStyle(n).opacity); return o; };

  const contraste = [], transform = [], caixas = [], toque = [], acao = [];
  for (const el of document.querySelectorAll('body *')) {
    if (el.closest('svg') && el.tagName.toLowerCase() !== 'svg') continue;
    if (!visivel(el)) continue;
    const s = getComputedStyle(el);
    const direto = [...el.childNodes].filter(n => n.nodeType === 3).map(n => n.nodeValue).join(' ').replace(/\s+/g, ' ').trim();
    const ehCampo = /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName) && !/^(checkbox|radio|range|color|hidden|file)$/.test(el.type || '');
    const texto = ehCampo ? (el.value || '') : direto;

    // 2. contraste
    if (texto.length >= 1 && /[\p{L}\p{N}]/u.test(texto) && !desligado(el)) {
      const f = fundoDe(el);
      if (f) {
        const op = opacidade(el);
        const cor = rgb(s.color); cor.a = (cor.a ?? 1) * op;
        const frente = sobre(cor, f);
        const a = lum(frente), b = lum(f);
        const c = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        const tam = parseFloat(s.fontSize), peso = parseInt(s.fontWeight, 10) || 400;
        const piso = (tam >= 24 || (tam >= 18.66 && peso >= 700)) ? 3.0 : 4.5;
        if (c < piso) contraste.push({ onde: nome(el), txt: texto.slice(0, 40), c: +c.toFixed(2), piso,
          cor: s.color, fundo: `rgb(${Math.round(f.r)}, ${Math.round(f.g)}, ${Math.round(f.b)})`, op: +op.toFixed(2) });
      }
    }
    // 5a. text-transform sobre DADO
    if (s.textTransform !== 'none' && direto) {
      for (const d of dados) if (direto.includes(d)) transform.push({ onde: nome(el), txt: direto.slice(0, 40), tt: s.textTransform, tipo: 'valor' });
      // O nome do banco tem caixa mista e e unico: casa em qualquer lugar. Os
      // nomes de tabela e coluna sao palavras comuns («limite» casava «tempo
      // limite» numa ficha de configuracao) -- so contam quando SAO o texto.
      for (const d of ident) {
        if (d === ident[0] ? direto.includes(d) : direto === d) transform.push({ onde: nome(el), txt: direto.slice(0, 40), tt: s.textTransform, tipo: 'identificador' });
      }
    }
    // 5c. a classe `.caixa` (o dialogo) fora de dialogo: veste de cartao flutuante.
    if (el.classList.contains('caixa') && el.getAttribute('role') !== 'dialog') {
      caixas.push({ onde: nome(el), mal: '.caixa do dialogo fora de dialogo' });
    }
    // 5b. caixa de marcar esticada
    if (el.tagName === 'INPUT' && /^(checkbox|radio)$/.test(el.type)) {
      const r = el.getBoundingClientRect();
      if (r.width > 24 || r.height > 24) caixas.push({ onde: nome(el), w: Math.round(r.width), h: Math.round(r.height) });
    }
    // 6. alvo de toque
    if (largura <= 390 && el.matches('button, a[href], input:not([type=hidden]), select, textarea, summary, [role=button], [role=menuitem], [role=tab], .no')
        && !desligado(el)) {
      const r = el.getBoundingClientRect();
      if ((r.width < 32 || r.height < 32) && r.right > 0 && r.left < innerWidth) {
        // A regua da WCAG 2.5.8 isenta o link dentro de frase.
        const emFrase = el.tagName === 'A' && getComputedStyle(el).display === 'inline';
        if (!emFrase) toque.push({ onde: nome(el), txt: (el.textContent || el.title || el.getAttribute('aria-label') || '').trim().slice(0, 24), w: Math.round(r.width), h: Math.round(r.height) });
      }
    }
    // 4. botoes de acao
    if (el.classList.contains('botao')) {
      const bg = rgb(s.backgroundColor);
      const acoes = ['incluir', 'alterar', 'marcar', 'excluir', 'consultar'].filter(a => el.classList.contains(a));
      const rot = (el.textContent || '').trim().slice(0, 30);
      if (acoes.length) {
        if (bg.a > 0.05) acao.push({ onde: nome(el), txt: rot, mal: 'fundo cheio em repouso', bg: s.backgroundColor });
        else if (s.borderTopStyle === 'none' || parseFloat(s.borderTopWidth) < 0.5) acao.push({ onde: nome(el), txt: rot, mal: 'sem contorno' });
      } else if (bg.a > 0.5 && !el.classList.contains('secundario') && !el.classList.contains('perigo')) {
        const verbo = /^\W*(incluir|novo|nova|criar|acrescentar|adicionar|gravar|salvar|alterar|editar|excluir|apagar|remover|eliminar|consultar|buscar|pesquisar|executar)/i.exec(rot);
        if (verbo) acao.push({ onde: nome(el), txt: rot, mal: `verbo de ação («${verbo[1]}») em .botao de fundo cheio, sem a cor da ação`, bg: s.backgroundColor });
      }
    }
  }
  const de = document.documentElement;
  const lateral = Math.max(de.scrollWidth, document.body.scrollWidth) - de.clientWidth;
  // Quem passa da borda direita (para dizer a CAUSA da rolagem, e nao so o numero).
  const culpados = [];
  if (lateral > 0) {
    for (const el of document.querySelectorAll('body *')) {
      const r = el.getBoundingClientRect();
      if (r.right > de.clientWidth + 1 && r.width > 0 && visivel(el)) {
        let rola = false;
        for (let n = el.parentElement; n && n !== document.body; n = n.parentElement) {
          const o = getComputedStyle(n).overflowX; if (o === 'auto' || o === 'scroll' || o === 'hidden') { rola = true; break; }
        }
        if (!rola) culpados.push(`${nome(el)} (direita=${Math.round(r.right)})`);
      }
      if (culpados.length >= 4) break;
    }
  }
  const fora = document.querySelector('#aviso.mal:not([hidden])');
  const dentro = document.querySelector('#painel .aviso.mal');
  return {
    grosso: matchMedia('(pointer:coarse)').matches,
    erro: [fora ? fora.textContent.trim().slice(0, 200) : '', dentro ? dentro.textContent.trim().slice(0, 200) : ''].filter(Boolean),
    lateral, culpados, contraste, transform, caixas, toque, acao,
  };
};

async function inventario(page) {
  return await page.evaluate(fora => {
    const vistos = new Map();
    const telas = [];
    MENUS.forEach((menu, m) => menu[3].forEach((it, i) => {
      if (it === 'sep') return;
      const chave = it.txt || it.rot;
      const corpo = String(it.faz);
      if (it.falta) { telas.push({ id: chave, rot: it.rot, via: 'menu', m, i, desligado: it.falta.slice(0, 60) }); return; }
      if (fora[chave]) { telas.push({ id: chave, rot: it.rot, via: 'menu', m, i, fora: fora[chave] }); return; }
      // `quando` falso e o item CINZA no menu: a pessoa nao alcanca a tela, e
      // chamar o `faz` por dentro mediria uma recusa que ninguem ve.
      if (it.quando && !it.quando()) { telas.push({ id: chave, rot: it.rot, via: 'menu', m, i, desligado: 'cinza neste estado (quando)' }); return; }
      if (vistos.has(corpo)) return;
      vistos.set(corpo, chave);
      telas.push({ id: chave, rot: it.rot, via: 'menu', m, i });
    }));
    FERRAMENTAS.forEach((f, i) => {
      if (typeof f !== 'object') return;
      const corpo = String(f.faz);
      if (vistos.has(corpo)) return;
      vistos.set(corpo, f.txt || f.rot);
      telas.push({ id: f.txt || ('fer:' + f.rot), rot: 'barra › ' + f.rot, via: 'fer', i });
    });
    for (const a of ['estrutura', 'conteudo', 'indices', 'diario', 'integridade']) telas.push({ id: 'aba:' + a, rot: 'aba › ' + a, via: 'aba', aba: a });
    return telas;
  }, Object.fromEntries(FORA));
}

async function main() {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  const servidor = await subir({ phxsqld, portaDados: PORTA, portaWeb: PORTA + 1, log: m => console.log(m) });
  const navegador = await chromium.launch();
  const resultado = { quando: new Date().toISOString(), combinacoes: [] };
  try {
    let preparado = false;
    for (const tema of TEMAS) {
      for (const largura of LARGURAS) {
        // 390 e CELULAR: dedo, e nao mouse. A casa decidiu que o teste do alvo
        // de toque e `(pointer:coarse)`, e nao a largura (ver o bloco «dedo
        // grosso» do index.html) -- medir 390 com ponteiro fino acusaria o que
        // nenhum celular ve.
        const celular = largura <= 390;
        const ctxNav = await navegador.newContext({
          viewport: { width: largura, height: celular ? 844 : (largura >= 1920 ? 1080 : 800) },
          ...(celular ? { isMobile: true, hasTouch: true } : {}),
        });
        await ctxNav.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* */ } }, tema);
        const page = await ctxNav.newPage();
        page.on('dialog', d => d.dismiss().catch(() => {}));
        page.setDefaultTimeout(15000);
        const erros = [];
        page.on('pageerror', e => erros.push(String(e.message).split('\n')[0]));
        const comb = { tema, largura, telas: [] };
        resultado.combinacoes.push(comb);

        // A tela de entrada, antes de entrar.
        await page.goto(servidor.url, { waitUntil: 'domcontentloaded' });
        await page.waitForSelector('#btEntrar');
        await page.waitForTimeout(400);
        comb.telas.push({ id: 'entrada', rot: 'tela de entrada', ...(await page.evaluate(MEDIR, { dados: DADOS, ident: IDENT, largura })), pageerror: erros.splice(0) });
        if (CAPTURAS) { mkdirSync(CAPTURAS, { recursive: true }); await page.screenshot({ path: join(CAPTURAS, `${tema}-${largura}-entrada.png`) }); }

        await entrar(page, servidor.url);
        if (!preparado) {
          await cenario(page, DB, 'clientes');
          await cenario(page, DB, 'pedidos');
          preparado = true;
        }
        const garantir = async () => {
          if (await page.evaluate(() => !est.atual)) {
            await page.evaluate(([d, t]) => abrirTabela(d, t), [DB, 'clientes']);
            await page.waitForTimeout(250);
          }
        };
        await garantir();
        const telas = await inventario(page);
        for (const t of telas) {
          if (t.fora || t.desligado) { comb.telas.push({ ...t }); continue; }
          await garantir();
          await page.evaluate(() => {
            const a = document.getElementById('aviso');
            if (a) { a.hidden = true; a.className = 'recado'; a.textContent = ''; }
            for (const x of document.querySelectorAll('#painel .aviso.mal')) x.remove();
          });
          erros.splice(0);
          try {
            // Dispara e NAO espera: a tela que abre um dialogo proprio
            // (`perguntarTexto`) so resolve quando alguem responde, e esperar
            // por ela pararia a revisao inteira na primeira.
            if (t.via === 'menu') await page.evaluate(([m, i]) => { Promise.resolve().then(() => MENUS[m][3][i].faz()).catch(e => avisar(String(e.message || e), true)); }, [t.m, t.i]);
            else if (t.via === 'fer') await page.evaluate(i => { Promise.resolve().then(() => FERRAMENTAS[i].faz()).catch(e => avisar(String(e.message || e), true)); }, t.i);
            else {
              await page.evaluate(([d, tb]) => abrirTabela(d, tb), [DB, 'clientes']);
              await page.evaluate(a => irAba(a), t.aba);
            }
          } catch (e) { erros.push('evaluate: ' + String(e.message).split('\n')[0]); }
          await page.mouse.move(2, 2);
          await page.waitForTimeout(600);
          const med = await page.evaluate(MEDIR, { dados: DADOS, ident: IDENT, largura }).catch(e => ({ falhaMedir: String(e.message) }));
          comb.telas.push({ id: t.id, rot: t.rot, ...med, pageerror: erros.splice(0) });
          if (process.env.REV_VERBOSO) console.log(`  ${t.rot}`);
          if (CAPTURAS) await page.screenshot({ path: join(CAPTURAS, `${tema}-${largura}-${t.id.replace(/[^\w.-]+/g, '_')}.png`) }).catch(() => {});
          // Dialogo proprio (.sobre) e regioes divididas nao podem sobrar para a proxima tela.
          await page.evaluate(() => {
            for (const s of document.querySelectorAll('.sobre')) s.remove();
            try { if (window.PhxTelas && PhxTelas.dividir) PhxTelas.dividir(1); } catch { /* */ }
          }).catch(() => {});
          await page.keyboard.press('Escape').catch(() => {});
        }
        console.log(`${tema} ${largura}: ${comb.telas.length} telas`);
        await ctxNav.close();
      }
    }
  } finally {
    await navegador.close();
    await servidor.derrubar();
  }
  writeFileSync(SAIDA, JSON.stringify(resultado, null, 1));
  console.log('gravado', SAIDA);
}

main().catch(e => { console.error(e); process.exit(1); });

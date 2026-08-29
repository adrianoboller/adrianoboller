// Exercicio do Centro de Controle: percorre as telas em tres viewports e dois
// temas, mede a rolagem horizontal do CORPO e o contraste dos pares principais,
// e guarda uma captura de cada tela.
//
// Uso:  node exercicio.mjs <pasta-de-saida> [rotulo]
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, writeFileSync } from 'node:fs';

const SAIDA = process.argv[2] || 'tiros';
const ROTULO = process.argv[3] || '';
const BASE = 'http://127.0.0.1:5770/';

const VIEWPORTS = [
  { nome: 'celular', width: 390, height: 844 },
  { nome: 'tablet', width: 820, height: 1180 },
  { nome: 'desktop', width: 1440, height: 900 },
];

// Cada tela: como chegar nela a partir do app aberto.
const TELAS = [
  { id: 'painel', ir: async p => clicarArvore(p, 'Painel') },
  { id: 'tabela-estrutura', ir: async p => { await clicarArvore(p, 'clientes'); } },
  { id: 'tabela-conteudo', ir: async p => { await clicarArvore(p, 'clientes'); await aba(p, 'Conteúdo'); } },
  { id: 'tabela-indices', ir: async p => { await clicarArvore(p, 'clientes'); await aba(p, 'Índices'); } },
  { id: 'tabela-diario', ir: async p => { await clicarArvore(p, 'clientes'); await aba(p, 'Diário'); } },
  { id: 'tabela-integridade', ir: async p => { await clicarArvore(p, 'clientes'); await aba(p, 'Integridade'); } },
  { id: 'view-db', ir: async p => fer(p, 'View DB') },
  { id: 'bancos', ir: async p => fer(p, 'Bancos') },
  { id: 'gerir-banco', ir: async p => fer(p, 'Gerir Banco') },
  { id: 'tabelas', ir: async p => fer(p, 'Tabelas') },
  { id: 'query', ir: async p => fer(p, 'Query') },
  { id: 'pivot', ir: async p => fer(p, 'Pivot') },
  { id: 'juncao', ir: async p => fer(p, 'Junção') },
  { id: 'exportar', ir: async p => { await clicarArvore(p, 'clientes'); await fer(p, 'Exportar'); } },
  { id: 'importar', ir: async p => { await clicarArvore(p, 'clientes'); await fer(p, 'Importar'); } },
  { id: 'usuarios', ir: async p => fer(p, 'Usuários') },
  { id: 'conexoes', ir: async p => fer(p, 'Conexões') },
  { id: 'config', ir: async p => fer(p, 'Config') },
  { id: 'jobs', ir: async p => fer(p, 'Jobs') },
  { id: 'lixeira', ir: async p => { await clicarArvore(p, 'clientes'); await fer(p, 'Lixeira'); } },
  { id: 'transacoes', ir: async p => fer(p, 'Transações') },
  { id: 'dblink', ir: async p => fer(p, 'DbLink') },
  { id: 'systables', ir: async p => fer(p, 'SysTables') },
  { id: 'diagrama-er', ir: async p => fer(p, 'Diagrama ER') },
  { id: 'lgpd', ir: async p => fer(p, 'LGPD') },
  { id: 'replicacao', ir: async p => fer(p, 'Replicação') },
  { id: 'profiler', ir: async p => fer(p, 'Profiler') },
  { id: 'diretivas', ir: async p => fer(p, 'Diretivas') },
  { id: 'servico', ir: async p => fer(p, 'Start/Stop') },
  { id: 'ajuda', ir: async p => fer(p, 'Ajuda') },
  { id: 'acessos', ir: async p => clicarArvore(p, 'Acessos') },
  { id: 'bloqueios', ir: async p => clicarArvore(p, 'Bloqueios') },
];

async function fer(p, rot) {
  // A barra de ferramentas esta SEMPRE na tela -- ela nao mora na gaveta. Se a
  // gaveta estiver aberta, porem, o veu cobre tudo: fecha antes de clicar.
  await fecharGavetaSeAberta(p);
  const b = p.locator(`#ferramentas .fer[title^="${rot}"]`).first();
  await b.click({ timeout: 4000 });
  await p.waitForTimeout(700);
}

async function fecharGavetaSeAberta(p) {
  const veu = p.locator('#veuArvore');
  if (await veu.count() && await veu.isVisible().catch(() => false)) {
    await veu.click({ force: true }).catch(() => {});
    await p.waitForTimeout(300);
  }
}

async function clicarArvore(p, texto) {
  await abrirNavegacaoSePreciso(p);
  const n = p.locator(`#arvore .no`, { hasText: texto }).first();
  await n.click({ timeout: 4000 });
  await p.waitForTimeout(700);
}

// Quando existir um botao de abrir o painel lateral e a arvore estiver
// escondida, abre antes de procurar o alvo. Antes do conserto isso e um no-op.
async function abrirNavegacaoSePreciso(p) {
  const bt = p.locator('#btLateral');
  if (await bt.count() === 0) return;
  const precisa = await p.evaluate(() => {
    const a = document.querySelector('#arvore');
    if (!a) return false;
    const r = a.getBoundingClientRect();
    return r.width < 40 || r.right <= 0 || getComputedStyle(a).visibility === 'hidden';
  });
  if (precisa) { await bt.click(); await p.waitForTimeout(360); }
}

async function aba(p, rot) {
  const b = p.locator(`#abas .aba`, { hasText: rot }).first();
  if (await b.count()) { await b.click(); await p.waitForTimeout(600); }
}

// ------------------------------------------------------------- vazamento
// `body{overflow:hidden}` faz o documentElement.scrollWidth NUNCA passar da
// janela: medir so ele responde "nao vaza" mesmo quando a tela esta cortada.
// Entao mede-se tambem o que passa da borda e o que fica CORTADO SEM ROLO --
// conteudo mais largo que o pai dentro de um `overflow:hidden` e conteudo que
// nao da para alcancar de jeito nenhum, que e pior que rolagem.
function medirVazamento() {
  const W = window.innerWidth;
  const passa = [];
  const cortado = [];
  // Algum ancestral rola de lado por conta propria?
  const temRoloHorizontal = el => {
    for (let n = el.parentElement; n && n !== document.body; n = n.parentElement) {
      const ox = getComputedStyle(n).overflowX;
      if ((ox === 'auto' || ox === 'scroll') && n.scrollWidth > n.clientWidth + 1)
        return true;
    }
    return false;
  };
  const nome = el => el.tagName.toLowerCase()
    + (el.id ? '#' + el.id : '')
    + (el.className && typeof el.className === 'string'
        ? '.' + el.className.trim().split(/\s+/).slice(0, 3).join('.') : '');
  for (const el of document.querySelectorAll('#app *')) {
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) continue;
    const cs = getComputedStyle(el);
    if (cs.visibility === 'hidden' || cs.display === 'none') continue;
    if (cs.position === 'fixed') continue;
    // Passar da borda so e defeito se NAO houver um rolo proprio no caminho:
    // um botao da barra de ferramentas que esta em 1806px continua alcancavel,
    // porque a barra rola. O que nao pode e a PAGINA rolar de lado.
    if (r.right > W + 1 && !temRoloHorizontal(el))
      passa.push({ el: nome(el), right: Math.round(r.right), largura: Math.round(r.width) });
    const ox = cs.overflowX;
    // Corte com reticencias nao e conteudo perdido: ele AVISA que continua, e
    // o texto inteiro fica no `title`. O que conta como defeito e o corte
    // mudo, sem reticencia e sem rolo.
    const comReticencia = cs.textOverflow === 'ellipsis';
    if ((ox === 'hidden' || ox === 'clip') && !comReticencia
        && el.scrollWidth > el.clientWidth + 1)
      cortado.push({ el: nome(el), scrollWidth: el.scrollWidth, clientWidth: el.clientWidth });
  }
  // Só os piores: um filho que vaza costuma arrastar todos os pais junto.
  const top = a => a.sort((x, y) => (y.right || y.scrollWidth) - (x.right || x.scrollWidth)).slice(0, 6);
  return {
    scrollWidth: document.documentElement.scrollWidth,
    innerWidth: W,
    bodyScroll: document.body.scrollWidth,
    passa: top(passa),
    cortado: top(cortado),
  };
}

// --------------------------------------------------------------- contraste
function CONTRASTE() {
  const lum = c => {
    const [r,g,b] = c;
    const f = v => { v/=255; return v<=0.03928 ? v/12.92 : Math.pow((v+0.055)/1.055,2.4); };
    return 0.2126*f(r)+0.7152*f(g)+0.0722*f(b);
  };
  const rgb = s => (s.match(/[\d.]+/g)||[0,0,0]).slice(0,3).map(Number);
  const alpha = s => { const m = s.match(/[\d.]+/g); return m && m.length>3 ? Number(m[3]) : 1; };
  // Sobe a arvore ate achar um fundo opaco, e compoe os translucidos por cima.
  const fundoReal = el => {
    const pilha = [];
    for (let n = el; n; n = n.parentElement) {
      const bg = getComputedStyle(n).backgroundColor;
      const a = alpha(bg);
      if (a === 0) continue;
      pilha.push([rgb(bg), a]);
      if (a === 1) break;
    }
    let cor = [255,255,255];
    for (let i = pilha.length-1; i>=0; i--) {
      const [c,a] = pilha[i];
      cor = cor.map((v,k) => c[k]*a + v*(1-a));
    }
    return cor;
  };
  const razao = (a,b) => { const l1=lum(a),l2=lum(b);
    return (Math.max(l1,l2)+0.05)/(Math.min(l1,l2)+0.05); };

  const alvos = [];
  const ver = (rotulo, sel) => {
    const el = document.querySelector(sel);
    if (!el) return;
    const cs = getComputedStyle(el);
    if (!el.getClientRects().length) return;
    const fg = rgb(cs.color);
    const bg = fundoReal(el);
    const px = parseFloat(cs.fontSize);
    const grande = px >= 24 || (px >= 18.66 && Number(cs.fontWeight) >= 700);
    alvos.push({ rotulo, sel, razao: +razao(fg,bg).toFixed(2),
                 px: +px.toFixed(1), grande,
                 minimo: grande ? 3 : 4.5,
                 cor: cs.color, fundo: 'rgb('+bg.map(Math.round).join(', ')+')' });
  };
  ver('corpo', 'body');
  ver('barra: eu', '.barra .eu');
  ver('barra: fita', '.barra .fita');
  ver('menubar: titulo', '.menubar .titulo');
  ver('ferramenta: rotulo', '#ferramentas .fer .rot');
  ver('arvore: grupo', '#arvore .grupo');
  ver('arvore: no', '#arvore .no');
  ver('arvore: no selecionado', '#arvore .no.sel');
  ver('cabecalho h2', '.cabecalho h2');
  ver('cabecalho sub', '.cabecalho .sub');
  ver('aba', '.aba');
  ver('aba selecionada', '.aba.sel');
  ver('tabela: th', 'thead th');
  ver('tabela: td', 'tbody td');
  ver('tabela: td.dado', 'td.dado');
  ver('ficha: rotulo', '.ficha .r');
  ver('ficha: valor', '.ficha .v');
  ver('kpi: rotulo', '.kpi .r');
  ver('kpi: valor', '.kpi .v');
  ver('pino', '.pino');
  ver('aviso', '.aviso');
  ver('legenda', '.leg');
  ver('dica', '.dica');
  ver('botao primario', '.botao:not(.secundario):not(.mini)');
  ver('botao secundario', '.botao.secundario');
  ver('botao mini', '.botao.mini');
  ver('vazio', '.vazio');
  ver('texto-3 generico', '.texto-3');
  return alvos;
}

// ------------------------------------------------------------------- corrida
async function main() {
  mkdirSync(SAIDA, { recursive: true });
  const nav = await chromium.launch();
  const relatorio = { rotulo: ROTULO, rolagem: [], contraste: [], falhas: [] };

  for (const vp of VIEWPORTS) {
    for (const tema of ['escuro', 'claro']) {
      const ctx = await nav.newContext({
        viewport: { width: vp.width, height: vp.height },
        deviceScaleFactor: 1,
        isMobile: vp.nome === 'celular',
        hasTouch: vp.nome !== 'desktop',
      });
      const p = await ctx.newPage();
      await p.addInitScript(t => {
        try { localStorage.setItem('phxsql-tema', t); } catch {}
      }, tema);
      await p.goto(BASE);
      // O campo do token so aparece depois do /saude: detectarModo desesconde
      // o pai dele. Sem esperar, o fill corre antes da resposta.
      await p.waitForSelector('#t', { state: 'visible', timeout: 15000 });
      await p.fill('#t', 'design-token');
      await p.fill('#u', 'adriano');
      await p.fill('#s', 'design123');
      await p.locator('#btEntrar').click();
      await p.waitForSelector('#app.ativo', { timeout: 15000 });
      await p.waitForTimeout(1200);

      for (const tela of TELAS) {
        const tag = `${vp.nome}-${tema}-${tela.id}`;
        try {
          await tela.ir(p);
        } catch (e) {
          relatorio.falhas.push({ tela: tag, erro: String(e).split('\n')[0] });
          continue;
        }
        await p.waitForTimeout(250);
        const med = await p.evaluate(medirVazamento);
        relatorio.rolagem.push({ tela: tag, ...med,
          vaza: med.scrollWidth > med.innerWidth || med.bodyScroll > med.innerWidth,
          nPassa: med.passa.length, nCortado: med.cortado.length });
        await p.screenshot({ path: `${SAIDA}/${tag}.png` }).catch(() => {});
        if (tela.id === 'painel' || tela.id === 'tabela-conteudo') {
          try {
            const c = await p.evaluate(CONTRASTE);
            relatorio.contraste.push({ tela: tag, alvos: c });
          } catch (e) {
            relatorio.falhas.push({ tela: tag + ' (contraste)', erro: String(e).split('\n')[0] });
          }
        }
      }
      await ctx.close();
    }
  }
  await nav.close();
  writeFileSync(`${SAIDA}/relatorio.json`, JSON.stringify(relatorio, null, 1));

  const vazam = relatorio.rolagem.filter(r => r.vaza);
  console.log(`\n== ROLAGEM HORIZONTAL DO CORPO: ${vazam.length} de ${relatorio.rolagem.length} telas ==`);
  for (const v of vazam) console.log(`  ${v.tela}: scrollWidth ${v.scrollWidth} > innerWidth ${v.innerWidth}`);

  const passam = relatorio.rolagem.filter(r => r.nPassa);
  console.log(`\n== CONTEUDO PASSANDO DA BORDA DIREITA: ${passam.length} de ${relatorio.rolagem.length} telas ==`);
  for (const v of passam.slice(0, 24))
    console.log(`  ${v.tela}: ${v.passa.map(x => `${x.el} ate ${x.right}px`).join(' | ')}`);
  if (passam.length > 24) console.log(`  ... e mais ${passam.length - 24}`);

  const cort = relatorio.rolagem.filter(r => r.nCortado);
  console.log(`\n== CONTEUDO CORTADO SEM ROLO (inalcancavel): ${cort.length} de ${relatorio.rolagem.length} telas ==`);
  for (const v of cort.slice(0, 24))
    console.log(`  ${v.tela}: ${v.cortado.map(x => `${x.el} ${x.scrollWidth}>${x.clientWidth}`).join(' | ')}`);
  if (cort.length > 24) console.log(`  ... e mais ${cort.length - 24}`);

  console.log(`\n== CONTRASTE ABAIXO DO MINIMO ==`);
  let ruins = 0;
  for (const c of relatorio.contraste)
    for (const a of (c.alvos || []))
      if (a.razao < a.minimo) { ruins++; console.log(`  ${c.tela} | ${a.rotulo} = ${a.razao}:1 (min ${a.minimo}) ${a.cor} sobre ${a.fundo}`); }
  if (!ruins) console.log('  nenhum');

  if (relatorio.falhas.length) {
    console.log(`\n== TELAS QUE NAO ABRIRAM: ${relatorio.falhas.length} ==`);
    for (const f of relatorio.falhas) console.log(`  ${f.tela}: ${f.erro}`);
  }
}
main();

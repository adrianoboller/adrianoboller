/* O VIDEO do CRUD, do login a prova no disco -- pedido do dono:
 * «um video de apresentacao desde a abertura pelo login ate criacao de tabela
 * e outras atividades de um CRUD usando o PhxSql».
 *
 *   node testes-web/video-crud.mjs          (da pasta phxsql/)
 *
 * Tudo pela TELA, contra um phxsqld de verdade subido pelo `servidor.mjs`.
 * O que a tela nao oferece, o video DIZ que a tela nao oferece -- nao finge.
 *
 * E a cena que o dono pediu por ultimo e a que justifica o resto: a PROVA NO
 * DISCO. Depois do CRUD o servidor e DERRUBADO e quem le a pasta e o proprio
 * motor pela CLI (`phxsql info|listar|log|verificar`) -- um leitor que nao
 * passa pela tela nem pela memoria do servidor. Por cima disso, so o que o
 * `docs/FORMATO.md` §1 manda ler do `.reg` cru: tres campos do cabecalho
 * (`slot_size` @16, `slot_count` @20, `data_offset` @44) e, por slot, o
 * status (@0) e a versao (@8). Nao ha parser novo do payload aqui: o texto
 * digitado e achado como BYTES UTF-8 dentro do slot, e e isso que o hex
 * mostra destacado.
 *
 * Cada cena roda dentro de `cena()`: falhou, vira uma linha no log dizendo
 * QUAL, e o roteiro segue -- uma gravacao inteira ja se perdeu por um passo.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, rmSync, readdirSync, statSync, existsSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { subir, USUARIO, SENHA, TOKEN } from './servidor.mjs';

const PORTA_DADOS = 6320;
const PORTA_WEB = 6321;
const SAIDA = process.env.SAIDA
  || '/tmp/claude-0/-home-user-adrianoboller/5451b70d-11db-5d31-b519-9670a7730887/scratchpad/video-crud';
const QUADROS = join(SAIDA, 'quadros');
const RAIZ = process.cwd();
const BANCO = 'Comercial';
const TABELA = 'clientes';
const LARG = 1280, ALT = 720;

// Os clientes. `rowid` e o que o motor DEVE dar, na ordem de digitacao.
const CLIENTES = [
  { nome: 'Ana Beatriz Moreira',    cidade: 'Blumenau',      limite: '1500.00',  nascimento: '1988-03-14', ativo: 'true' },
  { nome: 'Carlos Eduardo Lima',    cidade: 'Joinville',     limite: '2750.50',  nascimento: '1975-11-02', ativo: 'true' },
  { nome: 'Fernanda Souza',         cidade: 'Florianópolis', limite: '980.00',   nascimento: '1993-07-21', ativo: 'true' },
  { nome: 'João Pedro Albuquerque', cidade: 'Curitiba',      limite: '12000.00', nascimento: '1969-01-30', ativo: 'false' },
  { nome: 'Mariana Kowalski',       cidade: 'Blumenau',      limite: '640.25',   nascimento: '2001-05-09', ativo: 'true' },
];
const SEXTO = { nome: 'Rafael Nunes', cidade: 'Itajaí', limite: '3300.00', nascimento: '1990-09-12', ativo: 'true' };
const ALTERADO = { rowid: 2, antes: { cidade: 'Joinville', limite: '2750.50' },
                   depois: { cidade: 'Jaraguá do Sul', limite: '3100.00' } };
const SUAVE = 3;   // Fernanda: marcada, volta
const FISICO = 5;  // Mariana: de vez, vai para o .trash

const log = (...a) => console.log(...a);
const falhas = [];
let page;

const respirar = (ms = 900) => page.waitForTimeout(ms);

async function quadro(nome) {
  try { await page.screenshot({ path: join(QUADROS, `${nome}.png`) }); } catch { /* o video segue */ }
}

/** Cena: o erro vira linha no log e o roteiro continua. */
async function cena(nome, fn) {
  log('>>', nome);
  try { await fn(); }
  catch (e) {
    falhas.push(nome);
    log('   !! cena falhou:', nome, '|', String(e.message).split('\n')[0]);
    await quadro(`falha-${nome.replace(/\W+/g, '-')}`);
  }
}

/* ------------------------------------------------------------ a marca */
// A Exo 2 entra SO nas camadas do video, com nome proprio: dar a ela o nome
// «Exo 2» faria a TELA do produto herdar a fonte e esconderia no video o que
// ela mostra sem a fonte. Defeito de tela nao se conserta escondido aqui.
const EXO = readFileSync(join(RAIZ, 'crates/phxzip-web/ui/fonte/exo2-latin.woff2')).toString('base64');
const SIMBOLO = readFileSync(join(RAIZ, 'marca/derivados/phxsql-simbolo-440.png')).toString('base64');
const ICONE = readFileSync(join(RAIZ, 'marca/derivados/phxsql-icone-64.png')).toString('base64');

async function marcaNaPagina() {
  // Por `FontFace` com os BYTES, e nao `@font-face` com `data:`: quando o
  // video nasceu, o CSP da pagina (`font-src https://fonts.gstatic.com`)
  // recusava `data:`. Desde o pedido 691 a tela traz a Exo 2 embutida e o CSP
  // e `font-src data:`; o nome proprio `VideoExo` continua para as camadas do
  // video nao se confundirem com a fonte da tela.
  await page.evaluate(async exo => {
    if (window.__exoVideo) return;
    const bytes = Uint8Array.from(atob(exo), c => c.charCodeAt(0));
    const f = new FontFace('VideoExo', bytes, { weight: '100 900' });
    await f.load(); document.fonts.add(f); window.__exoVideo = true;
    const s = document.createElement('style');
    s.textContent = '#__faixa,#__cartaz,#__folha{font-family:"VideoExo",system-ui,sans-serif}';
    document.head.appendChild(s);
  }, EXO);
}

/** Faixa de legenda no rodape -- o capitulo e uma frase. */
async function diz(cap, txt, ms = 2600) {
  await marcaNaPagina();
  await page.evaluate(([c, t]) => {
    let d = document.getElementById('__faixa');
    if (!d) {
      d = document.createElement('div');
      d.id = '__faixa';
      d.style.cssText = 'position:fixed;left:0;right:0;bottom:0;z-index:99998;pointer-events:none;'
        + 'background:linear-gradient(0deg,rgba(1,4,24,.97),rgba(1,4,24,.86));'
        + 'border-top:2px solid #ff4d10;padding:9px 28px 11px;color:#dde2eb';
      d.innerHTML = phxHTML('<div id="__fcap" style="font-size:11px;letter-spacing:.2em;color:#ff8a1c;margin-bottom:3px"></div>'
        + '<div id="__ftxt" style="font-size:19px;line-height:1.3;font-weight:500"></div>');
      document.body.appendChild(d);
    }
    if (c) document.getElementById('__fcap').textContent = c;
    document.getElementById('__ftxt').textContent = t;
  }, [cap, txt]);
  await respirar(ms);
}

async function semFaixa() {
  await page.evaluate(() => { const d = document.getElementById('__faixa'); if (d) d.remove(); });
}

/** Cartaz curto entre as cenas. */
async function cartaz(numero, texto, ms = 1700) {
  await marcaNaPagina();
  await page.evaluate(([n, t, ico]) => {
    let d = document.getElementById('__cartaz');
    if (!d) {
      d = document.createElement('div');
      d.id = '__cartaz';
      d.style.cssText = 'position:fixed;inset:0;z-index:99999;display:flex;align-items:center;'
        + 'justify-content:center;flex-direction:column;gap:12px;background:rgba(1,4,24,.95);'
        + 'color:#fff;font-weight:600;font-size:34px;line-height:1.25;text-align:center;padding:40px;transition:opacity .25s';
      document.body.appendChild(d);
    }
    d.innerHTML = phxHTML(`<img src="data:image/png;base64,${ico}" style="width:44px;opacity:.9">`
      + `<div style="font-size:13px;letter-spacing:.24em;color:#ff8a1c">${n}</div><div>${t}</div>`);
    d.style.opacity = '1';
  }, [numero, texto, ICONE]);
  await respirar(ms);
  await page.evaluate(() => {
    const d = document.getElementById('__cartaz');
    if (d) { d.style.opacity = '0'; setTimeout(() => d.remove(), 300); }
  });
  await respirar(350);
}

/** Uma folha cheia, com a marca, por cima da tela (listagem e prova do disco). */
async function folha(html) {
  await marcaNaPagina();
  await page.evaluate(h => {
    let d = document.getElementById('__folha');
    if (!d) {
      d = document.createElement('div');
      d.id = '__folha';
      d.style.cssText = 'position:fixed;inset:0;z-index:99990;background:#010418;color:#e8eaf2;'
        + 'padding:26px 40px 70px;overflow:hidden;font-size:14px;line-height:1.45';
      document.body.appendChild(d);
    }
    d.innerHTML = phxHTML(h);
  }, html);
}
async function semFolha() {
  await page.evaluate(() => { const d = document.getElementById('__folha'); if (d) d.remove(); });
}

const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

/* ------------------------------------------------- leitura crua do disco */
function arquivosDe(pasta) {
  return readdirSync(pasta).sort().map(f => {
    const st = statSync(join(pasta, f));
    return { nome: f, bytes: st.size, quando: st.mtime.toISOString().slice(0, 19).replace('T', ' '), dir: st.isDirectory() };
  });
}

// O papel de cada extensao, da tabela do docs/FORMATO.md (topo do arquivo).
const PAPEL = {
  '.reg': 'os registros, na ordem de digitação (slots de largura fixa)',
  '.ndx': 'os índices (B+tree), todos no mesmo arquivo',
  '.bin': 'binários (imagens, anexos), fora do slot',
  '.memo': 'textos longos, fora do slot',
  '.log': 'o diário: inclusões, alterações e exclusões',
  '.trash': 'a linha inteira excluída de vez, antes de o slot sair',
  '.reason': 'por que cada linha foi excluída, quando e por quem',
  '.lgpd': 'quem alterou e quem leu dado pessoal',
  '.pag': 'descritor de partição',
  '.fts': 'índice de texto',
  '.bkp': 'espelho do .reg',
};
// Os de controle da pasta, do docs/FORMATO.md §8 e §11.
const PAPEL_ARQ = {
  '_database.json': 'o tipo do database (§11)',
  '_formato-volumes.json': 'o separador de volume (§8)',
  '.phxsql.trava': 'a trava de quem grava a pasta (§11.2)',
};
const extDe = n => { const i = n.lastIndexOf('.'); return i < 0 ? '' : n.slice(i); };

function tabelaDeArquivos(lista, antes) {
  const porNome = new Map((antes || []).map(a => [a.nome, a]));
  return `<table style="border-collapse:collapse;font-size:14px;margin-top:6px">
    <tr style="color:#8a93ad;font-size:12px;letter-spacing:.08em"><td style="padding:3px 26px 8px 0">ARQUIVO</td>
      <td style="padding:3px 26px 8px 0;text-align:right">BYTES</td>${antes ? '<td style="padding:3px 26px 8px 0;text-align:right">ANTES</td>' : ''}
      <td style="padding:3px 26px 8px 0">MODIFICADO</td><td style="padding:3px 0 8px">O QUE É (docs/FORMATO.md)</td></tr>
    ${lista.map(a => {
      const v = porNome.get(a.nome);
      const cresceu = v && v.bytes !== a.bytes;
      return `<tr><td style="padding:2px 26px 2px 0;font-family:ui-monospace,monospace;color:#ffb27a">${esc(a.nome)}</td>
        <td style="padding:2px 26px 2px 0;text-align:right;font-family:ui-monospace,monospace${cresceu ? ';color:#5fe08a' : ''}">${a.bytes.toLocaleString('pt-BR')}</td>
        ${antes ? `<td style="padding:2px 26px 2px 0;text-align:right;font-family:ui-monospace,monospace;color:#8a93ad">${v ? v.bytes.toLocaleString('pt-BR') : 'não existia'}</td>` : ''}
        <td style="padding:2px 26px 2px 0;color:#8a93ad">${a.quando}</td>
        <td style="padding:2px 0;color:#c9cfdc">${esc(PAPEL_ARQ[a.nome] || PAPEL[extDe(a.nome)] || (a.dir ? 'pasta' : '—'))}</td></tr>`;
    }).join('')}</table>`;
}

/** Hex com o texto ao lado; `marcas` = [[ini, fim, cor]] em offsets absolutos. */
function hexDump(buf, ini, fim, marcas = []) {
  const cor = o => { const m = marcas.find(([a, b]) => o >= a && o < b); return m ? m[2] : null; };
  let out = '';
  for (let o = ini; o < fim; o += 16) {
    let hx = '', tx = '';
    for (let k = 0; k < 16; k++) {
      const p = o + k;
      if (p >= fim || p >= buf.length) { hx += '   '; continue; }
      const b = buf[p], c = cor(p);
      const h = b.toString(16).padStart(2, '0');
      const ch = b >= 32 && b < 127 ? esc(String.fromCharCode(b)) : (b >= 0xc0 ? '·' : '.');
      hx += (c ? `<b style="background:${c};color:#010418">${h}</b>` : h) + ' ';
      tx += c ? `<b style="background:${c};color:#010418">${ch}</b>` : ch;
    }
    out += `<span style="color:#5d6680">${o.toString(16).padStart(6, '0')}</span>  ${hx} ${tx}\n`;
  }
  return `<pre style="margin:0;font:12px/1.35 ui-monospace,monospace;color:#c9cfdc">${out}</pre>`;
}

/** Os tres campos do cabecalho e o par status/versao por slot (FORMATO §1). */
function lerReg(arq) {
  const b = readFileSync(arq);
  const assinatura = b.subarray(0, 6).toString('latin1');
  const slotSize = b.readUInt32LE(16);
  const slotCount = Number(b.readBigUInt64LE(20));
  const liveCount = Number(b.readBigUInt64LE(28));
  const dataOffset = Number(b.readBigUInt64LE(44));
  const slots = [];
  for (let r = 1; r <= slotCount; r++) {
    const off = dataOffset + (r - 1) * slotSize;
    slots.push({ rowid: r, off, status: b[off], versao: Number(b.readBigUInt64LE(off + 8)),
                 fim: off + slotSize });
  }
  return { b, assinatura, slotSize, slotCount, liveCount, dataOffset, slots };
}
const temTexto = (buf, ini, fim, s) => {
  const i = buf.subarray(ini, fim).indexOf(Buffer.from(s, 'utf8'));
  return i < 0 ? -1 : ini + i;
};

function cli(args) {
  const r = spawnSync(join(RAIZ, 'target/debug/phxsql'), args, { encoding: 'utf8' });
  return { ok: r.status === 0, saida: (r.stdout || '') + (r.stderr || '') };
}

/* ------------------------------------------------------------- a tela */
const toolbar = rot => page.locator(`#ferramentas .fer[title^="${rot}"]`).first();

async function abrirConteudo() {
  await page.locator(`#arvore .no.db[data-db="${BANCO}"]`).first().click();
  await page.waitForSelector(`#gradeDb .bt-db[data-tab="${TABELA}"]`, { timeout: 15000 });
  await respirar(700);
  await page.locator(`#gradeDb .bt-db[data-tab="${TABELA}"]`).click();
  await page.waitForSelector('#btNova', { timeout: 15000 });
  await respirar(600);
}

async function incluir(c, devagar) {
  await page.locator('#btNova').click();
  await page.waitForSelector('#f_nome', { timeout: 10000 });
  await respirar(devagar ? 700 : 300);
  for (const campo of ['nome', 'cidade', 'limite', 'nascimento']) {
    const el = page.locator(`#f_${campo}`);
    if (devagar) { await el.click(); await el.pressSequentially(c[campo], { delay: 45 }); }
    else await el.fill(c[campo]);
    await respirar(devagar ? 250 : 90);
  }
  await page.selectOption('#f_ativo', c.ativo);
  await respirar(devagar ? 900 : 350);
  if (devagar) await quadro('05-ficha-preenchida');
  await page.locator('#btSalvar').click();
  await page.waitForSelector('#btNova', { timeout: 10000 });
  await page.waitForFunction(n => [...document.querySelectorAll('#gradeEdit td')]
    .some(td => td.textContent.trim() === n), c.nome, { timeout: 10000 });
  await respirar(devagar ? 1100 : 500);
}

/** Rola a grade para o topo do painel: a faixa de legenda cobre o rodape. */
async function verGrade(sel = '#gradeEdit') {
  await page.evaluate(q => { const g = document.querySelector(q); if (g) g.scrollIntoView({ block: 'start' }); }, sel);
  await respirar(250);
}

async function abrirFichaDe(rowid) {
  await page.locator(`#gradeEdit .bt-editar[data-rowid="${rowid}"]`).click();
  await page.waitForSelector('#btSalvar', { timeout: 10000 });
  await respirar(700);
}

async function principal() {
  rmSync(SAIDA, { recursive: true, force: true });
  mkdirSync(QUADROS, { recursive: true });

  const phxsqld = join(RAIZ, 'target/debug/phxsqld');
  for (const b of [phxsqld, join(RAIZ, 'target/debug/phxsql')]) {
    if (!existsSync(b)) { console.error(`falta ${b} -- compile antes (binario velho mostra o passado)`); return 2; }
    log('binario:', b, statSync(b).mtime.toISOString());
  }
  const srv = await subir({ phxsqld, portaDados: PORTA_DADOS, portaWeb: PORTA_WEB, log });
  const pasta = join(srv.base, BANCO);
  const navegador = await chromium.launch();
  const ctx = await navegador.newContext({
    viewport: { width: LARG, height: ALT }, colorScheme: 'dark',
    recordVideo: { dir: SAIDA, size: { width: LARG, height: ALT } },
  });
  page = await ctx.newPage();
  const errosDePagina = [];
  page.on('pageerror', e => errosDePagina.push(e.message));
  let arquivosAntes = null;
  const rowidDo = {};          // nome -> rowid que a tela mostrou
  const quadroInicio = Date.now();

  try {
    await page.goto(srv.url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });

    // ---- 1. abertura com a marca ---------------------------------------
    await cena('1-abertura', async () => {
      await folha(`<div style="position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:6px">
        <img src="data:image/png;base64,${SIMBOLO}" style="width:250px">
        <div style="font-size:64px;font-weight:600;letter-spacing:.02em;color:#fff">PhxSql</div>
        <div style="font-size:16px;letter-spacing:.32em;color:#ff8a1c">PHOENIX DATABASE ENGINE</div>
        <div style="margin-top:22px;font-size:22px;color:#c9cfdc;font-style:italic">Built to store. Engineered to scale.</div>
        <div style="margin-top:34px;font-size:15px;color:#8a93ad">Um CRUD do começo ao fim — pela tela, contra o servidor de verdade, com a prova no disco</div>
      </div>`);
      await respirar(1200);
      await quadro('01-abertura');
      await respirar(2600);
      await semFolha();
    });

    // ---- 2. login ------------------------------------------------------
    await cena('2-login', async () => {
      await cartaz('1 · ENTRAR', 'Login de verdade: usuário e senha');
      await diz('ENTRAR', 'A senha não trafega: o servidor manda um desafio e a tela devolve a prova.', 600);
      await page.fill('#u', ''); await page.fill('#s', '');
      await page.locator('#u').pressSequentially(USUARIO, { delay: 90 }); await respirar(300);
      await page.locator('#s').pressSequentially(SENHA, { delay: 70 });   await respirar(300);
      await page.fill('#t', TOKEN); await respirar(600);
      await quadro('02-login');
      await page.click('#btEntrar');
      await page.waitForSelector('#app.ativo', { timeout: 20000 });
      await page.waitForSelector('#arvore .no', { timeout: 20000 });
      await diz('ENTRAR', 'Dentro: a árvore dos bancos à esquerda, as ferramentas em cima.', 2200);
      await quadro('02-dentro');
    });

    // ---- 3. o database -------------------------------------------------
    await cena('3-database', async () => {
      await cartaz('2 · DATABASE', `Criar o banco «${BANCO}»`);
      await diz('CRIAR O BANCO', 'Pelo [+] da árvore. Um database é uma pasta no disco.', 1200);
      page.once('dialog', d => d.accept(BANCO));
      await page.click('#btNovoDb');
      await page.waitForFunction(n => [...document.querySelectorAll('#arvore .no.db')]
        .some(x => x.dataset.db === n), BANCO, { timeout: 15000 });
      await page.locator(`#arvore .no.db[data-db="${BANCO}"]`).first().click();
      await respirar(1600);
      await quadro('03-database');
    });

    // ---- 4. a tabela ---------------------------------------------------
    await cena('4-tabela', async () => {
      await cartaz('3 · TABELA', `Criar a tabela «${TABELA}»: tipos, chave primária e índice`);
      await toolbar('Tabelas').click();
      await page.waitForSelector('#btNovaTab', { timeout: 10000 });
      await respirar(900);
      await page.click('#btNovaTab');
      await page.waitForSelector('#nt_nome');
      await diz('CRIAR A TABELA', 'O cadastro já nasce com o id (Sequence, o motor numera) e a chave primária porId.', 1400);
      await page.locator('#nt_nome').pressSequentially(TABELA, { delay: 80 });
      const linha = n => page.locator(`#nt_cols tr:nth-child(${n})`);
      await linha(2).locator('.c-caption').fill('Nome do cliente');
      // As colunas que faltam: (nome, tipo, rotulo)
      const novas = [['cidade', 'Str(60)', 'Cidade'], ['limite', 'Decimal(15,2)', 'Limite de crédito'],
                     ['nascimento', 'Date', 'Nascimento'], ['ativo', 'Bool', 'Ativo']];
      for (let i = 0; i < novas.length; i++) {
        await page.click('#nt_addCol'); await respirar(250);
        const l = linha(3 + i);
        await l.locator('.c-nome').fill(novas[i][0]);
        await l.locator('.c-tipo').selectOption(novas[i][1]); await respirar(200);
        await linha(3 + i).locator('.c-caption').fill(novas[i][2]);
        await respirar(250);
      }
      await linha(2).locator('.c-obrig').check();
      await page.evaluate(() => document.querySelector('#nt_cols').scrollIntoView({ block: 'center' }));
      await diz('CRIAR A TABELA', 'Seis colunas de cinco tipos: Sequence, texto, decimal exato, data e lógico.', 1600);
      await quadro('04-campos');
      // O indice por cidade.
      await page.click('#nt_addIdx'); await respirar(300);
      const idx = page.locator('#nt_idxs tr:nth-child(2)');
      await idx.locator('.i-nome').fill('porCidade');
      await idx.locator('.i-cols').pressSequentially('cidade', { delay: 70 });
      await respirar(400);
      await page.evaluate(() => document.querySelector('#nt_idxs').scrollIntoView({ block: 'center' }));
      await diz('CRIAR A TABELA', 'E um índice porCidade, para achar por cidade sem varrer a tabela.', 1800);
      await quadro('04-indices');
      await page.click('#nt_criar');
      await page.waitForSelector('#gradeGerirTabs', { timeout: 15000 });
      await diz('CRIAR A TABELA', 'Criada. Os arquivos dela nasceram juntos no disco.', 1800);
      await quadro('04-criada');
    });

    // ---- 4b. os arquivos recem-nascidos --------------------------------
    await cena('4b-disco-nascimento', async () => {
      arquivosAntes = arquivosDe(pasta);
      const reg = lerReg(join(pasta, `${TABELA}.reg`));
      log('pasta:', pasta, arquivosAntes.map(a => `${a.nome}=${a.bytes}`).join(' '));
      await folha(`<div style="font-size:12px;letter-spacing:.2em;color:#8a93ad">LISTAGEM REAL DO DIRETÓRIO — lida do disco por readdirSync/statSync</div>
        <div style="font-size:20px;color:#ff8a1c;margin:4px 0 10px;font-family:ui-monospace,monospace">${esc(pasta)}</div>
        ${tabelaDeArquivos(arquivosAntes)}
        <div style="margin-top:16px;font-size:13px;color:#8a93ad">O começo do <b style="color:#ffb27a">${TABELA}.reg</b>: a assinatura <b style="color:#ffd27a">${esc(reg.assinatura)}</b>,
          slot de ${reg.slotSize} bytes, ${reg.slotCount} slots usados, dados a partir do byte ${reg.dataOffset}.</div>
        <div style="margin-top:6px">${hexDump(reg.b, 0, 64, [[0, 6, '#ffd27a'], [16, 20, '#7ab8ff'], [20, 28, '#5fe08a'], [44, 52, '#c99bff']])}</div>
        <div style="position:absolute;bottom:80px;left:40px;font-size:11px;color:#5d6680">este contêiner não tem ambiente gráfico: a listagem é a real, apresentada aqui em vez de num gerenciador de arquivos</div>`);
      await diz('NO DISCO', 'Tabela vazia: o .reg só tem cabeçalho e esquema. Azul = tamanho do slot, verde = slots usados (zero), roxo = onde começam os dados.', 3000);
      await quadro('04b-disco-nascimento');
      await respirar(2600);
      await semFolha();
    });

    // ---- 5. incluir ----------------------------------------------------
    await cena('5-incluir', async () => {
      await cartaz('4 · INCLUIR', 'Incluir clientes pela ficha');
      await abrirConteudo();
      await diz('INCLUIR', '«Nova linha» abre a ficha. O id fica em branco: quem numera é o motor.', 600);
      await incluir(CLIENTES[0], true);
      await diz('INCLUIR', 'Gravada. O nº é a ordem de digitação; o rowid, a posição física no .reg.', 1400);
      for (const c of CLIENTES.slice(1)) await incluir(c, false);
      await verGrade();
      await diz('INCLUIR', 'Cinco clientes, na ordem em que foram digitados.', 2200);
      await quadro('05-cinco');
      for (const c of CLIENTES) {
        rowidDo[c.nome] = await page.evaluate(n => {
          const tr = [...document.querySelectorAll('#gradeEdit tbody tr')]
            .find(t => [...t.querySelectorAll('td')].some(td => td.textContent.trim() === n));
          const b = tr && tr.querySelector('.bt-editar');
          return b ? +b.dataset.rowid : null;
        }, c.nome);
      }
      log('rowids:', JSON.stringify(rowidDo));
    });

    // ---- 6. consultar --------------------------------------------------
    await cena('6-consultar', async () => {
      await cartaz('5 · CONSULTAR', 'A lista na PhxGrid, e uma pesquisa');
      await diz('CONSULTAR', 'A grade ordena, filtra, agrupa e pesquisa sobre a página trazida do servidor.', 2000);
      const busca = page.locator('#gradeEdit .phx-busca-in').first();
      await busca.click();
      await busca.pressSequentially('Blumenau', { delay: 90 });
      await verGrade();
      await respirar(500);
      // «Blumenau» tem de aparecer como esta gravado -- rotulo se estiliza, dado nunca.
      const transf = await page.evaluate(() => {
        const td = [...document.querySelectorAll('#gradeEdit tbody td')].find(t => t.textContent.trim() === 'Blumenau');
        return td ? { visto: td.innerText, tt: getComputedStyle(td).textTransform } : null;
      });
      log('dado na grade:', JSON.stringify(transf));
      await diz('CONSULTAR', 'Pesquisa «Blumenau»: duas clientes. O dado aparece como foi gravado.', 2600);
      await quadro('06-pesquisa');
      await busca.fill('');
      await respirar(700);
    });

    // ---- 7. alterar ----------------------------------------------------
    await cena('7-alterar', async () => {
      await cartaz('6 · ALTERAR', 'Mudar a cidade e o limite de um cliente');
      const r = rowidDo[CLIENTES[1].nome] || ALTERADO.rowid;
      await abrirFichaDe(r);
      await diz('ALTERAR · ANTES', `${CLIENTES[1].nome}: ${ALTERADO.antes.cidade}, limite ${ALTERADO.antes.limite}.`, 2200);
      await quadro('07-antes');
      await page.locator('#f_cidade').fill('');
      await page.locator('#f_cidade').pressSequentially(ALTERADO.depois.cidade, { delay: 70 });
      await page.locator('#f_limite').fill('');
      await page.locator('#f_limite').pressSequentially(ALTERADO.depois.limite, { delay: 70 });
      await respirar(700);
      await page.locator('#btSalvar').click();
      await page.waitForSelector('#btNova', { timeout: 10000 });
      await page.waitForFunction(v => [...document.querySelectorAll('#gradeEdit td')]
        .some(td => td.textContent.trim() === v), ALTERADO.depois.cidade, { timeout: 10000 });
      await verGrade();
      await diz('ALTERAR · DEPOIS', `Agora ${ALTERADO.depois.cidade}, limite ${ALTERADO.depois.limite}. O nº de ordem não mudou: alterar não renumera.`, 2800);
      await quadro('07-depois');
    });

    // ---- 8. excluir ----------------------------------------------------
    await cena('8a-excluir-suave', async () => {
      await cartaz('7 · EXCLUIR', 'As duas exclusões: a que volta e a de vez');
      await abrirFichaDe(rowidDo[CLIENTES[2].nome] || SUAVE);
      await page.locator('#btExcluir').click();
      await page.waitForSelector('#btExcSim');
      await diz('EXCLUIR · MARCAR', 'O padrão é o reversível: a linha some das listas e continua inteira no .reg.', 1800);
      await page.locator('#excMotivo').pressSequentially('cliente pediu pausa no cadastro', { delay: 35 });
      await respirar(500);
      await quadro('08-dialogo-suave');
      await page.locator('#btExcSim').click();
      await page.waitForSelector('#btNova', { timeout: 10000 });
      await respirar(900);
      await page.click('#vwExcl');
      await page.waitForSelector('#gradeEdit .restaurar', { timeout: 10000 });
      await verGrade();
      await diz('EXCLUIR · MARCAR', 'Na visão «excluídas» ela está lá, com o botão de restaurar.', 2400);
      await quadro('08-excluidas');
      await page.click('#vwAtivas');
      await page.waitForSelector('#btNova');
      await respirar(500);
    });

    await cena('8b-excluir-de-vez', async () => {
      await abrirFichaDe(rowidDo[CLIENTES[4].nome] || FISICO);
      await page.locator('#btExcluir').click();
      await page.waitForSelector('#btExcSim');
      await page.locator('.modo[data-modo="fisico"]').click();
      await diz('EXCLUIR DE VEZ', 'A outra: sai do .reg. A linha inteira vai antes para o .trash, e o slot não volta a ser usado.', 1800);
      await page.locator('#excMotivo').pressSequentially('cadastro duplicado', { delay: 35 });
      await respirar(500);
      await quadro('08-dialogo-de-vez');
      // A confirmacao a mais e um confirm() nativo: o video nao o filma.
      page.once('dialog', d => { log('   confirm():', d.message().split('\n')[0]); d.accept(); });
      await page.locator('#btExcSim').click();
      await page.waitForSelector('#btNova', { timeout: 10000 });
      await diz('EXCLUIR DE VEZ', 'Confirmada (a pergunta extra é uma caixa do navegador). Ela saiu da lista.', 2000);
    });

    await cena('8c-lixeira', async () => {
      await toolbar('Lixeira').click();
      await page.waitForSelector('#btVoltaLix', { timeout: 10000 });
      await diz('LIXEIRA', 'A lixeira da tabela: a linha excluída de vez está aqui, inteira. Só quem administra lê.', 3000);
      await quadro('08-lixeira');
    });

    await cena('8d-incluir-depois', async () => {
      await abrirConteudo();
      await incluir(SEXTO, false);
      rowidDo[SEXTO.nome] = await page.evaluate(n => {
        const tr = [...document.querySelectorAll('#gradeEdit tbody tr')]
          .find(t => [...t.querySelectorAll('td')].some(td => td.textContent.trim() === n));
        const b = tr && tr.querySelector('.bt-editar');
        return b ? +b.dataset.rowid : null;
      }, SEXTO.nome);
      await verGrade();
      await diz('ORDEM DE DIGITAÇÃO', `Um cliente novo depois da exclusão: rowid ${rowidDo[SEXTO.nome]}. O slot ${FISICO} não foi reaproveitado.`, 2800);
      await quadro('08-sexto');
    });

    // ---- 9. consulta: o que a tela tem -----------------------------------
    await cena('9-consulta', async () => {
      await cartaz('8 · CONSULTA', 'Consulta com filtro pela tela');
      await diz('CONSULTA', 'A tela não tem console SQL próprio: o menu «Consulta SQL» abre o SelectMemory.', 2600);
      await page.evaluate(() => carregarNaMemoria());   // o item «Carregar esta tabela na RAM» do menu
      await respirar(1200);
      await toolbar('Query').click();
      await page.waitForSelector('#btConsultar', { timeout: 10000 });
      await page.locator('#cCol').fill('cidade');
      await page.selectOption('#cOp', 'igual');
      await page.locator('#cVal').pressSequentially('Blumenau', { delay: 70 });
      await respirar(500);
      await page.click('#btConsultar');
      await page.waitForSelector('#gradeConsulta, #saidaConsulta .vazio, #saidaConsulta .nota', { timeout: 10000 });
      await verGrade('#saidaConsulta');
      await diz('CONSULTA', 'cidade = Blumenau, na RAM. Sem ORDER BY: o SQL de verdade só existe pelo protocolo e pelo phxsqlcmd.', 3200);
      await quadro('09-consulta');
    });

    // ---- 10. a prova no disco, com o servidor PARADO ---------------------
    await cena('10-prova-no-disco', async () => {
      await cartaz('9 · NO DISCO', 'Deu certo? Quem responde é o disco, com o servidor parado', 2000);
      process.kill(srv.pid, 'SIGTERM');
      for (let i = 0; i < 300 && srv.morreuCom() === null; i++) await new Promise(r => setTimeout(r, 100));
      log('servidor parado, codigo', srv.morreuCom());

      const depois = arquivosDe(pasta);
      const info = cli(['info', pasta, TABELA]);
      const listar = cli(['listar', pasta, TABELA, '--max', '0']);
      const diario = cli(['log', pasta, TABELA, '--max', '0']);
      const verif = cli(['verificar', pasta, TABELA]);
      for (const [n, r] of [['info', info], ['listar', listar], ['log', diario], ['verificar', verif]])
        log(`--- phxsql ${n} (ok=${r.ok})\n${r.saida.trim()}`);
      const reg = lerReg(join(pasta, `${TABELA}.reg`));
      const trash = existsSync(join(pasta, `${TABELA}.trash`)) ? readFileSync(join(pasta, `${TABELA}.trash`)) : Buffer.alloc(0);
      const slot = r => reg.slots[r - 1];
      const rid = n => rowidDo[n];

      // As provas, uma por operacao. `null` = nao se prova pelo disco (e diz por que).
      const provas = [];
      const prova = (op, ok, como) => { provas.push({ op, ok, como }); log(`PROVA ${op}: ${ok === null ? 'NAO PROVAVEL' : ok ? 'CONFERE' : 'NAO CONFERE'} -- ${como}`); };

      const vivos = [CLIENTES[0], CLIENTES[1], CLIENTES[3], SEXTO];
      const incl = vivos.every(c => listar.saida.includes(c.nome)
        && slot(rid(c.nome)) && temTexto(reg.b, slot(rid(c.nome)).off, slot(rid(c.nome)).fim, c.nome) >= 0);
      prova('INCLUIR', incl, `os nomes digitados estão nos slots ${vivos.map(c => rid(c.nome)).join(', ')} do .reg e o «phxsql listar» os lê`);

      const sa = slot(rid(CLIENTES[1].nome));
      const novoNoDisco = sa && temTexto(reg.b, sa.off, sa.fim, ALTERADO.depois.cidade) >= 0
        && listar.saida.includes(ALTERADO.depois.cidade) && listar.saida.includes(ALTERADO.depois.limite.replace('.', ','))
          || (listar.saida.includes(ALTERADO.depois.cidade) && listar.saida.includes(ALTERADO.depois.limite));
      const velhoSumiu = sa && temTexto(reg.b, sa.off, sa.fim, ALTERADO.antes.cidade) < 0;
      prova('ALTERAR', !!(novoNoDisco && velhoSumiu && sa.versao >= 2),
        `slot ${sa && sa.rowid}: «${ALTERADO.depois.cidade}» gravado, «${ALTERADO.antes.cidade}» não está mais nele, versão ${sa && sa.versao}`);
      prova('VALOR ANTIGO', null,
        'o slot se regrava no lugar; o diário guarda a operação e a versão, não o valor. A imagem da linha só vai ao .log com replicacao.imagem_da_linha (FORMATO §4)');

      const ss = slot(rid(CLIENTES[2].nome));
      const suaveOk = ss && ss.status === 1 && ss.versao >= 2
        && temTexto(reg.b, ss.off, ss.fim, CLIENTES[2].nome) >= 0 && !listar.saida.includes(CLIENTES[2].nome);
      prova('EXCLUIR (marcar)', !!suaveOk,
        `slot ${ss && ss.rowid}: status ${ss && ss.status} (ativo), versão ${ss && ss.versao}, a linha inteira continua no .reg e o «listar» das ativas não a mostra`);

      const sf = slot(rid(CLIENTES[4].nome));
      const fisOk = sf && sf.status === 0 && !listar.saida.includes(CLIENTES[4].nome)
        && trash.indexOf(Buffer.from(CLIENTES[4].nome, 'utf8')) >= 0;
      prova('EXCLUIR (de vez)', !!fisOk,
        `slot ${sf && sf.rowid}: status ${sf && sf.status} (livre); «${CLIENTES[4].nome}» está no .trash (${trash.length} bytes)`);

      const ordemOk = reg.slotCount === 6 && rid(SEXTO.nome) === 6;
      prova('ORDEM DE DIGITAÇÃO', ordemOk,
        `${reg.slotCount} slots usados para 6 inclusões; o cliente incluído depois da exclusão ficou no slot ${rid(SEXTO.nome)}`);

      // O diario: uma entrada por operacao, na ordem feita.
      const evs = diario.saida.split('\n').map(l => l.trim().split(/\s{2,}/))
        .filter(c => c.length >= 4 && /^(inclusao|alteracao|exclusao)$/.test(c[1]))
        .map(c => ({ op: c[1], rowid: +c[2], versao: +c[3] }));
      const esperado = [1, 2, 3, 4, 5].map(r => ['inclusao', r]).concat(
        [['alteracao', rid(CLIENTES[1].nome)], ['?', rid(CLIENTES[2].nome)], ['exclusao', rid(CLIENTES[4].nome)], ['inclusao', 6]]);
      const logOk = evs.length === esperado.length && evs.every((e, i) => e.rowid === esperado[i][1]
        && (esperado[i][0] === '?' || e.op === esperado[i][0]));
      prova('DIÁRIO (.log)', logOk,
        `${evs.length} eventos: ${evs.map(e => `${e.op} ${e.rowid}`).join(' → ')} (a marca de exclusão entra como alteração: ela regrava o slot)`);
      // O phxsqld nao tem parada limpa: SIGTERM e queda (morre pelo padrao do
      // sinal). O `verificar` entao acha o .ndx atrasado -- e isso vai para a
      // tela como esta, vermelho, e nao escondido.
      const primeira = (verif.saida.split('\n').find(l => l.trim()) || '').replace(/\/tmp\/\S+\//, '');
      prova('INTEGRIDADE (ao parar)', verif.ok && /INTEGRA/.test(verif.saida),
        verif.ok ? '«phxsql verificar»: CRC de cada slot, página de índice e evento'
          : `«phxsql verificar» recusa logo depois de parar o servidor: ${primeira.slice(0, 150)}`);
      if (!verif.ok) {
        const rx = cli(['reindex', pasta, TABELA]);
        const v2 = cli(['verificar', pasta, TABELA]);
        log(`--- phxsql reindex (ok=${rx.ok})\n${rx.saida.trim()}\n--- phxsql verificar de novo (ok=${v2.ok})\n${v2.saida.trim()}`);
        prova('ÍNDICE REFEITO DO .reg', v2.ok && /INTEGRA/.test(v2.saida),
          `«phxsql reindex» reconstrói o .ndx a partir do .reg; o «verificar» seguinte: ${(v2.saida.split('\n')[0] || '').trim()}`);
      }

      // ---- a folha 1: os arquivos de novo
      await folha(`<div style="font-size:12px;letter-spacing:.2em;color:#8a93ad">DEPOIS DO CRUD — servidor PARADO, listagem real do diretório</div>
        <div style="font-size:20px;color:#ff8a1c;margin:4px 0 10px;font-family:ui-monospace,monospace">${esc(pasta)}</div>
        ${tabelaDeArquivos(depois, arquivosAntes)}
        <div style="margin-top:14px;color:#c9cfdc">Em verde, o que cresceu desde a criação. O <b>.reg</b> ganhou seis slots; o <b>.log</b>, um evento por operação;
          o <b>.trash</b> e o <b>.reason</b> guardam a exclusão de vez.</div>
        <div style="position:absolute;bottom:80px;left:40px;font-size:11px;color:#5d6680">este contêiner não tem ambiente gráfico: a listagem é a real, apresentada aqui em vez de num gerenciador de arquivos</div>`);
      await diz('NO DISCO · DEPOIS', 'O .reg cresceu seis slots, o .log tem um evento por operação, o .trash guarda a exclusão de vez.', 3600);
      await quadro('10-arquivos-depois');

      // ---- a folha 2: o hex do .reg com o texto digitado destacado
      const marcas = [];
      const realce = (r, s, cor) => { const x = slot(r); if (!x) return; const i = temTexto(reg.b, x.off, x.fim, s); if (i >= 0) marcas.push([i, i + Buffer.byteLength(s), cor]); };
      for (let r = 1; r <= reg.slotCount; r++) marcas.push([slot(r).off, slot(r).off + 1, slot(r).status ? '#5fe08a' : '#ff5f5f'], [slot(r).off + 8, slot(r).off + 9, '#7ab8ff']);
      realce(rid(CLIENTES[0].nome), CLIENTES[0].nome, '#ffd27a'); realce(rid(CLIENTES[0].nome), CLIENTES[0].cidade, '#ffb27a');
      realce(rid(CLIENTES[1].nome), ALTERADO.depois.cidade, '#ffb27a');
      const s1 = slot(1), s2 = slot(2);
      await folha(`<div style="font-size:12px;letter-spacing:.2em;color:#8a93ad">O ${TABELA}.reg CRU — slots 1 e 2, a partir do byte ${reg.dataOffset} (FORMATO §1)</div>
        <div style="display:flex;gap:26px;margin-top:8px;font-size:12px;color:#c9cfdc">
          <span><b style="background:#5fe08a;color:#010418">&nbsp;01&nbsp;</b> status ativo</span>
          <span><b style="background:#7ab8ff;color:#010418">&nbsp;02&nbsp;</b> versão (byte baixo)</span>
          <span><b style="background:#ffd27a;color:#010418">&nbsp;nome&nbsp;</b> e <b style="background:#ffb27a;color:#010418">&nbsp;cidade&nbsp;</b> digitados na tela</span></div>
        <div style="margin-top:10px">${hexDump(reg.b, s1.off, s1.fim, marcas)}</div>
        <div style="margin-top:8px">${hexDump(reg.b, s2.off, s2.fim, marcas)}</div>
        <div style="margin-top:8px;color:#c9cfdc">Slot ${s2.rowid}: versão ${s2.versao} — foi alterado; «${esc(ALTERADO.depois.cidade)}» está no lugar de «${esc(ALTERADO.antes.cidade)}».</div>`);
      await diz('NO DISCO · O .reg CRU', 'O que foi digitado na tela está nos bytes do arquivo — e o slot 2 já traz o valor alterado.', 4600);
      await quadro('10-hex');

      // ---- a folha 3: os selos
      const selo = p => p.ok === null
        ? '<b style="border:1.5px solid #8a93ad;color:#c9cfdc;padding:2px 9px;border-radius:4px">NÃO SE PROVA PELO DISCO</b>'
        : p.ok ? '<b style="border:1.5px solid #3ccf72;color:#5fe08a;padding:2px 9px;border-radius:4px">✔ CONFERIDO NO DISCO</b>'
          : '<b style="border:1.5px solid #ff4d4d;color:#ff7a7a;padding:2px 9px;border-radius:4px">✘ NÃO CONFERE</b>';
      await folha(`<div style="font-size:12px;letter-spacing:.2em;color:#8a93ad">A PROVA — o próprio motor lendo a pasta pela CLI (phxsql info · listar · log · verificar · reindex), servidor parado</div>
        <table style="border-collapse:collapse;margin-top:14px;font-size:14.5px">${provas.map(p => `<tr>
          <td style="padding:7px 18px 7px 0;color:#fff;font-weight:600;white-space:nowrap">${esc(p.op)}</td>
          <td style="padding:7px 18px 7px 0;white-space:nowrap">${selo(p)}</td>
          <td style="padding:7px 0;color:#c9cfdc">${esc(p.como)}</td></tr>`).join('')}</table>
        <div style="margin-top:16px;font-size:12px;color:#8a93ad">phxsql log, como saiu:</div>
        <pre style="margin:4px 0 0;font:11.5px/1.3 ui-monospace,monospace;color:#c9cfdc">${esc(diario.saida.trim().split('\n').slice(0, 12).join('\n'))}</pre>`);
      await semFaixa();
      await respirar(7500);
      await quadro('10-selos');
      const nao = provas.filter(p => p.ok === false).map(p => p.op);
      if (nao.length) log('!! PROVA NO DISCO NAO CONFERE em:', nao.join(', '));
    });

    // ---- 11. encerramento ------------------------------------------------
    await cena('11-encerramento', async () => {
      await semFaixa();
      await folha(`<div style="position:absolute;inset:0;display:flex;align-items:center;justify-content:center;gap:56px">
        <img src="data:image/png;base64,${SIMBOLO}" style="width:230px">
        <div><div style="font-size:13px;letter-spacing:.24em;color:#ff8a1c">O QUE SE VIU</div>
        <ol style="font-size:19px;line-height:1.6;color:#e8eaf2;margin:10px 0 0;padding-left:22px">
          <li>login de verdade, com desafio</li><li>um banco e uma tabela de seis colunas, chave primária e índice</li>
          <li>seis inclusões pela ficha, pesquisa na grade</li><li>uma alteração, com o antes e o depois</li>
          <li>exclusão que volta, exclusão de vez e a lixeira</li><li>a prova no disco, com o servidor parado</li></ol>
        <div style="margin-top:22px;font-size:30px;font-weight:600;color:#fff">PhxSql</div>
        <div style="font-size:18px;color:#c9cfdc;font-style:italic">Built to store. Engineered to scale.</div></div></div>`);
      await respirar(1000);
      await quadro('11-encerramento');
      await respirar(4000);
    });
    return falhas.length ? 1 : 0;
  } finally {
    log('duracao aproximada (s):', Math.round((Date.now() - quadroInicio) / 1000));
    log('erros de pagina:', errosDePagina.length ? errosDePagina : 'nenhum');
    log('cenas que falharam:', falhas.length ? falhas.join(', ') : 'nenhuma');
    await ctx.close();          // fecha ANTES do navegador: e o que grava o video
    await navegador.close();
    await srv.derrubar();
    for (const f of readdirSync(SAIDA).filter(f => f.endsWith('.webm')))
      log('video:', join(SAIDA, f), statSync(join(SAIDA, f)).size, 'bytes');
  }
}

principal().then(c => process.exit(c || 0));

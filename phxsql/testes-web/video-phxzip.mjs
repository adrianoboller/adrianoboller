/* O VIDEO da prova entre o 7-Zip e o PhxZip, pela porta web 4000.
 *
 *   cargo build --release -p phxzip-web
 *   node testes-web/video-phxzip.mjs
 *
 * O que ele filma, na ordem:
 *   A. porta 4000 SEM login
 *      1. o 7-Zip grava um .7z cifrado (nomes cifrados) -- o PhxZip abre,
 *         testa e extrai, e o sha256 do extraido bate com o original;
 *      2. o PhxZip compacta pela tela -- o 7-Zip testa e extrai, e o diff
 *         com os originais sai vazio;
 *      3. um .7z em BZip2 feito pelo 7-Zip: o PhxZip recusa pelo nome.
 *   B. a mesma porta 4000 COM usuario e senha
 *      4. senha errada recusada, senha certa entra, abre, e sai.
 *
 * A HONESTIDADE dos paineis de terminal: este conteiner nao tem ambiente
 * grafico para filmar um terminal. Os comandos do 7-Zip rodam DE VERDADE
 * enquanto o video grava (execFileSync), e o painel mostra a saida real
 * deles, capturada naquele instante -- nao um texto escrito antes.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, rmSync, writeFileSync, copyFileSync, readFileSync, existsSync, readdirSync } from 'node:fs';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import net from 'node:net';

const PORTA = 4000;
const URL_ = `http://localhost:${PORTA}/`;
const SAIDA = resolve(process.env.SAIDA || 'target/video-phxzip');
const AMOSTRA = join(SAIDA, 'amostra');
const TRABALHO = join(SAIDA, 'trabalho');
const SENHA_ARQ = 'Phoenix@2026';
const USUARIO = 'adriano';
const SENHA_LOGIN = 'porta-4000';
const BIN = resolve('target/release/phxzipweb');

const respirar = (p, ms = 900) => p.waitForTimeout(ms);
const sha = f => createHash('sha256').update(readFileSync(f)).digest('hex');

function sh(prog, args, opcoes = {}) {
  try {
    // stdin fechado: com o pipe aberto do node, o 7z le o fim da entrada como
    // Ctrl+C e para no meio da extracao («Break signaled») -- pago na 1a gravacao.
    return execFileSync(prog, args, { encoding: 'utf8', cwd: TRABALHO, stdio: ['ignore', 'pipe', 'pipe'], ...opcoes });
  } catch (e) {
    return (e.stdout || '') + (e.stderr || '');
  }
}

/** So as linhas que dizem algo -- o 7z imprime cabecalho e barra de progresso. */
function essencial(texto) {
  return texto.split('\n').map(l => l.replace(/\r/g, '').trimEnd())
    .filter(l => l && !/^(p7zip|7-Zip \(a\)|Scanning|Creating|Add new|Extracting archive|--|Path =|Type =|Physical|Headers|Solid|Blocks|Open archive|Testing archive|Listing archive|Items to|Files read|Archive size|Method =|\d+ file|Folders:|Files:|Size:|Compressed:)/.test(l)
      && !l.startsWith(' 64-bit'));
}

async function cartaz(page, numero, texto, sub = '') {
  await page.evaluate(([n, t, s]) => {
    let d = document.getElementById('__cartaz');
    if (!d) {
      d = document.createElement('div');
      d.id = '__cartaz';
      d.style.cssText = 'position:fixed;inset:0;z-index:99999;display:flex;align-items:center;'
        + 'justify-content:center;flex-direction:column;gap:14px;background:rgba(1,4,24,.94);'
        + 'color:#DDE2EB;font:600 36px/1.3 "Exo 2",system-ui,sans-serif;text-align:center;padding:40px;transition:opacity .3s';
      document.body.appendChild(d);
    }
    d.innerHTML = (n ? '<div style="font-size:15px;letter-spacing:.24em;color:#FF8A1C">' + n + '</div>' : '')
      + '<div>' + t + '</div>' + (s ? '<div style="font:400 19px system-ui;color:#8a93a8;max-width:900px">' + s + '</div>' : '');
    d.style.opacity = '1';
  }, [numero, texto, sub]);
  await respirar(page, 2600);
  await page.evaluate(() => { const d = document.getElementById('__cartaz'); if (d) { d.style.opacity = '0'; setTimeout(() => d.remove(), 350); } });
  await respirar(page, 450);
}

/** Painel de terminal com a saida REAL dos comandos, linha a linha. */
async function terminal(page, titulo, blocos, pausa = 3800) {
  await page.evaluate(([titulo, blocos]) => {
    const d = document.createElement('div');
    d.id = '__term';
    d.style.cssText = 'position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:1100px;max-height:760px;'
      + 'overflow:hidden;z-index:99998;background:#05070f;border:1px solid #2a3350;border-radius:12px;'
      + 'box-shadow:0 20px 80px rgba(0,0,0,.7);font:14px/1.55 ui-monospace,Menlo,Consolas,monospace;color:#c9d1e0';
    const barra = '<div style="padding:10px 16px;border-bottom:1px solid #1c2640;display:flex;gap:8px;align-items:center">'
      + '<span style="width:12px;height:12px;border-radius:50%;background:#ff5f57"></span>'
      + '<span style="width:12px;height:12px;border-radius:50%;background:#febc2e"></span>'
      + '<span style="width:12px;height:12px;border-radius:50%;background:#28c840"></span>'
      + '<span style="margin-left:12px;color:#8a93a8;font-family:system-ui">' + titulo + ' — saída real, capturada agora</span></div>';
    d.innerHTML = barra + '<pre id="__term_pre" style="margin:0;padding:16px 20px;white-space:pre-wrap"></pre>';
    document.body.appendChild(d);
    const pre = d.querySelector('pre');
    const esc = s => s.replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
    let atraso = 0;
    for (const [cmd, linhas] of blocos) {
      setTimeout(() => { pre.innerHTML += '<span style="color:#3ecf8e">$</span> <span style="color:#fff">' + esc(cmd) + '</span>\n'; }, atraso);
      atraso += 700;
      for (const l of linhas) {
        const cor = /Everything is Ok|IGUAL|OK$/.test(l) ? '#3ecf8e' : (/ERROR|Error|recus/i.test(l) ? '#ff5c5c' : '#c9d1e0');
        setTimeout(() => { pre.innerHTML += '<span style="color:' + cor + '">' + esc(l) + '</span>\n'; }, atraso);
        atraso += 90;
      }
      atraso += 350;
    }
  }, [titulo, blocos]);
  const n = blocos.reduce((s, [, l]) => s + 1050 + l.length * 90, 0);
  await respirar(page, n + pausa);
  await page.evaluate(() => document.getElementById('__term')?.remove());
  await respirar(page, 400);
}

/** Solta arquivos na zona com um DataTransfer de verdade: a tela ve o mesmo
 * evento `drop` de quem arrasta do gerenciador de arquivos, e a zona acende
 * antes, como acende para a mao. */
async function soltarNaZona(page, zona, arquivos) {
  const dados = arquivos.map(f => [f.split('/').pop(), [...readFileSync(f)]]);
  await page.evaluate(([zona, dados]) => {
    window.__dt = new DataTransfer();
    for (const [nome, bytes] of dados) window.__dt.items.add(new File([new Uint8Array(bytes)], nome));
    document.querySelector(zona).dispatchEvent(new DragEvent('dragover', { dataTransfer: window.__dt, bubbles: true, cancelable: true }));
  }, [zona, dados]);
  await respirar(page, 1100);
  await page.evaluate(zona => document.querySelector(zona).dispatchEvent(
    new DragEvent('drop', { dataTransfer: window.__dt, bubbles: true, cancelable: true })), zona);
  await respirar(page, 900);
  const n = await page.evaluate(() => fila.length);
  if (n !== arquivos.length) throw new Error(`a fila recebeu ${n} de ${arquivos.length} arquivos`);
}

/** Um destaque que aponta o elemento que a cena esta usando. */
async function apontar(page, seletor) {
  await page.evaluate(sel => {
    const el = document.querySelector(sel); if (!el) return;
    el.scrollIntoView({ block: 'center' });
    const r = el.getBoundingClientRect();
    const d = document.createElement('div');
    d.style.cssText = `position:fixed;left:${r.left - 6}px;top:${r.top - 6}px;width:${r.width + 12}px;height:${r.height + 12}px;`
      + 'border:2px solid #FFC43D;border-radius:10px;z-index:99990;pointer-events:none;transition:opacity .4s';
    document.body.appendChild(d);
    setTimeout(() => { d.style.opacity = '0'; setTimeout(() => d.remove(), 400); }, 1300);
  }, seletor);
  await respirar(page, 500);
}

function subirServidor(comLogin) {
  const args = ['--porta', String(PORTA)];
  const env = { ...process.env };
  if (comLogin) { args.push('--usuario', USUARIO); env.PHXZIP_WEB_SENHA = SENHA_LOGIN; }
  const p = spawn(BIN, args, { env, stdio: ['ignore', 'pipe', 'pipe'] });
  let saida = '';
  p.stdout.on('data', d => { saida += d; });
  p.stderr.on('data', d => { saida += d; });
  return new Promise((ok, falha) => {
    const t0 = Date.now();
    const olhar = () => {
      if (saida.includes('PhxZip web em')) return ok({ p, linha: saida.trim() });
      if (p.exitCode !== null || Date.now() - t0 > 8000) return falha(new Error('servidor nao subiu: ' + saida));
      setTimeout(olhar, 100);
    };
    olhar();
  });
}

function prepararAmostra() {
  rmSync(SAIDA, { recursive: true, force: true });
  mkdirSync(AMOSTRA, { recursive: true });
  mkdirSync(TRABALHO, { recursive: true });
  copyFileSync('docs/PENDENCIAS.md', join(AMOSTRA, 'PENDENCIAS.md'));
  copyFileSync('docs/PHXZIP.md', join(AMOSTRA, 'PHXZIP.md'));
  const png = readdirSync('marca').find(f => f.endsWith('.png'));
  if (png) copyFileSync(join('marca', png), join(AMOSTRA, png));
  writeFileSync(join(AMOSTRA, 'config.json'), JSON.stringify({ porta: 5433, web: 8433, replicacao: { papel: 'source' } }, null, 2) + '\n');
  writeFileSync(join(AMOSTRA, 'relatório-ação.txt'), 'Acentuação e cedilha: ação, coração, São Paulo, 日本語.\n'.repeat(40));
  return readdirSync(AMOSTRA).sort().map(f => join(AMOSTRA, f));
}

/* Os arquivos entram como BUFFER (nome + bytes lidos do disco), e nao como
 * caminho: medido na primeira gravacao, o `setInputFiles` por caminho
 * DESCARTA calado o arquivo de nome acentuado («relatório-ação.txt») -- o
 * .7z saiu com 4 de 5 e so o `diff` do 7-Zip acusou. E a contagem se confere
 * aqui, para o video nunca mais seguir com um arquivo a menos. */
async function escolherArquivo(page, seletor, arquivos) {
  const lista = [].concat(arquivos);
  await page.setInputFiles(seletor, lista.map(f => ({
    name: f.split('/').pop(), mimeType: 'application/octet-stream', buffer: readFileSync(f),
  })));
  // O input se esvazia depois de entregar (a fila e o arquivo aberto moram
  // no script da pagina): a contagem se confere la.
  const n = await page.evaluate(sel => sel === '#c_arquivos' ? fila.length : (aberto ? 1 : 0), seletor);
  if (n !== lista.length) throw new Error(`o navegador recebeu ${n} de ${lista.length} arquivos`);
  await respirar(page, 700);
}

async function baixarPor(page, acao, destino) {
  const [dl] = await Promise.all([page.waitForEvent('download'), acao()]);
  await dl.saveAs(destino);
  return destino;
}

function portaLivre(porta) {
  return new Promise(ok => {
    const s = net.createServer().once('error', () => ok(false)).once('listening', () => s.close(() => ok(true)));
    s.listen(porta, '127.0.0.1');
  });
}

async function principal() {
  if (!(await portaLivre(PORTA))) { console.error(`a porta ${PORTA} esta ocupada`); return 2; }
  if (!existsSync(BIN)) { console.error(`falta ${BIN} -- cargo build --release -p phxzip-web`); return 2; }
  const arquivos = prepararAmostra();
  const versao7z = sh('7z', ['i']).split('\n').find(l => l.startsWith('7-Zip')) || '7-Zip';

  let srv = await subirServidor(false);
  const navegador = await chromium.launch();
  const ctx = await navegador.newContext({
    viewport: { width: 1440, height: 900 }, acceptDownloads: true,
    recordVideo: { dir: SAIDA, size: { width: 1440, height: 900 } },
  });
  const page = await ctx.newPage();
  const log = [];
  try {
    await page.goto(URL_, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#principal:not(.oculto)');
    await cartaz(page, '', 'PhxZip × 7-Zip', 'prova de interoperabilidade nos dois sentidos, pela porta web '
      + `localhost:${PORTA}<br><br>${versao7z.split(':')[0]} · PhxZip 0.19 (Rust, zero dependências)`);

    // ---- A. sem login --------------------------------------------------
    await cartaz(page, 'PARTE A', `Porta ${PORTA} sem login`, srv.linha);

    // 1. 7-Zip grava, PhxZip abre
    await cartaz(page, 'CENA 1', 'O 7-Zip grava — o PhxZip abre', 'arquivo cifrado com AES-256 e nomes cifrados (-mhe=on)');
    const de7z = join(TRABALHO, 'feito-pelo-7zip.7z');
    const rel = arquivos.map(f => '../amostra/' + f.split('/').pop());
    const saida1 = sh('7z', ['a', '-mf=off', '-mx=9', '-mhe=on', `-p${SENHA_ARQ}`, 'feito-pelo-7zip.7z', ...rel]);
    const lista1 = sh('7z', ['l', '-pchute-errado', 'feito-pelo-7zip.7z']);
    await terminal(page, '7-Zip', [
      [`7z a -mx=9 -mhe=on -p******** feito-pelo-7zip.7z amostra/*`, essencial(saida1)],
      [`7z l -pchute-errado feito-pelo-7zip.7z   # sem a senha certa`, essencial(lista1).filter(l => !/^Listing/.test(l)).slice(0, 6)],
    ]);
    await apontar(page, '#a_zona');
    await escolherArquivo(page, '#a_arquivo', de7z);
    // abrir e automatico ao escolher: sem a senha, o PhxZip pede a senha
    await page.waitForSelector('#a_res.erro');
    await respirar(page, 1800);
    await apontar(page, '#a_senha');
    await page.type('#a_senha', SENHA_ARQ, { delay: 70 });
    await page.click('#abrir');
    await page.waitForSelector('#a_tabela:not(.oculto)');
    await respirar(page, 2200);
    await apontar(page, '#testar');
    await page.click('#testar');
    await page.waitForFunction(() => /Tudo certo/.test(document.getElementById('a_res').textContent));
    await respirar(page, 2200);
    const nomeExtrair = 'relatório-ação.txt';
    const botao = page.locator('#a_tabela tr', { hasText: nomeExtrair }).locator('button');
    await botao.scrollIntoViewIfNeeded();
    const extraido = await baixarPor(page, () => botao.click(), join(TRABALHO, 'extraido-pelo-phxzip.txt'));
    const iguais = sha(extraido) === sha(join(AMOSTRA, nomeExtrair));
    log.push(`cena 1: sha256 ${iguais ? 'IGUAL' : 'DIFERENTE'}`);
    await terminal(page, 'conferência', [
      [`sha256sum amostra/${nomeExtrair} extraido-pelo-phxzip.txt`,
        [`${sha(join(AMOSTRA, nomeExtrair))}  amostra/${nomeExtrair}`, `${sha(extraido)}  extraido-pelo-phxzip.txt`,
         iguais ? 'IGUAL: o PhxZip extraiu byte a byte o que o 7-Zip gravou' : 'DIFERENTE']],
    ]);

    // 2. PhxZip grava, 7-Zip abre
    await cartaz(page, 'CENA 2', 'O PhxZip grava — o 7-Zip abre', 'os arquivos soltos na área de arrastar; LZMA2 nível 9, AES-256, nomes cifrados');
    await apontar(page, '#c_zona');
    await soltarNaZona(page, '#c_zona', arquivos);
    await page.selectOption('#c_nivel', '9');
    await respirar(page, 500);
    await page.type('#c_senha', SENHA_ARQ, { delay: 70 });
    await page.fill('#c_nome', 'feito-pelo-phxzip.7z');
    await respirar(page, 600);
    await apontar(page, '#compactar');
    const dePhx = await baixarPor(page, () => page.click('#compactar'), join(TRABALHO, 'feito-pelo-phxzip.7z'));
    await page.waitForSelector('#c_res.ok');
    await respirar(page, 2600);
    const teste = sh('7z', ['t', `-p${SENHA_ARQ}`, 'feito-pelo-phxzip.7z']);
    const lista2 = sh('7z', ['l', '-slt', `-p${SENHA_ARQ}`, dePhx]);
    const metodo = (lista2.split('\n').find(l => l.startsWith('Method = ') && l.length > 9) || '').trim();
    rmSync(join(TRABALHO, 'x'), { recursive: true, force: true });
    sh('7z', ['x', `-p${SENHA_ARQ}`, `-o${join(TRABALHO, 'x')}`, dePhx]);
    const diff = sh('diff', ['-r', AMOSTRA, join(TRABALHO, 'x')]);
    const ok7z = /Everything is Ok/.test(teste);
    log.push(`cena 2: 7z t ${ok7z ? 'OK' : 'FALHOU'}, diff ${diff.trim() ? 'COM DIFERENCA' : 'vazio'}`);
    await terminal(page, '7-Zip', [
      [`7z l -pchute-errado feito-pelo-phxzip.7z   # sem a senha certa`, essencial(sh('7z', ['l', '-pchute-errado', 'feito-pelo-phxzip.7z'])).filter(l => !/^Listing/.test(l)).slice(0, 4)],
      [`7z t -p******** feito-pelo-phxzip.7z`, essencial(teste).concat(metodo ? [metodo] : [])],
      [`7z x -p******** feito-pelo-phxzip.7z -ox && diff -r amostra x`,
        diff.trim() ? essencial(diff) : ['(diff vazio) IGUAL: os 5 arquivos voltaram idênticos']],
    ], 4800);

    // 3. o que o PhxZip recusa
    await cartaz(page, 'CENA 3', 'O que o PhxZip recusa — pelo nome', 'formatos antigos (BZip2, Deflate, PPMd, BCJ) não têm código morto aqui');
    const bz = join(TRABALHO, 'antigo-bzip2.7z');
    const saida3 = sh('7z', ['a', '-m0=bzip2', 'antigo-bzip2.7z', '../amostra/config.json']);
    await terminal(page, '7-Zip', [[`7z a -m0=bzip2 antigo-bzip2.7z amostra/config.json`, essencial(saida3)]], 1500);
    await page.fill('#a_senha', '');
    await escolherArquivo(page, '#a_arquivo', bz);
    await page.waitForSelector('#a_tabela:not(.oculto)');
    await page.click('#testar');
    await page.waitForSelector('#a_res.erro');
    await apontar(page, '#a_res');
    await respirar(page, 3200);

    // ---- B. com login --------------------------------------------------
    srv.p.kill('SIGTERM');
    await new Promise(r => setTimeout(r, 400));
    srv = await subirServidor(true);
    await cartaz(page, 'PARTE B', `A mesma porta ${PORTA}, agora com usuário e senha`, srv.linha
      + '<br>a senha do login entra pela variável de ambiente e o servidor guarda só o hash PBKDF2');
    await page.goto(URL_, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#login:not(.oculto)');
    await respirar(page, 1200);
    await cartaz(page, 'CENA 4', 'Senha errada é recusada; a certa entra', '');
    await page.type('#l_usuario', USUARIO, { delay: 70 });
    await page.type('#l_senha', 'chute-errado', { delay: 60 });
    await page.click('#entrar');
    await page.waitForSelector('#l_res.erro');
    await respirar(page, 2000);
    await page.fill('#l_senha', '');
    await page.type('#l_senha', SENHA_LOGIN, { delay: 60 });
    await page.click('#entrar');
    await page.waitForSelector('#principal:not(.oculto)');
    await respirar(page, 1500);
    await escolherArquivo(page, '#a_arquivo', dePhx);
    await page.type('#a_senha', SENHA_ARQ, { delay: 50 });
    await page.click('#testar');
    await page.waitForFunction(() => /Tudo certo/.test(document.getElementById('a_res').textContent));
    await respirar(page, 2500);
    await apontar(page, '#sair');
    await page.click('#sair');
    await page.waitForSelector('#login:not(.oculto)');
    await respirar(page, 1500);

    await cartaz(page, 'RESULTADO', log.map(l => l.replace(/^cena \d: /, '')).join('<br>'),
      '7-Zip ⇄ PhxZip: os dois abrem o que o outro grava · porta localhost:4000 com ou sem login');
  } finally {
    await page.close();
    await ctx.close();
    await navegador.close();
    srv.p.kill('SIGTERM');
  }
  const webm = readdirSync(SAIDA).find(f => f.endsWith('.webm'));
  console.log(log.join('\n'));
  console.log('video bruto:', join(SAIDA, webm));
  return 0;
}

principal().then(c => process.exit(c)).catch(e => { console.error(e); process.exit(1); });

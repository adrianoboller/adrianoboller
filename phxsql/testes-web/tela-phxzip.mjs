// Exercicio da tela do PhxZip no navegador. Interface so se prova exercitando.
//   ./target/release/phxzipweb --porta 4711 --max-mib 4 &
//   node testes-web/tela-phxzip.mjs /pasta/das/capturas /caminho/arv.7z
// O arv.7z tem docs/leia.txt (1500), docs/sub/z.txt (3), img/foto.bin (9000)
// e a.txt (5) -- o `phxzipcmd a` monta em segundos.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync } from 'node:fs';
const [S, ARV] = process.argv.slice(2), U = 'http://127.0.0.1:4711/';
const b = await chromium.launch({ executablePath: process.env.CHROMIUM || undefined });
const res = [];
const ok = (n, c) => res.push((c ? 'OK   ' : 'FALHA') + ' ' + n);
const MIB = 1 << 20;

// Contraste WCAG entre duas cores CSS (rgb/rgba), calculado na pagina.
const contraste = (p, sel, prop, fundoSel) => p.evaluate(([sel, prop, fundoSel]) => {
  const rgb = c => c.match(/[\d.]+/g).slice(0, 3).map(Number);
  const lum = ([r, g, b]) => { const f = v => (v /= 255) <= .03928 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; return .2126 * f(r) + .7152 * f(g) + .0722 * f(b); };
  const a = lum(rgb(getComputedStyle(document.querySelector(sel))[prop]));
  const z = lum(rgb(getComputedStyle(document.querySelector(fundoSel)).backgroundColor));
  return (Math.max(a, z) + .05) / (Math.min(a, z) + .05);
}, [sel, prop, fundoSel]);

for (const w of [1200, 375]) {
  const ctx = await b.newContext({ viewport: { width: w, height: 900 }, acceptDownloads: true, colorScheme: 'dark' });
  const p = await ctx.newPage();
  await p.goto(U); await p.waitForSelector('#principal:not(.oculto)');
  // --- rodada anterior: dica, senha repetida, olho, conferencia, teto
  ok(`${w}: dica do nivel 5`, (await p.textContent('#c_dica')).includes('equilíbrio'));
  await p.selectOption('#c_nivel', '9');
  ok(`${w}: dica do nivel 9`, (await p.textContent('#c_dica')).includes('menor'));
  ok(`${w}: repeticao oculta sem senha`, await p.isHidden('#c_conf'));
  await p.fill('#c_senha', 'abc');
  ok(`${w}: repeticao aparece com senha`, await p.isVisible('#c_conf'));
  await p.click('#c_olho');
  ok(`${w}: olho mostra`, (await p.getAttribute('#c_senha', 'type')) === 'text' && (await p.getAttribute('#c_senha2', 'type')) === 'text');
  await p.click('#c_olho');
  await p.setInputFiles('#c_arquivos', [{ name: 'a.txt', mimeType: 'text/plain', buffer: Buffer.from('ola PhxZip\n'.repeat(500)) }]);
  await p.fill('#c_senha2', 'abd');
  let baixados = 0; p.on('download', () => { baixados++; });
  await p.click('#compactar'); await p.waitForTimeout(300);
  ok(`${w}: senha diferente recusa sem baixar`, (await p.textContent('#c_res')).includes('não conferem') && baixados === 0);
  await p.fill('#c_senha2', 'abc');
  await p.click('#compactar'); await p.waitForSelector('#c_res.ok');
  ok(`${w}: conferido apos compactar`, (await p.textContent('#c_res')).includes('conferido') && baixados === 1);
  ok(`${w}: sem rolagem lateral`, await p.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  const [a, c] = [await p.locator('#c_nivel').boundingBox(), await p.locator('#c_senha').boundingBox()];
  ok(`${w}: disposicao nivel/senha`, w < 520 ? c.y > a.y + a.height : Math.abs(a.y - c.y) < 2);

  // --- 2: progresso e cancelar. Envio estrangulado a 256 KiB/s para a
  // barra ter tempo de aparecer; 3 MiB levam ~12 s, e o Cancelar corta antes.
  await p.fill('#c_senha', ''); await p.dispatchEvent('#c_senha', 'input');
  await p.click('#c_limpar');
  await p.setInputFiles('#c_arquivos', [{ name: 'medio.bin', mimeType: 'application/octet-stream', buffer: Buffer.alloc(3 * MIB, 1) }]);
  const cdp = await ctx.newCDPSession(p);
  await cdp.send('Network.enable');
  await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: 256 * 1024 });
  const antes = baixados;
  await p.click('#compactar');
  await p.waitForSelector('#c_prog:not(.oculto)');
  await p.waitForTimeout(1500);
  const rot = await p.textContent('#c_prog .rotulo');
  const larg = await p.evaluate(() => parseFloat(getComputedStyle(document.querySelector('#c_prog .barra')).width));
  ok(`${w}: barra de envio com porcentagem (${rot.trim()})`, /Enviando… \d+%/.test(rot) && larg > 0);
  await p.click('#c_prog button');
  await p.waitForSelector('#c_res.erro');
  ok(`${w}: cancelar corta e nada baixa`, (await p.textContent('#c_res')).includes('Cancelado') && baixados === antes && await p.isHidden('#c_prog'));
  ok(`${w}: botao volta a funcionar`, !(await p.isDisabled('#compactar')));
  await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });

  // --- teto (porta de 4 MiB, fila de 5 MiB)
  await p.click('#c_limpar');
  await p.setInputFiles('#c_arquivos', [{ name: 'grande.bin', mimeType: 'application/octet-stream', buffer: Buffer.alloc(5 * MIB, 7) }]);
  ok(`${w}: aviso do teto na fila`, await p.isVisible('#c_teto'));
  let pedidos = 0; p.on('request', r => { if (r.url().includes('/api/compactar')) pedidos++; });
  await p.click('#compactar'); await p.waitForTimeout(300);
  ok(`${w}: recusa antes de enviar`, pedidos === 0 && (await p.textContent('#c_res')).includes('Nada foi enviado'));

  // --- 6: arvore e ordenacao
  await p.setInputFiles('#a_arquivo', ARV);
  await p.waitForSelector('#a_tabela:not(.oculto)');
  const nomes = () => p.$$eval('#a_tabela tbody td.nome', tds => tds.map(td => td.textContent.trim()));
  ok(`${w}: arvore com pastas antes (${(await nomes()).join(' ')})`, (await nomes()).join('|') === 'docs|sub|z.txt|leia.txt|img|foto.bin|a.txt');
  await p.click('#a_tabela th[data-ordem="tamanho"] button');
  ok(`${w}: ordena por tamanho, maior antes, pastas antes`, (await nomes()).join('|') === 'img|foto.bin|docs|sub|z.txt|leia.txt|a.txt'
    && await p.getAttribute('#a_tabela th[data-ordem="tamanho"]', 'aria-sort') === 'descending');
  await p.click('#a_tabela th[data-ordem="tamanho"] button');
  ok(`${w}: segundo clique inverte`, (await nomes()).join('|') === 'docs|sub|z.txt|leia.txt|img|foto.bin|a.txt'
    && await p.getAttribute('#a_tabela th[data-ordem="tamanho"]', 'aria-sort') === 'ascending');
  await p.click('#a_tabela th[data-ordem="nome"] button');
  await p.click('button.seta[aria-label$=" docs"]');
  ok(`${w}: recolher esconde os filhos`, (await nomes()).join('|') === 'docs|img|foto.bin|a.txt');
  await p.fill('#a_filtro', 'z.t');
  ok(`${w}: filtro abre o caminho do que casa`, (await nomes()).join('|') === 'docs|sub|z.txt');
  await p.fill('#a_filtro', '');
  await p.click('button.seta[aria-label$=" docs"]');

  // --- 4: extrair tudo, pelos dois caminhos
  await p.evaluate(() => { window.__showDirectoryPicker = window.showDirectoryPicker; delete window.showDirectoryPicker; });
  const n0 = baixados;
  await p.click('#tudo'); await p.waitForSelector('#a_res.ok');
  await p.waitForTimeout(800);
  ok(`${w}: extrair tudo baixa um a um sem API de pasta (${baixados - n0})`, baixados - n0 === 4 && (await p.textContent('#a_res')).includes('um a um'));
  // Pasta simulada em memoria: prova que a arvore sai inteira, com os bytes.
  await p.evaluate(() => {
    window.__gravados = {};
    const dir = caminho => ({ name: caminho || 'destino',
      getDirectoryHandle: async n => dir((caminho ? caminho + '/' : '') + n),
      getFileHandle: async n => ({ createWritable: async () => {
        const partes = [];
        return { write: async d => partes.push(new Uint8Array(d)), close: async () => { window.__gravados[(caminho ? caminho + '/' : '') + n] = partes.reduce((s, x) => s + x.length, 0); } };
      } }) });
    window.showDirectoryPicker = async () => dir('');
  });
  await p.click('#tudo'); await p.waitForFunction(() => document.querySelector('#a_res').textContent.includes('pasta'));
  const grav = await p.evaluate(() => window.__gravados);
  ok(`${w}: extrair tudo recria a arvore (${JSON.stringify(grav)})`, grav['docs/leia.txt'] === 1500 && grav['docs/sub/z.txt'] === 3 && grav['img/foto.bin'] === 9000 && grav['a.txt'] === 5);

  // --- 5: tema claro, contraste medido nos DOIS temas
  const medir = async () => {
    const m = {
      texto: await contraste(p, 'body', 'color', 'body'),
      fraco: await contraste(p, '.campo > span', 'color', '.cartao'),
      verde: await contraste(p, '#compactar', 'color', '.cartao'),
      azul: await contraste(p, '#abrir', 'color', '.cartao'),
      ambar: await contraste(p, '.selo', 'color', '.cartao'),
      erro: await contraste(p, '#c_res', 'color', '.cartao'),
    };
    return [Object.values(m).every(v => v >= 4.5), JSON.stringify(Object.fromEntries(Object.entries(m).map(([k, v]) => [k, +v.toFixed(2)])))];
  };
  ok(`${w}: sistema escuro abre escuro`, await p.getAttribute('html', 'data-tema') === 'escuro');
  const [okE, mE] = await medir();
  ok(`${w}: contraste >= 4,5 no escuro ${mE}`, okE);
  await p.click('#tema');
  ok(`${w}: tema claro aplicado`, await p.getAttribute('html', 'data-tema') === 'claro');
  const [okC, mC] = await medir();
  ok(`${w}: contraste >= 4,5 no claro ${mC}`, okC);
  await p.screenshot({ path: `${S}/zip-claro-${w}.png`, fullPage: true });
  await p.reload(); await p.waitForSelector('#principal:not(.oculto)');
  ok(`${w}: tema lembrado depois de recarregar`, await p.getAttribute('html', 'data-tema') === 'claro');
  await p.click('#tema');
  await p.screenshot({ path: `${S}/zip-${w}.png`, fullPage: true });
  await p.selectOption('#idioma', 'Alemao'); await p.waitForTimeout(300);
  ok(`${w}: alemao no botao novo`, (await p.textContent('#tudo')).includes('Alles'));
  await ctx.close();
}
// Sem escolha lembrada, o tema segue o sistema: contexto novo, sistema claro.
{
  const ctx = await b.newContext({ colorScheme: 'light' });
  const p = await ctx.newPage(); await p.goto(U);
  ok('sistema claro abre claro', await p.getAttribute('html', 'data-tema') === 'claro');
  await ctx.close();
}
await b.close();
console.log(res.join('\n'));
process.exit(res.some(r => r.startsWith('FALHA')) ? 1 : 0);

// Exercicio da tela do PhxZip no navegador (senha repetida, olho, dica do
// nivel, teto recusado ANTES de enviar, conferencia depois de compactar, 375
// e 1200 px, alemao). Interface so se prova exercitando.
//   ./target/release/phxzipweb --porta 4711 --max-mib 1 &
//   node testes-web/tela-phxzip.mjs /pasta/das/capturas
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
const S = process.argv[2], U = 'http://127.0.0.1:4711/';
const b = await chromium.launch({ executablePath: process.env.CHROMIUM || undefined });
const res = [];
const ok = (n, c) => { res.push((c ? 'OK   ' : 'FALHA') + ' ' + n); };
for (const w of [1200, 375]) {
  const p = await b.newPage({ viewport: { width: w, height: 900 }, acceptDownloads: true });
  await p.goto(U); await p.waitForSelector('#principal:not(.oculto)');
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
  let dl = false; p.on('download', () => { dl = true; });
  await p.click('#compactar'); await p.waitForTimeout(300);
  ok(`${w}: senha diferente recusa sem baixar`, (await p.textContent('#c_res')).includes('não conferem') && !dl);
  await p.fill('#c_senha2', 'abc');
  const d = p.waitForEvent('download');
  await p.click('#compactar'); await d;
  await p.waitForSelector('#c_res.ok');
  ok(`${w}: conferido apos compactar`, (await p.textContent('#c_res')).includes('conferido'));
  ok(`${w}: sem rolagem lateral`, await p.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  // Nivel e senha: a 375 um embaixo do outro; a 1200 lado a lado e alinhados pelo topo.
  const [a, c] = [await p.locator('#c_nivel').boundingBox(), await p.locator('#c_senha').boundingBox()];
  ok(`${w}: disposicao nivel/senha`, w < 520 ? c.y > a.y + a.height : Math.abs(a.y - c.y) < 2);
  // teto: 2 MiB de fila numa porta de 1 MiB
  await p.setInputFiles('#c_arquivos', [{ name: 'grande.bin', mimeType: 'application/octet-stream', buffer: Buffer.alloc(2 << 20, 7) }]);
  ok(`${w}: aviso do teto na fila`, await p.isVisible('#c_teto'));
  let pedidos = 0; p.on('request', r => { if (r.url().includes('/api/compactar')) pedidos++; });
  await p.click('#compactar'); await p.waitForTimeout(300);
  ok(`${w}: recusa antes de enviar`, pedidos === 0 && (await p.textContent('#c_res')).includes('Nada foi enviado'));
  await p.screenshot({ path: `${S}/zip-${w}.png`, fullPage: true });
  await p.selectOption('#idioma', 'Alemao');
  await p.waitForTimeout(300);
  ok(`${w}: alemao na dica`, (await p.textContent('#c_dica')).includes('kleinste'));
  await p.close();
}
await b.close();
console.log(res.join('\n'));

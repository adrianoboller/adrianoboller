import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1100, height: 720 }, deviceScaleFactor: 1.5 });
const log = [];
await p.goto('http://localhost:4173/'); await p.waitForSelector('table');
log.push('abriu a tela: ' + await p.locator('tbody tr').count() + ' linha(s) vindas da API');
await p.screenshot({ path: '/tmp/tela-1.png' });
// BR-003 pela tela: nome curto -> a MESMA mensagem do PHP
await p.fill('input >> nth=0', 'Jo'); await p.fill('input >> nth=1', 'jo@x.com'); await p.fill('input >> nth=2', '10');
await p.click('button.incluir'); await p.waitForSelector('.erro');
log.push('erro mostrado: ' + await p.locator('.erro').innerText());
await p.screenshot({ path: '/tmp/tela-2.png' });
// incluir de verdade
await p.fill('input >> nth=0', 'Ana Lima'); await p.fill('input >> nth=1', 'ANA@Loja.com'); await p.fill('input >> nth=2', '250.005');
await p.click('button.incluir'); await p.waitForFunction(() => document.querySelectorAll('tbody tr').length >= 2);
log.push('incluiu: ' + (await p.locator('tbody tr').allInnerTexts()).map(t => t.replace(/\s+/g, ' ')).join(' | '));
// mostrar desativados
await p.click('.filtro input'); await p.waitForFunction(() => document.querySelectorAll('tbody tr').length >= 3);
log.push('com desativados: ' + await p.locator('tbody tr').count() + ' linhas, ' + await p.locator('.tag').count() + ' marcada(s) como desativada');
await p.screenshot({ path: '/tmp/tela-3.png' });
console.log(log.join('\n')); await b.close();

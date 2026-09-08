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
// O defeito que so apareceu OLHANDO: a celula de acoes tinha display:flex direto
// no <td>, deixava de ser celula de tabela, e a borda da linha parava antes
// dela. Medido pela causa: todo <td> do corpo continua table-cell. A primeira
// medida (botao dentro do retangulo do td) passava COM o defeito: nao media.
const fora = await p.evaluate(() => [...document.querySelectorAll('tbody td')]
  .filter(td => getComputedStyle(td).display !== 'table-cell').length);
const total = await p.locator('tbody td').count();
log.push(`células que continuam célula de tabela: ${total - fora}/${total}`);
console.log(log.join('\n')); await b.close();
if (fora) { console.error(`${fora} célula(s) deixaram de ser table-cell: a borda da linha para antes delas`); process.exit(1); }

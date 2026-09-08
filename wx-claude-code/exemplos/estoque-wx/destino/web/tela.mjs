// Prova da tela WIN_Venda convertida, pelos QUATRO estados que o PDF de interfaces (p.1) descreve:
// vazia (Fechar desabilitado), com itens, erro de estoque, acima do limite de credito (Sim/Nao).
// Os numeros sao os do screenshot do legado (win-venda-com-itens.png): 200 PAR + 12 CHA + 6 TIN, 10 %.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1100, height: 760 }, deviceScaleFactor: 1.5 });
const log = []; const falhas = [];
const confere = (nome, obtido, esperado) => { const ok = obtido === esperado; log.push(`${ok ? 'ok ' : 'X  '} ${nome}: ${obtido}${ok ? '' : ' (esperado ' + esperado + ')'}`); if (!ok) falhas.push(nome); };
const val = (sel) => p.locator(sel).inputValue();
await p.goto('http://localhost:4174/'); await p.waitForFunction(() => document.querySelectorAll('select')[0]?.options.length > 1 && document.querySelectorAll('select')[2]?.options.length > 1);
// 1 · vazia
confere('estado vazia: botão Fechar desabilitado', await p.locator('button.fechar').isDisabled(), true);
await p.screenshot({ path: '/tmp/venda-1-vazia.png' });
// 2 · com itens, desconto 10 %
await p.selectOption('select >> nth=0', { label: 'Maria Aparecida Souza' });
confere('STC_Cliente mostra tipo e limite', await p.locator('#stc-cliente').innerText(), 'Tipo: Comum · Limite de crédito: 5.000,00');
async function adiciona(codigo, qtd) { await p.selectOption('select >> nth=2', { label: (await p.locator('select >> nth=2 >> option').allInnerTexts()).find(t => t.startsWith(codigo)) }); await p.fill('#qtd', qtd); await p.click('button.incluir'); }
await adiciona('PAR-0021', '200'); await adiciona('CHA-0007', '12'); await adiciona('TIN-0130', '6');
await p.waitForFunction(() => document.querySelectorAll('tbody tr').length === 3);
await p.fill('#perc', '10'); await p.locator('#perc').blur(); await p.waitForFunction(() => document.querySelector('#total').value === '2.244,42');
confere('subtotal', await val('label:has-text("Subtotal") input'), '2.493,80');
confere('desconto 10 % (BR-001)', await val('label:has-text("Desconto") >> nth=1 >> input'), '249,38');
confere('total', await val('#total'), '2.244,42');
await p.screenshot({ path: '/tmp/venda-2-itens.png' });
// 3 · erro de estoque (BR-002): 50 chapas no depósito Matriz, que tem 40
await adiciona('CHA-0007', '50'); await p.waitForSelector('.erro');
confere('erro de estoque com a mensagem do legado', await p.locator('.erro').innerText(), 'Estoque insuficiente. Disponível: 40.000');
confere('foco volta para a quantidade (ReturnToCapture)', await p.evaluate(() => document.activeElement?.id), 'qtd');
await p.screenshot({ path: '/tmp/venda-3-erro.png' });
// desconto acima do teto: zera e avisa (RecalculaTotais)
await p.fill('#perc', '20'); await p.locator('#perc').blur(); await p.waitForSelector('.aviso');
confere('20 % para cliente comum: aviso', (await p.locator('.aviso').innerText()).startsWith('Desconto de 20% ultrapassa o máximo de 15%'), true);
confere('e o percentual zera', await val('#perc'), '0');
await p.fill('#perc', '10'); await p.locator('#perc').blur(); await p.waitForFunction(() => document.querySelector('#total').value === '2.244,42');
// 4 · fechar em 3 parcelas (BR-004, BR-006)
await p.selectOption('label:has-text("Parcelas") select', '3'); await p.click('button.fechar'); await p.waitForSelector('.ok');
confere('venda fechada com as três parcelas iguais', /^Venda \d+ fechada\. Parcelas: 748,14, 748,14, 748,14$/.test(await p.locator('.ok').innerText()), true);
// 5 · acima do limite de crédito (BR-007): só cliente comum pergunta
await adiciona('CHA-0007', '28'); await adiciona('TIN-0130', '24'); await p.waitForFunction(() => document.querySelectorAll('tbody tr').length === 2);
await p.click('button.fechar'); await p.waitForSelector('#confirma');
confere('acima do limite: pergunta Sim/Não', (await p.locator('#confirma').innerText()).startsWith('Total acima do limite de crédito do cliente. Fechar mesmo assim?'), true);
await p.screenshot({ path: '/tmp/venda-4-limite.png' });
await p.click('#confirma button.fechar'); await p.waitForSelector('.ok');
confere('confirmou: fechada', /fechada/.test(await p.locator('.ok').innerText()), true);
// 6 · o estoque baixou de verdade (BaixaEstoque): chapa 40 → 12 → 0 na Matriz
const r = await fetch('http://localhost:8081/estoque/valida', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ idproduto: 7, iddeposito: 1, quantidade: 1 }) });
confere('saldo da chapa na Matriz depois das duas vendas', (await r.json()).erro, 'Estoque insuficiente. Disponível: 0.000');
// QRY-001 — QRY_VendasPeriodo pela API: as 3 vendas de agosto da amostra, decrescente por data e id
const v = await (await fetch('http://localhost:8081/vendas?ini=2026-08-01&fim=2026-08-31')).json();
confere('QRY_VendasPeriodo: vendas de agosto, decrescente', v.map(x => x.idvenda).join(','), '1003,1002,1001');
confere('  e o filtro por vendedor vazio = todos', (await (await fetch('http://localhost:8081/vendas?ini=2026-08-01&fim=2026-08-31&vendedor=ninguem')).json()).length, 0);
// 7 · a lição do primeiro projeto: toda célula continua célula de tabela
confere('células que continuam table-cell', await p.evaluate(() => [...document.querySelectorAll('tbody td')].filter(td => getComputedStyle(td).display !== 'table-cell').length), 0);
console.log(log.join('\n')); console.log(`${log.length - falhas.length}/${log.length} conferências`); await b.close();
if (falhas.length) process.exit(1);

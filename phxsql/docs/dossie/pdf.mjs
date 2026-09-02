/* Gera o PDF de uma pagina de relatorio, no tema pedido.
 *
 *   node pdf.mjs <html> <claro|escuro> <saida.pdf>
 *
 * O CSS de impressao entra AQUI e nao no artefato: o que se publica e a
 * pagina, e o que se imprime tem uma restricao que a tela nao tem -- a
 * largura. O min-width do diagrama existe para ele nao espremer no celular; na
 * A4 (794 px a 96 dpi) essa mesma regra CORTA o desenho dentro do overflow.
 * Em papel nao ha barra de rolagem: o que transborda some, e sumir calado e o
 * pior jeito de errar.
 *
 * E a saida e APAGADA antes de gerar. Aprendizado de hoje: um gerador que
 * falha deixando a saida anterior em disco faz o conferidor seguinte ler o
 * cadaver e dizer «ok» -- foi assim que eu quase entreguei um PDF velho
 * afirmando que os consertos estavam nele.
 *
 * Sem crase neste comentario, de proposito: ela e metacaractere em mais de um
 * lugar, e ja quebrou um template literal e uma mensagem de commit hoje. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { rmSync, existsSync } from 'node:fs';

const [fonte, tema, saida] = process.argv.slice(2);
if (!fonte || !tema || !saida) {
  console.error('uso: node pdf.mjs <html> <claro|escuro> <saida.pdf>');
  process.exit(2);
}

const PARA_PAPEL = `
  @page { size: A4; margin: 0; }
  html, body { background: var(--tinta) !important; }
  .folha { padding: 15mm 13mm 12mm !important; max-width: none !important; }
  .moldura { overflow: visible !important; }
  svg.fluxo { min-width: 0 !important; }
  .rolo { overflow: visible !important; }
  figure, .cartao, .aviso, ol.passos li, table, pre { break-inside: avoid; }
  h2 { break-after: avoid; }
  .rodape { margin-top: 34px !important; }
  body { font-size: 13.5px; }
  h1 { font-size: 34px; }
`;

rmSync(saida, { force: true });

const nav = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium' });
try {
  const ctx = await nav.newContext({
    viewport: { width: 1240, height: 1600 },
    colorScheme: tema === 'escuro' ? 'dark' : 'light',
  });
  const page = await ctx.newPage();
  await page.goto(`file://${fonte}`, { waitUntil: 'networkidle' });
  await page.evaluate(t => document.documentElement.setAttribute('data-theme', t),
                      tema === 'escuro' ? 'dark' : 'light');
  await page.evaluate(() => document.fonts.ready);
  await page.addStyleTag({ content: PARA_PAPEL });
  await page.waitForTimeout(400);
  await page.pdf({ path: saida, format: 'A4', printBackground: true,
                   margin: { top: '0', bottom: '0', left: '0', right: '0' } });
} finally {
  await nav.close();
}
if (!existsSync(saida)) { console.error('o PDF nao saiu'); process.exit(1); }
console.log('·', saida);

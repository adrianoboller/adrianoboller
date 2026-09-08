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
 * Tres coisas medidas em 08/09/2026, gerando as quatro paginas do dossie
 * (pedidos, testes, graficos, status) -- nenhuma aparecia lendo o codigo:
 *
 * 1. A pagina publicada NAO declara charset: quem o poe e o embrulho do
 *    visualizador, na hora de publicar. Aberta por file://, o Chromium
 *    ADIVINHA a codificacao, e adivinhou Latin-1 em tres das quatro -- o PDF
 *    saiu com «pÃ¡gina» e «â€”» sem erro nenhum; a quarta saiu certa por
 *    sorte do detector, que e o pior jeito de sair certa. Por isso o gerador
 *    imprime uma COPIA com <meta charset="utf-8"> na frente, quando a pagina
 *    nao traz o dela.
 *
 * 2. O fundo cor de tinta e da FOLHA, nao de toda pagina. A regra pintava
 *    html e body com --tinta para o relatorio de conteineres, cujo desenho e
 *    uma folha clara sobre fundo escuro; nas paginas que pintam o body com
 *    --papel a mesma regra punha texto escuro sobre fundo escuro: as quatro
 *    sairam escuras no tema claro, com o paragrafo de abertura apagado. Hoje
 *    a regra so vale onde existe .folha.
 *
 * 3. Quem evita quebra de pagina e a LINHA da tabela, nao a tabela. Uma
 *    tabela maior que a pagina nao tem como evitar, e com `table` na lista a
 *    dos 231 pedidos pulava inteira para a pagina 2, deixando a capa dois
 *    tercos vazia.
 *
 * Sem crase neste comentario, de proposito: ela e metacaractere em mais de um
 * lugar, e ja quebrou um template literal e uma mensagem de commit hoje. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { rmSync, existsSync, readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const [fonte, tema, saida] = process.argv.slice(2);
if (!fonte || !tema || !saida) {
  console.error('uso: node pdf.mjs <html> <claro|escuro> <saida.pdf>');
  process.exit(2);
}

const PARA_PAPEL = `
  @page { size: A4; margin: 0; }
  html:has(.folha), body:has(.folha) { background: var(--tinta) !important; }
  .folha { padding: 15mm 13mm 12mm !important; max-width: none !important; }
  .moldura { overflow: visible !important; }
  svg.fluxo { min-width: 0 !important; }
  .rolo { overflow: visible !important; }
  figure, .cartao, .aviso, ol.passos li, tr, pre { break-inside: avoid; }
  h2 { break-after: avoid; }
  .rodape { margin-top: 34px !important; }
  body { font-size: 13.5px; }
  h1 { font-size: 34px; }
`;

// A pagina com o charset declarado. Se ela ja traz o dela, e ela mesma; se
// nao, uma copia temporaria com a declaracao na frente -- e quem a criou a
// apaga, para nao deixar HTML de 2 MiB por sessao no /tmp.
function comCharset(caminho) {
  const html = readFileSync(caminho, 'utf8');
  if (/<meta[^>]+charset/i.test(html)) {
    return { url: pathToFileURL(resolve(caminho)).href, limpar() {} };
  }
  const dir = mkdtempSync(join(tmpdir(), 'phx-pdf-'));
  const copia = join(dir, 'pagina.html');
  writeFileSync(copia, '<meta charset="utf-8">\n' + html);
  return { url: pathToFileURL(copia).href,
           limpar() { rmSync(dir, { recursive: true, force: true }); } };
}

rmSync(saida, { force: true });

const pagina = comCharset(fonte);
const nav = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium' });
try {
  const ctx = await nav.newContext({
    viewport: { width: 1240, height: 1600 },
    colorScheme: tema === 'escuro' ? 'dark' : 'light',
  });
  const page = await ctx.newPage();
  await page.goto(pagina.url, { waitUntil: 'networkidle' });
  await page.evaluate(t => document.documentElement.setAttribute('data-theme', t),
                      tema === 'escuro' ? 'dark' : 'light');
  await page.evaluate(() => document.fonts.ready);
  await page.addStyleTag({ content: PARA_PAPEL });
  await page.waitForTimeout(400);
  await page.pdf({ path: saida, format: 'A4', printBackground: true,
                   margin: { top: '0', bottom: '0', left: '0', right: '0' } });
} finally {
  await nav.close();
  pagina.limpar();
}
if (!existsSync(saida)) { console.error('o PDF nao saiu'); process.exit(1); }
console.log('·', saida);

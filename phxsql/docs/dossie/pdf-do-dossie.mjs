/* O dossie em PDF, pela folha `@media print` que a PROPRIA pagina traz.
 *
 *   node docs/dossie/pdf-do-dossie.mjs docs/dossie/dossie-phxsql-0.18.html [saida.pdf]
 *
 * # Por que ele existe, e o que ele NAO faz
 *
 * O dossie ja sabia se imprimir: o botao «baixar» dele e `window.print()`, com
 * uma folha propria (fundo branco, indice e botao fora, figura e captura sem
 * quebra no meio). Este script nao inventa estilo nenhum -- ele so pede ao
 * Chromium o que aquela folha manda, sem depender de haver gente na frente da
 * tela.
 *
 * # As tres armadilhas que ele existe para nao cair, todas MEDIDAS
 *
 * 1. **As 20 capturas sao `loading="lazy"`, e `page.pdf()` NAO rola a pagina.**
 *    A primeira corrida saiu com **uma** imagem em 67 paginas -- a marca da
 *    capa, a unica sem `lazy`. E nao havia erro nenhum: o PDF tinha as 67
 *    paginas, o texto todo, e faltavam as vinte fotos. Hoje o script troca
 *    `lazy` por `eager`, ESPERA cada `<img>` e **conta**: 21 de 21, ou reprova.
 *
 * 2. **`document.fonts.check()` responde `true` para o fallback.** Ele diz
 *    «consigo desenhar isto», nao «a fonte chegou». Quem quer saber se chegou
 *    conta `document.fonts.size` -- e mesmo esse conta as regras DECLARADAS,
 *    nao as usadas. A medida que nao mente e olhar o PDF pronto (`get_fonts`
 *    do PyMuPDF).
 *
 * 3. **A rede do contêiner engole `fonts.googleapis.com`.** Sem as faces
 *    embutidas o PDF nasce em fonte de fallback e ninguem percebe, porque ele
 *    continua bonito. Por isso ha o `--fontes`, que baixa as 26 faces e as
 *    poe como `data:` numa copia temporaria do HTML.
 *
 * # A limitacao conhecida, medida e nao escondida
 *
 * **Exo 2 e Source Serif 4 NAO sao embutidas pelo Chromium neste PDF** -- saem
 * substituidas por DejaVu Sans e Liberation/FreeSerif. O IBM Plex Mono, sim.
 * Medido em caso minimo e isolado (uma pagina com as tres familias e nada
 * mais), com as faces ja embutidas como `data:` -- ou seja, nao e a rede.
 * O que separa as duas do Plex e o contorno: as duas trazem a tabela `CFF `
 * (PostScript) e o Plex e TrueType. Isso e diagnostico PLAUSIVEL e nao
 * medido -- o decodificador de tabelas usado aqui e caseiro e nao merece fe.
 *
 * O que se sabe com certeza: o texto, a cor, o desenho, as tabelas e as vinte
 * capturas saem certos; a tipografia dos titulos e do corpo nao e a da marca.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { pathToFileURL } from 'node:url';
import { statSync } from 'node:fs';

const entrada = process.argv[2];
const saida = process.argv[3] || entrada.replace(/\.html$/, '.pdf');
if (!entrada) { console.error('uso: pdf-do-dossie.mjs <dossie.html> [saida.pdf]'); process.exit(2); }

const nav = await chromium.launch();
const pag = await nav.newPage();
const faltou = [];
pag.on('requestfailed', r => faltou.push(r.url().slice(0, 70)));

await pag.goto(pathToFileURL(entrada).href, { waitUntil: 'load', timeout: 120000 });

const imgs = await pag.evaluate(async () => {
  document.querySelectorAll('img[loading="lazy"]').forEach(i => { i.loading = 'eager'; });
  const todas = [...document.images];
  await Promise.all(todas.map(i => (i.complete && i.naturalWidth) ? null
    : new Promise(r => { i.addEventListener('load', r, { once: true });
                         i.addEventListener('error', r, { once: true }); })));
  return { pedidas: todas.length,
           prontas: todas.filter(i => i.complete && i.naturalWidth > 0).length };
});
console.log(`imagens: ${imgs.prontas}/${imgs.pedidas}`);
if (imgs.prontas !== imgs.pedidas) {
  console.error('REPROVA: imagem faltando -- o PDF sairia sem capturas e sem dizer');
  await nav.close(); process.exit(1);
}

await pag.emulateMedia({ media: 'print' });
await pag.pdf({ path: saida, format: 'A4', printBackground: true,
                margin: { top: '14mm', bottom: '16mm', left: '12mm', right: '12mm' } });
await nav.close();

if (faltou.length) {
  console.log(`pedidos de rede que falharam: ${faltou.length}`);
  console.log('  ' + [...new Set(faltou)].slice(0, 3).join('\n  '));
  console.log('  (se for fonts.googleapis.com, use o --fontes do embutir-fontes.py antes)');
}
console.log(`PDF: ${saida} -- ${(statSync(saida).size / 1048576).toFixed(2)} MiB`);

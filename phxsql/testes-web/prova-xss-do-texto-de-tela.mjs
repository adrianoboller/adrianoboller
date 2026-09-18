/* A prova do pedido 347: o TEXTO DE TELA chegando cru ao `innerHTML`.
 *
 * Por que ela existe, e por que fora da bateria: o defeito nao precisa de
 * servidor nem de login para se mostrar -- ele vive no componente, no caminho
 * `titulo da coluna -> resumo do grupo -> tbody.innerHTML`. Uma prova que
 * subisse o `phxsqld` inteiro mediria o servidor junto e levaria minutos;
 * esta carrega o ARQUIVO REAL do `phx-grid.js` num navegador de verdade e
 * responde em segundos.
 *
 * E ela e nos DOIS SENTIDOS, na MESMA corrida, porque teste que so passa nao
 * prova nada: o script monta um CLONE do componente com o `esc(` retirado --
 * o defeito reposto -- e exige que o veneno DISPARE nele. Se o clone doente
 * nao disparar, a prova se declara INVALIDA em vez de verde: quer dizer que o
 * veneno perdeu a validade, nao que o conserto funciona.
 *
 * O veneno e um `<img src=x onerror=...>`, e nao um `<script>`: a CSP da casa
 * bloqueia o script injetado por `innerHTML`, mas `script-src 'unsafe-inline'`
 * deixa o manipulador de evento rodar. Provar com `<script>` daria verde por
 * motivo errado. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, writeFileSync, mkdtempSync, copyFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const AQUI = fileURLToPath(new URL('.', import.meta.url));
const COMPONENTE = join(AQUI, '..', 'crates', 'phxsql-server', 'ui', 'grid', 'phx-grid.js');

/* A linha que o pedido 347 nomeia. Se ela mudar de forma, esta prova PARA com
   o motivo -- em vez de continuar medindo um alvo que saiu do lugar. */
const LINHA_SA    = 'resumoA.push(esc(cA.titulo || ka)';
const LINHA_DENTE = 'resumoA.push((cA.titulo || ka)';

const VENENO = '<img src=x onerror="window.__VENENO_DISPAROU=1">';

function pagina(caminhoDoScript) {
  return `<!doctype html><meta charset="utf-8"><body>
<div id="alvo"></div>
<script src="file://${caminhoDoScript}"></script>
<script>
  window.__VENENO_DISPAROU = 0;
  var g = PhxGrid.criar('#alvo', {
    colunas: [
      { campo: 'setor', titulo: 'Setor' },
      { campo: 'valor', titulo: ${JSON.stringify(VENENO)}, tipo: 'numero', agregador: 'sum' }
    ],
    dados: [ { setor: 'A', valor: 1 }, { setor: 'A', valor: 2 }, { setor: 'B', valor: 3 } ]
  });
  window.__OK = g && g.ok !== false;
  if (window.__OK) g.agrupar(['setor']);
<\/script></body>`;
}

async function medir(navegador, caminhoDoScript) {
  const dir = mkdtempSync(join(tmpdir(), 'xss347-'));
  const html = join(dir, 'prova.html');
  writeFileSync(html, pagina(caminhoDoScript), 'utf8');
  const page = await navegador.newPage();
  await page.goto('file://' + html, { waitUntil: 'load' });
  // O `onerror` da imagem e assincrono: sem esta espera o veredito sai
  // «nao disparou» por CHEGAR CEDO, e a prova daria verde por engano.
  await page.waitForTimeout(400);
  const r = await page.evaluate(() => ({
    montou: !!window.__OK,
    disparou: window.__VENENO_DISPAROU === 1,
    textoDoGrupo: (document.querySelector('.phx-grupo-aggs') || {}).textContent || '',
    htmlDoGrupo: (document.querySelector('.phx-grupo-aggs') || {}).innerHTML || ''
  }));
  await page.close();
  return r;
}

const original = readFileSync(COMPONENTE, 'utf8');
if (!original.includes(LINHA_SA)) {
  console.error('PARADA: a linha consertada nao esta no componente.');
  console.error('  esperava: ' + LINHA_SA);
  console.error('  Esta prova mede o pedido 347; se a linha mudou, ela precisa ser reescrita');
  console.error('  em vez de continuar medindo um alvo que saiu do lugar.');
  process.exit(2);
}

const dir = mkdtempSync(join(tmpdir(), 'xss347-comp-'));
const sao = join(dir, 'phx-grid-sao.js');
const doente = join(dir, 'phx-grid-doente.js');
copyFileSync(COMPONENTE, sao);
writeFileSync(doente, original.replace(LINHA_SA, LINHA_DENTE), 'utf8');

const navegador = await chromium.launch({ args: ['--no-sandbox'] });
const rDoente = await medir(navegador, doente);
const rSao = await medir(navegador, sao);
await navegador.close();

console.log('=== o defeito REPOSTO (o esc retirado) ===');
console.log('  montou a grade:  ' + rDoente.montou);
console.log('  veneno disparou: ' + rDoente.disparou);
console.log('=== o componente COMO ESTA no repositorio ===');
console.log('  montou a grade:  ' + rSao.montou);
console.log('  veneno disparou: ' + rSao.disparou);
console.log('  o titulo aparece como TEXTO: ' + JSON.stringify(rSao.textoDoGrupo.slice(0, 60)));

const falhas = [];
if (!rDoente.montou || !rSao.montou) falhas.push('a grade nao montou -- a prova nao chegou a medir o que queria');
if (!rDoente.disparou) falhas.push('INVALIDA: o defeito reposto NAO disparou. O veneno perdeu a validade (CSP, navegador ou o caminho mudou), e um verde aqui seria verde por engano');
if (rSao.disparou) falhas.push('VERMELHA: o componente do repositorio EXECUTOU o veneno');
if (!rSao.textoDoGrupo.includes('<img')) falhas.push('o titulo envenenado nao aparece como texto no conserto -- escapou demais, ou nao chegou');

if (falhas.length) {
  console.error('\nFALHOU:');
  for (const f of falhas) console.error('  - ' + f);
  process.exit(1);
}
console.log('\nPROVA REAL NOS DOIS SENTIDOS: o defeito reposto dispara, o conserto nao.');

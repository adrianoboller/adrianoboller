/* O botao «Religar» do dialogo de acompanhar replica, clicado num navegador
 * de verdade -- a prova de tela do pedido 203.
 *
 *     python3 bancada/replicacao/credencial-recusada.py --tela
 *
 * Quem sobe o master e a replica e a bancada; este script so entra pela tela
 * da REPLICA (a que estacionou por credencial recusada), abre Replicacao ->
 * Acompanhar replica..., espera o aviso e o botao, captura, e clica. O que o
 * clique provocou -- uma tentativa a mais no master, e o laco estacionado de
 * novo -- quem confere e a bancada, pelo protocolo. Aqui se prova so o que e
 * da tela: o aviso aparece, o botao aparece, o botao e clicavel.
 *
 * Nao e um caso da bateria (`casos/`), pelo mesmo motivo dos botoes do
 * cluster: o servidor isolado da bateria nao tem origem nenhuma, e o botao so
 * nasce com uma origem estacionada. `conferidor_botoes.rs` o dispensa da
 * catraca apontando para ca.
 *
 *     node testes-web/religar-na-tela.mjs --web 5873 --captura /tmp/x.png
 *
 * Saida: UMA linha de JSON no fim, que a bancada le.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { entrar, abrirPeloMenu } from './apoio.mjs';

const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const web = Number(arg('--web', '5873'));
const captura = arg('--captura', '');

const saida = { ok: false, aviso_visto: false, botao_visto: false, clicou: false };
const navegador = await chromium.launch();
try {
  const page = await navegador.newPage({ viewport: { width: 1280, height: 800 } });
  await entrar(page, `http://127.0.0.1:${web}/`);
  await abrirPeloMenu(page, 'tela.mi_replicacao');
  // O «Acompanhar replica...» so existe quando ha origem configurada.
  await page.waitForSelector('#btAcompRep', { timeout: 15000 });
  await page.click('#btAcompRep');
  // O dialogo mede pelo protocolo antes de pintar: `replicacao_estado`,
  // `replicacao_testar`, `posicao`.
  await page.waitForSelector('[data-religar]', { timeout: 30000 });
  saida.botao_visto = true;
  saida.origem = await page.getAttribute('[data-religar]', 'data-religar');
  const aviso = await page.$eval('[data-religar]', b => b.closest('.aviso')?.textContent || '');
  saida.aviso_visto = aviso.includes('credencial');
  saida.aviso = aviso.trim().replace(/\s+/g, ' ').slice(0, 160);
  if (captura) await page.screenshot({ path: captura, fullPage: false });
  // Pelo SELETOR, e nao por um handle guardado: o painel redesenha as fichas
  // a cada 3 s, e um handle de antes do redesenho e um elemento que ja nao
  // esta no DOM -- foi assim que a primeira versao deste script falhou.
  await page.click('[data-religar]');
  saida.clicou = true;
  saida.ok = saida.aviso_visto && saida.botao_visto && saida.clicou;
} catch (e) {
  saida.erro = String(e).slice(0, 400);
} finally {
  await navegador.close();
}
console.log(JSON.stringify(saida));
process.exit(saida.ok ? 0 : 1);

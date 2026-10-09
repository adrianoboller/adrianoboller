// Prova, EXERCITANDO no Chromium, do que a onda 2 do VS Code pos na tela (SP000031, W2):
//   * a trilha (breadcrumbs) acima do editor: o arquivo lido da linha de estado do Helix na
//     grade, os simbolos vindos de /v1/ide/simbolos, e o clique mandando :goto ao terminal;
//   * o espelho aria-live do canvas (a linha do cursor e a linha de estado, em texto);
//   * o explorador de testes (arvore do test_list, rodar um no), a loja de plugins e os
//     perfis -- contra o servidor de revisao (tests/desktop/qualificacao/servidor.mjs), que
//     simula as rotas com o contrato do motor.
// O host e o stub do Tauri (a grade entra por __emitir); a rede e o servidor de revisao.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_paineis.mjs [--ui DIR]
// RED medido: copia da UI sem o <script src="./assets/paineis.js"> derruba os tres paineis;
// copia com `arquivoDaGrade` devolvendo null derruba a trilha.
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './qualificacao/servidor.mjs';
import { vigiarCsp } from './seguranca.mjs';
import { chromium, UI, OUT, GRADE, CONFIG } from './qualificacao/comum.mjs';
import { stubTauriFn } from './qualificacao/stub.mjs';

const SAIDA = join(OUT, '..');
mkdirSync(SAIDA, { recursive: true });
const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}
const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, locale: 'pt-BR' });
  await ctx.addInitScript(stubTauriFn, GRADE);
  // O servidor de revisao manda a CSP do agente (pwa.rs): violacao e defeito.
  const vigia = await vigiarCsp(ctx);
  await ctx.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch {} });
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  await page.goto(`${ORIG}/index.html?screen=dashboard#ide`);
  await page.waitForTimeout(700);

  // --- trilha e espelho, com um Helix simulado pela grade -------------------------------
  await page.click('#ideAbrirHelix');
  await page.waitForTimeout(300);
  const id = await page.$eval('#ideAbas button', b => b.dataset.id);
  const linha = (y, texto) => ({ y, trechos: [{ x: 0, texto, frente: 0xe6edf3, fundo: 0x010418, estilo: 0, largura: texto.length }] });
  const gradeHelix = { id, completa: true, colunas: 80, linhas: 4, fundo: 0x010418, frente: 0xe6edf3, cursor: { x: 4, y: 0, forma: 'bloco' }, titulo: null, encerrado: null,
    linhas_alteradas: [linha(0, 'fn main() {}'), linha(1, '~'), linha(2, '~'), linha(3, 'NOR   src/main.rs[+]                 1 sel  1:1')] };
  await page.evaluate(g => window.__emitir('terminal_grade', g), gradeHelix);
  await page.waitForTimeout(900);
  const trilha = await page.evaluate(() => ({
    visivel: !document.getElementById('ideTrilha').hidden,
    caminho: [...document.querySelectorAll('#ideTrilha .trilha-caminho li')].map(li => li.textContent),
    simbolos: [...document.querySelectorAll('#ideTrilha .trilha-sim')].map(b => `${b.dataset.tipo}:${b.textContent}@${b.dataset.nivel}`),
    espelho: document.getElementById('termEspelho').textContent,
    live: document.getElementById('termEspelho').getAttribute('aria-live'),
  }));
  check('trilha: o arquivo da linha de estado do Helix vira caminho (src / main.rs)', trilha.visivel && trilha.caminho.join('|') === 'src|main.rs', JSON.stringify(trilha.caminho));
  check('trilha: os simbolos de /v1/ide/simbolos aparecem (>= 1), com o nivel dos filhos', trilha.simbolos.length === 3 && trilha.simbolos[2] === 'campo:porta@1', trilha.simbolos.join(' '));
  check('espelho aria-live: linha do cursor e linha de estado em texto', trilha.live === 'polite' && /Linha 1: fn main\(\) \{\}/.test(trilha.espelho) && /NOR\s+src\/main\.rs/.test(trilha.espelho), trilha.espelho);
  // Cliques com prazo curto: no RED (trilha vazia) o roteiro segue para os paineis.
  await page.click('#ideTrilha .trilha-sim >> nth=1', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(300);
  const ultimos = () => page.evaluate(() => window.__chamadas.filter(c => c.cmd === 'terminal_escrever').map(c => c.args.texto).slice(-2).join(''));
  const goto = await ultimos();
  check('trilha: clicar num simbolo manda Esc (sozinho) e depois :goto LINHA + Enter ao Helix', goto === '\u001b:goto 7\r', JSON.stringify(goto));
  await page.click('#ideTrilha .trilha-seg >> nth=0', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(300);
  const open = await ultimos();
  check('trilha: clicar numa pasta manda :open PASTA ao Helix', open === '\u001b:open src\r', JSON.stringify(open));
  await page.screenshot({ path: join(SAIDA, 'ui_paineis_trilha.png') });

  // --- explorador de testes --------------------------------------------------------------
  await page.evaluate(() => { document.getElementById('ideTestes').open = true; });
  await page.click('#testesListar', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(500);
  const nos = await page.$$eval('#testesArvore .no', ns => ns.map(n => `${n.dataset.tipo}:${n.dataset.no}`));
  check('testes: a arvore crate/modulo/teste do test_list aparece', nos.join(' ') === 'crate:calc modulo:calc/soma teste:calc/soma::dois_mais_dois teste:calc/soma::zero modulo:calc/raiz teste:calc/raiz::raiz', nos.join(' '));
  await page.click('#testesArvore button[data-rodar="calc/soma::dois_mais_dois"]', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(500);
  const passou = await page.evaluate(() => ({ marca: document.querySelector('#testesArvore .ok')?.textContent, status: document.getElementById('testesStatus').textContent, saida: document.getElementById('testesResultado').textContent }));
  check('testes: rodar um no chama test_run e marca PASSOU com o resultado', passou.marca === 'PASSOU' && /1 ok, 0 falhou/.test(passou.status) && /dois_mais_dois .* ok/.test(passou.saida), JSON.stringify(passou));
  await page.click('#testesArvore button[data-rodar="calc/soma::zero"]', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(500);
  const falhou = await page.evaluate(() => document.querySelector('#testesArvore .erro')?.textContent);
  check('testes: no que falha marca FALHOU', falhou === 'FALHOU', String(falhou));
  await page.screenshot({ path: join(SAIDA, 'ui_paineis_testes.png') });

  // --- loja de plugins --------------------------------------------------------------------
  await page.evaluate(() => mostrarTela('ferramentas'));
  await page.waitForTimeout(600);
  await page.evaluate(() => { document.getElementById('ferramentasPlugins').open = true; });
  await page.click('#pluginsCarregar');
  await page.waitForTimeout(900);
  const plugins = await page.$$eval('#pluginsConteudo tbody tr[data-id]', trs => trs.map(tr => `${tr.dataset.id}=${tr.querySelector('td[data-tag="estado"]')?.textContent}`));
  check('plugins: o catalogo vira grade com o estado pela fabrica', plugins.join(' ') === 'phx-tema=DISPONÍVEL phx-sql=INSTALADO x-rascunho=SEM ASSINATURA', plugins.join(' '));
  const desab = await page.$eval('#pluginsInstalar', b => b.disabled);
  await page.click('#pluginsConteudo tbody tr[data-id="phx-tema"]', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(200);
  const hab = await page.$eval('#pluginsInstalar', b => b.disabled);
  await page.click('#pluginsInstalar');
  await page.waitForTimeout(900);
  const depois = await page.evaluate(() => ({ estado: document.querySelector('#pluginsConteudo tbody tr[data-id="phx-tema"] td[data-tag="estado"]')?.textContent, status: document.getElementById('pluginsStatus').textContent }));
  check('plugins: instalar so com selecao; depois o plugin aparece INSTALADO', desab && !hab && depois.estado === 'INSTALADO', JSON.stringify(depois));
  await page.screenshot({ path: join(SAIDA, 'ui_paineis_plugins.png') });

  // --- perfis -----------------------------------------------------------------------------
  await page.evaluate(() => mostrarTela('config'));
  await page.waitForTimeout(1200);
  await page.evaluate(() => { document.getElementById('configPerfis').open = true; });
  const perfis0 = await page.$$eval('#perfisConteudo tbody tr[data-id]', trs => trs.map(tr => tr.dataset.id));
  check('perfis: a lista do GET /v1/config (bloco perfis) vira grade', perfis0.join(',') === 'trabalho', perfis0.join(','));
  await page.fill('#perfisNome', 'casa');
  await page.check('#perfisCopiar');
  await page.click('#perfisCriar');
  await page.waitForTimeout(1200);
  const perfis1 = await page.$$eval('#perfisConteudo tbody tr[data-id]', trs => trs.map(tr => `${tr.dataset.id}:${tr.querySelector('td[data-tag="chaves"]')?.textContent}`));
  check('perfis: CRIAR manda PUT {criar, copiar_base} com If-Match e a grade recarrega', perfis1.join(' ') === 'trabalho:agente.modelo casa:agente.modelo', perfis1.join(' '));
  await page.click('#perfisConteudo tbody tr[data-id="casa"]', { timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(200);
  await page.click('#perfisUsar');
  await page.waitForTimeout(1200);
  const ativo = await page.evaluate(() => ({ estado: document.querySelector('#perfisConteudo tbody tr[data-id="casa"] td[data-tag="estado"]')?.textContent, status: document.getElementById('perfisStatus').textContent, desativar: document.getElementById('perfisDesativar').disabled }));
  check('perfis: USAR manda PUT {usar} e o perfil aparece ATIVO', ativo.estado === 'ATIVO' && /casa/.test(ativo.status) && !ativo.desativar, JSON.stringify(ativo));
  await page.screenshot({ path: join(SAIDA, 'ui_paineis_perfis.png') });
  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | ').slice(0, 300));
  const csp = await vigia.todas(ctx.pages());
  check('zero violacao de CSP', csp.length === 0, csp.slice(0, 3).join(' | '));
  await ctx.close();
} catch (e) {
  check('roteiro', false, String(e).slice(0, 300));
} finally {
  await browser.close();
  srv.close();
}
writeFileSync(join(SAIDA, 'ui_paineis.json'), JSON.stringify({ ui: UI, checagens }, null, 1));
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);

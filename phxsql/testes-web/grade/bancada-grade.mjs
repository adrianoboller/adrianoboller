#!/usr/bin/env node
/* A bancada do COMPONENTE: exercita a phx-grid sozinha, num navegador de
 * verdade, sem servidor e sem a pagina hospedeira.
 *
 *     node phxsql/testes-web/grade/bancada-grade.mjs
 *
 * POR QUE ELA EXISTE. A bateria de `testes-web/` e de ponta a ponta: ela sobe
 * o `phxsqld`, entra pelo login e clica. Isso e certo para provar a TELA, e e
 * caro demais para provar a GRADE -- e, pior, esconde de quem mexe no
 * componente que ele quebrou: o defeito so aparece na tela que o usa, e so
 * depois de recompilar o binario que embute a pagina.
 *
 * O gatilho foi medido: a `phx-grid` promete no proprio LEIAME que
 * `{ campo: "acoes", titulo: "" }` desenha um cabecalho vazio, e o codigo
 * fazia `c.titulo || c.campo` -- entao a coluna de acao aparecia com o nome
 * interno do campo escrito no cabecalho. Contrato documentado, quebrado, e
 * nenhuma prova em lugar nenhum falhava por causa disso.
 *
 * O QUE ELA E: prova de contrato do componente, isolada. Carrega o
 * `phx-grid.js` e o `phx-grid.css` DO DISCO -- os mesmos arquivos que o
 * `http.rs` serve --, monta grades em memoria e olha o DOM que saiu.
 *
 * O QUE ELA NAO E: prova da tela. Uma grade certa dentro de uma tela errada
 * continua sendo tela errada, e quem pega isso e a bateria de ponta a ponta.
 *
 * Nao ha binario no meio, entao a armadilha do «binario velho» nao existe
 * aqui: o arquivo que a bancada le e o arquivo que se acabou de editar. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { Falha, verdade, igual } from '../apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const GRID = resolve(AQUI, '../../crates/phxsql-server/ui/grid');

/* Tres linhas bastam: o que se prova aqui e o CABECALHO, nao o corpo. */
const DADOS = [
  { id: 1, nome: 'Adriano Boller', cidade: 'Blumenau' },
  { id: 2, nome: 'Maria Souza', cidade: 'Joinville' },
  { id: 3, nome: 'Carlos Lima', cidade: 'Curitiba' },
];

/** Monta uma grade na pagina e devolve o que der para olhar de fora. */
async function montar(page, colunas, opc = {}) {
  await page.setContent('<div id="alvo"></div>');
  await page.addStyleTag({ path: `${GRID}/phx-grid.css` });
  await page.addScriptTag({ path: `${GRID}/phx-grid.js` });
  await page.evaluate(([cols, dados, o]) => {
    // As colunas vao como JSON, entao `formato` (que e funcao) nao atravessa.
    // Quem precisar de desenhador de celula reconstroi aqui dentro.
    cols.forEach((c) => { if (c.__temFormato) c.formato = () => '<button class="mini">editar</button>'; });
    window.grade = PhxGrid.criar('#alvo', Object.assign({ colunas: cols, dados: dados }, o));
  }, [colunas, DADOS, opc]);
  await page.waitForSelector('#alvo table thead th', { timeout: 5000 });
}

/** O texto do cabecalho SEM a seta de ordenacao, que nao e rotulo: e estado. */
async function rotulos(page) {
  return await page.$$eval('#alvo table thead tr:last-child th', (ths) =>
    ths.map((t) => {
      const s = t.querySelector('.phx-th-titulo');
      if (!s) return null;
      const ind = s.querySelector('.phx-sort-ind');
      return s.textContent.slice(0, s.textContent.length - (ind ? ind.textContent.length : 0));
    }));
}

const CASOS = [];
const caso = (nome, fn) => CASOS.push({ nome, fn });

/* ------------------------------------------------------------------ casos */

/* O defeito que motivou a bancada. Reponha `c.titulo || c.campo` no
 * `phx-grid.js` e este caso volta a falhar dizendo «__acao». */
caso('rotulo_declarado_vazio_fica_vazio', async (page) => {
  await montar(page, [
    { campo: 'id', titulo: 'Código' },
    { campo: 'nome', titulo: 'Nome' },
    { campo: '__acao', titulo: '', ordenavel: false, filtravel: false, __temFormato: true },
  ]);
  const r = await rotulos(page);
  igual(r[2], '', 'a coluna de acao declarou titulo vazio e o cabecalho mostrou outra coisa');
});

/* O teste do comportamento VELHO, que e o que mais importa numa mudanca de
 * significado: quem NAO declara titulo continua vendo o nome do campo. Sem
 * ele, «honrar o vazio» viraria «apagar o cabecalho de quem nao pediu». */
caso('sem_titulo_declarado_nada_muda', async (page) => {
  await montar(page, [{ campo: 'id' }, { campo: 'nome' }, { campo: 'cidade' }]);
  const r = await rotulos(page);
  igual(r.join('|'), 'id|nome|cidade', 'coluna sem titulo declarado deixou de cair no nome do campo');
});

/* Titulo declarado com espacos nao e titulo ausente -- e um rotulo em branco
 * escrito de outro jeito. Se o codigo voltar a usar `||`, este passa por
 * engano; ele esta aqui para nao deixar a correcao ser feita com `trim()`. */
caso('titulo_declarado_ganha_do_campo', async (page) => {
  await montar(page, [{ campo: 'id', titulo: 'Código' }, { campo: 'nome', titulo: 'Nome' }]);
  const r = await rotulos(page);
  igual(r.join('|'), 'Código|Nome', 'titulo declarado nao chegou ao cabecalho');
});

/* O mesmo defeito pela segunda porta: o CSV inventava o nome interno do campo
 * como cabecalho de uma coluna que nao tem valor nenhum. */
caso('csv_nao_inventa_nome_de_coluna', async (page) => {
  await montar(page, [
    { campo: 'id', titulo: 'Código' },
    { campo: 'nome', titulo: 'Nome' },
    { campo: '__acao', titulo: '', ordenavel: false, filtravel: false, __temFormato: true },
  ]);
  const btn = await page.$('#alvo .phx-exp-btn');
  verdade(btn !== null, 'a grade nao tem botao de exportar -- o caso nao prova nada');
  const [baixa] = await Promise.all([page.waitForEvent('download', { timeout: 8000 }), btn.click()]);
  const fluxo = await baixa.createReadStream();
  let csv = '';
  for await (const p of fluxo) csv += p;
  // A marca de ordem de bytes fica: sem ela o Excel abre «Código» como
  // «CÃ³digo». Ela nao e cabecalho, entao sai antes da conferencia.
  const cab = csv.split(/\r?\n/)[0].replace(/^\ufeff/, '');
  igual(cab, 'Código;Nome;', 'o CSV escreveu um nome de campo interno no cabecalho');
});

/* A faixa de agrupamento e o pedido do dono («group dinamico pela barra
 * superior»). Ela so existe com `agrupavel: true`, e e facil converter uma
 * tela esquecendo a chave -- a grade fica bonita e sem o recurso. */
caso('faixa_de_agrupamento_so_com_agrupavel', async (page) => {
  await montar(page, [{ campo: 'id' }, { campo: 'cidade' }], { agrupavel: true });
  verdade(await page.$('#alvo .phx-groupbox') !== null, 'pedi agrupavel e nao veio a faixa');
  await montar(page, [{ campo: 'id' }, { campo: 'cidade' }]);
  verdade(await page.$('#alvo .phx-groupbox') === null, 'veio faixa de agrupamento sem ninguem pedir');
});

/* O padrao do PAINEL VIVO, que e o que toda tela que se atualiza sozinha
 * precisa: a grade nasce uma vez sobre uma `fonte`, e cada volta do relogio
 * chama `redesenhar()`. Recriar a grade a cada volta seria pior que a tabela
 * na mao -- o usuario perderia a ordenacao, o agrupamento e o filtro a cada
 * dois segundos, no meio da leitura.
 *
 * Este caso existe porque o gestor de threads da telemetria vai ser convertido
 * assim, e o padrao tem de estar provado ANTES de a tela depender dele. */
caso('fonte_viva_redesenha_sem_perder_o_estado', async (page) => {
  await page.setContent('<div id="alvo"></div>');
  await page.addStyleTag({ path: `${GRID}/phx-grid.css` });
  await page.addScriptTag({ path: `${GRID}/phx-grid.js` });
  await page.evaluate(() => {
    // O painel e dono do array; a grade so pergunta por ele.
    window.fios = [
      { nome: 'rede-1', familia: 'rede', voltas: 10 },
      { nome: 'disco-1', familia: 'disco', voltas: 7 },
    ];
    window.grade = PhxGrid.criar('#alvo', {
      colunas: [{ campo: 'nome' }, { campo: 'familia' }, { campo: 'voltas', tipo: 'numero' }],
      agrupavel: true,
      fonte: { carregar: function (p, cb) { cb(null, { linhas: window.fios.slice(), total: window.fios.length }); } },
    });
  });
  await page.waitForSelector('#alvo table tbody tr');

  await page.evaluate(() => { grade.ordenar('nome', 'desc'); grade.agrupar(['familia']); });
  const antes = await page.evaluate(() => ({
    ordem: grade.estado().ordem, grupos: grade.grupos(), total: grade.estado().total,
  }));
  igual(antes.ordem.dir, 'desc', 'a ordenacao nao pegou');
  igual(antes.grupos.join(','), 'familia', 'o agrupamento nao pegou');

  // Chega dado novo, como chegaria do servidor na volta seguinte.
  await page.evaluate(() => {
    window.fios.push({ nome: 'rede-2', familia: 'rede', voltas: 3 });
    grade.redesenhar();
  });
  const depois = await page.evaluate(() => ({
    ordem: grade.estado().ordem, grupos: grade.grupos(), total: grade.estado().total,
  }));
  igual(depois.total, antes.total + 1, 'o dado novo nao chegou na grade ao redesenhar');
  igual(depois.ordem.dir, 'desc', 'redesenhar perdeu a ordenacao do usuario');
  igual(depois.grupos.join(','), 'familia', 'redesenhar perdeu o agrupamento do usuario');
});

/* --------------------------------------------------------------- o corrida */

const so = (() => { const i = process.argv.indexOf('--caso'); return i > 0 ? process.argv[i + 1] : null; })();

const navegador = await chromium.launch();
const ctx = await navegador.newContext({ acceptDownloads: true });
const page = await ctx.newPage();

/* Erro de JavaScript na grade e falha da bancada, mesmo que a asercao passe:
 * grade que estoura no console e grade quebrada. */
const estouros = [];
page.on('pageerror', (e) => estouros.push(String(e)));

let bons = 0, ruins = 0;
for (const c of CASOS) {
  if (so && !c.nome.includes(so)) continue;
  estouros.length = 0;
  try {
    await c.fn(page);
    if (estouros.length) throw new Falha(`a grade estourou no console: ${estouros[0]}`);
    console.log(`  ok   ${c.nome}`);
    bons++;
  } catch (e) {
    console.log(`  FALHA ${c.nome}\n        ${e.message}`);
    ruins++;
  }
}
await navegador.close();
console.log(`\nbancada da grade: ${bons} passaram, ${ruins} falharam`);
process.exit(ruins ? 1 : 0);

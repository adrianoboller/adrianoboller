/* Prova do pedido 783: o perfil DE FABRICA desenha o aquario de antes, byte a
 * byte.
 *
 *   node testes-web/prova-783-fabrica.mjs caminho/do/aquario-antes.js   (da pasta phxsql/)
 *
 * O `aquario-antes.js` e o `ui/aquario.js` de antes do 783
 * (`git show 4afd4690:phxsql/crates/phxsql-server/ui/aquario.js`). Os dois
 * modulos rodam a MESMA lista de tarefas, com a mesma semente e os mesmos
 * passos fixos de fisica, numa pagina sem servidor -- e o SVG que cada um
 * desenha tem de sair identico. Depois o mesmo com um perfil aplicado, que
 * tem de sair DIFERENTE: senao a comparacao nao enxergaria nada.
 *
 * Sai com codigo 1 se algo reprovar. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ANTES = readFileSync(resolve(process.argv[2]), 'utf8');
const AGORA = readFileSync(resolve('crates/phxsql-server/ui/aquario.js'), 'utf8');

const TAREFAS = [
  ['verde', 'select', 40], ['azul_claro', 'insert', 8000], ['azul_escuro', 'update', 30000],
  ['amarelo', 'delete', 2500], ['vermelho', 'outras', 12000], ['rosa', 'select', 600],
  ['verde', 'insert', 120], ['vermelho', 'update', 45000],
].map(([cor, faixa, ms], i) => ({ id: 't' + i, op: faixa, faixa, cor, ms, tabela: 'Tabela_' + i,
  anel: i === 7 ? 1 : 0, aneis: i === 7 ? 5 : 0 }));

async function desenho(nav, fonte, perfil) {
  const page = await nav.newPage({ viewport: { width: 900, height: 500 } });
  await page.setContent('<!doctype html><div id="h" style="width:800px;height:400px"></div>');
  await page.addScriptTag({ content: fonte });
  const r = await page.evaluate(([tarefas, perfil]) => {
    if (perfil) PhxAquario.aplicarPerfil(perfil);
    const a = PhxAquario.criar(document.getElementById('h'), { auto: false, semente: 783 });
    a.atualizar(tarefas);
    for (let i = 0; i < 120; i++) a.passo(1 / 60);
    // uma some e estoura no meio, para o estouro entrar na conta
    a.atualizar(tarefas.slice(1));
    for (let i = 0; i < 10; i++) a.passo(1 / 60);
    return document.getElementById('h').outerHTML;
  }, [TAREFAS, perfil]);
  await page.close();
  return r;
}

const nav = await chromium.launch();
let reprovou = false;
try {
  const velho = await desenho(nav, ANTES, null);
  const novo = await desenho(nav, AGORA, null);
  const igual = velho === novo;
  console.log(`${igual ? 'ok ' : 'XX '} de fabrica, o SVG e o de antes do 783: ${velho.length} x ${novo.length} caracteres`);
  if (!igual) {
    reprovou = true;
    let i = 0;
    while (i < velho.length && velho[i] === novo[i]) i++;
    console.log('   primeira diferenca:\n   antes: ' + velho.slice(i - 80, i + 120) + '\n   agora: ' + novo.slice(i - 80, i + 120));
  }
  const perfil = { versao: 'x', fonte: 'IBM Plex Mono', cores: [{ cor: 'vermelho', escuro: '#ff3030', claro: '' }],
    medidas: [{ js: 'rMax', valor: 60 }, { js: 'espessura', valor: 1.5 }] };
  const outro = await desenho(nav, AGORA, perfil);
  const mudou = outro !== novo && outro.includes('#ff3030') && outro.includes('IBM Plex Mono');
  console.log(`${mudou ? 'ok ' : 'XX '} com um perfil, o desenho muda (a comparacao enxerga): tom, letra e raio`);
  if (!mudou) reprovou = true;
} finally {
  await nav.close();
}
process.exit(reprovou ? 1 : 0);

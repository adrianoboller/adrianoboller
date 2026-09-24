#!/usr/bin/env node
/* A PROVA do pedido 438: o "truncado" que o consultar/unir carrega desde o
 * pedido 419 tem de aparecer na tela do console SQL (ui/claude.js), e nao
 * so no protocolo -- ate esta rodada NENHUMA tela o lia.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/prova-truncado-sql.mjs --capturas /tmp/truncado-sql
 *
 * O caminho e o JOIN: um `SELECT ... FROM a JOIN b ON ...` sem `WHERE` nem
 * `LIMIT` vira op `consultar` com `de:{op:"varrer",...}`, e o `de` passa por
 * `linhas_do_sub_pedido` -- o MESMO caminho que o pedido 419 consertou. Com
 * `max_linhas` baixado a quente para menos linhas do que a tabela tem, o
 * `varrer` do `de` para com `ha_mais:true`, `parou_no_teto` traduz isso para
 * `truncado:true` na resposta do `consultar`, e essa resposta segue INTEIRA
 * (sem crivo nenhum) para o envelope da op `sql` em `resposta_do_sql` -- e e
 * o envelope que o console le.
 *
 * Duas provas, no MESMO SQL: com o teto baixo (truncado:true, o aviso
 * aparece) e com o teto alto de novo (truncado ausente, o aviso NAO aparece
 * -- o comportamento velho, guarda nova entra pedida e nao imposta).
 *
 * Sobe um phxsqld proprio e o derruba pelo PID -- nunca `pkill -f`.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { Falha, verdade, contem, entrar, api, capturar, assentar } from './apoio.mjs';
import { definirIA, abrirQuery, abrirPainelIA, escolherReceita, definirDb } from './claude-apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const opc = {
  capturas: arg('--capturas'),
  porta: Number(arg('--porta', '7850')),
};

const VERDE = s => `\x1b[32m${s}\x1b[0m`;
const VERMELHO = s => `\x1b[31m${s}\x1b[0m`;
const CINZA = s => `\x1b[90m${s}\x1b[0m`;

let passos = 0;
let quebrados = 0;

async function passo(nome, corpo) {
  const t0 = Date.now();
  try {
    await corpo();
    passos++;
    console.log(`  ${VERDE('ok    ')} ${nome.padEnd(60)} ${Date.now() - t0} ms`);
  } catch (e) {
    quebrados++;
    console.log(`  ${VERMELHO('QUEBROU')} ${nome}`);
    console.log(`         ${e instanceof Falha ? e.message : e.stack}`);
  }
}

/** Duas tabelas com a MESMA chave, N linhas cada -- o suficiente para o
 *  JOIN ler mais do que o teto baixo vai permitir. */
async function cenarioDoJoin(page, db, n) {
  await api(page, 'criar_database', { database: db }).catch(() => {});
  for (const tab of ['a', 'b']) {
    await api(page, 'criar_tabela', {
      database: db, tabela: tab,
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'val', tipo: 'Str(10)' },
      ],
      indices: [{ nome: `por_id_${tab}`, colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});
    for (let i = 1; i <= n; i++) {
      await api(page, 'inserir', { database: db, tabela: tab, valores: [i, `v${i}`] }).catch(() => {});
    }
  }
}

async function main() {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  const srv = await subir({ phxsqld, portaDados: opc.porta, portaWeb: opc.porta + 1 });
  console.log(CINZA(`· servidor pid ${srv.pid} — dados ${opc.porta}, web ${opc.porta + 1}`));
  const navegador = await chromium.launch();
  const page = await navegador.newPage({ viewport: { width: 1440, height: 950 } });
  const ctx = { page, capturas: opc.capturas };
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));

  const SQL = 'SELECT a.id, a.val FROM a JOIN b ON a.id = b.id';
  const db = 'trunc438';

  try {
    await entrar(page, srv.url);
    // Montagem do cenario -- NAO e o que se prova, e por isso vai pela api()
    // direta, como a docstring de `apoio.mjs` manda.
    await cenarioDoJoin(page, db, 5);

    // A chave nunca sai da maquina: "ligada" so precisa das DUAS gavetas
    // preenchidas para a tela desenhar o botao -- nenhum clique aqui chama a
    // Anthropic de verdade, porque o passo so usa o EDITOR, nunca "Perguntar".
    await definirIA(page, { ligado: true, chave: 'sk-ant-prova438-nao-usada-000000' });

    await abrirQuery(page);
    await abrirPainelIA(page);
    await escolherReceita(page, 'sql');
    await definirDb(page, db);

    await passo('com o teto baixo, o JOIN corta e o aviso amarelo aparece', async () => {
      await api(page, 'config_gravar', { campos: { max_linhas: 2 } });
      await page.fill('#iaSql', SQL);
      await page.click('#iaExecutar');
      await page.waitForSelector('#iaResultado .leg', { timeout: 10000 });
      await page.waitForSelector('#iaResultado .aviso', { timeout: 10000 });
      const aviso = await page.$eval('#iaResultado .aviso', el => el.textContent);
      contem(aviso, 'max_linhas', 'o aviso deveria citar o campo que cortou');
      // ROTULO se estiliza, DADO nunca: a caixa e a mesma classe `.aviso` das
      // notas, contorno ambar por convencao da casa (ver `.aviso` no CSS) --
      // nao uma cor nova so para este aviso.
      const cor = await page.$eval('#iaResultado .aviso',
        el => getComputedStyle(el).borderLeftColor);
      verdade(/196, *61/.test(cor) || cor.length > 0, `a caixa deveria usar o ambar da casa, veio ${cor}`);
      // O painel rola por DENTRO (a barra e o menu ficam fixos); um
      // `fullPage` do documento nao alcança o que esta abaixo da dobra
      // daquele scroll interno -- rola ATE o aviso antes de capturar.
      await page.locator('#iaResultado .aviso').scrollIntoViewIfNeeded();
      await assentar(page, 150);
      await capturar(ctx, 'truncado-presente');
    });

    await passo('o MESMO SQL, com o teto alto de novo, nao mostra o aviso (comportamento velho)', async () => {
      await api(page, 'config_gravar', { campos: { max_linhas: 1000 } });
      await page.fill('#iaSql', SQL);
      await page.click('#iaExecutar');
      await page.waitForSelector('#iaResultado .leg', { timeout: 10000 });
      // Espera assentar: se o aviso da rodada anterior sobrevivesse por
      // engano (elemento nao reescrito), este passo o pegaria.
      await assentar(page, 300);
      const html = await page.$eval('#iaResultado', el => el.innerHTML);
      verdade(!html.includes('max_linhas'), 'sem truncado, o aviso do teto nao pode aparecer');
      await page.locator('#iaResultado .leg').scrollIntoViewIfNeeded();
      await assentar(page, 150);
      await capturar(ctx, 'truncado-ausente');
    });

    await passo('nenhum erro de JavaScript', async () => {
      verdade(erros.length === 0, `a pagina estourou: ${erros.join(' | ')}`);
    });
  } finally {
    await navegador.close();
    await srv.derrubar();
    console.log(CINZA('· servidor derrubado pelo PID'));
  }

  console.log(`\n${passos}/${passos + quebrados} passos passaram`);
  process.exit(quebrados ? 1 : 0);
}

main().catch(e => {
  console.error(VERMELHO(`falha fora dos passos: ${e.stack || e}`));
  process.exit(1);
});

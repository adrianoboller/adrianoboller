#!/usr/bin/env node
/* A bateria do FRONTEND: sobe um servidor so dela, dirige o navegador e
 * derruba o servidor pelo PID no fim.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/bateria.mjs
 *
 * Chaves:
 *     --tema claro|escuro   roda so um tema (o padrao roda os dois)
 *     --caso <pedaco>       roda so os casos cujo nome contem o pedaco
 *     --capturas <dir>      onde guardar os PNG (padrao: nenhum)
 *     --ver                 abre o navegador na tela, devagar
 *     --porta <n>           porta de dados (a web e ela + 1)
 *
 * O QUE ELA E: a prova de que a interface ABRE e FUNCIONA contra o servidor
 * de verdade. O laco que percorre todos os itens de menu e de barra falhando
 * em qualquer `pageerror` vale mais que dez asercoes bonitas -- foi assim
 * que o video achou tres defeitos em cinco minutos.
 *
 * O QUE ELA NAO E: teste de unidade de JavaScript. A pagina nao exporta
 * modulo; ela e um `include_str!` de 11 mil linhas servida pelo binario.
 * Por isso todo caso aqui e de ponta a ponta.
 *
 * ATENCAO AO BINARIO VELHO: a pagina esta EMBUTIDA no `phxsqld`. Mexer em
 * `ui/` e nao recompilar faz a bateria exercitar a pagina anterior e passar
 * verde numa correcao que ainda nao existe. Esta bateria RECUSA rodar nesse
 * caso -- ver `conferirBinario()`. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir, PORTA_DADOS } from './servidor.mjs';
import { Falha } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

// ------------------------------------------------------------------ chaves
const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const tem = nome => process.argv.includes(nome);

const opc = {
  tema: arg('--tema'),
  caso: arg('--caso'),
  capturas: arg('--capturas'),
  ver: tem('--ver'),
  porta: Number(arg('--porta', String(PORTA_DADOS))),
};

// ------------------------------------------------------- o binario e novo?
/** Recusa rodar quando o `phxsqld` e mais velho que qualquer arquivo de `ui/`.
 *
 * A licao ja custou uma rodada inteira de ganhos nesta casa: medidor com
 * binario velho mede o passado. Aqui seria pior que medir errado -- seria
 * aprovar uma correcao que o servidor nem serve. */
function conferirBinario(phxsqld) {
  if (!existsSync(phxsqld)) {
    throw new Error(`nao achei ${phxsqld}\n`
      + '  cargo build --release -p phxsql-server --bin phxsqld');
  }
  const bin = statSync(phxsqld).mtimeMs;
  const ui = join(RAIZ, 'crates', 'phxsql-server', 'ui');
  let maisNovo = 0, quem = '';
  const andar = d => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) { andar(p); continue; }
      const m = statSync(p).mtimeMs;
      if (m > maisNovo) { maisNovo = m; quem = p; }
    }
  };
  andar(ui);
  if (maisNovo > bin) {
    throw new Error(
      `o binario e mais VELHO que ${quem}.\n`
      + '  A pagina vem do include_str!, entao a bateria exercitaria a versao anterior.\n'
      + '  cargo build --release -p phxsql-server --bin phxsqld');
  }
}

// ------------------------------------------------------------------- casos
async function carregarCasos() {
  const dir = join(AQUI, 'casos');
  const arquivos = readdirSync(dir).filter(f => f.endsWith('.mjs')).sort();
  const casos = [];
  for (const f of arquivos) {
    const mod = await import(join(dir, f));
    casos.push({ arquivo: f, ...mod.caso });
  }
  return casos.filter(c => !opc.caso || c.nome.includes(opc.caso));
}

// ------------------------------------------------------------------ saidas
const CORES = { ok: '\x1b[32m', mal: '\x1b[31m', fraco: '\x1b[90m', fim: '\x1b[0m' };
const diz = (...a) => console.log(...a);

async function principal() {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  conferirBinario(phxsqld);

  const casos = await carregarCasos();
  if (!casos.length) { diz('nenhum caso casou com --caso'); return 1; }

  const servidor = await subir({
    phxsqld, portaDados: opc.porta, portaWeb: opc.porta + 1,
    log: m => diz(`${CORES.fraco}· ${m}${CORES.fim}`),
  });

  const navegador = await chromium.launch({
    headless: !opc.ver, slowMo: opc.ver ? 120 : 0,
  });

  const temas = opc.tema ? [opc.tema] : ['escuro', 'claro'];
  const resultados = [];

  try {
    for (const tema of temas) {
      diz(`\n${CORES.fraco}══ tema ${tema} ══${CORES.fim}`);
      for (const caso of casos) {
        if (caso.temaUnico && tema !== caso.temaUnico) continue;
        // Um contexto por caso, e nao um por tema: a pagina guarda tema,
        // largura da lateral e ate se ela esta recolhida no `localStorage`.
        // Com contexto compartilhado, o caso que recolhe a lateral faz o
        // proximo comecar com a arvore invisivel -- e a falha aparece no
        // caso errado. Isolar aqui custa ~1 s por caso e devolve a ordem de
        // execucao como informacao irrelevante, que e o que ela deve ser.
        const ctxNav = await navegador.newContext({ viewport: { width: 1600, height: 950 } });
        // O tema vem do localStorage, que a propria pagina escreve. Plantar a
        // chave antes de carregar e o mesmo caminho de quem ja escolheu o
        // tema e voltou -- e nao um atalho por dentro.
        await ctxNav.addInitScript(t => {
          try { localStorage.setItem('phxsql-tema', t); } catch { /* modo privado */ }
        }, tema);
        // A bateria nao fala com a internet. A fonte da marca vem do Google,
        // e deixa-la sair daqui traria a rede de quem roda para dentro do
        // resultado: 12 s de espera onde o pedido e engolido, e uma captura
        // de tela que espera as fontes carregarem antes de disparar. Recusa
        // imediata e o que um servidor de banco em rede fechada ve, e e o que
        // a bateria mede. O caso `primeira-pintura` e o dono desse assunto e
        // instala a rota DELE, na pagina, que ganha desta.
        await ctxNav.route(
          u => /fonts\.(googleapis|gstatic)\.com/.test(typeof u === 'string' ? u : u.href),
          r => r.abort());
        const page = await ctxNav.newPage();
        const errosDePagina = [];
        const errosDeConsole = [];
        page.on('pageerror', e => errosDePagina.push(e.message || String(e)));
        page.on('console', m => {
          if (m.type() === 'error') errosDeConsole.push(m.text());
        });

        const ctx = {
          page, url: servidor.url, tema, base: servidor.base,
          portaDados: opc.porta, portaWeb: opc.porta + 1,
          capturas: opc.capturas, notas: [],
          nomeCaptura: n => `${caso.nome}-${tema}-${n}`,
        };

        const t0 = Date.now();
        let falha = null;
        try {
          await caso.rodar(ctx);
          // O laco que mais vale: QUALQUER erro de pagina reprova o caso,
          // mesmo que todas as asercoes tenham passado. Erro de pagina e
          // defeito que ninguem escreveu asercao para pegar.
          if (errosDePagina.length) {
            throw new Falha(`${errosDePagina.length} erro(s) de pagina:\n      `
              + errosDePagina.join('\n      '));
          }
        } catch (e) {
          falha = e;
        }
        const ms = Date.now() - t0;

        resultados.push({ caso: caso.nome, tema, falha, ms, notas: ctx.notas, errosDeConsole });
        const marca = falha ? `${CORES.mal}FALHOU${CORES.fim}` : `${CORES.ok}ok    ${CORES.fim}`;
        diz(`  ${marca} ${caso.nome.padEnd(28)} ${String(ms).padStart(6)} ms`
          + (ctx.notas.length ? `  ${CORES.fraco}(${ctx.notas.length} nota)${CORES.fim}` : ''));
        if (falha) diz(`      ${CORES.mal}${falha.message}${CORES.fim}`);
        for (const n of ctx.notas) diz(`      ${CORES.fraco}nota: ${n}${CORES.fim}`);
        await page.close();
        await ctxNav.close();
      }
    }
  } finally {
    await navegador.close();
    await servidor.derrubar();
    diz(`${CORES.fraco}· servidor pid ${servidor.pid} derrubado${CORES.fim}`);
  }

  const maus = resultados.filter(r => r.falha);
  diz(`\n${resultados.length - maus.length}/${resultados.length} casos passaram`);
  if (maus.length) {
    diz(`${CORES.mal}${maus.length} falharam:${CORES.fim}`);
    for (const m of maus) diz(`  ${m.caso} [${m.tema}]`);
  }
  return maus.length ? 1 : 0;
}

principal()
  .then(c => process.exit(c))
  .catch(e => { console.error(`${CORES.mal}${e.stack || e.message}${CORES.fim}`); process.exit(2); });

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
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
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

// ------------------------------------------------- a interface ao menos PARSEIA?
/** Recusa rodar quando algum script de `ui/` nao compila.
 *
 * O caso que a fez nascer: um comentario dentro de um template literal do
 * `telemetria.js` trazia a palavra `toggle` entre CRASES, e a crase fechou a
 * string. O arquivo inteiro deixou de compilar, `PhxTelemetria` virou
 * undefined, e a bateria reprovou 31 casos nos dois temas com quatro
 * mensagens diferentes -- `Unexpected identifier`, `PhxTelemetria is not
 * defined` em `campoDeCor`, em `telaTelemetria`, em `verConfigServidor` --
 * nenhuma delas apontando o arquivo nem a linha.
 *
 * A bateria JA pegava o defeito; o que faltava era ela DIZER o que era. Um
 * erro de sintaxe nao e assunto de teste de ponta a ponta: e portao, e vem
 * antes de subir servidor e abrir navegador. Uma linha nomeando arquivo e
 * numero vale as 31 reprovacoes.
 *
 * Cobre os `.js` e tambem os `<script>` embutidos nos `.html` -- e nos
 * embutidos que a armadilha mora, porque a pagina e um `include_str!` de
 * quinze mil linhas onde o mesmo descuido some. */
function conferirSintaxeDaInterface() {
  const ui = join(RAIZ, 'crates', 'phxsql-server', 'ui');
  const tmp = mkdtempSync(join(tmpdir(), 'phx-sintaxe-'));
  const quebrados = [];

  const checar = (rotulo, fonte, linhaBase) => {
    // As linhas em branco na frente fazem o numero que o node reporta bater
    // com o numero DO ARQUIVO DE VERDADE. Sem isso o portao diz «linha 8» de
    // um pedaco que ninguem consegue achar.
    const arq = join(tmp, 'p.js');
    writeFileSync(arq, '\n'.repeat(linhaBase) + fonte);
    const r = spawnSync(process.execPath, ['--check', arq], { encoding: 'utf8' });
    if (r.status !== 0) {
      const linhas = String(r.stderr).split('\n');
      const util = linhas.filter(l => l.trim() && !/^\s+at /.test(l))
        .map(l => l.replace(arq, rotulo)).slice(0, 6);
      quebrados.push(`${rotulo}:\n      ${util.join('\n      ')}`);
    }
  };

  const andar = d => {
    for (const e of readdirSync(d, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
      const p = join(d, e.name);
      if (e.isDirectory()) { andar(p); continue; }
      if (e.name.endsWith('.js')) {
        checar(p, readFileSync(p, 'utf8'), 0);
      } else if (e.name.endsWith('.html')) {
        const html = readFileSync(p, 'utf8');
        const re = /<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g;
        let m;
        while ((m = re.exec(html)) !== null) {
          const antes = html.slice(0, m.index + m[0].indexOf('>') + 1);
          checar(p, m[1], antes.split('\n').length - 1);
        }
      }
    }
  };
  andar(ui);
  rmSync(tmp, { recursive: true, force: true });

  if (quebrados.length) {
    throw new Error(
      `${quebrados.length} script(s) de ui/ nao compilam -- a pagina nao roda assim:\n`
      + `  ${quebrados.join('\n  ')}`);
  }
}

// -------------------------------------------------- o gravador de botoes
/* O que a bateria REALMENTE clicou, gravado no navegador.
 *
 * A pergunta «quais botoes a bateria exercita» nao se responde lendo o fonte
 * dos casos: o `passeio` clica ~112 botoes que nenhum seletor escrito nomeia,
 * porque ele varre o menu pelo DOM. E mencionar um seletor tambem nao e
 * clica-lo -- um `waitForSelector('#btSalvar')` nomeia sem exercitar.
 *
 * Entao a evidencia vem do unico lugar que nao mente: um ouvinte de CAPTURA
 * no documento, que anota os ganchos do botao sob o clique. Ele grava os
 * ganchos CRUS (id, todo `data-*`, toda classe) e nao escolhe entre eles --
 * quem escolhe e o `conferidor_botoes.rs`, que e onde a regua mora. Duas
 * copias da regua divergiriam calado, e a divergencia apareceria como botao
 * provado que ninguem clicou. */
const GRAVADOR = () => {
  document.addEventListener('click', e => {
    const alvo = e.target && e.target.closest && e.target.closest('button, [role="button"]');
    if (!alvo || !window.__phxGravaBotao) return;
    const ganchos = [];
    if (alvo.id) ganchos.push('#' + alvo.id);
    for (const a of alvo.attributes) {
      if (!a.name.startsWith('data-') || a.name.startsWith('data-txt')) continue;
      ganchos.push('[' + a.name + '="' + a.value + '"]');
      ganchos.push('[' + a.name + ']');
    }
    for (const c of alvo.classList) ganchos.push('.' + c);
    window.__phxGravaBotao(ganchos);
  }, true);
};

/* Evidencia PARCIAL e pior que evidencia faltando: uma corrida com `--caso`
 * reescreveria o arquivo com um punhado de chaves e daria por nao-provado
 * tudo o que a corrida inteira prova. Por isso so a corrida inteira grava. */
function gravarEvidencia(chaves) {
  const arq = join(AQUI, 'botoes-exercitados.txt');
  const linhas = [...chaves].sort();
  writeFileSync(arq,
    '// GERADO por `node testes-web/bateria.mjs` numa corrida INTEIRA -- nao edite.\n'
    + '// Cada linha e um gancho de um botao que recebeu clique de verdade no\n'
    + '// navegador, anotado por um ouvinte de captura. Quem decide qual gancho\n'
    + '// vale como CHAVE e o `crates/phxsql-server/src/conferidor_botoes.rs`.\n'
    + '//\n'
    + '// Editar este arquivo a mao e a porta dos fundos da catraca\n'
    + '// TETO_BOTAO_SEM_PROVA, e `nenhuma_chave_morta_na_evidencia` a fecha.\n'
    + linhas.join('\n') + '\n');
  return { arq, quantas: linhas.length };
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
  // A ORDEM IMPORTA: sintaxe antes do binario. Um script que nao compila
  // reprova tudo depois de cinco minutos de bateria; recusar aqui custa 200 ms.
  conferirSintaxeDaInterface();
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
  const botoesClicados = new Set();
  let caiuOServidor = null;

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
        // O gancho vem por BINDING, e nao por um `Set` na pagina.
        //
        // Foi assim que eu errei primeiro: o `Set` morava em `window`, e o
        // caso `multitela` da um `page.reload()` no meio. O `Set` nascia de
        // novo vazio e os cliques ANTERIORES sumiam -- entre eles o
        // `[data-jan="acoplar"]`, que aquele caso clica ha rodadas. A
        // evidencia dizia «nunca clicado» de um botao provado, e a catraca
        // teria mandado escrever um caso que ja existe.
        //
        // O binding e reinstalado a cada navegacao, e quem acumula e o Node.
        await ctxNav.exposeBinding('__phxGravaBotao', (_fonte, ganchos) => {
          for (const g of ganchos) botoesClicados.add(g);
        });
        await ctxNav.addInitScript(GRAVADOR);
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

        // O servidor morreu? Para AQUI, nomeando o caso depois do qual ele
        // caiu e mostrando a saida dele. Continuar so acrescenta reprovacoes
        // que dizem `ERR_CONNECTION_REFUSED` sem dizer por que.
        const codigo = servidor.morreuCom();
        if (codigo !== null) {
          diz(`\n${CORES.mal}o phxsqld caiu com codigo ${codigo} DEPOIS do caso `
            + `«${caso.nome}» [${tema}] — os casos seguintes nao teriam onde `
            + `acontecer.${CORES.fim}`);
          const cauda = servidor.saida.join('').split('\n').slice(-25).join('\n      ');
          diz(`${CORES.fraco}      saida do servidor (fim):\n      ${cauda}${CORES.fim}`);
          caiuOServidor = `${caso.nome} [${tema}], codigo ${codigo}`;
          break;
        }
      }
      if (caiuOServidor) break;
    }
  } finally {
    await navegador.close();
    await servidor.derrubar();
    diz(`${CORES.fraco}· servidor pid ${servidor.pid} derrubado${CORES.fim}`);
  }

  const maus = resultados.filter(r => r.falha);
  // A evidencia so se reescreve numa corrida INTEIRA E VERDE, e a segunda
  // metade desta frase custou uma corrida para aparecer: o `phxsqld` morreu no
  // meio de uma corrida cheia, os 41 casos seguintes reprovaram com
  // ERR_CONNECTION_REFUSED, e a gravacao aconteceu do mesmo jeito -- o arquivo
  // perdeu 110 ganchos e a catraca teria mandado escrever caso para botao que
  // ja tem prova. «Corrida inteira» nao quer dizer «corrida que chegou ao
  // fim»: quer dizer corrida que provou o que se propos a provar.
  const inteira = !opc.caso && !opc.tema && !caiuOServidor;
  if (inteira && !maus.length) {
    const { arq, quantas } = gravarEvidencia(botoesClicados);
    diz(`${CORES.fraco}· ${quantas} ganchos de botao gravados em ${arq}${CORES.fim}`);
  } else {
    diz(`${CORES.fraco}· ${inteira ? "corrida com falhas" : "corrida parcial"}:`
      + ` a evidencia de botoes NAO foi reescrita`
      + ` (${botoesClicados.size} ganchos nesta corrida)${CORES.fim}`);
  }

  diz(`\n${resultados.length - maus.length}/${resultados.length} casos passaram`);
  if (maus.length) {
    diz(`${CORES.mal}${maus.length} falharam:${CORES.fim}`);
    for (const m of maus) diz(`  ${m.caso} [${m.tema}]`);
  }
  if (caiuOServidor) {
    diz(`${CORES.mal}A CORRIDA PAROU: o phxsqld caiu em ${caiuOServidor}.${CORES.fim}`);
  }
  return maus.length || caiuOServidor ? 1 : 0;
}

principal()
  .then(c => process.exit(c))
  .catch(e => { console.error(`${CORES.mal}${e.stack || e.message}${CORES.fim}`); process.exit(2); });

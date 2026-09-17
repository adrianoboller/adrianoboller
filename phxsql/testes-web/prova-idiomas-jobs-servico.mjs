#!/usr/bin/env node
/* A PROVA das DUAS telas da leva 17 -- Jobs e Serviço -- pelo navegador, e
 * nao por leitura de codigo.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/prova-idiomas-jobs-servico.mjs --porta 7560 \
 *          --capturas /tmp/idiomas-jobs
 *
 * Irma da `prova-idiomas-telas.mjs`: o mesmo molde (cada passo comeca e
 * termina em portugues, com o retorno num `finally`), as mesmas afirmacoes
 * de sempre (o texto muda de verdade ao trocar de idioma, e a tela sem
 * escolha nenhuma continua a de sempre), e as TRES armadilhas proprias
 * desta leva, que ler o fonte nao pega:
 *
 *   1. o PAR `rot:`/`txt:` de duas listas lidas no ARRANQUE -- o
 *      `PINO_DO_ESTADO` (os pinos da grade) e o `JOBS_MODELO` (os seis
 *      modelos de pedido). Sem o par, o portugues fica guardado para sempre
 *      e a tela nunca troca, com o `txt(...)` la e resolvido cedo demais;
 *   2. a frase com enfase virada MARCA (`**assim**`, crase) e cortada em
 *      <b>/<code> DEPOIS da traducao, pelo `marcado()`. A prova confere que
 *      o `<code>` existe de verdade e que nao sobrou asterisco a mostra;
 *   3. a frase que era MEIA: as duas chaves do relogio terminavam em «: » e
 *      a tela concatenava o resto em portugues cravado. Ligar um job com o
 *      relogio parado dispara exatamente esse aviso, e ele tem de sair
 *      inteiro no idioma escolhido.
 *
 * Sobe um phxsqld proprio e o derruba pelo PID -- nunca `pkill -f`.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { Falha, verdade, contem, entrar, capturar, assentar, api } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const opc = {
  capturas: arg('--capturas'),
  porta: Number(arg('--porta', '7560')),
};

const VERDE = s => `\x1b[32m${s}\x1b[0m`;
const VERMELHO = s => `\x1b[31m${s}\x1b[0m`;
const CINZA = s => `\x1b[90m${s}\x1b[0m`;

let passos = 0;
let quebrados = 0;

async function passo(page, nome, corpo) {
  const t0 = Date.now();
  try {
    await trocarIdioma(page, 'Portugues');
    try {
      await corpo();
    } finally {
      await trocarIdioma(page, 'Portugues');
    }
    passos++;
    console.log(`  ${VERDE('ok    ')} ${nome.padEnd(56)} ${Date.now() - t0} ms`);
  } catch (e) {
    quebrados++;
    console.log(`  ${VERMELHO('QUEBROU')} ${nome}`);
    console.log(`         ${e instanceof Falha ? e.message : e.stack}`);
  }
}

/** Troca o idioma pelo MESMO caminho da pessoa -- a bandeira --, e nao por
 *  um `localStorage.setItem` por dentro. */
async function trocarIdioma(page, coluna) {
  await page.evaluate(c => escolherIdioma(c), coluna);
  await assentar(page, 400);
}

const painel = page =>
  page.$eval('#painel', e => e.textContent.replace(/\s+/g, ' ').trim()).catch(() => '');
const titulo = page =>
  page.$eval('#titulo', e => e.textContent.trim()).catch(() => '');
const subtitulo = page =>
  page.$eval('#subtitulo', e => e.textContent.trim()).catch(() => '');
const ultimoAviso = page =>
  page.$eval('#aviso', e => e.textContent.trim()).catch(() => '');

async function abrirJobs(page) {
  await page.evaluate(() => telaJobs());
  await page.waitForFunction(() => document.querySelector('#btJobNovo'), { timeout: 15000 });
  await assentar(page, 500);
}
async function abrirEditor(page, nome = null) {
  await page.evaluate(n => editarJob(n), nome);
  await page.waitForFunction(() => document.querySelector('#btJbGravar'), { timeout: 15000 });
  await assentar(page, 400);
}
async function abrirServico(page) {
  await page.evaluate(() => verServico());
  await page.waitForFunction(() => document.querySelector('#btSvSubir'), { timeout: 15000 });
  await assentar(page, 400);
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

  try {
    await entrar(page, srv.url);

    // ------------------------------------------------ a lista de Jobs, vazia
    await passo(page, 'Jobs vazio: titulo, quadro do e-mail e as tres notas', async () => {
      await abrirJobs(page);
      const pt = await painel(page);
      contem(pt, 'nenhum job cadastrado ainda', 'o vazio em portugues');
      contem(pt, 'Aviso por e-mail', 'o h3 do quadro');
      contem(pt, 'Este servidor não tem e-mail configurado', 'a redacao sem e-mail');
      contem(pt, 'Um job roda com o poder do usuário dele', 'a primeira nota');
      await capturar(ctx, 'jobs-vazio-pt');

      await trocarIdioma(page, 'Alemao');
      await abrirJobs(page);
      const de = await painel(page);
      contem(de, 'noch kein Job angelegt', 'o vazio em alemao');
      contem(de, 'Benachrichtigung per E-Mail', 'o h3 em alemao');
      contem(de, 'Dieser Server hat keine E-Mail eingerichtet', 'a redacao sem e-mail em alemao');
      contem(de, 'Ein Job läuft mit den Rechten seines Benutzers', 'a primeira nota em alemao');
      contem(de, 'Letzte Läufe', 'o h3 das corridas em alemao');
      verdade(!de.includes('Aviso por e-mail') && !de.includes('nenhum job'), 'sobrou portugues em alemao');
      // A marca virou etiqueta de verdade: `alertas.email` esta num <code>,
      // e nenhum `**` ficou a mostra.
      const codes = await page.$$eval('#painel .nota code', es => es.map(e => e.textContent));
      verdade(codes.includes('alertas.email'), `a crase nao virou <code>: ${JSON.stringify(codes)}`);
      verdade(!de.includes('**'), 'sobrou asterisco a mostra: a marca nao foi cortada');
      await capturar(ctx, 'jobs-vazio-de');
    });

    // -------------------------------------------------- a ficha de job nova
    await passo(page, 'ficha de job nova: rotulos, modelos (o par) e botoes', async () => {
      await abrirEditor(page);
      contem(await titulo(page), 'Novo job', 'o titulo em portugues');
      const pt = await painel(page);
      contem(pt, 'Modelo de pedido', 'o rotulo do select');
      contem(pt, 'Backup do servidor', 'o primeiro modelo (o par do JOBS_MODELO)');
      contem(pt, 'Criar o job', 'o botao de criar');
      await capturar(ctx, 'job-novo-pt');

      await trocarIdioma(page, 'Frances');
      await abrirEditor(page);
      contem(await titulo(page), 'Nouvelle tâche', 'o titulo em frances');
      const fr = await painel(page);
      contem(fr, 'Modèle de requête', 'o rotulo do select em frances');
      // Sem o par `rot:`/`txt:`, o JOBS_MODELO guardaria o portugues para
      // sempre -- e o codigo pareceria certo.
      contem(fr, 'Sauvegarde du serveur', 'o modelo em frances (o par do JOBS_MODELO)');
      contem(fr, 'Créer la tâche', 'o botao de criar em frances');
      contem(fr, '← Tâches', 'o botao de voltar em frances');
      verdade(!fr.includes('Backup do servidor') && !fr.includes('Modelo de pedido'), 'sobrou portugues na ficha em frances');
      const ph = await page.getAttribute('#jbUsuario', 'placeholder').catch(() => null);
      if (ph !== null) contem(ph, 'registre', 'o placeholder do usuario em frances');
      const codes = await page.$$eval('#painel .leg code', es => es.map(e => e.textContent));
      verdade(codes.includes('token'), `a crase da legenda nao virou <code>: ${JSON.stringify(codes)}`);
      await capturar(ctx, 'job-novo-fr');
    });

    // ------------------------------------- a grade com um job, e os pinos
    await passo(page, 'a grade de Jobs: pinos (o par), botoes e o aviso do relogio', async () => {
      await api(page, 'job_salvar', { job: {
        nome: 'prova_idiomas', descricao: 'só um pulso', usuario: 'adm',
        ligado: false, cada_minutos: 60, pedido: { op: 'ping' },
      } });
      await abrirJobs(page);
      const pt = await painel(page);
      contem(pt, 'nunca rodou', 'a ultima corrida em portugues');
      contem(pt, 'desligado', 'o pino de estado em portugues');
      await capturar(ctx, 'jobs-grade-pt');

      await trocarIdioma(page, 'Italiano');
      await abrirJobs(page);
      const it = await painel(page);
      contem(it, 'mai eseguito', 'a ultima corrida em italiano');
      contem(it, 'disattivato', 'o pino de estado em italiano (o par do PINO_DO_ESTADO)');
      contem(it, 'Esegui', 'o botao de rodar em italiano');
      contem(it, 'Attiva', 'o botao de ligar em italiano');
      contem(it, 'Elimina', 'o botao de excluir em italiano');
      // O subtitulo mora em `#subtitulo`, fora do `#painel`: a primeira
      // rodada desta prova o procurou no painel e reprovou a tela certa
      // pelo motivo errado.
      contem(await subtitulo(page), '1 job · registro in', 'o subtitulo em italiano');
      verdade(!it.includes('nunca rodou') && !it.includes('Rodar'), 'sobrou portugues na grade em italiano');
      await capturar(ctx, 'jobs-grade-it');

      // A frase que era MEIA: ligar o job com o relogio parado dispara o
      // aviso, e ele tem de sair INTEIRO em italiano.
      await page.click('.bt-ligar');
      await assentar(page, 800);
      const aviso = await ultimoAviso(page);
      contem(aviso, 'riavvia perché vada da solo', 'a segunda metade do aviso do relogio em italiano');
      verdade(!aviso.includes('reinicie'), 'a segunda metade do aviso continua em portugues');
      await capturar(ctx, 'jobs-aviso-relogio-it');
      await api(page, 'job_excluir', { nome: 'prova_idiomas' }).catch(() => {});
    });

    // ----------------------------------------------------------- o Serviço
    await passo(page, 'Serviço: fichas, aviso, notas e legenda', async () => {
      await abrirServico(page);
      const pt = await painel(page);
      contem(pt, 'porta de dados', 'a ficha em portugues');
      contem(pt, 'Trocar a porta, ou religar', 'o h3');
      contem(pt, 'Quem fica sem resposta.', 'a primeira nota');
      contem(pt, 'Trocar para esta porta', 'o botao com a porta no ar');
      await capturar(ctx, 'servico-pt');

      await trocarIdioma(page, 'Espanhol');
      await abrirServico(page);
      const es = await painel(page);
      contem(es, 'puerto de datos', 'a ficha em espanhol');
      contem(es, 'conexiones abiertas', 'a ficha das conexoes em espanhol');
      contem(es, 'Cambiar el puerto, o volver a subir', 'o h3 em espanhol');
      contem(es, 'Quién se queda sin respuesta.', 'a primeira nota em espanhol');
      contem(es, 'Cambiar a este puerto', 'o botao em espanhol');
      contem(es, 'Parar el puerto de datos', 'o botao de parar em espanhol');
      verdade(!es.includes('porta de dados') && !es.includes('Quem fica'), 'sobrou portugues no Servico em espanhol');
      const codes = await page.$$eval('#painel .nota code', es2 => es2.map(e => e.textContent));
      verdade(codes.includes('systemctl stop phxsql') && codes.includes('accept'),
        `as crases das notas nao viraram <code>: ${JSON.stringify(codes)}`);
      const negritos = await page.$$eval('#painel .nota b', es2 => es2.map(e => e.textContent));
      verdade(negritos.some(t => /^\d+$/.test(t)), `o {n} em negrito nao apareceu: ${JSON.stringify(negritos)}`);
      verdade(!es.includes('**'), 'sobrou asterisco a mostra no Servico');
      await capturar(ctx, 'servico-es');
    });

    // --------------------------------------- o comportamento VELHO, de novo
    await passo(page, 'sem escolher nada, as duas telas sao as de sempre', async () => {
      await page.evaluate(() => { localStorage.removeItem('phxsql-idioma'); });
      await entrar(page, srv.url);
      await page.waitForFunction(
        () => document.querySelector('#painel')?.textContent.includes('MEMÓRIA'),
        { timeout: 20000 });
      await abrirJobs(page);
      contem(await painel(page), 'Aviso por e-mail', 'Jobs sem escolha nenhuma tem de estar em portugues');
      await abrirServico(page);
      contem(await painel(page), 'Trocar a porta, ou religar', 'Servico sem escolha nenhuma tem de estar em portugues');
    });

    await passo(page, 'nenhum erro de JavaScript nas duas telas', async () => {
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
  console.error(e);
  process.exit(1);
});

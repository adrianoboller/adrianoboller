/* Os botoes do painel das OCORRENCIAS e do «explicar» (pedido 495, F9):
 *
 *   painel     #ocFiltrar
 *   grade      .bt-explicar-oc
 *   explicar   #iaOcNaoEnviar  #iaOcAprovar
 *
 * A ocorrencia nasce pelo caminho real, como na `prova-495-f9-ocorrencias.mjs`:
 * o `.reg` da tabela vira um DIRETORIO, o `inserir` recebe um `PhxError::Io`
 * do sistema operacional, o sumidouro marca `erro_de_disco` e o carteiro
 * grava a linha. O `.reg` volta logo depois.
 *
 * A Claude e a rota da Anthropic INTERCEPTADA (o mesmo caminho do caso 37):
 * nada sai da maquina, a chave e fabricada, e cada clique e conferido pelo
 * EFEITO -- quantos pedidos CHEGARAM a rota antes e depois de cada clique.
 * A prova inteira (chave, corpo sem literal, sem linha, sem login e sem IP, e
 * os defeitos repostos) e a da `prova-495-f9-ocorrencias.mjs`; aqui mora o
 * que alimenta `botoes-exercitados.txt`. */
import { renameSync, mkdirSync, rmdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';

import { entrar, api, verdade, igual, capturar, bancoDoCaso, Falha } from '../apoio.mjs';
import { definirIA } from '../claude-apoio.mjs';

const CHAVE = 'sk-ant-teste-FABRICADA-47F9A1B2C3D4';
const URL_API = 'https://api.anthropic.com/v1/messages';
const ESPERA = 20000;
let passo = 'inicio';

function sse(texto) {
  const ev = (tipo, dados) => `event: ${tipo}\ndata: ${JSON.stringify(dados)}\n\n`;
  return ev('message_start', { type: 'message_start', message: { id: 'm', type: 'message', role: 'assistant',
      model: 'roteiro', content: [], stop_reason: null, usage: { input_tokens: 9, output_tokens: 0 } } })
    + ev('content_block_start', { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } })
    + ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: texto } })
    + ev('content_block_stop', { type: 'content_block_stop', index: 0 })
    + ev('message_delta', { type: 'message_delta', delta: { stop_reason: 'end_turn' }, usage: { output_tokens: 4 } })
    + ev('message_stop', { type: 'message_stop' });
}

export const caso = {
  nome: 'botoes-das-ocorrencias',
  async rodar(ctx) {
    try {
      await corpo(ctx);
    } catch (e) {
      throw new Falha(`[passo: ${passo}] ${String(e.message).split('\n')[0]}`);
    }
  },
};

async function corpo(ctx) {
  const { page } = ctx;
  const db = bancoDoCaso(ctx, 'Oc');
  const chegou = [];
  await page.route(URL_API, async rota => {
    const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*',
                   'access-control-allow-methods': 'POST, OPTIONS' };
    if (rota.request().method() === 'OPTIONS') { await rota.fulfill({ status: 204, headers: CORS }); return; }
    chegou.push(rota.request().postData() || '');
    await rota.fulfill({ status: 200, headers: { ...CORS, 'content-type': 'text/event-stream' },
      body: sse('Isto é hipótese a CONFERIR, não diagnóstico.') });
  });
  await entrar(page, ctx.url);

  passo = 'a ocorrencia pelo caminho real';
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: db, tabela: 'quebra',
    colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  }).catch(() => {});
  await api(page, 'inserir', { database: db, tabela: 'quebra', valores: [1] }).catch(() => {});
  const reg = join(ctx.base, db, 'quebra.reg');
  verdade(existsSync(reg), `o .reg tem de existir: ${reg}`);
  renameSync(reg, `${reg}.guardado`);
  mkdirSync(reg);
  try {
    await api(page, 'inserir', { database: db, tabela: 'quebra', valores: [2] }).then(
      () => { throw new Falha('o inserir com o .reg quebrado foi aceito'); }, () => {});
  } finally {
    rmdirSync(reg);
    renameSync(`${reg}.guardado`, reg);
  }
  // O silencio da camada e por (alarme, usuario, IP) e dura um minuto: no
  // segundo tema a linha nova pode ter sido calada, e a do primeiro serve.
  let achou = false;
  for (let i = 0; i < 100 && !achou; i++) {
    const r = await api(page, 'ocorrencias', { alarme: 'erro_de_disco' });
    achou = (r.linhas || []).length > 0;
    if (!achou) await new Promise(f => setTimeout(f, 100));
  }
  verdade(achou, 'nenhuma ocorrencia erro_de_disco no ocorrencias.log');

  passo = 'o painel e o filtro';
  await definirIA(page, { chave: CHAVE, ligado: true });
  await page.click('[data-admin="ocorrencias"]');
  await page.waitForSelector('.bt-explicar-oc', { timeout: ESPERA });
  await page.selectOption('#ocAlarme', 'forca_bruta');
  await page.click('#ocFiltrar');
  await page.waitForFunction(() => !document.querySelector('.bt-explicar-oc')
    && document.querySelector('#gradeOcorrencias'), undefined, { timeout: ESPERA });
  await page.selectOption('#ocAlarme', 'erro_de_disco');
  await page.click('#ocFiltrar');
  await page.waitForSelector('.bt-explicar-oc', { timeout: ESPERA });
  await capturar(ctx, ctx.nomeCaptura('lista'));

  passo = 'explicar, e nao enviar';
  await page.click('.bt-explicar-oc');
  await page.waitForSelector('#iaOcNaoEnviar', { timeout: ESPERA });
  await page.waitForTimeout(400);
  igual(chegou.length, 0, 'pedidos a Anthropic com o painel de aprovacao aberto');
  await page.click('#iaOcNaoEnviar');
  await page.waitForSelector('#iaOcNaoEnviado', { timeout: ESPERA });
  igual(chegou.length, 0, 'pedidos a Anthropic depois de «Nao enviar»');

  passo = 'explicar, e aprovar';
  await page.click('.bt-explicar-oc');
  await page.waitForSelector('#iaOcAprovar', { timeout: ESPERA });
  await capturar(ctx, ctx.nomeCaptura('aprovar'));
  await page.click('#iaOcAprovar');
  await page.waitForSelector('#iaOcTokens b', { timeout: ESPERA });
  igual(chegou.length, 1, 'pedidos a Anthropic depois do clique');
  verdade(chegou[0].includes('erro_de_disco'), 'o corpo leva a ocorrencia');
  verdade(!chegou[0].includes('127.0.0.1'), 'o corpo nao leva o IP');
}

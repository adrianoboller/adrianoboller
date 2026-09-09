#!/usr/bin/env node
/* Servidor FALSO da API da Anthropic -- fala o contrato descrito em
 * `docs/CLAUDE-IA.md` SS2 (POST /v1/messages, com e sem stream, o SSE
 * pedaco a pedaco, e os erros que a SS2 lista) sem tocar rede nenhuma de
 * verdade.
 *
 * Existe porque o pedido 231 (docs/PENDENCIAS.md) achou tres baterias contra
 * um "servidor falso" RELATADAS em docs/CLAUDE-IA.md SS8 e NENHUM roteiro
 * versionado: rodaram numa sessao e morreram com ela, contra a lei da casa
 * ("script que resolveu algo nao pode morrer com a sessao"). Este arquivo e'
 * o roteiro que faltava.
 *
 * SO A `std` DO NODE -- nenhuma dependencia, nem para o HTTP nem para o SSE.
 * E' a mesma regra de "zero dependencias externas" do motor, aplicada aqui.
 *
 * ROTEIRO CONTROLAVEL POR ENDPOINT DE CONTROLE, e nao por variavel de
 * ambiente nem por argumento fixo na subida: cada `POST /v1/messages`
 * consome o PROXIMO item de uma fila que o teste enche por
 * `POST /_controle/roteiro` antes de clicar. Fila vazia cai no padrao
 * (sucesso, "ok"). E' o mesmo `veredito` de qualquer duble de teste -- aqui
 * so por HTTP, porque o cliente de verdade (claude.js) tambem so fala HTTP.
 *
 * O que ela GRAVA (`GET /_controle/pedidos`) e' metade da prova da SS3 do
 * documento: que a chave CHEGA ate aqui (senao a bateria nao mediria nada).
 * A outra metade -- que a chave NUNCA chega ao phxsqld -- se prova olhando
 * os pedidos que o phxsqld de verdade recebeu, e nao este arquivo.
 *
 * Uso standalone (fora da bateria, para conferir a mao com curl):
 *     node testes-web/claude-falsa.mjs --porta 6872
 */
import { createServer } from 'node:http';

// ---------------------------------------------------------------- o CONTRATO
// Um tipo de erro por codigo, para quando o roteiro so manda o CODIGO --
// caso mais comum de teste, que nao deveria ter que saber o vocabulario
// inteiro da API so para pedir um 429.
const TIPO_POR_CODIGO = {
  400: 'invalid_request_error',
  401: 'authentication_error',
  402: 'billing_error',
  403: 'permission_error',
  404: 'not_found_error',
  413: 'request_too_large',
  429: 'rate_limit_error',
  500: 'api_error',
  529: 'overloaded_error',
};

/** Parte um texto em N pedacos -- e' o que faz o streaming te ter MAIS de um
 *  `content_block_delta`, que e' o efeito que a prova "aparece aos pedacos"
 *  precisa observar (e nao um relogio fixo -- ver a licao da SS9 do
 *  documento). */
function partir(texto, pedacos) {
  const n = Math.max(1, pedacos | 0);
  const tam = Math.max(1, Math.ceil(texto.length / n));
  const partes = [];
  for (let i = 0; i < texto.length; i += tam) partes.push(texto.slice(i, i + tam));
  return partes.length ? partes : [''];
}

const dormir = ms => new Promise(r => setTimeout(r, ms));

export function criarFalsa() {
  let fila = [];
  let pedidos = [];
  const padraoDeFabrica = () => ({
    resposta: 'sucesso', texto: 'ok', tokensEntrada: 5, tokensSaida: 3,
    pedacos: 4, atrasoMs: 12,
  });
  let padrao = padraoDeFabrica();

  const proximoRoteiro = () => (fila.length ? fila.shift() : padrao);

  function enviarErro(res, codigo, tipo, mensagem) {
    const corpo = JSON.stringify({
      type: 'error',
      error: { type: tipo || TIPO_POR_CODIGO[codigo] || 'api_error', message: mensagem || `erro ${codigo}` },
    });
    res.writeHead(codigo, {
      'content-type': 'application/json',
      'access-control-allow-origin': res.__origem || '*',
    });
    res.end(corpo);
  }

  function enviarSucesso(res, roteiro) {
    // A forma de uma resposta NAO streaming da API de verdade. claude.js
    // sempre manda `stream:true` (o `corpo()` do arquivo e' fixo nisso), mas
    // um duble que so entendesse streaming nao falaria o CONTRATO -- so uma
    // parte dele.
    const corpo = JSON.stringify({
      id: 'msg_falsa_' + Math.random().toString(36).slice(2),
      type: 'message',
      role: 'assistant',
      model: 'claude-falsa',
      content: [{ type: 'text', text: roteiro.texto ?? '' }],
      stop_reason: 'end_turn',
      stop_sequence: null,
      usage: { input_tokens: roteiro.tokensEntrada ?? 0, output_tokens: roteiro.tokensSaida ?? 0 },
    });
    res.writeHead(200, {
      'content-type': 'application/json',
      'access-control-allow-origin': res.__origem || '*',
    });
    res.end(corpo);
  }

  async function streamDeSucesso(res, roteiro) {
    res.writeHead(200, {
      'content-type': 'text/event-stream; charset=utf-8',
      'access-control-allow-origin': res.__origem || '*',
      'cache-control': 'no-cache',
    });
    const ev = (tipo, dados) => res.write(`event: ${tipo}\ndata: ${JSON.stringify(dados)}\n\n`);

    ev('message_start', {
      type: 'message_start',
      message: {
        id: 'msg_falsa_' + Math.random().toString(36).slice(2), type: 'message', role: 'assistant',
        model: 'claude-falsa', content: [], stop_reason: null, stop_sequence: null,
        usage: { input_tokens: roteiro.tokensEntrada ?? 0, output_tokens: 0 },
      },
    });
    ev('content_block_start', { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } });

    const pedacos = partir(String(roteiro.texto ?? ''), roteiro.pedacos ?? 4);
    for (const pedaco of pedacos) {
      if (roteiro.atrasoMs) await dormir(roteiro.atrasoMs);
      ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: pedaco } });
    }

    ev('content_block_stop', { type: 'content_block_stop', index: 0 });
    ev('message_delta', {
      type: 'message_delta',
      delta: { stop_reason: 'end_turn', stop_sequence: null },
      usage: { output_tokens: roteiro.tokensSaida ?? pedacos.length },
    });
    ev('message_stop', { type: 'message_stop' });
    res.end();
  }

  /** O erro que chega NO MEIO do fluxo -- HTTP 200 e depois um
   *  `event: error`, que a SS2 do documento diz que so aparece a quem olha
   *  o EVENTO e nao o codigo de status (que ja veio 200). */
  async function streamComErroNoMeio(res, roteiro) {
    res.writeHead(200, {
      'content-type': 'text/event-stream; charset=utf-8',
      'access-control-allow-origin': res.__origem || '*',
      'cache-control': 'no-cache',
    });
    const ev = (tipo, dados) => res.write(`event: ${tipo}\ndata: ${JSON.stringify(dados)}\n\n`);
    ev('message_start', {
      type: 'message_start',
      message: {
        id: 'msg_falsa_' + Math.random().toString(36).slice(2), type: 'message', role: 'assistant',
        model: 'claude-falsa', content: [], stop_reason: null, stop_sequence: null,
        usage: { input_tokens: roteiro.tokensEntrada ?? 0, output_tokens: 0 },
      },
    });
    ev('content_block_start', { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } });
    const pedacos = partir(String(roteiro.texto ?? ''), roteiro.pedacos ?? 2);
    for (const pedaco of pedacos) {
      if (roteiro.atrasoMs) await dormir(roteiro.atrasoMs);
      ev('content_block_delta', { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: pedaco } });
    }
    // E aqui o fio quebra: nada de content_block_stop nem message_stop --
    // e' exatamente o que uma sobrecarga de verdade faz no meio da resposta.
    ev('error', {
      type: 'error',
      error: { type: roteiro.tipoErro || 'overloaded_error', message: roteiro.mensagemErro || 'sobrecarregado no meio da resposta' },
    });
    res.end();
  }

  async function tratarMensagens(req, res, corpoTexto) {
    const cabecalhos = {};
    for (const [k, v] of Object.entries(req.headers)) cabecalhos[k.toLowerCase()] = v;
    res.__origem = cabecalhos.origin || '*';

    let corpoJson = null;
    try { corpoJson = JSON.parse(corpoTexto || '{}'); } catch { /* fica null: registrado assim mesmo */ }

    // GRAVA ANTES de julgar -- inclusive o pedido que vai ser recusado, para
    // o roteiro conseguir provar "o pedido chegou e foi recusado", e nao so
    // "nao houve pedido nenhum".
    pedidos.push({
      quando: new Date().toISOString(),
      metodo: req.method,
      caminho: req.url,
      cabecalhos,
      corpo: corpoJson,
      chaveVista: cabecalhos['x-api-key'] || null,
    });

    if (!cabecalhos['x-api-key']) {
      return enviarErro(res, 401, 'authentication_error', 'x-api-key ausente');
    }
    if (cabecalhos['anthropic-version'] !== '2023-06-01') {
      return enviarErro(res, 400, 'invalid_request_error',
        `anthropic-version invalida ou ausente: ${cabecalhos['anthropic-version'] || '(nenhuma)'}`);
    }
    if (cabecalhos['anthropic-dangerous-direct-browser-access'] !== 'true') {
      // A API de verdade recusaria isto por CORS, antes mesmo de chegar num
      // servidor -- aqui a mesma recusa acontece do lado de ca, para o
      // roteiro conseguir provar "sem o cabecalho, nada funciona" sem
      // depender do preflight exato do Chromium.
      return enviarErro(res, 403, 'permission_error',
        'faltou anthropic-dangerous-direct-browser-access: true');
    }

    const roteiro = proximoRoteiro();

    if (roteiro.resposta === 'erro') {
      return enviarErro(res, roteiro.codigo || 500, roteiro.tipo, roteiro.mensagem);
    }

    const streaming = !!(corpoJson && corpoJson.stream === true);

    if (roteiro.resposta === 'erro_no_meio') {
      if (!streaming) {
        return enviarErro(res, 400, 'invalid_request_error',
          'roteiro "erro_no_meio" pedido sem "stream": true no corpo');
      }
      return streamComErroNoMeio(res, roteiro);
    }

    if (streaming) return streamDeSucesso(res, roteiro);
    return enviarSucesso(res, roteiro);
  }

  function tratarControle(req, res, caminho, corpoTexto) {
    const cab = { 'content-type': 'application/json', 'access-control-allow-origin': '*' };
    if (req.method === 'OPTIONS') { res.writeHead(204, cabecalhosCors('*')); res.end(); return; }

    if (caminho === '/_controle/roteiro' && req.method === 'POST') {
      let item;
      try { item = JSON.parse(corpoTexto || '{}'); }
      catch (e) { res.writeHead(400, cab); res.end(JSON.stringify({ erro: 'JSON invalido: ' + e.message })); return; }
      fila.push(item);
      res.writeHead(200, cab);
      res.end(JSON.stringify({ ok: true, na_fila: fila.length }));
      return;
    }
    if (caminho === '/_controle/padrao' && req.method === 'POST') {
      let item;
      try { item = JSON.parse(corpoTexto || '{}'); }
      catch (e) { res.writeHead(400, cab); res.end(JSON.stringify({ erro: 'JSON invalido: ' + e.message })); return; }
      padrao = item;
      res.writeHead(200, cab);
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    if (caminho === '/_controle/pedidos' && req.method === 'GET') {
      res.writeHead(200, cab);
      res.end(JSON.stringify({ pedidos }));
      return;
    }
    if (caminho === '/_controle/reset' && req.method === 'POST') {
      fila = [];
      pedidos = [];
      padrao = padraoDeFabrica();
      res.writeHead(200, cab);
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    res.writeHead(404, cab);
    res.end(JSON.stringify({ erro: `rota de controle desconhecida: ${req.method} ${caminho}` }));
  }

  function cabecalhosCors(origem) {
    return {
      'access-control-allow-origin': origem || '*',
      'access-control-allow-methods': 'POST, GET, OPTIONS',
      // Os quatro cabecalhos que claude.js manda -- sem os tres do protocolo
      // no allow-headers, o PREFLIGHT do navegador reprova antes mesmo de a
      // pagina tentar, e o defeito pareceria "rede", nao CORS.
      'access-control-allow-headers': 'content-type, x-api-key, anthropic-version, anthropic-dangerous-direct-browser-access',
      'access-control-max-age': '600',
    };
  }

  const servidor = createServer((req, res) => {
    const url = new URL(req.url, 'http://localhost');
    const caminho = url.pathname;
    const pedacos = [];
    req.on('data', d => pedacos.push(d));
    req.on('end', () => {
      const corpoTexto = Buffer.concat(pedacos).toString('utf8');
      if (req.method === 'OPTIONS') {
        res.writeHead(204, cabecalhosCors(req.headers.origin || '*'));
        res.end();
        return;
      }
      if (caminho === '/v1/messages' && req.method === 'POST') {
        Object.assign(res, { __origem: req.headers.origin || '*' });
        tratarMensagens(req, res, corpoTexto).catch(e => {
          try { res.writeHead(500, { 'content-type': 'application/json' }); } catch { /* ja escreveu */ }
          res.end(JSON.stringify({ type: 'error', error: { type: 'api_error', message: 'falha interna do falso: ' + e.message } }));
        });
        return;
      }
      if (caminho.startsWith('/_controle/')) { tratarControle(req, res, caminho, corpoTexto); return; }
      res.writeHead(404, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ erro: `rota desconhecida: ${req.method} ${caminho}` }));
    });
  });

  return {
    servidor,
    definirRoteiro: item => fila.push(item),
    definirPadrao: item => { padrao = item; },
    lerPedidos: () => pedidos.slice(),
    limpar: () => { fila = []; pedidos = []; padrao = padraoDeFabrica(); },
  };
}

/** Sobe a falsa numa porta e devolve o que a bateria precisa para falar com
 *  ela por dentro (chamada direta, sem HTTP) e por fora (o endpoint de
 *  controle, para quem quiser rodar `curl` a mao). */
export async function subirFalsa({ porta, log }) {
  const falsa = criarFalsa();
  await new Promise((resolve, reject) => {
    falsa.servidor.once('error', reject);
    falsa.servidor.listen(porta, '127.0.0.1', resolve);
  });
  if (log) log(`falso da Anthropic escutando em 127.0.0.1:${porta}`);
  return {
    url: `http://127.0.0.1:${porta}`,
    endpointMensagens: `http://127.0.0.1:${porta}/v1/messages`,
    porta,
    definirRoteiro: falsa.definirRoteiro,
    definirPadrao: falsa.definirPadrao,
    lerPedidos: falsa.lerPedidos,
    limpar: falsa.limpar,
    async derrubar() { await new Promise(r => falsa.servidor.close(r)); },
  };
}

// ------------------------------------------------------------ uso standalone
if (import.meta.url === `file://${process.argv[1]}`) {
  const arg = (nome, padrao) => {
    const i = process.argv.indexOf(nome);
    return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
  };
  const porta = Number(arg('--porta', '6872'));
  const s = await subirFalsa({ porta, log: console.log });
  console.log(`POST ${s.endpointMensagens}`);
  console.log(`controle: POST ${s.url}/_controle/roteiro | GET ${s.url}/_controle/pedidos | POST ${s.url}/_controle/reset`);
}

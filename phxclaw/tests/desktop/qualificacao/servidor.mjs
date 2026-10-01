// Servidor de revisao: serve a COPIA da UI e simula a API (/v1) com modos trocaveis.
import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';

const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

export function subir(raiz, dados) {
  const modo = { tarefas: 'normal', config: 'normal', semjson: false, semtextos: false, atraso: 0 };
  const ESTADOS = ['pending', 'awaiting_approval', 'awaiting_input', 'running', 'completed', 'failed', 'cancelled'];
  const TAREFAS = ESTADOS.map((status, i) => ({
    id: `t${i}`, objective: `Objetivo da tarefa ${i}: revisar o relatório de Blumenau e consolidar números`, status, model: 'falso',
    plan: status === 'awaiting_approval' ? [{ descricao: 'passo 1' }, { descricao: 'passo 2' }] : [],
    question: status === 'awaiting_input' ? 'Qual cor?' : undefined,
    steps: [], artifacts: [], created_at: `2026-10-01T0${(i * 3) % 7}:00:00Z`, updated_at: '2026-10-01T09:00:00Z',
  }));
  const srv = createServer(async (req, res) => {
    const u = new URL(req.url, 'http://x');
    const p = decodeURIComponent(u.pathname);
    if (p === '/__modo') { for (const [k, v] of u.searchParams) modo[k] = v === 'true' ? true : v === 'false' ? false : (isNaN(+v) ? v : +v); res.end(JSON.stringify(modo)); return; }
    if (modo.atraso) await new Promise(r => setTimeout(r, modo.atraso));
    const json = (st, b) => { res.writeHead(st, { 'content-type': 'application/json' }); res.end(JSON.stringify(b)); };
    if (p.startsWith('/v1/')) {
      if (p.startsWith('/v1/tasks')) {
        if (modo.tarefas === 'erro') return json(500, { error: 'falha interna simulada' });
        if (modo.tarefas === '401') return json(401, { error: 'unauthorized' });
        if (modo.tarefas === 'lento') await new Promise(r => setTimeout(r, 6000));
        const lista = modo.tarefas === 'vazio' ? [] : TAREFAS;
        const m = p.match(/^\/v1\/tasks\/(t\d)$/);
        if (m) { const t = lista.find(x => x.id === m[1]); return json(t ? 200 : 404, t || { error: 'nao existe' }); }
        if (p === '/v1/tasks') return json(200, lista);
        return json(200, {});
      }
      if (p === '/v1/config') {
        if (modo.config === 'erro') return json(500, { error: 'falha interna simulada' });
        if (modo.config === 'lento') await new Promise(r => setTimeout(r, 6000));
        return json(200, dados.config);
      }
      return json(404, { error: 'nao existe' });
    }
    const arq = join(raiz, p === '/' ? 'index.html' : p);
    const base = p.split('/').pop();
    if (modo.semjson && ['equipe.json', 'ferramentas.json', 'absorcao.json', 'config-catalogo.json'].includes(base)) { res.writeHead(404); res.end('nao existe'); return; }
    if (modo.semtextos && base === 'textos.json') { res.writeHead(404); res.end('nao existe'); return; }
    if (!arq.startsWith(raiz) || !existsSync(arq) || statSync(arq).isDirectory()) { res.writeHead(404); res.end('nao existe'); return; }
    res.writeHead(200, { 'content-type': TIPOS[extname(arq)] || 'application/octet-stream' });
    res.end(readFileSync(arq));
  });
  return new Promise(r => srv.listen(0, '127.0.0.1', () => r({ srv, porta: srv.address().port, modo })));
}

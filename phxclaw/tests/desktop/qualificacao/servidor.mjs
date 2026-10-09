// Servidor de revisao: serve a COPIA da UI e simula a API (/v1) com modos trocaveis.
import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';
import { cabecalhosDaUi, ROTA_POLITICA, POLITICA_PADRAO } from '../seguranca.mjs';

// Os cabecalhos de seguranca do agente (pwa.rs) em toda resposta, como o `servir` manda.
const CABECALHOS = cabecalhosDaUi();

const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

export function subir(raiz, dados) {
  const modo = { tarefas: 'normal', config: 'normal', semjson: false, semtextos: false, atraso: 0 };
  const ESTADOS = ['pending', 'awaiting_approval', 'awaiting_input', 'running', 'completed', 'failed', 'cancelled', 'budget_exceeded'];
  const TAREFAS = ESTADOS.map((status, i) => ({
    id: `t${i}`, objective: `Objetivo da tarefa ${i}: revisar o relatório de Blumenau e consolidar números`, status, model: 'falso',
    plan: status === 'awaiting_approval' ? [{ descricao: 'passo 1' }, { descricao: 'passo 2' }] : [],
    question: status === 'awaiting_input' ? 'Qual cor?' : undefined,
    steps: [], artifacts: [], created_at: `2026-10-01T0${(i * 3) % 7}:00:00Z`, updated_at: '2026-10-01T09:00:00Z',
  }));
  const perfis = { ativo: null, lista: [{ nome: 'trabalho', chaves: { 'agente.modelo': 'ollama:qwen2.5:7b' } }] };
  const PERFIS = () => ({ ativo: perfis.ativo, origem_do_ativo: perfis.ativo ? 'arquivo' : null, lista: perfis.lista.map(x => ({ ...x, ativo: x.nome === perfis.ativo })) });
  // A ficha de loja.rs::ficha (nome, versao, publicador, categorias, capacidades, instalado).
  const plugins = [{ nome: 'phx-tema', versao: '1.0.0', publicador: 'phoenix', categorias: ['tema'], capacidades: [], instalado: false },
    { nome: 'phx-sql', versao: '0.18.0', publicador: 'phoenix', categorias: ['dados'], capacidades: ['sql.query'], instalado: true },
    { nome: 'x-rascunho', versao: '0.1.0', publicador: 'alguem', categorias: [], capacidades: [], instalado: false, assinado: false }];
  const srv = createServer(async (req, res) => {
    const u = new URL(req.url, 'http://x');
    const p = decodeURIComponent(u.pathname);
    for (const [k, v] of Object.entries(CABECALHOS)) res.setHeader(k, v);
    if (p === ROTA_POLITICA) { res.writeHead(200, { 'content-type': 'application/json' }); res.end(POLITICA_PADRAO); return; }
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
        return json(200, { ...dados.config, perfis: PERFIS() });
      }
      // Os paineis da onda 2 do VS Code (testes, plugins, perfis), com o contrato do motor.
      if (p === '/v1/config/perfis') {
        if (req.method === 'PUT') {
          let corpo = '';
          for await (const c of req) corpo += c;
          const b = JSON.parse(corpo || '{}');
          if (req.headers['if-match'] !== String(dados.config.revisao)) return json(409, { revisao_atual: dados.config.revisao });
          if (b.criar) perfis.lista.push({ nome: b.criar, chaves: b.copiar_base ? { 'agente.modelo': 'ollama:x' } : {} });
          if ('usar' in b) perfis.ativo = b.usar;
          dados.config.revisao += 1;
          return json(200, { revisao: dados.config.revisao });
        }
        return json(200, PERFIS());
      }
      if (p === '/v1/ide/testes') return json(200, { linguagem: 'rust', projeto: '.', total: 3, crates: [{ crate: 'calc', modulos: [{ modulo: 'soma', testes: ['dois_mais_dois', 'zero'] }, { modulo: 'raiz', testes: ['raiz'] }] }] });
      if (p === '/v1/ide/testes/rodar') { let c = ''; for await (const x of req) c += x; const no = JSON.parse(c || '{}').no; return json(200, { linguagem: 'rust', projeto: '.', no, passou: !/zero/.test(no), ok: /zero/.test(no) ? 0 : 1, falhou: /zero/.test(no) ? 1 : 0, resultados: [`test ${no} ... ${/zero/.test(no) ? 'FAILED' : 'ok'}`], stderr_cauda: '' }); }
      if (p === '/v1/plugins/catalogo') return json(200, { plugins: plugins });
      if (p === '/v1/plugins/instalar') { let c = ''; for await (const x of req) c += x; const nome = JSON.parse(c || '{}').nome; const pl = plugins.find(x => x.nome === nome); if (!pl) return json(422, { error: 'plugin inexistente' }); pl.instalado = true; return json(200, { nome, versao: pl.versao, caminho: `/plugins/${nome}` }); }
      if (p === '/v1/ide/simbolos') return json(200, { arquivo: u.searchParams.get('arquivo'), simbolos: [{ nome: 'main', tipo: 'funcao', linha: 1, filhos: [] }, { nome: 'Config', tipo: 'struct', linha: 7, filhos: [{ nome: 'porta', tipo: 'campo', linha: 8, filhos: [] }] }] });
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

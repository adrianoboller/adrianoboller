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
  // Conversa (projeto -> pedido -> cartao): tarefas carimbadas por projeto (X-PhxClaw-Projeto),
  // como o RBAC do servidor faz. Sem o cabecalho, nada disto muda -- a lista velha segue igual.
  //   * `criadas`: as tarefas que o POST /v1/tasks cria (viram cartao na coluna Backlog/Fazendo);
  //     completam na 2a leitura do detalhe, para o roteiro ver o «trabalhando» virar resposta.
  //   * `curadas(proj)`: um retrato fixo por projeto, cobrindo as quatro colunas e uma tarefa
  //     RICA (passos de varias ferramentas, historico, ajustes, artefatos) para o detalhe.
  // Os campos `historico` e `ajustes` sao os que o engenheiro vai pôr no modelo; aqui sao stub.
  const criadas = {};
  const curadasPorProj = new Map();
  function curadas(proj) {
    if (!curadasPorProj.has(proj)) {
      const rico = {
        id: `${proj}~k3`, projeto: proj, objective: 'Resumir os documentos de Blumenau e consolidar números', status: 'completed',
        model: 'ollama:qwen2.5:7b', parent: null, created_at: '2026-10-10T14:02:00Z', updated_at: '2026-10-10T14:09:00Z',
        answer: 'Resumo pronto com os números consolidados.', error: null,
        plan: ['ler os documentos', 'resumir', 'entregar o relatório'],
        steps: [
          { n: 1, tool: 'web.search', outcome: 'ok', summary: 'buscou fontes' },
          { n: 2, tool: 'web.search', outcome: 'ok', summary: 'buscou mais fontes' },
          { n: 3, tool: 'fs.read', outcome: 'ok', summary: 'leu os documentos' },
          { n: 4, tool: 'fs.write', outcome: 'ok', summary: 'escreveu o relatório' },
          { n: 5, tool: null, outcome: 'ok', summary: 'pensou no formato' },
        ],
        historico: [
          { estado: 'pending', em: '2026-10-10T14:02:00Z' },
          { estado: 'running', em: '2026-10-10T14:03:00Z' },
          { estado: 'completed', em: '2026-10-10T14:09:00Z' },
        ],
        ajustes: [
          { texto: 'Inclua o total por mês', em: '2026-10-10T14:05:00Z' },
          { texto: 'Use reais, não dólares', em: '2026-10-10T14:06:00Z' },
        ],
        artifacts: [{ path: 'index.html' }, { path: 'relatorio.md' }, { path: 'dados/tabela.csv' }],
      };
      const base = { projeto: proj, model: 'ollama:qwen2.5:7b', parent: null, plan: [], steps: [], historico: [], ajustes: [], artifacts: [], answer: null, error: null, updated_at: '2026-10-10T14:00:00Z' };
      curadasPorProj.set(proj, [
        { ...base, id: `${proj}~k0`, objective: 'Pesquisar fornecedores de nuvem', status: 'pending', created_at: '2026-10-10T13:00:00Z', historico: [{ estado: 'pending', em: '2026-10-10T13:00:00Z' }] },
        { ...base, id: `${proj}~k1`, objective: 'Gerar o relatório trimestral', status: 'running', created_at: '2026-10-10T13:10:00Z', plan: ['coletar', 'calcular', 'desenhar', 'revisar'], steps: [{ n: 1, tool: 'fs.read', outcome: 'ok', summary: 'leu' }, { n: 2, tool: 'shell.run', outcome: 'ok', summary: 'rodou' }], historico: [{ estado: 'pending', em: '2026-10-10T13:10:00Z' }, { estado: 'running', em: '2026-10-10T13:11:00Z' }] },
        { ...base, id: `${proj}~k2`, objective: 'Qual formato de saída prefere?', status: 'awaiting_input', question: 'PDF ou DOCX?', created_at: '2026-10-10T13:20:00Z', historico: [{ estado: 'pending', em: '2026-10-10T13:20:00Z' }, { estado: 'running', em: '2026-10-10T13:21:00Z' }, { estado: 'awaiting_input', em: '2026-10-10T13:22:00Z' }] },
        rico,
        { ...base, id: `${proj}~k4`, objective: 'Converter a planilha antiga', status: 'failed', error: 'formato não reconhecido', created_at: '2026-10-10T13:30:00Z', historico: [{ estado: 'pending', em: '2026-10-10T13:30:00Z' }, { estado: 'running', em: '2026-10-10T13:31:00Z' }, { estado: 'failed', em: '2026-10-10T13:33:00Z' }] },
        { ...base, id: `${proj}~k5`, objective: 'Subtarefa: baixar os PDFs', status: 'running', parent: `${proj}~k3`, created_at: '2026-10-10T14:04:00Z', historico: [{ estado: 'pending', em: '2026-10-10T14:04:00Z' }, { estado: 'running', em: '2026-10-10T14:05:00Z' }] },
        // Feito, mas SEM artefato web (so .txt): o «Testar» nao pode aparecer para esta.
        { ...base, id: `${proj}~k6`, objective: 'Exportar os números para texto', status: 'completed', created_at: '2026-10-10T13:40:00Z', answer: 'Exportado.', artifacts: [{ path: 'saida.txt' }], historico: [{ estado: 'pending', em: '2026-10-10T13:40:00Z' }, { estado: 'completed', em: '2026-10-10T13:42:00Z' }] },
        // Mais duas em execucao: a coluna «Fazendo» passa de 3 cartoes e precisa rolar por dentro.
        { ...base, id: `${proj}~k7`, objective: 'Indexar a base de conhecimento', status: 'running', created_at: '2026-10-10T13:50:00Z', historico: [{ estado: 'pending', em: '2026-10-10T13:50:00Z' }, { estado: 'running', em: '2026-10-10T13:51:00Z' }] },
        { ...base, id: `${proj}~k8`, objective: 'Monitorar o custo das execuções', status: 'running', created_at: '2026-10-10T13:55:00Z', historico: [{ estado: 'pending', em: '2026-10-10T13:55:00Z' }, { estado: 'running', em: '2026-10-10T13:56:00Z' }] },
      ]);
    }
    return curadasPorProj.get(proj);
  }
  const resumo = t => ({ id: t.id, objective: t.objective, status: t.status, model: t.model, created_at: t.created_at, updated_at: t.updated_at, answer: t.answer, error: t.error, steps: (t.steps || []).length, artifacts: t.artifacts || [], parent: t.parent ?? null, projeto: t.projeto });
  const acharTarefa = (id, proj) => (proj ? curadas(proj) : []).find(t => t.id === id) || criadas[id] || null;

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
        // A voz (STT): o corpo e o audio WAV; a resposta e {texto} ou {error} com codigo. O
      // `modo.voz` troca para um codigo de erro (413/415/422/503/502/504) para exercitar o aviso.
      if (p === '/v1/voz/transcrever') {
        const ct = req.headers['content-type'] || '';
        if (modo.voz && modo.voz !== 'ok') { const st = +modo.voz; return json(st, { error: st === 503 ? 'motor de voz offline (whisper) não configurado' : st === 422 ? 'áudio vazio' : `erro ${st}` }); }
        if (!/^audio\/(wav|webm)/.test(ct)) return json(415, { error: `content-type não aceito: ${ct}` });
        return json(200, { texto: 'olá mundo' });
      }
      // Insights: a tela existe no menu, entao a varredura que clica todas as telas (ui_navegacao)
      // a abre; sem esta rota o GET /v1/insights dava 404 e sujava o console. Retrato vazio e valido.
      if (p === '/v1/insights') return json(200, { total: 0, terminadas: 0, taxa_falha: 0, falhas: 0, duracao: { p50_ms: null, p95_ms: null, amostras: 0 }, custo: { total: null, nao_medidas: 0 }, por_estado: {}, falhas_comuns: [], dias: [], fluxos: [], desde: null, ate: '2026-10-10T00:00:00Z' });
      // Artefato por caminho: o conteudo cai pela rota de arquivos da tarefa (a mesma da tela
      // Tarefas e do «Testar»). HTML vira texto/html; o resto, octeto.
      const art = p.match(/^\/v1\/tasks\/([^/]+)\/artifacts\/(.+)$/);
      if (art) {
        const html = /\.html?$/i.test(art[2]);
        res.writeHead(200, { 'content-type': html ? 'text/html' : 'application/octet-stream' });
        res.end(html ? `<!doctype html><title>Jogo da forca</title><h1>Jogo da forca</h1><p>${art[1]}/${art[2]}</p>` : `conteudo de ${art[2]}`);
        return;
      }
      if (p.startsWith('/v1/tasks')) {
        if (modo.tarefas === 'erro') return json(500, { error: 'falha interna simulada' });
        if (modo.tarefas === '401') return json(401, { error: 'unauthorized' });
        if (modo.tarefas === 'lento') await new Promise(r => setTimeout(r, 6000));
        const proj = req.headers['x-phxclaw-projeto'];
        const lista = modo.tarefas === 'vazio' ? [] : TAREFAS;
        // Criar (conversa): carimba o projeto do cabecalho e nasce EM EXECUCAO; completa na 2a
        // leitura do detalhe, para o roteiro ver o «trabalhando» virar resposta.
        if (p === '/v1/tasks' && req.method === 'POST') {
          let c = ''; for await (const x of req) c += x;
          const b = JSON.parse(c || '{}');
          const id = `c${Object.keys(criadas).length}`;
          const agora = new Date().toISOString();
          criadas[id] = { id, projeto: proj, objective: b.objective || '', status: 'running', model: 'ollama:qwen2.5:7b', parent: null,
            created_at: agora, updated_at: agora, answer: null, error: null, plan: ['entender', 'fazer'], steps: [{ n: 1, tool: 'web.search', outcome: 'ok', summary: 'buscou' }],
            historico: [{ estado: 'pending', em: agora }, { estado: 'running', em: agora }], ajustes: [], artifacts: [], gets: 0 };
          return json(202, { id });
        }
        const m = p.match(/^\/v1\/tasks\/([^/]+)$/);
        if (m) {
          const t = acharTarefa(m[1], proj) || (m[1].startsWith('t') ? lista.find(x => x.id === m[1]) : null);
          if (!t) return json(404, { error: 'nao existe' });
          if (t.gets !== undefined) { t.gets += 1; if (t.gets >= 2 && t.status === 'running') { t.status = 'completed'; t.answer = 'Pronto: ' + t.objective; t.historico.push({ estado: 'completed', em: new Date().toISOString() }); } }
          return json(200, t);
        }
        if (p === '/v1/tasks') {
          // Com o cabecalho de projeto (como o RBAC), a lista chega CORTADA por projeto: o
          // retrato curado + as criadas daquele projeto. Sem cabecalho, a lista velha, igual.
          if (proj) return json(200, [...curadas(proj), ...Object.values(criadas).filter(t => t.projeto === proj)].map(resumo));
          return json(200, lista);
        }
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

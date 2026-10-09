// Service worker do aplicativo instalavel. Guarda SO a casca da tela (HTML, CSS, JS,
// icones, textos) para abrir sem rede; a API (/v1/) vai sempre a rede e nunca entra no
// cache -- tarefa vista do cache seria estado velho apresentado como atual.
//
// A casca e servida da rede primeiro e do cache so quando a rede falha: assim a versao
// nova chega na primeira visita com rede, sem esperar um segundo carregamento.
// O nome do cache muda quando a casca ganha arquivo: o `activate` apaga o anterior, e o
// celular instalado nao fica com uma casca sem a grade.
//
// Casca 3: os tres JSON gerados que as telas leem e a fonte da marca entraram. Sem eles a
// PRIMEIRA visita sem rede mandava «rodar cargo» (o arquivo existia; quem caiu foi a rede) e
// a tela caia na fonte do sistema (qualificacao de 01/10/2026, G7).
// Casca 4: a IBM Plex Mono (dado em mono, Style Phoenix Padrao) entrou, local como a Exo 2.
// Casca 6: o tema.js (tema claro do Style Phoenix Padrao) entrou.
// Casca 7: a identidade virou as artes do dono (01/10/2026): sai o phoenix-mark.svg, entram
// a marca do topo, os favicons, a abertura nos dois temas e o icone mascaravel.
// Casca 8: o paineis.js (explorador de testes, loja de plugins e perfis) e a trilha do IDE
// entraram (VS Code onda 2, 02/10/2026).
// Casca 9: a casca nova (SP000036 L1: menu em areas, barra de comando, rodape lido, assistente)
// -- nenhum arquivo a mais, mas HTML, CSS e JS mudaram juntos e o instalado troca os tres de uma vez.
// Casca 10: os cartoes de acao e as execucoes recentes na Visao geral (SP000036 L2) -- de
// novo nenhum arquivo a mais, mas HTML, CSS, app.js, tarefas.js e textos.json mudaram juntos:
// um app.js novo sobre um tarefas.js velho nao acharia o cliente que ele reusa (window.tarefas).
// Casca 11: a lista de Tarefas ganhou piso min-content (grades.css: a «Criada em» saia cortada a
// 1536 px) e o #brandVersion declara a fonte (index.html) -- nenhum arquivo a mais; sem trocar o
// nome, o instalado sem rede ficaria com a coluna cortada.
// Casca 12: o minimapa do IDE (SP000032 R5) -- nenhum arquivo a mais, mas index.html, app.css,
// ide.js e textos.json mudaram juntos: um ide.js novo sobre um index.html velho nao acha o
// #ideMinimapa, e um textos.json velho mostraria as chaves cruas do painel.
const CACHE = 'phxclaw-casca-12';
const CASCA = [
  './', './index.html', './manifest.webmanifest',
  './assets/app.css', './assets/app.js', './assets/tema.js', './assets/idiomas.js', './assets/tarefas.js', './assets/tarefas.css',
  './assets/grades.js', './assets/grades.css', './assets/config.js', './assets/config-catalogo.json', './assets/ide.js', './assets/paineis.js',
  './assets/vendor/phx-grid/phx-grid.js', './assets/vendor/phx-grid/phx-grid.css',
  './assets/textos.json', './assets/fonte/exo2-latin.woff2',
  './assets/fonte/ibmplexmono-400.woff2', './assets/fonte/ibmplexmono-700.woff2',
  './assets/equipe.json', './assets/ferramentas.json', './assets/absorcao.json',
  './assets/icone-192.png', './assets/icone-512.png', './assets/icone-mascaravel-512.png',
  './assets/marca-96.png', './assets/favicon-32.png', './assets/favicon-48.png',
  './assets/abertura-escuro.png', './assets/abertura-claro.png',
];

self.addEventListener('install', e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(CASCA)).then(() => self.skipWaiting()));
});

self.addEventListener('activate', e => {
  e.waitUntil(caches.keys()
    .then(ks => Promise.all(ks.filter(k => k !== CACHE).map(k => caches.delete(k))))
    .then(() => self.clients.claim()));
});

self.addEventListener('fetch', e => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || url.origin !== location.origin || url.pathname.includes('/v1/')) return;
  e.respondWith(fetch(e.request).then(r => {
    if (r.ok) {
      const copia = r.clone();
      caches.open(CACHE).then(c => c.put(e.request, copia));
    }
    return r;
  }).catch(() => caches.match(e.request).then(r => {
    if (r) return r;
    // So a NAVEGACAO cai na casca: um JSON ou um script que nao esta no cache sai como falha
    // de rede, para a tela dizer «sem conexao» -- devolver o index.html no lugar de um JSON
    // virava «arquivo nao existe» (o JSON nao analisava) e mandava gerar o que ja existe.
    return e.request.mode === 'navigate' ? caches.match('./index.html') : Response.error();
  })));
});

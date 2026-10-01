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
const CACHE = 'phxclaw-casca-7';
const CASCA = [
  './', './index.html', './manifest.webmanifest',
  './assets/app.css', './assets/app.js', './assets/tema.js', './assets/idiomas.js', './assets/tarefas.js', './assets/tarefas.css',
  './assets/grades.js', './assets/grades.css', './assets/config.js', './assets/config-catalogo.json',
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

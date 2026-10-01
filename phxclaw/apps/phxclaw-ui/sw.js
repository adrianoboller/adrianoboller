// Service worker do aplicativo instalavel. Guarda SO a casca da tela (HTML, CSS, JS,
// icones, textos) para abrir sem rede; a API (/v1/) vai sempre a rede e nunca entra no
// cache -- tarefa vista do cache seria estado velho apresentado como atual.
//
// A casca e servida da rede primeiro e do cache so quando a rede falha: assim a versao
// nova chega na primeira visita com rede, sem esperar um segundo carregamento.
const CACHE = 'phxclaw-casca-1';
const CASCA = [
  './', './index.html', './manifest.webmanifest',
  './assets/app.css', './assets/app.js', './assets/idiomas.js', './assets/tarefas.js',
  './assets/textos.json', './assets/phoenix-mark.svg',
  './assets/icone-192.png', './assets/icone-512.png',
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
  }).catch(() => caches.match(e.request).then(r => r || caches.match('./index.html'))));
});

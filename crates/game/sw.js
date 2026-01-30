const CACHE_NAME = 'sim2d-__VERSION__';
const R2_WASM = 'https://bevy-pixel-world-assets.yura415.workers.dev/game.wasm';

// Assets to precache on install
const PRECACHE = [
  '/',
  '/index.html',
  '/manifest.json',
];

self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME).then(cache => cache.addAll(PRECACHE))
  );
  self.skipWaiting();
});

self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys().then(keys => Promise.all(
      keys.filter(k => k.startsWith('sim2d-') && k !== CACHE_NAME)
          .map(k => caches.delete(k))
    )).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', event => {
  const url = new URL(event.request.url);

  // Network-first for index.html (always get latest)
  if (url.pathname === '/' || url.pathname === '/index.html') {
    event.respondWith(networkFirst(event.request));
    return;
  }

  // Cache-first for immutable assets (content-hashed)
  if (url.pathname.match(/\.(js|wasm)$/) || url.pathname.startsWith('/assets/')) {
    event.respondWith(cacheFirst(event.request));
    return;
  }

  // Cache-first for R2 WASM
  if (url.origin === 'https://bevy-pixel-world-assets.yura415.workers.dev') {
    event.respondWith(cacheFirst(event.request));
    return;
  }

  // Default: network only
  event.respondWith(fetch(event.request));
});

async function networkFirst(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(CACHE_NAME);
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    const cached = await caches.match(request);
    if (cached) return cached;
    throw new Error('Network unavailable and no cache');
  }
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;

  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(CACHE_NAME);
    cache.put(request, response.clone());
  }
  return response;
}

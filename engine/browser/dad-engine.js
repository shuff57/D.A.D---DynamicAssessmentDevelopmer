// Main-thread client for the in-browser engine. The app calls window.dadEngine.post(route, body);
// the worker is started on first use, so pages that never test a question never download the engine.
// A request that takes longer than 30 s (a question stuck in a loop) restarts the worker.
(function () {
  const base = new URL('engine/', document.baseURI);
  let worker = null, ready = null, seq = 0;
  const pending = new Map();

  function send(msg, timeoutMs) {
    return new Promise((resolve, reject) => {
      const id = ++seq;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error('the engine took too long and was restarted'));
        restart();
      }, timeoutMs);
      pending.set(id, { resolve, reject, timer });
      worker.postMessage({ ...msg, id });
    });
  }

  function start() {
    worker = new Worker(new URL('engine-worker.js', base), { type: 'module' });
    worker.onmessage = (e) => {
      const p = pending.get(e.data.id);
      if (!p) return;
      pending.delete(e.data.id);
      clearTimeout(p.timer);
      e.data.ok ? p.resolve(e.data.text) : p.reject(new Error(e.data.error));
    };
    worker.onerror = (e) => console.error('D.A.D engine worker:', e.message);
    ready = send({ type: 'init', zipUrl: new URL('imathas-engine.zip', base).href }, 180000);
    ready.catch(() => {});
  }

  function restart() {
    if (worker) worker.terminate();
    for (const p of pending.values()) { clearTimeout(p.timer); p.reject(new Error('the engine restarted')); }
    pending.clear();
    worker = null;
  }

  window.dadEngine = {
    /** Start downloading the engine now (the app calls this when the Write screen opens). */
    warm() { if (!worker) start(); return ready; },
    async post(route, body) {
      if (!worker) start();
      await ready;
      return send({ type: 'request', route, body }, 30000);
    },
  };
})();

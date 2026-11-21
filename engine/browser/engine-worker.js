// Web Worker: runs the engine off the page's thread, so a slow question never freezes the app.
// Messages in: {id, type: 'init', zipUrl} then {id, type: 'request', route, body}. Out: {id, ok, text|error}.
import { loadWebRuntime } from '@php-wasm/web';
import { startEngine } from './engine-core.js';

let engine = null;
self.onmessage = async (e) => {
  const { id, type } = e.data;
  try {
    if (type === 'init') {
      const res = await fetch(e.data.zipUrl);
      if (!res.ok) throw new Error(`could not download the engine (${res.status})`);
      engine = await startEngine({ loadRuntime: () => loadWebRuntime('8.2'), zipBytes: new Uint8Array(await res.arrayBuffer()) });
      self.postMessage({ id, ok: true, text: String(engine.files) });
    } else {
      if (!engine) throw new Error('the engine has not started');
      self.postMessage({ id, ok: true, text: await engine.post(e.data.route, e.data.body) });
    }
  } catch (err) {
    self.postMessage({ id, ok: false, error: String(err && err.message || err) });
  }
};

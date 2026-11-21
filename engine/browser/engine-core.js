// The D.A.D engine on PHP compiled to WebAssembly: shared by the browser worker and the Node test.
// IMathAS's PHP (the parts the engine runs) arrives as one zip and is unpacked into PHP's memory
// filesystem. One PHP instance slows down over many requests, so its runtime is swapped for a fresh
// one every `maxRequests` (about 0.1 s); the files survive the swap.
import { PHP, PHPRequestHandler } from '@php-wasm/universal';
import { decodeZip } from '@php-wasm/stream-compression';

const ROOT = '/www';

export async function startEngine({ loadRuntime, zipBytes, maxRequests = 20 }) {
  const php = new PHP(await loadRuntime());
  php.enableRuntimeRotation({ recreateRuntime: loadRuntime, maxRequests });
  php.mkdirTree(ROOT);
  let files = 0;
  const stream = new Blob([zipBytes]).stream();
  for await (const file of decodeZip(stream)) {
    const path = `${ROOT}/${file.name}`;
    if (file.name.endsWith('/')) { php.mkdirTree(path); continue; }
    php.mkdirTree(path.slice(0, path.lastIndexOf('/')));
    php.writeFile(path, new Uint8Array(await file.arrayBuffer()));
    files++;
  }
  const handler = new PHPRequestHandler({ phpFactory: async () => php, documentRoot: ROOT, absoluteUrl: 'http://dad-engine.local' });
  return {
    files,
    /** POST a JSON body to an engine route ("/api/selftest", ...); resolves to the response text. */
    async post(route, body) {
      const r = await handler.request({
        method: 'POST',
        url: `/serve.php${route}`,
        headers: { 'content-type': 'application/json' },
        body: new TextEncoder().encode(body),
      });
      return r.text;
    },
  };
}

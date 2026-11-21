// bun build-worker.mjs: bundle engine-worker.js into dist/ as one ES module. wasm-feature-detect
// resolves to feature-detect-shim.js, so the loader only ever picks the asyncify build.
import path from 'node:path';
const shim = path.resolve(import.meta.dir, 'feature-detect-shim.js');
// The intl extension (not used: IMathAS does not need it) is imported as "...intl.so?url"; stand in an empty URL.
const emptyAsset = path.resolve(import.meta.dir, 'empty-asset-shim.js');
const NODE_ONLY = /^(node:.*|worker_threads|fs|path|url|os|crypto|child_process|net|tls|module)$/;
const out = await Bun.build({
  entrypoints: [path.resolve(import.meta.dir, 'engine-worker.js')],
  outdir: path.resolve(import.meta.dir, 'dist'),
  target: 'browser',
  format: 'esm',
  minify: true,
  splitting: true,
  external: ['node:*'],
  loader: { '.dat': 'file' },
  plugins: [{ name: 'asyncify-only', setup(b) {
    b.onResolve({ filter: /^wasm-feature-detect$/ }, () => ({ path: shim }));
    b.onResolve({ filter: /(\?url|\.dat)$/ }, () => ({ path: emptyAsset }));
    // Node-only code paths in @php-wasm/universal; never reached in a browser.
    b.onResolve({ filter: NODE_ONLY }, (a) => ({ path: a.path, external: true }));
  } }],
});
if (!out.success) { for (const l of out.logs) console.error(l); process.exit(1); }
for (const o of out.outputs) console.log(o.path.replace(import.meta.dir + '/', ''), o.size);

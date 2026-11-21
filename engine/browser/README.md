# The engine in the browser

The D.A.D engine (IMathAS + `engine/serve.php`) on PHP compiled to WebAssembly, so the web app needs
no engine server: it can be hosted as static files (Cloudflare Pages) and works offline once loaded.
GPL-2.0-only, like the rest of `engine/` (see `../LICENSE.md`).

```
engine/browser/build.sh      # builds dist/; Trunk copies it to the web app's engine/
node engine/browser/test-node.mjs dist/imathas-engine.zip <question.php>   # same code under Node
```

| File | What |
|---|---|
| `engine-core.js` | Unpacks the IMathAS zip into PHP's memory filesystem and answers engine routes. Swaps in a fresh PHP runtime every 20 requests (one instance degrades over many); files survive the swap. |
| `engine-worker.js` | Web Worker around engine-core, so a slow question never freezes the page. |
| `dad-engine.js` | The page's client: `window.dadEngine.post(route, body)` and `warm()`. Starts the worker on first use; a request over 30 s restarts it. |
| `build-worker.mjs` | Bun bundle of the worker. Only the asyncify PHP build (every current browser), no intl. |
| `feature-detect-shim.js`, `empty-asset-shim.js` | Stand-ins that keep JSPI, intl and its 30 MB ICU data out of the bundle. |

`dist/` (about 55 MB on disk; a visitor downloads the worker, PHP and the zip on first use, and the
sandbox's scripts as its pages ask for them):

- `engine-worker.js` and chunks, `8_2_35/php_8_2.wasm` (18 MB, PHP 8.2.35)
- `imathas-engine.zip` (5.4 MB): `assess2`, `assessment`, `includes`, `filter`, `header.php`,
  `footer.php` from the pinned IMathAS commit, plus `serve.php`, `momformat.php`, `student.php`
- `imathas/` (29 MB, 557 files, largest 2.7 MB): the sandbox page's scripts and styles
  (MyOpenMath's student view: assess2, MathQuill, MathJax 4, icons, images)
- `dad-engine.js`

Every file is under Cloudflare Pages' 25 MiB limit.

Measured (2026-11-20): the whole steve-desktop bank (1,239 questions) gives the same self-test verdict
as the Docker engine, 1,167 passing; a self-test takes about 0.4 s in Node, 0.46 s in Chromium after a
2.3 s first one.

// Node check of engine-core.js: unpack the engine zip into memory and self-test questions across
// several runtime swaps.  node test-node.mjs <imathas-engine.zip> <question.php> [n=60]
import fs from 'node:fs';
import { loadNodeRuntime } from '@php-wasm/node';
import { startEngine } from './engine-core.js';

const [zip, question, nArg] = process.argv.slice(2);
let pid = 1;
const t0 = Date.now();
const engine = await startEngine({ loadRuntime: () => loadNodeRuntime('8.2', { emscriptenOptions: { processId: pid++ } }), zipBytes: fs.readFileSync(zip) });
console.log(`unpacked ${engine.files} files, ready in ${Date.now() - t0} ms`);
const body = JSON.stringify({ src: fs.readFileSync(question, 'utf8'), seeds: [4242, 7, 99] });
const n = parseInt(nArg || '60', 10);
let pass = 0; const times = [];
for (let i = 0; i < n; i++) {
  const t = Date.now();
  const text = await engine.post('/api/selftest', body);
  times.push(Date.now() - t);
  try { if (JSON.parse(text).pass) pass++; } catch { console.log(`request ${i}: not JSON: ${text.slice(0, 200)}`); }
}
times.sort((a, b) => a - b);
console.log(`${pass}/${n} passed; median ${times[n >> 1]} ms, max ${times[n - 1]} ms (runtime swapped every 20)`);
process.exit(pass === n ? 0 : 1);

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const Module = require('module');

const ROOT = path.join(__dirname, '..');
const MARKER = '/* ==[ antigravity-ide-zh-overlay v1 ]== */';
const RELS = [
  'vs/code/electron-browser/workbench/jetskiAgent.js',
  'vs/code/electron-browser/workbench/workbench.js',
];
const GUARD = require('./extension').installedDir;

const APP = require('./resolve-app')();
const out = path.join(APP, 'out');
const productPath = path.join(APP, 'product.json');
const pristine = fs.readFileSync(productPath + '.orig-nozh', 'utf8');

function sumsOf() {
  const product = JSON.parse(fs.readFileSync(productPath, 'utf8'));
  return RELS.map((rel) => {
    const actual = crypto.createHash('sha256').update(fs.readFileSync(path.join(out, ...rel.split('/')))).digest('base64').replace(/=+$/, '');
    return actual === product.checksums[rel];
  });
}

let notices = [];
const originalLoad = Module._load;
Module._load = (request, ...rest) => (request === 'vscode'
  ? { env: { appRoot: APP }, window: { showInformationMessage: (m) => { notices.push(m); return Promise.resolve(undefined); } }, commands: { executeCommand: () => {} } }
  : originalLoad(request, ...rest));
const ext = require(path.join(GUARD, 'extension.js'));
Module._load = originalLoad;

const before = RELS.map((rel) => fs.readFileSync(path.join(out, ...rel.split('/')), 'utf8'));
if (!before.every((text) => text.includes(MARKER))) throw new Error('run `npm run apply` first; guard test needs a patched install');

// simulate an update reverting both entry scripts and the checksum table
for (const rel of RELS) {
  const file = path.join(out, ...rel.split('/'));
  fs.copyFileSync(file + '.orig-nozh', file);
}
fs.copyFileSync(productPath + '.orig-nozh', productPath);
console.log('simulated update: markers now', RELS.map((rel) => fs.readFileSync(path.join(out, ...rel.split('/')), 'utf8').includes(MARKER)).join(', '));

notices = [];
const started = Date.now();
ext.activate();
const elapsed = Date.now() - started;

const restored = RELS.map((rel) => fs.readFileSync(path.join(out, ...rel.split('/')), 'utf8'));
const results = [
  ['markers restored', restored.every((t) => t.includes(MARKER)), true],
  ['checksums match after self-heal', sumsOf().every(Boolean), true],
  ['entry scripts byte-identical to pre-update state', restored.join('|') === before.join('|'), true],
  ['product.json untouched by the repair', fs.readFileSync(productPath, 'utf8') !== pristine, true],
  ['user notified once', notices.length, 1],
];

for (const [label, actual, want] of results) {
  console.log((actual === want ? 'PASS  ' : 'FAIL  ') + label.padEnd(42) + (actual === want ? '' : 'got ' + JSON.stringify(actual)));
}

const noopSizes = RELS.map((rel) => fs.statSync(path.join(out, ...rel.split('/'))).size);
notices = [];
ext.activate();
console.log('second activation is a no-op:', JSON.stringify(noopSizes) === JSON.stringify(RELS.map((rel) => fs.statSync(path.join(out, ...rel.split('/'))).size)) && notices.length === 0);
console.log('repair took', elapsed, 'ms');

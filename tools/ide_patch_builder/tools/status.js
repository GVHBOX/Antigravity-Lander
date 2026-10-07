const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const MARKER = '/* ==[ antigravity-ide-zh-overlay v1 ]== */';
const TARGETS = [
  'vs/code/electron-browser/workbench/jetskiAgent.js',
  'vs/code/electron-browser/workbench/workbench.js',
];

const APP = require('./resolve-app')();
const product = JSON.parse(fs.readFileSync(path.join(APP, 'product.json'), 'utf8'));
const out = path.join(APP, 'out');

let bad = 0;
for (const rel of Object.keys(product.checksums)) {
  const file = path.join(out, ...rel.split('/'));
  const actual = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('base64').replace(/=+$/, '');
  const ok = actual === product.checksums[rel];
  if (!ok) bad++;
  console.log((ok ? 'OK    ' : 'DIFFER') + '  ' + rel);
}
console.log('\nintegrity mismatches:', bad, 'of', Object.keys(product.checksums).length);

for (const rel of TARGETS) {
  const content = fs.readFileSync(path.join(out, ...rel.split('/')), 'utf8');
  const scope = /EXCLUDE = '\.part\.sidebar/.test(content) ? 'excluded-containers' : 'whole document';
  console.log('overlay', rel.padEnd(52), content.includes(MARKER) ? 'installed' : 'MISSING', '|', scope);
}

const guard = require('./extension').installedDir;
console.log('self-heal extension:', fs.existsSync(path.join(guard, 'extension.js')) ? require('./extension').id : 'not installed');
console.log('backups kept:', TARGETS.filter((r) => fs.existsSync(path.join(out, ...r.split('/')) + '.orig-nozh')).length, 'entry scripts +', fs.existsSync(path.join(APP, 'product.json.orig-nozh')) ? 'product.json' : 'no product.json');

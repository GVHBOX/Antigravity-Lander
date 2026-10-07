const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const DICTIONARY = path.join(ROOT, 'data', 'dictionary.json');
const ENGINE = path.join(ROOT, 'src', 'overlay.js');
const DIST = path.join(ROOT, 'dist');

const pairs = JSON.parse(fs.readFileSync(DICTIONARY, 'utf8'));
for (const [en, zh] of Object.entries(pairs)) {
  if (typeof en !== 'string' || typeof zh !== 'string' || !en.trim() || !zh.trim()) throw new Error('bad dictionary entry: ' + JSON.stringify(en));
}

const phrases = 'const phrases = new Map(' + JSON.stringify(Object.entries(pairs), null, 1).replace(/\n/g, '\n  ') + ');';

const engine = fs.readFileSync(ENGINE, 'utf8');
if (!engine.includes('//__PHRASES__')) throw new Error('//__PHRASES__ anchor missing in src/overlay.js');

const variants = {
  'panel-zh-overlay.js': { scope: '  const SCOPE = null;', exclude: '  const EXCLUDE = null;' },
  'workbench-zh-overlay.js': {
    scope: '  const SCOPE = null;',
    exclude: "  const EXCLUDE = '.part.sidebar, .part.editor, .part.panel, .part.titlebar, .quick-input-widget, .monaco-editor, .xterm, .tabs-container, .breadcrumbs-control, .notifications-toasts, .notifications-center';",
  },
};

fs.mkdirSync(DIST, { recursive: true });
for (const [file, { scope, exclude }] of Object.entries(variants)) {
  const out = engine
    .replace('  const SCOPE = null;\n  const EXCLUDE = null;', scope + '\n' + exclude)
    .replace('//__PHRASES__', phrases);
  fs.writeFileSync(path.join(DIST, file), out, 'utf8');
  console.log(file.padEnd(26), Buffer.byteLength(out), 'bytes');
}
console.log('dictionary entries:', Object.keys(pairs).length);

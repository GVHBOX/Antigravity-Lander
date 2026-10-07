const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const APP = require('./resolve-app')();

const SOURCES = [
  path.join(APP, 'out', 'jetskiAgent', 'main.js'),
  path.join(APP, 'out', 'vs', 'workbench', 'workbench.desktop.main.js'),
];

const PROPERTY = /(?:label|title|description|tooltip|placeholder|ariaLabel|text|children|name)\s*:\s*(["'`])((?:(?!\1)[^\\\n]|\\.){2,160})\1/g;
const JSX_CHILD = /children:\s*\(\s*(["'`])((?:(?!\1)[^\\\n]|\\.){8,160})\1/g;

const SKIP = /[{}<>;=|]|=>|\$\{|https?:|\/\/|^[a-z0-9_.$-]+$|\bfunction\b|\/[a-z]+\/[gimsuy]*$/i;

const found = new Map();
for (const file of SOURCES) {
  const src = fs.readFileSync(file, 'utf8');
  for (const re of [PROPERTY, JSX_CHILD]) {
    re.lastIndex = 0;
    let m;
    while ((m = re.exec(src))) {
      const value = m[2].replace(/\\(["'\\])/g, '$1');
      if (!/^[\x20-\x7e]+$/.test(value)) continue;
      if (!/[A-Za-z]{3}/.test(value)) continue;
      if (SKIP.test(value)) continue;
      if (!/\s/.test(value) && value.length < 5) continue;
      found.set(value, (found.get(value) || 0) + 1);
    }
  }
}

const list = [...found.keys()].sort((a, b) => a.localeCompare(b));
const out = path.join(ROOT, 'data', 'ui-strings.txt');
fs.writeFileSync(out, list.join('\n') + '\n', 'utf8');

console.log('unique UI strings collected:', list.length, '-> data/ui-strings.txt');
console.log('multi-word only:', list.filter((s) => /\s/.test(s)).length);
console.log('\nsample:');
list.slice(0, 20).forEach((s) => console.log('  ' + s.slice(0, 96)));

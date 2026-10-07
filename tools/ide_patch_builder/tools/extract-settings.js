const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const BUNDLE = path.join(require('./resolve-app')(), 'out', 'jetskiAgent', 'main.js');

const src = fs.readFileSync(BUNDLE, 'utf8');
const re = /screen:"([A-Za-z ]{2,20})"[\s\S]{0,400}?label:(["'])((?:(?!\2)[^\\\n]|\\.){1,140})\2(?:[\s\S]{0,300}?description:(["'])((?:(?!\4)[^\\\n]|\\.){1,400})\4)?/g;

const clean = (s) => (s || '').replace(/\\(["'\\])/g, '$1');
const seen = new Set();
const rows = [];
let m;
while ((m = re.exec(src))) {
  const row = { screen: m[1], label: clean(m[3]), description: clean(m[5]) };
  const key = row.label + '|' + row.description;
  if (seen.has(key)) continue;
  seen.add(key);
  rows.push(row);
}

fs.writeFileSync(path.join(ROOT, 'data', 'settings-rows.json'), JSON.stringify(rows, null, 1), 'utf8');
console.log('settings rows extracted:', rows.length, '-> data/settings-rows.json');
console.log('next: add any missing translations to data/settings-zh-pairs.json, then npm run build && npm run probe');

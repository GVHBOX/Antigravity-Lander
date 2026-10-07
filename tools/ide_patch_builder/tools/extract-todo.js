const fs = require('fs');
const path = require('path');

const ROOT = 'D:\\Develop\\antigravity-ide-zh';
const APP = require(path.join(ROOT, 'tools', 'resolve-app'))();
const src = fs.readFileSync(path.join(APP, 'out', 'jetskiAgent', 'main.js'), 'utf8');

const dictionary = require(path.join(ROOT, 'data', 'dictionary.json'));

const RE = /(?:label|description|title|subtitle|section|group|tooltip|placeholder|heading|header|text)\s*:\s*(["'`])((?:(?!\1)[^\\\n]|\\.){6,220})\1/g;

const found = new Set();
let m;
while ((m = RE.exec(src))) {
  const v = m[2].replace(/\\(["'\\])/g, '$1');
  if (dictionary[v]) continue;
  if (!/^[\x20-\x7e]+$/.test(v)) continue;
  if (!/[a-z]{3}/.test(v)) continue;
  if (/[{}<>|;`]|=>|\$\{|%s|%1|https?:\/\/|\/\*|<\/|\bfunction\b|\breturn\b|\.js\b|\.css\b|node_modules/.test(v)) continue;
  if (/^[a-z0-9_.\-\/ ]+$/.test(v) && !/[A-Z]/.test(v)) continue;
  if (/\b(?:Error|TypeError|Cannot read|Invalid JSON|Expected|Received|token|tokens)\b/.test(v) && !/[.!?]$/.test(v)) continue;
  found.add(v);
}

const list = [...found].sort((a, b) => a.localeCompare(b));
fs.writeFileSync(path.join(ROOT, 'data', 'todo-strings.txt'), list.join('\n') + '\n', 'utf8');
console.log('untranslated UI strings:', list.length);
const sentences = list.filter((s) => /[.!?]$/.test(s));
console.log('of which sentence-like (descriptions):', sentences.length);

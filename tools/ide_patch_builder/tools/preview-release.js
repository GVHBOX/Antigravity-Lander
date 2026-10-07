const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const patterns = fs.readFileSync(path.join(ROOT, '.gitignore'), 'utf8')
  .split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith('#'));

function walk(dir, acc = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === '.git') continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, acc);
    else acc.push(path.relative(ROOT, full).split(path.sep).join('/'));
  }
  return acc;
}

function ignored(file) {
  return patterns.some((pattern) => {
    if (pattern.endsWith('/')) return file.startsWith(pattern) || file.includes('/' + pattern);
    return path.basename(file) === pattern || file === pattern;
  });
}

const all = walk(ROOT);
const tracked = all.filter((f) => !ignored(f)).sort();
const skipped = all.filter(ignored).sort();
const bytes = tracked.reduce((sum, f) => sum + fs.statSync(path.join(ROOT, f)).size, 0);

console.log('would publish', tracked.length, 'files,', bytes, 'bytes:');
tracked.forEach((f) => console.log('  ' + f));
console.log('excluded', skipped.length + ':');
skipped.forEach((f) => console.log('  ' + f));

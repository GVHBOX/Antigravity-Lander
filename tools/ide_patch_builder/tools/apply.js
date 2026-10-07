const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const TMP = path.join(ROOT, '.tmp');
const MARKER = '/* ==[ antigravity-ide-zh-overlay v1 ]== */';

const TARGETS = [
  { rel: 'vs/code/electron-browser/workbench/jetskiAgent.js', overlay: 'panel-zh-overlay.js' },
  { rel: 'vs/code/electron-browser/workbench/workbench.js', overlay: 'workbench-zh-overlay.js' },
];

const args = process.argv.slice(2);
const wantsRestore = args.includes('--restore');
const checkOnly = args.includes('--check-only');

const APP = require('./resolve-app')();const OUT = path.join(APP, 'out');
const PRODUCT = path.join(APP, 'product.json');

function parses(content) {
  fs.mkdirSync(TMP, { recursive: true });
  const probe = path.join(TMP, 'parse-probe.mjs');
  fs.writeFileSync(probe, content, 'utf8');
  try { execFileSync(process.execPath, ['--check', probe], { stdio: ['ignore', 'ignore', 'pipe'] }); return true; }
  catch (e) { console.error(String(e.stderr).slice(0, 400)); return false; }
  finally { fs.rmSync(probe, { force: true }); }
}

function build(original, overlay) {
  let stripped = original;
  if (original.includes(MARKER)) {
    const at = original.indexOf(MARKER);
    stripped = original.slice(0, at);
    const map = original.lastIndexOf('//# sourceMappingURL=');
    if (map > at && !stripped.includes('//# sourceMappingURL=')) stripped += original.slice(original.lastIndexOf('\n', map) + 1);
  }
  const block = MARKER + '\n' + overlay.trimEnd() + '\n';
  const at = stripped.lastIndexOf('//# sourceMappingURL=');
  if (at === -1) return { text: stripped.replace(/\s*$/, '\n\n') + block, stripped };
  const lineStart = stripped.lastIndexOf('\n', at) + 1;
  return { text: stripped.slice(0, lineStart) + block + '\n' + stripped.slice(lineStart), stripped };
}

function checksum(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('base64').replace(/=+$/, '');
}

function writeChecksums(map) {
  let raw = fs.readFileSync(PRODUCT, 'utf8');
  for (const [rel, value] of Object.entries(map)) {
    const re = new RegExp('("' + rel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '"\\s*:\\s*")([^"]+)(")');
    if (!re.test(raw)) throw new Error('checksum key missing from product.json: ' + rel);
    raw = raw.replace(re, '$1' + value + '$3');
  }
  JSON.parse(raw);
  fs.writeFileSync(PRODUCT, raw, 'utf8');
}

if (wantsRestore) {
  for (const t of TARGETS) {
    const file = path.join(OUT, ...t.rel.split('/'));
    if (fs.existsSync(file + '.orig-nozh')) {
      fs.copyFileSync(file + '.orig-nozh', file);
      try { fs.unlinkSync(file + '.orig-nozh'); } catch {}
    }
  }
  if (fs.existsSync(PRODUCT + '.orig-nozh')) {
    fs.copyFileSync(PRODUCT + '.orig-nozh', PRODUCT);
    try { fs.unlinkSync(PRODUCT + '.orig-nozh'); } catch {}
  }

  const ext = require('./extension');
  if (fs.existsSync(ext.installedDir)) {
    try { fs.rmSync(ext.installedDir, { recursive: true, force: true }); } catch {}
  }

  const extJsonPath = path.join(os.homedir(), '.antigravity-ide', 'extensions', 'extensions.json');
  if (fs.existsSync(extJsonPath)) {
    try {
      const arr = JSON.parse(fs.readFileSync(extJsonPath, 'utf8'));
      const filtered = arr.filter(item => item?.identifier?.id !== ext.id);
      fs.writeFileSync(extJsonPath, JSON.stringify(filtered), 'utf8');
    } catch {}
  }

  const argvPath = path.join(os.homedir(), '.antigravity-ide', 'argv.json');
  if (fs.existsSync(argvPath + '.orig-nozh')) {
    fs.copyFileSync(argvPath + '.orig-nozh', argvPath);
    try { fs.unlinkSync(argvPath + '.orig-nozh'); } catch {}
  } else if (fs.existsSync(argvPath)) {
    try {
      let raw = fs.readFileSync(argvPath, 'utf8');
      raw = raw.replace(/,?\r?\n\s*"locale"\s*:\s*"[^"]*"/, '');
      fs.writeFileSync(argvPath, raw, 'utf8');
    } catch {}
  }

  console.log('restored', TARGETS.length, 'entry scripts and product.json, removed guard and cleaned backups');
  process.exit(0);
}


require('./ensure-dist')();

const planned = {};
for (const t of TARGETS) {
  const file = path.join(OUT, ...t.rel.split('/'));
  const disk = fs.readFileSync(file, 'utf8');
  const base = fs.existsSync(file + '.orig-nozh') ? fs.readFileSync(file + '.orig-nozh', 'utf8') : disk;
  const { text, stripped } = build(base, fs.readFileSync(path.join(ROOT, 'dist', t.overlay), 'utf8'));
  if (!text.startsWith(stripped.slice(0, 2000))) throw new Error('prefix changed for ' + t.rel);
  if (!parses(text)) { console.error('REFUSED: patched ' + t.rel + ' does not parse'); process.exit(2); }
  planned[t.rel] = { file, text, base: stripped };
  console.log(t.rel.padEnd(52), stripped.length, '->', Buffer.byteLength(text), 'bytes, parses OK');
}

if (checkOnly) { console.log('check-only passed, nothing written'); process.exit(0); }

if (!fs.existsSync(PRODUCT + '.orig-nozh')) fs.copyFileSync(PRODUCT, PRODUCT + '.orig-nozh');
const sums = {};
for (const [rel, info] of Object.entries(planned)) {
  if (!fs.existsSync(info.file + '.orig-nozh')) fs.writeFileSync(info.file + '.orig-nozh', info.base, 'utf8');
  fs.writeFileSync(info.file, info.text, 'utf8');
  sums[rel] = checksum(info.file);
}
writeChecksums(sums);

const guardDir = require('./extension').installedDir;
if (fs.existsSync(guardDir)) {
  for (const t of TARGETS) fs.copyFileSync(path.join(ROOT, 'dist', t.overlay), path.join(guardDir, t.overlay));
  console.log('guard bundles refreshed');
}

console.log('applied', TARGETS.length, 'overlays under', APP);
console.log('checksums written for', Object.keys(sums).length, 'files; restart the IDE to take effect');

'use strict';

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const vscode = require('vscode');

const MARKER = '/* ==[ antigravity-ide-zh-overlay v1 ]== */';
const TARGETS = [
  { rel: 'vs/code/electron-browser/workbench/jetskiAgent.js', overlay: 'panel-zh-overlay.js' },
  { rel: 'vs/code/electron-browser/workbench/workbench.js', overlay: 'workbench-zh-overlay.js' },
];

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
  if (at === -1) return stripped.replace(/\s*$/, '\n\n') + block;
  const lineStart = stripped.lastIndexOf('\n', at) + 1;
  return stripped.slice(0, lineStart) + block + '\n' + stripped.slice(lineStart);
}

function writeAtomic(file, content) {
  const tmp = file + '.zh-tmp';
  fs.writeFileSync(tmp, content, 'utf8');
  fs.renameSync(tmp, file);
}

function refreshChecksums(map) {
  const product = path.join(vscode.env.appRoot, 'product.json');
  let raw = fs.readFileSync(product, 'utf8');
  for (const [rel, value] of Object.entries(map)) {
    const escaped = rel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp('("' + escaped + '"\\s*:\\s*")([^"]+)(")');
    if (!re.test(raw)) return false;
    raw = raw.replace(re, '$1' + value + '$3');
  }
  try { JSON.parse(raw); } catch { return false; }
  writeAtomic(product, raw);
  return true;
}

function activate() {
  if (fs.existsSync(path.join(__dirname, 'disabled'))) return;
  const out = path.join(vscode.env.appRoot, 'out');
  const repaired = {};
  const sums = {};

  for (const target of TARGETS) {
    const file = path.join(out, ...target.rel.split('/'));
    let current;
    try { current = fs.readFileSync(file, 'utf8'); } catch { continue; }
    if (current.includes(MARKER)) continue;
    let overlay;
    try { overlay = fs.readFileSync(path.join(__dirname, target.overlay), 'utf8'); } catch { continue; }
    const patched = build(current, overlay);
    if (!patched.startsWith(current.slice(0, 2000))) continue;
    const backup = file + '.orig-nozh';
    if (!fs.existsSync(backup)) writeAtomic(backup, current);
    writeAtomic(file, patched);
    sums[target.rel] = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('base64').replace(/=+$/, '');
    repaired[target.rel] = true;
  }

  const keys = Object.keys(sums);
  if (!keys.length) return;
  if (!refreshChecksums(sums)) console.log('[zh-guard] patched but checksum table not updated');
  vscode.window.showInformationMessage('Antigravity 面板汉化补丁已重新应用（' + keys.length + ' 个文件），重开面板或重启 IDE 后生效。');
}

module.exports = { activate };

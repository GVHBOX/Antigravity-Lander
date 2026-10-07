const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const OUTPUTS = ['panel-zh-overlay.js', 'workbench-zh-overlay.js'];
const SOURCES = [path.join(ROOT, 'src', 'overlay.js'), path.join(ROOT, 'data', 'dictionary.json')];

function stale() {
  for (const output of OUTPUTS) {
    const file = path.join(ROOT, 'dist', output);
    if (!fs.existsSync(file)) return true;
    const built = fs.statSync(file).mtimeMs;
    if (SOURCES.some((s) => fs.statSync(s).mtimeMs > built)) return true;
  }
  return false;
}

// Keeps dist/ a pure build artefact: any entry point can rebuild it instead of
// assuming a clone shipped it.
module.exports = function ensureDist() {
  if (!stale()) return false;
  const result = spawnSync(process.execPath, [path.join(__dirname, 'build.js')], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status || 1);
  return true;
};

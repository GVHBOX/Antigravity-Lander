const fs = require('fs');
const os = require('os');
const path = require('path');

const home = os.homedir();

const CANDIDATES = [
  process.env.AGIDE_APP,

  // Windows
  'D:\\Antigravity IDE\\resources\\app',
  path.join(process.env.LOCALAPPDATA || '', 'Programs', 'Antigravity IDE', 'resources', 'app'),
  path.join(process.env.LOCALAPPDATA || '', 'Programs', 'antigravity-ide', 'resources', 'app'),
  'C:\\Program Files\\Antigravity IDE\\resources\\app',

  // macOS
  '/Applications/Antigravity IDE.app/Contents/Resources/app',
  path.join(home, 'Applications', 'Antigravity IDE.app', 'Contents', 'Resources', 'app'),

  // Linux
  '/opt/Antigravity IDE/resources/app',
  '/usr/local/Antigravity IDE/resources/app',
  path.join(home, '.local', 'share', 'Antigravity IDE', 'resources', 'app'),
].filter(Boolean);

module.exports = function resolveApp() {
  for (const dir of CANDIDATES) {
    try {
      const product = JSON.parse(fs.readFileSync(path.join(dir, 'product.json'), 'utf8'));
      if (product.nameShort === 'Antigravity IDE' || product.applicationName === 'antigravity-ide') return dir;
    } catch { /* not here, keep looking */ }
  }
  throw new Error(
    'Antigravity IDE install not found. Set AGIDE_APP to the directory containing product.json, e.g.\n' +
    '  Windows  C:\\Users\\<you>\\AppData\\Local\\Programs\\Antigravity IDE\\resources\\app\n' +
    '  macOS    /Applications/Antigravity IDE.app/Contents/Resources/app\n' +
    '  Linux    /opt/Antigravity IDE/resources/app'
  );
};

module.exports.CANDIDATES = CANDIDATES;

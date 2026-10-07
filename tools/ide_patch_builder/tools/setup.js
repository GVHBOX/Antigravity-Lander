const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const APP = require('./resolve-app')();
const INSTALL = path.resolve(APP, '..', '..');
const CLI = process.platform === 'win32'
  ? path.join(INSTALL, 'bin', 'antigravity-ide.cmd')
  : path.join(INSTALL, 'bin', 'antigravity-ide');
const ARGV = path.join(os.homedir(), '.antigravity-ide', 'argv.json');
const langpack = require('./langpack');

function run(file, args = []) {
  const result = spawnSync(process.execPath, [path.join(ROOT, 'tools', file), ...args], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status || 1);
}

async function ensureLanguagePack(hostVersion) {
  const installed = langpack.installedVersion();
  const wanted = await langpack.resolve(hostVersion);
  const manual = '  "' + CLI + '" --install-extension ' + langpack.LANGPACK + (wanted ? '@' + wanted.version : '');

  if (!wanted) {
    console.log('language pack: Open VSX unreachable, install manually:\n' + manual);
    return;
  }
  const same = installed && `${installed.major}.${installed.minor}.${installed.patch}` === wanted.version;
  if (same) {
    console.log(`language pack: ${wanted.version} installed, compatible with host ${hostVersion}`);
    return;
  }
  const why = installed
    ? `installed ${installed.major}.${installed.minor}.${installed.patch} does not match host ${hostVersion}`
    : 'not installed';
  console.log(`language pack: ${why} -> installing ${wanted.version} (${wanted.reason})`);
  if (!fs.existsSync(CLI)) { console.log('language pack: CLI not found, run manually:\n' + manual); return; }
  const result = spawnSync(CLI, ['--install-extension', `${langpack.LANGPACK}@${wanted.version}`], { stdio: 'inherit', shell: process.platform === 'win32' });
  if (result.status !== 0) console.log('language pack: install failed, run manually:\n' + manual);
}

function ensureLocale() {
  if (!fs.existsSync(ARGV)) { console.log('locale: ' + ARGV + ' not found — start the IDE once, then re-run setup'); return; }
  const raw = fs.readFileSync(ARGV, 'utf8');
  if (/"locale"\s*:\s*"zh-cn"/.test(raw)) { console.log('locale: already zh-cn'); return; }
  const eol = raw.includes('\r\n') ? '\r\n' : '\n';
  let next;
  if (/"locale"\s*:\s*"[^"]*"/.test(raw)) next = raw.replace(/("locale"\s*:\s*")([^"]*)(")/, '$1zh-cn$3');
  else {
    const close = raw.lastIndexOf('}');
    const head = raw.slice(0, close).replace(/\s+$/, '');
    next = head + (/["\d\w\]}]$/.test(head.trimEnd()) ? ',' : '') + eol + '\t"locale": "zh-cn"' + eol + raw.slice(close);
  }
  if (!fs.existsSync(ARGV + '.orig-nozh')) fs.copyFileSync(ARGV, ARGV + '.orig-nozh');
  fs.writeFileSync(ARGV, next, 'utf8');
  console.log('locale: set to zh-cn (previous file kept as argv.json.orig-nozh)');
}

(async () => {
  const product = JSON.parse(fs.readFileSync(path.join(APP, 'product.json'), 'utf8'));
  console.log('Antigravity IDE', product.ideVersion || '?', '(VS Code ' + product.version + ') at', APP);
  console.log('');
  await ensureLanguagePack(product.version);
  ensureLocale();
  console.log('');
  run('build.js');
  console.log('');
  run('apply.js');
  console.log('');
  run('install-guard.js');
  console.log('');
  console.log('done. Restart the IDE to load the overlay.');
  console.log('check anytime with:  npm run status   |   revert with:  npm run restore');
})();

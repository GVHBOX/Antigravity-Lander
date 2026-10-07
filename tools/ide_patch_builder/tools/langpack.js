const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');

const LANGPACK = 'ms-ceintl.vscode-language-pack-zh-hans';
const API = 'https://open-vsx.org/api/MS-CEINTL/vscode-language-pack-zh-hans';

function get(url, timeout = 8000) {
  return new Promise((resolve) => {
    const req = https.get(url, (res) => {
      let body = '';
      res.on('data', (c) => { body += c; });
      res.on('end', () => resolve({ status: res.statusCode, body }));
    });
    req.on('error', () => resolve({ status: 0, body: '' }));
    req.setTimeout(timeout, () => { req.destroy(); resolve({ status: 0, body: '' }); });
  });
}

function installedVersion() {
  const dir = path.join(os.homedir(), '.antigravity-ide', 'extensions');
  if (!fs.existsSync(dir)) return null;
  for (const entry of fs.readdirSync(dir)) {
    const m = entry.match(new RegExp('^' + LANGPACK + '-(\\d+)\\.(\\d+)\\.(\\d+)'));
    if (m) return { major: +m[1], minor: +m[2], patch: +m[3], folder: entry };
  }
  return null;
}

// Language packs are built per VS Code release and declare engines.vscode as
// "^<major>.<minor>.0", so a pack newer than the host is rejected outright.
async function resolve(targetVersion) {
  const [major, minor] = targetVersion.split('.').map(Number);
  for (let candidate = minor; candidate >= minor - 3 && candidate >= 0; candidate--) {
    const version = `${major}.${candidate}.0`;
    const res = await get(`${API}/${version}`);
    if (res.status === 200) return { version, reason: candidate === minor ? 'exact match' : `host is ${targetVersion}, newest available at or below it` };
  }
  return null;
}

module.exports = { LANGPACK, installedVersion, resolve };

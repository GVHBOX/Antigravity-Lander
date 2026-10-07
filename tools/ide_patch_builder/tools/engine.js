const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.join(__dirname, '..');
const APP = require('./resolve-app')();
const OUT = path.join(APP, 'out');
const PRODUCT = path.join(APP, 'product.json');
const EXT_DIR = path.join(os.homedir(), '.antigravity-ide', 'extensions');
const EXT_JSON = path.join(EXT_DIR, 'extensions.json');
const ARGV = path.join(os.homedir(), '.antigravity-ide', 'argv.json');

const LANGPACK_ID = 'ms-ceintl.vscode-language-pack-zh-hans';
const LANGPACK_VER = '1.106.0';
const LANGPACK_TARGET = path.join(EXT_DIR, `${LANGPACK_ID}-${LANGPACK_VER}`);
const LANGPACK_SRC = path.join(ROOT, 'assets', 'langpack');

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

function installLanguagePack() {
  if (!fs.existsSync(LANGPACK_SRC)) {
    throw new Error('离线语言包资产缺失: ' + LANGPACK_SRC);
  }
  fs.mkdirSync(LANGPACK_TARGET, { recursive: true });
  fs.cpSync(LANGPACK_SRC, LANGPACK_TARGET, { recursive: true });

  if (fs.existsSync(EXT_JSON)) {
    try {
      const list = JSON.parse(fs.readFileSync(EXT_JSON, 'utf8'));
      const exists = list.some(item => item?.identifier?.id?.toLowerCase() === LANGPACK_ID);
      if (!exists) {
        list.push({
          identifier: { id: LANGPACK_ID, uuid: "45d16cc1-1185-48b4-a2fd-a10996bcffc4" },
          version: LANGPACK_VER,
          location: {
            $mid: 1,
            path: `/${LANGPACK_TARGET.replace(/\\/g, '/')}`,
            scheme: "file"
          },
          relativeLocation: `${LANGPACK_ID}-${LANGPACK_VER}`,
          metadata: {
            installedTimestamp: Date.now(),
            pinned: false,
            source: "gallery",
            publisherId: "ms-ceintl",
            publisherDisplayName: "Microsoft",
            targetPlatform: "universal",
            updated: false,
            private: false,
            isPreReleaseVersion: false,
            hasPreReleaseVersion: false
          }
        });
        fs.writeFileSync(EXT_JSON, JSON.stringify(list), 'utf8');
      }
    } catch {}
  }
}

function uninstallLanguagePack() {
  if (fs.existsSync(EXT_DIR)) {
    try {
      const dirs = fs.readdirSync(EXT_DIR);
      for (const d of dirs) {
        if (d.toLowerCase().startsWith(LANGPACK_ID)) {
          fs.rmSync(path.join(EXT_DIR, d), { recursive: true, force: true });
        }
      }
    } catch {}
  }

  if (fs.existsSync(EXT_JSON)) {
    try {
      const list = JSON.parse(fs.readFileSync(EXT_JSON, 'utf8'));
      const filtered = list.filter(item => item?.identifier?.id?.toLowerCase() !== LANGPACK_ID);
      fs.writeFileSync(EXT_JSON, JSON.stringify(filtered), 'utf8');
    } catch {}
  }
}

function configureLocale(enable) {
  if (!fs.existsSync(ARGV)) {
    if (!enable) return;
    fs.mkdirSync(path.dirname(ARGV), { recursive: true });
    fs.writeFileSync(ARGV, '{\n\t"locale": "zh-cn"\n}\n', 'utf8');
    return;
  }

  if (enable) {
    const raw = fs.readFileSync(ARGV, 'utf8');
    if (/"locale"\s*:\s*"zh-cn"/.test(raw)) return;
    if (!fs.existsSync(ARGV + '.orig-nozh')) {
      fs.copyFileSync(ARGV, ARGV + '.orig-nozh');
    }
    const eol = raw.includes('\r\n') ? '\r\n' : '\n';
    let next;
    if (/"locale"\s*:\s*"[^"]*"/.test(raw)) {
      next = raw.replace(/("locale"\s*:\s*")([^"]*)(")/, '$1zh-cn$3');
    } else {
      const close = raw.lastIndexOf('}');
      const head = raw.slice(0, close).replace(/\s+$/, '');
      next = head + (/["\d\w\]}]$/.test(head.trimEnd()) ? ',' : '') + eol + '\t"locale": "zh-cn"' + eol + raw.slice(close);
    }
    fs.writeFileSync(ARGV, next, 'utf8');
  } else {
    if (fs.existsSync(ARGV + '.orig-nozh')) {
      fs.copyFileSync(ARGV + '.orig-nozh', ARGV);
      try { fs.unlinkSync(ARGV + '.orig-nozh'); } catch {}
    } else if (fs.existsSync(ARGV)) {
      try {
        let raw = fs.readFileSync(ARGV, 'utf8');
        raw = raw.replace(/,?\r?\n\s*"locale"\s*:\s*"[^"]*"/, '');
        fs.writeFileSync(ARGV, raw, 'utf8');
      } catch {}
    }
  }
}

function cleanLegacyGuards() {
  const guardId = 'beichen.antigravity-zh-guard';
  if (fs.existsSync(EXT_DIR)) {
    try {
      const entries = fs.readdirSync(EXT_DIR);
      for (const e of entries) {
        if (e.toLowerCase().startsWith(guardId)) {
          fs.rmSync(path.join(EXT_DIR, e), { recursive: true, force: true });
        }
      }
    } catch {}
  }
  if (fs.existsSync(EXT_JSON)) {
    try {
      const list = JSON.parse(fs.readFileSync(EXT_JSON, 'utf8'));
      const filtered = list.filter(item => item?.identifier?.id?.toLowerCase() !== guardId);
      fs.writeFileSync(EXT_JSON, JSON.stringify(filtered), 'utf8');
    } catch {}
  }
}

function install() {
  installLanguagePack();
  configureLocale(true);
  cleanLegacyGuards();

  require('./ensure-dist')();

  if (!fs.existsSync(PRODUCT + '.orig-nozh')) {
    fs.copyFileSync(PRODUCT, PRODUCT + '.orig-nozh');
  }

  const sums = {};
  for (const t of TARGETS) {
    const file = path.join(OUT, ...t.rel.split('/'));
    const disk = fs.readFileSync(file, 'utf8');
    const base = fs.existsSync(file + '.orig-nozh') ? fs.readFileSync(file + '.orig-nozh', 'utf8') : disk;
    const { text, stripped } = build(base, fs.readFileSync(path.join(ROOT, 'dist', t.overlay), 'utf8'));

    if (!fs.existsSync(file + '.orig-nozh')) {
      fs.writeFileSync(file + '.orig-nozh', stripped, 'utf8');
    }
    fs.writeFileSync(file, text, 'utf8');
    sums[t.rel] = checksum(file);
  }

  writeChecksums(sums);
  console.log('installed: language-pack + 2 entry scripts, product.json checksums updated');
}

function uninstall() {
  for (const t of TARGETS) {
    const file = path.join(OUT, ...t.rel.split('/'));
    if (fs.existsSync(file + '.orig-nozh')) {
      fs.copyFileSync(file + '.orig-nozh', file);
      try { fs.unlinkSync(file + '.orig-nozh'); } catch {}
    } else {
      const cur = fs.readFileSync(file, 'utf8');
      if (cur.includes(MARKER)) {
        const { stripped } = build(cur, '');
        fs.writeFileSync(file, stripped, 'utf8');
      }
    }
  }

  if (fs.existsSync(PRODUCT + '.orig-nozh')) {
    fs.copyFileSync(PRODUCT + '.orig-nozh', PRODUCT);
    try { fs.unlinkSync(PRODUCT + '.orig-nozh'); } catch {}
  }

  uninstallLanguagePack();
  configureLocale(false);
  cleanLegacyGuards();
  console.log('uninstalled: restored 2 entry scripts, product.json, removed language pack and clean backups');
}

function status() {
  const product = JSON.parse(fs.readFileSync(PRODUCT, 'utf8'));
  let mismatches = 0;
  for (const rel of Object.keys(product.checksums)) {
    const file = path.join(OUT, ...rel.split('/'));
    const actual = checksum(file);
    if (actual !== product.checksums[rel]) mismatches++;
  }

  let patchedTargets = 0;
  for (const t of TARGETS) {
    const file = path.join(OUT, ...t.rel.split('/'));
    const content = fs.readFileSync(file, 'utf8');
    if (content.includes(MARKER)) patchedTargets++;
  }

  const langpackInstalled = fs.existsSync(LANGPACK_TARGET);

  console.log('integrity mismatches:', mismatches, 'of', Object.keys(product.checksums).length);
  console.log('overlays installed:', patchedTargets, 'of', TARGETS.length);
  console.log('language pack:', langpackInstalled ? 'installed' : 'not installed');

  if (patchedTargets === TARGETS.length && langpackInstalled) {
    process.exit(0);
  } else if (patchedTargets === 0 && !langpackInstalled) {
    process.exit(1);
  } else {
    process.exit(2);
  }
}

const args = process.argv.slice(2);
if (args.includes('--install')) {
  install();
} else if (args.includes('--uninstall') || args.includes('--restore')) {
  uninstall();
} else if (args.includes('--status')) {
  status();
} else {
  console.log('Usage: node engine.js [--install | --uninstall | --status]');
  process.exit(1);
}

const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const WORKSPACE_DIR = ROOT;
const EXTRACT_DIR = path.join(ROOT, '.tmp_extract');
const DIST_DIR = path.join(ROOT, 'dist');
const CACHE_ASAR = path.join(ROOT, '..', '..', 'data', 'patch', 'app.asar');
const CACHE_STAMP = path.join(ROOT, '..', '..', 'data', 'patch', 'app.asar.version.json');

const REQUIRED_MARKERS = [
  { rel: 'dist/preload.js', marker: 'Antigravity 2.0 Web UI DOM Localization Engine' },
  { rel: 'dist/menu.js', marker: 'Antigravity 2.0 Native Menu' },
  { rel: 'dist/ipcHandlers.js', marker: 'Antigravity 2.0 Native Menu' },
];

function readAsar(buf) {
  const jsonLen = buf.readUInt32LE(12);
  const header = JSON.parse(buf.subarray(16, 16 + jsonLen).toString('utf8'));
  return { header, dataBase: 8 + buf.readUInt32LE(4) };
}

function asarEntry(header, rel) {
  let node = header;
  for (const part of rel.split('/')) {
    node = node.files?.[part];
    if (!node) return null;
  }
  return node;
}

function asarRead(buf, dataBase, rel) {
  const node = asarEntry(readAsar(buf).header, rel);
  if (!node || node.unpacked || !node.size) return null;
  const offset = Number(node.offset);
  return buf.subarray(dataBase + offset, dataBase + offset + Number(node.size)).toString('utf8');
}

function hostVersion(asarPath) {
  try {
    const buf = fs.readFileSync(asarPath);
    const { dataBase } = readAsar(buf);
    const pkg = asarRead(buf, dataBase, 'package.json');
    if (!pkg) return null;
    const version = JSON.parse(pkg).version;
    return typeof version === 'string' && version ? version : null;
  } catch {
    return null;
  }
}

function localizedMarkers(asarPath) {
  const found = {};
  try {
    const buf = fs.readFileSync(asarPath);
    const { dataBase } = readAsar(buf);
    for (const { rel, marker } of REQUIRED_MARKERS) {
      const text = asarRead(buf, dataBase, rel) || '';
      found[rel] = text.includes(marker);
    }
  } catch {
    for (const { rel } of REQUIRED_MARKERS) found[rel] = false;
  }
  return found;
}

function verifyAsar(asarPath) {
  let version;
  try {
    version = hostVersion(asarPath);
  } catch (err) {
    throw new Error(`汉化产物无法解析（不是有效的 asar）: ${path.basename(asarPath)} (${err.message})`);
  }
  if (!version) {
    throw new Error('汉化产物缺少可读的 package.json，拒绝写入');
  }
  const found = localizedMarkers(asarPath);
  const missed = Object.entries(found).filter(([, ok]) => !ok).map(([rel]) => rel);
  if (missed.length) {
    throw new Error(`汉化产物校验失败，未注入: ${missed.join(', ')}`);
  }
  return version;
}

function writeCacheStamp(version) {
  try {
    fs.mkdirSync(path.dirname(CACHE_STAMP), { recursive: true });
    fs.writeFileSync(CACHE_STAMP, JSON.stringify({ version, built: new Date().toISOString() }, null, 2), 'utf8');
  } catch {}
}

function refreshCache(asarPath) {
  try {
    fs.mkdirSync(path.dirname(CACHE_ASAR), { recursive: true });
    const tmp = CACHE_ASAR + '.tmp';
    fs.copyFileSync(asarPath, tmp);
    fs.renameSync(tmp, CACHE_ASAR);
    writeCacheStamp(hostVersion(asarPath));
  } catch (err) {
    console.warn('警告: 离线汉化包缓存刷新失败:', err.message);
  }
}

function getHostUsername() {
  return process.env.USER || process.env.USERNAME || 'default';
}

function getAsarCmd() {
  const localMjs = path.join(WORKSPACE_DIR, 'node_modules', '@electron', 'asar', 'bin', 'asar.mjs');
  if (fs.existsSync(localMjs)) {
    return `"${process.execPath}" "${localMjs}"`;
  }
  const localBin = path.join(WORKSPACE_DIR, 'node_modules', '.bin', process.platform === 'win32' ? 'asar.cmd' : 'asar');
  if (fs.existsSync(localBin)) {
    return `"${localBin}"`;
  }
  const userHome = os.homedir();
  const npxCacheDir = path.join(process.env.LOCALAPPDATA || path.join(userHome, 'AppData', 'Local'), 'npm-cache', '_npx');
  if (fs.existsSync(npxCacheDir)) {
    try {
      const hashes = fs.readdirSync(npxCacheDir);
      for (const h of hashes) {
        const candidate = path.join(npxCacheDir, h, 'node_modules', '@electron', 'asar', 'bin', 'asar.mjs');
        if (fs.existsSync(candidate)) {
          return `"${process.execPath}" "${candidate}"`;
        }
      }
    } catch {}
  }
  const major = parseInt(process.versions.node.split('.')[0], 10);
  return major >= 18 ? 'npx --prefer-offline -y @electron/asar' : 'npx --prefer-offline -y asar@3.2.0';
}

function isAppRunning() {
  try {
    if (process.platform === 'win32') {
      const out = execSync('tasklist', { encoding: 'utf-8' }).toLowerCase();
      return out.includes('antigravity.exe') || out.includes('language_server.exe');
    } else if (process.platform === 'darwin') {
      execSync('pgrep -xi "antigravity|language_server"', { stdio: 'ignore' });
      return true;
    } else {
      execSync('pgrep -x "antigravity|language_server"', { stdio: 'ignore' });
      return true;
    }
  } catch {
    return false;
  }
}

function killApp() {
  try {
    if (process.platform === 'win32') {
      try { execSync('taskkill /F /IM Antigravity.exe', { stdio: 'ignore' }); } catch {}
      try { execSync('taskkill /F /IM language_server.exe', { stdio: 'ignore' }); } catch {}
    } else if (process.platform === 'darwin') {
      try { execSync('pkill -xi antigravity', { stdio: 'ignore' }); } catch {}
      try { execSync('pkill -xi language_server', { stdio: 'ignore' }); } catch {}
    } else {
      try { execSync('pkill -x antigravity', { stdio: 'ignore' }); } catch {}
      try { execSync('pkill -x language_server', { stdio: 'ignore' }); } catch {}
    }
  } catch {}
}

function cleanAppCache(username) {
  const user = username ? username.trim() : getHostUsername();
  let roots = [];
  if (process.platform === 'win32') {
    roots = [
      path.join(process.env.APPDATA || `C:\\Users\\${user}\\AppData\\Roaming`, 'Antigravity'),
      path.join(process.env.APPDATA || `C:\\Users\\${user}\\AppData\\Roaming`, 'antigravity'),
      path.join(process.env.LOCALAPPDATA || `C:\\Users\\${user}\\AppData\\Local`, 'antigravity'),
    ];
  } else if (process.platform === 'darwin') {
    roots = [
      path.join('/Users', user, 'Library', 'Application Support', 'Antigravity'),
      path.join('/Users', user, 'Library', 'Caches', 'com.google.antigravity'),
    ];
  } else {
    roots = [
      path.join('/home', user, '.config', 'Antigravity'),
      path.join('/home', user, '.cache', 'antigravity'),
    ];
  }

  const subDirs = ['Cache', 'Code Cache', 'GPUCache', 'DawnGraphiteCache', 'DawnWebGPUCache', 'blob_storage'];
  let count = 0;
  for (const r of roots) {
    if (!fs.existsSync(r)) continue;
    for (const s of subDirs) {
      const p = path.join(r, s);
      if (fs.existsSync(p)) {
        try {
          fs.rmSync(p, { recursive: true, force: true });
          count++;
        } catch {}
      }
    }
  }
  return count;
}

function resolveAppResourcesDir(customPath) {
  if (customPath) {
    if (customPath.endsWith('.asar')) return path.dirname(customPath);
    const candidate = path.join(customPath, 'resources');
    if (fs.existsSync(candidate)) return candidate;
    const upper = path.join(customPath, 'Resources');
    if (fs.existsSync(upper)) return upper;
    return customPath;
  }

  if (process.platform === 'win32') {
    const local = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    return path.join(local, 'Programs', 'antigravity', 'resources');
  } else if (process.platform === 'darwin') {
    return '/Applications/Antigravity.app/Contents/Resources';
  } else {
    return path.join(os.homedir(), 'Antigravity', 'Antigravity-x64', 'resources');
  }
}

function isLocalizedAsar(filePath) {
  if (!fs.existsSync(filePath)) return false;
  try {
    const buf = fs.readFileSync(filePath);
    return buf.includes(Buffer.from('Antigravity 2.0 Web UI DOM Localization Engine')) ||
           buf.includes(Buffer.from('Antigravity 2.0 Chinese Localization Engine')) ||
           buf.includes(Buffer.from('Antigravity 2.0 Native Menu')) ||
           buf.includes(Buffer.from('menuTranslationMap'));
  } catch {
    return false;
  }
}

function replaceInFile(filePath, target, replacement) {
  if (!fs.existsSync(filePath)) return false;
  let content = fs.readFileSync(filePath, 'utf8');
  if (!content.includes(target)) return false;
  content = content.split(target).join(replacement);
  fs.writeFileSync(filePath, content, 'utf8');
  return true;
}

// 锚点失配只意味着官方改了这段文案，不是错误；收集起来一次性报出，
// 免得汉化覆盖率悄悄掉下坡却没人知道。
const misses = [];

function applyAnchor(label, filePath, target, replacement) {
  const hit = replaceInFile(filePath, target, replacement);
  if (!hit) misses.push(label);
  return hit;
}

function applyAnchorAll(label, filePath, pairs) {
  if (!fs.existsSync(filePath)) {
    misses.push(label + ' (文件缺失)');
    return;
  }
  let content = fs.readFileSync(filePath, 'utf8');
  let hits = 0;
  for (const [target, replacement] of pairs) {
    if (!content.includes(target)) continue;
    content = content.split(target).join(replacement);
    hits++;
  }
  if (hits !== pairs.length) misses.push(`${label} (${hits}/${pairs.length})`);
  if (hits) fs.writeFileSync(filePath, content, 'utf8');
}

function injectOrUpdate(filePath, content, startMarker) {
  if (!fs.existsSync(filePath)) return false;
  let existing = fs.readFileSync(filePath, 'utf8');
  const at = existing.indexOf(startMarker);
  if (at !== -1) {
    existing = existing.substring(0, at).trimEnd() + '\n\n' + content;
  } else {
    existing = existing.trimEnd() + '\n\n' + content;
  }
  fs.writeFileSync(filePath, existing, 'utf8');
  return true;
}

function applyModificationsToExtracted(extractDir) {
  misses.length = 0;

  // Ensure dist files exist
  const domDist = path.join(DIST_DIR, 'dom_engine.js');
  const menuDist = path.join(DIST_DIR, 'menu_hook.js');
  if (!fs.existsSync(domDist) || !fs.existsSync(menuDist)) {
    require('./build')();
  }

  const domEngineCode = fs.readFileSync(domDist, 'utf8');
  const menuHookCode = fs.readFileSync(menuDist, 'utf8');

  // 1. Inject DOM Localization Engine into preloads
  const preloadPath = path.join(extractDir, 'dist', 'preload.js');
  const wizardPreloadPath = path.join(extractDir, 'dist', 'ideInstall', 'wizardPreload.js');
  if (!injectOrUpdate(preloadPath, domEngineCode, '// Antigravity 2.0')) misses.push('渲染层 preload.js (缺失)');
  if (!injectOrUpdate(wizardPreloadPath, domEngineCode, '// Antigravity 2.0')) misses.push('向导 preload (缺失)');

  // 2. Native application menu
  const menuPath = path.join(extractDir, 'dist', 'menu.js');
  if (fs.existsSync(menuPath)) {
    injectOrUpdate(menuPath, menuHookCode, '// Antigravity 2.0 Native Menu');
    applyAnchor(
      '主菜单 submenuItem 查找',
      menuPath,
      'const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);',
      'const submenuItem = appMenu.items.find((item) => item.label === submenuLabel || (typeof menuTranslationMap !== "undefined" && item.label === menuTranslationMap[submenuLabel]));'
    );
    applyAnchor(
      '主菜单 setApplicationMenu',
      menuPath,
      'electron_1.Menu.setApplicationMenu(menu);',
      'if (typeof translateMenu === "function") { menu.items.forEach(translateMenu); } electron_1.Menu.setApplicationMenu(menu);'
    );
  } else {
    misses.push('主菜单 menu.js (文件缺失)');
  }

  // 3. System Tray
  const trayPath = path.join(extractDir, 'dist', 'tray.js');
  if (fs.existsSync(trayPath)) {
    applyAnchor(
      '托盘计数文案',
      trayPath,
      `countItem.label =
                (count > 0 ? \`\${count}\` : 'No') +
                    ' agent' +
                    (count === 1 ? '' : 's') +
                    ' running';`,
      `countItem.label = count > 0 ? \`\${count} 个智能体运行中\` : '没有智能体在运行';`
    );
    applyAnchor(
      '托盘菜单模板',
      trayPath,
      'contextMenu = electron_1.Menu.buildFromTemplate(actions);',
      `const translatedActions = actions.map(action => {
        if (action.label === 'No agents running') action.label = '没有智能体在运行';
        if (action.label && action.label.startsWith('Open ')) action.label = '打开 Antigravity';
        if (action.label === 'Quit') action.label = '退出';
        return action;
      });
      contextMenu = electron_1.Menu.buildFromTemplate(translatedActions);`
    );
  } else {
    misses.push('托盘 tray.js (文件缺失)');
  }

  // 4. 首启向导页（2.21 起新增，失败不阻断）
  const wizardHtmlPath = path.join(extractDir, 'dist', 'ideInstall', 'wizardHtml.js');
  if (fs.existsSync(wizardHtmlPath)) {
    applyAnchorAll('首启向导文案', wizardHtmlPath, [
      ['<title>Welcome to Antigravity</title>', '<title>欢迎使用 Antigravity</title>'],
      ['>Setting up…<', '>正在准备…<'],
      ['<h1>Welcome to the new Antigravity!</h1>', '<h1>欢迎使用全新 Antigravity</h1>'],
      [
        "<p>Antigravity has been redesigned to put agents first with new capabilities. If you'd still like a code editor, you can download it as a separate app named <b>Antigravity IDE</b>.</p>",
        '<p>Antigravity 已围绕智能体重构，带来了新的能力。如果你仍需要代码编辑器，可以另行下载名为 <b>Antigravity IDE</b> 的独立应用。</p>',
      ],
      ['<span>Download the Antigravity IDE</span>', '<span>下载 Antigravity IDE</span>'],
      ['<button class="btn-primary" id="btn-skip">Explore the new Antigravity</button>', '<button class="btn-primary" id="btn-skip">开始使用全新 Antigravity</button>'],
    ]);
  }

  // 5. Context menu in ipcHandlers
  const ipcPath = path.join(extractDir, 'dist', 'ipcHandlers.js');
  if (fs.existsSync(ipcPath)) {
    applyAnchorAll('文件选择对话框', ipcPath, [
      ["title: 'Open workspace',", "title: '打开工作区',"],
      ["title: 'Open workspaces',", "title: '打开工作区',"],
      ["electron_1.dialog.showErrorBox('Cannot open folder', t.error);", "electron_1.dialog.showErrorBox('无法打开文件夹', t.error);"],
      ["message: 'Folder is on the Windows filesystem',", "message: '文件夹位于 Windows 文件系统中',"],
    ]);
    injectOrUpdate(ipcPath, menuHookCode, '// Antigravity 2.0 Native Menu');
    applyAnchor(
      '右键菜单标签替换',
      ipcPath,
      "label: item.label ?? '',",
      "label: (typeof translateContextLabel === 'function' ? translateContextLabel(item.label) : item.label) ?? '',"
    );
  } else {
    misses.push('右键菜单 ipcHandlers.js (文件缺失)');
  }

  // 6. WSL alerts and splash
  const splashPath = path.join(extractDir, 'dist', 'provisionSplash.js');
  if (fs.existsSync(splashPath)) {
    applyAnchor('WSL 配置提示', splashPath, '<div>Setting up WSL: ${escapeHtml(distro)}</div>', '<div>正在配置 WSL: ${escapeHtml(distro)}</div>');
  }

  const wslPath = path.join(extractDir, 'dist', 'wsl.js');
  if (fs.existsSync(wslPath)) {
    applyAnchorAll('WSL 提示文案', wslPath, [
      [
        "warning: 'This folder is on the Windows filesystem. Accessing it from WSL (via /mnt) can be slow — for best performance keep projects inside the WSL filesystem.',",
        "warning: '此文件夹位于 Windows 文件系统。从 WSL 访问（通过 /mnt）可能较慢 — 为获得最佳性能，建议将项目保留在 WSL 文件系统中。',",
      ],
      [
        "error: `This folder belongs to the WSL distro \"${unc[1]}\", but this window is connected to \"${distro}\".`,",
        "error: `此文件夹属于 WSL 发行版 \"${unc[1]}\"，但当前窗口连接到 \"${distro}\"。`,",
      ],
      ["error: `This location cannot be opened in WSL: ${winPath}`", "error: `无法在 WSL 中打开此位置: ${winPath}`"],
    ]);
  }

  // 7. Main process startup & exit confirm
  const mainPath = path.join(extractDir, 'dist', 'main.js');
  if (fs.existsSync(mainPath)) {
    applyAnchorAll('退出确认弹窗', mainPath, [
      ["title: 'Confirm Quit',", "title: '确认退出',"],
      ["message: 'Are you sure you want to quit?',", "message: '您确定要退出吗？',"],
      ["detail: 'There may be agents or background tasks running.',", "detail: '可能还有智能体或后台任务正在运行。',"],
    ]);
  } else {
    misses.push('主进程 main.js (文件缺失)');
  }
}

// 备份可能来自上一个版本（应用更新只替换 app.asar，不清理 .bak）。
// 用旧版备份还原会把新版客户端降级，所以版本不一致一律作废。
function reconcileBackup(asarPath, bakPath) {
  const live = hostVersion(asarPath);
  if (!fs.existsSync(bakPath)) {
    if (!isLocalizedAsar(asarPath)) {
      fs.copyFileSync(asarPath, bakPath);
      console.log(`已创建官方原版备份: app.asar.bak (v${live})`);
    }
    return;
  }

  const bak = hostVersion(bakPath);
  if (bak === live) return;

  if (isLocalizedAsar(asarPath)) {
    console.warn(`警告: 备份为 v${bak}，与已装 v${live} 不一致且当前版本已汉化，无法重建原版备份，已作废旧备份。`);
    try { fs.unlinkSync(bakPath); } catch {}
  } else {
    console.log(`备份版本 v${bak} 已过期，按 v${live} 原版刷新: app.asar.bak`);
    fs.copyFileSync(asarPath, bakPath);
  }
}

// 反向映射本身有损（两个英文串可能译成同一句中文），所以只要存在同版本原版备份，
// 就用备份里这几个文件的原始内容覆盖回去，保证反复开关不会逐次劣化。
const TOUCHED_FILES = [
  'dist/preload.js',
  'dist/ideInstall/wizardPreload.js',
  'dist/menu.js',
  'dist/ipcHandlers.js',
  'dist/tray.js',
  'dist/main.js',
  'dist/provisionSplash.js',
  'dist/wsl.js',
  'dist/ideInstall/wizardHtml.js',
];

function restoreFromBackup(extractDir, bakPath) {
  let restored = 0;
  let buf;
  let dataBase;
  try {
    buf = fs.readFileSync(bakPath);
    dataBase = readAsar(buf).dataBase;
  } catch {
    return 0;
  }
  for (const rel of TOUCHED_FILES) {
    const text = asarRead(buf, dataBase, rel);
    if (text === null) continue;
    const target = path.join(extractDir, ...rel.split('/'));
    if (!fs.existsSync(target)) continue;
    fs.writeFileSync(target, text, 'utf8');
    restored++;
  }
  return restored;
}

function ensureUnpackedJunction(asarPath) {
  const dir = path.dirname(asarPath);
  const expectedUnpacked = asarPath + '.unpacked';
  const defaultUnpacked = path.join(dir, 'app.asar.unpacked');

  if (!fs.existsSync(expectedUnpacked) && fs.existsSync(defaultUnpacked)) {
    try {
      fs.symlinkSync(defaultUnpacked, expectedUnpacked, 'junction');
      return () => {
        try { fs.unlinkSync(expectedUnpacked); } catch {}
      };
    } catch {}
  }
  return () => {};
}

function extractTree(asarPath, bakPath) {
  if (fs.existsSync(EXTRACT_DIR)) {
    fs.rmSync(EXTRACT_DIR, { recursive: true, force: true });
  }
  const cleanup = ensureUnpackedJunction(asarPath);
  try {
    execSync(`${getAsarCmd()} extract "${asarPath}" "${EXTRACT_DIR}"`, { cwd: WORKSPACE_DIR });
  } finally {
    cleanup();
  }
  if (!isLocalizedAsar(asarPath)) return;
  if (bakPath && fs.existsSync(bakPath) && hostVersion(bakPath) === hostVersion(asarPath)) {
    const n = restoreFromBackup(EXTRACT_DIR, bakPath);
    console.log(`检测到已有汉化层，已用同版本原版备份还原 ${n} 个文件`);
    return;
  }
  console.log('检测到已有汉化层，按注入标记剥离旧层（无同版本备份可对照）...');
  stripModifications(EXTRACT_DIR);
}

function install(customPath) {
  if (isAppRunning()) {
    console.log('检测到正在运行的 Antigravity 进程，正在关闭...');
    killApp();
  }

  const resDir = resolveAppResourcesDir(customPath);
  const asarPath = path.join(resDir, 'app.asar');
  const bakPath = path.join(resDir, 'app.asar.bak');

  if (!fs.existsSync(asarPath)) {
    throw new Error(`找不到 app.asar: ${asarPath}`);
  }

  reconcileBackup(asarPath, bakPath);
  extractTree(asarPath, bakPath);

  console.log('正在应用汉化层与拦截逻辑...');
  applyModificationsToExtracted(EXTRACT_DIR);

  const tempAsar = path.join(WORKSPACE_DIR, 'app.asar.temp');
  if (fs.existsSync(tempAsar)) fs.unlinkSync(tempAsar);

  console.log('正在重新封包 asar...');
  execSync(`${getAsarCmd()} pack "${EXTRACT_DIR}" "${tempAsar}" --unpack-dir "**/chrome-devtools-mcp"`, { cwd: WORKSPACE_DIR });

  verifyAsar(tempAsar);
  fs.copyFileSync(tempAsar, asarPath);
  refreshCache(asarPath);
  try { fs.unlinkSync(tempAsar); } catch {}
  try { fs.rmSync(tempAsar + '.unpacked', { recursive: true, force: true }); } catch {}
  try { fs.rmSync(EXTRACT_DIR, { recursive: true, force: true }); } catch {}

  cleanAppCache();
  reportMisses();
  console.log(`汉化完成，已成功写入目标目录 (适配桌面版 ${hostVersion(asarPath) || '未知'})。`);
}

function reportMisses() {
  if (!misses.length) return;
  console.log(`锚点失配 ${misses.length} 处（官方已改文案，跳过）: ${misses.join('; ')}`);
}

function repackTo(extractDir, asarPath) {
  const tempAsar = path.join(WORKSPACE_DIR, 'app.asar.temp');
  if (fs.existsSync(tempAsar)) fs.unlinkSync(tempAsar);
  execSync(`${getAsarCmd()} pack "${extractDir}" "${tempAsar}" --unpack-dir "**/chrome-devtools-mcp"`, { cwd: WORKSPACE_DIR });
  fs.copyFileSync(tempAsar, asarPath);
  try { fs.unlinkSync(tempAsar); } catch {}
  try { fs.rmSync(tempAsar + '.unpacked', { recursive: true, force: true }); } catch {}
  try { fs.rmSync(extractDir, { recursive: true, force: true }); } catch {}
}

function restore(customPath) {
  if (isAppRunning()) {
    console.log('检测到正在运行的 Antigravity 进程，正在关闭...');
    killApp();
  }

  const resDir = resolveAppResourcesDir(customPath);
  const asarPath = path.join(resDir, 'app.asar');
  const bakPath = path.join(resDir, 'app.asar.bak');

  if (!fs.existsSync(asarPath)) {
    throw new Error(`找不到 app.asar: ${asarPath}`);
  }

  const live = hostVersion(asarPath);
  const bak = fs.existsSync(bakPath) ? hostVersion(bakPath) : null;

  // 备份属于旧版本时用它还原等于把客户端降级，只能放弃备份并就地剥离汉化层。
  if (bak && bak !== live) {
    console.warn(`警告: 备份 v${bak} 与已装 v${live} 不一致，跳过还原，就地剥离汉化层。`);
  } else if (bak) {
    fs.copyFileSync(bakPath, asarPath);
    try { fs.unlinkSync(bakPath); } catch {}
    cleanAppCache();
    console.log(`已成功还原为官方原版 app.asar (v${live}) 并移除备份。`);
    return;
  }

  if (fs.existsSync(EXTRACT_DIR)) fs.rmSync(EXTRACT_DIR, { recursive: true, force: true });
  execSync(`${getAsarCmd()} extract "${asarPath}" "${EXTRACT_DIR}"`, { cwd: WORKSPACE_DIR });
  stripModifications(EXTRACT_DIR);
  repackTo(EXTRACT_DIR, asarPath);
  try { fs.unlinkSync(bakPath); } catch {}
  cleanAppCache();
  console.log(`已剥离汉化层 (保留 v${live} 官方代码)。`);
}

// 无原版备份时，从 pack 产物里按注入标记反向切除汉化层。
function stripModifications(extractDir) {
  const MARKER = '// Antigravity 2.0';
  for (const rel of ['dist/preload.js', 'dist/ideInstall/wizardPreload.js', 'dist/menu.js', 'dist/ipcHandlers.js']) {
    const file = path.join(extractDir, ...rel.split('/'));
    if (!fs.existsSync(file)) continue;
    const text = fs.readFileSync(file, 'utf8');
    const at = text.indexOf(MARKER);
    if (at === -1) continue;
    const head = text.slice(0, at).replace(/\s+$/, '');
    fs.writeFileSync(file, head ? head + '\n' : '', 'utf8');
  }
  restoreAnchors(extractDir);
}

// 反向替换：把安装时写入的中文与包裹代码还原为官方原文。
function restoreAnchors(extractDir) {
  const ANCHORS = [
    ['dist/menu.js', 'const submenuItem = appMenu.items.find((item) => item.label === submenuLabel || (typeof menuTranslationMap !== "undefined" && item.label === menuTranslationMap[submenuLabel]));', 'const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);'],
    ['dist/menu.js', 'if (typeof translateMenu === "function") { menu.items.forEach(translateMenu); } electron_1.Menu.setApplicationMenu(menu);', 'electron_1.Menu.setApplicationMenu(menu);'],
    ['dist/ipcHandlers.js', "title: '打开工作区',", "title: 'Open workspace',"],
    ['dist/ipcHandlers.js', "electron_1.dialog.showErrorBox('无法打开文件夹', t.error);", "electron_1.dialog.showErrorBox('Cannot open folder', t.error);"],
    ['dist/ipcHandlers.js', "message: '文件夹位于 Windows 文件系统中',", "message: 'Folder is on the Windows filesystem',"],
    ['dist/ipcHandlers.js', "label: (typeof translateContextLabel === 'function' ? translateContextLabel(item.label) : item.label) ?? '',", "label: item.label ?? '',"],
    ['dist/provisionSplash.js', '<div>正在配置 WSL: ${escapeHtml(distro)}</div>', '<div>Setting up WSL: ${escapeHtml(distro)}</div>'],
    [
      'dist/tray.js',
      "countItem.label = count > 0 ? `${count} 个智能体运行中` : '没有智能体在运行';",
      `countItem.label =
                (count > 0 ? \`\${count}\` : 'No') +
                    ' agent' +
                    (count === 1 ? '' : 's') +
                    ' running';`,
    ],
    [
      'dist/tray.js',
      "contextMenu = electron_1.Menu.buildFromTemplate(translatedActions);",
      'contextMenu = electron_1.Menu.buildFromTemplate(actions);',
    ],
    ['dist/wsl.js', "warning: '此文件夹位于 Windows 文件系统。从 WSL 访问（通过 /mnt）可能较慢 — 为获得最佳性能，建议将项目保留在 WSL 文件系统中。',", "warning: 'This folder is on the Windows filesystem. Accessing it from WSL (via /mnt) can be slow — for best performance keep projects inside the WSL filesystem.',"],
    ['dist/wsl.js', "error: `此文件夹属于 WSL 发行版 \"${unc[1]}\"，但当前窗口连接到 \"${distro}\"。`,", "error: `This folder belongs to the WSL distro \"${unc[1]}\", but this window is connected to \"${distro}\".`,"],
    ['dist/wsl.js', "error: `无法在 WSL 中打开此位置: ${winPath}`", "error: `This location cannot be opened in WSL: ${winPath}`"],
    ['dist/ideInstall/wizardHtml.js', '<title>欢迎使用 Antigravity</title>', '<title>Welcome to Antigravity</title>'],
    ['dist/ideInstall/wizardHtml.js', '>正在准备…<', '>Setting up…<'],
    ['dist/ideInstall/wizardHtml.js', '<h1>欢迎使用全新 Antigravity</h1>', '<h1>Welcome to the new Antigravity!</h1>'],
    ['dist/ideInstall/wizardHtml.js', '<span>下载 Antigravity IDE</span>', '<span>Download the Antigravity IDE</span>'],
    ['dist/ideInstall/wizardHtml.js', '<button class="btn-primary" id="btn-skip">开始使用全新 Antigravity</button>', '<button class="btn-primary" id="btn-skip">Explore the new Antigravity</button>'],
    ['dist/main.js', "title: '确认退出',", "title: 'Confirm Quit',"],
    ['dist/main.js', "message: '您确定要退出吗？',", "message: 'Are you sure you want to quit?',"],
    ['dist/main.js', "detail: '可能还有智能体或后台任务正在运行。',", "detail: 'There may be agents or background tasks running.',"],
  ];
  for (const [rel, zh, en] of ANCHORS) {
    replaceInFile(path.join(extractDir, ...rel.split('/')), zh, en);
  }
}

function status(customPath) {
  const resDir = resolveAppResourcesDir(customPath);
  const asarPath = path.join(resDir, 'app.asar');
  const bakPath = path.join(resDir, 'app.asar.bak');

  const exists = fs.existsSync(asarPath);
  // 只看注入标记：应用更新会换上原版 app.asar，但留在目录里的 .bak 不会消失，
  // 拿备份是否存在当判据会把未汉化误报成已汉化。
  const patched = exists && isLocalizedAsar(asarPath);

  console.log('app.asar:', exists ? '存在' : '不存在');
  console.log('状态:', patched ? '已汉化' : '未汉化');
  console.log('官方备份:', fs.existsSync(bakPath) ? '存在' : '不存在');

  if (exists) {
    const live = hostVersion(asarPath);
    console.log('已装版本:', live ? `v${live}` : '未知');
    if (patched) {
      const missed = Object.entries(localizedMarkers(asarPath)).filter(([, ok]) => !ok).map(([rel]) => rel);
      console.log('注入完整性:', missed.length ? `缺失 ${missed.join(', ')}` : '完整');
    }
  }

  if (fs.existsSync(CACHE_ASAR)) {
    const cached = hostVersion(CACHE_ASAR);
    const live = exists ? hostVersion(asarPath) : null;
    console.log('离线包版本:', cached ? `v${cached}` : '未知', live && cached && live !== cached ? `(与已装版本 v${live} 不一致)` : '');
  } else {
    console.log('离线包:', '不存在');
  }

  process.exit(patched ? 0 : 1);
}

function buildPatchFile(targetFile) {
  const outPath = targetFile ? path.resolve(targetFile) : path.resolve(CACHE_ASAR);

  const resDir = resolveAppResourcesDir();
  const sourceAsar = fs.existsSync(path.join(resDir, 'app.asar.bak'))
    ? path.join(resDir, 'app.asar.bak')
    : path.join(resDir, 'app.asar');

  if (!fs.existsSync(sourceAsar)) {
    throw new Error(`找不到源 app.asar: ${sourceAsar}`);
  }

  if (fs.existsSync(EXTRACT_DIR)) {
    fs.rmSync(EXTRACT_DIR, { recursive: true, force: true });
  }

  console.log('正在从源文件提取:', sourceAsar);
  const cleanup = ensureUnpackedJunction(sourceAsar);
  try {
    execSync(`${getAsarCmd()} extract "${sourceAsar}" "${EXTRACT_DIR}"`, { cwd: WORKSPACE_DIR });
  } finally {
    cleanup();
  }

  console.log('正在注入汉化补丁...');
  applyModificationsToExtracted(EXTRACT_DIR);

  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  if (fs.existsSync(outPath)) fs.unlinkSync(outPath);

  console.log('正在打包到:', outPath);
  execSync(`${getAsarCmd()} pack "${EXTRACT_DIR}" "${outPath}" --unpack-dir "**/chrome-devtools-mcp"`, { cwd: WORKSPACE_DIR });

  const version = verifyAsar(outPath);
  if (path.resolve(outPath) === path.resolve(CACHE_ASAR)) writeCacheStamp(version);
  try { fs.rmSync(EXTRACT_DIR, { recursive: true, force: true }); } catch {}
  reportMisses();
  console.log(`预编译汉化包构建完成: ${outPath} (适配桌面版 ${version})`);
}

// CLI args dispatcher
const args = process.argv.slice(2);
let customPath = '';
const pathIdx = args.indexOf('--target');
if (pathIdx !== -1 && args[pathIdx + 1]) {
  customPath = args[pathIdx + 1];
}

try {
  if (args.includes('--install') || args.includes('--now')) {
    install(customPath);
  } else if (args.includes('--restore') || args.includes('--uninstall')) {
    restore(customPath);
  } else if (args.includes('--status')) {
    status(customPath);
  } else if (args.includes('--build-patch')) {
    const outIdx = args.indexOf('--output');
    const outTarget = outIdx !== -1 ? args[outIdx + 1] : '';
    buildPatchFile(outTarget);
  } else if (args.includes('--clean-cache')) {
    const cleaned = cleanAppCache();
    console.log(`已清理 ${cleaned} 处缓存。`);
  } else {
    console.log('用法: node engine.js [--install | --restore | --status | --build-patch | --clean-cache]');
    process.exit(1);
  }
} catch (err) {
  console.error('执行失败:', err.message);
  process.exit(2);
}

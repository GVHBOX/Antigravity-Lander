const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const DATA_DIR = path.join(ROOT, 'data');
const SRC_DIR = path.join(ROOT, 'src');
const DIST_DIR = path.join(ROOT, 'dist');

fs.mkdirSync(DIST_DIR, { recursive: true });

function verifySyntax(filePath) {
  try {
    execFileSync(process.execPath, ['--check', filePath], { stdio: ['ignore', 'ignore', 'pipe'] });
    return true;
  } catch (err) {
    console.error(`Syntax check failed for ${path.basename(filePath)}:`, String(err.stderr));
    return false;
  }
}

function build() {
  console.log('Building localization payloads...');

  // 1. Build Web UI DOM injection engine
  const uiData = JSON.parse(fs.readFileSync(path.join(DATA_DIR, 'ui.json'), 'utf8'));
  const coreData = JSON.parse(fs.readFileSync(path.join(DATA_DIR, 'core.json'), 'utf8'));
  const domSrc = fs.readFileSync(path.join(SRC_DIR, 'dom_engine.js'), 'utf8');

  const dataCode = `const dictionary = ${JSON.stringify(uiData)};\nconst coreWords = ${JSON.stringify(coreData)};`;
  const builtDomEngine = domSrc.replace('//__DATA_PLACEHOLDER__', dataCode);

  const domDistPath = path.join(DIST_DIR, 'dom_engine.js');
  fs.writeFileSync(domDistPath, builtDomEngine, 'utf8');

  if (!verifySyntax(domDistPath)) {
    throw new Error('Built dom_engine.js syntax error');
  }
  console.log(`✓ dom_engine.js generated (${Buffer.byteLength(builtDomEngine)} bytes, ${Object.keys(uiData).length} UI terms, ${Object.keys(coreData).length} core words)`);

  // 2. Build Menu Hooks
  const menuData = JSON.parse(fs.readFileSync(path.join(DATA_DIR, 'menu.json'), 'utf8'));
  const contextMenuData = JSON.parse(fs.readFileSync(path.join(DATA_DIR, 'context_menu.json'), 'utf8'));
  const menuSrc = fs.readFileSync(path.join(SRC_DIR, 'menu_hook.js'), 'utf8');

  const menuDataCode = `const menuTranslationMap = ${JSON.stringify(menuData, null, 2)};`;
  const ctxDataCode = `const contextMenuTranslationMap = ${JSON.stringify(contextMenuData, null, 2)};`;

  const builtMenuHook = menuSrc
    .replace('//__MENU_DATA_PLACEHOLDER__', menuDataCode)
    .replace('//__CONTEXT_MENU_DATA_PLACEHOLDER__', ctxDataCode);

  const menuDistPath = path.join(DIST_DIR, 'menu_hook.js');
  fs.writeFileSync(menuDistPath, builtMenuHook, 'utf8');

  if (!verifySyntax(menuDistPath)) {
    throw new Error('Built menu_hook.js syntax error');
  }
  console.log(`✓ menu_hook.js generated (${Buffer.byteLength(builtMenuHook)} bytes, ${Object.keys(menuData).length} menu items, ${Object.keys(contextMenuData).length} context items)`);

  console.log('Build completed successfully.');
}

if (require.main === module) {
  build();
}

module.exports = build;

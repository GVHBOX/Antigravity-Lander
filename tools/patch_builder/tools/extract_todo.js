const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const DATA_DIR = path.join(ROOT, 'data');
const UI_DATA = path.join(DATA_DIR, 'ui.json');

const uiDict = JSON.parse(fs.readFileSync(UI_DATA, 'utf8'));

// 扫描目标目录中 JS 文件的生词
function scanDirectory(dir, found = new Set()) {
  if (!fs.existsSync(dir)) return found;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const ent of entries) {
    const full = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      scanDirectory(full, found);
    } else if (ent.name.endsWith('.js') || ent.name.endsWith('.html')) {
      const content = fs.readFileSync(full, 'utf8');
      const re = /(?:label|title|description|placeholder|heading|tooltip|message)\s*:\s*(["'])((?:(?!\1)[^\\\n]|\\.){4,120})\1/g;
      let m;
      while ((m = re.exec(content))) {
        const str = m[2].trim();
        if (!uiDict[str] && /^[A-Z][a-zA-Z0-9\s.,!?:'-]+$/.test(str) && str.length > 3) {
          found.add(str);
        }
      }
    }
  }
  return found;
}

const targetDir = process.argv[2] || path.join(ROOT, '.tmp_extract');
if (!fs.existsSync(targetDir)) {
  console.log(`未找到提取目录: ${targetDir}。请先解包或指定目录路径。`);
  process.exit(1);
}

const todoSet = scanDirectory(targetDir);
const todoList = [...todoSet].sort();
const todoObj = {};
for (const item of todoList) {
  todoObj[item] = "";
}

const outPath = path.join(DATA_DIR, 'todo.json');
fs.writeFileSync(outPath, JSON.stringify(todoObj, null, 2), 'utf8');
console.log(`提取完成，共发现 ${todoList.length} 条待翻译候选词条，已写入 ${outPath}`);

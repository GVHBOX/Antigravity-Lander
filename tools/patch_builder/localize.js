// 向后兼容代理入口：将旧指令转发至 tools/engine.js
const { spawnSync } = require('child_process');
const path = require('path');

const engineScript = path.join(__dirname, 'tools', 'engine.js');
const args = process.argv.slice(2);

// 映射旧参数
let mappedArgs = [...args];
if (args.includes('--now')) {
  mappedArgs = mappedArgs.map(a => a === '--now' ? '--install' : a);
}

const result = spawnSync(process.execPath, [engineScript, ...mappedArgs], {
  stdio: 'inherit',
  cwd: __dirname
});

process.exit(result.status || 0);

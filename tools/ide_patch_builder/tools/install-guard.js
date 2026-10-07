const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const ext = require('./extension');
require('./ensure-dist')();

const FROM_PROJECT = ['package.json', 'extension.js', 'README.md'];
const FROM_DIST = ['panel-zh-overlay.js', 'workbench-zh-overlay.js'];

fs.mkdirSync(ext.installedDir, { recursive: true });
for (const file of FROM_PROJECT) fs.copyFileSync(path.join(ext.sourceDir, file), path.join(ext.installedDir, file));
for (const file of FROM_DIST) fs.copyFileSync(path.join(ROOT, 'dist', file), path.join(ext.installedDir, file));

console.log('installed', ext.id, '->', ext.installedDir);
console.log('files:', [...FROM_PROJECT, ...FROM_DIST].join(', '));
console.log('to disable without uninstalling: create a file named "disabled" inside the installed folder');

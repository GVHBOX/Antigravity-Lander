const os = require('os');
const path = require('path');

const PUBLISHER = 'beichen';
const NAME = 'antigravity-zh-guard';
const VERSION = '1.0.0';

module.exports = {
  PUBLISHER,
  NAME,
  VERSION,
  id: `${PUBLISHER}.${NAME}`,
  folderName: `${PUBLISHER}.${NAME}-${VERSION}`,
  sourceDir: path.join(__dirname, '..', 'guard'),
  installedDir: path.join(os.homedir(), '.antigravity-ide', 'extensions', `${PUBLISHER}.${NAME}-${VERSION}`),
};

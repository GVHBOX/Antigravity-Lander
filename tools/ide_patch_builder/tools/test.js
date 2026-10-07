const fs = require('fs');
const path = require('path');

require('./ensure-dist')();

const DIR = path.join(__dirname, '..', 'dist') + path.sep;

class El {
  constructor(tag, className = '') {
    this.nodeType = 1;
    this.tagName = tag.toUpperCase();
    this.className = className;
    this.attrs = {};
    this.childNodes = [];
    this.parentElement = null;
    this.style = {};
  }
  get children() { return this.childNodes.filter((n) => n.nodeType === 1); }
  get textContent() { return this.childNodes.map((n) => (n.nodeType === 3 ? n.nodeValue : n.textContent)).join(''); }
  set textContent(value) { this.childNodes = []; this.addText(value); }
  appendChild(node) { node.parentElement = this; this.childNodes.push(node); return node; }
  getAttribute(name) { return name in this.attrs ? this.attrs[name] : null; }
  setAttribute(name, value) { this.attrs[name] = String(value); }
  add(node) { return this.appendChild(node); }
  addText(value) { return this.appendChild(new Text(value)); }
  addEl(tag, className) { return this.appendChild(new El(tag, className)); }
  matches(selector) {
    for (const raw of selector.split(',')) {
      const part = raw.trim();
      if (!part) continue;
      const attr = part.match(/^\[([a-zA-Z-]+)\]$/);
      if (attr) { if (this.getAttribute(attr[1]) !== null) return true; continue; }
      if (part.startsWith('.')) {
        if (part.slice(1).split('.').every((c) => this.className.split(/\s+/).includes(c))) return true;
      } else if (!part.includes('.') && this.tagName === part.toUpperCase()) return true;
    }
    return false;
  }
  closest(selector) {
    let current = this;
    while (current) {
      if (current.matches(selector)) return current;
      current = current.parentElement;
    }
    return null;
  }
}

class Text {
  constructor(value) { this.nodeType = 3; this.nodeValue = value; this.parentElement = null; }
}

function descendants(node, out = []) {
  for (const child of node.childNodes || []) { out.push(child); descendants(child, out); }
  return out;
}

function buildTree() {
  const root = new El('html');
  const body = root.addEl('body');
  const sidebar = body.addEl('div', 'part sidebar');
  const fileName = sidebar.addText('Conversation.md');
  const fileLabel = sidebar.addEl('span');
  fileLabel.setAttribute('aria-label', 'New Conversation');
  const aux = body.addEl('div', 'part auxiliarybar');
  const panelTitle = aux.addText('New Conversation');
  const panelPlaceholder = aux.addEl('div');
  panelPlaceholder.setAttribute('placeholder', 'Ask anything, @ to mention, / for actions');
  const panelSettings = aux.addText('Browser CDP Port');
  const panelPattern = aux.addText('See all (12)');
  const codeHost = aux.addEl('pre');
  const codeText = codeHost.addText('const conversation = pin(project);');
  const ta = body.addEl('textarea');
  ta.setAttribute('placeholder', 'Describe the bug you encountered...');
  const taTyped = ta.addText('New Conversation typed by the user');
  const perf = [];
  for (let i = 0; i < 3000; i++) { perf.push(`Line ${i}: New Conversation Manage Antigravity app settings.`); aux.addText(perf[i]); }
  const nav = body.addEl('div', 'nav-footer');
  const feedbackItem = nav.addEl('span');
  feedbackItem.addText('Provide feedback');
  const document = {
    nodeType: 9,
    documentElement: root,
    readyState: 'interactive',
    querySelectorAll(selector) { return descendants(root).filter((n) => n.nodeType === 1 && n.matches(selector)); },
    querySelector(selector) { return this.querySelectorAll(selector)[0] || null; },
    createElement: (tag) => new El(tag),
    createTextNode: (value) => new Text(value),
    createTreeWalker(node) {
      const queue = node.nodeType === Node.DOCUMENT_NODE ? descendants(root) : descendants(node);
      let index = 0;
      return { nextNode: () => (index < queue.length ? queue[index++] : null) };
    },
  };
  return { document, body, root, refs: { fileName, fileLabel, panelTitle, panelPlaceholder, panelSettings, panelPattern, codeText, ta, taTyped, perf }, window: {} };
}

const Node = { ELEMENT_NODE: 1, TEXT_NODE: 3, DOCUMENT_NODE: 9, DOCUMENT_FRAGMENT_NODE: 11 };
const NodeFilter = { SHOW_TEXT: 4, SHOW_ELEMENT: 1 };
let observers = [];
class MutationObserver { constructor(cb) { this.cb = cb; observers.push(this); } observe() {} disconnect() {} }

function run(file) {
  observers = [];
  const source = fs.readFileSync(DIR + file, 'utf8');
  const { document, root, refs, window, body } = buildTree();
  const started = Date.now();
  new Function('window', 'document', 'Node', 'NodeFilter', 'MutationObserver', source)(window, document, Node, NodeFilter, MutationObserver);
  return { ms: Date.now() - started, root, refs, window, document, body, observers };
}

const cases = [];
function check(label, actual, expected) { cases.push([label, actual, expected]); }

const EN_PLACEHOLDER = 'Ask anything, @ to mention, / for actions';
const panel = run('panel-zh-overlay.js');
check('[panel] agent text', panel.refs.panelTitle.nodeValue, '新建对话');
check('[panel] sidebar aria-label', panel.refs.fileLabel.getAttribute('aria-label'), '新建对话');
check('[panel] pattern rule', panel.refs.panelPattern.nodeValue, '查看全部 (12)');
check('[panel] settings label', panel.refs.panelSettings.nodeValue, '浏览器 CDP 端口');
check('[panel] code untouched', panel.refs.codeText.nodeValue, 'const conversation = pin(project);');
check('[panel] textarea placeholder translated', panel.refs.ta.getAttribute('placeholder'), '描述你遇到的缺陷……');
check('[panel] typed text inside textarea untouched', panel.refs.taTyped.nodeValue, 'New Conversation typed by the user');
check('[panel] file name safe even unscoped', panel.refs.fileName.nodeValue, 'Conversation.md');
check('[panel] placeholder translated?', panel.refs.panelPlaceholder.getAttribute('placeholder') !== EN_PLACEHOLDER, true);
const credits = panel.document.querySelectorAll('[data-ag-zh-credit]');
check('[credit] injected exactly once', credits.length, 1);
check('[credit] text', credits.length ? credits[0].textContent : '(none)', '中文翻译 by BeiChen & Qwen');
const creditLink = credits.length ? credits[0].children.find((c) => c.tagName === 'A') : null;
check('[credit] name links to github', creditLink ? creditLink.getAttribute('href') : '(none)', 'https://github.com/Re-BeiChen');
check('[credit] opens in external window', creditLink ? creditLink.getAttribute('target') + '|' + creditLink.getAttribute('rel') : '(none)', '_blank|noopener noreferrer');
panel.observers[0].cb([{ type: 'childList', addedNodes: [] }], panel.observers[0]);
check('[credit] not duplicated after mutation', panel.document.querySelectorAll('[data-ag-zh-credit]').length, 1);

const scoped = run('workbench-zh-overlay.js');
check('[scope] agent text translated', scoped.refs.panelTitle.nodeValue, '新建对话');
check('[scope] agent placeholder translated?', scoped.refs.panelPlaceholder.getAttribute('placeholder') !== EN_PLACEHOLDER, true);
check('[scope] explorer filename UNTOUCHED', scoped.refs.fileName.nodeValue, 'Conversation.md');
check('[scope] explorer aria-label UNTOUCHED', scoped.refs.fileLabel.getAttribute('aria-label'), 'New Conversation');
check('[scope] code untouched', scoped.refs.codeText.nodeValue, 'const conversation = pin(project);');
check('[credit] scoped variant injects too', scoped.document.querySelectorAll('[data-ag-zh-credit]').length, 1);
check('[scope] html lang set', scoped.root.lang, 'zh-CN');
check('[scope] guard flag', scoped.window.__agIdeZhOverlay, 1);

const latePanel = scoped.body.addEl('div', 'part auxiliarybar');
const lateText = latePanel.addText('New Conversation');
const lateHeader = latePanel.addEl('h2');
lateHeader.addText('Agent');
const lateReal = scoped.body.addEl('div', 'antigravity-agent-side-panel');
const realText = lateReal.addText('New Conversation');
const gapText = lateReal.addText('AI may make mistakes. Double-check all generated code.');
const modelText = lateReal.addText('Gemini 3.5 Flash High');
const lateOutside = scoped.body.addText('New Conversation');
const flyout = scoped.body.addEl('div', 'context-view');
const flyLints = flyout.addText('Agent Auto-Fix Lints');
const flyGit = flyout.addText('Tab Gitignore Access');
const flySnooze = flyout.addText('Snooze');
const flyFooter = flyout.addEl('span');
flyFooter.addText('AI Shortcuts');
const quickPick = scoped.body.addEl('div', 'quick-input-widget');
const quickFile = quickPick.addText('Conversation.md');
const toast = scoped.body.addEl('div', 'notifications-toasts');
const toastFile = toast.addText('Cannot resolve Conversation.md');
const prose = flyout.addText('Turn on this feature when working on Review Policy');
const midAgent = flyout.addText('Open the Agent panel from the sidebar');
const knownSentence = flyout.addText('Rules help guide the behavior of Agent.');
const splitPrefix = flyout.addText('Back to ');
const splitNoun = flyout.addText('Agent');
const artifact = flyout.addText('Shortcuts');
const openAgent = flyout.addText('Open Agent');
const sounds = flyout.addText('Enable Sounds for Agent');
const advanced = flyout.addText('Advanced');
const wfSentence = flyout.addText('Workflows are saved prompts that Agent can follow. To trigger a workflow, type "/" in Agent.');
const dropdownValue = flyout.addEl('span');
dropdownValue.addText('On');
scoped.observers[0].cb([{ type: 'childList', addedNodes: [latePanel, lateReal, lateOutside, flyout, quickPick, toast] }], scoped.observers[0]);
check('[late] body-level portal translated', lateOutside.nodeValue, '新建对话');
check('[flyout] dropdown value anchored', dropdownValue.childNodes[0].nodeValue, '开');
check('[flyout] prose not mangled by anchors', prose.nodeValue, 'Turn on this feature when working on 审核策略');
check('[anchor] unknown sentence with Agent left English', midAgent.nodeValue, 'Open the Agent panel from the sidebar');
check('[anchor] known sentence fully translated', knownSentence.nodeValue, '规则用于引导智能体的行为。');
check('[split] prefix node untouched', splitPrefix.nodeValue, 'Back to ');
check('[split] noun node untouched, no mixed prose', splitNoun.nodeValue, 'Agent');
check('[panel] shortcuts nav label', artifact.nodeValue, '快捷键');
check('[settings] open agent', openAgent.nodeValue, '打开智能体');
check('[settings] enable sounds', sounds.nodeValue, '启用智能体提示音');
check('[settings] advanced whole-node', advanced.nodeValue, '高级');
check('[settings] workflow sentence', wfSentence.nodeValue, '工作流是智能体可以遵循的预设提示。要触发工作流，请在智能体中输入“/”。');
check('[guard] notification toast untouched', toastFile.nodeValue, 'Cannot resolve Conversation.md');
check('[flyout] auto-fix lint label', flyLints.nodeValue, '智能体自动修复 Lint');
check('[flyout] gitignore label', flyGit.nodeValue, 'Tab 读取 .gitignore 权限');
check('[flyout] snooze label', flySnooze.nodeValue, '暂缓提醒');
check('[flyout] footer tab label', flyFooter.childNodes[0].nodeValue, 'AI 快捷键');
check('[guard] quick-open file name untouched', quickFile.nodeValue, 'Conversation.md');
check('[late] panel mounted after boot translated', lateText.nodeValue, '新建对话');
check('[late] bare Agent header left as product term', lateHeader.childNodes[0].nodeValue, 'Agent');
check('[late] real side-panel class in scope', realText.nodeValue, '新建对话');
check('[late] disclaimer translated', gapText.nodeValue, 'AI 可能出错，请复核所有生成的代码。');
check('[late] model effort label translated', modelText.nodeValue, 'Gemini 3.5 Flash 高');

let failed = 0;
for (const [label, actual, expected] of cases) {
  const ok = actual === expected;
  if (!ok) failed++;
  console.log((ok ? 'PASS  ' : 'FAIL  ') + label.padEnd(34) + (ok ? '' : `expected ${JSON.stringify(expected)} got ${JSON.stringify(actual)}`));
}
console.log(`\n${failed} failing of ${cases.length}`);
console.log(`3012-node walk: unscoped ${panel.ms} ms | scoped ${scoped.ms} ms`);

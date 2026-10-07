const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
require('./ensure-dist')();
const source = fs.readFileSync(path.join(ROOT, 'dist', 'panel-zh-overlay.js'), 'utf8');
const pairs = require(path.join(ROOT, 'data', 'dictionary.json'));

class El {
  constructor(tag) { this.nodeType = 1; this.tagName = tag.toUpperCase(); this.className = ''; this.attrs = {}; this.childNodes = []; this.parentElement = null; }
  getAttribute(n) { return n in this.attrs ? this.attrs[n] : null; }
  setAttribute(n, v) { this.attrs[n] = v; }
  closest() { return null; }
  add(n) { n.parentElement = this; this.childNodes.push(n); return n; }
}
class Text { constructor(v) { this.nodeType = 3; this.nodeValue = v; this.parentElement = null; } }

// Strings copied out of the live UI, including product-name interpolations that a
// verbatim phrase table cannot match.
const SAMPLES = [
  'When toggled on, Antigravity IDE collects usage data to help Google enhance performance and features.',
  'Receive product updates, tips, and promotions from Google Antigravity IDE via email.',
  'Agent security mode',
  'Full access',
  'Select one of the three options.',
  'Note: A change to this setting will only apply to new messages sent to Agent. In-progress responses will use the previous setting value.',
  "When enabled, Agent will use IDE's shell integration to detect and report terminal command execution. When disabled, the agent will use its own shell. Restart the application for this to take effect.",
  'Your Plan: Google AI Pro',
  'Open Editor Settings',
  "When enabled, 'Explain and Fix' actions will continue in the current conversation instead of starting a new one.",
  'Terminal commands always require review and the agent cannot access files outside of its given workspaces.',
  'Configure workspace-specific permissions, resources, and customizations.',
  'Show 1 breakdowns',
  'Search MCP servers',
  '97.9% of the customization budget is available.',
  'You have used some of your weekly limit, it will fully refresh in 3 days, 2 hours.',
  'Select one of the three options. Agent settings and permissions can be further customized below.',
  'Configure the browser subagent. It requires',
];

const root = new El('html');
const body = root.add(new El('body'));
const nodes = new Map();
for (const en of Object.keys(pairs)) nodes.set(en, body.add(new Text(en)));
const sampleNodes = SAMPLES.map((s) => body.add(new Text(s)));

const Node = { ELEMENT_NODE: 1, TEXT_NODE: 3, DOCUMENT_NODE: 9, DOCUMENT_FRAGMENT_NODE: 11 };
const NodeFilter = { SHOW_TEXT: 4, SHOW_ELEMENT: 1 };
const document = {
  nodeType: 9, documentElement: root, readyState: 'complete',
  querySelectorAll: () => [],
  querySelector: () => null,
  createElement: () => { throw new Error('probe should not reach credit injection'); },
  createTreeWalker(node) {
    const queue = [];
    const walk = (n) => { for (const c of n.childNodes || []) { queue.push(c); walk(c); } };
    walk(node.nodeType === Node.DOCUMENT_NODE ? root : node);
    let i = 0;
    return { nextNode: () => (i < queue.length ? queue[i++] : null) };
  },
};
class MutationObserver { constructor(cb) { this.cb = cb; } observe() {} disconnect() {} }
new Function('window', 'document', 'Node', 'NodeFilter', 'MutationObserver', source)({}, document, Node, NodeFilter, MutationObserver);

const misses = [];
const wrong = [];
for (const [en, want] of Object.entries(pairs)) {
  const got = nodes.get(en).nodeValue;
  if (got === en) misses.push(en);
  else if (got !== want) wrong.push([en, want, got]);
}

console.log('dictionary entries probed:', Object.keys(pairs).length, '| unmatched:', misses.length, '| mismatched:', wrong.length);
misses.slice(0, 12).forEach((s) => console.log('   MISS  ' + s.slice(0, 100)));
wrong.slice(0, 6).forEach(([en, want, got]) => console.log('   DIFF  ' + en.slice(0, 44) + '\n         want ' + want.slice(0, 56) + '\n         got  ' + got.slice(0, 56)));

console.log('\n=== live UI samples ===');
let kept = 0;
sampleNodes.forEach((n, i) => {
  if (n.nodeValue === SAMPLES[i]) { kept++; console.log('KEEP-EN ' + SAMPLES[i].slice(0, 62)); }
});
console.log(kept === 0 ? 'all samples translated' : kept + ' sample(s) still English');

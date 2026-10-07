;(function () {
  if (window.__agIdeZhOverlay === 1) return;
  window.__agIdeZhOverlay = 1;
//__PHRASES__
  const SKIP_SELECTOR = 'script, style, textarea, code, pre, .xterm, .monaco-editor';
  const TRANSLATE_ATTRS = ['aria-label', 'title', 'placeholder', 'alt'];
  const SCOPE = null;
  const EXCLUDE = null;

  const LIMIT_NAMES = { weekly: '每周', daily: '每日', monthly: '每月', hourly: '每小时', '5-hour': '5 小时', 'five hour': '5 小时', 'five-hour': '5 小时' };
  const limitName = (value) => LIMIT_NAMES[value.trim().toLowerCase()] || value.trim();

  // Applied after the phrase table. Anchored rules only fire when a text node holds
  // nothing but that text, because React splits sentences across sibling nodes and a
  // loose match lands as "Back to 智能体".
  const patterns = [
    [/You have used (some|all) of your (.+?) limit, it will fully refresh in (.+?)\./g,
      (match, used, limit, time) => '你的' + limitName(limit) + '额度' + (used === 'all' ? '已用完' : '只用了一部分') + '，将在 ' + time + '后完全刷新。'],
    [/^(\d+)s$/, '$1 秒前'], [/^(\d+)m$/, '$1 分钟前'], [/^(\d+)h$/, '$1 小时前'],
    [/^(\d+)d$/, '$1 天前'], [/^(\d+)w$/, '$1 周前'], [/^(\d+)mo$/, '$1 个月前'], [/^(\d+)y$/, '$1 年前'],
    [/Worked for (\d+) seconds?/g, '已工作 $1 秒'],
    [/Worked for (\d+) minutes?/g, '已工作 $1 分钟'],
    [/Worked for (\d+) hours?/g, '已工作 $1 小时'],
    [/See all \((\d+)\)/g, '查看全部 ($1)'],
    [/^Show (\d+) breakdowns?$/, '展开 $1 项明细'],
    [/^Your Plan: (.+)$/, '你的套餐：$1'],
    [/([\d.]+)% of the customization budget is available\./g, '自定义预算还剩 $1%。'],
    [/^Select model, current: (.+)$/, '选择模型，当前：$1'],
    [/^Requesting permission to running this command outside the sandbox\b/g, '正在请求权限：在沙盒外运行此命令'],
    [/^Requesting permission to (read access to this path|write access to this path|reading this URL|running this command|using this MCP tool)\b/g, '正在请求权限：$1'],
    [/^Agent needs permission to (.+)$/, '智能体需要权限才能$1'],
    [/^(Yes, )?and always allow\b/g, '$1始终允许'],
    [/^No$/, '否'], [/^Allow$/, '允许'], [/^Deny$/, '拒绝'],
    [/^On$/, '开'], [/^Off$/, '关'], [/^Fast$/, '快'], [/^Slow$/, '慢'], [/^Start$/, '开启'],
    [/^Settings$/, '设置'], [/^Manage$/, '管理'], [/^Skills$/, '技能'], [/^Rules$/, '规则'],
    [/High$/, '高'], [/Medium$/, '中'], [/Low$/, '低'],
  ];

  const escapeRe = (value) => value.replace(/[|\\{}()[\]^$+*?.]/g, '\\$&');

  // Single-word table entries only match a whole text node. Bundles interpolate and
  // split text, so a bare noun kept in the streaming table produces mixed prose.
  const anchored = new Map();
  for (const [source, target] of [...phrases]) {
    if (!/\s/.test(source)) { phrases.delete(source); anchored.set(source, target); }
  }

  const entries = [...phrases]
    .filter(([source, target]) => typeof source === 'string' && typeof target === 'string' && source.length > 0)
    .sort((a, b) => b[0].length - a[0].length);
  const lookup = new Map(entries);

  // One alternation instead of rebuilding a RegExp per phrase per text node; this
  // table stays resident in the panel's load path and runs on every DOM mutation.
  const phrasePattern = new RegExp(entries.map(([source]) => {
    const lead = /^[A-Za-z0-9]/.test(source) ? '(?<![A-Za-z0-9])' : '';
    const tail = /[A-Za-z0-9]$/.test(source) ? '(?![A-Za-z0-9])' : '';
    return lead + escapeRe(source) + tail;
  }).join('|'), 'g');

  function applyTable(text) {
    let next = text;
    if (/[A-Za-z]/.test(text)) {
      phrasePattern.lastIndex = 0;
      next = text.replace(phrasePattern, (matched) => lookup.get(matched) || matched);
    }
    for (const [pattern, target] of patterns) next = next.replace(pattern, target);
    return next;
  }

  function translate(value) {
    if (!value) return value;
    const trimmed = value.trim();
    if (/[A-Za-z]/.test(value) && anchored.has(trimmed)) return value.replace(trimmed, anchored.get(trimmed));
    let next = applyTable(value);
    // The bundles interpolate the product name, so copy written for the standalone app
    // arrives here with "IDE" appended and misses the table. Retrying a normalised
    // copy only when the original matched nothing keeps the wording intact.
    if (next === value && value.includes('Antigravity IDE')) next = applyTable(value.replace(/Antigravity IDE/g, 'Antigravity'));
    return next;
  }

  function skipped(node) {
    const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    return !!element?.closest?.(SKIP_SELECTOR);
  }

  function inScope(node) {
    if (!SCOPE && !EXCLUDE) return true;
    if (node.nodeType === Node.DOCUMENT_NODE || node.nodeType === Node.DOCUMENT_FRAGMENT_NODE) return true;
    const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    if (!element) return !SCOPE;
    if (EXCLUDE && element.closest(EXCLUDE)) return false;
    return SCOPE ? !!element.closest(SCOPE) : true;
  }

  function translateAttributes(element) {
    for (const attr of TRANSLATE_ATTRS) {
      const value = element.getAttribute?.(attr);
      if (!value) continue;
      const next = translate(value);
      if (next !== value) element.setAttribute(attr, next);
    }
  }

  // Skipped containers must keep their content verbatim, but a placeholder is a UI
  // hint rather than user data, so textareas still get their hint text translated.
  function translatePlaceholder(element) {
    const value = element.getAttribute?.('placeholder');
    if (!value) return;
    const next = translate(value);
    if (next !== value) element.setAttribute('placeholder', next);
  }

  function translateSubtree(root) {
    if (!root || !inScope(root)) return;
    if (skipped(root)) {
      if (root.nodeType === Node.ELEMENT_NODE) translatePlaceholder(root);
      return;
    }
    if (root.nodeType === Node.TEXT_NODE) {
      if (root.__agIdeZh === root.nodeValue) return;
      const next = translate(root.nodeValue || '');
      root.__agIdeZh = next;
      if (next !== root.nodeValue) root.nodeValue = next;
      return;
    }
    if (root.nodeType === Node.ELEMENT_NODE) {
      translateAttributes(root);
      if (root.__agIdeZhChildren === root.childNodes.length) return;
      root.__agIdeZhChildren = root.childNodes.length;
    } else if (root.nodeType !== Node.DOCUMENT_NODE && root.nodeType !== Node.DOCUMENT_FRAGMENT_NODE) {
      return;
    }
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (!inScope(node)) continue;
      if (skipped(node)) {
        if (node.nodeType === Node.ELEMENT_NODE) translatePlaceholder(node);
        continue;
      }
      if (node.nodeType === Node.TEXT_NODE) translateSubtree(node);
      else translateAttributes(node);
    }
  }

  function roots() {
    return SCOPE ? [...document.querySelectorAll(SCOPE)] : [document];
  }

  // The app has no DOM "About" surface (Help ▸ About is a native dialog), so the
  // translation credit is appended to the settings nav footer. Anchors are matched by
  // their visible text so the injection survives class-name churn; if none is present
  // nothing is inserted.
  const CREDIT_PREFIX = '中文翻译 by ';
  const CREDIT_SUFFIX = ' & Qwen';
  const CREDIT_NAME = 'BeiChen';
  const CREDIT_URL = 'https://github.com/Re-BeiChen';
  const CREDIT_ANCHORS = ['Provide feedback', 'Provide Feedback', '提供反馈', 'Keyboard shortcuts', '快捷键'];

  function ensureCredit() {
    if (document.querySelector('[data-ag-zh-credit]')) return;
    for (const anchor of CREDIT_ANCHORS) {
      for (const el of document.querySelectorAll('button, a, div, span, li')) {
        if (el.children.length || (el.textContent || '').trim() !== anchor) continue;
        const host = el.parentElement && el.parentElement.parentElement ? el.parentElement.parentElement : el.parentElement;
        if (!host) return;
        const line = document.createElement('div');
        line.setAttribute('data-ag-zh-credit', '1');
        line.style.cssText = 'opacity:.5;font-size:11px;line-height:1.6;padding:8px 12px 0';
        line.appendChild(document.createTextNode(CREDIT_PREFIX));
        // Built from nodes, never innerHTML: the window requires a Trusted Types
        // policy for HTML sinks but not for textContent or anchor href.
        const link = document.createElement('a');
        link.setAttribute('href', CREDIT_URL);
        link.setAttribute('target', '_blank');
        link.setAttribute('rel', 'noopener noreferrer');
        link.style.cssText = 'color:inherit;text-decoration:underline';
        link.textContent = CREDIT_NAME;
        line.appendChild(link);
        line.appendChild(document.createTextNode(CREDIT_SUFFIX));
        host.appendChild(line);
        return;
      }
    }
  }

  function run() {
    document.documentElement.lang = 'zh-CN';
    for (const root of roots()) translateSubtree(root);
    ensureCredit();
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === 'characterData') { if (inScope(mutation.target)) translateSubtree(mutation.target); }
        else if (mutation.type === 'attributes') { if (inScope(mutation.target)) translateAttributes(mutation.target); }
        // childList: the target is the parent, which is usually out of scope even when
        // the added node is the panel itself mounting late.
        else for (const node of mutation.addedNodes) translateSubtree(node);
      }
      ensureCredit();
    });
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: TRANSLATE_ATTRS,
    });
    window.__agIdeZhObserver = observer;
  }

  if (document.readyState === 'loading') window.addEventListener('DOMContentLoaded', run, { once: true });
  else run();
})();

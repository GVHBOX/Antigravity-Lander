// Antigravity 2.0 Web UI DOM Localization Engine
(function() {
//__DATA_PLACEHOLDER__

const combinedDict = Object.assign({}, coreWords, dictionary);
  // 性能优化 1：预先构建全小写字典 Map，将未命中小写查找由 O(N) 降低为 O(1)
  const lowerDictionary = new Map();
  for (const k in dictionary) {
    const lk = k.toLowerCase();
    if (!lowerDictionary.has(lk)) {
      lowerDictionary.set(lk, dictionary[k]);
    }
  }

  const stringCache = new Map();
  const MAX_STRING_CACHE = 5000;

  const escapeRegExp = (str) => {
    const specials = ['[', ']', '(', ')', '{', '}', '*', '+', '?', '.', '^', '$', '|', '\\\\'];
    return str.split('').map(c => specials.includes(c) ? '\\\\' + c : c).join('');
  };

  // 性能优化 4：启动阶段一次性预编译单次联合分词正则，彻底消灭运行时 80 次循环迭代正则匹配
  const sortedCoreKeys = Object.keys(coreWords)
    .sort((a, b) => b.length - a.length)
    .filter(w => w.length > 2 || /^[a-zA-Z0-9]+$/.test(w));
  const escapedCoreUnion = sortedCoreKeys.map(w => escapeRegExp(w)).join('|');
  const CORE_WORDS_UNION_REGEX = new RegExp('\\b(' + escapedCoreUnion + ')\\b', 'gi');

  function translateString(text) {
    if (!text) return text;
    const trimmed = text.trim();
    if (!trimmed) return text;

    // 性能优化 2：极速短路。若文本完全不包含任何英文字母（纯中文/纯数字/纯标点），绝对无需查英文词典或跑正则，瞬间原样返回
    if (!/[a-zA-Z]/.test(trimmed)) {
      return text;
    }

    // 保护软件版本号字符串（如 "v2.21.1", "2.21.1", "v1.0.0-beta" 等），绝不进行翻译或分词改动
    if (/^v?\d+(\.\d+)+(-[a-zA-Z0-9.]+)?$/i.test(trimmed)) {
      return text;
    }

    // 0. 极速缓存查询（O(1) 命中瞬间返回）
    if (stringCache.has(trimmed)) {
      return text.replace(trimmed, stringCache.get(trimmed));
    }

    if (trimmed.includes('could not be opened')) {
      const replacedCouldNot = trimmed.replace(/could not be opened/gi, '无法打开');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, replacedCouldNot);
      return text.replace(trimmed, replacedCouldNot);
    }

    if (/Scan the code to/i.test(trimmed)) {
      if (/Scan the code to.*,\s*or\s*$/i.test(trimmed)) {
        const fixed = '扫描二维码以在远程控制中打开此设备，或';
        if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
        return text.replace(trimmed, fixed);
      }
      if (/Scan the code to.*(?:copy link|复制链接)/i.test(trimmed)) {
        const fixed = '扫描二维码以在远程控制中打开此设备，或复制链接。';
        if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
        return text.replace(trimmed, fixed);
      }
      if (/Scan the code to.*(?:Remote Control|远程\s*控制)/i.test(trimmed)) {
        const fixed = '扫描二维码以在远程控制中打开此设备';
        if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
        return text.replace(trimmed, fixed);
      }
    }

    if (/Continue your work from another device/i.test(trimmed)) {
      let fixed = '借助远程控制从另一台设备继续工作。请扫描下方二维码或打开下方链接。';
      if (!/Scan the QR code/i.test(trimmed)) {
        fixed = '借助远程控制从另一台设备继续工作。';
      }
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/Scan the QR code or open the link below/i.test(trimmed)) {
      const fixed = '扫描下方二维码或打开下方链接。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Send feedback as\s*[:：]?\s*(.+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Send feedback as\s*[:：]?\s*(.+)$/i, '以 $1 身份发送反馈');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Describe the bug you encountered[…\.]*$/i.test(trimmed)) {
      const fixed = (trimmed.endsWith('...') || trimmed.endsWith('…')) ? '请描述您遇到的 Bug...' : '请描述您遇到的 Bug';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Please list the steps to reproduce the issue[…\.:：]*$/i.test(trimmed)) {
      const fixed = /[.。]$/.test(trimmed) ? '请列出复现该问题的步骤。' : (/[：:]$/.test(trimmed) ? '请列出复现该问题的步骤：' : '请列出复现该问题的步骤');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:to\s+)?ask for content changes for legal reasons[.。]?$/i.test(trimmed)) {
      const hasDot = /[.。]$/.test(trimmed);
      const fixed = (trimmed.toLowerCase().startsWith('to') ? '以因法律原因申请更改内容' : '因法律原因申请更改内容') + (hasDot ? '。' : '');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Visit\s+Legal Help\s+to ask for content changes for legal reasons[.。]?$/i.test(trimmed)) {
      const fixed = '访问法律帮助以因法律原因申请更改内容。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Legal Help$/i.test(trimmed)) {
      const fixed = '法律帮助';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Open\s+IDE$/i.test(trimmed)) {
      const fixed = '打开 IDE';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Open\s+(?:in\s+)?Antigravity\s+IDE$/i.test(trimmed)) {
      const fixed = '在 Antigravity IDE 中打开';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^New\s+Worktree$/i.test(trimmed)) {
      const fixed = '新建工作树';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Local(\s*[\^v▾▼▴▲])?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Local/i, '本地');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^This page is going away soon[.。]?$/i.test(trimmed)) {
      const fixed = '此页面即将停用';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search conversations with.*in the top left[.。]?$/i.test(trimmed)) {
      const fixed = '使用左上角的 🔍 搜索对话。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search conversations with\s*$/i.test(trimmed)) {
      const fixed = '使用左上角';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^in the top left[.。]?$/i.test(trimmed)) {
      const fixed = '搜索对话。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^View archived in the sidebar.*menu\)[.。]?$/i.test(trimmed)) {
      const fixed = '在侧边栏查看已归档对话（通过筛选菜单）。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^View archived in the sidebar\s*\(?$/i.test(trimmed)) {
      const fixed = '在侧边栏查看已归档（';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Create with Prompt$/i.test(trimmed)) {
      const fixed = '通过提示词创建';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^[+＋]?\s*New$/i.test(trimmed)) {
      const hasPlus = trimmed.startsWith('+') || trimmed.startsWith('＋');
      const fixed = hasPlus ? '+ 新建' : '新建';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^New\s+Automation$/i.test(trimmed)) {
      const fixed = '新建自动化';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Enter automation name[…\.]*$/i.test(trimmed)) {
      const fixed = (trimmed.endsWith('...') || trimmed.endsWith('…')) ? '输入自动化名称...' : '输入自动化名称';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Daily(\s*[▾▼▴▲v\^])?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Daily/i, '每天');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Weekly(\s*[▾▼▴▲v\^])?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Weekly/i, '每周');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Monthly(\s*[▾▼▴▲v\^])?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Monthly/i, '每月');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Hourly(\s*[▾▼▴▲v\^])?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Hourly/i, '每小时');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^around$/i.test(trimmed)) {
      const fixed = '大约在';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Prompt$/i.test(trimmed)) {
      const fixed = '提示词';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Enter a prompt for the agent to run[…\.]*$/i.test(trimmed)) {
      const fixed = (trimmed.endsWith('...') || trimmed.endsWith('…')) ? '输入供智能体运行的提示词...' : '输入供智能体运行的提示词';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^All automations run as Flash[.。]?$/i.test(trimmed)) {
      const fixed = '所有自动化均以 Flash 模型运行。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Add\s+Automation$/i.test(trimmed)) {
      const fixed = '添加自动化';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^No automations configured[.。]?$/i.test(trimmed)) {
      const fixed = '未配置任何自动化。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search automations[…\.]*$/i.test(trimmed)) {
      const fixed = '搜索自动化...';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search customizations[…\.]*$/i.test(trimmed)) {
      const fixed = '搜索自定义配置...';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Installed$/i.test(trimmed)) {
      const fixed = '已安装';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^[+＋]\s*See\s+(\d+)\s+more$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^[+＋]\s*See\s+(\d+)\s+more$/i, '+ 展开其余 $1 项');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^See\s+(\d+)\s+more$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^See\s+(\d+)\s+more$/i, '展开其余 $1 项');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^[-－]\s*See\s+(\d+)\s+less$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^[-－]\s*See\s+(\d+)\s+less$/i, '- 收起其余 $1 项');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^See\s+(\d+)\s+less$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^See\s+(\d+)\s+less$/i, '收起其余 $1 项');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^[-－]\s*See less$/i.test(trimmed) || /^See less$/i.test(trimmed)) {
      const fixed = trimmed.startsWith('-') || trimmed.startsWith('－') ? '- 收起' : '收起';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^[+＋]\s*See more$/i.test(trimmed) || /^See more$/i.test(trimmed)) {
      const fixed = trimmed.startsWith('+') || trimmed.startsWith('＋') ? '+ 展开' : '展开';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Workspace$/i.test(trimmed)) {
      const fixed = 'Google 工作区';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Connect to Google Workspace, including Docs, Sheets, Slides, Drive, and Calendar[.。]?$/i.test(trimmed) ||
        /连接到 Google Workspace，包括 Docs、Sheets、Slides、Drive 和 Calendar/i.test(trimmed)) {
      const fixed = '连接到 Google 工作区，包括文档、表格、幻灯片、云端硬盘和日历。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Curated collection of agent skills for science(?: tasks)?[.。]?$/i.test(trimmed)) {
      const fixed = '专为科学计算、学术研究与实验探索任务精选的智能体技能集合。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Build applications with the Gemini Interactions API and Live API.*$/i.test(trimmed)) {
      const fixed = '使用 Gemini Interactions API 与 Live API 构建应用，包括文本、图像、视频和语音生成、托管智能体以及实时多模态流式交互。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Using the Google Antigravity Python SDK to build AI agents[.。]?$/i.test(trimmed)) {
      const fixed = '使用官方 Google Antigravity Python SDK 构建自定义 AI 智能体。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Prototype, build & run modern apps that users love with Firebase.*$/i.test(trimmed)) {
      const fixed = '基于 Firebase 的后端服务、AI 与运维基础设施，快速原型设计、构建并运行深受用户喜爱的现代应用程序。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Core tools and knowledge required to develop for Android[.。]?$/i.test(trimmed)) {
      const fixed = '面向 Android 应用开发所需的核心工具集与专业知识库。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Reliable automation, in-depth debugging, and performance analysis in Chrome using Chrome DevTools and Puppeteer[.。]?$/i.test(trimmed)) {
      const fixed = '在 Chrome 中结合 Chrome DevTools 与 Puppeteer 实现可靠的自动化、深度调试与性能分析。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Official plugin for Dart and Flutter that installs.*$/i.test(trimmed)) {
      const fixed = 'Dart 与 Flutter 官方插件，提供技能、规则、智能体与 MCP 服务，支持通过单一代码库构建原生高性能应用。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search and retrieve official documentation across Google Cloud.*$/i.test(trimmed)) {
      const fixed = '检索 Google Cloud、Gemini、Android、Firebase、Flutter 与 Web 的官方文档。需 Google Cloud 凭据 (ADC) 或 API 密钥。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Integration skill and tools for Google Maps Platform APIs[.。]?$/i.test(trimmed)) {
      const fixed = '面向 Google Maps Platform API 的集成技能与开发工具。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Keep your coding agent up to date with the latest web best practices[.。]?$/i.test(trimmed)) {
      const fixed = '让您的编程智能体紧跟最新的现代 Web 开发最佳实践。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^This plugin provides a specialized suite of skills for data engineers.*$/i.test(trimmed)) {
      const fixed = '专为 Google Cloud 数据工程师与数据库从业者打造的专业技能套件。支持使用自然语言构建复杂数据管道、编写 Spark 与 BigQuery SQL Notebook，并编排端到端工作流。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Build on Google's developer platforms, including Android, Chrome, Gemini, and Google Cloud[.。]?$/i.test(trimmed)) {
      const fixed = '基于 Google 开发者平台进行构建，包括 Android、Chrome、Gemini 和 Google Cloud。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Read, draft, and edit docs[.。]?$/i.test(trimmed)) {
      const fixed = '阅读、起草和编辑文档。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Read, write, and format spreadsheets[.。]?$/i.test(trimmed)) {
      const fixed = '读取、写入并格式化电子表格。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Read, edit, and export presentations[.。]?$/i.test(trimmed)) {
      const fixed = '阅读、编辑并导出演示文稿。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Search, upload, download, and share files[.。]?$/i.test(trimmed)) {
      const fixed = '搜索、上传、下载和共享文件。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Check availability, schedule events, and RSVP[.。]?$/i.test(trimmed)) {
      const fixed = '查看空闲时间、安排日程并回复邀请。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Docs$/i.test(trimmed)) {
      const fixed = 'Google 文档';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Sheets$/i.test(trimmed)) {
      const fixed = 'Google 表格';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Slides$/i.test(trimmed)) {
      const fixed = 'Google 幻灯片';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Drive$/i.test(trimmed)) {
      const fixed = 'Google 云端硬盘';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Google Calendar$/i.test(trimmed)) {
      const fixed = 'Google 日历';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Invalid tool call$/i.test(trimmed)) {
      const fixed = '无效的工具调用';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^No Project$/i.test(trimmed)) {
      const fixed = '无项目';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^See all\s*\(([^)]+)\)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^See all\s*\(([^)]+)\)$/i, '查看全部 ($1)');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Ran\s+(\d+)\s*(?:commands|命令)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Ran\s+(\d+)\s*(?:commands|命令)$/i, '已运行 $1 条命令');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Load older messages,\s*showing\s+(\d+)\s+of\s+(\d+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Load older messages,\s*showing\s+(\d+)\s+of\s+(\d+)$/i, '加载历史消息，正在显示 $1 / $2 条');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Fold lines\s+([0-9-]+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Fold lines\s+([0-9-]+)$/i, '折叠第 $1 行');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:Advanced\s*Settings|Advanced\s*设置|advanced\s*settings)$/i.test(trimmed)) {
      const fixed = '高级设置';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:Collapse\s*All|Collapse\s*all|collapse\s*all)$/i.test(trimmed)) {
      const fixed = '全部折叠';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:Expand\s*All|Expand\s*all|expand\s*all)$/i.test(trimmed)) {
      const fixed = '全部展开';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:Inherit\s*Global|Inherits\s*Global|继承\s*Global)$/i.test(trimmed)) {
      const fixed = '继承全局';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Inherits\s+your\s+Global\s+Permissions\s+when\s+working\s+in\s+this\s+project\.?$/i.test(trimmed)) {
      const fixed = '在此项目中工作时继承您的全局权限。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Also\s+includes.*(?:Global\s+Permissions|全局权限).*when\s+working\s+in\s+this\s+project/i.test(trimmed)) {
      const fixed = '在当前项目中工作时，亦继承全局权限配置。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Modify permissions for/i.test(trimmed)) {
      const fixed = '修改针对文件系统、终端命令以及 MCP 工具的安全权限。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^(?:Tool[\s ]+Permissions|工具[\s ]*Permissions)$/i.test(trimmed)) {
      const fixed = '工具权限';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Setting up WSL:\s*(.+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Setting up WSL:\s*(.+)$/i, '正在配置 WSL: $1');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Installing into\s*(.+?)[…\.]*$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Installing into\s*(.+?)[…\.]*$/i, '正在安装到 $1…');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^This folder belongs to the WSL distro "([^"]+)", but this window is connected to "([^"]+)"\.?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^This folder belongs to the WSL distro "([^"]+)", but this window is connected to "([^"]+)"\.?$/i, '此文件夹属于 WSL 发行版“$1”，但当前窗口连接到“$2”。');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^This location cannot be opened in WSL:\s*(.+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^This location cannot be opened in WSL:\s*(.+)$/i, '无法在 WSL 中打开此位置: $1');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^The WSL distro "([^"]+)" is no longer installed\.?$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^The WSL distro "([^"]+)" is no longer installed\.?$/i, 'WSL 发行版“$1”已不再安装。');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Antigravity opened on Windows instead\.?$/i.test(trimmed)) {
      const fixed = 'Antigravity 已改为在 Windows 本地打开。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Connected to WSL:\s*(.+)$/i.test(trimmed)) {
      const fixed = trimmed.replace(/^Connected to WSL:\s*(.+)$/i, '已连接到 WSL: $1');
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/% of the (?:customization )?budget is (?:available|used)/i.test(trimmed)) {
      let fixed = trimmed;
      if (/^%\s*of the (?:customization )?budget is available[.。]?$/i.test(trimmed)) {
        fixed = '% 的自定义额度可用。';
      } else if (/^%\s*of the (?:customization )?budget is used[.。]?$/i.test(trimmed)) {
        fixed = '% 的自定义额度已使用。';
      } else if (/available/i.test(trimmed)) {
        fixed = trimmed.replace(/(\d+(?:\.\d+)?)% of the (?:customization )?budget is available[.。]?/i, '自定义额度尚有 $1% 可用。');
      } else if (/used/i.test(trimmed)) {
        fixed = trimmed.replace(/(\d+(?:\.\d+)?)% of the (?:customization )?budget is used[.。]?/i, '已使用 $1% 的自定义额度。');
      }
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Plugins are packaged collections of/i.test(trimmed)) {
      let fixed = '';
      if (/Google developer products/i.test(trimmed)) {
        if (/change your choices in Settings/i.test(trimmed)) {
          fixed = '插件是技能和 MCP 的打包集合，用于协助 Antigravity 中的智能体使用 Google 开发者产品。你可以随时在设置中更改你的选择。';
        } else {
          fixed = '插件是技能和 MCP 的打包集合，用于协助 Antigravity 中的智能体使用 Google 开发者产品。';
        }
      } else {
        fixed = '插件是技能和 MCP 的打包集合，用于协助智能体在';
      }
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^You can always change your choices in Settings[.。]?$/i.test(trimmed)) {
      const fixed = '你可以随时在设置中更改你的选择。';
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^Antigravity work with Google developer products/i.test(trimmed)) {
      let fixed = 'Antigravity 中协同 Google 开发者产品工作。';
      if (/change your choices in Settings/i.test(trimmed)) {
        fixed = 'Antigravity 中协同 Google 开发者产品工作。你可以随时在设置中更改你的选择。';
      }
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    if (/^work with Google developer products/i.test(trimmed)) {
      let fixed = '协同 Google 开发者产品工作。';
      if (/change your choices in Settings/i.test(trimmed)) {
        fixed = '协同 Google 开发者产品工作。你可以随时在设置中更改你的选择。';
      }
      if (stringCache.size < MAX_STRING_CACHE) stringCache.set(trimmed, fixed);
      return text.replace(trimmed, fixed);
    }

    // --- Dynamic Agent Logs Regex Rules (Fixed Escaping) ---
    let dynamicMatch = trimmed;
    let isDynamic = false;
    
    // 用户定制方案 3：思考了 (时间) 与 总耗时 (时间) 动态正则
    if (/^Thought (?:for|持续) (.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Thought (?:for|持续) (.+)$/i, '思考了 $1');
      isDynamic = true;
    }
    if (/^Worked (?:for|持续) (.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Worked (?:for|持续) (.+)$/i, '总耗时 $1');
      isDynamic = true;
    }
    if (/^\d+ files? changed(.*)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+) files? changed(.*)/i, '$1 个文件已更改$2');
      isDynamic = true;
    }
    if (/^(\d+)\s+searches?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+)\s+searches?/i, '$1 次搜索');
      isDynamic = true;
    }
    if (/^Edited (.*) \+(\d+) -(\d+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Edited (.*) \+(\d+) -(\d+)/i, '编辑 $1 (+$2 -$3)');
      isDynamic = true;
    }
    if (/^Canceled taskkill/.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Canceled (.*)/, '已取消 $1');
      isDynamic = true;
    }
    if (/^\d+(?:\.\d+)?% of the (?:customization )?budget is (?:available|used)[.。]?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch
        .replace(/(\d+(?:\.\d+)?)% of the (?:customization )?budget is available[.。]?/i, '自定义额度尚有 $1% 可用。')
        .replace(/(\d+(?:\.\d+)?)% of the (?:customization )?budget is used[.。]?/i, '已使用 $1% 的自定义额度。');
      isDynamic = true;
    }

    // 限额剩余标题动态匹配 (例如 "Five Hour Limit Remaining", "Weekly Limit Remaining", "5-hour limit remaining")
    if (/^(Weekly|Five[- ]Hour|5[- ]Hour|Hourly|Daily)\s+Limit\s+Remaining$/i.test(trimmed)) {
      const lower = trimmed.toLowerCase();
      if (lower.includes('weekly')) dynamicMatch = '每周限额剩余';
      else if (lower.includes('five') || lower.includes('5')) dynamicMatch = '5 小时限额剩余';
      else if (lower.includes('hourly')) dynamicMatch = '每小时限额剩余';
      else if (lower.includes('daily')) dynamicMatch = '每日限额剩余';
      isDynamic = true;
    }

    // 配额提示句 (含动态天数/小时/分钟，支持全英或半中文状态下自愈清洗)
    if (/(?:You have used some of your|您已使用了部分).*(?:limit|限额)/i.test(trimmed)) {
      dynamicMatch = dynamicMatch
        .replace(/^(?:You have used some of your|您已使用了部分)\s*(?:weekly|每周)\s*(?:limit|限额)?/i, '您已使用了部分每周限额')
        .replace(/^(?:You have used some of your|您已使用了部分)\s*(?:5[- ]hour|five[- ]hour|5 小时|五小时)\s*(?:limit|限额)?/i, '您已使用了部分 5 小时限额')
        .replace(/^(?:You have used some of your|您已使用了部分)\s*(?:hourly|每小时)\s*(?:limit|限额)?/i, '您已使用了部分每小时限额')
        .replace(/^(?:You have used some of your|您已使用了部分)\s*(?:daily|每日)\s*(?:limit|限额)?/i, '您已使用了部分每日限额')
        .replace(/(?:it will fully refresh in|它将在以下时间后完全刷新[：:]?)\s*/i, ' 它将在以下时间后完全刷新：')
        .replace(/(\d+)\s*days?/gi, ' $1 天')
        .replace(/(\d+)\s*hours?/gi, ' $1 小时')
        .replace(/(\d+)\s*minutes?\.?$/gi, ' $1 分钟')
        .replace(/\s+/g, ' ')
        .trim();
      isDynamic = true;
    }
    // 模型分组配额说明长句
    if (/^Within each group, models share/.test(trimmed)) {
      dynamicMatch = '在每个分组中，模型共享每周限额和 5 小时限额。配额按 token 成本比例消耗。因此，较短的任务或使用更具性价比的模型时，限额可持续更长时间。5 小时限额用于平滑总需求，以便在所有用户间公平分配全球容量，而每周限额则与您的个人等级直接挂钩。';
      isDynamic = true;
    }

    // 模型配额剩余动态匹配 (例如 "100% Remaining", "85.4% remaining")
    // 模型配额重置倒计时动态匹配 (例如 "Resets in 3h 6m", "Resets in 4h", "Resets in 1d 2h", "Resets in 45m", "Resets in 3 hours")
    if (/^(?:Next\s+)?Resets?(?:ting)?\s+in\s+/i.test(trimmed)) {
      let timePart = trimmed.replace(/^(?:Next\s+)?Resets?(?:ting)?\s+in\s+/i, '').trim();
      timePart = timePart
        .replace(/(\d+)\s*days?/gi, '$1 天')
        .replace(/(\d+)\s*hours?/gi, '$1 小时')
        .replace(/(\d+)\s*minutes?/gi, '$1 分钟')
        .replace(/(\d+)\s*seconds?/gi, '$1 秒')
        .replace(/(\d+)\s*d\b/gi, '$1 天')
        .replace(/(\d+)\s*h\b/gi, '$1 小时')
        .replace(/(\d+)\s*m\b/gi, '$1 分钟')
        .replace(/(\d+)\s*s\b/gi, '$1 秒')
        .replace(/\s+/g, ' ')
        .trim();
      dynamicMatch = timePart + '后重置';
      isDynamic = true;
    }

    if (/^(\d+(?:\.\d+)?%?)\s+remaining$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+(?:\.\d+)?%?)\s+remaining$/i, '$1 剩余');
      isDynamic = true;
    }
    // 剩余时间刷新动态匹配 (例如 "15 minutes", "1 hour 26 minutes", "2 hours", "1 day")
    if (/^(\d+)\s+hours?\s+(\d+)\s+minutes?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+)\s+hours?\s+(\d+)\s+minutes?$/i, '$1 小时 $2 分钟');
      isDynamic = true;
    }
    if (/^(\d+)\s+minutes?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+)\s+minutes?$/i, '$1 分钟');
      isDynamic = true;
    }
    if (/^(\d+)\s+hours?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+)\s+hours?$/i, '$1 小时');
      isDynamic = true;
    }
    if (/^(\d+)\s+days?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+)\s+days?$/i, '$1 天');
      isDynamic = true;
    }

    // 项目/路径不存在的动态提示 (项目名 + " does not exist"，超3词无法走分词)
    if (/^.+ does not exist\.?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(.+) does not exist\.?$/i, '$1 不存在');
      isDynamic = true;
    }
    // "xxx was not found" 动态提示
    if (/^.+ was not found\.?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(.+) was not found\.?$/i, '$1 未找到');
      isDynamic = true;
    }

    // 步骤与回合动态提示
    if (/^Step \d+ \([^\)]+\):?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Step (\d+) \(([^\)]+)\):?/i, '步骤 $1 ($2)：');
      isDynamic = true;
    }
    if (/^\d+ turns?$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^(\d+) turns?$/i, '$1 回合');
      isDynamic = true;
    }
    if (/^Turn \d+$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Turn (\d+)$/i, '第 $1 回合');
      isDynamic = true;
    }
    if (/^Task id "[^"]+" finished with result:$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Task id "([^"]+)" finished with result:$/i, '任务 "$1" 执行完成：');
      isDynamic = true;
    }
    if (/^Tool is running as a background task with task id: (.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Tool is running as a background task with task id: (.+)$/i, '工具正在后台运行 (任务ID: $1)');
      isDynamic = true;
    }
    if (/^Thought (?:for|持续) (.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Thought (?:for|持续) (.+)$/i, '思考了 $1');
      isDynamic = true;
    }
    if (/^Thinking (?:for|持续) (.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Thinking (?:for|持续) (.+)$/i, '思考了 $1');
      isDynamic = true;
    }
    if (/^Thinking \((.+)\)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Thinking \((.+)\)$/i, '正在思考 ($1)');
      isDynamic = true;
    }
    if (/^Version\s+(\d+.*)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Version\s+(\d+.*)$/i, '版本 $1');
      isDynamic = true;
    }
    if (/^Updated\s+(.+)$/i.test(trimmed)) {
      dynamicMatch = dynamicMatch.replace(/^Updated\s+(.+)$/i, '更新于 $1')
        .replace(/\b(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{1,2})\b/gi, (m, mon, day) => {
          const monMap = { jan: 1, feb: 2, mar: 3, apr: 4, may: 5, jun: 6, jul: 7, aug: 8, sep: 9, oct: 10, nov: 11, dec: 12 };
          return monMap[mon.toLowerCase()] + '月' + day + '日';
        });
      isDynamic = true;
    }

    // 智能体设置：计划审核策略提示 (包含纯英文、部分翻译、夹生等各种形态)
    if (/(?:Type|输入)\s*\/\s*(?:and|并|和)?\s*(?:select|选择)?\s*['"]?plan['"]?\s*(?:to\s+have\s+the\s+agent\s+generate\s+a\s+plan|来让智能体生成计划)[。.]?/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? '输入 / 并选择 plan 来让智能体生成计划。' : '输入 / 并选择 plan 来让智能体生成计划';
      isDynamic = true;
    }
    if (/^(?:Type|输入)\s*\/\s*(?:and|并|和)?\s*(?:select|选择)?\s*['"]?plan['"]?\s*$/i.test(trimmed)) {
      dynamicMatch = '输入 / 并选择 plan ';
      isDynamic = true;
    }

    // 自定义配置明细展开/折叠动态规则 (例如 "Show 1 breakdown", "Hide 2 breakdowns", "Show all breakdowns")
    if (/^(?:Show|Hide)\s+(\d+)\s+breakdowns?$/i.test(trimmed)) {
      const isHide = /^Hide/i.test(trimmed);
      dynamicMatch = trimmed.replace(/^(?:Show|Hide)\s+(\d+)\s+breakdowns?$/i, isHide ? '隐藏 $1 项明细' : '显示 $1 项明细');
      isDynamic = true;
    }
    if (/^(?:Show|Hide)\s+all\s+breakdowns?$/i.test(trimmed)) {
      const isHide = /^Hide/i.test(trimmed);
      dynamicMatch = isHide ? '隐藏全部明细' : '显示全部明细';
      isDynamic = true;
    }
    if (/^(?:to\s+)?have\s+the\s+agent\s+generate\s+a\s+plan[。.]?$/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? '来让智能体生成计划。' : '来让智能体生成计划';
      isDynamic = true;
    }
    if (/^plan\s+to\s+have\s+the\s+agent\s+generate\s+a\s+plan[。.]?$/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? 'plan 来让智能体生成计划。' : 'plan 来让智能体生成计划';
      isDynamic = true;
    }
    if (/^(?:Type|输入)\s*\/\s*(?:and|并|和)\s*$/i.test(trimmed)) {
      dynamicMatch = '输入 / 并';
      isDynamic = true;
    }

    // 权限模式与执行策略描述 (图 1)
    if (/^Every command requires approval[。.]?$/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? '每条命令均需人工审批。' : '每条命令均需人工审批';
      isDynamic = true;
    }
    if (/^Ask before sensitive operations[。.]?$/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? '执行敏感操作前进行询问。' : '执行敏感操作前进行询问';
      isDynamic = true;
    }
    if (/^Run all commands without restrictions[。.]?$/i.test(trimmed)) {
      dynamicMatch = /[。.]\s*$/.test(trimmed) ? '无限制运行所有命令。' : '无限制运行所有命令';
      isDynamic = true;
    }

    // 排队消息提示与键盘快捷键气泡 (图 2)
    if (/^Enter\s+Sends\s+immediately$/i.test(trimmed)) {
      dynamicMatch = 'Enter 立即发送';
      isDynamic = true;
    }
    if (/^Alt\+Enter\s+Queues\s+after\s+the\s+turn$/i.test(trimmed)) {
      dynamicMatch = 'Alt+Enter 在本轮结束后排队';
      isDynamic = true;
    }
    if (/^Alt\+Enter\s+On\s+empty\s+prompt,\s+sends\s+next\s+in\s+queue$/i.test(trimmed)) {
      dynamicMatch = 'Alt+Enter 提示词为空时，发送队列中的下一条';
      isDynamic = true;
    }
    if (/^Sends\s+immediately$/i.test(trimmed)) {
      dynamicMatch = '立即发送';
      isDynamic = true;
    }
    if (/^Queues\s+after\s+the\s+turn$/i.test(trimmed)) {
      dynamicMatch = '在本轮结束后排队';
      isDynamic = true;
    }
    if (/^On\s+empty\s+prompt,\s+sends\s+next\s+in\s+queue$/i.test(trimmed)) {
      dynamicMatch = '提示词为空时，发送队列中的下一条';
      isDynamic = true;
    }

    // 计划审核策略拆分节点与过渡短语 (图 2)
    if (/^and\s+(?:select|选择)\s*$/i.test(trimmed)) {
      dynamicMatch = '并选择';
      isDynamic = true;
    }
    if (/^and\s+(?:select|选择)\s+plan$/i.test(trimmed)) {
      dynamicMatch = '并选择 plan';
      isDynamic = true;
    }

    // 模型用量面板刷新状态 (图 3)
    if (/^Refreshing[.…]*$/i.test(trimmed)) {
      dynamicMatch = /[.…]$/.test(trimmed) ? '正在刷新...' : '正在刷新';
      isDynamic = true;
    }

    // 快捷键列表条目 (图 5)
    if (/^Archive\s+This\s+Conversation[….]*$/i.test(trimmed)) {
      dynamicMatch = /[.…]$/.test(trimmed) ? '归档此对话...' : '归档此对话';
      isDynamic = true;
    }

    // 侧边栏/遥测栏动态计数分组标题与空态
    if (/^Files Changed(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Files Changed(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `已更改文件 ${m[1]}` : '已更改文件';
      isDynamic = true;
    }
    if (/^Background Tasks(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Background Tasks(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `后台任务 ${m[1]}` : '后台任务';
      isDynamic = true;
    }
    if (/^Terminals(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Terminals(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `终端 ${m[1]}` : '终端';
      isDynamic = true;
    }
    if (/^Uploads(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Uploads(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `上传 ${m[1]}` : '上传';
      isDynamic = true;
    }
    if (/^Subagents(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Subagents(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `子智能体 ${m[1]}` : '子智能体';
      isDynamic = true;
    }
    if (/^Artifacts(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^Artifacts(?:\s+(\d+))?$/i);
      dynamicMatch = m[1] !== undefined ? `工件 ${m[1]}` : '工件';
      isDynamic = true;
    }
    if (/^No subagents(?:\s+running|\s+found)?$/i.test(trimmed)) {
      dynamicMatch = '无子智能体';
      isDynamic = true;
    }
    if (/^No file changes$/i.test(trimmed)) {
      dynamicMatch = '无文件更改';
      isDynamic = true;
    }
    if (/^No artifacts generated$/i.test(trimmed)) {
      dynamicMatch = '未生成工件';
      isDynamic = true;
    }
    if (/^No artifacts$/i.test(trimmed)) {
      dynamicMatch = '无工件';
      isDynamic = true;
    }
    if (/^No uploads$/i.test(trimmed)) {
      dynamicMatch = '无上传';
      isDynamic = true;
    }
    if (/^No background tasks$/i.test(trimmed)) {
      dynamicMatch = '无后台任务';
      isDynamic = true;
    }
    if (/^No terminals$/i.test(trimmed)) {
      dynamicMatch = '无终端';
      isDynamic = true;
    }
    if (/^No staged changes$/i.test(trimmed)) {
      dynamicMatch = '无已暂存的更改';
      isDynamic = true;
    }

    // Git / Review 审查面板动态与静态短语匹配
    if (/^All changes since\s+(.+)$/i.test(trimmed)) {
      const branch = trimmed.replace(/^All changes since\s+/i, '').trim();
      dynamicMatch = `自 ${branch} 以来的所有更改`;
      isDynamic = true;
    }
    if (/^Staged index changes and working tree changes$/i.test(trimmed)) {
      dynamicMatch = '暂存区索引更改与工作区更改';
      isDynamic = true;
    }
    if (/^Files modified by the agent in this conversation$/i.test(trimmed)) {
      dynamicMatch = '此会话中由智能体修改的文件';
      isDynamic = true;
    }
    if (/^Files modified by the agent$/i.test(trimmed)) {
      dynamicMatch = '由智能体修改的文件';
      isDynamic = true;
    }
    if (/^Staged Changes$/i.test(trimmed)) {
      dynamicMatch = '已暂存的更改';
      isDynamic = true;
    }
    if (/^Agent Edits$/i.test(trimmed)) {
      dynamicMatch = '智能体编辑';
      isDynamic = true;
    }
    if (/^Uncommitted$/i.test(trimmed)) {
      dynamicMatch = '未提交';
      isDynamic = true;
    }
    if (/^Review$/i.test(trimmed)) {
      dynamicMatch = '审查';
      isDynamic = true;
    }
    if (/^(?:View|Show)\s+diff$/i.test(trimmed)) {
      dynamicMatch = '查看差异';
      isDynamic = true;
    }
    if (/^Show in File Explorer$/i.test(trimmed)) {
      dynamicMatch = '在文件资源管理器中显示';
      isDynamic = true;
    }
    if (/^Show in Finder$/i.test(trimmed)) {
      dynamicMatch = '在访达中显示';
      isDynamic = true;
    }
    if (/^Push$/i.test(trimmed)) {
      dynamicMatch = '推送';
      isDynamic = true;
    }

    // 快捷键组合菜单
    if (/^Open File(?:\s+(?:Ctrl|Cmd)\+[a-zA-Z0-9]+)?$/i.test(trimmed)) {
      dynamicMatch = trimmed.replace(/^Open File/i, '打开文件');
      isDynamic = true;
    }
    if (/^New Terminal(?:\s+(?:Ctrl|Cmd)\+[a-zA-Z0-9]+)?$/i.test(trimmed)) {
      dynamicMatch = trimmed.replace(/^New Terminal/i, '新建终端');
      isDynamic = true;
    }
    if (/^Record\s+Audio(?:\s+(?:Ctrl|Cmd)\+[a-zA-Z0-9]+)?$/i.test(trimmed)) {
      dynamicMatch = trimmed.replace(/^Record\s+Audio/i, '录制音频');
      isDynamic = true;
    }
    if (/^Send\s+message(?:\s+Enter|\s*\(Enter\)|\s*\n\s*Enter)?$/i.test(trimmed)) {
      dynamicMatch = trimmed.replace(/^Send\s+message/i, '发送消息');
      isDynamic = true;
    }
    if (/^(?:Select|选择)\s+Environment(?:\s*\((?:Ctrl|Cmd)[^)]+\))?$/i.test(trimmed)) {
      dynamicMatch = trimmed
        .replace(/^(?:Select|选择)\s+Environment/i, '选择环境')
        .replace(/\(Ctrl\+。\)/g, '(Ctrl+.)')
        .replace(/\(Cmd\+。\)/g, '(Cmd+.)');
      isDynamic = true;
    }
    if (/^Quick\s+(?:Start|启动|Launch)$/i.test(trimmed)) {
      dynamicMatch = '快速启动';
      isDynamic = true;
    }
    if (/^Sort\s+By[:：]?$/i.test(trimmed)) {
      dynamicMatch = (trimmed.endsWith(':') || trimmed.endsWith('：')) ? '排序方式：' : '排序方式';
      isDynamic = true;
    }
    if (/^Custom\s+Order$/i.test(trimmed)) {
      dynamicMatch = '自定义排序';
      isDynamic = true;
    }
    if (/^(Skills?)\s+Used(?:\s+(\d+))?$/i.test(trimmed)) {
      const m = trimmed.match(/^(Skills?)\s+Used(?:\s+(\d+))?$/i);
      dynamicMatch = `${m[1]} 已使用` + (m[2] ? ` ${m[2]}` : '');
      isDynamic = true;
    }
    if (/^(\d+)\s*([smhdw]|mo|yr?)\s+ago$/i.test(trimmed)) {
      const m = trimmed.match(/^(\d+)\s*([smhdw]|mo|yr?)\s+ago$/i);
      const uMap = { s: '秒', m: '分钟', h: '小时', d: '天', w: '周', mo: '个月', y: '年', yr: '年' };
      dynamicMatch = `${m[1]} ${uMap[m[2].toLowerCase()] || m[2]}前`;
      isDynamic = true;
    }
    if (/^(\d+)\s+(second|minute|hour|day|week|month|year)s?\s+ago$/i.test(trimmed)) {
      const m = trimmed.match(/^(\d+)\s+(second|minute|hour|day|week|month|year)s?\s+ago$/i);
      const uMap = { second: '秒', minute: '分钟', hour: '小时', day: '天', week: '周', month: '个月', year: '年' };
      dynamicMatch = `${m[1]} ${uMap[m[2].toLowerCase()] || m[2]}前`;
      isDynamic = true;
    }
    if (/^Instantly create a new project and folder to start building([.。]?)$/i.test(trimmed)) {
      const hasDot = /[.。]$/.test(trimmed);
      dynamicMatch = '立即创建新项目和文件夹以开始构建' + (hasDot ? '。' : '');
      isDynamic = true;
    }
    if (/^ago$/i.test(trimmed)) {
      dynamicMatch = '前';
      isDynamic = true;
    }
    if (/^just\s+now$/i.test(trimmed)) {
      dynamicMatch = '刚刚';
      isDynamic = true;
    }

    if (isDynamic) {
      return text.replace(trimmed, dynamicMatch);
    }
    // --- End Dynamic Regex ---

    // 1. Direct Literal Match (Exact match including punctuation)
    if (dictionary[trimmed]) {
      return text.replace(trimmed, dictionary[trimmed]);
    }

    // Normalizing internal multiple whitespace/newlines for multi-sentence cards
    const normalizedSpace = trimmed.replace(/\s+/g, ' ');
    if (dictionary[normalizedSpace]) {
      return text.replace(trimmed, dictionary[normalizedSpace]);
    }
    
    const trimmedLower = trimmed.toLowerCase();
    for (const key in dictionary) {
      if (key.toLowerCase() === trimmedLower) {
        return text.replace(trimmed, dictionary[key]);
      }
    }

    // 2. Intelligent Punctuation Stripping & Reconstruction
    let core = trimmed;
    let trailPunc = '';
    let matchPunc = '';

    // Strip trailing common punctuation
    const puncRegex = /(\.\.\.|…|\.|\?|!|:|：|？|！|。)$/;
    const match = core.match(puncRegex);
    if (match) {
      matchPunc = match[0];
      core = core.slice(0, -matchPunc.length).trim();
      
      // Determine the correct Chinese counterpart punctuation
      if (matchPunc === '.') trailPunc = '。';
      else if (matchPunc === '?') trailPunc = '？';
      else if (matchPunc === '!') trailPunc = '！';
      else if (matchPunc === ':') trailPunc = '：';
      else if (matchPunc === '：') trailPunc = '：';
      else if (matchPunc === '？') trailPunc = '？';
      else if (matchPunc === '！') trailPunc = '！';
      else if (matchPunc === '。') trailPunc = '。';
      else trailPunc = matchPunc; // keep ..., …
    }

    // Check stripped core in dictionary (O(1) exact or O(1) case-insensitive)
    const coreTranslated = dictionary[core] || lowerDictionary.get(core.toLowerCase()) || '';

    if (coreTranslated) {
      return text.replace(trimmed, coreTranslated + trailPunc);
    }

    // 3. Fallback to word-by-word ONLY for short strings (<= 3 words)
    // 如果短语中已经包含了中文字符（即原本就是汉化内容或中英混排），则严禁进入英文分词翻译
    // 这可以完美阻止像中英文混排短语被分词规则执行二次翻译导致重叠和污染
    if (/[\u4e00-\u9fa5]/.test(core)) {
      return text;
    }
    // This prevents long unmatched sentences from getting mangled into Chinglish.
    const wordsCount = core.split(/\s+/).filter(Boolean).length;
    if (wordsCount > 3) {
      return text; // Do not translate, keep original English sentence clean
    }

    let replaced = false;
    let temp = core.replace(CORE_WORDS_UNION_REGEX, (matched) => {
      const lower = matched.toLowerCase();
      if (coreWords[lower]) {
        replaced = true;
        return coreWords[lower];
      }
      return matched;
    });

    let finalTranslated = replaced ? temp : core;
    // 消除中文字符之间可能由分词替换残留的英文空格，提升翻译句子的连贯精致度
    finalTranslated = finalTranslated.replace(/([\u4e00-\u9fa5])\s+([\u4e00-\u9fa5])/g, '$1$2');
    // 特殊去重清洗：防止前置分词造成的“使用使用”与半中半英长句残留
    finalTranslated = finalTranslated.replace(/使用使用 Google 插件构建/g, '使用 Google 插件构建');
    finalTranslated = finalTranslated.replace(/Advanced\s*设置/gi, '高级设置');
    finalTranslated = finalTranslated.replace(/Collapse\s*All/gi, '全部折叠');
    finalTranslated = finalTranslated.replace(/Expand\s*All/gi, '全部展开');
    finalTranslated = finalTranslated.replace(/了解更多关于\s*继承\s*Global/gi, '了解更多关于 继承全局');
    finalTranslated = finalTranslated.replace(/继承\s*Global/gi, '继承全局');
    finalTranslated = finalTranslated.replace(/Also\s+includes\s*(?:Global\s*Permissions|全局权限)\s*when\s+working\s+in\s+this\s+project\.?[。.]?/gi, '在当前项目中工作时，亦继承全局权限配置。');
    finalTranslated = finalTranslated.replace(/Configure 智能体 执行[,\s]+queued 消息 delivery[,\s]+and 权限[。.]?/g, '配置智能体执行策略、消息队列发送机制以及安全权限。');
    finalTranslated = finalTranslated.replace(/Automatic 检查更新/g, '自动检查更新');
    finalTranslated = finalTranslated.replace(/每周限额\s*Remaining/gi, '每周限额剩余');
    finalTranslated = finalTranslated.replace(/五小时限额\s*Remaining/gi, '5 小时限额剩余');
    finalTranslated = finalTranslated.replace(/Claude and GPT 模型/g, 'Claude 与 GPT 模型');
    finalTranslated = finalTranslated.replace(/命令\s*palette/gi, '命令面板');
    finalTranslated = finalTranslated.replace(/Scan the code to .*(?:copy link|复制链接)[。.]?/gi, '扫描二维码以在远程控制中打开此设备，或复制链接。');
    finalTranslated = finalTranslated.replace(/Scan the code to (?:打开|open) this (?:设备|device) in 远程\s*(?:Control|控制)[,\s]+or (?:复制链接|copy link)[。.]?/gi, '扫描二维码以在远程控制中打开此设备，或复制链接。');
    finalTranslated = finalTranslated.replace(/(?:工作了\s*持续|总耗时\s*持续|Worked for)\s*(.+)/gi, '总耗时 $1');
    finalTranslated = finalTranslated.replace(/(?:Thought\s*持续|思考了\s*持续)\s*(.+)/gi, '思考了 $1');
    finalTranslated = finalTranslated.replace(/查看\s*could not be opened/gi, '查看文件无法打开');
    finalTranslated = finalTranslated.replace(/could not be opened/gi, '无法打开');
    finalTranslated = finalTranslated.replace(/(\d+)\s+searches?/gi, '$1 次搜索');
    finalTranslated = finalTranslated.replace(/(?:Type|输入)\s*\/\s*(?:and|并|和)?\s*(?:select|选择)?\s*['"]?plan['"]?\s*to\s+have\s+the\s+agent\s+generate\s+a\s+plan[。.]?/gi, '输入 / 并选择 plan 来让智能体生成计划。');
    finalTranslated = finalTranslated.replace(/输入\s*\/\s*并\s*选择\s*plan\s*来让智能体生成计划[。.]?/g, '输入 / 并选择 plan 来让智能体生成计划。');
    finalTranslated = finalTranslated.replace(/Quick\s*启动/gi, '快速启动');
    finalTranslated = finalTranslated.replace(/Quick\s*开始/gi, '快速开始');
    finalTranslated = finalTranslated.replace(/选择\s*Environment/gi, '选择环境');
    finalTranslated = finalTranslated.replace(/Skills?\s*Used/gi, 'Skills 已使用');
    finalTranslated = finalTranslated.replace(/Send\s*message\s*Enter/gi, '发送消息 Enter');
    if (matchPunc) {
      finalTranslated += trailPunc;
    }
    // 严禁将非空文本替换为空白或全空格字符串，若分词/规则产出为空则无条件保底原样返回
    if (!finalTranslated || !finalTranslated.trim()) {
      return text;
    }
    if (stringCache.size < MAX_STRING_CACHE) {
      stringCache.set(trimmed, finalTranslated);
    }
    return text.replace(trimmed, finalTranslated);
  }

  // 用于精确匹配代码编辑器、语法高亮等容器类名（收敛范围，防止误杀带 font-mono 或 viewer 的正常 UI）
  const codeClassPattern = /(?:^|[\s_-])(monaco-editor|editor-instance|hljs|shiki|prism|codemirror|line-content|gutter|codeblock|code-block|code-line|view-line)(?:$|[\s_-])/i;

  // 提及菜单 (@ Mentions) 分类白名单：放行汉化，同时保持斜杠命令 (/boost 等) 与代码文件名严格跳过
  const MENTION_CATEGORIES = new Set([
    'Rules',
    '规则',
    'Conversation',
    '对话',
    'PDF Document',
    'PDF 文档',
    'MCP Resource',
    'MCP 资源',
    'Browser Page',
    '浏览器页面',
    'Browser Text',
    '浏览器文本',
    'Directory',
    '目录',
    'Git Commit',
    'Git 提交',
    'Git Diff',
    'Git 差异'
  ]);

  const skipCache = new WeakMap();

  function shouldSkipNode(node) {
    if (!node) return true;
    
    // 如果是文本节点，我们检查其父元素；如果是属性/元素节点，检查自身
    const element = node.nodeType === Node.TEXT_NODE ? node.parentElement : node;
    if (!element) return false;

    // 1. 记忆化缓存查询：O(1) 瞬间返回，消除折叠栏展开时的万次重复检查
    if (skipCache.has(element)) {
      return skipCache.get(element);
    }

    // 2. 绝对不能翻译的脚本/样式/按键标签
    const skipTags = ['SCRIPT', 'STYLE', 'NOSCRIPT', 'KBD', 'SAMP', 'VAR'];
    if (skipTags.includes(element.tagName)) {
      skipCache.set(element, true);
      return true;
    }

    // 核心保护：斜杠命令与提及建议菜单的触发词标签（保持 /boost, /schedule, /browser 等原生命令标识不变）
    // 但针对提及菜单（@）中的分类标签（如 Rules -> 规则, Conversation -> 对话 等）放行汉化
    const isMenuOptionLabel = element.closest && element.closest('[data-testid="menu-option-label"]');
    if (isMenuOptionLabel) {
      const labelText = (isMenuOptionLabel.innerText || isMenuOptionLabel.textContent || '').trim();
      if (MENTION_CATEGORIES.has(labelText)) {
        skipCache.set(element, false);
        return false;
      }
      skipCache.set(element, true);
      return true;
    }

    // 思考过程触发药丸按钮（如“Thought for 4s”折叠栏标题）：必须放行汉化为“思考了 4s”
    const isThinkingTrigger = element.closest && element.closest('button[data-testid="thinking-collapsible-trigger"]');
    if (isThinkingTrigger) {
      skipCache.set(element, false);
      return false;
    }

    // 3. 特殊特权放行：针对执行步骤的药丸标签（如 Ran, Explored, Edited, Viewed, Thought, Thinking, Working 等）
    // 无论其父级为 SPAN、CODE 还是 BUTTON，只要是系统执行药丸且不在用户提问气泡内，一律无条件放行汉化
    const textContent = (element.innerText || element.textContent || '').trim();
    const isActionPill = textContent.length <= 25 && /^(Explored|Ran|Viewed|Edited|Thought|Thinking|Working|Review|Reviewed)$/i.test(textContent);
    if (isActionPill) {
      let inUserInput = false;
      let inThinkingContent = false;
      let checkCur = element;
      while (checkCur && checkCur !== document.body) {
        if (checkCur.classList) {
          if (checkCur.classList.contains('group/user-input-step') || checkCur.classList.contains('cursor-edit')) {
            inUserInput = true;
            break;
          }
        }
        if (checkCur.parentElement && checkCur.parentElement.querySelector) {
          const trigger = checkCur.parentElement.querySelector(':scope > button[data-testid="thinking-collapsible-trigger"]');
          if (trigger && checkCur !== trigger && !trigger.contains(checkCur)) {
            inThinkingContent = true;
            break;
          }
        }
        checkCur = checkCur.parentElement;
      }
      if (!inUserInput && !inThinkingContent) {
        skipCache.set(element, false);
        return false;
      }
    }

    if (element.tagName === 'CODE') {
      if (!isActionPill) {
        skipCache.set(element, true);
        return true;
      }
    }
    if (element.tagName === 'PRE') {
      skipCache.set(element, true);
      return true;
    }

    // 4. 输入框/文本域/富文本编辑器/用户提问气泡绝对跳过（输入前、输入中、发送后 100% 原样保留）
    if (element.tagName === 'INPUT' || element.tagName === 'TEXTAREA') {
      skipCache.set(element, true);
      return true;
    }
    if (element.getAttribute && (
        element.getAttribute('contenteditable') === 'true' ||
        element.getAttribute('role') === 'textbox' ||
        element.getAttribute('data-lexical-editor') === 'true'
    )) {
      skipCache.set(element, true);
      return true;
    }

    // 5. 检查元素自身是否带有代码语言标记属性
    if (element.getAttribute) {
      if (element.getAttribute('data-language') || 
          element.getAttribute('data-code') ||
          element.getAttribute('data-line') ||
          element.getAttribute('data-line-number')) {
        skipCache.set(element, true);
        return true;
      }
    }

    // 6. 向上递归检查祖先节点（带路径记忆化剪枝）
    let cur = element;
    let shouldSkip = false;
    while (cur && cur !== document.body) {
      if (skipCache.has(cur)) {
        shouldSkip = skipCache.get(cur);
        break;
      }

      // 核心防御：模型思考链正文容器绝对跳过
      // 思考链为 AI 运行时生成的自由英文/动态推理流，切勿进行逐词或断句翻译，否则会导致中英夹杂混乱
      if (cur.classList && (
        cur.classList.contains('cursor-edit') ||
        cur.classList.contains('thought-content') ||
        cur.classList.contains('thinking-content') ||
        cur.classList.contains('thought-box')
      )) {
        shouldSkip = true;
        break;
      }

      // 思考折叠栏内容区域：位于 thinking-collapsible-trigger 旁的展开正文容器
      if (cur.parentElement && cur.parentElement.querySelector) {
        const trigger = cur.parentElement.querySelector(':scope > button[data-testid="thinking-collapsible-trigger"]');
        if (trigger && cur !== trigger && !trigger.contains(cur)) {
          shouldSkip = true;
          break;
        }
      }

      // 用户输入框与富文本编辑器（输入前/输入中绝对不翻译）
      if (cur.getAttribute && (
          cur.getAttribute('contenteditable') === 'true' ||
          cur.getAttribute('role') === 'textbox' ||
          cur.getAttribute('data-lexical-editor') === 'true'
      )) {
        shouldSkip = true;
        break;
      }

      if (cur.getAttribute && (
          cur.getAttribute('data-language') || 
          cur.getAttribute('data-code') ||
          cur.getAttribute('data-line') ||
          cur.getAttribute('data-line-number') ||
          cur.getAttribute('role') === 'code'
      )) {
        shouldSkip = true;
        break;
      }

      if (cur.classList && (
        cur.classList.contains('group/user-input-step') ||
        cur.classList.contains('user-message') ||
        cur.classList.contains('cursor-text') ||
        cur.classList.contains('monaco-editor') || 
        cur.classList.contains('editor-instance') ||
        cur.classList.contains('input-area') ||
        cur.classList.contains('chat-input')
      )) {
        shouldSkip = true;
        break;
      }

      if (cur.className && typeof cur.className === 'string') {
        const lowerClass = cur.className.toLowerCase();
        if (
          lowerClass.includes('code-line') ||
          lowerClass.includes('view-line') ||
          codeClassPattern.test(cur.className)
        ) {
          shouldSkip = true;
          break;
        }
      }

      if (cur.tagName === 'PRE') {
        shouldSkip = true;
        break;
      }

      cur = cur.parentElement;
    }

    skipCache.set(element, shouldSkip);
    return shouldSkip;
  }

  // 性能优化 3：已处理文本节点记忆化 WeakSet，消除父容器重绘时的重复遍历
  const translatedNodes = new WeakSet();

  function shouldSkipElementSubtree(element) {
    if (!element || !element.tagName) return false;
    const tag = element.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SCRIPT' || tag === 'STYLE' || tag === 'NOSCRIPT' || tag === 'PRE') {
      return true;
    }
    if (tag === 'CODE') {
      const textContent = (element.innerText || element.textContent || '').trim();
      const isActionPill = textContent.length <= 25 && /^(Explored|Ran|Viewed|Edited|Thought|Thinking|Working|Review|Reviewed)$/i.test(textContent);
      if (!isActionPill) return true;
    }
    if (element.getAttribute) {
      if (
        element.getAttribute('contenteditable') === 'true' ||
        element.getAttribute('data-lexical-editor') === 'true' ||
        element.getAttribute('data-language') ||
        element.getAttribute('data-code') ||
        element.getAttribute('role') === 'code'
      ) {
        return true;
      }
    }
    if (element.classList) {
      if (
        element.classList.contains('monaco-editor') ||
        element.classList.contains('editor-instance') ||
        element.classList.contains('thought-content') ||
        element.classList.contains('thinking-content') ||
        element.classList.contains('thought-box') ||
        element.classList.contains('group/user-input-step') ||
        element.classList.contains('user-message') ||
        element.classList.contains('chat-input')
      ) {
        return true;
      }
    }
    if (element.className && typeof element.className === 'string') {
      if (codeClassPattern.test(element.className)) {
        return true;
      }
    }
    return false;
  }

  function translateNode(node) {
    if (!node) return;

    if (node.nodeType === Node.TEXT_NODE) {
      if (shouldSkipNode(node)) return;
      if (translatedNodes.has(node)) return;
      const original = node.nodeValue;
      if (!original || !original.trim()) return;

      if (node.parentElement) {
        const pText = node.parentElement.textContent || '';
        if (/plan/i.test(pText) && (pText.includes('智能体生成计划') || /agent generate a plan/i.test(pText))) {
          const t = original.trim();
          if (t === 'Type') {
            node.nodeValue = original.replace('Type', '输入');
            translatedNodes.add(node);
            return;
          }
          if (t === 'and') {
            node.nodeValue = original.replace('and', '并');
            translatedNodes.add(node);
            return;
          }
          if (/^and\s+(?:select|选择)$/i.test(t)) {
            node.nodeValue = original.replace(/and\s+(?:select|选择)/i, '并选择');
            translatedNodes.add(node);
            return;
          }
        }
      }
      const translated = translateString(original);
      if (translated && translated.trim() && original !== translated) {
        node.nodeValue = translated;
        translatedNodes.add(node);
      } else if (!/[a-zA-Z]/.test(original)) {
        translatedNodes.add(node);
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      ['placeholder', 'title', 'aria-label', 'data-placeholder', 'value'].forEach(attr => {
        if (node.hasAttribute && node.hasAttribute(attr)) {
          // 双重锁死：绝对不翻译任何输入框或编辑区的用户 value 属性
          if (attr === 'value' && (node.tagName === 'INPUT' || node.tagName === 'TEXTAREA')) {
            return;
          }
          const original = node.getAttribute(attr);
          if (original && (node.tagName !== 'INPUT' || node.type === 'button' || node.type === 'submit' || attr !== 'value')) {
            const translated = translateString(original);
            if (original !== translated) {
              node.setAttribute(attr, translated);
              if (attr === 'placeholder' && 'placeholder' in node) {
                try { node.placeholder = translated; } catch (_) {}
              }
            }
          }
        }
      });
      if (node.shadowRoot) {
        observeRoot(node.shadowRoot);
        translateNode(node.shadowRoot);
      }
      if (shouldSkipElementSubtree(node)) {
        return;
      }
      for (let i = 0; i < node.childNodes.length; i++) {
        translateNode(node.childNodes[i]);
      }
    } else if (node.nodeType === Node.DOCUMENT_FRAGMENT_NODE) {
      for (let i = 0; i < node.childNodes.length; i++) {
        translateNode(node.childNodes[i]);
      }
    }
  }

  const observerConfig = {
    childList: true,
    subtree: true,
    characterData: true,
    attributes: true,
    attributeFilter: ['placeholder', 'title', 'aria-label', 'value', 'data-placeholder']
  };

  const observedRoots = new WeakSet();
  let isTranslating = false;
  let batchRafHandle = null;
  const pendingAddedNodes = new Set();
  const pendingTextNodes = new Set();
  const pendingAttrNodes = new Map();

  const scheduleBatchFrame = (fn) => {
    if (typeof queueMicrotask === 'function') {
      queueMicrotask(fn);
    } else if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(fn);
    } else {
      setTimeout(fn, 0);
    }
  };

  function scheduleBatchTranslation() {
    if (batchRafHandle !== null) return;
    batchRafHandle = true;
    scheduleBatchFrame(processBatchTranslation);
  }

  function processBatchTranslation() {
    batchRafHandle = null;
    if (isTranslating) return;
    isTranslating = true;

    try {
      // 1. 批量处理属性变更
      if (pendingAttrNodes.size > 0) {
        for (const [target, attrs] of pendingAttrNodes) {
          for (const attrName of attrs) {
            if (attrName === 'value' && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
              continue;
            }
            const original = target.getAttribute(attrName);
            if (original && (target.tagName !== 'INPUT' || target.type === 'button' || target.type === 'submit' || attrName !== 'value')) {
              const translated = translateString(original);
              if (original !== translated) {
                target.setAttribute(attrName, translated);
                if (attrName === 'placeholder' && 'placeholder' in target) {
                  try { target.placeholder = translated; } catch (_) {}
                }
              }
            }
          }
        }
        pendingAttrNodes.clear();
      }

      // 2. 批量处理文本变更 (characterData)
      if (pendingTextNodes.size > 0) {
        for (const node of pendingTextNodes) {
          if (!shouldSkipNode(node)) {
            const original = node.nodeValue;
            if (original && original.trim()) {
              const translated = translateString(original);
              if (original !== translated) {
                node.nodeValue = translated;
                translatedNodes.add(node);
              } else if (!/[a-zA-Z]/.test(original)) {
                translatedNodes.add(node);
              }
            }
          }
        }
        pendingTextNodes.clear();
      }

      // 3. 批量处理新增节点（性能优化 6：祖先包含剪枝，彻底消灭 O(N^2) 嵌套递归）
      if (pendingAddedNodes.size > 0) {
        const rootNodes = [];
        for (const node of pendingAddedNodes) {
          let hasAncestor = false;
          let p = node.parentElement;
          while (p) {
            if (pendingAddedNodes.has(p)) {
              hasAncestor = true;
              break;
            }
            p = p.parentElement;
          }
          if (!hasAncestor) {
            rootNodes.push(node);
          }
        }
        pendingAddedNodes.clear();

        for (let i = 0; i < rootNodes.length; i++) {
          const node = rootNodes[i];
          if (node.shadowRoot) {
            observeRoot(node.shadowRoot);
          }
          translateNode(node);
        }
      }
    } catch (e) {
      console.error('Batch translation error:', e);
    } finally {
      isTranslating = false;
    }
  }

  function observeRoot(root) {
    if (!root || observedRoots.has(root)) return;
    observedRoots.add(root);

    const observer = new MutationObserver((mutations) => {
      let needsSchedule = false;
      for (let i = 0; i < mutations.length; i++) {
        const mutation = mutations[i];
        if (mutation.type === 'childList') {
          const added = mutation.addedNodes;
          for (let j = 0; j < added.length; j++) {
            const node = added[j];
            if (node.shadowRoot) {
              observeRoot(node.shadowRoot);
            }
            pendingAddedNodes.add(node);
            needsSchedule = true;
          }
        } else if (mutation.type === 'characterData') {
          translatedNodes.delete(mutation.target);
          pendingTextNodes.add(mutation.target);
          needsSchedule = true;
        } else if (mutation.type === 'attributes') {
          const target = mutation.target;
          let attrSet = pendingAttrNodes.get(target);
          if (!attrSet) {
            attrSet = new Set();
            pendingAttrNodes.set(target, attrSet);
          }
          attrSet.add(mutation.attributeName);
          needsSchedule = true;
        }
      }

      if (needsSchedule) {
        scheduleBatchTranslation();
      }
    });

    observer.observe(root, observerConfig);
  }

  // Hook attachShadow
  const originalAttachShadow = Element.prototype.attachShadow;
  Element.prototype.attachShadow = function() {
    const shadowRoot = originalAttachShadow.apply(this, arguments);
    observeRoot(shadowRoot);
    return shadowRoot;
  };

  function safeScan() {
    if (document.body) {
      try {
        translateNode(document.body);
      } catch (e) {
        console.error('Translation scan error:', e);
      }
      observeRoot(document.body);
    }
  }

  function startObserver() {
    safeScan();

    // 1. 渐进式多阶挂载兜底 (Progressive Mounting Backstop)
    // 应对托盘重开、温热加载下 React 异步挂载延迟与路由初次渲染
    [50, 150, 400, 1000, 2500].forEach(delay => {
      setTimeout(safeScan, delay);
    });

    // 2. 窗口激活 (Focus) 与可见性恢复 (VisibilityChange) 兜底
    // 无论是窗口从最小化还原、托盘重新唤醒，还是失焦后重新聚焦，自动触发增量补丁
    window.addEventListener('focus', safeScan);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') {
        safeScan();
      }
    });

    // 3. 单页应用 (SPA) 路由切换监听 (History API Hook)
    // 保证在不同会话、项目切换、从欢迎根路径跳转至具体任务视图时即时汉化
    try {
      const origPushState = history.pushState;
      history.pushState = function() {
        const ret = origPushState.apply(this, arguments);
        setTimeout(safeScan, 50);
        return ret;
      };
      const origReplaceState = history.replaceState;
      history.replaceState = function() {
        const ret = origReplaceState.apply(this, arguments);
        setTimeout(safeScan, 50);
        return ret;
      };
      window.addEventListener('popstate', () => setTimeout(safeScan, 50));
    } catch (_) {}
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', startObserver);
  } else {
    startObserver();
  }

})();

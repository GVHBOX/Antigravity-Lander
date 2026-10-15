// Antigravity 2.0 Native Menu & Context Menu Localization Hook

const menuTranslationMap = {
  "File": "文件",
  "Edit": "编辑",
  "View": "视图",
  "Window": "窗口",
  "Help": "帮助",
  "New Window": "新建窗口",
  "Docs": "使用文档",
  "Toggle Developer Tools": "开发者工具",
  "Check for Updates": "检查更新",
  "Checking for Updates...": "正在检查更新...",
  "Downloading Update...": "正在下载更新...",
  "Restart to Update": "重启以应用更新",
  "Undo": "撤销",
  "Redo": "重做",
  "Cut": "剪切",
  "Copy": "复制",
  "Paste": "粘贴",
  "Select All": "全选",
  "Minimize": "最小化",
  "Close": "关闭",
  "Quit Antigravity": "退出 Antigravity",
  "About Antigravity": "关于 Antigravity",
  "Services": "服务",
  "Hide Antigravity": "隐藏 Antigravity",
  "Hide Others": "隐藏其他",
  "Show All": "显示全部",
  "Force Reload": "强制重新加载",
  "Reload": "重新加载",
  "Actual Size": "实际大小",
  "Zoom In": "放大",
  "Zoom Out": "缩小",
  "Toggle Full Screen": "切换全屏",
  "Toggle Fullscreen": "切换全屏",
  "Reset Zoom": "重置缩放",
  "New Conversation": "新建对话",
  "Create Project": "创建项目",
  "New Project": "新建项目",
  "Create New Project": "创建新项目",
  "Open Project": "打开项目",
  "Command Palette": "命令面板",
  "Split": "分屏",
  "Split Right": "向右分屏",
  "Split Down": "向下分屏",
  "Replace With New": "替换为新建",
  "Remove From Split": "从分屏中移除",
  "Split Terminal": "拆分终端",
  "Split Conversation Vertically": "垂直分屏对话",
  "Split Conversation Horizontally": "水平分屏对话",
  "Equalize Split Panes": "均分分屏窗格",
  "Fork": "派生",
  "Fork Conversation": "派生对话",
  "Connect to WSL": "连接到 WSL",
  "Reopen Locally": "本地重新打开",
  "Show in File Explorer": "在文件资源管理器中显示",
  "Show in Finder": "在访达中显示",
  "Open File": "打开文件",
  "New Terminal": "新建终端",
  "Review": "审查",
  "View Diff": "查看差异",
  "View diff": "查看差异",
  "Push": "推送",
  "Pull": "拉取",
  "Commit": "提交",
  "Stage All Changes": "暂存所有更改",
  "Unstage All Changes": "取消暂存所有更改",
  "Discard All Changes": "放弃所有更改"
};

function translateMenu(menuItem) {
  if (!menuItem) return;
  if (menuItem.label && menuTranslationMap[menuItem.label]) {
    menuItem.label = menuTranslationMap[menuItem.label];
  }
  if (menuItem.submenu) {
    if (Array.isArray(menuItem.submenu.items)) {
      menuItem.submenu.items.forEach(translateMenu);
    } else if (Array.isArray(menuItem.submenu)) {
      menuItem.submenu.forEach(translateMenu);
    }
  }
}

const contextMenuTranslationMap = {
  "Cut": "剪切",
  "Copy": "复制",
  "Paste": "粘贴",
  "Select All": "全选",
  "Undo": "撤销",
  "Redo": "重做",
  "Delete": "删除",
  "New Conversation": "新建对话",
  "Fork Conversation": "派生对话",
  "Rename": "重命名",
  "Pin": "置顶",
  "Unpin": "取消置顶",
  "Close": "关闭",
  "Close Others": "关闭其他",
  "Close All": "全部关闭",
  "Copy Path": "复制路径",
  "Copy Relative Path": "复制相对路径",
  "Reveal in File Explorer": "在文件资源管理器中显示",
  "Reveal in Finder": "在访达中显示",
  "Show in File Explorer": "在文件资源管理器中显示",
  "Show in Finder": "在访达中显示",
  "Show in Explorer": "在资源管理器中显示",
  "Open File": "打开文件",
  "New Terminal": "新建终端",
  "Copy Content": "复制内容",
  "Search": "搜索",
  "Review": "审查",
  "View Diff": "查看差异",
  "View diff": "查看差异",
  "Open in Terminal": "在终端中打开",
  "Uncommitted": "未提交",
  "Stage Change": "暂存更改",
  "Stage Changes": "暂存更改",
  "Stage All Changes": "暂存所有更改",
  "Unstage Change": "取消暂存更改",
  "Unstage Changes": "取消暂存更改",
  "Unstage All Changes": "取消暂存所有更改",
  "Discard Change": "放弃更改",
  "Discard Changes": "放弃更改",
  "Discard All Changes": "放弃所有更改",
  "Open in New Window": "在新窗口中打开",
  "Open in Preview": "在预览中打开",
  "Open in Editor": "在编辑器中打开",
  "Split Down": "向下分屏",
  "Split Right": "向右分屏",
  "Reveal in Explorer": "在资源管理器中显示",
  "Format Document": "格式化文档",
  "Format Selection": "格式化选定内容",
  "Change All Occurrences": "更改所有匹配项",
  "Go to Definition": "转到定义",
  "Peek Definition": "速览定义",
  "Find All References": "查找所有引用",
  "Rename Symbol": "重命名符号",
  "Refactor...": "重构...",
  "Source Action...": "源代码操作...",
  "Clear Terminal": "清空终端",
  "Kill Terminal": "终止终端",
  "Restart Terminal": "重启终端",
  "Toggle Word Wrap": "切换自动换行",
  "Inspect Element": "检查元素"
};

function translateContextLabel(lbl) {
  if (!lbl) return '';
  return contextMenuTranslationMap[lbl] || lbl;
}

// 只挂载新增符号：整体替换 module.exports 会清掉宿主模块自己的导出，
// 宿主 main.js 随后调用 menu_1.setupApplicationMenu 就会抛错。
if (typeof module !== 'undefined' && module.exports) {
  module.exports.translateMenu = translateMenu;
  module.exports.translateContextLabel = translateContextLabel;
}

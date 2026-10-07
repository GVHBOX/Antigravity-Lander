# antigravity-ide-zh

<p align="center">
  <img src="icon.png" alt="Antigravity IDE 中文汉化补丁" width="128" height="128" />
</p>

<h3 align="center">Antigravity IDE 纯静态无损中文补丁</h3>

<p align="center">
  补全官方语言包覆盖不到的自研界面：智能体面板、设置中心、快捷控制弹层、快捷键说明页。<br />
  零常驻进程 · 校验和自动同步 · 启动自愈恢复 · 零外置依赖
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows-0078D6?logo=windows&logoColor=white" alt="Platform" />
  <img src="https://img.shields.io/badge/Node.js-%3E%3D18.0.0-339933?logo=node.js&logoColor=white" alt="Node" />
  <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" />
</p>

---

## 🌟 核心特性

- **直击痛点**：官方中文语言包仅能覆盖 VS Code 原生容器（菜单栏、文件树、命令面板），无法触达 Antigravity 自研的 React 界面。本项目针对自研面板提供精准运行时静态补丁。
- **纯静态无注入进程**：不开启调试端口、不常驻后台、不需要开机自启，直接在入口渲染尾部追加轻量翻译层。
- **无感校验和同步**：自动重新计算受控文件 SHA-256 并同步至 `product.json.checksums`，彻底消除 VS Code 底层“安装似乎损坏”的弹窗警告。
- **排除式作用域安全隔离**：智能跳过代码编辑区、终端、资源管理器文件名与面包屑，**杜绝把真实工程文件名误汉化**（如 `Conversation.md` 绝不会被误篡改）。
- **智能启动自愈**：内置轻量本地守护扩展，IDE 后续升级冲掉补丁时，在启动完成时自动静默修复并补齐校验和。
- **开箱即用零依赖**：基于 Node.js 原生 API 编写，无需额外 `npm install` 下载庞大依赖。

---

## 📸 界面效果

智能体面板（输入引导提示、模型档位选择、底部免责声明等）：

![智能体面板](docs/screenshot-panel.png)

> **小贴士**：若需要抓取其他界面的实际效果（设置窗口、悬浮弹窗等），打开目标窗口后直接执行：
> ```powershell
> npm run capture
> ```
> 脚本会自动按窗口句柄完成静默截屏（无需置于前台，不干扰鼠标指针）。

---

## 🚀 快速上手

### 环境要求
- **操作系统**：Windows（在 Antigravity 2.5.5 / VS Code 1.107.0 实测验证；macOS / Linux 路径解析已内置，欢迎测试反馈）
- **运行环境**：Node.js >= 18.0.0
- **前置准备**：已安装 Antigravity IDE

### 一键安装与生效

```powershell
# 1. 克隆本仓库
git clone https://github.com/Re-BeiChen/antigravity-ide-zh.git
cd antigravity-ide-zh

# 2. 一键自动化部署（无需 npm install）
npm run setup
```

完成后**重启 Antigravity IDE** 即可享受完整中文界面。

---

## 🛠️ `setup` 做了什么？

`npm run setup` 是完全幂等的引导命令，可反复安全执行：

1. **自动寻径与语言包引导**：自动定位 IDE 安装目录（亦支持通过环境变量 `AGIDE_APP` 手动指定），匹配安装兼容版本的官方中文语言包；
2. **Locale 配置写入**：将用户配置 `~/.antigravity-ide/argv.json` 的 `locale` 自动设为 `zh-cn`（原配置备份为 `argv.json.orig-nozh`）；
3. **安全打包与补丁注入**：
   - 严格进行文件头一致性检查与 `node --check` 语法校验；
   - 注入静态覆盖层并保留 `*.orig-nozh` 原生备份；
   - 自动更新 `product.json` 完整性校验哈希；
4. **安装启动自愈扩展**：部署 `guard/` 扩展至本地插件目录，IDE 后续自动更新时静默恢复汉化。

---

## 📖 常用维护命令

| 命令 | 说明 |
| :--- | :--- |
| `npm run setup` | 一键完成环境检测、语言包安装、补丁应用及自愈配置 |
| `npm run status` | 检查补丁是否就位、11 项完整性校验文件是否全绿匹配 |
| `npm run restart` | 优雅关闭并重启 IDE（使补丁立即生效） |
| `npm run restore` | 还原入口文件与 `product.json`（保留语言包与配置） |
| `npm test` | 执行自动化 DOM 替换断言与作用域边界测试 |
| `npm run probe` | 逐条核对词表在当前环境下的匹配命中率 |
| `npm run extract-todo` | 提取当前 IDE bundle 中存在但词表中尚未收录的生词 |

---

## 🔍 原理与设计细节

### 1. 双入口隔离作用域
| 入口脚本 | 对应界面 | 拦截策略 |
| :--- | :--- | :--- |
| `out/vs/code/electron-browser/workbench/jetskiAgent.js` | 独立 Agent 窗口、设置中心 | 全局替换 |
| `out/vs/code/electron-browser/workbench/workbench.js` | 主编辑器窗口（内嵌 Docked 面板、悬浮弹窗） | 严格排除模式 |

### 2. 排除式作用域机制
在主窗口中，放弃脆弱的正向类名白名单，采用**容器黑名单机制**：
拦截逻辑会自动规避 `.monaco-editor`、`.part.editor`、`.part.sidebar`、`.part.titlebar`、`.quick-input-widget`、`.xterm`、`.tabs-container` 等原生容器。既能无死角覆盖挂载于 `<body>` 下由 Tailwind 构成的 React Portal 弹层，又能保证工程真实代码、输出日志、文件节点绝对不被破坏。

### 3. 单字全等防穿透
针对 `Open`、`Settings`、`Project` 等高频词汇，引擎会将其从流式词表剥离，**仅在文本节点完全与单字一致时才触发替换**，彻底杜绝“打开 the Agent panel”之类的机翻拼接现象。

### 4. 完整性校验（Anti-Corruption）
VS Code 的 `integrityService` 校验 11 个核心文件的 Base64 SHA-256。本工具会在覆盖层写入后实时同步更新 `product.json.checksums`，确保软件状态维持在纯净安全的 `isPure()` 基线。

---

## 🤝 贡献与词表扩充

所有翻译映射均保存在 `data/dictionary.json` 中，采用清晰的键值结构：
```json
{
  "English Text": "中文翻译"
}
```

### 扩充词条工作流：
1. 运行 `npm run extract-todo` 扫描未覆盖文本；
2. 在 `data/dictionary.json` 补充词条；
3. 执行 `npm run build && npm test` 验证断言通过；
4. 提交 Pull Request。

> **翻译规范**：
> - 常见专业术语（如 `Agent`、`MCP`、`GCP`、`Tab`、`Shell`、`CDP` 等）以及品牌名 `Antigravity` 保留英文不作替换；
> - 界面短语采用凝练的陈述语式，不主动追加句号。

---

## ⚠️ 已知限制与说明

- **原生标题栏**：Windows 原生标题栏上的 `Settings` 属于 Win32 原生窗口文本，不属于 Web DOM 范畴，无法由覆盖层处理；
- **第三方扩展数据**：MCP 插件市场描述、外部技能 Markdown 等动态抓取的数据正文保持原样，不属于本地界面文案范畴；
- **IDE 大版本更新**：若官方未来重大升级改动了入口逻辑，自愈模块如遇不可解析冲突会安全跳过，届时跟进本项目更新即可。

---

## 📄 开源许可

本项目基于 [MIT License](LICENSE) 协议开源。

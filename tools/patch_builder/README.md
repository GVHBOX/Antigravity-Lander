# Antigravity 2.0 桌面端汉化构建管线

Antigravity 桌面客户端（2.0+）深度汉化工程与自动化打包流水线。

## 架构说明

工程采用数据、源码与构建管线彻底分离的分层设计：

- **`data/`**：纯中英文映射数据层
  - `ui.json`：Web 前端 DOM 界面文案与提示词（1900+ 条目）
  - `core.json`：高频核心操作词汇与状态词
  - `menu.json`：Electron 原生主菜单与窗口菜单
  - `context_menu.json`：原生右键上下文菜单
- **`src/`**：运行时注入源码层（独立编写，受静态类型与语法高亮支持）
  - `dom_engine.js`：基于 `MutationObserver` 的高吞吐 DOM 监听、字符串匹配与代码区域防误伤过滤引擎
  - `menu_hook.js`：原生菜单与上下文菜单运行时拦截 Hook
- **`tools/`**：构建与调度管线（纯 CLI 接口）
  - `build.js`：将 `data/` 数据内联编译生成 `dist/` 注入脚本，内置 `node --check` 语法门禁
  - `engine.js`：核心操作引擎，支持原地解包注入、原位无损备份/还原与离线包预编译
  - `extract_todo.js`：自动化提取官方新版未收录生词工具
- **`dist/`**：编译产物目录
  - `dom_engine.js`：编译后内联全量词表的单文件注入脚本
  - `menu_hook.js`：编译后的原生菜单拦截脚本

## 常用操作

```bash
# 1. 编译词典数据到 dist/
node tools/build.js

# 2. 一键安装汉化到当前系统 Antigravity
node tools/engine.js --install

# 3. 一键还原为官方原版英文状态
node tools/engine.js --restore

# 4. 检查当前客户端汉化状态
node tools/engine.js --status

# 5. 生成预编译离线汉化包到 data/patch/app.asar（供桌面管理器直接调用）
node tools/engine.js --build-patch
```

## 客户端更新后的行为

Antigravity 自动更新会整体替换 `resources/app.asar`，但不会清理同目录的 `app.asar.bak`。
因此汉化状态一律以 `app.asar` 内的注入标记为准，不看备份是否存在：

- `--status` 同时打印已装版本与离线包版本，两者不一致时明确标出；
- `--install` 以当前 `app.asar` 为基准重新注入，打包后校验注入标记，校验不通过就中止，不写半成品；
- `--restore` 在备份与已装版本不一致时不拿它还原（那等于把客户端降级），改为就地剥离汉化层，保留新版官方代码；
- 反复执行 `--install` 是幂等的：有同版本备份时先用它还原被改动的文件再重新注入，避免逐次重译导致文案劣化；
- 锚点失配（官方改了某句原文）集中汇总打印，不影响其余条目生效。

每次安装后同步刷新 `data/patch/app.asar` 与 `data/patch/app.asar.version.json`。
桌面管理器只在离线包版本与已装客户端一致时直接套用，否则改调 `engine.js` 现场重做，
避免把上一版代码覆盖到新客户端上。

## 词条维护工作流

1. 如需补充或修正汉化词条，直接编辑 `data/ui.json`、`data/menu.json` 或 `data/core.json`；
2. 运行 `node tools/build.js` 验证语法并通过编译；
3. 运行 `node tools/engine.js --install` 应用最新汉化生效。

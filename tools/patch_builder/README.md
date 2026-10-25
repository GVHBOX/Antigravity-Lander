# Antigravity 桌面端汉化载荷

本目录现在是**注入载荷库**，不是独立的 Node 工程。解包、注入、打包、校验全部由 Rust 侧完成，
运行时不依赖 Node.js。

## 目录职责

- **`data/`**：纯中英文映射数据
  - `ui.json`：Web 前端 DOM 文案与提示词（1900+ 条目）
  - `core.json`：高频核心词汇与状态词
  - `menu.json`：Electron 原生主菜单
  - `context_menu.json`：原生右键上下文菜单
- **`src/`**：运行时注入源码
  - `dom_engine.js`：基于 `MutationObserver` 的 DOM 监听与替换引擎（含代码区域防误伤）
  - `menu_hook.js`：原生菜单与右键菜单运行时 Hook

这 6 个文件在编译期被 `src/localize.rs` 用 `include_str!` 嵌进二进制，
改完必须重新 `cargo build` 才生效。

## 各环节位置

| 环节 | 位置 |
|---|---|
| asar 解包 / 打包 / integrity 计算 | `src/asar.rs` |
| 载荷合成（词典内联进模板） | `src/localize.rs` 的 `build_dom_engine` / `build_menu_hook` |
| 锚点表（22 对，正反共用） | `src/localize.rs` 的 `ANCHOR_TABLE` |
| 正向注入 | `src/localize.rs` 的 `apply()` |
| 反向剥离 | `src/localize.rs` 的 `strip()` |
| 备份校准 / 安装 / 卸载 | `src/launcher.rs` 的 `reconcile_backup` / `install_ag2_*` / `restore_ag2_*` |

## 词条维护

1. 编辑 `data/` 下的 JSON
2. `cargo build`（编译期嵌入，不重新构建不生效）
3. 在界面里点「安装」

需要扫描官方新版未收录的生词时，从 git 历史取回旧版 `tools/extract_todo.js`。

## 官方更新后的行为

Antigravity 自动更新会整体替换 `resources/app.asar`，但不清理同目录的 `app.asar.bak`。
所以汉化状态一律以 `app.asar` 内的注入标记为准，不看备份是否存在。

- **安装**：先按宿主版本校准 `app.asar.bak`。过期就刷新；已汉化又无法重建时作废旧备份，
  避免用旧版备份还原把客户端降级。之后套用同版本离线包，没有就现场注入
- **卸载**：备份版本与宿主一致时直接拷贝原版；不一致时按注入标记就地剥离再重新打包，
  不依赖备份存在
- **写回前校验**：无残留标记、`package.json` 可读、条目数与 unpacked 数一致、版本未变，
  任一不符即中止，不留半成品
- **幂等**：反复安装结果一致
- **锚点失配**（官方改了某句原文）集中汇总，不影响其余条目生效

## 约束

`ANCHOR_TABLE` 的译文必须与原文一一对应。两条英文原文若译成同一句中文，反向剥离时无法区分，
会丢信息。这条约束由 `localize::tests::test_anchor_table_is_symmetric` 守着。

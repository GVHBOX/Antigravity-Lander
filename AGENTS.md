# Antigravity Lander 作业规程

`D:\Development_Environment\AI_Vibe_Coding_Workspace\Antigravity-Lander`：Rust + Slint 桌面端 Antigravity Lander，负责派生与监控主账号及 4 个独立沙箱分身（`data/instance_1` ~ `instance_4`），并提供界面缩放调节、汉化管理与系统代理注入。
运行形态：Slint GUI（带系统托盘与桌面悬浮球）。
本文件定义工程规范与纪律，功能说明见 `README.md`。

## 5 条铁律

### 1. 代码零注释

- **核心源码范围**：`src/`（Rust 与 Slint）。禁止编写任何注释，通过清晰命名自解释。
- **违规定义**：Rust 的 `///`、`//!`，Slint 的 `//` 与 `/* */` 均属禁止范围。属性宏（如 `#[derive]`、`#[allow(...)]`）除外。
- **字符串例外**：仅限运行时实际读取的值（URL、命令行参数、环境变量名等）。
- **非核心范围**：`tools/` 目录保留必要 docstring 与说明，不在清理范围。

### 2. 克制白描，说人话（界面、文档与日志）

- **客观白描**：只陈述事实与结果，坚决摒弃营销级与夸张修饰词（如“极致”、“彻底”、“根治”、“灰飞烟灭”、“完美”、“绝对”、“神级”、“企业级”）与虚假技术概念。
- **结果导向**：界面与提示仅向用户呈现最终操作结果或具体阻断原因，禁止汇报内部实现路径或附带免责废话。
- **动词规范**：实例未运行时为【启动】；运行中为【打开】（激活置顶已有窗口）与【停止】（终止进程）。严禁使用“置顶”、“唤醒”、“回收”等代称。
- **诊断日志价值**：仅记录事后排查所需的确定性事实（时间、对象、PID、端口、路径、状态码、耗时、错误原文），禁止打印无信息增益的冗余日志。
- **界面零说明字（Zero UI Noise）**：界面仅允许呈现字段名、必填标记 `*`、出错具体原因、禁用原因与空状态占位提示，禁止教学引导与操作解释。
- **高信息密度支持**：在遵循零说明字前提下，鼓励以紧凑指标盒、分段度量条、存储占用统计等纯客观数据切片呈现高密度遥测，充分利用 1080P 与 2K 视口。

### 3. `.scratch/` —— 执行方案、实机截图与临时收容

- **子目录规约**：
  - `plans/`：**零例外方案锁**。修改 `src/` 核心源码（Rust 与 Slint）或数据结构前，无论任务大小或是否修 Bug，**强制先在实体 `.md` 落盘方案**，作为用户确认基线与防长上下文截断锚点。必须获得用户明确肯定指令后方可动工。
  - `shots/`：验收凭证硬证据。普通 Slint 静态界面使用 `slint-viewer` 导出；**涉及 Win32 原生窗口、悬浮球、托盘与桌面交互的，强制使用用户交互桌面原生原画屏幕截图与物理交互回执**，严禁仅凭单组件查看器臆断。
  - `archive/`：废弃代码、旧探针与临时产物归档。
  - `scripts/`：临时诊断与验证单次脚本。
- **根目录严格白名单**：源码（`src/`）、资源（`assets/`）、工具（`tools/`）、构建缓存（`target/`）、数据（`data/`）、配置文件与元文件（`Cargo.toml`、`Cargo.lock`、`build.rs`、`README.md`、`AGENTS.md`、`LICENSE`、`.gitignore`）以及发布产物（`Antigravity-Lander.exe`）。
- **资源保护**：`assets/icon.png` 为 Slint 唯一引用的图片资产（`.gitignore` 显式保留，禁止排除）；`assets/icon.ico` 为应用原生图标。
- **路径与跨会话约束**：Windows 路径采用原生格式 `D:\...`。禁止向 `.workbuddy/` 写入或重建该目录；不创建跨会话的记忆文件。

### 4. 先方案后动手，步步为营（防注意力稀释）

- **严禁大包揽**：多模块或跨语言联动任务，必须在方案中细化拆解为轻量、自闭环的独立小步（Step 1, Step 2...）。一次只做一件事，严禁一次性全量铺开修改导致上下文混乱、丢三落四。
- **单步闭环验证**：Slint 界面改完即刻截图确认；Rust 逻辑改完即刻 `cargo check --offline` 验证零警告。单步实测通过后再推进下一步，杜绝并发改动导致的问题失控与定位困难。
- **禁止盲动与反问即停机**：未得到用户明确发出“确认/执行/开始/通过”等肯定指令前，严禁修改任何核心代码。用户的反问、质问（如“你已经执行？”、“有问题吗？”）一律视为暂停（HALT）信号，必须立即停机答疑，严禁以此为由顺水推舟推进写操作。

### 5. 网络与代理

- **出口模式**：本机运行系统代理（`http://127.0.0.1:7890`），非 TUN 模式。Cargo 拉包时按需使用 `HTTP_PROXY` / `HTTPS_PROXY`。
- **故障排查原则**：单次实测获取错误原文，附带实际出口与浏览器连通性反馈用户确认，禁止盲目试探、重复重装运行时或将网络问题归咎于代码。

## 核心红线资产清单（严禁误删 / 破坏）

| 资产 / 路径 | 关联系统 | 破坏性后果与防护边界 |
|---|---|---|
| `data/instance_*/home/.gemini/` | 官方 OAuth Token | **严禁触碰或删除**；分身关闭后仅可清理 `Cache` / `Code Cache` / `GPUCache` |
| `data/instance_*/home/AppData/` | NTFS Junction 宿主映射 | 包含宿主 `Roaming/Microsoft` 与 `LocalLow` 映射，**严禁递归整目录删除** |
| `data/scale_settings.json` | 界面调节持久化配置 | 缩放比例存储文件，由前端回调安全读写，禁止破坏数据结构 |
| `assets/icon.png` / `icon.ico` | Slint 唯一图资 / 原生图标 | `.gitignore` 显式保留，严禁从版本库中移除或误排除 |

## 项目纪律与架构边界

- **槽位与沙箱目录 1:1 对齐**：分身 1~4 严格对应 `data/instance_1` ~ `instance_4`，禁止槽位编号错位。
- **界面缩放调节**：缩放控制统一标识为【界面调节】，支持 80%~200%（步长 10%）及重置，启动时通过 `--force-device-scale-factor` 注入。
- **视口与窗口特性**：
  - 基准尺寸：宽 1500px、高 870px（最小 1200×870），适配 1080P 并深度兼容 2K 高分屏。
  - 窗口置顶：置顶图钉（📌）与 Win32 `SetWindowPos(HWND_TOPMOST / HWND_NOTOPMOST)` 物理状态联动。
  - 窗口可见性必须调用 Slint 的 `window.hide()` 与 `window.show()`，严禁直接调用 Win32 `ShowWindow`，避免绕过脏矩形增量重绘导致透明黑屏。
  - 主循环必须使用 `slint::run_event_loop_until_quit()`，避免托盘隐藏时生命周期异常终止。
  - 托盘回调运行于独立线程，操作 UI 或窗口前必须经由 `slint::invoke_from_event_loop` 派发。
  - 进程识别：通过进程名匹配时必须排除 `lander` 自身；运行状态以系统实际扫描结果为准，不可信赖历史 PID。
  - 最小化至托盘时保留 `trim_working_set()` 调用。
- **原型保真度**：当指令要求按设计原型实现时，必须以原型为视觉与功能基准，对齐信息密度与组件排布，禁止防御性删减数据切片。Slint 缺乏对应高级控件时使用基础图元组合实现。
- **架构边界**：核心职能仅限调度与监控（派生、隔离、遥测、缓存清理、界面缩放、汉化与代理注入）。不碰主机实例凭证，不接管分身内部账号体系，不外发遥测数据。

## 交付规范与进程接管

- **原位物理覆盖**：编译完成后，必须将 `target/release/antigravity-lander.exe` 原位覆盖到根目录 `Antigravity-Lander.exe`，禁止创建别名文件（如 `_new.exe`）或仅滞留在构建目录。
- **旧进程接管授权**：若覆盖或测试时遭遇文件锁（WinError 32）或单实例互斥锁占用，授权调用 `taskkill /PID <pid> /F` 精准终止本项目历史构建的旧管理器进程，解除占用后立即完成物理覆盖。
- **验收凭证原则**：普通组件 UI 变更验证使用 `slint-viewer` 截取原生渲染图（严禁使用脚本调用 Win32 API 强制缩放窗口）；涉及 Win32 窗口、异形透明悬浮窗与托盘的桌面级渲染与交互，强制使用交互桌面屏幕原画截图（Screen Grab）与鼠标/消息物理回执取证；后台逻辑与进程启停以真实 PID 与系统回执为准。

## 环境事实与检查指令

- **环境参数**：
  - Rust 1.98.1 + MSVC。若在沙箱终端内执行且 `USERPROFILE` 被重定向，指定 `$env:CARGO_HOME = "C:\Users\GVH\.cargo"`。
  - 默认渲染后端：软件光栅（未设 `SLINT_BACKEND` 时强制 `winit-software`），界面文案禁止宣称硬件加速/GPU。
  - Slint 语法：变亮 `color.brighter(factor)`，变暗 `color.darker(factor)`，不存在 `color.lighter`。
  - 单实例互斥量与事件：`Local\AntigravityCockpit.SingleInstance` 与 `Local\AntigravityCockpit.Activate`。
  - `target/` 目录保留，不随意清理。
- **校验指令集**：
  ```bash
  # 1. 静态检查与离线构建
  cargo check --offline                       # 语法与类型检查（零警告通过）
  cargo build --release --offline             # 离线发布构建

  # 2. 纪律与卫生检查
  python tools/scan_comment.py                # 铁律 1 代码注释检查
  python tools/scan_comment.py --selfcheck    # 注释检查器校准
  python tools/check_front_hygiene.py         # 铁律 2 界面文案白描检查

  # 3. 实机原画取证（输出至 .scratch/shots/）
  slint-viewer --screenshot .scratch/shots/preview_default.png src/cockpit.slint -I assets
  slint-viewer --screenshot .scratch/shots/preview_2k.png --size 2560x1440 src/cockpit.slint -I assets
  ```

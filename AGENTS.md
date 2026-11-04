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
- **客观白描**：只陈述事实与测试结果，坚决摒弃营销级修饰词与虚假技术概念。
- **结果导向**：界面与提示仅向用户呈现最终操作结果或具体阻断原因，禁止汇报内部实现路径或附带免责废话。
- **动词规范**：实例未运行时为【启动】；运行中为【打开】（激活置顶已有窗口）与【停止】（终止进程）。严禁使用“置顶”、“唤醒”、“回收”等代称。
- **诊断日志价值**：仅记录排查所需的确定性事实（时间、对象、PID、端口、路径、状态码、耗时、错误原文），禁止打印无信息增益的冗余日志。
- **界面零说明字（Zero UI Noise）**：界面仅允许呈现字段名、必填标记 `*`、出错具体原因、禁用原因与空状态占位提示，禁止教学引导与操作解释。
- **高信息密度支持**：在遵循零说明字前提下，鼓励以紧凑指标盒、分段度量条、存储占用统计等纯客观数据切片呈现高密度遥测。

### 3. `.scratch/` —— 执行方案、实机截图与临时收容
- **子目录规约**：
  - `plans/`：方案实体 `.md` 落盘目录（防长上下文截断锚点）。
  - `shots/`：验收凭据实机截图。
  - `archive/`：废弃代码、旧探针与临时产物归档。
  - `scripts/`：临时诊断与验证单次脚本。
- **根目录严格白名单**：源码（`src/`）、资源（`assets/`）、工具（`tools/`）、构建缓存（`target/`）、数据（`data/`）、配置文件与元文件（`Cargo.toml`、`Cargo.lock`、`build.rs`、`README.md`、`AGENTS.md`、`LICENSE`、`.gitignore`）以及发布产物（`Antigravity-Lander.exe`）。
- **资源保护**：`assets/icon.png` 为 Slint 唯一引用的图片资产（`.gitignore` 显式保留，禁止排除）；`assets/icon.ico` 为应用原生图标。
- **路径规范**：Windows 路径采用原生格式 `D:\...`。

### 4. 先方案后动手，等「允许」总闸
- **严禁大包揽**：多模块或跨语言联动任务，必须在方案中细化拆解为轻量、自闭环的独立小步（Step 1, Step 2...）。一次只做一件事。
- **单步闭环验证**：Slint 界面改完即刻截图确认；Rust 逻辑改完即刻 `cargo check --offline` 验证零警告。单步实测通过后再推进下一步。
- **禁止盲动与等「允许」总闸**：涉及代码修改的任务必须先在实体 `.md` 落盘方案；出具方案末尾**强制单独提示「若方案认可，请回复「允许」开始动工」**，便于用户直接引用或单键回车授权。未得到用户明确发出「允许」（或等同肯定指令）前，严禁修改任何核心代码。用户的反问、质问一律视为暂停（HALT）信号，必须立即停机答疑，严禁以此为由推进写操作。

### 5. 网络与代理
- **网络出口模式**：以系统实际出口网络与浏览器连通性为准。若用户明确指定本地代理，按需注入环境变量，禁止盲目硬编码固定代理端口。
- **故障排查原则**：单次实测获取错误原文，附带实际连通性反馈用户确认，禁止盲目试探、重复重装运行时或将网络问题归咎于代码。

## 核心红线资产清单（严禁误删 / 破坏）

| 资产 / 路径 | 关联系统 | 破坏性后果与防护边界 |
|---|---|---|
| `data/instance_*/home/.gemini/` | 官方 OAuth Token | **严禁触碰或删除**；分身关闭后仅可清理 `Cache` / `Code Cache` / `GPUCache` |
| `data/instance_*/home/AppData/` | NTFS Junction 宿主映射 | 包含宿主 `Roaming/Microsoft` 与 `LocalLow` 映射，**严禁递归整目录删除** |
| `data/scale_settings.json` | 界面调节持久化配置 | 缩放比例存储文件，由前端回调安全读写，禁止破坏数据结构 |
| `data/widget_config.json` | 桌面悬浮球持久化配置 | 悬浮球坐标与模式配置，由前端回调安全读写，禁止破坏数据结构 |
| `assets/icon.png` / `icon.ico` | Slint 唯一图资 / 原生图标 | `.gitignore` 显式保留，严禁从版本库中移除或误排除 |

## 项目纪律与架构边界

- **槽位与沙箱目录 1:1 对齐**：分身 1~4 严格对应 `data/instance_1` ~ `instance_4`，禁止槽位编号错位。
- **界面缩放调节**：缩放控制统一标识为【界面调节】，走固定档位表 `SCALE_STEPS`（`src/launcher.rs`）：80/90/100/110/120/125/130/140/150/160/170/180/190/200，按钮按档位索引逐格移动，支持重置，启动时通过 `--force-device-scale-factor` 注入。
- **Cockpit 自身缩放**：`SLINT_SCALE_FACTOR` 恒定写死 `1.0`（`src/main.rs`），不可切换。界面观感为 1.5x（系 2250×1305 物理像素设计），非运行时缩放因子，任何 UI px 均按 1:1 物理像素书写。
- **视口与窗口特性**：
  - 基准尺寸：宽 2250px、高 1305px（最小 1800×1080），面向 2K 屏（2560×1440）设计。
  - 窗口置顶：置顶图钉（📌）与 Win32 `SetWindowPos(HWND_TOPMOST / HWND_NOTOPMOST)` 物理状态联动。
  - 窗口可见性必须调用 Slint 的 `window.hide()` 与 `window.show()`，严禁直接调用 Win32 `ShowWindow`，避免绕过脏矩形增量重绘导致透明黑屏。
  - 主循环必须使用 `slint::run_event_loop_until_quit()`，避免托盘隐藏时生命周期异常终止。
  - 托盘回调运行于独立线程，操作 UI 或窗口前必须经由 `slint::invoke_from_event_loop` 派发。
  - 进程识别：通过进程名匹配时必须排除 `lander` 自身；运行状态以系统实际扫描结果为准，不可信赖历史 PID。
  - 最小化至托盘时保留 `trim_working_set()` 调用。
- **原型保真度**：以设计原型为视觉与功能基准，对齐信息密度与组件排布，禁止防御性删减数据切片。
- **架构边界**：核心职能仅限调度与监控（派生、隔离、遥测、缓存清理、界面缩放、汉化与代理注入）。不碰主机实例凭证，不接管分身内部账号体系，不外发遥测数据。

## 交付规范与进程接管

- **原位物理覆盖**：编译完成后，必须将 `target/release/antigravity-lander.exe` 原位覆盖到根目录 `Antigravity-Lander.exe`，禁止创建别名文件（如 `_new.exe`）或仅滞留在构建目录。
- **旧进程接管授权**：若覆盖或测试时遭遇文件锁（WinError 32）或单实例互斥锁占用，授权调用 `taskkill /PID <pid> /F` 精准终止本项目历史构建的旧管理器进程，解除占用后立即完成物理覆盖。
- **验收凭证原则**：
  - 普通组件静态 UI 变更：使用 `SLINT_SCALE_FACTOR=1.0 slint-viewer` 截取原生渲染图（禁止用 `--size` 拉伸视口产生失真图）；
  - 桌面级交互与原生窗口：涉及 Win32 窗口、异形透明悬浮窗与托盘的交互，必须使用 `tools/snap_desktop.py` 捕获 1:1 桌面物理原画及系统回执取证；
  - 后台逻辑与进程启停：以真实 PID 与系统返回值为准。

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
  cargo check --offline                                     # 语法与类型检查（零警告通过）
  cargo build --release --offline                           # 离线发布构建

  # 2. 纪律与质量门禁（主检查与 --selfcheck 校准）
  python tools/scan_comment.py                              # 铁律 1 代码注释检查
  python tools/check_front_hygiene.py                       # 铁律 2 界面文案白描检查
  python tools/check_slint_bindings.py                      # Slint <-> Rust 回调绑定完整性检查
  python tools/check_sandbox_health.py                      # 沙箱 Junction 与红线隔离健康巡检
  python tools/check_architecture_boundaries.py             # Rust 模块分层与依赖单向性检查

  # 3. 实机原画取证（输出至 .scratch/shots/）
  SLINT_SCALE_FACTOR=1.0 slint-viewer --screenshot .scratch/shots/preview_default.png src/cockpit.slint -I assets
  python tools/snap_desktop.py                              # 桌面实机主视口原画捕获（1:1 物理 2250x1305）
  python tools/snap_desktop.py --target widget               # 桌面悬浮球实机原画捕获
  ```

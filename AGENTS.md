# Antigravity Lander 作业规程

`D:\AI\multi-antigravity-rust`：Rust + Slint 1.18 桌面端 Antigravity Lander，负责派生与监控主账号及 4 个独立沙箱分身（`data/instance_1` ~ `instance_4`），并提供界面缩放调节、汉化管理与系统代理注入。
两种运行形态：默认 Slint GUI（带系统托盘），`--tui` / `-t` 走 Ratatui 终端仪表盘。
本文件只讲规矩，功能介绍在 `README.md`。

## 4 条铁律

### 1. 代码零注释

核心源码 = `src/`（Rust 与 Slint）：不写注释，名字起清楚就行。
Rust 的 `///` / `//!` 也算注释，一样禁止；Slint 的 `//` 与 `/* */` 同样禁止。

`tools/` 不算核心源码，那里的 docstring / 注释照常写，别去清理。
例外只有：被运行时真正读取的字符串（URL、命令行参数、环境变量值）。
`#[derive]` / `#[allow(...)]` 之类的属性不算注释。
整理存量代码时，已有注释是清除对象（只清 `src/` 里的）。

**机器强制**：`tools/scan_comment.py`（`--selfcheck` 走校准件反向注入）。
校准件在 `tools/fixture-comments/`：`clean.rs` 里的 URL、raw string、字符字面量、生命周期都不得误报，
`dirty.rs` 必须命中。

```bash
python tools/scan_comment.py
python tools/scan_comment.py --selfcheck
```

### 2. 克制白描，说人话（界面、文档与日志）

核心哲学：**客观、克制、白描**。只陈述事实与结果，不加戏、不教导、不向上汇报，坚决摒弃技术装腔。
本条适用于所有给人看的文字：UI 界面、命令行提示、Toast/弹窗、文档 README、诊断日志。

1. **拒绝概念通胀，禁止大词包装**：
   - 严禁堆砌营销级与绝对化修饰词（如“极致”、“彻底”、“神级”、“企业级”、“物理级”）。
   - 讲机制不讲噱头：写真实机制，不造玄学大词；用最平实的工程事实陈述，自信源于代码本身，不靠形容词壮胆。
2. **结果导向，禁止报备与邀功**：
   - 界面、回执与 Toast 只告诉用户「最终结果」，不向上级汇报内部执行路径（严禁写“已调用底层接口触发…”、“服务正自动拉起…”）。
   - 报错只讲具体阻断原因，严禁掺杂自我安抚与免责式废话（如“操作已中止，核心系统未受任何影响”）。
   - 动词与短语一律用白话通识（“启动/停止/打开/退出”），禁止生造冷门借代词（如把“唤醒置顶”写成“置顶”，把“重启”写成“回收”，把“后台独立运行”写成“脱机运行”）。
3. **日志具备诊断价值，拒绝表演式输出**：
   - 诊断日志的唯一意义是**事后排查与故障定位**，只记录确定性事实：时间、对象、PID/端口/路径、状态码、关键耗时、错误原文。
   - 严禁打印毫无信息增益的赛博表演日志（如一连串“XX 模块就绪”、“通道已启用”、“守护逻辑已载入”）。
4. **界面零说明字（Zero UI Noise）**：
   - 界面只写「它是什么（字段/状态）」和「出了什么问题（错误具体原因）」，坚决不写「怎么用（引导/解释/教导）」。
   - 全界面严格仅允许五类文案：字段名 · 必填标记 `*` · 出错的具体原因 · 禁用态的原因 · 空状态占位提示。
   - 严禁在界面放置技术自嗨标签（如“Slint 1.18 软件光栅”）。
   - **高信息密度 ≠ 界面噪音（Dense Telemetry is NOT Noise）**：零说明字禁止的是教学引导与废话，严格鼓励多维工程事实与数据切片（如 3 列指标盒、多色分段 Prompt/Cache/Output 度量条、分身独立磁盘缓存统计等纯客观数据），充分利用 1080P 视口空间。

**机器强制**：`python tools/check_front_hygiene.py`。
注意它是**关键词黑名单**，只挡已知说法；中英文都要过一遍（英文文案扫不到），
新增文案仍要人工判断，别因为扫描绿了就放行。

### 3. 非源码一律进 `.scratch/`

报告、备份、脚本、截图、临时中间物全进 `.scratch/`（整体不进版本库）。
根目录只允许：源码（`src/`）、资源（`assets/`）、仓库元文件
（`README.md`、`AGENTS.md`、`Cargo.toml`、`Cargo.lock`、`build.rs`、`.gitignore`、
`tools/`）、`target/`、`data/`、`Antigravity-Lander.exe`（与过渡兼容的 `Multi-Antigravity.exe`）
（发布产物，`*.exe` 已被 `.gitignore` 排除，是手工复制出来的，不是构建输出）。

`assets/icon.png` 是 Slint 唯一引用的图片，**不能让 `.gitignore` 的 `*.png` 把它排除掉**
（`!assets/icon.png` 这条例外就是为此存在的，别删）。
`assets/icon.ico` 是用户自己设计的图标，只装容器，不重画不缩放。

- 交付路径用原生 Windows 路径 `D:\...`，不用 `/d/...`。
- **不往 `.workbuddy/` 写东西，也不重建它。**
- **不写跨会话的记忆 / 日志文件。** 权威在 `AGENTS.md` 与 `README.md`。

### 4. 网络与代理

本机是**系统代理**（`http://127.0.0.1:7890`），不是 TUN。
`cargo` 拉包需要时走 `HTTPS_PROXY` / `HTTP_PROXY` 环境变量。

**排查网络问题前先问用户，不要自己反复试。** 原因跨好几层（目标站点 / 代理工具 /
TUN 还是系统代理 / PAC / 安全软件 / 地区封锁），只有用户知道。
顺序：一次实测拿错误原文 → 连「程序实际用的什么出口 + 浏览器能不能开」一起告诉用户 → 等确认。
禁止：反复试探、反复重装 Runtime、把环境问题当成代码问题改代码。

## 项目纪律

- **槽位与沙箱目录 1:1 对齐**：
  分身 1 对应 `data/instance_1`，分身 2 对应 `data/instance_2`，分身 3 对应 `data/instance_3`，分身 4 对应 `data/instance_4`。
  禁止槽位编号与底层目录序号错位。
- **保护已登录凭证文件**：
  **不要动 `data/instance_1/home/.gemini/` 和 `data/instance_2/home/.gemini/`**：里面有真实的 OAuth token。
  `data/` 下能安全清理的只有分身关闭后的 `Cache` / `Code Cache` / `GPUCache`。
- **界面动词与直觉**：
  - 未运行时动词为【启动】；运行中动词为【打开】（激活置顶已有窗口）与【停止】（终止进程）。小白用户看到【打开】才符合直觉，严禁写成【置顶】或【唤醒】。
  - 缩放控制统一写【界面调节】，支持 80%~200%（±10%）及点击数值快速重置，持久化到 `data/scale_settings.json`，启动时通过 `--force-device-scale-factor` 注入。
- **UI 布局与窗口设计**：
  - 基准尺寸：宽 1516px、高 889px（适配 1080P，最小 1200×680），自适应弹性拉伸。
  - 上半区 3 列卡片布局：
    - 卡片 1【主机实例】：主账号控制、进程启停、界面缩放、目录直达。
    - 卡片 2【系统代理与汉化】：代理端口检测与环境变量注入、汉化补丁状态与一键应用。
    - 卡片 3【会话遥测中枢】：TPS、上下文 Token 用量三色分段条、延迟分布、沙箱存储占用。
  - 下半区【分身管理】：4 个分身平铺卡片，右上角设置深浅主题切换（☀/🌙）与窗口置顶图钉（📌）。
  - 工具带与标题栏边界：移除纯装饰性大字 Banner；允许并推荐保留高度 ≤ 36px 的紧凑功能工具带（全局一键启动/停止、刷新、系统状态摘要）。
  - 窗口标题栏与版本标识：原生标题栏显示名称与版本号（如 `Antigravity Lander v1.0.1`）。
- **窗口置顶联动**：
  - 置顶图钉（📌）与 Win32 `SetWindowPos(HWND_TOPMOST / HWND_NOTOPMOST)` 真实物理状态联动，支持点击切换并在高亮态表明置顶激活。
- **原型驱动与防降级减配（Prototype Fidelity）**：
  - 当存在设计原型（如 `.scratch/ui_prototype.html`）且指令为执行时，实现必须以原型为唯一视觉和功能基准，1:1 对齐信息密度与组件排布。
  - 严禁以“白描”、“零噪音”为由防御性删减原型中的数据切片、多列指标盒或状态条。
  - Slint 缺少对应高级控件时，使用 `Rectangle`、`HorizontalLayout`、`Path` 等基础图元组合实现，不得通过减配或退化结构规避布局难度。
- **窗口可见性必须走 Slint**：隐藏用 `window.hide()`、恢复用 `window.show()`，
  **不许直接用 Win32 的 `ShowWindow(SW_HIDE/SW_SHOW)`**。
  窗口是 `with_transparent(true)` 建的，软件渲染器按 dirty region 增量呈现；
  绕过 Slint 就跳过了它 `set_visibility` 里的整窗 `mark_dirty_region`，恢复后大面积不绘制 = 透明窗口。
- **主循环必须是 `slint::run_event_loop_until_quit()`**：`hide()` 会 release 一个 keepalive，
  用 `run_event_loop()` 的话最小化到托盘会直接把进程结束掉。
- **托盘回调在 tray-icon 自己的线程**：动 Slint 或窗口前必须 `slint::invoke_from_event_loop`。
- **进程判定要排除自己**：发布 exe 叫 `Antigravity-Lander.exe`（或 `multi-antigravity.exe`），
  任何按进程名 `contains("antigravity")` 的匹配都要排除 `multi` 和 `lander`，避免把自己算成主机实例。
- **状态只信扫描结果**：分身的「是否运行」以是否真扫到进程为准，
  不要把「上次 spawn 记下的 PID」当成运行状态的依据。
- **不要动 `trim_working_set()` 在缩托盘时的那次调用**。
- **交付真实性与热更新进程接管授权（防假交付）**：
  核心原则：**「代码写完」不等于「生效交付」**。必须以**目标路径的二进制产物完成原位物理覆盖、且加载运行最新代码**为交付闭环。严禁新建别名副本（如 `_new.exe`）或仅将产物留在构建目录（如 `target/release/`）。
  1. **严格区分「源码编辑」与「运行态生效」**：严禁将“代码已修改”等同于“已生效完成”。
  2. **热更新进程接管授权（主动解除文件锁与端口占用）**：
     - **正式接管授权**：在覆盖根目录/目标路径二进制（如 `Antigravity-Lander.exe` / `Multi-Antigravity.exe`）或启动测试实例时，若遭遇旧实例正在运行引发的文件锁（Windows `WinError 32` / `Device or resource busy`）或单实例互斥锁占用，**AI 获得正式授权主动接管并精准终止旧版管理器进程**（`taskkill /PID <pid> /F`）。
     - **原位物理覆盖**：解除占用后，必须立即完成根目录 `Antigravity-Lander.exe` 的覆盖写入，确保用户调起即为最新版。
     - **严格限定接管边界**：仅限终止由本项目自身构建运行的旧版进程，绝不触碰用户宿主环境或其他系统关键进程。
  3. **验收凭证原则（拒绝幽灵验证）**：自测必须有新产物运行的确定性证据（如新 PID、真实截屏存证）。

## 测试与检查

```bash
cargo check --offline                       # 零警告才算过
cargo build --release --offline             # 产物 target/release/antigravity-lander.exe
python tools/scan_comment.py                # 铁律 1
python tools/scan_comment.py --selfcheck
python tools/check_front_hygiene.py         # 铁律 2
```

- 项目没有单元测试。验收靠两步：`cargo check` 零警告 + 探针实测截图留存。
- 改动涉及窗口 / 托盘 / 渲染时，**必须起真 exe 验一遍**，别只靠编译。用 `tools/ui_probe.py`：

```bash
python tools/ui_probe.py launch            # 起临时实例，输出 PID
python tools/ui_probe.py rect <pid>        # 读真实像素尺寸 + DPI + 逻辑尺寸
python tools/ui_probe.py max <pid>         # 最大化，用来验证超宽屏下的布局
python tools/ui_probe.py shot <pid>        # 截窗口内容到 .scratch/
python tools/ui_probe.py kill <pid>
```

- **发布产物原位覆盖**：`cargo build --release --offline` 编译完成后，将 `target/release/antigravity-lander.exe` 原位覆盖到根目录 `Antigravity-Lander.exe`（并同步更新 `Multi-Antigravity.exe` 保持过渡兼容）。

## 环境与语法事实

- Rust 1.98.1 + MSVC（`~/.cargo/bin`）。
- **Bash 工具的 PATH 被破坏**，每条命令前要
  `export PATH="/c/Users/GVH/.workbuddy/binaries/PortableGit/versions/1.2.0/usr/bin:$PATH"`，
  cargo 要 `~/.cargo/bin/cargo`。
- **默认渲染后端是软件光栅**：`main.rs` 里没设 `SLINT_BACKEND` 时强制 `winit-software`。
  界面文案别写「硬件加速 / GPU」，与事实不符。
- **`target/` 5.2 GB 不动**：D 盘空间充足，清掉会延长全量重编时间。
- **Slint 1.18 颜色调节语法**：变亮为 `color.brighter(factor)`，变暗为 `color.darker(factor)`，**不存在 `color.lighter`**。
- **单实例互斥锁与激活事件**：基于 `Local\AntigravityCockpit.SingleInstance` 与 `Local\AntigravityCockpit.Activate`。启动新实例若检测到已有实例运行，会自动激活旧窗口并静默退出（退出码 0）。测试前需确保旧实例已清理。
- **沙箱终端执行环境**：若在分身沙箱启动的终端内执行，`USERPROFILE` 会指向 `data/instance_*/home`。若离线编译时 `cargo` 找不到 crates 缓存，请显式指定 `$env:CARGO_HOME = "C:\Users\GVH\.cargo"`。
- **沙箱目录 Junction 隔离保护**：分身 `home/` 下不仅个人文件夹是 Junction，`AppData` 内部亦包含指向宿主 `C:\Users\GVH\AppData\Roaming\Microsoft` 与 `LocalLow` 的 NTFS Junction，严禁直接递归删除沙箱 `AppData`。
- Bash 里不能出现 "PowerShell" 字样（会被安全策略拦），要跑就用 PowerShell 工具。

## 边界

只做「调度与监控」：派生、隔离、遥测、凭据清理、界面调节、汉化与代理注入。
**不碰主机实例的凭据**（`clear_token` 只删分身沙箱里对应那一份），不接管分身内部的账号体系，
不上传任何遥测数据。

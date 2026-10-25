use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const DOM_TEMPLATE: &str = include_str!("../tools/patch_builder/src/dom_engine.js");
const MENU_TEMPLATE: &str = include_str!("../tools/patch_builder/src/menu_hook.js");
const UI_JSON: &str = include_str!("../tools/patch_builder/data/ui.json");
const CORE_JSON: &str = include_str!("../tools/patch_builder/data/core.json");
const MENU_JSON: &str = include_str!("../tools/patch_builder/data/menu.json");
const CONTEXT_MENU_JSON: &str = include_str!("../tools/patch_builder/data/context_menu.json");

const DOM_PLACEHOLDER: &str = "//__DATA_PLACEHOLDER__";
const MENU_PLACEHOLDER: &str = "//__MENU_DATA_PLACEHOLDER__";
const CONTEXT_MENU_PLACEHOLDER: &str = "//__CONTEXT_MENU_DATA_PLACEHOLDER__";

const DOM_MARKER: &str = "// Antigravity 2.0";
const MENU_MARKER: &str = "// Antigravity 2.0 Native Menu";

const FILE_PRELOAD: &str = "dist/preload.js";
const FILE_WIZARD_PRELOAD: &str = "dist/ideInstall/wizardPreload.js";
const FILE_MENU: &str = "dist/menu.js";
const FILE_IPC: &str = "dist/ipcHandlers.js";
const FILE_TRAY: &str = "dist/tray.js";
const FILE_MAIN: &str = "dist/main.js";
const FILE_SPLASH: &str = "dist/provisionSplash.js";
const FILE_WSL: &str = "dist/wsl.js";
const FILE_WIZARD_HTML: &str = "dist/ideInstall/wizardHtml.js";

const ANCHOR_TABLE: &[(&str, &[(&str, &str)])] = &[
    (
        FILE_MENU,
        &[
            (
                r#"const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);"#,
                r#"const submenuItem = appMenu.items.find((item) => item.label === submenuLabel || (typeof menuTranslationMap !== "undefined" && item.label === menuTranslationMap[submenuLabel]));"#,
            ),
            (
                r#"electron_1.Menu.setApplicationMenu(menu);"#,
                r#"if (typeof translateMenu === "function") { menu.items.forEach(translateMenu); } electron_1.Menu.setApplicationMenu(menu);"#,
            ),
        ],
    ),
    (
        FILE_TRAY,
        &[
            (
                "countItem.label =\n                (count > 0 ? `${count}` : 'No') +\n                    ' agent' +\n                    (count === 1 ? '' : 's') +\n                    ' running';",
                r#"countItem.label = count > 0 ? `${count} 个智能体运行中` : '没有智能体在运行';"#,
            ),
            (
                r#"contextMenu = electron_1.Menu.buildFromTemplate(actions);"#,
                "const translatedActions = actions.map(action => {\n        if (action.label === 'No agents running') action.label = '没有智能体在运行';\n        if (action.label && action.label.startsWith('Open ')) action.label = '打开 Antigravity';\n        if (action.label === 'Quit') action.label = '退出';\n        return action;\n      });\n      contextMenu = electron_1.Menu.buildFromTemplate(translatedActions);",
            ),
        ],
    ),
    (
        FILE_WIZARD_HTML,
        &[
            (
                r#"<title>Welcome to Antigravity</title>"#,
                r#"<title>欢迎使用 Antigravity</title>"#,
            ),
            (r#">Setting up…<"#, r#">正在准备…<"#),
            (
                r#"<h1>Welcome to the new Antigravity!</h1>"#,
                r#"<h1>欢迎使用全新 Antigravity</h1>"#,
            ),
            (
                r#"<p>Antigravity has been redesigned to put agents first with new capabilities. If you'd still like a code editor, you can download it as a separate app named <b>Antigravity IDE</b>.</p>"#,
                r#"<p>Antigravity 已围绕智能体重构，带来了新的能力。如果你仍需要代码编辑器，可以另行下载名为 <b>Antigravity IDE</b> 的独立应用。</p>"#,
            ),
            (
                r#"<span>Download the Antigravity IDE</span>"#,
                r#"<span>下载 Antigravity IDE</span>"#,
            ),
            (
                r#"<button class="btn-primary" id="btn-skip">Explore the new Antigravity</button>"#,
                r#"<button class="btn-primary" id="btn-skip">开始使用全新 Antigravity</button>"#,
            ),
        ],
    ),
    (
        FILE_IPC,
        &[
            (r#"title: 'Open workspace',"#, r#"title: '打开工作区',"#),
            (r#"title: 'Open workspaces',"#, r#"title: '打开多个工作区',"#),
            (
                r#"electron_1.dialog.showErrorBox('Cannot open folder', t.error);"#,
                r#"electron_1.dialog.showErrorBox('无法打开文件夹', t.error);"#,
            ),
            (
                r#"message: 'Folder is on the Windows filesystem',"#,
                r#"message: '文件夹位于 Windows 文件系统中',"#,
            ),
            (
                r#"label: item.label ?? '',"#,
                r#"label: (typeof translateContextLabel === 'function' ? translateContextLabel(item.label) : item.label) ?? '',"#,
            ),
        ],
    ),
    (
        FILE_SPLASH,
        &[(
            r#"<div>Setting up WSL: ${escapeHtml(distro)}</div>"#,
            r#"<div>正在配置 WSL: ${escapeHtml(distro)}</div>"#,
        )],
    ),
    (
        FILE_WSL,
        &[
            (
                r#"warning: 'This folder is on the Windows filesystem. Accessing it from WSL (via /mnt) can be slow — for best performance keep projects inside the WSL filesystem.',"#,
                r#"warning: '此文件夹位于 Windows 文件系统。从 WSL 访问（通过 /mnt）可能较慢 — 为获得最佳性能，建议将项目保留在 WSL 文件系统中。',"#,
            ),
            (
                r#"error: `This folder belongs to the WSL distro "${unc[1]}", but this window is connected to "${distro}".`,"#,
                r#"error: `此文件夹属于 WSL 发行版 "${unc[1]}"，但当前窗口连接到 "${distro}"。`,"#,
            ),
            (
                r#"error: `This location cannot be opened in WSL: ${winPath}`"#,
                r#"error: `无法在 WSL 中打开此位置: ${winPath}`"#,
            ),
        ],
    ),
    (
        FILE_MAIN,
        &[
            (r#"title: 'Confirm Quit',"#, r#"title: '确认退出',"#),
            (
                r#"message: 'Are you sure you want to quit?',"#,
                r#"message: '您确定要退出吗？',"#,
            ),
            (
                r#"detail: 'There may be agents or background tasks running.',"#,
                r#"detail: '可能还有智能体或后台任务正在运行。',"#,
            ),
        ],
    ),
];

pub fn build_dom_engine() -> Result<String> {
    let ui: Value = serde_json::from_str(UI_JSON)?;
    let core: Value = serde_json::from_str(CORE_JSON)?;
    let data_code = format!("const dictionary = {};\nconst coreWords = {};", ui, core);
    Ok(DOM_TEMPLATE.replace(DOM_PLACEHOLDER, &data_code))
}

pub fn build_menu_hook() -> Result<String> {
    let menu: Value = serde_json::from_str(MENU_JSON)?;
    let ctx: Value = serde_json::from_str(CONTEXT_MENU_JSON)?;
    let menu_code = format!("const menuTranslationMap = {:#};", menu);
    let ctx_code = format!("const contextMenuTranslationMap = {:#};", ctx);
    Ok(MENU_TEMPLATE
        .replace(MENU_PLACEHOLDER, &menu_code)
        .replace(CONTEXT_MENU_PLACEHOLDER, &ctx_code))
}

#[derive(Debug, Default)]
pub struct InjectionReport {
    pub misses: Vec<String>,
    pub injected: usize,
    pub replaced: usize,
}

impl InjectionReport {
    pub fn is_empty(&self) -> bool {
        self.misses.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct StripReport {
    pub cleared: usize,
    pub restored: usize,
}

fn rel_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |acc, seg| acc.join(seg))
}

fn snippet(text: &str) -> String {
    text.chars().take(46).collect::<String>().replace('\n', " ")
}

fn inject_or_update(path: &Path, code: &str, marker: &str) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let existing = fs::read_to_string(path)?;
    let next = match existing.find(marker) {
        Some(at) => format!("{}\n\n{}", existing[..at].trim_end(), code),
        None => format!("{}\n\n{}", existing.trim_end(), code),
    };
    fs::write(path, next)?;
    Ok(true)
}

fn strip_injection(path: &Path) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let Some(at) = text.find(DOM_MARKER) else {
        return Ok(false);
    };
    let head = text[..at].trim_end();
    let next = if head.is_empty() {
        String::new()
    } else {
        format!("{}\n", head)
    };
    fs::write(path, next)?;
    Ok(true)
}

pub fn apply(extract_dir: &Path) -> Result<InjectionReport> {
    let dom_engine = build_dom_engine()?;
    let menu_hook = build_menu_hook()?;
    let mut report = InjectionReport::default();

    for (rel, pairs) in ANCHOR_TABLE {
        let path = rel_path(extract_dir, rel);
        if !path.is_file() {
            report.misses.push(format!("{} (文件缺失)", rel));
            continue;
        }
        let mut content = fs::read_to_string(&path)?;
        let mut hits = 0usize;
        for (original, localized) in *pairs {
            if !content.contains(original) {
                report.misses.push(format!("{} 锚点失配: {}", rel, snippet(original)));
                continue;
            }
            content = content.replace(original, localized);
            hits += 1;
        }
        if hits > 0 {
            fs::write(&path, content)?;
            report.replaced += hits;
        }
    }

    for (rel, payload, marker) in [
        (FILE_PRELOAD, &dom_engine, DOM_MARKER),
        (FILE_WIZARD_PRELOAD, &dom_engine, DOM_MARKER),
        (FILE_MENU, &menu_hook, MENU_MARKER),
        (FILE_IPC, &menu_hook, MENU_MARKER),
    ] {
        if inject_or_update(&rel_path(extract_dir, rel), payload, marker)? {
            report.injected += 1;
        } else {
            report.misses.push(format!("{} (文件缺失)", rel));
        }
    }

    Ok(report)
}

pub fn strip(extract_dir: &Path) -> Result<StripReport> {
    let mut report = StripReport::default();

    for rel in [FILE_PRELOAD, FILE_WIZARD_PRELOAD, FILE_MENU, FILE_IPC] {
        if strip_injection(&rel_path(extract_dir, rel))? {
            report.cleared += 1;
        }
    }

    for (rel, pairs) in ANCHOR_TABLE {
        let path = rel_path(extract_dir, rel);
        if !path.is_file() {
            continue;
        }
        let mut content = fs::read_to_string(&path)?;
        let mut hits = 0usize;
        for (original, localized) in *pairs {
            if !content.contains(localized) {
                continue;
            }
            content = content.replace(localized, original);
            hits += 1;
        }
        if hits > 0 {
            fs::write(&path, content)?;
            report.restored += hits;
        }
    }

    Ok(report)
}

pub fn extract_root(work_dir: &Path) -> PathBuf {
    work_dir.join(".tmp_extract")
}

#[cfg(test)]
pub fn payload_sizes() -> (usize, usize) {
    (
        build_dom_engine().map(|s| s.len()).unwrap_or(0),
        build_menu_hook().map(|s| s.len()).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("dist")).unwrap();
        base
    }

    fn seed_minimal(base: &Path) {
        let dist = base.join("dist");
        fs::write(dist.join("preload.js"), "console.log('x');\n").unwrap();
        fs::write(
            dist.join("menu.js"),
            "const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);\nelectron_1.Menu.setApplicationMenu(menu);\n",
        )
        .unwrap();
        fs::write(dist.join("main.js"), "title: 'Confirm Quit',\n").unwrap();
        fs::write(dist.join("tray.js"), "contextMenu = electron_1.Menu.buildFromTemplate(actions);\n").unwrap();
    }

    #[test]
    fn test_payloads_are_built() {
        let (dom, menu) = payload_sizes();
        assert!(dom > 100_000, "dom_engine 过小: {}", dom);
        assert!(menu > 1_000, "menu_hook 过小: {}", menu);

        let dom_code = build_dom_engine().unwrap();
        assert!(!dom_code.contains(DOM_PLACEHOLDER));
        assert!(dom_code.contains("const dictionary = {"));

        let menu_code = build_menu_hook().unwrap();
        assert!(!menu_code.contains(MENU_PLACEHOLDER));
        assert!(!menu_code.contains(CONTEXT_MENU_PLACEHOLDER));
        assert!(menu_code.contains("const menuTranslationMap = {"));
        assert!(menu_code.contains("const contextMenuTranslationMap = {"));
    }

    #[test]
    fn test_anchor_table_is_symmetric() {
        let pairs: usize = ANCHOR_TABLE.iter().map(|(_, p)| p.len()).sum();
        assert_eq!(pairs, 22, "锚点总数应与历史版本一致");
        for (rel, list) in ANCHOR_TABLE {
            assert!(rel.starts_with("dist/"), "路径应为 asar 内相对路径: {}", rel);
            let mut seen = std::collections::HashSet::new();
            for (original, localized) in *list {
                assert!(!original.is_empty() && !localized.is_empty(), "{} 存在空锚点", rel);
                assert_ne!(original, localized, "{} 锚点未发生替换", rel);
                assert!(
                    seen.insert(*localized),
                    "{} 译文重复，反向还原会有损: {}",
                    rel,
                    localized
                );
            }
        }
    }

    #[test]
    fn test_apply_reports_misses_for_missing_files() {
        let base = fixture("lander_localize_missing");
        let report = apply(&base).unwrap();
        assert!(!report.is_empty());
        assert!(report.misses.iter().any(|m| m.contains("preload.js")));
        assert_eq!(report.injected, 0);
        assert_eq!(report.replaced, 0);
    }

    #[test]
    fn test_apply_injects_and_replaces() {
        let base = fixture("lander_localize_ok");
        seed_minimal(&base);

        let report = apply(&base).unwrap();
        assert_eq!(report.injected, 2, "preload 与 menu 两处注入");
        assert_eq!(report.replaced, 4, "menu 2 处 + tray 1 处 + main 1 处");

        let dist = base.join("dist");
        let menu = fs::read_to_string(dist.join("menu.js")).unwrap();
        assert!(menu.contains("menuTranslationMap[submenuLabel]"));
        assert!(menu.contains("translateMenu"));
        assert!(menu.contains(MENU_MARKER));

        let main = fs::read_to_string(dist.join("main.js")).unwrap();
        assert!(main.contains("title: '确认退出',"));

        let preload = fs::read_to_string(dist.join("preload.js")).unwrap();
        assert!(preload.contains(DOM_MARKER));

        assert!(
            !report.misses.iter().any(|m| m.contains(FILE_MENU)),
            "主菜单不应失配: {:?}",
            report.misses
        );
    }

    #[test]
    fn test_strip_removes_injected_payload() {
        let base = fixture("lander_localize_strip_payload");
        seed_minimal(&base);
        let dist = base.join("dist");
        apply(&base).unwrap();

        let report = strip(&base).unwrap();
        assert_eq!(report.cleared, 2);

        for name in ["preload.js", "menu.js"] {
            let text = fs::read_to_string(dist.join(name)).unwrap();
            assert!(!text.contains(DOM_MARKER), "{} 仍残留注入标记", name);
            assert!(!text.contains("menuTranslationMap = {"), "{} 仍残留词典", name);
        }
    }

    #[test]
    fn test_strip_restores_anchor_text() {
        let base = fixture("lander_localize_strip_anchor");
        seed_minimal(&base);
        let dist = base.join("dist");
        apply(&base).unwrap();
        assert!(fs::read_to_string(dist.join("main.js")).unwrap().contains("确认退出"));

        strip(&base).unwrap();

        let main = fs::read_to_string(dist.join("main.js")).unwrap();
        assert!(main.contains("title: 'Confirm Quit',"), "退出确认文案未还原: {}", main);
        let tray = fs::read_to_string(dist.join("tray.js")).unwrap();
        assert!(
            tray.contains("electron_1.Menu.buildFromTemplate(actions);"),
            "托盘模板未还原: {}",
            tray
        );
    }

    #[test]
    fn test_apply_strip_roundtrip_is_lossless() {
        let base = fixture("lander_localize_roundtrip");
        seed_minimal(&base);
        let dist = base.join("dist");
        let before: Vec<String> = ["preload.js", "menu.js", "main.js", "tray.js"]
            .iter()
            .map(|n| fs::read_to_string(dist.join(n)).unwrap())
            .collect();

        apply(&base).unwrap();
        strip(&base).unwrap();

        let after: Vec<String> = ["preload.js", "menu.js", "main.js", "tray.js"]
            .iter()
            .map(|n| fs::read_to_string(dist.join(n)).unwrap())
            .collect();
        assert_eq!(before, after, "apply 后 strip 应回到原始文本");
    }

    #[test]
    fn test_inject_or_update_is_idempotent() {
        let base = fixture("lander_localize_idem");
        let target = base.join("dist").join("preload.js");
        fs::write(&target, b"first line\n").unwrap();

        let payload = format!("{} test payload", DOM_MARKER);
        inject_or_update(&target, &payload, DOM_MARKER).unwrap();
        let once = fs::read_to_string(&target).unwrap();
        inject_or_update(&target, &payload, DOM_MARKER).unwrap();
        let twice = fs::read_to_string(&target).unwrap();

        assert_eq!(once, twice);
        assert_eq!(twice.matches("test payload").count(), 1);
    }
}

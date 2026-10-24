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

fn apply_anchor(path: &Path, target: &str, replacement: &str) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let content = fs::read_to_string(path)?;
    if !content.contains(target) {
        return Ok(false);
    }
    fs::write(path, content.replace(target, replacement))?;
    Ok(true)
}

fn apply_anchor_all(path: &Path, pairs: &[(&str, &str)]) -> Result<Option<(usize, usize)>> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut content = fs::read_to_string(path)?;
    let mut hits = 0usize;
    for (target, replacement) in pairs {
        if !content.contains(target) {
            continue;
        }
        content = content.replace(target, replacement);
        hits += 1;
    }
    if hits > 0 {
        fs::write(path, content)?;
    }
    Ok(Some((hits, pairs.len())))
}

pub fn apply(extract_dir: &Path) -> Result<InjectionReport> {
    let dom_engine = build_dom_engine()?;
    let menu_hook = build_menu_hook()?;
    let mut report = InjectionReport::default();

    let preload = extract_dir.join("dist").join("preload.js");
    if inject_or_update(&preload, &dom_engine, DOM_MARKER)? {
        report.injected += 1;
    } else {
        report.misses.push("渲染层 preload.js (缺失)".to_string());
    }

    let wizard_preload = extract_dir
        .join("dist")
        .join("ideInstall")
        .join("wizardPreload.js");
    if inject_or_update(&wizard_preload, &dom_engine, DOM_MARKER)? {
        report.injected += 1;
    } else {
        report.misses.push("向导 preload (缺失)".to_string());
    }

    let menu_path = extract_dir.join("dist").join("menu.js");
    if menu_path.is_file() {
        if inject_or_update(&menu_path, &menu_hook, MENU_MARKER)? {
            report.injected += 1;
        }
        if apply_anchor(
            &menu_path,
            r#"const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);"#,
            r#"const submenuItem = appMenu.items.find((item) => item.label === submenuLabel || (typeof menuTranslationMap !== "undefined" && item.label === menuTranslationMap[submenuLabel]));"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("主菜单 submenuItem 查找".to_string());
        }
        if apply_anchor(
            &menu_path,
            r#"electron_1.Menu.setApplicationMenu(menu);"#,
            r#"if (typeof translateMenu === "function") { menu.items.forEach(translateMenu); } electron_1.Menu.setApplicationMenu(menu);"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("主菜单 setApplicationMenu".to_string());
        }
    } else {
        report.misses.push("主菜单 menu.js (文件缺失)".to_string());
    }

    let tray_path = extract_dir.join("dist").join("tray.js");
    if tray_path.is_file() {
        if apply_anchor(
            &tray_path,
            r#"countItem.label =
                (count > 0 ? `${count}` : 'No') +
                    ' agent' +
                    (count === 1 ? '' : 's') +
                    ' running';"#,
            r#"countItem.label = count > 0 ? `${count} 个智能体运行中` : '没有智能体在运行';"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("托盘计数文案".to_string());
        }
        if apply_anchor(
            &tray_path,
            r#"contextMenu = electron_1.Menu.buildFromTemplate(actions);"#,
            r#"const translatedActions = actions.map(action => {
        if (action.label === 'No agents running') action.label = '没有智能体在运行';
        if (action.label && action.label.startsWith('Open ')) action.label = '打开 Antigravity';
        if (action.label === 'Quit') action.label = '退出';
        return action;
      });
      contextMenu = electron_1.Menu.buildFromTemplate(translatedActions);"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("托盘菜单模板".to_string());
        }
    } else {
        report.misses.push("托盘 tray.js (文件缺失)".to_string());
    }

    let wizard_html = extract_dir
        .join("dist")
        .join("ideInstall")
        .join("wizardHtml.js");
    if wizard_html.is_file() {
        let pairs: &[(&str, &str)] = &[
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
        ];
        match apply_anchor_all(&wizard_html, pairs)? {
            Some((hits, total)) => {
                report.replaced += hits;
                if hits != total {
                    report
                        .misses
                        .push(format!("首启向导文案 ({}/{})", hits, total));
                }
            }
            None => report.misses.push("首启向导文案 (文件缺失)".to_string()),
        }
    }

    let ipc_path = extract_dir.join("dist").join("ipcHandlers.js");
    if ipc_path.is_file() {
        let pairs: &[(&str, &str)] = &[
            (r#"title: 'Open workspace',"#, r#"title: '打开工作区',"#),
            (r#"title: 'Open workspaces',"#, r#"title: '打开工作区',"#),
            (
                r#"electron_1.dialog.showErrorBox('Cannot open folder', t.error);"#,
                r#"electron_1.dialog.showErrorBox('无法打开文件夹', t.error);"#,
            ),
            (
                r#"message: 'Folder is on the Windows filesystem',"#,
                r#"message: '文件夹位于 Windows 文件系统中',"#,
            ),
        ];
        if let Some((hits, total)) = apply_anchor_all(&ipc_path, pairs)? {
            report.replaced += hits;
            if hits != total {
                report
                    .misses
                    .push(format!("文件选择对话框 ({}/{})", hits, total));
            }
        }
        if inject_or_update(&ipc_path, &menu_hook, MENU_MARKER)? {
            report.injected += 1;
        }
        if apply_anchor(
            &ipc_path,
            r#"label: item.label ?? '',"#,
            r#"label: (typeof translateContextLabel === 'function' ? translateContextLabel(item.label) : item.label) ?? '',"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("右键菜单标签替换".to_string());
        }
    } else {
        report.misses.push("右键菜单 ipcHandlers.js (文件缺失)".to_string());
    }

    let splash_path = extract_dir.join("dist").join("provisionSplash.js");
    if splash_path.is_file() {
        if apply_anchor(
            &splash_path,
            r#"<div>Setting up WSL: ${escapeHtml(distro)}</div>"#,
            r#"<div>正在配置 WSL: ${escapeHtml(distro)}</div>"#,
        )? {
            report.replaced += 1;
        } else {
            report.misses.push("WSL 配置提示".to_string());
        }
    }

    let wsl_path = extract_dir.join("dist").join("wsl.js");
    if wsl_path.is_file() {
        let pairs: &[(&str, &str)] = &[
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
        ];
        if let Some((hits, total)) = apply_anchor_all(&wsl_path, pairs)? {
            report.replaced += hits;
            if hits != total {
                report.misses.push(format!("WSL 提示文案 ({}/{})", hits, total));
            }
        }
    }

    let main_path = extract_dir.join("dist").join("main.js");
    if main_path.is_file() {
        let pairs: &[(&str, &str)] = &[
            (r#"title: 'Confirm Quit',"#, r#"title: '确认退出',"#),
            (
                r#"message: 'Are you sure you want to quit?',"#,
                r#"message: '您确定要退出吗？',"#,
            ),
            (
                r#"detail: 'There may be agents or background tasks running.',"#,
                r#"detail: '可能还有智能体或后台任务正在运行。',"#,
            ),
        ];
        if let Some((hits, total)) = apply_anchor_all(&main_path, pairs)? {
            report.replaced += hits;
            if hits != total {
                report
                    .misses
                    .push(format!("退出确认弹窗 ({}/{})", hits, total));
            }
        }
    } else {
        report.misses.push("主进程 main.js (文件缺失)".to_string());
    }

    Ok(report)
}

#[cfg(test)]
pub fn payload_sizes() -> (usize, usize) {
    (
        build_dom_engine().map(|s| s.len()).unwrap_or(0),
        build_menu_hook().map(|s| s.len()).unwrap_or(0),
    )
}

pub fn extract_root(work_dir: &Path) -> PathBuf {
    work_dir.join(".tmp_extract")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_apply_reports_misses_for_missing_files() {
        let base = std::env::temp_dir().join("lander_localize_missing");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let report = apply(&base).unwrap();
        assert!(!report.is_empty());
        assert!(report.misses.iter().any(|m| m.contains("preload.js")));
        assert_eq!(report.injected, 0);
        assert_eq!(report.replaced, 0);
    }

    #[test]
    fn test_apply_injects_and_replaces() {
        let base = std::env::temp_dir().join("lander_localize_ok");
        let _ = fs::remove_dir_all(&base);
        let dist = base.join("dist");
        fs::create_dir_all(&dist).unwrap();

        fs::write(dist.join("preload.js"), b"console.log('x');\n").unwrap();
        fs::write(
            dist.join("menu.js"),
            b"const submenuItem = appMenu.items.find((item) => item.label === submenuLabel);\nelectron_1.Menu.setApplicationMenu(menu);\n",
        )
        .unwrap();
        fs::write(dist.join("main.js"), b"title: 'Confirm Quit',\n").unwrap();

        let report = apply(&base).unwrap();
        assert!(report.injected >= 1, "应有整段注入");
        assert!(report.replaced >= 3, "应有锚点替换: {}", report.replaced);

        let menu = fs::read_to_string(dist.join("menu.js")).unwrap();
        assert!(menu.contains("menuTranslationMap[submenuLabel]"));
        assert!(menu.contains("translateMenu"));
        assert!(menu.contains(MENU_MARKER));

        let main = fs::read_to_string(dist.join("main.js")).unwrap();
        assert!(main.contains("title: '确认退出',"));

        let preload = fs::read_to_string(dist.join("preload.js")).unwrap();
        assert!(preload.contains(DOM_MARKER));

        assert!(
            !report.misses.iter().any(|m| m.contains("主菜单")),
            "主菜单不应失配: {:?}",
            report.misses
        );
    }

    #[test]
    fn test_inject_or_update_is_idempotent() {
        let base = std::env::temp_dir().join("lander_localize_idem");
        let _ = fs::remove_dir_all(&base);
        let dist = base.join("dist");
        fs::create_dir_all(&dist).unwrap();
        let target = dist.join("preload.js");
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

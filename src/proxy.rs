#![allow(dead_code)]

use anyhow::Result;
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

pub const VERSION_DLL: &[u8] = include_bytes!("../assets/proxy/version.dll");
pub const CONFIG_JSON: &str = include_str!("../assets/proxy/config.json");
pub const DEFAULT_PROXY_PORT: u16 = 7890;

fn is_our_dll(data: &[u8]) -> bool {
    data == VERSION_DLL
}

fn is_injected(dir: &std::path::Path) -> bool {
    let dll = dir.join("version.dll");
    let cfg = dir.join("config.json");
    let dll_ok = dll_is_ours(&dll) == Some(true);
    let cfg_ok = config_is_ours(&cfg) == Some(true);
    dll_ok && cfg_ok
}

fn dll_is_ours(path: &std::path::Path) -> Option<bool> {
    let data = fs::read(path).ok()?;
    Some(is_our_dll(&data))
}

fn config_is_ours(path: &std::path::Path) -> Option<bool> {
    let text = fs::read_to_string(path).ok()?;
    Some(text == CONFIG_JSON)
}

fn backup_path(path: &std::path::Path) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".bak");
    Some(path.parent()?.join(name))
}

fn backup_original(path: &std::path::Path) {
    let Some(bak) = backup_path(path) else {
        return;
    };
    if bak.exists() {
        return;
    }
    let _ = fs::copy(path, &bak);
}

fn put_back_or_drop(target: &std::path::Path) {
    if let Some(bak) = backup_path(target).filter(|b| b.exists()) {
        let _ = fs::remove_file(target);
        if fs::rename(&bak, target).is_ok() {
            return;
        }
        if fs::copy(&bak, target).is_ok() {
            let _ = fs::remove_file(&bak);
            return;
        }
    }
    let _ = fs::remove_file(target);
}

pub enum RemovalOutcome {
    Empty,
    Done,
    Foreign,
}

pub struct RemovalSummary {
    pub clean: bool,
    pub foreign: bool,
}

impl RemovalSummary {
    pub fn status_text(&self) -> String {
        if self.clean {
            "已移除代理注入".to_string()
        } else if self.foreign {
            "version.dll 不是本工具放入，未改动".to_string()
        } else {
            "当前没有代理注入文件".to_string()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProxyStatus {
    pub app_installed: bool,
    pub app_deployed: bool,
    pub port_online: bool,
}

impl ProxyStatus {
    pub fn is_any_deployed(&self) -> bool {
        self.app_deployed
    }

    pub fn is_fully_deployed(&self) -> bool {
        self.app_installed && self.app_deployed
    }

    pub fn pending_targets(&self) -> Vec<&'static str> {
        let mut pending = Vec::new();
        if self.app_installed && !self.app_deployed {
            pending.push("桌面版");
        }
        pending
    }

    pub fn deployed_targets(&self) -> Vec<&'static str> {
        let mut done = Vec::new();
        if self.app_deployed {
            done.push("桌面版");
        }
        done
    }

    pub fn status_text(&self) -> String {
        if !self.app_installed {
            return "未安装".to_string();
        }
        if self.app_deployed {
            return "已注入 (version.dll)".to_string();
        }
        "未注入".to_string()
    }
}

pub struct ProxyManager;

impl ProxyManager {
    pub fn app_dir() -> Option<PathBuf> {
        crate::launcher::LauncherConfig::detect_app_dir().filter(|p| p.exists())
    }

    pub fn is_port_online(port: u16) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
    }

    pub fn check_status() -> ProxyStatus {
        let app_dir = Self::app_dir();
        let app_installed = app_dir.is_some();
        let app_deployed = app_dir
            .as_ref()
            .map(|d| is_injected(d))
            .unwrap_or(false);

        let port_online = Self::is_port_online(DEFAULT_PROXY_PORT);

        ProxyStatus {
            app_installed,
            app_deployed,
            port_online,
        }
    }

    fn deploy_to_dir(dir: &std::path::Path) -> Result<()> {
        let dll_path = dir.join("version.dll");
        let cfg_path = dir.join("config.json");

        if dll_is_ours(&dll_path) == Some(false) {
            backup_original(&dll_path);
        }
        if config_is_ours(&cfg_path) == Some(false) {
            backup_original(&cfg_path);
        }

        fs::write(&dll_path, VERSION_DLL)?;
        fs::write(&cfg_path, CONFIG_JSON)?;
        Ok(())
    }

    fn remove_from_dir(dir: &std::path::Path) -> RemovalOutcome {
        let dll_path = dir.join("version.dll");
        let cfg_path = dir.join("config.json");

        let dll_state = dll_is_ours(&dll_path);
        let cfg_state = config_is_ours(&cfg_path);

        if dll_state.is_none() && cfg_state.is_none() {
            return RemovalOutcome::Empty;
        }
        if dll_state == Some(false) {
            return RemovalOutcome::Foreign;
        }

        if dll_state == Some(true) {
            put_back_or_drop(&dll_path);
        }
        if cfg_state == Some(true) {
            put_back_or_drop(&cfg_path);
        }

        RemovalOutcome::Done
    }

    pub fn is_antigravity_running() -> bool {
        crate::launcher::LauncherConfig::is_antigravity_running()
    }

    pub fn deploy_all() -> Result<String> {
        if Self::is_antigravity_running() {
            anyhow::bail!("Antigravity 运行中");
        }

        let Some(dir) = Self::app_dir() else {
            anyhow::bail!("未找到 Antigravity 安装目录");
        };

        Self::deploy_to_dir(&dir)?;
        Ok("已开启系统代理".to_string())
    }

    pub fn remove_all() -> Result<RemovalSummary> {
        if Self::is_antigravity_running() {
            anyhow::bail!("Antigravity 运行中");
        }

        let Some(dir) = Self::app_dir() else {
            anyhow::bail!("未找到 Antigravity 安装目录");
        };

        let mut summary = RemovalSummary {
            clean: false,
            foreign: false,
        };

        match Self::remove_from_dir(&dir) {
            RemovalOutcome::Empty => {}
            RemovalOutcome::Done => summary.clean = true,
            RemovalOutcome::Foreign => summary.foreign = true,
        }

        Ok(summary)
    }
}

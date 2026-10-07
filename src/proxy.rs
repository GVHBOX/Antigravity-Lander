#![allow(dead_code)]

use anyhow::Result;
use std::env;
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

pub const VERSION_DLL: &[u8] = include_bytes!("../assets/proxy/version.dll");
pub const CONFIG_JSON: &str = include_str!("../assets/proxy/config.json");
pub const DEFAULT_PROXY_PORT: u16 = 7890;

#[derive(Debug, Clone, Default)]
pub struct ProxyStatus {
    pub app_installed: bool,
    pub app_deployed: bool,
    pub ide_installed: bool,
    pub ide_deployed: bool,
    pub port_online: bool,
}


impl ProxyStatus {
    pub fn is_any_deployed(&self) -> bool {
        self.app_deployed || self.ide_deployed
    }

    pub fn is_fully_deployed(&self) -> bool {
        let app_ok = !self.app_installed || self.app_deployed;
        let ide_ok = !self.ide_installed || self.ide_deployed;
        app_ok && ide_ok && (self.app_deployed || self.ide_deployed)
    }
}


pub struct ProxyManager;

impl ProxyManager {
    pub fn app_dir() -> Option<PathBuf> {
        let local_app_data = env::var("LOCALAPPDATA").ok()?;
        let dir = PathBuf::from(local_app_data).join("Programs").join("antigravity");
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
    }

    pub fn ide_dir() -> Option<PathBuf> {
        let local_app_data = env::var("LOCALAPPDATA").ok()?;
        let dir = PathBuf::from(local_app_data).join("Programs").join("Antigravity IDE");
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
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
            .map(|d| d.join("version.dll").exists())
            .unwrap_or(false);

        let ide_dir = Self::ide_dir();
        let ide_installed = ide_dir.is_some();
        let ide_deployed = ide_dir
            .as_ref()
            .map(|d| d.join("version.dll").exists())
            .unwrap_or(false);

        let port_online = Self::is_port_online(DEFAULT_PROXY_PORT);

        ProxyStatus {
            app_installed,
            app_deployed,
            ide_installed,
            ide_deployed,
            port_online,
        }

    }

    fn deploy_to_dir(dir: &std::path::Path) -> Result<()> {
        let dll_path = dir.join("version.dll");
        let bak_path = dir.join("version.dll.bak");
        if dll_path.exists() && !bak_path.exists() {
            let _ = fs::copy(&dll_path, &bak_path);
        }
        fs::write(&dll_path, VERSION_DLL)?;
        fs::write(dir.join("config.json"), CONFIG_JSON)?;
        Ok(())
    }

    fn remove_from_dir(dir: &std::path::Path) -> bool {
        let dll_path = dir.join("version.dll");
        let bak_path = dir.join("version.dll.bak");
        if bak_path.exists() {
            let _ = fs::rename(&bak_path, &dll_path);
        } else {
            let _ = fs::remove_file(&dll_path);
        }
        let _ = fs::remove_file(dir.join("config.json"));
        !dll_path.exists() || bak_path.exists()
    }

    pub fn deploy_all() -> Result<String> {
        let mut deployed = Vec::new();

        if let Some(dir) = Self::app_dir() {
            Self::deploy_to_dir(&dir)?;
            deployed.push("桌面版");
        }

        if let Some(dir) = Self::ide_dir() {
            Self::deploy_to_dir(&dir)?;
            deployed.push("IDE版");
        }

        if deployed.is_empty() {
            anyhow::bail!("未找到 Antigravity 或 Antigravity IDE 安装目录！");
        }

        Ok(format!("代理已部署至: {}", deployed.join(" & ")))
    }

    pub fn remove_all() -> Result<String> {
        let mut removed = Vec::new();

        if let Some(dir) = Self::app_dir() {
            if Self::remove_from_dir(&dir) {
                removed.push("桌面版");
            }
        }

        if let Some(dir) = Self::ide_dir() {
            if Self::remove_from_dir(&dir) {
                removed.push("IDE版");
            }
        }

        Ok(format!("已从 {} 移除代理注入文件", removed.join(" & ")))
    }
}


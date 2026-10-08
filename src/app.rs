use crate::i18n::Language;
use crate::launcher::LauncherConfig;
use crate::monitor::TelemetryMonitor;
use chrono::{DateTime, Local};
use std::collections::VecDeque;

pub struct LogEntry {
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

pub struct App {
    pub language: Language,
    pub config: LauncherConfig,
    pub monitor: TelemetryMonitor,
    pub logs: VecDeque<LogEntry>,
    pub toast_message: String,
    pub toast_is_alert: bool,
    pub sub_known_pid: Option<u32>,
    pub launcher_rss_mb: f64,
    pub should_quit: bool,
    sys: sysinfo::System,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = LauncherConfig::new()?;
        let monitor = TelemetryMonitor::new();
        let mut app = Self {
            language: Language::Zh,
            config,
            monitor,
            logs: VecDeque::with_capacity(300),
            toast_message: String::new(),
            toast_is_alert: false,
            sub_known_pid: None,
            launcher_rss_mb: 5.2,
            should_quit: false,
            sys: sysinfo::System::new(),
        };

        app.init_logs();
        app.refresh();
        Ok(app)
    }

    pub fn add_log(&mut self, level: &'static str, message: impl Into<String>) {
        let now: DateTime<Local> = Local::now();
        let timestamp = now.format("%H:%M:%S").to_string();

        if self.logs.len() >= 250 {
            self.logs.pop_front();
        }

        self.logs.push_back(LogEntry {
            timestamp,
            level,
            message: message.into(),
        });
    }

    pub fn set_toast(&mut self, message: impl Into<String>, is_alert: bool) {
        self.toast_message = message.into();
        self.toast_is_alert = is_alert;
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
        let msg = if self.language == Language::Zh {
            "日志缓冲区已清空。"
        } else {
            "Log buffer cleared."
        };
        self.add_log("SYSTEM", msg);
    }

    pub fn toggle_language(&mut self) {
        self.language.toggle();
        let msg = if self.language == Language::Zh {
            "界面语言已切换为：中文"
        } else {
            "UI Language switched to: English"
        };
        self.add_log("CONFIG", msg);
    }

    pub fn refresh(&mut self) {
        self.monitor.refresh_single(self.sub_known_pid);

        if let Some(pid) = self.monitor.sub.electron_pid {
            self.sub_known_pid = Some(pid);
        } else if !self.monitor.sub.is_running {
            self.sub_known_pid = None;
        }

        let current_pid = std::process::id();
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(current_pid)]), true);
        if let Some(proc) = self.sys.process(sysinfo::Pid::from_u32(current_pid)) {
            self.launcher_rss_mb = (proc.memory() as f64) / (1024.0 * 1024.0);
        }
    }

    pub fn launch_or_bring_to_front(&mut self) {
        if let Some(pid) = self.sub_known_pid {
            let brought = self.config.bring_to_front(pid);
            if brought {
                let msg = if self.language == Language::Zh {
                    format!("已唤醒分身窗口 (PID: {})。", pid)
                } else {
                    format!("Brought sub-instance window (PID: {}) to foreground.", pid)
                };
                self.add_log("WIN32", &msg);
                self.set_toast(msg, false);
            } else {
                let msg = if self.language == Language::Zh {
                    format!("分身正在独立运行中 (PID: {})。", pid)
                } else {
                    format!("Sub-instance is running (PID: {}).", pid)
                };
                self.add_log("MONITOR", &msg);
                self.set_toast(msg, false);
            }
        } else {
            match self.config.spawn_detached() {
                Ok(new_pid) => {
                    self.sub_known_pid = Some(new_pid);
                    let msg = if self.language == Language::Zh {
                        format!("分身已在后台启动 (PID: {})。", new_pid)
                    } else {
                        format!("Detached sub-instance spawned (PID: {}). Immune to terminal exit.", new_pid)
                    };
                    self.add_log("SPAWN", &msg);
                    self.set_toast(msg, false);
                }
                Err(err) => {
                    let err_msg = format!("启动失败: {}", err);
                    self.add_log("SPAWN", &err_msg);
                    self.set_toast(err_msg, true);
                }
            }
        }
    }

    pub fn stop_sub_instance(&mut self) {
        if let Some(pid) = self.sub_known_pid {
            let _ = self.config.kill_sub_instance_by_pid(pid);
            self.sub_known_pid = None;
            self.monitor.sub.is_running = false;
            let msg = if self.language == Language::Zh {
                format!("已终止分身进程 (PID: {})。", pid)
            } else {
                format!("Terminated sub-instance (PID: {}). Host instance untouched.", pid)
            };
            self.add_log("KILL", &msg);
            self.set_toast(msg, true);
        } else {
            let msg = if self.language == Language::Zh {
                "分身当前未在运行。"
            } else {
                "Sub-instance is not currently running."
            };
            self.set_toast(msg, true);
        }
    }

    pub fn restart_engine(&mut self) {
        if let Some(ls_pid) = self.monitor.sub.ls_pid {
            let _ = self.config.kill_sub_instance_by_pid(ls_pid);
            let msg = if self.language == Language::Zh {
                format!("正在重启语言服务 (PID: {})...", ls_pid)
            } else {
                format!("Recycling language server (PID: {})... Electron will restart it.", ls_pid)
            };
            self.add_log("LANG_SVR", &msg);
            self.set_toast(msg, false);
        } else if self.sub_known_pid.is_some() {
            let msg = if self.language == Language::Zh {
                "已向分身发送刷新请求。"
            } else {
                "Refresh signal sent to sub-instance."
            };
            self.add_log("LANG_SVR", msg);
            self.set_toast(msg, false);
        } else {
            let msg = if self.language == Language::Zh {
                "分身未在运行，无法重启语言服务。"
            } else {
                "Sub-instance is not running, no language server to recycle."
            };
            self.set_toast(msg, true);
        }
    }

    pub fn clear_token(&mut self) {
        match self.config.clear_token() {
            Ok(cleared) => {
                let msg = if cleared {
                    if self.language == Language::Zh {
                        "已退出分身登录状态。"
                    } else {
                        "Account logged out. Next login will prompt Google account selection."
                    }
                } else {
                    if self.language == Language::Zh {
                        "分身未处于登录状态。"
                    } else {
                        "Sub-instance is already in unauthenticated state."
                    }
                };
                self.add_log("AUTH", msg);
                self.set_toast(msg, false);
            }
            Err(e) => {
                let err_msg = format!("退出账号失败: {}", e);
                self.add_log("AUTH", &err_msg);
                self.set_toast(err_msg, true);
            }
        }
    }

    pub fn open_sandbox_dir(&mut self) {
        match self.config.open_sandbox_in_explorer() {
            Ok(_) => {
                let msg = if self.language == Language::Zh {
                    "已在 Windows 资源管理器中打开数据目录。"
                } else {
                    "Opened data directory in Windows Explorer."
                };
                self.add_log("DATA", msg);
                self.set_toast(msg, false);
            }
            Err(e) => {
                let err_msg = format!("打开目录失败: {}", e);
                self.set_toast(err_msg, true);
            }
        }
    }

    pub fn toggle_or_deploy_proxy(&mut self) {
        let status = crate::proxy::ProxyManager::check_status();
        if status.is_fully_deployed() {
            match crate::proxy::ProxyManager::remove_all() {
                Ok(msg) => {
                    self.add_log("PROXY", &msg);
                    self.set_toast(msg, false);
                }
                Err(e) => {
                    let err_msg = format!("移除代理失败: {}", e);
                    self.add_log("PROXY", &err_msg);
                    self.set_toast(err_msg, true);
                }
            }
        } else {
            match crate::proxy::ProxyManager::deploy_all() {
                Ok(msg) => {
                    self.add_log("PROXY", &msg);
                    self.set_toast(msg, false);
                    let now = crate::proxy::ProxyManager::check_status();
                    let pending = now.pending_targets();
                    if !pending.is_empty() {
                        self.add_log("WARN", &format!("代理未写入: {}", pending.join(" & ")));
                    }
                    if !now.port_online {
                        self.add_log("WARN", "本地 7890 端口未监听。");
                    }
                }
                Err(e) => {
                    let err_msg = format!("部署代理失败: {}", e);
                    self.add_log("PROXY", &err_msg);
                    self.set_toast(err_msg, true);
                }
            }
        }
    }

    fn init_logs(&mut self) {
        if self.language == Language::Zh {
            self.add_log("SYSTEM", "管理程序就绪。");
            self.add_log("DATA", "数据目录: data/instance_2");
        } else {
            self.add_log("SYSTEM", "Manager ready.");
            self.add_log("DATA", "Data directory: data/instance_2");
        }
    }
}

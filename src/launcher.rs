use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

const WIN32_DETACHED_FLAGS: u32 = 0x01000208;
static ASAR_PATCH_CACHE: Mutex<Option<(SystemTime, u64, bool, String)>> = Mutex::new(None);

fn run_taskkill_pid(pid: u32) -> Result<()> {
    if pid == 0 {
        return Ok(());
    }
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(WIN32_DETACHED_FLAGS)
        .output()
        .with_context(|| format!("Failed to execute taskkill for PID {}", pid))?;

    if output.status.success() || output.status.code() == Some(128) {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{} {}", stderr, stdout);

    if combined.contains("not found")
        || combined.contains("找不到")
        || combined.contains("没有找到")
    {
        return Ok(());
    }

    anyhow::bail!("终止进程失败 (PID: {}): {}", pid, stderr.trim())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScaleSettings {
    pub host: u32,
    pub sub1: u32,
    pub sub2: u32,
    pub sub3: u32,
    pub sub4: u32,
}

impl Default for ScaleSettings {
    fn default() -> Self {
        Self {
            host: 158,
            sub1: 144,
            sub2: 100,
            sub3: 100,
            sub4: 100,
        }
    }
}

impl ScaleSettings {
    pub fn load_or_detect() -> Self {
        let path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Some(settings) = Self::parse_json(&content) {
                return settings;
            }
        let settings = Self::default();
        let _ = settings.save();
        settings
    }

    pub fn get_sub_scale(&self, slot: usize) -> u32 {
        match slot {
            1 => self.sub1,
            2 => self.sub2,
            3 => self.sub3,
            4 => self.sub4,
            _ => 100,
        }
    }

    pub fn set_sub_scale(&mut self, slot: usize, val: u32) {
        let val = val.clamp(80, 200);
        match slot {
            1 => self.sub1 = val,
            2 => self.sub2 = val,
            3 => self.sub3 = val,
            4 => self.sub4 = val,
            _ => {}
        }
        let _ = self.save();
    }

    pub fn set_host_scale(&mut self, val: u32) {
        self.host = val.clamp(80, 200);
        let _ = self.save();
    }

    pub fn adjust_host(&mut self, delta: i32) {
        let cur = self.host as i32 + delta;
        self.set_host_scale(cur.clamp(80, 200) as u32);
    }

    pub fn adjust_sub(&mut self, slot: usize, delta: i32) {
        let cur = self.get_sub_scale(slot) as i32 + delta;
        self.set_sub_scale(slot, cur.clamp(80, 200) as u32);
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn get_config_path() -> PathBuf {
        if let Ok(base) = LauncherConfig::resolve_base_dir() {
            base.join("data").join("scale_settings.json")
        } else {
            PathBuf::from("data").join("scale_settings.json")
        }
    }

    pub fn parse_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}

const CACHE_TARGETS: [&str; 5] = [
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
];

pub struct SandboxCache {
    pub slot: usize,
    pub bytes: u64,
}

pub struct CleanResult {
    pub freed_bytes: u64,
    pub has_locked_files: bool,
}

pub struct LauncherConfig {
    pub slot: usize,
    pub executable_path: PathBuf,
    pub sandbox_root: PathBuf,
    pub home_dir: PathBuf,
    pub roaming_dir: PathBuf,
    pub token_path: PathBuf,
}

impl LauncherConfig {
    pub fn new() -> Result<Self> {
        Self::for_slot(1)
    }

    pub fn ensure_all_sandboxes() -> Result<()> {
        for slot in 1..=4 {
            let conf = Self::for_slot(slot)?;
            conf.ensure_sandbox_dirs()?;
        }
        Ok(())
    }

    pub fn for_slot(slot: usize) -> Result<Self> {
        let base_dir = Self::resolve_base_dir()?;
        let instance_name = format!("instance_{}", slot);
        let sandbox_root = base_dir.join("data").join(instance_name);
        let home_dir = sandbox_root.join("home");
        let roaming_dir = sandbox_root.join("AppData").join("Roaming");
        let token_path = home_dir.join(".gemini").join("jetski-standalone-oauth-token");

        let executable_path = Self::detect_executable_path();

        Ok(Self {
            slot,
            executable_path,
            sandbox_root,
            home_dir,
            roaming_dir,
            token_path,
        })
    }

    pub fn resolve_base_dir() -> Result<PathBuf> {
        if let Ok(exe_path) = env::current_exe()
            && let Some(exe_dir) = exe_path.parent() {
                let mut check_dir = Some(exe_dir);
                while let Some(dir) = check_dir {
                    if dir.file_name().map(|n| n == "target").unwrap_or(false)
                        && let Some(project_root) = dir.parent()
                            && (project_root.join("data").exists()
                                || project_root.join("Cargo.toml").exists())
                            {
                                return Ok(project_root.to_path_buf());
                            }
                    check_dir = dir.parent();
                }
                if exe_dir.join("data").exists()
                    || exe_dir.join("Antigravity-Lander.exe").exists()
                {
                    return Ok(exe_dir.to_path_buf());
                }
                if let Some(parent) = exe_dir.parent() {
                    if parent.join("data").exists() {
                        return Ok(parent.to_path_buf());
                    }
                    if let Some(grandparent) = parent.parent()
                        && grandparent.join("data").exists() {
                            return Ok(grandparent.to_path_buf());
                        }
                }
                return Ok(exe_dir.to_path_buf());
            }
        env::current_dir().context("Failed to get current directory")
    }

    pub fn detect_app_dir() -> Option<PathBuf> {
        let mut sys = sysinfo::System::new();
        let refresh_kind = sysinfo::ProcessRefreshKind::nothing()
            .with_exe(sysinfo::UpdateKind::OnlyIfNotSet);
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, refresh_kind);
        let self_pid = std::process::id();
        for (pid, proc) in sys.processes() {
            if pid.as_u32() == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            if (name == "antigravity.exe" || name == "antigravity")
                && !name.contains("multi")
                && !name.contains("lander")
                && !name.contains("cockpit")
                && let Some(exe_path) = proc.exe()
                    && let Some(dir) = exe_path.parent()
                        && (dir.join("Antigravity.exe").exists() || dir.join("antigravity.exe").exists()) {
                            return Some(dir.to_path_buf());
                        }
        }

        if let Ok(local) = env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Programs").join("antigravity");
            if p.join("Antigravity.exe").exists() || p.join("antigravity.exe").exists() {
                return Some(p);
            }
        }

        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Ok(pf) = env::var(var) {
                let p1 = PathBuf::from(&pf).join("antigravity");
                if p1.join("Antigravity.exe").exists() || p1.join("antigravity.exe").exists() {
                    return Some(p1);
                }
                let p2 = PathBuf::from(&pf).join("Google").join("antigravity");
                if p2.join("Antigravity.exe").exists() || p2.join("antigravity.exe").exists() {
                    return Some(p2);
                }
            }
        }

        for d in ['C', 'D', 'E', 'F'] {
            let p = PathBuf::from(format!("{}:\\Programs\\antigravity", d));
            if p.join("Antigravity.exe").exists() || p.join("antigravity.exe").exists() {
                return Some(p);
            }
        }

        if let Ok(local) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(local).join("Programs").join("antigravity"));
        }

        None
    }

    pub fn detect_executable_path() -> PathBuf {
        if let Some(dir) = Self::detect_app_dir() {
            let exe1 = dir.join("Antigravity.exe");
            if exe1.exists() {
                return exe1;
            }
            let exe2 = dir.join("antigravity.exe");
            if exe2.exists() {
                return exe2;
            }
            exe1
        } else {
            PathBuf::from("Antigravity.exe")
        }
    }

    pub fn find_node_executable() -> Option<PathBuf> {
        if let Ok(output) = Command::new("node")
            .arg("-v")
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output()
            && output.status.success() {
                return Some(PathBuf::from("node"));
            }
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Ok(pf) = env::var(var) {
                let p = PathBuf::from(pf).join("nodejs").join("node.exe");
                if p.exists() {
                    return Some(p);
                }
            }
        }
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Programs").join("node").join("node.exe");
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(appdata) = env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("nvm").join("nodejs").join("node.exe");
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

pub fn resolve_host_user_dir() -> Option<PathBuf> {
    if let (Ok(drive), Ok(path)) = (env::var("HOMEDRIVE"), env::var("HOMEPATH")) {
        let p = PathBuf::from(format!("{}{}", drive, path));
        if p.exists() {
            return Some(p);
        }
    }
    if let (Ok(drive), Ok(user)) = (env::var("SystemDrive"), env::var("USERNAME")) {
        let p = PathBuf::from(drive).join("Users").join(user);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(user) = env::var("USERPROFILE") {
        let p = PathBuf::from(user);
        let s = p.to_string_lossy().replace('/', "\\");
        if !s.contains("\\data\\instance_") && p.exists() {
            return Some(p);
        }
    }
    None
}

    pub fn ensure_sandbox_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.home_dir)
            .with_context(|| format!("Failed to create home dir: {:?}", self.home_dir))?;
        fs::create_dir_all(self.roaming_dir.join("Antigravity"))
            .with_context(|| format!("Failed to create roaming dir: {:?}", self.roaming_dir))?;

        let sandbox_config = self.home_dir.join(".gemini").join("config");
        let _ = fs::create_dir_all(&sandbox_config);

        if let Some(host_user) = Self::resolve_host_user_dir() {
            let host_locallow = host_user.join("AppData").join("LocalLow");
            let sub_locallow = self.home_dir.join("AppData").join("LocalLow");
            ensure_junction(&sub_locallow, &host_locallow);

            let shell_folders = [
                "Desktop",
                "Downloads",
                "Documents",
                "Pictures",
                "Videos",
                "Music",
                "Links",
                "Favorites",
            ];
            for folder in shell_folders {
                ensure_junction(&self.home_dir.join(folder), &host_user.join(folder));
            }

            let host_ms_roaming = host_user.join("AppData").join("Roaming").join("Microsoft");
            ensure_junction(&self.roaming_dir.join("Microsoft"), &host_ms_roaming);
            ensure_junction(&self.home_dir.join("AppData").join("Roaming").join("Microsoft"), &host_ms_roaming);

            let host_ms_local = host_user.join("AppData").join("Local").join("Microsoft");
            ensure_junction(&self.home_dir.join("AppData").join("Local").join("Microsoft"), &host_ms_local);

            let shared_dirs = ["skills", "rules", "plugins", "sidecars"];
            for dir_name in shared_dirs {
                let host_dir = host_user.join(".gemini").join("config").join(dir_name);
                let sub_dir = sandbox_config.join(dir_name);
                ensure_junction(&sub_dir, &host_dir);
            }

            let host_mcp = host_user.join(".gemini").join("config").join("mcp_config.json");
            let sub_mcp = sandbox_config.join("mcp_config.json");
            if host_mcp.exists() {
                let should_copy = if sub_mcp.exists() {
                    fs::metadata(&sub_mcp).map(|m| m.len() == 0).unwrap_or(false)
                } else {
                    true
                };
                if should_copy {
                    let _ = fs::copy(&host_mcp, &sub_mcp);
                }
            }
        }

        Ok(())
    }

    pub fn is_token_present(&self) -> bool {
        self.token_path.exists()
    }

    pub fn host_token_path() -> Option<PathBuf> {
        Self::resolve_host_user_dir().map(|h| h.join(".gemini").join("jetski-standalone-oauth-token"))
    }

    pub fn is_host_token_present() -> bool {
        Self::host_token_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn clear_host_token() -> Result<bool> {
        if let Some(token_path) = Self::host_token_path()
            && token_path.exists() {
                fs::remove_file(&token_path)
                    .with_context(|| format!("Failed to delete host token at {:?}", token_path))?;
                return Ok(true);
            }
        Ok(false)
    }

    pub fn clear_token(&self) -> Result<bool> {
        if self.token_path.exists() {
            fs::remove_file(&self.token_path)
                .with_context(|| format!("Failed to delete token at {:?}", self.token_path))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn open_sandbox_in_explorer(&self) -> Result<()> {
        self.ensure_sandbox_dirs()?;
        Command::new("explorer.exe")
            .arg(&self.sandbox_root)
            .spawn()
            .context("Failed to launch explorer.exe")?;
        Ok(())
    }

    pub fn spawn_detached(&self) -> Result<u32> {
        self.ensure_sandbox_dirs()?;

        if !self.executable_path.exists() {
            anyhow::bail!(
                "未找到 Antigravity.exe: {:?}",
                self.executable_path
            );
        }

        let user_data_dir = self.roaming_dir.join("Antigravity");
        let user_data_arg = format!("--user-data-dir={}", user_data_dir.display());

        let mut cmd = Command::new(&self.executable_path);
        cmd.arg(&user_data_arg);

        let scale = ScaleSettings::load_or_detect().get_sub_scale(self.slot);
        let scale_factor = (scale as f32) / 100.0;
        cmd.arg(format!("--force-device-scale-factor={:.2}", scale_factor));

        cmd.env("USERPROFILE", &self.home_dir);
        cmd.env("HOME", &self.home_dir);
        cmd.env("APPDATA", &self.roaming_dir);

        let ssh_port = 50000 + self.slot as u32;
        cmd.env("SSH_CONNECTION", format!("127.0.0.1 {} 127.0.0.1 22", ssh_port));

        if let Ok(real_local) = env::var("LOCALAPPDATA") {
            cmd.env("LOCALAPPDATA", real_local);
        }

        cmd.creation_flags(WIN32_DETACHED_FLAGS);

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn {:?}", self.executable_path))?;

        Ok(child.id())
    }

    pub fn kill_slot_processes(&self) -> Result<()> {
        let mut sys = sysinfo::System::new();
        let refresh_kind = sysinfo::ProcessRefreshKind::nothing()
            .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, refresh_kind);

        let self_pid = std::process::id();
        let mut targets: Vec<u32> = Vec::new();
        let slot_idx = self.slot.saturating_sub(1);

        for (pid, proc) in sys.processes() {
            let pid_u32 = pid.as_u32();
            if pid_u32 == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            let is_proc = (name == "antigravity.exe" || name == "antigravity" || name == "language_server.exe" || name == "language_server")
                && !name.contains("lander")
                && !name.contains("cockpit");
            if !is_proc {
                continue;
            }
            let cmd_line = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            if crate::monitor::matches_instance_slot(&cmd_line, slot_idx) {
                targets.push(pid_u32);
            }
        }

        for pid in targets {
            let _ = run_taskkill_pid(pid);
        }

        Ok(())
    }

    pub fn kill_sub_instance_by_pid(&self, pid: u32) -> Result<()> {
        let res = run_taskkill_pid(pid);
        let _ = self.kill_slot_processes();
        res
    }

    pub fn kill_managed_instances(known_pids: &[u32]) -> Result<usize> {
        let mut sys = sysinfo::System::new();
        let refresh_kind = sysinfo::ProcessRefreshKind::nothing()
            .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, refresh_kind);

        let self_pid = std::process::id();
        let mut targets: Vec<u32> = Vec::new();

        for (pid, proc) in sys.processes() {
            let pid_u32 = pid.as_u32();
            if pid_u32 == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            let is_proc = (name == "antigravity.exe" || name == "antigravity" || name == "language_server.exe" || name == "language_server")
                && !name.contains("lander")
                && !name.contains("cockpit");
            if !is_proc {
                continue;
            }
            let cmd_line = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            if (0..4).any(|slot| crate::monitor::matches_instance_slot(&cmd_line, slot)) {
                targets.push(pid_u32);
            }
        }

        for pid in known_pids {
            if *pid != 0 && !targets.contains(pid) {
                targets.push(*pid);
            }
        }

        let mut killed = 0usize;
        for pid in targets {
            if run_taskkill_pid(pid).is_ok() {
                killed += 1;
            }
        }

        Ok(killed)
    }


    pub fn kill_all_antigravity() -> Result<()> {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "Antigravity.exe", "/T"])
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output();
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "language_server.exe", "/T"])
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output();
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if !Self::is_antigravity_running() {
                break;
            }
        }
        Ok(())
    }

    pub fn spawn_host_native() -> Result<u32> {
        let executable_path = Self::detect_executable_path();

        if !executable_path.exists() {
            anyhow::bail!(
                "未找到 Antigravity.exe: {:?}",
                executable_path
            );
        }

        let mut cmd = Command::new(&executable_path);

        let scale = ScaleSettings::load_or_detect().host;
        let scale_factor = (scale as f32) / 100.0;
        cmd.arg(format!("--force-device-scale-factor={:.2}", scale_factor));

        cmd.creation_flags(WIN32_DETACHED_FLAGS);

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn host {:?}", executable_path))?;

        Ok(child.id())
    }

    pub fn bring_to_front(&self, target_pid: u32) -> bool {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
            use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                BringWindowToTop, EnumWindows, GetForegroundWindow,
                GetWindowRect, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
                IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
            };

            struct SearchState {
                target_pid: u32,
                best_hwnd: HWND,
                found_main: bool,
            }

            unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> i32 {
                unsafe {
                    let state = &mut *(lparam as *mut SearchState);
                    let mut process_id: u32 = 0;
                    GetWindowThreadProcessId(hwnd, &mut process_id);

                    if process_id == state.target_pid && (IsWindowVisible(hwnd) != 0 || IsIconic(hwnd) != 0) {
                        let mut rect: RECT = std::mem::zeroed();
                        GetWindowRect(hwnd, &mut rect);
                        let w = rect.right - rect.left;
                        let h = rect.bottom - rect.top;
                        let title_len = GetWindowTextLengthW(hwnd);
                        let is_iconic = IsIconic(hwnd) != 0;

                        if ((w > 100 && h > 100) || is_iconic) && title_len > 0 {
                            state.best_hwnd = hwnd;
                            state.found_main = true;
                            return 0;
                        } else if state.best_hwnd.is_null() {
                            state.best_hwnd = hwnd;
                        }
                    }
                    1
                }
            }

            let mut state = SearchState {
                target_pid,
                best_hwnd: std::ptr::null_mut(),
                found_main: false,
            };

            EnumWindows(Some(enum_windows_callback), &mut state as *mut _ as LPARAM);

            if !state.best_hwnd.is_null() {
                let fg_hwnd = GetForegroundWindow();
                let fg_thread = if !fg_hwnd.is_null() {
                    GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut())
                } else {
                    0
                };
                let cur_thread = GetCurrentThreadId();

                if IsIconic(state.best_hwnd) != 0 {
                    ShowWindow(state.best_hwnd, SW_RESTORE);
                } else {
                    ShowWindow(state.best_hwnd, SW_SHOW);
                }

                if fg_thread != 0 && fg_thread != cur_thread {
                    AttachThreadInput(cur_thread, fg_thread, 1);
                    SetForegroundWindow(state.best_hwnd);
                    BringWindowToTop(state.best_hwnd);
                    AttachThreadInput(cur_thread, fg_thread, 0);
                } else {
                    SetForegroundWindow(state.best_hwnd);
                    BringWindowToTop(state.best_hwnd);
                }
                return true;
            }
        }
        false
    }

    pub fn open_app_dir_in_explorer() -> Result<()> {
        let path = Self::detect_app_dir().unwrap_or_else(|| {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .context("Failed to launch explorer.exe")?;
        Ok(())
    }

    pub fn scan_sandbox_cache(running_slots: &[bool; 4]) -> Vec<SandboxCache> {
        (1..=4)
            .filter(|slot| !running_slots[slot - 1])
            .filter_map(|slot| {
                let conf = Self::for_slot(slot).ok()?;
                let bytes = Self::cache_dirs(&conf)
                    .iter()
                    .map(|dir| dir_size(dir))
                    .sum::<u64>();
                (bytes > 0).then_some(SandboxCache { slot, bytes })
            })
            .collect()
    }

    pub fn clean_sandbox_cache(running_slots: &[bool; 4]) -> Result<CleanResult> {
        let before_caches = Self::scan_sandbox_cache(running_slots);
        let before_total: u64 = before_caches.iter().map(|c| c.bytes).sum();
        let mut has_locked_files = false;

        for slot in 1..=4 {
            if running_slots[slot - 1] {
                continue;
            }
            let conf = Self::for_slot(slot)?;
            for dir in Self::cache_dirs(&conf) {
                if !dir.exists() {
                    continue;
                }
                let is_trash = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".lander_trash"))
                    .unwrap_or(false);

                if is_trash {
                    if fs::remove_dir_all(&dir).is_err() {
                        has_locked_files = true;
                    }
                    continue;
                }

                let Some(parent) = dir.parent() else {
                    continue;
                };
                let trash_name = format!(
                    "{}.lander_trash",
                    dir.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                );
                let trash = parent.join(trash_name);
                let _ = fs::remove_dir_all(&trash);

                if fs::rename(&dir, &trash).is_err() {
                    has_locked_files = true;
                    continue;
                }
                if fs::remove_dir_all(&trash).is_err() {
                    has_locked_files = true;
                }
            }
        }

        let after_caches = Self::scan_sandbox_cache(running_slots);
        let after_total: u64 = after_caches.iter().map(|c| c.bytes).sum();
        let freed_bytes = before_total.saturating_sub(after_total);

        if after_total > 0 {
            has_locked_files = true;
        }

        Ok(CleanResult {
            freed_bytes,
            has_locked_files,
        })
    }

    pub fn is_antigravity_running() -> bool {
        let mut sys = sysinfo::System::new();
        let refresh_kind = sysinfo::ProcessRefreshKind::nothing();
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, refresh_kind);
        let self_pid = std::process::id();
        for (pid, proc) in sys.processes() {
            if pid.as_u32() == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            if (name == "antigravity.exe" || name == "antigravity")
                && !name.contains("multi")
                && !name.contains("lander")
                && !name.contains("cockpit")
            {
                return true;
            }
        }
        false
    }

    fn cache_dirs(&self) -> Vec<PathBuf> {
        let user_data = self.roaming_dir.join("Antigravity");
        let mut trash_dirs = Vec::new();
        let mut real_dirs = Vec::new();
        for name in CACHE_TARGETS {
            let trash_name = format!("{name}.lander_trash");
            let trash_dir = user_data.join(&trash_name);
            let trash_inside = trash_dir.parent().map(|p| p == user_data).unwrap_or(false);
            let trash_plain_dir = fs::symlink_metadata(&trash_dir)
                .map(|m| m.is_dir())
                .unwrap_or(false);
            let trash_stays_inside = is_within(&trash_dir, &user_data);
            if trash_inside && trash_plain_dir && trash_stays_inside {
                trash_dirs.push(trash_dir);
            }

            let dir = user_data.join(name);
            let inside = dir.parent().map(|p| p == user_data).unwrap_or(false);
            let allowed = dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| CACHE_TARGETS.contains(&n))
                .unwrap_or(false);
            let plain_dir = fs::symlink_metadata(&dir)
                .map(|m| m.is_dir())
                .unwrap_or(false);
            let stays_inside = is_within(&dir, &user_data);
            if inside && allowed && plain_dir && stays_inside {
                real_dirs.push(dir);
            }
        }
        trash_dirs.extend(real_dirs);
        trash_dirs
    }

    pub fn check_ag2_patch_status() -> (bool, String) {
        if let Some(app_dir) = Self::detect_app_dir() {
            let exe = app_dir.join("Antigravity.exe");
            let asar = app_dir.join("resources").join("app.asar");

            if let Ok(meta) = fs::metadata(&asar) {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let len = meta.len();
                if let Ok(guard) = ASAR_PATCH_CACHE.lock()
                    && let Some((cached_mtime, cached_len, cached_patched, ref cached_ver)) = *guard
                        && cached_mtime == mtime && cached_len == len {
                            return (cached_patched, cached_ver.clone());
                        }

                let is_patched = asar_has_injection_markers(&asar);
                let ver = if exe.exists() {
                    get_pe_version(&exe).map(|v| format!("v{}", v)).unwrap_or_else(|| "已安装".to_string())
                } else {
                    "未安装".to_string()
                };

                if let Ok(mut guard) = ASAR_PATCH_CACHE.lock() {
                    *guard = Some((mtime, len, is_patched, ver.clone()));
                }
                return (is_patched, ver);
            }

            let ver = if exe.exists() {
                get_pe_version(&exe).map(|v| format!("v{}", v)).unwrap_or_else(|| "已安装".to_string())
            } else {
                "未安装".to_string()
            };
            return (false, ver);
        }
        (false, "未安装".to_string())
    }

    pub fn toggle_ag2_patch() -> Result<bool> {
        if Self::is_antigravity_running() {
            anyhow::bail!("Antigravity 运行中");
        }

        let Some(app_dir) = Self::detect_app_dir() else {
            anyhow::bail!("未找到 Antigravity 安装目录");
        };
        let res_dir = app_dir.join("resources");
        let asar = res_dir.join("app.asar");
        let exe = app_dir.join("Antigravity.exe");

        if !asar.exists() {
            anyhow::bail!("未找到 {:?}", asar);
        }

        let installed = get_pe_version(&exe);

        let result = if asar_has_injection_markers(&asar) {
            Self::restore_ag2_from_same_version_backup(&asar, &res_dir, &installed)?;
            Ok(false)
        } else {
            Self::install_ag2_from_cached_or_engine(&asar, &res_dir, &installed)?;
            Ok(true)
        };
        if let Ok(mut guard) = ASAR_PATCH_CACHE.lock() {
            *guard = None;
        }
        result
    }

    fn restore_ag2_from_same_version_backup(
        asar: &std::path::Path,
        res_dir: &std::path::Path,
        installed: &Option<String>,
    ) -> Result<()> {
        let bak = res_dir.join("app.asar.bak");
        let backup_matches_host = match (installed, asar_version(&bak)) {
            (Some(host), Some(bak)) => host == &bak,
            _ => false,
        };
        if backup_matches_host {
            fs::copy(&bak, asar)?;
            Ok(())
        } else {
            Self::run_patch_engine("--restore")
        }
    }

    fn install_ag2_from_cached_or_engine(
        asar: &std::path::Path,
        res_dir: &std::path::Path,
        installed: &Option<String>,
    ) -> Result<()> {
        let cached = Self::cached_ag2_patch_path()
            .filter(|p| installed.as_ref().map(|v| cached_patch_matches_host_version(p, v)).unwrap_or(false));
        match cached {
            Some(patch) => {
                let bak = res_dir.join("app.asar.bak");
                fs::copy(asar, &bak)?;
                fs::copy(&patch, asar)?;
                Ok(())
            }
            None => Self::run_patch_engine("--install"),
        }
    }

    fn cached_ag2_patch_path() -> Option<PathBuf> {
        let base = Self::resolve_base_dir().unwrap_or_else(|_| PathBuf::from("."));
        let patch = base.join("data").join("patch").join("app.asar");
        if patch.exists() {
            Some(patch)
        } else {
            None
        }
    }

    fn run_patch_engine(arg: &str) -> Result<()> {
        let base = Self::resolve_base_dir().unwrap_or_else(|_| PathBuf::from("."));
        let builder_dir = base.join("tools").join("patch_builder");
        let engine_script = builder_dir.join("tools").join("engine.js");
        if !engine_script.exists() {
            anyhow::bail!("未找到汉化引擎 tools/patch_builder/tools/engine.js");
        }
        let Some(node_exe) = Self::find_node_executable() else {
            anyhow::bail!("未检测到 Node.js 环境，无法构建汉化包");
        };
        let out = Command::new(&node_exe)
            .arg(&engine_script)
            .arg(arg)
            .current_dir(&builder_dir)
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output()
            .with_context(|| format!("运行 Node.js 失败: {:?}", node_exe))?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = if err.trim().is_empty() { stdout } else { err };
            anyhow::bail!("{}", detail.trim())
        }
    }
}

fn is_replaceable_shell_dir(dir: &std::path::Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name == "desktop.ini" || name == "antigravity.lnk" {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
            && is_replaceable_shell_dir(&entry.path()) {
                continue;
            }
        return false;
    }
    true
}

fn ensure_junction(link: &std::path::Path, target: &std::path::Path) {
    if !target.exists() {
        return;
    }
    use std::os::windows::fs::MetadataExt;
    let is_junc = link
        .symlink_metadata()
        .map(|m| m.file_attributes() & 0x400 != 0)
        .unwrap_or(false);
    if is_junc {
        return;
    }
    if link.exists() {
        if is_replaceable_shell_dir(link) {
            let _ = fs::remove_dir_all(link);
        } else {
            return;
        }
    }
    if let Some(parent) = link.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = Command::new("cmd")
        .args(["/c", "mklink", "/J", &link.to_string_lossy(), &target.to_string_lossy()])
        .creation_flags(WIN32_DETACHED_FLAGS)
        .output();
}

const LOCALIZED_MARKERS: [&str; 3] = [
    "Antigravity 2.0 Web UI DOM Localization Engine",
    "Antigravity 2.0 Native Menu",
    "menuTranslationMap",
];

fn asar_has_injection_markers(asar: &std::path::Path) -> bool {
    let Ok(data) = fs::read(asar) else {
        return false;
    };
    LOCALIZED_MARKERS
        .iter()
        .any(|m| find_bytes(&data, m.as_bytes()).is_some())
}

fn asar_version(asar: &std::path::Path) -> Option<String> {
    let data = fs::read(asar).ok()?;
    if data.len() < 16 {
        return None;
    }
    let json_len = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
    if data.len() < 16 + json_len {
        return None;
    }
    let header = std::str::from_utf8(&data[16..16 + json_len]).ok()?;
    let pkg = asar_entry_text(&data, header, "package.json")?;
    json_string_field(&pkg, "version")
}

fn root_files_direct_child(header: &str, name: &str) -> Option<String> {
    let key = format!("\"{}\"", name);
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0usize;
    let mut string_start = 0usize;
    let bytes: Vec<char> = header.chars().collect();

    while index < bytes.len() {
        let ch = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                let literal: String = bytes[string_start..=index].iter().collect();
                let after: String = bytes[index + 1..].iter().take(40).collect();
                if depth == 2 && literal == key && after.trim_start().starts_with(':') {
                    let rest: String = bytes[index + 1..].iter().collect();
                    let body = rest.find('{')? + 1;
                    return Some(rest[body..].to_string());
                }
            }
        } else {
            match ch {
                '"' => {
                    in_string = true;
                    string_start = index;
                }
                '{' | '[' => depth += 1,
                '}' | ']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn json_field_usize(text: &str, key: &str) -> Option<usize> {
    let at = text.find(key)? + key.len();
    let rest = &text[at..];
    let colon = rest.find(':')? + 1;
    let tail = rest[colon..].trim_start().trim_start_matches('"');
    let end = tail.find(|c: char| !c.is_ascii_digit())?;
    tail[..end].parse().ok()
}

fn asar_entry_text(data: &[u8], header: &str, name: &str) -> Option<String> {
    let data_base = 8 + u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let node = root_files_direct_child(header, name)?;
    let offset = json_field_usize(&node, "\"offset\"")?;
    let size = json_field_usize(&node, "\"size\"")?;
    let start = data_base + offset;
    if data.len() < start + size {
        return None;
    }
    String::from_utf8(data[start..start + size].to_vec()).ok()
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let at = text.find(&needle)? + needle.len();
    let rest = &text[at..];
    let colon = rest.find(':')? + 1;
    let tail = rest[colon..].trim_start();
    let open = tail.find('"')? + 1;
    let close = tail[open..].find('"')? + open;
    Some(tail[open..close].to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn dir_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += dir_size(&entry.path());
        } else if file_type.is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

fn is_within(target: &std::path::Path, root: &std::path::Path) -> bool {
    match (fs::canonicalize(target), fs::canonicalize(root)) {
        (Ok(target_real), Ok(root_real)) => target_real.starts_with(root_real),
        _ => false,
    }
}

fn cached_patch_matches_host_version(patch: &std::path::Path, installed: &str) -> bool {
    match asar_version(patch) {
        Some(v) => v == installed,
        None => false,
    }
}

#[cfg(windows)]
fn get_pe_version(path: &std::path::Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

    #[link(name = "version")]
    unsafe extern "system" {
        fn GetFileVersionInfoSizeW(lptstrFilename: *const u16, lpdwHandle: *mut u32) -> u32;
        fn GetFileVersionInfoW(lptstrFilename: *const u16, dwHandle: u32, dwLen: u32, lpData: *mut u8) -> i32;
        fn VerQueryValueW(pBlock: *const u8, lpSubBlock: *const u16, lplpBuffer: *mut *const u8, puLen: *mut u32) -> i32;
    }

    unsafe {
        let size = GetFileVersionInfoSizeW(wide_path.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide_path.as_ptr(), 0, size, buf.as_mut_ptr()) == 0 {
            return None;
        }

        let sub_block: Vec<u16> = "\\\0".encode_utf16().collect();
        let mut lp_buffer: *const u8 = std::ptr::null();
        let mut len = 0u32;
        if VerQueryValueW(buf.as_ptr(), sub_block.as_ptr(), &mut lp_buffer, &mut len) == 0 || lp_buffer.is_null() || len < 52 {
            return None;
        }

        let slice = std::slice::from_raw_parts(lp_buffer, len as usize);
        let ms = u32::from_ne_bytes(slice[8..12].try_into().ok()?);
        let ls = u32::from_ne_bytes(slice[12..16].try_into().ok()?);
        let major = ms >> 16;
        let minor = ms & 0xffff;
        let patch = ls >> 16;
        Some(format!("{}.{}.{}", major, minor, patch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asar_patch_cache_hit() {
        let (p1, v1) = LauncherConfig::check_ag2_patch_status();
        let guard = ASAR_PATCH_CACHE.lock().unwrap();
        if let Some((_mtime, _len, cp, ref cv)) = *guard {
            assert_eq!(p1, cp);
            assert_eq!(&v1, cv);
        }
        drop(guard);
        let (p2, v2) = LauncherConfig::check_ag2_patch_status();
        assert_eq!(p1, p2);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_cache_dirs_and_trash_cleanup() {
        let temp = std::env::temp_dir().join("lander_test_cache_dirs");
        let user_data = temp.join("Antigravity");
        let _ = fs::create_dir_all(&user_data);

        let cache_dir = user_data.join("Cache");
        let _ = fs::create_dir_all(&cache_dir);
        let _ = fs::write(cache_dir.join("data.bin"), b"12345");

        let trash_dir = user_data.join("GPUCache.lander_trash");
        let _ = fs::create_dir_all(&trash_dir);
        let _ = fs::write(trash_dir.join("trash.bin"), b"abcdef");

        let conf = LauncherConfig {
            slot: 1,
            executable_path: temp.join("antigravity.exe"),
            sandbox_root: temp.clone(),
            home_dir: temp.clone(),
            roaming_dir: temp.clone(),
            token_path: temp.join("token"),
        };

        let dirs = conf.cache_dirs();
        assert!(dirs.iter().any(|d| d.ends_with("GPUCache.lander_trash")));
        assert!(dirs.iter().any(|d| d.ends_with("Cache")));
        assert!(dirs[0].to_string_lossy().ends_with(".lander_trash"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_scale_settings_parse_json() {
        let single_line = r#"{"host":120,"sub1":110,"sub2":90,"sub3":80,"sub4":70}"#;
        let parsed = ScaleSettings::parse_json(single_line).unwrap();
        assert_eq!(parsed.host, 120);
        assert_eq!(parsed.sub1, 110);
        assert_eq!(parsed.sub2, 90);
        assert_eq!(parsed.sub3, 80);
        assert_eq!(parsed.sub4, 70);

        let multiline = "{\n  \"host\": 150,\n  \"sub1\": 130\n}";
        let parsed_partial = ScaleSettings::parse_json(multiline).unwrap();
        assert_eq!(parsed_partial.host, 150);
        assert_eq!(parsed_partial.sub1, 130);
        assert_eq!(parsed_partial.sub2, 100);
        assert_eq!(parsed_partial.sub3, 100);
        assert_eq!(parsed_partial.sub4, 100);

        let invalid = "not json at all";
        assert!(ScaleSettings::parse_json(invalid).is_none());

        let extra_fields = r#"{"host":140,"unknown_extra":"ignored","sub2":110}"#;
        let parsed_extra = ScaleSettings::parse_json(extra_fields).unwrap();
        assert_eq!(parsed_extra.host, 140);
        assert_eq!(parsed_extra.sub1, 144);
        assert_eq!(parsed_extra.sub2, 110);
    }
}

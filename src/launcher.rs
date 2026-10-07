use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const WIN32_DETACHED_FLAGS: u32 = 0x01000208;

#[derive(Debug, Clone, Copy)]
pub struct ScaleSettings {
    pub host: u32,
    pub sub1: u32,
    pub sub2: u32,
    pub sub3: u32,
    pub sub4: u32,
}

impl ScaleSettings {
    pub fn load_or_detect() -> Self {
        let path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(settings) = Self::parse_json(&content) {
                return settings;
            }
        }
        let settings = Self {
            host: 158,
            sub1: 144,
            sub2: 100,
            sub3: 100,
            sub4: 100,
        };
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
        let json = format!(
            "{{\"host\":{},\"sub1\":{},\"sub2\":{},\"sub3\":{},\"sub4\":{}}}",
            self.host, self.sub1, self.sub2, self.sub3, self.sub4
        );
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

    fn parse_json(s: &str) -> Option<Self> {
        let mut host = 158;
        let mut sub1 = 144;
        let mut sub2 = 100;
        let mut sub3 = 100;
        let mut sub4 = 100;
        for part in s.trim_matches(|c| c == '{' || c == '}').split(',') {
            let mut kv = part.split(':');
            let k = kv.next()?.trim().trim_matches('"');
            let v = kv.next()?.trim().parse::<u32>().ok()?;
            match k {
                "host" => host = v,
                "sub1" => sub1 = v,
                "sub2" => sub2 = v,
                "sub3" => sub3 = v,
                "sub4" => sub4 = v,
                _ => {}
            }
        }
        Some(Self {
            host,
            sub1,
            sub2,
            sub3,
            sub4,
        })
    }
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

    pub fn for_slot(slot: usize) -> Result<Self> {
        let base_dir = Self::resolve_base_dir()?;
        let instance_name = format!("instance_{}", slot);
        let sandbox_root = base_dir.join("data").join(instance_name);
        let home_dir = sandbox_root.join("home");
        let roaming_dir = sandbox_root.join("AppData").join("Roaming");
        let token_path = home_dir.join(".gemini").join("jetski-standalone-oauth-token");

        let local_app_data = env::var("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        let executable_path = PathBuf::from(local_app_data)
            .join("Programs")
            .join("antigravity")
            .join("Antigravity.exe");

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
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                if exe_dir.join("data").exists() || exe_dir.join("Multi-Antigravity.exe").exists() || exe_dir.join("multi-antigravity.exe").exists() {
                    return Ok(exe_dir.to_path_buf());
                }
                if let Some(parent) = exe_dir.parent() {
                    if parent.join("data").exists() {
                        return Ok(parent.to_path_buf());
                    }
                    if let Some(grandparent) = parent.parent() {
                        if grandparent.join("data").exists() {
                            return Ok(grandparent.to_path_buf());
                        }
                    }
                }
                return Ok(exe_dir.to_path_buf());
            }
        }
        env::current_dir().context("Failed to get current directory")
    }

    pub fn ensure_sandbox_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.home_dir)
            .with_context(|| format!("Failed to create home dir: {:?}", self.home_dir))?;
        fs::create_dir_all(self.roaming_dir.join("Antigravity"))
            .with_context(|| format!("Failed to create roaming dir: {:?}", self.roaming_dir))?;

        let sandbox_config = self.home_dir.join(".gemini").join("config");
        let _ = fs::create_dir_all(&sandbox_config);

        if let Ok(real_user) = env::var("USERPROFILE") {
            let host_user = PathBuf::from(&real_user);

            let host_locallow = host_user.join("AppData").join("LocalLow");
            let sub_appdata = self.home_dir.join("AppData");
            let sub_locallow = sub_appdata.join("LocalLow");
            if host_locallow.exists() {
                let is_junc = sub_locallow
                    .symlink_metadata()
                    .map(|m| {
                        use std::os::windows::fs::MetadataExt;
                        m.file_attributes() & 0x400 != 0
                    })
                    .unwrap_or(false);
                if !is_junc {
                    if sub_locallow.exists() {
                        let _ = fs::remove_dir_all(&sub_locallow);
                    }
                    if !sub_locallow.exists() {
                        let _ = fs::create_dir_all(&sub_appdata);
                        let _ = Command::new("cmd")
                            .args(["/c", "mklink", "/J", &sub_locallow.to_string_lossy(), &host_locallow.to_string_lossy()])
                            .creation_flags(WIN32_DETACHED_FLAGS)
                            .output();
                    }
                }
            }

            let shared_dirs = ["skills", "rules", "plugins", "sidecars"];
            for dir_name in shared_dirs {
                let host_dir = host_user.join(".gemini").join("config").join(dir_name);
                let sub_dir = sandbox_config.join(dir_name);
                if host_dir.exists() && !sub_dir.exists() {
                    let _ = Command::new("cmd")
                        .args(["/c", "mklink", "/J", &sub_dir.to_string_lossy(), &host_dir.to_string_lossy()])
                        .creation_flags(WIN32_DETACHED_FLAGS)
                        .output();
                }
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
                "Antigravity.exe not found at {:?}. Please verify installation.",
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

    pub fn kill_sub_instance_by_pid(&self, pid: u32) -> Result<()> {
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output()
            .with_context(|| format!("Failed to execute taskkill for PID {}", pid))?;

        if output.status.success() {
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

        anyhow::bail!(
            "终止进程失败 (PID: {}): {}",
            pid,
            stderr.trim()
        )
    }

    pub fn spawn_host_native() -> Result<u32> {
        let local_app_data = env::var("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        let executable_path = PathBuf::from(local_app_data)
            .join("Programs")
            .join("antigravity")
            .join("Antigravity.exe");

        if !executable_path.exists() {
            anyhow::bail!(
                "Antigravity.exe not found at {:?}. Please verify installation.",
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
                IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
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

                        if w > 100 && h > 100 && title_len > 0 {
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

                if fg_thread != 0 && fg_thread != cur_thread {
                    AttachThreadInput(cur_thread, fg_thread, 1);
                    ShowWindow(state.best_hwnd, SW_RESTORE);
                    SetForegroundWindow(state.best_hwnd);
                    BringWindowToTop(state.best_hwnd);
                    AttachThreadInput(cur_thread, fg_thread, 0);
                } else {
                    ShowWindow(state.best_hwnd, SW_RESTORE);
                    SetForegroundWindow(state.best_hwnd);
                    BringWindowToTop(state.best_hwnd);
                }
                return true;
            }
        }
        false
    }

    pub fn open_app_dir_in_explorer() -> Result<()> {
        let path = if let Ok(local) = env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Programs").join("antigravity");
            if p.exists() {
                p
            } else {
                env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            }
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .context("Failed to launch explorer.exe")?;
        Ok(())
    }

    pub fn clean_inactive_caches(running_slots: &[bool]) -> usize {
        let mut cleaned_bytes = 0usize;
        let base = PathBuf::from("data");
        for slot in 1..=4 {
            if slot - 1 < running_slots.len() && running_slots[slot - 1] {
                continue;
            }
            let inst_dir = base.join(format!("instance_{}", slot));
            let app_data = inst_dir.join("app_data").join("Antigravity");
            if !app_data.exists() {
                continue;
            }
            for target in ["Cache", "Code Cache", "GPUCache"] {
                let cache_dir = app_data.join(target);
                if cache_dir.exists() {
                    if let Ok(entries) = fs::read_dir(&cache_dir) {
                        for entry in entries.flatten() {
                            if let Ok(meta) = entry.metadata() {
                                cleaned_bytes += meta.len() as usize;
                            }
                        }
                    }
                    let _ = fs::remove_dir_all(&cache_dir);
                }
            }
        }
        cleaned_bytes
    }

    pub fn check_ag2_patch_status() -> (bool, String) {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let app_dir = PathBuf::from(&local).join("Programs").join("antigravity");
            let exe = app_dir.join("Antigravity.exe");
            let res_dir = app_dir.join("resources");
            let bak = res_dir.join("app.asar.bak");
            let is_patched = bak.exists();

            let ver = if exe.exists() {
                get_pe_version(&exe).map(|v| format!("v{}", v)).unwrap_or_else(|| "已安装".to_string())
            } else {
                "未安装".to_string()
            };
            return (is_patched, ver);
        }
        (false, "未安装".to_string())
    }

    pub fn toggle_ag2_patch() -> Result<bool> {
        let local = env::var("LOCALAPPDATA").context("LOCALAPPDATA not set")?;
        let res_dir = PathBuf::from(local)
            .join("Programs")
            .join("antigravity")
            .join("resources");
        let asar = res_dir.join("app.asar");
        let bak = res_dir.join("app.asar.bak");
        let zh_backup = res_dir.join("app.asar.zh");
        let local_patch = Self::resolve_base_dir()
            .map(|b| b.join("data").join("patch").join("app.asar"))
            .unwrap_or_else(|_| PathBuf::from("data").join("patch").join("app.asar"));

        if bak.exists() {
            if asar.exists() {
                let _ = fs::copy(&asar, &zh_backup);
                let _ = fs::remove_file(&asar);
            }
            fs::rename(&bak, &asar)?;
            Ok(false)
        } else if zh_backup.exists() {
            if asar.exists() {
                fs::copy(&asar, &bak)?;
            }
            fs::copy(&zh_backup, &asar)?;
            Ok(true)
        } else if local_patch.exists() {
            if asar.exists() {
                fs::copy(&asar, &bak)?;
            }
            fs::copy(&local_patch, &asar)?;
            Ok(true)
        } else {
            anyhow::bail!("未找到汉化包文件（请将汉化 app.asar 放入 data/patch 目录）")
        }
    }

    pub fn check_ide_patch_status() -> (bool, String) {
        let is_patched = if let Ok(local) = env::var("LOCALAPPDATA") {
            let jetski_script = PathBuf::from(&local)
                .join("Programs")
                .join("Antigravity IDE")
                .join("resources")
                .join("app")
                .join("out")
                .join("vs")
                .join("code")
                .join("electron-browser")
                .join("workbench")
                .join("jetskiAgent.js");

            let langpack_dir = env::var("USERPROFILE").map(|home| {
                PathBuf::from(home)
                    .join(".antigravity-ide")
                    .join("extensions")
                    .join("ms-ceintl.vscode-language-pack-zh-hans-1.106.0")
            });

            if jetski_script.exists() {
                if let Ok(meta) = jetski_script.metadata() {
                    meta.len() > 50000 || langpack_dir.map(|p| p.exists()).unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        let mut ver = None;
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let ide_dir = PathBuf::from(&local).join("Programs").join("Antigravity IDE");
            let exe = ide_dir.join("Antigravity IDE.exe");
            let product_json = ide_dir.join("resources").join("app").join("product.json");

            if product_json.exists() {
                if let Ok(content) = fs::read_to_string(&product_json) {
                    for line in content.lines() {
                        if line.contains("\"ideVersion\"") {
                            if let Some(v) = line.split(':').nth(1) {
                                let clean = v.trim().trim_matches(|c| c == '"' || c == ',' || c == ' ');
                                if !clean.is_empty() {
                                    ver = Some(format!("v{}", clean));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if ver.is_none() && exe.exists() {
                ver = get_pe_version(&exe).map(|v| format!("v{}", v));
            }
            if let Some(v) = ver {
                return (is_patched, v);
            } else if exe.exists() {
                return (is_patched, "已安装".to_string());
            }
        }
        (false, "未安装".to_string())
    }

    pub fn toggle_ide_patch() -> Result<bool> {
        let (cur_patched, _) = Self::check_ide_patch_status();
        let base = Self::resolve_base_dir().unwrap_or_else(|_| PathBuf::from("."));
        let builder_dir = base.join("tools").join("ide_patch_builder");
        let engine_script = builder_dir.join("tools").join("engine.js");

        if !engine_script.exists() {
            anyhow::bail!("未找到 IDE 汉化引擎 tools/ide_patch_builder/tools/engine.js");
        }

        let arg = if cur_patched { "--uninstall" } else { "--install" };
        let out = Command::new("node")
            .arg(&engine_script)
            .arg(arg)
            .current_dir(&builder_dir)
            .creation_flags(WIN32_DETACHED_FLAGS)
            .output()
            .context("Failed to run node engine.js")?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            if cur_patched {
                anyhow::bail!("还原失败: {}", err.trim());
            } else {
                anyhow::bail!("安装失败: {}", err.trim());
            }
        }
        Ok(!cur_patched)
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


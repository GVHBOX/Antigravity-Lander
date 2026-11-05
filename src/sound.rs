use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const MCI_ALIAS: &str = "lander_done";

const BUILTIN_WAV: &[u8] = include_bytes!("../assets/done.wav");

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SoundConfig {
    #[serde(default = "default_sound_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub sound_file: Option<String>,
}

fn default_sound_enabled() -> bool {
    true
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_file: None,
        }
    }
}

impl SoundConfig {
    fn resolve_base_dir() -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_dir) = exe_path.parent()
        {
            let mut check_dir = Some(exe_dir);
            while let Some(dir) = check_dir {
                if dir.file_name().map(|n| n == "target").unwrap_or(false)
                    && let Some(project_root) = dir.parent()
                    && (project_root.join("data").exists()
                        || project_root.join("Cargo.toml").exists())
                {
                    return project_root.to_path_buf();
                }
                check_dir = dir.parent();
            }
            if exe_dir.join("data").exists() || exe_dir.join("Antigravity-Lander.exe").exists() {
                return exe_dir.to_path_buf();
            }
        }
        PathBuf::from(".")
    }

    pub fn config_path() -> PathBuf {
        Self::resolve_base_dir().join("data").join("sound_settings.json")
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        Self::load_from_path(&path).unwrap_or_default()
    }

    pub fn load_from_path(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

pub fn play_done_or_builtin(cfg: &SoundConfig) -> Option<String> {
    if !cfg.enabled {
        return None;
    }
    if let Some(ref path_str) = cfg.sound_file {
        let path = Path::new(path_str);
        if path.exists() {
            play_done_sound(path);
            return Some(file_stem_of(path));
        }
    }
    play_builtin_sound();
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionTransition {
    Started {
        slot: u8,
    },
    Finished {
        slot: u8,
        duration_secs: f32,
        total_tokens: u64,
        avg_tps: f32,
    },
}

#[derive(Clone, Debug)]
pub struct SlotRuntimeRecord {
    pub is_running: bool,
    pub start_time: Option<std::time::Instant>,
    pub initial_tokens: u64,
    pub last_tokens: u64,
    pub last_tps: f32,
}

impl Default for SlotRuntimeRecord {
    fn default() -> Self {
        Self {
            is_running: false,
            start_time: None,
            initial_tokens: 0,
            last_tokens: 0,
            last_tps: 0.0,
        }
    }
}

pub struct StateTransitionDetector {
    slots: [SlotRuntimeRecord; 5],
}

impl Default for StateTransitionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTransitionDetector {
    pub fn new() -> Self {
        Self {
            slots: Default::default(),
        }
    }

    pub fn update(
        &mut self,
        slot_inputs: &[(u8, bool, u64, f32)],
    ) -> Vec<SessionTransition> {
        let mut transitions = Vec::new();
        let now = std::time::Instant::now();

        for &(slot, running, tokens, tps) in slot_inputs {
            let idx = slot as usize;
            if idx >= self.slots.len() {
                continue;
            }

            let prev_running = self.slots[idx].is_running;
            if !prev_running && running {
                self.slots[idx].is_running = true;
                self.slots[idx].start_time = Some(now);
                self.slots[idx].initial_tokens = tokens;
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = tps;
                transitions.push(SessionTransition::Started { slot });
            } else if prev_running && !running {
                let duration_secs = self.slots[idx]
                    .start_time
                    .map(|st| now.duration_since(st).as_secs_f32())
                    .unwrap_or(0.0);
                let diff_tokens = if tokens >= self.slots[idx].initial_tokens {
                    tokens - self.slots[idx].initial_tokens
                } else {
                    tokens
                };
                let avg_tps = if duration_secs > 0.1 {
                    diff_tokens as f32 / duration_secs
                } else {
                    self.slots[idx].last_tps
                };

                self.slots[idx].is_running = false;
                self.slots[idx].start_time = None;
                self.slots[idx].initial_tokens = 0;
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = 0.0;

                transitions.push(SessionTransition::Finished {
                    slot,
                    duration_secs,
                    total_tokens: diff_tokens,
                    avg_tps,
                });
            } else if running {
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = tps;
            }
        }

        transitions
    }

    #[allow(dead_code)]
    pub fn has_any_running(&self) -> bool {
        self.slots.iter().any(|s| s.is_running)
    }
}

pub fn play_builtin_sound() {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    unsafe {
        PlaySoundW(
            BUILTIN_WAV.as_ptr() as *const u16,
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn mci_send(command: &[u16]) -> u32 {
    use windows_sys::Win32::Media::Multimedia::mciSendStringW;
    unsafe {
        mciSendStringW(
            command.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
        )
    }
}

pub fn play_done_sound(path: &Path) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};

    if !path.exists() {
        return;
    }

    let Some(path_str) = path.to_str() else {
        return;
    };

    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "wav" {
        let wide = to_wide(path_str);
        unsafe {
            PlaySoundW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
            );
        }
        return;
    }

    let device_type = match ext.as_str() {
        "mp3" | "wma" | "m4a" | "aac" => "mpegvideo",
        "mid" | "midi" | "rmi" => "sequencer",
        "aiff" | "aif" => "avivideo",
        _ => return,
    };

    let close_cmd = format!("close {}", MCI_ALIAS);
    let open_typed_cmd = format!(
        "open \"{}\" type {} alias {}",
        path_str, device_type, MCI_ALIAS
    );
    let open_plain_cmd = format!("open \"{}\" alias {}", path_str, MCI_ALIAS);
    let play_cmd = format!("play {}", MCI_ALIAS);

    let close_w = to_wide(&close_cmd);
    mci_send(&close_w);

    let typed_w = to_wide(&open_typed_cmd);
    let typed_ret = mci_send(&typed_w);
    if typed_ret != 0 {
        let plain_w = to_wide(&open_plain_cmd);
        mci_send(&plain_w);
    }

    let play_w = to_wide(&play_cmd);
    mci_send(&play_w);
}

pub fn pick_sound_file(owner_hwnd: isize) -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
        OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };

    let filter = to_wide("音频文件\0*.wav;*.mp3;*.wma;*.m4a;*.aac;*.mid\0所有文件\0*.*\0");
    let title = to_wide("选择完成音效");
    let mut buffer = vec![0u16; 4096];

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner_hwnd as *mut core::ffi::c_void;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buffer.as_mut_ptr();
    ofn.nMaxFile = buffer.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_HIDEREADONLY
        | OFN_NOCHANGEDIR;

    let result = unsafe { GetOpenFileNameW(&mut ofn) };
    let picked = if result == 0 {
        None
    } else {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        let picked = String::from_utf16_lossy(&buffer[..end]);
        if picked.is_empty() {
            None
        } else {
            Some(PathBuf::from(picked))
        }
    };

    if hr >= 0 {
        unsafe {
            CoUninitialize();
        }
    }

    picked
}

pub fn file_stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_builtin_sound_does_not_panic() {
        play_builtin_sound();
    }

    #[test]
    fn test_file_stem_of() {
        assert_eq!(file_stem_of(Path::new("C:/test/sound.wav")), "sound");
        assert_eq!(file_stem_of(Path::new("music.mp3")), "music");
        assert_eq!(file_stem_of(Path::new("")), "");
    }

    #[test]
    fn test_sound_config_default_and_serde() {
        let config = SoundConfig::default();
        assert!(config.enabled);
        assert!(config.sound_file.is_none());

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SoundConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_transition_detector_lifecycle() {
        let mut detector = StateTransitionDetector::new();
        assert!(!detector.has_any_running());

        let t1 = detector.update(&[(1, true, 100, 50.0)]);
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0], SessionTransition::Started { slot: 1 });
        assert!(detector.has_any_running());

        let t2 = detector.update(&[(1, true, 200, 60.0)]);
        assert!(t2.is_empty());
        assert!(detector.has_any_running());

        let t3 = detector.update(&[(1, false, 350, 0.0)]);
        assert_eq!(t3.len(), 1);
        match t3[0] {
            SessionTransition::Finished {
                slot,
                total_tokens,
                ..
            } => {
                assert_eq!(slot, 1);
                assert_eq!(total_tokens, 250);
            }
            _ => panic!("expected Finished transition"),
        }
        assert!(!detector.has_any_running());
    }

    #[test]
    fn test_multi_slot_transitions() {
        let mut detector = StateTransitionDetector::new();
        let t1 = detector.update(&[(0, false, 0, 0.0), (3, true, 100, 40.0)]);
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0], SessionTransition::Started { slot: 3 });

        let t2 = detector.update(&[(0, false, 0, 0.0), (3, false, 300, 0.0)]);
        assert_eq!(t2.len(), 1);
        match t2[0] {
            SessionTransition::Finished { slot, total_tokens, .. } => {
                assert_eq!(slot, 3);
                assert_eq!(total_tokens, 200);
            }
            _ => panic!("expected Finished transition for slot 3"),
        }
    }
}

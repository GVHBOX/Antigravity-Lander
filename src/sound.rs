use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const MCI_ALIAS: &str = "lander_done";

const BUILTIN_WAV: &[u8] = include_bytes!("../assets/done.wav");

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
}

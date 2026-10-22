use std::path::{Path, PathBuf};

#[cfg(windows)]
fn find_rc_exe() -> Option<PathBuf> {
    let base = Path::new("C:\\Program Files (x86)\\Windows Kits\\10\\bin");
    let mut best: Option<(String, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("x64").join("rc.exe");
            if candidate.exists() {
                let version = entry.file_name().to_string_lossy().to_string();
                if best.as_ref().map_or(true, |(current, _)| version > *current) {
                    best = Some((version, candidate));
                }
            }
        }
    }
    best.map(|(_, path)| path)
}

#[cfg(windows)]
fn embed_windows_resources() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
    let icon = std::fs::canonicalize("assets/icon.ico").expect("assets/icon.ico missing");
    let icon_path = icon.to_string_lossy().replace('\\', "/");
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    let major = parts.next().unwrap_or("1");
    let minor = parts.next().unwrap_or("0");
    let patch = parts.next().unwrap_or("0");

    let rc_content = format!(
        "#pragma code_page(65001)\n\
         1 ICON \"{icon_path}\"\n\
         1 VERSIONINFO\n\
         FILEVERSION {major},{minor},{patch},0\n\
         PRODUCTVERSION {major},{minor},{patch},0\n\
         FILEFLAGSMASK 0x3f\n\
         FILEFLAGS 0x0\n\
         FILEOS 0x40004\n\
         FILETYPE 0x1\n\
         FILESUBTYPE 0x0\n\
         BEGIN\n\
         \x20   BLOCK \"StringFileInfo\"\n\
         \x20   BEGIN\n\
         \x20       BLOCK \"040904b0\"\n\
         \x20       BEGIN\n\
         \x20           VALUE \"FileDescription\", \"Antigravity Lander\"\n\
         \x20           VALUE \"ProductName\", \"Antigravity Lander\"\n\
         \x20           VALUE \"FileVersion\", \"{version}.0\"\n\
         \x20       END\n\
         \x20   END\n\
         \x20   BLOCK \"VarFileInfo\"\n\
         \x20   BEGIN\n\
         \x20       VALUE \"Translation\", 0x409, 1200\n\
         \x20   END\n\
         END\n"
    );

    let rc_path = out_dir.join("app.rc");
    let res_path = out_dir.join("app.res");
    std::fs::write(&rc_path, rc_content).expect("failed to write app.rc");

    let rc_exe = find_rc_exe().expect("rc.exe not found in Windows Kits");
    let output = std::process::Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .output()
        .expect("failed to launch rc.exe");
    assert!(
        output.status.success(),
        "rc.exe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("cargo:rustc-link-arg={}", res_path.display());
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=src/cockpit.slint");
    println!("cargo:rerun-if-changed=src/ui");
    println!("cargo:rerun-if-changed=build.rs");

    slint_build::compile("src/cockpit.slint").expect("Failed to compile Slint UI specification");

    #[cfg(windows)]
    embed_windows_resources();
}

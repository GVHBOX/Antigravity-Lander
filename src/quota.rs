use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AccountProfile {
    pub name: String,
    pub email: String,
    pub tier: String,
    pub avatar_path: Option<PathBuf>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct QuotaWindow {
    pub remaining_pct: f32,
    pub pct_text: String,
    pub reset_text: String,
    pub reset_time_rfc3339: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct InstanceQuotaState {
    pub is_available: bool,
    pub profile: AccountProfile,
    pub weekly: QuotaWindow,
    pub five_h: QuotaWindow,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct QuotaCacheFile {
    pub slots: Vec<Option<InstanceQuotaState>>,
}

impl QuotaCacheFile {
    pub fn get_slot(&self, idx: usize) -> Option<&InstanceQuotaState> {
        self.slots.get(idx).and_then(|s| s.as_ref())
    }

    pub fn set_slot(&mut self, idx: usize, st: InstanceQuotaState) {
        if self.slots.len() <= idx {
            self.slots.resize(idx + 1, None);
        }
        self.slots[idx] = Some(st);
    }
}

fn quota_cache_path() -> PathBuf {
    if let Ok(base) = crate::launcher::LauncherConfig::resolve_base_dir() {
        base.join("data").join("quota_cache.json")
    } else {
        PathBuf::from("data").join("quota_cache.json")
    }
}

pub fn load_quota_cache() -> QuotaCacheFile {
    let path = quota_cache_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(mut cache) = serde_json::from_str::<QuotaCacheFile>(&data) {
            if cache.slots.len() < 5 {
                cache.slots.resize(5, None);
            }
            return cache;
        }
    }
    let mut cache = QuotaCacheFile::default();
    cache.slots.resize(5, None);
    cache
}

pub fn save_quota_cache(cache: &QuotaCacheFile) {
    let path = quota_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, json);
    }
}

pub fn extract_offline_profile(token_path: &Path) -> Option<AccountProfile> {
    if !token_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(token_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let id_token = json.get("id_token").and_then(|v| v.as_str())?;
    let mut parts = id_token.split('.');
    let _header = parts.next()?;
    let payload_b64 = parts.next()?;

    use base64::Engine;
    let mut padded = payload_b64.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let decoded = base64::engine::general_purpose::URL_SAFE.decode(&padded).ok()
        .or_else(|| base64::engine::general_purpose::STANDARD.decode(&padded).ok())?;
    let payload_json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let email = payload_json.get("email").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let name = payload_json.get("name").and_then(|v| v.as_str()).unwrap_or("Antigravity User").to_string();
    if email.is_empty() {
        return None;
    }
    Some(AccountProfile {
        name,
        email,
        tier: "Pro".to_string(),
        avatar_path: None,
    })
}

pub fn compute_window_offline(window: &QuotaWindow, period_secs: i64) -> QuotaWindow {
    if window.reset_time_rfc3339.is_empty() {
        return window.clone();
    }
    let target = match chrono::DateTime::parse_from_rfc3339(&window.reset_time_rfc3339) {
        Ok(t) => t.with_timezone(&chrono::Utc),
        Err(_) => return window.clone(),
    };
    let now = chrono::Utc::now();
    let diff = target.signed_duration_since(now);
    if diff.num_seconds() > 0 {
        let total_minutes = diff.num_minutes();
        let days = total_minutes / (24 * 60);
        let hours = (total_minutes % (24 * 60)) / 60;
        let mins = total_minutes % 60;
        let reset_text = if days > 0 {
            format!("{}天{}小时后重置", days, hours)
        } else if hours > 0 {
            format!("{}小时{}分钟后重置", hours, mins)
        } else {
            format!("{}分钟后重置", mins.max(1))
        };
        QuotaWindow {
            remaining_pct: window.remaining_pct,
            pct_text: window.pct_text.clone(),
            reset_text,
            reset_time_rfc3339: window.reset_time_rfc3339.clone(),
        }
    } else {
        if period_secs <= 0 {
            return window.clone();
        }
        let mut next_target = target;
        let step = chrono::Duration::seconds(period_secs);
        while next_target <= now {
            next_target = next_target + step;
        }
        let next_diff = next_target.signed_duration_since(now);
        let total_minutes = next_diff.num_minutes();
        let days = total_minutes / (24 * 60);
        let hours = (total_minutes % (24 * 60)) / 60;
        let mins = total_minutes % 60;
        let reset_text = if days > 0 {
            format!("{}天{}小时后重置", days, hours)
        } else if hours > 0 {
            format!("{}小时{}分钟后重置", hours, mins)
        } else {
            format!("{}分钟后重置", mins.max(1))
        };
        QuotaWindow {
            remaining_pct: 1.0,
            pct_text: "100%".to_string(),
            reset_text,
            reset_time_rfc3339: next_target.to_rfc3339(),
        }
    }
}

fn extract_host_offline_profile(base_dir: &Path) -> Option<AccountProfile> {
    let roaming = if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
    } else {
        crate::launcher::LauncherConfig::resolve_host_user_dir()?
            .join("AppData")
            .join("Roaming")
    };
    let app_storage_p = roaming.join("Antigravity").join("app_storage.json");
    if !app_storage_p.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&app_storage_p).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let email = json.get("jetski.onboarding.lastLoginUsername")?.as_str()?;
    if email.is_empty() || !email.contains('@') {
        return None;
    }
    let default_avatar = base_dir.join("data").join("avatars").join("host.png");
    Some(AccountProfile {
        name: email.split('@').next().unwrap_or("Host").to_string(),
        email: email.to_string(),
        tier: "Pro".to_string(),
        avatar_path: if default_avatar.exists() { Some(default_avatar) } else { None },
    })
}

pub fn compute_offline_state(
    cached: Option<&InstanceQuotaState>,
    token_path: Option<&Path>,
    tag: &str,
) -> Option<InstanceQuotaState> {
    let mut state = cached.cloned().unwrap_or_default();
    let base_dir = crate::launcher::LauncherConfig::resolve_base_dir().unwrap_or_else(|_| PathBuf::from("."));
    let default_avatar = base_dir.join("data").join("avatars").join(format!("{}.png", tag));

    if !state.is_available {
        if let Some(tp) = token_path {
            if let Some(prof) = extract_offline_profile(tp) {
                state.is_available = true;
                state.profile = prof;
                if default_avatar.exists() {
                    state.profile.avatar_path = Some(default_avatar.clone());
                }
            }
        }
        if !state.is_available && tag == "host" {
            if let Some(prof) = extract_host_offline_profile(&base_dir) {
                state.is_available = true;
                state.profile = prof;
                if state.weekly.pct_text.is_empty() || state.weekly.pct_text == "--" {
                    state.weekly.remaining_pct = 1.0;
                    state.weekly.pct_text = "100%".to_string();
                }
                if state.five_h.pct_text.is_empty() || state.five_h.pct_text == "--" {
                    state.five_h.remaining_pct = 1.0;
                    state.five_h.pct_text = "100%".to_string();
                }
            }
        }
    }

    if !state.is_available {
        return None;
    }

    if let Some(ref p) = state.profile.avatar_path {
        let abs_p = if p.is_relative() { base_dir.join(p) } else { p.clone() };
        if abs_p.exists() {
            state.profile.avatar_path = Some(abs_p);
        } else if default_avatar.exists() {
            state.profile.avatar_path = Some(default_avatar);
        }
    } else if default_avatar.exists() {
        state.profile.avatar_path = Some(default_avatar);
    }

    if !state.weekly.reset_time_rfc3339.is_empty() {
        state.weekly = compute_window_offline(&state.weekly, 7 * 24 * 3600);
    }
    if !state.five_h.reset_time_rfc3339.is_empty() {
        state.five_h = compute_window_offline(&state.five_h, 5 * 3600);
    }
    Some(state)
}

#[derive(Deserialize)]
struct PlanInfo {
    #[serde(rename = "planName")]
    plan_name: Option<String>,
}

#[derive(Deserialize)]
struct PlanStatus {
    #[serde(rename = "planInfo")]
    plan_info: Option<PlanInfo>,
}

#[derive(Deserialize)]
struct RawUserStatus {
    name: Option<String>,
    email: Option<String>,
    #[serde(rename = "planStatus")]
    plan_status: Option<PlanStatus>,
    #[serde(rename = "profilePictureUrl")]
    profile_picture_url: Option<String>,
}

#[derive(Deserialize)]
struct UserStatusResponse {
    #[serde(rename = "userStatus")]
    user_status: Option<RawUserStatus>,
}

#[derive(Deserialize)]
struct RawBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Deserialize)]
struct RawGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    buckets: Option<Vec<RawBucket>>,
}

#[derive(Deserialize)]
struct QuotaSummaryResponse {
    response: Option<QuotaResponseInner>,
}

#[derive(Deserialize)]
struct QuotaResponseInner {
    groups: Option<Vec<RawGroup>>,
}

fn rpc_post(port: u16, method: &str, csrf: &str) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(1500),
    ).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));

    let req = format!(
        "POST /exa.language_server_pb.LanguageServerService/{} HTTP/1.1\r\n\
Host: 127.0.0.1:{}\r\n\
Content-Type: application/json\r\n\
Connect-Protocol-Version: 1\r\n\
x-codeium-csrf-token: {}\r\n\
Content-Length: 2\r\n\
Connection: close\r\n\
\r\n\
{{}}",
        method, port, csrf
    );

    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = Vec::with_capacity(32768);
    let mut chunk = [0u8; 4096];
    let mut content_length: Option<usize> = None;
    let mut header_len: Option<usize> = None;

    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                resp.extend_from_slice(&chunk[..n]);
                if header_len.is_none() {
                    if let Some(pos) = resp.windows(4).position(|w| w == b"\r\n\r\n") {
                        let h_len = pos + 4;
                        header_len = Some(h_len);
                        if let Ok(header_str) = std::str::from_utf8(&resp[..pos]) {
                            for line in header_str.lines() {
                                if let Some((k, v)) = line.split_once(':') {
                                    if k.trim().eq_ignore_ascii_case("content-length") {
                                        if let Ok(cl) = v.trim().parse::<usize>() {
                                            content_length = Some(cl);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let (Some(hlen), Some(clen)) = (header_len, content_length) {
                    if resp.len() >= hlen + clen {
                        break;
                    }
                } else if let Some(hlen) = header_len {
                    let body = &resp[hlen..];
                    if let Some(start) = body.iter().position(|&b| b == b'{') {
                        if let Some(end) = body.iter().rposition(|&b| b == b'}') {
                            if end >= start {
                                if serde_json::from_slice::<serde_json::Value>(&body[start..=end]).is_ok() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }

    let text = String::from_utf8_lossy(&resp);
    let body_start = text.find("\r\n\r\n")?;
    let raw_body = if let Some(clen) = content_length {
        let end = (body_start + 4 + clen).min(text.len());
        &text[body_start + 4..end]
    } else {
        &text[body_start + 4..]
    };
    let json_start = raw_body.find('{')?;
    let json_end = raw_body.rfind('}')?;
    if json_end >= json_start {
        Some(raw_body[json_start..=json_end].to_string())
    } else {
        None
    }
}

fn format_reset_time(rfc3339: &str) -> String {
    let target = match chrono::DateTime::parse_from_rfc3339(rfc3339) {
        Ok(t) => t.with_timezone(&chrono::Utc),
        Err(_) => return String::new(),
    };
    let now = chrono::Utc::now();
    let diff = target.signed_duration_since(now);
    if diff.num_seconds() <= 0 {
        return "重置中".to_string();
    }

    let total_minutes = diff.num_minutes();
    let days = total_minutes / (24 * 60);
    let hours = (total_minutes % (24 * 60)) / 60;
    let mins = total_minutes % 60;

    if days > 0 {
        format!("{}天{}小时后重置", days, hours)
    } else if hours > 0 {
        format!("{}小时{}分钟后重置", hours, mins)
    } else {
        format!("{}分钟后重置", mins.max(1))
    }
}

fn save_avatar_file(avatar_data_uri: &str, tag: &str) -> Option<PathBuf> {
    use base64::Engine;
    let b64_str = avatar_data_uri.split_once(',')
        .map(|(_, b)| b)
        .unwrap_or(avatar_data_uri);

    let bytes = base64::engine::general_purpose::STANDARD.decode(b64_str.trim()).ok()?;
    let base_dir = crate::launcher::LauncherConfig::resolve_base_dir().unwrap_or_else(|_| PathBuf::from("."));
    let avatars_dir = base_dir.join("data").join("avatars");
    let _ = std::fs::create_dir_all(&avatars_dir);
    let target = avatars_dir.join(format!("{}.png", tag));

    if let Ok(img) = image::load_from_memory(&bytes) {
        let mut png_bytes = Vec::new();
        if img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
            std::fs::write(&target, png_bytes).ok()?;
            return Some(target);
        }
    }
    std::fs::write(&target, bytes).ok()?;
    Some(target)
}

pub fn fetch_instance_quota(port: u16, csrf: &str, tag: &str) -> InstanceQuotaState {
    let mut state = InstanceQuotaState::default();
    let candidate_ports = [port, port.saturating_add(1), port.saturating_sub(1)];
    let mut valid_port = None;

    for &p in &candidate_ports {
        if let Some(user_json) = rpc_post(p, "GetUserStatus", csrf) {
            if let Ok(user_resp) = serde_json::from_str::<UserStatusResponse>(&user_json) {
                if let Some(u) = user_resp.user_status {
                    state.is_available = true;
                    state.profile.name = u.name.unwrap_or_else(|| "Antigravity User".to_string());
                    state.profile.email = u.email.unwrap_or_default();
                    state.profile.tier = u.plan_status
                        .and_then(|p| p.plan_info)
                        .and_then(|i| i.plan_name)
                        .unwrap_or_else(|| "Pro".to_string());

                    if let Some(pic_url) = u.profile_picture_url {
                        if pic_url.contains("data:image/") {
                            state.profile.avatar_path = save_avatar_file(&pic_url, tag);
                        }
                    }
                    valid_port = Some(p);
                    break;
                }
            }
        }
    }

    let active_port = valid_port.unwrap_or(port);
    if let Some(quota_json) = rpc_post(active_port, "RetrieveUserQuotaSummary", csrf) {
        if let Ok(quota_resp) = serde_json::from_str::<QuotaSummaryResponse>(&quota_json) {
            if let Some(inner) = quota_resp.response {
                if let Some(groups) = inner.groups {
                    for group in groups {
                        let is_gemini = group.display_name.as_deref().unwrap_or("").to_lowercase().contains("gemini");
                        if is_gemini {
                            if let Some(buckets) = group.buckets {
                                for bucket in buckets {
                                    let is_weekly = bucket.window.as_deref() == Some("weekly")
                                        || bucket.bucket_id.as_deref().unwrap_or("").ends_with("-weekly");
                                    let is_5h = bucket.window.as_deref() == Some("5h")
                                        || bucket.bucket_id.as_deref().unwrap_or("").ends_with("-5h");

                                    let fraction = bucket.remaining_fraction.unwrap_or(0.0) as f32;
                                    let pct_clamped = fraction.clamp(0.0, 1.0);
                                    let pct_text = format!("{:.0}%", pct_clamped * 100.0);
                                    let reset_rfc = bucket.reset_time.as_deref().unwrap_or("").to_string();
                                    let reset_text = bucket.reset_time.as_deref().map(format_reset_time).unwrap_or_default();

                                    if is_weekly {
                                        state.weekly = QuotaWindow {
                                            remaining_pct: pct_clamped,
                                            pct_text,
                                            reset_text,
                                            reset_time_rfc3339: reset_rfc,
                                        };
                                    } else if is_5h {
                                        state.five_h = QuotaWindow {
                                            remaining_pct: pct_clamped,
                                            pct_text,
                                            reset_text,
                                            reset_time_rfc3339: reset_rfc,
                                        };
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    state
}

pub fn render_quota_ring(pct: f32, is_dark: bool) -> slint::Image {
    let circumference = 50.265f32;
    let track_color = if is_dark { "#1e293b" } else { "#e2e8f0" };

    if pct < 0.0 {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="8" fill="none" stroke="{}" stroke-width="2.6"/></svg>"#,
            track_color
        );
        return slint::Image::load_from_svg_data(svg.as_bytes()).unwrap_or_default();
    }

    let clamped = pct.clamp(0.0, 1.0);
    let offset = circumference * (1.0 - clamped);
    let stroke_color = if clamped < 0.10 {
        if is_dark { "#dc2626" } else { "#ef4444" }
    } else if clamped < 0.25 {
        if is_dark { "#ca8a04" } else { "#eab308" }
    } else {
        if is_dark { "#16a34a" } else { "#22c55e" }
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="8" fill="none" stroke="{}" stroke-width="2.6"/><circle cx="10" cy="10" r="8" fill="none" stroke="{}" stroke-width="2.6" stroke-linecap="round" stroke-dasharray="50.265" stroke-dashoffset="{:.2}" transform="rotate(-90 10 10)"/></svg>"#,
        track_color, stroke_color, offset
    );
    slint::Image::load_from_svg_data(svg.as_bytes()).unwrap_or_default()
}

fn resolve_language_server_bin() -> Option<PathBuf> {
    if let Some(app_dir) = crate::launcher::LauncherConfig::detect_app_dir() {
        let p = app_dir
            .join("resources")
            .join("bin")
            .join("language_server.exe");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_appdata)
            .join("Programs")
            .join("antigravity")
            .join("resources")
            .join("bin")
            .join("language_server.exe");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(user_dir) = crate::launcher::LauncherConfig::resolve_host_user_dir() {
        let p = user_dir
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("antigravity")
            .join("resources")
            .join("bin")
            .join("language_server.exe");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn fetch_offline_quota_headless(slot: usize) -> Option<InstanceQuotaState> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let bin = resolve_language_server_bin()?;
    let tag = if slot == 0 { "host".to_string() } else { format!("sub{}", slot) };

    let (home_dir, roaming_dir) = if slot == 0 {
        if !crate::launcher::LauncherConfig::is_host_token_present() {
            return None;
        }
        let user_dir = crate::launcher::LauncherConfig::resolve_host_user_dir()?;
        let roaming = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| user_dir.join("AppData").join("Roaming"));
        (user_dir, roaming)
    } else {
        let launcher = crate::launcher::LauncherConfig::for_slot(slot).ok()?;
        if !launcher.is_token_present() {
            return None;
        }
        (launcher.home_dir, launcher.roaming_dir)
    };

    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let csrf = format!("{:016x}{:016x}", now_nanos, std::process::id());

    let mut cmd = Command::new(&bin);
    cmd.args([
        "--standalone",
        "--override_ide_name", "antigravity",
        "--subclient_type", "hub",
        "--override_ide_version", "2.22.0",
        "--override_user_agent_name", "antigravity",
        "--https_server_port", "0",
        "--csrf_token", &csrf,
        "--app_data_dir", "antigravity",
        "--api_server_url", "https://generativelanguage.googleapis.com",
        "--cloud_code_endpoint", "https://daily-cloudcode-pa.googleapis.com",
    ]);

    cmd.env("USERPROFILE", &home_dir);
    cmd.env("HOME", &home_dir);
    cmd.env("APPDATA", &roaming_dir);
    if let Ok(real_local) = std::env::var("LOCALAPPDATA") {
        cmd.env("LOCALAPPDATA", real_local);
    }

    if crate::proxy::ProxyManager::is_port_online(crate::proxy::DEFAULT_PROXY_PORT) {
        let proxy_url = format!("http://127.0.0.1:{}", crate::proxy::DEFAULT_PROXY_PORT);
        cmd.env("HTTP_PROXY", &proxy_url);
        cmd.env("HTTPS_PROXY", &proxy_url);
    }

    cmd.creation_flags(0x08000000);

    let mut child = cmd.spawn().ok()?;
    let child_pid = child.id();

    let mut detected_port = None;
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(100));
        let output = Command::new("netstat")
            .args(["-ano", "-p", "tcp"])
            .creation_flags(0x08000000)
            .output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            let target_pid = child_pid.to_string();
            let mut ports = Vec::new();
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0].eq_ignore_ascii_case("tcp") && parts.iter().any(|p| p.eq_ignore_ascii_case("listening")) && parts[parts.len() - 1] == target_pid {
                    if let Some(port) = parts[1].rfind(':').and_then(|pos| parts[1][pos + 1..].parse::<u16>().ok()) {
                        ports.push(port);
                    }
                }
            }
            if !ports.is_empty() {
                ports.sort_unstable();
                detected_port = ports.last().copied();
                if detected_port.is_some() {
                    break;
                }
            }
        }
    }

    let quota_state = if let Some(port) = detected_port {
        let st = fetch_instance_quota(port, &csrf, &tag);
        if st.is_available {
            Some(st)
        } else {
            None
        }
    } else {
        None
    };

    let _ = child.kill();
    let _ = child.wait();

    if let Some(ref st) = quota_state {
        let mut cache = load_quota_cache();
        cache.set_slot(slot, st.clone());
        save_quota_cache(&cache);
    }

    quota_state
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn test_rpc_post_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let body = "{\"userStatus\":{\"email\":\"test@example.com\"}}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).unwrap();
            std::thread::sleep(Duration::from_millis(500));
        });

        let start = std::time::Instant::now();
        let result = rpc_post(port, "GetUserStatus", "csrf123");
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_millis(300));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "{\"userStatus\":{\"email\":\"test@example.com\"}}");
        handle.join().unwrap();
    }

    #[test]
    fn test_rpc_post_without_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let body = "{\"userStatus\":{\"email\":\"chunked@example.com\"}}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
                body
            );
            stream.write_all(resp.as_bytes()).unwrap();
            std::thread::sleep(Duration::from_millis(500));
        });

        let start = std::time::Instant::now();
        let result = rpc_post(port, "GetUserStatus", "csrf123");
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_millis(300));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "{\"userStatus\":{\"email\":\"chunked@example.com\"}}");
        handle.join().unwrap();
    }
}

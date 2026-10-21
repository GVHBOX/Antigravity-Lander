use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
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
    if let Ok(data) = std::fs::read_to_string(&path)
        && let Ok(mut cache) = serde_json::from_str::<QuotaCacheFile>(&data) {
            if cache.slots.len() < 5 {
                cache.slots.resize(5, None);
            }
            return cache;
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
            next_target += step;
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
        if let Some(tp) = token_path
            && let Some(prof) = extract_offline_profile(tp) {
                state.is_available = true;
                state.profile = prof;
                if default_avatar.exists() {
                    state.profile.avatar_path = Some(default_avatar.clone());
                }
            }
        if !state.is_available && tag == "host"
            && let Some(prof) = extract_host_offline_profile(&base_dir) {
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
    rpc_post_json(port, method, csrf, "{}")
}

pub fn rpc_post_json(port: u16, method: &str, csrf: &str, body: &str) -> Option<String> {
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
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{}",
        method, port, csrf, body.len(), body
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
                if header_len.is_none()
                    && let Some(pos) = resp.windows(4).position(|w| w == b"\r\n\r\n") {
                        let h_len = pos + 4;
                        header_len = Some(h_len);
                        if let Ok(header_str) = std::str::from_utf8(&resp[..pos]) {
                            for line in header_str.lines() {
                                if let Some((k, v)) = line.split_once(':')
                                    && k.trim().eq_ignore_ascii_case("content-length")
                                        && let Ok(cl) = v.trim().parse::<usize>() {
                                            content_length = Some(cl);
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
                    if let Some(start) = body.iter().position(|&b| b == b'{')
                        && let Some(end) = body.iter().rposition(|&b| b == b'}')
                            && end >= start
                                && serde_json::from_slice::<serde_json::Value>(&body[start..=end]).is_ok() {
                                    break;
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
        if let Some(user_json) = rpc_post(p, "GetUserStatus", csrf)
            && let Ok(user_resp) = serde_json::from_str::<UserStatusResponse>(&user_json)
                && let Some(u) = user_resp.user_status {
                    state.is_available = true;
                    state.profile.name = u.name.unwrap_or_else(|| "Antigravity User".to_string());
                    state.profile.email = u.email.unwrap_or_default();
                    state.profile.tier = u.plan_status
                        .and_then(|p| p.plan_info)
                        .and_then(|i| i.plan_name)
                        .unwrap_or_else(|| "Pro".to_string());

                    if let Some(pic_url) = u.profile_picture_url
                        && pic_url.contains("data:image/") {
                            state.profile.avatar_path = save_avatar_file(&pic_url, tag);
                        }
                    valid_port = Some(p);
                    break;
                }
    }

    let active_port = valid_port.unwrap_or(port);
    if let Some(quota_json) = rpc_post(active_port, "RetrieveUserQuotaSummary", csrf)
        && let Ok(quota_resp) = serde_json::from_str::<QuotaSummaryResponse>(&quota_json)
            && let Some(inner) = quota_resp.response
                && let Some(groups) = inner.groups {
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

    state
}

thread_local! {
    static QUOTA_RING_CACHE: RefCell<HashMap<(i32, bool), slint::Image>> = RefCell::new(HashMap::new());
}

pub fn render_quota_ring(pct: f32, is_dark: bool) -> slint::Image {
    let bucket = if pct.is_nan() || pct < 0.0 {
        -1
    } else {
        (pct.clamp(0.0, 1.0) * 100.0).round() as i32
    };
    let key = (bucket, is_dark);

    if let Some(cached) = QUOTA_RING_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return cached;
    }

    let circumference = 50.265f32;
    let track_color = if is_dark { "#1e293b" } else { "#e2e8f0" };

    let image = if bucket < 0 {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="8" fill="none" stroke="{}" stroke-width="2.6"/></svg>"#,
            track_color
        );
        slint::Image::load_from_svg_data(svg.as_bytes()).unwrap_or_default()
    } else {
        let clamped = (bucket as f32) / 100.0;
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
    };

    QUOTA_RING_CACHE.with(|c| {
        c.borrow_mut().insert(key, image.clone());
    });

    image
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

    #[test]
    fn test_render_quota_ring_cache_and_robustness() {
        let img_normal = render_quota_ring(0.5, true);
        assert!(img_normal.size().width > 0);
        let img_cached = render_quota_ring(0.5, true);
        assert_eq!(img_normal.size(), img_cached.size());

        let img_zero = render_quota_ring(0.0, true);
        assert!(img_zero.size().width > 0);

        let img_full = render_quota_ring(1.0, false);
        assert!(img_full.size().width > 0);

        let img_neg = render_quota_ring(-1.0, true);
        assert!(img_neg.size().width > 0);

        let img_nan = render_quota_ring(f32::NAN, true);
        assert!(img_nan.size().width > 0);

        let img_overflow = render_quota_ring(2.5, false);
        assert!(img_overflow.size().width > 0);
    }

    #[test]
    fn test_compute_window_offline() {
        let empty = QuotaWindow::default();
        let computed_empty = compute_window_offline(&empty, 3600);
        assert_eq!(computed_empty.reset_text, "");

        let future = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
        let win_future = QuotaWindow {
            remaining_pct: 0.75,
            pct_text: "75%".to_string(),
            reset_text: String::new(),
            reset_time_rfc3339: future,
        };
        let computed_future = compute_window_offline(&win_future, 5 * 3600);
        assert_eq!(computed_future.remaining_pct, 0.75);
        assert_eq!(computed_future.pct_text, "75%");
        assert!(computed_future.reset_text.contains("小时"));

        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let win_past = QuotaWindow {
            remaining_pct: 0.1,
            pct_text: "10%".to_string(),
            reset_text: String::new(),
            reset_time_rfc3339: past,
        };
        let computed_past = compute_window_offline(&win_past, 5 * 3600);
        assert_eq!(computed_past.remaining_pct, 1.0);
        assert_eq!(computed_past.pct_text, "100%");
        assert!(computed_past.reset_text.contains("小时") || computed_past.reset_text.contains("分钟"));
    }
}

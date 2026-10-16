use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

type OpenFn = unsafe extern "C" fn(*const c_char, *mut *mut c_void, i32, *const c_char) -> i32;
type PrepareFn = unsafe extern "C" fn(*mut c_void, *const c_char, i32, *mut *mut c_void, *mut *const c_char) -> i32;
type StepFn = unsafe extern "C" fn(*mut c_void) -> i32;
type ColumnTextFn = unsafe extern "C" fn(*mut c_void, i32) -> *const u8;
type ColumnIntFn = unsafe extern "C" fn(*mut c_void, i32) -> i32;
type FinalizeFn = unsafe extern "C" fn(*mut c_void) -> i32;
type CloseFn = unsafe extern "C" fn(*mut c_void) -> i32;

unsafe extern "system" {
    fn LoadLibraryA(lpLibFileName: *const c_char) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const c_char) -> *mut c_void;
}

struct SqliteFunctions {
    open: OpenFn,
    prep: PrepareFn,
    step: StepFn,
    col: ColumnTextFn,
    col_int: ColumnIntFn,
    fin: FinalizeFn,
    close: CloseFn,
}

unsafe impl Send for SqliteFunctions {}
unsafe impl Sync for SqliteFunctions {}

static SQLITE_FUNCS: OnceLock<Option<SqliteFunctions>> = OnceLock::new();

fn get_sqlite_funcs() -> Option<&'static SqliteFunctions> {
    SQLITE_FUNCS.get_or_init(|| {
        unsafe {
            let handle = LoadLibraryA(c"winsqlite3.dll".as_ptr());
            if handle.is_null() {
                return None;
            }
            let open_fn: Option<OpenFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_open_v2".as_ptr()));
            let prep_fn: Option<PrepareFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_prepare_v2".as_ptr()));
            let step_fn: Option<StepFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_step".as_ptr()));
            let col_fn: Option<ColumnTextFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_column_text".as_ptr()));
            let col_int_fn: Option<ColumnIntFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_column_int".as_ptr()));
            let fin_fn: Option<FinalizeFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_finalize".as_ptr()));
            let close_fn: Option<CloseFn> = std::mem::transmute(GetProcAddress(handle, c"sqlite3_close".as_ptr()));

            if let (Some(open), Some(prep), Some(step), Some(col), Some(col_int), Some(fin), Some(close)) =
                (open_fn, prep_fn, step_fn, col_fn, col_int_fn, fin_fn, close_fn)
            {
                Some(SqliteFunctions { open, prep, step, col, col_int, fin, close })
            } else {
                None
            }
        }
    }).as_ref()
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
    pub project_name: String,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub context_tokens: u64,
    pub cache_rate: f32,
    pub is_running: bool,
    pub last_updated_secs_ago: u64,
    pub context_str: String,
    pub cache_rate_str: String,
    pub time_ago_str: String,
    pub step_count: u32,
    pub round_duration_secs: f32,
    pub round_tps: f32,
    pub peak_tps: f32,
    pub avg_tps: f32,
    pub recent_tps: [f32; 16],
    pub recent_turn_tokens: [u64; 16],
    pub recent_turn_durations: [f32; 16],
    pub recent_turn_indices: [u32; 16],
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct SessionGroup {
    pub project_name: String,
    pub sessions: Vec<SessionItem>,
    pub has_running: bool,
    pub latest_secs_ago: u64,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct InstanceSessions {
    pub slot: u8,
    pub groups: Vec<SessionGroup>,
    pub total_sessions: usize,
    pub summary_text: String,
    pub has_running: bool,
}

#[derive(Deserialize)]
struct HeaderStep {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct TailStep {
    #[serde(default)]
    step_index: Option<u64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

#[derive(Default, Clone)]
struct TailMetrics {
    input_tokens: u64,
    cache_read_tokens: u64,
    output_tokens: u64,
    context_tokens: u64,
    cache_rate: f32,
    is_running: bool,
    round_duration_secs: f32,
    round_tps: f32,
    peak_tps: f32,
    avg_tps: f32,
    recent_tps: [f32; 16],
    recent_turn_tokens: [u64; 16],
    recent_turn_durations: [f32; 16],
    recent_turn_indices: [u32; 16],
    file_step_count: u32,
}

static TAIL_CACHE: Mutex<Option<HashMap<String, (u64, SystemTime, TailMetrics)>>> = Mutex::new(None);

fn get_or_parse_tail_metrics(uuid: &str, log_path: &Path, len: u64, mtime: SystemTime) -> TailMetrics {
    let mut cache_guard = TAIL_CACHE.lock().unwrap();
    let cache = cache_guard.get_or_insert_with(HashMap::new);
    if let Some((c_len, c_mtime, metrics)) = cache.get(uuid) {
        if *c_len == len && *c_mtime == mtime {
            return metrics.clone();
        }
    }
    let metrics = parse_tail_metrics(log_path, len);
    if cache.len() > 512 {
        cache.clear();
    }
    cache.insert(uuid.to_string(), (len, mtime, metrics.clone()));
    metrics
}

#[derive(Deserialize)]
struct ProjectConfigFile {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
}

pub fn format_tokens(tokens: u64) -> String {
    if tokens < 1000 {
        format!("{tokens}")
    } else if tokens < 1_000_000 {
        format!("{:.1}K", tokens as f64 / 1000.0)
    } else {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    }
}

pub fn format_time_ago(secs: u64) -> String {
    if secs < 60 {
        "刚刚".to_string()
    } else if secs < 3600 {
        format!("{}分钟前", secs / 60)
    } else if secs < 86400 {
        format!("{}小时前", secs / 3600)
    } else {
        format!("{}天前", secs / 86400)
    }
}

fn sanitize_title(raw: &str) -> String {
    let mut text = raw;
    if let Some(pos) = text.find("<USER_REQUEST>") {
        text = &text[pos + 14..];
    }
    if let Some(pos) = text.find("</USER_REQUEST>") {
        text = &text[..pos];
    }
    if let Some(pos) = text.find("<SYSTEM_MESSAGE>") {
        text = &text[pos + 16..];
    }
    if let Some(pos) = text.find("</SYSTEM_MESSAGE>") {
        text = &text[..pos];
    }
    if let Some(pos) = text.find("content=") {
        text = &text[pos + 8..];
    }

    let cleaned: String = text
        .chars()
        .map(|c| if c == '\r' || c == '\n' || c == '\t' { ' ' } else { c })
        .collect();

    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() > 30 {
        let prefix: String = chars[..30].iter().collect();
        format!("{prefix}...")
    } else {
        trimmed.to_string()
    }
}

fn parse_title_from_log(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(8).flatten() {
        if let Ok(step) = serde_json::from_str::<HeaderStep>(&line)
            && let Some(content) = step.content
        {
            let title = sanitize_title(&content);
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

fn parse_rfc3339_secs(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

fn parse_tail_metrics(path: &Path, file_len: u64) -> TailMetrics {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return TailMetrics::default(),
    };

    let tail_bytes_to_read = if file_len > 131072 { 131072 } else { file_len as usize };
    if file.seek(SeekFrom::End(-(tail_bytes_to_read as i64))).is_err() {
        return TailMetrics::default();
    }

    let mut buf = vec![0u8; tail_bytes_to_read];
    if file.read_exact(&mut buf).is_err() {
        return TailMetrics::default();
    }

    let text = String::from_utf8_lossy(&buf);
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if let Ok(step) = serde_json::from_str::<TailStep>(trimmed) {
                lines.push(step);
            }
        }
    }

    if lines.is_empty() {
        return TailMetrics::default();
    }

    let mut is_running = false;
    for step in lines.iter().rev() {
        if let Some(ref st) = step.status {
            if st == "RUNNING" {
                is_running = true;
            }
            break;
        }
    }

    let mut found_input: u64 = 0;
    let mut found_cache: u64 = 0;
    let mut found_output: u64 = 0;

    for step in lines.iter().rev() {
        if step.input_tokens.is_some() || step.cache_read_tokens.is_some() || step.output_tokens.is_some() {
            found_input = step.input_tokens.unwrap_or(0);
            found_cache = step.cache_read_tokens.unwrap_or(0);
            found_output = step.output_tokens.unwrap_or(0);
            break;
        }
    }

    let total_context = found_input + found_cache + found_output;
    let cache_denom = found_input + found_cache;
    let cache_rate = if cache_denom > 0 {
        found_cache as f32 / cache_denom as f32
    } else {
        0.0
    };

    let file_step_count = lines
        .iter()
        .filter_map(|s| s.step_index)
        .max()
        .map(|idx| (idx + 1) as u32)
        .unwrap_or(0);

    let mut steps_with_out = Vec::new();
    for step in &lines {
        if let (Some(out), Some(ca)) = (step.output_tokens, &step.created_at) {
            if out > 0 {
                if let Some(sec) = parse_rfc3339_secs(ca) {
                    let idx = step.step_index.map(|i| (i + 1) as u32).unwrap_or(0);
                    steps_with_out.push((sec, out, idx));
                }
            }
        }
    }

    let mut tps_tuples = Vec::new();
    let mut last_dur = 0.0f32;
    if steps_with_out.len() >= 2 {
        for w in steps_with_out.windows(2) {
            let t0 = w[0].0;
            let t1 = w[1].0;
            let out = w[1].1;
            let idx = w[1].2;
            let dur = (t1 - t0).clamp(1, 60) as f32;
            let tps = out as f32 / dur;
            tps_tuples.push((tps, out, dur, idx));
            last_dur = dur;
        }
    } else if let Some(&(_, out, idx)) = steps_with_out.first() {
        let dur = 2.0f32;
        let tps = out as f32 / dur;
        tps_tuples.push((tps, out, dur, idx));
        last_dur = dur;
    }

    let round_tps = tps_tuples.last().map(|t| t.0).unwrap_or(0.0);
    let mut recent_tps = [0.0f32; 16];
    let mut recent_turn_tokens = [0u64; 16];
    let mut recent_turn_durations = [0.0f32; 16];
    let mut recent_turn_indices = [0u32; 16];

    let take_n = tps_tuples.len().min(16);
    let start_idx = 16 - take_n;
    for (i, &(tps, out, dur, idx)) in tps_tuples.iter().rev().take(take_n).collect::<Vec<_>>().into_iter().rev().enumerate() {
        recent_tps[start_idx + i] = tps;
        recent_turn_tokens[start_idx + i] = out;
        recent_turn_durations[start_idx + i] = dur;
        recent_turn_indices[start_idx + i] = idx;
    }

    let mut peak_tps: f32 = 0.0;
    let mut sum_tps: f32 = 0.0;
    let mut count_tps: usize = 0;
    for &val in &recent_tps {
        if val > 0.0 {
            if val > peak_tps {
                peak_tps = val;
            }
            sum_tps += val;
            count_tps += 1;
        }
    }
    let avg_tps = if count_tps > 0 {
        sum_tps / count_tps as f32
    } else {
        0.0
    };

    TailMetrics {
        input_tokens: found_input,
        cache_read_tokens: found_cache,
        output_tokens: found_output,
        context_tokens: total_context,
        cache_rate,
        is_running,
        round_duration_secs: last_dur,
        round_tps,
        peak_tps,
        avg_tps,
        recent_tps,
        recent_turn_tokens,
        recent_turn_durations,
        recent_turn_indices,
        file_step_count,
    }
}

fn load_project_map(projects_dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(entries) = fs::read_dir(projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(cfg) = serde_json::from_str::<ProjectConfigFile>(&content)
                && !cfg.id.is_empty()
                && !cfg.name.is_empty()
            {
                map.insert(cfg.id, cfg.name);
            }
        }
    }
    map
}

struct DbMeta {
    title: String,
    project_id: String,
    workspace_uris: String,
    status: String,
    not_fully_idle: bool,
    step_count: u32,
}

fn load_db_sessions(db_path: &Path) -> HashMap<String, DbMeta> {
    let mut map = HashMap::new();
    if !db_path.exists() {
        return map;
    }
    let db_str = db_path.to_string_lossy().to_string();
    let Ok(c_path) = CString::new(db_str) else { return map };

    let Some(funcs) = get_sqlite_funcs() else {
        return map;
    };

    unsafe {
        let mut db: *mut c_void = std::ptr::null_mut();
        let rc = (funcs.open)(c_path.as_ptr(), &mut db, 1, std::ptr::null());
        if rc == 0 {
            let query = c"SELECT conversation_id, title, project_id, workspace_uris, status, not_fully_idle, step_count FROM conversation_summaries";
            let mut stmt: *mut c_void = std::ptr::null_mut();
            if (funcs.prep)(db, query.as_ptr(), -1, &mut stmt, std::ptr::null_mut()) == 0 {
                while (funcs.step)(stmt) == 100 {
                    let id_p = (funcs.col)(stmt, 0);
                    let title_p = (funcs.col)(stmt, 1);
                    let proj_p = (funcs.col)(stmt, 2);
                    let ws_p = (funcs.col)(stmt, 3);
                    let status_p = (funcs.col)(stmt, 4);

                    let id = if !id_p.is_null() { CStr::from_ptr(id_p as *const c_char).to_string_lossy().to_string() } else { String::new() };
                    let title = if !title_p.is_null() { CStr::from_ptr(title_p as *const c_char).to_string_lossy().to_string() } else { String::new() };
                    let project_id = if !proj_p.is_null() { CStr::from_ptr(proj_p as *const c_char).to_string_lossy().to_string() } else { String::new() };
                    let workspace_uris = if !ws_p.is_null() { CStr::from_ptr(ws_p as *const c_char).to_string_lossy().to_string() } else { String::new() };
                    let status = if !status_p.is_null() { CStr::from_ptr(status_p as *const c_char).to_string_lossy().to_string() } else { String::new() };
                    let not_fully_idle = (funcs.col_int)(stmt, 5) != 0;
                    let step_count = (funcs.col_int)(stmt, 6).max(0) as u32;

                    if !id.is_empty() {
                        map.insert(id, DbMeta { title, project_id, workspace_uris, status, not_fully_idle, step_count });
                    }
                }
                (funcs.fin)(stmt);
            }
        }
        if !db.is_null() {
            (funcs.close)(db);
        }
    }
    map
}

fn extract_folder_from_workspace_uris(uris_json: &str) -> Option<String> {
    if uris_json.trim().is_empty() {
        return None;
    }
    let arr = serde_json::from_str::<Vec<String>>(uris_json).ok()?;
    let first = arr.first()?;
    let path_str = first.trim_end_matches('/');
    let last_seg = path_str.rsplit('/').next()?;
    if last_seg.is_empty() {
        return None;
    }
    let decoded = urlencoding_decode(last_seg);
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() {
            let h1 = chars[i + 1].to_digit(16);
            let h2 = chars[i + 2].to_digit(16);
            if let (Some(d1), Some(d2)) = (h1, h2) {
                bytes.push(((d1 << 4) | d2) as u8);
                i += 3;
                continue;
            }
        }
        let mut b = [0u8; 4];
        let sub = chars[i].encode_utf8(&mut b);
        bytes.extend_from_slice(sub.as_bytes());
        i += 1;
    }
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn scan_sandbox_sessions(sandbox_home: &Path, slot: u8) -> InstanceSessions {
    let gemini_dir = sandbox_home.join(".gemini");
    let brain_dir = gemini_dir.join("antigravity").join("brain");
    let projects_dir = gemini_dir.join("config").join("projects");
    let db_path = gemini_dir.join("antigravity").join("conversation_summaries.db");

    if !brain_dir.is_dir() {
        return InstanceSessions {
            slot,
            groups: Vec::new(),
            total_sessions: 0,
            summary_text: "无活动会话".to_string(),
            has_running: false,
        };
    }

    let project_map = load_project_map(&projects_dir);
    let db_sessions = load_db_sessions(&db_path);

    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&brain_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name == "tempmediaStorage" {
                continue;
            }

            let log_file = path.join(".system_generated").join("logs").join("transcript.jsonl");
            if log_file.is_file()
                && let Ok(meta) = fs::metadata(&log_file)
                && let Ok(mtime) = meta.modified()
            {
                entries.push((mtime, meta.len(), dir_name, log_file));
            }
        }
    }

    entries.sort_by_key(|a| std::cmp::Reverse(a.0));

    let now = SystemTime::now();
    let mut has_running = false;
    let mut all_sessions = Vec::new();

    for (mtime, len, uuid, log_path) in entries {
        let secs_ago = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);

        let db_meta = db_sessions.get(&uuid);
        let raw_title = db_meta.and_then(|m| if !m.title.trim().is_empty() { Some(m.title.clone()) } else { None });
        let title = raw_title.unwrap_or_else(|| {
            parse_title_from_log(&log_path).unwrap_or_else(|| {
                let short = if uuid.len() >= 8 { &uuid[..8] } else { &uuid };
                format!("会话-{short}")
            })
        });

        let project_name = if let Some(m) = db_meta {
            if let Some(pname) = project_map.get(&m.project_id) {
                pname.clone()
            } else if let Some(folder) = extract_folder_from_workspace_uris(&m.workspace_uris) {
                folder
            } else if m.project_id == "outside-of-project" {
                "其他会话".to_string()
            } else if !m.project_id.is_empty() {
                m.project_id.clone()
            } else {
                "其他会话".to_string()
            }
        } else {
            "其他会话".to_string()
        };

        let tail = get_or_parse_tail_metrics(&uuid, &log_path, len, mtime);

        let is_running_db = if let Some(m) = db_meta {
            (m.status == "CASCADE_RUN_STATUS_RUNNING" || m.not_fully_idle) && secs_ago <= 60
        } else {
            false
        };
        let is_running = (tail.is_running && secs_ago <= 30) || is_running_db;
        if is_running {
            has_running = true;
        }

        let context_str = format_tokens(tail.context_tokens);
        let cache_rate_str = format!("{:.1}%", tail.cache_rate * 100.0);
        let time_ago_str = format_time_ago(secs_ago);
        let step_count = if let Some(m) = db_meta && m.step_count > 0 {
            m.step_count
        } else {
            tail.file_step_count
        };

        all_sessions.push(SessionItem {
            id: uuid,
            title,
            project_name,
            input_tokens: tail.input_tokens,
            cache_read_tokens: tail.cache_read_tokens,
            output_tokens: tail.output_tokens,
            context_tokens: tail.context_tokens,
            cache_rate: tail.cache_rate,
            is_running,
            last_updated_secs_ago: secs_ago,
            context_str,
            cache_rate_str,
            time_ago_str,
            step_count,
            round_duration_secs: tail.round_duration_secs,
            round_tps: tail.round_tps,
            peak_tps: tail.peak_tps,
            avg_tps: tail.avg_tps,
            recent_tps: tail.recent_tps,
            recent_turn_tokens: tail.recent_turn_tokens,
            recent_turn_durations: tail.recent_turn_durations,
            recent_turn_indices: tail.recent_turn_indices,
        });
    }

    let total_sessions = all_sessions.len();

    let mut group_map: HashMap<String, Vec<SessionItem>> = HashMap::new();
    let mut group_order: Vec<String> = Vec::new();

    for item in all_sessions {
        if !group_map.contains_key(&item.project_name) {
            group_order.push(item.project_name.clone());
        }
        group_map.entry(item.project_name.clone()).or_default().push(item);
    }

    let mut groups = Vec::new();
    for name in group_order {
        if let Some(mut list) = group_map.remove(&name) {
            list.sort_by_key(|s| s.last_updated_secs_ago);
            let grp_running = list.iter().any(|s| s.is_running);
            let min_secs = list.iter().map(|s| s.last_updated_secs_ago).min().unwrap_or(u64::MAX);
            groups.push(SessionGroup {
                project_name: name,
                sessions: list,
                has_running: grp_running,
                latest_secs_ago: min_secs,
            });
        }
    }

    groups.sort_by(|a, b| {
        if a.has_running != b.has_running {
            return b.has_running.cmp(&a.has_running);
        }
        a.latest_secs_ago.cmp(&b.latest_secs_ago)
    });

    let summary_text = if total_sessions == 0 {
        "无活动会话".to_string()
    } else {
        let first_session = groups.first().and_then(|g| g.sessions.first());
        if let Some(s) = first_session {
            if has_running {
                format!("运行中 · 最新 {} · {}", s.context_str, s.cache_rate_str)
            } else {
                format!("{}个会话 · 最新 {} · {}", total_sessions, s.context_str, s.cache_rate_str)
            }
        } else {
            format!("{total_sessions}个会话")
        }
    };

    InstanceSessions {
        slot,
        groups,
        total_sessions,
        summary_text,
        has_running,
    }
}

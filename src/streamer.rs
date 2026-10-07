#![allow(dead_code)]

use std::collections::VecDeque;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSource {
    LanguageServer,
    SubLanguageServer,
    ProxyLog,
    CockpitSystem,
    MainApp,
}

impl StreamSource {
    pub fn next(&self) -> Self {
        match self {
            Self::CockpitSystem => Self::LanguageServer,
            Self::LanguageServer => Self::SubLanguageServer,
            Self::SubLanguageServer => Self::ProxyLog,
            Self::ProxyLog => Self::MainApp,
            Self::MainApp => Self::CockpitSystem,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CockpitSystem => "运行记录",
            Self::LanguageServer => "主号日志",
            Self::SubLanguageServer => "分身日志",
            Self::ProxyLog => "代理日志",
            Self::MainApp => "客户端日志",
        }
    }
}


#[derive(Debug, Clone)]
pub struct ParsedLogLine {
    pub line_no: String,
    pub time_str: String,
    pub level: String,
    pub level_color_rgb: (u8, u8, u8),
    pub tag: String,
    pub tag_color_rgb: (u8, u8, u8),
    pub text: String,
    pub text_color_rgb: (u8, u8, u8),
}

pub struct LogStreamer {
    pub current_source: StreamSource,
    last_offset: u64,
    last_path: Option<PathBuf>,
    pub buffer: VecDeque<String>,
    pub system_logs: VecDeque<String>,
    max_lines: usize,
}

impl LogStreamer {
    pub fn new() -> Self {
        let mut streamer = Self {
            current_source: StreamSource::CockpitSystem,
            last_offset: 0,
            last_path: None,
            buffer: VecDeque::with_capacity(300),
            system_logs: VecDeque::with_capacity(300),
            max_lines: 250,
        };
        streamer.init_source();
        streamer
    }

    pub fn cycle_source(&mut self) -> StreamSource {
        self.current_source = self.current_source.next();
        self.init_source();
        self.current_source
    }

    pub fn set_source(&mut self, source: StreamSource) {
        self.current_source = source;
        self.init_source();
    }

    pub fn get_source_index(&self) -> i32 {
        match self.current_source {
            StreamSource::CockpitSystem => 0,
            StreamSource::LanguageServer => 1,
            StreamSource::SubLanguageServer => 2,
            StreamSource::ProxyLog => 3,
            StreamSource::MainApp => 4,
        }
    }

    pub fn from_source_index(idx: i32) -> StreamSource {
        match idx {
            0 => StreamSource::CockpitSystem,
            1 => StreamSource::LanguageServer,
            2 => StreamSource::SubLanguageServer,
            3 => StreamSource::ProxyLog,
            _ => StreamSource::MainApp,
        }
    }

    pub fn clear_current(&mut self) {
        self.buffer.clear();
        if self.current_source == StreamSource::CockpitSystem {
            self.system_logs.clear();
        }
    }

    pub fn resolve_path(&self, source: StreamSource) -> Option<PathBuf> {
        let app_data = env::var("APPDATA").ok();
        let local_app_data = env::var("LOCALAPPDATA").ok();

        match source {
            StreamSource::LanguageServer => {
                let app_data = app_data?;
                let p = PathBuf::from(&app_data).join("Antigravity/logs/language_server.log");
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            }
            StreamSource::SubLanguageServer => {
                let sub_logs = Self::resolve_sub_logs_dir()?;
                let p = sub_logs.join("language_server.log");
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            }
            StreamSource::MainApp => {
                let app_data = app_data?;
                let p = PathBuf::from(&app_data).join("Antigravity/logs/main.log");
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            }
            StreamSource::ProxyLog => {
                if let Some(local) = local_app_data {
                    let dir1 = PathBuf::from(&local).join("Programs/antigravity/logs");
                    if let Some(latest) = Self::find_latest_log_in_dir(&dir1) {
                        return Some(latest);
                    }
                }
                let temp_dir = env::temp_dir().join("antigravity-proxy-logs");
                if let Some(latest) = Self::find_latest_log_in_dir(&temp_dir) {
                    return Some(latest);
                }
                let temp_file = env::temp_dir().join("antigravity-proxy.log");
                if temp_file.exists() {
                    return Some(temp_file);
                }
                None
            }
            StreamSource::CockpitSystem => None,
        }
    }

    fn resolve_sub_logs_dir() -> Option<PathBuf> {
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let p = exe_dir.join("data/instance_2/AppData/Roaming/Antigravity/logs");
                if p.exists() {
                    return Some(p);
                }
                if let Some(parent) = exe_dir.parent() {
                    let p = parent.join("data/instance_2/AppData/Roaming/Antigravity/logs");
                    if p.exists() {
                        return Some(p);
                    }
                    if let Some(grandparent) = parent.parent() {
                        let p = grandparent.join("data/instance_2/AppData/Roaming/Antigravity/logs");
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
        let cwd_p = PathBuf::from("data/instance_2/AppData/Roaming/Antigravity/logs");
        if cwd_p.exists() {
            return Some(cwd_p);
        }
        None
    }

    fn find_latest_log_in_dir(dir: &PathBuf) -> Option<PathBuf> {
        if !dir.exists() {
            return None;
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.extension().map_or(false, |ext| ext == "log"))
            .collect();
        entries.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
        entries.pop()
    }

    pub fn init_source(&mut self) {
        self.buffer.clear();
        self.last_offset = 0;

        if self.current_source == StreamSource::CockpitSystem {
            for line in &self.system_logs {
                self.buffer.push_back(line.clone());
            }
            return;
        }

        let Some(path) = self.resolve_path(self.current_source) else {
            let name = self.current_source.display_name();
            self.buffer.push_back(format!("[管道] 暂未检测到 {name} 的活动日志文件，等待写入...\n"));
            self.last_path = None;
            return;
        };

        self.last_path = Some(path.clone());

        if let Ok(mut file) = File::open(&path) {
            if let Ok(meta) = file.metadata() {
                let len = meta.len();
                let read_start = len.saturating_sub(32768);
                if file.seek(SeekFrom::Start(read_start)).is_ok() {
                    let mut content = Vec::new();
                    if file.read_to_end(&mut content).is_ok() {
                        let text = String::from_utf8_lossy(&content);
                        let mut lines: Vec<&str> = text.lines().collect();
                        if read_start > 0 && !lines.is_empty() {
                            lines.remove(0);
                        }
                        let take_count = lines.len().min(80);
                        for line in &lines[lines.len() - take_count..] {
                            if !line.trim().is_empty() {
                                self.buffer.push_back(format!("{line}\n"));
                            }
                        }
                    }
                }
                self.last_offset = len;
            }
        }
    }

    pub fn poll_updates(&mut self) -> bool {
        if self.current_source == StreamSource::CockpitSystem {
            return false;
        }

        let path = match self.resolve_path(self.current_source) {
            Some(p) => p,
            None => {
                if self.last_path.is_some() {
                    self.init_source();
                    return true;
                }
                return false;
            }
        };

        if self.last_path.as_ref() != Some(&path) {
            self.init_source();
            return true;
        }

        let Ok(mut file) = File::open(&path) else {
            return false;
        };
        let Ok(meta) = file.metadata() else {
            return false;
        };
        let len = meta.len();

        if len < self.last_offset {
            self.init_source();
            return true;
        }

        if len == self.last_offset {
            return false;
        }

        if file.seek(SeekFrom::Start(self.last_offset)).is_err() {
            return false;
        }

        let bytes_to_read = (len - self.last_offset).min(65536) as usize;
        let mut chunk = vec![0u8; bytes_to_read];
        if file.read_exact(&mut chunk).is_err() {
            return false;
        }

        let valid_bytes = if let Some(last_nl) = chunk.iter().rposition(|&b| b == b'\n') {
            last_nl + 1
        } else if bytes_to_read >= 65536 {
            bytes_to_read
        } else {
            return false;
        };

        self.last_offset += valid_bytes as u64;

        let text = String::from_utf8_lossy(&chunk[..valid_bytes]);
        let mut has_new = false;
        for line in text.lines() {
            if !line.trim().is_empty() {
                if self.buffer.len() >= self.max_lines {
                    self.buffer.pop_front();
                }
                self.buffer.push_back(format!("{line}\n"));
                has_new = true;
            }
        }

        has_new
    }

    pub fn add_system_log(&mut self, tag: &str, msg: &str) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let formatted = format!("[{time}] [{tag}] {msg}\n");

        if self.system_logs.len() >= self.max_lines {
            self.system_logs.pop_front();
        }
        self.system_logs.push_back(formatted.clone());

        if self.current_source == StreamSource::CockpitSystem {
            if self.buffer.len() >= self.max_lines {
                self.buffer.pop_front();
            }
            self.buffer.push_back(formatted);
        }
    }

    pub fn get_parsed_lines(&self) -> Vec<ParsedLogLine> {
        let take_count = self.buffer.len().min(50);
        let start = self.buffer.len() - take_count;
        self.buffer
            .iter()
            .skip(start)
            .enumerate()
            .map(|(idx, line)| Self::parse_line(idx + 1, line, self.current_source))
            .collect()
    }

    pub fn parse_line(idx: usize, raw: &str, _source: StreamSource) -> ParsedLogLine {
        let line = raw.trim();
        let line_no = format!("{:02}", idx);

        if line.starts_with("[管道]") {
            return ParsedLogLine {
                line_no,
                time_str: "--:--:--".into(),
                level: "PIPE".into(),
                level_color_rgb: (129, 140, 248),
                tag: "[Pipeline]".into(),
                tag_color_rgb: (148, 163, 184),
                text: line.trim_start_matches("[管道]").trim().into(),
                text_color_rgb: (148, 163, 184),
            };
        }

        let clean = if let Some(stripped) = line.strip_prefix("ERROR: logging before google.Init: ") {
            stripped.trim()
        } else {
            line
        };

        if (clean.starts_with('I') || clean.starts_with('W') || clean.starts_with('E'))
            && clean.len() > 20
            && clean.get(1..5).map_or(false, |s| s.chars().all(|c| c.is_ascii_digit()))
            && clean.as_bytes().get(5) == Some(&b' ')
        {
            let level_char = clean.chars().next().unwrap();
            let (level, level_color) = match level_char {
                'I' => ("INFO", (56, 189, 248)),
                'W' => ("WARN", (251, 191, 36)),
                'E' => ("ERROR", (244, 63, 94)),
                _ => ("LOG", (148, 163, 184)),
            };

            let time_part = clean.get(6..14).unwrap_or("");
            let time_str = if time_part.contains(':') {
                time_part.to_string()
            } else {
                "--:--:--".to_string()
            };

            let after_bracket = if let Some(pos) = clean.find(']') {
                clean[pos + 1..].trim()
            } else {
                clean
            };

            if after_bracket.contains("URL: https://") {
                let tag = if after_bracket.contains("streamGenerateContent") {
                    "[streamGenerateContent]"
                } else if after_bracket.contains("GenerateContent") {
                    "[GenerateContent]"
                } else {
                    "[API Call]"
                };

                let text = if let Some(trace_pos) = after_bracket.find("Trace:") {
                    &after_bracket[trace_pos..]
                } else {
                    after_bracket
                };

                return ParsedLogLine {
                    line_no,
                    time_str,
                    level: "SSE".into(),
                    level_color_rgb: (192, 132, 252),
                    tag: tag.into(),
                    tag_color_rgb: (34, 211, 238),
                    text: text.to_string(),
                    text_color_rgb: (226, 232, 240),
                };
            }

            if after_bracket.starts_with('[') {
                if let Some(tag_end) = after_bracket.find(']') {
                    let tag = &after_bracket[..=tag_end];
                    let text = after_bracket[tag_end + 1..].trim();
                    return ParsedLogLine {
                        line_no,
                        time_str,
                        level: level.into(),
                        level_color_rgb: level_color,
                        tag: tag.to_string(),
                        tag_color_rgb: (34, 211, 238),
                        text: text.to_string(),
                        text_color_rgb: (226, 232, 240),
                    };
                }
            }

            return ParsedLogLine {
                line_no,
                time_str,
                level: level.into(),
                level_color_rgb: level_color,
                tag: "".into(),
                tag_color_rgb: (148, 163, 184),
                text: after_bracket.to_string(),
                text_color_rgb: if level == "ERROR" {
                    (254, 205, 211)
                } else if level == "WARN" {
                    (254, 240, 138)
                } else {
                    (226, 232, 240)
                },
            };
        }

        if clean.starts_with('[') {
            if let Some(first_end) = clean.find(']') {
                let first_token = &clean[1..first_end];
                let remainder = clean[first_end + 1..].trim();

                let (time_str, rest) = if first_token.contains(':') {
                    let time = if first_token.len() >= 8 && first_token.contains(' ') {
                        first_token.split(' ').nth(1).unwrap_or(first_token)
                    } else {
                        first_token
                    };
                    let short_time = time.get(..8).unwrap_or(time);
                    (short_time.to_string(), remainder)
                } else {
                    ("--:--:--".to_string(), clean)
                };

                if rest.starts_with('[') {
                    if let Some(second_end) = rest.find(']') {
                        let second_token = &rest[1..second_end];
                        let msg = rest[second_end + 1..].trim();

                        let (level, level_color) = match second_token.to_uppercase().as_str() {
                            "SYSTEM" => ("SYS", (52, 211, 153)),
                            "SPAWN" | "WIN32" => ("EXEC", (56, 189, 248)),
                            "PROXY" => ("NET", (34, 211, 238)),
                            "AUTH" => ("AUTH", (251, 191, 36)),
                            "KILL" => ("STOP", (244, 63, 94)),
                            "ERROR" => ("ERR", (244, 63, 94)),
                            "WARN" => ("WARN", (251, 191, 36)),
                            "INFO" => ("INFO", (56, 189, 248)),
                            "SANDBOX" => ("BOX", (192, 132, 252)),
                            _ => (second_token, (148, 163, 184)),
                        };

                        return ParsedLogLine {
                            line_no,
                            time_str,
                            level: level.into(),
                            level_color_rgb: level_color,
                            tag: format!("[{second_token}]"),
                            tag_color_rgb: level_color,
                            text: msg.to_string(),
                            text_color_rgb: (226, 232, 240),
                        };
                    }
                }
            }
        }

        ParsedLogLine {
            line_no,
            time_str: "--:--:--".into(),
            level: "LOG".into(),
            level_color_rgb: (148, 163, 184),
            tag: "".into(),
            tag_color_rgb: (148, 163, 184),
            text: clean.to_string(),
            text_color_rgb: (203, 213, 225),
        }
    }
}

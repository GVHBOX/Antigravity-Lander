use std::collections::HashMap;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Clone)]
pub struct ProcessStats {
    pub is_running: bool,
    pub electron_pid: Option<u32>,
    pub cpu_usage: f32,
    pub memory_rss_mb: f64,
    pub ls_pid: Option<u32>,
    pub ls_port: Option<u16>,
    pub ls_ports: Vec<u16>,
    pub ls_csrf: Option<String>,
}

impl Default for ProcessStats {
    fn default() -> Self {
        Self {
            is_running: false,
            electron_pid: None,
            cpu_usage: 0.0,
            memory_rss_mb: 0.0,
            ls_pid: None,
            ls_port: None,
            ls_ports: Vec::new(),
            ls_csrf: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct MonitorView {
    pub host: ProcessStats,
    pub subs: [ProcessStats; 4],
    pub total_memory_mb: f64,
    pub system_total_ram_mb: f64,
    pub host_uptime_secs: Option<u64>,
}

pub struct TelemetryMonitor {
    sys: System,
    pub host: ProcessStats,
    pub subs: [ProcessStats; 4],
    pub total_memory_mb: f64,
    pub system_total_ram_mb: f64,
    cached_host_ls_pid: Option<u32>,
    cached_host_port: Option<u16>,
    cached_host_port_list: Vec<u16>,
    cached_host_csrf: Option<String>,
    host_port_retry_count: u32,
    cached_sub_ls_pids: [Option<u32>; 4],
    cached_sub_ports: [Option<u16>; 4],
    cached_sub_port_lists: [Vec<u16>; 4],
    cached_sub_csrfs: [Option<String>; 4],
    sub_port_retry_counts: [u32; 4],
}

fn extract_csrf_token(cmd: &[std::ffi::OsString]) -> Option<String> {
    let full = cmd
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    if let Some(pos) = full.find("--csrf_token") {
        let rest = full[pos + "--csrf_token".len()..].trim_start_matches([' ', '=']);
        let token = rest.split_whitespace().next()?.trim_matches('"');
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    None
}

fn is_descendant_of(sys: &System, mut current_pid: sysinfo::Pid, target_pid: u32) -> bool {
    for _ in 0..6 {
        if let Some(proc) = sys.process(current_pid) {
            if let Some(parent) = proc.parent() {
                if parent.as_u32() == target_pid {
                    return true;
                }
                current_pid = parent;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    false
}

pub(crate) fn is_sidecar_or_node_process(cmd_line: &str) -> bool {
    cmd_line.contains("--import=")
        || cmd_line.contains("register_loader.js")
        || cmd_line.contains("main.mjs")
        || cmd_line.contains("ELECTRON_RUN_AS_NODE")
        || cmd_line.contains(".mjs")
        || cmd_line.contains("sidecar_sdk")
        || cmd_line.contains("agy-node")
}

pub(crate) fn extract_user_data_dir(cmd_line: &str) -> Option<String> {
    let marker = "--user-data-dir=";
    let pos = cmd_line.find(marker)?;
    let rest = &cmd_line[pos + marker.len()..];
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest.find('"').or_else(|| rest.find(' ')).unwrap_or(rest.len());
    let dir = &rest[..end];
    (!dir.is_empty()).then(|| dir.to_string())
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

fn has_instance_segment(path: &str, tag: &str) -> bool {
    let norm = normalize_path(path);
    norm.ends_with(&format!("\\data\\{}", tag)) || norm.contains(&format!("\\data\\{}\\", tag))
}

pub(crate) fn matches_instance_user_data_dir(cmd_line: &str, slot: usize) -> bool {
    let tag = format!("instance_{}", slot + 1);
    extract_user_data_dir(cmd_line)
        .map(|dir| has_instance_segment(&dir, &tag))
        .unwrap_or(false)
}

pub(crate) fn matches_instance_slot(cmd_line: &str, slot: usize) -> bool {
    let tag = format!("instance_{}", slot + 1);
    match extract_user_data_dir(cmd_line) {
        Some(dir) => has_instance_segment(&dir, &tag),
        None => has_instance_segment(cmd_line, &tag),
    }
}


impl TelemetryMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet);
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);
        let total_ram = (sys.total_memory() as f64) / (1024.0 * 1024.0);
        let system_total_ram_mb = if total_ram > 0.0 { total_ram } else { 32768.0 };
        Self {
            sys,
            host: ProcessStats::default(),
            subs: [
                ProcessStats::default(),
                ProcessStats::default(),
                ProcessStats::default(),
                ProcessStats::default(),
            ],
            total_memory_mb: 0.0,
            system_total_ram_mb,
            cached_host_ls_pid: None,
            cached_host_port: None,
            cached_host_port_list: Vec::new(),
            cached_host_csrf: None,
            host_port_retry_count: 0,
            cached_sub_ls_pids: [None; 4],
            cached_sub_ports: [None; 4],
            cached_sub_port_lists: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            cached_sub_csrfs: [None, None, None, None],
            sub_port_retry_counts: [0; 4],
        }
    }


    pub fn host_uptime_seconds(&self) -> Option<u64> {
        let pid = self.host.electron_pid?;
        self.sys.process(sysinfo::Pid::from_u32(pid)).map(|p| p.run_time())
    }


    pub fn view(&self) -> MonitorView {
        MonitorView {
            host: self.host.clone(),
            subs: self.subs.clone(),
            total_memory_mb: self.total_memory_mb,
            system_total_ram_mb: self.system_total_ram_mb,
            host_uptime_secs: self.host_uptime_seconds(),
        }
    }


    pub fn apply_resolved_port(&mut self, ls_pid: u32, port: u16) {
        if self.cached_host_ls_pid == Some(ls_pid) {
            self.cached_host_port = Some(port);
            self.host.ls_port = Some(port);
        }
        for slot in 0..4 {
            if self.cached_sub_ls_pids[slot] == Some(ls_pid) {
                self.cached_sub_ports[slot] = Some(port);
                self.subs[slot].ls_port = Some(port);
            }
        }
    }


    pub fn refresh(&mut self, known_sub_pids: [Option<u32>; 4]) {
        self.sys.refresh_memory();
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet);
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);

        let self_pid = std::process::id();
        let mut host_electron_pid = None;
        let mut sub_main_pids = [None; 4];

        for (pid, proc) in self.sys.processes() {
            let pid_u32 = pid.as_u32();
            if pid_u32 == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            let is_antigravity = (name == "antigravity.exe" || name == "antigravity")
                && !name.contains("multi")
                && !name.contains("lander")
                && !name.contains("cockpit");
            if !is_antigravity {
                continue;
            }

            let cmd_line = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");

            let is_sidecar = is_sidecar_or_node_process(&cmd_line);
            let is_main_electron = !cmd_line.contains("--type=") && !is_sidecar;

            let mut matched_slot = None;
            for (slot, &known_pid) in known_sub_pids.iter().enumerate() {
                if matches_instance_user_data_dir(&cmd_line, slot)
                    || (known_pid == Some(pid_u32) && is_main_electron && cmd_line.contains("--user-data-dir="))
                {
                    matched_slot = Some(slot);
                    break;
                }
            }

            if let Some(slot) = matched_slot {
                if is_main_electron && sub_main_pids[slot].is_none() {
                    sub_main_pids[slot] = Some(pid_u32);
                }
            } else if is_main_electron && host_electron_pid.is_none() {
                let has_user_data_dir = cmd_line.contains("--user-data-dir=");
                let is_any_known_sub = known_sub_pids.contains(&Some(pid_u32));
                if !has_user_data_dir && !is_any_known_sub {
                    host_electron_pid = Some(pid_u32);
                }
            }

        }

        let active_sub_pids = sub_main_pids;

        let sub_electron_pids = active_sub_pids;
        let mut sub_ls_pids = [None, None, None, None];
        let mut sub_ls_csrfs = [None, None, None, None];
        let mut sub_cpus = [0.0f32, 0.0f32, 0.0f32, 0.0f32];
        let mut sub_mem_bytes = [0u64, 0u64, 0u64, 0u64];

        let mut host_ls_pid = None;
        let mut host_ls_csrf = None;
        let mut host_cpu = 0.0f32;
        let mut host_mem_bytes = 0u64;

        for (pid, proc) in self.sys.processes() {
            let pid_u32 = pid.as_u32();
            if pid_u32 == self_pid {
                continue;
            }
            let name = proc.name().to_string_lossy().to_lowercase();
            let is_antigravity = (name == "antigravity.exe" || name == "antigravity")
                && !name.contains("multi")
                && !name.contains("lander")
                && !name.contains("cockpit");
            let is_ls = name == "language_server.exe" || name == "language_server" || name.contains("jetski");

            if !is_antigravity && !is_ls {
                continue;
            }

            let cmd_line = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");

            let mut matched_slot = None;
            for (slot, &active_pid) in active_sub_pids.iter().enumerate() {
                let is_slot_main = active_pid == Some(pid_u32);
                let is_slot_descendant = active_pid
                    .map(|main_pid| is_descendant_of(&self.sys, *pid, main_pid))
                    .unwrap_or(false);
                let is_slot_tag = active_pid.is_some() && matches_instance_slot(&cmd_line, slot);

                if is_slot_main || is_slot_descendant || is_slot_tag {
                    matched_slot = Some(slot);
                    break;
                }

            }

            if let Some(slot) = matched_slot {
                if is_ls && sub_ls_pids[slot].is_none() {
                    sub_ls_pids[slot] = Some(pid_u32);
                    sub_ls_csrfs[slot] = extract_csrf_token(proc.cmd());
                }
                sub_cpus[slot] += proc.cpu_usage();
                sub_mem_bytes[slot] += proc.memory();
            } else {
                let is_host_main = host_electron_pid == Some(pid_u32);
                let is_host_descendant = host_electron_pid
                    .map(|main_pid| is_descendant_of(&self.sys, *pid, main_pid))
                    .unwrap_or(false);

                if is_host_main || is_host_descendant {
                    if is_ls && host_ls_pid.is_none() {
                        host_ls_pid = Some(pid_u32);
                        host_ls_csrf = extract_csrf_token(proc.cmd());
                    }
                    host_cpu += proc.cpu_usage();
                    host_mem_bytes += proc.memory();
                }
            }
        }

        if host_ls_pid != self.cached_host_ls_pid {
            self.cached_host_ls_pid = host_ls_pid;
            self.cached_host_port = None;
            self.cached_host_port_list.clear();
            self.cached_host_csrf = host_ls_csrf;
            self.host_port_retry_count = 0;
        } else if host_ls_csrf.is_some() && self.cached_host_csrf.is_none() {
            self.cached_host_csrf = host_ls_csrf;
        }

        let mut pids_to_detect = Vec::new();

        if let Some(pid) = host_ls_pid {
            if self.cached_host_port.is_none() {
                self.host_port_retry_count = self.host_port_retry_count.wrapping_add(1);
                if self.host_port_retry_count <= 10 || self.host_port_retry_count.is_multiple_of(5) {
                    pids_to_detect.push(pid);
                }
            }
        } else {
            self.cached_host_port = None;
            self.cached_host_port_list.clear();
            self.cached_host_csrf = None;
            self.host_port_retry_count = 0;
        }

        for slot in 0..4 {
            let ls_pid = sub_ls_pids[slot];
            if ls_pid != self.cached_sub_ls_pids[slot] {
                self.cached_sub_ls_pids[slot] = ls_pid;
                self.cached_sub_ports[slot] = None;
                self.cached_sub_port_lists[slot].clear();
                self.cached_sub_csrfs[slot] = sub_ls_csrfs[slot].clone();
                self.sub_port_retry_counts[slot] = 0;
            } else if sub_ls_csrfs[slot].is_some() && self.cached_sub_csrfs[slot].is_none() {
                self.cached_sub_csrfs[slot] = sub_ls_csrfs[slot].clone();
            }

            if let Some(pid) = ls_pid {
                if self.cached_sub_ports[slot].is_none() {
                    self.sub_port_retry_counts[slot] = self.sub_port_retry_counts[slot].wrapping_add(1);
                    if self.sub_port_retry_counts[slot] <= 10 || self.sub_port_retry_counts[slot].is_multiple_of(5) {
                        pids_to_detect.push(pid);
                    }
                }
            } else {
                self.cached_sub_ports[slot] = None;
                self.cached_sub_port_lists[slot].clear();
                self.cached_sub_csrfs[slot] = None;
                self.sub_port_retry_counts[slot] = 0;
            }
        }

        if !pids_to_detect.is_empty() {
            let detected = detect_listening_ports(&pids_to_detect);
            if let Some(pid) = host_ls_pid
                && let Some(ports) = detected.get(&pid)
                && *ports != self.cached_host_port_list {
                    self.cached_host_port_list = ports.clone();
                    self.cached_host_port = None;
                }
            for (slot, &ls_pid_opt) in sub_ls_pids.iter().enumerate() {
                if let Some(pid) = ls_pid_opt
                    && let Some(ports) = detected.get(&pid)
                    && *ports != self.cached_sub_port_lists[slot] {
                        self.cached_sub_port_lists[slot] = ports.clone();
                        self.cached_sub_ports[slot] = None;
                    }
            }
        }

        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1) as f32;

        self.host = ProcessStats {
            is_running: host_electron_pid.is_some(),
            electron_pid: host_electron_pid,
            cpu_usage: (host_cpu / num_cpus).min(100.0),
            memory_rss_mb: (host_mem_bytes as f64) / (1024.0 * 1024.0),
            ls_pid: self.cached_host_ls_pid,
            ls_port: self.cached_host_port,
            ls_ports: self.cached_host_port_list.clone(),
            ls_csrf: self.cached_host_csrf.clone(),
        };

        for slot in 0..4 {
            self.subs[slot] = ProcessStats {
                is_running: sub_electron_pids[slot].is_some(),
                electron_pid: sub_electron_pids[slot],
                cpu_usage: (sub_cpus[slot] / num_cpus).min(100.0),
                memory_rss_mb: (sub_mem_bytes[slot] as f64) / (1024.0 * 1024.0),
                ls_pid: self.cached_sub_ls_pids[slot],
                ls_port: self.cached_sub_ports[slot],
                ls_ports: self.cached_sub_port_lists[slot].clone(),
                ls_csrf: self.cached_sub_csrfs[slot].clone(),
            };
        }

        self.total_memory_mb = self.host.memory_rss_mb
            + self.subs.iter().map(|s| s.memory_rss_mb).sum::<f64>();
        let total_ram = (self.sys.total_memory() as f64) / (1024.0 * 1024.0);
        if total_ram > 0.0 {
            self.system_total_ram_mb = total_ram;
        }
    }
}

fn detect_listening_ports(pids: &[u32]) -> HashMap<u32, Vec<u16>> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let mut result: HashMap<u32, Vec<u16>> = HashMap::new();
    if pids.is_empty() {
        return result;
    }

    let Ok(output) = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .creation_flags(0x08000000)
        .output() else {
        return result;
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 || !parts[0].eq_ignore_ascii_case("tcp") {
            continue;
        }
        if !parts.iter().any(|p| p.eq_ignore_ascii_case("listening")) {
            continue;
        }
        let Ok(target_pid) = parts[parts.len() - 1].parse::<u32>() else {
            continue;
        };
        if pids.contains(&target_pid)
            && let Some(port) = parts[1]
                .rfind(':')
                .and_then(|pos| parts[1][pos + 1..].parse::<u16>().ok())
        {
            let list = result.entry(target_pid).or_default();
            if !list.contains(&port) {
                list.push(port);
            }
        }
    }

    for ports in result.values_mut() {
        ports.sort_unstable();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_matching_requires_path_segment() {
        let workspace = r#""C:\Program Files\Antigravity\Antigravity.exe" --folder-uri file:///D:/Work/Antigravity-Lander/data/instance_10"#;
        assert!(!matches_instance_slot(workspace, 0));

        let sub_main = r#""C:\app\Antigravity.exe" --user-data-dir=D:\Work\Lander\data\instance_2\AppData\Roaming\Antigravity --force-device-scale-factor=1.40"#;
        assert!(matches_instance_slot(sub_main, 1));
        assert!(!matches_instance_slot(sub_main, 0));

        let sub_child = r#""C:\app\Antigravity.exe" --type=renderer --user-data-dir="D:\Work\Lander\data\instance_2\AppData\Roaming\Antigravity" --lang=zh-CN"#;
        assert!(matches_instance_user_data_dir(sub_child, 1));

        let sidecar = r#""C:\app\Antigravity.exe" --import=file:///D:/Work/Lander/data/instance_2/home/.gemini/x.js main.mjs"#;
        assert!(matches_instance_slot(sidecar, 1));
        assert!(!matches_instance_slot(sidecar, 0));

        let decoy = r#""C:\app\Antigravity.exe" --user-data-dir=D:\Work\Lander\data\instance_20\AppData"#;
        assert!(!matches_instance_slot(decoy, 1));

        let host = r#""C:\app\Antigravity.exe" --force-device-scale-factor=1.50"#;
        assert!(extract_user_data_dir(host).is_none());
        assert!(!matches_instance_slot(host, 0));
    }

    #[test]
    fn test_extract_csrf_token_from_command_line() {
        let cmd = vec![
            std::ffi::OsString::from("language_server.exe"),
            std::ffi::OsString::from("--csrf_token"),
            std::ffi::OsString::from("abc-123"),
        ];
        assert_eq!(extract_csrf_token(&cmd).as_deref(), Some("abc-123"));
        let cmd_eq = vec![
            std::ffi::OsString::from("language_server.exe"),
            std::ffi::OsString::from("--csrf_token=xyz"),
        ];
        assert_eq!(extract_csrf_token(&cmd_eq).as_deref(), Some("xyz"));
    }
}

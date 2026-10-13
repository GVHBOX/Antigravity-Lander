use std::collections::HashMap;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Clone)]
pub struct ProcessStats {
    pub is_running: bool,
    pub electron_pid: Option<u32>,
    pub ls_pid: Option<u32>,
    pub cpu_usage: f32,
    pub memory_rss_mb: f64,
    pub ls_port: Option<u16>,
    pub ls_csrf: Option<String>,
}

impl Default for ProcessStats {
    fn default() -> Self {
        Self {
            is_running: false,
            electron_pid: None,
            ls_pid: None,
            cpu_usage: 0.0,
            memory_rss_mb: 0.0,
            ls_port: None,
            ls_csrf: None,
        }
    }
}

pub struct TelemetryMonitor {
    sys: System,
    pub host: ProcessStats,
    pub sub: ProcessStats,
    pub subs: [ProcessStats; 4],
    pub total_memory_mb: f64,
    pub system_total_ram_mb: f64,
    cached_host_ls_pid: Option<u32>,
    cached_host_port: Option<u16>,
    cached_host_csrf: Option<String>,
    host_port_retry_count: u32,
    cached_sub_ls_pids: [Option<u32>; 4],
    cached_sub_ports: [Option<u16>; 4],
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
        let rest = full[pos + "--csrf_token".len()..].trim_start_matches(|c| c == ' ' || c == '=');
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

fn matches_instance_slot(cmd_line: &str, slot: usize) -> bool {
    let tag = format!("instance_{}", slot + 1);
    let win_data = format!("data\\{}", tag);
    let unix_data = format!("data/{}", tag);
    (cmd_line.contains("--user-data-dir=") && cmd_line.contains(&tag))
        || cmd_line.contains(&win_data)
        || cmd_line.contains(&unix_data)
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
            sub: ProcessStats::default(),
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
            cached_host_csrf: None,
            host_port_retry_count: 0,
            cached_sub_ls_pids: [None; 4],
            cached_sub_ports: [None; 4],
            cached_sub_csrfs: [None, None, None, None],
            sub_port_retry_counts: [0; 4],
        }
    }

    pub fn is_pid_alive(&self, pid: u32) -> bool {
        self.sys.process(sysinfo::Pid::from_u32(pid)).is_some()
    }

    pub fn process_memory_mb(&self, pid: u32) -> Option<f64> {
        self.sys.process(sysinfo::Pid::from_u32(pid)).map(|p| (p.memory() as f64) / (1024.0 * 1024.0))
    }

    pub fn host_uptime_seconds(&self) -> Option<u64> {
        let pid = self.host.electron_pid?;
        self.sys.process(sysinfo::Pid::from_u32(pid)).map(|p| p.run_time())
    }

    pub fn refresh_single(&mut self, known_sub_pid: Option<u32>) {
        self.refresh([known_sub_pid, None, None, None]);
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

            let is_main_electron = !cmd_line.contains("--type=");

            let mut matched_slot = None;
            for slot in 0..4 {
                if matches_instance_slot(&cmd_line, slot) || (known_sub_pids[slot] == Some(pid_u32) && is_main_electron) {
                    matched_slot = Some(slot);
                    break;
                }
            }

            if let Some(slot) = matched_slot {
                if is_main_electron && sub_main_pids[slot].is_none() {
                    sub_main_pids[slot] = Some(pid_u32);
                }
            } else if is_main_electron && host_electron_pid.is_none() {
                let has_any_sub_tag = (0..4).any(|s| matches_instance_slot(&cmd_line, s));
                let is_any_known_sub = known_sub_pids.iter().any(|k| *k == Some(pid_u32));
                if !has_any_sub_tag && !is_any_known_sub {
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
            for slot in 0..4 {
                let is_slot_tag = matches_instance_slot(&cmd_line, slot);
                let is_slot_main = active_sub_pids[slot] == Some(pid_u32);
                let is_slot_descendant = active_sub_pids[slot]
                    .map(|main_pid| is_descendant_of(&self.sys, *pid, main_pid))
                    .unwrap_or(false);

                if is_slot_tag || is_slot_main || is_slot_descendant {
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
                } else if is_ls && host_ls_pid.is_none() && host_electron_pid.is_some() {
                    host_ls_pid = Some(pid_u32);
                    host_ls_csrf = extract_csrf_token(proc.cmd());
                    host_mem_bytes += proc.memory();
                }
            }
        }

        if host_ls_pid != self.cached_host_ls_pid {
            self.cached_host_ls_pid = host_ls_pid;
            self.cached_host_port = None;
            self.cached_host_csrf = host_ls_csrf;
            self.host_port_retry_count = 0;
        } else if host_ls_csrf.is_some() && self.cached_host_csrf.is_none() {
            self.cached_host_csrf = host_ls_csrf;
        }

        let mut pids_to_detect = Vec::new();

        if let Some(pid) = host_ls_pid {
            if self.cached_host_port.is_none() {
                self.host_port_retry_count = self.host_port_retry_count.wrapping_add(1);
                if self.host_port_retry_count <= 10 || self.host_port_retry_count % 5 == 0 {
                    pids_to_detect.push(pid);
                }
            }
        } else {
            self.cached_host_port = None;
            self.cached_host_csrf = None;
            self.host_port_retry_count = 0;
        }

        for slot in 0..4 {
            let ls_pid = sub_ls_pids[slot];
            if ls_pid != self.cached_sub_ls_pids[slot] {
                self.cached_sub_ls_pids[slot] = ls_pid;
                self.cached_sub_ports[slot] = None;
                self.cached_sub_csrfs[slot] = sub_ls_csrfs[slot].clone();
                self.sub_port_retry_counts[slot] = 0;
            } else if sub_ls_csrfs[slot].is_some() && self.cached_sub_csrfs[slot].is_none() {
                self.cached_sub_csrfs[slot] = sub_ls_csrfs[slot].clone();
            }

            if let Some(pid) = ls_pid {
                if self.cached_sub_ports[slot].is_none() {
                    self.sub_port_retry_counts[slot] = self.sub_port_retry_counts[slot].wrapping_add(1);
                    if self.sub_port_retry_counts[slot] <= 10 || self.sub_port_retry_counts[slot] % 5 == 0 {
                        pids_to_detect.push(pid);
                    }
                }
            } else {
                self.cached_sub_ports[slot] = None;
                self.cached_sub_csrfs[slot] = None;
                self.sub_port_retry_counts[slot] = 0;
            }
        }

        if !pids_to_detect.is_empty() {
            let detected = detect_listening_ports(&pids_to_detect);
            if let Some(pid) = host_ls_pid {
                if let Some(&p) = detected.get(&pid) {
                    self.cached_host_port = Some(p);
                }
            }
            for slot in 0..4 {
                if let Some(pid) = sub_ls_pids[slot] {
                    if let Some(&p) = detected.get(&pid) {
                        self.cached_sub_ports[slot] = Some(p);
                    }
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
            ls_pid: host_ls_pid,
            cpu_usage: (host_cpu / num_cpus).min(100.0),
            memory_rss_mb: (host_mem_bytes as f64) / (1024.0 * 1024.0),
            ls_port: self.cached_host_port,
            ls_csrf: self.cached_host_csrf.clone(),
        };

        for slot in 0..4 {
            self.subs[slot] = ProcessStats {
                is_running: sub_electron_pids[slot].is_some(),
                electron_pid: sub_electron_pids[slot],
                ls_pid: sub_ls_pids[slot],
                cpu_usage: (sub_cpus[slot] / num_cpus).min(100.0),
                memory_rss_mb: (sub_mem_bytes[slot] as f64) / (1024.0 * 1024.0),
                ls_port: self.cached_sub_ports[slot],
                ls_csrf: self.cached_sub_csrfs[slot].clone(),
            };
        }

        self.sub = self.subs[0].clone();
        self.total_memory_mb = self.host.memory_rss_mb
            + self.subs.iter().map(|s| s.memory_rss_mb).sum::<f64>();
        let total_ram = (self.sys.total_memory() as f64) / (1024.0 * 1024.0);
        if total_ram > 0.0 {
            self.system_total_ram_mb = total_ram;
        }
    }
}

fn detect_listening_ports(pids: &[u32]) -> HashMap<u32, u16> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let mut result = HashMap::new();
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
    let mut pid_ports: HashMap<u32, Vec<u16>> = HashMap::new();
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
        if pids.contains(&target_pid) {
            if let Some(port) = parts[1]
                .rfind(':')
                .and_then(|pos| parts[1][pos + 1..].parse::<u16>().ok())
            {
                pid_ports.entry(target_pid).or_default().push(port);
            }
        }
    }

    for (&pid, ports) in pid_ports.iter_mut() {
        ports.sort_unstable();
        if let Some(&last_port) = ports.last() {
            result.insert(pid, last_port);
        }
    }

    result
}

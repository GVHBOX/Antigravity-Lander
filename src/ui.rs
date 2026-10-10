use crate::app::App;
use chrono::Local;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

const COLOR_BORDER_DEFAULT: Color = Color::Rgb(40, 48, 68);
const COLOR_BORDER_ACTIVE: Color = Color::Rgb(56, 189, 248);
const COLOR_BG_CARD: Color = Color::Rgb(11, 14, 22);
const COLOR_BG_LOGS: Color = Color::Rgb(6, 8, 14);

const COLOR_EMERALD: Color = Color::Rgb(52, 211, 153);
const COLOR_CYAN: Color = Color::Rgb(56, 189, 248);
const COLOR_INDIGO: Color = Color::Rgb(129, 140, 248);
const COLOR_AMBER: Color = Color::Rgb(251, 191, 36);
const COLOR_ROSE: Color = Color::Rgb(251, 113, 133);
const COLOR_PURPLE: Color = Color::Rgb(192, 132, 252);
const COLOR_SLATE_MUTED: Color = Color::Rgb(148, 163, 184);
const COLOR_SLATE_DIM: Color = Color::Rgb(100, 116, 139);

pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();
    let compact = size.width < 120;
    let dock_height = if compact { 4 } else { 3 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(8),
            Constraint::Length(dock_height),
        ])
        .split(size);

    render_header(frame, chunks[0], app);
    render_kpi_tiles(frame, chunks[1], app);
    render_dual_instances(frame, chunks[2], app);

    let lower_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(44),
            Constraint::Percentage(56),
        ])
        .split(chunks[3]);

    render_sandbox_inspector(frame, lower_chunks[0], app);
    render_logs(frame, lower_chunks[1], app);

    render_dock(frame, chunks[4], app);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();
    let clock_time = Local::now().format("%H:%M:%S").to_string();

    let title_line = Line::from(vec![
        Span::styled(" ⚡ ", Style::default().fg(Color::Rgb(254, 240, 138)).bg(Color::Rgb(79, 70, 229)).bold()),
        Span::raw(" "),
        Span::styled(s.app_title, Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(COLOR_CYAN).bold()),
        Span::styled(" / ", Style::default().fg(COLOR_SLATE_DIM)),
        Span::styled(s.app_subtitle, Style::default().fg(COLOR_SLATE_MUTED)),
    ]);

    let right_line = Line::from(vec![
        Span::styled("● ", Style::default().fg(COLOR_EMERALD)),
        Span::styled(s.kernel_status, Style::default().fg(COLOR_EMERALD).bold()),
        Span::styled(" │ ", Style::default().fg(COLOR_BORDER_DEFAULT)),
        Span::styled(s.mem_label, Style::default().fg(COLOR_SLATE_MUTED)),
        Span::raw(": "),
        Span::styled(format!("{:.1} MB", app.launcher_rss_mb), Style::default().fg(Color::White).bold()),
        Span::styled(" │ ", Style::default().fg(COLOR_BORDER_DEFAULT)),
        Span::styled(clock_time, Style::default().fg(COLOR_CYAN).bold()),
        Span::styled(" │ ", Style::default().fg(COLOR_BORDER_DEFAULT)),
        Span::styled(format!("[T] {}", s.lang_btn), Style::default().fg(COLOR_INDIGO).bold()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
        .bg(COLOR_BG_CARD);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if area.width < 120 {
        frame.render_widget(Paragraph::new(title_line), inner);
    } else {
        frame.render_widget(Paragraph::new(title_line), inner);
        frame.render_widget(
            Paragraph::new(right_line).alignment(Alignment::Right),
            inner,
        );
    }
}

fn render_kpi_tiles(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();
    let compact = area.width < 120;
    let tiles = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let host_online = app.monitor.host.is_running;
    let host_pid_str = app.monitor.host.electron_pid.map(|p| {
        if compact {
            format!("{}", p)
        } else {
            format!("PID: {}", p)
        }
    }).unwrap_or_else(|| "--".into());
    let t1_content = Line::from(vec![
        Span::styled(if host_online { "● " } else { "○ " }, Style::default().fg(if host_online { COLOR_EMERALD } else { COLOR_SLATE_DIM })),
        Span::styled(if host_online { s.status_online } else { s.status_stopped }, Style::default().fg(if host_online { COLOR_EMERALD } else { COLOR_SLATE_DIM }).bold()),
        Span::raw("  "),
        Span::styled(host_pid_str, Style::default().fg(COLOR_SLATE_DIM)),
    ]);
    frame.render_widget(
        Paragraph::new(t1_content).block(
            Block::default()
                .title(Span::styled(format!(" {} ", if compact { "主机" } else { s.tile_host_title }), Style::default().fg(COLOR_SLATE_MUTED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        tiles[0],
    );

    let sub_online = app.monitor.sub.is_running;
    let sub_pid_str = app.monitor.sub.electron_pid.map(|p| {
        if compact {
            format!("{}", p)
        } else {
            format!("PID: {}", p)
        }
    }).unwrap_or_else(|| "--".into());
    let t2_content = Line::from(vec![
        Span::styled(if sub_online { "● " } else { "○ " }, Style::default().fg(if sub_online { COLOR_CYAN } else { COLOR_SLATE_DIM })),
        Span::styled(if sub_online { s.status_detached } else { s.status_stopped }, Style::default().fg(if sub_online { COLOR_CYAN } else { COLOR_SLATE_DIM }).bold()),
        Span::raw("  "),
        Span::styled(sub_pid_str, Style::default().fg(COLOR_SLATE_DIM)),
    ]);
    frame.render_widget(
        Paragraph::new(t2_content).block(
            Block::default()
                .title(Span::styled(format!(" {} ", if compact { "分身" } else { s.tile_sub_title }), Style::default().fg(COLOR_SLATE_MUTED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if sub_online { COLOR_CYAN } else { COLOR_BORDER_DEFAULT }))
                .bg(COLOR_BG_CARD),
        ),
        tiles[1],
    );

    let host_mem = app.monitor.host.memory_rss_mb;
    let sub_mem = app.monitor.sub.memory_rss_mb;
    let t3_content = Line::from(vec![
        Span::styled(
            if compact {
                format!("{:.0}M", app.monitor.total_memory_mb)
            } else {
                format!("{:.1} MB", app.monitor.total_memory_mb)
            },
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            if compact {
                format!("H:{:.0} S:{:.0}", host_mem, sub_mem)
            } else {
                format!("(H:{:.0}M|S:{:.0}M)", host_mem, sub_mem)
            },
            Style::default().fg(COLOR_SLATE_DIM),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(t3_content).block(
            Block::default()
                .title(Span::styled(format!(" {} ", if compact { "内存" } else { s.tile_mem_title }), Style::default().fg(COLOR_SLATE_MUTED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        tiles[2],
    );

    let t4_content = Line::from(vec![
        Span::styled(s.tile_iso_level, Style::default().fg(COLOR_PURPLE).bold()),
        Span::raw(" "),
        Span::styled(format!("[{}]", s.tile_iso_safe), Style::default().fg(COLOR_EMERALD)),
    ]);
    frame.render_widget(
        Paragraph::new(t4_content).block(
            Block::default()
                .title(Span::styled(format!(" {} ", if compact { "隔离" } else { s.tile_iso_title }), Style::default().fg(COLOR_SLATE_MUTED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        tiles[3],
    );
}

fn render_dual_instances(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();
    let compact = area.width < 120;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let host = &app.monitor.host;
    let host_pid_text = match (host.electron_pid, host.ls_pid, compact) {
        (Some(e), Some(ls), false) => format!("Electron ({}) + GoLS ({})", e, ls),
        (Some(e), None, false) => format!("Electron ({})", e),
        (Some(e), _, true) => format!("PID {}", e),
        _ => "--".to_string(),
    };
    let host_port_text = host.ls_port.map(|p| format!("127.0.0.1:{}", p)).unwrap_or_else(|| "--".into());

    let mut host_lines = vec![
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_process_arch), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(host_pid_text, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_service_port), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(host_port_text, Style::default().fg(COLOR_INDIGO).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_account_state), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(s.inst1_account, Style::default().fg(COLOR_EMERALD).bold()),
        ]),
    ];
    if compact {
        host_lines.push(Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1}% ", host.cpu_usage), Style::default().fg(COLOR_EMERALD).bold()),
            Span::styled(make_mini_bar(host.cpu_usage as f64, 100.0, 4), Style::default().fg(COLOR_EMERALD)),
            Span::styled(" RSS: ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.0}M", host.memory_rss_mb), Style::default().fg(Color::White).bold()),
        ]));
    } else {
        host_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_token_store), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(s.mode_win_cred, Style::default().fg(COLOR_SLATE_DIM)),
        ]));
        host_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_cpu_usage), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1}% ", host.cpu_usage), Style::default().fg(COLOR_EMERALD).bold()),
            Span::styled(make_mini_bar(host.cpu_usage as f64, 100.0, 8), Style::default().fg(COLOR_EMERALD)),
            Span::styled("  │  ", Style::default().fg(COLOR_BORDER_DEFAULT)),
            Span::styled(format!("{}: ", s.lbl_mem_rss), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1} MB ", host.memory_rss_mb), Style::default().fg(Color::White).bold()),
            Span::styled(make_mini_bar(host.memory_rss_mb, app.monitor.system_total_ram_mb.max(1024.0), 8), Style::default().fg(COLOR_INDIGO)),
        ]));
    }

    let host_title = format!(" {} [{}] ", s.inst1_title, s.badge_host_primary);
    frame.render_widget(
        Paragraph::new(host_lines).block(
            Block::default()
                .title(Span::styled(host_title, Style::default().fg(COLOR_EMERALD).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        cols[0],
    );

    let sub = &app.monitor.sub;
    let sub_pid_text = match (sub.electron_pid, sub.ls_pid, compact) {
        (Some(e), Some(ls), false) => format!("Detached ({}) + GoLS ({})", e, ls),
        (Some(e), None, false) => format!("Detached ({})", e),
        (Some(e), _, true) => format!("PID {}", e),
        _ => if sub.is_running { s.status_detached.to_string() } else { s.status_stopped.to_string() },
    };
    let sub_port_text = sub.ls_port.map(|p| format!("127.0.0.1:{}", p)).unwrap_or_else(|| "--".into());
    let token_present = app.config.is_token_present();
    let account_status = if token_present {
        Span::styled(s.inst2_bound_user, Style::default().fg(COLOR_EMERALD).bold())
    } else {
        Span::styled(s.inst2_empty_user, Style::default().fg(COLOR_AMBER).bold())
    };

    let mut sub_lines = vec![
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_process_arch), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(sub_pid_text, Style::default().fg(COLOR_CYAN)),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_service_port), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(sub_port_text, Style::default().fg(COLOR_INDIGO).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_account_state), Style::default().fg(COLOR_SLATE_MUTED)),
            account_status,
        ]),
    ];
    if compact {
        sub_lines.push(Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1}% ", sub.cpu_usage), Style::default().fg(COLOR_CYAN).bold()),
            Span::styled(make_mini_bar(sub.cpu_usage as f64, 100.0, 4), Style::default().fg(COLOR_CYAN)),
            Span::styled(" RSS: ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.0}M", sub.memory_rss_mb), Style::default().fg(Color::White).bold()),
        ]));
    } else {
        sub_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_token_store), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(s.mode_file_store, Style::default().fg(COLOR_CYAN)),
        ]));
        sub_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", s.lbl_cpu_usage), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1}% ", sub.cpu_usage), Style::default().fg(COLOR_CYAN).bold()),
            Span::styled(make_mini_bar(sub.cpu_usage as f64, 100.0, 8), Style::default().fg(COLOR_CYAN)),
            Span::styled("  │  ", Style::default().fg(COLOR_BORDER_DEFAULT)),
            Span::styled(format!("{}: ", s.lbl_mem_rss), Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("{:.1} MB ", sub.memory_rss_mb), Style::default().fg(Color::White).bold()),
            Span::styled(make_mini_bar(sub.memory_rss_mb, app.monitor.system_total_ram_mb.max(1024.0), 8), Style::default().fg(COLOR_CYAN)),
        ]));
    }

    let sub_badge = if sub.is_running { s.badge_sub_detached } else { s.badge_sub_stopped };
    let sub_title = format!(" {} [{}] ", s.inst2_title, sub_badge);
    let sub_border_color = if sub.is_running { COLOR_BORDER_ACTIVE } else { COLOR_BORDER_DEFAULT };

    frame.render_widget(
        Paragraph::new(sub_lines).block(
            Block::default()
                .title(Span::styled(sub_title, Style::default().fg(COLOR_CYAN).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(sub_border_color))
                .bg(COLOR_BG_CARD),
        ),
        cols[1],
    );
}

fn render_sandbox_inspector(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();
    let root_path = app.config.sandbox_root.display().to_string();

    let lines = vec![
        Line::from(vec![
            Span::styled(" USERPROFILE : ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("...\\data\\{}\\home", app.config.sandbox_root.file_name().and_then(|f| f.to_str()).unwrap_or("instance_1")), Style::default().fg(Color::White)),
            Span::styled(format!(" [{}]", s.tag_isolated), Style::default().fg(COLOR_EMERALD).bold()),
        ]),
        Line::from(vec![
            Span::styled(" APPDATA     : ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled(format!("...\\data\\{}\\AppData\\Roaming", app.config.sandbox_root.file_name().and_then(|f| f.to_str()).unwrap_or("instance_1")), Style::default().fg(Color::White)),
            Span::styled(format!(" [{}]", s.tag_isolated), Style::default().fg(COLOR_EMERALD).bold()),
        ]),
        Line::from(vec![
            Span::styled(" LOCALAPPDATA: ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled("C:\\...\\AppData\\Local", Style::default().fg(Color::White)),
            Span::styled(format!(" [{}]", s.tag_pass_through), Style::default().fg(COLOR_CYAN).bold()),
        ]),
        Line::from(vec![
            Span::styled(" HOOK BYPASS : ", Style::default().fg(COLOR_SLATE_MUTED)),
            Span::styled("SSH_CONNECTION=127.0.0.1", Style::default().fg(COLOR_INDIGO)),
            Span::styled(format!(" [{}]", s.tag_hook_active), Style::default().fg(COLOR_PURPLE).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!(" {}: ", s.sandbox_root_label), Style::default().fg(COLOR_SLATE_DIM)),
            Span::styled(root_path, Style::default().fg(COLOR_SLATE_MUTED)),
        ]),
    ];

    let title = format!(" {} ", s.sandbox_title);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(Color::White).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        area,
    );
}

fn render_logs(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();
    let max_lines = (area.height.saturating_sub(2)) as usize;

    let total_logs = app.logs.len();
    let skip_count = total_logs.saturating_sub(max_lines);

    let lines: Vec<Line> = app
        .logs
        .iter()
        .skip(skip_count)
        .map(|entry| {
            let level_color = match entry.level {
                "SYSTEM" => COLOR_SLATE_MUTED,
                "KERNEL" => COLOR_INDIGO,
                "MONITOR" => COLOR_CYAN,
                "SANDBOX" => COLOR_PURPLE,
                "HOOK" => COLOR_AMBER,
                "SPAWN" => COLOR_CYAN,
                "WIN32" => COLOR_EMERALD,
                "LANG_SVR" => COLOR_EMERALD,
                "KILL" => COLOR_ROSE,
                "AUTH" => COLOR_AMBER,
                "CONFIG" => COLOR_INDIGO,
                _ => Color::White,
            };

            Line::from(vec![
                Span::styled(format!("{} ", entry.timestamp), Style::default().fg(COLOR_SLATE_DIM)),
                Span::styled(format!("[{:^7}] ", entry.level), Style::default().fg(level_color).bold()),
                Span::styled(&entry.message, Style::default().fg(Color::Rgb(226, 232, 240))),
            ])
        })
        .collect();

    let title = format!(" {} ", s.log_title);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(Color::White).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_LOGS),
        ),
        area,
    );
}

fn render_dock(frame: &mut Frame, area: Rect, app: &App) {
    let s = app.language.strings();

    let items = [
        ("Space", s.btn_space, COLOR_CYAN),
        ("K", s.btn_kill, COLOR_ROSE),
        ("R", s.btn_restart, COLOR_AMBER),
        ("C", s.btn_clear, COLOR_PURPLE),
        ("P", "代理", COLOR_EMERALD),
        ("O", s.btn_open, COLOR_SLATE_MUTED),
        ("T", s.btn_lang, COLOR_INDIGO),
        ("Q", s.btn_quit, COLOR_SLATE_DIM),
    ];

    let notice_text = if !app.toast_message.is_empty() {
        &app.toast_message
    } else {
        s.toast_notice
    };

    let notice_color = if app.toast_is_alert {
        COLOR_ROSE
    } else if !app.toast_message.is_empty() {
        COLOR_CYAN
    } else {
        COLOR_SLATE_MUTED
    };

    let title_line = Line::from(vec![
        Span::styled(" ◆ ", Style::default().fg(COLOR_CYAN)),
        Span::styled(notice_text, Style::default().fg(notice_color)),
    ]);

    let lines = if area.width < 120 {
        vec![
            make_dock_line(&items[..4]),
            make_dock_line(&items[4..]),
        ]
    } else {
        vec![make_dock_line(&items)]
    };

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(title_line)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_DEFAULT))
                .bg(COLOR_BG_CARD),
        ),
        area,
    );
}

fn make_dock_line(items: &[(&str, &str, Color)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, label, color)) in items.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(COLOR_BORDER_DEFAULT)));
        }
        spans.push(Span::styled(
            format!(" [{}] ", key),
            Style::default().fg(Color::White).bg(Color::Rgb(30, 41, 59)).bold(),
        ));
        spans.push(Span::styled(
            format!(" {} ", label),
            Style::default().fg(*color).bold(),
        ));
    }
    Line::from(spans)
}

fn make_mini_bar(current: f64, max: f64, width: usize) -> String {
    let ratio = (current / max).clamp(0.0, 1.0);
    let filled = (ratio * (width as f64)).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "■".repeat(filled), "░".repeat(empty))
}

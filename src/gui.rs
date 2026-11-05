use crate::launcher::{LauncherConfig, ScaleSettings};
use crate::monitor::{MonitorView, TelemetryMonitor};
use crate::proxy::ProxyManager;
use crate::quota::InstanceQuotaState;
use crate::session::{format_tokens, format_tps_display, scan_sandbox_sessions, InstanceSessions};
use anyhow::Result;
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};

use std::sync::{Arc, Mutex};

slint::include_modules!();

fn to_session_ui_model(groups: &[crate::session::SessionGroup]) -> slint::ModelRc<SessionUiItem> {
    let mut ui_vec: Vec<SessionUiItem> = Vec::new();
    for g in groups {
        let count_str = format!("{} 个会话", g.sessions.len());
        ui_vec.push(SessionUiItem {
            is_group_header: true,
            group_name: g.project_name.as_str().into(),
            group_count_str: count_str.into(),
            group_has_running: g.has_running,
            id: "".into(),
            title: "".into(),
            context_str: "".into(),
            cache_rate_str: "".into(),
            time_ago_str: "".into(),
            is_running: false,
        });
        for it in &g.sessions {
            let short_id = if it.id.len() >= 8 { &it.id[..8] } else { &it.id };
            ui_vec.push(SessionUiItem {
                is_group_header: false,
                group_name: "".into(),
                group_count_str: "".into(),
                group_has_running: false,
                id: short_id.into(),
                title: it.title.as_str().into(),
                context_str: it.context_str.as_str().into(),
                cache_rate_str: it.cache_rate_str.as_str().into(),
                time_ago_str: it.time_ago_str.as_str().into(),
                is_running: it.is_running,
            });
        }
    }
    std::rc::Rc::new(slint::VecModel::from(ui_vec)).into()
}

fn to_recent_session_model(inst: &crate::session::InstanceSessions, limit: usize) -> slint::ModelRc<SessionUiItem> {
    let mut rows: Vec<&crate::session::SessionItem> = inst.groups.iter().flat_map(|g| g.sessions.iter()).collect();
    rows.sort_by_key(|s| s.last_updated_secs_ago);
    let ui_vec: Vec<SessionUiItem> = rows
        .into_iter()
        .take(limit)
        .map(|it| SessionUiItem {
            is_group_header: false,
            group_name: it.project_name.as_str().into(),
            group_count_str: "".into(),
            group_has_running: false,
            id: if it.id.len() >= 8 { it.id[..8].into() } else { it.id.as_str().into() },
            title: it.title.as_str().into(),
            context_str: it.context_str.as_str().into(),
            cache_rate_str: it.cache_rate_str.as_str().into(),
            time_ago_str: it.time_ago_str.as_str().into(),
            is_running: it.is_running,
        })
        .collect();
    std::rc::Rc::new(slint::VecModel::from(ui_vec)).into()
}

fn quota_rows(state: Option<&InstanceQuotaState>, is_dark: bool) -> Vec<ModelQuotaRow> {
    let Some(st) = state.filter(|s| s.is_available) else {
        return Vec::new();
    };
    st.models
        .iter()
        .map(|m| {
            let weekly_ring = if m.has_weekly && m.weekly.remaining_pct >= 0.0 {
                crate::quota::render_quota_ring(m.weekly.remaining_pct, is_dark)
            } else {
                crate::quota::render_quota_ring(-1.0, is_dark)
            };
            let five_h_ring = if m.has_five_h && m.five_h.remaining_pct >= 0.0 {
                crate::quota::render_quota_ring(m.five_h.remaining_pct, is_dark)
            } else {
                crate::quota::render_quota_ring(-1.0, is_dark)
            };
            let weekly_pct = if m.has_weekly && !m.weekly.pct_text.is_empty() {
                m.weekly.pct_text.as_str().into()
            } else {
                "--".into()
            };
            let five_h_pct = if m.has_five_h && !m.five_h.pct_text.is_empty() {
                m.five_h.pct_text.as_str().into()
            } else {
                "--".into()
            };
            let mut parts: Vec<String> = Vec::new();
            if m.has_weekly && !m.weekly.reset_text.is_empty() {
                parts.push(m.weekly.reset_text.replace("后重置", ""));
            }
            if m.has_five_h && !m.five_h.reset_text.is_empty() {
                parts.push(m.five_h.reset_text.replace("后重置", ""));
            }
            let reset_text = if parts.is_empty() {
                "--".to_string()
            } else {
                parts.join(" · ")
            };
            ModelQuotaRow {
                name: m.short_name.as_str().into(),
                weekly_pct,
                weekly_ring,
                five_h_pct,
                five_h_ring,
                has_five_h: m.has_five_h,
                reset_text: reset_text.into(),
            }
        })
        .collect()
}

fn apply_quota_to_ui(
    state: Option<&InstanceQuotaState>,
    is_dark: bool,
    set_profile: impl FnOnce(&str, &str, &str, Option<&std::path::Path>),
    set_models: impl FnOnce(slint::ModelRc<ModelQuotaRow>),
) {
    if let Some(st) = state.filter(|s| s.is_available) {
        set_profile(
            &st.profile.name,
            &st.profile.email,
            &st.profile.tier,
            st.profile.avatar_path.as_deref(),
        );
        let rows = quota_rows(state, is_dark);
        set_models(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(rows))));
        return;
    }
    set_profile("--", "--", "", None);
    set_models(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(
        Vec::<ModelQuotaRow>::new(),
    ))));
}

fn load_avatar_image(path: Option<&std::path::Path>) -> (slint::Image, bool) {
    if let Some(p) = path
        && p.exists()
            && let Ok(img) = slint::Image::load_from_path(p)
                && img.size().width > 0 {
                    return (img, true);
                }
    (slint::Image::default(), false)
}

fn slot_rpc_info(mon: &TelemetryMonitor, slot: usize) -> (Option<u32>, Option<u16>, Vec<u16>, Option<String>) {
    if slot == 0 {
        (mon.host.ls_pid, mon.host.ls_port, mon.host.ls_ports.clone(), mon.host.ls_csrf.clone())
    } else {
        let s = &mon.subs[slot - 1];
        (s.ls_pid, s.ls_port, s.ls_ports.clone(), s.ls_csrf.clone())
    }
}

fn slot_rpc_ports(mon_arc: &Arc<Mutex<TelemetryMonitor>>, slot: usize) -> Option<(Vec<u16>, String)> {
    if slot > 4 {
        return None;
    }
    let (pid, resolved, ports, csrf) = {
        let mon = mon_arc.lock().unwrap();
        slot_rpc_info(&mon, slot)
    };
    let csrf = csrf.filter(|c| !c.is_empty())?;
    let pid = pid?;
    let active = match resolved {
        Some(p) => p,
        None => {
            let p = crate::quota::resolve_rpc_port(pid, &ports, &csrf)?;
            mon_arc.lock().unwrap().apply_resolved_port(pid, p);
            p
        }
    };
    let mut ordered = Vec::with_capacity(ports.len() + 1);
    ordered.push(active);
    ordered.extend(ports.into_iter().filter(|&p| p != active));
    Some((ordered, csrf))
}

fn cache_report_text(found: &[crate::launcher::SandboxCache]) -> (String, String) {
    let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
    let total = total_bytes as f64 / (1024.0 * 1024.0);
    let detail = found
        .iter()
        .map(|c| format!("分身 {} · {:.1} MB", c.slot, c.bytes as f64 / (1024.0 * 1024.0)))
        .collect::<Vec<_>>()
        .join("\n");
    (detail, format!("共 {:.1} MB", total))
}

fn extract_recent_session_info(sessions: &InstanceSessions) -> (String, String, String, String) {
    if let Some(s) = sessions.groups.first().and_then(|g| g.sessions.first()) {
        (
            s.project_name.clone(),
            s.title.clone(),
            format!("{} · 缓存 {}", s.context_str, s.cache_rate_str),
            format!("最近更新: {}", s.time_ago_str),
        )
    } else {
        ("无活动会话".to_string(), "".to_string(), "".to_string(), "等待初始化".to_string())
    }
}

#[cfg(target_os = "windows")]
fn trim_working_set() {
    unsafe {
        windows_sys::Win32::System::Threading::SetProcessWorkingSetSize(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            usize::MAX,
            usize::MAX,
        );
    }
}

#[cfg(target_os = "windows")]
fn get_cockpit_hwnd() -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        unsafe {
            let mut proc_id = 0u32;
            GetWindowThreadProcessId(hwnd, &mut proc_id);
            if proc_id == GetCurrentProcessId() {
                let mut class_buf = [0u16; 64];
                let len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
                if len > 0 {
                    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
                    if class_name == "Window Class" {
                        let mut title_buf = [0u16; 64];
                        let tlen = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
                        let title = String::from_utf16_lossy(&title_buf[..tlen as usize]);
                        if title.contains("Lander") {
                            let out_ptr = lparam as *mut HWND;
                            *out_ptr = hwnd;
                            return 0;
                        }
                    }
                }
            }
            1
        }
    }

    let mut found_hwnd: HWND = std::ptr::null_mut();
    unsafe {
        EnumWindows(Some(enum_proc), &mut found_hwnd as *mut _ as LPARAM);
    }
    found_hwnd
}

#[cfg(target_os = "windows")]
fn apply_window_icon(hwnd: windows_sys::Win32::Foundation::HWND) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ExtractIconExW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HICON, ICON_BIG, ICON_SMALL, SendMessageW, WM_SETICON,
    };

    if hwnd.is_null() {
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut large: HICON = std::ptr::null_mut();
    let mut small: HICON = std::ptr::null_mut();
    unsafe {
        let extracted = ExtractIconExW(wide.as_ptr(), 0, &mut large, &mut small, 1);
        if extracted == 0 {
            return;
        }
        if !large.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, large as isize);
        }
        if !small.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small as isize);
        }
    }
}

#[cfg(target_os = "windows")]
fn get_window_monitor_work_area(hwnd: windows_sys::Win32::Foundation::HWND) -> (i32, i32, i32, i32) {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if !monitor.is_null() {
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut mi) != 0 {
                return (mi.rcWork.left, mi.rcWork.top, mi.rcWork.right, mi.rcWork.bottom);
            }
        }
        (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

#[cfg(target_os = "windows")]
struct BrushHandle(windows_sys::Win32::Graphics::Gdi::HBRUSH);
#[cfg(target_os = "windows")]
unsafe impl Send for BrushHandle {}
#[cfg(target_os = "windows")]
unsafe impl Sync for BrushHandle {}

#[cfg(target_os = "windows")]
static DARK_BRUSH: std::sync::OnceLock<BrushHandle> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static LIGHT_BRUSH: std::sync::OnceLock<BrushHandle> = std::sync::OnceLock::new();

#[cfg(target_os = "windows")]
fn apply_dark_window_attributes(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows_sys::Win32::Graphics::Gdi::CreateSolidBrush;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetClassLongPtrW, GCLP_HBRBACKGROUND};

    if hwnd.is_null() {
        return;
    }
    unsafe {
        let dark = 1i32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        let caption_color = 0x000E0907u32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            &caption_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let brush = DARK_BRUSH.get_or_init(|| {
            BrushHandle(CreateSolidBrush(0x000E0907))
        }).0;
        if !brush.is_null() {
            SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush as _);
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_light_window_attributes(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows_sys::Win32::Graphics::Gdi::CreateSolidBrush;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetClassLongPtrW, GCLP_HBRBACKGROUND};

    if hwnd.is_null() {
        return;
    }
    unsafe {
        let dark = 0i32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        let caption_color = 0x00F9F4F1u32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            &caption_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let brush = LIGHT_BRUSH.get_or_init(|| {
            BrushHandle(CreateSolidBrush(0x00F9F4F1))
        }).0;
        if !brush.is_null() {
            SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush as _);
        }
    }
}

#[cfg(target_os = "windows")]
fn center_and_scale_cockpit_window(hwnd: windows_sys::Win32::Foundation::HWND, scale: f32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_FRAMECHANGED, SWP_NOZORDER,
    };

    if hwnd.is_null() {
        return;
    }

    let (work_left, work_top, work_right, work_bottom) = get_window_monitor_work_area(hwnd);
    let s = if scale > 0.0 { scale } else { 1.0 };
    let work_w = work_right - work_left;
    let work_h = work_bottom - work_top;
    let ideal_w = (2250.0 * s).round() as i32;
    let ideal_h = (1305.0 * s).round() as i32;
    let min_w = (2050.0 * s).round() as i32;
    let min_h = (1160.0 * s).round() as i32;
    let target_w = (work_w - 20).min(ideal_w).max(min_w);
    let target_h = (work_h - 20).min(ideal_h).max(min_h);
    let x = work_left + ((work_w - target_w) / 2).max(0);
    let y = work_top + ((work_h - target_h) / 2).max(0);
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            target_w,
            target_h,
            SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

#[cfg(target_os = "windows")]
fn set_cockpit_always_on_top(pinned: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
    };
    let hwnd = get_cockpit_hwnd();
    if !hwnd.is_null() {
        let insert_after = if pinned { HWND_TOPMOST } else { HWND_NOTOPMOST };
        unsafe {
            SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }
}

#[cfg(target_os = "windows")]
fn hide_cockpit_to_tray(ui_weak: &slint::Weak<MainWindow>) {
    if let Some(ui) = ui_weak.upgrade() {
        let _ = ui.window().hide();
    }
    trim_working_set();
}

#[cfg(target_os = "windows")]
fn show_cockpit_from_tray(ui_weak: &slint::Weak<MainWindow>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let cur = ui.get_redraw_tick();
        ui.set_redraw_tick(if cur == 0 { 1 } else { 0 });
        let window = ui.window();
        let _ = window.show();

        let hwnd = get_cockpit_hwnd();
        if !hwnd.is_null() {
            if ui.get_is_dark() {
                apply_dark_window_attributes(hwnd);
            } else {
                apply_light_window_attributes(hwnd);
            }
            if ui.get_is_pinned() {
                set_cockpit_always_on_top(true);
            }
            unsafe {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                } else {
                    ShowWindow(hwnd, SW_SHOW);
                }
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                SetForegroundWindow(hwnd);
            }
        }
        window.request_redraw();
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingPatchAction {
    TogglePatchAg2,
    ToggleProxy,
}

fn interruptible_sleep(ms: u64, shutdown: &std::sync::atomic::AtomicBool) {
    const STEP_MS: u64 = 100;
    let mut left = ms;
    while left > 0 {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let chunk = left.min(STEP_MS);
        std::thread::sleep(std::time::Duration::from_millis(chunk));
        left -= chunk;
    }
}

fn interruptible_sleep_wake(
    ms: u64,
    shutdown: &std::sync::atomic::AtomicBool,
    wake: &std::sync::atomic::AtomicBool,
) {
    const STEP_MS: u64 = 100;
    let mut left = ms;
    while left > 0 {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if wake.swap(false, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let chunk = left.min(STEP_MS);
        std::thread::sleep(std::time::Duration::from_millis(chunk));
        left -= chunk;
    }
}

fn collect_session_snapshot(
    mon: &Arc<Mutex<TelemetryMonitor>>,
    enrich: bool,
) -> [InstanceSessions; 5] {
    let mut snapshot = [
        InstanceSessions::default(),
        InstanceSessions::default(),
        InstanceSessions::default(),
        InstanceSessions::default(),
        InstanceSessions::default(),
    ];
    let homes = [
        LauncherConfig::resolve_host_user_dir(),
        LauncherConfig::slot_home_dir(1),
        LauncherConfig::slot_home_dir(2),
        LauncherConfig::slot_home_dir(3),
        LauncherConfig::slot_home_dir(4),
    ];
    for (idx, home) in homes.iter().enumerate() {
        if let Some(h) = home {
            snapshot[idx] = scan_sandbox_sessions(h);
        }
    }
    if enrich {
        for (slot, inst) in snapshot.iter_mut().enumerate() {
            if let Some((ports, csrf)) = slot_rpc_ports(mon, slot)
                && let Some(&port) = ports.first()
            {
                crate::session::enrich_active_session(inst, port, &csrf);
            }
        }
    }
    snapshot
}

pub fn run_gui(activation_event: crate::ActivationEvent) -> Result<()> {
    let _ = LauncherConfig::ensure_all_sandboxes();
    let sub_pids = Arc::new(Mutex::new([None::<u32>; 4]));
    let monitor = Arc::new(Mutex::new(TelemetryMonitor::new()));
    let monitor_view = Arc::new(Mutex::new(MonitorView::default()));
    {
        let mut initial_mon = monitor.lock().unwrap();
        initial_mon.refresh([None, None, None, None]);
        *monitor_view.lock().unwrap() = initial_mon.view();
    }
    let scale_settings = Arc::new(Mutex::new(ScaleSettings::load_or_detect()));
    let scanned_cache_bytes = Arc::new(Mutex::new(0u64));
    let pending_patch_action = Arc::new(Mutex::new(None::<PendingPatchAction>));
    let is_patching = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sessions_store = Arc::new(Mutex::new([InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default()]));
    let live_session_slot = Arc::new(Mutex::new(-1i32));
    let active_live_session = Arc::new(Mutex::new(None::<crate::session::SessionItem>));
    let hover_bar_state = Arc::new(Mutex::new(None::<usize>));
    let quota_cache = Arc::new(Mutex::new(crate::quota::load_quota_cache()));
    let quota_states = Arc::new(Mutex::new([
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
    ]));
    let cleared_credentials = Arc::new(Mutex::new([false; 5]));

    let main_window = MainWindow::new()?;
    main_window.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    let sound_config = Arc::new(Mutex::new(crate::sound::SoundConfig::load_or_default()));

    main_window.set_is_dark(true);
    main_window.set_live_session_target_slot(-1);
    main_window.set_live_session_target_text("自动跟随".into());

    {
        let scales = scale_settings.lock().unwrap();
        main_window.set_host_scale_text(format!("{}%", scales.host).into());
        main_window.set_sub1_scale_text(format!("{}%", scales.get_sub_scale(1)).into());
        main_window.set_sub2_scale_text(format!("{}%", scales.get_sub_scale(2)).into());
        main_window.set_sub3_scale_text(format!("{}%", scales.get_sub_scale(3)).into());
        main_window.set_sub4_scale_text(format!("{}%", scales.get_sub_scale(4)).into());
    }

    let initial_sound_on = sound_config.lock().unwrap().enabled;
    main_window.set_sound_on(initial_sound_on);

    const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32.rgba");
    let tray_menu = tray_icon::menu::Menu::new();
    let item_show = tray_icon::menu::MenuItem::new("显示窗口", true, None);
    let item_sub1 = tray_icon::menu::MenuItem::new("启动/打开分身 1", true, None);
    let item_sub2 = tray_icon::menu::MenuItem::new("启动/打开分身 2", true, None);
    let item_sub3 = tray_icon::menu::MenuItem::new("启动/打开分身 3", true, None);
    let item_sub4 = tray_icon::menu::MenuItem::new("启动/打开分身 4", true, None);
    let sep = tray_icon::menu::PredefinedMenuItem::separator();

    let item_sound = tray_icon::menu::MenuItem::new("切换完成音效", true, None);
    let item_sound_pick = tray_icon::menu::MenuItem::new("选择完成音效文件", true, None);
    let sep_quit = tray_icon::menu::PredefinedMenuItem::separator();
    let item_quit = tray_icon::menu::MenuItem::new("退出", true, None);

    let show_id = item_show.id().clone();
    let sub1_id = item_sub1.id().clone();
    let sub2_id = item_sub2.id().clone();
    let sub3_id = item_sub3.id().clone();
    let sub4_id = item_sub4.id().clone();
    let sound_id = item_sound.id().clone();
    let sound_pick_id = item_sound_pick.id().clone();
    let quit_id = item_quit.id().clone();

    tray_menu.append_items(&[
        &item_show,
        &item_sub1,
        &item_sub2,
        &item_sub3,
        &item_sub4,
        &sep,
        &item_sound,
        &item_sound_pick,
        &sep_quit,
        &item_quit,
    ])?;
    let tray_icon_img = tray_icon::Icon::from_rgba(ICON_RGBA.to_vec(), 32, 32)?;
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Antigravity Lander")
        .with_icon(tray_icon_img)
        .build()?;

    let sub_pids_tray = Arc::clone(&sub_pids);
    let ui_weak_tray = main_window.as_weak();
    let mon_tray = Arc::clone(&monitor);
    let sound_cfg_tray = Arc::clone(&sound_config);

    tray_icon::TrayIconEvent::set_event_handler(Some({
        let ui_weak = ui_weak_tray.clone();
        move |event| {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            }
            | tray_icon::TrayIconEvent::DoubleClick {
                button: tray_icon::MouseButton::Left,
                ..
            } = event
            {
                #[cfg(target_os = "windows")]
                show_cockpit_from_tray(&ui_weak);
            }
        }
    }));

    tray_icon::menu::MenuEvent::set_event_handler(Some({
        let pid_lock = sub_pids_tray;
        let ui_weak = ui_weak_tray.clone();
        let mon_arc = mon_tray;
        let sound_cfg_menu = sound_cfg_tray;
        move |event: tray_icon::menu::MenuEvent| {
            if event.id == show_id {
                #[cfg(target_os = "windows")]
                show_cockpit_from_tray(&ui_weak);
            } else if event.id == quit_id {
                std::process::exit(0);
            } else if event.id == sound_id {
                let is_on = {
                    let mut cfg = sound_cfg_menu.lock().unwrap();
                    cfg.enabled = !cfg.enabled;
                    let _ = cfg.save();
                    cfg.enabled
                };
                let ui_w = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_w.upgrade() {
                        ui.set_sound_on(is_on);
                    }
                });
            } else if event.id == sound_pick_id {
                let owner_hwnd = get_cockpit_hwnd() as isize;
                let cfg_arc = Arc::clone(&sound_cfg_menu);
                std::thread::spawn(move || {
                    let Some(picked) = crate::sound::pick_sound_file(owner_hwnd) else {
                        return;
                    };
                    {
                        let mut cfg = cfg_arc.lock().unwrap();
                        cfg.sound_file = Some(picked.to_string_lossy().to_string());
                        let _ = cfg.save();
                    }
                    crate::sound::play_done_sound(&picked);
                });
            } else {
                let target_slot = if event.id == sub1_id {
                    Some(1usize)
                } else if event.id == sub2_id {
                    Some(2usize)
                } else if event.id == sub3_id {
                    Some(3usize)
                } else if event.id == sub4_id {
                    Some(4usize)
                } else {
                    None
                };

                if let Some(slot) = target_slot {
                    let pid_lock = pid_lock.clone();
                    let mon_arc = Arc::clone(&mon_arc);
                    std::thread::spawn(move || {
                        let idx = slot - 1;
                        let running_pid = {
                            let mon = mon_arc.lock().unwrap();
                            let pids = pid_lock.lock().unwrap();
                            mon.subs[idx].electron_pid.or(pids[idx])
                        };
                        let Ok(c) = LauncherConfig::for_slot(slot) else {
                            return;
                        };
                        if let Some(pid) = running_pid {
                            LauncherConfig::bring_to_front(pid);
                        } else if let Ok(new_pid) = c.spawn_detached() {
                            let mut pids = pid_lock.lock().unwrap();
                            pids[idx] = Some(new_pid);
                        }
                    });
                }
            }
        }
    }));

    let sound_cfg_sound = Arc::clone(&sound_config);
    let main_weak_sound = main_window.as_weak();
    main_window.on_toggle_sound(move || {
        let (is_on, cfg_to_play) = {
            let mut cfg = sound_cfg_sound.lock().unwrap();
            cfg.enabled = !cfg.enabled;
            let _ = cfg.save();
            (cfg.enabled, cfg.clone())
        };
        if let Some(ui) = main_weak_sound.upgrade() {
            ui.set_sound_on(is_on);
        }
        if is_on {
            let _ = crate::sound::play_done_or_builtin(&cfg_to_play);
        }
    });

    main_window.window().on_close_requested({
        let ui_weak = main_window.as_weak();
        move || {
            #[cfg(target_os = "windows")]
            hide_cockpit_to_tray(&ui_weak);
            CloseRequestResponse::KeepWindowShown
        }
    });

    main_window.on_toggle_theme({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let next = !ui.get_is_dark();
                ui.set_is_dark(next);
                #[cfg(target_os = "windows")]
                {
                    let hwnd = get_cockpit_hwnd();
                    if !hwnd.is_null() {
                        if next {
                            apply_dark_window_attributes(hwnd);
                        } else {
                            apply_light_window_attributes(hwnd);
                        }
                    }
                }
            }
        }
    });

    main_window.on_toggle_pin_window({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let next = !ui.get_is_pinned();
                ui.set_is_pinned(next);
                #[cfg(target_os = "windows")]
                set_cockpit_always_on_top(next);
            }
        }
    });

    main_window.on_action_launch_host({
        let view_arc = Arc::clone(&monitor_view);
        move || {
            let pid_opt = view_arc.lock().unwrap().host.electron_pid;
            if pid_opt.is_none() {
                let _ = LauncherConfig::spawn_host_native();
            }
        }
    });

    main_window.on_action_stop_host({
        let view_arc = Arc::clone(&monitor_view);
        let mon_arc = Arc::clone(&monitor);
        move || {
            let (pid_opt, ls_pid) = {
                let view = view_arc.lock().unwrap();
                (view.host.electron_pid, view.host.ls_pid)
            };
            let Some(pid) = pid_opt.or(ls_pid) else {
                return;
            };
            let mon_bg = Arc::clone(&mon_arc);
            std::thread::spawn(move || {
                let res = LauncherConfig::kill_host_by_pid(pid, ls_pid);
                if res.is_ok() {
                    let mut mon = mon_bg.lock().unwrap();
                    mon.host.is_running = false;
                    mon.host.electron_pid = None;
                    mon.host.ls_pid = None;
                    mon.host.ls_port = None;
                    mon.host.ls_ports.clear();
                }
            });
        }
    });


    main_window.on_action_launch_sub({
        let view_arc = Arc::clone(&monitor_view);
        let sub_pids = Arc::clone(&sub_pids);
        move |slot| {
            if !(1..=4).contains(&slot) {
                return;
            }
            let idx = (slot - 1) as usize;
            let conf = match LauncherConfig::for_slot(slot as usize) {
                Ok(c) => c,
                Err(_) => return,
            };

            let running_pid = {
                let view = view_arc.lock().unwrap();
                let pids = sub_pids.lock().unwrap();
                view.subs[idx].electron_pid.or(pids[idx])
            };

            if running_pid.is_none() {
                if let Ok(new_pid) = conf.spawn_detached() {
                    let mut pids = sub_pids.lock().unwrap();
                    pids[idx] = Some(new_pid);
                }
            }
        }
    });

    main_window.on_action_stop_sub({
        let ui_weak = main_window.as_weak();
        let view_arc = Arc::clone(&monitor_view);
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move |slot| {
            if !(1..=4).contains(&slot) {
                return;
            }
            let idx = (slot - 1) as usize;
            if ui_weak.upgrade().is_none() {
                return;
            };
            let conf = match LauncherConfig::for_slot(slot as usize) {
                Ok(c) => c,
                Err(_) => return,
            };
            let (pid_to_kill, ls_pid) = {
                let view = view_arc.lock().unwrap();
                let pids = sub_pids.lock().unwrap();
                (view.subs[idx].electron_pid.or(pids[idx]), view.subs[idx].ls_pid)
            };
            let Some(pid) = pid_to_kill.or(ls_pid) else {
                return;
            };
            let ui_w = ui_weak.clone();
            let mon_bg = Arc::clone(&mon_arc);
            let pids_arc = Arc::clone(&sub_pids);
            std::thread::spawn(move || {
                let res = conf.kill_slot_by_pid(pid, ls_pid);
                if res.is_ok() {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_w.upgrade() else {
                            return;
                        };
                        pids_arc.lock().unwrap()[idx] = None;
                        {
                            let mut mon = mon_bg.lock().unwrap();
                            mon.subs[idx].is_running = false;
                            mon.subs[idx].electron_pid = None;
                            mon.subs[idx].ls_pid = None;
                            mon.subs[idx].ls_port = None;
                            mon.subs[idx].ls_ports.clear();
                        }
                        match slot {
                            1 => {
                                ui.set_sub1_running(false);
                                ui.set_sub1_pid("--".into());
                            }
                            2 => {
                                ui.set_sub2_running(false);
                                ui.set_sub2_pid("--".into());
                            }
                            3 => {
                                ui.set_sub3_running(false);
                                ui.set_sub3_pid("--".into());
                            }
                            4 => {
                                ui.set_sub4_running(false);
                                ui.set_sub4_pid("--".into());
                            }
                            _ => {}
                        }
                    });
                }
            });
        }
    });

    let reset_cleared_quota = {
        let qs = Arc::clone(&quota_states);
        let qc = Arc::clone(&quota_cache);
        let cleared = Arc::clone(&cleared_credentials);
        move |slot: usize| {
            if let Some(state) = qs.lock().unwrap().get_mut(slot) {
                *state = InstanceQuotaState::default();
            }
            {
                let mut cache = qc.lock().unwrap();
                cache.remove_slot(slot);
                crate::quota::save_quota_cache(&cache);
            }
            if let Some(flag) = cleared.lock().unwrap().get_mut(slot) {
                *flag = true;
            }
        }
    };

    main_window.on_action_clear_token({
        let mon_arc = Arc::clone(&monitor);
        move |slot| {
            if !(0..=4).contains(&slot) {
                return;
            }
            let running = {
                let mon = mon_arc.lock().unwrap();
                if slot == 0 {
                    mon.host.is_running
                } else {
                    mon.subs[(slot - 1) as usize].is_running
                }
            };
            if running {
                return;
            }
            if slot == 0 {
                if let Ok(true) = LauncherConfig::clear_host_token() {
                    reset_cleared_quota(0);
                }
                return;
            }
            if let Ok(conf) = LauncherConfig::for_slot(slot as usize) {
                if let Ok(true) = conf.clear_token() {
                    reset_cleared_quota(slot as usize);
                }
            }
        }
    });

    main_window.on_action_open_dir({
        move |slot| {
            if slot == 0 {
                let _ = LauncherConfig::open_app_dir_in_explorer();
            } else if (1..=4).contains(&slot)
                && let Ok(conf) = LauncherConfig::for_slot(slot as usize) {
                    let _ = conf.open_sandbox_in_explorer();
                }
        }
    });

    main_window.on_action_scan_cache({
        let ui_weak = main_window.as_weak();
        let view_arc = Arc::clone(&monitor_view);
        let cache_bytes = Arc::clone(&scanned_cache_bytes);
        move || {
            let running = {
                let view = view_arc.lock().unwrap();
                [
                    view.subs[0].is_running,
                    view.subs[1].is_running,
                    view.subs[2].is_running,
                    view.subs[3].is_running,
                ]
            };
            let ui_w = ui_weak.clone();
            let cache_bytes = Arc::clone(&cache_bytes);
            std::thread::spawn(move || {
                let found = LauncherConfig::scan_sandbox_cache(&running);
                let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
                *cache_bytes.lock().unwrap() = total_bytes;
                let (detail, total_text) = cache_report_text(&found);
                let summary = if total_bytes == 0 {
                    "无缓存占用".to_string()
                } else {
                    format!("分身缓存 ({:.0} MB)", total_bytes as f64 / (1024.0 * 1024.0))
                };
                let empty = found.is_empty();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_w.upgrade() else {
                        return;
                    };
                    if empty {
                        return;
                    }
                    ui.set_cache_detail_text(detail.into());
                    ui.set_cache_total_text(total_text.into());
                    ui.set_cache_summary_text(summary.into());
                    ui.set_clean_modal_visible(true);
                });
            });
        }
    });

    main_window.on_action_clean_cache({
        let ui_weak = main_window.as_weak();
        let view_arc = Arc::clone(&monitor_view);
        let cache_bytes = Arc::clone(&scanned_cache_bytes);
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.set_clean_modal_visible(false);
            let running = {
                let view = view_arc.lock().unwrap();
                [
                    view.subs[0].is_running,
                    view.subs[1].is_running,
                    view.subs[2].is_running,
                    view.subs[3].is_running,
                ]
            };
            let before_bytes = *cache_bytes.lock().unwrap();
            let ui_w = ui_weak.clone();
            std::thread::spawn(move || {
                let res = LauncherConfig::clean_sandbox_cache(&running);
                if let Ok(clean) = res {
                    let after_mb = before_bytes.saturating_sub(clean.freed_bytes) as f64 / (1024.0 * 1024.0);
                    let summary = if after_mb <= 0.0 {
                        "无缓存占用".to_string()
                    } else {
                        format!("分身缓存 ({:.0} MB)", after_mb)
                    };
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_w.upgrade() {
                            ui.set_cache_summary_text(summary.into());
                        }
                    });
                }
            });
        }
    });

    let pending_ag2 = Arc::clone(&pending_patch_action);
    let is_patching_ag2 = Arc::clone(&is_patching);
    main_window.on_toggle_patch_ag2({
        let ui_weak = main_window.as_weak();
        move || {
            if is_patching_ag2.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let ui_w = ui_weak.clone();
            let pending = Arc::clone(&pending_ag2);
            let flag = Arc::clone(&is_patching_ag2);
            std::thread::spawn(move || {
                if LauncherConfig::is_antigravity_running() {
                    *pending.lock().unwrap() = Some(PendingPatchAction::TogglePatchAg2);
                    flag.store(false, std::sync::atomic::Ordering::SeqCst);
                    let ui_modal = ui_w.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_modal.upgrade() {
                            ui.set_patch_modal_title("Antigravity 正在运行".into());
                            ui.set_patch_modal_msg("文件被运行中的实例占用，需退出后继续。".into());
                            ui.set_patch_modal_btn_text("关闭并应用".into());
                            ui.set_patch_modal_visible(true);
                        }
                    });
                    return;
                }
                let _ = LauncherConfig::toggle_ag2_patch();
                let (is_patched, ver) = LauncherConfig::check_ag2_patch_status();
                flag.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_w.upgrade() {
                        ui.set_patch_ag2_installed(is_patched);
                        ui.set_patch_ag2_version(ver.into());
                    }
                });
            });
        }
    });

    let pending_proxy = Arc::clone(&pending_patch_action);
    let is_patching_proxy = Arc::clone(&is_patching);
    main_window.on_toggle_proxy({
        let ui_weak = main_window.as_weak();
        move || {
            if is_patching_proxy.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                if ProxyManager::is_antigravity_running() {
                    *pending_proxy.lock().unwrap() = Some(PendingPatchAction::ToggleProxy);
                    ui.set_patch_modal_title("Antigravity 正在运行".into());
                    ui.set_patch_modal_msg("文件被运行中的实例占用，需退出后继续。".into());
                    ui.set_patch_modal_btn_text("关闭并应用".into());
                    ui.set_patch_modal_visible(true);
                    return;
                }
                let status = ProxyManager::check_status();
                if status.is_fully_deployed() {
                    if ProxyManager::remove_all().is_ok() {
                        let now = ProxyManager::check_status();
                        ui.set_proxy_injected(now.is_fully_deployed());
                        ui.set_proxy_status_text(now.status_text().into());
                    }
                } else if ProxyManager::deploy_all().is_ok() {
                    let now = ProxyManager::check_status();
                    ui.set_proxy_injected(now.is_fully_deployed());
                    ui.set_proxy_status_text(now.status_text().into());
                }
            }
        }
    });

    let pending_confirm = Arc::clone(&pending_patch_action);
    let is_patching_confirm = Arc::clone(&is_patching);
    main_window.on_action_patch_modal_confirm({
        let ui_weak = main_window.as_weak();
        move || {
            if is_patching_confirm.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let Some(action) = pending_confirm.lock().unwrap().take() else {
                is_patching_confirm.store(false, std::sync::atomic::Ordering::SeqCst);
                return;
            };
            let ui_weak_async = ui_weak.clone();
            let is_patching_bg = Arc::clone(&is_patching_confirm);
            std::thread::spawn(move || {
                match action {
                    PendingPatchAction::TogglePatchAg2 => {
                        let _ = LauncherConfig::kill_all_antigravity();
                        let _ = LauncherConfig::toggle_ag2_patch();
                        let (is_patched, ver) = LauncherConfig::check_ag2_patch_status();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_async.upgrade() {
                                ui.set_patch_ag2_installed(is_patched);
                                ui.set_patch_ag2_version(ver.into());
                            }
                        });
                    }
                    PendingPatchAction::ToggleProxy => {
                        let _ = LauncherConfig::kill_all_antigravity();
                        let status = ProxyManager::check_status();
                        if status.is_fully_deployed() {
                            let _ = ProxyManager::remove_all();
                            let now = ProxyManager::check_status();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_async.upgrade() {
                                    ui.set_proxy_injected(now.is_fully_deployed());
                                    ui.set_proxy_status_text(now.status_text().into());
                                }
                            });
                        } else {
                            let _ = ProxyManager::deploy_all();
                            let now = ProxyManager::check_status();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_async.upgrade() {
                                    ui.set_proxy_injected(now.is_fully_deployed());
                                    ui.set_proxy_status_text(now.status_text().into());
                                }
                            });
                        }
                    }
                }
                is_patching_bg.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        }
    });

    main_window.on_adjust_host_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |delta| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                scales.adjust_host(delta);
                let _ = scales.save();
                ui.set_host_scale_text(format!("{}%", scales.host).into());
            }
        }
    });

    main_window.on_reset_host_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                scales.host = 100;
                let _ = scales.save();
                ui.set_host_scale_text("100%".into());
            }
        }
    });

    main_window.on_adjust_sub_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |slot, delta| {
            if !(1..=4).contains(&slot) {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                scales.adjust_sub(slot as usize, delta);
                let _ = scales.save();
                let val = scales.get_sub_scale(slot as usize);
                let text = format!("{}%", val);
                match slot {
                    1 => ui.set_sub1_scale_text(text.into()),
                    2 => ui.set_sub2_scale_text(text.into()),
                    3 => ui.set_sub3_scale_text(text.into()),
                    4 => ui.set_sub4_scale_text(text.into()),
                    _ => {}
                }
            }
        }
    });

    main_window.on_reset_sub_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |slot| {
            if !(1..=4).contains(&slot) {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                match slot {
                    1 => scales.sub1 = 100,
                    2 => scales.sub2 = 100,
                    3 => scales.sub3 = 100,
                    4 => scales.sub4 = 100,
                    _ => {}
                }
                let _ = scales.save();
                match slot {
                    1 => ui.set_sub1_scale_text("100%".into()),
                    2 => ui.set_sub2_scale_text("100%".into()),
                    3 => ui.set_sub3_scale_text("100%".into()),
                    4 => ui.set_sub4_scale_text("100%".into()),
                    _ => {}
                }
            }
        }
    });

    let sessions_store_modal = Arc::clone(&sessions_store);
    main_window.on_show_session_modal({
        let ui_weak = main_window.as_weak();
        move |slot| {
            if let Some(ui) = ui_weak.upgrade() {
                let slot_idx = (slot as usize).clamp(0, 4);
                let store = sessions_store_modal.lock().unwrap();
                let model = to_session_ui_model(&store[slot_idx].groups);
                ui.set_session_modal_slot(slot);
                ui.set_session_modal_items(model);
                ui.set_session_modal_visible(true);
            }
        }
    });

    let live_slot_cb = Arc::clone(&live_session_slot);
    main_window.on_switch_live_session_target({
        let ui_weak = main_window.as_weak();
        move |_| {
            let mut slot_val = *live_slot_cb.lock().unwrap();
            slot_val = if !(-1..4).contains(&slot_val) { -1 } else { slot_val + 1 };
            *live_slot_cb.lock().unwrap() = slot_val;
            if let Some(ui) = ui_weak.upgrade() {
                let target_text = match slot_val {
                    -1 => "自动跟随",
                    0 => "主机",
                    1 => "分身 1",
                    2 => "分身 2",
                    3 => "分身 3",
                    4 => "分身 4",
                    _ => "自动跟随",
                };
                ui.set_live_session_target_slot(slot_val);
                ui.set_live_session_target_text(target_text.into());
            }
        }
    });

    let active_hover = Arc::clone(&active_live_session);
    let hover_state = Arc::clone(&hover_bar_state);
    let ui_weak_hover = main_window.as_weak();
    main_window.on_bar_hover(move |idx| {
        let i = idx as usize;
        *hover_state.lock().unwrap() = Some(i);
        let Some(ui) = ui_weak_hover.upgrade() else { return; };
        let sess = active_hover.lock().unwrap();
        if let Some(ref s) = *sess
            && let Some(&turn_idx) = s.recent_turn_indices.get(i)
                && turn_idx > 0 {
                    ui.set_live_is_hovering_bar(true);
                    ui.set_live_hover_tag(format!("#{} 轮", turn_idx).into());
                    ui.set_live_session_speed_text(format_tps_display(s.recent_tps[i]).into());
                    ui.set_live_session_speed_sub(format!("该轮产出 {} · {:.1}s", format_tokens(s.recent_turn_tokens[i]), s.recent_turn_durations[i]).into());
                }
    });

    let active_leave = Arc::clone(&active_live_session);
    let hover_leave = Arc::clone(&hover_bar_state);
    let ui_weak_leave = main_window.as_weak();
    main_window.on_bar_leave(move || {
        *hover_leave.lock().unwrap() = None;
        let Some(ui) = ui_weak_leave.upgrade() else { return; };
        ui.set_live_is_hovering_bar(false);
        ui.set_live_hover_tag("".into());
        let sess = active_leave.lock().unwrap();
        if let Some(ref s) = *sess {
            if s.is_running {
                if s.round_tps > 0.0 {
                    ui.set_live_session_speed_text(format_tps_display(s.round_tps).into());
                    if s.ttft_secs > 0.0 {
                        ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 首字 {:.1}s", s.round_duration_secs, s.ttft_secs).into());
                    } else {
                        ui.set_live_session_speed_sub(format!("耗时 {:.1}s", s.round_duration_secs).into());
                    }
                } else {
                    ui.set_live_session_speed_text("--".into());
                    ui.set_live_session_speed_sub(format!("生成中 · 步数 #{}", s.step_count).into());
                }
            } else {
                ui.set_live_session_speed_text(format_tps_display(s.round_tps).into());
                if s.ttft_secs > 0.0 {
                    ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 首字 {:.1}s · 输出 {}", s.round_duration_secs, s.ttft_secs, format_tokens(s.output_tokens)).into());
                } else {
                    ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 输出 {}", s.round_duration_secs, format_tokens(s.output_tokens)).into());
                }
            }
        } else {
            ui.set_live_session_speed_text("0.0".into());
            ui.set_live_session_speed_sub("无吞吐".into());
        }
    });

    let ui_weak_cache = main_window.as_weak();
    std::thread::spawn(move || {
        let found = LauncherConfig::scan_sandbox_cache(&[false, false, false, false]);
        let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
        let mb = total_bytes as f64 / (1024.0 * 1024.0);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_cache.upgrade() {
                let summary = if mb <= 0.0 {
                    "无缓存占用".to_string()
                } else {
                    format!("分身缓存 ({:.0} MB)", mb)
                };
                ui.set_cache_summary_text(summary.into());
                for item in &found {
                    let size_str = format!("缓存 {:.0} MB", item.bytes as f64 / (1024.0 * 1024.0));
                    match item.slot {
                        1 => ui.set_sub1_cache_size_text(size_str.into()),
                        2 => ui.set_sub2_cache_size_text(size_str.into()),
                        3 => ui.set_sub3_cache_size_text(size_str.into()),
                        4 => ui.set_sub4_cache_size_text(size_str.into()),
                        _ => {}
                    }
                }
            }
        });
    });

    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let mon_bg = Arc::clone(&monitor);
    let qs_bg = Arc::clone(&quota_states);
    let qc_bg = Arc::clone(&quota_cache);
    let shutdown_quota = Arc::clone(&shutdown);
    let quota_worker = std::thread::spawn(move || {
        loop {
            if shutdown_quota.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let mut fetched_any = false;
            for idx in 0..5 {
                let Some((ports, csrf)) = slot_rpc_ports(&mon_bg, idx) else {
                    continue;
                };
                let tag = if idx == 0 { "host".to_string() } else { format!("sub{}", idx) };
                let st = crate::quota::fetch_instance_quota(&ports, &csrf, &tag);
                if !st.is_available {
                    continue;
                }
                {
                    let mut qs = qs_bg.lock().unwrap();
                    qs[idx] = st.clone();
                }
                {
                    let mut qc = qc_bg.lock().unwrap();
                    if crate::quota::quota_state_changed(qc.get_slot(idx), &st) {
                        qc.set_slot(idx, st);
                        crate::quota::save_quota_cache(&qc);
                    }
                }
                fetched_any = true;
            }

            let sleep_ms = if fetched_any { 3000 } else { 800 };
            interruptible_sleep(sleep_ms, &shutdown_quota);
        }
    });

    let bg_telemetry = Arc::new(Mutex::new((
        LauncherConfig::check_ag2_patch_status(),
        ProxyManager::check_status(),
    )));

    let bg_telem_worker = Arc::clone(&bg_telemetry);
    let sessions_store_worker = Arc::clone(&sessions_store);
    let mon_sessions_worker = Arc::clone(&monitor);
    let shutdown_sessions = Arc::clone(&shutdown);
    let wake_sessions = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wake_sessions_timer = Arc::clone(&wake_sessions);
    {
        let warm = collect_session_snapshot(&monitor, false);
        *sessions_store.lock().unwrap() = warm;
    }
    let session_worker = std::thread::spawn(move || {
        let mut idle_rounds = 0u32;
        loop {
            if shutdown_sessions.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let snapshot = collect_session_snapshot(&mon_sessions_worker, true);

            let all_idle = snapshot
                .iter()
                .all(|s| s.total_sessions == 0 && !s.has_running);
            idle_rounds = if all_idle { idle_rounds + 1 } else { 0 };

            {
                let mut store = sessions_store_worker.lock().unwrap();
                *store = snapshot;
            }

            let patch = LauncherConfig::check_ag2_patch_status();
            let proxy = ProxyManager::check_status();
            {
                let mut telem = bg_telem_worker.lock().unwrap();
                *telem = (patch, proxy);
            }

            let scan_interval = if idle_rounds >= 6 {
                3000
            } else if idle_rounds >= 3 {
                2000
            } else {
                1500
            };
            interruptible_sleep_wake(scan_interval, &shutdown_sessions, &wake_sessions);
        }
    });

    let bg_telem_timer = Arc::clone(&bg_telemetry);
    let qs_timer = Arc::clone(&quota_states);
    let qc_timer = Arc::clone(&quota_cache);
    let cleared_timer = Arc::clone(&cleared_credentials);
    let timer = Timer::default();
    let ui_weak_mon = main_window.as_weak();
    let monitor_view_timer = Arc::clone(&monitor_view);
    let sessions_store_timer = Arc::clone(&sessions_store);
    let live_session_slot_timer = Arc::clone(&live_session_slot);
    let active_live_timer = Arc::clone(&active_live_session);
    let hover_bar_timer = Arc::clone(&hover_bar_state);
    let sound_config_timer = Arc::clone(&sound_config);
    let sound_detector = Arc::new(Mutex::new(crate::sound::StateTransitionDetector::new()));

    let monitor_bg = Arc::clone(&monitor);
    let sub_pids_bg = Arc::clone(&sub_pids);
    let view_bg = Arc::clone(&monitor_view);
    std::thread::spawn(move || {
        loop {
            let known_subs = *sub_pids_bg.lock().unwrap();
            {
                let mut mon = monitor_bg.lock().unwrap();
                mon.refresh(known_subs);
                let mut pids = sub_pids_bg.lock().unwrap();
                for i in 0..4 {
                    if let Some(pid) = mon.subs[i].electron_pid {
                        pids[i] = Some(pid);
                    } else if !mon.subs[i].is_running {
                        pids[i] = None;
                    }
                }
                *view_bg.lock().unwrap() = mon.view();
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });

    #[cfg(windows)]
    let restore_timer = Timer::default();
    #[cfg(windows)]
    let ui_weak_restore = main_window.as_weak();
    #[cfg(windows)]
    let mut was_iconic = false;
    #[cfg(windows)]
    let mut configured_dark = false;
    #[cfg(windows)]
    let mut cached_hwnd: windows_sys::Win32::Foundation::HWND = std::ptr::null_mut();

    #[cfg(windows)]
    restore_timer.start(TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
        if unsafe {
            windows_sys::Win32::System::Threading::WaitForSingleObject(activation_event, 0)
        } == 0
        {
            show_cockpit_from_tray(&ui_weak_restore);
            wake_sessions_timer.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        if cached_hwnd.is_null()
            || unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(cached_hwnd) == 0 }
        {
            cached_hwnd = get_cockpit_hwnd();
            if cached_hwnd.is_null() {
                return;
            }
        }
        let hwnd = cached_hwnd;
        if !configured_dark {
            apply_dark_window_attributes(hwnd);
            let scale = ui_weak_restore.upgrade().map(|ui| ui.window().scale_factor()).unwrap_or(1.0);
            center_and_scale_cockpit_window(hwnd, scale);
            configured_dark = true;
        }
        let is_iconic = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(hwnd) != 0
        };
        if was_iconic && !is_iconic {
            let Some(ui) = ui_weak_restore.upgrade() else {
                was_iconic = is_iconic;
                return;
            };
            if ui.get_is_dark() {
                apply_dark_window_attributes(hwnd);
            } else {
                apply_light_window_attributes(hwnd);
            }
            unsafe {
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            let cur = ui.get_redraw_tick();
            ui.set_redraw_tick(if cur == 0 { 1 } else { 0 });
            ui.window().request_redraw();
            wake_sessions_timer.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        was_iconic = is_iconic;
    });

    let ui_tick = move || {

        if let Some(ui) = ui_weak_mon.upgrade() {
            let mon = monitor_view_timer.lock().unwrap().clone();

            ui.set_host_running(mon.host.is_running);
            ui.set_host_pid(mon.host.electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_host_port(mon.host.ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());

            ui.set_host_token_present(LauncherConfig::is_host_token_present());

            ui.set_sub1_running(mon.subs[0].is_running);
            ui.set_sub1_pid(mon.subs[0].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub1_port(mon.subs[0].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub1_token_present(LauncherConfig::is_slot_token_present(1));

            ui.set_sub2_running(mon.subs[1].is_running);
            ui.set_sub2_pid(mon.subs[1].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub2_port(mon.subs[1].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub2_token_present(LauncherConfig::is_slot_token_present(2));

            ui.set_sub3_running(mon.subs[2].is_running);
            ui.set_sub3_pid(mon.subs[2].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub3_port(mon.subs[2].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub3_token_present(LauncherConfig::is_slot_token_present(3));

            ui.set_sub4_running(mon.subs[3].is_running);
            ui.set_sub4_pid(mon.subs[3].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub4_port(mon.subs[3].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub4_token_present(LauncherConfig::is_slot_token_present(4));


            let is_dark = ui.get_is_dark();
            let qs_snapshot = qs_timer.lock().unwrap().clone();
            let cleared_snapshot = *cleared_timer.lock().unwrap();

            let host_token = LauncherConfig::resolve_host_user_dir()
                .map(|u| u.join(".gemini").join("jetski-standalone-oauth-token"));
            let host_state = if mon.host.is_running && qs_snapshot[0].is_available {
                Some(qs_snapshot[0].clone())
            } else if cleared_snapshot[0] {
                None
            } else {
                let qc = qc_timer.lock().unwrap();
                crate::quota::compute_offline_state(qc.get_slot(0), host_token.as_deref(), "host")
            };

            apply_quota_to_ui(
                host_state.as_ref(),
                is_dark,
                |name, email, tier, avatar| {
                    ui.set_host_user_name(name.into());
                    ui.set_host_user_email(email.into());
                    ui.set_host_tier(tier.into());
                    let (img, has_img) = load_avatar_image(avatar);
                    ui.set_host_avatar(img);
                    ui.set_host_has_avatar(has_img);
                },
                |rows| {
                    ui.set_host_model_quotas(rows);
                },
            );

            let sub1_token = LauncherConfig::slot_token_path(1);
            let sub1_state = if mon.subs[0].is_running && qs_snapshot[1].is_available {
                Some(qs_snapshot[1].clone())
            } else if cleared_snapshot[1] {
                None
            } else {
                let qc = qc_timer.lock().unwrap();
                crate::quota::compute_offline_state(qc.get_slot(1), sub1_token.as_deref(), "sub1")
            };
            apply_quota_to_ui(
                sub1_state.as_ref(),
                is_dark,
                |name, email, tier, avatar| {
                    ui.set_sub1_user_name(name.into());
                    ui.set_sub1_user_email(email.into());
                    ui.set_sub1_tier(tier.into());
                    let (img, has_img) = load_avatar_image(avatar);
                    ui.set_sub1_avatar(img);
                    ui.set_sub1_has_avatar(has_img);
                },
                |rows| {
                    ui.set_sub1_model_quotas(rows);
                },
            );

            let sub2_token = LauncherConfig::slot_token_path(2);
            let sub2_state = if mon.subs[1].is_running && qs_snapshot[2].is_available {
                Some(qs_snapshot[2].clone())
            } else if cleared_snapshot[2] {
                None
            } else {
                let qc = qc_timer.lock().unwrap();
                crate::quota::compute_offline_state(qc.get_slot(2), sub2_token.as_deref(), "sub2")
            };
            apply_quota_to_ui(
                sub2_state.as_ref(),
                is_dark,
                |name, email, tier, avatar| {
                    ui.set_sub2_user_name(name.into());
                    ui.set_sub2_user_email(email.into());
                    ui.set_sub2_tier(tier.into());
                    let (img, has_img) = load_avatar_image(avatar);
                    ui.set_sub2_avatar(img);
                    ui.set_sub2_has_avatar(has_img);
                },
                |rows| {
                    ui.set_sub2_model_quotas(rows);
                },
            );

            let sub3_token = LauncherConfig::slot_token_path(3);
            let sub3_state = if mon.subs[2].is_running && qs_snapshot[3].is_available {
                Some(qs_snapshot[3].clone())
            } else if cleared_snapshot[3] {
                None
            } else {
                let qc = qc_timer.lock().unwrap();
                crate::quota::compute_offline_state(qc.get_slot(3), sub3_token.as_deref(), "sub3")
            };
            apply_quota_to_ui(
                sub3_state.as_ref(),
                is_dark,
                |name, email, tier, avatar| {
                    ui.set_sub3_user_name(name.into());
                    ui.set_sub3_user_email(email.into());
                    ui.set_sub3_tier(tier.into());
                    let (img, has_img) = load_avatar_image(avatar);
                    ui.set_sub3_avatar(img);
                    ui.set_sub3_has_avatar(has_img);
                },
                |rows| {
                    ui.set_sub3_model_quotas(rows);
                },
            );

            let sub4_token = LauncherConfig::slot_token_path(4);
            let sub4_state = if mon.subs[3].is_running && qs_snapshot[4].is_available {
                Some(qs_snapshot[4].clone())
            } else if cleared_snapshot[4] {
                None
            } else {
                let qc = qc_timer.lock().unwrap();
                crate::quota::compute_offline_state(qc.get_slot(4), sub4_token.as_deref(), "sub4")
            };
            apply_quota_to_ui(
                sub4_state.as_ref(),
                is_dark,
                |name, email, tier, avatar| {
                    ui.set_sub4_user_name(name.into());
                    ui.set_sub4_user_email(email.into());
                    ui.set_sub4_tier(tier.into());
                    let (img, has_img) = load_avatar_image(avatar);
                    ui.set_sub4_avatar(img);
                    ui.set_sub4_has_avatar(has_img);
                },
                |rows| {
                    ui.set_sub4_model_quotas(rows);
                },
            );

            let (ag2_installed, ag2_ver, p_status) = {
                let telem = bg_telem_timer.lock().unwrap();
                (telem.0.0, telem.0.1.clone(), telem.1.clone())
            };
            ui.set_patch_ag2_installed(ag2_installed);
            ui.set_patch_ag2_version(ag2_ver.into());

            ui.set_proxy_injected(p_status.is_fully_deployed());
            ui.set_proxy_status_text(p_status.status_text().into());


            let (host_sessions, sub1_sessions, sub2_sessions, sub3_sessions, sub4_sessions) = {
                let store = sessions_store_timer.lock().unwrap();
                (store[0].clone(), store[1].clone(), store[2].clone(), store[3].clone(), store[4].clone())
            };

            ui.set_host_has_running(host_sessions.has_running);
            ui.set_host_has_sessions(host_sessions.total_sessions > 0);
            ui.set_host_session_count_text(if host_sessions.total_sessions > 0 { format!("{} 个", host_sessions.total_sessions).into() } else { "--".into() });
            let (h_p, h_title, h_t, h_time) = extract_recent_session_info(&host_sessions);
            ui.set_host_recent_project(h_p.into());
            ui.set_host_recent_title(h_title.into());
            ui.set_host_recent_tokens(h_t.into());
            ui.set_host_recent_time(h_time.into());
            ui.set_host_session_items(to_recent_session_model(&host_sessions, 16));

            ui.set_sub1_has_running(sub1_sessions.has_running);
            ui.set_sub1_has_sessions(sub1_sessions.total_sessions > 0);
            ui.set_sub1_session_count_text(if sub1_sessions.total_sessions > 0 { format!("{} 个", sub1_sessions.total_sessions).into() } else { "--".into() });
            let (s1_p, s1_title, s1_t, s1_time) = extract_recent_session_info(&sub1_sessions);
            ui.set_sub1_recent_project(s1_p.into());
            ui.set_sub1_recent_title(s1_title.into());
            ui.set_sub1_recent_tokens(s1_t.into());
            ui.set_sub1_recent_time(s1_time.into());
            ui.set_sub1_session_items(to_recent_session_model(&sub1_sessions, 16));

            ui.set_sub2_has_running(sub2_sessions.has_running);
            ui.set_sub2_has_sessions(sub2_sessions.total_sessions > 0);
            ui.set_sub2_session_count_text(if sub2_sessions.total_sessions > 0 { format!("{} 个", sub2_sessions.total_sessions).into() } else { "--".into() });
            let (s2_p, s2_title, s2_t, s2_time) = extract_recent_session_info(&sub2_sessions);
            ui.set_sub2_recent_project(s2_p.into());
            ui.set_sub2_recent_title(s2_title.into());
            ui.set_sub2_recent_tokens(s2_t.into());
            ui.set_sub2_recent_time(s2_time.into());
            ui.set_sub2_session_items(to_recent_session_model(&sub2_sessions, 16));

            ui.set_sub3_has_running(sub3_sessions.has_running);
            ui.set_sub3_has_sessions(sub3_sessions.total_sessions > 0);
            ui.set_sub3_session_count_text(if sub3_sessions.total_sessions > 0 { format!("{} 个", sub3_sessions.total_sessions).into() } else { "--".into() });
            let (s3_p, s3_title, s3_t, s3_time) = extract_recent_session_info(&sub3_sessions);
            ui.set_sub3_recent_project(s3_p.into());
            ui.set_sub3_recent_title(s3_title.into());
            ui.set_sub3_recent_tokens(s3_t.into());
            ui.set_sub3_recent_time(s3_time.into());
            ui.set_sub3_session_items(to_recent_session_model(&sub3_sessions, 16));

            ui.set_sub4_has_running(sub4_sessions.has_running);
            ui.set_sub4_has_sessions(sub4_sessions.total_sessions > 0);
            ui.set_sub4_session_count_text(if sub4_sessions.total_sessions > 0 { format!("{} 个", sub4_sessions.total_sessions).into() } else { "--".into() });
            let (s4_p, s4_title, s4_t, s4_time) = extract_recent_session_info(&sub4_sessions);
            ui.set_sub4_recent_project(s4_p.into());
            ui.set_sub4_recent_title(s4_title.into());
            ui.set_sub4_recent_tokens(s4_t.into());
            ui.set_sub4_recent_time(s4_time.into());
            ui.set_sub4_session_items(to_recent_session_model(&sub4_sessions, 16));

            let is_modal_visible = ui.get_session_modal_visible();
            let current_modal_slot = ui.get_session_modal_slot() as usize;

            {
                let store = sessions_store_timer.lock().unwrap();

                if is_modal_visible && current_modal_slot <= 4 {
                    let model = to_session_ui_model(&store[current_modal_slot].groups);
                    ui.set_session_modal_items(model);
                }

                let target_slot_setting = *live_session_slot_timer.lock().unwrap();
                let (effective_slot, is_auto) = if !(0..=4).contains(&target_slot_setting) {
                    let running_slot = (0..5).find(|&i| store[i].has_running);
                    let chosen = if let Some(r) = running_slot {
                        r
                    } else {
                        (0..5)
                            .filter(|&i| store[i].total_sessions > 0)
                            .min_by_key(|&i| {
                                store[i]
                                    .groups
                                    .first()
                                    .map(|g| g.latest_secs_ago)
                                    .unwrap_or(u64::MAX)
                            })
                            .unwrap_or(0)
                    };
                    (chosen, true)
                } else {
                    (target_slot_setting as usize, false)
                };
                let effective_slot = effective_slot.min(4);

                let slot_label = match effective_slot {
                    0 => "主机",
                    1 => "分身 1",
                    2 => "分身 2",
                    3 => "分身 3",
                    4 => "分身 4",
                    _ => "主机",
                };

                if is_auto {
                    ui.set_live_session_target_text(format!("自动 ({})", slot_label).into());
                } else {
                    ui.set_live_session_target_text(slot_label.into());
                }

                let target_sessions = &store[effective_slot];
                let active = target_sessions
                    .groups
                    .iter()
                    .flat_map(|g| g.sessions.iter())
                    .find(|s| s.is_running)
                    .or_else(|| {
                        target_sessions
                            .groups
                            .first()
                            .and_then(|g| g.sessions.first())
                    })
                    .cloned();

                *active_live_timer.lock().unwrap() = active.clone();
                let is_hovering = hover_bar_timer.lock().unwrap().is_some();

                if let Some(s) = active {
                    ui.set_live_session_is_running(s.is_running);
                    ui.set_live_session_status_text(if s.is_running { "运行中".into() } else { "".into() });
                    ui.set_live_session_project(s.project_name.as_str().into());
                    ui.set_live_session_title(s.title.as_str().into());
                    ui.set_live_model_name(if s.model_name.is_empty() { "--".into() } else { s.model_name.as_str().into() });
                    ui.set_live_model_level(if s.model_name.is_empty() || s.thinking_level == "--" { "".into() } else { s.thinking_level.as_str().into() });

                    if !is_hovering {
                        if s.is_running {
                            if s.round_tps > 0.0 {
                                ui.set_live_session_speed_text(format_tps_display(s.round_tps).into());
                                if s.ttft_secs > 0.0 {
                                    ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 首字 {:.1}s", s.round_duration_secs, s.ttft_secs).into());
                                } else {
                                    ui.set_live_session_speed_sub(format!("耗时 {:.1}s", s.round_duration_secs).into());
                                }
                            } else {
                                ui.set_live_session_speed_text("--".into());
                                ui.set_live_session_speed_sub(format!("生成中 · 步数 #{}", s.step_count).into());
                            }
                        } else {
                            ui.set_live_session_speed_text(format_tps_display(s.round_tps).into());
                            if s.ttft_secs > 0.0 {
                                ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 首字 {:.1}s · 输出 {}", s.round_duration_secs, s.ttft_secs, format_tokens(s.output_tokens)).into());
                            } else {
                                ui.set_live_session_speed_sub(format!("耗时 {:.1}s · 输出 {}", s.round_duration_secs, format_tokens(s.output_tokens)).into());
                            }
                        }
                        ui.set_live_is_hovering_bar(false);
                        ui.set_live_hover_tag("".into());
                    }

                    let max_tps = s.recent_tps.iter().copied().fold(0.0f32, f32::max).max(10.0);
                    let calc_ratio = |tps: f32| -> f32 { (tps / max_tps).clamp(0.0, 1.0) };
                    let bars: Vec<f32> = s.recent_tps.iter().map(|&v| calc_ratio(v)).collect();
                    ui.set_live_bars(std::rc::Rc::new(slint::VecModel::from(bars)).into());

                    ui.set_live_peak_tps_text(format!("峰值 {:.1}", s.peak_tps).into());
                    ui.set_live_avg_tps_text(format!("Avg {:.0}", s.avg_tps).into());
                    let avg_pct = if max_tps > 0.0 {
                        (s.avg_tps / max_tps).clamp(0.05, 0.95)
                    } else {
                        0.5
                    };
                    ui.set_live_avg_line_pct(avg_pct);

                    let max_tokens = 256_000u64;
                    let prompt_tokens = s.input_tokens;
                    let cache_tokens = s.cache_read_tokens;
                    let output_tokens = s.output_tokens;
                    let total_tokens = s.context_tokens;

                    let prompt_pct = (prompt_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    let cache_pct = (cache_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    let output_pct = (output_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);

                    ui.set_live_prompt_pct(prompt_pct);
                    ui.set_live_cache_pct(cache_pct);
                    ui.set_live_output_pct(output_pct);

                    ui.set_live_prompt_text(format!("输入 {}", format_tokens(prompt_tokens)).into());
                    ui.set_live_cache_legend(format!("缓存读取 {}", format_tokens(cache_tokens)).into());
                    ui.set_live_output_text(format!("输出 {}", format_tokens(output_tokens)).into());

                    if total_tokens > max_tokens {
                        ui.set_live_session_context_text(format!("{} / 256K (待压缩)", format_tokens(total_tokens)).into());
                        ui.set_live_session_context_pct(1.0);
                    } else {
                        let pct = (total_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                        ui.set_live_session_context_text(format!("{} / 256K ({:.1}%)", format_tokens(total_tokens), pct * 100.0).into());
                        ui.set_live_session_context_pct(pct);
                    }

                    ui.set_live_cache_hit_text(format!("缓存命中率 {}", s.cache_rate_str).into());
                    ui.set_live_step_text(format!("步数 #{}", s.step_count).into());
                    ui.set_live_compact_text(s.compact_str.clone().into());
                } else {
                    ui.set_live_prompt_pct(0.0);
                    ui.set_live_cache_pct(0.0);
                    ui.set_live_output_pct(0.0);
                    ui.set_live_prompt_text("输入 0".into());
                    ui.set_live_cache_legend("缓存读取 0".into());
                    ui.set_live_output_text("输出 0".into());

                    ui.set_live_peak_tps_text("峰值 0.0".into());
                    ui.set_live_avg_tps_text("Avg 0".into());
                    ui.set_live_avg_line_pct(0.5);

                    ui.set_live_bars(std::rc::Rc::new(slint::VecModel::from(vec![0.0f32; crate::session::HISTORY_LEN])).into());

                    ui.set_live_session_is_running(false);
                    ui.set_live_session_status_text("".into());
                    ui.set_live_session_project("--".into());
                    ui.set_live_session_title("无活动会话".into());
                    ui.set_live_model_name("--".into());
                    ui.set_live_model_level("".into());
                    ui.set_live_session_speed_text("0.0".into());
                    ui.set_live_session_speed_sub("无吞吐".into());
                    ui.set_live_session_context_text("0 / 256K (0.0%)".into());
                    ui.set_live_session_context_pct(0.0);
                    ui.set_live_cache_hit_text("缓存命中率 0.0%".into());
                    ui.set_live_step_text("步数 #0".into());
                    ui.set_live_compact_text("压缩 0 次".into());
                    ui.set_live_is_hovering_bar(false);
                    ui.set_live_hover_tag("".into());
                }
            }

            let store = sessions_store_timer.lock().unwrap();
            let mut slot_inputs = Vec::new();
            for slot in 0..=4 {
                let running = store[slot].has_running;
                let tokens: u64 = store[slot].groups.iter().flat_map(|g| &g.sessions).map(|s| s.output_tokens).sum();
                let tps = store[slot].groups.iter().flat_map(|g| &g.sessions).map(|s| s.round_tps).fold(0.0f32, f32::max);
                slot_inputs.push((slot as u8, running, tokens, tps));
            }

            let mut detector = sound_detector.lock().unwrap();
            let transitions = detector.update(&slot_inputs);
            for tr in transitions {
                if let crate::sound::SessionTransition::Finished { .. } = tr {
                    let cfg = sound_config_timer.lock().unwrap().clone();
                    let _ = crate::sound::play_done_or_builtin(&cfg);
                }
            }
        }
    };

    main_window.show()?;
    ui_tick();
    timer.start(TimerMode::Repeated, std::time::Duration::from_millis(1000), ui_tick);
    #[cfg(target_os = "windows")]
    {
        let hwnd = get_cockpit_hwnd();
        apply_dark_window_attributes(hwnd);
        apply_window_icon(hwnd);
        let scale = main_window.window().scale_factor();
        center_and_scale_cockpit_window(hwnd, scale);
    }

    let _ = slint::run_event_loop_until_quit();
    shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = quota_worker.join();
    let _ = session_worker.join();
    Ok(())
}

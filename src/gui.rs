use crate::launcher::{LauncherConfig, ScaleSettings};
use crate::monitor::TelemetryMonitor;
use crate::proxy::ProxyManager;
use crate::quota::InstanceQuotaState;
use crate::session::{format_tokens, scan_sandbox_sessions, InstanceSessions};
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

fn apply_quota_to_ui(
    state: Option<&InstanceQuotaState>,
    is_dark: bool,
    set_profile: impl FnOnce(&str, &str, &str, Option<&std::path::Path>),
    set_weekly: impl FnOnce(&str, &str, slint::Image),
    set_five_h: impl FnOnce(&str, &str, slint::Image),
) {
    if let Some(st) = state {
        if st.is_available {
            set_profile(
                &st.profile.name,
                &st.profile.email,
                &st.profile.tier,
                st.profile.avatar_path.as_deref(),
            );
            let weekly_ring = if st.weekly.remaining_pct >= 0.0 && !st.weekly.pct_text.is_empty() && st.weekly.pct_text != "--" {
                crate::quota::render_quota_ring(st.weekly.remaining_pct, is_dark)
            } else {
                crate::quota::render_quota_ring(-1.0, is_dark)
            };
            let weekly_pct = if st.weekly.pct_text.is_empty() { "--" } else { &st.weekly.pct_text };
            set_weekly(weekly_pct, &st.weekly.reset_text, weekly_ring);

            let five_h_ring = if st.five_h.remaining_pct >= 0.0 && !st.five_h.pct_text.is_empty() && st.five_h.pct_text != "--" {
                crate::quota::render_quota_ring(st.five_h.remaining_pct, is_dark)
            } else {
                crate::quota::render_quota_ring(-1.0, is_dark)
            };
            let five_h_pct = if st.five_h.pct_text.is_empty() { "--" } else { &st.five_h.pct_text };
            set_five_h(five_h_pct, &st.five_h.reset_text, five_h_ring);
            return;
        }
    }
    set_profile("--", "--", "", None);
    set_weekly("--", "", crate::quota::render_quota_ring(-1.0, is_dark));
    set_five_h("--", "", crate::quota::render_quota_ring(-1.0, is_dark));
}

fn load_avatar_image(path: Option<&std::path::Path>) -> (slint::Image, bool) {
    if let Some(p) = path {
        if p.exists() {
            if let Ok(img) = slint::Image::load_from_path(p) {
                if img.size().width > 0 {
                    return (img, true);
                }
            }
        }
    }
    (slint::Image::default(), false)
}

fn extract_recent_session_info(sessions: &InstanceSessions) -> (String, String, String) {
    if let Some(s) = sessions.groups.first().and_then(|g| g.sessions.first()) {
        (
            s.project_name.clone(),
            format!("{} ({})", s.context_str, s.cache_rate_str),
            format!("最近更新: {}", s.time_ago_str),
        )
    } else {
        ("无活动会话".to_string(), "".to_string(), "等待初始化".to_string())
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
        EnumWindows, GetClassNameW, GetWindowThreadProcessId,
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
                        let out_ptr = lparam as *mut HWND;
                        *out_ptr = hwnd;
                        return 0;
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
fn center_and_scale_cockpit_window(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SystemParametersInfoW, SPI_GETWORKAREA, SWP_FRAMECHANGED, SWP_NOZORDER,
    };

    if hwnd.is_null() {
        return;
    }

    unsafe {
        let mut work_area: RECT = std::mem::zeroed();
        let ok = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut work_area as *mut _ as *mut std::ffi::c_void,
            0,
        );
        if ok != 0 {
            let work_w = work_area.right - work_area.left;
            let work_h = work_area.bottom - work_area.top;
            let target_w = (work_w - 40).clamp(1200, 1840);
            let target_h = (work_h - 40).clamp(680, 1000);
            let x = work_area.left + (work_w - target_w) / 2;
            let y = work_area.top + (work_h - target_h) / 2;
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
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
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

pub fn run_gui(activation_event: crate::ActivationEvent) -> Result<()> {
    let _ = LauncherConfig::ensure_all_sandboxes();
    let sub_pids = Arc::new(Mutex::new([None::<u32>; 4]));
    let monitor = Arc::new(Mutex::new(TelemetryMonitor::new()));
    let scale_settings = Arc::new(Mutex::new(ScaleSettings::load_or_detect()));
    let scanned_cache_bytes = Arc::new(Mutex::new(0u64));
    let pending_patch_action = Arc::new(Mutex::new(None::<PendingPatchAction>));
    let sessions_store = Arc::new(Mutex::new([InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default(), InstanceSessions::default()]));
    let live_session_slot = Arc::new(Mutex::new(2usize));

    let main_window = MainWindow::new()?;
    main_window.set_is_dark(true);
    main_window.set_live_session_target_slot(2);
    main_window.set_live_session_target_text("分身 2".into());

    {
        let scales = scale_settings.lock().unwrap();
        main_window.set_host_scale_text(format!("{}%", scales.host).into());
        main_window.set_sub1_scale_text(format!("{}%", scales.get_sub_scale(1)).into());
        main_window.set_sub2_scale_text(format!("{}%", scales.get_sub_scale(2)).into());
        main_window.set_sub3_scale_text(format!("{}%", scales.get_sub_scale(3)).into());
        main_window.set_sub4_scale_text(format!("{}%", scales.get_sub_scale(4)).into());
    }

    const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32.rgba");
    let tray_menu = tray_icon::menu::Menu::new();
    let item_show = tray_icon::menu::MenuItem::new("显示窗口", true, None);
    let item_sub1 = tray_icon::menu::MenuItem::new("启动/打开分身 1", true, None);
    let item_sub2 = tray_icon::menu::MenuItem::new("启动/打开分身 2", true, None);
    let item_sub3 = tray_icon::menu::MenuItem::new("启动/打开分身 3", true, None);
    let item_sub4 = tray_icon::menu::MenuItem::new("启动/打开分身 4", true, None);
    let sep = tray_icon::menu::PredefinedMenuItem::separator();
    let item_quit = tray_icon::menu::MenuItem::new("退出", true, None);

    let show_id = item_show.id().clone();
    let sub1_id = item_sub1.id().clone();
    let sub2_id = item_sub2.id().clone();
    let sub3_id = item_sub3.id().clone();
    let sub4_id = item_sub4.id().clone();
    let quit_id = item_quit.id().clone();

    tray_menu.append_items(&[
        &item_show,
        &item_sub1,
        &item_sub2,
        &item_sub3,
        &item_sub4,
        &sep,
        &item_quit,
    ])?;
    let tray_icon_img = tray_icon::Icon::from_rgba(ICON_RGBA.to_vec(), 32, 32)?;
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Antigravity 多开分身工具")
        .with_icon(tray_icon_img)
        .build()?;

    let sub_pids_tray = Arc::clone(&sub_pids);
    let ui_weak_tray = main_window.as_weak();
    let mon_tray = Arc::clone(&monitor);

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
        move |event: tray_icon::menu::MenuEvent| {
            if event.id == show_id {
                #[cfg(target_os = "windows")]
                show_cockpit_from_tray(&ui_weak);
            } else if event.id == quit_id {
                std::process::exit(0);
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
                    let _ = slint::invoke_from_event_loop(move || {
                        let idx = slot - 1;
                        let running_pid = {
                            let mon = mon_arc.lock().unwrap();
                            let pids = pid_lock.lock().unwrap();
                            mon.subs[idx].electron_pid.or(pids[idx])
                        };
                        if let Ok(c) = LauncherConfig::for_slot(slot) {
                            if let Some(pid) = running_pid {
                                c.bring_to_front(pid);
                            } else if let Ok(new_pid) = c.spawn_detached() {
                                let mut pids = pid_lock.lock().unwrap();
                                pids[idx] = Some(new_pid);
                            }
                        }
                    });
                }
            }
        }
    }));

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
                ui.set_toast_message(if next { "窗口已固定置顶" } else { "已取消窗口置顶" }.into());
            }
        }
    });

    main_window.on_action_launch_host({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let pid_opt = mon_arc.lock().unwrap().host.electron_pid;
                if let Some(pid) = pid_opt {
                    if let Ok(conf) = LauncherConfig::for_slot(1) {
                        if conf.bring_to_front(pid) {
                            ui.set_toast_message(format!("主账号已打开 (PID: {})", pid).into());
                        } else {
                            ui.set_toast_message(format!("主账号正在运行 (PID: {})", pid).into());
                        }
                    }
                } else {
                    match LauncherConfig::spawn_host_native() {
                        Ok(new_pid) => {
                            ui.set_toast_message(format!("主账号已启动 (PID: {})", new_pid).into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("启动主账号失败: {}", e).into());
                        }
                    }
                }
            }
        }
    });

    main_window.on_action_stop_host({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let pid_opt = mon_arc.lock().unwrap().host.electron_pid;
                if let Some(pid) = pid_opt {
                    if let Ok(conf) = LauncherConfig::for_slot(1) {
                        match conf.kill_sub_instance_by_pid(pid) {
                            Ok(()) => {
                                let mut mon = mon_arc.lock().unwrap();
                                mon.host.is_running = false;
                                mon.host.electron_pid = None;
                                ui.set_toast_message(format!("主账号已停止 (PID: {})", pid).into());
                            }
                            Err(e) => {
                                ui.set_toast_message(format!("停止主账号失败: {}", e).into());
                            }
                        }
                    }
                } else {
                    ui.set_toast_message("主账号未在运行".into());
                }
            }
        }
    });

    main_window.on_action_launch_all({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut started = 0;
                let is_host_running = mon_arc.lock().unwrap().host.is_running;
                if !is_host_running {
                    if LauncherConfig::spawn_host_native().is_ok() {
                        started += 1;
                    }
                }
                for slot in 1..=4 {
                    let idx = slot - 1;
                    let is_sub_running = {
                        let mon = mon_arc.lock().unwrap();
                        let pids = sub_pids.lock().unwrap();
                        mon.subs[idx].is_running || pids[idx].is_some()
                    };
                    if !is_sub_running {
                        if let Ok(conf) = LauncherConfig::for_slot(slot) {
                            if let Ok(new_pid) = conf.spawn_detached() {
                                let mut pids = sub_pids.lock().unwrap();
                                pids[idx] = Some(new_pid);
                                started += 1;
                            }
                        }
                    }
                }
                ui.set_toast_message(format!("已启动 {} 个实例", started).into());
            }
        }
    });

    main_window.on_action_stop_all({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let _ = LauncherConfig::kill_all_antigravity();
                {
                    let mut pids = sub_pids.lock().unwrap();
                    for p in pids.iter_mut() {
                        *p = None;
                    }
                }
                {
                    let mut mon = mon_arc.lock().unwrap();
                    mon.host.is_running = false;
                    mon.host.electron_pid = None;
                    for s in mon.subs.iter_mut() {
                        s.is_running = false;
                        s.electron_pid = None;
                    }
                }
                ui.set_toast_message("已停止全部运行中实例".into());
            }
        }
    });

    main_window.on_action_refresh_telemetry({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let known_subs = *sub_pids.lock().unwrap();
                mon_arc.lock().unwrap().refresh(known_subs);
                ui.set_toast_message("遥测数据已刷新".into());
            }
        }
    });

    main_window.on_action_open_clean_modal({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let cache_bytes = Arc::clone(&scanned_cache_bytes);
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let running = {
                let mon = mon_arc.lock().unwrap();
                [
                    mon.subs[0].is_running,
                    mon.subs[1].is_running,
                    mon.subs[2].is_running,
                    mon.subs[3].is_running,
                ]
            };
            let found = LauncherConfig::scan_sandbox_cache(&running);
            if found.is_empty() {
                ui.set_toast_message("没有可清理的缓存".into());
                return;
            }
            let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
            *cache_bytes.lock().unwrap() = total_bytes;
            let total = total_bytes as f64 / (1024.0 * 1024.0);
            let detail = found
                .iter()
                .map(|c| format!("分身 {} · {:.1} MB", c.slot, c.bytes as f64 / (1024.0 * 1024.0)))
                .collect::<Vec<_>>()
                .join("\n");
            ui.set_cache_detail_text(detail.into());
            ui.set_cache_total_text(format!("共 {:.1} MB", total).into());
            ui.set_cache_summary_text(format!("Cache / Code / GPU ({:.0} MB)", total).into());
            ui.set_clean_modal_visible(true);
        }
    });

    main_window.on_action_launch_sub({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move |slot| {
            if slot < 1 || slot > 4 {
                return;
            }
            let idx = (slot - 1) as usize;
            if let Some(ui) = ui_weak.upgrade() {
                let conf = match LauncherConfig::for_slot(slot as usize) {
                    Ok(c) => c,
                    Err(e) => {
                        ui.set_toast_message(format!("分身 {} 配置异常: {}", slot, e).into());
                        return;
                    }
                };

                let running_pid = {
                    let mon = mon_arc.lock().unwrap();
                    let pids = sub_pids.lock().unwrap();
                    mon.subs[idx].electron_pid.or(pids[idx])
                };

                if let Some(pid) = running_pid {
                    if conf.bring_to_front(pid) {
                        ui.set_toast_message(format!("分身 {} 已打开 (PID: {})", slot, pid).into());
                    } else {
                        ui.set_toast_message(format!("分身 {} 正在运行 (PID: {})", slot, pid).into());
                    }
                } else {
                    match conf.spawn_detached() {
                        Ok(new_pid) => {
                            let mut pids = sub_pids.lock().unwrap();
                            pids[idx] = Some(new_pid);
                            ui.set_toast_message(format!("分身 {} 已启动 (PID: {})", slot, new_pid).into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("分身 {} 启动失败: {}", slot, e).into());
                        }
                    }
                }
            }
        }
    });

    main_window.on_action_stop_sub({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let sub_pids = Arc::clone(&sub_pids);
        move |slot| {
            if slot < 1 || slot > 4 {
                return;
            }
            let idx = (slot - 1) as usize;
            if let Some(ui) = ui_weak.upgrade() {
                let conf = match LauncherConfig::for_slot(slot as usize) {
                    Ok(c) => c,
                    Err(_) => return,
                };
                let pid_to_kill = {
                    let mon = mon_arc.lock().unwrap();
                    let pids = sub_pids.lock().unwrap();
                    mon.subs[idx].electron_pid.or(pids[idx])
                };
                if let Some(pid) = pid_to_kill {
                    match conf.kill_sub_instance_by_pid(pid) {
                        Ok(()) => {
                            let mut pids = sub_pids.lock().unwrap();
                            pids[idx] = None;
                            let mut mon = mon_arc.lock().unwrap();
                            mon.subs[idx].is_running = false;
                            mon.subs[idx].electron_pid = None;
                            ui.set_toast_message(format!("分身 {} 已停止 (PID: {})", slot, pid).into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("停止分身 {} 失败: {}", slot, e).into());
                        }
                    }
                } else {
                    ui.set_toast_message(format!("分身 {} 未在运行", slot).into());
                }
            }
        }
    });

    main_window.on_action_clear_token({
        let ui_weak = main_window.as_weak();
        move |slot| {
            if slot < 1 || slot > 4 {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                match LauncherConfig::for_slot(slot as usize) {
                    Ok(conf) => match conf.clear_token() {
                        Ok(true) => {
                            ui.set_toast_message(format!("分身 {} 凭据已清理", slot).into());
                        }
                        Ok(false) => {
                            ui.set_toast_message(format!("分身 {} 暂无已保存凭据", slot).into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("清理凭据失败: {}", e).into());
                        }
                    },
                    Err(e) => {
                        ui.set_toast_message(format!("获取配置失败: {}", e).into());
                    }
                }
            }
        }
    });

    main_window.on_action_open_sandbox({
        let ui_weak = main_window.as_weak();
        move |slot| {
            if slot < 1 || slot > 4 {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                if let Ok(conf) = LauncherConfig::for_slot(slot as usize) {
                    let _ = conf.open_sandbox_in_explorer();
                    ui.set_toast_message(format!("已打开分身 {} 目录", slot).into());
                }
            }
        }
    });

    main_window.on_action_open_app_dir({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let _ = LauncherConfig::open_app_dir_in_explorer();
                ui.set_toast_message("已打开程序目录".into());
            }
        }
    });

    main_window.on_action_scan_cache({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let cache_bytes = Arc::clone(&scanned_cache_bytes);
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let running = {
                let mon = mon_arc.lock().unwrap();
                [
                    mon.subs[0].is_running,
                    mon.subs[1].is_running,
                    mon.subs[2].is_running,
                    mon.subs[3].is_running,
                ]
            };
            let found = LauncherConfig::scan_sandbox_cache(&running);
            if found.is_empty() {
                ui.set_toast_message("没有可清理的缓存".into());
                return;
            }
            let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
            *cache_bytes.lock().unwrap() = total_bytes;
            let total = total_bytes as f64 / (1024.0 * 1024.0);
            let detail = found
                .iter()
                .map(|c| format!("分身 {} · {:.1} MB", c.slot, c.bytes as f64 / (1024.0 * 1024.0)))
                .collect::<Vec<_>>()
                .join("\n");
            ui.set_cache_detail_text(detail.into());
            ui.set_cache_total_text(format!("共 {:.1} MB", total).into());
            ui.set_cache_summary_text(format!("Cache / Code / GPU ({:.0} MB)", total).into());
            ui.set_clean_modal_visible(true);
        }
    });

    main_window.on_action_clean_cache({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.set_clean_modal_visible(false);
            let running = {
                let mon = mon_arc.lock().unwrap();
                [
                    mon.subs[0].is_running,
                    mon.subs[1].is_running,
                    mon.subs[2].is_running,
                    mon.subs[3].is_running,
                ]
            };
            match LauncherConfig::clean_sandbox_cache(&running) {
                Ok(res) => {
                    let mb = res.freed_bytes as f64 / (1024.0 * 1024.0);
                    let after_found = LauncherConfig::scan_sandbox_cache(&running);
                    let after_total: u64 = after_found.iter().map(|c| c.bytes).sum();
                    let after_mb = after_total as f64 / (1024.0 * 1024.0);
                    ui.set_cache_summary_text(format!("Cache / Code / GPU ({:.0} MB)", after_mb).into());
                    if res.has_locked_files && mb > 0.0 {
                        ui.set_toast_message(format!("已清理 {:.1} MB，部分文件被占用", mb).into());
                    } else if res.has_locked_files && mb == 0.0 {
                        ui.set_toast_message("文件被占用，无法清理".into());
                    } else {
                        ui.set_toast_message(format!("缓存已清理 ({:.1} MB)", mb).into());
                    }
                }
                Err(e) => {
                    ui.set_toast_message(format!("清理缓存失败: {}", e).into());
                }
            }
        }
    });

    main_window.on_action_check_patch({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let (ag2, ver1) = LauncherConfig::check_ag2_patch_status();
                ui.set_patch_ag2_installed(ag2);
                ui.set_patch_ag2_version(ver1.clone().into());
                let p = ProxyManager::check_status();
                ui.set_proxy_injected(p.is_fully_deployed());
                ui.set_proxy_status_text(p.status_text().into());
                ui.set_toast_message(format!("版本检测已刷新 ({})", ver1).into());
            }
        }
    });

    let pending_ag2 = Arc::clone(&pending_patch_action);
    main_window.on_toggle_patch_ag2({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                if LauncherConfig::is_antigravity_running() {
                    *pending_ag2.lock().unwrap() = Some(PendingPatchAction::TogglePatchAg2);
                    ui.set_patch_modal_title("Antigravity 正在运行".into());
                    ui.set_patch_modal_msg("文件被运行中的实例占用，需退出后继续。".into());
                    ui.set_patch_modal_btn_text("关闭并应用".into());
                    ui.set_patch_modal_visible(true);
                    return;
                }
                match LauncherConfig::toggle_ag2_patch() {
                    Ok(state) => {
                        ui.set_patch_ag2_installed(state);
                        ui.set_toast_message(if state { "已安装 Antigravity 2.0 汉化" } else { "已还原 Antigravity 2.0" }.into());
                    }
                    Err(e) => {
                        ui.set_toast_message(format!("{}", e).into());
                    }
                }
            }
        }
    });

    let pending_proxy = Arc::clone(&pending_patch_action);
    main_window.on_toggle_proxy({
        let ui_weak = main_window.as_weak();
        move || {
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
                    match ProxyManager::remove_all() {
                        Ok(summary) => {
                            let now = ProxyManager::check_status();
                            ui.set_proxy_injected(now.is_fully_deployed());
                            ui.set_proxy_status_text(now.status_text().into());
                            ui.set_toast_message(summary.status_text().into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("移除代理失败: {}", e).into());
                        }
                    }
                } else {
                    match ProxyManager::deploy_all() {
                        Ok(msg) => {
                            let now = ProxyManager::check_status();
                            ui.set_proxy_injected(now.is_fully_deployed());
                            ui.set_proxy_status_text(now.status_text().into());
                            ui.set_toast_message(msg.into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("开启代理失败: {}", e).into());
                        }
                    }
                }
            }
        }
    });

    let pending_confirm = Arc::clone(&pending_patch_action);
    main_window.on_action_patch_modal_confirm({
        let ui_weak = main_window.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let action = pending_confirm.lock().unwrap().take();
                let _ = LauncherConfig::kill_all_antigravity();
                match action {
                    Some(PendingPatchAction::TogglePatchAg2) => {
                        match LauncherConfig::toggle_ag2_patch() {
                            Ok(state) => {
                                ui.set_patch_ag2_installed(state);
                                let (is_patched, ver) = LauncherConfig::check_ag2_patch_status();
                                ui.set_patch_ag2_installed(is_patched);
                                ui.set_patch_ag2_version(ver.into());
                                ui.set_toast_message(if state { "已安装 Antigravity 2.0 汉化" } else { "已还原 Antigravity 2.0" }.into());
                            }
                            Err(e) => {
                                ui.set_toast_message(format!("{}", e).into());
                            }
                        }
                    }
                    Some(PendingPatchAction::ToggleProxy) => {
                        let status = ProxyManager::check_status();
                        if status.is_fully_deployed() {
                            match ProxyManager::remove_all() {
                                Ok(summary) => {
                                    let now = ProxyManager::check_status();
                                    ui.set_proxy_injected(now.is_fully_deployed());
                                    ui.set_proxy_status_text(now.status_text().into());
                                    ui.set_toast_message(summary.status_text().into());
                                }
                                Err(e) => {
                                    ui.set_toast_message(format!("移除代理失败: {}", e).into());
                                }
                            }
                        } else {
                            match ProxyManager::deploy_all() {
                                Ok(msg) => {
                                    let now = ProxyManager::check_status();
                                    ui.set_proxy_injected(now.is_fully_deployed());
                                    ui.set_proxy_status_text(now.status_text().into());
                                    ui.set_toast_message(msg.into());
                                }
                                Err(e) => {
                                    ui.set_toast_message(format!("开启代理失败: {}", e).into());
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
        }
    });

    main_window.on_adjust_host_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |delta| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                scales.adjust_host(delta as i32);
                let _ = scales.save();
                ui.set_host_scale_text(format!("{}%", scales.host).into());
                ui.set_toast_message(format!("主账号界面调节: {}% (重启生效)", scales.host).into());
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
                ui.set_toast_message("主账号界面已重置为 100% (重启生效)".into());
            }
        }
    });

    main_window.on_adjust_sub_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |slot, delta| {
            if slot < 1 || slot > 4 {
                return;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let mut scales = scales_lock.lock().unwrap();
                scales.adjust_sub(slot as usize, delta as i32);
                let _ = scales.save();
                let val = scales.get_sub_scale(slot as usize);
                let text = format!("{}%", val);
                match slot {
                    1 => ui.set_sub1_scale_text(text.clone().into()),
                    2 => ui.set_sub2_scale_text(text.clone().into()),
                    3 => ui.set_sub3_scale_text(text.clone().into()),
                    4 => ui.set_sub4_scale_text(text.clone().into()),
                    _ => {}
                }
                ui.set_toast_message(format!("分身 {} 界面调节: {} (重启生效)", slot, text).into());
            }
        }
    });

    main_window.on_reset_sub_scale({
        let ui_weak = main_window.as_weak();
        let scales_lock = Arc::clone(&scale_settings);
        move |slot| {
            if slot < 1 || slot > 4 {
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
                ui.set_toast_message(format!("分身 {} 界面已重置为 100% (重启生效)", slot).into());
            }
        }
    });

    let sessions_store_modal = Arc::clone(&sessions_store);
    main_window.on_show_session_modal({
        let ui_weak = main_window.as_weak();
        move |slot| {
            if let Some(ui) = ui_weak.upgrade() {
                let slot_idx = (slot as usize).clamp(0, 4);
                let title = if slot == 0 {
                    "主账号 会话监视".to_string()
                } else {
                    format!("分身 {} 会话监视", slot)
                };
                let store = sessions_store_modal.lock().unwrap();
                let model = to_session_ui_model(&store[slot_idx].groups);
                ui.set_session_modal_slot(slot);
                ui.set_session_modal_title(title.into());
                ui.set_session_modal_items(model);
                ui.set_session_modal_visible(true);
            }
        }
    });

    let live_slot_cb = Arc::clone(&live_session_slot);
    main_window.on_switch_live_session_target({
        let ui_weak = main_window.as_weak();
        move |next_slot| {
            let slot = (next_slot as usize) % 5;
            *live_slot_cb.lock().unwrap() = slot;
            if let Some(ui) = ui_weak.upgrade() {
                let target_text = match slot {
                    0 => "主账号",
                    1 => "分身 1",
                    2 => "分身 2",
                    3 => "分身 3",
                    4 => "分身 4",
                    _ => "分身 2",
                };
                ui.set_live_session_target_slot(slot as i32);
                ui.set_live_session_target_text(target_text.into());
                ui.set_toast_message(format!("已切换至 {}", target_text).into());
            }
        }
    });

    let ui_weak_cache = main_window.as_weak();
    std::thread::spawn(move || {
        let found = LauncherConfig::scan_sandbox_cache(&[false, false, false, false]);
        let total_bytes: u64 = found.iter().map(|c| c.bytes).sum();
        let mb = total_bytes as f64 / (1024.0 * 1024.0);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_cache.upgrade() {
                ui.set_cache_summary_text(format!("Cache / Code / GPU ({:.0} MB)", mb).into());
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

    let quota_cache = Arc::new(Mutex::new(crate::quota::load_quota_cache()));
    let quota_states = Arc::new(Mutex::new([
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
        InstanceQuotaState::default(),
    ]));

    let mon_bg = Arc::clone(&monitor);
    let qs_bg = Arc::clone(&quota_states);
    let qc_bg = Arc::clone(&quota_cache);
    std::thread::spawn(move || {
        loop {
            let targets = {
                let mon = mon_bg.lock().unwrap();
                let mut list = Vec::with_capacity(5);
                list.push((mon.host.ls_port, mon.host.ls_csrf.clone(), "host".to_string()));
                for (idx, sub) in mon.subs.iter().enumerate() {
                    list.push((sub.ls_port, sub.ls_csrf.clone(), format!("sub{}", idx + 1)));
                }
                list
            };

            let mut fetched_any = false;
            for (idx, (port_opt, csrf_opt, tag)) in targets.into_iter().enumerate() {
                if let (Some(port), Some(csrf)) = (port_opt, csrf_opt) {
                    if !csrf.is_empty() {
                        let st = crate::quota::fetch_instance_quota(port, &csrf, &tag);
                        if st.is_available {
                            {
                                let mut qs = qs_bg.lock().unwrap();
                                qs[idx] = st.clone();
                            }
                            {
                                let mut qc = qc_bg.lock().unwrap();
                                qc.set_slot(idx, st);
                                crate::quota::save_quota_cache(&qc);
                            }
                            fetched_any = true;
                        }
                    }
                }
            }

            let sleep_ms = if fetched_any { 3000 } else { 800 };
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    });

    let bg_telemetry = Arc::new(Mutex::new((
        LauncherConfig::check_ag2_patch_status(),
        ProxyManager::check_status(),
    )));

    let bg_telem_worker = Arc::clone(&bg_telemetry);
    let sessions_store_worker = Arc::clone(&sessions_store);
    std::thread::spawn(move || {
        loop {
            let host_home = LauncherConfig::resolve_host_user_dir();
            let host_sessions = if let Some(ref h) = host_home {
                scan_sandbox_sessions(h, 0)
            } else {
                InstanceSessions::default()
            };
            let sub1_home = LauncherConfig::for_slot(1).ok().map(|c| c.home_dir);
            let sub1_sessions = if let Some(ref h) = sub1_home {
                scan_sandbox_sessions(h, 1)
            } else {
                InstanceSessions::default()
            };
            let sub2_home = LauncherConfig::for_slot(2).ok().map(|c| c.home_dir);
            let sub2_sessions = if let Some(ref h) = sub2_home {
                scan_sandbox_sessions(h, 2)
            } else {
                InstanceSessions::default()
            };
            let sub3_home = LauncherConfig::for_slot(3).ok().map(|c| c.home_dir);
            let sub3_sessions = if let Some(ref h) = sub3_home {
                scan_sandbox_sessions(h, 3)
            } else {
                InstanceSessions::default()
            };
            let sub4_home = LauncherConfig::for_slot(4).ok().map(|c| c.home_dir);
            let sub4_sessions = if let Some(ref h) = sub4_home {
                scan_sandbox_sessions(h, 4)
            } else {
                InstanceSessions::default()
            };

            {
                let mut store = sessions_store_worker.lock().unwrap();
                store[0] = host_sessions;
                store[1] = sub1_sessions;
                store[2] = sub2_sessions;
                store[3] = sub3_sessions;
                store[4] = sub4_sessions;
            }

            let patch = LauncherConfig::check_ag2_patch_status();
            let proxy = ProxyManager::check_status();
            {
                let mut telem = bg_telem_worker.lock().unwrap();
                *telem = (patch, proxy);
            }

            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    });

    let bg_telem_timer = Arc::clone(&bg_telemetry);
    let qs_timer = Arc::clone(&quota_states);
    let qc_timer = Arc::clone(&quota_cache);
    let timer = Timer::default();
    let ui_weak_mon = main_window.as_weak();
    let monitor_timer = Arc::clone(&monitor);
    let sub_pids_mon = Arc::clone(&sub_pids);
    let sessions_store_timer = Arc::clone(&sessions_store);
    let live_session_slot_timer = Arc::clone(&live_session_slot);

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
            center_and_scale_cockpit_window(hwnd);
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
        }
        was_iconic = is_iconic;
    });

    timer.start(TimerMode::Repeated, std::time::Duration::from_millis(1000), move || {
        if let Some(ui) = ui_weak_mon.upgrade() {
            let mut mon = monitor_timer.lock().unwrap();
            let known_subs = *sub_pids_mon.lock().unwrap();
            mon.refresh(known_subs);

            let mut pids = sub_pids_mon.lock().unwrap();
            for i in 0..4 {
                if let Some(pid) = mon.subs[i].electron_pid {
                    pids[i] = Some(pid);
                } else if !mon.subs[i].is_running {
                    pids[i] = None;
                }
            }

            ui.set_host_running(mon.host.is_running);
            ui.set_host_pid(mon.host.electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_host_cpu(mon.host.cpu_usage);
            ui.set_host_cpu_text(format!("{:.1}%", mon.host.cpu_usage).into());
            ui.set_host_ram(mon.host.memory_rss_mb as f32);
            ui.set_host_ram_text(format!("{:.0} MB", mon.host.memory_rss_mb).into());
            ui.set_host_port(mon.host.ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());

            let uptime_str = if let Some(secs) = mon.host_uptime_seconds() {
                if secs < 60 {
                    format!("{}秒", secs)
                } else if secs < 3600 {
                    format!("{}分{}秒", secs / 60, secs % 60)
                } else {
                    format!("{}小时{}分", secs / 3600, (secs % 3600) / 60)
                }
            } else {
                "--".to_string()
            };
            ui.set_host_uptime_text(uptime_str.into());

            ui.set_sub1_running(mon.subs[0].is_running);
            ui.set_sub1_pid(mon.subs[0].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub1_port(mon.subs[0].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub1_cpu_text(if mon.subs[0].is_running { format!("{:.1}%", mon.subs[0].cpu_usage).into() } else { "--".into() });
            ui.set_sub1_ram_text(if mon.subs[0].is_running { format!("{:.0} MB", mon.subs[0].memory_rss_mb).into() } else { "--".into() });
            ui.set_sub1_token_present(LauncherConfig::for_slot(1).map(|c| c.is_token_present()).unwrap_or(false));

            ui.set_sub2_running(mon.subs[1].is_running);
            ui.set_sub2_pid(mon.subs[1].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub2_port(mon.subs[1].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub2_cpu_text(if mon.subs[1].is_running { format!("{:.1}%", mon.subs[1].cpu_usage).into() } else { "--".into() });
            ui.set_sub2_ram_text(if mon.subs[1].is_running { format!("{:.0} MB", mon.subs[1].memory_rss_mb).into() } else { "--".into() });
            ui.set_sub2_token_present(LauncherConfig::for_slot(2).map(|c| c.is_token_present()).unwrap_or(false));

            ui.set_sub3_running(mon.subs[2].is_running);
            ui.set_sub3_pid(mon.subs[2].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub3_port(mon.subs[2].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub3_cpu_text(if mon.subs[2].is_running { format!("{:.1}%", mon.subs[2].cpu_usage).into() } else { "--".into() });
            ui.set_sub3_ram_text(if mon.subs[2].is_running { format!("{:.0} MB", mon.subs[2].memory_rss_mb).into() } else { "--".into() });
            ui.set_sub3_token_present(LauncherConfig::for_slot(3).map(|c| c.is_token_present()).unwrap_or(false));

            ui.set_sub4_running(mon.subs[3].is_running);
            ui.set_sub4_pid(mon.subs[3].electron_pid.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub4_port(mon.subs[3].ls_port.map(|p| p.to_string()).unwrap_or_else(|| "--".to_string()).into());
            ui.set_sub4_cpu_text(if mon.subs[3].is_running { format!("{:.1}%", mon.subs[3].cpu_usage).into() } else { "--".into() });
            ui.set_sub4_ram_text(if mon.subs[3].is_running { format!("{:.0} MB", mon.subs[3].memory_rss_mb).into() } else { "--".into() });
            ui.set_sub4_token_present(LauncherConfig::for_slot(4).map(|c| c.is_token_present()).unwrap_or(false));

            let running_sub_count = mon.subs.iter().filter(|s| s.is_running).count();
            ui.set_sub_running_count_text(format!("当前活动分身: {} / 4", running_sub_count).into());

            let is_dark = ui.get_is_dark();
            let qs_snapshot = qs_timer.lock().unwrap().clone();

            let host_token = LauncherConfig::resolve_host_user_dir()
                .map(|u| u.join(".gemini").join("jetski-standalone-oauth-token"));
            let host_state = if mon.host.is_running && qs_snapshot[0].is_available {
                Some(qs_snapshot[0].clone())
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
                |pct, reset, ring| {
                    ui.set_host_weekly_text(pct.into());
                    ui.set_host_weekly_reset(reset.into());
                    ui.set_host_weekly_ring(ring);
                },
                |pct, reset, ring| {
                    ui.set_host_five_h_text(pct.into());
                    ui.set_host_five_h_reset(reset.into());
                    ui.set_host_five_h_ring(ring);
                },
            );

            let sub1_token = LauncherConfig::for_slot(1).ok().map(|c| c.token_path);
            let sub1_state = if mon.subs[0].is_running && qs_snapshot[1].is_available {
                Some(qs_snapshot[1].clone())
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
                |pct, reset, ring| {
                    ui.set_sub1_weekly_text(pct.into());
                    ui.set_sub1_weekly_reset(reset.into());
                    ui.set_sub1_weekly_ring(ring);
                },
                |pct, reset, ring| {
                    ui.set_sub1_five_h_text(pct.into());
                    ui.set_sub1_five_h_reset(reset.into());
                    ui.set_sub1_five_h_ring(ring);
                },
            );

            let sub2_token = LauncherConfig::for_slot(2).ok().map(|c| c.token_path);
            let sub2_state = if mon.subs[1].is_running && qs_snapshot[2].is_available {
                Some(qs_snapshot[2].clone())
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
                |pct, reset, ring| {
                    ui.set_sub2_weekly_text(pct.into());
                    ui.set_sub2_weekly_reset(reset.into());
                    ui.set_sub2_weekly_ring(ring);
                },
                |pct, reset, ring| {
                    ui.set_sub2_five_h_text(pct.into());
                    ui.set_sub2_five_h_reset(reset.into());
                    ui.set_sub2_five_h_ring(ring);
                },
            );

            let sub3_token = LauncherConfig::for_slot(3).ok().map(|c| c.token_path);
            let sub3_state = if mon.subs[2].is_running && qs_snapshot[3].is_available {
                Some(qs_snapshot[3].clone())
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
                |pct, reset, ring| {
                    ui.set_sub3_weekly_text(pct.into());
                    ui.set_sub3_weekly_reset(reset.into());
                    ui.set_sub3_weekly_ring(ring);
                },
                |pct, reset, ring| {
                    ui.set_sub3_five_h_text(pct.into());
                    ui.set_sub3_five_h_reset(reset.into());
                    ui.set_sub3_five_h_ring(ring);
                },
            );

            let sub4_token = LauncherConfig::for_slot(4).ok().map(|c| c.token_path);
            let sub4_state = if mon.subs[3].is_running && qs_snapshot[4].is_available {
                Some(qs_snapshot[4].clone())
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
                |pct, reset, ring| {
                    ui.set_sub4_weekly_text(pct.into());
                    ui.set_sub4_weekly_reset(reset.into());
                    ui.set_sub4_weekly_ring(ring);
                },
                |pct, reset, ring| {
                    ui.set_sub4_five_h_text(pct.into());
                    ui.set_sub4_five_h_reset(reset.into());
                    ui.set_sub4_five_h_ring(ring);
                },
            );

            let host_mb = mon.host.memory_rss_mb;
            let sub_mb: f64 = mon.subs.iter().map(|s| s.memory_rss_mb).sum();
            let total_mb = mon.total_memory_mb;
            ui.set_total_mem_text(format!("{:.0} MB", total_mb).into());
            ui.set_mem_breakdown_text(format!("主号 {:.0}M · 分身 {:.0}M", host_mb, sub_mb).into());
            let pct = if mon.system_total_ram_mb > 0.0 {
                (total_mb / mon.system_total_ram_mb) as f32
            } else {
                0.0
            };
            ui.set_total_mem_pct(pct.clamp(0.0, 1.0));

            let sys_ram = if mon.system_total_ram_mb > 0.0 { mon.system_total_ram_mb } else { 32768.0 };
            let host_mem_pct = (host_mb / sys_ram) as f32;
            let subs_mem_pct = (sub_mb / sys_ram) as f32;
            ui.set_mem_host_pct(host_mem_pct.clamp(0.0, 1.0));
            ui.set_mem_subs_pct(subs_mem_pct.clamp(0.0, 1.0));
            let avail_ram = (sys_ram - total_mb).max(0.0);
            ui.set_mem_detail_text(format!("主号 {:.0}M · 分身 {:.0}M · 待命可用 {:.0}M", host_mb, sub_mb, avail_ram).into());

            let (ag2_installed, ag2_ver, p_status) = {
                let telem = bg_telem_timer.lock().unwrap();
                (telem.0.0, telem.0.1.clone(), telem.1.clone())
            };
            ui.set_patch_ag2_installed(ag2_installed);
            ui.set_patch_ag2_version(ag2_ver.into());

            ui.set_proxy_injected(p_status.is_fully_deployed());
            ui.set_proxy_status_text(p_status.status_text().into());

            let mut running_count = if mon.host.is_running { 1 } else { 0 };
            for s in &mon.subs {
                if s.is_running {
                    running_count += 1;
                }
            }
            ui.set_global_summary_text(format!("{} 个实例运行中 · 内存占用 {:.0} MB · 端口正常", running_count, total_mb).into());
            let proxy_badge = if p_status.is_fully_deployed() {
                if p_status.port_online {
                    "系统代理 127.0.0.1:7890".to_string()
                } else {
                    "系统代理 127.0.0.1:7890 (未监听)".to_string()
                }
            } else {
                "系统代理未注入".to_string()
            };
            ui.set_proxy_badge_text(proxy_badge.into());

            let (host_sessions, sub1_sessions, sub2_sessions, sub3_sessions, sub4_sessions) = {
                let store = sessions_store_timer.lock().unwrap();
                (store[0].clone(), store[1].clone(), store[2].clone(), store[3].clone(), store[4].clone())
            };

            ui.set_host_summary_text(host_sessions.summary_text.as_str().into());
            ui.set_host_has_running(host_sessions.has_running);
            ui.set_host_has_sessions(host_sessions.total_sessions > 0);

            ui.set_sub1_summary_text(sub1_sessions.summary_text.as_str().into());
            ui.set_sub1_has_running(sub1_sessions.has_running);
            ui.set_sub1_has_sessions(sub1_sessions.total_sessions > 0);
            ui.set_sub1_session_count_text(if sub1_sessions.total_sessions > 0 { format!("{} 个", sub1_sessions.total_sessions).into() } else { "--".into() });
            let (s1_p, s1_t, s1_time) = extract_recent_session_info(&sub1_sessions);
            ui.set_sub1_recent_project(s1_p.into());
            ui.set_sub1_recent_tokens(s1_t.into());
            ui.set_sub1_recent_time(s1_time.into());

            ui.set_sub2_summary_text(sub2_sessions.summary_text.as_str().into());
            ui.set_sub2_has_running(sub2_sessions.has_running);
            ui.set_sub2_has_sessions(sub2_sessions.total_sessions > 0);
            ui.set_sub2_session_count_text(if sub2_sessions.total_sessions > 0 { format!("{} 个", sub2_sessions.total_sessions).into() } else { "--".into() });
            let (s2_p, s2_t, s2_time) = extract_recent_session_info(&sub2_sessions);
            ui.set_sub2_recent_project(s2_p.into());
            ui.set_sub2_recent_tokens(s2_t.into());
            ui.set_sub2_recent_time(s2_time.into());

            ui.set_sub3_summary_text(sub3_sessions.summary_text.as_str().into());
            ui.set_sub3_has_running(sub3_sessions.has_running);
            ui.set_sub3_has_sessions(sub3_sessions.total_sessions > 0);
            ui.set_sub3_session_count_text(if sub3_sessions.total_sessions > 0 { format!("{} 个", sub3_sessions.total_sessions).into() } else { "--".into() });
            let (s3_p, s3_t, s3_time) = extract_recent_session_info(&sub3_sessions);
            ui.set_sub3_recent_project(s3_p.into());
            ui.set_sub3_recent_tokens(s3_t.into());
            ui.set_sub3_recent_time(s3_time.into());

            ui.set_sub4_summary_text(sub4_sessions.summary_text.as_str().into());
            ui.set_sub4_has_running(sub4_sessions.has_running);
            ui.set_sub4_has_sessions(sub4_sessions.total_sessions > 0);
            ui.set_sub4_session_count_text(if sub4_sessions.total_sessions > 0 { format!("{} 个", sub4_sessions.total_sessions).into() } else { "0".into() });
            let (s4_p, s4_t, s4_time) = extract_recent_session_info(&sub4_sessions);
            ui.set_sub4_recent_project(s4_p.into());
            ui.set_sub4_recent_tokens(s4_t.into());
            ui.set_sub4_recent_time(s4_time.into());

            let is_modal_visible = ui.get_session_modal_visible();
            let current_modal_slot = ui.get_session_modal_slot() as usize;

            {
                let store = sessions_store_timer.lock().unwrap();

                if is_modal_visible && current_modal_slot <= 4 {
                    let model = to_session_ui_model(&store[current_modal_slot].groups);
                    ui.set_session_modal_items(model);
                }

                let target_slot = *live_session_slot_timer.lock().unwrap();
                let target_slot = target_slot.clamp(0, 4);
                let target_sessions = &store[target_slot];
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
                    });

                if let Some(s) = active {
                    ui.set_live_session_is_running(s.is_running);
                    ui.set_live_session_status_text(if s.is_running { "活跃运行中".into() } else { "就绪 · 待命".into() });
                    ui.set_live_session_project(s.project_name.as_str().into());
                    ui.set_live_session_title(s.title.as_str().into());
                    if s.is_running {
                        ui.set_live_session_speed_text("--".into());
                        ui.set_live_session_speed_sub("生成中".into());
                    } else {
                        ui.set_live_session_speed_text("0.0".into());
                        ui.set_live_session_speed_sub("待命中".into());
                    }
                    let max_tokens = 1_000_000u64;
                    let prompt_tokens = s.context_tokens.saturating_sub(s.cache_read_tokens).saturating_sub(s.output_tokens);
                    let prompt_pct = (prompt_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    let cache_pct = (s.cache_read_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    let output_pct = (s.output_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    ui.set_live_prompt_pct(prompt_pct);
                    ui.set_live_cache_pct(cache_pct);
                    ui.set_live_output_pct(output_pct);
                    ui.set_live_prompt_text(format!("输入 {}", format_tokens(prompt_tokens)).into());
                    ui.set_live_cache_legend(format!("缓存 {} ({})", format_tokens(s.cache_read_tokens), s.cache_rate_str).into());
                    ui.set_live_output_text(format!("输出 {}", format_tokens(s.output_tokens)).into());

                    let pct = (s.context_tokens as f32 / max_tokens as f32).clamp(0.0, 1.0);
                    ui.set_live_session_context_text(format!("{} / 1,000K ({:.1}%)", format_tokens(s.context_tokens), pct * 100.0).into());
                    ui.set_live_session_context_pct(pct);
                    ui.set_live_session_cache_text(format!("缓存 {} ({})", format_tokens(s.cache_read_tokens), s.cache_rate_str).into());
                    let avail = max_tokens.saturating_sub(s.context_tokens);
                    ui.set_live_session_available_text(format!("可用 {}", format_tokens(avail)).into());
                    if s.cache_rate > 0.0 {
                        let opt_val = (s.cache_rate * 100.0).clamp(10.0, 95.0);
                        ui.set_live_session_optimization_text(format!("首字延迟优化: ~{:.0}%", opt_val).into());
                    } else {
                        ui.set_live_session_optimization_text("首字延迟优化: --".into());
                    }
                } else {
                    ui.set_live_prompt_pct(0.0);
                    ui.set_live_cache_pct(0.0);
                    ui.set_live_output_pct(0.0);
                    ui.set_live_prompt_text("输入 0".into());
                    ui.set_live_cache_legend("缓存 0 (0.0%)".into());
                    ui.set_live_output_text("输出 0".into());

                    ui.set_live_session_is_running(false);
                    ui.set_live_session_status_text("待命中".into());
                    ui.set_live_session_project("--".into());
                    ui.set_live_session_title("无活动会话".into());
                    ui.set_live_session_speed_text("0.0".into());
                    ui.set_live_session_speed_sub("待命中 · 无吞吐".into());
                    ui.set_live_session_context_text("0 / 1,000K (0.0%)".into());
                    ui.set_live_session_context_pct(0.0);
                    ui.set_live_session_cache_text("缓存 0 (0.0%)".into());
                    ui.set_live_session_available_text("可用 1,000K".into());
                    ui.set_live_session_optimization_text("首字延迟优化: --".into());
                }
            }
        }
    });

    main_window.show()?;
    #[cfg(target_os = "windows")]
    {
        let hwnd = get_cockpit_hwnd();
        apply_dark_window_attributes(hwnd);
        center_and_scale_cockpit_window(hwnd);
    }
    let _ = slint::run_event_loop_until_quit();
    Ok(())
}

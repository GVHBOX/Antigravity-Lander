use crate::launcher::{LauncherConfig, ScaleSettings};
use crate::monitor::TelemetryMonitor;
use crate::proxy::ProxyManager;
use anyhow::Result;
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use std::sync::{Arc, Mutex};

slint::include_modules!();

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
    let scanned_cache_bytes_clean = Arc::clone(&scanned_cache_bytes);
    let pending_patch_action = Arc::new(Mutex::new(None::<PendingPatchAction>));

    let main_window = MainWindow::new()?;
    main_window.set_is_dark(true);

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
    let sep = tray_icon::menu::PredefinedMenuItem::separator();
    let item_quit = tray_icon::menu::MenuItem::new("退出", true, None);

    let show_id = item_show.id().clone();
    let sub1_id = item_sub1.id().clone();
    let quit_id = item_quit.id().clone();

    tray_menu.append_items(&[&item_show, &item_sub1, &sep, &item_quit])?;
    let tray_icon_img = tray_icon::Icon::from_rgba(ICON_RGBA.to_vec(), 32, 32)?;
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Antigravity 多开分身工具")
        .with_icon(tray_icon_img)
        .build()?;

    let sub_pids_tray = Arc::clone(&sub_pids);
    let ui_weak_tray = main_window.as_weak();

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
        move |event: tray_icon::menu::MenuEvent| {
            if event.id == show_id {
                #[cfg(target_os = "windows")]
                show_cockpit_from_tray(&ui_weak);
            } else if event.id == sub1_id {
                let pid_lock = pid_lock.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let mut p = pid_lock.lock().unwrap();
                    if let Ok(c) = LauncherConfig::for_slot(1) {
                        if let Some(pid) = p[0] {
                            c.bring_to_front(pid);
                        } else if let Ok(new_pid) = c.spawn_detached() {
                            p[0] = Some(new_pid);
                        }
                    }
                });
            } else if event.id == quit_id {
                std::process::exit(0);
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
            ui.set_clean_modal_visible(true);
        }
    });

    main_window.on_action_clean_cache({
        let ui_weak = main_window.as_weak();
        let mon_arc = Arc::clone(&monitor);
        let cache_bytes = Arc::clone(&scanned_cache_bytes_clean);
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
            let bytes = *cache_bytes.lock().unwrap();
            match LauncherConfig::clean_sandbox_cache(&running) {
                Ok(()) => {
                    let mb = bytes as f64 / (1024.0 * 1024.0);
                    ui.set_toast_message(format!("缓存已清理 ({:.1} MB)", mb).into());
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

    let timer = Timer::default();
    let ui_weak_mon = main_window.as_weak();
    let monitor_timer = Arc::clone(&monitor);
    let sub_pids_mon = Arc::clone(&sub_pids);

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

            let (ag2, ver) = LauncherConfig::check_ag2_patch_status();
            ui.set_patch_ag2_installed(ag2);
            ui.set_patch_ag2_version(ver.into());

            let p_status = ProxyManager::check_status();
            ui.set_proxy_injected(p_status.is_fully_deployed());
            ui.set_proxy_status_text(p_status.status_text().into());
        }
    });

    main_window.show()?;
    #[cfg(target_os = "windows")]
    {
        let hwnd = get_cockpit_hwnd();
        apply_dark_window_attributes(hwnd);
    }
    let _ = slint::run_event_loop_until_quit();
    Ok(())
}

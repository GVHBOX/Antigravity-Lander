#![windows_subsystem = "windows"]

mod app;
mod gui;
mod i18n;
mod launcher;
mod monitor;
mod proxy;
mod quota;
mod session;
mod sound;
mod ui;
pub mod widget_state;

use anyhow::{anyhow, Result};
use app::App;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::env;
use std::io::{stdout, Stdout};
use std::panic;
use std::ptr::null;
use std::time::{Duration, Instant};

#[cfg(windows)]
pub type ActivationEvent = windows_sys::Win32::Foundation::HANDLE;

#[cfg(not(windows))]
pub type ActivationEvent = ();

#[cfg(windows)]
struct SingleInstanceGuard {
    mutex: windows_sys::Win32::Foundation::HANDLE,
    activation_event: ActivationEvent,
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.activation_event);
            windows_sys::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

#[cfg(windows)]
fn acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent};

    let mutex_name: Vec<u16> = "Local\\AntigravityCockpit.SingleInstance\0"
        .encode_utf16()
        .collect();
    let event_name: Vec<u16> = "Local\\AntigravityCockpit.Activate\0"
        .encode_utf16()
        .collect();
    let activation_event = unsafe { CreateEventW(null(), 0, 0, event_name.as_ptr()) };
    if activation_event.is_null() {
        return Err(anyhow!("无法创建激活事件"));
    }
    let mutex = unsafe { CreateMutexW(null(), 1, mutex_name.as_ptr()) };
    if mutex.is_null() {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(activation_event);
        }
        return Err(anyhow!("无法创建单实例互斥锁"));
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            SetEvent(activation_event);
            windows_sys::Win32::Foundation::CloseHandle(activation_event);
            windows_sys::Win32::Foundation::CloseHandle(mutex);
        }
        return Ok(None);
    }
    Ok(Some(SingleInstanceGuard {
        mutex,
        activation_event,
    }))
}

#[cfg(not(windows))]
struct SingleInstanceGuard;

#[cfg(not(windows))]
impl SingleInstanceGuard {
    fn activation_event(&self) -> ActivationEvent {
        ()
    }
}

#[cfg(not(windows))]
fn acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    Ok(Some(SingleInstanceGuard))
}

#[cfg(windows)]
impl SingleInstanceGuard {
    fn activation_event(&self) -> ActivationEvent {
        self.activation_event
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);
    }
}

fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);
        original_hook(panic_info);
    }));
}

fn main() -> Result<()> {
    let Some(single_instance) = acquire_single_instance()? else {
        return Ok(());
    };
    let activation_event = single_instance.activation_event();

    let args: Vec<String> = env::args().collect();
    let use_tui = args.iter().any(|a| a == "--tui" || a == "-t");

    if !use_tui {
        if env::var("SLINT_BACKEND").is_err() {
            unsafe {
                env::set_var("SLINT_BACKEND", "winit-software");
            }
        }
        if env::var("SLINT_SCALE_FACTOR").is_err() {
            #[cfg(windows)]
            let scale_str = {
                use windows_sys::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSX};
                let dpi = unsafe {
                    let hdc = GetDC(std::ptr::null_mut());
                    if !hdc.is_null() {
                        let d = GetDeviceCaps(hdc, LOGPIXELSX as i32);
                        ReleaseDC(std::ptr::null_mut(), hdc);
                        d
                    } else {
                        96
                    }
                };
                if dpi <= 108 {
                    "1.25"
                } else {
                    ""
                }
            };
            #[cfg(not(windows))]
            let scale_str = "1.25";

            if !scale_str.is_empty() {
                unsafe {
                    env::set_var("SLINT_SCALE_FACTOR", scale_str);
                }
            }
        }
        return gui::run_gui(activation_event);
    }

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    setup_panic_hook();

    let mut app = App::new()?;

    enable_raw_mode()?;
    let guard = TerminalGuard;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = run_tui_app(&mut terminal, &mut app);

    drop(guard);

    if run_result.is_ok() {
        println!();
        println!(" [Antigravity Lander v{}] 退出。", env!("CARGO_PKG_VERSION"));
        println!(" 分身已在后台独立运行。");
        println!();
    }

    run_result
}

fn run_tui_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(200);
    let refresh_interval = Duration::from_millis(1200);
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    if app.handle_key(key.code) {
                        break;
                    }
                }

        if last_refresh.elapsed() >= refresh_interval {
            app.refresh();
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

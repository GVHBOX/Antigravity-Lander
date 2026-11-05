#![windows_subsystem = "windows"]

mod asar;
mod gui;
mod launcher;
mod localize;
mod monitor;
mod proxy;
mod quota;
mod session;
mod sound;


use anyhow::{anyhow, Result};
use std::env;
use std::ptr::null;

pub type ActivationEvent = windows_sys::Win32::Foundation::HANDLE;

struct SingleInstanceGuard {
    mutex: windows_sys::Win32::Foundation::HANDLE,
    activation_event: ActivationEvent,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.activation_event);
            windows_sys::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

fn acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent};
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

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
            AllowSetForegroundWindow(ASFW_ANY);
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

impl SingleInstanceGuard {
    fn activation_event(&self) -> ActivationEvent {
        self.activation_event
    }
}

fn main() -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let app_id: Vec<u16> = "AntigravityLander.Cockpit\0".encode_utf16().collect();
        SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }

    let Some(single_instance) = acquire_single_instance()? else {
        return Ok(());
    };
    let activation_event = single_instance.activation_event();

    if env::var("SLINT_BACKEND").is_err() {
        unsafe {
            env::set_var("SLINT_BACKEND", "winit-software");
        }
    }
    unsafe {
        env::set_var("SLINT_SCALE_FACTOR", "1.0");
    }
    gui::run_gui(activation_event)
}

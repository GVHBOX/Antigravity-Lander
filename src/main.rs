#![windows_subsystem = "windows"]

mod gui;
mod launcher;
mod monitor;
mod proxy;
mod quota;
mod session;
mod sound;
pub mod widget_state;

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

impl SingleInstanceGuard {
    fn activation_event(&self) -> ActivationEvent {
        self.activation_event
    }
}

fn main() -> Result<()> {
    let Some(single_instance) = acquire_single_instance()? else {
        return Ok(());
    };
    let activation_event = single_instance.activation_event();

    if env::var("SLINT_BACKEND").is_err() {
        unsafe {
            env::set_var("SLINT_BACKEND", "winit-software");
        }
    }
    if env::var("SLINT_SCALE_FACTOR").is_err() {
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

        if !scale_str.is_empty() {
            unsafe {
                env::set_var("SLINT_SCALE_FACTOR", scale_str);
            }
        }
    }
    gui::run_gui(activation_event)
}

//! Windows session-lock detection: locks the vault the instant the OS
//! session locks (Win+L, RDP lock), without waiting for the idle timer.
//!
//! A dedicated thread owns a message-only window (`HWND_MESSAGE`) that
//! registers with `WTSRegisterSessionNotification` and reacts to
//! `WM_WTSSESSION_CHANGE`. Pure `std::thread`, same shape as `auto_lock`.
//!
//! Session-UNLOCK events are deliberately ignored: unlocking the OS session
//! must never unlock the vault — the user re-unlocks explicitly, exactly as
//! after pressing Lock Vault.
//!
//! The registration lives exactly as long as the process. The window is
//! never destroyed while the process runs, so Windows drops the WTS
//! subscription at exit; no `WTSUnregisterSessionNotification` call exists
//! in the `windows` 0.62 bindings and none is needed here.

use crate::auth::session::SessionManager;
use std::cell::RefCell;
use std::sync::Arc;
use std::thread::JoinHandle;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::WTSRegisterSessionNotification;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
    TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_WTSSESSION_CHANGE,
    WNDCLASSEXW,
};

// Re-exported so tests and callers can react to raw WTS event codes without
// depending on the `windows` crate directly.
pub use windows::Win32::UI::WindowsAndMessaging::{WTS_SESSION_LOCK, WTS_SESSION_UNLOCK};

/// `NOTIFY_FOR_THIS_SESSION`: only this process's own session.
const NOTIFY_FOR_THIS_SESSION: u32 = 1;

thread_local! {
    /// The watcher thread owns the only window, so a thread-local is enough
    /// to reach the session manager from the window procedure.
    static MANAGER: RefCell<Option<Arc<SessionManager>>> = const { RefCell::new(None) };
}

/// React to a `WM_WTSSESSION_CHANGE` event: lock the vault on OS session
/// lock; never unlock on any event. Returns whether it locked.
///
/// Kept free of window types so tests can drive it without a real window.
pub fn on_session_change(manager: &SessionManager, event: u32) -> bool {
    if event == WTS_SESSION_LOCK {
        manager.lock();
        true
    } else {
        // WTS_SESSION_UNLOCK (and logoff/connect events) never unlock.
        false
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        MANAGER.with(|m| {
            if let Some(manager) = m.borrow().as_ref() {
                on_session_change(manager, wparam.0 as u32);
            }
        });
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Register the window class, create the message-only window, and subscribe
/// to session notifications. Split out so a test can prove the FFI wiring
/// without actually locking the workstation.
unsafe fn create_session_window() -> windows::core::Result<HWND> {
    let class = w!("VeyloraSessionLockWindow");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: class,
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class,
        w!(""),
        WINDOW_STYLE::default(),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        None,
        None,
    )?;
    WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)?;
    Ok(hwnd)
}

/// Spawn the watcher thread (see module docs). Mirrors
/// `auto_lock::spawn_watcher`: fires once, lives for the process.
pub fn spawn_watcher(manager: Arc<SessionManager>) -> JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        MANAGER.with(|m| *m.borrow_mut() = Some(manager));
        let _hwnd = match create_session_window() {
            Ok(h) => h,
            Err(e) => {
                // The idle auto-lock timer still covers us if WTS is unavailable.
                eprintln!("session-lock watcher unavailable: {e}");
                return;
            }
        };
        let mut msg = MSG::default();
        // >0: dispatch, 0: WM_QUIT, <0: error — all end the loop; the WTS
        // registration dies with the process.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves the FFI wiring — window class, message-only window, and WTS
    /// subscription — without locking the workstation. Event delivery itself
    /// is OS behavior (documented WM_WTSSESSION_CHANGE); the reaction to it
    /// is covered by phase4's `on_session_change` test.
    #[test]
    fn session_window_registers_for_lock_notifications() {
        unsafe {
            create_session_window().expect("class + message-only window + WTS register");
            // The window intentionally outlives the test: the real watcher
            // keeps it for the process lifetime, and the test binary exits
            // right after (Windows drops the registration then).
        }
    }
}

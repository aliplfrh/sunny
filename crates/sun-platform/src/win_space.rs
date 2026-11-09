//! Observe Win+Space without suppressing or synthesizing any keyboard events.

/// The special observer supports exactly Win+Space; other shortcuts use RegisterHotKey.
pub fn is_win_space(accel: &str) -> bool {
    let parts: Vec<_> = accel.split('+').map(str::trim).collect();
    parts.len() == 2
        && matches!(
            parts[0].to_ascii_lowercase().as_str(),
            "super" | "win" | "meta"
        )
        && parts[1].eq_ignore_ascii_case("space")
}

/// Only remembers the chord's key state, never text or a history of keystrokes.
#[derive(Default)]
struct Gesture {
    windows: u8,
    modifiers: u8,
    space: bool,
    pending: bool,
}

impl Gesture {
    fn event(&mut self, key: u32, down: bool) -> bool {
        let win = match key {
            0x5b => 1,
            0x5c => 2,
            _ => 0,
        };
        let modifier = match key {
            0xa0 => 1,
            0xa1 => 2,
            0xa2 => 4,
            0xa3 => 8,
            0xa4 => 16,
            0xa5 => 32,
            _ => 0,
        };
        if win != 0 {
            if down {
                self.windows |= win;
            } else {
                self.windows &= !win;
            }
        } else if key == 0x20 {
            if down && !self.space {
                self.pending = self.windows != 0 && self.modifiers == 0;
            }
            self.space = down;
        } else {
            if modifier != 0 {
                if down {
                    self.modifiers |= modifier;
                } else {
                    self.modifiers &= !modifier;
                }
            }
            if down {
                self.pending = false;
            }
        }
        if self.pending && self.windows == 0 && !self.space {
            self.pending = false;
            return true;
        }
        false
    }
}

#[cfg(windows)]
mod native {
    use super::Gesture;
    use std::cell::RefCell;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    };
    use std::thread::JoinHandle;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
        UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    struct Context {
        gesture: Gesture,
        press: SyncSender<()>,
        enabled: Arc<AtomicBool>,
    }

    thread_local! {
        static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
    }

    unsafe extern "system" fn observe(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let message = wparam.0 as u32;
            if matches!(message, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP) {
                // SAFETY: For WH_KEYBOARD_LL with nonnegative code, Windows supplies
                // a valid KBDLLHOOKSTRUCT for the duration of this callback.
                let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
                if !event.flags.contains(LLKHF_INJECTED) {
                    CONTEXT.with(|slot| {
                        if let Some(context) = slot.borrow_mut().as_mut() {
                            if context.enabled.load(Ordering::Acquire)
                                && context.gesture.event(
                                    event.vkCode,
                                    matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN),
                                )
                            {
                                // Bounded, nonblocking: no UI, locks, disk IO or waiting here.
                                let _ = context.press.try_send(());
                            }
                        }
                    });
                }
            }
        }
        // SAFETY: Forward the original arguments on EVERY path, including keyup.
        // Never return 1, suppress an event, or synthesize a replacement key.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// Owns one passive observer. Dropping it disables callbacks and removes the hook.
    pub struct WinSpaceObserver {
        thread_id: u32,
        thread: Option<JoinHandle<()>>,
        enabled: Arc<AtomicBool>,
    }

    impl WinSpaceObserver {
        pub fn start(on_release: impl Fn() + Send + 'static) -> Result<Self, String> {
            let enabled = Arc::new(AtomicBool::new(true));
            let worker_enabled = enabled.clone();
            let (press, receive) = mpsc::sync_channel(1);
            std::thread::Builder::new()
                .name("sunny-win-space-action".into())
                .spawn(move || {
                    while receive.recv().is_ok() {
                        if worker_enabled.load(Ordering::Acquire) {
                            on_release();
                        }
                    }
                })
                .map_err(|e| format!("Cannot start shortcut worker: {e}"))?;

            let (ready, result) = mpsc::sync_channel(1);
            let hook_enabled = enabled.clone();
            let thread = std::thread::Builder::new()
                .name("sunny-win-space-observer".into())
                .spawn(move || {
                    let mut message = MSG::default();
                    // SAFETY: Valid output pointer; PeekMessage creates this thread's
                    // queue before its ID is published, so WM_QUIT can always be posted.
                    unsafe {
                        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                    }
                    // SAFETY: None requests the process module; this borrowed handle
                    // remains valid until process exit and must not be freed here.
                    let module = match unsafe { GetModuleHandleW(None) } {
                        Ok(module) => module,
                        Err(e) => {
                            let _ = ready.send(Err(format!("Cannot get module handle: {e}")));
                            return;
                        }
                    };
                    CONTEXT.with(|slot| {
                        *slot.borrow_mut() = Some(Context {
                            gesture: Gesture::default(),
                            press,
                            enabled: hook_enabled,
                        })
                    });
                    // SAFETY: Static callback in this module, serviced on this thread's
                    // message loop. The hook is uninstalled on every successful exit.
                    let hook = match unsafe {
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(observe), Some(module.into()), 0)
                    } {
                        Ok(hook) => hook,
                        Err(e) => {
                            CONTEXT.with(|slot| *slot.borrow_mut() = None);
                            let _ = ready.send(Err(format!("Cannot observe Win+Space: {e}")));
                            return;
                        }
                    };
                    // SAFETY: GetCurrentThreadId has no preconditions.
                    let id = unsafe { GetCurrentThreadId() };
                    if ready.send(Ok(id)).is_ok() {
                        // SAFETY: Valid MSG storage. This dedicated thread owns no windows;
                        // GetMessage services the low-level callback until WM_QUIT or error.
                        while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {}
                    }
                    // SAFETY: This is the live hook installed above, removed exactly once.
                    unsafe {
                        let _ = UnhookWindowsHookEx(hook);
                    }
                    CONTEXT.with(|slot| *slot.borrow_mut() = None);
                })
                .map_err(|e| format!("Cannot start shortcut observer: {e}"))?;

            match result.recv() {
                Ok(Ok(thread_id)) => Ok(Self {
                    thread_id,
                    thread: Some(thread),
                    enabled,
                }),
                Ok(Err(error)) => {
                    let _ = thread.join();
                    Err(error)
                }
                Err(_) => {
                    enabled.store(false, Ordering::Release);
                    let _ = thread.join();
                    Err("Shortcut observer exited during startup".into())
                }
            }
        }
    }

    impl Drop for WinSpaceObserver {
        fn drop(&mut self) {
            self.enabled.store(false, Ordering::Release);
            // SAFETY: ID belongs to our dedicated thread, with its queue initialized
            // before startup acknowledged. No keyboard state or other threads are changed.
            if unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_ok()
            {
                if let Some(thread) = self.thread.take() {
                    let _ = thread.join();
                }
            }
        }
    }
}

#[cfg(windows)]
pub use native::WinSpaceObserver;

#[cfg(not(windows))]
pub struct WinSpaceObserver;

#[cfg(not(windows))]
impl WinSpaceObserver {
    pub fn start(_on_release: impl Fn() + Send + 'static) -> Result<Self, String> {
        Err("Win+Space is only supported on Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT_WIN: u32 = 0x5b;
    const RIGHT_WIN: u32 = 0x5c;
    const SPACE: u32 = 0x20;

    #[test]
    fn recognizes_only_the_exact_shortcut() {
        for value in ["Super+Space", "super+space", "Win+Space", "Meta+Space"] {
            assert!(is_win_space(value));
        }
        for value in [
            "Ctrl+Space",
            "Super+Shift+Space",
            "Super+J",
            "Space",
            "Super++Space",
        ] {
            assert!(!is_win_space(value));
        }
    }

    #[test]
    fn fires_only_after_both_keys_are_released() {
        for win in [LEFT_WIN, RIGHT_WIN] {
            let mut state = Gesture::default();
            assert!(!state.event(win, true));
            assert!(!state.event(SPACE, true));
            assert!(!state.event(SPACE, false));
            assert!(state.event(win, false));
            assert!(!state.event(win, false));
        }
    }

    #[test]
    fn release_order_and_repeats_do_not_duplicate_the_action() {
        let mut state = Gesture::default();
        assert!(!state.event(LEFT_WIN, true));
        for _ in 0..10 {
            assert!(!state.event(SPACE, true));
        }
        assert!(!state.event(LEFT_WIN, false));
        assert!(state.event(SPACE, false));
        assert!(!state.event(SPACE, false));
    }

    #[test]
    fn extra_modifiers_and_other_keys_cancel_the_gesture() {
        for extra in [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x41] {
            let mut state = Gesture::default();
            state.event(LEFT_WIN, true);
            state.event(SPACE, true);
            state.event(extra, true);
            state.event(extra, false);
            assert!(!state.event(SPACE, false));
            assert!(!state.event(LEFT_WIN, false));
        }
    }

    #[test]
    fn space_typing_and_win_alone_are_untouched() {
        let mut state = Gesture::default();
        for key in [SPACE, LEFT_WIN, RIGHT_WIN, 0x41, SPACE] {
            assert!(!state.event(key, true));
            assert!(!state.event(key, false));
        }
    }

    #[test]
    fn tracks_both_windows_keys_independently() {
        let mut state = Gesture::default();
        state.event(LEFT_WIN, true);
        state.event(RIGHT_WIN, true);
        state.event(SPACE, true);
        assert!(!state.event(LEFT_WIN, false));
        assert!(!state.event(SPACE, false));
        assert!(state.event(RIGHT_WIN, false));
    }

    #[test]
    fn holding_another_modifier_first_does_not_arm_the_shortcut() {
        let mut state = Gesture::default();
        state.event(0xa2, true);
        state.event(LEFT_WIN, true);
        state.event(SPACE, true);
        state.event(0xa2, false);
        assert!(!state.event(SPACE, false));
        assert!(!state.event(LEFT_WIN, false));
    }

    #[test]
    fn pressing_space_before_win_is_not_the_shortcut() {
        let mut state = Gesture::default();
        state.event(SPACE, true);
        state.event(LEFT_WIN, true);
        assert!(!state.event(SPACE, false));
        assert!(!state.event(LEFT_WIN, false));
    }

    #[test]
    fn cancelling_does_not_break_the_next_chord() {
        let mut state = Gesture::default();
        state.event(LEFT_WIN, true);
        state.event(SPACE, true);
        state.event(0x41, true);
        state.event(0x41, false);
        state.event(SPACE, false);
        assert!(!state.event(LEFT_WIN, false));
        state.event(RIGHT_WIN, true);
        state.event(SPACE, true);
        state.event(RIGHT_WIN, false);
        assert!(state.event(SPACE, false));
    }
}

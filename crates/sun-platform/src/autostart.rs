//! Current-user startup registration. Tests never access the registry.

#[cfg(any(windows, test))]
fn startup_command(executable: &std::path::Path) -> String {
    format!("\"{}\" --autostart", executable.display())
}

#[cfg(any(windows, test))]
fn startup_disabled(bytes: &[u8]) -> bool {
    let Some(prefix) = bytes.get(..4) else {
        return false;
    };
    matches!(
        u32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]),
        3 | 7
    )
}

/// Reconcile the current executable path, even if the saved setting has not changed.
/// Never override a deliberate disable in Windows Startup apps.
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
        RegKey,
    };
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let user = RegKey::predef(HKEY_CURRENT_USER);

    if !enabled {
        let key = match user.open_subkey_with_flags(RUN, KEY_SET_VALUE) {
            Ok(key) => key,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("Cannot open Windows startup entries: {error}")),
        };
        return match key.delete_value("Sunny") {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Cannot remove Sunny from Windows startup: {error}")),
        };
    }

    if let Ok(approval) =
        user.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run")
    {
        if let Ok(value) = approval.get_raw_value("Sunny") {
            if startup_disabled(&value.bytes) {
                return Err("Windows has disabled Sunny in Startup apps. Enable Sunny in Windows Settings > Apps > Startup, then enable Open at Login again.".into());
            }
        }
    }
    let executable =
        std::env::current_exe().map_err(|e| format!("Cannot find Sunny executable: {e}"))?;
    let (key, _) = user
        .create_subkey(RUN)
        .map_err(|e| format!("Cannot open Windows startup entries: {e}"))?;
    key.set_value("Sunny", &startup_command(&executable))
        .map_err(|e| format!("Cannot register Sunny for Windows startup: {e}"))
}

#[cfg(not(windows))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Err("Open at Login is supported only on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_the_executable_and_requests_a_background_start() {
        assert_eq!(
            startup_command(std::path::Path::new(r"C:\Program Files\Sunny\sunny.exe")),
            r#""C:\Program Files\Sunny\sunny.exe" --autostart"#
        );
    }

    #[test]
    fn keeps_unicode_paths_intact() {
        assert_eq!(
            startup_command(std::path::Path::new(r"C:\Users\José\Sunny\sunny.exe")),
            r#""C:\Users\José\Sunny\sunny.exe" --autostart"#
        );
    }

    #[test]
    fn recognizes_windows_disabled_startup_states_without_overriding_them() {
        assert!(startup_disabled(&[3, 0, 0, 0]));
        assert!(startup_disabled(&[7, 0, 0, 0]));
        assert!(!startup_disabled(&[2, 0, 0, 0]));
        assert!(!startup_disabled(&[6, 0, 0, 0]));
        assert!(!startup_disabled(&[]));
        assert!(!startup_disabled(&[3]));
    }
}

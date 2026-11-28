//! Atomic file replacement shared by the small JSON state files Sunny keeps.
//!
//! `fs::rename` replaces the destination atomically on Windows, but it can fail
//! *transiently* with `ERROR_ACCESS_DENIED` (5), `ERROR_SHARING_VIOLATION` (32)
//! or `ERROR_LOCK_VIOLATION` (33) while another process briefly holds the
//! destination open — a sync client such as OneDrive, an antivirus scanner, or
//! the file indexer. The save is not wrong, it lost a race, so retry briefly
//! instead of reporting a failure and discarding the user's data.
//!
//! Seen in the settings suite: eight concurrent saves, each writing its own
//! temporary file, still produced `Access is denied. (os error 5)` on the rename.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

/// Total attempts, including the first one.
const ATTEMPTS: u32 = 8;
/// Backoff grows linearly — 5, 10, … 35 ms, at most 140 ms in total.
const FIRST_DELAY_MS: u64 = 5;

/// Replace `destination` with `temporary`, retrying transient Windows lock errors.
pub(crate) fn replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    replace_with(temporary, destination, |from, to| fs::rename(from, to))
}

fn replace_with(
    temporary: &Path,
    destination: &Path,
    mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        match rename(temporary, destination) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < ATTEMPTS && is_transient(&error) => {
                std::thread::sleep(Duration::from_millis(FIRST_DELAY_MS * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient(error: &io::Error) -> bool {
    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(5) | Some(32) | Some(33))
    }
    // Unix renames either succeed or fail permanently: there is no sharing
    // violation to wait out.
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::ErrorKind;

    fn transient() -> io::Error {
        #[cfg(windows)]
        {
            io::Error::from_raw_os_error(5)
        }
        #[cfg(not(windows))]
        {
            io::Error::new(ErrorKind::PermissionDenied, "transient")
        }
    }

    #[test]
    fn retries_a_transient_lock_until_the_rename_succeeds() {
        let calls = Cell::new(0);
        let result = replace_with(Path::new("t"), Path::new("d"), |_, _| {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err(transient())
            } else {
                Ok(())
            }
        });

        assert!(result.is_ok());
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn gives_up_after_the_attempt_budget() {
        let calls = Cell::new(0);
        let result = replace_with(Path::new("t"), Path::new("d"), |_, _| {
            calls.set(calls.get() + 1);
            Err(transient())
        });

        assert!(result.is_err());
        assert_eq!(calls.get(), ATTEMPTS as usize);
    }

    #[test]
    fn a_permanent_error_is_not_retried() {
        let calls = Cell::new(0);
        let result = replace_with(Path::new("t"), Path::new("d"), |_, _| {
            calls.set(calls.get() + 1);
            Err(io::Error::new(ErrorKind::NotFound, "no such directory"))
        });

        assert!(result.is_err());
        assert_eq!(calls.get(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_lock_errors_are_transient_but_a_missing_path_is_not() {
        for code in [5, 32, 33] {
            assert!(is_transient(&io::Error::from_raw_os_error(code)));
        }
        assert!(!is_transient(&io::Error::from_raw_os_error(2)));
    }
}

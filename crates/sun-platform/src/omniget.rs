//! Optional installed OmniGet CLI. Never starts a shell, browser, or GUI app.

use std::path::{Path, PathBuf};

fn valid_cli(path: &Path) -> bool {
    path.is_absolute()
        && !path.to_string_lossy().starts_with(r"\\")
        && path
            .file_name()
            .map(|s| s.to_string_lossy().eq_ignore_ascii_case("omniget-cli.exe"))
            .unwrap_or(false)
        && path.is_file()
}

pub fn find_cli(configured: &str) -> Option<PathBuf> {
    if !configured.trim().is_empty() {
        let path = PathBuf::from(configured.trim());
        return valid_cli(&path).then_some(path);
    }
    if let Some(path) = managed_cli_path().filter(|p| valid_cli(p)) {
        return Some(path);
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|s| std::env::split_paths(&s).collect())
        .unwrap_or_default();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.extend([
            PathBuf::from(&local).join("Programs").join("omniget"),
            PathBuf::from(local).join("omniget"),
        ]);
    }
    if let Some(programs) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(programs).join("omniget"));
    }
    dirs.into_iter()
        .map(|dir| dir.join("omniget-cli.exe"))
        .find(|p| valid_cli(p))
}

/// Sunny-owned CLI location; downloaded only by an explicit Settings action.
pub fn managed_cli_path() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("Sunny")
            .join("tools")
            .join("omniget")
            .join("omniget-cli.exe"),
    )
}

/// Presence checks only: never execute tools or trigger upstream downloads here.
/// OmniGet may also have custom binary overrides; this list is advisory.
pub fn missing_download_tools() -> Vec<String> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|s| std::env::split_paths(&s).collect())
        .unwrap_or_default();
    if let Some(data) = std::env::var_os("OMNIGET_DATA_DIR") {
        dirs.push(PathBuf::from(data).join("bin"));
    }
    if let Some(data) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(&data).join("wtf.tonho.omniget").join("bin"));
        dirs.push(PathBuf::from(data).join("omniget").join("bin"));
    }
    if let Some(programs) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(programs).join("nodejs"));
    }
    let found = |name: &str| {
        dirs.iter()
            .filter(|d| d.is_absolute() && !d.to_string_lossy().starts_with(r"\\"))
            .any(|d| d.join(name).is_file())
    };
    let mut missing = Vec::new();
    for (name, label) in [
        ("yt-dlp.exe", "yt-dlp"),
        ("ffmpeg.exe", "FFmpeg"),
        ("ffprobe.exe", "FFprobe"),
    ] {
        if !found(name) {
            missing.push(label.into());
        }
    }
    if !["deno.exe", "node.exe", "bun.exe"]
        .iter()
        .any(|name| found(name))
    {
        missing.push("JavaScript runtime (Deno/Node/Bun)".into());
    }
    missing
}

#[cfg(windows)]
mod process {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read},
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::{Child, Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        },
        time::{Duration, Instant},
    };
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };

    struct Process {
        child: Child,
        job: HANDLE,
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: this handle was created below and is owned only here. Closing
            // a kill-on-close job terminates this CLI and its remaining children.
            let _ = unsafe { CloseHandle(self.job) };
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    fn spawn(command: &mut Command) -> Result<Process, String> {
        // SAFETY: no security descriptor/name; returns a new owned job handle.
        let job = unsafe { CreateJobObjectW(None, None) }
            .map_err(|_| "Could not create an isolated download job.")?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: points to a live, correctly sized structure for this info class.
        if unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .is_err()
        {
            // SAFETY: job is owned here, and not referenced after this branch.
            let _ = unsafe { CloseHandle(job) };
            return Err("Could not isolate the OmniGet process.".into());
        }
        let child = command
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let child = match child {
            Ok(child) => child,
            Err(_) => {
                // SAFETY: same owned job handle; no child was created.
                let _ = unsafe { CloseHandle(job) };
                return Err(
                    "Could not start OmniGet. Check the CLI path in Settings → General.".into(),
                );
            }
        };
        let mut process = Process { child, job };
        // SAFETY: Child retains ownership of its live process handle. The job is
        // owned by Process and is closed only by its cleanup, before joining readers.
        if unsafe { AssignProcessToJobObject(job, HANDLE(process.child.as_raw_handle())) }.is_err()
        {
            let _ = process.child.kill();
            return Err("Windows could not isolate OmniGet. The download was not started.".into());
        }
        Ok(process)
    }

    /// Bounded line reads, including a provider that never writes a newline.
    fn read_line(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
        let mut bytes = Vec::new();
        loop {
            let chunk = reader.fill_buf()?;
            if chunk.is_empty() {
                return Ok(
                    (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
                );
            }
            let count = chunk
                .iter()
                .position(|b| *b == b'\n')
                .map(|n| n + 1)
                .unwrap_or(chunk.len());
            if bytes.len() + count > 64 * 1024 {
                return Err(std::io::Error::other("OmniGet output line too large"));
            }
            bytes.extend_from_slice(&chunk[..count]);
            let done = bytes.last() == Some(&b'\n');
            reader.consume(count);
            if done {
                return Ok(Some(String::from_utf8_lossy(&bytes).into_owned()));
            }
        }
    }

    pub(super) fn run(
        command: &mut Command,
        cancel: Arc<AtomicBool>,
        mut on_line: impl FnMut(&str),
        timeout: Duration,
    ) -> Result<(), String> {
        let mut process = spawn(command)?;
        let stdout = process
            .child
            .stdout
            .take()
            .ok_or("OmniGet output unavailable.")?;
        let stderr = process
            .child
            .stderr
            .take()
            .ok_or("OmniGet diagnostics unavailable.")?;
        let (send, receive) = mpsc::sync_channel::<Result<String, ()>>(32);
        let reader = std::thread::Builder::new()
            .name("omniget-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_line(&mut reader) {
                        Ok(Some(line)) => {
                            if send.send(Ok(line)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => {
                            let _ = send.send(Err(()));
                            break;
                        }
                    }
                }
            })
            .map_err(|_| "Could not read OmniGet output.")?;
        let diagnostics = std::thread::Builder::new()
            .name("omniget-diagnostics".into())
            .spawn(move || {
                // Drain but do not log/store diagnostics: they can contain URL tokens.
                let _ = std::io::copy(
                    &mut BufReader::new(stderr).take(u64::MAX),
                    &mut std::io::sink(),
                );
            });
        if diagnostics.is_err() {
            drop(process);
            drop(receive);
            let _ = reader.join();
            return Err("Could not read OmniGet diagnostics.".into());
        }
        let started = Instant::now();
        let result = loop {
            if cancel.load(Ordering::Relaxed) {
                break Err("Download cancelled.".into());
            }
            if started.elapsed() >= timeout {
                break Err("OmniGet exceeded the six-hour download limit. Try downloading directly in OmniGet.".into());
            }
            match receive.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(line)) => on_line(&line),
                Ok(Err(())) => break Err(
                    "OmniGet returned invalid or oversized output. Update OmniGet and try again."
                        .into(),
                ),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_millis(20))
                }
            }
            match process.child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err("OmniGet could not download this URL. Check its dependencies, site support or login requirements in OmniGet, then try again.".into())
                    }
                }
                Ok(None) => {}
                Err(_) => break Err("Could not check the OmniGet process.".into()),
            }
        };
        // Kill descendants before joining readers: subprocesses can inherit pipes.
        drop(process);
        let mut output_valid = true;
        for line in receive {
            match line {
                Ok(line) => on_line(&line),
                Err(()) => output_valid = false,
            }
        }
        let read_ok = reader.join().is_ok();
        let diagnostics_ok = diagnostics
            .map(|thread| thread.join().is_ok())
            .unwrap_or(false);
        result.and_then(|()| {
            if output_valid && read_ok && diagnostics_ok {
                Ok(())
            } else {
                Err("Could not read OmniGet output.".into())
            }
        })
    }

    pub fn download(
        path: &Path,
        args: &[String],
        cancel: Arc<AtomicBool>,
        on_line: impl FnMut(&str),
    ) -> Result<(), String> {
        if !valid_cli(path) {
            return Err(
                "Select a local, absolute path to omniget-cli.exe in Settings → General.".into(),
            );
        }
        run(
            Command::new(path).args(args),
            cancel,
            on_line,
            Duration::from_secs(6 * 60 * 60),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn bounded_reader_rejects_oversized_lines_and_reads_final_unterminated_line() {
            assert!(read_line(&mut std::io::Cursor::new(vec![b'a'; 65537])).is_err());
            assert_eq!(
                read_line(&mut std::io::Cursor::new(b"hello")).unwrap(),
                Some("hello".into())
            );
            assert!(read_line(&mut std::io::Cursor::new(b"")).unwrap().is_none());
        }

        #[test]
        fn process_fixture() {
            if std::env::var("SUNNY_OMNIGET_TEST_CHILD").as_deref() != Ok("1") {
                return;
            }
            if std::env::var("SUNNY_OMNIGET_TEST_FAIL").is_ok() {
                std::process::exit(7);
            }
            if std::env::var("SUNNY_OMNIGET_TEST_OVERSIZE").is_ok() {
                println!("{}", "a".repeat(65537));
                return;
            }
            println!("{{\"type\":\"progress\",\"percent\":25}}");
            if std::env::var("SUNNY_OMNIGET_TEST_WAIT").is_ok() {
                std::thread::sleep(Duration::from_secs(3));
            }
            println!("{{\"type\":\"complete\",\"success\":true,\"file_path\":\"test.mp4\"}}");
        }

        fn fixture_command() -> Command {
            let mut cmd = Command::new(std::env::current_exe().unwrap());
            cmd.args([
                "--exact",
                "omniget::process::tests::process_fixture",
                "--nocapture",
            ]);
            cmd.env("SUNNY_OMNIGET_TEST_CHILD", "1");
            cmd
        }

        #[test]
        fn runner_streams_progress_without_network_or_an_omniget_installation() {
            let mut lines = Vec::new();
            run(
                &mut fixture_command(),
                Arc::new(AtomicBool::new(false)),
                |line| lines.push(line.to_owned()),
                Duration::from_secs(5),
            )
            .unwrap();
            assert!(lines.iter().any(|line| line.contains("\"percent\":25")));
            assert!(lines.iter().any(|line| line.contains("\"success\":true")));
        }

        #[test]
        fn runner_cancels_and_reaps_only_its_own_fixture() {
            let cancel = Arc::new(AtomicBool::new(false));
            let signal = cancel.clone();
            let now = Instant::now();
            let mut cmd = fixture_command();
            cmd.env("SUNNY_OMNIGET_TEST_WAIT", "1");
            assert!(run(
                &mut cmd,
                cancel,
                |line| {
                    if line.contains("\"percent\":25") {
                        signal.store(true, Ordering::Relaxed);
                    }
                },
                Duration::from_secs(5)
            )
            .is_err());
            assert!(now.elapsed() < Duration::from_secs(2));
        }

        #[test]
        fn runner_reports_exit_failure_oversized_output_and_timeout() {
            for env in [
                "SUNNY_OMNIGET_TEST_FAIL",
                "SUNNY_OMNIGET_TEST_OVERSIZE",
                "SUNNY_OMNIGET_TEST_WAIT",
            ] {
                let mut cmd = fixture_command();
                cmd.env(env, "1");
                assert!(
                    run(
                        &mut cmd,
                        Arc::new(AtomicBool::new(false)),
                        |_| {},
                        Duration::from_millis(250)
                    )
                    .is_err(),
                    "{env}"
                );
            }
        }
    }
}

#[cfg(windows)]
pub use process::download;

#[cfg(not(windows))]
pub fn download(
    _path: &Path,
    _args: &[String],
    _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _on_line: impl FnMut(&str),
) -> Result<(), String> {
    Err("Video Download is supported on Windows only.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_rejects_gui_relative_network_and_nonexistent_paths() {
        for path in [
            "omniget-cli.exe",
            r"C:\does-not-exist\omniget-cli.exe",
            r"\\server\share\omniget-cli.exe",
            r"C:\Windows\notepad.exe",
        ] {
            assert!(find_cli(path).is_none());
        }
    }

    #[test]
    fn detection_accepts_an_explicit_cli_file_without_executing_it() {
        let dir =
            std::env::temp_dir().join(format!("sunny-omniget-detection-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("omniget-cli.exe");
        std::fs::write(&path, b"not an executable; discovery must not run it").unwrap();
        assert_eq!(find_cli(&path.to_string_lossy()), Some(path.clone()));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}

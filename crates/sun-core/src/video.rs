//! Pure validation and progress parsing for the optional OmniGet CLI integration.

use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

/// Build only the download subcommand. URLs never go through a command shell.
pub fn download_args(url: &str, quality: &str, output: &str) -> Result<Vec<String>, String> {
    let url = url.trim();
    if url.is_empty() || url.len() > 4096 || url.chars().any(char::is_control) {
        return Err("Paste a valid HTTP or HTTPS video URL (up to 4096 bytes).".into());
    }
    let parsed = url::Url::parse(url).map_err(|_| "Paste a valid HTTP or HTTPS video URL.")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Use an HTTP or HTTPS URL without embedded login credentials.".into());
    }
    let mut args = vec![
        "--json".into(),
        "download".into(),
        "--output".into(),
        output.into(),
    ];
    match quality {
        "best" => {}
        "audio" => args.push("--audio-only".into()),
        "480" | "720" | "1080" | "2160" => args.extend(["--quality".into(), quality.into()]),
        _ => return Err("Choose one of the listed download qualities.".into()),
    }
    args.extend(["--".into(), url.into()]);
    Ok(args)
}

#[derive(Debug, PartialEq)]
pub enum CliUpdate {
    Progress(f64),
    Complete(String),
    Failed,
}

/// Ignore diagnostics and unknown records. Never forward raw errors/URLs to logs.
pub fn parse_line(line: &str) -> Option<CliUpdate> {
    if line.len() > 64 * 1024 {
        return None;
    }
    let value: serde_json::Value = match serde_json::from_str(line.trim()) {
        Ok(value) => value,
        Err(_) => {
            // OmniGet's download.rs currently formats Option fields as Some/None.
            // Accept only its exact progress prefix; do not evaluate the rest.
            let number = line
                .trim()
                .strip_prefix("{\"type\":\"progress\",\"percent\":")?
                .split(',')
                .next()?;
            return progress(number.parse().ok()?);
        }
    };
    match value.get("type")?.as_str()? {
        "progress" => progress(value.get("percent")?.as_f64()?),
        "complete" if value.get("success").and_then(|v| v.as_bool()) == Some(true) => {
            match value
                .get("file_path")
                .and_then(|v| v.as_str())
                .filter(|p| !p.is_empty() && p.len() <= 32768 && !p.chars().any(char::is_control))
            {
                Some(path) => Some(CliUpdate::Complete(path.into())),
                None => Some(CliUpdate::Failed),
            }
        }
        "complete" => Some(CliUpdate::Failed),
        _ => None,
    }
}

fn progress(percent: f64) -> Option<CliUpdate> {
    (percent.is_finite() && (-3.0..=100.0).contains(&percent))
        .then(|| CliUpdate::Progress(percent.max(0.0)))
}

/// Lifecycle of one download. Serialized as the lowercase strings the UI checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Running,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub id: u64,
    pub revision: u64,
    pub phase: Phase,
    pub percent: Option<f64>,
    pub file_path: Option<String>,
    pub message: String,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            id: 0,
            revision: 0,
            phase: Phase::Idle,
            percent: None,
            file_path: None,
            message: "Paste a video URL to begin.".into(),
        }
    }
}

#[derive(Default)]
struct Job {
    status: Status,
    cancel: Arc<AtomicBool>,
    reported_failure: bool,
}

/// One download at a time; state survives hiding/reopening the launcher.
#[derive(Default)]
pub struct Downloads(Mutex<Job>);

impl Downloads {
    pub fn status(&self) -> Result<Status, String> {
        self.0
            .lock()
            .map(|j| j.status.clone())
            .map_err(|_| "Download state unavailable.".into())
    }

    pub fn begin(&self) -> Result<(Status, Arc<AtomicBool>), String> {
        let mut job = self.0.lock().map_err(|_| "Download state unavailable.")?;
        if job.status.phase == Phase::Running {
            return Err(
                "A download is already running. Cancel it or wait for it to finish.".into(),
            );
        }
        job.status = Status {
            id: job.status.id.saturating_add(1),
            revision: 0,
            phase: Phase::Running,
            message: "Starting OmniGet…".into(),
            ..Status::default()
        };
        job.cancel = Arc::new(AtomicBool::new(false));
        job.reported_failure = false;
        Ok((job.status.clone(), job.cancel.clone()))
    }

    pub fn update(&self, id: u64, update: CliUpdate) -> Option<Status> {
        let mut job = self.0.lock().ok()?;
        if job.status.id != id || job.status.phase != Phase::Running {
            return None;
        }
        match update {
            CliUpdate::Progress(percent) => {
                // Bound event frequency even if a provider emits thousands of records.
                if job
                    .status
                    .percent
                    .map(|p| (p - percent).abs() < 0.5)
                    .unwrap_or(false)
                {
                    return None;
                }
                job.status.percent = Some(percent);
                job.status.message = "Downloading with OmniGet…".into();
            }
            // A completion record is provisional until the process exits successfully.
            CliUpdate::Complete(path) => {
                job.status.file_path = Some(path);
            }
            CliUpdate::Failed => {
                job.reported_failure = true;
                job.status.message = "OmniGet reported a download error.".into();
            }
        }
        job.status.revision += 1;
        Some(job.status.clone())
    }

    pub fn finish(&self, id: u64, result: Result<(), String>) -> Option<Status> {
        let mut job = self.0.lock().ok()?;
        if job.status.id != id || job.status.phase != Phase::Running {
            return None;
        }
        let cancelled = job.cancel.load(Ordering::Relaxed);
        let success =
            result.is_ok() && job.status.file_path.is_some() && !cancelled && !job.reported_failure;
        let (phase, message) = if cancelled {
            (
                Phase::Cancelled,
                "Download cancelled. Partial files may remain in Downloads.".to_string(),
            )
        } else if success {
            (Phase::Complete, "Saved to Downloads.".to_string())
        } else {
            let message = result.err().unwrap_or_else(|| {
                "OmniGet did not confirm a saved file. Update OmniGet and try again.".into()
            });
            (Phase::Failed, message)
        };
        job.status.phase = phase;
        job.status.message = message;
        if success {
            job.status.percent = Some(100.0);
        } else {
            job.status.file_path = None;
        }
        job.status.revision += 1;
        Some(job.status.clone())
    }

    pub fn cancel(&self, id: u64) -> Result<(), String> {
        let job = self.0.lock().map_err(|_| "Download state unavailable.")?;
        if job.status.id == id && job.status.phase == Phase::Running {
            job.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_without_credentials_or_control_characters_are_accepted() {
        for bad in [
            "",
            "file:///C:/private",
            "--help",
            "https://",
            "https://user:secret@example.com/v",
            "https://example.com/\nvideo",
            "\\\\server\\share",
        ] {
            assert!(
                download_args(bad, "best", "C:\\Downloads").is_err(),
                "{bad}"
            );
        }
        assert!(download_args(
            &format!("https://example.com/{}", "a".repeat(4096)),
            "best",
            "C:\\Downloads"
        )
        .is_err());
        assert!(
            download_args("https://example.com/video?a=1&b=2", "best", "C:\\Downloads").is_ok()
        );
    }

    #[test]
    fn arguments_are_separate_and_quality_is_allowlisted() {
        let args =
            download_args("https://example.com/v?x=&whoami", "720", "C:\\My Downloads").unwrap();
        assert_eq!(
            args,
            [
                "--json",
                "download",
                "--output",
                "C:\\My Downloads",
                "--quality",
                "720",
                "--",
                "https://example.com/v?x=&whoami"
            ]
        );
        assert!(download_args("https://example.com/v", "--proxy=evil", "C:\\Downloads").is_err());
        assert!(
            download_args("https://example.com/v", "audio", "C:\\Downloads")
                .unwrap()
                .contains(&"--audio-only".into())
        );
    }

    #[test]
    fn progress_parses_upstream_json_and_debug_option_output_without_panicking() {
        assert_eq!(
            parse_line(r#"{"type":"progress","percent":42.5,"speed_bps":123}"#),
            Some(CliUpdate::Progress(42.5))
        );
        // Upstream currently prints Rust Option debug values, not valid JSON.
        assert_eq!(
            parse_line(
                r#"{"type":"progress","percent":12,"downloaded_bytes":Some(123),"total_bytes":None}"#
            ),
            Some(CliUpdate::Progress(12.0))
        );
        assert_eq!(
            parse_line(r#"{"type":"progress","percent":-2}"#),
            Some(CliUpdate::Progress(0.0))
        );
        for bad in [
            "",
            "log message",
            r#"{"type":"progress","percent":NaN}"#,
            r#"{"type":"progress","percent":999}"#,
        ] {
            assert_eq!(parse_line(bad), None);
        }
    }

    #[test]
    fn completion_requires_success_and_a_file_path() {
        assert_eq!(
            parse_line(
                r#"{"type":"complete","success":true,"file_path":"C:\\Downloads\\video.mp4"}"#
            ),
            Some(CliUpdate::Complete("C:\\Downloads\\video.mp4".into()))
        );
        assert_eq!(
            parse_line(r#"{"type":"complete","success":false,"error":"secret URL"}"#),
            Some(CliUpdate::Failed)
        );
        assert_eq!(
            parse_line(r#"{"type":"complete","success":true}"#),
            Some(CliUpdate::Failed)
        );
    }

    #[test]
    fn phase_serializes_to_the_strings_the_ui_checks() {
        for (phase, wire) in [
            (Phase::Idle, "idle"),
            (Phase::Running, "running"),
            (Phase::Complete, "complete"),
            (Phase::Failed, "failed"),
            (Phase::Cancelled, "cancelled"),
        ] {
            assert_eq!(
                serde_json::to_string(&phase).unwrap(),
                format!("\"{wire}\"")
            );
        }
    }

    #[test]
    fn single_job_guard_stale_updates_and_cancel_are_isolated() {
        let jobs = Downloads::default();
        let (first, cancel) = jobs.begin().unwrap();
        assert!(jobs.begin().is_err());
        assert!(jobs
            .update(first.id + 1, CliUpdate::Progress(50.0))
            .is_none());
        jobs.cancel(first.id + 1).unwrap();
        assert!(!cancel.load(Ordering::Relaxed));
        jobs.cancel(first.id).unwrap();
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(
            jobs.finish(first.id, Ok(())).unwrap().phase,
            Phase::Cancelled
        );
        let (second, _) = jobs.begin().unwrap();
        assert!(second.id > first.id);
        assert!(jobs.finish(first.id, Ok(())).is_none());
        jobs.update(
            second.id,
            CliUpdate::Complete("C:\\Downloads\\a.mp4".into()),
        );
        assert_eq!(jobs.status().unwrap().phase, Phase::Running);
        assert_eq!(
            jobs.finish(second.id, Ok(())).unwrap().phase,
            Phase::Complete
        );
    }

    #[test]
    fn exit_failure_or_missing_completion_never_claims_success() {
        let jobs = Downloads::default();
        let (s, _) = jobs.begin().unwrap();
        assert_eq!(jobs.finish(s.id, Ok(())).unwrap().phase, Phase::Failed);
        let (s, _) = jobs.begin().unwrap();
        jobs.update(s.id, CliUpdate::Complete("video.mp4".into()));
        let failed = jobs.finish(s.id, Err("OmniGet failed.".into())).unwrap();
        assert_eq!(failed.phase, Phase::Failed);
        assert!(failed.file_path.is_none());
        let (s, _) = jobs.begin().unwrap();
        jobs.update(s.id, CliUpdate::Failed);
        jobs.update(s.id, CliUpdate::Complete("video.mp4".into()));
        assert_eq!(jobs.finish(s.id, Ok(())).unwrap().phase, Phase::Failed);
    }

    #[test]
    fn cli_path_defaults_for_legacy_settings_and_normalizes_without_changing_hotkeys() {
        let mut settings: crate::settings::Settings =
            serde_json::from_str(r#"{"hotkey":"Super+Space"}"#).unwrap();
        assert!(settings.omniget_cli_path.is_empty());
        settings.omniget_cli_path = "  C:\\OmniGet\\omniget-cli.exe  ".into();
        settings.normalize();
        assert_eq!(settings.omniget_cli_path, "C:\\OmniGet\\omniget-cli.exe");
        assert_eq!(settings.hotkey, "Super+Space");
        settings.omniget_cli_path = "bad\npath".into();
        settings.normalize();
        assert!(settings.omniget_cli_path.is_empty());
    }

    #[test]
    fn repetitive_progress_is_coalesced_and_terminal_state_cannot_regress() {
        let jobs = Downloads::default();
        let (s, _) = jobs.begin().unwrap();
        let first = jobs.update(s.id, CliUpdate::Progress(20.0)).unwrap();
        assert!(jobs.update(s.id, CliUpdate::Progress(20.1)).is_none());
        let next = jobs.update(s.id, CliUpdate::Progress(21.0)).unwrap();
        assert!(next.revision > first.revision);
        jobs.finish(s.id, Err("failed".into()));
        assert!(jobs.update(s.id, CliUpdate::Progress(90.0)).is_none());
    }
}

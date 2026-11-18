//! Verified, user-requested OmniGet CLI setup. No subprocess execution.

use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// Pinned Windows x64 archive and digest published on getomniget.com/download.
/// Updating this pair requires reviewing the upstream CLI contract again.
pub const PACKAGE_URL: &str =
    "https://getomniget.com/dl/v1.0.0/omniget-cli-1.0.0-x86_64-pc-windows-msvc.zip";
pub const PACKAGE_SHA256: &str = "06bd1f02c8492cbee718fc113f9838b83a866fbfb419d712b30001d9acbe11b0";
pub const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CLI_BYTES: u64 = 256 * 1024 * 1024;
static NEXT_STAGING_FILE: AtomicU64 = AtomicU64::new(0);

pub fn allowed_package_url(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.port().unwrap_or(443) == 443
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .map(|host| {
                matches!(
                    host,
                    "getomniget.com"
                        | "github.com"
                        | "release-assets.githubusercontent.com"
                        | "objects.githubusercontent.com"
                ) || host.ends_with(".getomniget.com")
            })
            .unwrap_or(false)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn install_cli_package(bytes: &[u8], target: &Path) -> Result<(), String> {
    install_verified(bytes, target, PACKAGE_SHA256)
}

fn install_verified(bytes: &[u8], target: &Path, expected: &str) -> Result<(), String> {
    if bytes.len() > MAX_PACKAGE_BYTES || digest(bytes) != expected {
        return Err("OmniGet checksum mismatch or oversized package. Nothing was installed. Try again later or use the official download site.".into());
    }
    if target.file_name().and_then(|s| s.to_str()) != Some("omniget-cli.exe") {
        return Err("Invalid OmniGet installation target.".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "The OmniGet package is not a valid ZIP archive.")?;
    if zip.len() > 100 {
        return Err("Unexpected OmniGet package layout.".into());
    }
    let mut cli = None;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|_| "Could not read the OmniGet package.")?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if name.file_name().and_then(|s| s.to_str()) != Some("omniget-cli.exe") {
            continue;
        }
        if cli.is_some()
            || entry.is_dir()
            || entry.size() > MAX_CLI_BYTES
            || entry
                .unix_mode()
                .map(|mode| mode & 0o170000 == 0o120000)
                .unwrap_or(false)
        {
            return Err("Unexpected or oversized OmniGet CLI entry.".into());
        }
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(MAX_CLI_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|_| "Could not extract the OmniGet CLI.")?;
        if data.len() as u64 > MAX_CLI_BYTES || !data.starts_with(b"MZ") {
            return Err("The package does not contain a Windows executable.".into());
        }
        cli = Some(data);
    }
    let data = cli.ok_or("omniget-cli.exe is missing from the official package.")?;
    let parent = target
        .parent()
        .ok_or("OmniGet installation folder is unavailable.")?;
    fs::create_dir_all(parent).map_err(|_| "Could not create the Sunny tools folder.")?;
    let stage = parent.join(format!(
        "omniget-cli.{}.{}.tmp",
        std::process::id(),
        NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage)
        .map_err(|_| "Could not stage the OmniGet CLI.")?;
    let result = file.write_all(&data).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| fs::rename(&stage, target));
    if result.is_err() {
        let _ = fs::remove_file(&stage);
        return Err("Could not install OmniGet. Cancel active downloads and try again; the previous CLI was kept.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    #[test]
    fn package_redirects_are_limited_to_trusted_https_hosts() {
        assert!(allowed_package_url(&url::Url::parse(PACKAGE_URL).unwrap()));
        assert!(allowed_package_url(
            &url::Url::parse("https://release-assets.githubusercontent.com/file").unwrap()
        ));
        assert_eq!(PACKAGE_SHA256.len(), 64);
        for value in [
            "http://getomniget.com/file",
            "https://getomniget.com.evil.test/file",
            "https://127.0.0.1/file",
            "https://getomniget.com:8443/file",
            "https://user:password@getomniget.com/file",
        ] {
            assert!(
                !allowed_package_url(&url::Url::parse(value).unwrap()),
                "{value}"
            );
        }
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn target(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("sunny-setup-{name}-{}", std::process::id()))
            .join("omniget-cli.exe")
    }

    #[test]
    fn bad_digest_writes_nothing_and_keeps_existing_cli() {
        let path = target("digest");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"previous").unwrap();
        let bytes = archive(&[("omniget-cli.exe", b"MZfake")]);
        assert!(install_verified(&bytes, &path, &"0".repeat(64)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn verified_archive_installs_only_cli_not_other_omniget_tools() {
        let path = target("allowlist");
        let bytes = archive(&[
            ("omniget-cli.exe", b"MZfake"),
            ("omniget-mcp.exe", b"never install"),
            ("../escape.exe", b"never install"),
        ]);
        install_verified(&bytes, &path, &digest(&bytes)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"MZfake");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_duplicate_traversal_and_invalid_executables_are_rejected() {
        for (name, entries) in [
            ("missing", vec![("README.txt", &b"text"[..])]),
            (
                "duplicate",
                vec![
                    ("a/omniget-cli.exe", &b"MZa"[..]),
                    ("b/omniget-cli.exe", &b"MZb"[..]),
                ],
            ),
            ("traversal", vec![("../omniget-cli.exe", &b"MZbad"[..])]),
            (
                "not-pe",
                vec![("omniget-cli.exe", &b"not an executable"[..])],
            ),
        ] {
            let bytes = archive(&entries);
            let path = target(name);
            assert!(
                install_verified(&bytes, &path, &digest(&bytes)).is_err(),
                "{name}"
            );
            assert!(!path.exists());
            if path.parent().unwrap().exists() {
                std::fs::remove_dir(path.parent().unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn replacement_is_complete_and_failed_rename_cleans_only_its_own_staging_file() {
        let bytes = archive(&[("omniget-cli.exe", b"MZnew")]);
        let path = target("replace");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"MZold").unwrap();
        install_verified(&bytes, &path, &digest(&bytes)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"MZnew");
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let unrelated = path.parent().unwrap().join("unrelated.tmp");
        std::fs::write(&unrelated, b"keep").unwrap();
        assert!(install_verified(&bytes, &path, &digest(&bytes)).is_err());
        assert!(path.is_dir());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            2
        );
        std::fs::remove_file(unrelated).unwrap();
        std::fs::remove_dir(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }
}

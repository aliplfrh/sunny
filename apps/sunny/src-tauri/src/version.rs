#[derive(serde::Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub release_tag: String,
}

#[derive(serde::Deserialize)]
struct ReleaseMetadata {
    version: String,
    release_tag: String,
}

impl VersionInfo {
    pub fn current(version: &str) -> Self {
        Self::from_metadata(version, include_str!("../release-info.json"))
    }

    fn from_metadata(version: &str, metadata: &str) -> Self {
        let release_tag = serde_json::from_str::<ReleaseMetadata>(metadata)
            .ok()
            .filter(|release| release.version == version && release.release_tag.starts_with('v'))
            .map(|release| release.release_tag)
            .unwrap_or_else(|| format!("v{version}"));
        Self {
            version: version.into(),
            release_tag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_release_metadata_uses_the_human_release_label() {
        let info =
            VersionInfo::from_metadata("1.0.3", r#"{"version":"1.0.3","release_tag":"v1.0.2.1"}"#);
        assert_eq!(info.version, "1.0.3");
        assert_eq!(info.release_tag, "v1.0.2.1");
    }

    #[test]
    fn stale_or_broken_metadata_never_overrides_the_running_binary() {
        for metadata in ["not json", r#"{"version":"1.0.0","release_tag":"v1.0.0"}"#] {
            let info = VersionInfo::from_metadata("1.0.4", metadata);
            assert_eq!(info.version, "1.0.4");
            assert_eq!(info.release_tag, "v1.0.4");
        }
    }
}

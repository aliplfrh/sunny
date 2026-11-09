//! Persistent Favorites and successful-launch ranking for the launcher home.

use crate::apps::AppEntry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SUGGESTION_LIMIT: usize = 8;
static NEXT_SAVE: AtomicU64 = AtomicU64::new(0);

/// Only installed app metadata is sent to the frontend, never raw usage records.
#[derive(Debug, Clone, Serialize)]
pub struct LauncherHome {
    pub favorites: Vec<AppEntry>,
    pub suggestions: Vec<AppEntry>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct LaunchUsage {
    launch_count: u64,
    last_launch: u64,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct HistoryData {
    favorites: Vec<String>,
    usage: BTreeMap<String, LaunchUsage>,
    // A persisted logical clock gives deterministic recency without wall-clock changes.
    sequence: u64,
}

/// Local-only app favorites and usage. Mutations persist before changing memory.
/// Callers must serialize access and record launches only after launch succeeds.
#[derive(Default)]
pub struct LauncherHistory {
    path: Option<PathBuf>,
    data: HistoryData,
}

impl LauncherHistory {
    /// Load `%LOCALAPPDATA%/Sunny/launcher-history.json` without creating files.
    /// Missing or corrupt history starts empty. If no local data directory can be
    /// resolved, keep history in memory rather than writing into a shared directory.
    pub fn load() -> Self {
        let base = std::env::var_os("LOCALAPPDATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(dirs::data_local_dir);
        match base {
            Some(base) => Self::load_from(base.join("Sunny").join("launcher-history.json")),
            None => Self::in_memory(),
        }
    }

    /// Load from an explicit file, useful for isolated tests and alternate profiles.
    pub fn load_from(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut data: HistoryData = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut seen = HashSet::new();
        data.favorites = data
            .favorites
            .into_iter()
            .filter_map(|path| app_key(&path).ok())
            .filter(|key| seen.insert(key.clone()))
            .collect();
        let mut usage: BTreeMap<String, LaunchUsage> = BTreeMap::new();
        for (path, record) in data.usage {
            if let Ok(key) = app_key(&path) {
                data.sequence = data.sequence.max(record.last_launch);
                let entry = usage.entry(key).or_default();
                entry.launch_count = entry.launch_count.saturating_add(record.launch_count);
                entry.last_launch = entry.last_launch.max(record.last_launch);
            }
        }
        data.usage = usage;
        Self {
            path: Some(path),
            data,
        }
    }

    /// Construct empty history with no filesystem reads or writes.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Append new favorites in selection order; repeated operations are idempotent.
    pub fn set_favorite(&mut self, path: &str, favorite: bool) -> Result<(), String> {
        let key = app_key(path)?;
        let present = self.data.favorites.contains(&key);
        if present == favorite {
            return Ok(());
        }
        let mut next = self.data.clone();
        if favorite {
            next.favorites.push(key);
        } else {
            next.favorites.retain(|entry| entry != &key);
        }
        self.commit(next)
    }

    /// Count one successful launch. Never call this for failed launch attempts.
    pub fn record_launch(&mut self, path: &str) -> Result<(), String> {
        let key = app_key(path)?;
        let mut next = self.data.clone();
        next.sequence = next.sequence.saturating_add(1);
        let usage = next.usage.entry(key).or_default();
        usage.launch_count = usage.launch_count.saturating_add(1);
        usage.last_launch = next.sequence;
        self.commit(next)
    }

    /// Build home from current installed apps, omitting stale history entries.
    /// Favorites are unbounded; suggestions exclude favorites and are limited to eight.
    pub fn home(&self, apps: &[AppEntry]) -> LauncherHome {
        let mut installed = BTreeMap::new();
        for app in apps {
            if let Ok(key) = app_key(&app.path) {
                installed.entry(key).or_insert(app);
            }
        }
        let favorites = self
            .data
            .favorites
            .iter()
            .filter_map(|key| installed.remove(key).cloned())
            .collect();
        let mut suggestions: Vec<_> = installed.into_iter().collect();
        suggestions.sort_by(|(key_a, app_a), (key_b, app_b)| {
            let usage_a = self.data.usage.get(key_a).cloned().unwrap_or_default();
            let usage_b = self.data.usage.get(key_b).cloned().unwrap_or_default();
            usage_b
                .launch_count
                .cmp(&usage_a.launch_count)
                .then_with(|| usage_b.last_launch.cmp(&usage_a.last_launch))
                .then_with(|| app_a.name.to_lowercase().cmp(&app_b.name.to_lowercase()))
                .then_with(|| app_a.name.cmp(&app_b.name))
                .then_with(|| key_a.cmp(key_b))
        });
        LauncherHome {
            favorites,
            suggestions: suggestions
                .into_iter()
                .take(SUGGESTION_LIMIT)
                .map(|(_, app)| app.clone())
                .collect(),
        }
    }

    fn commit(&mut self, next: HistoryData) -> Result<(), String> {
        if let Some(path) = &self.path {
            save_atomic(path, &next)?;
        }
        self.data = next;
        Ok(())
    }
}

fn app_key(path: &str) -> Result<String, String> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err("Choose a valid application.".into());
    }
    Ok(path.replace('/', "\\").to_lowercase())
}

fn save_atomic(path: &Path, data: &HistoryData) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(data).map_err(|_| "Could not encode launcher history.".to_string())?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|_| "Could not create the launcher history directory.".to_string())?;
    }
    // Same-directory rename replaces the old file atomically, including on Windows.
    // create_new prevents overwriting an unrelated file or following a stale temp link.
    let temporary = path.with_extension(format!(
        "json.{}.{}.tmp",
        std::process::id(),
        NEXT_SAVE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| "Could not create a temporary launcher history file.".to_string())?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("Could not save launcher history. Your previous history was kept.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "sunny-launcher-test-{}-{}",
                std::process::id(),
                NEXT_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&directory).expect("create isolated test directory");
            Self(directory)
        }

        fn history_path(&self) -> PathBuf {
            self.0.join("nested").join("launcher-history.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn app(name: &str) -> AppEntry {
        AppEntry {
            name: name.into(),
            path: format!("C:\\Apps\\{name}.lnk"),
            source: "start-menu",
            category: "Application",
            icon: None,
        }
    }

    fn names(apps: &[AppEntry]) -> Vec<&str> {
        apps.iter().map(|app| app.name.as_str()).collect()
    }

    #[test]
    fn unused_apps_are_alphabetical_and_suggestions_are_bounded() {
        let apps: Vec<_> = (0..12).rev().map(|i| app(&format!("App {i:02}"))).collect();
        let home = LauncherHistory::in_memory().home(&apps);
        assert!(home.favorites.is_empty());
        assert_eq!(
            names(&home.suggestions),
            (0..8).map(|i| format!("App {i:02}")).collect::<Vec<_>>()
        );
    }

    #[test]
    fn favorites_keep_insertion_order_and_are_excluded_from_suggestions() {
        let apps = vec![app("Alpha"), app("Beta"), app("Gamma")];
        let mut history = LauncherHistory::in_memory();
        history.set_favorite(&apps[2].path, true).unwrap();
        history.set_favorite(&apps[0].path, true).unwrap();
        history.set_favorite(&apps[2].path, true).unwrap();
        let home = history.home(&apps);
        assert_eq!(names(&home.favorites), ["Gamma", "Alpha"]);
        assert_eq!(names(&home.suggestions), ["Beta"]);
        history.set_favorite(&apps[2].path, false).unwrap();
        history.set_favorite(&apps[2].path, true).unwrap();
        assert_eq!(names(&history.home(&apps).favorites), ["Alpha", "Gamma"]);
    }

    #[test]
    fn frequency_precedes_recency_and_recency_breaks_equal_counts() {
        let apps = vec![app("Alpha"), app("Beta"), app("Gamma"), app("Delta")];
        let mut history = LauncherHistory::in_memory();
        history.record_launch(&apps[1].path).unwrap();
        history.record_launch(&apps[1].path).unwrap();
        history.record_launch(&apps[2].path).unwrap();
        history.record_launch(&apps[0].path).unwrap();
        assert_eq!(
            names(&history.home(&apps).suggestions),
            ["Beta", "Alpha", "Gamma", "Delta"]
        );
    }

    #[test]
    fn removing_a_favorite_preserves_its_usage_rank() {
        let apps = vec![app("Alpha"), app("Beta")];
        let mut history = LauncherHistory::in_memory();
        history.set_favorite(&apps[1].path, true).unwrap();
        history.record_launch(&apps[1].path).unwrap();
        history.set_favorite(&apps[1].path, false).unwrap();
        assert_eq!(names(&history.home(&apps).suggestions), ["Beta", "Alpha"]);
    }

    #[test]
    fn windows_path_case_and_separator_variants_identify_one_app() {
        let apps = vec![app("Alpha"), app("Beta")];
        let mut history = LauncherHistory::in_memory();
        history.set_favorite("c:/APPS/alpha.LNK", true).unwrap();
        history.set_favorite(&apps[0].path, true).unwrap();
        history.record_launch("c:/apps/BETA.lnk").unwrap();
        assert_eq!(names(&history.home(&apps).favorites), ["Alpha"]);
        history.set_favorite("C:/APPS/ALPHA.LNK", false).unwrap();
        assert_eq!(names(&history.home(&apps).suggestions), ["Beta", "Alpha"]);
    }

    #[test]
    fn missing_apps_are_hidden_without_forgetting_the_favorite() {
        let mut history = LauncherHistory::in_memory();
        history.set_favorite(&app("Missing").path, true).unwrap();
        assert!(history.home(&[]).favorites.is_empty());
        assert_eq!(
            names(&history.home(&[app("Missing")]).favorites),
            ["Missing"]
        );
    }

    #[test]
    fn duplicate_app_paths_appear_only_once() {
        let apps = vec![app("Alpha"), app("Alpha")];
        let mut history = LauncherHistory::in_memory();
        assert_eq!(history.home(&apps).suggestions.len(), 1);
        history.set_favorite(&apps[0].path, true).unwrap();
        assert_eq!(history.home(&apps).favorites.len(), 1);
        assert!(history.home(&apps).suggestions.is_empty());
    }

    #[test]
    fn persistence_round_trips_and_can_replace_an_existing_history() {
        let directory = TestDirectory::new();
        let path = directory.history_path();
        let apps = vec![app("Alpha"), app("Beta"), app("Gamma")];
        let mut history = LauncherHistory::load_from(&path);
        history.set_favorite(&apps[2].path, true).unwrap();
        history.set_favorite(&apps[0].path, true).unwrap();
        history.record_launch(&apps[1].path).unwrap();
        let mut reloaded = LauncherHistory::load_from(&path);
        assert_eq!(names(&reloaded.home(&apps).favorites), ["Gamma", "Alpha"]);
        reloaded.set_favorite(&apps[0].path, false).unwrap();
        reloaded.record_launch(&apps[0].path).unwrap();
        let restored = LauncherHistory::load_from(&path).home(&apps);
        assert_eq!(names(&restored.suggestions), ["Alpha", "Beta"]);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn missing_and_corrupt_files_load_empty_and_can_be_saved() {
        let directory = TestDirectory::new();
        let path = directory.history_path();
        assert!(LauncherHistory::load_from(&path)
            .home(&[])
            .favorites
            .is_empty());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for invalid in ["not json", "null", r#"{"favorites":42}"#] {
            fs::write(&path, invalid).unwrap();
            let mut history = LauncherHistory::load_from(&path);
            assert!(history.home(&[app("Alpha")]).favorites.is_empty());
            history.set_favorite(&app("Alpha").path, true).unwrap();
            assert_eq!(
                LauncherHistory::load_from(&path)
                    .home(&[app("Alpha")])
                    .favorites
                    .len(),
                1
            );
        }
    }

    #[test]
    fn save_failure_preserves_previous_favorites_and_usage() {
        let directory = TestDirectory::new();
        let path = directory.history_path();
        let apps = vec![app("Alpha"), app("Beta")];
        let mut history = LauncherHistory::load_from(&path);
        history.set_favorite(&apps[0].path, true).unwrap();
        history.record_launch(&apps[1].path).unwrap();
        // A directory at the destination deterministically blocks atomic replacement.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(history.set_favorite(&apps[0].path, false).is_err());
        assert!(history.set_favorite(&apps[1].path, true).is_err());
        assert!(history.record_launch(&apps[0].path).is_err());
        assert_eq!(names(&history.home(&apps).favorites), ["Alpha"]);
        fs::remove_dir(&path).unwrap();
        history.set_favorite(&apps[0].path, false).unwrap();
        assert_eq!(names(&history.home(&apps).suggestions), ["Beta", "Alpha"]);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn blocked_parent_reports_an_error_without_leaking_paths() {
        let directory = TestDirectory::new();
        let parent = directory.0.join("private-user-name");
        fs::write(&parent, "block directory creation").unwrap();
        let mut history = LauncherHistory::load_from(parent.join("history.json"));
        let error = history.record_launch(&app("Beta").path).unwrap_err();
        assert!(!error.contains("private-user-name"));
        assert_eq!(
            names(&history.home(&[app("Alpha"), app("Beta")]).suggestions),
            ["Alpha", "Beta"]
        );
    }

    #[test]
    fn empty_paths_are_rejected_without_creating_history() {
        let directory = TestDirectory::new();
        let path = directory.history_path();
        let mut history = LauncherHistory::load_from(&path);
        for invalid in ["", " \t", "bad\0path"] {
            assert!(history.record_launch(invalid).is_err());
            assert!(history.set_favorite(invalid, true).is_err());
        }
        assert!(!path.exists());
    }

    #[test]
    fn home_payload_contains_apps_not_private_usage_metadata() {
        let mut history = LauncherHistory::in_memory();
        history.record_launch(&app("Alpha").path).unwrap();
        let payload = serde_json::to_value(history.home(&[app("Alpha")])).unwrap();
        assert_eq!(payload.as_object().unwrap().len(), 2);
        assert_eq!(payload["suggestions"][0]["name"], "Alpha");
        assert!(payload["suggestions"][0].get("launch_count").is_none());
        assert!(payload["suggestions"][0].get("last_launch").is_none());
    }

    #[test]
    fn loaded_history_normalizes_duplicates_and_repairs_the_recency_clock() {
        let directory = TestDirectory::new();
        let path = directory.history_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "favorites": ["c:/apps/GAMMA.lnk", "C:\\Apps\\Gamma.lnk", ""],
                "usage": {
                    "c:/apps/ALPHA.lnk": {"launch_count": 1, "last_launch": 20},
                    "C:\\Apps\\Alpha.lnk": {"launch_count": 1, "last_launch": 10},
                    "c:/apps/BETA.lnk": {"launch_count": 1, "last_launch": 15},
                    "": {"launch_count": 100, "last_launch": 100}
                },
                "sequence": 0
            }))
            .unwrap(),
        )
        .unwrap();
        let mut history = LauncherHistory::load_from(&path);
        let apps = vec![app("Alpha"), app("Beta"), app("Gamma")];
        assert_eq!(names(&history.home(&apps).favorites), ["Gamma"]);
        assert_eq!(names(&history.home(&apps).suggestions), ["Alpha", "Beta"]);
        history.record_launch(&apps[1].path).unwrap();
        assert_eq!(
            names(&LauncherHistory::load_from(&path).home(&apps).suggestions),
            ["Beta", "Alpha"]
        );
    }

    #[test]
    fn counters_saturate_instead_of_panicking_on_extreme_loaded_values() {
        let mut history = LauncherHistory::in_memory();
        let beta = app("Beta");
        history.data.sequence = u64::MAX;
        history.data.usage.insert(
            app_key(&beta.path).unwrap(),
            LaunchUsage {
                launch_count: u64::MAX,
                last_launch: u64::MAX,
            },
        );
        history.record_launch(&beta.path).unwrap();
        assert_eq!(
            names(&history.home(&[app("Alpha"), beta]).suggestions),
            ["Beta", "Alpha"]
        );
    }

    #[test]
    fn alphabetical_ties_are_deterministic_regardless_of_input_order() {
        let mut a = app("Alpha");
        let mut b = app("alpha");
        a.path = "path-a".into();
        b.path = "path-b".into();
        let history = LauncherHistory::in_memory();
        let forward = history.home(&[a.clone(), b.clone()]);
        let reverse = history.home(&[b, a]);
        assert_eq!(names(&forward.suggestions), ["Alpha", "alpha"]);
        assert_eq!(names(&reverse.suggestions), ["Alpha", "alpha"]);
    }

    #[test]
    fn favorites_are_not_capped_and_store_app_ids_work() {
        let mut history = LauncherHistory::in_memory();
        let mut apps: Vec<_> = (0..12).map(|i| app(&format!("App {i}"))).collect();
        apps[0].path = "Vendor.Package_random!App".into();
        apps[0].source = "store";
        for app in &apps {
            history.set_favorite(&app.path, true).unwrap();
        }
        let home = history.home(&apps);
        assert_eq!(home.favorites.len(), 12);
        assert_eq!(home.favorites[0].path, "Vendor.Package_random!App");
        assert!(home.suggestions.is_empty());
    }
}

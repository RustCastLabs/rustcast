use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::apps::App;

const RECENT_ACTIONS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentActionsFile {
    version: u32,
    keys: Vec<String>,
}

impl RecentActionsFile {
    fn from_keys(keys: Vec<String>) -> Self {
        Self {
            version: RECENT_ACTIONS_SCHEMA_VERSION,
            keys,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecentActions {
    limit: usize,
    keys: Vec<String>,
    storage_path: PathBuf,
}

impl RecentActions {
    pub fn load(limit: usize) -> Self {
        Self::load_with_path(limit, default_storage_path())
    }

    fn load_with_path(limit: usize, storage_path: PathBuf) -> Self {
        let keys = read_keys(&storage_path);
        Self {
            limit,
            keys: normalize_keys(keys, limit),
            storage_path,
        }
    }

    pub fn set_limit(&mut self, limit: usize) -> bool {
        self.limit = limit;
        let old_len = self.keys.len();
        self.keys.truncate(limit);
        self.keys.len() != old_len
    }

    pub fn prune_by<F>(&mut self, keep: F) -> bool
    where
        F: Fn(&str) -> bool,
    {
        let old_len = self.keys.len();
        self.keys.retain(|key| keep(key));
        self.keys.len() != old_len
    }

    pub fn resolve<'a, F>(&self, lookup: F) -> Vec<App>
    where
        F: Fn(&str) -> Option<&'a App>,
    {
        self.keys
            .iter()
            .take(self.limit)
            .filter_map(|key| lookup(key).cloned())
            .collect()
    }

    pub fn record(&mut self, key: &str) {
        if !self.record_without_persist(key) {
            return;
        }

        self.persist_async();
    }

    fn record_without_persist(&mut self, key: &str) -> bool {
        if self.limit == 0 {
            self.keys.clear();
            return false;
        }

        let key = key.trim();
        if key.is_empty() {
            return false;
        }

        self.keys.retain(|existing| existing != key);
        self.keys.insert(0, key.to_string());
        self.keys.truncate(self.limit);
        true
    }

    pub fn persist_async(&self) {
        let path = self.storage_path.clone();
        let payload = RecentActionsFile::from_keys(self.keys.clone());
        std::thread::spawn(move || {
            let _ = persist_to_disk(path.as_path(), &payload);
        });
    }

    #[cfg(test)]
    pub fn persist_blocking(&self) -> io::Result<()> {
        let payload = RecentActionsFile::from_keys(self.keys.clone());
        persist_to_disk(self.storage_path.as_path(), &payload)
    }
}

fn default_storage_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(format!("{home}/.config/rustcast/recent_actions.json"))
}

fn read_keys(path: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let Ok(data) = serde_json::from_str::<RecentActionsFile>(&content) else {
        return Vec::new();
    };

    if data.version != RECENT_ACTIONS_SCHEMA_VERSION {
        return Vec::new();
    }

    data.keys
}

fn normalize_keys(keys: Vec<String>, limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let mut seen = HashSet::new();
    keys.into_iter()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty() && seen.insert(key.clone()))
        .take(limit)
        .collect()
}

fn persist_to_disk(path: &Path, payload: &RecentActionsFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let serialized =
        serde_json::to_vec(payload).map_err(|error| io::Error::other(error.to_string()))?;
    let tmp_path = path.with_extension("json.tmp");

    fs::write(&tmp_path, serialized)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should always move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("rustcast_recent_actions_{suffix}_{nanos}.json"))
    }

    #[test]
    fn record_dedupes_and_reorders() {
        let path = temp_file_path("dedupe");
        let mut recents = RecentActions::load_with_path(3, path);

        recents.record_without_persist("settings");
        recents.record_without_persist("iterm");
        recents.record_without_persist("settings");

        assert_eq!(recents.keys, vec!["settings", "iterm"]);
    }

    #[test]
    fn limit_truncation_works_when_limit_shrinks() {
        let path = temp_file_path("truncate");
        let mut recents = RecentActions::load_with_path(4, path);

        recents.record_without_persist("a");
        recents.record_without_persist("b");
        recents.record_without_persist("c");
        recents.record_without_persist("d");

        assert!(recents.set_limit(2));
        assert_eq!(recents.keys, vec!["d", "c"]);
    }

    #[test]
    fn load_handles_invalid_json_gracefully() {
        let path = temp_file_path("invalid");
        fs::write(&path, "{ this is invalid json").expect("should write invalid fixture");

        let recents = RecentActions::load_with_path(5, path.clone());
        assert!(recents.keys.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn prune_removes_stale_keys() {
        let path = temp_file_path("prune");
        let fixture = RecentActionsFile::from_keys(vec![
            "settings".to_string(),
            "iterm".to_string(),
            "finder".to_string(),
        ]);
        persist_to_disk(path.as_path(), &fixture).expect("fixture should be written");

        let mut recents = RecentActions::load_with_path(5, path.clone());
        let changed = recents.prune_by(|key| key != "finder");

        assert!(changed);
        assert_eq!(recents.keys, vec!["settings", "iterm"]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_load_roundtrip_preserves_order() {
        let path = temp_file_path("roundtrip");
        let mut recents = RecentActions::load_with_path(5, path.clone());

        recents.record_without_persist("settings");
        recents.record_without_persist("iterm");
        recents
            .persist_blocking()
            .expect("recent actions should persist");

        let loaded = RecentActions::load_with_path(5, path.clone());
        assert_eq!(loaded.keys, vec!["iterm", "settings"]);

        let _ = fs::remove_file(path);
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{
    AmbientScene, AppThemeMode, BatterySample, CachedHeadphoneSnapshot, CustomEqProfile,
    HeadphoneAge, HeadphoneFinishOverride, PendingOfflineChanges, SettingsVaultSnapshot,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub theme_mode: AppThemeMode,
    pub minimize_to_tray: bool,
    pub auto_enforce_vault: bool,
    pub last_device_address: Option<String>,
    pub per_device_age: HashMap<String, HeadphoneAge>,
    pub per_device_finish: HashMap<String, HeadphoneFinishOverride>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_mode: AppThemeMode::System,
            minimize_to_tray: true,
            auto_enforce_vault: false,
            last_device_address: None,
            per_device_age: HashMap::new(),
            per_device_finish: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCacheStore {
    pub snapshots: HashMap<String, CachedHeadphoneSnapshot>,
    pub pending: HashMap<String, PendingOfflineChanges>,
    pub vaults: HashMap<String, SettingsVaultSnapshot>,
}

pub struct MurmurStorage {
    dir: PathBuf,
}

impl MurmurStorage {
    pub fn new() -> Self {
        let base = directories::BaseDirs::new()
            .map(|b| b.config_dir().join("Murmur"))
            .unwrap_or_else(|| PathBuf::from(".murmur"));
        let _ = fs::create_dir_all(&base);
        Self { dir: base }
    }

    pub fn data_dir(&self) -> &Path {
        &self.dir
    }

    pub fn load_config(&self) -> AppConfig {
        self.read_json("config.json").unwrap_or_default()
    }

    pub fn save_config(&self, cfg: &AppConfig) {
        self.write_json("config.json", cfg);
    }

    pub fn load_eq_profiles(&self) -> Vec<CustomEqProfile> {
        self.read_json("eq_profiles.json")
            .unwrap_or_else(CustomEqProfile::default_profiles)
    }

    pub fn save_eq_profiles(&self, profiles: &[CustomEqProfile]) {
        self.write_json("eq_profiles.json", profiles);
    }

    pub fn load_ambient_scenes(&self) -> Vec<AmbientScene> {
        self.read_json("ambient_scenes.json")
            .unwrap_or_else(AmbientScene::defaults)
    }

    pub fn save_ambient_scenes(&self, scenes: &[AmbientScene]) {
        self.write_json("ambient_scenes.json", scenes);
    }

    pub fn load_cache_store(&self) -> DeviceCacheStore {
        self.read_json("cache.json").unwrap_or_default()
    }

    pub fn save_cache_store(&self, store: &DeviceCacheStore) {
        self.write_json("cache.json", store);
    }

    pub fn load_battery_history(&self) -> Vec<BatterySample> {
        self.read_json("battery_history.json").unwrap_or_default()
    }

    pub fn save_battery_history(&self, samples: &[BatterySample]) {
        let trimmed = if samples.len() > 500 {
            &samples[samples.len() - 500..]
        } else {
            samples
        };
        self.write_json("battery_history.json", trimmed);
    }

    pub fn export_vault_to_path(
        &self,
        path: &Path,
        vault: &SettingsVaultSnapshot,
    ) -> Result<(), String> {
        let json = serde_json::to_string_pretty(vault).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn import_vault_from_path(&self, path: &Path) -> Result<SettingsVaultSnapshot, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("Invalid Vault JSON: {}", e))
    }

    fn read_json<T: for<'de> Deserialize<'de>>(&self, filename: &str) -> Option<T> {
        let path = self.dir.join(filename);
        let text = fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_json<T: Serialize + ?Sized>(&self, filename: &str, value: &T) {
        let path = self.dir.join(filename);
        if let Ok(text) = serde_json::to_string_pretty(value) {
            let _ = fs::write(path, text);
        }
    }
}

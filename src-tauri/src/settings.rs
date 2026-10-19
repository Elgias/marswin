use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CHARGE_MIN: u32 = 100;
pub const CHARGE_MAX: u32 = 800;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DeviceRef {
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
    pub ver: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub lang: String,
    pub refresh_sec: u32,
    pub mode_check_sec: u32,
    pub timeout_sec: u32,
    pub charge_power: u32,
    pub device: Option<DeviceRef>,
    /// Autostart is switched on once, on the first run; after that the tray toggle owns it.
    pub autostart_initialized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: "uk".into(),
            refresh_sec: 30,
            mode_check_sec: 60,
            timeout_sec: 5,
            charge_power: CHARGE_MAX,
            device: None,
            autostart_initialized: false,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub lang: Option<String>,
    pub refresh_sec: Option<u32>,
    pub mode_check_sec: Option<u32>,
    pub timeout_sec: Option<u32>,
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .map(Settings::sanitized)
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write then rename, so a crash never leaves a half-written file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn apply(&mut self, patch: SettingsPatch) {
        if let Some(v) = patch.lang {
            self.lang = v;
        }
        if let Some(v) = patch.refresh_sec {
            self.refresh_sec = v;
        }
        if let Some(v) = patch.mode_check_sec {
            self.mode_check_sec = v;
        }
        if let Some(v) = patch.timeout_sec {
            self.timeout_sec = v;
        }
        *self = std::mem::take(self).sanitized();
    }

    /// Clamps values a hand-edited file could break.
    fn sanitized(mut self) -> Self {
        if !matches!(self.lang.as_str(), "uk" | "en") {
            self.lang = "uk".into();
        }
        self.refresh_sec = self.refresh_sec.clamp(2, 3600);
        self.mode_check_sec = self.mode_check_sec.clamp(5, 86400);
        self.timeout_sec = self.timeout_sec.clamp(1, 30);
        self.charge_power = self.charge_power.clamp(CHARGE_MIN, CHARGE_MAX);
        self
    }
}

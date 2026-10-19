use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// `refresh_sec` value for adaptive polling.
pub const AUTO: u32 = 0;

/// Adaptive poll interval, after marstip: a quick device is polled often, a slow or silent one is left alone.
/// `answer` is how long the status calls took, or `None` when the device did not answer.
pub fn auto_refresh(answer: Option<Duration>) -> Duration {
    let secs = match answer.map(|d| d.as_millis()) {
        None => 30,
        Some(ms) if ms < 500 => 3,
        Some(ms) if ms < 2000 => 5,
        Some(ms) if ms < 5000 => 10,
        Some(_) => 30,
    };
    Duration::from_secs(secs)
}

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
    /// 0 picks the interval from how fast the device answers (see `auto_refresh`).
    pub refresh_sec: u32,
    pub mode_check_sec: u32,
    pub timeout_sec: u32,
    pub device: Option<DeviceRef>,
    /// Autostart is switched on once, on the first run; after that the tray menu and the settings switch own it.
    pub autostart_initialized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: "uk".into(),
            refresh_sec: AUTO,
            mode_check_sec: 60,
            timeout_sec: 5,
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
        if self.refresh_sec != AUTO {
            self.refresh_sec = self.refresh_sec.clamp(2, 3600);
        }
        self.mode_check_sec = self.mode_check_sec.clamp(5, 86400);
        self.timeout_sec = self.timeout_sec.clamp(1, 30);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_refresh_follows_answer_time() {
        let ms = |n| Some(Duration::from_millis(n));
        assert_eq!(auto_refresh(ms(120)).as_secs(), 3);
        assert_eq!(auto_refresh(ms(500)).as_secs(), 5);
        assert_eq!(auto_refresh(ms(1999)).as_secs(), 5);
        assert_eq!(auto_refresh(ms(2000)).as_secs(), 10);
        assert_eq!(auto_refresh(ms(6000)).as_secs(), 30);
        assert_eq!(auto_refresh(None).as_secs(), 30);
    }

    #[test]
    fn auto_survives_sanitizing() {
        let mut s = Settings::default();
        assert_eq!(s.refresh_sec, AUTO);
        s.apply(SettingsPatch { lang: None, refresh_sec: Some(1), mode_check_sec: None, timeout_sec: None });
        assert_eq!(s.refresh_sec, 2);
        s.apply(SettingsPatch { lang: None, refresh_sec: Some(AUTO), mode_check_sec: None, timeout_sec: None });
        assert_eq!(s.refresh_sec, AUTO);
    }
}

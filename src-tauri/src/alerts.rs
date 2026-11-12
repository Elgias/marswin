//! Low-charge notices: a warning and a critical level, each shown once per discharge.

use crate::settings::Settings;
use crate::snapshot::{Flow, Snapshot};
use serde::Serialize;

/// A level fires again only after the charge rose this many points above it (no repeats
/// while the reading wobbles around the level).
const REARM_PCT: f64 = 2.0;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Warning,
    Critical,
}

/// What the notice window shows: "{soc}% left" with a yellow (warning) or red (critical) sign.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Notice {
    pub level: Level,
    pub soc: u32,
}

/// Which levels already fired. Changing the levels in Settings arms them again.
#[derive(Default)]
pub struct Alerts {
    levels: Option<(Option<u32>, Option<u32>)>,
    warned: bool,
    critical: bool,
}

impl Alerts {
    /// Feeds one poll; returns a notice when the charge has gone down to a level while
    /// the battery discharges. Critical wins when both levels are reached at once.
    pub fn check(&mut self, s: &Settings, snap: &Snapshot) -> Option<Notice> {
        let levels = (s.warn_on.then_some(s.warn_pct), s.critical_on.then_some(s.critical_pct));
        if self.levels != Some(levels) {
            *self = Alerts { levels: Some(levels), ..Default::default() };
        }
        let soc = snap.soc.filter(|_| snap.online)?;
        let (warn, critical) = levels;
        let rearmed = |level: Option<u32>| level.is_none_or(|l| soc >= (l as f64 + REARM_PCT).min(100.0));
        if rearmed(warn) {
            self.warned = false;
        }
        if rearmed(critical) {
            self.critical = false;
        }
        if snap.flow != Flow::Discharging {
            return None;
        }
        let reached = |level: Option<u32>| level.is_some_and(|l| soc <= l as f64);
        let level = if reached(critical) && !self.critical {
            Level::Critical
        } else if reached(warn) && !self.warned {
            Level::Warning
        } else {
            return None;
        };
        self.critical |= reached(critical);
        self.warned |= reached(warn);
        Some(Notice { level, soc: soc.round() as u32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(soc: f64, flow: Flow) -> Snapshot {
        Snapshot { online: true, soc: Some(soc), flow, ..Default::default() }
    }

    fn notice(level: Level, soc: u32) -> Option<Notice> {
        Some(Notice { level, soc })
    }

    #[test]
    fn warning_then_critical_once_each() {
        let s = Settings::default(); // 50 % and 20 %
        let mut a = Alerts::default();
        assert_eq!(a.check(&s, &snap(50.6, Flow::Discharging)), None);
        assert_eq!(a.check(&s, &snap(49.8, Flow::Discharging)), notice(Level::Warning, 50));
        assert_eq!(a.check(&s, &snap(40.0, Flow::Discharging)), None);
        assert_eq!(a.check(&s, &snap(20.0, Flow::Discharging)), notice(Level::Critical, 20));
        assert_eq!(a.check(&s, &snap(12.0, Flow::Discharging)), None);
    }

    #[test]
    fn no_notice_while_charging_or_idle() {
        let s = Settings::default();
        let mut a = Alerts::default();
        assert_eq!(a.check(&s, &snap(45.0, Flow::Charging)), None);
        assert_eq!(a.check(&s, &snap(45.0, Flow::Idle)), None);
        // It starts to discharge below the level: tell once.
        assert_eq!(a.check(&s, &snap(45.0, Flow::Discharging)), notice(Level::Warning, 45));
    }

    #[test]
    fn critical_wins_and_covers_the_warning() {
        let s = Settings::default();
        let mut a = Alerts::default();
        assert_eq!(a.check(&s, &snap(15.0, Flow::Discharging)), notice(Level::Critical, 15));
        assert_eq!(a.check(&s, &snap(14.0, Flow::Discharging)), None);
    }

    #[test]
    fn fires_again_only_after_a_real_charge() {
        let s = Settings::default();
        let mut a = Alerts::default();
        assert!(a.check(&s, &snap(50.0, Flow::Discharging)).is_some());
        // The reading wobbles around the level: no repeat.
        assert_eq!(a.check(&s, &snap(51.0, Flow::Idle)), None);
        assert_eq!(a.check(&s, &snap(50.0, Flow::Discharging)), None);
        // Charged to 52 % and down again: a new notice.
        assert_eq!(a.check(&s, &snap(52.0, Flow::Charging)), None);
        assert_eq!(a.check(&s, &snap(50.0, Flow::Discharging)), notice(Level::Warning, 50));
    }

    #[test]
    fn switched_off_levels_stay_quiet() {
        let s = Settings { warn_on: false, critical_on: false, ..Default::default() };
        let mut a = Alerts::default();
        assert_eq!(a.check(&s, &snap(10.0, Flow::Discharging)), None);
    }

    #[test]
    fn new_levels_arm_again() {
        let mut s = Settings::default();
        let mut a = Alerts::default();
        assert!(a.check(&s, &snap(45.0, Flow::Discharging)).is_some());
        s.warn_pct = 60;
        assert_eq!(a.check(&s, &snap(45.0, Flow::Discharging)), notice(Level::Warning, 45));
    }

    #[test]
    fn offline_or_unknown_charge_is_quiet() {
        let s = Settings::default();
        let mut a = Alerts::default();
        let mut off = snap(10.0, Flow::Discharging);
        off.online = false;
        assert_eq!(a.check(&s, &off), None);
        let unknown = Snapshot { online: true, flow: Flow::Discharging, ..Default::default() };
        assert_eq!(a.check(&s, &unknown), None);
    }
}

//! Turns raw API replies into what the dashboard shows.

use crate::settings::DeviceRef;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;

/// Below this many watts the battery counts as idle (sensor noise).
const IDLE_W: f64 = 5.0;

/// Polls in a row of discharge before the time estimate shows.
pub const DRAIN_MIN_POLLS: usize = 3;
/// The estimate averages the power of at most this many latest polls.
const DRAIN_MAX_POLLS: usize = 10;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wifi {
    pub ssid: Option<String>,
    pub rssi: Option<i64>,
    pub ip: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    #[default]
    Idle,
    Charging,
    Discharging,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub online: bool,
    pub error: Option<String>,
    pub device: Option<DeviceRef>,
    pub soc: Option<f64>,
    pub energy_wh: Option<f64>,
    pub rated_wh: Option<f64>,
    pub input_w: Option<f64>,
    pub output_w: Option<f64>,
    pub temp_c: Option<f64>,
    pub mode: Option<String>,
    pub flow: Flow,
    /// Minutes until the battery is empty at the present discharge rate (see `Drain`).
    pub remaining_min: Option<u32>,
    pub wifi: Option<Wifi>,
    /// Battery power: + charge, - discharge.
    #[serde(skip)]
    pub bat_w: Option<f64>,
}

/// Reads a number that firmware sends either as a JSON number or as a string.
pub fn num(v: &Value, key: &str) -> Option<f64> {
    match v.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Some firmware reports deci-degrees (e.g. 270 for 27.0 °C).
pub fn normalize_temp(raw: f64) -> f64 {
    let mut t = raw;
    while t.abs() > 80.0 {
        t /= 10.0;
    }
    t
}

/// Firmware reports remaining capacity in Wh, deci-Wh or centi-Wh. Pick the
/// power-of-ten scale that agrees best with SOC x rated capacity.
pub fn normalize_capacity(raw: f64, rated: f64, soc: Option<f64>) -> f64 {
    let expected = soc.map(|s| s / 100.0 * rated);
    [1.0, 0.1, 0.01, 0.001, 10.0]
        .iter()
        .map(|k| raw * k)
        .filter(|c| *c <= rated * 1.05)
        .min_by(|a, b| match expected {
            Some(e) => (a - e).abs().total_cmp(&(b - e).abs()),
            None => b.total_cmp(a),
        })
        .unwrap_or(raw.min(rated))
}

pub fn normalize_mode(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "ups" => "UPS".into(),
        "ai" => "AI".into(),
        "auto" => "Auto".into(),
        "manual" => "Manual".into(),
        "passive" => "Passive".into(),
        _ => raw.to_string(),
    }
}

impl Snapshot {
    /// `bat` is Bat.GetStatus, `es` is ES.GetStatus (either may be Null).
    pub fn apply_battery(&mut self, bat: &Value, es: &Value) {
        let soc = num(bat, "soc").or_else(|| num(es, "bat_soc"));
        let rated = num(bat, "rated_capacity")
            .filter(|r| *r > 0.0)
            .or_else(|| num(es, "bat_cap").filter(|r| *r > 0.0));
        let energy = match (num(bat, "bat_capacity"), rated) {
            (Some(raw), Some(r)) => Some(normalize_capacity(raw, r, soc)),
            (None, Some(r)) => soc.map(|s| s / 100.0 * r),
            _ => None,
        };
        // Capacity gives one decimal of precision that the integer SOC lacks;
        // use it only when both values agree.
        let precise = match (energy, rated) {
            (Some(e), Some(r)) => Some(e / r * 100.0),
            _ => None,
        };
        self.soc = match (precise, soc) {
            (Some(p), Some(s)) if (p - s).abs() <= 2.0 => Some(p),
            (_, Some(s)) => Some(s),
            (p, None) => p,
        };
        self.energy_wh = energy;
        self.rated_wh = rated;
        if let Some(t) = num(bat, "bat_temp") {
            self.temp_c = Some(normalize_temp(t));
        }
    }

    /// `es` is ES.GetStatus; `mode` is the last ES.GetMode reply, used when ES lacks grid power.
    /// ongrid_power < 0 means the battery takes power from the grid.
    pub fn apply_power(&mut self, es: &Value, mode: &Value) {
        let ongrid = num(es, "ongrid_power").or_else(|| num(mode, "ongrid_power"));
        let offgrid = num(es, "offgrid_power").or_else(|| num(mode, "offgrid_power"));
        if ongrid.is_none() && offgrid.is_none() {
            return;
        }
        let on = ongrid.unwrap_or(0.0);
        let input = (-on).max(0.0);
        let output = on.max(0.0) + offgrid.unwrap_or(0.0).max(0.0);
        self.input_w = Some(input);
        self.output_w = Some(output);
        // bat_power (+ charge / - discharge) is the direct reading when present.
        let bat = num(es, "bat_power").filter(|p| p.abs() >= IDLE_W);
        self.bat_w = Some(bat.unwrap_or(input - output));
        self.flow = match bat {
            Some(p) if p > 0.0 => Flow::Charging,
            Some(_) => Flow::Discharging,
            None if input >= IDLE_W => Flow::Charging,
            None if output >= IDLE_W => Flow::Discharging,
            None => Flow::Idle,
        };
    }

    pub fn apply_mode(&mut self, mode: &Value) {
        if let Some(m) = text(mode, "mode") {
            self.mode = Some(normalize_mode(&m));
        }
    }

    pub fn apply_wifi(&mut self, wifi: &Value) {
        self.wifi = Some(Wifi {
            ssid: text(wifi, "ssid"),
            rssi: num(wifi, "rssi").map(|r| r as i64),
            ip: text(wifi, "sta_ip"),
        });
    }

    /// Watts the battery gives out now; `None` unless it discharges.
    fn drain_w(&self) -> Option<f64> {
        let w = -self.bat_w?;
        (self.flow == Flow::Discharging && w >= IDLE_W).then_some(w)
    }

    /// Marstek.GetDevice: refresh the model name and firmware version.
    pub fn apply_device(&mut self, info: &Value) {
        if let Some(dev) = self.device.as_mut() {
            if let Some(name) = text(info, "device") {
                dev.name = Some(name);
            }
            if let Some(ver) = num(info, "ver") {
                dev.ver = Some(ver as u32);
            }
        }
    }
}

/// Time left on battery: remaining energy / mean discharge power of the latest polls.
///
/// minutes = energy_wh / mean(drain watts of the last 3..10 polls) * 60
///
/// It shows from the third poll in a row that discharges; charge, idle or a new device
/// starts the count again. Averaging over a few polls keeps short load spikes from making
/// the estimate jump.
#[derive(Default)]
pub struct Drain {
    watts: VecDeque<f64>,
}

impl Drain {
    /// Feeds one answered poll; returns the minutes left once there are enough polls.
    pub fn update(&mut self, snap: &Snapshot) -> Option<u32> {
        let Some(w) = snap.drain_w() else {
            self.watts.clear();
            return None;
        };
        if self.watts.len() == DRAIN_MAX_POLLS {
            self.watts.pop_front();
        }
        self.watts.push_back(w);
        if self.watts.len() < DRAIN_MIN_POLLS {
            return None;
        }
        let mean = self.watts.iter().sum::<f64>() / self.watts.len() as f64;
        snap.energy_wh.map(|wh| (wh.max(0.0) / mean * 60.0).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capacity_picks_scale_matching_soc() {
        assert_eq!(normalize_capacity(1888.0, 2000.0, Some(94.0)), 1888.0);
        assert!((normalize_capacity(18880.0, 2000.0, Some(94.0)) - 1888.0).abs() < 1e-9);
        assert!((normalize_capacity(188800.0, 2000.0, Some(94.0)) - 1888.0).abs() < 1e-9);
        assert!((normalize_capacity(256.0, 2560.0, Some(90.0)) - 2560.0).abs() < 1e-9);
    }

    #[test]
    fn temp_scales_deci_degrees() {
        assert_eq!(normalize_temp(27.0), 27.0);
        assert_eq!(normalize_temp(270.0), 27.0);
    }

    #[test]
    fn battery_soc_gets_decimal_from_capacity() {
        let mut s = Snapshot::default();
        s.apply_battery(
            &json!({"soc": 94, "bat_capacity": 1888.0, "rated_capacity": 2000.0, "bat_temp": 27}),
            &Value::Null,
        );
        assert!((s.soc.unwrap() - 94.4).abs() < 1e-9);
        assert_eq!(s.temp_c, Some(27.0));
    }

    #[test]
    fn battery_soc_ignores_disagreeing_capacity() {
        let mut s = Snapshot::default();
        s.apply_battery(&json!({"soc": "50", "bat_capacity": 2000, "rated_capacity": 2000}), &Value::Null);
        assert_eq!(s.soc, Some(50.0));
    }

    #[test]
    fn power_direction_follows_ongrid_sign() {
        let mut s = Snapshot::default();
        s.apply_power(&json!({"ongrid_power": 129, "offgrid_power": 0}), &Value::Null);
        assert_eq!((s.input_w, s.output_w, s.flow), (Some(0.0), Some(129.0), Flow::Discharging));
        s.apply_power(&json!({"ongrid_power": -800}), &Value::Null);
        assert_eq!((s.input_w, s.output_w, s.flow), (Some(800.0), Some(0.0), Flow::Charging));
        s.apply_power(&json!({"ongrid_power": 2, "bat_power": 0}), &Value::Null);
        assert_eq!(s.flow, Flow::Idle);
    }

    #[test]
    fn power_falls_back_to_mode_reply() {
        let mut s = Snapshot::default();
        s.apply_power(&Value::Null, &json!({"ongrid_power": 50}));
        assert_eq!(s.output_w, Some(50.0));
    }

    fn discharging(wh: f64, watts: f64) -> Snapshot {
        let mut s = Snapshot { energy_wh: Some(wh), ..Default::default() };
        s.apply_power(&json!({"ongrid_power": watts, "offgrid_power": 0}), &Value::Null);
        s
    }

    #[test]
    fn drain_estimate_starts_on_the_third_poll() {
        let mut d = Drain::default();
        assert_eq!(d.update(&discharging(1888.0, 129.0)), None);
        assert_eq!(d.update(&discharging(1888.0, 129.0)), None);
        // 1888 Wh / 129 W = 14.64 h = 878 min.
        assert_eq!(d.update(&discharging(1888.0, 129.0)), Some(878));
    }

    #[test]
    fn drain_estimate_averages_the_power() {
        let mut d = Drain::default();
        d.update(&discharging(1000.0, 100.0));
        d.update(&discharging(1000.0, 200.0));
        // Mean 200 W: 1000 Wh lasts 5 h.
        assert_eq!(d.update(&discharging(1000.0, 300.0)), Some(300));
        // Only the latest 10 polls count: after ten polls at 500 W the mean is 500 W.
        for _ in 0..9 {
            d.update(&discharging(1000.0, 500.0));
        }
        assert_eq!(d.update(&discharging(1000.0, 500.0)), Some(120));
    }

    #[test]
    fn drain_estimate_restarts_after_charge_or_idle() {
        let mut d = Drain::default();
        for _ in 0..3 {
            d.update(&discharging(1000.0, 100.0));
        }
        let mut charging = Snapshot { energy_wh: Some(1000.0), ..Default::default() };
        charging.apply_power(&json!({"ongrid_power": -800}), &Value::Null);
        assert_eq!(d.update(&charging), None);
        assert_eq!(d.update(&discharging(1000.0, 100.0)), None);
        assert_eq!(d.update(&discharging(1000.0, 100.0)), None);
        assert_eq!(d.update(&discharging(1000.0, 100.0)), Some(600));
    }

    #[test]
    fn drain_uses_battery_power_when_reported() {
        let mut d = Drain::default();
        let snap = || {
            let mut s = Snapshot { energy_wh: Some(1000.0), ..Default::default() };
            // 129 W at the socket, 150 W out of the battery (inverter losses).
            s.apply_power(&json!({"ongrid_power": 129, "bat_power": -150}), &Value::Null);
            s
        };
        d.update(&snap());
        d.update(&snap());
        assert_eq!(d.update(&snap()), Some(400));
    }

    #[test]
    fn mode_names_are_normalized() {
        assert_eq!(normalize_mode("Ups"), "UPS");
        assert_eq!(normalize_mode("ai"), "AI");
    }
}

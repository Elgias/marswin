//! Turns raw API replies into what the dashboard shows.

use crate::settings::DeviceRef;
use serde::Serialize;
use serde_json::Value;

/// Below this many watts the battery counts as idle (sensor noise).
const IDLE_W: f64 = 5.0;

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
    pub wifi: Option<Wifi>,
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
        self.flow = match num(es, "bat_power").filter(|p| p.abs() >= IDLE_W) {
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

    #[test]
    fn mode_names_are_normalized() {
        assert_eq!(normalize_mode("Ups"), "UPS");
        assert_eq!(normalize_mode("ai"), "AI");
    }
}

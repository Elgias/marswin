# MarsWin

A small Windows app that monitors a Marstek battery (Venus C/D/E, Venus E mini) over the
[Marstek Device Open API](https://static-eu.marstekcloud.com/ems/resource/agreement/MarstekDeviceOpenApi.pdf)
(JSON-RPC over UDP, local network only, no cloud).

## Features

- Charge level, energy (kWh), input/output power, temperature, operating mode.
- Device search on the local network (UDP broadcast), or manual IP and port.
- Charging at a set power (100–800 W). The ✓ button switches the device to **Passive** mode
  with `power = -W` (negative means charge) for 24 h (`cd_time = 86400`, the firmware maximum).
- Tray icon with charge level in the tooltip. × hides the window to the tray; quit from the tray menu.
- Starts with Windows (hidden in the tray). Toggle it in the tray menu.
- Ukrainian and English.

## Before first use

1. In the Marstek mobile app, enable **Open API** and note the UDP port (default 30000).
2. Put the PC and the battery on the same network.
3. On first launch, Windows Firewall asks about network access. Allow **private networks**,
   or the device search gets no replies.

## Development

Requirements: Node 20+, Rust (stable), WebView2 (included in Windows 11).

```bash
npm install
npm run tauri dev     # app with a real backend
npm run dev           # UI only, in a browser, with mock data (http://localhost:1420)
npm run tauri build   # installer: src-tauri/target/release/bundle/nsis/
cd src-tauri && cargo test
```

## Layout

| Path | Purpose |
|---|---|
| `src-tauri/src/marstek.rs` | UDP client: one request at a time, discovery over every adapter |
| `src-tauri/src/snapshot.rs` | Turns API replies into dashboard values (unit scaling per firmware) |
| `src-tauri/src/settings.rs` | Settings file (`%APPDATA%\com.marswin.app\settings.json`) |
| `src-tauri/src/lib.rs` | Polling loop, commands, windows, tray, autostart |
| `src/main.ts`, `src/settings.ts` | Main window and settings window |

## Data mapping

| UI | Source |
|---|---|
| % and kWh | `Bat.GetStatus` `soc`, `bat_capacity`, `rated_capacity` |
| Input / Output | `ES.GetStatus` `ongrid_power` (< 0 means from the grid) plus `offgrid_power` |
| Charging / Discharging | `bat_power` sign when present, otherwise `ongrid_power` |
| Temperature | `Bat.GetStatus` `bat_temp` |
| Mode | `ES.GetMode` (read every "Mode check" interval) |
| Connection | `Wifi.GetStatus`, `Marstek.GetDevice` |

## Pitfalls (found while building)

- **Window from a command:** creating a window inside a *sync* Tauri command deadlocks on
  Windows. `open_settings` must stay `async`.
- **Own packets look like replies:** the client binds the device port (some firmware answers
  only to it). On the same host, the request comes back to us. `marstek::call` skips any
  datagram that has `method` or lacks both `result` and `error`.
- **Screen fit:** window size comes from `fit.rs` (0.72 of the design, at most 55 % of the
  work-area height). The page follows with CSS `zoom` (`fitZoom` in `api.ts`). Change the
  numbers there, not the CSS.
- **Testing without a battery:** `npm run dev` gives mock data. For the full path, run a small
  UDP emulator on another port (e.g. 30002) and set the device to `127.0.0.1:30002`.
- **Screenshots of the real window:** a PowerShell capture must be per-monitor DPI aware, or
  it gets scaled coordinates and crops the window.

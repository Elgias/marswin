# MarsWin

A small Windows app that monitors a Marstek battery (Venus C/D/E, Venus E mini) over the
[Marstek Device Open API](https://static-eu.marstekcloud.com/ems/resource/agreement/MarstekDeviceOpenApi.pdf)
(JSON-RPC over UDP, local network only, no cloud).

## Features

- Charge level, energy (kWh), input/output power, temperature, operating mode.
- Device search on the local network (UDP broadcast), or manual IP and port.
- Read-only: MarsWin shows data and never changes the device's mode.
- Tray icon with charge level in the tooltip. **Left click** shows a small widget (charge, bar,
  in/out power, status) next to the taskbar: above a bottom taskbar, below a top one, beside a
  side one, centred on the tray icon. ⤢ opens the main window, × hides it.
  **Right click**: Open window, Settings, Start with Windows (check), Close (quit).
- `marswin.exe --widget` toggles the widget by the cursor (for a shortcut or hotkey).
- × on the main window hides it to the tray.
- Starts with Windows (hidden in the tray); switched on at first run. Toggle it in Settings or
  in the tray menu; both stay in step.
- Exchange log of raw requests and replies: `%APPDATA%\com.marswin.app\marswin.log`.
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
| `src-tauri/src/fit.rs` | Window sizes per screen (1080p, 1440p, 4K, any scaling) |
| `src/main.ts`, `src/settings.ts`, `src/widget.ts` | Main window, settings window, tray widget |
| `src/view.ts` | Formatting and status shared by the main window and the widget |

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
- **Undecorated window size:** on Windows a window built without decorations comes out one
  hidden caption (~29 logical px) taller than asked, and `set_size` to the size it "already has"
  does nothing. `set_client_size` in `lib.rs` steps through another size first. Check with
  `GetClientRect`, not by eye: a flex layout hides the extra height.
- **Widget position:** the taskbar can be on any edge (on the dev machine it is at the top).
  `fit::taskbar_edge` finds it from where the work area is cut from the monitor; an auto-hidden
  taskbar cuts nothing, so the edge nearest the click is used. Tests in `fit.rs` cover all edges.
- **Screen fit:** window size comes from `fit.rs` (0.72 of the design, at most 55 % of the
  work-area height). The page follows with CSS `zoom` (`fitZoom` in `api.ts`). Change the
  numbers there, not the CSS.
- **Testing without a battery:** `npm run dev` gives mock data. For the full path, run a small
  UDP emulator on another port (e.g. 30002) and set the device to `127.0.0.1:30002`.
- **Screenshots of the real window:** a PowerShell capture must be per-monitor DPI aware, or
  it gets scaled coordinates and crops the window.
- **Key order in requests:** `serde_json` sorts object keys unless the `preserve_order` feature
  is on. Polls (`{"id":0}`) work either way, but multi-key params (e.g. `ES.SetMode`) must go
  out in the documented order. A test in `marstek.rs` pins it.
- **If mode control comes back:** `ES.SetMode` is slow and lossy; the Home Assistant
  integration waits 15 s and retries 3 times. (MarsWin 1.1.0 had this; 1.2.0 removed charging.)

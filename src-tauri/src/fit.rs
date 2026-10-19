//! Window size for any screen (1080p, 1440p, 4K at any Windows scaling).
//!
//! The design is drawn large (565 x 713 for the main window). It is shown at
//! BASE_SCALE of that size, and smaller still when the screen's work area is
//! short. The page scales its content to the window width with CSS zoom.

/// Design units to logical pixels. 0.72 turns the design's 19 px body text into ~14 px.
pub const BASE_SCALE: f64 = 0.72;

pub struct Window {
    /// Design size in design units.
    pub design: (f64, f64),
    /// Largest share of the work area the window may take.
    pub max_share: (f64, f64),
}

pub const MAIN: Window = Window { design: (565.0, 713.0), max_share: (0.5, 0.55) };
pub const SETTINGS: Window = Window { design: (1128.0, 873.0), max_share: (0.9, 0.8) };
pub const WIDGET: Window = Window { design: (545.0, 211.0), max_share: (0.4, 0.25) };

/// Logical window size for a work area given in logical pixels.
pub fn size(win: &Window, area: (f64, f64)) -> (f64, f64) {
    let (w, h) = win.design;
    let k = BASE_SCALE
        .min(area.0 * win.max_share.0 / w)
        .min(area.1 * win.max_share.1 / h);
    ((w * k).round(), (h * k).round())
}

/// A rectangle in physical px: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// The taskbar's edge: the side where the work area is cut from the monitor. An auto-hidden
/// taskbar cuts nothing; then the monitor edge nearest to the click (the tray) wins.
pub fn taskbar_edge(monitor: Rect, work: Rect, click: (i32, i32)) -> Edge {
    let (mx, my, mw, mh) = monitor;
    let (wx, wy, ww, wh) = work;
    if wy > my {
        return Edge::Top;
    }
    if wy + wh < my + mh {
        return Edge::Bottom;
    }
    if wx > mx {
        return Edge::Left;
    }
    if wx + ww < mx + mw {
        return Edge::Right;
    }
    let gaps = [
        (click.1 - my, Edge::Top),
        (my + mh - click.1, Edge::Bottom),
        (click.0 - mx, Edge::Left),
        (mx + mw - click.0, Edge::Right),
    ];
    gaps.into_iter().min_by_key(|g| g.0).map_or(Edge::Bottom, |g| g.1)
}

/// Top-left corner for the widget: next to the taskbar (above a bottom one, below a top one),
/// centred on the tray click along the taskbar and kept inside the work area.
pub fn widget_position(edge: Edge, work: Rect, size: (i32, i32), click: (i32, i32), margin: i32) -> (i32, i32) {
    let (wx, wy, ww, wh) = work;
    let (w, h) = size;
    let along_x = (click.0 - w / 2).clamp(wx + margin, (wx + ww - w - margin).max(wx + margin));
    let along_y = (click.1 - h / 2).clamp(wy + margin, (wy + wh - h - margin).max(wy + margin));
    match edge {
        Edge::Bottom => (along_x, wy + wh - h - margin),
        Edge::Top => (along_x, wy + margin),
        Edge::Left => (wx + margin, along_y),
        Edge::Right => (wx + ww - w - margin, along_y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: Rect = (0, 0, 3840, 2160);
    const SIZE: (i32, i32) = (588, 228);

    #[test]
    fn widget_sits_above_a_bottom_taskbar() {
        let work = (0, 0, 3840, 2088);
        let edge = taskbar_edge(MON, work, (3600, 2120));
        assert_eq!(edge, Edge::Bottom);
        assert_eq!(widget_position(edge, work, SIZE, (3600, 2120), 18), (3234, 2088 - 228 - 18));
    }

    #[test]
    fn widget_sits_below_a_top_taskbar() {
        let work = (0, 72, 3840, 2088);
        let edge = taskbar_edge(MON, work, (3339, 36));
        assert_eq!(edge, Edge::Top);
        // Centred under the icon: 3339 - 294 = 3045.
        assert_eq!(widget_position(edge, work, SIZE, (3339, 36), 18), (3045, 72 + 18));
    }

    #[test]
    fn widget_sits_beside_side_taskbars() {
        let left = (96, 0, 3744, 2160);
        assert_eq!(taskbar_edge(MON, left, (40, 2000)), Edge::Left);
        // Centred on the click along the taskbar: 2000 - 228 / 2.
        assert_eq!(widget_position(Edge::Left, left, SIZE, (40, 2000), 18), (96 + 18, 1886));
        let right = (0, 0, 3744, 2160);
        assert_eq!(taskbar_edge(MON, right, (3800, 2000)), Edge::Right);
        assert_eq!(widget_position(Edge::Right, right, SIZE, (3800, 2000), 18).0, 3744 - 588 - 18);
    }

    #[test]
    fn auto_hidden_taskbar_uses_the_nearest_edge_to_the_click() {
        assert_eq!(taskbar_edge(MON, MON, (3700, 2150)), Edge::Bottom);
        assert_eq!(taskbar_edge(MON, MON, (3700, 5)), Edge::Top);
    }

    #[test]
    fn widget_on_a_secondary_monitor_stays_on_it() {
        // Monitor to the left of the primary, at negative x.
        let mon = (-1920, 0, 1920, 1080);
        let work = (-1920, 0, 1920, 1032);
        let edge = taskbar_edge(mon, work, (-100, 1060));
        let (x, y) = widget_position(edge, work, (392, 152), (-100, 1060), 12);
        assert!(x >= -1920 && x + 392 <= 0 && y + 152 <= 1032, "{x},{y}");
    }

    /// Work area in logical px: physical resolution / scaling, minus a 48 px taskbar.
    fn area(w: f64, h: f64, scale: f64) -> (f64, f64) {
        (w / scale, h / scale - 48.0)
    }

    #[test]
    fn main_window_never_dominates_the_screen() {
        for (w, h) in [(1920.0, 1080.0), (2560.0, 1440.0), (3840.0, 2160.0)] {
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                let a = area(w, h, scale);
                let (_, mh) = size(&MAIN, a);
                assert!(mh <= a.1 * 0.55 + 1.0, "{w}x{h}@{scale}: {mh} of {}", a.1);
                let (sw, sh) = size(&SETTINGS, a);
                assert!(sw <= a.0 * 0.9 + 1.0 && sh <= a.1 * 0.8 + 1.0);
            }
        }
    }

    #[test]
    fn roomy_screens_get_the_base_scale() {
        // 1440p at 100 % and 4K at 150 % both have a 2560 x 1392 logical work area.
        assert_eq!(size(&MAIN, area(2560.0, 1440.0, 1.0)), (407.0, 513.0));
        assert_eq!(size(&MAIN, area(3840.0, 2160.0, 1.5)), (407.0, 513.0));
    }

    #[test]
    fn short_screens_shrink_further() {
        // 1080p at 125 %: 830 px of work area height.
        let (w, h) = size(&MAIN, area(1920.0, 1080.0, 1.25));
        assert!(h < 513.0 && (w / h - 565.0 / 713.0).abs() < 0.01, "{w}x{h}");
    }
}

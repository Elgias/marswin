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

#[cfg(test)]
mod tests {
    use super::*;

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

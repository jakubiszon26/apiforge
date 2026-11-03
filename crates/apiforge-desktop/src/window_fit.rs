//! The window's size against the screen it opens on (UX-09, KTLO-38).
//!
//! The first window is the smaller of its usual size and the monitor's work
//! area less the title bar, never below the minimum size; a remembered size is
//! clamped the same way, so the bottom bar is on screen when the window opens.

use eframe::egui::Vec2;

/// The smallest window size: [`crate::MIN_WINDOW_SIZE`].
fn floor() -> Vec2 {
    Vec2::from(crate::MIN_WINDOW_SIZE)
}

/// The inner size of the first window on a monitor whose work area is
/// `work_area`, under a title bar `title_bar` points high.
pub fn first_window_size(work_area: Vec2, title_bar: f32) -> Vec2 {
    fit_window_size(Vec2::from(crate::DEFAULT_WINDOW_SIZE), work_area, title_bar)
}

/// A remembered inner size `size`, clamped so that the window with its title
/// bar fits the work area, never below the minimum size.
pub fn fit_window_size(size: Vec2, work_area: Vec2, title_bar: f32) -> Vec2 {
    // KTLO-38 acceptance stub: the implementation follows in the next commit.
    let _ = (work_area, title_bar, floor());
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    /// Criterion 1: a 1600 × 1000 work area under a 36 px title bar gives a
    /// 1600 × 964 window, whose frame is exactly the screen's height.
    #[test]
    fn the_first_window_fits_a_1600_by_1000_work_area_under_its_title_bar() {
        assert_eq!(
            first_window_size(vec2(1600.0, 1000.0), 36.0),
            vec2(1600.0, 964.0)
        );
    }

    /// Criterion 1: a larger work area keeps the usual 1600 × 1000.
    #[test]
    fn the_first_window_keeps_1600_by_1000_on_a_larger_screen() {
        assert_eq!(
            first_window_size(vec2(2560.0, 1400.0), 36.0),
            vec2(1600.0, 1000.0)
        );
    }

    /// Criterion 1 on a 1024 × 700 work area. The ticket's case reads
    /// "→ 960 × 540 floor noted", but its own rule — the smaller of
    /// 1600 × 1000 and the work area less the title bar, never below
    /// 960 × 540 — gives 1024 × 664, above the floor; the floor is the next test.
    #[test]
    fn the_first_window_fits_a_1024_by_700_work_area_above_the_floor() {
        assert_eq!(
            first_window_size(vec2(1024.0, 700.0), 36.0),
            vec2(1024.0, 664.0)
        );
    }

    /// Criterion 1: never below 960 × 540, even on a work area smaller than that.
    #[test]
    fn the_first_window_is_never_below_960_by_540() {
        assert_eq!(
            first_window_size(vec2(800.0, 560.0), 36.0),
            vec2(960.0, 540.0)
        );
    }

    /// Criterion 2: a remembered size larger than the work area is clamped
    /// the same way on restore, one that fits is kept, one below the minimum
    /// grows to it.
    #[test]
    fn a_remembered_size_is_clamped_the_same_way_on_restore() {
        let work_area = vec2(1600.0, 1000.0);
        assert_eq!(
            fit_window_size(vec2(1920.0, 1150.0), work_area, 36.0),
            vec2(1600.0, 964.0)
        );
        assert_eq!(
            fit_window_size(vec2(1600.0, 1000.0), work_area, 36.0),
            vec2(1600.0, 964.0)
        );
        assert_eq!(
            fit_window_size(vec2(1200.0, 800.0), work_area, 36.0),
            vec2(1200.0, 800.0)
        );
        assert_eq!(
            fit_window_size(vec2(700.0, 400.0), work_area, 36.0),
            vec2(960.0, 540.0)
        );
    }
}

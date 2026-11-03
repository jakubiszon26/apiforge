//! The window's size against the screen it opens on (UX-09, KTLO-38).
//!
//! The first window is the smaller of its usual size and the monitor's work
//! area less the title bar, never below the minimum size; a remembered size is
//! clamped the same way, so the bottom bar is on screen when the window opens.
//!
//! winit reports a monitor only to the event loop that eframe owns, and has no
//! work area at all, so the window opens at its usual size and is fitted on
//! its first frame from what egui reports: the monitor's size as the work
//! area, and the title bar as the difference between the outer and the inner
//! height. Where the frame cannot be measured — Wayland's server-side
//! decorations, a display without a window manager — a 36 px title bar is
//! assumed.

use eframe::egui::{self, Rect, Vec2};

/// The title bar assumed where the window's frame cannot be measured: the
/// height KWin draws at 100 % scaling, where KTLO-38 was found.
pub const ASSUMED_TITLE_BAR: f32 = 36.0;

/// Frames to wait for the monitor and the window's rectangles before giving
/// up and leaving the window at the size it opened with.
const ATTEMPTS: u8 = 30;

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
    let room = work_area - Vec2::new(0.0, title_bar.max(0.0));
    size.min(room).max(floor())
}

/// The title bar's height: what the frame adds above and below the inner
/// rectangle, or [`ASSUMED_TITLE_BAR`] where the frame adds nothing.
pub fn title_bar_height(inner: Rect, outer: Option<Rect>) -> f32 {
    match outer.map(|outer| outer.height() - inner.height()) {
        Some(frame) if frame >= 1.0 => frame,
        _ => ASSUMED_TITLE_BAR,
    }
}

/// The monitor's size and the window's title bar, or `None` while egui does
/// not know the monitor yet.
pub fn screen(context: &egui::Context) -> Option<(Vec2, f32)> {
    let (monitor, inner, outer) = context.input(|input| {
        let viewport = input.viewport();
        (
            viewport.monitor_size,
            viewport.inner_rect,
            viewport.outer_rect,
        )
    });
    let monitor = monitor.filter(|monitor| monitor.x >= 1.0 && monitor.y >= 1.0)?;
    let title_bar = inner.map_or(ASSUMED_TITLE_BAR, |inner| title_bar_height(inner, outer));
    Some((monitor, title_bar))
}

/// Fits the window that has just opened to its monitor, once.
#[derive(Debug, Default)]
pub struct FirstFit {
    attempts: u8,
    done: bool,
    /// The size the window was asked to take, or `None` for the size it has.
    size: Option<Vec2>,
}

impl FirstFit {
    /// Fits a window that was just asked to take `size`, before the window
    /// reports it: the stop window growing into the workspace window.
    pub fn toward(size: Vec2) -> Self {
        Self {
            size: Some(size),
            ..Self::default()
        }
    }

    /// Resizes the window if its size, opened or restored by the window
    /// manager, does not fit the monitor; waits a few frames for the monitor
    /// to be known.
    pub fn update(&mut self, context: &egui::Context) {
        if self.done {
            return;
        }
        self.attempts += 1;
        let inner = context.input(|input| input.viewport().inner_rect);
        let size = self.size.or(inner.map(|inner| inner.size()));
        let fitted = size
            .zip(screen(context))
            .map(|(size, (monitor, title_bar))| (size, fit_window_size(size, monitor, title_bar)));
        match fitted {
            Some((size, fitted)) => {
                self.done = true;
                if (fitted - size).abs().max_elem() >= 1.0 {
                    context.send_viewport_cmd(egui::ViewportCommand::InnerSize(fitted));
                }
            }
            None if self.attempts >= ATTEMPTS => self.done = true,
            None => context.request_repaint(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    /// Criterion 1 with the owner's panel allowance (2026-11-03): a
    /// 1600 × 1000 monitor under a 36 px title bar and a 48 px panel gives a
    /// 1600 × 916 window, whose frame ends above a bottom panel.
    #[test]
    fn the_first_window_fits_a_1600_by_1000_work_area_under_its_title_bar() {
        assert_eq!(
            first_window_size(vec2(1600.0, 1000.0), 36.0),
            vec2(1600.0, 916.0)
        );
    }

    /// Criterion 1: a larger monitor keeps the usual 1600 × 1000.
    #[test]
    fn the_first_window_keeps_1600_by_1000_on_a_larger_screen() {
        for monitor in [vec2(2560.0, 1400.0), vec2(2560.0, 1440.0)] {
            assert_eq!(first_window_size(monitor, 36.0), vec2(1600.0, 1000.0));
        }
    }

    /// Criterion 1 on a 1024 × 700 monitor. The ticket's case reads
    /// "→ 960 × 540 floor noted", but its own rule — the smaller of
    /// 1600 × 1000 and the monitor less the title bar and the panel, never
    /// below 960 × 540 — gives 1024 × 616, above the floor; the floor is the
    /// next test.
    #[test]
    fn the_first_window_fits_a_1024_by_700_work_area_above_the_floor() {
        assert_eq!(
            first_window_size(vec2(1024.0, 700.0), 36.0),
            vec2(1024.0, 616.0)
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
            vec2(1600.0, 916.0)
        );
        assert_eq!(
            fit_window_size(vec2(1600.0, 1000.0), work_area, 36.0),
            vec2(1600.0, 916.0)
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

    /// The title bar is measured where the frame is known, and assumed where
    /// the window manager reports no frame.
    #[test]
    fn the_title_bar_is_measured_or_assumed() {
        let inner = Rect::from_min_size(pos2(0.0, 31.0), vec2(1600.0, 964.0));
        let outer = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 995.0));
        assert_eq!(title_bar_height(inner, Some(outer)), 31.0);
        assert_eq!(title_bar_height(inner, Some(inner)), ASSUMED_TITLE_BAR);
        assert_eq!(title_bar_height(inner, None), ASSUMED_TITLE_BAR);
    }

    /// A window that already fits gets no command; one that does not is
    /// resized once the monitor is known.
    #[test]
    fn the_first_frame_resizes_only_a_window_that_does_not_fit() {
        let commands = |inner: Vec2, monitor: Option<Vec2>| {
            let context = egui::Context::default();
            let mut input = egui::RawInput::default();
            let viewport = input.viewports.entry(egui::ViewportId::ROOT).or_default();
            viewport.inner_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), inner));
            viewport.monitor_size = monitor;
            let mut fit = FirstFit::default();
            let mut output = context.run_ui(input, |ui| fit.update(ui.ctx()));
            output.textures_delta.clear();
            // egui adds a theme command of its own on a first pass.
            let resizes = output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .filter(|command| matches!(command, egui::ViewportCommand::InnerSize(_)))
                .cloned()
                .collect::<Vec<_>>();
            (fit.done, resizes)
        };
        let (done, sent) = commands(vec2(1600.0, 1000.0), Some(vec2(1600.0, 1000.0)));
        assert!(done);
        assert_eq!(
            sent,
            [egui::ViewportCommand::InnerSize(vec2(1600.0, 916.0))]
        );
        let (done, sent) = commands(vec2(1600.0, 1000.0), Some(vec2(2560.0, 1400.0)));
        assert!(done && sent.is_empty());
        let (done, sent) = commands(vec2(1600.0, 1000.0), None);
        assert!(!done && sent.is_empty(), "it waits for the monitor");
    }

    /// The stop window growing into the workspace window fits the size it
    /// asked for, not the dialog's size the window still reports, once the
    /// monitor is known.
    #[test]
    fn a_window_asked_to_grow_fits_the_size_it_asked_for() {
        let context = egui::Context::default();
        let mut fit = FirstFit::toward(vec2(1600.0, 1000.0));
        let mut frame = |monitor: Option<Vec2>| {
            let mut input = egui::RawInput::default();
            let viewport = input.viewports.entry(egui::ViewportId::ROOT).or_default();
            viewport.inner_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 420.0)));
            viewport.monitor_size = monitor;
            let mut output = context.run_ui(input, |ui| fit.update(ui.ctx()));
            output.textures_delta.clear();
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .filter(|command| matches!(command, egui::ViewportCommand::InnerSize(_)))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert!(frame(None).is_empty(), "it waits for the monitor");
        assert_eq!(
            frame(Some(vec2(1600.0, 1000.0))),
            [egui::ViewportCommand::InnerSize(vec2(1600.0, 916.0))]
        );
        assert!(frame(Some(vec2(1600.0, 1000.0))).is_empty(), "once");
    }
}

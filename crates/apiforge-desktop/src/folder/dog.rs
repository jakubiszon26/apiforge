//! The sleeping dog an empty board rests under (KTLO-20): the owner's drawing
//! painted from the paths of its 500 × 340 view box.
//!
//! The drawing animates on a four-second loop — the chest, the ground shadow,
//! the sleep bubble, the ears, the tail and the floating letters. Its rest
//! frame is the still frame the drawing itself shows under reduced motion, and
//! the frame the mock-up approved: every value of [`Motion`] is zero there, so
//! a window that does not animate shows exactly that drawing.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};

/// The view box's 50:34.
const ASPECT: f32 = 0.68;
/// How long the drawing takes to come back to its rest frame.
const LOOP_SECONDS: f64 = 4.0;

/// The drawing's box in points, in the view box's proportions: 220 wide at the
/// minimum window of 540 points high and 280 at the default one of 1000,
/// growing with the window's height between them.
pub(super) fn size_for(window_height: f32) -> egui::Vec2 {
    const MINIMUM: (f32, f32) = (540.0, 220.0);
    const DEFAULT: (f32, f32) = (1000.0, 280.0);
    let part = ((window_height - MINIMUM.0) / (DEFAULT.0 - MINIMUM.0)).clamp(0.0, 1.0);
    let width = MINIMUM.1 + part * (DEFAULT.1 - MINIMUM.1);
    vec2(width, width * ASPECT)
}

/// Where the drawing is in its four-second loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Motion {
    /// The chest's rise and fall: -1 breathed out, 1 breathed in.
    breath: f32,
    /// The ears' and tail's sway, twice per loop.
    sway: f32,
    /// How far the letters have floated up, 0 to 1. They only ever rise from
    /// the rest frame and sink back to it.
    drift: f32,
}

impl Motion {
    /// The rest frame: the drawing the mock-up approved, which a window that
    /// does not animate and the test harness show.
    pub(super) const REST: Self = Self {
        breath: 0.0,
        sway: 0.0,
        drift: 0.0,
    };

    /// The frame `seconds` into the window's clock.
    pub(super) fn at(seconds: f64) -> Self {
        let turn = (seconds / LOOP_SECONDS).rem_euclid(1.0) as f32 * std::f32::consts::TAU;
        Self {
            breath: turn.sin(),
            sway: (2.0 * turn).sin(),
            drift: (0.5 * turn).sin(),
        }
    }
}

/// Asks for the loop's next frame. The loop asks from here and nowhere else,
/// so its request is told from the window's others — the watcher's check
/// once a second, storage and analysis workers — by where it was made
/// ([`egui::Context::repaint_causes`]), whenever those arrive.
pub(super) fn next_frame(ctx: &egui::Context) {
    ctx.request_repaint();
}

const OUTLINE: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
const BODY: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);
const GREY: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
const LIGHT: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC);
const BLUSH: Color32 = Color32::from_rgb(0xFD, 0xA4, 0xAF);
const BUBBLE: Color32 = Color32::from_rgba_premultiplied(168, 181, 190, 191);
const SHADOW: Color32 = Color32::from_rgba_premultiplied(52, 57, 64, 89);

/// One cubic segment of the drawing: its start, two controls and its end, in
/// the view box's whole-number coordinates. A path's segments run on from one
/// another, as the drawing's do.
type Curve = [i16; 8];

const TAIL: [Curve; 2] = [
    [365, 225, 405, 205, 415, 255, 375, 262],
    [375, 262, 355, 265, 355, 240, 372, 238],
];
const LOAF: [Curve; 4] = [
    [105, 240, 95, 135, 175, 110, 255, 115],
    [255, 115, 335, 120, 385, 160, 375, 245],
    [375, 245, 370, 278, 330, 278, 240, 278],
    [240, 278, 150, 278, 110, 278, 105, 240],
];
const PATCHES: [Curve; 2] = [
    [245, 116, 245, 148, 285, 152, 295, 120],
    [310, 126, 308, 148, 335, 152, 342, 138],
];
const BACK: [Curve; 1] = [[230, 115, 275, 115, 325, 125, 355, 148]];
const EARS: [[Curve; 2]; 2] = [
    [
        [135, 145, 100, 140, 80, 180, 100, 205],
        [100, 205, 115, 220, 138, 195, 140, 165],
    ],
    [
        [225, 135, 255, 130, 275, 165, 255, 195],
        [255, 195, 240, 210, 220, 185, 218, 155],
    ],
];
const EYES: [Curve; 2] = [
    [135, 192, 140, 204, 155, 204, 160, 192],
    [196, 192, 201, 204, 216, 204, 221, 192],
];
const MOUTH: [Curve; 2] = [
    [164, 220, 170, 227, 178, 223, 178, 214],
    [178, 214, 178, 223, 186, 227, 192, 220],
];
/// The paws under the belly, which stay on the ground.
const PAWS: [(f32, f32); 3] = [(140.0, 276.0), (215.0, 276.0), (325.0, 274.0)];
/// z z Z: where each letter sits, how large, how thick its outline, and how
/// far it floats up over the loop.
const LETTERS: [(f32, f32, f32, f32, f32, &str); 3] = [
    (280.0, 100.0, 26.0, 8.0, 6.0, "z"),
    (320.0, 78.0, 38.0, 10.0, 9.0, "z"),
    (368.0, 58.0, 52.0, 12.0, 12.0, "Z"),
];

struct Canvas<'a> {
    painter: &'a egui::Painter,
    scale: f32,
    offset: egui::Vec2,
    /// Where the group being drawn has moved in the loop, in the view box's
    /// units. Zero at the rest frame.
    shift: egui::Vec2,
}

impl Canvas<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        (pos2(x, y) + self.shift) * self.scale + self.offset
    }

    fn stroke(&self, width: f32) -> Stroke {
        Stroke::new(width * self.scale, OUTLINE)
    }

    /// The points along a path, 24 to a segment, both ends included.
    fn path(&self, curves: &[Curve]) -> Vec<Pos2> {
        const STEPS: usize = 24;
        let mut points = Vec::with_capacity(curves.len() * STEPS + 1);
        for curve in curves {
            let [x0, y0, x1, y1, x2, y2, x3, y3] = curve.map(f32::from);
            if points.is_empty() {
                points.push(self.at(x0, y0));
            }
            for step in 1..=STEPS {
                let t = step as f32 / STEPS as f32;
                let u = 1.0 - t;
                let x =
                    u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
                let y =
                    u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
                points.push(self.at(x, y));
            }
        }
        points
    }

    fn ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<Pos2> {
        (0..48)
            .map(|index| {
                let angle = index as f32 / 48.0 * std::f32::consts::TAU;
                self.at(cx + rx * angle.cos(), cy + ry * angle.sin())
            })
            .collect()
    }

    fn fill(&self, points: Vec<Pos2>, fill: Color32) {
        self.painter
            .add(Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    fn fill_ellipse(
        &self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        fill: Color32,
        outline: Option<f32>,
    ) {
        let points = self.ellipse(cx, cy, rx, ry);
        let stroke = outline.map_or(Stroke::NONE, |width| self.stroke(width));
        self.painter
            .add(Shape::convex_polygon(points, fill, stroke));
    }

    fn line(&self, points: Vec<Pos2>, width: f32) {
        self.painter.add(Shape::line(points, self.stroke(width)));
    }

    /// A letter with a light face and a dark outline, like the drawing's
    /// `paint-order: stroke fill` text, fading as it floats away.
    fn letter(&self, x: f32, y: f32, size: f32, outline: f32, fade: f32, text: &str) {
        let font = FontId::new(size * self.scale, crate::theme::bold_family());
        let anchor = egui::Align2::LEFT_BOTTOM;
        let position = self.at(x, y + size * 0.22);
        let offset = outline * self.scale * 0.3;
        for (dx, dy) in [
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-0.7, -0.7),
            (0.7, -0.7),
            (-0.7, 0.7),
            (0.7, 0.7),
        ] {
            self.painter.text(
                position + vec2(dx, dy) * offset,
                anchor,
                text,
                font.clone(),
                OUTLINE.gamma_multiply(fade),
            );
        }
        self.painter
            .text(position, anchor, text, font, LIGHT.gamma_multiply(fade));
    }
}

/// Paints the dog inside `rect` at `motion`, keeping the view box's
/// proportions and centring its 500 × 340 in it.
pub(super) fn paint(painter: &egui::Painter, rect: Rect, motion: Motion) {
    let scale = (rect.width() / 500.0).min(rect.height() / 340.0);
    let offset = rect.center().to_vec2() - vec2(250.0, 170.0) * scale;
    let mut c = Canvas {
        painter,
        scale,
        offset,
        shift: egui::Vec2::ZERO,
    };

    // Ground shadow: it narrows as the chest lifts off it.
    c.fill_ellipse(
        235.0,
        282.0,
        155.0 - 5.0 * motion.breath,
        16.0,
        SHADOW,
        None,
    );

    // Curled tail, swaying: the fill as three convex pieces, then the outline
    // along the whole path.
    c.shift = vec2(2.0 * motion.sway, -1.5 * motion.sway);
    let curl = c.path(&TAIL[..1]);
    let tip = c.path(&TAIL[1..]);
    c.fill(curl.clone(), GREY);
    c.fill(tip.clone(), GREY);
    c.fill(
        vec![c.at(365.0, 225.0), c.at(375.0, 262.0), c.at(372.0, 238.0)],
        GREY,
    );
    let mut tail = curl;
    tail.extend(tip.into_iter().skip(1));
    c.line(tail, 6.0);

    // The loaf-shaped body and everything it carries breathe together; the
    // paws and the shadow below stay on the ground.
    c.shift = vec2(0.0, -2.0 * motion.breath);
    let mut body = c.path(&LOAF);
    body.pop();
    c.painter
        .add(Shape::convex_polygon(body, BODY, c.stroke(6.0)));

    // Grey patches on the back, and the back's outline over them.
    for patch in &PATCHES {
        c.fill(c.path(&[*patch]), GREY);
    }
    c.line(c.path(&BACK), 6.0);

    // Floppy ears, which sway a little behind the breath.
    c.shift += vec2(1.5 * motion.sway, 0.0);
    for ear in &EARS {
        let points = c.path(ear);
        c.fill(points.clone(), GREY);
        c.line(points, 6.0);
    }

    // Muzzle, blushes, closed eyes, nose and mouth.
    c.shift = vec2(0.0, -2.0 * motion.breath);
    c.fill_ellipse(178.0, 218.0, 34.0, 24.0, LIGHT, None);
    c.fill_ellipse(132.0, 215.0, 11.0, 6.0, BLUSH, None);
    c.fill_ellipse(224.0, 215.0, 11.0, 6.0, BLUSH, None);
    for eye in &EYES {
        c.line(c.path(&[*eye]), 6.0);
    }
    c.fill_ellipse(178.0, 208.0, 8.0, 5.5, OUTLINE, None);
    c.line(c.path(&MOUTH), 5.0);

    // The sleep bubble at the nose, which swells on the way out.
    let swell = 1.0 - 0.09 * motion.breath;
    c.fill_ellipse(
        145.0,
        232.0,
        18.0 * swell,
        18.0 * swell,
        BUBBLE,
        Some(5.0 / swell),
    );
    c.fill_ellipse(138.0, 225.0, 4.0, 4.0, Color32::WHITE, None);

    // Paws under the belly, on the ground.
    c.shift = egui::Vec2::ZERO;
    for (x, y) in PAWS {
        c.fill_ellipse(x, y, 18.0, 10.0, LIGHT, Some(6.0));
    }

    // z z Z, floating up and fading as they go, the largest the furthest.
    let fade = 1.0 - 0.4 * motion.drift;
    for (x, y, size, outline, rise, text) in LETTERS {
        c.shift = vec2(0.0, -rise * motion.drift);
        c.letter(x, y, size, outline, fade, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop rests at the frame the mock-up approved and comes back to it
    /// every four seconds, whenever the window's clock started.
    #[test]
    fn the_loop_is_four_seconds_long_and_rests_at_the_approved_frame() {
        assert_eq!(Motion::at(0.0), Motion::REST);
        for turn in 1..=3 {
            assert_eq!(
                Motion::at(f64::from(turn) * LOOP_SECONDS),
                Motion::REST,
                "{turn} loops on"
            );
        }
        for seconds in [0.5, 1.0, 2.0, 3.5] {
            assert_eq!(
                Motion::at(seconds),
                Motion::at(seconds + LOOP_SECONDS),
                "{seconds} s into the loop"
            );
            assert_ne!(Motion::at(seconds), Motion::REST, "{seconds} s: it moves");
        }
    }

    /// The letters only ever rise from the rest frame, never sink below it.
    #[test]
    fn the_letters_only_float_up() {
        for step in 0..=40 {
            let motion = Motion::at(f64::from(step) * LOOP_SECONDS / 40.0);
            assert!(motion.drift >= 0.0, "at {step}/40 of the loop");
        }
    }

    /// The box is the ticket's size at both standard windows, and the view
    /// box's proportions at every height.
    #[test]
    fn the_box_is_220_points_wide_at_the_minimum_window_and_280_at_the_default() {
        assert_eq!(size_for(540.0).x, 220.0);
        assert_eq!(size_for(1000.0).x, 280.0);
        // Smaller than the minimum window and larger than the default stay put.
        assert_eq!(size_for(400.0).x, 220.0);
        assert_eq!(size_for(1600.0).x, 280.0);
        for height in [540.0, 700.0, 1000.0, 1400.0] {
            let size = size_for(height);
            assert!((size.y - size.x * ASPECT).abs() < 0.01, "at {height}");
        }
    }
}

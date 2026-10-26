//! Throwaway mock-up for KTLO-20: the owner's sleeping dog, drawn with the
//! egui painter from the paths of the supplied SVG (500 × 340 view box), as
//! the still frame its `prefers-reduced-motion` block shows.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};

/// The dog's box in points: 220 wide at the minimum window, growing with the
/// board's height up to 280 at the default window, in the view box's 50:34.
pub(super) fn size_for(area_height: f32) -> egui::Vec2 {
    let width = (area_height * 0.32).clamp(220.0, 280.0);
    vec2(width, width * 0.68)
}

const OUTLINE: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
const BODY: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);
const GREY: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
const LIGHT: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC);
const BLUSH: Color32 = Color32::from_rgb(0xFD, 0xA4, 0xAF);
const BUBBLE: Color32 = Color32::from_rgba_premultiplied(168, 181, 190, 191);
const SHADOW: Color32 = Color32::from_rgba_premultiplied(52, 57, 64, 89);

struct Canvas<'a> {
    painter: &'a egui::Painter,
    scale: f32,
    offset: egui::Vec2,
}

impl Canvas<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(x, y) * self.scale + self.offset
    }

    fn stroke(&self, width: f32) -> Stroke {
        Stroke::new(width * self.scale, OUTLINE)
    }

    /// Points along a cubic Bézier, the first included and the last included.
    fn cubic(
        &self,
        points: &mut Vec<Pos2>,
        p0: (f32, f32),
        c1: (f32, f32),
        c2: (f32, f32),
        p1: (f32, f32),
    ) {
        const STEPS: usize = 24;
        if points.is_empty() {
            points.push(self.at(p0.0, p0.1));
        }
        for step in 1..=STEPS {
            let t = step as f32 / STEPS as f32;
            let u = 1.0 - t;
            let x = u * u * u * p0.0
                + 3.0 * u * u * t * c1.0
                + 3.0 * u * t * t * c2.0
                + t * t * t * p1.0;
            let y = u * u * u * p0.1
                + 3.0 * u * u * t * c1.1
                + 3.0 * u * t * t * c2.1
                + t * t * t * p1.1;
            points.push(self.at(x, y));
        }
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

    /// A letter with a light face and a dark outline, like the SVG's
    /// `paint-order: stroke fill` text.
    fn letter(&self, x: f32, y: f32, size: f32, outline: f32, text: &str) {
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
                OUTLINE,
            );
        }
        self.painter.text(position, anchor, text, font, LIGHT);
    }
}

/// Paints the dog inside `rect`, keeping the SVG's proportions and centring
/// the 500 × 340 view box in it.
pub(super) fn paint(painter: &egui::Painter, rect: Rect) {
    let scale = (rect.width() / 500.0).min(rect.height() / 340.0);
    let offset = rect.center().to_vec2() - vec2(250.0, 170.0) * scale;
    let c = Canvas {
        painter,
        scale,
        offset,
    };

    // Ground shadow.
    c.fill_ellipse(235.0, 282.0, 155.0, 16.0, SHADOW, None);

    // Curled tail: the fill as three convex pieces, then the outline along the path.
    let mut tail_a = Vec::new();
    c.cubic(
        &mut tail_a,
        (365.0, 225.0),
        (405.0, 205.0),
        (415.0, 255.0),
        (375.0, 262.0),
    );
    let mut tail_b = Vec::new();
    c.cubic(
        &mut tail_b,
        (375.0, 262.0),
        (355.0, 265.0),
        (355.0, 240.0),
        (372.0, 238.0),
    );
    c.fill(tail_a.clone(), GREY);
    c.fill(tail_b.clone(), GREY);
    c.fill(
        vec![c.at(365.0, 225.0), c.at(375.0, 262.0), c.at(372.0, 238.0)],
        GREY,
    );
    let mut tail = tail_a;
    tail.extend(tail_b.into_iter().skip(1));
    c.line(tail, 6.0);

    // The loaf-shaped body.
    let mut body = Vec::new();
    c.cubic(
        &mut body,
        (105.0, 240.0),
        (95.0, 135.0),
        (175.0, 110.0),
        (255.0, 115.0),
    );
    c.cubic(
        &mut body,
        (255.0, 115.0),
        (335.0, 120.0),
        (385.0, 160.0),
        (375.0, 245.0),
    );
    c.cubic(
        &mut body,
        (375.0, 245.0),
        (370.0, 278.0),
        (330.0, 278.0),
        (240.0, 278.0),
    );
    c.cubic(
        &mut body,
        (240.0, 278.0),
        (150.0, 278.0),
        (110.0, 278.0),
        (105.0, 240.0),
    );
    body.pop();
    c.painter
        .add(Shape::convex_polygon(body, BODY, c.stroke(6.0)));

    // Grey patches on the back, and the back's outline over them.
    let mut patch = Vec::new();
    c.cubic(
        &mut patch,
        (245.0, 116.0),
        (245.0, 148.0),
        (285.0, 152.0),
        (295.0, 120.0),
    );
    c.fill(patch, GREY);
    let mut patch = Vec::new();
    c.cubic(
        &mut patch,
        (310.0, 126.0),
        (308.0, 148.0),
        (335.0, 152.0),
        (342.0, 138.0),
    );
    c.fill(patch, GREY);
    let mut back = Vec::new();
    c.cubic(
        &mut back,
        (230.0, 115.0),
        (275.0, 115.0),
        (325.0, 125.0),
        (355.0, 148.0),
    );
    c.line(back, 6.0);

    // Floppy ears.
    let mut ear = Vec::new();
    c.cubic(
        &mut ear,
        (135.0, 145.0),
        (100.0, 140.0),
        (80.0, 180.0),
        (100.0, 205.0),
    );
    c.cubic(
        &mut ear,
        (100.0, 205.0),
        (115.0, 220.0),
        (138.0, 195.0),
        (140.0, 165.0),
    );
    c.fill(ear.clone(), GREY);
    c.line(ear, 6.0);
    let mut ear = Vec::new();
    c.cubic(
        &mut ear,
        (225.0, 135.0),
        (255.0, 130.0),
        (275.0, 165.0),
        (255.0, 195.0),
    );
    c.cubic(
        &mut ear,
        (255.0, 195.0),
        (240.0, 210.0),
        (220.0, 185.0),
        (218.0, 155.0),
    );
    c.fill(ear.clone(), GREY);
    c.line(ear, 6.0);

    // Muzzle, blushes, closed eyes, nose and mouth.
    c.fill_ellipse(178.0, 218.0, 34.0, 24.0, LIGHT, None);
    c.fill_ellipse(132.0, 215.0, 11.0, 6.0, BLUSH, None);
    c.fill_ellipse(224.0, 215.0, 11.0, 6.0, BLUSH, None);
    let mut eye = Vec::new();
    c.cubic(
        &mut eye,
        (135.0, 192.0),
        (140.0, 204.0),
        (155.0, 204.0),
        (160.0, 192.0),
    );
    c.line(eye, 6.0);
    let mut eye = Vec::new();
    c.cubic(
        &mut eye,
        (196.0, 192.0),
        (201.0, 204.0),
        (216.0, 204.0),
        (221.0, 192.0),
    );
    c.line(eye, 6.0);
    c.fill_ellipse(178.0, 208.0, 8.0, 5.5, OUTLINE, None);
    let mut mouth = Vec::new();
    c.cubic(
        &mut mouth,
        (164.0, 220.0),
        (170.0, 227.0),
        (178.0, 223.0),
        (178.0, 214.0),
    );
    c.cubic(
        &mut mouth,
        (178.0, 214.0),
        (178.0, 223.0),
        (186.0, 227.0),
        (192.0, 220.0),
    );
    c.line(mouth, 5.0);

    // The sleep bubble at the nose.
    c.fill_ellipse(145.0, 232.0, 18.0, 18.0, BUBBLE, Some(5.0));
    c.fill_ellipse(138.0, 225.0, 4.0, 4.0, Color32::WHITE, None);

    // Paws under the belly.
    for (x, y) in [(140.0, 276.0), (215.0, 276.0), (325.0, 274.0)] {
        c.fill_ellipse(x, y, 18.0, 10.0, LIGHT, Some(6.0));
    }

    // z z Z.
    c.letter(280.0, 100.0, 26.0, 8.0, "z");
    c.letter(320.0, 78.0, 38.0, 10.0, "z");
    c.letter(368.0, 58.0, 52.0, 12.0, "Z");
}

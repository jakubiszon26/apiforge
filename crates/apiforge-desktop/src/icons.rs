//! Symbolic actions drawn on a shared 24-unit grid, independent of font glyphs.
use eframe::egui::{self, Color32, Painter, Rect, Shape, Stroke, StrokeKind, vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Stop,
    Plus,
    Minus,
    Search,
    Settings,
    More,
    Undo,
    Redo,
    ChevronDown,
    ChevronRight,
    Panel,
    Close,
    ArrowLeft,
    ExternalLink,
    Import,
    Export,
    Save,
    Check,
    Copy,
    Trash,
    Layers,
    Fit,
    Clipboard,
    Variable,
    Refresh,
    Eye,
    EyeOff,
    /// Cards arranged in columns, for automatic layout.
    Layout,
    /// A wire turning a corner, for the wire style.
    Wire,
    /// Angle brackets, for the code view.
    Code,
}

impl Icon {
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let size = rect.width().min(rect.height());
        let origin = rect.center() - vec2(size, size) * 0.5;
        let p = |x: f32, y: f32| origin + vec2(x, y) * (size / 24.0);
        let stroke = Stroke::new(1.7 * size / 24.0, color);
        let line = |a: (f32, f32), b: (f32, f32)| {
            painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
        };
        let path = |points: &[(f32, f32)]| {
            painter.add(Shape::line(
                points.iter().map(|&(x, y)| p(x, y)).collect(),
                stroke,
            ));
        };
        let outline = |left: f32, top: f32, right: f32, bottom: f32| {
            painter.rect_stroke(
                Rect::from_min_max(p(left, top), p(right, bottom)),
                2.0 * size / 24.0,
                stroke,
                StrokeKind::Inside,
            );
        };
        match self {
            Self::Play => {
                painter.add(Shape::convex_polygon(
                    vec![p(7.0, 4.5), p(19.0, 12.0), p(7.0, 19.5)],
                    color,
                    Stroke::NONE,
                ));
            }
            Self::Stop => {
                painter.rect_filled(Rect::from_min_max(p(5.0, 5.0), p(19.0, 19.0)), 1, color);
            }
            Self::Plus => {
                line((5.0, 12.0), (19.0, 12.0));
                line((12.0, 5.0), (12.0, 19.0));
            }
            Self::Minus => line((5.0, 12.0), (19.0, 12.0)),
            Self::Search => {
                painter.circle_stroke(p(10.5, 10.5), size * 6.5 / 24.0, stroke);
                line((15.5, 15.5), (21.0, 21.0));
            }
            Self::Settings => {
                let points = (0..32)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 32.0;
                        let radius = if i % 4 == 0 || i % 4 == 3 { 10.0 } else { 8.0 };
                        p(12.0 + radius * angle.cos(), 12.0 + radius * angle.sin())
                    })
                    .collect();
                painter.add(Shape::closed_line(points, stroke));
                painter.circle_stroke(p(12.0, 12.0), size * 3.0 / 24.0, stroke);
            }
            Self::More => {
                for x in [5.0, 12.0, 19.0] {
                    painter.circle_filled(p(x, 12.0), size * 1.5 / 24.0, color);
                }
            }
            Self::Undo | Self::Redo => {
                let flip = |x: f32, y: f32| {
                    p(
                        if matches!(self, Self::Redo) {
                            24.0 - x
                        } else {
                            x
                        },
                        y,
                    )
                };
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [
                        flip(5.0, 8.0),
                        flip(23.0, 2.0),
                        flip(24.0, 24.0),
                        flip(9.0, 19.0),
                    ],
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
                painter.add(Shape::line(
                    vec![flip(5.0, 3.0), flip(5.0, 8.0), flip(10.0, 10.0)],
                    stroke,
                ));
            }
            Self::ChevronDown => path(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Self::ChevronRight => path(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]),
            Self::Panel => {
                outline(3.0, 4.0, 21.0, 20.0);
                line((15.0, 4.0), (15.0, 20.0));
            }
            Self::Close => {
                line((6.0, 6.0), (18.0, 18.0));
                line((18.0, 6.0), (6.0, 18.0));
            }
            Self::ArrowLeft => {
                line((4.0, 12.0), (20.0, 12.0));
                path(&[(10.0, 6.0), (4.0, 12.0), (10.0, 18.0)]);
            }
            Self::ExternalLink => {
                path(&[
                    (10.0, 4.0),
                    (4.0, 4.0),
                    (4.0, 20.0),
                    (20.0, 20.0),
                    (20.0, 14.0),
                ]);
                path(&[(14.0, 3.0), (21.0, 3.0), (21.0, 10.0)]);
                line((11.0, 13.0), (21.0, 3.0));
            }
            Self::Import | Self::Export => {
                path(&[(4.0, 15.0), (4.0, 21.0), (20.0, 21.0), (20.0, 15.0)]);
                line((12.0, 3.0), (12.0, 15.0));
                if matches!(self, Self::Import) {
                    path(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)]);
                } else {
                    path(&[(7.0, 8.0), (12.0, 3.0), (17.0, 8.0)]);
                }
            }
            Self::Save => {
                path(&[
                    (4.0, 3.0),
                    (17.0, 3.0),
                    (21.0, 7.0),
                    (21.0, 21.0),
                    (3.0, 21.0),
                    (3.0, 3.0),
                    (4.0, 3.0),
                ]);
                outline(7.0, 3.0, 16.0, 10.0);
                outline(7.0, 15.0, 17.0, 21.0);
            }
            Self::Check => path(&[(4.0, 12.0), (9.0, 17.0), (20.0, 6.0)]),
            Self::Copy => {
                outline(8.0, 8.0, 21.0, 21.0);
                path(&[
                    (16.0, 5.0),
                    (16.0, 3.0),
                    (3.0, 3.0),
                    (3.0, 16.0),
                    (5.0, 16.0),
                ]);
            }
            Self::Trash => {
                path(&[(5.0, 7.0), (6.0, 21.0), (18.0, 21.0), (19.0, 7.0)]);
                line((3.0, 6.0), (21.0, 6.0));
                path(&[(8.0, 6.0), (8.0, 3.0), (16.0, 3.0), (16.0, 6.0)]);
                line((10.0, 10.0), (10.0, 17.0));
                line((14.0, 10.0), (14.0, 17.0));
            }
            Self::Layers => {
                painter.add(Shape::closed_line(
                    vec![p(12.0, 3.0), p(22.0, 8.0), p(12.0, 13.0), p(2.0, 8.0)],
                    stroke,
                ));
                path(&[(3.0, 13.0), (12.0, 18.0), (21.0, 13.0)]);
                path(&[(3.0, 18.0), (12.0, 23.0), (21.0, 18.0)]);
            }
            Self::Fit => {
                for (x, y, sx, sy) in [
                    (3.0, 3.0, 1.0, 1.0),
                    (21.0, 3.0, -1.0, 1.0),
                    (3.0, 21.0, 1.0, -1.0),
                    (21.0, 21.0, -1.0, -1.0),
                ] {
                    path(&[(x, y + 6.0 * sy), (x, y), (x + 6.0 * sx, y)]);
                }
            }
            Self::Clipboard => {
                outline(5.0, 5.0, 19.0, 22.0);
                painter.rect_filled(Rect::from_min_max(p(8.0, 2.0), p(16.0, 7.0)), 1, color);
                line((9.0, 12.0), (15.0, 12.0));
                line((9.0, 16.0), (15.0, 16.0));
            }
            Self::Variable => {
                path(&[
                    (8.0, 3.0),
                    (5.0, 3.0),
                    (5.0, 10.0),
                    (3.0, 12.0),
                    (5.0, 14.0),
                    (5.0, 21.0),
                    (8.0, 21.0),
                ]);
                path(&[
                    (16.0, 3.0),
                    (19.0, 3.0),
                    (19.0, 10.0),
                    (21.0, 12.0),
                    (19.0, 14.0),
                    (19.0, 21.0),
                    (16.0, 21.0),
                ]);
            }
            Self::Refresh => {
                painter.add(Shape::line(
                    (0..25)
                        .map(|i| {
                            let angle = (i as f32 / 24.0 * 1.6 - 0.4) * std::f32::consts::PI;
                            p(12.0 + angle.cos() * 8.0, 12.0 + angle.sin() * 8.0)
                        })
                        .collect(),
                    stroke,
                ));
                path(&[(21.0, 3.0), (21.0, 9.0), (15.0, 9.0)]);
            }
            Self::Layout => {
                outline(3.0, 4.0, 10.0, 11.0);
                outline(3.0, 14.0, 10.0, 20.0);
                outline(14.0, 7.0, 21.0, 17.0);
            }
            Self::Wire => {
                path(&[(3.0, 6.0), (12.0, 6.0), (12.0, 18.0), (21.0, 18.0)]);
                painter.circle_stroke(p(3.5, 6.0), size * 1.5 / 24.0, stroke);
                painter.circle_stroke(p(20.5, 18.0), size * 1.5 / 24.0, stroke);
            }
            Self::Code => {
                path(&[(9.0, 7.0), (4.0, 12.0), (9.0, 17.0)]);
                path(&[(15.0, 7.0), (20.0, 12.0), (15.0, 17.0)]);
            }
            Self::Eye | Self::EyeOff => {
                let points = [
                    (2.0, 12.0),
                    (7.0, 5.0),
                    (17.0, 5.0),
                    (22.0, 12.0),
                    (17.0, 19.0),
                    (7.0, 19.0),
                    (2.0, 12.0),
                ];
                path(&points);
                painter.circle_stroke(p(12.0, 12.0), size * 3.0 / 24.0, stroke);
                if matches!(self, Self::EyeOff) {
                    line((3.0, 3.0), (21.0, 21.0));
                }
            }
        }
    }
}

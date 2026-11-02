//! COMP-00 mock-up (throwaway branch `mock/comp-00`): a component's inputs on
//! its board. The inputs become proxies of their parameter that exist only
//! while drawing, so the board's own routing wires them to the steps that
//! read them; nothing is written to the document or its layout.
use super::Camera;
use super::layout::{CHIP_HEIGHT, Card, Chip};
use crate::icons::Icon;
use crate::mock_comp00::{self as mock, Adding, BoardInput, Inputs};
use crate::{theme, widgets::paint_label as label};
use apiforge_core::format::{Position, Proxy, ProxySource};
use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};

/// Option A's card.
pub const FRAME_WIDTH: f32 = 252.0;
const FRAME_HEADER: f32 = 60.0;
const ROW: f32 = 40.0;
const INLINE_ROW: f32 = 44.0;
const ADD_ROW: f32 = 36.0;
/// Gap between option A's card, or option B's chips, and the first step.
const GAP: f32 = 72.0;

/// What the mock-up adds to the board this frame, in board coordinates.
#[derive(Clone, Debug, Default)]
pub struct Extra {
    /// Option A's card.
    pub frame: Option<Rect>,
    /// *+ Add input*: a row of A's card, a pill under B's chips.
    pub add: Option<Rect>,
    /// Q2 A: the placeholder of the input being added.
    pub placeholder: Option<Rect>,
    /// Q2 B: the inline row typing the input being added.
    pub inline: Option<Rect>,
    pub inputs: Vec<BoardInput>,
}

impl Extra {
    pub fn bounds(&self) -> Option<Rect> {
        [self.frame, self.add, self.placeholder, self.inline]
            .into_iter()
            .flatten()
            .reduce(Rect::union)
    }
}

fn chip(input: &BoardInput, origin: Pos2, width: f32) -> Chip {
    let id = mock::chip_id(&input.name);
    mock::set_chip_width(&id, width);
    Chip {
        proxy: Proxy {
            id,
            source: ProxySource::Parameter {
                name: input.name.clone(),
            },
            position: Position {
                x: f64::from(origin.x),
                y: f64::from(origin.y),
            },
        },
        label: input.name.clone(),
        caption: "input",
        known: true,
        origin,
    }
}

/// Text widths without a font, as `Chip::width` estimates them.
fn pill_width(input: &BoardInput) -> f32 {
    7.6 * (input.name.chars().count() + 2) as f32
        + 6.4 * input.detail().chars().count() as f32
        + 62.0
}

/// Adds the inputs of the component on the board to `chips`.
pub fn place(cards: &[Card], chips: &mut Vec<Chip>) -> Extra {
    let flags = mock::get();
    let Some(board) = mock::board() else {
        return Extra::default();
    };
    if matches!(flags.inputs, Inputs::Off | Inputs::Band) {
        return Extra {
            inputs: board.inputs,
            ..Extra::default()
        };
    }
    let entry = cards
        .iter()
        .min_by(|a, b| {
            a.origin
                .x
                .total_cmp(&b.origin.x)
                .then(a.origin.y.total_cmp(&b.origin.y))
        })
        .map_or(
            Rect::from_min_size(pos2(80.0, 80.0), vec2(1.0, 1.0)),
            Card::rect,
        );
    let top = cards
        .iter()
        .map(|card| card.origin.y)
        .reduce(f32::min)
        .unwrap_or(entry.top());
    let mut extra = Extra {
        inputs: board.inputs.clone(),
        ..Extra::default()
    };
    match flags.inputs {
        Inputs::Card => {
            let left = entry.left() - GAP - FRAME_WIDTH;
            let mut y = top + FRAME_HEADER + 4.0;
            for input in &board.inputs {
                chips.push(chip(input, pos2(left, y + 2.0), FRAME_WIDTH));
                y += ROW;
            }
            match flags.adding {
                Adding::InspectorForm => {
                    extra.placeholder = Some(Rect::from_min_size(
                        pos2(left, y + 2.0),
                        vec2(FRAME_WIDTH, CHIP_HEIGHT),
                    ));
                    y += ROW;
                }
                Adding::InlineRow => {
                    extra.inline = Some(Rect::from_min_size(
                        pos2(left, y + 2.0),
                        vec2(FRAME_WIDTH, INLINE_ROW - 4.0),
                    ));
                    y += INLINE_ROW;
                }
                Adding::Off => {}
            }
            extra.add = Some(Rect::from_min_size(
                pos2(left, y + 2.0),
                vec2(FRAME_WIDTH, ADD_ROW - 4.0),
            ));
            y += ADD_ROW;
            extra.frame = Some(Rect::from_min_max(
                pos2(left, top),
                pos2(left + FRAME_WIDTH, y + 8.0),
            ));
        }
        Inputs::Chips => {
            let right = entry.left() - GAP;
            let mut y = entry.top() + 12.0;
            for input in &board.inputs {
                let width = pill_width(input);
                chips.push(chip(input, pos2(right - width, y), width));
                y += CHIP_HEIGHT + 12.0;
            }
            if flags.adding == Adding::InspectorForm {
                let width = 150.0;
                extra.placeholder = Some(Rect::from_min_size(
                    pos2(right - width, y),
                    vec2(width, CHIP_HEIGHT),
                ));
                y += CHIP_HEIGHT + 12.0;
            }
            extra.add = Some(Rect::from_min_size(
                pos2(right - 112.0, y),
                vec2(112.0, CHIP_HEIGHT - 6.0),
            ));
        }
        Inputs::Off | Inputs::Band => {}
    }
    extra
}

/// Whether a chip is one of the mock-up's inputs.
pub fn is_input(chip: &Chip) -> bool {
    mock::input_of_chip(chip.id()).is_some()
}

/// Option A's card, under its rows.
pub fn paint_frame(painter: &Painter, extra: &Extra, camera: Camera) {
    let Some(frame) = extra.frame else {
        return;
    };
    let z = camera.zoom;
    let rect = camera.rect(frame);
    let radius = (10.0 * z).max(3.0);
    painter.rect_filled(
        rect.translate(vec2(0.0, 2.0 * z)),
        radius,
        Color32::from_black_alpha(8),
    );
    painter.rect_filled(rect, radius, theme::WHITE);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let local = |x: f32, y: f32| camera.to_screen(frame.min + vec2(x, y));
    let cell = |l: f32, t: f32, r: f32, b: f32| Rect::from_min_max(local(l, t), local(r, b));
    let icon = Rect::from_min_size(local(16.0, 16.0), Vec2::splat(28.0 * z));
    painter.rect_filled(icon, 7.0 * z, theme::LINK.gamma_multiply(0.10));
    Icon::Import.paint(painter, icon.shrink(5.0 * z), theme::LINK);
    label(
        painter,
        cell(56.0, 12.0, FRAME_WIDTH - 16.0, 28.0),
        "Component",
        FontId::proportional(10.5 * z),
        theme::MUTED,
        egui::Align::Min,
    );
    label(
        painter,
        cell(56.0, 28.0, FRAME_WIDTH - 16.0, 52.0),
        &format!("Inputs  {}", extra.inputs.len()),
        FontId::new((16.0 * z).max(7.5), theme::bold_family()),
        theme::TEXT,
        egui::Align::Min,
    );
    painter.line_segment(
        [local(0.0, FRAME_HEADER), local(FRAME_WIDTH, FRAME_HEADER)],
        Stroke::new(1.0, theme::BORDER),
    );
    let flags = mock::get();
    if let Some(row) = extra.placeholder {
        let rect = camera.rect(row.shrink2(vec2(8.0, 2.0)));
        painter.rect(
            rect,
            6.0 * z,
            theme::ACCENT_SOFT,
            Stroke::new(1.7, theme::ACCENT),
            StrokeKind::Inside,
        );
        let name = if flags.new_name.is_empty() {
            "New input"
        } else {
            flags.new_name.as_str()
        };
        label(
            painter,
            Rect::from_min_max(
                camera.to_screen(row.min + vec2(16.0, 0.0)),
                camera.to_screen(row.max - vec2(16.0, 0.0)),
            ),
            name,
            FontId::new(13.0 * z, theme::bold_family()),
            theme::TEXT,
            egui::Align::Min,
        );
        label(
            painter,
            Rect::from_min_max(
                camera.to_screen(row.min + vec2(16.0, 0.0)),
                camera.to_screen(row.max - vec2(16.0, 0.0)),
            ),
            "being added…",
            FontId::proportional(11.5 * z),
            theme::MUTED,
            egui::Align::Max,
        );
    }
    if let Some(row) = extra.inline {
        inline_row(painter, row, camera, &flags.new_name);
    }
    if let Some(add) = extra.add {
        let at = |x: f32, y: f32| camera.to_screen(add.min + vec2(x, y));
        Icon::Plus.paint(
            painter,
            Rect::from_min_size(at(16.0, 8.0), Vec2::splat(16.0 * z)),
            theme::LINK,
        );
        label(
            painter,
            Rect::from_min_max(at(38.0, 0.0), at(FRAME_WIDTH - 16.0, add.height())),
            "Add input",
            FontId::proportional((13.0 * z).max(7.0)),
            theme::LINK,
            egui::Align::Min,
        );
    }
}

/// Q2 B: name and type typed on the board.
fn inline_row(painter: &Painter, row: Rect, camera: Camera, name: &str) {
    let z = camera.zoom;
    let at = |x: f32, y: f32| camera.to_screen(row.min + vec2(x, y));
    let field = Rect::from_min_max(at(10.0, 4.0), at(132.0, row.height() - 4.0));
    painter.rect(
        field,
        5.0 * z,
        theme::WHITE,
        Stroke::new(1.5, theme::ACCENT),
        StrokeKind::Inside,
    );
    let text = Rect::from_min_max(field.min + vec2(8.0 * z, 0.0), field.max);
    let galley =
        painter.layout_no_wrap(name.to_owned(), FontId::proportional(13.0 * z), theme::TEXT);
    let text_left = text.left();
    painter.galley(
        pos2(text_left, text.center().y - galley.size().y / 2.0),
        galley.clone(),
        theme::TEXT,
    );
    let caret = text_left + galley.size().x + 1.0;
    painter.line_segment(
        [
            pos2(caret, text.center().y - 8.0 * z),
            pos2(caret, text.center().y + 8.0 * z),
        ],
        Stroke::new(1.2, theme::TEXT),
    );
    let combo = Rect::from_min_max(at(138.0, 4.0), at(212.0, row.height() - 4.0));
    painter.rect(
        combo,
        5.0 * z,
        theme::FIELD,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    label(
        painter,
        combo.shrink2(vec2(8.0 * z, 0.0)),
        "string",
        FontId::proportional(12.5 * z),
        theme::TEXT,
        egui::Align::Min,
    );
    Icon::ChevronDown.paint(
        painter,
        Rect::from_center_size(
            pos2(combo.right() - 10.0 * z, combo.center().y),
            Vec2::splat(12.0 * z),
        ),
        theme::MUTED,
    );
    let ok = Rect::from_min_max(at(218.0, 4.0), at(FRAME_WIDTH - 8.0, row.height() - 4.0));
    painter.rect_filled(ok, 5.0 * z, theme::ACCENT);
    Icon::Check.paint(painter, ok.shrink(5.0 * z), theme::WHITE);
}

/// One input: a row of A's card, or B's pill.
pub fn paint_input(painter: &Painter, chip: &Chip, camera: Camera, selected: bool, extra: &Extra) {
    let Some(name) = mock::input_of_chip(chip.id()) else {
        return;
    };
    let Some(input) = extra.inputs.iter().find(|input| input.name == name) else {
        return;
    };
    let flags = mock::get();
    let z = camera.zoom;
    let width = chip.width();
    let rect = camera.rect(chip.rect());
    let invalid = matches!(&flags.panel, mock::Panel::Input { name: edited, draft_name: Some(_) } if edited == name);
    let row = extra.frame.is_some();
    let (left_pad, right_pad) = if row { (16.0, 18.0) } else { (12.0, 16.0) };
    if row {
        if selected {
            painter.rect(
                camera.rect(chip.rect().shrink2(vec2(8.0, 0.0))),
                6.0 * z,
                theme::ACCENT_SOFT,
                Stroke::new(
                    1.7,
                    if invalid {
                        theme::DANGER
                    } else {
                        theme::ACCENT
                    },
                ),
                StrokeKind::Inside,
            );
        }
    } else {
        let radius = (CHIP_HEIGHT / 2.0 * z).max(2.0);
        painter.rect_filled(
            rect.translate(vec2(0.0, 1.5 * z)),
            radius,
            Color32::from_black_alpha(8),
        );
        painter.rect(
            rect,
            radius,
            if selected {
                theme::ACCENT_SOFT
            } else {
                theme::WHITE
            },
            Stroke::new(
                if selected { 1.7 } else { 1.0 },
                if invalid {
                    theme::DANGER
                } else if selected {
                    theme::ACCENT
                } else {
                    theme::LINK.gamma_multiply(0.55)
                },
            ),
            StrokeKind::Inside,
        );
        Icon::Import.paint(
            painter,
            Rect::from_min_size(
                camera.to_screen(chip.origin + vec2(10.0, 10.0)),
                Vec2::splat(16.0 * z),
            ),
            theme::LINK,
        );
    }
    let start = if row { left_pad } else { 32.0 };
    let shown = match &flags.panel {
        mock::Panel::Input {
            name: edited,
            draft_name: Some(draft),
        } if edited == name => draft.clone(),
        _ => input.name.clone(),
    };
    // Far out, a row keeps a legible name, as a card keeps its title.
    let overview = z < 0.45;
    let name_font = FontId::new(
        (13.0 * z).max(if overview { 7.5 } else { 6.0 }),
        theme::bold_family(),
    );
    let galley = painter.layout_no_wrap(
        shown.clone(),
        name_font.clone(),
        if invalid { theme::DANGER } else { theme::TEXT },
    );
    let text_rect = Rect::from_min_max(
        camera.to_screen(chip.origin + vec2(start, 0.0)),
        camera.to_screen(chip.origin + vec2(width - right_pad, CHIP_HEIGHT)),
    );
    let clip = painter.with_clip_rect(text_rect.intersect(painter.clip_rect()));
    let name_at = pos2(
        text_rect.left(),
        text_rect.center().y - galley.size().y / 2.0,
    );
    let name_width = galley.size().x;
    clip.galley(name_at, galley, theme::TEXT);
    let mut after = name_at.x + name_width;
    if input.required {
        let star = painter.layout_no_wrap(" *".to_owned(), name_font, theme::DANGER);
        let star_width = star.size().x;
        clip.galley(pos2(after, name_at.y), star, theme::DANGER);
        after += star_width;
    }
    let detail_rect = Rect::from_min_max(
        pos2(after + 8.0 * z, text_rect.top()),
        text_rect.right_bottom(),
    );
    let mut detail = if overview {
        String::new()
    } else {
        input.detail()
    };
    if !input.exposed && !overview {
        detail.push_str(" · hidden");
    }
    label(
        painter,
        detail_rect,
        &detail,
        FontId::proportional((11.5 * z).max(5.5)),
        theme::MUTED,
        if row {
            egui::Align::Max
        } else {
            egui::Align::Min
        },
    );
    socket(painter, camera.to_screen(chip.output_anchor()), z);
}

fn socket(painter: &Painter, center: Pos2, zoom: f32) {
    let radius = (4.5 * zoom).max(1.25);
    painter.circle(
        center,
        radius,
        theme::WHITE,
        Stroke::new((1.0 * zoom).clamp(0.6, 1.0), theme::LINK),
    );
}

/// Option B's *+ Input* pill and the placeholder pill of Q2.
pub fn paint_pills(painter: &Painter, extra: &Extra, camera: Camera) {
    if extra.frame.is_some() {
        return;
    }
    let z = camera.zoom;
    let flags = mock::get();
    if let Some(placeholder) = extra.placeholder {
        let rect = camera.rect(placeholder);
        painter.rect(
            rect,
            CHIP_HEIGHT / 2.0 * z,
            theme::ACCENT_SOFT,
            Stroke::new(1.7, theme::ACCENT),
            StrokeKind::Inside,
        );
        label(
            painter,
            rect.shrink2(vec2(14.0 * z, 0.0)),
            if flags.new_name.is_empty() {
                "New input"
            } else {
                &flags.new_name
            },
            FontId::new(13.0 * z, theme::bold_family()),
            theme::TEXT,
            egui::Align::Min,
        );
    }
    if let Some(add) = extra.add {
        let rect = camera.rect(add);
        painter.rect(
            rect,
            rect.height() / 2.0,
            theme::WHITE,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        Icon::Plus.paint(
            painter,
            Rect::from_min_size(
                camera.to_screen(add.min + vec2(12.0, 7.0)),
                Vec2::splat(16.0 * z),
            ),
            theme::LINK,
        );
        // Far out the pill keeps only its plus, as a card keeps its title.
        if z >= 0.45 {
            label(
                painter,
                Rect::from_min_max(
                    camera.to_screen(add.min + vec2(34.0, 0.0)),
                    camera.to_screen(add.max - vec2(8.0, 0.0)),
                ),
                "Add input",
                FontId::proportional((12.5 * z).max(7.0)),
                theme::LINK,
                egui::Align::Min,
            );
        }
    }
}

/// Option C's height under the header row.
pub const BAND: f32 = 40.0;

/// Option C: the inputs as pills in a strip under the header row.
pub fn paint_band(painter: &Painter, band: Rect, inputs: &[BoardInput]) {
    let flags = mock::get();
    painter.rect_filled(band, 0.0, theme::WHITE);
    painter.line_segment(
        [band.left_bottom(), band.right_bottom()],
        Stroke::new(1.0, theme::BORDER),
    );
    let caption = painter.layout_no_wrap(
        "INPUTS".to_owned(),
        FontId::new(theme::FONT_META, theme::bold_family()),
        theme::MUTED,
    );
    let mut x = band.left() + 18.0;
    painter.galley(
        pos2(x, band.center().y - caption.size().y / 2.0),
        caption.clone(),
        theme::MUTED,
    );
    x += caption.size().x + 12.0;
    let add_width = 96.0;
    let room = band.right() - 14.0 - add_width - x;
    let font = FontId::new(12.0, theme::bold_family());
    let small = FontId::proportional(11.0);
    let measure = |text: &str, font: &FontId| {
        painter
            .layout_no_wrap(text.to_owned(), font.clone(), theme::TEXT)
            .size()
            .x
    };
    // Full pills when they fit, names only when they do not.
    let full: f32 = inputs
        .iter()
        .map(|input| measure(&input.name, &font) + measure(&input.detail(), &small) + 40.0)
        .sum();
    let compact = full > room;
    for input in inputs {
        let name_w = measure(&input.name, &font);
        let star_w = if input.required {
            measure(" *", &font)
        } else {
            0.0
        };
        let detail_w = if compact {
            0.0
        } else {
            measure(&input.detail(), &small) + 8.0
        };
        let width = name_w + star_w + detail_w + 24.0;
        let pill = Rect::from_min_size(pos2(x, band.center().y - 13.0), vec2(width, 26.0));
        if pill.right() > band.right() - 14.0 - add_width {
            break;
        }
        let selected = flags.selected_input.as_deref() == Some(input.name.as_str());
        painter.rect(
            pill,
            13.0,
            if selected {
                theme::ACCENT_SOFT
            } else {
                theme::LINK.gamma_multiply(0.08)
            },
            Stroke::new(
                if selected { 1.7 } else { 1.0 },
                if selected {
                    theme::ACCENT
                } else {
                    theme::LINK.gamma_multiply(0.45)
                },
            ),
            StrokeKind::Inside,
        );
        let mut tx = pill.left() + 12.0;
        let name = painter.layout_no_wrap(input.name.clone(), font.clone(), theme::TEXT);
        painter.galley(
            pos2(tx, pill.center().y - name.size().y / 2.0),
            name,
            theme::TEXT,
        );
        tx += name_w;
        if input.required {
            let star = painter.layout_no_wrap(" *".to_owned(), font.clone(), theme::DANGER);
            painter.galley(
                pos2(tx, pill.center().y - star.size().y / 2.0),
                star,
                theme::DANGER,
            );
            tx += star_w;
        }
        if !compact {
            let detail = painter.layout_no_wrap(input.detail(), small.clone(), theme::MUTED);
            painter.galley(
                pos2(tx + 8.0, pill.center().y - detail.size().y / 2.0),
                detail,
                theme::MUTED,
            );
        }
        x = pill.right() + 8.0;
    }
    let add = Rect::from_min_size(pos2(x + 4.0, band.center().y - 13.0), vec2(add_width, 26.0));
    Icon::Plus.paint(
        painter,
        Rect::from_min_size(add.min + vec2(4.0, 5.0), Vec2::splat(16.0)),
        theme::LINK,
    );
    label(
        painter,
        Rect::from_min_max(add.min + vec2(24.0, 0.0), add.max),
        "Add input",
        FontId::proportional(12.5),
        theme::LINK,
        egui::Align::Min,
    );
}

//! The rectangles, ports and edges every algorithm in this crate works on.
//!
//! Nothing here knows about blocks, documents or a renderer: a caller translates its
//! cards into these types and translates the results back. Coordinates are board
//! coordinates in pixels, y growing downwards, and every value is finite — a caller
//! that passes a NaN gets a deterministic but meaningless result rather than a panic.

/// A point in board coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Manhattan distance, the length of an orthogonal route between two points.
    pub fn orthogonal_distance(self, other: Self) -> f32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned rectangle. `min` is the top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn from_size(position: Point, size: Size) -> Self {
        Self {
            min: position,
            max: Point::new(position.x + size.width, position.y + size.height),
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn center(&self) -> Point {
        Point::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    /// Grows the rectangle by `margin` on every side.
    #[must_use]
    pub fn inflate(&self, margin: f32) -> Self {
        Self {
            min: Point::new(self.min.x - margin, self.min.y - margin),
            max: Point::new(self.max.x + margin, self.max.y + margin),
        }
    }

    /// Whether the point is inside, the border included.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Whether the point is strictly inside, so touching the border is outside.
    pub fn contains_strictly(&self, point: Point) -> bool {
        point.x > self.min.x && point.x < self.max.x && point.y > self.min.y && point.y < self.max.y
    }

    /// Whether the two rectangles share any area; touching borders do not.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }

    /// The smallest rectangle containing both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// The smallest rectangle containing all of them, or `None` for none.
    pub fn bounding(rectangles: impl IntoIterator<Item = Self>) -> Option<Self> {
        rectangles
            .into_iter()
            .reduce(|bounds, rectangle| bounds.union(&rectangle))
    }
}

/// Which side of a card a port sits on. Data flows left to right, order downwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The direction a wire leaves this side, as a unit vector.
    pub fn outward(self) -> Point {
        match self {
            Self::Left => Point::new(-1.0, 0.0),
            Self::Right => Point::new(1.0, 0.0),
            Self::Top => Point::new(0.0, -1.0),
            Self::Bottom => Point::new(0.0, 1.0),
        }
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// A port on one side of a card, `offset` pixels along that side from its start
/// (from the top for a left or right side, from the left for a top or bottom one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    pub block: usize,
    pub side: Side,
    pub offset: f32,
}

impl Port {
    pub const fn new(block: usize, side: Side, offset: f32) -> Self {
        Self {
            block,
            side,
            offset,
        }
    }

    /// Where the port sits on the given rectangle. The offset is clamped to the
    /// side, so a stale offset cannot place a port outside its card.
    pub fn point_on(&self, rectangle: &Rect) -> Point {
        match self.side {
            Side::Left | Side::Right => {
                let y = (rectangle.min.y + self.offset).clamp(rectangle.min.y, rectangle.max.y);
                let x = if self.side == Side::Left {
                    rectangle.min.x
                } else {
                    rectangle.max.x
                };
                Point::new(x, y)
            }
            Side::Top | Side::Bottom => {
                let x = (rectangle.min.x + self.offset).clamp(rectangle.min.x, rectangle.max.x);
                let y = if self.side == Side::Top {
                    rectangle.min.y
                } else {
                    rectangle.max.y
                };
                Point::new(x, y)
            }
        }
    }
}

/// A card to lay out: its size, where it is now and whether layout may move it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub size: Size,
    /// The current top-left corner. A block that has never been placed still needs
    /// one; callers pass the position the document holds.
    pub position: Point,
    /// A block the user moved by hand. Incremental placement treats it as an
    /// obstacle and never moves it; *tidy up* ignores the flag.
    pub pinned: bool,
}

impl Block {
    pub fn new(size: Size, position: Point) -> Self {
        Self {
            size,
            position,
            pinned: false,
        }
    }

    #[must_use]
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    pub fn rect(&self) -> Rect {
        Rect::from_size(self.position, self.size)
    }
}

/// A dependency between two cards, pointing in the direction of the flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
    /// Offset of the source port along its side, used to order cards within a layer
    /// so that wires leaving one card do not cross each other needlessly.
    pub source_offset: f32,
    pub target_offset: f32,
}

impl Edge {
    pub const fn new(source: usize, target: usize) -> Self {
        Self {
            source,
            target,
            source_offset: 0.0,
            target_offset: 0.0,
        }
    }

    #[must_use]
    pub const fn with_offsets(mut self, source: f32, target: f32) -> Self {
        self.source_offset = source;
        self.target_offset = target;
        self
    }
}

/// Compares two finite numbers, treating a NaN as larger than everything so that
/// sorting stays total and deterministic instead of panicking.
pub(crate) fn compare(left: f32, right: f32) -> std::cmp::Ordering {
    left.partial_cmp(&right)
        .unwrap_or(match (left.is_nan(), right.is_nan()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_touching_at_a_border_do_not_overlap() {
        let left = Rect::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let right = Rect::new(Point::new(10.0, 0.0), Point::new(20.0, 10.0));
        assert!(!left.overlaps(&right));
        assert!(left.overlaps(&right.inflate(0.5)));
        assert!(left.contains(Point::new(10.0, 10.0)));
        assert!(!left.contains_strictly(Point::new(10.0, 10.0)));
    }

    #[test]
    fn a_port_sits_on_its_side_and_cannot_leave_the_card() {
        let card = Rect::new(Point::new(100.0, 50.0), Point::new(300.0, 150.0));
        assert_eq!(
            Port::new(0, Side::Right, 30.0).point_on(&card),
            Point::new(300.0, 80.0)
        );
        assert_eq!(
            Port::new(0, Side::Left, 30.0).point_on(&card),
            Point::new(100.0, 80.0)
        );
        assert_eq!(
            Port::new(0, Side::Bottom, 40.0).point_on(&card),
            Point::new(140.0, 150.0)
        );
        // An offset beyond the side is clamped rather than trusted.
        assert_eq!(
            Port::new(0, Side::Right, 400.0).point_on(&card),
            Point::new(300.0, 150.0)
        );
    }

    #[test]
    fn bounding_covers_every_rectangle() {
        let bounds = Rect::bounding([
            Rect::from_size(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
            Rect::from_size(Point::new(50.0, -20.0), Size::new(10.0, 10.0)),
        ])
        .unwrap();
        assert_eq!(bounds.min, Point::new(0.0, -20.0));
        assert_eq!(bounds.max, Point::new(60.0, 10.0));
        assert_eq!(Rect::bounding([]), None);
    }

    #[test]
    fn comparison_is_total_even_with_a_nan() {
        let mut values = [1.0, f32::NAN, -1.0];
        values.sort_by(|a, b| compare(*a, *b));
        assert_eq!(values[0], -1.0);
        assert_eq!(values[1], 1.0);
        assert!(values[2].is_nan());
    }
}

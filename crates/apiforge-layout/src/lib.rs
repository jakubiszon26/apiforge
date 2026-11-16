//! Layered automatic layout and orthogonal wire routing, as pure geometry.
//!
//! The crate knows nothing about documents, blocks or a renderer: its input is
//! rectangles with ports and edges, and its output is positions and polylines. That
//! keeps the algorithms testable with generated geometry and reusable by any
//! renderer. [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md)
//! records the algorithms and the guarantees.
mod geometry;
mod layered;
mod order;
mod packed;
mod route;
mod smooth;

pub use geometry::{Block, Edge, Point, Port, Rect, Side, Size};
pub use layered::{
    Graph, Hanging, Room, Row, Rows, Spacing, any_overlap, columns_of, hang, hang_with, layers_of,
    make_room, place, rows_of, tidy, widen,
};
pub use packed::{Breach, GAP, Near, packed};
pub use route::{
    Bridge, Route, Routing, Wire, intersection as crossing, route, route_among, route_reserving,
    under_own, violations,
};
pub use smooth::{DEFAULT_RADIUS, Piece, smooth, violations as smooth_violations};

//! Small, dependency-free geometry primitives for a 2D design canvas.
//!
//! Coordinates are expressed in document space unless a caller explicitly
//! transforms them. Rectangles use a top-left origin with positive width and
//! height. The types deliberately use `f32`, which maps directly to common
//! GPU vertex formats and the editor's document model.

use core::ops::{Add, AddAssign, Sub, SubAssign};

/// A two-dimensional point or vector in canvas coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate or component.
    pub x: f32,
    /// Vertical coordinate or component.
    pub y: f32,
}

impl Point {
    /// Creates a point from its coordinates.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The origin vector `(0, 0)`.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Returns whether both components are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Returns the Euclidean distance to `other`.
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Returns the vector length.
    #[must_use]
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}
impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}
impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

/// An axis-aligned rectangle with a top-left origin and non-negative size.
///
/// The fields are public for convenient interoperation with document models.
/// Methods that construct or modify rectangles reject invalid values by
/// returning `None`; methods on directly-created invalid values remain total
/// and return `false` or an empty result where possible.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left coordinate.
    pub x: f32,
    /// Top coordinate.
    pub y: f32,
    /// Width (expected to be non-negative).
    pub width: f32,
    /// Height (expected to be non-negative).
    pub height: f32,
}

impl Rect {
    /// Creates a rectangle. This is a const constructor and does not validate
    /// its arguments; use [`Rect::try_new`] for untrusted input.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a rectangle only when its origin and dimensions are finite and
    /// its dimensions are non-negative.
    #[must_use]
    pub fn try_new(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let rect = Self::new(x, y, width, height);
        rect.is_valid().then_some(rect)
    }

    /// Returns the smallest rectangle enclosing two points, regardless of
    /// their order.
    #[must_use]
    pub fn from_points(a: Point, b: Point) -> Option<Self> {
        (a.is_finite() && b.is_finite()).then(|| {
            Self::new(
                a.x.min(b.x),
                a.y.min(b.y),
                (a.x - b.x).abs(),
                (a.y - b.y).abs(),
            )
        })
    }

    /// Returns whether the rectangle has finite coordinates and non-negative
    /// dimensions.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height >= 0.0
    }

    /// Left edge.
    #[must_use]
    pub fn left(self) -> f32 {
        self.x
    }
    /// Top edge.
    #[must_use]
    pub fn top(self) -> f32 {
        self.y
    }
    /// Right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }
    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
    /// Horizontal midpoint.
    #[must_use]
    pub fn center_x(self) -> f32 {
        self.x + self.width * 0.5
    }
    /// Vertical midpoint.
    #[must_use]
    pub fn center_y(self) -> f32 {
        self.y + self.height * 0.5
    }
    /// Center point.
    #[must_use]
    pub fn center(self) -> Point {
        Point::new(self.center_x(), self.center_y())
    }
    /// Rectangle area.
    #[must_use]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    /// Returns true when `point` is inside or on the rectangle boundary.
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        self.is_valid()
            && point.is_finite()
            && point.x >= self.left()
            && point.x <= self.right()
            && point.y >= self.top()
            && point.y <= self.bottom()
    }

    /// Returns true when two valid rectangles overlap or touch at an edge.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.is_valid()
            && other.is_valid()
            && self.left() <= other.right()
            && self.right() >= other.left()
            && self.top() <= other.bottom()
            && self.bottom() >= other.top()
    }

    /// Returns the intersection, including zero-area edge or point contact.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        Some(Self::new(
            left,
            top,
            self.right().min(other.right()) - left,
            self.bottom().min(other.bottom()) - top,
        ))
    }

    /// Returns the smallest rectangle containing both valid rectangles.
    #[must_use]
    pub fn union(self, other: Self) -> Option<Self> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        Some(Self::new(
            left,
            top,
            self.right().max(other.right()) - left,
            self.bottom().max(other.bottom()) - top,
        ))
    }

    /// Returns a rectangle translated by `delta`.
    #[must_use]
    pub fn translated(self, delta: Point) -> Option<Self> {
        if !self.is_valid() || !delta.is_finite() {
            return None;
        }
        Self::try_new(self.x + delta.x, self.y + delta.y, self.width, self.height)
    }

    /// Returns this rectangle expanded on all sides by `amount`. Negative
    /// amounts shrink it and are accepted only while dimensions stay valid.
    #[must_use]
    pub fn expanded(self, amount: f32) -> Option<Self> {
        if !self.is_valid() || !amount.is_finite() {
            return None;
        }
        Self::try_new(
            self.x - amount,
            self.y - amount,
            self.width + 2.0 * amount,
            self.height + 2.0 * amount,
        )
    }

    /// Returns the rectangle's four corners in clockwise order, starting at
    /// the top-left.
    #[must_use]
    pub fn corners(self) -> Option<[Point; 4]> {
        self.is_valid().then(|| {
            [
                Point::new(self.left(), self.top()),
                Point::new(self.right(), self.top()),
                Point::new(self.right(), self.bottom()),
                Point::new(self.left(), self.bottom()),
            ]
        })
    }
}

/// A 2D affine transform represented as a 2×2 linear part and translation.
///
/// Points are transformed as `x' = m11*x + m12*y + tx` and
/// `y' = m21*x + m22*y + ty`. Composition via [`Transform::then`] applies
/// `self` first and `next` second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// X output from the input X component.
    pub m11: f32,
    /// X output from the input Y component.
    pub m12: f32,
    /// Y output from the input X component.
    pub m21: f32,
    /// Y output from the input Y component.
    pub m22: f32,
    /// X translation.
    pub tx: f32,
    /// Y translation.
    pub ty: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// Identity transform.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    /// Creates an affine transform from its matrix coefficients.
    #[must_use]
    pub const fn new(m11: f32, m12: f32, m21: f32, m22: f32, tx: f32, ty: f32) -> Self {
        Self {
            m11,
            m12,
            m21,
            m22,
            tx,
            ty,
        }
    }

    /// Creates a translation transform.
    #[must_use]
    pub const fn translation(x: f32, y: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, x, y)
    }

    /// Creates a non-uniform scale transform.
    #[must_use]
    pub const fn scale(x: f32, y: f32) -> Self {
        Self::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    /// Creates a rotation around the origin in radians.
    #[must_use]
    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self::new(cos, -sin, sin, cos, 0.0, 0.0)
    }

    /// Returns whether every coefficient is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.m11.is_finite()
            && self.m12.is_finite()
            && self.m21.is_finite()
            && self.m22.is_finite()
            && self.tx.is_finite()
            && self.ty.is_finite()
    }

    /// Transforms a point. Returns `None` if the transform, input, or output
    /// is non-finite.
    #[must_use]
    pub fn transform_point(self, point: Point) -> Option<Point> {
        if !self.is_finite() || !point.is_finite() {
            return None;
        }
        let result = Point::new(
            self.m11 * point.x + self.m12 * point.y + self.tx,
            self.m21 * point.x + self.m22 * point.y + self.ty,
        );
        result.is_finite().then_some(result)
    }

    /// Transforms an axis-aligned rectangle and returns the axis-aligned bounds
    /// enclosing its four transformed corners.
    #[must_use]
    pub fn transform_rect(self, rect: Rect) -> Option<Rect> {
        let corners = rect.corners()?;
        let points = corners
            .map(|point| self.transform_point(point))
            .into_iter()
            .collect::<Option<Vec<_>>>()?;
        let (mut left, mut top, mut right, mut bottom) =
            (points[0].x, points[0].y, points[0].x, points[0].y);
        for point in &points[1..] {
            left = left.min(point.x);
            top = top.min(point.y);
            right = right.max(point.x);
            bottom = bottom.max(point.y);
        }
        Rect::try_new(left, top, right - left, bottom - top)
    }

    /// Returns the inverse transform, or `None` for non-finite or singular
    /// matrices.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        if !self.is_finite() {
            return None;
        }
        let determinant = self.m11 * self.m22 - self.m12 * self.m21;
        if !determinant.is_finite() || determinant == 0.0 {
            return None;
        }
        let inv_det = 1.0 / determinant;
        let m11 = self.m22 * inv_det;
        let m12 = -self.m12 * inv_det;
        let m21 = -self.m21 * inv_det;
        let m22 = self.m11 * inv_det;
        let inverse = Self::new(
            m11,
            m12,
            m21,
            m22,
            -(m11 * self.tx + m12 * self.ty),
            -(m21 * self.tx + m22 * self.ty),
        );
        inverse.is_finite().then_some(inverse)
    }

    /// Composes this transform followed by `next`.
    #[must_use]
    pub fn then(self, next: Self) -> Option<Self> {
        if !self.is_finite() || !next.is_finite() {
            return None;
        }
        let result = Self::new(
            next.m11 * self.m11 + next.m12 * self.m21,
            next.m11 * self.m12 + next.m12 * self.m22,
            next.m21 * self.m11 + next.m22 * self.m21,
            next.m21 * self.m12 + next.m22 * self.m22,
            next.m11 * self.tx + next.m12 * self.ty + next.tx,
            next.m21 * self.tx + next.m22 * self.ty + next.ty,
        );
        result.is_finite().then_some(result)
    }
}

/// Horizontal or vertical axis used by snapping and layout operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// A rectangle anchor used as a snap point or alignment target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Center,
    End,
}

/// A detected snap between one moving rectangle anchor and a reference anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapCandidate {
    /// The axis along which the snap occurs.
    pub axis: Axis,
    /// Anchor on the moving rectangle.
    pub moving_anchor: Anchor,
    /// Anchor on the reference rectangle.
    pub target_anchor: Anchor,
    /// Signed displacement to apply on `axis` to make the anchors coincide.
    pub delta: f32,
    /// Coordinate at which the alignment guide is drawn.
    pub coordinate: f32,
    /// The line segment for the guide in document coordinates.
    pub guide_start: Point,
    /// The line segment for the guide in document coordinates.
    pub guide_end: Point,
}

/// Returns all anchor-to-anchor snap candidates within `tolerance`.
///
/// Candidates are sorted by absolute displacement, then horizontal before
/// vertical, then anchor order. Invalid rectangles, non-finite tolerance, or a
/// negative tolerance produce an empty vector.
#[must_use]
pub fn snap_candidates(moving: Rect, references: &[Rect], tolerance: f32) -> Vec<SnapCandidate> {
    if !moving.is_valid() || !tolerance.is_finite() || tolerance < 0.0 {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    for reference in references.iter().copied().filter(|rect| rect.is_valid()) {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let moving_bounds = axis_bounds(moving, axis);
            let target_bounds = axis_bounds(reference, axis);
            for moving_anchor in [Anchor::Start, Anchor::Center, Anchor::End] {
                let moving_value = anchor_value(moving_bounds, moving_anchor);
                for target_anchor in [Anchor::Start, Anchor::Center, Anchor::End] {
                    let target_value = anchor_value(target_bounds, target_anchor);
                    let delta = target_value - moving_value;
                    if delta.abs() <= tolerance {
                        let coordinate = target_value;
                        let (guide_start, guide_end) = match axis {
                            Axis::Horizontal => {
                                let start = moving.top().min(reference.top());
                                let end = moving.bottom().max(reference.bottom());
                                (Point::new(coordinate, start), Point::new(coordinate, end))
                            }
                            Axis::Vertical => {
                                let start = moving.left().min(reference.left());
                                let end = moving.right().max(reference.right());
                                (Point::new(start, coordinate), Point::new(end, coordinate))
                            }
                        };
                        candidates.push(SnapCandidate {
                            axis,
                            moving_anchor,
                            target_anchor,
                            delta,
                            coordinate,
                            guide_start,
                            guide_end,
                        });
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.delta
            .abs()
            .total_cmp(&b.delta.abs())
            .then_with(|| axis_order(a.axis).cmp(&axis_order(b.axis)))
            .then_with(|| anchor_order(a.moving_anchor).cmp(&anchor_order(b.moving_anchor)))
            .then_with(|| anchor_order(a.target_anchor).cmp(&anchor_order(b.target_anchor)))
    });
    candidates
}

/// Returns the first (closest, deterministic) snap candidate, if any.
#[must_use]
pub fn best_snap(moving: Rect, references: &[Rect], tolerance: f32) -> Option<SnapCandidate> {
    snap_candidates(moving, references, tolerance)
        .into_iter()
        .next()
}

/// Alignment operations for a selection of rectangles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// Align every rectangle's left edge to the selection's left edge.
    Left,
    /// Align every rectangle's horizontal center to the selection's center.
    HorizontalCenter,
    /// Align every rectangle's right edge to the selection's right edge.
    Right,
    /// Align every rectangle's top edge to the selection's top edge.
    Top,
    /// Align every rectangle's vertical center to the selection's center.
    VerticalCenter,
    /// Align every rectangle's bottom edge to the selection's bottom edge.
    Bottom,
}

/// Returns per-rectangle translations that align all rectangles to their
/// collective bounds. The output has the same order and length as the input.
/// Invalid input returns `None`; an empty selection returns an empty vector.
#[must_use]
pub fn alignment_offsets(rects: &[Rect], alignment: Alignment) -> Option<Vec<Point>> {
    if rects.iter().any(|rect| !rect.is_valid()) {
        return None;
    }
    let Some(Some(bounds)) = bounds(rects) else {
        return rects.is_empty().then(Vec::new);
    };
    Some(
        rects
            .iter()
            .map(|rect| match alignment {
                Alignment::Left => Point::new(bounds.left() - rect.left(), 0.0),
                Alignment::HorizontalCenter => Point::new(bounds.center_x() - rect.center_x(), 0.0),
                Alignment::Right => Point::new(bounds.right() - rect.right(), 0.0),
                Alignment::Top => Point::new(0.0, bounds.top() - rect.top()),
                Alignment::VerticalCenter => Point::new(0.0, bounds.center_y() - rect.center_y()),
                Alignment::Bottom => Point::new(0.0, bounds.bottom() - rect.bottom()),
            })
            .collect(),
    )
}

/// Returns per-rectangle translations that distribute rectangles evenly while
/// preserving the positions of the first and last items along `axis`.
///
/// Items are ordered by their start coordinate, with original input order used
/// to break ties. Fewer than three rectangles need no distribution and return
/// zero offsets. Invalid input returns `None`.
#[must_use]
pub fn distribution_offsets(rects: &[Rect], axis: Axis) -> Option<Vec<Point>> {
    if rects.iter().any(|rect| !rect.is_valid()) {
        return None;
    }
    let mut offsets = vec![Point::ZERO; rects.len()];
    if rects.len() < 3 {
        return Some(offsets);
    }

    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|&a, &b| {
        start_value(rects[a], axis)
            .total_cmp(&start_value(rects[b], axis))
            .then_with(|| a.cmp(&b))
    });
    let first = rects[order[0]];
    let last = rects[*order.last()?];
    let span = end_value(last, axis) - start_value(first, axis);
    let total_size: f32 = order
        .iter()
        .map(|&index| size_value(rects[index], axis))
        .sum();
    let gap = (span - total_size) / (rects.len() - 1) as f32;
    if !gap.is_finite() {
        return None;
    }

    let mut cursor = start_value(first, axis);
    for index in order {
        let rect = rects[index];
        let delta = cursor - start_value(rect, axis);
        offsets[index] = match axis {
            Axis::Horizontal => Point::new(delta, 0.0),
            Axis::Vertical => Point::new(0.0, delta),
        };
        cursor += size_value(rect, axis) + gap;
    }
    Some(offsets)
}

/// Returns the axis-aligned bounding rectangle of valid rectangles.
/// Empty input returns `Some(None)`; invalid input returns `None`.
#[must_use]
pub fn bounds(rects: &[Rect]) -> Option<Option<Rect>> {
    let mut iter = rects.iter().copied();
    let Some(first) = iter.next() else {
        return Some(None);
    };
    if !first.is_valid() {
        return None;
    }
    let mut result = first;
    for rect in iter {
        result = result.union(rect)?;
    }
    Some(Some(result))
}

fn axis_bounds(rect: Rect, axis: Axis) -> (f32, f32) {
    match axis {
        Axis::Horizontal => (rect.left(), rect.right()),
        Axis::Vertical => (rect.top(), rect.bottom()),
    }
}
fn anchor_value(bounds: (f32, f32), anchor: Anchor) -> f32 {
    match anchor {
        Anchor::Start => bounds.0,
        Anchor::Center => (bounds.0 + bounds.1) * 0.5,
        Anchor::End => bounds.1,
    }
}
fn axis_order(axis: Axis) -> u8 {
    match axis {
        Axis::Horizontal => 0,
        Axis::Vertical => 1,
    }
}
fn anchor_order(anchor: Anchor) -> u8 {
    match anchor {
        Anchor::Start => 0,
        Anchor::Center => 1,
        Anchor::End => 2,
    }
}
fn start_value(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.left(),
        Axis::Vertical => rect.top(),
    }
}
fn end_value(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.right(),
        Axis::Vertical => rect.bottom(),
    }
}
fn size_value(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.width,
        Axis::Vertical => rect.height,
    }
}

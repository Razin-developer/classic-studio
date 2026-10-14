//! Layout, responsive resizing, and prototype interaction primitives for Classic.
//!
//! The crate is UI- and renderer-independent. It does not own document nodes or
//! execute prototype actions; callers resolve the string IDs against their own
//! document model and use the returned rectangles when rendering.

use serde::{Deserialize, Serialize};

/// A rectangle expressed in the coordinate space of its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Constructs a rectangle without clamping its dimensions.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Insets applied around the children of a stack container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    /// Creates uniform padding on all four sides.
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Main axis used by an auto-layout stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackAxis {
    Horizontal,
    Vertical,
}

/// Alignment of children on the axis perpendicular to the stack direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossAxisAlignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Placement of children along the stack's main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainAxisDistribution {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// How a child obtains its size along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode", content = "value")]
pub enum DimensionSizing {
    /// Use the child's preferred dimension.
    #[default]
    Fixed,
    /// Divide remaining stack space proportionally with other fill children.
    Fill,
    /// Use the child's content-measured preferred dimension.
    Hug,
}

/// Preferred child dimensions and sizing rules passed to [`solve_stack`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackChild {
    pub id: u64,
    pub preferred_width: f32,
    pub preferred_height: f32,
    pub width_sizing: DimensionSizing,
    pub height_sizing: DimensionSizing,
    pub min_width: f32,
    pub min_height: f32,
    /// Optional maximum width. `None` means unbounded.
    pub max_width: Option<f32>,
    /// Optional maximum height. `None` means unbounded.
    pub max_height: Option<f32>,
}

impl StackChild {
    /// Creates a fixed-size child with no minimum or maximum beyond zero.
    pub const fn fixed(id: u64, width: f32, height: f32) -> Self {
        Self {
            id,
            preferred_width: width,
            preferred_height: height,
            width_sizing: DimensionSizing::Fixed,
            height_sizing: DimensionSizing::Fixed,
            min_width: 0.0,
            min_height: 0.0,
            max_width: None,
            max_height: None,
        }
    }
}

/// Configuration for a horizontal or vertical auto-layout stack.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackLayout {
    pub axis: StackAxis,
    pub gap: f32,
    pub padding: Insets,
    pub cross_alignment: CrossAxisAlignment,
    pub distribution: MainAxisDistribution,
}

impl Default for StackLayout {
    fn default() -> Self {
        Self {
            axis: StackAxis::Vertical,
            gap: 0.0,
            padding: Insets::default(),
            cross_alignment: CrossAxisAlignment::Start,
            distribution: MainAxisDistribution::Start,
        }
    }
}

/// A child rectangle produced by [`solve_stack`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionedChild {
    pub id: u64,
    pub rect: Rect,
}

/// Solves child rectangles for a stack in the container's local coordinates.
///
/// Negative gaps and padding are treated as zero. Preferred sizes and bounds
/// are sanitized to finite, non-negative values. Fill children share remaining
/// main-axis space proportionally (equal shares); any space left after maximum
/// constraints is handled by the configured distribution. When preferred
/// fixed sizes exceed the available space, children retain their preferred
/// sizes and the result may overflow the container.
pub fn solve_stack(
    container: Rect,
    layout: StackLayout,
    children: &[StackChild],
) -> Vec<PositionedChild> {
    if children.is_empty() {
        return Vec::new();
    }

    let horizontal = layout.axis == StackAxis::Horizontal;
    let main_extent = if horizontal {
        container.width
    } else {
        container.height
    }
    .max(0.0);
    let cross_extent = if horizontal {
        container.height
    } else {
        container.width
    }
    .max(0.0);
    let leading = if horizontal {
        layout.padding.left
    } else {
        layout.padding.top
    }
    .max(0.0);
    let trailing = if horizontal {
        layout.padding.right
    } else {
        layout.padding.bottom
    }
    .max(0.0);
    let cross_leading = if horizontal {
        layout.padding.top
    } else {
        layout.padding.left
    }
    .max(0.0);
    let cross_trailing = if horizontal {
        layout.padding.bottom
    } else {
        layout.padding.right
    }
    .max(0.0);
    let inner_main = (main_extent - leading - trailing).max(0.0);
    let inner_cross = (cross_extent - cross_leading - cross_trailing).max(0.0);
    let gap = finite_nonnegative(layout.gap);
    let gaps_total = gap * children.len().saturating_sub(1) as f32;
    let available_for_children = (inner_main - gaps_total).max(0.0);

    let mut main_sizes: Vec<f32> = children
        .iter()
        .map(|child| {
            let (preferred, sizing, minimum, maximum) = if horizontal {
                (
                    child.preferred_width,
                    child.width_sizing,
                    child.min_width,
                    child.max_width,
                )
            } else {
                (
                    child.preferred_height,
                    child.height_sizing,
                    child.min_height,
                    child.max_height,
                )
            };
            if sizing == DimensionSizing::Fill {
                0.0
            } else {
                clamp_dimension(preferred, minimum, maximum)
            }
        })
        .collect();

    let fixed_total: f32 = main_sizes.iter().sum();
    let fill_count = children
        .iter()
        .enumerate()
        .filter(|(index, child)| {
            let sizing = if horizontal {
                child.width_sizing
            } else {
                child.height_sizing
            };
            sizing == DimensionSizing::Fill && main_sizes[*index] == 0.0
        })
        .count();
    if fill_count > 0 {
        let share = ((available_for_children - fixed_total).max(0.0)) / fill_count as f32;
        for (index, child) in children.iter().enumerate() {
            let sizing = if horizontal {
                child.width_sizing
            } else {
                child.height_sizing
            };
            if sizing == DimensionSizing::Fill && main_sizes[index] == 0.0 {
                let (minimum, maximum) = if horizontal {
                    (child.min_width, child.max_width)
                } else {
                    (child.min_height, child.max_height)
                };
                main_sizes[index] = clamp_dimension(share, minimum, maximum);
            }
        }
    }

    let occupied = main_sizes.iter().sum::<f32>() + gaps_total;
    let free_space = (inner_main - occupied).max(0.0);
    let (start_offset, extra_gap) = match layout.distribution {
        MainAxisDistribution::Start => (0.0, 0.0),
        MainAxisDistribution::Center => (free_space * 0.5, 0.0),
        MainAxisDistribution::End => (free_space, 0.0),
        MainAxisDistribution::SpaceBetween if children.len() > 1 => {
            (0.0, free_space / (children.len() - 1) as f32)
        }
        MainAxisDistribution::SpaceAround => {
            let unit = free_space / children.len() as f32;
            (unit * 0.5, unit)
        }
        MainAxisDistribution::SpaceEvenly => {
            let unit = free_space / (children.len() + 1) as f32;
            (unit, unit)
        }
        _ => (0.0, 0.0),
    };

    let mut cursor = leading + start_offset;
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let preferred_cross = if horizontal {
                child.preferred_height
            } else {
                child.preferred_width
            };
            let cross_sizing = if horizontal {
                child.height_sizing
            } else {
                child.width_sizing
            };
            let cross_min = if horizontal {
                child.min_height
            } else {
                child.min_width
            };
            let cross_max = if horizontal {
                child.max_height
            } else {
                child.max_width
            };
            let cross_size = if layout.cross_alignment == CrossAxisAlignment::Stretch
                || cross_sizing == DimensionSizing::Fill
            {
                clamp_dimension(inner_cross, cross_min, cross_max)
            } else {
                clamp_dimension(preferred_cross, cross_min, cross_max)
            };
            let cross_free = (inner_cross - cross_size).max(0.0);
            let cross_offset = match layout.cross_alignment {
                CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => 0.0,
                CrossAxisAlignment::Center => cross_free * 0.5,
                CrossAxisAlignment::End => cross_free,
            };
            let main_size = main_sizes[index];
            let rect = if horizontal {
                Rect::new(
                    container.x + cursor,
                    container.y + cross_leading + cross_offset,
                    main_size,
                    cross_size,
                )
            } else {
                Rect::new(
                    container.x + cross_leading + cross_offset,
                    container.y + cursor,
                    cross_size,
                    main_size,
                )
            };
            cursor += main_size + gap + extra_gap;
            PositionedChild { id: child.id, rect }
        })
        .collect()
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn clamp_dimension(value: f32, minimum: f32, maximum: Option<f32>) -> f32 {
    let min = finite_nonnegative(minimum);
    let preferred = finite_nonnegative(value).max(min);
    match maximum.filter(|limit| limit.is_finite()) {
        Some(limit) => preferred.min(finite_nonnegative(limit).max(min)),
        None => preferred,
    }
}

/// Responsive anchoring rule for one axis of a child in its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisConstraint {
    /// Keep the distance from the parent's leading edge.
    #[default]
    Start,
    /// Keep the distance from the parent's trailing edge.
    End,
    /// Keep the child's center at the same relative offset from the parent center.
    Center,
    /// Scale position and size by the parent's size ratio.
    Scale,
    /// Preserve both edge insets, expanding or shrinking the child with its parent.
    Stretch,
}

/// Horizontal and vertical resize behavior for a child frame or layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeConstraints {
    pub horizontal: AxisConstraint,
    pub vertical: AxisConstraint,
}

/// Applies resize constraints from an old parent rectangle to a resized parent.
///
/// The child rectangle is expected in the old parent's local coordinate space;
/// the returned rectangle is in the new parent's local coordinate space.
pub fn apply_constraints(
    child: Rect,
    old_parent: Rect,
    new_parent: Rect,
    constraints: ResizeConstraints,
) -> Rect {
    let (x, width) = constrain_axis(
        child.x,
        child.width,
        old_parent.width,
        new_parent.width,
        constraints.horizontal,
    );
    let (y, height) = constrain_axis(
        child.y,
        child.height,
        old_parent.height,
        new_parent.height,
        constraints.vertical,
    );
    Rect::new(x, y, width, height)
}

fn constrain_axis(
    position: f32,
    size: f32,
    old_extent: f32,
    new_extent: f32,
    constraint: AxisConstraint,
) -> (f32, f32) {
    let old_extent = finite_nonnegative(old_extent);
    let new_extent = finite_nonnegative(new_extent);
    let position = if position.is_finite() { position } else { 0.0 };
    let size = finite_nonnegative(size);
    let delta = new_extent - old_extent;
    match constraint {
        AxisConstraint::Start => (position, size),
        AxisConstraint::End => (position + delta, size),
        AxisConstraint::Center => (position + delta * 0.5, size),
        AxisConstraint::Scale => {
            if old_extent == 0.0 {
                (position, size)
            } else {
                let ratio = new_extent / old_extent;
                (position * ratio, size * ratio)
            }
        }
        AxisConstraint::Stretch => (position, (size + delta).max(0.0)),
    }
}

/// A directed link from one frame to another in a prototype flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameLink {
    pub id: String,
    pub source_frame_id: String,
    pub destination_frame_id: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// Input event that can start a prototype interaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum Trigger {
    OnClick,
    OnHover,
    OnPress,
    OnDrag,
    OnKeyPress(String),
    AfterDelay { milliseconds: u32 },
}

/// Navigation or presentation effect run when a trigger fires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum Action {
    NavigateTo { frame_id: String },
    Back,
    OpenOverlay { frame_id: String },
    CloseOverlay,
    ScrollTo { node_id: String },
    SetVariable { name: String, value: String },
}

/// Direction used by push and move transitions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionDirection {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

/// Transition animation between prototype states.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Transition {
    Instant,
    Dissolve {
        duration_ms: u32,
    },
    SmartAnimate {
        duration_ms: u32,
    },
    MoveIn {
        direction: TransitionDirection,
        duration_ms: u32,
    },
    MoveOut {
        direction: TransitionDirection,
        duration_ms: u32,
    },
    Push {
        direction: TransitionDirection,
        duration_ms: u32,
    },
    Slide {
        direction: TransitionDirection,
        duration_ms: u32,
    },
}

impl Default for Transition {
    fn default() -> Self {
        Self::Instant
    }
}

/// Interaction attached to a node or frame in a prototype flow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub id: String,
    /// Node that receives the trigger. Use a frame ID for frame-level triggers.
    pub source_node_id: String,
    pub trigger: Trigger,
    pub action: Action,
    #[serde(default)]
    pub transition: Transition,
}

/// Serializable graph of frame links and trigger-action interactions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrototypeGraph {
    #[serde(default)]
    pub frame_links: Vec<FrameLink>,
    #[serde(default)]
    pub interactions: Vec<Interaction>,
}

impl PrototypeGraph {
    /// Adds a frame-to-frame link.
    pub fn add_frame_link(&mut self, link: FrameLink) {
        self.frame_links.push(link);
    }

    /// Adds a trigger-action interaction.
    pub fn add_interaction(&mut self, interaction: Interaction) {
        self.interactions.push(interaction);
    }

    /// Returns all interactions attached to the requested node or frame ID.
    pub fn interactions_for<'a>(
        &'a self,
        source_node_id: &'a str,
    ) -> impl Iterator<Item = &'a Interaction> + 'a {
        self.interactions
            .iter()
            .filter(move |interaction| interaction.source_node_id == source_node_id)
    }
}

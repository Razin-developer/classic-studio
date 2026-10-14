//! Serializable ruler guides and deterministic snapping for a 2D canvas.
//!
//! The types in this crate are renderer-independent. Coordinates are document
//! coordinates, expressed in logical pixels. Horizontal guides constrain the
//! `y` axis; vertical guides constrain the `x` axis.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// The axis constrained by a ruler guide.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideAxis {
    /// A guide running left to right at a fixed vertical coordinate.
    #[default]
    Horizontal,
    /// A guide running top to bottom at a fixed horizontal coordinate.
    Vertical,
}

impl GuideAxis {
    /// Returns the coordinate component used by this axis.
    #[must_use]
    pub const fn coordinate_name(self) -> &'static str {
        match self {
            Self::Horizontal => "y",
            Self::Vertical => "x",
        }
    }
}

/// A named, optionally locked ruler guide.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulerGuide {
    /// Stable document-local identifier.
    pub id: String,
    /// The line's orientation.
    pub axis: GuideAxis,
    /// The constrained document coordinate.
    pub position: f32,
    /// Whether the editor should prevent accidental moves or removal.
    #[serde(default)]
    pub locked: bool,
    /// Optional human-readable label for the guide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether this guide is visible and eligible for snapping.
    #[serde(default = "default_visible")]
    pub visible: bool,
}

const fn default_visible() -> bool {
    true
}

impl RulerGuide {
    /// Creates a visible, unlocked guide with a generated identifier.
    #[must_use]
    pub fn new(axis: GuideAxis, position: f32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            axis,
            position,
            locked: false,
            name: None,
            visible: true,
        }
    }

    /// Returns an error when this guide cannot be safely represented in a document.
    pub fn validate(&self) -> Result<(), GuideValidationError> {
        if self.id.trim().is_empty() {
            return Err(GuideValidationError::EmptyId);
        }
        if !self.position.is_finite() {
            return Err(GuideValidationError::NonFinitePosition {
                guide_id: self.id.clone(),
                position: self.position,
            });
        }
        if self
            .name
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(GuideValidationError::BlankName {
                guide_id: self.id.clone(),
            });
        }
        Ok(())
    }
}

/// A serializable collection of document ruler guides.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideStore {
    /// The guides in front-to-back display order.
    #[serde(default)]
    pub guides: Vec<RulerGuide>,
    /// Whether guides should be rendered by the editor.
    #[serde(default = "default_visible")]
    pub visible: bool,
    /// Whether visible guides should affect snapping.
    #[serde(default = "default_snap_enabled")]
    pub snap_enabled: bool,
}

const fn default_snap_enabled() -> bool {
    true
}

impl GuideStore {
    /// Creates an empty guide collection with display and snapping enabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            guides: Vec::new(),
            visible: true,
            snap_enabled: true,
        }
    }

    /// Adds a guide after validating it and checking identifier uniqueness.
    pub fn insert(&mut self, guide: RulerGuide) -> Result<(), GuideStoreError> {
        guide.validate().map_err(GuideStoreError::InvalidGuide)?;
        if self.guides.iter().any(|existing| existing.id == guide.id) {
            return Err(GuideStoreError::DuplicateId(guide.id));
        }
        self.guides.push(guide);
        Ok(())
    }

    /// Creates, stores, and returns a new guide.
    pub fn add(&mut self, axis: GuideAxis, position: f32) -> Result<&RulerGuide, GuideStoreError> {
        let guide = RulerGuide::new(axis, position);
        self.insert(guide)?;
        Ok(self.guides.last().expect("a guide was inserted"))
    }

    /// Returns a guide by stable identifier.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RulerGuide> {
        self.guides.iter().find(|guide| guide.id == id)
    }

    /// Returns a mutable guide by stable identifier.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut RulerGuide> {
        self.guides.iter_mut().find(|guide| guide.id == id)
    }

    /// Moves a guide unless it is locked.
    pub fn move_to(&mut self, id: &str, position: f32) -> Result<(), GuideStoreError> {
        if !position.is_finite() {
            return Err(GuideStoreError::InvalidGuide(
                GuideValidationError::NonFinitePosition {
                    guide_id: id.to_owned(),
                    position,
                },
            ));
        }
        let guide = self
            .get_mut(id)
            .ok_or_else(|| GuideStoreError::MissingId(id.to_owned()))?;
        if guide.locked {
            return Err(GuideStoreError::Locked(id.to_owned()));
        }
        guide.position = position;
        Ok(())
    }

    /// Removes a guide unless it is locked, returning the removed model.
    pub fn remove(&mut self, id: &str) -> Result<RulerGuide, GuideStoreError> {
        let index = self
            .guides
            .iter()
            .position(|guide| guide.id == id)
            .ok_or_else(|| GuideStoreError::MissingId(id.to_owned()))?;
        if self.guides[index].locked {
            return Err(GuideStoreError::Locked(id.to_owned()));
        }
        Ok(self.guides.remove(index))
    }

    /// Returns visible guide snap candidates when guide snapping is enabled.
    #[must_use]
    pub fn snap_candidates(&self) -> impl Iterator<Item = GuideSnapCandidate<'_>> {
        self.guides.iter().filter_map(move |guide| {
            (self.snap_enabled && guide.visible).then_some(GuideSnapCandidate {
                guide_id: &guide.id,
                axis: guide.axis,
                position: guide.position,
            })
        })
    }

    /// Validates every guide and rejects duplicate identifiers.
    pub fn validate(&self) -> Result<(), GuideStoreError> {
        let mut ids = std::collections::BTreeSet::new();
        for guide in &self.guides {
            guide.validate().map_err(GuideStoreError::InvalidGuide)?;
            if !ids.insert(&guide.id) {
                return Err(GuideStoreError::DuplicateId(guide.id.clone()));
            }
        }
        Ok(())
    }
}

/// A borrowed snapping target supplied by a guide store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideSnapCandidate<'a> {
    /// Source guide identifier.
    pub guide_id: &'a str,
    /// Axis to snap.
    pub axis: GuideAxis,
    /// Document coordinate of the guide.
    pub position: f32,
}

/// A snap hit produced for one candidate and one input coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideSnapHit<'a> {
    /// Winning guide target.
    pub candidate: GuideSnapCandidate<'a>,
    /// Original coordinate before snapping.
    pub input: f32,
    /// Signed change applied to the input coordinate.
    pub delta: f32,
}

impl GuideSnapHit<'_> {
    /// Returns the aligned coordinate.
    #[must_use]
    pub fn snapped_position(self) -> f32 {
        self.input + self.delta
    }
}

/// Finds the nearest candidate on `axis` within `threshold` document units.
///
/// Ties preserve input iteration order, making a stored guide order stable for
/// equal-distance targets.
#[must_use]
pub fn nearest_snap<'a>(
    candidates: impl IntoIterator<Item = GuideSnapCandidate<'a>>,
    axis: GuideAxis,
    input: f32,
    threshold: f32,
) -> Option<GuideSnapHit<'a>> {
    if !input.is_finite() || !threshold.is_finite() || threshold < 0.0 {
        return None;
    }

    candidates
        .into_iter()
        .filter(|candidate| candidate.axis == axis && candidate.position.is_finite())
        .filter_map(|candidate| {
            let delta = candidate.position - input;
            (delta.abs() <= threshold).then_some(GuideSnapHit {
                candidate,
                input,
                delta,
            })
        })
        .min_by(|left, right| left.delta.abs().total_cmp(&right.delta.abs()))
}

/// Describes invalid persisted guide data.
#[derive(Clone, Debug, PartialEq)]
pub enum GuideValidationError {
    /// Guide IDs must be present so mutations can target a stable guide.
    EmptyId,
    /// Guide position contained NaN or infinity.
    NonFinitePosition { guide_id: String, position: f32 },
    /// Optional names must contain visible text when present.
    BlankName { guide_id: String },
}

impl fmt::Display for GuideValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("guide ID cannot be empty"),
            Self::NonFinitePosition { guide_id, .. } => {
                write!(formatter, "guide '{guide_id}' has a non-finite position")
            }
            Self::BlankName { guide_id } => {
                write!(formatter, "guide '{guide_id}' has a blank name")
            }
        }
    }
}

impl std::error::Error for GuideValidationError {}

/// Errors from guide collection mutations and validation.
#[derive(Clone, Debug, PartialEq)]
pub enum GuideStoreError {
    /// The guide itself failed persistence validation.
    InvalidGuide(GuideValidationError),
    /// Two guides use the same stable identifier.
    DuplicateId(String),
    /// The requested guide was not found.
    MissingId(String),
    /// An operation would alter a locked guide.
    Locked(String),
}

impl fmt::Display for GuideStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGuide(error) => write!(formatter, "invalid guide: {error}"),
            Self::DuplicateId(id) => write!(formatter, "guide ID '{id}' already exists"),
            Self::MissingId(id) => write!(formatter, "guide '{id}' was not found"),
            Self::Locked(id) => write!(formatter, "guide '{id}' is locked"),
        }
    }
}

impl std::error::Error for GuideStoreError {}

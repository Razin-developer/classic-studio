//! Durable, local-first review-thread domain models.
//!
//! The crate deliberately contains no network transport, authentication, or UI concerns.
//! Callers own persistence and determine how a local [`Actor`] maps to a collaborating user.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COMMENT_SCHEMA_VERSION: u32 = 1;

macro_rules! identifier {
    ($name:ident, $prefix:literal) => {
        #[derive(
            Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(format!(concat!($prefix, "_{}"), Uuid::new_v4()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

identifier!(ThreadId, "thread");
identifier!(MessageId, "message");
identifier!(UserId, "user");

/// The minimal identity snapshot necessary to render authorship locally.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: UserId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl Actor {
    pub fn new(id: impl Into<UserId>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            avatar_url: None,
        }
    }
}

/// A location on a page that keeps a comment meaningful while canvas selection changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentAnchor {
    pub page_id: String,
    /// The target layer, when a comment belongs to a concrete canvas item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub region: AnchorRegion,
}

impl CommentAnchor {
    pub fn point(page_id: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            page_id: page_id.into(),
            node_id: None,
            region: AnchorRegion::Point { x, y },
        }
    }

    pub fn node(page_id: impl Into<String>, node_id: impl Into<String>) -> Self {
        Self {
            page_id: page_id.into(),
            node_id: Some(node_id.into()),
            region: AnchorRegion::Target,
        }
    }
}

/// Spatial detail associated with a thread's anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRegion {
    /// The whole target layer, or the page when `node_id` is absent.
    Target,
    Point {
        x: f32,
        y: f32,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

/// A mention extracted by the editor. Ranges use Unicode scalar indexes into message body text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    pub user: Actor,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl TextRange {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// One immutable conversational entry, with optional edit metadata for an edited body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentMessage {
    pub id: MessageId,
    pub author: Actor,
    pub body: String,
    /// Unix milliseconds, supplied by the host so the crate remains deterministic.
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
}

impl CommentMessage {
    pub fn new(author: Actor, body: impl Into<String>, created_at_ms: i64) -> Self {
        Self {
            id: MessageId::new(),
            author,
            body: body.into(),
            created_at_ms,
            edited_at_ms: None,
            mentions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Open,
    Resolved,
}

impl ThreadStatus {
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved)
    }
}

/// A locally persisted conversation associated with an area of a design document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThread {
    pub id: ThreadId,
    pub document_id: String,
    pub anchor: CommentAnchor,
    pub status: ThreadStatus,
    pub created_by: Actor,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at_ms: Option<i64>,
    pub messages: Vec<CommentMessage>,
}

impl ReviewThread {
    pub fn new(
        document_id: impl Into<String>,
        anchor: CommentAnchor,
        created_by: Actor,
        first_message: CommentMessage,
    ) -> Self {
        let created_at_ms = first_message.created_at_ms;
        Self {
            id: ThreadId::new(),
            document_id: document_id.into(),
            anchor,
            status: ThreadStatus::Open,
            created_by,
            created_at_ms,
            updated_at_ms: created_at_ms,
            resolved_by: None,
            resolved_at_ms: None,
            messages: vec![first_message],
        }
    }

    pub fn add_message(&mut self, message: CommentMessage) -> Result<(), ValidationErrors> {
        let mut candidate = self.clone();
        candidate.updated_at_ms = candidate.updated_at_ms.max(message.created_at_ms);
        candidate.messages.push(message);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn resolve(&mut self, actor: Actor, at_ms: i64) -> Result<(), ValidationErrors> {
        let mut candidate = self.clone();
        candidate.status = ThreadStatus::Resolved;
        candidate.resolved_by = Some(actor);
        candidate.resolved_at_ms = Some(at_ms);
        candidate.updated_at_ms = candidate.updated_at_ms.max(at_ms);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn reopen(&mut self, at_ms: i64) -> Result<(), ValidationErrors> {
        let mut candidate = self.clone();
        candidate.status = ThreadStatus::Open;
        candidate.resolved_by = None;
        candidate.resolved_at_ms = None;
        candidate.updated_at_ms = candidate.updated_at_ms.max(at_ms);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn is_resolved(&self) -> bool {
        self.status.is_resolved()
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        require_non_blank(&mut errors, "thread.id", self.id.as_str());
        require_non_blank(&mut errors, "thread.document_id", &self.document_id);
        require_non_blank(&mut errors, "anchor.page_id", &self.anchor.page_id);
        validate_anchor(&self.anchor, &mut errors);
        validate_actor(&self.created_by, "thread.created_by", &mut errors);

        if self.messages.is_empty() {
            errors.push(ValidationError::new(
                "thread.messages",
                "must contain an initial message",
            ));
        }
        if self.updated_at_ms < self.created_at_ms {
            errors.push(ValidationError::new(
                "thread.updated_at_ms",
                "cannot precede created_at_ms",
            ));
        }
        match (&self.status, &self.resolved_by, self.resolved_at_ms) {
            (ThreadStatus::Resolved, None, _) => errors.push(ValidationError::new(
                "thread.resolved_by",
                "is required for resolved threads",
            )),
            (ThreadStatus::Resolved, _, None) => errors.push(ValidationError::new(
                "thread.resolved_at_ms",
                "is required for resolved threads",
            )),
            (ThreadStatus::Open, Some(_), _) | (ThreadStatus::Open, _, Some(_)) => {
                errors.push(ValidationError::new(
                    "thread.resolution",
                    "open threads cannot retain resolution metadata",
                ))
            }
            _ => {}
        }
        if let Some(actor) = &self.resolved_by {
            validate_actor(actor, "thread.resolved_by", &mut errors);
        }
        if let Some(resolved_at_ms) = self.resolved_at_ms {
            if resolved_at_ms < self.created_at_ms {
                errors.push(ValidationError::new(
                    "thread.resolved_at_ms",
                    "cannot precede created_at_ms",
                ));
            }
        }

        let mut message_ids = HashSet::new();
        for (index, message) in self.messages.iter().enumerate() {
            let path = format!("thread.messages[{index}]");
            if !message_ids.insert(message.id.as_str()) {
                errors.push(ValidationError::new(
                    format!("{path}.id"),
                    "must be unique within a thread",
                ));
            }
            validate_message(message, &path, &mut errors);
        }
        finish_validation(errors)
    }
}

/// A serializable aggregate that can be saved next to a design document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentStore {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub threads: Vec<ReviewThread>,
}

impl CommentStore {
    pub fn new() -> Self {
        Self {
            schema_version: COMMENT_SCHEMA_VERSION,
            threads: Vec::new(),
        }
    }

    pub fn insert(&mut self, thread: ReviewThread) -> Result<(), ValidationErrors> {
        let mut candidate = self.clone();
        candidate.threads.push(thread);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn get(&self, id: &ThreadId) -> Option<&ReviewThread> {
        self.threads.iter().find(|thread| thread.id == *id)
    }

    pub fn get_mut(&mut self, id: &ThreadId) -> Option<&mut ReviewThread> {
        self.threads.iter_mut().find(|thread| thread.id == *id)
    }

    pub fn open_for_page(&self, page_id: &str) -> impl Iterator<Item = &ReviewThread> {
        self.threads
            .iter()
            .filter(move |thread| thread.anchor.page_id == page_id && !thread.is_resolved())
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        if self.schema_version != COMMENT_SCHEMA_VERSION {
            errors.push(ValidationError::new(
                "store.schema_version",
                format!("unsupported schema version {}", self.schema_version),
            ));
        }
        let mut thread_ids = HashSet::new();
        for (index, thread) in self.threads.iter().enumerate() {
            if !thread_ids.insert(thread.id.as_str()) {
                errors.push(ValidationError::new(
                    format!("store.threads[{index}].id"),
                    "must be unique",
                ));
            }
            if let Err(thread_errors) = thread.validate() {
                errors.extend(thread_errors.into_inner().into_iter().map(|error| {
                    ValidationError::new(
                        format!("store.threads[{index}].{}", error.path),
                        error.message,
                    )
                }));
            }
        }
        finish_validation(errors)
    }
}

impl Default for CommentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationErrors(Vec<ValidationError>);

impl ValidationErrors {
    pub fn errors(&self) -> &[ValidationError] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<ValidationError> {
        self.0
    }
}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "comment validation failed with {} error(s)",
            self.0.len()
        )
    }
}

impl std::error::Error for ValidationErrors {}

fn default_schema_version() -> u32 {
    COMMENT_SCHEMA_VERSION
}

fn finish_validation(errors: Vec<ValidationError>) -> Result<(), ValidationErrors> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(errors))
    }
}

fn require_non_blank(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: &str) {
    if value.trim().is_empty() {
        errors.push(ValidationError::new(path, "must not be blank"));
    }
}

fn validate_actor(actor: &Actor, path: &str, errors: &mut Vec<ValidationError>) {
    require_non_blank(errors, format!("{path}.id"), actor.id.as_str());
    require_non_blank(errors, format!("{path}.display_name"), &actor.display_name);
}

fn validate_anchor(anchor: &CommentAnchor, errors: &mut Vec<ValidationError>) {
    if let Some(node_id) = &anchor.node_id {
        require_non_blank(errors, "anchor.node_id", node_id);
    }
    match &anchor.region {
        AnchorRegion::Target => {}
        AnchorRegion::Point { x, y } => {
            validate_coordinate("anchor.region.x", *x, errors);
            validate_coordinate("anchor.region.y", *y, errors);
        }
        AnchorRegion::Rect {
            x,
            y,
            width,
            height,
        } => {
            validate_coordinate("anchor.region.x", *x, errors);
            validate_coordinate("anchor.region.y", *y, errors);
            if !width.is_finite() || *width <= 0.0 {
                errors.push(ValidationError::new(
                    "anchor.region.width",
                    "must be finite and positive",
                ));
            }
            if !height.is_finite() || *height <= 0.0 {
                errors.push(ValidationError::new(
                    "anchor.region.height",
                    "must be finite and positive",
                ));
            }
        }
    }
}

fn validate_coordinate(path: &str, value: f32, errors: &mut Vec<ValidationError>) {
    if !value.is_finite() {
        errors.push(ValidationError::new(path, "must be finite"));
    }
}

fn validate_message(message: &CommentMessage, path: &str, errors: &mut Vec<ValidationError>) {
    require_non_blank(errors, format!("{path}.id"), message.id.as_str());
    validate_actor(&message.author, &format!("{path}.author"), errors);
    require_non_blank(errors, format!("{path}.body"), &message.body);
    if let Some(edited_at_ms) = message.edited_at_ms {
        if edited_at_ms < message.created_at_ms {
            errors.push(ValidationError::new(
                format!("{path}.edited_at_ms"),
                "cannot precede created_at_ms",
            ));
        }
    }
    let body_length = message.body.chars().count();
    let mut mention_ranges = Vec::with_capacity(message.mentions.len());
    for (index, mention) in message.mentions.iter().enumerate() {
        let mention_path = format!("{path}.mentions[{index}]");
        validate_actor(&mention.user, &format!("{mention_path}.user"), errors);
        let start = mention.range.start as usize;
        let end = mention.range.end as usize;
        if start >= end || end > body_length {
            errors.push(ValidationError::new(
                format!("{mention_path}.range"),
                "must be an ordered range within the message body",
            ));
        }
        mention_ranges.push((start, end, mention_path));
    }
    mention_ranges.sort_unstable_by_key(|(start, _, _)| *start);
    for pair in mention_ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            errors.push(ValidationError::new(
                pair[1].2.as_str(),
                "must not overlap another mention range",
            ));
        }
    }
}

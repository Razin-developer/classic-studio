//! Serializable, transport-independent collaboration domain types.
//!
//! This crate deliberately owns protocol validation and ordering, but not HTTP,
//! WebSockets, authentication, persistence, UI, CRDTs, or operational
//! transforms. Hosts authenticate a caller, map it to a [`RoomMember`], then
//! use [`RoomState`] as the validated command boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COLLAB_SCHEMA_VERSION: u32 = 1;
const MAX_IDENTIFIER_LEN: usize = 160;
const MAX_NAME_LEN: usize = 120;
const MAX_TEXT_LEN: usize = 262_144;
const MAX_SELECTION_SIZE: usize = 256;
const MAX_BATCH_SIZE: usize = 128;
const MAX_COORDINATE: f32 = 10_000_000.0;
const MAX_ZOOM: f32 = 128.0;

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

identifier!(ActorId, "actor");
identifier!(SessionId, "session");
identifier!(RoomId, "room");
identifier!(OperationId, "op");
identifier!(DocumentId, "document");
identifier!(PageId, "page");
identifier!(NodeId, "node");

/// A rendering-safe identity snapshot. Authentication and authorization happen
/// outside this crate; these fields must still be treated as untrusted text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorIdentity {
    pub id: ActorId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl ActorIdentity {
    pub fn new(id: impl Into<ActorId>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            avatar_url: None,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.id, "actor.id", &mut errors);
        validate_label(
            &self.display_name,
            "actor.display_name",
            MAX_NAME_LEN,
            &mut errors,
        );
        if let Some(url) = &self.avatar_url {
            validate_url(url, "actor.avatar_url", &mut errors);
        }
        finish_validation(errors)
    }
}

/// Identifies one connected client session for a person.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentity {
    pub id: SessionId,
    pub actor: ActorIdentity,
    /// Caller-provided diagnostic label, for example `desktop/0.1.0`.
    pub client_label: String,
}

impl SessionIdentity {
    pub fn new(actor: ActorIdentity, client_label: impl Into<String>) -> Self {
        Self {
            id: SessionId::new(),
            actor,
            client_label: client_label.into(),
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.id, "session.id", &mut errors);
        extend(&mut errors, "session.actor", self.actor.validate());
        validate_label(
            &self.client_label,
            "session.client_label",
            MAX_NAME_LEN,
            &mut errors,
        );
        finish_validation(errors)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Owner,
    Editor,
    Commenter,
    Viewer,
}

impl MemberRole {
    pub fn can_edit(self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }
}

/// One authorized session in a room. A service should build this after it has
/// verified a token and document-level access policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomMember {
    pub room_id: RoomId,
    pub session: SessionIdentity,
    pub role: MemberRole,
    pub joined_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<i64>,
}

impl RoomMember {
    pub fn new(
        room_id: impl Into<RoomId>,
        session: SessionIdentity,
        role: MemberRole,
        joined_at_ms: i64,
    ) -> Self {
        Self {
            room_id: room_id.into(),
            session,
            role,
            joined_at_ms,
            last_heartbeat_at_ms: None,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.room_id, "member.room_id", &mut errors);
        extend(&mut errors, "member.session", self.session.validate());
        validate_time(self.joined_at_ms, "member.joined_at_ms", &mut errors);
        if let Some(last) = self.last_heartbeat_at_ms {
            validate_time(last, "member.last_heartbeat_at_ms", &mut errors);
            if last < self.joined_at_ms {
                errors.push(ValidationError::new(
                    "member.last_heartbeat_at_ms",
                    "cannot precede joined_at_ms",
                ));
            }
        }
        finish_validation(errors)
    }
}

/// Stable document-room metadata. `document_id` belongs to the native `.classic`
/// document and is used to reject accidental cross-document writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationRoom {
    pub id: RoomId,
    pub document_id: DocumentId,
    pub created_at_ms: i64,
}

impl CollaborationRoom {
    pub fn new(document_id: impl Into<DocumentId>, created_at_ms: i64) -> Self {
        Self {
            id: RoomId::new(),
            document_id: document_id.into(),
            created_at_ms,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.id, "room.id", &mut errors);
        validate_id(&self.document_id, "room.document_id", &mut errors);
        validate_time(self.created_at_ms, "room.created_at_ms", &mut errors);
        finish_validation(errors)
    }
}

/// Cursor coordinates are document-world coordinates, never screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPosition {
    pub x: f32,
    pub y: f32,
}

/// The currently visible canvas region reported by a client.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
}

/// A session-local heartbeat. `sequence` must increase for each session. It is
/// intentionally separate from document-operation ordering, because a cursor
/// update must never alter document versioning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceHeartbeat {
    pub room_id: RoomId,
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub page_id: PageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub selected_node_ids: BTreeSet<NodeId>,
    pub sequence: u64,
    pub sent_at_ms: i64,
}

impl PresenceHeartbeat {
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.room_id, "presence.room_id", &mut errors);
        validate_id(&self.actor_id, "presence.actor_id", &mut errors);
        validate_id(&self.session_id, "presence.session_id", &mut errors);
        validate_id(&self.page_id, "presence.page_id", &mut errors);
        if self.sequence == 0 {
            errors.push(ValidationError::new(
                "presence.sequence",
                "must be greater than zero",
            ));
        }
        validate_time(self.sent_at_ms, "presence.sent_at_ms", &mut errors);
        if self.selected_node_ids.len() > MAX_SELECTION_SIZE {
            errors.push(ValidationError::new(
                "presence.selected_node_ids",
                format!("must contain at most {MAX_SELECTION_SIZE} items"),
            ));
        }
        for (index, id) in self.selected_node_ids.iter().enumerate() {
            validate_id(
                id,
                format!("presence.selected_node_ids[{index}]"),
                &mut errors,
            );
        }
        if let Some(cursor) = self.cursor {
            validate_position(cursor, "presence.cursor", &mut errors);
        }
        if let Some(viewport) = self.viewport {
            validate_viewport(viewport, "presence.viewport", &mut errors);
        }
        finish_validation(errors)
    }
}

/// A restricted document operation vocabulary. The transport can encode every
/// branch as JSON while the desktop maps the operation to its native command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum DocumentAction {
    CreateNode {
        page_id: PageId,
        node_id: NodeId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<NodeId>,
        node_kind: NodeKind,
    },
    DeleteNodes {
        page_id: PageId,
        node_ids: Vec<NodeId>,
    },
    SetNodeName {
        node_id: NodeId,
        name: String,
    },
    SetTextContent {
        node_id: NodeId,
        text: String,
    },
    SetNodeTransform {
        node_id: NodeId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rotation_degrees: f32,
    },
    SetNodeVisibility {
        node_id: NodeId,
        visible: bool,
    },
    ReparentNodes {
        page_id: PageId,
        node_ids: Vec<NodeId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<NodeId>,
        index: u32,
    },
    RenamePage {
        page_id: PageId,
        name: String,
    },
    CreatePage {
        page_id: PageId,
        name: String,
    },
    DeletePage {
        page_id: PageId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Frame,
    Rectangle,
    Ellipse,
    Line,
    Text,
    Image,
    Component,
    Instance,
}

impl DocumentAction {
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        match self {
            Self::CreateNode {
                page_id,
                node_id,
                parent_id,
                ..
            } => {
                validate_id(page_id, "action.page_id", &mut errors);
                validate_id(node_id, "action.node_id", &mut errors);
                if let Some(parent_id) = parent_id {
                    validate_id(parent_id, "action.parent_id", &mut errors);
                }
            }
            Self::DeleteNodes { page_id, node_ids } => {
                validate_id(page_id, "action.page_id", &mut errors);
                validate_node_batch(node_ids, "action.node_ids", &mut errors);
            }
            Self::SetNodeName { node_id, name } => {
                validate_id(node_id, "action.node_id", &mut errors);
                validate_label(name, "action.name", MAX_NAME_LEN, &mut errors);
            }
            Self::SetTextContent { node_id, text } => {
                validate_id(node_id, "action.node_id", &mut errors);
                validate_text(text, "action.text", MAX_TEXT_LEN, &mut errors);
            }
            Self::SetNodeTransform {
                node_id,
                x,
                y,
                width,
                height,
                rotation_degrees,
            } => {
                validate_id(node_id, "action.node_id", &mut errors);
                validate_coordinate(*x, "action.x", &mut errors);
                validate_coordinate(*y, "action.y", &mut errors);
                validate_positive(*width, "action.width", &mut errors);
                validate_positive(*height, "action.height", &mut errors);
                if !rotation_degrees.is_finite() || rotation_degrees.abs() > 360_000.0 {
                    errors.push(ValidationError::new(
                        "action.rotation_degrees",
                        "must be finite and within +/-360000",
                    ));
                }
            }
            Self::SetNodeVisibility { node_id, .. } => {
                validate_id(node_id, "action.node_id", &mut errors)
            }
            Self::ReparentNodes {
                page_id,
                node_ids,
                parent_id,
                ..
            } => {
                validate_id(page_id, "action.page_id", &mut errors);
                validate_node_batch(node_ids, "action.node_ids", &mut errors);
                if let Some(parent_id) = parent_id {
                    validate_id(parent_id, "action.parent_id", &mut errors);
                }
            }
            Self::RenamePage { page_id, name } | Self::CreatePage { page_id, name } => {
                validate_id(page_id, "action.page_id", &mut errors);
                validate_label(name, "action.name", MAX_NAME_LEN, &mut errors);
            }
            Self::DeletePage { page_id } => validate_id(page_id, "action.page_id", &mut errors),
        }
        finish_validation(errors)
    }
}

/// One immutable, ordered request to mutate the document. A server is expected
/// to reject it unless `base_version` and `sequence` match [`RoomState`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOperation {
    pub id: OperationId,
    pub room_id: RoomId,
    pub document_id: DocumentId,
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub base_version: u64,
    pub sequence: u64,
    pub created_at_ms: i64,
    pub action: DocumentAction,
}

impl DocumentOperation {
    pub fn new(
        room: &CollaborationRoom,
        member: &RoomMember,
        base_version: u64,
        action: DocumentAction,
        created_at_ms: i64,
    ) -> Self {
        Self {
            id: OperationId::new(),
            room_id: room.id.clone(),
            document_id: room.document_id.clone(),
            actor_id: member.session.actor.id.clone(),
            session_id: member.session.id.clone(),
            base_version,
            sequence: base_version.saturating_add(1),
            created_at_ms,
            action,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        validate_id(&self.id, "operation.id", &mut errors);
        validate_id(&self.room_id, "operation.room_id", &mut errors);
        validate_id(&self.document_id, "operation.document_id", &mut errors);
        validate_id(&self.actor_id, "operation.actor_id", &mut errors);
        validate_id(&self.session_id, "operation.session_id", &mut errors);
        if self.sequence == 0 {
            errors.push(ValidationError::new(
                "operation.sequence",
                "must be greater than zero",
            ));
        }
        validate_time(self.created_at_ms, "operation.created_at_ms", &mut errors);
        extend(&mut errors, "operation.action", self.action.validate());
        finish_validation(errors)
    }
}

/// A versioned room aggregate suitable for a database record or in-memory room
/// coordinator. It retains all operations; persistence services may archive a
/// validated prefix and retain an equivalent snapshot/version separately.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomState {
    #[serde(default = "current_schema_version")]
    pub schema_version: u32,
    pub room: CollaborationRoom,
    #[serde(default)]
    members: BTreeMap<SessionId, RoomMember>,
    #[serde(default)]
    presence: BTreeMap<SessionId, PresenceHeartbeat>,
    #[serde(default)]
    operations: Vec<DocumentOperation>,
    #[serde(default)]
    pub version: u64,
}

impl RoomState {
    pub fn new(room: CollaborationRoom) -> Self {
        Self {
            schema_version: COLLAB_SCHEMA_VERSION,
            room,
            members: BTreeMap::new(),
            presence: BTreeMap::new(),
            operations: Vec::new(),
            version: 0,
        }
    }

    pub fn members(&self) -> impl Iterator<Item = &RoomMember> {
        self.members.values()
    }
    pub fn operation_log(&self) -> &[DocumentOperation] {
        &self.operations
    }
    pub fn presence_for(&self, session_id: &SessionId) -> Option<&PresenceHeartbeat> {
        self.presence.get(session_id)
    }
    pub fn active_presence(&self) -> impl Iterator<Item = &PresenceHeartbeat> {
        self.presence.values()
    }

    pub fn add_member(&mut self, member: RoomMember) -> Result<(), CollaborationError> {
        let mut candidate = self.clone();
        if member.room_id != candidate.room.id {
            return Err(Conflict::RoomMismatch.into());
        }
        if candidate.members.contains_key(&member.session.id) {
            return Err(Conflict::DuplicateSession(member.session.id).into());
        }
        candidate.members.insert(member.session.id.clone(), member);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn remove_member(
        &mut self,
        session_id: &SessionId,
    ) -> Result<RoomMember, CollaborationError> {
        let mut candidate = self.clone();
        let removed = candidate
            .members
            .remove(session_id)
            .ok_or_else(|| Conflict::UnknownSession(session_id.clone()))?;
        candidate.presence.remove(session_id);
        candidate.validate()?;
        *self = candidate;
        Ok(removed)
    }

    /// Applies a heartbeat only when it belongs to a current member and carries
    /// a sequence greater than that session's last accepted heartbeat.
    pub fn accept_presence(
        &mut self,
        heartbeat: PresenceHeartbeat,
    ) -> Result<(), CollaborationError> {
        let mut candidate = self.clone();
        heartbeat.validate()?;
        if heartbeat.room_id != candidate.room.id {
            return Err(Conflict::RoomMismatch.into());
        }
        let member = candidate
            .members
            .get_mut(&heartbeat.session_id)
            .ok_or_else(|| Conflict::UnknownSession(heartbeat.session_id.clone()))?;
        if member.session.actor.id != heartbeat.actor_id {
            return Err(Conflict::ActorSessionMismatch.into());
        }
        if let Some(current) = candidate.presence.get(&heartbeat.session_id) {
            if heartbeat.sequence <= current.sequence {
                return Err(Conflict::StaleHeartbeat {
                    received: heartbeat.sequence,
                    current: current.sequence,
                }
                .into());
            }
        }
        if heartbeat.sent_at_ms < member.joined_at_ms {
            return Err(Conflict::HeartbeatBeforeJoin.into());
        }
        member.last_heartbeat_at_ms = Some(heartbeat.sent_at_ms);
        candidate
            .presence
            .insert(heartbeat.session_id.clone(), heartbeat);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Appends an edit after enforcing exact optimistic concurrency semantics.
    /// `base_version` must equal the current room version and `sequence` must
    /// equal the next global sequence. The method never attempts a hidden merge.
    pub fn accept_operation(
        &mut self,
        operation: DocumentOperation,
    ) -> Result<u64, CollaborationError> {
        let mut candidate = self.clone();
        operation.validate()?;
        if operation.room_id != candidate.room.id
            || operation.document_id != candidate.room.document_id
        {
            return Err(Conflict::RoomMismatch.into());
        }
        let member = candidate
            .members
            .get(&operation.session_id)
            .ok_or_else(|| Conflict::UnknownSession(operation.session_id.clone()))?;
        if member.session.actor.id != operation.actor_id {
            return Err(Conflict::ActorSessionMismatch.into());
        }
        if !member.role.can_edit() {
            return Err(Conflict::EditForbidden(member.role).into());
        }
        if candidate
            .operations
            .iter()
            .any(|current| current.id == operation.id)
        {
            return Err(Conflict::DuplicateOperation(operation.id).into());
        }
        if operation.base_version != candidate.version {
            return Err(Conflict::VersionMismatch {
                received: operation.base_version,
                current: candidate.version,
            }
            .into());
        }
        let expected_sequence = candidate
            .version
            .checked_add(1)
            .ok_or(Conflict::SequenceExhausted)?;
        if operation.sequence != expected_sequence {
            return Err(Conflict::SequenceMismatch {
                received: operation.sequence,
                expected: expected_sequence,
            }
            .into());
        }
        candidate.operations.push(operation);
        candidate.version = expected_sequence;
        candidate.validate()?;
        *self = candidate;
        Ok(self.version)
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();
        if self.schema_version != COLLAB_SCHEMA_VERSION {
            errors.push(ValidationError::new(
                "state.schema_version",
                format!("unsupported schema version {}", self.schema_version),
            ));
        }
        extend(&mut errors, "state.room", self.room.validate());
        for (session_id, member) in &self.members {
            if session_id != &member.session.id {
                errors.push(ValidationError::new(
                    "state.members",
                    "map key does not match member session ID",
                ));
            }
            if member.room_id != self.room.id {
                errors.push(ValidationError::new(
                    "state.members",
                    "member belongs to another room",
                ));
            }
            extend(
                &mut errors,
                format!("state.members[{}]", session_id.as_str()),
                member.validate(),
            );
        }
        for (session_id, heartbeat) in &self.presence {
            if session_id != &heartbeat.session_id {
                errors.push(ValidationError::new(
                    "state.presence",
                    "map key does not match heartbeat session ID",
                ));
            }
            let Some(member) = self.members.get(session_id) else {
                errors.push(ValidationError::new(
                    "state.presence",
                    "heartbeat belongs to a removed session",
                ));
                continue;
            };
            if heartbeat.room_id != self.room.id || heartbeat.actor_id != member.session.actor.id {
                errors.push(ValidationError::new(
                    "state.presence",
                    "heartbeat identity does not match its room member",
                ));
            }
            extend(
                &mut errors,
                format!("state.presence[{}]", session_id.as_str()),
                heartbeat.validate(),
            );
        }
        let mut expected_version = 0_u64;
        let mut operation_ids = BTreeSet::new();
        for (index, operation) in self.operations.iter().enumerate() {
            let path = format!("state.operations[{index}]");
            extend(&mut errors, &path, operation.validate());
            if !operation_ids.insert(operation.id.clone()) {
                errors.push(ValidationError::new(format!("{path}.id"), "must be unique"));
            }
            if operation.room_id != self.room.id || operation.document_id != self.room.document_id {
                errors.push(ValidationError::new(
                    &path,
                    "operation belongs to another room or document",
                ));
            }
            if operation.base_version != expected_version {
                errors.push(ValidationError::new(
                    format!("{path}.base_version"),
                    format!("must equal {expected_version}"),
                ));
            }
            expected_version = expected_version.saturating_add(1);
            if operation.sequence != expected_version {
                errors.push(ValidationError::new(
                    format!("{path}.sequence"),
                    format!("must equal {expected_version}"),
                ));
            }
        }
        if self.version != expected_version {
            errors.push(ValidationError::new(
                "state.version",
                format!("must equal operation log version {expected_version}"),
            ));
        }
        finish_validation(errors)
    }
}

impl Default for RoomState {
    fn default() -> Self {
        Self::new(CollaborationRoom::new(DocumentId::new(), 0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Conflict {
    RoomMismatch,
    UnknownSession(SessionId),
    DuplicateSession(SessionId),
    ActorSessionMismatch,
    HeartbeatBeforeJoin,
    StaleHeartbeat { received: u64, current: u64 },
    EditForbidden(MemberRole),
    DuplicateOperation(OperationId),
    VersionMismatch { received: u64, current: u64 },
    SequenceMismatch { received: u64, expected: u64 },
    SequenceExhausted,
}

impl fmt::Display for Conflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomMismatch => write!(formatter, "room or document identity does not match"),
            Self::UnknownSession(id) => write!(formatter, "unknown session {}", id.as_str()),
            Self::DuplicateSession(id) => {
                write!(formatter, "session {} is already a member", id.as_str())
            }
            Self::ActorSessionMismatch => {
                write!(formatter, "actor does not own the declared session")
            }
            Self::HeartbeatBeforeJoin => write!(formatter, "heartbeat precedes session join time"),
            Self::StaleHeartbeat { received, current } => write!(
                formatter,
                "heartbeat sequence {received} is not newer than {current}"
            ),
            Self::EditForbidden(role) => {
                write!(formatter, "role {role:?} cannot submit document edits")
            }
            Self::DuplicateOperation(id) => {
                write!(formatter, "operation {} was already accepted", id.as_str())
            }
            Self::VersionMismatch { received, current } => write!(
                formatter,
                "operation base version {received} does not match room version {current}"
            ),
            Self::SequenceMismatch { received, expected } => write!(
                formatter,
                "operation sequence {received} does not match expected {expected}"
            ),
            Self::SequenceExhausted => {
                write!(formatter, "room sequence has reached its maximum value")
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CollaborationError {
    Validation(ValidationErrors),
    Conflict(Conflict),
}

impl From<ValidationErrors> for CollaborationError {
    fn from(value: ValidationErrors) -> Self {
        Self::Validation(value)
    }
}
impl From<Conflict> for CollaborationError {
    fn from(value: Conflict) -> Self {
        Self::Conflict(value)
    }
}
impl fmt::Display for CollaborationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Conflict(error) => write!(formatter, "{error}"),
        }
    }
}
impl Error for CollaborationError {}

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
impl fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "collaboration validation failed with {} error(s)",
            self.0.len()
        )
    }
}
impl Error for ValidationErrors {}

fn current_schema_version() -> u32 {
    COLLAB_SCHEMA_VERSION
}
fn finish_validation(errors: Vec<ValidationError>) -> Result<(), ValidationErrors> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(errors))
    }
}
fn extend(
    errors: &mut Vec<ValidationError>,
    prefix: impl AsRef<str>,
    result: Result<(), ValidationErrors>,
) {
    if let Err(source) = result {
        let prefix = prefix.as_ref();
        errors.extend(
            source.into_inner().into_iter().map(|error| {
                ValidationError::new(format!("{prefix}.{}", error.path), error.message)
            }),
        );
    }
}
fn validate_id<T: AsRef<str>>(
    value: &T,
    path: impl Into<String>,
    errors: &mut Vec<ValidationError>,
) {
    let value = value.as_ref();
    if value.trim().is_empty()
        || value.len() > MAX_IDENTIFIER_LEN
        || value.chars().any(char::is_control)
    {
        errors.push(ValidationError::new(path, format!("must be nonblank, at most {MAX_IDENTIFIER_LEN} bytes, and contain no control characters")));
    }
}
fn validate_label(
    value: &str,
    path: impl Into<String>,
    max_len: usize,
    errors: &mut Vec<ValidationError>,
) {
    if value.trim().is_empty()
        || value.chars().count() > max_len
        || value.chars().any(char::is_control)
    {
        errors.push(ValidationError::new(
            path,
            format!(
                "must be nonblank, at most {max_len} characters, and contain no control characters"
            ),
        ));
    }
}
fn validate_text(
    value: &str,
    path: impl Into<String>,
    max_len: usize,
    errors: &mut Vec<ValidationError>,
) {
    if value.chars().count() > max_len
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        errors.push(ValidationError::new(path, format!("must be at most {max_len} characters and contain only tab/newline control characters")));
    }
}
fn validate_url(value: &str, path: impl Into<String>, errors: &mut Vec<ValidationError>) {
    let allowed = value.starts_with("https://") || value.starts_with("http://");
    if !allowed || value.len() > 2_048 || value.chars().any(char::is_control) {
        errors.push(ValidationError::new(
            path,
            "must be an http(s) URL no longer than 2048 bytes",
        ));
    }
}
fn validate_time(value: i64, path: impl Into<String>, errors: &mut Vec<ValidationError>) {
    if value < 0 {
        errors.push(ValidationError::new(
            path,
            "must be Unix milliseconds greater than or equal to zero",
        ));
    }
}
fn validate_coordinate(value: f32, path: impl Into<String>, errors: &mut Vec<ValidationError>) {
    if !value.is_finite() || value.abs() > MAX_COORDINATE {
        errors.push(ValidationError::new(
            path,
            format!("must be finite and within +/-{MAX_COORDINATE}"),
        ));
    }
}
fn validate_positive(value: f32, path: impl Into<String>, errors: &mut Vec<ValidationError>) {
    if !value.is_finite() || value <= 0.0 || value > MAX_COORDINATE {
        errors.push(ValidationError::new(
            path,
            format!("must be finite, positive, and no greater than {MAX_COORDINATE}"),
        ));
    }
}
fn validate_position(position: CursorPosition, path: &str, errors: &mut Vec<ValidationError>) {
    validate_coordinate(position.x, format!("{path}.x"), errors);
    validate_coordinate(position.y, format!("{path}.y"), errors);
}
fn validate_viewport(viewport: Viewport, path: &str, errors: &mut Vec<ValidationError>) {
    validate_coordinate(viewport.x, format!("{path}.x"), errors);
    validate_coordinate(viewport.y, format!("{path}.y"), errors);
    validate_positive(viewport.width, format!("{path}.width"), errors);
    validate_positive(viewport.height, format!("{path}.height"), errors);
    if !viewport.zoom.is_finite() || viewport.zoom <= 0.0 || viewport.zoom > MAX_ZOOM {
        errors.push(ValidationError::new(
            format!("{path}.zoom"),
            format!("must be finite, positive, and no greater than {MAX_ZOOM}"),
        ));
    }
}
fn validate_node_batch(nodes: &[NodeId], path: &str, errors: &mut Vec<ValidationError>) {
    if nodes.is_empty() || nodes.len() > MAX_BATCH_SIZE {
        errors.push(ValidationError::new(
            path,
            format!("must contain between 1 and {MAX_BATCH_SIZE} nodes"),
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, node) in nodes.iter().enumerate() {
        validate_id(node, format!("{path}[{index}]"), errors);
        if !ids.insert(node) {
            errors.push(ValidationError::new(
                format!("{path}[{index}]"),
                "must not duplicate a node ID",
            ));
        }
    }
}

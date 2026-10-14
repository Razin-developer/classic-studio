//! Local asset catalog primitives for Classic Studio.
//!
//! The catalog supports either stable linked references or self-contained,
//! hash-verified embedded payloads. Importing files, decoding media, thumbnail
//! generation, syncing, and network transfer belong to higher layers, keeping
//! this crate deterministic and usable in native or headless Rust applications.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Current persistence schema for [`AssetCatalog`].
pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;

/// The content-digest format used by [`EmbeddedAssetPayload`].
pub const EMBEDDED_ASSET_HASH_ALGORITHM: &str = "sha256";

/// A stable catalog-local asset identifier.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub String);

impl AssetId {
    /// Creates a UUID-backed ID for a newly imported asset.
    pub fn new() -> Self {
        Self(format!("asset_{}", Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AssetId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AssetId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for AssetId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Broad media category used for filtering and validating metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    RasterImage,
    VectorImage,
    Font,
    Video,
    Audio,
    Document,
    Other,
}

impl AssetKind {
    fn supports_dimensions(self) -> bool {
        matches!(self, Self::RasterImage | Self::VectorImage | Self::Video)
    }
}

/// Pixel dimensions for visual media.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Non-file metadata required to identify and display an asset in the editor.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMetadata {
    /// Original filename or the user-visible name assigned by the caller.
    pub name: String,
    /// IANA MIME type, such as `image/png`.
    pub media_type: String,
    /// Source byte count, where known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_len: Option<u64>,
    /// SHA-256 (or another caller-selected) content checksum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_size: Option<PixelSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<String>,
}

impl AssetMetadata {
    pub fn new(name: impl Into<String>, media_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            media_type: media_type.into(),
            ..Self::default()
        }
    }
}

/// Self-contained asset bytes and their integrity metadata.
///
/// The serialized `data` field is base64 so a `.classic` JSON document stays
/// portable. The size and SHA-256 digest are stored beside the bytes and are
/// verified whenever the containing catalog is validated.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedAssetPayload {
    #[serde(with = "base64_bytes")]
    data: Vec<u8>,
    pub byte_len: u64,
    pub content_hash: String,
}

impl EmbeddedAssetPayload {
    /// Creates a payload with byte length and SHA-256 digest derived from
    /// `data`. Callers cannot accidentally create inconsistent metadata via
    /// this constructor.
    pub fn from_bytes(data: impl Into<Vec<u8>>) -> Self {
        let data = data.into();
        let byte_len = data.len() as u64;
        let content_hash = content_hash(&data);
        Self {
            data,
            byte_len,
            content_hash,
        }
    }

    /// Returns the original unencoded asset bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns the declared hashing algorithm for [`Self::content_hash`].
    pub const fn hash_algorithm(&self) -> &'static str {
        EMBEDDED_ASSET_HASH_ALGORITHM
    }

    /// Checks that the persisted byte count and hash describe the embedded
    /// bytes exactly.
    pub fn validate(&self) -> Result<(), EmbeddedAssetError> {
        if self.byte_len != self.data.len() as u64 {
            return Err(EmbeddedAssetError::ByteLengthMismatch {
                declared: self.byte_len,
                actual: self.data.len() as u64,
            });
        }
        if !is_sha256_hex(&self.content_hash) {
            return Err(EmbeddedAssetError::InvalidContentHash {
                content_hash: self.content_hash.clone(),
            });
        }

        let actual = content_hash(&self.data);
        if self.content_hash != actual {
            return Err(EmbeddedAssetError::ContentHashMismatch {
                declared: self.content_hash.clone(),
                actual,
            });
        }
        Ok(())
    }
}

/// Integrity failures for an [`EmbeddedAssetPayload`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddedAssetError {
    ByteLengthMismatch { declared: u64, actual: u64 },
    InvalidContentHash { content_hash: String },
    ContentHashMismatch { declared: String, actual: String },
}

impl fmt::Display for EmbeddedAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLengthMismatch { declared, actual } => write!(
                formatter,
                "embedded payload length {declared} does not match actual length {actual}"
            ),
            Self::InvalidContentHash { content_hash } => write!(
                formatter,
                "embedded payload hash is not a lowercase SHA-256 hex digest: {content_hash}"
            ),
            Self::ContentHashMismatch { declared, actual } => write!(
                formatter,
                "embedded payload hash {declared} does not match computed SHA-256 {actual}"
            ),
        }
    }
}

impl Error for EmbeddedAssetError {}

/// A stable asset reference. The URI is opaque to this crate and may address
/// an app-owned cache, embedded document blob, or another local source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRecord {
    pub id: AssetId,
    pub kind: AssetKind,
    pub metadata: AssetMetadata,
    /// The original linked location. Embedded records use an `embedded://`
    /// URI, retaining one stable source-reference field for all asset forms.
    pub source_uri: String,
    /// Optional self-contained bytes. The field is absent for existing linked
    /// records, preserving their serialized representation and behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_payload: Option<EmbeddedAssetPayload>,
}

impl AssetRecord {
    pub fn new(kind: AssetKind, metadata: AssetMetadata, source_uri: impl Into<String>) -> Self {
        Self {
            id: AssetId::new(),
            kind,
            metadata,
            source_uri: source_uri.into(),
            embedded_payload: None,
        }
    }

    /// Creates a portable asset record whose bytes are stored in the document.
    ///
    /// The metadata size and checksum are synchronized to the payload so UI
    /// layers can use their existing metadata fields without recomputing them.
    pub fn new_embedded(
        kind: AssetKind,
        mut metadata: AssetMetadata,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        let id = AssetId::new();
        let embedded_payload = EmbeddedAssetPayload::from_bytes(data);
        synchronize_metadata_with_payload(&mut metadata, &embedded_payload);
        Self {
            source_uri: format!("embedded://{}", id.as_str()),
            id,
            kind,
            metadata,
            embedded_payload: Some(embedded_payload),
        }
    }

    /// Replaces or adds the portable payload and synchronizes the related
    /// metadata fields.
    pub fn set_embedded_bytes(&mut self, data: impl Into<Vec<u8>>) {
        let payload = EmbeddedAssetPayload::from_bytes(data);
        synchronize_metadata_with_payload(&mut self.metadata, &payload);
        self.embedded_payload = Some(payload);
        if !self.source_uri.starts_with("embedded://") {
            self.source_uri = format!("embedded://{}", self.id.as_str());
        }
    }

    /// Drops the stored payload while retaining the source URI for the caller
    /// to replace with an external location if needed.
    pub fn clear_embedded_payload(&mut self) -> Option<EmbeddedAssetPayload> {
        self.embedded_payload.take()
    }

    /// Gives callers direct read access to a verified-on-save payload.
    pub fn embedded_bytes(&self) -> Option<&[u8]> {
        self.embedded_payload
            .as_ref()
            .map(EmbeddedAssetPayload::bytes)
    }

    pub fn is_embedded(&self) -> bool {
        self.embedded_payload.is_some()
    }

    pub fn validate(&self) -> Result<(), AssetCatalogError> {
        validate_id(&self.id)?;
        validate_nonempty("asset name", &self.metadata.name)?;
        validate_nonempty("media type", &self.metadata.media_type)?;
        validate_nonempty("source URI", &self.source_uri)?;

        if let Some(payload) = &self.embedded_payload {
            payload
                .validate()
                .map_err(|source| AssetCatalogError::InvalidEmbeddedPayload {
                    id: self.id.clone(),
                    source,
                })?;
            if self.metadata.byte_len != Some(payload.byte_len) {
                return Err(AssetCatalogError::EmbeddedByteLengthMetadataMismatch {
                    id: self.id.clone(),
                    metadata: self.metadata.byte_len,
                    payload: payload.byte_len,
                });
            }
            if self.metadata.checksum.as_deref() != Some(payload.content_hash.as_str()) {
                return Err(AssetCatalogError::EmbeddedChecksumMetadataMismatch {
                    id: self.id.clone(),
                    metadata: self.metadata.checksum.clone(),
                    payload: payload.content_hash.clone(),
                });
            }
        }

        if self.metadata.pixel_size.is_some() && !self.kind.supports_dimensions() {
            return Err(AssetCatalogError::UnsupportedMetadata {
                id: self.id.clone(),
                field: "pixelSize",
                kind: self.kind,
            });
        }
        if let Some(size) = self.metadata.pixel_size {
            if size.width == 0 || size.height == 0 {
                return Err(AssetCatalogError::InvalidPixelSize {
                    id: self.id.clone(),
                    width: size.width,
                    height: size.height,
                });
            }
        }
        if self.metadata.duration_ms.is_some()
            && !matches!(self.kind, AssetKind::Video | AssetKind::Audio)
        {
            return Err(AssetCatalogError::UnsupportedMetadata {
                id: self.id.clone(),
                field: "durationMs",
                kind: self.kind,
            });
        }
        if self.metadata.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(AssetCatalogError::EmptyTag {
                id: self.id.clone(),
            });
        }
        Ok(())
    }
}

/// A deterministic, document-local asset index sorted by [`AssetId`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetCatalog {
    #[serde(default = "current_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    assets: BTreeMap<AssetId, AssetRecord>,
}

fn current_schema_version() -> u32 {
    ASSET_CATALOG_SCHEMA_VERSION
}

impl Default for AssetCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetCatalog {
    pub fn new() -> Self {
        Self {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            assets: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn get(&self, id: &AssetId) -> Option<&AssetRecord> {
        self.assets.get(id)
    }

    pub fn contains(&self, id: &AssetId) -> bool {
        self.assets.contains_key(id)
    }

    /// Returns records in stable ascending ID order.
    pub fn iter(&self) -> impl Iterator<Item = &AssetRecord> {
        self.assets.values()
    }

    /// Returns assets of one kind in stable ascending ID order.
    pub fn by_kind(&self, kind: AssetKind) -> impl Iterator<Item = &AssetRecord> {
        self.iter().filter(move |record| record.kind == kind)
    }

    /// Inserts a valid record if and only if its ID has not been used.
    pub fn insert(&mut self, record: AssetRecord) -> Result<(), AssetCatalogError> {
        record.validate()?;
        if self.assets.contains_key(&record.id) {
            return Err(AssetCatalogError::DuplicateId(record.id));
        }
        self.assets.insert(record.id.clone(), record);
        Ok(())
    }

    /// Replaces an existing valid record without changing catalog ordering.
    pub fn replace(&mut self, record: AssetRecord) -> Result<AssetRecord, AssetCatalogError> {
        record.validate()?;
        if !self.assets.contains_key(&record.id) {
            return Err(AssetCatalogError::MissingId(record.id));
        }
        Ok(self
            .assets
            .insert(record.id.clone(), record)
            .expect("existing asset was checked before replacement"))
    }

    /// Removes and returns a record by its stable ID.
    pub fn remove(&mut self, id: &AssetId) -> Result<AssetRecord, AssetCatalogError> {
        self.assets
            .remove(id)
            .ok_or_else(|| AssetCatalogError::MissingId(id.clone()))
    }

    /// Validates the persistence schema and every stored record.
    pub fn validate(&self) -> Result<(), AssetCatalogError> {
        if self.schema_version != ASSET_CATALOG_SCHEMA_VERSION {
            return Err(AssetCatalogError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        for (id, record) in &self.assets {
            if id != &record.id {
                return Err(AssetCatalogError::MismatchedKey {
                    key: id.clone(),
                    record_id: record.id.clone(),
                });
            }
            record.validate()?;
        }
        Ok(())
    }
}

/// Catalog validation and mutation failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetCatalogError {
    UnsupportedSchemaVersion(u32),
    EmptyField(&'static str),
    EmptyId,
    DuplicateId(AssetId),
    MissingId(AssetId),
    MismatchedKey {
        key: AssetId,
        record_id: AssetId,
    },
    InvalidPixelSize {
        id: AssetId,
        width: u32,
        height: u32,
    },
    UnsupportedMetadata {
        id: AssetId,
        field: &'static str,
        kind: AssetKind,
    },
    EmptyTag {
        id: AssetId,
    },
    InvalidEmbeddedPayload {
        id: AssetId,
        source: EmbeddedAssetError,
    },
    EmbeddedByteLengthMetadataMismatch {
        id: AssetId,
        metadata: Option<u64>,
        payload: u64,
    },
    EmbeddedChecksumMetadataMismatch {
        id: AssetId,
        metadata: Option<String>,
        payload: String,
    },
}

impl fmt::Display for AssetCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported asset catalog schema version {version}"
                )
            }
            Self::EmptyField(field) => write!(formatter, "{field} must not be empty"),
            Self::EmptyId => write!(formatter, "asset ID must not be empty"),
            Self::DuplicateId(id) => write!(formatter, "duplicate asset ID: {}", id.as_str()),
            Self::MissingId(id) => write!(formatter, "asset ID was not found: {}", id.as_str()),
            Self::MismatchedKey { key, record_id } => write!(
                formatter,
                "asset catalog key {} does not match record ID {}",
                key.as_str(),
                record_id.as_str()
            ),
            Self::InvalidPixelSize { id, width, height } => write!(
                formatter,
                "asset {} has invalid pixel dimensions {width}x{height}",
                id.as_str()
            ),
            Self::UnsupportedMetadata { id, field, kind } => write!(
                formatter,
                "asset {} of kind {kind:?} cannot use {field}",
                id.as_str()
            ),
            Self::EmptyTag { id } => {
                write!(formatter, "asset {} contains an empty tag", id.as_str())
            }
            Self::InvalidEmbeddedPayload { id, source } => {
                write!(
                    formatter,
                    "asset {} has an invalid embedded payload: {source}",
                    id.as_str()
                )
            }
            Self::EmbeddedByteLengthMetadataMismatch {
                id,
                metadata,
                payload,
            } => write!(
                formatter,
                "asset {} metadata length {metadata:?} does not match embedded payload length {payload}",
                id.as_str()
            ),
            Self::EmbeddedChecksumMetadataMismatch {
                id,
                metadata,
                payload,
            } => write!(
                formatter,
                "asset {} metadata checksum {metadata:?} does not match embedded payload SHA-256 {payload}",
                id.as_str()
            ),
        }
    }
}

impl Error for AssetCatalogError {}

fn synchronize_metadata_with_payload(metadata: &mut AssetMetadata, payload: &EmbeddedAssetPayload) {
    metadata.byte_len = Some(payload.byte_len);
    metadata.checksum = Some(payload.content_hash.clone());
}

fn content_hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

mod base64_bytes {
    use super::BASE64_STANDARD;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64_STANDARD
            .decode(encoded.as_bytes())
            .map_err(D::Error::custom)
    }
}

fn validate_id(id: &AssetId) -> Result<(), AssetCatalogError> {
    if id.0.trim().is_empty() {
        Err(AssetCatalogError::EmptyId)
    } else {
        Ok(())
    }
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<(), AssetCatalogError> {
    if value.trim().is_empty() {
        Err(AssetCatalogError::EmptyField(field))
    } else {
        Ok(())
    }
}

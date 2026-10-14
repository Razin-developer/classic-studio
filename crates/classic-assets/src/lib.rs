//! Local asset catalog primitives for Classic Studio.
//!
//! The catalog contains only metadata and stable references. Importing files,
//! decoding media, thumbnail generation, syncing, and network transfer belong
//! to higher layers, keeping this crate deterministic and usable in native or
//! headless Rust applications.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current persistence schema for [`AssetCatalog`].
pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;

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

/// A stable asset reference. The URI is opaque to this crate and may address
/// an app-owned cache, embedded document blob, or another local source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRecord {
    pub id: AssetId,
    pub kind: AssetKind,
    pub metadata: AssetMetadata,
    pub source_uri: String,
}

impl AssetRecord {
    pub fn new(kind: AssetKind, metadata: AssetMetadata, source_uri: impl Into<String>) -> Self {
        Self {
            id: AssetId::new(),
            kind,
            metadata,
            source_uri: source_uri.into(),
        }
    }

    pub fn validate(&self) -> Result<(), AssetCatalogError> {
        validate_id(&self.id)?;
        validate_nonempty("asset name", &self.metadata.name)?;
        validate_nonempty("media type", &self.metadata.media_type)?;
        validate_nonempty("source URI", &self.source_uri)?;

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
        }
    }
}

impl Error for AssetCatalogError {}

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

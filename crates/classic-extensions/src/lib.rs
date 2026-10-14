//! Extension manifest primitives for Classic Studio.
//!
//! This crate is intentionally only a schema and policy validator. It does
//! not load executable code, open sockets, access files, or spawn processes.
//! A host must validate a manifest before it exposes a constrained extension
//! API. Capabilities are declarations, never authority by themselves.

use std::{
    collections::{BTreeSet, HashSet},
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The current extension-manifest schema version.
pub const EXTENSION_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// A complete, portable declaration for an extension package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    #[serde(default = "current_schema_version")]
    pub schema_version: u32,
    /// A stable, lower-kebab-case extension identifier.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// A semantic version in `MAJOR.MINOR.PATCH` form, optionally with
    /// prerelease and build metadata.
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub origin: ExtensionOrigin,
    /// All privileged host APIs the package may request.
    #[serde(default)]
    pub requested_capabilities: BTreeSet<Capability>,
    /// Host UI and format contributions. No executable entry point is part of
    /// this schema; a host chooses how an approved contribution is realized.
    #[serde(default)]
    pub contributions: Vec<Contribution>,
}

fn current_schema_version() -> u32 {
    EXTENSION_MANIFEST_SCHEMA_VERSION
}

impl ExtensionManifest {
    /// Creates a minimal manifest. The caller must set a trusted origin before
    /// it can pass validation.
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            schema_version: EXTENSION_MANIFEST_SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: String::new(),
            origin: ExtensionOrigin::Local {
                path: PathBuf::new(),
            },
            requested_capabilities: BTreeSet::new(),
            contributions: Vec::new(),
        }
    }

    /// Validates this manifest against the host's explicit trust and consent
    /// policy. Every invalid field is returned together so a package author can
    /// fix a manifest in one pass.
    pub fn validate(&self, policy: &ExtensionPolicy) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        if self.schema_version != EXTENSION_MANIFEST_SCHEMA_VERSION {
            errors.push(ValidationError::new(
                ValidationErrorCode::UnsupportedSchemaVersion,
                "schemaVersion",
                format!(
                    "expected schema version {EXTENSION_MANIFEST_SCHEMA_VERSION}, found {}",
                    self.schema_version
                ),
            ));
        }
        validate_extension_id(&self.id, "id", &mut errors);
        if self.name.trim().is_empty() || self.name.chars().count() > 96 {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidName,
                "name",
                "name must contain 1 to 96 visible characters",
            ));
        }
        if !is_semver_like(&self.version) {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidVersion,
                "version",
                "version must follow MAJOR.MINOR.PATCH with optional prerelease/build metadata",
            ));
        }
        if self.description.chars().count() > 2_000 {
            errors.push(ValidationError::new(
                ValidationErrorCode::DescriptionTooLong,
                "description",
                "description may not exceed 2,000 characters",
            ));
        }

        validate_origin(&self.origin, policy, &mut errors);
        for capability in &self.requested_capabilities {
            if !policy.allowed_capabilities.contains(capability) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::CapabilityNotConsented,
                    format!("requestedCapabilities.{}", capability.as_str()),
                    format!(
                        "the host has not granted the '{}' capability",
                        capability.as_str()
                    ),
                ));
            }
        }

        if self.contributions.len() > policy.max_contributions {
            errors.push(ValidationError::new(
                ValidationErrorCode::TooManyContributions,
                "contributions",
                format!(
                    "at most {} contributions are permitted by this host",
                    policy.max_contributions
                ),
            ));
        }

        let mut contribution_ids = HashSet::new();
        for (index, contribution) in self.contributions.iter().enumerate() {
            contribution.validate(
                index,
                &self.requested_capabilities,
                &mut contribution_ids,
                &mut errors,
            );
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors { errors })
        }
    }
}

/// Where a host found the extension package. A package location is accepted
/// only when the host explicitly trusts its local root or registry/publisher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExtensionOrigin {
    Local {
        path: PathBuf,
    },
    Registry {
        registry: String,
        publisher: String,
        package: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signing_key_id: Option<String>,
    },
}

/// Explicit capability names understood by the Classic host boundary.
///
/// Network, filesystem, and process capabilities are deliberately represented
/// separately and are denied by [`ExtensionPolicy::default`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    DocumentRead,
    DocumentWrite,
    UserInterface,
    Export,
    Network,
    FilesystemRead,
    FilesystemWrite,
    ProcessExecution,
}

impl Capability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentRead => "documentRead",
            Self::DocumentWrite => "documentWrite",
            Self::UserInterface => "userInterface",
            Self::Export => "export",
            Self::Network => "network",
            Self::FilesystemRead => "filesystemRead",
            Self::FilesystemWrite => "filesystemWrite",
            Self::ProcessExecution => "processExecution",
        }
    }

    /// Whether this capability crosses a host security boundary.
    pub const fn is_sensitive(self) -> bool {
        matches!(
            self,
            Self::Network | Self::FilesystemRead | Self::FilesystemWrite | Self::ProcessExecution
        )
    }
}

/// A named integration point exposed by the application chrome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "definition", rename_all = "camelCase")]
pub enum Contribution {
    Tool(ToolContribution),
    Exporter(ExporterContribution),
    Panel(PanelContribution),
}

impl Contribution {
    fn id(&self) -> &str {
        match self {
            Self::Tool(value) => &value.id,
            Self::Exporter(value) => &value.id,
            Self::Panel(value) => &value.id,
        }
    }

    fn required_capabilities(&self) -> &'static [Capability] {
        match self {
            Self::Tool(_) => &[Capability::DocumentWrite, Capability::UserInterface],
            Self::Exporter(_) => &[Capability::DocumentRead, Capability::Export],
            Self::Panel(_) => &[Capability::UserInterface],
        }
    }

    fn validate(
        &self,
        index: usize,
        declared_capabilities: &BTreeSet<Capability>,
        seen_ids: &mut HashSet<String>,
        errors: &mut Vec<ValidationError>,
    ) {
        let prefix = format!("contributions[{index}]");
        validate_contribution_id(self.id(), &format!("{prefix}.id"), errors);
        if !seen_ids.insert(self.id().to_owned()) {
            errors.push(ValidationError::new(
                ValidationErrorCode::DuplicateContributionId,
                format!("{prefix}.id"),
                "contribution IDs must be unique across tools, exporters, and panels",
            ));
        }
        for capability in self.required_capabilities() {
            if !declared_capabilities.contains(capability) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::RequiredCapabilityMissing,
                    format!("{prefix}.id"),
                    format!(
                        "this contribution requires a declared '{}' capability",
                        capability.as_str()
                    ),
                ));
            }
        }
        match self {
            Self::Tool(value) => value.validate(&prefix, errors),
            Self::Exporter(value) => value.validate(&prefix, errors),
            Self::Panel(value) => value.validate(&prefix, errors),
        }
    }
}

/// A tool shown in the design toolbar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolContribution {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

impl ToolContribution {
    fn validate(&self, prefix: &str, errors: &mut Vec<ValidationError>) {
        validate_label(&self.label, &format!("{prefix}.label"), errors);
        if self.icon.as_deref().is_some_and(|value| value.len() > 128) {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidContribution,
                format!("{prefix}.icon"),
                "icon identifier may not exceed 128 bytes",
            ));
        }
        if self
            .shortcut
            .as_deref()
            .is_some_and(|value| value.len() > 64)
        {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidContribution,
                format!("{prefix}.shortcut"),
                "shortcut may not exceed 64 bytes",
            ));
        }
    }
}

/// A document exporter supplied by an extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExporterContribution {
    pub id: String,
    pub label: String,
    pub file_extensions: Vec<String>,
    #[serde(default)]
    pub mime_types: Vec<String>,
}

impl ExporterContribution {
    fn validate(&self, prefix: &str, errors: &mut Vec<ValidationError>) {
        validate_label(&self.label, &format!("{prefix}.label"), errors);
        if self.file_extensions.is_empty() || self.file_extensions.len() > 16 {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidContribution,
                format!("{prefix}.fileExtensions"),
                "an exporter must declare between 1 and 16 file extensions",
            ));
        }
        for (index, extension) in self.file_extensions.iter().enumerate() {
            if !is_file_extension(extension) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::InvalidContribution,
                    format!("{prefix}.fileExtensions[{index}]"),
                    "file extensions must be lowercase alphanumeric suffixes without a dot",
                ));
            }
        }
        for (index, mime_type) in self.mime_types.iter().enumerate() {
            if !is_mime_type(mime_type) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::InvalidContribution,
                    format!("{prefix}.mimeTypes[{index}]"),
                    "MIME types must be a lowercase type/subtype value",
                ));
            }
        }
    }
}

/// A dockable panel supplied by an extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelContribution {
    pub id: String,
    pub title: String,
    pub placement: PanelPlacement,
}

impl PanelContribution {
    fn validate(&self, prefix: &str, errors: &mut Vec<ValidationError>) {
        validate_label(&self.title, &format!("{prefix}.title"), errors);
    }
}

/// The stable host placement for a contributed panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelPlacement {
    LeftDock,
    RightDock,
    BottomDock,
}

/// A host's explicit extension trust, consent, and size policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPolicy {
    /// Local package paths must live below one of these absolute roots.
    #[serde(default)]
    pub trusted_local_roots: Vec<PathBuf>,
    /// Exact trusted registry origins, such as `https://extensions.example`.
    #[serde(default)]
    pub trusted_registries: BTreeSet<String>,
    /// Exact trusted publisher IDs for registry packages.
    #[serde(default)]
    pub trusted_publishers: BTreeSet<String>,
    /// Capabilities the user or administrator has explicitly consented to.
    #[serde(default = "default_allowed_capabilities")]
    pub allowed_capabilities: BTreeSet<Capability>,
    #[serde(default = "default_require_registry_signature")]
    pub require_registry_signature: bool,
    #[serde(default = "default_max_contributions")]
    pub max_contributions: usize,
}

impl Default for ExtensionPolicy {
    fn default() -> Self {
        Self {
            trusted_local_roots: Vec::new(),
            trusted_registries: BTreeSet::new(),
            trusted_publishers: BTreeSet::new(),
            allowed_capabilities: default_allowed_capabilities(),
            require_registry_signature: true,
            max_contributions: default_max_contributions(),
        }
    }
}

impl ExtensionPolicy {
    /// Creates a development policy for one local directory. This is intended
    /// for an opt-in developer mode, never as a production default.
    pub fn for_local_development(root: impl Into<PathBuf>) -> Self {
        let mut policy = Self::default();
        policy.trusted_local_roots.push(root.into());
        policy.allowed_capabilities.extend([
            Capability::DocumentWrite,
            Capability::UserInterface,
            Capability::Export,
        ]);
        policy
    }
}

fn default_allowed_capabilities() -> BTreeSet<Capability> {
    BTreeSet::from([
        Capability::DocumentRead,
        Capability::UserInterface,
        Capability::Export,
    ])
}

const fn default_require_registry_signature() -> bool {
    true
}

const fn default_max_contributions() -> usize {
    32
}

/// A machine-readable validation category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationErrorCode {
    UnsupportedSchemaVersion,
    InvalidExtensionId,
    InvalidContributionId,
    InvalidName,
    InvalidVersion,
    DescriptionTooLong,
    UntrustedOrigin,
    UnsafePath,
    UnsignedRegistryPackage,
    CapabilityNotConsented,
    RequiredCapabilityMissing,
    TooManyContributions,
    DuplicateContributionId,
    InvalidContribution,
}

/// One field-level validation failure suitable for UI, logs, or JSON output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub code: ValidationErrorCode,
    pub path: String,
    pub message: String,
}

impl ValidationError {
    fn new(code: ValidationErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// All failures discovered while validating one manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "extension manifest has {} validation error(s)",
            self.errors.len()
        )
    }
}

impl Error for ValidationErrors {}

fn validate_origin(
    origin: &ExtensionOrigin,
    policy: &ExtensionPolicy,
    errors: &mut Vec<ValidationError>,
) {
    match origin {
        ExtensionOrigin::Local { path } => {
            if !is_safe_absolute_path(path) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::UnsafePath,
                    "origin.path",
                    "local extension path must be absolute and may not contain '..' traversal",
                ));
                return;
            }
            if !policy
                .trusted_local_roots
                .iter()
                .filter(|root| is_safe_absolute_path(root))
                .any(|root| path.starts_with(root))
            {
                errors.push(ValidationError::new(
                    ValidationErrorCode::UntrustedOrigin,
                    "origin.path",
                    "local extension path is outside the host's trusted extension roots",
                ));
            }
        }
        ExtensionOrigin::Registry {
            registry,
            publisher,
            package,
            signing_key_id,
        } => {
            if !policy.trusted_registries.contains(registry)
                || !policy.trusted_publishers.contains(publisher)
            {
                errors.push(ValidationError::new(
                    ValidationErrorCode::UntrustedOrigin,
                    "origin",
                    "registry or publisher is not trusted by this host",
                ));
            }
            if !is_safe_registry_name(package) {
                errors.push(ValidationError::new(
                    ValidationErrorCode::UntrustedOrigin,
                    "origin.package",
                    "registry package must be a lowercase safe identifier",
                ));
            }
            if policy.require_registry_signature
                && signing_key_id.as_deref().is_none_or(str::is_empty)
            {
                errors.push(ValidationError::new(
                    ValidationErrorCode::UnsignedRegistryPackage,
                    "origin.signingKeyId",
                    "registry packages require a signing key identifier",
                ));
            }
        }
    }
}

fn validate_extension_id(value: &str, path: &str, errors: &mut Vec<ValidationError>) {
    if !is_extension_id(value) {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidExtensionId,
            path,
            "extension ID must be 3-64 lowercase kebab-case ASCII characters",
        ));
    }
}

fn validate_contribution_id(value: &str, path: &str, errors: &mut Vec<ValidationError>) {
    if !is_contribution_id(value) {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidContributionId,
            path,
            "contribution ID must be 1-64 lowercase kebab-case ASCII characters",
        ));
    }
}

fn validate_label(value: &str, path: &str, errors: &mut Vec<ValidationError>) {
    if value.trim().is_empty() || value.chars().count() > 96 {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidContribution,
            path,
            "label must contain 1 to 96 visible characters",
        ));
    }
}

fn is_extension_id(value: &str) -> bool {
    value.len() >= 3 && value.len() <= 64 && is_kebab_identifier(value)
}

fn is_contribution_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && is_kebab_identifier(value)
}

fn is_safe_registry_name(value: &str) -> bool {
    value.len() <= 128 && !value.is_empty() && is_kebab_identifier(value)
}

fn is_kebab_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

fn is_file_extension(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn is_mime_type(value: &str) -> bool {
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'.' | b'+' | b'-')
        })
}

fn is_safe_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn is_semver_like(value: &str) -> bool {
    let (without_build, build) = match value.split_once('+') {
        Some((main, build)) => (main, Some(build)),
        None => (value, None),
    };
    if value.matches('+').count() > 1 || build.is_some_and(|part| !is_semver_identifier_set(part)) {
        return false;
    }
    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (without_build, None),
    };
    if prerelease.is_some_and(|part| !is_semver_identifier_set(part)) {
        return false;
    }
    let pieces: Vec<_> = core.split('.').collect();
    pieces.len() == 3
        && pieces
            .iter()
            .all(|piece| !piece.is_empty() && !(*piece).starts_with('0') || *piece == "0")
        && pieces
            .iter()
            .all(|piece| piece.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_semver_identifier_set(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_manifest() -> ExtensionManifest {
        ExtensionManifest {
            schema_version: EXTENSION_MANIFEST_SCHEMA_VERSION,
            id: "acme-color-audit".to_owned(),
            name: "Acme Color Audit".to_owned(),
            version: "1.2.3-beta.1+build.7".to_owned(),
            description: String::new(),
            origin: ExtensionOrigin::Local {
                path: PathBuf::from(if cfg!(windows) {
                    r"C:\\ClassicExtensions\\acme-color-audit"
                } else {
                    "/opt/classic/extensions/acme-color-audit"
                }),
            },
            requested_capabilities: BTreeSet::from([
                Capability::DocumentRead,
                Capability::DocumentWrite,
                Capability::UserInterface,
                Capability::Export,
            ]),
            contributions: vec![Contribution::Tool(ToolContribution {
                id: "color-audit".to_owned(),
                label: "Color audit".to_owned(),
                icon: Some("palette".to_owned()),
                shortcut: None,
            })],
        }
    }

    #[test]
    fn accepts_a_constrained_local_tool() {
        let manifest = local_manifest();
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\\ClassicExtensions")
        } else {
            PathBuf::from("/opt/classic/extensions")
        };
        manifest
            .validate(&ExtensionPolicy::for_local_development(root))
            .expect("development policy should consent to the tool");
    }

    #[test]
    fn rejects_sensitive_capability_without_consent() {
        let mut manifest = local_manifest();
        manifest.requested_capabilities.insert(Capability::Network);
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\\ClassicExtensions")
        } else {
            PathBuf::from("/opt/classic/extensions")
        };
        let errors = manifest
            .validate(&ExtensionPolicy::for_local_development(root))
            .expect_err("network must require explicit consent");
        assert!(
            errors
                .errors
                .iter()
                .any(|error| error.code == ValidationErrorCode::CapabilityNotConsented)
        );
    }
}

//! Reusable component primitives for Classic Studio.
//!
//! This crate models the part of a design document that is shared by
//! component definitions and their instances. It deliberately does not depend
//! on the canvas renderer or the main document crate: a host can translate a
//! [`ResolvedComponent`] into its own scene graph, persistence schema, or
//! collaboration protocol.
//!
//! A component has a base [`ComponentNode`] tree, typed component properties,
//! optional variant axes, and named variants. A [`ComponentInstance`] selects
//! a component/variant and supplies property or node overrides. Calling
//! [`ComponentLibrary::resolve`] returns an immutable, fully merged tree.
//! Invalid documents are rejected explicitly rather than silently falling back
//! to a surprising variant or property value.

use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current on-disk schema version for [`ComponentLibrary`].
pub const COMPONENT_SCHEMA_VERSION: u32 = 1;

/// A stable, document-local component or node identifier.
///
/// IDs are strings so this crate can round-trip IDs created by other systems.
/// [`new_id`] produces UUID-backed IDs suitable for newly created values.
pub type ComponentId = String;

/// Produces a stable random ID with a readable prefix.
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

/// A collection of reusable component definitions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibrary {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub definitions: Vec<ComponentDefinition>,
}

fn default_schema_version() -> u32 {
    COMPONENT_SCHEMA_VERSION
}

impl Default for ComponentLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentLibrary {
    /// Creates an empty, current-schema component library.
    pub fn new() -> Self {
        Self {
            schema_version: COMPONENT_SCHEMA_VERSION,
            definitions: Vec::new(),
        }
    }

    /// Looks up a definition by its stable ID.
    pub fn definition(&self, id: &str) -> Option<&ComponentDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    /// Looks up a mutable definition by its stable ID.
    pub fn definition_mut(&mut self, id: &str) -> Option<&mut ComponentDefinition> {
        self.definitions
            .iter_mut()
            .find(|definition| definition.id == id)
    }

    /// Adds a component only when its ID is new and its definition is valid.
    pub fn add_definition(
        &mut self,
        definition: ComponentDefinition,
    ) -> Result<(), ComponentError> {
        if self.definition(&definition.id).is_some() {
            return Err(ComponentError::DuplicateComponentId(definition.id));
        }
        definition.validate()?;
        self.definitions.push(definition);
        Ok(())
    }

    /// Validates every definition and every cross-definition invariant.
    pub fn validate(&self) -> Result<(), ComponentError> {
        if self.schema_version != COMPONENT_SCHEMA_VERSION {
            return Err(ComponentError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        let mut ids = HashSet::new();
        for definition in &self.definitions {
            if !ids.insert(definition.id.as_str()) {
                return Err(ComponentError::DuplicateComponentId(definition.id.clone()));
            }
            definition.validate()?;
        }
        for definition in &self.definitions {
            definition.validate_component_references(&ids)?;
        }
        Ok(())
    }

    /// Resolves an instance into a ready-to-render component tree.
    ///
    /// The returned content never mutates the definition or instance. Variant
    /// node overrides are applied first, then instance node overrides, then
    /// property bindings are evaluated with the final property values.
    pub fn resolve(
        &self,
        instance: &ComponentInstance,
    ) -> Result<ResolvedComponent, ComponentError> {
        self.validate()?;
        let definition = self
            .definition(&instance.component_id)
            .ok_or_else(|| ComponentError::UnknownComponent(instance.component_id.clone()))?;
        let component_ids = self
            .definitions
            .iter()
            .map(|definition| definition.id.as_str())
            .collect::<HashSet<_>>();
        validate_instance_component_references(instance, &component_ids)?;
        definition.resolve(instance)
    }
}

/// A reusable component with a base tree, properties, and optional variants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDefinition {
    pub id: ComponentId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub root: ComponentNode,
    #[serde(default)]
    pub properties: Vec<ComponentPropertyDefinition>,
    #[serde(default)]
    pub variant_properties: Vec<VariantPropertyDefinition>,
    #[serde(default)]
    pub variants: Vec<ComponentVariant>,
}

impl ComponentDefinition {
    /// Creates a component with no properties or variants.
    pub fn new(name: impl Into<String>, root: ComponentNode) -> Self {
        Self {
            id: new_id("component"),
            name: name.into(),
            description: None,
            root,
            properties: Vec::new(),
            variant_properties: Vec::new(),
            variants: Vec::new(),
        }
    }

    /// Finds a property definition by its public name.
    pub fn property(&self, name: &str) -> Option<&ComponentPropertyDefinition> {
        self.properties
            .iter()
            .find(|property| property.name == name)
    }

    /// Validates this definition in isolation.
    pub fn validate(&self) -> Result<(), ComponentError> {
        require_id("component", &self.id)?;
        if self.name.trim().is_empty() {
            return Err(ComponentError::EmptyName("component"));
        }

        let node_ids = self.root.collect_ids()?;
        let mut property_names = HashSet::new();
        for property in &self.properties {
            if property.name.trim().is_empty() {
                return Err(ComponentError::EmptyName("component property"));
            }
            if !property_names.insert(property.name.as_str()) {
                return Err(ComponentError::DuplicateProperty(property.name.clone()));
            }
            if !property.value_type.accepts(&property.default_value) {
                return Err(ComponentError::PropertyValueTypeMismatch {
                    property: property.name.clone(),
                    expected: property.value_type,
                });
            }
        }
        for property in self.root.collect_property_binding_names() {
            if !property_names.contains(property.as_str()) {
                return Err(ComponentError::UnknownProperty(property));
            }
        }

        let mut variant_property_names = HashSet::new();
        for property in &self.variant_properties {
            if property.name.trim().is_empty() {
                return Err(ComponentError::EmptyName("variant property"));
            }
            if !variant_property_names.insert(property.name.as_str()) {
                return Err(ComponentError::DuplicateVariantProperty(
                    property.name.clone(),
                ));
            }
            if property.options.is_empty() {
                return Err(ComponentError::EmptyVariantOptions(property.name.clone()));
            }
            if !property
                .options
                .iter()
                .any(|option| option == &property.default_value)
            {
                return Err(ComponentError::InvalidVariantValue {
                    property: property.name.clone(),
                    value: property.default_value.clone(),
                });
            }
        }

        let mut variant_ids = HashSet::new();
        let mut variant_signatures = HashSet::new();
        for variant in &self.variants {
            require_id("variant", &variant.id)?;
            if !variant_ids.insert(variant.id.as_str()) {
                return Err(ComponentError::DuplicateVariantId(variant.id.clone()));
            }
            self.validate_variant_selection(&variant.selection, true)?;
            validate_node_overrides(&variant.node_overrides, &node_ids)?;
            let signature = variant_signature(&variant.selection);
            if !variant_signatures.insert(signature) {
                return Err(ComponentError::DuplicateVariantSelection(
                    variant.name.clone(),
                ));
            }
        }
        Ok(())
    }

    fn validate_component_references(
        &self,
        component_ids: &HashSet<&str>,
    ) -> Result<(), ComponentError> {
        for property in &self.properties {
            validate_component_reference(&property.default_value, component_ids)?;
        }
        self.root.validate_component_references(component_ids)?;
        for variant in &self.variants {
            for override_value in variant.node_overrides.values() {
                for value in override_value.attributes.values() {
                    validate_component_reference(value, component_ids)?;
                }
            }
        }
        Ok(())
    }

    /// Resolves an instance belonging to this definition.
    pub fn resolve(
        &self,
        instance: &ComponentInstance,
    ) -> Result<ResolvedComponent, ComponentError> {
        if instance.component_id != self.id {
            return Err(ComponentError::InstanceTargetsDifferentComponent {
                expected: self.id.clone(),
                found: instance.component_id.clone(),
            });
        }
        self.validate()?;
        self.validate_instance(instance)?;

        let mut properties = BTreeMap::new();
        for definition in &self.properties {
            properties.insert(definition.name.clone(), definition.default_value.clone());
        }
        properties.extend(instance.property_overrides.clone());

        let selection = self.complete_variant_selection(&instance.variant_selection);
        let variant = self.variant_for_selection(&selection)?;
        let mut node_overrides = BTreeMap::new();
        if let Some(variant) = variant {
            node_overrides.extend(variant.node_overrides.clone());
        }
        for (node_id, override_value) in &instance.node_overrides {
            node_overrides
                .entry(node_id.clone())
                .and_modify(|current: &mut NodeOverride| current.merge_from(override_value))
                .or_insert_with(|| override_value.clone());
        }

        Ok(ResolvedComponent {
            instance_id: instance.id.clone(),
            component_id: self.id.clone(),
            component_name: self.name.clone(),
            variant_id: variant.map(|item| item.id.clone()),
            variant_selection: selection,
            properties: properties.clone(),
            root: self.root.resolve(&node_overrides, &properties),
        })
    }

    fn validate_instance(&self, instance: &ComponentInstance) -> Result<(), ComponentError> {
        require_id("instance", &instance.id)?;
        for (name, value) in &instance.property_overrides {
            let definition = self
                .property(name)
                .ok_or_else(|| ComponentError::UnknownProperty(name.clone()))?;
            if !definition.value_type.accepts(value) {
                return Err(ComponentError::PropertyValueTypeMismatch {
                    property: name.clone(),
                    expected: definition.value_type,
                });
            }
        }
        self.validate_variant_selection(&instance.variant_selection, false)?;
        let ids = self.root.collect_ids()?;
        validate_node_overrides(&instance.node_overrides, &ids)
    }

    fn validate_variant_selection(
        &self,
        selection: &BTreeMap<String, String>,
        require_complete: bool,
    ) -> Result<(), ComponentError> {
        for (name, value) in selection {
            let property = self
                .variant_properties
                .iter()
                .find(|property| property.name == *name)
                .ok_or_else(|| ComponentError::UnknownVariantProperty(name.clone()))?;
            if !property.options.iter().any(|option| option == value) {
                return Err(ComponentError::InvalidVariantValue {
                    property: name.clone(),
                    value: value.clone(),
                });
            }
        }
        if require_complete && selection.len() != self.variant_properties.len() {
            return Err(ComponentError::IncompleteVariantSelection);
        }
        Ok(())
    }

    fn complete_variant_selection(
        &self,
        selection: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut complete = self
            .variant_properties
            .iter()
            .map(|property| (property.name.clone(), property.default_value.clone()))
            .collect::<BTreeMap<_, _>>();
        complete.extend(selection.clone());
        complete
    }

    fn variant_for_selection(
        &self,
        selection: &BTreeMap<String, String>,
    ) -> Result<Option<&ComponentVariant>, ComponentError> {
        if self.variants.is_empty() {
            return Ok(None);
        }
        self.variants
            .iter()
            .find(|variant| variant.selection == *selection)
            .map(Some)
            .ok_or_else(|| ComponentError::NoMatchingVariant {
                component: self.name.clone(),
                selection: selection.clone(),
            })
    }
}

/// A typed public property exposed by a component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPropertyDefinition {
    pub name: String,
    pub value_type: PropertyValueType,
    pub default_value: PropertyValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Value families supported by public component properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyValueType {
    Text,
    Boolean,
    Number,
    Color,
    ComponentReference,
}

impl PropertyValueType {
    fn accepts(self, value: &PropertyValue) -> bool {
        match (self, value) {
            (Self::Text, PropertyValue::Text(_))
            | (Self::Boolean, PropertyValue::Boolean(_))
            | (Self::Color, PropertyValue::Color(_))
            | (Self::ComponentReference, PropertyValue::ComponentReference(_)) => true,
            (Self::Number, PropertyValue::Number(number)) => number.is_finite(),
            _ => false,
        }
    }
}

/// A serializable component property value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Text(String),
    Boolean(bool),
    Number(f64),
    Color(String),
    ComponentReference(ComponentId),
}

/// One selectable variant axis, such as `Size` or `State`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantPropertyDefinition {
    pub name: String,
    pub options: Vec<String>,
    pub default_value: String,
}

/// A named combination of variant axis values and node-level changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentVariant {
    pub id: ComponentId,
    pub name: String,
    pub selection: BTreeMap<String, String>,
    #[serde(default)]
    pub node_overrides: BTreeMap<String, NodeOverride>,
}

impl ComponentVariant {
    /// Creates a variant with a fresh ID and no visual overrides.
    pub fn new(name: impl Into<String>, selection: BTreeMap<String, String>) -> Self {
        Self {
            id: new_id("variant"),
            name: name.into(),
            selection,
            node_overrides: BTreeMap::new(),
        }
    }
}

/// A renderer-independent template node.
///
/// `kind` is intentionally an open string, allowing the host to support
/// rectangles, text, vectors, nested frames, or future node types without a
/// migration in this crate. `attributes` stores host-defined fields such as
/// `fill`, `text`, `opacity`, or `cornerRadius`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentNode {
    pub id: ComponentId,
    pub name: String,
    pub kind: String,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub attributes: BTreeMap<String, PropertyValue>,
    /// Maps an attribute name to one of the definition's public property names.
    #[serde(default)]
    pub property_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub children: Vec<ComponentNode>,
}

impl ComponentNode {
    /// Creates an empty visible template node.
    pub fn new(name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: new_id("component-node"),
            name: name.into(),
            kind: kind.into(),
            visible: true,
            attributes: BTreeMap::new(),
            property_bindings: BTreeMap::new(),
            children: Vec::new(),
        }
    }

    fn collect_ids(&self) -> Result<HashSet<String>, ComponentError> {
        let mut ids = HashSet::new();
        self.collect_ids_into(&mut ids)?;
        Ok(ids)
    }

    fn collect_ids_into(&self, ids: &mut HashSet<String>) -> Result<(), ComponentError> {
        require_id("component node", &self.id)?;
        if self.name.trim().is_empty() {
            return Err(ComponentError::EmptyName("component node"));
        }
        if self.kind.trim().is_empty() {
            return Err(ComponentError::EmptyNodeKind(self.id.clone()));
        }
        if !ids.insert(self.id.clone()) {
            return Err(ComponentError::DuplicateNodeId(self.id.clone()));
        }
        for value in self.attributes.values() {
            validate_untyped_value(value)?;
        }
        for child in &self.children {
            child.collect_ids_into(ids)?;
        }
        Ok(())
    }

    fn collect_property_binding_names(&self) -> Vec<String> {
        let mut names = self.property_bindings.values().cloned().collect::<Vec<_>>();
        for child in &self.children {
            names.extend(child.collect_property_binding_names());
        }
        names
    }

    fn validate_component_references(
        &self,
        component_ids: &HashSet<&str>,
    ) -> Result<(), ComponentError> {
        for value in self.attributes.values() {
            validate_component_reference(value, component_ids)?;
        }
        for child in &self.children {
            child.validate_component_references(component_ids)?;
        }
        Ok(())
    }

    fn resolve(
        &self,
        overrides: &BTreeMap<String, NodeOverride>,
        properties: &BTreeMap<String, PropertyValue>,
    ) -> ResolvedComponentNode {
        let mut visible = self.visible;
        let mut attributes = self.attributes.clone();
        if let Some(override_value) = overrides.get(&self.id) {
            if let Some(value) = override_value.visible {
                visible = value;
            }
            attributes.extend(override_value.attributes.clone());
        }
        for (attribute, property) in &self.property_bindings {
            if let Some(value) = properties.get(property) {
                attributes.insert(attribute.clone(), value.clone());
            }
        }
        ResolvedComponentNode {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            visible,
            attributes,
            children: self
                .children
                .iter()
                .map(|child| child.resolve(overrides, properties))
                .collect(),
        }
    }
}

/// Changes made to a template node by a variant or an instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default)]
    pub attributes: BTreeMap<String, PropertyValue>,
}

impl NodeOverride {
    /// Applies fields from `other`, preserving fields that `other` does not set.
    pub fn merge_from(&mut self, other: &Self) {
        if other.visible.is_some() {
            self.visible = other.visible;
        }
        self.attributes.extend(other.attributes.clone());
    }
}

/// A placed reference to a reusable component definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInstance {
    pub id: ComponentId,
    pub component_id: ComponentId,
    #[serde(default)]
    pub variant_selection: BTreeMap<String, String>,
    #[serde(default)]
    pub property_overrides: BTreeMap<String, PropertyValue>,
    #[serde(default)]
    pub node_overrides: BTreeMap<String, NodeOverride>,
}

impl ComponentInstance {
    /// Creates an instance that uses the definition's default properties and variant values.
    pub fn new(component_id: impl Into<String>) -> Self {
        Self {
            id: new_id("instance"),
            component_id: component_id.into(),
            variant_selection: BTreeMap::new(),
            property_overrides: BTreeMap::new(),
            node_overrides: BTreeMap::new(),
        }
    }
}

/// The immutable result of resolving a [`ComponentInstance`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedComponent {
    pub instance_id: ComponentId,
    pub component_id: ComponentId,
    pub component_name: String,
    pub variant_id: Option<ComponentId>,
    pub variant_selection: BTreeMap<String, String>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub root: ResolvedComponentNode,
}

/// A renderer-ready component node with bindings and overrides evaluated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedComponentNode {
    pub id: ComponentId,
    pub name: String,
    pub kind: String,
    pub visible: bool,
    pub attributes: BTreeMap<String, PropertyValue>,
    pub children: Vec<ResolvedComponentNode>,
}

/// Validation or resolution failure.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentError {
    UnsupportedSchemaVersion(u32),
    EmptyId(&'static str),
    EmptyName(&'static str),
    EmptyNodeKind(String),
    DuplicateComponentId(String),
    DuplicateNodeId(String),
    DuplicateProperty(String),
    DuplicateVariantProperty(String),
    DuplicateVariantId(String),
    DuplicateVariantSelection(String),
    EmptyVariantOptions(String),
    NonFiniteNumber,
    InvalidVariantValue {
        property: String,
        value: String,
    },
    IncompleteVariantSelection,
    UnknownComponent(String),
    UnknownReferencedComponent(String),
    UnknownProperty(String),
    UnknownVariantProperty(String),
    PropertyValueTypeMismatch {
        property: String,
        expected: PropertyValueType,
    },
    UnknownNode(String),
    NoMatchingVariant {
        component: String,
        selection: BTreeMap<String, String>,
    },
    InstanceTargetsDifferentComponent {
        expected: String,
        found: String,
    },
}

impl fmt::Display for ComponentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported component schema version {version}")
            }
            Self::EmptyId(kind) => write!(formatter, "{kind} ID cannot be empty"),
            Self::EmptyName(kind) => write!(formatter, "{kind} name cannot be empty"),
            Self::EmptyNodeKind(id) => write!(formatter, "component node {id} has no kind"),
            Self::DuplicateComponentId(id) => write!(formatter, "duplicate component ID {id}"),
            Self::DuplicateNodeId(id) => write!(formatter, "duplicate component node ID {id}"),
            Self::DuplicateProperty(name) => {
                write!(formatter, "duplicate component property {name}")
            }
            Self::DuplicateVariantProperty(name) => {
                write!(formatter, "duplicate variant property {name}")
            }
            Self::DuplicateVariantId(id) => write!(formatter, "duplicate variant ID {id}"),
            Self::DuplicateVariantSelection(name) => {
                write!(formatter, "duplicate variant selection for {name}")
            }
            Self::EmptyVariantOptions(name) => {
                write!(formatter, "variant property {name} has no options")
            }
            Self::NonFiniteNumber => write!(formatter, "component number values must be finite"),
            Self::InvalidVariantValue { property, value } => write!(
                formatter,
                "{value} is not a valid value for variant property {property}"
            ),
            Self::IncompleteVariantSelection => write!(
                formatter,
                "a component variant must select every variant property"
            ),
            Self::UnknownComponent(id) => write!(formatter, "unknown component {id}"),
            Self::UnknownReferencedComponent(id) => {
                write!(formatter, "unknown referenced component {id}")
            }
            Self::UnknownProperty(name) => write!(formatter, "unknown component property {name}"),
            Self::UnknownVariantProperty(name) => {
                write!(formatter, "unknown variant property {name}")
            }
            Self::PropertyValueTypeMismatch { property, expected } => write!(
                formatter,
                "component property {property} does not match expected type {expected:?}"
            ),
            Self::UnknownNode(id) => write!(formatter, "unknown component node {id}"),
            Self::NoMatchingVariant { component, .. } => write!(
                formatter,
                "no variant in {component} matches the selected values"
            ),
            Self::InstanceTargetsDifferentComponent { expected, found } => {
                write!(formatter, "instance targets {found}, expected {expected}")
            }
        }
    }
}

impl Error for ComponentError {}

fn require_id(kind: &'static str, id: &str) -> Result<(), ComponentError> {
    if id.trim().is_empty() {
        Err(ComponentError::EmptyId(kind))
    } else {
        Ok(())
    }
}

fn validate_node_overrides(
    overrides: &BTreeMap<String, NodeOverride>,
    node_ids: &HashSet<String>,
) -> Result<(), ComponentError> {
    for (id, override_value) in overrides {
        if !node_ids.contains(id) {
            return Err(ComponentError::UnknownNode(id.clone()));
        }
        for value in override_value.attributes.values() {
            validate_untyped_value(value)?;
        }
    }
    Ok(())
}

fn validate_untyped_value(value: &PropertyValue) -> Result<(), ComponentError> {
    if let PropertyValue::Number(number) = value {
        if !number.is_finite() {
            return Err(ComponentError::NonFiniteNumber);
        }
    }
    Ok(())
}

fn validate_component_reference(
    value: &PropertyValue,
    component_ids: &HashSet<&str>,
) -> Result<(), ComponentError> {
    if let PropertyValue::ComponentReference(id) = value {
        if !component_ids.contains(id.as_str()) {
            return Err(ComponentError::UnknownReferencedComponent(id.clone()));
        }
    }
    Ok(())
}

fn validate_instance_component_references(
    instance: &ComponentInstance,
    component_ids: &HashSet<&str>,
) -> Result<(), ComponentError> {
    for value in instance.property_overrides.values() {
        validate_component_reference(value, component_ids)?;
    }
    for override_value in instance.node_overrides.values() {
        for value in override_value.attributes.values() {
            validate_component_reference(value, component_ids)?;
        }
    }
    Ok(())
}

fn default_visible() -> bool {
    true
}

fn variant_signature(selection: &BTreeMap<String, String>) -> String {
    // BTreeMap gives deterministic ordering. The separator is length-prefixed
    // so user-entered strings cannot create ambiguous signatures.
    selection
        .iter()
        .map(|(key, value)| format!("{}:{key}{}:{value}", key.len(), value.len()))
        .collect::<Vec<_>>()
        .join("|")
}

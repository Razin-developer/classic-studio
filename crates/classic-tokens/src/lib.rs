//! Serializable design tokens for Classic Studio.
//!
//! The crate intentionally has no renderer or UI dependency. Token collections
//! use ordered maps so serialized output and lookup order are deterministic.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// An address that uniquely identifies one token in a library.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TokenAddress {
    pub collection: String,
    pub name: String,
}

impl TokenAddress {
    pub fn new(collection: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            collection: collection.into(),
            name: name.into(),
        }
    }
}

impl fmt::Display for TokenAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.collection, self.name)
    }
}

/// A reference to another token. A missing collection resolves in the current
/// collection, which makes local aliases concise and keeps exported documents
/// portable when a collection is renamed as a unit.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TokenReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    pub name: String,
}

impl TokenReference {
    pub fn local(name: impl Into<String>) -> Self {
        Self {
            collection: None,
            name: name.into(),
        }
    }

    pub fn external(collection: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            collection: Some(collection.into()),
            name: name.into(),
        }
    }

    pub fn address_in(&self, current_collection: &str) -> TokenAddress {
        TokenAddress::new(
            self.collection.as_deref().unwrap_or(current_collection),
            &self.name,
        )
    }
}

/// An sRGBA design-token color. Channels are unpremultiplied and use 8-bit
/// values to avoid introducing renderer-specific color representations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    #[serde(default = "TokenColor::opaque")]
    pub alpha: u8,
}

impl TokenColor {
    pub const fn opaque() -> u8 {
        u8::MAX
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl Default for TokenColor {
    fn default() -> Self {
        Self::rgba(0, 0, 0, Self::opaque())
    }
}

/// The terminal category of a token value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Color,
    Number,
    String,
    Reference,
}

/// A color, number, string, or alias to another token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum TokenValue {
    Color(TokenColor),
    Number(f64),
    String(String),
    Reference(TokenReference),
}

impl TokenValue {
    pub const fn kind(&self) -> TokenKind {
        match self {
            Self::Color(_) => TokenKind::Color,
            Self::Number(_) => TokenKind::Number,
            Self::String(_) => TokenKind::String,
            Self::Reference(_) => TokenKind::Reference,
        }
    }
}

/// A named style token stored in a collection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleToken {
    pub name: String,
    pub value: TokenValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl StyleToken {
    pub fn new(name: impl Into<String>, value: TokenValue) -> Self {
        Self {
            name: name.into(),
            value,
            description: None,
        }
    }
}

/// An ordered, named collection of tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenCollection {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tokens: BTreeMap<String, StyleToken>,
}

impl TokenCollection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            tokens: BTreeMap::new(),
        }
    }

    /// Inserts a new token. Existing token names are rejected deliberately so
    /// callers must choose between an insert and an explicit replacement.
    pub fn insert(&mut self, token: StyleToken) -> Result<(), TokenError> {
        validate_name("token", &token.name)?;
        if self.tokens.contains_key(&token.name) {
            return Err(TokenError::DuplicateToken {
                collection: self.name.clone(),
                name: token.name,
            });
        }
        self.tokens.insert(token.name.clone(), token);
        Ok(())
    }

    /// Replaces or inserts a token and returns its previous value, if present.
    pub fn upsert(&mut self, token: StyleToken) -> Result<Option<StyleToken>, TokenError> {
        validate_name("token", &token.name)?;
        Ok(self.tokens.insert(token.name.clone(), token))
    }

    pub fn get(&self, name: &str) -> Option<&StyleToken> {
        self.tokens.get(name)
    }
}

/// A complete deterministic library of named token collections.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenLibrary {
    #[serde(default)]
    pub collections: BTreeMap<String, TokenCollection>,
}

impl TokenLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a collection. Collection names are unique library-wide.
    pub fn insert_collection(&mut self, collection: TokenCollection) -> Result<(), TokenError> {
        validate_name("collection", &collection.name)?;
        if self.collections.contains_key(&collection.name) {
            return Err(TokenError::DuplicateCollection {
                name: collection.name,
            });
        }
        self.collections.insert(collection.name.clone(), collection);
        Ok(())
    }

    /// Replaces or inserts a collection and returns its old value, if present.
    pub fn upsert_collection(
        &mut self,
        collection: TokenCollection,
    ) -> Result<Option<TokenCollection>, TokenError> {
        validate_name("collection", &collection.name)?;
        Ok(self.collections.insert(collection.name.clone(), collection))
    }

    pub fn collection(&self, name: &str) -> Option<&TokenCollection> {
        self.collections.get(name)
    }

    pub fn collection_mut(&mut self, name: &str) -> Option<&mut TokenCollection> {
        self.collections.get_mut(name)
    }

    /// Gets the raw token without expanding aliases.
    pub fn get(&self, address: &TokenAddress) -> Result<&StyleToken, TokenError> {
        let collection = self.collections.get(&address.collection).ok_or_else(|| {
            TokenError::CollectionNotFound {
                name: address.collection.clone(),
            }
        })?;
        collection
            .tokens
            .get(&address.name)
            .ok_or_else(|| TokenError::TokenNotFound {
                address: address.clone(),
            })
    }

    /// Resolves aliases to their terminal value and records the deterministic
    /// traversal path from the requested token to that terminal token.
    pub fn resolve(&self, address: &TokenAddress) -> Result<ResolvedToken, TokenError> {
        let mut active = BTreeSet::new();
        let mut chain = Vec::new();
        self.resolve_inner(address, &mut active, &mut chain)
    }

    /// Validates names, map-key integrity, finite numeric values, reference
    /// targets, and every reference cycle in the library.
    pub fn validate(&self) -> Result<(), TokenError> {
        for (collection_key, collection) in &self.collections {
            validate_name("collection", collection_key)?;
            if collection_key != &collection.name {
                return Err(TokenError::CollectionKeyMismatch {
                    key: collection_key.clone(),
                    name: collection.name.clone(),
                });
            }

            for (token_key, token) in &collection.tokens {
                validate_name("token", token_key)?;
                if token_key != &token.name {
                    return Err(TokenError::TokenKeyMismatch {
                        collection: collection_key.clone(),
                        key: token_key.clone(),
                        name: token.name.clone(),
                    });
                }
                if let TokenValue::Number(value) = &token.value {
                    if !value.is_finite() {
                        return Err(TokenError::NonFiniteNumber {
                            address: TokenAddress::new(collection_key, token_key),
                        });
                    }
                }
                self.resolve(&TokenAddress::new(collection_key, token_key))?;
            }
        }
        Ok(())
    }

    fn resolve_inner(
        &self,
        address: &TokenAddress,
        active: &mut BTreeSet<TokenAddress>,
        chain: &mut Vec<TokenAddress>,
    ) -> Result<ResolvedToken, TokenError> {
        if !active.insert(address.clone()) {
            let cycle_start = chain
                .iter()
                .position(|candidate| candidate == address)
                .unwrap_or(0);
            let mut cycle = chain[cycle_start..].to_vec();
            cycle.push(address.clone());
            return Err(TokenError::ReferenceCycle { cycle });
        }

        chain.push(address.clone());
        let value = self.get(address)?.value.clone();
        let result = match value {
            TokenValue::Reference(reference) => {
                let target = reference.address_in(&address.collection);
                self.resolve_inner(&target, active, chain)
            }
            value => Ok(ResolvedToken {
                address: address.clone(),
                value,
                chain: chain.clone(),
            }),
        };
        chain.pop();
        active.remove(address);
        result
    }
}

/// A resolved terminal token and the reference chain used to reach it.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedToken {
    pub address: TokenAddress,
    pub value: TokenValue,
    pub chain: Vec<TokenAddress>,
}

impl ResolvedToken {
    pub const fn kind(&self) -> TokenKind {
        self.value.kind()
    }
}

/// Errors emitted while inserting, validating, looking up, or resolving tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenError {
    InvalidName {
        kind: &'static str,
        value: String,
    },
    DuplicateCollection {
        name: String,
    },
    DuplicateToken {
        collection: String,
        name: String,
    },
    CollectionNotFound {
        name: String,
    },
    TokenNotFound {
        address: TokenAddress,
    },
    CollectionKeyMismatch {
        key: String,
        name: String,
    },
    TokenKeyMismatch {
        collection: String,
        key: String,
        name: String,
    },
    NonFiniteNumber {
        address: TokenAddress,
    },
    ReferenceCycle {
        cycle: Vec<TokenAddress>,
    },
}

impl fmt::Display for TokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { kind, value } => {
                write!(formatter, "{kind} name must not be blank: {value:?}")
            }
            Self::DuplicateCollection { name } => {
                write!(formatter, "collection already exists: {name}")
            }
            Self::DuplicateToken { collection, name } => {
                write!(formatter, "token already exists: {collection}/{name}")
            }
            Self::CollectionNotFound { name } => write!(formatter, "collection not found: {name}"),
            Self::TokenNotFound { address } => write!(formatter, "token not found: {address}"),
            Self::CollectionKeyMismatch { key, name } => {
                write!(
                    formatter,
                    "collection key {key:?} does not match name {name:?}"
                )
            }
            Self::TokenKeyMismatch {
                collection,
                key,
                name,
            } => write!(
                formatter,
                "token key {collection}/{key:?} does not match name {name:?}"
            ),
            Self::NonFiniteNumber { address } => {
                write!(formatter, "token number is not finite: {address}")
            }
            Self::ReferenceCycle { cycle } => {
                let path = cycle
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(formatter, "token reference cycle: {path}")
            }
        }
    }
}

impl std::error::Error for TokenError {}

fn validate_name(kind: &'static str, value: &str) -> Result<(), TokenError> {
    if value.trim().is_empty() {
        Err(TokenError::InvalidName {
            kind,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

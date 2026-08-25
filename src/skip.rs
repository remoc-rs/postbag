//! Skipping struct fields that carry no information.
//!
//! Serde omits a struct field whose
//! [`skip_serializing_if`](https://serde.rs/field-attrs.html#skip_serializing_if)
//! predicate holds. This is supported in [`Full`](crate::cfg::Full) configuration
//! but not under [`Slim`](crate::cfg::Slim).
//!
//! The predicates in this module allow to query whether the currently running
//! Postbag serialization supports skipping fields.
//!
//! # Example
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Person {
//!     name: String,
//!     #[serde(skip_serializing_if = "postbag::skip::Option::is_none", default)]
//!     nickname: Option<String>,
//!     #[serde(skip_serializing_if = "postbag::skip::Vec::is_empty", default)]
//!     pets: Vec<String>,
//! }
//! ```
//!
//! # Other data formats
//!
//! [`is_allowed`] holds while no Postbag serialization is in progress, so a
//! type using these predicates keeps skipping its fields as usual under
//! another data format, such as JSON.

use crate::info::{self, Direction};

/// Whether a struct field may be omitted from the data currently being written.
///
/// This matches [`Cfg::allow_skip`](crate::cfg::Cfg::allow_skip) when serializing with
/// Postbag. Otherwise it is always `true`.
///
/// ```rust
/// # use serde::Serialize;
/// fn is_zero(value: &u32) -> bool {
///     *value == 0 && postbag::skip::is_allowed()
/// }
///
/// #[derive(Serialize)]
/// struct Measurement {
///     #[serde(skip_serializing_if = "is_zero", default)]
///     offset: u32,
/// }
/// ```
///
/// The predicates of this module are defined in just this way.
pub fn is_allowed() -> bool {
    match info::active() {
        Some(info) if info.direction == Direction::Serialize => info.allow_skip,
        _ => true,
    }
}

/// Whether the value is its [default](Default) and may be omitted.
///
/// This pairs with serde's [`default`](https://serde.rs/field-attrs.html#default).
///
/// ```rust
/// # use serde::{Serialize, Deserialize};
/// #[derive(Serialize, Deserialize)]
/// struct Settings {
///     name: String,
///     #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
///     retries: u32,
///     #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
///     verbose: bool,
/// }
/// ```
pub fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    is_allowed() && *value == T::default()
}

/// Predicates for [`Option`](core::option::Option).
pub enum Option {}

impl Option {
    /// Whether the option holds no value and may be omitted.
    ///
    /// Stands in for
    /// [`Option::is_none`](core::option::Option::is_none) as a
    /// `skip_serializing_if` predicate.
    pub fn is_none<T>(value: &core::option::Option<T>) -> bool {
        value.is_none() && is_allowed()
    }
}

/// Predicates for [`String`](std::string::String) and other string types.
pub enum String {}

impl String {
    /// Whether the string is empty and may be omitted.
    ///
    /// Stands in for [`str::is_empty`] as a `skip_serializing_if` predicate.
    /// Applies to every type that dereferences to [`str`], such as
    /// [`String`](std::string::String), [`Box<str>`](std::boxed::Box) and
    /// [`Cow<str>`](std::borrow::Cow).
    pub fn is_empty(value: &str) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`Vec`](std::vec::Vec) and other slice types.
pub enum Vec {}

impl Vec {
    /// Whether the sequence is empty and may be omitted.
    ///
    /// Stands in for [`slice::is_empty`] as a `skip_serializing_if` predicate.
    /// Applies to every type that dereferences to a slice, such as
    /// [`Vec`](std::vec::Vec), [`Box<[T]>`](std::boxed::Box) and
    /// [`Cow<[T]>`](std::borrow::Cow).
    pub fn is_empty<T>(value: &[T]) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`VecDeque`](std::collections::VecDeque).
pub enum VecDeque {}

impl VecDeque {
    /// Whether the deque is empty and may be omitted.
    pub fn is_empty<T>(value: &std::collections::VecDeque<T>) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`HashMap`](std::collections::HashMap).
pub enum HashMap {}

impl HashMap {
    /// Whether the map is empty and may be omitted.
    pub fn is_empty<K, V, S>(value: &std::collections::HashMap<K, V, S>) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`HashSet`](std::collections::HashSet).
pub enum HashSet {}

impl HashSet {
    /// Whether the set is empty and may be omitted.
    pub fn is_empty<T, S>(value: &std::collections::HashSet<T, S>) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`BTreeMap`](std::collections::BTreeMap).
pub enum BTreeMap {}

impl BTreeMap {
    /// Whether the map is empty and may be omitted.
    pub fn is_empty<K, V>(value: &std::collections::BTreeMap<K, V>) -> bool {
        value.is_empty() && is_allowed()
    }
}

/// Predicates for [`BTreeSet`](std::collections::BTreeSet).
pub enum BTreeSet {}

impl BTreeSet {
    /// Whether the set is empty and may be omitted.
    pub fn is_empty<T>(value: &std::collections::BTreeSet<T>) -> bool {
        value.is_empty() && is_allowed()
    }
}

//! Recoverable deserialization.
//!
//! When a value fails to deserialize, the error normally aborts deserialization
//! of the whole data structure, because a deserializer cannot know where the
//! undecodable value ends and thus cannot continue after it. Consequently, a
//! change to a type that breaks [forward compatibility](crate#backwards-and-forwards-compatibility)
//! renders every enclosing value undecodable as well, even when the enclosing
//! type itself did not change.
//!
//! A value wrapped in [`Recoverable`] is deserialized in isolation, so that a
//! failure is confined to it. The rest of the enclosing data structure is
//! deserialized as usual and the value itself is replaced by one obtained from
//! a [recovery policy](Recover), which by default is [`Default::default`].
//!
//! ```rust
//! # use serde::{Serialize, Deserialize};
//! # #[derive(Default, Serialize, Deserialize)]
//! # struct B { x: u32 }
//! use postbag::recoverable::Recoverable;
//!
//! #[derive(Serialize, Deserialize)]
//! struct A {
//!     a: u32,
//!     b: Recoverable<B>,
//!     c: u16,
//! }
//! ```
//!
//! Should `B` change incompatibly, `a` and `c` still deserialize correctly and
//! `b` becomes `B::default()`.
//!
//! # Choosing a recovery policy
//!
//! Implement [`Recover`] on a marker type and name it as the second type
//! parameter. The policy is passed the error and either provides a replacement
//! value or returns the error, which propagates as usual.
//!
//! ```rust
//! # use serde::{Serialize, Deserialize};
//! # #[derive(Serialize, Deserialize)]
//! # struct B { x: u32 }
//! use postbag::recoverable::{Recover, Recoverable};
//!
//! struct WarnAndUseZero;
//!
//! impl Recover<B> for WarnAndUseZero {
//!     fn recover<E: serde::de::Error>(err: E) -> Result<B, E> {
//!         eprintln!("field b was dropped: {err}");
//!         Ok(B { x: 0 })
//!     }
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct A {
//!     b: Recoverable<B, WarnAndUseZero>,
//! }
//! ```
//!
//! # Use without a wrapper type
//!
//! To keep the field type unchanged, use this module with
//! `#[serde(with = "postbag::recoverable")]` for the default policy, or
//! [`With`] for a specific one.
//!
//! ```rust
//! # use serde::{Serialize, Deserialize};
//! # #[derive(Default, Serialize, Deserialize)]
//! # struct B { x: u32 }
//! # #[derive(Default, Serialize, Deserialize)]
//! # struct C { x: u32 }
//! # struct MyPolicy;
//! # impl postbag::recoverable::Recover<C> for MyPolicy {
//! #     fn recover<E: serde::de::Error>(_err: E) -> Result<C, E> { Ok(C::default()) }
//! # }
//! #[derive(Serialize, Deserialize)]
//! struct A {
//!     #[serde(with = "postbag::recoverable")]
//!     b: B,
//!     #[serde(with = "postbag::recoverable::With::<MyPolicy>")]
//!     c: C,
//! }
//! ```
//!
//! Both forms produce the same representation as [`Recoverable`] itself, thus
//! a field can be switched between them without breaking compatibility. In
//! exchange for the unchanged field type they lose the ability to tell whether
//! recovery took place, which [`Recoverable::is_recovered`] provides.
//!
//! # Retrofitting recoverability to an existing type
//!
//! Under [`Full`](crate::cfg::Full) a struct field can be wrapped without
//! changing the serialized representation, so data written before the change
//! is still read after it:
//!
//! ```rust
//! # use serde::{Serialize, Deserialize};
//! # #[derive(Default, Serialize, Deserialize)]
//! # struct B { x: u32 }
//! use postbag::recoverable::Recoverable;
//!
//! #[derive(Serialize, Deserialize)]
//! struct Before {
//!     a: u32,
//!     b: B,
//!     c: u16,
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct After {
//!     a: u32,
//!     b: Recoverable<B>, // the only change
//!     c: u16,
//! }
//!
//! let bytes = postbag::to_full_vec(&Before { a: 1, b: B { x: 7 }, c: 2 })?;
//! let after: After = postbag::from_full_slice(&bytes)?;
//! assert_eq!(after.b.x, 7);
//! # Ok::<(), postbag::Error>(())
//! ```
//!
//! The same holds for the payload of an enum variant, `V(B)` becoming
//! `V(Recoverable<B>)`.
//!
//!

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Provides a replacement for a value that failed to deserialize.
///
/// Implemented on a marker type, which is then named as the policy of a
/// [`Recoverable`] value.
pub trait Recover<T> {
    /// Returns the value to use in place of the one that failed to deserialize.
    ///
    /// Return `Ok(_)` to replace the value that failed deserialization with the
    /// returned value.
    ///
    /// Return `Err(err)` (or another error) to propagate it, aborting
    /// deserialization of the enclosing data structure as it would without
    /// recovery.
    fn recover<E: serde::de::Error>(err: E) -> Result<T, E>;
}

/// Recovery policy that replaces a value that failed to deserialize by
/// [`Default::default`].
///
/// This is the default policy of [`Recoverable`].
pub struct RecoverDefault;

impl<T: Default> Recover<T> for RecoverDefault {
    fn recover<E: serde::de::Error>(_err: E) -> Result<T, E> {
        Ok(T::default())
    }
}

/// A value that is deserialized in isolation, so that a failure to deserialize
/// it does not affect the enclosing data structure.
///
/// See the [module-level documentation](self) for details.
pub struct Recoverable<T, P = RecoverDefault> {
    value: T,
    recovered: bool,
    _policy: PhantomData<fn() -> P>,
}

impl Recoverable<(), ()> {
    pub(crate) const NEWTYPE_NAME: &str = "$postbag::recoverable::Recoverable";
}

impl<T, P> Recoverable<T, P> {
    /// Wraps the value.
    pub fn new(value: T) -> Self {
        Self { value, recovered: false, _policy: PhantomData }
    }

    /// Returns the contained value.
    pub fn into_inner(this: Self) -> T {
        this.value
    }

    /// Whether the value was provided by the [recovery policy](Recover)
    /// because deserialization of the contained value failed.
    ///
    /// Always `false` for a value that was not deserialized.
    pub fn is_recovered(this: &Self) -> bool {
        this.recovered
    }

    /// Wraps a value provided by the recovery policy.
    fn recovered(value: T) -> Self {
        Self { value, recovered: true, _policy: PhantomData }
    }
}

impl<T, P> From<T> for Recoverable<T, P> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T, P> Deref for Recoverable<T, P> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, P> DerefMut for Recoverable<T, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: fmt::Debug, P> fmt::Debug for Recoverable<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recoverable").field("value", &self.value).field("recovered", &self.recovered).finish()
    }
}

impl<T: Clone, P> Clone for Recoverable<T, P> {
    fn clone(&self) -> Self {
        Self { value: self.value.clone(), recovered: self.recovered, _policy: PhantomData }
    }
}

impl<T: Copy, P> Copy for Recoverable<T, P> {}

impl<T: Default, P> Default for Recoverable<T, P> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: PartialEq, P> PartialEq for Recoverable<T, P> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq, P> Eq for Recoverable<T, P> {}

impl<T: PartialOrd, P> PartialOrd for Recoverable<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord, P> Ord for Recoverable<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T: Hash, P> Hash for Recoverable<T, P> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T: Serialize, P> Serialize for Recoverable<T, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_struct(Recoverable::NEWTYPE_NAME, &self.value)
    }
}

impl<'de, T: Deserialize<'de>, P: Recover<T>> Deserialize<'de> for Recoverable<T, P> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_newtype_struct(Recoverable::NEWTYPE_NAME, RecoverableVisitor(PhantomData))
    }
}

struct RecoverableVisitor<T, P>(PhantomData<fn() -> (T, P)>);

impl<'de, T: Deserialize<'de>, P: Recover<T>> Visitor<'de> for RecoverableVisitor<T, P> {
    type Value = Recoverable<T, P>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a recoverable value")
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        match T::deserialize(deserializer) {
            Ok(value) => Ok(Recoverable::new(value)),
            Err(err) => P::recover(err).map(Recoverable::recovered),
        }
    }
}

/// A value that is deserialized in isolation and implements its own [`Recover::recover`] function
/// that is called when deserialization fails to provide a replacement value.
pub type SelfRecoverable<T> = Recoverable<T, T>;

// ============================================================================
// Functions implementing #[serde(with = "postbag::recoverable::With::<_>")]
// ============================================================================

/// Applies a [recovery policy](Recover) to a field, leaving its type unchanged.
///
/// For use with `#[serde(with = "postbag::recoverable::With::<MyPolicy>")]`;
/// see the [module-level documentation](self#use-without-a-wrapper-type).
pub struct With<P>(PhantomData<fn() -> P>);

impl<P> With<P> {
    /// Serializes the value so that it can be recovered from when
    /// deserializing it fails.
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize + ?Sized,
        S: Serializer,
    {
        Recoverable::<&T, P>::new(value).serialize(serializer)
    }

    /// Deserializes the value, applying the recovery policy `P` if that fails.
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: Deserialize<'de>,
        P: Recover<T>,
        D: Deserializer<'de>,
    {
        Recoverable::<T, P>::deserialize(deserializer).map(Recoverable::into_inner)
    }
}

// ============================================================================
// Functions implementing #[serde(with = "postbag::recoverable")]
// ============================================================================

/// Serializes the value so that it can be recovered from when deserializing it
/// fails.
///
/// For use with `#[serde(with = "postbag::recoverable")]`; see the
/// [module-level documentation](self#use-without-a-wrapper-type).
pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize + ?Sized,
    S: Serializer,
{
    With::<RecoverDefault>::serialize(value, serializer)
}

/// Deserializes the value, replacing it by [`Default::default`] if that fails.
///
/// For use with `#[serde(with = "postbag::recoverable")]`; see the
/// [module-level documentation](self#use-without-a-wrapper-type).
pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: Deserializer<'de>,
{
    With::<RecoverDefault>::deserialize(deserializer)
}

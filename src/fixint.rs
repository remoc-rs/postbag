//! # Fixed Size Integers
//!
//! In some cases, the use of variably length encoded data may not be
//! preferable. These modules, for use with `#[serde(with = "postbag::fixint")]`
//! "opt out" of variable length encoding.
//!
//! Disables varint serialization/deserialization for the specified integer
//! field. The integer will always be serialized in the same way as a fixed
//! size array.
//!
//!
//! ```rust
//! # use serde::Serialize;
//! #[derive(Serialize)]
//! pub struct DefinitelyFixint {
//!     #[serde(with = "postbag::fixint")]
//!     x: u16,
//! }
//! ```
//!
//! The attribute applies to the field itself, so it cannot reach the integers
//! inside a container. Wrap them in [`Fixint`] instead, which opts out of
//! variable length encoding wherever it appears.
//!
//! ```rust
//! # use serde::Serialize;
//! # use postbag::fixint::Fixint;
//! #[derive(Serialize)]
//! pub struct DefinitelyFixints {
//!     xs: Vec<Fixint<u16>>,
//! }
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serialize the integer value as a fixed-size array.
pub fn serialize<S, T>(val: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Copy,
    Fixint<T>: Serialize,
{
    Fixint(*val).serialize(serializer)
}

/// Deserialize the integer value from a fixed-size array.
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    Fixint<T>: Deserialize<'de>,
{
    Fixint::<T>::deserialize(deserializer).map(|x| x.0)
}

/// An integer serialized as a fixed-size array.
///
/// This wrapper opts the integer it holds out of variable length encoding,
/// like `#[serde(with = "postbag::fixint")]` does for a field.
///
/// It is implemented for all integer types.
///
/// ```rust
/// # use postbag::fixint::Fixint;
/// let xs: Vec<Fixint<u32>> = vec![Fixint(1), Fixint(2), 3.into()];
/// let sum: u32 = xs.iter().map(|x| x.0).sum();
/// assert_eq!(sum, 6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Fixint<T>(pub T);

impl<T> Fixint<T> {
    /// Wraps the integer value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the wrapped integer value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Fixint<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

macro_rules! impl_fixint {
    ($( $int:ty ),*) => {
        $(
            impl Serialize for Fixint<$int> {

                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    self.0.to_le_bytes().serialize(serializer)
                }
            }

            impl<'de> Deserialize<'de> for Fixint<$int> {

                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    <_ as Deserialize>::deserialize(deserializer)
                        .map(<$int>::from_le_bytes)
                        .map(Self)
                }
            }
        )*
    };
}

macro_rules! impl_fixint_ptr_width {
    ($( $int:ty as $fixed:ty ),*) => {
        $(
            impl Serialize for Fixint<$int> {

                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    let value = <$fixed>::try_from(self.0).map_err(|_| {
                        <S::Error as serde::ser::Error>::custom(concat!(stringify!($int), " overflow"))
                    })?;
                    value.to_le_bytes().serialize(serializer)
                }
            }

            impl<'de> Deserialize<'de> for Fixint<$int> {

                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let value = <_ as Deserialize>::deserialize(deserializer).map(<$fixed>::from_le_bytes)?;
                    <$int>::try_from(value).map(Self).map_err(|_| {
                        <D::Error as serde::de::Error>::custom(concat!(stringify!($int), " overflow"))
                    })
                }
            }
        )*
    };
}

impl_fixint![i8, i16, i32, i64, i128, u8, u16, u32, u64, u128];
impl_fixint_ptr_width![usize as u64, isize as i64];

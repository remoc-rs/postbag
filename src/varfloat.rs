//! # Variable Size Floats
//!
//! In some cases, the use of fixed size floating point data may be wasteful.
//! These modules, for use with `#[serde(with = "postbag::varfloat")]` "opt in"
//! to variable length encoding.
//!
//! Enables variable length serialization/deserialization for the specified
//! floating point field. The encoding is lossless and  preserves the bit
//! pattern of every value, including quiet and signaling NaN payloads,
//! both infinities, negative zero and subnormal values.
//!
//! Whether this saves space depends entirely on the data:
//!
//! | Value | `f64` bytes | `f32` bytes |
//! | --- | ---: | ---: |
//! | `0.0` | 1 | 1 |
//! | `-0.0` | 2 | 2 |
//! | `1.0` | 3 | 3 |
//! | `-0.5` | 3 | 2 |
//! | `INFINITY` | 3 | 3 |
//! | `NAN` | 3 | 3 |
//! | `1234.0 / 32768.0` | 4 | 4 |
//! | `0.1` | 9 | 5 |
//! | `PI` | 9 | 5 |
//! | unencoded | 8 | 4 |
//!
//! So this is worth applying to values that carry fewer significant bits than
//! their type provides, such as data quantized to a power of two, values that
//! are whole numbers, and fields that are zero most of the time.
//!
//! ```rust
//! # use serde::Serialize;
//! #[derive(Serialize)]
//! pub struct DefinitelyVarfloat {
//!     #[serde(with = "postbag::varfloat")]
//!     x: f64,
//! }
//! ```
//!
//! The attribute applies to the field itself, so it cannot reach the floats
//! inside a container. Wrap them in [`Varfloat`] instead, which opts in to
//! variable length encoding wherever it appears.
//!
//! ```rust
//! # use serde::Serialize;
//! # use postbag::varfloat::Varfloat;
//! #[derive(Serialize)]
//! pub struct DefinitelyVarfloats {
//!     xs: Vec<Varfloat<f32>>,
//! }
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Serialize the floating point value as a byte string.
pub fn serialize<S, T>(val: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Copy,
    Varfloat<T>: Serialize,
{
    Varfloat(*val).serialize(serializer)
}

/// Deserialize the floating point value from a byte string.
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    Varfloat<T>: Deserialize<'de>,
{
    Varfloat::<T>::deserialize(deserializer).map(|x| x.0)
}

/// A floating point value serialized using variable length encoding.
///
/// This wrapper opts the value it holds in to variable length encoding, like
/// `#[serde(with = "postbag::varfloat")]` does for a field. Since it is part of
/// the type, it also applies inside containers such as `Vec<Varfloat<f64>>` or
/// `Option<Varfloat<f64>>`, which the attribute cannot reach.
///
/// It is implemented for `f32` and `f64`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
#[repr(transparent)]
pub struct Varfloat<T>(pub T);

impl<T> Varfloat<T> {
    /// Wraps the floating point value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the wrapped floating point value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Varfloat<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

macro_rules! impl_varfloat {
    ($( $float:ty as $bits:ty ),*) => {
        $(
            impl Serialize for Varfloat<$float> {

                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    // The significand occupies the low bits, so its trailing
                    // zeros are the trailing bytes of the big-endian pattern.
                    let bits = self.0.to_bits();
                    let bytes = bits.to_be_bytes();
                    let len = bytes.len() - (bits.trailing_zeros() / 8) as usize;
                    serializer.serialize_bytes(&bytes[..len])
                }
            }

            impl<'de> Deserialize<'de> for Varfloat<$float> {

                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    struct FloatVisitor;

                    impl<'de> serde::de::Visitor<'de> for FloatVisitor {
                        type Value = Varfloat<$float>;

                        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                            write!(f, concat!("a variable length ", stringify!($float)))
                        }

                        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                        where
                            E: serde::de::Error,
                        {
                            const LEN: usize = size_of::<$float>();

                            // Trailing zero bytes are removed, so an encoding
                            // holding one is not the shortest representation.
                            if v.len() > LEN || v.last() == Some(&0) {
                                return Err(E::custom(concat!("invalid ", stringify!($float))));
                            }

                            let mut bytes = [0; LEN];
                            bytes[..v.len()].copy_from_slice(v);
                            Ok(Varfloat(<$float>::from_bits(<$bits>::from_be_bytes(bytes))))
                        }
                    }

                    deserializer.deserialize_bytes(FloatVisitor)
                }
            }
        )*
    };
}

impl_varfloat![f32 as u32, f64 as u64];

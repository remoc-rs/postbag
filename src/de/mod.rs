use deserializer::Deserializer;
use serde::de::DeserializeOwned;

use crate::{
    cfg::{Cfg, Full, Slim},
    error::Result,
};

pub(crate) mod deserializer;
mod skippable;

/// Deserialize a value of type `T` from a [`std::io::Read`] using the specified
/// configuration.
///
/// The configuration must match the one used during serialization:
/// [`Full`] expects struct field identifiers and enum variant identifiers,
/// [`Slim`] expects data without identifiers, using indices for enum variants.
///
/// # Example
///
/// This example demonstrates a complete round-trip serialization and deserialization:
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{serialize, deserialize, cfg::Full};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let original = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let mut buffer = Vec::new();
/// serialize(Full::new(), &mut buffer, &original).unwrap();
///
/// let deserialized: Person = deserialize(Full::new(), buffer.as_slice()).unwrap();
/// assert_eq!(original, deserialized);
/// ```
///
/// The configuration also carries the nesting depth limit:
///
/// ```rust
/// # use serde::{Serialize, Deserialize};
/// # use postbag::{to_vec, deserialize, cfg::Full};
/// # #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// # struct Person { name: String, age: u32 }
/// # let person = Person { name: "Alice".to_string(), age: 30 };
/// let cfg = Full::new().with_depth_limit(512);
/// let bytes = to_vec(cfg, &person).unwrap();
/// let deserialized: Person = deserialize(cfg, bytes.as_slice()).unwrap();
/// ```
pub fn deserialize<R, T, const WITH_IDENTS: bool>(cfg: Cfg<WITH_IDENTS>, read: R) -> Result<T>
where
    R: std::io::Read,
    T: DeserializeOwned,
{
    let mut deserializer = Deserializer::<R, WITH_IDENTS>::new(read, cfg);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.finalize();
    Ok(t)
}

/// Deserialize a value from a byte slice using the specified configuration.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{to_vec, from_slice, cfg::Slim};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let person = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let bytes = to_vec(Slim::new(), &person).unwrap();
/// let deserialized: Person = from_slice(Slim::new(), &bytes).unwrap();
/// assert_eq!(person, deserialized);
/// ```
pub fn from_slice<T, const WITH_IDENTS: bool>(cfg: Cfg<WITH_IDENTS>, slice: &[u8]) -> Result<T>
where
    T: DeserializeOwned,
{
    deserialize(cfg, slice)
}

/// Deserialize a value using the [`Full`] configuration.
///
/// This is a convenience function equivalent to `deserialize(Full::new(), reader)`.
/// It expects struct field identifiers and enum variant identifiers as strings.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{serialize_full, deserialize_full};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let person = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let mut buffer = Vec::new();
/// serialize_full(&mut buffer, &person).unwrap();
///
/// let deserialized: Person = deserialize_full(buffer.as_slice()).unwrap();
/// assert_eq!(person, deserialized);
/// ```
pub fn deserialize_full<R, T>(reader: R) -> Result<T>
where
    R: std::io::Read,
    T: DeserializeOwned,
{
    deserialize(Full::new(), reader)
}

/// Deserialize a value using the [`Slim`] configuration.
///
/// This is a convenience function equivalent to `deserialize(Slim::new(), reader)`.
/// It expects serialized data without identifiers, using indices for enum variants.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{serialize_slim, deserialize_slim};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let person = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let mut buffer = Vec::new();
/// serialize_slim(&mut buffer, &person).unwrap();
///
/// let deserialized: Person = deserialize_slim(buffer.as_slice()).unwrap();
/// assert_eq!(person, deserialized);
/// ```
pub fn deserialize_slim<R, T>(reader: R) -> Result<T>
where
    R: std::io::Read,
    T: DeserializeOwned,
{
    deserialize(Slim::new(), reader)
}

/// Deserialize a value from a byte slice using the [`Full`] configuration.
///
/// This is a convenience function equivalent to `from_slice(Full::new(), slice)`.
/// It deserializes data that includes struct field identifiers and enum variant
/// identifiers as strings.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{to_full_vec, from_full_slice};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let person = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let bytes = to_full_vec(&person).unwrap();
/// let deserialized: Person = from_full_slice(&bytes).unwrap();
/// assert_eq!(person, deserialized);
/// ```
pub fn from_full_slice<T>(slice: &[u8]) -> Result<T>
where
    T: DeserializeOwned,
{
    from_slice(Full::new(), slice)
}

/// Deserialize a value from a byte slice using the [`Slim`] configuration.
///
/// This is a convenience function equivalent to `from_slice(Slim::new(), slice)`.
/// It deserializes data without identifiers, using indices for enum variants.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{to_slim_vec, from_slim_slice};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let person = Person {
///     name: "Alice".to_string(),
///     age: 30,
/// };
///
/// let bytes = to_slim_vec(&person).unwrap();
/// let deserialized: Person = from_slim_slice(&bytes).unwrap();
/// assert_eq!(person, deserialized);
/// ```
pub fn from_slim_slice<T>(slice: &[u8]) -> Result<T>
where
    T: DeserializeOwned,
{
    from_slice(Slim::new(), slice)
}

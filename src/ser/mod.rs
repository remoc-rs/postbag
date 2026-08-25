use serde::Serialize;

use crate::{
    cfg::{Cfg, Full, Slim},
    error::Result,
    info,
    ser::serializer::Serializer,
};

pub(crate) mod serializer;
pub(crate) mod skippable;

/// Serialize a value of type `T` to a [`std::io::Write`] using the specified
/// configuration.
///
/// Use [`Full`] to serialize struct field identifiers and enum variant
/// identifiers, or [`Slim`] to serialize without identifiers, using indices for
/// enum variants.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{serialize, cfg::Full};
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
/// serialize(Full::new(), &mut buffer, &person).unwrap();
/// println!("Serialized {} bytes", buffer.len());
/// ```
pub fn serialize<W, T, const WITH_IDENTS: bool>(cfg: Cfg<WITH_IDENTS>, mut writer: W, value: &T) -> Result<()>
where
    W: std::io::Write,
    T: Serialize + ?Sized,
{
    if cfg.header() {
        writer.write_all(&cfg.header_bytes())?;
    }

    let _info_guard = info::Guard::new(info::Direction::Serialize, &cfg);

    let mut serializer = Serializer::<W, WITH_IDENTS>::new(writer, cfg);
    // The root value occupies one nesting level, mirroring the deserializer,
    // which charges a level for entering a container even when that container
    // turns out to be empty (an empty struct or a unit enum variant). Without
    // this, serialization would accept values one level deeper than
    // deserialization, breaking round-tripping at the limit boundary.
    serializer.recurse(false, |ser| value.serialize(ser))?;
    serializer.finalize();
    Ok(())
}

/// Serialize a value using the specified configuration and return a `Vec<u8>`.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::{to_vec, cfg::Slim};
///
/// #[derive(Serialize, Deserialize)]
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
/// println!("Serialized {} bytes", bytes.len());
/// ```
pub fn to_vec<T, const WITH_IDENTS: bool>(cfg: Cfg<WITH_IDENTS>, value: &T) -> Result<Vec<u8>>
where
    T: Serialize + ?Sized,
{
    let mut buffer = Vec::new();
    serialize(cfg, &mut buffer, value)?;
    Ok(buffer)
}

/// Serialize a value using the [`Full`] configuration.
///
/// This is a convenience function equivalent to `serialize(Full::new(), writer, value)`.
/// It serializes struct field identifiers and enum variant identifiers as strings.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::serialize_full;
///
/// #[derive(Serialize, Deserialize)]
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
/// ```
pub fn serialize_full<W, T>(writer: W, value: &T) -> Result<()>
where
    W: std::io::Write,
    T: Serialize + ?Sized,
{
    serialize(Full::new(), writer, value)
}

/// Serialize a value using the [`Slim`] configuration.
///
/// This is a convenience function equivalent to `serialize(Slim::new(), writer, value)`.
/// It serializes without identifiers, using indices for enum variants.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::serialize_slim;
///
/// #[derive(Serialize, Deserialize)]
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
/// ```
pub fn serialize_slim<W, T>(writer: W, value: &T) -> Result<()>
where
    W: std::io::Write,
    T: Serialize + ?Sized,
{
    serialize(Slim::new(), writer, value)
}

/// Serialize a value using the [`Full`] configuration and return a `Vec<u8>`.
///
/// This is a convenience function equivalent to `to_vec(Full::new(), value)`.
/// It serializes struct field identifiers and enum variant identifiers as strings.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::to_full_vec;
///
/// #[derive(Serialize, Deserialize)]
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
/// println!("Serialized {} bytes", bytes.len());
/// ```
pub fn to_full_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize + ?Sized,
{
    to_vec(Full::new(), value)
}

/// Serialize a value using the [`Slim`] configuration and return a `Vec<u8>`.
///
/// This is a convenience function equivalent to `to_vec(Slim::new(), value)`.
/// It serializes without identifiers, using indices for enum variants.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use postbag::to_slim_vec;
///
/// #[derive(Serialize, Deserialize)]
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
/// println!("Serialized {} bytes", bytes.len());
/// ```
pub fn to_slim_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize + ?Sized,
{
    to_vec(Slim::new(), value)
}

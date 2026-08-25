//! Configuration of Postbag serialization data format.

use std::fmt;

/// The header a stream begins with.
///
/// Two bytes: a magic byte, then one holding a fixed bit pattern, whether
/// identifiers are serialized, and the version of the data format.
///
/// ```text
///  byte 0     byte 1
/// ┌────────┐ ┌───┬───┬───┬───┬───┬───┬───┬───┐
/// │  0xba  │ │ 1 │ 0 │ 1 │ I │ v │ v │ v │ v │
/// └────────┘ └───┴───┴───┴───┴───┴───┴───┴───┘
///              fixed      │    version
///                         └ identifiers
/// ```
pub(crate) mod header {
    use super::Version;
    use crate::error::{Error, Result};

    /// First byte, identifying the data as Postbag.
    const MAGIC: u8 = 0xba;

    /// Bits of the second byte that are always the same, and their value.
    const FIXED_MASK: u8 = 0b1110_0000;
    const FIXED: u8 = 0b1010_0000;

    /// Bit of the second byte stating whether identifiers are serialized.
    const IDENTS: u8 = 0b0001_0000;

    /// Bits of the second byte stating the version of the data format.
    ///
    /// Version 15 is reserved to state that an extended version follows.
    const VERSION_MASK: u8 = 0b0000_1111;

    /// The header stating the version of the data format and whether
    /// identifiers are serialized.
    pub(crate) const fn bytes(version: Version, with_idents: bool) -> [u8; 2] {
        let idents = if with_idents { IDENTS } else { 0 };
        [MAGIC, FIXED | idents | version.as_u8()]
    }

    /// Reads the header, returning the version of the data format it states.
    pub(crate) fn parse(bytes: [u8; 2], with_idents: bool) -> Result<Version> {
        if bytes[0] != MAGIC || bytes[1] & FIXED_MASK != FIXED {
            return Err(Error::BadHeader);
        }

        let data_has_idents = bytes[1] & IDENTS != 0;
        if data_has_idents != with_idents {
            return Err(Error::WithIdentsMismatch(data_has_idents));
        }

        let version = bytes[1] & VERSION_MASK;
        match Version::try_from(version) {
            Ok(version) if !version.is_0_4() => Ok(version),
            _ => Err(Error::UnsupportedVersion(version)),
        }
    }
}

/// Default limit for the nesting depth of serialized and deserialized data.
///
/// Serialization and deserialization of nested data is recursive, so deeply
/// nested data consumes stack space. Data nested deeper than this fails with
/// [`Error::RecursionLimit`](crate::Error::RecursionLimit); without a limit,
/// untrusted input containing deeply nested (in particular recursive) types
/// could abort the process by overflowing the stack.
///
/// Use [`Cfg::with_depth_limit`] to specify a different limit.
pub const DEFAULT_DEPTH_LIMIT: usize = 128;

/// Version of the data format.
///
/// Both serializer and deserializer must use the same version.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Version {
    /// The format of Postbag 0.4 and earlier.
    ///
    /// Only use for backwards compatibility.
    Postbag0_4,
    /// The format of Postbag 1.0.
    ///
    /// This is the default.
    #[default]
    Postbag1,
}

impl Version {
    /// Whether this is the format of Postbag 0.4 and earlier.
    pub(crate) const fn is_0_4(self) -> bool {
        matches!(self, Self::Postbag0_4)
    }

    /// The byte identifying this version.
    const fn as_u8(self) -> u8 {
        match self {
            Version::Postbag0_4 => 0,
            Version::Postbag1 => 1,
        }
    }
}

/// Identifies the version by a byte.
impl From<Version> for u8 {
    fn from(version: Version) -> Self {
        version.as_u8()
    }
}

impl TryFrom<u8> for Version {
    type Error = UnknownVersion;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Postbag0_4),
            1 => Ok(Self::Postbag1),
            unknown => Err(UnknownVersion(unknown)),
        }
    }
}

/// A byte that identifies no [`Version`] this build knows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UnknownVersion(pub u8);

impl fmt::Display for UnknownVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unknown Postbag data format version {}", self.0)
    }
}

impl std::error::Error for UnknownVersion {}

/// Configuration.
///
/// Whether identifiers are serialized is part of the type, so that the
/// corresponding code paths are resolved at compile time.
/// Use the type aliases [`Full`] and [`Slim`] to name a configuration.
///
/// # Example
///
/// ```rust
/// use postbag::cfg::Full;
///
/// let cfg = Full::new().with_depth_limit(512);
/// assert_eq!(cfg.depth_limit(), 512);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cfg<const WITH_IDENTS: bool> {
    depth_limit: usize,
    version: Version,
    header: bool,
    allow_skip: bool,
}

impl<const WITH_IDENTS: bool> Cfg<WITH_IDENTS> {
    /// Default limit for the nesting depth of serialized and deserialized data.
    ///
    /// This is [`DEFAULT_DEPTH_LIMIT`].
    pub const DEFAULT_DEPTH_LIMIT: usize = DEFAULT_DEPTH_LIMIT;

    /// Creates a new configuration using default values.
    pub const fn new() -> Self {
        Self { depth_limit: DEFAULT_DEPTH_LIMIT, version: Version::Postbag1, header: true, allow_skip: true }
    }

    /// Whether struct field identifiers and enum variant identifiers
    /// are serialized.
    pub const fn with_idents(&self) -> bool {
        WITH_IDENTS
    }

    /// Sets the limit for the nesting depth of serialized and deserialized data.
    ///
    /// Data nested deeper than this fails with
    /// [`Error::RecursionLimit`](crate::Error::RecursionLimit).
    ///
    /// Defaults to [`DEFAULT_DEPTH_LIMIT`].    
    pub const fn with_depth_limit(self, depth_limit: usize) -> Self {
        Self { depth_limit, ..self }
    }

    /// Sets the version of the data format.
    ///
    /// Both ends must use the same version.
    ///
    /// When deserializing and the [header](Self::with_header) is enabled (default), the Postbag version
    /// used for serialization is auto-detected.
    ///
    /// Defaults to this Postbag version.
    pub const fn with_version(self, version: Version) -> Self {
        Self { version, ..self }
    }

    /// Sets whether the data begins with a header stating the version of the
    /// data format and whether identifiers are serialized.
    ///
    /// A headers enables a future version of Postbag to auto-detect the version
    /// used for serializing the data. Otherwise the deserializer must be configured
    /// to use the same version using [`with_version`](Self::with_version).
    ///
    /// Headers are always disabled for Postbag 0.4.
    ///
    /// Defaults to `true`.
    pub const fn with_header(self, header: bool) -> Self {
        Self { header, ..self }
    }

    /// Sets whether the data being written may leave out a struct field.
    ///
    /// This is what [`skip::is_allowed`](crate::skip::is_allowed) returns
    /// during serialization.
    ///
    /// Defaults to `true`.
    pub const fn with_allow_skip(self, allow_skip: bool) -> Self {
        Self { allow_skip, ..self }
    }

    /// The limit for the nesting depth of serialized and deserialized data.
    pub const fn depth_limit(&self) -> usize {
        self.depth_limit
    }

    /// Whether the data begins with a header stating the version of the data
    /// format and whether identifiers are serialized.
    pub const fn header(&self) -> bool {
        self.header && !self.version().is_0_4()
    }

    /// The header this configuration writes.
    pub(crate) const fn header_bytes(&self) -> [u8; 2] {
        header::bytes(self.version, WITH_IDENTS)
    }

    /// Whether the data being written may leave out a struct field.
    ///
    /// Per default this is `true`, but *always* `false` under [`Slim`]
    /// and in a build using the `postbag_fast_compile` configuration.
    pub const fn allow_skip(&self) -> bool {
        self.allow_skip && WITH_IDENTS && !cfg!(postbag_fast_compile)
    }

    /// The version of the data format.
    pub const fn version(&self) -> Version {
        self.version
    }
}

impl<const WITH_IDENTS: bool> Default for Cfg<WITH_IDENTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const WITH_IDENTS: bool> fmt::Debug for Cfg<WITH_IDENTS> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Cfg")
            .field("with_idents", &WITH_IDENTS)
            .field("depth_limit", &self.depth_limit)
            .field("version", &self.version)
            .field("header", &self.header())
            .field("allow_skip", &self.allow_skip())
            .finish()
    }
}

/// Serialize with identifiers.
///
/// Struct field identifiers and enum variant identifiers are serialized
/// as strings or using numerical identifier encoding.
///
/// See the [Postbag Full format 1.0 specification][specification].
///
/// [specification]: https://github.com/remoc-rs/postbag/blob/main/POSTBAG-FULL.md
pub type Full = Cfg<true>;

/// Serialize without identifiers.
///
/// Struct field identifiers are not serialized.
/// Enum variants are serialized using their index.
///
/// See the [Postbag Slim format 1.0 specification][specification].
///
/// [specification]: https://github.com/remoc-rs/postbag/blob/main/POSTBAG-SLIM.md
pub type Slim = Cfg<false>;

//! Configuration of Postbag serialization data format.

use std::fmt;

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
}

/// Identifies the version by a byte.
impl From<Version> for u8 {
    fn from(version: Version) -> Self {
        match version {
            Version::Postbag0_4 => 0,
            Version::Postbag1 => 1,
        }
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
}

impl<const WITH_IDENTS: bool> Cfg<WITH_IDENTS> {
    /// Default limit for the nesting depth of serialized and deserialized data.
    ///
    /// This is [`DEFAULT_DEPTH_LIMIT`].
    pub const DEFAULT_DEPTH_LIMIT: usize = DEFAULT_DEPTH_LIMIT;

    /// Creates a new configuration using default values.
    pub const fn new() -> Self {
        Self { depth_limit: DEFAULT_DEPTH_LIMIT, version: Version::Postbag1 }
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use postbag::cfg::Slim;
    ///
    /// let cfg = Slim::new().with_depth_limit(1024);
    /// assert_eq!(cfg.depth_limit(), 1024);
    /// ```
    pub const fn with_depth_limit(self, depth_limit: usize) -> Self {
        Self { depth_limit, ..self }
    }

    /// Sets the version of the data format.
    ///
    /// Both ends must use the same version.
    ///
    /// Defaults to [`Version::Postbag1`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use postbag::cfg::{Full, Version};
    ///
    /// let cfg = Full::new().with_version(Version::Postbag0_4);
    /// assert_eq!(cfg.version(), Version::Postbag0_4);
    /// ```
    pub const fn with_version(self, version: Version) -> Self {
        Self { version, ..self }
    }

    /// The limit for the nesting depth of serialized and deserialized data.
    pub const fn depth_limit(&self) -> usize {
        self.depth_limit
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
            .finish()
    }
}

/// Serialize with identifiers.
///
/// Struct field identifiers and enum variant identifiers are serialized
/// as strings or using numerical identifier encoding.
pub type Full = Cfg<true>;

/// Serialize without identifiers.
///
/// Struct field identifiers are not serialized.
/// Enum variants are serialized using their index.
pub type Slim = Cfg<false>;

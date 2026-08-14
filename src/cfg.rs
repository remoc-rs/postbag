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

/// Which lengths a value writes for itself in [`Full`] mode.
///
/// This is part of the data format and the serializer and deserializer must
/// use the same setting.
///
/// This is ignored for [`Slim`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum SizeHints {
    /// Every value writes its own length, even when it can be deduced from the
    /// enclosing block.
    ///
    /// Use this to read data written by Postbag 0.4 and earlier.
    All,
    /// Sequences and maps write their element count; nothing else writes a
    /// length or count that can be deduced from the enclosing block.
    ///
    /// The count of a sequence or map is not deducible: an element or entry
    /// can occupy no bytes at all, so any number of them looks the same as
    /// none. It is also what lets the reader allocate once.
    ///
    /// This saves some space versus [All](Self::All) but is incompatible with
    /// Postbag 0.4 and earlier.
    ///
    /// This is the default.
    #[default]
    Sequences,
}

impl SizeHints {
    /// Whether a value that extends to the end of its block writes its own
    /// length as well.
    pub(crate) const fn value_writes_len(self) -> bool {
        matches!(self, Self::All)
    }
}

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
    size_hints: SizeHints,
}

impl<const WITH_IDENTS: bool> Cfg<WITH_IDENTS> {
    /// Default limit for the nesting depth of serialized and deserialized data.
    ///
    /// This is [`DEFAULT_DEPTH_LIMIT`].
    pub const DEFAULT_DEPTH_LIMIT: usize = DEFAULT_DEPTH_LIMIT;

    /// Creates a new configuration using default values.
    pub const fn new() -> Self {
        Self { depth_limit: DEFAULT_DEPTH_LIMIT, size_hints: SizeHints::Sequences }
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

    /// Sets which lengths a value writes for itself.
    ///
    /// This is part of the data format: both ends must agree, or the reader
    /// misinterprets the bytes. Use [`SizeHints::All`] to read data written
    /// by Postbag 0.4 and earlier.
    ///
    /// Defaults to [`SizeHints::Sequences`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use postbag::cfg::{Full, SizeHints};
    ///
    /// let cfg = Full::new().with_size_hints(SizeHints::All);
    /// assert_eq!(cfg.size_hints(), SizeHints::All);
    /// ```
    pub const fn with_size_hints(self, size_hints: SizeHints) -> Self {
        Self { size_hints, ..self }
    }

    /// The limit for the nesting depth of serialized and deserialized data.
    pub const fn depth_limit(&self) -> usize {
        self.depth_limit
    }

    /// Which lengths a value writes for itself.
    pub const fn size_hints(&self) -> SizeHints {
        self.size_hints
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
            .field("size_hints", &self.size_hints)
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

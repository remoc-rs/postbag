//! Configuration of Postbag serialization data format.

use std::fmt;

mod sealed {
    pub trait Sealed {}

    impl<const WITH_IDENTS: bool> Sealed for super::StaticCfg<WITH_IDENTS> {}
}

/// Configuration trait.
///
/// This trait is sealed and cannot be implemented outside of Postbag.
/// Use [`Full`] or [`Slim`].
pub trait Cfg: sealed::Sealed {
    /// Whether struct field identifiers and enum variant identifiers
    /// are serialized.
    fn with_idents() -> bool;
}

/// Static (compile-time) configuration.
#[derive(Clone, Copy)]
pub struct StaticCfg<const WITH_IDENTS: bool>;

impl<const WITH_IDENTS: bool> fmt::Debug for StaticCfg<WITH_IDENTS> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("StaticCfg").field("with_idents", &WITH_IDENTS).finish()
    }
}

impl<const WITH_IDENTS: bool> Cfg for StaticCfg<WITH_IDENTS> {
    fn with_idents() -> bool {
        WITH_IDENTS
    }
}

/// Serialize with identifiers.
///
/// Struct field identifiers and enum variant identifiers are serialized
/// as strings or using numerical identifier encoding.
pub type Full = StaticCfg<true>;

/// Serialize without identifiers.
///
/// Struct field identifiers are not serialized.
/// Enum variants are serialized using their index.
pub type Slim = StaticCfg<false>;

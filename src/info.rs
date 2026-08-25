//! The Postbag (de)serialization running on the current thread.
//!
//! Serde offers no way of passing the configuration in use to a
//! [`skip_serializing_if`](https://serde.rs/field-attrs.html#skip_serializing_if)
//! predicate, a [`serialize_with`](https://serde.rs/field-attrs.html#serialize_with)
//! function or a `Serialize` implementation, since all of these are reached
//! through serde's traits and receive nothing but the value itself. A
//! thread-local record of what the current (de)serialization is doing gives
//! them a way to ask.
//!
//! Only [`crate::skip`] consults this, so the module is internal.

use std::cell::Cell;

use crate::cfg::{Cfg, Version};

/// Whether data is being written or read.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Direction {
    /// Data is being serialized.
    Serialize,
    /// Data is being deserialized.
    Deserialize,
}

/// What the Postbag (de)serialization running on the current thread is doing.
///
/// This is the configuration in use, stripped of the const parameter of
/// [`Cfg`] so that it can be held in a thread-local, together with the
/// direction the data is flowing in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Info {
    /// Whether data is being written or read.
    pub(crate) direction: Direction,
    /// Whether the data being written may leave out a struct field.
    ///
    /// Already accounts for whether identifiers are serialized.
    pub(crate) allow_skip: bool,
    // The rest of the configuration is recorded so that it is all in one
    // place. Nothing consults it yet.
    /// The limit for the nesting depth of serialized and deserialized data.
    #[allow(dead_code)]
    pub(crate) depth_limit: usize,
    /// The version of the data format.
    #[allow(dead_code)]
    pub(crate) version: Version,
    /// Whether the data begins with a header.
    #[allow(dead_code)]
    pub(crate) header: bool,
}

thread_local! {
    /// The Postbag (de)serialization running on this thread, if any.
    static ACTIVE: Cell<Option<Info>> = const { Cell::new(None) };
}

/// The Postbag (de)serialization running on the current thread.
///
/// `None` when this thread is not inside a Postbag (de)serialization, so that
/// another data format is left to decide for itself.
pub(crate) fn active() -> Option<Info> {
    ACTIVE.get()
}

/// Records a running (de)serialization, restoring the previous one when
/// dropped.
///
/// Restoring rather than clearing keeps a Postbag (de)serialization nested
/// inside another one — a value written to a buffer of its own — from ending
/// the enclosing one early, and leaves the record intact should it panic.
pub(crate) struct Guard(Option<Info>);

impl Guard {
    /// Records the specified (de)serialization for as long as the returned
    /// guard lives.
    ///
    /// When deserializing, pass the configuration *after* the version stated
    /// by the header has been applied to it, so that the version recorded is
    /// the one the data is actually written in.
    pub(crate) fn new<const WITH_IDENTS: bool>(direction: Direction, cfg: &Cfg<WITH_IDENTS>) -> Self {
        let info = Info {
            direction,
            allow_skip: cfg.allow_skip(),
            depth_limit: cfg.depth_limit(),
            version: cfg.version(),
            header: cfg.header(),
        };

        Self(ACTIVE.replace(Some(info)))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.set(self.0);
    }
}

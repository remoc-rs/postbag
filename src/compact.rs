//! Compact representations.
//!
//! This module contains types with a more compact serialized representation.
//!
//! The representations serde provides for some types of the standard library
//! spell out struct field names and enum variant names, which is wasteful when
//! serializing with [identifiers](crate::cfg::Full). The types in this module
//! avoid that by using unnamed fields, numerical identifiers and, where
//! applicable, a more efficient encoding of the value itself.
//!
//! Its usage is completely optional, but it must be applied consistently, since
//! the compacted representation is not compatible with the plain
//! representation.
//!
//! ```rust
//! # use serde::Serialize;
//! # use std::time::Duration;
//! #[derive(Serialize)]
//! pub struct MyData {
//!     #[serde(with = "postbag::compact")]
//!     result: Result<u32, String>,
//!     #[serde(with = "postbag::compact")]
//!     duration: Duration,
//! }
//! ```
//!
//! Serialization borrows from the value, thus it never clones it. Shared
//! references are supported as well and yield the same representation.
//!
//! ```rust
//! # use serde::Serialize;
//! # use std::time::Duration;
//! #[derive(Serialize)]
//! pub struct MyDataRef<'a> {
//!     #[serde(with = "postbag::compact")]
//!     result: &'a Result<u32, String>,
//!     #[serde(with = "postbag::compact")]
//!     duration: &'a Duration,
//! }
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A type that can be serialized using a compacted representation.
///
/// This is also implemented for shared references to compactable types.
pub trait AsCompact {
    /// Type of compacted representation.
    ///
    /// It may borrow from the value it was created from.
    type Compacted<'a>
    where
        Self: 'a;

    /// Returns the compacted representation of this value.
    fn as_compacted(&self) -> Self::Compacted<'_>;
}

impl<T> AsCompact for &T
where
    T: AsCompact + ?Sized,
{
    type Compacted<'a>
        = T::Compacted<'a>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        T::as_compacted(self)
    }
}

/// A type that can be deserialized from a compacted representation.
pub trait FromCompact: Sized {
    /// Type of compacted representation.
    type Compacted;

    /// Creates the value from its compacted representation.
    fn from_compacted(compacted: Self::Compacted) -> Self;
}

/// Serialize value using compacted representation.
pub fn serialize<'a, T, S>(value: &'a T, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    T: AsCompact + ?Sized,
    <T as AsCompact>::Compacted<'a>: Serialize,
    S: Serializer,
{
    value.as_compacted().serialize(serializer)
}

/// Deserialize value from compacted representation.
pub fn deserialize<'de, T, D>(deserializer: D) -> std::result::Result<T, D::Error>
where
    T: FromCompact,
    <T as FromCompact>::Compacted: Deserialize<'de>,
    D: Deserializer<'de>,
{
    let compacted = T::Compacted::deserialize(deserializer)?;
    Ok(T::from_compacted(compacted))
}

/// Compact representation of [Result](std::result::Result).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Result<T, E> {
    /// Contains the success value
    #[serde(rename = "_0")]
    Ok(T),
    /// Contains the error value
    #[serde(rename = "_1")]
    Err(E),
}

impl<T, E> From<std::result::Result<T, E>> for Result<T, E> {
    fn from(res: std::result::Result<T, E>) -> Self {
        match res {
            Ok(t) => Self::Ok(t),
            Err(err) => Self::Err(err),
        }
    }
}

impl<T, E> From<Result<T, E>> for std::result::Result<T, E> {
    fn from(compact: Result<T, E>) -> Self {
        match compact {
            Result::Ok(v) => std::result::Result::Ok(v),
            Result::Err(err) => std::result::Result::Err(err),
        }
    }
}

impl<T, E> AsCompact for std::result::Result<T, E> {
    type Compacted<'a>
        = Result<&'a T, &'a E>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        match self {
            Ok(value) => Result::Ok(value),
            Err(err) => Result::Err(err),
        }
    }
}

impl<T, E> FromCompact for std::result::Result<T, E> {
    type Compacted = Result<T, E>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

const NANOS_PER_SEC: i128 = 1_000_000_000;

/// Compact representation of [Duration](std::time::Duration).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "(u64, u32)")]
pub struct Duration(pub u64, pub u32);

impl TryFrom<(u64, u32)> for Duration {
    type Error = &'static str;

    fn try_from((secs, nanos): (u64, u32)) -> std::result::Result<Self, Self::Error> {
        if i128::from(nanos) >= NANOS_PER_SEC {
            return Err("Duration nanoseconds out of range");
        }
        Ok(Self(secs, nanos))
    }
}

impl From<std::time::Duration> for Duration {
    fn from(duration: std::time::Duration) -> Self {
        Self(duration.as_secs(), duration.subsec_nanos())
    }
}

impl From<Duration> for std::time::Duration {
    /// Converts into [Duration](std::time::Duration).
    ///
    /// # Panics
    /// Panics if the nanoseconds are not less than one second.
    ///
    /// Deserialization never panics, since the value is checked while
    /// deserializing.
    fn from(compact: Duration) -> Self {
        assert!(i128::from(compact.1) < NANOS_PER_SEC, "Duration nanoseconds out of range");

        // Since the nanoseconds are less than one second, this cannot overflow.
        std::time::Duration::new(compact.0, compact.1)
    }
}

impl AsCompact for std::time::Duration {
    type Compacted<'a> = Duration;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        (*self).into()
    }
}

impl FromCompact for std::time::Duration {
    type Compacted = Duration;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [SystemTime](std::time::SystemTime).
///
/// Contrary to the representation provided by serde, points in time before
/// the UNIX epoch are supported and represented by negative seconds.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "(i64, u32)")]
pub struct SystemTime(pub i64, pub u32);

impl TryFrom<(i64, u32)> for SystemTime {
    type Error = &'static str;

    fn try_from((secs, nanos): (i64, u32)) -> std::result::Result<Self, Self::Error> {
        let compact = Self(secs, nanos);
        match compact.checked_into() {
            Some(_) => Ok(compact),
            None => Err("SystemTime out of range"),
        }
    }
}

impl SystemTime {
    /// Converts into [SystemTime](std::time::SystemTime), returning [None] if
    /// the value cannot be represented.
    fn checked_into(self) -> Option<std::time::SystemTime> {
        if i128::from(self.1) >= NANOS_PER_SEC {
            return None;
        }

        let secs = std::time::Duration::from_secs(self.0.unsigned_abs());
        let subsec = std::time::Duration::from_nanos(self.1.into());

        if self.0 >= 0 {
            std::time::UNIX_EPOCH.checked_add(secs)?.checked_add(subsec)
        } else {
            // The nanoseconds are positive, thus they are added back after
            // subtracting the rounded down amount of whole seconds.
            std::time::UNIX_EPOCH.checked_sub(secs)?.checked_add(subsec)
        }
    }
}

impl From<std::time::SystemTime> for SystemTime {
    /// Converts from [SystemTime](std::time::SystemTime).
    ///
    /// # Panics
    /// Panics if the seconds since [UNIX_EPOCH](std::time::UNIX_EPOCH) do not
    /// fit into an [i64], which cannot occur on any supported platform.
    fn from(time: std::time::SystemTime) -> Self {
        let nanos: i128 = match time.duration_since(std::time::UNIX_EPOCH) {
            // The nanoseconds of a Duration never exceed i128::MAX,
            // thus the conversions cannot overflow.
            Ok(duration) => duration.as_nanos() as i128,
            Err(err) => -(err.duration().as_nanos() as i128),
        };

        let secs = i64::try_from(nanos.div_euclid(NANOS_PER_SEC)).expect("SystemTime out of range");
        Self(secs, nanos.rem_euclid(NANOS_PER_SEC) as u32)
    }
}

impl From<SystemTime> for std::time::SystemTime {
    /// Converts into [SystemTime](std::time::SystemTime).
    ///
    /// # Panics
    /// Panics if the point in time cannot be represented by
    /// [SystemTime](std::time::SystemTime).
    ///
    /// Deserialization never panics, since the value is checked while
    /// deserializing.
    fn from(compact: SystemTime) -> Self {
        compact.checked_into().expect("SystemTime out of range")
    }
}

impl AsCompact for std::time::SystemTime {
    type Compacted<'a> = SystemTime;

    /// Returns the compacted representation of this value.
    ///
    /// # Panics
    /// Panics if the seconds since [UNIX_EPOCH](std::time::UNIX_EPOCH) do not
    /// fit into an [i64], which cannot occur on any supported platform.
    fn as_compacted(&self) -> Self::Compacted<'_> {
        (*self).into()
    }
}

impl FromCompact for std::time::SystemTime {
    type Compacted = SystemTime;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [Range](std::ops::Range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Range<Idx>(pub Idx, pub Idx);

impl<Idx> From<std::ops::Range<Idx>> for Range<Idx> {
    fn from(range: std::ops::Range<Idx>) -> Self {
        Self(range.start, range.end)
    }
}

impl<Idx> From<Range<Idx>> for std::ops::Range<Idx> {
    fn from(compact: Range<Idx>) -> Self {
        compact.0..compact.1
    }
}

impl<Idx> AsCompact for std::ops::Range<Idx> {
    type Compacted<'a>
        = Range<&'a Idx>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        Range(&self.start, &self.end)
    }
}

impl<Idx> FromCompact for std::ops::Range<Idx> {
    type Compacted = Range<Idx>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [RangeInclusive](std::ops::RangeInclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RangeInclusive<Idx>(pub Idx, pub Idx);

impl<Idx> From<std::ops::RangeInclusive<Idx>> for RangeInclusive<Idx> {
    fn from(range: std::ops::RangeInclusive<Idx>) -> Self {
        let (start, end) = range.into_inner();
        Self(start, end)
    }
}

impl<Idx> From<RangeInclusive<Idx>> for std::ops::RangeInclusive<Idx> {
    fn from(compact: RangeInclusive<Idx>) -> Self {
        Self::new(compact.0, compact.1)
    }
}

impl<Idx> AsCompact for std::ops::RangeInclusive<Idx> {
    type Compacted<'a>
        = RangeInclusive<&'a Idx>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        RangeInclusive(self.start(), self.end())
    }
}

impl<Idx> FromCompact for std::ops::RangeInclusive<Idx> {
    type Compacted = RangeInclusive<Idx>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [RangeFrom](std::ops::RangeFrom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RangeFrom<Idx>(pub Idx);

impl<Idx> From<std::ops::RangeFrom<Idx>> for RangeFrom<Idx> {
    fn from(range: std::ops::RangeFrom<Idx>) -> Self {
        Self(range.start)
    }
}

impl<Idx> From<RangeFrom<Idx>> for std::ops::RangeFrom<Idx> {
    fn from(compact: RangeFrom<Idx>) -> Self {
        compact.0..
    }
}

impl<Idx> AsCompact for std::ops::RangeFrom<Idx> {
    type Compacted<'a>
        = RangeFrom<&'a Idx>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        RangeFrom(&self.start)
    }
}

impl<Idx> FromCompact for std::ops::RangeFrom<Idx> {
    type Compacted = RangeFrom<Idx>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [RangeTo](std::ops::RangeTo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RangeTo<Idx>(pub Idx);

impl<Idx> From<std::ops::RangeTo<Idx>> for RangeTo<Idx> {
    fn from(range: std::ops::RangeTo<Idx>) -> Self {
        Self(range.end)
    }
}

impl<Idx> From<RangeTo<Idx>> for std::ops::RangeTo<Idx> {
    fn from(compact: RangeTo<Idx>) -> Self {
        ..compact.0
    }
}

impl<Idx> AsCompact for std::ops::RangeTo<Idx> {
    type Compacted<'a>
        = RangeTo<&'a Idx>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        RangeTo(&self.end)
    }
}

impl<Idx> FromCompact for std::ops::RangeTo<Idx> {
    type Compacted = RangeTo<Idx>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [Bound](std::ops::Bound).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Bound<T> {
    /// An infinite endpoint. Indicates that there is no bound in this direction.
    #[serde(rename = "_0")]
    Unbounded,
    /// An inclusive bound.
    #[serde(rename = "_1")]
    Included(T),
    /// An exclusive bound.
    #[serde(rename = "_2")]
    Excluded(T),
}

impl<T> From<std::ops::Bound<T>> for Bound<T> {
    fn from(bound: std::ops::Bound<T>) -> Self {
        match bound {
            std::ops::Bound::Unbounded => Self::Unbounded,
            std::ops::Bound::Included(value) => Self::Included(value),
            std::ops::Bound::Excluded(value) => Self::Excluded(value),
        }
    }
}

impl<T> From<Bound<T>> for std::ops::Bound<T> {
    fn from(compact: Bound<T>) -> Self {
        match compact {
            Bound::Unbounded => Self::Unbounded,
            Bound::Included(value) => Self::Included(value),
            Bound::Excluded(value) => Self::Excluded(value),
        }
    }
}

impl<T> AsCompact for std::ops::Bound<T> {
    type Compacted<'a>
        = Bound<&'a T>
    where
        Self: 'a;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        match self {
            std::ops::Bound::Unbounded => Bound::Unbounded,
            std::ops::Bound::Included(value) => Bound::Included(value),
            std::ops::Bound::Excluded(value) => Bound::Excluded(value),
        }
    }
}

impl<T> FromCompact for std::ops::Bound<T> {
    type Compacted = Bound<T>;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [IpAddr](std::net::IpAddr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IpAddr {
    /// An IPv4 address.
    #[serde(rename = "_0")]
    V4(std::net::Ipv4Addr),
    /// An IPv6 address.
    #[serde(rename = "_1")]
    V6(std::net::Ipv6Addr),
}

impl From<std::net::IpAddr> for IpAddr {
    fn from(addr: std::net::IpAddr) -> Self {
        match addr {
            std::net::IpAddr::V4(addr) => Self::V4(addr),
            std::net::IpAddr::V6(addr) => Self::V6(addr),
        }
    }
}

impl From<IpAddr> for std::net::IpAddr {
    fn from(compact: IpAddr) -> Self {
        match compact {
            IpAddr::V4(addr) => Self::V4(addr),
            IpAddr::V6(addr) => Self::V6(addr),
        }
    }
}

impl AsCompact for std::net::IpAddr {
    type Compacted<'a> = IpAddr;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        (*self).into()
    }
}

impl FromCompact for std::net::IpAddr {
    type Compacted = IpAddr;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [SocketAddrV6](std::net::SocketAddrV6).
///
/// The fields are the IP address, the port, the flow information and the
/// scope id.
///
/// Contrary to the representation provided by serde, the flow information and
/// the scope id are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SocketAddrV6(pub std::net::Ipv6Addr, pub u16, pub u32, pub u32);

impl From<std::net::SocketAddrV6> for SocketAddrV6 {
    fn from(addr: std::net::SocketAddrV6) -> Self {
        Self(*addr.ip(), addr.port(), addr.flowinfo(), addr.scope_id())
    }
}

impl From<SocketAddrV6> for std::net::SocketAddrV6 {
    fn from(compact: SocketAddrV6) -> Self {
        Self::new(compact.0, compact.1, compact.2, compact.3)
    }
}

impl AsCompact for std::net::SocketAddrV6 {
    type Compacted<'a> = SocketAddrV6;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        (*self).into()
    }
}

impl FromCompact for std::net::SocketAddrV6 {
    type Compacted = SocketAddrV6;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

/// Compact representation of [SocketAddr](std::net::SocketAddr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SocketAddr {
    /// An IPv4 socket address.
    #[serde(rename = "_0")]
    V4(std::net::SocketAddrV4),
    /// An IPv6 socket address.
    #[serde(rename = "_1")]
    V6(SocketAddrV6),
}

impl From<std::net::SocketAddr> for SocketAddr {
    fn from(addr: std::net::SocketAddr) -> Self {
        match addr {
            std::net::SocketAddr::V4(addr) => Self::V4(addr),
            std::net::SocketAddr::V6(addr) => Self::V6(addr.into()),
        }
    }
}

impl From<SocketAddr> for std::net::SocketAddr {
    fn from(compact: SocketAddr) -> Self {
        match compact {
            SocketAddr::V4(addr) => Self::V4(addr),
            SocketAddr::V6(addr) => Self::V6(addr.into()),
        }
    }
}

impl AsCompact for std::net::SocketAddr {
    type Compacted<'a> = SocketAddr;

    fn as_compacted(&self) -> Self::Compacted<'_> {
        (*self).into()
    }
}

impl FromCompact for std::net::SocketAddr {
    type Compacted = SocketAddr;

    fn from_compacted(compacted: Self::Compacted) -> Self {
        compacted.into()
    }
}

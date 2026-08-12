use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ops::{Bound, Range, RangeFrom, RangeInclusive, RangeTo},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use postbag::{
    cfg::{Cfg, Full, Slim},
    compact::{AsCompact, FromCompact},
    deserialize, serialize,
};
use serde::de::DeserializeOwned;

/// Wrapper serializing its value using its compacted representation.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: AsCompact, for<'a> <T as AsCompact>::Compacted<'a>: Serialize",
    deserialize = "T: FromCompact, <T as FromCompact>::Compacted: Deserialize<'de>"
))]
struct Compact<T>(#[serde(with = "postbag::compact")] T);

/// A value whose compacted representation can be checked by the helpers of
/// this test.
trait Checkable: AsCompact + FromCompact + Serialize + DeserializeOwned + Debug + PartialEq
where
    for<'a> <Self as AsCompact>::Compacted<'a>: Serialize,
    <Self as FromCompact>::Compacted: DeserializeOwned,
{
}

impl<T> Checkable for T
where
    T: AsCompact + FromCompact + Serialize + DeserializeOwned + Debug + PartialEq,
    for<'a> <T as AsCompact>::Compacted<'a>: Serialize,
    <T as FromCompact>::Compacted: DeserializeOwned,
{
}

/// Serializes the value directly and using its compacted representation,
/// verifying that the compacted representation loops back.
///
/// If `check_size` is set and serialization is performed with identifiers, the
/// compacted representation must not be larger than the plain representation.
///
/// The value is passed by value and returned, since compacting never clones it.
#[track_caller]
fn compact_loopback_with_cfg<T, const WITH_IDENTS: bool>(value: T, cfg: Cfg<WITH_IDENTS>, check_size: bool) -> T
where
    T: Checkable,
    for<'a> <T as AsCompact>::Compacted<'a>: Serialize,
    <T as FromCompact>::Compacted: DeserializeOwned,
{
    let mut plain = Vec::new();
    let plain_len = match serialize(cfg, &mut plain, &value) {
        Ok(()) => Some(plain.len()),
        Err(_) => None,
    };

    let value = Compact(value);
    let mut compact = Vec::new();
    serialize(cfg, &mut compact, &value).expect("compact serialization failed");
    let Compact(value) = value;

    println!("{value:?}: plain {plain_len:?} bytes, compact {} bytes", compact.len());
    if let Some(plain_len) = plain_len
        && check_size
        && cfg.with_idents()
    {
        assert!(
            compact.len() <= plain_len,
            "compacted representation of {value:?} is larger than plain representation"
        );
    }

    let deserialized: Compact<T> = deserialize(cfg, compact.as_slice()).expect("compact deserialization failed");
    assert_eq!(deserialized.0, value, "deserialized value does not match original value");

    value
}

/// Checks the compacted representation with all configurations.
#[track_caller]
fn compact_loopback<T>(value: T)
where
    T: Checkable,
    for<'a> <T as AsCompact>::Compacted<'a>: Serialize,
    <T as FromCompact>::Compacted: DeserializeOwned,
{
    let value = compact_loopback_with_cfg(value, Full::new(), true);
    compact_loopback_with_cfg(value, Slim::new(), true);
}

/// Checks the compacted representation with all configurations, without
/// verifying that it is not larger than the plain representation.
#[track_caller]
fn compact_loopback_unchecked_size<T>(value: T)
where
    T: Checkable,
    for<'a> <T as AsCompact>::Compacted<'a>: Serialize,
    <T as FromCompact>::Compacted: DeserializeOwned,
{
    let value = compact_loopback_with_cfg(value, Full::new(), false);
    compact_loopback_with_cfg(value, Slim::new(), false);
}

/// Serializes the value using its compacted representation.
fn to_compact_vec<T, const WITH_IDENTS: bool>(value: T, cfg: Cfg<WITH_IDENTS>) -> Vec<u8>
where
    T: AsCompact,
    for<'a> <T as AsCompact>::Compacted<'a>: Serialize,
{
    let mut compact = Vec::new();
    serialize(cfg, &mut compact, &Compact(value)).expect("compact serialization failed");
    compact
}

/// Deserializes the value from its compacted representation.
fn from_compact_slice<T, const WITH_IDENTS: bool>(data: &[u8], cfg: Cfg<WITH_IDENTS>) -> postbag::Result<T>
where
    T: FromCompact,
    <T as FromCompact>::Compacted: DeserializeOwned,
{
    deserialize::<_, Compact<T>, WITH_IDENTS>(cfg, data).map(|value| value.0)
}

#[test]
fn results() {
    compact_loopback(Ok::<u32, String>(123));
    compact_loopback(Err::<u32, String>("failed".to_string()));
    compact_loopback(Ok::<(), ()>(()));
}

#[test]
fn durations() {
    compact_loopback(Duration::ZERO);
    compact_loopback(Duration::from_nanos(1));
    compact_loopback(Duration::from_secs(1));
    compact_loopback(Duration::from_millis(1500));
    compact_loopback(Duration::MAX);
}

#[test]
fn duration_rejects_invalid() {
    let data = to_compact_vec(Duration::MAX, Full::new());
    from_compact_slice::<Duration, _>(&data, Full::new()).expect("Duration::MAX must be representable");

    let mut invalid_nanos = Vec::new();
    serialize(Full::new(), &mut invalid_nanos, &(0u64, 1_000_000_000u32)).unwrap();
    from_compact_slice::<Duration, _>(&invalid_nanos, Full::new())
        .expect_err("invalid nanoseconds must be rejected");
}

#[test]
fn system_times() {
    compact_loopback(UNIX_EPOCH);
    compact_loopback(UNIX_EPOCH + Duration::from_nanos(1));
    compact_loopback(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    compact_loopback(SystemTime::now());
}

#[test]
fn system_times_before_unix_epoch() {
    for before_epoch in [
        UNIX_EPOCH - Duration::from_nanos(1),
        UNIX_EPOCH - Duration::from_millis(500),
        UNIX_EPOCH - Duration::from_secs(1_000_000),
        UNIX_EPOCH - Duration::from_nanos(1_500_000_000),
    ] {
        // serde is unable to represent points in time before the unix epoch.
        let mut plain = Vec::new();
        serialize(Full::new(), &mut plain, &before_epoch).expect_err("serde must reject pre-epoch times");

        compact_loopback(before_epoch);
    }
}

#[test]
fn system_time_rejects_invalid() {
    let mut invalid_nanos = Vec::new();
    serialize(Full::new(), &mut invalid_nanos, &(0i64, 1_000_000_000u32)).unwrap();
    from_compact_slice::<SystemTime, _>(&invalid_nanos, Full::new())
        .expect_err("invalid nanoseconds must be rejected");

    // Whether extreme values are representable is platform-dependent, but they
    // must never cause a panic.
    for secs in [i64::MIN, i64::MAX] {
        let mut extreme = Vec::new();
        serialize(Full::new(), &mut extreme, &(secs, 999_999_999u32)).unwrap();
        if let Ok(time) = from_compact_slice::<SystemTime, _>(&extreme, Full::new()) {
            assert_eq!(to_compact_vec(time, Full::new()), extreme);
        }
    }
}

#[test]
fn ranges() {
    compact_loopback(3u32..10);
    compact_loopback(0u8..0);
    compact_loopback(3u32..=10);
    compact_loopback(3u32..);
    compact_loopback(..10u32);
    compact_loopback("a".to_string().."z".to_string());
}

#[test]
fn bounds() {
    compact_loopback(Bound::Unbounded::<u32>);
    compact_loopback(Bound::Included(5u32));
    compact_loopback(Bound::Excluded(5u32));
}

#[test]
fn ip_addrs() {
    compact_loopback(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    compact_loopback(IpAddr::V6(Ipv6Addr::LOCALHOST));
    compact_loopback(IpAddr::V6("2001:db8::dead:beef".parse::<Ipv6Addr>().unwrap()));
}

#[test]
fn socket_addrs() {
    compact_loopback(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080)));

    // The IPv6 representation preserves the flow information and the scope id,
    // which costs two additional bytes.
    compact_loopback_unchecked_size(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 0, 0)));
    compact_loopback_unchecked_size(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 0, 0));
    compact_loopback_unchecked_size(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 7, 42));
}

#[test]
fn socket_addr_v6_preserves_flowinfo_and_scope_id() {
    let addr = SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 7, 42);

    // serde discards the flow information and the scope id.
    let mut plain = Vec::new();
    serialize(Full::new(), &mut plain, &addr).unwrap();
    let plain_deserialized: SocketAddrV6 = deserialize(Full::new(), plain.as_slice()).unwrap();
    assert_eq!(plain_deserialized.flowinfo(), 0);
    assert_eq!(plain_deserialized.scope_id(), 0);

    let data = to_compact_vec(addr, Full::new());
    let deserialized: SocketAddrV6 = from_compact_slice::<_, _>(&data, Full::new()).unwrap();
    assert_eq!(deserialized, addr);
    assert_eq!(deserialized.flowinfo(), 7);
    assert_eq!(deserialized.scope_id(), 42);
}

/// Struct with owned fields using compacted representations.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Owned {
    #[serde(with = "postbag::compact")]
    result: Result<u32, String>,
    #[serde(with = "postbag::compact")]
    duration: Duration,
    #[serde(with = "postbag::compact")]
    system_time: SystemTime,
    #[serde(with = "postbag::compact")]
    range: Range<u32>,
    #[serde(with = "postbag::compact")]
    range_inclusive: RangeInclusive<String>,
    #[serde(with = "postbag::compact")]
    range_from: RangeFrom<u32>,
    #[serde(with = "postbag::compact")]
    range_to: RangeTo<u32>,
    #[serde(with = "postbag::compact")]
    bound: Bound<u32>,
    #[serde(with = "postbag::compact")]
    ip_addr: IpAddr,
    #[serde(with = "postbag::compact")]
    socket_addr: SocketAddr,
    #[serde(with = "postbag::compact")]
    socket_addr_v6: SocketAddrV6,
}

/// Struct borrowing the fields of [Owned].
#[derive(Debug, Serialize)]
struct Borrowed<'a> {
    #[serde(with = "postbag::compact")]
    result: &'a Result<u32, String>,
    #[serde(with = "postbag::compact")]
    duration: &'a Duration,
    #[serde(with = "postbag::compact")]
    system_time: &'a SystemTime,
    #[serde(with = "postbag::compact")]
    range: &'a Range<u32>,
    #[serde(with = "postbag::compact")]
    range_inclusive: &'a RangeInclusive<String>,
    #[serde(with = "postbag::compact")]
    range_from: &'a RangeFrom<u32>,
    #[serde(with = "postbag::compact")]
    range_to: &'a RangeTo<u32>,
    #[serde(with = "postbag::compact")]
    bound: &'a Bound<u32>,
    #[serde(with = "postbag::compact")]
    ip_addr: &'a IpAddr,
    #[serde(with = "postbag::compact")]
    socket_addr: &'a SocketAddr,
    #[serde(with = "postbag::compact")]
    socket_addr_v6: &'a SocketAddrV6,
}

#[test]
fn borrowed_fields() {
    let owned = Owned {
        result: Err("failed".to_string()),
        duration: Duration::from_millis(1500),
        system_time: SystemTime::now(),
        range: 3..10,
        range_inclusive: "a".to_string()..="z".to_string(),
        range_from: 3..,
        range_to: ..10,
        bound: Bound::Included(5),
        ip_addr: IpAddr::V6(Ipv6Addr::LOCALHOST),
        socket_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080)),
        socket_addr_v6: SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 7, 42),
    };

    let borrowed = Borrowed {
        result: &owned.result,
        duration: &owned.duration,
        system_time: &owned.system_time,
        range: &owned.range,
        range_inclusive: &owned.range_inclusive,
        range_from: &owned.range_from,
        range_to: &owned.range_to,
        bound: &owned.bound,
        ip_addr: &owned.ip_addr,
        socket_addr: &owned.socket_addr,
        socket_addr_v6: &owned.socket_addr_v6,
    };

    borrowed_matches_owned(&owned, &borrowed, Full::new());
    borrowed_matches_owned(&owned, &borrowed, Slim::new());
}

/// Verifies that the borrowed struct serializes to the same data as the owned
/// struct and that the data deserializes back into the owned struct.
#[track_caller]
fn borrowed_matches_owned<const WITH_IDENTS: bool>(owned: &Owned, borrowed: &Borrowed, cfg: Cfg<WITH_IDENTS>) {
    let mut owned_data = Vec::new();
    serialize(cfg, &mut owned_data, owned).unwrap();

    let mut borrowed_data = Vec::new();
    serialize(cfg, &mut borrowed_data, borrowed).unwrap();

    assert_eq!(borrowed_data, owned_data, "borrowed representation differs from owned representation");

    let deserialized: Owned = deserialize(cfg, borrowed_data.as_slice()).unwrap();
    assert_eq!(deserialized, *owned);
}

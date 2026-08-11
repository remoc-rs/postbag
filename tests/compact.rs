use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ops::Bound,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use postbag::{
    cfg::{Cfg, Full, Slim},
    compact::Compactable,
    deserialize, serialize,
};

/// Wrapper serializing its value using its compacted representation.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "T: Compactable + Clone")]
struct Compact<T: Compactable + Clone>(#[serde(with = "postbag::compact")] T);

/// Serializes the value directly and using its compacted representation,
/// verifying that the compacted representation loops back.
///
/// If `check_size` is set and serialization is performed with identifiers, the
/// compacted representation must not be larger than the plain representation.
#[track_caller]
fn compact_loopback_with_cfg<T, CFG>(value: &T, check_size: bool)
where
    T: Compactable + Clone + Debug + PartialEq,
    CFG: Cfg,
{
    let mut plain = Vec::new();
    let plain_len = match serialize::<CFG, _, _>(&mut plain, value) {
        Ok(()) => Some(plain.len()),
        Err(_) => None,
    };

    let mut compact = Vec::new();
    serialize::<CFG, _, _>(&mut compact, &Compact(value.clone())).expect("compact serialization failed");

    println!("{value:?}: plain {plain_len:?} bytes, compact {} bytes", compact.len());
    if let Some(plain_len) = plain_len
        && check_size
        && CFG::with_idents()
    {
        assert!(
            compact.len() <= plain_len,
            "compacted representation of {value:?} is larger than plain representation"
        );
    }

    let deserialized: Compact<T> =
        deserialize::<CFG, _, _>(compact.as_slice()).expect("compact deserialization failed");
    assert_eq!(deserialized.0, *value, "deserialized value does not match original value");
}

/// Checks the compacted representation with all configurations.
#[track_caller]
fn compact_loopback<T>(value: T)
where
    T: Compactable + Clone + Debug + PartialEq,
{
    compact_loopback_with_cfg::<_, Full>(&value, true);
    compact_loopback_with_cfg::<_, Slim>(&value, true);
}

/// Checks the compacted representation with all configurations, without
/// verifying that it is not larger than the plain representation.
#[track_caller]
fn compact_loopback_unchecked_size<T>(value: T)
where
    T: Compactable + Clone + Debug + PartialEq,
{
    compact_loopback_with_cfg::<_, Full>(&value, false);
    compact_loopback_with_cfg::<_, Slim>(&value, false);
}

/// Serializes the value using its compacted representation.
fn to_compact_vec<T, CFG>(value: &T) -> Vec<u8>
where
    T: Compactable + Clone,
    CFG: Cfg,
{
    let mut compact = Vec::new();
    serialize::<CFG, _, _>(&mut compact, &Compact(value.clone())).expect("compact serialization failed");
    compact
}

/// Deserializes the value from its compacted representation.
fn from_compact_slice<T, CFG>(data: &[u8]) -> postbag::Result<T>
where
    T: Compactable + Clone,
    CFG: Cfg,
{
    deserialize::<CFG, _, Compact<T>>(data).map(|value| value.0)
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
    let data = to_compact_vec::<_, Full>(&Duration::MAX);
    from_compact_slice::<Duration, Full>(&data).expect("Duration::MAX must be representable");

    let mut invalid_nanos = Vec::new();
    serialize::<Full, _, _>(&mut invalid_nanos, &(0u64, 1_000_000_000u32)).unwrap();
    from_compact_slice::<Duration, Full>(&invalid_nanos).expect_err("invalid nanoseconds must be rejected");
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
        serialize::<Full, _, _>(&mut plain, &before_epoch).expect_err("serde must reject pre-epoch times");

        compact_loopback(before_epoch);
    }
}

#[test]
fn system_time_rejects_invalid() {
    let mut invalid_nanos = Vec::new();
    serialize::<Full, _, _>(&mut invalid_nanos, &(0i64, 1_000_000_000u32)).unwrap();
    from_compact_slice::<SystemTime, Full>(&invalid_nanos).expect_err("invalid nanoseconds must be rejected");

    // Whether extreme values are representable is platform-dependent, but they
    // must never cause a panic.
    for secs in [i64::MIN, i64::MAX] {
        let mut extreme = Vec::new();
        serialize::<Full, _, _>(&mut extreme, &(secs, 999_999_999u32)).unwrap();
        if let Ok(time) = from_compact_slice::<SystemTime, Full>(&extreme) {
            assert_eq!(to_compact_vec::<_, Full>(&time), extreme);
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
    serialize::<Full, _, _>(&mut plain, &addr).unwrap();
    let plain_deserialized: SocketAddrV6 = deserialize::<Full, _, _>(plain.as_slice()).unwrap();
    assert_eq!(plain_deserialized.flowinfo(), 0);
    assert_eq!(plain_deserialized.scope_id(), 0);

    let data = to_compact_vec::<_, Full>(&addr);
    let deserialized: SocketAddrV6 = from_compact_slice::<_, Full>(&data).unwrap();
    assert_eq!(deserialized, addr);
    assert_eq!(deserialized.flowinfo(), 7);
    assert_eq!(deserialized.scope_id(), 42);
}

//! The header a stream begins with.

use serde::{Deserialize, Serialize};

use postbag::{
    Error,
    cfg::{Full, Slim, Version},
    from_full_slice, from_slice, from_slim_slice, to_full_vec, to_slim_vec, to_vec,
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Value {
    a: u32,
    b: String,
}

fn value() -> Value {
    Value { a: 7, b: "hi".into() }
}

/// Magic, then the fixed bits, the identifier flag and the version.
#[test]
fn the_header_is_two_bytes() {
    let full = to_full_vec(&value()).unwrap();
    let slim = to_slim_vec(&value()).unwrap();

    assert_eq!(full[..2], [0xba, 0b1011_0001]);
    assert_eq!(slim[..2], [0xba, 0b1010_0001]);

    // Only the identifier bit tells the two headers apart.
    assert_eq!(full[1] ^ slim[1], 0b0001_0000);
}

/// Switching it off writes the value and nothing else.
#[test]
fn without_a_header_the_value_stands_alone() {
    let with = to_full_vec(&value()).unwrap();
    let without = to_vec(Full::new().with_header(false), &value()).unwrap();

    assert_eq!(with.len(), without.len() + 2);
    assert_eq!(with[2..], without[..]);
}

#[test]
fn a_header_round_trips() {
    assert_eq!(from_full_slice::<Value>(&to_full_vec(&value()).unwrap()).unwrap(), value());
    assert_eq!(from_slim_slice::<Value>(&to_slim_vec(&value()).unwrap()).unwrap(), value());

    let cfg = Full::new().with_header(false);
    assert_eq!(from_slice::<Value, _>(cfg, &to_vec(cfg, &value()).unwrap()).unwrap(), value());
}

/// Reading `Full` data as `Slim` used to yield whatever the bytes happened to
/// mean; the header says which it is.
#[test]
fn a_swapped_configuration_is_refused() {
    let full = to_full_vec(&value()).unwrap();
    let slim = to_slim_vec(&value()).unwrap();

    assert!(matches!(from_slim_slice::<Value>(&full), Err(Error::WithIdentsMismatch(true))));
    assert!(matches!(from_full_slice::<Value>(&slim), Err(Error::WithIdentsMismatch(false))));
}

/// Data that never had a header, and data that is no Postbag data at all.
#[test]
fn what_is_not_a_header_is_refused() {
    let headerless = to_vec(Full::new().with_header(false), &value()).unwrap();
    assert!(matches!(from_full_slice::<Value>(&headerless), Err(Error::BadHeader)));

    // No UTF-8 text begins with the magic, so text is always turned down.
    for text in ["{\"a\":7}", "a,b,c", "<xml/>", ""] {
        let res = from_full_slice::<Value>(text.as_bytes());
        assert!(matches!(res, Err(Error::BadHeader) | Err(Error::Io(_))), "{text:?} gave {res:?}");
    }
}

/// A version this build does not know, and the two values a header never
/// states: 0 identifies Postbag 0.4, 15 is reserved for extending the field.
#[test]
fn an_unknown_version_is_refused() {
    let mut bytes = to_full_vec(&value()).unwrap();

    for version in [0u8, 2, 7, 15] {
        bytes[1] = (bytes[1] & !0b0000_1111) | version;
        let res = from_full_slice::<Value>(&bytes);
        assert!(
            matches!(res, Err(Error::UnsupportedVersion(v)) if v == version),
            "version {version} gave {res:?}"
        );
    }
}

/// The fixed bits are what tells a header from a stray byte pair.
#[test]
fn a_wrong_fixed_pattern_is_refused() {
    let bytes = to_full_vec(&value()).unwrap();

    for pattern in [0b0000_0000, 0b0100_0000, 0b1100_0000, 0b1110_0000] {
        let mut bytes = bytes.clone();
        bytes[1] = (bytes[1] & !0b1110_0000) | pattern;
        assert!(matches!(from_full_slice::<Value>(&bytes), Err(Error::BadHeader)), "pattern {pattern:#010b}");
    }

    // Neither an all-zero nor an all-ones second byte passes.
    for second in [0x00, 0xff] {
        let mut bytes = bytes.clone();
        bytes[1] = second;
        assert!(from_full_slice::<Value>(&bytes).is_err(), "second byte {second:#04x}");
    }
}

/// Postbag 0.4 knows no header, so asking for that version writes none
/// whatever the setting says.
#[test]
fn the_legacy_version_never_carries_a_header() {
    let legacy = Full::new().with_version(Version::Postbag0_4);
    assert!(!legacy.header());
    assert!(!legacy.with_header(true).header());

    let bytes = to_vec(legacy.with_header(true), &value()).unwrap();
    assert_ne!(bytes[0], 0xba, "a header was written for a version that knows none");
    assert_eq!(from_slice::<Value, _>(legacy, &bytes).unwrap(), value());
}

/// The setting is order independent, so it does not matter whether the
/// version or the header is stated first.
#[test]
fn the_setting_does_not_depend_on_order() {
    let a = Full::new().with_header(true).with_version(Version::Postbag0_4);
    let b = Full::new().with_version(Version::Postbag0_4).with_header(true);
    assert_eq!(a.header(), b.header());

    let c = Slim::new().with_header(false).with_version(Version::Postbag1);
    let d = Slim::new().with_version(Version::Postbag1).with_header(false);
    assert_eq!(c.header(), d.header());
}

/// Truncated input fails rather than being taken for something else.
#[test]
fn a_truncated_header_is_refused() {
    let bytes = to_full_vec(&value()).unwrap();

    for len in 0..2 {
        assert!(from_full_slice::<Value>(&bytes[..len]).is_err(), "truncated to {len} bytes");
    }
}

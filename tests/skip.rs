//! Tests for the conditional `skip_serializing_if` predicates of `postbag::skip`.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use postbag::cfg::{Cfg, Full, Slim, Version};

/// A struct that may omit fields in the first, middle and last position.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
struct Sparse {
    #[serde(skip_serializing_if = "postbag::skip::Option::is_none", default)]
    first: Option<u32>,
    second: u32,
    #[serde(skip_serializing_if = "postbag::skip::Vec::is_empty", default)]
    middle: Vec<u8>,
    third: u32,
    #[serde(skip_serializing_if = "postbag::skip::String::is_empty", default)]
    last: String,
}

impl Sparse {
    /// A value whose every skippable field is empty.
    fn empty() -> Self {
        Self { second: 2, third: 3, ..Default::default() }
    }

    /// A value whose every skippable field holds something.
    fn filled() -> Self {
        Self { first: Some(1), second: 2, middle: vec![7, 8], third: 3, last: "l".to_string() }
    }
}

/// Round-trips every combination of present and omitted fields.
#[track_caller]
fn round_trip<const WITH_IDENTS: bool>(cfg: Cfg<WITH_IDENTS>) {
    for first in [None, Some(1)] {
        for middle in [Vec::new(), vec![7, 8]] {
            for last in ["", "l"] {
                let value = Sparse { first, second: 2, middle: middle.clone(), third: 3, last: last.to_string() };

                let bytes = postbag::to_vec(cfg, &value).unwrap();
                let back: Sparse = postbag::from_slice(cfg, bytes.as_slice()).unwrap();

                assert_eq!(back, value, "{value:?} did not round-trip using {cfg:?}");
            }
        }
    }
}

#[test]
fn round_trips_under_every_configuration() {
    round_trip(Full::new());
    round_trip(Slim::new());
    round_trip(Full::new().with_version(Version::Postbag0_4));
    round_trip(Slim::new().with_version(Version::Postbag0_4));
    round_trip(Full::new().with_header(false));
    round_trip(Slim::new().with_header(false));

    // Refusing to leave a field out is always safe to read back.
    round_trip(Full::new().with_allow_skip(false));
    round_trip(Slim::new().with_allow_skip(false));
    round_trip(Slim::new().with_allow_skip(true));
}

#[test]
fn leaving_a_field_out_is_allowed_by_default() {
    assert_eq!(Full::new().allow_skip(), !cfg!(postbag_fast_compile));
}

#[test]
fn slim_never_leaves_a_field_out() {
    // Fields are found by their position, so the setting cannot apply.
    assert!(!Slim::new().allow_skip());
    assert!(!Slim::new().with_allow_skip(true).allow_skip());

    let bytes = postbag::to_vec(Slim::new().with_allow_skip(true), &Sparse::empty()).unwrap();
    let unchanged = postbag::to_vec(Slim::new(), &Sparse::empty()).unwrap();
    assert_eq!(bytes, unchanged, "the setting must not reach Slim");
}

/// The identifier of a field that [`Sparse`] leaves out when it may.
const OMITTED_IDENT: &[u8] = b"first";

#[test]
fn a_reader_that_cannot_cope_with_a_gap_can_be_accommodated() {
    // What a build that does not use `postbag_fast_compile` sets once it
    // learns that the reader of its data does.
    let cfg = Full::new().with_allow_skip(false);
    let bytes = postbag::to_vec(cfg, &Sparse::empty()).unwrap();

    assert!(
        bytes.windows(OMITTED_IDENT.len()).any(|w| w == OMITTED_IDENT),
        "every field must be written out for a reader that cannot cope with a gap"
    );

    // Which costs exactly what writing every field costs, no more.
    let filled = postbag::to_vec(cfg, &Sparse::filled()).unwrap();
    assert!(bytes.len() <= filled.len());

    let back: Sparse = postbag::from_slice(cfg, bytes.as_slice()).unwrap();
    assert_eq!(back, Sparse::empty());
}

#[test]
fn a_build_that_cannot_read_a_gap_writes_none() {
    // `postbag_fast_compile` overrides the setting, so that such a build never
    // writes what it could not read back itself.
    assert_eq!(Full::new().with_allow_skip(true).allow_skip(), !cfg!(postbag_fast_compile));

    let cfg = Full::new().with_allow_skip(true);
    let bytes = postbag::to_vec(cfg, &Sparse::empty()).unwrap();

    let written = bytes.windows(OMITTED_IDENT.len()).any(|w| w == OMITTED_IDENT);
    assert_eq!(written, cfg!(postbag_fast_compile), "the setting must not override the build");

    // Whatever such a build writes, it reads back.
    let back: Sparse = postbag::from_slice(cfg, bytes.as_slice()).unwrap();
    assert_eq!(back, Sparse::empty());
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile omits no field at all")]
fn full_omits_the_empty_fields() {
    let empty = postbag::to_vec(Full::new(), &Sparse::empty()).unwrap();
    let filled = postbag::to_vec(Full::new(), &Sparse::filled()).unwrap();

    assert!(empty.len() < filled.len(), "Full must omit an empty field");
    assert!(
        !empty.windows(OMITTED_IDENT.len()).any(|w| w == OMITTED_IDENT),
        "an omitted field must leave no identifier behind"
    );
}

#[test]
fn slim_omits_nothing() {
    let empty = postbag::to_vec(Slim::new(), &Sparse::empty()).unwrap();
    let also_empty = postbag::to_vec(Slim::new(), &Sparse::empty()).unwrap();
    let filled = postbag::to_vec(Slim::new(), &Sparse::filled()).unwrap();

    assert_eq!(empty, also_empty);

    // Every field is written, so the two values state the same field count.
    let empty_back: Sparse = postbag::from_slice(Slim::new(), empty.as_slice()).unwrap();
    let filled_back: Sparse = postbag::from_slice(Slim::new(), filled.as_slice()).unwrap();
    assert_eq!(empty_back, Sparse::empty());
    assert_eq!(filled_back, Sparse::filled());
}

#[test]
fn the_predicates_accept_what_dereferences_to_them() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
    struct Deref<'a> {
        #[serde(skip_serializing_if = "postbag::skip::String::is_empty", default)]
        boxed_str: Box<str>,
        #[serde(skip_serializing_if = "postbag::skip::String::is_empty", default)]
        cow_str: Cow<'a, str>,
        #[serde(skip_serializing_if = "postbag::skip::Vec::is_empty", default)]
        boxed_slice: Box<[u8]>,
        #[serde(skip_serializing_if = "postbag::skip::VecDeque::is_empty", default)]
        deque: std::collections::VecDeque<u8>,
        #[serde(skip_serializing_if = "postbag::skip::BTreeMap::is_empty", default)]
        map: std::collections::BTreeMap<u32, u32>,
        #[serde(skip_serializing_if = "postbag::skip::BTreeSet::is_empty", default)]
        set: std::collections::BTreeSet<u32>,
        #[serde(skip_serializing_if = "postbag::skip::HashMap::is_empty", default)]
        hash_map: std::collections::HashMap<u32, u32>,
        #[serde(skip_serializing_if = "postbag::skip::HashSet::is_empty", default)]
        hash_set: std::collections::HashSet<u32>,
        tail: u32,
    }

    let value = Deref { tail: 42, ..Default::default() };

    for cfg in [Full::new(), Full::new().with_header(false)] {
        let bytes = postbag::to_vec(cfg, &value).unwrap();
        let back: Deref = postbag::from_slice(cfg, bytes.as_slice()).unwrap();
        assert_eq!(back, value);
    }

    let bytes = postbag::to_vec(Slim::new(), &value).unwrap();
    let back: Deref = postbag::from_slice(Slim::new(), bytes.as_slice()).unwrap();
    assert_eq!(back, value);
}

/// A struct whose every skippable field is compared against its default.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
struct Defaulted {
    #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
    count: u32,
    kept: u32,
    #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
    flag: bool,
    #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
    name: String,
    #[serde(skip_serializing_if = "postbag::skip::is_default", default)]
    nested: Sparse,
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile omits no field at all")]
fn a_default_value_is_omitted() {
    let bytes = postbag::to_vec(Full::new(), &Defaulted { kept: 1, ..Default::default() }).unwrap();

    for ident in [&b"count"[..], b"flag", b"name", b"nested"] {
        assert!(!bytes.windows(ident.len()).any(|w| w == ident), "a field holding its default must be left out");
    }
    assert!(bytes.windows(4).any(|w| w == b"kept"), "a field without the attribute stays");
}

#[test]
fn a_value_that_is_not_its_default_is_kept() {
    for value in [
        Defaulted { count: 7, ..Default::default() },
        Defaulted { flag: true, ..Default::default() },
        Defaulted { name: "n".to_string(), ..Default::default() },
        Defaulted { nested: Sparse::filled(), ..Default::default() },
        Defaulted { count: 7, kept: 1, flag: true, name: "n".to_string(), nested: Sparse::filled() },
        Defaulted::default(),
    ] {
        for cfg in [Full::new(), Full::new().with_allow_skip(false)] {
            let bytes = postbag::to_vec(cfg, &value).unwrap();
            let back: Defaulted = postbag::from_slice(cfg, bytes.as_slice()).unwrap();
            assert_eq!(back, value, "{value:?} did not round-trip using {cfg:?}");
        }

        let bytes = postbag::to_vec(Slim::new(), &value).unwrap();
        let back: Defaulted = postbag::from_slice(Slim::new(), bytes.as_slice()).unwrap();
        assert_eq!(back, value, "{value:?} did not round-trip using Slim");
    }
}

#[test]
fn slim_keeps_a_default_value() {
    let value = Defaulted { kept: 1, ..Default::default() };

    let bytes = postbag::to_vec(Slim::new(), &value).unwrap();
    let unchanged = postbag::to_vec(Slim::new().with_allow_skip(true), &value).unwrap();

    assert_eq!(bytes, unchanged);
    assert!(
        postbag::from_slice::<Defaulted, false>(Slim::new(), bytes.as_slice()).is_ok(),
        "Slim must write every field, whatever it holds"
    );
}

#[test]
fn a_struct_variant_omits_its_fields_too() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Enum {
        Variant {
            #[serde(skip_serializing_if = "postbag::skip::Option::is_none", default)]
            first: Option<u32>,
            second: u32,
            #[serde(skip_serializing_if = "postbag::skip::Option::is_none", default)]
            last: Option<u32>,
        },
    }

    for first in [None, Some(1)] {
        for last in [None, Some(9)] {
            let value = Enum::Variant { first, second: 2, last };

            for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
                let bytes = postbag::to_vec(cfg, &value).unwrap();
                let back: Enum = postbag::from_slice(cfg, bytes.as_slice()).unwrap();
                assert_eq!(back, value, "{value:?} did not round-trip using {cfg:?}");
            }

            let bytes = postbag::to_vec(Slim::new(), &value).unwrap();
            let back: Enum = postbag::from_slice(Slim::new(), bytes.as_slice()).unwrap();
            assert_eq!(back, value);
        }
    }
}

#[test]
fn omitting_is_allowed_while_postbag_is_not_serializing() {
    assert!(postbag::skip::is_allowed(), "another data format must be left to decide for itself");

    postbag::to_vec(Slim::new(), &Sparse::empty()).unwrap();

    assert!(postbag::skip::is_allowed(), "the setting must be restored once serialization ends");
}

#[test]
fn a_deserialization_leaves_another_data_format_to_decide_for_itself() {
    thread_local! {
        /// What `is_allowed` said while the field below was being read.
        static SEEN: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    /// Records what `is_allowed` says in the middle of a deserialization.
    fn note_and_read<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
        SEEN.set(Some(postbag::skip::is_allowed()));
        u32::deserialize(deserializer)
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Watched {
        #[serde(deserialize_with = "note_and_read")]
        value: u32,
    }

    // `Slim` refuses to omit a field while serializing, but a deserialization
    // says nothing about the format a value nested inside it is written to.
    let bytes = postbag::to_vec(Slim::new(), &Watched { value: 42 }).unwrap();
    let back: Watched = postbag::from_slice(Slim::new(), bytes.as_slice()).unwrap();

    assert_eq!(back.value, 42);
    assert_eq!(SEEN.get(), Some(true), "a deserialization must not hold back another data format");
    assert!(postbag::skip::is_allowed(), "the record must be restored once deserialization ends");
}

#[test]
fn a_panic_restores_the_setting() {
    /// Panics while being serialized.
    struct Panicking;

    impl Serialize for Panicking {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            panic!("serialization panicked");
        }
    }

    let result = std::panic::catch_unwind(|| postbag::to_vec(Slim::new(), &Panicking));

    assert!(result.is_err());
    assert!(postbag::skip::is_allowed(), "a panic must restore the setting");
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile omits no field at all")]
fn a_nested_serialization_restores_the_setting_of_the_one_around_it() {
    /// Serializes its value as a `Slim` buffer of its own, the way a value
    /// carrying pre-encoded data would.
    fn as_slim_buffer<T: Serialize, S: serde::Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
        let bytes = postbag::to_vec(Slim::new(), value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_bytes(&bytes)
    }

    #[derive(Serialize)]
    struct Outer {
        #[serde(serialize_with = "as_slim_buffer")]
        nested: Sparse,
        #[serde(skip_serializing_if = "postbag::skip::Option::is_none")]
        after: Option<u32>,
    }

    let value = Outer { nested: Sparse::empty(), after: None };
    let bytes = postbag::to_vec(Full::new(), &value).unwrap();

    assert!(!bytes.windows(5).any(|w| w == b"after"), "the Slim serialization inside must not outlast itself");
}

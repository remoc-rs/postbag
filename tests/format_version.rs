//! What each version of the data format writes.
//!
//! In the `Full` configuration a field value sits in a skippable block that
//! states its length, so a value reaching the end of that block need not
//! state its length again. Postbag 1.0 stopped doing so; `Version::Postbag0_4`
//! still does.
//!
//! The byte-level tests are what keeps the two sides in step: writer and
//! reader must leave the same things out in the same places, and only a test
//! on the bytes notices when one of them stops doing so.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fmt::Debug, marker::PhantomData};

use postbag::{
    cfg::{Full, Slim, Version},
    deserialize, serialize,
};

/// The bytes stated below are the framing of the value alone, so nothing here
/// writes or expects the header a stream otherwise begins with. It would in
/// any case be absent under `Version::Postbag0_4`, which knows none, and so
/// would tell the two versions apart on its own.
fn full() -> Full {
    Full::new().with_header(false)
}

fn slim() -> Slim {
    Slim::new().with_header(false)
}

/// A struct with one numbered field, so that the bytes are the framing of
/// that field and nothing else.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct One<T> {
    #[serde(rename = "_0")]
    v: T,
}

impl<T> One<T> {
    fn new(v: T) -> Self {
        Self { v }
    }
}

/// Serializes and deserializes under both versions, checking that the value
/// survives each, and returns the bytes each one wrote.
#[track_caller]
fn both<T>(value: &T) -> (Vec<u8>, Vec<u8>)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    let mut written = Vec::new();

    for version in [Version::Postbag1, Version::Postbag0_4] {
        let cfg = full().with_version(version);

        let mut bytes = Vec::new();
        serialize(cfg, &mut bytes, value).expect("serialization failed");
        let back: T = deserialize(cfg, bytes.as_slice()).expect("deserialization failed");
        assert_eq!(back, *value, "value did not survive {version:?}");

        written.push(bytes);
    }

    (written.remove(0), written.remove(0))
}

/// Asserts that both versions write the same bytes, which is the case for
/// every value that does not reach the end of a block of its own.
#[track_caller]
fn unaffected<T>(value: &T)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    let (v1, v0_4) = both(value);
    assert_eq!(v1, v0_4, "the version changed the bytes of a value it should not reach");
}

// --------------------------------------------------------------------------
// What 1.0 changed
// --------------------------------------------------------------------------

#[test]
fn string_field() {
    let (v1, v0_4) = both(&One::new("temp".to_string()));

    // count, identifier `_0`, block length, the four bytes.
    assert_eq!(v1, b"\x01\x41\x04temp");
    // What Postbag 0.4 wrote: the block says five, the string says four.
    assert_eq!(v0_4, b"\x01\x41\x05\x04temp");
}

#[test]
fn empty_string_field() {
    let (v1, v0_4) = both(&One::new(String::new()));

    assert_eq!(v1, b"\x01\x41\x00");
    assert_eq!(v0_4, b"\x01\x41\x01\x00");
}

#[test]
fn string_length_is_bytes_not_chars() {
    // Two characters, three bytes: the block length is the byte length, which
    // is the only length a string needs.
    let (v1, v0_4) = both(&One::new("°C".to_string()));

    assert_eq!(v1, b"\x01\x41\x03\xc2\xb0C");
    assert_eq!(v0_4, b"\x01\x41\x04\x03\xc2\xb0C");
}

#[test]
fn bytes_field() {
    let (v1, v0_4) = both(&One::new(serde_bytes::ByteBuf::from(vec![1u8, 2, 3])));

    assert_eq!(v1, b"\x01\x41\x03\x01\x02\x03");
    assert_eq!(v0_4, b"\x01\x41\x04\x03\x01\x02\x03");
}

#[test]
fn char_field() {
    // A char is written through the same path as a string, so it loses its
    // length in the same case — and the reader parses it separately, which is
    // exactly where the two sides could drift apart.
    let (v1, v0_4) = both(&One::new('°'));

    assert_eq!(v1, b"\x01\x41\x02\xc2\xb0");
    assert_eq!(v0_4, b"\x01\x41\x03\x02\xc2\xb0");
}

#[test]
fn option_passes_it_on() {
    // `Some` writes its tag and then the value, which still ends the block.
    let (v1, v0_4) = both(&One::new(Some("hi".to_string())));

    assert_eq!(v1, b"\x01\x41\x03\x01hi");
    assert_eq!(v0_4, b"\x01\x41\x04\x01\x02hi");

    // Nothing follows a `None`, so there is nothing to shorten.
    unaffected(&One::new(Option::<String>::None));
}

#[test]
fn nested_option_passes_it_on() {
    let (v1, v0_4) = both(&One::new(Some(Some("hi".to_string()))));

    assert_eq!(v1, b"\x01\x41\x04\x01\x01hi");
    assert_eq!(v0_4, b"\x01\x41\x05\x01\x01\x02hi");

    // `Some(None)` stays distinct from `None` under both settings.
    let (v1, v0_4) = both(&One::new(Some(Option::<String>::None)));
    assert_eq!(v1, b"\x01\x41\x02\x01\x00");
    assert_eq!(v0_4, v1);
}

#[test]
fn newtype_struct_passes_it_on() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Name(String);

    // A newtype struct writes nothing of its own.
    let (v1, v0_4) = both(&One::new(Name("hi".to_string())));

    assert_eq!(v1, b"\x01\x41\x02hi");
    assert_eq!(v0_4, b"\x01\x41\x03\x02hi");
}

#[test]
fn a_nested_struct_leaves_out_its_field_count() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Inner {
        #[serde(rename = "_0")]
        text: String,
    }

    // Two savings: the inner struct's field count, because the block says
    // where its fields end, and the inner string's length, because its own
    // block says where it ends.
    let (v1, v0_4) = both(&One::new(Inner { text: "hi".to_string() }));

    assert_eq!(v1, b"\x01\x41\x04\x41\x02hi");
    assert_eq!(v0_4, b"\x01\x41\x06\x01\x41\x03\x02hi");
}

#[test]
fn a_top_level_struct_keeps_its_field_count() {
    // Nothing delimits it, so the count is the only thing that does.
    let mut bytes = Vec::new();
    serialize(full(), &mut bytes, &One::new(7u8)).unwrap();
    assert_eq!(bytes, b"\x01\x41\x01\x07");
}

#[test]
fn a_struct_in_a_sequence_keeps_its_field_count() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Inner {
        #[serde(rename = "_0")]
        n: u8,
    }

    // An element does not end its block, so it says how many fields it has.
    let (v1, v0_4) = both(&One::new(vec![Inner { n: 7 }, Inner { n: 8 }]));

    assert_eq!(v1, b"\x01\x41\x09\x02\x01\x41\x01\x07\x01\x41\x01\x08");
    assert_eq!(v0_4, v1);
}

// --------------------------------------------------------------------------
// What it left alone
// --------------------------------------------------------------------------

#[test]
fn a_sequence_keeps_its_count() {
    // The count is not the byte length: it cannot be recovered from one, and
    // it is what lets the reader allocate once.
    let (v1, v0_4) = both(&One::new(vec!["a".to_string(), "bb".to_string()]));

    assert_eq!(v1, b"\x01\x41\x06\x02\x01a\x02bb");
    assert_eq!(v0_4, v1);

    // An element that occupies no bytes at all is why the count has to stay.
    let (v1, _) = both(&One::new(vec![(), (), ()]));
    assert_eq!(v1, b"\x01\x41\x01\x03");
}

#[test]
fn a_map_keeps_its_count() {
    // Same reason as a sequence, and it does not take a contrived type to
    // reach it: an entry of a `BTreeMap<(), ()>` occupies no bytes, so the
    // count is the whole content of the block and an empty map would be
    // indistinguishable from one holding an entry.
    let (v1, v0_4) = both(&One::new(BTreeMap::<(), ()>::new()));
    assert_eq!(v1, b"\x01\x41\x01\x00");
    assert_eq!(v0_4, v1);

    let (v1, v0_4) = both(&One::new(BTreeMap::from([((), ())])));
    assert_eq!(v1, b"\x01\x41\x01\x01");
    assert_eq!(v0_4, v1);
}

#[test]
fn elements_and_members_keep_their_lengths() {
    // None of these values ends a block of its own: a reader arriving at one
    // cannot know that nothing follows it.
    unaffected(&One::new(vec!["a".to_string(), "bb".to_string()]));
    unaffected(&One::new(("a".to_string(), "bb".to_string())));
    unaffected(&One::new(("a".to_string(), 7u32)));
    unaffected(&One::new((7u32, "a".to_string())));
    unaffected(&One::new([[1u8, 2], [3, 4]]));
    unaffected(&One::new(BTreeMap::from([("k".to_string(), "v".to_string())])));
    unaffected(&One::new(vec![Some("a".to_string()), None]));
}

#[test]
fn an_enum_variant_passes_it_on() {
    // The variant identifier is written first and the payload still reaches
    // the end of the block, so a newtype variant behaves like `Some` and a
    // struct variant like a struct.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Unit {
        #[serde(rename = "_0")]
        Celsius,
        #[serde(rename = "_1")]
        Other(String),
        #[serde(rename = "_2")]
        Scaled {
            #[serde(rename = "_0")]
            by: u8,
        },
    }

    let (v1, v0_4) = both(&One::new(Unit::Other("K".to_string())));
    assert_eq!(v1, b"\x01\x41\x02\x42K");
    assert_eq!(v0_4, b"\x01\x41\x03\x42\x01K");

    let (v1, v0_4) = both(&One::new(Unit::Scaled { by: 7 }));
    assert_eq!(v1, b"\x01\x41\x04\x43\x41\x01\x07");
    assert_eq!(v0_4, b"\x01\x41\x05\x43\x01\x41\x01\x07");

    // A unit variant has no payload to shorten.
    let (v1, v0_4) = both(&One::new(Unit::Celsius));
    assert_eq!(v1, b"\x01\x41\x01\x41");
    assert_eq!(v0_4, v1);
}

#[test]
fn a_value_of_its_own_keeps_its_length() {
    // Nothing encloses a top-level value, so it has to state its own length.
    let mut bytes = Vec::new();
    serialize(full(), &mut bytes, &"temp".to_string()).unwrap();
    assert_eq!(bytes, b"\x04temp");

    unaffected(&"temp".to_string());
}

#[test]
fn nothing_is_read_beyond_the_value() {
    // Reading to the end of a block must never become reading to the end of
    // the input: a reader may well hold more than this one value.
    for value in ["temp".to_string(), String::new()] {
        let mut bytes = Vec::new();
        serialize(full(), &mut bytes, &value).unwrap();
        bytes.extend_from_slice(b"and more");

        let mut input = bytes.as_slice();
        let back: String = deserialize(full(), &mut input).unwrap();

        assert_eq!(back, value);
        assert_eq!(input, b"and more", "the reader was advanced past the value");
    }

    // The same for a string that does leave out its length.
    let mut bytes = Vec::new();
    serialize(full(), &mut bytes, &One::new("temp".to_string())).unwrap();
    bytes.extend_from_slice(b"and more");

    let mut input = bytes.as_slice();
    let back: One<String> = deserialize(full(), &mut input).unwrap();

    assert_eq!(back.v, "temp");
    assert_eq!(input, b"and more", "the reader was advanced past the value");
}

#[test]
fn slim_is_unaffected() {
    // `Slim` writes one block per struct rather than one per field, so no
    // field value reaches the end of a block.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Reading {
        sensor: u32,
        label: String,
    }

    let value = Reading { sensor: 300, label: "temp".to_string() };
    let mut written = Vec::new();

    for version in [Version::Postbag1, Version::Postbag0_4] {
        let cfg = slim().with_version(version);
        let mut bytes = Vec::new();
        serialize(cfg, &mut bytes, &value).unwrap();
        let back: Reading = deserialize(cfg, bytes.as_slice()).unwrap();
        assert_eq!(back, value);
        written.push(bytes);
    }

    assert_eq!(written[0], written[1]);
}

// --------------------------------------------------------------------------
// Long values, where a block is written in chunks
// --------------------------------------------------------------------------

/// A block carries at most this many bytes before it is continued in another,
/// so the reader has to be able to read a value across that boundary.
const MAX_BLOCK: usize = u16::MAX as usize;

#[test]
fn a_string_longer_than_a_block() {
    for len in [MAX_BLOCK - 1, MAX_BLOCK, MAX_BLOCK + 1, 2 * MAX_BLOCK, 2 * MAX_BLOCK + 1] {
        let value = One::new("x".repeat(len));
        let (v1, v0_4) = both(&value);

        // Every full block costs a two-byte length, the last one costs one to
        // three, and 0.4 states the string's own length on top of that.
        assert!(v1.len() > len, "at {len} bytes");
        assert!(v0_4.len() > v1.len(), "at {len} bytes");
    }
}

#[test]
fn bytes_longer_than_a_block() {
    for len in [MAX_BLOCK, MAX_BLOCK + 1] {
        let value = One::new(serde_bytes::ByteBuf::from(vec![0xab; len]));
        both(&value);
    }
}

#[test]
fn no_field_is_empty_in_full() {
    // This is what lets the field count go: the end of the block is the end
    // of the fields only if no field can occupy zero bytes. In `Full` every
    // field costs at least an identifier and a length, even a unit.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Units {
        #[serde(rename = "_0")]
        a: (),
        #[serde(rename = "_1")]
        b: u8,
        #[serde(rename = "_2")]
        c: (),
    }

    let (v1, v0_4) = both(&One::new(Units { a: (), b: 7, c: () }));

    assert_eq!(v1, b"\x01\x41\x07\x41\x00\x42\x01\x07\x43\x00");
    assert_eq!(v0_4, b"\x01\x41\x08\x03\x41\x00\x42\x01\x07\x43\x00");
}

#[test]
fn slim_needs_its_count_because_a_field_can_be_empty() {
    // The mirror image: without identifiers a field really can be zero bytes,
    // so the count is the only thing that says how many there are.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Units {
        a: (),
        b: (),
    }

    let bytes = postbag::to_vec(slim(), &Units { a: (), b: () }).unwrap();
    let back: Units = deserialize(slim(), bytes.as_slice()).unwrap();

    assert_eq!(back, Units { a: (), b: () });
    assert_eq!(bytes, b"\x02\x00", "the count is all there is");

    // A marker field is ordinary in a generic struct and zero sized.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Marked {
        a: u32,
        marker: PhantomData<u8>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Unmarked {
        a: u32,
    }

    let marked = postbag::to_vec(slim(), &Marked { a: 300, marker: PhantomData }).unwrap();
    let unmarked = postbag::to_vec(slim(), &Unmarked { a: 300 }).unwrap();

    assert_eq!(marked, b"\x02\x02\xac\x02");
    assert_eq!(unmarked, b"\x01\x02\xac\x02");
    assert_eq!(marked[1..], unmarked[1..], "only the count tells them apart");

    let err = postbag::from_slice::<Marked, _>(slim(), unmarked.as_slice()).unwrap_err();
    assert!(err.to_string().contains("invalid length"), "got {err}");
}

// --------------------------------------------------------------------------
// Naming a version to another program
// --------------------------------------------------------------------------

#[test]
fn versions_have_stable_bytes() {
    // Other programs exchange these to agree on a version, so the values are
    // part of the interface and may not be renumbered.
    assert_eq!(u8::from(Version::Postbag0_4), 0);
    assert_eq!(u8::from(Version::Postbag1), 1);

    assert_eq!(Version::try_from(0u8).unwrap(), Version::Postbag0_4);
    assert_eq!(Version::try_from(1u8).unwrap(), Version::Postbag1);
}

#[test]
fn versions_order_with_their_bytes() {
    // What lets two programs settle on the lower of the two they name.
    assert!(Version::Postbag0_4 < Version::Postbag1);
    assert!(u8::from(Version::Postbag0_4) < u8::from(Version::Postbag1));
    assert_eq!(Version::Postbag1.min(Version::Postbag0_4), Version::Postbag0_4);
}

#[test]
fn an_unknown_byte_names_no_version() {
    // A program built against a newer Postbag can name one this build has
    // never heard of.
    let err = Version::try_from(200u8).unwrap_err();

    assert_eq!(err, postbag::cfg::UnknownVersion(200));
    assert_eq!(err.to_string(), "unknown Postbag data format version 200");
}

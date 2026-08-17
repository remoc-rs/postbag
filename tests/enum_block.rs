//! Enum variants that carry their payload in a block of its own.
//!
//! A reader that does not know a variant, or that knows a smaller version of
//! one, has to be able to step over the payload. Where the payload already
//! reaches the end of a block — a struct field, and whatever passes that on —
//! the enclosing block is what delimits it. Everywhere else there was nothing
//! to delimit it at all, and a payload the reader did not consume was left in
//! the input for the next value to misread.
//!
//! So outside such a block a variant carries one of its own, and the high bit
//! of its tag says whether it does. That bit is free because no identifier
//! reaches it, which is what keeps a unit variant at the single byte it has
//! always been.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use postbag::{
    Error,
    cfg::{Full, Slim, Version},
    deserialize, to_vec,
};

/// The bytes stated below are the framing of the value alone, so nothing here
/// writes or expects the header a stream otherwise begins with.
fn full() -> Full {
    Full::new().with_header(false)
}

fn slim() -> Slim {
    Slim::new().with_header(false)
}

fn to_full_vec<T: Serialize + ?Sized>(value: &T) -> postbag::Result<Vec<u8>> {
    to_vec(full(), value)
}

fn from_full_slice<T: DeserializeOwned>(slice: &[u8]) -> postbag::Result<T> {
    postbag::from_slice(full(), slice)
}

/// Puts a value in a field, where it reaches the end of a block.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Wrap<T> {
    #[serde(rename = "_0")]
    v: T,
}

/// Every kind of variant, numbered so the bytes are the framing and nothing else.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Shape {
    #[serde(rename = "_0")]
    Unit,
    #[serde(rename = "_1")]
    Num(u8),
    #[serde(rename = "_2")]
    Text(String),
    #[serde(rename = "_3")]
    Rec {
        #[serde(rename = "_0")]
        w: u8,
        #[serde(rename = "_1")]
        h: u8,
    },
    #[serde(rename = "_4")]
    Pair(u8, u8),
    #[serde(rename = "_5")]
    Empty {},
}

// --------------------------------------------------------------------------
// What the tag costs
// --------------------------------------------------------------------------

#[test]
fn a_unit_variant_stays_a_single_byte() {
    // The whole point of putting the flag in the tag rather than always writing
    // a block: a sequence of plain variants must not double in size.
    let bytes = to_full_vec(&Wrap { v: vec![Shape::Unit, Shape::Unit, Shape::Unit] }).unwrap();

    assert_eq!(bytes, b"\x01\x41\x04\x03\x41\x41\x41");
}

#[test]
fn an_empty_struct_variant_is_a_unit_variant() {
    // `A {}` and `A` are the same bytes, so a variant declared with a body it
    // does not yet use costs nothing over one declared without.
    let unit = to_full_vec(&Wrap { v: vec![Shape::Unit] }).unwrap();
    let empty = to_full_vec(&Wrap { v: vec![Shape::Empty {}] }).unwrap();

    assert_eq!(unit, b"\x01\x41\x02\x01\x41");
    assert_eq!(empty, b"\x01\x41\x02\x01\x46");
    assert_eq!(unit.len(), empty.len(), "an empty body must cost nothing");
}

#[test]
fn a_payload_is_tagged_and_blocked() {
    // The tag carries the flag in its high bit and the block length follows.
    let bytes = to_full_vec(&Wrap { v: vec![Shape::Num(5)] }).unwrap();
    assert_eq!(bytes, b"\x01\x41\x04\x01\xc2\x01\x05");

    let bytes = to_full_vec(&Wrap { v: vec![Shape::Pair(2, 3)] }).unwrap();
    assert_eq!(bytes, b"\x01\x41\x05\x01\xc5\x02\x02\x03");
}

#[test]
fn a_payload_that_stated_a_length_pays_nothing() {
    // The payload reaches the end of the block it was given, so it leaves out
    // the length or count it used to state: the block length replaces a byte
    // that was there anyway.
    let bytes = to_full_vec(&Wrap { v: vec![Shape::Text("K".to_string())] }).unwrap();
    assert_eq!(bytes, b"\x01\x41\x04\x01\xc3\x01K", "the string states no length of its own");

    let bytes = to_full_vec(&Wrap { v: vec![Shape::Rec { w: 2, h: 3 }] }).unwrap();
    assert_eq!(bytes, b"\x01\x41\x09\x01\xc4\x06\x41\x01\x02\x42\x01\x03");
}

#[test]
fn a_variant_in_a_field_is_unchanged() {
    // Where the payload already reaches the end of a block there is nothing to
    // add, so the most common placement of an enum costs exactly what it did.
    assert_eq!(to_full_vec(&Wrap { v: Shape::Unit }).unwrap(), b"\x01\x41\x01\x41");
    assert_eq!(to_full_vec(&Wrap { v: Shape::Num(5) }).unwrap(), b"\x01\x41\x02\x42\x05");
    assert_eq!(to_full_vec(&Wrap { v: Shape::Text("K".into()) }).unwrap(), b"\x01\x41\x02\x43K");
    assert_eq!(to_full_vec(&Wrap { v: Shape::Pair(2, 3) }).unwrap(), b"\x01\x41\x03\x45\x02\x03");
    assert_eq!(
        to_full_vec(&Wrap { v: Shape::Rec { w: 2, h: 3 } }).unwrap(),
        b"\x01\x41\x07\x44\x41\x01\x02\x42\x01\x03"
    );
}

#[test]
fn a_top_level_variant_is_self_delimiting() {
    assert_eq!(to_full_vec(&Shape::Unit).unwrap(), b"\x41");
    assert_eq!(to_full_vec(&Shape::Num(5)).unwrap(), b"\xc2\x01\x05");

    // Which is what lets a value be followed by something else.
    let mut bytes = to_full_vec(&Shape::Num(5)).unwrap();
    bytes.extend_from_slice(b"and more");

    let mut input = bytes.as_slice();
    let back: Shape = deserialize(full(), &mut input).unwrap();

    assert_eq!(back, Shape::Num(5));
    assert_eq!(input, b"and more", "the reader was advanced past the value");
}

// --------------------------------------------------------------------------
// Variants the reader does not know
// --------------------------------------------------------------------------

/// What the writer has.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum New {
    #[serde(rename = "_0")]
    A,
    #[serde(rename = "_1")]
    B(u32),
    /// The variant the reader has never heard of, and it carries a payload.
    #[serde(rename = "_2")]
    C(u32),
}

/// What the reader has.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Old {
    #[serde(rename = "_0")]
    A,
    #[serde(rename = "_1")]
    B(u32),
    #[serde(other)]
    Unknown,
}

#[track_caller]
fn reads_back<T, R>(value: &T, expected: &R)
where
    T: Serialize,
    R: DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let bytes = to_full_vec(value).expect("serialization failed");
    let back: R = from_full_slice(&bytes).expect("deserialization failed");
    assert_eq!(back, *expected);
}

#[test]
fn an_unknown_payload_is_skipped_in_a_sequence() {
    reads_back(
        &Wrap { v: vec![New::A, New::C(7), New::B(9)] },
        &Wrap { v: vec![Old::A, Old::Unknown, Old::B(9)] },
    );
}

#[test]
fn an_unknown_payload_does_not_shift_what_follows() {
    // The one that used to be silent: `C`'s payload is 65, whose varint is the
    // identifier of `A`, so the element after it was read as `A` and the real
    // one discarded — without an error to show for it.
    let value = Wrap { v: vec![New::C(65), New::B(9)] };
    let bytes = to_full_vec(&value).unwrap();
    let back: Wrap<Vec<Old>> = from_full_slice(&bytes).unwrap();

    assert_eq!(back.v, vec![Old::Unknown, Old::B(9)]);
}

#[test]
fn an_unknown_payload_is_skipped_in_a_tuple() {
    // The other silent one: the payload was read as the element after it, and
    // the element that really was there vanished with the block.
    reads_back(&Wrap { v: (New::C(7), 42u8) }, &Wrap { v: (Old::Unknown, 42u8) });
}

#[test]
fn an_unknown_payload_is_skipped_in_a_map() {
    reads_back(
        &Wrap { v: BTreeMap::from([(1u8, New::C(7)), (2, New::B(9))]) },
        &Wrap { v: BTreeMap::from([(1u8, Old::Unknown), (2, Old::B(9))]) },
    );
}

#[test]
fn an_unknown_payload_is_skipped_at_the_top_level() {
    let bytes = to_full_vec(&New::C(7)).unwrap();

    let mut input = bytes.as_slice();
    let back: Old = deserialize(full(), &mut input).unwrap();

    assert_eq!(back, Old::Unknown);
    assert!(input.is_empty(), "the payload was left in the input, got {input:02x?}");
}

#[test]
fn an_unknown_payload_is_skipped_in_a_field() {
    // Where it already worked, and must keep working.
    reads_back(&Wrap { v: New::C(7) }, &Wrap { v: Old::Unknown });
}

// --------------------------------------------------------------------------
// A variant that grows
// --------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct Body {
    #[serde(rename = "_0", default)]
    f: u8,
    #[serde(rename = "_1", default)]
    g: String,
}

/// The four shapes a variant that may carry data can take. They are one shape
/// on the wire, and the point of the block is that they stay so wherever the
/// value sits.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AsUnit {
    #[serde(rename = "_0")]
    A,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AsEmpty {
    #[serde(rename = "_0")]
    A {},
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AsFields {
    #[serde(rename = "_0")]
    A {
        #[serde(rename = "_0", default)]
        f: u8,
        #[serde(rename = "_1", default)]
        g: String,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AsBody {
    #[serde(rename = "_0")]
    A(Body),
}

#[test]
fn the_shapes_of_a_variant_agree_in_a_sequence() {
    let unit = to_full_vec(&Wrap { v: vec![AsUnit::A] }).unwrap();
    let empty = to_full_vec(&Wrap { v: vec![AsEmpty::A {}] }).unwrap();
    let fields = to_full_vec(&Wrap { v: vec![AsFields::A { f: 5, g: "x".into() }] }).unwrap();
    let body = to_full_vec(&Wrap { v: vec![AsBody::A(Body { f: 5, g: "x".into() })] }).unwrap();

    assert_eq!(unit, empty, "an empty body is a unit variant");
    assert_eq!(fields, body, "a body of fields is a struct holding them");

    // Arriving from a variant that carried nothing, every field takes its default.
    assert_eq!(
        from_full_slice::<Wrap<Vec<AsFields>>>(&unit).unwrap().v,
        vec![AsFields::A { f: 0, g: String::new() }]
    );
    assert_eq!(from_full_slice::<Wrap<Vec<AsBody>>>(&unit).unwrap().v, vec![AsBody::A(Body::default())]);

    // And a reader that has not grown yet steps over what it does not know.
    assert_eq!(from_full_slice::<Wrap<Vec<AsUnit>>>(&fields).unwrap().v, vec![AsUnit::A]);
    assert_eq!(from_full_slice::<Wrap<Vec<AsEmpty>>>(&fields).unwrap().v, vec![AsEmpty::A {}]);

    // Between the two shapes that carry the fields, in both directions.
    assert_eq!(
        from_full_slice::<Wrap<Vec<AsBody>>>(&fields).unwrap().v,
        vec![AsBody::A(Body { f: 5, g: "x".into() })]
    );
    assert_eq!(
        from_full_slice::<Wrap<Vec<AsFields>>>(&body).unwrap().v,
        vec![AsFields::A { f: 5, g: "x".into() }]
    );
}

#[test]
fn a_variant_that_grew_does_not_shift_what_follows() {
    // The element after it is what an unskipped payload would have eaten.
    let bytes =
        to_full_vec(&Wrap { v: vec![AsFields::A { f: 5, g: "x".into() }, AsFields::A { f: 6, g: "y".into() }] })
            .unwrap();

    assert_eq!(from_full_slice::<Wrap<Vec<AsUnit>>>(&bytes).unwrap().v, vec![AsUnit::A, AsUnit::A]);
}

// --------------------------------------------------------------------------
// Input that does not follow the rules
// --------------------------------------------------------------------------

#[test]
fn a_reserved_tag_is_refused() {
    // The codes above the last numbered identifier name nothing, with the flag
    // set or clear.
    for tag in [0x7du8, 0x7e, 0x7f, 0xfd, 0xfe, 0xff] {
        let res = from_full_slice::<Shape>(&[tag, 0x00]);
        assert!(matches!(res, Err(Error::BadIdentifier)), "tag {tag:#04x} gave {res:?}");
    }
}

#[test]
fn a_block_that_is_not_there_is_refused() {
    // The tag claims a payload block and the input ends instead.
    let res = from_full_slice::<Shape>(&[0xc2]);
    assert!(res.is_err(), "got {res:?}");
}

#[test]
fn a_payload_that_is_not_there_is_refused() {
    // The tag says the variant carries nothing, but the reader wants a `u8`,
    // and reads it from an empty block rather than from what follows.
    let res = from_full_slice::<Wrap<Vec<Shape>>>(b"\x01\x41\x03\x01\x42\x05");
    assert!(matches!(res, Err(Error::EndOfBlock)), "got {res:?}");
}

// --------------------------------------------------------------------------
// The configurations this does not reach
// --------------------------------------------------------------------------

#[test]
fn slim_is_unaffected() {
    // `Slim` writes variant indices and has no identifiers to hide a flag in.
    let bytes = to_vec(slim(), &Wrap { v: vec![Shape::Num(5)] }).unwrap();
    assert_eq!(bytes, b"\x01\x03\x01\x01\x05");

    let bytes = to_vec(slim(), &Wrap { v: Shape::Rec { w: 2, h: 3 } }).unwrap();
    assert_eq!(bytes, b"\x01\x05\x03\x02\x02\x02\x03");

    let back: Wrap<Shape> = postbag::from_slice(slim(), &bytes).unwrap();
    assert_eq!(back.v, Shape::Rec { w: 2, h: 3 });
}

#[test]
fn the_legacy_format_is_unaffected() {
    let cfg = Full::new().with_version(Version::Postbag0_4);

    // No flag, no block, and a struct variant still states its field count.
    assert_eq!(to_vec(cfg, &Wrap { v: vec![Shape::Num(5)] }).unwrap(), b"\x01\x41\x03\x01\x42\x05");
    assert_eq!(to_vec(cfg, &Shape::Rec { w: 2, h: 3 }).unwrap(), b"\x44\x02\x41\x01\x02\x42\x01\x03");

    let bytes = to_vec(cfg, &Wrap { v: vec![Shape::Rec { w: 2, h: 3 }] }).unwrap();
    let back: Wrap<Vec<Shape>> = postbag::from_slice(cfg, &bytes).unwrap();
    assert_eq!(back.v, vec![Shape::Rec { w: 2, h: 3 }]);
}

//! Sequences and maps serialized without stating how many elements follow.
//!
//! Serde asks for this whenever an iterator cannot say how long it is — a
//! `filter` is enough. There is then no count to end the sequence, and the end
//! cannot be found from the bytes either, since an element may consume none
//! and how many bytes an element takes is the element's business. So each one
//! is announced instead.

use std::{cell::RefCell, collections::BTreeMap, fmt::Debug};

use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};

use postbag::{
    Error,
    cfg::{Cfg, Full, Slim},
    to_vec,
};

/// The bytes stated below are the framing of the value alone, so nothing here
/// writes or expects the header a stream otherwise begins with.
fn full_cfg() -> Full {
    Full::new().with_header(false)
}

fn slim_cfg() -> Slim {
    Slim::new().with_header(false)
}

fn to_full_vec<T: serde::Serialize + ?Sized>(value: &T) -> postbag::Result<Vec<u8>> {
    to_vec(full_cfg(), value)
}

fn to_slim_vec<T: serde::Serialize + ?Sized>(value: &T) -> postbag::Result<Vec<u8>> {
    to_vec(slim_cfg(), value)
}

fn from_full_slice<T: serde::de::DeserializeOwned>(slice: &[u8]) -> postbag::Result<T> {
    postbag::from_slice(full_cfg(), slice)
}

/// Serializes an iterator, which is what reaches the uncounted path.
struct Uncounted<I>(RefCell<Option<I>>);

impl<I> Uncounted<I> {
    fn new(iter: I) -> Self {
        Self(RefCell::new(Some(iter)))
    }
}

impl<I> Serialize for Uncounted<I>
where
    I: IntoIterator,
    I::Item: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.borrow_mut().take().expect("serialized once"))
    }
}

/// The same for a map.
struct UncountedMap<I>(RefCell<Option<I>>);

impl<I> UncountedMap<I> {
    fn new(iter: I) -> Self {
        Self(RefCell::new(Some(iter)))
    }
}

impl<I, K, V> Serialize for UncountedMap<I>
where
    I: IntoIterator<Item = (K, V)>,
    K: Serialize,
    V: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.borrow_mut().take().expect("serialized once"))
    }
}

/// Asserts that a value written without a count reads back whole, in both
/// configurations.
///
/// Takes a factory rather than a value, since serializing an iterator
/// consumes it and each configuration needs its own.
#[track_caller]
fn survives<T, R>(make: impl Fn() -> T, expected: &R)
where
    T: Serialize,
    R: DeserializeOwned + Debug + PartialEq,
{
    let full = to_full_vec(&make()).expect("full serialization");
    let slim = to_slim_vec(&make()).expect("slim serialization");

    assert!(full.starts_with(&[0x7d, 0x00]), "expected the uncounted path, got {full:02x?}");
    assert_eq!(full, slim, "the uncounted path does not depend on identifiers");

    let back: R = postbag::from_slice(full_cfg(), full.as_slice()).expect("full deserialization");
    assert_eq!(back, *expected);

    let back: R = postbag::from_slice(slim_cfg(), slim.as_slice()).expect("slim deserialization");
    assert_eq!(back, *expected);
}

#[test]
fn an_uncounted_sequence_survives() {
    survives(|| Uncounted::new(vec![1u32, 2, 3, 4].into_iter().filter(|n| n % 2 == 0)), &vec![2u32, 4]);
}

#[test]
fn an_uncounted_sequence_of_empty_elements_keeps_them_all() {
    // The reason for announcing each element: these occupy no bytes at all,
    // so nothing about where they end could reveal how many there are.
    survives(|| Uncounted::new(vec![(), (), ()].into_iter().filter(|_| true)), &vec![(), (), ()]);
}

#[test]
fn an_uncounted_map_survives() {
    let entries = vec![(1u32, "one".to_string()), (2, "two".to_string())];
    survives(
        || UncountedMap::new(entries.clone().into_iter().filter(|_| true)),
        &BTreeMap::from_iter(entries.clone()),
    );
}

#[test]
fn an_uncounted_map_of_empty_entries_keeps_them_all() {
    // A map cannot hold two `()` keys, so this is a sequence of pairs that
    // occupies no bytes, read back as the count it was written with.
    let value = UncountedMap::new(vec![((), ()), ((), ())].into_iter().filter(|_| true));

    let bytes = to_full_vec(&value).unwrap();
    assert_eq!(bytes, [0x7d, 0x00, 0x03, 0x01, 0x01, 0x00], "two announced entries, then the end");
}

#[test]
fn an_uncounted_sequence_that_turns_out_empty_survives() {
    // Filtering an empty vector still reports an exact size, so it takes the
    // counted path; filtering everything away out of a non-empty one does not.
    survives(|| Uncounted::new(vec![1u32, 2, 3].into_iter().filter(|_| false)), &Vec::<u32>::new());

    let bytes = to_full_vec(&Uncounted::new(vec![1u32].into_iter().filter(|_| false))).unwrap();
    assert_eq!(bytes, [0x7d, 0x00, 0x01, 0x00], "the end, and nothing before it");
}

// --------------------------------------------------------------------------
// Input that does not follow the rules
// --------------------------------------------------------------------------

#[test]
fn a_block_that_never_ends_is_refused() {
    // Before the announcements, this looped forever: the block still held a
    // byte, and an element consuming none of it never reached the end.
    let hostile = [0x7du8, 0x00, 0x01, 0xff];

    assert!(matches!(from_full_slice::<Vec<()>>(&hostile), Err(Error::BadLen)));
    assert!(matches!(from_full_slice::<BTreeMap<(), ()>>(&hostile), Err(Error::BadLen)));
}

#[test]
fn a_missing_end_is_refused() {
    // An announcement with nothing after it, and no end.
    let truncated = [0x7du8, 0x00, 0x01, 0x01];

    assert!(from_full_slice::<Vec<()>>(&truncated).is_err());
}

#[test]
fn an_unfinished_element_is_refused() {
    // The element is announced but its varint never terminates.
    let truncated = [0x7du8, 0x00, 0x03, 0x01, 0x80, 0x80];

    assert!(from_full_slice::<Vec<u32>>(&truncated).is_err());
}

#[test]
fn a_counted_sequence_is_unchanged() {
    // The announcements are only for sequences that arrive without a count.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Holder {
        #[serde(rename = "_0")]
        v: Vec<u32>,
    }

    for cfg in [full_cfg(), full_cfg().with_version(postbag::cfg::Version::Postbag0_4)] {
        let value = Holder { v: vec![1, 2, 3] };
        let bytes = postbag::to_vec(cfg, &value).unwrap();
        let back: Holder = postbag::from_slice(cfg, bytes.as_slice()).unwrap();

        assert_eq!(back, value);
        assert_eq!(bytes, b"\x01\x41\x04\x03\x01\x02\x03");
    }

    let _: Cfg<true> = Full::new();
}

// --------------------------------------------------------------------------
// What Postbag 0.4 wrote
// --------------------------------------------------------------------------

/// 0.4 announced nothing and ended the sequence where its block ran out.
const LEGACY: Cfg<true> = Full::new().with_header(false).with_version(postbag::cfg::Version::Postbag0_4);

#[test]
fn the_legacy_framing_is_what_0_4_wrote() {
    let bytes = postbag::to_vec(LEGACY, &Uncounted::new(vec![1u8, 0, 0].into_iter().filter(|_| true)));

    // Marker, block of three, and the three values with nothing between them.
    assert_eq!(bytes.unwrap(), [0x7d, 0x00, 0x03, 0x01, 0x00, 0x00]);
}

#[test]
fn what_0_4_wrote_is_read_back() {
    // Without the setting these same bytes read as `[0]`: the leading `01`
    // looks like an announcement, and the trailing `00` like an end.
    let old = [0x7du8, 0x00, 0x03, 0x01, 0x00, 0x00];

    assert_eq!(postbag::from_slice::<Vec<u8>, _>(LEGACY, old.as_slice()).unwrap(), vec![1, 0, 0]);
    assert_eq!(from_full_slice::<Vec<u8>>(&old).unwrap(), vec![0], "the reason the setting exists");
}

#[test]
fn the_legacy_framing_round_trips() {
    for expected in [vec![2u32, 4], vec![], vec![7]] {
        let value = Uncounted::new(expected.clone().into_iter().filter(|_| true));
        let bytes = postbag::to_vec(LEGACY, &value).unwrap();
        let back: Vec<u32> = postbag::from_slice(LEGACY, bytes.as_slice()).unwrap();

        assert_eq!(back, expected);
    }
}

#[test]
fn the_legacy_framing_ends_where_its_block_does() {
    // Not exercised, because it does not return: `[7d, 00, 01, ff]` read as a
    // `Vec<()>` leaves a byte in the block that no element consumes, so this
    // framing never reaches an end and reads the same element forever, as 0.4
    // did. It is why the announced framing exists, and why `Version::Postbag0_4` is
    // for reading data you have rather than input you were given.

    // An empty one is fine, and is what 0.4 wrote for a sequence of nothing.
    assert_eq!(
        postbag::from_slice::<Vec<u8>, _>(LEGACY, [0x7du8, 0x00, 0x00].as_slice()).unwrap(),
        Vec::<u8>::new()
    );
}

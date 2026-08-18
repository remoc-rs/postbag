//! Serialized representation of recoverable values.

use serde::{Deserialize, Serialize};

use postbag::{
    cfg::{Full, Slim},
    from_full_slice, from_slim_slice,
    recoverable::{Recover, Recoverable},
    to_full_vec, to_slim_vec, to_vec,
};

/// The tests stating exact bytes are about the framing of the value alone, so
/// they leave out the header a stream otherwise begins with.
fn bare_full<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    to_vec(Full::new().with_header(false), value).unwrap()
}

fn bare_slim<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    to_vec(Slim::new().with_header(false), value).unwrap()
}

#[derive(Default, Serialize, Deserialize)]
struct B {
    x: u32,
    y: String,
}

fn b() -> B {
    B { x: 42, y: "hello".into() }
}

/// A policy other than the default one, to check that it does not affect the
/// representation.
struct OtherPolicy;

impl Recover<B> for OtherPolicy {
    fn recover<E: serde::de::Error>(err: E) -> Result<B, E> {
        Err(err)
    }
}

#[derive(Serialize, Deserialize)]
struct Plain {
    a: u32,
    b: B,
    c: u16,
}

#[derive(Serialize, Deserialize)]
struct Wrapped {
    a: u32,
    b: Recoverable<B>,
    c: u16,
}

#[derive(Serialize)]
struct WrappedOtherPolicy {
    a: u32,
    b: Recoverable<B, OtherPolicy>,
    c: u16,
}

#[derive(Serialize)]
struct WithModule {
    a: u32,
    #[serde(with = "postbag::recoverable")]
    b: B,
    c: u16,
}

#[derive(Serialize)]
struct WithPolicy {
    a: u32,
    #[serde(with = "postbag::recoverable::With::<OtherPolicy>")]
    b: B,
    c: u16,
}

fn plain() -> Plain {
    Plain { a: 1, b: b(), c: 2 }
}

fn wrapped() -> Wrapped {
    Wrapped { a: 1, b: Recoverable::new(b()), c: 2 }
}

/// A field value is enclosed in a block of its own in `Full`, so wrapping it
/// changes nothing: an existing field can be made recoverable without breaking
/// compatibility with data already written.
#[test]
fn full_field_representation_is_unchanged() {
    assert_eq!(to_full_vec(&plain()).unwrap(), to_full_vec(&wrapped()).unwrap());
}

/// An enum variant payload is enclosed in a block just as a field value is,
/// whether the enum sits at the top level or inside a field, so wrapping one
/// leaves the representation unchanged as well.
#[test]
fn full_variant_payload_representation_is_unchanged() {
    #[derive(Serialize)]
    enum Plain {
        V(B),
    }

    #[derive(Serialize)]
    enum Wrapped {
        V(Recoverable<B>),
    }

    #[derive(Serialize)]
    struct HoldsPlain {
        e: Plain,
        tail: u8,
    }

    #[derive(Serialize)]
    struct HoldsWrapped {
        e: Wrapped,
        tail: u8,
    }

    assert_eq!(to_full_vec(&Plain::V(b())).unwrap(), to_full_vec(&Wrapped::V(Recoverable::new(b()))).unwrap());

    assert_eq!(
        to_full_vec(&HoldsPlain { e: Plain::V(b()), tail: 9 }).unwrap(),
        to_full_vec(&HoldsWrapped { e: Wrapped::V(Recoverable::new(b())), tail: 9 }).unwrap()
    );
}

/// What is written before a field is wrapped is read back after, and the other
/// way around, which is what makes the wrapper safe to add to a type in use.
#[test]
fn full_reads_across_the_change() {
    let plain = to_full_vec(&plain()).unwrap();
    let wrapped = to_full_vec(&wrapped()).unwrap();

    let read: Wrapped = from_full_slice(&plain).unwrap();
    assert_eq!(read.a, 1);
    assert_eq!(read.c, 2);
    assert!(!Recoverable::is_recovered(&read.b));

    let read: Plain = from_full_slice(&wrapped).unwrap();
    assert_eq!(read.a, 1);
    assert_eq!(read.c, 2);
}

/// `Slim` encloses nothing, so the block has to be added there.
#[test]
fn slim_field_gains_a_block() {
    let plain = to_slim_vec(&plain()).unwrap();
    let wrapped = to_slim_vec(&wrapped()).unwrap();

    assert_ne!(plain, wrapped);
    assert!(wrapped.len() > plain.len(), "the block states a length");
}

/// The wrapper and both `#[serde(with)]` forms go through the same code, so
/// a field can be switched between them.
#[test]
fn all_forms_agree() {
    for (name, bytes) in [
        ("other policy", to_full_vec(&WrappedOtherPolicy { a: 1, b: Recoverable::new(b()), c: 2 }).unwrap()),
        ("with module", to_full_vec(&WithModule { a: 1, b: b(), c: 2 }).unwrap()),
        ("with policy", to_full_vec(&WithPolicy { a: 1, b: b(), c: 2 }).unwrap()),
    ] {
        assert_eq!(to_full_vec(&wrapped()).unwrap(), bytes, "{name} differs in Full");
    }

    for (name, bytes) in [
        ("other policy", to_slim_vec(&WrappedOtherPolicy { a: 1, b: Recoverable::new(b()), c: 2 }).unwrap()),
        ("with module", to_slim_vec(&WithModule { a: 1, b: b(), c: 2 }).unwrap()),
        ("with policy", to_slim_vec(&WithPolicy { a: 1, b: b(), c: 2 }).unwrap()),
    ] {
        assert_eq!(to_slim_vec(&wrapped()).unwrap(), bytes, "{name} differs in Slim");
    }
}

/// Nothing encloses a sequence element, so it gains a block of its own in
/// `Full` as well.
#[test]
fn sequence_element_gains_a_block() {
    #[derive(Serialize)]
    struct PlainSeq {
        v: Vec<u32>,
    }

    #[derive(Serialize)]
    struct WrappedSeq {
        v: Vec<Recoverable<u32>>,
    }

    let plain = PlainSeq { v: vec![7, 8] };
    let wrapped = WrappedSeq { v: vec![Recoverable::new(7), Recoverable::new(8)] };

    // One field, identifier `v`, length of its value, two elements.
    assert_eq!(bare_full(&plain), [1, 1, b'v', 3, 2, 7, 8]);
    // Each element is preceded by the length of its block.
    assert_eq!(bare_full(&wrapped), [1, 1, b'v', 5, 2, 1, 7, 1, 8]);

    // One field, length of the struct body, two elements.
    assert_eq!(bare_slim(&plain), [1, 3, 2, 7, 8]);
    assert_eq!(bare_slim(&wrapped), [1, 5, 2, 1, 7, 1, 8]);
}

/// A value that owns a block may leave out its own length, but only in `Full`;
/// in `Slim` it states it as it would anywhere else.
#[test]
fn slim_nested_value_is_self_describing() {
    #[derive(Serialize)]
    struct Nested {
        v: Recoverable<Vec<u8>>,
    }

    // One field, length of the struct body, length of the block, length of the
    // vector, its elements.
    assert_eq!(bare_slim(&Nested { v: Recoverable::new(vec![1, 2, 3]) }), [1, 5, 4, 3, 1, 2, 3]);
}

/// A struct that owns the block added for it leaves out its field count, so
/// that the block replaces it rather than adding to it.
#[test]
fn full_nested_struct_drops_its_field_count() {
    #[derive(Serialize)]
    struct Small {
        x: u32,
    }

    #[derive(Serialize)]
    struct PlainSeq {
        v: Vec<Small>,
    }

    #[derive(Serialize)]
    struct WrappedSeq {
        v: Vec<Recoverable<Small>>,
    }

    // Element: field count, identifier `x`, length of its value, the value.
    assert_eq!(bare_full(&PlainSeq { v: vec![Small { x: 7 }] }), [1, 1, b'v', 6, 1, 1, 1, b'x', 1, 7]);
    // Element: length of its block, then the fields, without a count.
    assert_eq!(
        bare_full(&WrappedSeq { v: vec![Recoverable::new(Small { x: 7 })] }),
        [1, 1, b'v', 6, 1, 4, 1, b'x', 1, 7]
    );
}

/// Values that are not recoverable are unaffected by the name check.
#[test]
fn plain_newtype_struct_is_transparent() {
    #[derive(Serialize)]
    struct Newtype(B);

    assert_eq!(to_full_vec(&Newtype(b())).unwrap(), to_full_vec(&b()).unwrap());
    assert_eq!(to_slim_vec(&Newtype(b())).unwrap(), to_slim_vec(&b()).unwrap());
}

/// A variant that a later version of the type no longer knows, which is a
/// change that breaks forward compatibility.
#[derive(Serialize)]
enum KindOld {
    A,
    C,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
enum KindNew {
    #[default]
    A,
}

/// The value that changed incompatibly, holding the changed type behind a
/// field of its own, so that deserializing it fails with blocks left open.
#[derive(Serialize)]
struct InnerOld {
    k: KindOld,
    n: u32,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct InnerNew {
    k: KindNew,
    n: u32,
}

#[derive(Serialize)]
struct OuterOld {
    a: u32,
    b: InnerOld,
    c: u16,
}

#[derive(Debug, Deserialize)]
struct OuterNew {
    a: u32,
    b: Recoverable<InnerNew>,
    c: u16,
}

fn outer_old(k: KindOld) -> OuterOld {
    OuterOld { a: 1, b: InnerOld { k, n: 7 }, c: 2 }
}

/// The whole point: the fields beside the one that changed still deserialize.
#[test]
fn full_recovers_from_an_incompatible_change() {
    let bytes = to_full_vec(&outer_old(KindOld::C)).unwrap();
    let outer: OuterNew = from_full_slice(&bytes).unwrap();

    assert_eq!(outer.a, 1);
    assert_eq!(outer.c, 2);
    assert_eq!(*outer.b, InnerNew::default());
    assert!(Recoverable::is_recovered(&outer.b));
}

/// Nothing is recovered from where nothing failed.
#[test]
fn full_leaves_a_value_it_can_read_alone() {
    let bytes = to_full_vec(&outer_old(KindOld::A)).unwrap();
    let outer: OuterNew = from_full_slice(&bytes).unwrap();

    assert_eq!(outer.a, 1);
    assert_eq!(outer.c, 2);
    assert_eq!(*outer.b, InnerNew { k: KindNew::A, n: 7 });
    assert!(!Recoverable::is_recovered(&outer.b));
}

/// `Slim` has to be written with the wrapper in place, since it adds a block.
#[test]
fn slim_recovers_from_an_incompatible_change() {
    #[derive(Serialize)]
    struct SlimOld {
        a: u32,
        b: Recoverable<InnerOld>,
        c: u16,
    }

    let bytes =
        to_slim_vec(&SlimOld { a: 1, b: Recoverable::new(InnerOld { k: KindOld::C, n: 7 }), c: 2 }).unwrap();
    let outer: OuterNew = from_slim_slice(&bytes).unwrap();

    assert_eq!(outer.a, 1);
    assert_eq!(outer.c, 2);
    assert_eq!(*outer.b, InnerNew::default());
    assert!(Recoverable::is_recovered(&outer.b));
}

/// Every element is bounded on its own, so the ones that can be read are.
#[test]
fn only_the_failing_sequence_element_is_replaced() {
    #[derive(Serialize)]
    struct SeqOld {
        v: Vec<Recoverable<KindOld>>,
        after: String,
    }

    #[derive(Deserialize)]
    struct SeqNew {
        v: Vec<Recoverable<KindNew>>,
        after: String,
    }

    for cfg in ["full", "slim"] {
        let old = SeqOld {
            v: vec![KindOld::A, KindOld::C, KindOld::A].into_iter().map(Recoverable::new).collect(),
            after: "still here".into(),
        };

        let new: SeqNew = if cfg == "full" {
            from_full_slice(&to_full_vec(&old).unwrap()).unwrap()
        } else {
            from_slim_slice(&to_slim_vec(&old).unwrap()).unwrap()
        };

        assert_eq!(new.v.len(), 3, "{cfg}");
        assert!(!Recoverable::is_recovered(&new.v[0]), "{cfg}");
        assert!(Recoverable::is_recovered(&new.v[1]), "{cfg}");
        assert!(!Recoverable::is_recovered(&new.v[2]), "{cfg}");
        assert_eq!(new.after, "still here", "{cfg}");
    }
}

/// The `#[serde(with)]` form recovers just as the wrapper does.
#[test]
fn with_module_recovers() {
    #[derive(Debug, Deserialize)]
    struct WithNew {
        a: u32,
        #[serde(with = "postbag::recoverable")]
        b: InnerNew,
        c: u16,
    }

    let bytes = to_full_vec(&outer_old(KindOld::C)).unwrap();
    let outer: WithNew = from_full_slice(&bytes).unwrap();

    assert_eq!(outer.a, 1);
    assert_eq!(outer.c, 2);
    assert_eq!(outer.b, InnerNew::default());
}

/// A policy may decline to recover, which propagates the error as it would
/// without the wrapper.
#[test]
fn a_policy_can_propagate_the_error() {
    struct Propagate;

    impl Recover<InnerNew> for Propagate {
        fn recover<E: serde::de::Error>(err: E) -> Result<InnerNew, E> {
            Err(err)
        }
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Declining {
        a: u32,
        #[serde(with = "postbag::recoverable::With::<Propagate>")]
        b: InnerNew,
        c: u16,
    }

    let bytes = to_full_vec(&outer_old(KindOld::C)).unwrap();
    let err = from_full_slice::<Declining>(&bytes).unwrap_err();

    assert!(matches!(err, postbag::Error::Custom(_)), "{err:?}");
}

/// An input that broke off mid-value gives no place to resume from, so it is
/// never recovered from, however willing the policy is.
#[test]
fn a_truncated_input_is_not_recovered_from() {
    let bytes = to_full_vec(&outer_old(KindOld::A)).unwrap();

    // Cutting anywhere inside the value must fail rather than yield a default.
    for len in 1..bytes.len() {
        let err = from_full_slice::<OuterNew>(&bytes[..len]).unwrap_err();
        assert!(
            matches!(err, postbag::Error::Io(_) | postbag::Error::EndOfBlock),
            "truncated to {len} bytes gave {err:?}"
        );
    }
}

/// A reader that fails once part way through a read and then carries on, as a
/// reader over a network connection may.
///
/// The failed read has taken the bytes it delivered with it, so how much of
/// the open blocks is left no longer matches the input, even though reading
/// continues to succeed.
struct FailsOnce {
    data: Vec<u8>,
    pos: usize,
    fail_at: usize,
    failed: bool,
}

impl std::io::Read for FailsOnce {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.failed && self.pos >= self.fail_at {
            self.failed = true;
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "reader failed once"));
        }

        // One byte at a time, so that a failure lands in the middle of a read
        // of several bytes rather than before it.
        let n = buf.len().min(1).min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// A reader that failed leaves no place to resume from, even where it carries
/// on afterwards and the blocks could seemingly be skipped over.
#[test]
fn a_failing_reader_is_not_recovered_from() {
    #[derive(Serialize)]
    struct PayloadOld {
        data: Vec<u8>,
    }

    #[derive(Debug, Default, Deserialize)]
    struct PayloadNew {
        #[allow(dead_code)]
        data: Vec<u8>,
    }

    #[derive(Serialize)]
    struct HolderOld {
        payload: Recoverable<PayloadOld>,
        tail: u32,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct HolderNew {
        payload: Recoverable<PayloadNew>,
        tail: u32,
    }

    let bytes =
        to_full_vec(&HolderOld { payload: Recoverable::new(PayloadOld { data: vec![9; 16] }), tail: 1234 })
            .unwrap();

    // Fail somewhere inside the payload, after enough of it has been read for
    // the failure to land in the middle of a multi-byte read.
    for fail_at in 4..bytes.len() - 2 {
        let read = FailsOnce { data: bytes.clone(), pos: 0, fail_at, failed: false };
        let res = postbag::deserialize::<_, HolderNew, true>(postbag::cfg::Full::new(), read);

        assert!(res.is_err(), "failing at byte {fail_at} yielded {res:?} instead of an error");
    }
}

/// Recovery of an inner value leaves the outer one able to continue.
#[test]
fn recovery_nests() {
    #[derive(Serialize)]
    struct NestedOld {
        inner: Recoverable<InnerOld>,
        n: u32,
    }

    #[derive(Debug, Default, Deserialize)]
    struct NestedNew {
        inner: Recoverable<InnerNew>,
        n: u32,
    }

    #[derive(Serialize)]
    struct HolderOld {
        nested: Recoverable<NestedOld>,
        tail: String,
    }

    #[derive(Deserialize)]
    struct HolderNew {
        nested: Recoverable<NestedNew>,
        tail: String,
    }

    let old = HolderOld {
        nested: Recoverable::new(NestedOld { inner: Recoverable::new(InnerOld { k: KindOld::C, n: 7 }), n: 9 }),
        tail: "tail".into(),
    };

    let new: HolderNew = from_full_slice(&to_full_vec(&old).unwrap()).unwrap();

    // Only the innermost value failed, so everything around it survived.
    assert!(!Recoverable::is_recovered(&new.nested));
    assert!(Recoverable::is_recovered(&new.nested.inner));
    assert_eq!(new.nested.n, 9);
    assert_eq!(new.tail, "tail");
}

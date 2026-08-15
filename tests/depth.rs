//! Tests for the nesting depth limit.

use std::fmt::Debug;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use postbag::{
    Error,
    cfg::{Cfg, DEFAULT_DEPTH_LIMIT, Full, Slim},
    from_full_slice, from_slice, from_slim_slice, to_full_vec, to_slim_vec, to_vec,
};

/// Recursive type nested via an enum variant.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Tree {
    Leaf,
    Node(Box<Tree>),
}

impl Tree {
    fn nested(depth: usize) -> Self {
        let mut tree = Tree::Leaf;
        for _ in 0..depth {
            tree = Tree::Node(Box::new(tree));
        }
        tree
    }
}

/// Recursive type nested via `Option`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct List {
    next: Option<Box<List>>,
}

impl List {
    fn nested(depth: usize) -> Self {
        let mut list = List { next: None };
        for _ in 0..depth {
            list = List { next: Some(Box::new(list)) };
        }
        list
    }
}

/// Hostile input: a long run of `Node` variant tags in slim encoding.
fn hostile_slim(depth: usize) -> Vec<u8> {
    let mut bytes = vec![1u8; depth];
    bytes.push(0);
    bytes
}

/// The length of a block whose contents fit in a single chunk.
fn block_len(len: usize) -> Vec<u8> {
    assert!(len < u16::MAX as usize, "a longer block would need continuation chunks");

    let mut out = Vec::new();
    let mut value = len as u16;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// Hostile input: `Node` variants nested far deeper than the limit allows, in
/// full encoding.
///
/// Only the outermost variant carries a block. Every level below it reaches the
/// end of that same block, so it states no length of its own and the nest stays
/// a flat run of tags inside a single block.
fn hostile_full(depth: usize) -> Vec<u8> {
    assert!(depth > 0, "a tree of no depth is just a leaf");

    let mut bytes = Vec::new();
    bytes.push(0x80 | 4); // `Node`, tagged as carrying a payload block
    bytes.extend_from_slice(b"Node");
    bytes.extend_from_slice(&block_len(5 * depth));

    for _ in 0..depth - 1 {
        bytes.push(4); // `Node`, reaching the end of the block above
        bytes.extend_from_slice(b"Node");
    }
    bytes.push(4); // `Leaf`, which carries nothing
    bytes.extend_from_slice(b"Leaf");

    bytes
}

#[test]
fn the_hostile_input_is_what_the_writer_writes() {
    // Without this the test below could pass on input the reader turns down for
    // a reason that has nothing to do with the depth limit.
    for depth in 1..8 {
        assert_eq!(hostile_full(depth), to_full_vec(&Tree::nested(depth)).unwrap(), "at depth {depth}");
    }
}

#[test]
fn default_depth_limit_is_reachable_from_config() {
    assert_eq!(Full::DEFAULT_DEPTH_LIMIT, DEFAULT_DEPTH_LIMIT);
    assert_eq!(Slim::DEFAULT_DEPTH_LIMIT, DEFAULT_DEPTH_LIMIT);
    assert_eq!(Full::new().depth_limit(), DEFAULT_DEPTH_LIMIT);
    assert_eq!(Slim::new().depth_limit(), DEFAULT_DEPTH_LIMIT);
}

#[test]
fn deeply_nested_input_is_rejected_slim() {
    let res: Result<Tree, _> = from_slim_slice(&hostile_slim(1_000_000));
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");
}

#[test]
fn deeply_nested_input_is_rejected_full() {
    // Far beyond the limit, and as deep as one chunk per level can express.
    let res: Result<Tree, _> = from_full_slice(&hostile_full(4_000));
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");
}

#[test]
fn nesting_within_limit_is_accepted() {
    // Each level of `Tree` costs one enum level plus one newtype variant level,
    // so stay well within the default limit.
    let tree = Tree::nested(DEFAULT_DEPTH_LIMIT / 4);

    let bytes = to_slim_vec(&tree).unwrap();
    let restored: Tree = from_slim_slice(&bytes).unwrap();
    assert_eq!(tree, restored);

    let bytes = to_full_vec(&tree).unwrap();
    let restored: Tree = from_full_slice(&bytes).unwrap();
    assert_eq!(tree, restored);
}

#[test]
fn option_nesting_is_limited() {
    let list = List::nested(1000);

    let bytes = to_vec(Slim::new().with_depth_limit(usize::MAX), &list).unwrap();
    let res: Result<List, _> = from_slim_slice(&bytes);
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");

    let bytes = to_vec(Full::new().with_depth_limit(usize::MAX), &list).unwrap();
    let res: Result<List, _> = from_full_slice(&bytes);
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");
}

#[test]
fn seq_nesting_is_limited() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Nested(Vec<Nested>);

    let mut nested = Nested(Vec::new());
    for _ in 0..1000 {
        nested = Nested(vec![nested]);
    }

    let bytes = to_vec(Slim::new().with_depth_limit(usize::MAX), &nested).unwrap();
    let res: Result<Nested, _> = from_slim_slice(&bytes);
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");
}

#[test]
fn raised_limit_allows_deeper_nesting() {
    let depth = DEFAULT_DEPTH_LIMIT * 4;
    let tree = Tree::nested(depth);
    let bytes = to_vec(Slim::new().with_depth_limit(usize::MAX), &tree).unwrap();

    // Rejected with the default limit.
    let res: Result<Tree, _> = from_slim_slice(&bytes);
    assert!(matches!(res, Err(Error::RecursionLimit)));

    // Accepted with a raised limit.
    let restored: Tree = from_slice(Slim::new().with_depth_limit(depth * 4), &bytes).unwrap();
    assert_eq!(tree, restored);
}

#[test]
fn lowered_limit_is_enforced() {
    let tree = Tree::nested(8);
    let bytes = to_full_vec(&tree).unwrap();

    let res: Result<Tree, _> = from_slice(Full::new().with_depth_limit(4), &bytes);
    assert!(matches!(res, Err(Error::RecursionLimit)), "expected recursion limit error, got {res:?}");
}

#[test]
fn limit_is_not_consumed_by_sibling_fields() {
    // Depth is about nesting, not about the total number of values: a struct
    // with many non-nested fields must not exhaust the budget.
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Wide {
        a: Vec<u32>,
        b: Vec<u32>,
        c: Vec<u32>,
        d: Vec<u32>,
    }

    let wide = Wide { a: vec![1; 100], b: vec![2; 100], c: vec![3; 100], d: vec![4; 100] };

    let bytes = to_full_vec(&wide).unwrap();
    let restored: Wide = from_slice(Full::new().with_depth_limit(8), &bytes).unwrap();
    assert_eq!(wide, restored);

    let bytes = to_slim_vec(&wide).unwrap();
    let restored: Wide = from_slice(Slim::new().with_depth_limit(8), &bytes).unwrap();
    assert_eq!(wide, restored);
}

#[test]
fn serialization_and_deserialization_limits_agree() {
    // A value that can be serialized with a given limit must also be
    // deserializable with the same limit, otherwise round-tripping a value
    // would break at the limit boundary.
    for limit in [2, 3, 4, 8, 16, 32, 128] {
        for depth in 0..limit * 2 {
            let tree = Tree::nested(depth);

            let slim = Slim::new().with_depth_limit(limit);
            if let Ok(bytes) = to_vec(slim, &tree) {
                let restored: Tree = from_slice(slim, &bytes).unwrap_or_else(|err| {
                    panic!("slim: depth {depth} serialized with limit {limit} but failed to deserialize: {err}")
                });
                assert_eq!(tree, restored);
            }

            let full = Full::new().with_depth_limit(limit);
            if let Ok(bytes) = to_vec(full, &tree) {
                let restored: Tree = from_slice(full, &bytes).unwrap_or_else(|err| {
                    panic!("full: depth {depth} serialized with limit {limit} but failed to deserialize: {err}")
                });
                assert_eq!(tree, restored);
            }
        }
    }
}

/// Anything that can be written with a given limit must be readable with the
/// same limit; otherwise the codec would produce data it cannot take back.
#[track_caller]
fn writable_is_readable<T, const WITH_IDENTS: bool>(what: &str, value: &T, cfg: Cfg<WITH_IDENTS>)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    let Ok(bytes) = to_vec(cfg, value) else { return };

    let restored: T = from_slice(cfg, &bytes)
        .unwrap_or_else(|err| panic!("{what}: written with {cfg:?} but not readable: {err}"));

    assert_eq!(*value, restored, "{what}");
}

#[track_caller]
fn limits_agree<T>(what: &str, value: &T)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    for limit in 0..=6 {
        writable_is_readable(what, value, Full::new().with_depth_limit(limit));
        writable_is_readable(what, value, Slim::new().with_depth_limit(limit));
    }
}

#[test]
fn limits_agree_where_the_two_sides_charge_differently() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Empty {}

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Wrapper {
        inner: Empty,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum Choice {
        Unit,
        Newtype(u8),
        Struct { a: u8 },
    }

    limits_agree("bare scalar", &7u8);
    limits_agree("empty struct", &Empty {});
    limits_agree("struct holding an empty struct", &Wrapper { inner: Empty {} });
    limits_agree("unit variant", &Choice::Unit);
    limits_agree("newtype variant", &Choice::Newtype(7));
    limits_agree("struct variant", &Choice::Struct { a: 7 });
    limits_agree("string", &"x".to_string());
    limits_agree("empty vec", &Vec::<u8>::new());
    limits_agree("nested vec", &vec![vec![1u8, 2]]);
    limits_agree("nested option", &Some(Some(1u8)));
    limits_agree("tuple", &(1u8, "x".to_string()));
}

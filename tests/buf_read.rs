use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt::Debug, io::BufReader};

use postbag::{
    Error,
    cfg::{Cfg, Full, Slim},
    deserialize, deserialize_buf_read, serialize,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Outer {
    a: u32,
    inner: Inner,
    tail: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Inner {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    list: Vec<Item>,
    opt: Option<Box<Inner>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Item {
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
    Struct {
        name: String,
        #[serde(with = "serde_bytes")]
        b: Vec<u8>,
    },
    Unit,
}

fn bytes(n: usize, seed: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

fn value(n: usize) -> Outer {
    Outer {
        a: n as u32,
        inner: Inner {
            data: bytes(n, 1),
            list: vec![
                Item::Bytes(bytes(n / 2, 2)),
                Item::Struct { name: "x".repeat(n % 300), b: bytes(n / 3, 3) },
                Item::Unit,
            ],
            opt: Some(Box::new(Inner { data: bytes(7, 4), list: vec![], opt: None })),
        },
        tail: "z".repeat(n % 1000),
    }
}

/// Deserializes from buffered readers of several buffer sizes, with and without
/// the length of the input, and checks that the value matches the one read from
/// a plain reader.
#[track_caller]
fn check<T, const WITH_IDENTS: bool>(value: &T, cfg: Cfg<WITH_IDENTS>)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    let mut data = Vec::new();
    serialize(cfg, &mut data, value).unwrap();

    let plain: T = deserialize(cfg, data.as_slice()).unwrap();
    assert_eq!(&plain, value);

    for capacity in [1, 7, 4_096, 1 << 20] {
        for len in [Some(data.len()), Some(data.len() + 10), None] {
            let reader = BufReader::with_capacity(capacity, data.as_slice());
            let read: T = deserialize_buf_read(cfg, reader, len).unwrap();
            assert_eq!(&read, value, "buffer capacity {capacity}, length {len:?}");
        }
    }
}

#[test]
fn values_of_all_sizes_arrive_intact() {
    for n in [0, 1, 100, 65_534, 65_535, 65_536, 131_070, 131_071, 200_000, 1 << 20] {
        println!("n = {n}");
        check(&value(n), Full::new());
        check(&value(n), Slim::new());
    }
}

#[test]
fn reading_beyond_the_length_fails() {
    let value = value(100_000);
    let data = postbag::to_full_vec(&value).unwrap();

    for len in [0, 1, 1_000, data.len() / 2, data.len() - 1] {
        let reader = BufReader::new(data.as_slice());
        let result: Result<Outer, Error> = deserialize_buf_read(Full::new(), reader, Some(len));
        assert!(result.is_err(), "{len} of {} bytes were enough", data.len());
    }
}

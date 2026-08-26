# Postbag 💼

[![Crates.io](https://img.shields.io/crates/v/postbag.svg)](https://crates.io/crates/postbag)
[![Documentation](https://docs.rs/postbag/badge.svg)](https://docs.rs/postbag)

Postbag is a compact binary [serde] codec for Rust that keeps the Rust type system
fully intact and has support for backwards and forwards compatibility built in.
This is also known as schema evolution: data written with one version of your types
remains readable with another.

## Quick start

Normally you will want to use `to_full_vec` and `from_full_slice`:

```rust
use serde::{Serialize, Deserialize};
use postbag::{to_full_vec, from_full_slice};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Person {
    name: String,
    age: u32,
}

let person = Person { name: "Alice".to_string(), age: 30 };

let bytes = to_full_vec(&person)?;
let restored: Person = from_full_slice(&bytes)?;

assert_eq!(person, restored);
# Ok::<(), postbag::Error>(())
```

## Two data format variants

Postbag `Full` writes each field with its, [optionally numbered](#numbered-identifiers),
identifier and the length of its value, which is what lets fields be added, removed and reordered.

Postbag `Slim` writes the values and nothing else, in declaration order, which is smaller
but only allows add struct fields and enum variants at the end.

Both variants use variable-length integer encoding to save space.

Start with `Full` and use `Slim` when you need minimal size and can accept less compatibility.

The wire formats are specified separately:

- [Postbag Full format 1.0](https://github.com/remoc-rs/postbag/blob/main/POSTBAG-FULL.md)
- [Postbag Slim format 1.0](https://github.com/remoc-rs/postbag/blob/main/POSTBAG-SLIM.md)

## Backwards and forwards compatibility

As usual a field a reader expects but does not receive takes its `#[serde(default)]`, and
a variant it does not know needs a `#[serde(other)]` fallback.

The following changes to your types are supported:

| Change to your types | `Full` | `Slim` |
| --- | --- | --- |
| **Structs** | | |
| Add a field | anywhere | at the end |
| Remove a field | anywhere | at the end |
| Rename a field | when numbered | always |
| Reorder fields | yes | no |
| **Enums** | | |
| Add a variant | anywhere | at the end |
| Remove a variant | anywhere | at the end |
| Rename a variant | when numbered | always |
| Reorder variants | yes | no |
| **Size** | small | even smaller |


## Recoverable values

When a value fails to deserialize, the error normally aborts the whole deserialization.
An incompatible change to one type thus renders every enclosing value undecodable as well.

When using Postbag `Full`, a field annotated with `#[serde(with = "postbag::recoverable")]` 
confines a deserialization failure to it. The rest is deserialized as usual and the value 
is replaced by its `Default`.

```rust
use serde::{Serialize, Deserialize};

# #[derive(Default, Serialize, Deserialize)]
# struct Details { size: u32 }
#[derive(Serialize, Deserialize)]
struct Data {
    name: String,
    #[serde(with = "postbag::recoverable")]
    details: Details,
    count: u16,
}
```

Should `Details` change incompatibly, `name` and `count` still deserialize correctly
and `details` becomes `Details::default()`.

## Numbered identifiers

A struct field or enum variant renamed to `_0` through `_59` is encoded as a
single byte instead of its name.
This allows significant space savings in `Full` mode, while still providing full
backwards and forwards compatibility.

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct CompactData {
    #[serde(rename = "_3")]
    my_field: u32,
    #[serde(rename = "_15")]
    another_field: String,
    // Regular field names work normally.
    normal_field: bool,
}

#[derive(Serialize, Deserialize)]
enum CompactEnum {
    #[serde(rename = "_0")]
    MyLongVariantName,
    #[serde(rename = "_1")]
    AnotherLongVariantName(u32),
    #[serde(rename = "_2")]
    YetAnotherVariant {
        // Fields of struct variants can be numbered as well.
        #[serde(rename = "_0")]
        my_field: u32,
    },
    // Regular variant names work normally.
    NormalVariant,
}
```

Numbering is optional and can be mixed with names in the same type. A name that
is not of the form `_n` is written out as a string.

The identifier is what a reader matches on, so **changing the id of a field or
variant is a breaking change**, and an id that has been retired must never be
given to a different field or variant.

The [`compact`] module provides smaller representations of common standard library
types, which would otherwise spell out their field and variant names.

## Unsupported serde attributes

As a binary format Postbag cannot be used with serde's `untagged`, internally tagged
and `flatten` attributes.

## Nesting depth limit

Serialization and deserialization of nested data is recursive, so deeply nested
data consumes stack space. To prevent untrusted input from aborting the process
by overflowing the stack, the nesting depth is limited to
`cfg::DEFAULT_DEPTH_LIMIT` (128) and exceeding it fails with
`Error::RecursionLimit`.

This only becomes relevant for recursive types, since the nesting depth of a
non-recursive type is bounded by the type itself. Unknown fields are skipped by
length rather than parsed, so unknown data cannot cause recursion.

## Fast compile mode (for development use)

Postbag supports an optional fast compile mode that reduces compilation time at
the cost of buffering struct field data in memory during deserialization,
instead of streaming it directly from the reader.

Enable it by setting the `postbag_fast_compile` cfg flag:

```sh
RUSTFLAGS="--cfg postbag_fast_compile" cargo build
```

Or add it to your `.cargo/config.toml` for development:

```toml
[build]
rustflags = ["--cfg", "postbag_fast_compile"]
```

This flag is intended **for development use only**; production builds must not use it.

**Limitation**: in fast compile mode, fields are read positionally, so adding or
removing a struct field anywhere but at the end is not supported.
Adding and removing fields at the end continues to work.
Serialization is unaffected, so an endpoint built with this flag
interoperates with one built without it as long as both use the same types.

## Origins

Postbag started as a fork of [postcard](https://github.com/jamesmunns/postcard) with the intent to add forward and backward compatibility to the serialized data format. While postcard provides excellent performance and compact encoding, postbag extends this foundation to support schema evolution and data format compatibility across different versions of your applications.

## License

Postbag is licensed under the [Apache 2.0 license].

[Apache 2.0 license]: https://github.com/remoc-rs/postbag/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Postbag by you, shall be licensed as Apache 2.0, without any
additional terms or conditions.

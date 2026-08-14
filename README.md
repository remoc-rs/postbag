# Postbag 💼

[![Crates.io](https://img.shields.io/crates/v/postbag.svg)](https://crates.io/crates/postbag)
[![Documentation](https://docs.rs/postbag/badge.svg)](https://docs.rs/postbag)

Postbag is a high-performance binary [serde] codec for Rust that provides efficient data encoding with configurable levels of forward and backward compatibility.

[serde]: https://serde.rs

## Key Features

- **Full fidelity of Rust type system**: Structs, enums, tuples, arrays, maps and all primitive types keep their shape; `Some(None)` stays distinct from `None`, and 128-bit integers stay integers.
- **Efficient binary format**: Uses variable-length encoding (varint) for integers, compact representations for common types, and minimal overhead
- **Configurable compatibility**: Choose between space-efficient encoding (`Slim`) or forward/backward compatible encoding (`Full`) with field identifiers

### Limitations
Like every non-self-describing format, Postbag cannot serve serde's `untagged`, internally tagged and `flatten` attributes, which need to inspect a value to decide how to read it.

## Quick Start

```rust
use serde::{Serialize, Deserialize};
use postbag::{to_full_vec, from_full_slice};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Person {
    name: String,
    age: u32,
}

let original = Person {
    name: "Alice".to_string(),
    age: 30,
};

// Serialize to a byte vector using Full configuration
let bytes = to_full_vec(&original).unwrap();

// Deserialize back to the original type
let deserialized: Person = from_full_slice(&bytes).unwrap();
assert_eq!(original, deserialized);
```

## Encoding Configurations

Postbag provides two configurations: `Full` and `Slim`.
Use the convenience functions `to_full_vec`, `from_full_slice`, `to_slim_vec` and
`from_slim_slice`, or pass a configuration value to `to_vec`, `from_slice`,
`serialize` and `deserialize`:

```rust
# use serde::{Serialize, Deserialize};
# #[derive(Serialize, Deserialize, Debug, PartialEq)]
# struct Person { name: String, age: u32 }
# let person = Person { name: "Alice".to_string(), age: 30 };
use postbag::{cfg::Slim, to_vec, from_slice};

let bytes = to_vec(Slim::new(), &person).unwrap();
let deserialized: Person = from_slice(Slim::new(), &bytes).unwrap();
assert_eq!(person, deserialized);
```

The configuration also carries the [nesting depth limit](#nesting-depth-limit).

### `Full` Configuration

The `Full` configuration provides maximum compatibility and schema evolution capabilities:

- **Forward/backward compatibility**: Fields and enum variants can be reordered, added, or removed
- **Schema evolution**: Safe evolution of data structures over time
- **Widening `char` to `String` and vice versa**: the two encode identically, and a peer that still expects a `char` reads the first character instead of failing
- **Numerical identifier encoding**: Struct fields and enum variants named `_0` through `_59` are encoded with just a single byte

#### Numerical Identifier Encoding

When using `Full` configuration, struct fields and enum variants named `_n` (where `n` is 0-59) are encoded using just a single byte instead of the full string. Use `#[serde(rename = "...")]` to specify the numerical id for each field or variant.
This can significantly reduce serialized size for structs with many fields and enums with long variant names:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct CompactData {
    #[serde(rename = "_3")]
    my_field: u32,
    #[serde(rename = "_15")]
    another_field: String,
    // Regular field names work normally
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
        // Fields of struct variants can be numbered as well
        #[serde(rename = "_0")]
        my_field: u32,
    },
    // Regular variant names work normally
    NormalVariant,
}
```

This feature is entirely optional; regular field and variant names continue to work as expected. Normal and numerical names can be mixed without limitations within a single struct or enum.

Names that do not have the form `_n`, as well as ids of 60 and above, are encoded as regular strings.
Since the identifier determines compatibility, changing the id of a field or variant is a breaking change, but fields and variants can be reordered freely.
An id that has been retired should never be given to a different field or variant. 

In addition, the [`compact`](https://docs.rs/postbag/latest/postbag/compact/) module provides more
efficient representations of common types from the standard library.

### `Slim` Configuration

The `Slim` configuration prioritizes performance and compact size:

- **Compact encoding**: Smaller serialized data size
- **Fast processing**: No string lookups during serialization/deserialization  
- **Limited schema evolution**: Fields/variants can only be added/removed at the end

**Supported changes** when using the `Slim` configuration:
- Adding fields to the end of structs (with serde defaults for deserialization)
- Removing fields from the end of structs (with serde defaults for deserialization)
- Adding enum variants at the end
- Removing enum variants from the end

**Important**: Fields and enum variants must maintain their order for compatibility when using `Slim` configuration.

## Experimental Fast Compile Mode (for development use)

Postbag supports an optional fast compile mode that reduces compilation time at the cost of buffering struct field data in memory during deserialization (instead of streaming it directly from the reader).

Enable it by setting the `postbag_fast_compile` cfg flag:

```sh
RUSTFLAGS="--cfg postbag_fast_compile" cargo build
```

Or add it to your `.cargo/config.toml` for development:

```toml
[build]
rustflags = ["--cfg", "postbag_fast_compile"]
```

This flag is intended for development use only. Production builds should use the default streaming mode.

**Limitation**: Forward/backward compatibility for adding or removing struct fields in the middle (i.e. not at the end) is not supported in fast compile mode. Adding or removing fields at the end of structs continues to work.

## Nesting Depth Limit

Serialization and deserialization of nested data is recursive, so deeply nested
data consumes stack space. To prevent untrusted input from aborting the process
by overflowing the stack, the nesting depth is limited to `cfg::DEFAULT_DEPTH_LIMIT`
(128) and exceeding it fails with `Error::RecursionLimit`.

This only becomes relevant for recursive types, since the nesting depth of
non-recursive types is bounded by the type itself. Unknown fields are skipped
by length, thus unknown data cannot cause recursion.

The limit is part of the configuration:

```rust
# use serde::{Serialize, Deserialize};
# #[derive(Serialize, Deserialize)]
# struct MyType { value: u32 }
# let my_value = MyType { value: 1 };
use postbag::{cfg::Full, to_vec, from_slice};

let cfg = Full::new().with_depth_limit(1024);
let bytes = to_vec(cfg, &my_value).unwrap();
let value: MyType = from_slice(cfg, &bytes).unwrap();
```

Raise it for legitimately deeply nested data, or lower it when deserializing
untrusted input on threads with a small stack.

## Origins

Postbag started as a fork of [postcard](https://github.com/jamesmunns/postcard) with the intent to add forward and backward compatibility to the serialized data format. While postcard provides excellent performance and compact encoding, postbag extends this foundation to support schema evolution and data format compatibility across different versions of your applications.

## License

Postbag is licensed under the [Apache 2.0 license].

[Apache 2.0 license]: https://github.com/remoc-rs/postbag/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Postbag by you, shall be licensed as Apache 2.0, without any
additional terms or conditions.

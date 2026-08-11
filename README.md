# Postbag 💼

[![Crates.io](https://img.shields.io/crates/v/postbag.svg)](https://crates.io/crates/postbag)
[![Documentation](https://docs.rs/postbag/badge.svg)](https://docs.rs/postbag)

Postbag is a high-performance binary [serde] codec for Rust that provides efficient data encoding with configurable levels of forward and backward compatibility.

[serde]: https://serde.rs

## Key Features

- **Full fidelity of Rust type system**: Supports all serde-compatible types including structs, enums, tuples, arrays, maps, and all primitive types
- **Efficient binary format**: Uses variable-length encoding (varint) for integers, compact representations for common types, and minimal overhead
- **Configurable compatibility**: Choose between space-efficient encoding (`Slim`) or forward/backward compatible encoding (`Full`) with field identifiers

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

### `Full` Configuration

The `Full` configuration provides maximum compatibility and schema evolution capabilities:

- **Forward/backward compatibility**: Fields and enum variants can be reordered, added, or removed
- **Schema evolution**: Safe evolution of data structures over time
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
Since the identifier determines compatibility, changing the id of a field or variant is a breaking change,
but fields and variants can be reordered freely.

In addition, the [`compact`](https://docs.rs/postbag/latest/postbag/compact/) module provides more
efficient representations of common types from the standard library.

### `Slim` Configuration

The `Slim` configuration prioritizes performance and compact size:

- **Compact encoding**: Smaller serialized data size
- **Fast processing**: No string lookups during serialization/deserialization  
- **Limited schema evolution**: Fields/variants can only be added/removed at the end

**Supported changes** when using them `Slim` configuration:
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

## Origins

Postbag started as a fork of [postcard](https://github.com/jamesmunns/postcard) with the intent to add forward and backward compatibility to the serialized data format. While postcard provides excellent performance and compact encoding, postbag extends this foundation to support schema evolution and data format compatibility across different versions of your applications.

## License

Postbag is licensed under the [Apache 2.0 license].

[Apache 2.0 license]: https://github.com/surban/postbag/blob/master/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Postbag by you, shall be licensed as Apache 2.0, without any
additional terms or conditions.

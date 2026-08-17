# Changelog

## 1.0.0

This release includes some serialization format changes. 
To be compatible with Postbag 0.4 use `cfg::Version::Postbag0_4`.

- Format: deduplicate lengths of strings and byte arrays. 
- Format: prefix each sequence element of sequences of 
  unknown length with a marker to reliably detect the end of the sequence,
  even when types have zero size.
- A field type can now be changed from `char` to `String` and vice versa.
- Limit memory preallocation from size hints to prevent malicious data from
  overflowing a deserializer's memory
- Added `compact` module providing more efficient representations of common
  standard library types.
- Configuration is now passed as a value to `serialize` and `deserialize`,
  replacing the `Cfg` trait and its type parameter.
- Added a limit on the nesting depth of serialized and deserialized data,
  defaulting to `cfg::DEFAULT_DEPTH_LIMIT` (128).
- Recoverable deserialization via recoverable module allows replacements
  of values that failed deserialization either by their default or custom
  replacement values.
- Minimum supported Rust version (MSRV) is 1.95

## 0.4.3

- make #[serde(alias="...")] work in fast compile mode

## 0.4.2

- Reduced compile times by adding `#[inline(never)]` to serde trait implementation
  methods that are monomorphized per type (serializer and deserializer).
- Added optional `postbag_fast_compile` mode that uses buffered `visit_seq`
  instead of streaming `visit_map` for Full struct deserialization, further
  reducing compile times at the cost of buffering struct data in memory.
  Enable with `RUSTFLAGS="--cfg postbag_fast_compile"`.

## 0.4.1

- Implemented conversion from `Error` to `std::io::Error`.

## 0.4.0

- Added convenient API (`to_full_vec`, `from_full_slice`, `to_slim_vec`,
  `from_slim_slice`).

## 0.3.0

- Added `Full` configuration with forward/backward compatible encoding
  using field identifiers and skippable blocks.
- Added `Slim` configuration for compact positional encoding.
- Numerical identifier encoding for fields named `_0` through `_59`.

## 0.1.0

- Initial release with basic serde serialization and deserialization.

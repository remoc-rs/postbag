# Changelog

## 1.0.0

- Added `compact` module providing more efficient representations of common
  standard library types.
- Configuration is now passed as a value to `serialize` and `deserialize`,
  replacing the `Cfg` trait and its type parameter.
- Added `to_vec` and `from_slice`, which take a configuration.
- Added a limit on the nesting depth of serialized and deserialized data,
  defaulting to `cfg::DEFAULT_DEPTH_LIMIT` (128).
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
